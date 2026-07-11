#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use anyhow::{Context, Result, bail};
use mclone_assets::{AssetPath, AssetSource};
use mclone_core::{CHUNK_WIDTH, ChunkPos, LodTileKey, chunk_min_block_coord};
use mclone_render::far_lod::{FarTerrainLodFrameUpdate, FarTerrainLodTileMesh};
use mclone_render_session::{
    ResidentTileCache, ResidentTileUploadCoordinator, ResidentTileUploadPayload,
};
use mclone_worldgen::block::{
    ANDESITE, BEDROCK, BLACK_TERRACOTTA, BLUE_TERRACOTTA, BROWN_TERRACOTTA, CLAY, COARSE_DIRT,
    CYAN_TERRACOTTA, DIORITE, DIRT, GRANITE, GRASS_BLOCK, GRAVEL, GRAY_TERRACOTTA,
    GREEN_TERRACOTTA, GeneratedBlockId, ICE, LIGHT_BLUE_TERRACOTTA, LIGHT_GRAY_TERRACOTTA,
    LIME_TERRACOTTA, MAGENTA_TERRACOTTA, MYCELIUM, ORANGE_TERRACOTTA, PACKED_ICE, PINK_TERRACOTTA,
    PODZOL, PURPLE_TERRACOTTA, RED_SAND, RED_SANDSTONE, RED_TERRACOTTA, SAND, SANDSTONE, SNOW,
    SNOW_BLOCK, STONE, TERRACOTTA, WHITE_TERRACOTTA, YELLOW_TERRACOTTA, base_block_id, has_fluid,
    is_air_like, is_lava, is_water,
};
use mclone_worldgen::levelgen::{GeneratedChunk, generate_overworld_surface_chunk};
use serde::Deserialize;

use crate::monotonic::MonotonicInstant;

pub const DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS: u32 = 0;
pub const MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 1;
pub const DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 12;
pub const MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 64;
pub const DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS: u32 = 4;
pub const DEFAULT_FAR_TERRAIN_LOD_CHUNK_BUILD_BUDGET: usize = 4;
pub const DEFAULT_FAR_TERRAIN_LOD_EVICTION_MARGIN_CHUNKS: u32 = 2;
pub const MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES: usize = 4096;

/// Startup visual-coverage prewarm defaults (tactical 162 Slice 1). These bound
/// how much cheap synthetic LOD is built before the first playable frame so the
/// initial view has less blank space, without letting slow LOD delay entry into
/// play. The prewarm is presentation-only: it never satisfies spawn authority.
pub const DEFAULT_STARTUP_LOD_PREWARM_EXTRA_CHUNKS: u32 = 5;
pub const DEFAULT_STARTUP_LOD_PREWARM_TIME_CAP: Duration = Duration::from_millis(2000);
pub const DEFAULT_STARTUP_LOD_PREWARM_TILE_CAP: usize = 1024;
/// Chunk patches built per prewarm step. The pump calls the prewarm each poll,
/// so this is a per-step burst bounded by the overall time/tile caps.
pub const STARTUP_LOD_PREWARM_CHUNK_BUILD_BUDGET: usize = 24;

pub const SEA_LEVEL: f32 = 63.0;
pub const FAR_TERRAIN_LOD_MATERIALS_PATH: &str = "assets/mclone/lod/materials.v1.json";

#[cfg(not(target_arch = "wasm32"))]
fn far_lod_now() -> MonotonicInstant {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    let elapsed = ORIGIN.get_or_init(Instant::now).elapsed();
    MonotonicInstant::from_nanos(u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX))
}

#[cfg(target_arch = "wasm32")]
fn far_lod_now() -> MonotonicInstant {
    MonotonicInstant::ZERO
}

#[derive(Clone, Debug)]
pub struct FarTerrainLodBuildRequest {
    pub key: LodTileKey,
    source_key: FarTerrainLodSourceKey,
    materials: Option<Arc<FarTerrainLodMaterialPalette>>,
    queued_at: MonotonicInstant,
}

/// Portable payload a platform compiler needs to build one synthetic tile.
///
/// Queue timing, source identity, and material ownership stay with the request
/// on the host. Browser workers receive only these small deterministic inputs
/// and return a packed [`FarTerrainLodTileMesh`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FarTerrainLodWorkerInput {
    pub key: LodTileKey,
    pub seed: i64,
    pub sample_spacing_blocks: u32,
}

impl FarTerrainLodBuildRequest {
    fn new(
        pos: ChunkPos,
        source_key: FarTerrainLodSourceKey,
        materials: Option<Arc<FarTerrainLodMaterialPalette>>,
        queued_at: MonotonicInstant,
    ) -> Self {
        Self {
            key: LodTileKey::synthetic(pos),
            source_key,
            materials,
            queued_at,
        }
    }

    #[cfg(test)]
    pub(crate) fn synthetic_test_request(pos: ChunkPos) -> Self {
        Self::new(
            pos,
            FarTerrainLodSourceKey::new(12345, FarTerrainLodConfig::enabled(), false),
            None,
            far_lod_now(),
        )
    }

    pub fn worker_input(&self) -> FarTerrainLodWorkerInput {
        FarTerrainLodWorkerInput {
            key: self.key,
            seed: self.source_key.seed,
            sample_spacing_blocks: self.source_key.sample_spacing_blocks,
        }
    }

    pub fn complete_worker_mesh(
        self,
        mesh: FarTerrainLodTileMesh,
    ) -> Result<FarTerrainLodBuildResult> {
        if mesh.key() != self.key {
            bail!(
                "far LOD worker returned tile {:?} for request {:?}",
                mesh.key(),
                self.key
            );
        }
        Ok(FarTerrainLodBuildResult {
            key: self.key,
            queued_at: self.queued_at,
            mesh,
            source_key: self.source_key,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FarTerrainLodBuildResult {
    pub key: LodTileKey,
    pub queued_at: MonotonicInstant,
    pub mesh: FarTerrainLodTileMesh,
    source_key: FarTerrainLodSourceKey,
}

pub trait FarTerrainLodCompiler {
    fn available_far_lod_job_slots(&self) -> usize;
    fn submit_far_lod(&mut self, request: FarTerrainLodBuildRequest) -> Result<()>;
    fn try_recv_completed_far_lod(&mut self) -> Result<Vec<FarTerrainLodBuildResult>>;
    fn release_completed_far_lod_jobs(&mut self, count: usize) -> usize;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FarTerrainLodProducerStats {
    pub desired_tiles: usize,
    pub resident_tiles: usize,
    pub visible_tiles: usize,
    pub pending_builds: usize,
    pub inflight_builds: usize,
    pub oldest_build_age_ms: Option<f64>,
    pub submitted_builds: u64,
    pub completed_builds: u64,
    pub stale_builds: u64,
    pub queued_uploads: usize,
    pub last_upload_bytes: usize,
    pub total_upload_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FarTerrainLodConfig {
    pub enabled: bool,
    pub start_margin_chunks: u32,
    pub extra_radius_chunks: u32,
    pub sample_spacing_blocks: u32,
}

impl FarTerrainLodConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            start_margin_chunks: DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS,
            extra_radius_chunks: DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            sample_spacing_blocks: DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS,
        }
    }

    pub const fn enabled() -> Self {
        Self {
            enabled: true,
            start_margin_chunks: DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS,
            extra_radius_chunks: DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            sample_spacing_blocks: DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS,
        }
    }

    pub fn with_extra_radius_chunks(mut self, extra_radius_chunks: u32) -> Self {
        self.extra_radius_chunks = extra_radius_chunks.clamp(
            MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
        );
        self
    }

    fn normalized(self) -> Self {
        Self {
            enabled: self.enabled,
            start_margin_chunks: self.start_margin_chunks,
            extra_radius_chunks: self.extra_radius_chunks.clamp(
                MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            ),
            sample_spacing_blocks: self.sample_spacing_blocks.max(1),
        }
    }
}

impl Default for FarTerrainLodConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Startup visual-coverage prewarm policy (tactical 162 Slice 1).
///
/// This is presentation-only. It builds cheap retained synthetic LOD patches
/// around the spawn center before the first playable frame so the opening view
/// has less blank space. It must never satisfy the spawn-authority gate: real
/// underfoot/near chunks still decide playable honesty. Coverage is bounded by a
/// hard time cap and tile cap so slow LOD degrades to current behavior.
///
/// `sample_spacing_blocks` must match the live far-LOD spacing so prewarmed
/// patches share the same [`FarTerrainLodSourceKey`] and are reused by live
/// rendering instead of being reset and rebuilt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupLodPrewarmConfig {
    pub enabled: bool,
    pub extra_chunks: u32,
    pub time_cap: Duration,
    pub tile_cap: usize,
    pub sample_spacing_blocks: u32,
}

impl StartupLodPrewarmConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            extra_chunks: DEFAULT_STARTUP_LOD_PREWARM_EXTRA_CHUNKS,
            time_cap: DEFAULT_STARTUP_LOD_PREWARM_TIME_CAP,
            tile_cap: DEFAULT_STARTUP_LOD_PREWARM_TILE_CAP,
            sample_spacing_blocks: DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS,
        }
    }

    /// Derive a prewarm policy for a given live far-LOD config. Prewarm is only
    /// enabled when both the caller opts in and far LOD itself is enabled, since
    /// prewarmed patches are useless if the live path never draws them.
    pub fn for_far_lod(far_lod: FarTerrainLodConfig, enabled: bool) -> Self {
        Self {
            enabled: enabled && far_lod.enabled,
            sample_spacing_blocks: far_lod.normalized().sample_spacing_blocks,
            ..Self::disabled()
        }
    }

    pub fn with_extra_chunks(mut self, extra_chunks: u32) -> Self {
        self.extra_chunks = extra_chunks.clamp(
            MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
        );
        self
    }

    pub const fn with_time_cap(mut self, time_cap: Duration) -> Self {
        self.time_cap = time_cap;
        self
    }

    pub const fn with_tile_cap(mut self, tile_cap: usize) -> Self {
        self.tile_cap = tile_cap;
        self
    }

    /// The far-LOD config used to build prewarm coverage: enabled, spawn-centered,
    /// and reaching `render_distance + extra_chunks`, at the live sample spacing.
    pub fn far_lod_config(&self) -> FarTerrainLodConfig {
        FarTerrainLodConfig {
            enabled: true,
            start_margin_chunks: DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS,
            extra_radius_chunks: self.extra_chunks,
            sample_spacing_blocks: self.sample_spacing_blocks,
        }
        .normalized()
    }
}

impl Default for StartupLodPrewarmConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Snapshot of how much desired far-LOD coverage is currently drawable, used to
/// track startup prewarm readiness. `target_tiles` is the desired chunk-patch
/// count and `ready_tiles` is how many of those patches are already retained.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FarTerrainLodCoverage {
    pub ready_tiles: usize,
    pub target_tiles: usize,
}

impl FarTerrainLodCoverage {
    pub const fn is_complete(&self) -> bool {
        self.ready_tiles >= self.target_tiles
    }
}

#[derive(Clone, Debug, Default)]
pub struct FarTerrainLodMaterialPalette {
    block_colors: BTreeMap<String, FarTerrainLodSurfaceColors>,
    texture_colors: BTreeMap<String, FarTerrainLodSurfaceColors>,
}

impl FarTerrainLodMaterialPalette {
    pub fn load_from_asset_source(source: &impl AssetSource) -> Result<Option<Self>> {
        let path = AssetPath::new(FAR_TERRAIN_LOD_MATERIALS_PATH);
        let Some(bytes) = source
            .read(&path)
            .with_context(|| format!("failed to read {FAR_TERRAIN_LOD_MATERIALS_PATH}"))?
        else {
            return Ok(None);
        };
        Self::from_json_bytes(&bytes).map(Some)
    }

    pub fn color_count(&self) -> usize {
        self.block_colors.len() + self.texture_colors.len()
    }

    fn from_json_bytes(bytes: &[u8]) -> Result<Self> {
        let raw: RawLodMaterials = serde_json::from_slice(bytes)
            .with_context(|| format!("failed to parse {FAR_TERRAIN_LOD_MATERIALS_PATH}"))?;
        if raw.schema_version != 1 {
            bail!(
                "unsupported Far LOD material schema version {}; expected 1",
                raw.schema_version
            );
        }
        Ok(Self::from_raw(raw))
    }

    fn from_raw(raw: RawLodMaterials) -> Self {
        let texture_colors = raw
            .textures
            .into_iter()
            .map(|(name, texture)| {
                (
                    name,
                    FarTerrainLodSurfaceColors::same(srgb_color(texture.average_srgb)),
                )
            })
            .collect();

        let mut block_colors = BTreeMap::new();
        for (name, block) in raw.blocks {
            let top = face_color(
                &block.faces,
                &["top", "all", "side", "north", "south", "east", "west"],
            );
            let side = face_color(
                &block.faces,
                &["side", "north", "south", "east", "west", "top", "all"],
            );
            let Some(top) = top else {
                continue;
            };
            block_colors.insert(
                name,
                FarTerrainLodSurfaceColors {
                    top,
                    side: side.unwrap_or(top),
                },
            );
        }

        Self {
            block_colors,
            texture_colors,
        }
    }

    fn colors_for_block(
        &self,
        block: GeneratedBlockId,
        surface_y: i32,
    ) -> Option<FarTerrainLodSurfaceColors> {
        let block_id = base_block_id(block.raw());
        let block_name = GeneratedBlockId(block_id).name();
        if let Some(colors) = self.block_colors.get(block_name) {
            return Some(*colors);
        }
        let texture_name = block_name.strip_prefix("minecraft:").unwrap_or(block_name);
        if let Some(colors) = self.texture_colors.get(texture_name) {
            return Some(*colors);
        }
        if block_id == GRASS_BLOCK && surface_y > SEA_LEVEL as i32 + 1 {
            return self.texture_colors.get("grass_block_top").copied();
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FarTerrainLodSurfaceColors {
    top: [f32; 4],
    side: [f32; 4],
}

impl FarTerrainLodSurfaceColors {
    const fn same(color: [f32; 4]) -> Self {
        Self {
            top: color,
            side: color,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLodMaterials {
    schema_version: u32,
    #[serde(default)]
    textures: BTreeMap<String, RawLodTexture>,
    #[serde(default)]
    blocks: BTreeMap<String, RawLodBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLodTexture {
    average_srgb: [u8; 4],
}

#[derive(Debug, Deserialize)]
struct RawLodBlock {
    #[serde(default)]
    faces: BTreeMap<String, RawLodFace>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLodFace {
    average_srgb: [u8; 4],
}

fn face_color(faces: &BTreeMap<String, RawLodFace>, names: &[&str]) -> Option<[f32; 4]> {
    for name in names {
        if let Some(face) = faces.get(*name) {
            return Some(srgb_color(face.average_srgb));
        }
    }
    None
}

fn srgb_color(color: [u8; 4]) -> [f32; 4] {
    [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        color[3] as f32 / 255.0,
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FarTerrainLodBuildKey {
    seed: i64,
    center: ChunkPos,
    render_distance: u32,
    start_margin_chunks: u32,
    extra_radius_chunks: u32,
    sample_spacing_blocks: u32,
    normal_chunk_hash: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FarTerrainLodSourceKey {
    seed: i64,
    sample_spacing_blocks: u32,
    materials_available: bool,
}

impl FarTerrainLodSourceKey {
    fn new(seed: i64, config: FarTerrainLodConfig, materials_available: bool) -> Self {
        Self {
            seed,
            sample_spacing_blocks: config.normalized().sample_spacing_blocks,
            materials_available,
        }
    }

    fn patch_build_key(self, pos: ChunkPos) -> FarTerrainLodBuildKey {
        FarTerrainLodBuildKey {
            seed: self.seed,
            center: pos,
            render_distance: 0,
            start_margin_chunks: 0,
            extra_radius_chunks: 1,
            sample_spacing_blocks: self.sample_spacing_blocks,
            normal_chunk_hash: 0,
        }
    }
}

impl FarTerrainLodBuildKey {
    fn new(
        seed: i64,
        center: ChunkPos,
        render_distance: u32,
        config: FarTerrainLodConfig,
        normal_terrain_chunks: Option<&BTreeSet<ChunkPos>>,
    ) -> Self {
        let config = config.normalized();
        Self {
            seed,
            center,
            render_distance,
            start_margin_chunks: config.start_margin_chunks,
            extra_radius_chunks: config.extra_radius_chunks,
            sample_spacing_blocks: config.sample_spacing_blocks,
            normal_chunk_hash: normal_terrain_chunk_hash(normal_terrain_chunks),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum FarTerrainNormalCoverage<'a> {
    Radius,
    ReadyChunks(&'a BTreeSet<ChunkPos>),
    NoNormalChunks,
}

impl<'a> FarTerrainNormalCoverage<'a> {
    fn new(normal_terrain_chunks: Option<&'a BTreeSet<ChunkPos>>) -> Self {
        match normal_terrain_chunks {
            Some(chunks) if !chunks.is_empty() => Self::ReadyChunks(chunks),
            _ => Self::Radius,
        }
    }

    fn covers_chunk(self, key: FarTerrainLodBuildKey, pos: ChunkPos) -> bool {
        match self {
            Self::Radius => {
                let (inner_chunk_radius, _) = far_lod_chunk_radii(key);
                chunk_distance_from_center_chunk(key.center, pos) <= inner_chunk_radius
            }
            Self::ReadyChunks(chunks) => chunks.contains(&pos),
            Self::NoNormalChunks => false,
        }
    }
}

fn normal_terrain_chunk_hash(normal_terrain_chunks: Option<&BTreeSet<ChunkPos>>) -> u64 {
    let Some(chunks) = normal_terrain_chunks.filter(|chunks| !chunks.is_empty()) else {
        return 0;
    };
    let mut hash = mix_hash(chunks.len() as u64);
    for pos in chunks {
        hash = mix_hash(hash ^ pos.x as u32 as u64);
        hash = mix_hash(hash ^ ((pos.z as u32 as u64) << 1));
    }
    hash
}

#[derive(Debug)]
struct FarTerrainLodPatch {
    vertices: Vec<f32>,
    indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FarTerrainLodTileMetadata {
    vertex_count: usize,
    index_count: usize,
    owned_bytes: usize,
}

#[derive(Debug)]
pub struct FarTerrainLodCache {
    source_key: Option<FarTerrainLodSourceKey>,
    view_key: Option<FarTerrainLodBuildKey>,
    desired_chunks: BTreeSet<ChunkPos>,
    pending_builds: VecDeque<FarTerrainLodBuildRequest>,
    inflight_builds: BTreeMap<LodTileKey, MonotonicInstant>,
    resident_tiles: ResidentTileCache<LodTileKey, FarTerrainLodTileMetadata>,
    uploaded_tiles: BTreeSet<LodTileKey>,
    uploads: ResidentTileUploadCoordinator<LodTileKey, FarTerrainLodTileMesh>,
    materials: Option<Arc<FarTerrainLodMaterialPalette>>,
    frame: FarTerrainLodFrameUpdate,
    handed_off_lifecycle_items: usize,
    submitted_builds: u64,
    completed_builds: u64,
    stale_builds: u64,
    last_upload_bytes: usize,
    total_upload_bytes: u64,
    now: MonotonicInstant,
}

impl Default for FarTerrainLodCache {
    fn default() -> Self {
        Self {
            source_key: None,
            view_key: None,
            desired_chunks: BTreeSet::new(),
            pending_builds: VecDeque::new(),
            inflight_builds: BTreeMap::new(),
            resident_tiles: ResidentTileCache::default(),
            uploaded_tiles: BTreeSet::new(),
            uploads: ResidentTileUploadCoordinator::default(),
            materials: None,
            frame: FarTerrainLodFrameUpdate::default(),
            handed_off_lifecycle_items: 0,
            submitted_builds: 0,
            completed_builds: 0,
            stale_builds: 0,
            last_upload_bytes: 0,
            total_upload_bytes: 0,
            now: MonotonicInstant::ZERO,
        }
    }
}

impl FarTerrainLodCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) -> usize {
        let abandoned_jobs = self.inflight_builds.len();
        self.source_key = None;
        self.view_key = None;
        self.desired_chunks.clear();
        self.pending_builds.clear();
        self.inflight_builds.clear();
        self.resident_tiles = ResidentTileCache::default();
        self.uploaded_tiles.clear();
        self.uploads.clear();
        self.materials = None;
        self.frame = FarTerrainLodFrameUpdate::default();
        self.handed_off_lifecycle_items = 0;
        self.last_upload_bytes = 0;
        abandoned_jobs
    }

    pub fn advance_for_camera<C: FarTerrainLodCompiler>(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        render_distance: u32,
        normal_terrain_chunks: Option<&BTreeSet<ChunkPos>>,
        materials: Option<&FarTerrainLodMaterialPalette>,
        compiler: &mut C,
        build_budget: usize,
    ) -> Result<()> {
        self.advance_for_camera_at(
            far_lod_now(),
            config,
            seed,
            center,
            render_distance,
            normal_terrain_chunks,
            materials,
            compiler,
            build_budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn advance_for_camera_at<C: FarTerrainLodCompiler>(
        &mut self,
        now: MonotonicInstant,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        render_distance: u32,
        normal_terrain_chunks: Option<&BTreeSet<ChunkPos>>,
        materials: Option<&FarTerrainLodMaterialPalette>,
        compiler: &mut C,
        build_budget: usize,
    ) -> Result<()> {
        self.now = self.now.max(now);
        if !config.enabled {
            let abandoned = self.clear();
            compiler.release_completed_far_lod_jobs(abandoned);
            return Ok(());
        }
        let source_key = FarTerrainLodSourceKey::new(seed, config, materials.is_some());
        if self.source_key != Some(source_key) {
            let abandoned = self.reset_for_source(source_key, materials);
            compiler.release_completed_far_lod_jobs(abandoned);
        }
        let key = FarTerrainLodBuildKey::new(
            seed,
            center,
            render_distance,
            config,
            normal_terrain_chunks,
        );
        self.update_target(key, normal_terrain_chunks, self.now);
        self.accept_completed(compiler)?;
        self.submit_builds(compiler, build_budget)?;
        Ok(())
    }

    /// Advance retained coverage toward `render_distance + config.extra_radius`
    /// with an explicit build budget and report how much of the desired coverage
    /// is currently drawable. Used by startup prewarm; shares the same retained
    /// tiles and source key as [`Self::advance_for_camera`] so prewarmed tiles are
    /// reused (not reset) once live rendering takes over.
    pub fn prewarm<C: FarTerrainLodCompiler>(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        center: ChunkPos,
        render_distance: u32,
        normal_terrain_chunks: Option<&BTreeSet<ChunkPos>>,
        materials: Option<&FarTerrainLodMaterialPalette>,
        chunk_budget: usize,
        compiler: &mut C,
    ) -> FarTerrainLodCoverage {
        let _ = self.advance_for_camera(
            config,
            seed,
            center,
            render_distance,
            normal_terrain_chunks,
            materials,
            compiler,
            chunk_budget,
        );
        self.coverage()
    }

    pub fn drain_render_uploads<C: FarTerrainLodCompiler>(
        &mut self,
        upload_budget: usize,
        compiler: &mut C,
    ) {
        if self.handed_off_lifecycle_items > 0 {
            let released = self
                .uploads
                .complete_applied_lifecycle_items(self.handed_off_lifecycle_items);
            compiler.release_completed_far_lod_jobs(released);
            self.handed_off_lifecycle_items = 0;
        }
        self.frame.uploads.clear();
        self.frame.removals.clear();

        let drain = self.uploads.drain_budgeted(Some(upload_budget), None);
        for mesh in &drain.uploads {
            self.uploaded_tiles.insert(mesh.key());
        }
        for tile in &drain.removed_tile_keys {
            self.uploaded_tiles.remove(tile);
        }
        let upload_bytes = drain
            .uploads
            .iter()
            .map(ResidentTileUploadPayload::estimated_owned_bytes)
            .sum::<usize>();
        self.last_upload_bytes = upload_bytes;
        self.total_upload_bytes = self.total_upload_bytes.saturating_add(upload_bytes as u64);
        self.frame.uploads = drain.uploads;
        self.frame.removals = drain.removed_tile_keys;
        self.handed_off_lifecycle_items = drain.lifecycle_item_count;
    }

    pub fn prepare_render_update(
        &mut self,
        visible_chunks: &BTreeSet<ChunkPos>,
    ) -> &FarTerrainLodFrameUpdate {
        let visible_tiles = visible_chunks
            .iter()
            .copied()
            .map(LodTileKey::synthetic)
            .filter(|key| self.uploaded_tiles.contains(key))
            .collect::<BTreeSet<_>>();
        let changed = !self.frame.uploads.is_empty()
            || !self.frame.removals.is_empty()
            || self.frame.visible_tiles != visible_tiles;
        if changed {
            self.frame.revision = self.frame.revision.wrapping_add(1);
            self.frame.visible_tiles = visible_tiles;
        }
        &self.frame
    }

    /// Chunk-aligned tiles that currently have a drawable retained synthetic
    /// patch (desired tiles whose patch geometry is built). This is the synthetic
    /// availability the [`crate::lod_coverage::LodCoverageCoordinator`] resolves
    /// against; it excludes tiles suppressed by drawable normal chunks, since
    /// those are never desired.
    pub fn drawable_lod_tiles(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.desired_chunks
            .iter()
            .copied()
            .filter(|pos| self.uploaded_tiles.contains(&LodTileKey::synthetic(*pos)))
    }

    /// How many of the currently desired chunk patches are already retained.
    pub fn coverage(&self) -> FarTerrainLodCoverage {
        let ready_tiles = self
            .desired_chunks
            .iter()
            .filter(|pos| {
                self.resident_tiles
                    .contains_tile(LodTileKey::synthetic(**pos))
            })
            .count();
        FarTerrainLodCoverage {
            ready_tiles,
            target_tiles: self.desired_chunks.len(),
        }
    }

    pub fn stats(&self) -> FarTerrainLodProducerStats {
        let oldest = self
            .pending_builds
            .iter()
            .map(|request| request.queued_at)
            .chain(self.inflight_builds.values().copied())
            .min();
        let upload_stats = self.uploads.stats();
        FarTerrainLodProducerStats {
            desired_tiles: self.desired_chunks.len(),
            resident_tiles: self.resident_tiles.len(),
            visible_tiles: self.frame.visible_tiles.len(),
            pending_builds: self.pending_builds.len(),
            inflight_builds: self.inflight_builds.len(),
            oldest_build_age_ms: oldest
                .map(|queued| self.now.saturating_duration_since(queued).as_secs_f64() * 1_000.0),
            submitted_builds: self.submitted_builds,
            completed_builds: self.completed_builds,
            stale_builds: self.stale_builds,
            queued_uploads: upload_stats.queued_uploads,
            last_upload_bytes: self.last_upload_bytes,
            total_upload_bytes: self.total_upload_bytes,
        }
    }

    fn reset_for_source(
        &mut self,
        source_key: FarTerrainLodSourceKey,
        materials: Option<&FarTerrainLodMaterialPalette>,
    ) -> usize {
        let abandoned = self.inflight_builds.len();
        self.source_key = Some(source_key);
        self.view_key = None;
        self.desired_chunks.clear();
        self.pending_builds.clear();
        self.inflight_builds.clear();
        self.resident_tiles = ResidentTileCache::default();
        self.uploaded_tiles.clear();
        self.uploads.clear();
        self.materials = materials.cloned().map(Arc::new);
        self.frame = FarTerrainLodFrameUpdate::default();
        self.handed_off_lifecycle_items = 0;
        abandoned
    }

    fn update_target(
        &mut self,
        key: FarTerrainLodBuildKey,
        normal_terrain_chunks: Option<&BTreeSet<ChunkPos>>,
        now: MonotonicInstant,
    ) {
        if self.view_key == Some(key) {
            return;
        }
        let desired_queue = capped_far_lod_chunk_positions(key, normal_terrain_chunks);
        let desired_chunks = desired_queue.iter().copied().collect::<BTreeSet<_>>();
        self.view_key = Some(key);
        self.desired_chunks = desired_chunks;
        self.pending_builds = desired_queue
            .into_iter()
            .filter(|pos| {
                let tile = LodTileKey::synthetic(*pos);
                !self.resident_tiles.contains_tile(tile)
                    && !self.inflight_builds.contains_key(&tile)
            })
            .map(|pos| {
                FarTerrainLodBuildRequest::new(
                    pos,
                    self.source_key.expect("far LOD source initialized"),
                    self.materials.clone(),
                    now,
                )
            })
            .collect();
        self.evict_resident_tiles(key);
    }

    fn submit_builds<C: FarTerrainLodCompiler>(
        &mut self,
        compiler: &mut C,
        build_budget: usize,
    ) -> Result<()> {
        let admitted = build_budget.min(compiler.available_far_lod_job_slots());
        for _ in 0..admitted {
            let Some(request) = self.pending_builds.pop_front() else {
                break;
            };
            if !self.desired_chunks.contains(&request.key.chunk)
                || self.resident_tiles.contains_tile(request.key)
                || self.inflight_builds.contains_key(&request.key)
            {
                continue;
            }
            let key = request.key;
            let queued_at = request.queued_at;
            compiler.submit_far_lod(request)?;
            self.inflight_builds.insert(key, queued_at);
            self.submitted_builds = self.submitted_builds.saturating_add(1);
        }
        Ok(())
    }

    fn accept_completed<C: FarTerrainLodCompiler>(&mut self, compiler: &mut C) -> Result<()> {
        let completed = compiler.try_recv_completed_far_lod()?;
        for result in completed {
            self.inflight_builds.remove(&result.key);
            let retain = self.source_key == Some(result.source_key)
                && self.view_key.is_some_and(|key| {
                    chunk_distance_from_center_chunk(key.center, result.key.chunk)
                        <= far_lod_retain_radius(key)
                });
            if !retain {
                self.stale_builds = self.stale_builds.saturating_add(1);
                compiler.release_completed_far_lod_jobs(1);
                continue;
            }
            let metadata = FarTerrainLodTileMetadata {
                vertex_count: result.mesh.vertex_count(),
                index_count: result.mesh.index_count(),
                owned_bytes: result.mesh.estimated_owned_bytes(),
            };
            self.resident_tiles.insert(result.key, metadata);
            self.uploads.enqueue(vec![result.mesh], BTreeSet::new(), 1);
            self.completed_builds = self.completed_builds.saturating_add(1);
        }
        Ok(())
    }

    fn evict_resident_tiles(&mut self, key: FarTerrainLodBuildKey) {
        let retain_radius = far_lod_retain_radius(key);
        let mut removals = self
            .resident_tiles
            .tile_keys()
            .filter(|tile| chunk_distance_from_center_chunk(key.center, tile.chunk) > retain_radius)
            .collect::<BTreeSet<_>>();
        let limit = max_retained_lod_patches(key);
        if self.resident_tiles.len().saturating_sub(removals.len()) > limit {
            let mut eviction_order = self
                .resident_tiles
                .tile_keys()
                .filter(|tile| !removals.contains(tile))
                .map(|tile| {
                    let desired = self.desired_chunks.contains(&tile.chunk);
                    let distance = chunk_distance_from_center_chunk(key.center, tile.chunk);
                    (tile, desired, distance)
                })
                .collect::<Vec<_>>();
            eviction_order.sort_by_key(|(tile, desired, distance)| {
                (
                    *desired,
                    std::cmp::Reverse(*distance),
                    std::cmp::Reverse(tile.chunk.z),
                    std::cmp::Reverse(tile.chunk.x),
                )
            });
            for (tile, _, _) in eviction_order
                .into_iter()
                .take(self.resident_tiles.len().saturating_sub(removals.len()) - limit)
            {
                removals.insert(tile);
            }
        }
        for tile in &removals {
            self.resident_tiles.remove(*tile);
        }
        if !removals.is_empty() {
            self.uploads.enqueue(Vec::new(), removals, 0);
        }
    }
}

fn far_lod_chunk_radii(key: FarTerrainLodBuildKey) -> (u32, u32) {
    let inner_chunk_radius = key.render_distance.saturating_add(key.start_margin_chunks);
    let outer_chunk_radius = key
        .render_distance
        .saturating_add(key.extra_radius_chunks)
        .max(inner_chunk_radius.saturating_add(1));
    (inner_chunk_radius, outer_chunk_radius)
}

fn far_lod_chunk_positions(
    key: FarTerrainLodBuildKey,
    normal_terrain_chunks: Option<&BTreeSet<ChunkPos>>,
) -> VecDeque<ChunkPos> {
    let (_, outer_chunk_radius) = far_lod_chunk_radii(key);
    let normal_coverage = FarTerrainNormalCoverage::new(normal_terrain_chunks);
    let outer = outer_chunk_radius as i32;
    let mut positions = Vec::new();
    for dz in -outer..=outer {
        for dx in -outer..=outer {
            let pos = ChunkPos::new(key.center.x + dx, key.center.z + dz);
            let distance = dx.unsigned_abs().max(dz.unsigned_abs());
            if distance <= outer_chunk_radius && !normal_coverage.covers_chunk(key, pos) {
                positions.push(pos);
            }
        }
    }
    positions.sort_by_key(|pos| {
        let distance = (pos.x - key.center.x)
            .unsigned_abs()
            .max((pos.z - key.center.z).unsigned_abs());
        (distance, pos.z, pos.x)
    });
    positions.into()
}

fn capped_far_lod_chunk_positions(
    key: FarTerrainLodBuildKey,
    normal_terrain_chunks: Option<&BTreeSet<ChunkPos>>,
) -> VecDeque<ChunkPos> {
    let mut positions = far_lod_chunk_positions(key, normal_terrain_chunks);
    positions.truncate(max_retained_lod_patches(key));
    positions
}

fn far_lod_retain_radius(key: FarTerrainLodBuildKey) -> u32 {
    let (_, outer_chunk_radius) = far_lod_chunk_radii(key);
    outer_chunk_radius.saturating_add(DEFAULT_FAR_TERRAIN_LOD_EVICTION_MARGIN_CHUNKS)
}

fn max_retained_lod_patches(key: FarTerrainLodBuildKey) -> usize {
    square_chunk_count(far_lod_retain_radius(key)).min(MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES)
}

fn square_chunk_count(radius: u32) -> usize {
    let diameter = radius as usize * 2 + 1;
    diameter.saturating_mul(diameter)
}

fn build_retained_far_terrain_lod_patch(
    surface_chunks: &mut BTreeMap<ChunkPos, GeneratedChunk>,
    pos: ChunkPos,
    source_key: FarTerrainLodSourceKey,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> FarTerrainLodPatch {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    append_far_terrain_lod_chunk_patch(
        &mut vertices,
        &mut indices,
        surface_chunks,
        source_key.seed,
        pos,
        source_key.patch_build_key(pos),
        FarTerrainNormalCoverage::NoNormalChunks,
        materials,
    );
    FarTerrainLodPatch { vertices, indices }
}

pub(crate) fn compile_far_terrain_lod_request(
    request: FarTerrainLodBuildRequest,
) -> FarTerrainLodBuildResult {
    let mesh =
        compile_far_terrain_lod_worker_input(request.worker_input(), request.materials.as_deref());
    request
        .complete_worker_mesh(mesh)
        .expect("local far LOD compiler preserves the request tile key")
}

pub fn compile_far_terrain_lod_worker_input(
    input: FarTerrainLodWorkerInput,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> FarTerrainLodTileMesh {
    let mut surface_chunks = BTreeMap::new();
    let source_key = FarTerrainLodSourceKey {
        seed: input.seed,
        sample_spacing_blocks: input.sample_spacing_blocks.max(1),
        materials_available: materials.is_some(),
    };
    let patch = build_retained_far_terrain_lod_patch(
        &mut surface_chunks,
        input.key.chunk,
        source_key,
        materials,
    );
    FarTerrainLodTileMesh::new(input.key, patch.vertices, patch.indices)
}

#[cfg(test)]
fn build_far_terrain_lod_tile(key: FarTerrainLodBuildKey, pos: ChunkPos) -> FarTerrainLodTileMesh {
    compile_far_terrain_lod_request(FarTerrainLodBuildRequest::new(
        pos,
        FarTerrainLodSourceKey::new(
            key.seed,
            FarTerrainLodConfig {
                enabled: true,
                start_margin_chunks: 0,
                extra_radius_chunks: 1,
                sample_spacing_blocks: key.sample_spacing_blocks,
            },
            false,
        ),
        None,
        far_lod_now(),
    ))
    .mesh
}

fn append_far_terrain_lod_chunk_patch(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    surface_chunks: &mut BTreeMap<ChunkPos, GeneratedChunk>,
    seed: i64,
    pos: ChunkPos,
    key: FarTerrainLodBuildKey,
    normal_coverage: FarTerrainNormalCoverage<'_>,
    materials: Option<&FarTerrainLodMaterialPalette>,
) {
    let spacing = key.sample_spacing_blocks.max(1);
    let offsets = chunk_sample_offsets(spacing);
    debug_assert!(offsets.len() >= 2);

    let mut cells = Vec::with_capacity((offsets.len() - 1) * (offsets.len() - 1));
    let min_x = chunk_min_block_coord(pos.x);
    let min_z = chunk_min_block_coord(pos.z);
    for z_cell in 0..offsets.len() - 1 {
        for x_cell in 0..offsets.len() - 1 {
            let x0 = min_x + offsets[x_cell];
            let x1 = min_x + offsets[x_cell + 1];
            let z0 = min_z + offsets[z_cell];
            let z1 = min_z + offsets[z_cell + 1];
            cells.push(sample_lod_cell(
                surface_chunks,
                seed,
                x0,
                x1,
                z0,
                z1,
                materials,
            ));
        }
    }

    for cell in cells.iter().flatten() {
        append_top_face(vertices, indices, cell);
    }

    let cells_per_axis = offsets.len() - 1;
    for z_cell in 0..cells_per_axis {
        for x_cell in 0..cells_per_axis {
            let Some(cell) = cells[z_cell * cells_per_axis + x_cell] else {
                continue;
            };
            for direction in [
                LodCellDirection::West,
                LodCellDirection::East,
                LodCellDirection::North,
                LodCellDirection::South,
            ] {
                let neighbor = neighbor_lod_cell(
                    &cells,
                    cells_per_axis,
                    x_cell,
                    z_cell,
                    direction,
                    surface_chunks,
                    seed,
                    key,
                    cell,
                    normal_coverage,
                    materials,
                );
                let Some(neighbor) = neighbor else {
                    continue;
                };
                if neighbor.region == LodNeighborRegion::Outer {
                    continue;
                }
                if cell.sample.y > neighbor.cell.sample.y {
                    append_drop_face(
                        vertices,
                        indices,
                        cell,
                        neighbor.cell.sample.y,
                        direction,
                        0.74,
                    );
                } else if neighbor.region == LodNeighborRegion::Inner
                    && neighbor.cell.sample.y > cell.sample.y
                {
                    append_neighbor_inner_drop_face(
                        vertices,
                        indices,
                        cell,
                        neighbor.cell,
                        direction,
                        0.68,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FarTerrainLodCell {
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    sample: FarTerrainSurfaceSample,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LodCellDirection {
    West,
    East,
    North,
    South,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LodNeighborRegion {
    Inner,
    Lod,
    Outer,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LodNeighborCell {
    cell: FarTerrainLodCell,
    region: LodNeighborRegion,
}

fn sample_lod_cell(
    chunks: &mut BTreeMap<ChunkPos, GeneratedChunk>,
    seed: i64,
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> Option<FarTerrainLodCell> {
    let sample_x = x0 + (x1 - x0) / 2;
    let sample_z = z0 + (z1 - z0) / 2;
    surface_sample_world(chunks, seed, sample_x, sample_z, materials).map(|sample| {
        FarTerrainLodCell {
            x0,
            x1,
            z0,
            z1,
            sample,
        }
    })
}

fn neighbor_lod_cell(
    cells: &[Option<FarTerrainLodCell>],
    cells_per_axis: usize,
    x_cell: usize,
    z_cell: usize,
    direction: LodCellDirection,
    surface_chunks: &mut BTreeMap<ChunkPos, GeneratedChunk>,
    seed: i64,
    key: FarTerrainLodBuildKey,
    cell: FarTerrainLodCell,
    normal_coverage: FarTerrainNormalCoverage<'_>,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> Option<LodNeighborCell> {
    match direction {
        LodCellDirection::West if x_cell > 0 => {
            let cell = cells[z_cell * cells_per_axis + x_cell - 1]?;
            Some(LodNeighborCell {
                cell,
                region: LodNeighborRegion::Lod,
            })
        }
        LodCellDirection::East if x_cell + 1 < cells_per_axis => {
            let cell = cells[z_cell * cells_per_axis + x_cell + 1]?;
            Some(LodNeighborCell {
                cell,
                region: LodNeighborRegion::Lod,
            })
        }
        LodCellDirection::North if z_cell > 0 => {
            let cell = cells[(z_cell - 1) * cells_per_axis + x_cell]?;
            Some(LodNeighborCell {
                cell,
                region: LodNeighborRegion::Lod,
            })
        }
        LodCellDirection::South if z_cell + 1 < cells_per_axis => {
            let cell = cells[(z_cell + 1) * cells_per_axis + x_cell]?;
            Some(LodNeighborCell {
                cell,
                region: LodNeighborRegion::Lod,
            })
        }
        _ => {
            let (x0, x1, z0, z1) = neighbor_cell_bounds(cell, direction);
            let neighbor = sample_lod_cell(surface_chunks, seed, x0, x1, z0, z1, materials)?;
            Some(LodNeighborCell {
                cell: neighbor,
                region: neighbor_region(key, neighbor, normal_coverage),
            })
        }
    }
}

fn neighbor_cell_bounds(
    cell: FarTerrainLodCell,
    direction: LodCellDirection,
) -> (i32, i32, i32, i32) {
    let width = cell.x1 - cell.x0;
    let depth = cell.z1 - cell.z0;
    match direction {
        LodCellDirection::West => (cell.x0 - width, cell.x0, cell.z0, cell.z1),
        LodCellDirection::East => (cell.x1, cell.x1 + width, cell.z0, cell.z1),
        LodCellDirection::North => (cell.x0, cell.x1, cell.z0 - depth, cell.z0),
        LodCellDirection::South => (cell.x0, cell.x1, cell.z1, cell.z1 + depth),
    }
}

fn neighbor_region(
    key: FarTerrainLodBuildKey,
    cell: FarTerrainLodCell,
    normal_coverage: FarTerrainNormalCoverage<'_>,
) -> LodNeighborRegion {
    let (_, outer_chunk_radius) = far_lod_chunk_radii(key);
    let center_x = cell.x0 + (cell.x1 - cell.x0) / 2;
    let center_z = cell.z0 + (cell.z1 - cell.z0) / 2;
    let distance = chunk_distance_from_center(key.center, center_x, center_z);
    if distance > outer_chunk_radius {
        LodNeighborRegion::Outer
    } else if normal_coverage.covers_chunk(key, ChunkPos::from_block_coords(center_x, center_z)) {
        LodNeighborRegion::Inner
    } else {
        LodNeighborRegion::Lod
    }
}

fn append_top_face(vertices: &mut Vec<f32>, indices: &mut Vec<u32>, cell: &FarTerrainLodCell) {
    append_quad(
        vertices,
        indices,
        [
            [cell.x0 as f32, cell.sample.y, cell.z0 as f32],
            [cell.x1 as f32, cell.sample.y, cell.z0 as f32],
            [cell.x0 as f32, cell.sample.y, cell.z1 as f32],
            [cell.x1 as f32, cell.sample.y, cell.z1 as f32],
        ],
        cell.sample.top_color,
    );
}

fn append_drop_face(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    cell: FarTerrainLodCell,
    low_y: f32,
    direction: LodCellDirection,
    shade: f32,
) {
    append_vertical_face(
        vertices,
        indices,
        cell,
        cell.sample.y,
        low_y,
        direction,
        shade_color(cell.sample.side_color, shade),
    );
}

fn append_neighbor_inner_drop_face(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    cell: FarTerrainLodCell,
    neighbor: FarTerrainLodCell,
    direction: LodCellDirection,
    shade: f32,
) {
    append_vertical_face(
        vertices,
        indices,
        cell,
        neighbor.sample.y,
        cell.sample.y,
        direction,
        shade_color(neighbor.sample.side_color, shade),
    );
}

fn append_vertical_face(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    cell: FarTerrainLodCell,
    high_y: f32,
    low_y: f32,
    direction: LodCellDirection,
    color: [f32; 4],
) {
    if high_y <= low_y {
        return;
    }
    let x0 = cell.x0 as f32;
    let x1 = cell.x1 as f32;
    let z0 = cell.z0 as f32;
    let z1 = cell.z1 as f32;
    let positions = match direction {
        LodCellDirection::West => [
            [x0, high_y, z0],
            [x0, low_y, z0],
            [x0, high_y, z1],
            [x0, low_y, z1],
        ],
        LodCellDirection::East => [
            [x1, high_y, z1],
            [x1, low_y, z1],
            [x1, high_y, z0],
            [x1, low_y, z0],
        ],
        LodCellDirection::North => [
            [x1, high_y, z0],
            [x1, low_y, z0],
            [x0, high_y, z0],
            [x0, low_y, z0],
        ],
        LodCellDirection::South => [
            [x0, high_y, z1],
            [x0, low_y, z1],
            [x1, high_y, z1],
            [x1, low_y, z1],
        ],
    };
    append_quad(vertices, indices, positions, color);
}

fn append_quad(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    positions: [[f32; 3]; 4],
    color: [f32; 4],
) {
    let base = u32::try_from(vertices.len() / 7).expect("far terrain LOD vertex count fits u32");
    for position in positions {
        vertices.extend_from_slice(&position);
        vertices.extend_from_slice(&color);
    }
    indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
}

fn shade_color(mut color: [f32; 4], shade: f32) -> [f32; 4] {
    color[0] *= shade;
    color[1] *= shade;
    color[2] *= shade;
    color
}

fn chunk_sample_offsets(spacing: u32) -> Vec<i32> {
    let spacing = spacing.max(1).min(CHUNK_WIDTH as u32);
    let mut offsets = Vec::new();
    let mut offset = 0;
    while offset < CHUNK_WIDTH as u32 {
        offsets.push(offset as i32);
        offset = offset.saturating_add(spacing);
    }
    if offsets.last().copied() != Some(CHUNK_WIDTH) {
        offsets.push(CHUNK_WIDTH);
    }
    offsets
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FarTerrainSurfaceSample {
    y: f32,
    top_color: [f32; 4],
    side_color: [f32; 4],
}

fn surface_sample(
    chunk: &GeneratedChunk,
    local_x: i32,
    local_z: i32,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> Option<FarTerrainSurfaceSample> {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        let block = chunk.block_at_y(local_x, y, local_z);
        if is_air_like(block.raw()) {
            continue;
        }
        let surface_y = y + 1;
        let colors = surface_colors(block, surface_y, materials);
        return Some(FarTerrainSurfaceSample {
            y: surface_y as f32,
            top_color: colors.top,
            side_color: colors.side,
        });
    }
    None
}

fn surface_sample_world(
    chunks: &mut BTreeMap<ChunkPos, GeneratedChunk>,
    seed: i64,
    world_x: i32,
    world_z: i32,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> Option<FarTerrainSurfaceSample> {
    let pos = ChunkPos::from_block_coords(world_x, world_z);
    let chunk = chunks
        .entry(pos)
        .or_insert_with(|| generate_overworld_surface_chunk(seed, pos.x, pos.z));
    let local_x = world_x - chunk_min_block_coord(pos.x);
    let local_z = world_z - chunk_min_block_coord(pos.z);
    surface_sample(chunk, local_x, local_z, materials)
}

fn chunk_distance_from_center(center: ChunkPos, world_x: i32, world_z: i32) -> u32 {
    let chunk = ChunkPos::from_block_coords(world_x, world_z);
    chunk_distance_from_center_chunk(center, chunk)
}

fn chunk_distance_from_center_chunk(center: ChunkPos, chunk: ChunkPos) -> u32 {
    (chunk.x - center.x)
        .unsigned_abs()
        .max((chunk.z - center.z).unsigned_abs())
}

fn surface_colors(
    block: GeneratedBlockId,
    surface_y: i32,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> FarTerrainLodSurfaceColors {
    if let Some(colors) = materials.and_then(|palette| palette.colors_for_block(block, surface_y)) {
        return colors;
    }
    FarTerrainLodSurfaceColors::same(surface_color(block, surface_y))
}

fn surface_color(block: GeneratedBlockId, surface_y: i32) -> [f32; 4] {
    let block_id = base_block_id(block.raw());
    if is_water(block_id) {
        return [0.14, 0.28, 0.58, 1.0];
    }
    if is_lava(block_id) {
        return [0.88, 0.30, 0.08, 1.0];
    }
    if has_fluid(block_id) {
        return [0.16, 0.30, 0.62, 1.0];
    }

    match block_id {
        GRASS_BLOCK => [0.30, 0.50, 0.22, 1.0],
        DIRT => [0.36, 0.25, 0.15, 1.0],
        COARSE_DIRT | PODZOL => [0.28, 0.20, 0.13, 1.0],
        MYCELIUM => [0.42, 0.36, 0.42, 1.0],
        SAND | SANDSTONE => [0.72, 0.66, 0.42, 1.0],
        RED_SAND | RED_SANDSTONE => [0.64, 0.32, 0.16, 1.0],
        GRAVEL => [0.42, 0.42, 0.40, 1.0],
        STONE | BEDROCK | ANDESITE | DIORITE | GRANITE => {
            if surface_y > 104 {
                [0.58, 0.58, 0.54, 1.0]
            } else {
                [0.44, 0.44, 0.40, 1.0]
            }
        }
        SNOW | SNOW_BLOCK => [0.88, 0.90, 0.86, 1.0],
        ICE | PACKED_ICE => [0.58, 0.74, 0.86, 1.0],
        CLAY => [0.48, 0.50, 0.54, 1.0],
        TERRACOTTA => [0.55, 0.30, 0.20, 1.0],
        WHITE_TERRACOTTA => [0.72, 0.62, 0.52, 1.0],
        ORANGE_TERRACOTTA => [0.64, 0.32, 0.16, 1.0],
        MAGENTA_TERRACOTTA => [0.58, 0.32, 0.44, 1.0],
        LIGHT_BLUE_TERRACOTTA => [0.44, 0.46, 0.58, 1.0],
        YELLOW_TERRACOTTA => [0.70, 0.54, 0.24, 1.0],
        LIME_TERRACOTTA => [0.48, 0.54, 0.28, 1.0],
        PINK_TERRACOTTA => [0.62, 0.38, 0.36, 1.0],
        GRAY_TERRACOTTA => [0.36, 0.30, 0.28, 1.0],
        LIGHT_GRAY_TERRACOTTA => [0.52, 0.46, 0.42, 1.0],
        CYAN_TERRACOTTA => [0.34, 0.38, 0.38, 1.0],
        PURPLE_TERRACOTTA => [0.46, 0.30, 0.42, 1.0],
        BLUE_TERRACOTTA => [0.32, 0.30, 0.44, 1.0],
        BROWN_TERRACOTTA => [0.38, 0.24, 0.16, 1.0],
        GREEN_TERRACOTTA => [0.34, 0.36, 0.20, 1.0],
        RED_TERRACOTTA => [0.56, 0.24, 0.18, 1.0],
        BLACK_TERRACOTTA => [0.18, 0.15, 0.14, 1.0],
        _ => {
            if surface_y <= SEA_LEVEL as i32 + 1 {
                [0.32, 0.44, 0.24, 1.0]
            } else if surface_y > 102 {
                [0.58, 0.58, 0.54, 1.0]
            } else {
                [0.30, 0.50, 0.22, 1.0]
            }
        }
    }
}

fn mix_hash(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct ImmediateFarLodCompiler {
        completed: Vec<FarTerrainLodBuildResult>,
        submitted: Vec<LodTileKey>,
        released: usize,
    }

    impl FarTerrainLodCompiler for ImmediateFarLodCompiler {
        fn available_far_lod_job_slots(&self) -> usize {
            MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES
        }

        fn submit_far_lod(&mut self, request: FarTerrainLodBuildRequest) -> Result<()> {
            self.submitted.push(request.key);
            self.completed
                .push(compile_far_terrain_lod_request(request));
            Ok(())
        }

        fn try_recv_completed_far_lod(&mut self) -> Result<Vec<FarTerrainLodBuildResult>> {
            Ok(std::mem::take(&mut self.completed))
        }

        fn release_completed_far_lod_jobs(&mut self, count: usize) -> usize {
            self.released += count;
            count
        }
    }

    fn advance_until_complete(
        cache: &mut FarTerrainLodCache,
        compiler: &mut ImmediateFarLodCompiler,
        config: FarTerrainLodConfig,
        center: ChunkPos,
        build_budget: usize,
    ) {
        for _ in 0..64 {
            cache
                .advance_for_camera(config, 12345, center, 0, None, None, compiler, build_budget)
                .unwrap();
            if cache.coverage().is_complete() {
                return;
            }
        }
        panic!("far LOD coverage did not complete within the test bound");
    }

    fn upload_all_desired(
        cache: &mut FarTerrainLodCache,
        compiler: &mut ImmediateFarLodCompiler,
    ) -> FarTerrainLodFrameUpdate {
        let visible = cache.desired_chunks.clone();
        cache.drain_render_uploads(MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES, compiler);
        cache.prepare_render_update(&visible).clone()
    }

    #[test]
    fn far_terrain_lod_config_defaults_disabled() {
        assert!(!FarTerrainLodConfig::default().enabled);
        assert!(FarTerrainLodConfig::enabled().enabled);
        assert_eq!(
            FarTerrainLodConfig::enabled().start_margin_chunks,
            DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS
        );
        assert_eq!(DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS, 0);
        assert_eq!(DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS, 4);
    }

    #[test]
    fn far_terrain_lod_cache_returns_none_when_disabled() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();

        cache
            .advance_for_camera(
                FarTerrainLodConfig::default(),
                12345,
                ChunkPos::new(0, 0),
                5,
                None,
                None,
                &mut compiler,
                4,
            )
            .unwrap();

        assert_eq!(cache.coverage(), FarTerrainLodCoverage::default());
        assert_eq!(cache.stats(), FarTerrainLodProducerStats::default());
        assert!(compiler.submitted.is_empty());
    }

    #[test]
    fn far_terrain_lod_mesh_has_surface_ring_geometry() {
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let key = FarTerrainLodBuildKey::new(12345, ChunkPos::new(0, 0), 0, config, None);

        let mesh = build_far_terrain_lod_tile(key, ChunkPos::new(1, 0));

        assert!(mesh.vertex_count() > 0);
        assert!(mesh.index_count() > 0);
        assert_eq!(mesh.index_count() % 3, 0);
        assert_eq!(mesh.key(), LodTileKey::synthetic(ChunkPos::new(1, 0)));
        assert!(mesh.vertex_count() <= 512);
        assert!(mesh.index_count() <= 768);
    }

    #[test]
    fn far_terrain_lod_chunk_patch_emits_flat_blocky_cell_caps() {
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let key = FarTerrainLodBuildKey::new(12345, ChunkPos::new(0, 0), 0, config, None);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut chunks = BTreeMap::new();

        append_far_terrain_lod_chunk_patch(
            &mut vertices,
            &mut indices,
            &mut chunks,
            12345,
            ChunkPos::new(1, 0),
            key,
            FarTerrainNormalCoverage::Radius,
            None,
        );

        assert!(vertices.len() >= 28);
        let y0 = vertices[1];
        let y1 = vertices[8];
        let y2 = vertices[15];
        let y3 = vertices[22];
        assert_eq!(y0, y1);
        assert_eq!(y0, y2);
        assert_eq!(y0, y3);

        let xs = [vertices[0], vertices[7], vertices[14], vertices[21]];
        let zs = [vertices[2], vertices[9], vertices[16], vertices[23]];
        let min_x = xs.iter().fold(f32::INFINITY, |min, x| min.min(*x));
        let max_x = xs.iter().fold(f32::NEG_INFINITY, |max, x| max.max(*x));
        let min_z = zs.iter().fold(f32::INFINITY, |min, z| min.min(*z));
        let max_z = zs.iter().fold(f32::NEG_INFINITY, |max, z| max.max(*z));
        assert_eq!(
            max_x - min_x,
            DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS as f32
        );
        assert_eq!(
            max_z - min_z,
            DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS as f32
        );
        assert_eq!(&indices[0..6], &[0, 2, 1, 1, 2, 3]);
    }

    #[test]
    fn far_terrain_lod_drop_face_is_vertical() {
        let cell = FarTerrainLodCell {
            x0: 16,
            x1: 20,
            z0: 32,
            z1: 36,
            sample: FarTerrainSurfaceSample {
                y: 72.0,
                top_color: [0.30, 0.50, 0.22, 1.0],
                side_color: [0.36, 0.25, 0.15, 1.0],
            },
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        append_drop_face(
            &mut vertices,
            &mut indices,
            cell,
            64.0,
            LodCellDirection::West,
            0.74,
        );

        assert_eq!(vertices.len(), 28);
        assert_eq!(indices.len(), 6);
        for vertex in 0..4 {
            assert_eq!(vertices[vertex * 7], cell.x0 as f32);
        }
        assert_eq!(vertices[1], 72.0);
        assert_eq!(vertices[8], 64.0);
        assert_eq!(vertices[15], 72.0);
        assert_eq!(vertices[22], 64.0);
    }

    #[test]
    fn far_terrain_lod_chunk_distance_matches_render_distance_boundary() {
        let center = ChunkPos::new(0, 0);
        let last_normal_x = chunk_min_block_coord(5) + CHUNK_WIDTH - 1;
        let first_lod_x = chunk_min_block_coord(6);

        assert_eq!(chunk_distance_from_center(center, last_normal_x, 0), 5);
        assert_eq!(chunk_distance_from_center(center, first_lod_x, 0), 6);
    }

    #[test]
    fn far_terrain_lod_fills_chunks_missing_from_ready_terrain_set() {
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let center = ChunkPos::new(0, 0);
        let missing_inside_render_distance = ChunkPos::new(1, 0);
        let covered_inside_render_distance = ChunkPos::new(0, 0);
        let first_outer_lod_chunk = ChunkPos::new(3, 0);
        let mut ready_chunks = BTreeSet::new();
        for z in -2..=2 {
            for x in -2..=2 {
                let pos = ChunkPos::new(x, z);
                if pos != missing_inside_render_distance {
                    ready_chunks.insert(pos);
                }
            }
        }
        let key = FarTerrainLodBuildKey::new(12345, center, 2, config, Some(&ready_chunks));

        let positions = far_lod_chunk_positions(key, Some(&ready_chunks));

        assert!(positions.contains(&missing_inside_render_distance));
        assert!(!positions.contains(&covered_inside_render_distance));
        assert!(positions.contains(&first_outer_lod_chunk));
    }

    #[test]
    fn far_terrain_lod_range_is_clamped() {
        assert_eq!(
            FarTerrainLodConfig::enabled()
                .with_extra_radius_chunks(0)
                .normalized()
                .extra_radius_chunks,
            MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
        );
        assert_eq!(
            FarTerrainLodConfig::enabled()
                .with_extra_radius_chunks(MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS + 1)
                .normalized()
                .extra_radius_chunks,
            MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
        );
    }

    #[test]
    fn far_terrain_lod_material_palette_uses_block_faces_and_texture_fallback() {
        let palette = FarTerrainLodMaterialPalette::from_json_bytes(
            br#"{
                "schemaVersion": 1,
                "textures": {
                    "dirt": {
                        "averageSrgb": [64, 48, 32, 255]
                    }
                },
                "blocks": {
                    "minecraft:grass_block": {
                        "faces": {
                            "top": {
                                "averageSrgb": [32, 128, 16, 255]
                            },
                            "side": {
                                "averageSrgb": [96, 64, 32, 255]
                            }
                        }
                    }
                }
            }"#,
        )
        .expect("valid Far LOD material metadata parses");

        let grass = palette
            .colors_for_block(GeneratedBlockId(GRASS_BLOCK), 64)
            .expect("grass block material exists");
        assert_eq!(grass.top, srgb_color([32, 128, 16, 255]));
        assert_eq!(grass.side, srgb_color([96, 64, 32, 255]));

        let dirt = palette
            .colors_for_block(GeneratedBlockId(DIRT), 64)
            .expect("dirt texture fallback exists");
        assert_eq!(dirt.top, srgb_color([64, 48, 32, 255]));
        assert_eq!(dirt.side, srgb_color([64, 48, 32, 255]));
    }

    #[test]
    fn far_terrain_lod_mesh_uses_real_surface_chunk_height() {
        let chunk = generate_overworld_surface_chunk(12345, 1, 0);
        let sample = surface_sample(&chunk, 0, 0, None).expect("surface chunk has terrain");
        let mut expected = None;
        for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
            let block = chunk.block_at_y(0, y, 0);
            if !is_air_like(block.raw()) {
                expected = Some(y + 1);
                break;
            }
        }

        assert_eq!(
            sample.y,
            expected.expect("manual scan found terrain") as f32
        );
    }

    #[test]
    fn far_terrain_lod_world_boundary_sample_uses_owning_chunk() {
        let mut chunks = BTreeMap::new();
        let world_x = chunk_min_block_coord(1);

        let _sample = surface_sample_world(&mut chunks, 12345, world_x, 0, None)
            .expect("surface sample exists");

        assert!(chunks.contains_key(&ChunkPos::new(1, 0)));
        assert!(!chunks.contains_key(&ChunkPos::new(0, 0)));
    }

    #[test]
    fn far_terrain_lod_cache_advances_incrementally_then_stabilizes() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        cache
            .advance_for_camera(
                config,
                12345,
                ChunkPos::new(0, 0),
                0,
                None,
                None,
                &mut compiler,
                3,
            )
            .unwrap();
        assert_eq!(cache.stats().inflight_builds, 3);
        assert_eq!(cache.stats().pending_builds, 5);

        advance_until_complete(&mut cache, &mut compiler, config, ChunkPos::new(0, 0), 3);
        let frame = upload_all_desired(&mut cache, &mut compiler);
        assert_eq!(frame.uploads.len(), 8);
        assert_eq!(frame.visible_tiles.len(), 8);
        let stable_revision = frame.revision;
        let visible = cache.desired_chunks.clone();
        cache.drain_render_uploads(3, &mut compiler);
        assert_eq!(
            cache.prepare_render_update(&visible).revision,
            stable_revision
        );
    }

    #[test]
    fn far_terrain_lod_cache_retains_overlapping_patches_after_chunk_move() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let first_center = ChunkPos::new(0, 0);
        let moved_center = ChunkPos::new(1, 0);

        advance_until_complete(&mut cache, &mut compiler, config, first_center, 32);
        let first_ready = cache
            .resident_tiles
            .tile_keys()
            .map(|tile| tile.chunk)
            .collect::<BTreeSet<_>>();
        assert_eq!(first_ready.len(), 8);
        cache
            .advance_for_camera(config, 12345, moved_center, 0, None, None, &mut compiler, 0)
            .unwrap();

        let moved_key = FarTerrainLodBuildKey::new(12345, moved_center, 0, config, None);
        let moved_desired = far_lod_chunk_positions(moved_key, None)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let reused = first_ready
            .intersection(&moved_desired)
            .copied()
            .collect::<Vec<_>>();
        assert!(!reused.is_empty());
        for pos in reused {
            assert!(
                cache
                    .resident_tiles
                    .contains_tile(LodTileKey::synthetic(pos))
            );
        }
        assert!(!cache.desired_chunks.is_empty());
    }

    #[test]
    fn far_terrain_lod_cache_caps_desired_patch_count() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled()
            .with_extra_radius_chunks(MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS);

        cache
            .advance_for_camera(
                config,
                12345,
                ChunkPos::new(0, 0),
                0,
                None,
                None,
                &mut compiler,
                0,
            )
            .unwrap();

        assert_eq!(
            cache.desired_chunks.len(),
            MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES
        );
        assert!(
            cache.resident_tiles.len() + cache.pending_builds.len() + cache.inflight_builds.len()
                <= MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES
        );
    }

    #[test]
    fn startup_lod_prewarm_config_gates_on_far_lod_enabled() {
        let enabled = StartupLodPrewarmConfig::for_far_lod(FarTerrainLodConfig::enabled(), true);
        assert!(enabled.enabled);
        assert_eq!(
            enabled.sample_spacing_blocks,
            DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS
        );
        assert_eq!(
            enabled.extra_chunks,
            DEFAULT_STARTUP_LOD_PREWARM_EXTRA_CHUNKS
        );

        assert!(
            !StartupLodPrewarmConfig::for_far_lod(FarTerrainLodConfig::enabled(), false).enabled
        );
        assert!(
            !StartupLodPrewarmConfig::for_far_lod(FarTerrainLodConfig::disabled(), true).enabled
        );
        assert!(!StartupLodPrewarmConfig::default().enabled);
    }

    #[test]
    fn startup_lod_prewarm_far_lod_config_targets_extra_chunks() {
        let config = StartupLodPrewarmConfig::for_far_lod(FarTerrainLodConfig::enabled(), true)
            .with_extra_chunks(5);
        let far_lod = config.far_lod_config();

        assert!(far_lod.enabled);
        assert_eq!(far_lod.extra_radius_chunks, 5);
        assert_eq!(
            far_lod.sample_spacing_blocks,
            DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS
        );

        // Extra chunks below the minimum ring are clamped up.
        assert_eq!(
            StartupLodPrewarmConfig::for_far_lod(FarTerrainLodConfig::enabled(), true)
                .with_extra_chunks(0)
                .extra_chunks,
            MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS
        );
    }

    #[test]
    fn far_terrain_lod_cache_prewarm_fills_coverage_incrementally() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = StartupLodPrewarmConfig::for_far_lod(FarTerrainLodConfig::enabled(), true)
            .with_extra_chunks(1)
            .far_lod_config();

        let first = cache.prewarm(
            config,
            12345,
            ChunkPos::new(0, 0),
            0,
            None,
            None,
            3,
            &mut compiler,
        );
        assert_eq!(first.target_tiles, 8);
        assert_eq!(first.ready_tiles, 0);
        assert!(!first.is_complete());

        let mut coverage = first;
        for _ in 0..8 {
            if coverage.is_complete() {
                break;
            }
            coverage = cache.prewarm(
                config,
                12345,
                ChunkPos::new(0, 0),
                0,
                None,
                None,
                3,
                &mut compiler,
            );
        }
        assert!(coverage.is_complete());
        assert_eq!(coverage.ready_tiles, 8);
        assert_eq!(cache.coverage(), coverage);
    }

    #[test]
    fn far_terrain_lod_cache_prewarm_patches_are_reused_by_live_mesh() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let live = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let prewarm = StartupLodPrewarmConfig::for_far_lod(live, true)
            .with_extra_chunks(1)
            .far_lod_config();

        // Prewarm to full coverage before any live frame.
        let mut coverage = FarTerrainLodCoverage::default();
        for _ in 0..8 {
            coverage = cache.prewarm(
                prewarm,
                12345,
                ChunkPos::new(0, 0),
                0,
                None,
                None,
                24,
                &mut compiler,
            );
            if coverage.is_complete() {
                break;
            }
        }
        assert!(coverage.is_complete());
        let prewarmed = cache
            .resident_tiles
            .tile_keys()
            .map(|tile| tile.chunk)
            .collect::<BTreeSet<_>>();
        assert_eq!(prewarmed.len(), 8);

        // The first live frame must reuse the prewarmed patches, not reset them:
        // prewarm and live share the same source key (seed + spacing + materials).
        cache
            .advance_for_camera(
                live,
                12345,
                ChunkPos::new(0, 0),
                0,
                None,
                None,
                &mut compiler,
                0,
            )
            .unwrap();
        for pos in &prewarmed {
            assert!(
                cache
                    .resident_tiles
                    .contains_tile(LodTileKey::synthetic(*pos)),
                "live rendering should reuse prewarmed patch {pos:?}"
            );
        }
    }

    #[test]
    fn far_terrain_lod_cache_evicts_patches_outside_retention_radius() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let first_center = ChunkPos::new(0, 0);
        let far_center = ChunkPos::new(10, 0);

        advance_until_complete(&mut cache, &mut compiler, config, first_center, 32);
        let first_ready = cache
            .resident_tiles
            .tile_keys()
            .map(|tile| tile.chunk)
            .collect::<BTreeSet<_>>();
        assert!(!first_ready.is_empty());

        cache
            .advance_for_camera(config, 12345, far_center, 0, None, None, &mut compiler, 0)
            .unwrap();

        for pos in first_ready {
            assert!(
                !cache
                    .resident_tiles
                    .contains_tile(LodTileKey::synthetic(pos)),
                "old patch {pos:?} should be evicted after a far move"
            );
        }
    }
}
