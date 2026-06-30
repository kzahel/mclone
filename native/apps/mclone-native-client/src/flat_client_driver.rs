use std::collections::BTreeSet;

use anyhow::Context;
use mclone_app_runtime::frame_render::{
    FlatRenderResources, FullFrameGui, FullFrameRenderSummary, RenderStreamStats,
    record_render_section_update_stats,
};
use mclone_app_runtime::local_single_view::LocalSingleViewStartupStep;
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionCoordinator, GameSessionState, PendingSessionStart,
    RemoteSessionEndpoint, SessionFailure, SessionStartRequest, SessionStartResult,
    StartedGameSession,
};
use mclone_assets::AssetSource;
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, BlockInteractionTarget,
    ClientInteractionController, LOCAL_PLAYER_STANDING_EYE_HEIGHT,
};
use mclone_core::{BlockStateId, Vec3d};
use mclone_input::{FLAT_HOTBAR_SLOT_COUNT, FlatInputAction, FlatInputFrame, TouchControlsMode};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh, quad_face_count_from_indices};
use mclone_protocol::ClientCommand;
use mclone_render::chunk::{
    ChunkCamera, ChunkTextureAtlas, TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::color_profile::RenderConfig;
use mclone_render::entity::ActorTextureAtlas;
use mclone_render::entity::{ActorFigure, ActorInstance};
use mclone_render::screen_effect::{UnderwaterEffectState, UnderwaterOverlay};
use mclone_render::selection_outline::SelectionOutline;
use mclone_render_session::{
    EngineCameraController, EngineCameraFrameState, EngineCameraInput, EngineCameraMovementImpulse,
    EngineCameraMovementMode, EngineCameraViewMode, EngineDebugVisualOptions,
    RenderSectionCacheUpdate, actor_instances_from_presentations, engine_debug_world_lines,
    local_player_actor_instance_for_view, render_camera_from_snapshot_with_view_mode,
};
use mclone_ui::{
    BlockPaletteOverlay, DEFAULT_JOIN_REMOTE_ADDR, FlatHud, GameFramePacingMode, GameHelpParent,
    GameMovementMode, GameScreen, GameUi, GameUiAction, GameUiRenderState, GuiDrawList, GuiKey,
    GuiScale, LoadingProgressOverlay, Point, StatusOverlay, render_flat_hud,
    render_loading_progress_overlay, render_loading_progress_panel_at, touch_controls_mode_label,
};

use crate::camera::{SpectatorCamera, chunk_camera_from_engine};
use crate::cli::SceneOptions;
use crate::frame_pacing::{FramePacingMode, FramePacingUiState, FrameTimingStats};
use crate::scene_runtime::{
    WindowSceneRuntime, WindowSceneStartupPump, poll_window_runtime_until_idle,
};
use crate::ui::{DebugPaneStats, render_debug_pane};
use crate::{MAX_RENDER_DISTANCE, MIN_RENDER_DISTANCE};
use mclone_render_session::{
    ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER,
};

const PLAYER_SURFACE_FEET_OFFSET: f64 = 1.0;
const GROUND_PROBE_DISTANCE: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlatClientCameraView {
    pub(crate) snapshot: mclone_render_session::EngineCameraSnapshot,
    pub(crate) eye: glam::Vec3,
    pub(crate) render_eye: glam::Vec3,
    pub(crate) view_mode: EngineCameraViewMode,
}

impl FlatClientCameraView {
    pub(crate) fn from_camera(camera: &EngineCameraController) -> Self {
        let snapshot = camera.snapshot();
        let view_mode = camera.view_mode();
        let render_camera = render_camera_from_snapshot_with_view_mode(snapshot, view_mode, 0);
        Self {
            snapshot,
            eye: glam_vec3_from_vec3d(snapshot.eye),
            render_eye: glam::Vec3::from_array(render_camera.eye),
            view_mode,
        }
    }

    pub(crate) fn block_column(self) -> (i32, i32) {
        (
            self.snapshot.eye.x.floor() as i32,
            self.snapshot.eye.z.floor() as i32,
        )
    }

    pub(crate) fn chunk_camera(self, render_distance: u32) -> ChunkCamera {
        chunk_camera_from_engine(render_camera_from_snapshot_with_view_mode(
            self.snapshot,
            self.view_mode,
            render_distance,
        ))
    }
}

pub(crate) struct FlatClientDriver {
    pub(crate) scene: SceneOptions,
    pub(crate) runtime: Option<WindowSceneRuntime>,
    pub(crate) startup: Option<FlatClientPendingStartup>,
    pub(crate) session: GameSessionCoordinator<FlatClientPendingSessionStart>,
    pub(crate) ui: GameUi,
    pub(crate) spectator: SpectatorCamera,
    pub(crate) camera: EngineCameraController,
    pub(crate) actor_interpolation: ActorInterpolationState,
    pub(crate) interaction: ClientInteractionController,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) player_collision_box_visible: bool,
    pub(crate) render_resources: Option<FlatRenderResources>,
    pub(crate) render_stats: RenderStreamStats,
    pub(crate) frame_timing: FrameTimingStats,
    underwater_effect: UnderwaterEffectState,
    seed_reroll_state: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct FlatClientPendingSessionStart {
    pub(crate) scene: SceneOptions,
    pub(crate) arm_mouse_lock: bool,
    pub(crate) show_title_on_failure: bool,
}

#[derive(Debug)]
pub(crate) struct FlatClientPendingStartup {
    request: SessionStartRequest,
    arm_mouse_lock: bool,
    show_title_on_failure: bool,
    pump: WindowSceneStartupPump,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FlatClientSessionUpdate {
    pub(crate) mouse_lock_requested: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FlatClientWorldActionStatus {
    NoRuntime,
    NoTarget,
    NoCommand,
    Sent {
        target: BlockInteractionTarget,
        changed: bool,
    },
}

pub(crate) struct FlatClientRuntimePoll {
    pub(crate) needs_section_upload: bool,
}

pub(crate) struct FlatClientSectionSync {
    pub(crate) section_update: RenderSectionCacheUpdate,
    pub(crate) remesh_ms: f64,
    pub(crate) loaded_chunk_count: usize,
}

pub(crate) struct FlatClientFullSectionSync {
    pub(crate) camera_view: FlatClientCameraView,
    pub(crate) section_update: RenderSectionCacheUpdate,
    pub(crate) sections: Vec<TexturedRenderSectionMesh>,
    pub(crate) remesh_ms: f64,
}

pub(crate) struct FlatClientCachedSections {
    pub(crate) camera_view: FlatClientCameraView,
    pub(crate) sections: Vec<TexturedRenderSectionMesh>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlatClientSectionUploadSummary {
    pub(crate) section_count: usize,
    pub(crate) face_count: u32,
    pub(crate) index_count: u32,
    pub(crate) loaded_chunk_count: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlatClientFrameInputs {
    pub(crate) camera_view: FlatClientCameraView,
    pub(crate) camera: ChunkCamera,
    pub(crate) sky_clear_color: wgpu::Color,
    pub(crate) time_of_day: f32,
    pub(crate) sun_angle: f32,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) underwater_overlay: Option<UnderwaterOverlay>,
    pub(crate) actor_instances: Vec<ActorInstance>,
    pub(crate) selection_outline: Option<SelectionOutline>,
    pub(crate) traversal_ready_sections: BTreeSet<RenderSectionKey>,
    pub(crate) render_stats: RenderStreamStats,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlatClientUiRenderOptions {
    pub(crate) render_distance: i32,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) frame_pacing: FramePacingUiState,
    pub(crate) movement_mode: GameMovementMode,
    pub(crate) fly_speed_multiplier: f32,
    pub(crate) movement_speed_multiplier: f32,
    pub(crate) player_collision_box_visible: bool,
    pub(crate) first_person_player_visible: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FlatClientDebugFrame {
    pub(crate) stats: Option<DebugPaneStats>,
    pub(crate) view_readiness_overlay: Option<LoadingProgressOverlay>,
}

#[derive(Debug)]
pub(crate) struct FlatClientUiFrame {
    pub(crate) render_options: FlatClientUiRenderOptions,
    pub(crate) block_palette: BlockPaletteOverlay,
    pub(crate) hud: Option<FlatHud>,
    pub(crate) loading_progress_overlay: Option<LoadingProgressOverlay>,
    pub(crate) debug: FlatClientDebugFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FlatClientUiActionContext<'a> {
    pub(crate) session_starting: bool,
    pub(crate) from_pointer_click: bool,
    pub(crate) fallback_remote_addr: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FlatClientHostAction {
    QuitToTitle,
    Quit,
    CycleFramePacing,
    CycleFpsCap,
    SetTouchControlsMode(TouchControlsMode),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlatClientUiActionResult {
    pub(crate) host_action: Option<FlatClientHostAction>,
    pub(crate) mouse_lock_requested: Option<bool>,
    pub(crate) session_start_queued: bool,
    pub(crate) preserve_pointer_state: bool,
    pub(crate) clear_gameplay_input: bool,
}

impl FlatClientDriver {
    pub(crate) fn new(scene: &SceneOptions, render_options: TexturedSectionRenderOptions) -> Self {
        Self::new_with_ui(scene, render_options, GameUi::new())
    }

    pub(crate) fn new_with_ui(
        scene: &SceneOptions,
        render_options: TexturedSectionRenderOptions,
        ui: GameUi,
    ) -> Self {
        let spectator = SpectatorCamera::spawn_for_scene(scene);
        let camera = engine_camera_controller_from_spectator(
            &spectator,
            scene.movement_speed_multiplier,
            scene.first_person_player_visible,
        );
        Self {
            scene: scene.clone(),
            runtime: None,
            startup: None,
            session: GameSessionCoordinator::new(),
            ui,
            spectator,
            camera,
            actor_interpolation: ActorInterpolationState::new(),
            interaction: ClientInteractionController::new(),
            render_options,
            player_collision_box_visible: false,
            render_resources: None,
            render_stats: RenderStreamStats::default(),
            frame_timing: FrameTimingStats::default(),
            underwater_effect: UnderwaterEffectState::new(),
            seed_reroll_state: initial_seed_reroll_state(scene.seed),
        }
    }

    pub(crate) fn camera_view(&self) -> FlatClientCameraView {
        FlatClientCameraView::from_camera(&self.camera)
    }

    pub(crate) fn camera_frame_state(&self) -> EngineCameraFrameState {
        self.camera.frame_state(&self.interaction)
    }

    pub(crate) fn current_render_distance(&self, fallback_render_distance: i32) -> u32 {
        self.runtime.as_ref().map_or_else(
            || u32::try_from(fallback_render_distance).unwrap_or(0),
            WindowSceneRuntime::render_distance,
        )
    }

    pub(crate) fn current_render_scale(&self, fallback_render_scale: f32) -> f32 {
        self.render_resources
            .as_ref()
            .map_or(fallback_render_scale, |resources| {
                resources.render_config().render_scale
            })
    }

    pub(crate) fn tick_frame_timing(&mut self, frame_ms: f64, target_frame_ms: Option<f64>) {
        self.render_stats.last_frame_ms = frame_ms as f32;
        self.frame_timing.begin_frame(frame_ms, target_frame_ms);
    }

    pub(crate) fn apply_held_input_frame(
        &mut self,
        frame: FlatInputFrame,
        dt_seconds: f64,
    ) -> bool {
        let Some(runtime) = self.runtime.as_ref() else {
            return false;
        };
        let input = engine_camera_input_from_flat_frame(frame, dt_seconds);
        let before = self.camera.snapshot();
        let after = self.camera.apply_movement_input(runtime.client(), input);
        after != before
    }

    pub(crate) fn clear_camera_input(&mut self) {
        self.camera.clear_keys();
    }

    pub(crate) fn ui_is_active(&self) -> bool {
        self.ui.is_active()
    }

    pub(crate) fn ui_screen(&self) -> Option<GameScreen> {
        self.ui.screen()
    }

    pub(crate) fn set_ui_screen(&mut self, screen: Option<GameScreen>) {
        self.ui.set_screen(screen);
    }

    pub(crate) fn set_new_world_seed(&mut self, seed: i64) {
        self.ui.set_new_world_seed(seed);
    }

    pub(crate) fn set_join_remote_addr(&mut self, addr: impl Into<String>) {
        self.ui.set_join_remote_addr(addr);
    }

    #[cfg(test)]
    pub(crate) fn join_remote_addr(&self) -> &str {
        self.ui.join_remote_addr()
    }

    pub(crate) fn set_ui_scale(&mut self, scale: GuiScale) {
        self.ui.set_scale(scale);
    }

    pub(crate) fn clear_ui_input(&mut self) {
        self.ui.clear_input();
    }

    pub(crate) fn open_pause_menu(&mut self) {
        self.ui.open_pause();
    }

    pub(crate) fn open_help(&mut self, parent: GameHelpParent) {
        self.ui.apply_action(GameUiAction::OpenHelp(parent));
    }

    pub(crate) fn ui_key_pressed(&mut self, key: GuiKey) -> (bool, Option<GameUiAction>) {
        self.ui.key_pressed(key)
    }

    pub(crate) fn ui_pointer_down(&mut self, point: Point, state: GameUiRenderState) -> bool {
        self.ui.pointer_down(point, state)
    }

    pub(crate) fn ui_pointer_up(
        &mut self,
        point: Point,
        state: GameUiRenderState,
    ) -> (bool, Option<GameUiAction>) {
        self.ui.pointer_up(point, state)
    }

    pub(crate) fn ui_pointer_move(
        &mut self,
        point: Point,
        state: GameUiRenderState,
    ) -> (bool, Option<GameUiAction>) {
        self.ui.pointer_move(point, state)
    }

    pub(crate) fn apply_started_session_ui(&mut self, descriptor: &ActiveSessionDescriptor) {
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

    pub(crate) fn apply_failed_session_ui(
        &mut self,
        request: &SessionStartRequest,
        show_title_on_failure: bool,
    ) {
        match request {
            SessionStartRequest::NewLocalWorld { seed } => self.ui.set_new_world_seed(*seed),
            SessionStartRequest::JoinRemote { endpoint } => {
                self.ui.set_join_remote_addr(endpoint.address.clone());
            }
            SessionStartRequest::Unknown => {}
        }
        if show_title_on_failure {
            self.ui.set_screen(Some(GameScreen::Title));
        }
    }

    pub(crate) fn apply_quit_to_title_ui(&mut self) {
        self.ui.apply_action(GameUiAction::QuitToTitle);
    }

    pub(crate) fn session_status_overlay(&self) -> StatusOverlay {
        self.session
            .status()
            .map_or_else(StatusOverlay::hidden, |status| {
                StatusOverlay::new(status.message, status.ok)
            })
    }

    pub(crate) fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.startup
            .as_ref()
            .and_then(|startup| startup.pump.progress_overlay())
    }

    pub(crate) fn request_current_scene_start(
        &mut self,
        arm_mouse_lock: bool,
        show_title_on_failure: bool,
    ) {
        let scene = self.scene.clone();
        self.session.request_start(
            session_start_request_for_scene(&scene),
            FlatClientPendingSessionStart {
                scene,
                arm_mouse_lock,
                show_title_on_failure,
            },
        );
    }

    pub(crate) fn request_local_world_start(&mut self, seed: i64, arm_mouse_lock: bool) {
        let scene = self.local_world_scene(seed);
        self.session.request_start(
            SessionStartRequest::NewLocalWorld { seed },
            FlatClientPendingSessionStart {
                scene,
                arm_mouse_lock,
                show_title_on_failure: false,
            },
        );
        self.clear_ui_input();
    }

    pub(crate) fn request_remote_session_start(
        &mut self,
        remote_addr: String,
        arm_mouse_lock: bool,
    ) {
        self.set_join_remote_addr(remote_addr.clone());
        self.session.request_start(
            SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new(remote_addr.clone()),
            },
            FlatClientPendingSessionStart {
                scene: self.remote_session_scene(remote_addr),
                arm_mouse_lock,
                show_title_on_failure: false,
            },
        );
        self.clear_ui_input();
    }

    pub(crate) fn clear_inactive_session_status(&mut self) {
        if !matches!(self.session.state(), GameSessionState::Active { .. }) {
            self.session.clear();
        }
    }

    pub(crate) fn clear_session(&mut self) {
        self.session.clear();
    }

    pub(crate) fn finish_pending_session_start(
        &mut self,
        device: Option<&wgpu::Device>,
        mut make_runtime: impl FnMut(&SceneOptions) -> anyhow::Result<WindowSceneRuntime>,
        mut make_startup_pump: impl FnMut(&SceneOptions) -> anyhow::Result<WindowSceneStartupPump>,
    ) -> FlatClientSessionUpdate {
        let Some(pending) = self.session.take_pending_start() else {
            return FlatClientSessionUpdate::default();
        };
        if matches!(&pending.request, SessionStartRequest::NewLocalWorld { .. }) {
            let request = pending.request.clone();
            let show_title_on_failure = pending.payload.show_title_on_failure;
            if let Err(err) =
                self.begin_pending_local_world_start(pending, device, &mut make_startup_pump)
            {
                self.fail_session_start(request, show_title_on_failure, err);
                return FlatClientSessionUpdate {
                    mouse_lock_requested: Some(false),
                };
            }
            return FlatClientSessionUpdate {
                mouse_lock_requested: Some(false),
            };
        }

        let request = pending.request.clone();
        let arm_mouse_lock = pending.payload.arm_mouse_lock;
        let show_title_on_failure = pending.payload.show_title_on_failure;
        let result = self.start_pending_session(pending, device, &mut make_runtime);
        self.session.apply_start_result(&result);
        match result {
            Ok(started) => {
                self.apply_started_session_ui(started.descriptor());
                FlatClientSessionUpdate {
                    mouse_lock_requested: Some(arm_mouse_lock),
                }
            }
            Err(_) => {
                self.apply_failed_session_ui(&request, show_title_on_failure);
                FlatClientSessionUpdate {
                    mouse_lock_requested: Some(false),
                }
            }
        }
    }

    pub(crate) fn advance_local_world_startup(
        &mut self,
        device: Option<&wgpu::Device>,
    ) -> FlatClientSessionUpdate {
        if self.startup.is_none() {
            return FlatClientSessionUpdate::default();
        }

        let render_eye = self.camera_view().render_eye;
        let step = match self
            .startup
            .as_mut()
            .expect("startup presence checked")
            .pump
            .step(render_eye)
        {
            Ok(step) => step,
            Err(err) => {
                let startup = self
                    .startup
                    .take()
                    .expect("startup must exist after failed step");
                self.fail_session_start(startup.request, startup.show_title_on_failure, err);
                return FlatClientSessionUpdate {
                    mouse_lock_requested: Some(false),
                };
            }
        };

        if !step.playable_ready {
            return FlatClientSessionUpdate::default();
        }

        let startup = self
            .startup
            .take()
            .expect("startup must exist after playable step");
        let request = startup.request.clone();
        let show_title_on_failure = startup.show_title_on_failure;
        match self.complete_local_world_startup(startup, step, device) {
            Ok(update) => update,
            Err(err) => {
                self.fail_session_start(request, show_title_on_failure, err);
                FlatClientSessionUpdate {
                    mouse_lock_requested: Some(false),
                }
            }
        }
    }

    pub(crate) fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        context: FlatClientUiActionContext<'_>,
    ) -> FlatClientUiActionResult {
        let mut result = FlatClientUiActionResult {
            host_action: None,
            mouse_lock_requested: (context.from_pointer_click
                && matches!(
                    action,
                    GameUiAction::StartWorld
                        | GameUiAction::Resume
                        | GameUiAction::JoinRemote
                        | GameUiAction::AssignHotbarBlock { .. }
                ))
            .then_some(true),
            session_start_queued: false,
            preserve_pointer_state: matches!(
                action,
                GameUiAction::SetRenderDistance(_)
                    | GameUiAction::SetFlySpeed(_)
                    | GameUiAction::SetMovementSpeed(_)
                    | GameUiAction::SetTouchLookSensitivity(_)
                    | GameUiAction::SetTouchControlsMode(_)
            ),
            clear_gameplay_input: true,
        };

        if context.session_starting && !matches!(action, GameUiAction::Quit) {
            result.clear_gameplay_input = false;
            result.mouse_lock_requested = None;
            result.preserve_pointer_state = true;
            return result;
        }

        let mut apply_ui_action = true;
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
            GameUiAction::TogglePlayerCollisionBox => {
                self.player_collision_box_visible = !self.player_collision_box_visible;
                log::info!(
                    "player collision box debug {}",
                    if self.player_collision_box_visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
            }
            GameUiAction::ToggleFirstPersonPlayer => {
                let visible = !self.camera.first_person_player_visible();
                self.camera.set_first_person_player_visible(visible);
                self.scene.first_person_player_visible = visible;
                log::info!(
                    "first-person player body {}",
                    if visible { "visible" } else { "hidden" }
                );
            }
            GameUiAction::SetMovementMode(movement_mode) => {
                self.camera
                    .set_movement_mode(engine_movement_mode(movement_mode));
                let movement_mode = self.camera.movement_mode();
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
            GameUiAction::SetMovementSpeed(multiplier) => {
                self.camera
                    .set_movement_speed_multiplier(f64::from(multiplier));
                let multiplier = self.camera.movement_speed_multiplier() as f32;
                self.scene.movement_speed_multiplier = multiplier;
                log::info!(
                    "movement speed multiplier set to {:.1}x",
                    self.camera.movement_speed_multiplier()
                );
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
                            return result;
                        }
                    }
                } else {
                    log::info!("new-world render distance set to {render_distance}");
                }
            }
            GameUiAction::SetTouchControlsMode(mode) => {
                result.host_action = Some(FlatClientHostAction::SetTouchControlsMode(mode));
                log::info!("touch controls set to {}", touch_controls_mode_label(mode));
            }
            GameUiAction::CycleFramePacing => {
                result.host_action = Some(FlatClientHostAction::CycleFramePacing);
            }
            GameUiAction::CycleFpsCap => {
                result.host_action = Some(FlatClientHostAction::CycleFpsCap);
            }
            GameUiAction::AssignHotbarBlock { slot, block_state } => {
                if let Err(err) = self.assign_debug_hotbar_slot(slot, BlockStateId(block_state)) {
                    log::error!("failed to assign debug hotbar slot: {err:#}");
                }
            }
            GameUiAction::OpenNewWorld => {
                let seed = self.next_new_world_seed();
                self.ui.set_new_world_seed(seed);
                self.clear_inactive_session_status();
            }
            GameUiAction::OpenJoinRemote => {
                let remote_addr = context
                    .fallback_remote_addr
                    .unwrap_or(DEFAULT_JOIN_REMOTE_ADDR)
                    .to_owned();
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
                self.request_local_world_start(seed, context.from_pointer_click);
                result.session_start_queued = true;
                result.mouse_lock_requested = Some(false);
                apply_ui_action = false;
            }
            GameUiAction::JoinRemote => {
                self.request_remote_session_start(
                    self.ui.join_remote_addr().to_owned(),
                    context.from_pointer_click,
                );
                result.session_start_queued = true;
                result.mouse_lock_requested = Some(false);
                apply_ui_action = false;
            }
            GameUiAction::QuitToTitle => {
                result.host_action = Some(FlatClientHostAction::QuitToTitle);
                apply_ui_action = false;
            }
            GameUiAction::Quit => {
                result.host_action = Some(FlatClientHostAction::Quit);
                apply_ui_action = false;
            }
            GameUiAction::BackToTitle => {
                self.clear_inactive_session_status();
            }
            GameUiAction::StartWorld
            | GameUiAction::Resume
            | GameUiAction::OpenBlockPalette
            | GameUiAction::OpenHelp(_)
            | GameUiAction::CloseHelp(_)
            | GameUiAction::OpenOptions(_)
            | GameUiAction::BackToPause
            | GameUiAction::SetTouchLookSensitivity(_) => {}
        }

        if apply_ui_action {
            self.ui.apply_action(action);
        }
        result
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

    fn start_pending_session(
        &mut self,
        pending: PendingSessionStart<FlatClientPendingSessionStart>,
        device: Option<&wgpu::Device>,
        make_runtime: &mut impl FnMut(&SceneOptions) -> anyhow::Result<WindowSceneRuntime>,
    ) -> SessionStartResult<()> {
        self.start_session_from_scene(pending.request, pending.payload.scene, device, make_runtime)
    }

    fn start_session_from_scene(
        &mut self,
        request: SessionStartRequest,
        scene: SceneOptions,
        device: Option<&wgpu::Device>,
        make_runtime: &mut impl FnMut(&SceneOptions) -> anyhow::Result<WindowSceneRuntime>,
    ) -> SessionStartResult<()> {
        let Some(descriptor) = request.active_descriptor() else {
            return Err(SessionFailure::new("Unsupported session start"));
        };
        match self.start_world_from_scene(scene, device, make_runtime) {
            Ok(()) => Ok(StartedGameSession::new(descriptor, ())),
            Err(err) => {
                log::error!("failed to start session {request:?}: {err:#}");
                Err(SessionFailure::new(request.default_failure_message()))
            }
        }
    }

    pub(crate) fn start_world_from_scene(
        &mut self,
        scene: SceneOptions,
        device: Option<&wgpu::Device>,
        make_runtime: &mut impl FnMut(&SceneOptions) -> anyhow::Result<WindowSceneRuntime>,
    ) -> anyhow::Result<()> {
        self.runtime = None;
        self.startup = None;
        self.scene = scene;
        self.reset_world_state();
        if let Some(device) = device {
            self.clear_draw_sections(device)?;
        } else {
            self.clear_render_stats();
        }

        let mut runtime =
            make_runtime(&self.scene).context("failed to create flat client world runtime")?;
        let initial_poll_start = std::time::Instant::now();
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
            initial_poll_start.elapsed().as_secs_f64() * 1000.0,
            loaded
        );
        if let Some(device) = device {
            self.upload_all_runtime_sections(device, |elapsed| elapsed.as_secs_f64() * 1000.0)
                .context("failed to upload initial world render sections")?;
        }
        Ok(())
    }

    fn begin_pending_local_world_start(
        &mut self,
        pending: PendingSessionStart<FlatClientPendingSessionStart>,
        device: Option<&wgpu::Device>,
        make_startup_pump: &mut impl FnMut(&SceneOptions) -> anyhow::Result<WindowSceneStartupPump>,
    ) -> anyhow::Result<()> {
        if pending.payload.scene.remote_addr.is_some() {
            anyhow::bail!("local startup pump received a remote scene");
        }
        if pending.request.active_descriptor().is_none() {
            anyhow::bail!("unsupported local session start");
        }

        self.startup = None;
        self.runtime = None;
        self.scene = pending.payload.scene;
        self.reset_world_state();
        if let Some(device) = device {
            self.clear_draw_sections(device)?;
        } else {
            self.clear_render_stats();
        }

        let pump =
            make_startup_pump(&self.scene).context("failed to start local world loading pump")?;
        log::info!(
            "started non-blocking local world startup seed={} center=({}, {}) render_distance={}",
            self.scene.seed,
            self.scene.chunk_x,
            self.scene.chunk_z,
            self.scene.render_distance
        );
        self.startup = Some(FlatClientPendingStartup {
            request: pending.request,
            arm_mouse_lock: pending.payload.arm_mouse_lock,
            show_title_on_failure: pending.payload.show_title_on_failure,
            pump,
        });
        Ok(())
    }

    fn complete_local_world_startup(
        &mut self,
        startup: FlatClientPendingStartup,
        step: LocalSingleViewStartupStep,
        device: Option<&wgpu::Device>,
    ) -> anyhow::Result<FlatClientSessionUpdate> {
        let descriptor = startup
            .request
            .active_descriptor()
            .context("unsupported local session start")?;
        self.runtime = Some(startup.pump.into_runtime());
        match self.apply_pending_player_position_updates() {
            Ok(true) => {}
            Ok(false) => self.place_spectator_above_loaded_surface(),
            Err(err) => {
                self.runtime = None;
                return Err(err).context("failed to apply initial server player position");
            }
        }
        if let Some(device) = device {
            self.upload_cached_runtime_sections(device)
                .context("failed to upload playable startup render sections")?;
        }
        let loaded = self
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.client().loaded_chunk_count());
        log::info!(
            "world seed={} playable startup ready polls={} poll_ms={:.3} loaded={} cached_sections={} target_ready={}/{}",
            self.scene.seed,
            step.poll_count,
            step.poll_ms,
            loaded,
            step.cached_section_count,
            step.progress
                .as_ref()
                .map_or(0, |progress| progress.target_ready_chunks),
            step.progress
                .as_ref()
                .map_or(0, |progress| progress.target_chunk_count)
        );
        self.session.complete_start(descriptor.clone());
        self.apply_started_session_ui(&descriptor);
        Ok(FlatClientSessionUpdate {
            mouse_lock_requested: Some(startup.arm_mouse_lock),
        })
    }

    fn fail_session_start(
        &mut self,
        request: SessionStartRequest,
        show_title_on_failure: bool,
        err: anyhow::Error,
    ) {
        log::error!("failed to start session {request:?}: {err:#}");
        self.startup = None;
        self.runtime = None;
        let failure_message = request.default_failure_message();
        self.apply_failed_session_ui(&request, show_title_on_failure);
        self.session
            .fail_start(SessionFailure::new(failure_message));
    }

    pub(crate) fn teardown_world(&mut self, device: Option<&wgpu::Device>) -> anyhow::Result<()> {
        self.startup = None;
        self.runtime = None;
        self.reset_world_state();
        if let Some(device) = device {
            self.clear_draw_sections(device)?;
        } else {
            self.clear_render_stats();
        }
        Ok(())
    }

    fn place_spectator_above_loaded_surface(&mut self) {
        let view = self.camera_view();
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
        if self.camera.movement_mode() == mclone_render_session::EngineCameraMovementMode::Walking {
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
            self.camera_view().eye.y
        );
    }

    pub(crate) fn render_config(&self) -> Option<RenderConfig> {
        self.render_resources
            .as_ref()
            .map(FlatRenderResources::render_config)
    }

    pub(crate) fn rebuild_render_resources(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_size: [u32; 2],
        render_config: RenderConfig,
        chunk_atlas: ChunkTextureAtlas<'_>,
        actor_atlas: ActorTextureAtlas<'_>,
        actor_figure: Option<&ActorFigure>,
        asset_source: &impl AssetSource,
    ) -> anyhow::Result<(Option<RenderConfig>, RenderConfig)> {
        let previous_config = self.render_config();
        let resources = FlatRenderResources::new(
            device,
            queue,
            target_size,
            render_config,
            chunk_atlas,
            actor_atlas,
            actor_figure,
            asset_source,
        )?;
        let next_config = resources.render_config();
        self.render_resources = Some(resources);
        Ok((previous_config, next_config))
    }

    pub(crate) fn resize_frame_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if let Some(render_resources) = &mut self.render_resources {
            render_resources.resize_frame_targets(device, size);
        }
    }

    pub(crate) fn clear_draw_sections(
        &mut self,
        device: &wgpu::Device,
    ) -> anyhow::Result<TexturedSectionUploadReport> {
        let Some(render_resources) = &mut self.render_resources else {
            return Ok(TexturedSectionUploadReport::default());
        };
        let report = render_resources
            .draw_mut()
            .update_sections(device, &[])
            .context("failed to clear world render sections")?;
        self.clear_render_stats();
        Ok(report)
    }

    pub(crate) fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        self.interaction.select_hotbar_slot(slot)
    }

    pub(crate) fn open_block_palette(&mut self) {
        self.ui.apply_action(GameUiAction::OpenBlockPalette);
    }

    pub(crate) fn assign_debug_hotbar_slot(
        &mut self,
        slot: u8,
        block_state: BlockStateId,
    ) -> anyhow::Result<bool> {
        let Some(command) = self
            .interaction
            .set_debug_hotbar_slot(slot, Some(block_state))
        else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime.send_gameplay_command(command).map_err(Into::into)
    }

    pub(crate) fn step_hotbar_slot(&mut self, step: i8) -> bool {
        if step == 0 {
            return false;
        }
        let slot_count = i16::from(FLAT_HOTBAR_SLOT_COUNT);
        let selected = i16::from(self.interaction.selected_hotbar_slot());
        let next = (selected + i16::from(step)).rem_euclid(slot_count) as u8;
        self.select_hotbar_slot(next)
    }

    pub(crate) fn toggle_camera_view_mode(&mut self) -> EngineCameraViewMode {
        self.camera.toggle_view_mode()
    }

    pub(crate) fn apply_look_frame(&mut self, frame: FlatInputFrame) -> bool {
        if frame.look_delta.x == 0.0 && frame.look_delta.y == 0.0 {
            return false;
        }
        self.camera
            .turn_mouse_delta(f64::from(frame.look_delta.x), f64::from(frame.look_delta.y));
        self.sync_spectator_from_camera();
        true
    }

    pub(crate) fn adjust_camera_speed(&mut self, amount: f64) {
        self.camera.adjust_speed(amount);
        self.sync_spectator_from_camera();
    }

    pub(crate) fn commit_player_pose_change(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.commit_engine_camera_player_pose(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn sync_server_player_pose(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.sync_engine_camera_player_pose(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn apply_pending_player_position_updates(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.apply_pending_engine_camera_position_updates(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn sync_carried_item(&mut self) -> anyhow::Result<bool> {
        let Some(command) = self.interaction.ensure_has_sent_carried_item() else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime.send_gameplay_command(command).map_err(Into::into)
    }

    pub(crate) fn shoot_debug_physics_cube(&mut self) -> anyhow::Result<bool> {
        if self.runtime.is_none() {
            return Ok(false);
        }
        self.sync_server_player_pose()?;
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(ClientCommand::ShootDebugPhysicsCube)
            .map_err(Into::into)
    }

    pub(crate) fn current_block_interaction_target(&self) -> Option<BlockInteractionTarget> {
        let runtime = self.runtime.as_ref()?;
        self.camera
            .target_block(runtime.client(), &self.interaction)
    }

    pub(crate) fn current_selection_outline(&self, ui_active: bool) -> Option<SelectionOutline> {
        if ui_active {
            return None;
        }
        self.current_block_interaction_target()
            .map(|target| SelectionOutline::new(target.outline_boxes))
    }

    pub(crate) fn handle_world_action(
        &mut self,
        action: FlatInputAction,
    ) -> anyhow::Result<FlatClientWorldActionStatus> {
        if self.runtime.is_none() {
            return Ok(FlatClientWorldActionStatus::NoRuntime);
        }
        self.sync_server_player_pose()?;
        self.sync_carried_item()?;
        let Some(target) = self.current_block_interaction_target() else {
            return Ok(FlatClientWorldActionStatus::NoTarget);
        };
        let command = match action {
            FlatInputAction::Attack => self.interaction.debug_instant_break_command(target.hit),
            FlatInputAction::Use => self.interaction.use_item_on_command(target.hit),
            _ => None,
        };
        let Some(command) = command else {
            return Ok(FlatClientWorldActionStatus::NoCommand);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(FlatClientWorldActionStatus::NoRuntime);
        };
        let changed = runtime.send_gameplay_command(command)?;
        Ok(FlatClientWorldActionStatus::Sent { target, changed })
    }

    pub(crate) fn effective_render_options(&self) -> TexturedSectionRenderOptions {
        effective_render_options_for_camera(
            self.render_options,
            self.runtime.as_ref().is_some_and(|runtime| {
                runtime.camera_inside_occluding_block(self.camera_view().render_eye)
            }),
        )
    }

    pub(crate) fn underwater_overlay(
        &mut self,
        camera_view: FlatClientCameraView,
    ) -> Option<UnderwaterOverlay> {
        let underwater = self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.camera_inside_water(camera_view.render_eye));
        let dt_seconds = (self.render_stats.last_frame_ms * 0.001).min(0.1);
        let underwater_effect = self.underwater_effect.update(underwater, dt_seconds);
        underwater.then(|| {
            UnderwaterOverlay::vanilla_from_native_camera(
                camera_view.snapshot.yaw_radians as f32,
                camera_view.snapshot.pitch_radians as f32,
            )
            .with_effect(
                underwater_effect.water_vision,
                underwater_effect.effect_strength,
            )
        })
    }

    pub(crate) fn interpolated_actor_instances(&mut self) -> Vec<ActorInstance> {
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
        .into_iter()
        .chain(local_player_actor_instance_for_view(
            &self.camera,
            runtime.client(),
        ))
        .collect()
    }

    pub(crate) fn poll_runtime(&mut self) -> anyhow::Result<FlatClientRuntimePoll> {
        if self.runtime.is_none() {
            return Ok(FlatClientRuntimePoll {
                needs_section_upload: false,
            });
        }
        let render_eye = self.camera_view().render_eye;
        let changed = {
            let runtime = self.runtime.as_mut().expect("runtime presence checked");
            runtime.poll()?
        };
        let changed = changed | self.apply_pending_player_position_updates()?;
        let needs_pending_upload = self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.has_pending_render_work(render_eye));
        Ok(FlatClientRuntimePoll {
            needs_section_upload: changed || needs_pending_upload,
        })
    }

    pub(crate) fn sync_runtime_sections(
        &mut self,
        elapsed_ms: impl FnOnce(std::time::Duration) -> f64,
    ) -> anyhow::Result<Option<FlatClientSectionSync>> {
        let camera_view = self.camera_view();
        let Some(runtime) = &mut self.runtime else {
            return Ok(None);
        };
        let remesh_start = std::time::Instant::now();
        let section_update = runtime.sync_render_sections(camera_view.render_eye)?;
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let loaded_chunk_count = runtime.client().loaded_chunk_count();
        Ok(Some(FlatClientSectionSync {
            section_update,
            remesh_ms,
            loaded_chunk_count,
        }))
    }

    pub(crate) fn sync_all_runtime_sections(
        &mut self,
        elapsed_ms: impl FnOnce(std::time::Duration) -> f64,
    ) -> anyhow::Result<Option<FlatClientFullSectionSync>> {
        let camera_view = self.camera_view();
        let Some(runtime) = &mut self.runtime else {
            return Ok(None);
        };
        let remesh_start = std::time::Instant::now();
        let section_update = runtime.sync_all_render_sections(camera_view.render_eye)?;
        let sections = runtime.cached_sections();
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        Ok(Some(FlatClientFullSectionSync {
            camera_view,
            section_update,
            sections,
            remesh_ms,
        }))
    }

    pub(crate) fn cached_runtime_sections(&self) -> Option<FlatClientCachedSections> {
        let camera_view = self.camera_view();
        let runtime = self.runtime.as_ref()?;
        Some(FlatClientCachedSections {
            camera_view,
            sections: runtime.cached_sections(),
        })
    }

    pub(crate) fn traversal_ready_section_keys(
        &self,
        camera_view: FlatClientCameraView,
    ) -> BTreeSet<RenderSectionKey> {
        self.runtime.as_ref().map_or_else(BTreeSet::new, |runtime| {
            runtime.traversal_ready_render_section_keys(camera_view.render_eye)
        })
    }

    pub(crate) fn record_section_upload(
        &mut self,
        section_update: &RenderSectionCacheUpdate,
        upload_report: TexturedSectionUploadReport,
        section_count: usize,
        index_count: u32,
        remesh_ms: f64,
        upload_ms: f64,
        loaded_chunk_count: Option<usize>,
    ) -> FlatClientSectionUploadSummary {
        let face_count = quad_face_count_from_indices(index_count);
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        record_render_section_update_stats(&mut self.render_stats, section_update, upload_report);
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        self.frame_timing.record_remesh_upload(remesh_ms, upload_ms);
        FlatClientSectionUploadSummary {
            section_count,
            face_count,
            index_count,
            loaded_chunk_count,
        }
    }

    pub(crate) fn upload_runtime_sections(
        &mut self,
        device: &wgpu::Device,
        elapsed_ms: impl FnOnce(std::time::Duration) -> f64,
    ) -> anyhow::Result<Option<(FlatClientSectionSync, FlatClientSectionUploadSummary)>> {
        let Some(sync) = self.sync_runtime_sections(elapsed_ms)? else {
            return Ok(None);
        };
        let Some(render_resources) = &mut self.render_resources else {
            return Ok(None);
        };
        let upload_start = std::time::Instant::now();
        let upload_report = render_resources
            .draw_mut()
            .apply_section_updates(
                device,
                &sync.section_update.rebuilt_sections,
                &sync.section_update.removed_section_keys,
            )
            .context("failed to upload streamed chunk section updates")?;
        let upload_ms = upload_start.elapsed().as_secs_f64() * 1000.0;
        let summary = self.record_section_upload(
            &sync.section_update,
            upload_report,
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .section_count(),
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .index_count(),
            sync.remesh_ms,
            upload_ms,
            Some(sync.loaded_chunk_count),
        );
        Ok(Some((sync, summary)))
    }

    pub(crate) fn upload_all_runtime_sections(
        &mut self,
        device: &wgpu::Device,
        elapsed_ms: impl FnOnce(std::time::Duration) -> f64,
    ) -> anyhow::Result<Option<(FlatClientFullSectionSync, FlatClientSectionUploadSummary)>> {
        let Some(sync) = self.sync_all_runtime_sections(elapsed_ms)? else {
            return Ok(None);
        };
        let traversal_ready_sections = self.traversal_ready_section_keys(sync.camera_view);
        let Some(render_resources) = &mut self.render_resources else {
            return Ok(None);
        };
        let upload_start = std::time::Instant::now();
        let upload_report = render_resources
            .draw_mut()
            .update_sections(device, &sync.sections)
            .context("failed to upload world chunk section resources")?;
        render_resources
            .draw_mut()
            .set_traversal_ready_sections(&traversal_ready_sections);
        let upload_ms = upload_start.elapsed().as_secs_f64() * 1000.0;
        let summary = self.record_section_upload(
            &sync.section_update,
            upload_report,
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .section_count(),
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .index_count(),
            sync.remesh_ms,
            upload_ms,
            None,
        );
        Ok(Some((sync, summary)))
    }

    pub(crate) fn upload_cached_runtime_sections(
        &mut self,
        device: &wgpu::Device,
    ) -> anyhow::Result<
        Option<(
            FlatClientCachedSections,
            FlatClientSectionUploadSummary,
            TexturedSectionUploadReport,
        )>,
    > {
        let Some(cached) = self.cached_runtime_sections() else {
            return Ok(None);
        };
        let traversal_ready_sections = self.traversal_ready_section_keys(cached.camera_view);
        let Some(render_resources) = &mut self.render_resources else {
            return Ok(None);
        };
        let upload_start = std::time::Instant::now();
        let upload_report = render_resources
            .draw_mut()
            .update_sections(device, &cached.sections)
            .context("failed to upload cached startup chunk section resources")?;
        render_resources
            .draw_mut()
            .set_traversal_ready_sections(&traversal_ready_sections);
        let upload_ms = upload_start.elapsed().as_secs_f64() * 1000.0;
        let summary = self.record_cached_section_upload(
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .section_count(),
            self.render_resources
                .as_ref()
                .expect("render resources present after upload")
                .index_count(),
            upload_ms,
        );
        Ok(Some((cached, summary, upload_report)))
    }

    pub(crate) fn record_cached_section_upload(
        &mut self,
        section_count: usize,
        index_count: u32,
        upload_ms: f64,
    ) -> FlatClientSectionUploadSummary {
        let face_count = quad_face_count_from_indices(index_count);
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.render_stats.last_remesh_ms = 0.0;
        self.render_stats.last_upload_ms = upload_ms;
        self.frame_timing.record_remesh_upload(0.0, upload_ms);
        FlatClientSectionUploadSummary {
            section_count,
            face_count,
            index_count,
            loaded_chunk_count: None,
        }
    }

    pub(crate) fn clear_render_stats(&mut self) {
        self.render_stats.section_count = 0;
        self.render_stats.index_count = 0;
        self.render_stats.face_count = 0;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
    }

    pub(crate) fn prepare_frame_inputs(
        &mut self,
        fallback_render_distance: i32,
        ui_active: bool,
    ) -> FlatClientFrameInputs {
        let camera_view = self.camera_view();
        let camera =
            camera_view.chunk_camera(self.current_render_distance(fallback_render_distance));
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
        let actor_instances = self.interpolated_actor_instances();
        let selection_outline = self.current_selection_outline(ui_active);
        let traversal_ready_sections = self.traversal_ready_section_keys(camera_view);
        let render_stats = self.render_stats;
        FlatClientFrameInputs {
            camera_view,
            camera,
            sky_clear_color,
            time_of_day,
            sun_angle,
            render_options,
            underwater_overlay,
            actor_instances,
            selection_outline,
            traversal_ready_sections,
            render_stats,
        }
    }

    pub(crate) fn render_full_frame<BuildGuiDraw>(
        &mut self,
        frame: mclone_render::target::RenderFrameContext<'_>,
        fallback_render_distance: i32,
        ui_active: bool,
        gui_state: FullFrameGui,
        build_gui_draw: BuildGuiDraw,
    ) -> anyhow::Result<FullFrameRenderSummary>
    where
        BuildGuiDraw: FnOnce(&RenderStreamStats) -> GuiDrawList,
    {
        let frame_inputs = self.prepare_frame_inputs(fallback_render_distance, ui_active);
        let world_debug_lines = engine_debug_world_lines(
            &self.camera,
            EngineDebugVisualOptions::new(self.player_collision_box_visible),
        );
        let Some(render_resources) = &mut self.render_resources else {
            anyhow::bail!("flat client render resources are not initialized");
        };
        render_resources
            .draw_mut()
            .set_traversal_ready_sections(&frame_inputs.traversal_ready_sections);
        let mut render_stats = frame_inputs.render_stats;
        let summary = render_resources.render_full_frame(
            frame,
            frame_inputs.camera,
            &frame_inputs.actor_instances,
            frame_inputs.underwater_overlay,
            frame_inputs.sky_clear_color,
            frame_inputs.time_of_day,
            frame_inputs.sun_angle,
            frame_inputs.render_options,
            frame_inputs.selection_outline.as_ref(),
            &world_debug_lines,
            gui_state,
            build_gui_draw,
            &mut render_stats,
        )?;
        self.render_stats = render_stats;
        Ok(summary)
    }

    pub(crate) fn render_full_frame_with_ui(
        &mut self,
        frame: mclone_render::target::RenderFrameContext<'_>,
        fallback_render_distance: i32,
        ui_frame: FlatClientUiFrame,
    ) -> anyhow::Result<FullFrameRenderSummary> {
        let ui_active = self.ui.is_active();
        let ui_covers_world = self.ui.covers_world();
        let gui_scale = self.ui.scale();
        let debug_stats = (!ui_active).then_some(ui_frame.debug.stats).flatten();
        let debug_view_readiness_overlay = debug_stats
            .is_some()
            .then_some(ui_frame.debug.view_readiness_overlay)
            .flatten();
        let hud = ui_frame.hud;
        let loading_progress_overlay = ui_frame.loading_progress_overlay;
        let gui_active = ui_active
            || debug_stats.is_some()
            || hud
                .as_ref()
                .is_some_and(mclone_ui::FlatHud::has_visible_commands)
            || loading_progress_overlay.is_some()
            || debug_view_readiness_overlay.is_some();
        let mut ui_render_state = game_ui_render_state(ui_frame.render_options);
        ui_render_state.block_palette = ui_frame.block_palette;
        let base_ui_draw = self.ui.render_draw_list(ui_render_state);
        let gui_state = FullFrameGui::new(
            gui_active,
            ui_covers_world || loading_progress_overlay.is_some(),
            [gui_scale.width, gui_scale.height],
        );
        self.render_full_frame(
            frame,
            fallback_render_distance,
            ui_active,
            gui_state,
            |stats| {
                render_flat_client_gui_draw(
                    base_ui_draw,
                    gui_scale,
                    hud.as_ref(),
                    loading_progress_overlay.as_ref(),
                    debug_stats,
                    debug_view_readiness_overlay.as_ref(),
                    stats,
                )
            },
        )
    }

    pub(crate) fn sync_spectator_from_camera(&mut self) {
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
    }

    pub(crate) fn set_spectator_camera(
        &mut self,
        spectator: SpectatorCamera,
        movement_speed_multiplier: f32,
        first_person_player_visible: bool,
    ) {
        self.spectator = spectator;
        self.camera = engine_camera_controller_from_spectator(
            &self.spectator,
            movement_speed_multiplier,
            first_person_player_visible,
        );
    }

    pub(crate) fn reset_world_state(&mut self) {
        self.runtime = None;
        self.spectator = SpectatorCamera::spawn_for_scene(&self.scene);
        self.camera = engine_camera_controller_from_spectator(
            &self.spectator,
            self.scene.movement_speed_multiplier,
            self.scene.first_person_player_visible,
        );
        self.actor_interpolation = ActorInterpolationState::new();
        self.interaction = ClientInteractionController::new();
        self.render_stats = RenderStreamStats::default();
        self.frame_timing = FrameTimingStats::default();
        self.underwater_effect.reset();
    }
}

pub(crate) fn game_ui_render_state(options: FlatClientUiRenderOptions) -> GameUiRenderState {
    GameUiRenderState {
        render_distance: options.render_distance,
        min_render_distance: MIN_RENDER_DISTANCE,
        max_render_distance: MAX_RENDER_DISTANCE,
        section_occlusion_culling: options.render_options.section_occlusion_culling,
        force_fullbright: options.render_options.force_fullbright,
        player_collision_box_visible: options.player_collision_box_visible,
        first_person_player_visible: options.first_person_player_visible,
        movement_mode: options.movement_mode,
        fly_speed_multiplier: options.fly_speed_multiplier,
        min_fly_speed_multiplier: ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER as f32,
        max_fly_speed_multiplier: ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER as f32,
        movement_speed_multiplier: options.movement_speed_multiplier,
        min_movement_speed_multiplier: ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER as f32,
        max_movement_speed_multiplier: ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER as f32,
        frame_pacing_mode: game_frame_pacing_mode(options.frame_pacing.mode),
        fps_cap: options.frame_pacing.fps_cap,
        touch_controls_mode: None,
        touch_settings: None,
        block_palette: Default::default(),
    }
}

pub(crate) const fn game_movement_mode(mode: EngineCameraMovementMode) -> GameMovementMode {
    match mode {
        EngineCameraMovementMode::Walking => GameMovementMode::Walk,
        EngineCameraMovementMode::NoClip => GameMovementMode::Fly,
        EngineCameraMovementMode::HandPush => GameMovementMode::HandPush,
    }
}

pub(crate) const fn engine_movement_mode(mode: GameMovementMode) -> EngineCameraMovementMode {
    match mode {
        GameMovementMode::Walk => EngineCameraMovementMode::Walking,
        GameMovementMode::Fly => EngineCameraMovementMode::NoClip,
        GameMovementMode::HandPush => EngineCameraMovementMode::HandPush,
    }
}

pub(crate) fn game_frame_pacing_mode(mode: FramePacingMode) -> GameFramePacingMode {
    match mode {
        FramePacingMode::Vsync => GameFramePacingMode::Vsync,
        FramePacingMode::Capped => GameFramePacingMode::Capped,
        FramePacingMode::Uncapped => GameFramePacingMode::Uncapped,
    }
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

fn render_flat_client_gui_draw(
    mut ui_draw: GuiDrawList,
    gui_scale: GuiScale,
    hud: Option<&FlatHud>,
    loading_progress_overlay: Option<&LoadingProgressOverlay>,
    debug_stats: Option<DebugPaneStats>,
    debug_view_readiness_overlay: Option<&LoadingProgressOverlay>,
    stats: &RenderStreamStats,
) -> GuiDrawList {
    if let Some(progress) = loading_progress_overlay {
        render_loading_progress_overlay(gui_scale, &mut ui_draw, progress);
    }
    if let Some(hud) = hud {
        render_flat_hud(gui_scale, &mut ui_draw, hud);
    }
    if let Some(mut debug_stats) = debug_stats {
        debug_stats.render = *stats;
        render_debug_pane(gui_scale, &mut ui_draw, &debug_stats);
    }
    if let Some(progress) = debug_view_readiness_overlay {
        render_loading_progress_panel_at(
            gui_scale,
            &mut ui_draw,
            progress,
            Point {
                x: (gui_scale.width - 132.0).max(4.0),
                y: 4.0,
            },
            "VIEW",
        );
    }
    ui_draw
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui_action_context() -> FlatClientUiActionContext<'static> {
        FlatClientUiActionContext {
            session_starting: false,
            from_pointer_click: true,
            fallback_remote_addr: Some("10.0.0.9:25565"),
        }
    }

    #[test]
    fn flat_camera_view_exposes_third_person_render_eye() {
        let mut camera =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 70.0, 8.0), 0.0, 0.0, 24.0);
        camera.set_view_mode(EngineCameraViewMode::ThirdPersonBack);

        let view = FlatClientCameraView::from_camera(&camera);

        assert_eq!(view.eye, glam::vec3(8.0, 70.0, 8.0));
        assert_eq!(view.render_eye, glam::vec3(8.0, 70.0, 4.0));
    }

    #[test]
    fn ui_action_routing_queues_local_world_start_in_driver() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        let open_result = driver.apply_ui_action(GameUiAction::OpenNewWorld, ui_action_context());
        assert_eq!(open_result.host_action, None);
        assert_eq!(driver.ui_screen(), Some(GameScreen::NewWorld));

        let seed = driver.ui.new_world_seed();
        let start_result =
            driver.apply_ui_action(GameUiAction::CreateWorld(seed), ui_action_context());
        assert_eq!(start_result.host_action, None);
        assert!(start_result.session_start_queued);
        assert_eq!(start_result.mouse_lock_requested, Some(false));
        assert_eq!(
            driver.session.state(),
            &GameSessionState::Starting {
                request: SessionStartRequest::NewLocalWorld { seed }
            }
        );
        let pending = driver.session.take_pending_start().unwrap();
        assert_eq!(pending.request, SessionStartRequest::NewLocalWorld { seed });
        assert_eq!(pending.payload.scene.seed, seed);
        assert_eq!(pending.payload.scene.remote_addr, None);
        assert!(pending.payload.arm_mouse_lock);
        assert_eq!(driver.ui_screen(), Some(GameScreen::NewWorld));
    }

    #[test]
    fn ui_action_routing_handles_join_remote_without_desktop_payloads() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        let open_result = driver.apply_ui_action(GameUiAction::OpenJoinRemote, ui_action_context());
        assert_eq!(open_result.host_action, None);
        assert_eq!(driver.ui_screen(), Some(GameScreen::JoinRemote));
        assert_eq!(driver.join_remote_addr(), "10.0.0.9:25565");

        let join_result = driver.apply_ui_action(GameUiAction::JoinRemote, ui_action_context());
        assert_eq!(join_result.host_action, None);
        assert!(join_result.session_start_queued);
        assert_eq!(join_result.mouse_lock_requested, Some(false));
        assert_eq!(
            driver.session.state(),
            &GameSessionState::Starting {
                request: SessionStartRequest::JoinRemote {
                    endpoint: RemoteSessionEndpoint::new("10.0.0.9:25565")
                }
            }
        );
        let pending = driver.session.take_pending_start().unwrap();
        assert_eq!(
            pending.request,
            SessionStartRequest::JoinRemote {
                endpoint: RemoteSessionEndpoint::new("10.0.0.9:25565")
            }
        );
        assert_eq!(
            pending.payload.scene.remote_addr,
            Some("10.0.0.9:25565".to_owned())
        );
        assert!(pending.payload.arm_mouse_lock);
        assert_eq!(driver.ui_screen(), Some(GameScreen::JoinRemote));
    }

    #[test]
    fn ui_action_toggles_player_collision_box_debug_lines() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        assert!(!driver.player_collision_box_visible);
        let result =
            driver.apply_ui_action(GameUiAction::TogglePlayerCollisionBox, ui_action_context());

        assert!(result.host_action.is_none());
        assert!(driver.player_collision_box_visible);
    }

    #[test]
    fn ui_action_toggles_first_person_player_body() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        assert!(!driver.scene.first_person_player_visible);
        assert!(!driver.camera.first_person_player_visible());
        let result =
            driver.apply_ui_action(GameUiAction::ToggleFirstPersonPlayer, ui_action_context());

        assert!(result.host_action.is_none());
        assert!(driver.scene.first_person_player_visible);
        assert!(driver.camera.first_person_player_visible());
    }

    #[test]
    fn hotbar_step_wraps_around_selected_slot() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        assert_eq!(driver.interaction.selected_hotbar_slot(), 0);
        assert!(driver.step_hotbar_slot(-1));
        assert_eq!(
            driver.interaction.selected_hotbar_slot(),
            FLAT_HOTBAR_SLOT_COUNT - 1
        );
        assert!(driver.step_hotbar_slot(2));
        assert_eq!(driver.interaction.selected_hotbar_slot(), 1);
        assert!(!driver.step_hotbar_slot(0));
        assert_eq!(driver.interaction.selected_hotbar_slot(), 1);
    }
}

pub(crate) fn effective_render_options_for_camera(
    mut render_options: TexturedSectionRenderOptions,
    camera_inside_occluding_block: bool,
) -> TexturedSectionRenderOptions {
    if camera_inside_occluding_block {
        render_options.section_occlusion_culling = false;
    }
    render_options
}

pub(crate) fn vec3d_from_glam(value: glam::Vec3) -> Vec3d {
    Vec3d::new(value.x as f64, value.y as f64, value.z as f64)
}

pub(crate) fn glam_vec3_from_vec3d(value: Vec3d) -> glam::Vec3 {
    glam::Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

pub(crate) fn engine_camera_controller_from_spectator(
    spectator: &SpectatorCamera,
    movement_speed_multiplier: f32,
    first_person_player_visible: bool,
) -> EngineCameraController {
    let mut camera = EngineCameraController::from_eye_pose(
        vec3d_from_glam(spectator.position),
        f64::from(spectator.yaw),
        f64::from(spectator.pitch),
        f64::from(spectator.speed),
    );
    camera.set_movement_speed_multiplier(f64::from(movement_speed_multiplier));
    camera.set_first_person_player_visible(first_person_player_visible);
    camera
}

pub(crate) fn sync_spectator_from_camera(
    spectator: &mut SpectatorCamera,
    camera: &EngineCameraController,
) {
    let snapshot = camera.snapshot();
    spectator.position = glam_vec3_from_vec3d(snapshot.eye);
    spectator.yaw = snapshot.yaw_radians as f32;
    spectator.pitch = snapshot.pitch_radians as f32;
    spectator.speed = snapshot.speed_blocks_per_second as f32;
}

pub(crate) fn engine_camera_input_from_flat_frame(
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
        hand_push_emulation: true,
        ..EngineCameraInput::default()
    }
}
