use crate::block::{
    AIR, BLUE_ICE, COARSE_DIRT, DIRT, GRASS_BLOCK, ICE, MYCELIUM, PACKED_ICE, PODZOL, RawBlockId,
    SNOW, SNOW_BLOCK, WATER, is_air_like, is_water,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{DiskConfiguration, FeatureWorld};

const SEA_LEVEL: i32 = 63;
const DIRECTIONS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

pub(super) fn place_iceberg<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    state: RawBlockId,
) -> bool {
    let base = BlockPos::new(origin.x, SEA_LEVEL, origin.z);
    let snowy = random.next_double() > 0.7;
    let angle = random.next_double() * 2.0 * std::f64::consts::PI;
    let round_radius = 11 - random.next_int_bound(5);
    let ellipse_c = 3 + random.next_int_bound(3);
    let ellipse = random.next_double() > 0.7;
    let mut height = if ellipse {
        random.next_int_bound(6) + 6
    } else {
        random.next_int_bound(15) + 3
    };
    if !ellipse && random.next_double() > 0.9 {
        height += random.next_int_bound(19) + 7;
    }

    let below_height = (height + random.next_int_bound(11)).min(18);
    let top_radius = (height + random.next_int_bound(7) - random.next_int_bound(5)).min(11);
    let horizontal_bound = if ellipse { round_radius } else { 11 };
    let mut placed = false;

    for dx in -horizontal_bound..horizontal_bound {
        for dz in -horizontal_bound..horizontal_bound {
            for dy in 0..height {
                let radius = if ellipse {
                    height_dependent_radius_ellipse(dy, height, top_radius)
                } else {
                    height_dependent_radius_round(random, dy, height, top_radius)
                };
                if ellipse || dx < radius {
                    placed |= generate_iceberg_block(
                        world,
                        random,
                        base,
                        height,
                        dx,
                        dy,
                        dz,
                        radius,
                        horizontal_bound,
                        ellipse,
                        ellipse_c,
                        angle,
                        snowy,
                        state,
                    );
                }
            }
        }
    }

    smooth_iceberg(world, base, top_radius, height, ellipse, round_radius);

    for dx in -horizontal_bound..horizontal_bound {
        for dz in -horizontal_bound..horizontal_bound {
            for dy in (-below_height + 1)..=-1 {
                let below_radius = if ellipse {
                    ceil_i32(
                        horizontal_bound as f64
                            * (1.0 - (dy as f64).powi(2) / (below_height as f64 * 8.0)),
                    )
                } else {
                    horizontal_bound
                };
                let radius = height_dependent_radius_steep(random, -dy, below_height, top_radius);
                if dx < radius {
                    placed |= generate_iceberg_block(
                        world,
                        random,
                        base,
                        below_height,
                        dx,
                        dy,
                        dz,
                        radius,
                        below_radius,
                        ellipse,
                        ellipse_c,
                        angle,
                        snowy,
                        state,
                    );
                }
            }
        }
    }

    let cut_out = if ellipse {
        random.next_double() > 0.1
    } else {
        random.next_double() > 0.7
    };
    if cut_out {
        generate_cut_out(
            random,
            world,
            top_radius,
            height,
            base,
            ellipse,
            round_radius,
            angle,
            ellipse_c,
        );
    }

    placed
}

pub(super) fn place_blue_ice<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
) -> bool {
    if origin.y > SEA_LEVEL - 1 {
        return false;
    }

    let below = offset(origin, 0, -1, 0);
    if !world.block_at_world(origin).is_some_and(is_water)
        && !world.block_at_world(below).is_some_and(is_water)
    {
        return false;
    }

    if !DIRECTIONS
        .iter()
        .filter(|(_, dy, _)| *dy != -1)
        .any(|(dx, dy, dz)| world.block_at_world(offset(origin, *dx, *dy, *dz)) == Some(PACKED_ICE))
    {
        return false;
    }

    let mut placed = world.set_block_world(origin, BLUE_ICE);
    for _ in 0..200 {
        let dy = random.next_int_bound(5) - random.next_int_bound(6);
        let mut radius = 3;
        if dy < 2 {
            radius += dy / 2;
        }
        if radius < 1 {
            continue;
        }

        let pos = offset(
            origin,
            random.next_int_bound(radius) - random.next_int_bound(radius),
            dy,
            random.next_int_bound(radius) - random.next_int_bound(radius),
        );
        let Some(current) = world.block_at_world(pos) else {
            continue;
        };
        if !(is_air_like(current) || is_water(current) || current == PACKED_ICE || current == ICE) {
            continue;
        }

        if DIRECTIONS
            .iter()
            .any(|(dx, dy, dz)| world.block_at_world(offset(pos, *dx, *dy, *dz)) == Some(BLUE_ICE))
        {
            placed |= world.set_block_world(pos, BLUE_ICE);
        }
    }

    placed
}

pub(super) fn place_ice_spike<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
) -> bool {
    let mut base = origin;
    while world.block_at_world(base).is_some_and(is_air_like) && base.y > world.min_y() + 2 {
        base.y -= 1;
    }

    if world.block_at_world(base) != Some(SNOW_BLOCK) {
        return false;
    }

    base.y += random.next_int_bound(4);
    let height = random.next_int_bound(4) + 7;
    let radius = height / 4 + random.next_int_bound(2);
    if radius > 1 && random.next_int_bound(60) == 0 {
        base.y += 10 + random.next_int_bound(30);
    }

    let mut placed = false;
    for y_offset in 0..height {
        let radius_at_y = (1.0 - y_offset as f32 / height as f32) * radius as f32;
        let radius_int = radius_at_y.ceil() as i32;

        for dx in -radius_int..=radius_int {
            let x_dist = dx.abs() as f32 - 0.25;
            for dz in -radius_int..=radius_int {
                let z_dist = dz.abs() as f32 - 0.25;
                let inside_radius = dx == 0 && dz == 0
                    || x_dist * x_dist + z_dist * z_dist <= radius_at_y * radius_at_y;
                let edge =
                    dx == -radius_int || dx == radius_int || dz == -radius_int || dz == radius_int;
                if inside_radius && (!edge || random.next_float() <= 0.75) {
                    placed |=
                        set_packed_ice_if_replaceable(world, offset(base, dx, y_offset, dz), false);

                    if y_offset != 0 && radius_int > 1 {
                        placed |= set_packed_ice_if_replaceable(
                            world,
                            offset(base, dx, -y_offset, dz),
                            false,
                        );
                    }
                }
            }
        }
    }

    let root_radius = (radius - 1).clamp(0, 1);
    for dx in -root_radius..=root_radius {
        for dz in -root_radius..=root_radius {
            let mut root = offset(base, dx, -1, dz);
            let mut remaining = if dx.abs() == 1 && dz.abs() == 1 {
                random.next_int_bound(5)
            } else {
                50
            };

            while root.y > 50 {
                let Some(current) = world.block_at_world(root) else {
                    break;
                };
                if !is_ice_spike_replaceable(current, true) {
                    break;
                }

                placed |= world.set_block_world(root, PACKED_ICE);
                root.y -= 1;
                remaining -= 1;
                if remaining <= 0 {
                    root.y -= random.next_int_bound(5) + 1;
                    remaining = random.next_int_bound(5);
                }
            }
        }
    }

    placed
}

pub(super) fn place_ice_patch<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: DiskConfiguration,
) -> bool {
    let mut base = origin;
    while world.block_at_world(base).is_some_and(is_air_like) && base.y > world.min_y() + 2 {
        base.y -= 1;
    }

    if world.block_at_world(base) != Some(SNOW_BLOCK) {
        return false;
    }

    place_disk(world, random, base, config)
}

fn place_disk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: DiskConfiguration,
) -> bool {
    let radius = config.radius.sample(random);
    let max_y = origin.y + config.half_height;
    let min_y_exclusive = origin.y - config.half_height - 1;
    let mut placed = false;

    for x in origin.x - radius..=origin.x + radius {
        for z in origin.z - radius..=origin.z + radius {
            let dx = x - origin.x;
            let dz = z - origin.z;
            if dx * dx + dz * dz > radius * radius {
                continue;
            }

            for y in (min_y_exclusive..=max_y).rev() {
                let pos = BlockPos::new(x, y, z);
                let Some(current) = world.block_at_world(pos) else {
                    continue;
                };
                if y > min_y_exclusive && config.targets.contains(&current) {
                    placed |= world.set_block_world(pos, config.state);
                }
            }
        }
    }

    placed
}

fn set_packed_ice_if_replaceable<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
    allow_packed_ice: bool,
) -> bool {
    world
        .block_at_world(pos)
        .is_some_and(|block| is_ice_spike_replaceable(block, allow_packed_ice))
        && world.set_block_world(pos, PACKED_ICE)
}

fn is_ice_spike_replaceable(block: RawBlockId, allow_packed_ice: bool) -> bool {
    is_air_like(block)
        || is_dirt_tag(block)
        || block == SNOW_BLOCK
        || block == ICE
        || (allow_packed_ice && block == PACKED_ICE)
}

fn is_dirt_tag(block: RawBlockId) -> bool {
    matches!(block, DIRT | GRASS_BLOCK | PODZOL | COARSE_DIRT | MYCELIUM)
}

fn generate_iceberg_block<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    base: BlockPos,
    height: i32,
    dx: i32,
    dy: i32,
    dz: i32,
    radius: i32,
    horizontal_bound: i32,
    ellipse: bool,
    ellipse_c: i32,
    angle: f64,
    snowy: bool,
    state: RawBlockId,
) -> bool {
    let signed_distance = if ellipse {
        signed_distance_ellipse(
            dx,
            dz,
            BlockPos::new(0, 0, 0),
            horizontal_bound,
            get_ellipse_c(dy, height, ellipse_c),
            angle,
        )
    } else {
        signed_distance_circle(dx, dz, BlockPos::new(0, 0, 0), radius, random)
    };
    if signed_distance >= 0.0 {
        return false;
    }

    let threshold = if ellipse {
        -0.5
    } else {
        -6.0 - random.next_int_bound(3) as f64
    };
    if signed_distance > threshold && random.next_double() > 0.9 {
        return false;
    }

    set_iceberg_block(
        world,
        offset(base, dx, dy, dz),
        random,
        height - dy,
        height,
        ellipse,
        snowy,
        state,
    )
}

fn set_iceberg_block<W: FeatureWorld>(
    world: &mut W,
    pos: BlockPos,
    random: &mut impl RandomSource,
    remaining_height: i32,
    height: i32,
    ellipse: bool,
    snowy: bool,
    state: RawBlockId,
) -> bool {
    let Some(current) = world.block_at_world(pos) else {
        return false;
    };
    if !(is_air_like(current) || current == SNOW_BLOCK || current == ICE || is_water(current)) {
        return false;
    }

    let keep_solid = !ellipse || random.next_double() > 0.05;
    let divisor = if ellipse { 3 } else { 2 };
    let snow_limit = random.next_int_bound((height / divisor).max(1)) as f64 + height as f64 * 0.6;
    if snowy && !is_water(current) && remaining_height as f64 <= snow_limit && keep_solid {
        world.set_block_world(pos, SNOW_BLOCK)
    } else {
        world.set_block_world(pos, state)
    }
}

fn generate_cut_out<W: FeatureWorld>(
    random: &mut impl RandomSource,
    world: &mut W,
    top_radius: i32,
    height: i32,
    base: BlockPos,
    ellipse: bool,
    round_radius: i32,
    angle: f64,
    ellipse_c: i32,
) {
    let sign_x = if random.next_boolean() { -1 } else { 1 };
    let sign_z = if random.next_boolean() { -1 } else { 1 };
    let mut cut_x = random.next_int_bound((top_radius / 2 - 2).max(1));
    if random.next_boolean() {
        cut_x =
            top_radius / 2 + 1 - random.next_int_bound((top_radius - top_radius / 2 - 1).max(1));
    }
    let mut cut_z = random.next_int_bound((top_radius / 2 - 2).max(1));
    if random.next_boolean() {
        cut_z =
            top_radius / 2 + 1 - random.next_int_bound((top_radius - top_radius / 2 - 1).max(1));
    }
    if ellipse {
        let cut = random.next_int_bound((round_radius - 5).max(1));
        cut_x = cut;
        cut_z = cut;
    }

    let cut_origin = BlockPos::new(sign_x * cut_x, 0, sign_z * cut_z);
    let cut_angle = if ellipse {
        angle + std::f64::consts::FRAC_PI_2
    } else {
        random.next_double() * 2.0 * std::f64::consts::PI
    };

    for dy in 0..height - 3 {
        let radius = height_dependent_radius_round(random, dy, height, top_radius);
        carve(
            radius,
            dy,
            base,
            world,
            false,
            cut_angle,
            cut_origin,
            round_radius,
            ellipse_c,
        );
    }

    let min_dy = -height + random.next_int_bound(5) + 1;
    for dy in min_dy..=-1 {
        let radius = height_dependent_radius_steep(random, -dy, height, top_radius);
        carve(
            radius,
            dy,
            base,
            world,
            true,
            cut_angle,
            cut_origin,
            round_radius,
            ellipse_c,
        );
    }
}

fn carve<W: FeatureWorld>(
    radius: i32,
    dy: i32,
    base: BlockPos,
    world: &mut W,
    below_water: bool,
    angle: f64,
    cut_origin: BlockPos,
    round_radius: i32,
    ellipse_c: i32,
) {
    let a = radius + 1 + round_radius / 3;
    let b = (radius - 3).min(3) + ellipse_c / 2 - 1;
    for dx in -a..a {
        for dz in -a..a {
            if signed_distance_ellipse(dx, dz, cut_origin, a, b, angle) >= 0.0 {
                continue;
            }

            let pos = offset(base, dx, dy, dz);
            let Some(current) = world.block_at_world(pos) else {
                continue;
            };
            if !is_iceberg_state(current) && current != SNOW_BLOCK {
                continue;
            }

            if below_water {
                world.set_block_world(pos, WATER);
            } else {
                world.set_block_world(pos, AIR);
                remove_floating_snow_layer(world, pos);
            }
        }
    }
}

fn remove_floating_snow_layer<W: FeatureWorld>(world: &mut W, pos: BlockPos) {
    let above = offset(pos, 0, 1, 0);
    if world.block_at_world(above) == Some(SNOW) {
        world.set_block_world(above, AIR);
    }
}

fn smooth_iceberg<W: FeatureWorld>(
    world: &mut W,
    base: BlockPos,
    top_radius: i32,
    height: i32,
    ellipse: bool,
    round_radius: i32,
) {
    let radius = if ellipse {
        round_radius
    } else {
        top_radius / 2
    };
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            for dy in 0..=height {
                let pos = offset(base, dx, dy, dz);
                let Some(current) = world.block_at_world(pos) else {
                    continue;
                };
                if !is_iceberg_state(current) && current != SNOW {
                    continue;
                }

                if world
                    .block_at_world(offset(pos, 0, -1, 0))
                    .is_some_and(is_air_like)
                {
                    world.set_block_world(pos, AIR);
                    world.set_block_world(offset(pos, 0, 1, 0), AIR);
                } else if is_iceberg_state(current) {
                    let exposed_faces = [
                        offset(pos, -1, 0, 0),
                        offset(pos, 1, 0, 0),
                        offset(pos, 0, 0, -1),
                        offset(pos, 0, 0, 1),
                    ]
                    .iter()
                    .filter(|neighbor| {
                        !world
                            .block_at_world(**neighbor)
                            .is_some_and(is_iceberg_state)
                    })
                    .count();
                    if exposed_faces >= 3 {
                        world.set_block_world(pos, AIR);
                    }
                }
            }
        }
    }
}

fn get_ellipse_c(y: i32, height: i32, ellipse_c: i32) -> i32 {
    if y > 0 && height - y <= 3 {
        ellipse_c - (4 - (height - y))
    } else {
        ellipse_c
    }
}

fn signed_distance_circle(
    dx: i32,
    dz: i32,
    origin: BlockPos,
    radius: i32,
    random: &mut impl RandomSource,
) -> f64 {
    let fuzz = 10.0 * random.next_float().clamp(0.2, 0.8) as f64 / radius as f64;
    fuzz + ((dx - origin.x) as f64).powi(2) + ((dz - origin.z) as f64).powi(2)
        - (radius as f64).powi(2)
}

fn signed_distance_ellipse(dx: i32, dz: i32, origin: BlockPos, a: i32, b: i32, angle: f64) -> f64 {
    let rotated_x =
        ((dx - origin.x) as f64 * angle.cos() - (dz - origin.z) as f64 * angle.sin()) / a as f64;
    let rotated_z =
        ((dx - origin.x) as f64 * angle.sin() + (dz - origin.z) as f64 * angle.cos()) / b as f64;
    rotated_x.powi(2) + rotated_z.powi(2) - 1.0
}

fn height_dependent_radius_round(
    random: &mut impl RandomSource,
    y: i32,
    height: i32,
    top_radius: i32,
) -> i32 {
    let f = 3.5 - random.next_float();
    let mut radius = (1.0 - (y as f32).powi(2) / (height as f32 * f)) * top_radius as f32;
    if height > 15 + random.next_int_bound(5) {
        let adjusted_y = if y < 3 + random.next_int_bound(6) {
            y / 2
        } else {
            y
        };
        radius = (1.0 - adjusted_y as f32 / (height as f32 * f * 0.4)) * top_radius as f32;
    }

    ceil_i32(radius as f64 / 2.0)
}

fn height_dependent_radius_ellipse(y: i32, height: i32, top_radius: i32) -> i32 {
    ceil_i32(((1.0 - (y as f32).powi(2) / height as f32) * top_radius as f32) as f64 / 2.0)
}

fn height_dependent_radius_steep(
    random: &mut impl RandomSource,
    y: i32,
    height: i32,
    top_radius: i32,
) -> i32 {
    let f = 1.0 + random.next_float() / 2.0;
    ceil_i32(((1.0 - y as f32 / (height as f32 * f)) * top_radius as f32) as f64 / 2.0)
}

fn ceil_i32(value: f64) -> i32 {
    value.ceil() as i32
}

fn is_iceberg_state(block: RawBlockId) -> bool {
    matches!(block, PACKED_ICE | SNOW_BLOCK | BLUE_ICE)
}

fn offset(pos: BlockPos, dx: i32, dy: i32, dz: i32) -> BlockPos {
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}
