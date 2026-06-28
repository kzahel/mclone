//! Per-player chunk view tracking and update routing.
//!
//! Java keeps this shape in `ChunkMap`: requested/accepted player views,
//! player-visible chunk sets, and chunk load/unload fan-out live next to player
//! chunk tracking, while `DistanceManager` consumes aggregate player-ticket
//! inputs. This module keeps that boundary explicit for the native server.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mclone_core::{ChunkPos, ChunkSnapshot};
use mclone_protocol::{ChunkView, SectionBlockUpdate, ServerUpdate};

use crate::players::ServerPlayerId;

pub(crate) const JAVA_MIN_VIEW_DISTANCE: u32 = 3;
pub(crate) const JAVA_MAX_VIEW_DISTANCE: u32 = 33;
pub(crate) const DEFAULT_SERVER_MAX_VIEW_DISTANCE: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlayerChunkTrackingPolicy {
    max_render_distance: u32,
    max_chunk_tracking_radius: u32,
}

impl Default for PlayerChunkTrackingPolicy {
    fn default() -> Self {
        Self::new(
            DEFAULT_SERVER_MAX_VIEW_DISTANCE,
            DEFAULT_SERVER_MAX_VIEW_DISTANCE,
        )
    }
}

impl PlayerChunkTrackingPolicy {
    pub(crate) const fn new(max_render_distance: u32, max_chunk_tracking_radius: u32) -> Self {
        Self {
            max_render_distance,
            max_chunk_tracking_radius,
        }
    }

    pub(crate) fn clamp_view(self, requested: &ChunkView) -> ChunkView {
        debug_assert!(JAVA_MIN_VIEW_DISTANCE <= JAVA_MAX_VIEW_DISTANCE);
        let max_render_distance = self.max_render_distance.min(JAVA_MAX_VIEW_DISTANCE);
        let max_chunk_tracking_radius = self.max_chunk_tracking_radius.min(JAVA_MAX_VIEW_DISTANCE);
        ChunkView {
            center: requested.center,
            render_distance: requested.render_distance.min(max_render_distance),
            // Native tests and debug captures intentionally use radius 0/1.
            // Keep those valid while capping untrusted protocol requests above.
            chunk_tracking_radius: requested
                .chunk_tracking_radius
                .min(max_chunk_tracking_radius),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlayerChunkViewState {
    pub(crate) requested: Option<ChunkView>,
    pub(crate) accepted: Option<ChunkView>,
    visible_chunks: BTreeSet<ChunkPos>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlayerChunkViewChange {
    pub(crate) accepted: Option<ChunkView>,
    pub(crate) added_chunks: Vec<ChunkPos>,
    pub(crate) removed_chunks: Vec<ChunkPos>,
    pub(crate) aggregate_changed: bool,
    pub(crate) priority_centers_changed: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlayerChunkTrackingDiagnostics {
    pub player_count: usize,
    pub aggregate_player_ticket_chunks: usize,
    pub total_player_visible_chunks: usize,
    pub total_outbound_queue_depth: usize,
    pub max_player_visible_chunks: usize,
    pub max_outbound_queue_depth: usize,
    pub players: Vec<PlayerChunkTrackingPlayerDiagnostics>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerChunkTrackingPlayerDiagnostics {
    pub player_id: ServerPlayerId,
    pub requested_view: Option<ChunkView>,
    pub accepted_view: Option<ChunkView>,
    pub visible_chunks: usize,
    pub outbound_queue_depth: usize,
}

#[derive(Debug)]
pub(crate) struct PlayerChunkTracking {
    policy: PlayerChunkTrackingPolicy,
    players: BTreeMap<ServerPlayerId, PlayerChunkViewState>,
    aggregate_player_ticket_positions: BTreeSet<ChunkPos>,
    pending_updates: BTreeMap<ServerPlayerId, VecDeque<ServerUpdate>>,
}

impl PlayerChunkTracking {
    pub(crate) fn new(policy: PlayerChunkTrackingPolicy) -> Self {
        Self {
            policy,
            players: BTreeMap::new(),
            aggregate_player_ticket_positions: BTreeSet::new(),
            pending_updates: BTreeMap::new(),
        }
    }

    pub(crate) fn add_player(&mut self, player_id: ServerPlayerId) {
        self.players.entry(player_id).or_default();
        self.pending_updates.entry(player_id).or_default();
    }

    pub(crate) fn remove_player(&mut self, player_id: ServerPlayerId) -> PlayerChunkViewChange {
        let removed = self
            .players
            .remove(&player_id)
            .map(|state| state.visible_chunks)
            .unwrap_or_default();
        self.pending_updates.remove(&player_id);
        let aggregate_changed = self.rebuild_aggregate_player_ticket_positions();
        PlayerChunkViewChange {
            accepted: None,
            added_chunks: Vec::new(),
            removed_chunks: removed.into_iter().collect(),
            aggregate_changed,
            priority_centers_changed: true,
        }
    }

    pub(crate) fn set_requested_view(
        &mut self,
        player_id: ServerPlayerId,
        requested: ChunkView,
    ) -> PlayerChunkViewChange {
        self.add_player(player_id);
        let old_priority_centers = self.aggregate_player_ticket_priority_centers();
        let accepted = self.policy.clamp_view(&requested);
        let new_visible = chunk_positions_for_view(&accepted);
        let state = self
            .players
            .get_mut(&player_id)
            .expect("player state must exist after add_player");
        let old_visible = std::mem::replace(&mut state.visible_chunks, new_visible);
        state.requested = Some(requested);
        state.accepted = Some(accepted.clone());

        let added_chunks = state
            .visible_chunks
            .difference(&old_visible)
            .copied()
            .collect();
        let removed_chunks = old_visible
            .difference(&state.visible_chunks)
            .copied()
            .collect();
        let aggregate_changed = self.rebuild_aggregate_player_ticket_positions();
        let priority_centers_changed =
            self.aggregate_player_ticket_priority_centers() != old_priority_centers;

        PlayerChunkViewChange {
            accepted: Some(accepted),
            added_chunks,
            removed_chunks,
            aggregate_changed,
            priority_centers_changed,
        }
    }

    #[cfg(test)]
    pub(crate) fn accepted_view(&self, player_id: ServerPlayerId) -> Option<&ChunkView> {
        self.players
            .get(&player_id)
            .and_then(|state| state.accepted.as_ref())
    }

    pub(crate) fn aggregate_player_ticket_positions(&self) -> BTreeSet<ChunkPos> {
        self.aggregate_player_ticket_positions.clone()
    }

    pub(crate) fn aggregate_player_ticket_priority_centers(&self) -> Vec<ChunkPos> {
        self.players
            .values()
            .filter_map(|state| state.accepted.as_ref().map(|view| view.center))
            .collect()
    }

    pub(crate) fn diagnostics(&self) -> PlayerChunkTrackingDiagnostics {
        let players = self
            .players
            .iter()
            .map(|(player_id, state)| {
                let outbound_queue_depth =
                    self.pending_updates.get(player_id).map_or(0, VecDeque::len);
                PlayerChunkTrackingPlayerDiagnostics {
                    player_id: *player_id,
                    requested_view: state.requested.clone(),
                    accepted_view: state.accepted.clone(),
                    visible_chunks: state.visible_chunks.len(),
                    outbound_queue_depth,
                }
            })
            .collect::<Vec<_>>();
        let total_player_visible_chunks = players.iter().map(|player| player.visible_chunks).sum();
        let total_outbound_queue_depth = players
            .iter()
            .map(|player| player.outbound_queue_depth)
            .sum();
        let max_player_visible_chunks = players
            .iter()
            .map(|player| player.visible_chunks)
            .max()
            .unwrap_or(0);
        let max_outbound_queue_depth = players
            .iter()
            .map(|player| player.outbound_queue_depth)
            .max()
            .unwrap_or(0);

        PlayerChunkTrackingDiagnostics {
            player_count: players.len(),
            aggregate_player_ticket_chunks: self.aggregate_player_ticket_positions.len(),
            total_player_visible_chunks,
            total_outbound_queue_depth,
            max_player_visible_chunks,
            max_outbound_queue_depth,
            players,
        }
    }

    pub(crate) fn player_tracks_chunk(&self, player_id: ServerPlayerId, pos: ChunkPos) -> bool {
        self.players
            .get(&player_id)
            .is_some_and(|state| state.visible_chunks.contains(&pos))
    }

    pub(crate) fn queue_unload_for_player(&mut self, player_id: ServerPlayerId, pos: ChunkPos) {
        self.queue_update_for_player(player_id, ServerUpdate::ChunkUnload { pos });
    }

    pub(crate) fn queue_unload_for_tracking_players(&mut self, pos: ChunkPos) {
        let recipients = self.players_tracking_chunk(pos);
        for player_id in recipients {
            self.queue_unload_for_player(player_id, pos);
        }
    }

    pub(crate) fn queue_snapshot_for_player(
        &mut self,
        player_id: ServerPlayerId,
        snapshot: ChunkSnapshot,
    ) {
        self.queue_update_for_player(player_id, ServerUpdate::ChunkSnapshot(snapshot));
    }

    pub(crate) fn queue_snapshot_for_tracking_players(&mut self, snapshot: ChunkSnapshot) {
        let recipients = self.players_tracking_chunk(snapshot.pos);
        for player_id in recipients {
            self.queue_snapshot_for_player(player_id, snapshot.clone());
        }
    }

    pub(crate) fn queue_section_updates_for_tracking_players(
        &mut self,
        pos: ChunkPos,
        section_y: i32,
        updates: Vec<SectionBlockUpdate>,
    ) {
        if updates.is_empty() {
            return;
        }
        let recipients = self.players_tracking_chunk(pos);
        for player_id in recipients {
            self.queue_update_for_player(
                player_id,
                ServerUpdate::SectionBlockUpdates {
                    pos,
                    section_y,
                    updates: updates.clone(),
                },
            );
        }
    }

    pub(crate) fn drain_updates(&mut self, player_id: ServerPlayerId) -> Vec<ServerUpdate> {
        self.pending_updates
            .entry(player_id)
            .or_default()
            .drain(..)
            .collect()
    }

    pub(crate) fn queue_update_for_player(
        &mut self,
        player_id: ServerPlayerId,
        update: ServerUpdate,
    ) {
        if self.players.contains_key(&player_id) {
            self.pending_updates
                .entry(player_id)
                .or_default()
                .push_back(update);
        }
    }

    fn players_tracking_chunk(&self, pos: ChunkPos) -> Vec<ServerPlayerId> {
        self.players
            .iter()
            .filter_map(|(player_id, state)| {
                state.visible_chunks.contains(&pos).then_some(*player_id)
            })
            .collect()
    }

    fn rebuild_aggregate_player_ticket_positions(&mut self) -> bool {
        let next = self
            .players
            .values()
            .flat_map(|state| state.visible_chunks.iter().copied())
            .collect::<BTreeSet<_>>();
        if next == self.aggregate_player_ticket_positions {
            return false;
        }
        self.aggregate_player_ticket_positions = next;
        true
    }
}

pub(crate) fn chunk_positions_for_view(view: &ChunkView) -> BTreeSet<ChunkPos> {
    let radius =
        i32::try_from(view.chunk_tracking_radius).expect("chunk tracking radius exceeds i32");
    let min_x = view.center.x - radius;
    let max_x = view.center.x + radius;
    let min_z = view.center.z - radius;
    let max_z = view.center.z + radius;
    (min_x..=max_x)
        .flat_map(|x| (min_z..=max_z).map(move |z| ChunkPos::new(x, z)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(center: ChunkPos, radius: u32) -> ChunkView {
        ChunkView {
            center,
            render_distance: radius,
            chunk_tracking_radius: radius,
        }
    }

    #[test]
    fn policy_clamps_large_client_requests_without_raising_small_views() {
        let policy = PlayerChunkTrackingPolicy::new(4, 4);

        assert_eq!(
            policy.clamp_view(&view(ChunkPos::new(0, 0), 0)),
            view(ChunkPos::new(0, 0), 0)
        );
        assert_eq!(
            policy.clamp_view(&view(ChunkPos::new(0, 0), 3)),
            view(ChunkPos::new(0, 0), 3)
        );
        assert_eq!(
            policy.clamp_view(&view(ChunkPos::new(0, 0), 100)),
            view(ChunkPos::new(0, 0), 4)
        );
    }

    #[test]
    fn per_player_views_diff_visible_sets_and_aggregate_tickets() {
        let mut tracking = PlayerChunkTracking::new(PlayerChunkTrackingPolicy::new(4, 4));
        let player_a = ServerPlayerId::LOCAL;
        let player_b = ServerPlayerId::from_raw_for_tests(1);

        let first = tracking.set_requested_view(player_a, view(ChunkPos::new(0, 0), 0));
        assert_eq!(first.added_chunks, vec![ChunkPos::new(0, 0)]);
        assert!(first.removed_chunks.is_empty());
        assert!(first.aggregate_changed);
        assert_eq!(
            tracking.accepted_view(player_a),
            Some(&view(ChunkPos::new(0, 0), 0))
        );
        assert!(tracking.player_tracks_chunk(player_a, ChunkPos::new(0, 0)));

        let second = tracking.set_requested_view(player_b, view(ChunkPos::new(2, 0), 0));
        assert_eq!(second.added_chunks, vec![ChunkPos::new(2, 0)]);
        assert!(second.aggregate_changed);
        assert_eq!(
            tracking.aggregate_player_ticket_positions(),
            BTreeSet::from([ChunkPos::new(0, 0), ChunkPos::new(2, 0)])
        );

        let moved = tracking.set_requested_view(player_a, view(ChunkPos::new(1, 0), 0));
        assert_eq!(moved.added_chunks, vec![ChunkPos::new(1, 0)]);
        assert_eq!(moved.removed_chunks, vec![ChunkPos::new(0, 0)]);
        assert_eq!(
            tracking.aggregate_player_ticket_positions(),
            BTreeSet::from([ChunkPos::new(1, 0), ChunkPos::new(2, 0)])
        );

        let diagnostics = tracking.diagnostics();
        assert_eq!(diagnostics.player_count, 2);
        assert_eq!(diagnostics.aggregate_player_ticket_chunks, 2);
        assert_eq!(diagnostics.total_player_visible_chunks, 2);
        assert_eq!(diagnostics.total_outbound_queue_depth, 0);
        assert_eq!(diagnostics.max_player_visible_chunks, 1);
    }

    #[test]
    fn disconnect_removes_only_that_players_ticket_contribution() {
        let mut tracking = PlayerChunkTracking::new(PlayerChunkTrackingPolicy::new(4, 4));
        let player_a = ServerPlayerId::LOCAL;
        let player_b = ServerPlayerId::from_raw_for_tests(1);
        tracking.set_requested_view(player_a, view(ChunkPos::new(0, 0), 0));
        tracking.set_requested_view(player_b, view(ChunkPos::new(1, 0), 0));

        let removed = tracking.remove_player(player_a);

        assert_eq!(removed.removed_chunks, vec![ChunkPos::new(0, 0)]);
        assert_eq!(
            tracking.aggregate_player_ticket_positions(),
            BTreeSet::from([ChunkPos::new(1, 0)])
        );
    }
}
