use std::{error::Error, fmt};

use mclone_core::{CHUNK_WIDTH, ChunkPos};

use super::mclone_overworld::{
    MCLONE_OVERWORLD_PERIOD_BLOCKS, MCLONE_OVERWORLD_SEA_LEVEL, McloneOverworldBiomeRecipe,
    McloneOverworldLandformKind, McloneOverworldSampler, McloneOverworldSamplingTopology,
    McloneOverworldVegetationPlanner, McloneVegetationSource, mclone_overworld_biome_recipe,
    mclone_overworld_landform_kind,
};

pub const MCLONE_WILDLIFE_POPULATION_REVISION: u16 = 2;
pub const MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS: i32 = CHUNK_WIDTH * 4;
pub const MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS: i32 =
    MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS / CHUNK_WIDTH;
pub const MCLONE_WILDLIFE_CANDIDATES_PER_CELL: usize = 9;

const PERMILLE: u32 = 1_000;
const HASH_LANE_POSITION_X: u64 = 0x7769_6c64_5f78_3031;
const HASH_LANE_POSITION_Z: u64 = 0x7769_6c64_5f7a_3031;
const HASH_LANE_OCCUPANCY: u64 = 0x7769_6c64_5f6f_3031;
const HASH_LANE_SPECIES: u64 = 0x7769_6c64_5f73_3031;
const HASH_LANE_GROUP_SIZE: u64 = 0x7769_6c64_5f67_3031;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct McloneWildlifePopulationCell {
    pub x: i32,
    pub z: i32,
}

impl McloneWildlifePopulationCell {
    pub fn from_chunk(chunk: ChunkPos) -> Self {
        Self {
            x: chunk.x.div_euclid(MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS),
            z: chunk.z.div_euclid(MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS),
        }
    }

    pub fn from_block(world_x: i32, world_z: i32) -> Self {
        Self {
            x: world_x.div_euclid(MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS),
            z: world_z.div_euclid(MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS),
        }
    }

    pub fn min_block_x(self) -> Result<i32, McloneWildlifePlanError> {
        checked_cell_origin(self.x)
    }

    pub fn min_block_z(self) -> Result<i32, McloneWildlifePlanError> {
        checked_cell_origin(self.z)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum McloneWildlifeSpecies {
    Rabbit = 0,
    Deer = 1,
    Mallard = 2,
    Bee = 3,
    Squirrel = 4,
}

impl McloneWildlifeSpecies {
    pub const ALL: [Self; 5] = [
        Self::Rabbit,
        Self::Deer,
        Self::Mallard,
        Self::Bee,
        Self::Squirrel,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Rabbit => "rabbit",
            Self::Deer => "deer",
            Self::Mallard => "mallard",
            Self::Bee => "bee",
            Self::Squirrel => "squirrel",
        }
    }

    pub const fn group_size_range(self) -> (u8, u8) {
        match self {
            Self::Rabbit => (2, 4),
            Self::Deer => (2, 3),
            Self::Mallard => (2, 4),
            Self::Bee => (2, 3),
            Self::Squirrel => (2, 4),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McloneWildlifeSuitability {
    pub rabbit: u16,
    pub deer: u16,
    pub mallard: u16,
    pub bee: u16,
    pub squirrel: u16,
}

impl McloneWildlifeSuitability {
    pub const fn for_species(self, species: McloneWildlifeSpecies) -> u16 {
        match species {
            McloneWildlifeSpecies::Rabbit => self.rabbit,
            McloneWildlifeSpecies::Deer => self.deer,
            McloneWildlifeSpecies::Mallard => self.mallard,
            McloneWildlifeSpecies::Bee => self.bee,
            McloneWildlifeSpecies::Squirrel => self.squirrel,
        }
    }

    pub fn total(self) -> u32 {
        McloneWildlifeSpecies::ALL
            .into_iter()
            .map(|species| u32::from(self.for_species(species)))
            .sum()
    }

    pub fn maximum(self) -> u16 {
        McloneWildlifeSpecies::ALL
            .into_iter()
            .map(|species| self.for_species(species))
            .max()
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneWildlifeHabitatSample {
    pub world_x: i32,
    pub world_z: i32,
    pub surface_y: i32,
    pub biome: McloneOverworldBiomeRecipe,
    pub landform: McloneOverworldLandformKind,
    pub land: u16,
    pub productivity: u16,
    pub openness: u16,
    pub forest_cover: u16,
    pub wetland: u16,
    pub water: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneWildlifeEncounter {
    pub species: McloneWildlifeSpecies,
    pub group_size: u8,
    pub anchor_x: i32,
    pub anchor_z: i32,
    pub owner_chunk: ChunkPos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McloneWildlifeCellPlan {
    pub cell: McloneWildlifePopulationCell,
    pub representative: McloneWildlifeHabitatSample,
    pub selected_habitat: McloneWildlifeHabitatSample,
    pub suitability: McloneWildlifeSuitability,
    pub desired_density: u16,
    pub occupancy_roll: u16,
    pub species_roll: u32,
    pub encounter: Option<McloneWildlifeEncounter>,
}

impl McloneWildlifeCellPlan {
    pub fn encounter_for_chunk(self, chunk: ChunkPos) -> Option<McloneWildlifeEncounter> {
        self.encounter
            .filter(|encounter| encounter.owner_chunk == chunk)
    }
}

#[derive(Clone, Debug)]
pub struct McloneOverworldWildlifePlanner {
    seed: i64,
    topology: McloneOverworldSamplingTopology,
    terrain: McloneOverworldSampler,
    vegetation: McloneOverworldVegetationPlanner,
}

impl McloneOverworldWildlifePlanner {
    pub fn new(seed: i64, topology: McloneOverworldSamplingTopology) -> Self {
        Self {
            seed,
            topology,
            terrain: McloneOverworldSampler::new_with_topology(seed, topology),
            vegetation: McloneOverworldVegetationPlanner::new(McloneVegetationSource::new(
                seed, topology,
            )),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn topology(&self) -> McloneOverworldSamplingTopology {
        self.topology
    }

    pub fn plan_for_chunk(
        &self,
        chunk: ChunkPos,
    ) -> Result<McloneWildlifeCellPlan, McloneWildlifePlanError> {
        self.plan_cell(McloneWildlifePopulationCell::from_chunk(chunk))
    }

    pub fn plan_cell(
        &self,
        requested_cell: McloneWildlifePopulationCell,
    ) -> Result<McloneWildlifeCellPlan, McloneWildlifePlanError> {
        let cell = canonical_cell(requested_cell, self.topology);
        let origin_x = cell.min_block_x()?;
        let origin_z = cell.min_block_z()?;
        let mut samples = Vec::with_capacity(MCLONE_WILDLIFE_CANDIDATES_PER_CELL);
        let mut sample_weights = Vec::with_capacity(MCLONE_WILDLIFE_CANDIDATES_PER_CELL);

        for candidate in 0..MCLONE_WILDLIFE_CANDIDATES_PER_CELL {
            let column = i32::try_from(candidate % 3).expect("candidate column fits i32");
            let row = i32::try_from(candidate / 3).expect("candidate row fits i32");
            let stratum_min_x = origin_x
                .checked_add(column * (MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS / 3))
                .ok_or(McloneWildlifePlanError::CoordinateOverflow)?;
            let stratum_min_z = origin_z
                .checked_add(row * (MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS / 3))
                .ok_or(McloneWildlifePlanError::CoordinateOverflow)?;
            let stratum_width = if column == 2 { 22 } else { 21 };
            let stratum_depth = if row == 2 { 22 } else { 21 };
            let world_x = stratum_min_x
                .checked_add(hash_bounded(
                    self.source_hash(cell, candidate as u8, HASH_LANE_POSITION_X),
                    stratum_width,
                ))
                .ok_or(McloneWildlifePlanError::CoordinateOverflow)?;
            let world_z = stratum_min_z
                .checked_add(hash_bounded(
                    self.source_hash(cell, candidate as u8, HASH_LANE_POSITION_Z),
                    stratum_depth,
                ))
                .ok_or(McloneWildlifePlanError::CoordinateOverflow)?;
            let (sample, weights) = self.sample_habitat(world_x, world_z);
            samples.push(sample);
            sample_weights.push(weights);
        }

        let representative = samples[4];
        let mut suitability = McloneWildlifeSuitability::default();
        let mut best_candidate = [0_usize; 5];
        for species in McloneWildlifeSpecies::ALL {
            let (index, weight) = sample_weights
                .iter()
                .enumerate()
                .map(|(index, weights)| (index, weights.for_species(species)))
                .max_by_key(|(index, weight)| (*weight, std::cmp::Reverse(*index)))
                .expect("wildlife cells always have candidates");
            best_candidate[species as usize] = index;
            match species {
                McloneWildlifeSpecies::Rabbit => suitability.rabbit = weight,
                McloneWildlifeSpecies::Deer => suitability.deer = weight,
                McloneWildlifeSpecies::Mallard => suitability.mallard = weight,
                McloneWildlifeSpecies::Bee => suitability.bee = weight,
                McloneWildlifeSpecies::Squirrel => suitability.squirrel = weight,
            }
        }

        let mean_productivity = u32::try_from(
            samples
                .iter()
                .map(|sample| u32::from(sample.productivity))
                .sum::<u32>()
                / u32::try_from(samples.len()).expect("candidate count fits u32"),
        )
        .expect("mean productivity fits u32");
        let maximum = u32::from(suitability.maximum());
        let desired_density = if maximum < 80 {
            0
        } else {
            u16::try_from((150 + maximum * 35 / 100 + mean_productivity * 18 / 100).min(720))
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
            McloneWildlifeEncounter {
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

        Ok(McloneWildlifeCellPlan {
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

    fn sample_habitat(
        &self,
        world_x: i32,
        world_z: i32,
    ) -> (McloneWildlifeHabitatSample, McloneWildlifeSuitability) {
        let landform = self.terrain.sample_landform(world_x, world_z);
        let terrain = landform.terrain;
        let forest = self.vegetation.forest_intent(landform, world_x, world_z);
        let biome = mclone_overworld_biome_recipe(landform);
        let landform_kind = mclone_overworld_landform_kind(landform);
        let is_water = terrain.watercourse.is_water()
            || (terrain.continentalness <= 0.0 && terrain.surface_y < MCLONE_OVERWORLD_SEA_LEVEL);
        let land = if !is_water && terrain.surface_y >= MCLONE_OVERWORLD_SEA_LEVEL - 1 {
            1.0
        } else {
            0.0
        };
        let moisture = (terrain.climate.moisture * 0.5 + 0.5).clamp(0.0, 1.0);
        let adjusted_temperature = terrain
            .climate
            .altitude_adjusted_temperature(terrain.surface_y);
        let temperature_comfort = (1.0 - adjusted_temperature.abs() * 0.72).clamp(0.0, 1.0);
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
        let water: f64 = if is_water { 1.0 } else { 0.0 };
        let rabbit =
            land * (0.24 + productivity * 0.46 + openness * 0.30) * (0.72 + slope_comfort * 0.28);
        let deer = land
            * productivity
            * (0.42 + forest_edge * 0.38 + forest_cover * 0.20)
            * slope_comfort
            * 0.72;
        let inland_water = if terrain.watercourse.is_water() {
            1.0_f64
        } else {
            0.0_f64
        };
        let mallard = inland_water
            .max(wetland)
            .max(terrain.watercourse.bank_influence.clamp(0.0, 1.0) * 0.74)
            * (0.72 + productivity * 0.28)
            * 0.68;
        let flowering = productivity * (0.48 + openness * 0.34 + forest_edge * 0.18);
        let bee = land * flowering * temperature_comfort * 0.48;
        let squirrel = land
            * productivity
            * (forest_edge * 0.52 + forest_cover * 0.36 + openness * 0.12)
            * slope_comfort
            * 0.62;
        (
            McloneWildlifeHabitatSample {
                world_x,
                world_z,
                surface_y: terrain.surface_y,
                biome,
                landform: landform_kind,
                land: to_permille(land),
                productivity: to_permille(productivity),
                openness: to_permille(openness),
                forest_cover: to_permille(forest_cover),
                wetland: to_permille(wetland),
                water: to_permille(water),
            },
            McloneWildlifeSuitability {
                rabbit: to_permille(rabbit),
                deer: to_permille(deer),
                mallard: to_permille(mallard),
                bee: to_permille(bee),
                squirrel: to_permille(squirrel),
            },
        )
    }

    fn source_hash(&self, cell: McloneWildlifePopulationCell, candidate: u8, lane: u64) -> u64 {
        let mut value = self.seed as u64 ^ lane;
        value = splitmix64(value ^ self.topology.cache_scope());
        value = splitmix64(value ^ u64::from(MCLONE_WILDLIFE_POPULATION_REVISION));
        value = splitmix64(value ^ cell.x as u64);
        value = splitmix64(value ^ (cell.z as u64).rotate_left(32));
        splitmix64(value ^ u64::from(candidate))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McloneWildlifePlanError {
    CoordinateOverflow,
}

impl fmt::Display for McloneWildlifePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateOverflow => write!(formatter, "wildlife plan coordinate overflow"),
        }
    }
}

impl Error for McloneWildlifePlanError {}

fn canonical_cell(
    cell: McloneWildlifePopulationCell,
    topology: McloneOverworldSamplingTopology,
) -> McloneWildlifePopulationCell {
    match topology {
        McloneOverworldSamplingTopology::Unbounded => cell,
        McloneOverworldSamplingTopology::PeriodicX => McloneWildlifePopulationCell {
            x: cell.x.rem_euclid(
                MCLONE_OVERWORLD_PERIOD_BLOCKS / MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS,
            ),
            z: cell.z,
        },
    }
}

fn checked_cell_origin(cell_coordinate: i32) -> Result<i32, McloneWildlifePlanError> {
    i32::try_from(i64::from(cell_coordinate) * i64::from(MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS))
        .map_err(|_| McloneWildlifePlanError::CoordinateOverflow)
}

fn select_species(
    suitability: McloneWildlifeSuitability,
    mut roll: u32,
) -> Option<McloneWildlifeSpecies> {
    for species in McloneWildlifeSpecies::ALL {
        let weight = u32::from(suitability.for_species(species));
        if roll < weight {
            return Some(species);
        }
        roll -= weight;
    }
    None
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

    #[test]
    fn plans_are_stable_across_query_order() {
        let planner = McloneOverworldWildlifePlanner::new(
            -98_765,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let cells = [
            McloneWildlifePopulationCell { x: -8, z: 3 },
            McloneWildlifePopulationCell { x: 0, z: 0 },
            McloneWildlifePopulationCell { x: 12, z: -7 },
            McloneWildlifePopulationCell { x: -1, z: -1 },
        ];
        let forward = cells
            .into_iter()
            .map(|cell| (cell, planner.plan_cell(cell).unwrap()))
            .collect::<BTreeMap<_, _>>();
        let reverse = cells
            .into_iter()
            .rev()
            .map(|cell| (cell, planner.plan_cell(cell).unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(forward, reverse);
    }

    #[test]
    fn negative_chunk_edges_use_euclidean_cells() {
        assert_eq!(
            McloneWildlifePopulationCell::from_chunk(ChunkPos::new(-1, -1)),
            McloneWildlifePopulationCell { x: -1, z: -1 }
        );
        assert_eq!(
            McloneWildlifePopulationCell::from_chunk(ChunkPos::new(-4, -4)),
            McloneWildlifePopulationCell { x: -1, z: -1 }
        );
        assert_eq!(
            McloneWildlifePopulationCell::from_chunk(ChunkPos::new(-5, -5)),
            McloneWildlifePopulationCell { x: -2, z: -2 }
        );
    }

    #[test]
    fn an_encounter_belongs_to_exactly_one_member_chunk() {
        let planner =
            McloneOverworldWildlifePlanner::new(12_345, McloneOverworldSamplingTopology::Unbounded);
        for cell_x in -8..8 {
            for cell_z in -8..8 {
                let cell = McloneWildlifePopulationCell {
                    x: cell_x,
                    z: cell_z,
                };
                let plan = planner.plan_cell(cell).unwrap();
                let Some(encounter) = plan.encounter else {
                    continue;
                };
                assert_eq!(
                    McloneWildlifePopulationCell::from_chunk(encounter.owner_chunk),
                    cell
                );
                let owners = (0..MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS)
                    .flat_map(|local_x| {
                        (0..MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS).map(move |local_z| {
                            ChunkPos::new(
                                cell_x * MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS + local_x,
                                cell_z * MCLONE_WILDLIFE_POPULATION_CELL_CHUNKS + local_z,
                            )
                        })
                    })
                    .filter(|chunk| plan.encounter_for_chunk(*chunk).is_some())
                    .count();
                assert_eq!(owners, 1);
            }
        }
    }

    #[test]
    fn population_receipt_is_pinned_and_species_are_broadly_represented() {
        let planner = McloneOverworldWildlifePlanner::new(
            -98_765,
            McloneOverworldSamplingTopology::Unbounded,
        );
        let mut occupied = 0_u32;
        let mut animals = 0_u32;
        let mut counts = [0_u32; 5];
        let mut density_sum = 0_u64;
        for x in -32..32 {
            for z in -32..32 {
                let plan = planner
                    .plan_cell(McloneWildlifePopulationCell { x, z })
                    .unwrap();
                density_sum += u64::from(plan.desired_density);
                if let Some(encounter) = plan.encounter {
                    occupied += 1;
                    animals += u32::from(encounter.group_size);
                    counts[encounter.species as usize] += 1;
                }
            }
        }
        assert_eq!(
            (occupied, animals, counts, density_sum),
            (1_500, 4_279, [750, 254, 151, 186, 159], 1_536_547)
        );
        assert!((1_300..=1_750).contains(&occupied));
        assert!(counts.into_iter().all(|count| count >= 150));
        assert!(counts[McloneWildlifeSpecies::Rabbit as usize] > counts[1]);
    }

    #[test]
    fn periodic_x_repeats_population_cells() {
        let planner =
            McloneOverworldWildlifePlanner::new(12_345, McloneOverworldSamplingTopology::PeriodicX);
        let period_cells = MCLONE_OVERWORLD_PERIOD_BLOCKS / MCLONE_WILDLIFE_POPULATION_CELL_BLOCKS;
        let left = planner
            .plan_cell(McloneWildlifePopulationCell { x: 7, z: -3 })
            .unwrap();
        let right = planner
            .plan_cell(McloneWildlifePopulationCell {
                x: 7 + period_cells,
                z: -3,
            })
            .unwrap();
        assert_eq!(left, right);
    }
}
