use crate::block::{RawBlockId, has_fluid, is_air_like, is_leaves, material_blocks_motion};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{BlockPos, HeightmapType};
use crate::prng::RandomSource;

mod bamboo;
mod configured;
mod context;
mod disk;
mod dripstone;
mod glow_lichen;
mod ice;
mod lake;
mod mushroom;
mod ocean;
mod ore;
mod patch;
mod placed;
mod region;
mod spring;
mod tables;
mod top_layer;
mod tree;
mod vines;

pub use configured::{
    BambooConfiguration, BasicTreeConfiguration, ConfiguredFeature, CoralShape,
    DecoratedFeatureConfiguration, DiskConfiguration, DripstoneClusterConfiguration, FloatProvider,
    FoliagePlacerConfiguration, GlowLichenConfiguration, HugeMushroomConfiguration,
    HugeMushroomKind, LakeConfiguration, OreConfiguration, OreTarget, OreTargetBlockState,
    RandomBooleanFeatureConfiguration, RandomFeatureConfiguration, RandomPatchConfiguration,
    RandomPatchStateProvider, SeagrassConfiguration, SimpleBlockConfiguration,
    SimpleRandomFeatureConfiguration, SmallDripstoneConfiguration, SpringConfiguration,
    StraightTrunkPlacerConfiguration, TreeConfiguration, TrunkPlacerConfiguration,
    TwoLayersFeatureSize, WeightedBlockState, WeightedConfiguredFeature,
};
pub use context::{DecorationStep, FeatureDecorationTiming, FeatureWorld};
pub use placed::{
    DecorationReport, PlacedFeature, apply_overworld_biome_decoration,
    apply_overworld_biome_decoration_to_region, apply_overworld_biome_features,
};
pub use region::{FeatureRegion, FeatureRegionMetrics};
pub use tables::overworld_features_for_biome;

pub(crate) use context::{
    ConstantFeatureBiomeResolver, DEFAULT_FEATURE_BIOME, FeatureBiomeResolver,
    OverworldFeatureBiomeResolver,
};
pub(crate) use placed::apply_overworld_biome_decoration_to_region_timed;

#[cfg(test)]
pub(crate) use placed::test_support;

pub const FEATURES_WRITE_RADIUS_CUTOFF: i32 = 1;
/// Java `FEATURES` has status dependency range 8, but current native feature
/// placement only needs full mutable block buffers in the write/read band. The
/// farther Java shell is a weaker status/structure dependency and should not
/// force full terrain generation during startup.
pub const FEATURES_CHUNK_DEPENDENCY_RADIUS: i32 = 8;
pub const FEATURES_BLOCK_DEPENDENCY_RADIUS: i32 = FEATURES_WRITE_RADIUS_CUTOFF;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    const GLOW_LICHEN_VALID: [Self; 5] =
        [Self::Up, Self::North, Self::East, Self::South, Self::West];

    const fn offset(self) -> (i32, i32, i32) {
        match self {
            Self::Down => (0, -1, 0),
            Self::Up => (0, 1, 0),
            Self::North => (0, 0, -1),
            Self::South => (0, 0, 1),
            Self::West => (-1, 0, 0),
            Self::East => (1, 0, 0),
        }
    }

    const fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    const fn axis(self) -> i32 {
        match self {
            Self::Down | Self::Up => 0,
            Self::North | Self::South => 1,
            Self::West | Self::East => 2,
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Down => 1 << 0,
            Self::Up => 1 << 1,
            Self::North => 1 << 2,
            Self::South => 1 << 3,
            Self::West => 1 << 4,
            Self::East => 1 << 5,
        }
    }
}

impl ConfiguredFeature {
    pub fn place<W: FeatureWorld>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: BlockPos,
    ) -> bool {
        let biomes = ConstantFeatureBiomeResolver::new(DEFAULT_FEATURE_BIOME);
        self.place_with_biomes(world, &biomes, random, origin)
    }

    pub(crate) fn place_with_biomes<W: FeatureWorld, B: FeatureBiomeResolver>(
        &self,
        world: &mut W,
        biomes: &B,
        random: &mut impl RandomSource,
        origin: BlockPos,
    ) -> bool {
        match self {
            Self::Noop => false,
            Self::Lake(config) => lake::place_lake(world, random, origin, *config),
            Self::Spring(config) => spring::place_spring(world, origin, *config),
            Self::SimpleBlock(config) => patch::place_simple_block(world, random, origin, *config),
            Self::RandomPatch(config) => patch::place_random_patch(world, random, origin, *config),
            Self::Flower(config) => patch::place_flower(world, random, origin, *config),
            Self::Disk(config) => disk::place_disk(world, random, origin, *config),
            Self::Iceberg(state) => ice::place_iceberg(world, random, origin, *state),
            Self::BlueIce => ice::place_blue_ice(world, random, origin),
            Self::IceSpike => ice::place_ice_spike(world, random, origin),
            Self::IcePatch(config) => ice::place_ice_patch(world, random, origin, *config),
            Self::GlowLichen(config) => {
                glow_lichen::place_glow_lichen(world, random, origin, *config)
            }
            Self::DripstoneCluster(config) => {
                dripstone::place_dripstone_cluster(world, random, origin, *config)
            }
            Self::SmallDripstone(config) => {
                dripstone::place_small_dripstone(world, random, origin, *config)
            }
            Self::BasicTree(config) => tree::place_basic_tree(world, random, origin, *config),
            Self::Tree(config) => tree::place_tree(world, random, origin, *config),
            Self::RandomSelector(config) => {
                place_random_selector(world, biomes, random, origin, config)
            }
            Self::SimpleRandomSelector(config) => {
                place_simple_random_selector(world, biomes, random, origin, config)
            }
            Self::RandomBooleanSelector(config) => {
                place_random_boolean_selector(world, biomes, random, origin, config)
            }
            Self::Decorated(config) => {
                placed::place_configured_decorated_feature(world, biomes, random, origin, config)
            }
            Self::HugeMushroom(config) => {
                mushroom::place_huge_mushroom(world, random, origin, *config)
            }
            Self::Coral(shape) => ocean::place_coral(world, random, origin, *shape),
            Self::SeaPickle(config) => ocean::place_sea_pickle(world, random, origin, *config),
            Self::Seagrass(config) => ocean::place_seagrass(world, random, origin, *config),
            Self::Bamboo(config) => bamboo::place_bamboo(world, random, origin, *config),
            Self::Vines => vines::place_vines(world, origin),
            Self::Kelp => ocean::place_kelp(world, random, origin),
            Self::Ore(config) => ore::place_ore(world, random, origin, config),
            Self::FreezeTopLayer => top_layer::place_freeze_top_layer(world, biomes, origin),
        }
    }
}

fn offset_pos(pos: BlockPos, direction: Direction) -> BlockPos {
    let (dx, dy, dz) = direction.offset();
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}

fn place_random_selector<W: FeatureWorld, B: FeatureBiomeResolver>(
    world: &mut W,
    biomes: &B,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: &RandomFeatureConfiguration,
) -> bool {
    for weighted in &config.features {
        if random.next_float() < weighted.chance {
            return weighted
                .feature
                .place_with_biomes(world, biomes, random, origin);
        }
    }

    config
        .default_feature
        .place_with_biomes(world, biomes, random, origin)
}

fn place_simple_random_selector<W: FeatureWorld, B: FeatureBiomeResolver>(
    world: &mut W,
    biomes: &B,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: &SimpleRandomFeatureConfiguration,
) -> bool {
    if config.features.is_empty() {
        return false;
    }

    let index = random.next_int_bound(config.features.len() as i32) as usize;
    config.features[index].place_with_biomes(world, biomes, random, origin)
}

fn place_random_boolean_selector<W: FeatureWorld, B: FeatureBiomeResolver>(
    world: &mut W,
    biomes: &B,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: &RandomBooleanFeatureConfiguration,
) -> bool {
    if random.next_boolean() {
        config
            .feature_true
            .place_with_biomes(world, biomes, random, origin)
    } else {
        config
            .feature_false
            .place_with_biomes(world, biomes, random, origin)
    }
}

fn project_to_surface<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> Option<BlockPos> {
    Some(BlockPos::new(
        pos.x,
        world.world_surface_height_at(pos.x, pos.z)?,
        pos.z,
    ))
}

fn heightmap_height(
    chunk: &MutableChunkBlockBuffer,
    heightmap: HeightmapType,
    local_x: i32,
    local_z: i32,
) -> i32 {
    if let Some(height) = chunk.cached_worldgen_height(heightmap, local_x, local_z) {
        return height;
    }

    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        if heightmap_is_opaque(heightmap, chunk.get_block_at_y(local_x, y, local_z)) {
            return y + 1;
        }
    }
    chunk.min_y
}

fn heightmap_is_opaque(heightmap: HeightmapType, block_id: RawBlockId) -> bool {
    match heightmap {
        HeightmapType::WorldSurfaceWg | HeightmapType::WorldSurface => !is_air_like(block_id),
        HeightmapType::OceanFloorWg | HeightmapType::OceanFloor => material_blocks_motion(block_id),
        HeightmapType::MotionBlocking => material_blocks_motion(block_id) || has_fluid(block_id),
        HeightmapType::MotionBlockingNoLeaves => {
            (material_blocks_motion(block_id) || has_fluid(block_id)) && !is_leaves(block_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::{BiomeDefinition, get_layered_biome_by_id};
    use crate::block::{
        ACACIA_LEAVES, ACACIA_LOG, AIR, ANDESITE, BAMBOO, BAMBOO_FINAL_LARGE, BAMBOO_TOP_LARGE,
        BAMBOO_TOP_SMALL, BIRCH_LEAVES, BIRCH_LOG, BLUE_ICE, BLUE_ORCHID, BRAIN_CORAL_BLOCK,
        BROWN_MUSHROOM, BROWN_MUSHROOM_BLOCK, BUBBLE_CORAL_BLOCK, CACTUS, CAVE_AIR, CLAY, COAL_ORE,
        COCOA_AGE0_EAST, COCOA_AGE0_NORTH, COCOA_AGE0_SOUTH, COCOA_AGE0_WEST, COCOA_AGE1_EAST,
        COCOA_AGE1_NORTH, COCOA_AGE1_SOUTH, COCOA_AGE1_WEST, COCOA_AGE2_EAST, COCOA_AGE2_NORTH,
        COCOA_AGE2_SOUTH, COCOA_AGE2_WEST, COPPER_ORE, DANDELION, DARK_OAK_LEAVES, DARK_OAK_LOG,
        DEAD_BUSH, DEEPSLATE, DEEPSLATE_COAL_ORE, DEEPSLATE_COPPER_ORE, DEEPSLATE_DIAMOND_ORE,
        DEEPSLATE_GOLD_ORE, DEEPSLATE_IRON_ORE, DEEPSLATE_LAPIS_ORE, DEEPSLATE_REDSTONE_ORE,
        DIAMOND_ORE, DIORITE, DIRT, FIRE_CORAL_BLOCK, GLOW_LICHEN, GOLD_ORE, GRANITE, GRASS,
        GRASS_BLOCK, GRAVEL, HORN_CORAL_BLOCK, ICE, IRON_ORE, JUNGLE_LEAVES, JUNGLE_LOG, KELP,
        KELP_PLANT, LAPIS_ORE, LARGE_FERN_LOWER, LARGE_FERN_UPPER, LAVA, LILAC_LOWER,
        LILY_OF_THE_VALLEY, LILY_PAD, MELON, MUSHROOM_STEM, MYCELIUM, OAK_LEAVES, OAK_LOG,
        PACKED_ICE, PEONY_LOWER, PODZOL, POPPY, PUMPKIN, RED_MUSHROOM, RED_MUSHROOM_BLOCK,
        RED_SAND, REDSTONE_ORE, ROSE_BUSH_LOWER, ROSE_BUSH_UPPER, SAND, SEA_PICKLE_1, SEA_PICKLE_2,
        SEA_PICKLE_3, SEA_PICKLE_4, SEAGRASS, SNOW, SPRUCE_LEAVES, STONE, SUGAR_CANE,
        SUNFLOWER_LOWER, SWEET_BERRY_BUSH, TALL_GRASS_LOWER, TALL_GRASS_UPPER, TALL_SEAGRASS_LOWER,
        TALL_SEAGRASS_UPPER, TERRACOTTA, TUBE_CORAL_BLOCK, TUFF, VINE_EAST, VINE_NORTH, VINE_SOUTH,
        VINE_UP, VINE_WEST, WATER,
    };
    use crate::placement::{
        ConfiguredDecorator, CountConfiguration, DecorationContext, HeightProvider, IntProvider,
        VerticalAnchor,
    };
    use crate::prng::{RandomSource, WorldgenRandom};
    use mclone_core::CHUNK_WIDTH;

    fn flat_grass_chunk() -> MutableChunkBlockBuffer {
        flat_grass_chunk_at(0, 0)
    }

    fn flat_grass_chunk_at(chunk_x: i32, chunk_z: i32) -> MutableChunkBlockBuffer {
        let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, 0, 32);
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {
                chunk.set_block_at_y(x, 0, z, STONE);
                chunk.set_block_at_y(x, 1, z, DIRT);
                chunk.set_block_at_y(x, 2, z, GRASS_BLOCK);
            }
        }
        chunk
    }

    fn flat_ocean_chunk() -> MutableChunkBlockBuffer {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 32);
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {
                chunk.set_block_at_y(x, 0, z, STONE);
                chunk.set_block_at_y(x, 1, z, SAND);
                for y in 2..=10 {
                    chunk.set_block_at_y(x, y, z, WATER);
                }
            }
        }
        chunk
    }

    fn solid_stone_chunk() -> MutableChunkBlockBuffer {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 64);
        for x in 0..CHUNK_WIDTH {
            for y in 0..64 {
                for z in 0..CHUNK_WIDTH {
                    chunk.set_block_at_y(x, y, z, STONE);
                }
            }
        }
        chunk
    }

    fn count_blocks(chunk: &MutableChunkBlockBuffer, block_id: RawBlockId) -> usize {
        chunk
            .blocks
            .iter()
            .filter(|current| **current == block_id)
            .count()
    }

    fn count_any_blocks(chunk: &MutableChunkBlockBuffer, block_ids: &[RawBlockId]) -> usize {
        chunk
            .blocks
            .iter()
            .filter(|current| block_ids.contains(*current))
            .count()
    }

    fn count_blocks_at_y(chunk: &MutableChunkBlockBuffer, y: i32, block_id: RawBlockId) -> usize {
        let mut count = 0;
        for z in 0..CHUNK_WIDTH {
            for x in 0..CHUNK_WIDTH {
                if chunk.get_block_at_y(x, y, z) == block_id {
                    count += 1;
                }
            }
        }
        count
    }

    fn has_two_by_two_log_square_at(
        chunk: &MutableChunkBlockBuffer,
        base: BlockPos,
        log: RawBlockId,
    ) -> bool {
        chunk.get_block_at_y(base.x, base.y, base.z) == log
            && chunk.get_block_at_y(base.x + 1, base.y, base.z) == log
            && chunk.get_block_at_y(base.x, base.y, base.z + 1) == log
            && chunk.get_block_at_y(base.x + 1, base.y, base.z + 1) == log
    }

    fn count_logs_outside_two_by_two_column(
        chunk: &MutableChunkBlockBuffer,
        base: BlockPos,
        log: RawBlockId,
    ) -> usize {
        let mut count = 0;
        for y in chunk.min_y..chunk.min_y + chunk.height {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    let in_trunk_column =
                        (base.x..=base.x + 1).contains(&x) && (base.z..=base.z + 1).contains(&z);
                    if !in_trunk_column && chunk.get_block_at_y(x, y, z) == log {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    struct BooleanRandom {
        value: bool,
    }

    impl BooleanRandom {
        const fn new(value: bool) -> Self {
            Self { value }
        }
    }

    impl RandomSource for BooleanRandom {
        fn set_seed(&mut self, _seed: i64) {}

        fn next_int(&mut self) -> i32 {
            panic!("next_int should not be used by this test random")
        }

        fn next_int_bound(&mut self, _bound: i32) -> i32 {
            panic!("next_int_bound should not be used by this test random")
        }

        fn next_long(&mut self) -> i64 {
            panic!("next_long should not be used by this test random")
        }

        fn next_boolean(&mut self) -> bool {
            self.value
        }

        fn next_float(&mut self) -> f32 {
            panic!("next_float should not be used by this test random")
        }

        fn next_double(&mut self) -> f64 {
            panic!("next_double should not be used by this test random")
        }

        fn next_gaussian(&mut self) -> f64 {
            panic!("next_gaussian should not be used by this test random")
        }
    }

    #[derive(Default)]
    struct OreHeightmapProbeWorld {
        height_queries: Vec<HeightmapType>,
        world_surface_queries: usize,
        writes: usize,
    }

    impl FeatureWorld for OreHeightmapProbeWorld {
        fn center_chunk_x(&self) -> i32 {
            0
        }

        fn center_chunk_z(&self) -> i32 {
            0
        }

        fn min_y(&self) -> i32 {
            0
        }

        fn height(&self) -> i32 {
            64
        }

        fn non_air_block_count(&self) -> usize {
            0
        }

        fn block_at_world(&mut self, pos: BlockPos) -> Option<RawBlockId> {
            (0..64).contains(&pos.y).then_some(STONE)
        }

        fn height_at(
            &mut self,
            heightmap: HeightmapType,
            _world_x: i32,
            _world_z: i32,
        ) -> Option<i32> {
            self.height_queries.push(heightmap);
            match heightmap {
                HeightmapType::OceanFloorWg => Some(64),
                _ => Some(0),
            }
        }

        fn world_surface_height_at(&mut self, _world_x: i32, _world_z: i32) -> Option<i32> {
            self.world_surface_queries += 1;
            Some(0)
        }

        fn set_block_world(&mut self, pos: BlockPos, _block_id: RawBlockId) -> bool {
            if (0..64).contains(&pos.y) {
                self.writes += 1;
                true
            } else {
                false
            }
        }
    }

    #[test]
    fn simple_block_feature_places_on_grass_surface() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(0);
        let feature = ConfiguredFeature::simple_block(
            SimpleBlockConfiguration::new(DANDELION)
                .place_on(&[GRASS_BLOCK])
                .place_in(&[AIR]),
        );

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(4, 3, 5)));
        assert_eq!(chunk.get_block_at_y(4, 3, 5), DANDELION);
    }

    #[test]
    fn random_boolean_selector_uses_java_next_boolean_branching() {
        let feature =
            ConfiguredFeature::random_boolean_selector(RandomBooleanFeatureConfiguration::new(
                ConfiguredFeature::simple_block(SimpleBlockConfiguration::new(POPPY)),
                ConfiguredFeature::simple_block(SimpleBlockConfiguration::new(DANDELION)),
            ));

        let mut true_chunk = flat_grass_chunk();
        assert!(feature.place(
            &mut true_chunk,
            &mut BooleanRandom::new(true),
            BlockPos::new(8, 3, 8)
        ));
        assert_eq!(true_chunk.get_block_at_y(8, 3, 8), POPPY);

        let mut false_chunk = flat_grass_chunk();
        assert!(feature.place(
            &mut false_chunk,
            &mut BooleanRandom::new(false),
            BlockPos::new(8, 3, 8)
        ));
        assert_eq!(false_chunk.get_block_at_y(8, 3, 8), DANDELION);
    }

    #[test]
    fn spring_feature_places_fluid_and_schedules_liquid_tick() {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        let origin = BlockPos::new(8, 8, 8);
        for pos in [
            BlockPos::new(8, 9, 8),
            BlockPos::new(8, 7, 8),
            BlockPos::new(7, 8, 8),
            BlockPos::new(9, 8, 8),
            BlockPos::new(8, 8, 7),
        ] {
            chunk.set_block_at_y(pos.x, pos.y, pos.z, STONE);
        }
        let mut random = WorldgenRandom::new(0);
        let feature = ConfiguredFeature::spring(SpringConfiguration::water());

        assert!(feature.place(&mut chunk, &mut random, origin));
        assert_eq!(chunk.get_block_at_y(8, 8, 8), WATER);
        assert_eq!(
            chunk.liquid_ticks(),
            &[crate::levelgen::ScheduledTick::new(
                8,
                8,
                8,
                "minecraft:water",
                0
            )]
        );
    }

    #[test]
    fn random_patch_projects_to_surface_and_places_multiple_blocks() {
        let mut chunk = flat_grass_chunk();
        let before = chunk.non_air_block_count();
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: GRASS,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 16,
            xspread: 3,
            yspread: 1,
            zspread: 3,
            project: true,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK],
        });

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert!(chunk.non_air_block_count() > before);
        assert!(chunk.blocks.iter().any(|block_id| *block_id == GRASS));
    }

    #[test]
    fn random_patch_double_plant_writes_large_fern_halves() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(3);
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: LARGE_FERN_LOWER,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: true,
            column_height: None,
            need_water: false,
            place_on: &[],
        });

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert_eq!(chunk.get_block_at_y(8, 3, 8), LARGE_FERN_LOWER);
        assert_eq!(chunk.get_block_at_y(8, 4, 8), LARGE_FERN_UPPER);
    }

    #[test]
    fn random_patch_double_plant_writes_tall_grass_halves() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(3);
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: TALL_GRASS_LOWER,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: true,
            column_height: None,
            need_water: false,
            place_on: &[],
        });

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert_eq!(chunk.get_block_at_y(8, 3, 8), TALL_GRASS_LOWER);
        assert_eq!(chunk.get_block_at_y(8, 4, 8), TALL_GRASS_UPPER);
    }

    #[test]
    fn random_patch_double_plant_writes_requested_flower_halves() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(3);
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: ROSE_BUSH_LOWER,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: true,
            column_height: None,
            need_water: false,
            place_on: &[],
        });

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert_eq!(chunk.get_block_at_y(8, 3, 8), ROSE_BUSH_LOWER);
        assert_eq!(chunk.get_block_at_y(8, 4, 8), ROSE_BUSH_UPPER);
    }

    #[test]
    fn random_patch_places_sweet_berry_bush_on_grass_whitelist() {
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: SWEET_BERRY_BUSH,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK],
        });
        let mut grass_chunk = flat_grass_chunk();
        let mut grass_random = WorldgenRandom::new(0);

        assert!(feature.place(&mut grass_chunk, &mut grass_random, BlockPos::new(8, 3, 8)));
        assert_eq!(grass_chunk.get_block_at_y(8, 3, 8), SWEET_BERRY_BUSH);

        let mut sand_chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        sand_chunk.set_block_at_y(8, 2, 8, SAND);
        let mut sand_random = WorldgenRandom::new(0);

        assert!(!feature.place(&mut sand_chunk, &mut sand_random, BlockPos::new(8, 3, 8)));
        assert_eq!(sand_chunk.get_block_at_y(8, 3, 8), AIR);
    }

    #[test]
    fn random_patch_small_mushrooms_follow_vanilla_light_and_substrate_gate() {
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: BROWN_MUSHROOM,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[],
        });

        let mut exposed_grass = flat_grass_chunk();
        let mut exposed_random = WorldgenRandom::new(0);
        assert!(!feature.place(
            &mut exposed_grass,
            &mut exposed_random,
            BlockPos::new(8, 3, 8)
        ));
        assert_eq!(exposed_grass.get_block_at_y(8, 3, 8), AIR);

        let mut podzol = flat_grass_chunk();
        podzol.set_block_at_y(8, 2, 8, PODZOL);
        let mut podzol_random = WorldgenRandom::new(0);
        assert!(feature.place(&mut podzol, &mut podzol_random, BlockPos::new(8, 3, 8)));
        assert_eq!(podzol.get_block_at_y(8, 3, 8), BROWN_MUSHROOM);

        let mut mycelium = flat_grass_chunk();
        mycelium.set_block_at_y(8, 2, 8, MYCELIUM);
        let mut mycelium_random = WorldgenRandom::new(0);
        assert!(feature.place(&mut mycelium, &mut mycelium_random, BlockPos::new(8, 3, 8)));
        assert_eq!(mycelium.get_block_at_y(8, 3, 8), BROWN_MUSHROOM);

        let mut shaded_grass = flat_grass_chunk();
        for y in 4..=6 {
            shaded_grass.set_block_at_y(8, y, 8, SPRUCE_LEAVES);
        }
        let mut shaded_random = WorldgenRandom::new(0);
        assert!(feature.place(
            &mut shaded_grass,
            &mut shaded_random,
            BlockPos::new(8, 3, 8)
        ));
        assert_eq!(shaded_grass.get_block_at_y(8, 3, 8), BROWN_MUSHROOM);
    }

    #[test]
    fn random_patch_column_placer_supports_cactus_and_water_gated_sugar_cane() {
        let mut cactus_chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        cactus_chunk.set_block_at_y(8, 2, 8, SAND);
        let mut cactus_random = WorldgenRandom::new(4);
        let cactus = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: CACTUS,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: false,
            column_height: Some(IntProvider::biased_to_bottom(1, 3)),
            need_water: false,
            place_on: &[],
        });

        assert!(cactus.place(
            &mut cactus_chunk,
            &mut cactus_random,
            BlockPos::new(8, 3, 8)
        ));
        let cactus_height = (3..=5)
            .filter(|y| cactus_chunk.get_block_at_y(8, *y, 8) == CACTUS)
            .count();
        assert!((1..=3).contains(&cactus_height));

        let mut dry_chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        dry_chunk.set_block_at_y(8, 2, 8, SAND);
        let sugar_cane = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: SUGAR_CANE,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: false,
            column_height: Some(IntProvider::constant(2)),
            need_water: true,
            place_on: &[],
        });
        let mut dry_random = WorldgenRandom::new(0);

        assert!(!sugar_cane.place(&mut dry_chunk, &mut dry_random, BlockPos::new(8, 3, 8)));

        let mut wet_chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        wet_chunk.set_block_at_y(8, 2, 8, SAND);
        wet_chunk.set_block_at_y(9, 2, 8, WATER);
        let mut wet_random = WorldgenRandom::new(0);

        assert!(sugar_cane.place(&mut wet_chunk, &mut wet_random, BlockPos::new(8, 3, 8)));
        assert_eq!(wet_chunk.get_block_at_y(8, 3, 8), SUGAR_CANE);
        assert_eq!(wet_chunk.get_block_at_y(8, 4, 8), SUGAR_CANE);
    }

    #[test]
    fn random_patch_places_lily_pad_on_projected_water_surface() {
        let lily_pad = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: LILY_PAD,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: true,
            can_replace: false,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[],
        });

        let mut ocean_chunk = flat_ocean_chunk();
        let mut ocean_random = WorldgenRandom::new(0);

        assert!(lily_pad.place(&mut ocean_chunk, &mut ocean_random, BlockPos::new(8, 0, 8)));
        assert_eq!(ocean_chunk.get_block_at_y(8, 11, 8), LILY_PAD);

        let mut grass_chunk = flat_grass_chunk();
        let mut grass_random = WorldgenRandom::new(0);

        assert!(!lily_pad.place(&mut grass_chunk, &mut grass_random, BlockPos::new(8, 0, 8)));
        assert_eq!(grass_chunk.get_block_at_y(8, 3, 8), AIR);
    }

    #[test]
    fn seagrass_feature_places_single_or_tall_water_plant_on_ocean_floor() {
        let mut short_chunk = flat_ocean_chunk();
        let mut short_random = WorldgenRandom::new(1);
        let short = ConfiguredFeature::seagrass(SeagrassConfiguration::new(0.0));
        assert!(short.place(&mut short_chunk, &mut short_random, BlockPos::new(8, 0, 8)));
        assert_eq!(count_blocks(&short_chunk, SEAGRASS), 1);

        let mut tall_chunk = flat_ocean_chunk();
        let mut tall_random = WorldgenRandom::new(1);
        let tall = ConfiguredFeature::seagrass(SeagrassConfiguration::new(1.0));
        assert!(tall.place(&mut tall_chunk, &mut tall_random, BlockPos::new(8, 0, 8)));
        assert_eq!(count_blocks(&tall_chunk, TALL_SEAGRASS_LOWER), 1);
        assert_eq!(count_blocks(&tall_chunk, TALL_SEAGRASS_UPPER), 1);
    }

    #[test]
    fn kelp_feature_places_body_column_with_head_in_water() {
        let mut chunk = flat_ocean_chunk();
        let mut random = WorldgenRandom::new(2);
        let feature = ConfiguredFeature::kelp();

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert_eq!(count_blocks(&chunk, KELP), 1);
        assert!(count_blocks(&chunk, KELP_PLANT) > 0);
    }

    #[test]
    fn sea_pickle_feature_places_waterlogged_pickles_on_ocean_floor() {
        let mut chunk = flat_ocean_chunk();
        let mut random = WorldgenRandom::new(3);
        let feature = ConfiguredFeature::sea_pickle(CountConfiguration::new(20));

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert!(
            [SEA_PICKLE_1, SEA_PICKLE_2, SEA_PICKLE_3, SEA_PICKLE_4]
                .iter()
                .any(|state| count_blocks(&chunk, *state) > 0)
        );
    }

    #[test]
    fn blue_ice_feature_spreads_from_water_next_to_packed_ice() {
        let mut chunk = flat_ocean_chunk();
        chunk.set_block_at_y(9, 5, 8, PACKED_ICE);
        let mut random = WorldgenRandom::new(4);
        let feature = ConfiguredFeature::blue_ice();

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 5, 8)));
        assert!(count_blocks(&chunk, BLUE_ICE) > 0);
    }

    #[test]
    fn coral_features_place_live_coral_blocks_in_water() {
        for (shape, seed) in [
            (CoralShape::Tree, 4),
            (CoralShape::Claw, 5),
            (CoralShape::Mushroom, 6),
        ] {
            let mut chunk = flat_ocean_chunk();
            let mut random = WorldgenRandom::new(seed);
            let feature = ConfiguredFeature::coral(shape);

            assert!(
                feature.place(&mut chunk, &mut random, BlockPos::new(8, 2, 8)),
                "{shape:?}"
            );
            assert!(
                [
                    TUBE_CORAL_BLOCK,
                    BRAIN_CORAL_BLOCK,
                    BUBBLE_CORAL_BLOCK,
                    FIRE_CORAL_BLOCK,
                    HORN_CORAL_BLOCK,
                ]
                .iter()
                .any(|state| count_blocks(&chunk, *state) > 0),
                "{shape:?}"
            );
        }
    }

    #[test]
    fn glow_lichen_feature_places_against_stone_face() {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        chunk.set_block_at_y(8, 9, 8, STONE);
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld());

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 8, 8)));
        assert_eq!(chunk.get_block_at_y(8, 8, 8), GLOW_LICHEN);
        assert_eq!(chunk.glow_lichen_faces_at_y(8, 8, 8), Direction::Up.bit());
    }

    #[test]
    fn glow_lichen_spread_can_write_visible_neighbor_block() {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        chunk.set_glow_lichen_faces_at_y(8, 8, 8, Direction::North.bit());
        chunk.set_block_at_y(9, 8, 7, STONE);

        assert!(glow_lichen::spread_glow_lichen_from_face_toward_direction(
            &mut chunk,
            BlockPos::new(8, 8, 8),
            Direction::North,
            Direction::East,
        ));
        assert_eq!(chunk.get_block_at_y(9, 8, 8), GLOW_LICHEN);
    }

    #[test]
    fn glow_lichen_spread_stops_when_toward_face_already_exists() {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        chunk.set_glow_lichen_faces_at_y(8, 8, 8, Direction::North.bit() | Direction::East.bit());
        chunk.set_block_at_y(9, 8, 7, STONE);

        assert!(!glow_lichen::spread_glow_lichen_from_face_toward_direction(
            &mut chunk,
            BlockPos::new(8, 8, 8),
            Direction::North,
            Direction::East,
        ));
        assert_eq!(chunk.get_block_at_y(9, 8, 8), AIR);
        assert_eq!(
            chunk.glow_lichen_faces_at_y(8, 8, 8),
            Direction::North.bit() | Direction::East.bit(),
        );
    }

    #[test]
    fn lake_feature_uses_cave_air_for_upper_cavity() {
        let mut chunk = solid_stone_chunk();
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::lake(LakeConfiguration::new(WATER));

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(0, 20, 0)));
        assert!(count_blocks(&chunk, CAVE_AIR) > 0);
        assert!(count_blocks(&chunk, WATER) > 0);
    }

    #[test]
    fn placed_feature_applies_count_then_square_decorators() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(12_345);
        let placed = PlacedFeature::new(
            DecorationStep::VegetalDecoration,
            ConfiguredFeature::random_patch(RandomPatchConfiguration {
                state: POPPY,
                weighted_states: &[],
                state_provider: RandomPatchStateProvider::Simple,
                tries: 8,
                xspread: 1,
                yspread: 1,
                zspread: 1,
                project: true,
                can_replace: false,
                double_plant: false,
                column_height: None,
                need_water: false,
                place_on: &[GRASS_BLOCK],
            }),
            vec![ConfiguredDecorator::count(2), ConfiguredDecorator::square()],
        );

        assert!(placed.place(&mut chunk, &mut random, BlockPos::new(0, 0, 0)));
        assert!(chunk.blocks.iter().any(|block_id| *block_id == POPPY));
    }

    #[test]
    fn feature_world_heightmaps_follow_reduced_java_predicates() {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        chunk.set_block_at_y(8, 2, 8, DIRT);
        chunk.set_block_at_y(8, 3, 8, WATER);
        chunk.set_block_at_y(8, 4, 8, GRASS);
        chunk.set_block_at_y(8, 5, 8, SPRUCE_LEAVES);
        chunk.set_block_at_y(8, 6, 8, SNOW);

        assert_eq!(chunk.height_at(HeightmapType::WorldSurface, 8, 8), Some(7));
        assert_eq!(chunk.height_at(HeightmapType::OceanFloor, 8, 8), Some(6));
        assert_eq!(
            chunk.height_at(HeightmapType::MotionBlocking, 8, 8),
            Some(6)
        );
        assert_eq!(
            chunk.height_at(HeightmapType::MotionBlockingNoLeaves, 8, 8),
            Some(4)
        );
    }

    #[test]
    fn primed_worldgen_heightmaps_ignore_later_feature_writes() {
        let mut chunk = MutableChunkBlockBuffer::new(0, 0, 0, 16);
        chunk.set_block_at_y(8, 2, 8, DIRT);
        chunk.prime_worldgen_heightmaps();
        chunk.set_block_at_y(8, 5, 8, SPRUCE_LEAVES);

        assert_eq!(
            chunk.height_at(HeightmapType::WorldSurfaceWg, 8, 8),
            Some(3)
        );
        assert_eq!(chunk.height_at(HeightmapType::OceanFloorWg, 8, 8), Some(3));
        assert_eq!(chunk.height_at(HeightmapType::WorldSurface, 8, 8), Some(6));
        assert_eq!(chunk.height_at(HeightmapType::OceanFloor, 8, 8), Some(6));
    }

    #[test]
    fn placed_feature_uses_world_heightmap_and_water_depth_decorators() {
        let feature = ConfiguredFeature::simple_block(
            SimpleBlockConfiguration::new(POPPY)
                .place_on(&[GRASS_BLOCK])
                .place_in(&[AIR]),
        );
        let placed = PlacedFeature::new(
            DecorationStep::VegetalDecoration,
            feature,
            vec![
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ],
        );
        let origin = BlockPos::new(8, 0, 8);

        let mut dry_chunk = flat_grass_chunk();
        let mut dry_random = WorldgenRandom::new(12_345);
        assert!(placed.place(&mut dry_chunk, &mut dry_random, origin));
        assert_eq!(dry_chunk.get_block_at_y(8, 3, 8), POPPY);

        let mut wet_chunk = flat_grass_chunk();
        wet_chunk.set_block_at_y(8, 3, 8, WATER);
        let mut wet_random = WorldgenRandom::new(12_345);
        assert!(!placed.place(&mut wet_chunk, &mut wet_random, origin));
        assert_eq!(wet_chunk.get_block_at_y(8, 3, 8), WATER);
        assert_eq!(dry_random.get_count(), wet_random.get_count());
    }

    #[test]
    fn decorated_configured_feature_applies_nested_decorators() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::decorated(DecoratedFeatureConfiguration::new(
            ConfiguredFeature::simple_block(
                SimpleBlockConfiguration::new(DANDELION)
                    .place_on(&[GRASS_BLOCK])
                    .place_in(&[AIR]),
            ),
            [ConfiguredDecorator::heightmap(HeightmapType::OceanFloor)],
        ));

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert_eq!(chunk.get_block_at_y(8, 3, 8), DANDELION);
        assert_eq!(random.get_count(), 0);
    }

    #[test]
    fn freeze_top_layer_places_snow_on_cold_motion_blocking_surface() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(12_345);
        let biomes = ConstantFeatureBiomeResolver::new(get_layered_biome_by_id(12));
        let feature = ConfiguredFeature::freeze_top_layer();

        assert!(feature.place_with_biomes(
            &mut chunk,
            &biomes,
            &mut random,
            BlockPos::new(0, 0, 0),
        ));
        assert_eq!(chunk.get_block_at_y(8, 3, 8), SNOW);
        assert_eq!(random.get_count(), 0);
    }

    #[test]
    fn freeze_top_layer_freezes_surface_water_before_snow_check() {
        let mut chunk = flat_grass_chunk();
        chunk.set_block_at_y(8, 2, 8, WATER);
        let mut random = WorldgenRandom::new(12_345);
        let biomes = ConstantFeatureBiomeResolver::new(get_layered_biome_by_id(12));
        let feature = ConfiguredFeature::freeze_top_layer();

        assert!(feature.place_with_biomes(
            &mut chunk,
            &biomes,
            &mut random,
            BlockPos::new(0, 0, 0),
        ));
        assert_eq!(chunk.get_block_at_y(8, 2, 8), ICE);
        assert_eq!(chunk.get_block_at_y(8, 3, 8), SNOW);
    }

    #[test]
    fn ore_feature_replaces_natural_stone_blob() {
        let mut chunk = solid_stone_chunk();
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::ore(OreConfiguration::natural_stone(DIORITE, 33));

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 32, 8)));
        assert!(count_blocks(&chunk, DIORITE) > 0);
        assert!(count_blocks(&chunk, STONE) < CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 64);
    }

    #[test]
    fn ore_feature_uses_ocean_floor_wg_height_gate() {
        let mut world = OreHeightmapProbeWorld::default();
        let mut random = WorldgenRandom::new(12_345);
        let feature = ConfiguredFeature::ore(OreConfiguration::natural_stone(DIORITE, 33));

        assert!(feature.place(&mut world, &mut random, BlockPos::new(8, 32, 8)));
        assert!(world.writes > 0);
        assert_eq!(world.world_surface_queries, 0);
        assert_eq!(world.height_queries, vec![HeightmapType::OceanFloorWg]);
    }

    #[test]
    fn placed_feature_interleaves_decorator_branches_with_feature_random() {
        let origin = BlockPos::new(0, 0, 0);
        let range =
            HeightProvider::uniform(VerticalAnchor::absolute(8), VerticalAnchor::absolute(48));
        let ore = ConfiguredFeature::ore(OreConfiguration::natural_stone(DIORITE, 33));
        let placed = PlacedFeature::new(
            DecorationStep::UndergroundOres,
            ore.clone(),
            vec![
                ConfiguredDecorator::count(2),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::range(range),
            ],
        );
        let mut placed_chunk = solid_stone_chunk();
        let mut placed_random = WorldgenRandom::new(12_345);

        placed.place(&mut placed_chunk, &mut placed_random, origin);

        let mut manual_chunk = solid_stone_chunk();
        let mut manual_random = WorldgenRandom::new(12_345);
        let context = DecorationContext::new(manual_chunk.min_y, manual_chunk.height);
        for _ in 0..2 {
            for square_pos in
                ConfiguredDecorator::square().get_positions(&context, &mut manual_random, origin)
            {
                for range_pos in ConfiguredDecorator::range(range).get_positions(
                    &context,
                    &mut manual_random,
                    square_pos,
                ) {
                    ore.place(&mut manual_chunk, &mut manual_random, range_pos);
                }
            }
        }

        assert_eq!(placed_chunk.blocks, manual_chunk.blocks);
        assert_eq!(placed_random.get_count(), manual_random.get_count());
    }

    #[test]
    fn basic_tree_places_configured_log_and_leaf_blocks() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(1);
        let feature = ConfiguredFeature::basic_tree(BasicTreeConfiguration::birch());

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 0, 8)));
        assert!(chunk.blocks.iter().any(|block_id| *block_id == BIRCH_LOG));
        assert!(
            chunk
                .blocks
                .iter()
                .any(|block_id| *block_id == BIRCH_LEAVES)
        );
    }

    #[test]
    fn dark_oak_tree_places_two_by_two_trunk_and_dark_leaves() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(2);
        let feature = ConfiguredFeature::tree(TreeConfiguration::dark_oak());

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert!(count_blocks(&chunk, DARK_OAK_LOG) >= 4);
        assert!(count_blocks(&chunk, DARK_OAK_LEAVES) > 0);
    }

    #[test]
    fn acacia_tree_places_forking_trunk_and_flat_canopy() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(5);
        let feature = ConfiguredFeature::tree(TreeConfiguration::acacia());

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert!(count_blocks(&chunk, ACACIA_LOG) > 0);
        assert!(count_blocks(&chunk, ACACIA_LEAVES) > 0);
    }

    #[test]
    fn jungle_tree_places_jungle_log_and_leaves() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(6);
        let feature = ConfiguredFeature::tree(TreeConfiguration::jungle());

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert!(count_blocks(&chunk, JUNGLE_LOG) > 0);
        assert!(count_blocks(&chunk, JUNGLE_LEAVES) > 0);
    }

    #[test]
    fn mega_jungle_tree_uses_java_two_by_two_trunk_branches_and_foliage() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(6);
        let config = TreeConfiguration::mega_jungle();
        let feature = ConfiguredFeature::tree(config);
        let base = BlockPos::new(6, 3, 6);

        assert_eq!(
            config.trunk_placer,
            TrunkPlacerConfiguration::mega_jungle(10, 2, 19)
        );
        assert_eq!(
            config.foliage_placer,
            FoliagePlacerConfiguration::MegaJungle {
                radius: IntProvider::constant(2),
                offset: IntProvider::constant(0),
                height: 2,
            }
        );
        assert!(feature.place(&mut chunk, &mut random, base));
        assert!(has_two_by_two_log_square_at(&chunk, base, JUNGLE_LOG));
        assert!(count_logs_outside_two_by_two_column(&chunk, base, JUNGLE_LOG) >= 5);
        assert!(count_blocks(&chunk, JUNGLE_LEAVES) >= 120);
    }

    #[test]
    fn jungle_bush_uses_java_bush_foliage_shape() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(6);
        let config = TreeConfiguration::jungle_bush();
        let feature = ConfiguredFeature::tree(config);
        let base = BlockPos::new(8, 3, 8);

        assert_eq!(config.log, JUNGLE_LOG);
        assert_eq!(config.leaves, OAK_LEAVES);
        assert_eq!(
            config.trunk_placer,
            TrunkPlacerConfiguration::straight(1, 0, 0)
        );
        assert_eq!(
            config.foliage_placer,
            FoliagePlacerConfiguration::Bush {
                radius: IntProvider::constant(2),
                offset: IntProvider::constant(1),
                height: 2,
            }
        );

        assert!(feature.place(&mut chunk, &mut random, base));
        assert_eq!(chunk.get_block_at_y(base.x, base.y, base.z), JUNGLE_LOG);
        assert_eq!(count_blocks(&chunk, JUNGLE_LOG), 1);
        assert_eq!(count_blocks_at_y(&chunk, base.y + 2, OAK_LEAVES), 1);
        assert!(count_blocks_at_y(&chunk, base.y + 1, OAK_LEAVES) >= 5);
        assert!(count_blocks_at_y(&chunk, base.y, OAK_LEAVES) >= 20);
        assert_eq!(chunk.get_block_at_y(base.x + 2, base.y, base.z), OAK_LEAVES);
        assert_eq!(chunk.get_block_at_y(base.x, base.y, base.z + 2), OAK_LEAVES);
    }

    #[test]
    fn jungle_tree_decorators_place_cocoa_and_face_vines() {
        let cocoa_states = [
            COCOA_AGE0_NORTH,
            COCOA_AGE0_EAST,
            COCOA_AGE0_SOUTH,
            COCOA_AGE0_WEST,
            COCOA_AGE1_NORTH,
            COCOA_AGE1_EAST,
            COCOA_AGE1_SOUTH,
            COCOA_AGE1_WEST,
            COCOA_AGE2_NORTH,
            COCOA_AGE2_EAST,
            COCOA_AGE2_SOUTH,
            COCOA_AGE2_WEST,
        ];
        let vine_states = [VINE_UP, VINE_NORTH, VINE_EAST, VINE_SOUTH, VINE_WEST];
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(6);
        let config = TreeConfiguration::jungle().with_cocoa_probability(1.0);
        let feature = ConfiguredFeature::tree(config);

        assert_eq!(TreeConfiguration::jungle().cocoa_probability, Some(0.2));
        assert!(TreeConfiguration::jungle().trunk_vines);
        assert!(TreeConfiguration::jungle().leaf_vines);
        assert!(TreeConfiguration::mega_jungle().trunk_vines);
        assert!(TreeConfiguration::mega_jungle().leaf_vines);
        assert_eq!(TreeConfiguration::mega_jungle().cocoa_probability, None);
        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert!(count_any_blocks(&chunk, &cocoa_states) > 0);
        assert!(count_any_blocks(&chunk, &vine_states) > 0);
    }

    #[test]
    fn bamboo_feature_places_java_height_column() {
        let mut chunk = flat_grass_chunk();
        let mut random = WorldgenRandom::new(7);
        let feature = ConfiguredFeature::bamboo(BambooConfiguration::new(1.0));

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert!(count_blocks(&chunk, BAMBOO) >= 3);
        assert_eq!(count_blocks(&chunk, BAMBOO_TOP_SMALL), 1);
        assert_eq!(count_blocks(&chunk, BAMBOO_TOP_LARGE), 1);
        assert_eq!(count_blocks(&chunk, BAMBOO_FINAL_LARGE), 1);
    }

    #[test]
    fn melon_patch_places_on_grass_and_honors_can_replace() {
        let feature = ConfiguredFeature::random_patch(RandomPatchConfiguration {
            state: MELON,
            weighted_states: &[],
            state_provider: RandomPatchStateProvider::Simple,
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: true,
            double_plant: false,
            column_height: None,
            need_water: false,
            place_on: &[GRASS_BLOCK],
        });

        let mut air_chunk = flat_grass_chunk();
        let mut air_random = WorldgenRandom::new(8);
        assert!(feature.place(&mut air_chunk, &mut air_random, BlockPos::new(8, 3, 8)));
        assert_eq!(air_chunk.get_block_at_y(8, 3, 8), MELON);

        let mut grass_chunk = flat_grass_chunk();
        grass_chunk.set_block_at_y(8, 3, 8, GRASS);
        let mut grass_random = WorldgenRandom::new(8);
        assert!(feature.place(&mut grass_chunk, &mut grass_random, BlockPos::new(8, 3, 8)));
        assert_eq!(grass_chunk.get_block_at_y(8, 3, 8), MELON);
    }

    #[test]
    fn vines_feature_places_against_solid_neighbor_only() {
        let feature = ConfiguredFeature::vines();

        let mut unsupported = flat_grass_chunk();
        let mut unsupported_random = WorldgenRandom::new(9);
        assert!(!feature.place(
            &mut unsupported,
            &mut unsupported_random,
            BlockPos::new(8, 3, 8)
        ));
        assert_eq!(unsupported.get_block_at_y(8, 3, 8), AIR);

        let mut supported = flat_grass_chunk();
        supported.set_block_at_y(9, 3, 8, STONE);
        let mut supported_random = WorldgenRandom::new(9);
        assert!(feature.place(
            &mut supported,
            &mut supported_random,
            BlockPos::new(8, 3, 8)
        ));
        assert_eq!(supported.get_block_at_y(8, 3, 8), VINE_EAST);

        let mut supported_above = flat_grass_chunk();
        supported_above.set_block_at_y(8, 4, 8, STONE);
        let mut supported_above_random = WorldgenRandom::new(9);
        assert!(feature.place(
            &mut supported_above,
            &mut supported_above_random,
            BlockPos::new(8, 3, 8)
        ));
        assert_eq!(supported_above.get_block_at_y(8, 3, 8), VINE_UP);

        let mut occupied = flat_grass_chunk();
        occupied.set_block_at_y(8, 3, 8, GRASS);
        occupied.set_block_at_y(9, 3, 8, STONE);
        let mut occupied_random = WorldgenRandom::new(9);
        assert!(!feature.place(&mut occupied, &mut occupied_random, BlockPos::new(8, 3, 8)));
        assert_eq!(occupied.get_block_at_y(8, 3, 8), GRASS);
    }

    #[test]
    fn huge_mushrooms_place_cap_and_stem_blocks() {
        let mut brown_chunk = flat_grass_chunk();
        let mut brown_random = WorldgenRandom::new(3);
        let brown = ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown());

        assert!(brown.place(&mut brown_chunk, &mut brown_random, BlockPos::new(8, 3, 8)));
        assert!(count_blocks(&brown_chunk, BROWN_MUSHROOM_BLOCK) > 0);
        assert!(count_blocks(&brown_chunk, MUSHROOM_STEM) > 0);

        let mut red_chunk = flat_grass_chunk();
        let mut red_random = WorldgenRandom::new(4);
        let red = ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red());

        assert!(red.place(&mut red_chunk, &mut red_random, BlockPos::new(8, 3, 8)));
        assert!(count_blocks(&red_chunk, RED_MUSHROOM_BLOCK) > 0);
        assert!(count_blocks(&red_chunk, MUSHROOM_STEM) > 0);
    }

    #[test]
    fn feature_region_allows_neighbor_tree_to_spill_into_center_chunk() {
        let mut region = FeatureRegion::with_radii(
            -1,
            0,
            1,
            1,
            vec![flat_grass_chunk_at(-1, 0), flat_grass_chunk_at(0, 0)],
        );
        let mut random = WorldgenRandom::new(1);
        let feature = ConfiguredFeature::basic_tree(BasicTreeConfiguration::oak());

        assert!(feature.place(&mut region, &mut random, BlockPos::new(-1, 0, 8)));

        let center = region.chunk(0, 0).expect("center chunk exists");
        assert!(center.blocks.iter().any(|block_id| *block_id == OAK_LEAVES));
        assert_eq!(region.metrics().blocked_block_writes, 0);
    }

    #[test]
    fn feature_region_blocks_writes_beyond_cutoff() {
        let mut region = FeatureRegion::with_radii(
            0,
            0,
            2,
            1,
            vec![flat_grass_chunk_at(0, 0), flat_grass_chunk_at(2, 0)],
        );

        assert!(!region.set_block_world(BlockPos::new(32, 3, 0), POPPY));
        assert_eq!(
            region.metrics(),
            FeatureRegionMetrics {
                block_write_attempts: 1,
                blocked_block_writes: 1,
                ..FeatureRegionMetrics::default()
            }
        );
        assert_eq!(region.chunk(2, 0).unwrap().get_block_at_y(0, 3, 0), AIR);
    }

    #[test]
    #[should_panic(expected = "outside dependency window")]
    fn feature_region_rejects_reads_outside_dependency_window() {
        let mut region = FeatureRegion::with_radii(0, 0, 1, 1, vec![flat_grass_chunk()]);

        let _ = region.block_at_world(BlockPos::new(32, 3, 0));
    }

    #[test]
    fn biome_feature_tables_select_distinct_visible_families() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let forest = overworld_features_for_biome(get_layered_biome_by_id(4));
        let birch = overworld_features_for_biome(get_layered_biome_by_id(27));
        let taiga = overworld_features_for_biome(get_layered_biome_by_id(5));
        let desert = overworld_features_for_biome(get_layered_biome_by_id(2));

        assert!(plains.iter().any(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::BasicTree(BasicTreeConfiguration { log: OAK_LOG, .. })
            )
        }));
        assert!(birch.iter().any(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::Tree(TreeConfiguration { log: BIRCH_LOG, .. })
            )
        }));
        assert!(
            forest
                .iter()
                .any(|feature| matches!(feature.feature, ConfiguredFeature::RandomSelector(_)))
        );
        assert!(
            taiga
                .iter()
                .any(|feature| matches!(feature.feature, ConfiguredFeature::RandomSelector(_)))
        );
        assert!(desert.iter().any(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                    state: DEAD_BUSH,
                    ..
                })
            )
        }));
    }

    #[test]
    fn biome_feature_tables_include_desert_badlands_and_swamp_extra_vegetation() {
        let desert = overworld_features_for_biome(get_layered_biome_by_id(2));
        let badlands = overworld_features_for_biome(get_layered_biome_by_id(37));
        let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));

        assert!(has_random_patch(&desert, SUGAR_CANE, 60));
        assert!(has_pumpkin_patch(&desert));
        assert!(has_random_patch(&desert, CACTUS, 10));
        assert!(has_default_water_spring(&desert));
        assert!(has_default_lava_spring(&desert));
        assert!(has_random_patch(&badlands, SUGAR_CANE, 13));
        assert!(has_pumpkin_patch(&badlands));
        assert!(has_random_patch(&badlands, CACTUS, 5));
        assert!(has_default_water_spring(&badlands));
        assert!(has_default_lava_spring(&badlands));
        assert!(has_random_patch(&swamp, SUGAR_CANE, 20));
        assert!(has_pumpkin_patch(&swamp));
        assert!(has_random_patch(&swamp, LILY_PAD, 4));
        assert!(has_default_water_spring(&swamp));
        assert!(has_default_lava_spring(&swamp));
    }

    #[test]
    fn badlands_feature_tables_match_java_wooded_tree_split() {
        for biome_id in [37, 39, 165, 167] {
            let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
            assert_badlands_common_features(&features);
            assert!(
                !has_badlands_tree_feature(&features),
                "non-wooded badlands biome {biome_id} should not include TREES_BADLANDS"
            );
        }

        for biome_id in [38, 166] {
            let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
            assert_badlands_common_features(&features);
            assert!(
                has_badlands_tree_feature(&features),
                "wooded badlands biome {biome_id} should include TREES_BADLANDS"
            );
        }
    }

    #[test]
    fn biome_feature_tables_include_java_normal_mushroom_patches_for_current_lanes() {
        for biome_id in [
            1, 3, 4, 5, 6, 12, 13, 14, 15, 18, 19, 20, 21, 22, 23, 27, 28, 29, 30, 31, 32, 33, 34,
            35, 36, 37, 38, 39, 129, 131, 132, 133, 134, 140, 149, 151, 155, 156, 157, 158, 160,
            161, 162, 163, 164, 165, 166, 167, 168, 169,
        ] {
            let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
            assert!(has_normal_mushroom_patch(&features, BROWN_MUSHROOM, 4));
            assert!(has_normal_mushroom_patch(&features, RED_MUSHROOM, 8));
        }

        let fallback = overworld_features_for_biome(BiomeDefinition::new(
            999,
            "minecraft:test_fallback",
            0.0,
            0.0,
        ));
        assert!(has_normal_mushroom_patch(&fallback, BROWN_MUSHROOM, 4));
        assert!(has_normal_mushroom_patch(&fallback, RED_MUSHROOM, 8));
    }

    #[test]
    fn biome_feature_tables_include_java_taiga_mushroom_patches_for_current_lanes() {
        for biome_id in [5, 14, 15, 19, 30, 31, 133, 158] {
            let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
            assert!(has_taiga_mushroom_patch(
                &features,
                BROWN_MUSHROOM,
                4,
                None,
                false
            ));
            assert!(has_taiga_mushroom_patch(
                &features,
                RED_MUSHROOM,
                8,
                None,
                true
            ));
        }

        for biome_id in [32, 33, 160, 161] {
            let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
            assert!(has_taiga_mushroom_patch(
                &features,
                BROWN_MUSHROOM,
                4,
                Some(3),
                false
            ));
            assert!(has_taiga_mushroom_patch(
                &features,
                RED_MUSHROOM,
                8,
                Some(3),
                true
            ));
        }
    }

    #[test]
    fn biome_feature_tables_include_java_default_extra_vegetation_for_current_lanes() {
        for biome_id in [
            1, 3, 4, 5, 12, 13, 14, 15, 18, 19, 20, 21, 22, 23, 27, 28, 29, 30, 31, 32, 33, 34, 35,
            36, 129, 131, 132, 133, 140, 149, 151, 155, 156, 157, 158, 160, 161, 162, 163, 164,
            168, 169,
        ] {
            let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
            assert!(has_random_patch(&features, SUGAR_CANE, 10));
            assert!(has_pumpkin_patch(&features));
        }

        let fallback = overworld_features_for_biome(BiomeDefinition::new(
            999,
            "minecraft:test_fallback",
            0.0,
            0.0,
        ));
        assert!(has_random_patch(&fallback, SUGAR_CANE, 10));
        assert!(has_pumpkin_patch(&fallback));
    }

    #[test]
    fn biome_feature_tables_include_java_default_springs_for_current_land_lanes() {
        for biome_id in [
            1, 2, 3, 4, 5, 6, 7, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 25, 26, 27,
            28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 129, 130, 131, 132, 133, 134, 140, 149,
            151, 155, 156, 157, 158, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169,
        ] {
            let features = overworld_features_for_biome(get_layered_biome_by_id(biome_id));
            assert!(has_default_water_spring(&features));
            assert!(has_default_lava_spring(&features));
        }

        let fallback = overworld_features_for_biome(BiomeDefinition::new(
            999,
            "minecraft:test_fallback",
            0.0,
            0.0,
        ));
        assert!(has_default_water_spring(&fallback));
        assert!(has_default_lava_spring(&fallback));
    }

    #[test]
    fn river_and_beach_feature_tables_include_java_default_vegetation_subset() {
        let river = overworld_features_for_biome(get_layered_biome_by_id(7));
        let frozen_river = overworld_features_for_biome(get_layered_biome_by_id(11));
        let beach = overworld_features_for_biome(get_layered_biome_by_id(16));
        let stone_shore = overworld_features_for_biome(get_layered_biome_by_id(25));
        let snowy_beach = overworld_features_for_biome(get_layered_biome_by_id(26));

        assert!(has_water_tree_feature(&river));
        assert!(has_water_tree_feature(&frozen_river));
        assert!(!has_water_tree_feature(&beach));
        assert!(!has_water_tree_feature(&stone_shore));
        assert!(!has_water_tree_feature(&snowy_beach));

        for features in [&river, &frozen_river, &beach, &stone_shore, &snowy_beach] {
            assert!(has_default_flower_feature(features));
            assert!(has_default_grass_patch_feature(features));
            assert!(has_normal_mushroom_patch(features, BROWN_MUSHROOM, 4));
            assert!(has_normal_mushroom_patch(features, RED_MUSHROOM, 8));
            assert!(has_random_patch(features, SUGAR_CANE, 10));
            assert!(has_pumpkin_patch(features));
        }

        assert!(has_seagrass_feature(&river, 48, 0.4));
        assert!(!has_seagrass_feature(&frozen_river, 48, 0.4));
    }

    #[test]
    fn swamp_feature_table_includes_java_blue_orchid_patch() {
        let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));
        let blue_orchid = swamp
            .iter()
            .find(|feature| {
                matches!(
                    feature.feature,
                    ConfiguredFeature::Flower(RandomPatchConfiguration {
                        state: BLUE_ORCHID,
                        ..
                    })
                )
            })
            .expect("swamp blue orchid feature");

        assert_eq!(
            blue_orchid.decorators,
            vec![
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ConfiguredDecorator::spread_32_above(),
            ]
        );
        match &blue_orchid.feature {
            ConfiguredFeature::Flower(config) => {
                assert_eq!(
                    *config,
                    RandomPatchConfiguration {
                        state: BLUE_ORCHID,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 64,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: true,
                        can_replace: false,
                        double_plant: false,
                        column_height: None,
                        need_water: false,
                        place_on: &[],
                    }
                );
            }
            other => panic!("expected blue orchid flower feature, got {other:?}"),
        }
    }

    #[test]
    fn swamp_feature_table_includes_java_small_mushroom_patches() {
        let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));
        assert_swamp_mushroom_patch(
            &swamp,
            BROWN_MUSHROOM,
            vec![
                ConfiguredDecorator::count(8),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ConfiguredDecorator::chance(4),
            ],
        );
        assert_swamp_mushroom_patch(
            &swamp,
            RED_MUSHROOM,
            vec![
                ConfiguredDecorator::count(8),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                ConfiguredDecorator::chance(8),
            ],
        );
    }

    fn assert_swamp_mushroom_patch(
        swamp: &[PlacedFeature],
        state: RawBlockId,
        expected_decorators: Vec<ConfiguredDecorator>,
    ) {
        let feature = swamp
            .iter()
            .find(|feature| {
                matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: patch_state,
                        ..
                    }) if patch_state == state
                )
            })
            .expect("swamp mushroom feature");

        assert_eq!(feature.decorators, expected_decorators);
        match &feature.feature {
            ConfiguredFeature::RandomPatch(config) => {
                assert_eq!(
                    *config,
                    RandomPatchConfiguration {
                        state,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 64,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: false,
                        can_replace: false,
                        double_plant: false,
                        column_height: None,
                        need_water: false,
                        place_on: &[],
                    }
                );
            }
            other => panic!("expected swamp mushroom random patch, got {other:?}"),
        }
    }

    #[test]
    fn swamp_feature_table_includes_java_waterlily_patch() {
        let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));
        let waterlily = swamp
            .iter()
            .find(|feature| {
                matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: LILY_PAD,
                        ..
                    })
                )
            })
            .expect("swamp waterlily feature");

        assert_eq!(
            waterlily.decorators,
            vec![
                ConfiguredDecorator::count(4),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ]
        );
        match &waterlily.feature {
            ConfiguredFeature::RandomPatch(config) => {
                assert_eq!(
                    *config,
                    RandomPatchConfiguration {
                        state: LILY_PAD,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 10,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: true,
                        can_replace: false,
                        double_plant: false,
                        column_height: None,
                        need_water: false,
                        place_on: &[],
                    }
                );
            }
            other => panic!("expected waterlily random patch, got {other:?}"),
        }
    }

    #[test]
    fn warm_ocean_feature_table_includes_coral_and_sea_pickles() {
        let warm_ocean = overworld_features_for_biome(get_layered_biome_by_id(44));

        let coral = warm_ocean
            .iter()
            .find(|feature| matches!(feature.feature, ConfiguredFeature::SimpleRandomSelector(_)))
            .expect("warm ocean coral vegetation feature");
        assert_eq!(coral.step, DecorationStep::VegetalDecoration);
        assert_eq!(
            coral.decorators,
            vec![
                ConfiguredDecorator::count_noise_biased(20, 400.0, 0.0),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
            ]
        );
        match &coral.feature {
            ConfiguredFeature::SimpleRandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![
                        ConfiguredFeature::coral(CoralShape::Tree),
                        ConfiguredFeature::coral(CoralShape::Claw),
                        ConfiguredFeature::coral(CoralShape::Mushroom),
                    ]
                );
            }
            other => panic!("expected simple random coral selector, got {other:?}"),
        }

        let sea_pickle = warm_ocean
            .iter()
            .find(|feature| {
                matches!(
                    feature.feature,
                    ConfiguredFeature::SeaPickle(config) if config == CountConfiguration::new(20)
                )
            })
            .expect("warm ocean sea pickle feature");
        assert_eq!(
            sea_pickle.decorators,
            vec![
                ConfiguredDecorator::chance(16),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
            ]
        );
    }

    #[test]
    fn savanna_feature_table_uses_vanilla_acacia_selector() {
        let savanna = overworld_features_for_biome(get_layered_biome_by_id(35));
        let savanna_plateau = overworld_features_for_biome(get_layered_biome_by_id(36));
        let shattered = overworld_features_for_biome(get_layered_biome_by_id(163));
        assert!(has_savanna_tall_grass_patch_feature(&savanna));
        assert!(has_savanna_tall_grass_patch_feature(&savanna_plateau));
        assert!(has_warm_flower_feature(&savanna));
        assert!(has_warm_flower_feature(&savanna_plateau));
        assert!(has_counted_default_grass_patch_feature(&savanna, 20));
        assert!(has_counted_default_grass_patch_feature(
            &savanna_plateau,
            20
        ));
        let feature = savanna
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
            })
            .expect("savanna tree selector");

        assert_eq!(
            feature.decorators,
            vec![
                ConfiguredDecorator::count_extra(1, 0.1, 1),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ]
        );
        match &feature.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::acacia()),
                        0.8,
                    )]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::oak())
                );
            }
            other => panic!("expected savanna random selector, got {other:?}"),
        }

        let shattered_feature = shattered
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
            })
            .expect("shattered savanna tree selector");
        assert_eq!(
            shattered_feature.decorators.first(),
            Some(&ConfiguredDecorator::count_extra(2, 0.1, 1))
        );
        assert!(!has_savanna_tall_grass_patch_feature(&shattered));
        assert!(has_default_flower_feature(&shattered));
        assert!(!has_warm_flower_feature(&shattered));
        assert!(has_counted_default_grass_patch_feature(&shattered, 5));
    }

    #[test]
    fn jungle_feature_table_uses_vanilla_jungle_selectors() {
        let jungle = overworld_features_for_biome(get_layered_biome_by_id(21));
        let modified = overworld_features_for_biome(get_layered_biome_by_id(149));
        let edge = overworld_features_for_biome(get_layered_biome_by_id(23));

        let light_bamboo = jungle
            .iter()
            .find(|feature| {
                matches!(
                    feature.feature,
                    ConfiguredFeature::Bamboo(BambooConfiguration { probability: 0.0 })
                )
            })
            .expect("jungle light bamboo feature");
        assert_eq!(
            light_bamboo.decorators,
            vec![
                ConfiguredDecorator::count(16),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ]
        );

        let jungle_tree = jungle
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
            })
            .expect("jungle tree selector");
        assert_eq!(
            jungle_tree.decorators,
            tables::tree_threshold_decorators(50, 0.1, 1)
        );
        match &jungle_tree.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::fancy_oak()),
                            0.1,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::jungle_bush()),
                            0.5,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::mega_jungle()),
                            0.33333334,
                        ),
                    ]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::jungle())
                );
            }
            other => panic!("expected jungle random selector, got {other:?}"),
        }

        assert!(!modified.iter().any(|feature| {
            matches!(
                feature.feature,
                ConfiguredFeature::Bamboo(BambooConfiguration { probability: 0.0 })
            )
        }));
        assert!(has_jungle_melon_patch(&jungle));
        assert!(has_jungle_vines_feature(&jungle));
        assert!(has_jungle_melon_patch(&modified));
        assert!(has_jungle_vines_feature(&modified));

        let edge_tree = edge
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
            })
            .expect("jungle edge tree selector");
        assert_eq!(
            edge_tree.decorators,
            tables::tree_threshold_decorators(2, 0.1, 1)
        );
        match &edge_tree.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(config.features.len(), 2);
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::jungle())
                );
            }
            other => panic!("expected jungle edge random selector, got {other:?}"),
        }
        assert!(has_jungle_melon_patch(&edge));
        assert!(has_jungle_vines_feature(&edge));
    }

    #[test]
    fn bamboo_jungle_feature_table_includes_bamboo_and_vegetation_selector() {
        let bamboo_jungle = overworld_features_for_biome(get_layered_biome_by_id(168));

        let bamboo = bamboo_jungle
            .iter()
            .find(|feature| {
                matches!(
                    feature.feature,
                    ConfiguredFeature::Bamboo(BambooConfiguration { probability: 0.2 })
                )
            })
            .expect("bamboo jungle bamboo feature");
        assert_eq!(
            bamboo.decorators,
            vec![
                ConfiguredDecorator::count_noise_biased(160, 80.0, 0.3),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::WorldSurface),
            ]
        );

        let vegetation = bamboo_jungle
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
            })
            .expect("bamboo vegetation selector");
        assert_eq!(
            vegetation.decorators,
            tables::tree_threshold_decorators(30, 0.1, 1)
        );
        match &vegetation.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::fancy_oak()),
                            0.05,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::jungle_bush()),
                            0.15,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::mega_jungle()),
                            0.7,
                        ),
                    ]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::random_patch(tables::jungle_grass_patch_config())
                );
            }
            other => panic!("expected bamboo random selector, got {other:?}"),
        }
        assert!(has_jungle_melon_patch(&bamboo_jungle));
        assert!(has_jungle_vines_feature(&bamboo_jungle));
    }

    #[test]
    fn dark_forest_feature_table_uses_vanilla_dark_oak_selector() {
        let dark_forest = overworld_features_for_biome(get_layered_biome_by_id(29));
        let dark_hills = overworld_features_for_biome(get_layered_biome_by_id(157));
        let feature = dark_forest
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
            })
            .expect("dark forest vegetation feature");

        assert_eq!(
            feature.decorators,
            vec![
                ConfiguredDecorator::dark_oak_tree(),
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ]
        );
        match &feature.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown()),
                            0.025,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red()),
                            0.05,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::dark_oak()),
                            0.6666667,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::birch()),
                            0.2,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::fancy_oak()),
                            0.1,
                        ),
                    ]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::oak())
                );
            }
            other => panic!("expected dark forest random selector, got {other:?}"),
        }

        let hills_feature = dark_hills
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
            })
            .expect("dark forest hills vegetation feature");
        match &hills_feature.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features[0],
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red()),
                        0.025,
                    )
                );
                assert_eq!(
                    config.features[1],
                    WeightedConfiguredFeature::new(
                        ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown()),
                        0.05,
                    )
                );
            }
            other => panic!("expected dark forest hills random selector, got {other:?}"),
        }
    }

    #[test]
    fn mushroom_field_feature_table_includes_java_huge_mushroom_selector() {
        let mushroom_fields = overworld_features_for_biome(get_layered_biome_by_id(14));
        let feature = mushroom_fields
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomBooleanSelector(_))
            })
            .expect("mushroom field vegetation feature");

        assert_eq!(
            feature.decorators,
            vec![
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
            ]
        );
        match &feature.feature {
            ConfiguredFeature::RandomBooleanSelector(config) => {
                assert_eq!(
                    *config.feature_true,
                    ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::red())
                );
                assert_eq!(
                    *config.feature_false,
                    ConfiguredFeature::huge_mushroom(HugeMushroomConfiguration::brown())
                );
            }
            other => panic!("expected random boolean selector, got {other:?}"),
        }
    }

    #[test]
    fn birch_feature_tables_use_vanilla_normal_and_tall_slots() {
        let birch = overworld_features_for_biome(get_layered_biome_by_id(27));
        let tall_birch = overworld_features_for_biome(get_layered_biome_by_id(155));

        let normal_tree = birch
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::Tree(_))
            })
            .expect("birch tree feature");
        assert_eq!(
            normal_tree.decorators,
            vec![
                ConfiguredDecorator::count_extra(10, 0.1, 1),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ]
        );
        assert_eq!(
            normal_tree.feature,
            ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002())
        );

        let tall_tree = tall_birch
            .iter()
            .find(|feature| {
                feature.step == DecorationStep::VegetalDecoration
                    && matches!(feature.feature, ConfiguredFeature::RandomSelector(_))
            })
            .expect("tall birch tree selector");
        assert_eq!(tall_tree.decorators, normal_tree.decorators);
        match &tall_tree.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::super_birch_bees_0002()),
                        0.5,
                    )]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002())
                );
            }
            other => panic!("expected tall birch random selector, got {other:?}"),
        }
    }

    fn has_random_patch(features: &[PlacedFeature], state: RawBlockId, count: i32) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators.first() == Some(&ConfiguredDecorator::count(count))
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: patch_state,
                        ..
                    }) if patch_state == state
                )
        })
    }

    fn has_pumpkin_patch(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::chance(32),
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                    ]
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: PUMPKIN,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 64,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: false,
                        can_replace: false,
                        double_plant: false,
                        column_height: None,
                        need_water: false,
                        place_on: &[GRASS_BLOCK],
                    })
                )
        })
    }

    fn assert_badlands_common_features(features: &[PlacedFeature]) {
        assert!(has_default_grass_patch_feature(features));
        assert!(has_badlands_dead_bush_patch_feature(features));
        assert!(has_normal_mushroom_patch(features, BROWN_MUSHROOM, 4));
        assert!(has_normal_mushroom_patch(features, RED_MUSHROOM, 8));
        assert!(has_random_patch(features, SUGAR_CANE, 13));
        assert!(has_pumpkin_patch(features));
        assert!(has_random_patch(features, CACTUS, 5));
        assert!(has_default_water_spring(features));
        assert!(has_default_lava_spring(features));
    }

    fn has_badlands_tree_feature(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators == tables::tree_threshold_decorators(5, 0.1, 1)
                && feature.feature == ConfiguredFeature::tree(TreeConfiguration::oak())
        })
    }

    fn has_badlands_dead_bush_patch_feature(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::count(20),
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                    ]
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: DEAD_BUSH,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 16,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: true,
                        can_replace: false,
                        double_plant: false,
                        column_height: None,
                        need_water: false,
                        place_on,
                    }) if place_on == &[SAND, RED_SAND, TERRACOTTA, DIRT, GRASS_BLOCK, PODZOL]
                )
        })
    }

    fn has_jungle_melon_patch(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                    ]
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: MELON,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 64,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: false,
                        can_replace: true,
                        double_plant: false,
                        column_height: None,
                        need_water: false,
                        place_on: &[GRASS_BLOCK],
                    })
                )
        })
    }

    fn has_jungle_vines_feature(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.feature == ConfiguredFeature::vines()
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::count(50),
                        ConfiguredDecorator::square(),
                    ]
        })
    }

    fn has_default_water_spring(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.feature == ConfiguredFeature::spring(SpringConfiguration::water())
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::count(50),
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::range(HeightProvider::biased_to_bottom(
                            VerticalAnchor::bottom(),
                            VerticalAnchor::below_top(8),
                            8,
                        )),
                    ]
        })
    }

    fn has_default_lava_spring(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.feature == ConfiguredFeature::spring(SpringConfiguration::lava())
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::count(20),
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::range(HeightProvider::very_biased_to_bottom(
                            VerticalAnchor::bottom(),
                            VerticalAnchor::below_top(8),
                            8,
                        )),
                    ]
        })
    }

    fn has_normal_mushroom_patch(
        features: &[PlacedFeature],
        state: RawBlockId,
        rarity: i32,
    ) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::chance(rarity),
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                    ]
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: patch_state,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 64,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: false,
                        can_replace: false,
                        double_plant: false,
                        column_height: None,
                        need_water: false,
                        place_on: &[],
                    }) if patch_state == state
                )
        })
    }

    fn has_taiga_mushroom_patch(
        features: &[PlacedFeature],
        state: RawBlockId,
        rarity: i32,
        count: Option<i32>,
        heightmap_double_square: bool,
    ) -> bool {
        let mut decorators = Vec::new();
        if let Some(count) = count {
            decorators.push(ConfiguredDecorator::count(count));
        }
        decorators.push(ConfiguredDecorator::square());
        decorators.push(if heightmap_double_square {
            ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking)
        } else {
            ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking)
        });
        decorators.push(ConfiguredDecorator::chance(rarity));

        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators == decorators
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: patch_state,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 64,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: false,
                        can_replace: false,
                        double_plant: false,
                        column_height: None,
                        need_water: false,
                        place_on: &[],
                    }) if patch_state == state
                )
        })
    }

    fn has_water_tree_feature(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators == tables::tree_threshold_decorators(0, 0.1, 1)
                && matches!(
                    &feature.feature,
                    ConfiguredFeature::RandomSelector(RandomFeatureConfiguration {
                        features: weighted,
                        default_feature,
                    }) if weighted.len() == 1
                        && weighted[0].chance == 0.1
                        && *weighted[0].feature == ConfiguredFeature::tree(TreeConfiguration::fancy_oak())
                        && **default_feature == ConfiguredFeature::tree(TreeConfiguration::oak())
                )
        })
    }

    fn has_default_flower_feature(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && matches!(
                    &feature.feature,
                    ConfiguredFeature::Flower(RandomPatchConfiguration {
                        state: POPPY,
                        weighted_states,
                        state_provider: RandomPatchStateProvider::Weighted,
                        tries: 64,
                        ..
                    }) if weighted_states == &tables::DEFAULT_FLOWER_STATES
                )
        })
    }

    fn has_warm_flower_feature(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators.first() == Some(&ConfiguredDecorator::count(4))
                && matches!(
                    &feature.feature,
                    ConfiguredFeature::Flower(RandomPatchConfiguration {
                        state: POPPY,
                        weighted_states,
                        state_provider: RandomPatchStateProvider::Weighted,
                        tries: 64,
                        ..
                    }) if weighted_states == &tables::DEFAULT_FLOWER_STATES
                )
        })
    }

    fn has_default_grass_patch_feature(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                    ]
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: GRASS,
                        tries: 32,
                        ..
                    })
                )
        })
    }

    fn has_counted_default_grass_patch_feature(features: &[PlacedFeature], count: i32) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::count(count),
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
                    ]
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: GRASS,
                        tries: 32,
                        ..
                    })
                )
        })
    }

    fn has_savanna_tall_grass_patch_feature(features: &[PlacedFeature]) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators
                    == vec![
                        ConfiguredDecorator::count(7),
                        ConfiguredDecorator::square(),
                        ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                        ConfiguredDecorator::spread_32_above(),
                    ]
                && matches!(
                    feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: TALL_GRASS_LOWER,
                        tries: 64,
                        double_plant: true,
                        ..
                    })
                )
        })
    }

    fn has_seagrass_feature(features: &[PlacedFeature], count: i32, tall_probability: f32) -> bool {
        features.iter().any(|feature| {
            feature.step == DecorationStep::VegetalDecoration
                && feature.decorators.first() == Some(&ConfiguredDecorator::count(count))
                && matches!(
                    feature.feature,
                    ConfiguredFeature::Seagrass(SeagrassConfiguration {
                        tall_probability: actual,
                    }) if actual == tall_probability
                )
        })
    }

    #[test]
    fn forest_feature_table_uses_vanilla_birch_other_slot() {
        let forest = overworld_features_for_biome(get_layered_biome_by_id(4));
        let vegetal_features = forest
            .iter()
            .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
            .collect::<Vec<_>>();

        assert_eq!(vegetal_features.len(), 11);
        assert_eq!(
            vegetal_features[0].decorators,
            vec![
                ConfiguredDecorator::count(5),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ConfiguredDecorator::spread_32_above(),
                ConfiguredDecorator::Count(CountConfiguration::from_provider(
                    IntProvider::clamped_uniform(-3, 1, 0, 1),
                )),
            ]
        );
        assert!(matches!(
            vegetal_features[0].feature,
            ConfiguredFeature::SimpleRandomSelector(_)
        ));
        assert_eq!(
            vegetal_features[1].feature,
            ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld())
        );
        assert_eq!(
            vegetal_features[2].decorators,
            vec![
                ConfiguredDecorator::count_extra(10, 0.1, 1),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ]
        );
        match &vegetal_features[2].feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002()),
                            0.2,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::fancy_oak_bees_0002()),
                            0.1,
                        ),
                    ]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::oak_bees_0002())
                );
            }
            other => panic!("expected forest birch_other random selector, got {other:?}"),
        }
        assert_eq!(
            vegetal_features[3].decorators,
            vec![
                ConfiguredDecorator::count(2),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ConfiguredDecorator::spread_32_above(),
            ]
        );
        assert_eq!(
            vegetal_features[4].decorators,
            vec![
                ConfiguredDecorator::count(2),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ]
        );
        assert_eq!(
            vegetal_features[7].decorators.first(),
            Some(&ConfiguredDecorator::count(10))
        );
        assert!(matches!(
            vegetal_features[7].feature,
            ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                state: SUGAR_CANE,
                ..
            })
        ));
        assert_eq!(
            vegetal_features[8].decorators,
            vec![
                ConfiguredDecorator::chance(32),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ]
        );
        assert!(matches!(
            vegetal_features[8].feature,
            ConfiguredFeature::RandomPatch(RandomPatchConfiguration { state: PUMPKIN, .. })
        ));
    }

    #[test]
    fn sunflower_plains_feature_table_includes_java_sunflower_patch() {
        let sunflower_plains = overworld_features_for_biome(get_layered_biome_by_id(129));
        let vegetal_features = sunflower_plains
            .iter()
            .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
            .collect::<Vec<_>>();

        assert_eq!(
            vegetal_features[0].decorators,
            vec![
                ConfiguredDecorator::count(10),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ConfiguredDecorator::spread_32_above(),
            ]
        );
        match &vegetal_features[0].feature {
            ConfiguredFeature::RandomPatch(config) => {
                assert_eq!(config.state, SUNFLOWER_LOWER);
                assert_eq!(config.weighted_states, &[]);
                assert_eq!(config.state_provider, RandomPatchStateProvider::Simple);
                assert_eq!(config.tries, 64);
                assert_eq!(config.xspread, 7);
                assert_eq!(config.yspread, 3);
                assert_eq!(config.zspread, 7);
                assert!(!config.project);
                assert!(!config.can_replace);
                assert!(config.double_plant);
                assert_eq!(config.column_height, None);
                assert!(!config.need_water);
                assert!(config.place_on.is_empty());
            }
            other => panic!("expected sunflower random patch, got {other:?}"),
        }
    }

    #[test]
    fn flower_forest_feature_table_includes_java_flower_provider() {
        let flower_forest = overworld_features_for_biome(get_layered_biome_by_id(132));
        let vegetal_features = flower_forest
            .iter()
            .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
            .collect::<Vec<_>>();

        assert_eq!(
            vegetal_features[0].decorators,
            vec![
                ConfiguredDecorator::count(5),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ConfiguredDecorator::spread_32_above(),
                ConfiguredDecorator::Count(CountConfiguration::from_provider(
                    IntProvider::clamped_uniform(-1, 3, 0, 3),
                )),
            ]
        );
        match &vegetal_features[0].feature {
            ConfiguredFeature::SimpleRandomSelector(config) => {
                let double_patch = |state| {
                    ConfiguredFeature::random_patch(RandomPatchConfiguration {
                        state,
                        weighted_states: &[],
                        state_provider: RandomPatchStateProvider::Simple,
                        tries: 64,
                        xspread: 7,
                        yspread: 3,
                        zspread: 7,
                        project: false,
                        can_replace: false,
                        double_plant: true,
                        column_height: None,
                        need_water: false,
                        place_on: &[],
                    })
                };
                assert_eq!(
                    config.features,
                    vec![
                        double_patch(LILAC_LOWER),
                        double_patch(ROSE_BUSH_LOWER),
                        double_patch(PEONY_LOWER),
                        ConfiguredFeature::flower(RandomPatchConfiguration::new(
                            LILY_OF_THE_VALLEY,
                        )),
                    ]
                );
            }
            other => panic!("expected common forest flower simple random selector, got {other:?}"),
        }

        match &vegetal_features[2].feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::birch_bees_0002()),
                            0.2,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::fancy_oak_bees_0002()),
                            0.1,
                        ),
                    ]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::oak_bees_0002())
                );
            }
            other => panic!("expected flower forest tree random selector, got {other:?}"),
        }
        assert_eq!(
            vegetal_features[2].decorators,
            vec![
                ConfiguredDecorator::count_extra(6, 0.1, 1),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ]
        );

        let dense_flowers = vegetal_features[3];
        assert_eq!(
            dense_flowers.decorators,
            vec![
                ConfiguredDecorator::count(100),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ConfiguredDecorator::spread_32_above(),
            ]
        );
        match &dense_flowers.feature {
            ConfiguredFeature::Flower(config) => {
                assert_eq!(
                    config.state_provider,
                    RandomPatchStateProvider::ForestFlower
                );
                assert_eq!(config.tries, 64);
                assert_eq!(config.xspread, 7);
                assert_eq!(config.yspread, 3);
                assert_eq!(config.zspread, 7);
                assert!(config.project);
                assert!(!config.can_replace);
                assert!(config.place_on.is_empty());
            }
            other => panic!("expected flower forest provider feature, got {other:?}"),
        }
    }

    #[test]
    fn taiga_feature_table_uses_vanilla_taiga_vegetation_selector() {
        let taiga = overworld_features_for_biome(get_layered_biome_by_id(133));
        let vegetal_features = taiga
            .iter()
            .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
            .collect::<Vec<_>>();

        assert!(matches!(
            vegetal_features[0].feature,
            ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                state: LARGE_FERN_LOWER,
                double_plant: true,
                ..
            })
        ));
        assert_eq!(
            vegetal_features[1].feature,
            ConfiguredFeature::glow_lichen(GlowLichenConfiguration::default_overworld())
        );
        let feature = vegetal_features[2];

        assert_eq!(feature.step, DecorationStep::VegetalDecoration);
        assert_eq!(
            feature.decorators,
            vec![
                ConfiguredDecorator::count_extra(10, 0.1, 1),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::water_depth_threshold(0),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloor),
            ]
        );
        match &feature.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![WeightedConfiguredFeature::new(
                        ConfiguredFeature::tree(TreeConfiguration::pine()),
                        0.33333334,
                    )]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::spruce())
                );
            }
            other => panic!("expected random selector, got {other:?}"),
        }

        assert_eq!(
            vegetal_features[3].decorators,
            vec![
                ConfiguredDecorator::count(2),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::MotionBlocking),
                ConfiguredDecorator::spread_32_above(),
            ]
        );
        match &vegetal_features[3].feature {
            ConfiguredFeature::Flower(config) => {
                assert_eq!(
                    config.weighted_states,
                    tables::DEFAULT_FLOWER_STATES.as_slice()
                );
                assert_eq!(config.tries, 64);
            }
            other => panic!("expected default flower feature, got {other:?}"),
        }
        assert_eq!(
            vegetal_features[4].decorators,
            vec![
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ]
        );
        match &vegetal_features[4].feature {
            ConfiguredFeature::RandomPatch(config) => {
                assert_eq!(
                    config.weighted_states,
                    tables::TAIGA_GRASS_STATES.as_slice()
                );
                assert_eq!(config.tries, 32);
            }
            other => panic!("expected taiga grass random patch, got {other:?}"),
        }
        assert_eq!(
            vegetal_features[9].decorators.first(),
            Some(&ConfiguredDecorator::count(10))
        );
        assert!(matches!(
            vegetal_features[9].feature,
            ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                state: SUGAR_CANE,
                ..
            })
        ));
        assert_eq!(
            vegetal_features[10].decorators,
            vec![
                ConfiguredDecorator::chance(32),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ]
        );
        assert!(matches!(
            vegetal_features[10].feature,
            ConfiguredFeature::RandomPatch(RandomPatchConfiguration { state: PUMPKIN, .. })
        ));

        let water_spring =
            vegetal_features[test_support::JAVA_TAIGA_WATER_SPRING_FEATURE_INDEX as usize];
        assert_eq!(
            water_spring.feature,
            ConfiguredFeature::spring(SpringConfiguration::water())
        );
        assert_eq!(
            water_spring.decorators,
            vec![
                ConfiguredDecorator::count(50),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::range(HeightProvider::biased_to_bottom(
                    VerticalAnchor::bottom(),
                    VerticalAnchor::below_top(8),
                    8,
                )),
            ]
        );

        let lava_spring =
            vegetal_features[test_support::JAVA_TAIGA_LAVA_SPRING_FEATURE_INDEX as usize];
        assert_eq!(
            lava_spring.feature,
            ConfiguredFeature::spring(SpringConfiguration::lava())
        );
        assert_eq!(
            lava_spring.decorators,
            vec![
                ConfiguredDecorator::count(20),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::range(HeightProvider::very_biased_to_bottom(
                    VerticalAnchor::bottom(),
                    VerticalAnchor::below_top(8),
                    8,
                )),
            ]
        );
        let berry_patch = vegetal_features[13];
        assert_eq!(
            berry_patch.decorators,
            vec![
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ]
        );
        match &berry_patch.feature {
            ConfiguredFeature::RandomPatch(config) => {
                assert_eq!(config.state, SWEET_BERRY_BUSH);
                assert_eq!(config.weighted_states, &[]);
                assert_eq!(config.tries, 64);
                assert_eq!(config.xspread, 7);
                assert_eq!(config.yspread, 3);
                assert_eq!(config.zspread, 7);
                assert!(!config.project);
                assert!(!config.can_replace);
                assert!(!config.double_plant);
                assert_eq!(config.column_height, None);
                assert!(!config.need_water);
                assert_eq!(config.place_on, &[GRASS_BLOCK]);
            }
            other => panic!("expected sweet berry random patch, got {other:?}"),
        }

        let snowy_taiga = overworld_features_for_biome(get_layered_biome_by_id(30));
        let snowy_taiga_vegetal_features = snowy_taiga
            .iter()
            .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
            .collect::<Vec<_>>();
        let snowy_berry_patch = snowy_taiga_vegetal_features
            .iter()
            .find(|feature| {
                matches!(
                    &feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: SWEET_BERRY_BUSH,
                        ..
                    })
                )
            })
            .expect("snowy taiga should include Java berry bush patch");
        assert_eq!(
            snowy_berry_patch.decorators,
            vec![
                ConfiguredDecorator::chance(12),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap_spread_double(HeightmapType::MotionBlocking),
            ]
        );

        let snowy_tundra = overworld_features_for_biome(get_layered_biome_by_id(12));
        assert!(
            !snowy_tundra.iter().any(|feature| {
                matches!(
                    &feature.feature,
                    ConfiguredFeature::RandomPatch(RandomPatchConfiguration {
                        state: SWEET_BERRY_BUSH,
                        ..
                    })
                )
            }),
            "snowy tundra should not inherit snowy taiga berry bushes"
        );
    }

    #[test]
    fn giant_taiga_feature_table_uses_vanilla_mega_tree_selectors() {
        let giant_tree_taiga = overworld_features_for_biome(get_layered_biome_by_id(32));
        let giant_spruce_taiga = overworld_features_for_biome(get_layered_biome_by_id(160));

        let giant_tree_vegetal_features = giant_tree_taiga
            .iter()
            .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
            .collect::<Vec<_>>();
        let giant_spruce_vegetal_features = giant_spruce_taiga
            .iter()
            .filter(|feature| feature.step == DecorationStep::VegetalDecoration)
            .collect::<Vec<_>>();

        let giant_tree_feature = giant_tree_vegetal_features[2];
        assert_eq!(
            giant_tree_feature.decorators,
            tables::tree_threshold_decorators(10, 0.1, 1)
        );
        match &giant_tree_feature.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::mega_spruce()),
                            0.025641026,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::mega_pine()),
                            0.30769232,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::pine()),
                            0.33333334,
                        ),
                    ]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::spruce())
                );
            }
            other => panic!("expected giant tree taiga random selector, got {other:?}"),
        }

        let giant_spruce_feature = giant_spruce_vegetal_features[2];
        assert_eq!(
            giant_spruce_feature.decorators,
            tables::tree_threshold_decorators(10, 0.1, 1)
        );
        match &giant_spruce_feature.feature {
            ConfiguredFeature::RandomSelector(config) => {
                assert_eq!(
                    config.features,
                    vec![
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::mega_spruce()),
                            0.33333334,
                        ),
                        WeightedConfiguredFeature::new(
                            ConfiguredFeature::tree(TreeConfiguration::pine()),
                            0.33333334,
                        ),
                    ]
                );
                assert_eq!(
                    *config.default_feature,
                    ConfiguredFeature::tree(TreeConfiguration::spruce())
                );
            }
            other => panic!("expected giant spruce taiga random selector, got {other:?}"),
        }
    }

    #[test]
    fn biome_feature_tables_start_with_default_lakes() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let desert = overworld_features_for_biome(get_layered_biome_by_id(2));

        assert_eq!(plains[0].step, DecorationStep::Lakes);
        assert_eq!(
            plains[0].feature,
            ConfiguredFeature::lake(LakeConfiguration::new(WATER))
        );
        assert_eq!(
            plains[0].decorators,
            vec![
                ConfiguredDecorator::chance(4),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::range(HeightProvider::uniform(
                    VerticalAnchor::bottom(),
                    VerticalAnchor::top(),
                )),
            ]
        );
        assert_eq!(plains[1].step, DecorationStep::Lakes);
        assert_eq!(
            plains[1].feature,
            ConfiguredFeature::lake(LakeConfiguration::new(LAVA))
        );
        assert_eq!(
            plains[1].decorators,
            vec![
                ConfiguredDecorator::chance(8),
                ConfiguredDecorator::square(),
                ConfiguredDecorator::range(HeightProvider::biased_to_bottom(
                    VerticalAnchor::bottom(),
                    VerticalAnchor::top(),
                    8,
                )),
                ConfiguredDecorator::lava_lake(80),
            ]
        );

        assert_eq!(desert[0].step, DecorationStep::Lakes);
        assert_eq!(
            desert[0].feature,
            ConfiguredFeature::lake(LakeConfiguration::new(LAVA))
        );
    }

    #[test]
    fn biome_feature_tables_start_with_default_underground_variety() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let expected = [
            (DIRT, 33, 10, VerticalAnchor::top()),
            (GRAVEL, 33, 8, VerticalAnchor::top()),
            (GRANITE, 33, 10, VerticalAnchor::absolute(79)),
            (DIORITE, 33, 10, VerticalAnchor::absolute(79)),
            (ANDESITE, 33, 10, VerticalAnchor::absolute(79)),
            (TUFF, 33, 1, VerticalAnchor::absolute(16)),
            (DEEPSLATE, 64, 2, VerticalAnchor::absolute(16)),
        ];

        for (feature, (expected_block, expected_size, expected_count, max_y)) in
            plains.iter().skip(2).zip(expected)
        {
            assert_eq!(feature.step, DecorationStep::UndergroundOres);
            assert_eq!(
                feature.decorators,
                vec![
                    ConfiguredDecorator::count(expected_count),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::range(HeightProvider::uniform(
                        VerticalAnchor::absolute(0),
                        max_y,
                    )),
                ]
            );
            match &feature.feature {
                ConfiguredFeature::Ore(config) => {
                    assert_eq!(config.size, expected_size);
                    assert_eq!(
                        config.target_states,
                        vec![OreTargetBlockState::new(
                            OreTarget::NaturalStone,
                            expected_block,
                        )]
                    );
                }
                other => panic!("expected ore feature, got {other:?}"),
            }
        }
    }

    #[test]
    fn biome_feature_tables_include_default_ores_after_variety() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let expected = [
            (
                COAL_ORE,
                DEEPSLATE_COAL_ORE,
                17,
                Some(20),
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(127)),
            ),
            (
                IRON_ORE,
                DEEPSLATE_IRON_ORE,
                9,
                Some(20),
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(63)),
            ),
            (
                GOLD_ORE,
                DEEPSLATE_GOLD_ORE,
                9,
                Some(2),
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(31)),
            ),
            (
                REDSTONE_ORE,
                DEEPSLATE_REDSTONE_ORE,
                8,
                Some(8),
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
            ),
            (
                DIAMOND_ORE,
                DEEPSLATE_DIAMOND_ORE,
                8,
                None,
                HeightProvider::uniform(VerticalAnchor::bottom(), VerticalAnchor::absolute(15)),
            ),
            (
                LAPIS_ORE,
                DEEPSLATE_LAPIS_ORE,
                7,
                None,
                HeightProvider::trapezoid(
                    VerticalAnchor::absolute(0),
                    VerticalAnchor::absolute(30),
                    0,
                ),
            ),
            (
                COPPER_ORE,
                DEEPSLATE_COPPER_ORE,
                10,
                Some(6),
                HeightProvider::trapezoid(
                    VerticalAnchor::absolute(0),
                    VerticalAnchor::absolute(96),
                    0,
                ),
            ),
        ];

        for (feature, (stone_ore, deepslate_ore, size, count, height)) in
            plains.iter().skip(11).zip(expected)
        {
            assert_eq!(feature.step, DecorationStep::UndergroundOres);
            let mut expected_decorators = Vec::new();
            if let Some(count) = count {
                expected_decorators.push(ConfiguredDecorator::count(count));
            }
            expected_decorators.push(ConfiguredDecorator::square());
            expected_decorators.push(ConfiguredDecorator::range(height));
            assert_eq!(feature.decorators, expected_decorators);

            match &feature.feature {
                ConfiguredFeature::Ore(config) => {
                    assert_eq!(config.size, size);
                    assert_eq!(
                        config.target_states,
                        vec![
                            OreTargetBlockState::new(OreTarget::StoneOreReplaceables, stone_ore),
                            OreTargetBlockState::new(
                                OreTarget::DeepslateOreReplaceables,
                                deepslate_ore,
                            ),
                        ]
                    );
                }
                other => panic!("expected ore feature, got {other:?}"),
            }
        }
    }

    #[test]
    fn biome_feature_tables_include_default_soft_disks_after_ores() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let expected = [
            (
                SAND,
                IntProvider::uniform(2, 6),
                2,
                &[DIRT, GRASS_BLOCK][..],
                vec![
                    ConfiguredDecorator::count(3),
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
                ],
            ),
            (
                CLAY,
                IntProvider::uniform(2, 3),
                1,
                &[DIRT, CLAY][..],
                vec![
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
                ],
            ),
            (
                GRAVEL,
                IntProvider::uniform(2, 5),
                2,
                &[DIRT, GRASS_BLOCK][..],
                vec![
                    ConfiguredDecorator::square(),
                    ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
                ],
            ),
        ];

        for (feature, (state, radius, half_height, targets, decorators)) in
            plains.iter().skip(18).zip(expected)
        {
            assert_eq!(feature.step, DecorationStep::UndergroundOres);
            assert_eq!(feature.decorators, decorators);
            match &feature.feature {
                ConfiguredFeature::Disk(config) => {
                    assert_eq!(config.state, state);
                    assert_eq!(config.radius, radius);
                    assert_eq!(config.half_height, half_height);
                    assert_eq!(config.targets, targets);
                }
                other => panic!("expected disk feature, got {other:?}"),
            }
        }
    }

    #[test]
    fn swamp_feature_table_uses_clay_only_soft_disk() {
        let swamp = overworld_features_for_biome(get_layered_biome_by_id(6));
        let disk_features: Vec<_> = swamp
            .iter()
            .filter_map(|feature| match &feature.feature {
                ConfiguredFeature::Disk(config) => Some((feature, config)),
                _ => None,
            })
            .collect();

        assert_eq!(disk_features.len(), 1);
        let (feature, config) = disk_features[0];
        assert_eq!(feature.step, DecorationStep::UndergroundOres);
        assert_eq!(config.state, CLAY);
        assert_eq!(config.radius, IntProvider::uniform(2, 3));
        assert_eq!(config.half_height, 1);
        assert_eq!(config.targets, &[DIRT, CLAY]);
        assert_eq!(
            feature.decorators,
            vec![
                ConfiguredDecorator::square(),
                ConfiguredDecorator::heightmap(HeightmapType::OceanFloorWg),
            ]
        );
    }

    #[test]
    fn biome_feature_tables_end_with_default_freeze_top_layer() {
        let plains = overworld_features_for_biome(get_layered_biome_by_id(1));
        let feature = plains.last().expect("plains has features");

        assert_eq!(feature.step, DecorationStep::TopLayerModification);
        assert_eq!(feature.feature, ConfiguredFeature::freeze_top_layer());
        assert!(feature.decorators.is_empty());
    }

    #[test]
    fn biome_overworld_decoration_reports_added_blocks() {
        let mut chunk = flat_grass_chunk();
        let report = apply_overworld_biome_features(12_345, get_layered_biome_by_id(4), &mut chunk);

        assert_eq!(report.biome_key, "minecraft:forest");
        assert_eq!(report.attempted_features, 33);
        assert!(report.placed_features > 0);
        assert!(report.added_non_air_blocks > 0);
    }
}
