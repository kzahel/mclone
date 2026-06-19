use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockHitResult, BlockPos, BlockStateId, Direction, HitResultType, Vec3d,
    block_to_section_coord, chunk_section_index, local_block_coord, local_section_block_coord,
};
use mclone_protocol::{
    ClientCommand, InteractionHand, PlayerActionCommand, PlayerActionKind, UseItemOnCommand,
};

use crate::{ClientInventory, ClientRuntime, block_shapes::clip_block_outline};

pub const CREATIVE_PICK_RANGE: f64 = 5.0;

#[derive(Clone, Debug, PartialEq)]
pub struct ClientInteractionController {
    pick_range: f64,
    inventory: ClientInventory,
}

impl Default for ClientInteractionController {
    fn default() -> Self {
        Self {
            pick_range: CREATIVE_PICK_RANGE,
            inventory: ClientInventory::new(),
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

    pub const fn selected_hotbar_slot(&self) -> u8 {
        self.inventory.selected_hotbar_slot()
    }

    pub fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        self.inventory.select_hotbar_slot(slot)
    }

    pub fn ensure_has_sent_carried_item(&mut self) -> Option<ClientCommand> {
        self.inventory.ensure_has_sent_carried_item()
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

    pub fn use_item_on_command(&self, hit: BlockHitResult) -> Option<ClientCommand> {
        (hit.hit_type() == HitResultType::Block).then_some(ClientCommand::UseItemOn(
            UseItemOnCommand {
                hand: InteractionHand::MainHand,
                hit,
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
        clip_block_outline(state, from, to, pos)
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
    fn raycast_uses_java_outline_shape_height() {
        let client = client_with_blocks(&[(BlockPos::new(4, 2, 1), BlockStateId(8))]);

        let low_hit = client.clip_blocks(Vec3d::new(1.5, 2.05, 1.5), Vec3d::new(8.0, 2.05, 1.5));
        let high_hit = client.clip_blocks(Vec3d::new(1.5, 2.2, 1.5), Vec3d::new(8.0, 2.2, 1.5));

        assert_eq!(low_hit.hit_type(), HitResultType::Block);
        assert_eq!(low_hit.block_pos, BlockPos::new(4, 2, 1));
        assert_eq!(low_hit.direction, Direction::West);
        assert_eq!(high_hit.hit_type(), HitResultType::Miss);
    }

    #[test]
    fn raycast_skips_empty_outline_blocks() {
        let client = client_with_blocks(&[
            (BlockPos::new(4, 2, 1), BlockStateId(2)),
            (BlockPos::new(6, 2, 1), BlockStateId(7)),
        ]);

        let hit = client.clip_blocks(Vec3d::new(1.5, 2.5, 1.5), Vec3d::new(8.0, 2.5, 1.5));

        assert_eq!(hit.hit_type(), HitResultType::Block);
        assert_eq!(hit.block_pos, BlockPos::new(6, 2, 1));
        assert_eq!(hit.direction, Direction::West);
    }

    #[test]
    fn raycast_hits_java_plant_outline_box() {
        let client = client_with_blocks(&[(BlockPos::new(4, 2, 1), BlockStateId(43))]);

        let hit = client.clip_blocks(Vec3d::new(1.5, 2.5, 1.5), Vec3d::new(8.0, 2.5, 1.5));

        assert_eq!(hit.hit_type(), HitResultType::Block);
        assert_eq!(hit.block_pos, BlockPos::new(4, 2, 1));
        assert_eq!(hit.direction, Direction::West);
        assert!((hit.location.x - 4.125).abs() < 1.0e-12);
        assert!((hit.location.y - 2.5).abs() < 1.0e-12);
        assert!((hit.location.z - 1.5).abs() < 1.0e-12);
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
        assert!(matches!(
            controller.use_item_on_command(block_hit),
            Some(ClientCommand::UseItemOn(UseItemOnCommand {
                hand: InteractionHand::MainHand,
                ..
            }))
        ));
        assert_eq!(controller.debug_instant_break_command(miss), None);
        assert_eq!(controller.use_item_on_command(miss), None);
    }

    #[test]
    fn interaction_controller_tracks_selected_hotbar_slot_and_syncs_when_changed() {
        let mut controller = ClientInteractionController::new();

        assert_eq!(controller.selected_hotbar_slot(), 0);
        assert_eq!(controller.ensure_has_sent_carried_item(), None);
        assert!(controller.select_hotbar_slot(3));
        assert_eq!(controller.selected_hotbar_slot(), 3);
        assert_eq!(
            controller.ensure_has_sent_carried_item(),
            Some(ClientCommand::SetCarriedItem(
                mclone_protocol::SetCarriedItemCommand { slot: 3 }
            ))
        );
        assert_eq!(controller.ensure_has_sent_carried_item(), None);
        assert!(!controller.select_hotbar_slot(mclone_protocol::HOTBAR_SLOT_COUNT));
    }
}
