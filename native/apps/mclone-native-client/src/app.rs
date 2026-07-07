use std::fs;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use mclone_app_runtime::{RuntimePollDiagnostics, debug_block_palette_overlay, debug_hotbar_icons};
use mclone_assets::AssetSource;
#[cfg(test)]
use mclone_core::Vec3d;
use mclone_input::{
    FlatInputAction, FlatInputFrame, InputCapabilities, InputCapabilityState, InputDeviceKind,
    InputPreferences, KeyboardKey, KeyboardMouseInputAdapter, PointerButton,
};
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render::color_profile::{DEFAULT_RENDER_SCALE, RenderConfig};
use mclone_render::native::{NativeSurfaceContext, SurfaceFrameStatus};
use mclone_ui::{
    DEFAULT_JOIN_REMOTE_ADDR, FlatHotbarOverlay, FlatHud, GameHelpParent, GameScreen, GameUiAction,
    GameUiHost, GuiKey, GuiScale, Point, StatusOverlay,
};
use serde_json::{Value, json};
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::MAX_RENDER_DISTANCE;
use crate::cli::{SceneOptions, StartupWaitPolicy, WindowFrameReportOptions, WindowStartIntent};
use crate::flat_client_driver::{
    DesktopBlinkDebugCommitStatus, FlatClientCameraView, FlatClientDebugFrame, FlatClientDriver,
    FlatClientHostAction, FlatClientUiActionContext, FlatClientUiFrame, FlatClientUiRenderOptions,
    FlatClientWorldActionStatus, game_collision_mode, game_movement_mode,
};
use crate::frame_pacing::{
    FramePacing, FramePacingMode, FrameTimingStats, RedrawSchedule, elapsed_ms,
    next_capped_redraw_deadline, redraw_schedule,
};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{
    WindowRuntimeStats, WindowSceneAssets, WindowSceneRuntime, WindowSceneStartupPump,
};
use crate::ui::DebugPaneStats;
use mclone_audio::{AudioEngine, AudioSettings, landing_playback_for_impact};

const NO_CLIP_TOGGLE_KEY: KeyCode = KeyCode::KeyN;
const DESKTOP_BLINK_DEBUG_KEY: KeyCode = KeyCode::KeyT;
const FRAME_PIPELINE_OVERLAY_KEY: KeyCode = KeyCode::F6;
const DEBUG_PHYSICS_CUBE_SHOOT_KEY: KeyCode = KeyCode::F7;
const RENDER_RESOURCE_REBUILD_KEY: KeyCode = KeyCode::F8;
const RENDER_SCALE_REBUILD_KEY: KeyCode = KeyCode::F9;
const DESKTOP_RENDER_SCALE_PRESETS: [f32; 4] = [DEFAULT_RENDER_SCALE, 0.5, 0.75, 1.5];
const RENDER_SCALE_PRESET_EPSILON: f32 = 0.000_1;
const UI_V2_HIT_DEBUG_ENV: &str = "MCLONE_UI_V2_HIT_DEBUG";

pub(crate) fn run_window(
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    start_intent: WindowStartIntent,
    startup_wait: StartupWaitPolicy,
    frame_report: Option<WindowFrameReportOptions>,
) -> Result<()> {
    let assets = WindowSceneAssets::load()?;
    log::info!(
        "native window startup seed={} initial_center=({}, {}) render_distance={} render_compile_workers={} lighting={} cadence={}/{}/{} color_profile={} remote={:?} atlas={}x{} start={:?} startup_wait={:?}",
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
        start_intent,
        startup_wait,
        frame_report,
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn next_desktop_render_scale(current: f32) -> f32 {
    let Some(index) = DESKTOP_RENDER_SCALE_PRESETS
        .iter()
        .position(|scale| (current - *scale).abs() <= RENDER_SCALE_PRESET_EPSILON)
    else {
        return DEFAULT_RENDER_SCALE;
    };
    DESKTOP_RENDER_SCALE_PRESETS[(index + 1) % DESKTOP_RENDER_SCALE_PRESETS.len()]
}

fn gui_key_from_key_code(key_code: KeyCode) -> Option<GuiKey> {
    match key_code {
        KeyCode::Escape => Some(GuiKey::Escape),
        KeyCode::F1 => Some(GuiKey::F1),
        _ => None,
    }
}

#[derive(Clone, Debug)]
struct DesktopFlatInputAdapter {
    capability_state: InputCapabilityState,
    keyboard_mouse: KeyboardMouseInputAdapter,
}

impl Default for DesktopFlatInputAdapter {
    fn default() -> Self {
        Self {
            capability_state: InputCapabilityState::new(InputCapabilities::NONE),
            keyboard_mouse: KeyboardMouseInputAdapter::new(),
        }
    }
}

impl DesktopFlatInputAdapter {
    fn new() -> Self {
        Self::default()
    }

    fn clear_held(&mut self) {
        self.keyboard_mouse.clear_held();
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

    fn handle_keyboard_input(
        &mut self,
        key_code: KeyCode,
        state: ElementState,
        repeat: bool,
    ) -> Option<FlatInputFrame> {
        self.note_keyboard_activity();
        let key = desktop_keyboard_key_from_key_code(key_code)?;
        self.keyboard_mouse
            .handle_key(key, state == ElementState::Pressed, repeat)
            .frame
    }

    fn handle_mouse_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
    ) -> Option<FlatInputFrame> {
        self.note_mouse_activity();
        let button = desktop_pointer_button_from_mouse_button(button)?;
        self.keyboard_mouse
            .handle_mouse_button(button, state == ElementState::Pressed)
            .frame
    }

    fn mouse_look_frame(&mut self, delta_x: f32, delta_y: f32) -> Option<FlatInputFrame> {
        self.note_mouse_activity();
        self.keyboard_mouse.mouse_motion_frame(delta_x, delta_y)
    }

    fn held_frame(&self) -> FlatInputFrame {
        self.keyboard_mouse.held_frame().unwrap_or_default()
    }
}

struct ChunkApp {
    assets: WindowSceneAssets,
    driver: FlatClientDriver,
    flat_input: DesktopFlatInputAdapter,
    input_preferences: InputPreferences,
    frame_pacing: FramePacing,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    audio: Option<AudioEngine>,
    mouse_locked: bool,
    mouse_lock_requested: bool,
    last_cursor: Option<(f64, f64)>,
    ui_v2_hit_debug: bool,
    last_frame: Instant,
    next_redraw_at: Option<Instant>,
    start_intent: WindowStartIntent,
    startup_wait: StartupWaitPolicy,
    frame_report: Option<WindowFrameReportRecorder>,
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
        })
    }
}

#[derive(Debug)]
struct WindowFrameReportRecorder {
    options: WindowFrameReportOptions,
    start: Instant,
    frames: Vec<WindowFrameSample>,
}

impl WindowFrameReportRecorder {
    fn new(options: WindowFrameReportOptions) -> Self {
        Self {
            frames: Vec::with_capacity(options.frames),
            options,
            start: Instant::now(),
        }
    }

    fn record(&mut self, sample: WindowFrameSample) -> bool {
        self.frames.push(sample);
        self.frames.len() >= self.options.frames
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
        let scene = &app.driver.scene;
        let render_options = app.driver.render_options;
        let final_timing = app.driver.frame_timing;
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
            "startup_wait": format!("{:?}", app.startup_wait),
            "simulation_cadence": {
                "host_rate_hz": scene.simulation_cadence.host_rate_hz,
                "gameplay_rate_hz": scene.simulation_cadence.gameplay_rate_hz,
                "physics_rate_hz": scene.simulation_cadence.physics_rate_hz,
            },
        });
        let render_options_json = json!({
            "section_occlusion_culling": render_options.section_occlusion_culling,
            "force_fullbright": render_options.force_fullbright,
            "sky_darken": render_options.sky_darken,
            "fog": format!("{:?}", render_options.fog),
            "color_profile": render_options.color_profile.as_str(),
        });
        let surface_json = app.surface.as_ref().map(|surface| {
            json!({
                "width": surface.config.width,
                "height": surface.config.height,
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
            .driver
            .runtime
            .as_ref()
            .map(|runtime| runtime_stats_json(runtime.stats()));
        let final_scheduler_json = app
            .driver
            .runtime
            .as_ref()
            .map(|runtime| runtime_scheduler_json(runtime.last_poll_diagnostics()));
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
            "requested_frames": self.options.frames,
            "frames": self.frames.len(),
            "elapsed_wall_ms": elapsed_ms(self.start.elapsed()),
            "scene": scene_json,
            "render_options": render_options_json,
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
        "last_simulation_scheduler_tick_ms": stats.last_simulation_scheduler_tick_ms,
        "last_simulation_block_tick_ms": stats.last_simulation_block_tick_ms,
        "last_simulation_fluid_tick_ms": stats.last_simulation_fluid_tick_ms,
        "last_simulation_entity_tick_ms": stats.last_simulation_entity_tick_ms,
    })
}

fn runtime_scheduler_json(diagnostics: RuntimePollDiagnostics) -> Value {
    json!({
        "server_update_queue_depth": diagnostics.server_update_queue_depth,
        "server_update_queue_bytes": diagnostics.server_update_queue_bytes,
        "server_pending_jobs": diagnostics.server_pending_jobs,
        "server_pending_publications": diagnostics.server_pending_publications,
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
    })
}

impl ChunkApp {
    #[cfg(test)]
    fn new(
        scene: SceneOptions,
        assets: WindowSceneAssets,
        render_options: TexturedSectionRenderOptions,
        start_intent: WindowStartIntent,
        startup_wait: StartupWaitPolicy,
    ) -> Self {
        Self::new_with_frame_report(
            scene,
            assets,
            render_options,
            start_intent,
            startup_wait,
            None,
        )
    }

    fn new_with_frame_report(
        scene: SceneOptions,
        assets: WindowSceneAssets,
        render_options: TexturedSectionRenderOptions,
        start_intent: WindowStartIntent,
        startup_wait: StartupWaitPolicy,
        frame_report: Option<WindowFrameReportOptions>,
    ) -> Self {
        let mut ui = match start_intent {
            WindowStartIntent::InWorld => GameUiHost::new_ingame(),
            WindowStartIntent::Menu => GameUiHost::new(),
        };
        ui.set_join_remote_addr(
            scene
                .remote_addr
                .clone()
                .unwrap_or_else(|| DEFAULT_JOIN_REMOTE_ADDR.to_owned()),
        );
        let ui_v2_hit_debug = std::env::var_os(UI_V2_HIT_DEBUG_ENV).is_some();
        let mut driver = FlatClientDriver::new_with_ui(&scene, render_options, ui);
        driver.set_ui_v2_debug_overlay(ui_v2_hit_debug);
        if ui_v2_hit_debug {
            log::info!("{UI_V2_HIT_DEBUG_ENV}=1; UI v2 hit debug overlay/logging enabled");
        }
        Self {
            driver,
            assets,
            flat_input: DesktopFlatInputAdapter::new(),
            input_preferences: InputPreferences::AUTO,
            frame_pacing: FramePacing::default(),
            window: None,
            surface: None,
            audio: None,
            mouse_locked: false,
            mouse_lock_requested: false,
            last_cursor: None,
            ui_v2_hit_debug,
            last_frame: Instant::now(),
            next_redraw_at: None,
            start_intent,
            startup_wait,
            frame_report: frame_report.map(WindowFrameReportRecorder::new),
        }
    }

    fn schedule_next_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

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

    fn finish_redraw(&mut self, event_loop: &ActiveEventLoop, frame_start: Instant) {
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

    fn record_window_frame_report_sample(&mut self, status: SurfaceFrameStatus) -> bool {
        let Some(recorder) = &mut self.frame_report else {
            return false;
        };
        let timing = self.driver.frame_timing;
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
        })
    }

    fn write_window_frame_report(&self) -> Result<()> {
        let Some(recorder) = &self.frame_report else {
            return Ok(());
        };
        recorder.write(self)
    }

    fn window_camera_view(&self) -> FlatClientCameraView {
        self.driver.camera_view()
    }

    fn current_render_distance(&self) -> u32 {
        self.driver
            .current_render_distance(self.driver.scene.render_distance)
    }

    fn current_render_scale(&self) -> f32 {
        let fallback = self
            .surface
            .as_ref()
            .map_or(DEFAULT_RENDER_SCALE, |surface| {
                surface.render_config.render_scale
            });
        self.driver.current_render_scale(fallback)
    }

    fn update_camera_from_keys(&mut self, now: Instant) -> Result<()> {
        let frame_dt = now.duration_since(self.last_frame);
        let movement_dt = frame_dt.as_secs_f32().min(0.05);
        self.last_frame = now;
        self.driver.tick_frame_timing(
            frame_dt.as_secs_f64() * 1000.0,
            self.frame_pacing.target_frame_ms(),
        );

        if self.driver.ui_is_active() {
            return Ok(());
        }

        let frame = self.flat_input.held_frame();
        if self
            .driver
            .apply_held_input_frame(frame, f64::from(movement_dt))
        {
            self.play_landing_events();
            self.commit_player_pose_change()?;
        } else {
            self.play_landing_events();
        }
        self.driver.update_desktop_blink_debug_preview();
        Ok(())
    }

    fn clear_flat_gameplay_input(&mut self) {
        self.flat_input.clear_held();
        self.driver.clear_camera_input();
        self.driver.clear_desktop_blink_debug();
    }

    fn apply_flat_keyboard_frame(
        &mut self,
        frame: FlatInputFrame,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        if frame.open_menu {
            self.mouse_lock_requested = false;
            self.driver.open_pause_menu();
            self.clear_flat_gameplay_input();
            self.last_cursor = None;
            self.sync_mouse_lock();
            self.schedule_next_redraw(event_loop);
            return true;
        }
        if frame.open_block_palette {
            self.mouse_lock_requested = false;
            self.driver.open_block_palette();
            self.clear_flat_gameplay_input();
            self.last_cursor = None;
            self.sync_mouse_lock();
            self.schedule_next_redraw(event_loop);
            return true;
        }
        if frame.open_help {
            self.mouse_lock_requested = false;
            self.driver.open_help(GameHelpParent::Game);
            self.clear_flat_gameplay_input();
            self.last_cursor = None;
            self.sync_mouse_lock();
            self.schedule_next_redraw(event_loop);
            return true;
        }
        if frame.toggle_camera_view {
            let view_mode = self.driver.toggle_camera_view_mode();
            log::info!("camera view mode {}", view_mode.label());
            self.schedule_next_redraw(event_loop);
            return true;
        }
        if let Some(slot) = frame.selected_hotbar_slot {
            if self.driver.select_hotbar_slot(slot) {
                log::info!("selected hotbar slot {}", slot + 1);
            }
            self.schedule_next_redraw(event_loop);
            return true;
        }
        false
    }

    fn apply_flat_world_action_frame(&mut self, frame: FlatInputFrame) -> Result<()> {
        if frame.attack {
            self.handle_world_flat_action(FlatInputAction::Attack)?;
        }
        if frame.use_item {
            self.handle_world_flat_action(FlatInputAction::Use)?;
        }
        Ok(())
    }

    fn apply_flat_look_frame(&mut self, frame: FlatInputFrame, event_loop: &ActiveEventLoop) {
        let changed = self.driver.apply_look_frame(frame);
        let preview_changed = self.driver.update_desktop_blink_debug_preview();
        if changed || preview_changed {
            self.schedule_next_redraw(event_loop);
        }
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
        if !self.ui_v2_hit_debug || !self.driver.ui_v2_is_active() {
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
        let Some(snapshot) = self.driver.ui_v2_debug_snapshot() else {
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

    fn current_flat_hud(&self, status: StatusOverlay, ui_active: bool) -> FlatHud {
        let mut hud = FlatHud::new(
            self.flat_input
                .capability_state
                .resolve(self.input_preferences),
        );
        let palette_active = self.driver.ui_screen() == Some(GameScreen::BlockPalette);
        hud.world_hud_visible = (!ui_active || palette_active) && self.driver.runtime.is_some();
        hud.crosshair_visible =
            self.driver.crosshair_visible && !ui_active && self.driver.runtime.is_some();
        hud.hotbar = FlatHotbarOverlay::selected_with_icons(
            self.driver.interaction.selected_hotbar_slot(),
            debug_hotbar_icons(
                self.driver.interaction.hotbar_items(),
                &self.assets.mesh_assets.catalog,
            ),
        );
        hud.status = status;
        hud.frame_pipeline = self.driver.latest_frame_pipeline_overlay();
        hud
    }

    fn apply_session_update(&mut self, update: crate::flat_client_driver::FlatClientSessionUpdate) {
        if let Some(mouse_lock_requested) = update.mouse_lock_requested {
            self.mouse_lock_requested = mouse_lock_requested;
            self.sync_mouse_lock();
        }
    }

    fn finish_pending_session_start(&mut self) {
        if self.driver.pending_session_start_needs_teardown() {
            if let Err(err) = self.teardown_world() {
                log::error!(
                    "failed to tear down world before starting replacement session: {err:#}"
                );
                return;
            }
        }
        let device = self.surface.as_ref().map(|surface| &surface.device);
        let assets = &self.assets;
        let update = self.driver.finish_pending_session_start(
            device,
            |scene| WindowSceneRuntime::with_assets(scene, assets),
            |scene| WindowSceneStartupPump::new_local(scene, assets),
        );
        self.apply_session_update(update);
    }

    fn advance_local_world_startup(&mut self) {
        let device = self.surface.as_ref().map(|surface| &surface.device);
        let update = self.driver.advance_local_world_startup(device);
        self.apply_session_update(update);
    }

    fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        event_loop: &ActiveEventLoop,
        from_pointer_click: bool,
    ) {
        let fallback_remote_addr = self.driver.scene.remote_addr.clone();
        let session_starting = self.driver.session.is_starting();
        let result = self.driver.apply_ui_action(
            action,
            FlatClientUiActionContext {
                session_starting,
                from_pointer_click,
                fallback_remote_addr: fallback_remote_addr.as_deref(),
            },
        );

        if let Some(host_action) = result.host_action {
            match host_action {
                FlatClientHostAction::QuitToTitle => {
                    let transition = self.driver.quit_to_title_transition_effects();
                    if transition.teardown_world {
                        if let Err(err) = self.teardown_world() {
                            log::error!("failed to tear down world: {err:#}");
                            self.schedule_next_redraw(event_loop);
                            return;
                        }
                    }
                    self.driver
                        .apply_client_session_transition_effects(transition);
                }
                FlatClientHostAction::Quit => {
                    event_loop.exit();
                    return;
                }
                FlatClientHostAction::CycleFramePacing => {
                    self.frame_pacing.cycle_mode();
                    self.next_redraw_at = None;
                    if let Some(surface) = &mut self.surface {
                        self.frame_pacing.apply_to_surface(surface);
                    }
                    log::info!(
                        "frame pacing set to {} at cap {} fps",
                        self.frame_pacing.mode.label(),
                        self.frame_pacing.fps_cap
                    );
                }
                FlatClientHostAction::CycleFpsCap => {
                    self.frame_pacing.cycle_fps_cap();
                    self.next_redraw_at = None;
                    log::info!("fps cap set to {}", self.frame_pacing.fps_cap);
                }
                FlatClientHostAction::SetTouchControlsMode(mode) => {
                    self.input_preferences.touch_controls = mode;
                }
            }
        }

        if result.session_start_queued {
            self.mouse_lock_requested = false;
        }
        if let Some(mouse_lock_requested) = result.mouse_lock_requested {
            self.mouse_lock_requested = mouse_lock_requested;
        }
        if result.clear_gameplay_input {
            self.clear_flat_gameplay_input();
        }
        if !result.preserve_pointer_state {
            self.last_cursor = None;
        }
        self.sync_mouse_lock();
        self.schedule_next_redraw(event_loop);
    }

    fn sync_mouse_lock(&mut self) {
        self.set_mouse_lock(
            self.mouse_lock_requested
                && !self.driver.ui_is_active()
                && self.driver.runtime.is_some(),
        );
    }

    fn toggle_movement_mode(&mut self) {
        let movement_mode = self.driver.camera.toggle_movement_mode();
        let collision_mode = self.driver.camera.collision_mode();
        log::info!(
            "player movement mode {} collision {}",
            movement_mode.label(),
            collision_mode.label()
        );
    }

    fn rebuild_render_resources(&mut self, asset_source: &impl AssetSource) -> Result<()> {
        let Some(render_config) = self.surface.as_ref().map(|surface| surface.render_config) else {
            return Ok(());
        };
        self.rebuild_render_resources_with_config(asset_source, render_config)
    }

    fn rebuild_render_resources_with_config(
        &mut self,
        asset_source: &impl AssetSource,
        render_config: RenderConfig,
    ) -> Result<()> {
        let Some(surface) = self.surface.as_ref() else {
            return Ok(());
        };
        let (previous_config, next_config) = self.driver.rebuild_render_resources(
            &surface.device,
            &surface.queue,
            [surface.config.width, surface.config.height],
            render_config,
            self.assets.mesh_assets.atlas.as_upload(),
            self.assets.actor_textures.atlas.as_upload(),
            Some(&self.assets.actor_textures.figures),
            asset_source,
        )?;
        if let Some(surface) = &mut self.surface {
            surface.render_config = next_config;
        }
        if self.driver.runtime.is_some() {
            self.upload_all_runtime_sections()
                .context("failed to restore runtime sections after render resource rebuild")?;
        }
        match previous_config {
            Some(previous) if previous == next_config => {
                log::info!(
                    "rebuilt desktop flat render resources with unchanged config: {}",
                    next_config.diagnostic_label()
                );
            }
            Some(previous) => {
                log::info!(
                    "rebuilt desktop flat render resources: {} -> {}",
                    previous.diagnostic_label(),
                    next_config.diagnostic_label()
                );
            }
            None => {
                log::info!(
                    "initialized desktop flat render resources: {}",
                    next_config.diagnostic_label()
                );
            }
        }
        Ok(())
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
        let Some(current_config) = self.surface.as_ref().map(|surface| surface.render_config)
        else {
            return;
        };
        let next_scale = next_desktop_render_scale(current_config.render_scale);
        let next_config = current_config.with_render_scale(next_scale);
        let result = load_asset_source()
            .context("failed to load assets for render-scale rebuild")
            .and_then(|asset_source| {
                self.rebuild_render_resources_with_config(&asset_source, next_config)
            });
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

    fn teardown_world(&mut self) -> Result<()> {
        let device = self.surface.as_ref().map(|surface| &surface.device);
        self.driver.teardown_world(device)?;
        self.flat_input.clear_held();
        self.last_cursor = None;
        self.mouse_lock_requested = false;
        self.set_mouse_lock(false);
        Ok(())
    }

    fn start_world_from_scene(&mut self, scene: SceneOptions) -> Result<()> {
        let assets = &self.assets;
        let device = self.surface.as_ref().map(|surface| &surface.device);
        self.driver
            .start_world_from_scene(scene, device, &mut |scene| {
                WindowSceneRuntime::with_assets(scene, assets)
            })
    }

    #[cfg(test)]
    fn start_local_world(&mut self, seed: i64) -> Result<()> {
        let mut scene = self.driver.scene.clone();
        scene.seed = seed;
        scene.remote_addr = None;
        self.start_world_from_scene(scene)
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

    fn poll_runtime_and_upload(&mut self) -> Result<()> {
        let poll_start = Instant::now();
        let poll = self.driver.poll_runtime()?;
        self.driver
            .record_runtime_poll_timing(elapsed_ms(poll_start.elapsed()));
        if !poll.needs_section_upload {
            return Ok(());
        }
        self.upload_runtime_sections()?;
        Ok(())
    }

    fn upload_runtime_sections(&mut self) -> Result<()> {
        let Some(surface) = &self.surface else {
            return Ok(());
        };
        let Some((sync, summary)) = self
            .driver
            .upload_runtime_sections(&surface.device, elapsed_ms)?
        else {
            return Ok(());
        };
        log::info!(
            "streamed chunks loaded={} sections={} faces={} indices={} rebuilt={} visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} uploaded={} uploaded_vertices={} uploaded_faces={} uploaded_indices={} removed={} remesh_ms={:.3} upload_ms={:.3}",
            sync.loaded_chunk_count,
            summary.section_count,
            summary.face_count,
            summary.index_count,
            sync.section_update.rebuilt_section_count(),
            sync.section_update.visibility_graph_stats.build_count,
            sync.section_update.visibility_graph_stats.total_ms,
            sync.section_update.visibility_graph_stats.worst_ms,
            self.driver.render_stats.last_uploaded_section_count,
            self.driver.render_stats.last_uploaded_vertex_count,
            self.driver.render_stats.last_uploaded_face_count,
            self.driver.render_stats.last_uploaded_index_count,
            self.driver.render_stats.last_upload_removed_section_count,
            sync.remesh_ms,
            self.driver.render_stats.last_upload_ms
        );
        Ok(())
    }

    fn upload_all_runtime_sections(&mut self) -> Result<()> {
        let Some(surface) = &self.surface else {
            return Ok(());
        };
        let Some((sync, summary)) = self
            .driver
            .upload_all_runtime_sections(&surface.device, elapsed_ms)?
        else {
            return Ok(());
        };
        if sync.sections.is_empty() {
            anyhow::bail!(
                "world seed={} center=({}, {}) render_distance={} produced no render sections",
                self.driver.scene.seed,
                self.driver.scene.chunk_x,
                self.driver.scene.chunk_z,
                self.driver.scene.render_distance
            );
        }
        log::info!(
            "uploaded {} world render sections with {} vertices / {} faces / {} indices visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} remesh_ms={:.3} upload_ms={:.3}",
            summary.section_count,
            sync.section_update.rebuilt_vertex_count,
            summary.face_count,
            summary.index_count,
            sync.section_update.visibility_graph_stats.build_count,
            sync.section_update.visibility_graph_stats.total_ms,
            sync.section_update.visibility_graph_stats.worst_ms,
            sync.remesh_ms,
            self.driver.render_stats.last_upload_ms
        );
        Ok(())
    }

    fn debug_pane_stats(&self, render_options: TexturedSectionRenderOptions) -> DebugPaneStats {
        let camera_state = self.driver.camera_frame_state();
        DebugPaneStats {
            position: self.window_camera_view().eye,
            speed: camera_state.camera.speed_blocks_per_second as f32,
            movement_mode: format!(
                "{}/{}",
                camera_state.movement_mode_label(),
                camera_state.collision_mode_label()
            ),
            on_ground: camera_state.on_ground,
            seed: self.driver.scene.seed,
            runtime: self
                .driver
                .runtime
                .as_ref()
                .expect("debug pane requires an active runtime")
                .stats(),
            render: self.driver.render_stats,
            frame: self.driver.frame_timing,
            pacing: self.frame_pacing.debug_stats(),
            section_occlusion: render_options.section_occlusion_culling,
            force_fullbright: render_options.force_fullbright,
            color_profile: render_options.color_profile.label(),
            render_scale: self.current_render_scale(),
        }
    }

    fn commit_player_pose_change(&mut self) -> Result<bool> {
        self.driver.commit_player_pose_change()
    }

    fn play_landing_events(&mut self) {
        let events = self.driver.camera.take_landing_events();
        let Some(audio) = &self.audio else {
            return;
        };
        for event in events {
            let (sound, gain) = landing_playback_for_impact(event.impact_speed);
            audio.play(sound, gain);
        }
    }

    fn handle_world_flat_action(&mut self, action: FlatInputAction) -> Result<()> {
        if let FlatClientWorldActionStatus::Sent { target, changed } =
            self.driver.handle_world_action(action)?
        {
            log::info!(
                "gameplay interaction {:?} at ({}, {}, {}) face={:?} changed={}",
                action,
                target.hit.block_pos.x,
                target.hit.block_pos.y,
                target.hit.block_pos.z,
                target.hit.direction,
                changed
            );
        }
        Ok(())
    }
}

impl ApplicationHandler for ChunkApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("mclone native")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 900));
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
            self.driver.render_options.color_profile,
        ) {
            Ok(surface) => surface,
            Err(err) => {
                log::error!("failed to initialize native GPU: {err:#}");
                event_loop.exit();
                return;
            }
        };
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
        self.driver.set_ui_scale(GuiScale::from_pixels(
            surface.config.width,
            surface.config.height,
        ));
        self.surface = Some(surface);
        if let Err(err) = self.rebuild_render_resources(&asset_source) {
            log::error!("failed to initialize desktop render resources: {err:#}");
            self.surface = None;
            event_loop.exit();
            return;
        }
        let audio = match AudioEngine::new(&asset_source, AudioSettings::default()) {
            Ok(audio) => Some(audio),
            Err(err) => {
                log::warn!("audio disabled: {err:#}");
                None
            }
        };
        self.audio = audio;
        self.window = Some(window);
        self.last_frame = Instant::now();
        self.driver.frame_timing = FrameTimingStats::default();
        self.next_redraw_at = None;
        event_loop.listen_device_events(DeviceEvents::WhenFocused);
        if self.start_intent == WindowStartIntent::InWorld {
            match self.startup_wait {
                StartupWaitPolicy::Idle => {
                    if let Err(err) = self.start_world_from_scene(self.driver.scene.clone()) {
                        log::error!("failed to complete idle desktop startup: {err:#}");
                        event_loop.exit();
                        return;
                    }
                }
                StartupWaitPolicy::None
                | StartupWaitPolicy::Progress
                | StartupWaitPolicy::Playable
                | StartupWaitPolicy::Frames(_) => {
                    self.driver.request_current_scene_start(false, true);
                }
            }
        }
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
                if let Some(surface) = &self.surface {
                    self.driver.resize_frame_targets(
                        &surface.device,
                        [surface.config.width, surface.config.height],
                    );
                    self.driver.set_ui_scale(GuiScale::from_pixels(
                        surface.config.width,
                        surface.config.height,
                    ));
                }
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    if let Some(scale) = self.gui_scale() {
                        self.driver.set_ui_scale(scale);
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
                    if event.state == ElementState::Pressed && self.driver.ui_is_active() {
                        if !event.repeat
                            && let Some(gui_key) = gui_key_from_key_code(key_code)
                        {
                            let (handled, action) = self.driver.ui_key_pressed(gui_key);
                            if let Some(action) = action {
                                self.apply_ui_action(action, event_loop, false);
                            } else if handled {
                                self.schedule_next_redraw(event_loop);
                            }
                        }
                        return;
                    }
                    if key_code == DESKTOP_BLINK_DEBUG_KEY && !event.repeat {
                        match event.state {
                            ElementState::Pressed => {
                                if self.driver.begin_desktop_blink_debug() {
                                    log::info!("desktop Blink debug preview armed");
                                } else {
                                    log::debug!("desktop Blink debug preview ignored");
                                }
                            }
                            ElementState::Released => {
                                match self.driver.commit_desktop_blink_debug() {
                                    Ok(DesktopBlinkDebugCommitStatus::Committed {
                                        target_feet,
                                        changed,
                                    }) => {
                                        log::info!(
                                            "desktop Blink debug committed feet=({:.2}, {:.2}, {:.2}) changed={}",
                                            target_feet.x,
                                            target_feet.y,
                                            target_feet.z,
                                            changed
                                        );
                                        self.play_landing_events();
                                    }
                                    Ok(DesktopBlinkDebugCommitStatus::NoValidPreview {
                                        validity,
                                    }) => {
                                        log::info!(
                                            "desktop Blink debug release had no valid preview: {:?}",
                                            validity
                                        );
                                    }
                                    Ok(status) => {
                                        log::debug!(
                                            "desktop Blink debug release ignored: {:?}",
                                            status
                                        );
                                    }
                                    Err(err) => {
                                        log::error!("desktop Blink debug commit failed: {err:#}");
                                        event_loop.exit();
                                        return;
                                    }
                                }
                            }
                        }
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == DEBUG_PHYSICS_CUBE_SHOOT_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        match self.driver.shoot_debug_physics_cube() {
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
                        self.driver.render_options.section_occlusion_culling =
                            !self.driver.render_options.section_occlusion_culling;
                        log::info!(
                            "section occlusion culling {}",
                            if self.driver.render_options.section_occlusion_culling {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == KeyCode::KeyL
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.driver.render_options.force_fullbright =
                            !self.driver.render_options.force_fullbright;
                        log::info!(
                            "fullbright {}",
                            if self.driver.render_options.force_fullbright {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == NO_CLIP_TOGGLE_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.toggle_movement_mode();
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if let Some(frame) =
                        self.flat_input
                            .handle_keyboard_input(key_code, event.state, event.repeat)
                        && self.apply_flat_keyboard_frame(frame, event_loop)
                    {
                        return;
                    }
                    self.schedule_next_redraw(event_loop);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let input_frame = self.flat_input.handle_mouse_button(button, state);
                if self.driver.ui_is_active() {
                    if button == MouseButton::Left {
                        if let Some((x, y)) = self.last_cursor
                            && let Some(point) = self.gui_point(x, y)
                        {
                            match state {
                                ElementState::Pressed => {
                                    self.driver.ui_pointer_down(point);
                                    self.log_ui_v2_pointer_debug("down", (x, y), point, None);
                                }
                                ElementState::Released => {
                                    let (_handled, action) = self.driver.ui_pointer_up(point);
                                    self.log_ui_v2_pointer_debug("up", (x, y), point, action);
                                    if let Some(action) = action {
                                        self.apply_ui_action(action, event_loop, true);
                                        return;
                                    }
                                }
                            }
                        }
                        self.schedule_next_redraw(event_loop);
                    }
                    return;
                }
                if self.driver.runtime.is_none() {
                    self.mouse_lock_requested = false;
                    self.sync_mouse_lock();
                    self.schedule_next_redraw(event_loop);
                    return;
                }
                if state == ElementState::Pressed {
                    let was_locked = self.mouse_locked;
                    self.mouse_lock_requested = true;
                    self.last_cursor = None;
                    self.sync_mouse_lock();
                    if was_locked && let Some(frame) = input_frame {
                        if let Err(err) = self.apply_flat_world_action_frame(frame) {
                            log::error!("failed to handle world mouse input: {err:#}");
                            event_loop.exit();
                            return;
                        }
                    }
                    self.schedule_next_redraw(event_loop);
                } else if button == MouseButton::Left || button == MouseButton::Right {
                    self.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.flat_input.note_mouse_activity();
                let cursor = (position.x, position.y);
                if self.driver.ui_is_active() {
                    self.last_cursor = Some(cursor);
                    if let Some(point) = self.gui_point(cursor.0, cursor.1) {
                        let (_handled, action) = self.driver.ui_pointer_move(point);
                        self.log_ui_v2_pointer_debug("move", cursor, point, action);
                        if let Some(action) = action {
                            self.apply_ui_action(action, event_loop, true);
                            return;
                        }
                    }
                    self.schedule_next_redraw(event_loop);
                    return;
                }
                if self.mouse_lock_requested && !self.mouse_locked {
                    if let Some(previous) = self.last_cursor {
                        let dx = (cursor.0 - previous.0) as f32;
                        let dy = (cursor.1 - previous.1) as f32;
                        if let Some(frame) = self.flat_input.mouse_look_frame(dx, dy) {
                            self.apply_flat_look_frame(frame, event_loop);
                        }
                    }
                }
                self.last_cursor = Some(cursor);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.flat_input.note_mouse_activity();
                if self.driver.ui_is_active() {
                    return;
                }
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.12,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.001,
                };
                self.driver.adjust_camera_speed(f64::from(amount));
                log::info!(
                    "no-clip speed {:.1} blocks/s",
                    self.driver.camera.snapshot().speed_blocks_per_second
                );
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::Touch(_touch) => {
                self.flat_input.note_touch_activity();
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::Focused(false) => {
                self.mouse_lock_requested = false;
                self.clear_flat_gameplay_input();
                self.last_cursor = None;
                self.driver.clear_ui_input();
                self.set_mouse_lock(false);
            }
            WindowEvent::Focused(true) => {
                self.sync_mouse_lock();
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                if let Some(window) = &self.window {
                    self.frame_pacing.update_monitor(window);
                }
                if let Err(err) = self.update_camera_from_keys(frame_start) {
                    log::error!("failed to update spectator camera: {err:#}");
                    event_loop.exit();
                    return;
                }
                if let Err(err) = self.poll_runtime_and_upload() {
                    log::error!("failed to poll chunk runtime: {err:#}");
                    event_loop.exit();
                    return;
                }
                self.advance_local_world_startup();
                let Some(gui_scale) = self.gui_scale() else {
                    return;
                };
                self.driver.set_ui_scale(gui_scale);
                let render_options = self.driver.effective_render_options();
                let debug_stats = (self.driver.debug_diagnostics_visible
                    && self.driver.runtime.is_some())
                .then(|| self.debug_pane_stats(render_options));
                let session_projection = self.driver.session_projection();
                let flat_hud = self.current_flat_hud(
                    session_projection.status_overlay.clone(),
                    self.driver.ui_is_active(),
                );
                let loading_progress_overlay = session_projection.loading_progress_overlay;
                let debug_view_readiness_overlay = (self.driver.debug_diagnostics_visible
                    && loading_progress_overlay.is_none())
                .then(|| {
                    self.driver
                        .runtime
                        .as_ref()
                        .and_then(WindowSceneRuntime::view_readiness_overlay)
                })
                .flatten();
                let ui_render_options = FlatClientUiRenderOptions {
                    render_distance: i32::try_from(self.current_render_distance())
                        .unwrap_or(MAX_RENDER_DISTANCE),
                    render_options: self.driver.render_options,
                    far_lod_enabled: self.driver.scene.far_lod.enabled,
                    far_lod_range_chunks: self.driver.scene.far_lod.extra_radius_chunks as i32,
                    frame_pacing: self.frame_pacing.ui_state(),
                    movement_mode: game_movement_mode(self.driver.camera.movement_mode()),
                    collision_mode: game_collision_mode(self.driver.camera.collision_mode()),
                    travel_assist_mode: self.driver.travel_assist_mode,
                    fly_speed_multiplier: self.driver.camera.fly_speed_multiplier() as f32,
                    movement_speed_multiplier: self.driver.camera.movement_speed_multiplier()
                        as f32,
                    player_collision_box_visible: self.driver.player_collision_box_visible,
                    first_person_player_visible: self.driver.camera.first_person_player_visible(),
                    crosshair_visible: self.driver.crosshair_visible,
                    frame_pipeline_overlay_visible: self.driver.frame_pipeline_overlay_visible,
                    debug_diagnostics_visible: self.driver.debug_diagnostics_visible,
                    player_model: self.driver.player_model,
                    server_cadence: self.driver.server_simulation_cadence(),
                };
                let ui_frame = FlatClientUiFrame {
                    render_options: ui_render_options,
                    block_palette: debug_block_palette_overlay(
                        &self.assets.mesh_assets.catalog,
                        self.driver.interaction.selected_hotbar_slot(),
                    ),
                    hud: Some(flat_hud),
                    loading_progress_overlay,
                    debug: FlatClientDebugFrame {
                        stats: debug_stats,
                        view_readiness_overlay: debug_view_readiness_overlay,
                    },
                };
                let render_start = Instant::now();
                let result = {
                    let Some(surface) = &mut self.surface else {
                        return;
                    };
                    self.frame_pacing.apply_to_surface(surface);
                    surface.render_with_report(|frame| {
                        self.driver.render_full_frame_with_ui(
                            frame,
                            self.driver.scene.render_distance,
                            ui_frame,
                        )?;
                        Ok(())
                    })
                };
                match result {
                    Ok(report) => {
                        self.driver.record_surface_frame_timing(
                            elapsed_ms(render_start.elapsed()),
                            report.acquire_ms,
                            report.encode_ms,
                            report.submit_ms,
                            report.present_ms,
                        );
                        let frame_report_ready =
                            self.record_window_frame_report_sample(report.status);
                        match report.status {
                            SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped => {
                                self.finish_pending_session_start();
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
        if self.driver.ui_is_active() || !self.mouse_locked || self.driver.runtime.is_none() {
            return;
        }
        if let DeviceEvent::MouseMotion { delta } = event {
            let dx = delta.0 as f32;
            let dy = delta.1 as f32;
            if let Some(frame) = self.flat_input.mouse_look_frame(dx, dy) {
                self.apply_flat_look_frame(frame, event_loop);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.schedule_next_redraw(event_loop);
    }
}

fn desktop_keyboard_key_from_key_code(key_code: KeyCode) -> Option<KeyboardKey> {
    match key_code {
        KeyCode::KeyW => Some(KeyboardKey::KeyW),
        KeyCode::KeyA => Some(KeyboardKey::KeyA),
        KeyCode::KeyS => Some(KeyboardKey::KeyS),
        KeyCode::KeyD => Some(KeyboardKey::KeyD),
        KeyCode::ArrowUp => Some(KeyboardKey::ArrowUp),
        KeyCode::ArrowDown => Some(KeyboardKey::ArrowDown),
        KeyCode::ArrowLeft => Some(KeyboardKey::ArrowLeft),
        KeyCode::ArrowRight => Some(KeyboardKey::ArrowRight),
        KeyCode::KeyE => Some(KeyboardKey::KeyE),
        KeyCode::KeyB => Some(KeyboardKey::KeyB),
        KeyCode::KeyX => Some(KeyboardKey::KeyX),
        KeyCode::Space => Some(KeyboardKey::Space),
        KeyCode::ShiftLeft => Some(KeyboardKey::ShiftLeft),
        KeyCode::ShiftRight => Some(KeyboardKey::ShiftRight),
        KeyCode::ControlLeft => Some(KeyboardKey::ControlLeft),
        KeyCode::ControlRight => Some(KeyboardKey::ControlRight),
        KeyCode::Escape => Some(KeyboardKey::Escape),
        KeyCode::F1 => Some(KeyboardKey::F1),
        KeyCode::F5 => Some(KeyboardKey::F5),
        KeyCode::Digit1 => Some(KeyboardKey::Digit1),
        KeyCode::Digit2 => Some(KeyboardKey::Digit2),
        KeyCode::Digit3 => Some(KeyboardKey::Digit3),
        KeyCode::Digit4 => Some(KeyboardKey::Digit4),
        KeyCode::Digit5 => Some(KeyboardKey::Digit5),
        KeyCode::Digit6 => Some(KeyboardKey::Digit6),
        KeyCode::Digit7 => Some(KeyboardKey::Digit7),
        KeyCode::Digit8 => Some(KeyboardKey::Digit8),
        KeyCode::Digit9 => Some(KeyboardKey::Digit9),
        _ => None,
    }
}

fn desktop_pointer_button_from_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        _ => None,
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
