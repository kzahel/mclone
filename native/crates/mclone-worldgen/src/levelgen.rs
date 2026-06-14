use crate::noise::{BlendedNoise, PerlinNoise, SimplexNoise};

const OLD_CELL_COUNT_Y: i32 = 32;
const BIOME_WEIGHT_RADIUS: i32 = 2;
const BITS_FOR_Y: i32 = 12;
const Y_SIZE: i32 = (1 << BITS_FOR_Y) - 32;
const MAX_Y: i32 = (Y_SIZE >> 1) - 1;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prng::WorldgenRandom;
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use std::panic;

    const DEPTH_NOISE_OCTAVES: [i32; 16] = [
        -15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0,
    ];

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
