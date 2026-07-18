use std::sync::OnceLock;

use crate::block::{AIR, DIRT, GRASS_BLOCK, LAVA, STONE, WATER};
use crate::prng::SimpleRandomSource;

use super::{BETA_ACTIVE_HEIGHT, MutableChunkBlockBuffer};

pub(super) fn carve_beta_caves(seed: i64, chunk: &mut MutableChunkBlockBuffer) {
    let range = 8_i32;
    let mut random = SimpleRandomSource::new(seed);
    let multiplier_x = odd_multiplier(random.next_long());
    let multiplier_z = odd_multiplier(random.next_long());
    let mut carver = BetaCaveCarver::new(seed);

    for source_x in chunk.chunk_x - range..=chunk.chunk_x + range {
        for source_z in chunk.chunk_z - range..=chunk.chunk_z + range {
            let source_seed = i64::from(source_x)
                .wrapping_mul(multiplier_x)
                .wrapping_add(i64::from(source_z).wrapping_mul(multiplier_z))
                ^ seed;
            carver.random.set_seed(source_seed);
            carver.carve_source(source_x, source_z, chunk);
        }
    }
}

fn odd_multiplier(value: i64) -> i64 {
    value.wrapping_div(2).wrapping_mul(2).wrapping_add(1)
}

struct BetaCaveCarver {
    random: SimpleRandomSource,
}

impl BetaCaveCarver {
    fn new(seed: i64) -> Self {
        Self {
            random: SimpleRandomSource::new(seed),
        }
    }

    fn carve_source(
        &mut self,
        source_chunk_x: i32,
        source_chunk_z: i32,
        target: &mut MutableChunkBlockBuffer,
    ) {
        let inner = self.random.next_int_bound(40) + 1;
        let middle = self.random.next_int_bound(inner) + 1;
        let mut cave_count = self.random.next_int_bound(middle);
        if self.random.next_int_bound(15) != 0 {
            cave_count = 0;
        }

        for _ in 0..cave_count {
            let x = f64::from(source_chunk_x * 16 + self.random.next_int_bound(16));
            let y_bound = self.random.next_int_bound(120) + 8;
            let y = f64::from(self.random.next_int_bound(y_bound));
            let z = f64::from(source_chunk_z * 16 + self.random.next_int_bound(16));
            let mut tunnel_count = 1;
            if self.random.next_int_bound(4) == 0 {
                let width = 1.0 + self.random.next_float() * 6.0;
                self.carve_tunnel(target, x, y, z, width, 0.0, 0.0, -1, -1, 0.5);
                tunnel_count += self.random.next_int_bound(4);
            }

            for _ in 0..tunnel_count {
                let yaw = self.random.next_float() * std::f32::consts::PI * 2.0;
                let pitch = (self.random.next_float() - 0.5) * 2.0 / 8.0;
                let width = self.random.next_float() * 2.0 + self.random.next_float();
                self.carve_tunnel(target, x, y, z, width, yaw, pitch, 0, 0, 1.0);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn carve_tunnel(
        &mut self,
        target: &mut MutableChunkBlockBuffer,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        base_width: f32,
        mut yaw: f32,
        mut pitch: f32,
        mut tunnel: i32,
        mut tunnel_count: i32,
        width_height_ratio: f64,
    ) {
        let target_center_x = f64::from(target.chunk_x * 16 + 8);
        let target_center_z = f64::from(target.chunk_z * 16 + 8);
        let mut yaw_velocity = 0.0_f32;
        let mut pitch_velocity = 0.0_f32;
        let mut random = SimpleRandomSource::new(self.random.next_long());

        if tunnel_count <= 0 {
            let span = 8 * 16 - 16;
            tunnel_count = span - random.next_int_bound(span / 4);
        }

        let mut room = false;
        if tunnel == -1 {
            tunnel = tunnel_count / 2;
            room = true;
        }

        let branch_at = random.next_int_bound(tunnel_count / 2) + tunnel_count / 4;
        let gentle_pitch = random.next_int_bound(6) == 0;

        while tunnel < tunnel_count {
            let horizontal_radius = 1.5
                + f64::from(
                    beta_sin(tunnel as f32 * std::f32::consts::PI / tunnel_count as f32)
                        * base_width,
                );
            let vertical_radius = horizontal_radius * width_height_ratio;
            let horizontal_pitch = beta_cos(pitch);
            let vertical_pitch = beta_sin(pitch);
            x += f64::from(beta_cos(yaw) * horizontal_pitch);
            y += f64::from(vertical_pitch);
            z += f64::from(beta_sin(yaw) * horizontal_pitch);
            pitch *= if gentle_pitch { 0.92 } else { 0.7 };
            pitch += pitch_velocity * 0.1;
            yaw += yaw_velocity * 0.1;
            pitch_velocity *= 0.9;
            yaw_velocity *= 0.75;
            pitch_velocity +=
                (random.next_float() - random.next_float()) * random.next_float() * 2.0;
            yaw_velocity += (random.next_float() - random.next_float()) * random.next_float() * 4.0;

            if !room && tunnel == branch_at && base_width > 1.0 {
                let left_width = random.next_float() * 0.5 + 0.5;
                self.carve_tunnel(
                    target,
                    x,
                    y,
                    z,
                    left_width,
                    yaw - std::f32::consts::FRAC_PI_2,
                    pitch / 3.0,
                    tunnel,
                    tunnel_count,
                    1.0,
                );
                let right_width = random.next_float() * 0.5 + 0.5;
                self.carve_tunnel(
                    target,
                    x,
                    y,
                    z,
                    right_width,
                    yaw + std::f32::consts::FRAC_PI_2,
                    pitch / 3.0,
                    tunnel,
                    tunnel_count,
                    1.0,
                );
                return;
            }

            if room || random.next_int_bound(4) != 0 {
                let distance_x = x - target_center_x;
                let distance_z = z - target_center_z;
                let remaining = f64::from(tunnel_count - tunnel);
                let reach = f64::from(base_width + 18.0);
                if distance_x * distance_x + distance_z * distance_z - remaining * remaining
                    > reach * reach
                {
                    return;
                }

                if x >= target_center_x - 16.0 - horizontal_radius * 2.0
                    && z >= target_center_z - 16.0 - horizontal_radius * 2.0
                    && x <= target_center_x + 16.0 + horizontal_radius * 2.0
                    && z <= target_center_z + 16.0 + horizontal_radius * 2.0
                {
                    let mut min_x = beta_floor(x - horizontal_radius) - target.chunk_x * 16 - 1;
                    let mut max_x = beta_floor(x + horizontal_radius) - target.chunk_x * 16 + 1;
                    let mut min_y = beta_floor(y - vertical_radius) - 1;
                    let mut max_y = beta_floor(y + vertical_radius) + 1;
                    let mut min_z = beta_floor(z - horizontal_radius) - target.chunk_z * 16 - 1;
                    let mut max_z = beta_floor(z + horizontal_radius) - target.chunk_z * 16 + 1;
                    min_x = min_x.max(0);
                    max_x = max_x.min(16);
                    min_y = min_y.max(1);
                    max_y = max_y.min(120);
                    min_z = min_z.max(0);
                    max_z = max_z.min(16);

                    let mut found_water = false;
                    'water_scan: for local_x in min_x..max_x {
                        for local_z in min_z..max_z {
                            let mut scan_y = max_y + 1;
                            while scan_y >= min_y - 1 {
                                if (0..BETA_ACTIVE_HEIGHT).contains(&scan_y)
                                    && target.get_block_at_y(local_x, scan_y, local_z) == WATER
                                {
                                    found_water = true;
                                    break 'water_scan;
                                }
                                if scan_y != min_y - 1
                                    && local_x != min_x
                                    && local_x != max_x - 1
                                    && local_z != min_z
                                    && local_z != max_z - 1
                                {
                                    scan_y = min_y - 1;
                                } else {
                                    scan_y -= 1;
                                }
                            }
                        }
                    }

                    if !found_water {
                        for local_x in min_x..max_x {
                            let normalized_x = (f64::from(local_x + target.chunk_x * 16) + 0.5 - x)
                                / horizontal_radius;
                            for local_z in min_z..max_z {
                                let normalized_z = (f64::from(local_z + target.chunk_z * 16) + 0.5
                                    - z)
                                    / horizontal_radius;
                                let mut exposed_grass = false;
                                // Beta added this horizontal ellipse guard.
                                // It changes the raw Y cursor compared with
                                // the otherwise nearly identical Alpha code.
                                if normalized_x * normalized_x + normalized_z * normalized_z < 1.0 {
                                    for block_y in (min_y..max_y).rev() {
                                        let normalized_y =
                                            (f64::from(block_y) + 0.5 - y) / vertical_radius;
                                        let storage_y = block_y + 1;
                                        if normalized_y > -0.7
                                            && normalized_x * normalized_x
                                                + normalized_y * normalized_y
                                                + normalized_z * normalized_z
                                                < 1.0
                                        {
                                            let block =
                                                target.get_block_at_y(local_x, storage_y, local_z);
                                            if block == GRASS_BLOCK {
                                                exposed_grass = true;
                                            }
                                            if matches!(block, STONE | DIRT | GRASS_BLOCK) {
                                                if block_y < 10 {
                                                    target.set_block_at_y(
                                                        local_x, storage_y, local_z, LAVA,
                                                    );
                                                } else {
                                                    target.set_block_at_y(
                                                        local_x, storage_y, local_z, AIR,
                                                    );
                                                    if exposed_grass
                                                        && target.get_block_at_y(
                                                            local_x,
                                                            storage_y - 1,
                                                            local_z,
                                                        ) == DIRT
                                                    {
                                                        target.set_block_at_y(
                                                            local_x,
                                                            storage_y - 1,
                                                            local_z,
                                                            GRASS_BLOCK,
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if room {
                            break;
                        }
                    }
                }
            }

            tunnel += 1;
        }
    }
}

pub(super) fn beta_floor(value: f64) -> i32 {
    value.floor() as i32
}

pub(super) fn beta_sin(value: f32) -> f32 {
    let index = ((value * 10_430.378).trunc() as i32 & 65_535) as usize;
    beta_sin_table()[index]
}

pub(super) fn beta_cos(value: f32) -> f32 {
    let index = ((value * 10_430.378 + 16_384.0).trunc() as i32 & 65_535) as usize;
    beta_sin_table()[index]
}

fn beta_sin_table() -> &'static [f32] {
    static SIN_TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    SIN_TABLE
        .get_or_init(|| {
            (0..65_536)
                .map(|index| ((index as f64) * std::f64::consts::TAU / 65_536.0).sin() as f32)
                .collect()
        })
        .as_slice()
}
