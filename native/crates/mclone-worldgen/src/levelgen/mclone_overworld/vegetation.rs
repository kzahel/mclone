use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;

use mclone_core::{ChunkPos, block_to_chunk_coord};

use crate::block::{
    ACACIA_LEAVES, ACACIA_LOG, AIR, DIRT, JUNGLE_LEAVES, JUNGLE_LOG, OAK_LEAVES, OAK_LOG,
    RawBlockId, SPRUCE_LEAVES, SPRUCE_LOG, is_leaves,
};
use crate::feature::FeatureRegion;
use crate::levelgen::profile::FLAT_GRASS_HEIGHT;
use crate::noise::{SeedDomain, ValueNoise2d};
use crate::placement::BlockPos;

use super::biomes::{
    McloneOverworldBiomeRecipe, McloneOverworldSteppeBand, mclone_overworld_biome_recipe,
    mclone_overworld_steppe_band,
};
use super::fields::{
    MCLONE_OVERWORLD_PERIOD_BLOCKS, MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS,
    McloneOverworldLandformSample, McloneOverworldSamplingTopology,
};
use super::streams::{McloneOverworldStreamPlan, McloneOverworldStreamPlanCache};
use super::surface::{McloneOverworldSurfaceRecipe, mclone_overworld_surface_recipe};
use super::terrain::{
    apply_stream_plans, sample_mclone_overworld_landform_with_sampler_and_stream_cache,
};

pub const MCLONE_OVERWORLD_VEGETATION_REVISION: &str = "mclone-overworld-v1-vegetation-2";
pub const MCLONE_VEGETATION_PLANNING_CELL_BLOCKS: i32 = 32;
pub const MCLONE_VEGETATION_CANDIDATES_PER_CELL: u8 = 32;
pub const MCLONE_OVERWORLD_GROVE_DOMAIN: u64 = 0x6d63_6f76_6772_6f76;
pub const MCLONE_OVERWORLD_GROVE_SCALE_BLOCKS: i32 = 256;
pub const MCLONE_FOREST_SUMMARY_FOOTPRINT_TAPS: usize = 4;
pub const MCLONE_FOREST_EDGE_SAMPLE_RADIUS_BLOCKS: i32 = 64;

const VEGETATION_PLAN_REVISION: u16 = 2;
const GROVE_DOMAIN: SeedDomain = SeedDomain::new(MCLONE_OVERWORLD_GROVE_DOMAIN);
const MAX_CONFLICT_DISTANCE_BLOCKS: i32 = 7;
const MAX_TREE_HORIZONTAL_OVERHANG: i32 = 6;
const CONFLICT_CELL_HALO: i32 =
    (MAX_CONFLICT_DISTANCE_BLOCKS + MCLONE_VEGETATION_PLANNING_CELL_BLOCKS - 1)
        / MCLONE_VEGETATION_PLANNING_CELL_BLOCKS;
const MAX_QUERY_CELLS: i64 = 65_536;
const MAX_CACHED_CELLS: usize = 4_096;

const HASH_LANE_POSITION_X: u64 = 0x706f_735f_785f_3031;
const HASH_LANE_POSITION_Z: u64 = 0x706f_735f_7a5f_3031;
const HASH_LANE_DENSITY: u64 = 0x6465_6e73_6974_7931;
const HASH_LANE_PRIORITY: u64 = 0x7072_696f_7269_7479;
const HASH_LANE_SILHOUETTE: u64 = 0x7369_6c68_6f75_6574;
const HASH_LANE_ORIENTATION: u64 = 0x6f72_6965_6e74_3031;
const HASH_LANE_VARIANT: u64 = 0x7661_7269_616e_7431;
const HASH_LANE_LANDMARK: u64 = 0x6c61_6e64_6d61_726b;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneVegetationSource {
    pub seed: i64,
    pub topology: McloneOverworldSamplingTopology,
}

impl McloneVegetationSource {
    pub const fn new(seed: i64, topology: McloneOverworldSamplingTopology) -> Self {
        Self { seed, topology }
    }

    pub const fn revision(self) -> &'static str {
        MCLONE_OVERWORLD_VEGETATION_REVISION
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneTreeFamily {
    TemperateBroadleaf,
    CoolWetConifer,
    WarmDryAcacia,
    HumidJungleBroadleaf,
}

impl McloneTreeFamily {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TemperateBroadleaf => "temperateBroadleaf",
            Self::CoolWetConifer => "coolWetConifer",
            Self::WarmDryAcacia => "warmDryAcacia",
            Self::HumidJungleBroadleaf => "humidJungleBroadleaf",
        }
    }

    const fn minimum_spacing(self) -> i32 {
        match self {
            Self::TemperateBroadleaf => 5,
            Self::CoolWetConifer => 4,
            Self::WarmDryAcacia => 7,
            Self::HumidJungleBroadleaf => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneTreeArchetype {
    RoundedBroadleaf,
    LayeredConifer,
    ForkedAcacia,
    LayeredJungle,
}

impl McloneTreeArchetype {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RoundedBroadleaf => "roundedBroadleaf",
            Self::LayeredConifer => "layeredConifer",
            Self::ForkedAcacia => "forkedAcacia",
            Self::LayeredJungle => "layeredJungle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneForestIntentSample {
    pub coverage: f32,
    pub density: f32,
    pub dominant_family: Option<McloneTreeFamily>,
    pub secondary_family: Option<McloneTreeFamily>,
    pub family_mix: f32,
    pub mean_canopy_height: f32,
    pub canopy_height_variation: f32,
    pub grove_or_opening_influence: f32,
}

impl McloneForestIntentSample {
    pub const EMPTY: Self = Self {
        coverage: 0.0,
        density: 0.0,
        dominant_family: None,
        secondary_family: None,
        family_mix: 0.0,
        mean_canopy_height: 0.0,
        canopy_height_variation: 0.0,
        grove_or_opening_influence: 0.0,
    };

    fn density_threshold(self) -> u16 {
        (self.density.clamp(0.0, 1.0) * f32::from(u16::MAX)).round() as u16
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneForestDirection {
    pub x: i8,
    pub z: i8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct McloneForestEdgeIntentSample {
    pub local: McloneForestIntentSample,
    pub nearby_min_coverage: f32,
    pub nearby_max_coverage: f32,
    pub edge_contrast: f32,
    pub clearing_direction: McloneForestDirection,
    pub cover_direction: McloneForestDirection,
}

impl McloneForestEdgeIntentSample {
    pub fn is_transitional_edge(self) -> bool {
        self.local.coverage >= 0.10
            && self.local.coverage <= 0.86
            && self.nearby_min_coverage <= 0.36
            && self.nearby_max_coverage >= 0.48
            && self.edge_contrast >= 0.24
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneTreeId {
    pub planning_cell_x: i32,
    pub planning_cell_z: i32,
    pub candidate_slot: u8,
    pub vegetation_revision: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneTreeBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}

impl McloneTreeBounds {
    pub fn new(
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> Result<Self, McloneVegetationError> {
        if min_x > max_x || min_y > max_y || min_z > max_z {
            return Err(McloneVegetationError::InvalidBounds);
        }
        Ok(Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        })
    }

    pub const fn intersects_horizontal(self, bounds: McloneVegetationBounds) -> bool {
        self.max_x >= bounds.min_x
            && self.min_x <= bounds.max_x
            && self.max_z >= bounds.min_z
            && self.min_z <= bounds.max_z
    }

    fn shifted_x(self, offset: i64) -> Result<Self, McloneVegetationError> {
        Ok(Self {
            min_x: shift_coordinate(self.min_x, offset)?,
            min_y: self.min_y,
            min_z: self.min_z,
            max_x: shift_coordinate(self.max_x, offset)?,
            max_y: self.max_y,
            max_z: self.max_z,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneTreeRecord {
    pub id: McloneTreeId,
    pub canonical_base: BlockPos,
    pub family: McloneTreeFamily,
    pub archetype: McloneTreeArchetype,
    pub trunk_height: u16,
    pub crown_radius: u16,
    pub crown_depth: u16,
    pub orientation: u8,
    pub landmark_rank: u8,
    pub variant_seed: u64,
    pub bounds: McloneTreeBounds,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneTreeOccurrence {
    pub record: McloneTreeRecord,
    pub x_lift: i64,
    pub working_bounds: McloneTreeBounds,
}

impl McloneTreeOccurrence {
    pub fn working_base(self) -> Result<BlockPos, McloneVegetationError> {
        Ok(BlockPos::new(
            shift_coordinate(self.record.canonical_base.x, self.x_lift)?,
            self.record.canonical_base.y,
            self.record.canonical_base.z,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneVegetationBounds {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

impl McloneVegetationBounds {
    pub fn new(
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    ) -> Result<Self, McloneVegetationError> {
        if min_x > max_x || min_z > max_z {
            return Err(McloneVegetationError::InvalidBounds);
        }
        Ok(Self {
            min_x,
            min_z,
            max_x,
            max_z,
        })
    }

    fn expanded(self, amount: i32) -> Result<Self, McloneVegetationError> {
        Self::new(
            self.min_x
                .checked_sub(amount)
                .ok_or(McloneVegetationError::CoordinateOverflow)?,
            self.min_z
                .checked_sub(amount)
                .ok_or(McloneVegetationError::CoordinateOverflow)?,
            self.max_x
                .checked_add(amount)
                .ok_or(McloneVegetationError::CoordinateOverflow)?,
            self.max_z
                .checked_add(amount)
                .ok_or(McloneVegetationError::CoordinateOverflow)?,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McloneVegetationPlanCacheReport {
    pub cell_requests: u64,
    pub cell_hits: u64,
    pub cell_misses: u64,
    pub retained_cells: usize,
    pub retained_preliminary_candidates: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McloneTreeRealizationReport {
    pub requested_occurrences: usize,
    pub realized_occurrences: usize,
    pub block_write_attempts: usize,
    pub block_writes: usize,
    pub clipped_block_writes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McloneVegetationError {
    InvalidBounds,
    CoordinateOverflow,
    QueryTooLarge { cells: i64, maximum: i64 },
    StructuredTerrain(String),
}

impl fmt::Display for McloneVegetationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBounds => write!(formatter, "vegetation bounds are invalid"),
            Self::CoordinateOverflow => write!(formatter, "vegetation coordinate overflow"),
            Self::QueryTooLarge { cells, maximum } => write!(
                formatter,
                "vegetation query requires {cells} planning cells; maximum is {maximum}"
            ),
            Self::StructuredTerrain(error) => {
                write!(formatter, "failed to sample structured terrain: {error}")
            }
        }
    }
}

impl Error for McloneVegetationError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct PlanningCell {
    x: i32,
    z: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreliminaryCandidate {
    record: McloneTreeRecord,
    conflict_priority: u64,
    minimum_spacing: i32,
}

#[derive(Clone, Debug)]
pub struct McloneOverworldVegetationPlanner {
    source: McloneVegetationSource,
    grove_field: ValueNoise2d,
    terrain_sampler: super::fields::McloneOverworldSampler,
}

impl McloneOverworldVegetationPlanner {
    pub fn new(source: McloneVegetationSource) -> Self {
        let grove_field = match source.topology {
            McloneOverworldSamplingTopology::Unbounded => ValueNoise2d::new(
                source.seed,
                GROVE_DOMAIN,
                MCLONE_OVERWORLD_GROVE_SCALE_BLOCKS,
            ),
            McloneOverworldSamplingTopology::PeriodicX => ValueNoise2d::new_periodic_x(
                source.seed,
                GROVE_DOMAIN,
                MCLONE_OVERWORLD_GROVE_SCALE_BLOCKS,
                MCLONE_OVERWORLD_PERIOD_BLOCKS,
            ),
        };
        Self {
            source,
            grove_field,
            terrain_sampler: super::fields::McloneOverworldSampler::new_with_topology(
                source.seed,
                source.topology,
            ),
        }
    }

    pub const fn source(&self) -> McloneVegetationSource {
        self.source
    }

    pub fn forest_intent(
        &self,
        landform: McloneOverworldLandformSample,
        world_x: i32,
        world_z: i32,
    ) -> McloneForestIntentSample {
        forest_intent_from_landform(landform, self.grove_field.sample(world_x, world_z) as f32)
    }

    pub fn forest_footprint_summary(
        &self,
        taps: [(McloneOverworldLandformSample, i32, i32); MCLONE_FOREST_SUMMARY_FOOTPRINT_TAPS],
    ) -> McloneForestIntentSample {
        forest_footprint_summary(
            taps.map(|(landform, world_x, world_z)| self.forest_intent(landform, world_x, world_z)),
        )
    }

    fn preliminary_candidates(
        &self,
        cell: PlanningCell,
        stream_cache: &mut McloneOverworldStreamPlanCache,
    ) -> Result<Vec<PreliminaryCandidate>, McloneVegetationError> {
        let origin_x = cell
            .x
            .checked_mul(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        let origin_z = cell
            .z
            .checked_mul(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        // Any structured stream that can influence a center or cardinal slope
        // sample intersects this one-cell-plus-slope-halo chunk rectangle.
        // Loading the bounded union once is equivalent to point queries because
        // `apply_stream_plans` still tests every plan against each exact point.
        let stream_plans = stream_cache
            .plans_intersecting_chunks(
                ChunkPos::new(
                    block_to_chunk_coord(origin_x)
                        .checked_sub(1)
                        .ok_or(McloneVegetationError::CoordinateOverflow)?,
                    block_to_chunk_coord(origin_z)
                        .checked_sub(1)
                        .ok_or(McloneVegetationError::CoordinateOverflow)?,
                ),
                ChunkPos::new(
                    block_to_chunk_coord(
                        origin_x
                            .checked_add(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS - 1)
                            .ok_or(McloneVegetationError::CoordinateOverflow)?,
                    )
                    .checked_add(1)
                    .ok_or(McloneVegetationError::CoordinateOverflow)?,
                    block_to_chunk_coord(
                        origin_z
                            .checked_add(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS - 1)
                            .ok_or(McloneVegetationError::CoordinateOverflow)?,
                    )
                    .checked_add(1)
                    .ok_or(McloneVegetationError::CoordinateOverflow)?,
                ),
            )
            .map_err(|error| McloneVegetationError::StructuredTerrain(error.to_string()))?;
        let mut candidates = Vec::new();
        let mut terrain_samples = BTreeMap::new();
        for candidate_slot in 0..MCLONE_VEGETATION_CANDIDATES_PER_CELL {
            let local_x = (candidate_hash(self.source, cell, candidate_slot, HASH_LANE_POSITION_X)
                % MCLONE_VEGETATION_PLANNING_CELL_BLOCKS as u64) as i32;
            let local_z = (candidate_hash(self.source, cell, candidate_slot, HASH_LANE_POSITION_Z)
                % MCLONE_VEGETATION_PLANNING_CELL_BLOCKS as u64) as i32;
            let world_x = origin_x
                .checked_add(local_x)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            let world_z = origin_z
                .checked_add(local_z)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            let density_roll =
                candidate_hash(self.source, cell, candidate_slot, HASH_LANE_DENSITY) as u16;
            let center = sample_candidate_terrain_cached(
                &mut terrain_samples,
                self.terrain_sampler,
                world_x,
                world_z,
                &stream_plans,
            );
            if center.watercourse.is_water() || center.watercourse.bank_influence >= 0.82 {
                continue;
            }
            // Slope can only switch an otherwise eligible woodland recipe to
            // meadow, whose tree density is lower. Therefore slope zero is a
            // conservative upper bound: candidates rejected here cannot pass
            // after the exact cardinal slope samples are reconstructed.
            let maximum_intent = self.forest_intent(
                McloneOverworldLandformSample {
                    terrain: center,
                    slope: 0.0,
                },
                world_x,
                world_z,
            );
            if maximum_intent.dominant_family.is_none()
                || density_roll > maximum_intent.density_threshold()
            {
                continue;
            }
            let radius = MCLONE_OVERWORLD_SLOPE_SAMPLE_RADIUS;
            let west_x = world_x
                .checked_sub(radius)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            let east_x = world_x
                .checked_add(radius)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            let north_z = world_z
                .checked_sub(radius)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            let south_z = world_z
                .checked_add(radius)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            let landform = McloneOverworldLandformSample::from_cardinal_samples(
                center,
                sample_candidate_terrain_cached(
                    &mut terrain_samples,
                    self.terrain_sampler,
                    west_x,
                    world_z,
                    &stream_plans,
                ),
                sample_candidate_terrain_cached(
                    &mut terrain_samples,
                    self.terrain_sampler,
                    east_x,
                    world_z,
                    &stream_plans,
                ),
                sample_candidate_terrain_cached(
                    &mut terrain_samples,
                    self.terrain_sampler,
                    world_x,
                    north_z,
                    &stream_plans,
                ),
                sample_candidate_terrain_cached(
                    &mut terrain_samples,
                    self.terrain_sampler,
                    world_x,
                    south_z,
                    &stream_plans,
                ),
            );
            let intent = self.forest_intent(landform, world_x, world_z);
            if intent.dominant_family.is_none() || density_roll > intent.density_threshold() {
                continue;
            }
            let family = intent
                .dominant_family
                .expect("non-empty forest intent has a family");
            if !candidate_supports_landform(family, landform) {
                continue;
            }
            let base_y = landform
                .terrain
                .surface_y
                .checked_add(1)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            let silhouette =
                candidate_hash(self.source, cell, candidate_slot, HASH_LANE_SILHOUETTE);
            let (archetype, trunk_height, crown_radius, crown_depth) =
                resolve_silhouette(family, silhouette);
            let orientation =
                (candidate_hash(self.source, cell, candidate_slot, HASH_LANE_ORIENTATION) & 3)
                    as u8;
            let landmark_rank =
                (candidate_hash(self.source, cell, candidate_slot, HASH_LANE_LANDMARK) >> 62) as u8;
            let id = McloneTreeId {
                planning_cell_x: cell.x,
                planning_cell_z: cell.z,
                candidate_slot,
                vegetation_revision: VEGETATION_PLAN_REVISION,
            };
            let canonical_base = BlockPos::new(world_x, base_y, world_z);
            let bounds = tree_bounds(
                canonical_base,
                family,
                trunk_height,
                crown_radius,
                crown_depth,
            )?;
            if bounds.max_y >= FLAT_GRASS_HEIGHT {
                continue;
            }
            candidates.push(PreliminaryCandidate {
                record: McloneTreeRecord {
                    id,
                    canonical_base,
                    family,
                    archetype,
                    trunk_height,
                    crown_radius,
                    crown_depth,
                    orientation,
                    landmark_rank,
                    variant_seed: candidate_hash(
                        self.source,
                        cell,
                        candidate_slot,
                        HASH_LANE_VARIANT,
                    ),
                    bounds,
                },
                conflict_priority: candidate_hash(
                    self.source,
                    cell,
                    candidate_slot,
                    HASH_LANE_PRIORITY,
                ),
                minimum_spacing: family.minimum_spacing(),
            });
        }
        candidates.sort_by_key(|candidate| candidate.record.id);
        Ok(candidates)
    }
}

fn sample_candidate_terrain(
    sampler: super::fields::McloneOverworldSampler,
    world_x: i32,
    world_z: i32,
    stream_plans: &[McloneOverworldStreamPlan],
) -> super::fields::McloneOverworldTerrainSample {
    let mut sample = sampler.sample(world_x, world_z);
    let _ = apply_stream_plans(&mut sample, world_x, world_z, stream_plans);
    sample
}

fn sample_candidate_terrain_cached(
    cache: &mut BTreeMap<(i32, i32), super::fields::McloneOverworldTerrainSample>,
    sampler: super::fields::McloneOverworldSampler,
    world_x: i32,
    world_z: i32,
    stream_plans: &[McloneOverworldStreamPlan],
) -> super::fields::McloneOverworldTerrainSample {
    if let Some(sample) = cache.get(&(world_x, world_z)) {
        return *sample;
    }
    let sample = sample_candidate_terrain(sampler, world_x, world_z, stream_plans);
    cache.insert((world_x, world_z), sample);
    sample
}

#[derive(Clone, Debug)]
pub struct McloneOverworldVegetationPlanCache {
    planner: McloneOverworldVegetationPlanner,
    stream_cache: McloneOverworldStreamPlanCache,
    cells: BTreeMap<PlanningCell, Vec<PreliminaryCandidate>>,
    insertion_order: VecDeque<PlanningCell>,
    cell_requests: u64,
    cell_hits: u64,
}

impl McloneOverworldVegetationPlanCache {
    pub fn new(source: McloneVegetationSource) -> Self {
        Self {
            planner: McloneOverworldVegetationPlanner::new(source),
            stream_cache: McloneOverworldStreamPlanCache::new(source.seed, source.topology),
            cells: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            cell_requests: 0,
            cell_hits: 0,
        }
    }

    pub const fn source(&self) -> McloneVegetationSource {
        self.planner.source()
    }

    pub fn matches(&self, source: McloneVegetationSource) -> bool {
        self.source() == source
    }

    pub fn clear(&mut self) {
        let source = self.source();
        self.cells.clear();
        self.insertion_order.clear();
        self.stream_cache = McloneOverworldStreamPlanCache::new(source.seed, source.topology);
        self.cell_requests = 0;
        self.cell_hits = 0;
    }

    pub fn report(&self) -> McloneVegetationPlanCacheReport {
        McloneVegetationPlanCacheReport {
            cell_requests: self.cell_requests,
            cell_hits: self.cell_hits,
            cell_misses: self.cell_requests - self.cell_hits,
            retained_cells: self.cells.len(),
            retained_preliminary_candidates: self.cells.values().map(Vec::len).sum(),
        }
    }

    pub fn forest_intent_at(
        &mut self,
        world_x: i32,
        world_z: i32,
    ) -> Result<McloneForestIntentSample, McloneVegetationError> {
        let source = self.source();
        let (landform, _) = sample_mclone_overworld_landform_with_sampler_and_stream_cache(
            source.seed,
            source.topology,
            self.planner.terrain_sampler,
            world_x,
            world_z,
            &mut self.stream_cache,
        )
        .map_err(McloneVegetationError::StructuredTerrain)?;
        Ok(self.planner.forest_intent(landform, world_x, world_z))
    }

    pub fn forest_edge_intent_at(
        &mut self,
        world_x: i32,
        world_z: i32,
    ) -> Result<McloneForestEdgeIntentSample, McloneVegetationError> {
        let local = self.forest_intent_at(world_x, world_z)?;
        let directions = [
            McloneForestDirection { x: 0, z: -1 },
            McloneForestDirection { x: 1, z: 0 },
            McloneForestDirection { x: 0, z: 1 },
            McloneForestDirection { x: -1, z: 0 },
        ];
        let mut samples = [(directions[0], 0.0_f32); 4];
        for (index, direction) in directions.into_iter().enumerate() {
            let sample_x = world_x
                .checked_add(i32::from(direction.x) * MCLONE_FOREST_EDGE_SAMPLE_RADIUS_BLOCKS)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            let sample_z = world_z
                .checked_add(i32::from(direction.z) * MCLONE_FOREST_EDGE_SAMPLE_RADIUS_BLOCKS)
                .ok_or(McloneVegetationError::CoordinateOverflow)?;
            samples[index] = (
                direction,
                self.forest_intent_at(sample_x, sample_z)?.coverage,
            );
        }
        let (clearing_direction, nearby_min_coverage) = samples
            .iter()
            .copied()
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .expect("four forest-edge samples");
        let (cover_direction, nearby_max_coverage) = samples
            .iter()
            .copied()
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .expect("four forest-edge samples");

        Ok(McloneForestEdgeIntentSample {
            local,
            nearby_min_coverage,
            nearby_max_coverage,
            edge_contrast: nearby_max_coverage - nearby_min_coverage,
            clearing_direction,
            cover_direction,
        })
    }

    pub fn tree_records_intersecting(
        &mut self,
        bounds: McloneVegetationBounds,
    ) -> Result<Vec<McloneTreeOccurrence>, McloneVegetationError> {
        let candidate_bounds = bounds.expanded(MAX_TREE_HORIZONTAL_OVERHANG)?;
        let min_cell_x = candidate_bounds
            .min_x
            .div_euclid(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS);
        let max_cell_x = candidate_bounds
            .max_x
            .div_euclid(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS);
        let min_cell_z = candidate_bounds
            .min_z
            .div_euclid(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS);
        let max_cell_z = candidate_bounds
            .max_z
            .div_euclid(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS);
        let load_min_x = min_cell_x
            .checked_sub(CONFLICT_CELL_HALO)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        let load_max_x = max_cell_x
            .checked_add(CONFLICT_CELL_HALO)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        let load_min_z = min_cell_z
            .checked_sub(CONFLICT_CELL_HALO)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        let load_max_z = max_cell_z
            .checked_add(CONFLICT_CELL_HALO)
            .ok_or(McloneVegetationError::CoordinateOverflow)?;
        validate_query_cell_count(load_min_x, load_max_x, load_min_z, load_max_z)?;

        let mut loaded = BTreeMap::new();
        for work_z in load_min_z..=load_max_z {
            for work_x in load_min_x..=load_max_x {
                let canonical = canonical_cell(self.source().topology, work_x, work_z);
                if !loaded.contains_key(&canonical) {
                    let candidates = self.cell_candidates(canonical)?;
                    loaded.insert(canonical, candidates);
                }
            }
        }

        let mut occurrences = BTreeMap::new();
        for work_z in min_cell_z..=max_cell_z {
            for work_x in min_cell_x..=max_cell_x {
                let canonical = canonical_cell(self.source().topology, work_x, work_z);
                let x_lift = (i64::from(work_x) - i64::from(canonical.x))
                    * i64::from(MCLONE_VEGETATION_PLANNING_CELL_BLOCKS);
                for candidate in loaded
                    .get(&canonical)
                    .expect("candidate cell was loaded with its conflict halo")
                {
                    if !candidate_survives(self.source().topology, *candidate, &loaded) {
                        continue;
                    }
                    let working_bounds = candidate.record.bounds.shifted_x(x_lift)?;
                    if !working_bounds.intersects_horizontal(bounds) {
                        continue;
                    }
                    occurrences.insert(
                        (candidate.record.id, x_lift),
                        McloneTreeOccurrence {
                            record: candidate.record,
                            x_lift,
                            working_bounds,
                        },
                    );
                }
            }
        }
        Ok(occurrences.into_values().collect())
    }

    fn cell_candidates(
        &mut self,
        cell: PlanningCell,
    ) -> Result<Vec<PreliminaryCandidate>, McloneVegetationError> {
        self.cell_requests += 1;
        if let Some(candidates) = self.cells.get(&cell) {
            self.cell_hits += 1;
            return Ok(candidates.clone());
        }
        let candidates = self
            .planner
            .preliminary_candidates(cell, &mut self.stream_cache)?;
        while self.cells.len() >= MAX_CACHED_CELLS {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            self.cells.remove(&oldest);
        }
        self.cells.insert(cell, candidates.clone());
        self.insertion_order.push_back(cell);
        Ok(candidates)
    }
}

pub fn tree_records_intersecting(
    source: McloneVegetationSource,
    bounds: McloneVegetationBounds,
) -> Result<Vec<McloneTreeOccurrence>, McloneVegetationError> {
    McloneOverworldVegetationPlanCache::new(source).tree_records_intersecting(bounds)
}

pub(crate) fn realize_mclone_tree_occurrences(
    region: &mut FeatureRegion,
    occurrences: &[McloneTreeOccurrence],
) -> McloneTreeRealizationReport {
    let mut report = McloneTreeRealizationReport {
        requested_occurrences: occurrences.len(),
        ..McloneTreeRealizationReport::default()
    };
    for occurrence in occurrences {
        report.realized_occurrences += 1;
        match occurrence.record.archetype {
            McloneTreeArchetype::RoundedBroadleaf => {
                realize_rounded_broadleaf(region, *occurrence, &mut report);
            }
            McloneTreeArchetype::LayeredConifer => {
                realize_layered_conifer(region, *occurrence, &mut report);
            }
            McloneTreeArchetype::ForkedAcacia => {
                realize_forked_acacia(region, *occurrence, &mut report);
            }
            McloneTreeArchetype::LayeredJungle => {
                realize_layered_jungle(region, *occurrence, &mut report);
            }
        }
    }
    report
}

fn realize_rounded_broadleaf(
    region: &mut FeatureRegion,
    occurrence: McloneTreeOccurrence,
    report: &mut McloneTreeRealizationReport,
) {
    let record = occurrence.record;
    let base = occurrence
        .working_base()
        .expect("validated tree occurrence has a representable working base");
    write_tree_block(
        region,
        occurrence,
        BlockPos::new(base.x, base.y - 1, base.z),
        DIRT,
        report,
    );

    let crown_center_y = base.y + i32::from(record.trunk_height) - 1;
    let crown_radius = i32::from(record.crown_radius);
    for dy in -2_i32..=2 {
        let layer_radius = match dy {
            -2..=0 => crown_radius,
            1 => crown_radius - 1,
            2 => 1,
            _ => unreachable!(),
        };
        for dz in -layer_radius..=layer_radius {
            for dx in -layer_radius..=layer_radius {
                let edge_depth = dx.abs() + dz.abs() - layer_radius;
                if edge_depth > 1 {
                    continue;
                }
                if edge_depth == 1 && (tree_voxel_hash(record.variant_seed, dx, dy, dz) & 3) != 0 {
                    continue;
                }
                write_tree_leaf(
                    region,
                    occurrence,
                    BlockPos::new(base.x + dx, crown_center_y + dy, base.z + dz),
                    OAK_LEAVES,
                    report,
                );
            }
        }
    }

    for dy in 0..i32::from(record.trunk_height) {
        write_tree_block(
            region,
            occurrence,
            BlockPos::new(base.x, base.y + dy, base.z),
            OAK_LOG,
            report,
        );
    }
}

fn realize_layered_jungle(
    region: &mut FeatureRegion,
    occurrence: McloneTreeOccurrence,
    report: &mut McloneTreeRealizationReport,
) {
    let record = occurrence.record;
    let base = occurrence
        .working_base()
        .expect("validated tree occurrence has a representable working base");
    write_tree_block(
        region,
        occurrence,
        BlockPos::new(base.x, base.y - 1, base.z),
        DIRT,
        report,
    );

    let trunk_height = i32::from(record.trunk_height);
    let crown_radius = i32::from(record.crown_radius);
    let crown_depth = i32::from(record.crown_depth).max(4);
    let broad_trunk = crown_radius >= 6 && trunk_height >= 18;
    for dy in 0..trunk_height {
        write_tree_block(
            region,
            occurrence,
            BlockPos::new(base.x, base.y + dy, base.z),
            JUNGLE_LOG,
            report,
        );
        if broad_trunk {
            write_tree_block(
                region,
                occurrence,
                BlockPos::new(base.x + 1, base.y + dy, base.z),
                JUNGLE_LOG,
                report,
            );
            write_tree_block(
                region,
                occurrence,
                BlockPos::new(base.x, base.y + dy, base.z + 1),
                JUNGLE_LOG,
                report,
            );
            write_tree_block(
                region,
                occurrence,
                BlockPos::new(base.x + 1, base.y + dy, base.z + 1),
                JUNGLE_LOG,
                report,
            );
        }
    }

    let crown_top_y = base.y + trunk_height + 1;
    for layer in 0..=crown_depth {
        let layer_radius = if layer == 0 {
            1
        } else if layer <= 2 {
            (crown_radius - 2 + layer).min(crown_radius)
        } else if layer + 2 >= crown_depth {
            (crown_radius - (layer + 2 - crown_depth)).max(2)
        } else {
            crown_radius
        };
        let y = crown_top_y - layer;
        for dz in -layer_radius..=layer_radius {
            for dx in -layer_radius..=layer_radius {
                let edge_depth = dx.abs() + dz.abs() - layer_radius;
                if edge_depth > 2 {
                    continue;
                }
                if edge_depth >= 1
                    && (tree_voxel_hash(record.variant_seed, dx, -layer, dz) & 3) != 0
                {
                    continue;
                }
                if layer > 1
                    && layer + 1 < crown_depth
                    && dx.abs() < 2
                    && dz.abs() < 2
                    && (tree_voxel_hash(record.variant_seed.rotate_left(11), dx, layer, dz) & 7)
                        == 0
                {
                    continue;
                }
                write_tree_leaf(
                    region,
                    occurrence,
                    BlockPos::new(base.x + dx, y, base.z + dz),
                    JUNGLE_LEAVES,
                    report,
                );
            }
        }
    }

    let branch_y = crown_top_y - crown_depth / 2;
    let branch_length = (crown_radius / 2).max(2);
    for turn in 0..4_u8 {
        let direction = tree_direction((record.orientation + turn) & 3);
        for step in 1..=branch_length {
            write_tree_block(
                region,
                occurrence,
                BlockPos::new(
                    base.x + direction.0 * step,
                    branch_y + step / 2,
                    base.z + direction.1 * step,
                ),
                JUNGLE_LOG,
                report,
            );
        }
    }
}

fn realize_layered_conifer(
    region: &mut FeatureRegion,
    occurrence: McloneTreeOccurrence,
    report: &mut McloneTreeRealizationReport,
) {
    let record = occurrence.record;
    let base = occurrence
        .working_base()
        .expect("validated tree occurrence has a representable working base");
    write_tree_block(
        region,
        occurrence,
        BlockPos::new(base.x, base.y - 1, base.z),
        DIRT,
        report,
    );

    let crown_top_y = base.y + i32::from(record.trunk_height);
    let crown_depth = i32::from(record.crown_depth);
    let crown_radius = i32::from(record.crown_radius);
    for layer in 0..=crown_depth {
        let layer_radius = if layer == 0 {
            0
        } else {
            let tapered = (1 + layer * crown_radius / crown_depth.max(1)).min(crown_radius);
            if layer % 3 == 0 {
                tapered.saturating_sub(1)
            } else {
                tapered
            }
        };
        let y = crown_top_y - layer;
        for dz in -layer_radius..=layer_radius {
            for dx in -layer_radius..=layer_radius {
                if dx.abs() == layer_radius
                    && dz.abs() == layer_radius
                    && (tree_voxel_hash(record.variant_seed, dx, -layer, dz) & 1) != 0
                {
                    continue;
                }
                write_tree_leaf(
                    region,
                    occurrence,
                    BlockPos::new(base.x + dx, y, base.z + dz),
                    SPRUCE_LEAVES,
                    report,
                );
            }
        }
    }

    for dy in 0..i32::from(record.trunk_height) {
        write_tree_block(
            region,
            occurrence,
            BlockPos::new(base.x, base.y + dy, base.z),
            SPRUCE_LOG,
            report,
        );
    }
}

fn realize_forked_acacia(
    region: &mut FeatureRegion,
    occurrence: McloneTreeOccurrence,
    report: &mut McloneTreeRealizationReport,
) {
    let record = occurrence.record;
    let base = occurrence
        .working_base()
        .expect("validated tree occurrence has a representable working base");
    write_tree_block(
        region,
        occurrence,
        BlockPos::new(base.x, base.y - 1, base.z),
        DIRT,
        report,
    );

    let trunk_height = i32::from(record.trunk_height);
    for dy in 0..=trunk_height - 3 {
        write_tree_block(
            region,
            occurrence,
            BlockPos::new(base.x, base.y + dy, base.z),
            ACACIA_LOG,
            report,
        );
    }

    let main_direction = tree_direction(record.orientation);
    let main_end = realize_acacia_branch(
        region,
        occurrence,
        BlockPos::new(base.x, base.y + trunk_height - 3, base.z),
        main_direction,
        3,
        report,
    );
    realize_flat_acacia_crown(region, occurrence, main_end, report);

    let fork_turn = if record.variant_seed & 1 == 0 { 1 } else { 3 };
    let fork_direction = tree_direction((record.orientation + fork_turn) & 3);
    let fork_end = realize_acacia_branch(
        region,
        occurrence,
        BlockPos::new(base.x, base.y + trunk_height - 4, base.z),
        fork_direction,
        2,
        report,
    );
    realize_flat_acacia_crown(region, occurrence, fork_end, report);
}

fn realize_acacia_branch(
    region: &mut FeatureRegion,
    occurrence: McloneTreeOccurrence,
    start: BlockPos,
    direction: (i32, i32),
    length: i32,
    report: &mut McloneTreeRealizationReport,
) -> BlockPos {
    let mut end = start;
    for step in 1..=length {
        end = BlockPos::new(
            start.x + direction.0 * step,
            start.y + step,
            start.z + direction.1 * step,
        );
        write_tree_block(region, occurrence, end, ACACIA_LOG, report);
    }
    end
}

fn realize_flat_acacia_crown(
    region: &mut FeatureRegion,
    occurrence: McloneTreeOccurrence,
    center: BlockPos,
    report: &mut McloneTreeRealizationReport,
) {
    let record = occurrence.record;
    let crown_radius = i32::from(record.crown_radius);
    for layer in 0..=1 {
        let layer_radius = crown_radius - layer;
        for dz in -layer_radius..=layer_radius {
            for dx in -layer_radius..=layer_radius {
                if dx.abs() + dz.abs() > layer_radius + 1 {
                    continue;
                }
                if dx.abs() + dz.abs() == layer_radius + 1
                    && (tree_voxel_hash(
                        record.variant_seed.rotate_left(17),
                        center.x + dx,
                        center.y + layer,
                        center.z + dz,
                    ) & 3)
                        != 0
                {
                    continue;
                }
                write_tree_leaf(
                    region,
                    occurrence,
                    BlockPos::new(center.x + dx, center.y + layer, center.z + dz),
                    ACACIA_LEAVES,
                    report,
                );
            }
        }
    }
}

const fn tree_direction(orientation: u8) -> (i32, i32) {
    match orientation & 3 {
        0 => (0, -1),
        1 => (1, 0),
        2 => (0, 1),
        3 => (-1, 0),
        _ => unreachable!(),
    }
}

fn write_tree_leaf(
    region: &mut FeatureRegion,
    occurrence: McloneTreeOccurrence,
    pos: BlockPos,
    leaves: RawBlockId,
    report: &mut McloneTreeRealizationReport,
) {
    if region
        .block_at_world_clipped(pos)
        .is_none_or(|block| block != AIR && !is_leaves(block))
    {
        return;
    }
    write_tree_block(region, occurrence, pos, leaves, report);
}

fn write_tree_block(
    region: &mut FeatureRegion,
    occurrence: McloneTreeOccurrence,
    pos: BlockPos,
    block: RawBlockId,
    report: &mut McloneTreeRealizationReport,
) {
    assert!(
        tree_bounds_contains(occurrence.working_bounds, pos),
        "tree {:?} emitted ({}, {}, {}) outside {:?}",
        occurrence.record,
        pos.x,
        pos.y,
        pos.z,
        occurrence.working_bounds
    );
    report.block_write_attempts += 1;
    if region.set_block_world_clipped(pos, block) {
        report.block_writes += 1;
    } else {
        report.clipped_block_writes += 1;
    }
}

const fn tree_bounds_contains(bounds: McloneTreeBounds, pos: BlockPos) -> bool {
    pos.x >= bounds.min_x
        && pos.x <= bounds.max_x
        && pos.y >= bounds.min_y
        && pos.y <= bounds.max_y
        && pos.z >= bounds.min_z
        && pos.z <= bounds.max_z
}

fn tree_voxel_hash(seed: u64, dx: i32, dy: i32, dz: i32) -> u64 {
    let mut value = splitmix64(seed ^ dx as u64);
    value = splitmix64(value ^ (dy as u64).rotate_left(21));
    splitmix64(value ^ (dz as u64).rotate_left(42))
}

fn forest_intent_from_landform(
    landform: McloneOverworldLandformSample,
    grove_source: f32,
) -> McloneForestIntentSample {
    let grove = (grove_source * 0.5 + 0.5).clamp(0.0, 1.0);
    let (family, density, coverage, mean_canopy_height, canopy_height_variation) =
        match mclone_overworld_biome_recipe(landform) {
            McloneOverworldBiomeRecipe::TemperateMeadow => {
                let clustered = grove * grove * grove;
                (
                    McloneTreeFamily::TemperateBroadleaf,
                    0.025 + clustered * 0.105,
                    0.04 + clustered * 0.16,
                    7.0,
                    1.4,
                )
            }
            McloneOverworldBiomeRecipe::TemperateWoodland => (
                McloneTreeFamily::TemperateBroadleaf,
                0.56 + grove * 0.32,
                0.58 + grove * 0.30,
                8.0,
                1.8,
            ),
            McloneOverworldBiomeRecipe::CoolWetConifer => (
                McloneTreeFamily::CoolWetConifer,
                0.66 + grove * 0.30,
                0.68 + grove * 0.29,
                10.0,
                2.4,
            ),
            McloneOverworldBiomeRecipe::WarmDrySteppe => {
                let (base_density, density_span, base_coverage, coverage_span) =
                    match mclone_overworld_steppe_band(landform.terrain.climate) {
                        McloneOverworldSteppeBand::Core => (0.14, 0.14, 0.12, 0.16),
                        McloneOverworldSteppeBand::Shoulder => (0.045, 0.075, 0.04, 0.09),
                        McloneOverworldSteppeBand::Outside => {
                            return McloneForestIntentSample::EMPTY;
                        }
                    };
                (
                    McloneTreeFamily::WarmDryAcacia,
                    base_density + grove * density_span,
                    base_coverage + grove * coverage_span,
                    7.5,
                    1.5,
                )
            }
            McloneOverworldBiomeRecipe::Ocean
            | McloneOverworldBiomeRecipe::Shore
            | McloneOverworldBiomeRecipe::River
            | McloneOverworldBiomeRecipe::SnowyAlpine => return McloneForestIntentSample::EMPTY,
        };
    McloneForestIntentSample {
        coverage,
        density,
        dominant_family: Some(family),
        secondary_family: None,
        family_mix: 0.0,
        mean_canopy_height,
        canopy_height_variation,
        grove_or_opening_influence: grove,
    }
}

fn forest_footprint_summary(
    samples: [McloneForestIntentSample; MCLONE_FOREST_SUMMARY_FOOTPRINT_TAPS],
) -> McloneForestIntentSample {
    let sample_weight = 1.0 / MCLONE_FOREST_SUMMARY_FOOTPRINT_TAPS as f32;
    let coverage = samples.iter().map(|sample| sample.coverage).sum::<f32>() * sample_weight;
    let density = samples.iter().map(|sample| sample.density).sum::<f32>() * sample_weight;
    let grove_or_opening_influence = samples
        .iter()
        .map(|sample| sample.grove_or_opening_influence)
        .sum::<f32>()
        * sample_weight;
    let mut family_coverage = [0.0_f32; 3];
    for sample in samples {
        let Some(family) = sample.dominant_family else {
            continue;
        };
        family_coverage[forest_family_index(family)] += sample.coverage;
    }
    let total_family_coverage = family_coverage.iter().sum::<f32>();
    if total_family_coverage <= f32::EPSILON {
        return McloneForestIntentSample::EMPTY;
    }

    let mut ranked_families = [
        (McloneTreeFamily::TemperateBroadleaf, family_coverage[0]),
        (McloneTreeFamily::CoolWetConifer, family_coverage[1]),
        (McloneTreeFamily::WarmDryAcacia, family_coverage[2]),
    ];
    ranked_families.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let dominant_family = ranked_families[0].0;
    let secondary_family = (ranked_families[1].1 > f32::EPSILON).then_some(ranked_families[1].0);
    let family_mix = 1.0 - ranked_families[0].1 / total_family_coverage;
    let mean_canopy_height = samples
        .iter()
        .map(|sample| sample.mean_canopy_height * sample.coverage)
        .sum::<f32>()
        / total_family_coverage;
    let canopy_height_variance = samples
        .iter()
        .map(|sample| {
            if sample.dominant_family.is_none() {
                return 0.0;
            }
            let mean_delta = sample.mean_canopy_height - mean_canopy_height;
            sample.coverage
                * (sample.canopy_height_variation * sample.canopy_height_variation
                    + mean_delta * mean_delta)
        })
        .sum::<f32>()
        / total_family_coverage;

    McloneForestIntentSample {
        coverage,
        density,
        dominant_family: Some(dominant_family),
        secondary_family,
        family_mix,
        mean_canopy_height,
        canopy_height_variation: canopy_height_variance.sqrt(),
        grove_or_opening_influence,
    }
}

const fn forest_family_index(family: McloneTreeFamily) -> usize {
    match family {
        McloneTreeFamily::TemperateBroadleaf => 0,
        McloneTreeFamily::CoolWetConifer => 1,
        McloneTreeFamily::WarmDryAcacia => 2,
        McloneTreeFamily::HumidJungleBroadleaf => 0,
    }
}

fn candidate_supports_landform(
    family: McloneTreeFamily,
    landform: McloneOverworldLandformSample,
) -> bool {
    if landform.terrain.watercourse.is_water() {
        return false;
    }
    if landform.terrain.watercourse.bank_influence >= 0.82 {
        return false;
    }
    if !matches!(
        mclone_overworld_surface_recipe(landform),
        McloneOverworldSurfaceRecipe::GrassSoil | McloneOverworldSurfaceRecipe::RiverBank
    ) {
        return false;
    }
    let maximum_slope = match family {
        McloneTreeFamily::TemperateBroadleaf => 0.65,
        McloneTreeFamily::CoolWetConifer => 0.85,
        McloneTreeFamily::WarmDryAcacia => 0.55,
        McloneTreeFamily::HumidJungleBroadleaf => 0.72,
    };
    landform.slope <= maximum_slope
}

fn resolve_silhouette(family: McloneTreeFamily, hash: u64) -> (McloneTreeArchetype, u16, u16, u16) {
    match family {
        McloneTreeFamily::TemperateBroadleaf => (
            McloneTreeArchetype::RoundedBroadleaf,
            5 + (hash % 3) as u16,
            2 + ((hash >> 8) % 2) as u16,
            4,
        ),
        McloneTreeFamily::CoolWetConifer => {
            let trunk_height = 7 + (hash % 6) as u16;
            (
                McloneTreeArchetype::LayeredConifer,
                trunk_height,
                2 + ((hash >> 8) % 2) as u16,
                trunk_height.saturating_sub(2),
            )
        }
        McloneTreeFamily::WarmDryAcacia => (
            McloneTreeArchetype::ForkedAcacia,
            5 + (hash % 4) as u16,
            3,
            3,
        ),
        McloneTreeFamily::HumidJungleBroadleaf => (
            McloneTreeArchetype::LayeredJungle,
            12 + (hash % 9) as u16,
            5 + ((hash >> 8) % 3) as u16,
            7 + ((hash >> 16) % 4) as u16,
        ),
    }
}

fn tree_bounds(
    base: BlockPos,
    family: McloneTreeFamily,
    trunk_height: u16,
    crown_radius: u16,
    crown_depth: u16,
) -> Result<McloneTreeBounds, McloneVegetationError> {
    let horizontal_radius = match family {
        McloneTreeFamily::WarmDryAcacia => i32::from(crown_radius) + 3,
        McloneTreeFamily::TemperateBroadleaf
        | McloneTreeFamily::CoolWetConifer
        | McloneTreeFamily::HumidJungleBroadleaf => i32::from(crown_radius),
    };
    let extra_height = match family {
        McloneTreeFamily::TemperateBroadleaf => 1,
        McloneTreeFamily::CoolWetConifer => 1,
        McloneTreeFamily::WarmDryAcacia => 2,
        McloneTreeFamily::HumidJungleBroadleaf => 2,
    };
    let max_y = base
        .y
        .checked_add(i32::from(trunk_height))
        .and_then(|value| value.checked_add(extra_height))
        .ok_or(McloneVegetationError::CoordinateOverflow)?;
    let crown_min_y = match family {
        McloneTreeFamily::CoolWetConifer => base
            .y
            .checked_add(i32::from(trunk_height))
            .and_then(|value| value.checked_sub(i32::from(crown_depth)))
            .map_or(base.y, |value| value.min(base.y)),
        McloneTreeFamily::TemperateBroadleaf
        | McloneTreeFamily::WarmDryAcacia
        | McloneTreeFamily::HumidJungleBroadleaf => base.y,
    };
    let support_y = base
        .y
        .checked_sub(1)
        .ok_or(McloneVegetationError::CoordinateOverflow)?;
    let min_y = crown_min_y.min(support_y);
    McloneTreeBounds::new(
        base.x
            .checked_sub(horizontal_radius)
            .ok_or(McloneVegetationError::CoordinateOverflow)?,
        min_y,
        base.z
            .checked_sub(horizontal_radius)
            .ok_or(McloneVegetationError::CoordinateOverflow)?,
        base.x
            .checked_add(horizontal_radius)
            .ok_or(McloneVegetationError::CoordinateOverflow)?,
        max_y,
        base.z
            .checked_add(horizontal_radius)
            .ok_or(McloneVegetationError::CoordinateOverflow)?,
    )
}

fn candidate_survives(
    topology: McloneOverworldSamplingTopology,
    candidate: PreliminaryCandidate,
    loaded: &BTreeMap<PlanningCell, Vec<PreliminaryCandidate>>,
) -> bool {
    let source_cell = PlanningCell {
        x: candidate.record.id.planning_cell_x,
        z: candidate.record.id.planning_cell_z,
    };
    for offset_z in -CONFLICT_CELL_HALO..=CONFLICT_CELL_HALO {
        for offset_x in -CONFLICT_CELL_HALO..=CONFLICT_CELL_HALO {
            let neighbor =
                canonical_cell(topology, source_cell.x + offset_x, source_cell.z + offset_z);
            let Some(others) = loaded.get(&neighbor) else {
                continue;
            };
            for other in others {
                if other.record.id == candidate.record.id
                    || !candidates_conflict(topology, candidate, *other)
                {
                    continue;
                }
                if (other.conflict_priority, other.record.id)
                    < (candidate.conflict_priority, candidate.record.id)
                {
                    return false;
                }
            }
        }
    }
    true
}

fn candidates_conflict(
    topology: McloneOverworldSamplingTopology,
    left: PreliminaryCandidate,
    right: PreliminaryCandidate,
) -> bool {
    let horizontal = topology.horizontal_topology();
    let dx = horizontal
        .x
        .shortest_block_displacement(left.record.canonical_base.x, right.record.canonical_base.x);
    let dz = i64::from(right.record.canonical_base.z) - i64::from(left.record.canonical_base.z);
    let spacing = i64::from(left.minimum_spacing.max(right.minimum_spacing));
    dx * dx + dz * dz < spacing * spacing
}

fn canonical_cell(
    topology: McloneOverworldSamplingTopology,
    work_x: i32,
    work_z: i32,
) -> PlanningCell {
    let x = match topology {
        McloneOverworldSamplingTopology::Unbounded => work_x,
        McloneOverworldSamplingTopology::PeriodicX => work_x
            .rem_euclid(MCLONE_OVERWORLD_PERIOD_BLOCKS / MCLONE_VEGETATION_PLANNING_CELL_BLOCKS),
    };
    PlanningCell { x, z: work_z }
}

fn validate_query_cell_count(
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
) -> Result<(), McloneVegetationError> {
    let width = i64::from(max_x) - i64::from(min_x) + 1;
    let depth = i64::from(max_z) - i64::from(min_z) + 1;
    let cells = width
        .checked_mul(depth)
        .ok_or(McloneVegetationError::CoordinateOverflow)?;
    if cells > MAX_QUERY_CELLS {
        return Err(McloneVegetationError::QueryTooLarge {
            cells,
            maximum: MAX_QUERY_CELLS,
        });
    }
    Ok(())
}

fn candidate_hash(
    source: McloneVegetationSource,
    cell: PlanningCell,
    candidate_slot: u8,
    lane: u64,
) -> u64 {
    let topology = match source.topology {
        McloneOverworldSamplingTopology::Unbounded => 0_u64,
        McloneOverworldSamplingTopology::PeriodicX => 1_u64,
    };
    let mut value = source.seed as u64 ^ lane;
    value = splitmix64(value ^ topology);
    value = splitmix64(value ^ u64::from(VEGETATION_PLAN_REVISION));
    value = splitmix64(value ^ cell.x as u64);
    value = splitmix64(value ^ cell.z as u64);
    splitmix64(value ^ u64::from(candidate_slot))
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn shift_coordinate(coordinate: i32, offset: i64) -> Result<i32, McloneVegetationError> {
    i32::try_from(i64::from(coordinate) + offset)
        .map_err(|_| McloneVegetationError::CoordinateOverflow)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn bounds(min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> McloneVegetationBounds {
        McloneVegetationBounds::new(min_x, min_z, max_x, max_z).unwrap()
    }

    #[test]
    fn footprint_summary_filters_coverage_family_and_canopy() {
        let intent = |family, coverage, height, variation, grove| McloneForestIntentSample {
            coverage,
            density: coverage * 0.8,
            dominant_family: Some(family),
            secondary_family: None,
            family_mix: 0.0,
            mean_canopy_height: height,
            canopy_height_variation: variation,
            grove_or_opening_influence: grove,
        };
        let summary = forest_footprint_summary([
            intent(McloneTreeFamily::TemperateBroadleaf, 0.8, 8.0, 1.0, 0.8),
            intent(McloneTreeFamily::CoolWetConifer, 0.4, 10.0, 2.0, 0.4),
            McloneForestIntentSample::EMPTY,
            intent(McloneTreeFamily::TemperateBroadleaf, 0.4, 6.0, 1.0, 0.2),
        ]);

        assert!((summary.coverage - 0.4).abs() < 1.0e-6);
        assert!((summary.density - 0.32).abs() < 1.0e-6);
        assert_eq!(
            summary.dominant_family,
            Some(McloneTreeFamily::TemperateBroadleaf)
        );
        assert_eq!(
            summary.secondary_family,
            Some(McloneTreeFamily::CoolWetConifer)
        );
        assert!((summary.family_mix - 0.25).abs() < 1.0e-6);
        assert!((summary.mean_canopy_height - 8.0).abs() < 1.0e-6);
        assert!(summary.canopy_height_variation > 1.7);
        assert!((summary.grove_or_opening_influence - 0.35).abs() < 1.0e-6);
    }

    #[test]
    fn forest_edge_intent_is_deterministic_and_directional() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let mut first = McloneOverworldVegetationPlanCache::new(source);
        let mut second = McloneOverworldVegetationPlanCache::new(source);
        let sample = first.forest_edge_intent_at(-640, 96).unwrap();

        assert_eq!(sample, second.forest_edge_intent_at(-640, 96).unwrap());
        assert!(sample.nearby_min_coverage <= sample.nearby_max_coverage);
        assert_eq!(
            sample.edge_contrast,
            sample.nearby_max_coverage - sample.nearby_min_coverage
        );
        assert_ne!(sample.clearing_direction, McloneForestDirection::default());
        assert_ne!(sample.cover_direction, McloneForestDirection::default());
    }

    #[test]
    fn forest_edge_intent_finds_sparse_but_useful_transitions() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let mut cache = McloneOverworldVegetationPlanCache::new(source);
        let mut edges = 0_usize;
        let mut dense_interiors = 0_usize;
        let mut open_interiors = 0_usize;
        let mut max_contrast = 0.0_f32;
        let mut max_contrast_sample = None;
        let mut transition_candidates = 0_usize;

        for z in (-1536..=1536).step_by(32) {
            for x in (-1536..=1536).step_by(32) {
                let sample = cache.forest_edge_intent_at(x, z).unwrap();
                edges += usize::from(sample.is_transitional_edge());
                if sample.edge_contrast > max_contrast {
                    max_contrast = sample.edge_contrast;
                    max_contrast_sample = Some((x, z, sample));
                }
                transition_candidates += usize::from(
                    sample.nearby_min_coverage <= 0.45
                        && sample.nearby_max_coverage >= 0.42
                        && sample.edge_contrast >= 0.12,
                );
                dense_interiors += usize::from(
                    sample.local.coverage >= 0.70 && sample.nearby_min_coverage >= 0.48,
                );
                open_interiors += usize::from(
                    sample.local.coverage <= 0.16 && sample.nearby_max_coverage <= 0.36,
                );
            }
        }

        assert!(
            (24..=900).contains(&edges),
            "unexpected edge count {edges}; candidates {transition_candidates}; max contrast {max_contrast}; sample {max_contrast_sample:?}"
        );
        assert!(dense_interiors > 16, "missing dense forest interiors");
        assert!(open_interiors > 16, "missing open interiors");
    }

    #[test]
    fn record_queries_are_partition_and_order_independent() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let whole = tree_records_intersecting(source, bounds(-256, -256, 255, 255)).unwrap();
        assert!(!whole.is_empty());

        let partitions = [
            bounds(-256, -256, -1, -1),
            bounds(0, -256, 255, -1),
            bounds(-256, 0, -1, 255),
            bounds(0, 0, 255, 255),
        ];
        let forward = partitions
            .into_iter()
            .flat_map(|query| tree_records_intersecting(source, query).unwrap())
            .collect::<BTreeSet<_>>();
        let reversed = partitions
            .into_iter()
            .rev()
            .flat_map(|query| tree_records_intersecting(source, query).unwrap())
            .collect::<BTreeSet<_>>();
        let whole = whole.into_iter().collect::<BTreeSet<_>>();
        assert_eq!(forward, whole);
        assert_eq!(reversed, whole);
    }

    #[test]
    fn cache_is_optional_for_records() {
        let source =
            McloneVegetationSource::new(-98_765, McloneOverworldSamplingTopology::Unbounded);
        let query = bounds(-96, -96, 95, 95);
        let cold = tree_records_intersecting(source, query).unwrap();
        let mut cache = McloneOverworldVegetationPlanCache::new(source);
        let first = cache.tree_records_intersecting(query).unwrap();
        let before = cache.report();
        let second = cache.tree_records_intersecting(query).unwrap();
        let after = cache.report();

        assert_eq!(cold, first);
        assert_eq!(first, second);
        assert!(after.cell_hits > before.cell_hits);
        assert_eq!(after.cell_requests, after.cell_hits + after.cell_misses);
    }

    #[test]
    fn surviving_records_respect_symmetric_minimum_spacing() {
        let source =
            McloneVegetationSource::new(8_675_309, McloneOverworldSamplingTopology::Unbounded);
        let records = tree_records_intersecting(source, bounds(-768, -768, 767, 767)).unwrap();
        assert!(
            records.len() > 16,
            "spacing survey requires at least 17 records, got {}",
            records.len()
        );
        for (index, left) in records.iter().enumerate() {
            for right in &records[index + 1..] {
                let dx = i64::from(right.record.canonical_base.x)
                    - i64::from(left.record.canonical_base.x);
                let dz = i64::from(right.record.canonical_base.z)
                    - i64::from(left.record.canonical_base.z);
                let spacing = i64::from(
                    left.record
                        .family
                        .minimum_spacing()
                        .max(right.record.family.minimum_spacing()),
                );
                assert!(
                    dx * dx + dz * dz >= spacing * spacing,
                    "{:?} conflicts with {:?}",
                    left.record.id,
                    right.record.id
                );
            }
        }
    }

    #[test]
    fn negative_coordinates_and_cell_edges_are_stable() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let crossing = tree_records_intersecting(source, bounds(-33, -33, 32, 32)).unwrap();
        let columns = [bounds(-33, -33, -1, 32), bounds(0, -33, 32, 32)]
            .into_iter()
            .flat_map(|query| tree_records_intersecting(source, query).unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(crossing.into_iter().collect::<BTreeSet<_>>(), columns);
    }

    #[test]
    fn record_queries_reject_coordinate_overflow() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);

        assert_eq!(
            tree_records_intersecting(
                source,
                bounds(i32::MAX - 1, i32::MAX - 1, i32::MAX, i32::MAX),
            ),
            Err(McloneVegetationError::CoordinateOverflow)
        );
    }

    #[test]
    fn periodic_queries_reuse_canonical_records_across_seam_lifts() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::PeriodicX);
        let near_zero = tree_records_intersecting(source, bounds(-64, -64, 63, 63)).unwrap();
        let near_period = tree_records_intersecting(
            source,
            bounds(
                MCLONE_OVERWORLD_PERIOD_BLOCKS - 64,
                -64,
                MCLONE_OVERWORLD_PERIOD_BLOCKS + 63,
                63,
            ),
        )
        .unwrap();
        let zero_records = near_zero
            .iter()
            .map(|occurrence| occurrence.record)
            .collect::<BTreeSet<_>>();
        let period_records = near_period
            .iter()
            .map(|occurrence| occurrence.record)
            .collect::<BTreeSet<_>>();

        assert!(!zero_records.is_empty());
        assert_eq!(zero_records, period_records);
        assert!(near_zero.iter().any(|occurrence| occurrence.x_lift < 0));
        assert!(near_period.iter().any(|occurrence| occurrence.x_lift > 0));
    }

    #[test]
    fn record_bounds_contain_base_and_are_build_height_bounded() {
        let source =
            McloneVegetationSource::new(-98_765, McloneOverworldSamplingTopology::Unbounded);
        for occurrence in tree_records_intersecting(source, bounds(-512, -512, 511, 511)).unwrap() {
            let record = occurrence.record;
            assert!(record.bounds.min_x <= record.canonical_base.x);
            assert!(record.bounds.max_x >= record.canonical_base.x);
            assert!(record.bounds.min_y <= record.canonical_base.y);
            assert!(record.bounds.max_y < FLAT_GRASS_HEIGHT);
            assert!(record.bounds.min_z <= record.canonical_base.z);
            assert!(record.bounds.max_z >= record.canonical_base.z);
        }
    }

    #[test]
    fn representative_record_receipt_is_pinned() {
        let source =
            McloneVegetationSource::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        let records = [
            bounds(-46 * 16, 3 * 16, -40 * 16 + 15, 9 * 16 + 15),
            bounds(-8 * 16, -3 * 16, -2 * 16 + 15, 3 * 16 + 15),
        ]
        .into_iter()
        .flat_map(|query| tree_records_intersecting(source, query).unwrap())
        .map(|occurrence| occurrence.record)
        .collect::<BTreeSet<_>>();
        let mut family_counts = [0_usize; 3];
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for record in &records {
            let family = match record.family {
                McloneTreeFamily::TemperateBroadleaf => 0_u8,
                McloneTreeFamily::CoolWetConifer => 1_u8,
                McloneTreeFamily::WarmDryAcacia => 2_u8,
                McloneTreeFamily::HumidJungleBroadleaf => {
                    panic!("V1 receipt unexpectedly contains a humid-jungle tree")
                }
            };
            family_counts[usize::from(family)] += 1;
            let archetype = match record.archetype {
                McloneTreeArchetype::RoundedBroadleaf => 0_u8,
                McloneTreeArchetype::LayeredConifer => 1_u8,
                McloneTreeArchetype::ForkedAcacia => 2_u8,
                McloneTreeArchetype::LayeredJungle => {
                    panic!("V1 receipt unexpectedly contains a layered-jungle tree")
                }
            };
            for byte in record
                .id
                .planning_cell_x
                .to_le_bytes()
                .into_iter()
                .chain(record.id.planning_cell_z.to_le_bytes())
                .chain([record.id.candidate_slot])
                .chain(record.id.vegetation_revision.to_le_bytes())
                .chain(record.canonical_base.x.to_le_bytes())
                .chain(record.canonical_base.y.to_le_bytes())
                .chain(record.canonical_base.z.to_le_bytes())
                .chain([family, archetype])
                .chain(record.trunk_height.to_le_bytes())
                .chain(record.crown_radius.to_le_bytes())
                .chain(record.crown_depth.to_le_bytes())
                .chain([record.orientation, record.landmark_rank])
                .chain(record.variant_seed.to_le_bytes())
                .chain(record.bounds.min_x.to_le_bytes())
                .chain(record.bounds.min_y.to_le_bytes())
                .chain(record.bounds.min_z.to_le_bytes())
                .chain(record.bounds.max_x.to_le_bytes())
                .chain(record.bounds.max_y.to_le_bytes())
                .chain(record.bounds.max_z.to_le_bytes())
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }

        assert_eq!(
            (records.len(), family_counts, hash),
            (89, [8, 50, 31], 8_961_720_747_770_129_807)
        );
    }
}
