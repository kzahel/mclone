use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockHitResult, BlockPos, BlockStateId, Direction, HitResultType, Vec3d,
    block_to_section_coord, chunk_section_index, local_block_coord, local_section_block_coord,
};
use mclone_protocol::{
    ClientCommand, PlayerActionCommand, PlayerActionKind, UseItemOnCommand, UseItemOnKind,
};

use crate::ClientRuntime;

pub const CREATIVE_PICK_RANGE: f64 = 5.0;
pub const DEFAULT_DEBUG_PLACE_BLOCK: BlockStateId = BlockStateId(1);

#[derive(Clone, Debug, PartialEq)]
pub struct ClientInteractionController {
    pick_range: f64,
    selected_debug_block: BlockStateId,
}

impl Default for ClientInteractionController {
    fn default() -> Self {
        Self {
            pick_range: CREATIVE_PICK_RANGE,
            selected_debug_block: DEFAULT_DEBUG_PLACE_BLOCK,
        }
    }
}

impl ClientInteractionController {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn pick_range(&self) -> f64 {
        self.pick_range
    }

    pub const fn selected_debug_block(&self) -> BlockStateId {
        self.selected_debug_block
    }

    pub fn set_selected_debug_block(&mut self, block_state: BlockStateId) {
        self.selected_debug_block = block_state;
    }

    pub fn pick_block(
        &self,
        client: &ClientRuntime,
        eye_position: Vec3d,
        view_vector: Vec3d,
    ) -> BlockHitResult {
        client.pick_block(eye_position, view_vector, self.pick_range)
    }

    pub fn debug_instant_break_command(&self, hit: BlockHitResult) -> Option<ClientCommand> {
        (hit.hit_type() == HitResultType::Block).then_some(ClientCommand::PlayerAction(
            PlayerActionCommand {
                pos: hit.block_pos,
                direction: hit.direction,
                kind: PlayerActionKind::DebugInstantBreak,
            },
        ))
    }

    pub fn debug_place_block_command(&self, hit: BlockHitResult) -> Option<ClientCommand> {
        (hit.hit_type() == HitResultType::Block).then_some(ClientCommand::UseItemOn(
            UseItemOnCommand {
                hit,
                action: UseItemOnKind::DebugPlaceBlock {
                    block_state: self.selected_debug_block,
                },
            },
        ))
    }
}

impl ClientRuntime {
    pub fn block_state_at_block_pos(&self, pos: BlockPos) -> Option<BlockStateId> {
        let snapshot = self.chunk_snapshot(pos.chunk_pos())?;
        if pos.y < snapshot.min_y || pos.y >= snapshot.min_y + snapshot.height {
            return None;
        }
        let section_y = block_to_section_coord(pos.y);
        let local_x = local_block_coord(pos.x);
        let local_y = local_section_block_coord(pos.y);
        let local_z = local_block_coord(pos.z);
        let index = chunk_section_index(local_x, local_y, local_z);
        Some(
            snapshot
                .sections
                .iter()
                .find(|section| section.section_y == section_y)
                .map(|section| section.unpack_block_state_ids()[index])
                .unwrap_or(AIR_BLOCK_STATE_ID),
        )
    }

    pub fn pick_block(
        &self,
        eye_position: Vec3d,
        view_vector: Vec3d,
        range: f64,
    ) -> BlockHitResult {
        let to = eye_position.add(view_vector.scale(range));
        self.clip_blocks(eye_position, to)
    }

    pub fn clip_blocks(&self, from: Vec3d, to: Vec3d) -> BlockHitResult {
        if !from.is_finite() || !to.is_finite() || from == to {
            return miss(from, to);
        }

        let delta = to.subtract(from);
        let mut pos = BlockPos::containing(from);
        if let Some(hit) = self.clip_block_at(from, to, pos) {
            return hit;
        }

        let step_x = sign(delta.x);
        let step_y = sign(delta.y);
        let step_z = sign(delta.z);
        let mut t_max_x = initial_t_max(from.x, delta.x, step_x);
        let mut t_max_y = initial_t_max(from.y, delta.y, step_y);
        let mut t_max_z = initial_t_max(from.z, delta.z, step_z);
        let t_delta_x = t_delta(delta.x);
        let t_delta_y = t_delta(delta.y);
        let t_delta_z = t_delta(delta.z);

        while t_max_x <= 1.0 || t_max_y <= 1.0 || t_max_z <= 1.0 {
            if t_max_x < t_max_y {
                if t_max_x < t_max_z {
                    pos = pos.offset(step_x, 0, 0);
                    t_max_x += t_delta_x;
                } else {
                    pos = pos.offset(0, 0, step_z);
                    t_max_z += t_delta_z;
                }
            } else if t_max_y < t_max_z {
                pos = pos.offset(0, step_y, 0);
                t_max_y += t_delta_y;
            } else {
                pos = pos.offset(0, 0, step_z);
                t_max_z += t_delta_z;
            }

            if let Some(hit) = self.clip_block_at(from, to, pos) {
                return hit;
            }
        }

        miss(from, to)
    }

    fn clip_block_at(&self, from: Vec3d, to: Vec3d, pos: BlockPos) -> Option<BlockHitResult> {
        let state = self.block_state_at_block_pos(pos)?;
        if state == AIR_BLOCK_STATE_ID {
            return None;
        }
        clip_unit_block(from, to, pos)
    }
}

fn miss(from: Vec3d, to: Vec3d) -> BlockHitResult {
    let delta = from.subtract(to);
    BlockHitResult::miss(
        to,
        Direction::nearest(delta.x, delta.y, delta.z),
        BlockPos::containing(to),
    )
}

fn sign(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn initial_t_max(position: f64, delta: f64, step: i32) -> f64 {
    if step == 0 {
        return f64::INFINITY;
    }
    let block = position.floor();
    if step > 0 {
        (block + 1.0 - position) / delta
    } else {
        (position - block) / -delta
    }
}

fn t_delta(delta: f64) -> f64 {
    if delta == 0.0 {
        f64::INFINITY
    } else {
        1.0 / delta.abs()
    }
}

fn clip_unit_block(from: Vec3d, to: Vec3d, pos: BlockPos) -> Option<BlockHitResult> {
    let delta = to.subtract(from);
    if delta.length_sqr() < 1.0e-7 {
        return None;
    }
    if contains_unit_block(pos, from) {
        return Some(BlockHitResult::new(
            from.add(delta.scale(0.001)),
            Direction::nearest(delta.x, delta.y, delta.z).opposite(),
            pos,
            true,
        ));
    }

    let min = Vec3d::new(pos.x as f64, pos.y as f64, pos.z as f64);
    let max = Vec3d::new(min.x + 1.0, min.y + 1.0, min.z + 1.0);
    let mut t_min = 0.0;
    let mut t_max = 1.0;
    let mut face = None;

    if !clip_axis(
        from.x,
        delta.x,
        min.x,
        max.x,
        Direction::West,
        Direction::East,
        &mut t_min,
        &mut t_max,
        &mut face,
    ) || !clip_axis(
        from.y,
        delta.y,
        min.y,
        max.y,
        Direction::Down,
        Direction::Up,
        &mut t_min,
        &mut t_max,
        &mut face,
    ) || !clip_axis(
        from.z,
        delta.z,
        min.z,
        max.z,
        Direction::North,
        Direction::South,
        &mut t_min,
        &mut t_max,
        &mut face,
    ) {
        return None;
    }

    face.map(|direction| BlockHitResult::new(from.add(delta.scale(t_min)), direction, pos, false))
}

#[allow(clippy::too_many_arguments)]
fn clip_axis(
    origin: f64,
    delta: f64,
    min: f64,
    max: f64,
    low_face: Direction,
    high_face: Direction,
    t_min: &mut f64,
    t_max: &mut f64,
    face: &mut Option<Direction>,
) -> bool {
    const EPSILON: f64 = 1.0e-7;
    if delta.abs() < EPSILON {
        return origin >= min && origin <= max;
    }

    let inv = 1.0 / delta;
    let mut near = (min - origin) * inv;
    let mut far = (max - origin) * inv;
    let mut near_face = low_face;
    if near > far {
        std::mem::swap(&mut near, &mut far);
        near_face = high_face;
    }
    if near > *t_min {
        *t_min = near;
        *face = Some(near_face);
    }
    *t_max = t_max.min(far);
    *t_min <= *t_max && *t_max >= 0.0 && *t_min <= 1.0
}

fn contains_unit_block(pos: BlockPos, point: Vec3d) -> bool {
    point.x >= pos.x as f64
        && point.x < pos.x as f64 + 1.0
        && point.y >= pos.y as f64
        && point.y < pos.y as f64 + 1.0
        && point.z >= pos.z as f64
        && point.z < pos.z as f64 + 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{
        CHUNK_SECTION_VOLUME, ChunkPos, ChunkRevision, ChunkSnapshot, ChunkStatus, SECTION_HEIGHT,
    };

    fn client_with_blocks(blocks: &[(BlockPos, BlockStateId)]) -> ClientRuntime {
        let mut section_blocks = vec![AIR_BLOCK_STATE_ID; CHUNK_SECTION_VOLUME];
        for (pos, state) in blocks {
            assert_eq!(pos.chunk_pos(), ChunkPos::new(0, 0));
            assert!((0..SECTION_HEIGHT).contains(&pos.y));
            let index = chunk_section_index(pos.x, pos.y, pos.z);
            section_blocks[index] = *state;
        }
        let snapshot = ChunkSnapshot::from_block_state_ids(
            ChunkPos::new(0, 0),
            ChunkStatus::Full,
            ChunkRevision(1),
            0,
            SECTION_HEIGHT,
            &section_blocks,
        );
        let mut client = ClientRuntime::local_integrated();
        client.apply_update(mclone_protocol::ServerUpdate::ChunkSnapshot(snapshot));
        client
    }

    #[test]
    fn client_runtime_reads_world_block_state_from_loaded_snapshot() {
        let client = client_with_blocks(&[(BlockPos::new(1, 2, 3), BlockStateId(7))]);

        assert_eq!(
            client.block_state_at_block_pos(BlockPos::new(1, 2, 3)),
            Some(BlockStateId(7))
        );
        assert_eq!(
            client.block_state_at_block_pos(BlockPos::new(1, 2, 4)),
            Some(AIR_BLOCK_STATE_ID)
        );
        assert_eq!(
            client.block_state_at_block_pos(BlockPos::new(20, 2, 3)),
            None
        );
    }

    #[test]
    fn raycast_hits_first_non_air_block() {
        let client = client_with_blocks(&[
            (BlockPos::new(4, 2, 1), BlockStateId(7)),
            (BlockPos::new(6, 2, 1), BlockStateId(8)),
        ]);

        let hit = client.clip_blocks(Vec3d::new(1.5, 2.5, 1.5), Vec3d::new(8.0, 2.5, 1.5));

        assert_eq!(hit.hit_type(), HitResultType::Block);
        assert_eq!(hit.block_pos, BlockPos::new(4, 2, 1));
        assert_eq!(hit.direction, Direction::West);
        assert!(!hit.inside);
    }

    #[test]
    fn raycast_reports_inside_block() {
        let client = client_with_blocks(&[(BlockPos::new(1, 2, 3), BlockStateId(7))]);

        let hit = client.clip_blocks(Vec3d::new(1.5, 2.5, 3.5), Vec3d::new(3.0, 2.5, 3.5));

        assert_eq!(hit.hit_type(), HitResultType::Block);
        assert_eq!(hit.block_pos, BlockPos::new(1, 2, 3));
        assert_eq!(hit.direction, Direction::West);
        assert!(hit.inside);
    }

    #[test]
    fn raycast_misses_air_and_unloaded_chunks() {
        let client = client_with_blocks(&[]);

        let hit = client.clip_blocks(Vec3d::new(-2.0, 2.5, 1.5), Vec3d::new(30.0, 2.5, 1.5));

        assert_eq!(hit.hit_type(), HitResultType::Miss);
        assert_eq!(hit.block_pos, BlockPos::new(30, 2, 1));
    }

    #[test]
    fn interaction_controller_builds_debug_commands_only_for_block_hits() {
        let controller = ClientInteractionController::new();
        let block_hit = BlockHitResult::new(
            Vec3d::new(1.0, 2.0, 3.0),
            Direction::North,
            BlockPos::new(1, 2, 3),
            false,
        );
        let miss = BlockHitResult::miss(Vec3d::ZERO, Direction::North, BlockPos::ZERO);

        assert!(matches!(
            controller.debug_instant_break_command(block_hit),
            Some(ClientCommand::PlayerAction(PlayerActionCommand {
                kind: PlayerActionKind::DebugInstantBreak,
                ..
            }))
        ));
        assert!(controller.debug_place_block_command(block_hit).is_some());
        assert_eq!(controller.debug_instant_break_command(miss), None);
        assert_eq!(controller.debug_place_block_command(miss), None);
    }
}
