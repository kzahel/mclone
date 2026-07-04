use crate::block::{
    COARSE_DIRT, DIRT, GRASS_BLOCK, ICE, MYCELIUM, PACKED_ICE, PODZOL, RawBlockId, SNOW_BLOCK,
    is_air_like,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{DiskConfiguration, FeatureWorld};

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

fn offset(pos: BlockPos, dx: i32, dy: i32, dz: i32) -> BlockPos {
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}
