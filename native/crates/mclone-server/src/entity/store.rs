use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{BlockPos, BlockStateId, ChunkPos, Vec3d};
#[cfg(feature = "physics-rapier")]
use mclone_protocol::EntityRotation;
use mclone_protocol::{EntityId, EntityKind};

use super::ServerEntityState;
use super::metadata::{EntityMetadata, PASSIVE_MOB_KINDS};
use super::mob::{MobPlayerTarget, MobRuntimeState};
use super::tick_list::ServerEntityTickList;

#[derive(Debug, Default)]
pub(crate) struct ServerEntityStore {
    entities: BTreeMap<EntityId, ServerEntityState>,
    mobs: BTreeMap<EntityId, MobRuntimeState>,
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
        let mut updated = Vec::new();
        for id in self.tick_list.iteration_ids() {
            if let Some(entity) = self.entities.get_mut(&id) {
                if let Some(mob) = self.mobs.get_mut(&id) {
                    mob.tick_entity(entity, nearby_players, &block_state_at);
                }
                entity.age_ticks = entity.age_ticks.saturating_add(1);
                updated.push(*entity);
            }
        }
        updated
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::metadata::{EntityDimensions, EntityMetadata, PASSIVE_MOB_KINDS};
    use crate::entity::mob::BlockPathType;

    fn no_blocks(_pos: BlockPos) -> Option<BlockStateId> {
        None
    }

    fn flat_ground(pos: BlockPos) -> Option<BlockStateId> {
        (pos.y == 63).then_some(BlockStateId(1))
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
    }

    #[test]
    fn debug_passive_showcase_can_be_disabled() {
        let mut store = ServerEntityStore::default();

        let ids = store.ensure_debug_passive_showcase_near_spawn(Vec3d::new(8.0, 64.0, 8.0), false);

        assert!(ids.is_empty());
        assert_eq!(store.diagnostics().stored_entities, 0);
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
        assert_eq!(mob.available_goal_count(), 0);
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
}
