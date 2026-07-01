use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{BlockPos, ChunkPos, Vec3d};
#[cfg(feature = "physics-rapier")]
use mclone_protocol::EntityRotation;
use mclone_protocol::{EntityId, EntityKind};

use super::ServerEntityState;
use super::tick_list::ServerEntityTickList;

#[derive(Debug, Default)]
pub(crate) struct ServerEntityStore {
    entities: BTreeMap<EntityId, ServerEntityState>,
    tick_list: ServerEntityTickList,
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

    pub(crate) fn tick_stationary(
        &mut self,
        entity_ticking_chunks: &[ChunkPos],
    ) -> Vec<ServerEntityState> {
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
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: 1,
                ticking_entities: 0
            }
        );

        let updated = store.tick_stationary(&[ChunkPos::new(0, 0)]);

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].id, id);
        assert_eq!(updated[0].age_ticks, 1);
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: 1,
                ticking_entities: 1
            }
        );
    }

    #[test]
    fn entity_tick_list_demotes_without_removing_stored_entity() {
        let mut store = ServerEntityStore::default();
        let id = store.ensure_starter_passive_near_spawn(Vec3d::new(8.0, 64.0, 8.0));
        store.tick_stationary(&[ChunkPos::new(0, 0)]);

        let updated = store.tick_stationary(&[]);

        assert!(updated.is_empty());
        assert_eq!(store.state(id).unwrap().age_ticks, 1);
        assert_eq!(
            store.diagnostics(),
            ServerEntityStoreDiagnostics {
                stored_entities: 1,
                ticking_entities: 0
            }
        );
    }
}
