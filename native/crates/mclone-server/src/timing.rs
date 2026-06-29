//! Server tick timing diagnostics.
//!
//! Move-only home for the per-tick timing breakdown structs, the public tick
//! report structs that carry them, and the cfg-gated timing-sample helpers
//! (native `Instant` vs. wasm no-op, mirroring `levelgen/timing.rs`).

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use mclone_core::{ChunkPos, Vec3d};
use mclone_protocol::ServerUpdate;

use crate::{ChunkSchedulerEvent, PlayerChunkTrackingDiagnostics};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkSchedulerTickReport {
    pub ticket_tick: u64,
    pub block_ticking_chunks: Vec<ChunkPos>,
    pub entity_ticking_chunks: Vec<ChunkPos>,
    pub pending_unloads_processed: usize,
    pub events: Vec<ChunkSchedulerEvent>,
    pub timing: ChunkSchedulerTickTiming,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServerSimulationTickTiming {
    pub total_us: u128,
    pub scheduler_tick_us: u128,
    pub scheduler_report_us: u128,
    pub scheduler_purge_stale_tickets_us: u128,
    pub scheduler_reconcile_holders_us: u128,
    pub scheduler_publish_completed_us: u128,
    pub scheduler_pending_unload_us: u128,
    pub scheduler_apply_events_us: u128,
    pub block_tick_us: u128,
    pub fluid_tick_us: u128,
    pub fluid_event_apply_us: u128,
    pub fluid_due_scan_us: u128,
    pub fluid_remove_due_us: u128,
    pub fluid_tick_fluid_us: u128,
    pub fluid_set_block_us: u128,
    pub entity_tick_us: u128,
    pub physics_tick_us: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChunkSchedulerTickTiming {
    pub total_us: u128,
    pub purge_stale_tickets_us: u128,
    pub reconcile_holders_us: u128,
    pub publish_completed_us: u128,
    pub pending_unload_us: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServerTickTiming {
    pub total_us: u128,
    pub scheduler_report_us: u128,
    pub scheduler_purge_stale_tickets_us: u128,
    pub scheduler_reconcile_holders_us: u128,
    pub scheduler_publish_completed_us: u128,
    pub scheduler_pending_unload_us: u128,
    pub scheduler_apply_events_us: u128,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerTickReport {
    pub ticket_tick: u64,
    pub block_ticking_chunks: Vec<ChunkPos>,
    pub entity_ticking_chunks: Vec<ChunkPos>,
    pub pending_unloads_processed: usize,
    pub scheduler_event_count: usize,
    pub chunk_tracking: PlayerChunkTrackingDiagnostics,
    pub updates: Vec<ServerUpdate>,
    pub timing: ServerTickTiming,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerSimulationTickReport {
    pub simulation_tick: u64,
    pub chunk_tick: u64,
    pub block_tick_chunks: usize,
    pub fluid_ticks_executed: usize,
    pub fluid_due_ticks: usize,
    pub deferred_fluid_ticks: usize,
    pub fluid_mutated_blocks: usize,
    pub fluid_snapshot_events: usize,
    pub fluid_event_count: usize,
    pub scheduled_fluid_ticks: usize,
    pub entity_tick_chunks: usize,
    pub physics: ServerPhysicsTickDiagnostics,
    pub pending_unloads_processed: usize,
    pub scheduler_event_count: usize,
    pub chunk_tracking: PlayerChunkTrackingDiagnostics,
    pub updates: Vec<ServerUpdate>,
    pub timing: ServerSimulationTickTiming,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ServerPhysicsTickDiagnostics {
    pub enabled: bool,
    pub body_count: usize,
    pub collider_count: usize,
    pub active_body_count: usize,
    pub terrain_collider_count: usize,
    pub body_pose_update_count: usize,
    pub test_cube_spawned: bool,
    pub test_cube_position: Option<Vec3d>,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type TimingSample = Instant;

#[cfg(target_arch = "wasm32")]
pub(crate) type TimingSample = ();

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn simulation_timing_start() -> Option<Instant> {
    timing_start()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn simulation_timing_start() -> Option<()> {
    timing_start()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn simulation_timing_elapsed_us(start: Option<Instant>) -> u128 {
    timing_elapsed_us(start)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn simulation_timing_elapsed_us(_start: Option<()>) -> u128 {
    timing_elapsed_us(_start)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn timing_start() -> Option<TimingSample> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn timing_start() -> Option<TimingSample> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn timing_elapsed_us(start: Option<TimingSample>) -> u128 {
    start.map_or(0, |start| start.elapsed().as_micros())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn timing_elapsed_us(_start: Option<TimingSample>) -> u128 {
    0
}
