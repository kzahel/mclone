//! Integrated single-process server facade.
//!
//! Move-only home for `IntegratedServer`, the in-process wrapper that drives a
//! `ChunkScheduler` plus the fluid tick list and turns scheduler events into
//! protocol `ServerUpdate`s. Roughly mirrors Java's integrated-server glue over
//! `ServerChunkCache`; the heavy chunk-management logic lives in the scheduler.

use std::time::Duration;

use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, ChunkPos};
use mclone_protocol::{
    ChunkView, ClientCommand, InteractionHand, MovePlayerCommand, PlayerActionCommand,
    PlayerActionKind, ServerUpdate, SetCarriedItemCommand, UseItemOnCommand,
};
use mclone_worldgen::block::RawBlockId;

use crate::game_mode::ServerInteractionContext;
use crate::inventory::ServerInventory;
use crate::placement::DebugBlockItem;
use crate::player::{MovePlayerApplyResult, ServerPlayerState};
use crate::spawn::find_safe_surface_spawn;
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
    initial_spawn_center: Option<ChunkPos>,
    player: ServerPlayerState,
    inventory: ServerInventory,
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
            initial_spawn_center: None,
            player: ServerPlayerState::default(),
            inventory: ServerInventory::default(),
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
            ClientCommand::AcceptTeleport(command) => Ok(self.handle_accept_teleport(command.id)),
            ClientCommand::SetCarriedItem(command) => Ok(self.handle_set_carried_item(command)),
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
        self.player.mark_tick_boundary();

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
        if self.initial_spawn_center.is_none() && self.player.needs_initial_position_sync() {
            self.initial_spawn_center = Some(view.center);
        }
        let events = self.scheduler.apply_interest(view)?;
        Ok(self.apply_scheduler_events(events))
    }

    fn handle_move_player(&mut self, command: MovePlayerCommand) -> Vec<ServerUpdate> {
        match self.player.apply_move_player(command) {
            MovePlayerApplyResult::Accepted | MovePlayerApplyResult::RejectedInvalid => Vec::new(),
            MovePlayerApplyResult::AwaitingTeleport => self
                .player
                .resend_pending_correction_update(self.simulation_tick)
                .map(ServerUpdate::PlayerPosition)
                .into_iter()
                .collect(),
        }
    }

    fn handle_accept_teleport(&mut self, id: u32) -> Vec<ServerUpdate> {
        self.player.accept_teleport(id);
        Vec::new()
    }

    fn handle_set_carried_item(&mut self, command: SetCarriedItemCommand) -> Vec<ServerUpdate> {
        self.inventory.apply_set_carried_item(command);
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
        if let Some((target, block_state)) = self.held_item_place_target(command) {
            self.set_block_debug(target, block_state);
        }
        self.drain_pending_block_delta_updates()
    }

    fn held_item_place_target(
        &self,
        command: UseItemOnCommand,
    ) -> Option<(BlockPos, BlockStateId)> {
        if command.hand != InteractionHand::MainHand {
            return None;
        }
        let block_state = self.inventory.selected_block_state()?;
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
        context
            .may_place_at(placement.pos)
            .then_some((placement.pos, block_state))
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
        if let Some(update) = self.initial_spawn_update() {
            updates.push(ServerUpdate::PlayerPosition(update));
        }
        updates
    }

    fn initial_spawn_update(&mut self) -> Option<mclone_protocol::PlayerPositionUpdate> {
        if !self.player.needs_initial_position_sync() {
            return None;
        }
        let center = self.initial_spawn_center?;
        let position = find_safe_surface_spawn(center, |pos| self.scheduler.block_at_world(pos))?;
        Some(
            self.player
                .initial_position_update(position, 0.0, 0.0, self.simulation_tick),
        )
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
    use mclone_protocol::{AcceptTeleportCommand, PlayerPositionRelativeFlags, ServerUpdate};
    use mclone_worldgen::block::{DIRT, GRASS, SNOW, STONE, has_fluid, material_blocks_motion};

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
        let updates = server
            .try_handle_command(ClientCommand::SetChunkView(ChunkView {
                center: ChunkPos::new(0, 0),
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

        let mut spawn = None;
        for _ in 0..60_000 {
            let updates = server.try_poll().expect("poll");
            spawn = spawn.or_else(|| {
                updates.iter().find_map(|update| match update {
                    ServerUpdate::PlayerPosition(update) => Some(*update),
                    _ => None,
                })
            });
            if spawn.is_some() {
                break;
            }
            if server.pending_job_count() > 0 && server.pending_publication_count() == 0 {
                server.wait_for_worldgen_completion(Duration::from_secs(1));
            }
        }
        let spawn = spawn.expect("initial spawn position update");

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
