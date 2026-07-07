use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_render::{
    FullFrameGui, RenderStreamStats, record_render_section_update_stats, render_full_frame_for_view,
};
use mclone_app_runtime::{RenderSectionSyncTiming, RuntimeUpdatePumpBudget};
use mclone_client::{ActorInterpolationConfig, ActorInterpolationState};
use mclone_core::{CHUNK_WIDTH, ChunkPos};
use mclone_diagnostics::{
    BudgetDecisionPanelReport, FrameAccountingConfig, FrameAccumulator, FrameObservation,
    FramePipelineReport, FrameSummaryReport, PeerThreadActivityReport, PeerThreadId,
    PeerThreadPanelReport, PercentileMethod, QueueAgeTracker, QueueId, QueuePanelReport, StageId,
    StageSpan,
};
use mclone_mesh::{RenderSectionKey, VisibilityGraphBuildStats, quad_face_count_from_indices};
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, TexturedSectionDrawResources, TexturedSectionUploadReport,
    textured_section_visibility_stats_with_options_and_ready_sections,
};
use mclone_render::entity::ActorDrawResources;
use mclone_render::gui::GuiRenderer;
use mclone_render::headless::{
    HeadlessFrameLoopOptions, HeadlessTimedemoOptions, run_headless_frame_loop,
    run_headless_textured_sections_timedemo,
};
use mclone_render::screen_effect::{ScreenEffectsRenderer, UnderwaterOverlay};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::RenderFrameContext;
use mclone_render_session::{actor_instances_from_presentations, render_section_chunk_pos};
use mclone_server::{SqliteWorldStore, WorkerFrameMetrics, initial_spawn_center_for_seed};
use mclone_ui::{GameCollisionMode, GameMovementMode, GameTravelAssistMode, GameUiHost, GuiScale};

use crate::camera::{
    SPECTATOR_BASE_SPEED, SPECTATOR_MAX_SPEED, SPECTATOR_MIN_SPEED, SpectatorCamera,
};
use crate::cli::{
    FrameBudgetProbeMode, FrameBudgetProbeOptions, LoadingSettlePerfOptions, MovementPerfOptions,
    SceneOptions, StartupStreamingPerfOptions, TimedemoOptions,
};
use crate::flat_client_driver::{FlatClientUiRenderOptions, game_ui_render_state};
use crate::frame_pacing::{FramePacingUiState, elapsed_ms};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{
    WindowSceneAssets, WindowSceneRuntime, WindowSceneStartupPump, build_scene_textured_sections,
    chunk_tracking_radius_for_render_distance, poll_window_runtime_until_idle,
    poll_window_runtime_until_idle_with_timeout, square_count,
};
use crate::{
    MAX_RENDER_DISTANCE, MAX_STARTUP_STREAMING_PERF_FRAMES, json_escape, print_benchmark_metadata,
};

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
        println!(
            "  \"adaptive_chunk_publication_budget\": {},",
            self.options.scene.adaptive_chunk_publication_budget
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
    runner_frame_metrics: WorkerFrameMetrics,
    worldgen_job_frame_metrics: WorkerFrameMetrics,
    light_status_job_frame_metrics: WorkerFrameMetrics,
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

impl FrameBudgetProbeFrameReport {
    fn add_section_timing(&mut self, timing: FrameBudgetProbeSectionTiming) {
        self.remesh_ms += timing.remesh_ms;
        self.upload_ms += timing.upload_ms;
        self.section_sync_timing.merge(timing.sync_timing);
        self.rebuilt_sections += timing.rebuilt_sections;
        self.removed_sections += timing.removed_sections;
        self.submitted_compile_sections += timing.submitted_compile_sections;
        self.deadline_skipped_compile_requests += timing.deadline_skipped_compile_requests;
        self.accepted_compile_results += timing.accepted_compile_results;
        self.queued_completed_compile_results += timing.queued_completed_compile_results;
        self.completed_compile_sections += timing.completed_compile_sections;
        self.stale_compile_sections += timing.stale_compile_sections;
        self.uploaded_sections += timing.uploaded_sections;
        self.upload_removed_sections += timing.upload_removed_sections;
        self.uploaded_vertices += timing.uploaded_vertices;
        self.uploaded_indices += timing.uploaded_indices;
    }
}

struct FrameBudgetProbeState {
    runtime: WindowSceneRuntime,
    actor_interpolation: ActorInterpolationState,
    depth: ChunkDepthTarget,
    sky: SkyRenderer,
    draw: TexturedSectionDrawResources,
    actors: ActorDrawResources,
    screen_effects: ScreenEffectsRenderer,
    gui: GuiRenderer,
    ui: GameUiHost,
    render_stats: RenderStreamStats,
    frame_accounting: Option<FrameAccumulator>,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameBudgetProbeSectionTiming {
    remesh_ms: f64,
    upload_ms: f64,
    sync_timing: RenderSectionSyncTiming,
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
    target_rebuilt_sections: usize,
    non_target_rebuilt_sections: usize,
    target_removed_sections: usize,
    non_target_removed_sections: usize,
    uploaded_vertices: u32,
    uploaded_indices: u32,
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
        print_frame_accounting_json_field(
            pipeline_frame_accounting,
            QueuePanelReport::new(Vec::new()),
            PeerThreadPanelReport::empty(),
            BudgetDecisionPanelReport::empty(),
            true,
        );
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
        let frame_accounting = startup_streaming_frame_accounting_report(
            &self.frames,
            &self.headless.frames,
            target_frame_ms,
        );
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
            "  \"update_pump_stalled_frames\": {},",
            update_pump_stalled_frames
        );
        print_frame_accounting_json_field(
            &frame_accounting,
            startup_streaming_queue_panel(&self.frames),
            startup_streaming_peer_panel(&self.frames),
            self.budget_decision_panel.clone(),
            true,
        );
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
            "    \"scheduler_light_mailbox_pending_statuses\": {}",
            final_frame.scheduler_light_mailbox_pending_statuses
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

fn print_frame_accounting_json_field(
    summary: &FrameSummaryReport,
    queue_panel: QueuePanelReport,
    peer_thread_panel: PeerThreadPanelReport,
    budget_decision_panel: BudgetDecisionPanelReport,
    trailing_comma: bool,
) {
    let report = FramePipelineReport::new(summary.clone(), queue_panel)
        .with_peer_thread_panel(peer_thread_panel)
        .with_budget_decision_panel(budget_decision_panel);
    let json = serde_json::to_string_pretty(&report).expect("frame accounting report serializes");
    let lines = json.lines().collect::<Vec<_>>();
    let suffix = if trailing_comma { "," } else { "" };
    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            println!("  \"frame_pipeline_accounting\": {line}");
        } else if index + 1 == lines.len() {
            println!("  {line}{suffix}");
        } else {
            println!("  {line}");
        }
    }
}

fn render_section_sync_stage_spans(
    timing: RenderSectionSyncTiming,
    total_sync_ms: f64,
) -> Vec<StageSpan> {
    let dirty_ready_scan_ms = timing.dirty_seed_ms + timing.prepare_ms;
    let request_build_ms = timing.submit_snapshot_ms + timing.submit_request_build_ms;
    let worker_submit_ms = timing.submit_compiler_ms;
    let prepared_record_ms = timing.submit_mark_inflight_ms
        + timing.submit_apply_ready_plan_ms
        + timing.submit_ready_update_ms;
    let attributed_ms = timing.completed_result_accept_ms
        + dirty_ready_scan_ms
        + request_build_ms
        + worker_submit_ms
        + prepared_record_ms;
    let unattributed_ms = (total_sync_ms - attributed_ms).max(0.0);
    vec![
        StageSpan::new(
            StageId::CompletedResultAcceptance,
            timing.completed_result_accept_ms,
        ),
        StageSpan::new(StageId::RenderAdmissionDirtyReadyScan, dirty_ready_scan_ms),
        StageSpan::new(StageId::RenderAdmissionRequestBuild, request_build_ms),
        StageSpan::new(StageId::RenderAdmissionWorkerSubmit, worker_submit_ms),
        StageSpan::new(
            StageId::RenderAdmissionPreparedRecordMaintenance,
            prepared_record_ms,
        ),
        StageSpan::new(StageId::RenderSectionAdmission, unattributed_ms),
    ]
}

fn render_section_in_target(
    key: RenderSectionKey,
    interest_center: ChunkPos,
    render_distance: u32,
) -> bool {
    let distance = i32::try_from(render_distance).unwrap_or(i32::MAX);
    let pos = render_section_chunk_pos(key);
    (pos.x - interest_center.x)
        .abs()
        .max((pos.z - interest_center.z).abs())
        <= distance
}

fn count_target_section_keys(
    keys: impl Iterator<Item = RenderSectionKey>,
    interest_center: ChunkPos,
    render_distance: u32,
) -> usize {
    keys.filter(|key| render_section_in_target(*key, interest_center, render_distance))
        .count()
}

fn startup_streaming_frame_accounting_report(
    frames: &[StartupStreamingFrameReport],
    headless_frames: &[mclone_render::headless::HeadlessFrameLoopTiming],
    target_frame_ms: f64,
) -> FrameSummaryReport {
    let mut accumulator = FrameAccumulator::new(
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
        accumulator.record_frame(observation);
    }
    accumulator.summary_report()
}

fn startup_streaming_queue_panel(frames: &[StartupStreamingFrameReport]) -> QueuePanelReport {
    let mut inbound_updates = QueueAgeTracker::new(QueueId::InboundUpdates);
    let mut completed_results = QueueAgeTracker::new(QueueId::CompletedRenderResults);
    let mut upload_work = QueueAgeTracker::new(QueueId::UploadWork);
    let mut host_publication = QueueAgeTracker::new(QueueId::HostPublication);
    let mut render_compile_jobs = QueueAgeTracker::new(QueueId::RenderCompileJobs);

    for frame in frames {
        let now_ms = frame.elapsed_ms;
        inbound_updates.reconcile_depth(usize_to_u64(frame.server_update_queue_depth), now_ms);

        let completed = usize_to_u64(frame.completed_compile_sections);
        if completed > 0 {
            completed_results.enqueue(completed, now_ms);
            completed_results.dequeue(completed, now_ms);
        }
        completed_results
            .reconcile_depth(usize_to_u64(frame.queued_completed_compile_results), now_ms);

        let uploaded = usize_to_u64(frame.uploaded_sections + frame.upload_removed_sections);
        if uploaded > 0 {
            upload_work.enqueue(uploaded, now_ms);
            upload_work.dequeue(uploaded, now_ms);
        }

        let publication_depth = frame
            .pending_publications
            .saturating_add(frame.scheduler_pending_worldgen_publication_chunks)
            .saturating_add(frame.scheduler_pending_light_publications);
        host_publication.reconcile_depth(usize_to_u64(publication_depth), now_ms);
        render_compile_jobs
            .reconcile_depth(usize_to_u64(frame.pending_render_compile_jobs), now_ms);
    }

    let report_at_ms = frames.last().map_or(0.0, |frame| frame.elapsed_ms);
    QueuePanelReport::new(vec![
        inbound_updates.report(report_at_ms),
        completed_results.report(report_at_ms),
        upload_work.report(report_at_ms),
        host_publication.report(report_at_ms),
        render_compile_jobs.report(report_at_ms),
    ])
}

fn startup_streaming_peer_panel(frames: &[StartupStreamingFrameReport]) -> PeerThreadPanelReport {
    let final_frame = frames.last().copied().unwrap_or_default();
    let server_busy_ms = frames
        .iter()
        .map(|frame| frame.poll_server_reported_total_ms)
        .sum::<f64>();
    let max_server_pending_jobs = frames
        .iter()
        .map(|frame| frame.pending_jobs)
        .max()
        .unwrap_or(0);
    let max_worldgen_pending_jobs = frames
        .iter()
        .map(|frame| frame.scheduler_worldgen_mailbox_pending_jobs)
        .max()
        .unwrap_or(0);
    let max_light_pending_jobs = frames
        .iter()
        .map(|frame| frame.scheduler_light_mailbox_pending_statuses)
        .max()
        .unwrap_or(0);
    let max_render_compile_pending_jobs = frames
        .iter()
        .map(|frame| frame.pending_render_compile_jobs)
        .max()
        .unwrap_or(0);
    let submitted_compile_sections = frames
        .iter()
        .map(|frame| frame.submitted_compile_sections)
        .sum::<usize>();
    let completed_compile_sections = frames
        .iter()
        .map(|frame| frame.completed_compile_sections)
        .sum::<usize>();

    PeerThreadPanelReport::new(vec![
        PeerThreadActivityReport::new(PeerThreadId::ServerRunner)
            .with_pending_jobs(usize_to_u64(max_server_pending_jobs))
            .with_busy_idle_ms(Some(server_busy_ms), None),
        worker_metrics_peer_report(
            PeerThreadId::Worldgen,
            final_frame.worldgen_job_frame_metrics,
            max_worldgen_pending_jobs,
        ),
        worker_metrics_peer_report(
            PeerThreadId::LightStatus,
            final_frame.light_status_job_frame_metrics,
            max_light_pending_jobs,
        ),
        PeerThreadActivityReport::new(PeerThreadId::RenderCompileWorkers)
            .with_pending_jobs(usize_to_u64(max_render_compile_pending_jobs))
            .with_frames(
                usize_to_u64(submitted_compile_sections),
                usize_to_u64(completed_compile_sections),
            ),
    ])
}

fn worker_metrics_peer_report(
    lane: PeerThreadId,
    metrics: WorkerFrameMetrics,
    pending_jobs: usize,
) -> PeerThreadActivityReport {
    PeerThreadActivityReport::new(lane)
        .with_pending_jobs(usize_to_u64(pending_jobs))
        .with_frames(
            usize_to_u64(metrics.request_frames),
            usize_to_u64(metrics.response_frames),
        )
        .with_bytes(
            usize_to_u64(metrics.request_bytes),
            usize_to_u64(metrics.response_bytes),
        )
        .with_max_pending_frames(usize_to_u64(metrics.max_pending_frames))
        .with_request_timing_ms(
            (metrics.last_request_us > 0).then_some(micros_to_ms(metrics.last_request_us)),
            micros_to_ms(metrics.total_request_us),
            micros_to_ms(metrics.max_request_us),
        )
}

fn usize_to_u64(value: usize) -> u64 {
    value.try_into().unwrap_or(u64::MAX)
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
        let sections = runtime.cached_sections();
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
        let cached_sections = runtime.cached_sections().len();
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
    let frame_duration = Duration::from_secs_f64(1.0 / options.target_hz.max(1.0));

    let mut startup = WindowSceneStartupPump::new_local(&scene, &assets)?;
    let startup_start = Instant::now();
    let mut startup_playable_frame = 0_usize;
    let playable_step = loop {
        let frame_start = Instant::now();
        let step = startup.step(spectator.position)?;
        if step.playable_ready {
            break step;
        }
        startup_playable_frame += 1;
        if startup_playable_frame > MAX_STARTUP_STREAMING_PERF_FRAMES {
            bail!(
                "startup streaming perf did not reach playable within {} startup frames",
                MAX_STARTUP_STREAMING_PERF_FRAMES
            );
        }
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    };
    let startup_playable_ms = elapsed_ms(startup_start.elapsed());
    let startup_progress = playable_step.progress.as_ref();
    let startup_target_ready_chunks =
        startup_progress.map_or(0, |progress| progress.target_ready_chunks);
    let startup_target_chunk_count =
        startup_progress.map_or(0, |progress| progress.target_chunk_count);
    let startup_target_percent = startup_progress.map_or(0, |progress| progress.percent());
    let runtime = startup.into_runtime();
    let initial_sections = runtime.cached_sections();
    if initial_sections.is_empty() {
        bail!("startup streaming perf entered playable with no cached render sections");
    }
    let initial_index_count = initial_sections
        .iter()
        .map(|section| section.stats().index_count)
        .sum::<u32>();

    let mut frame_reports = Vec::with_capacity(options.frames);
    let mut first_full_view_ready_frame = None;
    let mut first_full_view_ready_ms = None;
    let render_options = options.render_options;
    let streaming_start = Instant::now();

    let (headless, state) = run_headless_frame_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: options.frames,
            pace_frame_duration: Some(frame_duration),
        },
        move |device, queue, format, size| {
            let depth = ChunkDepthTarget::new(device, size[0], size[1]);
            let mut draw = TexturedSectionDrawResources::new(
                device,
                queue,
                format,
                &initial_sections,
                runtime.mesh_assets().atlas.as_upload(),
            )?;
            draw.set_traversal_ready_sections(
                &runtime.traversal_ready_render_section_keys(spectator.position),
            );
            let sky =
                SkyRenderer::new_with_color_profile(device, format, render_options.color_profile);
            let actors = ActorDrawResources::new(
                device,
                queue,
                format,
                runtime.actor_textures.atlas.as_upload(),
                Some(&runtime.actor_textures.figures),
            )?;
            let asset_source = load_asset_source()?;
            let screen_effects = ScreenEffectsRenderer::new(device, queue, format, &asset_source)?;
            let gui = GuiRenderer::new(device, format);
            let render_stats = RenderStreamStats {
                section_count: draw.section_count(),
                index_count: initial_index_count,
                face_count: quad_face_count_from_indices(initial_index_count),
                ..RenderStreamStats::default()
            };
            let mut ui = GameUiHost::new();
            ui.set_screen(None);
            ui.set_scale(GuiScale::from_pixels(size[0], size[1]));
            Ok(FrameBudgetProbeState {
                runtime,
                actor_interpolation: ActorInterpolationState::new(),
                depth,
                sky,
                draw,
                actors,
                screen_effects,
                gui,
                ui,
                render_stats,
                frame_accounting: None,
            })
        },
        |index, frame, state| {
            let frame_start = Instant::now();
            let frame_deadline = frame_start + frame_duration;
            let mut report = StartupStreamingFrameReport {
                elapsed_ms: elapsed_ms(streaming_start.elapsed()),
                ..StartupStreamingFrameReport::default()
            };

            let poll_start = Instant::now();
            let _runtime_changed = state.runtime.poll()?;
            report.poll_ms = elapsed_ms(poll_start.elapsed());
            let poll_diagnostics = state.runtime.last_poll_diagnostics();
            report.poll_server_reported_total_ms = poll_diagnostics.server_reported_total_ms;
            report.poll_scheduler_publish_completed_ms =
                poll_diagnostics.scheduler_publish_completed_ms;
            report.scheduler_adaptive_publication_budget_enabled =
                poll_diagnostics.scheduler_adaptive_publication_budget_enabled;
            report.scheduler_feature_publish_budget_max_units =
                poll_diagnostics.scheduler_feature_publish_budget_max_units;
            report.scheduler_feature_publish_budget_ms =
                poll_diagnostics.scheduler_feature_publish_budget_ms;
            report.scheduler_feature_publish_spent_units =
                poll_diagnostics.scheduler_feature_publish_spent_units;
            report.scheduler_feature_publish_spent_ms =
                poll_diagnostics.scheduler_feature_publish_spent_ms;
            report.scheduler_feature_publish_estimated_unit_ms =
                poll_diagnostics.scheduler_feature_publish_estimated_unit_ms;
            report.scheduler_light_publish_budget_max_units =
                poll_diagnostics.scheduler_light_publish_budget_max_units;
            report.scheduler_light_publish_budget_ms =
                poll_diagnostics.scheduler_light_publish_budget_ms;
            report.scheduler_light_publish_spent_units =
                poll_diagnostics.scheduler_light_publish_spent_units;
            report.scheduler_light_publish_spent_ms =
                poll_diagnostics.scheduler_light_publish_spent_ms;
            report.scheduler_light_publish_estimated_unit_ms =
                poll_diagnostics.scheduler_light_publish_estimated_unit_ms;
            report.scheduler_pending_worldgen_publication_chunk_limit =
                poll_diagnostics.scheduler_pending_worldgen_publication_chunk_limit;
            report.poll_apply_updates_ms = poll_diagnostics.apply_updates_ms;
            report.poll_dirty_mark_ms = poll_diagnostics.dirty_mark_ms;
            report.poll_client_apply_updates_ms = poll_diagnostics.client_apply_updates_ms;
            report.poll_producer_read_ms = poll_diagnostics.producer_read_ms;
            report.poll_producer_decode_ms = poll_diagnostics.producer_decode_ms;
            report.poll_producer_response_sequence = poll_diagnostics.producer_response_sequence;
            report.update_pump_stalled = poll_diagnostics.update_pump_stalled;
            report.server_update_queue_depth = poll_diagnostics.server_update_queue_depth;
            report.server_update_queue_bytes = poll_diagnostics.server_update_queue_bytes;
            report.server_update_oldest_applied_age_ms =
                poll_diagnostics.server_update_oldest_applied_age_ms;
            report.scheduler_completed_feature_jobs_drained =
                poll_diagnostics.scheduler_completed_feature_jobs_drained;
            report.scheduler_feature_chunks_published =
                poll_diagnostics.scheduler_feature_chunks_published;
            report.scheduler_feature_chunks_skipped =
                poll_diagnostics.scheduler_feature_chunks_skipped;
            report.scheduler_feature_jobs_completed =
                poll_diagnostics.scheduler_feature_jobs_completed;
            report.scheduler_feature_snapshot_ready_events =
                poll_diagnostics.scheduler_feature_snapshot_ready_events;
            report.scheduler_light_status_batches_enqueued =
                poll_diagnostics.scheduler_light_status_batches_enqueued;
            report.scheduler_completed_light_statuses_drained =
                poll_diagnostics.scheduler_completed_light_statuses_drained;
            report.scheduler_light_statuses_published =
                poll_diagnostics.scheduler_light_statuses_published;
            report.scheduler_light_statuses_skipped =
                poll_diagnostics.scheduler_light_statuses_skipped;
            report.scheduler_light_snapshot_ready_events =
                poll_diagnostics.scheduler_light_snapshot_ready_events;
            report.scheduler_pending_worldgen_publication_jobs =
                poll_diagnostics.scheduler_pending_worldgen_publication_jobs;
            report.scheduler_pending_worldgen_publication_chunks =
                poll_diagnostics.scheduler_pending_worldgen_publication_chunks;
            report.scheduler_pending_light_publications =
                poll_diagnostics.scheduler_pending_light_publications;
            report.scheduler_worldgen_mailbox_pending_jobs =
                poll_diagnostics.scheduler_worldgen_mailbox_pending_jobs;
            report.scheduler_light_mailbox_pending_statuses =
                poll_diagnostics.scheduler_light_mailbox_pending_statuses;
            report.runner_frame_metrics = poll_diagnostics.runner_frame_metrics;
            report.worldgen_job_frame_metrics = poll_diagnostics.worldgen_job_frame_metrics;
            report.light_status_job_frame_metrics = poll_diagnostics.light_status_job_frame_metrics;

            if state.runtime.has_pending_render_work(spectator.position) {
                let update =
                    probe_sync_upload_sections(&frame, state, spectator.position, frame_deadline)?;
                report.remesh_ms = update.remesh_ms;
                report.upload_ms = update.upload_ms;
                report.section_sync_timing = update.sync_timing;
                report.submitted_compile_sections = update.submitted_compile_sections;
                report.accepted_compile_results = update.accepted_compile_results;
                report.queued_completed_compile_results = update.queued_completed_compile_results;
                report.completed_compile_sections = update.completed_compile_sections;
                report.uploaded_sections = update.uploaded_sections;
                report.upload_removed_sections = update.upload_removed_sections;
                report.target_rebuilt_sections = update.target_rebuilt_sections;
                report.non_target_rebuilt_sections = update.non_target_rebuilt_sections;
                report.target_removed_sections = update.target_removed_sections;
                report.non_target_removed_sections = update.non_target_removed_sections;
                report.deadline_skipped_compile_requests = update.deadline_skipped_compile_requests;
            }

            let camera = spectator.camera(state.runtime.render_distance());
            let underwater_overlay =
                state
                    .runtime
                    .camera_inside_water(spectator.position)
                    .then(|| {
                        UnderwaterOverlay::vanilla_from_native_camera(
                            spectator.yaw,
                            spectator.pitch,
                        )
                    });
            let sky_clear_color = state.runtime.sky_clear_color();
            let time_of_day = state.runtime.time_of_day();
            let sun_angle = state.runtime.sun_angle();
            state
                .actor_interpolation
                .reconcile_authoritative(state.runtime.client().actor_presentations());
            state.actor_interpolation.step(
                (1.0 / options.target_hz.max(1.0)) as f32,
                ActorInterpolationConfig::default(),
            );
            let actor_instances = actor_instances_from_presentations(
                &state.actor_interpolation.presentations(),
                state.runtime.client(),
            );
            state.draw.set_traversal_ready_sections(
                &state
                    .runtime
                    .traversal_ready_render_section_keys(spectator.position),
            );
            let ui_render_state = game_ui_render_state(FlatClientUiRenderOptions {
                render_distance: state.runtime.render_distance() as i32,
                render_options,
                far_lod_enabled: false,
                far_lod_range_chunks:
                    mclone_app_runtime::far_lod::DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
                frame_pacing: FramePacingUiState::default(),
                movement_mode: GameMovementMode::Walk,
                collision_mode: GameCollisionMode::Normal,
                travel_assist_mode: GameTravelAssistMode::Off,
                fly_speed_multiplier: 1.0,
                movement_speed_multiplier: 1.0,
                player_collision_box_visible: false,
                first_person_player_visible: false,
                crosshair_visible: true,
                frame_pipeline_overlay_visible: false,
                debug_diagnostics_visible: false,
                player_model: Default::default(),
                server_cadence: None,
            });
            let gui_scale = state.ui.scale();
            let gui_state = FullFrameGui::new(
                state.ui.is_active(),
                state.ui.covers_world(),
                [gui_scale.width, gui_scale.height],
            );
            let ui_draw = state.ui.render_draw_list(ui_render_state);
            let render_start = Instant::now();
            let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
            render_full_frame_for_view(
                frame,
                &state.depth,
                &state.sky,
                &mut state.draw,
                Some(&mut state.actors),
                Some(&mut state.screen_effects),
                Some(&mut state.gui),
                render_view,
                &actor_instances,
                underwater_overlay,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                gui_state,
                |_| ui_draw,
                &mut state.render_stats,
            )?;
            report.render_ms = elapsed_ms(render_start.elapsed());

            let view_progress = state.runtime.view_readiness_overlay();
            report.target_ready_chunks = view_progress
                .as_ref()
                .map_or(0, |progress| progress.target_ready_chunks);
            report.target_chunk_count = view_progress
                .as_ref()
                .map_or(0, |progress| progress.target_chunk_count);
            report.target_percent = view_progress
                .as_ref()
                .map_or(0, |progress| progress.percent());
            let stats = state.runtime.stats();
            report.loaded_chunks = stats.loaded_chunks;
            report.cached_sections = state.draw.section_count();
            report.pending_jobs = stats.pending_jobs;
            report.pending_publications = stats.pending_publications;
            report.pending_render_chunks = stats.pending_render_chunks;
            report.ready_render_work_pending =
                state.runtime.has_pending_render_work(spectator.position);
            report.pending_render_compile_jobs = stats.pending_render_compile_jobs;
            report.inflight_render_sections = stats.inflight_render_sections;
            let target_render_work = state.runtime.target_render_work_stats(spectator.position);
            report.target_pending_render_chunks = target_render_work.pending_render_chunks;
            report.target_ready_render_work_pending = target_render_work.ready_render_work_pending;
            report.target_inflight_render_sections = target_render_work.inflight_render_sections;

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
    let budget_decision_panel = state
        .runtime
        .last_poll_diagnostics()
        .scheduler_budget_decision_panel;

    Ok(StartupStreamingPerfReport {
        options: options.clone(),
        world_dir,
        prewarm,
        asset_load_ms,
        startup_playable_frame,
        startup_playable_ms,
        startup_cached_sections: playable_step.cached_section_count,
        startup_target_ready_chunks,
        startup_target_chunk_count,
        startup_target_percent,
        streaming_wall_ms: elapsed_ms(streaming_start.elapsed()),
        headless,
        frames: frame_reports,
        budget_decision_panel,
        first_full_view_ready_frame,
        first_full_view_ready_ms,
        first_render_quiescent_frame,
        first_render_quiescent_ms,
        first_target_render_quiescent_frame,
        first_target_render_quiescent_ms,
    })
}

pub(crate) fn run_timedemo(options: &TimedemoOptions) -> Result<TimedemoReport> {
    if options.scene.remote_addr.is_some() {
        bail!("--timedemo currently requires the local integrated server path");
    }
    let loaded_scene = timedemo_loaded_scene(options)?;
    let scene_start = Instant::now();
    let scene_mesh = build_scene_textured_sections(&loaded_scene)?;
    let scene_build_ms = elapsed_ms(scene_start.elapsed());
    let cameras = timedemo_cameras(&options.scene, options.path_radius_chunks, options.frames);
    let render = run_headless_textured_sections_timedemo(
        HeadlessTimedemoOptions {
            width: options.width,
            height: options.height,
            color: mclone_render::default_clear_color(),
            cameras,
            render_options: options.render_options,
        },
        &scene_mesh.sections,
        scene_mesh.atlas.as_upload(),
    )?;

    let vertex_count = scene_mesh
        .sections
        .iter()
        .map(|section| section.stats().vertex_count)
        .sum();
    let index_count = scene_mesh
        .sections
        .iter()
        .map(|section| section.stats().index_count)
        .sum();
    Ok(TimedemoReport {
        options: options.clone(),
        loaded_render_distance: loaded_scene.render_distance,
        visibility_graph_stats: scene_mesh.visibility_graph_stats,
        scene_build_ms,
        section_count: scene_mesh.sections.len(),
        vertex_count,
        face_count: quad_face_count_from_indices(index_count),
        index_count,
        render,
    })
}

fn probe_sync_upload_sections(
    frame: &RenderFrameContext<'_>,
    state: &mut FrameBudgetProbeState,
    camera_position: Vec3,
    deadline: Instant,
) -> Result<FrameBudgetProbeSectionTiming> {
    let remesh_start = Instant::now();
    let timed_section_update = state
        .runtime
        .sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
            camera_position,
            deadline,
            None,
        )?;
    let sync_timing = timed_section_update.timing;
    let section_update = timed_section_update.cache_update;
    let remesh_ms = elapsed_ms(remesh_start.elapsed());
    let upload_start = Instant::now();
    let upload_report = state
        .draw
        .apply_section_updates(
            frame.device,
            &section_update.rebuilt_sections,
            &section_update.removed_section_keys,
        )
        .context("failed to upload frame-budget probe section updates")?;
    let upload_ms = elapsed_ms(upload_start.elapsed());
    let runtime_stats = state.runtime.stats();
    let target_rebuilt_sections = count_target_section_keys(
        section_update
            .rebuilt_sections
            .iter()
            .map(|section| section.key),
        runtime_stats.interest_center,
        runtime_stats.render_distance,
    );
    let target_removed_sections = count_target_section_keys(
        section_update.removed_section_keys.iter().copied(),
        runtime_stats.interest_center,
        runtime_stats.render_distance,
    );
    let rebuilt_section_count = section_update.rebuilt_section_count();
    let removed_section_count = section_update.removed_section_count();
    state
        .runtime
        .release_render_compile_jobs(section_update.accepted_compile_result_count);

    state.render_stats.section_count = state.draw.section_count();
    state.render_stats.index_count = state.draw.index_count();
    state.render_stats.face_count = quad_face_count_from_indices(state.render_stats.index_count);
    state.render_stats.drawn_section_count = 0;
    state.render_stats.drawn_face_count = 0;
    state.render_stats.drawn_index_count = 0;
    record_render_section_update_stats(&mut state.render_stats, &section_update, upload_report);
    state.render_stats.last_remesh_ms = remesh_ms;
    state.render_stats.last_upload_ms = upload_ms;

    Ok(FrameBudgetProbeSectionTiming {
        remesh_ms,
        upload_ms,
        sync_timing,
        rebuilt_sections: section_update.rebuilt_section_count(),
        removed_sections: section_update.removed_section_count(),
        submitted_compile_sections: section_update.submitted_compile_section_count,
        deadline_skipped_compile_requests: section_update.deadline_skipped_compile_request_count,
        accepted_compile_results: section_update.accepted_compile_result_count,
        queued_completed_compile_results: section_update.queued_completed_compile_result_count,
        completed_compile_sections: section_update.completed_compile_section_count,
        stale_compile_sections: section_update.stale_compile_section_count,
        uploaded_sections: upload_report.uploaded_section_count,
        upload_removed_sections: upload_report.removed_section_count,
        target_rebuilt_sections,
        non_target_rebuilt_sections: rebuilt_section_count.saturating_sub(target_rebuilt_sections),
        target_removed_sections,
        non_target_removed_sections: removed_section_count.saturating_sub(target_removed_sections),
        uploaded_vertices: upload_report.uploaded_vertex_count,
        uploaded_indices: upload_report.uploaded_index_count,
    })
}

pub(crate) fn run_frame_budget_probe(
    options: &FrameBudgetProbeOptions,
) -> Result<FrameBudgetProbeReport> {
    if options.scene.remote_addr.is_some() {
        bail!("frame-budget probes currently require the local integrated server path");
    }

    let runtime_setup_start = Instant::now();
    let initial_spectator = frame_budget_probe_spectator(options, 0);
    let initial_center = initial_spectator.chunk_pos();
    let mut runtime_scene = options.scene.clone();
    runtime_scene.chunk_x = initial_center.x;
    runtime_scene.chunk_z = initial_center.z;
    let mut runtime = WindowSceneRuntime::new(&runtime_scene)?;
    let (initial_poll_count, initial_poll_ms) = poll_window_runtime_until_idle(&mut runtime)?;
    let initial_remesh_start = Instant::now();
    let initial_update = runtime.sync_all_render_sections(initial_spectator.position)?;
    let initial_sections = runtime.cached_sections();
    let initial_remesh_ms = elapsed_ms(initial_remesh_start.elapsed());
    if initial_sections.is_empty() {
        bail!(
            "frame-budget probe seed={} center=({}, {}) render_distance={} produced no initial render sections",
            options.scene.seed,
            options.scene.chunk_x,
            options.scene.chunk_z,
            options.scene.render_distance
        );
    }
    let initial_index_count = initial_sections
        .iter()
        .map(|section| section.stats().index_count)
        .sum::<u32>();
    let initial_section_count = initial_sections.len();
    let initial_face_count = quad_face_count_from_indices(initial_index_count);
    let runtime_setup_ms = elapsed_ms(runtime_setup_start.elapsed());

    let mut frame_reports = Vec::with_capacity(options.frames);
    let probe_options = options.clone();
    let render_options = options.render_options;
    let initial_upload = TexturedSectionUploadReport {
        uploaded_section_count: initial_update.rebuilt_section_count(),
        removed_section_count: initial_update.removed_section_count(),
        uploaded_vertex_count: initial_update.rebuilt_vertex_count,
        uploaded_index_count: initial_update.rebuilt_index_count,
    };

    let (headless, _state) = run_headless_frame_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: options.frames,
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let depth = ChunkDepthTarget::new(device, size[0], size[1]);
            let draw = TexturedSectionDrawResources::new(
                device,
                queue,
                format,
                &initial_sections,
                runtime.mesh_assets().atlas.as_upload(),
            )?;
            let mut draw = draw;
            draw.set_traversal_ready_sections(
                &runtime.traversal_ready_render_section_keys(initial_spectator.position),
            );
            let sky =
                SkyRenderer::new_with_color_profile(device, format, render_options.color_profile);
            let actors = ActorDrawResources::new(
                device,
                queue,
                format,
                runtime.actor_textures.atlas.as_upload(),
                Some(&runtime.actor_textures.figures),
            )?;
            let asset_source = load_asset_source()?;
            let screen_effects = ScreenEffectsRenderer::new(device, queue, format, &asset_source)?;
            let gui = GuiRenderer::new(device, format);
            let mut render_stats = RenderStreamStats {
                section_count: draw.section_count(),
                index_count: draw.index_count(),
                face_count: quad_face_count_from_indices(draw.index_count()),
                ..RenderStreamStats::default()
            };
            record_render_section_update_stats(&mut render_stats, &initial_update, initial_upload);
            let mut ui = GameUiHost::new();
            ui.set_screen(None);
            ui.set_scale(GuiScale::from_pixels(size[0], size[1]));
            Ok(FrameBudgetProbeState {
                runtime,
                actor_interpolation: ActorInterpolationState::new(),
                depth,
                sky,
                draw,
                actors,
                screen_effects,
                gui,
                ui,
                render_stats,
                frame_accounting: options.frame_accounting_enabled.then(|| {
                    FrameAccumulator::new(headless_frame_accounting_config(target_frame_ms(
                        options.target_hz,
                    )))
                }),
            })
        },
        |index, frame, state| {
            let frame_start = Instant::now();
            let frame_deadline =
                frame_start + Duration::from_secs_f64(1.0 / probe_options.target_hz.max(1.0));
            let spectator = frame_budget_probe_spectator(&probe_options, index);
            let center = spectator.chunk_pos();
            let mut report = FrameBudgetProbeFrameReport {
                index,
                center,
                ..FrameBudgetProbeFrameReport::default()
            };

            let set_interest_start = Instant::now();
            report.interest_center_changed = state.runtime.interest_center() != center;
            report.interest_updates_changed = state.runtime.set_interest_center(center)?;
            report.set_interest_ms = elapsed_ms(set_interest_start.elapsed());
            let poll_start = Instant::now();
            report.runtime_changed = state.runtime.poll()?;
            report.poll_ms = elapsed_ms(poll_start.elapsed());
            let poll_diagnostics = state.runtime.last_poll_diagnostics();
            report.poll_flush_commands_ms = poll_diagnostics.flush_commands_ms;
            report.poll_server_tick_ms = poll_diagnostics.server_tick_ms;
            report.poll_server_reported_total_ms = poll_diagnostics.server_reported_total_ms;
            report.poll_scheduler_tick_ms = poll_diagnostics.scheduler_tick_ms;
            report.poll_scheduler_report_ms = poll_diagnostics.scheduler_report_ms;
            report.poll_scheduler_purge_stale_tickets_ms =
                poll_diagnostics.scheduler_purge_stale_tickets_ms;
            report.poll_scheduler_reconcile_holders_ms =
                poll_diagnostics.scheduler_reconcile_holders_ms;
            report.poll_scheduler_publish_completed_ms =
                poll_diagnostics.scheduler_publish_completed_ms;
            report.poll_scheduler_adaptive_publication_budget_enabled =
                poll_diagnostics.scheduler_adaptive_publication_budget_enabled;
            report.poll_scheduler_feature_publish_budget_max_units =
                poll_diagnostics.scheduler_feature_publish_budget_max_units;
            report.poll_scheduler_feature_publish_budget_ms =
                poll_diagnostics.scheduler_feature_publish_budget_ms;
            report.poll_scheduler_feature_publish_spent_units =
                poll_diagnostics.scheduler_feature_publish_spent_units;
            report.poll_scheduler_feature_publish_spent_ms =
                poll_diagnostics.scheduler_feature_publish_spent_ms;
            report.poll_scheduler_feature_publish_estimated_unit_ms =
                poll_diagnostics.scheduler_feature_publish_estimated_unit_ms;
            report.poll_scheduler_light_publish_budget_max_units =
                poll_diagnostics.scheduler_light_publish_budget_max_units;
            report.poll_scheduler_light_publish_budget_ms =
                poll_diagnostics.scheduler_light_publish_budget_ms;
            report.poll_scheduler_light_publish_spent_units =
                poll_diagnostics.scheduler_light_publish_spent_units;
            report.poll_scheduler_light_publish_spent_ms =
                poll_diagnostics.scheduler_light_publish_spent_ms;
            report.poll_scheduler_light_publish_estimated_unit_ms =
                poll_diagnostics.scheduler_light_publish_estimated_unit_ms;
            report.poll_scheduler_pending_worldgen_publication_chunk_limit =
                poll_diagnostics.scheduler_pending_worldgen_publication_chunk_limit;
            report.poll_scheduler_pending_unload_ms = poll_diagnostics.scheduler_pending_unload_ms;
            report.poll_scheduler_apply_events_ms = poll_diagnostics.scheduler_apply_events_ms;
            report.poll_scheduler_completed_feature_jobs_drained =
                poll_diagnostics.scheduler_completed_feature_jobs_drained;
            report.poll_scheduler_feature_chunks_published =
                poll_diagnostics.scheduler_feature_chunks_published;
            report.poll_scheduler_feature_chunks_skipped =
                poll_diagnostics.scheduler_feature_chunks_skipped;
            report.poll_scheduler_feature_jobs_completed =
                poll_diagnostics.scheduler_feature_jobs_completed;
            report.poll_scheduler_feature_snapshot_ready_events =
                poll_diagnostics.scheduler_feature_snapshot_ready_events;
            report.poll_scheduler_light_status_batches_enqueued =
                poll_diagnostics.scheduler_light_status_batches_enqueued;
            report.poll_scheduler_completed_light_statuses_drained =
                poll_diagnostics.scheduler_completed_light_statuses_drained;
            report.poll_scheduler_light_statuses_published =
                poll_diagnostics.scheduler_light_statuses_published;
            report.poll_scheduler_light_statuses_skipped =
                poll_diagnostics.scheduler_light_statuses_skipped;
            report.poll_scheduler_light_snapshot_ready_events =
                poll_diagnostics.scheduler_light_snapshot_ready_events;
            report.poll_scheduler_pending_worldgen_publication_jobs =
                poll_diagnostics.scheduler_pending_worldgen_publication_jobs;
            report.poll_scheduler_pending_worldgen_publication_chunks =
                poll_diagnostics.scheduler_pending_worldgen_publication_chunks;
            report.poll_scheduler_pending_light_publications =
                poll_diagnostics.scheduler_pending_light_publications;
            report.poll_scheduler_worldgen_mailbox_pending_jobs =
                poll_diagnostics.scheduler_worldgen_mailbox_pending_jobs;
            report.poll_scheduler_light_mailbox_pending_statuses =
                poll_diagnostics.scheduler_light_mailbox_pending_statuses;
            report.poll_block_tick_ms = poll_diagnostics.block_tick_ms;
            report.poll_fluid_tick_ms = poll_diagnostics.fluid_tick_ms;
            report.poll_fluid_event_apply_ms = poll_diagnostics.fluid_event_apply_ms;
            report.poll_fluid_due_scan_ms = poll_diagnostics.fluid_due_scan_ms;
            report.poll_fluid_remove_due_ms = poll_diagnostics.fluid_remove_due_ms;
            report.poll_fluid_tick_fluid_ms = poll_diagnostics.fluid_tick_fluid_ms;
            report.poll_fluid_set_block_ms = poll_diagnostics.fluid_set_block_ms;
            report.poll_entity_tick_ms = poll_diagnostics.entity_tick_ms;
            report.poll_apply_updates_ms = poll_diagnostics.apply_updates_ms;
            report.poll_dirty_mark_ms = poll_diagnostics.dirty_mark_ms;
            report.poll_client_apply_updates_ms = poll_diagnostics.client_apply_updates_ms;
            report.poll_producer_read_ms = poll_diagnostics.producer_read_ms;
            report.poll_producer_decode_ms = poll_diagnostics.producer_decode_ms;
            report.poll_producer_response_sequence = poll_diagnostics.producer_response_sequence;
            report.update_pump_stalled = poll_diagnostics.update_pump_stalled;
            report.update_pump_stall_count = poll_diagnostics.update_pump_stall_count;
            report.server_update_queue_depth = poll_diagnostics.server_update_queue_depth;
            report.server_update_queue_bytes = poll_diagnostics.server_update_queue_bytes;
            report.server_update_applied_bytes = poll_diagnostics.server_update_applied_bytes;
            report.server_update_oldest_applied_age_ms =
                poll_diagnostics.server_update_oldest_applied_age_ms;
            report.poll_scheduler_events = poll_diagnostics.scheduler_events;
            report.poll_updates = poll_diagnostics.updates;
            report.poll_snapshot_updates = poll_diagnostics.snapshot_updates;
            report.poll_section_block_updates = poll_diagnostics.section_block_updates;
            report.poll_unload_updates = poll_diagnostics.unload_updates;
            report.poll_pending_unloads_processed = poll_diagnostics.pending_unloads_processed;
            report.poll_fluid_due_ticks = poll_diagnostics.fluid_due_ticks;
            report.poll_fluid_executed_ticks = poll_diagnostics.fluid_executed_ticks;
            report.poll_fluid_deferred_ticks = poll_diagnostics.fluid_deferred_ticks;
            report.poll_fluid_mutated_blocks = poll_diagnostics.fluid_mutated_blocks;
            report.poll_fluid_snapshot_events = poll_diagnostics.fluid_snapshot_events;
            report.poll_fluid_event_count = poll_diagnostics.fluid_event_count;
            report.poll_scheduled_fluid_ticks = poll_diagnostics.scheduled_fluid_ticks;
            if state.runtime.has_pending_render_work(spectator.position) {
                let update =
                    probe_sync_upload_sections(&frame, state, spectator.position, frame_deadline)?;
                report.add_section_timing(update);
            }

            let camera = spectator.camera(state.runtime.render_distance());
            let underwater_overlay =
                state
                    .runtime
                    .camera_inside_water(spectator.position)
                    .then(|| {
                        UnderwaterOverlay::vanilla_from_native_camera(
                            spectator.yaw,
                            spectator.pitch,
                        )
                    });
            let sky_clear_color = state.runtime.sky_clear_color();
            let time_of_day = state.runtime.time_of_day();
            let sun_angle = state.runtime.sun_angle();
            state
                .actor_interpolation
                .reconcile_authoritative(state.runtime.client().actor_presentations());
            state.actor_interpolation.step(
                (1.0 / probe_options.target_hz.max(1.0)) as f32,
                ActorInterpolationConfig::default(),
            );
            let actor_instances = actor_instances_from_presentations(
                &state.actor_interpolation.presentations(),
                state.runtime.client(),
            );
            state.draw.set_traversal_ready_sections(
                &state
                    .runtime
                    .traversal_ready_render_section_keys(spectator.position),
            );
            let ui_render_state = game_ui_render_state(FlatClientUiRenderOptions {
                render_distance: state.runtime.render_distance() as i32,
                render_options,
                far_lod_enabled: false,
                far_lod_range_chunks:
                    mclone_app_runtime::far_lod::DEFAULT_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
                frame_pacing: FramePacingUiState::default(),
                movement_mode: GameMovementMode::Walk,
                collision_mode: GameCollisionMode::Normal,
                travel_assist_mode: GameTravelAssistMode::Off,
                fly_speed_multiplier: 1.0,
                movement_speed_multiplier: 1.0,
                player_collision_box_visible: false,
                first_person_player_visible: false,
                crosshair_visible: true,
                frame_pipeline_overlay_visible: false,
                debug_diagnostics_visible: false,
                player_model: Default::default(),
                server_cadence: None,
            });
            let gui_scale = state.ui.scale();
            let gui_state = FullFrameGui::new(
                state.ui.is_active(),
                state.ui.covers_world(),
                [gui_scale.width, gui_scale.height],
            );
            let ui_draw = state.ui.render_draw_list(ui_render_state);
            let render_start = Instant::now();
            let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
            render_full_frame_for_view(
                frame,
                &state.depth,
                &state.sky,
                &mut state.draw,
                Some(&mut state.actors),
                Some(&mut state.screen_effects),
                Some(&mut state.gui),
                render_view,
                &actor_instances,
                underwater_overlay,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                gui_state,
                |_| ui_draw,
                &mut state.render_stats,
            )?;
            report.render_ms = elapsed_ms(render_start.elapsed());

            let stats = state.runtime.stats();
            report.loaded_chunks = stats.loaded_chunks;
            report.pending_jobs = stats.pending_jobs;
            report.pending_publications = stats.pending_publications;
            report.pending_render_chunks = stats.pending_render_chunks;
            report.pending_render_compile_jobs = stats.pending_render_compile_jobs;
            report.max_pending_render_compile_jobs =
                state.runtime.render_compile_max_pending_job_count();
            report.available_render_compile_slots =
                state.runtime.render_compile_available_pending_job_slots();
            report.inflight_render_sections = stats.inflight_render_sections;
            report.drawn_sections = state.render_stats.drawn_section_count;
            report.drawn_indices = state.render_stats.drawn_index_count;
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

    let frame_accounting = _state
        .frame_accounting
        .as_ref()
        .map(FrameAccumulator::summary_report);
    Ok(FrameBudgetProbeReport {
        options: options.clone(),
        runtime_setup_ms,
        initial_poll_count,
        initial_poll_ms,
        initial_remesh_ms,
        initial_section_count,
        initial_face_count,
        initial_index_count,
        headless,
        frame_accounting,
        frames: frame_reports,
    })
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
