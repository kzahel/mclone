use mclone_blocks::block_collision_aabbs;
use mclone_core::{BlockPos, Direction};
use mclone_worldgen::block::{
    OakFenceGateState, OakFenceState, RawBlockId, generated_block_state_id, is_leaves, is_water,
    oak_fence_for_state, oak_fence_gate_for_state, oak_fence_gate_state, oak_fence_state,
};

pub(crate) fn placed_oak_fence_state<F>(pos: BlockPos, block_at: F) -> RawBlockId
where
    F: Fn(BlockPos) -> Option<RawBlockId>,
{
    oak_fence_for_state(resolved_fence_state(
        pos,
        OakFenceState {
            waterlogged: block_at(pos).is_some_and(is_water),
            ..OakFenceState::default()
        },
        &block_at,
    ))
}

pub(crate) fn placed_oak_fence_gate_state(facing: Direction) -> RawBlockId {
    oak_fence_gate_for_state(OakFenceGateState {
        facing: direction_index(facing),
        ..OakFenceGateState::default()
    })
    .expect("horizontal player facing produces a fence gate state")
}

pub(crate) fn toggled_oak_fence_gate_state(
    block: RawBlockId,
    player_facing: Direction,
) -> Option<RawBlockId> {
    let mut state = oak_fence_gate_state(block)?;
    if state.open {
        state.open = false;
    } else {
        let player_facing = direction_index(player_facing);
        if state.facing == opposite_index(player_facing) {
            state.facing = player_facing;
        }
        state.open = true;
    }
    oak_fence_gate_for_state(state)
}

pub(crate) fn connected_fence_updates<F>(
    changed: BlockPos,
    block_at: F,
) -> Vec<(BlockPos, RawBlockId)>
where
    F: Fn(BlockPos) -> Option<RawBlockId>,
{
    let mut updates = Vec::with_capacity(5);
    for pos in [
        changed,
        changed.relative(Direction::North),
        changed.relative(Direction::East),
        changed.relative(Direction::South),
        changed.relative(Direction::West),
    ] {
        let Some(current) = block_at(pos) else {
            continue;
        };
        let Some(state) = oak_fence_state(current) else {
            continue;
        };
        let next = oak_fence_for_state(resolved_fence_state(pos, state, &block_at));
        if next != current {
            updates.push((pos, next));
        }
    }
    updates
}

fn resolved_fence_state<F>(pos: BlockPos, mut state: OakFenceState, block_at: &F) -> OakFenceState
where
    F: Fn(BlockPos) -> Option<RawBlockId>,
{
    state.north = block_at(pos.relative(Direction::North))
        .is_some_and(|block| fence_connects_to(block, Direction::North));
    state.east = block_at(pos.relative(Direction::East))
        .is_some_and(|block| fence_connects_to(block, Direction::East));
    state.south = block_at(pos.relative(Direction::South))
        .is_some_and(|block| fence_connects_to(block, Direction::South));
    state.west = block_at(pos.relative(Direction::West))
        .is_some_and(|block| fence_connects_to(block, Direction::West));
    state
}

fn fence_connects_to(block: RawBlockId, direction: Direction) -> bool {
    if oak_fence_state(block).is_some() {
        return true;
    }
    if let Some(gate) = oak_fence_gate_state(block) {
        let gate_axis_x = gate.facing == 1 || gate.facing == 3;
        let direction_axis_z = matches!(direction, Direction::North | Direction::South);
        return gate_axis_x == direction_axis_z;
    }
    is_full_cube_collision(block) && !is_leaves(block)
}

fn is_full_cube_collision(block: RawBlockId) -> bool {
    let pos = BlockPos::ZERO;
    let mut shapes = block_collision_aabbs(generated_block_state_id(block), pos);
    let Some(shape) = shapes.next() else {
        return false;
    };
    shapes.next().is_none()
        && shape.min_x == 0.0
        && shape.min_y == 0.0
        && shape.min_z == 0.0
        && shape.max_x == 1.0
        && shape.max_y == 1.0
        && shape.max_z == 1.0
}

pub(crate) fn horizontal_direction_from_y_rot(y_rot_degrees: f32) -> Direction {
    let yaw = f64::from(y_rot_degrees).to_radians();
    let x = -yaw.sin();
    let z = yaw.cos();
    if x.abs() > z.abs() {
        if x > 0.0 {
            Direction::East
        } else {
            Direction::West
        }
    } else if z > 0.0 {
        Direction::South
    } else {
        Direction::North
    }
}

const fn direction_index(direction: Direction) -> u8 {
    match direction {
        Direction::North => 0,
        Direction::East => 1,
        Direction::South => 2,
        Direction::West => 3,
        Direction::Up | Direction::Down => 2,
    }
}

const fn opposite_index(index: u8) -> u8 {
    (index + 2) & 3
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mclone_worldgen::block::{AIR, OAK_FENCE, OAK_FENCE_GATE, STONE, oak_fence_state};

    use super::*;

    #[test]
    fn fence_connections_follow_fences_full_cubes_and_gate_axis() {
        let center = BlockPos::new(0, 64, 0);
        let blocks = BTreeMap::from([
            (center, OAK_FENCE),
            (center.relative(Direction::North), STONE),
            (center.relative(Direction::East), OAK_FENCE),
            (
                center.relative(Direction::South),
                placed_oak_fence_gate_state(Direction::East),
            ),
            (center.relative(Direction::West), AIR),
        ]);
        let placed = placed_oak_fence_state(center, |pos| blocks.get(&pos).copied().or(Some(AIR)));
        let state = oak_fence_state(placed).unwrap();
        assert!(state.north);
        assert!(state.east);
        assert!(state.south);
        assert!(!state.west);
    }

    #[test]
    fn opening_gate_can_turn_it_to_face_away_from_player() {
        let opened = toggled_oak_fence_gate_state(OAK_FENCE_GATE, Direction::South).unwrap();
        let state = oak_fence_gate_state(opened).unwrap();
        assert!(state.open);
        assert_eq!(state.facing, 2);
        let closed = toggled_oak_fence_gate_state(opened, Direction::North).unwrap();
        assert!(!oak_fence_gate_state(closed).unwrap().open);
    }

    #[test]
    fn yaw_maps_to_cardinal_player_direction() {
        assert_eq!(horizontal_direction_from_y_rot(0.0), Direction::South);
        assert_eq!(horizontal_direction_from_y_rot(90.0), Direction::West);
        assert_eq!(horizontal_direction_from_y_rot(180.0), Direction::North);
        assert_eq!(horizontal_direction_from_y_rot(-90.0), Direction::East);
    }
}
