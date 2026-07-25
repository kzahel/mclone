#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::biome::{BiomeDefinition, OverworldBiomeSource};
use crate::levelgen::MutableChunkBlockBuffer;
use crate::placement::{BlockPos, ConfiguredDecorator, DecorationContext, HeightmapType};
use crate::prng::{RandomSource, WorldgenRandom};
use mclone_core::chunk_min_block_coord;

use super::{
    ConfiguredFeature, ConstantFeatureBiomeResolver, DEFAULT_FEATURE_BIOME,
    DecoratedFeatureConfiguration, DecorationStep, FeatureBiomeResolver, FeatureDecorationTiming,
    FeatureRegion, FeatureWorld, OverworldFeatureBiomeResolver, tables,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacedFeature {
    pub step: DecorationStep,
    pub feature: ConfiguredFeature,
    pub decorators: Vec<ConfiguredDecorator>,
}

impl PlacedFeature {
    pub fn new(
        step: DecorationStep,
        feature: ConfiguredFeature,
        decorators: impl Into<Vec<ConfiguredDecorator>>,
    ) -> Self {
        Self {
            step,
            feature,
            decorators: decorators.into(),
        }
    }

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
        let decoration_context = DecorationContext::new(world.min_y(), world.height());
        self.place_decorated(world, biomes, random, &decoration_context, 0, origin)
    }

    fn place_decorated<W: FeatureWorld, B: FeatureBiomeResolver>(
        &self,
        world: &mut W,
        biomes: &B,
        random: &mut impl RandomSource,
        decoration_context: &DecorationContext,
        decorator_index: usize,
        pos: BlockPos,
    ) -> bool {
        let Some(decorator) = self.decorators.get(decorator_index) else {
            return self.feature.place_with_biomes(world, biomes, random, pos);
        };

        let mut placed_any = false;
        for next_pos in decorator_positions(world, decoration_context, random, *decorator, pos) {
            placed_any |= self.place_decorated(
                world,
                biomes,
                random,
                decoration_context,
                decorator_index + 1,
                next_pos,
            );
        }
        placed_any
    }
}

fn decorator_positions<W: FeatureWorld>(
    world: &mut W,
    context: &DecorationContext,
    random: &mut impl RandomSource,
    decorator: ConfiguredDecorator,
    pos: BlockPos,
) -> Vec<BlockPos> {
    match decorator {
        ConfiguredDecorator::Heightmap(config) => {
            let Some(y) = world.height_at(config.heightmap, pos.x, pos.z) else {
                return Vec::new();
            };
            if y > context.get_min_build_height() {
                vec![BlockPos::new(pos.x, y, pos.z)]
            } else {
                Vec::new()
            }
        }
        ConfiguredDecorator::HeightmapSpreadDouble(config) => {
            let Some(y) = world.height_at(config.heightmap, pos.x, pos.z) else {
                return Vec::new();
            };
            if y == context.get_min_build_height() {
                Vec::new()
            } else {
                vec![BlockPos::new(
                    pos.x,
                    context.get_min_build_height()
                        + random.next_int_bound((y - context.get_min_build_height()) * 2),
                    pos.z,
                )]
            }
        }
        ConfiguredDecorator::WaterDepthThreshold(config) => {
            let Some(ocean_floor) = world.height_at(HeightmapType::OceanFloor, pos.x, pos.z) else {
                return Vec::new();
            };
            let Some(world_surface) = world.height_at(HeightmapType::WorldSurface, pos.x, pos.z)
            else {
                return Vec::new();
            };
            if world_surface - ocean_floor > config.max_water_depth {
                Vec::new()
            } else {
                vec![pos]
            }
        }
        _ => decorator.get_positions(context, random, pos),
    }
}

pub(super) fn place_configured_decorated_feature<W: FeatureWorld, B: FeatureBiomeResolver>(
    world: &mut W,
    biomes: &B,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: &DecoratedFeatureConfiguration,
) -> bool {
    let decoration_context = DecorationContext::new(world.min_y(), world.height());
    place_configured_decorated_feature_at(
        world,
        biomes,
        random,
        &decoration_context,
        config.feature.as_ref(),
        &config.decorators,
        0,
        origin,
    )
}

fn place_configured_decorated_feature_at<W: FeatureWorld, B: FeatureBiomeResolver>(
    world: &mut W,
    biomes: &B,
    random: &mut impl RandomSource,
    decoration_context: &DecorationContext,
    feature: &ConfiguredFeature,
    decorators: &[ConfiguredDecorator],
    decorator_index: usize,
    pos: BlockPos,
) -> bool {
    let Some(decorator) = decorators.get(decorator_index) else {
        return feature.place_with_biomes(world, biomes, random, pos);
    };

    let mut placed_any = false;
    for next_pos in decorator_positions(world, decoration_context, random, *decorator, pos) {
        placed_any |= place_configured_decorated_feature_at(
            world,
            biomes,
            random,
            decoration_context,
            feature,
            decorators,
            decorator_index + 1,
            next_pos,
        );
    }
    placed_any
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DecorationReport {
    pub biome_key: &'static str,
    pub attempted_features: usize,
    pub placed_features: usize,
    pub added_non_air_blocks: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TimedDecorationReport {
    pub report: DecorationReport,
    pub timing: FeatureDecorationTiming,
}

pub fn apply_overworld_biome_decoration(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    chunk: &mut MutableChunkBlockBuffer,
) -> DecorationReport {
    let biome = chunk_primary_biome(biome_source, chunk.chunk_x, chunk.chunk_z);
    let biomes = OverworldFeatureBiomeResolver::new(seed, biome_source);
    apply_overworld_biome_features_with_biomes_timed(seed, biome, &biomes, chunk, true).report
}

pub fn apply_overworld_biome_decoration_to_region(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    region: &mut FeatureRegion,
) -> DecorationReport {
    let biome = chunk_primary_biome(
        biome_source,
        region.center_chunk_x(),
        region.center_chunk_z(),
    );
    let biomes = OverworldFeatureBiomeResolver::new(seed, biome_source);
    apply_overworld_biome_features_with_biomes_timed(seed, biome, &biomes, region, true).report
}

pub(crate) fn apply_overworld_biome_decoration_to_region_timed(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    region: &mut FeatureRegion,
) -> TimedDecorationReport {
    let biome = chunk_primary_biome(
        biome_source,
        region.center_chunk_x(),
        region.center_chunk_z(),
    );
    let biomes = OverworldFeatureBiomeResolver::new(seed, biome_source);
    apply_overworld_biome_features_with_biomes_timed(seed, biome, &biomes, region, false)
}

pub fn apply_overworld_biome_features<W: FeatureWorld>(
    seed: i64,
    biome: BiomeDefinition,
    world: &mut W,
) -> DecorationReport {
    let biomes = ConstantFeatureBiomeResolver::new(biome);
    apply_overworld_biome_features_with_biomes_timed(seed, biome, &biomes, world, true).report
}

fn apply_overworld_biome_features_with_biomes_timed<W: FeatureWorld, B: FeatureBiomeResolver>(
    seed: i64,
    biome: BiomeDefinition,
    biomes: &B,
    world: &mut W,
    count_added_blocks: bool,
) -> TimedDecorationReport {
    let features = tables::overworld_features_for_biome_cached(biome);
    apply_feature_table_with_biomes_timed(
        seed,
        biome,
        biomes,
        features,
        world,
        count_added_blocks,
        0,
    )
}

pub(crate) fn apply_feature_table_to_region_timed(
    seed: i64,
    biome: BiomeDefinition,
    features: &[PlacedFeature],
    region: &mut FeatureRegion,
) -> TimedDecorationReport {
    apply_feature_table_to_region_with_index_offset_timed(seed, biome, features, region, 0)
}

pub(crate) fn apply_feature_table_to_region_with_index_offset_timed(
    seed: i64,
    biome: BiomeDefinition,
    features: &[PlacedFeature],
    region: &mut FeatureRegion,
    feature_index_offset: i32,
) -> TimedDecorationReport {
    let biomes = ConstantFeatureBiomeResolver::new(biome);
    apply_feature_table_with_biomes_timed(
        seed,
        biome,
        &biomes,
        features,
        region,
        false,
        feature_index_offset,
    )
}

fn apply_feature_table_with_biomes_timed<W: FeatureWorld, B: FeatureBiomeResolver>(
    seed: i64,
    biome: BiomeDefinition,
    biomes: &B,
    features: &[PlacedFeature],
    world: &mut W,
    count_added_blocks: bool,
    feature_index_offset: i32,
) -> TimedDecorationReport {
    let before = if count_added_blocks {
        world.non_air_block_count()
    } else {
        0
    };
    let min_block_x = chunk_min_block_coord(world.center_chunk_x());
    let min_block_z = chunk_min_block_coord(world.center_chunk_z());
    let origin = BlockPos::new(min_block_x, world.min_y(), min_block_z);
    let decoration_min_block_x = chunk_min_block_coord(world.decoration_chunk_x());
    let decoration_min_block_z = chunk_min_block_coord(world.decoration_chunk_z());
    let mut random = WorldgenRandom::default();
    let decoration_seed =
        random.set_decoration_seed(seed, decoration_min_block_x, decoration_min_block_z);
    let mut placed_features = 0;
    let mut timing = FeatureDecorationTiming::default();

    for step in DecorationStep::ALL {
        let step_index = step.index();
        let mut feature_index = feature_index_offset;
        let step_start = feature_timing_start();
        for feature in features
            .iter()
            .filter(|feature| feature.step.index() == step_index)
        {
            random.set_feature_seed(decoration_seed, feature_index, step_index);
            if feature.place_with_biomes(world, biomes, &mut random, origin) {
                placed_features += 1;
            }
            feature_index += 1;
        }
        timing.step_us[step_index as usize] += feature_timing_elapsed_us(step_start);
    }

    TimedDecorationReport {
        report: DecorationReport {
            biome_key: biome.key(),
            attempted_features: features.len(),
            placed_features,
            added_non_air_blocks: if count_added_blocks {
                world.non_air_block_count().saturating_sub(before)
            } else {
                0
            },
        },
        timing,
    }
}

fn chunk_primary_biome(
    biome_source: &OverworldBiomeSource,
    chunk_x: i32,
    chunk_z: i32,
) -> BiomeDefinition {
    biome_source.get_primary_biome_definition(chunk_x, chunk_z)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) const CURRENT_TAIGA_VEGETATION_FEATURE_INDEX: i32 = 2;
    pub(crate) const JAVA_TAIGA_VEGETATION_FEATURE_INDEX: i32 = 2;
    pub(crate) const JAVA_TAIGA_WATER_SPRING_FEATURE_INDEX: i32 = 11;
    pub(crate) const JAVA_TAIGA_LAVA_SPRING_FEATURE_INDEX: i32 = 12;

    pub(crate) fn place_taiga_vegetation_with_feature_index<W: FeatureWorld>(
        seed: i64,
        world: &mut W,
        feature_index: i32,
    ) -> bool {
        let min_block_x = chunk_min_block_coord(world.center_chunk_x());
        let min_block_z = chunk_min_block_coord(world.center_chunk_z());
        let origin = BlockPos::new(min_block_x, world.min_y(), min_block_z);
        let mut random = WorldgenRandom::default();
        let decoration_seed = random.set_decoration_seed(seed, min_block_x, min_block_z);
        random.set_feature_seed(
            decoration_seed,
            feature_index,
            DecorationStep::VegetalDecoration.index(),
        );
        tables::taiga_vegetation_feature().place(world, &mut random, origin)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn feature_timing_start() -> Option<Instant> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
fn feature_timing_start() -> Option<()> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn feature_timing_elapsed_us(start: Option<Instant>) -> u128 {
    start.map_or(0, |start| start.elapsed().as_micros())
}

#[cfg(target_arch = "wasm32")]
fn feature_timing_elapsed_us(_start: Option<()>) -> u128 {
    0
}
