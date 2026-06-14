use crate::prng::{RandomSource, WorldgenRandom};

const SHIFT_UP_EPSILON: f64 = 1.0e-7_f32 as f64;
const SQRT_3: f64 = 1.732_050_807_568_877_2;
const SIMPLEX_F2: f64 = 0.5 * (SQRT_3 - 1.0);
const SIMPLEX_G2: f64 = (3.0 - SQRT_3) / 6.0;
const SIMPLEX_F3: f64 = 0.333_333_333_333_333_3;
const SIMPLEX_G3: f64 = 0.166_666_666_666_666_66;
const ROUND_OFF: f64 = 33_554_432.0;
const PERLIN_SIMPLEX_RESEED_FACTOR: f64 = 9.223372E18_f32 as f64;
const LIMIT_OCTAVES: [i32; 16] = [
    -15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0,
];
const MAIN_OCTAVES: [i32; 8] = [-7, -6, -5, -4, -3, -2, -1, 0];
const GRADIENTS: [[i32; 3]; 16] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
    [1, 1, 0],
    [0, -1, 1],
    [-1, 1, 0],
    [0, -1, -1],
];

#[derive(Clone, Debug)]
pub struct ImprovedNoise {
    p: [u8; 256],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
}

impl ImprovedNoise {
    pub fn new(random: &mut impl RandomSource) -> Self {
        let xo = random.next_double() * 256.0;
        let yo = random.next_double() * 256.0;
        let zo = random.next_double() * 256.0;
        let mut p = [0_u8; 256];

        for (index, value) in p.iter_mut().enumerate() {
            *value = index as u8;
        }

        for index in 0..256 {
            let offset = random.next_int_bound(256 - index as i32) as usize;
            p.swap(index, index + offset);
        }

        Self { p, xo, yo, zo }
    }

    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        self.noise(x, y, z)
    }

    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        self.noise_scaled(x, y, z, 0.0, 0.0)
    }

    pub fn noise_scaled(&self, x: f64, y: f64, z: f64, y_scale: f64, y_max: f64) -> f64 {
        let shifted_x = x + self.xo;
        let shifted_y = y + self.yo;
        let shifted_z = z + self.zo;
        let grid_x = floor(shifted_x);
        let grid_y = floor(shifted_y);
        let grid_z = floor(shifted_z);
        let delta_x = shifted_x - grid_x as f64;
        let delta_y = shifted_y - grid_y as f64;
        let delta_z = shifted_z - grid_z as f64;
        let y_shift = if y_scale != 0.0 {
            let capped_y = if y_max >= 0.0 && y_max < delta_y {
                y_max
            } else {
                delta_y
            };
            floor(capped_y / y_scale + SHIFT_UP_EPSILON) as f64 * y_scale
        } else {
            0.0
        };

        self.sample_and_lerp(
            grid_x,
            grid_y,
            grid_z,
            delta_x,
            delta_y - y_shift,
            delta_z,
            delta_y,
        )
    }

    fn permutation(&self, index: i32) -> i32 {
        self.p[(index & 0xff) as usize] as i32
    }

    #[allow(clippy::too_many_arguments)]
    fn sample_and_lerp(
        &self,
        grid_x: i32,
        grid_y: i32,
        grid_z: i32,
        delta_x: f64,
        weird_delta_y: f64,
        delta_z: f64,
        delta_y: f64,
    ) -> f64 {
        let perm_x0 = self.permutation(grid_x);
        let perm_x1 = self.permutation(grid_x + 1);
        let perm_x0_y0 = self.permutation(perm_x0 + grid_y);
        let perm_x0_y1 = self.permutation(perm_x0 + grid_y + 1);
        let perm_x1_y0 = self.permutation(perm_x1 + grid_y);
        let perm_x1_y1 = self.permutation(perm_x1 + grid_y + 1);

        let x0_y0_z0 = grad_dot(
            self.permutation(perm_x0_y0 + grid_z),
            delta_x,
            weird_delta_y,
            delta_z,
        );
        let x1_y0_z0 = grad_dot(
            self.permutation(perm_x1_y0 + grid_z),
            delta_x - 1.0,
            weird_delta_y,
            delta_z,
        );
        let x0_y1_z0 = grad_dot(
            self.permutation(perm_x0_y1 + grid_z),
            delta_x,
            weird_delta_y - 1.0,
            delta_z,
        );
        let x1_y1_z0 = grad_dot(
            self.permutation(perm_x1_y1 + grid_z),
            delta_x - 1.0,
            weird_delta_y - 1.0,
            delta_z,
        );
        let x0_y0_z1 = grad_dot(
            self.permutation(perm_x0_y0 + grid_z + 1),
            delta_x,
            weird_delta_y,
            delta_z - 1.0,
        );
        let x1_y0_z1 = grad_dot(
            self.permutation(perm_x1_y0 + grid_z + 1),
            delta_x - 1.0,
            weird_delta_y,
            delta_z - 1.0,
        );
        let x0_y1_z1 = grad_dot(
            self.permutation(perm_x0_y1 + grid_z + 1),
            delta_x,
            weird_delta_y - 1.0,
            delta_z - 1.0,
        );
        let x1_y1_z1 = grad_dot(
            self.permutation(perm_x1_y1 + grid_z + 1),
            delta_x - 1.0,
            weird_delta_y - 1.0,
            delta_z - 1.0,
        );

        lerp3(
            smoothstep(delta_x),
            smoothstep(delta_y),
            smoothstep(delta_z),
            x0_y0_z0,
            x1_y0_z0,
            x0_y1_z0,
            x1_y1_z0,
            x0_y0_z1,
            x1_y0_z1,
            x0_y1_z1,
            x1_y1_z1,
        )
    }
}

#[derive(Clone, Debug)]
pub struct SimplexNoise {
    p: [u8; 256],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
}

impl SimplexNoise {
    pub fn new(random: &mut impl RandomSource) -> Self {
        let xo = random.next_double() * 256.0;
        let yo = random.next_double() * 256.0;
        let zo = random.next_double() * 256.0;
        let mut p = [0_u8; 256];

        for (index, value) in p.iter_mut().enumerate() {
            *value = index as u8;
        }

        for index in 0..256 {
            let offset = random.next_int_bound(256 - index as i32) as usize;
            p.swap(index, index + offset);
        }

        Self { p, xo, yo, zo }
    }

    pub fn get_value_2d(&self, x: f64, z: f64) -> f64 {
        let skew = (x + z) * SIMPLEX_F2;
        let cell_x = floor(x + skew);
        let cell_z = floor(z + skew);
        let unskew = (cell_x + cell_z) as f64 * SIMPLEX_G2;
        let cell_origin_x = cell_x as f64 - unskew;
        let cell_origin_z = cell_z as f64 - unskew;
        let local_x = x - cell_origin_x;
        let local_z = z - cell_origin_z;
        let (offset_x, offset_z) = if local_x > local_z { (1, 0) } else { (0, 1) };
        let second_corner_x = local_x - offset_x as f64 + SIMPLEX_G2;
        let second_corner_z = local_z - offset_z as f64 + SIMPLEX_G2;
        let third_corner_x = local_x - 1.0 + 2.0 * SIMPLEX_G2;
        let third_corner_z = local_z - 1.0 + 2.0 * SIMPLEX_G2;
        let perm_x = cell_x & 0xff;
        let perm_z = cell_z & 0xff;
        let gradient0 = self.permutation(perm_x + self.permutation(perm_z)) % 12;
        let gradient1 =
            self.permutation(perm_x + offset_x + self.permutation(perm_z + offset_z)) % 12;
        let gradient2 = self.permutation(perm_x + 1 + self.permutation(perm_z + 1)) % 12;
        let corner0 = self.get_corner_noise_3d(gradient0, local_x, local_z, 0.0, 0.5);
        let corner1 =
            self.get_corner_noise_3d(gradient1, second_corner_x, second_corner_z, 0.0, 0.5);
        let corner2 = self.get_corner_noise_3d(gradient2, third_corner_x, third_corner_z, 0.0, 0.5);
        70.0 * (corner0 + corner1 + corner2)
    }

    pub fn get_value_3d(&self, x: f64, y: f64, z: f64) -> f64 {
        let skew = (x + y + z) * SIMPLEX_F3;
        let cell_x = floor(x + skew);
        let cell_y = floor(y + skew);
        let cell_z = floor(z + skew);
        let unskew = (cell_x + cell_y + cell_z) as f64 * SIMPLEX_G3;
        let cell_origin_x = cell_x as f64 - unskew;
        let cell_origin_y = cell_y as f64 - unskew;
        let cell_origin_z = cell_z as f64 - unskew;
        let local_x = x - cell_origin_x;
        let local_y = y - cell_origin_y;
        let local_z = z - cell_origin_z;

        let (offset0_x, offset0_y, offset0_z, offset1_x, offset1_y, offset1_z) =
            if local_x >= local_y {
                if local_y >= local_z {
                    (1, 0, 0, 1, 1, 0)
                } else if local_x >= local_z {
                    (1, 0, 0, 1, 0, 1)
                } else {
                    (0, 0, 1, 1, 0, 1)
                }
            } else if local_y < local_z {
                (0, 0, 1, 0, 1, 1)
            } else if local_x < local_z {
                (0, 1, 0, 0, 1, 1)
            } else {
                (0, 1, 0, 1, 1, 0)
            };

        let second_corner_x = local_x - offset0_x as f64 + SIMPLEX_G3;
        let second_corner_y = local_y - offset0_y as f64 + SIMPLEX_G3;
        let second_corner_z = local_z - offset0_z as f64 + SIMPLEX_G3;
        let third_corner_x = local_x - offset1_x as f64 + 2.0 * SIMPLEX_G3;
        let third_corner_y = local_y - offset1_y as f64 + 2.0 * SIMPLEX_G3;
        let third_corner_z = local_z - offset1_z as f64 + 2.0 * SIMPLEX_G3;
        let fourth_corner_x = local_x - 1.0 + 0.5;
        let fourth_corner_y = local_y - 1.0 + 0.5;
        let fourth_corner_z = local_z - 1.0 + 0.5;
        let perm_x = cell_x & 0xff;
        let perm_y = cell_y & 0xff;
        let perm_z = cell_z & 0xff;
        let gradient0 =
            self.permutation(perm_x + self.permutation(perm_y + self.permutation(perm_z))) % 12;
        let gradient1 = self.permutation(
            perm_x
                + offset0_x
                + self.permutation(perm_y + offset0_y + self.permutation(perm_z + offset0_z)),
        ) % 12;
        let gradient2 = self.permutation(
            perm_x
                + offset1_x
                + self.permutation(perm_y + offset1_y + self.permutation(perm_z + offset1_z)),
        ) % 12;
        let gradient3 = self
            .permutation(perm_x + 1 + self.permutation(perm_y + 1 + self.permutation(perm_z + 1)))
            % 12;
        let corner0 = self.get_corner_noise_3d(gradient0, local_x, local_y, local_z, 0.6);
        let corner1 = self.get_corner_noise_3d(
            gradient1,
            second_corner_x,
            second_corner_y,
            second_corner_z,
            0.6,
        );
        let corner2 = self.get_corner_noise_3d(
            gradient2,
            third_corner_x,
            third_corner_y,
            third_corner_z,
            0.6,
        );
        let corner3 = self.get_corner_noise_3d(
            gradient3,
            fourth_corner_x,
            fourth_corner_y,
            fourth_corner_z,
            0.6,
        );
        32.0 * (corner0 + corner1 + corner2 + corner3)
    }

    fn permutation(&self, index: i32) -> i32 {
        self.p[(index & 0xff) as usize] as i32
    }

    fn get_corner_noise_3d(&self, gradient_index: i32, x: f64, y: f64, z: f64, offset: f64) -> f64 {
        let mut value = offset - x * x - y * y - z * z;
        if value < 0.0 {
            return 0.0;
        }

        value *= value;
        value * value * dot(gradient_index as usize, x, y, z)
    }
}

#[derive(Clone, Debug)]
pub struct PerlinNoise {
    noise_levels: Vec<Option<ImprovedNoise>>,
    amplitudes: Vec<f64>,
    lowest_freq_value_factor: f64,
    lowest_freq_input_factor: f64,
}

impl PerlinNoise {
    pub fn from_octaves(random: &mut impl RandomSource, octaves: &[i32]) -> Self {
        let (first_octave, amplitudes) = make_amplitudes(octaves);
        Self::new_with_amplitudes(random, first_octave, amplitudes)
    }

    pub fn create(random: &mut impl RandomSource, first_octave: i32, amplitudes: &[f64]) -> Self {
        if amplitudes.is_empty() {
            panic!("Need some amplitudes!");
        }
        Self::new_with_amplitudes(random, first_octave, amplitudes.to_vec())
    }

    fn new_with_amplitudes(
        random: &mut impl RandomSource,
        first_octave: i32,
        amplitudes: Vec<f64>,
    ) -> Self {
        let base_noise = ImprovedNoise::new(random);
        let size = amplitudes.len();
        let zero_octave_index = -first_octave;
        let mut noise_levels = vec![None; size];

        if zero_octave_index >= 0 && (zero_octave_index as usize) < size {
            let index = zero_octave_index as usize;
            if amplitudes[index] != 0.0 {
                noise_levels[index] = Some(base_noise);
            }
        }

        for index in (0..zero_octave_index).rev() {
            if (index as usize) < size {
                if amplitudes[index as usize] != 0.0 {
                    noise_levels[index as usize] = Some(ImprovedNoise::new(random));
                } else {
                    skip_octave(random);
                }
            } else {
                skip_octave(random);
            }
        }

        if zero_octave_index < size as i32 - 1 {
            panic!("Positive octaves are temporarily disabled");
        }

        Self {
            noise_levels,
            amplitudes,
            lowest_freq_input_factor: 2.0_f64.powi(-zero_octave_index),
            lowest_freq_value_factor: 2.0_f64.powi(size as i32 - 1)
                / (2.0_f64.powi(size as i32) - 1.0),
        }
    }

    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        self.get_value_scaled(x, y, z, 0.0, 0.0, false)
    }

    pub fn get_value_scaled(
        &self,
        x: f64,
        y: f64,
        z: f64,
        y_scale: f64,
        y_max: f64,
        use_fixed_y: bool,
    ) -> f64 {
        let mut value = 0.0;
        let mut input_factor = self.lowest_freq_input_factor;
        let mut value_factor = self.lowest_freq_value_factor;

        for (index, noise) in self.noise_levels.iter().enumerate() {
            if let Some(noise) = noise {
                let sample = noise.noise_scaled(
                    Self::wrap(x * input_factor),
                    if use_fixed_y {
                        -noise.yo
                    } else {
                        Self::wrap(y * input_factor)
                    },
                    Self::wrap(z * input_factor),
                    y_scale * input_factor,
                    y_max * input_factor,
                );
                value += self.amplitudes[index] * sample * value_factor;
            }

            input_factor *= 2.0;
            value_factor /= 2.0;
        }

        value
    }

    pub fn get_surface_noise_value(&self, x: f64, y: f64, z: f64, y_max: f64) -> f64 {
        self.get_value_scaled(x, y, 0.0, z, y_max, false)
    }

    pub fn get_octave_noise(&self, octave: i32) -> Option<&ImprovedNoise> {
        let index = self.noise_levels.len() as i32 - 1 - octave;
        if index < 0 {
            return None;
        }
        self.noise_levels
            .get(index as usize)
            .and_then(Option::as_ref)
    }

    pub fn wrap(value: f64) -> f64 {
        value - floor_i64(value / ROUND_OFF + 0.5) as f64 * ROUND_OFF
    }
}

#[derive(Clone, Debug)]
pub struct PerlinSimplexNoise {
    noise_levels: Vec<Option<SimplexNoise>>,
    highest_freq_value_factor: f64,
    highest_freq_input_factor: f64,
}

impl PerlinSimplexNoise {
    pub fn from_octaves(random: &mut impl RandomSource, octaves: &[i32]) -> Self {
        let mut unique_sorted_octaves = octaves.to_vec();
        unique_sorted_octaves.sort_unstable();
        unique_sorted_octaves.dedup();

        if unique_sorted_octaves.is_empty() {
            panic!("Need some octaves!");
        }

        let first = -unique_sorted_octaves[0];
        let last = *unique_sorted_octaves.last().expect("non-empty octave set");
        let total = first + last + 1;
        if total < 1 {
            panic!("Total number of octaves needs to be >= 1");
        }

        let base_noise = SimplexNoise::new(random);
        let highest_octave = last;
        let mut noise_levels = vec![None; total as usize];

        if last >= 0 && last < total && unique_sorted_octaves.binary_search(&0).is_ok() {
            noise_levels[last as usize] = Some(base_noise.clone());
        }

        for index in last + 1..total {
            if index >= 0
                && unique_sorted_octaves
                    .binary_search(&(highest_octave - index))
                    .is_ok()
            {
                noise_levels[index as usize] = Some(SimplexNoise::new(random));
            } else {
                random.consume_count(262);
            }
        }

        if last > 0 {
            let seed = to_java_long(
                base_noise.get_value_3d(base_noise.xo, base_noise.yo, base_noise.zo)
                    * PERLIN_SIMPLEX_RESEED_FACTOR,
            );
            let mut fork = WorldgenRandom::new(seed);

            for index in (0..highest_octave).rev() {
                if index < total
                    && unique_sorted_octaves
                        .binary_search(&(highest_octave - index))
                        .is_ok()
                {
                    noise_levels[index as usize] = Some(SimplexNoise::new(&mut fork));
                } else {
                    fork.consume_count(262);
                }
            }
        }

        Self {
            noise_levels,
            highest_freq_input_factor: 2.0_f64.powi(last),
            highest_freq_value_factor: 1.0 / (2.0_f64.powi(total) - 1.0),
        }
    }

    pub fn get_value(&self, x: f64, y: f64, use_noise_offsets: bool) -> f64 {
        let mut value = 0.0;
        let mut input_factor = self.highest_freq_input_factor;
        let mut value_factor = self.highest_freq_value_factor;

        for noise in &self.noise_levels {
            if let Some(noise) = noise {
                value += noise.get_value_2d(
                    x * input_factor + if use_noise_offsets { noise.xo } else { 0.0 },
                    y * input_factor + if use_noise_offsets { noise.yo } else { 0.0 },
                ) * value_factor;
            }

            input_factor /= 2.0;
            value_factor *= 2.0;
        }

        value
    }

    pub fn get_surface_noise_value(&self, x: f64, y: f64, _z: f64, _y_max: f64) -> f64 {
        self.get_value(x, y, true) * 0.55
    }
}

#[derive(Clone, Debug)]
pub struct BlendedNoise {
    min_limit_noise: PerlinNoise,
    max_limit_noise: PerlinNoise,
    main_noise: PerlinNoise,
}

impl BlendedNoise {
    pub fn new(random: &mut impl RandomSource) -> Self {
        let min_limit_noise = PerlinNoise::from_octaves(random, &LIMIT_OCTAVES);
        let max_limit_noise = PerlinNoise::from_octaves(random, &LIMIT_OCTAVES);
        let main_noise = PerlinNoise::from_octaves(random, &MAIN_OCTAVES);
        Self::from_noises(min_limit_noise, max_limit_noise, main_noise)
    }

    pub fn from_noises(
        min_limit_noise: PerlinNoise,
        max_limit_noise: PerlinNoise,
        main_noise: PerlinNoise,
    ) -> Self {
        Self {
            min_limit_noise,
            max_limit_noise,
            main_noise,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sample_and_clamp_noise(
        &self,
        x: i32,
        y: i32,
        z: i32,
        limit_horizontal_scale: f64,
        limit_vertical_scale: f64,
        main_horizontal_scale: f64,
        main_vertical_scale: f64,
    ) -> f64 {
        let mut min = 0.0;
        let mut max = 0.0;
        let mut main = 0.0;
        let mut input_factor = 1.0;

        for octave in 0..8 {
            if let Some(noise) = self.main_noise.get_octave_noise(octave) {
                main += noise.noise_scaled(
                    PerlinNoise::wrap(x as f64 * main_horizontal_scale * input_factor),
                    PerlinNoise::wrap(y as f64 * main_vertical_scale * input_factor),
                    PerlinNoise::wrap(z as f64 * main_horizontal_scale * input_factor),
                    main_vertical_scale * input_factor,
                    y as f64 * main_vertical_scale * input_factor,
                ) / input_factor;
            }

            input_factor /= 2.0;
        }

        let blend = (main / 10.0 + 1.0) / 2.0;
        let skip_min = blend >= 1.0;
        let skip_max = blend <= 0.0;
        input_factor = 1.0;

        for octave in 0..16 {
            let wrapped_x = PerlinNoise::wrap(x as f64 * limit_horizontal_scale * input_factor);
            let wrapped_y = PerlinNoise::wrap(y as f64 * limit_vertical_scale * input_factor);
            let wrapped_z = PerlinNoise::wrap(z as f64 * limit_horizontal_scale * input_factor);
            let y_scale = limit_vertical_scale * input_factor;

            if !skip_min {
                if let Some(noise) = self.min_limit_noise.get_octave_noise(octave) {
                    min += noise.noise_scaled(
                        wrapped_x,
                        wrapped_y,
                        wrapped_z,
                        y_scale,
                        y as f64 * y_scale,
                    ) / input_factor;
                }
            }

            if !skip_max {
                if let Some(noise) = self.max_limit_noise.get_octave_noise(octave) {
                    max += noise.noise_scaled(
                        wrapped_x,
                        wrapped_y,
                        wrapped_z,
                        y_scale,
                        y as f64 * y_scale,
                    ) / input_factor;
                }
            }

            input_factor /= 2.0;
        }

        clamped_lerp(min / 512.0, max / 512.0, blend)
    }
}

fn floor(value: f64) -> i32 {
    value.floor() as i32
}

fn floor_i64(value: f64) -> i64 {
    value.floor() as i64
}

fn skip_octave(random: &mut impl RandomSource) {
    random.consume_count(262);
}

fn make_amplitudes(octaves: &[i32]) -> (i32, Vec<f64>) {
    let mut unique_sorted_octaves = octaves.to_vec();
    unique_sorted_octaves.sort_unstable();
    unique_sorted_octaves.dedup();

    if unique_sorted_octaves.is_empty() {
        panic!("Need some octaves!");
    }

    let first = -unique_sorted_octaves[0];
    let last = *unique_sorted_octaves.last().expect("non-empty octave set");
    let total = first + last + 1;
    if total < 1 {
        panic!("Total number of octaves needs to be >= 1");
    }

    let mut amplitudes = vec![0.0; total as usize];
    for octave in unique_sorted_octaves {
        amplitudes[(octave + first) as usize] = 1.0;
    }

    (-first, amplitudes)
}

fn to_java_long(value: f64) -> i64 {
    value as i64
}

fn grad_dot(gradient_index: i32, x_factor: f64, y_factor: f64, z_factor: f64) -> f64 {
    let gradient = GRADIENTS[(gradient_index & 15) as usize];
    (gradient[0] as f64 * x_factor)
        + (gradient[1] as f64 * y_factor)
        + (gradient[2] as f64 * z_factor)
}

fn dot(gradient_index: usize, x: f64, y: f64, z: f64) -> f64 {
    let gradient = GRADIENTS[gradient_index];
    (gradient[0] as f64 * x) + (gradient[1] as f64 * y) + (gradient[2] as f64 * z)
}

fn smoothstep(value: f64) -> f64 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}

fn lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

fn clamped_lerp(start: f64, end: f64, delta: f64) -> f64 {
    if delta < 0.0 {
        start
    } else if delta > 1.0 {
        end
    } else {
        lerp(delta, start, end)
    }
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
    use crate::prng::SimpleRandomSource;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NoiseFixture {
        module: String,
        minecraft_version: String,
        noise_class: String,
        random_source_class: String,
        random_source_alias: String,
        noise_method: String,
        grid_order: String,
        seed: String,
        sample_count: usize,
        wire_format: WireFormat,
        offsets: Offsets,
        x: Vec<f64>,
        y: Vec<f64>,
        z: Vec<f64>,
        values: Vec<f64>,
    }

    #[derive(Debug, Deserialize)]
    struct WireFormat {
        coordinates: String,
        values: String,
    }

    #[derive(Debug, Deserialize)]
    struct Offsets {
        xo: f64,
        yo: f64,
        zo: f64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SimplexFixture {
        module: String,
        minecraft_version: String,
        noise_class: String,
        random_source_class: String,
        random_source_alias: String,
        seed: String,
        wire_format: WireFormat,
        offsets: Offsets,
        samples2d: SimplexSampleSet2D,
        samples3d: SimplexSampleSet3D,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SimplexSampleSet2D {
        noise_method: String,
        grid_order: String,
        sample_count: usize,
        x: Vec<f64>,
        z: Vec<f64>,
        values: Vec<f64>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SimplexSampleSet3D {
        noise_method: String,
        grid_order: String,
        sample_count: usize,
        x: Vec<f64>,
        y: Vec<f64>,
        z: Vec<f64>,
        values: Vec<f64>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PerlinFixture {
        module: String,
        minecraft_version: String,
        noise_class: String,
        random_source_class: String,
        random_source_alias: String,
        noise_method: String,
        grid_order: String,
        seed: String,
        sample_count: usize,
        octaves: Vec<i32>,
        wire_format: WireFormat,
        x: Vec<f64>,
        y: Vec<f64>,
        z: Vec<f64>,
        values: Vec<f64>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PerlinSimplexFixture {
        module: String,
        minecraft_version: String,
        noise_class: String,
        random_source_class: String,
        random_source_alias: String,
        seed: String,
        octaves: Vec<i32>,
        wire_format: WireFormat,
        samples_without_offsets: PerlinSimplexSampleSet,
        samples_with_offsets: PerlinSimplexSampleSet,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PerlinSimplexSampleSet {
        noise_method: String,
        grid_order: String,
        sample_count: usize,
        x: Vec<f64>,
        z: Vec<f64>,
        values: Vec<f64>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BlendedFixture {
        module: String,
        minecraft_version: String,
        noise_class: String,
        random_source_class: String,
        random_source_alias: String,
        noise_method: String,
        seed: String,
        octaves: BlendedOctaves,
        wire_format: BlendedWireFormat,
        sample_sets: BlendedSampleSets,
    }

    #[derive(Debug, Deserialize)]
    struct BlendedOctaves {
        limit: Vec<i32>,
        main: Vec<i32>,
    }

    #[derive(Debug, Deserialize)]
    struct BlendedWireFormat {
        coordinates: String,
        parameters: String,
        values: String,
    }

    #[derive(Debug, Deserialize)]
    struct BlendedSampleSets {
        overworld: BlendedSampleSet,
        nether: BlendedSampleSet,
        end: BlendedSampleSet,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BlendedSampleSet {
        grid_order: String,
        sample_count: usize,
        settings_keys: Vec<String>,
        parameters: BlendedSampleParameters,
        x: Vec<i32>,
        y: Vec<i32>,
        z: Vec<i32>,
        values: Vec<f64>,
        blend_factor_range: BlendFactorRange,
        blend_region_counts: BlendRegionCounts,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BlendedSampleParameters {
        limit_horizontal_scale: f64,
        limit_vertical_scale: f64,
        main_horizontal_scale: f64,
        main_vertical_scale: f64,
    }

    #[derive(Debug, Deserialize)]
    struct BlendFactorRange {
        min: f64,
        max: f64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BlendRegionCounts {
        below_or_equal_zero: usize,
        interior: usize,
        above_or_equal_one: usize,
    }

    fn fixtures() -> Vec<NoiseFixture> {
        [
            include_str!("../../../../test/fixtures/noise/seed-0.json"),
            include_str!("../../../../test/fixtures/noise/seed-1.json"),
            include_str!("../../../../test/fixtures/noise/seed-12345.json"),
            include_str!("../../../../test/fixtures/noise/seed-2151901553968352745.json"),
        ]
        .into_iter()
        .map(|json| serde_json::from_str(json).expect("valid ImprovedNoise fixture"))
        .collect()
    }

    fn simplex_fixtures() -> Vec<SimplexFixture> {
        [
            include_str!("../../../../test/fixtures/noise/simplex-seed-0.json"),
            include_str!("../../../../test/fixtures/noise/simplex-seed-1.json"),
            include_str!("../../../../test/fixtures/noise/simplex-seed-12345.json"),
            include_str!("../../../../test/fixtures/noise/simplex-seed-2151901553968352745.json"),
        ]
        .into_iter()
        .map(|json| serde_json::from_str(json).expect("valid SimplexNoise fixture"))
        .collect()
    }

    fn perlin_fixture_sets() -> Vec<(&'static str, Vec<i32>, Vec<PerlinFixture>)> {
        vec![
            (
                "[-7..0]",
                vec![-7, -6, -5, -4, -3, -2, -1, 0],
                [
                    include_str!("../../../../test/fixtures/noise/perlin-seed-0-oct-m7-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-seed-1-oct-m7-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-seed-12345-oct-m7-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-seed-2151901553968352745-oct-m7-0.json"),
                ]
                .into_iter()
                .map(|json| serde_json::from_str(json).expect("valid PerlinNoise [-7..0] fixture"))
                .collect(),
            ),
            (
                "[-15..0]",
                vec![
                    -15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0,
                ],
                [
                    include_str!("../../../../test/fixtures/noise/perlin-seed-0-oct-m15-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-seed-1-oct-m15-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-seed-12345-oct-m15-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-seed-2151901553968352745-oct-m15-0.json"),
                ]
                .into_iter()
                .map(|json| serde_json::from_str(json).expect("valid PerlinNoise [-15..0] fixture"))
                .collect(),
            ),
        ]
    }

    fn perlin_simplex_fixture_sets() -> Vec<(&'static str, Vec<i32>, Vec<PerlinSimplexFixture>)> {
        vec![
            (
                "[-3..0]",
                vec![-3, -2, -1, 0],
                [
                    include_str!("../../../../test/fixtures/noise/perlin-simplex-seed-0-oct-m3-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-simplex-seed-1-oct-m3-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-simplex-seed-12345-oct-m3-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-simplex-seed-2151901553968352745-oct-m3-0.json"),
                ]
                .into_iter()
                .map(|json| serde_json::from_str(json).expect("valid PerlinSimplexNoise [-3..0] fixture"))
                .collect(),
            ),
            (
                "[0]",
                vec![0],
                [
                    include_str!("../../../../test/fixtures/noise/perlin-simplex-seed-0-oct-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-simplex-seed-1-oct-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-simplex-seed-12345-oct-0.json"),
                    include_str!("../../../../test/fixtures/noise/perlin-simplex-seed-2151901553968352745-oct-0.json"),
                ]
                .into_iter()
                .map(|json| serde_json::from_str(json).expect("valid PerlinSimplexNoise [0] fixture"))
                .collect(),
            ),
        ]
    }

    fn blended_fixtures() -> Vec<BlendedFixture> {
        [
            include_str!("../../../../test/fixtures/noise/blended-seed-0.json"),
            include_str!("../../../../test/fixtures/noise/blended-seed-1.json"),
            include_str!("../../../../test/fixtures/noise/blended-seed-12345.json"),
            include_str!("../../../../test/fixtures/noise/blended-seed-2151901553968352745.json"),
        ]
        .into_iter()
        .map(|json| serde_json::from_str(json).expect("valid BlendedNoise fixture"))
        .collect()
    }

    #[test]
    fn fixture_metadata_stays_consistent() {
        for fixture in fixtures() {
            assert_eq!(fixture.module, "noise");
            assert_eq!(fixture.minecraft_version, "1.17.1");
            assert_eq!(
                fixture.noise_class,
                "net.minecraft.world.level.levelgen.synth.ImprovedNoise"
            );
            assert_eq!(
                fixture.random_source_class,
                "net.minecraft.world.level.levelgen.SimpleRandomSource"
            );
            assert_eq!(fixture.random_source_alias, "LegacyRandomSource");
            assert_eq!(fixture.noise_method, "noise(x,y,z)");
            assert_eq!(fixture.grid_order, "x-major,y-major,z-minor");
            assert_eq!(fixture.wire_format.coordinates, "number");
            assert_eq!(fixture.wire_format.values, "number");
            assert_eq!(fixture.values.len(), fixture.sample_count);
            assert_eq!(
                fixture.sample_count,
                fixture.x.len() * fixture.y.len() * fixture.z.len()
            );
        }
    }

    #[test]
    fn matches_java_oracle_across_fixed_grid() {
        for fixture in fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let mut random = SimpleRandomSource::new(seed);
            let noise = ImprovedNoise::new(&mut random);

            assert_eq!(noise.xo.to_bits(), fixture.offsets.xo.to_bits());
            assert_eq!(noise.yo.to_bits(), fixture.offsets.yo.to_bits());
            assert_eq!(noise.zo.to_bits(), fixture.offsets.zo.to_bits());

            let mut index = 0;
            for x in &fixture.x {
                for y in &fixture.y {
                    for z in &fixture.z {
                        let actual = noise.get_value(*x, *y, *z);
                        let expected = fixture.values[index];
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "noise(seed={}) mismatch at index {index} for ({x}, {y}, {z}): expected {expected}, got {actual}",
                            fixture.seed
                        );
                        index += 1;
                    }
                }
            }
        }
    }

    #[test]
    fn get_value_delegates_to_default_noise_path() {
        let mut random = SimpleRandomSource::new(12_345);
        let noise = ImprovedNoise::new(&mut random);
        assert_eq!(
            noise.get_value(0.5, -0.75, 1.25).to_bits(),
            noise.noise(0.5, -0.75, 1.25).to_bits()
        );
    }

    #[test]
    fn simplex_fixture_metadata_stays_consistent() {
        for fixture in simplex_fixtures() {
            assert_eq!(fixture.module, "noise");
            assert_eq!(fixture.minecraft_version, "1.17.1");
            assert_eq!(
                fixture.noise_class,
                "net.minecraft.world.level.levelgen.synth.SimplexNoise"
            );
            assert_eq!(
                fixture.random_source_class,
                "net.minecraft.world.level.levelgen.SimpleRandomSource"
            );
            assert_eq!(fixture.random_source_alias, "LegacyRandomSource");
            assert_eq!(fixture.wire_format.coordinates, "number");
            assert_eq!(fixture.wire_format.values, "number");

            assert_eq!(fixture.samples2d.noise_method, "getValue(x,z)");
            assert_eq!(fixture.samples2d.grid_order, "x-major,z-minor");
            assert_eq!(
                fixture.samples2d.values.len(),
                fixture.samples2d.sample_count
            );
            assert_eq!(
                fixture.samples2d.sample_count,
                fixture.samples2d.x.len() * fixture.samples2d.z.len()
            );

            assert_eq!(fixture.samples3d.noise_method, "getValue(x,y,z)");
            assert_eq!(fixture.samples3d.grid_order, "x-major,y-major,z-minor");
            assert_eq!(
                fixture.samples3d.values.len(),
                fixture.samples3d.sample_count
            );
            assert_eq!(
                fixture.samples3d.sample_count,
                fixture.samples3d.x.len() * fixture.samples3d.y.len() * fixture.samples3d.z.len()
            );
        }
    }

    #[test]
    fn simplex_constructor_offsets_match_java_oracle() {
        for fixture in simplex_fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let mut random = SimpleRandomSource::new(seed);
            let noise = SimplexNoise::new(&mut random);
            assert_eq!(noise.xo.to_bits(), fixture.offsets.xo.to_bits());
            assert_eq!(noise.yo.to_bits(), fixture.offsets.yo.to_bits());
            assert_eq!(noise.zo.to_bits(), fixture.offsets.zo.to_bits());
        }
    }

    #[test]
    fn simplex_matches_java_oracle_on_2d_sample_grid() {
        for fixture in simplex_fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let mut random = SimpleRandomSource::new(seed);
            let noise = SimplexNoise::new(&mut random);
            let mut index = 0;

            for x in &fixture.samples2d.x {
                for z in &fixture.samples2d.z {
                    let actual = noise.get_value_2d(*x, *z);
                    let expected = fixture.samples2d.values[index];
                    assert_eq!(
                        actual.to_bits(),
                        expected.to_bits(),
                        "simplex(seed={}, dimensions=2d) mismatch at index {index} for ({x}, {z}): expected {expected}, got {actual}",
                        fixture.seed
                    );
                    index += 1;
                }
            }
        }
    }

    #[test]
    fn simplex_matches_java_oracle_on_3d_sample_grid() {
        for fixture in simplex_fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let mut random = SimpleRandomSource::new(seed);
            let noise = SimplexNoise::new(&mut random);
            let mut index = 0;

            for x in &fixture.samples3d.x {
                for y in &fixture.samples3d.y {
                    for z in &fixture.samples3d.z {
                        let actual = noise.get_value_3d(*x, *y, *z);
                        let expected = fixture.samples3d.values[index];
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "simplex(seed={}, dimensions=3d) mismatch at index {index} for ({x}, {y}, {z}): expected {expected}, got {actual}",
                            fixture.seed
                        );
                        index += 1;
                    }
                }
            }
        }
    }

    #[test]
    fn perlin_fixture_metadata_stays_consistent() {
        for (_range_label, expected_octaves, fixtures) in perlin_fixture_sets() {
            for fixture in fixtures {
                assert_eq!(fixture.module, "noise");
                assert_eq!(fixture.minecraft_version, "1.17.1");
                assert_eq!(
                    fixture.noise_class,
                    "net.minecraft.world.level.levelgen.synth.PerlinNoise"
                );
                assert_eq!(
                    fixture.random_source_class,
                    "net.minecraft.world.level.levelgen.SimpleRandomSource"
                );
                assert_eq!(fixture.random_source_alias, "LegacyRandomSource");
                assert_eq!(fixture.noise_method, "getValue(x,y,z)");
                assert_eq!(fixture.grid_order, "x-major,y-major,z-minor");
                assert_eq!(fixture.octaves, expected_octaves);
                assert_eq!(fixture.wire_format.coordinates, "number");
                assert_eq!(fixture.wire_format.values, "number");
                assert_eq!(fixture.values.len(), fixture.sample_count);
                assert_eq!(
                    fixture.sample_count,
                    fixture.x.len() * fixture.y.len() * fixture.z.len()
                );
            }
        }
    }

    #[test]
    fn perlin_matches_java_oracle_across_shared_sample_grid() {
        for (range_label, _expected_octaves, fixtures) in perlin_fixture_sets() {
            for fixture in fixtures {
                let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
                let mut random = SimpleRandomSource::new(seed);
                let noise = PerlinNoise::from_octaves(&mut random, &fixture.octaves);

                let mut index = 0;
                for x in &fixture.x {
                    for y in &fixture.y {
                        for z in &fixture.z {
                            let actual = noise.get_value(*x, *y, *z);
                            let expected = fixture.values[index];
                            assert_eq!(
                                actual.to_bits(),
                                expected.to_bits(),
                                "perlin(seed={}, octaves={range_label}) mismatch at index {index} for ({x}, {y}, {z}): expected {expected}, got {actual}",
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
    fn perlin_default_get_value_delegates_to_explicit_default_arguments() {
        let mut random = SimpleRandomSource::new(12_345);
        let noise = PerlinNoise::from_octaves(&mut random, &[-7, -6, -5, -4, -3, -2, -1, 0]);
        assert_eq!(
            noise.get_value(0.5, -0.75, 1.25).to_bits(),
            noise
                .get_value_scaled(0.5, -0.75, 1.25, 0.0, 0.0, false)
                .to_bits()
        );
    }

    #[test]
    fn perlin_surface_noise_value_preserves_java_argument_remapping() {
        let mut random = SimpleRandomSource::new(12_345);
        let noise = PerlinNoise::from_octaves(&mut random, &[-7, -6, -5, -4, -3, -2, -1, 0]);
        assert_eq!(
            noise
                .get_surface_noise_value(0.5, -0.75, 1.25, 2.5)
                .to_bits(),
            noise
                .get_value_scaled(0.5, -0.75, 0.0, 1.25, 2.5, false)
                .to_bits()
        );
    }

    #[test]
    fn perlin_create_uses_explicit_amplitudes_instead_of_octave_presence() {
        let mut explicit_random = SimpleRandomSource::new(12_345);
        let explicit = PerlinNoise::create(&mut explicit_random, -3, &[1.0, 0.0, 2.0]);
        let mut manual_random = SimpleRandomSource::new(12_345);
        let manual = PerlinNoise::create(&mut manual_random, -3, &[1.0, 0.0, 2.0]);

        assert_eq!(
            explicit.get_value(0.25, -0.5, 0.75).to_bits(),
            manual.get_value(0.25, -0.5, 0.75).to_bits()
        );
    }

    #[test]
    fn perlin_simplex_fixture_metadata_stays_consistent() {
        for (_range_label, expected_octaves, fixtures) in perlin_simplex_fixture_sets() {
            for fixture in fixtures {
                assert_eq!(fixture.module, "noise");
                assert_eq!(fixture.minecraft_version, "1.17.1");
                assert_eq!(
                    fixture.noise_class,
                    "net.minecraft.world.level.levelgen.synth.PerlinSimplexNoise"
                );
                assert_eq!(
                    fixture.random_source_class,
                    "net.minecraft.world.level.levelgen.SimpleRandomSource"
                );
                assert_eq!(fixture.random_source_alias, "LegacyRandomSource");
                assert_eq!(fixture.octaves, expected_octaves);
                assert_eq!(fixture.wire_format.coordinates, "number");
                assert_eq!(fixture.wire_format.values, "number");

                for sample_set in [
                    &fixture.samples_without_offsets,
                    &fixture.samples_with_offsets,
                ] {
                    assert_eq!(sample_set.grid_order, "x-major,z-minor");
                    assert_eq!(sample_set.values.len(), sample_set.sample_count);
                    assert_eq!(
                        sample_set.sample_count,
                        sample_set.x.len() * sample_set.z.len()
                    );
                }

                assert_eq!(
                    fixture.samples_without_offsets.noise_method,
                    "getValue(x,y,false)"
                );
                assert_eq!(
                    fixture.samples_with_offsets.noise_method,
                    "getValue(x,y,true)"
                );
            }
        }
    }

    #[test]
    fn perlin_simplex_matches_java_oracle_without_offsets() {
        for (range_label, _expected_octaves, fixtures) in perlin_simplex_fixture_sets() {
            for fixture in fixtures {
                let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
                let mut random = SimpleRandomSource::new(seed);
                let noise = PerlinSimplexNoise::from_octaves(&mut random, &fixture.octaves);
                let mut index = 0;

                for x in &fixture.samples_without_offsets.x {
                    for z in &fixture.samples_without_offsets.z {
                        let actual = noise.get_value(*x, *z, false);
                        let expected = fixture.samples_without_offsets.values[index];
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "perlin-simplex(seed={}, octaves={range_label}, useOffsets=false) mismatch at index {index} for ({x}, {z}): expected {expected}, got {actual}",
                            fixture.seed
                        );
                        index += 1;
                    }
                }
            }
        }
    }

    #[test]
    fn perlin_simplex_matches_java_oracle_with_offsets() {
        for (range_label, _expected_octaves, fixtures) in perlin_simplex_fixture_sets() {
            for fixture in fixtures {
                let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
                let mut random = SimpleRandomSource::new(seed);
                let noise = PerlinSimplexNoise::from_octaves(&mut random, &fixture.octaves);
                let mut index = 0;

                for x in &fixture.samples_with_offsets.x {
                    for z in &fixture.samples_with_offsets.z {
                        let actual = noise.get_value(*x, *z, true);
                        let expected = fixture.samples_with_offsets.values[index];
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "perlin-simplex(seed={}, octaves={range_label}, useOffsets=true) mismatch at index {index} for ({x}, {z}): expected {expected}, got {actual}",
                            fixture.seed
                        );
                        index += 1;
                    }
                }
            }
        }
    }

    #[test]
    fn perlin_simplex_surface_noise_value_uses_offsets_and_java_scale_factor() {
        let mut random = SimpleRandomSource::new(12_345);
        let noise = PerlinSimplexNoise::from_octaves(&mut random, &[-3, -2, -1, 0]);
        assert_eq!(
            noise
                .get_surface_noise_value(0.5, -0.75, 123.0, 456.0)
                .to_bits(),
            (noise.get_value(0.5, -0.75, true) * 0.55).to_bits()
        );
    }

    #[test]
    fn blended_fixture_metadata_stays_consistent() {
        for fixture in blended_fixtures() {
            assert_eq!(fixture.module, "noise");
            assert_eq!(fixture.minecraft_version, "1.17.1");
            assert_eq!(
                fixture.noise_class,
                "net.minecraft.world.level.levelgen.synth.BlendedNoise"
            );
            assert_eq!(
                fixture.random_source_class,
                "net.minecraft.world.level.levelgen.SimpleRandomSource"
            );
            assert_eq!(fixture.random_source_alias, "LegacyRandomSource");
            assert_eq!(
                fixture.noise_method,
                "sampleAndClampNoise(x,y,z,limitHorizontalScale,limitVerticalScale,mainHorizontalScale,mainVerticalScale)"
            );
            assert_eq!(fixture.octaves.limit, LIMIT_OCTAVES);
            assert_eq!(fixture.octaves.main, MAIN_OCTAVES);
            assert_eq!(fixture.wire_format.coordinates, "integer");
            assert_eq!(fixture.wire_format.parameters, "number");
            assert_eq!(fixture.wire_format.values, "number");

            for (preset, expected_settings_keys, sample_set) in [
                (
                    "overworld",
                    &["overworld", "amplified"][..],
                    &fixture.sample_sets.overworld,
                ),
                (
                    "nether",
                    &["nether", "caves"][..],
                    &fixture.sample_sets.nether,
                ),
                (
                    "end",
                    &["end", "floating_islands"][..],
                    &fixture.sample_sets.end,
                ),
            ] {
                assert_eq!(sample_set.grid_order, "x-major,y-major,z-minor");
                assert_eq!(
                    sample_set.settings_keys, expected_settings_keys,
                    "{preset} settings keys drifted"
                );
                assert_eq!(sample_set.values.len(), sample_set.sample_count);
                assert_eq!(
                    sample_set.sample_count,
                    sample_set.x.len() * sample_set.y.len() * sample_set.z.len()
                );

                let region_count = sample_set.blend_region_counts.below_or_equal_zero
                    + sample_set.blend_region_counts.interior
                    + sample_set.blend_region_counts.above_or_equal_one;
                assert_eq!(region_count, sample_set.sample_count);
                assert!(sample_set.blend_factor_range.min <= 0.0);
                assert!(sample_set.blend_factor_range.max >= 1.0);
                assert!(sample_set.blend_region_counts.below_or_equal_zero > 0);
                assert!(sample_set.blend_region_counts.interior > 0);
                assert!(sample_set.blend_region_counts.above_or_equal_one > 0);
            }
        }
    }

    #[test]
    fn blended_matches_java_oracle_across_shared_cell_grid() {
        for fixture in blended_fixtures() {
            let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
            let mut random = SimpleRandomSource::new(seed);
            let noise = BlendedNoise::new(&mut random);

            for (preset, sample_set) in [
                ("overworld", &fixture.sample_sets.overworld),
                ("nether", &fixture.sample_sets.nether),
                ("end", &fixture.sample_sets.end),
            ] {
                let mut index = 0;
                for x in &sample_set.x {
                    for y in &sample_set.y {
                        for z in &sample_set.z {
                            let actual = noise.sample_and_clamp_noise(
                                *x,
                                *y,
                                *z,
                                sample_set.parameters.limit_horizontal_scale,
                                sample_set.parameters.limit_vertical_scale,
                                sample_set.parameters.main_horizontal_scale,
                                sample_set.parameters.main_vertical_scale,
                            );
                            let expected = sample_set.values[index];
                            assert_eq!(
                                actual.to_bits(),
                                expected.to_bits(),
                                "blended(seed={}, preset={preset}) mismatch at index {index} for ({x}, {y}, {z}): expected {expected}, got {actual}",
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
    fn blended_random_source_construction_matches_explicitly_chained_perlin_noises() {
        let seed = 12_345;
        let mut direct_random = SimpleRandomSource::new(seed);
        let direct = BlendedNoise::new(&mut direct_random);

        let mut explicit_random = SimpleRandomSource::new(seed);
        let explicit = BlendedNoise::from_noises(
            PerlinNoise::from_octaves(&mut explicit_random, &LIMIT_OCTAVES),
            PerlinNoise::from_octaves(&mut explicit_random, &LIMIT_OCTAVES),
            PerlinNoise::from_octaves(&mut explicit_random, &MAIN_OCTAVES),
        );

        assert_eq!(
            direct
                .sample_and_clamp_noise(4, -2, 8, 684.412, 684.412, 684.412 / 80.0, 684.412 / 160.0)
                .to_bits(),
            explicit
                .sample_and_clamp_noise(4, -2, 8, 684.412, 684.412, 684.412 / 80.0, 684.412 / 160.0)
                .to_bits()
        );
    }
}
