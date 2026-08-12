use std::collections::BTreeSet;

use mclone_core::ChunkPos;
use mclone_protocol::EntityId;

use super::ServerEntityState;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ServerEntityTickList {
    active: BTreeSet<EntityId>,
}

impl ServerEntityTickList {
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.active.len()
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, id: EntityId) -> bool {
        self.active.contains(&id)
    }

    pub(crate) fn reconcile(
        &mut self,
        entities: impl IntoIterator<Item = ServerEntityState>,
        entity_ticking_chunks: &BTreeSet<ChunkPos>,
    ) -> EntityTickListReconcile {
        let previous = self.active.clone();
        self.active = entities
            .into_iter()
            .filter(|entity| entity.alive && entity_ticking_chunks.contains(&entity.chunk_pos()))
            .map(|entity| entity.id)
            .collect();
        EntityTickListReconcile {
            added: self.active.difference(&previous).count(),
            removed: previous.difference(&self.active).count(),
        }
    }

    pub(crate) fn iteration_ids(&self) -> Vec<EntityId> {
        self.active.iter().copied().collect()
    }

    pub(crate) fn remove(&mut self, id: EntityId) {
        self.active.remove(&id);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct EntityTickListReconcile {
    pub(crate) added: usize,
    pub(crate) removed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::Vec3d;
    use mclone_protocol::{EntityKind, EntityRotation};

    fn state(id: u64, x: f64, z: f64, alive: bool) -> ServerEntityState {
        ServerEntityState {
            id: EntityId(id),
            persistent_id: mclone_protocol::EntityPersistentId::new(0, id),
            kind: EntityKind::Cow,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            position: Vec3d::new(x, 64.0, z),
            y_rot_degrees: 45.0,
            x_rot_degrees: 0.0,
            rotation: Some(EntityRotation::IDENTITY),
            on_ground: true,
            width: 0.9,
            height: 1.4,
            tick_count: 0,
            alive,
        }
    }

    #[test]
    fn reconcile_adds_only_alive_entities_in_entity_ticking_chunks() {
        let mut tick_list = ServerEntityTickList::default();
        let ticking_chunks = [ChunkPos::new(0, 0)].into_iter().collect::<BTreeSet<_>>();

        let report = tick_list.reconcile(
            [
                state(1, 8.0, 8.0, true),
                state(2, 24.0, 8.0, true),
                state(3, 8.0, 8.0, false),
            ],
            &ticking_chunks,
        );

        assert_eq!(report.added, 1);
        assert_eq!(report.removed, 0);
        assert!(tick_list.contains(EntityId(1)));
        assert!(!tick_list.contains(EntityId(2)));
        assert!(!tick_list.contains(EntityId(3)));
    }

    #[test]
    fn reconcile_removes_entities_after_chunk_demotes() {
        let mut tick_list = ServerEntityTickList::default();
        let ticking_chunks = [ChunkPos::new(0, 0)].into_iter().collect::<BTreeSet<_>>();
        tick_list.reconcile([state(1, 8.0, 8.0, true)], &ticking_chunks);

        let report = tick_list.reconcile([state(1, 8.0, 8.0, true)], &BTreeSet::new());

        assert_eq!(report.added, 0);
        assert_eq!(report.removed, 1);
        assert!(!tick_list.contains(EntityId(1)));
    }

    #[test]
    fn iteration_ids_are_a_stable_snapshot() {
        let mut tick_list = ServerEntityTickList::default();
        let ticking_chunks = [ChunkPos::new(0, 0)].into_iter().collect::<BTreeSet<_>>();
        tick_list.reconcile(
            [state(2, 8.0, 8.0, true), state(1, 8.0, 8.0, true)],
            &ticking_chunks,
        );

        assert_eq!(tick_list.iteration_ids(), vec![EntityId(1), EntityId(2)]);
    }
}
