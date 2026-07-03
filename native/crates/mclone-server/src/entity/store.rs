use std::collections::{BTreeMap, BTreeSet};

use mclone_blocks::collision_aabb_for_feet_position;
use mclone_core::{Aabb, BlockPos, BlockStateId, ChunkPos, Vec3d};
#[cfg(feature = "physics-rapier")]
use mclone_protocol::EntityRotation;
use mclone_protocol::{EntityId, EntityKind, ItemKind, ItemStackSnapshot};

use crate::players::ServerPlayerId;

use super::ServerEntityState;
use super::item::{ITEM_ENTITY_LIFETIME_TICKS, ItemEntityRuntimeState};
use super::metadata::{EntityMetadata, PASSIVE_MOB_KINDS};
use super::mob::{MobPlayerTarget, MobRuntimeState};
use super::spawning::mob_category::MobCategory;
use super::spawning::spawn_state::MobCategoryCounts;
use super::tick_list::ServerEntityTickList;

const PLAYER_PICKUP_WIDTH: f64 = 0.6;
const PLAYER_PICKUP_HEIGHT: f64 = 1.8;
const PLAYER_PICKUP_INFLATE_XZ: f64 = 1.0;
const PLAYER_PICKUP_INFLATE_Y: f64 = 0.5;
const ITEM_MERGE_INFLATE_XZ: f64 = 0.5;
const ITEM_STATIONARY_MERGE_INTERVAL_TICKS: u64 = 40;
const ITEM_MOVED_BLOCK_MERGE_INTERVAL_TICKS: u64 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ItemPickupTarget {
    pub(crate) player_id: ServerPlayerId,
    pub(crate) position: Vec3d,
}

#[derive(Debug, Default)]
pub(crate) struct ServerEntityStore {
    entities: BTreeMap<EntityId, ServerEntityState>,
    mobs: BTreeMap<EntityId, MobRuntimeState>,
    items: BTreeMap<EntityId, ItemEntityRuntimeState>,
    tick_list: ServerEntityTickList,
    next_entity_id: u64,
    debug_passive_showcase_ids: Vec<(EntityKind, EntityId)>,
    #[cfg(feature = "physics-rapier")]
    debug_physics_cube_id: Option<EntityId>,
}

#[cfg(feature = "physics-rapier")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DebugPhysicsCubeEntitySpawn {
    pub(crate) removed: Option<ServerEntityState>,
    pub(crate) current: ServerEntityState,
}

impl ServerEntityStore {
    pub(crate) fn ensure_debug_passive_showcase_near_spawn(
        &mut self,
        spawn_position: Vec3d,
        enabled: bool,
    ) -> Vec<EntityId> {
        if !enabled {
            return Vec::new();
        }

        let mut ids = Vec::with_capacity(PASSIVE_MOB_KINDS.len());
        for (index, kind) in PASSIVE_MOB_KINDS.iter().copied().enumerate() {
            if let Some(id) = self
                .debug_passive_showcase_ids
                .iter()
                .find_map(|(stored_kind, id)| (*stored_kind == kind).then_some(*id))
            {
                ids.push(id);
                continue;
            }

            let id = self.allocate_entity_id();
            let position = debug_passive_showcase_position(spawn_position, index);
            let y_rot_degrees = debug_passive_showcase_y_rot(index);
            self.insert_passive_mob(id, kind, position, y_rot_degrees);
            self.debug_passive_showcase_ids.push((kind, id));
            ids.push(id);
        }
        ids
    }

    #[cfg(test)]
    pub(crate) fn insert_passive_mob_for_test(
        &mut self,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> EntityId {
        let id = self.allocate_entity_id();
        self.insert_passive_mob(id, kind, position, y_rot_degrees);
        id
    }

    #[cfg(feature = "physics-rapier")]
    pub(crate) fn spawn_debug_physics_cube(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        rotation: EntityRotation,
        age_ticks: u64,
    ) -> DebugPhysicsCubeEntitySpawn {
        let removed = self
            .debug_physics_cube_id
            .take()
            .and_then(|id| self.entities.remove(&id))
            .map(|mut state| {
                state.alive = false;
                state
            });
        let current = self.insert_debug_physics_cube(
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            age_ticks,
        );
        DebugPhysicsCubeEntitySpawn { removed, current }
    }

    #[cfg(feature = "physics-rapier")]
    pub(crate) fn upsert_debug_physics_cube(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        rotation: EntityRotation,
        age_ticks: u64,
    ) -> ServerEntityState {
        if let Some(id) = self.debug_physics_cube_id {
            let state = debug_physics_cube_state(
                id,
                position,
                y_rot_degrees,
                x_rot_degrees,
                rotation,
                age_ticks,
            );
            self.entities.insert(id, state);
            return state;
        }
        self.insert_debug_physics_cube(position, y_rot_degrees, x_rot_degrees, rotation, age_ticks)
    }

    #[cfg(feature = "physics-rapier")]
    fn insert_debug_physics_cube(
        &mut self,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        rotation: EntityRotation,
        age_ticks: u64,
    ) -> ServerEntityState {
        let id = self.allocate_entity_id();
        self.debug_physics_cube_id = Some(id);
        let state = debug_physics_cube_state(
            id,
            position,
            y_rot_degrees,
            x_rot_degrees,
            rotation,
            age_ticks,
        );
        self.entities.insert(id, state);
        state
    }

    pub(crate) fn states(&self) -> Vec<ServerEntityState> {
        self.entities.values().copied().collect()
    }

    pub(crate) fn natural_spawn_category_counts(&self) -> MobCategoryCounts {
        let mut counts = MobCategoryCounts::new();
        for entity in self
            .entities
            .values()
            .copied()
            .filter(|entity| entity.alive)
        {
            let Some(metadata) = EntityMetadata::for_kind(entity.kind) else {
                continue;
            };
            let category = MobCategory::from_entity_category(metadata.category);
            if category.is_natural_spawning() {
                counts.increment(category);
            }
        }
        counts
    }

    pub(crate) fn spawn_volatile_passive_mob(
        &mut self,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let id = self.allocate_entity_id();
        self.insert_passive_mob(id, kind, position, y_rot_degrees)
    }

    pub(crate) fn on_block_changed(&mut self, pos: BlockPos) -> usize {
        let mut affected_mobs = 0;
        for (id, mob) in &mut self.mobs {
            let Some(entity) = self.entities.get(id).copied() else {
                continue;
            };
            if mob.on_block_changed(entity, pos) {
                affected_mobs += 1;
            }
        }
        affected_mobs
    }

    pub(crate) fn on_blocks_changed(&mut self, positions: &[BlockPos]) -> usize {
        positions
            .iter()
            .copied()
            .map(|pos| self.on_block_changed(pos))
            .sum()
    }

    #[allow(dead_code)]
    pub(crate) fn diagnostics(&self) -> ServerEntityStoreDiagnostics {
        ServerEntityStoreDiagnostics {
            stored_entities: self.entities.len(),
            ticking_entities: self.tick_list.len(),
        }
    }

    #[cfg(test)]
    pub(crate) fn state(&self, id: EntityId) -> Option<ServerEntityState> {
        self.entities.get(&id).copied()
    }

    #[cfg(test)]
    pub(crate) fn mob_state(&self, id: EntityId) -> Option<&MobRuntimeState> {
        self.mobs.get(&id)
    }

    #[cfg(test)]
    pub(crate) fn item_state(&self, id: EntityId) -> Option<&ItemEntityRuntimeState> {
        self.items.get(&id)
    }

    #[cfg(test)]
    pub(crate) fn set_item_pickup_delay_for_test(&mut self, id: EntityId, pickup_delay: i32) {
        if let Some(item) = self.items.get_mut(&id) {
            item.set_pickup_delay_for_test(pickup_delay);
        }
    }

    pub(crate) fn tick_stationary<F>(
        &mut self,
        entity_ticking_chunks: &[ChunkPos],
        nearby_players: &[MobPlayerTarget],
        block_state_at: F,
    ) -> Vec<ServerEntityState>
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        let entity_ticking_chunks = entity_ticking_chunks
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let debug_physics_cube_id = self.debug_physics_cube_id();
        self.tick_list.reconcile(
            self.entities
                .values()
                .copied()
                .filter(|entity| Some(entity.id) != debug_physics_cube_id),
            &entity_ticking_chunks,
        );
        let ticking_ids = self.tick_list.iteration_ids();
        let mut updated = Vec::new();
        let mut egg_spawns = Vec::new();
        let mut merge_due_ids = Vec::new();
        let mut removed_ids = Vec::new();
        for id in &ticking_ids {
            let id = *id;
            if let Some(entity) = self.entities.get_mut(&id) {
                if let Some(mob) = self.mobs.get_mut(&id) {
                    mob.tick_entity(entity, nearby_players, &block_state_at);
                    let egg_count = mob.take_chicken_pending_egg_lays();
                    egg_spawns
                        .extend((0..egg_count).map(|_| (entity.position, entity.y_rot_degrees)));
                }
                let mut item_block_changed = false;
                let mut is_item = false;
                if let Some(item) = self.items.get_mut(&id) {
                    is_item = true;
                    let previous_position = entity.position;
                    item.tick_entity(entity, &block_state_at);
                    item_block_changed = BlockPos::containing(previous_position)
                        != BlockPos::containing(entity.position);
                    let next_age = entity.age_ticks.saturating_add(1);
                    if next_age >= ITEM_ENTITY_LIFETIME_TICKS {
                        entity.alive = false;
                        removed_ids.push(id);
                    }
                }
                entity.age_ticks = entity.age_ticks.saturating_add(1);
                if is_item && entity.alive && item_merge_due(entity.age_ticks, item_block_changed) {
                    merge_due_ids.push(id);
                }
                updated.push(*entity);
            }
        }
        for id in removed_ids {
            self.entities.remove(&id);
            self.items.remove(&id);
        }
        updated.extend(self.merge_item_entities(&merge_due_ids));
        for (position, y_rot_degrees) in egg_spawns {
            updated.push(self.insert_item_entity(
                ItemStackSnapshot {
                    kind: ItemKind::Egg,
                    count: 1,
                },
                position,
                y_rot_degrees,
            ));
        }
        updated
    }

    pub(crate) fn collect_item_entities<F>(
        &mut self,
        pickup_targets: &[ItemPickupTarget],
        mut accept_stack: F,
    ) -> Vec<ServerEntityState>
    where
        F: FnMut(ServerPlayerId, ItemStackSnapshot) -> Option<ItemStackSnapshot>,
    {
        let item_ids = self.items.keys().copied().collect::<Vec<_>>();
        let mut updated = Vec::new();
        let mut removed_ids = Vec::new();
        for id in item_ids {
            let Some(entity) = self.entities.get(&id).copied() else {
                continue;
            };
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if !entity.alive || !item.can_pick_up() {
                continue;
            }
            let item_box = entity_aabb(entity);
            for target in pickup_targets {
                if !player_pickup_box(target.position).intersects(item_box) {
                    continue;
                }
                let Some(item) = self.items.get_mut(&id) else {
                    break;
                };
                let stack = item.stack();
                let remaining = accept_stack(target.player_id, stack);
                if remaining == Some(stack) {
                    continue;
                }
                let Some(entity) = self.entities.get_mut(&id) else {
                    break;
                };
                match remaining {
                    Some(remaining) => {
                        item.replace_stack(remaining);
                        entity.item_stack = Some(remaining);
                        updated.push(*entity);
                    }
                    None => {
                        entity.alive = false;
                        updated.push(*entity);
                        removed_ids.push(id);
                    }
                }
                break;
            }
        }
        for id in removed_ids {
            self.entities.remove(&id);
            self.items.remove(&id);
        }
        updated
    }

    fn merge_item_entities(&mut self, ticking_ids: &[EntityId]) -> Vec<ServerEntityState> {
        let mut updated = Vec::new();
        let mut removed_ids = BTreeSet::new();
        for id in ticking_ids {
            if removed_ids.contains(id) {
                continue;
            }
            let Some(entity) = self.entities.get(id).copied() else {
                continue;
            };
            let Some(item) = self.items.get(id) else {
                continue;
            };
            if !item.is_mergeable(entity) {
                continue;
            }
            let merge_box = item_merge_box(entity);
            let candidates = self.items.keys().copied().collect::<Vec<_>>();
            for other_id in candidates {
                if other_id == *id || removed_ids.contains(&other_id) {
                    continue;
                }
                let Some(other_entity) = self.entities.get(&other_id).copied() else {
                    continue;
                };
                if !merge_box.intersects(entity_aabb(other_entity)) {
                    continue;
                }
                if let Some((target, removed, target_update)) = self.merge_item_pair(*id, other_id)
                {
                    removed_ids.insert(removed.id);
                    updated.push(removed);
                    updated.push(target_update);
                    if target != *id {
                        break;
                    }
                }
            }
        }
        for id in removed_ids {
            self.entities.remove(&id);
            self.items.remove(&id);
        }
        updated
    }

    fn merge_item_pair(
        &mut self,
        left_id: EntityId,
        right_id: EntityId,
    ) -> Option<(EntityId, ServerEntityState, ServerEntityState)> {
        let left_entity = self.entities.get(&left_id).copied()?;
        let right_entity = self.entities.get(&right_id).copied()?;
        let left_item = self.items.get(&left_id)?.clone();
        let right_item = self.items.get(&right_id)?.clone();
        if !left_item.is_mergeable(left_entity) || !right_item.is_mergeable(right_entity) {
            return None;
        }

        let (target_id, source_id, target_item, source_item) =
            if right_item.stack().count < left_item.stack().count {
                (left_id, right_id, left_item, right_item)
            } else {
                (right_id, left_id, right_item, left_item)
            };
        let (merged_stack, pickup_delay) = target_item.merged_with(&source_item)?;

        let source_age = self.entities.get(&source_id)?.age_ticks;
        let target_entity = self.entities.get_mut(&target_id)?;
        target_entity.age_ticks = target_entity.age_ticks.min(source_age);
        target_entity.item_stack = Some(merged_stack);
        let target_update = *target_entity;

        let target_runtime = self.items.get_mut(&target_id)?;
        target_runtime.replace_stack(merged_stack);
        target_runtime.set_pickup_delay(pickup_delay);

        let removed = self.entities.get_mut(&source_id)?;
        removed.alive = false;
        let removed = *removed;
        Some((target_id, removed, target_update))
    }

    fn allocate_entity_id(&mut self) -> EntityId {
        self.next_entity_id = self.next_entity_id.saturating_add(1);
        EntityId(self.next_entity_id)
    }

    fn insert_passive_mob(
        &mut self,
        id: EntityId,
        kind: EntityKind,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let metadata = EntityMetadata::for_kind(kind).expect("passive mob metadata must exist");
        debug_assert!(metadata.is_passive_mob());
        let state = ServerEntityState::from_metadata(
            id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            true,
        );
        self.mobs.insert(
            id,
            MobRuntimeState::from_spawn(id, metadata, state.on_ground, state.y_rot_degrees),
        );
        self.entities.insert(id, state);
        state
    }

    fn insert_item_entity(
        &mut self,
        stack: ItemStackSnapshot,
        position: Vec3d,
        y_rot_degrees: f32,
    ) -> ServerEntityState {
        let id = self.allocate_entity_id();
        let metadata = EntityMetadata::for_kind(EntityKind::Item).expect("item metadata");
        let mut state = ServerEntityState::from_metadata(
            id,
            metadata,
            position,
            y_rot_degrees,
            0.0,
            None,
            false,
        );
        state.item_stack = Some(stack);
        let item = ItemEntityRuntimeState::from_spawn(id, stack);
        debug_assert_eq!(item.stack(), stack);
        self.items.insert(id, item);
        self.entities.insert(id, state);
        state
    }

    #[cfg(test)]
    pub(crate) fn insert_item_entity_for_test(
        &mut self,
        stack: ItemStackSnapshot,
        position: Vec3d,
    ) -> EntityId {
        self.insert_item_entity(stack, position, 0.0).id
    }

    fn debug_physics_cube_id(&self) -> Option<EntityId> {
        #[cfg(feature = "physics-rapier")]
        {
            self.debug_physics_cube_id
        }
        #[cfg(not(feature = "physics-rapier"))]
        {
            None
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ServerEntityStoreDiagnostics {
    pub(crate) stored_entities: usize,
    pub(crate) ticking_entities: usize,
}

#[cfg(feature = "physics-rapier")]
fn debug_physics_cube_state(
    id: EntityId,
    position: Vec3d,
    y_rot_degrees: f32,
    x_rot_degrees: f32,
    rotation: EntityRotation,
    age_ticks: u64,
) -> ServerEntityState {
    ServerEntityState {
        id,
        kind: EntityKind::DebugCube,
        item_stack: None,
        position,
        y_rot_degrees,
        x_rot_degrees,
        rotation: Some(rotation),
        on_ground: false,
        width: 1.0,
        height: 1.0,
        age_ticks,
        alive: true,
    }
}

fn debug_passive_showcase_position(spawn_position: Vec3d, index: usize) -> Vec3d {
    let chunk = BlockPos::containing(spawn_position).chunk_pos();
    let offset = debug_passive_showcase_offset(index);
    Vec3d::new(
        offset_within_chunk(spawn_position.x + offset.x, chunk.min_block_x()),
        spawn_position.y,
        offset_within_chunk(spawn_position.z + offset.z, chunk.min_block_z()),
    )
}

fn debug_passive_showcase_offset(index: usize) -> Vec3d {
    const OFFSETS: [Vec3d; 8] = [
        Vec3d::new(2.5, 0.0, 2.5),
        Vec3d::new(-2.5, 0.0, 2.5),
        Vec3d::new(2.5, 0.0, -2.5),
        Vec3d::new(-2.5, 0.0, -2.5),
        Vec3d::new(4.5, 0.0, 0.0),
        Vec3d::new(-4.5, 0.0, 0.0),
        Vec3d::new(0.0, 0.0, 4.5),
        Vec3d::new(0.0, 0.0, -4.5),
    ];
    OFFSETS[index % OFFSETS.len()]
}

fn debug_passive_showcase_y_rot(index: usize) -> f32 {
    45.0 + (index % 8) as f32 * 45.0
}

fn offset_within_chunk(value: f64, chunk_min: i32) -> f64 {
    (value + 2.5).clamp(chunk_min as f64 + 1.5, chunk_min as f64 + 14.5)
}

fn entity_aabb(entity: ServerEntityState) -> Aabb {
    collision_aabb_for_feet_position(
        entity.position,
        f64::from(entity.width),
        f64::from(entity.height),
    )
}

fn item_merge_box(entity: ServerEntityState) -> Aabb {
    let aabb = entity_aabb(entity);
    Aabb::new(
        aabb.min_x - ITEM_MERGE_INFLATE_XZ,
        aabb.min_y,
        aabb.min_z - ITEM_MERGE_INFLATE_XZ,
        aabb.max_x + ITEM_MERGE_INFLATE_XZ,
        aabb.max_y,
        aabb.max_z + ITEM_MERGE_INFLATE_XZ,
    )
}

fn player_pickup_box(position: Vec3d) -> Aabb {
    let aabb =
        collision_aabb_for_feet_position(position, PLAYER_PICKUP_WIDTH, PLAYER_PICKUP_HEIGHT);
    Aabb::new(
        aabb.min_x - PLAYER_PICKUP_INFLATE_XZ,
        aabb.min_y - PLAYER_PICKUP_INFLATE_Y,
        aabb.min_z - PLAYER_PICKUP_INFLATE_XZ,
        aabb.max_x + PLAYER_PICKUP_INFLATE_XZ,
        aabb.max_y + PLAYER_PICKUP_INFLATE_Y,
        aabb.max_z + PLAYER_PICKUP_INFLATE_XZ,
    )
}

fn item_merge_due(age_ticks: u64, block_position_changed: bool) -> bool {
    let interval = if block_position_changed {
        ITEM_MOVED_BLOCK_MERGE_INTERVAL_TICKS
    } else {
        ITEM_STATIONARY_MERGE_INTERVAL_TICKS
    };
    age_ticks % interval == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::metadata::{EntityDimensions, EntityMetadata, PASSIVE_MOB_KINDS};
    use crate::entity::mob::BlockPathType;

    fn no_blocks(_pos: BlockPos) -> Option<BlockStateId> {
        None
    }

    fn flat_ground(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    #[test]
    fn debug_passive_showcase_spawns_once_near_initial_spawn() {
        let mut store = ServerEntityStore::default();
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();

        let first =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(15.0, 64.0, 15.0), true);
        let second =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(1.0, 64.0, 1.0), true);

        assert_eq!(first, second);
        assert_eq!(first.len(), PASSIVE_MOB_KINDS.len());
        let entity = store.state(first[0]).unwrap();
        assert_eq!(entity.kind, EntityKind::Cow);
        assert_eq!(entity.width, metadata.dimensions.width);
        assert_eq!(entity.height, metadata.dimensions.height);
        assert_eq!(entity.position, Vec3d::new(14.5, 64.0, 14.5));
        let mob = store.mob_state(first[0]).expect("starter cow mob state");
        assert_eq!(mob.movement_speed(), metadata.movement_speed);
        assert_eq!(mob.eye_height(), metadata.standing_eye_height() as f64);
        assert_eq!(mob.pathfinding_malus(BlockPathType::Water), 8.0);
        assert_eq!(mob.available_goal_count(), 3);

        let chicken = store.state(first[1]).unwrap();
        assert_eq!(chicken.kind, EntityKind::Chicken);
        let chicken_mob = store
            .mob_state(first[1])
            .expect("starter chicken mob state");
        assert_eq!(chicken_mob.available_goal_count(), 3);
    }

    #[test]
    fn debug_passive_showcase_can_be_disabled() {
        let mut store = ServerEntityStore::default();

        let ids = store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), false);

        assert!(ids.is_empty());
        assert_eq!(store.diagnostics().stored_entities, 0);
    }

    #[test]
    fn natural_spawn_category_counts_include_live_passive_mobs_but_not_items_or_misc() {
        let mut store = ServerEntityStore::default();
        store.insert_passive_mob_for_test(EntityKind::Cow, Vec3d::new(4.0, 64.0, 4.0), 0.0);
        store.insert_passive_mob_for_test(EntityKind::Chicken, Vec3d::new(5.0, 64.0, 4.0), 0.0);
        store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(6.0, 64.0, 4.0),
        );

        let counts = store.natural_spawn_category_counts();

        assert_eq!(counts.get(MobCategory::Creature), 2);
        assert_eq!(counts.get(MobCategory::Misc), 0);
    }

    #[test]
    fn volatile_passive_spawn_uses_normal_mob_runtime_and_counts_as_creature() {
        let mut store = ServerEntityStore::default();
        let state =
            store.spawn_volatile_passive_mob(EntityKind::Chicken, Vec3d::new(8.5, 64.0, 8.5), 90.0);

        assert_eq!(state.kind, EntityKind::Chicken);
        assert_eq!(state.position, Vec3d::new(8.5, 64.0, 8.5));
        assert!(state.on_ground);
        assert!(store.mob_state(state.id).is_some());
        assert_eq!(
            store
                .natural_spawn_category_counts()
                .get(MobCategory::Creature),
            1
        );
    }

    #[test]
    fn chicken_passive_mob_can_snapshot_metadata_dimensions() {
        let mut store = ServerEntityStore::default();
        let metadata = EntityMetadata::for_kind(EntityKind::Chicken).unwrap();

        let id =
            store.insert_passive_mob_for_test(EntityKind::Chicken, Vec3d::new(4.0, 64.0, 4.0), 0.0);
        let snapshot = store.state(id).unwrap().snapshot();

        assert_eq!(snapshot.kind, EntityKind::Chicken);
        assert_eq!(snapshot.width, metadata.dimensions.width);
        assert_eq!(snapshot.height, metadata.dimensions.height);
        assert_eq!(metadata.dimensions, EntityDimensions::scalable(0.4, 0.7));
        assert_eq!(metadata.standing_eye_height(), snapshot.height * 0.92);
        let mob = store.mob_state(id).expect("chicken mob state");
        assert_eq!(mob.movement_speed(), metadata.movement_speed);
        assert_eq!(mob.pathfinding_malus(BlockPathType::Water), 0.0);
        assert_eq!(mob.available_goal_count(), 3);
    }

    #[test]
    fn item_entity_can_snapshot_stack_metadata() {
        let mut store = ServerEntityStore::default();
        let metadata = EntityMetadata::for_kind(EntityKind::Item).unwrap();
        let stack = ItemStackSnapshot {
            kind: ItemKind::Egg,
            count: 1,
        };

        let id = store.insert_item_entity_for_test(stack, Vec3d::new(4.0, 64.0, 4.0));
        let snapshot = store.state(id).unwrap().snapshot();

        assert_eq!(snapshot.kind, EntityKind::Item);
        assert_eq!(snapshot.item_stack, Some(stack));
        assert_eq!(snapshot.width, metadata.dimensions.width);
        assert_eq!(snapshot.height, metadata.dimensions.height);
        assert!(store.item_state(id).is_some());
        assert!(store.mob_state(id).is_none());
    }

    #[test]
    fn item_entity_ticks_only_in_entity_ticking_chunks() {
        let mut store = ServerEntityStore::default();
        let id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        let start = store.state(id).unwrap();

        assert!(
            store
                .tick_stationary(&[ChunkPos::new(1, 0)], &[], no_blocks)
                .is_empty()
        );
        assert_eq!(store.state(id).unwrap(), start);

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], no_blocks);
        let item = updated
            .iter()
            .find(|entity| entity.id == id)
            .expect("item should tick in entity ticking chunk");

        assert_ne!(item.position, start.position);
        assert_eq!(item.age_ticks, 1);
        assert_eq!(store.item_state(id).unwrap().pickup_delay(), 9);
    }

    #[test]
    fn item_entity_expires_after_java_lifetime() {
        let mut store = ServerEntityStore::default();
        let id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        store.entities.get_mut(&id).unwrap().age_ticks = ITEM_ENTITY_LIFETIME_TICKS - 1;

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], no_blocks);
        let removed = updated
            .iter()
            .find(|entity| entity.id == id)
            .expect("expired item should emit final update");

        assert!(!removed.alive);
        assert_eq!(store.state(id), None);
        assert_eq!(store.item_state(id), None);
    }

    #[test]
    fn item_entity_pickup_waits_for_pickup_delay() {
        let mut store = ServerEntityStore::default();
        let id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        let target = ItemPickupTarget {
            player_id: ServerPlayerId::LOCAL,
            position: Vec3d::new(4.0, 64.0, 4.0),
        };

        let blocked = store.collect_item_entities(&[target], |_player_id, _stack| None);
        assert!(blocked.is_empty());
        assert!(store.state(id).unwrap().alive);

        store
            .items
            .get_mut(&id)
            .unwrap()
            .set_pickup_delay_for_test(0);
        let mut collected = 0;
        let picked_up = store.collect_item_entities(&[target], |_player_id, stack| {
            collected += stack.count;
            None
        });

        assert_eq!(collected, 1);
        assert_eq!(picked_up.len(), 1);
        assert_eq!(picked_up[0].id, id);
        assert!(!picked_up[0].alive);
        assert_eq!(store.state(id), None);
        assert_eq!(store.item_state(id), None);
    }

    #[test]
    fn item_entity_pickup_keeps_remaining_stack_after_partial_acceptance() {
        let mut store = ServerEntityStore::default();
        let id = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 4,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        store
            .items
            .get_mut(&id)
            .unwrap()
            .set_pickup_delay_for_test(0);
        let target = ItemPickupTarget {
            player_id: ServerPlayerId::LOCAL,
            position: Vec3d::new(4.0, 64.0, 4.0),
        };

        let updated = store.collect_item_entities(&[target], |_player_id, stack| {
            Some(ItemStackSnapshot {
                count: stack.count - 2,
                ..stack
            })
        });

        assert_eq!(updated.len(), 1);
        assert!(updated[0].alive);
        assert_eq!(
            updated[0].item_stack,
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 2,
            })
        );
        assert_eq!(store.state(id), Some(updated[0]));
        assert_eq!(
            store.item_state(id).unwrap().stack(),
            updated[0].item_stack.unwrap()
        );
    }

    #[test]
    fn item_entities_merge_nearby_egg_stacks() {
        let mut store = ServerEntityStore::default();
        let first = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.0, 64.0, 4.0),
        );
        let second = store.insert_item_entity_for_test(
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            Vec3d::new(4.1, 64.0, 4.0),
        );
        store.entities.get_mut(&first).unwrap().age_ticks =
            ITEM_STATIONARY_MERGE_INTERVAL_TICKS - 1;
        store.entities.get_mut(&second).unwrap().age_ticks =
            ITEM_STATIONARY_MERGE_INTERVAL_TICKS - 1;

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], no_blocks);
        let surviving_items = store
            .states()
            .into_iter()
            .filter(|entity| entity.kind == EntityKind::Item)
            .collect::<Vec<_>>();

        assert_eq!(surviving_items.len(), 1);
        assert_eq!(
            surviving_items[0].item_stack,
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 2,
            })
        );
        assert!(updated.iter().any(|entity| {
            (entity.id == first || entity.id == second)
                && !entity.alive
                && entity.kind == EntityKind::Item
        }));
        assert!(updated.iter().any(|entity| {
            entity.id == surviving_items[0].id
                && entity.alive
                && entity.item_stack == surviving_items[0].item_stack
        }));
    }

    #[test]
    fn chicken_egg_timer_spawns_egg_item_entity() {
        let mut store = ServerEntityStore::default();
        let chicken_id =
            store.insert_passive_mob_for_test(EntityKind::Chicken, Vec3d::new(4.0, 64.0, 4.0), 0.0);
        store
            .mobs
            .get_mut(&chicken_id)
            .unwrap()
            .set_chicken_egg_time_for_test(1);

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
        let egg = updated
            .iter()
            .find(|entity| entity.kind == EntityKind::Item)
            .expect("chicken should spawn egg item");

        assert_eq!(
            egg.item_stack,
            Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            })
        );
        assert_eq!(egg.width, 0.25);
        assert_eq!(egg.height, 0.25);
        assert_eq!(egg.position.x, store.state(chicken_id).unwrap().position.x);
        assert_eq!(egg.position.z, store.state(chicken_id).unwrap().position.z);
        assert_eq!(
            store
                .mob_state(chicken_id)
                .unwrap()
                .chicken_pending_egg_lays_for_test(),
            Some(0)
        );
    }

    #[test]
    fn entity_tick_advances_age_in_entity_ticking_chunks() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];

        assert!(
            store
                .tick_stationary(&[ChunkPos::new(1, 0)], &[], flat_ground)
                .is_empty()
        );
        assert_eq!(store.state(id).unwrap().age_ticks, 0);
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: PASSIVE_MOB_KINDS.len(),
                ticking_entities: 0
            }
        );

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);

        assert_eq!(updated.len(), PASSIVE_MOB_KINDS.len());
        let updated = updated
            .iter()
            .find(|entity| entity.id == id)
            .expect("showcase cow should update");
        assert_eq!(updated.age_ticks, 1);
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: PASSIVE_MOB_KINDS.len(),
                ticking_entities: PASSIVE_MOB_KINDS.len()
            }
        );
    }

    #[test]
    fn entity_tick_list_demotes_without_removing_stored_entity() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];
        store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);

        let updated = store.tick_stationary(&[], &[], flat_ground);

        assert!(updated.is_empty());
        assert_eq!(store.state(id).unwrap().age_ticks, 1);
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: PASSIVE_MOB_KINDS.len(),
                ticking_entities: 0
            }
        );
    }

    #[test]
    fn ticking_starter_cow_eventually_applies_passive_movement() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];
        let start = store.state(id).unwrap();

        let mut moved = None;
        for _ in 0..2_000 {
            let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
            let entity = updated
                .into_iter()
                .find(|entity| entity.id == id)
                .expect("starter cow should tick while chunk is entity ticking");
            if entity.position != start.position {
                moved = Some(entity);
                break;
            }
        }

        let moved = moved.expect("cow passive AI should choose a stroll target");
        assert!(moved.age_ticks > start.age_ticks);
        assert!(moved.position.distance_to_sqr(start.position) > 0.0);
        let mob = store.mob_state(id).expect("starter cow mob state");
        assert!(mob.running_goal_count() > 0);
    }

    #[test]
    fn ticking_starter_chicken_eventually_applies_passive_movement() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[1];
        let start = store.state(id).unwrap();

        let mut moved = None;
        for _ in 0..2_000 {
            let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], flat_ground);
            let entity = updated
                .into_iter()
                .find(|entity| entity.id == id)
                .expect("starter chicken should tick while chunk is entity ticking");
            if entity.position != start.position {
                moved = Some(entity);
                break;
            }
        }

        let moved = moved.expect("chicken passive AI should choose a stroll target");
        assert!(moved.age_ticks > start.age_ticks);
        assert!(moved.position.distance_to_sqr(start.position) > 0.0);
        let mob = store.mob_state(id).expect("starter chicken mob state");
        assert!(mob.running_goal_count() > 0);
    }

    #[test]
    fn ticking_passive_without_support_falls_and_clears_ground_state() {
        let mut store = ServerEntityStore::default();
        let id =
            store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), true)[0];
        let start = store.state(id).unwrap();

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)], &[], no_blocks);

        let entity = updated
            .into_iter()
            .find(|entity| entity.id == id)
            .expect("starter cow should tick while chunk is entity ticking");
        assert!(entity.position.y < start.position.y);
        assert!(!entity.on_ground);
        let mob = store.mob_state(id).expect("starter cow mob state");
        assert!(mob.delta_movement().y < 0.0);
    }

    #[test]
    fn block_change_near_mob_path_marks_navigation_for_recompute() {
        let mut store = ServerEntityStore::default();
        let id =
            store.insert_passive_mob_for_test(EntityKind::Cow, Vec3d::new(0.5, 64.0, 0.5), 0.0);
        let entity = store.state(id).unwrap();
        let mob = store.mobs.get_mut(&id).expect("cow should have mob state");
        assert!(mob.move_to_for_test(entity, Vec3d::new(4.5, 64.0, 0.5), &flat_ground));
        assert!(!mob.navigation_has_delayed_recomputation());

        assert_eq!(store.on_block_changed(BlockPos::new(40, 64, 40)), 0);
        assert!(
            !store
                .mob_state(id)
                .unwrap()
                .navigation_has_delayed_recomputation()
        );

        assert_eq!(store.on_block_changed(BlockPos::new(2, 64, 0)), 1);
        assert!(
            store
                .mob_state(id)
                .unwrap()
                .navigation_has_delayed_recomputation()
        );
    }
}
