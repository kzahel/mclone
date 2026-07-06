use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context;
use mclone_app_runtime::client_catalog_policy::{ClientCatalogEffects, ClientCatalogRequest};
use mclone_app_runtime::client_experience::{
    ClientExperienceActionContext, ClientExperienceCapabilityProjection,
    ClientExperienceCapabilityStatus, ClientExperienceController, ClientExperienceEffects,
    ClientExperienceGameplayEffect, ClientExperienceProfile, ClientExperienceProjectionEffect,
    ClientExperienceSettingEffect, ClientExperienceSettingsProfile, ClientExperienceSettingsState,
    client_experience_should_apply_ui_projection,
};
use mclone_app_runtime::client_session_policy::{
    ClientSessionEffects, ClientSessionHostAction, ClientSessionStatusProjection,
    ClientSessionTransitionEffects, ClientSessionUiEffects, client_session_failed_start_ui_effects,
    client_session_quit_to_title_transition, client_session_should_clear_inactive_status,
    client_session_start_transition, client_session_status_projection,
};
use mclone_app_runtime::far_lod::{
    MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS, MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
};
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
use mclone_app_runtime::set_player_appearance_command_for_ui_model;
use mclone_app_runtime::world_catalog::{
    LocalWorldId, LocalWorldSummary, NativeWorldCatalog, WorldCatalogCapabilities,
    WorldCatalogError, WorldCatalogRequest, WorldCatalogResponse,
};
use mclone_assets::{ActorFigureId, AssetSource};
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, BlockInteractionTarget,
    ClientInteractionController, LOCAL_PLAYER_STANDING_EYE_HEIGHT, NativeTeleportPreviewWorker,
    TeleportConfig, TeleportIntent, TeleportPreview, TeleportPreviewRequestId,
    TeleportPreviewResult, TeleportValidityReason,
};
use mclone_core::{BlockStateId, Vec3d};
use mclone_input::{
    FLAT_HOTBAR_SLOT_COUNT, FlatInputAction, FlatInputFrame, TouchControlsMode,
    keyboard_turn_mouse_delta,
};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh, quad_face_count_from_indices};
use mclone_protocol::ClientCommand;
use mclone_render::chunk::{
    ChunkTextureAtlas, PerspectiveRenderPose, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
use mclone_render::color_profile::RenderConfig;
use mclone_render::entity::ActorTextureAtlas;
use mclone_render::entity::{ActorFigureSet, ActorInstance};
use mclone_render::gui::WorldGuiLine;
use mclone_render::screen_effect::{UnderwaterEffectState, UnderwaterOverlay};
use mclone_render::selection_outline::SelectionOutline;
use mclone_render_session::{
    EngineCameraController, EngineCameraFrameState, EngineCameraInput, EngineCameraMovementImpulse,
    EngineCameraMovementMode, EngineCameraViewMode, EngineDebugVisualOptions,
    RenderSectionCacheUpdate, actor_instances_from_presentations, engine_debug_world_lines,
    local_player_actor_instance_for_view, render_pose_from_snapshot_with_view_mode,
};
use mclone_server::SimulationCadenceConfig;
use mclone_ui::{
    BlockPaletteOverlay, DEFAULT_JOIN_REMOTE_ADDR, FlatHud, FramePipelineHudOverlay,
    GameFramePacingMode, GameHelpParent, GameMovementMode, GamePlayerModel, GameScreen,
    GameSimulationCadence, GameUiAction, GameUiHost, GameUiRenderState, GuiKey, GuiScale,
    LoadingProgressOverlay, LoadingProgressOverlayLayer, Point, StatusOverlay, UiDebugSnapshot,
    UiDrawCacheStats, WorldCatalogUiState, WorldCatalogUiStatus, touch_controls_mode_label,
};

use crate::camera::SpectatorCamera;
use crate::cli::SceneOptions;
use crate::frame_pacing::{FramePacingMode, FramePacingUiState, FrameTimingStats};
use crate::frame_pipeline_accounting::DesktopFramePipelineAccounting;
use crate::scene_runtime::{
    WindowSceneRuntime, WindowSceneStartupPump, poll_window_runtime_until_idle,
};
use crate::ui::DebugPaneStats;
use crate::{MAX_RENDER_DISTANCE, MIN_RENDER_DISTANCE};
use mclone_render_session::{
    ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER,
};

const PLAYER_SURFACE_FEET_OFFSET: f64 = 1.0;
const GROUND_PROBE_DISTANCE: f64 = 0.01;
const DESKTOP_BLINK_DEBUG_MAX_DISTANCE: f64 = 8.0;
const DESKTOP_BLINK_DEBUG_ARC_HEIGHT: f64 = 1.25;
const DESKTOP_BLINK_DEBUG_ORIGIN_LEFT_OFFSET: f64 = 0.35;
const DESKTOP_BLINK_DEBUG_ORIGIN_DOWN_OFFSET: f64 = 0.25;
const DESKTOP_BLINK_DEBUG_ORIGIN_FORWARD_OFFSET: f64 = 0.2;
const DESKTOP_BLINK_DEBUG_UPWARD_PITCH_BIAS_RADIANS: f64 = 0.18;
const DESKTOP_BLINK_DEBUG_VALID_ARC_COLOR: [f32; 4] = [0.1, 0.85, 1.0, 1.0];
const DESKTOP_BLINK_DEBUG_INVALID_ARC_COLOR: [f32; 4] = [1.0, 0.25, 0.15, 1.0];
const DESKTOP_BLINK_DEBUG_FEET_COLOR: [f32; 4] = [0.1, 1.0, 0.35, 1.0];
const DESKTOP_BLINK_DEBUG_DOT_COLOR: [f32; 4] = [1.0, 0.95, 0.2, 1.0];
const DESKTOP_BLINK_DEBUG_MARKER_RADIUS: f64 = 0.28;
const DESKTOP_BLINK_DEBUG_DOT_RADIUS: f64 = 0.1;

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
        let render_pose = render_pose_from_snapshot_with_view_mode(snapshot, view_mode, 0);
        Self {
            snapshot,
            eye: glam_vec3_from_vec3d(snapshot.eye),
            render_eye: render_pose.eye,
            view_mode,
        }
    }

    pub(crate) fn block_column(self) -> (i32, i32) {
        (
            self.snapshot.eye.x.floor() as i32,
            self.snapshot.eye.z.floor() as i32,
        )
    }

    pub(crate) fn render_pose(self, render_distance: u32) -> PerspectiveRenderPose {
        render_pose_from_snapshot_with_view_mode(self.snapshot, self.view_mode, render_distance)
    }
}

pub(crate) struct FlatClientDriver {
    pub(crate) scene: SceneOptions,
    pub(crate) runtime: Option<WindowSceneRuntime>,
    pub(crate) startup: Option<FlatClientPendingStartup>,
    pub(crate) session: GameSessionCoordinator<FlatClientPendingSessionStart>,
    pub(crate) ui: GameUiHost,
    pub(crate) spectator: SpectatorCamera,
    pub(crate) camera: EngineCameraController,
    pub(crate) actor_interpolation: ActorInterpolationState,
    pub(crate) interaction: ClientInteractionController,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) player_collision_box_visible: bool,
    pub(crate) crosshair_visible: bool,
    pub(crate) frame_pipeline_overlay_visible: bool,
    pub(crate) player_model: GamePlayerModel,
    pub(crate) render_resources: Option<FlatRenderResources>,
    pub(crate) render_stats: RenderStreamStats,
    pub(crate) frame_timing: FrameTimingStats,
    frame_pipeline_accounting: DesktopFramePipelineAccounting,
    world_catalog: Option<NativeWorldCatalog>,
    client_experience: ClientExperienceController,
    status_overlay: StatusOverlay,
    desktop_blink_debug: DesktopBlinkDebugState,
    desktop_blink_debug_worker: Option<NativeTeleportPreviewWorker>,
    underwater_effect: UnderwaterEffectState,
    seed_reroll_state: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct FlatClientPendingSessionStart {
    pub(crate) scene: SceneOptions,
    pub(crate) arm_mouse_lock: bool,
    pub(crate) show_title_on_failure: bool,
    pub(crate) descriptor: Option<ActiveSessionDescriptor>,
    pub(crate) transition: ClientSessionTransitionEffects,
}

#[derive(Debug)]
pub(crate) struct FlatClientPendingStartup {
    request: SessionStartRequest,
    descriptor: ActiveSessionDescriptor,
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
    pub(crate) sync_timing: mclone_app_runtime::RenderSectionSyncTiming,
    pub(crate) remesh_ms: f64,
    pub(crate) loaded_chunk_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DesktopBlinkDebugState {
    active: bool,
    preview: Option<TeleportPreview>,
    first_request_id: Option<TeleportPreviewRequestId>,
    latest_request_id: Option<TeleportPreviewRequestId>,
    preview_request_id: Option<TeleportPreviewRequestId>,
    last_submitted_intent: Option<TeleportIntent>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DesktopBlinkDebugCommitStatus {
    Inactive,
    NoRuntime,
    NoValidPreview { validity: TeleportValidityReason },
    Committed { target_feet: Vec3d, changed: bool },
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
    pub(crate) camera: PerspectiveRenderPose,
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
    pub(crate) far_lod_enabled: bool,
    pub(crate) far_lod_range_chunks: i32,
    pub(crate) frame_pacing: FramePacingUiState,
    pub(crate) movement_mode: GameMovementMode,
    pub(crate) fly_speed_multiplier: f32,
    pub(crate) movement_speed_multiplier: f32,
    pub(crate) player_collision_box_visible: bool,
    pub(crate) first_person_player_visible: bool,
    pub(crate) crosshair_visible: bool,
    pub(crate) frame_pipeline_overlay_visible: bool,
    pub(crate) player_model: GamePlayerModel,
    pub(crate) server_cadence: Option<GameSimulationCadence>,
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlatClientUiPointerClickReport {
    pub(crate) before: Option<UiDebugSnapshot>,
    pub(crate) down_handled: bool,
    pub(crate) after_down: Option<UiDebugSnapshot>,
    pub(crate) up_handled: bool,
    pub(crate) after_up: Option<UiDebugSnapshot>,
    pub(crate) action: Option<GameUiAction>,
    pub(crate) action_result: Option<FlatClientUiActionResult>,
}

impl FlatClientDriver {
    pub(crate) fn new(scene: &SceneOptions, render_options: TexturedSectionRenderOptions) -> Self {
        Self::new_with_ui(scene, render_options, GameUiHost::new())
    }

    pub(crate) fn new_with_ui(
        scene: &SceneOptions,
        render_options: TexturedSectionRenderOptions,
        ui: GameUiHost,
    ) -> Self {
        let spectator = SpectatorCamera::spawn_for_scene(scene);
        let camera = engine_camera_controller_from_spectator(
            &spectator,
            scene.movement_speed_multiplier,
            scene.first_person_player_visible,
        );
        let mut driver = Self {
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
            crosshair_visible: true,
            frame_pipeline_overlay_visible: false,
            player_model: GamePlayerModel::default(),
            render_resources: None,
            render_stats: RenderStreamStats::default(),
            frame_timing: FrameTimingStats::default(),
            frame_pipeline_accounting: DesktopFramePipelineAccounting::default(),
            world_catalog: scene.world_root.clone().map(NativeWorldCatalog::new),
            client_experience: ClientExperienceController::new(desktop_client_experience_profile()),
            status_overlay: StatusOverlay::hidden(),
            desktop_blink_debug: DesktopBlinkDebugState::default(),
            desktop_blink_debug_worker: None,
            underwater_effect: UnderwaterEffectState::new(),
            seed_reroll_state: initial_seed_reroll_state(scene.seed),
        };
        driver.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        driver
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

    pub(crate) fn server_simulation_cadence(&self) -> Option<GameSimulationCadence> {
        self.runtime
            .as_ref()
            .and_then(WindowSceneRuntime::simulation_cadence)
            .map(game_simulation_cadence_from_config)
    }

    pub(crate) fn tick_frame_timing(&mut self, frame_ms: f64, target_frame_ms: Option<f64>) {
        self.render_stats.last_frame_ms = frame_ms as f32;
        self.frame_timing.begin_frame(frame_ms, target_frame_ms);
        self.frame_pipeline_accounting
            .begin_frame(frame_ms, target_frame_ms);
    }

    pub(crate) fn latest_frame_pipeline_overlay(&self) -> Option<FramePipelineHudOverlay> {
        if !self.frame_pipeline_overlay_visible {
            return None;
        }
        self.frame_pipeline_accounting
            .latest_report()
            .map(|(report, revision)| FramePipelineHudOverlay::new(report, revision))
    }

    pub(crate) fn record_runtime_poll_timing(&mut self, elapsed_ms: f64) {
        self.frame_timing.record_runtime_poll(elapsed_ms);
        self.frame_pipeline_accounting
            .record_runtime_poll(elapsed_ms);
    }

    pub(crate) fn record_surface_frame_timing(
        &mut self,
        render_ms: f64,
        acquire_ms: f64,
        encode_ms: f64,
        submit_ms: f64,
        present_ms: f64,
    ) {
        self.frame_timing
            .record_surface_frame(render_ms, acquire_ms, encode_ms, submit_ms, present_ms);
        let runtime_stats = self.runtime.as_ref().map(WindowSceneRuntime::stats);
        self.frame_pipeline_accounting.finish_frame(
            render_ms,
            acquire_ms,
            submit_ms,
            present_ms,
            runtime_stats,
            self.render_stats,
        );
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

    pub(crate) fn begin_desktop_blink_debug(&mut self) -> bool {
        if self.runtime.is_none() {
            self.desktop_blink_debug = DesktopBlinkDebugState::default();
            return false;
        }
        if !self.ensure_desktop_blink_debug_worker() {
            self.desktop_blink_debug = DesktopBlinkDebugState::default();
            return false;
        }
        self.desktop_blink_debug.active = true;
        self.desktop_blink_debug.preview = None;
        self.desktop_blink_debug.first_request_id = None;
        self.desktop_blink_debug.latest_request_id = None;
        self.desktop_blink_debug.preview_request_id = None;
        self.desktop_blink_debug.last_submitted_intent = None;
        self.update_desktop_blink_debug_preview();
        true
    }

    pub(crate) fn clear_desktop_blink_debug(&mut self) {
        self.desktop_blink_debug = DesktopBlinkDebugState::default();
    }

    pub(crate) fn update_desktop_blink_debug_preview(&mut self) -> bool {
        if !self.desktop_blink_debug.active {
            return false;
        }
        let mut changed = self.submit_desktop_blink_debug_request();
        changed |= self.poll_desktop_blink_debug_worker();
        changed
    }

    pub(crate) fn wait_for_desktop_blink_debug_preview(&mut self, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            self.update_desktop_blink_debug_preview();
            if self.desktop_blink_debug.preview.is_some() {
                return true;
            }
            if start.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    pub(crate) fn commit_desktop_blink_debug(
        &mut self,
    ) -> anyhow::Result<DesktopBlinkDebugCommitStatus> {
        if !self.desktop_blink_debug.active {
            return Ok(DesktopBlinkDebugCommitStatus::Inactive);
        }
        if self.runtime.is_none() {
            self.clear_desktop_blink_debug();
            return Ok(DesktopBlinkDebugCommitStatus::NoRuntime);
        }

        self.poll_desktop_blink_debug_worker();
        self.desktop_blink_debug.active = false;
        let Some(preview) = self.desktop_blink_debug.preview.take() else {
            return Ok(DesktopBlinkDebugCommitStatus::NoValidPreview {
                validity: TeleportValidityReason::NoCandidate,
            });
        };
        let Some(target_feet) = preview.target_feet.filter(|_| preview.is_valid()) else {
            return Ok(DesktopBlinkDebugCommitStatus::NoValidPreview {
                validity: preview.validity,
            });
        };

        let pitch_radians = self.camera.snapshot().pitch_radians;
        self.camera.set_player_feet_pose(
            target_feet,
            -preview.target_yaw_degrees.to_radians(),
            pitch_radians,
        );
        if let Some(runtime) = self.runtime.as_ref() {
            self.camera
                .probe_ground(runtime.client(), GROUND_PROBE_DISTANCE);
        }
        let changed = self.commit_player_pose_change()?;
        Ok(DesktopBlinkDebugCommitStatus::Committed {
            target_feet,
            changed,
        })
    }

    fn ensure_desktop_blink_debug_worker(&mut self) -> bool {
        if self.desktop_blink_debug_worker.is_some() {
            return true;
        }
        match NativeTeleportPreviewWorker::new() {
            Ok(worker) => {
                self.desktop_blink_debug_worker = Some(worker);
                true
            }
            Err(error) => {
                log::warn!("failed to start desktop Blink debug worker: {error}");
                false
            }
        }
    }

    fn submit_desktop_blink_debug_request(&mut self) -> bool {
        let intent = desktop_blink_debug_intent(&self.camera);
        if self.desktop_blink_debug.last_submitted_intent == Some(intent) {
            return false;
        }
        let Some(runtime) = self.runtime.as_ref() else {
            return false;
        };
        let Some(worker) = self.desktop_blink_debug_worker.as_mut() else {
            return false;
        };
        match worker.submit_from_world(runtime.client(), intent, desktop_blink_debug_config()) {
            Ok(request_id) => {
                self.desktop_blink_debug
                    .first_request_id
                    .get_or_insert(request_id);
                self.desktop_blink_debug.latest_request_id = Some(request_id);
                self.desktop_blink_debug.last_submitted_intent = Some(intent);
                true
            }
            Err(error) => {
                log::warn!("desktop Blink debug preview submit failed: {error}");
                self.desktop_blink_debug_worker = None;
                false
            }
        }
    }

    fn poll_desktop_blink_debug_worker(&mut self) -> bool {
        let Some(worker) = self.desktop_blink_debug_worker.as_mut() else {
            return false;
        };
        match worker.try_recv_latest() {
            Ok(Some(result)) => self.accept_desktop_blink_debug_result(result),
            Ok(None) => false,
            Err(error) => {
                log::warn!("desktop Blink debug preview worker failed: {error}");
                self.desktop_blink_debug_worker = None;
                false
            }
        }
    }

    fn accept_desktop_blink_debug_result(&mut self, result: TeleportPreviewResult) -> bool {
        if !self.desktop_blink_debug.active {
            return false;
        }
        if self
            .desktop_blink_debug
            .first_request_id
            .is_some_and(|first_request_id| result.id < first_request_id)
        {
            return false;
        }
        if self
            .desktop_blink_debug
            .preview_request_id
            .is_some_and(|preview_request_id| result.id <= preview_request_id)
        {
            return false;
        }
        self.desktop_blink_debug.preview = Some(result.preview);
        self.desktop_blink_debug.preview_request_id = Some(result.id);
        true
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

    pub(crate) fn ui_pointer_down(&mut self, point: Point) -> bool {
        self.ui.pointer_down(point)
    }

    pub(crate) fn ui_pointer_up(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        self.ui.pointer_up(point)
    }

    pub(crate) fn ui_pointer_move(&mut self, point: Point) -> (bool, Option<GameUiAction>) {
        self.ui.pointer_move(point)
    }

    pub(crate) fn set_ui_v2_debug_overlay(&mut self, enabled: bool) {
        self.ui.set_v2_debug_overlay(enabled);
    }

    pub(crate) fn ui_v2_is_active(&self) -> bool {
        self.ui.v2_is_active()
    }

    pub(crate) fn ui_v2_debug_snapshot(&mut self) -> Option<UiDebugSnapshot> {
        self.ui.v2_debug_snapshot()
    }

    pub(crate) fn commit_ui_render_state(&mut self, state: GameUiRenderState) {
        self.ui.commit_render_state(state);
    }

    #[cfg(test)]
    pub(crate) fn ui_v2_debug_snapshot_for_state(
        &mut self,
        state: GameUiRenderState,
    ) -> Option<UiDebugSnapshot> {
        self.commit_ui_render_state(state);
        self.ui_v2_debug_snapshot()
    }

    pub(crate) fn current_ui_render_state(
        &self,
        frame_pacing: FramePacingUiState,
        block_palette: BlockPaletteOverlay,
    ) -> GameUiRenderState {
        let mut state = game_ui_render_state(FlatClientUiRenderOptions {
            render_distance: self.current_render_distance(self.scene.render_distance) as i32,
            render_options: self.render_options,
            far_lod_enabled: self.scene.far_lod.enabled,
            far_lod_range_chunks: self.scene.far_lod.extra_radius_chunks as i32,
            frame_pacing,
            movement_mode: game_movement_mode(self.camera.movement_mode()),
            fly_speed_multiplier: self.camera.fly_speed_multiplier() as f32,
            movement_speed_multiplier: self.camera.movement_speed_multiplier() as f32,
            player_collision_box_visible: self.player_collision_box_visible,
            first_person_player_visible: self.camera.first_person_player_visible(),
            crosshair_visible: self.crosshair_visible,
            frame_pipeline_overlay_visible: self.frame_pipeline_overlay_visible,
            player_model: self.player_model,
            server_cadence: self.server_simulation_cadence(),
        });
        state.world_catalog = self.world_catalog_ui_for_render();
        state.block_palette = block_palette;
        state
    }

    fn world_catalog_ui_for_render(&self) -> WorldCatalogUiState {
        self.client_experience
            .catalog()
            .ui_state_with_active_world(self.active_local_world_id())
    }

    fn refresh_world_catalog_ui(&mut self, status: WorldCatalogUiStatus) {
        let Some(catalog) = self.world_catalog.clone() else {
            self.client_experience.catalog_mut().set_worlds(
                WorldCatalogCapabilities::default(),
                Vec::new(),
                status,
            );
            return;
        };

        match catalog.list_worlds() {
            Ok(worlds) => {
                self.client_experience.catalog_mut().set_worlds(
                    catalog.capabilities(),
                    worlds,
                    status,
                );
            }
            Err(error) => {
                log::warn!("failed to refresh local world catalog: {error}");
                self.client_experience.catalog_mut().set_worlds(
                    catalog.capabilities(),
                    Vec::new(),
                    WorldCatalogUiStatus::new(&error.message, false),
                );
            }
        }
    }

    fn active_local_world_id(&self) -> Option<&LocalWorldId> {
        let GameSessionState::Active { session } = self.session.state() else {
            return None;
        };
        session.local_world_id()
    }

    pub(crate) fn apply_ui_pointer_click(
        &mut self,
        point: Point,
        context: FlatClientUiActionContext<'_>,
    ) -> FlatClientUiPointerClickReport {
        let before = self.ui_v2_debug_snapshot();
        let down_handled = self.ui_pointer_down(point);
        let after_down = self.ui_v2_debug_snapshot();
        let (up_handled, action) = self.ui_pointer_up(point);
        let after_up = self.ui_v2_debug_snapshot();
        let action_result = action.map(|action| self.apply_ui_action(action, context));

        FlatClientUiPointerClickReport {
            before,
            down_handled,
            after_down,
            up_handled,
            after_up,
            action,
            action_result,
        }
    }

    pub(crate) fn apply_started_session_ui(&mut self, descriptor: &ActiveSessionDescriptor) {
        match descriptor {
            ActiveSessionDescriptor::LocalWorld {
                seed,
                id,
                display_name,
            } => {
                if let Some(id) = id {
                    self.ui.close();
                    log::info!(
                        "started local world {} ({}) seed={seed}",
                        display_name.as_deref().unwrap_or(id.as_str()),
                        id
                    );
                } else {
                    self.ui.apply_action(GameUiAction::CreateWorld(*seed));
                    log::info!("created local world seed={seed}");
                }
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
        self.apply_client_session_ui_effects(client_session_failed_start_ui_effects(
            request,
            show_title_on_failure,
        ));
    }

    pub(crate) fn quit_to_title_transition_effects(&self) -> ClientSessionTransitionEffects {
        client_session_quit_to_title_transition(self.session.state())
    }

    pub(crate) fn session_projection(&self) -> ClientSessionStatusProjection {
        client_session_status_projection(
            self.session.status(),
            self.status_overlay.clone(),
            self.startup_progress_overlay(),
        )
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
        let request = session_start_request_for_scene(&scene);
        let transition = client_session_start_transition(self.session.state());
        self.session.request_start(
            request,
            FlatClientPendingSessionStart {
                scene,
                arm_mouse_lock,
                show_title_on_failure,
                descriptor: None,
                transition,
            },
        );
    }

    pub(crate) fn request_local_world_start(&mut self, seed: i64, arm_mouse_lock: bool) {
        let scene = self.local_world_scene(seed);
        let request = SessionStartRequest::new_seed_local_world(seed);
        let transition = client_session_start_transition(self.session.state());
        self.session.request_start(
            request,
            FlatClientPendingSessionStart {
                scene,
                arm_mouse_lock,
                show_title_on_failure: false,
                descriptor: None,
                transition,
            },
        );
        self.clear_ui_input();
    }

    fn request_catalog_world_start(
        &mut self,
        request: SessionStartRequest,
        descriptor: ActiveSessionDescriptor,
        scene: SceneOptions,
        arm_mouse_lock: bool,
    ) {
        let transition = client_session_start_transition(self.session.state());
        self.session.request_start(
            request,
            FlatClientPendingSessionStart {
                scene,
                arm_mouse_lock,
                show_title_on_failure: false,
                descriptor: Some(descriptor),
                transition,
            },
        );
        self.clear_ui_input();
    }

    fn apply_world_catalog_effects(
        &mut self,
        effects: ClientCatalogEffects,
        arm_mouse_lock: bool,
    ) -> bool {
        let mut session_start_queued = false;
        for start in effects.session_starts {
            session_start_queued = true;
            let Some(catalog) = self.world_catalog.clone() else {
                log::warn!(
                    "shared catalog controller returned a session start without a native catalog"
                );
                continue;
            };
            self.request_catalog_world_start(
                start.request,
                start.descriptor,
                self.catalog_world_scene(&start.summary, catalog.world_dir(&start.summary.id)),
                arm_mouse_lock,
            );
        }
        for request in effects.catalog_requests {
            session_start_queued |= self.execute_world_catalog_request(request, arm_mouse_lock);
        }
        session_start_queued
    }

    fn execute_world_catalog_request(
        &mut self,
        request: ClientCatalogRequest,
        arm_mouse_lock: bool,
    ) -> bool {
        let Some(catalog) = self.world_catalog.clone() else {
            let error = WorldCatalogError::unsupported("Persistent worlds unavailable");
            log::warn!("world catalog action failed: {error}");
            let effects = self
                .client_experience
                .catalog_mut()
                .apply_catalog_error(request.id, error);
            return self.apply_world_catalog_effects(effects, arm_mouse_lock);
        };

        let response = match request.request {
            WorldCatalogRequest::ListWorlds => match catalog.list_worlds() {
                Ok(worlds) => WorldCatalogResponse::WorldList {
                    capabilities: catalog.capabilities(),
                    worlds,
                },
                Err(error) => {
                    log::warn!("world catalog list failed: {error}");
                    let effects = self
                        .client_experience
                        .catalog_mut()
                        .apply_catalog_error(request.id, error);
                    return self.apply_world_catalog_effects(effects, arm_mouse_lock);
                }
            },
            WorldCatalogRequest::CreateWorld { options } => match catalog.create_world(options) {
                Ok(summary) => WorldCatalogResponse::WorldCreated { summary },
                Err(error) => {
                    log::warn!("world catalog create failed: {error}");
                    let effects = self
                        .client_experience
                        .catalog_mut()
                        .apply_catalog_error(request.id, error);
                    return self.apply_world_catalog_effects(effects, arm_mouse_lock);
                }
            },
            WorldCatalogRequest::OpenWorld { id } => match catalog.open_world(&id) {
                Ok(opened) => WorldCatalogResponse::WorldOpened {
                    summary: opened.summary,
                },
                Err(error) => {
                    log::warn!("world catalog open failed: {error}");
                    let effects = self
                        .client_experience
                        .catalog_mut()
                        .apply_catalog_error(request.id, error);
                    return self.apply_world_catalog_effects(effects, arm_mouse_lock);
                }
            },
            WorldCatalogRequest::DeleteWorld { id } => {
                let active_world = self.active_local_world_id().cloned();
                match catalog.delete_world(&id, active_world.as_ref()) {
                    Ok(summary) => WorldCatalogResponse::WorldDeleted { id: summary.id },
                    Err(error) => {
                        log::warn!("world catalog delete failed: {error}");
                        let effects = self
                            .client_experience
                            .catalog_mut()
                            .apply_catalog_error(request.id, error);
                        return self.apply_world_catalog_effects(effects, arm_mouse_lock);
                    }
                }
            }
        };

        let effects = self
            .client_experience
            .catalog_mut()
            .apply_catalog_response(request.id, response);
        self.apply_world_catalog_effects(effects, arm_mouse_lock)
    }

    fn client_experience_settings_state(&self) -> ClientExperienceSettingsState {
        ClientExperienceSettingsState::from(game_ui_render_state(FlatClientUiRenderOptions {
            render_distance: self.current_render_distance(self.scene.render_distance) as i32,
            render_options: self.render_options,
            far_lod_enabled: self.scene.far_lod.enabled,
            far_lod_range_chunks: self.scene.far_lod.extra_radius_chunks as i32,
            frame_pacing: FramePacingUiState::default(),
            movement_mode: game_movement_mode(self.camera.movement_mode()),
            fly_speed_multiplier: self.camera.fly_speed_multiplier() as f32,
            movement_speed_multiplier: self.camera.movement_speed_multiplier() as f32,
            player_collision_box_visible: self.player_collision_box_visible,
            first_person_player_visible: self.camera.first_person_player_visible(),
            crosshair_visible: self.crosshair_visible,
            frame_pipeline_overlay_visible: self.frame_pipeline_overlay_visible,
            player_model: self.player_model,
            server_cadence: self.server_simulation_cadence(),
        }))
    }

    fn apply_client_experience_effects(
        &mut self,
        effects: ClientExperienceEffects,
        arm_mouse_lock: bool,
        result: &mut FlatClientUiActionResult,
    ) -> bool {
        if self.apply_world_catalog_effects(effects.catalog, arm_mouse_lock) {
            result.session_start_queued = true;
            result.mouse_lock_requested = Some(false);
        }
        self.apply_client_session_effects(effects.session, arm_mouse_lock, result);
        if !self.apply_client_experience_settings_effects(effects.settings, result) {
            return false;
        }
        for effect in effects.gameplay {
            match effect {
                ClientExperienceGameplayEffect::AssignHotbarBlock { slot, block_state } => {
                    if let Err(err) = self.assign_debug_hotbar_slot(slot, BlockStateId(block_state))
                    {
                        log::error!("failed to assign debug hotbar slot: {err:#}");
                    }
                }
            }
        }
        for effect in effects.projection {
            match effect {
                ClientExperienceProjectionEffect::ApplyUiAction(action) => {
                    self.ui.apply_action(action);
                }
            }
        }
        true
    }

    fn apply_client_session_effects(
        &mut self,
        effects: ClientSessionEffects,
        arm_mouse_lock: bool,
        result: &mut FlatClientUiActionResult,
    ) {
        if let Some(seed) = effects.new_world_seed {
            self.ui.set_new_world_seed(seed);
        }
        if let Some(addr) = effects.join_remote_addr {
            self.ui.set_join_remote_addr(addr);
        }
        if effects.clear_inactive_session_status {
            self.clear_inactive_session_status();
        }
        if let Some(request) = effects.session_start {
            match request {
                SessionStartRequest::CreateLocalWorld { options } => {
                    self.request_local_world_start(options.seed, arm_mouse_lock);
                    result.session_start_queued = true;
                    result.mouse_lock_requested = Some(false);
                }
                SessionStartRequest::JoinRemote { endpoint } => {
                    self.request_remote_session_start(endpoint.address, arm_mouse_lock);
                    result.session_start_queued = true;
                    result.mouse_lock_requested = Some(false);
                }
                SessionStartRequest::OpenLocalWorld { .. } | SessionStartRequest::Unknown => {
                    log::warn!(
                        "shared flat session policy emitted unsupported desktop start: {request:?}"
                    );
                }
            }
        }
        if let Some(host_action) = effects.host_action {
            result.host_action = Some(match host_action {
                ClientSessionHostAction::QuitToTitle => FlatClientHostAction::QuitToTitle,
                ClientSessionHostAction::Quit => FlatClientHostAction::Quit,
            });
        }
    }

    fn apply_client_experience_settings_effects(
        &mut self,
        effects: mclone_app_runtime::client_experience::ClientExperienceSettingsEffects,
        result: &mut FlatClientUiActionResult,
    ) -> bool {
        let mut ok = true;
        let has_setting_effects = !effects.setting_effects.is_empty();
        let has_unavailable =
            !effects.capability_projection.is_empty() || !effects.rejections.is_empty();
        for effect in effects.setting_effects {
            match effect {
                ClientExperienceSettingEffect::SetSectionOcclusionCulling(enabled) => {
                    self.render_options.section_occlusion_culling = enabled;
                    log::info!(
                        "section occlusion culling {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                }
                ClientExperienceSettingEffect::SetFullbright(enabled) => {
                    self.render_options.force_fullbright = enabled;
                    log::info!(
                        "fullbright {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                }
                ClientExperienceSettingEffect::SetFarLod {
                    enabled,
                    extra_radius_chunks,
                } => {
                    self.scene.far_lod = self
                        .scene
                        .far_lod
                        .with_extra_radius_chunks(extra_radius_chunks);
                    self.scene.far_lod.enabled = enabled;
                    log::info!(
                        "far LOD {}",
                        if self.scene.far_lod.enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                    log::info!(
                        "far LOD range set to {} chunks beyond render distance",
                        self.scene.far_lod.extra_radius_chunks
                    );
                }
                ClientExperienceSettingEffect::ClearFarLod => {
                    if let Some(runtime) = &mut self.runtime {
                        runtime.clear_far_lod();
                    }
                }
                ClientExperienceSettingEffect::SetPlayerCollisionBoxVisible(visible) => {
                    self.player_collision_box_visible = visible;
                    log::info!(
                        "player collision box debug {}",
                        if visible { "visible" } else { "hidden" }
                    );
                }
                ClientExperienceSettingEffect::SetFirstPersonPlayerVisible(visible) => {
                    self.camera.set_first_person_player_visible(visible);
                    self.scene.first_person_player_visible = visible;
                    log::info!(
                        "first-person player body {}",
                        if visible { "visible" } else { "hidden" }
                    );
                }
                ClientExperienceSettingEffect::SetCrosshairVisible(visible) => {
                    self.crosshair_visible = visible;
                    log::info!("crosshair {}", if visible { "visible" } else { "hidden" });
                }
                ClientExperienceSettingEffect::SetFramePipelineOverlayVisible(visible) => {
                    self.frame_pipeline_overlay_visible = visible;
                    log::info!(
                        "frame pipeline overlay {}",
                        if visible { "visible" } else { "hidden" }
                    );
                }
                ClientExperienceSettingEffect::SetPlayerModel(model) => {
                    self.player_model = model;
                    log::info!(
                        "player model set to {} ({})",
                        model.label(),
                        actor_figure_id_for_player_model(model).as_str()
                    );
                }
                ClientExperienceSettingEffect::SyncPlayerAppearance => {
                    if let Err(err) = self.sync_player_appearance() {
                        log::warn!("failed to sync player appearance to server: {err:#}");
                    }
                }
                ClientExperienceSettingEffect::SetMovementMode(movement_mode) => {
                    self.camera
                        .set_movement_mode(engine_movement_mode(movement_mode));
                    let movement_mode = self.camera.movement_mode();
                    log::info!("player movement mode {}", movement_mode.label());
                }
                ClientExperienceSettingEffect::SetXrTurnMode(_) => {}
                ClientExperienceSettingEffect::CycleFramePacing => {
                    result.host_action = Some(FlatClientHostAction::CycleFramePacing);
                }
                ClientExperienceSettingEffect::CycleFpsCap => {
                    result.host_action = Some(FlatClientHostAction::CycleFpsCap);
                }
                ClientExperienceSettingEffect::SetRenderDistance(render_distance_chunks) => {
                    let render_distance =
                        i32::try_from(render_distance_chunks).unwrap_or(MAX_RENDER_DISTANCE);
                    self.scene.render_distance = render_distance;
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
                                ok = false;
                            }
                        }
                    } else {
                        log::info!("new-world render distance set to {render_distance}");
                    }
                }
                ClientExperienceSettingEffect::SetFlySpeedMultiplier(multiplier) => {
                    self.camera.set_fly_speed_multiplier(f64::from(multiplier));
                    log::info!(
                        "fly speed set to {:.1}x ({:.0} blocks/s)",
                        self.camera.fly_speed_multiplier(),
                        self.camera.speed_blocks_per_second()
                    );
                }
                ClientExperienceSettingEffect::SetMovementSpeedMultiplier(multiplier) => {
                    self.camera
                        .set_movement_speed_multiplier(f64::from(multiplier));
                    let multiplier = self.camera.movement_speed_multiplier() as f32;
                    self.scene.movement_speed_multiplier = multiplier;
                    log::info!(
                        "movement speed multiplier set to {:.1}x",
                        self.camera.movement_speed_multiplier()
                    );
                }
                ClientExperienceSettingEffect::SetTouchLookSensitivity(_) => {}
                ClientExperienceSettingEffect::SetTouchControlsMode(mode) => {
                    result.host_action = Some(FlatClientHostAction::SetTouchControlsMode(mode));
                    log::info!("touch controls set to {}", touch_controls_mode_label(mode));
                }
                ClientExperienceSettingEffect::SetServerSimulationCadence(cadence) => {
                    let config = simulation_cadence_config_from_game(cadence);
                    if let Some(runtime) = &mut self.runtime {
                        match runtime.set_simulation_cadence(config) {
                            Ok(true) => {
                                self.scene.simulation_cadence = config;
                                log::info!("local server cadence set to {}", cadence.label());
                            }
                            Ok(false) => {
                                self.scene.simulation_cadence = config;
                            }
                            Err(err) => {
                                log::error!("failed to set local server cadence: {err:#}");
                                ok = false;
                            }
                        }
                    } else {
                        self.scene.simulation_cadence = config;
                        log::info!("local server cadence preset set to {}", cadence.label());
                    }
                }
            }
        }
        self.apply_client_experience_capability_projection(effects.capability_projection);
        for rejection in effects.rejections {
            log::error!("{}", rejection.message);
            self.status_overlay = StatusOverlay::new(rejection.message, false);
            ok = false;
        }
        if has_setting_effects && !has_unavailable {
            self.status_overlay = StatusOverlay::hidden();
        }
        ok
    }

    fn apply_client_experience_capability_projection(
        &mut self,
        projection: ClientExperienceCapabilityProjection,
    ) {
        if let Some(unavailable) = projection.first_unavailable() {
            if let Some(message) = unavailable.status.message() {
                log::warn!("{message}");
                self.status_overlay = StatusOverlay::new(message, unavailable.status.ok());
            }
        }
    }

    fn apply_client_session_ui_effects(&mut self, effects: ClientSessionUiEffects) {
        if let Some(seed) = effects.new_world_seed {
            self.ui.set_new_world_seed(seed);
        }
        if let Some(addr) = effects.join_remote_addr {
            self.ui.set_join_remote_addr(addr);
        }
        if let Some(screen) = effects.screen {
            self.ui.set_screen(Some(screen));
        }
    }

    pub(crate) fn apply_client_session_transition_effects(
        &mut self,
        effects: ClientSessionTransitionEffects,
    ) {
        if effects.clear_session {
            self.session.clear();
        }
        self.apply_client_session_ui_effects(effects.ui);
    }

    pub(crate) fn request_remote_session_start(
        &mut self,
        remote_addr: String,
        arm_mouse_lock: bool,
    ) {
        self.set_join_remote_addr(remote_addr.clone());
        let request = SessionStartRequest::JoinRemote {
            endpoint: RemoteSessionEndpoint::new(remote_addr.clone()),
        };
        let transition = client_session_start_transition(self.session.state());
        self.session.request_start(
            request,
            FlatClientPendingSessionStart {
                scene: self.remote_session_scene(remote_addr),
                arm_mouse_lock,
                show_title_on_failure: false,
                descriptor: None,
                transition,
            },
        );
        self.clear_ui_input();
    }

    pub(crate) fn clear_inactive_session_status(&mut self) {
        if client_session_should_clear_inactive_status(self.session.state()) {
            self.session.clear();
        }
        self.status_overlay = StatusOverlay::hidden();
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
        if matches!(
            &pending.request,
            SessionStartRequest::CreateLocalWorld { .. }
                | SessionStartRequest::OpenLocalWorld { .. }
        ) {
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

    pub(crate) fn pending_session_start_needs_teardown(&self) -> bool {
        self.session
            .pending_start()
            .is_some_and(|pending| pending.payload.transition.teardown_world)
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
                    | GameUiAction::SetXrTurnMode(_)
                    | GameUiAction::SetTouchLookSensitivity(_)
                    | GameUiAction::SetTouchControlsMode(_)
                    | GameUiAction::SetServerSimulationCadence(_)
            ),
            clear_gameplay_input: true,
        };

        if context.session_starting && !matches!(action, GameUiAction::Quit) {
            result.clear_gameplay_input = false;
            result.mouse_lock_requested = None;
            result.preserve_pointer_state = true;
            return result;
        }

        self.client_experience
            .set_settings_state(self.client_experience_settings_state());
        let new_world_seed = if matches!(action, GameUiAction::OpenWorldCreate) {
            self.next_new_world_seed()
        } else {
            self.ui.new_world_seed()
        };
        let next_new_world_seed = matches!(
            action,
            GameUiAction::OpenNewWorld | GameUiAction::RerollSeed
        )
        .then(|| self.next_new_world_seed());
        let effects = self.client_experience.apply_ui_action(
            action,
            ClientExperienceActionContext {
                new_world_seed,
                next_new_world_seed,
                current_join_remote_addr: self.ui.join_remote_addr(),
                fallback_remote_addr: context
                    .fallback_remote_addr
                    .or(Some(DEFAULT_JOIN_REMOTE_ADDR)),
            },
        );
        let apply_ui_action =
            client_experience_should_apply_ui_projection(action) && effects.projection.is_empty();
        if !self.apply_client_experience_effects(effects, context.from_pointer_click, &mut result) {
            return result;
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
        scene.world_dir = None;
        scene
    }

    fn catalog_world_scene(&self, summary: &LocalWorldSummary, world_dir: PathBuf) -> SceneOptions {
        let mut scene = self.local_world_scene(summary.seed);
        scene.world_dir = Some(world_dir);
        scene
    }

    fn remote_session_scene(&self, remote_addr: String) -> SceneOptions {
        let mut scene = self.scene.clone();
        scene.remote_addr = Some(remote_addr);
        scene.world_dir = None;
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
        self.sync_player_appearance()
            .context("failed to sync initial player appearance")?;
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
        let Some(descriptor) = pending
            .payload
            .descriptor
            .clone()
            .or_else(|| pending.request.active_descriptor())
        else {
            anyhow::bail!("unsupported local session start");
        };

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
            descriptor,
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
        let descriptor = startup.descriptor;
        self.runtime = Some(startup.pump.into_runtime());
        self.sync_player_appearance()
            .context("failed to sync initial player appearance")?;
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
        actor_figures: Option<&ActorFigureSet>,
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
            actor_figures,
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

    pub(crate) fn sync_player_appearance(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(set_player_appearance_command_for_ui_model(
                self.player_model,
            ))
            .map_err(Into::into)
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
            actor_figure_id_for_player_model(self.player_model),
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
        let timed_update = runtime.sync_render_sections_with_completed_result_acceptance_timed(
            camera_view.render_eye,
            None,
        )?;
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let loaded_chunk_count = runtime.client().loaded_chunk_count();
        Ok(Some(FlatClientSectionSync {
            section_update: timed_update.cache_update,
            sync_timing: timed_update.timing,
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
        sync_timing: Option<mclone_app_runtime::RenderSectionSyncTiming>,
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
        if let Some(sync_timing) = sync_timing {
            self.frame_pipeline_accounting
                .record_render_section_sync(sync_timing, remesh_ms);
        }
        self.frame_pipeline_accounting
            .record_upload_apply(upload_ms);
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
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.release_render_compile_jobs(sync.section_update.accepted_compile_result_count);
        }
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
            Some(sync.sync_timing),
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
            None,
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
        self.frame_pipeline_accounting
            .record_upload_apply(upload_ms);
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
            camera_view.render_pose(self.current_render_distance(fallback_render_distance));
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
        let mut hud = ui_frame.hud;
        let loading_progress_overlay = ui_frame.loading_progress_overlay;
        let gui_active = ui_active
            || debug_stats.is_some()
            || hud
                .as_ref()
                .is_some_and(mclone_ui::FlatHud::has_visible_commands)
            || loading_progress_overlay.is_some()
            || debug_view_readiness_overlay.is_some();
        let mut ui_render_state = game_ui_render_state(ui_frame.render_options);
        ui_render_state.world_catalog = self.world_catalog_ui_for_render();
        ui_render_state.block_palette = ui_frame.block_palette;
        let base_ui_draw = self.ui.render_draw_list(ui_render_state);
        let gui_state = FullFrameGui::new(
            gui_active,
            ui_covers_world || loading_progress_overlay.is_some(),
            [gui_scale.width, gui_scale.height],
        );

        let frame_inputs = self.prepare_frame_inputs(fallback_render_distance, ui_active);
        let mut world_debug_lines = engine_debug_world_lines(
            &self.camera,
            EngineDebugVisualOptions::new(self.player_collision_box_visible),
        );
        world_debug_lines.extend(self.desktop_blink_debug_lines());
        let far_lod_mesh = self.runtime.as_mut().and_then(|runtime| {
            runtime.prepare_far_lod_mesh(
                self.scene.far_lod,
                self.scene.seed,
                frame_inputs.camera_view.snapshot.chunk_pos,
                frame_inputs.camera_view.render_eye,
            )
        });
        let Some(render_resources) = &mut self.render_resources else {
            anyhow::bail!("flat client render resources are not initialized");
        };
        render_resources
            .draw_mut()
            .set_traversal_ready_sections(&frame_inputs.traversal_ready_sections);
        let mut render_stats = frame_inputs.render_stats;
        let mut flat_hud_retained_cache = UiDrawCacheStats::default();
        let ui = &mut self.ui;
        let mut summary = render_resources.render_full_frame_for_pose(
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
            far_lod_mesh,
            gui_state,
            |stats| {
                if let Some(mut debug_stats) = debug_stats {
                    debug_stats.render = *stats;
                    let debug = debug_stats.hud_debug_overlay();
                    if let Some(hud) = hud.as_mut() {
                        hud.debug = Some(debug);
                    } else {
                        hud = Some(FlatHud::debug_only(debug));
                    }
                }
                let mut ui_draw = base_ui_draw;
                if let Some(progress) = loading_progress_overlay.as_ref() {
                    ui.append_loading_progress_draw(
                        gui_scale,
                        &mut ui_draw,
                        progress,
                        LoadingProgressOverlayLayer::fullscreen(),
                    );
                }
                if let Some(hud) = hud.as_ref() {
                    flat_hud_retained_cache = ui.append_flat_hud_draw(gui_scale, &mut ui_draw, hud);
                }
                if let Some(progress) = debug_view_readiness_overlay.as_ref() {
                    ui.append_loading_progress_draw(
                        gui_scale,
                        &mut ui_draw,
                        progress,
                        LoadingProgressOverlayLayer::panel(
                            Point {
                                x: (gui_scale.width - 132.0).max(4.0),
                                y: 4.0,
                            },
                            "VIEW",
                        ),
                    );
                }
                ui_draw
            },
            &mut render_stats,
        )?;
        self.render_stats = render_stats;
        summary.flat_hud_retained_cache = flat_hud_retained_cache;
        Ok(summary)
    }

    pub(crate) fn sync_spectator_from_camera(&mut self) {
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
    }

    fn desktop_blink_debug_lines(&self) -> Vec<WorldGuiLine> {
        let Some(preview) = self.desktop_blink_debug.preview.as_ref() else {
            return Vec::new();
        };
        desktop_blink_debug_lines(preview)
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
        self.frame_pipeline_accounting = DesktopFramePipelineAccounting::default();
        self.underwater_effect.reset();
        if let Some(runtime) = &mut self.runtime {
            runtime.clear_far_lod();
        }
    }
}

fn desktop_client_experience_profile() -> ClientExperienceProfile {
    let mut settings = ClientExperienceSettingsProfile::all_supported();
    settings.xr_turn = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.touch_look = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.touch_controls = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    ClientExperienceProfile::new(settings)
}

pub(crate) fn game_ui_render_state(options: FlatClientUiRenderOptions) -> GameUiRenderState {
    GameUiRenderState {
        world_catalog: Default::default(),
        render_distance: options.render_distance,
        min_render_distance: MIN_RENDER_DISTANCE,
        max_render_distance: MAX_RENDER_DISTANCE,
        section_occlusion_culling: options.render_options.section_occlusion_culling,
        force_fullbright: options.render_options.force_fullbright,
        far_lod_enabled: options.far_lod_enabled,
        far_lod_range_chunks: options.far_lod_range_chunks,
        min_far_lod_range_chunks: MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
        max_far_lod_range_chunks: MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
        player_collision_box_visible: options.player_collision_box_visible,
        first_person_player_visible: options.first_person_player_visible,
        crosshair_visible: Some(options.crosshair_visible),
        player_model: options.player_model,
        movement_mode: options.movement_mode,
        xr_turn_mode: None,
        fly_speed_multiplier: options.fly_speed_multiplier,
        min_fly_speed_multiplier: ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER as f32,
        max_fly_speed_multiplier: ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER as f32,
        movement_speed_multiplier: options.movement_speed_multiplier,
        min_movement_speed_multiplier: ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER as f32,
        max_movement_speed_multiplier: ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER as f32,
        frame_pacing_mode: game_frame_pacing_mode(options.frame_pacing.mode),
        fps_cap: options.frame_pacing.fps_cap,
        server_cadence: options.server_cadence,
        frame_pipeline_overlay_visible: options.frame_pipeline_overlay_visible,
        touch_controls_mode: None,
        touch_settings: None,
        block_palette: Default::default(),
    }
}

pub(crate) const fn game_simulation_cadence_from_config(
    cadence: SimulationCadenceConfig,
) -> GameSimulationCadence {
    GameSimulationCadence::new(
        cadence.host_rate_hz,
        cadence.gameplay_rate_hz,
        cadence.physics_rate_hz,
    )
}

pub(crate) const fn simulation_cadence_config_from_game(
    cadence: GameSimulationCadence,
) -> SimulationCadenceConfig {
    SimulationCadenceConfig::new(
        cadence.host_rate_hz,
        cadence.gameplay_rate_hz,
        cadence.physics_rate_hz,
    )
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

pub(crate) const fn actor_figure_id_for_player_model(model: GamePlayerModel) -> ActorFigureId {
    match model {
        GamePlayerModel::Player => mclone_assets::DEFAULT_PLAYER_FIGURE_ID,
        GamePlayerModel::UprightBear => mclone_assets::UPRIGHT_BEAR_FIGURE_ID,
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

fn desktop_blink_debug_config() -> TeleportConfig {
    TeleportConfig {
        max_distance: DESKTOP_BLINK_DEBUG_MAX_DISTANCE,
        arc_height: DESKTOP_BLINK_DEBUG_ARC_HEIGHT,
        ..TeleportConfig::default()
    }
}

fn desktop_blink_debug_intent(camera: &EngineCameraController) -> TeleportIntent {
    let pose = camera.player().pose();
    let snapshot = camera.snapshot();
    let forward = horizontal_forward_from_yaw(snapshot.yaw_radians);
    let right = horizontal_right_from_yaw(snapshot.yaw_radians);
    let aim_origin = pose
        .eye_position()
        .add(right.scale(-DESKTOP_BLINK_DEBUG_ORIGIN_LEFT_OFFSET))
        .add(Vec3d::new(
            0.0,
            -DESKTOP_BLINK_DEBUG_ORIGIN_DOWN_OFFSET,
            0.0,
        ))
        .add(forward.scale(DESKTOP_BLINK_DEBUG_ORIGIN_FORWARD_OFFSET));
    let aim_direction = view_forward(
        snapshot.yaw_radians,
        (snapshot.pitch_radians + DESKTOP_BLINK_DEBUG_UPWARD_PITCH_BIAS_RADIANS)
            .clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2),
    );
    TeleportIntent::new(pose.position, aim_origin, aim_direction, pose.y_rot_degrees)
}

fn desktop_blink_debug_lines(preview: &TeleportPreview) -> Vec<WorldGuiLine> {
    let mut lines = Vec::new();
    let arc_color = if preview.is_valid() {
        DESKTOP_BLINK_DEBUG_VALID_ARC_COLOR
    } else {
        DESKTOP_BLINK_DEBUG_INVALID_ARC_COLOR
    };
    for points in preview.arc_points.windows(2) {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(points[0]),
            glam_vec3_from_vec3d(points[1]),
            arc_color,
        ));
    }
    if let Some(feet) = preview.target_feet {
        push_debug_cross(
            &mut lines,
            feet,
            DESKTOP_BLINK_DEBUG_MARKER_RADIUS,
            DESKTOP_BLINK_DEBUG_FEET_COLOR,
        );
    }
    if let (Some(feet), Some(dot)) = (preview.target_feet, preview.marker_dot) {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(feet),
            glam_vec3_from_vec3d(dot),
            DESKTOP_BLINK_DEBUG_DOT_COLOR,
        ));
        push_debug_cross(
            &mut lines,
            dot,
            DESKTOP_BLINK_DEBUG_DOT_RADIUS,
            DESKTOP_BLINK_DEBUG_DOT_COLOR,
        );
    }
    lines
}

fn push_debug_cross(lines: &mut Vec<WorldGuiLine>, center: Vec3d, radius: f64, color: [f32; 4]) {
    lines.push(WorldGuiLine::new(
        glam_vec3_from_vec3d(center.add(Vec3d::new(-radius, 0.0, 0.0))),
        glam_vec3_from_vec3d(center.add(Vec3d::new(radius, 0.0, 0.0))),
        color,
    ));
    lines.push(WorldGuiLine::new(
        glam_vec3_from_vec3d(center.add(Vec3d::new(0.0, 0.0, -radius))),
        glam_vec3_from_vec3d(center.add(Vec3d::new(0.0, 0.0, radius))),
        color,
    ));
}

fn horizontal_forward_from_yaw(yaw_radians: f64) -> Vec3d {
    Vec3d::new(yaw_radians.sin(), 0.0, yaw_radians.cos())
}

fn horizontal_right_from_yaw(yaw_radians: f64) -> Vec3d {
    Vec3d::new(yaw_radians.cos(), 0.0, -yaw_radians.sin())
}

fn view_forward(yaw_radians: f64, pitch_radians: f64) -> Vec3d {
    let yaw_sin = yaw_radians.sin();
    let yaw_cos = yaw_radians.cos();
    let pitch_sin = pitch_radians.sin();
    let pitch_cos = pitch_radians.cos();
    Vec3d::new(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos)
}

fn session_start_request_for_scene(scene: &SceneOptions) -> SessionStartRequest {
    scene.remote_addr.as_ref().map_or(
        SessionStartRequest::new_seed_local_world(scene.seed),
        |remote_addr| SessionStartRequest::JoinRemote {
            endpoint: RemoteSessionEndpoint::new(remote_addr.clone()),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_app_runtime::world_catalog::LocalWorldCreateOptions;
    use mclone_ui::WorldCatalogUiWorldId;
    use mclone_ui::{UiScreenId, UiSurface};

    fn ui_action_context() -> FlatClientUiActionContext<'static> {
        FlatClientUiActionContext {
            session_starting: false,
            from_pointer_click: true,
            fallback_remote_addr: Some("10.0.0.9:25565"),
        }
    }

    fn headless_ui_state(driver: &FlatClientDriver) -> GameUiRenderState {
        driver.current_ui_render_state(FramePacingUiState::default(), BlockPaletteOverlay::hidden())
    }

    fn debug_widget_rect(snapshot: &UiDebugSnapshot, label: &str) -> mclone_ui::Rect {
        snapshot
            .widgets
            .iter()
            .find(|widget| widget.label == label)
            .unwrap_or_else(|| panic!("missing debug widget {label:?}"))
            .rect
    }

    fn debug_hovered_label(snapshot: &UiDebugSnapshot) -> Option<&str> {
        let hovered = snapshot.hovered?;
        snapshot
            .widgets
            .iter()
            .find(|widget| widget.id == hovered)
            .map(|widget| widget.label.as_str())
    }

    fn scene_with_world_root(root: std::path::PathBuf) -> SceneOptions {
        SceneOptions {
            world_root: Some(root),
            ..SceneOptions::default()
        }
    }

    fn unique_temp_world_root(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mclone-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn desktop_blink_debug_intent_uses_visible_left_hand_offset_and_upward_aim() {
        let camera =
            EngineCameraController::from_eye_pose(Vec3d::new(8.0, 70.0, 8.0), 0.0, 0.0, 24.0);

        let intent = desktop_blink_debug_intent(&camera);

        assert_eq!(intent.start_feet, camera.player().pose().position);
        assert!(intent.aim_origin.x < camera.snapshot().eye.x);
        assert!(intent.aim_origin.y < camera.snapshot().eye.y);
        assert!(intent.aim_origin.z > camera.snapshot().eye.z);
        assert!(intent.aim_direction.y > 0.0);
        assert!(intent.aim_direction.z > 0.0);
    }

    fn row_probe_points(rect: mclone_ui::Rect) -> [Point; 5] {
        [
            Point {
                x: rect.x + 1.0,
                y: rect.y + 1.0,
            },
            Point {
                x: rect.x + 12.0,
                y: rect.y + rect.height * 0.5,
            },
            Point {
                x: rect.x + 64.0,
                y: rect.y + rect.height * 0.5,
            },
            Point {
                x: rect.right() - 1.0,
                y: rect.y + rect.height * 0.5,
            },
            Point {
                x: rect.right() - 1.0,
                y: rect.bottom() - 1.0,
            },
        ]
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

    fn driver_catalog_state(driver: &FlatClientDriver) -> WorldCatalogUiState {
        driver
            .current_ui_render_state(FramePacingUiState::default(), BlockPaletteOverlay::hidden())
            .world_catalog
    }

    fn selected_catalog_ui_id(driver: &FlatClientDriver) -> WorldCatalogUiWorldId {
        driver_catalog_state(driver)
            .selected
            .expect("test catalog should have a selected row")
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
                request: SessionStartRequest::new_seed_local_world(seed)
            }
        );
        let pending = driver.session.take_pending_start().unwrap();
        assert_eq!(
            pending.request,
            SessionStartRequest::new_seed_local_world(seed)
        );
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
    fn catalog_create_world_action_creates_world_and_queues_local_start() {
        let root = unique_temp_world_root("catalog-create");
        let scene = scene_with_world_root(root.clone());
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());
        driver.set_ui_screen(Some(GameScreen::WorldCreate));
        driver.set_new_world_seed(4242);

        let result = driver.apply_ui_action(GameUiAction::CreateCatalogWorld, ui_action_context());

        assert!(result.session_start_queued);
        assert_eq!(result.mouse_lock_requested, Some(false));
        assert_eq!(driver_catalog_state(&driver).entry_count(), 1);
        let worlds = NativeWorldCatalog::new(root.clone()).list_worlds().unwrap();
        assert_eq!(worlds.len(), 1);
        let summary = worlds[0].clone();
        let pending = driver.session.take_pending_start().unwrap();
        assert_eq!(
            pending.request,
            SessionStartRequest::create_local_world(
                LocalWorldCreateOptions::new("New World", 4242)
                    .unwrap()
                    .with_requested_id(summary.id.clone())
            )
        );
        assert_eq!(pending.payload.scene.seed, 4242);
        assert_eq!(pending.payload.scene.remote_addr, None);
        assert_eq!(
            pending.payload.scene.world_dir,
            Some(root.join(summary.id.as_str()))
        );
        assert_eq!(
            pending.payload.descriptor,
            Some(ActiveSessionDescriptor::from_local_world_summary(&summary))
        );
        assert!(root.join(summary.id.as_str()).join("world.json").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_open_world_action_resolves_row_and_queues_open_start() {
        let root = unique_temp_world_root("catalog-open");
        let catalog = NativeWorldCatalog::new(root.clone());
        let summary = catalog
            .create_world(LocalWorldCreateOptions::new("Alpha Base", 99).unwrap())
            .unwrap();
        let scene = scene_with_world_root(root.clone());
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());
        let ui_id = selected_catalog_ui_id(&driver);

        let result = driver.apply_ui_action(GameUiAction::OpenWorld(ui_id), ui_action_context());

        assert!(result.session_start_queued);
        assert_eq!(result.mouse_lock_requested, Some(false));
        let opened_summary = catalog
            .list_worlds()
            .unwrap()
            .into_iter()
            .find(|entry| entry.id == summary.id)
            .unwrap();
        let pending = driver.session.take_pending_start().unwrap();
        assert_eq!(
            pending.request,
            SessionStartRequest::open_local_world(summary.id.clone())
        );
        assert_eq!(pending.payload.scene.seed, 99);
        assert_eq!(
            pending.payload.scene.world_dir,
            Some(root.join(summary.id.as_str()))
        );
        assert_eq!(
            pending.payload.descriptor,
            Some(ActiveSessionDescriptor::from_local_world_summary(
                &opened_summary
            ))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_delete_world_action_removes_inactive_world() {
        let root = unique_temp_world_root("catalog-delete");
        let catalog = NativeWorldCatalog::new(root.clone());
        let summary = catalog
            .create_world(LocalWorldCreateOptions::new("Delete Me", 7).unwrap())
            .unwrap();
        let scene = scene_with_world_root(root.clone());
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());
        let ui_id = selected_catalog_ui_id(&driver);

        let result = driver.apply_ui_action(GameUiAction::DeleteWorld(ui_id), ui_action_context());

        assert!(!result.session_start_queued);
        assert_eq!(driver.ui_screen(), Some(GameScreen::WorldList));
        assert!(!catalog.world_dir(&summary.id).exists());
        let state = driver_catalog_state(&driver);
        assert!(state.status.visible);
        assert!(state.status.ok);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_delete_world_action_rejects_active_world() {
        let root = unique_temp_world_root("catalog-delete-active");
        let catalog = NativeWorldCatalog::new(root.clone());
        let summary = catalog
            .create_world(LocalWorldCreateOptions::new("Active World", 11).unwrap())
            .unwrap();
        let scene = scene_with_world_root(root.clone());
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());
        let ui_id = selected_catalog_ui_id(&driver);
        driver
            .session
            .complete_start(ActiveSessionDescriptor::from_local_world_summary(&summary));

        let result = driver.apply_ui_action(GameUiAction::DeleteWorld(ui_id), ui_action_context());
        let state = driver
            .current_ui_render_state(FramePacingUiState::default(), BlockPaletteOverlay::hidden());

        assert!(!result.session_start_queued);
        assert!(catalog.world_dir(&summary.id).exists());
        assert_eq!(state.world_catalog.active, Some(ui_id));
        assert!(state.world_catalog.status.visible);
        assert!(!state.world_catalog.status.ok);
        assert!(
            state
                .world_catalog
                .status
                .message
                .as_str()
                .contains("cannot delete active local world")
        );
        let _ = std::fs::remove_dir_all(root);
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
    fn ui_action_toggles_crosshair_visibility() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        assert!(driver.crosshair_visible);
        let result = driver.apply_ui_action(GameUiAction::ToggleCrosshair, ui_action_context());

        assert!(result.host_action.is_none());
        assert!(!driver.crosshair_visible);
    }

    #[test]
    fn ui_action_toggles_frame_pipeline_overlay_visibility() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        assert!(!driver.frame_pipeline_overlay_visible);
        let result = driver.apply_ui_action(
            GameUiAction::ToggleFramePipelineOverlay,
            ui_action_context(),
        );

        assert!(result.host_action.is_none());
        assert!(driver.frame_pipeline_overlay_visible);
        assert!(
            driver
                .current_ui_render_state(
                    FramePacingUiState::default(),
                    BlockPaletteOverlay::hidden(),
                )
                .frame_pipeline_overlay_visible
        );
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
    fn headless_ui_click_toggles_options_crosshair_from_full_row() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());
        driver.set_ui_screen(Some(GameScreen::Options {
            parent: mclone_ui::GameOptionsParent::Pause,
        }));
        driver.set_ui_scale(GuiScale::from_pixels(960, 540));
        let state = headless_ui_state(&driver);
        let snapshot = driver
            .ui_v2_debug_snapshot_for_state(state)
            .expect("Options is a v2 screen");
        let crosshair = debug_widget_rect(&snapshot, "Crosshair");

        for point in row_probe_points(crosshair) {
            let before = driver.crosshair_visible;
            let state = headless_ui_state(&driver);
            driver.commit_ui_render_state(state);
            let report = driver.apply_ui_pointer_click(point, ui_action_context());

            assert!(report.down_handled, "down missed at {point:?}");
            assert!(report.up_handled, "up missed at {point:?}");
            assert_eq!(report.action, Some(GameUiAction::ToggleCrosshair));
            assert!(
                report
                    .action_result
                    .is_some_and(|result| result.host_action.is_none())
            );
            assert_eq!(driver.crosshair_visible, !before);
            assert_eq!(
                report.after_down.and_then(|snapshot| snapshot.captured),
                report.after_up.and_then(|snapshot| snapshot.hovered),
                "captured widget should still be hovered on release at {point:?}"
            );
        }
    }

    #[test]
    fn headless_ui_click_toggles_options_first_person_body_from_full_row() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());
        driver.set_ui_screen(Some(GameScreen::Options {
            parent: mclone_ui::GameOptionsParent::Pause,
        }));
        driver.set_ui_scale(GuiScale::from_pixels(960, 540));
        let state = headless_ui_state(&driver);
        let snapshot = driver
            .ui_v2_debug_snapshot_for_state(state)
            .expect("Options is a v2 screen");
        let first_person = debug_widget_rect(&snapshot, "First Person Body");

        for point in row_probe_points(first_person) {
            let before = driver.camera.first_person_player_visible();
            let state = headless_ui_state(&driver);
            driver.commit_ui_render_state(state);
            let report = driver.apply_ui_pointer_click(point, ui_action_context());

            assert!(report.down_handled, "down missed at {point:?}");
            assert!(report.up_handled, "up missed at {point:?}");
            assert_eq!(report.action, Some(GameUiAction::ToggleFirstPersonPlayer));
            assert!(
                report
                    .action_result
                    .is_some_and(|result| result.host_action.is_none())
            );
            assert_eq!(driver.camera.first_person_player_visible(), !before);
            assert_eq!(driver.scene.first_person_player_visible, !before);
        }
    }

    #[test]
    fn committed_ui_state_controls_pointer_hit_testing() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());
        driver.set_ui_screen(Some(GameScreen::Options {
            parent: mclone_ui::GameOptionsParent::Pause,
        }));
        driver.set_ui_scale(GuiScale::from_pixels(960, 540));

        let committed_state = headless_ui_state(&driver);
        let committed_snapshot = driver
            .ui_v2_debug_snapshot_for_state(committed_state)
            .expect("Options is a v2 screen");
        let first_person = debug_widget_rect(&committed_snapshot, "First Person Body");
        let point = Point {
            x: first_person.x + 16.0,
            y: first_person.bottom() - 2.0,
        };

        let mut divergent_state = committed_state;
        divergent_state.touch_controls_mode = Some(TouchControlsMode::Auto);
        let mut divergent_surface = UiSurface::new();
        divergent_surface.set_screen(Some(UiScreenId::Options {
            parent: mclone_ui::GameOptionsParent::Pause,
        }));
        divergent_surface.set_scale(GuiScale::from_pixels(960, 540));
        let (_handled, _action) = divergent_surface.pointer_move(point, divergent_state);
        let divergent_snapshot = divergent_surface
            .debug_snapshot()
            .expect("divergent Options surface should be active");

        assert_eq!(debug_hovered_label(&divergent_snapshot), Some("Crosshair"));

        driver.commit_ui_render_state(committed_state);
        let report = driver.apply_ui_pointer_click(point, ui_action_context());

        assert_eq!(report.action, Some(GameUiAction::ToggleFirstPersonPlayer));
        assert!(driver.camera.first_person_player_visible());
    }

    #[test]
    fn ui_action_toggles_far_lod() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        assert!(!driver.scene.far_lod.enabled);
        let result = driver.apply_ui_action(GameUiAction::ToggleFarLod, ui_action_context());

        assert!(result.host_action.is_none());
        assert!(driver.scene.far_lod.enabled);

        let result = driver.apply_ui_action(GameUiAction::ToggleFarLod, ui_action_context());

        assert!(result.host_action.is_none());
        assert!(!driver.scene.far_lod.enabled);
    }

    #[test]
    fn ui_action_sets_far_lod_range() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        let result = driver.apply_ui_action(GameUiAction::SetFarLodRange(24), ui_action_context());

        assert!(result.host_action.is_none());
        assert_eq!(driver.scene.far_lod.extra_radius_chunks, 24);
    }

    #[test]
    fn ui_action_sets_player_model() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());

        assert_eq!(driver.player_model, GamePlayerModel::Player);
        let result = driver.apply_ui_action(
            GameUiAction::SetPlayerModel(GamePlayerModel::UprightBear),
            ui_action_context(),
        );

        assert!(result.host_action.is_none());
        assert_eq!(driver.player_model, GamePlayerModel::UprightBear);
        assert_eq!(
            actor_figure_id_for_player_model(driver.player_model),
            mclone_assets::upright_bear_figure_id()
        );
    }

    #[test]
    fn ui_action_sets_server_simulation_cadence_preset_without_runtime() {
        let scene = SceneOptions::default();
        let mut driver = FlatClientDriver::new(&scene, TexturedSectionRenderOptions::default());
        let cadence = GameSimulationCadence::new(60, 20, 120);

        let result = driver.apply_ui_action(
            GameUiAction::SetServerSimulationCadence(cadence),
            ui_action_context(),
        );

        assert!(result.host_action.is_none());
        assert!(result.preserve_pointer_state);
        assert_eq!(
            driver.scene.simulation_cadence,
            SimulationCadenceConfig::new(60, 20, 120)
        );
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
        mouse_delta_x: f64::from(frame.look_delta.x)
            + keyboard_turn_mouse_delta(frame.keyboard_turn, dt_seconds),
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
