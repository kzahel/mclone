use crate::block::is_air_like;
use crate::placement::BlockPos;

use super::{Direction, FeatureWorld, SpringConfiguration, offset_pos};

const SPRING_NEIGHBOR_DIRECTIONS: [Direction; 5] = [
    Direction::West,
    Direction::East,
    Direction::North,
    Direction::South,
    Direction::Down,
];

pub(super) fn place_spring<W: FeatureWorld>(
    world: &mut W,
    origin: BlockPos,
    config: SpringConfiguration,
) -> bool {
    let above = offset_pos(origin, Direction::Up);
    let below = offset_pos(origin, Direction::Down);
    if !world
        .block_at_world(above)
        .is_some_and(|block_id| config.valid_blocks.contains(&block_id))
    {
        return false;
    }
    if config.requires_block_below
        && !world
            .block_at_world(below)
            .is_some_and(|block_id| config.valid_blocks.contains(&block_id))
    {
        return false;
    }

    let Some(current) = world.block_at_world(origin) else {
        return false;
    };
    if !is_air_like(current) && !config.valid_blocks.contains(&current) {
        return false;
    }

    let mut rock_count = 0;
    let mut hole_count = 0;
    for direction in SPRING_NEIGHBOR_DIRECTIONS {
        let neighbor = offset_pos(origin, direction);
        let Some(block_id) = world.block_at_world(neighbor) else {
            continue;
        };
        if config.valid_blocks.contains(&block_id) {
            rock_count += 1;
        }
        if is_air_like(block_id) {
            hole_count += 1;
        }
    }

    if rock_count != config.rock_count || hole_count != config.hole_count {
        return false;
    }

    if world.set_block_world(origin, config.state) {
        world.schedule_liquid_tick_world(origin, config.state, 0);
        true
    } else {
        false
    }
}
