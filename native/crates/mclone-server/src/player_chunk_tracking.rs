//! Per-source dimension interest and update routing.
//!
//! Java keeps this shape in `ChunkMap`: requested/accepted player views,
//! player-visible chunk sets, and chunk load/unload fan-out live next to player
//! chunk tracking, while `DistanceManager` consumes aggregate player-ticket
//! inputs. Mclone extends that boundary with player-free observers while
//! keeping residency and simulation interest separate.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mclone_core::{ChunkPos, ChunkSnapshot};
use mclone_protocol::{ChunkView, SectionBlockUpdate, ServerUpdate, encode_server_update};

use crate::players::ServerPlayerId;

pub(crate) const JAVA_MIN_VIEW_DISTANCE: u32 = 3;
pub(crate) const JAVA_MAX_VIEW_DISTANCE: u32 = 33;
pub(crate) const DEFAULT_DEDICATED_SERVER_VIEW_DISTANCE: u32 = 10;
pub(crate) const DEFAULT_DEDICATED_SERVER_CHUNK_TRACKING_RADIUS: u32 =
    DEFAULT_DEDICATED_SERVER_VIEW_DISTANCE + 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObserverId(u64);

impl ObserverId {
    pub(crate) const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DimensionInterestSource {
    Player(ServerPlayerId),
    Observer(ObserverId),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ObserverSimulationInterest {
    #[default]
    ResidencyOnly,
    BlockAndEntityTicking,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlayerChunkTrackingPolicy {
    max_render_distance: u32,
    max_chunk_tracking_radius: u32,
    unload_hysteresis_chunks: u32,
}

impl Default for PlayerChunkTrackingPolicy {
    fn default() -> Self {
        Self::dedicated_default()
    }
}

impl PlayerChunkTrackingPolicy {
    pub(crate) const fn new(max_render_distance: u32, max_chunk_tracking_radius: u32) -> Self {
        Self {
            max_render_distance,
            max_chunk_tracking_radius,
            unload_hysteresis_chunks: 0,
        }
    }

    pub(crate) const fn dedicated_default() -> Self {
        Self::new(
            DEFAULT_DEDICATED_SERVER_CHUNK_TRACKING_RADIUS,
            DEFAULT_DEDICATED_SERVER_CHUNK_TRACKING_RADIUS,
        )
    }

    pub(crate) const fn java_max() -> Self {
        Self::new(JAVA_MAX_VIEW_DISTANCE, JAVA_MAX_VIEW_DISTANCE)
    }

    pub(crate) const fn with_unload_hysteresis_chunks(mut self, chunks: u32) -> Self {
        self.unload_hysteresis_chunks = chunks;
        self
    }

    pub(crate) fn session_limits(self) -> (u32, u32) {
        (
            self.max_render_distance.min(JAVA_MAX_VIEW_DISTANCE),
            self.max_chunk_tracking_radius.min(JAVA_MAX_VIEW_DISTANCE),
        )
    }

    #[cfg(test)]
    pub(crate) const fn unload_hysteresis_chunks(self) -> u32 {
        self.unload_hysteresis_chunks
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

    fn unload_radius(self, accepted: &ChunkView) -> u32 {
        if accepted.chunk_tracking_radius < JAVA_MIN_VIEW_DISTANCE {
            return accepted.chunk_tracking_radius;
        }
        accepted
            .chunk_tracking_radius
            .saturating_add(self.unload_hysteresis_chunks)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlayerChunkViewState {
    pub(crate) requested: Option<ChunkView>,
    pub(crate) accepted: Option<ChunkView>,
    // Chunks the player may still hold locally. Hysteresis policies can retain
    // chunks just outside the accepted target view to avoid edge churn.
    visible_chunks: BTreeSet<ChunkPos>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ObserverChunkViewState {
    view: PlayerChunkViewState,
    simulation: ObserverSimulationInterest,
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
    pub observer_count: usize,
    pub aggregate_player_ticket_chunks: usize,
    pub aggregate_resident_chunks: usize,
    pub aggregate_simulation_ticket_chunks: usize,
    pub total_player_visible_chunks: usize,
    pub total_observer_visible_chunks: usize,
    pub total_outbound_queue_depth: usize,
    pub total_observer_outbound_queue_depth: usize,
    pub total_observer_outbound_bytes: usize,
    pub max_player_visible_chunks: usize,
    pub max_observer_visible_chunks: usize,
    pub max_outbound_queue_depth: usize,
    pub max_observer_outbound_queue_depth: usize,
    pub players: Vec<PlayerChunkTrackingPlayerDiagnostics>,
    pub observers: Vec<ObserverChunkTrackingDiagnostics>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerChunkTrackingPlayerDiagnostics {
    pub player_id: ServerPlayerId,
    pub requested_view: Option<ChunkView>,
    pub accepted_view: Option<ChunkView>,
    pub visible_chunks: usize,
    pub outbound_queue_depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverChunkTrackingDiagnostics {
    pub observer_id: ObserverId,
    pub simulation: ObserverSimulationInterest,
    pub requested_view: Option<ChunkView>,
    pub accepted_view: Option<ChunkView>,
    pub visible_chunks: usize,
    pub outbound_queue_depth: usize,
    pub outbound_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct PlayerChunkTracking {
    policy: PlayerChunkTrackingPolicy,
    players: BTreeMap<ServerPlayerId, PlayerChunkViewState>,
    observers: BTreeMap<ObserverId, ObserverChunkViewState>,
    aggregate_player_ticket_positions: BTreeSet<ChunkPos>,
    aggregate_resident_positions: BTreeSet<ChunkPos>,
    aggregate_simulation_ticket_positions: BTreeSet<ChunkPos>,
    pending_updates: BTreeMap<ServerPlayerId, VecDeque<ServerUpdate>>,
    pending_observer_updates: BTreeMap<ObserverId, VecDeque<ServerUpdate>>,
}

impl PlayerChunkTracking {
    pub(crate) fn new(policy: PlayerChunkTrackingPolicy) -> Self {
        Self {
            policy,
            players: BTreeMap::new(),
            observers: BTreeMap::new(),
            aggregate_player_ticket_positions: BTreeSet::new(),
            aggregate_resident_positions: BTreeSet::new(),
            aggregate_simulation_ticket_positions: BTreeSet::new(),
            pending_updates: BTreeMap::new(),
            pending_observer_updates: BTreeMap::new(),
        }
    }

    pub(crate) const fn policy(&self) -> PlayerChunkTrackingPolicy {
        self.policy
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
        let aggregate_changed = self.rebuild_aggregate_interest_positions();
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
        let old_priority_centers = self.aggregate_interest_priority_centers();
        let accepted = self.policy.clamp_view(&requested);
        let state = self
            .players
            .get_mut(&player_id)
            .expect("player state must exist after add_player");
        let old_visible = std::mem::take(&mut state.visible_chunks);
        let target_visible = chunk_positions_for_view(&accepted);
        let unload_visible = chunk_positions_for_center_radius(
            accepted.center,
            self.policy.unload_radius(&accepted),
        );
        let retained_visible = old_visible
            .intersection(&unload_visible)
            .copied()
            .collect::<BTreeSet<_>>();
        let new_visible = target_visible.union(&retained_visible).copied().collect();
        state.visible_chunks = new_visible;
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
        let aggregate_changed = self.rebuild_aggregate_interest_positions();
        let priority_centers_changed =
            self.aggregate_interest_priority_centers() != old_priority_centers;

        PlayerChunkViewChange {
            accepted: Some(accepted),
            added_chunks,
            removed_chunks,
            aggregate_changed,
            priority_centers_changed,
        }
    }

    pub(crate) fn accepted_view(&self, player_id: ServerPlayerId) -> Option<&ChunkView> {
        self.players
            .get(&player_id)
            .and_then(|state| state.accepted.as_ref())
    }

    pub(crate) fn add_observer(
        &mut self,
        observer_id: ObserverId,
        simulation: ObserverSimulationInterest,
    ) {
        self.observers.entry(observer_id).or_default().simulation = simulation;
        self.pending_observer_updates
            .entry(observer_id)
            .or_default();
    }

    pub(crate) fn remove_observer(&mut self, observer_id: ObserverId) -> PlayerChunkViewChange {
        let removed = self
            .observers
            .remove(&observer_id)
            .map(|state| state.view.visible_chunks)
            .unwrap_or_default();
        self.pending_observer_updates.remove(&observer_id);
        let aggregate_changed = self.rebuild_aggregate_interest_positions();
        PlayerChunkViewChange {
            accepted: None,
            added_chunks: Vec::new(),
            removed_chunks: removed.into_iter().collect(),
            aggregate_changed,
            priority_centers_changed: true,
        }
    }

    pub(crate) fn set_observer_requested_view(
        &mut self,
        observer_id: ObserverId,
        requested: ChunkView,
        simulation: ObserverSimulationInterest,
    ) -> PlayerChunkViewChange {
        self.add_observer(observer_id, simulation);
        let old_priority_centers = self.aggregate_interest_priority_centers();
        let accepted = self.policy.clamp_view(&requested);
        let state = self
            .observers
            .get_mut(&observer_id)
            .expect("observer state must exist after add_observer");
        let old_visible = std::mem::take(&mut state.view.visible_chunks);
        let target_visible = chunk_positions_for_view(&accepted);
        let unload_visible = chunk_positions_for_center_radius(
            accepted.center,
            self.policy.unload_radius(&accepted),
        );
        let retained_visible = old_visible
            .intersection(&unload_visible)
            .copied()
            .collect::<BTreeSet<_>>();
        state.view.visible_chunks = target_visible.union(&retained_visible).copied().collect();
        state.view.requested = Some(requested);
        state.view.accepted = Some(accepted.clone());
        state.simulation = simulation;

        let added_chunks = state
            .view
            .visible_chunks
            .difference(&old_visible)
            .copied()
            .collect();
        let removed_chunks = old_visible
            .difference(&state.view.visible_chunks)
            .copied()
            .collect();
        let aggregate_changed = self.rebuild_aggregate_interest_positions();
        let priority_centers_changed =
            self.aggregate_interest_priority_centers() != old_priority_centers;
        PlayerChunkViewChange {
            accepted: Some(accepted),
            added_chunks,
            removed_chunks,
            aggregate_changed,
            priority_centers_changed,
        }
    }

    #[cfg(test)]
    pub(crate) fn accepted_observer_view(&self, observer_id: ObserverId) -> Option<&ChunkView> {
        self.observers
            .get(&observer_id)
            .and_then(|state| state.view.accepted.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn aggregate_player_ticket_positions(&self) -> BTreeSet<ChunkPos> {
        self.aggregate_player_ticket_positions.clone()
    }

    pub(crate) fn aggregate_resident_positions(&self) -> BTreeSet<ChunkPos> {
        self.aggregate_resident_positions.clone()
    }

    pub(crate) fn aggregate_simulation_ticket_positions(&self) -> BTreeSet<ChunkPos> {
        self.aggregate_simulation_ticket_positions.clone()
    }

    pub(crate) fn aggregate_interest_priority_centers(&self) -> Vec<ChunkPos> {
        self.players
            .values()
            .filter_map(|state| state.accepted.as_ref().map(|view| view.center))
            .chain(
                self.observers
                    .values()
                    .filter_map(|state| state.view.accepted.as_ref().map(|view| view.center)),
            )
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
        let observers = self
            .observers
            .iter()
            .map(|(observer_id, state)| {
                let queue = self.pending_observer_updates.get(observer_id);
                let outbound_queue_depth = queue.map_or(0, VecDeque::len);
                let outbound_bytes = queue.map_or(0, |updates| {
                    updates
                        .iter()
                        .filter_map(|update| encode_server_update(update).ok())
                        .map(|frame| frame.len())
                        .sum()
                });
                ObserverChunkTrackingDiagnostics {
                    observer_id: *observer_id,
                    simulation: state.simulation,
                    requested_view: state.view.requested.clone(),
                    accepted_view: state.view.accepted.clone(),
                    visible_chunks: state.view.visible_chunks.len(),
                    outbound_queue_depth,
                    outbound_bytes,
                }
            })
            .collect::<Vec<_>>();
        let total_player_visible_chunks = players.iter().map(|player| player.visible_chunks).sum();
        let total_observer_visible_chunks = observers
            .iter()
            .map(|observer| observer.visible_chunks)
            .sum();
        let total_outbound_queue_depth = players
            .iter()
            .map(|player| player.outbound_queue_depth)
            .sum();
        let total_observer_outbound_queue_depth = observers
            .iter()
            .map(|observer| observer.outbound_queue_depth)
            .sum();
        let total_observer_outbound_bytes = observers
            .iter()
            .map(|observer| observer.outbound_bytes)
            .sum();
        let max_player_visible_chunks = players
            .iter()
            .map(|player| player.visible_chunks)
            .max()
            .unwrap_or(0);
        let max_observer_visible_chunks = observers
            .iter()
            .map(|observer| observer.visible_chunks)
            .max()
            .unwrap_or(0);
        let max_outbound_queue_depth = players
            .iter()
            .map(|player| player.outbound_queue_depth)
            .max()
            .unwrap_or(0);
        let max_observer_outbound_queue_depth = observers
            .iter()
            .map(|observer| observer.outbound_queue_depth)
            .max()
            .unwrap_or(0);

        PlayerChunkTrackingDiagnostics {
            player_count: players.len(),
            observer_count: observers.len(),
            aggregate_player_ticket_chunks: self.aggregate_player_ticket_positions.len(),
            aggregate_resident_chunks: self.aggregate_resident_positions.len(),
            aggregate_simulation_ticket_chunks: self.aggregate_simulation_ticket_positions.len(),
            total_player_visible_chunks,
            total_observer_visible_chunks,
            total_outbound_queue_depth,
            total_observer_outbound_queue_depth,
            total_observer_outbound_bytes,
            max_player_visible_chunks,
            max_observer_visible_chunks,
            max_outbound_queue_depth,
            max_observer_outbound_queue_depth,
            players,
            observers,
        }
    }

    pub(crate) fn player_tracks_chunk(&self, player_id: ServerPlayerId, pos: ChunkPos) -> bool {
        self.players
            .get(&player_id)
            .is_some_and(|state| state.visible_chunks.contains(&pos))
    }

    pub(crate) fn source_tracks_chunk(
        &self,
        source: DimensionInterestSource,
        pos: ChunkPos,
    ) -> bool {
        match source {
            DimensionInterestSource::Player(player_id) => self.player_tracks_chunk(player_id, pos),
            DimensionInterestSource::Observer(observer_id) => self
                .observers
                .get(&observer_id)
                .is_some_and(|state| state.view.visible_chunks.contains(&pos)),
        }
    }

    pub(crate) fn interest_sources(&self) -> Vec<DimensionInterestSource> {
        self.players
            .keys()
            .copied()
            .map(DimensionInterestSource::Player)
            .chain(
                self.observers
                    .keys()
                    .copied()
                    .map(DimensionInterestSource::Observer),
            )
            .collect()
    }

    pub(crate) fn queue_unload_for_player(&mut self, player_id: ServerPlayerId, pos: ChunkPos) {
        self.queue_update_for_player(player_id, ServerUpdate::ChunkUnload { pos });
    }

    pub(crate) fn queue_unload_for_observer(&mut self, observer_id: ObserverId, pos: ChunkPos) {
        self.queue_update_for_observer(observer_id, ServerUpdate::ChunkUnload { pos });
    }

    pub(crate) fn queue_unload_for_tracking_sources(&mut self, pos: ChunkPos) {
        for source in self.sources_tracking_chunk(pos) {
            self.queue_update_for_source(source, ServerUpdate::ChunkUnload { pos });
        }
    }

    pub(crate) fn queue_snapshot_for_player(
        &mut self,
        player_id: ServerPlayerId,
        snapshot: ChunkSnapshot,
    ) {
        self.queue_update_for_player(player_id, ServerUpdate::ChunkSnapshot(snapshot));
    }

    pub(crate) fn queue_snapshot_for_observer(
        &mut self,
        observer_id: ObserverId,
        snapshot: ChunkSnapshot,
    ) {
        self.queue_update_for_observer(observer_id, ServerUpdate::ChunkSnapshot(snapshot));
    }

    pub(crate) fn queue_snapshot_for_tracking_sources(&mut self, snapshot: ChunkSnapshot) {
        for source in self.sources_tracking_chunk(snapshot.pos) {
            self.queue_update_for_source(source, ServerUpdate::ChunkSnapshot(snapshot.clone()));
        }
    }

    pub(crate) fn queue_section_updates_for_tracking_sources(
        &mut self,
        pos: ChunkPos,
        section_y: i32,
        updates: Vec<SectionBlockUpdate>,
    ) {
        if updates.is_empty() {
            return;
        }
        for source in self.sources_tracking_chunk(pos) {
            self.queue_update_for_source(
                source,
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

    pub(crate) fn drain_observer_updates(&mut self, observer_id: ObserverId) -> Vec<ServerUpdate> {
        self.pending_observer_updates
            .entry(observer_id)
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

    pub(crate) fn queue_update_for_observer(
        &mut self,
        observer_id: ObserverId,
        update: ServerUpdate,
    ) {
        if self.observers.contains_key(&observer_id) {
            self.pending_observer_updates
                .entry(observer_id)
                .or_default()
                .push_back(update);
        }
    }

    pub(crate) fn queue_update_for_source(
        &mut self,
        source: DimensionInterestSource,
        update: ServerUpdate,
    ) {
        match source {
            DimensionInterestSource::Player(player_id) => {
                self.queue_update_for_player(player_id, update);
            }
            DimensionInterestSource::Observer(observer_id) => {
                self.queue_update_for_observer(observer_id, update);
            }
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

    fn sources_tracking_chunk(&self, pos: ChunkPos) -> Vec<DimensionInterestSource> {
        self.players_tracking_chunk(pos)
            .into_iter()
            .map(DimensionInterestSource::Player)
            .chain(self.observers.iter().filter_map(|(observer_id, state)| {
                state
                    .view
                    .visible_chunks
                    .contains(&pos)
                    .then_some(DimensionInterestSource::Observer(*observer_id))
            }))
            .collect()
    }

    fn rebuild_aggregate_interest_positions(&mut self) -> bool {
        let player_positions = self
            .players
            .values()
            .flat_map(|state| state.visible_chunks.iter().copied())
            .collect::<BTreeSet<_>>();
        let observer_positions = self
            .observers
            .values()
            .flat_map(|state| state.view.visible_chunks.iter().copied())
            .collect::<BTreeSet<_>>();
        let observer_simulation_positions = self
            .observers
            .values()
            .filter(|state| state.simulation == ObserverSimulationInterest::BlockAndEntityTicking)
            .flat_map(|state| state.view.visible_chunks.iter().copied())
            .collect::<BTreeSet<_>>();
        let resident_positions = player_positions
            .union(&observer_positions)
            .copied()
            .collect::<BTreeSet<_>>();
        let simulation_positions = player_positions
            .union(&observer_simulation_positions)
            .copied()
            .collect::<BTreeSet<_>>();
        let changed = player_positions != self.aggregate_player_ticket_positions
            || resident_positions != self.aggregate_resident_positions
            || simulation_positions != self.aggregate_simulation_ticket_positions;
        self.aggregate_player_ticket_positions = player_positions;
        self.aggregate_resident_positions = resident_positions;
        self.aggregate_simulation_ticket_positions = simulation_positions;
        changed
    }
}

pub(crate) fn chunk_positions_for_view(view: &ChunkView) -> BTreeSet<ChunkPos> {
    chunk_positions_for_center_radius(view.center, view.chunk_tracking_radius)
}

fn chunk_positions_for_center_radius(center: ChunkPos, radius: u32) -> BTreeSet<ChunkPos> {
    let radius = i32::try_from(radius).expect("chunk tracking radius exceeds i32");
    let min_x = center.x - radius;
    let max_x = center.x + radius;
    let min_z = center.z - radius;
    let max_z = center.z + radius;
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

    fn row_at_x(x: i32, z_min: i32, z_max: i32) -> Vec<ChunkPos> {
        (z_min..=z_max).map(|z| ChunkPos::new(x, z)).collect()
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
    fn default_policy_matches_dedicated_server_view_distance() {
        let policy = PlayerChunkTrackingPolicy::default();

        assert_eq!(
            policy.clamp_view(&view(ChunkPos::new(0, 0), 32)),
            view(
                ChunkPos::new(0, 0),
                DEFAULT_DEDICATED_SERVER_CHUNK_TRACKING_RADIUS
            )
        );
    }

    #[test]
    fn java_max_policy_allows_full_client_render_distance_range() {
        let policy = PlayerChunkTrackingPolicy::java_max();

        assert_eq!(
            policy.clamp_view(&view(ChunkPos::new(0, 0), 32)),
            view(ChunkPos::new(0, 0), 32)
        );
        assert_eq!(
            policy.clamp_view(&view(ChunkPos::new(0, 0), 100)),
            view(ChunkPos::new(0, 0), JAVA_MAX_VIEW_DISTANCE)
        );
    }

    #[test]
    fn per_player_views_diff_visible_sets_and_aggregate_tickets() {
        let mut tracking = PlayerChunkTracking::new(PlayerChunkTrackingPolicy::new(4, 4));
        let player_a = ServerPlayerId::from_raw_for_tests(0);
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
    fn unload_hysteresis_keeps_tiny_debug_views_exact() {
        let mut tracking = PlayerChunkTracking::new(
            PlayerChunkTrackingPolicy::new(4, 4).with_unload_hysteresis_chunks(1),
        );
        let player = ServerPlayerId::from_raw_for_tests(0);

        tracking.set_requested_view(player, view(ChunkPos::new(0, 0), 0));
        let moved = tracking.set_requested_view(player, view(ChunkPos::new(1, 0), 0));

        assert_eq!(moved.added_chunks, vec![ChunkPos::new(1, 0)]);
        assert_eq!(moved.removed_chunks, vec![ChunkPos::new(0, 0)]);
        assert!(!tracking.player_tracks_chunk(player, ChunkPos::new(0, 0)));
        assert_eq!(tracking.diagnostics().total_player_visible_chunks, 1);
    }

    #[test]
    fn unload_hysteresis_retains_chunks_inside_unload_margin() {
        let mut tracking = PlayerChunkTracking::new(
            PlayerChunkTrackingPolicy::new(8, 8).with_unload_hysteresis_chunks(1),
        );
        let player = ServerPlayerId::from_raw_for_tests(0);

        let first = tracking.set_requested_view(player, view(ChunkPos::new(0, 0), 3));
        assert_eq!(first.added_chunks.len(), 49);
        assert!(first.removed_chunks.is_empty());
        assert_eq!(tracking.diagnostics().total_player_visible_chunks, 49);

        let moved = tracking.set_requested_view(player, view(ChunkPos::new(1, 0), 3));
        assert_eq!(moved.added_chunks, row_at_x(4, -3, 3));
        assert!(moved.removed_chunks.is_empty());
        assert!(tracking.player_tracks_chunk(player, ChunkPos::new(-3, 0)));
        assert!(tracking.player_tracks_chunk(player, ChunkPos::new(4, 0)));
        assert_eq!(tracking.diagnostics().aggregate_player_ticket_chunks, 56);
        assert_eq!(tracking.diagnostics().total_player_visible_chunks, 56);

        let moved_again = tracking.set_requested_view(player, view(ChunkPos::new(2, 0), 3));
        assert_eq!(moved_again.added_chunks, row_at_x(5, -3, 3));
        assert_eq!(moved_again.removed_chunks, row_at_x(-3, -3, 3));
        assert!(tracking.player_tracks_chunk(player, ChunkPos::new(-2, 0)));
        assert!(!tracking.player_tracks_chunk(player, ChunkPos::new(-3, 0)));
        assert_eq!(tracking.diagnostics().aggregate_player_ticket_chunks, 56);
        assert_eq!(tracking.diagnostics().total_player_visible_chunks, 56);
    }

    #[test]
    fn disconnect_removes_only_that_players_ticket_contribution() {
        let mut tracking = PlayerChunkTracking::new(PlayerChunkTrackingPolicy::new(4, 4));
        let player_a = ServerPlayerId::from_raw_for_tests(0);
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

    #[test]
    fn observers_are_source_owned_clamped_and_simulation_optional() {
        let mut tracking = PlayerChunkTracking::new(PlayerChunkTrackingPolicy::new(4, 4));
        let player = ServerPlayerId::from_raw_for_tests(0);
        let resident_observer = ObserverId::from_raw(7);
        let ticking_observer = ObserverId::from_raw(8);
        tracking.set_requested_view(player, view(ChunkPos::new(0, 0), 0));

        let resident = tracking.set_observer_requested_view(
            resident_observer,
            view(ChunkPos::new(0, 0), u32::MAX),
            ObserverSimulationInterest::ResidencyOnly,
        );
        assert_eq!(resident.accepted, Some(view(ChunkPos::new(0, 0), 4)));
        assert_eq!(
            tracking.accepted_observer_view(resident_observer),
            Some(&view(ChunkPos::new(0, 0), 4))
        );
        assert_eq!(tracking.aggregate_player_ticket_positions().len(), 1);
        assert_eq!(tracking.aggregate_resident_positions().len(), 81);
        assert_eq!(tracking.aggregate_simulation_ticket_positions().len(), 1);

        tracking.set_observer_requested_view(
            ticking_observer,
            view(ChunkPos::new(8, 0), 0),
            ObserverSimulationInterest::BlockAndEntityTicking,
        );
        assert_eq!(tracking.aggregate_resident_positions().len(), 82);
        assert_eq!(tracking.aggregate_simulation_ticket_positions().len(), 2);

        tracking.remove_observer(resident_observer);
        assert_eq!(tracking.aggregate_resident_positions().len(), 2);
        assert_eq!(tracking.aggregate_simulation_ticket_positions().len(), 2);
        tracking.set_observer_requested_view(
            resident_observer,
            view(ChunkPos::new(0, 0), u32::MAX),
            ObserverSimulationInterest::ResidencyOnly,
        );
        assert_eq!(tracking.aggregate_resident_positions().len(), 82);

        tracking.remove_player(player);
        assert!(tracking.aggregate_player_ticket_positions().is_empty());
        assert_eq!(tracking.aggregate_resident_positions().len(), 82);
        assert_eq!(tracking.aggregate_simulation_ticket_positions().len(), 1);
        tracking.remove_observer(resident_observer);
        assert_eq!(
            tracking.aggregate_resident_positions(),
            BTreeSet::from([ChunkPos::new(8, 0)])
        );
        tracking.remove_observer(ticking_observer);
        assert!(tracking.aggregate_resident_positions().is_empty());
        assert!(tracking.aggregate_simulation_ticket_positions().is_empty());
    }
}
