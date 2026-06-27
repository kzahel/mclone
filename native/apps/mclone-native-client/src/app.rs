use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, ClientInteractionController,
    LOCAL_PLAYER_STANDING_EYE_HEIGHT,
};
use mclone_core::Vec3d;
use mclone_input::{
    FlatInputAction, FlatInputFrame, InputCapabilities, InputCapabilityState, InputDeviceKind,
    InputPreferences, KeyboardKey, KeyboardMouseInputAdapter, PointerButton,
};
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
use mclone_render::entity::{ActorDrawResources, ActorInstance};
use mclone_render::gui::GuiRenderer;
use mclone_render::native::{NativeSurfaceContext, SurfaceFrameStatus};
use mclone_render::screen_effect::{ScreenEffectsRenderer, UnderwaterOverlay};
use mclone_render::sky_render::SkyRenderer;
use mclone_ui::{
    DEFAULT_JOIN_REMOTE_ADDR, FlatHotbarOverlay, FlatHud, GameFramePacingMode, GameScreen, GameUi,
    GameUiAction, GameUiRenderState, GuiKey, GuiScale, Point, StatusOverlay, render_flat_hud,
};
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::camera::{SpectatorCamera, chunk_camera_from_engine};
use crate::cli::{SceneOptions, WindowStartIntent};
use crate::frame_pacing::{
    FramePacing, FramePacingMode, FramePacingUiState, FrameTimingStats, RedrawSchedule, elapsed_ms,
    next_capped_redraw_deadline, redraw_schedule,
};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowSceneAssets, WindowSceneRuntime, poll_window_runtime_until_idle};
use crate::ui::{DebugPaneStats, render_debug_pane};
use crate::{MAX_RENDER_DISTANCE, MIN_RENDER_DISTANCE};
use mclone_app_runtime::frame_render::{
    FullFrameGui, RenderStreamStats, record_render_section_update_stats, render_full_frame_for_view,
};
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionCoordinator, GameSessionState, PendingSessionStart,
    RemoteSessionEndpoint, SessionFailure, SessionStartRequest, SessionStartResult,
    StartedGameSession,
};
use mclone_audio::{AudioEngine, AudioSettings, landing_playback_for_impact};
use mclone_render_session::{
    ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER,
    EngineCameraController, EngineCameraFrameState, EngineCameraInput, EngineCameraMovementImpulse,
    EngineCameraMovementMode, EngineCameraSnapshot, RenderSectionCacheUpdate,
    actor_instances_from_presentations, render_camera_from_snapshot,
};

const NO_CLIP_TOGGLE_KEY: KeyCode = KeyCode::KeyN;
const PLAYER_SURFACE_FEET_OFFSET: f64 = 1.0;
const GROUND_PROBE_DISTANCE: f64 = 0.01;

pub(crate) fn run_window(
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
    start_intent: WindowStartIntent,
) -> Result<()> {
    let assets = WindowSceneAssets::load()?;
    log::info!(
        "native window startup seed={} initial_center=({}, {}) render_distance={} lighting={} color_profile={} remote={:?} atlas={}x{} start={:?}",
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        scene.render_distance,
        if scene.lighting_enabled {
            "enabled"
        } else {
            "disabled"
        },
        render_options.color_profile.as_str(),
        scene.remote_addr,
        assets.mesh_assets.atlas.width,
        assets.mesh_assets.atlas.height,
        start_intent
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ChunkApp::new(scene, assets, render_options, start_intent);
    event_loop.run_app(&mut app)?;
    Ok(())
}

pub(crate) fn game_ui_render_state(
    render_distance: i32,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacingUiState,
    fly_enabled: bool,
    fly_speed_multiplier: f32,
) -> GameUiRenderState {
    GameUiRenderState {
        render_distance,
        min_render_distance: MIN_RENDER_DISTANCE,
        max_render_distance: MAX_RENDER_DISTANCE,
        section_occlusion_culling: render_options.section_occlusion_culling,
        force_fullbright: render_options.force_fullbright,
        fly_enabled,
        fly_speed_multiplier,
        min_fly_speed_multiplier: ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER as f32,
        max_fly_speed_multiplier: ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER as f32,
        frame_pacing_mode: game_frame_pacing_mode(frame_pacing.mode),
        fps_cap: frame_pacing.fps_cap,
        touch_controls_mode: None,
        touch_settings: None,
    }
}

fn game_frame_pacing_mode(mode: FramePacingMode) -> GameFramePacingMode {
    match mode {
        FramePacingMode::Vsync => GameFramePacingMode::Vsync,
        FramePacingMode::Capped => GameFramePacingMode::Capped,
        FramePacingMode::Uncapped => GameFramePacingMode::Uncapped,
    }
}

fn gui_key_from_key_code(key_code: KeyCode) -> Option<GuiKey> {
    match key_code {
        KeyCode::Escape => Some(GuiKey::Escape),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowCameraView {
    snapshot: EngineCameraSnapshot,
    eye: glam::Vec3,
}

impl WindowCameraView {
    fn from_snapshot(snapshot: EngineCameraSnapshot) -> Self {
        Self {
            snapshot,
            eye: glam_vec3_from_vec3d(snapshot.eye),
        }
    }

    fn block_column(self) -> (i32, i32) {
        (
            self.snapshot.eye.x.floor() as i32,
            self.snapshot.eye.z.floor() as i32,
        )
    }

    fn chunk_camera(self, render_distance: u32) -> ChunkCamera {
        chunk_camera_from_engine(render_camera_from_snapshot(self.snapshot, render_distance))
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
    scene: SceneOptions,
    assets: WindowSceneAssets,
    runtime: Option<WindowSceneRuntime>,
    session: GameSessionCoordinator<PendingWindowSessionStart>,
    spectator: SpectatorCamera,
    camera: EngineCameraController,
    actor_interpolation: ActorInterpolationState,
    interaction: ClientInteractionController,
    flat_input: DesktopFlatInputAdapter,
    input_preferences: InputPreferences,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacing,
    ui: GameUi,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    depth: Option<ChunkDepthTarget>,
    sky: Option<SkyRenderer>,
    draw: Option<TexturedSectionDrawResources>,
    actors: Option<ActorDrawResources>,
    screen_effects: Option<ScreenEffectsRenderer>,
    gui: Option<GuiRenderer>,
    audio: Option<AudioEngine>,
    mouse_locked: bool,
    mouse_lock_requested: bool,
    last_cursor: Option<(f64, f64)>,
    debug_visible: bool,
    last_frame: Instant,
    next_redraw_at: Option<Instant>,
    render_stats: RenderStreamStats,
    frame_timing: FrameTimingStats,
    start_intent: WindowStartIntent,
    seed_reroll_state: u64,
}

#[derive(Clone, Debug)]
struct PendingWindowSessionStart {
    scene: SceneOptions,
    arm_mouse_lock: bool,
}

impl ChunkApp {
    fn new(
        scene: SceneOptions,
        assets: WindowSceneAssets,
        render_options: TexturedSectionRenderOptions,
        start_intent: WindowStartIntent,
    ) -> Self {
        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let camera = engine_camera_controller_from_spectator(&spectator);
        let seed_reroll_state = initial_seed_reroll_state(scene.seed);
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
            scene,
            assets,
            runtime: None,
            session: GameSessionCoordinator::new(),
            spectator,
            camera,
            actor_interpolation: ActorInterpolationState::new(),
            interaction: ClientInteractionController::new(),
            flat_input: DesktopFlatInputAdapter::new(),
            input_preferences: InputPreferences::AUTO,
            render_options,
            frame_pacing: FramePacing::default(),
            ui,
            window: None,
            surface: None,
            depth: None,
            sky: None,
            draw: None,
            actors: None,
            screen_effects: None,
            gui: None,
            audio: None,
            mouse_locked: false,
            mouse_lock_requested: false,
            last_cursor: None,
            debug_visible: false,
            last_frame: Instant::now(),
            next_redraw_at: None,
            render_stats: RenderStreamStats::default(),
            frame_timing: FrameTimingStats::default(),
            start_intent,
            seed_reroll_state,
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

    fn window_camera_view(&self) -> WindowCameraView {
        WindowCameraView::from_snapshot(self.camera.snapshot())
    }

    fn current_render_distance(&self) -> u32 {
        self.runtime.as_ref().map_or_else(
            || u32::try_from(self.scene.render_distance).unwrap_or(0),
            WindowSceneRuntime::render_distance,
        )
    }

    fn update_camera_from_keys(&mut self, now: Instant) -> Result<()> {
        let frame_dt = now.duration_since(self.last_frame);
        let movement_dt = frame_dt.as_secs_f32().min(0.05);
        self.last_frame = now;
        self.render_stats.last_frame_ms = (frame_dt.as_secs_f64() * 1000.0) as f32;
        self.frame_timing.begin_frame(
            frame_dt.as_secs_f64() * 1000.0,
            self.frame_pacing.target_frame_ms(),
        );

        if self.ui.is_active() {
            return Ok(());
        }

        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(());
        };

        let frame = self.flat_input.held_frame();
        let input = engine_camera_input_from_flat_frame(frame, f64::from(movement_dt));
        let before = self.camera.snapshot();
        let after = self.camera.apply_movement_input(runtime.client(), input);

        if after != before {
            self.play_landing_events();
            self.commit_player_pose_change()?;
        } else {
            self.play_landing_events();
        }
        Ok(())
    }

    fn clear_flat_gameplay_input(&mut self) {
        self.flat_input.clear_held();
        self.camera.clear_keys();
    }

    fn apply_flat_keyboard_frame(
        &mut self,
        frame: FlatInputFrame,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        if frame.open_menu {
            self.mouse_lock_requested = false;
            self.ui.open_pause();
            self.clear_flat_gameplay_input();
            self.last_cursor = None;
            self.sync_mouse_lock();
            self.schedule_next_redraw(event_loop);
            return true;
        }
        if let Some(slot) = frame.selected_hotbar_slot {
            if self.interaction.select_hotbar_slot(slot) {
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
        if frame.look_delta.x == 0.0 && frame.look_delta.y == 0.0 {
            return;
        }
        self.camera
            .turn_mouse_delta(f64::from(frame.look_delta.x), f64::from(frame.look_delta.y));
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
        self.schedule_next_redraw(event_loop);
    }

    fn gui_scale(&self) -> Option<GuiScale> {
        self.surface.as_ref().map(|surface| {
            GuiScale::from_pixels(surface.config.width.max(1), surface.config.height.max(1))
        })
    }

    fn gui_point(&self, x: f64, y: f64) -> Option<Point> {
        self.gui_scale().map(|scale| scale.client_to_gui(x, y))
    }

    fn current_ui_render_state(&self) -> GameUiRenderState {
        let mut state = game_ui_render_state(
            i32::try_from(self.current_render_distance()).unwrap_or(MAX_RENDER_DISTANCE),
            self.render_options,
            self.frame_pacing.ui_state(),
            self.camera.movement_mode() == EngineCameraMovementMode::NoClip,
            self.camera.fly_speed_multiplier() as f32,
        );
        state.touch_controls_mode = Some(self.input_preferences.touch_controls);
        state
    }

    fn current_flat_hud(&self, status: StatusOverlay, ui_active: bool) -> FlatHud {
        let mut hud = FlatHud::new(
            self.flat_input
                .capability_state
                .resolve(self.input_preferences),
        );
        hud.world_hud_visible = !ui_active && self.runtime.is_some();
        hud.crosshair_visible = hud.world_hud_visible;
        hud.hotbar = FlatHotbarOverlay::selected(self.interaction.selected_hotbar_slot());
        hud.status = status;
        hud
    }

    fn next_new_world_seed(&mut self) -> i64 {
        self.seed_reroll_state = self
            .seed_reroll_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.seed_reroll_state as i64
    }

    fn local_world_scene(&self, seed: i64) -> SceneOptions {
        let mut scene = self.scene.clone();
        scene.seed = seed;
        scene.remote_addr = None;
        scene
    }

    fn remote_session_scene(&self, remote_addr: String) -> SceneOptions {
        let mut scene = self.scene.clone();
        scene.remote_addr = Some(remote_addr);
        scene
    }

    fn session_status_overlay(&self) -> StatusOverlay {
        self.session
            .status()
            .map_or_else(StatusOverlay::hidden, |status| {
                StatusOverlay::new(status.message, status.ok)
            })
    }

    fn clear_inactive_session_status(&mut self) {
        if !matches!(self.session.state(), GameSessionState::Active { .. }) {
            self.session.clear();
        }
    }

    fn queue_local_world_start(&mut self, seed: i64, arm_mouse_lock: bool) {
        self.session.request_start(
            SessionStartRequest::NewLocalWorld { seed },
            PendingWindowSessionStart {
                scene: self.local_world_scene(seed),
                arm_mouse_lock,
            },
        );
        self.mouse_lock_requested = false;
        self.clear_flat_gameplay_input();
        self.ui.clear_input();
        self.sync_mouse_lock();
    }

    fn queue_remote_session_start(&mut self, remote_addr: String, arm_mouse_lock: bool) {
        self.ui.set_join_remote_addr(remote_addr.clone());
        self.session.request_start(
            SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new(remote_addr.clone()),
            },
            PendingWindowSessionStart {
                scene: self.remote_session_scene(remote_addr),
                arm_mouse_lock,
            },
        );
        self.mouse_lock_requested = false;
        self.clear_flat_gameplay_input();
        self.ui.clear_input();
        self.sync_mouse_lock();
    }

    fn finish_pending_session_start(
        &mut self,
        pending: PendingSessionStart<PendingWindowSessionStart>,
    ) {
        let request = pending.request.clone();
        let arm_mouse_lock = pending.payload.arm_mouse_lock;
        let result = self.start_pending_window_session(pending);
        self.session.apply_start_result(&result);
        match result {
            Ok(started) => {
                self.apply_started_session_ui(started.descriptor());
                self.mouse_lock_requested = arm_mouse_lock;
                self.sync_mouse_lock();
            }
            Err(_) => {
                match request {
                    SessionStartRequest::NewLocalWorld { seed } => self.ui.set_new_world_seed(seed),
                    SessionStartRequest::JoinRemote { endpoint } => {
                        self.ui.set_join_remote_addr(endpoint.address);
                    }
                    SessionStartRequest::Unknown => {}
                }
                self.mouse_lock_requested = false;
                self.sync_mouse_lock();
            }
        }
    }

    fn start_pending_window_session(
        &mut self,
        pending: PendingSessionStart<PendingWindowSessionStart>,
    ) -> SessionStartResult<()> {
        self.start_window_session_from_scene(pending.request, pending.payload.scene)
    }

    fn start_window_session_from_scene(
        &mut self,
        request: SessionStartRequest,
        scene: SceneOptions,
    ) -> SessionStartResult<()> {
        let Some(descriptor) = request.active_descriptor() else {
            return Err(SessionFailure::new("Unsupported session start"));
        };
        match self.start_world_from_scene(scene) {
            Ok(()) => Ok(StartedGameSession::new(descriptor, ())),
            Err(err) => {
                log::error!("failed to start session {request:?}: {err:#}");
                Err(SessionFailure::new(request.default_failure_message()))
            }
        }
    }

    fn apply_started_session_ui(&mut self, descriptor: &ActiveSessionDescriptor) {
        match descriptor {
            ActiveSessionDescriptor::LocalWorld { seed } => {
                self.ui.apply_action(GameUiAction::CreateWorld(*seed));
                log::info!("created local world seed={seed}");
            }
            ActiveSessionDescriptor::Remote { endpoint } => {
                self.ui.apply_action(GameUiAction::JoinRemote);
                log::info!("joined remote session {}", endpoint.address);
            }
        }
    }

    fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        event_loop: &ActiveEventLoop,
        from_pointer_click: bool,
    ) {
        if self.session.is_starting() && !matches!(action, GameUiAction::Quit) {
            self.schedule_next_redraw(event_loop);
            return;
        }

        let should_arm_mouse_lock = from_pointer_click
            && matches!(
                action,
                GameUiAction::StartWorld | GameUiAction::Resume | GameUiAction::JoinRemote
            );
        let preserve_pointer_state = matches!(
            action,
            GameUiAction::SetRenderDistance(_)
                | GameUiAction::SetFlySpeed(_)
                | GameUiAction::SetTouchLookSensitivity(_)
                | GameUiAction::SetTouchControlsMode(_)
        );
        match action {
            GameUiAction::ToggleSectionOcclusion => {
                self.render_options.section_occlusion_culling =
                    !self.render_options.section_occlusion_culling;
                log::info!(
                    "section occlusion culling {}",
                    if self.render_options.section_occlusion_culling {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            GameUiAction::ToggleFullbright => {
                self.render_options.force_fullbright = !self.render_options.force_fullbright;
                log::info!(
                    "fullbright {}",
                    if self.render_options.force_fullbright {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            GameUiAction::ToggleFly => {
                let movement_mode = self.camera.toggle_movement_mode();
                log::info!("player movement mode {}", movement_mode.label());
            }
            GameUiAction::SetFlySpeed(multiplier) => {
                self.camera.set_fly_speed_multiplier(f64::from(multiplier));
                log::info!(
                    "fly speed set to {:.1}x ({:.0} blocks/s)",
                    self.camera.fly_speed_multiplier(),
                    self.camera.speed_blocks_per_second()
                );
            }
            GameUiAction::SetTouchControlsMode(mode) => {
                self.input_preferences.touch_controls = mode;
                log::info!(
                    "touch controls set to {}",
                    mclone_ui::touch_controls_mode_label(mode)
                );
            }
            GameUiAction::CycleFramePacing => {
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
            GameUiAction::CycleFpsCap => {
                self.frame_pacing.cycle_fps_cap();
                self.next_redraw_at = None;
                log::info!("fps cap set to {}", self.frame_pacing.fps_cap);
            }
            GameUiAction::SetRenderDistance(render_distance) => {
                let render_distance =
                    render_distance.clamp(MIN_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
                self.scene.render_distance = render_distance;
                let render_distance_chunks =
                    u32::try_from(render_distance).expect("clamped render distance must fit u32");
                if let Some(runtime) = &mut self.runtime {
                    match runtime.set_render_distance(render_distance_chunks) {
                        Ok(true) => {
                            log::info!(
                                "render distance set to {} (chunk tracking radius {})",
                                render_distance,
                                runtime.chunk_tracking_radius()
                            );
                        }
                        Ok(false) => {}
                        Err(err) => {
                            log::error!(
                                "failed to set render distance to {render_distance}: {err:#}"
                            );
                            return;
                        }
                    }
                } else {
                    log::info!("new-world render distance set to {render_distance}");
                }
            }
            GameUiAction::OpenNewWorld => {
                let seed = self.next_new_world_seed();
                self.ui.set_new_world_seed(seed);
                self.clear_inactive_session_status();
            }
            GameUiAction::OpenJoinRemote => {
                let remote_addr = self
                    .scene
                    .remote_addr
                    .clone()
                    .unwrap_or_else(|| DEFAULT_JOIN_REMOTE_ADDR.to_owned());
                self.ui.set_join_remote_addr(remote_addr);
                self.clear_inactive_session_status();
            }
            GameUiAction::RerollSeed => {
                let seed = self.next_new_world_seed();
                self.ui.set_new_world_seed(seed);
                self.clear_inactive_session_status();
                log::info!("new-world seed rerolled to {seed}");
            }
            GameUiAction::CreateWorld(seed) => {
                self.queue_local_world_start(seed, from_pointer_click);
                self.schedule_next_redraw(event_loop);
                return;
            }
            GameUiAction::JoinRemote => {
                self.queue_remote_session_start(
                    self.ui.join_remote_addr().to_owned(),
                    from_pointer_click,
                );
                self.schedule_next_redraw(event_loop);
                return;
            }
            GameUiAction::QuitToTitle => {
                if let Err(err) = self.teardown_world() {
                    log::error!("failed to tear down world: {err:#}");
                    self.schedule_next_redraw(event_loop);
                    return;
                }
                self.session.clear();
            }
            GameUiAction::Quit => {
                event_loop.exit();
                return;
            }
            GameUiAction::BackToTitle => {
                self.clear_inactive_session_status();
            }
            GameUiAction::StartWorld
            | GameUiAction::Resume
            | GameUiAction::OpenOptions(_)
            | GameUiAction::BackToPause
            | GameUiAction::SetTouchLookSensitivity(_) => {}
        }
        self.ui.apply_action(action);
        if should_arm_mouse_lock {
            self.mouse_lock_requested = true;
        }
        self.clear_flat_gameplay_input();
        if !preserve_pointer_state {
            self.last_cursor = None;
        }
        self.sync_mouse_lock();
        self.schedule_next_redraw(event_loop);
    }

    fn sync_mouse_lock(&mut self) {
        self.set_mouse_lock(
            self.mouse_lock_requested && !self.ui.is_active() && self.runtime.is_some(),
        );
    }

    fn toggle_movement_mode(&mut self) {
        let movement_mode = self.camera.toggle_movement_mode();
        log::info!("player movement mode {}", movement_mode.label());
    }

    fn reset_world_local_state(&mut self) {
        self.spectator = SpectatorCamera::spawn_for_scene(&self.scene);
        self.camera = engine_camera_controller_from_spectator(&self.spectator);
        self.actor_interpolation = ActorInterpolationState::new();
        self.interaction = ClientInteractionController::new();
        self.flat_input.clear_held();
        self.render_stats = RenderStreamStats::default();
        self.frame_timing = FrameTimingStats::default();
        self.last_cursor = None;
    }

    fn clear_draw_sections(&mut self) -> Result<()> {
        let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) else {
            return Ok(());
        };
        let report = draw
            .update_sections(&surface.device, &[])
            .context("failed to clear world render sections")?;
        if report.removed_section_count > 0 {
            log::info!(
                "cleared {} render sections for world teardown",
                report.removed_section_count
            );
        }
        self.render_stats.section_count = 0;
        self.render_stats.index_count = 0;
        self.render_stats.face_count = 0;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        Ok(())
    }

    fn teardown_world(&mut self) -> Result<()> {
        self.runtime = None;
        self.reset_world_local_state();
        self.clear_draw_sections()?;
        self.mouse_lock_requested = false;
        self.set_mouse_lock(false);
        Ok(())
    }

    fn start_world_from_scene(&mut self, scene: SceneOptions) -> Result<()> {
        self.runtime = None;
        self.scene = scene;
        self.reset_world_local_state();
        self.clear_draw_sections()?;

        let mut runtime = WindowSceneRuntime::with_assets(&self.scene, &self.assets)
            .context("failed to create window world runtime")?;
        let initial_poll_start = Instant::now();
        let (initial_poll_count, initial_poll_ms) = poll_window_runtime_until_idle(&mut runtime)
            .context("failed to load initial light-ready chunks")?;
        self.runtime = Some(runtime);
        match self.apply_pending_player_position_updates() {
            Ok(true) => {}
            Ok(false) => self.place_spectator_above_loaded_surface(),
            Err(err) => {
                self.runtime = None;
                return Err(err).context("failed to apply initial server player position");
            }
        }
        let loaded = self
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.client().loaded_chunk_count());
        log::info!(
            "world seed={} initial chunks loaded in {} polls poll_ms={:.3} elapsed_ms={:.3} loaded={}",
            self.scene.seed,
            initial_poll_count,
            initial_poll_ms,
            elapsed_ms(initial_poll_start.elapsed()),
            loaded
        );
        self.upload_all_runtime_sections()
            .context("failed to upload initial world render sections")?;
        Ok(())
    }

    #[cfg(test)]
    fn start_local_world(&mut self, seed: i64) -> Result<()> {
        self.start_world_from_scene(self.local_world_scene(seed))
    }

    fn place_spectator_above_loaded_surface(&mut self) {
        let view = self.window_camera_view();
        let (world_x, world_z) = view.block_column();
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let Some(surface_y) = runtime.highest_non_air_block_y_at_world(world_x, world_z) else {
            log::warn!(
                "no loaded surface column found for initial spectator at ({world_x}, {world_z})"
            );
            return;
        };
        let eye_position = Vec3d::new(
            view.snapshot.eye.x,
            surface_y as f64 + PLAYER_SURFACE_FEET_OFFSET + LOCAL_PLAYER_STANDING_EYE_HEIGHT,
            view.snapshot.eye.z,
        );
        self.camera.set_eye_pose(
            eye_position,
            view.snapshot.yaw_radians,
            f64::from(crate::camera::SPECTATOR_SURFACE_PITCH),
        );
        if self.camera.movement_mode() == EngineCameraMovementMode::Walking {
            if let Some(runtime) = self.runtime.as_ref() {
                self.camera
                    .probe_ground(runtime.client(), GROUND_PROBE_DISTANCE);
            }
        }
        if let Err(err) = self.commit_player_pose_change() {
            log::warn!("failed to commit initial player pose: {err:#}");
        }
        log::info!(
            "placed player above loaded surface column ({world_x}, {world_z}) y={} -> feet_y={:.1} eye_y={:.1}",
            surface_y,
            self.camera.player().pose().position.y,
            self.window_camera_view().eye.y
        );
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
        if self.runtime.is_none() {
            return Ok(());
        }
        let poll_start = Instant::now();
        let mut changed = self
            .runtime
            .as_mut()
            .expect("runtime presence checked")
            .poll()?;
        changed |= self.apply_pending_player_position_updates()?;
        self.frame_timing
            .record_runtime_poll(elapsed_ms(poll_start.elapsed()));
        let camera_eye = self.window_camera_view().eye;
        if !changed
            && !self
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.has_pending_render_work(camera_eye))
        {
            return Ok(());
        }
        self.upload_runtime_sections()?;
        Ok(())
    }

    fn upload_runtime_sections(&mut self) -> Result<()> {
        let camera_view = self.window_camera_view();
        let Some(runtime) = &mut self.runtime else {
            return Ok(());
        };
        let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) else {
            return Ok(());
        };
        let remesh_start = Instant::now();
        let section_update = runtime.sync_render_sections(camera_view.eye)?;
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let upload_start = Instant::now();
        let upload_report = draw
            .apply_section_updates(
                &surface.device,
                &section_update.rebuilt_sections,
                &section_update.removed_section_keys,
            )
            .context("failed to upload streamed chunk section updates")?;
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let section_count = draw.section_count();
        let index_count = draw.index_count();
        let face_count = quad_face_count_from_indices(index_count);
        let loaded_chunk_count = runtime.client().loaded_chunk_count();
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.record_section_update_stats(&section_update, upload_report);
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        self.frame_timing.record_remesh_upload(remesh_ms, upload_ms);
        log::info!(
            "streamed chunks loaded={} sections={} faces={} indices={} rebuilt={} visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} uploaded={} uploaded_vertices={} uploaded_faces={} uploaded_indices={} removed={} remesh_ms={:.3} upload_ms={:.3}",
            loaded_chunk_count,
            section_count,
            face_count,
            index_count,
            section_update.rebuilt_section_count(),
            section_update.visibility_graph_stats.build_count,
            section_update.visibility_graph_stats.total_ms,
            section_update.visibility_graph_stats.worst_ms,
            upload_report.uploaded_section_count,
            upload_report.uploaded_vertex_count,
            upload_report.uploaded_face_count(),
            upload_report.uploaded_index_count,
            upload_report.removed_section_count,
            remesh_ms,
            upload_ms
        );
        Ok(())
    }

    fn upload_all_runtime_sections(&mut self) -> Result<()> {
        let camera_view = self.window_camera_view();
        let Some(runtime) = &mut self.runtime else {
            return Ok(());
        };
        let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) else {
            return Ok(());
        };
        let remesh_start = Instant::now();
        let section_update = runtime.sync_all_render_sections(camera_view.eye)?;
        let sections = runtime.cached_sections();
        if sections.is_empty() {
            anyhow::bail!(
                "world seed={} center=({}, {}) render_distance={} produced no render sections",
                self.scene.seed,
                self.scene.chunk_x,
                self.scene.chunk_z,
                self.scene.render_distance
            );
        }
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let upload_start = Instant::now();
        let upload_report = draw
            .update_sections(&surface.device, &sections)
            .context("failed to upload world chunk section resources")?;
        draw.set_traversal_ready_sections(
            &runtime.traversal_ready_render_section_keys(camera_view.eye),
        );
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let section_count = draw.section_count();
        let index_count = draw.index_count();
        let face_count = quad_face_count_from_indices(index_count);
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.record_section_update_stats(&section_update, upload_report);
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        log::info!(
            "uploaded {} world render sections with {} vertices / {} faces / {} indices visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} remesh_ms={:.3} upload_ms={:.3}",
            section_count,
            section_update.rebuilt_vertex_count,
            face_count,
            index_count,
            section_update.visibility_graph_stats.build_count,
            section_update.visibility_graph_stats.total_ms,
            section_update.visibility_graph_stats.worst_ms,
            remesh_ms,
            upload_ms
        );
        Ok(())
    }

    fn record_section_update_stats(
        &mut self,
        section_update: &RenderSectionCacheUpdate,
        upload_report: TexturedSectionUploadReport,
    ) {
        record_render_section_update_stats(&mut self.render_stats, section_update, upload_report);
    }

    fn effective_render_options(&self) -> TexturedSectionRenderOptions {
        effective_render_options_for_camera(
            self.render_options,
            self.runtime.as_ref().is_some_and(|runtime| {
                runtime.camera_inside_occluding_block(self.window_camera_view().eye)
            }),
        )
    }

    fn debug_pane_stats(&self, render_options: TexturedSectionRenderOptions) -> DebugPaneStats {
        let camera_state = self.camera_frame_state();
        DebugPaneStats {
            position: self.window_camera_view().eye,
            speed: camera_state.camera.speed_blocks_per_second as f32,
            movement_mode: camera_state.movement_mode_label(),
            on_ground: camera_state.on_ground,
            runtime: self
                .runtime
                .as_ref()
                .expect("debug pane requires an active runtime")
                .stats(),
            render: self.render_stats,
            frame: self.frame_timing,
            pacing: self.frame_pacing.debug_stats(),
            section_occlusion: render_options.section_occlusion_culling,
            force_fullbright: render_options.force_fullbright,
            color_profile: render_options.color_profile.label(),
        }
    }

    fn camera_frame_state(&self) -> EngineCameraFrameState {
        self.camera.frame_state(&self.interaction)
    }

    fn underwater_overlay(&self, camera_view: WindowCameraView) -> Option<UnderwaterOverlay> {
        self.runtime
            .as_ref()?
            .camera_inside_water(camera_view.eye)
            .then(|| {
                UnderwaterOverlay::vanilla_from_native_camera(
                    camera_view.snapshot.yaw_radians as f32,
                    camera_view.snapshot.pitch_radians as f32,
                )
            })
    }

    fn commit_player_pose_change(&mut self) -> Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.commit_engine_camera_player_pose(&mut self.camera)?;
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
        Ok(changed)
    }

    fn play_landing_events(&mut self) {
        let events = self.camera.take_landing_events();
        let Some(audio) = &self.audio else {
            return;
        };
        for event in events {
            let (sound, gain) = landing_playback_for_impact(event.impact_speed);
            audio.play(sound, gain);
        }
    }

    fn interpolated_actor_instances(&mut self) -> Vec<ActorInstance> {
        let Some(runtime) = self.runtime.as_ref() else {
            return Vec::new();
        };
        self.actor_interpolation
            .reconcile_authoritative(runtime.client().actor_presentations());
        self.actor_interpolation.step(
            (self.render_stats.last_frame_ms * 0.001).min(0.1),
            ActorInterpolationConfig::default(),
        );
        actor_instances_from_presentations(
            &self.actor_interpolation.presentations(),
            runtime.client(),
        )
    }

    fn sync_server_player_pose(&mut self) -> Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.sync_engine_camera_player_pose(&mut self.camera)?;
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
        Ok(changed)
    }

    fn apply_pending_player_position_updates(&mut self) -> Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.apply_pending_engine_camera_position_updates(&mut self.camera)?;
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
        Ok(changed)
    }

    fn sync_carried_item(&mut self) -> Result<bool> {
        let Some(command) = self.interaction.ensure_has_sent_carried_item() else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(command)
            .context("failed to sync carried item to server")
    }

    fn handle_world_flat_action(&mut self, action: FlatInputAction) -> Result<()> {
        if self.runtime.is_none() {
            return Ok(());
        }
        self.sync_server_player_pose()?;
        self.sync_carried_item()?;
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(());
        };
        let hit = self.camera.pick_block(runtime.client(), &self.interaction);
        let command = match action {
            FlatInputAction::Attack => self.interaction.debug_instant_break_command(hit),
            FlatInputAction::Use => self.interaction.use_item_on_command(hit),
            _ => None,
        };
        let Some(command) = command else {
            return Ok(());
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(());
        };
        let changed = runtime
            .send_gameplay_command(command)
            .context("failed to send gameplay interaction command")?;
        log::info!(
            "gameplay interaction {:?} at ({}, {}, {}) face={:?} changed={}",
            action,
            hit.block_pos.x,
            hit.block_pos.y,
            hit.block_pos.z,
            hit.direction,
            changed
        );
        Ok(())
    }
}

fn effective_render_options_for_camera(
    mut render_options: TexturedSectionRenderOptions,
    camera_inside_occluding_block: bool,
) -> TexturedSectionRenderOptions {
    if camera_inside_occluding_block {
        render_options.section_occlusion_culling = false;
    }
    render_options
}

fn vec3d_from_glam(value: glam::Vec3) -> Vec3d {
    Vec3d::new(value.x as f64, value.y as f64, value.z as f64)
}

fn glam_vec3_from_vec3d(value: Vec3d) -> glam::Vec3 {
    glam::Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

fn initial_seed_reroll_state(seed: i64) -> u64 {
    (seed as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0xD1B5_4A32_D192_ED03)
        .max(1)
}

fn session_start_request_for_scene(scene: &SceneOptions) -> SessionStartRequest {
    scene.remote_addr.as_ref().map_or(
        SessionStartRequest::NewLocalWorld { seed: scene.seed },
        |remote_addr| SessionStartRequest::JoinRemote {
            endpoint: RemoteSessionEndpoint::new(remote_addr.clone()),
        },
    )
}

fn engine_camera_controller_from_spectator(spectator: &SpectatorCamera) -> EngineCameraController {
    EngineCameraController::from_eye_pose(
        vec3d_from_glam(spectator.position),
        f64::from(spectator.yaw),
        f64::from(spectator.pitch),
        f64::from(spectator.speed),
    )
}

fn sync_spectator_from_camera(spectator: &mut SpectatorCamera, camera: &EngineCameraController) {
    let snapshot = camera.snapshot();
    spectator.position = glam_vec3_from_vec3d(snapshot.eye);
    spectator.yaw = snapshot.yaw_radians as f32;
    spectator.pitch = snapshot.pitch_radians as f32;
    spectator.speed = snapshot.speed_blocks_per_second as f32;
}

fn engine_camera_input_from_flat_frame(
    frame: FlatInputFrame,
    dt_seconds: f64,
) -> EngineCameraInput {
    EngineCameraInput {
        dt_seconds,
        mouse_delta_x: f64::from(frame.look_delta.x),
        mouse_delta_y: f64::from(frame.look_delta.y),
        forward: frame.forward,
        backward: frame.backward,
        left: frame.left,
        right: frame.right,
        jump: frame.jump,
        descend: frame.descend,
        shift: frame.sneak,
        sprint: frame.sprint,
        movement_impulse: frame
            .analog_movement
            .map(|movement| EngineCameraMovementImpulse::new(movement.left, movement.forward)),
        movement_yaw_radians: None,
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
            self.render_options.color_profile,
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
        let depth =
            ChunkDepthTarget::new(&surface.device, surface.config.width, surface.config.height);
        let draw = match TexturedSectionDrawResources::new(
            &surface.device,
            &surface.queue,
            surface.config.format,
            &[],
            self.assets.mesh_assets.atlas.as_upload(),
        ) {
            Ok(draw) => draw,
            Err(err) => {
                log::error!("failed to initialize chunk draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let sky = SkyRenderer::new_with_color_profile(
            &surface.device,
            surface.config.format,
            self.render_options.color_profile,
        );
        let actors = match ActorDrawResources::new(
            &surface.device,
            &surface.queue,
            surface.config.format,
            self.assets.actor_textures.atlas.as_upload(),
        ) {
            Ok(actors) => actors,
            Err(err) => {
                log::error!("failed to initialize actor draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let gui = GuiRenderer::new(&surface.device, surface.config.format);
        let asset_source = match load_asset_source() {
            Ok(source) => source,
            Err(err) => {
                log::error!("failed to load assets for screen effects: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let screen_effects = match ScreenEffectsRenderer::new(
            &surface.device,
            &surface.queue,
            surface.config.format,
            &asset_source,
        ) {
            Ok(screen_effects) => screen_effects,
            Err(err) => {
                log::error!("failed to initialize screen effects renderer: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let audio = match AudioEngine::new(&asset_source, AudioSettings::default()) {
            Ok(audio) => Some(audio),
            Err(err) => {
                log::warn!("audio disabled: {err:#}");
                None
            }
        };
        self.ui.set_scale(GuiScale::from_pixels(
            surface.config.width,
            surface.config.height,
        ));
        self.depth = Some(depth);
        self.sky = Some(sky);
        self.draw = Some(draw);
        self.actors = Some(actors);
        self.screen_effects = Some(screen_effects);
        self.gui = Some(gui);
        self.audio = audio;
        self.surface = Some(surface);
        self.window = Some(window);
        self.last_frame = Instant::now();
        self.frame_timing = FrameTimingStats::default();
        self.next_redraw_at = None;
        event_loop.listen_device_events(DeviceEvents::WhenFocused);
        if self.start_intent == WindowStartIntent::InWorld {
            let scene = self.scene.clone();
            let request = session_start_request_for_scene(&scene);
            self.session.begin_start(request.clone());
            let result = self.start_window_session_from_scene(request, scene);
            self.session.apply_start_result(&result);
            if result.is_err() {
                self.ui.set_screen(Some(GameScreen::Title));
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
                if let (Some(surface), Some(depth)) = (&self.surface, &mut self.depth) {
                    depth.resize(&surface.device, surface.config.width, surface.config.height);
                    self.ui.set_scale(GuiScale::from_pixels(
                        surface.config.width,
                        surface.config.height,
                    ));
                }
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    if let Some(scale) = self.gui_scale() {
                        self.ui.set_scale(scale);
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
                    if event.state == ElementState::Pressed && self.ui.is_active() {
                        if let Some(gui_key) = gui_key_from_key_code(key_code) {
                            let (handled, action) = self.ui.key_pressed(gui_key);
                            if let Some(action) = action {
                                self.apply_ui_action(action, event_loop, false);
                            } else if handled {
                                self.schedule_next_redraw(event_loop);
                            }
                        }
                        return;
                    }
                    if key_code == KeyCode::KeyO
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.render_options.section_occlusion_culling =
                            !self.render_options.section_occlusion_culling;
                        log::info!(
                            "section occlusion culling {}",
                            if self.render_options.section_occlusion_culling {
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
                        self.render_options.force_fullbright =
                            !self.render_options.force_fullbright;
                        log::info!(
                            "fullbright {}",
                            if self.render_options.force_fullbright {
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
                if self.ui.is_active() {
                    if button == MouseButton::Left {
                        if let Some((x, y)) = self.last_cursor
                            && let Some(point) = self.gui_point(x, y)
                        {
                            match state {
                                ElementState::Pressed => {
                                    let ui_state = self.current_ui_render_state();
                                    self.ui.pointer_down(point, ui_state);
                                }
                                ElementState::Released => {
                                    let ui_state = self.current_ui_render_state();
                                    let (_handled, action) = self.ui.pointer_up(point, ui_state);
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
                if self.runtime.is_none() {
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
                if self.ui.is_active() {
                    self.last_cursor = Some(cursor);
                    if let Some(point) = self.gui_point(cursor.0, cursor.1) {
                        let ui_state = self.current_ui_render_state();
                        let (_handled, action) = self.ui.pointer_move(point, ui_state);
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
                if self.ui.is_active() {
                    return;
                }
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.12,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.001,
                };
                self.camera.adjust_speed(f64::from(amount));
                sync_spectator_from_camera(&mut self.spectator, &self.camera);
                log::info!(
                    "no-clip speed {:.1} blocks/s",
                    self.camera.snapshot().speed_blocks_per_second
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
                self.ui.clear_input();
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
                let Some(gui_scale) = self.gui_scale() else {
                    return;
                };
                self.ui.set_scale(gui_scale);
                let camera_view = self.window_camera_view();
                let camera = camera_view.chunk_camera(self.current_render_distance());
                let sky_clear_color = self.runtime.as_ref().map_or_else(
                    mclone_render::default_clear_color,
                    WindowSceneRuntime::sky_clear_color,
                );
                let time_of_day = self
                    .runtime
                    .as_ref()
                    .map_or(0.0, WindowSceneRuntime::time_of_day);
                let sun_angle = self
                    .runtime
                    .as_ref()
                    .map_or(0.0, WindowSceneRuntime::sun_angle);
                let render_options = self.effective_render_options();
                let underwater_overlay = self.underwater_overlay(camera_view);
                let ui_render_state = self.current_ui_render_state();
                let actor_instances = self.interpolated_actor_instances();
                let debug_stats = (self.debug_visible && self.runtime.is_some())
                    .then(|| self.debug_pane_stats(render_options));
                let ui_active = self.ui.is_active();
                let ui_covers_world = self.ui.covers_world();
                let debug_stats = (!ui_active).then_some(debug_stats).flatten();
                let status_overlay = self.session_status_overlay();
                let flat_hud = self.current_flat_hud(status_overlay, ui_active);
                let gui_active =
                    ui_active || debug_stats.is_some() || flat_hud.has_visible_commands();
                let gui_scale = self.ui.scale();
                let base_ui_draw = self.ui.render_draw_list(ui_render_state);
                let gui_state = FullFrameGui::new(
                    gui_active,
                    ui_covers_world,
                    [gui_scale.width, gui_scale.height],
                );
                let traversal_ready_sections =
                    self.runtime.as_ref().map_or_else(BTreeSet::new, |runtime| {
                        runtime.traversal_ready_render_section_keys(camera_view.eye)
                    });
                let mut render_stats = self.render_stats;
                let render_start = Instant::now();
                let result = {
                    let (
                        Some(surface),
                        Some(depth),
                        Some(sky),
                        Some(draw),
                        Some(actors),
                        Some(screen_effects),
                        Some(gui),
                    ) = (
                        &mut self.surface,
                        &self.depth,
                        &self.sky,
                        &mut self.draw,
                        &mut self.actors,
                        &mut self.screen_effects,
                        &mut self.gui,
                    )
                    else {
                        return;
                    };
                    draw.set_traversal_ready_sections(&traversal_ready_sections);
                    self.frame_pacing.apply_to_surface(surface);
                    surface.render_with_report(|frame| {
                        let render_view =
                            camera.render_view(frame.target.size[0], frame.target.size[1]);
                        render_full_frame_for_view(
                            frame,
                            depth,
                            sky,
                            draw,
                            Some(actors),
                            Some(screen_effects),
                            Some(gui),
                            render_view,
                            &actor_instances,
                            underwater_overlay,
                            sky_clear_color,
                            time_of_day,
                            sun_angle,
                            render_options,
                            gui_state,
                            |stats| {
                                let mut ui_draw = base_ui_draw;
                                render_flat_hud(gui_scale, &mut ui_draw, &flat_hud);
                                if let Some(mut debug_stats) = debug_stats {
                                    debug_stats.render = *stats;
                                    render_debug_pane(gui_scale, &mut ui_draw, &debug_stats);
                                }
                                ui_draw
                            },
                            &mut render_stats,
                        )?;
                        Ok(())
                    })
                };
                match result {
                    Ok(report) => {
                        self.frame_timing.record_surface_frame(
                            elapsed_ms(render_start.elapsed()),
                            report.acquire_ms,
                            report.encode_ms,
                            report.submit_ms,
                            report.present_ms,
                        );
                        match report.status {
                            SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped => {
                                self.render_stats = render_stats;
                                if let Some(pending) = self.session.take_pending_start() {
                                    self.finish_pending_session_start(pending);
                                }
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
        if self.ui.is_active() || !self.mouse_locked || self.runtime.is_none() {
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
        KeyCode::KeyX => Some(KeyboardKey::KeyX),
        KeyCode::Space => Some(KeyboardKey::Space),
        KeyCode::ShiftLeft => Some(KeyboardKey::ShiftLeft),
        KeyCode::ShiftRight => Some(KeyboardKey::ShiftRight),
        KeyCode::ControlLeft => Some(KeyboardKey::ControlLeft),
        KeyCode::ControlRight => Some(KeyboardKey::ControlRight),
        KeyCode::Escape => Some(KeyboardKey::Escape),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_SEED;

    fn test_app_with_runtime(scene: SceneOptions) -> ChunkApp {
        let assets = WindowSceneAssets::load().unwrap();
        let runtime = WindowSceneRuntime::with_assets(&scene, &assets).unwrap();
        let mut app = ChunkApp::new(
            scene,
            assets,
            TexturedSectionRenderOptions::default(),
            WindowStartIntent::InWorld,
        );
        app.runtime = Some(runtime);
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
            mclone_render::entity::ActorInstanceShape::Humanoid
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
        app.camera.set_eye_pose(target_eye, 0.0, 0.0);

        assert!(app.commit_player_pose_change().unwrap());

        assert_eq!(
            app.spectator.position,
            glam::Vec3::new(
                target_eye.x as f32,
                target_eye.y as f32,
                target_eye.z as f32
            )
        );
        assert_eq!(
            app.camera_frame_state().camera.chunk_pos,
            mclone_core::ChunkPos::new(1, 0)
        );
        assert_eq!(
            app.runtime.as_ref().unwrap().stats().interest_center,
            mclone_core::ChunkPos::new(1, 0)
        );
    }

    #[test]
    fn window_camera_view_uses_controller_snapshot_not_spectator_mirror() {
        let scene = SceneOptions::default();
        let mut app = test_app_with_runtime(scene);
        let target_eye = Vec3d::new(32.25, 80.0, -0.25);

        app.camera.set_eye_pose(target_eye, 0.25, -0.125);
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
        assert_ne!(app.spectator.position, view.eye);
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
        );
        app.ui.set_screen(Some(GameScreen::NewWorld));

        app.queue_local_world_start(44, true);

        assert_eq!(
            app.session.state(),
            &GameSessionState::Starting {
                request: SessionStartRequest::NewLocalWorld { seed: 44 }
            }
        );
        let status = app.session.status().unwrap();
        assert!(status.ok);
        assert_eq!(status.message, "Creating world...");
        let pending = app.session.take_pending_start().unwrap();
        assert_eq!(
            pending.request,
            SessionStartRequest::NewLocalWorld { seed: 44 }
        );
        assert_eq!(pending.payload.scene.seed, 44);
        assert_eq!(pending.payload.scene.remote_addr, None);
        assert!(pending.payload.arm_mouse_lock);
        assert_eq!(app.ui.screen(), Some(GameScreen::NewWorld));
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
        );
        app.ui.set_screen(Some(GameScreen::JoinRemote));

        app.queue_remote_session_start("10.0.0.5:25565".to_owned(), true);

        assert_eq!(
            app.session.state(),
            &GameSessionState::Starting {
                request: SessionStartRequest::JoinRemote {
                    endpoint: RemoteSessionEndpoint::new("10.0.0.5:25565")
                }
            }
        );
        let status = app.session.status().unwrap();
        assert!(status.ok);
        assert_eq!(status.message, "Connecting...");
        let pending = app.session.take_pending_start().unwrap();
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
        assert_eq!(app.ui.screen(), Some(GameScreen::JoinRemote));
        assert_eq!(app.ui.join_remote_addr(), "10.0.0.5:25565");
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
        );

        app.start_world_from_scene(scene).unwrap();
        let first_runtime = app.runtime.as_ref().unwrap();
        assert!(first_runtime.client().chunk_snapshot(center).is_some());
        let first_signature = center_chunk_signature(first_runtime, center);

        let camera_eye = app.window_camera_view().eye;
        let first_update = app
            .runtime
            .as_mut()
            .unwrap()
            .sync_all_render_sections(camera_eye)
            .unwrap();
        assert!(first_update.rebuilt_section_count() > 0);
        assert!(!app.runtime.as_ref().unwrap().cached_sections().is_empty());

        app.start_local_world(98_765).unwrap();
        let second_runtime = app.runtime.as_ref().unwrap();
        assert_eq!(app.scene.seed, 98_765);
        assert_eq!(app.scene.remote_addr, None);
        assert!(second_runtime.client().chunk_snapshot(center).is_some());
        let second_signature = center_chunk_signature(second_runtime, center);

        assert_ne!(first_signature, second_signature);
        assert!(second_runtime.cached_sections().is_empty());
        assert_eq!(app.render_stats.section_count, 0);
        assert_eq!(app.render_stats.index_count, 0);
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
        assert_eq!(desktop_keyboard_key_from_key_code(NO_CLIP_TOGGLE_KEY), None);
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
    fn player_movement_mode_toggles_between_walking_and_no_clip() {
        assert_eq!(
            EngineCameraMovementMode::Walking.toggled(),
            EngineCameraMovementMode::NoClip
        );
        assert_eq!(
            EngineCameraMovementMode::NoClip.toggled(),
            EngineCameraMovementMode::Walking
        );
        assert_eq!(EngineCameraMovementMode::Walking.label(), "WALK");
        assert_eq!(EngineCameraMovementMode::NoClip.label(), "NOCLIP");
    }
}
