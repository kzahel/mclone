//! Remote player visibility and publication routing.
//!
//! Java's `ChunkMap.TrackedEntity` owns the set of players currently paired
//! with each tracked entity, while `ServerEntity` emits add/move/remove packets
//! to those pairings. This native slice mirrors that boundary for server
//! players only; broader entity tracking can grow from this module later.

use std::collections::{BTreeMap, BTreeSet};

use mclone_core::{BlockPos, ChunkPos, Vec3d};
use mclone_protocol::{PlayerAppearance, RemotePlayerId, RemotePlayerUpdate, ServerUpdate};

use crate::players::ServerPlayerId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RemotePlayerState {
    pub(crate) player_id: ServerPlayerId,
    pub(crate) appearance: PlayerAppearance,
    pub(crate) position: Vec3d,
    pub(crate) y_rot_degrees: f32,
    pub(crate) x_rot_degrees: f32,
    pub(crate) on_ground: bool,
    pub(crate) publishable: bool,
}

impl RemotePlayerState {
    fn chunk_pos(self) -> ChunkPos {
        BlockPos::containing(self.position).chunk_pos()
    }

    fn protocol_update(self) -> RemotePlayerUpdate {
        RemotePlayerUpdate {
            id: remote_player_id(self.player_id),
            appearance: self.appearance,
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            on_ground: self.on_ground,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RoutedRemotePlayerUpdate {
    pub(crate) recipient: ServerPlayerId,
    pub(crate) update: ServerUpdate,
}

#[derive(Debug, Default)]
pub(crate) struct RemotePlayerTracking {
    seen_by_subject: BTreeMap<ServerPlayerId, BTreeSet<ServerPlayerId>>,
}

impl RemotePlayerTracking {
    pub(crate) fn add_player(&mut self, player_id: ServerPlayerId) {
        self.seen_by_subject.entry(player_id).or_default();
    }

    pub(crate) fn remove_player(
        &mut self,
        player_id: ServerPlayerId,
    ) -> Vec<RoutedRemotePlayerUpdate> {
        let mut routes = Vec::new();
        if let Some(observers) = self.seen_by_subject.remove(&player_id) {
            for observer in observers {
                if observer != player_id {
                    routes.push(remote_remove_route(observer, player_id));
                }
            }
        }
        for observers in self.seen_by_subject.values_mut() {
            observers.remove(&player_id);
        }
        routes
    }

    pub(crate) fn reconcile_observer(
        &mut self,
        observer: ServerPlayerId,
        subjects: &[RemotePlayerState],
        mut tracks_chunk: impl FnMut(ServerPlayerId, ChunkPos) -> bool,
    ) -> Vec<RoutedRemotePlayerUpdate> {
        let mut routes = Vec::new();
        for subject in subjects {
            self.reconcile_pair(observer, *subject, &mut tracks_chunk, false, &mut routes);
        }
        routes
    }

    pub(crate) fn reconcile_subject(
        &mut self,
        subject: RemotePlayerState,
        observers: impl IntoIterator<Item = ServerPlayerId>,
        mut tracks_chunk: impl FnMut(ServerPlayerId, ChunkPos) -> bool,
        emit_existing_updates: bool,
    ) -> Vec<RoutedRemotePlayerUpdate> {
        let mut routes = Vec::new();
        self.add_player(subject.player_id);
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
        subject: RemotePlayerState,
        tracks_chunk: &mut impl FnMut(ServerPlayerId, ChunkPos) -> bool,
        emit_existing_update: bool,
        routes: &mut Vec<RoutedRemotePlayerUpdate>,
    ) {
        if observer == subject.player_id {
            return;
        }
        let observers = self.seen_by_subject.entry(subject.player_id).or_default();
        let should_see = subject.publishable && tracks_chunk(observer, subject.chunk_pos());
        if should_see {
            if observers.insert(observer) {
                routes.push(RoutedRemotePlayerUpdate {
                    recipient: observer,
                    update: ServerUpdate::RemotePlayerAdd(subject.protocol_update()),
                });
            } else if emit_existing_update {
                routes.push(RoutedRemotePlayerUpdate {
                    recipient: observer,
                    update: ServerUpdate::RemotePlayerUpdate(subject.protocol_update()),
                });
            }
        } else if observers.remove(&observer) {
            routes.push(remote_remove_route(observer, subject.player_id));
        }
    }
}

fn remote_remove_route(
    recipient: ServerPlayerId,
    subject: ServerPlayerId,
) -> RoutedRemotePlayerUpdate {
    RoutedRemotePlayerUpdate {
        recipient,
        update: ServerUpdate::RemotePlayerRemove {
            id: remote_player_id(subject),
        },
    }
}

fn remote_player_id(player_id: ServerPlayerId) -> RemotePlayerId {
    RemotePlayerId(player_id.as_u64())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(raw: u64) -> ServerPlayerId {
        ServerPlayerId::from_raw_for_tests(raw)
    }

    fn state(player_id: ServerPlayerId, x: f64, z: f64) -> RemotePlayerState {
        RemotePlayerState {
            player_id,
            appearance: PlayerAppearance::default(),
            position: Vec3d::new(x, 64.0, z),
            y_rot_degrees: 45.0,
            x_rot_degrees: 10.0,
            on_ground: true,
            publishable: true,
        }
    }

    #[test]
    fn observer_reconcile_adds_and_removes_visible_subjects() {
        let observer = player(1);
        let subject = player(2);
        let mut tracking = RemotePlayerTracking::default();
        tracking.add_player(observer);
        tracking.add_player(subject);
        let subject_state = state(subject, 8.0, 8.0);

        let routes = tracking.reconcile_observer(observer, &[subject_state], |_, _| true);

        assert_eq!(
            routes,
            vec![RoutedRemotePlayerUpdate {
                recipient: observer,
                update: ServerUpdate::RemotePlayerAdd(subject_state.protocol_update()),
            }]
        );

        let routes = tracking.reconcile_observer(observer, &[subject_state], |_, _| false);

        assert_eq!(
            routes,
            vec![RoutedRemotePlayerUpdate {
                recipient: observer,
                update: ServerUpdate::RemotePlayerRemove {
                    id: RemotePlayerId(subject.as_u64()),
                },
            }]
        );
    }

    #[test]
    fn subject_reconcile_updates_existing_observers() {
        let observer = player(1);
        let subject = player(2);
        let mut tracking = RemotePlayerTracking::default();
        let initial = state(subject, 8.0, 8.0);
        tracking.reconcile_subject(initial, [observer], |_, _| true, false);
        let moved = state(subject, 9.0, 8.0);

        let routes = tracking.reconcile_subject(moved, [observer], |_, _| true, true);

        assert_eq!(
            routes,
            vec![RoutedRemotePlayerUpdate {
                recipient: observer,
                update: ServerUpdate::RemotePlayerUpdate(moved.protocol_update()),
            }]
        );
    }
}
