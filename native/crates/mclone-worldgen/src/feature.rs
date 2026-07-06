use crate::placement::BlockPos;
use crate::prng::RandomSource;

#[cfg(test)]
use crate::{block::RawBlockId, levelgen::MutableChunkBlockBuffer, placement::HeightmapType};

mod bamboo;
mod blob;
mod configured;
mod context;
mod direction;
mod disk;
mod dripstone;
mod glow_lichen;
mod heightmap;
mod ice;
mod lake;
mod mushroom;
mod ocean;
mod ore;
mod patch;
mod placed;
mod region;
mod selectors;
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
pub use direction::Direction;
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
pub(crate) use direction::offset_pos;
pub(crate) use heightmap::{heightmap_height, project_to_surface};
pub(crate) use placed::apply_overworld_biome_decoration_to_region_timed;
pub(crate) use selectors::{
    place_random_boolean_selector, place_random_selector, place_simple_random_selector,
};

#[cfg(test)]
pub(crate) use placed::test_support;

pub const FEATURES_WRITE_RADIUS_CUTOFF: i32 = 1;
/// Java `FEATURES` has status dependency range 8, but current native feature
/// placement only needs full mutable block buffers in the write/read band. The
/// farther Java shell is a weaker status/structure dependency and should not
/// force full terrain generation during startup.
pub const FEATURES_CHUNK_DEPENDENCY_RADIUS: i32 = 8;
pub const FEATURES_BLOCK_DEPENDENCY_RADIUS: i32 = FEATURES_WRITE_RADIUS_CUTOFF;

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

#[cfg(test)]
mod tests;
