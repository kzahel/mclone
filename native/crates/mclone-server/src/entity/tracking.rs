use std::collections::{BTreeMap, BTreeSet};

use mclone_core::ChunkPos;
use mclone_protocol::{EntityId, ServerUpdate};

use crate::player_chunk_tracking::DimensionInterestSource;

use super::ServerEntityState;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RoutedEntityUpdate {
    pub(crate) recipient: DimensionInterestSource,
    pub(crate) update: ServerUpdate,
}

#[derive(Debug, Default)]
pub(crate) struct EntityTracking {
    seen_by_entity: BTreeMap<EntityId, BTreeSet<DimensionInterestSource>>,
}

impl EntityTracking {
    #[allow(dead_code)]
    pub(crate) fn diagnostics(&self) -> EntityTrackingDiagnostics {
        EntityTrackingDiagnostics {
            tracked_entities: self
                .seen_by_entity
                .values()
                .filter(|observers| !observers.is_empty())
                .count(),
            tracked_observer_pairs: self.seen_by_entity.values().map(BTreeSet::len).sum(),
        }
    }

    pub(crate) fn remove_observer(&mut self, observer: DimensionInterestSource) {
        for observers in self.seen_by_entity.values_mut() {
            observers.remove(&observer);
        }
    }

    pub(crate) fn reconcile_observer(
        &mut self,
        observer: DimensionInterestSource,
        subjects: &[ServerEntityState],
        mut tracks_chunk: impl FnMut(DimensionInterestSource, ChunkPos) -> bool,
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
        observers: impl IntoIterator<Item = DimensionInterestSource>,
        mut tracks_chunk: impl FnMut(DimensionInterestSource, ChunkPos) -> bool,
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
        observer: DimensionInterestSource,
        subject: ServerEntityState,
        tracks_chunk: &mut impl FnMut(DimensionInterestSource, ChunkPos) -> bool,
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

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct EntityTrackingDiagnostics {
    pub(crate) tracked_entities: usize,
    pub(crate) tracked_observer_pairs: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServerPlayerId;
    use mclone_core::Vec3d;
    use mclone_protocol::EntityKind;

    fn player(raw: u64) -> ServerPlayerId {
        ServerPlayerId::from_raw_for_tests(raw)
    }

    fn state(id: u64, x: f64, z: f64) -> ServerEntityState {
        ServerEntityState {
            id: EntityId(id),
            persistent_id: mclone_protocol::EntityPersistentId::new(0, id),
            kind: EntityKind::Cow,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            animation: None,
            position: Vec3d::new(x, 64.0, z),
            y_rot_degrees: 45.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            tick_count: 0,
            alive: true,
        }
    }

    #[test]
    fn observer_reconcile_adds_and_removes_visible_entities() {
        let observer = DimensionInterestSource::Player(player(1));
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
        assert_eq!(
            tracking.diagnostics(),
            EntityTrackingDiagnostics {
                tracked_entities: 1,
                tracked_observer_pairs: 1
            }
        );

        let routes = tracking.reconcile_observer(observer, &[subject], |_, _| false);

        assert_eq!(
            routes,
            vec![RoutedEntityUpdate {
                recipient: observer,
                update: ServerUpdate::EntityRemove { id: subject.id },
            }]
        );
        assert_eq!(
            tracking.diagnostics(),
            EntityTrackingDiagnostics {
                tracked_entities: 0,
                tracked_observer_pairs: 0
            }
        );
    }

    #[test]
    fn subject_reconcile_updates_existing_observers() {
        let observer = DimensionInterestSource::Player(player(1));
        let mut tracking = EntityTracking::default();
        let initial = state(7, 8.0, 8.0);
        tracking.reconcile_subject(initial, [observer], |_, _| true, false);
        let moved = ServerEntityState {
            position: Vec3d::new(9.0, 64.0, 8.0),
            tick_count: 2,
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
