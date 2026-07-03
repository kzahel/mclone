use crate::block::{
    AIR, CAVE_AIR, COARSE_DIRT, DIRT, GRASS, GRASS_BLOCK, LARGE_FERN_LOWER, LARGE_FERN_UPPER,
    MYCELIUM, PODZOL, RawBlockId, is_leaves, material_blocks_motion,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{FeatureWorld, HugeMushroomConfiguration, HugeMushroomKind};

pub(super) fn place_huge_mushroom<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: HugeMushroomConfiguration,
) -> bool {
    let mut height = random.next_int_bound(3) + 4;
    if random.next_int_bound(12) == 0 {
        height *= 2;
    }

    if !is_valid_position(world, origin, height, config) {
        return false;
    }

    match config.kind {
        HugeMushroomKind::Brown => make_brown_cap(world, origin, height, config),
        HugeMushroomKind::Red => make_red_cap(world, origin, height, config),
    }
    place_trunk(world, origin, height, config);
    true
}

fn is_valid_position<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    height: i32,
    config: HugeMushroomConfiguration,
) -> bool {
    if origin.y < world.min_y() + 1 || origin.y + height + 1 >= world.min_y() + world.height() {
        return false;
    }

    let below = BlockPos::new(origin.x, origin.y - 1, origin.z);
    if !world
        .block_at_world(below)
        .is_some_and(can_huge_mushroom_grow_on)
    {
        return false;
    }

    for y_offset in 0..=height {
        let radius = validation_radius(config, y_offset);
        for x_offset in -radius..=radius {
            for z_offset in -radius..=radius {
                let pos = BlockPos::new(
                    origin.x + x_offset,
                    origin.y + y_offset,
                    origin.z + z_offset,
                );
                if !can_replace_huge_mushroom_space(world, pos) {
                    return false;
                }
            }
        }
    }

    true
}

fn validation_radius(config: HugeMushroomConfiguration, y_offset: i32) -> i32 {
    match config.kind {
        HugeMushroomKind::Brown => {
            if y_offset <= 3 {
                0
            } else {
                config.foliage_radius
            }
        }
        HugeMushroomKind::Red => 0,
    }
}

fn make_brown_cap<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    height: i32,
    config: HugeMushroomConfiguration,
) {
    let radius = config.foliage_radius;
    for x_offset in -radius..=radius {
        for z_offset in -radius..=radius {
            let edge_x = x_offset == -radius || x_offset == radius;
            let edge_z = z_offset == -radius || z_offset == radius;
            if edge_x && edge_z {
                continue;
            }
            set_if_replaceable(
                world,
                BlockPos::new(origin.x + x_offset, origin.y + height, origin.z + z_offset),
                config.cap,
            );
        }
    }
}

fn make_red_cap<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    height: i32,
    config: HugeMushroomConfiguration,
) {
    for y_offset in height - 3..=height {
        let radius = if y_offset < height {
            config.foliage_radius
        } else {
            config.foliage_radius - 1
        };

        for x_offset in -radius..=radius {
            for z_offset in -radius..=radius {
                let edge_x = x_offset == -radius || x_offset == radius;
                let edge_z = z_offset == -radius || z_offset == radius;
                if y_offset >= height || edge_x != edge_z {
                    set_if_replaceable(
                        world,
                        BlockPos::new(
                            origin.x + x_offset,
                            origin.y + y_offset,
                            origin.z + z_offset,
                        ),
                        config.cap,
                    );
                }
            }
        }
    }
}

fn place_trunk<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    height: i32,
    config: HugeMushroomConfiguration,
) {
    for y_offset in 0..height {
        set_if_replaceable(
            world,
            BlockPos::new(origin.x, origin.y + y_offset, origin.z),
            config.stem,
        );
    }
}

fn set_if_replaceable<W: FeatureWorld>(world: &mut W, pos: BlockPos, block: RawBlockId) -> bool {
    if can_replace_huge_mushroom_space(world, pos) {
        world.set_block_world(pos, block)
    } else {
        false
    }
}

fn can_huge_mushroom_grow_on(block: RawBlockId) -> bool {
    matches!(block, GRASS_BLOCK | DIRT | COARSE_DIRT | PODZOL | MYCELIUM)
}

fn can_replace_huge_mushroom_space<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    world.block_at_world(pos).is_some_and(|block| {
        matches!(
            block,
            AIR | CAVE_AIR | GRASS | LARGE_FERN_LOWER | LARGE_FERN_UPPER
        ) || is_leaves(block)
            || !material_blocks_motion(block)
    })
}
