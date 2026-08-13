use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_min_block_coord};

use crate::block::{
    AIR, BIRCH_LEAVES, BIRCH_LOG, BROWN_MUSHROOM, CACTUS, CLAY, COAL_ORE, DANDELION, DEAD_BUSH,
    DIAMOND_ORE, DIRT, FERN, GOLD_ORE, GRASS, GRASS_BLOCK, GRAVEL, ICE, IRON_ORE, LAPIS_ORE, LAVA,
    OAK_LEAVES, OAK_LOG, POPPY, PUMPKIN, RED_MUSHROOM, REDSTONE_ORE, RawBlockId, SAND, SNOW,
    SPRUCE_LEAVES, SPRUCE_LOG, STONE, SUGAR_CANE, WATER,
};
use crate::feature::{
    BasicTreeConfiguration, ConfiguredFeature, FeatureRegion, FeatureWorld, LakeConfiguration,
};
use crate::placement::BlockPos;
use crate::prng::SimpleRandomSource;

use super::caves::{beta_cos, beta_floor, beta_sin};
use super::{
    BETA_ACTIVE_HEIGHT, BetaBiome, BetaClimateSource, BetaNoiseBanks, beta_generated_chunk,
    generate_beta_surface_buffer_with_core,
};
use crate::levelgen::feature_batch::sorted_chunk_positions_z_major;
use crate::levelgen::surface_dependency_cache::{
    PreparedSurfaceDependencies, SurfaceDependencyCache, SurfaceDependencyCacheReport,
};
use crate::levelgen::{ChunkGenerationPlan, GeneratedChunk, MutableChunkBlockBuffer};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BetaFeatureDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BetaFeatureBatchResult {
    pub chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub cache_report: BetaFeatureDependencyCacheReport,
}

#[derive(Debug, Default)]
pub struct BetaFeatureDependencyCache {
    seed: Option<i64>,
    banks: Option<BetaNoiseBanks>,
    climate: Option<BetaClimateSource>,
    cache: SurfaceDependencyCache,
}

impl BetaFeatureDependencyCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn retained_chunk_count(&self) -> usize {
        self.cache.retained_chunk_count()
    }

    pub fn resident_positions(&self) -> BTreeSet<ChunkPos> {
        self.cache.resident_positions()
    }

    pub fn clear(&mut self) {
        self.seed = None;
        self.banks = None;
        self.climate = None;
        self.cache.clear();
    }

    pub fn generate_features_chunks(
        &mut self,
        seed: i64,
        targets: impl IntoIterator<Item = ChunkPos>,
    ) -> BetaFeatureBatchResult {
        self.generate_features_chunks_with_dependencies(seed, targets, std::iter::empty())
    }

    pub fn generate_features_chunks_with_dependencies(
        &mut self,
        seed: i64,
        targets: impl IntoIterator<Item = ChunkPos>,
        dependencies: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> BetaFeatureBatchResult {
        if self.seed != Some(seed) {
            self.clear();
            self.seed = Some(seed);
            self.banks = Some(BetaNoiseBanks::new(seed));
            self.climate = Some(BetaClimateSource::new(seed));
        }

        let plan = ChunkGenerationPlan::beta_features(targets);
        let banks = self.banks.as_ref().expect("Beta banks initialized");
        let climate = self.climate.as_ref().expect("Beta climate initialized");
        let PreparedSurfaceDependencies {
            region_chunks,
            retained_dependencies,
            report,
        } = self.cache.prepare(seed, &plan, dependencies, |pos| {
            generate_beta_surface_buffer_with_core(banks, climate, seed, pos.x, pos.z)
        });
        let cache_report = beta_cache_report(report);

        if plan.output_chunks().is_empty() {
            return BetaFeatureBatchResult {
                chunks: BTreeMap::new(),
                retained_dependencies,
                cache_report,
            };
        }

        let first_target = *plan
            .output_chunks()
            .iter()
            .next()
            .expect("non-empty Beta targets");
        let mut region = FeatureRegion::new(first_target.x, first_target.z, region_chunks);
        for center in sorted_chunk_positions_z_major(plan.backend_work_chunks().iter().copied()) {
            region.set_center(center.x, center.z);
            populate_beta_center(banks, climate, seed, center, &mut region);
        }

        let (targets, _, _) = plan.into_parts();
        let mut chunks = BTreeMap::new();
        for target in targets {
            let mut chunk = region.remove_chunk(target.x, target.z).unwrap_or_else(|| {
                panic!(
                    "Beta feature region omitted target ({}, {})",
                    target.x, target.z
                )
            });
            apply_beta_snow(&mut chunk, climate);
            chunk.prime_worldgen_heightmaps();
            chunks.insert(target, beta_generated_chunk(chunk, climate));
        }

        BetaFeatureBatchResult {
            chunks,
            retained_dependencies,
            cache_report,
        }
    }
}

fn beta_cache_report(report: SurfaceDependencyCacheReport) -> BetaFeatureDependencyCacheReport {
    BetaFeatureDependencyCacheReport {
        requested_dependency_chunks: report.requested_dependency_chunks,
        cache_hits: report.cache_hits,
        generated_dependency_chunks: report.generated_dependency_chunks,
        retained_dependency_chunks: report.retained_dependency_chunks,
    }
}

pub fn generate_beta_chunk(seed: i64, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    let pos = ChunkPos::new(chunk_x, chunk_z);
    BetaFeatureDependencyCache::new()
        .generate_features_chunks(seed, [pos])
        .chunks
        .remove(&pos)
        .unwrap_or_else(|| panic!("Beta feature batch omitted ({chunk_x}, {chunk_z})"))
}

fn populate_beta_center(
    banks: &BetaNoiseBanks,
    climate: &BetaClimateSource,
    seed: i64,
    center: ChunkPos,
    world: &mut FeatureRegion,
) {
    let origin_x = chunk_min_block_coord(center.x);
    let origin_z = chunk_min_block_coord(center.z);
    let biome = climate.region(origin_x + 16, origin_z + 16, 1, 1).biomes[0];
    let mut seed_random = SimpleRandomSource::new(seed);
    let multiplier_x = odd_multiplier(seed_random.next_long());
    let multiplier_z = odd_multiplier(seed_random.next_long());
    let population_seed = i64::from(center.x)
        .wrapping_mul(multiplier_x)
        .wrapping_add(i64::from(center.z).wrapping_mul(multiplier_z))
        ^ seed;
    let mut random = SimpleRandomSource::new(population_seed);

    if random.next_int_bound(4) == 0 {
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_lake(world, &mut random, origin, WATER);
    }
    if random.next_int_bound(8) == 0 {
        let x = origin_x + random.next_int_bound(16) + 8;
        let y_bound = random.next_int_bound(120) + 8;
        let y = random.next_int_bound(y_bound);
        let z = origin_z + random.next_int_bound(16) + 8;
        if y < 64 || random.next_int_bound(10) == 0 {
            place_beta_lake(world, &mut random, BlockPos::new(x, y, z), LAVA);
        }
    }

    for _ in 0..8 {
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_dungeon(world, &mut random, origin);
    }
    for _ in 0..10 {
        let origin = BlockPos::new(
            origin_x + random.next_int_bound(16),
            random.next_int_bound(BETA_ACTIVE_HEIGHT),
            origin_z + random.next_int_bound(16),
        );
        place_beta_vein(world, &mut random, origin, CLAY, 32, SAND, true);
    }
    place_beta_vein_attempts(world, &mut random, origin_x, origin_z, DIRT, 32, 20, 128);
    place_beta_vein_attempts(world, &mut random, origin_x, origin_z, GRAVEL, 32, 10, 128);
    place_beta_vein_attempts(
        world,
        &mut random,
        origin_x,
        origin_z,
        COAL_ORE,
        16,
        20,
        128,
    );
    place_beta_vein_attempts(world, &mut random, origin_x, origin_z, IRON_ORE, 8, 20, 64);
    place_beta_vein_attempts(world, &mut random, origin_x, origin_z, GOLD_ORE, 8, 2, 32);
    place_beta_vein_attempts(
        world,
        &mut random,
        origin_x,
        origin_z,
        REDSTONE_ORE,
        7,
        8,
        16,
    );
    place_beta_vein_attempts(
        world,
        &mut random,
        origin_x,
        origin_z,
        DIAMOND_ORE,
        7,
        1,
        16,
    );
    for _ in 0..1 {
        let origin = BlockPos::new(
            origin_x + random.next_int_bound(16),
            random.next_int_bound(16) + random.next_int_bound(16),
            origin_z + random.next_int_bound(16),
        );
        place_beta_vein(world, &mut random, origin, LAPIS_ORE, 6, STONE, false);
    }

    let forest_count = ((banks
        .forest
        .value_2d(f64::from(origin_x) * 0.5, f64::from(origin_z) * 0.5)
        / 8.0
        + random.next_double() * 4.0
        + 4.0)
        / 3.0) as i32;
    let mut tree_count = i32::from(random.next_int_bound(10) == 0);
    tree_count += match biome {
        BetaBiome::Forest | BetaBiome::Rainforest | BetaBiome::Taiga => forest_count + 5,
        BetaBiome::SeasonalForest => forest_count + 2,
        BetaBiome::Desert | BetaBiome::Tundra | BetaBiome::Plains => -20,
        _ => 0,
    };
    for _ in 0..tree_count.max(0) {
        let x = origin_x + random.next_int_bound(16) + 8;
        let z = origin_z + random.next_int_bound(16) + 8;
        let tree = ConfiguredFeature::basic_tree(beta_tree_configuration(biome, &mut random));
        tree.place(world, &mut random, BlockPos::new(x, 0, z));
    }

    let flowers = match biome {
        BetaBiome::Forest | BetaBiome::Taiga => 2,
        BetaBiome::SeasonalForest => 4,
        BetaBiome::Plains => 3,
        _ => 0,
    };
    for _ in 0..flowers {
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_plant_patch(world, &mut random, origin, DANDELION, 64, false);
    }
    let grasses = match biome {
        BetaBiome::Forest | BetaBiome::SeasonalForest => 2,
        BetaBiome::Rainforest | BetaBiome::Plains => 10,
        BetaBiome::Taiga => 1,
        _ => 0,
    };
    for _ in 0..grasses {
        let plant = if biome == BetaBiome::Rainforest && random.next_int_bound(3) != 0 {
            FERN
        } else {
            GRASS
        };
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_plant_patch(world, &mut random, origin, plant, 128, true);
    }
    if biome == BetaBiome::Desert {
        for _ in 0..2 {
            let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
            place_beta_plant_patch(world, &mut random, origin, DEAD_BUSH, 4, true);
        }
    }
    if random.next_int_bound(2) == 0 {
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_plant_patch(world, &mut random, origin, POPPY, 64, false);
    }
    if random.next_int_bound(4) == 0 {
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_plant_patch(world, &mut random, origin, BROWN_MUSHROOM, 64, false);
    }
    if random.next_int_bound(8) == 0 {
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_plant_patch(world, &mut random, origin, RED_MUSHROOM, 64, false);
    }
    for _ in 0..10 {
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_sugar_cane(world, &mut random, origin);
    }
    if random.next_int_bound(32) == 0 {
        let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
        place_beta_pumpkins(world, &mut random, origin);
    }
    if biome == BetaBiome::Desert {
        for _ in 0..10 {
            let origin = beta_offset_origin(&mut random, origin_x, origin_z, false);
            place_beta_cactus(world, &mut random, origin);
        }
    }

    for _ in 0..50 {
        let x = origin_x + random.next_int_bound(16) + 8;
        let y_bound = random.next_int_bound(120) + 8;
        let y = random.next_int_bound(y_bound);
        let z = origin_z + random.next_int_bound(16) + 8;
        place_beta_spring(world, BlockPos::new(x, y, z), WATER);
    }
    for _ in 0..20 {
        let x = origin_x + random.next_int_bound(16) + 8;
        let first = random.next_int_bound(112) + 8;
        let second = random.next_int_bound(first) + 8;
        let y = random.next_int_bound(second);
        let z = origin_z + random.next_int_bound(16) + 8;
        place_beta_spring(world, BlockPos::new(x, y, z), LAVA);
    }
}

fn place_beta_lake(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    mut origin: BlockPos,
    liquid: RawBlockId,
) -> bool {
    // The historical feature subtracts eight before forming its 16x16 mask;
    // the shared geometric primitive accepts that adjusted base directly.
    origin.x -= 8;
    origin.z -= 8;
    ConfiguredFeature::lake(LakeConfiguration::new(liquid)).place(world, random, origin)
}

fn beta_tree_configuration(
    biome: BetaBiome,
    random: &mut SimpleRandomSource,
) -> BasicTreeConfiguration {
    match biome {
        BetaBiome::Forest => {
            if random.next_int_bound(5) == 0 {
                BasicTreeConfiguration::new(BIRCH_LOG, BIRCH_LEAVES, 5, 3)
            } else if random.next_int_bound(3) == 0 {
                BasicTreeConfiguration::new(OAK_LOG, OAK_LEAVES, 6, 4)
            } else {
                BasicTreeConfiguration::oak()
            }
        }
        BetaBiome::Rainforest => {
            if random.next_int_bound(3) == 0 {
                BasicTreeConfiguration::new(OAK_LOG, OAK_LEAVES, 7, 5)
            } else {
                BasicTreeConfiguration::oak()
            }
        }
        BetaBiome::Taiga => {
            if random.next_int_bound(3) == 0 {
                BasicTreeConfiguration::new(SPRUCE_LOG, SPRUCE_LEAVES, 7, 5)
            } else {
                BasicTreeConfiguration::spruce()
            }
        }
        _ => {
            if random.next_int_bound(10) == 0 {
                BasicTreeConfiguration::new(OAK_LOG, OAK_LEAVES, 6, 4)
            } else {
                BasicTreeConfiguration::oak()
            }
        }
    }
}

fn beta_offset_origin(
    random: &mut SimpleRandomSource,
    origin_x: i32,
    origin_z: i32,
    unshifted: bool,
) -> BlockPos {
    let offset = if unshifted { 0 } else { 8 };
    BlockPos::new(
        origin_x + random.next_int_bound(16) + offset,
        random.next_int_bound(BETA_ACTIVE_HEIGHT),
        origin_z + random.next_int_bound(16) + offset,
    )
}

#[allow(clippy::too_many_arguments)]
fn place_beta_vein_attempts(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin_x: i32,
    origin_z: i32,
    block: RawBlockId,
    size: i32,
    attempts: i32,
    y_bound: i32,
) {
    for _ in 0..attempts {
        let origin = BlockPos::new(
            origin_x + random.next_int_bound(16),
            random.next_int_bound(y_bound),
            origin_z + random.next_int_bound(16),
        );
        place_beta_vein(world, random, origin, block, size, STONE, false);
    }
}

fn place_beta_vein(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
    block: RawBlockId,
    size: i32,
    replace: RawBlockId,
    require_water_origin: bool,
) -> bool {
    if require_water_origin && beta_block(world, origin) != WATER {
        return false;
    }
    let angle = random.next_float() * std::f32::consts::PI;
    let start_x = f64::from(origin.x + 8) + f64::from(beta_sin(angle) * size as f32 / 8.0);
    let end_x = f64::from(origin.x + 8) - f64::from(beta_sin(angle) * size as f32 / 8.0);
    let start_z = f64::from(origin.z + 8) + f64::from(beta_cos(angle) * size as f32 / 8.0);
    let end_z = f64::from(origin.z + 8) - f64::from(beta_cos(angle) * size as f32 / 8.0);
    let start_y = f64::from(origin.y + random.next_int_bound(3) + 2);
    let end_y = f64::from(origin.y + random.next_int_bound(3) + 2);
    for step in 0..=size {
        let progress = f64::from(step) / f64::from(size);
        let center_x = start_x + (end_x - start_x) * progress;
        let center_y = start_y + (end_y - start_y) * progress;
        let center_z = start_z + (end_z - start_z) * progress;
        let scale = random.next_double() * f64::from(size) / 16.0;
        let radius = f64::from(beta_sin(step as f32 * std::f32::consts::PI / size as f32) + 1.0)
            * scale
            + 1.0;
        for x in beta_floor(center_x - radius / 2.0)..=beta_floor(center_x + radius / 2.0) {
            let dx = (f64::from(x) + 0.5 - center_x) / (radius / 2.0);
            if dx * dx >= 1.0 {
                continue;
            }
            for y in beta_floor(center_y - radius / 2.0)..=beta_floor(center_y + radius / 2.0) {
                let dy = (f64::from(y) + 0.5 - center_y) / (radius / 2.0);
                if dx * dx + dy * dy >= 1.0 {
                    continue;
                }
                for z in beta_floor(center_z - radius / 2.0)..=beta_floor(center_z + radius / 2.0) {
                    let dz = (f64::from(z) + 0.5 - center_z) / (radius / 2.0);
                    let pos = BlockPos::new(x, y, z);
                    if dx * dx + dy * dy + dz * dz < 1.0 && beta_block(world, pos) == replace {
                        world.set_block_world(pos, block);
                    }
                }
            }
        }
    }
    true
}

fn place_beta_dungeon(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
) -> bool {
    let radius_x = random.next_int_bound(2) + 2;
    let radius_z = random.next_int_bound(2) + 2;
    let mut openings = 0;
    for x in origin.x - radius_x - 1..=origin.x + radius_x + 1 {
        for y in origin.y - 1..=origin.y + 4 {
            for z in origin.z - radius_z - 1..=origin.z + radius_z + 1 {
                let block = beta_block(world, BlockPos::new(x, y, z));
                if (y == origin.y - 1 || y == origin.y + 4) && !beta_is_solid(block) {
                    return false;
                }
                if (x == origin.x - radius_x - 1
                    || x == origin.x + radius_x + 1
                    || z == origin.z - radius_z - 1
                    || z == origin.z + radius_z + 1)
                    && y == origin.y
                    && block == AIR
                    && beta_block(world, BlockPos::new(x, y + 1, z)) == AIR
                {
                    openings += 1;
                }
            }
        }
    }
    if !(1..=5).contains(&openings) {
        return false;
    }
    for x in origin.x - radius_x - 1..=origin.x + radius_x + 1 {
        for y in (origin.y - 1..=origin.y + 3).rev() {
            for z in origin.z - radius_z - 1..=origin.z + radius_z + 1 {
                let boundary = x == origin.x - radius_x - 1
                    || x == origin.x + radius_x + 1
                    || y == origin.y - 1
                    || y == origin.y + 3
                    || z == origin.z - radius_z - 1
                    || z == origin.z + radius_z + 1;
                let pos = BlockPos::new(x, y, z);
                if !boundary {
                    world.set_block_world(pos, AIR);
                } else if beta_is_solid(beta_block(world, pos)) {
                    world.set_block_world(pos, STONE);
                }
            }
        }
    }
    true
}

fn place_beta_plant_patch(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    mut origin: BlockPos,
    plant: RawBlockId,
    tries: i32,
    seek_ground: bool,
) {
    if seek_ground {
        while origin.y > 0 && beta_block(world, origin) == AIR {
            origin.y -= 1;
        }
    }
    for _ in 0..tries {
        let pos = BlockPos::new(
            origin.x + random.next_int_bound(8) - random.next_int_bound(8),
            origin.y + random.next_int_bound(4) - random.next_int_bound(4),
            origin.z + random.next_int_bound(8) - random.next_int_bound(8),
        );
        let below = beta_block(world, BlockPos::new(pos.x, pos.y - 1, pos.z));
        let allowed = match plant {
            DEAD_BUSH => matches!(below, SAND | DIRT),
            BROWN_MUSHROOM | RED_MUSHROOM => matches!(below, GRASS_BLOCK | DIRT | STONE),
            _ => matches!(below, GRASS_BLOCK | DIRT),
        };
        if beta_block(world, pos) == AIR && allowed {
            world.set_block_world(pos, plant);
        }
    }
}

fn place_beta_sugar_cane(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
) {
    for _ in 0..20 {
        let pos = BlockPos::new(
            origin.x + random.next_int_bound(4) - random.next_int_bound(4),
            origin.y,
            origin.z + random.next_int_bound(4) - random.next_int_bound(4),
        );
        let below_y = pos.y - 1;
        let near_water = [(-1, 0), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .any(|(dx, dz)| {
                beta_block(world, BlockPos::new(pos.x + dx, below_y, pos.z + dz)) == WATER
            });
        if beta_block(world, pos) == AIR && near_water {
            let height_bound = random.next_int_bound(3) + 1;
            let height = 2 + random.next_int_bound(height_bound);
            for dy in 0..height {
                let target = BlockPos::new(pos.x, pos.y + dy, pos.z);
                if beta_block(world, target) != AIR {
                    break;
                }
                world.set_block_world(target, SUGAR_CANE);
            }
        }
    }
}

fn place_beta_cactus(world: &mut FeatureRegion, random: &mut SimpleRandomSource, origin: BlockPos) {
    for _ in 0..10 {
        let pos = BlockPos::new(
            origin.x + random.next_int_bound(8) - random.next_int_bound(8),
            origin.y + random.next_int_bound(4) - random.next_int_bound(4),
            origin.z + random.next_int_bound(8) - random.next_int_bound(8),
        );
        if beta_block(world, pos) != AIR {
            continue;
        }
        let height_bound = random.next_int_bound(3) + 1;
        let height = 1 + random.next_int_bound(height_bound);
        for dy in 0..height {
            let target = BlockPos::new(pos.x, pos.y + dy, pos.z);
            let below = beta_block(world, BlockPos::new(target.x, target.y - 1, target.z));
            let sides_clear = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .all(|(dx, dz)| {
                    beta_block(world, BlockPos::new(target.x + dx, target.y, target.z + dz)) == AIR
                });
            if beta_block(world, target) == AIR && matches!(below, SAND | CACTUS) && sides_clear {
                world.set_block_world(target, CACTUS);
            }
        }
    }
}

fn place_beta_pumpkins(
    world: &mut FeatureRegion,
    random: &mut SimpleRandomSource,
    origin: BlockPos,
) {
    for _ in 0..64 {
        let pos = BlockPos::new(
            origin.x + random.next_int_bound(8) - random.next_int_bound(8),
            origin.y + random.next_int_bound(4) - random.next_int_bound(4),
            origin.z + random.next_int_bound(8) - random.next_int_bound(8),
        );
        if beta_block(world, pos) == AIR
            && beta_block(world, BlockPos::new(pos.x, pos.y - 1, pos.z)) == GRASS_BLOCK
        {
            world.set_block_world(pos, PUMPKIN);
            random.next_int_bound(4);
        }
    }
}

fn place_beta_spring(world: &mut FeatureRegion, origin: BlockPos, liquid: RawBlockId) -> bool {
    if beta_block(world, BlockPos::new(origin.x, origin.y + 1, origin.z)) != STONE
        || beta_block(world, BlockPos::new(origin.x, origin.y - 1, origin.z)) != STONE
        || !matches!(beta_block(world, origin), AIR | STONE)
    {
        return false;
    }
    let neighbours = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let stone = neighbours
        .into_iter()
        .filter(|(dx, dz)| {
            beta_block(world, BlockPos::new(origin.x + dx, origin.y, origin.z + dz)) == STONE
        })
        .count();
    let air = neighbours
        .into_iter()
        .filter(|(dx, dz)| {
            beta_block(world, BlockPos::new(origin.x + dx, origin.y, origin.z + dz)) == AIR
        })
        .count();
    if stone == 3 && air == 1 {
        world.set_block_world(origin, liquid);
    }
    true
}

fn apply_beta_snow(chunk: &mut MutableChunkBlockBuffer, climate: &BetaClimateSource) {
    let min_x = chunk_min_block_coord(chunk.chunk_x);
    let min_z = chunk_min_block_coord(chunk.chunk_z);
    let temperatures = climate.temperatures(min_x, min_z, 16, 16);
    for local_x in 0..CHUNK_WIDTH {
        for local_z in 0..CHUNK_WIDTH {
            let mut surface_y = 0;
            for y in (0..BETA_ACTIVE_HEIGHT).rev() {
                if chunk.get_block_at_y(local_x, y, local_z) != AIR {
                    surface_y = y + 1;
                    break;
                }
            }
            if surface_y <= 0 || surface_y >= BETA_ACTIVE_HEIGHT {
                continue;
            }
            let temperature = temperatures[(local_x * CHUNK_WIDTH + local_z) as usize]
                - f64::from(surface_y - 64) / 64.0 * 0.3;
            let below = chunk.get_block_at_y(local_x, surface_y - 1, local_z);
            if temperature < 0.5 && beta_is_solid(below) && below != ICE {
                chunk.set_block_at_y(local_x, surface_y, local_z, SNOW);
            }
        }
    }
}

fn beta_block(world: &mut FeatureRegion, pos: BlockPos) -> RawBlockId {
    world.block_at_world(pos).unwrap_or(AIR)
}

fn beta_is_solid(block: RawBlockId) -> bool {
    !matches!(
        block,
        AIR | WATER
            | LAVA
            | SNOW
            | GRASS
            | FERN
            | DEAD_BUSH
            | DANDELION
            | POPPY
            | BROWN_MUSHROOM
            | RED_MUSHROOM
            | SUGAR_CANE
    )
}

fn odd_multiplier(value: i64) -> i64 {
    value.wrapping_div(2).wrapping_mul(2).wrapping_add(1)
}
