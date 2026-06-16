use std::collections::{BTreeMap, BTreeSet};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::biome::OverworldBiomeSource;
use crate::block::{AIR, BEDROCK};
use crate::carver::{apply_overworld_air_carvers, apply_overworld_liquid_carvers};
use crate::feature::{
    FEATURES_CHUNK_DEPENDENCY_RADIUS, FEATURES_WRITE_RADIUS_CUTOFF, FeatureDecorationTiming,
    FeatureRegion, apply_overworld_biome_decoration_to_region_timed,
};
use crate::noise::{BlendedNoise, PerlinNoise, PerlinSimplexNoise, SimplexNoise};
use crate::prng::WorldgenRandom;
use crate::surface::apply_overworld_surface;
use mclone_core::{CHUNK_WIDTH, ChunkPos, chunk_min_block_coord};

#[cfg(test)]
use crate::block::RawBlockId;
#[cfg(test)]
use mclone_core::chunk_block_index;

mod chunk;
mod sampler;
mod settings;

use chunk::world_surface_height;
pub use chunk::{GeneratedChunk, MutableChunkBlockBuffer, ScheduledTick};
use sampler::{BIOME_WEIGHT_RADIUS, BiomeDensity, compute_biome_density_from_neighborhood};
pub use sampler::{ConstantBiomeSource, NoiseBiome, NoiseBiomeSource, NoiseSampler};
pub use settings::{
    NoiseGeneratorSettings, NoiseModifier, NoiseSamplingSettings, NoiseSettings, NoiseSlideSettings,
};

const SURFACE_NOISE_OCTAVES: [i32; 4] = [-3, -2, -1, 0];
const DEPTH_NOISE_OCTAVES: [i32; 16] = [
    -15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0,
];

#[derive(Clone, Debug)]
enum SurfaceNoiseSource {
    Perlin(PerlinNoise),
    PerlinSimplex(PerlinSimplexNoise),
}

impl SurfaceNoiseSource {
    fn get_surface_noise_value(&self, x: f64, y: f64, z: f64, y_max: f64) -> f64 {
        match self {
            Self::Perlin(noise) => noise.get_surface_noise_value(x, y, z, y_max),
            Self::PerlinSimplex(noise) => noise.get_surface_noise_value(x, y, z, y_max),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NoiseBasedChunkGenerator<B: NoiseBiomeSource> {
    seed: i64,
    cell_height: i32,
    cell_width: i32,
    cell_count_x: i32,
    cell_count_y: i32,
    cell_count_z: i32,
    min_y: i32,
    height: i32,
    min_cell_y: i32,
    sea_level: i32,
    settings: NoiseGeneratorSettings,
    surface_noise: SurfaceNoiseSource,
    sampler: NoiseSampler<B>,
}

#[derive(Debug)]
struct TimedSurfaceFill {
    chunk: MutableChunkBlockBuffer,
    timing: SurfaceFillTiming,
}

#[derive(Clone, Debug)]
struct NoiseBiomeCache {
    min_x: i32,
    min_z: i32,
    z_count: i32,
    biomes: Vec<NoiseBiome>,
}

impl NoiseBiomeCache {
    fn get(&self, x: i32, z: i32) -> NoiseBiome {
        let index = ((x - self.min_x) * self.z_count + (z - self.min_z)) as usize;
        self.biomes[index]
    }
}

impl<B: NoiseBiomeSource> NoiseBasedChunkGenerator<B> {
    pub fn new(biome_source: B, seed: i64, settings: NoiseGeneratorSettings) -> Self {
        if settings.is_aquifers_enabled()
            || settings.is_noise_caves_enabled()
            || settings.is_deepslate_enabled()
            || settings.is_ore_veins_enabled()
            || settings.is_noodle_caves_enabled()
        {
            panic!("This terrain-only generator only supports the default 1.17.1 overworld flags");
        }

        let noise_settings = settings.noise_settings().clone();
        let min_y = noise_settings.min_y();
        let height = noise_settings.height();
        let cell_height = noise_settings.noise_size_vertical() * 4;
        let cell_width = noise_settings.noise_size_horizontal() * 4;
        let cell_count_x = CHUNK_WIDTH / cell_width;
        let cell_count_y = height / cell_height;
        let cell_count_z = CHUNK_WIDTH / cell_width;
        let min_cell_y = min_y.div_euclid(cell_height);
        let sea_level = settings.sea_level();

        let mut random = WorldgenRandom::new(seed);
        let blended_noise = BlendedNoise::new(&mut random);
        let surface_noise = if noise_settings.use_simplex_surface_noise() {
            SurfaceNoiseSource::PerlinSimplex(PerlinSimplexNoise::from_octaves(
                &mut random,
                &SURFACE_NOISE_OCTAVES,
            ))
        } else {
            SurfaceNoiseSource::Perlin(PerlinNoise::from_octaves(
                &mut random,
                &SURFACE_NOISE_OCTAVES,
            ))
        };
        random.consume_count(2620);
        let depth_noise = PerlinNoise::from_octaves(&mut random, &DEPTH_NOISE_OCTAVES);

        let island_noise = if noise_settings.island_noise_override() {
            let mut island_random = WorldgenRandom::new(seed);
            island_random.consume_count(17292);
            Some(SimplexNoise::new(&mut island_random))
        } else {
            None
        };

        let sampler = NoiseSampler::new(
            biome_source,
            cell_width,
            cell_height,
            cell_count_y,
            noise_settings,
            blended_noise,
            island_noise,
            depth_noise,
            NoiseModifier::Passthrough,
        );

        Self {
            seed,
            cell_height,
            cell_width,
            cell_count_x,
            cell_count_y,
            cell_count_z,
            min_y,
            height,
            min_cell_y,
            sea_level,
            settings,
            surface_noise,
            sampler,
        }
    }

    pub fn fill_from_noise(&self, chunk_x: i32, chunk_z: i32) -> MutableChunkBlockBuffer {
        let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, self.min_y, self.height);
        self.fill_terrain_block_buffer(&mut chunk);
        chunk
    }

    fn fill_from_noise_timed(&self, chunk_x: i32, chunk_z: i32) -> TimedSurfaceFill {
        let mut timing = SurfaceFillTiming::default();

        let chunk_alloc_start = timing_start();
        let mut chunk = MutableChunkBlockBuffer::new(chunk_x, chunk_z, self.min_y, self.height);
        timing.chunk_alloc_us = timing_elapsed_us(chunk_alloc_start);

        let fill_timing = self.fill_terrain_block_buffer_timed(&mut chunk);
        timing.add_assign(fill_timing);

        TimedSurfaceFill { chunk, timing }
    }

    fn fill_terrain_block_buffer(&self, chunk: &mut MutableChunkBlockBuffer) {
        let noise_columns = self.create_noise_columns(chunk.chunk_x, chunk.chunk_z);
        self.fill_terrain_block_buffer_from_columns(chunk, &noise_columns);
    }

    fn fill_terrain_block_buffer_timed(
        &self,
        chunk: &mut MutableChunkBlockBuffer,
    ) -> SurfaceFillTiming {
        let mut timing = SurfaceFillTiming::default();

        let noise_columns_start = timing_start();
        let noise_columns = self.create_noise_columns(chunk.chunk_x, chunk.chunk_z);
        timing.noise_columns_us = timing_elapsed_us(noise_columns_start);

        let terrain_fill_start = timing_start();
        timing.non_air_blocks_written =
            self.fill_terrain_block_buffer_from_columns(chunk, &noise_columns);
        timing.terrain_fill_us = timing_elapsed_us(terrain_fill_start);
        timing
    }

    fn fill_terrain_block_buffer_from_columns(
        &self,
        chunk: &mut MutableChunkBlockBuffer,
        noise_columns: &[f64],
    ) -> usize {
        let mut non_air_blocks_written = 0;
        let column_y_count = (self.cell_count_y + 1) as usize;
        let column_z_stride = column_y_count;
        let column_x_stride = (self.cell_count_z + 1) as usize * column_y_count;
        let inv_cell_height = 1.0 / self.cell_height as f64;
        let inv_cell_width = 1.0 / self.cell_width as f64;

        for cell_x in 0..self.cell_count_x {
            for cell_z in 0..self.cell_count_z {
                for cell_y in (0..self.cell_count_y).rev() {
                    let column_base = cell_x as usize * column_x_stride
                        + cell_z as usize * column_z_stride
                        + cell_y as usize;
                    let x0z0y0 = noise_columns[column_base];
                    let x0z0y1 = noise_columns[column_base + 1];
                    let x1z0y0 = noise_columns[column_base + column_x_stride];
                    let x1z0y1 = noise_columns[column_base + column_x_stride + 1];
                    let x0z1y0 = noise_columns[column_base + column_z_stride];
                    let x0z1y1 = noise_columns[column_base + column_z_stride + 1];
                    let x1z1y0 = noise_columns[column_base + column_x_stride + column_z_stride];
                    let x1z1y1 = noise_columns[column_base + column_x_stride + column_z_stride + 1];

                    for y_offset in (0..self.cell_height).rev() {
                        let y_fraction = y_offset as f64 * inv_cell_height;
                        let block_y = (self.min_cell_y + cell_y) * self.cell_height + y_offset;
                        let local_y = block_y - self.min_y;
                        let local_y_base = (local_y as usize) << 8;
                        let x0z0 = lerp(y_fraction, x0z0y0, x0z0y1);
                        let x1z0 = lerp(y_fraction, x1z0y0, x1z0y1);
                        let x0z1 = lerp(y_fraction, x0z1y0, x0z1y1);
                        let x1z1 = lerp(y_fraction, x1z1y0, x1z1y1);

                        for x_offset in 0..self.cell_width {
                            let x_fraction = x_offset as f64 * inv_cell_width;
                            let local_x = cell_x * self.cell_width + x_offset;
                            let z0 = lerp(x_fraction, x0z0, x1z0);
                            let z1 = lerp(x_fraction, x0z1, x1z1);

                            for z_offset in 0..self.cell_width {
                                let z_fraction = z_offset as f64 * inv_cell_width;
                                let density = lerp(z_fraction, z0, z1);
                                let block_id = self.resolve_terrain_block(block_y, density);
                                if block_id != AIR {
                                    let local_z = cell_z * self.cell_width + z_offset;
                                    let index =
                                        local_y_base | ((local_z as usize) << 4) | local_x as usize;
                                    chunk.blocks[index] = block_id;
                                    non_air_blocks_written += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        non_air_blocks_written
    }

    fn create_noise_columns(&self, chunk_x: i32, chunk_z: i32) -> Vec<f64> {
        let mut columns = vec![
            0.0;
            (self.cell_count_x + 1) as usize
                * (self.cell_count_z + 1) as usize
                * (self.cell_count_y + 1) as usize
        ];
        let cell_min_x = chunk_x * self.cell_count_x;
        let cell_min_z = chunk_z * self.cell_count_z;
        let noise_settings = self.settings.noise_settings();
        let biome_cache = self.create_noise_biome_cache(cell_min_x, cell_min_z);
        let column_y_count = self.cell_count_y as usize + 1;
        let column_z_stride = column_y_count;
        let column_x_stride = (self.cell_count_z + 1) as usize * column_y_count;
        let mut noise_values = vec![0.0; column_y_count];

        for cell_x in 0..=self.cell_count_x {
            for cell_z in 0..=self.cell_count_z {
                let noise_cell_x = cell_min_x + cell_x;
                let noise_cell_z = cell_min_z + cell_z;
                let density = self.compute_cached_biome_density(
                    noise_cell_x,
                    noise_cell_z,
                    noise_settings,
                    &biome_cache,
                );
                self.sampler.fill_noise_column_with_density(
                    &mut noise_values,
                    noise_cell_x,
                    noise_cell_z,
                    noise_settings,
                    self.min_cell_y,
                    self.cell_count_y,
                    density,
                );

                let column_base =
                    cell_x as usize * column_x_stride + cell_z as usize * column_z_stride;
                columns[column_base..column_base + column_y_count].copy_from_slice(&noise_values);
            }
        }

        columns
    }

    fn create_noise_biome_cache(&self, cell_min_x: i32, cell_min_z: i32) -> NoiseBiomeCache {
        let min_x = cell_min_x - BIOME_WEIGHT_RADIUS;
        let min_z = cell_min_z - BIOME_WEIGHT_RADIUS;
        let width = self.cell_count_x + 1 + BIOME_WEIGHT_RADIUS * 2;
        let depth = self.cell_count_z + 1 + BIOME_WEIGHT_RADIUS * 2;
        let mut biomes = Vec::with_capacity((width * depth) as usize);

        for offset_x in 0..width {
            for offset_z in 0..depth {
                biomes.push(self.sampler.biome_source.get_noise_biome(
                    min_x + offset_x,
                    self.sea_level,
                    min_z + offset_z,
                ));
            }
        }

        NoiseBiomeCache {
            min_x,
            min_z,
            z_count: depth,
            biomes,
        }
    }

    fn compute_cached_biome_density(
        &self,
        cell_x: i32,
        cell_z: i32,
        noise_settings: &NoiseSettings,
        biome_cache: &NoiseBiomeCache,
    ) -> BiomeDensity {
        let center_depth = biome_cache.get(cell_x, cell_z).get_depth();
        compute_biome_density_from_neighborhood(
            center_depth,
            noise_settings,
            |offset_x, offset_z| biome_cache.get(cell_x + offset_x, cell_z + offset_z),
        )
    }

    fn resolve_terrain_block(&self, y: i32, noise: f64) -> u8 {
        let mut density = (noise / 200.0).clamp(-1.0, 1.0);
        density = density / 2.0 - density * density * density / 24.0;
        if density > 0.0 {
            self.settings.default_block()
        } else if y >= self.sea_level {
            AIR
        } else {
            self.settings.default_fluid()
        }
    }

    fn assert_compatible_chunk(&self, chunk: &MutableChunkBlockBuffer) {
        if chunk.min_y != self.min_y || chunk.height != self.height {
            panic!(
                "chunk height range {}..{} is incompatible with generator range {}..{}",
                chunk.min_y,
                chunk.min_y + chunk.height,
                self.min_y,
                self.min_y + self.height
            );
        }
    }
}

impl NoiseBasedChunkGenerator<OverworldBiomeSource> {
    pub fn build_surface_and_bedrock(&self, chunk: &mut MutableChunkBlockBuffer) {
        self.assert_compatible_chunk(chunk);
        let mut random = WorldgenRandom::default();
        random.set_base_chunk_seed(chunk.chunk_x, chunk.chunk_z);
        self.build_surface(chunk, &mut random);
        self.set_bedrock(chunk, &mut random);
    }

    fn build_surface(&self, chunk: &mut MutableChunkBlockBuffer, random: &mut WorldgenRandom) {
        let min_block_x = chunk_min_block_coord(chunk.chunk_x);
        let min_block_z = chunk_min_block_coord(chunk.chunk_z);
        let min_surface_level = self.settings.min_surface_level();

        for local_x in 0..CHUNK_WIDTH {
            for local_z in 0..CHUNK_WIDTH {
                let x = min_block_x + local_x;
                let z = min_block_z + local_z;
                let height_y = world_surface_height(chunk, local_x, local_z);
                let surface_value = self.surface_noise.get_surface_noise_value(
                    x as f64 * 0.0625,
                    z as f64 * 0.0625,
                    0.0625,
                    local_x as f64 * 0.0625,
                ) * 15.0;
                let biome = self
                    .sampler
                    .biome_source
                    .get_block_position_biome_definition(self.seed, x, z);
                apply_overworld_surface(
                    random,
                    chunk,
                    biome,
                    x,
                    z,
                    height_y,
                    surface_value,
                    self.sea_level,
                    min_surface_level,
                    self.seed,
                );
            }
        }
    }

    fn set_bedrock(&self, chunk: &mut MutableChunkBlockBuffer, random: &mut WorldgenRandom) {
        let floor_y = self.min_y as i64 + self.settings.bedrock_floor_position() as i64;
        let roof_y = self.height as i64 - 1 + self.min_y as i64
            - self.settings.bedrock_roof_position() as i64;
        let min_build_height = self.min_y as i64;
        let max_build_height = (self.min_y + self.height) as i64;
        let has_roof = roof_y + 4 >= min_build_height && roof_y < max_build_height;
        let has_floor = floor_y + 4 >= min_build_height && floor_y < max_build_height;

        if !has_roof && !has_floor {
            return;
        }

        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                if has_roof {
                    for offset in 0..5 {
                        if offset <= random.next_int_bound(5) {
                            set_block_at_i64_y_if_inside(
                                chunk,
                                local_x,
                                roof_y - offset as i64,
                                local_z,
                                BEDROCK,
                            );
                        }
                    }
                }

                if has_floor {
                    for offset in (0..5).rev() {
                        if offset <= random.next_int_bound(5) {
                            set_block_at_i64_y_if_inside(
                                chunk,
                                local_x,
                                floor_y + offset as i64,
                                local_z,
                                BEDROCK,
                            );
                        }
                    }
                }
            }
        }
    }
}

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OverworldFeatureBatchTiming {
    pub seed_dependency_insert_us: u128,
    pub plan_us: u128,
    pub dependency_cache_hit_clone_us: u128,
    pub dependency_generate_us: u128,
    pub dependency_generation: OverworldDependencyGenerationTiming,
    pub dependency_insert_clone_us: u128,
    pub dependency_retain_us: u128,
    pub retained_dependency_clone_us: u128,
    pub feature_region_init_us: u128,
    pub feature_decoration_us: u128,
    pub feature_decoration_steps: FeatureDecorationTiming,
    pub target_extract_us: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SurfaceFillTiming {
    pub chunk_alloc_us: u128,
    pub noise_columns_us: u128,
    pub terrain_fill_us: u128,
    pub non_air_blocks_written: usize,
}

impl SurfaceFillTiming {
    pub fn total_us(self) -> u128 {
        self.chunk_alloc_us + self.noise_columns_us + self.terrain_fill_us
    }

    pub fn add_assign(&mut self, other: Self) {
        self.chunk_alloc_us += other.chunk_alloc_us;
        self.noise_columns_us += other.noise_columns_us;
        self.terrain_fill_us += other.terrain_fill_us;
        self.non_air_blocks_written += other.non_air_blocks_written;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OverworldDependencyGenerationTiming {
    pub generator_setup_us: u128,
    pub surface_fill_us: u128,
    pub surface_fill: SurfaceFillTiming,
    pub surface_bedrock_us: u128,
    pub air_carvers_us: u128,
    pub liquid_carvers_us: u128,
    pub heightmap_prime_us: u128,
}

impl OverworldDependencyGenerationTiming {
    pub fn total_us(self) -> u128 {
        self.generator_setup_us
            + self.surface_fill_us
            + self.surface_bedrock_us
            + self.air_carvers_us
            + self.liquid_carvers_us
            + self.heightmap_prime_us
    }

    pub fn add_assign(&mut self, other: Self) {
        self.generator_setup_us += other.generator_setup_us;
        self.surface_fill_us += other.surface_fill_us;
        self.surface_fill.add_assign(other.surface_fill);
        self.surface_bedrock_us += other.surface_bedrock_us;
        self.air_carvers_us += other.air_carvers_us;
        self.liquid_carvers_us += other.liquid_carvers_us;
        self.heightmap_prime_us += other.heightmap_prime_us;
    }
}

impl OverworldFeatureBatchTiming {
    pub fn total_us(self) -> u128 {
        self.seed_dependency_insert_us
            + self.plan_us
            + self.dependency_cache_hit_clone_us
            + self.dependency_generate_us
            + self.dependency_insert_clone_us
            + self.dependency_retain_us
            + self.retained_dependency_clone_us
            + self.feature_region_init_us
            + self.feature_decoration_us
            + self.target_extract_us
    }

    pub fn add_assign(&mut self, other: Self) {
        self.seed_dependency_insert_us += other.seed_dependency_insert_us;
        self.plan_us += other.plan_us;
        self.dependency_cache_hit_clone_us += other.dependency_cache_hit_clone_us;
        self.dependency_generate_us += other.dependency_generate_us;
        self.dependency_generation
            .add_assign(other.dependency_generation);
        self.dependency_insert_clone_us += other.dependency_insert_clone_us;
        self.dependency_retain_us += other.dependency_retain_us;
        self.retained_dependency_clone_us += other.retained_dependency_clone_us;
        self.feature_region_init_us += other.feature_region_init_us;
        self.feature_decoration_us += other.feature_decoration_us;
        self.feature_decoration_steps
            .add_assign(other.feature_decoration_steps);
        self.target_extract_us += other.target_extract_us;
    }
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

#[cfg(not(target_arch = "wasm32"))]
fn timing_start() -> Option<Instant> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
fn timing_start() -> Option<()> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn timing_elapsed_us(start: Option<Instant>) -> u128 {
    start.map_or(0, |start| start.elapsed().as_micros())
}

#[cfg(target_arch = "wasm32")]
fn timing_elapsed_us(_start: Option<()>) -> u128 {
    0
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FeatureBatchPlan {
    targets: BTreeSet<ChunkPos>,
    feature_centers: BTreeSet<ChunkPos>,
    dependency_chunks: BTreeSet<ChunkPos>,
}

impl FeatureBatchPlan {
    fn new(targets: impl IntoIterator<Item = ChunkPos>) -> Self {
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

    fn ordered_feature_centers(&self) -> Vec<ChunkPos> {
        sorted_chunk_positions_z_major(self.feature_centers.iter().copied())
    }
}

fn sorted_chunk_positions_z_major(positions: impl IntoIterator<Item = ChunkPos>) -> Vec<ChunkPos> {
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
fn generate_overworld_liquid_carved_buffer_with_biome_source(
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

fn set_block_at_i64_y_if_inside(
    chunk: &mut MutableChunkBlockBuffer,
    local_x: i32,
    y: i64,
    local_z: i32,
    block_id: u8,
) {
    if y >= chunk.min_y as i64 && y < (chunk.min_y + chunk.height) as i64 {
        chunk.set_block_at_y(local_x, y as i32, local_z, block_id);
    }
}

fn lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::OverworldBiomeSource;
    use crate::block::{STONE, WATER};
    use crate::feature::FeatureWorld;
    use crate::prng::WorldgenRandom;
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use std::panic;

    const DEPTH_NOISE_OCTAVES: [i32; 16] = [
        -15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0,
    ];
    const BEDROCK: u8 = 3;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerFixture {
        module: String,
        minecraft_version: String,
        noise_class: String,
        random_source_class: String,
        settings_preset: String,
        noise_modifier: String,
        seed: String,
        cell_width: i32,
        cell_height: i32,
        cell_count_y: i32,
        biome_y: i32,
        min_cell_y: i32,
        column_value_count: usize,
        wire_format: NoiseSamplerWireFormatFixture,
        noise_settings: NoiseSettingsFixture,
        sample_sets: BTreeMap<String, NoiseSamplerSampleSetFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerWireFormatFixture {
        coordinates: String,
        biome_keys: String,
        biome_factors: String,
        values: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSettingsFixture {
        min_y: i32,
        height: i32,
        sampling: NoiseSamplingSettingsFixture,
        top_slide: NoiseSlideSettingsFixture,
        bottom_slide: NoiseSlideSettingsFixture,
        noise_size_horizontal: i32,
        noise_size_vertical: i32,
        density_factor: f64,
        density_offset: f64,
        use_simplex_surface_noise: bool,
        random_density_offset: bool,
        island_noise_override: bool,
        is_amplified: bool,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplingSettingsFixture {
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
    }

    #[derive(Debug, Deserialize)]
    struct NoiseSlideSettingsFixture {
        target: i32,
        size: i32,
        offset: i32,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerSampleSetFixture {
        biome_pattern: NoiseSamplerBiomePatternFixture,
        columns: NoiseSamplerColumnFixture,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerBiomePatternFixture {
        grid_order: String,
        sample_count: usize,
        x: Vec<i32>,
        z: Vec<i32>,
        keys: Vec<String>,
        depths: Vec<f32>,
        scales: Vec<f32>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseSamplerColumnFixture {
        noise_method: String,
        grid_order: String,
        column_value_count: usize,
        sample_count: usize,
        x: Vec<i32>,
        z: Vec<i32>,
        values: Vec<f64>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TerrainChunkOracleFixture {
        module: String,
        minecraft_version: String,
        generator_class: String,
        seed: String,
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        block_order: String,
        palette: Vec<String>,
        blocks: Vec<u8>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FullChunkOracleFixture {
        module: String,
        minecraft_version: String,
        seed: String,
        generator: String,
        generate_structures: bool,
        wire_format: FullChunkWireFormatFixture,
        chunks: Vec<FullChunkEntryFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FullChunkWireFormatFixture {
        block_order: String,
        palette_entries: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FullChunkEntryFixture {
        chunk_x: i32,
        chunk_z: i32,
        status: String,
        sections: Vec<FullChunkSectionFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SchedulerTraceFixture {
        module: String,
        minecraft_version: String,
        seed: String,
        target_chunk_x: i32,
        target_chunk_z: i32,
        target_radius: i32,
        stop_status: String,
        #[serde(rename = "featureCompletionOrder3x3")]
        feature_completion_order_3x3: Vec<SchedulerTraceChunkFixture>,
        #[serde(default)]
        chunks: Vec<FullChunkEntryFixture>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SchedulerTraceChunkFixture {
        chunk_x: i32,
        chunk_z: i32,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FullChunkSectionFixture {
        y: i32,
        palette: Vec<String>,
        block_order: String,
        blocks: Vec<usize>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct FullChunkDiffReport {
        total_blocks: usize,
        matched_blocks: usize,
        mismatched_blocks: usize,
        top_mismatch_pairs: Vec<MismatchBucket>,
        first_mismatches: Vec<BlockMismatch>,
    }

    impl FullChunkDiffReport {
        fn is_exact(&self) -> bool {
            self.mismatched_blocks == 0
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MismatchBucket {
        actual: String,
        expected: String,
        count: usize,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct BlockMismatch {
        local_x: i32,
        y: i32,
        local_z: i32,
        actual: String,
        expected: String,
    }

    #[derive(Clone, Debug)]
    struct RepeatingPatternBiomeSource {
        width: usize,
        height: usize,
        biomes: Vec<NoiseBiome>,
    }

    impl RepeatingPatternBiomeSource {
        fn new(pattern: &NoiseSamplerBiomePatternFixture) -> Self {
            Self {
                width: pattern.x.len(),
                height: pattern.z.len(),
                biomes: pattern
                    .depths
                    .iter()
                    .zip(&pattern.scales)
                    .map(|(depth, scale)| NoiseBiome::new(*depth, *scale))
                    .collect(),
            }
        }
    }

    impl NoiseBiomeSource for RepeatingPatternBiomeSource {
        fn get_noise_biome(&self, x: i32, _y: i32, z: i32) -> NoiseBiome {
            let wrapped_x = x.rem_euclid(self.width as i32) as usize;
            let wrapped_z = z.rem_euclid(self.height as i32) as usize;
            self.biomes[wrapped_x * self.height + wrapped_z]
        }
    }

    fn fixtures() -> Vec<NoiseSamplerFixture> {
        [
            include_str!("../../../../test/fixtures/noise/noise-sampler-overworld-seed-0.json"),
            include_str!("../../../../test/fixtures/noise/noise-sampler-overworld-seed-1.json"),
            include_str!("../../../../test/fixtures/noise/noise-sampler-overworld-seed-12345.json"),
            include_str!(
                "../../../../test/fixtures/noise/noise-sampler-overworld-seed-2151901553968352745.json"
            ),
        ]
        .into_iter()
        .map(|json| serde_json::from_str(json).expect("valid NoiseSampler fixture"))
        .collect()
    }

    fn terrain_fixture() -> TerrainChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json"
        ))
        .expect("valid terrain chunk oracle fixture")
    }

    fn surface_fixture() -> TerrainChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json"
        ))
        .expect("valid surface chunk oracle fixture")
    }

    fn frozen_ocean_surface_fixture() -> TerrainChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks--247--247-surface-only.json"
        ))
        .expect("valid frozen ocean surface chunk oracle fixture")
    }

    fn badlands_surface_fixture() -> TerrainChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks--320-99-surface-only.json"
        ))
        .expect("valid badlands surface chunk oracle fixture")
    }

    fn full_chunk_fixture() -> FullChunkOracleFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0.json"
        ))
        .expect("valid full chunk oracle fixture")
    }

    fn scheduler_trace_fixture() -> SchedulerTraceFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json"
        ))
        .expect("valid vanilla scheduler trace fixture")
    }

    fn scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
        serde_json::from_str(include_str!(
            "../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-12345-chunk-0-0.json"
        ))
        .expect("valid vanilla scheduler features snapshot fixture")
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TaigaTreeIndexShiftCenterDiagnostic {
        center: ChunkPos,
        biome_key: &'static str,
        current_tree_blocks: usize,
        java_tree_blocks: usize,
        shared_tree_blocks: usize,
        current_only_tree_blocks: usize,
        java_only_tree_blocks: usize,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TaigaFullTableTreeDeltaDiagnostic {
        center: ChunkPos,
        biome_key: &'static str,
        before_tree_blocks: usize,
        after_tree_blocks: usize,
        added_tree_blocks: usize,
        removed_tree_blocks: usize,
        feature_random_calls: usize,
        random_count: usize,
        added_tree_block_samples: Vec<TreeBlockSample>,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TreeBlockSample {
        local_x: i32,
        y: i32,
        local_z: i32,
        block_id: RawBlockId,
    }

    fn chunk_primary_biome_key(biome_source: &OverworldBiomeSource, pos: ChunkPos) -> &'static str {
        biome_source
            .get_primary_biome_definition(pos.x, pos.z)
            .key()
    }

    fn is_taiga_vegetation_biome(key: &str) -> bool {
        matches!(
            key,
            "minecraft:taiga"
                | "minecraft:taiga_hills"
                | "minecraft:taiga_mountains"
                | "minecraft:giant_tree_taiga"
                | "minecraft:giant_tree_taiga_hills"
                | "minecraft:giant_spruce_taiga"
                | "minecraft:giant_spruce_taiga_hills"
        )
    }

    fn target_tree_blocks_after_taiga_vegetation_center(
        seed: i64,
        target: ChunkPos,
        center: ChunkPos,
        feature_index: i32,
    ) -> BTreeMap<(i32, i32, i32), RawBlockId> {
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        let chunks = (center.z - FEATURES_WRITE_RADIUS_CUTOFF
            ..=center.z + FEATURES_WRITE_RADIUS_CUTOFF)
            .flat_map(|chunk_z| {
                let biome_source = biome_source.clone();
                (center.x - FEATURES_WRITE_RADIUS_CUTOFF..=center.x + FEATURES_WRITE_RADIUS_CUTOFF)
                    .map(move |chunk_x| {
                        generate_overworld_liquid_carved_buffer_with_biome_source(
                            seed,
                            chunk_x,
                            chunk_z,
                            biome_source.clone(),
                        )
                    })
            })
            .collect::<Vec<_>>();
        let mut region = FeatureRegion::with_radii(
            center.x,
            center.z,
            FEATURES_WRITE_RADIUS_CUTOFF,
            FEATURES_WRITE_RADIUS_CUTOFF,
            chunks,
        );

        crate::feature::test_support::place_taiga_vegetation_with_feature_index(
            seed,
            &mut region,
            feature_index,
        );
        let chunk = region
            .remove_chunk(target.x, target.z)
            .expect("target chunk should be inside center write window");
        tree_blocks_in_chunk(&chunk)
    }

    fn tree_blocks_in_chunk(
        chunk: &MutableChunkBlockBuffer,
    ) -> BTreeMap<(i32, i32, i32), RawBlockId> {
        let mut blocks = BTreeMap::new();
        for y in chunk.min_y..chunk.min_y + chunk.height {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    let block_id = chunk.get_block_at_y(x, y, z);
                    if matches!(
                        block_id,
                        crate::block::SPRUCE_LOG | crate::block::SPRUCE_LEAVES
                    ) {
                        blocks.insert((x, y, z), block_id);
                    }
                }
            }
        }
        blocks
    }

    fn tree_blocks_in_target_region(
        region: &FeatureRegion,
        target: ChunkPos,
    ) -> BTreeMap<(i32, i32, i32), RawBlockId> {
        tree_blocks_in_chunk(
            region
                .chunk(target.x, target.z)
                .expect("target chunk should be inside feature region"),
        )
    }

    fn tree_block_delta(
        before: &BTreeMap<(i32, i32, i32), RawBlockId>,
        after: &BTreeMap<(i32, i32, i32), RawBlockId>,
    ) -> (usize, usize) {
        let added = after
            .iter()
            .filter(|(pos, block_id)| before.get(pos) != Some(block_id))
            .count();
        let removed = before
            .iter()
            .filter(|(pos, block_id)| after.get(pos) != Some(block_id))
            .count();
        (added, removed)
    }

    fn added_tree_block_samples(
        before: &BTreeMap<(i32, i32, i32), RawBlockId>,
        after: &BTreeMap<(i32, i32, i32), RawBlockId>,
        limit: usize,
    ) -> Vec<TreeBlockSample> {
        let added = after
            .iter()
            .filter(|(pos, block_id)| before.get(pos) != Some(block_id))
            .collect::<Vec<_>>();
        if added.len() > limit {
            return Vec::new();
        }

        added
            .into_iter()
            .map(|(&(local_x, y, local_z), &block_id)| TreeBlockSample {
                local_x,
                y,
                local_z,
                block_id,
            })
            .collect()
    }

    fn taiga_tree_index_shift_diagnostics(
        seed: i64,
        target: ChunkPos,
    ) -> Vec<TaigaTreeIndexShiftCenterDiagnostic> {
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        FeatureBatchPlan::new([target])
            .ordered_feature_centers()
            .into_iter()
            .filter_map(|center| {
                let biome_key = chunk_primary_biome_key(&biome_source, center);
                if !is_taiga_vegetation_biome(biome_key) {
                    return None;
                }

                let current = target_tree_blocks_after_taiga_vegetation_center(
                    seed,
                    target,
                    center,
                    crate::feature::test_support::CURRENT_TAIGA_VEGETATION_FEATURE_INDEX,
                );
                let java = target_tree_blocks_after_taiga_vegetation_center(
                    seed,
                    target,
                    center,
                    crate::feature::test_support::JAVA_TAIGA_VEGETATION_FEATURE_INDEX,
                );
                let shared_tree_blocks = current
                    .iter()
                    .filter(|(pos, block_id)| java.get(pos) == Some(block_id))
                    .count();
                Some(TaigaTreeIndexShiftCenterDiagnostic {
                    center,
                    biome_key,
                    current_tree_blocks: current.len(),
                    java_tree_blocks: java.len(),
                    shared_tree_blocks,
                    current_only_tree_blocks: current.len() - shared_tree_blocks,
                    java_only_tree_blocks: java.len() - shared_tree_blocks,
                })
            })
            .collect()
    }

    fn taiga_full_table_tree_delta_diagnostics(
        seed: i64,
        target: ChunkPos,
    ) -> Vec<TaigaFullTableTreeDeltaDiagnostic> {
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        let plan = FeatureBatchPlan::new([target]);
        let chunks = sorted_chunk_positions_z_major(plan.dependency_chunks.iter().copied())
            .into_iter()
            .map(|pos| {
                generate_overworld_liquid_carved_buffer_with_biome_source(
                    seed,
                    pos.x,
                    pos.z,
                    biome_source.clone(),
                )
            })
            .collect::<Vec<_>>();
        let first_target = *plan.targets.iter().next().expect("non-empty target plan");
        let mut region = FeatureRegion::new(first_target.x, first_target.z, chunks);
        let feature_biomes =
            crate::feature::OverworldFeatureBiomeResolver::new(seed, &biome_source);
        let mut diagnostics = Vec::new();

        for center in plan.ordered_feature_centers() {
            region.set_center(center.x, center.z);
            let biome = biome_source.get_primary_biome_definition(center.x, center.z);
            let biome_key = biome.key();
            let min_block_x = chunk_min_block_coord(center.x);
            let min_block_z = chunk_min_block_coord(center.z);
            let origin = crate::placement::BlockPos::new(min_block_x, region.min_y(), min_block_z);
            let features = crate::feature::overworld_features_for_biome(biome);
            let mut random = WorldgenRandom::default();
            let decoration_seed = random.set_decoration_seed(seed, min_block_x, min_block_z);

            for step_index in 0..=crate::feature::DecorationStep::TopLayerModification.index() {
                let mut feature_index = 0;
                for feature in features
                    .iter()
                    .filter(|feature| feature.step.index() == step_index)
                {
                    random.set_feature_seed(decoration_seed, feature_index, step_index);
                    let capture = is_taiga_vegetation_biome(biome_key)
                        && step_index == crate::feature::DecorationStep::VegetalDecoration.index()
                        && feature_index
                            == crate::feature::test_support::JAVA_TAIGA_VEGETATION_FEATURE_INDEX;
                    let random_count_before_feature = random.get_count();
                    let before = capture.then(|| tree_blocks_in_target_region(&region, target));
                    feature.place_with_biomes(&mut region, &feature_biomes, &mut random, origin);
                    if let Some(before) = before {
                        let after = tree_blocks_in_target_region(&region, target);
                        let (added_tree_blocks, removed_tree_blocks) =
                            tree_block_delta(&before, &after);
                        let random_count = random.get_count();
                        diagnostics.push(TaigaFullTableTreeDeltaDiagnostic {
                            center,
                            biome_key,
                            before_tree_blocks: before.len(),
                            after_tree_blocks: after.len(),
                            added_tree_blocks,
                            removed_tree_blocks,
                            feature_random_calls: random_count - random_count_before_feature,
                            random_count,
                            added_tree_block_samples: added_tree_block_samples(&before, &after, 20),
                        });
                    }
                    feature_index += 1;
                }
            }
        }

        diagnostics
    }

    fn create_noise_settings(fixture: &NoiseSamplerFixture) -> NoiseSettings {
        let settings = &fixture.noise_settings;
        NoiseSettings::create(
            settings.min_y,
            settings.height,
            NoiseSamplingSettings::new(
                settings.sampling.xz_scale,
                settings.sampling.y_scale,
                settings.sampling.xz_factor,
                settings.sampling.y_factor,
            ),
            NoiseSlideSettings::new(
                settings.top_slide.target,
                settings.top_slide.size,
                settings.top_slide.offset,
            ),
            NoiseSlideSettings::new(
                settings.bottom_slide.target,
                settings.bottom_slide.size,
                settings.bottom_slide.offset,
            ),
            settings.noise_size_horizontal,
            settings.noise_size_vertical,
            settings.density_factor,
            settings.density_offset,
            settings.use_simplex_surface_noise,
            settings.random_density_offset,
            settings.island_noise_override,
            settings.is_amplified,
        )
    }

    fn create_noise_sampler(
        fixture: &NoiseSamplerFixture,
        pattern: &NoiseSamplerBiomePatternFixture,
    ) -> NoiseSampler<RepeatingPatternBiomeSource> {
        let settings = create_noise_settings(fixture);
        let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
        let mut random = WorldgenRandom::new(seed);
        let blended_noise = BlendedNoise::new(&mut random);
        random.consume_count(2620);
        let depth_noise = PerlinNoise::from_octaves(&mut random, &DEPTH_NOISE_OCTAVES);

        NoiseSampler::new(
            RepeatingPatternBiomeSource::new(pattern),
            fixture.cell_width,
            fixture.cell_height,
            fixture.cell_count_y,
            settings,
            blended_noise,
            None,
            depth_noise,
            NoiseModifier::Passthrough,
        )
    }

    fn terrain_stage_blocks_from_oracle(oracle: &TerrainChunkOracleFixture) -> Vec<u8> {
        oracle
            .blocks
            .iter()
            .map(|block_id| {
                if *block_id == BEDROCK {
                    STONE
                } else {
                    *block_id
                }
            })
            .collect()
    }

    fn assert_chunk_blocks_match(actual: &[u8], expected: &[u8], min_y: i32) {
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual,
                expected,
                "chunk block mismatch at local ({}, {}, {}): expected block id {}, got {}",
                index & 15,
                (index >> 8) as i32 + min_y,
                (index >> 4) & 15,
                expected,
                actual
            );
        }
    }

    fn compare_generated_chunk_to_full_fixture(
        actual: &GeneratedChunk,
        expected: &FullChunkEntryFixture,
    ) -> FullChunkDiffReport {
        let expected_blocks = expand_full_fixture_blocks(expected, actual.min_y, actual.height);
        let actual_blocks = actual
            .blocks()
            .iter()
            .map(|block_id| crate::block::block_name(*block_id).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(actual_blocks.len(), expected_blocks.len());

        let mut mismatch_counts = BTreeMap::<(String, String), usize>::new();
        let mut first_mismatches = Vec::new();
        let mut matched_blocks = 0;

        for (index, (actual_name, expected_name)) in
            actual_blocks.iter().zip(expected_blocks.iter()).enumerate()
        {
            if actual_name == expected_name {
                matched_blocks += 1;
                continue;
            }

            *mismatch_counts
                .entry((actual_name.clone(), expected_name.clone()))
                .or_default() += 1;
            if first_mismatches.len() < 12 {
                first_mismatches.push(block_mismatch_at_index(
                    index,
                    actual.min_y,
                    actual_name.clone(),
                    expected_name.clone(),
                ));
            }
        }

        let mut top_mismatch_pairs = mismatch_counts
            .into_iter()
            .map(|((actual, expected), count)| MismatchBucket {
                actual,
                expected,
                count,
            })
            .collect::<Vec<_>>();
        top_mismatch_pairs.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.actual.cmp(&right.actual))
                .then_with(|| left.expected.cmp(&right.expected))
        });
        top_mismatch_pairs.truncate(16);

        FullChunkDiffReport {
            total_blocks: actual_blocks.len(),
            matched_blocks,
            mismatched_blocks: actual_blocks.len() - matched_blocks,
            top_mismatch_pairs,
            first_mismatches,
        }
    }

    fn expand_full_fixture_blocks(
        fixture: &FullChunkEntryFixture,
        min_y: i32,
        height: i32,
    ) -> Vec<String> {
        let mut blocks = vec!["minecraft:air".to_owned(); height as usize * 16 * 16];

        for section in &fixture.sections {
            assert_eq!(section.block_order, "y-major,z-major,x-minor");
            assert_eq!(section.blocks.len(), 16 * 16 * 16);
            for (section_index, palette_index) in section.blocks.iter().copied().enumerate() {
                let local_y_in_section = section_index as i32 / 256;
                let within_layer = section_index as i32 % 256;
                let local_x = within_layer & 15;
                let local_z = (within_layer >> 4) & 15;
                let y = section.y * 16 + local_y_in_section;
                if !(min_y..min_y + height).contains(&y) {
                    continue;
                }
                let block_name = section
                    .palette
                    .get(palette_index)
                    .unwrap_or_else(|| {
                        panic!("palette index {palette_index} outside section palette")
                    })
                    .clone();
                let index = chunk_block_index(local_x, y - min_y, local_z);
                blocks[index] = block_name;
            }
        }

        blocks
    }

    fn block_mismatch_at_index(
        index: usize,
        min_y: i32,
        actual: String,
        expected: String,
    ) -> BlockMismatch {
        BlockMismatch {
            local_x: (index & 15) as i32,
            y: ((index >> 8) as i32) + min_y,
            local_z: ((index >> 4) & 15) as i32,
            actual,
            expected,
        }
    }

    fn assert_surface_chunk_matches_java_oracle(oracle: TerrainChunkOracleFixture) {
        assert_eq!(oracle.module, "surface-chunk");
        assert_eq!(oracle.minecraft_version, "1.17.1");
        assert_eq!(
            oracle.generator_class,
            "net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator"
        );
        assert_eq!(oracle.seed, "12345");
        assert_eq!(oracle.min_y, 0);
        assert_eq!(oracle.height, 256);
        assert_eq!(oracle.block_order, "y-major,z-major,x-minor");

        let seed = oracle.seed.parse::<i64>().expect("i64 fixture seed");
        let generator = NoiseBasedChunkGenerator::new(
            OverworldBiomeSource::new(seed, false, false),
            seed,
            NoiseGeneratorSettings::overworld(),
        );
        let mut chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
        generator.build_surface_and_bedrock(&mut chunk);

        assert_eq!(chunk.chunk_x, oracle.chunk_x);
        assert_eq!(chunk.chunk_z, oracle.chunk_z);
        assert_eq!(chunk.min_y, oracle.min_y);
        assert_eq!(chunk.height, oracle.height);
        assert_chunk_blocks_match(&chunk.blocks, &oracle.blocks, oracle.min_y);
    }

    #[test]
    fn fixture_metadata_stays_consistent() {
        for fixture in fixtures() {
            assert_eq!(fixture.module, "noise");
            assert_eq!(fixture.minecraft_version, "1.17.1");
            assert_eq!(
                fixture.noise_class,
                "net.minecraft.world.level.levelgen.NoiseSampler"
            );
            assert_eq!(
                fixture.random_source_class,
                "net.minecraft.world.level.levelgen.WorldgenRandom"
            );
            assert_eq!(fixture.settings_preset, "overworld");
            assert_eq!(fixture.noise_modifier, "PASSTHROUGH");
            assert_eq!(fixture.cell_width, 4);
            assert_eq!(fixture.cell_height, 8);
            assert_eq!(fixture.cell_count_y, 32);
            assert_eq!(fixture.biome_y, 63);
            assert_eq!(fixture.min_cell_y, 0);
            assert_eq!(fixture.column_value_count, 33);
            assert_eq!(fixture.wire_format.coordinates, "integer");
            assert_eq!(fixture.wire_format.biome_keys, "string");
            assert_eq!(fixture.wire_format.biome_factors, "number");
            assert_eq!(fixture.wire_format.values, "number");
            assert!(fixture.noise_settings.use_simplex_surface_noise);
            assert!(fixture.noise_settings.random_density_offset);
            assert!(!fixture.noise_settings.island_noise_override);
            assert!(!fixture.noise_settings.is_amplified);

            for (name, sample_set) in &fixture.sample_sets {
                assert!(
                    name == "constantPlains" || name == "mixedOverworld",
                    "unexpected sample set {name}"
                );
                assert_eq!(sample_set.biome_pattern.grid_order, "x-major,z-minor");
                assert_eq!(
                    sample_set.biome_pattern.sample_count,
                    sample_set.biome_pattern.x.len() * sample_set.biome_pattern.z.len()
                );
                assert_eq!(
                    sample_set.biome_pattern.keys.len(),
                    sample_set.biome_pattern.sample_count
                );
                assert_eq!(
                    sample_set.biome_pattern.depths.len(),
                    sample_set.biome_pattern.sample_count
                );
                assert_eq!(
                    sample_set.biome_pattern.scales.len(),
                    sample_set.biome_pattern.sample_count
                );
                assert_eq!(
                    sample_set.columns.noise_method,
                    "fillNoiseColumn(noiseValues,cellX,cellZ,noiseSettings,biomeY,minCellY,cellCountY)"
                );
                assert_eq!(sample_set.columns.grid_order, "x-major,z-minor,y-minor");
                assert_eq!(
                    sample_set.columns.column_value_count,
                    fixture.column_value_count
                );
                assert_eq!(
                    sample_set.columns.sample_count,
                    sample_set.columns.x.len()
                        * sample_set.columns.z.len()
                        * sample_set.columns.column_value_count
                );
                assert_eq!(
                    sample_set.columns.values.len(),
                    sample_set.columns.sample_count
                );
            }
        }
    }

    #[test]
    fn default_overworld_settings_keep_dormant_caves_and_cliffs_flags_disabled() {
        let settings = NoiseGeneratorSettings::overworld();
        assert_eq!(settings.default_block(), STONE);
        assert_eq!(settings.default_fluid(), WATER);
        assert_eq!(settings.bedrock_floor_position(), 0);
        assert_eq!(settings.sea_level(), 63);
        assert!(!settings.is_aquifers_enabled());
        assert!(!settings.is_noise_caves_enabled());
        assert!(!settings.is_deepslate_enabled());
        assert!(!settings.is_ore_veins_enabled());
        assert!(!settings.is_noodle_caves_enabled());
    }

    #[test]
    fn terrain_only_oracle_fixture_stays_pinned_to_generator_target() {
        let oracle = terrain_fixture();
        assert_eq!(oracle.module, "terrain-chunk");
        assert_eq!(oracle.minecraft_version, "1.17.1");
        assert_eq!(
            oracle.generator_class,
            "net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator"
        );
        assert_eq!(oracle.seed, "12345");
        assert_eq!(oracle.chunk_x, 0);
        assert_eq!(oracle.chunk_z, 0);
        assert_eq!(oracle.min_y, 0);
        assert_eq!(oracle.height, 256);
        assert_eq!(oracle.block_order, "y-major,z-major,x-minor");
        assert_eq!(
            oracle.palette,
            [
                "minecraft:air",
                "minecraft:stone",
                "minecraft:water",
                "minecraft:bedrock",
            ]
        );
        assert_eq!(
            oracle.blocks.len(),
            CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 256
        );
    }

    #[test]
    fn fills_chunk_zero_zero_with_terrain_only_java_oracle() {
        let oracle = terrain_fixture();
        let generator = NoiseBasedChunkGenerator::new(
            OverworldBiomeSource::new(
                oracle.seed.parse::<i64>().expect("i64 fixture seed"),
                false,
                false,
            ),
            oracle.seed.parse::<i64>().expect("i64 fixture seed"),
            NoiseGeneratorSettings::overworld(),
        );
        let chunk = generator.fill_from_noise(oracle.chunk_x, oracle.chunk_z);
        let expected = terrain_stage_blocks_from_oracle(&oracle);

        assert_eq!(chunk.chunk_x, oracle.chunk_x);
        assert_eq!(chunk.chunk_z, oracle.chunk_z);
        assert_eq!(chunk.min_y, oracle.min_y);
        assert_eq!(chunk.height, oracle.height);
        assert!(!chunk.blocks.contains(&BEDROCK));
        assert_eq!(chunk.blocks.len(), expected.len());

        for (index, (actual, expected)) in chunk.blocks.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual,
                expected,
                "terrain mismatch at local ({}, {}, {}): expected block id {}, got {}",
                index & 15,
                index >> 8,
                (index >> 4) & 15,
                expected,
                actual
            );
        }
    }

    #[test]
    fn build_surface_and_bedrock_matches_java_oracle() {
        let oracle = surface_fixture();
        assert_eq!(oracle.chunk_x, 0);
        assert_eq!(oracle.chunk_z, 0);
        assert_surface_chunk_matches_java_oracle(oracle);
    }

    #[test]
    fn build_frozen_ocean_surface_and_bedrock_matches_java_oracle() {
        let oracle = frozen_ocean_surface_fixture();
        assert_eq!(oracle.chunk_x, -247);
        assert_eq!(oracle.chunk_z, -247);
        assert_surface_chunk_matches_java_oracle(oracle);
    }

    #[test]
    fn build_badlands_surface_and_bedrock_matches_java_oracle() {
        let oracle = badlands_surface_fixture();
        assert_eq!(oracle.chunk_x, -320);
        assert_eq!(oracle.chunk_z, 99);
        assert_surface_chunk_matches_java_oracle(oracle);
    }

    #[test]
    fn generated_chunk_wraps_surface_buffer_for_render_consumers() {
        let chunk = generate_overworld_surface_chunk(12345, 0, 0);

        assert_eq!(chunk.chunk_x, 0);
        assert_eq!(chunk.chunk_z, 0);
        assert_eq!(chunk.min_y, 0);
        assert_eq!(chunk.height, 256);
        assert_eq!(
            chunk.blocks().len(),
            CHUNK_WIDTH as usize * CHUNK_WIDTH as usize * 256
        );
        assert!(chunk.non_air_block_count() > 0);
        assert_eq!(chunk.block_at_local(0, 0, 0).name(), "minecraft:bedrock");
    }

    #[test]
    fn generated_features_chunk_adds_visible_decoration_blocks() {
        let features = generate_overworld_features_chunk(12345, 0, 0);
        let feature_block_count = features.block_count(crate::block::OAK_LOG)
            + features.block_count(crate::block::OAK_LEAVES)
            + features.block_count(crate::block::BIRCH_LOG)
            + features.block_count(crate::block::BIRCH_LEAVES)
            + features.block_count(crate::block::SPRUCE_LOG)
            + features.block_count(crate::block::SPRUCE_LEAVES)
            + features.block_count(crate::block::GRASS)
            + features.block_count(crate::block::FERN)
            + features.block_count(crate::block::DANDELION)
            + features.block_count(crate::block::POPPY)
            + features.block_count(crate::block::DEAD_BUSH);

        assert!(feature_block_count > 0);
        assert!(
            features.block_count(crate::block::GRANITE)
                + features.block_count(crate::block::DIORITE)
                + features.block_count(crate::block::ANDESITE)
                + features.block_count(crate::block::TUFF)
                + features.block_count(crate::block::DEEPSLATE)
                > 0
        );
        assert!(
            features.block_count(crate::block::COAL_ORE)
                + features.block_count(crate::block::DEEPSLATE_COAL_ORE)
                + features.block_count(crate::block::IRON_ORE)
                + features.block_count(crate::block::DEEPSLATE_IRON_ORE)
                + features.block_count(crate::block::COPPER_ORE)
                + features.block_count(crate::block::DEEPSLATE_COPPER_ORE)
                > 0
        );
        assert!(features.block_count(crate::block::LAVA) > 0);
    }

    #[test]
    fn feature_batch_plan_reuses_overlapping_dependency_windows() {
        let single = FeatureBatchPlan::new([ChunkPos::new(0, 0)]);
        assert_eq!(single.targets.len(), 1);
        assert_eq!(single.feature_centers.len(), 3 * 3);
        assert_eq!(single.dependency_chunks.len(), 19 * 19);

        let radius_one_targets = (-1..=1).flat_map(|z| (-1..=1).map(move |x| ChunkPos::new(x, z)));
        let radius_one = FeatureBatchPlan::new(radius_one_targets);

        assert_eq!(radius_one.targets.len(), 3 * 3);
        assert_eq!(radius_one.feature_centers.len(), 5 * 5);
        assert_eq!(radius_one.dependency_chunks.len(), 21 * 21);
        assert!(
            radius_one
                .dependency_chunks
                .contains(&ChunkPos::new(-10, -10))
        );
        assert!(
            radius_one
                .dependency_chunks
                .contains(&ChunkPos::new(10, 10))
        );
    }

    #[test]
    fn feature_center_order_matches_vanilla_scheduler_trace_for_spawn_bootstrap() {
        let trace = scheduler_trace_fixture();
        assert_eq!(trace.module, "scheduler-trace");
        assert_eq!(trace.minecraft_version, "1.17.1");
        assert_eq!(trace.seed, "12345");
        assert_eq!(trace.target_radius, FEATURES_WRITE_RADIUS_CUTOFF);
        assert_eq!(trace.stop_status, "FEATURES");

        let plan =
            FeatureBatchPlan::new([ChunkPos::new(trace.target_chunk_x, trace.target_chunk_z)]);
        let expected = trace
            .feature_completion_order_3x3
            .into_iter()
            .map(|entry| ChunkPos::new(entry.chunk_x, entry.chunk_z))
            .collect::<Vec<_>>();

        assert_eq!(plan.ordered_feature_centers(), expected);
    }

    #[test]
    fn taiga_vegetation_feature_index_shift_is_isolated_by_center() {
        let diagnostics = taiga_tree_index_shift_diagnostics(12_345, ChunkPos::new(0, 0));

        assert_eq!(
            diagnostics,
            vec![
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(-1, -1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 0,
                    java_tree_blocks: 0,
                    shared_tree_blocks: 0,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(0, -1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 9,
                    java_tree_blocks: 9,
                    shared_tree_blocks: 9,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(1, -1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 0,
                    java_tree_blocks: 0,
                    shared_tree_blocks: 0,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(-1, 0),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 0,
                    java_tree_blocks: 0,
                    shared_tree_blocks: 0,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(0, 0),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 236,
                    java_tree_blocks: 236,
                    shared_tree_blocks: 236,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(1, 0),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 12,
                    java_tree_blocks: 12,
                    shared_tree_blocks: 12,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(0, 1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 14,
                    java_tree_blocks: 14,
                    shared_tree_blocks: 14,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
                TaigaTreeIndexShiftCenterDiagnostic {
                    center: ChunkPos::new(1, 1),
                    biome_key: "minecraft:taiga_mountains",
                    current_tree_blocks: 0,
                    java_tree_blocks: 0,
                    shared_tree_blocks: 0,
                    current_only_tree_blocks: 0,
                    java_only_tree_blocks: 0,
                },
            ]
        );
    }

    #[test]
    fn taiga_full_table_tree_deltas_match_vanilla_scheduler_probe() {
        let diagnostics = taiga_full_table_tree_delta_diagnostics(12_345, ChunkPos::new(0, 0));
        let tree_deltas = diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.center,
                    diagnostic.biome_key,
                    diagnostic.before_tree_blocks,
                    diagnostic.after_tree_blocks,
                    diagnostic.added_tree_blocks,
                    diagnostic.removed_tree_blocks,
                    diagnostic.feature_random_calls,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            tree_deltas,
            vec![
                (
                    ChunkPos::new(-1, -1),
                    "minecraft:taiga_mountains",
                    0,
                    0,
                    0,
                    0,
                    75
                ),
                (
                    ChunkPos::new(0, -1),
                    "minecraft:taiga_mountains",
                    0,
                    0,
                    0,
                    0,
                    68
                ),
                (
                    ChunkPos::new(1, -1),
                    "minecraft:taiga_mountains",
                    0,
                    0,
                    0,
                    0,
                    75
                ),
                (
                    ChunkPos::new(-1, 0),
                    "minecraft:taiga_mountains",
                    0,
                    0,
                    0,
                    0,
                    77
                ),
                (
                    ChunkPos::new(0, 0),
                    "minecraft:taiga_mountains",
                    0,
                    236,
                    236,
                    0,
                    77
                ),
                (
                    ChunkPos::new(1, 0),
                    "minecraft:taiga_mountains",
                    236,
                    246,
                    10,
                    0,
                    86
                ),
                (
                    ChunkPos::new(0, 1),
                    "minecraft:taiga_mountains",
                    246,
                    246,
                    0,
                    0,
                    73
                ),
                (
                    ChunkPos::new(1, 1),
                    "minecraft:taiga_mountains",
                    246,
                    246,
                    0,
                    0,
                    73
                ),
            ]
        );
    }

    #[test]
    fn overworld_feature_dependency_cache_reuses_overlapping_windows() {
        let mut cache = OverworldFeatureDependencyCache::new();

        let first = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
        assert_eq!(
            first.cache_report,
            OverworldFeatureDependencyCacheReport {
                requested_dependency_chunks: 19 * 19,
                cache_hits: 0,
                generated_dependency_chunks: 19 * 19,
                retained_dependency_chunks: 19 * 19,
            }
        );

        let second = cache.generate_features_chunks(12_345, [ChunkPos::new(1, 0)]);
        assert_eq!(
            second.cache_report,
            OverworldFeatureDependencyCacheReport {
                requested_dependency_chunks: 19 * 19,
                cache_hits: 18 * 19,
                generated_dependency_chunks: 19,
                retained_dependency_chunks: 19 * 19,
            }
        );
        assert_eq!(cache.retained_chunk_count(), 19 * 19);
        assert_eq!(
            second.chunks.get(&ChunkPos::new(1, 0)),
            Some(&generate_overworld_features_chunk(12_345, 1, 0))
        );
    }

    #[test]
    fn overworld_feature_dependency_cache_keeps_clean_lower_status_chunks() {
        let mut cache = OverworldFeatureDependencyCache::new();

        let first = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
        let second = cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);

        assert_eq!(
            second.cache_report,
            OverworldFeatureDependencyCacheReport {
                requested_dependency_chunks: 19 * 19,
                cache_hits: 19 * 19,
                generated_dependency_chunks: 0,
                retained_dependency_chunks: 19 * 19,
            }
        );
        assert_eq!(
            second.chunks.get(&ChunkPos::new(0, 0)),
            first.chunks.get(&ChunkPos::new(0, 0))
        );
    }

    #[test]
    fn overworld_feature_dependency_cache_resets_when_seed_changes() {
        let mut cache = OverworldFeatureDependencyCache::new();

        cache.generate_features_chunks(12_345, [ChunkPos::new(0, 0)]);
        let changed_seed = cache.generate_features_chunks(54_321, [ChunkPos::new(0, 0)]);

        assert_eq!(
            changed_seed.cache_report,
            OverworldFeatureDependencyCacheReport {
                requested_dependency_chunks: 19 * 19,
                cache_hits: 0,
                generated_dependency_chunks: 19 * 19,
                retained_dependency_chunks: 19 * 19,
            }
        );
    }

    #[test]
    fn full_decorated_chunk_gauntlet_reports_current_native_gap() {
        let fixture = full_chunk_fixture();
        assert_eq!(fixture.module, "integration");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(fixture.seed, "12345");
        assert_eq!(fixture.generator, "default");
        assert!(!fixture.generate_structures);
        assert_eq!(fixture.wire_format.block_order, "y-major,z-major,x-minor");
        assert_eq!(fixture.wire_format.palette_entries, "resource-key");
        assert_eq!(fixture.chunks.len(), 1);

        let expected = &fixture.chunks[0];
        assert_eq!(expected.chunk_x, 0);
        assert_eq!(expected.chunk_z, 0);
        assert_eq!(expected.status, "full");
        let actual = generate_overworld_features_chunk(12_345, expected.chunk_x, expected.chunk_z);
        let report = compare_generated_chunk_to_full_fixture(&actual, expected);

        assert_eq!(report.total_blocks, 16 * 16 * 256);
        assert!(report.matched_blocks < report.total_blocks);
        assert_eq!(report.mismatched_blocks, 5, "{report:#?}");
        assert_eq!(
            report.top_mismatch_pairs,
            vec![
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:water".to_owned(),
                    count: 2,
                },
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:glow_lichen".to_owned(),
                    count: 1,
                },
                MismatchBucket {
                    actual: "minecraft:air".to_owned(),
                    expected: "minecraft:lava".to_owned(),
                    count: 1,
                },
                MismatchBucket {
                    actual: "minecraft:glow_lichen".to_owned(),
                    expected: "minecraft:air".to_owned(),
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn features_status_chunk_snapshot_excludes_runtime_liquid_tick_results() {
        let fixture = scheduler_features_snapshot_fixture();
        assert_eq!(fixture.module, "scheduler-trace");
        assert_eq!(fixture.minecraft_version, "1.17.1");
        assert_eq!(fixture.seed, "12345");
        assert_eq!(fixture.target_chunk_x, 0);
        assert_eq!(fixture.target_chunk_z, 0);
        assert_eq!(fixture.target_radius, 1);
        assert_eq!(fixture.stop_status, "FEATURES");
        assert_eq!(fixture.chunks.len(), 1);

        let expected = &fixture.chunks[0];
        assert_eq!(expected.chunk_x, 0);
        assert_eq!(expected.chunk_z, 0);
        assert_eq!(expected.status, "features");

        let actual = generate_overworld_features_chunk(
            fixture.seed.parse::<i64>().expect("fixture seed is i64"),
            expected.chunk_x,
            expected.chunk_z,
        );
        let expected_blocks = expand_full_fixture_blocks(expected, actual.min_y, actual.height);
        for (local_x, y, local_z, expected_name) in [
            (9, 12, 15, "minecraft:air"),
            (10, 17, 2, "minecraft:air"),
            (7, 17, 10, "minecraft:glow_lichen"),
            (9, 18, 2, "minecraft:water"),
            (10, 18, 2, "minecraft:air"),
        ] {
            let index = ((y - actual.min_y) << 8) | (local_z << 4) | local_x;
            assert_eq!(expected_blocks[index as usize], expected_name);
            assert_eq!(actual.block_at_y(local_x, y, local_z).name(), expected_name);
        }

        let report = compare_generated_chunk_to_full_fixture(&actual, expected);
        assert!(report.is_exact(), "{report:#?}");
    }

    #[test]
    #[ignore = "active gauntlet: native full decorated chunk parity is not expected to pass yet"]
    fn full_decorated_chunk_zero_zero_matches_java_oracle() {
        let fixture = full_chunk_fixture();
        let expected = &fixture.chunks[0];
        let actual = generate_overworld_features_chunk(
            fixture.seed.parse::<i64>().expect("fixture seed is i64"),
            expected.chunk_x,
            expected.chunk_z,
        );
        let report = compare_generated_chunk_to_full_fixture(&actual, expected);

        assert!(report.is_exact(), "{report:#?}");
    }

    #[test]
    fn matches_java_oracle_across_sampled_cell_columns() {
        for fixture in fixtures() {
            let settings = create_noise_settings(&fixture);

            for (sample_set_name, sample_set) in &fixture.sample_sets {
                let sampler = create_noise_sampler(&fixture, &sample_set.biome_pattern);
                let mut column = vec![0.0; fixture.column_value_count];
                let mut index = 0;

                for cell_x in &sample_set.columns.x {
                    for cell_z in &sample_set.columns.z {
                        sampler.fill_noise_column(
                            &mut column,
                            *cell_x,
                            *cell_z,
                            &settings,
                            fixture.biome_y,
                            fixture.min_cell_y,
                            fixture.cell_count_y,
                        );

                        for (y_index, actual) in column.iter().enumerate() {
                            let expected = sample_set.columns.values[index];
                            assert_eq!(
                                actual.to_bits(),
                                expected.to_bits(),
                                "noise-sampler(seed={}, sampleSet={sample_set_name}) mismatch at flattened index {index} for cell ({cell_x}, {cell_z}) and column index {y_index}: expected {expected}, got {actual}",
                                fixture.seed
                            );
                            index += 1;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn noise_settings_create_enforces_java_min_y_height_guard() {
        let sampling = NoiseSamplingSettings::new(1.0, 1.0, 80.0, 160.0);
        let top_slide = NoiseSlideSettings::new(-10, 3, 0);
        let bottom_slide = NoiseSlideSettings::new(15, 3, 0);

        assert_panic_message(
            || {
                NoiseSettings::create(
                    0,
                    255,
                    sampling,
                    top_slide,
                    bottom_slide,
                    1,
                    2,
                    1.0,
                    -0.46875,
                    true,
                    true,
                    false,
                    false,
                );
            },
            "height has to be a multiple of 16",
        );
        assert_panic_message(
            || {
                NoiseSettings::create(
                    1,
                    256,
                    sampling,
                    top_slide,
                    bottom_slide,
                    1,
                    2,
                    1.0,
                    -0.46875,
                    true,
                    true,
                    false,
                    false,
                );
            },
            "min_y has to be a multiple of 16",
        );
        assert_panic_message(
            || {
                NoiseSettings::create(
                    0,
                    2048 + 16,
                    sampling,
                    top_slide,
                    bottom_slide,
                    1,
                    2,
                    1.0,
                    -0.46875,
                    true,
                    true,
                    false,
                    false,
                );
            },
            "min_y + height cannot be higher than: 2032",
        );
    }

    #[test]
    fn island_noise_override_is_rejected_in_this_overworld_slice() {
        let fixture = fixtures()
            .into_iter()
            .find(|fixture| fixture.seed == "12345")
            .expect("seed 12345 fixture");
        let sample_set = fixture
            .sample_sets
            .get("constantPlains")
            .expect("constantPlains sample set");
        let settings = create_noise_settings(&fixture);
        let mut random = WorldgenRandom::new(12_345);
        let blended_noise = BlendedNoise::new(&mut random);
        random.consume_count(2620);
        let depth_noise = PerlinNoise::from_octaves(&mut random, &DEPTH_NOISE_OCTAVES);
        let island_noise = SimplexNoise::new(&mut WorldgenRandom::new(12_345));
        let sampler = NoiseSampler::new(
            RepeatingPatternBiomeSource::new(&sample_set.biome_pattern),
            fixture.cell_width,
            fixture.cell_height,
            fixture.cell_count_y,
            settings.clone(),
            blended_noise,
            Some(island_noise),
            depth_noise,
            NoiseModifier::Passthrough,
        );

        assert_panic_message(
            || {
                let mut column = vec![0.0; fixture.column_value_count];
                sampler.fill_noise_column(
                    &mut column,
                    0,
                    0,
                    &settings,
                    fixture.biome_y,
                    fixture.min_cell_y,
                    fixture.cell_count_y,
                );
            },
            "NoiseSampler island noise override is out of scope for the 1.17.1 overworld target",
        );
    }

    #[test]
    fn generated_chunk_preserves_scheduled_ticks_from_mutable_buffer() {
        let mut buffer = MutableChunkBlockBuffer::new(2, -3, 0, 32);
        buffer.schedule_block_tick(33, 8, -47, "minecraft:stone", 2);
        buffer.schedule_liquid_tick(34, 9, -46, "minecraft:water", 0);

        let chunk = GeneratedChunk::from_mutable_buffer(buffer);

        assert_eq!(
            chunk.block_ticks(),
            &[ScheduledTick::new(33, 8, -47, "minecraft:stone", 2)]
        );
        assert_eq!(
            chunk.liquid_ticks(),
            &[ScheduledTick::new(34, 9, -46, "minecraft:water", 0)]
        );
    }

    #[test]
    fn generated_chunk_converts_to_packed_snapshot() {
        let mut blocks = vec![AIR; 32 * 16 * 16];
        blocks[16 * 16 * 16] = STONE;
        let chunk = GeneratedChunk::from_raw_parts(2, -3, 0, 32, blocks);

        let snapshot = chunk.to_chunk_snapshot(
            mclone_core::ChunkRevision(9),
            mclone_core::ChunkStatus::Surface,
        );

        assert_eq!(snapshot.pos, mclone_core::ChunkPos::new(2, -3));
        assert_eq!(snapshot.revision, mclone_core::ChunkRevision(9));
        assert_eq!(snapshot.status, mclone_core::ChunkStatus::Surface);
        assert_eq!(snapshot.sections.len(), 1);
        assert_eq!(snapshot.sections[0].section_y, 1);
        assert_eq!(
            snapshot.sections[0].unpack_block_state_ids()[0],
            mclone_core::BlockStateId(STONE as u32)
        );
    }

    fn assert_panic_message(work: impl FnOnce() + panic::UnwindSafe, expected: &str) {
        let panic = panic::catch_unwind(work).expect_err("expected panic");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .expect("panic message");
        assert_eq!(message, expected);
    }
}
