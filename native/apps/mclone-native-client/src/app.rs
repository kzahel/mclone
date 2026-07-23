use std::fs;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use mclone_app_runtime::frame_render::{MIN_FLAT_RENDER_SCALE, scaled_frame_size};
use mclone_app_runtime::{DEFAULT_STARTUP_READINESS_TIMEOUT, RuntimePollDiagnostics};
use mclone_assets::AssetSource;
use mclone_input::{
    ControllerInputPreferences, InputCapabilities, InputCapabilityState, InputDeviceKind,
    InputPreferences, KeyboardKey, MouseWheelDirection, PointerButton,
};
use mclone_render::chunk::TexturedSectionRenderOptions;
use mclone_render::color_profile::DEFAULT_RENDER_SCALE;
use mclone_render::native::{NativeSurfaceContext, SurfaceFrameStatus};
use mclone_ui::{GameUiAction, GuiScale, Point};
use serde_json::{Value, json};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
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
use mclone_scene::{MonoBlinkCommitStatus, MonoUiContext};

const NO_CLIP_TOGGLE_KEY: KeyCode = KeyCode::KeyN;
const DESKTOP_BLINK_DEBUG_KEY: KeyCode = KeyCode::KeyT;
const FRAME_PIPELINE_OVERLAY_KEY: KeyCode = KeyCode::F6;
const DEBUG_PHYSICS_CUBE_SHOOT_KEY: KeyCode = KeyCode::F7;
const RENDER_RESOURCE_REBUILD_KEY: KeyCode = KeyCode::F8;
const RENDER_SCALE_REBUILD_KEY: KeyCode = KeyCode::F9;
const DESKTOP_RENDER_SCALE_PRESETS: [f32; 4] = [DEFAULT_RENDER_SCALE, 0.5, 0.75, 1.5];
const RENDER_SCALE_PRESET_EPSILON: f32 = 0.000_1;
const STEAMOS_WORLD_RENDER_MAX_HEIGHT: u32 = 1080;
const STEAMOS_WORLD_RENDER_MAX_PIXELS: u64 = 1920 * 1080;
const UI_V2_HIT_DEBUG_ENV: &str = "MCLONE_UI_V2_HIT_DEBUG";

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

fn next_desktop_render_scale(current: f32) -> f32 {
    let Some(index) = DESKTOP_RENDER_SCALE_PRESETS
        .iter()
        .position(|scale| (current - *scale).abs() <= RENDER_SCALE_PRESET_EPSILON)
    else {
        return DEFAULT_RENDER_SCALE;
    };
    DESKTOP_RENDER_SCALE_PRESETS[(index + 1) % DESKTOP_RENDER_SCALE_PRESETS.len()]
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
    fn new() -> Self {
        Self::default()
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
    controller_preferences: ControllerInputPreferences,
    frame_pacing: FramePacing,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    frame_timing: FrameTimingStats,
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
        Self {
            scene: scene.clone(),
            render_options,
            window_options,
            scene_driver: None,
            gamepad_collector,
            assets,
            flat_input: DesktopFlatInputAdapter::new(),
            input_preferences,
            controller_preferences: client_input_preferences.controller,
            frame_pacing: FramePacing::default(),
            window: None,
            surface: None,
            frame_timing: FrameTimingStats::default(),
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

    fn apply_automatic_world_render_scale(&mut self) {
        let Some(surface) = &mut self.surface else {
            return;
        };
        let Some(render_scale) = automatic_world_render_scale(
            self.window_options.platform_profile,
            [surface.config.width, surface.config.height],
        ) else {
            return;
        };
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
            "automatic {} world render scale {:.3}: output={}x{} world={}x{} native_ui=true",
            self.window_options.platform_profile.label(),
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

        let Some(driver) = self.scene_driver.as_mut() else {
            return Ok(());
        };
        driver.advance_held_input(frame_dt.as_secs_f64())?;
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
        self.flat_input
            .set_gamepad_present(poll.connected_count() > 0);
        let result = match (self.surface.as_ref(), self.scene_driver.as_mut()) {
            (Some(surface), Some(driver)) => {
                driver.route_controller_poll(poll, &surface.device, &surface.queue)
            }
            _ => return,
        };
        self.apply_input_outcome("gamepad input", result, event_loop);
    }

    fn clear_flat_gameplay_input(&mut self) {
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
        if result.controller_activity {
            self.flat_input.note_gamepad_activity();
        }
        self.apply_host_effect_outcome(result.host, result.scene.clear_transient_input, event_loop);
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
        if let Some(mode) = outcome.touch_controls_mode {
            self.input_preferences.touch_controls = mode;
        }
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
        let Some(current_config) = self.surface.as_ref().map(|surface| surface.render_config)
        else {
            return;
        };
        let next_scale = next_desktop_render_scale(current_config.render_scale);
        let next_config = current_config.with_render_scale(next_scale);
        if let Some(surface) = &mut self.surface {
            surface.render_config = next_config;
            if let Some(driver) = &mut self.scene_driver {
                driver.set_render_config(
                    &surface.device,
                    [surface.config.width, surface.config.height],
                    next_config,
                );
            }
        }
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
        if let Some(render_scale) = automatic_world_render_scale(
            self.window_options.platform_profile,
            [surface.config.width, surface.config.height],
        ) {
            surface.render_config = surface.render_config.with_render_scale(render_scale);
        }
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
            self.start_intent,
            self.ui_v2_hit_debug,
            &self.controller_preferences,
        ) {
            Ok(driver) => driver,
            Err(err) => {
                log::error!("failed to initialize desktop scene host: {err:#}");
                event_loop.exit();
                return;
            }
        };
        if self.start_intent == WindowStartIntent::InWorld
            && self.startup_wait == StartupWaitPolicy::Idle
            && let Err(error) = scene_driver.drive_until_idle(
                &surface.device,
                &surface.queue,
                DEFAULT_STARTUP_READINESS_TIMEOUT,
            )
        {
            log::error!("failed to complete idle desktop startup: {error:#}");
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
                self.apply_automatic_world_render_scale();
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
            WindowEvent::Touch(_touch) => {
                self.flat_input.note_touch_activity();
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::Focused(false) => {
                self.mouse_lock_requested = false;
                self.clear_flat_gameplay_input();
                self.last_cursor = None;
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
                let ui_context = MonoUiContext {
                    resolved_input: self
                        .flat_input
                        .capability_state
                        .resolve(self.input_preferences),
                    frame_pacing: self.frame_pacing.ui_state(),
                    pacing_debug: self.frame_pacing.debug_stats(),
                    frame_timing: self.frame_timing,
                    render_scale: self.current_render_scale(),
                    hud_visible: true,
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
                        let frame_report_ready =
                            self.record_window_frame_report_sample(report.status);
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
        self.schedule_next_redraw(event_loop);
    }
}

fn desktop_keyboard_key_from_key_code(key_code: KeyCode) -> Option<KeyboardKey> {
    if matches!(
        key_code,
        KeyCode::KeyN | KeyCode::KeyO | KeyCode::KeyL | KeyCode::F8 | KeyCode::F9
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
