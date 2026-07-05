use crate::block::{
    ANDESITE, COARSE_DIRT, DEEPSLATE, DIORITE, DIRT, GRANITE, GRASS_BLOCK, MYCELIUM, PODZOL,
    RawBlockId, STONE, TUFF, is_air_like,
};
use crate::placement::BlockPos;
use crate::prng::RandomSource;

use super::{BlockStateConfiguration, FeatureWorld};

pub(super) fn place_block_blob<W: FeatureWorld>(
    world: &mut W,
    random: &mut impl RandomSource,
    origin: BlockPos,
    config: BlockStateConfiguration,
) -> bool {
    let mut pos = origin;
    while pos.y > world.min_y() + 3 {
        let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
        if let Some(block) = world.block_at_world(below) {
            if !is_air_like(block) && (is_dirt(block) || is_stone(block)) {
                break;
            }
        }
        pos = below;
    }

    if pos.y <= world.min_y() + 3 {
        return false;
    }

    for _ in 0..3 {
        let x_radius = random.next_int_bound(2);
        let y_radius = random.next_int_bound(2);
        let z_radius = random.next_int_bound(2);
        let radius = (x_radius + y_radius + z_radius) as f32 * 0.333 + 0.5;

        for x in pos.x - x_radius..=pos.x + x_radius {
            for y in pos.y - y_radius..=pos.y + y_radius {
                for z in pos.z - z_radius..=pos.z + z_radius {
                    let current = BlockPos::new(x, y, z);
                    if dist_sqr(current, pos) <= radius * radius {
                        world.set_block_world(current, config.state);
                    }
                }
            }
        }

        pos = BlockPos::new(
            pos.x - 1 + random.next_int_bound(2),
            pos.y - random.next_int_bound(2),
            pos.z - 1 + random.next_int_bound(2),
        );
    }

    true
}

fn dist_sqr(pos: BlockPos, origin: BlockPos) -> f32 {
    let dx = pos.x as f32 + 0.5 - origin.x as f32;
    let dy = pos.y as f32 + 0.5 - origin.y as f32;
    let dz = pos.z as f32 + 0.5 - origin.z as f32;
    dx * dx + dy * dy + dz * dz
}

fn is_dirt(block: RawBlockId) -> bool {
    matches!(block, DIRT | GRASS_BLOCK | PODZOL | COARSE_DIRT | MYCELIUM)
}

fn is_stone(block: RawBlockId) -> bool {
    matches!(
        block,
        STONE | GRANITE | DIORITE | ANDESITE | TUFF | DEEPSLATE
    )
}
