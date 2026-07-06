use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{
    FeatureBiomeResolver, FeatureWorld, RandomBooleanFeatureConfiguration,
    RandomFeatureConfiguration, SimpleRandomFeatureConfiguration,
};

pub(crate) fn place_random_selector<W: FeatureWorld, B: FeatureBiomeResolver>(
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

pub(crate) fn place_simple_random_selector<W: FeatureWorld, B: FeatureBiomeResolver>(
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

pub(crate) fn place_random_boolean_selector<W: FeatureWorld, B: FeatureBiomeResolver>(
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
