//! Integrated single-process server facade.
//!
//! Move-only home for `IntegratedServer`, the in-process wrapper that drives a
//! `ChunkScheduler` plus the fluid tick list and turns scheduler events into
//! protocol `ServerUpdate`s. Roughly mirrors Java's integrated-server glue over
//! `ServerChunkCache`; the heavy chunk-management logic lives in the scheduler.

use std::time::Duration;

use mclone_core::ChunkPos;
use mclone_protocol::{ChunkView, ClientCommand, ServerUpdate};

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
}

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
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn simulation_tick(&self) -> u64 {
        self.simulation_tick
    }

    pub fn schedule_fluid_tick(&mut self, pos: WorldBlockPos, fluid: FluidKind, delay: i32) {
        self.liquid_ticks
            .schedule_tick(pos, fluid, delay, self.simulation_tick);
    }

    pub fn scheduled_fluid_tick_count(&self) -> usize {
        self.liquid_ticks.size()
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
