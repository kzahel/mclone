//! Server tick timing diagnostics.
//!
//! Move-only home for the per-tick timing breakdown structs, the public tick
//! report structs that carry them, and the cfg-gated timing-sample helpers
//! (native `Instant` vs. wasm no-op, mirroring `levelgen/timing.rs`).

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use mclone_core::{ChunkPos, Vec3d};
use mclone_frame_budget::BudgetDecisionPanelReport;
use mclone_protocol::ServerUpdate;

use crate::{ChunkSchedulerEvent, PlayerChunkTrackingDiagnostics};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkSchedulerPublicationDiagnostics {
    pub budget_decision_panel: BudgetDecisionPanelReport,
    pub adaptive_budget_enabled: bool,
    pub feature_publish_budget_min_units: usize,
    pub feature_publish_budget_max_units: usize,
    pub feature_publish_budget_elapsed_us: u128,
    pub feature_publish_spent_units: usize,
    pub feature_publish_spent_us: u128,
    pub feature_publish_estimated_unit_us: Option<u128>,
    pub light_publish_budget_min_units: usize,
    pub light_publish_budget_max_units: usize,
    pub light_publish_budget_elapsed_us: u128,
    pub light_publish_spent_units: usize,
    pub light_publish_spent_us: u128,
    pub light_publish_estimated_unit_us: Option<u128>,
    pub pending_worldgen_publication_chunk_limit: usize,
    pub completed_feature_jobs_drained: usize,
    pub feature_jobs_pipeline_completed: usize,
    pub feature_chunks_published: usize,
    pub feature_chunks_skipped: usize,
    pub feature_jobs_completed: usize,
    pub feature_snapshot_ready_events: usize,
    pub light_status_batches_enqueued: usize,
    pub completed_light_statuses_drained: usize,
    pub light_statuses_published: usize,
    pub light_statuses_skipped: usize,
    pub light_snapshot_ready_events: usize,
    pub pending_worldgen_publication_jobs: usize,
    pub pending_worldgen_publication_chunks: usize,
    pub pending_light_publications: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NaturalSpawningDiagnostics {
    pub live_attempts_enabled: bool,
    pub live_spawns_are_volatile: bool,
    pub ready_for_live_attempts: bool,
    pub blocker_count: usize,
    pub player_distance_spawnable_chunks: usize,
    pub eligible_entity_ticking_spawn_chunks: usize,
    pub entity_ticking_spawn_chunks_awaiting_entity_load: usize,
    pub creature_count: u32,
    pub creature_cap: u32,
    pub creature_cadence_ready: bool,
    pub creature_cap_has_room: bool,
    pub creature_should_attempt_if_enabled: bool,
    pub live_chunks_checked: usize,
    pub live_chunk_budget_exhausted: bool,
    pub live_attempts: usize,
    pub live_spawned: usize,
    pub live_spawn_budget_exhausted: bool,
    pub live_blocked_by_biome: usize,
    pub live_blocked_missing_biome_data: usize,
    pub live_blocked_missing_block_data: usize,
    pub live_blocked_player_distance: usize,
    pub live_blocked_world_predicate: usize,
    pub live_blocked_unsupported: usize,
    pub live_wetland_habitats_detected: usize,
    pub live_blocked_missing_wetland_data: usize,
    pub live_mallard_flocks_spawned: usize,
    pub live_flowering_habitats_detected: usize,
    pub live_blocked_missing_flowering_data: usize,
    pub live_bee_colonies_spawned: usize,
    pub dry_run_chunks_checked: usize,
    pub dry_run_chunk_budget_exhausted: bool,
    pub dry_run_positions_checked: usize,
    pub dry_run_biome_supported_positions: usize,
    pub dry_run_implemented_entries_checked: usize,
    pub dry_run_valid_candidates: usize,
    pub dry_run_blocked_by_biome: usize,
    pub dry_run_blocked_missing_biome_data: usize,
    pub dry_run_blocked_missing_block_data: usize,
    pub dry_run_blocked_missing_brightness: usize,
    pub dry_run_blocked_invalid_floor: usize,
    pub dry_run_blocked_space: usize,
    pub dry_run_blocked_collision: usize,
    pub dry_run_blocked_too_dark: usize,
    pub dry_run_blocked_unsupported: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChunkSchedulerTickReport {
    pub ticket_tick: u64,
    pub block_ticking_chunks: Vec<ChunkPos>,
    pub entity_ticking_chunks: Vec<ChunkPos>,
    pub pending_unloads_processed: usize,
    pub events: Vec<ChunkSchedulerEvent>,
    pub publication: ChunkSchedulerPublicationDiagnostics,
    pub timing: ChunkSchedulerTickTiming,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServerSimulationTickTiming {
    pub total_us: u128,
    pub scheduler_tick_us: u128,
    pub scheduler_report_us: u128,
    pub scheduler_purge_stale_tickets_us: u128,
    pub scheduler_reconcile_holders_us: u128,
    pub scheduler_active_levels_us: u128,
    pub scheduler_holder_updates_us: u128,
    pub scheduler_runtime_enqueue_us: u128,
    pub scheduler_active_levels_calls: usize,
    pub scheduler_active_levels_cache_hits: usize,
    pub scheduler_holder_update_count: usize,
    pub scheduler_runtime_target_count: usize,
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
    pub active_levels_us: u128,
    pub holder_updates_us: u128,
    pub runtime_enqueue_us: u128,
    pub active_levels_calls: usize,
    pub active_levels_cache_hits: usize,
    pub holder_update_count: usize,
    pub runtime_target_count: usize,
    pub publish_completed_us: u128,
    pub pending_unload_us: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServerTickTiming {
    pub total_us: u128,
    pub scheduler_report_us: u128,
    pub scheduler_purge_stale_tickets_us: u128,
    pub scheduler_reconcile_holders_us: u128,
    pub scheduler_active_levels_us: u128,
    pub scheduler_holder_updates_us: u128,
    pub scheduler_runtime_enqueue_us: u128,
    pub scheduler_active_levels_calls: usize,
    pub scheduler_active_levels_cache_hits: usize,
    pub scheduler_holder_update_count: usize,
    pub scheduler_runtime_target_count: usize,
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
    pub scheduler_publication: ChunkSchedulerPublicationDiagnostics,
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
    pub natural_spawning: NaturalSpawningDiagnostics,
    pub physics: ServerPhysicsTickDiagnostics,
    pub pending_unloads_processed: usize,
    pub scheduler_event_count: usize,
    pub scheduler_publication: ChunkSchedulerPublicationDiagnostics,
    pub chunk_tracking: PlayerChunkTrackingDiagnostics,
    pub updates: Vec<ServerUpdate>,
    pub timing: ServerSimulationTickTiming,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServerPhysicsStepTiming {
    pub total_us: u128,
    pub physics_tick_us: u128,
    pub physics_event_apply_us: u128,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerPhysicsStepReport {
    pub simulation_tick: u64,
    pub physics_steps: u32,
    pub physics: ServerPhysicsTickDiagnostics,
    pub chunk_tracking: PlayerChunkTrackingDiagnostics,
    pub updates: Vec<ServerUpdate>,
    pub timing: ServerPhysicsStepTiming,
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
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn simulation_timing_start() -> Option<()> {
    timing_start()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn simulation_timing_elapsed_us(start: Option<Instant>) -> u128 {
    start.map_or(0, |start| start.elapsed().as_micros())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn simulation_timing_elapsed_us(_start: Option<()>) -> u128 {
    timing_elapsed_us(_start)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "perf-diagnostics"))]
pub(crate) fn timing_start() -> Option<TimingSample> {
    Some(Instant::now())
}

#[cfg(any(target_arch = "wasm32", not(feature = "perf-diagnostics")))]
pub(crate) fn timing_start() -> Option<TimingSample> {
    None
}

#[cfg(all(not(target_arch = "wasm32"), feature = "perf-diagnostics"))]
pub(crate) fn timing_elapsed_us(start: Option<TimingSample>) -> u128 {
    start.map_or(0, |start| start.elapsed().as_micros())
}

#[cfg(any(target_arch = "wasm32", not(feature = "perf-diagnostics")))]
pub(crate) fn timing_elapsed_us(_start: Option<TimingSample>) -> u128 {
    0
}

#[cfg(all(test, not(target_arch = "wasm32"), not(feature = "perf-diagnostics")))]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_timing_compiles_out_without_perf_diagnostics() {
        assert!(timing_start().is_none());
        assert_eq!(timing_elapsed_us(timing_start()), 0);
        assert!(simulation_timing_start().is_some());
    }
}
