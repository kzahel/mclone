//! Server-owned entity replicas and visibility routing.
//!
//! This is intentionally narrower than Java's full `ChunkMap.TrackedEntity`
//! stack. It gives native clients authoritative snapshots for simple passive
//! actor rendering without adding natural spawning, AI, persistence, or combat.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_protocol::{
    EntityId, EntityKind, EntityRotation, EntitySnapshot, EntityUpdate, ServerUpdate,
};

use crate::players::ServerPlayerId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ServerEntityState {
    pub(crate) id: EntityId,
    pub(crate) kind: EntityKind,
    pub(crate) position: Vec3d,
    pub(crate) y_rot_degrees: f32,
    pub(crate) x_rot_degrees: f32,
    pub(crate) rotation: Option<EntityRotation>,
    pub(crate) on_ground: bool,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) age_ticks: u64,
    pub(crate) alive: bool,
}

impl ServerEntityState {
    fn chunk_pos(self) -> ChunkPos {
        BlockPos::containing(self.position).chunk_pos()
    }

    fn snapshot(self) -> EntitySnapshot {
        EntitySnapshot {
            id: self.id,
            kind: self.kind,
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            rotation: self.rotation,
            on_ground: self.on_ground,
            width: self.width,
            height: self.height,
            age_ticks: self.age_ticks,
        }
    }

    fn update(self) -> EntityUpdate {
        EntityUpdate {
            id: self.id,
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            rotation: self.rotation,
            on_ground: self.on_ground,
            age_ticks: self.age_ticks,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ServerEntityStore {
    entities: BTreeMap<EntityId, ServerEntityState>,
    next_entity_id: u64,
    starter_passive_id: Option<EntityId>,
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
    pub(crate) fn ensure_starter_passive_near_spawn(&mut self, spawn_position: Vec3d) -> EntityId {
        if let Some(id) = self.starter_passive_id {
            return id;
        }
        let id = self.allocate_entity_id();
        let position = starter_passive_position(spawn_position);
        self.entities.insert(
            id,
            ServerEntityState {
                id,
                kind: EntityKind::Cow,
                position,
                y_rot_degrees: 135.0,
                x_rot_degrees: 0.0,
                rotation: None,
                on_ground: true,
                width: 0.9,
                height: 1.4,
                age_ticks: 0,
                alive: true,
            },
        );
        self.starter_passive_id = Some(id);
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

    #[cfg(test)]
    pub(crate) fn state(&self, id: EntityId) -> Option<ServerEntityState> {
        self.entities.get(&id).copied()
    }

    pub(crate) fn tick_stationary(
        &mut self,
        entity_ticking_chunks: &[ChunkPos],
    ) -> Vec<ServerEntityState> {
        let entity_ticking_chunks = entity_ticking_chunks
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut updated = Vec::new();
        for entity in self.entities.values_mut() {
            #[cfg(feature = "physics-rapier")]
            if Some(entity.id) == self.debug_physics_cube_id {
                continue;
            }
            if entity.alive && entity_ticking_chunks.contains(&entity.chunk_pos()) {
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RoutedEntityUpdate {
    pub(crate) recipient: ServerPlayerId,
    pub(crate) update: ServerUpdate,
}

#[derive(Debug, Default)]
pub(crate) struct EntityTracking {
    seen_by_entity: BTreeMap<EntityId, BTreeSet<ServerPlayerId>>,
}

impl EntityTracking {
    pub(crate) fn remove_observer(&mut self, player_id: ServerPlayerId) {
        for observers in self.seen_by_entity.values_mut() {
            observers.remove(&player_id);
        }
    }

    pub(crate) fn reconcile_observer(
        &mut self,
        observer: ServerPlayerId,
        subjects: &[ServerEntityState],
        mut tracks_chunk: impl FnMut(ServerPlayerId, ChunkPos) -> bool,
    ) -> Vec<RoutedEntityUpdate> {
        let mut routes = Vec::new();
        for subject in subjects {
            self.reconcile_pair(observer, *subject, &mut tracks_chunk, false, &mut routes);
        }
        routes
    }

    pub(crate) fn reconcile_subject(
        &mut self,
        subject: ServerEntityState,
        observers: impl IntoIterator<Item = ServerPlayerId>,
        mut tracks_chunk: impl FnMut(ServerPlayerId, ChunkPos) -> bool,
        emit_existing_updates: bool,
    ) -> Vec<RoutedEntityUpdate> {
        let mut routes = Vec::new();
        for observer in observers {
            self.reconcile_pair(
                observer,
                subject,
                &mut tracks_chunk,
                emit_existing_updates,
                &mut routes,
            );
        }
        routes
    }

    fn reconcile_pair(
        &mut self,
        observer: ServerPlayerId,
        subject: ServerEntityState,
        tracks_chunk: &mut impl FnMut(ServerPlayerId, ChunkPos) -> bool,
        emit_existing_update: bool,
        routes: &mut Vec<RoutedEntityUpdate>,
    ) {
        let observers = self.seen_by_entity.entry(subject.id).or_default();
        let should_see = subject.alive && tracks_chunk(observer, subject.chunk_pos());
        if should_see {
            if observers.insert(observer) {
                routes.push(RoutedEntityUpdate {
                    recipient: observer,
                    update: ServerUpdate::EntitySnapshot(subject.snapshot()),
                });
            } else if emit_existing_update {
                routes.push(RoutedEntityUpdate {
                    recipient: observer,
                    update: ServerUpdate::EntityUpdate(subject.update()),
                });
            }
        } else if observers.remove(&observer) {
            routes.push(RoutedEntityUpdate {
                recipient: observer,
                update: ServerUpdate::EntityRemove { id: subject.id },
            });
        }
    }
}

fn starter_passive_position(spawn_position: Vec3d) -> Vec3d {
    let chunk = BlockPos::containing(spawn_position).chunk_pos();
    Vec3d::new(
        offset_within_chunk(spawn_position.x, chunk.min_block_x()),
        spawn_position.y,
        offset_within_chunk(spawn_position.z, chunk.min_block_z()),
    )
}

fn offset_within_chunk(value: f64, chunk_min: i32) -> f64 {
    (value + 2.5).clamp(chunk_min as f64 + 1.5, chunk_min as f64 + 14.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(raw: u64) -> ServerPlayerId {
        ServerPlayerId::from_raw_for_tests(raw)
    }

    fn state(id: u64, x: f64, z: f64) -> ServerEntityState {
        ServerEntityState {
            id: EntityId(id),
            kind: EntityKind::Cow,
            position: Vec3d::new(x, 64.0, z),
            y_rot_degrees: 45.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            age_ticks: 0,
            alive: true,
        }
    }

    #[test]
    fn starter_passive_spawns_once_near_initial_spawn() {
        let mut store = ServerEntityStore::default();

        let first = store.ensure_starter_passive_near_spawn(Vec3d::new(15.0, 64.0, 15.0));
        let second = store.ensure_starter_passive_near_spawn(Vec3d::new(1.0, 64.0, 1.0));

        assert_eq!(first, second);
        let entity = store.state(first).unwrap();
        assert_eq!(entity.kind, EntityKind::Cow);
        assert_eq!(entity.width, 0.9);
        assert_eq!(entity.height, 1.4);
        assert_eq!(entity.position, Vec3d::new(14.5, 64.0, 14.5));
    }

    #[test]
    fn entity_tick_advances_age_in_entity_ticking_chunks() {
        let mut store = ServerEntityStore::default();
        let id = store.ensure_starter_passive_near_spawn(Vec3d::new(8.0, 64.0, 8.0));

        assert!(store.tick_stationary(&[ChunkPos::new(1, 0)]).is_empty());
        assert_eq!(store.state(id).unwrap().age_ticks, 0);

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)]);

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].id, id);
        assert_eq!(updated[0].age_ticks, 1);
    }

    #[test]
    fn observer_reconcile_adds_and_removes_visible_entities() {
        let observer = player(1);
        let subject = state(7, 8.0, 8.0);
        let mut tracking = EntityTracking::default();

        let routes = tracking.reconcile_observer(observer, &[subject], |_, _| true);

        assert_eq!(
            routes,
            vec![RoutedEntityUpdate {
                recipient: observer,
                update: ServerUpdate::EntitySnapshot(subject.snapshot()),
            }]
        );

        let routes = tracking.reconcile_observer(observer, &[subject], |_, _| false);

        assert_eq!(
            routes,
            vec![RoutedEntityUpdate {
                recipient: observer,
                update: ServerUpdate::EntityRemove { id: subject.id },
            }]
        );
    }

    #[test]
    fn subject_reconcile_updates_existing_observers() {
        let observer = player(1);
        let mut tracking = EntityTracking::default();
        let initial = state(7, 8.0, 8.0);
        tracking.reconcile_subject(initial, [observer], |_, _| true, false);
        let moved = ServerEntityState {
            position: Vec3d::new(9.0, 64.0, 8.0),
            age_ticks: 2,
            ..initial
        };

        let routes = tracking.reconcile_subject(moved, [observer], |_, _| true, true);

        assert_eq!(
            routes,
            vec![RoutedEntityUpdate {
                recipient: observer,
                update: ServerUpdate::EntityUpdate(moved.update()),
            }]
        );
    }
}
