use crate::prng::SimpleRandomSource;

const SHIFT_UP_EPSILON: f64 = 1.0e-7_f32 as f64;
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
    pub fn new(random: &mut SimpleRandomSource) -> Self {
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

fn floor(value: f64) -> i32 {
    value.floor() as i32
}

fn grad_dot(gradient_index: i32, x_factor: f64, y_factor: f64, z_factor: f64) -> f64 {
    let gradient = GRADIENTS[(gradient_index & 15) as usize];
    (gradient[0] as f64 * x_factor)
        + (gradient[1] as f64 * y_factor)
        + (gradient[2] as f64 * z_factor)
}

fn smoothstep(value: f64) -> f64 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
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
}
