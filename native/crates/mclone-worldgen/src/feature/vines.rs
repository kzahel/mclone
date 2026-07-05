use crate::block::{
    RawBlockId, VINE_EAST, VINE_NORTH, VINE_SOUTH, VINE_UP, VINE_WEST, is_air_like,
    material_blocks_motion,
};
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
            let Some(vine) = vine_block_for_face(direction) else {
                continue;
            };
            return world.set_block_world(origin, vine);
        }
    }

    false
}

fn vine_block_for_face(face: Direction) -> Option<RawBlockId> {
    match face {
        Direction::Up => Some(VINE_UP),
        Direction::North => Some(VINE_NORTH),
        Direction::South => Some(VINE_SOUTH),
        Direction::West => Some(VINE_WEST),
        Direction::East => Some(VINE_EAST),
        Direction::Down => None,
    }
}
