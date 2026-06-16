use crate::biome::BiomeDefinition;
use crate::block::{
    AIR, BIRCH_LEAVES, BIRCH_LOG, CAVE_AIR, DANDELION, DEAD_BUSH, DIRT, FERN, GLOW_LICHEN, GRASS,
    GRASS_BLOCK, ICE, LARGE_FERN_LOWER, LARGE_FERN_UPPER, MYCELIUM, OAK_LEAVES, OAK_LOG, PODZOL,
    POPPY, RawBlockId, SNOW, SPRUCE_LEAVES, SPRUCE_LOG, WATER, has_fluid, is_air_like, is_leaves,
    material_blocks_motion,
};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{BlockPos, HeightmapType};
use crate::prng::RandomSource;
use crate::surface::biome_temperature;
use mclone_core::CHUNK_WIDTH;

mod configured;
mod context;
mod glow_lichen;
mod lake;
mod ore;
mod patch;
mod placed;
mod region;
mod spring;
mod tables;

pub use configured::{
    BasicTreeConfiguration, ConfiguredFeature, DecoratedFeatureConfiguration,
    FoliagePlacerConfiguration, GlowLichenConfiguration, LakeConfiguration, OreConfiguration,
    OreTarget, OreTargetBlockState, RandomFeatureConfiguration, RandomPatchConfiguration,
    SimpleBlockConfiguration, SpringConfiguration, StraightTrunkPlacerConfiguration,
    TreeConfiguration, TwoLayersFeatureSize, WeightedBlockState, WeightedConfiguredFeature,
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

pub const FEATURES_CHUNK_DEPENDENCY_RADIUS: i32 = 8;
pub const FEATURES_WRITE_RADIUS_CUTOFF: i32 = 1;

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
            Self::GlowLichen(config) => {
                glow_lichen::place_glow_lichen(world, random, origin, *config)
            }
            Self::BasicTree(config) => place_basic_tree(world, random, origin, *config),
            Self::Tree(config) => place_tree(world, random, origin, *config),
            Self::RandomSelector(config) => {
                place_random_selector(world, biomes, random, origin, config)
            }
            Self::Decorated(config) => {
                placed::place_configured_decorated_feature(world, biomes, random, origin, config)
            }
            Self::Ore(config) => ore::place_ore(world, random, origin, config),
            Self::FreezeTopLayer => place_freeze_top_layer(world, biomes, origin),
        }
    }
}

fn place_freeze_top_layer<W: FeatureWorld, B: FeatureBiomeResolver>(
    world: &mut W,
    biomes: &B,
    origin: BlockPos,
) -> bool {
    for dx in 0..CHUNK_WIDTH {
        for dz in 0..CHUNK_WIDTH {
            let x = origin.x + dx;
            let z = origin.z + dz;
            let Some(y) = world.height_at(HeightmapType::MotionBlocking, x, z) else {
                continue;
            };
            let surface_pos = BlockPos::new(x, y, z);
            let below = BlockPos::new(x, y - 1, z);
            let biome = biomes.biome_at(x, y, z);

            if should_freeze(world, biome, below) {
                world.set_block_world(below, ICE);
            }
            if should_snow(world, biome, surface_pos) {
                world.set_block_world(surface_pos, SNOW);
            }
        }
    }

    true
}

fn should_freeze<W: FeatureWorld>(world: &mut W, biome: BiomeDefinition, pos: BlockPos) -> bool {
    is_within_build_height(world, pos)
        && biome_temperature(biome, pos.x, pos.y, pos.z) < 0.15
        && world.block_at_world(pos) == Some(WATER)
}

fn should_snow<W: FeatureWorld>(world: &mut W, biome: BiomeDefinition, pos: BlockPos) -> bool {
    if !is_within_build_height(world, pos)
        || biome_temperature(biome, pos.x, pos.y, pos.z) >= 0.15
        || !world.block_at_world(pos).is_some_and(is_air_like)
    {
        return false;
    }

    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    world
        .block_at_world(below)
        .is_some_and(snow_layer_can_survive_on)
}

fn is_within_build_height<W: FeatureWorld>(world: &W, pos: BlockPos) -> bool {
    (world.min_y()..world.min_y() + world.height()).contains(&pos.y)
}

fn snow_layer_can_survive_on(block_id: RawBlockId) -> bool {
    material_blocks_motion(block_id)
}

fn offset_pos(pos: BlockPos, direction: Direction) -> BlockPos {
    let (dx, dy, dz) = direction.offset();
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}

fn place_basic_tree<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: BasicTreeConfiguration,
) -> bool {
    let Some(base) = project_to_surface(world, origin) else {
        return false;
    };
    let below = BlockPos::new(base.x, base.y - 1, base.z);
    let Some(block_below) = world.block_at_world(below) else {
        return false;
    };
    if !matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM) {
        return false;
    }

    let height = config.min_height + random.next_int_bound(config.random_height.max(1));
    let leaves_center_y = base.y + height;
    if base.y < world.min_y() + 1 || leaves_center_y + 1 >= world.min_y() + world.height() {
        return false;
    }

    let mut targets = Vec::new();
    for y in base.y..base.y + height {
        targets.push((BlockPos::new(base.x, y, base.z), config.log));
    }

    for dy in -2_i32..=1 {
        let radius: i32 = if dy == 1 { 1 } else { 2 };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let corner = dx.abs() == radius && dz.abs() == radius;
                if corner && (dy == 1 || random.next_boolean()) {
                    continue;
                }
                targets.push((
                    BlockPos::new(base.x + dx, leaves_center_y + dy, base.z + dz),
                    config.leaves,
                ));
            }
        }
    }

    if targets
        .iter()
        .any(|(pos, _)| !can_replace_tree_block(world, *pos))
    {
        return false;
    }

    world.set_block_world(below, DIRT);
    for (pos, block_id) in targets {
        world.set_block_world(pos, block_id);
    }
    true
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

fn place_tree<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: TreeConfiguration,
) -> bool {
    let base = origin;

    let tree_height = config.trunk_placer.tree_height(random);
    let foliage_height = config
        .foliage_placer
        .foliage_height(random, tree_height, config);
    let trunk_height = tree_height - foliage_height;
    let foliage_radius = config.foliage_placer.foliage_radius(random, trunk_height);

    if base.y < world.min_y() + 1 || base.y + tree_height + 1 > world.min_y() + world.height() {
        return false;
    }
    if !can_survive_tree_sapling(world, base) {
        return false;
    }
    let max_free_tree_height = get_max_free_tree_height(world, tree_height, base, config);
    if max_free_tree_height < tree_height {
        return false;
    }

    place_straight_trunk(world, random, base, tree_height, config);
    let foliage_attachment = BlockPos::new(base.x, base.y + tree_height, base.z);
    create_foliage(
        world,
        random,
        config,
        foliage_attachment,
        foliage_height,
        foliage_radius,
    );
    true
}

fn get_max_free_tree_height<W: FeatureWorld>(
    world: &mut W,
    tree_height: i32,
    base: BlockPos,
    config: TreeConfiguration,
) -> i32 {
    for y_offset in 0..=tree_height + 1 {
        let radius = config.minimum_size.size_at_height(y_offset);
        for x_offset in -radius..=radius {
            for z_offset in -radius..=radius {
                let pos = BlockPos::new(base.x + x_offset, base.y + y_offset, base.z + z_offset);
                if !is_free_tree_pos(world, pos) {
                    return y_offset - 2;
                }
            }
        }
    }

    tree_height
}

fn place_straight_trunk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    config: TreeConfiguration,
) {
    set_dirt_at(world, random, BlockPos::new(base.x, base.y - 1, base.z));

    for y_offset in 0..height {
        place_log(
            world,
            random,
            BlockPos::new(base.x, base.y + y_offset, base.z),
            config,
        );
    }
}

fn create_foliage<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    attachment: BlockPos,
    foliage_height: i32,
    foliage_radius: i32,
) {
    let offset = config.foliage_placer.offset(random);
    match config.foliage_placer {
        FoliagePlacerConfiguration::Spruce { .. } => {
            let mut radius = random.next_int_bound(2);
            let mut radius_limit = 1;
            let mut reset_radius = 0;

            for y_offset in (-(foliage_height)..=offset).rev() {
                place_leaves_row(world, random, config, attachment, radius, y_offset);
                if radius >= radius_limit {
                    radius = reset_radius;
                    reset_radius = 1;
                    radius_limit = (radius_limit + 1).min(foliage_radius);
                } else {
                    radius += 1;
                }
            }
        }
        FoliagePlacerConfiguration::Pine { .. } => {
            let mut radius = 0;

            for y_offset in ((offset - foliage_height)..=offset).rev() {
                place_leaves_row(world, random, config, attachment, radius, y_offset);
                if radius >= 1 && y_offset == offset - foliage_height + 1 {
                    radius -= 1;
                } else if radius < foliage_radius {
                    radius += 1;
                }
            }
        }
    }
}

fn place_leaves_row<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: TreeConfiguration,
    center: BlockPos,
    radius: i32,
    y_offset: i32,
) {
    for x_offset in -radius..=radius {
        for z_offset in -radius..=radius {
            if should_skip_conifer_leaf(x_offset.abs(), z_offset.abs(), radius) {
                continue;
            }
            try_place_leaf(
                world,
                random,
                BlockPos::new(
                    center.x + x_offset,
                    center.y + y_offset,
                    center.z + z_offset,
                ),
                config,
            );
        }
    }
}

fn should_skip_conifer_leaf(abs_x: i32, abs_z: i32, radius: i32) -> bool {
    abs_x == radius && abs_z == radius && radius > 0
}

fn place_log<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
) -> bool {
    if valid_tree_pos(world, pos) {
        world.set_block_world(pos, config.log)
    } else {
        false
    }
}

fn try_place_leaf<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
    config: TreeConfiguration,
) -> bool {
    if valid_tree_pos(world, pos) {
        world.set_block_world(pos, config.leaves)
    } else {
        false
    }
}

fn set_dirt_at<W: FeatureWorld>(
    world: &mut W,
    _random: &mut impl RandomSource,
    pos: BlockPos,
) -> bool {
    let Some(current) = world.block_at_world(pos) else {
        return false;
    };
    if matches!(current, DIRT | PODZOL) {
        true
    } else {
        world.set_block_world(pos, DIRT)
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

fn can_survive_tree_sapling<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    let Some(block_below) = world.block_at_world(below) else {
        return false;
    };
    matches!(block_below, GRASS_BLOCK | DIRT | PODZOL | MYCELIUM)
}

fn valid_tree_pos<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(
        block_id,
        AIR | CAVE_AIR
            | WATER
            | GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
            | OAK_LEAVES
            | BIRCH_LEAVES
            | SPRUCE_LEAVES
    )
}

fn is_free_tree_pos<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    if valid_tree_pos(world, pos) {
        return true;
    }
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(block_id, OAK_LOG | BIRCH_LOG | SPRUCE_LOG)
}

fn can_replace_tree_block<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let Some(block_id) = world.block_at_world(pos) else {
        return false;
    };
    matches!(
        block_id,
        AIR | CAVE_AIR
            | WATER
            | GRASS
            | FERN
            | DANDELION
            | POPPY
            | DEAD_BUSH
            | LARGE_FERN_LOWER
            | LARGE_FERN_UPPER
            | GLOW_LICHEN
            | OAK_LEAVES
            | OAK_LOG
            | BIRCH_LEAVES
            | BIRCH_LOG
            | SPRUCE_LEAVES
            | SPRUCE_LOG
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::get_layered_biome_by_id;
    use crate::block::{
        ANDESITE, COAL_ORE, COPPER_ORE, DEEPSLATE, DEEPSLATE_COAL_ORE, DEEPSLATE_COPPER_ORE,
        DEEPSLATE_DIAMOND_ORE, DEEPSLATE_GOLD_ORE, DEEPSLATE_IRON_ORE, DEEPSLATE_LAPIS_ORE,
        DEEPSLATE_REDSTONE_ORE, DIAMOND_ORE, DIORITE, GOLD_ORE, GRANITE, GRAVEL, IRON_ORE,
        LAPIS_ORE, LAVA, REDSTONE_ORE, STONE, TUFF,
    };
    use crate::placement::{
        ConfiguredDecorator, DecorationContext, HeightProvider, VerticalAnchor,
    };
    use crate::prng::WorldgenRandom;

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
            tries: 16,
            xspread: 3,
            yspread: 1,
            zspread: 3,
            project: true,
            can_replace: false,
            double_plant: false,
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
            tries: 1,
            xspread: 0,
            yspread: 0,
            zspread: 0,
            project: false,
            can_replace: false,
            double_plant: true,
            place_on: &[],
        });

        assert!(feature.place(&mut chunk, &mut random, BlockPos::new(8, 3, 8)));
        assert_eq!(chunk.get_block_at_y(8, 3, 8), LARGE_FERN_LOWER);
        assert_eq!(chunk.get_block_at_y(8, 4, 8), LARGE_FERN_UPPER);
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
                tries: 8,
                xspread: 1,
                yspread: 1,
                zspread: 1,
                project: true,
                can_replace: false,
                double_plant: false,
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
                ConfiguredFeature::BasicTree(BasicTreeConfiguration { log: BIRCH_LOG, .. })
            )
        }));
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
            vegetal_features[5..11]
                .iter()
                .filter(|feature| feature.feature == ConfiguredFeature::noop())
                .count(),
            6
        );
        assert!(
            vegetal_features[5..11]
                .iter()
                .all(|feature| feature.decorators.is_empty())
        );

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
        assert_eq!(vegetal_features[13].feature, ConfiguredFeature::noop());
        assert!(vegetal_features[13].decorators.is_empty());
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
            plains.iter().skip(9).zip(expected)
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
        assert_eq!(report.attempted_features, 23);
        assert!(report.placed_features > 0);
        assert!(report.added_non_air_blocks > 0);
    }
}
