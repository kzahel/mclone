use crate::block::{RawBlockId, has_fluid, is_air_like, is_leaves, material_blocks_motion};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{BlockPos, HeightmapType};
use crate::prng::RandomSource;

mod bamboo;
mod blob;
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
    BambooConfiguration, BasicTreeConfiguration, BlockStateConfiguration, ConfiguredFeature,
    CoralShape, DecoratedFeatureConfiguration, DiskConfiguration, DripstoneClusterConfiguration,
    FloatProvider, FoliagePlacerConfiguration, GlowLichenConfiguration, HugeMushroomConfiguration,
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
            Self::BlockBlob(config) => blob::place_block_blob(world, random, origin, *config),
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
mod tests;
