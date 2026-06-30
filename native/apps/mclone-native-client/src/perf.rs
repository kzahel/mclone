use std::time::Instant;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_render::{
    FullFrameGui, RenderStreamStats, record_render_section_update_stats, render_full_frame_for_view,
};
use mclone_client::{ActorInterpolationConfig, ActorInterpolationState};
use mclone_core::{CHUNK_WIDTH, ChunkPos};
use mclone_mesh::{VisibilityGraphBuildStats, quad_face_count_from_indices};
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
use mclone_render_session::actor_instances_from_presentations;
use mclone_ui::{GameMovementMode, GameUi, GuiScale};

use crate::camera::{
    SPECTATOR_BASE_SPEED, SPECTATOR_MAX_SPEED, SPECTATOR_MIN_SPEED, SpectatorCamera,
};
use crate::cli::{
    FrameBudgetProbeMode, FrameBudgetProbeOptions, MovementPerfOptions, SceneOptions,
    TimedemoOptions,
};
use crate::flat_client_driver::{FlatClientUiRenderOptions, game_ui_render_state};
use crate::frame_pacing::{FramePacingUiState, elapsed_ms};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{
    WindowSceneRuntime, build_scene_textured_sections, chunk_tracking_radius_for_render_distance,
    poll_window_runtime_until_idle, square_count,
};
use crate::{MAX_RENDER_DISTANCE, print_benchmark_metadata};

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
        for step in &self.steps {
            if step.loaded_chunks != expected_tracked_chunks {
                bail!(
                    "movement step {} loaded_chunks={} expected {expected_tracked_chunks}",
                    step.index,
                    step.loaded_chunks
                );
            }
            if step.client_visible_chunks != expected_tracked_chunks {
                bail!(
                    "movement step {} client_visible_chunks={} expected {expected_tracked_chunks}",
                    step.index,
                    step.client_visible_chunks
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
    frames: Vec<FrameBudgetProbeFrameReport>,
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
    poll_scheduler_pending_unload_ms: f64,
    poll_scheduler_apply_events_ms: f64,
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
    rebuilt_sections: usize,
    removed_sections: usize,
    submitted_compile_sections: usize,
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
            poll_scheduler_pending_unload_ms: 0.0,
            poll_scheduler_apply_events_ms: 0.0,
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
            rebuilt_sections: 0,
            removed_sections: 0,
            submitted_compile_sections: 0,
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
        self.rebuilt_sections += timing.rebuilt_sections;
        self.removed_sections += timing.removed_sections;
        self.submitted_compile_sections += timing.submitted_compile_sections;
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
    ui: GameUi,
    render_stats: RenderStreamStats,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameBudgetProbeSectionTiming {
    remesh_ms: f64,
    upload_ms: f64,
    rebuilt_sections: usize,
    removed_sections: usize,
    submitted_compile_sections: usize,
    completed_compile_sections: usize,
    stale_compile_sections: usize,
    uploaded_sections: usize,
    upload_removed_sections: usize,
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
        Ok(())
    }

    pub(crate) fn print_json(&self) {
        let target_frame_ms = self.target_frame_ms();
        let over_budget = frame_budget_over_count(&self.headless.frames, target_frame_ms, 1.0);
        let over_2x = frame_budget_over_count(&self.headless.frames, target_frame_ms, 2.0);
        let over_4x = frame_budget_over_count(&self.headless.frames, target_frame_ms, 4.0);
        let p95 = frame_budget_percentile_ms(&self.headless.frames, 0.95);
        let p99 = frame_budget_percentile_ms(&self.headless.frames, 0.99);
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
            "  \"movement_speed_blocks_per_sec\": {:.3},",
            self.options.movement_speed
        );
        println!("  \"over_budget_frames\": {},", over_budget);
        println!("  \"over_2x_budget_frames\": {},", over_2x);
        println!("  \"over_4x_budget_frames\": {},", over_4x);
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

fn target_frame_ms(target_hz: f64) -> f64 {
    1000.0 / target_hz.max(1.0)
}

fn frame_budget_over_count(
    frames: &[mclone_render::headless::HeadlessFrameLoopTiming],
    target_frame_ms: f64,
    multiplier: f64,
) -> usize {
    let threshold = target_frame_ms * multiplier;
    frames
        .iter()
        .filter(|frame| frame.frame_ms > threshold)
        .count()
}

fn frame_budget_percentile_ms(
    frames: &[mclone_render::headless::HeadlessFrameLoopTiming],
    percentile: f64,
) -> f64 {
    if frames.is_empty() {
        return 0.0;
    }
    let mut values = frames
        .iter()
        .map(|frame| frame.frame_ms)
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let clamped = percentile.clamp(0.0, 1.0);
    let index = ((values.len() - 1) as f64 * clamped).ceil() as usize;
    values[index.min(values.len() - 1)]
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
) -> Result<FrameBudgetProbeSectionTiming> {
    let remesh_start = Instant::now();
    let section_update = state.runtime.sync_render_sections(camera_position)?;
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
        rebuilt_sections: section_update.rebuilt_section_count(),
        removed_sections: section_update.removed_section_count(),
        submitted_compile_sections: section_update.submitted_compile_section_count,
        completed_compile_sections: section_update.completed_compile_section_count,
        stale_compile_sections: section_update.stale_compile_section_count,
        uploaded_sections: upload_report.uploaded_section_count,
        upload_removed_sections: upload_report.removed_section_count,
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
            let mut ui = GameUi::new();
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
            })
        },
        |index, frame, state| {
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
            report.poll_scheduler_pending_unload_ms = poll_diagnostics.scheduler_pending_unload_ms;
            report.poll_scheduler_apply_events_ms = poll_diagnostics.scheduler_apply_events_ms;
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
                let update = probe_sync_upload_sections(&frame, state, spectator.position)?;
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
                frame_pacing: FramePacingUiState::default(),
                movement_mode: GameMovementMode::Walk,
                fly_speed_multiplier: 1.0,
                movement_speed_multiplier: 1.0,
                player_collision_box_visible: false,
                first_person_player_visible: false,
                player_model: Default::default(),
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
            report.inflight_render_sections = stats.inflight_render_sections;
            report.drawn_sections = state.render_stats.drawn_section_count;
            report.drawn_indices = state.render_stats.drawn_index_count;
            frame_reports.push(report);
            Ok(())
        },
    )?;

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
    fn frame_budget_helpers_count_and_percentile_frame_times() {
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
        assert_eq!(frame_budget_over_count(&frames, 8.0, 1.0), 3);
        assert_eq!(frame_budget_over_count(&frames, 8.0, 2.0), 2);
        assert_eq!(frame_budget_over_count(&frames, 8.0, 4.0), 1);
        assert_eq!(frame_budget_percentile_ms(&frames, 0.95), 35.0);
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
