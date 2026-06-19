//! Integrated single-process server facade.
//!
//! Move-only home for `IntegratedServer`, the in-process wrapper that drives a
//! `ChunkScheduler` plus the fluid tick list and turns scheduler events into
//! protocol `ServerUpdate`s. Roughly mirrors Java's integrated-server glue over
//! `ServerChunkCache`; the heavy chunk-management logic lives in the scheduler.

use std::time::Duration;

use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkPos};
use mclone_protocol::{
    ChunkView, ClientCommand, MovePlayerCommand, PlayerActionCommand, PlayerActionKind,
    ServerUpdate, UseItemOnCommand, UseItemOnKind,
};
use mclone_worldgen::block::RawBlockId;

use crate::game_mode::ServerInteractionContext;
use crate::placement::DebugBlockItem;
use crate::player::ServerPlayerState;
use crate::timing::{simulation_timing_elapsed_us, simulation_timing_start};
use crate::{
    ChunkScheduler, ChunkSchedulerEvent, ChunkSnapshotStore, ChunkStoreResult, FluidKind,
    FluidTickList, NullChunkSnapshotStore, ServerSimulationTickReport, ServerSimulationTickTiming,
    ServerTickReport, ServerTickTiming, WorldBlockPos,
};

#[derive(Debug)]
pub struct IntegratedServer {
    seed: i64,
    scheduler: ChunkScheduler,
    pub(crate) liquid_ticks: FluidTickList,
    simulation_tick: u64,
    day_time: u64,
    day_time_frozen: bool,
    player: ServerPlayerState,
}

/// Vanilla overworld spawns at morning (`dayTime` 1000), not midnight.
const INITIAL_DAY_TIME: u64 = 1000;

impl IntegratedServer {
    pub fn new(seed: i64) -> Self {
        Self::with_chunk_store(seed, Box::<NullChunkSnapshotStore>::default())
    }

    pub fn with_chunk_store(seed: i64, store: Box<dyn ChunkSnapshotStore>) -> Self {
        Self {
            seed,
            scheduler: ChunkScheduler::with_store(seed, store),
            liquid_ticks: FluidTickList::new(),
            simulation_tick: 0,
            day_time: INITIAL_DAY_TIME,
            day_time_frozen: false,
            player: ServerPlayerState::default(),
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

    pub fn lighting_enabled(&self) -> bool {
        self.scheduler.lighting_enabled()
    }

    pub fn set_lighting_enabled(&mut self, enabled: bool) {
        self.scheduler.set_lighting_enabled(enabled);
    }

    pub fn handle_command(&mut self, command: ClientCommand) -> Vec<ServerUpdate> {
        self.try_handle_command(command)
            .expect("integrated server command failed")
    }

    pub fn try_handle_command(
        &mut self,
        command: ClientCommand,
    ) -> ChunkStoreResult<Vec<ServerUpdate>> {
        match command {
            ClientCommand::SetChunkView(view) => self.set_chunk_view(view),
            ClientCommand::MovePlayer(command) => Ok(self.handle_move_player(command)),
            ClientCommand::PlayerAction(command) => Ok(self.handle_player_action(command)),
            ClientCommand::UseItemOn(command) => Ok(self.handle_use_item_on(command)),
        }
    }

    pub fn poll(&mut self) -> Vec<ServerUpdate> {
        self.try_poll().expect("integrated server poll failed")
    }

    pub fn try_poll(&mut self) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let events = self.scheduler.poll()?;
        Ok(self.apply_scheduler_events(events))
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
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let report = self.scheduler.tick_report()?;
        let scheduler_report_us = simulation_timing_elapsed_us(scheduler_start);
        let scheduler_event_count = report.events.len();
        let scheduler_apply_start = simulation_timing_start();
        let updates = self.apply_scheduler_events(report.events);
        let scheduler_apply_events_us = simulation_timing_elapsed_us(scheduler_apply_start);
        let scheduler_timing = report.timing;
        Ok(ServerTickReport {
            ticket_tick: report.ticket_tick,
            block_ticking_chunks: report.block_ticking_chunks,
            entity_ticking_chunks: report.entity_ticking_chunks,
            pending_unloads_processed: report.pending_unloads_processed,
            scheduler_event_count,
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
        let total_start = simulation_timing_start();
        let scheduler_start = simulation_timing_start();
        let tick_report = self.try_tick_report()?;
        let scheduler_tick_us = simulation_timing_elapsed_us(scheduler_start);
        let tick_timing = tick_report.timing;

        let simulation_tick = self.simulation_tick.saturating_add(1);
        self.simulation_tick = simulation_tick;

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
        let entity_tick_us = simulation_timing_elapsed_us(entity_tick_start);

        let mut updates = tick_report.updates;
        updates.push(ServerUpdate::TimeUpdate {
            day_time: self.day_time,
        });
        let fluid_event_apply_start = simulation_timing_start();
        updates.extend(
            fluid_events
                .into_iter()
                .filter_map(server_update_from_scheduler_event),
        );
        let fluid_event_apply_us = simulation_timing_elapsed_us(fluid_event_apply_start);

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

    fn set_chunk_view(&mut self, view: ChunkView) -> ChunkStoreResult<Vec<ServerUpdate>> {
        let events = self.scheduler.apply_interest(view)?;
        Ok(self.apply_scheduler_events(events))
    }

    fn handle_move_player(&mut self, command: MovePlayerCommand) -> Vec<ServerUpdate> {
        self.player.apply_move_player(command);
        Vec::new()
    }

    fn handle_player_action(&mut self, command: PlayerActionCommand) -> Vec<ServerUpdate> {
        if command.kind == PlayerActionKind::DebugInstantBreak {
            let context = ServerInteractionContext::debug_creative(self.player.position());
            if context.may_break_block(command.pos) {
                self.set_block_debug(command.pos, AIR_BLOCK_STATE_ID);
            }
        }
        self.drain_pending_block_delta_updates()
    }

    fn handle_use_item_on(&mut self, command: UseItemOnCommand) -> Vec<ServerUpdate> {
        match command.action {
            UseItemOnKind::DebugPlaceBlock { block_state } => {
                if let Some(target) = self.debug_place_target(command, block_state) {
                    self.set_block_debug(target, block_state);
                }
            }
        }
        self.drain_pending_block_delta_updates()
    }

    fn debug_place_target(
        &self,
        command: UseItemOnCommand,
        block_state: BlockStateId,
    ) -> Option<BlockPos> {
        let block_id = raw_block_id_from_block_state(block_state)?;
        let context = ServerInteractionContext::debug_creative(self.player.position());
        if !context.may_use_item_on(command.hit) {
            return None;
        }
        let block_item = DebugBlockItem::new(block_id)?;
        let clicked_block = self.scheduler.block_at_world(command.hit.block_pos)?;
        let relative_pos = command.hit.block_pos.relative(command.hit.direction);
        let relative_block = self.scheduler.block_at_world(relative_pos);
        let placement = block_item.use_on(command.hit, clicked_block, relative_block)?;
        context.may_place_at(placement.pos).then_some(placement.pos)
    }

    fn set_block_debug(&mut self, pos: BlockPos, block_state: BlockStateId) -> bool {
        let Some(block_id) = raw_block_id_from_block_state(block_state) else {
            return false;
        };
        self.scheduler.set_block_at_world(pos, block_id)
    }

    fn drain_pending_block_delta_updates(&mut self) -> Vec<ServerUpdate> {
        self.scheduler
            .drain_pending_block_delta_events()
            .into_iter()
            .filter_map(server_update_from_scheduler_event)
            .collect()
    }

    fn apply_scheduler_events(&mut self, events: Vec<ChunkSchedulerEvent>) -> Vec<ServerUpdate> {
        let mut updates = Vec::new();
        for event in events {
            match event {
                ChunkSchedulerEvent::FluidTickScheduled { pos, fluid, delay } => {
                    self.liquid_ticks
                        .schedule_tick(pos, fluid, delay, self.simulation_tick);
                }
                event => {
                    if let Some(update) = server_update_from_scheduler_event(event) {
                        updates.push(update);
                    }
                }
            }
        }
        updates
    }
}

fn raw_block_id_from_block_state(block_state: BlockStateId) -> Option<RawBlockId> {
    RawBlockId::try_from(block_state.0).ok()
}

fn server_update_from_scheduler_event(event: ChunkSchedulerEvent) -> Option<ServerUpdate> {
    match event {
        ChunkSchedulerEvent::SnapshotReady(snapshot) => Some(ServerUpdate::ChunkSnapshot(snapshot)),
        ChunkSchedulerEvent::Unloaded { pos } => Some(ServerUpdate::ChunkUnload { pos }),
        ChunkSchedulerEvent::SectionBlockUpdates {
            pos,
            section_y,
            updates,
        } => Some(ServerUpdate::SectionBlockUpdates {
            pos,
            section_y,
            updates,
        }),
        ChunkSchedulerEvent::StatusChanged { .. }
        | ChunkSchedulerEvent::FluidTickScheduled { .. } => None,
    }
}

fn run_noop_simulation_phase(chunks: &[ChunkPos]) -> usize {
    chunks.iter().fold(0, |count, _pos| count + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_core::{BlockHitResult, BlockStateId, Direction, Vec3d};
    use mclone_protocol::ServerUpdate;
    use mclone_worldgen::block::{DIRT, GRASS, SNOW, STONE, generated_block_state_id};

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

    fn load_center_chunk(server: &mut IntegratedServer) {
        server.set_lighting_enabled(false);
        server
            .try_handle_command(ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
                render_distance: 0,
                chunk_tracking_radius: 0,
            }))
            .expect("set chunk view");
        for _ in 0..60_000 {
            server.try_poll().expect("poll");
            if server.pending_job_count() == 0 {
                return;
            }
            if server.pending_publication_count() == 0 {
                server.wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
        panic!("timed out loading center chunk");
    }

    fn sync_player(server: &mut IntegratedServer, position: Vec3d) {
        let updates = server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand {
                position,
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            }))
            .expect("move player");
        assert!(updates.is_empty());
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
    fn move_player_command_updates_server_player_state_without_world_updates() {
        let mut server = IntegratedServer::new(0);

        let updates = server
            .try_handle_command(ClientCommand::MovePlayer(MovePlayerCommand {
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
        let clicked = BlockPos::new(8, 80, 8);
        let target = clicked.relative(Direction::Up);
        assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(ClientCommand::UseItemOn(UseItemOnCommand {
                hit: BlockHitResult::new(Vec3d::new(8.5, 81.0, 8.5), Direction::Up, clicked, false),
                action: UseItemOnKind::DebugPlaceBlock {
                    block_state: generated_block_state_id(DIRT),
                },
            }))
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
    fn debug_place_command_replaces_clicked_replaceable_block() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        let clicked = BlockPos::new(8, 80, 8);
        let adjacent = clicked.relative(Direction::Up);
        assert!(server.scheduler_mut().set_block_at_world(clicked, GRASS));
        assert!(server.scheduler_mut().set_block_at_world(adjacent, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(ClientCommand::UseItemOn(UseItemOnCommand {
                hit: BlockHitResult::new(Vec3d::new(8.5, 81.0, 8.5), Direction::Up, clicked, false),
                action: UseItemOnKind::DebugPlaceBlock {
                    block_state: generated_block_state_id(DIRT),
                },
            }))
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
        let clicked = BlockPos::new(8, 80, 8);
        let target = clicked.relative(Direction::Up);
        assert!(server.scheduler_mut().set_block_at_world(clicked, STONE));
        assert!(server.scheduler_mut().set_block_at_world(target, STONE));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(ClientCommand::UseItemOn(UseItemOnCommand {
                hit: BlockHitResult::new(Vec3d::new(8.5, 81.0, 8.5), Direction::Up, clicked, false),
                action: UseItemOnKind::DebugPlaceBlock {
                    block_state: generated_block_state_id(DIRT),
                },
            }))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(clicked), Some(STONE));
        assert_eq!(server.scheduler().block_at_world(target), Some(STONE));
        assert!(updates.is_empty());
    }

    #[test]
    fn debug_place_command_rejects_air_block_items() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(8.5, 80.0, 8.5));
        let clicked = BlockPos::new(8, 80, 8);
        assert!(server.scheduler_mut().set_block_at_world(clicked, GRASS));
        server.scheduler_mut().drain_pending_block_delta_events();

        let updates = server
            .try_handle_command(ClientCommand::UseItemOn(UseItemOnCommand {
                hit: BlockHitResult::new(Vec3d::new(8.5, 81.0, 8.5), Direction::Up, clicked, false),
                action: UseItemOnKind::DebugPlaceBlock {
                    block_state: AIR_BLOCK_STATE_ID,
                },
            }))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(clicked), Some(GRASS));
        assert!(updates.is_empty());
    }

    #[test]
    fn debug_interaction_commands_reject_far_server_player_positions() {
        let mut server = IntegratedServer::new(0);
        load_center_chunk(&mut server);
        sync_player(&mut server, Vec3d::new(100.0, 80.0, 100.0));
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
            .try_handle_command(ClientCommand::UseItemOn(UseItemOnCommand {
                hit: BlockHitResult::new(Vec3d::new(8.5, 81.0, 8.5), Direction::Up, clicked, false),
                action: UseItemOnKind::DebugPlaceBlock {
                    block_state: generated_block_state_id(DIRT),
                },
            }))
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
        let clicked = BlockPos::new(8, 80, 8);
        assert!(server.scheduler_mut().set_block_at_world(clicked, SNOW));
        server.scheduler_mut().drain_pending_block_delta_events();

        server
            .try_handle_command(ClientCommand::UseItemOn(UseItemOnCommand {
                hit: BlockHitResult::new(
                    Vec3d::new(8.5, 80.125, 8.5),
                    Direction::North,
                    clicked,
                    false,
                ),
                action: UseItemOnKind::DebugPlaceBlock {
                    block_state: generated_block_state_id(DIRT),
                },
            }))
            .expect("place command");

        assert_eq!(server.scheduler().block_at_world(clicked), Some(DIRT));
    }
}
