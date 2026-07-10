use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_pipeline_accounting::{
    FramePipelineAccountant, FramePipelinePeerThreadAccumulator, FramePipelinePeerThreadInput,
    FramePipelineQueueDepths, FramePipelineReportExtras, render_section_sync_stage_spans,
};
use mclone_app_runtime::frame_pipeline_presentation::frame_pipeline_report_json_field_lines;
use mclone_app_runtime::{RenderSectionSyncTiming, RuntimeUpdatePumpBudget};
use mclone_core::{CHUNK_WIDTH, ChunkPos};
use mclone_diagnostics::{
    BudgetDecisionPanelReport, FrameAccountingConfig, FrameAccumulator, FrameObservation,
    FramePipelineReport, FrameSummaryReport, PercentileMethod, QueuePanelReport, StageId,
    StageSpan,
};
use mclone_frame_budget::RenderCompileMeshFootprint;
use mclone_mesh::{TexturedChunkVertex, VisibilityGraphBuildStats, quad_face_count_from_indices};
use mclone_render::chunk::{
    ChunkCamera, textured_section_visibility_stats_with_options_and_ready_sections,
};
use mclone_render::headless::{HeadlessFrameLoopOptions, run_headless_frame_loop};
use mclone_server::{
    LightStatusMailboxMetrics, SqliteWorldStore, WorkerFrameMetrics, initial_spawn_center_for_seed,
};

use crate::camera::{
    SPECTATOR_BASE_SPEED, SPECTATOR_MAX_SPEED, SPECTATOR_MIN_SPEED, SpectatorCamera,
};
use crate::cli::{
    FrameBudgetProbeMode, FrameBudgetProbeOptions, LoadingSettlePerfOptions, MovementPerfOptions,
    SceneOptions, StartupStreamingPerfOptions, TimedemoOptions,
};
use crate::frame_pacing::elapsed_ms;
use crate::render_cache::load_asset_source;
use crate::render_compile_capacity::{
    RenderCompileCapacityHostKind, preflight_render_compile_capacity_report,
    render_compile_capacity_report,
};
use crate::scene_runtime::{
    WindowSceneAssets, WindowSceneRuntime, chunk_tracking_radius_for_render_distance,
    poll_window_runtime_until_idle, poll_window_runtime_until_idle_with_timeout, square_count,
};
use crate::{MAX_RENDER_DISTANCE, json_escape, print_benchmark_metadata};

const LOADING_SETTLE_IDLE_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Debug)]
pub(crate) struct MovementPerfReport {
    options: MovementPerfOptions,
    total_elapsed_ms: f64,
    steps: Vec<MovementPerfStepReport>,
}

#[derive(Clone, Copy, Debug)]
struct MovementPerfStepReport {
    index: usize,
    center: ChunkPos,
    elapsed_ms: f64,
    set_interest_ms: f64,
    poll_ms: f64,
    remesh_ms: f64,
    poll_count: usize,
    loaded_chunks: usize,
    client_visible_chunks: usize,
    active_ticket_chunks: usize,
    tracked_players: usize,
    player_visible_chunks: usize,
    aggregate_player_ticket_chunks: usize,
    player_outbound_queue_depth: usize,
    pending_render_compile_jobs: usize,
    max_pending_render_compile_jobs: usize,
    available_render_compile_slots: usize,
    pending_unload_chunks: usize,
    block_ticking_chunks: usize,
    entity_ticking_chunks: usize,
    simulation_tick: u64,
    simulation_scheduler_tick_ms: f64,
    simulation_block_tick_ms: f64,
    simulation_fluid_tick_ms: f64,
    simulation_entity_tick_ms: f64,
    fluid_ticks_executed: usize,
    deferred_fluid_ticks: usize,
    fluid_mutated_blocks: usize,
    scheduled_fluid_ticks: usize,
    rebuilt_sections: usize,
    removed_sections: usize,
    rebuilt_vertices: u32,
    rebuilt_faces: u32,
    rebuilt_indices: u32,
    visibility_graph_build_count: usize,
    visibility_graph_total_ms: f64,
    visibility_graph_average_ms: f64,
    visibility_graph_worst_ms: f64,
    loaded_sections: usize,
    visible_sections: usize,
    frustum_sections: usize,
    graph_cull_enabled: bool,
    graph_culled_sections: usize,
    loaded_faces: u32,
    visible_faces: u32,
    frustum_faces: u32,
    graph_culled_faces: u32,
    loaded_indices: u32,
    visible_indices: u32,
    frustum_indices: u32,
    graph_culled_indices: u32,
}

impl MovementPerfReport {
    pub(crate) fn validate(&self) -> Result<()> {
        let render_distance = u32::try_from(self.options.scene.render_distance)
            .context("render distance must be non-negative")?;
        let expected_tracking_radius =
            i32::try_from(chunk_tracking_radius_for_render_distance(render_distance))
                .context("chunk tracking radius exceeds i32")?;
        let expected_tracked_chunks = square_count(expected_tracking_radius)?;
        let max_reasonable_chunks = expected_tracked_chunks.saturating_mul(2);
        for step in &self.steps {
            if step.loaded_chunks < expected_tracked_chunks {
                bail!(
                    "movement step {} loaded_chunks={} below expected minimum {expected_tracked_chunks}",
                    step.index,
                    step.loaded_chunks
                );
            }
            if step.loaded_chunks > max_reasonable_chunks {
                bail!(
                    "movement step {} loaded_chunks={} exceeds generous cap {max_reasonable_chunks}",
                    step.index,
                    step.loaded_chunks
                );
            }
            if step.client_visible_chunks < expected_tracked_chunks {
                bail!(
                    "movement step {} client_visible_chunks={} below expected minimum {expected_tracked_chunks}",
                    step.index,
                    step.client_visible_chunks
                );
            }
            if step.client_visible_chunks > step.loaded_chunks {
                bail!(
                    "movement step {} client_visible_chunks={} exceeds loaded_chunks={}",
                    step.index,
                    step.client_visible_chunks,
                    step.loaded_chunks
                );
            }
            if step.loaded_sections == 0 {
                bail!("movement step {} produced no render sections", step.index);
            }
            if step.visible_sections == 0 {
                bail!(
                    "movement step {} frustum culled every render section",
                    step.index
                );
            }
            if step.visible_sections > step.loaded_sections {
                bail!(
                    "movement step {} visible_sections={} exceeds loaded_sections={}",
                    step.index,
                    step.visible_sections,
                    step.loaded_sections
                );
            }
            if step.visible_faces == 0 {
                bail!(
                    "movement step {} frustum culled every render face",
                    step.index
                );
            }
            if step.visible_faces > step.loaded_faces {
                bail!(
                    "movement step {} visible_faces={} exceeds loaded_faces={}",
                    step.index,
                    step.visible_faces,
                    step.loaded_faces
                );
            }
        }
        if self
            .steps
            .iter()
            .all(|step| step.visible_sections == step.loaded_sections)
        {
            bail!("movement perf smoke did not cull any loaded render section");
        }
        Ok(())
    }

    pub(crate) fn print_json(&self) {
        println!("{{");
        print_benchmark_metadata("native_runtime_movement", "  ", true);
        println!("  \"seed\": {},", self.options.scene.seed);
        println!(
            "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
            self.options.scene.chunk_x, self.options.scene.chunk_z
        );
        println!(
            "  \"render_distance\": {},",
            self.options.scene.render_distance
        );
        println!(
            "  \"render_compile_workers\": {},",
            self.options.scene.render_compile_worker_count
        );
        print_optional_usize_json(
            "  ",
            "render_compile_max_pending_jobs",
            self.options.scene.render_compile_max_pending_jobs,
            true,
        );
        println!(
            "  \"render_compile_capacity_mode\": \"{}\",",
            self.options.scene.render_compile_capacity_mode.as_str()
        );
        println!(
            "  \"adaptive_chunk_publication_budget\": {},",
            self.options.scene.adaptive_chunk_publication_budget
        );
        println!(
            "  \"light_status_batch_size\": {},",
            self.options.scene.light_status_batch_size
        );
        println!(
            "  \"section_occlusion_culling\": {},",
            self.options.render_options.section_occlusion_culling
        );
        println!(
            "  \"force_fullbright\": {},",
            self.options.render_options.force_fullbright
        );
        println!(
            "  \"render_color_profile\": \"{}\",",
            self.options.render_options.color_profile.as_str()
        );
        println!(
            "  \"path_radius_chunks\": {},",
            self.options.path_radius_chunks
        );
        println!("  \"steps\": {},", self.options.steps);
        println!("  \"width\": {},", self.options.width);
        println!("  \"height\": {},", self.options.height);
        println!("  \"total_elapsed_ms\": {:.3},", self.total_elapsed_ms);
        println!("  \"step_reports\": [");
        for (index, step) in self.steps.iter().enumerate() {
            let suffix = if index + 1 == self.steps.len() {
                ""
            } else {
                ","
            };
            println!("    {{");
            println!("      \"index\": {},", step.index);
            println!(
                "      \"center\": {{ \"x\": {}, \"z\": {} }},",
                step.center.x, step.center.z
            );
            println!("      \"elapsed_ms\": {:.3},", step.elapsed_ms);
            println!("      \"set_interest_ms\": {:.3},", step.set_interest_ms);
            println!("      \"poll_ms\": {:.3},", step.poll_ms);
            println!("      \"remesh_ms\": {:.3},", step.remesh_ms);
            println!("      \"poll_count\": {},", step.poll_count);
            println!("      \"loaded_chunks\": {},", step.loaded_chunks);
            println!(
                "      \"client_visible_chunks\": {},",
                step.client_visible_chunks
            );
            println!(
                "      \"active_ticket_chunks\": {},",
                step.active_ticket_chunks
            );
            println!("      \"tracked_players\": {},", step.tracked_players);
            println!(
                "      \"player_visible_chunks\": {},",
                step.player_visible_chunks
            );
            println!(
                "      \"aggregate_player_ticket_chunks\": {},",
                step.aggregate_player_ticket_chunks
            );
            println!(
                "      \"player_outbound_queue_depth\": {},",
                step.player_outbound_queue_depth
            );
            println!(
                "      \"pending_render_compile_jobs\": {},",
                step.pending_render_compile_jobs
            );
            println!(
                "      \"max_pending_render_compile_jobs\": {},",
                step.max_pending_render_compile_jobs
            );
            println!(
                "      \"available_render_compile_slots\": {},",
                step.available_render_compile_slots
            );
            println!(
                "      \"pending_unload_chunks\": {},",
                step.pending_unload_chunks
            );
            println!(
                "      \"block_ticking_chunks\": {},",
                step.block_ticking_chunks
            );
            println!(
                "      \"entity_ticking_chunks\": {},",
                step.entity_ticking_chunks
            );
            println!("      \"simulation_tick\": {},", step.simulation_tick);
            println!(
                "      \"simulation_scheduler_tick_ms\": {:.3},",
                step.simulation_scheduler_tick_ms
            );
            println!(
                "      \"simulation_block_tick_ms\": {:.3},",
                step.simulation_block_tick_ms
            );
            println!(
                "      \"simulation_fluid_tick_ms\": {:.3},",
                step.simulation_fluid_tick_ms
            );
            println!(
                "      \"simulation_entity_tick_ms\": {:.3},",
                step.simulation_entity_tick_ms
            );
            println!(
                "      \"fluid_ticks_executed\": {},",
                step.fluid_ticks_executed
            );
            println!(
                "      \"deferred_fluid_ticks\": {},",
                step.deferred_fluid_ticks
            );
            println!(
                "      \"fluid_mutated_blocks\": {},",
                step.fluid_mutated_blocks
            );
            println!(
                "      \"scheduled_fluid_ticks\": {},",
                step.scheduled_fluid_ticks
            );
            println!("      \"rebuilt_sections\": {},", step.rebuilt_sections);
            println!("      \"removed_sections\": {},", step.removed_sections);
            println!("      \"rebuilt_vertices\": {},", step.rebuilt_vertices);
            println!("      \"rebuilt_faces\": {},", step.rebuilt_faces);
            println!("      \"rebuilt_indices\": {},", step.rebuilt_indices);
            println!(
                "      \"visibility_graph_build_count\": {},",
                step.visibility_graph_build_count
            );
            println!(
                "      \"visibility_graph_total_ms\": {:.3},",
                step.visibility_graph_total_ms
            );
            println!(
                "      \"visibility_graph_average_ms\": {:.6},",
                step.visibility_graph_average_ms
            );
            println!(
                "      \"visibility_graph_worst_ms\": {:.6},",
                step.visibility_graph_worst_ms
            );
            println!("      \"loaded_sections\": {},", step.loaded_sections);
            println!("      \"visible_sections\": {},", step.visible_sections);
            println!("      \"frustum_sections\": {},", step.frustum_sections);
            println!("      \"graph_cull_enabled\": {},", step.graph_cull_enabled);
            println!(
                "      \"graph_culled_sections\": {},",
                step.graph_culled_sections
            );
            println!("      \"loaded_faces\": {},", step.loaded_faces);
            println!("      \"visible_faces\": {},", step.visible_faces);
            println!("      \"frustum_faces\": {},", step.frustum_faces);
            println!("      \"graph_culled_faces\": {},", step.graph_culled_faces);
            println!("      \"loaded_indices\": {},", step.loaded_indices);
            println!("      \"visible_indices\": {},", step.visible_indices);
            println!("      \"frustum_indices\": {},", step.frustum_indices);
            println!(
                "      \"graph_culled_indices\": {}",
                step.graph_culled_indices
            );
            println!("    }}{suffix}");
        }
        println!("  ]");
        println!("}}");
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TimedemoReport {
    options: TimedemoOptions,
    loaded_render_distance: i32,
    visibility_graph_stats: VisibilityGraphBuildStats,
    scene_build_ms: f64,
    section_count: usize,
    vertex_count: u32,
    face_count: u32,
    index_count: u32,
    render: mclone_render::headless::HeadlessTimedemoReport,
}

#[derive(Clone, Debug)]
pub(crate) struct FrameBudgetProbeReport {
    options: FrameBudgetProbeOptions,
    runtime_setup_ms: f64,
    initial_poll_count: usize,
    initial_poll_ms: f64,
    initial_remesh_ms: f64,
    initial_section_count: usize,
    initial_face_count: u32,
    initial_index_count: u32,
    headless: mclone_render::headless::HeadlessFrameLoopReport,
    frame_accounting: Option<FrameSummaryReport>,
    frames: Vec<FrameBudgetProbeFrameReport>,
}

#[derive(Clone, Debug)]
pub(crate) struct LoadingSettlePerfReport {
    options: LoadingSettlePerfOptions,
    asset_load_ms: f64,
    total_elapsed_ms: f64,
    samples: Vec<LoadingSettlePerfSampleReport>,
}

#[derive(Clone, Debug)]
pub(crate) struct StartupStreamingPerfReport {
    options: StartupStreamingPerfOptions,
    world_dir: Option<PathBuf>,
    prewarm: Option<StartupStreamingPrewarmReport>,
    asset_load_ms: f64,
    startup_playable_frame: usize,
    startup_playable_ms: f64,
    startup_cached_sections: usize,
    startup_target_ready_chunks: usize,
    startup_target_chunk_count: usize,
    startup_target_percent: u8,
    streaming_wall_ms: f64,
    headless: mclone_render::headless::HeadlessFrameLoopReport,
    frames: Vec<StartupStreamingFrameReport>,
    budget_decision_panel: BudgetDecisionPanelReport,
    first_full_view_ready_frame: Option<usize>,
    first_full_view_ready_ms: Option<f64>,
    first_initial_target_render_complete_frame: Option<usize>,
    first_initial_target_render_complete_ms: Option<f64>,
    first_render_quiescent_frame: Option<usize>,
    first_render_quiescent_ms: Option<f64>,
    first_target_render_quiescent_frame: Option<usize>,
    first_target_render_quiescent_ms: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct StartupStreamingPrewarmReport {
    runtime_create_ms: f64,
    runtime_poll_count: usize,
    runtime_poll_ms: f64,
    total_ms: f64,
    target_ready_chunks: usize,
    target_chunk_count: usize,
    target_percent: u8,
    loaded_chunks: usize,
    client_visible_chunks: usize,
    active_ticket_chunks: usize,
    pending_jobs: usize,
    pending_publications: usize,
    pending_render_chunks: usize,
    pending_render_compile_jobs: usize,
    player_position_updates: usize,
    sqlite_storage_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
struct LoadingSettlePerfSampleReport {
    render_distance: i32,
    chunk_tracking_radius: u32,
    spawn_center: ChunkPos,
    expected_target_chunks: usize,
    target_chunk_count: usize,
    target_ready_chunks: usize,
    loaded_chunks: usize,
    client_visible_chunks: usize,
    active_ticket_chunks: usize,
    runtime_create_ms: f64,
    runtime_poll_count: usize,
    runtime_poll_ms: f64,
    runtime_settle_ms: f64,
    render_mesh_settle_ms: f64,
    full_settle_ms: f64,
    runtime_chunks_per_second: f64,
    full_chunks_per_second: f64,
    player_position_updates: usize,
    cached_sections: usize,
    rebuilt_sections: usize,
    removed_sections: usize,
    rebuilt_vertices: u32,
    rebuilt_faces: u32,
    rebuilt_indices: u32,
    visibility_graph_build_count: usize,
    visibility_graph_total_ms: f64,
    pending_jobs: usize,
    pending_publications: usize,
    pending_render_chunks: usize,
    pending_render_compile_jobs: usize,
    simulation_tick: u64,
    simulation_seconds: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct StartupStreamingFrameReport {
    elapsed_ms: f64,
    poll_ms: f64,
    poll_server_reported_total_ms: f64,
    poll_scheduler_publish_completed_ms: f64,
    scheduler_adaptive_publication_budget_enabled: bool,
    scheduler_feature_publish_budget_max_units: usize,
    scheduler_feature_publish_budget_ms: f64,
    scheduler_feature_publish_spent_units: usize,
    scheduler_feature_publish_spent_ms: f64,
    scheduler_feature_publish_estimated_unit_ms: Option<f64>,
    scheduler_light_publish_budget_max_units: usize,
    scheduler_light_publish_budget_ms: f64,
    scheduler_light_publish_spent_units: usize,
    scheduler_light_publish_spent_ms: f64,
    scheduler_light_publish_estimated_unit_ms: Option<f64>,
    scheduler_pending_worldgen_publication_chunk_limit: usize,
    poll_apply_updates_ms: f64,
    poll_dirty_mark_ms: f64,
    poll_client_apply_updates_ms: f64,
    poll_producer_read_ms: f64,
    poll_producer_decode_ms: f64,
    poll_producer_response_sequence: Option<u64>,
    remesh_ms: f64,
    upload_ms: f64,
    render_ms: f64,
    target_ready_chunks: usize,
    target_chunk_count: usize,
    target_percent: u8,
    loaded_chunks: usize,
    simulation_tick: u64,
    cached_sections: usize,
    pending_jobs: usize,
    pending_publications: usize,
    pending_render_chunks: usize,
    ready_render_work_pending: bool,
    target_pending_render_chunks: usize,
    target_ready_render_work_pending: bool,
    pending_render_compile_jobs: usize,
    inflight_render_sections: usize,
    target_inflight_render_sections: usize,
    section_sync_timing: RenderSectionSyncTiming,
    submitted_compile_sections: usize,
    accepted_compile_results: usize,
    queued_completed_compile_results: usize,
    completed_compile_sections: usize,
    uploaded_sections: usize,
    uploaded_vertices: u32,
    uploaded_indices: u32,
    upload_removed_sections: usize,
    target_rebuilt_sections: usize,
    non_target_rebuilt_sections: usize,
    target_removed_sections: usize,
    non_target_removed_sections: usize,
    deadline_skipped_compile_requests: usize,
    update_pump_stalled: bool,
    server_update_queue_depth: usize,
    server_update_queue_bytes: usize,
    server_update_oldest_applied_age_ms: f64,
    scheduler_completed_feature_jobs_drained: usize,
    scheduler_feature_chunks_published: usize,
    scheduler_feature_chunks_skipped: usize,
    scheduler_feature_jobs_completed: usize,
    scheduler_feature_snapshot_ready_events: usize,
    scheduler_light_status_batches_enqueued: usize,
    scheduler_completed_light_statuses_drained: usize,
    scheduler_light_statuses_published: usize,
    scheduler_light_statuses_skipped: usize,
    scheduler_light_snapshot_ready_events: usize,
    scheduler_pending_worldgen_publication_jobs: usize,
    scheduler_pending_worldgen_publication_chunks: usize,
    scheduler_pending_light_publications: usize,
    scheduler_worldgen_mailbox_pending_jobs: usize,
    scheduler_light_mailbox_pending_statuses: usize,
    poll_fluid_due_ticks: usize,
    poll_fluid_executed_ticks: usize,
    poll_fluid_deferred_ticks: usize,
    poll_fluid_mutated_blocks: usize,
    poll_scheduled_fluid_ticks: usize,
    runner_frame_metrics: WorkerFrameMetrics,
    worldgen_job_frame_metrics: WorkerFrameMetrics,
    light_status_job_frame_metrics: WorkerFrameMetrics,
    light_status_mailbox_metrics: LightStatusMailboxMetrics,
}

#[derive(Clone, Copy, Debug)]
struct FrameBudgetProbeFrameReport {
    index: usize,
    center: ChunkPos,
    interest_center_changed: bool,
    interest_updates_changed: bool,
    runtime_changed: bool,
    set_interest_ms: f64,
    poll_ms: f64,
    poll_flush_commands_ms: f64,
    poll_server_tick_ms: f64,
    poll_server_reported_total_ms: f64,
    poll_scheduler_tick_ms: f64,
    poll_scheduler_report_ms: f64,
    poll_scheduler_purge_stale_tickets_ms: f64,
    poll_scheduler_reconcile_holders_ms: f64,
    poll_scheduler_publish_completed_ms: f64,
    poll_scheduler_adaptive_publication_budget_enabled: bool,
    poll_scheduler_feature_publish_budget_max_units: usize,
    poll_scheduler_feature_publish_budget_ms: f64,
    poll_scheduler_feature_publish_spent_units: usize,
    poll_scheduler_feature_publish_spent_ms: f64,
    poll_scheduler_feature_publish_estimated_unit_ms: Option<f64>,
    poll_scheduler_light_publish_budget_max_units: usize,
    poll_scheduler_light_publish_budget_ms: f64,
    poll_scheduler_light_publish_spent_units: usize,
    poll_scheduler_light_publish_spent_ms: f64,
    poll_scheduler_light_publish_estimated_unit_ms: Option<f64>,
    poll_scheduler_pending_worldgen_publication_chunk_limit: usize,
    poll_scheduler_pending_unload_ms: f64,
    poll_scheduler_apply_events_ms: f64,
    poll_scheduler_completed_feature_jobs_drained: usize,
    poll_scheduler_feature_chunks_published: usize,
    poll_scheduler_feature_chunks_skipped: usize,
    poll_scheduler_feature_jobs_completed: usize,
    poll_scheduler_feature_snapshot_ready_events: usize,
    poll_scheduler_light_status_batches_enqueued: usize,
    poll_scheduler_completed_light_statuses_drained: usize,
    poll_scheduler_light_statuses_published: usize,
    poll_scheduler_light_statuses_skipped: usize,
    poll_scheduler_light_snapshot_ready_events: usize,
    poll_scheduler_pending_worldgen_publication_jobs: usize,
    poll_scheduler_pending_worldgen_publication_chunks: usize,
    poll_scheduler_pending_light_publications: usize,
    poll_scheduler_worldgen_mailbox_pending_jobs: usize,
    poll_scheduler_light_mailbox_pending_statuses: usize,
    poll_block_tick_ms: f64,
    poll_fluid_tick_ms: f64,
    poll_fluid_event_apply_ms: f64,
    poll_fluid_due_scan_ms: f64,
    poll_fluid_remove_due_ms: f64,
    poll_fluid_tick_fluid_ms: f64,
    poll_fluid_set_block_ms: f64,
    poll_entity_tick_ms: f64,
    poll_apply_updates_ms: f64,
    poll_dirty_mark_ms: f64,
    poll_client_apply_updates_ms: f64,
    poll_producer_read_ms: f64,
    poll_producer_decode_ms: f64,
    poll_producer_response_sequence: Option<u64>,
    update_pump_stalled: bool,
    update_pump_stall_count: usize,
    server_update_queue_depth: usize,
    server_update_queue_bytes: usize,
    server_update_applied_bytes: usize,
    server_update_oldest_applied_age_ms: f64,
    poll_scheduler_events: usize,
    poll_updates: usize,
    poll_snapshot_updates: usize,
    poll_section_block_updates: usize,
    poll_unload_updates: usize,
    poll_pending_unloads_processed: usize,
    poll_fluid_due_ticks: usize,
    poll_fluid_executed_ticks: usize,
    poll_fluid_deferred_ticks: usize,
    poll_fluid_mutated_blocks: usize,
    poll_fluid_snapshot_events: usize,
    poll_fluid_event_count: usize,
    poll_scheduled_fluid_ticks: usize,
    remesh_ms: f64,
    upload_ms: f64,
    render_ms: f64,
    section_sync_timing: RenderSectionSyncTiming,
    rebuilt_sections: usize,
    removed_sections: usize,
    submitted_compile_sections: usize,
    deadline_skipped_compile_requests: usize,
    accepted_compile_results: usize,
    queued_completed_compile_results: usize,
    completed_compile_sections: usize,
    stale_compile_sections: usize,
    uploaded_sections: usize,
    upload_removed_sections: usize,
    uploaded_vertices: u32,
    uploaded_indices: u32,
    loaded_chunks: usize,
    pending_jobs: usize,
    pending_publications: usize,
    pending_render_chunks: usize,
    pending_render_compile_jobs: usize,
    max_pending_render_compile_jobs: usize,
    available_render_compile_slots: usize,
    inflight_render_sections: usize,
    drawn_sections: usize,
    drawn_indices: u32,
}

impl Default for FrameBudgetProbeFrameReport {
    fn default() -> Self {
        Self {
            index: 0,
            center: ChunkPos::new(0, 0),
            interest_center_changed: false,
            interest_updates_changed: false,
            runtime_changed: false,
            set_interest_ms: 0.0,
            poll_ms: 0.0,
            poll_flush_commands_ms: 0.0,
            poll_server_tick_ms: 0.0,
            poll_server_reported_total_ms: 0.0,
            poll_scheduler_tick_ms: 0.0,
            poll_scheduler_report_ms: 0.0,
            poll_scheduler_purge_stale_tickets_ms: 0.0,
            poll_scheduler_reconcile_holders_ms: 0.0,
            poll_scheduler_publish_completed_ms: 0.0,
            poll_scheduler_adaptive_publication_budget_enabled: false,
            poll_scheduler_feature_publish_budget_max_units: 0,
            poll_scheduler_feature_publish_budget_ms: 0.0,
            poll_scheduler_feature_publish_spent_units: 0,
            poll_scheduler_feature_publish_spent_ms: 0.0,
            poll_scheduler_feature_publish_estimated_unit_ms: None,
            poll_scheduler_light_publish_budget_max_units: 0,
            poll_scheduler_light_publish_budget_ms: 0.0,
            poll_scheduler_light_publish_spent_units: 0,
            poll_scheduler_light_publish_spent_ms: 0.0,
            poll_scheduler_light_publish_estimated_unit_ms: None,
            poll_scheduler_pending_worldgen_publication_chunk_limit: 0,
            poll_scheduler_pending_unload_ms: 0.0,
            poll_scheduler_apply_events_ms: 0.0,
            poll_scheduler_completed_feature_jobs_drained: 0,
            poll_scheduler_feature_chunks_published: 0,
            poll_scheduler_feature_chunks_skipped: 0,
            poll_scheduler_feature_jobs_completed: 0,
            poll_scheduler_feature_snapshot_ready_events: 0,
            poll_scheduler_light_status_batches_enqueued: 0,
            poll_scheduler_completed_light_statuses_drained: 0,
            poll_scheduler_light_statuses_published: 0,
            poll_scheduler_light_statuses_skipped: 0,
            poll_scheduler_light_snapshot_ready_events: 0,
            poll_scheduler_pending_worldgen_publication_jobs: 0,
            poll_scheduler_pending_worldgen_publication_chunks: 0,
            poll_scheduler_pending_light_publications: 0,
            poll_scheduler_worldgen_mailbox_pending_jobs: 0,
            poll_scheduler_light_mailbox_pending_statuses: 0,
            poll_block_tick_ms: 0.0,
            poll_fluid_tick_ms: 0.0,
            poll_fluid_event_apply_ms: 0.0,
            poll_fluid_due_scan_ms: 0.0,
            poll_fluid_remove_due_ms: 0.0,
            poll_fluid_tick_fluid_ms: 0.0,
            poll_fluid_set_block_ms: 0.0,
            poll_entity_tick_ms: 0.0,
            poll_apply_updates_ms: 0.0,
            poll_dirty_mark_ms: 0.0,
            poll_client_apply_updates_ms: 0.0,
            poll_producer_read_ms: 0.0,
            poll_producer_decode_ms: 0.0,
            poll_producer_response_sequence: None,
            update_pump_stalled: false,
            update_pump_stall_count: 0,
            server_update_queue_depth: 0,
            server_update_queue_bytes: 0,
            server_update_applied_bytes: 0,
            server_update_oldest_applied_age_ms: 0.0,
            poll_scheduler_events: 0,
            poll_updates: 0,
            poll_snapshot_updates: 0,
            poll_section_block_updates: 0,
            poll_unload_updates: 0,
            poll_pending_unloads_processed: 0,
            poll_fluid_due_ticks: 0,
            poll_fluid_executed_ticks: 0,
            poll_fluid_deferred_ticks: 0,
            poll_fluid_mutated_blocks: 0,
            poll_fluid_snapshot_events: 0,
            poll_fluid_event_count: 0,
            poll_scheduled_fluid_ticks: 0,
            remesh_ms: 0.0,
            upload_ms: 0.0,
            render_ms: 0.0,
            section_sync_timing: RenderSectionSyncTiming::default(),
            rebuilt_sections: 0,
            removed_sections: 0,
            submitted_compile_sections: 0,
            deadline_skipped_compile_requests: 0,
            accepted_compile_results: 0,
            queued_completed_compile_results: 0,
            completed_compile_sections: 0,
            stale_compile_sections: 0,
            uploaded_sections: 0,
            upload_removed_sections: 0,
            uploaded_vertices: 0,
            uploaded_indices: 0,
            loaded_chunks: 0,
            pending_jobs: 0,
            pending_publications: 0,
            pending_render_chunks: 0,
            pending_render_compile_jobs: 0,
            max_pending_render_compile_jobs: 0,
            available_render_compile_slots: 0,
            inflight_render_sections: 0,
            drawn_sections: 0,
            drawn_indices: 0,
        }
    }
}

struct FrameBudgetProbeState {
    driver: crate::offscreen_scene_host::OffscreenDriver,
    frame_accounting: Option<FrameAccumulator>,
    runtime_setup_ms: f64,
    initial_poll_count: usize,
    initial_poll_ms: f64,
    initial_remesh_ms: f64,
    initial_section_count: usize,
    initial_face_count: u32,
    initial_index_count: u32,
}

struct TimedemoState {
    driver: crate::offscreen_scene_host::OffscreenDriver,
    warmup: crate::offscreen_scene_host::OffscreenWarmupReport,
    drawn_sections: usize,
    max_drawn_sections: usize,
    frustum_sections: usize,
    max_frustum_sections: usize,
    graph_cull_frames: usize,
    graph_culled_sections: usize,
    max_graph_culled_sections: usize,
    drawn_indices: u64,
    max_drawn_indices: u32,
    frustum_indices: u64,
    max_frustum_indices: u32,
    graph_culled_indices: u64,
    max_graph_culled_indices: u32,
}

impl TimedemoReport {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.render.frame_count != self.options.frames {
            bail!(
                "timedemo rendered {} frames, expected {}",
                self.render.frame_count,
                self.options.frames
            );
        }
        if self.section_count == 0 || self.index_count == 0 {
            bail!("timedemo scene produced no render sections");
        }
        if self.render.max_drawn_section_count == 0 {
            bail!("timedemo drew no render sections");
        }
        Ok(())
    }

    pub(crate) fn print_json(&self) {
        println!("{{");
        print_benchmark_metadata("native_render_timedemo", "  ", true);
        println!("  \"seed\": {},", self.options.scene.seed);
        println!(
            "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
            self.options.scene.chunk_x, self.options.scene.chunk_z
        );
        println!(
            "  \"render_distance\": {},",
            self.options.scene.render_distance
        );
        println!(
            "  \"loaded_render_distance\": {},",
            self.loaded_render_distance
        );
        println!(
            "  \"section_occlusion_culling\": {},",
            self.options.render_options.section_occlusion_culling
        );
        println!(
            "  \"force_fullbright\": {},",
            self.options.render_options.force_fullbright
        );
        println!(
            "  \"render_color_profile\": \"{}\",",
            self.options.render_options.color_profile.as_str()
        );
        println!(
            "  \"path_radius_chunks\": {},",
            self.options.path_radius_chunks
        );
        println!("  \"frames\": {},", self.options.frames);
        println!("  \"width\": {},", self.options.width);
        println!("  \"height\": {},", self.options.height);
        println!("  \"scene_build_ms\": {:.3},", self.scene_build_ms);
        println!(
            "  \"visibility_graph_build_count\": {},",
            self.visibility_graph_stats.build_count
        );
        println!(
            "  \"visibility_graph_total_ms\": {:.3},",
            self.visibility_graph_stats.total_ms
        );
        println!(
            "  \"visibility_graph_average_ms\": {:.6},",
            self.visibility_graph_stats.average_ms()
        );
        println!(
            "  \"visibility_graph_worst_ms\": {:.6},",
            self.visibility_graph_stats.worst_ms
        );
        println!("  \"section_count\": {},", self.section_count);
        println!("  \"vertex_count\": {},", self.vertex_count);
        println!("  \"face_count\": {},", self.face_count);
        println!("  \"index_count\": {},", self.index_count);
        println!("  \"render\": {{");
        println!("    \"frame_count\": {},", self.render.frame_count);
        println!("    \"setup_ms\": {:.3},", self.render.setup_ms);
        println!("    \"total_frame_ms\": {:.3},", self.render.total_frame_ms);
        println!(
            "    \"average_frame_ms\": {:.3},",
            self.render.average_frame_ms
        );
        println!("    \"min_frame_ms\": {:.3},", self.render.min_frame_ms);
        println!("    \"max_frame_ms\": {:.3},", self.render.max_frame_ms);
        println!(
            "    \"loaded_section_count\": {},",
            self.render.loaded_section_count
        );
        println!(
            "    \"average_drawn_section_count\": {:.3},",
            self.render.average_drawn_section_count
        );
        println!(
            "    \"max_drawn_section_count\": {},",
            self.render.max_drawn_section_count
        );
        println!(
            "    \"average_frustum_section_count\": {:.3},",
            self.render.average_frustum_section_count
        );
        println!(
            "    \"max_frustum_section_count\": {},",
            self.render.max_frustum_section_count
        );
        println!(
            "    \"graph_cull_enabled_frame_count\": {},",
            self.render.graph_cull_enabled_frame_count
        );
        println!(
            "    \"average_graph_culled_section_count\": {:.3},",
            self.render.average_graph_culled_section_count
        );
        println!(
            "    \"max_graph_culled_section_count\": {},",
            self.render.max_graph_culled_section_count
        );
        println!(
            "    \"loaded_index_count\": {},",
            self.render.loaded_index_count
        );
        println!(
            "    \"average_drawn_index_count\": {:.3},",
            self.render.average_drawn_index_count
        );
        println!(
            "    \"max_drawn_index_count\": {},",
            self.render.max_drawn_index_count
        );
        println!(
            "    \"average_frustum_index_count\": {:.3},",
            self.render.average_frustum_index_count
        );
        println!(
            "    \"max_frustum_index_count\": {},",
            self.render.max_frustum_index_count
        );
        println!(
            "    \"average_graph_culled_index_count\": {:.3},",
            self.render.average_graph_culled_index_count
        );
        println!(
            "    \"max_graph_culled_index_count\": {}",
            self.render.max_graph_culled_index_count
        );
        println!("  }}");
        println!("}}");
    }
}

impl FrameBudgetProbeReport {
    fn target_frame_ms(&self) -> f64 {
        target_frame_ms(self.options.target_hz)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.headless.frame_count != self.options.frames {
            bail!(
                "frame-budget probe rendered {} frames, expected {}",
                self.headless.frame_count,
                self.options.frames
            );
        }
        if self.frames.len() != self.options.frames {
            bail!(
                "frame-budget probe recorded {} frame reports, expected {}",
                self.frames.len(),
                self.options.frames
            );
        }
        if self.initial_section_count == 0 || self.initial_index_count == 0 {
            bail!("frame-budget probe initial scene produced no render sections");
        }
        if self.options.frame_accounting_enabled {
            let Some(frame_accounting) = self.frame_accounting.as_ref() else {
                bail!("frame-budget probe did not record enabled frame accounting");
            };
            if frame_accounting.frames != self.options.frames as u64 {
                bail!(
                    "frame-budget probe accounting recorded {} frames, expected {}",
                    frame_accounting.frames,
                    self.options.frames
                );
            }
            if !frame_accounting.conservation_violations.is_empty() {
                bail!(
                    "frame-budget probe accounting conservation violations: {:?}",
                    frame_accounting.conservation_violations
                );
            }
        } else if self.frame_accounting.is_some() {
            bail!("frame-budget probe recorded frame accounting while disabled");
        }
        Ok(())
    }

    pub(crate) fn print_json(&self) {
        let target_frame_ms = self.target_frame_ms();
        let frame_accounting =
            headless_frame_accounting_report(&self.headless.frames, target_frame_ms);
        let over_budget = frame_accounting.over_budget.single_period_frames();
        let over_double = frame_accounting.over_budget.double_period_frames();
        let over_quad = frame_accounting.over_budget.quad_period_frames();
        let p95 = frame_accounting.frame_wall.p95_ms;
        let p99 = frame_accounting.frame_wall.p99_ms;
        println!("{{");
        print_benchmark_metadata(self.options.mode.benchmark_name(), "  ", true);
        println!("  \"probe_mode\": \"{}\",", self.options.mode.as_str());
        println!("  \"seed\": {},", self.options.scene.seed);
        println!(
            "  \"origin\": {{ \"x\": {}, \"z\": {} }},",
            self.options.scene.chunk_x, self.options.scene.chunk_z
        );
        println!(
            "  \"render_distance\": {},",
            self.options.scene.render_distance
        );
        println!(
            "  \"render_compile_workers\": {},",
            self.options.scene.render_compile_worker_count
        );
        print_optional_usize_json(
            "  ",
            "render_compile_max_pending_jobs",
            self.options.scene.render_compile_max_pending_jobs,
            true,
        );
        println!(
            "  \"render_compile_capacity_mode\": \"{}\",",
            self.options.scene.render_compile_capacity_mode.as_str()
        );
        println!(
            "  \"light_status_batch_size\": {},",
            self.options.scene.light_status_batch_size
        );
        println!(
            "  \"section_occlusion_culling\": {},",
            self.options.render_options.section_occlusion_culling
        );
        println!(
            "  \"force_fullbright\": {},",
            self.options.render_options.force_fullbright
        );
        println!(
            "  \"render_color_profile\": \"{}\",",
            self.options.render_options.color_profile.as_str()
        );
        println!(
            "  \"path_radius_chunks\": {},",
            self.options.path_radius_chunks
        );
        println!("  \"frames\": {},", self.options.frames);
        println!("  \"width\": {},", self.options.width);
        println!("  \"height\": {},", self.options.height);
        println!("  \"target_hz\": {:.3},", self.options.target_hz);
        println!("  \"target_frame_ms\": {:.3},", target_frame_ms);
        println!(
            "  \"frame_accounting_enabled\": {},",
            self.options.frame_accounting_enabled
        );
        println!(
            "  \"frame_accounting_observed_frames\": {},",
            frame_accounting_observed_frames(self.frame_accounting.as_ref())
        );
        println!(
            "  \"frame_accounting_conservation_violations\": {},",
            frame_accounting_total_violations(self.frame_accounting.as_ref())
        );
        println!(
            "  \"movement_speed_blocks_per_sec\": {:.3},",
            self.options.movement_speed
        );
        println!("  \"over_budget_frames\": {},", over_budget);
        println!(
            "  \"{}\": {},",
            mclone_diagnostics::legacy_json_keys::OVER_DOUBLE_BUDGET_FRAMES,
            over_double
        );
        println!(
            "  \"{}\": {},",
            mclone_diagnostics::legacy_json_keys::OVER_QUAD_BUDGET_FRAMES,
            over_quad
        );
        println!(
            "  \"average_frame_ms\": {:.3},",
            self.headless.average_frame_ms
        );
        println!("  \"p95_frame_ms\": {:.3},", p95);
        println!("  \"p99_frame_ms\": {:.3},", p99);
        println!("  \"max_frame_ms\": {:.3},", self.headless.max_frame_ms);
        println!("  \"runtime_setup_ms\": {:.3},", self.runtime_setup_ms);
        println!("  \"initial_poll_count\": {},", self.initial_poll_count);
        println!("  \"initial_poll_ms\": {:.3},", self.initial_poll_ms);
        println!("  \"initial_remesh_ms\": {:.3},", self.initial_remesh_ms);
        println!(
            "  \"initial_section_count\": {},",
            self.initial_section_count
        );
        println!("  \"initial_face_count\": {},", self.initial_face_count);
        println!("  \"initial_index_count\": {},", self.initial_index_count);
        println!("  \"headless\": {{");
        println!("    \"setup_ms\": {:.3},", self.headless.setup_ms);
        println!(
            "    \"total_frame_ms\": {:.3},",
            self.headless.total_frame_ms
        );
        println!(
            "    \"average_frame_ms\": {:.3},",
            self.headless.average_frame_ms
        );
        println!("    \"min_frame_ms\": {:.3},", self.headless.min_frame_ms);
        println!("    \"max_frame_ms\": {:.3}", self.headless.max_frame_ms);
        println!("  }},");
        let pipeline_frame_accounting = self.frame_accounting.as_ref().unwrap_or(&frame_accounting);
        let frame_pipeline_report = FramePipelineReport::new(
            pipeline_frame_accounting.clone(),
            QueuePanelReport::new(Vec::new()),
        )
        .with_budget_decision_panel(BudgetDecisionPanelReport::empty());
        print_frame_accounting_json_field(&frame_pipeline_report, true);
        println!("  \"frame_reports\": [");
        for (index, frame) in self.frames.iter().enumerate() {
            let timing = self.headless.frames.get(index).copied().unwrap_or_default();
            let suffix = if index + 1 == self.frames.len() {
                ""
            } else {
                ","
            };
            println!("    {{");
            println!("      \"index\": {},", frame.index);
            println!(
                "      \"center\": {{ \"x\": {}, \"z\": {} }},",
                frame.center.x, frame.center.z
            );
            println!("      \"frame_ms\": {:.3},", timing.frame_ms);
            println!(
                "      \"budget_multiple\": {:.3},",
                timing.frame_ms / target_frame_ms
            );
            println!(
                "      \"over_budget\": {},",
                timing.frame_ms > target_frame_ms
            );
            println!(
                "      \"interest_center_changed\": {},",
                frame.interest_center_changed
            );
            println!(
                "      \"interest_updates_changed\": {},",
                frame.interest_updates_changed
            );
            println!("      \"runtime_changed\": {},", frame.runtime_changed);
            println!("      \"set_interest_ms\": {:.3},", frame.set_interest_ms);
            println!("      \"poll_ms\": {:.3},", frame.poll_ms);
            println!(
                "      \"poll_flush_commands_ms\": {:.3},",
                frame.poll_flush_commands_ms
            );
            println!(
                "      \"poll_server_tick_ms\": {:.3},",
                frame.poll_server_tick_ms
            );
            println!(
                "      \"poll_server_reported_total_ms\": {:.3},",
                frame.poll_server_reported_total_ms
            );
            println!(
                "      \"poll_scheduler_tick_ms\": {:.3},",
                frame.poll_scheduler_tick_ms
            );
            println!(
                "      \"poll_scheduler_report_ms\": {:.3},",
                frame.poll_scheduler_report_ms
            );
            println!(
                "      \"poll_scheduler_purge_stale_tickets_ms\": {:.3},",
                frame.poll_scheduler_purge_stale_tickets_ms
            );
            println!(
                "      \"poll_scheduler_reconcile_holders_ms\": {:.3},",
                frame.poll_scheduler_reconcile_holders_ms
            );
            println!(
                "      \"poll_scheduler_publish_completed_ms\": {:.3},",
                frame.poll_scheduler_publish_completed_ms
            );
            println!(
                "      \"poll_scheduler_adaptive_publication_budget_enabled\": {},",
                frame.poll_scheduler_adaptive_publication_budget_enabled
            );
            println!(
                "      \"poll_scheduler_feature_publish_budget_max_units\": {},",
                frame.poll_scheduler_feature_publish_budget_max_units
            );
            println!(
                "      \"poll_scheduler_feature_publish_budget_ms\": {:.3},",
                frame.poll_scheduler_feature_publish_budget_ms
            );
            println!(
                "      \"poll_scheduler_feature_publish_spent_units\": {},",
                frame.poll_scheduler_feature_publish_spent_units
            );
            println!(
                "      \"poll_scheduler_feature_publish_spent_ms\": {:.3},",
                frame.poll_scheduler_feature_publish_spent_ms
            );
            print_optional_f64_json(
                "      ",
                "poll_scheduler_feature_publish_estimated_unit_ms",
                frame.poll_scheduler_feature_publish_estimated_unit_ms,
                true,
            );
            println!(
                "      \"poll_scheduler_light_publish_budget_max_units\": {},",
                frame.poll_scheduler_light_publish_budget_max_units
            );
            println!(
                "      \"poll_scheduler_light_publish_budget_ms\": {:.3},",
                frame.poll_scheduler_light_publish_budget_ms
            );
            println!(
                "      \"poll_scheduler_light_publish_spent_units\": {},",
                frame.poll_scheduler_light_publish_spent_units
            );
            println!(
                "      \"poll_scheduler_light_publish_spent_ms\": {:.3},",
                frame.poll_scheduler_light_publish_spent_ms
            );
            print_optional_f64_json(
                "      ",
                "poll_scheduler_light_publish_estimated_unit_ms",
                frame.poll_scheduler_light_publish_estimated_unit_ms,
                true,
            );
            println!(
                "      \"poll_scheduler_pending_worldgen_publication_chunk_limit\": {},",
                frame.poll_scheduler_pending_worldgen_publication_chunk_limit
            );
            println!(
                "      \"poll_scheduler_completed_feature_jobs_drained\": {},",
                frame.poll_scheduler_completed_feature_jobs_drained
            );
            println!(
                "      \"poll_scheduler_feature_chunks_published\": {},",
                frame.poll_scheduler_feature_chunks_published
            );
            println!(
                "      \"poll_scheduler_feature_chunks_skipped\": {},",
                frame.poll_scheduler_feature_chunks_skipped
            );
            println!(
                "      \"poll_scheduler_feature_jobs_completed\": {},",
                frame.poll_scheduler_feature_jobs_completed
            );
            println!(
                "      \"poll_scheduler_feature_snapshot_ready_events\": {},",
                frame.poll_scheduler_feature_snapshot_ready_events
            );
            println!(
                "      \"poll_scheduler_light_status_batches_enqueued\": {},",
                frame.poll_scheduler_light_status_batches_enqueued
            );
            println!(
                "      \"poll_scheduler_completed_light_statuses_drained\": {},",
                frame.poll_scheduler_completed_light_statuses_drained
            );
            println!(
                "      \"poll_scheduler_light_statuses_published\": {},",
                frame.poll_scheduler_light_statuses_published
            );
            println!(
                "      \"poll_scheduler_light_statuses_skipped\": {},",
                frame.poll_scheduler_light_statuses_skipped
            );
            println!(
                "      \"poll_scheduler_light_snapshot_ready_events\": {},",
                frame.poll_scheduler_light_snapshot_ready_events
            );
            println!(
                "      \"poll_scheduler_pending_worldgen_publication_jobs\": {},",
                frame.poll_scheduler_pending_worldgen_publication_jobs
            );
            println!(
                "      \"poll_scheduler_pending_worldgen_publication_chunks\": {},",
                frame.poll_scheduler_pending_worldgen_publication_chunks
            );
            println!(
                "      \"poll_scheduler_pending_light_publications\": {},",
                frame.poll_scheduler_pending_light_publications
            );
            println!(
                "      \"poll_scheduler_worldgen_mailbox_pending_jobs\": {},",
                frame.poll_scheduler_worldgen_mailbox_pending_jobs
            );
            println!(
                "      \"poll_scheduler_light_mailbox_pending_statuses\": {},",
                frame.poll_scheduler_light_mailbox_pending_statuses
            );
            println!(
                "      \"poll_scheduler_pending_unload_ms\": {:.3},",
                frame.poll_scheduler_pending_unload_ms
            );
            println!(
                "      \"poll_scheduler_apply_events_ms\": {:.3},",
                frame.poll_scheduler_apply_events_ms
            );
            println!(
                "      \"poll_block_tick_ms\": {:.3},",
                frame.poll_block_tick_ms
            );
            println!(
                "      \"poll_fluid_tick_ms\": {:.3},",
                frame.poll_fluid_tick_ms
            );
            println!(
                "      \"poll_fluid_event_apply_ms\": {:.3},",
                frame.poll_fluid_event_apply_ms
            );
            println!(
                "      \"poll_fluid_due_scan_ms\": {:.3},",
                frame.poll_fluid_due_scan_ms
            );
            println!(
                "      \"poll_fluid_remove_due_ms\": {:.3},",
                frame.poll_fluid_remove_due_ms
            );
            println!(
                "      \"poll_fluid_tick_fluid_ms\": {:.3},",
                frame.poll_fluid_tick_fluid_ms
            );
            println!(
                "      \"poll_fluid_set_block_ms\": {:.3},",
                frame.poll_fluid_set_block_ms
            );
            println!(
                "      \"poll_entity_tick_ms\": {:.3},",
                frame.poll_entity_tick_ms
            );
            println!(
                "      \"poll_apply_updates_ms\": {:.3},",
                frame.poll_apply_updates_ms
            );
            println!(
                "      \"poll_dirty_mark_ms\": {:.3},",
                frame.poll_dirty_mark_ms
            );
            println!(
                "      \"poll_client_apply_updates_ms\": {:.3},",
                frame.poll_client_apply_updates_ms
            );
            println!(
                "      \"poll_producer_read_ms\": {:.3},",
                frame.poll_producer_read_ms
            );
            println!(
                "      \"poll_producer_decode_ms\": {:.3},",
                frame.poll_producer_decode_ms
            );
            match frame.poll_producer_response_sequence {
                Some(sequence) => {
                    println!("      \"poll_producer_response_sequence\": {sequence},")
                }
                None => println!("      \"poll_producer_response_sequence\": null,"),
            }
            println!(
                "      \"update_pump_stalled\": {},",
                frame.update_pump_stalled
            );
            println!(
                "      \"update_pump_stall_count\": {},",
                frame.update_pump_stall_count
            );
            println!(
                "      \"server_update_queue_depth\": {},",
                frame.server_update_queue_depth
            );
            println!(
                "      \"server_update_queue_bytes\": {},",
                frame.server_update_queue_bytes
            );
            println!(
                "      \"server_update_applied_bytes\": {},",
                frame.server_update_applied_bytes
            );
            println!(
                "      \"server_update_oldest_applied_age_ms\": {:.3},",
                frame.server_update_oldest_applied_age_ms
            );
            println!(
                "      \"poll_scheduler_events\": {},",
                frame.poll_scheduler_events
            );
            println!("      \"poll_updates\": {},", frame.poll_updates);
            println!(
                "      \"poll_snapshot_updates\": {},",
                frame.poll_snapshot_updates
            );
            println!(
                "      \"poll_section_block_updates\": {},",
                frame.poll_section_block_updates
            );
            println!(
                "      \"poll_unload_updates\": {},",
                frame.poll_unload_updates
            );
            println!(
                "      \"poll_pending_unloads_processed\": {},",
                frame.poll_pending_unloads_processed
            );
            println!(
                "      \"poll_fluid_due_ticks\": {},",
                frame.poll_fluid_due_ticks
            );
            println!(
                "      \"poll_fluid_executed_ticks\": {},",
                frame.poll_fluid_executed_ticks
            );
            println!(
                "      \"poll_fluid_deferred_ticks\": {},",
                frame.poll_fluid_deferred_ticks
            );
            println!(
                "      \"poll_fluid_mutated_blocks\": {},",
                frame.poll_fluid_mutated_blocks
            );
            println!(
                "      \"poll_fluid_snapshot_events\": {},",
                frame.poll_fluid_snapshot_events
            );
            println!(
                "      \"poll_fluid_event_count\": {},",
                frame.poll_fluid_event_count
            );
            println!(
                "      \"poll_scheduled_fluid_ticks\": {},",
                frame.poll_scheduled_fluid_ticks
            );
            println!("      \"remesh_ms\": {:.3},", frame.remesh_ms);
            println!("      \"upload_ms\": {:.3},", frame.upload_ms);
            println!("      \"render_ms\": {:.3},", frame.render_ms);
            println!("      \"encode_ms\": {:.3},", timing.encode_ms);
            println!("      \"submit_ms\": {:.3},", timing.submit_ms);
            println!("      \"device_poll_ms\": {:.3},", timing.device_poll_ms);
            println!("      \"rebuilt_sections\": {},", frame.rebuilt_sections);
            println!("      \"removed_sections\": {},", frame.removed_sections);
            println!(
                "      \"submitted_compile_sections\": {},",
                frame.submitted_compile_sections
            );
            println!(
                "      \"deadline_skipped_compile_requests\": {},",
                frame.deadline_skipped_compile_requests
            );
            println!(
                "      \"completed_compile_sections\": {},",
                frame.completed_compile_sections
            );
            println!(
                "      \"stale_compile_sections\": {},",
                frame.stale_compile_sections
            );
            println!("      \"uploaded_sections\": {},", frame.uploaded_sections);
            println!(
                "      \"upload_removed_sections\": {},",
                frame.upload_removed_sections
            );
            println!("      \"uploaded_vertices\": {},", frame.uploaded_vertices);
            println!("      \"uploaded_indices\": {},", frame.uploaded_indices);
            println!("      \"loaded_chunks\": {},", frame.loaded_chunks);
            println!("      \"pending_jobs\": {},", frame.pending_jobs);
            println!(
                "      \"pending_publications\": {},",
                frame.pending_publications
            );
            println!(
                "      \"pending_render_chunks\": {},",
                frame.pending_render_chunks
            );
            println!(
                "      \"pending_render_compile_jobs\": {},",
                frame.pending_render_compile_jobs
            );
            println!(
                "      \"max_pending_render_compile_jobs\": {},",
                frame.max_pending_render_compile_jobs
            );
            println!(
                "      \"available_render_compile_slots\": {},",
                frame.available_render_compile_slots
            );
            println!(
                "      \"inflight_render_sections\": {},",
                frame.inflight_render_sections
            );
            println!("      \"drawn_sections\": {},", frame.drawn_sections);
            println!("      \"drawn_indices\": {}", frame.drawn_indices);
            println!("    }}{suffix}");
        }
        println!("  ]");
        println!("}}");
    }
}

impl StartupStreamingPerfReport {
    fn target_frame_ms(&self) -> f64 {
        target_frame_ms(self.options.target_hz)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.headless.frame_count != self.options.frames {
            bail!(
                "startup streaming perf rendered {} frames, expected {}",
                self.headless.frame_count,
                self.options.frames
            );
        }
        if self.frames.len() != self.options.frames {
            bail!(
                "startup streaming perf recorded {} frame reports, expected {}",
                self.frames.len(),
                self.options.frames
            );
        }
        if self.startup_cached_sections == 0 {
            bail!("startup streaming perf entered playable with no cached render sections");
        }
        if self.options.persisted_world && self.prewarm.is_none() {
            bail!("persisted startup streaming perf did not record a prewarm report");
        }
        if self.options.persisted_world && self.world_dir.is_none() {
            bail!("persisted startup streaming perf did not record a world directory");
        }
        Ok(())
    }

    pub(crate) fn print_json(&self) {
        let target_frame_ms = self.target_frame_ms();
        let frame_pipeline_report = startup_streaming_frame_pipeline_report(
            &self.frames,
            &self.headless.frames,
            target_frame_ms,
            self.streaming_wall_ms,
            self.budget_decision_panel.clone(),
        );
        let frame_accounting = &frame_pipeline_report.frame_summary;
        let over_budget = frame_accounting.over_budget.single_period_frames();
        let over_double = frame_accounting.over_budget.double_period_frames();
        let over_quad = frame_accounting.over_budget.quad_period_frames();
        let p95 = frame_accounting.frame_wall.p95_ms;
        let p99 = frame_accounting.frame_wall.p99_ms;
        let total_poll_ms = self.frames.iter().map(|frame| frame.poll_ms).sum::<f64>();
        let total_remesh_ms = self.frames.iter().map(|frame| frame.remesh_ms).sum::<f64>();
        let total_upload_ms = self.frames.iter().map(|frame| frame.upload_ms).sum::<f64>();
        let total_render_ms = self.frames.iter().map(|frame| frame.render_ms).sum::<f64>();
        let total_poll_scheduler_publish_completed_ms = self
            .frames
            .iter()
            .map(|frame| frame.poll_scheduler_publish_completed_ms)
            .sum::<f64>();
        let total_poll_apply_updates_ms = self
            .frames
            .iter()
            .map(|frame| frame.poll_apply_updates_ms)
            .sum::<f64>();
        let total_poll_dirty_mark_ms = self
            .frames
            .iter()
            .map(|frame| frame.poll_dirty_mark_ms)
            .sum::<f64>();
        let total_poll_client_apply_updates_ms = self
            .frames
            .iter()
            .map(|frame| frame.poll_client_apply_updates_ms)
            .sum::<f64>();
        let total_poll_producer_read_ms = self
            .frames
            .iter()
            .map(|frame| frame.poll_producer_read_ms)
            .sum::<f64>();
        let total_poll_producer_decode_ms = self
            .frames
            .iter()
            .map(|frame| frame.poll_producer_decode_ms)
            .sum::<f64>();
        let total_scheduler_feature_chunks_published = self
            .frames
            .iter()
            .map(|frame| frame.scheduler_feature_chunks_published)
            .sum::<usize>();
        let total_scheduler_light_statuses_published = self
            .frames
            .iter()
            .map(|frame| frame.scheduler_light_statuses_published)
            .sum::<usize>();
        let total_scheduler_snapshot_ready_events = self
            .frames
            .iter()
            .map(|frame| {
                frame.scheduler_feature_snapshot_ready_events
                    + frame.scheduler_light_snapshot_ready_events
            })
            .sum::<usize>();
        let max_poll_scheduler_publish_completed_ms = self
            .frames
            .iter()
            .map(|frame| frame.poll_scheduler_publish_completed_ms)
            .fold(0.0, f64::max);
        let max_server_update_queue_depth = self
            .frames
            .iter()
            .map(|frame| frame.server_update_queue_depth)
            .max()
            .unwrap_or(0);
        let max_server_update_oldest_applied_age_ms = self
            .frames
            .iter()
            .map(|frame| frame.server_update_oldest_applied_age_ms)
            .fold(0.0, f64::max);
        let max_poll_producer_response_sequence = self
            .frames
            .iter()
            .filter_map(|frame| frame.poll_producer_response_sequence)
            .max();
        let max_scheduler_pending_worldgen_publication_chunks = self
            .frames
            .iter()
            .map(|frame| frame.scheduler_pending_worldgen_publication_chunks)
            .max()
            .unwrap_or(0);
        let max_scheduler_pending_light_publications = self
            .frames
            .iter()
            .map(|frame| frame.scheduler_pending_light_publications)
            .max()
            .unwrap_or(0);
        let max_scheduler_worldgen_mailbox_pending_jobs = self
            .frames
            .iter()
            .map(|frame| frame.scheduler_worldgen_mailbox_pending_jobs)
            .max()
            .unwrap_or(0);
        let max_scheduler_light_mailbox_pending_statuses = self
            .frames
            .iter()
            .map(|frame| frame.scheduler_light_mailbox_pending_statuses)
            .max()
            .unwrap_or(0);
        let total_submitted_compile_sections = self
            .frames
            .iter()
            .map(|frame| frame.submitted_compile_sections)
            .sum::<usize>();
        let total_completed_compile_sections = self
            .frames
            .iter()
            .map(|frame| frame.completed_compile_sections)
            .sum::<usize>();
        let total_uploaded_sections = self
            .frames
            .iter()
            .map(|frame| frame.uploaded_sections)
            .sum::<usize>();
        let total_target_rebuilt_sections = self
            .frames
            .iter()
            .map(|frame| frame.target_rebuilt_sections)
            .sum::<usize>();
        let total_non_target_rebuilt_sections = self
            .frames
            .iter()
            .map(|frame| frame.non_target_rebuilt_sections)
            .sum::<usize>();
        let total_target_removed_sections = self
            .frames
            .iter()
            .map(|frame| frame.target_removed_sections)
            .sum::<usize>();
        let total_non_target_removed_sections = self
            .frames
            .iter()
            .map(|frame| frame.non_target_removed_sections)
            .sum::<usize>();
        let post_full_view_frames = self
            .first_full_view_ready_frame
            .map(|frame| self.frames.iter().skip(frame))
            .into_iter()
            .flatten();
        let post_full_view_target_rebuilt_sections = post_full_view_frames
            .clone()
            .map(|frame| frame.target_rebuilt_sections)
            .sum::<usize>();
        let post_full_view_non_target_rebuilt_sections = post_full_view_frames
            .clone()
            .map(|frame| frame.non_target_rebuilt_sections)
            .sum::<usize>();
        let post_full_view_target_removed_sections = post_full_view_frames
            .clone()
            .map(|frame| frame.target_removed_sections)
            .sum::<usize>();
        let post_full_view_non_target_removed_sections = post_full_view_frames
            .map(|frame| frame.non_target_removed_sections)
            .sum::<usize>();
        let total_deadline_skipped_compile_requests = self
            .frames
            .iter()
            .map(|frame| frame.deadline_skipped_compile_requests)
            .sum::<usize>();
        let update_pump_stalled_frames = self
            .frames
            .iter()
            .filter(|frame| frame.update_pump_stalled)
            .count();
        let total_fluid_mutated_blocks = self
            .frames
            .iter()
            .map(|frame| frame.poll_fluid_mutated_blocks)
            .sum::<usize>();
        let total_fluid_executed_ticks = self
            .frames
            .iter()
            .map(|frame| frame.poll_fluid_executed_ticks)
            .sum::<usize>();
        let last_fluid_mutation = self
            .frames
            .iter()
            .enumerate()
            .rev()
            .find(|(_, frame)| frame.poll_fluid_mutated_blocks > 0);
        let final_frame = self.frames.last().copied().unwrap_or_default();

        println!("{{");
        let benchmark_name = if self.options.persisted_world {
            "native_startup_streaming_persisted_perf"
        } else {
            "native_startup_streaming_perf"
        };
        print_benchmark_metadata(benchmark_name, "  ", true);
        println!("  \"seed\": {},", self.options.scene.seed);
        println!(
            "  \"render_distance\": {},",
            self.options.scene.render_distance
        );
        println!(
            "  \"render_compile_workers\": {},",
            self.options.scene.render_compile_worker_count
        );
        print_optional_usize_json(
            "  ",
            "render_compile_max_pending_jobs",
            self.options.scene.render_compile_max_pending_jobs,
            true,
        );
        println!(
            "  \"render_compile_capacity_mode\": \"{}\",",
            self.options.scene.render_compile_capacity_mode.as_str()
        );
        println!(
            "  \"render_compile_capacity_advisory\": {},",
            serde_json::to_string(&startup_streaming_render_compile_capacity_advisory_json(
                &self.options.scene,
                &self.frames
            ))
            .expect("render compile capacity advisory JSON serialization should not fail")
        );
        let render_compile_worker_timing_compiled =
            mclone_app_runtime::render_assets::RENDER_COMPILE_WORKER_TIMING_COMPILED;
        println!(
            "  \"render_compile_worker_timing_requested\": {},",
            self.options.scene.render_compile_worker_timing_enabled
        );
        println!(
            "  \"render_compile_worker_timing_compiled\": {},",
            render_compile_worker_timing_compiled
        );
        println!(
            "  \"render_compile_worker_timing_enabled\": {},",
            self.options.scene.render_compile_worker_timing_enabled
                && render_compile_worker_timing_compiled
        );
        println!(
            "  \"simulation_cadence\": {{ \"host_hz\": {}, \"gameplay_hz\": {}, \"physics_hz\": {} }},",
            self.options.scene.simulation_cadence.host_rate_hz,
            self.options.scene.simulation_cadence.gameplay_rate_hz,
            self.options.scene.simulation_cadence.physics_rate_hz
        );
        println!(
            "  \"adaptive_chunk_publication_budget\": {},",
            self.options.scene.adaptive_chunk_publication_budget
        );
        println!(
            "  \"light_status_batch_size\": {},",
            self.options.scene.light_status_batch_size
        );
        println!(
            "  \"freeze_scheduled_fluid_ticks\": {},",
            self.options.freeze_scheduled_fluid_ticks
        );
        println!(
            "  \"section_occlusion_culling\": {},",
            self.options.render_options.section_occlusion_culling
        );
        println!(
            "  \"force_fullbright\": {},",
            self.options.render_options.force_fullbright
        );
        println!(
            "  \"render_color_profile\": \"{}\",",
            self.options.render_options.color_profile.as_str()
        );
        println!(
            "  \"lighting_enabled\": {},",
            self.options.scene.lighting_enabled
        );
        println!("  \"frames\": {},", self.options.frames);
        println!("  \"width\": {},", self.options.width);
        println!("  \"height\": {},", self.options.height);
        println!("  \"target_hz\": {:.3},", self.options.target_hz);
        println!("  \"target_frame_ms\": {:.3},", target_frame_ms);
        println!(
            "  \"world_storage\": \"{}\",",
            if self.options.persisted_world {
                "temp_sqlite_prewarmed"
            } else {
                "transient"
            }
        );
        match &self.world_dir {
            Some(path) => println!(
                "  \"world_dir\": \"{}\",",
                json_escape(&path.display().to_string())
            ),
            None => println!("  \"world_dir\": null,"),
        }
        match self.prewarm {
            Some(prewarm) => {
                println!("  \"prewarm\": {{");
                println!(
                    "    \"runtime_create_ms\": {:.3},",
                    prewarm.runtime_create_ms
                );
                println!(
                    "    \"runtime_poll_count\": {},",
                    prewarm.runtime_poll_count
                );
                println!("    \"runtime_poll_ms\": {:.3},", prewarm.runtime_poll_ms);
                println!("    \"total_ms\": {:.3},", prewarm.total_ms);
                println!(
                    "    \"target_ready_chunks\": {},",
                    prewarm.target_ready_chunks
                );
                println!(
                    "    \"target_chunk_count\": {},",
                    prewarm.target_chunk_count
                );
                println!("    \"target_percent\": {},", prewarm.target_percent);
                println!("    \"loaded_chunks\": {},", prewarm.loaded_chunks);
                println!(
                    "    \"client_visible_chunks\": {},",
                    prewarm.client_visible_chunks
                );
                println!(
                    "    \"active_ticket_chunks\": {},",
                    prewarm.active_ticket_chunks
                );
                println!("    \"pending_jobs\": {},", prewarm.pending_jobs);
                println!(
                    "    \"pending_publications\": {},",
                    prewarm.pending_publications
                );
                println!(
                    "    \"pending_render_chunks\": {},",
                    prewarm.pending_render_chunks
                );
                println!(
                    "    \"pending_render_compile_jobs\": {},",
                    prewarm.pending_render_compile_jobs
                );
                println!(
                    "    \"player_position_updates\": {},",
                    prewarm.player_position_updates
                );
                match prewarm.sqlite_storage_bytes {
                    Some(bytes) => println!("    \"sqlite_storage_bytes\": {bytes}"),
                    None => println!("    \"sqlite_storage_bytes\": null"),
                }
                println!("  }},");
            }
            None => println!("  \"prewarm\": null,"),
        }
        println!("  \"asset_load_ms\": {:.3},", self.asset_load_ms);
        println!(
            "  \"startup_playable_frame\": {},",
            self.startup_playable_frame
        );
        println!(
            "  \"startup_playable_ms\": {:.3},",
            self.startup_playable_ms
        );
        println!(
            "  \"startup_cached_sections\": {},",
            self.startup_cached_sections
        );
        println!(
            "  \"startup_target_ready_chunks\": {},",
            self.startup_target_ready_chunks
        );
        println!(
            "  \"startup_target_chunk_count\": {},",
            self.startup_target_chunk_count
        );
        println!(
            "  \"startup_target_percent\": {},",
            self.startup_target_percent
        );
        println!("  \"streaming_wall_ms\": {:.3},", self.streaming_wall_ms);
        match self.first_full_view_ready_frame {
            Some(frame) => println!("  \"first_full_view_ready_frame\": {frame},"),
            None => println!("  \"first_full_view_ready_frame\": null,"),
        }
        match self.first_full_view_ready_ms {
            Some(ms) => println!("  \"first_full_view_ready_ms\": {ms:.3},"),
            None => println!("  \"first_full_view_ready_ms\": null,"),
        }
        match self.first_initial_target_render_complete_frame {
            Some(frame) => {
                println!("  \"first_initial_target_render_complete_frame\": {frame},")
            }
            None => println!("  \"first_initial_target_render_complete_frame\": null,"),
        }
        match self.first_initial_target_render_complete_ms {
            Some(ms) => println!("  \"first_initial_target_render_complete_ms\": {ms:.3},"),
            None => println!("  \"first_initial_target_render_complete_ms\": null,"),
        }
        match self.first_render_quiescent_frame {
            Some(frame) => println!("  \"first_render_quiescent_frame\": {frame},"),
            None => println!("  \"first_render_quiescent_frame\": null,"),
        }
        match self.first_render_quiescent_ms {
            Some(ms) => println!("  \"first_render_quiescent_ms\": {ms:.3},"),
            None => println!("  \"first_render_quiescent_ms\": null,"),
        }
        match self.first_target_render_quiescent_frame {
            Some(frame) => println!("  \"first_target_render_quiescent_frame\": {frame},"),
            None => println!("  \"first_target_render_quiescent_frame\": null,"),
        }
        match self.first_target_render_quiescent_ms {
            Some(ms) => println!("  \"first_target_render_quiescent_ms\": {ms:.3},"),
            None => println!("  \"first_target_render_quiescent_ms\": null,"),
        }
        println!("  \"over_budget_frames\": {},", over_budget);
        println!(
            "  \"{}\": {},",
            mclone_diagnostics::legacy_json_keys::OVER_DOUBLE_BUDGET_FRAMES,
            over_double
        );
        println!(
            "  \"{}\": {},",
            mclone_diagnostics::legacy_json_keys::OVER_QUAD_BUDGET_FRAMES,
            over_quad
        );
        println!(
            "  \"average_frame_ms\": {:.3},",
            self.headless.average_frame_ms
        );
        println!("  \"p95_frame_ms\": {:.3},", p95);
        println!("  \"p99_frame_ms\": {:.3},", p99);
        println!("  \"max_frame_ms\": {:.3},", self.headless.max_frame_ms);
        println!("  \"total_poll_ms\": {:.3},", total_poll_ms);
        println!(
            "  \"total_poll_scheduler_publish_completed_ms\": {:.3},",
            total_poll_scheduler_publish_completed_ms
        );
        println!(
            "  \"total_poll_apply_updates_ms\": {:.3},",
            total_poll_apply_updates_ms
        );
        println!(
            "  \"total_poll_dirty_mark_ms\": {:.3},",
            total_poll_dirty_mark_ms
        );
        println!(
            "  \"total_poll_client_apply_updates_ms\": {:.3},",
            total_poll_client_apply_updates_ms
        );
        println!(
            "  \"total_poll_producer_read_ms\": {:.3},",
            total_poll_producer_read_ms
        );
        println!(
            "  \"total_poll_producer_decode_ms\": {:.3},",
            total_poll_producer_decode_ms
        );
        println!(
            "  \"max_poll_scheduler_publish_completed_ms\": {:.3},",
            max_poll_scheduler_publish_completed_ms
        );
        println!(
            "  \"total_scheduler_feature_chunks_published\": {},",
            total_scheduler_feature_chunks_published
        );
        println!(
            "  \"total_scheduler_light_statuses_published\": {},",
            total_scheduler_light_statuses_published
        );
        println!(
            "  \"total_scheduler_snapshot_ready_events\": {},",
            total_scheduler_snapshot_ready_events
        );
        println!(
            "  \"max_server_update_queue_depth\": {},",
            max_server_update_queue_depth
        );
        println!(
            "  \"max_server_update_oldest_applied_age_ms\": {:.3},",
            max_server_update_oldest_applied_age_ms
        );
        match max_poll_producer_response_sequence {
            Some(sequence) => println!("  \"max_poll_producer_response_sequence\": {sequence},"),
            None => println!("  \"max_poll_producer_response_sequence\": null,"),
        }
        println!(
            "  \"max_scheduler_pending_worldgen_publication_chunks\": {},",
            max_scheduler_pending_worldgen_publication_chunks
        );
        println!(
            "  \"max_scheduler_pending_light_publications\": {},",
            max_scheduler_pending_light_publications
        );
        println!(
            "  \"max_scheduler_worldgen_mailbox_pending_jobs\": {},",
            max_scheduler_worldgen_mailbox_pending_jobs
        );
        println!(
            "  \"max_scheduler_light_mailbox_pending_statuses\": {},",
            max_scheduler_light_mailbox_pending_statuses
        );
        print_light_status_mailbox_metrics_json(
            "  ",
            "light_status_mailbox_metrics",
            final_frame.light_status_mailbox_metrics,
            true,
        );
        println!("  \"total_remesh_ms\": {:.3},", total_remesh_ms);
        println!("  \"total_upload_ms\": {:.3},", total_upload_ms);
        println!("  \"total_render_ms\": {:.3},", total_render_ms);
        println!(
            "  \"total_submitted_compile_sections\": {},",
            total_submitted_compile_sections
        );
        println!(
            "  \"total_completed_compile_sections\": {},",
            total_completed_compile_sections
        );
        println!(
            "  \"total_uploaded_sections\": {},",
            total_uploaded_sections
        );
        println!(
            "  \"total_target_rebuilt_sections\": {},",
            total_target_rebuilt_sections
        );
        println!(
            "  \"total_non_target_rebuilt_sections\": {},",
            total_non_target_rebuilt_sections
        );
        println!(
            "  \"total_target_removed_sections\": {},",
            total_target_removed_sections
        );
        println!(
            "  \"total_non_target_removed_sections\": {},",
            total_non_target_removed_sections
        );
        println!(
            "  \"post_full_view_target_rebuilt_sections\": {},",
            post_full_view_target_rebuilt_sections
        );
        println!(
            "  \"post_full_view_non_target_rebuilt_sections\": {},",
            post_full_view_non_target_rebuilt_sections
        );
        println!(
            "  \"post_full_view_target_removed_sections\": {},",
            post_full_view_target_removed_sections
        );
        println!(
            "  \"post_full_view_non_target_removed_sections\": {},",
            post_full_view_non_target_removed_sections
        );
        println!(
            "  \"total_deadline_skipped_compile_requests\": {},",
            total_deadline_skipped_compile_requests
        );
        println!(
            "  \"total_fluid_executed_ticks\": {},",
            total_fluid_executed_ticks
        );
        println!(
            "  \"total_fluid_mutated_blocks\": {},",
            total_fluid_mutated_blocks
        );
        match last_fluid_mutation {
            Some((frame_index, frame)) => {
                println!("  \"last_fluid_mutation_frame\": {frame_index},");
                println!("  \"last_fluid_mutation_ms\": {:.3},", frame.elapsed_ms);
                println!(
                    "  \"last_fluid_mutation_simulation_tick\": {},",
                    frame.simulation_tick
                );
            }
            None => {
                println!("  \"last_fluid_mutation_frame\": null,");
                println!("  \"last_fluid_mutation_ms\": null,");
                println!("  \"last_fluid_mutation_simulation_tick\": null,");
            }
        }
        println!(
            "  \"update_pump_stalled_frames\": {},",
            update_pump_stalled_frames
        );
        print_frame_accounting_json_field(&frame_pipeline_report, true);
        let sampled_frames = self
            .frames
            .iter()
            .enumerate()
            .filter(|(index, _)| *index % 60 == 0 || *index + 1 == self.frames.len())
            .collect::<Vec<_>>();
        println!("  \"queue_samples\": [");
        for (sample_index, (index, frame)) in sampled_frames.iter().enumerate() {
            let suffix = if sample_index + 1 == sampled_frames.len() {
                ""
            } else {
                ","
            };
            println!("    {{");
            println!("      \"frame\": {},", index);
            println!("      \"elapsed_ms\": {:.3},", frame.elapsed_ms);
            println!(
                "      \"target_ready_chunks\": {},",
                frame.target_ready_chunks
            );
            println!(
                "      \"target_chunk_count\": {},",
                frame.target_chunk_count
            );
            println!("      \"loaded_chunks\": {},", frame.loaded_chunks);
            println!("      \"poll_ms\": {:.3},", frame.poll_ms);
            println!(
                "      \"poll_scheduler_publish_completed_ms\": {:.3},",
                frame.poll_scheduler_publish_completed_ms
            );
            println!(
                "      \"scheduler_adaptive_publication_budget_enabled\": {},",
                frame.scheduler_adaptive_publication_budget_enabled
            );
            println!(
                "      \"scheduler_feature_publish_budget_max_units\": {},",
                frame.scheduler_feature_publish_budget_max_units
            );
            println!(
                "      \"scheduler_feature_publish_budget_ms\": {:.3},",
                frame.scheduler_feature_publish_budget_ms
            );
            println!(
                "      \"scheduler_feature_publish_spent_units\": {},",
                frame.scheduler_feature_publish_spent_units
            );
            println!(
                "      \"scheduler_feature_publish_spent_ms\": {:.3},",
                frame.scheduler_feature_publish_spent_ms
            );
            print_optional_f64_json(
                "      ",
                "scheduler_feature_publish_estimated_unit_ms",
                frame.scheduler_feature_publish_estimated_unit_ms,
                true,
            );
            println!(
                "      \"scheduler_light_publish_budget_max_units\": {},",
                frame.scheduler_light_publish_budget_max_units
            );
            println!(
                "      \"scheduler_light_publish_budget_ms\": {:.3},",
                frame.scheduler_light_publish_budget_ms
            );
            println!(
                "      \"scheduler_light_publish_spent_units\": {},",
                frame.scheduler_light_publish_spent_units
            );
            println!(
                "      \"scheduler_light_publish_spent_ms\": {:.3},",
                frame.scheduler_light_publish_spent_ms
            );
            print_optional_f64_json(
                "      ",
                "scheduler_light_publish_estimated_unit_ms",
                frame.scheduler_light_publish_estimated_unit_ms,
                true,
            );
            println!(
                "      \"scheduler_pending_worldgen_publication_chunk_limit\": {},",
                frame.scheduler_pending_worldgen_publication_chunk_limit
            );
            println!(
                "      \"poll_apply_updates_ms\": {:.3},",
                frame.poll_apply_updates_ms
            );
            println!(
                "      \"poll_dirty_mark_ms\": {:.3},",
                frame.poll_dirty_mark_ms
            );
            println!(
                "      \"poll_client_apply_updates_ms\": {:.3},",
                frame.poll_client_apply_updates_ms
            );
            println!(
                "      \"poll_producer_read_ms\": {:.3},",
                frame.poll_producer_read_ms
            );
            println!(
                "      \"poll_producer_decode_ms\": {:.3},",
                frame.poll_producer_decode_ms
            );
            match frame.poll_producer_response_sequence {
                Some(sequence) => {
                    println!("      \"poll_producer_response_sequence\": {sequence},")
                }
                None => println!("      \"poll_producer_response_sequence\": null,"),
            }
            println!(
                "      \"scheduler_feature_chunks_published\": {},",
                frame.scheduler_feature_chunks_published
            );
            println!(
                "      \"scheduler_light_statuses_published\": {},",
                frame.scheduler_light_statuses_published
            );
            println!(
                "      \"scheduler_feature_snapshot_ready_events\": {},",
                frame.scheduler_feature_snapshot_ready_events
            );
            println!(
                "      \"scheduler_light_snapshot_ready_events\": {},",
                frame.scheduler_light_snapshot_ready_events
            );
            println!(
                "      \"scheduler_pending_worldgen_publication_jobs\": {},",
                frame.scheduler_pending_worldgen_publication_jobs
            );
            println!(
                "      \"scheduler_pending_worldgen_publication_chunks\": {},",
                frame.scheduler_pending_worldgen_publication_chunks
            );
            println!(
                "      \"scheduler_pending_light_publications\": {},",
                frame.scheduler_pending_light_publications
            );
            println!(
                "      \"scheduler_worldgen_mailbox_pending_jobs\": {},",
                frame.scheduler_worldgen_mailbox_pending_jobs
            );
            println!(
                "      \"scheduler_light_mailbox_pending_statuses\": {},",
                frame.scheduler_light_mailbox_pending_statuses
            );
            println!(
                "      \"server_update_queue_depth\": {},",
                frame.server_update_queue_depth
            );
            println!(
                "      \"server_update_queue_bytes\": {},",
                frame.server_update_queue_bytes
            );
            println!(
                "      \"server_update_oldest_applied_age_ms\": {:.3},",
                frame.server_update_oldest_applied_age_ms
            );
            println!("      \"simulation_tick\": {},", frame.simulation_tick);
            println!(
                "      \"poll_fluid_due_ticks\": {},",
                frame.poll_fluid_due_ticks
            );
            println!(
                "      \"poll_fluid_executed_ticks\": {},",
                frame.poll_fluid_executed_ticks
            );
            println!(
                "      \"poll_fluid_deferred_ticks\": {},",
                frame.poll_fluid_deferred_ticks
            );
            println!(
                "      \"poll_fluid_mutated_blocks\": {},",
                frame.poll_fluid_mutated_blocks
            );
            println!(
                "      \"poll_scheduled_fluid_ticks\": {},",
                frame.poll_scheduled_fluid_ticks
            );
            println!(
                "      \"pending_render_chunks\": {},",
                frame.pending_render_chunks
            );
            println!(
                "      \"ready_render_work_pending\": {},",
                frame.ready_render_work_pending
            );
            println!(
                "      \"target_pending_render_chunks\": {},",
                frame.target_pending_render_chunks
            );
            println!(
                "      \"target_ready_render_work_pending\": {},",
                frame.target_ready_render_work_pending
            );
            println!(
                "      \"pending_render_compile_jobs\": {},",
                frame.pending_render_compile_jobs
            );
            println!(
                "      \"target_inflight_render_sections\": {},",
                frame.target_inflight_render_sections
            );
            println!(
                "      \"submitted_compile_sections\": {},",
                frame.submitted_compile_sections
            );
            println!(
                "      \"completed_compile_sections\": {},",
                frame.completed_compile_sections
            );
            println!("      \"uploaded_sections\": {},", frame.uploaded_sections);
            println!(
                "      \"target_rebuilt_sections\": {},",
                frame.target_rebuilt_sections
            );
            println!(
                "      \"non_target_rebuilt_sections\": {},",
                frame.non_target_rebuilt_sections
            );
            println!(
                "      \"target_removed_sections\": {},",
                frame.target_removed_sections
            );
            println!(
                "      \"non_target_removed_sections\": {}",
                frame.non_target_removed_sections
            );
            println!("    }}{suffix}");
        }
        println!("  ],");
        println!("  \"final\": {{");
        println!("    \"elapsed_ms\": {:.3},", final_frame.elapsed_ms);
        println!(
            "    \"target_ready_chunks\": {},",
            final_frame.target_ready_chunks
        );
        println!(
            "    \"target_chunk_count\": {},",
            final_frame.target_chunk_count
        );
        println!("    \"target_percent\": {},", final_frame.target_percent);
        println!("    \"loaded_chunks\": {},", final_frame.loaded_chunks);
        println!("    \"simulation_tick\": {},", final_frame.simulation_tick);
        println!("    \"cached_sections\": {},", final_frame.cached_sections);
        println!("    \"pending_jobs\": {},", final_frame.pending_jobs);
        println!(
            "    \"pending_publications\": {},",
            final_frame.pending_publications
        );
        println!(
            "    \"pending_render_chunks\": {},",
            final_frame.pending_render_chunks
        );
        println!(
            "    \"ready_render_work_pending\": {},",
            final_frame.ready_render_work_pending
        );
        println!(
            "    \"target_pending_render_chunks\": {},",
            final_frame.target_pending_render_chunks
        );
        println!(
            "    \"target_ready_render_work_pending\": {},",
            final_frame.target_ready_render_work_pending
        );
        println!(
            "    \"pending_render_compile_jobs\": {},",
            final_frame.pending_render_compile_jobs
        );
        println!(
            "    \"inflight_render_sections\": {},",
            final_frame.inflight_render_sections
        );
        println!(
            "    \"target_inflight_render_sections\": {},",
            final_frame.target_inflight_render_sections
        );
        println!(
            "    \"server_update_queue_depth\": {},",
            final_frame.server_update_queue_depth
        );
        println!(
            "    \"server_update_queue_bytes\": {},",
            final_frame.server_update_queue_bytes
        );
        println!(
            "    \"poll_fluid_due_ticks\": {},",
            final_frame.poll_fluid_due_ticks
        );
        println!(
            "    \"poll_fluid_executed_ticks\": {},",
            final_frame.poll_fluid_executed_ticks
        );
        println!(
            "    \"poll_fluid_mutated_blocks\": {},",
            final_frame.poll_fluid_mutated_blocks
        );
        println!(
            "    \"poll_scheduled_fluid_ticks\": {},",
            final_frame.poll_scheduled_fluid_ticks
        );
        println!(
            "    \"poll_producer_read_ms\": {:.3},",
            final_frame.poll_producer_read_ms
        );
        println!(
            "    \"poll_producer_decode_ms\": {:.3},",
            final_frame.poll_producer_decode_ms
        );
        match final_frame.poll_producer_response_sequence {
            Some(sequence) => println!("    \"poll_producer_response_sequence\": {sequence},"),
            None => println!("    \"poll_producer_response_sequence\": null,"),
        }
        println!(
            "    \"server_update_oldest_applied_age_ms\": {:.3},",
            final_frame.server_update_oldest_applied_age_ms
        );
        println!(
            "    \"scheduler_pending_worldgen_publication_jobs\": {},",
            final_frame.scheduler_pending_worldgen_publication_jobs
        );
        println!(
            "    \"scheduler_pending_worldgen_publication_chunks\": {},",
            final_frame.scheduler_pending_worldgen_publication_chunks
        );
        println!(
            "    \"scheduler_pending_light_publications\": {},",
            final_frame.scheduler_pending_light_publications
        );
        println!(
            "    \"scheduler_adaptive_publication_budget_enabled\": {},",
            final_frame.scheduler_adaptive_publication_budget_enabled
        );
        println!(
            "    \"scheduler_feature_publish_budget_max_units\": {},",
            final_frame.scheduler_feature_publish_budget_max_units
        );
        println!(
            "    \"scheduler_feature_publish_budget_ms\": {:.3},",
            final_frame.scheduler_feature_publish_budget_ms
        );
        println!(
            "    \"scheduler_feature_publish_spent_units\": {},",
            final_frame.scheduler_feature_publish_spent_units
        );
        println!(
            "    \"scheduler_feature_publish_spent_ms\": {:.3},",
            final_frame.scheduler_feature_publish_spent_ms
        );
        print_optional_f64_json(
            "    ",
            "scheduler_feature_publish_estimated_unit_ms",
            final_frame.scheduler_feature_publish_estimated_unit_ms,
            true,
        );
        println!(
            "    \"scheduler_light_publish_budget_max_units\": {},",
            final_frame.scheduler_light_publish_budget_max_units
        );
        println!(
            "    \"scheduler_light_publish_budget_ms\": {:.3},",
            final_frame.scheduler_light_publish_budget_ms
        );
        println!(
            "    \"scheduler_light_publish_spent_units\": {},",
            final_frame.scheduler_light_publish_spent_units
        );
        println!(
            "    \"scheduler_light_publish_spent_ms\": {:.3},",
            final_frame.scheduler_light_publish_spent_ms
        );
        print_optional_f64_json(
            "    ",
            "scheduler_light_publish_estimated_unit_ms",
            final_frame.scheduler_light_publish_estimated_unit_ms,
            true,
        );
        println!(
            "    \"scheduler_pending_worldgen_publication_chunk_limit\": {},",
            final_frame.scheduler_pending_worldgen_publication_chunk_limit
        );
        println!(
            "    \"scheduler_worldgen_mailbox_pending_jobs\": {},",
            final_frame.scheduler_worldgen_mailbox_pending_jobs
        );
        println!(
            "    \"scheduler_light_mailbox_pending_statuses\": {},",
            final_frame.scheduler_light_mailbox_pending_statuses
        );
        print_light_status_mailbox_metrics_json(
            "    ",
            "light_status_mailbox_metrics",
            final_frame.light_status_mailbox_metrics,
            false,
        );
        println!("  }}");
        println!("}}");
    }
}

impl LoadingSettlePerfReport {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.samples.len() != self.options.distances.len() {
            bail!(
                "loading-settle report recorded {} samples, expected {}",
                self.samples.len(),
                self.options.distances.len()
            );
        }
        for sample in &self.samples {
            if sample.target_chunk_count != sample.expected_target_chunks {
                bail!(
                    "render distance {} target_chunk_count={} expected {}",
                    sample.render_distance,
                    sample.target_chunk_count,
                    sample.expected_target_chunks
                );
            }
            if sample.target_ready_chunks != sample.target_chunk_count {
                bail!(
                    "render distance {} target_ready_chunks={} target_chunk_count={}",
                    sample.render_distance,
                    sample.target_ready_chunks,
                    sample.target_chunk_count
                );
            }
            if sample.loaded_chunks != sample.target_chunk_count {
                bail!(
                    "render distance {} loaded_chunks={} target_chunk_count={}",
                    sample.render_distance,
                    sample.loaded_chunks,
                    sample.target_chunk_count
                );
            }
            if sample.player_position_updates == 0 {
                bail!(
                    "render distance {} did not publish the initial player spawn position",
                    sample.render_distance
                );
            }
            if sample.cached_sections == 0 {
                bail!(
                    "render distance {} produced no cached render sections",
                    sample.render_distance
                );
            }
            if sample.pending_jobs != 0
                || sample.pending_publications != 0
                || sample.pending_render_compile_jobs != 0
            {
                bail!(
                    "render distance {} did not fully settle: pending_jobs={} pending_publications={} pending_render_compile_jobs={}",
                    sample.render_distance,
                    sample.pending_jobs,
                    sample.pending_publications,
                    sample.pending_render_compile_jobs
                );
            }
        }
        Ok(())
    }

    pub(crate) fn print_json(&self) {
        println!("{{");
        print_benchmark_metadata("native_loading_settle", "  ", true);
        println!("  \"seed\": {},", self.options.scene.seed);
        println!("  \"world_storage\": \"transient\",");
        println!(
            "  \"render_compile_workers\": {},",
            self.options.scene.render_compile_worker_count
        );
        print_optional_usize_json(
            "  ",
            "render_compile_max_pending_jobs",
            self.options.scene.render_compile_max_pending_jobs,
            true,
        );
        println!(
            "  \"render_compile_capacity_mode\": \"{}\",",
            self.options.scene.render_compile_capacity_mode.as_str()
        );
        println!(
            "  \"simulation_cadence\": {{ \"host_hz\": {}, \"gameplay_hz\": {}, \"physics_hz\": {} }},",
            self.options.scene.simulation_cadence.host_rate_hz,
            self.options.scene.simulation_cadence.gameplay_rate_hz,
            self.options.scene.simulation_cadence.physics_rate_hz
        );
        println!(
            "  \"debug_passive_showcase\": {},",
            self.options.scene.debug_passive_showcase
        );
        println!(
            "  \"lighting_enabled\": {},",
            self.options.scene.lighting_enabled
        );
        println!(
            "  \"light_status_batch_size\": {},",
            self.options.scene.light_status_batch_size
        );
        println!("  \"asset_load_ms\": {:.3},", self.asset_load_ms);
        println!("  \"total_elapsed_ms\": {:.3},", self.total_elapsed_ms);
        println!("  \"samples\": [");
        for (index, sample) in self.samples.iter().enumerate() {
            let suffix = if index + 1 == self.samples.len() {
                ""
            } else {
                ","
            };
            println!("    {{");
            println!(
                "      \"render_distance_chunks\": {},",
                sample.render_distance
            );
            println!(
                "      \"chunk_tracking_radius\": {},",
                sample.chunk_tracking_radius
            );
            println!(
                "      \"spawn_center\": {{ \"x\": {}, \"z\": {} }},",
                sample.spawn_center.x, sample.spawn_center.z
            );
            println!(
                "      \"expected_target_chunks\": {},",
                sample.expected_target_chunks
            );
            println!(
                "      \"target_chunk_count\": {},",
                sample.target_chunk_count
            );
            println!(
                "      \"target_ready_chunks\": {},",
                sample.target_ready_chunks
            );
            println!("      \"loaded_chunks\": {},", sample.loaded_chunks);
            println!(
                "      \"client_visible_chunks\": {},",
                sample.client_visible_chunks
            );
            println!(
                "      \"active_ticket_chunks\": {},",
                sample.active_ticket_chunks
            );
            println!(
                "      \"runtime_create_ms\": {:.3},",
                sample.runtime_create_ms
            );
            println!(
                "      \"runtime_poll_count\": {},",
                sample.runtime_poll_count
            );
            println!("      \"runtime_poll_ms\": {:.3},", sample.runtime_poll_ms);
            println!(
                "      \"runtime_settle_ms\": {:.3},",
                sample.runtime_settle_ms
            );
            println!(
                "      \"render_mesh_settle_ms\": {:.3},",
                sample.render_mesh_settle_ms
            );
            println!("      \"full_settle_ms\": {:.3},", sample.full_settle_ms);
            println!(
                "      \"runtime_chunks_per_second\": {:.3},",
                sample.runtime_chunks_per_second
            );
            println!(
                "      \"full_chunks_per_second\": {:.3},",
                sample.full_chunks_per_second
            );
            println!(
                "      \"player_position_updates\": {},",
                sample.player_position_updates
            );
            println!("      \"cached_sections\": {},", sample.cached_sections);
            println!("      \"rebuilt_sections\": {},", sample.rebuilt_sections);
            println!("      \"removed_sections\": {},", sample.removed_sections);
            println!("      \"rebuilt_vertices\": {},", sample.rebuilt_vertices);
            println!("      \"rebuilt_faces\": {},", sample.rebuilt_faces);
            println!("      \"rebuilt_indices\": {},", sample.rebuilt_indices);
            println!(
                "      \"visibility_graph_build_count\": {},",
                sample.visibility_graph_build_count
            );
            println!(
                "      \"visibility_graph_total_ms\": {:.3},",
                sample.visibility_graph_total_ms
            );
            println!("      \"pending_jobs\": {},", sample.pending_jobs);
            println!(
                "      \"pending_publications\": {},",
                sample.pending_publications
            );
            println!(
                "      \"pending_render_chunks\": {},",
                sample.pending_render_chunks
            );
            println!(
                "      \"pending_render_compile_jobs\": {},",
                sample.pending_render_compile_jobs
            );
            println!("      \"simulation_tick\": {},", sample.simulation_tick);
            println!(
                "      \"simulation_seconds\": {:.3}",
                sample.simulation_seconds
            );
            println!("    }}{suffix}");
        }
        println!("  ]");
        println!("}}");
    }
}

fn startup_streaming_render_compile_capacity_advisory_json(
    scene: &SceneOptions,
    frames: &[StartupStreamingFrameReport],
) -> serde_json::Value {
    let mesh_footprint = startup_streaming_render_compile_mesh_footprint(frames);
    let report =
        render_compile_capacity_report(RenderCompileCapacityHostKind::Flat, mesh_footprint);
    let applied_derived = (scene.render_compile_capacity_mode
        == crate::cli::RenderCompileCapacityMode::DerivedApplied)
        .then(|| preflight_render_compile_capacity_report(RenderCompileCapacityHostKind::Flat));

    serde_json::json!({
        "mode": scene.render_compile_capacity_mode.as_str(),
        "applied": applied_derived.is_some(),
        "activeWorkers": scene.render_compile_worker_count,
        "activeMaxPendingJobs": scene.render_compile_max_pending_jobs,
        "appliedFootprintSource": if applied_derived.is_some() { "preflightEstimate" } else { "none" },
        "advisoryFootprintSource": "measuredFrameReports",
        "appliedDerived": applied_derived,
        "derived": report,
    })
}

fn startup_streaming_render_compile_mesh_footprint(
    frames: &[StartupStreamingFrameReport],
) -> RenderCompileMeshFootprint {
    let compile_request_bytes = frames
        .iter()
        .map(|frame| {
            frame
                .section_sync_timing
                .submit_request_estimated_payload_bytes_worst
        })
        .max()
        .and_then(non_zero_usize_to_u64);
    let mesh_vertex_bytes = frames
        .iter()
        .map(|frame| u64::from(frame.uploaded_vertices) * textured_chunk_vertex_byte_size())
        .max()
        .filter(|bytes| *bytes > 0);
    let mesh_index_bytes = frames
        .iter()
        .map(|frame| u64::from(frame.uploaded_indices) * std::mem::size_of::<u32>() as u64)
        .max()
        .filter(|bytes| *bytes > 0);

    RenderCompileMeshFootprint {
        compile_request_bytes,
        mesh_vertex_bytes,
        mesh_index_bytes,
    }
}

fn non_zero_usize_to_u64(value: usize) -> Option<u64> {
    (value > 0).then(|| u64::try_from(value).unwrap_or(u64::MAX))
}

fn textured_chunk_vertex_byte_size() -> u64 {
    std::mem::size_of::<TexturedChunkVertex>() as u64
}

fn target_frame_ms(target_hz: f64) -> f64 {
    1000.0 / target_hz.max(1.0)
}

fn per_second(count: usize, elapsed_ms: f64) -> f64 {
    if elapsed_ms <= 0.0 {
        0.0
    } else {
        count as f64 / (elapsed_ms / 1000.0)
    }
}

fn print_optional_f64_json(indent: &str, key: &str, value: Option<f64>, trailing_comma: bool) {
    let suffix = if trailing_comma { "," } else { "" };
    match value {
        Some(value) => println!("{indent}\"{key}\": {value:.3}{suffix}"),
        None => println!("{indent}\"{key}\": null{suffix}"),
    }
}

fn print_optional_usize_json(indent: &str, key: &str, value: Option<usize>, trailing_comma: bool) {
    let suffix = if trailing_comma { "," } else { "" };
    match value {
        Some(value) => println!("{indent}\"{key}\": {value}{suffix}"),
        None => println!("{indent}\"{key}\": null{suffix}"),
    }
}

fn print_light_status_mailbox_metrics_json(
    indent: &str,
    key: &str,
    metrics: LightStatusMailboxMetrics,
    trailing_comma: bool,
) {
    let suffix = if trailing_comma { "," } else { "" };
    println!("{indent}\"{key}\": {{");
    println!(
        "{indent}  \"enqueued_batches\": {},",
        metrics.enqueued_batches
    );
    println!(
        "{indent}  \"enqueued_statuses\": {},",
        metrics.enqueued_statuses
    );
    println!(
        "{indent}  \"completed_batches\": {},",
        metrics.completed_batches
    );
    println!(
        "{indent}  \"completed_statuses\": {},",
        metrics.completed_statuses
    );
    println!(
        "{indent}  \"max_pending_batches\": {},",
        metrics.max_pending_batches
    );
    println!(
        "{indent}  \"max_pending_statuses\": {},",
        metrics.max_pending_statuses
    );
    println!(
        "{indent}  \"max_batch_statuses\": {},",
        metrics.max_batch_statuses
    );
    println!(
        "{indent}  \"last_queue_wait_ms\": {:.3},",
        micros_to_ms(metrics.last_queue_wait_us)
    );
    println!(
        "{indent}  \"total_queue_wait_ms\": {:.3},",
        micros_to_ms(metrics.total_queue_wait_us)
    );
    println!(
        "{indent}  \"max_queue_wait_ms\": {:.3},",
        micros_to_ms(metrics.max_queue_wait_us)
    );
    println!(
        "{indent}  \"last_compute_ms\": {:.3},",
        micros_to_ms(metrics.last_compute_us)
    );
    println!(
        "{indent}  \"total_compute_ms\": {:.3},",
        micros_to_ms(metrics.total_compute_us)
    );
    println!(
        "{indent}  \"max_compute_ms\": {:.3},",
        micros_to_ms(metrics.max_compute_us)
    );
    println!(
        "{indent}  \"last_completion_drain_wait_ms\": {:.3},",
        micros_to_ms(metrics.last_completion_drain_wait_us)
    );
    println!(
        "{indent}  \"total_completion_drain_wait_ms\": {:.3},",
        micros_to_ms(metrics.total_completion_drain_wait_us)
    );
    println!(
        "{indent}  \"max_completion_drain_wait_ms\": {:.3}",
        micros_to_ms(metrics.max_completion_drain_wait_us)
    );
    println!("{indent}}}{suffix}");
}

fn headless_frame_accounting_report(
    frames: &[mclone_render::headless::HeadlessFrameLoopTiming],
    target_frame_ms: f64,
) -> FrameSummaryReport {
    let mut accumulator = FrameAccumulator::new(headless_frame_accounting_config(target_frame_ms));
    for (index, frame) in frames.iter().enumerate() {
        accumulator.record_frame(
            FrameObservation::new(index as u64, frame.frame_ms)
                .with_stage_span(StageSpan::new(
                    StageId::DrawEncode,
                    frame.encode_ms + frame.submit_ms,
                ))
                .with_stage_span(StageSpan::new(
                    StageId::GpuExecutionPresentationWait,
                    frame.device_poll_ms,
                )),
        );
    }
    accumulator.summary_report()
}

fn headless_frame_accounting_config(target_frame_ms: f64) -> FrameAccountingConfig {
    FrameAccountingConfig::from_target_period_ms(target_frame_ms)
        .with_percentile_method(PercentileMethod::InclusiveCeil)
}

fn record_frame_budget_probe_accounting(
    accumulator: &mut FrameAccumulator,
    frame_index: u64,
    frame_ms: f64,
    report: &FrameBudgetProbeFrameReport,
) {
    let mut observation = FrameObservation::new(frame_index, frame_ms)
        .with_stage_span(StageSpan::new(StageId::HostSessionCommands, report.poll_ms));
    for span in render_section_sync_stage_spans(report.section_sync_timing, report.remesh_ms) {
        observation = observation.with_stage_span(span);
    }
    observation = observation
        .with_stage_span(StageSpan::new(StageId::UploadApply, report.upload_ms))
        .with_stage_span(StageSpan::new(StageId::DrawEncode, report.render_ms));
    accumulator.record_frame(observation);
}

fn frame_accounting_total_violations(summary: Option<&FrameSummaryReport>) -> u64 {
    summary.map_or(0, |summary| summary.conservation_violations.total())
}

fn frame_accounting_observed_frames(summary: Option<&FrameSummaryReport>) -> u64 {
    summary.map_or(0, |summary| summary.frames)
}

fn print_frame_accounting_json_field(report: &FramePipelineReport, trailing_comma: bool) {
    for line in frame_pipeline_report_json_field_lines(
        "frame_pipeline_accounting",
        report,
        "  ",
        trailing_comma,
    )
    .expect("frame accounting report serializes")
    {
        println!("{line}");
    }
}

fn startup_streaming_frame_pipeline_report(
    frames: &[StartupStreamingFrameReport],
    headless_frames: &[mclone_render::headless::HeadlessFrameLoopTiming],
    target_frame_ms: f64,
    wall_ms: f64,
    budget_decision_panel: BudgetDecisionPanelReport,
) -> FramePipelineReport {
    let peer_threads = startup_streaming_peer_threads(frames, wall_ms);
    let mut accountant = FramePipelineAccountant::new(
        FrameAccountingConfig::from_target_period_ms(target_frame_ms)
            .with_percentile_method(PercentileMethod::InclusiveCeil),
    );
    for (index, frame) in frames.iter().enumerate() {
        let frame_wall_ms = headless_frames.get(index).map_or(
            frame.poll_ms + frame.remesh_ms + frame.upload_ms + frame.render_ms,
            |frame| frame.frame_ms,
        );
        let mut observation = FrameObservation::new(index as u64 + 1, frame_wall_ms)
            .with_stage_span(StageSpan::new(StageId::HostSessionCommands, frame.poll_ms));
        for span in render_section_sync_stage_spans(frame.section_sync_timing, frame.remesh_ms) {
            observation = observation.with_stage_span(span);
        }
        observation = observation
            .with_stage_span(StageSpan::new(StageId::UploadApply, frame.upload_ms))
            .with_stage_span(StageSpan::new(StageId::DrawEncode, frame.render_ms));
        accountant.record_prebuilt_at(
            observation,
            startup_streaming_queue_depths(*frame),
            FramePipelineReportExtras::default()
                .with_peer_threads(peer_threads)
                .with_budget_decision_panel(budget_decision_panel.clone()),
            frame.elapsed_ms,
        );
    }
    accountant
        .latest_report()
        .map(|(report, _)| (*report).clone())
        .expect("startup streaming report has at least one frame")
}

fn startup_streaming_queue_depths(frame: StartupStreamingFrameReport) -> FramePipelineQueueDepths {
    FramePipelineQueueDepths {
        observed: true,
        server_owned_lanes_remote: false,
        inbound_updates: frame.server_update_queue_depth,
        host_publication_runner: frame.pending_publications,
        host_publication_worldgen: frame.scheduler_pending_worldgen_publication_chunks,
        host_publication_light: frame.scheduler_pending_light_publications,
        render_compile_jobs: frame.pending_render_compile_jobs,
        completed_results: frame.queued_completed_compile_results,
        completed_results_processed: frame.completed_compile_sections,
        upload_work: 0,
        upload_work_processed: frame
            .uploaded_sections
            .saturating_add(frame.upload_removed_sections),
    }
}

fn startup_streaming_peer_threads(
    frames: &[StartupStreamingFrameReport],
    wall_ms: f64,
) -> FramePipelinePeerThreadInput {
    let mut peers = FramePipelinePeerThreadAccumulator::default();
    for frame in frames {
        peers.observe(FramePipelinePeerThreadInput {
            server_owned_lanes_remote: false,
            server_pending_jobs: frame.pending_jobs,
            server_busy_ms: frame.poll_server_reported_total_ms,
            worldgen_metrics: frame.worldgen_job_frame_metrics,
            worldgen_pending_jobs: frame.scheduler_worldgen_mailbox_pending_jobs,
            light_metrics: frame.light_status_job_frame_metrics,
            light_pending_jobs: frame.scheduler_light_mailbox_pending_statuses,
            render_compile_pending_jobs: frame.pending_render_compile_jobs,
            render_compile_submitted: frame.submitted_compile_sections,
            render_compile_completed: frame.completed_compile_sections,
            render_compile_busy_ms: frame
                .section_sync_timing
                .dispatcher_total_compile_worker_busy_ms,
            render_compile_max_task_ms: frame
                .section_sync_timing
                .dispatcher_max_compile_worker_task_ms,
            render_compile_idle_ms: None,
        });
    }
    let render_compile_worker_count = frames
        .iter()
        .map(|frame| frame.section_sync_timing.dispatcher_compile_worker_count)
        .max()
        .unwrap_or(0);
    let render_compile_busy_ms = peers.report().render_compile_busy_ms;
    peers
        .with_render_compile_idle_ms(worker_idle_ms(
            wall_ms,
            render_compile_worker_count,
            render_compile_busy_ms,
        ))
        .report()
}

fn worker_idle_ms(wall_ms: f64, worker_count: usize, busy_ms: f64) -> Option<f64> {
    if !wall_ms.is_finite() || wall_ms <= 0.0 || worker_count == 0 {
        return None;
    }
    let capacity_ms = wall_ms * worker_count as f64;
    (capacity_ms >= busy_ms).then_some((capacity_ms - busy_ms).max(0.0))
}

fn micros_to_ms(value: u128) -> f64 {
    value as f64 / 1000.0
}

pub(crate) fn run_movement_perf_smoke(options: &MovementPerfOptions) -> Result<MovementPerfReport> {
    if options.scene.remote_addr.is_some() {
        bail!("--movement-perf currently requires the local integrated server path");
    }
    let total_start = Instant::now();
    let mut runtime = WindowSceneRuntime::new(&options.scene)?;
    let mut steps = Vec::with_capacity(options.steps);

    for index in 0..options.steps {
        let step_start = Instant::now();
        let spectator = circular_movement_spectator(
            &options.scene,
            options.path_radius_chunks,
            index,
            options.steps,
        );
        let center = spectator.chunk_pos();
        let set_interest_start = Instant::now();
        runtime.set_interest_center(center)?;
        let set_interest_ms = elapsed_ms(set_interest_start.elapsed());
        let (poll_count, poll_ms) = poll_window_runtime_until_idle(&mut runtime)?;

        let remesh_start = Instant::now();
        let section_update = runtime.sync_all_render_sections(spectator.position)?;
        let sections = runtime.resident_section_metadata();
        let ready_sections = runtime.traversal_ready_render_section_keys(spectator.position);
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let camera = spectator.camera(runtime.render_distance());
        let render_view = camera.render_view(options.width, options.height);
        let visibility = textured_section_visibility_stats_with_options_and_ready_sections(
            &sections,
            render_view,
            options.render_options,
            Some(&ready_sections),
        );
        let stats = runtime.stats();

        steps.push(MovementPerfStepReport {
            index,
            center,
            elapsed_ms: elapsed_ms(step_start.elapsed()),
            set_interest_ms,
            poll_ms,
            remesh_ms,
            poll_count,
            loaded_chunks: stats.loaded_chunks,
            client_visible_chunks: stats.client_visible_chunks,
            active_ticket_chunks: stats.active_ticket_chunks,
            tracked_players: stats.tracked_players,
            player_visible_chunks: stats.player_visible_chunks,
            aggregate_player_ticket_chunks: stats.aggregate_player_ticket_chunks,
            player_outbound_queue_depth: stats.player_outbound_queue_depth,
            pending_render_compile_jobs: runtime.render_compile_pending_job_count(),
            max_pending_render_compile_jobs: runtime.render_compile_max_pending_job_count(),
            available_render_compile_slots: runtime.render_compile_available_pending_job_slots(),
            pending_unload_chunks: stats.pending_unload_chunks,
            block_ticking_chunks: stats.block_ticking_chunks,
            entity_ticking_chunks: stats.entity_ticking_chunks,
            simulation_tick: stats.last_simulation_tick,
            simulation_scheduler_tick_ms: stats.last_simulation_scheduler_tick_ms,
            simulation_block_tick_ms: stats.last_simulation_block_tick_ms,
            simulation_fluid_tick_ms: stats.last_simulation_fluid_tick_ms,
            simulation_entity_tick_ms: stats.last_simulation_entity_tick_ms,
            fluid_ticks_executed: stats.last_simulation_fluid_ticks_executed,
            deferred_fluid_ticks: stats.last_simulation_deferred_fluid_ticks,
            fluid_mutated_blocks: stats.last_simulation_fluid_mutated_blocks,
            scheduled_fluid_ticks: stats.scheduled_fluid_ticks,
            rebuilt_sections: section_update.rebuilt_section_count(),
            removed_sections: section_update.removed_section_count(),
            rebuilt_vertices: section_update.rebuilt_vertex_count,
            rebuilt_faces: section_update.rebuilt_face_count(),
            rebuilt_indices: section_update.rebuilt_index_count,
            visibility_graph_build_count: section_update.visibility_graph_stats.build_count,
            visibility_graph_total_ms: section_update.visibility_graph_stats.total_ms,
            visibility_graph_average_ms: section_update.visibility_graph_stats.average_ms(),
            visibility_graph_worst_ms: section_update.visibility_graph_stats.worst_ms,
            loaded_sections: visibility.loaded_section_count,
            visible_sections: visibility.drawn_section_count,
            frustum_sections: visibility.frustum_section_count,
            graph_cull_enabled: visibility.graph_cull_enabled,
            graph_culled_sections: visibility.graph_culled_section_count,
            loaded_faces: visibility.loaded_face_count(),
            visible_faces: visibility.drawn_face_count(),
            frustum_faces: visibility.frustum_face_count(),
            graph_culled_faces: visibility.graph_culled_face_count(),
            loaded_indices: visibility.loaded_index_count,
            visible_indices: visibility.drawn_index_count,
            frustum_indices: visibility.frustum_index_count,
            graph_culled_indices: visibility.graph_culled_index_count,
        });
    }

    Ok(MovementPerfReport {
        options: options.clone(),
        total_elapsed_ms: elapsed_ms(total_start.elapsed()),
        steps,
    })
}

pub(crate) fn run_loading_settle_perf(
    options: &LoadingSettlePerfOptions,
) -> Result<LoadingSettlePerfReport> {
    if options.scene.remote_addr.is_some() {
        bail!("--loading-settle-perf requires the local integrated server path");
    }
    if options.scene.world_dir.is_some() {
        bail!("--loading-settle-perf uses fresh transient worlds; omit --world-dir");
    }

    let total_start = Instant::now();
    let asset_start = Instant::now();
    let assets = WindowSceneAssets::load().context("failed to load window scene assets")?;
    let asset_load_ms = elapsed_ms(asset_start.elapsed());
    let mut samples = Vec::with_capacity(options.distances.len());

    for &render_distance in &options.distances {
        let chunk_tracking_radius = chunk_tracking_radius_for_render_distance(
            u32::try_from(render_distance).context("render distance must be non-negative")?,
        );
        let expected_target_chunks = square_count(
            i32::try_from(chunk_tracking_radius).context("chunk tracking radius exceeds i32")?,
        )?;
        let spawn_center = initial_spawn_center_for_seed(options.scene.seed);
        let mut scene = options.scene.clone();
        scene.chunk_x = spawn_center.x;
        scene.chunk_z = spawn_center.z;
        scene.render_distance = render_distance;
        scene.world_dir = None;
        let spectator = SpectatorCamera::spawn_for_scene(&scene);

        let runtime_start = Instant::now();
        let runtime_create_start = Instant::now();
        let mut runtime = WindowSceneRuntime::with_assets(&scene, &assets)?;
        let runtime_create_ms = elapsed_ms(runtime_create_start.elapsed());
        let (runtime_poll_count, runtime_poll_ms) =
            poll_window_runtime_until_loading_target_settled(
                &mut runtime,
                expected_target_chunks,
                LOADING_SETTLE_IDLE_TIMEOUT,
            )
            .with_context(|| {
                format!(
                    "render distance {render_distance} did not reach loading target before idle"
                )
            })?;
        let runtime_settle_ms = elapsed_ms(runtime_start.elapsed());
        let player_position_updates = runtime.drain_player_position_updates().len();

        let runtime_stats = runtime.stats();
        let loading_progress = runtime_stats.loading_progress.with_context(|| {
            format!("render distance {render_distance} did not publish loading progress")
        })?;
        let render_mesh_start = Instant::now();
        let section_update = runtime.sync_all_render_sections(spectator.position)?;
        let render_mesh_settle_ms = elapsed_ms(render_mesh_start.elapsed());
        let cached_sections = runtime.cached_section_count();
        let final_stats = runtime.stats();
        let full_settle_ms = runtime_settle_ms + render_mesh_settle_ms;
        let simulation_seconds = if scene.simulation_cadence.gameplay_rate_hz == 0 {
            0.0
        } else {
            final_stats.last_simulation_tick as f64
                / f64::from(scene.simulation_cadence.gameplay_rate_hz)
        };

        samples.push(LoadingSettlePerfSampleReport {
            render_distance,
            chunk_tracking_radius,
            spawn_center,
            expected_target_chunks,
            target_chunk_count: loading_progress.target_chunk_count,
            target_ready_chunks: loading_progress.target_ready_chunks,
            loaded_chunks: final_stats.loaded_chunks,
            client_visible_chunks: final_stats.client_visible_chunks,
            active_ticket_chunks: final_stats.active_ticket_chunks,
            runtime_create_ms,
            runtime_poll_count,
            runtime_poll_ms,
            runtime_settle_ms,
            render_mesh_settle_ms,
            full_settle_ms,
            runtime_chunks_per_second: per_second(
                loading_progress.target_chunk_count,
                runtime_settle_ms,
            ),
            full_chunks_per_second: per_second(loading_progress.target_chunk_count, full_settle_ms),
            player_position_updates,
            cached_sections,
            rebuilt_sections: section_update.rebuilt_section_count(),
            removed_sections: section_update.removed_section_count(),
            rebuilt_vertices: section_update.rebuilt_vertex_count,
            rebuilt_faces: section_update.rebuilt_face_count(),
            rebuilt_indices: section_update.rebuilt_index_count,
            visibility_graph_build_count: section_update.visibility_graph_stats.build_count,
            visibility_graph_total_ms: section_update.visibility_graph_stats.total_ms,
            pending_jobs: final_stats.pending_jobs,
            pending_publications: final_stats.pending_publications,
            pending_render_chunks: final_stats.pending_render_chunks,
            pending_render_compile_jobs: final_stats.pending_render_compile_jobs,
            simulation_tick: final_stats.last_simulation_tick,
            simulation_seconds,
        });
    }

    Ok(LoadingSettlePerfReport {
        options: options.clone(),
        asset_load_ms,
        total_elapsed_ms: elapsed_ms(total_start.elapsed()),
        samples,
    })
}

fn poll_window_runtime_until_loading_target_settled(
    runtime: &mut WindowSceneRuntime,
    expected_target_chunks: usize,
    timeout: Duration,
) -> Result<(usize, f64)> {
    let deadline = Instant::now() + timeout;
    let mut polls = 0_usize;
    let mut poll_ms = 0.0_f64;

    loop {
        let poll_start = Instant::now();
        runtime
            .poll_with_update_budget(RuntimeUpdatePumpBudget::unlimited())
            .context("failed to poll window runtime while waiting for loading progress")?;
        poll_ms += elapsed_ms(poll_start.elapsed());
        polls += 1;

        let stats = runtime.stats();
        if let Some(progress) = stats.loading_progress {
            if progress.target_chunk_count == expected_target_chunks
                && progress.target_ready_chunks == progress.target_chunk_count
            {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let (idle_polls, idle_ms) =
                    poll_window_runtime_until_idle_with_timeout(runtime, remaining)?;
                return Ok((polls + idle_polls, poll_ms + idle_ms));
            }
        }

        if Instant::now() >= deadline {
            let progress = stats.loading_progress;
            bail!(
                "timed out waiting for loading target after {:.3}s: expected_target_chunks={} loading_progress={:?} loaded_chunks={} pending_jobs={} pending_publications={} server_update_queue_depth={}",
                timeout.as_secs_f64(),
                expected_target_chunks,
                progress,
                stats.loaded_chunks,
                stats.pending_jobs,
                stats.pending_publications,
                stats.server_update_queue_depth
            );
        }

        if stats.server_update_queue_depth == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn prewarm_startup_streaming_world(
    scene: &SceneOptions,
    assets: &WindowSceneAssets,
    world_dir: &Path,
) -> Result<StartupStreamingPrewarmReport> {
    let total_start = Instant::now();
    let runtime_create_start = Instant::now();
    let mut runtime = WindowSceneRuntime::with_assets(scene, assets)
        .context("failed to create persisted startup-streaming prewarm runtime")?;
    let runtime_create_ms = elapsed_ms(runtime_create_start.elapsed());
    let (runtime_poll_count, runtime_poll_ms) =
        poll_window_runtime_until_idle_with_timeout(&mut runtime, LOADING_SETTLE_IDLE_TIMEOUT)
            .context("failed to prewarm persisted startup-streaming world")?;
    let progress = runtime.view_readiness_overlay();
    let target_ready_chunks = progress
        .as_ref()
        .map_or(0, |progress| progress.target_ready_chunks);
    let target_chunk_count = progress
        .as_ref()
        .map_or(0, |progress| progress.target_chunk_count);
    let target_percent = progress.as_ref().map_or(0, |progress| progress.percent());
    let stats = runtime.stats();
    let player_position_updates = runtime.drain_player_position_updates().len();
    drop(runtime);
    let sqlite_storage_bytes = startup_streaming_sqlite_storage_bytes(world_dir);
    Ok(StartupStreamingPrewarmReport {
        runtime_create_ms,
        runtime_poll_count,
        runtime_poll_ms,
        total_ms: elapsed_ms(total_start.elapsed()),
        target_ready_chunks,
        target_chunk_count,
        target_percent,
        loaded_chunks: stats.loaded_chunks,
        client_visible_chunks: stats.client_visible_chunks,
        active_ticket_chunks: stats.active_ticket_chunks,
        pending_jobs: stats.pending_jobs,
        pending_publications: stats.pending_publications,
        pending_render_chunks: stats.pending_render_chunks,
        pending_render_compile_jobs: stats.pending_render_compile_jobs,
        player_position_updates,
        sqlite_storage_bytes,
    })
}

fn unique_startup_streaming_world_dir(options: &StartupStreamingPerfOptions) -> PathBuf {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    env::temp_dir().join(format!(
        "mclone-startup-streaming-persisted-rd{}-{}-{timestamp_ms}",
        options.scene.render_distance,
        std::process::id()
    ))
}

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to remove existing benchmark world {}",
                path.display()
            )
        }),
    }
}

fn startup_streaming_sqlite_storage_bytes(world_dir: &Path) -> Option<u64> {
    let database_path = SqliteWorldStore::database_path_for_world_dir(world_dir);
    let mut total = 0_u64;
    let mut found = false;
    for path in [
        database_path.clone(),
        sqlite_auxiliary_path(&database_path, "-wal"),
        sqlite_auxiliary_path(&database_path, "-shm"),
    ] {
        if let Ok(metadata) = fs::metadata(path) {
            total = total.saturating_add(metadata.len());
            found = true;
        }
    }
    found.then_some(total)
}

fn sqlite_auxiliary_path(database_path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", database_path.display(), suffix))
}

fn startup_streaming_frame_render_quiescent(frame: &StartupStreamingFrameReport) -> bool {
    frame.target_chunk_count > 0
        && frame.target_ready_chunks == frame.target_chunk_count
        && frame.pending_jobs == 0
        && frame.pending_publications == 0
        && !frame.ready_render_work_pending
        && frame.pending_render_compile_jobs == 0
        && frame.inflight_render_sections == 0
        && frame.submitted_compile_sections == 0
        && frame.completed_compile_sections == 0
        && frame.uploaded_sections == 0
}

fn startup_streaming_frame_target_render_quiescent(frame: &StartupStreamingFrameReport) -> bool {
    frame.target_chunk_count > 0
        && frame.target_ready_chunks == frame.target_chunk_count
        && frame.pending_jobs == 0
        && frame.pending_publications == 0
        && !frame.target_ready_render_work_pending
        && frame.target_inflight_render_sections == 0
        && frame.target_rebuilt_sections == 0
        && frame.target_removed_sections == 0
}

fn startup_streaming_frame_initial_target_render_complete(
    frame: &StartupStreamingFrameReport,
) -> bool {
    frame.target_chunk_count > 0
        && frame.target_ready_chunks == frame.target_chunk_count
        && frame.target_pending_render_chunks == 0
        && !frame.target_ready_render_work_pending
        && frame.target_inflight_render_sections == 0
        && frame.target_rebuilt_sections == 0
        && frame.target_removed_sections == 0
}

fn first_startup_streaming_initial_target_render_complete(
    frames: &[StartupStreamingFrameReport],
) -> (Option<usize>, Option<f64>) {
    frames
        .iter()
        .enumerate()
        .find(|(_, frame)| startup_streaming_frame_initial_target_render_complete(frame))
        .map_or((None, None), |(index, frame)| {
            (Some(index), Some(frame.elapsed_ms))
        })
}

fn first_stable_startup_streaming_render_quiescent_with(
    frames: &[StartupStreamingFrameReport],
    mut is_quiescent: impl FnMut(&StartupStreamingFrameReport) -> bool,
) -> (Option<usize>, Option<f64>) {
    let mut suffix_quiescent = true;
    let mut first_frame = None;
    let mut first_ms = None;
    for (index, frame) in frames.iter().enumerate().rev() {
        suffix_quiescent &= is_quiescent(frame);
        if suffix_quiescent {
            first_frame = Some(index);
            first_ms = Some(frame.elapsed_ms);
        }
    }
    (first_frame, first_ms)
}

fn first_stable_startup_streaming_render_quiescent(
    frames: &[StartupStreamingFrameReport],
) -> (Option<usize>, Option<f64>) {
    first_stable_startup_streaming_render_quiescent_with(
        frames,
        startup_streaming_frame_render_quiescent,
    )
}

fn first_stable_startup_streaming_target_render_quiescent(
    frames: &[StartupStreamingFrameReport],
) -> (Option<usize>, Option<f64>) {
    first_stable_startup_streaming_render_quiescent_with(
        frames,
        startup_streaming_frame_target_render_quiescent,
    )
}

pub(crate) fn run_startup_streaming_perf(
    options: &StartupStreamingPerfOptions,
) -> Result<StartupStreamingPerfReport> {
    if options.scene.remote_addr.is_some() {
        bail!("--startup-streaming-perf requires the local integrated server path");
    }
    if options.scene.world_dir.is_some() {
        bail!("--startup-streaming-perf owns its benchmark world storage; omit --world-dir");
    }

    let asset_start = Instant::now();
    let assets = WindowSceneAssets::load().context("failed to load window scene assets")?;
    let asset_source = load_asset_source()?;
    let asset_load_ms = elapsed_ms(asset_start.elapsed());
    let mut scene = options.scene.clone();
    let spawn_center = initial_spawn_center_for_seed(scene.seed);
    scene.chunk_x = spawn_center.x;
    scene.chunk_z = spawn_center.z;
    let world_dir = if options.persisted_world {
        let world_dir = unique_startup_streaming_world_dir(options);
        remove_dir_if_exists(&world_dir)?;
        scene.world_dir = Some(world_dir.clone());
        Some(world_dir)
    } else {
        scene.world_dir = None;
        None
    };
    let prewarm = if let Some(world_dir) = world_dir.as_ref() {
        Some(prewarm_startup_streaming_world(&scene, &assets, world_dir)?)
    } else {
        None
    };
    let spectator = SpectatorCamera::spawn_for_scene(&scene);
    let startup_spectator = spectator.clone();
    let frame_duration = Duration::from_secs_f64(1.0 / options.target_hz.max(1.0));
    let target_frame_ms = 1_000.0 / options.target_hz.max(1.0);
    let freeze_scheduled_fluid_ticks = options.freeze_scheduled_fluid_ticks;
    let render_options = options.render_options;
    let streaming_start = Instant::now();
    let mut frame_reports = Vec::with_capacity(options.frames);
    let mut first_full_view_ready_frame = None;
    let mut first_full_view_ready_ms = None;

    let (headless, state) = run_headless_frame_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: options.frames,
            pace_frame_duration: Some(frame_duration),
        },
        move |device, queue, format, size| {
            let startup_start = Instant::now();
            let mut driver = crate::offscreen_scene_host::OffscreenDriver::new_with_options(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
                Some(&startup_spectator),
                crate::offscreen_scene_host::OffscreenDriverOptions::mono_with_frozen_scheduled_fluid_ticks(
                    freeze_scheduled_fluid_ticks,
                ),
            )?;
            let playable = driver.drive_to_wait_policy(
                device,
                queue,
                crate::cli::StartupWaitPolicy::Playable,
            )?;
            let startup_playable_ms = elapsed_ms(startup_start.elapsed());
            let progress = driver.host().mono_view_readiness_overlay();
            let startup_cached_sections = driver
                .host()
                .render_stats()
                .resident_cpu_mesh_section_count
                .max(driver.host().render_stats().section_count);
            driver.set_clock(crate::offscreen_scene_host::OffscreenFrameClock {
                frame_ms: target_frame_ms,
                target_frame_ms: Some(target_frame_ms),
            });
            Ok(StartupStreamingState {
                driver,
                startup_playable_frame: playable.frame_count,
                startup_playable_ms,
                startup_cached_sections,
                startup_target_ready_chunks: progress
                    .as_ref()
                    .map_or(0, |progress| progress.target_ready_chunks),
                startup_target_chunk_count: progress
                    .as_ref()
                    .map_or(0, |progress| progress.target_chunk_count),
                startup_target_percent: progress.as_ref().map_or(0, |progress| progress.percent()),
            })
        },
        |index, frame, state| {
            state.driver.set_camera(&spectator);
            let summary =
                state
                    .driver
                    .render(frame, mclone_scene::MonoUiPresentation::None, false)?;
            let mut report = StartupStreamingFrameReport {
                elapsed_ms: elapsed_ms(streaming_start.elapsed()),
                ..StartupStreamingFrameReport::default()
            };
            fill_startup_streaming_report_from_scene(
                &mut report,
                &summary,
                state.driver.host(),
                spectator.position,
            );
            let full_view_ready = report.target_chunk_count > 0
                && report.target_ready_chunks == report.target_chunk_count;
            if full_view_ready && first_full_view_ready_frame.is_none() {
                first_full_view_ready_frame = Some(index);
                first_full_view_ready_ms = Some(report.elapsed_ms);
            }
            frame_reports.push(report);
            Ok(())
        },
    )?;

    let (first_render_quiescent_frame, first_render_quiescent_ms) =
        first_stable_startup_streaming_render_quiescent(&frame_reports);
    let (first_target_render_quiescent_frame, first_target_render_quiescent_ms) =
        first_stable_startup_streaming_target_render_quiescent(&frame_reports);
    let (first_initial_target_render_complete_frame, first_initial_target_render_complete_ms) =
        first_startup_streaming_initial_target_render_complete(&frame_reports);

    Ok(StartupStreamingPerfReport {
        options: options.clone(),
        world_dir,
        prewarm,
        asset_load_ms,
        startup_playable_frame: state.startup_playable_frame,
        startup_playable_ms: state.startup_playable_ms,
        startup_cached_sections: state.startup_cached_sections,
        startup_target_ready_chunks: state.startup_target_ready_chunks,
        startup_target_chunk_count: state.startup_target_chunk_count,
        startup_target_percent: state.startup_target_percent,
        streaming_wall_ms: elapsed_ms(streaming_start.elapsed()),
        headless,
        frames: frame_reports,
        budget_decision_panel: state.driver.latest_budget_decision_panel(),
        first_full_view_ready_frame,
        first_full_view_ready_ms,
        first_initial_target_render_complete_frame,
        first_initial_target_render_complete_ms,
        first_render_quiescent_frame,
        first_render_quiescent_ms,
        first_target_render_quiescent_frame,
        first_target_render_quiescent_ms,
    })
}

struct StartupStreamingState {
    driver: crate::offscreen_scene_host::OffscreenDriver,
    startup_playable_frame: usize,
    startup_playable_ms: f64,
    startup_cached_sections: usize,
    startup_target_ready_chunks: usize,
    startup_target_chunk_count: usize,
    startup_target_percent: u8,
}

fn fill_startup_streaming_report_from_scene(
    report: &mut StartupStreamingFrameReport,
    summary: &mclone_scene::MonoSceneFrameSummary,
    host: &crate::desktop_scene_host::DesktopMonoSceneHost,
    camera_position: Vec3,
) {
    report.poll_ms = summary.timing.runtime_poll_ms;
    report.remesh_ms = summary.timing.runtime_sync_ms;
    report.upload_ms = summary.timing.runtime_gpu_upload_ms;
    report.render_ms = summary.timing.render_views_ms;
    report.section_sync_timing = summary.upload.section_sync_timing;
    report.submitted_compile_sections = summary.upload.submitted_compile_section_count;
    report.accepted_compile_results = summary.upload.accepted_compile_result_count;
    report.queued_completed_compile_results = summary.upload.queued_completed_compile_result_count;
    report.completed_compile_sections = summary.upload.completed_compile_section_count;
    report.uploaded_sections = summary.upload.uploaded_section_count;
    report.uploaded_vertices = summary.upload.uploaded_vertex_count;
    report.uploaded_indices = summary.upload.uploaded_index_count;
    report.upload_removed_sections = summary.upload.upload_removed_section_count;
    report.target_rebuilt_sections = summary.upload.target_rebuilt_section_count;
    report.non_target_rebuilt_sections = summary.upload.non_target_rebuilt_section_count;
    report.target_removed_sections = summary.upload.target_removed_section_count;
    report.non_target_removed_sections = summary.upload.non_target_removed_section_count;
    report.deadline_skipped_compile_requests =
        summary.upload.deadline_skipped_compile_request_count;

    let progress = host.mono_view_readiness_overlay();
    report.target_ready_chunks = progress
        .as_ref()
        .map_or(0, |progress| progress.target_ready_chunks);
    report.target_chunk_count = progress
        .as_ref()
        .map_or(0, |progress| progress.target_chunk_count);
    report.target_percent = progress.as_ref().map_or(0, |progress| progress.percent());
    if let Some(stats) = host.runtime_stats() {
        report.loaded_chunks = stats.loaded_chunks;
        report.simulation_tick = stats.last_simulation_tick;
        report.pending_jobs = stats.pending_jobs;
        report.pending_publications = stats.pending_publications;
        report.pending_render_chunks = stats.pending_render_chunks;
        report.pending_render_compile_jobs = stats.pending_render_compile_jobs;
        report.inflight_render_sections = stats.inflight_render_sections;
    }
    report.cached_sections = host
        .render_stats()
        .resident_cpu_mesh_section_count
        .max(host.render_stats().section_count);
    report.ready_render_work_pending = host.pending_stream_work(camera_position) > 0;
    let target = host.mono_target_render_work_stats(camera_position);
    report.target_pending_render_chunks = target.pending_render_chunks;
    report.target_ready_render_work_pending = target.ready_render_work_pending;
    report.target_inflight_render_sections = target.inflight_render_sections;
    if let Some(diagnostics) = host.runtime_poll_diagnostics() {
        fill_startup_streaming_poll_diagnostics(report, &diagnostics);
    }
}

fn fill_startup_streaming_poll_diagnostics(
    report: &mut StartupStreamingFrameReport,
    diagnostics: &mclone_app_runtime::RuntimePollDiagnostics,
) {
    report.poll_server_reported_total_ms = diagnostics.server_reported_total_ms;
    report.poll_scheduler_publish_completed_ms = diagnostics.scheduler_publish_completed_ms;
    report.scheduler_adaptive_publication_budget_enabled =
        diagnostics.scheduler_adaptive_publication_budget_enabled;
    report.scheduler_feature_publish_budget_max_units =
        diagnostics.scheduler_feature_publish_budget_max_units;
    report.scheduler_feature_publish_budget_ms = diagnostics.scheduler_feature_publish_budget_ms;
    report.scheduler_feature_publish_spent_units =
        diagnostics.scheduler_feature_publish_spent_units;
    report.scheduler_feature_publish_spent_ms = diagnostics.scheduler_feature_publish_spent_ms;
    report.scheduler_feature_publish_estimated_unit_ms =
        diagnostics.scheduler_feature_publish_estimated_unit_ms;
    report.scheduler_light_publish_budget_max_units =
        diagnostics.scheduler_light_publish_budget_max_units;
    report.scheduler_light_publish_budget_ms = diagnostics.scheduler_light_publish_budget_ms;
    report.scheduler_light_publish_spent_units = diagnostics.scheduler_light_publish_spent_units;
    report.scheduler_light_publish_spent_ms = diagnostics.scheduler_light_publish_spent_ms;
    report.scheduler_light_publish_estimated_unit_ms =
        diagnostics.scheduler_light_publish_estimated_unit_ms;
    report.scheduler_pending_worldgen_publication_chunk_limit =
        diagnostics.scheduler_pending_worldgen_publication_chunk_limit;
    report.poll_fluid_due_ticks = diagnostics.fluid_due_ticks;
    report.poll_fluid_executed_ticks = diagnostics.fluid_executed_ticks;
    report.poll_fluid_deferred_ticks = diagnostics.fluid_deferred_ticks;
    report.poll_fluid_mutated_blocks = diagnostics.fluid_mutated_blocks;
    report.poll_scheduled_fluid_ticks = diagnostics.scheduled_fluid_ticks;
    report.poll_apply_updates_ms = diagnostics.apply_updates_ms;
    report.poll_dirty_mark_ms = diagnostics.dirty_mark_ms;
    report.poll_client_apply_updates_ms = diagnostics.client_apply_updates_ms;
    report.poll_producer_read_ms = diagnostics.producer_read_ms;
    report.poll_producer_decode_ms = diagnostics.producer_decode_ms;
    report.poll_producer_response_sequence = diagnostics.producer_response_sequence;
    report.update_pump_stalled = diagnostics.update_pump_stalled;
    report.server_update_queue_depth = diagnostics.server_update_queue_depth;
    report.server_update_queue_bytes = diagnostics.server_update_queue_bytes;
    report.server_update_oldest_applied_age_ms = diagnostics.server_update_oldest_applied_age_ms;
    report.scheduler_completed_feature_jobs_drained =
        diagnostics.scheduler_completed_feature_jobs_drained;
    report.scheduler_feature_chunks_published = diagnostics.scheduler_feature_chunks_published;
    report.scheduler_feature_chunks_skipped = diagnostics.scheduler_feature_chunks_skipped;
    report.scheduler_feature_jobs_completed = diagnostics.scheduler_feature_jobs_completed;
    report.scheduler_feature_snapshot_ready_events =
        diagnostics.scheduler_feature_snapshot_ready_events;
    report.scheduler_light_status_batches_enqueued =
        diagnostics.scheduler_light_status_batches_enqueued;
    report.scheduler_completed_light_statuses_drained =
        diagnostics.scheduler_completed_light_statuses_drained;
    report.scheduler_light_statuses_published = diagnostics.scheduler_light_statuses_published;
    report.scheduler_light_statuses_skipped = diagnostics.scheduler_light_statuses_skipped;
    report.scheduler_light_snapshot_ready_events =
        diagnostics.scheduler_light_snapshot_ready_events;
    report.scheduler_pending_worldgen_publication_jobs =
        diagnostics.scheduler_pending_worldgen_publication_jobs;
    report.scheduler_pending_worldgen_publication_chunks =
        diagnostics.scheduler_pending_worldgen_publication_chunks;
    report.scheduler_pending_light_publications = diagnostics.scheduler_pending_light_publications;
    report.scheduler_worldgen_mailbox_pending_jobs =
        diagnostics.scheduler_worldgen_mailbox_pending_jobs;
    report.scheduler_light_mailbox_pending_statuses =
        diagnostics.scheduler_light_mailbox_pending_statuses;
    report.runner_frame_metrics = diagnostics.runner_frame_metrics;
    report.worldgen_job_frame_metrics = diagnostics.worldgen_job_frame_metrics;
    report.light_status_job_frame_metrics = diagnostics.light_status_job_frame_metrics;
    report.light_status_mailbox_metrics = diagnostics.light_status_mailbox_metrics;
}

pub(crate) fn run_timedemo(options: &TimedemoOptions) -> Result<TimedemoReport> {
    if options.scene.remote_addr.is_some() {
        bail!("--timedemo currently requires the local integrated server path");
    }
    let loaded_scene = timedemo_loaded_scene(options)?;
    let loaded_render_distance = loaded_scene.render_distance;
    let cameras = timedemo_cameras(&options.scene, options.path_radius_chunks, options.frames);
    let assets = WindowSceneAssets::load()?;
    let asset_source = load_asset_source()?;
    let render_options = options.render_options;
    let startup_camera = SpectatorCamera::spawn_for_scene(&loaded_scene);
    let frame_cameras = cameras.clone();
    let (headless, state) = run_headless_frame_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: cameras.len(),
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let mut driver = crate::offscreen_scene_host::OffscreenDriver::new(
                device,
                queue,
                format,
                size,
                &loaded_scene,
                render_options,
                &assets,
                &asset_source,
                Some(&startup_camera),
            )?;
            let warmup = driver.drive_until_target_complete(device, queue)?;
            Ok(TimedemoState {
                driver,
                warmup,
                drawn_sections: 0,
                max_drawn_sections: 0,
                frustum_sections: 0,
                max_frustum_sections: 0,
                graph_cull_frames: 0,
                graph_culled_sections: 0,
                max_graph_culled_sections: 0,
                drawn_indices: 0,
                max_drawn_indices: 0,
                frustum_indices: 0,
                max_frustum_indices: 0,
                graph_culled_indices: 0,
                max_graph_culled_indices: 0,
            })
        },
        |index, frame, state| {
            let summary = state.driver.render_chunk_camera_frozen(
                frame,
                frame_cameras[index],
                mclone_scene::MonoUiPresentation::None,
            )?;
            let render = summary.render;
            state.drawn_sections += render.drawn_section_count;
            state.max_drawn_sections = state.max_drawn_sections.max(render.drawn_section_count);
            state.frustum_sections += render.frustum_section_count;
            state.max_frustum_sections =
                state.max_frustum_sections.max(render.frustum_section_count);
            state.graph_cull_frames += usize::from(render.graph_cull_enabled);
            state.graph_culled_sections += render.graph_culled_section_count;
            state.max_graph_culled_sections = state
                .max_graph_culled_sections
                .max(render.graph_culled_section_count);
            state.drawn_indices += u64::from(render.drawn_index_count);
            state.max_drawn_indices = state.max_drawn_indices.max(render.drawn_index_count);
            state.frustum_indices += u64::from(render.frustum_index_count);
            state.max_frustum_indices = state.max_frustum_indices.max(render.frustum_index_count);
            state.graph_culled_indices += u64::from(render.graph_culled_index_count);
            state.max_graph_culled_indices = state
                .max_graph_culled_indices
                .max(render.graph_culled_index_count);
            Ok(())
        },
    )?;

    let frame_count = headless.frame_count.max(1);
    let divisor = frame_count as f64;
    let stream = state.driver.host().render_stats();
    let section_count = stream
        .resident_cpu_mesh_section_count
        .max(stream.section_count);
    let vertex_count = state.driver.host().mono_render_vertex_count();
    let index_count = stream.index_count;
    let render = mclone_render::headless::HeadlessTimedemoReport {
        width: headless.width,
        height: headless.height,
        frame_count: headless.frame_count,
        setup_ms: headless.setup_ms,
        total_frame_ms: headless.total_frame_ms,
        average_frame_ms: headless.average_frame_ms,
        min_frame_ms: headless.min_frame_ms,
        max_frame_ms: headless.max_frame_ms,
        loaded_section_count: stream.section_count,
        average_drawn_section_count: state.drawn_sections as f64 / divisor,
        max_drawn_section_count: state.max_drawn_sections,
        average_frustum_section_count: state.frustum_sections as f64 / divisor,
        max_frustum_section_count: state.max_frustum_sections,
        graph_cull_enabled_frame_count: state.graph_cull_frames,
        average_graph_culled_section_count: state.graph_culled_sections as f64 / divisor,
        max_graph_culled_section_count: state.max_graph_culled_sections,
        loaded_index_count: stream.index_count,
        average_drawn_index_count: state.drawn_indices as f64 / divisor,
        max_drawn_index_count: state.max_drawn_indices,
        average_frustum_index_count: state.frustum_indices as f64 / divisor,
        max_frustum_index_count: state.max_frustum_indices,
        average_graph_culled_index_count: state.graph_culled_indices as f64 / divisor,
        max_graph_culled_index_count: state.max_graph_culled_indices,
    };
    Ok(TimedemoReport {
        options: options.clone(),
        loaded_render_distance,
        visibility_graph_stats: state.warmup.visibility_graph,
        scene_build_ms: state.warmup.elapsed_ms,
        section_count,
        vertex_count,
        face_count: quad_face_count_from_indices(index_count),
        index_count,
        render,
    })
}

pub(crate) fn run_frame_budget_probe(
    options: &FrameBudgetProbeOptions,
) -> Result<FrameBudgetProbeReport> {
    if options.scene.remote_addr.is_some() {
        bail!("frame-budget probes currently require the local integrated server path");
    }
    if options.frames == 0 {
        bail!("frame-budget probe requires at least one frame");
    }
    if !options.target_hz.is_finite() || options.target_hz <= 0.0 {
        bail!("frame-budget probe target Hz must be finite and greater than zero");
    }
    if matches!(options.mode, FrameBudgetProbeMode::MovementWalk)
        && (!options.movement_speed.is_finite() || options.movement_speed <= 0.0)
    {
        bail!("movement frame probe speed must be finite and greater than zero");
    }

    let initial_spectator = frame_budget_probe_spectator(options, 0);
    let initial_center = initial_spectator.chunk_pos();
    let mut runtime_scene = options.scene.clone();
    runtime_scene.chunk_x = initial_center.x;
    runtime_scene.chunk_z = initial_center.z;
    let assets = WindowSceneAssets::load()?;
    let asset_source = load_asset_source()?;
    let probe_options = options.clone();
    let render_options = options.render_options;

    let mut frame_reports = Vec::with_capacity(options.frames);
    let (headless, state) = run_headless_frame_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: options.frames,
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let setup_start = Instant::now();
            let mut driver = crate::offscreen_scene_host::OffscreenDriver::new(
                device,
                queue,
                format,
                size,
                &runtime_scene,
                render_options,
                &assets,
                &asset_source,
                Some(&initial_spectator),
            )?;
            let warmup = driver.drive_until_target_complete(device, queue)?;
            driver.set_clock(crate::offscreen_scene_host::OffscreenFrameClock {
                frame_ms: 1_000.0 / probe_options.target_hz,
                target_frame_ms: Some(1_000.0 / probe_options.target_hz),
            });
            let render_stats = driver.host().render_stats();
            Ok(FrameBudgetProbeState {
                initial_poll_count: warmup.frame_count,
                initial_poll_ms: warmup.poll_ms,
                initial_remesh_ms: warmup.sync_ms,
                initial_section_count: render_stats
                    .resident_cpu_mesh_section_count
                    .max(render_stats.section_count),
                initial_face_count: render_stats.face_count,
                initial_index_count: render_stats.index_count,
                runtime_setup_ms: elapsed_ms(setup_start.elapsed()),
                frame_accounting: probe_options.frame_accounting_enabled.then(|| {
                    FrameAccumulator::new(headless_frame_accounting_config(target_frame_ms(
                        probe_options.target_hz,
                    )))
                }),
                driver,
            })
        },
        |index, frame, state| {
            let frame_start = Instant::now();
            let spectator = frame_budget_probe_spectator(&probe_options, index);
            let center = spectator.chunk_pos();
            let before_stats = state.driver.host().runtime_stats();
            let set_interest_start = Instant::now();
            state.driver.set_camera(&spectator);
            let interest_updates_changed = state.driver.commit_camera()?;
            let set_interest_ms = elapsed_ms(set_interest_start.elapsed());
            let summary =
                state
                    .driver
                    .render(frame, mclone_scene::MonoUiPresentation::None, false)?;

            let mut report = FrameBudgetProbeFrameReport {
                index,
                center,
                interest_center_changed: before_stats
                    .is_some_and(|stats| stats.interest_center != center),
                interest_updates_changed,
                set_interest_ms,
                ..FrameBudgetProbeFrameReport::default()
            };
            fill_frame_budget_report_from_scene(&mut report, &summary, state.driver.host());
            if let Some(accounting) = state.frame_accounting.as_mut() {
                let accounting_frame_ms = elapsed_ms(frame_start.elapsed());
                let accounting_frame = accounting.len() as u64 + 1;
                record_frame_budget_probe_accounting(
                    accounting,
                    accounting_frame,
                    accounting_frame_ms,
                    &report,
                );
            }
            frame_reports.push(report);
            Ok(())
        },
    )?;

    let frame_accounting = state
        .frame_accounting
        .as_ref()
        .map(FrameAccumulator::summary_report);
    Ok(FrameBudgetProbeReport {
        options: options.clone(),
        runtime_setup_ms: state.runtime_setup_ms,
        initial_poll_count: state.initial_poll_count,
        initial_poll_ms: state.initial_poll_ms,
        initial_remesh_ms: state.initial_remesh_ms,
        initial_section_count: state.initial_section_count,
        initial_face_count: state.initial_face_count,
        initial_index_count: state.initial_index_count,
        headless,
        frame_accounting,
        frames: frame_reports,
    })
}

fn fill_frame_budget_report_from_scene(
    report: &mut FrameBudgetProbeFrameReport,
    summary: &mclone_scene::MonoSceneFrameSummary,
    host: &crate::desktop_scene_host::DesktopMonoSceneHost,
) {
    report.runtime_changed = summary.upload.poll_changed;
    report.poll_ms = summary.timing.runtime_poll_ms;
    report.remesh_ms = summary.timing.runtime_sync_ms;
    report.upload_ms = summary.timing.runtime_gpu_upload_ms;
    report.render_ms = summary.timing.render_views_ms;
    report.section_sync_timing = summary.upload.section_sync_timing;
    report.rebuilt_sections = summary.upload.rebuilt_section_count;
    report.removed_sections = summary.upload.removed_section_count;
    report.submitted_compile_sections = summary.upload.submitted_compile_section_count;
    report.deadline_skipped_compile_requests =
        summary.upload.deadline_skipped_compile_request_count;
    report.accepted_compile_results = summary.upload.accepted_compile_result_count;
    report.queued_completed_compile_results = summary.upload.queued_completed_compile_result_count;
    report.completed_compile_sections = summary.upload.completed_compile_section_count;
    report.stale_compile_sections = summary.upload.stale_compile_section_count;
    report.uploaded_sections = summary.upload.uploaded_section_count;
    report.upload_removed_sections = summary.upload.upload_removed_section_count;
    report.uploaded_vertices = summary.upload.uploaded_vertex_count;
    report.uploaded_indices = summary.upload.uploaded_index_count;
    report.max_pending_render_compile_jobs = summary.upload.max_pending_compile_jobs;
    report.available_render_compile_slots = summary.upload.available_compile_slots_after;
    report.drawn_sections = summary.render.drawn_section_count;
    report.drawn_indices = summary.render.drawn_index_count;

    if let Some(stats) = host.runtime_stats() {
        report.loaded_chunks = stats.loaded_chunks;
        report.pending_jobs = stats.pending_jobs;
        report.pending_publications = stats.pending_publications;
        report.pending_render_chunks = stats.pending_render_chunks;
        report.pending_render_compile_jobs = stats.pending_render_compile_jobs;
        report.inflight_render_sections = stats.inflight_render_sections;
    }
    if let Some(diagnostics) = host.runtime_poll_diagnostics() {
        fill_frame_budget_poll_diagnostics(report, &diagnostics);
    }
}

fn fill_frame_budget_poll_diagnostics(
    report: &mut FrameBudgetProbeFrameReport,
    diagnostics: &mclone_app_runtime::RuntimePollDiagnostics,
) {
    report.poll_flush_commands_ms = diagnostics.flush_commands_ms;
    report.poll_server_tick_ms = diagnostics.server_tick_ms;
    report.poll_server_reported_total_ms = diagnostics.server_reported_total_ms;
    report.poll_scheduler_tick_ms = diagnostics.scheduler_tick_ms;
    report.poll_scheduler_report_ms = diagnostics.scheduler_report_ms;
    report.poll_scheduler_purge_stale_tickets_ms = diagnostics.scheduler_purge_stale_tickets_ms;
    report.poll_scheduler_reconcile_holders_ms = diagnostics.scheduler_reconcile_holders_ms;
    report.poll_scheduler_publish_completed_ms = diagnostics.scheduler_publish_completed_ms;
    report.poll_scheduler_adaptive_publication_budget_enabled =
        diagnostics.scheduler_adaptive_publication_budget_enabled;
    report.poll_scheduler_feature_publish_budget_max_units =
        diagnostics.scheduler_feature_publish_budget_max_units;
    report.poll_scheduler_feature_publish_budget_ms =
        diagnostics.scheduler_feature_publish_budget_ms;
    report.poll_scheduler_feature_publish_spent_units =
        diagnostics.scheduler_feature_publish_spent_units;
    report.poll_scheduler_feature_publish_spent_ms = diagnostics.scheduler_feature_publish_spent_ms;
    report.poll_scheduler_feature_publish_estimated_unit_ms =
        diagnostics.scheduler_feature_publish_estimated_unit_ms;
    report.poll_scheduler_light_publish_budget_max_units =
        diagnostics.scheduler_light_publish_budget_max_units;
    report.poll_scheduler_light_publish_budget_ms = diagnostics.scheduler_light_publish_budget_ms;
    report.poll_scheduler_light_publish_spent_units =
        diagnostics.scheduler_light_publish_spent_units;
    report.poll_scheduler_light_publish_spent_ms = diagnostics.scheduler_light_publish_spent_ms;
    report.poll_scheduler_light_publish_estimated_unit_ms =
        diagnostics.scheduler_light_publish_estimated_unit_ms;
    report.poll_scheduler_pending_worldgen_publication_chunk_limit =
        diagnostics.scheduler_pending_worldgen_publication_chunk_limit;
    report.poll_scheduler_pending_unload_ms = diagnostics.scheduler_pending_unload_ms;
    report.poll_scheduler_apply_events_ms = diagnostics.scheduler_apply_events_ms;
    report.poll_scheduler_completed_feature_jobs_drained =
        diagnostics.scheduler_completed_feature_jobs_drained;
    report.poll_scheduler_feature_chunks_published = diagnostics.scheduler_feature_chunks_published;
    report.poll_scheduler_feature_chunks_skipped = diagnostics.scheduler_feature_chunks_skipped;
    report.poll_scheduler_feature_jobs_completed = diagnostics.scheduler_feature_jobs_completed;
    report.poll_scheduler_feature_snapshot_ready_events =
        diagnostics.scheduler_feature_snapshot_ready_events;
    report.poll_scheduler_light_status_batches_enqueued =
        diagnostics.scheduler_light_status_batches_enqueued;
    report.poll_scheduler_completed_light_statuses_drained =
        diagnostics.scheduler_completed_light_statuses_drained;
    report.poll_scheduler_light_statuses_published = diagnostics.scheduler_light_statuses_published;
    report.poll_scheduler_light_statuses_skipped = diagnostics.scheduler_light_statuses_skipped;
    report.poll_scheduler_light_snapshot_ready_events =
        diagnostics.scheduler_light_snapshot_ready_events;
    report.poll_scheduler_pending_worldgen_publication_jobs =
        diagnostics.scheduler_pending_worldgen_publication_jobs;
    report.poll_scheduler_pending_worldgen_publication_chunks =
        diagnostics.scheduler_pending_worldgen_publication_chunks;
    report.poll_scheduler_pending_light_publications =
        diagnostics.scheduler_pending_light_publications;
    report.poll_scheduler_worldgen_mailbox_pending_jobs =
        diagnostics.scheduler_worldgen_mailbox_pending_jobs;
    report.poll_scheduler_light_mailbox_pending_statuses =
        diagnostics.scheduler_light_mailbox_pending_statuses;
    report.poll_block_tick_ms = diagnostics.block_tick_ms;
    report.poll_fluid_tick_ms = diagnostics.fluid_tick_ms;
    report.poll_fluid_event_apply_ms = diagnostics.fluid_event_apply_ms;
    report.poll_fluid_due_scan_ms = diagnostics.fluid_due_scan_ms;
    report.poll_fluid_remove_due_ms = diagnostics.fluid_remove_due_ms;
    report.poll_fluid_tick_fluid_ms = diagnostics.fluid_tick_fluid_ms;
    report.poll_fluid_set_block_ms = diagnostics.fluid_set_block_ms;
    report.poll_entity_tick_ms = diagnostics.entity_tick_ms;
    report.poll_apply_updates_ms = diagnostics.apply_updates_ms;
    report.poll_dirty_mark_ms = diagnostics.dirty_mark_ms;
    report.poll_client_apply_updates_ms = diagnostics.client_apply_updates_ms;
    report.poll_producer_read_ms = diagnostics.producer_read_ms;
    report.poll_producer_decode_ms = diagnostics.producer_decode_ms;
    report.poll_producer_response_sequence = diagnostics.producer_response_sequence;
    report.update_pump_stalled = diagnostics.update_pump_stalled;
    report.update_pump_stall_count = diagnostics.update_pump_stall_count;
    report.server_update_queue_depth = diagnostics.server_update_queue_depth;
    report.server_update_queue_bytes = diagnostics.server_update_queue_bytes;
    report.server_update_applied_bytes = diagnostics.server_update_applied_bytes;
    report.server_update_oldest_applied_age_ms = diagnostics.server_update_oldest_applied_age_ms;
    report.poll_scheduler_events = diagnostics.scheduler_events;
    report.poll_updates = diagnostics.updates;
    report.poll_snapshot_updates = diagnostics.snapshot_updates;
    report.poll_section_block_updates = diagnostics.section_block_updates;
    report.poll_unload_updates = diagnostics.unload_updates;
    report.poll_pending_unloads_processed = diagnostics.pending_unloads_processed;
    report.poll_fluid_due_ticks = diagnostics.fluid_due_ticks;
    report.poll_fluid_executed_ticks = diagnostics.fluid_executed_ticks;
    report.poll_fluid_deferred_ticks = diagnostics.fluid_deferred_ticks;
    report.poll_fluid_mutated_blocks = diagnostics.fluid_mutated_blocks;
    report.poll_fluid_snapshot_events = diagnostics.fluid_snapshot_events;
    report.poll_fluid_event_count = diagnostics.fluid_event_count;
    report.poll_scheduled_fluid_ticks = diagnostics.scheduled_fluid_ticks;
}

fn frame_budget_probe_spectator(
    options: &FrameBudgetProbeOptions,
    index: usize,
) -> SpectatorCamera {
    match options.mode {
        FrameBudgetProbeMode::StressOrbit => circular_movement_spectator(
            &options.scene,
            options.path_radius_chunks,
            index,
            options.frames,
        ),
        FrameBudgetProbeMode::MovementWalk => movement_frame_probe_spectator(
            &options.scene,
            options.path_radius_chunks,
            index,
            options.target_hz,
            options.movement_speed,
        ),
    }
}

fn timedemo_loaded_scene(options: &TimedemoOptions) -> Result<SceneOptions> {
    let loaded_radius = options
        .scene
        .render_distance
        .max(options.path_radius_chunks);
    if loaded_radius > MAX_RENDER_DISTANCE {
        bail!(
            "--timedemo requires static loaded radius {loaded_radius}, but the current max is {MAX_RENDER_DISTANCE}; lower --path-radius"
        );
    }
    let mut scene = options.scene.clone();
    scene.render_distance = loaded_radius;
    Ok(scene)
}

fn circular_movement_spectator(
    scene: &SceneOptions,
    path_radius_chunks: i32,
    index: usize,
    steps: usize,
) -> SpectatorCamera {
    let origin_x = scene.chunk_x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let origin_z = scene.chunk_z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let radius_blocks = path_radius_chunks.max(1) as f32 * CHUNK_WIDTH as f32;
    let angle = std::f32::consts::TAU * index as f32 / steps.max(1) as f32;
    let position = Vec3::new(
        origin_x + radius_blocks * angle.cos(),
        88.0,
        origin_z + radius_blocks * angle.sin(),
    );
    let target = Vec3::new(origin_x, 56.0, origin_z);
    let direction = (target - position).normalize_or_zero();
    let horizontal = Vec3::new(direction.x, 0.0, direction.z).length();
    SpectatorCamera {
        position,
        yaw: direction.x.atan2(direction.z),
        pitch: direction.y.atan2(horizontal),
        speed: SPECTATOR_BASE_SPEED,
    }
}

fn movement_frame_probe_spectator(
    scene: &SceneOptions,
    path_radius_chunks: i32,
    index: usize,
    target_hz: f64,
    movement_speed: f32,
) -> SpectatorCamera {
    let origin_x = scene.chunk_x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let origin_z = scene.chunk_z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let radius_blocks = path_radius_chunks.max(1) as f32 * CHUNK_WIDTH as f32;
    let elapsed_secs = index as f32 / target_hz.max(1.0) as f32;
    let angle = movement_speed.clamp(SPECTATOR_MIN_SPEED, SPECTATOR_MAX_SPEED) * elapsed_secs
        / radius_blocks;
    let position = Vec3::new(
        origin_x + radius_blocks * angle.cos(),
        88.0,
        origin_z + radius_blocks * angle.sin(),
    );
    let target = Vec3::new(origin_x, 56.0, origin_z);
    let direction = (target - position).normalize_or_zero();
    let horizontal = Vec3::new(direction.x, 0.0, direction.z).length();
    SpectatorCamera {
        position,
        yaw: direction.x.atan2(direction.z),
        pitch: direction.y.atan2(horizontal),
        speed: movement_speed,
    }
}

fn timedemo_cameras(
    scene: &SceneOptions,
    path_radius_chunks: i32,
    frames: usize,
) -> Vec<ChunkCamera> {
    (0..frames)
        .map(|index| timedemo_camera(scene, path_radius_chunks, index, frames))
        .collect()
}

fn timedemo_camera(
    scene: &SceneOptions,
    path_radius_chunks: i32,
    index: usize,
    frames: usize,
) -> ChunkCamera {
    let center_x = scene.chunk_x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let center_z = scene.chunk_z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
    let radius_blocks = path_radius_chunks.max(1) as f32 * CHUNK_WIDTH as f32;
    let angle = std::f32::consts::TAU * index as f32 / frames.max(1) as f32;
    let bob = (angle * 2.0).sin() * 8.0;
    ChunkCamera {
        eye: [
            center_x + radius_blocks * angle.cos(),
            92.0 + bob,
            center_z + radius_blocks * angle.sin(),
        ],
        target: [center_x, 52.0, center_z],
        up: [0.0, 1.0, 0.0],
        fov_y_radians: 65.0_f32.to_radians(),
        z_near: 0.1,
        z_far: 900.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_budget_summary_uses_shared_accounting_math() {
        let frames = [
            mclone_render::headless::HeadlessFrameLoopTiming {
                frame_ms: 4.0,
                ..Default::default()
            },
            mclone_render::headless::HeadlessFrameLoopTiming {
                frame_ms: 9.0,
                ..Default::default()
            },
            mclone_render::headless::HeadlessFrameLoopTiming {
                frame_ms: 17.0,
                ..Default::default()
            },
            mclone_render::headless::HeadlessFrameLoopTiming {
                frame_ms: 35.0,
                ..Default::default()
            },
        ];

        assert_eq!(target_frame_ms(125.0), 8.0);
        let summary = headless_frame_accounting_report(&frames, 8.0);
        assert_eq!(summary.over_budget.single_period_frames(), 3);
        assert_eq!(summary.over_budget.double_period_frames(), 2);
        assert_eq!(summary.over_budget.quad_period_frames(), 1);
        assert_eq!(summary.frame_wall.p95_ms, 35.0);
    }

    #[test]
    fn startup_streaming_target_quiescence_ignores_non_target_edge_uploads() {
        let frame = |elapsed_ms, target_rebuilt_sections, non_target_rebuilt_sections| {
            StartupStreamingFrameReport {
                elapsed_ms,
                target_ready_chunks: 1,
                target_chunk_count: 1,
                target_pending_render_chunks: 0,
                target_inflight_render_sections: 0,
                uploaded_sections: target_rebuilt_sections + non_target_rebuilt_sections,
                target_rebuilt_sections,
                non_target_rebuilt_sections,
                ..StartupStreamingFrameReport::default()
            }
        };
        let frames = [frame(1.0, 0, 1), frame(2.0, 0, 1), frame(3.0, 0, 0)];

        assert_eq!(
            first_stable_startup_streaming_render_quiescent(&frames),
            (Some(2), Some(3.0))
        );
        assert_eq!(
            first_stable_startup_streaming_target_render_quiescent(&frames),
            (Some(0), Some(1.0))
        );

        let target_frames = [frame(1.0, 0, 0), frame(2.0, 1, 0), frame(3.0, 0, 0)];
        assert_eq!(
            first_stable_startup_streaming_target_render_quiescent(&target_frames),
            (Some(2), Some(3.0))
        );
    }

    #[test]
    fn startup_streaming_initial_target_render_complete_ignores_later_target_rebuilds() {
        let frame = |elapsed_ms, target_rebuilt_sections| StartupStreamingFrameReport {
            elapsed_ms,
            target_ready_chunks: 1,
            target_chunk_count: 1,
            target_pending_render_chunks: 0,
            target_inflight_render_sections: 0,
            target_rebuilt_sections,
            ..StartupStreamingFrameReport::default()
        };
        let frames = [frame(1.0, 0), frame(2.0, 1), frame(3.0, 0)];

        assert_eq!(
            first_startup_streaming_initial_target_render_complete(&frames),
            (Some(0), Some(1.0))
        );
        assert_eq!(
            first_stable_startup_streaming_target_render_quiescent(&frames),
            (Some(2), Some(3.0))
        );
    }

    #[test]
    fn timedemo_loaded_scene_covers_camera_path_radius() {
        let options = TimedemoOptions {
            scene: SceneOptions {
                render_distance: 1,
                ..SceneOptions::default()
            },
            path_radius_chunks: 4,
            ..TimedemoOptions::default()
        };

        let loaded_scene = timedemo_loaded_scene(&options).unwrap();

        assert_eq!(loaded_scene.render_distance, 4);
        assert_eq!(options.scene.render_distance, 1);
    }

    #[test]
    fn timedemo_loaded_scene_rejects_oversized_static_radius() {
        let options = TimedemoOptions {
            scene: SceneOptions {
                render_distance: 1,
                ..SceneOptions::default()
            },
            path_radius_chunks: MAX_RENDER_DISTANCE + 1,
            ..TimedemoOptions::default()
        };

        assert!(timedemo_loaded_scene(&options).is_err());
    }

    #[test]
    fn circular_movement_spectator_faces_origin_from_ring() {
        let scene = SceneOptions::default();
        let spectator = circular_movement_spectator(&scene, 4, 0, 8);

        assert_eq!(spectator.chunk_pos(), ChunkPos::new(4, 0));
        assert!(spectator.forward().x < -0.5);
        assert!(spectator.forward().z.abs() < 0.2);
    }

    #[test]
    fn movement_frame_probe_spectator_advances_by_speed_and_target_hz() {
        let scene = SceneOptions::default();
        let spectator = movement_frame_probe_spectator(&scene, 4, 120, 120.0, 32.0);
        let origin_x = scene.chunk_x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
        let origin_z = scene.chunk_z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
        let angle = (spectator.position.z - origin_z).atan2(spectator.position.x - origin_x);

        assert!((angle - 0.5).abs() < 0.001);
        assert_eq!(spectator.speed, 32.0);
    }
}
