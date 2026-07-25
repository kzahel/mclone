#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use mclone_assets::{AssetPath, AssetSource};
use mclone_core::{CHUNK_WIDTH, ChunkPos, LodTileKey, chunk_min_block_coord};
use mclone_mesh::TexturedTerrainAssets;
use mclone_render::far_lod::{FarTerrainLodFrameUpdate, FarTerrainLodTileMesh};
use mclone_render_session::{
    ResidentTileCache, ResidentTileUploadCoordinator, ResidentTileUploadPayload,
};
use mclone_server::WorldGenerationProfile;
use mclone_worldgen::block::{
    ACACIA_LEAVES, ACACIA_LOG, ANDESITE, BEDROCK, BLACK_TERRACOTTA, BLUE_TERRACOTTA,
    BROWN_TERRACOTTA, CLAY, COARSE_DIRT, CYAN_TERRACOTTA, DIORITE, DIRT, GRANITE, GRASS_BLOCK,
    GRAVEL, GRAY_TERRACOTTA, GREEN_TERRACOTTA, GeneratedBlockId, ICE, LIGHT_BLUE_TERRACOTTA,
    LIGHT_GRAY_TERRACOTTA, LIME_TERRACOTTA, MAGENTA_TERRACOTTA, MYCELIUM, OAK_LEAVES, OAK_LOG,
    ORANGE_TERRACOTTA, PACKED_ICE, PINK_TERRACOTTA, PODZOL, PURPLE_TERRACOTTA, RED_SAND,
    RED_SANDSTONE, RED_TERRACOTTA, SAND, SANDSTONE, SNOW, SNOW_BLOCK, SPRUCE_LEAVES, SPRUCE_LOG,
    STONE, TERRACOTTA, WHITE_TERRACOTTA, YELLOW_TERRACOTTA, base_block_id, has_fluid, is_air_like,
    is_lava, is_water,
};
use mclone_worldgen::levelgen::{
    AlphaGenerationStage, BetaGenerationStage, GeneratedChunk, McloneForestIntentSample,
    McloneOverworldSamplingTopology, McloneOverworldStreamPlanCache,
    McloneOverworldVegetationPlanCache, McloneTreeFamily, McloneTreeOccurrence,
    McloneVegetationBounds, McloneVegetationSource, generate_alpha_stage_chunk,
    generate_beta_stage_chunk, generate_flat_grass_chunk, generate_mclone_overworld_surface_chunk,
    generate_mclone_overworld_surface_chunk_with_stream_cache, generate_overworld_surface_chunk,
    generate_small_island_surface_chunk,
};
use serde::Deserialize;

use crate::monotonic::{MonotonicClockHandle, MonotonicInstant};

pub const DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS: u32 = 0;
pub const MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 1;
pub const DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 12;
pub const MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS: u32 = 64;
pub const DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS: u32 = 4;
pub const FAR_TERRAIN_LOD_LEVEL_COUNT: usize = 3;
pub const FAR_TERRAIN_LOD_LEVEL_HYSTERESIS_CHUNKS: u32 = 2;
pub const MAX_FAR_TERRAIN_LOD_DOUBLE_RESIDENT_TILES: usize = 256;
pub const DEFAULT_FAR_TERRAIN_LOD_CHUNK_BUILD_BUDGET: usize = 4;
pub const DEFAULT_FAR_TERRAIN_LOD_EVICTION_MARGIN_CHUNKS: u32 = 2;
/// One drawable synthetic ring overlaps the real-terrain boundary so a real
/// chunk is suppressed only after it is actually drawable.
pub const FAR_TERRAIN_LOD_HANDOFF_OVERLAP_CHUNKS: u32 = 1;
/// One hidden inner ring covers the real/synthetic handoff while the camera
/// crosses a chunk boundary.
pub const FAR_TERRAIN_LOD_INNER_GUARD_CHUNKS: u32 = 1;
/// Two hidden outer rings give distance-priority builds two camera steps to
/// finish before entering the drawable shell during flight.
pub const FAR_TERRAIN_LOD_OUTER_GUARD_CHUNKS: u32 = 2;
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

#[derive(Clone, Debug)]
pub struct FarTerrainLodBuildRequest {
    pub key: LodTileKey,
    source_key: FarTerrainLodSourceKey,
    neighbor_sample_spacings: [u32; 4],
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
    pub generation_profile: WorldGenerationProfile,
    pub sample_spacing_blocks: u32,
    pub neighbor_sample_spacings: [u32; 4],
}

impl FarTerrainLodBuildRequest {
    #[cfg(test)]
    fn new(
        pos: ChunkPos,
        source_key: FarTerrainLodSourceKey,
        materials: Option<Arc<FarTerrainLodMaterialPalette>>,
        queued_at: MonotonicInstant,
    ) -> Self {
        Self::new_for_tile(
            LodTileKey::synthetic(pos),
            source_key,
            [source_key.sample_spacing_blocks; 4],
            materials,
            queued_at,
        )
    }

    fn new_for_tile(
        key: LodTileKey,
        source_key: FarTerrainLodSourceKey,
        neighbor_sample_spacings: [u32; 4],
        materials: Option<Arc<FarTerrainLodMaterialPalette>>,
        queued_at: MonotonicInstant,
    ) -> Self {
        Self {
            key,
            source_key,
            neighbor_sample_spacings,
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
            MonotonicInstant::ZERO,
        )
    }

    pub fn worker_input(&self) -> FarTerrainLodWorkerInput {
        FarTerrainLodWorkerInput {
            key: self.key,
            seed: self.source_key.seed,
            generation_profile: self.source_key.generation_profile,
            sample_spacing_blocks: far_lod_sample_spacing_for_level(
                self.source_key.sample_spacing_blocks,
                self.key.level,
            ),
            neighbor_sample_spacings: self.neighbor_sample_spacings,
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
            neighbor_sample_spacings: self.neighbor_sample_spacings,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FarTerrainLodBuildResult {
    pub key: LodTileKey,
    pub queued_at: MonotonicInstant,
    pub mesh: FarTerrainLodTileMesh,
    source_key: FarTerrainLodSourceKey,
    neighbor_sample_spacings: [u32; 4],
}

pub trait FarTerrainLodCompiler {
    fn ensure_far_lod_capacity(&mut self) -> Result<()> {
        Ok(())
    }
    fn available_far_lod_job_slots(&self) -> usize;
    fn submit_far_lod(&mut self, request: FarTerrainLodBuildRequest) -> Result<()>;
    fn try_recv_completed_far_lod(&mut self) -> Result<Vec<FarTerrainLodBuildResult>>;
    fn release_completed_far_lod_jobs(&mut self, count: usize) -> usize;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FarTerrainLodProducerStats {
    pub desired_tiles: usize,
    pub prefetch_tiles: usize,
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
    pub resident_tiles_by_level: [usize; FAR_TERRAIN_LOD_LEVEL_COUNT],
    pub visible_tiles_by_level: [usize; FAR_TERRAIN_LOD_LEVEL_COUNT],
    pub double_resident_tiles: usize,
    pub max_double_resident_tiles: usize,
    pub level_flips: u64,
    pub max_level_flips_per_tile: u64,
}

/// Pull-only exact producer state used by the far-LOD settle harness.
///
/// Normal frame reporting deliberately keeps count-only stats. Constructing
/// this snapshot clones the producer's bounded sets only when a diagnostic
/// caller asks for them, so enabling far LOD does not add steady-state work by
/// itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FarTerrainLodSettleSnapshot {
    pub desired_tiles: BTreeMap<ChunkPos, u8>,
    pub prefetch_tiles: BTreeMap<ChunkPos, u8>,
    pub resident_tiles: BTreeSet<LodTileKey>,
    pub uploaded_tiles: BTreeSet<LodTileKey>,
    /// Per-chunk replacement winner retained by the producer before coverage
    /// precedence suppresses tiles behind normal terrain.
    pub published_tiles_by_chunk: BTreeMap<ChunkPos, LodTileKey>,
    /// Tiles admitted to the current renderer frame after coverage precedence.
    pub visible_tiles: BTreeSet<LodTileKey>,
    pub pending_builds: BTreeSet<LodTileKey>,
    pub inflight_builds: BTreeSet<LodTileKey>,
    pub queued_uploads: BTreeSet<LodTileKey>,
    pub queued_removals: BTreeSet<LodTileKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FarLodDetailMode {
    Auto,
    Fixed4,
    Fixed8,
    Fixed16,
}

impl FarLodDetailMode {
    pub const fn sample_spacing_blocks(self, auto_spacing_blocks: u32) -> u32 {
        match self {
            Self::Auto => auto_spacing_blocks,
            Self::Fixed4 => 4,
            Self::Fixed8 => 8,
            Self::Fixed16 => 16,
        }
    }

    pub const fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl Default for FarLodDetailMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FarTerrainLodConfig {
    pub enabled: bool,
    pub detail_mode: FarLodDetailMode,
    pub start_margin_chunks: u32,
    pub extra_radius_chunks: u32,
    pub sample_spacing_blocks: u32,
}

impl FarTerrainLodConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            detail_mode: FarLodDetailMode::Auto,
            start_margin_chunks: DEFAULT_FAR_TERRAIN_LOD_START_MARGIN_CHUNKS,
            extra_radius_chunks: DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            sample_spacing_blocks: DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS,
        }
    }

    pub const fn enabled() -> Self {
        Self {
            enabled: true,
            detail_mode: FarLodDetailMode::Auto,
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

    pub const fn with_detail_mode(mut self, detail_mode: FarLodDetailMode) -> Self {
        self.detail_mode = detail_mode;
        self
    }

    fn normalized(self) -> Self {
        Self {
            enabled: self.enabled,
            detail_mode: self.detail_mode,
            start_margin_chunks: self.start_margin_chunks,
            extra_radius_chunks: self.extra_radius_chunks.clamp(
                MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
            ),
            sample_spacing_blocks: self.sample_spacing_blocks.clamp(1, CHUNK_WIDTH as u32),
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
    pub detail_mode: FarLodDetailMode,
}

impl StartupLodPrewarmConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            extra_chunks: DEFAULT_STARTUP_LOD_PREWARM_EXTRA_CHUNKS,
            time_cap: DEFAULT_STARTUP_LOD_PREWARM_TIME_CAP,
            tile_cap: DEFAULT_STARTUP_LOD_PREWARM_TILE_CAP,
            sample_spacing_blocks: DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS,
            detail_mode: FarLodDetailMode::Auto,
        }
    }

    /// Derive a prewarm policy for a given live far-LOD config. Prewarm is only
    /// enabled when both the caller opts in and far LOD itself is enabled, since
    /// prewarmed patches are useless if the live path never draws them.
    pub fn for_far_lod(far_lod: FarTerrainLodConfig, enabled: bool) -> Self {
        Self {
            enabled: enabled && far_lod.enabled,
            sample_spacing_blocks: far_lod.normalized().sample_spacing_blocks,
            detail_mode: far_lod.normalized().detail_mode,
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
            detail_mode: self.detail_mode,
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
    derived_state_colors: BTreeMap<u8, FarTerrainLodSurfaceColors>,
    block_colors: BTreeMap<String, FarTerrainLodSurfaceColors>,
    texture_colors: BTreeMap<String, FarTerrainLodSurfaceColors>,
}

impl FarTerrainLodMaterialPalette {
    /// Derive distant-terrain colors from the terrain source and presentation
    /// that actually won resolution for this asset epoch.
    pub fn from_textured_terrain_assets(assets: &TexturedTerrainAssets) -> Self {
        let derived_state_colors = (u8::MIN..=u8::MAX)
            .filter_map(|raw| {
                assets
                    .material_summary(GeneratedBlockId(raw).block_state_id())
                    .map(|summary| {
                        (
                            raw,
                            FarTerrainLodSurfaceColors {
                                top: summary.top,
                                side: summary.side,
                            },
                        )
                    })
            })
            .collect();
        Self {
            derived_state_colors,
            block_colors: BTreeMap::new(),
            texture_colors: BTreeMap::new(),
        }
    }

    /// Legacy reader retained for old standalone asset sources and fixtures.
    ///
    /// Profile-driven scene preparation derives this palette from the resolved
    /// terrain atlas instead.
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
        self.derived_state_colors.len() + self.block_colors.len() + self.texture_colors.len()
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
            derived_state_colors: BTreeMap::new(),
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
        if let Some(colors) = self.derived_state_colors.get(&block_id) {
            return Some(*colors);
        }
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
    generation_profile: WorldGenerationProfile,
    center: ChunkPos,
    render_distance: u32,
    start_margin_chunks: u32,
    extra_radius_chunks: u32,
    sample_spacing_blocks: u32,
    detail_mode: FarLodDetailMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FarTerrainLodSourceKey {
    seed: i64,
    generation_profile: WorldGenerationProfile,
    sample_spacing_blocks: u32,
    detail_mode: FarLodDetailMode,
    materials_available: bool,
}

impl FarTerrainLodSourceKey {
    #[cfg(test)]
    fn new(seed: i64, config: FarTerrainLodConfig, materials_available: bool) -> Self {
        Self::new_with_profile(
            seed,
            WorldGenerationProfile::Overworld,
            config,
            materials_available,
        )
    }

    fn new_with_profile(
        seed: i64,
        generation_profile: WorldGenerationProfile,
        config: FarTerrainLodConfig,
        materials_available: bool,
    ) -> Self {
        let config = config.normalized();
        Self {
            seed,
            generation_profile,
            sample_spacing_blocks: config
                .detail_mode
                .sample_spacing_blocks(config.sample_spacing_blocks),
            detail_mode: config.detail_mode,
            materials_available,
        }
    }

    fn patch_build_key(self, pos: ChunkPos) -> FarTerrainLodBuildKey {
        FarTerrainLodBuildKey {
            seed: self.seed,
            generation_profile: self.generation_profile,
            center: pos,
            render_distance: 0,
            start_margin_chunks: 0,
            extra_radius_chunks: 1,
            sample_spacing_blocks: self.sample_spacing_blocks,
            detail_mode: self.detail_mode,
        }
    }
}

impl FarTerrainLodBuildKey {
    #[cfg(test)]
    fn new(seed: i64, center: ChunkPos, render_distance: u32, config: FarTerrainLodConfig) -> Self {
        Self::new_with_profile(
            seed,
            WorldGenerationProfile::Overworld,
            center,
            render_distance,
            config,
        )
    }

    fn new_with_profile(
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        render_distance: u32,
        config: FarTerrainLodConfig,
    ) -> Self {
        let config = config.normalized();
        Self {
            seed,
            generation_profile,
            center,
            render_distance,
            start_margin_chunks: config.start_margin_chunks,
            extra_radius_chunks: config.extra_radius_chunks,
            sample_spacing_blocks: config.sample_spacing_blocks,
            detail_mode: config.detail_mode,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum FarTerrainNormalCoverage {
    #[cfg(test)]
    Radius,
    NoNormalChunks,
}

impl FarTerrainNormalCoverage {
    fn covers_chunk(self, _key: FarTerrainLodBuildKey, _pos: ChunkPos) -> bool {
        match self {
            #[cfg(test)]
            Self::Radius => {
                let (inner_chunk_radius, _) = far_lod_chunk_radii(_key);
                chunk_distance_from_center_chunk(_key.center, _pos) <= inner_chunk_radius
            }
            Self::NoNormalChunks => false,
        }
    }
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
    clock: MonotonicClockHandle,
    source_key: Option<FarTerrainLodSourceKey>,
    view_key: Option<FarTerrainLodBuildKey>,
    movement_direction: (i32, i32),
    desired_tiles: BTreeMap<ChunkPos, LodTileKey>,
    /// Retained guard tiles built just beyond the visible LOD radius and under
    /// the real-terrain handoff boundary. They are uploaded but never offered
    /// to coverage arbitration until they enter `desired_tiles`.
    prefetch_tiles: BTreeMap<ChunkPos, LodTileKey>,
    level_history: BTreeMap<ChunkPos, u8>,
    visible_tiles_by_chunk: BTreeMap<ChunkPos, LodTileKey>,
    dirty_tiles: BTreeSet<LodTileKey>,
    level_flip_counts: BTreeMap<ChunkPos, u64>,
    resident_tiles_by_level: [usize; FAR_TERRAIN_LOD_LEVEL_COUNT],
    visible_tiles_by_level: [usize; FAR_TERRAIN_LOD_LEVEL_COUNT],
    double_resident_tiles: usize,
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
    level_flips: u64,
    max_level_flips_per_tile: u64,
    max_double_resident_tiles: usize,
    now: MonotonicInstant,
}

impl Default for FarTerrainLodCache {
    fn default() -> Self {
        Self::with_clock(MonotonicClockHandle::default())
    }
}

impl FarTerrainLodCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_clock(clock: MonotonicClockHandle) -> Self {
        Self {
            clock,
            source_key: None,
            view_key: None,
            movement_direction: (0, 0),
            desired_tiles: BTreeMap::new(),
            prefetch_tiles: BTreeMap::new(),
            level_history: BTreeMap::new(),
            visible_tiles_by_chunk: BTreeMap::new(),
            dirty_tiles: BTreeSet::new(),
            level_flip_counts: BTreeMap::new(),
            resident_tiles_by_level: [0; FAR_TERRAIN_LOD_LEVEL_COUNT],
            visible_tiles_by_level: [0; FAR_TERRAIN_LOD_LEVEL_COUNT],
            double_resident_tiles: 0,
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
            level_flips: 0,
            max_level_flips_per_tile: 0,
            max_double_resident_tiles: 0,
            now: MonotonicInstant::ZERO,
        }
    }

    pub fn clear(&mut self) -> usize {
        let abandoned_jobs = self.inflight_builds.len();
        self.source_key = None;
        self.view_key = None;
        self.movement_direction = (0, 0);
        self.desired_tiles.clear();
        self.prefetch_tiles.clear();
        self.level_history.clear();
        self.visible_tiles_by_chunk.clear();
        self.dirty_tiles.clear();
        self.level_flip_counts.clear();
        self.resident_tiles_by_level = [0; FAR_TERRAIN_LOD_LEVEL_COUNT];
        self.visible_tiles_by_level = [0; FAR_TERRAIN_LOD_LEVEL_COUNT];
        self.double_resident_tiles = 0;
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
        materials: Option<&FarTerrainLodMaterialPalette>,
        compiler: &mut C,
        build_budget: usize,
    ) -> Result<()> {
        self.advance_for_camera_with_profile(
            config,
            seed,
            WorldGenerationProfile::Overworld,
            center,
            render_distance,
            materials,
            compiler,
            build_budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn advance_for_camera_with_profile<C: FarTerrainLodCompiler>(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        render_distance: u32,
        materials: Option<&FarTerrainLodMaterialPalette>,
        compiler: &mut C,
        build_budget: usize,
    ) -> Result<()> {
        self.advance_for_camera_at_with_profile(
            self.clock.now(),
            config,
            seed,
            generation_profile,
            center,
            render_distance,
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
        materials: Option<&FarTerrainLodMaterialPalette>,
        compiler: &mut C,
        build_budget: usize,
    ) -> Result<()> {
        self.advance_for_camera_at_with_profile(
            now,
            config,
            seed,
            WorldGenerationProfile::Overworld,
            center,
            render_distance,
            materials,
            compiler,
            build_budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn advance_for_camera_at_with_profile<C: FarTerrainLodCompiler>(
        &mut self,
        now: MonotonicInstant,
        config: FarTerrainLodConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        render_distance: u32,
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
        compiler.ensure_far_lod_capacity()?;
        let source_key = FarTerrainLodSourceKey::new_with_profile(
            seed,
            generation_profile,
            config,
            materials.is_some(),
        );
        if self.source_key != Some(source_key) {
            let abandoned = self.reset_for_source(source_key, materials);
            compiler.release_completed_far_lod_jobs(abandoned);
        }
        let key = FarTerrainLodBuildKey::new_with_profile(
            seed,
            generation_profile,
            center,
            render_distance,
            config,
        );
        self.update_target(key, self.now);
        self.accept_completed(compiler)?;
        self.ensure_pending_builds(self.now);
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
        materials: Option<&FarTerrainLodMaterialPalette>,
        chunk_budget: usize,
        compiler: &mut C,
    ) -> FarTerrainLodCoverage {
        self.prewarm_with_profile(
            config,
            seed,
            WorldGenerationProfile::Overworld,
            center,
            render_distance,
            materials,
            chunk_budget,
            compiler,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prewarm_with_profile<C: FarTerrainLodCompiler>(
        &mut self,
        config: FarTerrainLodConfig,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        center: ChunkPos,
        render_distance: u32,
        materials: Option<&FarTerrainLodMaterialPalette>,
        chunk_budget: usize,
        compiler: &mut C,
    ) -> FarTerrainLodCoverage {
        let _ = self.advance_for_camera_with_profile(
            config,
            seed,
            generation_profile,
            center,
            render_distance,
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
        let mut replacement_removals = BTreeSet::new();
        for mesh in &drain.uploads {
            let key = mesh.key();
            self.uploaded_tiles.insert(key);
            if self.desired_tiles.get(&key.chunk) == Some(&key) {
                if let Some(old) = self.visible_tiles_by_chunk.insert(key.chunk, key) {
                    if old != key {
                        replacement_removals.insert(old);
                    }
                }
            } else if self.prefetch_tiles.get(&key.chunk) == Some(&key) {
                // Guard coverage is deliberately resident/uploaded but hidden.
                // It becomes publishable without another build/upload when a
                // later camera center moves the tile into the visible radius.
                replacement_removals.extend(
                    self.resident_tiles
                        .tile_keys()
                        .filter(|resident| resident.chunk == key.chunk && *resident != key),
                );
            } else if self.visible_tiles_by_chunk.get(&key.chunk) != Some(&key) {
                replacement_removals.insert(key);
            }
        }
        for tile in &drain.removed_tile_keys {
            self.uploaded_tiles.remove(tile);
            self.remove_resident_tile(*tile);
            if self.visible_tiles_by_chunk.get(&tile.chunk) == Some(tile) {
                self.visible_tiles_by_chunk.remove(&tile.chunk);
            }
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
        if !replacement_removals.is_empty() {
            self.uploads.enqueue(Vec::new(), replacement_removals, 0);
        }
    }

    pub fn prepare_render_update(
        &mut self,
        visible_chunks: &BTreeSet<ChunkPos>,
    ) -> &FarTerrainLodFrameUpdate {
        let visible_tiles = visible_chunks
            .iter()
            .filter_map(|pos| self.visible_tiles_by_chunk.get(pos).copied())
            .filter(|key| self.uploaded_tiles.contains(key))
            .collect::<BTreeSet<_>>();
        let mut visible_tiles_by_level = [0; FAR_TERRAIN_LOD_LEVEL_COUNT];
        for tile in &visible_tiles {
            if let Some(count) =
                visible_tiles_by_level.get_mut(tile.level.saturating_sub(1) as usize)
            {
                *count += 1;
            }
        }
        self.visible_tiles_by_level = visible_tiles_by_level;
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
    /// against. It intentionally does not apply presentation precedence: the
    /// handoff ring may have both a built synthetic tile and a drawable normal
    /// chunk, but the coordinator must suppress the synthetic tile before render.
    pub fn drawable_lod_tiles(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.desired_tiles.keys().copied().filter(|pos| {
            self.visible_tiles_by_chunk
                .get(pos)
                .is_some_and(|key| self.uploaded_tiles.contains(key))
        })
    }

    /// How many of the currently desired chunk patches are already retained.
    pub fn coverage(&self) -> FarTerrainLodCoverage {
        let ready_tiles = self
            .desired_tiles
            .values()
            .filter(|tile| self.resident_tiles.contains_tile(**tile))
            .count();
        FarTerrainLodCoverage {
            ready_tiles,
            target_tiles: self.desired_tiles.len(),
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
            desired_tiles: self.desired_tiles.len(),
            prefetch_tiles: self.prefetch_tiles.len(),
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
            resident_tiles_by_level: self.resident_tiles_by_level,
            visible_tiles_by_level: self.visible_tiles_by_level,
            double_resident_tiles: self.double_resident_tiles,
            max_double_resident_tiles: self.max_double_resident_tiles,
            level_flips: self.level_flips,
            max_level_flips_per_tile: self.max_level_flips_per_tile,
        }
    }

    /// Clone the exact bounded lifecycle sets for an explicit settle probe.
    /// This is intentionally an accessor rather than mirrored diagnostic state:
    /// callers pay for collection only when they request a snapshot.
    pub fn settle_snapshot(&self) -> FarTerrainLodSettleSnapshot {
        FarTerrainLodSettleSnapshot {
            desired_tiles: self
                .desired_tiles
                .iter()
                .map(|(pos, tile)| (*pos, tile.level))
                .collect(),
            prefetch_tiles: self
                .prefetch_tiles
                .iter()
                .map(|(pos, tile)| (*pos, tile.level))
                .collect(),
            resident_tiles: self.resident_tiles.tile_keys().collect(),
            uploaded_tiles: self.uploaded_tiles.clone(),
            published_tiles_by_chunk: self.visible_tiles_by_chunk.clone(),
            visible_tiles: self.frame.visible_tiles.clone(),
            pending_builds: self
                .pending_builds
                .iter()
                .map(|request| request.key)
                .collect(),
            inflight_builds: self.inflight_builds.keys().copied().collect(),
            queued_uploads: self.uploads.queued_upload_tile_keys().collect(),
            queued_removals: self.uploads.queued_removal_tile_keys().collect(),
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
        self.movement_direction = (0, 0);
        self.desired_tiles.clear();
        self.prefetch_tiles.clear();
        self.level_history.clear();
        self.visible_tiles_by_chunk.clear();
        self.dirty_tiles.clear();
        self.level_flip_counts.clear();
        self.resident_tiles_by_level = [0; FAR_TERRAIN_LOD_LEVEL_COUNT];
        self.visible_tiles_by_level = [0; FAR_TERRAIN_LOD_LEVEL_COUNT];
        self.double_resident_tiles = 0;
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

    fn update_target(&mut self, key: FarTerrainLodBuildKey, now: MonotonicInstant) {
        if self.view_key == Some(key) {
            return;
        }
        if let Some(previous) = self.view_key {
            let direction = (
                (key.center.x - previous.center.x).signum(),
                (key.center.z - previous.center.z).signum(),
            );
            if direction != (0, 0) {
                self.movement_direction = direction;
            }
        }
        let desired_queue = capped_far_lod_chunk_positions(key);
        let desired_positions = desired_queue.iter().copied().collect::<BTreeSet<_>>();
        let prefetch_queue = far_lod_prefetch_chunk_positions(
            key,
            &desired_positions,
            max_retained_lod_patches(key).saturating_sub(desired_queue.len()),
        );
        let previous_desired = std::mem::take(&mut self.desired_tiles);
        let previous_prefetch = std::mem::take(&mut self.prefetch_tiles);
        let previous_targets = previous_desired
            .iter()
            .chain(&previous_prefetch)
            .map(|(pos, tile)| (*pos, *tile))
            .collect::<BTreeMap<_, _>>();
        let mut desired_tiles = BTreeMap::new();
        let mut prefetch_tiles = BTreeMap::new();
        for (pos, prefetch) in desired_queue
            .iter()
            .map(|pos| (*pos, false))
            .chain(prefetch_queue.iter().map(|pos| (*pos, true)))
        {
            let distance = chunk_distance_from_center_chunk(key.center, pos);
            let raw_level = far_lod_raw_level(key, distance);
            let level = far_lod_stabilized_level(
                key,
                distance,
                self.level_history.get(&pos).copied(),
                raw_level,
            );
            if self
                .level_history
                .insert(pos, level)
                .is_some_and(|old| old != level)
            {
                self.level_flips = self.level_flips.saturating_add(1);
                let count = self.level_flip_counts.entry(pos).or_default();
                *count += 1;
                self.max_level_flips_per_tile = self.max_level_flips_per_tile.max(*count);
            }
            let tile = LodTileKey::new(pos, level);
            if prefetch {
                prefetch_tiles.insert(pos, tile);
            } else {
                desired_tiles.insert(pos, tile);
            }
        }
        let targets = desired_tiles
            .iter()
            .chain(&prefetch_tiles)
            .map(|(pos, tile)| (*pos, *tile))
            .collect::<BTreeMap<_, _>>();
        let changed_chunks = previous_targets
            .keys()
            .copied()
            .chain(targets.keys().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|pos| previous_targets.get(pos) != targets.get(pos))
            .collect::<Vec<_>>();
        self.desired_tiles = desired_tiles;
        self.prefetch_tiles = prefetch_tiles;
        for pos in changed_chunks {
            if let Some(tile) = self.target_tile(pos) {
                self.dirty_tiles.insert(tile);
            }
            for neighbor in cardinal_chunk_neighbors(pos) {
                if let Some(tile) = self.target_tile(neighbor) {
                    self.dirty_tiles.insert(tile);
                }
            }
        }
        let desired_tiles = &self.desired_tiles;
        let prefetch_tiles = &self.prefetch_tiles;
        self.dirty_tiles.retain(|tile| {
            desired_tiles
                .get(&tile.chunk)
                .or_else(|| prefetch_tiles.get(&tile.chunk))
                == Some(tile)
        });
        let mut promoted_replacement_removals = BTreeSet::new();
        for tile in self.desired_tiles.values().copied() {
            if self.uploaded_tiles.contains(&tile)
                && let Some(old) = self.visible_tiles_by_chunk.insert(tile.chunk, tile)
                && old != tile
            {
                promoted_replacement_removals.insert(old);
            }
        }
        if !promoted_replacement_removals.is_empty() {
            self.uploads
                .enqueue(Vec::new(), promoted_replacement_removals, 0);
        }
        self.view_key = Some(key);
        let mut build_candidates = targets.values().copied().collect::<Vec<_>>();
        build_candidates.sort_by_key(|tile| self.build_priority(key, *tile));
        self.pending_builds = build_candidates
            .into_iter()
            .filter(|tile| {
                (!self.resident_tiles.contains_tile(*tile) || self.dirty_tiles.contains(tile))
                    && !self.inflight_builds.contains_key(&tile)
            })
            .map(|tile| {
                FarTerrainLodBuildRequest::new_for_tile(
                    tile,
                    self.source_key.expect("far LOD source initialized"),
                    self.neighbor_sample_spacings(tile.chunk),
                    self.materials.clone(),
                    now,
                )
            })
            .collect();
        self.evict_resident_tiles(key);
    }

    fn target_tile(&self, pos: ChunkPos) -> Option<LodTileKey> {
        self.desired_tiles
            .get(&pos)
            .or_else(|| self.prefetch_tiles.get(&pos))
            .copied()
    }

    fn build_priority(
        &self,
        key: FarTerrainLodBuildKey,
        tile: LodTileKey,
    ) -> (u8, u32, std::cmp::Reverse<i32>, u32, i32, i32) {
        let desired = self.desired_tiles.get(&tile.chunk) == Some(&tile);
        let has_coverage = self.visible_tiles_by_chunk.contains_key(&tile.chunk);
        let uploaded = self.uploaded_tiles.contains(&tile);
        let class = match (desired, has_coverage, uploaded) {
            (true, false, _) => 0,  // missing visible coverage
            (false, _, false) => 1, // movement guard not built yet
            (true, true, _) => 2,   // quality/level replacement
            (false, _, true) => 3,  // prefetched seam refresh
        };
        let distance = chunk_distance_from_center_chunk(key.center, tile.chunk);
        let (inner_radius, outer_radius) = far_lod_chunk_radii(key);
        let exposure_steps = if desired {
            0
        } else if distance > outer_radius {
            distance - outer_radius
        } else {
            inner_radius.saturating_sub(distance).saturating_add(1)
        };
        let leading_projection = if !desired && distance > outer_radius {
            (tile.chunk.x - key.center.x) * self.movement_direction.0
                + (tile.chunk.z - key.center.z) * self.movement_direction.1
        } else {
            0
        };
        (
            class,
            exposure_steps,
            std::cmp::Reverse(leading_projection),
            distance,
            tile.chunk.z,
            tile.chunk.x,
        )
    }

    fn submit_builds<C: FarTerrainLodCompiler>(
        &mut self,
        compiler: &mut C,
        build_budget: usize,
    ) -> Result<()> {
        let admitted = build_budget.min(compiler.available_far_lod_job_slots());
        let candidates = self.pending_builds.len();
        let mut submitted = 0usize;
        for _ in 0..candidates {
            if submitted >= admitted {
                break;
            }
            let Some(request) = self.pending_builds.pop_front() else {
                break;
            };
            if self.target_tile(request.key.chunk) != Some(request.key)
                || (self.resident_tiles.contains_tile(request.key)
                    && !self.dirty_tiles.contains(&request.key))
                || self.inflight_builds.contains_key(&request.key)
            {
                continue;
            }
            let replacement =
                self.visible_tiles_by_chunk
                    .get(&request.key.chunk)
                    .is_some_and(|visible| *visible != request.key)
                    || self.resident_tiles.tile_keys().any(|resident| {
                        resident.chunk == request.key.chunk && resident != request.key
                    });
            if replacement
                && self.active_level_transition_count() >= MAX_FAR_TERRAIN_LOD_DOUBLE_RESIDENT_TILES
            {
                // A saturated replacement allowance must not block fresh
                // missing coverage queued behind this request. Keep the
                // replacement pending and scan the remainder of the bounded
                // queue for work that does not require double residency.
                self.pending_builds.push_back(request);
                continue;
            }
            let key = request.key;
            let queued_at = request.queued_at;
            compiler.submit_far_lod(request)?;
            self.inflight_builds.insert(key, queued_at);
            self.submitted_builds = self.submitted_builds.saturating_add(1);
            submitted += 1;
        }
        Ok(())
    }

    fn ensure_pending_builds(&mut self, now: MonotonicInstant) {
        let mut queued = self
            .pending_builds
            .iter()
            .map(|request| request.key)
            .chain(self.inflight_builds.keys().copied())
            .collect::<BTreeSet<_>>();
        let mut candidates = self
            .desired_tiles
            .values()
            .chain(self.prefetch_tiles.values())
            .copied()
            .collect::<Vec<_>>();
        if let Some(view) = self.view_key {
            candidates.sort_by_key(|tile| self.build_priority(view, *tile));
        }
        for tile in candidates {
            if queued.contains(&tile)
                || (self.resident_tiles.contains_tile(tile) && !self.dirty_tiles.contains(&tile))
            {
                continue;
            }
            self.pending_builds
                .push_back(FarTerrainLodBuildRequest::new_for_tile(
                    tile,
                    self.source_key.expect("far LOD source initialized"),
                    self.neighbor_sample_spacings(tile.chunk),
                    self.materials.clone(),
                    now,
                ));
            queued.insert(tile);
        }
    }

    fn accept_completed<C: FarTerrainLodCompiler>(&mut self, compiler: &mut C) -> Result<()> {
        let completed = compiler.try_recv_completed_far_lod()?;
        for result in completed {
            self.inflight_builds.remove(&result.key);
            let retain = self.source_key == Some(result.source_key)
                && self.target_tile(result.key.chunk) == Some(result.key)
                && self.neighbor_sample_spacings(result.key.chunk)
                    == result.neighbor_sample_spacings
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
            self.insert_resident_tile(result.key, metadata);
            self.dirty_tiles.remove(&result.key);
            self.uploads.enqueue(vec![result.mesh], BTreeSet::new(), 1);
            self.completed_builds = self.completed_builds.saturating_add(1);
            self.max_double_resident_tiles = self
                .max_double_resident_tiles
                .max(self.double_resident_tiles);
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
        let limit =
            max_retained_lod_patches(key).saturating_add(MAX_FAR_TERRAIN_LOD_DOUBLE_RESIDENT_TILES);
        if self.resident_tiles.len().saturating_sub(removals.len()) > limit {
            let mut eviction_order = self
                .resident_tiles
                .tile_keys()
                .filter(|tile| !removals.contains(tile))
                .map(|tile| {
                    let desired = self.target_tile(tile.chunk) == Some(tile);
                    let visible = self.visible_tiles_by_chunk.get(&tile.chunk) == Some(&tile);
                    let distance = chunk_distance_from_center_chunk(key.center, tile.chunk);
                    (tile, desired, visible, distance)
                })
                .collect::<Vec<_>>();
            eviction_order.sort_by_key(|(tile, desired, visible, distance)| {
                (
                    *desired,
                    *visible,
                    std::cmp::Reverse(*distance),
                    std::cmp::Reverse(tile.chunk.z),
                    std::cmp::Reverse(tile.chunk.x),
                )
            });
            for (tile, _, _, _) in eviction_order
                .into_iter()
                .take(self.resident_tiles.len().saturating_sub(removals.len()) - limit)
            {
                removals.insert(tile);
            }
        }
        for tile in &removals {
            self.remove_resident_tile(*tile);
            self.dirty_tiles.remove(tile);
            if self.visible_tiles_by_chunk.get(&tile.chunk) == Some(tile) {
                self.visible_tiles_by_chunk.remove(&tile.chunk);
            }
        }
        if !removals.is_empty() {
            self.uploads.enqueue(Vec::new(), removals, 0);
        }
        let retain_radius = far_lod_retain_radius(key);
        self.level_history
            .retain(|pos, _| chunk_distance_from_center_chunk(key.center, *pos) <= retain_radius);
        self.level_flip_counts
            .retain(|pos, _| chunk_distance_from_center_chunk(key.center, *pos) <= retain_radius);
    }

    fn neighbor_sample_spacings(&self, pos: ChunkPos) -> [u32; 4] {
        let base = self
            .source_key
            .map_or(DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS, |key| {
                key.sample_spacing_blocks
            });
        let own_level = self.target_tile(pos).map_or(1, |tile| tile.level);
        cardinal_chunk_neighbors(pos).map(|neighbor| {
            let level = self
                .target_tile(neighbor)
                .map_or(own_level, |tile| tile.level);
            far_lod_sample_spacing_for_level(base, level)
        })
    }

    fn insert_resident_tile(&mut self, tile: LodTileKey, metadata: FarTerrainLodTileMetadata) {
        if !self.resident_tiles.contains_tile(tile) {
            let chunk_level_count = self
                .resident_tiles
                .tile_keys()
                .filter(|resident| resident.chunk == tile.chunk)
                .count();
            if chunk_level_count == 1 {
                self.double_resident_tiles = self.double_resident_tiles.saturating_add(1);
            }
            if let Some(count) = self
                .resident_tiles_by_level
                .get_mut(tile.level.saturating_sub(1) as usize)
            {
                *count += 1;
            }
        }
        self.resident_tiles.insert(tile, metadata);
    }

    fn remove_resident_tile(&mut self, tile: LodTileKey) {
        if !self.resident_tiles.contains_tile(tile) {
            return;
        }
        let chunk_level_count = self
            .resident_tiles
            .tile_keys()
            .filter(|resident| resident.chunk == tile.chunk)
            .count();
        if chunk_level_count == 2 {
            self.double_resident_tiles = self.double_resident_tiles.saturating_sub(1);
        }
        if let Some(count) = self
            .resident_tiles_by_level
            .get_mut(tile.level.saturating_sub(1) as usize)
        {
            *count = count.saturating_sub(1);
        }
        self.resident_tiles.remove(tile);
    }

    fn active_level_transition_count(&self) -> usize {
        self.desired_tiles
            .iter()
            .chain(&self.prefetch_tiles)
            .filter(|(pos, desired)| {
                (self
                    .visible_tiles_by_chunk
                    .get(pos)
                    .is_some_and(|visible| visible != *desired)
                    || self
                        .resident_tiles
                        .tile_keys()
                        .any(|resident| resident.chunk == **pos && resident != **desired))
                    && (self.resident_tiles.contains_tile(**desired)
                        || self.inflight_builds.contains_key(*desired))
            })
            .count()
    }
}

fn cardinal_chunk_neighbors(pos: ChunkPos) -> [ChunkPos; 4] {
    [
        ChunkPos::new(pos.x - 1, pos.z),
        ChunkPos::new(pos.x + 1, pos.z),
        ChunkPos::new(pos.x, pos.z - 1),
        ChunkPos::new(pos.x, pos.z + 1),
    ]
}

pub fn far_lod_sample_spacing_for_level(base_spacing: u32, level: u8) -> u32 {
    let shift = level.saturating_sub(1).min(2);
    base_spacing
        .max(1)
        .saturating_mul(1_u32 << shift)
        .min(CHUNK_WIDTH as u32)
}

fn far_lod_level_band_ends(key: FarTerrainLodBuildKey) -> [u32; FAR_TERRAIN_LOD_LEVEL_COUNT] {
    let (inner, outer) = far_lod_chunk_radii(key);
    let span = outer.saturating_sub(inner).max(1);
    let band_width = span.div_ceil(FAR_TERRAIN_LOD_LEVEL_COUNT as u32);
    [
        inner.saturating_add(band_width).min(outer),
        inner
            .saturating_add(band_width.saturating_mul(2))
            .min(outer),
        outer,
    ]
}

fn far_lod_raw_level(key: FarTerrainLodBuildKey, distance: u32) -> u8 {
    if !key.detail_mode.is_auto() {
        return 1;
    }
    let ends = far_lod_level_band_ends(key);
    if distance <= ends[0] {
        1
    } else if distance <= ends[1] {
        2
    } else {
        3
    }
}

fn far_lod_stabilized_level(
    key: FarTerrainLodBuildKey,
    distance: u32,
    previous: Option<u8>,
    raw: u8,
) -> u8 {
    if !key.detail_mode.is_auto() {
        return raw;
    }
    let Some(previous) = previous.filter(|level| (1..=3).contains(level)) else {
        return raw;
    };
    if previous == raw {
        return raw;
    }
    let ends = far_lod_level_band_ends(key);
    let min = match previous {
        1 => 0,
        2 => ends[0].saturating_add(1),
        _ => ends[1].saturating_add(1),
    };
    let max = ends[usize::from(previous - 1)];
    let guarded_min = min.saturating_sub(FAR_TERRAIN_LOD_LEVEL_HYSTERESIS_CHUNKS);
    let guarded_max = max.saturating_add(FAR_TERRAIN_LOD_LEVEL_HYSTERESIS_CHUNKS);
    if (guarded_min..=guarded_max).contains(&distance) {
        previous
    } else {
        raw
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

fn far_lod_desired_inner_radius(key: FarTerrainLodBuildKey) -> u32 {
    let (inner_chunk_radius, _) = far_lod_chunk_radii(key);
    inner_chunk_radius.saturating_sub(FAR_TERRAIN_LOD_HANDOFF_OVERLAP_CHUNKS.saturating_sub(1))
}

fn far_lod_chunk_positions(key: FarTerrainLodBuildKey) -> VecDeque<ChunkPos> {
    let (_, outer_chunk_radius) = far_lod_chunk_radii(key);
    let desired_inner_radius = far_lod_desired_inner_radius(key);
    let outer = outer_chunk_radius as i32;
    let mut positions = Vec::new();
    for dz in -outer..=outer {
        for dx in -outer..=outer {
            let pos = ChunkPos::new(key.center.x + dx, key.center.z + dz);
            let distance = dx.unsigned_abs().max(dz.unsigned_abs());
            if (desired_inner_radius..=outer_chunk_radius).contains(&distance) {
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

fn capped_far_lod_chunk_positions(key: FarTerrainLodBuildKey) -> VecDeque<ChunkPos> {
    let mut positions = far_lod_chunk_positions(key);
    positions.truncate(max_retained_lod_patches(key));
    positions
}

fn far_lod_prefetch_chunk_positions(
    key: FarTerrainLodBuildKey,
    desired_positions: &BTreeSet<ChunkPos>,
    capacity: usize,
) -> VecDeque<ChunkPos> {
    let (_, outer_radius) = far_lod_chunk_radii(key);
    let desired_inner_radius = far_lod_desired_inner_radius(key);
    let retain_radius = far_lod_retain_radius(key);
    let inner_guard_start = desired_inner_radius.saturating_sub(FAR_TERRAIN_LOD_INNER_GUARD_CHUNKS);
    let outer_guard_end = outer_radius.saturating_add(FAR_TERRAIN_LOD_OUTER_GUARD_CHUNKS);
    let mut positions = Vec::new();
    let retain = retain_radius as i32;
    for dz in -retain..=retain {
        for dx in -retain..=retain {
            let pos = ChunkPos::new(key.center.x + dx, key.center.z + dz);
            if desired_positions.contains(&pos) {
                continue;
            }
            let distance = dx.unsigned_abs().max(dz.unsigned_abs());
            let inner_guard = distance >= inner_guard_start && distance < desired_inner_radius;
            let outside_guard = distance > outer_radius && distance <= outer_guard_end;
            if inner_guard || outside_guard {
                positions.push(pos);
            }
        }
    }
    positions.sort_by_key(|pos| {
        let distance = chunk_distance_from_center_chunk(key.center, *pos);
        let outside = distance > outer_radius;
        (outside, distance, pos.z, pos.x)
    });
    positions.truncate(capacity);
    positions.into()
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

fn build_retained_far_terrain_lod_patch<S: FarTerrainSurfaceSource>(
    surface_chunks: &mut S,
    pos: ChunkPos,
    lod_level: u8,
    source_key: FarTerrainLodSourceKey,
    neighbor_sample_spacings: [u32; 4],
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
        lod_level,
        source_key.patch_build_key(pos),
        neighbor_sample_spacings,
        FarTerrainNormalCoverage::NoNormalChunks,
        materials,
    );
    FarTerrainLodPatch { vertices, indices }
}

#[cfg(test)]
pub(crate) fn compile_far_terrain_lod_request(
    request: FarTerrainLodBuildRequest,
) -> FarTerrainLodBuildResult {
    let mesh =
        compile_far_terrain_lod_worker_input(request.worker_input(), request.materials.as_deref());
    request
        .complete_worker_mesh(mesh)
        .expect("local far LOD compiler preserves the request tile key")
}

pub(crate) fn compile_far_terrain_lod_request_cached(
    request: FarTerrainLodBuildRequest,
    cache: &mut FarTerrainLodWorkerCache,
) -> FarTerrainLodBuildResult {
    let input = request.worker_input();
    let mesh =
        compile_far_terrain_lod_worker_input_cached(input, request.materials.as_deref(), cache);
    request
        .complete_worker_mesh(mesh)
        .expect("cached far LOD compiler preserves the request tile key")
}

pub fn compile_far_terrain_lod_worker_input_cached(
    input: FarTerrainLodWorkerInput,
    materials: Option<&FarTerrainLodMaterialPalette>,
    cache: &mut FarTerrainLodWorkerCache,
) -> FarTerrainLodTileMesh {
    let source_key = FarTerrainLodSourceKey {
        seed: input.seed,
        generation_profile: input.generation_profile,
        sample_spacing_blocks: input.sample_spacing_blocks.max(1),
        detail_mode: FarLodDetailMode::Auto,
        materials_available: materials.is_some(),
    };
    let patch = build_retained_far_terrain_lod_patch(
        cache,
        input.key.chunk,
        input.key.level,
        source_key,
        input.neighbor_sample_spacings,
        materials,
    );
    FarTerrainLodTileMesh::new(input.key, patch.vertices, patch.indices)
}

pub fn compile_far_terrain_lod_worker_input(
    input: FarTerrainLodWorkerInput,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> FarTerrainLodTileMesh {
    let mut surface_chunks = FarTerrainLodWorkerCache::default();
    compile_far_terrain_lod_worker_input_cached(input, materials, &mut surface_chunks)
}

#[cfg(test)]
fn build_far_terrain_lod_tile(key: FarTerrainLodBuildKey, pos: ChunkPos) -> FarTerrainLodTileMesh {
    compile_far_terrain_lod_request(FarTerrainLodBuildRequest::new(
        pos,
        FarTerrainLodSourceKey::new_with_profile(
            key.seed,
            key.generation_profile,
            FarTerrainLodConfig {
                enabled: true,
                detail_mode: key.detail_mode,
                start_margin_chunks: 0,
                extra_radius_chunks: 1,
                sample_spacing_blocks: key.sample_spacing_blocks,
            },
            false,
        ),
        None,
        MonotonicInstant::ZERO,
    ))
    .mesh
}

fn append_far_terrain_lod_chunk_patch<S: FarTerrainSurfaceSource>(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    surface_chunks: &mut S,
    seed: i64,
    pos: ChunkPos,
    lod_level: u8,
    key: FarTerrainLodBuildKey,
    neighbor_sample_spacings: [u32; 4],
    normal_coverage: FarTerrainNormalCoverage,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> FarTerrainLodVegetationReport {
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
                key.generation_profile,
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
    let mut vegetation_report = FarTerrainLodVegetationReport {
        summary_cells: cells
            .iter()
            .flatten()
            .filter(|cell| cell.sample.forest_summary_available)
            .count(),
        ..FarTerrainLodVegetationReport::default()
    };

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
                    neighbor_sample_spacings,
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

    let first_proxy_vertex = vertices.len() / 7;
    let first_proxy_index = indices.len();
    if key.generation_profile == WorldGenerationProfile::McloneOverworldV1
        && far_lod_tree_record_level_admitted(lod_level)
        && let Some(bounds) = far_lod_chunk_vegetation_bounds(pos)
    {
        let occurrences =
            surface_chunks.mclone_tree_occurrences(seed, key.generation_profile, bounds);
        vegetation_report.record_queries = 1;
        for occurrence in occurrences {
            if !far_lod_tree_record_admitted(lod_level, occurrence.record.landmark_rank) {
                continue;
            }
            vegetation_report.admitted_occurrences += 1;
            vegetation_report.proxy_boxes +=
                append_far_lod_tree_proxy(vertices, indices, occurrence, pos, materials);
        }
    }
    vegetation_report.proxy_vertices = vertices.len() / 7 - first_proxy_vertex;
    vegetation_report.proxy_indices = indices.len() - first_proxy_index;
    vegetation_report
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FarTerrainLodVegetationReport {
    summary_cells: usize,
    record_queries: usize,
    admitted_occurrences: usize,
    proxy_boxes: usize,
    proxy_vertices: usize,
    proxy_indices: usize,
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

fn sample_lod_cell<S: FarTerrainSurfaceSource>(
    chunks: &mut S,
    seed: i64,
    generation_profile: WorldGenerationProfile,
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> Option<FarTerrainLodCell> {
    let sample_x = x0 + (x1 - x0) / 2;
    let sample_z = z0 + (z1 - z0) / 2;
    surface_sample_world(
        chunks,
        seed,
        generation_profile,
        sample_x,
        sample_z,
        materials,
    )
    .map(|sample| FarTerrainLodCell {
        x0,
        x1,
        z0,
        z1,
        sample,
    })
}

fn neighbor_lod_cell<S: FarTerrainSurfaceSource>(
    cells: &[Option<FarTerrainLodCell>],
    cells_per_axis: usize,
    x_cell: usize,
    z_cell: usize,
    direction: LodCellDirection,
    surface_chunks: &mut S,
    seed: i64,
    key: FarTerrainLodBuildKey,
    neighbor_sample_spacings: [u32; 4],
    cell: FarTerrainLodCell,
    normal_coverage: FarTerrainNormalCoverage,
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
            let spacing =
                neighbor_sample_spacings[direction.index()].clamp(1, CHUNK_WIDTH as u32) as i32;
            let (x0, x1, z0, z1) = neighbor_cell_bounds_at_spacing(cell, direction, spacing);
            let neighbor = sample_lod_cell(
                surface_chunks,
                seed,
                key.generation_profile,
                x0,
                x1,
                z0,
                z1,
                materials,
            )?;
            Some(LodNeighborCell {
                cell: neighbor,
                region: neighbor_region(key, neighbor, normal_coverage),
            })
        }
    }
}

impl LodCellDirection {
    const fn index(self) -> usize {
        match self {
            Self::West => 0,
            Self::East => 1,
            Self::North => 2,
            Self::South => 3,
        }
    }
}

fn neighbor_cell_bounds_at_spacing(
    cell: FarTerrainLodCell,
    direction: LodCellDirection,
    spacing: i32,
) -> (i32, i32, i32, i32) {
    let center_x = cell.x0 + (cell.x1 - cell.x0) / 2;
    let center_z = cell.z0 + (cell.z1 - cell.z0) / 2;
    let aligned_x = center_x.div_euclid(spacing) * spacing;
    let aligned_z = center_z.div_euclid(spacing) * spacing;
    match direction {
        LodCellDirection::West => (cell.x0 - spacing, cell.x0, aligned_z, aligned_z + spacing),
        LodCellDirection::East => (cell.x1, cell.x1 + spacing, aligned_z, aligned_z + spacing),
        LodCellDirection::North => (aligned_x, aligned_x + spacing, cell.z0 - spacing, cell.z0),
        LodCellDirection::South => (aligned_x, aligned_x + spacing, cell.z1, cell.z1 + spacing),
    }
}

fn neighbor_region(
    key: FarTerrainLodBuildKey,
    cell: FarTerrainLodCell,
    normal_coverage: FarTerrainNormalCoverage,
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

fn forest_summary_colors(
    mut terrain: FarTerrainLodSurfaceColors,
    intent: McloneForestIntentSample,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> FarTerrainLodSurfaceColors {
    let Some(family) = intent.dominant_family else {
        return terrain;
    };
    let (_, crown) = far_lod_tree_colors(family, materials);
    let summary_weight = (intent.coverage * (0.30 + intent.density * 0.24)).clamp(0.0, 0.52);
    terrain.top = mix_color(terrain.top, crown, summary_weight);
    terrain.side = mix_color(terrain.side, crown, summary_weight * 0.42);
    terrain
}

fn mix_color(from: [f32; 4], to: [f32; 4], amount: f32) -> [f32; 4] {
    let amount = amount.clamp(0.0, 1.0);
    [
        from[0] + (to[0] - from[0]) * amount,
        from[1] + (to[1] - from[1]) * amount,
        from[2] + (to[2] - from[2]) * amount,
        from[3] + (to[3] - from[3]) * amount,
    ]
}

const fn far_lod_tree_record_level_admitted(lod_level: u8) -> bool {
    matches!(lod_level, 1 | 2)
}

const fn far_lod_tree_record_admitted(lod_level: u8, landmark_rank: u8) -> bool {
    match lod_level {
        1 => true,
        2 => landmark_rank >= 2,
        _ => false,
    }
}

fn far_lod_chunk_vegetation_bounds(pos: ChunkPos) -> Option<McloneVegetationBounds> {
    let min_x = chunk_min_block_coord(pos.x);
    let min_z = chunk_min_block_coord(pos.z);
    McloneVegetationBounds::new(
        min_x,
        min_z,
        min_x.checked_add(CHUNK_WIDTH - 1)?,
        min_z.checked_add(CHUNK_WIDTH - 1)?,
    )
    .ok()
}

fn far_lod_tree_colors(
    family: McloneTreeFamily,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> ([f32; 4], [f32; 4]) {
    let (log, leaves, trunk_fallback, crown_fallback) = match family {
        McloneTreeFamily::TemperateBroadleaf => (
            OAK_LOG,
            OAK_LEAVES,
            [0.34, 0.20, 0.09, 1.0],
            [0.16, 0.45, 0.20, 1.0],
        ),
        McloneTreeFamily::CoolWetConifer => (
            SPRUCE_LOG,
            SPRUCE_LEAVES,
            [0.28, 0.17, 0.08, 1.0],
            [0.10, 0.30, 0.18, 1.0],
        ),
        McloneTreeFamily::WarmDryAcacia => (
            ACACIA_LOG,
            ACACIA_LEAVES,
            [0.43, 0.26, 0.11, 1.0],
            [0.37, 0.50, 0.16, 1.0],
        ),
    };
    let trunk = materials
        .and_then(|palette| palette.colors_for_block(GeneratedBlockId(log), 64))
        .map_or(trunk_fallback, |colors| colors.side);
    let crown = materials
        .and_then(|palette| palette.colors_for_block(GeneratedBlockId(leaves), 64))
        .map_or(crown_fallback, |colors| colors.top);
    (trunk, crown)
}

fn append_far_lod_tree_proxy(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    occurrence: McloneTreeOccurrence,
    tile: ChunkPos,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> usize {
    let Ok(base) = occurrence.working_base() else {
        return 0;
    };
    let record = occurrence.record;
    let (trunk_color, crown_color) = far_lod_tree_colors(record.family, materials);
    let tile_min_x = chunk_min_block_coord(tile.x) as f32;
    let tile_min_z = chunk_min_block_coord(tile.z) as f32;
    let tile_max_x = tile_min_x + CHUNK_WIDTH as f32;
    let tile_max_z = tile_min_z + CHUNK_WIDTH as f32;
    let mut boxes = 0;
    boxes += usize::from(append_clipped_box(
        vertices,
        indices,
        [base.x as f32, base.y as f32, base.z as f32],
        [
            base.x as f32 + 1.0,
            base.y as f32 + f32::from(record.trunk_height),
            base.z as f32 + 1.0,
        ],
        tile_min_x,
        tile_max_x,
        tile_min_z,
        tile_max_z,
        trunk_color,
    ));

    let radius = f32::from(record.crown_radius);
    let depth = f32::from(record.crown_depth);
    let crown_top = base.y as f32 + f32::from(record.trunk_height) + 1.0;
    match record.family {
        McloneTreeFamily::TemperateBroadleaf => {
            boxes += usize::from(append_clipped_box(
                vertices,
                indices,
                [
                    base.x as f32 - radius,
                    crown_top - depth,
                    base.z as f32 - radius,
                ],
                [
                    base.x as f32 + radius + 1.0,
                    crown_top,
                    base.z as f32 + radius + 1.0,
                ],
                tile_min_x,
                tile_max_x,
                tile_min_z,
                tile_max_z,
                crown_color,
            ));
            let upper_radius = (radius - 1.0).max(1.0);
            boxes += usize::from(append_clipped_box(
                vertices,
                indices,
                [
                    base.x as f32 - upper_radius,
                    crown_top - depth * 0.45,
                    base.z as f32 - upper_radius,
                ],
                [
                    base.x as f32 + upper_radius + 1.0,
                    crown_top + 1.0,
                    base.z as f32 + upper_radius + 1.0,
                ],
                tile_min_x,
                tile_max_x,
                tile_min_z,
                tile_max_z,
                shade_color(crown_color, 1.06),
            ));
        }
        McloneTreeFamily::CoolWetConifer => {
            boxes += usize::from(append_clipped_box(
                vertices,
                indices,
                [
                    base.x as f32 - radius,
                    crown_top - depth,
                    base.z as f32 - radius,
                ],
                [
                    base.x as f32 + radius + 1.0,
                    crown_top - depth * 0.30,
                    base.z as f32 + radius + 1.0,
                ],
                tile_min_x,
                tile_max_x,
                tile_min_z,
                tile_max_z,
                shade_color(crown_color, 0.90),
            ));
            let upper_radius = (radius * 0.62).max(1.0);
            boxes += usize::from(append_clipped_box(
                vertices,
                indices,
                [
                    base.x as f32 - upper_radius,
                    crown_top - depth * 0.42,
                    base.z as f32 - upper_radius,
                ],
                [
                    base.x as f32 + upper_radius + 1.0,
                    crown_top,
                    base.z as f32 + upper_radius + 1.0,
                ],
                tile_min_x,
                tile_max_x,
                tile_min_z,
                tile_max_z,
                crown_color,
            ));
        }
        McloneTreeFamily::WarmDryAcacia => {
            let directions = [[0.0, -1.0], [1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]];
            let direction = directions[usize::from(record.orientation & 3)];
            let side = [-direction[1], direction[0]];
            for (offset, y_offset, shade) in [
                ([direction[0] * 2.0, direction[1] * 2.0], 0.0, 1.0),
                ([side[0] * 1.5, side[1] * 1.5], -1.0, 0.92),
            ] {
                boxes += usize::from(append_clipped_box(
                    vertices,
                    indices,
                    [
                        base.x as f32 + offset[0] - radius,
                        crown_top - 2.0 + y_offset,
                        base.z as f32 + offset[1] - radius,
                    ],
                    [
                        base.x as f32 + offset[0] + radius + 1.0,
                        crown_top + y_offset,
                        base.z as f32 + offset[1] + radius + 1.0,
                    ],
                    tile_min_x,
                    tile_max_x,
                    tile_min_z,
                    tile_max_z,
                    shade_color(crown_color, shade),
                ));
            }
        }
    }
    boxes
}

#[allow(clippy::too_many_arguments)]
fn append_clipped_box(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    min: [f32; 3],
    max: [f32; 3],
    tile_min_x: f32,
    tile_max_x: f32,
    tile_min_z: f32,
    tile_max_z: f32,
    color: [f32; 4],
) -> bool {
    let min = [min[0].max(tile_min_x), min[1], min[2].max(tile_min_z)];
    let max = [max[0].min(tile_max_x), max[1], max[2].min(tile_max_z)];
    if max[0] <= min[0] || max[1] <= min[1] || max[2] <= min[2] {
        return false;
    }
    append_box(vertices, indices, min, max, color);
    true
}

fn append_box(
    vertices: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    min: [f32; 3],
    max: [f32; 3],
    color: [f32; 4],
) {
    let base = u32::try_from(vertices.len() / 7).expect("far terrain LOD vertex count fits u32");
    for position in [
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [min[0], max[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], min[1], max[2]],
        [max[0], min[1], max[2]],
        [min[0], max[1], max[2]],
        [max[0], max[1], max[2]],
    ] {
        vertices.extend_from_slice(&position);
        vertices.extend_from_slice(&color);
    }
    indices.extend(
        [
            0, 1, 4, 1, 5, 4, // bottom
            2, 6, 3, 3, 6, 7, // top
            0, 2, 1, 1, 2, 3, // north
            4, 5, 6, 5, 7, 6, // south
            0, 4, 2, 4, 6, 2, // west
            1, 3, 5, 3, 7, 5, // east
        ]
        .into_iter()
        .map(|index| base + index),
    );
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
    forest_summary_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FarTerrainSurfaceColumn {
    y: i32,
    block: GeneratedBlockId,
}

trait FarTerrainSurfaceSource {
    fn sample_world(
        &mut self,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        world_x: i32,
        world_z: i32,
        materials: Option<&FarTerrainLodMaterialPalette>,
    ) -> Option<FarTerrainSurfaceSample>;

    fn mclone_tree_occurrences(
        &mut self,
        _seed: i64,
        _generation_profile: WorldGenerationProfile,
        _bounds: McloneVegetationBounds,
    ) -> Vec<McloneTreeOccurrence> {
        Vec::new()
    }
}

impl FarTerrainSurfaceSource for BTreeMap<ChunkPos, GeneratedChunk> {
    fn sample_world(
        &mut self,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        world_x: i32,
        world_z: i32,
        materials: Option<&FarTerrainLodMaterialPalette>,
    ) -> Option<FarTerrainSurfaceSample> {
        let pos = ChunkPos::from_block_coords(world_x, world_z);
        let chunk = self.entry(pos).or_insert_with(|| {
            generate_far_lod_surface_chunk(generation_profile, seed, pos.x, pos.z)
        });
        let local_x = world_x - chunk_min_block_coord(pos.x);
        let local_z = world_z - chunk_min_block_coord(pos.z);
        surface_sample(chunk, local_x, local_z, materials)
    }
}

const MAX_FAR_LOD_WORKER_SURFACE_CHUNKS: usize = 128;

fn generate_far_lod_surface_chunk(
    profile: WorldGenerationProfile,
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> GeneratedChunk {
    match profile {
        WorldGenerationProfile::Overworld => {
            generate_overworld_surface_chunk(seed, chunk_x, chunk_z)
        }
        WorldGenerationProfile::FlatGrassV1 => generate_flat_grass_chunk(chunk_x, chunk_z),
        WorldGenerationProfile::SmallIslandV1 => {
            generate_small_island_surface_chunk(seed, chunk_x, chunk_z)
        }
        WorldGenerationProfile::McloneOverworldV1 => {
            generate_mclone_overworld_surface_chunk(seed, chunk_x, chunk_z)
        }
        WorldGenerationProfile::AlphaV1 { winter } => generate_alpha_stage_chunk(
            seed,
            chunk_x,
            chunk_z,
            winter,
            AlphaGenerationStage::Surface,
        ),
        WorldGenerationProfile::BetaV1 => {
            generate_beta_stage_chunk(seed, chunk_x, chunk_z, BetaGenerationStage::Surface)
        }
        // Authored terrain has no deterministic procedural source. Existing
        // callers keep far LOD disabled for this profile; retain the historic
        // Overworld fallback if one is requested directly.
        WorldGenerationProfile::AuthoredOnly { .. } => {
            generate_overworld_surface_chunk(seed, chunk_x, chunk_z)
        }
    }
}

#[derive(Debug, Default)]
pub struct FarTerrainLodWorkerCache {
    source: Option<(i64, WorldGenerationProfile)>,
    chunks: BTreeMap<ChunkPos, Vec<Option<FarTerrainSurfaceColumn>>>,
    insertion_order: VecDeque<ChunkPos>,
    mclone_stream_plans: Option<McloneOverworldStreamPlanCache>,
    mclone_vegetation: Option<McloneOverworldVegetationPlanCache>,
}

impl FarTerrainLodWorkerCache {
    fn reset_for_source(&mut self, seed: i64, generation_profile: WorldGenerationProfile) {
        let source = (seed, generation_profile);
        if self.source == Some(source) {
            return;
        }
        self.source = Some(source);
        self.chunks.clear();
        self.insertion_order.clear();
        self.mclone_stream_plans =
            (generation_profile == WorldGenerationProfile::McloneOverworldV1).then(|| {
                McloneOverworldStreamPlanCache::new(
                    seed,
                    McloneOverworldSamplingTopology::Unbounded,
                )
            });
        self.mclone_vegetation = (generation_profile == WorldGenerationProfile::McloneOverworldV1)
            .then(|| {
                McloneOverworldVegetationPlanCache::new(McloneVegetationSource::new(
                    seed,
                    McloneOverworldSamplingTopology::Unbounded,
                ))
            });
    }

    fn insert_generated_chunk(
        &mut self,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        pos: ChunkPos,
    ) {
        self.reset_for_source(seed, generation_profile);
        while self.chunks.len() >= MAX_FAR_LOD_WORKER_SURFACE_CHUNKS {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            self.chunks.remove(&oldest);
        }
        let chunk = if let Some(stream_plans) = self.mclone_stream_plans.as_mut() {
            generate_mclone_overworld_surface_chunk_with_stream_cache(
                seed,
                McloneOverworldSamplingTopology::Unbounded,
                pos.x,
                pos.z,
                stream_plans,
            )
        } else {
            generate_far_lod_surface_chunk(generation_profile, seed, pos.x, pos.z)
        };
        let mut columns = Vec::with_capacity((CHUNK_WIDTH * CHUNK_WIDTH) as usize);
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                columns.push(surface_column(&chunk, local_x, local_z));
            }
        }
        self.chunks.insert(pos, columns);
        self.insertion_order.push_back(pos);
    }
}

impl FarTerrainSurfaceSource for FarTerrainLodWorkerCache {
    fn sample_world(
        &mut self,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        world_x: i32,
        world_z: i32,
        materials: Option<&FarTerrainLodMaterialPalette>,
    ) -> Option<FarTerrainSurfaceSample> {
        self.reset_for_source(seed, generation_profile);
        let pos = ChunkPos::from_block_coords(world_x, world_z);
        if !self.chunks.contains_key(&pos) {
            self.insert_generated_chunk(seed, generation_profile, pos);
        }
        let local_x = world_x - chunk_min_block_coord(pos.x);
        let local_z = world_z - chunk_min_block_coord(pos.z);
        let index = (local_z * CHUNK_WIDTH + local_x) as usize;
        let column = self.chunks.get(&pos)?.get(index).copied().flatten()?;
        let mut colors = surface_colors(column.block, column.y, materials);
        let forest_summary_available =
            generation_profile == WorldGenerationProfile::McloneOverworldV1;
        if let Some(intent) = self
            .mclone_vegetation
            .as_mut()
            .and_then(|cache| cache.forest_intent_at(world_x, world_z).ok())
        {
            colors = forest_summary_colors(colors, intent, materials);
        }
        Some(FarTerrainSurfaceSample {
            y: column.y as f32,
            top_color: colors.top,
            side_color: colors.side,
            forest_summary_available,
        })
    }

    fn mclone_tree_occurrences(
        &mut self,
        seed: i64,
        generation_profile: WorldGenerationProfile,
        bounds: McloneVegetationBounds,
    ) -> Vec<McloneTreeOccurrence> {
        self.reset_for_source(seed, generation_profile);
        self.mclone_vegetation
            .as_mut()
            .and_then(|cache| cache.tree_records_intersecting(bounds).ok())
            .unwrap_or_default()
    }
}

fn surface_column(
    chunk: &GeneratedChunk,
    local_x: i32,
    local_z: i32,
) -> Option<FarTerrainSurfaceColumn> {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        let block = chunk.block_at_y(local_x, y, local_z);
        if !is_air_like(block.raw()) {
            return Some(FarTerrainSurfaceColumn { y: y + 1, block });
        }
    }
    None
}

fn surface_sample(
    chunk: &GeneratedChunk,
    local_x: i32,
    local_z: i32,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> Option<FarTerrainSurfaceSample> {
    let column = surface_column(chunk, local_x, local_z)?;
    let colors = surface_colors(column.block, column.y, materials);
    Some(FarTerrainSurfaceSample {
        y: column.y as f32,
        top_color: colors.top,
        side_color: colors.side,
        forest_summary_available: false,
    })
}

fn surface_sample_world<S: FarTerrainSurfaceSource>(
    chunks: &mut S,
    seed: i64,
    generation_profile: WorldGenerationProfile,
    world_x: i32,
    world_z: i32,
    materials: Option<&FarTerrainLodMaterialPalette>,
) -> Option<FarTerrainSurfaceSample> {
    chunks.sample_world(seed, generation_profile, world_x, world_z, materials)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn far_lod_mesh_fingerprint(mesh: &FarTerrainLodTileMesh) -> u64 {
        mesh.vertices()
            .iter()
            .map(|value| u64::from(value.to_bits()))
            .chain(mesh.indices().iter().copied().map(u64::from))
            .fold(0xcbf2_9ce4_8422_2325, |hash, value| {
                (hash ^ value).wrapping_mul(0x1000_0000_01b3)
            })
    }

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
                .advance_for_camera(config, 12345, center, 0, None, compiler, build_budget)
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
        let visible = cache.desired_tiles.keys().copied().collect();
        cache.drain_render_uploads(MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES, compiler);
        cache.prepare_render_update(&visible).clone()
    }

    #[test]
    fn settle_snapshot_exposes_exact_producer_lifecycle_sets() {
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
                &mut compiler,
                0,
            )
            .unwrap();

        let queued = cache.settle_snapshot();
        assert!(!queued.desired_tiles.is_empty());
        assert_eq!(
            queued.pending_builds.len(),
            queued.desired_tiles.len() + queued.prefetch_tiles.len()
        );
        assert!(queued.resident_tiles.is_empty());
        assert!(queued.uploaded_tiles.is_empty());
        assert!(queued.visible_tiles.is_empty());

        advance_until_complete(&mut cache, &mut compiler, config, ChunkPos::new(0, 0), 64);
        let built = cache.settle_snapshot();
        assert_eq!(
            built.resident_tiles.len(),
            built.desired_tiles.len() + built.prefetch_tiles.len()
        );
        assert_eq!(built.queued_uploads, built.resident_tiles);
        assert!(built.pending_builds.is_empty());
        assert!(built.inflight_builds.is_empty());

        let frame = upload_all_desired(&mut cache, &mut compiler);
        let visible = cache.settle_snapshot();
        assert_eq!(visible.uploaded_tiles, visible.resident_tiles);
        assert_eq!(visible.visible_tiles, frame.visible_tiles);
        assert_eq!(visible.visible_tiles.len(), visible.desired_tiles.len());
        assert!(visible.queued_uploads.is_empty());
        assert!(visible.queued_removals.is_empty());
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
    fn far_terrain_lod_level_bands_map_to_4_8_16_spacing() {
        let key = FarTerrainLodBuildKey::new(
            12345,
            ChunkPos::new(0, 0),
            0,
            FarTerrainLodConfig::enabled(),
        );

        assert_eq!(far_lod_level_band_ends(key), [4, 8, 12]);
        assert_eq!(far_lod_raw_level(key, 4), 1);
        assert_eq!(far_lod_raw_level(key, 5), 2);
        assert_eq!(far_lod_raw_level(key, 8), 2);
        assert_eq!(far_lod_raw_level(key, 9), 3);
        assert_eq!(far_lod_sample_spacing_for_level(4, 1), 4);
        assert_eq!(far_lod_sample_spacing_for_level(4, 2), 8);
        assert_eq!(far_lod_sample_spacing_for_level(4, 3), 16);
    }

    #[test]
    fn fixed_detail_modes_use_one_level_at_the_selected_spacing() {
        for (mode, spacing) in [
            (FarLodDetailMode::Fixed4, 4),
            (FarLodDetailMode::Fixed8, 8),
            (FarLodDetailMode::Fixed16, 16),
        ] {
            let config = FarTerrainLodConfig::enabled().with_detail_mode(mode);
            let key = FarTerrainLodBuildKey::new(12345, ChunkPos::new(0, 0), 0, config);
            let source = FarTerrainLodSourceKey::new(12345, config, false);

            assert_eq!(source.detail_mode, mode);
            assert_eq!(source.sample_spacing_blocks, spacing);
            assert_eq!(far_lod_raw_level(key, 12), 1);
            assert_eq!(far_lod_stabilized_level(key, 12, Some(3), 1), 1);
            assert_eq!(
                far_lod_sample_spacing_for_level(source.sample_spacing_blocks, 1),
                spacing
            );
        }
    }

    #[test]
    fn auto_and_fixed_four_have_distinct_source_keys() {
        let auto = FarTerrainLodSourceKey::new(
            12345,
            FarTerrainLodConfig::enabled().with_detail_mode(FarLodDetailMode::Auto),
            false,
        );
        let fixed = FarTerrainLodSourceKey::new(
            12345,
            FarTerrainLodConfig::enabled().with_detail_mode(FarLodDetailMode::Fixed4),
            false,
        );

        assert_ne!(auto, fixed);
        assert_eq!(auto.sample_spacing_blocks, fixed.sample_spacing_blocks);
    }

    #[test]
    fn generation_profiles_have_distinct_far_lod_source_keys() {
        let config = FarTerrainLodConfig::enabled();
        let overworld = FarTerrainLodSourceKey::new_with_profile(
            12345,
            WorldGenerationProfile::Overworld,
            config,
            false,
        );
        let mclone = FarTerrainLodSourceKey::new_with_profile(
            12345,
            WorldGenerationProfile::McloneOverworldV1,
            config,
            false,
        );

        assert_ne!(overworld, mclone);
        let request = FarTerrainLodBuildRequest::new(
            ChunkPos::new(0, 0),
            mclone,
            None,
            MonotonicInstant::ZERO,
        );
        assert_eq!(
            request.worker_input().generation_profile,
            WorldGenerationProfile::McloneOverworldV1
        );
    }

    #[test]
    fn coarser_fixed_detail_modes_reduce_tile_mesh_work() {
        let mesh_counts = |mode| {
            let source = FarTerrainLodSourceKey::new(
                12345,
                FarTerrainLodConfig::enabled().with_detail_mode(mode),
                false,
            );
            let mesh = compile_far_terrain_lod_request(FarTerrainLodBuildRequest::new(
                ChunkPos::new(0, 0),
                source,
                None,
                MonotonicInstant::ZERO,
            ))
            .mesh;
            (mesh.vertex_count(), mesh.index_count())
        };

        let fixed_4 = mesh_counts(FarLodDetailMode::Fixed4);
        let fixed_8 = mesh_counts(FarLodDetailMode::Fixed8);
        let fixed_16 = mesh_counts(FarLodDetailMode::Fixed16);
        assert!(fixed_4.0 > fixed_8.0 && fixed_8.0 > fixed_16.0);
        assert!(fixed_4.1 > fixed_8.1 && fixed_8.1 > fixed_16.1);
    }

    #[test]
    fn far_terrain_lod_level_hysteresis_prevents_boundary_thrash() {
        let key = FarTerrainLodBuildKey::new(
            12345,
            ChunkPos::new(0, 0),
            0,
            FarTerrainLodConfig::enabled(),
        );

        assert_eq!(far_lod_stabilized_level(key, 5, Some(1), 2), 1);
        assert_eq!(far_lod_stabilized_level(key, 6, Some(1), 2), 1);
        assert_eq!(far_lod_stabilized_level(key, 7, Some(1), 2), 2);
        assert_eq!(far_lod_stabilized_level(key, 4, Some(2), 1), 2);
        assert_eq!(far_lod_stabilized_level(key, 3, Some(2), 1), 2);
        assert_eq!(far_lod_stabilized_level(key, 2, Some(2), 1), 1);
    }

    #[test]
    fn far_terrain_lod_requests_carry_cross_level_neighbor_spacing() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        cache
            .advance_for_camera(
                FarTerrainLodConfig::enabled(),
                12345,
                ChunkPos::new(0, 0),
                0,
                None,
                &mut compiler,
                0,
            )
            .unwrap();

        let request = cache
            .pending_builds
            .iter()
            .find(|request| request.key.chunk == ChunkPos::new(4, 0))
            .expect("level-1 boundary tile is queued");
        assert_eq!(request.key.level, 1);
        assert_eq!(request.worker_input().sample_spacing_blocks, 4);
        assert_eq!(request.worker_input().neighbor_sample_spacings[1], 8);
    }

    #[test]
    fn far_terrain_lod_boundary_oscillation_records_no_tile_flips() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled();
        let watched = ChunkPos::new(5, 0);

        for center_x in std::iter::once(0).chain([1, 0].into_iter().cycle().take(20)) {
            cache
                .advance_for_camera(
                    config,
                    12345,
                    ChunkPos::new(center_x, 0),
                    0,
                    None,
                    &mut compiler,
                    0,
                )
                .unwrap();
        }

        assert_eq!(cache.level_history.get(&watched), Some(&2));
        assert_eq!(
            cache.level_flip_counts.get(&watched).copied().unwrap_or(0),
            0
        );
    }

    #[test]
    fn far_terrain_lod_replaces_levels_without_a_blank_frame() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled();
        let pos = ChunkPos::new(5, 0);

        cache
            .advance_for_camera(
                config,
                12345,
                ChunkPos::new(0, 0),
                0,
                None,
                &mut compiler,
                0,
            )
            .unwrap();
        let old = cache.desired_tiles[&pos];
        assert_eq!(old.level, 2);
        cache.insert_resident_tile(old, FarTerrainLodTileMetadata::default());
        cache.uploaded_tiles.insert(old);
        cache.visible_tiles_by_chunk.insert(pos, old);

        cache
            .advance_for_camera(
                config,
                12345,
                ChunkPos::new(3, 0),
                0,
                None,
                &mut compiler,
                0,
            )
            .unwrap();
        let replacement = cache.desired_tiles[&pos];
        assert_eq!(replacement.level, 1);
        assert_eq!(cache.visible_tiles_by_chunk[&pos], old);

        let request = FarTerrainLodBuildRequest::new_for_tile(
            replacement,
            cache.source_key.unwrap(),
            cache.neighbor_sample_spacings(pos),
            None,
            MonotonicInstant::ZERO,
        );
        compiler
            .completed
            .push(compile_far_terrain_lod_request(request));
        cache
            .inflight_builds
            .insert(replacement, MonotonicInstant::ZERO);
        cache.accept_completed(&mut compiler).unwrap();
        assert_eq!(cache.double_resident_tiles, 1);
        assert_eq!(cache.max_double_resident_tiles, 1);
        assert_eq!(cache.visible_tiles_by_chunk[&pos], old);

        cache.drain_render_uploads(1, &mut compiler);
        let visible = BTreeSet::from([pos]);
        let frame = cache.prepare_render_update(&visible);
        assert!(frame.visible_tiles.contains(&replacement));
        assert!(!frame.visible_tiles.contains(&old));
        assert_eq!(cache.visible_tiles_by_chunk[&pos], replacement);
        assert_eq!(cache.stats().double_resident_tiles, 1);
        assert!(
            cache.stats().max_double_resident_tiles <= MAX_FAR_TERRAIN_LOD_DOUBLE_RESIDENT_TILES
        );

        cache.drain_render_uploads(1, &mut compiler);
        assert!(!cache.resident_tiles.contains_tile(old));
        assert_eq!(cache.stats().double_resident_tiles, 0);
    }

    #[test]
    fn saturated_replacements_do_not_block_fresh_coverage_builds() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled();
        let source_key = FarTerrainLodSourceKey::new(12345, config, false);
        cache.source_key = Some(source_key);

        for x in 0..MAX_FAR_TERRAIN_LOD_DOUBLE_RESIDENT_TILES as i32 {
            let pos = ChunkPos::new(x, 0);
            let desired = LodTileKey::new(pos, 2);
            cache.desired_tiles.insert(pos, desired);
            cache
                .visible_tiles_by_chunk
                .insert(pos, LodTileKey::new(pos, 1));
            cache.insert_resident_tile(desired, FarTerrainLodTileMetadata::default());
        }

        let replacement = LodTileKey::new(ChunkPos::new(0, 0), 2);
        cache.dirty_tiles.insert(replacement);
        let fresh = LodTileKey::new(ChunkPos::new(1000, 0), 1);
        cache.desired_tiles.insert(fresh.chunk, fresh);
        cache
            .pending_builds
            .push_back(FarTerrainLodBuildRequest::new_for_tile(
                replacement,
                source_key,
                [4; 4],
                None,
                MonotonicInstant::ZERO,
            ));
        cache
            .pending_builds
            .push_back(FarTerrainLodBuildRequest::new_for_tile(
                fresh,
                source_key,
                [4; 4],
                None,
                MonotonicInstant::ZERO,
            ));

        cache.submit_builds(&mut compiler, 1).unwrap();

        assert_eq!(compiler.submitted, vec![fresh]);
        assert_eq!(cache.pending_builds.len(), 1);
        assert_eq!(
            cache.pending_builds.front().map(|request| request.key),
            Some(replacement)
        );
    }

    #[test]
    fn far_terrain_lod_cache_exposes_all_three_levels() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(3);

        advance_until_complete(&mut cache, &mut compiler, config, ChunkPos::new(0, 0), 128);
        let frame = upload_all_desired(&mut cache, &mut compiler);
        let stats = cache.stats();
        assert!(stats.resident_tiles_by_level.iter().all(|count| *count > 0));
        assert!(stats.visible_tiles_by_level.iter().all(|count| *count > 0));
        assert!(frame.visible_tiles.iter().any(|tile| tile.level == 1));
        assert!(frame.visible_tiles.iter().any(|tile| tile.level == 2));
        assert!(frame.visible_tiles.iter().any(|tile| tile.level == 3));
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
        let key = FarTerrainLodBuildKey::new(12345, ChunkPos::new(0, 0), 0, config);

        let mesh = build_far_terrain_lod_tile(key, ChunkPos::new(1, 0));

        assert!(mesh.vertex_count() > 0);
        assert!(mesh.index_count() > 0);
        assert_eq!(mesh.index_count() % 3, 0);
        assert_eq!(mesh.key(), LodTileKey::synthetic(ChunkPos::new(1, 0)));
        assert!(mesh.vertex_count() <= 512);
        assert!(mesh.index_count() <= 768);
    }

    #[test]
    fn overworld_far_lod_payload_baseline_is_pinned() {
        let source = FarTerrainLodSourceKey::new_with_profile(
            12_345,
            WorldGenerationProfile::Overworld,
            FarTerrainLodConfig::enabled(),
            false,
        );
        let mesh = compile_far_terrain_lod_request(FarTerrainLodBuildRequest::new_for_tile(
            LodTileKey::new(ChunkPos::new(1, 0), 1),
            source,
            [4; 4],
            None,
            MonotonicInstant::ZERO,
        ))
        .mesh;
        assert_eq!(mesh.vertex_count(), 100);
        assert_eq!(mesh.index_count(), 150);
        assert_eq!(far_lod_mesh_fingerprint(&mesh), 0x5c1c_22da_08c4_89e6);
    }

    #[test]
    fn mclone_far_lod_uses_summary_and_level_rank_admission() {
        let seed = 12_345;
        let source = McloneVegetationSource::new(seed, McloneOverworldSamplingTopology::Unbounded);
        let mut discovery = McloneOverworldVegetationPlanCache::new(source);
        let occurrences = discovery
            .tree_records_intersecting(McloneVegetationBounds::new(-512, -512, 511, 511).unwrap())
            .unwrap();
        let any = occurrences.first().copied().expect("review area has trees");
        let ranked = occurrences
            .iter()
            .copied()
            .find(|occurrence| occurrence.record.landmark_rank >= 2)
            .expect("review area has a rank-two-or-higher tree");
        let compile =
            |occurrence: McloneTreeOccurrence, level: u8, cache: &mut FarTerrainLodWorkerCache| {
                let base = occurrence.working_base().unwrap();
                let pos = ChunkPos::from_block_coords(base.x, base.z);
                let spacing = far_lod_sample_spacing_for_level(4, level);
                let key = FarTerrainLodBuildKey {
                    seed,
                    generation_profile: WorldGenerationProfile::McloneOverworldV1,
                    center: pos,
                    render_distance: 0,
                    start_margin_chunks: 0,
                    extra_radius_chunks: 1,
                    sample_spacing_blocks: spacing,
                    detail_mode: FarLodDetailMode::Auto,
                };
                let mut vertices = Vec::new();
                let mut indices = Vec::new();
                let report = append_far_terrain_lod_chunk_patch(
                    &mut vertices,
                    &mut indices,
                    cache,
                    seed,
                    pos,
                    level,
                    key,
                    [spacing; 4],
                    FarTerrainNormalCoverage::NoNormalChunks,
                    None,
                );
                (report, vertices, indices)
            };

        let mut cache = FarTerrainLodWorkerCache::default();
        let any_base = any.working_base().unwrap();
        let summary_sample = surface_sample_world(
            &mut cache,
            seed,
            WorldGenerationProfile::McloneOverworldV1,
            any_base.x,
            any_base.z,
            None,
        )
        .expect("tree base has a Far LOD surface sample");
        let any_chunk = ChunkPos::from_block_coords(any_base.x, any_base.z);
        let local_x = any_base.x - chunk_min_block_coord(any_chunk.x);
        let local_z = any_base.z - chunk_min_block_coord(any_chunk.z);
        let column = cache.chunks[&any_chunk][(local_z * CHUNK_WIDTH + local_x) as usize]
            .expect("tree base surface column exists");
        let untinted = surface_colors(column.block, column.y, None);
        assert!(summary_sample.forest_summary_available);
        assert_ne!(summary_sample.top_color, untinted.top);

        let (level_one, level_one_vertices, level_one_indices) = compile(any, 1, &mut cache);
        assert!(level_one.summary_cells > 0);
        assert_eq!(level_one.record_queries, 1);
        assert!(level_one.admitted_occurrences > 0);
        assert!(level_one.proxy_boxes > 0);
        assert_eq!(level_one.proxy_vertices, level_one.proxy_boxes * 8);
        assert_eq!(level_one.proxy_indices, level_one.proxy_boxes * 36);
        assert!(level_one_vertices.len() / 7 > level_one.proxy_vertices);
        assert!(level_one_indices.len() > level_one.proxy_indices);

        let (level_two, _, _) = compile(ranked, 2, &mut cache);
        assert!(level_two.summary_cells > 0);
        assert_eq!(level_two.record_queries, 1);
        assert!(level_two.admitted_occurrences > 0);

        let record_requests_before = cache
            .mclone_vegetation
            .as_ref()
            .unwrap()
            .report()
            .cell_requests;
        let (level_three, _, _) = compile(any, 3, &mut cache);
        let record_requests_after = cache
            .mclone_vegetation
            .as_ref()
            .unwrap()
            .report()
            .cell_requests;
        assert!(level_three.summary_cells > 0);
        assert_eq!(level_three.record_queries, 0);
        assert_eq!(level_three.admitted_occurrences, 0);
        assert_eq!(level_three.proxy_boxes, 0);
        assert_eq!(record_requests_after, record_requests_before);
    }

    #[test]
    fn far_lod_tree_record_admission_is_global_and_monotonic() {
        for rank in 0..=3 {
            assert!(far_lod_tree_record_admitted(1, rank));
            assert_eq!(far_lod_tree_record_admitted(2, rank), rank >= 2);
            assert!(!far_lod_tree_record_admitted(3, rank));
        }
    }

    #[test]
    fn cross_chunk_tree_proxy_fragments_are_clipped_to_each_tile() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let occurrence = McloneOverworldVegetationPlanCache::new(source)
            .tree_records_intersecting(McloneVegetationBounds::new(-512, -512, 511, 511).unwrap())
            .unwrap()
            .into_iter()
            .find(|occurrence| {
                let bounds = occurrence.working_bounds;
                bounds.min_x.div_euclid(CHUNK_WIDTH) != bounds.max_x.div_euclid(CHUNK_WIDTH)
                    || bounds.min_z.div_euclid(CHUNK_WIDTH) != bounds.max_z.div_euclid(CHUNK_WIDTH)
            })
            .expect("review area has a cross-chunk tree");
        let bounds = occurrence.working_bounds;
        let (first, second, boundary, x_axis) =
            if bounds.min_x.div_euclid(CHUNK_WIDTH) != bounds.max_x.div_euclid(CHUNK_WIDTH) {
                let first = ChunkPos::new(
                    bounds.min_x.div_euclid(CHUNK_WIDTH),
                    occurrence.working_base().unwrap().z.div_euclid(CHUNK_WIDTH),
                );
                (
                    first,
                    ChunkPos::new(first.x + 1, first.z),
                    chunk_min_block_coord(first.x + 1) as f32,
                    true,
                )
            } else {
                let first = ChunkPos::new(
                    occurrence.working_base().unwrap().x.div_euclid(CHUNK_WIDTH),
                    bounds.min_z.div_euclid(CHUNK_WIDTH),
                );
                (
                    first,
                    ChunkPos::new(first.x, first.z + 1),
                    chunk_min_block_coord(first.z + 1) as f32,
                    false,
                )
            };
        let compile_fragment = |tile| {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let boxes =
                append_far_lod_tree_proxy(&mut vertices, &mut indices, occurrence, tile, None);
            (boxes, vertices, indices)
        };
        let (first_boxes, first_vertices, first_indices) = compile_fragment(first);
        let (second_boxes, second_vertices, second_indices) = compile_fragment(second);

        assert!(first_boxes > 0 && second_boxes > 0);
        assert_eq!(first_vertices.len() / 7, first_boxes * 8);
        assert_eq!(second_vertices.len() / 7, second_boxes * 8);
        assert_eq!(first_indices.len(), first_boxes * 36);
        assert_eq!(second_indices.len(), second_boxes * 36);
        for (tile, vertices) in [
            (first, first_vertices.as_slice()),
            (second, second_vertices.as_slice()),
        ] {
            let min_x = chunk_min_block_coord(tile.x) as f32;
            let min_z = chunk_min_block_coord(tile.z) as f32;
            for vertex in vertices.chunks_exact(7) {
                assert!((min_x..=min_x + CHUNK_WIDTH as f32).contains(&vertex[0]));
                assert!((min_z..=min_z + CHUNK_WIDTH as f32).contains(&vertex[2]));
            }
        }
        let axis_offset = if x_axis { 0 } else { 2 };
        assert!(
            first_vertices
                .chunks_exact(7)
                .any(|vertex| vertex[axis_offset] == boundary)
        );
        assert!(
            second_vertices
                .chunks_exact(7)
                .any(|vertex| vertex[axis_offset] == boundary)
        );
    }

    #[test]
    fn far_lod_worker_cache_reuses_neighbor_surface_facts() {
        let source = FarTerrainLodSourceKey::new(12345, FarTerrainLodConfig::enabled(), false);
        let mut cache = FarTerrainLodWorkerCache::default();
        let first = FarTerrainLodBuildRequest::new_for_tile(
            LodTileKey::new(ChunkPos::new(0, 0), 1),
            source,
            [4; 4],
            None,
            MonotonicInstant::ZERO,
        );
        compile_far_terrain_lod_request_cached(first, &mut cache);
        let first_chunks = cache.chunks.len();

        let adjacent = FarTerrainLodBuildRequest::new_for_tile(
            LodTileKey::new(ChunkPos::new(1, 0), 1),
            source,
            [4; 4],
            None,
            MonotonicInstant::ZERO,
        );
        compile_far_terrain_lod_request_cached(adjacent, &mut cache);

        assert!(first_chunks >= 5);
        assert!(cache.chunks.len() <= first_chunks + 3);
    }

    #[test]
    fn far_lod_worker_uses_mclone_profile_and_cached_valley_stream_plan() {
        let seed = -98_765;
        let planner = mclone_worldgen::levelgen::McloneOverworldStreamPlanner::new(
            seed,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let candidate = planner
            .potential_start(ChunkPos::new(147, -126))
            .expect("stream placement");
        let plan = planner
            .plan_start(candidate)
            .expect("stream plan")
            .expect("reviewed stream start");
        let mut mclone_cache = FarTerrainLodWorkerCache::default();
        let mut overworld_cache = FarTerrainLodWorkerCache::default();
        let mut direct_chunks = BTreeMap::new();
        let mut observed_stream_water = false;
        let mut differs_from_vanilla = false;

        for node in &plan.nodes {
            let pos = ChunkPos::from_block_coords(node.x, node.z);
            let chunk = direct_chunks
                .entry(pos)
                .or_insert_with(|| generate_mclone_overworld_surface_chunk(seed, pos.x, pos.z));
            let local_x = node.x - chunk_min_block_coord(pos.x);
            let local_z = node.z - chunk_min_block_coord(pos.z);
            let direct = surface_column(chunk, local_x, local_z)
                .expect("planned stream centerline has a surface");
            observed_stream_water |= is_water(direct.block.raw());

            let sampled = surface_sample_world(
                &mut mclone_cache,
                seed,
                WorldGenerationProfile::McloneOverworldV1,
                node.x,
                node.z,
                None,
            )
            .expect("Mclone far LOD sample exists");
            assert_eq!(sampled.y, direct.y as f32);

            let vanilla = surface_sample_world(
                &mut overworld_cache,
                seed,
                WorldGenerationProfile::Overworld,
                node.x,
                node.z,
                None,
            )
            .expect("vanilla far LOD sample exists");
            differs_from_vanilla |= sampled != vanilla;
        }

        assert!(observed_stream_water);
        assert!(differs_from_vanilla);
        let report = mclone_cache
            .mclone_stream_plans
            .as_ref()
            .expect("Mclone LOD owns a stream plan cache")
            .report();
        assert!(report.requests > 0);
        assert!(report.hits > 0);
        assert_eq!(
            mclone_cache.source,
            Some((seed, WorldGenerationProfile::McloneOverworldV1))
        );
        assert_eq!(
            mclone_cache
                .mclone_vegetation
                .as_ref()
                .expect("Mclone LOD owns a vegetation cache")
                .source(),
            McloneVegetationSource::new(seed, McloneOverworldSamplingTopology::Unbounded)
        );

        surface_sample_world(
            &mut mclone_cache,
            seed + 1,
            WorldGenerationProfile::Overworld,
            0,
            0,
            None,
        )
        .expect("replacement source sample exists");
        assert_eq!(
            mclone_cache.source,
            Some((seed + 1, WorldGenerationProfile::Overworld))
        );
        assert!(mclone_cache.mclone_stream_plans.is_none());
        assert!(mclone_cache.mclone_vegetation.is_none());
    }

    #[test]
    fn far_lod_worker_preserves_mclone_alpine_snow_surface() {
        let seed = -98_765;
        let sampler = mclone_worldgen::levelgen::McloneOverworldSampler::new(seed);
        let (world_x, world_z) = 'site: {
            for world_z in (-2_048..2_048).step_by(16) {
                for world_x in (-2_048..2_048).step_by(16) {
                    let landform = sampler.sample_landform(world_x, world_z);
                    if mclone_worldgen::levelgen::mclone_overworld_surface_recipe(landform)
                        == mclone_worldgen::levelgen::McloneOverworldSurfaceRecipe::AlpineSnow
                    {
                        break 'site (world_x, world_z);
                    }
                }
            }
            panic!("review seed should contain an alpine snow surface");
        };
        let pos = ChunkPos::from_block_coords(world_x, world_z);
        let direct_chunk = generate_mclone_overworld_surface_chunk(seed, pos.x, pos.z);
        let direct = surface_column(
            &direct_chunk,
            world_x - pos.min_block_x(),
            world_z - pos.min_block_z(),
        )
        .expect("alpine snow column has a surface");
        assert_eq!(direct.block.raw(), SNOW);

        let mut cache = FarTerrainLodWorkerCache::default();
        let sampled = surface_sample_world(
            &mut cache,
            seed,
            WorldGenerationProfile::McloneOverworldV1,
            world_x,
            world_z,
            None,
        )
        .expect("Mclone alpine far LOD sample exists");
        assert_eq!(sampled.y, direct.y as f32);
        assert_eq!(
            cache.source,
            Some((seed, WorldGenerationProfile::McloneOverworldV1))
        );
    }

    #[test]
    fn far_terrain_lod_chunk_patch_emits_flat_blocky_cell_caps() {
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let key = FarTerrainLodBuildKey::new(12345, ChunkPos::new(0, 0), 0, config);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut chunks = BTreeMap::new();

        append_far_terrain_lod_chunk_patch(
            &mut vertices,
            &mut indices,
            &mut chunks,
            12345,
            ChunkPos::new(1, 0),
            1,
            key,
            [DEFAULT_FAR_TERRAIN_LOD_SAMPLE_SPACING_BLOCKS; 4],
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
                forest_summary_available: false,
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
    fn far_terrain_lod_uses_stable_shell_and_bounded_guards() {
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let center = ChunkPos::new(0, 0);
        let key = FarTerrainLodBuildKey::new(12345, center, 2, config);
        let positions = far_lod_chunk_positions(key);
        let desired = positions.iter().copied().collect::<BTreeSet<_>>();
        let guards = far_lod_prefetch_chunk_positions(key, &desired, usize::MAX);

        assert_eq!(positions.len(), 40);
        assert!(!positions.contains(&center));
        assert!(!positions.contains(&ChunkPos::new(1, 0)));
        assert!(positions.contains(&ChunkPos::new(2, 0)));
        assert!(positions.contains(&ChunkPos::new(3, 0)));
        assert_eq!(guards.len(), 80);
        assert!(!guards.contains(&center));
        assert!(guards.contains(&ChunkPos::new(1, 0)));
        assert!(guards.contains(&ChunkPos::new(4, 0)));
        assert!(guards.contains(&ChunkPos::new(5, 0)));
        assert!(!guards.contains(&ChunkPos::new(6, 0)));
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

        let _sample = surface_sample_world(
            &mut chunks,
            12345,
            WorldGenerationProfile::Overworld,
            world_x,
            0,
            None,
        )
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
                &mut compiler,
                3,
            )
            .unwrap();
        let queued = cache.settle_snapshot();
        assert_eq!(queued.inflight_builds.len(), 3);
        assert_eq!(
            queued.pending_builds.len() + queued.inflight_builds.len(),
            queued.desired_tiles.len() + queued.prefetch_tiles.len()
        );

        advance_until_complete(&mut cache, &mut compiler, config, ChunkPos::new(0, 0), 3);
        let frame = upload_all_desired(&mut cache, &mut compiler);
        assert!(frame.uploads.len() >= cache.desired_tiles.len());
        assert_eq!(frame.visible_tiles.len(), cache.desired_tiles.len());
        let stable_revision = frame.revision;
        let visible = cache.desired_tiles.keys().copied().collect();
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
        assert!(first_ready.len() > 8);
        cache
            .advance_for_camera(config, 12345, moved_center, 0, None, &mut compiler, 0)
            .unwrap();

        let moved_key = FarTerrainLodBuildKey::new(12345, moved_center, 0, config);
        let moved_desired = far_lod_chunk_positions(moved_key)
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
                    .tile_keys()
                    .any(|resident| resident.chunk == pos)
            );
        }
        assert!(!cache.desired_tiles.is_empty());
    }

    #[test]
    fn movement_guard_prefetch_promotes_without_a_rebuild() {
        let mut cache = FarTerrainLodCache::new();
        let mut compiler = ImmediateFarLodCompiler::default();
        let config = FarTerrainLodConfig::enabled().with_extra_radius_chunks(1);
        let guarded = ChunkPos::new(2, 0);

        for _ in 0..8 {
            cache
                .advance_for_camera(
                    config,
                    12345,
                    ChunkPos::new(0, 0),
                    0,
                    None,
                    &mut compiler,
                    64,
                )
                .unwrap();
            if cache.pending_builds.is_empty() && cache.inflight_builds.is_empty() {
                break;
            }
        }
        cache.drain_render_uploads(MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES, &mut compiler);
        let guarded_tile = cache.prefetch_tiles[&guarded];
        assert!(cache.uploaded_tiles.contains(&guarded_tile));
        assert!(!cache.drawable_lod_tiles().any(|pos| pos == guarded));

        cache
            .advance_for_camera(
                config,
                12345,
                ChunkPos::new(1, 0),
                0,
                None,
                &mut compiler,
                0,
            )
            .unwrap();

        assert_eq!(cache.desired_tiles[&guarded], guarded_tile);
        assert_eq!(cache.visible_tiles_by_chunk[&guarded], guarded_tile);
        assert!(cache.drawable_lod_tiles().any(|pos| pos == guarded));
        // A seam refresh may be queued after neighbors change level, but the
        // prefetched tile is already promoted and covers the chunk meanwhile.
    }

    #[test]
    fn movement_guard_prioritizes_the_leading_edge() {
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
                &mut compiler,
                0,
            )
            .unwrap();
        cache
            .advance_for_camera(
                config,
                12345,
                ChunkPos::new(1, 0),
                0,
                None,
                &mut compiler,
                0,
            )
            .unwrap();

        let key = cache.view_key.unwrap();
        let leading = cache.prefetch_tiles[&ChunkPos::new(4, 0)];
        let side = cache.prefetch_tiles[&ChunkPos::new(1, 3)];
        let trailing = cache.prefetch_tiles[&ChunkPos::new(-2, 0)];
        assert!(cache.build_priority(key, leading) < cache.build_priority(key, side));
        assert!(cache.build_priority(key, side) < cache.build_priority(key, trailing));
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
                &mut compiler,
                0,
            )
            .unwrap();

        assert_eq!(
            cache.desired_tiles.len(),
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
            3,
            &mut compiler,
        );
        assert_eq!(first.target_tiles, 9);
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
                3,
                &mut compiler,
            );
        }
        assert!(coverage.is_complete());
        assert_eq!(coverage.ready_tiles, 9);
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
        assert!(prewarmed.len() > 8);

        // The first live frame must reuse the prewarmed patches, not reset them:
        // prewarm and live share the same source key (seed + spacing + materials).
        cache
            .advance_for_camera(live, 12345, ChunkPos::new(0, 0), 0, None, &mut compiler, 0)
            .unwrap();
        for pos in &prewarmed {
            assert!(
                cache
                    .resident_tiles
                    .tile_keys()
                    .any(|resident| resident.chunk == *pos),
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
            .advance_for_camera(config, 12345, far_center, 0, None, &mut compiler, 0)
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
