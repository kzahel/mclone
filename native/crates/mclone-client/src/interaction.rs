use mclone_core::{
    AIR_BLOCK_STATE_ID, Aabb, BlockHitResult, BlockPos, BlockStateId, Direction, HitResultType,
    Vec3d, block_to_section_coord, chunk_section_index, local_block_coord,
    local_section_block_coord,
};
use mclone_protocol::{
    AttackEntityCommand, ClientCommand, DebugHotbarItem, EntityId, EntityKind,
    HOTBAR_SLOT_COUNT_USIZE, InteractEntityCommand, InteractionHand, ItemKind, PlayerActionCommand,
    PlayerActionKind, UseItemOnCommand,
};

use crate::{
    ClientInventory, ClientRuntime,
    block_shapes::{block_collision_aabbs, block_outline_aabbs, clip_block_outline},
};

pub const CREATIVE_PICK_RANGE: f64 = 5.0;

#[derive(Clone, Debug, PartialEq)]
pub struct ClientInteractionController {
    pick_range: f64,
    inventory: ClientInventory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockInteractionTarget {
    pub hit: BlockHitResult,
    pub outline_boxes: Vec<Aabb>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityInteractionTarget {
    pub id: EntityId,
    pub position: Vec3d,
    pub distance: f64,
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

    pub const fn hotbar_items(&self) -> [Option<DebugHotbarItem>; HOTBAR_SLOT_COUNT_USIZE] {
        self.inventory.hotbar_items()
    }

    pub fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        self.inventory.select_hotbar_slot(slot)
    }

    pub fn set_debug_hotbar_slot(
        &mut self,
        slot: u8,
        block_state: Option<BlockStateId>,
    ) -> Option<ClientCommand> {
        self.inventory.set_debug_hotbar_slot(slot, block_state)
    }

    pub fn set_debug_hotbar_item(
        &mut self,
        slot: u8,
        item: Option<DebugHotbarItem>,
    ) -> Option<ClientCommand> {
        self.inventory.set_debug_hotbar_item(slot, item)
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

    pub fn target_block(
        &self,
        client: &ClientRuntime,
        eye_position: Vec3d,
        view_vector: Vec3d,
    ) -> Option<BlockInteractionTarget> {
        // When the ray origin is embedded inside a solid block (e.g. noclip flying
        // through terrain), suppress the target so we don't draw a block outline from
        // inside the world geometry.
        if client.is_point_inside_solid_block(eye_position) {
            return None;
        }
        self.target_from_hit(client, self.pick_block(client, eye_position, view_vector))
    }

    pub fn target_entity(
        &self,
        client: &ClientRuntime,
        eye_position: Vec3d,
        view_vector: Vec3d,
    ) -> Option<EntityInteractionTarget> {
        let selected = client.player_inventory()[usize::from(self.selected_hotbar_slot())];
        let hunting_spear = selected.is_some_and(|stack| stack.kind == ItemKind::HuntingSpear);
        let direction_length = view_vector.length_sqr().sqrt();
        if !eye_position.is_finite() || direction_length <= f64::EPSILON {
            return None;
        }
        let direction = view_vector.scale(direction_length.recip());
        let to = eye_position.add(direction.scale(self.pick_range));
        let block_distance = client
            .clip_blocks(eye_position, to)
            .location
            .distance_to_sqr(eye_position)
            .sqrt();
        client
            .entity_snapshots()
            .filter(|entity| {
                entity.kind == EntityKind::RabbitBurrow
                    || (hunting_spear
                        && entity.kind == EntityKind::Deer
                        && entity.deer.is_some_and(|deer| deer.health > 0))
            })
            .filter_map(|entity| {
                let position = client
                    .topology()
                    .nearest_position_lift(entity.position, eye_position);
                let box_center = position.add(Vec3d::new(0.0, f64::from(entity.height) * 0.5, 0.0));
                let bounds = Aabb::of_size(
                    box_center,
                    f64::from(entity.width) + 0.2,
                    f64::from(entity.height) + 0.2,
                    f64::from(entity.width) + 0.2,
                );
                let fraction = bounds.ray_intersection_fraction(eye_position, to)?;
                let distance = fraction * self.pick_range;
                (distance <= block_distance + 1.0e-6).then_some(EntityInteractionTarget {
                    id: entity.id,
                    position,
                    distance,
                })
            })
            .min_by(|left, right| left.distance.total_cmp(&right.distance))
    }

    pub const fn attack_entity_command(&self, target: EntityInteractionTarget) -> ClientCommand {
        ClientCommand::AttackEntity(AttackEntityCommand { target: target.id })
    }

    pub fn target_bee_colony(
        &self,
        client: &ClientRuntime,
        eye_position: Vec3d,
        view_vector: Vec3d,
    ) -> Option<EntityInteractionTarget> {
        self.target_entity_matching(client, eye_position, view_vector, |entity| {
            matches!(entity.kind, EntityKind::BeeNest | EntityKind::BeeHotel)
        })
    }

    pub fn target_use_entity(
        &self,
        client: &ClientRuntime,
        eye_position: Vec3d,
        view_vector: Vec3d,
    ) -> Option<EntityInteractionTarget> {
        self.target_entity_matching(client, eye_position, view_vector, |entity| {
            matches!(
                entity.kind,
                EntityKind::BeeNest | EntityKind::BeeHotel | EntityKind::Rabbit
            )
        })
    }

    fn target_entity_matching(
        &self,
        client: &ClientRuntime,
        eye_position: Vec3d,
        view_vector: Vec3d,
        predicate: impl Fn(&mclone_protocol::EntitySnapshot) -> bool,
    ) -> Option<EntityInteractionTarget> {
        let direction_length = view_vector.length_sqr().sqrt();
        if !eye_position.is_finite() || direction_length <= f64::EPSILON {
            return None;
        }
        let direction = view_vector.scale(direction_length.recip());
        let to = eye_position.add(direction.scale(self.pick_range));
        let block_distance = client
            .clip_blocks(eye_position, to)
            .location
            .distance_to_sqr(eye_position)
            .sqrt();
        client
            .entity_snapshots()
            .filter(|entity| predicate(entity))
            .filter_map(|entity| {
                let position = client
                    .topology()
                    .nearest_position_lift(entity.position, eye_position);
                let box_center = position.add(Vec3d::new(0.0, f64::from(entity.height) * 0.5, 0.0));
                let bounds = Aabb::of_size(
                    box_center,
                    f64::from(entity.width) + 0.3,
                    f64::from(entity.height) + 0.3,
                    f64::from(entity.width) + 0.3,
                );
                let fraction = bounds.ray_intersection_fraction(eye_position, to)?;
                let distance = fraction * self.pick_range;
                (distance <= block_distance + 1.0e-6).then_some(EntityInteractionTarget {
                    id: entity.id,
                    position,
                    distance,
                })
            })
            .min_by(|left, right| left.distance.total_cmp(&right.distance))
    }

    pub const fn interact_entity_command(&self, target: EntityInteractionTarget) -> ClientCommand {
        ClientCommand::InteractEntity(InteractEntityCommand {
            target: target.id,
            hand: InteractionHand::MainHand,
        })
    }

    pub fn target_from_hit(
        &self,
        client: &ClientRuntime,
        hit: BlockHitResult,
    ) -> Option<BlockInteractionTarget> {
        if hit.hit_type() != HitResultType::Block {
            return None;
        }
        let outline_boxes = client.block_outline_aabbs_at_block_pos(hit.block_pos)?;
        (!outline_boxes.is_empty()).then_some(BlockInteractionTarget { hit, outline_boxes })
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
                .map(|section| section.block_state_id_at(index))
                .unwrap_or(AIR_BLOCK_STATE_ID),
        )
    }

    pub fn block_outline_aabbs_at_block_pos(&self, pos: BlockPos) -> Option<Vec<Aabb>> {
        let state = self.block_state_at_block_pos(pos)?;
        Some(block_outline_aabbs(state, pos))
    }

    /// Returns true when `point` lies within the collision shape of the block that
    /// contains it, i.e. the point is inside solid world geometry.
    pub fn is_point_inside_solid_block(&self, point: Vec3d) -> bool {
        let pos = BlockPos::containing(point);
        let Some(state) = self.block_state_at_block_pos(pos) else {
            return false;
        };
        block_collision_aabbs(state, pos)
            .into_iter()
            .any(|aabb| aabb.contains(point))
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
    fn periodic_raycast_and_outline_use_the_observer_local_seam_lift() {
        let mut client = client_with_blocks(&[(BlockPos::new(0, 2, 1), BlockStateId(7))]);
        client.apply_update(mclone_protocol::ServerUpdate::WorldInfo {
            dimension: mclone_protocol::DimensionKey::overworld(),
            biome_zoom_seed: 12_345,
            topology: mclone_core::HorizontalTopology::cylinder_x(0, 32),
        });
        let controller = ClientInteractionController::new();

        let target = controller
            .target_block(
                &client,
                Vec3d::new(511.5, 2.5, 1.5),
                Vec3d::new(1.0, 0.0, 0.0),
            )
            .expect("wrapped block target");

        assert_eq!(target.hit.block_pos, BlockPos::new(512, 2, 1));
        assert_eq!(target.hit.direction, Direction::West);
        assert_eq!(
            target.outline_boxes,
            vec![Aabb::new(512.0, 2.0, 1.0, 513.0, 3.0, 2.0)]
        );
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
    fn interaction_target_carries_matching_outline_box() {
        let pos = BlockPos::new(4, 2, 1);
        let client = client_with_blocks(&[(pos, BlockStateId(8))]);
        let controller = ClientInteractionController::new();

        let target = controller
            .target_block(
                &client,
                Vec3d::new(1.5, 2.05, 1.5),
                Vec3d::new(1.0, 0.0, 0.0),
            )
            .expect("snow layer target");

        assert_eq!(target.hit.hit_type(), HitResultType::Block);
        assert_eq!(target.hit.block_pos, pos);
        assert_eq!(
            target.outline_boxes,
            vec![Aabb::new(4.0, 2.0, 1.0, 5.0, 2.125, 2.0)]
        );
    }

    #[test]
    fn interaction_target_suppressed_when_eye_inside_solid_block() {
        let eye_pos = BlockPos::new(4, 2, 1);
        let client = client_with_blocks(&[
            (eye_pos, BlockStateId(7)),
            (BlockPos::new(6, 2, 1), BlockStateId(8)),
        ]);
        let controller = ClientInteractionController::new();

        // Eye embedded inside the solid block at eye_pos: no outline/target.
        assert!(client.is_point_inside_solid_block(Vec3d::new(4.5, 2.5, 1.5)));
        assert_eq!(
            controller.target_block(
                &client,
                Vec3d::new(4.5, 2.5, 1.5),
                Vec3d::new(1.0, 0.0, 0.0),
            ),
            None
        );

        // Eye in open air still targets the block it looks at.
        assert!(!client.is_point_inside_solid_block(Vec3d::new(1.5, 2.5, 1.5)));
        let target = controller
            .target_block(
                &client,
                Vec3d::new(1.5, 2.5, 1.5),
                Vec3d::new(1.0, 0.0, 0.0),
            )
            .expect("air-origin ray should target a block");
        assert_eq!(target.hit.block_pos, eye_pos);
    }

    #[test]
    fn is_point_inside_solid_block_ignores_non_colliding_blocks() {
        // State 43 is a plant with an outline shape but no collision volume.
        let client = client_with_blocks(&[(BlockPos::new(4, 2, 1), BlockStateId(43))]);

        assert!(!client.is_point_inside_solid_block(Vec3d::new(4.5, 2.5, 1.5)));
    }

    #[test]
    fn interaction_target_skips_misses_and_empty_outline_blocks() {
        let client = client_with_blocks(&[]);
        let controller = ClientInteractionController::new();
        let miss = BlockHitResult::miss(Vec3d::ZERO, Direction::North, BlockPos::ZERO);

        assert_eq!(controller.target_from_hit(&client, miss), None);
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

    #[test]
    fn hunting_spear_targets_nearest_visible_living_deer() {
        let mut client = client_with_blocks(&[]);
        client.apply_update(mclone_protocol::ServerUpdate::PlayerInventory {
            hotbar: std::array::from_fn(|slot| {
                (slot == 0).then_some(mclone_protocol::ItemStackSnapshot {
                    kind: ItemKind::HuntingSpear,
                    count: 1,
                })
            }),
        });
        for (id, z) in [(EntityId(9), 3.0), (EntityId(10), 4.0)] {
            client.apply_update(mclone_protocol::ServerUpdate::EntitySnapshot(
                mclone_protocol::EntitySnapshot {
                    id,
                    persistent_id: mclone_protocol::EntityPersistentId::new(0, id.0),
                    kind: EntityKind::Deer,
                    item_stack: None,
                    mallard: None,
                    mallard_nest: None,
                    deer: Some(mclone_protocol::DeerSnapshotData {
                        sex: mclone_protocol::DeerSex::Female,
                        life_stage: mclone_protocol::DeerLifeStage::Adult,
                        antlered: false,
                        behavior: mclone_protocol::DeerBehavior::Idle,
                        health: 20,
                        max_health: 20,
                    }),
                    animation: None,
                    position: Vec3d::new(1.5, 1.0, z),
                    y_rot_degrees: 0.0,
                    x_rot_degrees: 0.0,
                    rotation: None,
                    on_ground: true,
                    width: 0.9,
                    height: 1.8,
                    tick_count: 0,
                },
            ));
        }
        let target = ClientInteractionController::new()
            .target_entity(
                &client,
                Vec3d::new(1.5, 1.62, 0.5),
                Vec3d::new(0.0, 0.0, 1.0),
            )
            .unwrap();
        assert_eq!(target.id, EntityId(9));
        assert!(target.distance < 3.0);
    }

    #[test]
    fn rabbit_burrow_is_an_attack_target_without_a_hunting_spear() {
        let mut client = client_with_blocks(&[]);
        client.apply_update(mclone_protocol::ServerUpdate::EntitySnapshot(
            mclone_protocol::EntitySnapshot {
                id: EntityId(12),
                persistent_id: mclone_protocol::EntityPersistentId::new(0, 12),
                kind: EntityKind::RabbitBurrow,
                item_stack: None,
                mallard: None,
                mallard_nest: None,
                deer: None,
                animation: None,
                position: Vec3d::new(1.5, 1.0, 3.0),
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                rotation: None,
                on_ground: true,
                width: 0.9,
                height: 0.76,
                tick_count: 0,
            },
        ));

        let target = ClientInteractionController::new()
            .target_entity(
                &client,
                Vec3d::new(1.5, 1.62, 0.5),
                Vec3d::new(0.0, 0.0, 1.0),
            )
            .expect("burrow should use the ordinary attack ray");
        assert_eq!(target.id, EntityId(12));
    }
}
