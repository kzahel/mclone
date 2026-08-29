use std::{error::Error, fmt};

use mclone_core::{AxisTopology, BlockPos, CHUNK_WIDTH, ChunkPos, HorizontalTopology};

use crate::{
    continental_ecoregion::ContinentalEcoregionDescriptor,
    continental_surface::{
        ContinentalSurfacePlan, ContinentalSurfaceSubstrate, ContinentalSurfaceWaterKind,
    },
    mclone_overworld_v3::{McloneOverworldV3TerrainPlan, V3SurfaceSubstrate, V3WaterKind},
};

use super::mclone_overworld::{
    MCLONE_OVERWORLD_PERIOD_CHUNKS, MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldSampler,
    McloneOverworldSamplingTopology, McloneOverworldVegetationPlanner, McloneVegetationSource,
};

pub const WILDLIFE_POPULATION_REVISION: u16 = 3;
pub const WILDLIFE_POPULATION_CELL_BLOCKS: i32 = CHUNK_WIDTH * 4;
pub const WILDLIFE_POPULATION_CELL_CHUNKS: i32 = WILDLIFE_POPULATION_CELL_BLOCKS / CHUNK_WIDTH;
pub const WILDLIFE_CANDIDATES_PER_CELL: usize = 9;

// Compatibility names for the first habitat-driven population revision.
pub const MCLONE_WILDLIFE_POPULATION_REVISION: u16 = WILDLIFE_POPULATION_REVISION;
pub const MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS: i32 = WILDLIFE_POPULATION_CELL_BLOCKS;
pub const MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS: i32 = WILDLIFE_POPULATION_CELL_CHUNKS;
pub const MCLONE_WILDLIFE_CANDIDATES_PER_CELL: usize = WILDLIFE_CANDIDATES_PER_CELL;

const PERMILLE: u32 = 1_000;
const HASH_LANE_POSITION_X: u64 = 0x7769_6c64_5f78_3031;
const HASH_LANE_POSITION_Z: u64 = 0x7769_6c64_5f7a_3031;
const HASH_LANE_OCCUPANCY: u64 = 0x7769_6c64_5f6f_3031;
const HASH_LANE_SPECIES: u64 = 0x7769_6c64_5f73_3031;
const HASH_LANE_GROUP_SIZE: u64 = 0x7769_6c64_5f67_3031;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WildlifePopulationCell {
    pub x: i32,
    pub z: i32,
}

impl WildlifePopulationCell {
    pub fn from_chunk(chunk: ChunkPos) -> Self {
        Self {
            x: chunk.x.div_euclid(WILDLIFE_POPULATION_CELL_CHUNKS),
            z: chunk.z.div_euclid(WILDLIFE_POPULATION_CELL_CHUNKS),
        }
    }

    pub fn from_block(world_x: i32, world_z: i32) -> Self {
        Self {
            x: world_x.div_euclid(WILDLIFE_POPULATION_CELL_BLOCKS),
            z: world_z.div_euclid(WILDLIFE_POPULATION_CELL_BLOCKS),
        }
    }

    pub fn min_block_x(self) -> Result<i32, WildlifePlanError> {
        checked_cell_origin(self.x)
    }

    pub fn min_block_z(self) -> Result<i32, WildlifePlanError> {
        checked_cell_origin(self.z)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WildlifeSpecies {
    Rabbit = 0,
    Deer = 1,
    Mallard = 2,
    Bee = 3,
    Squirrel = 4,
    Cow = 5,
    Chicken = 6,
}

impl WildlifeSpecies {
    pub const ALL: [Self; 7] = [
        Self::Rabbit,
        Self::Deer,
        Self::Mallard,
        Self::Bee,
        Self::Squirrel,
        Self::Cow,
        Self::Chicken,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Rabbit => "rabbit",
            Self::Deer => "deer",
            Self::Mallard => "mallard",
            Self::Bee => "bee",
            Self::Squirrel => "squirrel",
            Self::Cow => "cow",
            Self::Chicken => "chicken",
        }
    }

    pub const fn group_size_range(self) -> (u8, u8) {
        match self {
            Self::Rabbit => (2, 4),
            Self::Deer => (2, 3),
            Self::Mallard => (2, 4),
            Self::Bee => (2, 3),
            Self::Squirrel => (2, 4),
            Self::Cow => (2, 4),
            Self::Chicken => (2, 4),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WildlifeSuitability {
    pub rabbit: u16,
    pub deer: u16,
    pub mallard: u16,
    pub bee: u16,
    pub squirrel: u16,
    pub cow: u16,
    pub chicken: u16,
}

impl WildlifeSuitability {
    pub const fn for_species(self, species: WildlifeSpecies) -> u16 {
        match species {
            WildlifeSpecies::Rabbit => self.rabbit,
            WildlifeSpecies::Deer => self.deer,
            WildlifeSpecies::Mallard => self.mallard,
            WildlifeSpecies::Bee => self.bee,
            WildlifeSpecies::Squirrel => self.squirrel,
            WildlifeSpecies::Cow => self.cow,
            WildlifeSpecies::Chicken => self.chicken,
        }
    }

    pub fn total(self) -> u32 {
        WildlifeSpecies::ALL
            .into_iter()
            .map(|species| u32::from(self.for_species(species)))
            .sum()
    }

    pub fn maximum(self) -> u16 {
        WildlifeSpecies::ALL
            .into_iter()
            .map(|species| self.for_species(species))
            .max()
            .unwrap_or(0)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WildlifeHabitatSource {
    McloneOverworldV1 = 0,
    McloneOverworldV2 = 1,
    McloneOverworldV3 = 2,
    PublishedBlocks = 3,
}

impl WildlifeHabitatSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::McloneOverworldV1 => "mclone-overworld-v1",
            Self::McloneOverworldV2 => "mclone-overworld-v2",
            Self::McloneOverworldV3 => "mclone-overworld-v3",
            Self::PublishedBlocks => "published-blocks",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WildlifeEvidenceSet(u32);

impl WildlifeEvidenceSet {
    pub const NONE: Self = Self(0);
    pub const LAND: Self = Self(1 << 0);
    pub const PRODUCTIVITY: Self = Self(1 << 1);
    pub const OPENNESS: Self = Self(1 << 2);
    pub const LOW_COVER: Self = Self(1 << 3);
    pub const FOREST_COVER: Self = Self(1 << 4);
    pub const FOREST_EDGE: Self = Self(1 << 5);
    pub const WETLAND: Self = Self(1 << 6);
    pub const WATER: Self = Self(1 << 7);
    pub const INLAND_WATER: Self = Self(1 << 8);
    pub const SHORE: Self = Self(1 << 9);
    pub const BANK: Self = Self(1 << 10);
    pub const FLOWERING: Self = Self(1 << 11);
    pub const SEEDS_AND_SOFT_MAST: Self = Self(1 << 12);
    pub const MATURE_TREES: Self = Self(1 << 13);
    pub const ALL: Self = Self((1 << 14) - 1);
    pub const FACTS: [(Self, &'static str); 14] = [
        (Self::LAND, "land"),
        (Self::PRODUCTIVITY, "productivity"),
        (Self::OPENNESS, "openness"),
        (Self::LOW_COVER, "low-cover"),
        (Self::FOREST_COVER, "forest-cover"),
        (Self::FOREST_EDGE, "forest-edge"),
        (Self::WETLAND, "wetland"),
        (Self::WATER, "water"),
        (Self::INLAND_WATER, "inland-water"),
        (Self::SHORE, "shore"),
        (Self::BANK, "bank"),
        (Self::FLOWERING, "flowering"),
        (Self::SEEDS_AND_SOFT_MAST, "seeds-and-soft-mast"),
        (Self::MATURE_TREES, "mature-trees"),
    ];

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    pub fn labels(self) -> impl Iterator<Item = &'static str> {
        Self::FACTS
            .into_iter()
            .filter_map(move |(fact, label)| self.contains(fact).then_some(label))
    }
}

/// Profile-neutral evidence consumed by the shared species scorer.
///
/// Values are permille. `available_evidence` distinguishes an observed zero
/// from a fact the source cannot prove; neither becomes a random fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WildlifeHabitatSample {
    pub world_x: i32,
    pub world_z: i32,
    pub surface_y: i32,
    pub source: WildlifeHabitatSource,
    pub supported: bool,
    pub available_evidence: WildlifeEvidenceSet,
    pub land: u16,
    pub productivity: u16,
    pub openness: u16,
    pub low_cover: u16,
    pub forest_cover: u16,
    pub forest_edge: u16,
    pub wetland: u16,
    pub water: u16,
    pub inland_water: u16,
    pub shore: u16,
    pub bank: u16,
    pub flowering: u16,
    pub seeds_and_soft_mast: u16,
    pub mature_trees: u16,
}

impl WildlifeHabitatSample {
    pub const fn unavailable(world_x: i32, world_z: i32, source: WildlifeHabitatSource) -> Self {
        Self {
            world_x,
            world_z,
            surface_y: 0,
            source,
            supported: false,
            available_evidence: WildlifeEvidenceSet::NONE,
            land: 0,
            productivity: 0,
            openness: 0,
            low_cover: 0,
            forest_cover: 0,
            forest_edge: 0,
            wetland: 0,
            water: 0,
            inland_water: 0,
            shore: 0,
            bank: 0,
            flowering: 0,
            seeds_and_soft_mast: 0,
            mature_trees: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WildlifeCandidateSite {
    pub slot: u8,
    pub world_x: i32,
    pub world_z: i32,
    pub available: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WildlifeEncounter {
    pub species: WildlifeSpecies,
    pub group_size: u8,
    pub anchor_x: i32,
    pub anchor_z: i32,
    pub owner_chunk: ChunkPos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WildlifeCellPlan {
    pub cell: WildlifePopulationCell,
    pub representative: WildlifeHabitatSample,
    pub selected_habitat: WildlifeHabitatSample,
    pub suitability: WildlifeSuitability,
    pub desired_density: u16,
    pub occupancy_roll: u16,
    pub species_roll: u32,
    pub encounter: Option<WildlifeEncounter>,
}

impl WildlifeCellPlan {
    pub fn encounter_for_chunk(self, chunk: ChunkPos) -> Option<WildlifeEncounter> {
        self.encounter
            .filter(|encounter| encounter.owner_chunk == chunk)
    }
}

#[derive(Clone, Debug)]
enum WildlifeHabitatAdapter {
    V1 {
        terrain: McloneOverworldSampler,
        vegetation: McloneOverworldVegetationPlanner,
    },
    V2(ContinentalSurfacePlan),
    V3(McloneOverworldV3TerrainPlan),
    PublishedBlocks,
}

#[derive(Clone, Debug)]
pub struct WildlifePopulationPlanner {
    seed: i64,
    topology: HorizontalTopology,
    source: WildlifeHabitatSource,
    adapter: WildlifeHabitatAdapter,
}

impl WildlifePopulationPlanner {
    /// Compatibility constructor for the original V1 population compiler.
    pub fn new(seed: i64, topology: McloneOverworldSamplingTopology) -> Self {
        let horizontal = match topology {
            McloneOverworldSamplingTopology::Unbounded => HorizontalTopology::UNBOUNDED,
            McloneOverworldSamplingTopology::PeriodicX => {
                HorizontalTopology::cylinder_x(0, MCLONE_OVERWORLD_PERIOD_CHUNKS as u32)
            }
        };
        Self::for_habitat(seed, horizontal, WildlifeHabitatSource::McloneOverworldV1)
            .expect("the compatibility V1 topology is supported")
    }

    pub fn for_habitat(
        seed: i64,
        topology: HorizontalTopology,
        source: WildlifeHabitatSource,
    ) -> Result<Self, WildlifePlanError> {
        topology
            .validate()
            .map_err(|_| WildlifePlanError::UnsupportedTopology(source))?;
        let adapter = match source {
            WildlifeHabitatSource::McloneOverworldV1 => {
                let v1_topology =
                    v1_topology(topology).ok_or(WildlifePlanError::UnsupportedTopology(source))?;
                WildlifeHabitatAdapter::V1 {
                    terrain: McloneOverworldSampler::new_with_topology(seed, v1_topology),
                    vegetation: McloneOverworldVegetationPlanner::new(McloneVegetationSource::new(
                        seed,
                        v1_topology,
                    )),
                }
            }
            WildlifeHabitatSource::McloneOverworldV2 if topology.is_unbounded() => {
                WildlifeHabitatAdapter::V2(
                    ContinentalSurfacePlan::new(ContinentalEcoregionDescriptor::plane(seed))
                        .map_err(|_| WildlifePlanError::AdapterConstruction(source))?,
                )
            }
            WildlifeHabitatSource::McloneOverworldV3 if topology.is_unbounded() => {
                WildlifeHabitatAdapter::V3(McloneOverworldV3TerrainPlan::new(seed))
            }
            WildlifeHabitatSource::PublishedBlocks => WildlifeHabitatAdapter::PublishedBlocks,
            _ => return Err(WildlifePlanError::UnsupportedTopology(source)),
        };
        Ok(Self {
            seed,
            topology,
            source,
            adapter,
        })
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn horizontal_topology(&self) -> HorizontalTopology {
        self.topology
    }

    pub const fn habitat_source(&self) -> WildlifeHabitatSource {
        self.source
    }

    pub fn candidate_sites(
        &self,
        requested_cell: WildlifePopulationCell,
    ) -> Result<Vec<WildlifeCandidateSite>, WildlifePlanError> {
        let cell = self.canonical_cell(requested_cell)?;
        let origin_x = cell.min_block_x()?;
        let origin_z = cell.min_block_z()?;
        let mut sites = Vec::with_capacity(WILDLIFE_CANDIDATES_PER_CELL);
        for candidate in 0..WILDLIFE_CANDIDATES_PER_CELL {
            let column = i32::try_from(candidate % 3).expect("candidate column fits i32");
            let row = i32::try_from(candidate / 3).expect("candidate row fits i32");
            let stratum_min_x = origin_x
                .checked_add(column * (WILDLIFE_POPULATION_CELL_BLOCKS / 3))
                .ok_or(WildlifePlanError::CoordinateOverflow)?;
            let stratum_min_z = origin_z
                .checked_add(row * (WILDLIFE_POPULATION_CELL_BLOCKS / 3))
                .ok_or(WildlifePlanError::CoordinateOverflow)?;
            let stratum_width = if column == 2 { 22 } else { 21 };
            let stratum_depth = if row == 2 { 22 } else { 21 };
            let world_x = stratum_min_x
                .checked_add(hash_bounded(
                    self.source_hash(cell, candidate as u8, HASH_LANE_POSITION_X),
                    stratum_width,
                ))
                .ok_or(WildlifePlanError::CoordinateOverflow)?;
            let world_z = stratum_min_z
                .checked_add(hash_bounded(
                    self.source_hash(cell, candidate as u8, HASH_LANE_POSITION_Z),
                    stratum_depth,
                ))
                .ok_or(WildlifePlanError::CoordinateOverflow)?;
            let canonical = self
                .topology
                .canonicalize_block(BlockPos::new(world_x, 0, world_z));
            sites.push(WildlifeCandidateSite {
                slot: candidate as u8,
                world_x,
                world_z,
                available: canonical.is_some_and(|pos| pos.x == world_x && pos.z == world_z),
            });
        }
        Ok(sites)
    }

    pub fn plan_for_chunk(&self, chunk: ChunkPos) -> Result<WildlifeCellPlan, WildlifePlanError> {
        self.plan_cell(WildlifePopulationCell::from_chunk(chunk))
    }

    pub fn plan_cell(
        &self,
        requested_cell: WildlifePopulationCell,
    ) -> Result<WildlifeCellPlan, WildlifePlanError> {
        if matches!(self.adapter, WildlifeHabitatAdapter::PublishedBlocks) {
            return Err(WildlifePlanError::ExternalEvidenceRequired);
        }
        let sites = self.candidate_sites(requested_cell)?;
        let samples = sites
            .iter()
            .map(|site| {
                if site.available {
                    self.sample_habitat(site.world_x, site.world_z)
                } else {
                    WildlifeHabitatSample::unavailable(site.world_x, site.world_z, self.source)
                }
            })
            .collect::<Vec<_>>();
        self.plan_from_samples(requested_cell, &sites, &samples)
    }

    pub fn plan_cell_with_samples(
        &self,
        requested_cell: WildlifePopulationCell,
        samples: &[WildlifeHabitatSample],
    ) -> Result<WildlifeCellPlan, WildlifePlanError> {
        let sites = self.candidate_sites(requested_cell)?;
        self.plan_from_samples(requested_cell, &sites, samples)
    }

    fn plan_from_samples(
        &self,
        requested_cell: WildlifePopulationCell,
        sites: &[WildlifeCandidateSite],
        samples: &[WildlifeHabitatSample],
    ) -> Result<WildlifeCellPlan, WildlifePlanError> {
        if samples.len() != WILDLIFE_CANDIDATES_PER_CELL {
            return Err(WildlifePlanError::EvidenceCount {
                expected: WILDLIFE_CANDIDATES_PER_CELL,
                actual: samples.len(),
            });
        }
        for (site, sample) in sites.iter().zip(samples) {
            if sample.world_x != site.world_x
                || sample.world_z != site.world_z
                || (sample.supported && !site.available)
            {
                return Err(WildlifePlanError::MismatchedEvidence(site.slot));
            }
        }

        let cell = self.canonical_cell(requested_cell)?;
        let sample_weights = samples
            .iter()
            .copied()
            .map(score_habitat)
            .collect::<Vec<_>>();
        let representative = samples[4];
        let mut suitability = WildlifeSuitability::default();
        let mut best_candidate = [0_usize; 7];
        for species in WildlifeSpecies::ALL {
            let (index, weight) = sample_weights
                .iter()
                .enumerate()
                .map(|(index, weights)| (index, weights.for_species(species)))
                .max_by_key(|(index, weight)| (*weight, std::cmp::Reverse(*index)))
                .expect("wildlife cells always have candidates");
            best_candidate[species as usize] = index;
            match species {
                WildlifeSpecies::Rabbit => suitability.rabbit = weight,
                WildlifeSpecies::Deer => suitability.deer = weight,
                WildlifeSpecies::Mallard => suitability.mallard = weight,
                WildlifeSpecies::Bee => suitability.bee = weight,
                WildlifeSpecies::Squirrel => suitability.squirrel = weight,
                WildlifeSpecies::Cow => suitability.cow = weight,
                WildlifeSpecies::Chicken => suitability.chicken = weight,
            }
        }

        let supported = samples.iter().filter(|sample| sample.supported);
        let supported_count = supported.clone().count();
        let mean_productivity = if supported_count == 0 {
            0
        } else {
            supported
                .map(|sample| u32::from(sample.productivity))
                .sum::<u32>()
                / u32::try_from(supported_count).expect("candidate count fits u32")
        };
        let maximum = u32::from(suitability.maximum());
        let desired_density = if maximum < 80 {
            0
        } else {
            u16::try_from((130 + maximum * 32 / 100 + mean_productivity * 16 / 100).min(680))
                .expect("density is bounded to permille")
        };
        let occupancy_roll = hash_permille(self.source_hash(cell, 0, HASH_LANE_OCCUPANCY));
        let total_weight = suitability.total();
        let species_roll = if total_weight == 0 {
            0
        } else {
            (self.source_hash(cell, 0, HASH_LANE_SPECIES) % u64::from(total_weight)) as u32
        };
        let selected_species = (occupancy_roll < desired_density && total_weight > 0)
            .then(|| select_species(suitability, species_roll))
            .flatten();
        let selected_index = selected_species
            .map(|species| best_candidate[species as usize])
            .unwrap_or(4);
        let selected_habitat = samples[selected_index];
        let encounter = selected_species.map(|species| {
            let (minimum, maximum) = species.group_size_range();
            let width = maximum - minimum + 1;
            let group_size = minimum
                + (self.source_hash(cell, species as u8, HASH_LANE_GROUP_SIZE) % u64::from(width))
                    as u8;
            WildlifeEncounter {
                species,
                group_size,
                anchor_x: selected_habitat.world_x,
                anchor_z: selected_habitat.world_z,
                owner_chunk: ChunkPos::from_block_coords(
                    selected_habitat.world_x,
                    selected_habitat.world_z,
                ),
            }
        });

        Ok(WildlifeCellPlan {
            cell,
            representative,
            selected_habitat,
            suitability,
            desired_density,
            occupancy_roll,
            species_roll,
            encounter,
        })
    }

    fn sample_habitat(&self, world_x: i32, world_z: i32) -> WildlifeHabitatSample {
        match &self.adapter {
            WildlifeHabitatAdapter::V1 {
                terrain,
                vegetation,
            } => sample_v1(terrain, vegetation, world_x, world_z),
            WildlifeHabitatAdapter::V2(surface) => sample_v2(surface, world_x, world_z),
            WildlifeHabitatAdapter::V3(terrain) => sample_v3(terrain, world_x, world_z),
            WildlifeHabitatAdapter::PublishedBlocks => {
                WildlifeHabitatSample::unavailable(world_x, world_z, self.source)
            }
        }
    }

    fn canonical_cell(
        &self,
        requested: WildlifePopulationCell,
    ) -> Result<WildlifePopulationCell, WildlifePlanError> {
        let origin = BlockPos::new(requested.min_block_x()?, 0, requested.min_block_z()?);
        self.topology
            .canonicalize_block(origin)
            .map(|origin| WildlifePopulationCell::from_block(origin.x, origin.z))
            .ok_or(WildlifePlanError::OutsideTopology)
    }

    fn source_hash(&self, cell: WildlifePopulationCell, candidate: u8, lane: u64) -> u64 {
        let mut value = self.seed as u64 ^ lane;
        value = splitmix64(value ^ topology_scope(self.topology));
        value = splitmix64(value ^ u64::from(WILDLIFE_POPULATION_REVISION));
        value = splitmix64(value ^ cell.x as u64);
        value = splitmix64(value ^ (cell.z as u64).rotate_left(32));
        splitmix64(value ^ u64::from(candidate))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WildlifePlanError {
    CoordinateOverflow,
    OutsideTopology,
    UnsupportedTopology(WildlifeHabitatSource),
    AdapterConstruction(WildlifeHabitatSource),
    ExternalEvidenceRequired,
    EvidenceCount { expected: usize, actual: usize },
    MismatchedEvidence(u8),
}

impl fmt::Display for WildlifePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateOverflow => write!(formatter, "wildlife plan coordinate overflow"),
            Self::OutsideTopology => write!(formatter, "wildlife cell is outside the topology"),
            Self::UnsupportedTopology(source) => write!(
                formatter,
                "{} wildlife habitat does not support this topology",
                source.label()
            ),
            Self::AdapterConstruction(source) => {
                write!(formatter, "failed to construct {} habitat", source.label())
            }
            Self::ExternalEvidenceRequired => {
                write!(formatter, "published-block wildlife evidence is required")
            }
            Self::EvidenceCount { expected, actual } => write!(
                formatter,
                "wildlife plan needs {expected} evidence samples, received {actual}"
            ),
            Self::MismatchedEvidence(slot) => {
                write!(
                    formatter,
                    "wildlife evidence does not match candidate {slot}"
                )
            }
        }
    }
}

impl Error for WildlifePlanError {}

pub type McloneWildlifePopulationCell = WildlifePopulationCell;
pub type McloneWildlifeSpecies = WildlifeSpecies;
pub type McloneWildlifeSuitability = WildlifeSuitability;
pub type McloneWildlifeHabitatSample = WildlifeHabitatSample;
pub type McloneWildlifeEncounter = WildlifeEncounter;
pub type McloneWildlifeCellPlan = WildlifeCellPlan;
pub type McloneWildlifePlanError = WildlifePlanError;
pub type McloneOverworldWildlifePlanner = WildlifePopulationPlanner;

fn sample_v1(
    terrain: &McloneOverworldSampler,
    vegetation: &McloneOverworldVegetationPlanner,
    world_x: i32,
    world_z: i32,
) -> WildlifeHabitatSample {
    let landform = terrain.sample_landform(world_x, world_z);
    let terrain = landform.terrain;
    let forest = vegetation.forest_intent(landform, world_x, world_z);
    let is_water = terrain.watercourse.is_water()
        || (terrain.continentalness <= 0.0 && terrain.surface_y < MCLONE_OVERWORLD_SEA_LEVEL);
    let land = if !is_water && terrain.surface_y >= MCLONE_OVERWORLD_SEA_LEVEL - 1 {
        1.0
    } else {
        0.0
    };
    let moisture = (terrain.climate.moisture * 0.5 + 0.5).clamp(0.0, 1.0);
    let temperature = terrain
        .climate
        .altitude_adjusted_temperature(terrain.surface_y);
    let temperature_comfort = (1.0 - temperature.abs() * 0.72).clamp(0.0, 1.0);
    let slope_comfort = (1.0 - landform.slope / 1.15).clamp(0.0, 1.0);
    let exposure_comfort = (1.0 - terrain.exposure() * 0.85).clamp(0.0, 1.0);
    let productivity = ((0.28 + moisture * 0.45 + temperature_comfort * 0.27)
        * (0.55 + slope_comfort * 0.30 + exposure_comfort * 0.15))
        .clamp(0.0, 1.0);
    let forest_cover = f64::from(forest.coverage).clamp(0.0, 1.0);
    let openness = ((1.0 - forest_cover * 0.82) * slope_comfort).clamp(0.0, 1.0);
    let forest_edge = (1.0 - ((forest_cover - 0.48).abs() / 0.48)).clamp(0.0, 1.0);
    let wetland = terrain
        .watercourse
        .wetland_influence
        .max(terrain.watercourse.bank_influence * 0.86)
        .max(terrain.watercourse.channel_influence * 0.78)
        .clamp(0.0, 1.0);
    let inland_water = if terrain.watercourse.is_water() {
        1.0
    } else {
        0.0
    };
    let bank = terrain.watercourse.bank_influence.clamp(0.0, 1.0);
    let flowering = productivity * (0.48 + openness * 0.34 + forest_edge * 0.18);
    WildlifeHabitatSample {
        world_x,
        world_z,
        surface_y: terrain.surface_y,
        source: WildlifeHabitatSource::McloneOverworldV1,
        supported: true,
        available_evidence: WildlifeEvidenceSet::ALL,
        land: to_permille(land),
        productivity: to_permille(productivity),
        openness: to_permille(openness),
        low_cover: to_permille(openness * (0.55 + productivity * 0.45)),
        forest_cover: to_permille(forest_cover),
        forest_edge: to_permille(forest_edge),
        wetland: to_permille(wetland),
        water: to_permille(if is_water { 1.0 } else { 0.0 }),
        inland_water: to_permille(inland_water),
        shore: to_permille(bank.max(wetland * 0.72)),
        bank: to_permille(bank),
        flowering: to_permille(flowering),
        seeds_and_soft_mast: to_permille(productivity * (0.62 + openness * 0.38)),
        mature_trees: to_permille(forest_cover * (0.58 + productivity * 0.42)),
    }
}

fn sample_v2(
    surface: &ContinentalSurfacePlan,
    world_x: i32,
    world_z: i32,
) -> WildlifeHabitatSample {
    let sample = surface.query_point(world_x, world_z).sample;
    let is_water = sample.is_water();
    let supported_land = matches!(
        sample.substrate,
        ContinentalSurfaceSubstrate::Grass | ContinentalSurfaceSubstrate::CoarseSoil
    );
    let land = if !is_water && supported_land {
        1.0
    } else {
        0.0
    };
    let openness = f64::from(sample.openness).clamp(0.0, 1.0);
    let forest_cover = f64::from(sample.forest_core).clamp(0.0, 1.0);
    let forest_edge = f64::from(sample.forest_edge).clamp(0.0, 1.0);
    let moisture = f64::from(sample.moisture).clamp(0.0, 1.0);
    let temperature_comfort = (1.0 - f64::from(sample.temperature).abs() * 0.68).clamp(0.0, 1.0);
    let productivity = (0.22 + moisture * 0.48 + temperature_comfort * 0.30)
        * (0.62 + f64::from(sample.clearing).clamp(0.0, 1.0) * 0.12 + forest_edge * 0.26);
    let wetland = f64::from(sample.wetland)
        .max(f64::from(sample.floodplain) * 0.78)
        .max(f64::from(sample.riparian) * 0.84)
        .clamp(0.0, 1.0);
    let inland_water = if matches!(
        sample.water_kind,
        ContinentalSurfaceWaterKind::Lake
            | ContinentalSurfaceWaterKind::River
            | ContinentalSurfaceWaterKind::WetlandPool
    ) {
        1.0
    } else {
        0.0
    };
    let shore = if sample.shore_intent as u8 != 0 {
        1.0
    } else {
        wetland * 0.64
    };
    let flowering = productivity * (openness * 0.48 + forest_edge * 0.32 + wetland * 0.20);
    WildlifeHabitatSample {
        world_x,
        world_z,
        surface_y: sample.solid_surface_y.floor() as i32,
        source: WildlifeHabitatSource::McloneOverworldV2,
        supported: true,
        available_evidence: WildlifeEvidenceSet::ALL,
        land: to_permille(land),
        productivity: to_permille(productivity),
        openness: to_permille(openness),
        low_cover: to_permille((openness * 0.66 + f64::from(sample.clearing) * 0.34).min(1.0)),
        forest_cover: to_permille(forest_cover),
        forest_edge: to_permille(forest_edge),
        wetland: to_permille(wetland),
        water: to_permille(if is_water { 1.0 } else { 0.0 }),
        inland_water: to_permille(inland_water),
        shore: to_permille(shore),
        bank: to_permille(f64::from(sample.riparian).max(wetland * 0.68)),
        flowering: to_permille(flowering),
        seeds_and_soft_mast: to_permille(
            productivity * (0.55 + openness * 0.25 + forest_edge * 0.20),
        ),
        mature_trees: to_permille(forest_cover * (0.62 + productivity * 0.38)),
    }
}

fn sample_v3(
    terrain: &McloneOverworldV3TerrainPlan,
    world_x: i32,
    world_z: i32,
) -> WildlifeHabitatSample {
    let sample = terrain.query_point(world_x, world_z).sample;
    let is_water = sample.is_water();
    let supported_land = matches!(
        sample.substrate,
        V3SurfaceSubstrate::Grass | V3SurfaceSubstrate::CoarseSoil
    );
    let land = if !is_water && supported_land {
        1.0
    } else {
        0.0
    };
    let openness = f64::from(sample.openness).clamp(0.0, 1.0);
    let forest = f64::from(sample.forest_opportunity).clamp(0.0, 1.0);
    let moisture = f64::from(sample.moisture).clamp(0.0, 1.0);
    let gentle =
        (1.0 - f64::from(sample.range_strength) * 0.62 - f64::from(sample.escarpment) * 0.72)
            .clamp(0.0, 1.0);
    let productivity =
        (0.30 + moisture * 0.48 + f64::from(sample.clearing) * 0.22) * (0.62 + gentle * 0.38);
    let forest_edge = (forest * (1.0 - forest) * 4.0).clamp(0.0, 1.0);
    let basin = f64::from(sample.basin).clamp(0.0, 1.0);
    let inland_water = if sample.water_kind == V3WaterKind::BasinLake {
        1.0
    } else {
        0.0
    };
    let shore = if !is_water && basin > 0.54 {
        basin
    } else {
        0.0
    };
    WildlifeHabitatSample {
        world_x,
        world_z,
        surface_y: sample.solid_surface_y.floor() as i32,
        source: WildlifeHabitatSource::McloneOverworldV3,
        supported: true,
        available_evidence: WildlifeEvidenceSet::LAND
            .union(WildlifeEvidenceSet::PRODUCTIVITY)
            .union(WildlifeEvidenceSet::OPENNESS)
            .union(WildlifeEvidenceSet::LOW_COVER)
            .union(WildlifeEvidenceSet::FOREST_COVER)
            .union(WildlifeEvidenceSet::FOREST_EDGE)
            .union(WildlifeEvidenceSet::WATER)
            .union(WildlifeEvidenceSet::INLAND_WATER)
            .union(WildlifeEvidenceSet::SHORE)
            .union(WildlifeEvidenceSet::MATURE_TREES),
        land: to_permille(land),
        productivity: to_permille(productivity),
        openness: to_permille(openness),
        low_cover: to_permille(openness * (0.58 + f64::from(sample.clearing) * 0.42)),
        forest_cover: to_permille(forest),
        forest_edge: to_permille(forest_edge),
        wetland: 0,
        water: to_permille(if is_water { 1.0 } else { 0.0 }),
        inland_water: to_permille(inland_water),
        shore: to_permille(shore),
        bank: 0,
        // V3 does not yet publish flowers or seed-bearing ground cover. Its
        // adapter stays conservative until it does.
        flowering: 0,
        seeds_and_soft_mast: 0,
        mature_trees: to_permille(forest),
    }
}

fn score_habitat(sample: WildlifeHabitatSample) -> WildlifeSuitability {
    if !sample.supported {
        return WildlifeSuitability::default();
    }
    let land = unit(sample.land);
    let productivity = unit(sample.productivity);
    let openness = unit(sample.openness);
    let low_cover = unit(sample.low_cover);
    let forest = unit(sample.forest_cover);
    let edge = unit(sample.forest_edge);
    let wetland = unit(sample.wetland);
    let inland_water = unit(sample.inland_water);
    let shore = unit(sample.shore);
    let bank = unit(sample.bank);
    let flowering = unit(sample.flowering);
    let seeds = unit(sample.seeds_and_soft_mast);
    let mature_trees = unit(sample.mature_trees);
    let available = sample.available_evidence;
    let general_land = WildlifeEvidenceSet::LAND
        .union(WildlifeEvidenceSet::PRODUCTIVITY)
        .union(WildlifeEvidenceSet::OPENNESS);
    let rabbit = available.contains(general_land.union(WildlifeEvidenceSet::LOW_COVER));
    let deer = available.contains(
        WildlifeEvidenceSet::LAND
            .union(WildlifeEvidenceSet::PRODUCTIVITY)
            .union(WildlifeEvidenceSet::FOREST_COVER)
            .union(WildlifeEvidenceSet::FOREST_EDGE),
    );
    let mallard = available.contains(WildlifeEvidenceSet::PRODUCTIVITY)
        && (available.contains(WildlifeEvidenceSet::INLAND_WATER)
            || available.contains(WildlifeEvidenceSet::WETLAND)
            || available.contains(WildlifeEvidenceSet::SHORE)
            || available.contains(WildlifeEvidenceSet::BANK));
    let bee = available.contains(
        WildlifeEvidenceSet::LAND
            .union(WildlifeEvidenceSet::FLOWERING)
            .union(WildlifeEvidenceSet::OPENNESS)
            .union(WildlifeEvidenceSet::FOREST_EDGE),
    );
    let squirrel = available.contains(
        WildlifeEvidenceSet::LAND
            .union(WildlifeEvidenceSet::MATURE_TREES)
            .union(WildlifeEvidenceSet::FOREST_COVER)
            .union(WildlifeEvidenceSet::FOREST_EDGE),
    );
    let cow = available.contains(general_land.union(WildlifeEvidenceSet::LOW_COVER));
    let chicken = available.contains(
        general_land
            .union(WildlifeEvidenceSet::LOW_COVER)
            .union(WildlifeEvidenceSet::SEEDS_AND_SOFT_MAST),
    );
    WildlifeSuitability {
        rabbit: rabbit
            .then(|| {
                to_permille(
                    land * (0.18 + productivity * 0.38 + openness * 0.24 + low_cover * 0.20),
                )
            })
            .unwrap_or(0),
        deer: deer
            .then(|| {
                to_permille(
                    land * productivity
                        * (0.34 + edge * 0.32 + forest * 0.22 + openness * 0.12)
                        * 0.78,
                )
            })
            .unwrap_or(0),
        mallard: mallard
            .then(|| {
                to_permille(
                    inland_water
                        .max(wetland * 0.88)
                        .max(shore * 0.74)
                        .max(bank * 0.66)
                        * (0.70 + productivity * 0.30)
                        * 0.72,
                )
            })
            .unwrap_or(0),
        bee: bee
            .then(|| to_permille(land * flowering * (0.62 + openness * 0.22 + edge * 0.16) * 0.56))
            .unwrap_or(0),
        squirrel: squirrel
            .then(|| to_permille(land * mature_trees * (0.42 + forest * 0.34 + edge * 0.24) * 0.70))
            .unwrap_or(0),
        cow: cow
            .then(|| to_permille(land * productivity * openness * (0.72 + low_cover * 0.28) * 0.62))
            .unwrap_or(0),
        chicken: chicken
            .then(|| to_permille(land * productivity * (seeds * 0.72 + low_cover * 0.28) * 0.58))
            .unwrap_or(0),
    }
}

fn v1_topology(topology: HorizontalTopology) -> Option<McloneOverworldSamplingTopology> {
    if topology == HorizontalTopology::UNBOUNDED {
        return Some(McloneOverworldSamplingTopology::Unbounded);
    }
    match (topology.x, topology.z) {
        (
            AxisTopology::Periodic {
                minimum_chunk: 0,
                period_chunks,
            },
            AxisTopology::Unbounded,
        ) if period_chunks == MCLONE_OVERWORLD_PERIOD_CHUNKS as u32 => {
            Some(McloneOverworldSamplingTopology::PeriodicX)
        }
        _ => None,
    }
}

fn checked_cell_origin(cell_coordinate: i32) -> Result<i32, WildlifePlanError> {
    i32::try_from(i64::from(cell_coordinate) * i64::from(WILDLIFE_POPULATION_CELL_BLOCKS))
        .map_err(|_| WildlifePlanError::CoordinateOverflow)
}

fn select_species(suitability: WildlifeSuitability, mut roll: u32) -> Option<WildlifeSpecies> {
    for species in WildlifeSpecies::ALL {
        let weight = u32::from(suitability.for_species(species));
        if roll < weight {
            return Some(species);
        }
        roll -= weight;
    }
    None
}

fn unit(value: u16) -> f64 {
    f64::from(value) / f64::from(PERMILLE)
}

fn to_permille(value: f64) -> u16 {
    (value.clamp(0.0, 1.0) * f64::from(PERMILLE)).round() as u16
}

fn hash_permille(value: u64) -> u16 {
    (value % u64::from(PERMILLE)) as u16
}

fn hash_bounded(value: u64, bound: i32) -> i32 {
    (value % u64::try_from(bound).expect("positive wildlife candidate bound")) as i32
}

fn topology_scope(topology: HorizontalTopology) -> u64 {
    fn axis_scope(axis: AxisTopology) -> u64 {
        match axis {
            AxisTopology::Unbounded => 0x756e_626f_756e_6464,
            AxisTopology::Finite {
                minimum_chunk,
                maximum_chunk_exclusive,
            } => splitmix64(
                0x6669_6e69_7465_3031
                    ^ minimum_chunk as u64
                    ^ (maximum_chunk_exclusive as u64).rotate_left(32),
            ),
            AxisTopology::Periodic {
                minimum_chunk,
                period_chunks,
            } => splitmix64(
                0x7065_7269_6f64_3031
                    ^ minimum_chunk as u64
                    ^ u64::from(period_chunks).rotate_left(32),
            ),
        }
    }
    splitmix64(axis_scope(topology.x) ^ axis_scope(topology.z).rotate_left(17))
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const SEED: i64 = -98_765;

    #[test]
    fn plans_are_stable_across_query_order_for_every_direct_adapter() {
        let cells = [
            WildlifePopulationCell { x: -8, z: 3 },
            WildlifePopulationCell { x: 0, z: 0 },
            WildlifePopulationCell { x: 12, z: -7 },
            WildlifePopulationCell { x: -1, z: -1 },
        ];
        for source in [
            WildlifeHabitatSource::McloneOverworldV1,
            WildlifeHabitatSource::McloneOverworldV2,
            WildlifeHabitatSource::McloneOverworldV3,
        ] {
            let planner =
                WildlifePopulationPlanner::for_habitat(SEED, HorizontalTopology::UNBOUNDED, source)
                    .unwrap();
            let forward = cells
                .into_iter()
                .map(|cell| (cell, planner.plan_cell(cell).unwrap()))
                .collect::<BTreeMap<_, _>>();
            let reverse = cells
                .into_iter()
                .rev()
                .map(|cell| (cell, planner.plan_cell(cell).unwrap()))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(forward, reverse, "{}", source.label());
        }
    }

    #[test]
    fn negative_chunk_edges_use_euclidean_cells() {
        assert_eq!(
            WildlifePopulationCell::from_chunk(ChunkPos::new(-1, -1)),
            WildlifePopulationCell { x: -1, z: -1 }
        );
        assert_eq!(
            WildlifePopulationCell::from_chunk(ChunkPos::new(-4, -4)),
            WildlifePopulationCell { x: -1, z: -1 }
        );
        assert_eq!(
            WildlifePopulationCell::from_chunk(ChunkPos::new(-5, -5)),
            WildlifePopulationCell { x: -2, z: -2 }
        );
    }

    #[test]
    fn encounters_belong_to_one_member_chunk_for_each_direct_adapter() {
        for source in [
            WildlifeHabitatSource::McloneOverworldV1,
            WildlifeHabitatSource::McloneOverworldV2,
            WildlifeHabitatSource::McloneOverworldV3,
        ] {
            let planner = WildlifePopulationPlanner::for_habitat(
                12_345,
                HorizontalTopology::UNBOUNDED,
                source,
            )
            .unwrap();
            for cell_x in -4..4 {
                for cell_z in -4..4 {
                    let cell = WildlifePopulationCell {
                        x: cell_x,
                        z: cell_z,
                    };
                    let plan = planner.plan_cell(cell).unwrap();
                    let Some(encounter) = plan.encounter else {
                        continue;
                    };
                    assert_eq!(
                        WildlifePopulationCell::from_chunk(encounter.owner_chunk),
                        cell
                    );
                    let owners = (0..WILDLIFE_POPULATION_CELL_CHUNKS)
                        .flat_map(|local_x| {
                            (0..WILDLIFE_POPULATION_CELL_CHUNKS).map(move |local_z| {
                                ChunkPos::new(
                                    cell_x * WILDLIFE_POPULATION_CELL_CHUNKS + local_x,
                                    cell_z * WILDLIFE_POPULATION_CELL_CHUNKS + local_z,
                                )
                            })
                        })
                        .filter(|chunk| plan.encounter_for_chunk(*chunk).is_some())
                        .count();
                    assert_eq!(owners, 1);
                }
            }
        }
    }

    #[test]
    fn direct_adapter_receipts_are_pinned() {
        let receipts = [
            WildlifeHabitatSource::McloneOverworldV1,
            WildlifeHabitatSource::McloneOverworldV2,
            WildlifeHabitatSource::McloneOverworldV3,
        ]
        .map(survey_receipt);
        assert_eq!(
            receipts,
            [
                (54, 155, [18, 6, 2, 5, 0, 10, 13], 60_684),
                (103, 295, [31, 20, 0, 10, 20, 8, 14], 99_636),
                (117, 350, [63, 30, 0, 0, 3, 21, 0], 109_907),
            ]
        );
    }

    #[test]
    fn zero_evidence_never_falls_back_to_random_spawning() {
        let planner = WildlifePopulationPlanner::for_habitat(
            SEED,
            HorizontalTopology::UNBOUNDED,
            WildlifeHabitatSource::PublishedBlocks,
        )
        .unwrap();
        let cell = WildlifePopulationCell { x: 0, z: 0 };
        let samples = planner
            .candidate_sites(cell)
            .unwrap()
            .into_iter()
            .map(|site| {
                WildlifeHabitatSample::unavailable(
                    site.world_x,
                    site.world_z,
                    WildlifeHabitatSource::PublishedBlocks,
                )
            })
            .collect::<Vec<_>>();
        let plan = planner.plan_cell_with_samples(cell, &samples).unwrap();
        assert_eq!(plan.suitability, WildlifeSuitability::default());
        assert_eq!(plan.desired_density, 0);
        assert_eq!(plan.encounter, None);
    }

    #[test]
    fn roster_requires_truthful_specialist_evidence() {
        let rich = WildlifeHabitatSample {
            world_x: 0,
            world_z: 0,
            surface_y: 64,
            source: WildlifeHabitatSource::PublishedBlocks,
            supported: true,
            available_evidence: WildlifeEvidenceSet::ALL,
            land: 1_000,
            productivity: 800,
            openness: 650,
            low_cover: 700,
            forest_cover: 650,
            forest_edge: 700,
            wetland: 600,
            water: 0,
            inland_water: 700,
            shore: 800,
            bank: 800,
            flowering: 750,
            seeds_and_soft_mast: 750,
            mature_trees: 800,
        };
        let suitability = score_habitat(rich);
        assert!(
            WildlifeSpecies::ALL
                .into_iter()
                .all(|species| suitability.for_species(species) > 0)
        );

        let no_specialist_facts = WildlifeHabitatSample {
            available_evidence: WildlifeEvidenceSet::LAND
                .union(WildlifeEvidenceSet::PRODUCTIVITY)
                .union(WildlifeEvidenceSet::OPENNESS)
                .union(WildlifeEvidenceSet::LOW_COVER),
            ..rich
        };
        let suitability = score_habitat(no_specialist_facts);
        assert!(suitability.rabbit > 0);
        assert!(suitability.cow > 0);
        assert_eq!(suitability.mallard, 0);
        assert_eq!(suitability.bee, 0);
        assert_eq!(suitability.squirrel, 0);
        assert_eq!(suitability.chicken, 0);

        let ocean_only = WildlifeHabitatSample {
            land: 0,
            water: 1_000,
            inland_water: 0,
            wetland: 0,
            shore: 0,
            bank: 0,
            ..rich
        };
        assert_eq!(score_habitat(ocean_only).mallard, 0);
    }

    #[test]
    fn periodic_v1_repeats_population_cells() {
        let planner =
            WildlifePopulationPlanner::new(12_345, McloneOverworldSamplingTopology::PeriodicX);
        let period_cells = i32::try_from(MCLONE_OVERWORLD_PERIOD_CHUNKS)
            .expect("V1 period fits i32")
            / WILDLIFE_POPULATION_CELL_CHUNKS;
        let left = planner
            .plan_cell(WildlifePopulationCell { x: 7, z: -3 })
            .unwrap();
        let right = planner
            .plan_cell(WildlifePopulationCell {
                x: 7 + period_cells,
                z: -3,
            })
            .unwrap();
        assert_eq!(left, right);
    }

    fn survey_receipt(source: WildlifeHabitatSource) -> (u32, u32, [u32; 7], u64) {
        let planner =
            WildlifePopulationPlanner::for_habitat(SEED, HorizontalTopology::UNBOUNDED, source)
                .unwrap();
        let mut occupied = 0_u32;
        let mut animals = 0_u32;
        let mut counts = [0_u32; 7];
        let mut density_sum = 0_u64;
        for x in -8..8 {
            for z in -8..8 {
                let plan = planner.plan_cell(WildlifePopulationCell { x, z }).unwrap();
                density_sum += u64::from(plan.desired_density);
                if let Some(encounter) = plan.encounter {
                    occupied += 1;
                    animals += u32::from(encounter.group_size);
                    counts[encounter.species as usize] += 1;
                }
            }
        }
        (occupied, animals, counts, density_sum)
    }
}
