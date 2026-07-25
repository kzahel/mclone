use crate::biome::{NoiseBiome, NoiseBiomeSource};
use crate::noise::{BlendedNoise, PerlinNoise, SimplexNoise};

use super::{NoiseModifier, NoiseSettings};

const OLD_CELL_COUNT_Y: i32 = 32;
pub(super) const BIOME_WEIGHT_RADIUS: i32 = 2;

#[derive(Clone, Debug)]
pub struct NoiseSampler<B: NoiseBiomeSource> {
    pub(super) biome_source: B,
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
        let density = self.compute_biome_density(cell_x, cell_z, biome_y, noise_settings);
        self.fill_noise_column_with_density(
            noise_values,
            cell_x,
            cell_z,
            noise_settings,
            min_cell_y,
            cell_count_y,
            density,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn fill_noise_column_with_density(
        &self,
        noise_values: &mut [f64],
        cell_x: i32,
        cell_z: i32,
        noise_settings: &NoiseSettings,
        min_cell_y: i32,
        cell_count_y: i32,
        density: BiomeDensity,
    ) {
        self.for_each_noise_column_value(
            cell_x,
            cell_z,
            noise_settings,
            min_cell_y,
            density,
            0..=cell_count_y,
            |index, noise| noise_values[index as usize] = noise,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn fill_sparse_noise_column_with_density(
        &self,
        noise_values: &mut [f64],
        cell_x: i32,
        cell_z: i32,
        noise_settings: &NoiseSettings,
        min_cell_y: i32,
        cell_count_y: i32,
        vertical_cell_step: i32,
        density: BiomeDensity,
    ) {
        assert!(vertical_cell_step > 0);
        assert_eq!(cell_count_y.rem_euclid(vertical_cell_step), 0);
        assert_eq!(
            noise_values.len(),
            (cell_count_y / vertical_cell_step + 1) as usize
        );
        self.for_each_noise_column_value(
            cell_x,
            cell_z,
            noise_settings,
            min_cell_y,
            density,
            (0..=cell_count_y).step_by(vertical_cell_step as usize),
            |index, noise| {
                noise_values[(index / vertical_cell_step) as usize] = noise;
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn for_each_noise_column_value(
        &self,
        cell_x: i32,
        cell_z: i32,
        noise_settings: &NoiseSettings,
        min_cell_y: i32,
        density: BiomeDensity,
        indices: impl IntoIterator<Item = i32>,
        mut visitor: impl FnMut(i32, f64),
    ) {
        if self.island_noise.is_some() {
            panic!(
                "NoiseSampler island noise override is out of scope for the 1.17.1 overworld target"
            );
        }

        let sampling = noise_settings.noise_sampling_settings();
        let limit_horizontal_scale = 684.412 * sampling.xz_scale();
        let limit_vertical_scale = 684.412 * sampling.y_scale();
        let main_horizontal_scale = limit_horizontal_scale / sampling.xz_factor();
        let main_vertical_scale = limit_vertical_scale / sampling.y_factor();
        let blended_noise_column = BlendedNoise::create_column_cache(
            cell_x,
            cell_z,
            limit_horizontal_scale,
            limit_vertical_scale,
            main_horizontal_scale,
            main_vertical_scale,
        );
        let random_density_offset = if noise_settings.random_density_offset() {
            self.get_random_density(cell_x, cell_z)
        } else {
            0.0
        };

        for index in indices {
            let y = index + min_cell_y;
            let mut noise = self
                .blended_noise
                .sample_and_clamp_noise_cached(y, &blended_noise_column);
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
            visitor(index, self.apply_slide(noise, y));
        }
    }

    fn compute_biome_density(
        &self,
        cell_x: i32,
        cell_z: i32,
        biome_y: i32,
        noise_settings: &NoiseSettings,
    ) -> BiomeDensity {
        let center_depth = self
            .biome_source
            .get_noise_biome(cell_x, biome_y, cell_z)
            .get_depth();
        compute_biome_density_from_neighborhood(
            center_depth,
            noise_settings,
            |offset_x, offset_z| {
                self.biome_source
                    .get_noise_biome(cell_x + offset_x, biome_y, cell_z + offset_z)
            },
        )
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
pub(super) struct BiomeDensity {
    pub(super) depth: f64,
    pub(super) scale: f64,
}

pub(super) fn compute_biome_density_from_neighborhood(
    center_depth: f32,
    noise_settings: &NoiseSettings,
    mut biome_at_offset: impl FnMut(i32, i32) -> NoiseBiome,
) -> BiomeDensity {
    let mut weighted_scale = 0.0_f32;
    let mut weighted_depth = 0.0_f32;
    let mut total_weight = 0.0_f32;

    for offset_x in -BIOME_WEIGHT_RADIUS..=BIOME_WEIGHT_RADIUS {
        for offset_z in -BIOME_WEIGHT_RADIUS..=BIOME_WEIGHT_RADIUS {
            let biome = biome_at_offset(offset_x, offset_z);
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
            let weight =
                (weight_multiplier * biome_weight(offset_x, offset_z)) / (adjusted_depth + 2.0_f32);
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
