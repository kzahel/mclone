use crate::block::{VINE, is_air_like, material_blocks_motion};
use crate::placement::BlockPos;

use super::{Direction, FeatureWorld, offset_pos};

pub(super) fn place_vines<W: FeatureWorld>(world: &mut W, origin: BlockPos) -> bool {
    let Some(current) = world.block_at_world(origin) else {
        return false;
    };
    if !is_air_like(current) {
        return false;
    }

    for direction in Direction::ALL {
        if direction == Direction::Down {
            continue;
        }
        let neighbor = offset_pos(origin, direction);
        if world
            .block_at_world(neighbor)
            .is_some_and(material_blocks_motion)
        {
            return world.set_block_world(origin, VINE);
        }
    }

    false
}
