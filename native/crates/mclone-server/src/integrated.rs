//! Integrated single-process server facade.
//!
//! Move-only home for `IntegratedServer`, the in-process wrapper that drives a
//! `ChunkScheduler` plus the fluid tick list and turns scheduler events into
//! protocol `ServerUpdate`s. Roughly mirrors Java's integrated-server glue over
//! `ServerChunkCache`; the heavy chunk-management logic lives in the scheduler.

use std::time::Duration;

use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkPos, Vec3d};
use mclone_protocol::{
    ChunkView, ClientCommand, InteractionHand, MovePlayerCommand, PlayerActionCommand,
    PlayerActionKind, ServerUpdate, SetCarriedItemCommand, UseItemOnCommand,
};
use mclone_worldgen::biome::OverworldBiomeSource;
use mclone_worldgen::block::RawBlockId;

#[cfg(target_arch = "wasm32")]
use crate::WasmServerJobWorkerConfig;
use crate::entities::{EntityTracking, RoutedEntityUpdate, ServerEntityState, ServerEntityStore};
use crate::game_mode::ServerInteractionContext;
use crate::inventory::ServerInventory;
use crate::placement::DebugBlockItem;
use crate::player::{MovePlayerApplyResult, ServerPlayerState};
use crate::player_chunk_tracking::{PlayerChunkTracking, PlayerChunkTrackingPolicy};
use crate::players::{ServerPlayerId, ServerPlayerList};
use crate::remote_players::{RemotePlayerState, RemotePlayerTracking, RoutedRemotePlayerUpdate};
use crate::spawn::find_safe_surface_spawn;
use crate::timing::{simulation_timing_elapsed_us, simulation_timing_start};
use crate::{
    ChunkLoadingProgress, ChunkLoadingProgressSnapshot, ChunkLoadingProgressStats, ChunkScheduler,
    ChunkSchedulerEvent, ChunkSnapshotStore, ChunkStoreError, ChunkStoreResult, FluidKind,
    FluidTickList, NullChunkSnapshotStore, PlayerChunkTrackingDiagnostics,
    ServerSimulationTickReport, ServerSimulationTickTiming, ServerTickReport, ServerTickTiming,
    WorldBlockPos,
};

#[derive(Debug)]
pub struct IntegratedServer {
    seed: i64,
    scheduler: ChunkScheduler,
    pub(crate) liquid_ticks: FluidTickList,
    simulation_tick: u64,
    day_time: u64,
    day_time_frozen: bool,
    initial_spawn_center: Option<ChunkPos>,
    player: ServerPlayerState,
    inventory: ServerInventory,
    dedicated_players: ServerPlayerList,
    chunk_tracking: PlayerChunkTracking,
    remote_players: RemotePlayerTracking,
    entities: ServerEntityStore,
    entity_tracking: EntityTracking,
    loading_progress: ChunkLoadingProgress,
}

/// Vanilla overworld spawns at morning (`dayTime` 1000), not midnight.
pub const INITIAL_DAY_TIME: u64 = 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandTarget {
    Local,
    Dedicated(ServerPlayerId),
}

impl IntegratedServer {
    pub fn new(seed: i64) -> Self {
        Self::with_chunk_store(seed, Box::<NullChunkSnapshotStore>::default())
    }

    pub fn with_chunk_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self::with_scheduler(seed, ChunkScheduler::with_store(seed, store))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn with_wasm_job_workers(seed: i64, config: WasmServerJobWorkerConfig) -> Self {
        Self::with_scheduler(
            seed,
            ChunkScheduler::with_wasm_job_workers(
                seed,
                Box::<NullChunkSnapshotStore>::default(),
                config,
            ),
        )
    }

    fn with_scheduler(seed: i64, scheduler: ChunkScheduler) -> Self {
        let mut chunk_tracking = PlayerChunkTracking::new(PlayerChunkTrackingPolicy::default());
        chunk_tracking.add_player(ServerPlayerId::LOCAL);
        let loading_progress = ChunkLoadingProgress::new(runtime_chunk_target_status(&scheduler));
        Self {
            seed,
            scheduler,
            liquid_ticks: FluidTickList::new(),
            simulation_tick: 0,
            day_time: INITIAL_DAY_TIME,
            day_time_frozen: false,
            initial_spawn_center: None,
            player: ServerPlayerState::default(),
            inventory: ServerInventory::default(),
            dedicated_players: ServerPlayerList::default(),
            chunk_tracking,
            remote_players: RemotePlayerTracking::default(),
            entities: ServerEntityStore::default(),
            entity_tracking: EntityTracking::default(),
            loading_progress,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn simulation_tick(&self) -> u64 {
        self.simulation_tick
    }

    /// Authoritative world day-time in ticks, driving the day/night cycle.
    pub const fn day_time(&self) -> u64 {
        self.day_time
    }

    /// Set the authoritative day-time. Debug hook for forcing a starting time.
    pub fn set_day_time(&mut self, day_time: u64) {
        self.day_time = day_time;
    }

    /// Freeze or resume the day/night clock. While frozen, simulation ticks leave
    /// `day_time` unchanged (debug hook for inspecting a fixed time of day).
    pub fn set_day_time_frozen(&mut self, frozen: bool) {
        self.day_time_frozen = frozen;
    }

    pub fn schedule_fluid_tick(&mut self, pos: WorldBlockPos, fluid: FluidKind, delay: i32) {
        self.liquid_ticks
            .schedule_tick(pos, fluid, delay, self.simulation_tick);
    }

    pub fn scheduled_fluid_tick_count(&self) -> usize {
        self.liquid_ticks.size()
    }

    pub fn loading_progress_stats(&self) -> Option<ChunkLoadingProgressStats> {
        self.loading_progress.stats()
    }

    pub fn loading_progress_snapshot(&self) -> Option<ChunkLoadingProgressSnapshot> {
        self.loading_progress.snapshot()
    }

    pub fn view_readiness_snapshot(&self) -> Option<ChunkLoadingProgressSnapshot> {
        let view = self.chunk_tracking.accepted_view(ServerPlayerId::LOCAL)?;
        Some(self.scheduler.view_readiness_snapshot(view))
    }

    pub fn lighting_enabled(&self) -> bool {
        self.scheduler.lighting_enabled()
    }

    pub fn set_lighting_enabled(&mut self, enabled: bool) {
        self.scheduler.set_lighting_enabled(enabled);
        self.loading_progress
            .set_target_status(runtime_chunk_target_status(&self.scheduler));
    }

    pub fn add_dedicated_player(&mut self) -> ServerPlayerId {
        let player_id = self.dedicated_players.add();
        self.chunk_tracking.add_player(player_id);
        self.remote_players.add_player(player_id);
        player_id
    }

    pub fn remove_dedicated_player(&mut self, player_id: ServerPlayerId) -> bool {
        if self.dedicated_players.remove(player_id).is_none() {
            return false;
        }
        let routes = self.remote_players.remove_player(player_id);
        self.route_remote_player_updates(routes);
        self.entity_tracking.remove_observer(player_id);
        self.remove_player_chunk_tracking(player_id);
        true
    }

    pub fn dedicated_player_count(&self) -> usize {
        self.dedicated_players.len()
    }

    pub fn dedicated_player_position(&self, player_id: ServerPlayerId) -> Option<Vec3d> {
        self.dedicated_players.position(player_id)
    }

    pub fn handle_command(&mut self, command: ClientCommand) -> Vec<ServerUpdate> {
        self.try_handle_command(command)
            .expect("integrated server command failed")
    }

    pub fn try_handle_command(
        &mut self,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_handle_command_for_target(CommandTarget::Local, command)
    }

    pub fn try_handle_command_for_player(
        &mut self,
        player_id: ServerPlayerId,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_handle_command_for_target(CommandTarget::Dedicated(player_id), command)
    }

    fn try_handle_command_for_target(
        &mut self,
        target: CommandTarget,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        match command {
            ClientCommand::SetChunkView(view) => self.set_chunk_view_for_target(target, view),
            ClientCommand::MovePlayer(command) => {
                self.handle_move_player_for_target(target, command)
            }
            ClientCommand::AcceptTeleport(command) => {
                self.handle_accept_teleport_for_target(target, command.id)
            }
            ClientCommand::SetCarriedItem(command) => {
                self.handle_set_carried_item_for_target(target, command)
            }
            ClientCommand::PlayerAction(command) => {
                self.handle_player_action_for_target(target, command)
            }
            ClientCommand::UseItemOn(command) => {
                self.handle_use_item_on_for_target(target, command)
            }
        }
    }

    pub fn poll(&mut self) -> Vec<ServerUpdate> {
        self.try_poll().expect("integrated server poll failed")
    }

    pub fn try_poll(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_poll_for_target(CommandTarget::Local)
    }

    pub fn try_poll_for_player(
        &mut self,
        player_id: ServerPlayerId,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.try_poll_for_target(CommandTarget::Dedicated(player_id))
    }

    fn try_poll_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        let events = self.scheduler.poll()?;
        self.apply_scheduler_events_for_target(target, events)
    }

    pub fn tick(&mut self) -> Vec<ServerUpdate> {
        self.try_tick().expect("integrated server tick failed")
    }

    pub fn try_tick(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        Ok(self.try_simulation_tick_report()?.updates)
    }

    pub fn tick_report(&mut self) -> ServerTickReport {
        self.try_tick_report()
            .expect("integrated server tick report failed")
    }

    pub fn try_tick_report(&mut self) -> ChunkStoreResult<ServerTickReport> {
        self.try_tick_report_for_target(CommandTarget::Local)
    }

    fn try_tick_report_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<ServerTickReport> {
        self.ensure_target_exists(target)?;
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let report = self.scheduler.tick_report()?;
        let scheduler_report_us = simulation_timing_elapsed_us(scheduler_start);
        let scheduler_event_count = report.events.len();
        let scheduler_apply_start = simulation_timing_start();
        let updates = self.apply_scheduler_events_for_target(target, report.events)?;
        let scheduler_apply_events_us = simulation_timing_elapsed_us(scheduler_apply_start);
        let chunk_tracking = self.chunk_tracking_diagnostics();
        let scheduler_timing = report.timing;
        Ok(ServerTickReport {
            ticket_tick: report.ticket_tick,
            block_ticking_chunks: report.block_ticking_chunks,
            entity_ticking_chunks: report.entity_ticking_chunks,
            pending_unloads_processed: report.pending_unloads_processed,
            scheduler_event_count,
            chunk_tracking,
            updates,
            timing: ServerTickTiming {
                total_us: simulation_timing_elapsed_us(total_start),
                scheduler_report_us,
                scheduler_purge_stale_tickets_us: scheduler_timing.purge_stale_tickets_us,
                scheduler_reconcile_holders_us: scheduler_timing.reconcile_holders_us,
                scheduler_publish_completed_us: scheduler_timing.publish_completed_us,
                scheduler_pending_unload_us: scheduler_timing.pending_unload_us,
                scheduler_apply_events_us,
            },
        })
    }

    pub fn simulation_tick_report(&mut self) -> ServerSimulationTickReport {
        self.try_simulation_tick_report()
            .expect("integrated server simulation tick report failed")
    }

    pub fn try_simulation_tick_report(&mut self) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.try_simulation_tick_report_for_target(CommandTarget::Local)
    }

    pub fn try_simulation_tick_report_for_player(
        &mut self,
        player_id: ServerPlayerId,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        self.try_simulation_tick_report_for_target(CommandTarget::Dedicated(player_id))
    }

    fn try_simulation_tick_report_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<ServerSimulationTickReport> {
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let tick_report = self.try_tick_report_for_target(target)?;
        let scheduler_tick_us = simulation_timing_elapsed_us(scheduler_start);
        let tick_timing = tick_report.timing;

        let simulation_tick = self.simulation_tick.saturating_add(1);
        self.simulation_tick = simulation_tick;
        self.mark_player_tick_boundaries();

        // Advance the day/night clock one tick (Java `ServerLevel.tickTime` with
        // `doDaylightCycle` on). Coupled to the simulation tick cadence, which is
        // itself frame-driven in the current runtime; revisit if/when ticks are
        // fixed-step. Skipped while frozen (debug `--freeze-time`).
        if !self.day_time_frozen {
            self.day_time = self.day_time.wrapping_add(1);
        }

        let block_tick_start = simulation_timing_start();
        let block_tick_chunks = run_noop_simulation_phase(&tick_report.block_ticking_chunks);
        let block_tick_us = simulation_timing_elapsed_us(block_tick_start);

        let fluid_tick_start = simulation_timing_start();
        let (fluid_report, mut fluid_events) = self.liquid_ticks.tick(
            simulation_tick,
            &tick_report.entity_ticking_chunks,
            &mut self.scheduler,
        );
        let fluid_tick_us = simulation_timing_elapsed_us(fluid_tick_start);
        fluid_events.extend(self.scheduler.drain_pending_block_delta_events());
        let fluid_event_count = fluid_events.len();

        let entity_tick_start = simulation_timing_start();
        let entity_tick_chunks = run_noop_simulation_phase(&tick_report.entity_ticking_chunks);
        let entity_updates = self
            .entities
            .tick_stationary(&tick_report.entity_ticking_chunks);
        let entity_tick_us = simulation_timing_elapsed_us(entity_tick_start);

        let mut updates = tick_report.updates;
        updates.push(ServerUpdate::TimeUpdate {
            day_time: self.day_time,
        });
        let fluid_event_apply_start = simulation_timing_start();
        self.route_scheduler_events(fluid_events);
        self.reconcile_entity_subjects(entity_updates, true);
        updates.extend(self.drain_chunk_updates_for_target(target)?);
        let fluid_event_apply_us = simulation_timing_elapsed_us(fluid_event_apply_start);
        let chunk_tracking = self.chunk_tracking_diagnostics();

        Ok(ServerSimulationTickReport {
            simulation_tick,
            chunk_tick: tick_report.ticket_tick,
            block_tick_chunks,
            fluid_ticks_executed: fluid_report.executed_ticks,
            fluid_due_ticks: fluid_report.due_ticks,
            deferred_fluid_ticks: fluid_report.deferred_due_ticks,
            fluid_mutated_blocks: fluid_report.mutated_blocks,
            fluid_snapshot_events: fluid_report.snapshot_events,
            fluid_event_count,
            scheduled_fluid_ticks: fluid_report.scheduled_ticks,
            entity_tick_chunks,
            pending_unloads_processed: tick_report.pending_unloads_processed,
            scheduler_event_count: tick_report.scheduler_event_count,
            chunk_tracking,
            updates,
            timing: ServerSimulationTickTiming {
                total_us: simulation_timing_elapsed_us(total_start),
                scheduler_tick_us,
                scheduler_report_us: tick_timing.scheduler_report_us,
                scheduler_purge_stale_tickets_us: tick_timing.scheduler_purge_stale_tickets_us,
                scheduler_reconcile_holders_us: tick_timing.scheduler_reconcile_holders_us,
                scheduler_publish_completed_us: tick_timing.scheduler_publish_completed_us,
                scheduler_pending_unload_us: tick_timing.scheduler_pending_unload_us,
                scheduler_apply_events_us: tick_timing.scheduler_apply_events_us,
                block_tick_us,
                fluid_tick_us,
                fluid_event_apply_us,
                fluid_due_scan_us: fluid_report.due_scan_us,
                fluid_remove_due_us: fluid_report.remove_due_us,
                fluid_tick_fluid_us: fluid_report.tick_fluid_us,
                fluid_set_block_us: fluid_report.set_block_us,
                entity_tick_us,
            },
        })
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.scheduler.loaded_chunk_count()
    }

    pub fn chunk_tracking_diagnostics(&self) -> PlayerChunkTrackingDiagnostics {
        self.chunk_tracking.diagnostics()
    }

    pub fn pending_job_count(&self) -> usize {
        self.scheduler.pending_job_count()
    }

    pub fn pending_publication_count(&self) -> usize {
        self.scheduler.pending_publication_count()
    }

    pub fn wait_for_worldgen_completion(&mut self, timeout: Duration) -> bool {
        self.scheduler.wait_for_worldgen_completion(timeout)
    }

    pub fn wait_for_light_completion(&mut self, timeout: Duration) -> bool {
        self.scheduler.wait_for_light_completion(timeout)
    }

    pub fn scheduler(&self) -> &ChunkScheduler {
        &self.scheduler
    }

    pub fn scheduler_mut(&mut self) -> &mut ChunkScheduler {
        &mut self.scheduler
    }

    pub fn save_dirty_chunks(&mut self) -> ChunkStoreResult<usize> {
        self.scheduler.save_dirty_chunks()
    }

    fn set_chunk_view_for_target(
        &mut self,
        target: CommandTarget,
        view: ChunkView,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        if target == CommandTarget::Local {
            self.loading_progress.set_view(&view);
        }
        self.set_initial_spawn_center_for_target(target, view.center)?;
        let player_id = target.player_id();
        let change = self.chunk_tracking.set_requested_view(player_id, view);
        for pos in &change.removed_chunks {
            self.chunk_tracking.queue_unload_for_player(player_id, *pos);
        }
        for pos in &change.added_chunks {
            if let Some(snapshot) = self.scheduler.client_visible_snapshot(*pos) {
                self.chunk_tracking
                    .queue_snapshot_for_player(player_id, snapshot);
            }
        }
        let events = if change.aggregate_changed || change.priority_centers_changed {
            self.scheduler.apply_player_ticket_positions_with_priority(
                self.chunk_tracking.aggregate_player_ticket_positions(),
                self.chunk_tracking
                    .aggregate_player_ticket_priority_centers(),
            )?
        } else {
            Vec::new()
        };
        self.apply_scheduler_events_for_target(target, events)
    }

    fn handle_move_player_for_target(
        &mut self,
        target: CommandTarget,
        command: MovePlayerCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let simulation_tick = self.simulation_tick;
        let player = self.player_mut_for_target(target)?;
        let result = player.apply_move_player(command);
        let updates = match result {
            MovePlayerApplyResult::Accepted | MovePlayerApplyResult::RejectedInvalid => Vec::new(),
            MovePlayerApplyResult::AwaitingTeleport => player
                .resend_pending_correction_update(simulation_tick)
                .map(ServerUpdate::PlayerPosition)
                .into_iter()
                .collect(),
        };
        if result == MovePlayerApplyResult::Accepted {
            self.reconcile_remote_player_subject(target.player_id(), true);
        }
        Ok(updates)
    }

    fn handle_accept_teleport_for_target(
        &mut self,
        target: CommandTarget,
        id: u32,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        if self.player_mut_for_target(target)?.accept_teleport(id) {
            self.reconcile_remote_player_subject(target.player_id(), true);
        }
        Ok(Vec::new())
    }

    fn handle_set_carried_item_for_target(
        &mut self,
        target: CommandTarget,
        command: SetCarriedItemCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.inventory_mut_for_target(target)?
            .apply_set_carried_item(command);
        Ok(Vec::new())
    }

    fn handle_player_action_for_target(
        &mut self,
        target: CommandTarget,
        command: PlayerActionCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        if command.kind == PlayerActionKind::DebugInstantBreak {
            let player_position = self.player_for_target(target)?.position();
            let context = ServerInteractionContext::debug_creative(player_position);
            if context.may_break_block(command.pos) {
                self.set_block_debug(command.pos, AIR_BLOCK_STATE_ID);
            }
        }
        self.drain_pending_block_delta_updates_for_target(target)
    }

    fn handle_use_item_on_for_target(
        &mut self,
        target: CommandTarget,
        command: UseItemOnCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        if let Some((target, block_state)) =
            self.held_item_place_target_for_target(target, command)?
        {
            self.set_block_debug(target, block_state);
        }
        self.drain_pending_block_delta_updates_for_target(target)
    }

    fn held_item_place_target_for_target(
        &self,
        target: CommandTarget,
        command: UseItemOnCommand,
    ) -> ChunkStoreResult<Option<(BlockPos, BlockStateId)>> {
        if command.hand != InteractionHand::MainHand {
            return Ok(None);
        }
        let block_state = match self.inventory_for_target(target)?.selected_block_state() {
            Some(block_state) => block_state,
            None => return Ok(None),
        };
        let Some(block_id) = raw_block_id_from_block_state(block_state) else {
            return Ok(None);
        };
        let player_position = self.player_for_target(target)?.position();
        let context = ServerInteractionContext::debug_creative(player_position);
        if !context.may_use_item_on(command.hit) {
            return Ok(None);
        }
        let Some(block_item) = DebugBlockItem::new(block_id) else {
            return Ok(None);
        };
        let Some(clicked_block) = self.scheduler.block_at_world(command.hit.block_pos) else {
            return Ok(None);
        };
        let relative_pos = command.hit.block_pos.relative(command.hit.direction);
        let relative_block = self.scheduler.block_at_world(relative_pos);
        let Some(placement) = block_item.use_on(command.hit, clicked_block, relative_block) else {
            return Ok(None);
        };
        Ok(context
            .may_place_at(placement.pos)
            .then_some((placement.pos, block_state)))
    }

    fn set_block_debug(&mut self, pos: BlockPos, block_state: BlockStateId) -> bool {
        let Some(block_id) = raw_block_id_from_block_state(block_state) else {
            return false;
        };
        self.scheduler.set_block_at_world(pos, block_id)
    }

    fn drain_pending_block_delta_updates_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let events = self.scheduler.drain_pending_block_delta_events();
        self.apply_scheduler_events_for_target(target, events)
    }

    fn apply_scheduler_events_for_target(
        &mut self,
        target: CommandTarget,
        events: Vec<ChunkSchedulerEvent>,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        self.route_scheduler_events(events);
        self.reconcile_remote_players_for_target_observer(target);
        let initial_spawn_update = self.initial_spawn_update_for_target(target)?;
        self.reconcile_entities_for_target_observer(target);
        let mut updates = self.drain_chunk_updates_for_target(target)?;
        if let Some(update) = initial_spawn_update {
            updates.push(ServerUpdate::PlayerPosition(update));
        }
        Ok(updates)
    }

    fn route_scheduler_events(&mut self, events: Vec<ChunkSchedulerEvent>) {
        for event in events {
            match event {
                ChunkSchedulerEvent::SnapshotReady(snapshot) => {
                    self.chunk_tracking
                        .queue_snapshot_for_tracking_players(snapshot);
                }
                ChunkSchedulerEvent::Unloaded { pos } => {
                    self.loading_progress.clear_chunk(pos);
                    self.chunk_tracking.queue_unload_for_tracking_players(pos);
                }
                ChunkSchedulerEvent::SectionBlockUpdates {
                    pos,
                    section_y,
                    updates,
                } => {
                    self.chunk_tracking
                        .queue_section_updates_for_tracking_players(pos, section_y, updates);
                }
                ChunkSchedulerEvent::FluidTickScheduled { pos, fluid, delay } => {
                    self.liquid_ticks
                        .schedule_tick(pos, fluid, delay, self.simulation_tick);
                }
                ChunkSchedulerEvent::StatusChanged { pos, status, step } => {
                    self.loading_progress
                        .record_status_change(pos, status, step);
                }
            }
        }
    }

    fn route_remote_player_updates(&mut self, routes: Vec<RoutedRemotePlayerUpdate>) {
        for route in routes {
            self.chunk_tracking
                .queue_update_for_player(route.recipient, route.update);
        }
    }

    fn route_entity_updates(&mut self, routes: Vec<RoutedEntityUpdate>) {
        for route in routes {
            self.chunk_tracking
                .queue_update_for_player(route.recipient, route.update);
        }
    }

    fn reconcile_remote_players_for_target_observer(&mut self, target: CommandTarget) {
        let Some(observer) = target.dedicated_player_id() else {
            return;
        };
        let states = self.remote_player_states();
        let chunk_tracking = &self.chunk_tracking;
        let routes = self
            .remote_players
            .reconcile_observer(observer, &states, |player_id, pos| {
                chunk_tracking.player_tracks_chunk(player_id, pos)
            });
        self.route_remote_player_updates(routes);
    }

    fn reconcile_remote_player_subject(
        &mut self,
        subject: ServerPlayerId,
        emit_existing_updates: bool,
    ) {
        if !self.dedicated_players.contains(subject) {
            return;
        }
        let Some(state) = self.remote_player_state(subject) else {
            return;
        };
        let observers = self
            .dedicated_players
            .iter()
            .map(|(player_id, _)| player_id)
            .collect::<Vec<_>>();
        let chunk_tracking = &self.chunk_tracking;
        let routes = self.remote_players.reconcile_subject(
            state,
            observers,
            |player_id, pos| chunk_tracking.player_tracks_chunk(player_id, pos),
            emit_existing_updates,
        );
        self.route_remote_player_updates(routes);
    }

    fn remote_player_states(&self) -> Vec<RemotePlayerState> {
        self.dedicated_players
            .iter()
            .map(|(player_id, _)| {
                self.remote_player_state(player_id)
                    .expect("iterated dedicated player must have state")
            })
            .collect()
    }

    fn remote_player_state(&self, player_id: ServerPlayerId) -> Option<RemotePlayerState> {
        let player = self.dedicated_players.get(player_id)?;
        Some(RemotePlayerState {
            player_id,
            position: player.state.position(),
            y_rot_degrees: player.state.y_rot_degrees(),
            x_rot_degrees: player.state.x_rot_degrees(),
            on_ground: player.state.on_ground(),
            publishable: player.state.has_accepted_position(),
        })
    }

    fn reconcile_entities_for_target_observer(&mut self, target: CommandTarget) {
        let observer = target.player_id();
        let states = self.entities.states();
        let chunk_tracking = &self.chunk_tracking;
        let routes =
            self.entity_tracking
                .reconcile_observer(observer, &states, |player_id, pos| {
                    chunk_tracking.player_tracks_chunk(player_id, pos)
                });
        self.route_entity_updates(routes);
    }

    fn reconcile_entity_subjects(
        &mut self,
        subjects: impl IntoIterator<Item = ServerEntityState>,
        emit_existing_updates: bool,
    ) {
        let observers = self.player_observers();
        let chunk_tracking = &self.chunk_tracking;
        let mut routes = Vec::new();
        for subject in subjects {
            routes.extend(self.entity_tracking.reconcile_subject(
                subject,
                observers.iter().copied(),
                |player_id, pos| chunk_tracking.player_tracks_chunk(player_id, pos),
                emit_existing_updates,
            ));
        }
        self.route_entity_updates(routes);
    }

    fn player_observers(&self) -> Vec<ServerPlayerId> {
        std::iter::once(ServerPlayerId::LOCAL)
            .chain(
                self.dedicated_players
                    .iter()
                    .map(|(player_id, _)| player_id),
            )
            .collect()
    }

    fn drain_chunk_updates_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        self.ensure_target_exists(target)?;
        Ok(self.chunk_tracking.drain_updates(target.player_id()))
    }

    fn initial_spawn_update_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Option<mclone_protocol::PlayerPositionUpdate>> {
        let player = self.player_for_target(target)?;
        if !player.needs_initial_position_sync() {
            return Ok(None);
        }
        let Some(center) = self.initial_spawn_center_for_target(target)? else {
            return Ok(None);
        };
        let seed = self.seed;
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        let Some(position) = find_safe_surface_spawn(
            center,
            |pos| self.scheduler.block_at_world(pos),
            |x, z| biome_source.get_block_position_biome_definition(seed, x, z),
            |chunk| self.scheduler.client_visible_snapshot(chunk).is_some(),
        ) else {
            return Ok(None);
        };
        self.entities.ensure_starter_passive_near_spawn(position);
        let simulation_tick = self.simulation_tick;
        Ok(Some(
            self.player_mut_for_target(target)?.initial_position_update(
                position,
                0.0,
                0.0,
                simulation_tick,
            ),
        ))
    }

    fn set_initial_spawn_center_for_target(
        &mut self,
        target: CommandTarget,
        center: ChunkPos,
    ) -> ChunkStoreResult<()> {
        match target {
            CommandTarget::Local => {
                if self.initial_spawn_center.is_none() && self.player.needs_initial_position_sync()
                {
                    self.initial_spawn_center = Some(center);
                }
                Ok(())
            }
            CommandTarget::Dedicated(player_id) => {
                let player = self
                    .dedicated_players
                    .get_mut(player_id)
                    .ok_or_else(|| unknown_player_error(player_id))?;
                if player.initial_spawn_center.is_none()
                    && player.state.needs_initial_position_sync()
                {
                    player.initial_spawn_center = Some(center);
                }
                Ok(())
            }
        }
    }

    fn initial_spawn_center_for_target(
        &self,
        target: CommandTarget,
    ) -> ChunkStoreResult<Option<ChunkPos>> {
        match target {
            CommandTarget::Local => Ok(self.initial_spawn_center),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get(player_id)
                .map(|player| player.initial_spawn_center)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn player_for_target(&self, target: CommandTarget) -> ChunkStoreResult<&ServerPlayerState> {
        match target {
            CommandTarget::Local => Ok(&self.player),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get(player_id)
                .map(|player| &player.state)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn player_mut_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<&mut ServerPlayerState> {
        match target {
            CommandTarget::Local => Ok(&mut self.player),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get_mut(player_id)
                .map(|player| &mut player.state)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn inventory_for_target(&self, target: CommandTarget) -> ChunkStoreResult<&ServerInventory> {
        match target {
            CommandTarget::Local => Ok(&self.inventory),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get(player_id)
                .map(|player| &player.inventory)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn inventory_mut_for_target(
        &mut self,
        target: CommandTarget,
    ) -> ChunkStoreResult<&mut ServerInventory> {
        match target {
            CommandTarget::Local => Ok(&mut self.inventory),
            CommandTarget::Dedicated(player_id) => self
                .dedicated_players
                .get_mut(player_id)
                .map(|player| &mut player.inventory)
                .ok_or_else(|| unknown_player_error(player_id)),
        }
    }

    fn ensure_target_exists(&self, target: CommandTarget) -> ChunkStoreResult<()> {
        match target {
            CommandTarget::Local => Ok(()),
            CommandTarget::Dedicated(player_id) if self.dedicated_players.contains(player_id) => {
                Ok(())
            }
            CommandTarget::Dedicated(player_id) => Err(unknown_player_error(player_id)),
        }
    }

    fn mark_player_tick_boundaries(&mut self) {
        self.player.mark_tick_boundary();
        for player in self.dedicated_players.values_mut() {
            player.state.mark_tick_boundary();
        }
    }

    fn remove_player_chunk_tracking(&mut self, player_id: ServerPlayerId) {
        let change = self.chunk_tracking.remove_player(player_id);
        if !change.aggregate_changed {
            return;
        }
        let events = self
            .scheduler
            .apply_player_ticket_positions_with_priority(
                self.chunk_tracking.aggregate_player_ticket_positions(),
                self.chunk_tracking
                    .aggregate_player_ticket_priority_centers(),
            )
            .expect("failed to reconcile chunk tracking after dedicated player disconnect");
        self.route_scheduler_events(events);
    }
}

impl CommandTarget {
    const fn player_id(self) -> ServerPlayerId {
        match self {
            CommandTarget::Local => ServerPlayerId::LOCAL,
            CommandTarget::Dedicated(player_id) => player_id,
        }
    }

    const fn dedicated_player_id(self) -> Option<ServerPlayerId> {
        match self {
            CommandTarget::Local => None,
            CommandTarget::Dedicated(player_id) => Some(player_id),
        }
    }
}

fn unknown_player_error(player_id: ServerPlayerId) -> ChunkStoreError {
    ChunkStoreError::InvalidData(format!("unknown server player {player_id}"))
}

fn raw_block_id_from_block_state(block_state: BlockStateId) -> Option<RawBlockId> {
    RawBlockId::try_from(block_state.0).ok()
}

fn runtime_chunk_target_status(scheduler: &ChunkScheduler) -> mclone_core::ChunkStatus {
    if scheduler.lighting_enabled() {
        mclone_core::ChunkStatus::Light
    } else {
        mclone_core::ChunkStatus::Features
    }
}

fn run_noop_simulation_phase(chunks: &[ChunkPos]) -> usize {
    chunks.iter().fold(0, |count, _pos| count + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::ChunkHolder;
    use mclone_core::{BlockHitResult, BlockStateId, Direction, Vec3d};
    use mclone_protocol::{
        AcceptTeleportCommand, EntityId, EntityKind, EntitySnapshot, EntityUpdate,
        PlayerPositionRelativeFlags, RemotePlayerId, RemotePlayerUpdate, ServerUpdate,
    };
    use mclone_worldgen::block::{
        BRICKS, DIRT, GRASS, SNOW, STONE, has_fluid, material_blocks_motion,
    };

    fn last_time_update(report: &ServerSimulationTickReport) -> u64 {
        report
            .updates
            .iter()
            .rev()
            .find_map(|update| match update {
                ServerUpdate::TimeUpdate { day_time } => Some(*day_time),
                _ => None,
            })
            .expect("simulation tick should emit a TimeUpdate")
    }

    fn request_initial_chunk_view(server: &mut IntegratedServer) {
        server.set_lighting_enabled(false);
        let updates = server
            .try_handle_command(ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .expect("set chunk view");
        assert!(
            updates
                .iter()
                .all(|update| !matches!(update, ServerUpdate::PlayerPosition(_)))
        );
    }

    fn wait_for_initial_spawn_update(
        server: &mut IntegratedServer,
    ) -> mclone_protocol::PlayerPositionUpdate {
        let mut spawn = None;
        for _ in 0..60_000 {
            let updates = server.try_poll().expect("poll");
            spawn = spawn.or_else(|| {
                updates.iter().find_map(|update| match update {
                    ServerUpdate::PlayerPosition(update) => Some(*update),
                    _ => None,
                })
            });
            if let Some(spawn) = spawn {
                return spawn;
            }
            if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
                server.wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
        panic!("timed out waiting for initial spawn position update");
    }

    fn load_chunk_view_with_lighting(
        server: &mut IntegratedServer,
        center: ChunkPos,
        lighting_enabled: bool,
    ) {
        server.set_lighting_enabled(lighting_enabled);
        let updates = server
            .try_handle_command(ClientCommand::SetChunkView(ChunkView {
                center,
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .expect("set chunk view");
        accept_player_position_updates(server, &updates);
        for _ in 0..60_000 {
            let updates = server.try_poll().expect("poll");
            accept_player_position_updates(server, &updates);
            if server.pending_job_count() == 0 {
                return;
            }
            if server.pending_publication_count() == 0 {
                server.wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
        panic!("timed out loading center chunk");
    }

    fn load_chunk_view(server: &mut IntegratedServer, center: ChunkPos) {
        load_chunk_view_with_lighting(server, center, false);
    }

    fn load_center_chunk(server: &mut IntegratedServer) {
        load_chunk_view(server, ChunkPos::new(0, 0));
    }

    #[test]
    fn local_chunk_view_routes_status_events_into_loading_progress() {
        let mut server = IntegratedServer::new(0);
        assert_eq!(server.loading_progress_stats(), None);

        load_center_chunk(&mut server);
        for _ in 0..4 {
            if server
                .loading_progress_stats()
                .is_some_and(|stats| stats.playable_chunk_ready)
            {
                break;
            }
            let updates = server.try_poll().expect("poll");
            accept_player_position_updates(&mut server, &updates);
        }

        let stats = server
            .loading_progress_stats()
            .expect("local chunk view should create loading progress stats");
        assert_eq!(stats.center, ChunkPos::new(0, 0));
        assert_eq!(stats.target_radius, 0);
        assert_eq!(stats.target_status, mclone_core::ChunkStatus::Features);
        assert_eq!(stats.target_chunk_count, 1);
        assert_eq!(stats.target_ready_chunks, 1);
        assert_eq!(stats.playable_chunk, ChunkPos::new(0, 0));
        assert!(stats.playable_chunk_ready);
    }

    #[test]
    fn view_readiness_snapshot_tracks_current_accepted_view() {
        let mut server = IntegratedServer::new(0);
        assert_eq!(server.view_readiness_snapshot(), None);

        load_chunk_view(&mut server, ChunkPos::new(0, 0));
        let first = server
            .view_readiness_snapshot()
            .expect("loaded local view should expose readiness snapshot");
        assert_eq!(first.stats.center, ChunkPos::new(0, 0));
        assert_eq!(first.stats.target_radius, 0);
        assert_eq!(first.stats.target_chunk_count, 1);
        assert_eq!(first.stats.target_ready_chunks, 1);
        assert_eq!(first.cells.len(), 1);
        assert!(first.stats.playable_chunk_ready);

        load_chunk_view(&mut server, ChunkPos::new(2, 0));
        let moved = server
            .view_readiness_snapshot()
            .expect("moved local view should expose readiness snapshot");
        assert_eq!(moved.stats.center, ChunkPos::new(2, 0));
        assert_eq!(moved.stats.target_radius, 0);
        assert_eq!(moved.stats.target_chunk_count, 1);
        assert_eq!(moved.stats.target_ready_chunks, 1);
        assert_eq!(moved.cells.len(), 1);
        assert_eq!(moved.cells[0].relative_x, 0);
        assert_eq!(moved.cells[0].relative_z, 0);
        assert!(moved.cells[0].playable);
        assert!(moved.stats.playable_chunk_ready);
    }

    #[test]
    fn view_readiness_snapshot_uses_runtime_target_status() {
        let mut unlit_server = IntegratedServer::new(0);
        load_chunk_view_with_lighting(&mut unlit_server, ChunkPos::new(0, 0), false);
        let unlit = unlit_server
            .view_readiness_snapshot()
            .expect("unlit loaded view should expose readiness snapshot");
        assert_eq!(
            unlit.stats.target_status,
            mclone_core::ChunkStatus::Features
        );
        assert_eq!(unlit.stats.target_ready_chunks, 1);
        assert!(unlit.stats.playable_chunk_ready);

        let mut lit_server = IntegratedServer::new(0);
        load_chunk_view_with_lighting(&mut lit_server, ChunkPos::new(0, 0), true);
        let lit = lit_server
            .view_readiness_snapshot()
            .expect("lit loaded view should expose readiness snapshot");
        assert_eq!(lit.stats.target_status, mclone_core::ChunkStatus::Light);
        assert_eq!(lit.stats.target_ready_chunks, 1);
        assert!(lit.stats.playable_chunk_ready);
    }

    fn accept_player_position_updates(server: &mut IntegratedServer, updates: &[ServerUpdate]) {
        for update in updates {
            let ServerUpdate::PlayerPosition(update) = update else {
                continue;
            };
            let ack_updates = server
                .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                    id: update.teleport_id,
                }))
                .expect("accept initial player position");
            assert!(ack_updates.is_empty());
        }
    }

    fn set_dedicated_chunk_view_and_poll(
        server: &mut IntegratedServer,
        player_id: ServerPlayerId,
        center: ChunkPos,
        radius: u32,
    ) -> Vec<ServerUpdate> {
        let mut updates = server
            .try_handle_command_for_player(
                player_id,
                ClientCommand::SetChunkView(ChunkView {
                    center,
                    render_distance: radius,
                    chunk_tracking_radius: radius,
                }),
            )
            .expect("set dedicated chunk view");
        accept_dedicated_player_position_updates(server, player_id, &updates);
        for _ in 0..60_000 {
            let polled = server.try_poll_for_player(player_id).expect("poll player");
            accept_dedicated_player_position_updates(server, player_id, &polled);
            updates.extend(polled);
            if server.pending_job_count() == 0 {
                return updates;
            }
            if server.pending_publication_count() == 0 {
                server.wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
        panic!("timed out loading dedicated player chunk view");
    }

    fn accept_dedicated_player_position_updates(
        server: &mut IntegratedServer,
        player_id: ServerPlayerId,
        updates: &[ServerUpdate],
    ) {
        for update in updates {
            let ServerUpdate::PlayerPosition(update) = update else {
                continue;
            };
            let ack_updates = server
                .try_handle_command_for_player(
                    player_id,
                    ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                        id: update.teleport_id,
                    }),
                )
                .expect("accept dedicated player position");
            assert!(ack_updates.is_empty());
        }
    }

    fn snapshot_positions(updates: &[ServerUpdate]) -> BTreeSet<ChunkPos> {
        updates
            .iter()
            .filter_map(|update| match update {
                ServerUpdate::ChunkSnapshot(snapshot) => Some(snapshot.pos),
                _ => None,
            })
            .collect()
    }

    fn has_chunk_unload(updates: &[ServerUpdate], pos: ChunkPos) -> bool {
        updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::ChunkUnload { pos: unloaded } if *unloaded == pos))
    }

    fn has_section_block_updates(updates: &[ServerUpdate]) -> bool {
        updates
            .iter()
            .any(|update| matches!(update, ServerUpdate::SectionBlockUpdates { .. }))
    }

    fn remote_player_add(
        updates: &[ServerUpdate],
        player_id: ServerPlayerId,
    ) -> Option<RemotePlayerUpdate> {
        updates.iter().find_map(|update| match update {
            ServerUpdate::RemotePlayerAdd(update)
                if update.id == RemotePlayerId(player_id.as_u64()) =>
            {
                Some(*update)
            }
            _ => None,
        })
    }

    fn remote_player_update(
        updates: &[ServerUpdate],
        player_id: ServerPlayerId,
    ) -> Option<RemotePlayerUpdate> {
        updates.iter().find_map(|update| match update {
            ServerUpdate::RemotePlayerUpdate(update)
                if update.id == RemotePlayerId(player_id.as_u64()) =>
            {
                Some(*update)
            }
            _ => None,
        })
    }

    fn has_remote_player_remove(updates: &[ServerUpdate], player_id: ServerPlayerId) -> bool {
        updates.iter().any(|update| {
            matches!(
                update,
                ServerUpdate::RemotePlayerRemove { id }
                    if *id == RemotePlayerId(player_id.as_u64())
            )
        })
    }

    fn first_entity_snapshot(updates: &[ServerUpdate]) -> Option<EntitySnapshot> {
        updates.iter().find_map(|update| match update {
            ServerUpdate::EntitySnapshot(snapshot) => Some(*snapshot),
            _ => None,
        })
    }

    fn first_entity_update(updates: &[ServerUpdate], id: EntityId) -> Option<EntityUpdate> {
        updates.iter().find_map(|update| match update {
            ServerUpdate::EntityUpdate(update) if update.id == id => Some(*update),
            _ => None,
        })
    }

    fn has_entity_remove(updates: &[ServerUpdate], id: EntityId) -> bool {
        updates.iter().any(
            |update| matches!(update, ServerUpdate::EntityRemove { id: removed } if *removed == id),
        )
    }

    #[test]
    fn dedicated_player_views_keep_disjoint_ticket_sets() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();

        let updates_a =
            set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
        let updates_b =
            set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(4, 0), 0);

        let snapshots_a = snapshot_positions(&updates_a);
        let snapshots_b = snapshot_positions(&updates_b);
        assert!(snapshots_a.contains(&ChunkPos::new(0, 0)));
        assert!(!snapshots_a.contains(&ChunkPos::new(4, 0)));
        assert!(snapshots_b.contains(&ChunkPos::new(4, 0)));
        assert!(!snapshots_b.contains(&ChunkPos::new(0, 0)));
        assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 1);
        assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(4, 0)), 1);
    }

    #[test]
    fn overlapping_player_views_unload_only_for_player_leaving_chunk() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);

        let updates_a =
            set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(1, 0), 0);
        let updates_b = server.try_poll_for_player(player_b).expect("poll player b");

        assert!(has_chunk_unload(&updates_a, ChunkPos::new(0, 0)));
        assert!(!has_chunk_unload(&updates_b, ChunkPos::new(0, 0)));
        assert!(
            server
                .scheduler()
                .holder(ChunkPos::new(0, 0))
                .is_some_and(ChunkHolder::is_client_visible)
        );
        assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 1);
    }

    #[test]
    fn smaller_player_view_receives_fewer_snapshots_than_larger_view() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();

        let updates_a =
            set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
        let updates_b =
            set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 1);

        let snapshots_a = snapshot_positions(&updates_a);
        let snapshots_b = snapshot_positions(&updates_b);
        assert_eq!(snapshots_a.len(), 1);
        assert_eq!(snapshots_b.len(), 9);
        assert!(snapshots_b.len() > snapshots_a.len());
    }

    #[test]
    fn block_delta_routing_sends_only_to_players_tracking_changed_chunk() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(2, 0), 0);
        server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                    position: Vec3d::new(8.5, 80.0, 8.5),
                    y_rot_degrees: 0.0,
                    x_rot_degrees: 0.0,
                    on_ground: true,
                }),
            )
            .expect("move player a");
        let pos = BlockPos::new(8, 80, 8);
        assert!(server.scheduler_mut().set_block_at_world(pos, DIRT));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates_a = server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::PlayerAction(PlayerActionCommand {
                    pos,
                    direction: Direction::Up,
                    kind: PlayerActionKind::DebugInstantBreak,
                }),
            )
            .expect("break block");
        let updates_b = server.try_poll_for_player(player_b).expect("poll player b");

        assert!(has_section_block_updates(&updates_a));
        assert!(!has_section_block_updates(&updates_b));
    }

    #[test]
    fn dedicated_players_publish_remote_state_when_visible() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
        server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                    position: Vec3d::new(8.5, 80.0, 8.5),
                    y_rot_degrees: 0.0,
                    x_rot_degrees: 0.0,
                    on_ground: true,
                }),
            )
            .expect("move player a into tracked chunk");
        let updates_b =
            set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);

        let add_a =
            remote_player_add(&updates_b, player_a).expect("player b should receive player a add");
        assert_eq!(add_a.id, RemotePlayerId(player_a.as_u64()));

        let updates_a = server.try_poll_for_player(player_a).expect("poll player a");
        let add_b = remote_player_add(&updates_a, player_b)
            .expect("player a should receive player b add after b accepts spawn");
        assert_eq!(add_b.id, RemotePlayerId(player_b.as_u64()));

        let moved = Vec3d::new(9.5, 80.0, 8.5);
        server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                    position: moved,
                    y_rot_degrees: 90.0,
                    x_rot_degrees: -15.0,
                    on_ground: true,
                }),
            )
            .expect("move player a");

        let updates_b = server.try_poll_for_player(player_b).expect("poll player b");
        let moved_a = remote_player_update(&updates_b, player_a)
            .expect("player b should receive player a movement update");
        assert_eq!(moved_a.position, moved);
        assert_eq!(moved_a.y_rot_degrees, 90.0);
        assert_eq!(moved_a.x_rot_degrees, -15.0);
        assert!(moved_a.on_ground);
    }

    #[test]
    fn dedicated_remote_players_are_removed_when_the_observer_view_stops_tracking_them() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
        server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                    position: Vec3d::new(8.5, 80.0, 8.5),
                    y_rot_degrees: 0.0,
                    x_rot_degrees: 0.0,
                    on_ground: true,
                }),
            )
            .expect("move player a into tracked chunk");
        let updates_b =
            set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);
        assert!(remote_player_add(&updates_b, player_a).is_some());

        let updates_b =
            set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(4, 0), 0);

        assert!(has_remote_player_remove(&updates_b, player_a));
    }

    #[test]
    fn dedicated_remote_players_are_removed_on_disconnect() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
        server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                    position: Vec3d::new(8.5, 80.0, 8.5),
                    y_rot_degrees: 0.0,
                    x_rot_degrees: 0.0,
                    on_ground: true,
                }),
            )
            .expect("move player a into tracked chunk");
        let updates_b =
            set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);
        assert!(remote_player_add(&updates_b, player_a).is_some());

        assert!(server.remove_dedicated_player(player_a));
        let updates_b = server.try_poll_for_player(player_b).expect("poll player b");

        assert!(has_remote_player_remove(&updates_b, player_a));
    }

    #[test]
    fn dedicated_player_receives_starter_passive_entity_when_visible() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player = server.add_dedicated_player();

        let updates =
            set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);

        let snapshot = first_entity_snapshot(&updates)
            .expect("dedicated player should receive starter passive entity snapshot");
        assert_eq!(snapshot.kind, EntityKind::Cow);
        assert_eq!(snapshot.width, 0.9);
        assert_eq!(snapshot.height, 1.4);
        let chunk = BlockPos::containing(snapshot.position).chunk_pos();
        assert!(chunk.x.abs() <= 2);
        assert!(chunk.z.abs() <= 2);
    }

    #[test]
    fn starter_passive_entity_updates_age_on_simulation_tick() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player = server.add_dedicated_player();
        let updates =
            set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);
        let snapshot = first_entity_snapshot(&updates).expect("entity snapshot");

        let report = server
            .try_simulation_tick_report_for_player(player)
            .expect("tick dedicated player");
        let update = first_entity_update(&report.updates, snapshot.id).expect("entity age update");

        assert_eq!(update.id, snapshot.id);
        assert_eq!(update.position, snapshot.position);
        assert!(update.age_ticks > snapshot.age_ticks);
    }

    #[test]
    fn starter_passive_entity_is_removed_when_observer_view_stops_tracking_it() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player = server.add_dedicated_player();
        let updates =
            set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(0, 0), 2);
        let snapshot = first_entity_snapshot(&updates).expect("entity snapshot");

        let updates =
            set_dedicated_chunk_view_and_poll(&mut server, player, ChunkPos::new(8, 0), 0);

        assert!(has_entity_remove(&updates, snapshot.id));
    }

    #[test]
    fn chunk_tracking_diagnostics_reports_outbound_queue_depth() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        set_dedicated_chunk_view_and_poll(&mut server, player_a, ChunkPos::new(0, 0), 0);
        set_dedicated_chunk_view_and_poll(&mut server, player_b, ChunkPos::new(0, 0), 0);
        server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                    position: Vec3d::new(8.5, 80.0, 8.5),
                    y_rot_degrees: 0.0,
                    x_rot_degrees: 0.0,
                    on_ground: true,
                }),
            )
            .expect("move player a");
        let pos = BlockPos::new(8, 80, 8);
        assert!(server.scheduler_mut().set_block_at_world(pos, DIRT));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates_a = server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::PlayerAction(PlayerActionCommand {
                    pos,
                    direction: Direction::Up,
                    kind: PlayerActionKind::DebugInstantBreak,
                }),
            )
            .expect("break block");

        assert!(has_section_block_updates(&updates_a));
        let diagnostics = server.chunk_tracking_diagnostics();
        assert_eq!(diagnostics.aggregate_player_ticket_chunks, 1);
        assert_eq!(diagnostics.total_player_visible_chunks, 2);
        assert_eq!(diagnostics.total_outbound_queue_depth, 2);
        assert_eq!(diagnostics.max_outbound_queue_depth, 2);
        assert_eq!(
            diagnostics
                .players
                .iter()
                .find(|player| player.player_id == player_b)
                .map(|player| player.outbound_queue_depth),
            Some(2)
        );

        let updates_b = server.try_poll_for_player(player_b).expect("poll player b");
        assert!(has_section_block_updates(&updates_b));
        assert!(
            remote_player_add(&updates_b, player_a).is_some()
                || remote_player_update(&updates_b, player_a).is_some()
        );
        assert_eq!(
            server
                .chunk_tracking_diagnostics()
                .total_outbound_queue_depth,
            0
        );
    }

    #[test]
    fn disconnect_removes_player_chunk_ticket_contribution() {
        let mut server = IntegratedServer::new(12_345);
        server.set_lighting_enabled(false);
        let player_a = server.add_dedicated_player();
        let player_b = server.add_dedicated_player();
        server
            .try_handle_command_for_player(
                player_a,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(0, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .expect("set view a");
        server
            .try_handle_command_for_player(
                player_b,
                ClientCommand::SetChunkView(ChunkView {
                    center: ChunkPos::new(4, 0),
                    render_distance: 0,
                    chunk_tracking_radius: 0,
                }),
            )
            .expect("set view b");
        assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 1);
        assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(4, 0)), 1);

        assert!(server.remove_dedicated_player(player_a));

        assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(0, 0)), 0);
        assert_eq!(server.scheduler().ticket_count_at(ChunkPos::new(4, 0)), 1);
    }

    fn sync_player(server: &mut IntegratedServer, position: Vec3d) {
        let mut current = server.player.position();
        for _ in 0..64 {
            let delta = position.subtract(current);
            if delta.length_sqr() <= 64.0 {
                send_player_move(server, position);
                return;
            }
            let length = delta.length_sqr().sqrt();
            current = current.add(delta.scale(8.0 / length));
            send_player_move(server, current);
            server
                .try_simulation_tick_report()
                .expect("movement helper tick");
        }
        panic!("timed out walking test player to {position:?}");
    }

    fn send_player_move(server: &mut IntegratedServer, position: Vec3d) {
        let updates = server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position,
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }))
            .expect("move player");
        assert!(updates.is_empty());
    }

    fn sync_carried_slot(server: &mut IntegratedServer, slot: u8) {
        let updates = server
            .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
                slot,
            }))
            .expect("set carried item");
        assert!(updates.is_empty());
    }

    fn use_held_item_on(hit: BlockHitResult) -> ClientCommand {
        ClientCommand::UseItemOn(UseItemOnCommand {
            hand: InteractionHand::MainHand,
            hit,
        })
    }

    #[test]
    fn day_time_advances_each_tick_by_default() {
        let mut server = IntegratedServer::new(0);
        let start = server.day_time();
        let report = server.try_simulation_tick_report().expect("tick");
        assert_eq!(server.day_time(), start + 1);
        assert_eq!(last_time_update(&report), start + 1);
    }

    #[test]
    fn frozen_day_time_holds_a_forced_value() {
        let mut server = IntegratedServer::new(0);
        server.set_day_time(23000);
        server.set_day_time_frozen(true);
        for _ in 0..5 {
            let report = server.try_simulation_tick_report().expect("tick");
            assert_eq!(server.day_time(), 23000);
            assert_eq!(last_time_update(&report), 23000);
        }
    }

    #[test]
    fn first_chunk_view_sends_safe_surface_spawn_position() {
        let mut server = IntegratedServer::new(12345);
        request_initial_chunk_view(&mut server);
        let spawn = wait_for_initial_spawn_update(&mut server);

        assert_eq!(spawn.relative, PlayerPositionRelativeFlags::ABSOLUTE);
        assert_eq!(spawn.teleport_id, 1);
        assert_eq!(spawn.y_rot_degrees, 0.0);
        assert_eq!(spawn.x_rot_degrees, 0.0);
        assert_eq!(spawn.position.x - spawn.position.x.floor(), 0.5);
        assert_eq!(spawn.position.z - spawn.position.z.floor(), 0.5);

        let feet = BlockPos::new(
            spawn.position.x.floor() as i32,
            spawn.position.y as i32,
            spawn.position.z.floor() as i32,
        );
        let floor = feet.below();
        let floor_block = server
            .scheduler()
            .block_at_world(floor)
            .expect("spawn floor block");
        let feet_block = server
            .scheduler()
            .block_at_world(feet)
            .expect("spawn feet block");
        let head_block = server
            .scheduler()
            .block_at_world(feet.offset(0, 1, 0))
            .expect("spawn head block");
        assert!(material_blocks_motion(floor_block));
        assert!(!has_fluid(floor_block));
        assert!(!material_blocks_motion(feet_block));
        assert!(!has_fluid(feet_block));
        assert!(!material_blocks_motion(head_block));
        assert!(!has_fluid(head_block));

        assert_eq!(
            server
                .player
                .awaiting_teleport()
                .map(|awaiting| awaiting.id),
            Some(spawn.teleport_id)
        );
        let updates = server
            .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: spawn.teleport_id,
            }))
            .expect("accept spawn");
        assert!(updates.is_empty());
        assert_eq!(server.player.awaiting_teleport(), None);
        assert_eq!(server.player.position(), spawn.position);
    }

    #[test]
    fn seed_789_initial_spawn_uses_surface_not_underground_cave() {
        let mut server = IntegratedServer::new(789);
        request_initial_chunk_view(&mut server);
        let spawn = wait_for_initial_spawn_update(&mut server);
        let feet = BlockPos::new(
            spawn.position.x.floor() as i32,
            spawn.position.y as i32,
            spawn.position.z.floor() as i32,
        );

        for y in feet.y..crate::game_mode::JAVA_OVERWORLD_MAX_BUILD_HEIGHT {
            let block = server
                .scheduler()
                .block_at_world(BlockPos::new(feet.x, y, feet.z))
                .expect("spawn column block above feet");
            assert!(
                !material_blocks_motion(block) && !has_fluid(block),
                "seed 789 spawn selected underground column with blocking/fluid block {block} at y={y}; spawn={:?}",
                spawn.position
            );
        }
    }

    #[test]
    fn move_player_command_updates_server_player_state_without_world_updates() {
        let mut server = IntegratedServer::new(0);

        let updates = server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
                position: Vec3d::new(1.25, 63.0, -4.5),
                y_rot_degrees: 181.0,
                x_rot_degrees: -181.0,
                on_ground: true,
            }))
            .expect("move player");

        assert!(updates.is_empty());
        assert_eq!(server.player.position(), Vec3d::new(1.25, 63.0, -4.5));
        assert_eq!(server.player.y_rot_degrees(), -179.0);
        assert_eq!(server.player.x_rot_degrees(), 179.0);
        assert!(server.player.on_ground());
    }

    #[test]
    fn simulation_tick_records_java_shaped_movement_packet_boundary() {
        let mut server = IntegratedServer::new(0);

        server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(1.0, 64.0, 1.0),
                on_ground: true,
            }))
            .expect("move player");
        server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Rot {
                y_rot_degrees: 90.0,
                x_rot_degrees: 10.0,
                on_ground: false,
            }))
            .expect("rotate player");

        assert_eq!(server.player.position(), Vec3d::new(1.0, 64.0, 1.0));
        assert_eq!(server.player.received_move_packet_count(), 2);
        assert_eq!(server.player.known_move_packet_count(), 0);

        server
            .try_simulation_tick_report()
            .expect("simulation tick");

        assert_eq!(server.player.received_move_packet_count(), 2);
        assert_eq!(server.player.known_move_packet_count(), 2);
        assert_eq!(
            server.player.first_good_position(),
            Vec3d::new(1.0, 64.0, 1.0)
        );
        assert_eq!(
            server.player.last_good_position(),
            Vec3d::new(1.0, 64.0, 1.0)
        );
    }

    #[test]
    fn pending_player_position_update_blocks_moves_until_ack_and_resends() {
        let mut server = IntegratedServer::new(0);

        let first = server.player.initial_position_update(
            Vec3d::new(0.0, 64.0, 0.0),
            45.0,
            10.0,
            server.simulation_tick,
        );
        assert_eq!(first.teleport_id, 1);

        let updates = server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(32.0, 64.0, 0.0),
                on_ground: true,
            }))
            .expect("move while awaiting teleport");
        assert!(updates.is_empty());
        assert_eq!(server.player.position(), Vec3d::new(0.0, 64.0, 0.0));
        assert_eq!(
            server
                .player
                .awaiting_teleport()
                .map(|awaiting| awaiting.id),
            Some(first.teleport_id)
        );

        for _ in 0..20 {
            server
                .try_simulation_tick_report()
                .expect("pending teleport tick");
        }
        let updates = server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(32.0, 64.0, 0.0),
                on_ground: true,
            }))
            .expect("move while awaiting teleport at threshold");
        assert!(updates.is_empty());

        server
            .try_simulation_tick_report()
            .expect("pending teleport resend tick");
        let updates = server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(32.0, 64.0, 0.0),
                on_ground: true,
            }))
            .expect("move while awaiting stale teleport");
        assert_eq!(updates.len(), 1);
        let resend = match &updates[0] {
            ServerUpdate::PlayerPosition(update) => *update,
            _ => panic!("stale pending teleport should resend a player position update"),
        };
        assert_eq!(resend.position, Vec3d::new(0.0, 64.0, 0.0));
        assert_eq!(resend.y_rot_degrees, 45.0);
        assert_eq!(resend.x_rot_degrees, 10.0);
        assert_eq!(resend.relative, PlayerPositionRelativeFlags::ABSOLUTE);
        assert_eq!(resend.teleport_id, 2);
        assert_eq!(
            server
                .player
                .awaiting_teleport()
                .map(|awaiting| awaiting.id),
            Some(2)
        );

        server
            .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: first.teleport_id,
            }))
            .expect("accept stale teleport");
        assert_eq!(
            server
                .player
                .awaiting_teleport()
                .map(|awaiting| awaiting.id),
            Some(2)
        );

        server
            .try_handle_command(ClientCommand::AcceptTeleport(AcceptTeleportCommand {
                id: resend.teleport_id,
            }))
            .expect("accept resent teleport");
        assert_eq!(server.player.awaiting_teleport(), None);

        let updates = server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand::Pos {
                position: Vec3d::new(32.0, 64.0, 0.0),
                on_ground: true,
            }))
            .expect("move after ack");
        assert!(updates.is_empty());
        assert_eq!(server.player.position(), Vec3d::new(32.0, 64.0, 0.0));
    }

    #[test]
    fn set_carried_item_updates_server_selected_hotbar_slot_without_world_updates() {
        let mut server = IntegratedServer::new(0);

        let updates = server
            .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
                slot: 4,
            }))
            .expect("set carried item");

        assert!(updates.is_empty());
        assert_eq!(server.inventory.selected_hotbar_slot(), 4);

        let updates = server
            .try_handle_command(ClientCommand::SetCarriedItem(SetCarriedItemCommand {
                slot: mclone_protocol::HOTBAR_SLOT_COUNT,
            }))
            .expect("invalid carried item");

        assert!(updates.is_empty());
        assert_eq!(server.inventory.selected_hotbar_slot(), 4);
    }

    #[test]
    fn debug_break_command_mutates_block_and_returns_section_delta() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        let pos = BlockPos::new(8, 80, 8);
        assert!(server.scheduler_mut().set_block_at_world(pos, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
                pos,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }))
            .expect("break command");

        assert_eq!(server.scheduler().block_at_world(pos), Some(0));
        assert!(updates.iter().any(|update| {
            match update {
                ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                    .iter()
                    .any(|update| update.block_state == AIR_BLOCK_STATE_ID),
                _ => false,
            }
        }));
    }

    #[test]
    fn debug_place_command_places_adjacent_to_hit_face() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        sync_carried_slot(&mut server, 1);
        let clicked = BlockPos::new(8, 80, 8);
        let target = clicked.relative(Direction::Up);
        assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 81.0, 8.5),
                Direction::Up,
                clicked,
                false,
            )))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(target), Some(DIRT));
        assert!(updates.iter().any(|update| {
            match update {
                ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                    .iter()
                    .any(|update| update.block_state == BlockStateId(DIRT as u32)),
                _ => false,
            }
        }));
    }

    #[test]
    fn debug_place_command_places_bricks_from_selected_hotbar_slot() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        sync_carried_slot(&mut server, 7);
        let clicked = BlockPos::new(8, 80, 8);
        let target = clicked.relative(Direction::Up);
        assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 81.0, 8.5),
                Direction::Up,
                clicked,
                false,
            )))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(target), Some(BRICKS));
        assert!(updates.iter().any(|update| {
            match update {
                ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                    .iter()
                    .any(|update| update.block_state == BlockStateId(BRICKS as u32)),
                _ => false,
            }
        }));
    }

    #[test]
    fn debug_place_command_replaces_clicked_replaceable_block() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        sync_carried_slot(&mut server, 1);
        let clicked = BlockPos::new(8, 80, 8);
        let adjacent = clicked.relative(Direction::Up);
        assert!(server.scheduler_mut().set_block_at_world(clicked, GRASS));
        assert!(server.scheduler_mut().set_block_at_world(adjacent, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 81.0, 8.5),
                Direction::Up,
                clicked,
                false,
            )))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(clicked), Some(DIRT));
        assert_eq!(server.scheduler().block_at_world(adjacent), Some(STONE));
        assert!(updates.iter().any(|update| {
            match update {
                ServerUpdate::SectionBlockUpdates { updates, .. } => updates
                    .iter()
                    .any(|update| update.block_state == BlockStateId(DIRT as u32)),
                _ => false,
            }
        }));
    }

    #[test]
    fn debug_place_command_does_not_overwrite_solid_relative_target() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        sync_carried_slot(&mut server, 1);
        let clicked = BlockPos::new(8, 80, 8);
        let target = clicked.relative(Direction::Up);
        assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
        assert!(server.scheduler_mut().set_block_at_world(target, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 81.0, 8.5),
                Direction::Up,
                clicked,
                false,
            )))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(clicked), Some(STONE));
        assert_eq!(server.scheduler().block_at_world(target), Some(STONE));
        assert!(updates.is_empty());
    }

    #[test]
    fn debug_place_command_rejects_empty_selected_hotbar_slot() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        sync_carried_slot(&mut server, 8);
        let clicked = BlockPos::new(8, 80, 8);
        assert!(server.scheduler_mut().set_block_at_world(clicked, GRASS));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 81.0, 8.5),
                Direction::Up,
                clicked,
                false,
            )))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(clicked), Some(GRASS));
        assert!(updates.is_empty());
    }

    #[test]
    fn debug_interaction_commands_reject_far_server_player_positions() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(100.0, 80.0, 100.0));
        sync_carried_slot(&mut server, 1);
        let clicked = BlockPos::new(8, 80, 8);
        let target = clicked.relative(Direction::Up);
        assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let break_updates = server
            .try_handle_command(ClientCommand::PlayerAction(PlayerActionCommand {
                pos: clicked,
                direction: Direction::Up,
                kind: PlayerActionKind::DebugInstantBreak,
            }))
            .expect("break command");
        let place_updates = server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 81.0, 8.5),
                Direction::Up,
                clicked,
                false,
            )))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(clicked), Some(STONE));
        assert_eq!(server.scheduler().block_at_world(target), Some(0));
        assert!(break_updates.is_empty());
        assert!(place_updates.is_empty());
    }

    #[test]
    fn debug_place_command_replaces_one_layer_snow_in_place() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        sync_carried_slot(&mut server, 1);
        let clicked = BlockPos::new(8, 80, 8);
        assert!(server.scheduler_mut().set_block_at_world(clicked, SNOW));
        server.scheduler_mut().drain_pending_block_delta_events();

        server
            .try_handle_command(use_held_item_on(BlockHitResult::new(
                Vec3d::new(8.5, 80.125, 8.5),
                Direction::North,
                clicked,
                false,
            )))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(clicked), Some(DIRT));
    }
}
