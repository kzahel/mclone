use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result, bail};
use mclone_assets::{AssetSourceChain, FilesystemAssetSource, PackedAssetSource};
use mclone_client::ClientRuntime;
use mclone_core::{
    AIR_BLOCK_STATE_ID, CHUNK_SECTION_VOLUME, CHUNK_WIDTH, ChunkPos, ChunkSnapshot,
    PackedLightSection, SECTION_HEIGHT,
};
use mclone_mesh::{
    RenderSectionKey, TextureAtlasImage as MeshTextureAtlasImage, TexturedChunkMeshInput,
    TexturedMeshCatalog, TexturedRenderSectionBuildReport, TexturedRenderSectionMesh,
    VisibilityGraphBuildStats, build_textured_render_sections_for_section_set_with_stats,
    build_textured_render_sections_with_stats, load_textured_terrain_assets,
    quad_face_count_from_indices,
};
use mclone_render::chunk::ChunkTextureAtlas;

const DEFAULT_REFERENCE_ASSET_VERSION: &str = "1.17.1";
const DEFAULT_REFERENCE_PACK_FILE: &str = "extracted.zip";
const DEFAULT_NAMED_PACK_FILE: &str = "mclone-vanilla-1.17.1.pbp";

#[derive(Clone, Debug)]
pub(crate) struct SceneTexturedSections {
    pub(crate) sections: Vec<TexturedRenderSectionMesh>,
    pub(crate) visibility_graph_stats: VisibilityGraphBuildStats,
    pub(crate) atlas: TextureAtlasImage,
}

impl SceneTexturedSections {
    #[cfg(test)]
    pub(crate) fn section_count(&self) -> usize {
        self.sections.len()
    }

    #[cfg(test)]
    pub(crate) fn index_count(&self) -> u32 {
        self.sections
            .iter()
            .map(|section| section.stats().index_count)
            .sum()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TextureAtlasImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    rgba: Vec<u8>,
}

impl TextureAtlasImage {
    pub(crate) fn as_upload(&self) -> ChunkTextureAtlas<'_> {
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
pub(crate) fn build_client_textured_sections(
    client: &ClientRuntime,
    catalog: &TexturedMeshCatalog,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = mesh_chunks_from_client(client)?;
    let inputs = textured_mesh_inputs(&chunks);
    build_textured_render_sections_with_stats(&inputs, catalog)
        .context("failed to build textured sections")
}

fn build_render_sections_from_snapshots(
    snapshots: &[ChunkSnapshot],
    catalog: &TexturedMeshCatalog,
    target_sections: &BTreeSet<RenderSectionKey>,
) -> Result<TexturedRenderSectionBuildReport> {
    let chunks = snapshots
        .iter()
        .map(snapshot_mesh_block_state_ids)
        .collect::<Result<Vec<_>>>()?;
    let inputs = textured_mesh_inputs(&chunks);
    build_textured_render_sections_for_section_set_with_stats(&inputs, catalog, target_sections)
        .context("failed to build queued textured render sections")
}

fn mesh_chunks_from_client(client: &ClientRuntime) -> Result<Vec<MeshChunkBlocks>> {
    client
        .chunk_snapshots()
        .map(snapshot_mesh_block_state_ids)
        .collect::<Result<Vec<_>>>()
}

fn textured_mesh_inputs(chunks: &[MeshChunkBlocks]) -> Vec<TexturedChunkMeshInput<'_>> {
    chunks
        .iter()
        .map(|chunk| {
            TexturedChunkMeshInput::new(
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.min_y,
                chunk.height,
                &chunk.blocks,
            )
            .with_light_sections(&chunk.light_sections)
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CachedTexturedRenderSections {
    sections: BTreeMap<RenderSectionKey, TexturedRenderSectionMesh>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderSectionCacheUpdate {
    pub(crate) rebuilt_sections: Vec<TexturedRenderSectionMesh>,
    pub(crate) removed_section_keys: BTreeSet<RenderSectionKey>,
    pub(crate) rebuilt_vertex_count: u32,
    pub(crate) rebuilt_index_count: u32,
    pub(crate) neighbor_ready_section_count: usize,
    pub(crate) near_exception_section_count: usize,
    pub(crate) deferred_section_count: usize,
    pub(crate) submitted_compile_section_count: usize,
    pub(crate) completed_compile_section_count: usize,
    pub(crate) stale_compile_section_count: usize,
    pub(crate) pending_compile_jobs: usize,
    pub(crate) visibility_graph_stats: VisibilityGraphBuildStats,
}

impl RenderSectionCacheUpdate {
    pub(crate) fn rebuilt_section_count(&self) -> usize {
        self.rebuilt_sections.len()
    }

    pub(crate) fn removed_section_count(&self) -> usize {
        self.removed_section_keys.len()
    }

    pub(crate) fn rebuilt_face_count(&self) -> u32 {
        quad_face_count_from_indices(self.rebuilt_index_count)
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.rebuilt_sections.extend(other.rebuilt_sections);
        self.removed_section_keys.extend(other.removed_section_keys);
        self.rebuilt_vertex_count += other.rebuilt_vertex_count;
        self.rebuilt_index_count += other.rebuilt_index_count;
        self.neighbor_ready_section_count += other.neighbor_ready_section_count;
        self.near_exception_section_count += other.near_exception_section_count;
        self.deferred_section_count += other.deferred_section_count;
        self.submitted_compile_section_count += other.submitted_compile_section_count;
        self.completed_compile_section_count += other.completed_compile_section_count;
        self.stale_compile_section_count += other.stale_compile_section_count;
        self.pending_compile_jobs = other.pending_compile_jobs;
        self.visibility_graph_stats.build_count += other.visibility_graph_stats.build_count;
        self.visibility_graph_stats.total_ms += other.visibility_graph_stats.total_ms;
        self.visibility_graph_stats.worst_ms = self
            .visibility_graph_stats
            .worst_ms
            .max(other.visibility_graph_stats.worst_ms);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RenderSectionCompileRequest {
    pub(crate) target_sections: BTreeSet<RenderSectionKey>,
    pub(crate) section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub(crate) snapshots: Vec<ChunkSnapshot>,
}

#[derive(Debug)]
pub(crate) struct RenderSectionCompileResult {
    pub(crate) target_sections: BTreeSet<RenderSectionKey>,
    pub(crate) section_revisions: BTreeMap<RenderSectionKey, u64>,
    pub(crate) result: std::result::Result<TexturedRenderSectionBuildReport, String>,
}

enum RenderSectionCompileCommand {
    Build(RenderSectionCompileRequest),
    Shutdown,
}

#[derive(Debug)]
pub(crate) struct RenderSectionCompileWorker {
    sender: mpsc::Sender<RenderSectionCompileCommand>,
    receiver: mpsc::Receiver<RenderSectionCompileResult>,
    handle: Option<thread::JoinHandle<()>>,
    pending_jobs: usize,
}

impl RenderSectionCompileWorker {
    pub(crate) fn new(catalog: TexturedMeshCatalog) -> Result<Self> {
        let (sender, command_receiver) = mpsc::channel::<RenderSectionCompileCommand>();
        let (result_sender, receiver) = mpsc::channel::<RenderSectionCompileResult>();
        let handle = thread::Builder::new()
            .name("mclone-render-compile".to_string())
            .spawn(move || {
                while let Ok(command) = command_receiver.recv() {
                    match command {
                        RenderSectionCompileCommand::Build(request) => {
                            let result = build_render_sections_from_snapshots(
                                &request.snapshots,
                                &catalog,
                                &request.target_sections,
                            )
                            .map_err(|error| format!("{error:#}"));
                            if result_sender
                                .send(RenderSectionCompileResult {
                                    target_sections: request.target_sections,
                                    section_revisions: request.section_revisions,
                                    result,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        RenderSectionCompileCommand::Shutdown => break,
                    }
                }
            })
            .context("failed to spawn render section compile worker")?;
        Ok(Self {
            sender,
            receiver,
            handle: Some(handle),
            pending_jobs: 0,
        })
    }

    pub(crate) fn submit(&mut self, request: RenderSectionCompileRequest) -> Result<()> {
        self.sender
            .send(RenderSectionCompileCommand::Build(request))
            .context("failed to submit render section compile task")?;
        self.pending_jobs += 1;
        Ok(())
    }

    pub(crate) fn try_recv_completed(&mut self) -> Result<Vec<RenderSectionCompileResult>> {
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

    pub(crate) fn pending_job_count(&self) -> usize {
        self.pending_jobs
    }
}

impl Drop for RenderSectionCompileWorker {
    fn drop(&mut self) {
        let _ = self.sender.send(RenderSectionCompileCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl CachedTexturedRenderSections {
    pub(crate) fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub(crate) fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.sections
            .keys()
            .any(|key| key.chunk_x == pos.x && key.chunk_z == pos.z)
    }

    pub(crate) fn contains_section(&self, key: RenderSectionKey) -> bool {
        self.sections.contains_key(&key)
    }

    pub(crate) fn sections(&self) -> Vec<TexturedRenderSectionMesh> {
        self.sections.values().cloned().collect()
    }

    pub(crate) fn section_keys(&self) -> impl Iterator<Item = RenderSectionKey> + '_ {
        self.sections.keys().copied()
    }

    pub(crate) fn apply_build_report(
        &mut self,
        ready_section_keys: &BTreeSet<RenderSectionKey>,
        rebuilt_report: TexturedRenderSectionBuildReport,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        let rebuilt_keys = rebuilt_report
            .sections
            .iter()
            .map(|section| section.key)
            .collect::<BTreeSet<_>>();
        let rebuilt_sections = rebuilt_report
            .sections
            .into_iter()
            .filter(|section| ready_section_keys.contains(&section.key))
            .collect::<Vec<_>>();
        let old_ready_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| ready_section_keys.contains(key))
            .collect::<BTreeSet<_>>();
        let removal_keys = self
            .sections
            .keys()
            .copied()
            .filter(|key| removal_chunks.contains(&ChunkPos::new(key.chunk_x, key.chunk_z)))
            .collect::<BTreeSet<_>>();
        let mut removed_section_keys = old_ready_keys
            .difference(&rebuilt_keys)
            .copied()
            .collect::<BTreeSet<_>>();
        removed_section_keys.extend(removal_keys);
        removed_section_keys.extend(removal_section_keys.iter().copied());

        for key in &removed_section_keys {
            self.sections.remove(key);
        }

        let mut report = RenderSectionCacheUpdate {
            rebuilt_sections,
            removed_section_keys,
            rebuilt_vertex_count: 0,
            rebuilt_index_count: 0,
            visibility_graph_stats: rebuilt_report.visibility_graph,
            neighbor_ready_section_count: ready_section_keys.len(),
            completed_compile_section_count: ready_section_keys.len(),
            ..RenderSectionCacheUpdate::default()
        };
        for section in &report.rebuilt_sections {
            let stats = section.stats();
            report.rebuilt_vertex_count += stats.vertex_count;
            report.rebuilt_index_count += stats.index_count;
            self.sections.insert(section.key, section.clone());
        }
        report
    }

    pub(crate) fn remove_sections(
        &mut self,
        removal_chunks: &BTreeSet<ChunkPos>,
        removal_section_keys: &BTreeSet<RenderSectionKey>,
    ) -> RenderSectionCacheUpdate {
        self.apply_build_report(
            &BTreeSet::new(),
            TexturedRenderSectionBuildReport::default(),
            removal_chunks,
            removal_section_keys,
        )
    }
}
#[derive(Clone, Debug)]
pub(crate) struct TexturedMeshAssets {
    pub(crate) catalog: TexturedMeshCatalog,
    pub(crate) atlas: TextureAtlasImage,
}

pub(crate) fn load_textured_mesh_assets() -> Result<TexturedMeshAssets> {
    let source = load_asset_source()?;
    let assets =
        load_textured_terrain_assets(&source).context("failed to load textured terrain assets")?;

    Ok(TexturedMeshAssets {
        catalog: assets.catalog,
        atlas: assets.atlas.into(),
    })
}

pub(crate) fn extracted_asset_root() -> PathBuf {
    repo_root().join(format!(
        "reference/minecraft-{DEFAULT_REFERENCE_ASSET_VERSION}/extracted"
    ))
}

pub(crate) fn default_asset_pack_path() -> PathBuf {
    repo_root().join(format!(
        "reference/minecraft-{DEFAULT_REFERENCE_ASSET_VERSION}/{DEFAULT_REFERENCE_PACK_FILE}"
    ))
}

pub(crate) fn load_asset_source() -> Result<AssetSourceChain> {
    let mode = AssetMode::from_env()?;
    let mut source = AssetSourceChain::new();

    match mode {
        AssetMode::LooseFirst => {
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

fn loose_asset_roots() -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    if let Some(root) = env_path("MCLONE_ASSET_ROOT") {
        roots.push(root);
    }
    roots.push(extracted_asset_root());
    Ok(dedup_paths(roots))
}

fn asset_pack_candidates() -> Result<(Vec<PathBuf>, bool)> {
    let explicit = env::var_os("MCLONE_ASSET_PACK");
    if let Some(paths) = explicit {
        return Ok((dedup_paths(env::split_paths(&paths).collect()), true));
    }

    let mut candidates = Vec::new();
    if let Some(root) = env_path("MCLONE_ASSET_ROOT") {
        candidates.push(root.join(DEFAULT_REFERENCE_PACK_FILE));
        candidates.push(root.join("assets/packs").join(DEFAULT_NAMED_PACK_FILE));
    }

    let root = repo_root();
    candidates.push(default_asset_pack_path());
    candidates.push(root.join("assets/packs").join(DEFAULT_NAMED_PACK_FILE));
    candidates.push(root.join("assets/packs").join(DEFAULT_REFERENCE_PACK_FILE));
    Ok((dedup_paths(candidates), false))
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

#[derive(Clone, Debug)]
pub(crate) struct MeshChunkBlocks {
    pub(crate) chunk_x: i32,
    pub(crate) chunk_z: i32,
    pub(crate) min_y: i32,
    pub(crate) height: i32,
    pub(crate) blocks: Vec<mclone_core::BlockStateId>,
    pub(crate) light_sections: Vec<PackedLightSection>,
}

pub(crate) fn snapshot_mesh_block_state_ids(snapshot: &ChunkSnapshot) -> Result<MeshChunkBlocks> {
    if snapshot.height <= 0 || snapshot.height % SECTION_HEIGHT != 0 {
        bail!(
            "chunk snapshot {:?} has invalid height {}",
            snapshot.pos,
            snapshot.height
        );
    }
    let expected_len = snapshot.height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
    let mut blocks = vec![AIR_BLOCK_STATE_ID; expected_len];
    let min_section_y = snapshot.min_y / SECTION_HEIGHT;
    let section_count = snapshot.height / SECTION_HEIGHT;

    for section in &snapshot.sections {
        let section_offset = section.section_y - min_section_y;
        if !(0..section_count).contains(&section_offset) {
            bail!(
                "chunk snapshot {:?} contains section {} outside {}..{}",
                snapshot.pos,
                section.section_y,
                min_section_y,
                min_section_y + section_count - 1
            );
        }
        let unpacked = section.unpack_block_state_ids();
        if unpacked.len() != CHUNK_SECTION_VOLUME {
            bail!(
                "chunk snapshot {:?} section {} unpacked to {} blocks",
                snapshot.pos,
                section.section_y,
                unpacked.len()
            );
        }
        let start = section_offset as usize * CHUNK_SECTION_VOLUME;
        for (index, state_id) in unpacked.into_iter().enumerate() {
            blocks[start + index] = state_id;
        }
    }

    Ok(MeshChunkBlocks {
        chunk_x: snapshot.pos.x,
        chunk_z: snapshot.pos.z,
        min_y: snapshot.min_y,
        height: snapshot.height,
        blocks,
        light_sections: snapshot.light_sections.clone(),
    })
}
