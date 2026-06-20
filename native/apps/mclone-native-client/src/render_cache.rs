use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result, bail};
use mclone_assets::{AssetSourceChain, FilesystemAssetSource, PackedAssetSource};
use mclone_mesh::{
    TextureAtlasImage as MeshTextureAtlasImage, TexturedMeshCatalog, TexturedRenderSectionMesh,
    VisibilityGraphBuildStats, load_textured_terrain_assets,
};
use mclone_render::chunk::ChunkTextureAtlas;
use mclone_render_session::{
    RenderSectionCompileRequest, RenderSectionCompileResult, build_render_sections_from_snapshots,
};

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
