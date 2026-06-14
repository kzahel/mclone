use crate::biome::OverworldBiomeSource;
use crate::block::{AIR, BEDROCK, GeneratedBlockId, RawBlockId, STONE, WATER};
use crate::noise::{BlendedNoise, PerlinNoise, PerlinSimplexNoise, SimplexNoise};
use crate::prng::WorldgenRandom;
use crate::surface::apply_overworld_surface;

const OLD_CELL_COUNT_Y: i32 = 32;
const BIOME_WEIGHT_RADIUS: i32 = 2;
const BITS_FOR_Y: i32 = 12;
const Y_SIZE: i32 = (1 << BITS_FOR_Y) - 32;
const MAX_Y: i32 = (Y_SIZE >> 1) - 1;
const CHUNK_WIDTH: i32 = 16;
const SECTION_HEIGHT: i32 = 16;
const SURFACE_NOISE_OCTAVES: [i32; 4] = [-3, -2, -1, 0];
const DEPTH_NOISE_OCTAVES: [i32; 16] = [
    -15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0,
];
const INT_MIN: i32 = i32::MIN;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseBiome {
    depth: f32,
    scale: f32,
}

impl NoiseBiome {
    pub fn new(depth: f32, scale: f32) -> Self {
        Self { depth, scale }
    }

    pub fn get_depth(&self) -> f32 {
        self.depth
    }

    pub fn get_scale(&self) -> f32 {
        self.scale
    }
}

pub trait NoiseBiomeSource {
    fn get_noise_biome(&self, x: i32, y: i32, z: i32) -> NoiseBiome;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstantBiomeSource {
    biome: NoiseBiome,
}

impl ConstantBiomeSource {
    pub fn new(biome: NoiseBiome) -> Self {
        Self { biome }
    }
}

impl NoiseBiomeSource for ConstantBiomeSource {
    fn get_noise_biome(&self, _x: i32, _y: i32, _z: i32) -> NoiseBiome {
        self.biome
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseSamplingSettings {
    xz_scale: f64,
    y_scale: f64,
    xz_factor: f64,
    y_factor: f64,
}

impl NoiseSamplingSettings {
    pub fn new(xz_scale: f64, y_scale: f64, xz_factor: f64, y_factor: f64) -> Self {
        Self {
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
        }
    }

    pub fn xz_scale(&self) -> f64 {
        self.xz_scale
    }

    pub fn y_scale(&self) -> f64 {
        self.y_scale
    }

    pub fn xz_factor(&self) -> f64 {
        self.xz_factor
    }

    pub fn y_factor(&self) -> f64 {
        self.y_factor
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseSlideSettings {
    target: i32,
    size: i32,
    offset: i32,
}

impl NoiseSlideSettings {
    pub fn new(target: i32, size: i32, offset: i32) -> Self {
        Self {
            target,
            size,
            offset,
        }
    }

    pub fn target(&self) -> i32 {
        self.target
    }

    pub fn size(&self) -> i32 {
        self.size
    }

    pub fn offset(&self) -> i32 {
        self.offset
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoiseSettings {
    min_y: i32,
    height: i32,
    noise_sampling_settings: NoiseSamplingSettings,
    top_slide_settings: NoiseSlideSettings,
    bottom_slide_settings: NoiseSlideSettings,
    noise_size_horizontal: i32,
    noise_size_vertical: i32,
    density_factor: f64,
    density_offset: f64,
    use_simplex_surface_noise: bool,
    random_density_offset: bool,
    island_noise_override: bool,
    is_amplified: bool,
}

impl NoiseSettings {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        min_y: i32,
        height: i32,
        noise_sampling_settings: NoiseSamplingSettings,
        top_slide_settings: NoiseSlideSettings,
        bottom_slide_settings: NoiseSlideSettings,
        noise_size_horizontal: i32,
        noise_size_vertical: i32,
        density_factor: f64,
        density_offset: f64,
        use_simplex_surface_noise: bool,
        random_density_offset: bool,
        island_noise_override: bool,
        is_amplified: bool,
    ) -> Self {
        let settings = Self {
            min_y,
            height,
            noise_sampling_settings,
            top_slide_settings,
            bottom_slide_settings,
            noise_size_horizontal,
            noise_size_vertical,
            density_factor,
            density_offset,
            use_simplex_surface_noise,
            random_density_offset,
            island_noise_override,
            is_amplified,
        };
        settings.guard_y();
        settings
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn noise_sampling_settings(&self) -> &NoiseSamplingSettings {
        &self.noise_sampling_settings
    }

    pub fn top_slide_settings(&self) -> &NoiseSlideSettings {
        &self.top_slide_settings
    }

    pub fn bottom_slide_settings(&self) -> &NoiseSlideSettings {
        &self.bottom_slide_settings
    }

    pub fn noise_size_horizontal(&self) -> i32 {
        self.noise_size_horizontal
    }

    pub fn noise_size_vertical(&self) -> i32 {
        self.noise_size_vertical
    }

    pub fn density_factor(&self) -> f64 {
        self.density_factor
    }

    pub fn density_offset(&self) -> f64 {
        self.density_offset
    }

    pub fn use_simplex_surface_noise(&self) -> bool {
        self.use_simplex_surface_noise
    }

    pub fn random_density_offset(&self) -> bool {
        self.random_density_offset
    }

    pub fn island_noise_override(&self) -> bool {
        self.island_noise_override
    }

    pub fn is_amplified(&self) -> bool {
        self.is_amplified
    }

    fn guard_y(&self) {
        if self.min_y() + self.height() > MAX_Y + 1 {
            panic!("min_y + height cannot be higher than: {}", MAX_Y + 1);
        }

        if self.height() % 16 != 0 {
            panic!("height has to be a multiple of 16");
        }

        if self.min_y() % 16 != 0 {
            panic!("min_y has to be a multiple of 16");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseModifier {
    Passthrough,
}

impl NoiseModifier {
    pub fn modify_noise(&self, noise: f64, _y: i32, _z: i32, _x: i32) -> f64 {
        match self {
            Self::Passthrough => noise,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoiseGeneratorSettings {
    noise_settings: NoiseSettings,
    default_block: u8,
    default_fluid: u8,
    bedrock_roof_position: i32,
    bedrock_floor_position: i32,
    sea_level: i32,
    min_surface_level: i32,
    disable_mob_generation: bool,
    aquifers_enabled: bool,
    noise_caves_enabled: bool,
    deepslate_enabled: bool,
    ore_veins_enabled: bool,
    noodle_caves_enabled: bool,
}

impl NoiseGeneratorSettings {
    pub fn overworld() -> Self {
        Self::overworld_with_amplified(false)
    }

    pub fn overworld_with_amplified(is_amplified: bool) -> Self {
        Self {
            noise_settings: NoiseSettings::create(
                0,
                256,
                NoiseSamplingSettings::new(0.9999999814507745, 0.9999999814507745, 80.0, 160.0),
                NoiseSlideSettings::new(-10, 3, 0),
                NoiseSlideSettings::new(15, 3, 0),
                1,
                2,
                1.0,
                -0.46875,
                true,
                true,
                false,
                is_amplified,
            ),
            default_block: STONE,
            default_fluid: WATER,
            bedrock_roof_position: INT_MIN,
            bedrock_floor_position: 0,
            sea_level: 63,
            min_surface_level: 0,
            disable_mob_generation: false,
            aquifers_enabled: false,
            noise_caves_enabled: false,
            deepslate_enabled: false,
            ore_veins_enabled: false,
            noodle_caves_enabled: false,
        }
    }

    pub fn noise_settings(&self) -> &NoiseSettings {
        &self.noise_settings
    }

    pub fn default_block(&self) -> u8 {
        self.default_block
    }

    pub fn default_fluid(&self) -> u8 {
        self.default_fluid
    }

    pub fn bedrock_roof_position(&self) -> i32 {
        self.bedrock_roof_position
    }

    pub fn bedrock_floor_position(&self) -> i32 {
        self.bedrock_floor_position
    }

    pub fn sea_level(&self) -> i32 {
        self.sea_level
    }

    pub fn min_surface_level(&self) -> i32 {
        self.min_surface_level
    }

    pub fn disable_mob_generation(&self) -> bool {
        self.disable_mob_generation
    }

    pub fn is_aquifers_enabled(&self) -> bool {
        self.aquifers_enabled
    }

    pub fn is_noise_caves_enabled(&self) -> bool {
        self.noise_caves_enabled
    }

    pub fn is_deepslate_enabled(&self) -> bool {
        self.deepslate_enabled
    }

    pub fn is_ore_veins_enabled(&self) -> bool {
        self.ore_veins_enabled
    }

    pub fn is_noodle_caves_enabled(&self) -> bool {
        self.noodle_caves_enabled
    }
}

#[derive(Clone, Debug)]
pub struct NoiseSampler<B: NoiseBiomeSource> {
    biome_source: B,
    cell_width: i32,
    cell_height: i32,
    cell_count_y: i32,
    noise_settings: NoiseSettings,
    blended_noise: BlendedNoise,
    island_noise: Option<SimplexNoise>,
    depth_noise: PerlinNoise,
    top_slide_target: f64,
    top_slide_size: f64,
    top_slide_offset: f64,
    bottom_slide_target: f64,
    bottom_slide_size: f64,
    bottom_slide_offset: f64,
    dimension_density_factor: f64,
    dimension_density_offset: f64,
    cave_noise_modifier: NoiseModifier,
}

impl<B: NoiseBiomeSource> NoiseSampler<B> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        biome_source: B,
        cell_width: i32,
        cell_height: i32,
        cell_count_y: i32,
        noise_settings: NoiseSettings,
        blended_noise: BlendedNoise,
        island_noise: Option<SimplexNoise>,
        depth_noise: PerlinNoise,
        cave_noise_modifier: NoiseModifier,
    ) -> Self {
        let top_slide_target = noise_settings.top_slide_settings().target() as f64;
        let top_slide_size = noise_settings.top_slide_settings().size() as f64;
        let top_slide_offset = noise_settings.top_slide_settings().offset() as f64;
        let bottom_slide_target = noise_settings.bottom_slide_settings().target() as f64;
        let bottom_slide_size = noise_settings.bottom_slide_settings().size() as f64;
        let bottom_slide_offset = noise_settings.bottom_slide_settings().offset() as f64;
        let dimension_density_factor = noise_settings.density_factor();
        let dimension_density_offset = noise_settings.density_offset();

        Self {
            biome_source,
            cell_width,
            cell_height,
            cell_count_y,
            noise_settings,
            blended_noise,
            island_noise,
            depth_noise,
            top_slide_target,
            top_slide_size,
            top_slide_offset,
            bottom_slide_target,
            bottom_slide_size,
            bottom_slide_offset,
            dimension_density_factor,
            dimension_density_offset,
            cave_noise_modifier,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_noise_column(
        &self,
        noise_values: &mut [f64],
        cell_x: i32,
        cell_z: i32,
        noise_settings: &NoiseSettings,
        biome_y: i32,
        min_cell_y: i32,
        cell_count_y: i32,
    ) {
        if self.island_noise.is_some() {
            panic!(
                "NoiseSampler island noise override is out of scope for the 1.17.1 overworld target"
            );
        }

        let density = self.compute_biome_density(cell_x, cell_z, biome_y, noise_settings);
        let sampling = noise_settings.noise_sampling_settings();
        let limit_horizontal_scale = 684.412 * sampling.xz_scale();
        let limit_vertical_scale = 684.412 * sampling.y_scale();
        let main_horizontal_scale = limit_horizontal_scale / sampling.xz_factor();
        let main_vertical_scale = limit_vertical_scale / sampling.y_factor();
        let random_density_offset = if noise_settings.random_density_offset() {
            self.get_random_density(cell_x, cell_z)
        } else {
            0.0
        };

        for index in 0..=cell_count_y {
            let y = index + min_cell_y;
            let mut noise = self.blended_noise.sample_and_clamp_noise(
                cell_x,
                y,
                cell_z,
                limit_horizontal_scale,
                limit_vertical_scale,
                main_horizontal_scale,
                main_vertical_scale,
            );
            noise = self.compute_initial_density(
                y,
                density.depth,
                density.scale,
                random_density_offset,
            ) + noise;
            noise = self.cave_noise_modifier.modify_noise(
                noise,
                y * self.cell_height,
                cell_z * self.cell_width,
                cell_x * self.cell_width,
            );
            noise_values[index as usize] = self.apply_slide(noise, y);
        }
    }

    fn compute_biome_density(
        &self,
        cell_x: i32,
        cell_z: i32,
        biome_y: i32,
        noise_settings: &NoiseSettings,
    ) -> BiomeDensity {
        let mut weighted_scale = 0.0_f32;
        let mut weighted_depth = 0.0_f32;
        let mut total_weight = 0.0_f32;
        let center_depth = self
            .biome_source
            .get_noise_biome(cell_x, biome_y, cell_z)
            .get_depth();

        for offset_x in -BIOME_WEIGHT_RADIUS..=BIOME_WEIGHT_RADIUS {
            for offset_z in -BIOME_WEIGHT_RADIUS..=BIOME_WEIGHT_RADIUS {
                let biome = self.biome_source.get_noise_biome(
                    cell_x + offset_x,
                    biome_y,
                    cell_z + offset_z,
                );
                let biome_depth = biome.get_depth();
                let biome_scale = biome.get_scale();
                let (adjusted_depth, adjusted_scale) =
                    if noise_settings.is_amplified() && biome_depth > 0.0 {
                        (
                            1.0_f32 + biome_depth * 2.0_f32,
                            1.0_f32 + biome_scale * 4.0_f32,
                        )
                    } else {
                        (biome_depth, biome_scale)
                    };

                let weight_multiplier = if biome_depth > center_depth {
                    0.5_f32
                } else {
                    1.0_f32
                };
                let weight = (weight_multiplier * biome_weight(offset_x, offset_z))
                    / (adjusted_depth + 2.0_f32);
                weighted_scale += adjusted_scale * weight;
                weighted_depth += adjusted_depth * weight;
                total_weight += weight;
            }
        }

        let average_depth = weighted_depth / total_weight;
        let average_scale = weighted_scale / total_weight;
        let depth_offset = average_depth * 0.5_f32 - 0.125_f32;
        let scale_factor = average_scale * 0.9_f32 + 0.1_f32;

        BiomeDensity {
            depth: f64::from(depth_offset) * 0.265625,
            scale: 96.0 / f64::from(scale_factor),
        }
    }

    fn compute_initial_density(
        &self,
        y: i32,
        depth: f64,
        scale: f64,
        random_density_offset: f64,
    ) -> f64 {
        let density = 1.0 - y as f64 * 2.0 / OLD_CELL_COUNT_Y as f64 + random_density_offset;
        let dimension_density =
            density * self.dimension_density_factor + self.dimension_density_offset;
        let value = (dimension_density + depth) * scale;
        value * if value > 0.0 { 4.0 } else { 1.0 }
    }

    fn apply_slide(&self, mut noise: f64, y: i32) -> f64 {
        let min_cell_y = self.noise_settings.min_y().div_euclid(self.cell_height);
        let relative_y = y - min_cell_y;

        if self.top_slide_size > 0.0 {
            let top_slide_delta = (self.cell_count_y - relative_y) as f64 - self.top_slide_offset;
            noise = clamped_lerp(
                self.top_slide_target,
                noise,
                top_slide_delta / self.top_slide_size,
            );
        }

        if self.bottom_slide_size > 0.0 {
            let bottom_slide_delta = relative_y as f64 - self.bottom_slide_offset;
            noise = clamped_lerp(
                self.bottom_slide_target,
                noise,
                bottom_slide_delta / self.bottom_slide_size,
            );
        }

        noise
    }

    fn get_random_density(&self, x: i32, z: i32) -> f64 {
        let value = self.depth_noise.get_value_scaled(
            x.wrapping_mul(200) as f64,
            10.0,
            z.wrapping_mul(200) as f64,
            1.0,
            0.0,
            true,
        );
        let adjusted_value = if value < 0.0 { -value * 0.3 } else { value };
        let density = adjusted_value * 24.575625 - 2.0;
        if density < 0.0 {
            density * 0.009486607142857142
        } else {
            density.min(1.0) * 0.006640625
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTick {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub target: String,
    pub delay: i32,
}

impl ScheduledTick {
    pub fn new(x: i32, y: i32, z: i32, target: impl Into<String>, delay: i32) -> Self {
        Self {
            x,
            y,
            z,
            target: target.into(),
            delay,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutableChunkBlockBuffer {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    pub blocks: Vec<u8>,
    block_ticks: Vec<ScheduledTick>,
    liquid_ticks: Vec<ScheduledTick>,
}

impl MutableChunkBlockBuffer {
    pub fn new(chunk_x: i32, chunk_z: i32, min_y: i32, height: i32) -> Self {
        if height <= 0 || height % SECTION_HEIGHT != 0 {
            panic!("chunk height {height} must be a positive multiple of {SECTION_HEIGHT}");
        }

        Self {
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks: vec![AIR; height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize],
            block_ticks: Vec::new(),
            liquid_ticks: Vec::new(),
        }
    }

    pub fn get_block(&self, local_x: i32, local_y: i32, local_z: i32) -> u8 {
        self.blocks[block_buffer_index(local_x, local_y, local_z)]
    }

    pub fn set_block(&mut self, local_x: i32, local_y: i32, local_z: i32, block_id: u8) {
        let index = block_buffer_index(local_x, local_y, local_z);
        self.blocks[index] = block_id;
    }

    pub fn get_block_at_y(&self, local_x: i32, y: i32, local_z: i32) -> u8 {
        self.get_block(local_x, y - self.min_y, local_z)
    }

    pub fn set_block_at_y(&mut self, local_x: i32, y: i32, local_z: i32, block_id: u8) {
        self.set_block(local_x, y - self.min_y, local_z, block_id);
    }

    pub fn schedule_block_tick(
        &mut self,
        world_x: i32,
        y: i32,
        world_z: i32,
        target: impl Into<String>,
        delay: i32,
    ) {
        self.block_ticks
            .push(ScheduledTick::new(world_x, y, world_z, target, delay));
    }

    pub fn schedule_liquid_tick(
        &mut self,
        world_x: i32,
        y: i32,
        world_z: i32,
        target: impl Into<String>,
        delay: i32,
    ) {
        self.liquid_ticks
            .push(ScheduledTick::new(world_x, y, world_z, target, delay));
    }

    pub fn block_ticks(&self) -> &[ScheduledTick] {
        &self.block_ticks
    }

    pub fn liquid_ticks(&self) -> &[ScheduledTick] {
        &self.liquid_ticks
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub height: i32,
    blocks: Vec<RawBlockId>,
}

impl GeneratedChunk {
    pub const WIDTH: i32 = CHUNK_WIDTH;

    pub fn from_raw_parts(
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        blocks: Vec<RawBlockId>,
    ) -> Self {
        if height <= 0 || height % SECTION_HEIGHT != 0 {
            panic!("chunk height {height} must be a positive multiple of {SECTION_HEIGHT}");
        }
        let expected_len = height as usize * CHUNK_WIDTH as usize * CHUNK_WIDTH as usize;
        if blocks.len() != expected_len {
            panic!(
                "generated chunk block buffer has {} entries; expected {expected_len}",
                blocks.len()
            );
        }
        Self {
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks,
        }
    }

    pub fn from_mutable_buffer(buffer: MutableChunkBlockBuffer) -> Self {
        Self::from_raw_parts(
            buffer.chunk_x,
            buffer.chunk_z,
            buffer.min_y,
            buffer.height,
            buffer.blocks,
        )
    }

    pub fn blocks(&self) -> &[RawBlockId] {
        &self.blocks
    }

    pub fn block_at_local(&self, local_x: i32, local_y: i32, local_z: i32) -> GeneratedBlockId {
        self.assert_local_position(local_x, local_y, local_z);
        GeneratedBlockId(self.blocks[block_buffer_index(local_x, local_y, local_z)])
    }

    pub fn block_at_y(&self, local_x: i32, y: i32, local_z: i32) -> GeneratedBlockId {
        self.block_at_local(local_x, y - self.min_y, local_z)
    }

    pub fn non_air_block_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block_id| **block_id != AIR)
            .count()
    }

    fn assert_local_position(&self, local_x: i32, local_y: i32, local_z: i32) {
        if !(0..CHUNK_WIDTH).contains(&local_x)
            || !(0..self.height).contains(&local_y)
            || !(0..CHUNK_WIDTH).contains(&local_z)
        {
            panic!(
                "local block position ({local_x}, {local_y}, {local_z}) is outside generated chunk {}..{}",
                self.min_y,
                self.min_y + self.height
            );
        }
    }
}

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

    fn fill_terrain_block_buffer(&self, chunk: &mut MutableChunkBlockBuffer) {
        let noise_columns = self.create_noise_columns(chunk.chunk_x, chunk.chunk_z);

        for cell_x in 0..self.cell_count_x {
            for cell_z in 0..self.cell_count_z {
                for cell_y in (0..self.cell_count_y).rev() {
                    let x0z0y0 = noise_columns[column_index(
                        cell_x,
                        cell_z,
                        cell_y,
                        self.cell_count_z,
                        self.cell_count_y,
                    )];
                    let x0z0y1 = noise_columns[column_index(
                        cell_x,
                        cell_z,
                        cell_y + 1,
                        self.cell_count_z,
                        self.cell_count_y,
                    )];
                    let x1z0y0 = noise_columns[column_index(
                        cell_x + 1,
                        cell_z,
                        cell_y,
                        self.cell_count_z,
                        self.cell_count_y,
                    )];
                    let x1z0y1 = noise_columns[column_index(
                        cell_x + 1,
                        cell_z,
                        cell_y + 1,
                        self.cell_count_z,
                        self.cell_count_y,
                    )];
                    let x0z1y0 = noise_columns[column_index(
                        cell_x,
                        cell_z + 1,
                        cell_y,
                        self.cell_count_z,
                        self.cell_count_y,
                    )];
                    let x0z1y1 = noise_columns[column_index(
                        cell_x,
                        cell_z + 1,
                        cell_y + 1,
                        self.cell_count_z,
                        self.cell_count_y,
                    )];
                    let x1z1y0 = noise_columns[column_index(
                        cell_x + 1,
                        cell_z + 1,
                        cell_y,
                        self.cell_count_z,
                        self.cell_count_y,
                    )];
                    let x1z1y1 = noise_columns[column_index(
                        cell_x + 1,
                        cell_z + 1,
                        cell_y + 1,
                        self.cell_count_z,
                        self.cell_count_y,
                    )];

                    for y_offset in (0..self.cell_height).rev() {
                        let y_fraction = y_offset as f64 / self.cell_height as f64;
                        let block_y = (self.min_cell_y + cell_y) * self.cell_height + y_offset;
                        let local_y = block_y - self.min_y;

                        for x_offset in 0..self.cell_width {
                            let x_fraction = x_offset as f64 / self.cell_width as f64;
                            let local_x = cell_x * self.cell_width + x_offset;

                            for z_offset in 0..self.cell_width {
                                let z_fraction = z_offset as f64 / self.cell_width as f64;
                                let density = lerp3(
                                    y_fraction, x_fraction, z_fraction, x0z0y0, x0z0y1, x1z0y0,
                                    x1z0y1, x0z1y0, x0z1y1, x1z1y0, x1z1y1,
                                );
                                let block_id = self.resolve_terrain_block(block_y, density);
                                if block_id != AIR {
                                    let local_z = cell_z * self.cell_width + z_offset;
                                    chunk.set_block(local_x, local_y, local_z, block_id);
                                }
                            }
                        }
                    }
                }
            }
        }
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

        for cell_x in 0..=self.cell_count_x {
            for cell_z in 0..=self.cell_count_z {
                let mut noise_values = vec![0.0; self.cell_count_y as usize + 1];
                self.sampler.fill_noise_column(
                    &mut noise_values,
                    cell_min_x + cell_x,
                    cell_min_z + cell_z,
                    noise_settings,
                    self.sea_level,
                    self.min_cell_y,
                    self.cell_count_y,
                );

                for (cell_y, value) in noise_values.into_iter().enumerate() {
                    columns[column_index(
                        cell_x,
                        cell_z,
                        cell_y as i32,
                        self.cell_count_z,
                        self.cell_count_y,
                    )] = value;
                }
            }
        }

        columns
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
        let min_block_x = chunk.chunk_x * CHUNK_WIDTH;
        let min_block_z = chunk.chunk_z * CHUNK_WIDTH;
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
    let generator = NoiseBasedChunkGenerator::new(
        OverworldBiomeSource::new(seed, false, false),
        seed,
        NoiseGeneratorSettings::overworld(),
    );
    let mut chunk = generator.fill_from_noise(chunk_x, chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    GeneratedChunk::from_mutable_buffer(chunk)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BiomeDensity {
    depth: f64,
    scale: f64,
}

fn biome_weight(offset_x: i32, offset_z: i32) -> f32 {
    let squared_distance = (offset_x * offset_x + offset_z * offset_z) as f32 + 0.2_f32;
    10.0_f32 / (squared_distance as f64).sqrt() as f32
}

fn clamped_lerp(start: f64, end: f64, delta: f64) -> f64 {
    if delta < 0.0 {
        start
    } else if delta > 1.0 {
        end
    } else {
        start + delta * (end - start)
    }
}

fn block_buffer_index(local_x: i32, local_y: i32, local_z: i32) -> usize {
    ((local_y << 8) | (local_z << 4) | local_x) as usize
}

fn world_surface_height(chunk: &MutableChunkBlockBuffer, local_x: i32, local_z: i32) -> i32 {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        if chunk.get_block_at_y(local_x, y, local_z) != AIR {
            return y + 1;
        }
    }
    chunk.min_y
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

fn column_index(
    cell_x: i32,
    cell_z: i32,
    cell_y: i32,
    cell_count_z: i32,
    cell_count_y: i32,
) -> usize {
    ((cell_x * (cell_count_z + 1) + cell_z) * (cell_count_y + 1) + cell_y) as usize
}

fn lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

fn lerp2(delta_x: f64, delta_y: f64, x0_y0: f64, x1_y0: f64, x0_y1: f64, x1_y1: f64) -> f64 {
    lerp(
        delta_y,
        lerp(delta_x, x0_y0, x1_y0),
        lerp(delta_x, x0_y1, x1_y1),
    )
}

#[allow(clippy::too_many_arguments)]
fn lerp3(
    delta_x: f64,
    delta_y: f64,
    delta_z: f64,
    x0_y0_z0: f64,
    x1_y0_z0: f64,
    x0_y1_z0: f64,
    x1_y1_z0: f64,
    x0_y0_z1: f64,
    x1_y0_z1: f64,
    x0_y1_z1: f64,
    x1_y1_z1: f64,
) -> f64 {
    lerp(
        delta_z,
        lerp2(delta_x, delta_y, x0_y0_z0, x1_y0_z0, x0_y1_z0, x1_y1_z0),
        lerp2(delta_x, delta_y, x0_y0_z1, x1_y0_z1, x0_y1_z1, x1_y1_z1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::OverworldBiomeSource;
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
