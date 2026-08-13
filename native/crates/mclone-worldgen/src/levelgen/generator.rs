use crate::biome::{NoiseBiome, NoiseBiomeSource, OverworldBiomeSource};
use crate::block::{AIR, BEDROCK, RawBlockId};
use crate::noise::{BlendedNoise, PerlinNoise, PerlinSimplexNoise, SimplexNoise};
use crate::prng::WorldgenRandom;
use crate::surface::apply_overworld_surface;
use mclone_core::{CHUNK_WIDTH, chunk_min_block_coord};

use super::chunk::world_surface_height;
use super::sampler::{BIOME_WEIGHT_RADIUS, BiomeDensity, compute_biome_density_from_neighborhood};
use super::timing::{timing_elapsed_us, timing_start};
use super::{
    MutableChunkBlockBuffer, NoiseGeneratorSettings, NoiseModifier, NoiseSampler, NoiseSettings,
    SurfaceFillTiming,
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
pub(super) struct TimedSurfaceFill {
    pub(super) chunk: MutableChunkBlockBuffer,
    pub(super) timing: SurfaceFillTiming,
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

    pub(super) fn fill_from_noise_timed(&self, chunk_x: i32, chunk_z: i32) -> TimedSurfaceFill {
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

    pub(crate) fn fill_lod_noise_column(&self, cell_x: i32, cell_z: i32, noise_values: &mut [f64]) {
        assert_eq!(
            noise_values.len(),
            (self.cell_count_y + 1) as usize,
            "vanilla LOD density column has the generator's vertical sample count"
        );
        let noise_settings = self.settings.noise_settings();
        let density = self.lod_biome_density(cell_x, cell_z, noise_settings);
        self.sampler.fill_noise_column_with_density(
            noise_values,
            cell_x,
            cell_z,
            noise_settings,
            self.min_cell_y,
            self.cell_count_y,
            density,
        );
    }

    pub(crate) fn fill_lod_sparse_noise_column(
        &self,
        cell_x: i32,
        cell_z: i32,
        vertical_cell_step: i32,
        noise_values: &mut [f64],
    ) {
        assert!(vertical_cell_step > 0);
        assert_eq!(self.cell_count_y.rem_euclid(vertical_cell_step), 0);
        assert_eq!(
            noise_values.len(),
            (self.cell_count_y / vertical_cell_step + 1) as usize,
            "vanilla macro LOD density column has the requested sparse vertical sample count"
        );
        let noise_settings = self.settings.noise_settings();
        let density = self.lod_macro_biome_density(cell_x, cell_z, noise_settings);
        self.sampler.fill_sparse_noise_column_with_density(
            noise_values,
            cell_x,
            cell_z,
            noise_settings,
            self.min_cell_y,
            self.cell_count_y,
            vertical_cell_step,
            density,
        );
    }

    fn lod_biome_density(
        &self,
        cell_x: i32,
        cell_z: i32,
        noise_settings: &NoiseSettings,
    ) -> BiomeDensity {
        let center_depth = self
            .sampler
            .biome_source
            .get_noise_biome(cell_x, self.sea_level, cell_z)
            .get_depth();
        compute_biome_density_from_neighborhood(
            center_depth,
            noise_settings,
            |offset_x, offset_z| {
                self.sampler.biome_source.get_noise_biome(
                    cell_x + offset_x,
                    self.sea_level,
                    cell_z + offset_z,
                )
            },
        )
    }

    fn lod_macro_biome_density(
        &self,
        cell_x: i32,
        cell_z: i32,
        noise_settings: &NoiseSettings,
    ) -> BiomeDensity {
        let biome = self
            .sampler
            .biome_source
            .get_noise_biome(cell_x, self.sea_level, cell_z);
        let biome_depth = biome.get_depth();
        let biome_scale = biome.get_scale();
        let (adjusted_depth, adjusted_scale) = if noise_settings.is_amplified() && biome_depth > 0.0
        {
            (
                1.0_f32 + biome_depth * 2.0_f32,
                1.0_f32 + biome_scale * 4.0_f32,
            )
        } else {
            (biome_depth, biome_scale)
        };
        BiomeDensity {
            depth: f64::from(adjusted_depth * 0.5_f32 - 0.125_f32) * 0.265625,
            scale: 96.0 / f64::from(adjusted_scale * 0.9_f32 + 0.1_f32),
        }
    }

    pub(crate) const fn lod_cell_width(&self) -> i32 {
        self.cell_width
    }

    pub(crate) const fn lod_cell_height(&self) -> i32 {
        self.cell_height
    }

    pub(crate) const fn lod_cell_count_y(&self) -> i32 {
        self.cell_count_y
    }

    pub(crate) const fn lod_min_cell_y(&self) -> i32 {
        self.min_cell_y
    }

    pub(crate) const fn lod_min_y(&self) -> i32 {
        self.min_y
    }

    pub(crate) const fn lod_sea_level(&self) -> i32 {
        self.sea_level
    }

    pub(crate) fn resolve_terrain_block(&self, y: i32, noise: f64) -> RawBlockId {
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

    pub(crate) fn lod_surface_noise(&self, world_x: i32, world_z: i32) -> f64 {
        self.surface_noise.get_surface_noise_value(
            f64::from(world_x) * 0.0625,
            f64::from(world_z) * 0.0625,
            0.0625,
            f64::from(world_x.rem_euclid(CHUNK_WIDTH)) * 0.0625,
        ) * 15.0
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

fn set_block_at_i64_y_if_inside(
    chunk: &mut MutableChunkBlockBuffer,
    local_x: i32,
    y: i64,
    local_z: i32,
    block_id: RawBlockId,
) {
    if y >= chunk.min_y as i64 && y < (chunk.min_y + chunk.height) as i64 {
        chunk.set_block_at_y(local_x, y as i32, local_z, block_id);
    }
}

fn lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}
