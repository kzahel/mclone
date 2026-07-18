use crate::prng::SimpleRandomSource;

#[derive(Clone, Debug)]
pub(super) struct BetaPerlinNoise {
    levels: Vec<BetaImprovedNoise>,
}

impl BetaPerlinNoise {
    pub(super) fn new(random: &mut SimpleRandomSource, levels: usize) -> Self {
        Self {
            levels: (0..levels)
                .map(|_| BetaImprovedNoise::new(random))
                .collect(),
        }
    }

    #[allow(dead_code)]
    pub(super) fn value_2d(&self, x: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut octave_scale = 1.0;
        for level in &self.levels {
            value += level.noise(x * octave_scale, z * octave_scale, 0.0) / octave_scale;
            octave_scale /= 2.0;
        }
        value
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn region(
        &self,
        x: f64,
        y: f64,
        z: f64,
        size_x: usize,
        size_y: usize,
        size_z: usize,
        scale_x: f64,
        scale_y: f64,
        scale_z: f64,
    ) -> Vec<f64> {
        let mut values = vec![0.0; size_x * size_y * size_z];
        let mut octave_scale = 1.0;
        for level in &self.levels {
            level.add(
                &mut values,
                x,
                y,
                z,
                size_x,
                size_y,
                size_z,
                scale_x * octave_scale,
                scale_y * octave_scale,
                scale_z * octave_scale,
                octave_scale,
            );
            octave_scale /= 2.0;
        }
        values
    }

    pub(super) fn region_2d(
        &self,
        x: i32,
        z: i32,
        size_x: usize,
        size_z: usize,
        scale_x: f64,
        scale_z: f64,
    ) -> Vec<f64> {
        self.region(
            f64::from(x),
            10.0,
            f64::from(z),
            size_x,
            1,
            size_z,
            scale_x,
            1.0,
            scale_z,
        )
    }
}

#[derive(Clone, Debug)]
struct BetaImprovedNoise {
    permutations: [usize; 512],
    offset_x: f64,
    offset_y: f64,
    offset_z: f64,
}

impl BetaImprovedNoise {
    fn new(random: &mut SimpleRandomSource) -> Self {
        let offset_x = random.next_double() * 256.0;
        let offset_y = random.next_double() * 256.0;
        let offset_z = random.next_double() * 256.0;
        let mut permutations = [0; 512];
        for (index, value) in permutations[..256].iter_mut().enumerate() {
            *value = index;
        }
        for index in 0..256 {
            let other = index + random.next_int_bound((256 - index) as i32) as usize;
            permutations.swap(index, other);
            permutations[index + 256] = permutations[index];
        }
        Self {
            permutations,
            offset_x,
            offset_y,
            offset_z,
        }
    }

    #[allow(dead_code)]
    fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut delta_x = x + self.offset_x;
        let mut delta_y = y + self.offset_y;
        let mut delta_z = z + self.offset_z;
        let grid_x = beta_floor(delta_x);
        let grid_y = beta_floor(delta_y);
        let grid_z = beta_floor(delta_z);
        let perm_x = (grid_x & 0xff) as usize;
        let perm_y = (grid_y & 0xff) as usize;
        let perm_z = (grid_z & 0xff) as usize;
        delta_x -= f64::from(grid_x);
        delta_y -= f64::from(grid_y);
        delta_z -= f64::from(grid_z);
        let fade_x = beta_fade(delta_x);
        let fade_y = beta_fade(delta_y);
        let fade_z = beta_fade(delta_z);
        let x0 = self.permutations[perm_x] + perm_y;
        let x0_y0 = self.permutations[x0] + perm_z;
        let x0_y1 = self.permutations[x0 + 1] + perm_z;
        let x1 = self.permutations[perm_x + 1] + perm_y;
        let x1_y0 = self.permutations[x1] + perm_z;
        let x1_y1 = self.permutations[x1 + 1] + perm_z;
        beta_lerp(
            fade_z,
            beta_lerp(
                fade_y,
                beta_lerp(
                    fade_x,
                    beta_gradient_3d(self.permutations[x0_y0], delta_x, delta_y, delta_z),
                    beta_gradient_3d(self.permutations[x1_y0], delta_x - 1.0, delta_y, delta_z),
                ),
                beta_lerp(
                    fade_x,
                    beta_gradient_3d(self.permutations[x0_y1], delta_x, delta_y - 1.0, delta_z),
                    beta_gradient_3d(
                        self.permutations[x1_y1],
                        delta_x - 1.0,
                        delta_y - 1.0,
                        delta_z,
                    ),
                ),
            ),
            beta_lerp(
                fade_y,
                beta_lerp(
                    fade_x,
                    beta_gradient_3d(
                        self.permutations[x0_y0 + 1],
                        delta_x,
                        delta_y,
                        delta_z - 1.0,
                    ),
                    beta_gradient_3d(
                        self.permutations[x1_y0 + 1],
                        delta_x - 1.0,
                        delta_y,
                        delta_z - 1.0,
                    ),
                ),
                beta_lerp(
                    fade_x,
                    beta_gradient_3d(
                        self.permutations[x0_y1 + 1],
                        delta_x,
                        delta_y - 1.0,
                        delta_z - 1.0,
                    ),
                    beta_gradient_3d(
                        self.permutations[x1_y1 + 1],
                        delta_x - 1.0,
                        delta_y - 1.0,
                        delta_z - 1.0,
                    ),
                ),
            ),
        )
    }

    #[allow(clippy::too_many_arguments, unused_assignments)]
    fn add(
        &self,
        values: &mut [f64],
        x: f64,
        y: f64,
        z: f64,
        size_x: usize,
        size_y: usize,
        size_z: usize,
        scale_x: f64,
        scale_y: f64,
        scale_z: f64,
        noise_scale: f64,
    ) {
        if size_y == 1 {
            self.add_2d(values, x, z, size_x, size_z, scale_x, scale_z, noise_scale);
            return;
        }

        let mut index = 0;
        let amplitude = 1.0 / noise_scale;
        let mut previous_grid_y = -1;
        let mut x0_y0 = 0;
        let mut x0_y1 = 0;
        let mut x1_y0 = 0;
        let mut x1_y1 = 0;
        let mut lower_x0 = 0.0;
        let mut lower_x1 = 0.0;
        let mut upper_x0 = 0.0;
        let mut upper_x1 = 0.0;

        for local_x in 0..size_x {
            let mut delta_x = (x + local_x as f64) * scale_x + self.offset_x;
            let grid_x = beta_floor(delta_x);
            let perm_x = (grid_x & 0xff) as usize;
            delta_x -= f64::from(grid_x);
            let fade_x = beta_fade(delta_x);
            for local_z in 0..size_z {
                let mut delta_z = (z + local_z as f64) * scale_z + self.offset_z;
                let grid_z = beta_floor(delta_z);
                let perm_z = (grid_z & 0xff) as usize;
                delta_z -= f64::from(grid_z);
                let fade_z = beta_fade(delta_z);
                for local_y in 0..size_y {
                    let mut delta_y = (y + local_y as f64) * scale_y + self.offset_y;
                    let grid_y = beta_floor(delta_y);
                    let perm_y = (grid_y & 0xff) as usize;
                    delta_y -= f64::from(grid_y);
                    let fade_y = beta_fade(delta_y);
                    if local_y == 0 || perm_y as i32 != previous_grid_y {
                        previous_grid_y = perm_y as i32;
                        let p_x0 = self.permutations[perm_x] + perm_y;
                        x0_y0 = self.permutations[p_x0] + perm_z;
                        x0_y1 = self.permutations[p_x0 + 1] + perm_z;
                        let p_x1 = self.permutations[perm_x + 1] + perm_y;
                        x1_y0 = self.permutations[p_x1] + perm_z;
                        x1_y1 = self.permutations[p_x1 + 1] + perm_z;
                        lower_x0 = beta_lerp(
                            fade_x,
                            beta_gradient_3d(self.permutations[x0_y0], delta_x, delta_y, delta_z),
                            beta_gradient_3d(
                                self.permutations[x1_y0],
                                delta_x - 1.0,
                                delta_y,
                                delta_z,
                            ),
                        );
                        lower_x1 = beta_lerp(
                            fade_x,
                            beta_gradient_3d(
                                self.permutations[x0_y1],
                                delta_x,
                                delta_y - 1.0,
                                delta_z,
                            ),
                            beta_gradient_3d(
                                self.permutations[x1_y1],
                                delta_x - 1.0,
                                delta_y - 1.0,
                                delta_z,
                            ),
                        );
                        upper_x0 = beta_lerp(
                            fade_x,
                            beta_gradient_3d(
                                self.permutations[x0_y0 + 1],
                                delta_x,
                                delta_y,
                                delta_z - 1.0,
                            ),
                            beta_gradient_3d(
                                self.permutations[x1_y0 + 1],
                                delta_x - 1.0,
                                delta_y,
                                delta_z - 1.0,
                            ),
                        );
                        upper_x1 = beta_lerp(
                            fade_x,
                            beta_gradient_3d(
                                self.permutations[x0_y1 + 1],
                                delta_x,
                                delta_y - 1.0,
                                delta_z - 1.0,
                            ),
                            beta_gradient_3d(
                                self.permutations[x1_y1 + 1],
                                delta_x - 1.0,
                                delta_y - 1.0,
                                delta_z - 1.0,
                            ),
                        );
                    }
                    let lower = beta_lerp(fade_y, lower_x0, lower_x1);
                    let upper = beta_lerp(fade_y, upper_x0, upper_x1);
                    values[index] += beta_lerp(fade_z, lower, upper) * amplitude;
                    index += 1;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_2d(
        &self,
        values: &mut [f64],
        x: f64,
        z: f64,
        size_x: usize,
        size_z: usize,
        scale_x: f64,
        scale_z: f64,
        noise_scale: f64,
    ) {
        let mut index = 0;
        let amplitude = 1.0 / noise_scale;
        for local_x in 0..size_x {
            let mut delta_x = (x + local_x as f64) * scale_x + self.offset_x;
            let grid_x = beta_floor(delta_x);
            let perm_x = (grid_x & 0xff) as usize;
            delta_x -= f64::from(grid_x);
            let fade_x = beta_fade(delta_x);
            for local_z in 0..size_z {
                let mut delta_z = (z + local_z as f64) * scale_z + self.offset_z;
                let grid_z = beta_floor(delta_z);
                let perm_z = (grid_z & 0xff) as usize;
                delta_z -= f64::from(grid_z);
                let fade_z = beta_fade(delta_z);
                let x0 = self.permutations[perm_x];
                let x0_z0 = self.permutations[x0] + perm_z;
                let x1 = self.permutations[perm_x + 1];
                let x1_z0 = self.permutations[x1] + perm_z;
                let lower = beta_lerp(
                    fade_x,
                    beta_gradient_2d(self.permutations[x0_z0], delta_x, delta_z),
                    beta_gradient_3d(self.permutations[x1_z0], delta_x - 1.0, 0.0, delta_z),
                );
                let upper = beta_lerp(
                    fade_x,
                    beta_gradient_3d(self.permutations[x0_z0 + 1], delta_x, 0.0, delta_z - 1.0),
                    beta_gradient_3d(
                        self.permutations[x1_z0 + 1],
                        delta_x - 1.0,
                        0.0,
                        delta_z - 1.0,
                    ),
                );
                values[index] += beta_lerp(fade_z, lower, upper) * amplitude;
                index += 1;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct BetaPerlinSimplexNoise {
    levels: Vec<BetaSimplexNoise>,
}

impl BetaPerlinSimplexNoise {
    pub(super) fn new(random: &mut SimpleRandomSource, levels: usize) -> Self {
        Self {
            levels: (0..levels).map(|_| BetaSimplexNoise::new(random)).collect(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn region(
        &self,
        x: f64,
        z: f64,
        size_x: usize,
        size_z: usize,
        mut scale_x: f64,
        mut scale_z: f64,
        scale_exponent_x: f64,
        scale_exponent_z: f64,
    ) -> Vec<f64> {
        scale_x /= 1.5;
        scale_z /= 1.5;
        let mut values = vec![0.0; size_x * size_z];
        let mut amplitude_divisor = 1.0;
        let mut frequency = 1.0;
        for level in &self.levels {
            level.add(
                &mut values,
                x,
                z,
                size_x,
                size_z,
                scale_x * frequency,
                scale_z * frequency,
                0.55 / amplitude_divisor,
            );
            frequency *= scale_exponent_x;
            amplitude_divisor *= scale_exponent_z;
        }
        values
    }
}

#[derive(Clone, Debug)]
struct BetaSimplexNoise {
    permutations: [usize; 512],
    offset_x: f64,
    offset_y: f64,
    #[allow(dead_code)]
    offset_z: f64,
}

impl BetaSimplexNoise {
    fn new(random: &mut SimpleRandomSource) -> Self {
        let offset_x = random.next_double() * 256.0;
        let offset_y = random.next_double() * 256.0;
        let offset_z = random.next_double() * 256.0;
        let mut permutations = [0; 512];
        for (index, value) in permutations[..256].iter_mut().enumerate() {
            *value = index;
        }
        for index in 0..256 {
            let other = index + random.next_int_bound((256 - index) as i32) as usize;
            permutations.swap(index, other);
            permutations[index + 256] = permutations[index];
        }
        Self {
            permutations,
            offset_x,
            offset_y,
            offset_z,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        values: &mut [f64],
        x: f64,
        z: f64,
        size_x: usize,
        size_z: usize,
        scale_x: f64,
        scale_z: f64,
        noise_scale: f64,
    ) {
        const GRADIENTS: [[i32; 3]; 12] = [
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
        ];
        let f2 = 0.5 * (3.0_f64.sqrt() - 1.0);
        let g2 = (3.0 - 3.0_f64.sqrt()) / 6.0;
        let mut index = 0;
        for local_x in 0..size_x {
            let sample_x = (x + local_x as f64) * scale_x + self.offset_x;
            for local_z in 0..size_z {
                let sample_z = (z + local_z as f64) * scale_z + self.offset_y;
                let skew = (sample_x + sample_z) * f2;
                let lattice_x = beta_simplex_floor(sample_x + skew);
                let lattice_z = beta_simplex_floor(sample_z + skew);
                let unskew = f64::from(lattice_x + lattice_z) * g2;
                let origin_x = f64::from(lattice_x) - unskew;
                let origin_z = f64::from(lattice_z) - unskew;
                let delta_x = sample_x - origin_x;
                let delta_z = sample_z - origin_z;
                let (middle_x, middle_z) = if delta_x > delta_z { (1, 0) } else { (0, 1) };
                let middle_delta_x = delta_x - f64::from(middle_x) + g2;
                let middle_delta_z = delta_z - f64::from(middle_z) + g2;
                let far_delta_x = delta_x - 1.0 + 2.0 * g2;
                let far_delta_z = delta_z - 1.0 + 2.0 * g2;
                let perm_x = (lattice_x & 0xff) as usize;
                let perm_z = (lattice_z & 0xff) as usize;
                let gradient0 = self.permutations[perm_x + self.permutations[perm_z]] % 12;
                let gradient1 = self.permutations
                    [perm_x + middle_x as usize + self.permutations[perm_z + middle_z as usize]]
                    % 12;
                let gradient2 = self.permutations[perm_x + 1 + self.permutations[perm_z + 1]] % 12;
                let near = simplex_corner(GRADIENTS[gradient0], delta_x, delta_z);
                let middle = simplex_corner(GRADIENTS[gradient1], middle_delta_x, middle_delta_z);
                let far = simplex_corner(GRADIENTS[gradient2], far_delta_x, far_delta_z);
                values[index] += 70.0 * (near + middle + far) * noise_scale;
                index += 1;
            }
        }
    }
}

fn simplex_corner(gradient: [i32; 3], x: f64, z: f64) -> f64 {
    let mut attenuation = 0.5 - x * x - z * z;
    if attenuation < 0.0 {
        return 0.0;
    }
    attenuation *= attenuation;
    attenuation * attenuation * (f64::from(gradient[0]) * x + f64::from(gradient[1]) * z)
}

fn beta_simplex_floor(value: f64) -> i32 {
    if value > 0.0 {
        value as i32
    } else {
        value as i32 - 1
    }
}

fn beta_floor(value: f64) -> i32 {
    value.floor() as i32
}

fn beta_fade(value: f64) -> f64 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}

fn beta_lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

fn beta_gradient_2d(hash: usize, x: f64, z: f64) -> f64 {
    let masked = hash & 15;
    let first = f64::from(1 - ((masked & 8) >> 3) as i32) * x;
    let second = if masked < 4 {
        0.0
    } else if masked != 12 && masked != 14 {
        z
    } else {
        x
    };
    (if masked & 1 == 0 { first } else { -first }) + if masked & 2 == 0 { second } else { -second }
}

fn beta_gradient_3d(hash: usize, x: f64, y: f64, z: f64) -> f64 {
    let masked = hash & 15;
    let first = if masked < 8 { x } else { y };
    let second = if masked < 4 {
        y
    } else if masked != 12 && masked != 14 {
        z
    } else {
        x
    };
    (if masked & 1 == 0 { first } else { -first }) + if masked & 2 == 0 { second } else { -second }
}
