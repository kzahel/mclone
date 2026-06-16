use std::sync::OnceLock;

use crate::block::{RawBlockId, is_air_like};
use crate::placement::{BlockPos, HeightmapType};
use crate::prng::RandomSource;

use super::{FeatureWorld, OreConfiguration, OreTargetBlockState};

const SIN_TABLE_SIZE: usize = 65_536;
const SIN_TABLE_MASK: i32 = 65_535;
const SIN_SCALE: f32 = 10_430.378_f32;

pub(super) fn place_ore<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: &OreConfiguration,
) -> bool {
    if config.size <= 0 {
        return false;
    }

    let angle = random.next_float() * std::f32::consts::PI;
    let radius = config.size as f32 / 8.0;
    let padding = ceil_f32((config.size as f32 / 16.0 * 2.0 + 1.0) / 2.0);
    let start_x = origin.x as f64 + (angle as f64).sin() * radius as f64;
    let end_x = origin.x as f64 - (angle as f64).sin() * radius as f64;
    let start_z = origin.z as f64 + (angle as f64).cos() * radius as f64;
    let end_z = origin.z as f64 - (angle as f64).cos() * radius as f64;
    let start_y = origin.y as f64 + random.next_int_bound(3) as f64 - 2.0;
    let end_y = origin.y as f64 + random.next_int_bound(3) as f64 - 2.0;
    let min_x = origin.x - ceil_f32(radius) - padding;
    let min_y = origin.y - 2 - padding;
    let min_z = origin.z - ceil_f32(radius) - padding;
    let width_xz = 2 * (ceil_f32(radius) + padding);
    let height_y = 2 * (2 + padding);

    for x in min_x..=min_x + width_xz {
        for z in min_z..=min_z + width_xz {
            let height = world
                .height_at(HeightmapType::OceanFloorWg, x, z)
                .unwrap_or(world.min_y() - 1);
            if min_y <= height {
                return do_place_ore(
                    world, random, config, start_x, end_x, start_z, end_z, start_y, end_y, min_x,
                    min_y, min_z, width_xz, height_y,
                );
            }
        }
    }

    false
}

#[allow(clippy::too_many_arguments)]
fn do_place_ore<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    config: &OreConfiguration,
    start_x: f64,
    end_x: f64,
    start_z: f64,
    end_z: f64,
    start_y: f64,
    end_y: f64,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    width_xz: i32,
    height_y: i32,
) -> bool {
    if width_xz <= 0 || height_y <= 0 {
        return false;
    }

    let mut placed = 0;
    let mut visited = vec![false; (width_xz * height_y * width_xz) as usize];
    let size = config.size as usize;
    let mut spheres = vec![0.0; size * 4];

    for index in 0..size {
        let progress = index as f32 / config.size as f32;
        let center_x = lerp(progress as f64, start_x, end_x);
        let center_y = lerp(progress as f64, start_y, end_y);
        let center_z = lerp(progress as f64, start_z, end_z);
        let scale = random.next_double() * config.size as f64 / 16.0;
        let radius = ((mth_sin(std::f32::consts::PI * progress) as f64 + 1.0) * scale + 1.0) / 2.0;
        spheres[index * 4] = center_x;
        spheres[index * 4 + 1] = center_y;
        spheres[index * 4 + 2] = center_z;
        spheres[index * 4 + 3] = radius;
    }

    for left in 0..size.saturating_sub(1) {
        if spheres[left * 4 + 3] <= 0.0 {
            continue;
        }
        for right in left + 1..size {
            if spheres[right * 4 + 3] <= 0.0 {
                continue;
            }

            let dx = spheres[left * 4] - spheres[right * 4];
            let dy = spheres[left * 4 + 1] - spheres[right * 4 + 1];
            let dz = spheres[left * 4 + 2] - spheres[right * 4 + 2];
            let dr = spheres[left * 4 + 3] - spheres[right * 4 + 3];
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    spheres[right * 4 + 3] = -1.0;
                } else {
                    spheres[left * 4 + 3] = -1.0;
                }
            }
        }
    }

    for index in 0..size {
        let radius = spheres[index * 4 + 3];
        if radius < 0.0 {
            continue;
        }

        let center_x = spheres[index * 4];
        let center_y = spheres[index * 4 + 1];
        let center_z = spheres[index * 4 + 2];
        let block_min_x = floor_f64(center_x - radius).max(min_x);
        let block_min_y = floor_f64(center_y - radius).max(min_y);
        let block_min_z = floor_f64(center_z - radius).max(min_z);
        let block_max_x = floor_f64(center_x + radius).max(block_min_x);
        let block_max_y = floor_f64(center_y + radius).max(block_min_y);
        let block_max_z = floor_f64(center_z + radius).max(block_min_z);

        for x in block_min_x..=block_max_x {
            let normalized_x = (x as f64 + 0.5 - center_x) / radius;
            if normalized_x * normalized_x >= 1.0 {
                continue;
            }
            for y in block_min_y..=block_max_y {
                let normalized_y = (y as f64 + 0.5 - center_y) / radius;
                if normalized_x * normalized_x + normalized_y * normalized_y >= 1.0 {
                    continue;
                }
                for z in block_min_z..=block_max_z {
                    let normalized_z = (z as f64 + 0.5 - center_z) / radius;
                    if normalized_x * normalized_x
                        + normalized_y * normalized_y
                        + normalized_z * normalized_z
                        >= 1.0
                        || !(world.min_y()..world.min_y() + world.height()).contains(&y)
                    {
                        continue;
                    }

                    let visited_index =
                        x - min_x + (y - min_y) * width_xz + (z - min_z) * width_xz * height_y;
                    if visited_index < 0 || visited_index as usize >= visited.len() {
                        continue;
                    }
                    let visited_index = visited_index as usize;
                    if visited[visited_index] {
                        continue;
                    }
                    visited[visited_index] = true;

                    let pos = BlockPos::new(x, y, z);
                    let Some(current) = world.block_at_world(pos) else {
                        continue;
                    };
                    for target in &config.target_states {
                        if can_place_ore(current, world, random, config, *target, pos)
                            && world.set_block_world(pos, target.state)
                        {
                            placed += 1;
                            break;
                        }
                    }
                }
            }
        }
    }

    placed > 0
}

fn can_place_ore<W: FeatureWorld>(
    current: RawBlockId,
    world: &mut W,
    random: &mut impl RandomSource,
    config: &OreConfiguration,
    target: OreTargetBlockState,
    pos: BlockPos,
) -> bool {
    if !target.target.matches(current) {
        return false;
    }
    if should_skip_air_check(random, config.discard_chance_on_air_exposure) {
        true
    } else {
        !is_adjacent_to_air(world, pos)
    }
}

fn should_skip_air_check(random: &mut impl RandomSource, chance: f32) -> bool {
    if chance <= 0.0 {
        true
    } else if chance >= 1.0 {
        false
    } else {
        random.next_float() >= chance
    }
}

fn is_adjacent_to_air<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    const NEIGHBORS: [(i32, i32, i32); 6] = [
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
        (-1, 0, 0),
        (1, 0, 0),
    ];

    NEIGHBORS.iter().any(|(dx, dy, dz)| {
        world
            .block_at_world(BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz))
            .is_some_and(is_air_like)
    })
}

fn ceil_f32(value: f32) -> i32 {
    let truncated = value as i32;
    if value > truncated as f32 {
        truncated + 1
    } else {
        truncated
    }
}

fn floor_f64(value: f64) -> i32 {
    let truncated = value as i32;
    if value < truncated as f64 {
        truncated - 1
    } else {
        truncated
    }
}

fn lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

fn mth_sin(value: f32) -> f32 {
    let index = ((value * SIN_SCALE).trunc() as i32 & SIN_TABLE_MASK) as usize;
    sin_table()[index]
}

fn sin_table() -> &'static [f32] {
    static SIN_TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    SIN_TABLE
        .get_or_init(|| {
            (0..SIN_TABLE_SIZE)
                .map(|index| {
                    ((index as f64) * std::f64::consts::TAU / SIN_TABLE_SIZE as f64).sin() as f32
                })
                .collect()
        })
        .as_slice()
}
