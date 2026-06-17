use std::collections::{BTreeMap, BTreeSet};

use crate::biome::OverworldBiomeSource;
use crate::carver::{apply_overworld_air_carvers, apply_overworld_liquid_carvers};
use crate::feature::{
    FEATURES_CHUNK_DEPENDENCY_RADIUS, FEATURES_WRITE_RADIUS_CUTOFF, FeatureRegion,
    apply_overworld_biome_decoration_to_region_timed,
};
use mclone_core::ChunkPos;

use super::timing::{timing_elapsed_us, timing_start};
use super::{
    GeneratedChunk, MutableChunkBlockBuffer, NoiseBasedChunkGenerator, NoiseGeneratorSettings,
    OverworldDependencyGenerationTiming, OverworldFeatureBatchTiming,
};

pub fn generate_overworld_surface_chunk(seed: i64, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    let chunk = generate_overworld_surface_buffer(seed, chunk_x, chunk_z);
    GeneratedChunk::from_mutable_buffer(chunk)
}

pub fn generate_overworld_features_chunk(seed: i64, chunk_x: i32, chunk_z: i32) -> GeneratedChunk {
    let pos = ChunkPos::new(chunk_x, chunk_z);
    generate_overworld_features_chunks(seed, [pos])
        .remove(&pos)
        .unwrap_or_else(|| {
            panic!("feature batch did not return target chunk ({chunk_x}, {chunk_z})")
        })
}

pub fn generate_overworld_features_chunks(
    seed: i64,
    targets: impl IntoIterator<Item = ChunkPos>,
) -> BTreeMap<ChunkPos, GeneratedChunk> {
    let mut cache = OverworldFeatureDependencyCache::new();
    cache.generate_features_chunks(seed, targets).chunks
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OverworldFeatureDependencyCacheReport {
    pub requested_dependency_chunks: usize,
    pub cache_hits: usize,
    pub generated_dependency_chunks: usize,
    pub retained_dependency_chunks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverworldFeatureBatchResult {
    pub chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    pub retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    pub cache_report: OverworldFeatureDependencyCacheReport,
    pub timing: OverworldFeatureBatchTiming,
}

#[derive(Debug, Default)]
pub struct OverworldFeatureDependencyCache {
    seed: Option<i64>,
    chunks: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
}

impl OverworldFeatureDependencyCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn retained_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn clear(&mut self) {
        self.seed = None;
        self.chunks.clear();
    }

    pub fn generate_features_chunks(
        &mut self,
        seed: i64,
        targets: impl IntoIterator<Item = ChunkPos>,
    ) -> OverworldFeatureBatchResult {
        self.generate_features_chunks_with_dependencies(seed, targets, std::iter::empty())
    }

    pub fn generate_features_chunks_with_dependencies(
        &mut self,
        seed: i64,
        targets: impl IntoIterator<Item = ChunkPos>,
        dependencies: impl IntoIterator<Item = MutableChunkBlockBuffer>,
    ) -> OverworldFeatureBatchResult {
        if self.seed != Some(seed) {
            self.seed = Some(seed);
            self.chunks.clear();
        }

        let mut timing = OverworldFeatureBatchTiming::default();
        let seed_dependency_insert_start = timing_start();
        for dependency in dependencies {
            self.chunks.insert(
                ChunkPos::new(dependency.chunk_x, dependency.chunk_z),
                dependency,
            );
        }
        timing.seed_dependency_insert_us = timing_elapsed_us(seed_dependency_insert_start);

        let plan_start = timing_start();
        let plan = FeatureBatchPlan::new(targets);
        timing.plan_us = timing_elapsed_us(plan_start);
        let mut cache_report = OverworldFeatureDependencyCacheReport {
            requested_dependency_chunks: plan.dependency_chunks.len(),
            ..OverworldFeatureDependencyCacheReport::default()
        };

        if plan.targets.is_empty() {
            cache_report.retained_dependency_chunks = self.chunks.len();
            return OverworldFeatureBatchResult {
                chunks: BTreeMap::new(),
                retained_dependencies: self.chunks.clone(),
                cache_report,
                timing,
            };
        }

        let biome_source = OverworldBiomeSource::new(seed, false, false);
        let mut dependency_generator = None;
        let mut region_chunks = Vec::with_capacity(plan.dependency_chunks.len());
        for pos in sorted_chunk_positions_z_major(plan.dependency_chunks.iter().copied()) {
            if let Some(chunk) = self.chunks.get(&pos) {
                cache_report.cache_hits += 1;
                let clone_start = timing_start();
                region_chunks.push(chunk.clone());
                timing.dependency_cache_hit_clone_us += timing_elapsed_us(clone_start);
                continue;
            }

            if dependency_generator.is_none() {
                let generator_setup_start = timing_start();
                dependency_generator = Some(NoiseBasedChunkGenerator::new(
                    biome_source.clone(),
                    seed,
                    NoiseGeneratorSettings::overworld(),
                ));
                timing.dependency_generation.generator_setup_us +=
                    timing_elapsed_us(generator_setup_start);
            }
            let generator = dependency_generator
                .as_ref()
                .expect("dependency generator was just initialized");
            let generate_start = timing_start();
            let generated = generate_overworld_liquid_carved_buffer_with_generator_timed(
                seed,
                pos.x,
                pos.z,
                &biome_source,
                generator,
            );
            timing.dependency_generate_us += timing_elapsed_us(generate_start);
            timing.dependency_generation.add_assign(generated.timing);
            let chunk = generated.chunk;
            cache_report.generated_dependency_chunks += 1;
            let insert_start = timing_start();
            self.chunks.insert(pos, chunk.clone());
            region_chunks.push(chunk);
            timing.dependency_insert_clone_us += timing_elapsed_us(insert_start);
        }

        let retain_start = timing_start();
        self.chunks
            .retain(|pos, _| plan.dependency_chunks.contains(pos));
        timing.dependency_retain_us = timing_elapsed_us(retain_start);
        cache_report.retained_dependency_chunks = self.chunks.len();
        let retained_clone_start = timing_start();
        let retained_dependencies = self.chunks.clone();
        timing.retained_dependency_clone_us = timing_elapsed_us(retained_clone_start);

        let feature_result = generate_overworld_features_chunks_from_plan_timed(
            seed,
            &biome_source,
            plan,
            region_chunks,
        );
        timing.add_assign(feature_result.timing);
        OverworldFeatureBatchResult {
            chunks: feature_result.chunks,
            retained_dependencies,
            cache_report,
            timing,
        }
    }
}

#[derive(Debug)]
struct FeatureBatchChunkResult {
    chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    timing: OverworldFeatureBatchTiming,
}

fn generate_overworld_features_chunks_from_plan_timed(
    seed: i64,
    biome_source: &OverworldBiomeSource,
    plan: FeatureBatchPlan,
    chunks: Vec<MutableChunkBlockBuffer>,
) -> FeatureBatchChunkResult {
    if plan.targets.is_empty() {
        return FeatureBatchChunkResult {
            chunks: BTreeMap::new(),
            timing: OverworldFeatureBatchTiming::default(),
        };
    }

    let mut timing = OverworldFeatureBatchTiming::default();
    let first_target = *plan.targets.iter().next().expect("non-empty targets");
    let region_init_start = timing_start();
    let mut region = FeatureRegion::new(first_target.x, first_target.z, chunks);
    timing.feature_region_init_us = timing_elapsed_us(region_init_start);

    let decoration_start = timing_start();
    for center in plan.ordered_feature_centers() {
        region.set_center(center.x, center.z);
        let report =
            apply_overworld_biome_decoration_to_region_timed(seed, biome_source, &mut region);
        timing.feature_decoration_steps.add_assign(report.timing);
    }
    timing.feature_decoration_us = timing_elapsed_us(decoration_start);

    let target_extract_start = timing_start();
    let mut generated = BTreeMap::new();
    for target in plan.targets {
        let chunk = region.remove_chunk(target.x, target.z).unwrap_or_else(|| {
            panic!(
                "feature region did not retain target chunk ({}, {})",
                target.x, target.z
            )
        });
        generated.insert(target, GeneratedChunk::from_mutable_buffer(chunk));
    }
    timing.target_extract_us = timing_elapsed_us(target_extract_start);
    FeatureBatchChunkResult {
        chunks: generated,
        timing,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FeatureBatchPlan {
    pub(super) targets: BTreeSet<ChunkPos>,
    pub(super) feature_centers: BTreeSet<ChunkPos>,
    pub(super) dependency_chunks: BTreeSet<ChunkPos>,
}

impl FeatureBatchPlan {
    pub(super) fn new(targets: impl IntoIterator<Item = ChunkPos>) -> Self {
        let targets = targets.into_iter().collect::<BTreeSet<_>>();
        let mut feature_centers = BTreeSet::new();
        let mut dependency_chunks = BTreeSet::new();

        for target in &targets {
            for dz in -FEATURES_WRITE_RADIUS_CUTOFF..=FEATURES_WRITE_RADIUS_CUTOFF {
                for dx in -FEATURES_WRITE_RADIUS_CUTOFF..=FEATURES_WRITE_RADIUS_CUTOFF {
                    feature_centers.insert(ChunkPos::new(target.x + dx, target.z + dz));
                }
            }
        }

        for center in &feature_centers {
            for dz in -FEATURES_CHUNK_DEPENDENCY_RADIUS..=FEATURES_CHUNK_DEPENDENCY_RADIUS {
                for dx in -FEATURES_CHUNK_DEPENDENCY_RADIUS..=FEATURES_CHUNK_DEPENDENCY_RADIUS {
                    dependency_chunks.insert(ChunkPos::new(center.x + dx, center.z + dz));
                }
            }
        }

        Self {
            targets,
            feature_centers,
            dependency_chunks,
        }
    }

    pub(super) fn ordered_feature_centers(&self) -> Vec<ChunkPos> {
        sorted_chunk_positions_z_major(self.feature_centers.iter().copied())
    }
}

pub(super) fn sorted_chunk_positions_z_major(
    positions: impl IntoIterator<Item = ChunkPos>,
) -> Vec<ChunkPos> {
    let mut positions = positions.into_iter().collect::<Vec<_>>();
    positions.sort_by_key(|pos| (pos.z, pos.x));
    positions
}

fn generate_overworld_surface_buffer(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
) -> MutableChunkBlockBuffer {
    generate_overworld_surface_buffer_with_biome_source(
        seed,
        chunk_x,
        chunk_z,
        OverworldBiomeSource::new(seed, false, false),
    )
}

fn generate_overworld_surface_buffer_with_biome_source(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    biome_source: OverworldBiomeSource,
) -> MutableChunkBlockBuffer {
    let generator =
        NoiseBasedChunkGenerator::new(biome_source, seed, NoiseGeneratorSettings::overworld());
    let mut chunk = generator.fill_from_noise(chunk_x, chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    chunk
}

#[cfg(test)]
pub(super) fn generate_overworld_liquid_carved_buffer_with_biome_source(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    biome_source: OverworldBiomeSource,
) -> MutableChunkBlockBuffer {
    generate_overworld_liquid_carved_buffer_with_biome_source_timed(
        seed,
        chunk_x,
        chunk_z,
        biome_source,
    )
    .chunk
}

#[derive(Debug)]
struct TimedDependencyBuffer {
    chunk: MutableChunkBlockBuffer,
    timing: OverworldDependencyGenerationTiming,
}

#[cfg(test)]
fn generate_overworld_liquid_carved_buffer_with_biome_source_timed(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    biome_source: OverworldBiomeSource,
) -> TimedDependencyBuffer {
    let generator_setup_start = timing_start();
    let generator = NoiseBasedChunkGenerator::new(
        biome_source.clone(),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut timing = OverworldDependencyGenerationTiming::default();
    timing.generator_setup_us = timing_elapsed_us(generator_setup_start);
    generate_overworld_liquid_carved_buffer_with_generator_timed(
        seed,
        chunk_x,
        chunk_z,
        &biome_source,
        &generator,
    )
    .with_generator_setup_us(timing.generator_setup_us)
}

fn generate_overworld_liquid_carved_buffer_with_generator_timed(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    biome_source: &OverworldBiomeSource,
    generator: &NoiseBasedChunkGenerator<OverworldBiomeSource>,
) -> TimedDependencyBuffer {
    let mut timing = OverworldDependencyGenerationTiming::default();
    let surface_fill_start = timing_start();
    let surface_fill = generator.fill_from_noise_timed(chunk_x, chunk_z);
    timing.surface_fill_us = timing_elapsed_us(surface_fill_start);
    timing.surface_fill = surface_fill.timing;
    let mut chunk = surface_fill.chunk;

    let surface_bedrock_start = timing_start();
    generator.build_surface_and_bedrock(&mut chunk);
    timing.surface_bedrock_us = timing_elapsed_us(surface_bedrock_start);

    let air_carvers_start = timing_start();
    apply_overworld_air_carvers(seed, biome_source, &mut chunk);
    timing.air_carvers_us = timing_elapsed_us(air_carvers_start);

    let liquid_carvers_start = timing_start();
    apply_overworld_liquid_carvers(seed, biome_source, &mut chunk);
    timing.liquid_carvers_us = timing_elapsed_us(liquid_carvers_start);

    let heightmap_prime_start = timing_start();
    chunk.prime_worldgen_heightmaps();
    timing.heightmap_prime_us = timing_elapsed_us(heightmap_prime_start);

    TimedDependencyBuffer { chunk, timing }
}

#[cfg(test)]
impl TimedDependencyBuffer {
    fn with_generator_setup_us(mut self, generator_setup_us: u128) -> Self {
        self.timing.generator_setup_us += generator_setup_us;
        self
    }
}
