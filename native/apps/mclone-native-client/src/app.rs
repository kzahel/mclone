use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use glam::Vec2;
use mclone_app_runtime::frame_render::{MIN_FLAT_RENDER_SCALE, scaled_frame_size};
use mclone_app_runtime::input_preferences::ClientInputPreferences;
use mclone_app_runtime::{DEFAULT_STARTUP_READINESS_TIMEOUT, RuntimePollDiagnostics};
use mclone_assets::AssetSource;
use mclone_diagnostics::{GpuPassId, GpuTimestampPanelReport};
use mclone_input::{
    ControllerInputPreferences, InputCapabilities, InputCapabilityState, InputDeviceKind,
    InputPreferences, KeyboardKey, MouseWheelDirection, PointerButton, TouchContactPhase,
    TouchControlsMode, TouchInputAdapter, TouchInputEvent, TouchInputSettings, TouchUiContactRoute,
    TouchUiContactTracker,
};
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render::color_profile::DEFAULT_RENDER_SCALE;
use mclone_render::native::{NativeSurfaceContext, SurfaceFrameStatus};
use mclone_ui::{
    EMPTY_HOTBAR_ICONS, GameFlatPresentationState, GameTouchSettings, GameUiAction,
    GameWorldRenderScaleMode, GuiScale, Point, TouchJoystickOverlay, TouchOverlay,
    touch_control_at, touch_menu_button_rect,
};
use serde_json::{Value, json};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase,
    WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use crate::cli::{
    NativeWindowOptions, SceneOptions, StartupWaitPolicy, WindowFrameReportOptions,
    WindowPlatformProfile, WindowStartIntent,
};
use crate::frame_pacing::{
    FramePacing, FramePacingMode, FrameTimingStats, RedrawSchedule, elapsed_ms,
    next_capped_redraw_deadline, redraw_schedule,
};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowRuntimeStats, WindowSceneAssets};
use crate::winit_frame_driver::{WinitFrameDriver, WinitHostEffectOutcome, WinitInputOutcome};
use mclone_audio::{AudioEngine, AudioSettings};
use mclone_scene::{MonoBlinkCommitStatus, MonoSceneFrameSummary, MonoUiContext};

const NO_CLIP_TOGGLE_KEY: KeyCode = KeyCode::KeyN;
const DESKTOP_BLINK_DEBUG_KEY: KeyCode = KeyCode::KeyT;
const WORLDGEN_LENS_TOGGLE_KEY: KeyCode = KeyCode::F3;
const WORLDGEN_LENS_CYCLE_KEY: KeyCode = KeyCode::F4;
const FRAME_PIPELINE_OVERLAY_KEY: KeyCode = KeyCode::F6;
const DEBUG_PHYSICS_CUBE_SHOOT_KEY: KeyCode = KeyCode::F7;
const RENDER_RESOURCE_REBUILD_KEY: KeyCode = KeyCode::F8;
const RENDER_SCALE_REBUILD_KEY: KeyCode = KeyCode::F9;
const RENDER_SCALE_PRESET_EPSILON: f32 = 0.000_1;
const STEAMOS_WORLD_RENDER_MAX_HEIGHT: u32 = 1080;
const STEAMOS_WORLD_RENDER_MAX_PIXELS: u64 = 1920 * 1080;
const UI_V2_HIT_DEBUG_ENV: &str = "MCLONE_UI_V2_HIT_DEBUG";
const STATIC_CONTROLLER_POLL_INTERVAL: Duration = Duration::from_millis(33);

pub(crate) fn run_window(
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    window_options: NativeWindowOptions,
    start_intent: WindowStartIntent,
    startup_wait: StartupWaitPolicy,
    frame_report: Option<WindowFrameReportOptions>,
) -> Result<()> {
    let assets = WindowSceneAssets::load()?;
    log::info!(
        "native window startup profile={} initial_physical={}x{} seed={} initial_center=({}, {}) render_distance={} render_compile_workers={} lighting={} cadence={}/{}/{} color_profile={} remote={:?} atlas={}x{} start={:?} startup_wait={:?}",
        window_options.platform_profile.label(),
        window_options.initial_width,
        window_options.initial_height,
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        scene.render_distance,
        scene.render_compile_worker_count,
        if scene.lighting_enabled {
            "enabled"
        } else {
            "disabled"
        },
        scene.simulation_cadence.host_rate_hz,
        scene.simulation_cadence.gameplay_rate_hz,
        scene.simulation_cadence.physics_rate_hz,
        render_options.color_profile.as_str(),
        scene.remote_addr,
        assets.mesh_assets.atlas.width,
        assets.mesh_assets.atlas.height,
        start_intent,
        startup_wait
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ChunkApp::new_with_frame_report(
        scene,
        assets,
        render_options,
        window_options,
        start_intent,
        startup_wait,
        frame_report,
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn automatic_world_render_scale(
    profile: WindowPlatformProfile,
    output_size: [u32; 2],
) -> Option<f32> {
    if profile != WindowPlatformProfile::SteamOs {
        return None;
    }
    let width = output_size[0].max(1);
    let height = output_size[1].max(1);
    let output_pixels = u64::from(width) * u64::from(height);
    let height_scale = f64::from(STEAMOS_WORLD_RENDER_MAX_HEIGHT) / f64::from(height);
    let pixel_scale = (STEAMOS_WORLD_RENDER_MAX_PIXELS as f64 / output_pixels as f64).sqrt();
    Some(
        height_scale
            .min(pixel_scale)
            .min(f64::from(DEFAULT_RENDER_SCALE))
            .max(f64::from(MIN_FLAT_RENDER_SCALE)) as f32,
    )
}

fn world_render_scale(
    profile: WindowPlatformProfile,
    output_size: [u32; 2],
    mode: GameWorldRenderScaleMode,
) -> f32 {
    mode.fixed_scale().unwrap_or_else(|| {
        automatic_world_render_scale(profile, output_size).unwrap_or(DEFAULT_RENDER_SCALE)
    })
}

const fn desktop_startup_touch_present(profile: WindowPlatformProfile) -> bool {
    matches!(profile, WindowPlatformProfile::SteamOs)
}

fn desktop_touch_preferences(
    baseline: &ClientInputPreferences,
    mode: TouchControlsMode,
    look_sensitivity: f32,
) -> ClientInputPreferences {
    let mut current = baseline.clone();
    current.touch_look_sensitivity = look_sensitivity;
    current.touch_controls_mode = mode;
    current.normalized()
}

#[derive(Clone, Debug)]
struct DesktopFlatInputAdapter {
    capability_state: InputCapabilityState,
}

impl Default for DesktopFlatInputAdapter {
    fn default() -> Self {
        Self {
            capability_state: InputCapabilityState::new(InputCapabilities {
                keyboard: true,
                mouse: true,
                touch: false,
                gamepad: false,
                xr_controller: false,
            }),
        }
    }
}

impl DesktopFlatInputAdapter {
    fn with_touch_present(touch_present: bool) -> Self {
        let mut input = Self::default();
        input
            .capability_state
            .set_present(InputDeviceKind::Touch, touch_present);
        input
    }

    fn note_keyboard_activity(&mut self) {
        self.capability_state
            .note_activity(InputDeviceKind::Keyboard);
    }

    fn note_mouse_activity(&mut self) {
        self.capability_state.note_activity(InputDeviceKind::Mouse);
    }

    fn note_touch_activity(&mut self) {
        self.capability_state.note_activity(InputDeviceKind::Touch);
    }

    fn set_gamepad_present(&mut self, present: bool) {
        self.capability_state
            .set_present(InputDeviceKind::Gamepad, present);
    }

    fn note_gamepad_activity(&mut self) {
        self.capability_state
            .note_activity(InputDeviceKind::Gamepad);
    }

    fn normalize_keyboard_input(
        &mut self,
        key_code: KeyCode,
        state: ElementState,
        repeat: bool,
    ) -> Option<(KeyboardKey, bool, bool)> {
        self.note_keyboard_activity();
        let key = desktop_keyboard_key_from_key_code(key_code)?;
        Some((key, state == ElementState::Pressed, repeat))
    }

    fn normalize_mouse_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
    ) -> Option<(PointerButton, bool)> {
        self.note_mouse_activity();
        let button = desktop_pointer_button_from_mouse_button(button)?;
        Some((button, state == ElementState::Pressed))
    }

    fn normalize_mouse_motion(&mut self, delta_x: f32, delta_y: f32) -> (f32, f32) {
        self.note_mouse_activity();
        (delta_x, delta_y)
    }

    fn normalize_mouse_wheel(&mut self, delta: MouseScrollDelta) -> MouseWheelDirection {
        self.note_mouse_activity();
        let amount = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(position) => position.y as f32,
        };
        if amount >= 0.0 {
            MouseWheelDirection::Up
        } else {
            MouseWheelDirection::Down
        }
    }
}

struct ChunkApp {
    assets: WindowSceneAssets,
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    window_options: NativeWindowOptions,
    scene_driver: Option<WinitFrameDriver>,
    gamepad_collector: Option<crate::desktop_gamepad::DesktopGamepadCollector>,
    flat_input: DesktopFlatInputAdapter,
    input_preferences: InputPreferences,
    client_input_preferences: ClientInputPreferences,
    controller_preferences: ControllerInputPreferences,
    touch: TouchInputAdapter,
    frame_pacing: FramePacing,
    world_render_scale_mode: GameWorldRenderScaleMode,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    frame_timing: FrameTimingStats,
    mouse_locked: bool,
    mouse_lock_requested: bool,
    last_cursor: Option<(f64, f64)>,
    ui_touch: TouchUiContactTracker,
    ui_v2_hit_debug: bool,
    last_frame: Instant,
    next_redraw_at: Option<Instant>,
    next_controller_poll_at: Option<Instant>,
    start_intent: WindowStartIntent,
    startup_wait: StartupWaitPolicy,
    frame_report: Option<WindowFrameReportRecorder>,
    camera_traversal_elapsed_seconds: f64,
}

#[derive(Clone, Copy, Debug)]
struct WindowFrameSample {
    status: &'static str,
    frame_wall_ms: f64,
    budget_ms: Option<f64>,
    runtime_poll_ms: f64,
    remesh_ms: f64,
    upload_ms: f64,
    render_ms: f64,
    surface_acquire_ms: f64,
    surface_encode_ms: f64,
    surface_submit_ms: f64,
    surface_present_ms: f64,
    gpu: WindowGpuTimestampSample,
    camera_eye: Option<[f32; 3]>,
    work: Option<WindowFrameWorkSample>,
}

#[derive(Clone, Copy, Debug, Default)]
struct WindowGpuTimestampSample {
    supported: bool,
    frames_resolved: u64,
    dropped_frames: u64,
    total_ms: Option<f64>,
    terrain_ms: Option<f64>,
    terrain_opaque_ms: Option<f64>,
    terrain_translucent_ms: Option<f64>,
    sky_ms: Option<f64>,
    actor_ms: Option<f64>,
    ui_ms: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct WindowFrameWorkSample {
    section_count: usize,
    drawn_section_count: usize,
    frustum_section_count: usize,
    drawn_index_count: u32,
    server_tick_ms: f64,
    server_reported_total_ms: f64,
    scheduler_tick_ms: f64,
    scheduler_reconcile_holders_ms: f64,
    scheduler_active_levels_ms: f64,
    scheduler_holder_updates_ms: f64,
    scheduler_runtime_enqueue_ms: f64,
    scheduler_active_levels_calls: usize,
    scheduler_active_levels_cache_hits: usize,
    scheduler_holder_update_count: usize,
    scheduler_runtime_target_count: usize,
    scheduler_adaptive_publication_budget_enabled: bool,
    scheduler_feature_publish_budget_max_units: usize,
    scheduler_feature_publish_budget_ms: f64,
    scheduler_feature_publish_spent_units: usize,
    scheduler_feature_publish_spent_ms: f64,
    scheduler_light_publish_budget_max_units: usize,
    scheduler_light_publish_budget_ms: f64,
    scheduler_light_publish_spent_units: usize,
    scheduler_light_publish_spent_ms: f64,
    scheduler_pending_worldgen_publication_chunk_limit: usize,
    scheduler_pending_worldgen_publication_jobs: usize,
    scheduler_pending_worldgen_publication_chunks: usize,
    scheduler_pending_light_publications: usize,
    scheduler_cumulative_feature_chunks_published: u64,
    scheduler_cumulative_light_statuses_published: u64,
    worldgen_mailbox_pending_jobs: usize,
    light_mailbox_pending_statuses: usize,
    server_update_queue_depth: usize,
    server_update_queue_bytes: usize,
    server_update_oldest_applied_age_ms: f64,
    update_pump_stalled: bool,
    update_pump_stall_count: usize,
    server_pending_jobs: usize,
    server_pending_publications: usize,
    pending_render_chunks_before: usize,
    pending_render_chunks_after: usize,
    pending_compile_jobs_before: usize,
    pending_compile_jobs_after: usize,
    submitted_compile_sections: usize,
    completed_compile_sections: usize,
    stale_compile_sections: usize,
    deadline_skipped_compile_requests: usize,
    rebuilt_sections: usize,
    uploaded_sections: usize,
    uploaded_vertices: u32,
    uploaded_indices: u32,
    upload_limited: bool,
    upload_backpressured: bool,
    compile_worker_count: usize,
    completed_compile_tasks: usize,
    total_compile_worker_busy_ms: f64,
    max_compile_worker_task_ms: f64,
    fluid_due_ticks: usize,
    fluid_executed_ticks: usize,
    fluid_deferred_ticks: usize,
    fluid_mutated_blocks: usize,
    scheduled_fluid_ticks: usize,
    pending_render_count_ms: f64,
    render_sync_ms: f64,
    render_dirty_seed_ms: f64,
    render_prepare_ms: f64,
    traversal_ready_sections_ms: f64,
    traversal_ready_publish_ms: f64,
    terrain_records_ms: f64,
    terrain_cull_ms: f64,
    terrain_cull_cache_lookups: usize,
    terrain_cull_cache_hits: usize,
    terrain_prepare_ms: f64,
    terrain_encode_ms: f64,
    terrain_direct_draw_calls: usize,
    terrain_multi_draw_calls: usize,
    terrain_indirect_draw_count: usize,
    terrain_arena_vertex_used_bytes: u64,
    terrain_arena_vertex_capacity_bytes: u64,
    terrain_arena_index_used_bytes: u64,
    terrain_arena_index_capacity_bytes: u64,
    terrain_opaque_ms: f64,
    terrain_translucent_ms: f64,
}

impl WindowFrameWorkSample {
    fn from_summary(summary: &MonoSceneFrameSummary) -> Self {
        let upload = summary.upload;
        Self {
            section_count: summary.render.section_count,
            drawn_section_count: summary.render.drawn_section_count,
            frustum_section_count: summary.render.frustum_section_count,
            drawn_index_count: summary.render.drawn_index_count,
            server_tick_ms: upload.poll_server_tick_ms,
            server_reported_total_ms: upload.poll_server_reported_total_ms,
            scheduler_tick_ms: upload.poll_scheduler_tick_ms,
            scheduler_reconcile_holders_ms: upload.poll_scheduler_reconcile_holders_ms,
            scheduler_active_levels_ms: upload.poll_scheduler_active_levels_ms,
            scheduler_holder_updates_ms: upload.poll_scheduler_holder_updates_ms,
            scheduler_runtime_enqueue_ms: upload.poll_scheduler_runtime_enqueue_ms,
            scheduler_active_levels_calls: upload.poll_scheduler_active_levels_calls,
            scheduler_active_levels_cache_hits: upload.poll_scheduler_active_levels_cache_hits,
            scheduler_holder_update_count: upload.poll_scheduler_holder_update_count,
            scheduler_runtime_target_count: upload.poll_scheduler_runtime_target_count,
            scheduler_adaptive_publication_budget_enabled: upload
                .poll_scheduler_adaptive_publication_budget_enabled,
            scheduler_feature_publish_budget_max_units: upload
                .poll_scheduler_feature_publish_budget_max_units,
            scheduler_feature_publish_budget_ms: upload.poll_scheduler_feature_publish_budget_ms,
            scheduler_feature_publish_spent_units: upload
                .poll_scheduler_feature_publish_spent_units,
            scheduler_feature_publish_spent_ms: upload.poll_scheduler_feature_publish_spent_ms,
            scheduler_light_publish_budget_max_units: upload
                .poll_scheduler_light_publish_budget_max_units,
            scheduler_light_publish_budget_ms: upload.poll_scheduler_light_publish_budget_ms,
            scheduler_light_publish_spent_units: upload.poll_scheduler_light_publish_spent_units,
            scheduler_light_publish_spent_ms: upload.poll_scheduler_light_publish_spent_ms,
            scheduler_pending_worldgen_publication_chunk_limit: upload
                .poll_scheduler_pending_worldgen_publication_chunk_limit,
            scheduler_pending_worldgen_publication_jobs: upload
                .poll_scheduler_pending_worldgen_publication_jobs,
            scheduler_pending_worldgen_publication_chunks: upload
                .poll_scheduler_pending_worldgen_publication_chunks,
            scheduler_pending_light_publications: upload.poll_scheduler_pending_light_publications,
            scheduler_cumulative_feature_chunks_published: upload
                .poll_scheduler_cumulative_feature_chunks_published,
            scheduler_cumulative_light_statuses_published: upload
                .poll_scheduler_cumulative_light_statuses_published,
            worldgen_mailbox_pending_jobs: upload.poll_scheduler_worldgen_mailbox_pending_jobs,
            light_mailbox_pending_statuses: upload.poll_scheduler_light_mailbox_pending_statuses,
            server_update_queue_depth: upload.server_update_queue_depth,
            server_update_queue_bytes: upload.server_update_queue_bytes,
            server_update_oldest_applied_age_ms: upload.server_update_oldest_applied_age_ms,
            update_pump_stalled: upload.update_pump_stalled,
            update_pump_stall_count: upload.update_pump_stall_count,
            server_pending_jobs: upload.server_pending_jobs,
            server_pending_publications: upload.server_pending_publications,
            pending_render_chunks_before: upload.pending_render_chunks_before,
            pending_render_chunks_after: upload.pending_render_chunks_after,
            pending_compile_jobs_before: upload.pending_compile_jobs_before,
            pending_compile_jobs_after: upload.pending_compile_jobs_after,
            submitted_compile_sections: upload.submitted_compile_section_count,
            completed_compile_sections: upload.completed_compile_section_count,
            stale_compile_sections: upload.stale_compile_section_count,
            deadline_skipped_compile_requests: upload.deadline_skipped_compile_request_count,
            rebuilt_sections: upload.rebuilt_section_count,
            uploaded_sections: upload.uploaded_section_count,
            uploaded_vertices: upload.uploaded_vertex_count,
            uploaded_indices: upload.uploaded_index_count,
            upload_limited: upload.upload_limited,
            upload_backpressured: upload.upload_backpressured,
            compile_worker_count: upload.dispatcher_compile_worker_count,
            completed_compile_tasks: upload.dispatcher_completed_compile_tasks,
            total_compile_worker_busy_ms: upload.dispatcher_total_compile_worker_busy_ms,
            max_compile_worker_task_ms: upload.dispatcher_max_compile_worker_task_ms,
            fluid_due_ticks: upload.poll_fluid_due_ticks,
            fluid_executed_ticks: upload.poll_fluid_executed_ticks,
            fluid_deferred_ticks: upload.poll_fluid_deferred_ticks,
            fluid_mutated_blocks: upload.poll_fluid_mutated_blocks,
            scheduled_fluid_ticks: upload.poll_scheduled_fluid_ticks,
            pending_render_count_ms: summary.timing.runtime_pending_render_count_ms,
            render_sync_ms: summary.timing.runtime_sync_ms,
            render_dirty_seed_ms: summary.timing.runtime_dirty_seed_ms,
            render_prepare_ms: summary.timing.runtime_prepare_ms,
            traversal_ready_sections_ms: summary.timing.runtime_ready_sections_ms,
            traversal_ready_publish_ms: summary.timing.runtime_ready_publish_ms,
            terrain_records_ms: summary.render_timing.terrain_records_ms,
            terrain_cull_ms: summary.render_timing.terrain_cull_ms,
            terrain_cull_cache_lookups: summary.render_timing.terrain_cull_cache_lookups,
            terrain_cull_cache_hits: summary.render_timing.terrain_cull_cache_hits,
            terrain_prepare_ms: summary.render_timing.terrain_prepare_ms,
            terrain_encode_ms: summary.render_timing.terrain_encode_ms,
            terrain_direct_draw_calls: summary.render_timing.terrain_direct_draw_calls,
            terrain_multi_draw_calls: summary.render_timing.terrain_multi_draw_calls,
            terrain_indirect_draw_count: summary.render_timing.terrain_indirect_draw_count,
            terrain_arena_vertex_used_bytes: summary.render_timing.terrain_arena_vertex_used_bytes,
            terrain_arena_vertex_capacity_bytes: summary
                .render_timing
                .terrain_arena_vertex_capacity_bytes,
            terrain_arena_index_used_bytes: summary.render_timing.terrain_arena_index_used_bytes,
            terrain_arena_index_capacity_bytes: summary
                .render_timing
                .terrain_arena_index_capacity_bytes,
            terrain_opaque_ms: summary.render_timing.terrain_opaque_ms,
            terrain_translucent_ms: summary.render_timing.terrain_translucent_ms,
        }
    }

    fn json(self) -> Value {
        json!({
            "render": {
                "section_count": self.section_count,
                "drawn_section_count": self.drawn_section_count,
                "frustum_section_count": self.frustum_section_count,
                "drawn_index_count": self.drawn_index_count,
            },
            "server": {
                "tick_ms": self.server_tick_ms,
                "reported_total_ms": self.server_reported_total_ms,
                "update_queue_depth": self.server_update_queue_depth,
                "update_queue_bytes": self.server_update_queue_bytes,
                "update_oldest_applied_age_ms": self.server_update_oldest_applied_age_ms,
                "update_pump_stalled": self.update_pump_stalled,
                "update_pump_stall_count": self.update_pump_stall_count,
                "pending_jobs": self.server_pending_jobs,
                "pending_publications": self.server_pending_publications,
            },
            "scheduler": {
                "tick_ms": self.scheduler_tick_ms,
                "reconcile_holders_ms": self.scheduler_reconcile_holders_ms,
                "active_levels_ms": self.scheduler_active_levels_ms,
                "holder_updates_ms": self.scheduler_holder_updates_ms,
                "runtime_enqueue_ms": self.scheduler_runtime_enqueue_ms,
                "active_levels_calls": self.scheduler_active_levels_calls,
                "active_levels_cache_hits": self.scheduler_active_levels_cache_hits,
                "holder_update_count": self.scheduler_holder_update_count,
                "runtime_target_count": self.scheduler_runtime_target_count,
                "adaptive_publication_budget_enabled": self.scheduler_adaptive_publication_budget_enabled,
                "feature_publish_budget_max_units": self.scheduler_feature_publish_budget_max_units,
                "feature_publish_budget_ms": self.scheduler_feature_publish_budget_ms,
                "feature_publish_spent_units": self.scheduler_feature_publish_spent_units,
                "feature_publish_spent_ms": self.scheduler_feature_publish_spent_ms,
                "light_publish_budget_max_units": self.scheduler_light_publish_budget_max_units,
                "light_publish_budget_ms": self.scheduler_light_publish_budget_ms,
                "light_publish_spent_units": self.scheduler_light_publish_spent_units,
                "light_publish_spent_ms": self.scheduler_light_publish_spent_ms,
                "pending_worldgen_publication_chunk_limit": self.scheduler_pending_worldgen_publication_chunk_limit,
                "pending_worldgen_publication_jobs": self.scheduler_pending_worldgen_publication_jobs,
                "pending_worldgen_publication_chunks": self.scheduler_pending_worldgen_publication_chunks,
                "pending_light_publications": self.scheduler_pending_light_publications,
                "cumulative_feature_chunks_published": self.scheduler_cumulative_feature_chunks_published,
                "cumulative_light_statuses_published": self.scheduler_cumulative_light_statuses_published,
                "worldgen_mailbox_pending_jobs": self.worldgen_mailbox_pending_jobs,
                "light_mailbox_pending_statuses": self.light_mailbox_pending_statuses,
            },
            "render_stream": {
                "pending_render_chunks_before": self.pending_render_chunks_before,
                "pending_render_chunks_after": self.pending_render_chunks_after,
                "pending_compile_jobs_before": self.pending_compile_jobs_before,
                "pending_compile_jobs_after": self.pending_compile_jobs_after,
                "submitted_compile_sections": self.submitted_compile_sections,
                "completed_compile_sections": self.completed_compile_sections,
                "stale_compile_sections": self.stale_compile_sections,
                "deadline_skipped_compile_requests": self.deadline_skipped_compile_requests,
                "rebuilt_sections": self.rebuilt_sections,
                "uploaded_sections": self.uploaded_sections,
                "uploaded_vertices": self.uploaded_vertices,
                "uploaded_indices": self.uploaded_indices,
                "upload_limited": self.upload_limited,
                "upload_backpressured": self.upload_backpressured,
                "compile_worker_count": self.compile_worker_count,
                "completed_compile_tasks": self.completed_compile_tasks,
                "total_compile_worker_busy_ms": self.total_compile_worker_busy_ms,
                "max_compile_worker_task_ms": self.max_compile_worker_task_ms,
                "fluid_due_ticks": self.fluid_due_ticks,
                "fluid_executed_ticks": self.fluid_executed_ticks,
                "fluid_deferred_ticks": self.fluid_deferred_ticks,
                "fluid_mutated_blocks": self.fluid_mutated_blocks,
                "scheduled_fluid_ticks": self.scheduled_fluid_ticks,
                "pending_render_count_ms": self.pending_render_count_ms,
                "sync_ms": self.render_sync_ms,
                "dirty_seed_ms": self.render_dirty_seed_ms,
                "prepare_ms": self.render_prepare_ms,
                "traversal_ready_sections_ms": self.traversal_ready_sections_ms,
                "traversal_ready_publish_ms": self.traversal_ready_publish_ms,
            },
            "render_timing": {
                "terrain_records_ms": self.terrain_records_ms,
                "terrain_cull_ms": self.terrain_cull_ms,
                "terrain_cull_cache_lookups": self.terrain_cull_cache_lookups,
                "terrain_cull_cache_hits": self.terrain_cull_cache_hits,
                "terrain_prepare_ms": self.terrain_prepare_ms,
                "terrain_encode_ms": self.terrain_encode_ms,
                "terrain_direct_draw_calls": self.terrain_direct_draw_calls,
                "terrain_multi_draw_calls": self.terrain_multi_draw_calls,
                "terrain_indirect_draw_count": self.terrain_indirect_draw_count,
                "terrain_arena_vertex_used_bytes": self.terrain_arena_vertex_used_bytes,
                "terrain_arena_vertex_capacity_bytes": self.terrain_arena_vertex_capacity_bytes,
                "terrain_arena_index_used_bytes": self.terrain_arena_index_used_bytes,
                "terrain_arena_index_capacity_bytes": self.terrain_arena_index_capacity_bytes,
                "terrain_opaque_ms": self.terrain_opaque_ms,
                "terrain_translucent_ms": self.terrain_translucent_ms,
            },
        })
    }
}

impl WindowGpuTimestampSample {
    fn from_panel(panel: &GpuTimestampPanelReport) -> Self {
        let mut sample = Self {
            supported: panel.supported,
            frames_resolved: panel.frames_resolved,
            dropped_frames: panel.dropped_frames,
            ..Self::default()
        };
        if panel.latest_passes.is_empty() {
            return sample;
        }
        let mut total_ms = 0.0;
        let mut terrain_ms = 0.0;
        let mut terrain_opaque_ms = 0.0;
        let mut terrain_translucent_ms = 0.0;
        let mut sky_ms = 0.0;
        let mut actor_ms = 0.0;
        let mut ui_ms = 0.0;
        let mut valid_passes = 0;
        for pass in panel.latest_passes.iter().filter(|pass| pass.valid) {
            valid_passes += 1;
            total_ms += pass.elapsed_ms;
            match &pass.pass {
                GpuPassId::Terrain => terrain_ms += pass.elapsed_ms,
                GpuPassId::TerrainOpaque => {
                    terrain_ms += pass.elapsed_ms;
                    terrain_opaque_ms += pass.elapsed_ms;
                }
                GpuPassId::TerrainTranslucent => {
                    terrain_ms += pass.elapsed_ms;
                    terrain_translucent_ms += pass.elapsed_ms;
                }
                GpuPassId::Sky => sky_ms += pass.elapsed_ms,
                GpuPassId::Actor => actor_ms += pass.elapsed_ms,
                GpuPassId::Ui => ui_ms += pass.elapsed_ms,
                _ => {}
            }
        }
        if valid_passes > 0 {
            sample.total_ms = Some(total_ms);
            sample.terrain_ms = Some(terrain_ms);
            sample.terrain_opaque_ms = Some(terrain_opaque_ms);
            sample.terrain_translucent_ms = Some(terrain_translucent_ms);
            sample.sky_ms = Some(sky_ms);
            sample.actor_ms = Some(actor_ms);
            sample.ui_ms = Some(ui_ms);
        }
        sample
    }

    fn json(self) -> Value {
        json!({
            "supported": self.supported,
            "frames_resolved": self.frames_resolved,
            "dropped_frames": self.dropped_frames,
            "total_ms": self.total_ms,
            "terrain_ms": self.terrain_ms,
            "terrain_opaque_ms": self.terrain_opaque_ms,
            "terrain_translucent_ms": self.terrain_translucent_ms,
            "sky_ms": self.sky_ms,
            "actor_ms": self.actor_ms,
            "ui_ms": self.ui_ms,
        })
    }
}

impl WindowFrameSample {
    fn json(self) -> Value {
        json!({
            "status": self.status,
            "frame_wall_ms": self.frame_wall_ms,
            "budget_ms": self.budget_ms,
            "runtime_poll_ms": self.runtime_poll_ms,
            "remesh_ms": self.remesh_ms,
            "upload_ms": self.upload_ms,
            "render_ms": self.render_ms,
            "surface_acquire_ms": self.surface_acquire_ms,
            "surface_encode_ms": self.surface_encode_ms,
            "surface_submit_ms": self.surface_submit_ms,
            "surface_present_ms": self.surface_present_ms,
            "gpu": self.gpu.json(),
            "camera_eye": self.camera_eye,
            "work": self.work.map(WindowFrameWorkSample::json),
        })
    }
}

#[derive(Debug)]
struct WindowFrameReportRecorder {
    options: WindowFrameReportOptions,
    recorded_wall_ms: f64,
    frames: Vec<WindowFrameSample>,
}

impl WindowFrameReportRecorder {
    fn new(options: WindowFrameReportOptions) -> Self {
        Self {
            frames: Vec::with_capacity(options.frames.min(72000)),
            options,
            recorded_wall_ms: 0.0,
        }
    }

    fn record(&mut self, sample: WindowFrameSample) -> bool {
        self.recorded_wall_ms += sample.frame_wall_ms;
        self.frames.push(sample);
        self.options.duration_seconds.map_or_else(
            || self.frames.len() >= self.options.frames,
            |seconds| self.recorded_wall_ms >= seconds * 1_000.0,
        )
    }

    fn write(&self, app: &ChunkApp) -> Result<()> {
        let report = self.json(app);
        if let Some(parent) = self
            .options
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create window frame report directory {}",
                    parent.display()
                )
            })?;
        }
        let bytes = serde_json::to_vec_pretty(&report)
            .context("failed to serialize window frame report")?;
        fs::write(&self.options.path, bytes).with_context(|| {
            format!(
                "failed to write window frame report {}",
                self.options.path.display()
            )
        })?;
        log::info!(
            "window frame report wrote {} frames to {}",
            self.frames.len(),
            self.options.path.display()
        );
        Ok(())
    }

    fn json(&self, app: &ChunkApp) -> Value {
        let budget_counts = window_frame_budget_counts(&self.frames);
        let frame_pacing = app.frame_pacing.debug_stats();
        let scene = &app.scene;
        let render_options = app.render_options;
        let final_timing = app.frame_timing;
        let scene_json = json!({
            "seed": scene.seed,
            "chunk_x": scene.chunk_x,
            "chunk_z": scene.chunk_z,
            "render_distance": scene.render_distance,
            "render_compile_workers": scene.render_compile_worker_count,
            "render_compile_max_pending_jobs": scene.render_compile_max_pending_jobs,
            "render_compile_capacity_mode": scene.render_compile_capacity_mode.as_str(),
            "remote_addr": &scene.remote_addr,
            "world_root": scene.world_root.as_ref().map(|path| path.display().to_string()),
            "world_dir": scene.world_dir.as_ref().map(|path| path.display().to_string()),
            "lighting_enabled": scene.lighting_enabled,
            "adaptive_chunk_publication_budget": scene.adaptive_chunk_publication_budget,
            "adaptive_render_admission_budget": scene.adaptive_render_admission_budget,
            "debug_passive_showcase": scene.debug_passive_showcase,
            "window_freeze_scheduled_fluid_ticks": self.options.freeze_scheduled_fluid_ticks,
            "window_world_render_scale_mode": self
                .options
                .world_render_scale_mode
                .map(|mode| format!("{mode:?}")),
            "startup_wait": format!("{:?}", app.startup_wait),
            "simulation_cadence": {
                "host_rate_hz": scene.simulation_cadence.host_rate_hz,
                "gameplay_rate_hz": scene.simulation_cadence.gameplay_rate_hz,
                "physics_rate_hz": scene.simulation_cadence.physics_rate_hz,
            },
        });
        let camera_pose_json = self.options.camera_pose.map(|pose| {
            json!({
                "eye": pose.eye,
                "target": pose.target,
                "velocity_blocks_per_second": self.options.camera_velocity,
            })
        });
        let final_camera_pose_json = self.options.camera_pose.map(|pose| {
            let delta = self
                .options
                .camera_velocity
                .unwrap_or([0.0; 3])
                .map(|component| component * app.camera_traversal_elapsed_seconds as f32);
            json!({
                "eye": translate_camera_point(pose.eye, delta),
                "target": translate_camera_point(pose.target, delta),
                "traversal_elapsed_seconds": app.camera_traversal_elapsed_seconds,
            })
        });
        let render_options_json = json!({
            "section_occlusion_culling": render_options.section_occlusion_culling,
            "force_fullbright": render_options.force_fullbright,
            "sky_darken": render_options.sky_darken,
            "fog": format!("{:?}", render_options.fog),
            "color_profile": render_options.color_profile.as_str(),
        });
        let surface_json = app.surface.as_ref().map(|surface| {
            let world_size = scaled_frame_size(
                [surface.config.width, surface.config.height],
                surface.render_config.render_scale,
            );
            json!({
                "width": surface.config.width,
                "height": surface.config.height,
                "world_width": world_size[0],
                "world_height": world_size[1],
                "world_render_scale": surface.render_config.render_scale,
                "native_ui": true,
            })
        });
        let window_json = app.window.as_ref().map(|window| {
            let inner_size = window.inner_size();
            let monitor = window.current_monitor().map(|monitor| {
                let size = monitor.size();
                json!({
                    "name": monitor.name(),
                    "width": size.width,
                    "height": size.height,
                    "scale_factor": monitor.scale_factor(),
                    "refresh_hz": monitor
                        .refresh_rate_millihertz()
                        .map(|millihertz| f64::from(millihertz) / 1000.0),
                })
            });
            json!({
                "platform_profile": app.window_options.platform_profile.label(),
                "requested_initial_width": app.window_options.initial_width,
                "requested_initial_height": app.window_options.initial_height,
                "inner_width": inner_size.width,
                "inner_height": inner_size.height,
                "scale_factor": window.scale_factor(),
                "fullscreen": window.fullscreen().is_some(),
                "monitor": monitor,
            })
        });
        let frame_pacing_json = json!({
            "mode": frame_pacing.mode.label(),
            "fps_cap": frame_pacing.fps_cap,
            "monitor_name": &app.frame_pacing.monitor_name,
            "monitor_refresh_hz": frame_pacing.monitor_refresh_hz,
            "target_frame_ms": frame_pacing.target_frame_ms,
            "active_present_mode": frame_pacing.active_present_mode_label,
        });
        let final_frame_timing_json = json!({
            "frame_count": final_timing.frame_count,
            "over_budget_count": final_timing.over_budget_count,
            "double_budget_count": final_timing.double_budget_count,
            "quad_budget_count": final_timing.quad_budget_count,
            "worst_frame_ms": final_timing.worst_frame_ms,
            "budget_ms": final_timing.budget_ms,
        });
        let final_runtime_json = app
            .scene_driver
            .as_ref()
            .and_then(|driver| driver.host().runtime_stats())
            .map(runtime_stats_json);
        let final_scheduler_json = app
            .scene_driver
            .as_ref()
            .and_then(|driver| driver.host().runtime_poll_diagnostics())
            .map(runtime_scheduler_json);
        let gpu_timestamp_panel = app
            .surface
            .as_ref()
            .map(NativeSurfaceContext::gpu_timestamp_panel_report)
            .unwrap_or_else(GpuTimestampPanelReport::unsupported);
        let samples_json = self
            .frames
            .iter()
            .copied()
            .map(WindowFrameSample::json)
            .collect::<Vec<_>>();
        json!({
            "benchmark": "native_window_frame_report",
            "recorded_unix_seconds": crate::current_unix_seconds(),
            "git_commit": crate::git_short_commit(),
            "git_dirty": crate::git_dirty(),
            "debug_assertions": cfg!(debug_assertions),
            "requested_frames": self.options.duration_seconds.is_none().then_some(self.options.frames),
            "requested_seconds": self.options.duration_seconds,
            "frames": self.frames.len(),
            "elapsed_wall_ms": self.recorded_wall_ms,
            "scene": scene_json,
            "camera_pose": camera_pose_json,
            "final_camera_pose": final_camera_pose_json,
            "render_options": render_options_json,
            "window": window_json,
            "surface": surface_json,
            "frame_pacing": frame_pacing_json,
            "budgeted_frames": budget_counts.budgeted_frames,
            "over_budget_frames": budget_counts.over_budget_frames,
            "over_2x_budget_frames": budget_counts.over_2x_budget_frames,
            "over_4x_budget_frames": budget_counts.over_4x_budget_frames,
            "status_counts": window_frame_status_counts(&self.frames),
            "frame_wall": window_series_summary(self.frames.iter().map(|sample| sample.frame_wall_ms)),
            "runtime_poll": window_series_summary(self.frames.iter().map(|sample| sample.runtime_poll_ms)),
            "remesh": window_series_summary(self.frames.iter().map(|sample| sample.remesh_ms)),
            "upload": window_series_summary(self.frames.iter().map(|sample| sample.upload_ms)),
            "render": window_series_summary(self.frames.iter().map(|sample| sample.render_ms)),
            "surface_acquire": window_series_summary(self.frames.iter().map(|sample| sample.surface_acquire_ms)),
            "surface_encode": window_series_summary(self.frames.iter().map(|sample| sample.surface_encode_ms)),
            "surface_submit": window_series_summary(self.frames.iter().map(|sample| sample.surface_submit_ms)),
            "surface_present": window_series_summary(self.frames.iter().map(|sample| sample.surface_present_ms)),
            "gpu_total": window_series_summary(self.frames.iter().filter_map(|sample| sample.gpu.total_ms)),
            "gpu_terrain": window_series_summary(self.frames.iter().filter_map(|sample| sample.gpu.terrain_ms)),
            "gpu_timestamp_panel": gpu_timestamp_panel,
            "final_frame_timing": final_frame_timing_json,
            "final_runtime": final_runtime_json,
            "final_scheduler": final_scheduler_json,
            "samples": samples_json,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WindowFrameBudgetCounts {
    budgeted_frames: usize,
    over_budget_frames: usize,
    over_2x_budget_frames: usize,
    over_4x_budget_frames: usize,
}

fn surface_frame_status_label(status: SurfaceFrameStatus) -> &'static str {
    match status {
        SurfaceFrameStatus::Presented => "presented",
        SurfaceFrameStatus::Reconfigured => "reconfigured",
        SurfaceFrameStatus::Skipped => "skipped",
    }
}

fn window_frame_budget_counts(frames: &[WindowFrameSample]) -> WindowFrameBudgetCounts {
    let mut counts = WindowFrameBudgetCounts::default();
    for frame in frames {
        let Some(budget_ms) = frame.budget_ms.filter(|budget_ms| *budget_ms > 0.0) else {
            continue;
        };
        counts.budgeted_frames += 1;
        if frame.frame_wall_ms > budget_ms {
            counts.over_budget_frames += 1;
        }
        if frame.frame_wall_ms > budget_ms * 2.0 {
            counts.over_2x_budget_frames += 1;
        }
        if frame.frame_wall_ms > budget_ms * 4.0 {
            counts.over_4x_budget_frames += 1;
        }
    }
    counts
}

fn window_frame_status_counts(frames: &[WindowFrameSample]) -> Value {
    let mut presented = 0usize;
    let mut reconfigured = 0usize;
    let mut skipped = 0usize;
    for frame in frames {
        match frame.status {
            "presented" => presented += 1,
            "reconfigured" => reconfigured += 1,
            "skipped" => skipped += 1,
            _ => {}
        }
    }
    json!({
        "presented": presented,
        "reconfigured": reconfigured,
        "skipped": skipped,
    })
}

fn window_series_summary(values: impl IntoIterator<Item = f64>) -> Value {
    let mut values = values
        .into_iter()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return json!({
            "count": 0,
        });
    }
    let sum = values.iter().sum::<f64>();
    json!({
        "count": values.len(),
        "average_ms": sum / values.len() as f64,
        "min_ms": values[0],
        "p50_ms": window_percentile(&values, 0.50),
        "p95_ms": window_percentile(&values, 0.95),
        "p99_ms": window_percentile(&values, 0.99),
        "max_ms": values[values.len() - 1],
    })
}

fn window_percentile(sorted_values: &[f64], quantile: f64) -> f64 {
    let last_index = sorted_values.len().saturating_sub(1);
    let index = (last_index as f64 * quantile)
        .ceil()
        .clamp(0.0, last_index as f64) as usize;
    sorted_values[index]
}

fn translate_camera_point(point: [f32; 3], delta: [f32; 3]) -> [f32; 3] {
    [
        point[0] + delta[0],
        point[1] + delta[1],
        point[2] + delta[2],
    ]
}

fn runtime_stats_json(stats: WindowRuntimeStats) -> Value {
    json!({
        "host_mode": format!("{:?}", stats.host_mode),
        "server_runner_kind": stats.server_runner_kind.map(|kind| format!("{:?}", kind)),
        "server_command_queue_depth": stats.server_command_queue_depth,
        "server_update_queue_depth": stats.server_update_queue_depth,
        "server_update_queue_bytes": stats.server_update_queue_bytes,
        "interest_center": {
            "x": stats.interest_center.x,
            "z": stats.interest_center.z,
        },
        "render_distance": stats.render_distance,
        "chunk_tracking_radius": stats.chunk_tracking_radius,
        "loaded_chunks": stats.loaded_chunks,
        "pending_jobs": stats.pending_jobs,
        "pending_publications": stats.pending_publications,
        "pending_render_chunks": stats.pending_render_chunks,
        "pending_render_compile_jobs": stats.pending_render_compile_jobs,
        "inflight_render_sections": stats.inflight_render_sections,
        "client_visible_chunks": stats.client_visible_chunks,
        "active_ticket_chunks": stats.active_ticket_chunks,
        "loading_progress": stats.loading_progress.map(|progress| json!({
            "center": {
                "x": progress.center.x,
                "z": progress.center.z,
            },
            "target_radius": progress.target_radius,
            "target_status": format!("{:?}", progress.target_status),
            "target_chunk_count": progress.target_chunk_count,
            "target_ready_chunks": progress.target_ready_chunks,
            "playable_chunk": {
                "x": progress.playable_chunk.x,
                "z": progress.playable_chunk.z,
            },
            "playable_gate_radius": progress.playable_gate_radius,
            "playable_gate_chunk_count": progress.playable_gate_chunk_count,
            "playable_gate_ready_chunks": progress.playable_gate_ready_chunks,
            "playable_chunk_ready": progress.playable_chunk_ready,
        })),
        "tracked_players": stats.tracked_players,
        "player_visible_chunks": stats.player_visible_chunks,
        "aggregate_player_ticket_chunks": stats.aggregate_player_ticket_chunks,
        "player_outbound_queue_depth": stats.player_outbound_queue_depth,
        "max_player_visible_chunks": stats.max_player_visible_chunks,
        "max_player_outbound_queue_depth": stats.max_player_outbound_queue_depth,
        "pending_unload_chunks": stats.pending_unload_chunks,
        "block_ticking_chunks": stats.block_ticking_chunks,
        "entity_ticking_chunks": stats.entity_ticking_chunks,
        "last_tick": stats.last_tick,
        "last_simulation_tick": stats.last_simulation_tick,
        "last_tick_unloads_processed": stats.last_tick_unloads_processed,
        "last_simulation_block_tick_chunks": stats.last_simulation_block_tick_chunks,
        "last_simulation_entity_tick_chunks": stats.last_simulation_entity_tick_chunks,
        "last_simulation_scheduler_tick_ms": stats.last_simulation_scheduler_tick_ms,
        "last_simulation_block_tick_ms": stats.last_simulation_block_tick_ms,
        "last_simulation_fluid_tick_ms": stats.last_simulation_fluid_tick_ms,
        "last_simulation_entity_tick_ms": stats.last_simulation_entity_tick_ms,
        "last_simulation_fluid_ticks_executed": stats.last_simulation_fluid_ticks_executed,
        "last_simulation_deferred_fluid_ticks": stats.last_simulation_deferred_fluid_ticks,
        "last_simulation_fluid_mutated_blocks": stats.last_simulation_fluid_mutated_blocks,
        "scheduled_fluid_ticks": stats.scheduled_fluid_ticks,
    })
}

fn runtime_scheduler_json(diagnostics: RuntimePollDiagnostics) -> Value {
    json!({
        "server_update_queue_depth": diagnostics.server_update_queue_depth,
        "server_update_queue_bytes": diagnostics.server_update_queue_bytes,
        "server_pending_jobs": diagnostics.server_pending_jobs,
        "server_pending_publications": diagnostics.server_pending_publications,
        "server_tick_ms": diagnostics.server_tick_ms,
        "server_reported_total_ms": diagnostics.server_reported_total_ms,
        "scheduler_tick_ms": diagnostics.scheduler_tick_ms,
        "adaptive_publication_budget_enabled": diagnostics.scheduler_adaptive_publication_budget_enabled,
        "feature_publish_budget_min_units": diagnostics.scheduler_feature_publish_budget_min_units,
        "feature_publish_budget_max_units": diagnostics.scheduler_feature_publish_budget_max_units,
        "feature_publish_budget_ms": diagnostics.scheduler_feature_publish_budget_ms,
        "feature_publish_spent_units": diagnostics.scheduler_feature_publish_spent_units,
        "feature_publish_spent_ms": diagnostics.scheduler_feature_publish_spent_ms,
        "feature_publish_estimated_unit_ms": diagnostics.scheduler_feature_publish_estimated_unit_ms,
        "light_publish_budget_min_units": diagnostics.scheduler_light_publish_budget_min_units,
        "light_publish_budget_max_units": diagnostics.scheduler_light_publish_budget_max_units,
        "light_publish_budget_ms": diagnostics.scheduler_light_publish_budget_ms,
        "light_publish_spent_units": diagnostics.scheduler_light_publish_spent_units,
        "light_publish_spent_ms": diagnostics.scheduler_light_publish_spent_ms,
        "light_publish_estimated_unit_ms": diagnostics.scheduler_light_publish_estimated_unit_ms,
        "pending_worldgen_publication_chunk_limit": diagnostics.scheduler_pending_worldgen_publication_chunk_limit,
        "pending_worldgen_publication_jobs": diagnostics.scheduler_pending_worldgen_publication_jobs,
        "pending_worldgen_publication_chunks": diagnostics.scheduler_pending_worldgen_publication_chunks,
        "pending_light_publications": diagnostics.scheduler_pending_light_publications,
        "worldgen_mailbox_pending_jobs": diagnostics.scheduler_worldgen_mailbox_pending_jobs,
        "light_mailbox_pending_statuses": diagnostics.scheduler_light_mailbox_pending_statuses,
        "cumulative_feature_chunks_published": diagnostics.scheduler_cumulative_feature_chunks_published,
        "cumulative_light_statuses_published": diagnostics.scheduler_cumulative_light_statuses_published,
        "block_tick_ms": diagnostics.block_tick_ms,
        "fluid_tick_ms": diagnostics.fluid_tick_ms,
        "entity_tick_ms": diagnostics.entity_tick_ms,
        "fluid_due_ticks": diagnostics.fluid_due_ticks,
        "fluid_executed_ticks": diagnostics.fluid_executed_ticks,
        "fluid_deferred_ticks": diagnostics.fluid_deferred_ticks,
        "fluid_mutated_blocks": diagnostics.fluid_mutated_blocks,
        "scheduled_fluid_ticks": diagnostics.scheduled_fluid_ticks,
    })
}

impl ChunkApp {
    fn new_with_frame_report(
        scene: SceneOptions,
        assets: WindowSceneAssets,
        render_options: TexturedSectionRenderOptions,
        window_options: NativeWindowOptions,
        start_intent: WindowStartIntent,
        startup_wait: StartupWaitPolicy,
        frame_report: Option<WindowFrameReportOptions>,
    ) -> Self {
        let ui_v2_hit_debug = std::env::var_os(UI_V2_HIT_DEBUG_ENV).is_some();
        if ui_v2_hit_debug {
            log::info!("{UI_V2_HIT_DEBUG_ENV}=1; UI v2 hit debug overlay/logging enabled");
        }
        let world_render_scale_mode = frame_report
            .as_ref()
            .and_then(|report| report.world_render_scale_mode)
            .unwrap_or(GameWorldRenderScaleMode::Automatic);
        let gamepad_collector = match crate::desktop_gamepad::DesktopGamepadCollector::new() {
            Ok(collector) => {
                log::info!("desktop gamepad collector initialized");
                Some(collector)
            }
            Err(error) => {
                log::warn!("desktop gamepad collector unavailable: {error:#}");
                None
            }
        };
        let client_input_preferences =
            match mclone_app_runtime::input_preferences::load_native_input_preferences(
                scene.world_root.as_deref(),
            ) {
                Ok(preferences) => preferences,
                Err(error) => {
                    log::warn!("desktop input preferences unavailable: {error:#}");
                    mclone_app_runtime::input_preferences::ClientInputPreferences::default()
                }
            };
        let mut input_preferences = InputPreferences::AUTO;
        input_preferences.preferred_scheme = client_input_preferences.controller.preferred_input;
        input_preferences.touch_controls = client_input_preferences.touch_controls_mode;
        let touch = TouchInputAdapter::with_settings(TouchInputSettings {
            look_sensitivity: client_input_preferences.touch_look_sensitivity,
            ..TouchInputSettings::default()
        });
        let startup_touch_present = desktop_startup_touch_present(window_options.platform_profile);
        Self {
            scene: scene.clone(),
            render_options,
            window_options,
            scene_driver: None,
            gamepad_collector,
            assets,
            flat_input: DesktopFlatInputAdapter::with_touch_present(startup_touch_present),
            input_preferences,
            client_input_preferences: client_input_preferences.clone(),
            controller_preferences: client_input_preferences.controller.clone(),
            touch,
            frame_pacing: FramePacing::default(),
            world_render_scale_mode,
            window: None,
            surface: None,
            frame_timing: FrameTimingStats::default(),
            mouse_locked: false,
            mouse_lock_requested: false,
            last_cursor: None,
            ui_touch: TouchUiContactTracker::default(),
            ui_v2_hit_debug,
            last_frame: Instant::now(),
            next_redraw_at: None,
            next_controller_poll_at: None,
            start_intent,
            startup_wait,
            frame_report: frame_report.map(WindowFrameReportRecorder::new),
            camera_traversal_elapsed_seconds: 0.0,
        }
    }

    fn schedule_next_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };
        self.next_controller_poll_at = None;

        if self.frame_pacing.mode != FramePacingMode::Capped {
            self.next_redraw_at = None;
        }

        match redraw_schedule(self.frame_pacing.mode, Instant::now(), self.next_redraw_at) {
            RedrawSchedule::RequestNow => {
                event_loop.set_control_flow(ControlFlow::Poll);
                window.request_redraw();
            }
            RedrawSchedule::WaitUntil(deadline) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        }
    }

    fn continuous_redraw_required(&self) -> bool {
        self.frame_report.is_some()
            || self
                .scene_driver
                .as_ref()
                .is_some_and(|driver| driver.activity_demand().requires_continuous_frames())
    }

    fn wait_for_product_event(&mut self, event_loop: &ActiveEventLoop) {
        self.next_redraw_at = None;
        if self.gamepad_collector.is_some() {
            let deadline = *self
                .next_controller_poll_at
                .get_or_insert_with(|| Instant::now() + STATIC_CONTROLLER_POLL_INTERVAL);
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn poll_static_controller_if_due(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self
            .next_controller_poll_at
            .is_some_and(|deadline| now < deadline)
        {
            return;
        }
        self.next_controller_poll_at = Some(now + STATIC_CONTROLLER_POLL_INTERVAL);
        self.poll_controller_input(event_loop);
    }

    fn finish_redraw(&mut self, event_loop: &ActiveEventLoop, frame_start: Instant) {
        if !self.continuous_redraw_required() {
            self.wait_for_product_event(event_loop);
            return;
        }
        if let Some(frame_duration) = self.frame_pacing.target_frame_duration() {
            let next_redraw_at = next_capped_redraw_deadline(
                frame_start,
                Instant::now(),
                self.next_redraw_at,
                frame_duration,
            );
            self.next_redraw_at = Some(next_redraw_at);
        } else {
            self.next_redraw_at = None;
        }
        self.schedule_next_redraw(event_loop);
    }

    fn record_window_frame_report_sample(
        &mut self,
        status: SurfaceFrameStatus,
        summary: Option<&MonoSceneFrameSummary>,
    ) -> bool {
        let camera_eye = self.frame_report.as_ref().and_then(|recorder| {
            recorder.options.camera_pose.map(|pose| {
                let delta = recorder
                    .options
                    .camera_velocity
                    .unwrap_or([0.0; 3])
                    .map(|component| component * self.camera_traversal_elapsed_seconds as f32);
                translate_camera_point(pose.eye, delta)
            })
        });
        let gpu = self
            .surface
            .as_ref()
            .map(|surface| {
                WindowGpuTimestampSample::from_panel(&surface.gpu_timestamp_panel_report())
            })
            .unwrap_or_default();
        let Some(recorder) = &mut self.frame_report else {
            return false;
        };
        let timing = self.frame_timing;
        recorder.record(WindowFrameSample {
            status: surface_frame_status_label(status),
            frame_wall_ms: timing.last_frame_ms,
            budget_ms: timing.budget_ms,
            runtime_poll_ms: timing.last_runtime_poll_ms,
            remesh_ms: timing.last_remesh_ms,
            upload_ms: timing.last_upload_ms,
            render_ms: timing.last_render_ms,
            surface_acquire_ms: timing.last_surface_acquire_ms,
            surface_encode_ms: timing.last_surface_encode_ms,
            surface_submit_ms: timing.last_surface_submit_ms,
            surface_present_ms: timing.last_surface_present_ms,
            gpu,
            camera_eye,
            work: summary.map(WindowFrameWorkSample::from_summary),
        })
    }

    fn write_window_frame_report(&self) -> Result<()> {
        let Some(recorder) = &self.frame_report else {
            return Ok(());
        };
        recorder.write(self)
    }

    fn current_render_scale(&self) -> f32 {
        let fallback = self
            .surface
            .as_ref()
            .map_or(DEFAULT_RENDER_SCALE, |surface| {
                surface.render_config.render_scale
            });
        fallback
    }

    fn touch_gameplay_enabled(&self) -> bool {
        self.flat_input
            .capability_state
            .resolve(self.input_preferences)
            .touch_controls_visible
            && self
                .scene_driver
                .as_ref()
                .is_some_and(|driver| driver.has_runtime() && !driver.ui_is_active())
    }

    fn touch_overlay(&self) -> TouchOverlay {
        let state = self.touch.overlay_state();
        TouchOverlay {
            visible: true,
            menu_pressed: state.menu_pressed,
            movement: state
                .movement
                .map(|movement| TouchJoystickOverlay {
                    active: true,
                    base: point_from_vec2(movement.base),
                    thumb: point_from_vec2(movement.thumb),
                })
                .unwrap_or_default(),
            jump_pressed: state.jump_pressed,
            sprint_pressed: state.sprint_pressed,
            sneak_pressed: state.sneak_pressed,
            descend_pressed: state.descend_pressed,
            interaction_visible: true,
            attack_pressed: state.attack_pressed,
            use_pressed: state.use_pressed,
            hotbar_visible: true,
            selected_hotbar_slot: self
                .scene_driver
                .as_ref()
                .map_or(0, |driver| driver.host().selected_mono_hotbar_slot()),
            hotbar_pressed_slot: state.hotbar_pressed_slot,
            hotbar_icons: EMPTY_HOTBAR_ICONS,
        }
    }

    fn apply_world_render_scale(&mut self) {
        let Some(surface) = &mut self.surface else {
            return;
        };
        let render_scale = world_render_scale(
            self.window_options.platform_profile,
            [surface.config.width, surface.config.height],
            self.world_render_scale_mode,
        );
        if (surface.render_config.render_scale - render_scale).abs() <= RENDER_SCALE_PRESET_EPSILON
        {
            return;
        }
        surface.render_config = surface.render_config.with_render_scale(render_scale);
        if let Some(driver) = &mut self.scene_driver {
            driver.set_render_config(
                &surface.device,
                [surface.config.width, surface.config.height],
                surface.render_config,
            );
        }
        let world_size = scaled_frame_size(
            [surface.config.width, surface.config.height],
            surface.render_config.render_scale,
        );
        log::info!(
            "{} world render scale mode={:?} scale={:.3}: output={}x{} world={}x{} native_ui=true",
            self.window_options.platform_profile.label(),
            self.world_render_scale_mode,
            surface.render_config.render_scale,
            surface.config.width,
            surface.config.height,
            world_size[0],
            world_size[1],
        );
    }

    fn update_camera_from_keys(&mut self, now: Instant) -> Result<()> {
        let frame_dt = now.duration_since(self.last_frame);
        self.last_frame = now;
        self.frame_timing.begin_frame(
            frame_dt.as_secs_f64() * 1000.0,
            self.frame_pacing.target_frame_ms(),
        );

        let supplemental = self
            .touch_gameplay_enabled()
            .then(|| self.touch.held_frame())
            .flatten();
        let Some(driver) = self.scene_driver.as_mut() else {
            return Ok(());
        };
        let camera_traversal = self.frame_report.as_ref().and_then(|recorder| {
            recorder
                .options
                .camera_pose
                .zip(recorder.options.camera_velocity)
        });
        if let Some((pose, velocity)) = camera_traversal {
            self.camera_traversal_elapsed_seconds += frame_dt.as_secs_f64();
            let delta =
                velocity.map(|component| component * self.camera_traversal_elapsed_seconds as f32);
            driver.set_capture_camera_pose(crate::cli::WindowCameraPose {
                eye: translate_camera_point(pose.eye, delta),
                target: translate_camera_point(pose.target, delta),
            })?;
            driver.reconcile_capture_camera_pose()?;
        } else {
            driver.advance_held_input(supplemental, frame_dt.as_secs_f64())?;
        }
        if !driver.ui_is_active() {
            driver.update_blink_debug();
        }
        Ok(())
    }

    fn poll_controller_input(&mut self, event_loop: &ActiveEventLoop) {
        let poll = match self.gamepad_collector.as_mut() {
            Some(collector) => match collector.poll() {
                Ok(poll) => poll,
                Err(error) => {
                    log::error!("desktop gamepad poll failed; disabling collector: {error:#}");
                    self.gamepad_collector = None;
                    self.flat_input.set_gamepad_present(false);
                    self.clear_flat_gameplay_input();
                    return;
                }
            },
            None => return,
        };
        let topology_changed = !poll.connected.is_empty() || !poll.disconnected.is_empty();
        self.flat_input
            .set_gamepad_present(poll.connected_count() > 0);
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_controller_poll(poll, &surface.device, &surface.queue)
            }
            _ => return,
        };
        let handled = self.apply_input_outcome("gamepad input", result, event_loop);
        if topology_changed && !handled {
            self.schedule_next_redraw(event_loop);
        }
    }

    fn clear_flat_gameplay_input(&mut self) {
        self.touch.clear();
        if let Some(driver) = &mut self.scene_driver {
            driver.clear_interactive_input();
            driver.clear_blink_debug();
        }
    }

    fn route_key_input(
        &mut self,
        key: KeyboardKey,
        pressed: bool,
        repeat: bool,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_key(key, pressed, repeat, &surface.device, &surface.queue)
            }
            _ => return false,
        };
        self.apply_input_outcome("keyboard input", result, event_loop)
    }

    fn route_pointer_button_input(
        &mut self,
        button: PointerButton,
        pressed: bool,
        point: Option<Point>,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_pointer_button(button, pressed, point, &surface.device, &surface.queue)
            }
            _ => return false,
        };
        self.apply_input_outcome("pointer button input", result, event_loop)
    }

    fn route_touch_pointer_button_input(
        &mut self,
        pressed: bool,
        point: Point,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_touch_pointer_button(pressed, point, &surface.device, &surface.queue)
            }
            _ => return false,
        };
        self.apply_input_outcome("touch pointer button input", result, event_loop)
    }

    fn route_touch_look_input(
        &mut self,
        delta: mclone_input::TouchLookDelta,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let result = match self.scene_driver.as_mut() {
            Some(driver) => driver.route_touch_look(delta),
            None => return false,
        };
        self.apply_input_outcome("touch look input", result, event_loop)
    }

    fn route_flat_frame_input(
        &mut self,
        frame: mclone_input::FlatInputFrame,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_flat_frame(frame, &surface.device, &surface.queue)
            }
            _ => return false,
        };
        self.apply_input_outcome("touch action input", result, event_loop)
    }

    fn route_pointer_move_input(&mut self, point: Point, event_loop: &ActiveEventLoop) -> bool {
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_pointer_move(point, &surface.device, &surface.queue)
            }
            _ => return false,
        };
        self.apply_input_outcome("pointer move input", result, event_loop)
    }

    fn route_mouse_motion_input(
        &mut self,
        delta_x: f32,
        delta_y: f32,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_mouse_motion(delta_x, delta_y, &surface.device, &surface.queue)
            }
            _ => return false,
        };
        let handled = self.apply_input_outcome("mouse motion input", result, event_loop);
        if handled
            && self
                .scene_driver
                .as_mut()
                .is_some_and(WinitFrameDriver::update_blink_debug)
        {
            self.schedule_next_redraw(event_loop);
        }
        handled
    }

    fn route_mouse_wheel_input(
        &mut self,
        direction: MouseWheelDirection,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_wheel(direction, &surface.device, &surface.queue)
            }
            _ => return false,
        };
        self.apply_input_outcome("mouse wheel input", result, event_loop)
    }

    fn handle_touch_input(&mut self, touch: Touch, event_loop: &ActiveEventLoop) {
        self.flat_input.note_touch_activity();
        let phase = touch_contact_phase(touch.phase);
        let ui_active = self
            .scene_driver
            .as_ref()
            .is_some_and(WinitFrameDriver::ui_is_active);
        let Some(scale) = self.gui_scale() else {
            self.schedule_next_redraw(event_loop);
            return;
        };
        let Some(point) = self.gui_point(touch.location.x, touch.location.y) else {
            self.schedule_next_redraw(event_loop);
            return;
        };
        match self.ui_touch.route(touch.id, phase, ui_active) {
            TouchUiContactRoute::PointerDown => {
                self.route_touch_pointer_button_input(true, point, event_loop);
            }
            TouchUiContactRoute::PointerMove => {
                self.route_pointer_move_input(point, event_loop);
            }
            TouchUiContactRoute::PointerUp => {
                self.route_touch_pointer_button_input(false, point, event_loop);
            }
            TouchUiContactRoute::Cancel => {
                if let Some(driver) = &mut self.scene_driver {
                    driver.clear_ui_input();
                }
                self.schedule_next_redraw(event_loop);
            }
            TouchUiContactRoute::Ignore => self.schedule_next_redraw(event_loop),
            TouchUiContactRoute::Gameplay => {
                if !self.touch_gameplay_enabled() {
                    if matches!(
                        phase,
                        TouchContactPhase::Ended | TouchContactPhase::Cancelled
                    ) {
                        self.touch
                            .end_contact(touch.id, phase == TouchContactPhase::Cancelled);
                    }
                    self.schedule_next_redraw(event_loop);
                    return;
                }
                let position = Vec2::new(point.x, point.y);
                self.touch
                    .set_viewport_size(Vec2::new(scale.width, scale.height));
                let event = match phase {
                    TouchContactPhase::Started => {
                        let control = touch_control_at(scale, point);
                        self.touch.begin_contact(touch.id, control, position)
                    }
                    TouchContactPhase::Moved => {
                        let menu_active = touch_menu_button_rect().contains(point);
                        self.touch.move_contact(touch.id, position, menu_active)
                    }
                    TouchContactPhase::Ended | TouchContactPhase::Cancelled => self
                        .touch
                        .end_contact(touch.id, phase == TouchContactPhase::Cancelled),
                };
                self.apply_touch_input_event(event, event_loop);
            }
        }
    }

    fn apply_touch_input_event(
        &mut self,
        event: TouchInputEvent,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        if let Some(driver) = &mut self.scene_driver {
            driver.observe_supplemental_movement(self.touch.held_frame());
        }
        let mut handled = event.handled;
        if let Some(delta) = event.look_delta {
            handled |= self.route_touch_look_input(delta, event_loop);
        }
        if let Some(frame) = event.frame {
            handled |= self.route_flat_frame_input(frame, event_loop);
        }
        if handled {
            self.schedule_next_redraw(event_loop);
        }
        handled
    }

    fn apply_input_outcome(
        &mut self,
        context: &str,
        result: Result<WinitInputOutcome>,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                log::error!("failed to route desktop {context}: {error:#}");
                event_loop.exit();
                return true;
            }
        };
        let handled = result.scene.handled;
        let material = handled
            || result.scene.clear_transient_input
            || result.controller_activity
            || result.host != WinitHostEffectOutcome::default();
        if result.controller_activity {
            self.flat_input.note_gamepad_activity();
        }
        if material {
            self.apply_host_effect_outcome(
                result.host,
                result.scene.clear_transient_input,
                event_loop,
            );
        }
        handled
    }

    fn gui_scale(&self) -> Option<GuiScale> {
        self.surface.as_ref().map(|surface| {
            GuiScale::from_pixels(surface.config.width.max(1), surface.config.height.max(1))
        })
    }

    fn gui_point(&self, x: f64, y: f64) -> Option<Point> {
        let surface = self.surface.as_ref()?;
        let window_size = self
            .window
            .as_ref()
            .map(|window| window.inner_size())
            .map(|size| [size.width, size.height])
            .unwrap_or([surface.config.width, surface.config.height]);
        Some(gui_point_from_physical_cursor(
            (x, y),
            window_size,
            [surface.config.width, surface.config.height],
        ))
    }

    fn log_ui_v2_pointer_debug(
        &mut self,
        phase: &str,
        raw_cursor: (f64, f64),
        gui_point: Point,
        action: Option<GameUiAction>,
    ) {
        if !self.ui_v2_hit_debug
            || !self
                .scene_driver
                .as_ref()
                .is_some_and(WinitFrameDriver::ui_v2_is_active)
        {
            return;
        }
        let window_size = self
            .window
            .as_ref()
            .map(|window| window.inner_size())
            .map(|size| [size.width, size.height])
            .unwrap_or([0, 0]);
        let surface_size = self
            .surface
            .as_ref()
            .map(|surface| [surface.config.width, surface.config.height])
            .unwrap_or([0, 0]);
        let Some(snapshot) = self
            .scene_driver
            .as_mut()
            .and_then(WinitFrameDriver::ui_v2_debug_snapshot)
        else {
            return;
        };
        let widgets = snapshot
            .widgets
            .iter()
            .map(|widget| {
                format!(
                    "{:?} '{}' enabled={} rect=({:.1},{:.1},{:.1},{:.1})",
                    widget.id,
                    widget.label,
                    widget.enabled,
                    widget.rect.x,
                    widget.rect.y,
                    widget.rect.width,
                    widget.rect.height
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        log::info!(
            "ui_v2_hit {phase} raw=({:.1},{:.1}) window={}x{} surface={}x{} gui_scale={} gui=({:.1},{:.1}) screen={:?} frame_rev={} layout_rev={} hovered={:?} captured={:?} action={:?} widgets=[{}]",
            raw_cursor.0,
            raw_cursor.1,
            window_size[0],
            window_size[1],
            surface_size[0],
            surface_size[1],
            snapshot.scale.scale,
            gui_point.x,
            gui_point.y,
            snapshot.screen,
            snapshot.frame_revision,
            snapshot.layout_revision,
            snapshot.hovered,
            snapshot.captured,
            action,
            widgets
        );
    }

    fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        event_loop: &ActiveEventLoop,
        from_pointer_click: bool,
    ) {
        let Some(surface) = self.surface.as_ref() else {
            return;
        };
        let Some(driver) = self.scene_driver.as_mut() else {
            return;
        };
        let result = match driver.apply_ui_action(
            action,
            from_pointer_click,
            &surface.device,
            &surface.queue,
        ) {
            Ok(result) => result,
            Err(error) => {
                log::error!("failed to apply desktop scene UI action: {error:#}");
                self.schedule_next_redraw(event_loop);
                return;
            }
        };

        self.apply_host_effect_outcome(result.host, result.scene.clear_gameplay_input, event_loop);
    }

    fn apply_host_effect_outcome(
        &mut self,
        outcome: WinitHostEffectOutcome,
        clear_gameplay_input: bool,
        event_loop: &ActiveEventLoop,
    ) {
        if outcome.exit {
            event_loop.exit();
            return;
        }
        if outcome.quit_to_title {
            self.mouse_lock_requested = false;
        }
        if outcome.cycle_frame_pacing {
            self.frame_pacing.cycle_mode();
            self.next_redraw_at = None;
            if let Some(surface) = &mut self.surface {
                self.frame_pacing.apply_to_surface(surface);
            }
        }
        if outcome.cycle_fps_cap {
            self.frame_pacing.cycle_fps_cap();
            self.next_redraw_at = None;
        }
        if let Some(mode) = outcome.world_render_scale_mode {
            self.world_render_scale_mode = mode;
            self.apply_world_render_scale();
        }
        if let Some(mode) = outcome.touch_controls_mode {
            self.input_preferences.touch_controls = mode;
            if mode == TouchControlsMode::Off {
                self.touch.clear();
            }
        }
        if let Some(settings) = self
            .scene_driver
            .as_ref()
            .and_then(|driver| driver.host().mono_ui_render_state().touch_settings)
        {
            self.touch
                .set_look_sensitivity(settings.clamped_look_sensitivity());
        }
        self.persist_input_preferences_if_changed();
        if let Some(mouse_lock_requested) = outcome.mouse_lock_requested {
            self.mouse_lock_requested = mouse_lock_requested;
        }
        if clear_gameplay_input {
            // Keep the cursor position: winit's button-release event has no
            // coordinates, and the UI needs the last position to end capture.
            self.clear_flat_gameplay_input();
        }
        self.sync_mouse_lock();
        self.schedule_next_redraw(event_loop);
    }

    fn persist_input_preferences_if_changed(&mut self) {
        let current = desktop_touch_preferences(
            &self.client_input_preferences,
            self.input_preferences.touch_controls,
            self.touch.settings.look_sensitivity,
        );
        if current == self.client_input_preferences {
            return;
        }
        match mclone_app_runtime::input_preferences::store_native_input_preferences(
            self.scene.world_root.as_deref(),
            &current,
        ) {
            Ok(true) => {}
            Ok(false) => {
                log::debug!("desktop input preferences have no configured storage path");
            }
            Err(error) => {
                log::warn!("failed to store desktop input preferences: {error:#}");
            }
        }
        self.client_input_preferences = current;
    }

    fn sync_mouse_lock(&mut self) {
        self.set_mouse_lock(
            self.mouse_lock_requested
                && self
                    .scene_driver
                    .as_ref()
                    .is_some_and(|driver| !driver.ui_is_active() && driver.has_runtime()),
        );
    }

    fn toggle_walk_fly_movement_mode(&mut self) {
        if let Some(driver) = &mut self.scene_driver {
            let movement_mode = driver.toggle_walk_fly_movement_mode();
            log::info!("player movement mode {}", movement_mode.label());
        }
    }

    fn rebuild_render_resources(&mut self, asset_source: &impl AssetSource) -> Result<()> {
        let Some(surface) = self.surface.as_ref() else {
            return Ok(());
        };
        let Some(driver) = self.scene_driver.as_mut() else {
            return Ok(());
        };
        driver.rebuild_render_resources(&surface.device, &surface.queue, &self.assets, asset_source)
    }

    fn trigger_render_resource_rebuild(&mut self, event_loop: &ActiveEventLoop) {
        let result = load_asset_source()
            .context("failed to load assets for render resource rebuild")
            .and_then(|asset_source| self.rebuild_render_resources(&asset_source));
        match result {
            Ok(()) => {
                log::info!("desktop flat render resource rebuild trigger completed");
                self.schedule_next_redraw(event_loop);
            }
            Err(err) => {
                log::error!("desktop flat render resource rebuild trigger failed: {err:#}");
            }
        }
    }

    fn trigger_render_scale_rebuild(&mut self, event_loop: &ActiveEventLoop) {
        self.world_render_scale_mode = self.world_render_scale_mode.next();
        self.apply_world_render_scale();
        let next_scale = self.current_render_scale();
        let result = load_asset_source()
            .context("failed to load assets for render-scale rebuild")
            .and_then(|asset_source| self.rebuild_render_resources(&asset_source));
        match result {
            Ok(()) => {
                log::info!("desktop flat render scale set to {next_scale:.2}");
                self.schedule_next_redraw(event_loop);
            }
            Err(err) => {
                log::error!("desktop flat render scale rebuild failed: {err:#}");
            }
        }
    }

    fn set_mouse_lock(&mut self, should_lock: bool) {
        let Some(window) = &self.window else {
            self.mouse_locked = false;
            return;
        };
        if should_lock == self.mouse_locked {
            return;
        }

        if should_lock {
            let grab_result =
                window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|locked_err| {
                        log::warn!(
                            "cursor lock unavailable ({locked_err}); trying confined cursor grab"
                        );
                        window.set_cursor_grab(CursorGrabMode::Confined)
                    });
            match grab_result {
                Ok(()) => {
                    window.set_cursor_visible(false);
                    self.mouse_locked = true;
                    self.last_cursor = None;
                }
                Err(err) => {
                    log::warn!("failed to grab cursor for mouse look: {err}");
                    window.set_cursor_visible(true);
                    self.mouse_locked = false;
                }
            }
        } else {
            if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                log::warn!("failed to release cursor grab: {err}");
            }
            window.set_cursor_visible(true);
            self.mouse_locked = false;
            self.last_cursor = None;
        }
    }
}

impl ApplicationHandler for ChunkApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title("mclone native")
            .with_inner_size(PhysicalSize::new(
                self.window_options.initial_width,
                self.window_options.initial_height,
            ));
        if self.window_options.platform_profile == WindowPlatformProfile::SteamOs {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                log::error!("failed to create native window: {err}");
                event_loop.exit();
                return;
            }
        };
        let mut surface = match NativeSurfaceContext::new_with_color_profile(
            window.clone(),
            self.render_options.color_profile,
        ) {
            Ok(surface) => surface,
            Err(err) => {
                log::error!("failed to initialize native GPU: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let render_scale = world_render_scale(
            self.window_options.platform_profile,
            [surface.config.width, surface.config.height],
            self.world_render_scale_mode,
        );
        surface.render_config = surface.render_config.with_render_scale(render_scale);
        let initial_world_size = scaled_frame_size(
            [surface.config.width, surface.config.height],
            surface.render_config.render_scale,
        );
        log::info!(
            "native presentation profile={} fullscreen={} window_scale_factor={:.3} output={}x{} world={}x{} render_scale={:.3} native_ui=true",
            self.window_options.platform_profile.label(),
            window.fullscreen().is_some(),
            window.scale_factor(),
            surface.config.width,
            surface.config.height,
            initial_world_size[0],
            initial_world_size[1],
            surface.render_config.render_scale,
        );
        self.frame_pacing.update_monitor(&window);
        self.frame_pacing.apply_to_surface(&mut surface);
        let asset_source = match load_asset_source() {
            Ok(source) => source,
            Err(err) => {
                log::error!("failed to load assets for screen effects: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let mut scene_driver = match WinitFrameDriver::new(
            &surface.device,
            &surface.queue,
            surface.render_config,
            [surface.config.width, surface.config.height],
            &self.scene,
            self.render_options,
            &self.assets,
            &asset_source,
            self.start_intent.resolve(&self.scene),
            self.ui_v2_hit_debug,
            &self.controller_preferences,
            self.frame_report
                .as_ref()
                .is_some_and(|report| report.options.freeze_scheduled_fluid_ticks),
        ) {
            Ok(driver) => driver,
            Err(err) => {
                log::error!("failed to initialize desktop scene host: {err:#}");
                event_loop.exit();
                return;
            }
        };
        if let Some(camera_pose) = self
            .frame_report
            .as_ref()
            .and_then(|recorder| recorder.options.camera_pose)
            && let Err(error) = scene_driver.set_capture_camera_pose(camera_pose)
        {
            log::error!("failed to apply window report camera pose: {error:#}");
            event_loop.exit();
            return;
        }
        if self.start_intent == WindowStartIntent::InWorld
            && self.startup_wait == StartupWaitPolicy::ViewSettled
            && let Err(error) = scene_driver.drive_until_view_settled(
                &surface.device,
                &surface.queue,
                DEFAULT_STARTUP_READINESS_TIMEOUT,
            )
        {
            log::error!("failed to complete view-settled desktop startup: {error:#}");
            event_loop.exit();
            return;
        }
        if let Some(camera_pose) = self
            .frame_report
            .as_ref()
            .and_then(|recorder| recorder.options.camera_pose)
        {
            if let Err(error) = scene_driver.set_capture_camera_pose(camera_pose) {
                log::error!("failed to restore window report camera pose after startup: {error:#}");
                event_loop.exit();
                return;
            }
            if let Err(error) = scene_driver.reconcile_capture_camera_pose() {
                log::error!("failed to reconcile window report camera pose: {error:#}");
                event_loop.exit();
                return;
            }
            if self.startup_wait == StartupWaitPolicy::ViewSettled
                && let Err(error) = scene_driver.drive_until_view_settled(
                    &surface.device,
                    &surface.queue,
                    DEFAULT_STARTUP_READINESS_TIMEOUT,
                )
            {
                log::error!("failed to settle restored window report camera pose: {error:#}");
                event_loop.exit();
                return;
            }
        }
        let audio = match AudioEngine::new(&asset_source, AudioSettings::default()) {
            Ok(audio) => Some(audio),
            Err(err) => {
                log::warn!("audio disabled: {err:#}");
                None
            }
        };
        scene_driver.set_audio_engine(audio);
        self.scene_driver = Some(scene_driver);
        self.surface = Some(surface);
        if self.scene_driver.is_none() {
            event_loop.exit();
            return;
        }
        self.window = Some(window);
        self.last_frame = Instant::now();
        self.frame_timing = FrameTimingStats::default();
        self.next_redraw_at = None;
        event_loop.listen_device_events(DeviceEvents::WhenFocused);
        self.sync_mouse_lock();
        self.schedule_next_redraw(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    surface.resize(size);
                }
                self.apply_world_render_scale();
                if let Some(surface) = &self.surface {
                    if let Some(driver) = &mut self.scene_driver {
                        driver.resize(
                            &surface.device,
                            [surface.config.width, surface.config.height],
                        );
                    }
                }
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    if let Some(scale) = self.gui_scale() {
                        if let Some(driver) = &mut self.scene_driver {
                            driver.host_mut().set_mono_ui_scale(scale);
                        }
                    }
                    self.flat_input.note_keyboard_activity();
                    if key_code == KeyCode::Backquote
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.apply_ui_action(
                            GameUiAction::ToggleDebugDiagnostics,
                            event_loop,
                            false,
                        );
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == RENDER_RESOURCE_REBUILD_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.trigger_render_resource_rebuild(event_loop);
                        return;
                    }
                    if key_code == RENDER_SCALE_REBUILD_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.trigger_render_scale_rebuild(event_loop);
                        return;
                    }
                    if key_code == WORLDGEN_LENS_TOGGLE_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        let active = self
                            .scene_driver
                            .as_mut()
                            .and_then(|driver| driver.host_mut().toggle_worldgen_lens());
                        log::info!(
                            "worldgen lens {}",
                            active.map_or("OFF", |layer| layer.label())
                        );
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == WORLDGEN_LENS_CYCLE_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        if let Some(driver) = self.scene_driver.as_mut() {
                            let layer = driver.host_mut().cycle_worldgen_lens();
                            log::info!("worldgen lens {}", layer.label());
                        }
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == FRAME_PIPELINE_OVERLAY_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.apply_ui_action(
                            GameUiAction::ToggleFramePipelineOverlay,
                            event_loop,
                            false,
                        );
                        return;
                    }
                    if key_code == DESKTOP_BLINK_DEBUG_KEY && !event.repeat {
                        let status = match event.state {
                            ElementState::Pressed => {
                                let armed = self
                                    .scene_driver
                                    .as_mut()
                                    .is_some_and(WinitFrameDriver::begin_blink_debug);
                                log::debug!("desktop Blink preview armed={armed}");
                                None
                            }
                            ElementState::Released => self
                                .scene_driver
                                .as_mut()
                                .map(WinitFrameDriver::commit_blink_debug),
                        };
                        match status {
                            Some(Ok(MonoBlinkCommitStatus::Committed {
                                target_feet,
                                changed,
                            })) => log::info!(
                                "desktop Blink committed feet=({:.2}, {:.2}, {:.2}) changed={changed}",
                                target_feet.x,
                                target_feet.y,
                                target_feet.z
                            ),
                            Some(Ok(MonoBlinkCommitStatus::NoValidPreview { validity })) => {
                                log::info!("desktop Blink had no valid preview: {validity:?}")
                            }
                            Some(Ok(status)) => log::debug!("desktop Blink ignored: {status:?}"),
                            Some(Err(error)) => {
                                log::error!("desktop Blink commit failed: {error:#}")
                            }
                            None => {}
                        }
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == DEBUG_PHYSICS_CUBE_SHOOT_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        match self
                            .scene_driver
                            .as_mut()
                            .map_or(Ok(false), WinitFrameDriver::shoot_debug_physics_cube)
                        {
                            Ok(true) => log::info!("shot debug physics cube"),
                            Ok(false) => log::debug!("debug physics cube shot ignored"),
                            Err(err) => log::error!("failed to shoot debug physics cube: {err:#}"),
                        }
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == KeyCode::KeyO
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.apply_ui_action(
                            GameUiAction::ToggleSectionOcclusion,
                            event_loop,
                            false,
                        );
                        return;
                    }
                    if key_code == KeyCode::KeyL
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.apply_ui_action(GameUiAction::ToggleFullbright, event_loop, false);
                        return;
                    }
                    if key_code == NO_CLIP_TOGGLE_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.toggle_walk_fly_movement_mode();
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if let Some((key, pressed, repeat)) = self.flat_input.normalize_keyboard_input(
                        key_code,
                        event.state,
                        event.repeat,
                    ) && self.route_key_input(key, pressed, repeat, event_loop)
                    {
                        return;
                    }
                    self.schedule_next_redraw(event_loop);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some((button, pressed)) = self.flat_input.normalize_mouse_button(button, state)
                else {
                    return;
                };
                if self
                    .scene_driver
                    .as_ref()
                    .is_some_and(WinitFrameDriver::ui_is_active)
                {
                    let point = self.last_cursor.and_then(|(x, y)| self.gui_point(x, y));
                    self.route_pointer_button_input(button, pressed, point, event_loop);
                    if let (Some(raw), Some(point)) = (self.last_cursor, point) {
                        self.log_ui_v2_pointer_debug(
                            if pressed { "down" } else { "up" },
                            raw,
                            point,
                            None,
                        );
                    }
                    return;
                }
                if !self
                    .scene_driver
                    .as_ref()
                    .is_some_and(WinitFrameDriver::has_runtime)
                {
                    self.mouse_lock_requested = false;
                    self.sync_mouse_lock();
                    self.schedule_next_redraw(event_loop);
                    return;
                }
                if pressed {
                    let was_locked = self.mouse_locked;
                    self.mouse_lock_requested = true;
                    self.last_cursor = None;
                    self.sync_mouse_lock();
                    if was_locked {
                        self.route_pointer_button_input(button, true, None, event_loop);
                    }
                    self.schedule_next_redraw(event_loop);
                } else {
                    if self.mouse_locked {
                        self.route_pointer_button_input(button, false, None, event_loop);
                    }
                    self.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.flat_input.note_mouse_activity();
                let cursor = (position.x, position.y);
                if self
                    .scene_driver
                    .as_ref()
                    .is_some_and(WinitFrameDriver::ui_is_active)
                {
                    self.last_cursor = Some(cursor);
                    if let Some(point) = self.gui_point(cursor.0, cursor.1) {
                        self.route_pointer_move_input(point, event_loop);
                        self.log_ui_v2_pointer_debug("move", cursor, point, None);
                    }
                    return;
                }
                if self.mouse_lock_requested && !self.mouse_locked {
                    if let Some(previous) = self.last_cursor {
                        let dx = (cursor.0 - previous.0) as f32;
                        let dy = (cursor.1 - previous.1) as f32;
                        let (dx, dy) = self.flat_input.normalize_mouse_motion(dx, dy);
                        self.route_mouse_motion_input(dx, dy, event_loop);
                    }
                }
                self.last_cursor = Some(cursor);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let direction = self.flat_input.normalize_mouse_wheel(delta);
                self.route_mouse_wheel_input(direction, event_loop);
            }
            WindowEvent::Touch(touch) => self.handle_touch_input(touch, event_loop),
            WindowEvent::Focused(false) => {
                self.mouse_lock_requested = false;
                self.clear_flat_gameplay_input();
                self.last_cursor = None;
                self.ui_touch.clear();
                if let Some(driver) = &mut self.scene_driver {
                    driver.clear_ui_input();
                }
                self.set_mouse_lock(false);
            }
            WindowEvent::Focused(true) => {
                self.sync_mouse_lock();
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                self.poll_controller_input(event_loop);
                if let Some(window) = &self.window {
                    self.frame_pacing.update_monitor(window);
                }
                if let Err(err) = self.update_camera_from_keys(frame_start) {
                    log::error!("failed to update spectator camera: {err:#}");
                    event_loop.exit();
                    return;
                }
                let flat_presentation = self.surface.as_ref().map(|surface| {
                    let output_size = [surface.config.width, surface.config.height];
                    GameFlatPresentationState::new(
                        output_size,
                        scaled_frame_size(output_size, surface.render_config.render_scale),
                        surface.render_config.render_scale,
                        self.world_render_scale_mode,
                    )
                });
                let resolved_input = self
                    .flat_input
                    .capability_state
                    .resolve(self.input_preferences);
                let touch_settings_available = resolved_input.accepts_touch;
                let ui_context = MonoUiContext {
                    resolved_input,
                    frame_pacing: self.frame_pacing.ui_state(),
                    pacing_debug: self.frame_pacing.debug_stats(),
                    frame_timing: self.frame_timing,
                    render_scale: self.current_render_scale(),
                    flat_presentation,
                    hud_visible: true,
                    touch_overlay: self.touch_overlay(),
                    touch_controls_mode: touch_settings_available
                        .then_some(self.input_preferences.touch_controls),
                    touch_settings: touch_settings_available.then_some(GameTouchSettings::new(
                        self.touch.settings.look_sensitivity,
                        TouchInputSettings::MIN_LOOK_SENSITIVITY,
                        TouchInputSettings::MAX_LOOK_SENSITIVITY,
                    )),
                    ..MonoUiContext::default()
                };
                let render_start = Instant::now();
                let mut scene_summary = None;
                let result = {
                    let Some(surface) = &mut self.surface else {
                        return;
                    };
                    let Some(driver) = &mut self.scene_driver else {
                        return;
                    };
                    self.frame_pacing.apply_to_surface(surface);
                    surface.render_with_report(|frame| {
                        scene_summary = Some(driver.render(frame, ui_context)?);
                        Ok(())
                    })
                };
                match result {
                    Ok(report) => {
                        let frame_wall_ms = elapsed_ms(frame_start.elapsed());
                        let rendered = scene_summary.is_some();
                        if let Some(summary) = scene_summary.as_ref() {
                            self.frame_timing
                                .record_runtime_poll(summary.timing.runtime_poll_ms);
                            self.frame_timing.record_remesh_upload(
                                summary.timing.runtime_sync_ms,
                                summary.timing.runtime_gpu_upload_ms,
                            );
                        }
                        self.frame_timing.record_surface_frame(
                            elapsed_ms(render_start.elapsed()),
                            report.acquire_ms,
                            report.encode_ms,
                            report.submit_ms,
                            report.present_ms,
                        );
                        let frame_report_ready = self.record_window_frame_report_sample(
                            report.status,
                            scene_summary.as_ref(),
                        );
                        if let Some(driver) = &mut self.scene_driver {
                            driver.record_frame_pipeline(frame_wall_ms, rendered, scene_summary);
                        }
                        if self.mouse_locked
                            && self
                                .scene_driver
                                .as_ref()
                                .is_some_and(WinitFrameDriver::ui_is_active)
                        {
                            self.mouse_lock_requested = false;
                            self.sync_mouse_lock();
                            self.clear_flat_gameplay_input();
                        }
                        if let Some(requested) = self
                            .scene_driver
                            .as_mut()
                            .and_then(WinitFrameDriver::take_deferred_mouse_lock_request)
                        {
                            self.mouse_lock_requested = requested;
                            self.sync_mouse_lock();
                        }
                        match report.status {
                            SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped => {
                                if frame_report_ready {
                                    if let Err(err) = self.write_window_frame_report() {
                                        log::error!("failed to write window frame report: {err:#}");
                                    }
                                    event_loop.exit();
                                    return;
                                }
                                self.finish_redraw(event_loop, frame_start);
                            }
                            SurfaceFrameStatus::Reconfigured => {
                                if frame_report_ready {
                                    if let Err(err) = self.write_window_frame_report() {
                                        log::error!("failed to write window frame report: {err:#}");
                                    }
                                    event_loop.exit();
                                    return;
                                }
                                self.schedule_next_redraw(event_loop);
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("render failed: {err:#}");
                        event_loop.exit();
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if !self.mouse_locked
            || self
                .scene_driver
                .as_ref()
                .is_none_or(|driver| driver.ui_is_active() || !driver.has_runtime())
        {
            return;
        }
        if let DeviceEvent::MouseMotion { delta } = event {
            let dx = delta.0 as f32;
            let dy = delta.1 as f32;
            let (dx, dy) = self.flat_input.normalize_mouse_motion(dx, dy);
            self.route_mouse_motion_input(dx, dy, event_loop);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.continuous_redraw_required() {
            self.schedule_next_redraw(event_loop);
        } else {
            self.poll_static_controller_if_due(event_loop);
            if self.continuous_redraw_required() {
                self.schedule_next_redraw(event_loop);
            } else {
                self.wait_for_product_event(event_loop);
            }
        }
    }
}

fn desktop_keyboard_key_from_key_code(key_code: KeyCode) -> Option<KeyboardKey> {
    if matches!(
        key_code,
        KeyCode::KeyN
            | KeyCode::KeyO
            | KeyCode::KeyL
            | KeyCode::F3
            | KeyCode::F4
            | KeyCode::F8
            | KeyCode::F9
    ) {
        return None;
    }
    KeyboardKey::from_code_name(&format!("{key_code:?}"))
}

fn desktop_pointer_button_from_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}

fn touch_contact_phase(phase: TouchPhase) -> TouchContactPhase {
    match phase {
        TouchPhase::Started => TouchContactPhase::Started,
        TouchPhase::Moved => TouchContactPhase::Moved,
        TouchPhase::Ended => TouchContactPhase::Ended,
        TouchPhase::Cancelled => TouchContactPhase::Cancelled,
    }
}

fn point_from_vec2(point: Vec2) -> Point {
    Point {
        x: point.x,
        y: point.y,
    }
}

fn gui_point_from_physical_cursor(
    cursor: (f64, f64),
    window_size: [u32; 2],
    surface_size: [u32; 2],
) -> Point {
    let surface_width = surface_size[0].max(1);
    let surface_height = surface_size[1].max(1);
    let window_width = window_size[0].max(1);
    let window_height = window_size[1].max(1);
    let surface_x = cursor.0 * f64::from(surface_width) / f64::from(window_width);
    let surface_y = cursor.1 * f64::from(surface_height) / f64::from(window_height);
    GuiScale::from_pixels(surface_width, surface_height).client_to_gui(surface_x, surface_y)
}

#[cfg(test)]
mod tests;
