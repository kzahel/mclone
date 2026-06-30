use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use mclone_app_runtime::{debug_block_palette_overlay, debug_hotbar_icons};
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
    DEFAULT_JOIN_REMOTE_ADDR, FlatHotbarOverlay, FlatHud, GameHelpParent, GameScreen, GameUi,
    GameUiAction, GameUiRenderState, GuiKey, GuiScale, Point, StatusOverlay,
};
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::MAX_RENDER_DISTANCE;
use crate::cli::{SceneOptions, StartupWaitPolicy, WindowStartIntent};
use crate::flat_client_driver::{
    FlatClientCameraView, FlatClientDebugFrame, FlatClientDriver, FlatClientHostAction,
    FlatClientUiActionContext, FlatClientUiFrame, FlatClientUiRenderOptions,
    FlatClientWorldActionStatus, game_movement_mode, game_ui_render_state,
};
use crate::frame_pacing::{
    FramePacing, FramePacingMode, FrameTimingStats, RedrawSchedule, elapsed_ms,
    next_capped_redraw_deadline, redraw_schedule,
};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowSceneAssets, WindowSceneRuntime, WindowSceneStartupPump};
use crate::ui::DebugPaneStats;
use mclone_audio::{AudioEngine, AudioSettings, landing_playback_for_impact};

const NO_CLIP_TOGGLE_KEY: KeyCode = KeyCode::KeyN;
const DEBUG_PHYSICS_CUBE_SHOOT_KEY: KeyCode = KeyCode::F7;
const RENDER_RESOURCE_REBUILD_KEY: KeyCode = KeyCode::F8;
const RENDER_SCALE_REBUILD_KEY: KeyCode = KeyCode::F9;
const DESKTOP_RENDER_SCALE_PRESETS: [f32; 4] = [DEFAULT_RENDER_SCALE, 0.5, 0.75, 1.5];
const RENDER_SCALE_PRESET_EPSILON: f32 = 0.000_1;

pub(crate) fn run_window(
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    start_intent: WindowStartIntent,
    startup_wait: StartupWaitPolicy,
) -> Result<()> {
    let assets = WindowSceneAssets::load()?;
    log::info!(
        "native window startup seed={} initial_center=({}, {}) render_distance={} lighting={} cadence={}/{}/{} color_profile={} remote={:?} atlas={}x{} start={:?} startup_wait={:?}",
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        scene.render_distance,
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
    let mut app = ChunkApp::new(scene, assets, render_options, start_intent, startup_wait);
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
    debug_visible: bool,
    last_frame: Instant,
    next_redraw_at: Option<Instant>,
    start_intent: WindowStartIntent,
    startup_wait: StartupWaitPolicy,
}

impl ChunkApp {
    fn new(
        scene: SceneOptions,
        assets: WindowSceneAssets,
        render_options: TexturedSectionRenderOptions,
        start_intent: WindowStartIntent,
        startup_wait: StartupWaitPolicy,
    ) -> Self {
        let mut ui = match start_intent {
            WindowStartIntent::InWorld => GameUi::new_ingame(),
            WindowStartIntent::Menu => GameUi::new(),
        };
        ui.set_join_remote_addr(
            scene
                .remote_addr
                .clone()
                .unwrap_or_else(|| DEFAULT_JOIN_REMOTE_ADDR.to_owned()),
        );
        Self {
            driver: FlatClientDriver::new_with_ui(&scene, render_options, ui),
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
            debug_visible: false,
            last_frame: Instant::now(),
            next_redraw_at: None,
            start_intent,
            startup_wait,
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
        Ok(())
    }

    fn clear_flat_gameplay_input(&mut self) {
        self.flat_input.clear_held();
        self.driver.clear_camera_input();
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
        if self.driver.apply_look_frame(frame) {
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

    fn current_ui_render_state(&self) -> GameUiRenderState {
        let mut state = game_ui_render_state(FlatClientUiRenderOptions {
            render_distance: i32::try_from(self.current_render_distance())
                .unwrap_or(MAX_RENDER_DISTANCE),
            render_options: self.driver.render_options,
            frame_pacing: self.frame_pacing.ui_state(),
            movement_mode: game_movement_mode(self.driver.camera.movement_mode()),
            fly_speed_multiplier: self.driver.camera.fly_speed_multiplier() as f32,
            movement_speed_multiplier: self.driver.camera.movement_speed_multiplier() as f32,
            player_collision_box_visible: self.driver.player_collision_box_visible,
            first_person_player_visible: self.driver.camera.first_person_player_visible(),
            player_model: self.driver.player_model,
            server_cadence: self.driver.server_simulation_cadence(),
        });
        state.touch_controls_mode = Some(self.input_preferences.touch_controls);
        state.block_palette = debug_block_palette_overlay(
            &self.assets.mesh_assets.catalog,
            self.driver.interaction.selected_hotbar_slot(),
        );
        state
    }

    fn current_flat_hud(&self, status: StatusOverlay, ui_active: bool) -> FlatHud {
        let mut hud = FlatHud::new(
            self.flat_input
                .capability_state
                .resolve(self.input_preferences),
        );
        let palette_active = self.driver.ui_screen() == Some(GameScreen::BlockPalette);
        hud.world_hud_visible = (!ui_active || palette_active) && self.driver.runtime.is_some();
        hud.crosshair_visible = !ui_active && self.driver.runtime.is_some();
        hud.hotbar = FlatHotbarOverlay::selected_with_icons(
            self.driver.interaction.selected_hotbar_slot(),
            debug_hotbar_icons(
                self.driver.interaction.hotbar_items(),
                &self.assets.mesh_assets.catalog,
            ),
        );
        hud.status = status;
        hud
    }

    fn apply_session_update(&mut self, update: crate::flat_client_driver::FlatClientSessionUpdate) {
        if let Some(mouse_lock_requested) = update.mouse_lock_requested {
            self.mouse_lock_requested = mouse_lock_requested;
            self.sync_mouse_lock();
        }
    }

    fn finish_pending_session_start(&mut self) {
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
                    if let Err(err) = self.teardown_world() {
                        log::error!("failed to tear down world: {err:#}");
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    self.driver.clear_session();
                    self.driver.apply_quit_to_title_ui();
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
        log::info!("player movement mode {}", movement_mode.label());
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
            .frame_timing
            .record_runtime_poll(elapsed_ms(poll_start.elapsed()));
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
            movement_mode: camera_state.movement_mode_label(),
            on_ground: camera_state.on_ground,
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
                        self.debug_visible = !self.debug_visible;
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
                                    let ui_state = self.current_ui_render_state();
                                    self.driver.ui_pointer_down(point, ui_state);
                                }
                                ElementState::Released => {
                                    let ui_state = self.current_ui_render_state();
                                    let (_handled, action) =
                                        self.driver.ui_pointer_up(point, ui_state);
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
                        let ui_state = self.current_ui_render_state();
                        let (_handled, action) = self.driver.ui_pointer_move(point, ui_state);
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
                let debug_stats = (self.debug_visible && self.driver.runtime.is_some())
                    .then(|| self.debug_pane_stats(render_options));
                let status_overlay = self.driver.session_status_overlay();
                let flat_hud = self.current_flat_hud(status_overlay, self.driver.ui_is_active());
                let loading_progress_overlay = self.driver.startup_progress_overlay();
                let debug_view_readiness_overlay = (self.debug_visible
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
                    frame_pacing: self.frame_pacing.ui_state(),
                    movement_mode: game_movement_mode(self.driver.camera.movement_mode()),
                    fly_speed_multiplier: self.driver.camera.fly_speed_multiplier() as f32,
                    movement_speed_multiplier: self.driver.camera.movement_speed_multiplier()
                        as f32,
                    player_collision_box_visible: self.driver.player_collision_box_visible,
                    first_person_player_visible: self.driver.camera.first_person_player_visible(),
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
                        self.driver.frame_timing.record_surface_frame(
                            elapsed_ms(render_start.elapsed()),
                            report.acquire_ms,
                            report.encode_ms,
                            report.submit_ms,
                            report.present_ms,
                        );
                        match report.status {
                            SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped => {
                                self.finish_pending_session_start();
                                self.finish_redraw(event_loop, frame_start);
                            }
                            SurfaceFrameStatus::Reconfigured => {
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
mod tests {
    use super::*;
    use crate::DEFAULT_SEED;
    use crate::flat_client_driver::{
        effective_render_options_for_camera, engine_camera_input_from_flat_frame,
    };
    use mclone_app_runtime::session::{
        GameSessionState, RemoteSessionEndpoint, SessionStartRequest,
    };
    use mclone_client::{ActorInterpolationConfig, ActorInterpolationState};
    use mclone_render_session::{EngineCameraMovementMode, actor_instances_from_presentations};
    use mclone_ui::GameScreen;

    fn test_app_with_runtime(scene: SceneOptions) -> ChunkApp {
        let assets = WindowSceneAssets::load().unwrap();
        let runtime = WindowSceneRuntime::with_assets(&scene, &assets).unwrap();
        let mut app = ChunkApp::new(
            scene,
            assets,
            TexturedSectionRenderOptions::default(),
            WindowStartIntent::InWorld,
            StartupWaitPolicy::DESKTOP_DEFAULT,
        );
        app.driver.runtime = Some(runtime);
        app
    }

    fn center_chunk_signature(runtime: &WindowSceneRuntime, center: mclone_core::ChunkPos) -> u64 {
        let snapshot = runtime
            .client()
            .chunk_snapshot(center)
            .expect("center chunk must be loaded");
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash = hash_signature_value(hash, snapshot.sections.len() as u64);
        for section in &snapshot.sections {
            hash = hash_signature_value(hash, section.section_y as u64);
            for state_id in section.unpack_block_state_ids() {
                hash = hash_signature_value(hash, u64::from(state_id.0));
            }
        }
        hash
    }

    fn hash_signature_value(hash: u64, value: u64) -> u64 {
        (hash ^ value).wrapping_mul(0x0000_0100_0000_01b3)
    }

    #[test]
    fn effective_render_options_disable_section_occlusion_inside_occluding_block() {
        let enabled = TexturedSectionRenderOptions {
            section_occlusion_culling: true,
            ..TexturedSectionRenderOptions::default()
        };
        let disabled = TexturedSectionRenderOptions {
            section_occlusion_culling: false,
            ..TexturedSectionRenderOptions::default()
        };

        assert!(effective_render_options_for_camera(enabled, false).section_occlusion_culling);
        assert!(!effective_render_options_for_camera(enabled, true).section_occlusion_culling);
        assert!(!effective_render_options_for_camera(disabled, true).section_occlusion_culling);
    }

    #[test]
    fn desktop_render_scale_presets_cycle_from_default() {
        assert_eq!(next_desktop_render_scale(1.0), 0.5);
        assert_eq!(next_desktop_render_scale(0.5), 0.75);
        assert_eq!(next_desktop_render_scale(0.75), 1.5);
        assert_eq!(next_desktop_render_scale(1.5), 1.0);
        assert_eq!(next_desktop_render_scale(1.25), 1.0);
    }

    #[test]
    fn gui_point_from_physical_cursor_uses_surface_gui_scale() {
        let point = gui_point_from_physical_cursor((640.0, 450.0), [1280, 900], [1280, 900]);

        assert!((point.x - 640.0 / 3.0).abs() < f32::EPSILON);
        assert_eq!(point.y, 150.0);
    }

    #[test]
    fn gui_point_from_physical_cursor_maps_window_to_surface_size() {
        let point = gui_point_from_physical_cursor((640.0, 360.0), [1280, 720], [2560, 1440]);
        let scale = GuiScale::from_pixels(2560, 1440);

        assert_eq!(
            point,
            Point {
                x: scale.width * 0.5,
                y: scale.height * 0.5,
            }
        );
    }

    #[test]
    fn actor_instances_follow_client_actor_presentations() {
        let integrated = mclone_client::ClientRuntime::local_integrated();
        assert!(integrated.actor_presentations().is_empty());
        assert!(
            actor_instances_from_presentations(&integrated.actor_presentations(), &integrated)
                .is_empty()
        );

        let mut remote =
            mclone_client::ClientRuntime::new(mclone_client::ClientHost::RemoteDedicated);
        remote.apply_update(mclone_protocol::ServerUpdate::RemotePlayerAdd(
            mclone_protocol::RemotePlayerUpdate {
                id: mclone_protocol::RemotePlayerId(9),
                appearance: mclone_protocol::PlayerAppearance::default(),
                position: Vec3d::new(1.0, 64.0, 2.0),
                y_rot_degrees: -90.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            },
        ));

        let mut interpolation =
            ActorInterpolationState::from_authoritative(remote.actor_presentations());
        interpolation.step(1.0, ActorInterpolationConfig::default());
        let actors = actor_instances_from_presentations(&interpolation.presentations(), &remote);

        assert_eq!(actors.len(), 1);
        assert_eq!(actors[0].feet_position, glam::Vec3::new(1.0, 64.0, 2.0));
        assert!((actors[0].yaw_radians - 90.0_f32.to_radians()).abs() < 1.0e-6);
        assert_eq!(
            actors[0].shape,
            mclone_render::entity::ActorInstanceShape::Figure(
                mclone_assets::default_player_figure_id()
            )
        );
        assert_eq!(
            actors[0].packed_light,
            mclone_render::light_texture::FULL_BRIGHT
        );

        let mut entity_client =
            mclone_client::ClientRuntime::new(mclone_client::ClientHost::RemoteDedicated);
        entity_client.apply_update(mclone_protocol::ServerUpdate::EntitySnapshot(
            mclone_protocol::EntitySnapshot {
                id: mclone_protocol::EntityId(1),
                kind: mclone_protocol::EntityKind::Cow,
                position: Vec3d::new(3.0, 64.0, 4.0),
                y_rot_degrees: 45.0,
                x_rot_degrees: 0.0,
                rotation: None,
                on_ground: true,
                width: 0.9,
                height: 1.4,
                age_ticks: 0,
            },
        ));
        let entity_actors = actor_instances_from_presentations(
            &entity_client.actor_presentations(),
            &entity_client,
        );

        assert_eq!(entity_actors.len(), 1);
        assert_eq!(
            entity_actors[0].shape,
            mclone_render::entity::ActorInstanceShape::CowModel
        );
        assert_eq!(entity_actors[0].width, 0.9);
        assert_eq!(entity_actors[0].height, 1.4);
        assert_eq!(
            entity_actors[0].packed_light,
            mclone_render::light_texture::FULL_BRIGHT
        );
    }

    #[test]
    fn commit_player_pose_change_syncs_spectator_and_interest_center() {
        let scene = SceneOptions {
            render_distance: 1,
            ..SceneOptions::default()
        };
        let mut app = test_app_with_runtime(scene);
        let target_eye = Vec3d::new(16.25, 96.0, 8.0);
        app.driver.camera.set_eye_pose(target_eye, 0.0, 0.0);

        assert!(app.commit_player_pose_change().unwrap());

        assert_eq!(
            app.driver.spectator.position,
            glam::Vec3::new(
                target_eye.x as f32,
                target_eye.y as f32,
                target_eye.z as f32
            )
        );
        assert_eq!(
            app.driver.camera_frame_state().camera.chunk_pos,
            mclone_core::ChunkPos::new(1, 0)
        );
        assert_eq!(
            app.driver.runtime.as_ref().unwrap().stats().interest_center,
            mclone_core::ChunkPos::new(1, 0)
        );
    }

    #[test]
    fn window_camera_view_uses_controller_snapshot_not_spectator_mirror() {
        let scene = SceneOptions::default();
        let mut app = test_app_with_runtime(scene);
        let target_eye = Vec3d::new(32.25, 80.0, -0.25);

        app.driver.camera.set_eye_pose(target_eye, 0.25, -0.125);
        let view = app.window_camera_view();

        assert_eq!(
            view.eye,
            glam::Vec3::new(
                target_eye.x as f32,
                target_eye.y as f32,
                target_eye.z as f32
            )
        );
        assert_eq!(view.snapshot.chunk_pos, mclone_core::ChunkPos::new(2, -1));
        assert_ne!(app.driver.spectator.position, view.eye);
    }

    #[test]
    fn queue_local_world_start_sets_loading_status_without_closing_menu() {
        let scene = SceneOptions {
            remote_addr: Some("127.0.0.1:25565".to_owned()),
            ..SceneOptions::default()
        };
        let assets = WindowSceneAssets::load().unwrap();
        let mut app = ChunkApp::new(
            scene,
            assets,
            TexturedSectionRenderOptions::default(),
            WindowStartIntent::Menu,
            StartupWaitPolicy::DESKTOP_DEFAULT,
        );
        app.driver.set_ui_screen(Some(GameScreen::NewWorld));

        app.driver.request_local_world_start(44, true);

        assert_eq!(
            app.driver.session.state(),
            &GameSessionState::Starting {
                request: SessionStartRequest::NewLocalWorld { seed: 44 }
            }
        );
        let status = app.driver.session.status().unwrap();
        assert!(status.ok);
        assert_eq!(status.message, "Creating world...");
        let pending = app.driver.session.take_pending_start().unwrap();
        assert_eq!(
            pending.request,
            SessionStartRequest::NewLocalWorld { seed: 44 }
        );
        assert_eq!(pending.payload.scene.seed, 44);
        assert_eq!(pending.payload.scene.remote_addr, None);
        assert!(pending.payload.arm_mouse_lock);
        assert!(!pending.payload.show_title_on_failure);
        assert_eq!(app.driver.ui_screen(), Some(GameScreen::NewWorld));
        assert!(!app.mouse_lock_requested);
    }

    #[test]
    fn queue_remote_session_start_sets_connecting_status_without_closing_menu() {
        let scene = SceneOptions::default();
        let assets = WindowSceneAssets::load().unwrap();
        let mut app = ChunkApp::new(
            scene,
            assets,
            TexturedSectionRenderOptions::default(),
            WindowStartIntent::Menu,
            StartupWaitPolicy::DESKTOP_DEFAULT,
        );
        app.driver.set_ui_screen(Some(GameScreen::JoinRemote));

        app.driver
            .request_remote_session_start("10.0.0.5:25565".to_owned(), true);

        assert_eq!(
            app.driver.session.state(),
            &GameSessionState::Starting {
                request: SessionStartRequest::JoinRemote {
                    endpoint: RemoteSessionEndpoint::new("10.0.0.5:25565")
                }
            }
        );
        let status = app.driver.session.status().unwrap();
        assert!(status.ok);
        assert_eq!(status.message, "Connecting...");
        let pending = app.driver.session.take_pending_start().unwrap();
        assert_eq!(
            pending.request,
            SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new("10.0.0.5:25565")
            }
        );
        assert_eq!(
            pending.payload.scene.remote_addr,
            Some("10.0.0.5:25565".to_owned())
        );
        assert_eq!(pending.payload.scene.seed, DEFAULT_SEED);
        assert!(pending.payload.arm_mouse_lock);
        assert!(!pending.payload.show_title_on_failure);
        assert_eq!(app.driver.ui_screen(), Some(GameScreen::JoinRemote));
        assert_eq!(app.driver.join_remote_addr(), "10.0.0.5:25565");
        assert!(!app.mouse_lock_requested);
    }

    #[test]
    fn pending_local_world_start_creates_startup_pump_without_blocking_until_active() {
        let scene = SceneOptions {
            render_distance: 0,
            ..SceneOptions::default()
        };
        let assets = WindowSceneAssets::load().unwrap();
        let mut app = ChunkApp::new(
            scene,
            assets,
            TexturedSectionRenderOptions::default(),
            WindowStartIntent::Menu,
            StartupWaitPolicy::DESKTOP_DEFAULT,
        );
        app.driver.request_local_world_start(77, true);

        app.finish_pending_session_start();

        assert!(app.driver.startup.is_some());
        assert!(app.driver.runtime.is_none());
        assert_eq!(
            app.driver.session.state(),
            &GameSessionState::Starting {
                request: SessionStartRequest::NewLocalWorld { seed: 77 }
            }
        );
        assert!(!app.mouse_lock_requested);
    }

    #[test]
    fn start_local_world_rebuilds_runtime_for_new_seed_without_stale_sections() {
        let scene = SceneOptions {
            seed: 12_345,
            render_distance: 0,
            ..SceneOptions::default()
        };
        let center = mclone_core::ChunkPos::new(scene.chunk_x, scene.chunk_z);
        let assets = WindowSceneAssets::load().unwrap();
        let mut app = ChunkApp::new(
            scene.clone(),
            assets,
            TexturedSectionRenderOptions::default(),
            WindowStartIntent::Menu,
            StartupWaitPolicy::DESKTOP_DEFAULT,
        );

        app.start_world_from_scene(scene).unwrap();
        let first_runtime = app.driver.runtime.as_ref().unwrap();
        assert!(first_runtime.client().chunk_snapshot(center).is_some());
        let first_signature = center_chunk_signature(first_runtime, center);

        assert_eq!(app.driver.render_stats.section_count, 0);
        assert_eq!(app.driver.render_stats.index_count, 0);

        app.start_local_world(98_765).unwrap();
        let second_runtime = app.driver.runtime.as_ref().unwrap();
        assert_eq!(app.driver.scene.seed, 98_765);
        assert_eq!(app.driver.scene.remote_addr, None);
        assert!(second_runtime.client().chunk_snapshot(center).is_some());
        let second_signature = center_chunk_signature(second_runtime, center);

        assert_ne!(first_signature, second_signature);
        assert_eq!(app.driver.render_stats.section_count, 0);
        assert_eq!(app.driver.render_stats.index_count, 0);
    }

    #[test]
    fn native_key_codes_map_to_shared_flat_input_controls() {
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::KeyW),
            Some(KeyboardKey::KeyW)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::KeyS),
            Some(KeyboardKey::KeyS)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::KeyA),
            Some(KeyboardKey::KeyA)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::KeyD),
            Some(KeyboardKey::KeyD)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::KeyE),
            Some(KeyboardKey::KeyE)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::KeyB),
            Some(KeyboardKey::KeyB)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::Space),
            Some(KeyboardKey::Space)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::KeyX),
            Some(KeyboardKey::KeyX)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::ShiftLeft),
            Some(KeyboardKey::ShiftLeft)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::ControlLeft),
            Some(KeyboardKey::ControlLeft)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::F1),
            Some(KeyboardKey::F1)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::F5),
            Some(KeyboardKey::F5)
        );
        assert_eq!(desktop_keyboard_key_from_key_code(NO_CLIP_TOGGLE_KEY), None);
        assert_eq!(
            desktop_keyboard_key_from_key_code(RENDER_RESOURCE_REBUILD_KEY),
            None
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(RENDER_SCALE_REBUILD_KEY),
            None
        );
        assert_eq!(desktop_keyboard_key_from_key_code(KeyCode::KeyO), None);
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::Digit1),
            Some(KeyboardKey::Digit1)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::Digit5),
            Some(KeyboardKey::Digit5)
        );
        assert_eq!(
            desktop_keyboard_key_from_key_code(KeyCode::Digit9),
            Some(KeyboardKey::Digit9)
        );
        assert_eq!(
            desktop_pointer_button_from_mouse_button(MouseButton::Left),
            Some(PointerButton::Primary)
        );
    }

    #[test]
    fn desktop_adapter_builds_shared_frame_from_held_keys() {
        let mut input = DesktopFlatInputAdapter::new();

        assert_eq!(
            input.handle_keyboard_input(KeyCode::KeyW, ElementState::Pressed, false),
            None
        );
        assert_eq!(
            input.handle_keyboard_input(KeyCode::KeyS, ElementState::Pressed, false),
            None
        );
        assert_eq!(
            input.handle_keyboard_input(KeyCode::Space, ElementState::Pressed, false),
            None
        );
        assert_eq!(
            input.handle_keyboard_input(KeyCode::ShiftLeft, ElementState::Pressed, false),
            None
        );

        let frame = input.held_frame();
        assert!(frame.forward);
        assert!(frame.backward);
        assert!(!frame.left);
        assert!(!frame.right);
        assert_eq!(frame.movement.forward, 0.0);
        assert!(frame.jump);
        assert!(frame.sneak);
        assert!(input.capability_state.capabilities.keyboard);

        let camera_input = engine_camera_input_from_flat_frame(frame, 0.016);
        assert_eq!(camera_input.dt_seconds, 0.016);
        assert!(camera_input.forward);
        assert!(camera_input.backward);
        assert!(camera_input.jump);
        assert!(camera_input.shift);

        input.handle_keyboard_input(KeyCode::KeyW, ElementState::Released, false);
        let frame = input.held_frame();
        assert!(!frame.forward);
        assert!(frame.backward);
    }

    #[test]
    fn desktop_adapter_emits_menu_and_hotbar_one_shots_without_repeat() {
        let mut input = DesktopFlatInputAdapter::new();

        let escape = input
            .handle_keyboard_input(KeyCode::Escape, ElementState::Pressed, false)
            .expect("escape should emit open-menu frame");
        assert!(escape.open_menu);

        let slot = input
            .handle_keyboard_input(KeyCode::Digit5, ElementState::Pressed, false)
            .expect("digit should emit hotbar frame");
        assert_eq!(slot.selected_hotbar_slot, Some(4));

        assert_eq!(
            input.handle_keyboard_input(KeyCode::Digit5, ElementState::Pressed, true),
            None
        );
    }

    #[test]
    fn desktop_adapter_emits_mouse_action_and_look_frames() {
        let mut input = DesktopFlatInputAdapter::new();

        let attack = input
            .handle_mouse_button(MouseButton::Left, ElementState::Pressed)
            .expect("left click should emit attack frame");
        assert!(attack.attack);
        assert!(!attack.use_item);

        let use_item = input
            .handle_mouse_button(MouseButton::Right, ElementState::Pressed)
            .expect("right click should emit use frame");
        assert!(use_item.use_item);
        assert!(input.capability_state.capabilities.mouse);

        let look = input
            .mouse_look_frame(4.0, -2.0)
            .expect("finite mouse delta should emit look frame");
        assert_eq!(look.look_delta.x, 4.0);
        assert_eq!(look.look_delta.y, -2.0);
        assert!(input.mouse_look_frame(0.0, 0.0).is_none());
    }

    #[test]
    fn desktop_touch_events_feed_shared_capability_resolution() {
        let mut input = DesktopFlatInputAdapter::new();

        input.note_touch_activity();
        let resolved = input
            .capability_state
            .resolve(mclone_input::InputPreferences::AUTO);
        assert_eq!(
            resolved.preferred_prompt,
            Some(mclone_input::InputPromptKind::Touch)
        );
        assert!(resolved.touch_controls_visible);
        assert!(resolved.accepts_touch);

        input.note_keyboard_activity();
        let resolved = input
            .capability_state
            .resolve(mclone_input::InputPreferences::AUTO);
        assert_eq!(
            resolved.preferred_prompt,
            Some(mclone_input::InputPromptKind::KeyboardMouse)
        );
        assert!(!resolved.touch_controls_visible);

        input.note_touch_activity();
        assert!(
            input
                .capability_state
                .resolve(mclone_input::InputPreferences::AUTO)
                .touch_controls_visible
        );
    }

    #[test]
    fn player_movement_mode_cycles_through_shared_modes() {
        assert_eq!(
            EngineCameraMovementMode::Walking.toggled(),
            EngineCameraMovementMode::NoClip
        );
        assert_eq!(
            EngineCameraMovementMode::NoClip.toggled(),
            EngineCameraMovementMode::HandPush
        );
        assert_eq!(
            EngineCameraMovementMode::HandPush.toggled(),
            EngineCameraMovementMode::Walking
        );
        assert_eq!(EngineCameraMovementMode::Walking.label(), "WALK");
        assert_eq!(EngineCameraMovementMode::NoClip.label(), "NOCLIP");
        assert_eq!(EngineCameraMovementMode::HandPush.label(), "HAND");
    }
}
