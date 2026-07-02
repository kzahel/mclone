use std::collections::{BTreeSet, VecDeque};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use mclone_assets::{AssetSource, AssetSourceChain, FilesystemAssetSource, PackedAssetSource};
use mclone_mesh::{
    TextureAtlasImage as MeshTextureAtlasImage, TexturedMeshCatalog, TexturedRenderSectionMesh,
    VisibilityGraphBuildStats, load_textured_terrain_assets,
};
pub use mclone_render::actor_assets::ActorTextureAssets;
use mclone_render::actor_assets::load_actor_texture_assets as load_actor_texture_assets_from_source;
use mclone_render::chunk::ChunkTextureAtlas;
use mclone_render_session::{
    RenderSectionCompileQueueHealth, RenderSectionCompileRequest, RenderSectionCompileResult,
    RenderSectionCompileSubmitTiming, RenderSectionCompiler, build_render_sections_from_snapshots,
};

use crate::elapsed_ms;
use crate::far_lod::FarTerrainLodMaterialPalette;
pub const DEFAULT_REFERENCE_ASSET_VERSION: &str = "1.17.1";
pub const DEFAULT_REFERENCE_PACK_FILE: &str = "extracted.zip";
pub const DEFAULT_NAMED_PACK_FILE: &str = "mclone-game-1.17.1.pbp";
pub const DEFAULT_OVERLAY_PACK_FILE: &str = "mclone-default-overlay.pbp";
pub const DEFAULT_ANDROID_APP_ID: &str = "com.kzahel.mclone";
pub const DEFAULT_FIRST_PARTY_ASSET_DIR: &str = "assets";
pub use crate::DEFAULT_RENDER_SECTION_COMPILE_WORKERS;

#[derive(Clone, Debug)]
pub struct SceneTexturedSections {
    pub sections: Vec<TexturedRenderSectionMesh>,
    pub visibility_graph_stats: VisibilityGraphBuildStats,
    pub atlas: TextureAtlasImage,
}

impl SceneTexturedSections {
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn index_count(&self) -> u32 {
        self.sections
            .iter()
            .map(|section| section.stats().index_count)
            .sum()
    }
}

#[derive(Clone, Debug)]
pub struct TextureAtlasImage {
    pub width: u32,
    pub height: u32,
    rgba: Vec<u8>,
}

impl TextureAtlasImage {
    pub fn as_upload(&self) -> ChunkTextureAtlas<'_> {
        ChunkTextureAtlas {
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
        }
    }
}

impl From<MeshTextureAtlasImage> for TextureAtlasImage {
    fn from(value: MeshTextureAtlasImage) -> Self {
        Self {
            width: value.width,
            height: value.height,
            rgba: value.rgba,
        }
    }
}

#[derive(Debug)]
struct RenderSectionCompileQueue {
    state: Mutex<RenderSectionCompileQueueState>,
    work_available: Condvar,
}

#[derive(Debug)]
struct RenderSectionCompileQueueState {
    request_slots: Vec<Option<RenderSectionCompileRequest>>,
    queued_slots: VecDeque<usize>,
    shutdown: bool,
}

impl RenderSectionCompileQueue {
    fn new(slot_count: usize) -> Self {
        Self {
            state: Mutex::new(RenderSectionCompileQueueState {
                request_slots: vec![None; slot_count],
                queued_slots: VecDeque::new(),
                shutdown: false,
            }),
            work_available: Condvar::new(),
        }
    }

    fn enqueue(&self, request: RenderSectionCompileRequest) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("render section compile queue lock poisoned"))?;
        if state.shutdown {
            bail!("render section compile queue is shutting down");
        }
        let Some(slot_index) = state.request_slots.iter().position(Option::is_none) else {
            bail!("render section compile queue has no free request slots");
        };
        state.request_slots[slot_index] = Some(request);
        state.queued_slots.push_back(slot_index);
        self.work_available.notify_one();
        Ok(())
    }

    fn take_next(&self) -> Option<RenderSectionCompileRequest> {
        let mut state = self.state.lock().ok()?;
        loop {
            if state.shutdown {
                return None;
            }
            if let Some(slot_index) = state.queued_slots.pop_front() {
                if let Some(request) = state.request_slots[slot_index].take() {
                    return Some(request);
                }
                continue;
            }
            state = self.work_available.wait(state).ok()?;
        }
    }

    fn queued_task_count(&self) -> usize {
        self.state
            .lock()
            .map(|state| state.queued_slots.len())
            .unwrap_or_default()
    }

    fn shutdown(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.shutdown = true;
            state.queued_slots.clear();
            for slot in &mut state.request_slots {
                *slot = None;
            }
        }
        self.work_available.notify_all();
    }
}

#[derive(Debug)]
pub struct RenderSectionCompileWorker {
    queue: Arc<RenderSectionCompileQueue>,
    receiver: mpsc::Receiver<RenderSectionCompileResult>,
    handles: Vec<thread::JoinHandle<()>>,
    pending_jobs: usize,
    max_pending_jobs: usize,
}

#[derive(Debug)]
pub struct NativeRenderSectionCompileDispatcher {
    worker: RenderSectionCompileWorker,
}

impl NativeRenderSectionCompileDispatcher {
    pub fn new(catalog: TexturedMeshCatalog) -> Result<Self> {
        Self::with_worker_count(catalog, DEFAULT_RENDER_SECTION_COMPILE_WORKERS)
    }

    pub fn with_worker_count(catalog: TexturedMeshCatalog, worker_count: usize) -> Result<Self> {
        Ok(Self {
            worker: RenderSectionCompileWorker::with_worker_count(catalog, worker_count)?,
        })
    }

    pub fn pending_job_count(&self) -> usize {
        self.worker.pending_job_count()
    }

    pub fn max_pending_job_count(&self) -> usize {
        self.worker.max_pending_job_count()
    }

    pub fn available_pending_job_slots(&self) -> usize {
        self.worker.available_pending_job_slots()
    }

    pub fn queue_health(&self) -> RenderSectionCompileQueueHealth {
        RenderSectionCompileQueueHealth::from_compiler(&self.worker)
    }
}

impl RenderSectionCompiler for NativeRenderSectionCompileDispatcher {
    fn submit(&mut self, request: RenderSectionCompileRequest) -> Result<()> {
        self.worker.submit(request)
    }

    fn submit_with_timing(
        &mut self,
        request: RenderSectionCompileRequest,
    ) -> Result<RenderSectionCompileSubmitTiming> {
        self.worker.submit_with_timing(request)
    }

    fn try_recv_completed(&mut self) -> Result<Vec<RenderSectionCompileResult>> {
        self.worker.try_recv_completed()
    }

    fn pending_job_count(&self) -> usize {
        self.worker.pending_job_count()
    }

    fn max_pending_job_count(&self) -> usize {
        self.worker.max_pending_job_count()
    }

    fn queued_compile_task_count(&self) -> usize {
        self.worker.queued_compile_task_count()
    }
}

impl RenderSectionCompileWorker {
    pub fn new(catalog: TexturedMeshCatalog) -> Result<Self> {
        Self::with_worker_count(catalog, DEFAULT_RENDER_SECTION_COMPILE_WORKERS)
    }

    pub fn with_worker_count(catalog: TexturedMeshCatalog, worker_count: usize) -> Result<Self> {
        if worker_count == 0 {
            bail!("render section compile worker count must be greater than zero");
        }

        let queue = Arc::new(RenderSectionCompileQueue::new(worker_count));
        let (result_sender, receiver) = mpsc::channel::<RenderSectionCompileResult>();
        let mut handles = Vec::with_capacity(worker_count);

        for worker_index in 0..worker_count {
            let catalog = catalog.clone();
            let queue = Arc::clone(&queue);
            let result_sender = result_sender.clone();
            let thread_name = if worker_count == 1 {
                "mclone-render-compile".to_string()
            } else {
                format!("mclone-render-compile-{worker_index}")
            };
            let handle = thread::Builder::new()
                .name(thread_name)
                .spawn(move || {
                    while let Some(request) = queue.take_next() {
                        let result = compile_render_section_request(&catalog, request);
                        if result_sender.send(result).is_err() {
                            break;
                        }
                    }
                })
                .context("failed to spawn render section compile worker")?;
            handles.push(handle);
        }

        Ok(Self {
            queue,
            receiver,
            handles,
            pending_jobs: 0,
            max_pending_jobs: worker_count,
        })
    }

    pub fn pending_job_count(&self) -> usize {
        self.pending_jobs
    }

    pub fn max_pending_job_count(&self) -> usize {
        self.max_pending_jobs
    }

    pub fn available_pending_job_slots(&self) -> usize {
        self.max_pending_jobs.saturating_sub(self.pending_jobs)
    }
}

impl RenderSectionCompiler for RenderSectionCompileWorker {
    fn submit(&mut self, request: RenderSectionCompileRequest) -> Result<()> {
        self.submit_with_timing(request).map(|_| ())
    }

    fn submit_with_timing(
        &mut self,
        request: RenderSectionCompileRequest,
    ) -> Result<RenderSectionCompileSubmitTiming> {
        let capacity_start = Instant::now();
        if self.pending_jobs >= self.max_pending_jobs {
            bail!(
                "render section compile worker has no free compile slots \
                 (pending_jobs={}, max_pending_jobs={})",
                self.pending_jobs,
                self.max_pending_jobs
            );
        }
        let capacity_check_ms = elapsed_ms(capacity_start.elapsed());

        let send_start = Instant::now();
        self.queue
            .enqueue(request)
            .context("failed to submit render section compile task")?;
        let command_send_ms = elapsed_ms(send_start.elapsed());

        let pending_start = Instant::now();
        self.pending_jobs += 1;
        let pending_mark_ms = elapsed_ms(pending_start.elapsed());

        Ok(RenderSectionCompileSubmitTiming {
            capacity_check_ms,
            command_send_ms,
            pending_mark_ms,
        })
    }

    fn try_recv_completed(&mut self) -> Result<Vec<RenderSectionCompileResult>> {
        let mut completed = Vec::new();
        loop {
            match self.receiver.try_recv() {
                Ok(result) => {
                    self.pending_jobs = self.pending_jobs.saturating_sub(1);
                    completed.push(result);
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(completed),
                Err(mpsc::TryRecvError::Disconnected) => {
                    bail!("render section compile worker disconnected")
                }
            }
        }
    }

    fn pending_job_count(&self) -> usize {
        self.pending_jobs
    }

    fn max_pending_job_count(&self) -> usize {
        self.max_pending_jobs
    }

    fn queued_compile_task_count(&self) -> usize {
        self.queue.queued_task_count()
    }
}

impl Drop for RenderSectionCompileWorker {
    fn drop(&mut self) {
        self.queue.shutdown();
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

fn compile_render_section_request(
    catalog: &TexturedMeshCatalog,
    request: RenderSectionCompileRequest,
) -> RenderSectionCompileResult {
    let result =
        build_render_sections_from_snapshots(&request.snapshots, catalog, &request.target_sections)
            .map_err(|error| format!("{error:#}"));
    RenderSectionCompileResult {
        target_sections: request.target_sections,
        section_revisions: request.section_revisions,
        result,
    }
}

#[derive(Clone, Debug)]
pub struct TexturedMeshAssets {
    pub catalog: TexturedMeshCatalog,
    pub atlas: TextureAtlasImage,
    pub far_lod_materials: Option<FarTerrainLodMaterialPalette>,
}

pub fn load_textured_mesh_assets() -> Result<TexturedMeshAssets> {
    let source = load_asset_source()?;
    load_textured_mesh_assets_from_source(&source)
}

pub fn load_textured_mesh_assets_from_source(
    source: &impl AssetSource,
) -> Result<TexturedMeshAssets> {
    let assets =
        load_textured_terrain_assets(source).context("failed to load textured terrain assets")?;
    let far_lod_materials = FarTerrainLodMaterialPalette::load_from_asset_source(source)
        .context("failed to load Far LOD material metadata")?;

    Ok(TexturedMeshAssets {
        catalog: assets.catalog,
        atlas: assets.atlas.into(),
        far_lod_materials,
    })
}

pub fn load_actor_texture_assets() -> Result<ActorTextureAssets> {
    let source = load_asset_source()?;
    load_actor_texture_assets_from_source(&source).context("failed to load actor texture assets")
}

pub fn load_actor_texture_assets_from_asset_source(
    source: &impl AssetSource,
) -> Result<ActorTextureAssets> {
    load_actor_texture_assets_from_source(source).context("failed to load actor texture assets")
}

pub fn extracted_asset_root() -> PathBuf {
    repo_root().join(format!(
        "reference/minecraft-{DEFAULT_REFERENCE_ASSET_VERSION}/extracted"
    ))
}

pub fn default_asset_pack_path() -> PathBuf {
    repo_root().join(format!(
        "reference/minecraft-{DEFAULT_REFERENCE_ASSET_VERSION}/{DEFAULT_REFERENCE_PACK_FILE}"
    ))
}

pub fn default_android_external_files_dir() -> PathBuf {
    PathBuf::from(format!(
        "/sdcard/Android/data/{DEFAULT_ANDROID_APP_ID}/files"
    ))
}

pub fn default_local_sound_asset_root() -> PathBuf {
    repo_root().join(format!(
        "reference/minecraft-{DEFAULT_REFERENCE_ASSET_VERSION}/local-sounds"
    ))
}

pub fn legacy_sound_overlay_root() -> PathBuf {
    repo_root().join(format!(
        "reference/minecraft-{DEFAULT_REFERENCE_ASSET_VERSION}/sound-overlay"
    ))
}

pub fn load_asset_source() -> Result<AssetSourceChain> {
    let mode = AssetMode::from_env()?;
    let mut source = AssetSourceChain::new();

    for overlay_pack in load_overlay_pack_asset_sources()? {
        source.push(overlay_pack);
    }

    match mode {
        AssetMode::LooseFirst => {
            if let Some(first_party) = load_first_party_asset_source()? {
                source.push(first_party);
            }
            if let Some(loose) = load_loose_asset_source()? {
                source.push(loose);
            }
            if let Some(pack) = load_pack_asset_source(false)? {
                source.push(pack);
            }
        }
        AssetMode::PackFirst => {
            if let Some(pack) = load_pack_asset_source(false)? {
                source.push(pack);
            }
            if let Some(first_party) = load_first_party_asset_source()? {
                source.push(first_party);
            }
            if let Some(loose) = load_loose_asset_source()? {
                source.push(loose);
            }
        }
        AssetMode::PackOnly => {
            source.push(load_pack_asset_source(true)?.ok_or_else(|| {
                anyhow::anyhow!(
                    "missing Minecraft asset pack; run `pnpm assets:pack` from the repository root"
                )
            })?);
        }
        AssetMode::LooseOnly => {
            if let Some(first_party) = load_first_party_asset_source()? {
                source.push(first_party);
            }
            source.push(load_loose_asset_source()?.ok_or_else(|| {
                anyhow::anyhow!(
                    "missing extracted Minecraft assets at {}; run ./scripts/decompile-mc.sh from the repository root",
                    extracted_asset_root().display()
                )
            })?);
        }
    }

    if source.is_empty() {
        bail!(
            "missing Minecraft assets; run ./scripts/decompile-mc.sh and `pnpm assets:pack` from the repository root"
        );
    }
    if let Some(local_sounds) = load_local_sound_asset_source()? {
        source.push(local_sounds);
    }
    Ok(source)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssetMode {
    LooseFirst,
    PackFirst,
    PackOnly,
    LooseOnly,
}

impl AssetMode {
    fn from_env() -> Result<Self> {
        match env::var("MCLONE_ASSET_MODE") {
            Ok(value) => Self::parse(&value),
            Err(env::VarError::NotPresent) => Ok(Self::LooseFirst),
            Err(error) => Err(error).context("failed to read MCLONE_ASSET_MODE"),
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "loose-first" => Ok(Self::LooseFirst),
            "pack-first" => Ok(Self::PackFirst),
            "pack-only" => Ok(Self::PackOnly),
            "loose-only" => Ok(Self::LooseOnly),
            other => bail!(
                "invalid MCLONE_ASSET_MODE `{other}`; expected loose-first, pack-first, pack-only, or loose-only"
            ),
        }
    }
}

fn load_loose_asset_source() -> Result<Option<FilesystemAssetSource>> {
    for root in loose_asset_roots()? {
        if root.join("assets").is_dir() {
            return Ok(Some(FilesystemAssetSource::new(root)));
        }
    }
    Ok(None)
}

fn load_first_party_asset_source() -> Result<Option<FilesystemAssetSource>> {
    if let Some(root) = env_path("MCLONE_FIRST_PARTY_ASSET_ROOT") {
        if root.join(DEFAULT_FIRST_PARTY_ASSET_DIR).is_dir() {
            log::info!("loaded first-party assets {}", root.display());
            return Ok(Some(FilesystemAssetSource::new(root)));
        }
        bail!(
            "MCLONE_FIRST_PARTY_ASSET_ROOT points to {}, but it does not contain an assets/ directory",
            root.display()
        );
    }

    let root = repo_root();
    if root.join(DEFAULT_FIRST_PARTY_ASSET_DIR).is_dir() {
        log::info!("loaded first-party assets {}", root.display());
        return Ok(Some(FilesystemAssetSource::new(root)));
    }
    Ok(None)
}

fn load_pack_asset_source(required: bool) -> Result<Option<PackedAssetSource>> {
    let (candidates, explicit) = asset_pack_candidates()?;
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        match PackedAssetSource::from_file(&path) {
            Ok(pack) => {
                log::info!(
                    "loaded Minecraft asset pack {} (asset_set={}, files={})",
                    path.display(),
                    pack.manifest().asset_set,
                    pack.file_count()
                );
                return Ok(Some(pack));
            }
            Err(error) if required || explicit => {
                return Err(error).with_context(|| {
                    format!("failed to load Minecraft asset pack {}", path.display())
                });
            }
            Err(error) => {
                log::warn!(
                    "ignoring invalid optional Minecraft asset pack {}: {error}",
                    path.display()
                );
            }
        }
    }
    if required || explicit {
        bail!(
            "no readable Minecraft asset pack found; checked {}",
            asset_pack_candidates()?
                .0
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(None)
}

fn load_overlay_pack_asset_sources() -> Result<Vec<PackedAssetSource>> {
    let (candidates, explicit) = overlay_pack_candidates();
    let mut packs = Vec::new();
    for path in candidates {
        if !path.is_file() {
            if explicit {
                bail!(
                    "MCLONE_ASSET_OVERLAY_PACK entry is not a file: {}",
                    path.display()
                );
            }
            continue;
        }
        match PackedAssetSource::from_file(&path) {
            Ok(pack) => {
                log::info!(
                    "loaded mclone asset overlay pack {} (asset_set={}, files={})",
                    path.display(),
                    pack.manifest().asset_set,
                    pack.file_count()
                );
                packs.push(pack);
            }
            Err(error) if explicit => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to load mclone asset overlay pack {}",
                        path.display()
                    )
                });
            }
            Err(error) => {
                log::warn!(
                    "ignoring invalid optional mclone asset overlay pack {}: {error}",
                    path.display()
                );
            }
        }
    }
    Ok(packs)
}

fn load_local_sound_asset_source() -> Result<Option<FilesystemAssetSource>> {
    if let Some(root) = env_path("MCLONE_SOUND_ASSET_ROOT") {
        if root.join("assets").is_dir() {
            log::info!("loaded local sound assets {}", root.display());
            return Ok(Some(FilesystemAssetSource::new(root)));
        }
        bail!(
            "MCLONE_SOUND_ASSET_ROOT points to {}, but it does not contain an assets/ directory",
            root.display()
        );
    }

    for root in local_sound_asset_roots() {
        if root.join("assets").is_dir() {
            log::info!("loaded local sound assets {}", root.display());
            return Ok(Some(FilesystemAssetSource::new(root)));
        }
    }
    Ok(None)
}

fn local_sound_asset_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for root in platform_configured_asset_roots() {
        roots.push(root.join("assets/local-sounds"));
        roots.push(root.join("assets/sound-overlay"));
    }
    roots.push(default_local_sound_asset_root());
    roots.push(legacy_sound_overlay_root());
    dedup_paths(roots)
}

fn loose_asset_roots() -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    roots.extend(platform_configured_asset_roots());
    roots.push(extracted_asset_root());
    Ok(dedup_paths(roots))
}

fn asset_pack_candidates() -> Result<(Vec<PathBuf>, bool)> {
    let explicit = env::var_os("MCLONE_ASSET_PACK");
    if let Some(paths) = explicit {
        return Ok((dedup_paths(env::split_paths(&paths).collect()), true));
    }

    let mut candidates = Vec::new();
    let roots = platform_configured_asset_roots();

    for root in roots {
        candidates.push(root.join(DEFAULT_REFERENCE_PACK_FILE));
        candidates.push(root.join("assets/packs").join(DEFAULT_NAMED_PACK_FILE));
        candidates.push(root.join("assets/packs").join(DEFAULT_REFERENCE_PACK_FILE));
    }

    let root = repo_root();
    candidates.push(default_asset_pack_path());
    candidates.push(root.join("assets/packs").join(DEFAULT_NAMED_PACK_FILE));
    candidates.push(root.join("assets/packs").join(DEFAULT_REFERENCE_PACK_FILE));
    Ok((dedup_paths(candidates), false))
}

fn overlay_pack_candidates() -> (Vec<PathBuf>, bool) {
    let explicit = env::var_os("MCLONE_ASSET_OVERLAY_PACK");
    if let Some(paths) = explicit {
        if paths.is_empty() {
            return (Vec::new(), false);
        }
        return (dedup_paths(env::split_paths(&paths).collect()), true);
    }

    let mut candidates = Vec::new();
    for root in platform_configured_asset_roots() {
        candidates.push(root.join("assets/packs").join(DEFAULT_OVERLAY_PACK_FILE));
    }
    candidates.push(
        repo_root()
            .join("assets/packs")
            .join(DEFAULT_OVERLAY_PACK_FILE),
    );
    (dedup_paths(candidates), false)
}

fn platform_configured_asset_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = env_path("MCLONE_ASSET_ROOT") {
        roots.push(root);
    }
    if let Some(root) = env_path("MCLONE_ANDROID_ASSET_ROOT") {
        roots.push(root);
    }
    roots.extend(platform_asset_roots());
    dedup_paths(roots)
}

#[cfg(target_os = "android")]
fn platform_asset_roots() -> Vec<PathBuf> {
    vec![
        default_android_external_files_dir(),
        PathBuf::from(format!(
            "/storage/emulated/0/Android/data/{DEFAULT_ANDROID_APP_ID}/files"
        )),
    ]
}

#[cfg(not(target_os = "android"))]
fn platform_asset_roots() -> Vec<PathBuf> {
    Vec::new()
}

fn env_path(name: &str) -> Option<PathBuf> {
    match env::var_os(name) {
        Some(value) if value.is_empty() => None,
        Some(value) => Some(PathBuf::from(value)),
        None => None,
    }
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            deduped.push(path);
        }
    }
    deduped
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    fn empty_compile_request() -> RenderSectionCompileRequest {
        RenderSectionCompileRequest {
            target_sections: BTreeSet::new(),
            section_revisions: BTreeMap::new(),
            snapshots: Vec::new(),
        }
    }

    #[test]
    fn render_compile_worker_rejects_zero_slots() {
        let error =
            RenderSectionCompileWorker::with_worker_count(TexturedMeshCatalog::default(), 0)
                .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("worker count must be greater than zero")
        );
    }

    #[test]
    fn render_compile_worker_exposes_bounded_capacity() -> Result<()> {
        let mut worker =
            RenderSectionCompileWorker::with_worker_count(TexturedMeshCatalog::default(), 2)?;
        assert_eq!(worker.pending_job_count(), 0);
        assert_eq!(worker.max_pending_job_count(), 2);
        assert_eq!(worker.available_pending_job_slots(), 2);

        worker.submit(empty_compile_request())?;
        worker.submit(empty_compile_request())?;
        assert_eq!(worker.pending_job_count(), 2);
        assert_eq!(worker.available_pending_job_slots(), 0);

        let error = worker.submit(empty_compile_request()).unwrap_err();
        assert!(error.to_string().contains("no free compile slots"));
        Ok(())
    }

    #[test]
    fn native_render_compile_dispatcher_wraps_worker_capacity() -> Result<()> {
        let mut dispatcher = NativeRenderSectionCompileDispatcher::with_worker_count(
            TexturedMeshCatalog::default(),
            2,
        )?;
        assert_eq!(dispatcher.pending_job_count(), 0);
        assert_eq!(dispatcher.max_pending_job_count(), 2);
        assert_eq!(dispatcher.available_pending_job_slots(), 2);

        dispatcher.submit(empty_compile_request())?;
        let health = dispatcher.queue_health();
        assert_eq!(health.pending_jobs, 1);
        assert_eq!(health.max_pending_jobs, 2);
        assert_eq!(health.available_job_slots, 1);
        assert!(health.queued_compile_tasks <= health.pending_jobs);
        Ok(())
    }
}
