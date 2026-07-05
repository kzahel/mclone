use crate::block::{
    BAMBOO, BAMBOO_FINAL_LARGE, BAMBOO_TOP_LARGE, BAMBOO_TOP_SMALL, COARSE_DIRT, DIRT, GRASS_BLOCK,
    MYCELIUM, PODZOL, RED_SAND, SAND, is_air_like, is_bamboo,
};
use crate::placement::{BlockPos, HeightmapType};
use crate::prng::RandomSource;

use super::{BambooConfiguration, FeatureWorld};

pub(super) fn place_bamboo<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: BambooConfiguration,
) -> bool {
    let Some(current) = world.block_at_world(origin) else {
        return false;
    };
    if !is_air_like(current) || !can_survive_bamboo(world, origin) {
        return false;
    }

    let height = random.next_int_bound(12) + 5;
    if random.next_float() < config.probability {
        place_podzol_disk(world, random, origin);
    }

    let mut placed = 0;
    let mut pos = origin;
    while placed < height {
        if !world.block_at_world(pos).is_some_and(is_air_like) {
            break;
        }
        if world.set_block_world(pos, BAMBOO) {
            placed += 1;
        }
        pos = BlockPos::new(pos.x, pos.y + 1, pos.z);
    }

    if placed >= 3 {
        world.set_block_world(pos, BAMBOO_FINAL_LARGE);
        world.set_block_world(BlockPos::new(pos.x, pos.y - 1, pos.z), BAMBOO_TOP_LARGE);
        world.set_block_world(BlockPos::new(pos.x, pos.y - 2, pos.z), BAMBOO_TOP_SMALL);
    }

    placed > 0
}

fn can_survive_bamboo<W: FeatureWorld>(world: &mut W, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    world.block_at_world(below).is_some_and(|block| {
        is_bamboo(block)
            || matches!(
                block,
                GRASS_BLOCK | DIRT | COARSE_DIRT | PODZOL | MYCELIUM | SAND | RED_SAND
            )
    })
}

fn place_podzol_disk<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
) {
    let radius = random.next_int_bound(4) + 1;
    for x in origin.x - radius..=origin.x + radius {
        for z in origin.z - radius..=origin.z + radius {
            let dx = x - origin.x;
            let dz = z - origin.z;
            if dx * dx + dz * dz > radius * radius {
                continue;
            }
            let Some(surface_y) = world.height_at(HeightmapType::WorldSurface, x, z) else {
                continue;
            };
            let pos = BlockPos::new(x, surface_y - 1, z);
            let Some(block) = world.block_at_world(pos) else {
                continue;
            };
            if matches!(block, GRASS_BLOCK | DIRT | COARSE_DIRT | PODZOL | MYCELIUM) {
                world.set_block_world(pos, PODZOL);
            }
        }
    }
}
