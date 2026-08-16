#![forbid(unsafe_code)]

mod interactive_input;
mod player_movement;
mod pose_sync;

pub use interactive_input::{
    MonoInputDisposition, MonoInteractiveInputRouter, XrControllerInputDisposition,
    XrControllerInputRouter,
};
pub use mclone_server::PersistenceQueueMetrics;
pub use player_movement::{
    DEFAULT_PLAYER_MOVEMENT_MAX_CATCH_UP_STEPS, DEFAULT_PLAYER_MOVEMENT_RATE_HZ,
    PlayerMovementAdvance, PlayerMovementCadenceConfig, PlayerMovementCommand,
};

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use glam::{Quat, Vec2, Vec3};
use mclone_app_runtime::asset_pack_preferences::{AssetPackPreference, AssetPackPreferenceStorage};
use mclone_app_runtime::catalog_executor::WorldCatalogOperationService;
use mclone_app_runtime::client_catalog_policy::{ClientCatalogEffects, ClientCatalogRequest};
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::client_experience::xr_native_client_experience_profile;
use mclone_app_runtime::client_experience::{
    ClientExperienceActionContext, ClientExperienceController, ClientExperienceEffects,
    ClientExperienceGameplayEffect, ClientExperienceLocalDataEffect,
    ClientExperienceMovementSettingChange, ClientExperienceProfile,
    ClientExperienceProjectionEffect, ClientExperienceScenarioEffect,
    ClientExperienceSettingEffect, ClientExperienceSettingsEffects, ClientExperienceSettingsState,
    client_experience_should_apply_ui_projection, desktop_native_client_experience_profile,
};
use mclone_app_runtime::client_session_policy::client_session_failed_start_ui_effects;
use mclone_app_runtime::client_session_policy::{
    ClientSessionEffects, ClientSessionHostAction, ClientSessionStatusProjection,
    ClientSessionTransitionEffects, ClientSessionUiEffects,
    client_session_quit_to_title_transition, client_session_should_clear_inactive_status,
    client_session_status_projection,
};
use mclone_app_runtime::debug_overlay::DebugPaneStats;
use mclone_app_runtime::frame_pacing::{
    FramePacingDebugStats, FramePacingUiState, FrameTimingStats,
};
use mclone_app_runtime::frame_render::{
    FlatSurfacePresentation, FrameActorPreparation, FullFrameGui, FullFrameRenderSummary,
    FullFrameRenderTiming, PlacedActorFrame, PlacedTerrainFrame, PlacedTerrainPrepared,
    RenderStreamStats, TerrainBackdropRenderContext, TerrainBackdropRenderer,
    TerrainCompositionFrame, TerrainCompositionSource, TerrainTranslucentSubmission,
    render_full_frame_for_view_with_opaque_gate_timed,
    render_full_frame_for_view_with_placed_terrain_timed,
    render_full_frame_for_view_with_prepared_records_and_opaque_gate_in_slot,
    render_full_frame_for_view_with_prepared_stereo_draw_and_opaque_gate_in_slot,
    render_full_frame_for_view_with_prepared_stereo_draw_and_opaque_gate_timed_in_slot,
    render_full_frame_for_view_with_prepared_stereo_draw_and_placed_terrain_in_slot,
    render_full_frame_for_view_with_prepared_stereo_draw_and_placed_terrain_timed_in_slot,
    render_full_frame_for_view_with_prepared_stereo_draw_terrain_backdrop_and_opaque_gate_in_slot,
    render_full_frame_for_view_with_prepared_stereo_draw_terrain_backdrop_and_opaque_gate_timed_in_slot,
    render_full_frame_for_view_with_terrain_backdrop_and_opaque_gate_timed,
    render_view_with_underwater_effect,
};
use mclone_app_runtime::graphics_preferences::{
    ClientGraphicsPreferenceStorage, ClientGraphicsPreferences,
};
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::host_mode::RemoteDedicatedServerSession;
use mclone_app_runtime::host_mode::{SingleViewHostMode, SingleViewHostOptions};
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::monotonic::system_monotonic_clock;
use mclone_app_runtime::monotonic::{MonotonicClockHandle, MonotonicDeadline, MonotonicInstant};
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::native_service_assembly::{
    IntegratedWorldSessionStorage, LocalIntegratedSceneOptions, LocalIntegratedStartupPump,
    LocalIntegratedStartupStep, MAX_STARTUP_RECONCILE_PASSES, NativeSceneServices,
    NativeSessionServices, NativeSessionStartupCompletion, NativeSessionStartupPump,
    native_world_catalog_operations,
};
use mclone_app_runtime::platform_operation::{PlatformOperation, PlatformOperationLedger};
use mclone_app_runtime::prepared_assets::{AssetPackSourceRegistry, PreparedSceneAssets};
use mclone_app_runtime::render_asset_data::TexturedMeshAssets;
use mclone_app_runtime::scene_session_runtime::SceneSessionRuntime;
use mclone_app_runtime::seed_reroll::NewWorldSeedReroll;
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::session::RemoteSessionEndpoint;
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionCoordinator, GameSessionState, SessionFailure,
    SessionRuntimeKind, SessionStartPayload, SessionStartRequest, plan_session_start,
};
#[cfg(not(target_arch = "wasm32"))]
use mclone_app_runtime::world_catalog::LocalWorldCreateOptions;
use mclone_app_runtime::world_catalog::{
    LocalWorldId, LocalWorldSummary, WorldCatalogCapabilities, WorldCatalogError,
};
use mclone_app_runtime::{
    EngineCameraCommitContext, EngineCameraCommitTiming, GameplayCommandTiming,
    GameplayCommandUpdatePolicy, RuntimePollDiagnostics, RuntimeUpdatePumpBudget,
    SingleViewRuntimeStats, TargetRenderWorkStats, TraversalReadySectionCache,
    debug_block_palette_overlay, debug_hotbar_icons, elapsed_ms, micros_to_ms,
    set_player_appearance_command_for_ui_model,
};
#[cfg(not(target_arch = "wasm32"))]
use mclone_assets::AssetSource;
use mclone_assets::{ActorFigureId, AssetPackCatalog, AssetPackSelection, BlockStateRegistry};
#[cfg(not(target_arch = "wasm32"))]
use mclone_audio::PreparedAudioAssets;
use mclone_audio::{
    AcousticMaterial, AudioOutputCapability, BEE_BUZZ, DEER_ALARM, DEER_CONTACT, DEER_IMPACT,
    MALLARD_CALL, PlaybackParams, RABBIT_DIG, RABBIT_RUSTLE, RABBIT_THUMP, SoundKey, UI_BACK,
    UI_CONFIRM, UI_ERROR, UI_OPEN, UI_SELECT, WOOD_CREAK, landing_playback_for_impact,
};
use mclone_client::block_facts::terrain_id;
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, ActorPresentation, BlockInteractionTarget,
    ClientInteractionController, ClientRuntime, EntityInteractionTarget,
    HAND_PUSH_DEFAULT_HEAD_RADIUS, TeleportCollisionSnapshot, TeleportConfig, TeleportIntent,
    TeleportPreview, TeleportPreviewCapability, TeleportPreviewRequestId, TeleportPreviewResult,
    TeleportValidityReason, sphere_intersects_solid_blocks, view_vector_from_rot_degrees,
};
use mclone_core::{
    AIR_BLOCK_STATE_ID, Aabb, AxisTopology, BlockPos, BlockStateId, CHUNK_WIDTH, ChunkPos,
    HorizontalTopology, Vec3d, time,
};
use mclone_diagnostics::{
    BudgetDecisionPanelReport, BudgetHostMode, FrameHostKind, FramePipelineReport, WorkWindow,
};
use mclone_input::{
    ControllerLayoutFamily, FLAT_HOTBAR_SLOT_COUNT, FlatInputAction, FlatInputFrame,
    InputPromptKind, MouseWheelDirection, PlayerAction, PlayerActionFrame, ResolvedFlatInput,
    TouchControlsMode, TouchLookDelta, TrackedControllerState, XrHand, XrInputFrame,
    keyboard_turn_mouse_delta,
};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh, quad_face_count_from_indices};
use mclone_protocol::{
    DebugActorKind, DebugHotbarItem, EntitySnapshot, ItemKind, ItemStackSnapshot,
    RemotePlayerUpdate,
};
#[cfg(not(target_arch = "wasm32"))]
use mclone_render::actor_assets::ActorTextureAssets;
use mclone_render::actor_assets::ActorTextureImage;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkMultiviewDepthTarget, ChunkMultiviewRenderTarget,
    ChunkProjectionKind, ChunkRenderTarget, ChunkRenderView, PreparedTexturedSectionStereoDraw,
    TexturedSectionDrawResources, TexturedSectionRecordCacheStats,
    TexturedSectionRecordPrepareStats, TexturedSectionRenderOptions, TexturedSectionRenderPhase,
    TexturedSectionRenderStats, TexturedSectionTranslucentRecord, TexturedSectionUploadReport,
    TexturedSectionUploadTiming,
};
use mclone_render::entity::{
    ActorDrawResources, ActorFigureSet, ActorInstance, ActorInstanceId, ActorRenderStats,
};
use mclone_render::fog::{RenderFog, RenderFogMode};
use mclone_render::gui::{
    GuiRenderOptions, GuiRenderer, WorldGuiLine, WorldGuiPanel, WorldGuiPanelRenderStats,
    WorldGuiRenderer,
};
use mclone_render::opaque_world_gate::OpaqueWorldGateRenderer;
#[cfg(not(target_arch = "wasm32"))]
use mclone_render::screen_effect::load_screen_effect_texture_assets;
use mclone_render::screen_effect::{
    ScreenEffectTextureAssets, ScreenEffectsRenderer, ScreenFadeOverlay, UnderwaterEffectState,
    UnderwaterOverlay,
};
use mclone_render::selection_outline::{SelectionOutline, SelectionOutlineRenderer};
use mclone_render::sky::SkyRenderState;
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_render::uniform::{
    LEFT_EYE_VIEW_SLOT, MAX_PRESENTATION_VIEW_COUNT, PER_VIEW_UNIFORM_FRAME_COUNT, PerViewSlot,
    PresentationViewIndex, RIGHT_EYE_VIEW_SLOT, SINGLE_VIEW_SLOT,
};
use mclone_render::world_color_mesh::WorldColorMeshRenderer;
use mclone_render::{
    GrassInteractor, GrassInteractorIdentity, GrassInteractorSet, GrassQuality,
    SeasonalAppearanceRenderState,
};
use mclone_render_session::{
    ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MOUSE_SENSITIVITY, EngineCameraCollisionMode, EngineCameraController,
    EngineCameraFrameState, EngineCameraInput, EngineCameraMovementImpulse,
    EngineCameraMovementMode, EngineCameraSnapshot, EngineCameraViewMode, EngineDebugVisualOptions,
    EngineHandPushInput, EngineRoomScaleReconciliation, EngineThrusterHand, EngineThrusterInput,
    FlatSurfaceLayout, PixelExtent, RenderSectionCacheUpdate, RenderSectionUploadCoordinator,
    RenderSectionUploadFramePolicy, RenderSectionUploadPhaseReport, SafeAreaInsets, XrFov,
    XrRenderView, XrView, XrViewPose, actor_instances_from_presentations,
    actor_instances_from_presentations_near_observer, engine_debug_world_lines,
    local_player_actor_instance_for_view, render_pose_from_snapshot_with_view_mode,
    render_view_from_world_pose,
};
use mclone_season::{
    EvaluatedLocalSeason, LatitudeSource, LocalSeasonInput, MCLONE_AXIAL_TILT_DEGREES,
    SeasonPreviewSettings, SolarCoordinatePolicy, SolarFrameDiagnostics, SolarInput, SolarSample,
    SolarTimeSource, solar_time_fraction_from_day_time,
};
use mclone_server::{SimulationCadenceConfig, WorkerFrameMetrics};
use mclone_ui::{
    Color, DEFAULT_JOIN_REMOTE_ADDR, DebugActorTool, DebugOverlay, FlatHotbarOverlay, FlatHud,
    FlatHudDebugOverlay, Font, GameAuxiliarySplitMode, GameCollisionMode, GameDeathCause,
    GameFlatPresentationState, GameFogSettings, GameFramePacingMode, GameGrassDetail,
    GameLeafDetail, GameLocalPlayControllerFamily, GameLocalPlayGuestInput, GameLocalPlayState,
    GameMovementMode, GamePlayerModel, GameScreen, GameSimulationCadence, GameTerrainPresentation,
    GameTouchSettings, GameTravelAssistMode, GameTurnMode, GameUiAction, GameUiHost,
    GameUiRenderState, GameWorldRenderScaleMode, GameXrRenderMode, GameXrRenderPathState,
    GameXrRenderTransitionState, GameXrTurnMode, GamepadHudOverlay, GuiDrawList, GuiKey, GuiScale,
    LoadingProgressOverlay, Point, Rect, StatusOverlay, StorageProfileBackend,
    StorageProfileUiState, TouchOverlay, UiDebugSnapshot, UiDrawCacheStats, UiPanelRevision,
    WheatTargetHud, WorldCatalogUiStatus, render_loading_progress_overlay, render_status_overlay,
};

mod asset_replacement;
mod comfort;
mod diagnostic_panel;
mod frame_pipeline_reporter;
mod host_effects;
mod locomotion;
mod mono;
mod options;
mod render_admission;
mod session;
mod teleport;
mod terrain_view;
mod timing;
mod tracking;
mod ui_panels;
mod warm_world;
mod worldgen_lens;

pub(crate) const fn engine_leaf_detail(detail: GameLeafDetail) -> mclone_mesh::LeafDetail {
    match detail {
        GameLeafDetail::Blocky => mclone_mesh::LeafDetail::Blocky,
        GameLeafDetail::Bushy => mclone_mesh::LeafDetail::Bushy,
    }
}

pub(crate) const fn game_leaf_detail(detail: mclone_mesh::LeafDetail) -> GameLeafDetail {
    match detail {
        mclone_mesh::LeafDetail::Blocky => GameLeafDetail::Blocky,
        mclone_mesh::LeafDetail::Bushy => GameLeafDetail::Bushy,
    }
}

pub(crate) const fn engine_grass_detail(detail: GameGrassDetail) -> GrassQuality {
    match detail {
        GameGrassDetail::Off => GrassQuality::Off,
        GameGrassDetail::Sparse => GrassQuality::Sparse,
        GameGrassDetail::Lush => GrassQuality::Lush,
        GameGrassDetail::Ultra => GrassQuality::Ultra,
    }
}

pub(crate) const fn game_grass_detail(detail: GrassQuality) -> GameGrassDetail {
    match detail {
        GrassQuality::Off => GameGrassDetail::Off,
        GrassQuality::Sparse => GameGrassDetail::Sparse,
        GrassQuality::Lush => GameGrassDetail::Lush,
        GrassQuality::Ultra => GameGrassDetail::Ultra,
    }
}

pub(crate) const fn engine_terrain_presentation(
    presentation: GameTerrainPresentation,
) -> mclone_app_runtime::startup_args::TerrainPresentationMode {
    match presentation {
        GameTerrainPresentation::ExactOnly => {
            mclone_app_runtime::startup_args::TerrainPresentationMode::ExactOnly
        }
        GameTerrainPresentation::Composed => {
            mclone_app_runtime::startup_args::TerrainPresentationMode::Composed
        }
    }
}

pub(crate) const fn game_terrain_presentation(
    mode: mclone_app_runtime::startup_args::TerrainPresentationMode,
) -> GameTerrainPresentation {
    match mode {
        mclone_app_runtime::startup_args::TerrainPresentationMode::ExactOnly => {
            GameTerrainPresentation::ExactOnly
        }
        mclone_app_runtime::startup_args::TerrainPresentationMode::Composed => {
            GameTerrainPresentation::Composed
        }
    }
}

pub use comfort::*;
pub use host_effects::*;
pub use locomotion::*;
pub use mclone_terrain_view::TerrainHorizonDiagnostic;
pub use mono::*;
pub use options::*;
pub use render_admission::*;
pub use session::*;
pub(crate) use teleport::*;
pub use terrain_view::SceneTerrainViewDiagnostics;
pub use timing::*;
pub use tracking::*;
pub use ui_panels::*;
pub use warm_world::*;
pub use worldgen_lens::WorldgenLensLayer;

use diagnostic_panel::XrDiagnosticPanel;
pub use frame_pipeline_reporter::{
    XrFramePipelineHostTiming, record_mono_frame_pipeline, record_xr_frame_pipeline,
    record_xr_frame_pipeline_with_peer_threads, xr_frame_pipeline_accounting_config,
    xr_frame_pipeline_observation, xr_frame_pipeline_peer_threads, xr_frame_pipeline_queue_depths,
    xr_frame_pipeline_stage_spans,
};

pub const MAX_XR_RENDER_DISTANCE: u32 = 16;
pub const XR_NEAR: f32 = 0.05;
pub const XR_FAR: f32 = 700.0;
pub const XR_JOYPAD_DEAD_ZONE: f32 = 0.18;
pub const XR_DEFAULT_SNAP_TURN_DEGREES: f32 = 15.0;
pub const XR_SNAP_TURN_ENGAGE_THRESHOLD: f32 = 0.65;
pub const XR_SNAP_TURN_RECENTER_THRESHOLD: f32 = 0.25;
pub const XR_LOCOMOTION_MAX_FRAME_SECONDS: f64 = 0.1;
pub const XR_BLINK_TELEPORT_STICK_THRESHOLD: f32 = 0.75;
pub const XR_BLINK_TELEPORT_HEADING_STICK_THRESHOLD: f32 = 0.9;
pub const XR_AUTOMATED_ORBIT_RADIUS_BLOCKS: f64 = 16.0;
pub const XR_UI_FPS_CAP: u32 = 90;
pub const XR_MENU_PANEL_PIXELS: [u32; 2] = [1024, 576];
pub const XR_MENU_PANEL_DISTANCE_BLOCKS: f32 = 2.2;
pub const XR_MENU_PANEL_WIDTH_BLOCKS: f32 = 1.75;
pub const XR_DIAGNOSTIC_PANEL_PIXELS: [u32; 2] = [336, 192];
pub const XR_DIAGNOSTIC_PANEL_DISTANCE_BLOCKS: f32 = 2.25;
pub const XR_DIAGNOSTIC_PANEL_WIDTH_BLOCKS: f32 = 1.2;
pub const XR_DIAGNOSTIC_PANEL_RIGHT_OFFSET_BLOCKS: f32 = 0.0;
pub const XR_DIAGNOSTIC_PANEL_UP_OFFSET_BLOCKS: f32 = -0.46;
pub const XR_FIELD_GUIDE_PANEL_PIXELS: [u32; 2] = [504, 108];
pub const XR_FIELD_GUIDE_PANEL_WIDTH_BLOCKS: f32 = 1.6;
pub const XR_FIELD_GUIDE_PANEL_UP_OFFSET_BLOCKS: f32 = -0.62;
pub const XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS: f32 = 6.0;
pub const XR_MENU_POINTER_TRIGGER_PRESS: f32 = 0.55;
pub const XR_MENU_POINTER_TRIGGER_RELEASE: f32 = 0.35;
pub const XR_GAMEPLAY_INTERACTION_HAND: XrHand = XrHand::Right;
pub const XR_GAME_UI_PANEL_HAND: XrHand = XrHand::Left;
pub const XR_GAME_UI_PANEL_WIDTH_BLOCKS: f32 = 1.35;
pub const XR_GAME_UI_PANEL_UP_OFFSET_BLOCKS: f32 = 0.18;
pub const XR_GAME_UI_PANEL_FORWARD_OFFSET_BLOCKS: f32 = 0.12;
const XR_FIXED_RENDER_EYE_SEPARATION_BLOCKS: f32 = 0.064;
const XR_MENU_LEFT_RAY_COLOR: [f32; 4] = [0.18, 0.85, 1.0, 0.95];
const XR_MENU_RIGHT_RAY_COLOR: [f32; 4] = [0.2, 1.0, 0.45, 0.95];
const XR_MENU_TRIGGER_RAY_COLOR: [f32; 4] = [1.0, 0.42, 0.12, 1.0];
const XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS: f64 = 0.15;
const XR_HEAD_COMFORT_RESIDUAL_FULL_BLOCKS: f64 = 0.6;
const XR_HEAD_COMFORT_HEAD_PENETRATION_ALPHA: f32 = 0.65;
const XR_HEAD_COMFORT_MAX_ALPHA: f32 = 0.9;
const XR_HEAD_COMFORT_FADE_IN_SECONDS: f32 = 0.12;
const XR_HEAD_COMFORT_FADE_OUT_SECONDS: f32 = 0.22;
const XR_BLINK_TELEPORT_MAX_DISTANCE: f64 = 8.0;
const XR_BLINK_TELEPORT_ARC_HEIGHT: f64 = 1.25;
const XR_BLINK_TELEPORT_GROUND_PROBE_DISTANCE: f64 = 0.01;
const XR_BLINK_TELEPORT_VALID_ARC_COLOR: [f32; 4] = [0.1, 0.85, 1.0, 1.0];
const XR_BLINK_TELEPORT_INVALID_ARC_COLOR: [f32; 4] = [1.0, 0.25, 0.15, 1.0];
const XR_BLINK_TELEPORT_FEET_COLOR: [f32; 4] = [0.1, 1.0, 0.35, 1.0];
const XR_BLINK_TELEPORT_DOT_COLOR: [f32; 4] = [1.0, 0.95, 0.2, 1.0];
const XR_BLINK_TELEPORT_HEADING_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const XR_BLINK_TELEPORT_MARKER_RADIUS: f64 = 0.28;
const XR_BLINK_TELEPORT_DOT_RADIUS: f64 = 0.1;
const XR_BLINK_TELEPORT_HEADING_ARROW_LENGTH: f64 = 0.7;
const XR_BLINK_TELEPORT_HEADING_ARROW_HEAD_LENGTH: f64 = 0.22;
const XR_BLINK_TELEPORT_HEADING_ARROW_HEAD_ANGLE_DEGREES: f64 = 35.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XrTerrainRuntimeUpdateMode {
    Live,
    Frozen,
}

#[derive(Clone, Copy)]
pub struct XrTerrainEyeTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth: &'a ChunkDepthTarget,
    pub size: [u32; 2],
}

#[derive(Clone, Copy)]
pub struct XrTerrainMultiviewTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth: &'a ChunkMultiviewDepthTarget,
    pub size: [u32; 2],
}

#[derive(Clone, Copy)]
pub enum XrSceneFrameTarget<'a> {
    PerEye {
        left: XrTerrainEyeTarget<'a>,
        right: XrTerrainEyeTarget<'a>,
    },
    Multiview(XrTerrainMultiviewTarget<'a>),
}

#[derive(Clone, Copy, Debug)]
pub struct XrTerrainMultiviewFrameSummary {
    pub rendered_frames: u32,
    pub section_count: usize,
    pub left: TexturedSectionRenderStats,
    pub right: TexturedSectionRenderStats,
    pub actor_count: usize,
    pub drawn_actor_count: usize,
    pub placed_actor_count: usize,
    pub drawn_placed_actor_count: usize,
    pub ui_panel: WorldGuiPanelRenderStats,
    pub ui_draw_cache: UiDrawCacheStats,
    pub upload: XrTerrainUploadSummary,
}

#[derive(Clone, Copy, Debug)]
pub struct XrTerrainStereoFrameSummary {
    pub rendered_frames: u32,
    pub section_count: usize,
    pub left: TexturedSectionRenderStats,
    pub right: TexturedSectionRenderStats,
}

struct XrRenderedEye {
    summary: FullFrameRenderSummary,
    timing: XrTerrainEyeRenderTiming,
    ui_panel: WorldGuiPanelRenderStats,
    ui_draw_cache: UiDrawCacheStats,
    submission: Option<wgpu::SubmissionIndex>,
}

#[derive(Clone, Copy, Debug, Default)]
struct XrWorldOverlayStats {
    panel: WorldGuiPanelRenderStats,
    draw_cache: UiDrawCacheStats,
}

struct SceneHostServices {
    clock: MonotonicClockHandle,
    catalog_operations: Option<WorldCatalogOperationService>,
    teleport_preview: TeleportPreviewCapability,
    audio: AudioOutputCapability,
}

/// Host policy supplied to one slot's poll/compile/upload preparation pass.
///
/// Keeping these inputs together makes the active and standby paths share one
/// implementation without teaching the single-world leaf about presentation
/// ownership or reaching back through `McloneSceneHost` for mutable globals.
#[derive(Clone)]
struct WorldPreparationPolicy {
    clock: MonotonicClockHandle,
    target_period_ms: Option<f64>,
    poll_budget: RuntimeUpdatePumpBudget,
    upload_budget: Option<usize>,
    accept_budget: Option<usize>,
    completed_result_accept_budget: Option<usize>,
    max_compile_requests: Option<usize>,
    work_elapsed_budget: Option<Duration>,
    defer_sync_after_pre_drain: bool,
}

/// One complete scene-owned world, including startup/lifecycle state and all
/// mutable render state derived from its runtime.
///
/// Tactical 174 Slice 2 deliberately retains exactly one of these. Keeping the
/// leaf concrete and directly addressed preserves the ordinary one-world frame
/// path while allowing one explicitly requested detached standby beside it.
struct LocalParticipantPresentation {
    camera: EngineCameraController,
    movement: player_movement::PlayerMovementState,
    interaction: ClientInteractionController,
    player_model: GamePlayerModel,
    field_guide_notification: FieldGuideNotificationState,
}

impl LocalParticipantPresentation {
    fn new(camera: EngineCameraController, cadence: PlayerMovementCadenceConfig) -> Self {
        let snapshot = camera.snapshot();
        Self {
            camera,
            movement: player_movement::PlayerMovementState::new(cadence, snapshot),
            interaction: ClientInteractionController::new(),
            player_model: GamePlayerModel::default(),
            field_guide_notification: FieldGuideNotificationState::default(),
        }
    }

    fn advance_movement(
        &mut self,
        client: &ClientRuntime,
        mut input: EngineCameraInput,
        elapsed_seconds: f64,
        frame_end: MonotonicInstant,
    ) -> (bool, PlayerMovementAdvance) {
        let before = self.camera.snapshot();
        let look_rate_mouse_delta_per_second =
            if elapsed_seconds.is_finite() && elapsed_seconds > 0.0 {
                [
                    input.mouse_delta_x / elapsed_seconds,
                    input.mouse_delta_y / elapsed_seconds,
                ]
            } else {
                [0.0; 2]
            };
        self.camera
            .turn_mouse_delta(input.mouse_delta_x, input.mouse_delta_y);
        input.mouse_delta_x = 0.0;
        input.mouse_delta_y = 0.0;

        self.movement.synchronize_eye(before.eye);
        self.movement
            .observe_input_at(input, look_rate_mouse_delta_per_second, frame_end);
        let advance = self.movement.advance_elapsed(frame_end, elapsed_seconds);
        for _ in 0..advance.steps {
            let command = self
                .movement
                .next_step_command()
                .expect("movement advance reported a missing command");
            let after = self.camera.apply_movement_input(client, command.input);
            self.movement.record_step(after.eye);
        }
        if advance.dropped_steps > 0 {
            log::warn!(
                "dropped {} local-player movement steps after a long frame",
                advance.dropped_steps
            );
        }
        (self.camera.snapshot() != before, advance)
    }

    fn observe_movement(
        &mut self,
        input: EngineCameraInput,
        look_rate_mouse_delta_per_second: [f64; 2],
        observed_at: MonotonicInstant,
    ) {
        self.movement
            .observe_input_at(input, look_rate_mouse_delta_per_second, observed_at);
    }

    fn observe_movement_without_heading(
        &mut self,
        input: EngineCameraInput,
        look_rate_mouse_delta_per_second: [f64; 2],
        observed_at: MonotonicInstant,
    ) {
        self.movement.observe_input_without_heading_at(
            input,
            look_rate_mouse_delta_per_second,
            observed_at,
        );
    }

    fn observe_movement_heading(&mut self, observed_at: MonotonicInstant) {
        let camera = self.camera.snapshot();
        self.movement
            .observe_heading_at(camera.yaw_radians, camera.pitch_radians, observed_at);
    }

    fn reset_movement(&mut self) {
        self.camera.clear_keys();
        self.movement.reset(self.camera.snapshot());
    }

    fn presentation_camera_snapshot(&self) -> EngineCameraSnapshot {
        let authoritative = self.camera.snapshot();
        EngineCameraSnapshot::from_eye_pose(
            self.movement.presentation_eye(authoritative.eye),
            authoritative.yaw_radians,
            authoritative.pitch_radians,
            authoritative.speed_blocks_per_second,
        )
    }
}

struct DrawableWorldSlot {
    id: WorldInstanceId,
    descriptor: Option<ActiveSessionDescriptor>,
    storage: WorldSlotStorage,
    lifecycle: WorldSlotLifecycle,
    /// Content generation shared by this slot's CPU, compiler, and GPU
    /// resources. Platform-operation completion is keyed separately.
    asset_epoch: u64,
    scene: McloneSceneHostOptions,
    runtime: Option<SceneSessionRuntime>,
    local_startup: Option<SceneLocalStartup>,
    /// A connected runtime may still be waiting for authoritative spawn/view
    /// admission. Keep gameplay frozen until that slot-local evidence is ready.
    external_runtime_startup_pending: bool,
    /// Cardinality-one compatibility envelope for participant-private scene
    /// state. The temporary `Deref` below preserves existing call sites while
    /// making ownership explicit before the bounded participant group admits
    /// more than one presentation state.
    local_participant: LocalParticipantPresentation,
    footsteps: FootstepCadence,
    pending_interaction_sounds: VecDeque<PendingInteractionSound>,
    interaction_sound_sequence: u64,
    mallard_tracks: VecDeque<ActiveMallardTrack>,
    /// First live consumer of the bounded local-participant foundation.
    ///
    /// This tactical keeps Guest 2 presentation-only until the live session
    /// boundary can expose a second ordinary client connection. The accepted
    /// durable cardinality remains 1-4; this optional preview is not the
    /// participant collection itself.
    local_guest_preview: Option<LocalParticipantPresentation>,
    draw: TexturedSectionDrawResources,
    actors: Option<ActorDrawResources>,
    actor_interpolation: ActorInterpolationState,
    last_actor_presentation_update: Option<MonotonicInstant>,
    traversal_ready_sections: TraversalReadySectionCache,
    section_uploads: RenderSectionUploadCoordinator,
    render_stats: RenderStreamStats,
    render_admission_policy: RenderAdmissionPolicy,
    accepted_entry_pose: Option<WorldEntryPose>,
    pending_startup_sections: Vec<TexturedRenderSectionMesh>,
}

const FOOTSTEP_STRIDE_BLOCKS: f64 = 1.65;
const MAX_FOOTSTEP_FRAME_DISTANCE: f64 = 1.0;
const MAX_PENDING_INTERACTION_SOUNDS: usize = 16;
const INTERACTION_SOUND_TIMEOUT: Duration = Duration::from_secs(2);
const MALLARD_TRACK_LIFETIME: Duration = Duration::from_secs(12);
const MAX_ACTIVE_MALLARD_TRACKS: usize = 48;

#[derive(Clone, Copy, Debug)]
struct ActiveMallardTrack {
    cue: mclone_protocol::MallardTrackCue,
    expires_at: MonotonicInstant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FootstepEvent {
    position: Vec3d,
    sequence: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FootstepCadence {
    distance_since_step: f64,
    sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalInteractionSoundIntent {
    Break,
    Use,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PendingInteractionSoundKind {
    Break {
        pos: BlockPos,
        before: BlockStateId,
    },
    Place {
        candidates: [(BlockPos, Option<BlockStateId>); 2],
    },
    Toggle {
        pos: BlockPos,
        before: BlockStateId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingInteractionSound {
    kind: PendingInteractionSoundKind,
    material: AcousticMaterial,
    submitted_at: MonotonicInstant,
    sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingInteractionResolution {
    Pending,
    Rejected,
    Confirmed { pos: BlockPos, sound: SoundKey },
}

impl PendingInteractionSound {
    fn resolve(
        self,
        now: MonotonicInstant,
        mut block_state_at: impl FnMut(BlockPos) -> Option<BlockStateId>,
    ) -> PendingInteractionResolution {
        if now.saturating_duration_since(self.submitted_at) > INTERACTION_SOUND_TIMEOUT {
            return PendingInteractionResolution::Rejected;
        }
        match self.kind {
            PendingInteractionSoundKind::Break { pos, before } => match block_state_at(pos) {
                Some(current) if current == AIR_BLOCK_STATE_ID && before != current => {
                    PendingInteractionResolution::Confirmed {
                        pos,
                        sound: self.material.break_sound(),
                    }
                }
                Some(current) if current != before => PendingInteractionResolution::Rejected,
                _ => PendingInteractionResolution::Pending,
            },
            PendingInteractionSoundKind::Place { candidates } => {
                for (pos, before) in candidates {
                    if let (Some(before), Some(current)) = (before, block_state_at(pos))
                        && current != before
                        && current != AIR_BLOCK_STATE_ID
                    {
                        return PendingInteractionResolution::Confirmed {
                            pos,
                            sound: self.material.place_sound(),
                        };
                    }
                }
                PendingInteractionResolution::Pending
            }
            PendingInteractionSoundKind::Toggle { pos, before } => match block_state_at(pos) {
                Some(current) if current != before => PendingInteractionResolution::Confirmed {
                    pos,
                    sound: WOOD_CREAK,
                },
                _ => PendingInteractionResolution::Pending,
            },
        }
    }
}

impl FootstepCadence {
    fn advance(&mut self, before: Vec3d, after: Vec3d, grounded: bool) -> Option<FootstepEvent> {
        if !grounded || !before.is_finite() || !after.is_finite() {
            self.distance_since_step = 0.0;
            return None;
        }
        let dx = after.x - before.x;
        let dz = after.z - before.z;
        let distance = dx.hypot(dz);
        if distance > MAX_FOOTSTEP_FRAME_DISTANCE {
            self.distance_since_step = 0.0;
            return None;
        }
        self.distance_since_step += distance;
        if self.distance_since_step < FOOTSTEP_STRIDE_BLOCKS {
            return None;
        }
        self.distance_since_step %= FOOTSTEP_STRIDE_BLOCKS;
        let event = FootstepEvent {
            position: after,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);
        Some(event)
    }

    fn reset(&mut self) {
        self.distance_since_step = 0.0;
    }
}

fn spatial_sound_seed(position: Vec3d, salt: u64) -> u64 {
    position.x.to_bits()
        ^ position.y.to_bits().rotate_left(21)
        ^ position.z.to_bits().rotate_left(42)
        ^ salt
}

fn acoustic_material_for_state(state: BlockStateId) -> AcousticMaterial {
    static BLOCK_STATES: LazyLock<BlockStateRegistry> =
        LazyLock::new(BlockStateRegistry::terrain_mvp);
    BLOCK_STATES
        .by_id(state)
        .map_or(AcousticMaterial::Neutral, |state| {
            AcousticMaterial::from_block_path(state.block.path())
        })
}

fn wheat_target_hud_for_state(state: BlockStateId) -> Option<WheatTargetHud> {
    static BLOCK_STATES: LazyLock<BlockStateRegistry> =
        LazyLock::new(BlockStateRegistry::terrain_mvp);
    let record = BLOCK_STATES.by_id(state)?;
    if record.block.path() != "wheat" {
        return None;
    }
    match record.properties.get("age")?.parse::<u8>().ok()? {
        0 => Some(WheatTargetHud::Sprout),
        1..=6 => Some(WheatTargetHud::Growing),
        7 => Some(WheatTargetHud::Mature),
        _ => None,
    }
}

fn placement_acoustic_material(
    debug_item: Option<DebugHotbarItem>,
    inventory_stack: Option<ItemStackSnapshot>,
) -> Option<AcousticMaterial> {
    match debug_item {
        Some(DebugHotbarItem::Block(state)) => Some(acoustic_material_for_state(state)),
        _ => match inventory_stack?.kind {
            ItemKind::WoodenHoe => Some(AcousticMaterial::Soft),
            ItemKind::WheatSeeds => Some(AcousticMaterial::Grass),
            ItemKind::Carrot => Some(AcousticMaterial::Grass),
            ItemKind::OakFence | ItemKind::OakFenceGate => Some(AcousticMaterial::Wood),
            _ => None,
        },
    }
}

fn block_sound_position(pos: BlockPos) -> Vec3d {
    Vec3d::new(
        f64::from(pos.x) + 0.5,
        f64::from(pos.y) + 0.5,
        f64::from(pos.z) + 0.5,
    )
}

fn sound_for_ui_action(action: GameUiAction) -> SoundKey {
    match action {
        GameUiAction::CancelDeleteWorld
        | GameUiAction::CloseHelp(_)
        | GameUiAction::CancelAssetPacks
        | GameUiAction::CancelStorageAction(_)
        | GameUiAction::BackToTitle
        | GameUiAction::BackToPause
        | GameUiAction::QuitToTitle => UI_BACK,
        GameUiAction::OpenWorldList
        | GameUiAction::OpenWorldCreate
        | GameUiAction::ConfirmDeleteWorld(_)
        | GameUiAction::OpenNewWorld
        | GameUiAction::OpenJoinRemote
        | GameUiAction::OpenBlockPalette
        | GameUiAction::OpenHelp(_)
        | GameUiAction::OpenOptions(_)
        | GameUiAction::OpenOptionsCategory(_, _)
        | GameUiAction::OpenServerSettings(_)
        | GameUiAction::OpenAssetPacks(_)
        | GameUiAction::ConfirmStorageAction(_, _) => UI_OPEN,
        GameUiAction::StartWorld
        | GameUiAction::EnterScenario(_)
        | GameUiAction::OpenWorld(_)
        | GameUiAction::CreateCatalogWorld
        | GameUiAction::DeleteWorld(_)
        | GameUiAction::CreateWorld(_)
        | GameUiAction::JoinRemote
        | GameUiAction::Resume
        | GameUiAction::Respawn
        | GameUiAction::ApplyHomesteadShowcasePreset
        | GameUiAction::AssignHotbarBlock { .. }
        | GameUiAction::AssignHotbarActor { .. }
        | GameUiAction::ApplyAssetPacks
        | GameUiAction::ExecuteStorageAction(_, _)
        | GameUiAction::ClearRebuildableCache
        | GameUiAction::Quit => UI_CONFIRM,
        _ => UI_SELECT,
    }
}

#[cfg(test)]
mod sound_effect_tests {
    use super::*;

    #[test]
    fn footstep_cadence_uses_grounded_horizontal_distance() {
        let mut cadence = FootstepCadence::default();
        assert_eq!(
            cadence.advance(Vec3d::ZERO, Vec3d::new(0.8, 0.0, 0.0), true),
            None
        );
        let event = cadence
            .advance(Vec3d::new(0.8, 0.0, 0.0), Vec3d::new(1.7, 0.0, 0.0), true)
            .expect("stride should emit one footstep");
        assert_eq!(event.position, Vec3d::new(1.7, 0.0, 0.0));
        assert_eq!(event.sequence, 0);

        assert_eq!(
            cadence.advance(Vec3d::new(1.7, 0.0, 0.0), Vec3d::new(1.7, 2.0, 0.0), false,),
            None
        );
        assert_eq!(cadence.distance_since_step, 0.0);
    }

    #[test]
    fn footstep_cadence_rejects_teleport_sized_frame_motion() {
        let mut cadence = FootstepCadence::default();
        assert_eq!(
            cadence.advance(Vec3d::ZERO, Vec3d::new(10.0, 0.0, 0.0), true),
            None
        );
        assert_eq!(cadence.distance_since_step, 0.0);
    }

    #[test]
    fn ui_actions_have_stable_semantic_feedback() {
        assert_eq!(sound_for_ui_action(GameUiAction::BackToTitle), UI_BACK);
        assert_eq!(sound_for_ui_action(GameUiAction::OpenWorldList), UI_OPEN);
        assert_eq!(
            sound_for_ui_action(GameUiAction::CreateCatalogWorld),
            UI_CONFIRM
        );
        assert_eq!(
            sound_for_ui_action(GameUiAction::CycleTexturePresentation),
            UI_SELECT
        );
    }

    #[test]
    fn interaction_sounds_wait_for_authoritative_block_changes() {
        let pos = BlockPos::new(1, 2, 3);
        let submitted = PendingInteractionSound {
            kind: PendingInteractionSoundKind::Break {
                pos,
                before: BlockStateId(7),
            },
            material: AcousticMaterial::Wood,
            submitted_at: MonotonicInstant::ZERO,
            sequence: 0,
        };

        assert_eq!(
            submitted.resolve(MonotonicInstant::from_nanos(1), |_| Some(BlockStateId(7))),
            PendingInteractionResolution::Pending
        );
        assert_eq!(
            submitted.resolve(MonotonicInstant::from_nanos(2), |_| {
                Some(AIR_BLOCK_STATE_ID)
            }),
            PendingInteractionResolution::Confirmed {
                pos,
                sound: mclone_audio::BREAK_WOOD,
            }
        );
    }

    #[test]
    fn placement_sound_confirms_only_a_changed_non_air_candidate() {
        let clicked = BlockPos::new(1, 2, 3);
        let adjacent = BlockPos::new(1, 3, 3);
        let submitted = PendingInteractionSound {
            kind: PendingInteractionSoundKind::Place {
                candidates: [
                    (clicked, Some(BlockStateId(7))),
                    (adjacent, Some(AIR_BLOCK_STATE_ID)),
                ],
            },
            material: AcousticMaterial::Stone,
            submitted_at: MonotonicInstant::ZERO,
            sequence: 0,
        };

        assert_eq!(
            submitted.resolve(MonotonicInstant::from_nanos(1), |pos| {
                (pos == clicked)
                    .then_some(BlockStateId(7))
                    .or_else(|| (pos == adjacent).then_some(BlockStateId(8)))
            }),
            PendingInteractionResolution::Confirmed {
                pos: adjacent,
                sound: mclone_audio::PLACE_STONE,
            }
        );
    }

    #[test]
    fn gate_sound_waits_for_authoritative_state_toggle() {
        let pos = BlockPos::new(1, 2, 3);
        let before = BlockStateId(terrain_id::OAK_FENCE_GATE_STATE_START);
        let submitted = PendingInteractionSound {
            kind: PendingInteractionSoundKind::Toggle { pos, before },
            material: AcousticMaterial::Wood,
            submitted_at: MonotonicInstant::ZERO,
            sequence: 0,
        };

        assert_eq!(
            submitted.resolve(MonotonicInstant::from_nanos(1), |_| Some(before)),
            PendingInteractionResolution::Pending
        );
        assert_eq!(
            submitted.resolve(MonotonicInstant::from_nanos(2), |_| {
                Some(BlockStateId(before.0 + 4))
            }),
            PendingInteractionResolution::Confirmed {
                pos,
                sound: WOOD_CREAK,
            }
        );
    }

    #[test]
    fn farming_items_use_shared_confirmed_placement_feedback() {
        assert_eq!(
            placement_acoustic_material(
                None,
                Some(ItemStackSnapshot {
                    kind: ItemKind::WoodenHoe,
                    count: 1,
                }),
            ),
            Some(AcousticMaterial::Soft)
        );
        assert_eq!(
            placement_acoustic_material(
                None,
                Some(ItemStackSnapshot {
                    kind: ItemKind::WheatSeeds,
                    count: 4,
                }),
            ),
            Some(AcousticMaterial::Grass)
        );
        assert_eq!(
            placement_acoustic_material(
                None,
                Some(ItemStackSnapshot {
                    kind: ItemKind::HuntingSpear,
                    count: 1,
                }),
            ),
            None
        );
    }

    #[test]
    fn wheat_target_states_have_actionable_hud_labels() {
        assert_eq!(
            wheat_target_hud_for_state(BlockStateId(229)),
            Some(WheatTargetHud::Sprout)
        );
        assert_eq!(
            wheat_target_hud_for_state(BlockStateId(234)),
            Some(WheatTargetHud::Growing)
        );
        assert_eq!(
            wheat_target_hud_for_state(BlockStateId(236)),
            Some(WheatTargetHud::Mature)
        );
        assert_eq!(wheat_target_hud_for_state(BlockStateId(1)), None);
    }
}

impl std::ops::Deref for DrawableWorldSlot {
    type Target = LocalParticipantPresentation;

    fn deref(&self) -> &Self::Target {
        &self.local_participant
    }
}

impl std::ops::DerefMut for DrawableWorldSlot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.local_participant
    }
}

/// Target-neutral prepared state for constructing or replacing the runtime
/// half of a drawable slot. Values are fully prepared before installation, so
/// a host never exposes a runtime paired with the previous camera or draw map.
struct DrawableWorldSlotInstall {
    id: WorldInstanceId,
    descriptor: Option<ActiveSessionDescriptor>,
    lifecycle: WorldSlotLifecycle,
    /// Content generation to install with the complete slot resource cluster;
    /// never used as platform-operation identity.
    asset_epoch: u64,
    scene: McloneSceneHostOptions,
    runtime: Option<SceneSessionRuntime>,
    local_startup: Option<SceneLocalStartup>,
    external_runtime_startup_pending: bool,
    camera: EngineCameraController,
    draw: TexturedSectionDrawResources,
    actors: Option<ActorDrawResources>,
    render_stats: RenderStreamStats,
    accepted_entry_pose: Option<WorldEntryPose>,
    pending_startup_sections: Vec<TexturedRenderSectionMesh>,
}

struct PreviewActorInstances {
    source_world: WorldInstanceId,
    instances: Vec<ActorInstance>,
    entity_count: usize,
    remote_player_count: usize,
    source_local_player_count: usize,
    entity_observations: Vec<EmbeddedWorldPreviewActorObservation>,
    remote_player_observations: Vec<EmbeddedWorldPreviewRemotePlayerObservation>,
}

fn changed_entity_motion(
    before: &[EntitySnapshot],
    after: &[EntitySnapshot],
) -> Option<(EntitySnapshot, EntitySnapshot)> {
    before
        .iter()
        .filter_map(|before| {
            after
                .iter()
                .find(|after| after.id == before.id && after.position != before.position)
                .copied()
                .map(|after| (*before, after))
        })
        .max_by(|(left_before, left_after), (right_before, right_after)| {
            left_after
                .position
                .subtract(left_before.position)
                .length_sqr()
                .total_cmp(
                    &right_after
                        .position
                        .subtract(right_before.position)
                        .length_sqr(),
                )
        })
}

fn changed_remote_player_motion(
    before: &[RemotePlayerUpdate],
    after: &[RemotePlayerUpdate],
) -> Option<(RemotePlayerUpdate, RemotePlayerUpdate)> {
    before.iter().find_map(|before| {
        after
            .iter()
            .find(|after| after.id == before.id && after.position != before.position)
            .copied()
            .map(|after| (*before, after))
    })
}

impl DrawableWorldSlot {
    fn new(
        install: DrawableWorldSlotInstall,
        render_admission_policy: RenderAdmissionPolicy,
    ) -> Self {
        let storage = WorldSlotStorage::from_scene(&install.scene, install.descriptor.as_ref());
        let local_participant = LocalParticipantPresentation::new(
            install.camera,
            install.scene.player_movement_cadence,
        );
        Self {
            id: install.id,
            descriptor: install.descriptor,
            storage,
            lifecycle: install.lifecycle,
            asset_epoch: install.asset_epoch,
            scene: install.scene,
            runtime: install.runtime,
            local_startup: install.local_startup,
            external_runtime_startup_pending: install.external_runtime_startup_pending,
            local_participant,
            footsteps: FootstepCadence::default(),
            pending_interaction_sounds: VecDeque::new(),
            interaction_sound_sequence: 0,
            mallard_tracks: VecDeque::new(),
            local_guest_preview: None,
            draw: install.draw,
            actors: install.actors,
            actor_interpolation: ActorInterpolationState::new(),
            last_actor_presentation_update: None,
            traversal_ready_sections: TraversalReadySectionCache::default(),
            section_uploads: RenderSectionUploadCoordinator::default(),
            render_stats: install.render_stats,
            render_admission_policy,
            accepted_entry_pose: install.accepted_entry_pose,
            pending_startup_sections: install.pending_startup_sections,
        }
    }

    fn install(&mut self, install: DrawableWorldSlotInstall) {
        self.id = install.id;
        self.descriptor = install.descriptor;
        self.storage = WorldSlotStorage::from_scene(&install.scene, self.descriptor.as_ref());
        self.lifecycle = install.lifecycle;
        self.asset_epoch = install.asset_epoch;
        self.scene = install.scene;
        self.runtime = install.runtime;
        self.local_startup = install.local_startup;
        self.external_runtime_startup_pending = install.external_runtime_startup_pending;
        self.camera = install.camera;
        self.movement = player_movement::PlayerMovementState::new(
            self.scene.player_movement_cadence,
            self.camera.snapshot(),
        );
        self.local_guest_preview = None;
        self.footsteps = FootstepCadence::default();
        self.pending_interaction_sounds.clear();
        self.interaction_sound_sequence = 0;
        self.draw = install.draw;
        self.actors = install.actors;
        self.actor_interpolation = ActorInterpolationState::new();
        self.last_actor_presentation_update = None;
        self.render_stats = install.render_stats;
        self.accepted_entry_pose = install.accepted_entry_pose;
        self.pending_startup_sections = install.pending_startup_sections;
        self.reset_field_guide_notification();
    }

    fn clear_stream_state(&mut self) {
        self.traversal_ready_sections.clear();
        self.section_uploads.clear();
        self.render_admission_policy.reset();
    }

    fn flush_persistence(&mut self) -> Result<usize> {
        match self.runtime.as_mut() {
            Some(runtime) => runtime.flush_persistence(),
            None => Ok(0),
        }
    }

    fn complete_detached_local_startup(
        &mut self,
        descriptor: ActiveSessionDescriptor,
        runtime: SceneSessionRuntime,
        camera: EngineCameraController,
        pending_startup_sections: Vec<TexturedRenderSectionMesh>,
    ) {
        self.descriptor = Some(descriptor);
        self.storage = WorldSlotStorage::from_scene(&self.scene, self.descriptor.as_ref());
        self.lifecycle = WorldSlotLifecycle::StandbyCpuReady;
        self.runtime = Some(runtime);
        self.local_startup = None;
        self.external_runtime_startup_pending = false;
        self.accepted_entry_pose = Some(WorldEntryPose::from_camera(&camera));
        self.camera = camera;
        self.movement = player_movement::PlayerMovementState::new(
            self.scene.player_movement_cadence,
            self.camera.snapshot(),
        );
        self.footsteps = FootstepCadence::default();
        self.pending_interaction_sounds.clear();
        self.interaction_sound_sequence = 0;
        self.pending_startup_sections = pending_startup_sections;
        self.render_stats = RenderStreamStats::default();
        self.actor_interpolation = ActorInterpolationState::new();
        self.last_actor_presentation_update = None;
        self.reset_field_guide_notification();
    }

    fn interpolated_actor_presentations(
        &mut self,
        now: MonotonicInstant,
    ) -> Vec<ActorPresentation> {
        let Some(runtime) = self.runtime.as_ref() else {
            self.actor_interpolation.reconcile_authoritative([]);
            self.last_actor_presentation_update = None;
            return Vec::new();
        };
        self.actor_interpolation.reconcile_authoritative_in(
            runtime.client().topology(),
            runtime
                .client()
                .actor_presentations_at(now.as_nanos() / 1_000_000),
        );
        let dt_seconds = self
            .last_actor_presentation_update
            .replace(now)
            .map_or(0.0, |last| {
                now.saturating_duration_since(last).as_secs_f32()
            });
        self.actor_interpolation
            .step_with_preinterpolated_remote_players(
                dt_seconds,
                ActorInterpolationConfig::default(),
            );
        self.actor_interpolation.presentations()
    }

    fn visible_field_guide_notification(
        &mut self,
        now: MonotonicInstant,
    ) -> Option<FieldGuideProgressSnapshot> {
        let progress = self.field_guide_progress()?;
        self.local_participant
            .field_guide_notification
            .observe(now, progress)
    }

    fn reset_field_guide_notification(&mut self) {
        self.local_participant.field_guide_notification.reset();
    }

    fn field_guide_progress(&self) -> Option<FieldGuideProgressSnapshot> {
        let runtime = self.runtime.as_ref()?;
        Some(FieldGuideProgressSnapshot::new(
            runtime.client().mallard_field_guide(),
            runtime.client().deer_field_guide(),
            runtime.client().bee_field_guide(),
            runtime.client().rabbit_field_guide(),
        ))
    }
}

pub struct McloneSceneHost {
    active_world: DrawableWorldSlot,
    standby_world: Option<DrawableWorldSlot>,
    next_world_instance_id: u64,
    warm_world_standby: Option<WarmWorldStandbyState>,
    prepared_warm_world_shell: Option<PreparedWarmWorldRendererShell>,
    lobby_launch: Option<LobbyLaunchState>,
    debug_lobby_auxiliary_player_script: bool,
    embedded_world_preview: Option<EmbeddedWorldPreview>,
    embedded_world_activation: EmbeddedWorldActivationState,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    embedded_world_activation_sequence: u64,
    world_gate: Option<WorldGate>,
    opaque_world_gate_renderer: Option<OpaqueWorldGateRenderer>,
    warm_world_switch_sequence: u64,
    last_warm_world_switch: Option<WarmWorldSwitchReport>,
    services: SceneHostServices,
    color_format: wgpu::TextureFormat,
    mesh_assets: TexturedMeshAssets,
    active_assets: PreparedSceneAssets,
    asset_replacement: Option<SceneAssetReplacementPending>,
    asset_replacement_status: AssetReplacementStatus,
    last_asset_replacement_commit: Option<AssetReplacementCommitReport>,
    asset_replacement_started_at: Option<MonotonicInstant>,
    asset_replacement_assets_ready_at: Option<MonotonicInstant>,
    asset_pack_sources: Option<AssetPackSourceRegistry>,
    asset_pack_preference: AssetPackPreference,
    asset_pack_preference_storage: Option<Box<dyn AssetPackPreferenceStorage>>,
    asset_pack_preference_error: Option<String>,
    graphics_preference_storage: Option<Box<dyn ClientGraphicsPreferenceStorage>>,
    graphics_preference_error: Option<String>,
    terrain_presentation_preference: GameTerrainPresentation,
    fog_settings: GameFogSettings,
    pending_leaf_detail: Option<mclone_mesh::LeafDetail>,
    pending_restored_asset_pack_selection:
        Option<(AssetPackSelection, mclone_assets::TexturePresentation)>,
    external_asset_pack_preparation: bool,
    pending_external_asset_pack_selection: Option<PlatformOperation<ExternalAssetPackSelection>>,
    external_asset_pack_operations: PlatformOperationLedger<ExternalAssetPackSelection, ()>,
    session: GameSessionCoordinator<ScenePendingSessionStart>,
    active_session_start_operations: PlatformOperationLedger<WorldInstanceId, SessionStartRequest>,
    #[cfg(not(target_arch = "wasm32"))]
    session_runtime_factory: Option<Box<dyn SceneSessionRuntimeFactory>>,
    client_experience: ClientExperienceController,
    storage_profile_ui: StorageProfileUiState,
    initial_alignment_mode: XrViewAlignmentMode,
    render_options: TexturedSectionRenderOptions,
    season_preview: SeasonPreviewSettings,
    player_collision_box_visible: bool,
    crosshair_visible: bool,
    travel_assist_mode: GameTravelAssistMode,
    selection_outline: SelectionOutlineRenderer,
    worldgen_lens: worldgen_lens::WorldgenLensState,
    worldgen_lens_renderer: WorldColorMeshRenderer,
    world_gui_renderer: WorldGuiRenderer,
    world_gui_overlay_renderer: WorldGuiRenderer,
    // Screen-space HUD renderer for the flat (mono) view topology (tactical 168
    // Slice 3). The stereo path draws UI as a world-space quad through
    // `world_gui_renderer`; the mono path draws it in screen space through the
    // shared `render_full_frame_for_view*` GUI slot. Built lazily on first
    // screen-space-HUD mono render (needs device/queue + the chunk atlas), so it
    // stays `None` on headsets that never take the mono path.
    mono_gui: Option<GuiRenderer>,
    mono_ui_context: Option<MonoUiContext>,
    diagnostic_panel: XrDiagnosticPanel,
    ui: GameUiHost,
    menu_overlay_cache: XrMenuPanelOverlayCache,
    status_overlay: StatusOverlay,
    xr_render_path_state: Option<GameXrRenderPathState>,
    pending_xr_render_mode_request: Option<GameXrRenderMode>,
    sky: SkyRenderer,
    screen_effects: ScreenEffectsRenderer,
    terrain_view: Option<terrain_view::SceneTerrainViewState>,
    terrain_horizon_diagnostic: mclone_terrain_view::TerrainHorizonDiagnostic,
    terrain_vegetation_executor_factory:
        Option<terrain_view::SceneTerrainVegetationExecutorFactory>,
    underwater_effects: XrUnderwaterEffectStates,
    last_underwater_update: Option<MonotonicInstant>,
    head_comfort: XrHeadComfortState,
    tracking_origin: Option<XrTrackingOrigin>,
    locomotion_mode: XrLocomotionMode,
    turn_policy: XrTurnPolicy,
    snap_turn_state: XrSnapTurnState,
    blink_teleport: XrBlinkTeleportState,
    mono_blink_debug: MonoBlinkDebugState,
    display_refresh_hz: Option<f32>,
    render_split_timing_enabled: bool,
    defer_eye_waits_enabled: bool,
    overlap_runtime_prefetch_enabled: bool,
    prefetched_live_upload: Option<XrTerrainUploadSummary>,
    render_section_upload_budget: Option<usize>,
    render_section_accept_budget: Option<usize>,
    render_completed_result_accept_budget: Option<usize>,
    per_view_uniform_frame: u32,
    last_locomotion_update: Option<MonotonicInstant>,
    player_pose_sync: pose_sync::PlayerPoseSyncCadence,
    menu_toggle_down: bool,
    game_ui_toggle_down: bool,
    menu_pointer_down: bool,
    menu_panel_pose: Option<WorldGuiPanel>,
    menu_panel_anchor: XrUiPanelAnchor,
    menu_panel_recenter_pending: bool,
    latest_xr_input: XrInputFrame,
    latest_xr_head_gaze_stage: Option<(Vec3, Vec3)>,
    first_eye_summary: Option<FullFrameRenderSummary>,
    last_ui_panel_stats: WorldGuiPanelRenderStats,
    last_ui_draw_cache_stats: UiDrawCacheStats,
    rendered_frames: u32,
    seed_reroll: NewWorldSeedReroll,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct QualifiedTerrainTranslucentSubmission {
    world: WorldInstanceId,
    source: TerrainCompositionSource,
    section: RenderSectionKey,
    composition_center: Vec3,
}

fn topology_debug_world_lines(topology: HorizontalTopology, observer: Vec3) -> Vec<WorldGuiLine> {
    const HALF_SPAN: f32 = 48.0;
    const HALF_HEIGHT: f32 = 32.0;
    const COLOR: [f32; 4] = [0.1, 0.95, 1.0, 0.95];
    let mut lines = Vec::new();
    let bottom = observer.y - HALF_HEIGHT;
    let top = observer.y + HALF_HEIGHT;

    if let AxisTopology::Periodic {
        minimum_chunk,
        period_chunks,
    } = topology.x
    {
        let minimum = minimum_chunk as f32 * CHUNK_WIDTH as f32;
        let period = period_chunks as f32 * CHUNK_WIDTH as f32;
        let seam = minimum + ((observer.x - minimum) / period).round() * period;
        for offset in (-3..=3).map(|step| step as f32 * 16.0) {
            let z = observer.z + offset;
            lines.push(WorldGuiLine::new(
                Vec3::new(seam, bottom, z),
                Vec3::new(seam, top, z),
                COLOR,
            ));
        }
        for y in [bottom, observer.y, top] {
            lines.push(WorldGuiLine::new(
                Vec3::new(seam, y, observer.z - HALF_SPAN),
                Vec3::new(seam, y, observer.z + HALF_SPAN),
                COLOR,
            ));
        }
    }
    if let AxisTopology::Periodic {
        minimum_chunk,
        period_chunks,
    } = topology.z
    {
        let minimum = minimum_chunk as f32 * CHUNK_WIDTH as f32;
        let period = period_chunks as f32 * CHUNK_WIDTH as f32;
        let seam = minimum + ((observer.z - minimum) / period).round() * period;
        for offset in (-3..=3).map(|step| step as f32 * 16.0) {
            let x = observer.x + offset;
            lines.push(WorldGuiLine::new(
                Vec3::new(x, bottom, seam),
                Vec3::new(x, top, seam),
                COLOR,
            ));
        }
        for y in [bottom, observer.y, top] {
            lines.push(WorldGuiLine::new(
                Vec3::new(observer.x - HALF_SPAN, y, seam),
                Vec3::new(observer.x + HALF_SPAN, y, seam),
                COLOR,
            ));
        }
    }
    lines
}

/// Merge per-store translucent records at the scene boundary, where durable
/// world identity is available. Stereo and multiview callers pass both eyes;
/// their midpoint and averaged forward vector produce one immutable order for
/// the whole frame.
fn compose_translucent_terrain_order(
    active_world: WorldInstanceId,
    active_records: Vec<TexturedSectionTranslucentRecord>,
    placed_world: WorldInstanceId,
    placed_records: Vec<TexturedSectionTranslucentRecord>,
    render_views: &[ChunkRenderView],
) -> Vec<TerrainTranslucentSubmission> {
    debug_assert!(!render_views.is_empty());
    let view_count = render_views.len() as f32;
    let camera_position = render_views
        .iter()
        .fold(Vec3::ZERO, |sum, view| sum + view.camera_position)
        / view_count;
    let averaged_forward = render_views
        .iter()
        .fold(Vec3::ZERO, |sum, view| sum + view.camera_forward)
        .normalize_or_zero();
    let camera_forward = if averaged_forward.length_squared() > 0.0 {
        averaged_forward
    } else {
        render_views[0].camera_forward
    };
    let mut qualified = active_records
        .into_iter()
        .map(|record| QualifiedTerrainTranslucentSubmission {
            world: active_world,
            source: TerrainCompositionSource::Active,
            section: record.key,
            composition_center: record.composition_center,
        })
        .chain(
            placed_records
                .into_iter()
                .map(|record| QualifiedTerrainTranslucentSubmission {
                    world: placed_world,
                    source: TerrainCompositionSource::Placed,
                    section: record.key,
                    composition_center: record.composition_center,
                }),
        )
        .collect::<Vec<_>>();
    qualified.sort_by(|left, right| {
        let left_depth = (left.composition_center - camera_position).dot(camera_forward);
        let right_depth = (right.composition_center - camera_position).dot(camera_forward);
        right_depth
            .partial_cmp(&left_depth)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.world.cmp(&left.world))
            .then_with(|| right.section.cmp(&left.section))
    });
    qualified
        .into_iter()
        .map(|submission| TerrainTranslucentSubmission {
            source: submission.source,
            section: submission.section,
        })
        .collect()
}

fn embedded_translucent_order_snapshot(
    active_world: WorldInstanceId,
    preview_world: WorldInstanceId,
    order: &[TerrainTranslucentSubmission],
) -> EmbeddedWorldPreviewTranslucentOrderSnapshot {
    let qualified = |submission: TerrainTranslucentSubmission| {
        EmbeddedWorldPreviewTranslucentSubmissionSnapshot {
            world: match submission.source {
                TerrainCompositionSource::Active => active_world,
                TerrainCompositionSource::Placed => preview_world,
            },
            section: submission.section,
        }
    };
    EmbeddedWorldPreviewTranslucentOrderSnapshot {
        section_count: order.len(),
        active_section_count: order
            .iter()
            .filter(|submission| submission.source == TerrainCompositionSource::Active)
            .count(),
        preview_section_count: order
            .iter()
            .filter(|submission| submission.source == TerrainCompositionSource::Placed)
            .count(),
        source_switch_count: order
            .windows(2)
            .filter(|pair| pair[0].source != pair[1].source)
            .count(),
        first: order.first().copied().map(qualified),
        last: order.last().copied().map(qualified),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_composed_translucent_terrain_multiview(
    active_draw: &TexturedSectionDrawResources,
    placed_draw: &TexturedSectionDrawResources,
    placed_renderer: &mclone_render::chunk::PlacedTexturedSectionRenderer,
    order: &[TerrainTranslucentSubmission],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: ChunkMultiviewRenderTarget<'_>,
    render_views: [ChunkRenderView; 2],
    active_options: [TexturedSectionRenderOptions; 2],
    placed_options: [TexturedSectionRenderOptions; 2],
    context: mclone_render::placement::WorldCompositionContext,
) -> Result<()> {
    let mut start = 0;
    while start < order.len() {
        let source = order[start].source;
        let mut end = start + 1;
        while end < order.len() && order[end].source == source {
            end += 1;
        }
        let keys = order[start..end]
            .iter()
            .map(|submission| submission.section)
            .collect::<Vec<_>>();
        match source {
            TerrainCompositionSource::Active => {
                active_draw.render_ordered_translucent_sections_multiview(
                    &keys,
                    device,
                    queue,
                    encoder,
                    target,
                    render_views,
                    active_options,
                )?;
            }
            TerrainCompositionSource::Placed => {
                placed_draw.render_ordered_placed_translucent_sections_multiview(
                    placed_renderer,
                    &keys,
                    device,
                    queue,
                    encoder,
                    target,
                    render_views,
                    placed_options,
                    context,
                )?;
            }
        }
        start = end;
    }
    Ok(())
}

impl McloneSceneHost {
    /// Synchronously flush world edits to persistent storage without tearing
    /// down the session. Backs the lifecycle save point so Quest world edits
    /// survive an activity Pause -> OS kill, which previously only saved on the
    /// Drop-driven `shutdown_persistence` (tactical 168 Slice 0). Returns the
    /// total number of chunks queued for write across the active and retained
    /// standby worlds; a no-op (`Ok(0)`) before either runtime exists or when
    /// both host modes are remote.
    pub fn flush_persistence(&mut self) -> Result<usize> {
        // Attempt both slots even if one flush fails. Backgrounding is a
        // lifecycle save point, so an active-world failure must not prevent an
        // independently persisted retained world from committing its edits.
        let active = self.active_world.flush_persistence();
        let standby = self
            .standby_world
            .as_mut()
            .map_or(Ok(0), DrawableWorldSlot::flush_persistence);
        match (active, standby) {
            (Ok(active), Ok(standby)) => active
                .checked_add(standby)
                .context("sum active and standby persistence flush counts"),
            (Err(active), Ok(_)) => Err(active).context("flush active world persistence"),
            (Ok(_), Err(standby)) => Err(standby).context("flush standby world persistence"),
            (Err(active), Err(standby)) => Err(anyhow!(
                "flush active and standby world persistence: active={active:#}; standby={standby:#}"
            )),
        }
    }

    /// Shared lifecycle policy for a host entering the background. Native local
    /// worlds synchronously commit durable edits in both retained slots;
    /// remote sessions are a no-op because persistence belongs to the
    /// dedicated server. Platform drivers report lifecycle transitions but do
    /// not choose the save policy.
    pub fn on_background(&mut self) -> Result<usize> {
        self.flush_persistence()
    }

    pub fn render_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        self.render_frame_inner(
            device,
            queue,
            views,
            left_target,
            right_target,
            XrTerrainRuntimeUpdateMode::Live,
        )
    }

    pub fn render_xr_scene_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tracked_views: [XrView; 2],
        eye_fovs: [XrFov; 2],
        frozen_render: bool,
        fixed_render_view_pose: Option<XrStartupViewPose>,
        target: XrSceneFrameTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        match (target, frozen_render) {
            (XrSceneFrameTarget::PerEye { left, right }, false) => {
                self.render_frame(device, queue, tracked_views, left, right)
            }
            (XrSceneFrameTarget::Multiview(target), false) => {
                self.render_frame_multiview(device, queue, tracked_views, target)
            }
            (XrSceneFrameTarget::PerEye { left, right }, true) => self
                .render_frame_frozen_runtime_at_view_pose(
                    device,
                    queue,
                    fixed_render_view_pose
                        .context("frozen XR scene frame requires a fixed render view pose")?,
                    eye_fovs,
                    left,
                    right,
                ),
            (XrSceneFrameTarget::Multiview(target), true) => self
                .render_frame_multiview_frozen_runtime_at_view_pose(
                    device,
                    queue,
                    fixed_render_view_pose
                        .context("frozen XR scene frame requires a fixed render view pose")?,
                    eye_fovs,
                    target,
                ),
        }
    }

    pub fn render_frame_frozen_runtime(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        self.render_frame_inner(
            device,
            queue,
            views,
            left_target,
            right_target,
            XrTerrainRuntimeUpdateMode::Frozen,
        )
    }

    pub fn render_frame_frozen_runtime_at_view_pose(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_pose: XrStartupViewPose,
        eye_fovs: [XrFov; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        self.sync_player_lifecycle_ui();
        let mut timing = XrTerrainFrameTiming::default();
        let render_views_start = self.services.clock.now();
        let render_views = fixed_startup_view_pose_render_views_with_far(
            view_pose,
            eye_fovs,
            self.terrain_projection_far_distance(XR_FAR),
        )?;
        timing.render_views_ms = elapsed_ms(self.services.clock.elapsed_since(render_views_start));
        self.render_prepared_frame(
            device,
            queue,
            render_views,
            left_target,
            right_target,
            XrTerrainRuntimeUpdateMode::Frozen,
            timing,
        )
    }

    pub fn render_frame_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        self.render_frame_multiview_inner(
            device,
            queue,
            views,
            target,
            XrTerrainRuntimeUpdateMode::Live,
        )
    }

    pub fn render_frame_multiview_frozen_runtime_at_view_pose(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_pose: XrStartupViewPose,
        eye_fovs: [XrFov; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        self.sync_player_lifecycle_ui();
        let mut timing = XrTerrainFrameTiming::default();
        let render_views_start = self.services.clock.now();
        let render_views = fixed_startup_view_pose_render_views_with_far(
            view_pose,
            eye_fovs,
            self.terrain_projection_far_distance(XR_FAR),
        )?;
        timing.render_views_ms = elapsed_ms(self.services.clock.elapsed_since(render_views_start));
        self.render_prepared_frame_multiview(
            device,
            queue,
            render_views,
            target,
            XrTerrainRuntimeUpdateMode::Frozen,
            timing,
        )
    }

    pub fn render_terrain_multiview_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let mut render_views = self.render_views(&views)?;
        if self.advance_local_startup(device, queue)? {
            render_views = self.render_views(&views)?;
        }
        self.render_prepared_terrain_multiview_frame(device, queue, render_views, target)
    }

    pub fn render_terrain_multiview_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let render_views = self.render_views(&views)?;
        self.render_prepared_terrain_multiview_frame_frozen(device, queue, render_views, target)
    }

    pub fn render_sky_terrain_multiview_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let render_views = self.render_views(&views)?;
        self.render_prepared_terrain_multiview_frame_frozen_inner(
            device,
            queue,
            render_views,
            target,
            true,
            false,
            false,
        )
    }

    pub fn render_sky_terrain_actors_multiview_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let render_views = self.render_views(&views)?;
        self.render_prepared_terrain_multiview_frame_frozen_inner(
            device,
            queue,
            render_views,
            target,
            true,
            true,
            false,
        )
    }

    pub fn render_sky_terrain_actors_overlays_multiview_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let render_views = self.render_views(&views)?;
        self.update_menu_panel_pose(render_views);
        self.render_prepared_terrain_multiview_frame_frozen_inner(
            device,
            queue,
            render_views,
            target,
            true,
            true,
            true,
        )
    }

    pub fn render_overlay_multiview_smoke_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<()> {
        let render_views = self.render_views(&views)?;
        let center_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let forward = average_unit_direction(
            render_views[0].camera_forward,
            render_views[1].camera_forward,
            Vec3::Z,
        );
        let right = average_unit_direction(
            render_views[0].camera_right,
            render_views[1].camera_right,
            Vec3::X,
        );
        let up = average_unit_direction(
            render_views[0].camera_up,
            render_views[1].camera_up,
            Vec3::Y,
        );
        let marker_center = center_position + forward * 2.0;
        let outline_min = marker_center - Vec3::splat(0.12);
        let outline_max = marker_center + Vec3::splat(0.12);
        let outline = SelectionOutline::new(vec![Aabb::new(
            f64::from(outline_min.x),
            f64::from(outline_min.y),
            f64::from(outline_min.z),
            f64::from(outline_max.x),
            f64::from(outline_max.y),
            f64::from(outline_max.z),
        )])
        .with_color([1.0, 0.0, 1.0, 0.8]);
        let panel = WorldGuiPanel::new(marker_center + up * 0.24, right, up, 0.42, 0.22);
        let mut gui = GuiDrawList::new();
        gui.fill(
            Rect::new(0.0, 0.0, 96.0, 48.0),
            Color::rgba(40, 170, 240, 220),
        );
        let line = WorldGuiLine::new(
            marker_center - right * 0.3,
            marker_center + right * 0.3,
            [1.0, 1.0, 0.0, 1.0],
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_xr_overlay_multiview_smoke_encoder"),
        });
        let overlay_target = RenderFrameTarget::color(target.color_view, target.size);
        self.screen_effects
            .render_underwater_multiview(
                device,
                queue,
                &mut encoder,
                overlay_target,
                [
                    Some(UnderwaterOverlay::new(0.85, 0.04, [0.0, 0.0], 1.0)),
                    Some(UnderwaterOverlay::new(0.85, 0.04, [0.25, -0.15], 1.0)),
                ],
            )
            .context("render synthetic XR underwater screen effect multiview smoke")?;
        self.selection_outline
            .render_multiview(
                device,
                queue,
                &mut encoder,
                overlay_target,
                target.depth,
                render_views,
                Some(&outline),
            )
            .context("render synthetic XR selection outline multiview smoke")?;
        self.world_gui_renderer
            .render_panel_multiview(
                device,
                queue,
                &mut encoder,
                overlay_target,
                render_views,
                [96, 48],
                [96.0, 48.0],
                &gui,
                panel,
                &[line],
            )
            .context("render synthetic XR world GUI multiview smoke")?;
        let submission = queue.submit(Some(encoder.finish()));
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .map(|_| ())
            .context("wait for XR overlay multiview smoke submission")?;
        device
            .poll(wgpu::PollType::Wait)
            .map(|_| ())
            .context("wait for XR overlay multiview smoke device idle")?;
        Ok(())
    }

    pub fn render_terrain_stereo_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainStereoFrameSummary> {
        let render_views = self.render_views(&views)?;
        self.render_prepared_terrain_stereo_frame_frozen(
            device,
            queue,
            render_views,
            left_target,
            right_target,
        )
    }

    pub fn render_sky_terrain_stereo_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainStereoFrameSummary> {
        let render_views = self.render_views(&views)?;
        self.render_prepared_terrain_stereo_frame_frozen_inner(
            device,
            queue,
            render_views,
            left_target,
            right_target,
            true,
            false,
        )
    }

    pub fn render_sky_terrain_actors_stereo_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainStereoFrameSummary> {
        let render_views = self.render_views(&views)?;
        self.render_prepared_terrain_stereo_frame_frozen_inner(
            device,
            queue,
            render_views,
            left_target,
            right_target,
            true,
            true,
        )
    }

    fn render_frame_inner(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
        runtime_mode: XrTerrainRuntimeUpdateMode,
    ) -> Result<XrTerrainFrameSummary> {
        let mut timing = XrTerrainFrameTiming::default();
        let render_views_start = self.services.clock.now();
        let mut render_views = self.render_views(&views)?;
        timing.render_views_ms += elapsed_ms(self.services.clock.elapsed_since(render_views_start));
        self.update_menu_panel_pose(render_views);
        let menu_pointer_start = self.services.clock.now();
        if self
            .apply_menu_pointer_input(device, queue)
            .context("apply XR menu pointer input")?
        {
            timing.menu_pointer_ms +=
                elapsed_ms(self.services.clock.elapsed_since(menu_pointer_start));
            let render_views_start = self.services.clock.now();
            render_views = self.render_views(&views)?;
            timing.render_views_ms +=
                elapsed_ms(self.services.clock.elapsed_since(render_views_start));
            self.update_menu_panel_pose(render_views);
        } else {
            timing.menu_pointer_ms +=
                elapsed_ms(self.services.clock.elapsed_since(menu_pointer_start));
        }
        if self.advance_local_startup(device, queue)? {
            let render_views_start = self.services.clock.now();
            render_views = self.render_views(&views)?;
            timing.render_views_ms +=
                elapsed_ms(self.services.clock.elapsed_since(render_views_start));
            self.update_menu_panel_pose(render_views);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.world_gate.is_some() && matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live) {
            let midpoint =
                (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
            if self.apply_world_gate_visual_midpoint(Vec3d::new(
                f64::from(midpoint.x),
                f64::from(midpoint.y),
                f64::from(midpoint.z),
            ))? {
                let render_views_start = self.services.clock.now();
                render_views = self.render_views(&views)?;
                timing.render_views_ms +=
                    elapsed_ms(self.services.clock.elapsed_since(render_views_start));
                self.update_menu_panel_pose(render_views);
            }
        }
        self.render_prepared_frame(
            device,
            queue,
            render_views,
            left_target,
            right_target,
            runtime_mode,
            timing,
        )
    }

    fn render_frame_multiview_inner(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        target: XrTerrainMultiviewTarget<'_>,
        runtime_mode: XrTerrainRuntimeUpdateMode,
    ) -> Result<XrTerrainFrameSummary> {
        let mut timing = XrTerrainFrameTiming::default();
        let render_views_start = self.services.clock.now();
        let mut render_views = self.render_views(&views)?;
        timing.render_views_ms += elapsed_ms(self.services.clock.elapsed_since(render_views_start));
        self.update_menu_panel_pose(render_views);
        let menu_pointer_start = self.services.clock.now();
        if self
            .apply_menu_pointer_input(device, queue)
            .context("apply XR menu pointer input for multiview frame")?
        {
            timing.menu_pointer_ms +=
                elapsed_ms(self.services.clock.elapsed_since(menu_pointer_start));
            let render_views_start = self.services.clock.now();
            render_views = self.render_views(&views)?;
            timing.render_views_ms +=
                elapsed_ms(self.services.clock.elapsed_since(render_views_start));
            self.update_menu_panel_pose(render_views);
        } else {
            timing.menu_pointer_ms +=
                elapsed_ms(self.services.clock.elapsed_since(menu_pointer_start));
        }
        if self.advance_local_startup(device, queue)? {
            let render_views_start = self.services.clock.now();
            render_views = self.render_views(&views)?;
            timing.render_views_ms +=
                elapsed_ms(self.services.clock.elapsed_since(render_views_start));
            self.update_menu_panel_pose(render_views);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.world_gate.is_some() && matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live) {
            let midpoint =
                (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
            if self.apply_world_gate_visual_midpoint(Vec3d::new(
                f64::from(midpoint.x),
                f64::from(midpoint.y),
                f64::from(midpoint.z),
            ))? {
                let render_views_start = self.services.clock.now();
                render_views = self.render_views(&views)?;
                timing.render_views_ms +=
                    elapsed_ms(self.services.clock.elapsed_since(render_views_start));
                self.update_menu_panel_pose(render_views);
            }
        }
        self.render_prepared_frame_multiview(
            device,
            queue,
            render_views,
            target,
            runtime_mode,
            timing,
        )
    }

    fn render_prepared_frame_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        target: XrTerrainMultiviewTarget<'_>,
        runtime_mode: XrTerrainRuntimeUpdateMode,
        mut timing: XrTerrainFrameTiming,
    ) -> Result<XrTerrainFrameSummary> {
        self.poll_asset_replacement(device, queue)?;
        let center_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let frame_deadline = self.render_compile_frame_deadline();
        let upload = self.live_upload_for_frame(
            device,
            center_position,
            runtime_mode,
            frame_deadline,
            &mut timing,
        )?;
        let terrain_view_enabled = self.prepare_terrain_view_for_frame(
            device,
            queue,
            [
                f64::from(center_position.x),
                f64::from(center_position.y),
                f64::from(center_position.z),
            ],
        )?;
        let multiview = self
            .render_prepared_terrain_multiview_frame_with_upload_inner(
                device,
                queue,
                render_views,
                target,
                upload,
                true,
                true,
                true,
                terrain_view_enabled,
                Some(&mut timing),
            )
            .context("render XR full-frame multiview")?;
        Ok(self.frame_summary_from_multiview(multiview, timing))
    }

    fn render_prepared_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
        runtime_mode: XrTerrainRuntimeUpdateMode,
        mut timing: XrTerrainFrameTiming,
    ) -> Result<XrTerrainFrameSummary> {
        self.poll_asset_replacement(device, queue)?;
        let center_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let frame_deadline = self.render_compile_frame_deadline();
        let upload = self.live_upload_for_frame(
            device,
            center_position,
            runtime_mode,
            frame_deadline,
            &mut timing,
        )?;
        let sky_state = self.solar_render_state();
        let render_options = self.effective_render_options(center_position, sky_state);
        let sky_clear_color = sky_state.clear_color();
        let underwater_overlays = self.underwater_overlays(render_views);
        let actor_instances = self.current_actor_instances();
        let preview_actor_instances = self.current_preview_actor_instances();
        let render_options =
            render_options_with_actor_grass_interactors(render_options, &actor_instances);
        let terrain_view_enabled = self.prepare_terrain_view_for_frame(
            device,
            queue,
            [
                f64::from(center_position.x),
                f64::from(center_position.y),
                f64::from(center_position.z),
            ],
        )?;
        let collect_split_timing = self.render_split_timing_enabled;
        let records_start = collect_split_timing.then(|| self.services.clock.now());
        let (prepared_records, record_cache_prepare) =
            self.active_world.draw.prepare_render_records_with_stats();
        timing.record_cache_prepare = record_cache_prepare;
        timing.shared_records_ms = records_start.map_or(0.0, |start| {
            elapsed_ms(self.services.clock.elapsed_since(start))
        });
        let (terrain_views, mut terrain_options, _) =
            self.terrain_render_views_and_options(render_views, sky_state);
        terrain_options = terrain_options
            .map(|options| render_options_with_actor_grass_interactors(options, &actor_instances));
        let (prepared_stereo_draw, stereo_draw_timing) = if collect_split_timing {
            self.active_world.draw.prepare_stereo_draw_timed(
                &prepared_records,
                terrain_views,
                terrain_options,
            )
        } else {
            (
                self.active_world.draw.prepare_stereo_draw(
                    &prepared_records,
                    terrain_views,
                    terrain_options,
                ),
                Default::default(),
            )
        };
        let preview_stereo_draw = self
            .embedded_world_preview
            .as_ref()
            .filter(|preview| preview.phase == EmbeddedWorldPreviewPhase::Visible)
            .and_then(|preview| {
                self.standby_world
                    .as_ref()
                    .filter(|slot| slot.id == preview.source_world)
                    .map(|slot| {
                        let records = slot
                            .draw
                            .prepare_render_records_for_context(preview.context);
                        let options =
                            self.render_options
                                .with_sky_darken(sky_state.sky_darken())
                                .with_grass_time_seconds(render_options.grass_time_seconds)
                                .with_grass_interactors(
                                    preview_actor_instances.as_ref().map_or_else(
                                        GrassInteractorSet::default,
                                        |actors| {
                                            grass_interactors_from_actors(None, &actors.instances)
                                        },
                                    ),
                                )
                                .with_topology(slot.runtime.as_ref().map_or(
                                    mclone_core::HorizontalTopology::UNBOUNDED,
                                    |runtime| runtime.client().topology(),
                                ));
                        let bounded_section_count = records.section_keys().len();
                        let out_of_region_submission_count = records
                            .section_keys()
                            .filter(|key| !preview.region.contains(*key))
                            .count();
                        let (prepared, timing) = slot.draw.prepare_placed_stereo_draw_timed(
                            &records,
                            render_views,
                            [options; 2],
                            preview.context,
                        );
                        (
                            prepared,
                            timing,
                            bounded_section_count,
                            out_of_region_submission_count,
                        )
                    })
            });
        let preview_translucent_order = self
            .embedded_world_preview
            .as_ref()
            .filter(|preview| preview.phase == EmbeddedWorldPreviewPhase::Visible)
            .zip(preview_stereo_draw.as_ref())
            .map(|(preview, prepared)| {
                compose_translucent_terrain_order(
                    self.active_world.id,
                    prepared_stereo_draw
                        .translucent_records(mclone_render::placement::WorldPlacement::identity()),
                    preview.source_world,
                    prepared.0.translucent_records(preview.context.placement()),
                    &terrain_views,
                )
            });
        let defer_eye_waits = self.defer_eye_waits_enabled;
        let uniform_frame = self.next_per_view_uniform_frame();
        let left_view_slot = LEFT_EYE_VIEW_SLOT.in_uniform_frame(uniform_frame);
        let right_view_slot = RIGHT_EYE_VIEW_SLOT.in_uniform_frame(uniform_frame);
        let diagnostic_panel = xr_diagnostic_panel_from_render_views(render_views);
        let left_eye_start = self.services.clock.now();
        let left_eye = self.render_eye_target(
            device,
            queue,
            &prepared_stereo_draw,
            preview_stereo_draw.as_ref().map(|prepared| &prepared.0),
            preview_translucent_order.as_deref(),
            left_target,
            render_views[0],
            diagnostic_panel,
            &actor_instances,
            preview_actor_instances.as_ref(),
            render_options,
            sky_clear_color,
            sky_state,
            underwater_overlays[0],
            "left",
            left_view_slot,
            !defer_eye_waits,
            terrain_view_enabled,
        )?;
        timing.left_eye_ms = elapsed_ms(self.services.clock.elapsed_since(left_eye_start));
        timing.left_eye_render = left_eye.timing;
        timing.left_eye_render.cull_ms += stereo_draw_timing.cull_ms;
        timing.left_eye_render.translucent_collect_ms += stereo_draw_timing.translucent_collect_ms;
        timing.left_eye_render.translucent_sort_ms += stereo_draw_timing.translucent_sort_ms;
        timing.left_eye_render.prepare_ms += stereo_draw_timing.prepare_ms;
        let right_eye_start = self.services.clock.now();
        let mut right_eye = self.render_eye_target(
            device,
            queue,
            &prepared_stereo_draw,
            preview_stereo_draw.as_ref().map(|prepared| &prepared.0),
            preview_translucent_order.as_deref(),
            right_target,
            render_views[1],
            diagnostic_panel,
            &actor_instances,
            preview_actor_instances.as_ref(),
            render_options,
            sky_clear_color,
            sky_state,
            underwater_overlays[1],
            "right",
            right_view_slot,
            !defer_eye_waits,
            terrain_view_enabled,
        )?;
        timing.right_eye_ms = elapsed_ms(self.services.clock.elapsed_since(right_eye_start));
        timing.right_eye_render = right_eye.timing;
        timing.stereo_submit_ms =
            timing.left_eye_render.submit_ms + timing.right_eye_render.submit_ms;
        if defer_eye_waits {
            if self.overlap_runtime_prefetch_enabled
                && matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live)
                && self.active_world.local_startup.is_none()
                && self.active_world.runtime.is_some()
            {
                let prefetch_start = self.services.clock.now();
                let mut prefetch_timing = XrTerrainFrameTiming::default();
                let prefetch_deadline = self.render_compile_frame_deadline();
                let upload = self
                    .poll_runtime_and_upload(
                        device,
                        center_position,
                        prefetch_deadline,
                        &mut prefetch_timing,
                    )
                    .context("prefetch XR runtime upload before deferred stereo wait")?;
                self.prefetched_live_upload = Some(upload);
                timing.overlap_runtime_prefetch_ms =
                    elapsed_ms(self.services.clock.elapsed_since(prefetch_start));
                timing.overlap_runtime_prefetch_poll_ms = prefetch_timing.runtime_poll_ms;
                timing.overlap_runtime_prefetch_sync_ms = prefetch_timing.runtime_sync_ms;
                timing.overlap_runtime_prefetch_gpu_upload_ms =
                    prefetch_timing.runtime_gpu_upload_ms;
                timing.overlap_runtime_prefetch_ready_sections_ms =
                    prefetch_timing.runtime_ready_sections_ms;
            }
            let submission = right_eye
                .submission
                .take()
                .context("deferred XR eye wait missing right-eye submission")?;
            let poll_wait_start = self.services.clock.now();
            Self::wait_for_xr_submission(device, submission, "deferred XR terrain stereo render")
                .context("wait for deferred XR terrain stereo render")?;
            timing.stereo_poll_wait_ms =
                elapsed_ms(self.services.clock.elapsed_since(poll_wait_start));
        } else {
            timing.stereo_poll_wait_ms =
                timing.left_eye_render.poll_wait_ms + timing.right_eye_render.poll_wait_ms;
        }
        let mut ui_panel_stats = left_eye.ui_panel;
        ui_panel_stats.add(right_eye.ui_panel);
        let mut ui_draw_cache_stats = left_eye.ui_draw_cache;
        ui_draw_cache_stats.add(right_eye.ui_draw_cache);
        self.last_ui_panel_stats = ui_panel_stats;
        self.last_ui_draw_cache_stats = ui_draw_cache_stats;
        let first_drawn_section_count = left_eye.summary.drawn_section_count;
        let active_world_id = self.active_world.id;
        let preview_actor_receipt = preview_actor_instances.as_ref().and_then(|actors| {
            self.standby_world
                .as_ref()
                .filter(|slot| slot.id == actors.source_world)
                .and_then(|slot| slot.actors.as_ref())
                .map(|resources| {
                    (
                        actors.entity_count,
                        actors.remote_player_count,
                        actors.source_local_player_count,
                        resources.resource_snapshot(),
                    )
                })
        });
        let actor_rendered_at = self.services.clock.now();
        if let Some(preview) = self.embedded_world_preview.as_mut() {
            let stats = TexturedSectionRenderStats {
                drawn_section_count: left_eye.summary.placed_drawn_section_count,
                drawn_index_count: left_eye.summary.placed_drawn_index_count,
                ..TexturedSectionRenderStats::default()
            };
            let (bounded_section_count, out_of_region_submission_count, cull_ms) =
                preview_stereo_draw
                    .as_ref()
                    .map_or((0, 0, 0.0), |prepared| {
                        (prepared.2, prepared.3, prepared.1.cull_ms)
                    });
            preview.record_render(
                bounded_section_count,
                out_of_region_submission_count,
                cull_ms,
                left_eye.timing.placed_draw_ms + right_eye.timing.placed_draw_ms,
                stats,
                preview_translucent_order.as_deref().map_or_else(
                    EmbeddedWorldPreviewTranslucentOrderSnapshot::default,
                    |order| {
                        embedded_translucent_order_snapshot(
                            active_world_id,
                            preview.source_world,
                            order,
                        )
                    },
                ),
            );
            if let Some((entities, remote_players, source_local_players, resources)) =
                preview_actor_receipt
            {
                preview.record_actor_render(
                    entities,
                    remote_players,
                    source_local_players,
                    left_eye.timing.placed_actor_ms + right_eye.timing.placed_actor_ms,
                    left_eye.summary.placed_actor_stats,
                    resources,
                    &preview_actor_instances
                        .as_ref()
                        .expect("preview actor receipt requires collected actors")
                        .entity_observations,
                    &preview_actor_instances
                        .as_ref()
                        .expect("preview actor receipt requires collected actors")
                        .remote_player_observations,
                    actor_rendered_at,
                );
            }
        }
        self.record_eye0_summary(left_eye.summary);
        self.record_warm_world_first_destination_frame(first_drawn_section_count, upload);
        self.record_embedded_world_activation_frame(first_drawn_section_count, upload, 2);
        Ok(self.frame_summary_with_timing(timing, upload))
    }

    fn live_upload_for_frame(
        &mut self,
        device: &wgpu::Device,
        center_position: Vec3,
        runtime_mode: XrTerrainRuntimeUpdateMode,
        frame_deadline: Option<MonotonicDeadline>,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadSummary> {
        if !matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live) {
            self.prefetched_live_upload = None;
            return Ok(self.frozen_runtime_upload_summary());
        }
        if self.active_world.local_startup.is_some() || self.active_world.runtime.is_none() {
            self.prefetched_live_upload = None;
            return Ok(self.frozen_runtime_upload_summary());
        }
        if let Some(upload) = self.prefetched_live_upload.take() {
            return Ok(upload);
        }
        let runtime_upload_start = self.services.clock.now();
        let upload =
            self.poll_runtime_and_upload(device, center_position, frame_deadline, timing)?;
        self.advance_external_runtime_startup_admission(center_position);
        timing.runtime_upload_ms =
            elapsed_ms(self.services.clock.elapsed_since(runtime_upload_start));
        Ok(upload)
    }

    fn advance_external_runtime_startup_admission(&mut self, _camera_position: Vec3) {
        if !self.active_world.external_runtime_startup_pending {
            return;
        }
        let camera_changed = {
            let Some(runtime) = self.active_world.runtime.as_mut() else {
                return;
            };
            match mclone_app_runtime::apply_pending_engine_camera_position_updates(
                runtime,
                &mut self.active_world.local_participant.camera,
                XR_CAMERA_COMMIT_CONTEXT,
            ) {
                Ok(changed) => changed,
                Err(error) => {
                    log::error!("apply authoritative active startup camera correction: {error:#}");
                    return;
                }
            }
        };
        if camera_changed {
            self.active_world.local_participant.reset_movement();
            // Observe a subsequent stable frame around the corrected camera
            // before admitting it as the active slot's retained entry.
            self.active_world.accepted_entry_pose = None;
            return;
        }
        let Some(runtime) = self.active_world.runtime.as_ref() else {
            return;
        };
        let eye = self.active_world.camera.snapshot().eye;
        let camera_position = Vec3::new(eye.x as f32, eye.y as f32, eye.z as f32);
        let traversal_ready = runtime
            .traversal_ready_render_section_keys(camera_position)
            .len();
        let evidence = mclone_app_runtime::StartupAdmissionEvidence {
            host_ready: runtime.startup_host_ready(
                mclone_app_runtime::StartupReadinessPolicy::Playable,
                camera_position,
            ),
            drawable_section_count: self.active_world.draw.section_count().min(traversal_ready),
        };
        if evidence.ready() {
            self.active_world.external_runtime_startup_pending = false;
            self.active_world.lifecycle = WorldSlotLifecycle::ActiveReady;
            self.active_world.accepted_entry_pose =
                Some(WorldEntryPose::from_camera(&self.active_world.camera));
        }
    }

    fn render_compile_frame_deadline(&self) -> Option<MonotonicDeadline> {
        self.render_admission_target_period_ms().map(|period_ms| {
            self.services
                .clock
                .deadline_after(Duration::from_secs_f64(period_ms / 1_000.0))
        })
    }

    fn render_admission_target_period_ms(&self) -> Option<f64> {
        self.display_refresh_hz
            .filter(|hz| hz.is_finite() && *hz > 0.0)
            .map(|hz| 1_000.0 / f64::from(hz))
    }

    fn next_per_view_uniform_frame(&mut self) -> u32 {
        let frame = self.per_view_uniform_frame;
        self.per_view_uniform_frame =
            (self.per_view_uniform_frame + 1) % PER_VIEW_UNIFORM_FRAME_COUNT;
        frame
    }

    fn terrain_render_views_and_options(
        &mut self,
        render_views: [ChunkRenderView; 2],
        sky_state: SkyRenderState,
    ) -> (
        [ChunkRenderView; 2],
        [TexturedSectionRenderOptions; 2],
        [Option<UnderwaterOverlay>; 2],
    ) {
        let center_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let base_options = self
            .effective_render_options(center_position, sky_state)
            .with_sky_darken(sky_state.sky_darken());
        let underwater_overlays = self.underwater_overlays(render_views);
        let terrain_views = [
            render_view_with_underwater_effect(render_views[0], underwater_overlays[0]),
            render_view_with_underwater_effect(render_views[1], underwater_overlays[1]),
        ];
        let terrain_options = underwater_overlays.map(|overlay| {
            let fog = overlay
                .map(|overlay| RenderFog::underwater_with_water_vision(overlay.water_vision))
                .unwrap_or(base_options.fog);
            base_options.with_fog(fog)
        });
        (terrain_views, terrain_options, underwater_overlays)
    }

    fn render_prepared_terrain_stereo_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainStereoFrameSummary> {
        self.render_prepared_terrain_stereo_frame_frozen_inner(
            device,
            queue,
            render_views,
            left_target,
            right_target,
            false,
            false,
        )
    }

    fn render_prepared_terrain_stereo_frame_frozen_inner(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
        include_sky: bool,
        include_actors: bool,
    ) -> Result<XrTerrainStereoFrameSummary> {
        self.poll_asset_replacement(device, queue)?;
        let sky_state = self.solar_render_state();
        let (terrain_views, mut terrain_options, _) =
            self.terrain_render_views_and_options(render_views, sky_state);
        let actor_instances = if include_actors {
            self.current_actor_instances()
        } else {
            Vec::new()
        };
        let center_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let terrain_view_enabled = self.prepare_terrain_view_for_frame(
            device,
            queue,
            [
                f64::from(center_position.x),
                f64::from(center_position.y),
                f64::from(center_position.z),
            ],
        )?;
        terrain_options = terrain_options
            .map(|options| render_options_with_actor_grass_interactors(options, &actor_instances));
        let prepared_records = self.active_world.draw.prepare_render_records();
        let prepared_stereo_draw = self.active_world.draw.prepare_stereo_draw(
            &prepared_records,
            terrain_views,
            terrain_options,
        );
        let uniform_frame = self.next_per_view_uniform_frame();
        let left_view_slot = LEFT_EYE_VIEW_SLOT.in_uniform_frame(uniform_frame);
        let right_view_slot = RIGHT_EYE_VIEW_SLOT.in_uniform_frame(uniform_frame);
        let left = self.render_terrain_eye_only_target(
            device,
            queue,
            &prepared_stereo_draw,
            left_target,
            terrain_views[0],
            terrain_options[0],
            &actor_instances,
            "left",
            left_view_slot,
            include_sky,
            include_actors,
            terrain_view_enabled,
            sky_state,
        )?;
        let right = self.render_terrain_eye_only_target(
            device,
            queue,
            &prepared_stereo_draw,
            right_target,
            terrain_views[1],
            terrain_options[1],
            &actor_instances,
            "right",
            right_view_slot,
            include_sky,
            include_actors,
            terrain_view_enabled,
            sky_state,
        )?;

        self.active_world.render_stats.drawn_section_count = left.drawn_section_count;
        self.active_world.render_stats.drawn_face_count = left.drawn_face_count();
        self.active_world.render_stats.drawn_index_count = left.drawn_index_count;
        self.active_world.render_stats.grass_resident_patch_count = left.grass_resident_patch_count;
        self.active_world.render_stats.grass_drawn_patch_count = left.grass_drawn_patch_count;
        self.active_world.render_stats.grass_estimated_blade_count =
            left.grass_estimated_blade_count;
        self.active_world.render_stats.grass_draw_calls = left.grass_draw_calls;
        self.active_world.render_stats.grass_resident_bytes = left.grass_resident_bytes;
        self.active_world.render_stats.grass_interaction_field_count =
            left.grass_interaction_field_count;
        self.active_world
            .render_stats
            .grass_interaction_active_cell_count = left.grass_interaction_active_cell_count;
        self.active_world.render_stats.grass_interaction_stamp_count =
            left.grass_interaction_stamp_count;
        self.active_world
            .render_stats
            .grass_interaction_recenter_count = left.grass_interaction_recenter_count;
        self.active_world.render_stats.grass_interaction_reset_count =
            left.grass_interaction_reset_count;
        self.active_world
            .render_stats
            .grass_interaction_uploaded_bytes = left.grass_interaction_uploaded_bytes;
        self.rendered_frames += 1;
        self.record_warm_world_first_destination_frame(
            left.drawn_section_count,
            XrTerrainUploadSummary::default(),
        );
        Ok(XrTerrainStereoFrameSummary {
            rendered_frames: self.rendered_frames,
            section_count: self.active_world.draw.section_count(),
            left,
            right,
        })
    }

    fn render_terrain_eye_only_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        target: XrTerrainEyeTarget<'_>,
        render_view: ChunkRenderView,
        render_options: TexturedSectionRenderOptions,
        actor_instances: &[ActorInstance],
        label: &'static str,
        view_slot: PerViewSlot,
        include_sky: bool,
        include_actors: bool,
        terrain_view_enabled: bool,
        sky_state: SkyRenderState,
    ) -> Result<TexturedSectionRenderStats> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(match label {
                "left" => "mclone_xr_terrain_only_left_eye_encoder",
                "right" => "mclone_xr_terrain_only_right_eye_encoder",
                _ => "mclone_xr_terrain_only_eye_encoder",
            }),
        });
        let mut render_target = ChunkRenderTarget::new(
            target.color_view,
            &target.depth.view,
            target.size,
            sky_state.clear_color(),
        );
        if include_sky {
            self.sky.render_in_slot(
                queue,
                &mut encoder,
                target.color_view,
                sky_state.clear_color(),
                render_view.sky_view_projection(),
                sky_state,
                view_slot,
            );
            render_target = render_target.with_loaded_color();
        }
        #[cfg(not(target_arch = "wasm32"))]
        let opaque_world_gate = self
            .opaque_world_gate_renderer
            .as_ref()
            .zip(self.world_gate.as_ref().map(WorldGate::render_gate));
        #[cfg(target_arch = "wasm32")]
        let opaque_world_gate: Option<(
            &mclone_render::opaque_world_gate::OpaqueWorldGateRenderer,
            mclone_render::opaque_world_gate::OpaqueWorldGate,
        )> = None;
        let split_translucent_terrain = (include_actors && !actor_instances.is_empty())
            || opaque_world_gate.is_some()
            || terrain_view_enabled;
        let terrain_phase = if split_translucent_terrain {
            TexturedSectionRenderPhase::Opaque
        } else {
            TexturedSectionRenderPhase::All
        };
        let stats = self
            .active_world
            .draw
            .render_prepared_stereo_draw_phase_with_options_in_slot(
                prepared_draw,
                queue,
                &mut encoder,
                render_target,
                render_view,
                render_options,
                view_slot,
                terrain_phase,
            )
            .context("render XR terrain-only chunks")?;
        if terrain_view_enabled {
            self.terrain_view
                .as_mut()
                .expect("enabled scene terrain view remains initialized")
                .render(TerrainBackdropRenderContext {
                    device,
                    queue,
                    encoder: &mut encoder,
                    color_view: target.color_view,
                    depth_view: &target.depth.view,
                    size: target.size,
                    render_view,
                    sky_darken: render_options.sky_darken,
                    fog: render_options.fog,
                    view_slot,
                })?;
        }
        if let Some((gate_renderer, gate)) = opaque_world_gate {
            gate_renderer.render_in_slot(
                queue,
                &mut encoder,
                RenderFrameTarget::color(target.color_view, target.size),
                target.depth,
                render_view,
                Some(gate),
                view_slot,
            );
        }
        if include_actors {
            self.active_world
                .actors
                .as_mut()
                .expect("active world owns actor draw state")
                .render_in_slot(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(target.color_view, target.size)
                        .with_depth(&target.depth.view),
                    render_view,
                    render_options,
                    actor_instances,
                    view_slot,
                )
                .context("render XR terrain-only actors")?;
        }
        if split_translucent_terrain {
            self.active_world
                .draw
                .render_prepared_stereo_draw_phase_with_options_in_slot(
                    prepared_draw,
                    queue,
                    &mut encoder,
                    render_target.with_loaded_color().with_loaded_depth(),
                    render_view,
                    render_options,
                    view_slot,
                    TexturedSectionRenderPhase::Translucent,
                )
                .context("render XR terrain-only translucent chunks")?;
        }
        let submission = queue.submit(Some(encoder.finish()));
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .map(|_| ())
            .context("wait for XR terrain-only submission")?;
        device
            .poll(wgpu::PollType::Wait)
            .map(|_| ())
            .context("wait for XR terrain-only device idle")?;
        Ok(stats)
    }

    fn render_prepared_terrain_multiview_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let center_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let mut timing = XrTerrainFrameTiming::default();
        let upload =
            if self.active_world.local_startup.is_none() && self.active_world.runtime.is_some() {
                let frame_deadline = self.render_compile_frame_deadline();
                self.poll_runtime_and_upload(device, center_position, frame_deadline, &mut timing)?
            } else {
                self.frozen_runtime_upload_summary()
            };

        self.render_prepared_terrain_multiview_frame_with_upload(
            device,
            queue,
            render_views,
            target,
            upload,
        )
    }

    fn render_prepared_terrain_multiview_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        self.render_prepared_terrain_multiview_frame_frozen_inner(
            device,
            queue,
            render_views,
            target,
            false,
            false,
            false,
        )
    }

    fn render_prepared_terrain_multiview_frame_frozen_inner(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        target: XrTerrainMultiviewTarget<'_>,
        include_sky: bool,
        include_actors: bool,
        include_overlays: bool,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        self.poll_asset_replacement(device, queue)?;
        self.render_prepared_terrain_multiview_frame_with_upload_inner(
            device,
            queue,
            render_views,
            target,
            XrTerrainUploadSummary::default(),
            include_sky,
            include_actors,
            include_overlays,
            false,
            None,
        )
    }

    fn render_prepared_terrain_multiview_frame_with_upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        target: XrTerrainMultiviewTarget<'_>,
        upload: XrTerrainUploadSummary,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        self.render_prepared_terrain_multiview_frame_with_upload_inner(
            device,
            queue,
            render_views,
            target,
            upload,
            false,
            false,
            false,
            false,
            None,
        )
    }

    fn render_prepared_terrain_multiview_frame_with_upload_inner(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_views: [ChunkRenderView; 2],
        target: XrTerrainMultiviewTarget<'_>,
        upload: XrTerrainUploadSummary,
        include_sky: bool,
        include_actors: bool,
        include_overlays: bool,
        terrain_view_enabled: bool,
        mut timing: Option<&mut XrTerrainFrameTiming>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let sky_state = self.solar_render_state();
        let (terrain_views, mut terrain_options, underwater_overlays) =
            self.terrain_render_views_and_options(render_views, sky_state);
        let actor_instances = if include_actors {
            self.current_actor_instances()
        } else {
            Vec::new()
        };
        let preview_actor_instances = include_actors
            .then(|| self.current_preview_actor_instances())
            .flatten();
        terrain_options = terrain_options
            .map(|options| render_options_with_actor_grass_interactors(options, &actor_instances));
        let sky_clear_color = sky_state.clear_color();
        let records_start = self.services.clock.now();
        let (prepared_records, record_cache_prepare) =
            self.active_world.draw.prepare_render_records_with_stats();
        if let Some(timing) = timing.as_deref_mut() {
            timing.shared_records_ms = elapsed_ms(self.services.clock.elapsed_since(records_start));
            timing.record_cache_prepare = record_cache_prepare;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_xr_terrain_multiview_encoder"),
        });
        let mut render_target = ChunkMultiviewRenderTarget::new(
            target.color_view,
            &target.depth.view,
            target.size,
            sky_clear_color,
        );
        if include_sky {
            let sky_start = self.services.clock.now();
            self.sky.render_multiview(
                device,
                queue,
                &mut encoder,
                target.color_view,
                sky_clear_color,
                [
                    terrain_views[0].sky_view_projection(),
                    terrain_views[1].sky_view_projection(),
                ],
                sky_state,
            )?;
            if let Some(timing) = timing.as_deref_mut() {
                timing.multiview_sky_ms = elapsed_ms(self.services.clock.elapsed_since(sky_start));
            }
            render_target = render_target.with_loaded_color();
        }
        let terrain_start = self.services.clock.now();
        let prepared_stereo_draw = self.active_world.draw.prepare_stereo_draw(
            &prepared_records,
            terrain_views,
            terrain_options,
        );
        #[cfg(not(target_arch = "wasm32"))]
        let opaque_world_gate = self
            .opaque_world_gate_renderer
            .as_ref()
            .zip(self.world_gate.as_ref().map(WorldGate::render_gate));
        #[cfg(target_arch = "wasm32")]
        let opaque_world_gate: Option<(
            &mclone_render::opaque_world_gate::OpaqueWorldGateRenderer,
            mclone_render::opaque_world_gate::OpaqueWorldGate,
        )> = None;
        let preview_frame = self
            .embedded_world_preview
            .as_ref()
            .filter(|preview| preview.phase == EmbeddedWorldPreviewPhase::Visible)
            .and_then(|preview| {
                self.standby_world
                    .as_ref()
                    .filter(|slot| slot.id == preview.source_world)
                    .map(|slot| {
                        let records = slot
                            .draw
                            .prepare_render_records_for_context(preview.context);
                        let options =
                            self.render_options
                                .with_sky_darken(sky_state.sky_darken())
                                .with_grass_time_seconds(terrain_options[0].grass_time_seconds)
                                .with_grass_interactors(
                                    preview_actor_instances.as_ref().map_or_else(
                                        GrassInteractorSet::default,
                                        |actors| {
                                            grass_interactors_from_actors(None, &actors.instances)
                                        },
                                    ),
                                )
                                .with_topology(slot.runtime.as_ref().map_or(
                                    mclone_core::HorizontalTopology::UNBOUNDED,
                                    |runtime| runtime.client().topology(),
                                ));
                        let bounded_section_count = records.section_keys().len();
                        let out_of_region_submission_count = records
                            .section_keys()
                            .filter(|key| !preview.region.contains(*key))
                            .count();
                        (
                            preview.source_world,
                            records,
                            [options; 2],
                            preview.context,
                            bounded_section_count,
                            out_of_region_submission_count,
                        )
                    })
            });
        let split_translucent_terrain = (include_actors && !actor_instances.is_empty())
            || opaque_world_gate.is_some()
            || preview_frame.is_some()
            || terrain_view_enabled;
        let terrain_phase = if split_translucent_terrain {
            TexturedSectionRenderPhase::Opaque
        } else {
            TexturedSectionRenderPhase::All
        };
        let stats = self
            .active_world
            .draw
            .render_prepared_multiview_stereo_draw_phase_with_options(
                &prepared_stereo_draw,
                device,
                queue,
                &mut encoder,
                render_target,
                terrain_views,
                terrain_options,
                terrain_phase,
            )
            .context("render XR terrain multiview chunks")?;
        if terrain_view_enabled {
            self.terrain_view
                .as_mut()
                .expect("enabled scene terrain view remains initialized")
                .render_multiview(
                    device,
                    queue,
                    &mut encoder,
                    target.color_view,
                    &target.depth.view,
                    target.size,
                    terrain_views,
                    terrain_options[0].sky_darken,
                    terrain_options[0].fog,
                )
                .context("render XR procedural horizon multiview")?;
        }
        if let Some(timing) = timing.as_deref_mut() {
            timing.multiview_terrain_ms =
                elapsed_ms(self.services.clock.elapsed_since(terrain_start));
        }
        let mut preview_translucent_frame = None;
        let preview_stats = if let Some((
            source_world,
            records,
            options,
            context,
            bounded_section_count,
            out_of_region_submission_count,
        )) = preview_frame
        {
            let preview = self
                .embedded_world_preview
                .as_ref()
                .filter(|preview| preview.source_world == source_world)
                .context("visible embedded preview lost its presentation state")?;
            let standby = self
                .standby_world
                .as_ref()
                .filter(|slot| slot.id == source_world)
                .context("visible embedded preview lost its source world")?;
            let (prepared, prepare_timing) = standby.draw.prepare_placed_stereo_draw_timed(
                &records,
                terrain_views,
                options,
                context,
            );
            let draw_started_at = self.services.clock.now();
            let stats = standby
                .draw
                .render_placed_prepared_multiview_stereo_draw_with_options(
                    &preview.renderer,
                    &prepared,
                    device,
                    queue,
                    &mut encoder,
                    render_target.with_loaded_color().with_loaded_depth(),
                    terrain_views,
                    options,
                    context,
                )
                .context("render embedded world preview multiview")?;
            let draw_ms = elapsed_ms(self.services.clock.elapsed_since(draw_started_at));
            preview_translucent_frame = Some((source_world, prepared, options, context));
            Some((
                stats,
                prepare_timing.cull_ms,
                draw_ms,
                bounded_section_count,
                out_of_region_submission_count,
            ))
        } else {
            None
        };
        if let Some((gate_renderer, gate)) = opaque_world_gate {
            gate_renderer
                .render_multiview(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(target.color_view, target.size),
                    target.depth,
                    terrain_views,
                    Some(gate),
                )
                .context("render opaque world gate multiview")?;
        }
        let actor_stats = if include_actors {
            let actor_start = self.services.clock.now();
            let actor_stats = self
                .active_world
                .actors
                .as_mut()
                .expect("active world owns actor draw state")
                .render_multiview(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(target.color_view, target.size)
                        .with_depth(&target.depth.view),
                    terrain_views,
                    terrain_options,
                    &actor_instances,
                )
                .context("render XR actor multiview pass")?;
            if let Some(timing) = timing.as_deref_mut() {
                timing.multiview_actor_ms =
                    elapsed_ms(self.services.clock.elapsed_since(actor_start));
            }
            actor_stats
        } else {
            ActorRenderStats::default()
        };
        let preview_actor_receipt = if let Some(actors) = preview_actor_instances.as_ref() {
            let preview = self
                .embedded_world_preview
                .as_ref()
                .filter(|preview| preview.source_world == actors.source_world)
                .context("preview actor source lost its presentation state")?;
            let standby = self
                .standby_world
                .as_mut()
                .filter(|slot| slot.id == actors.source_world)
                .context("preview actor source lost its world slot")?;
            let options = self.render_options.with_sky_darken(sky_state.sky_darken());
            let actor_start = self.services.clock.now();
            let resources = standby
                .actors
                .as_mut()
                .context("visible preview actor capability was not published")?;
            let stats = resources
                .render_composed_multiview(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(target.color_view, target.size)
                        .with_depth(&target.depth.view),
                    terrain_views,
                    [options; 2],
                    &actors.instances,
                    preview.context,
                )
                .context("render embedded world preview actors multiview")?;
            let draw_ms = elapsed_ms(self.services.clock.elapsed_since(actor_start));
            if let Some(timing) = timing.as_deref_mut() {
                timing.multiview_actor_ms += draw_ms;
            }
            Some((stats, resources.resource_snapshot(), draw_ms))
        } else {
            None
        };
        let mut preview_translucent_order_snapshot =
            EmbeddedWorldPreviewTranslucentOrderSnapshot::default();
        if split_translucent_terrain {
            let translucent_start = self.services.clock.now();
            if let Some((source_world, prepared, options, context)) =
                preview_translucent_frame.as_ref()
            {
                let preview = self
                    .embedded_world_preview
                    .as_ref()
                    .filter(|preview| preview.source_world == *source_world)
                    .context("visible embedded preview lost before translucent multiview")?;
                let standby = self
                    .standby_world
                    .as_ref()
                    .filter(|slot| slot.id == *source_world)
                    .context("embedded source world lost before translucent multiview")?;
                let order = compose_translucent_terrain_order(
                    self.active_world.id,
                    prepared_stereo_draw
                        .translucent_records(mclone_render::placement::WorldPlacement::identity()),
                    *source_world,
                    prepared.translucent_records(context.placement()),
                    &terrain_views,
                );
                preview_translucent_order_snapshot = embedded_translucent_order_snapshot(
                    self.active_world.id,
                    *source_world,
                    &order,
                );
                render_composed_translucent_terrain_multiview(
                    &self.active_world.draw,
                    &standby.draw,
                    &preview.renderer,
                    &order,
                    device,
                    queue,
                    &mut encoder,
                    render_target.with_loaded_color().with_loaded_depth(),
                    terrain_views,
                    terrain_options,
                    *options,
                    *context,
                )
                .context("render XR composed terrain multiview translucent chunks")?;
            } else {
                self.active_world
                    .draw
                    .render_prepared_multiview_stereo_draw_phase_with_options(
                        &prepared_stereo_draw,
                        device,
                        queue,
                        &mut encoder,
                        render_target.with_loaded_color().with_loaded_depth(),
                        terrain_views,
                        terrain_options,
                        TexturedSectionRenderPhase::Translucent,
                    )
                    .context("render XR terrain multiview translucent chunks")?;
            }
            if let Some(timing) = timing.as_deref_mut() {
                timing.multiview_terrain_ms +=
                    elapsed_ms(self.services.clock.elapsed_since(translucent_start));
            }
        }
        let mut ui_panel_stats = WorldGuiPanelRenderStats::default();
        let mut ui_draw_cache_stats = UiDrawCacheStats::default();
        if include_overlays {
            let screen_effect_start = self.services.clock.now();
            self.screen_effects
                .render_underwater_multiview(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(target.color_view, target.size),
                    underwater_overlays,
                )
                .context("render XR underwater screen effect multiview")?;
            self.screen_effects
                .render_fade_multiview(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(target.color_view, target.size),
                    xr_head_comfort_fade_overlays(self.head_comfort),
                )
                .context("render XR head comfort fade multiview")?;
            if let Some(timing) = timing.as_deref_mut() {
                timing.multiview_screen_effect_ms =
                    elapsed_ms(self.services.clock.elapsed_since(screen_effect_start));
            }
            let overlays_start = self.services.clock.now();
            let world_overlay_stats = self
                .render_xr_world_overlays_multiview(
                    device,
                    queue,
                    &mut encoder,
                    target,
                    render_views,
                )
                .context("render XR multiview world overlays")?;
            ui_panel_stats = world_overlay_stats.panel;
            ui_draw_cache_stats = world_overlay_stats.draw_cache;
            if let Some(timing) = timing.as_deref_mut() {
                timing.multiview_world_overlays_ms =
                    elapsed_ms(self.services.clock.elapsed_since(overlays_start));
            }
        }
        if let Some(overlay) = self.embedded_world_activation_fade_overlay() {
            self.screen_effects
                .render_fade_multiview(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(target.color_view, target.size),
                    [Some(overlay), Some(overlay)],
                )
                .context("render embedded-world activation fade multiview")?;
        }
        let submit_start = self.services.clock.now();
        let submission = queue.submit(Some(encoder.finish()));
        if let Some(timing) = timing.as_deref_mut() {
            timing.multiview_submit_ms =
                elapsed_ms(self.services.clock.elapsed_since(submit_start));
        }
        let poll_wait_start = self.services.clock.now();
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .map(|_| ())
            .context("wait for XR terrain multiview submission")?;
        device
            .poll(wgpu::PollType::Wait)
            .map(|_| ())
            .context("wait for XR terrain multiview device idle")?;
        if let Some(timing) = timing.as_deref_mut() {
            timing.multiview_poll_wait_ms =
                elapsed_ms(self.services.clock.elapsed_since(poll_wait_start));
        }

        let actor_rendered_at = self.services.clock.now();
        self.active_world.render_stats.drawn_section_count = stats[0].drawn_section_count;
        self.active_world.render_stats.drawn_face_count = stats[0].drawn_face_count();
        self.active_world.render_stats.drawn_index_count = stats[0].drawn_index_count;
        self.active_world.render_stats.grass_resident_patch_count =
            stats[0].grass_resident_patch_count;
        self.active_world.render_stats.grass_drawn_patch_count = stats[0].grass_drawn_patch_count;
        self.active_world.render_stats.grass_estimated_blade_count =
            stats[0].grass_estimated_blade_count;
        self.active_world.render_stats.grass_draw_calls = stats[0].grass_draw_calls;
        self.active_world.render_stats.grass_resident_bytes = stats[0].grass_resident_bytes;
        self.active_world.render_stats.grass_interaction_field_count =
            stats[0].grass_interaction_field_count;
        self.active_world
            .render_stats
            .grass_interaction_active_cell_count = stats[0].grass_interaction_active_cell_count;
        self.active_world.render_stats.grass_interaction_stamp_count =
            stats[0].grass_interaction_stamp_count;
        self.active_world
            .render_stats
            .grass_interaction_recenter_count = stats[0].grass_interaction_recenter_count;
        self.active_world.render_stats.grass_interaction_reset_count =
            stats[0].grass_interaction_reset_count;
        self.active_world
            .render_stats
            .grass_interaction_uploaded_bytes = stats[0].grass_interaction_uploaded_bytes;
        if let (Some(preview), Some((preview_stats, cull_ms, draw_ms, bounded, outside))) =
            (self.embedded_world_preview.as_mut(), preview_stats)
        {
            preview.record_render(
                bounded,
                outside,
                cull_ms,
                draw_ms,
                preview_stats[0],
                preview_translucent_order_snapshot,
            );
            if let (Some(actors), Some((stats, resources, actor_draw_ms))) =
                (preview_actor_instances.as_ref(), preview_actor_receipt)
            {
                preview.record_actor_render(
                    actors.entity_count,
                    actors.remote_player_count,
                    actors.source_local_player_count,
                    actor_draw_ms,
                    stats,
                    resources,
                    &actors.entity_observations,
                    &actors.remote_player_observations,
                    actor_rendered_at,
                );
            }
        }
        self.last_ui_panel_stats = ui_panel_stats;
        self.last_ui_draw_cache_stats = ui_draw_cache_stats;
        self.rendered_frames += 1;
        self.record_warm_world_first_destination_frame(stats[0].drawn_section_count, upload);
        self.record_embedded_world_activation_frame(stats[0].drawn_section_count, upload, 2);
        Ok(XrTerrainMultiviewFrameSummary {
            rendered_frames: self.rendered_frames,
            section_count: self.active_world.draw.section_count(),
            left: stats[0],
            right: stats[1],
            actor_count: actor_instances.len(),
            drawn_actor_count: actor_stats.drawn_actor_count,
            placed_actor_count: preview_actor_receipt
                .map_or(0, |(stats, _, _)| stats.submitted_actor_count),
            drawn_placed_actor_count: preview_actor_receipt
                .map_or(0, |(stats, _, _)| stats.drawn_actor_count),
            ui_panel: ui_panel_stats,
            ui_draw_cache: ui_draw_cache_stats,
            upload,
        })
    }

    fn render_xr_world_overlays_multiview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: XrTerrainMultiviewTarget<'_>,
        render_views: [ChunkRenderView; 2],
    ) -> Result<XrWorldOverlayStats> {
        let overlay_target = RenderFrameTarget::color(target.color_view, target.size);
        let selection_outline = self.current_xr_selection_outline();
        let lens_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let lens_mesh = if self.worldgen_lens.active_layer().is_some() {
            let topology = self
                .active_world
                .runtime
                .as_ref()
                .map_or(HorizontalTopology::UNBOUNDED, |runtime| {
                    runtime.client().topology()
                });
            let loaded_chunks = self
                .active_world
                .runtime
                .as_ref()
                .map(|runtime| {
                    runtime
                        .client()
                        .loaded_chunk_positions()
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .unwrap_or_default();
            self.worldgen_lens.prepare_mesh(
                self.active_world.scene.seed,
                self.active_world.scene.world_generation_profile,
                topology,
                lens_position,
                &loaded_chunks,
            )
        } else {
            None
        };
        self.worldgen_lens_renderer
            .render_multiview(
                device,
                queue,
                encoder,
                overlay_target,
                target.depth,
                render_views,
                lens_mesh,
            )
            .context("render XR worldgen lens multiview")?;
        self.selection_outline
            .render_multiview(
                device,
                queue,
                encoder,
                overlay_target,
                target.depth,
                render_views,
                selection_outline.as_ref(),
            )
            .context("render XR selection outline multiview")?;
        let mut world_lines = engine_debug_world_lines(
            &self.active_world.camera,
            EngineDebugVisualOptions::new(self.player_collision_box_visible),
        );
        if self.diagnostic_panel.debug_diagnostics_visible() {
            let observer =
                (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
            world_lines.extend(topology_debug_world_lines(
                self.active_world
                    .runtime
                    .as_ref()
                    .map_or(HorizontalTopology::UNBOUNDED, |runtime| {
                        runtime.client().topology()
                    }),
                observer,
            ));
        }
        if let Some(gameplay_ray) = self
            .xr_gameplay_controller_ray_line()
            .context("build XR gameplay controller ray visual")?
        {
            world_lines.push(gameplay_ray);
        }
        world_lines.extend(self.xr_blink_teleport_lines());
        if !world_lines.is_empty() {
            self.world_gui_renderer
                .render_lines_multiview(
                    device,
                    queue,
                    encoder,
                    overlay_target,
                    render_views,
                    &world_lines,
                )
                .context("render XR world debug lines multiview")?;
        }
        let mut panel_stats = WorldGuiPanelRenderStats::default();
        let mut draw_cache_stats = UiDrawCacheStats::default();
        let diagnostic_panel = xr_diagnostic_panel_from_render_views(render_views);
        self.refresh_debug_diagnostics_overlay();
        let (diagnostic_panel_stats, diagnostic_draw_cache) = self
            .diagnostic_panel
            .render_multiview(
                device,
                queue,
                encoder,
                overlay_target,
                render_views,
                diagnostic_panel,
            )
            .context("render XR diagnostic panel multiview")?;
        panel_stats.add(diagnostic_panel_stats);
        draw_cache_stats.add(diagnostic_draw_cache);
        let field_guide_notification = self
            .active_world
            .visible_field_guide_notification(self.services.clock.now());
        if !self.ui.is_active() {
            let draw = field_guide_notification.map_or_else(GuiDrawList::new, |progress| {
                xr_field_guide_draw(
                    progress.mallard,
                    progress.deer,
                    progress.bee,
                    progress.rabbit,
                )
            });
            if !draw.commands().is_empty() {
                panel_stats.add(
                    self.world_gui_overlay_renderer
                        .render_panel_multiview_report(
                            device,
                            queue,
                            encoder,
                            overlay_target,
                            render_views,
                            XR_FIELD_GUIDE_PANEL_PIXELS,
                            [
                                XR_FIELD_GUIDE_PANEL_PIXELS[0] as f32,
                                XR_FIELD_GUIDE_PANEL_PIXELS[1] as f32,
                            ],
                            &draw,
                            xr_field_guide_panel_from_render_views(render_views),
                            &[],
                        )
                        .context("render XR field-guide panel multiview")?,
                );
            }
            return Ok(XrWorldOverlayStats {
                panel: panel_stats,
                draw_cache: draw_cache_stats,
            });
        }
        let Some(panel) = self.menu_panel_pose else {
            return Ok(XrWorldOverlayStats {
                panel: panel_stats,
                draw_cache: draw_cache_stats,
            });
        };
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        let ui_state = self.current_ui_render_state();
        let session_projection = self.session_projection();
        let panel_draw = prepare_xr_menu_panel_draw(
            &mut self.ui,
            &mut self.menu_overlay_cache,
            gui_scale,
            ui_state,
            session_projection.loading_progress_overlay.as_ref(),
            &session_projection.status_overlay,
        );
        let controller_ray_lines = self
            .xr_menu_controller_ray_lines(panel)
            .context("build XR menu controller ray visuals")?;
        let mut menu_panel_stats = if let Some(cache_revision) = panel_draw.cache_revision {
            self.world_gui_renderer.render_panel_cached_multiview(
                device,
                queue,
                encoder,
                overlay_target,
                render_views,
                XR_MENU_PANEL_PIXELS,
                [gui_scale.width, gui_scale.height],
                &panel_draw.panel_draw,
                panel,
                if panel_draw.overlay_draw.commands().is_empty() {
                    &controller_ray_lines
                } else {
                    &[]
                },
                cache_revision,
            )
        } else {
            self.world_gui_renderer.render_panel_multiview_report(
                device,
                queue,
                encoder,
                overlay_target,
                render_views,
                XR_MENU_PANEL_PIXELS,
                [gui_scale.width, gui_scale.height],
                &panel_draw.panel_draw,
                panel,
                if panel_draw.overlay_draw.commands().is_empty() {
                    &controller_ray_lines
                } else {
                    &[]
                },
            )
        }
        .context("render XR menu panel multiview")?;
        if !panel_draw.overlay_draw.commands().is_empty() {
            let overlay_stats = if let Some(cache_revision) = panel_draw.overlay_cache_revision {
                self.world_gui_overlay_renderer
                    .render_panel_cached_multiview(
                        device,
                        queue,
                        encoder,
                        overlay_target,
                        render_views,
                        XR_MENU_PANEL_PIXELS,
                        [gui_scale.width, gui_scale.height],
                        &panel_draw.overlay_draw,
                        panel,
                        &controller_ray_lines,
                        cache_revision,
                    )
            } else {
                self.world_gui_overlay_renderer
                    .render_panel_multiview_report(
                        device,
                        queue,
                        encoder,
                        overlay_target,
                        render_views,
                        XR_MENU_PANEL_PIXELS,
                        [gui_scale.width, gui_scale.height],
                        &panel_draw.overlay_draw,
                        panel,
                        &controller_ray_lines,
                    )
            }
            .context("render XR menu panel overlay multiview")?;
            menu_panel_stats.add(overlay_stats);
        }
        panel_stats.add(menu_panel_stats);
        draw_cache_stats.add(panel_draw.draw_cache);
        Ok(XrWorldOverlayStats {
            panel: panel_stats,
            draw_cache: draw_cache_stats,
        })
    }

    fn refresh_world_slot_traversal_ready_sections(
        slot: &mut DrawableWorldSlot,
        clock: &MonotonicClockHandle,
        camera_position: Vec3,
        upload_backpressured: bool,
        skip_refresh: bool,
        timing: &mut XrTerrainFrameTiming,
    ) -> usize {
        let ready_start = clock.now();
        if skip_refresh {
            timing.runtime_ready_sections_ms = elapsed_ms(clock.elapsed_since(ready_start));
            let ready_publish_start = clock.now();
            slot.draw
                .record_traversal_ready_sections_skipped(upload_backpressured);
            timing.runtime_ready_publish_ms = elapsed_ms(clock.elapsed_since(ready_publish_start));
            return slot.draw.traversal_ready_section_count();
        }
        let draw_section_generation = slot.draw.traversal_ready_source_generation();
        let refresh = {
            let runtime = slot
                .runtime
                .as_ref()
                .expect("runtime presence checked before ready refresh");
            slot.traversal_ready_sections.refresh(
                runtime.core(),
                camera_position,
                draw_section_generation,
            )
        };
        timing.runtime_ready_sections_ms = elapsed_ms(clock.elapsed_since(ready_start));
        let ready_publish_start = clock.now();
        if refresh.refreshed {
            slot.draw.set_traversal_ready_columns_with_context(
                slot.traversal_ready_sections.ready_columns(),
                upload_backpressured,
            );
        } else {
            slot.draw
                .record_traversal_ready_sections_skipped(upload_backpressured);
        }
        timing.runtime_ready_publish_ms = elapsed_ms(clock.elapsed_since(ready_publish_start));
        refresh.section_count
    }

    fn poll_runtime_and_upload(
        &mut self,
        device: &wgpu::Device,
        camera_position: Vec3,
        frame_deadline: Option<MonotonicDeadline>,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadSummary> {
        self.reconcile_grass_compile_policy();
        if self.defer_embedded_world_destination_preparation() {
            let upload = self.frozen_runtime_upload_summary();
            self.advance_warm_world_gpu(device, camera_position, frame_deadline)?;
            self.synchronize_world_gate_state();
            return Ok(upload);
        }
        let first_frame_after_warm_world_selection =
            self.last_warm_world_switch.as_ref().is_some_and(|report| {
                report.destination_instance_id == self.active_world.id
                    && report.first_drawable_destination_frame.is_none()
            });
        let selection_budget = |configured: Option<usize>| {
            first_frame_after_warm_world_selection
                .then(|| configured.unwrap_or(1).min(1))
                .or(configured)
        };
        let policy = WorldPreparationPolicy {
            clock: self.services.clock.clone(),
            target_period_ms: self.render_admission_target_period_ms(),
            poll_budget: RuntimeUpdatePumpBudget::unlimited(),
            upload_budget: selection_budget(self.render_section_upload_budget),
            accept_budget: selection_budget(self.render_section_accept_budget),
            completed_result_accept_budget: selection_budget(
                self.render_completed_result_accept_budget,
            ),
            max_compile_requests: first_frame_after_warm_world_selection.then_some(1),
            work_elapsed_budget: first_frame_after_warm_world_selection
                .then_some(Duration::from_micros(750)),
            defer_sync_after_pre_drain: false,
        };
        let standby_deadline = frame_deadline.clone();
        let upload = Self::prepare_world_slot(
            &mut self.active_world,
            device,
            camera_position,
            frame_deadline,
            &policy,
            timing,
        )?;
        self.play_confirmed_interaction_sounds();
        self.play_mallard_calls_and_update_tracks();
        self.sync_player_lifecycle_ui();
        self.advance_warm_world_gpu(device, camera_position, standby_deadline)?;
        self.synchronize_world_gate_state();
        Ok(upload)
    }

    fn reconcile_grass_compile_policy(&mut self) {
        let enabled = self.render_options.grass_detail.enabled();
        if let Some(runtime) = self.active_world.runtime.as_mut() {
            runtime.set_grass_patches_enabled(enabled);
        }
        if let Some(runtime) = self
            .standby_world
            .as_mut()
            .and_then(|slot| slot.runtime.as_mut())
        {
            runtime.set_grass_patches_enabled(enabled);
        }
    }

    fn sync_player_lifecycle_ui(&mut self) {
        let death_cause = self
            .active_world
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.client().player_death_cause());
        let death_screen = death_cause.map(|cause| GameScreen::Death {
            cause: match cause {
                mclone_protocol::PlayerDamageCause::Lava => GameDeathCause::Lava,
            },
        });
        match (death_screen, self.ui.screen()) {
            (Some(screen), current) if current != Some(screen) => self.ui.set_screen(Some(screen)),
            (None, Some(GameScreen::Death { .. })) => {
                self.ui.close();
                self.clear_menu_input_state();
            }
            _ => {}
        }
    }

    fn prepare_world_slot(
        slot: &mut DrawableWorldSlot,
        device: &wgpu::Device,
        camera_position: Vec3,
        frame_deadline: Option<MonotonicDeadline>,
        policy: &WorldPreparationPolicy,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadSummary> {
        let pending_render_count_start = policy.clock.now();
        let (
            pending_render_chunks_before,
            pending_compile_jobs_before,
            max_pending_compile_jobs,
            available_compile_slots_before,
        ) = if let Some(runtime) = slot.runtime.as_ref() {
            (
                runtime.pending_render_chunk_count(),
                runtime.render_compile_pending_job_count(),
                runtime.render_compile_max_pending_job_count(),
                runtime.render_compile_available_pending_job_slots(),
            )
        } else {
            timing.runtime_pending_render_count_ms +=
                elapsed_ms(policy.clock.elapsed_since(pending_render_count_start));
            let upload_queue = slot.section_uploads.stats();
            return Ok(XrTerrainUploadSummary {
                host_mode: XrTerrainHostMode::LocalIntegrated,
                queued_upload_section_count: upload_queue.queued_upload_sections,
                queued_upload_removed_section_count: upload_queue.queued_removed_sections,
                queued_upload_lifecycle_item_count: upload_queue.queued_lifecycle_items,
                queued_upload_mesh_owned_bytes: upload_queue.queued_upload_mesh_owned_bytes,
                upload_held_lifecycle_item_count: upload_queue.held_release_lifecycle_items,
                upload_held_compile_job_count: upload_queue.held_compile_jobs,
                traversal_ready_section_count: slot.draw.traversal_ready_section_count(),
                record_cache: slot.draw.record_cache_stats(),
                ..XrTerrainUploadSummary::default()
            });
        };
        timing.runtime_pending_render_count_ms +=
            elapsed_ms(policy.clock.elapsed_since(pending_render_count_start));
        let poll_start = policy.clock.now();
        let poll_changed = slot
            .runtime
            .as_mut()
            .expect("runtime presence checked before poll")
            .poll_with_update_budget(policy.poll_budget)
            .context("poll prepared world runtime")?;
        timing.runtime_poll_ms = elapsed_ms(policy.clock.elapsed_since(poll_start));
        let (poll_summary, has_runtime_render_work) = {
            let runtime = slot
                .runtime
                .as_ref()
                .expect("runtime presence checked before poll");
            (
                xr_poll_diagnostics_upload_summary(
                    runtime.host_mode().into(),
                    runtime.last_poll_diagnostics(),
                ),
                runtime.has_pending_render_work(camera_position),
            )
        };
        let target_period_ms = policy.target_period_ms;
        let budget_host_mode = match slot
            .runtime
            .as_ref()
            .expect("runtime presence checked before budget decision")
            .host_mode()
            .into()
        {
            XrTerrainHostMode::LocalIntegrated => BudgetHostMode::LocalIntegrated,
            XrTerrainHostMode::RemoteDedicated => BudgetHostMode::RemoteHost,
        };
        let render_admission_grant = slot.render_admission_policy.decide(
            target_period_ms,
            budget_host_mode,
            policy.upload_budget,
        );
        if !poll_changed && !has_runtime_render_work && !slot.section_uploads.has_pending_work() {
            let traversal_ready_section_count = Self::refresh_world_slot_traversal_ready_sections(
                slot,
                &policy.clock,
                camera_position,
                false,
                false,
                timing,
            );
            let runtime = slot
                .runtime
                .as_ref()
                .expect("runtime presence checked before poll");
            let compile_health = runtime.render_compile_queue_health();
            let upload_queue = slot.section_uploads.stats();
            let pending_render_count_start = policy.clock.now();
            let pending_render_chunks_after = runtime.pending_render_chunk_count();
            timing.runtime_pending_render_count_ms +=
                elapsed_ms(policy.clock.elapsed_since(pending_render_count_start));
            return Ok(XrTerrainUploadSummary {
                poll_changed,
                pending_render_chunks_before,
                pending_render_chunks_after,
                pending_compile_jobs_before,
                pending_compile_jobs_after: runtime.render_compile_pending_job_count(),
                max_pending_compile_jobs,
                available_compile_slots_before,
                available_compile_slots_after: runtime.render_compile_available_pending_job_slots(),
                dispatcher_compile_worker_count: compile_health.compile_worker_count,
                dispatcher_completed_compile_tasks: compile_health.completed_compile_tasks,
                dispatcher_total_compile_worker_busy_ms: micros_to_ms(
                    compile_health.total_compile_worker_busy_us,
                ),
                dispatcher_max_compile_worker_task_ms: micros_to_ms(
                    compile_health.max_compile_worker_task_us,
                ),
                queued_completed_compile_result_count: runtime
                    .pending_completed_compile_result_count(),
                queued_upload_section_count: upload_queue.queued_upload_sections,
                queued_upload_removed_section_count: upload_queue.queued_removed_sections,
                queued_upload_lifecycle_item_count: upload_queue.queued_lifecycle_items,
                queued_upload_mesh_owned_bytes: upload_queue.queued_upload_mesh_owned_bytes,
                upload_held_lifecycle_item_count: upload_queue.held_release_lifecycle_items,
                upload_held_compile_job_count: upload_queue.held_compile_jobs,
                traversal_ready_section_count,
                record_cache: slot.draw.record_cache_stats(),
                ..poll_summary
            });
        }
        let upload_frame_policy =
            RenderSectionUploadFramePolicy::new(policy.upload_budget, policy.accept_budget);
        let mut upload_report = TexturedSectionUploadReport::default();
        let mut upload_phase = RenderSectionUploadPhaseReport::default();
        let drained_pending_uploads_before_sync = slot
            .section_uploads
            .should_drain_before_runtime_sync(upload_frame_policy);
        if drained_pending_uploads_before_sync {
            let upload_start = policy.clock.now();
            let drained_report = Self::apply_world_slot_section_update_uploads(
                slot,
                device,
                RenderSectionCacheUpdate::default(),
                false,
                policy,
                timing,
            )?;
            let upload_elapsed_ms = elapsed_ms(policy.clock.elapsed_since(upload_start));
            timing.runtime_gpu_upload_ms += upload_elapsed_ms;
            timing.runtime_gpu_upload_pre_sync_ms += upload_elapsed_ms;
            accumulate_upload_report(&mut upload_report, drained_report.upload);
            upload_phase.absorb(drained_report.phase);
            Self::release_world_slot_render_compile_jobs(slot, drained_report.release_compile_jobs);
        }
        let runtime_work_requested = poll_changed || has_runtime_render_work;
        let upload_frame_decision = slot.section_uploads.frame_decision_after_pre_sync_drain(
            upload_frame_policy,
            runtime_work_requested,
            drained_pending_uploads_before_sync,
        );
        let should_sync_render_sections = upload_frame_decision.should_sync_render_sections
            && !(policy.defer_sync_after_pre_drain && drained_pending_uploads_before_sync);
        let sync_start = policy.clock.now();
        let timed_section_update = if should_sync_render_sections {
            if let Some(grant) = render_admission_grant {
                let admission_deadline = policy.clock.deadline_after(grant.elapsed_budget);
                let mut deadline = frame_deadline
                    .map(|frame_deadline| frame_deadline.earlier(admission_deadline.clone()))
                    .unwrap_or(admission_deadline);
                if let Some(work_elapsed_budget) = policy.work_elapsed_budget {
                    deadline = deadline.earlier(policy.clock.deadline_after(work_elapsed_budget));
                }
                let max_compile_requests = policy
                    .max_compile_requests
                    .map_or(grant.max_compile_requests, |limit| {
                        limit.min(grant.max_compile_requests)
                    });
                slot.runtime
                    .as_mut()
                    .expect("runtime presence checked before section sync")
                    .sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
                        camera_position,
                        deadline,
                        max_compile_requests,
                        policy.completed_result_accept_budget,
                    )
                    .context("sync prepared world render sections with adaptive admission")?
            } else if let Some(max_compile_requests) = policy.max_compile_requests {
                let work_deadline = policy.clock.deadline_after(
                    policy
                        .work_elapsed_budget
                        .expect("bounded compile admission requires an elapsed budget"),
                );
                let deadline = frame_deadline
                    .map(|frame_deadline| frame_deadline.earlier(work_deadline.clone()))
                    .unwrap_or(work_deadline);
                slot.runtime
                    .as_mut()
                    .expect("runtime presence checked before section sync")
                    .sync_render_sections_until_deadline_with_admission_budget_and_completed_result_acceptance_timed(
                        camera_position,
                        deadline,
                        max_compile_requests,
                        policy.completed_result_accept_budget,
                    )
                    .context("sync prepared world render sections with bounded admission")?
            } else if let Some(deadline) = frame_deadline {
                slot.runtime
                    .as_mut()
                    .expect("runtime presence checked before section sync")
                    .sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
                        camera_position,
                        deadline,
                        policy.completed_result_accept_budget,
                    )
                    .context("sync prepared world render sections until deadline")?
            } else {
                slot.runtime
                    .as_mut()
                    .expect("runtime presence checked before section sync")
                    .sync_render_sections_with_completed_result_acceptance_timed(
                        camera_position,
                        policy.completed_result_accept_budget,
                    )
                    .context("sync prepared world render sections")?
            }
        } else {
            mclone_app_runtime::TimedRenderSectionCacheUpdate::default()
        };
        slot.render_admission_policy
            .observe_sync(target_period_ms, &timed_section_update);
        timing.runtime_sync_ms = elapsed_ms(policy.clock.elapsed_since(sync_start));
        timing.runtime_result_accept_ms = timed_section_update.timing.completed_result_accept_ms;
        timing.runtime_dirty_seed_ms = timed_section_update.timing.dirty_seed_ms;
        timing.runtime_prepare_ms = timed_section_update.timing.prepare_ms;
        timing.runtime_submit_ms = timed_section_update.timing.submit_ms;
        timing.runtime_sync_unattributed_ms = (timing.runtime_sync_ms
            - timing.runtime_result_accept_ms
            - timing.runtime_dirty_seed_ms
            - timing.runtime_prepare_ms
            - timing.runtime_submit_ms)
            .max(0.0);
        timing.runtime_submit_snapshot_ms = timed_section_update.timing.submit_snapshot_ms;
        timing.runtime_submit_handoff_ms = timed_section_update.timing.submit_handoff_ms;
        timing.runtime_submit_handoff_worst_ms =
            timed_section_update.timing.submit_handoff_worst_ms;
        timing.runtime_submit_request_count = timed_section_update.timing.submit_request_count;
        timing.runtime_submit_request_build_ms =
            timed_section_update.timing.submit_request_build_ms;
        timing.runtime_submit_compiler_ms = timed_section_update.timing.submit_compiler_ms;
        timing.runtime_submit_compiler_worst_ms =
            timed_section_update.timing.submit_compiler_worst_ms;
        timing.runtime_submit_compiler_capacity_check_ms = timed_section_update
            .timing
            .submit_compiler_capacity_check_ms;
        timing.runtime_submit_compiler_capacity_check_worst_ms = timed_section_update
            .timing
            .submit_compiler_capacity_check_worst_ms;
        timing.runtime_submit_compiler_command_send_ms =
            timed_section_update.timing.submit_compiler_command_send_ms;
        timing.runtime_submit_compiler_command_send_worst_ms = timed_section_update
            .timing
            .submit_compiler_command_send_worst_ms;
        timing.runtime_submit_compiler_command_lock_wait_ms = timed_section_update
            .timing
            .submit_compiler_command_lock_wait_ms;
        timing.runtime_submit_compiler_command_lock_wait_worst_ms = timed_section_update
            .timing
            .submit_compiler_command_lock_wait_worst_ms;
        timing.runtime_submit_compiler_command_slot_select_ms = timed_section_update
            .timing
            .submit_compiler_command_slot_select_ms;
        timing.runtime_submit_compiler_command_slot_select_worst_ms = timed_section_update
            .timing
            .submit_compiler_command_slot_select_worst_ms;
        timing.runtime_submit_compiler_command_slot_write_ms = timed_section_update
            .timing
            .submit_compiler_command_slot_write_ms;
        timing.runtime_submit_compiler_command_slot_write_worst_ms = timed_section_update
            .timing
            .submit_compiler_command_slot_write_worst_ms;
        timing.runtime_submit_compiler_command_queue_push_ms = timed_section_update
            .timing
            .submit_compiler_command_queue_push_ms;
        timing.runtime_submit_compiler_command_queue_push_worst_ms = timed_section_update
            .timing
            .submit_compiler_command_queue_push_worst_ms;
        timing.runtime_submit_compiler_command_notify_ms = timed_section_update
            .timing
            .submit_compiler_command_notify_ms;
        timing.runtime_submit_compiler_command_notify_worst_ms = timed_section_update
            .timing
            .submit_compiler_command_notify_worst_ms;
        timing.runtime_submit_compiler_command_post_enqueue_ms = timed_section_update
            .timing
            .submit_compiler_command_post_enqueue_ms;
        timing.runtime_submit_compiler_command_post_enqueue_worst_ms = timed_section_update
            .timing
            .submit_compiler_command_post_enqueue_worst_ms;
        timing.runtime_submit_compiler_pending_mark_ms =
            timed_section_update.timing.submit_compiler_pending_mark_ms;
        timing.runtime_submit_compiler_pending_mark_worst_ms = timed_section_update
            .timing
            .submit_compiler_pending_mark_worst_ms;
        timing.runtime_submit_mark_inflight_ms =
            timed_section_update.timing.submit_mark_inflight_ms;
        timing.runtime_submit_apply_ready_plan_ms =
            timed_section_update.timing.submit_apply_ready_plan_ms;
        timing.runtime_submit_ready_update_ms = timed_section_update.timing.submit_ready_update_ms;
        timing.runtime_submit_ready_section_count =
            timed_section_update.timing.submit_ready_section_count;
        timing.runtime_submit_deferred_section_count =
            timed_section_update.timing.submit_deferred_section_count;
        timing.runtime_submit_dirty_chunk_count_before =
            timed_section_update.timing.submit_dirty_chunk_count_before;
        timing.runtime_submit_dirty_chunk_count_after =
            timed_section_update.timing.submit_dirty_chunk_count_after;
        timing.runtime_submit_dirty_section_count_before = timed_section_update
            .timing
            .submit_dirty_section_count_before;
        timing.runtime_submit_dirty_section_count_after =
            timed_section_update.timing.submit_dirty_section_count_after;
        timing.runtime_submit_inflight_section_count_before = timed_section_update
            .timing
            .submit_inflight_section_count_before;
        timing.runtime_submit_inflight_section_count_after = timed_section_update
            .timing
            .submit_inflight_section_count_after;
        timing.runtime_submit_request_target_section_count = timed_section_update
            .timing
            .submit_request_target_section_count;
        timing.runtime_submit_request_target_section_count_worst = timed_section_update
            .timing
            .submit_request_target_section_count_worst;
        timing.runtime_submit_request_snapshot_count =
            timed_section_update.timing.submit_request_snapshot_count;
        timing.runtime_submit_request_snapshot_section_count = timed_section_update
            .timing
            .submit_request_snapshot_section_count;
        timing.runtime_submit_request_snapshot_section_count_worst = timed_section_update
            .timing
            .submit_request_snapshot_section_count_worst;
        timing.runtime_submit_request_light_section_count = timed_section_update
            .timing
            .submit_request_light_section_count;
        timing.runtime_submit_request_light_section_count_worst = timed_section_update
            .timing
            .submit_request_light_section_count_worst;
        timing.runtime_submit_request_revision_count =
            timed_section_update.timing.submit_request_revision_count;
        timing.runtime_submit_request_estimated_payload_bytes = timed_section_update
            .timing
            .submit_request_estimated_payload_bytes;
        timing.runtime_submit_request_estimated_payload_bytes_worst = timed_section_update
            .timing
            .submit_request_estimated_payload_bytes_worst;
        timing.runtime_dispatcher_pending_jobs =
            timed_section_update.timing.dispatcher_pending_jobs;
        timing.runtime_dispatcher_max_pending_jobs =
            timed_section_update.timing.dispatcher_max_pending_jobs;
        timing.runtime_dispatcher_available_job_slots =
            timed_section_update.timing.dispatcher_available_job_slots;
        timing.runtime_dispatcher_queued_compile_tasks =
            timed_section_update.timing.dispatcher_queued_compile_tasks;
        timing.runtime_dispatcher_compile_worker_count =
            timed_section_update.timing.dispatcher_compile_worker_count;
        timing.runtime_dispatcher_completed_compile_tasks = timed_section_update
            .timing
            .dispatcher_completed_compile_tasks;
        timing.runtime_dispatcher_total_compile_worker_busy_ms = timed_section_update
            .timing
            .dispatcher_total_compile_worker_busy_ms;
        timing.runtime_dispatcher_max_compile_worker_task_ms = timed_section_update
            .timing
            .dispatcher_max_compile_worker_task_ms;
        let section_sync_timing = timed_section_update.timing;
        let section_update = timed_section_update.cache_update;
        let rebuilt_section_count = section_update.rebuilt_section_count();
        let removed_section_count = section_update.removed_section_count();
        let runtime_stats = slot
            .runtime
            .as_ref()
            .expect("runtime presence checked before section sync")
            .stats();
        let target_distance = i32::try_from(runtime_stats.render_distance).unwrap_or(i32::MAX);
        let in_target = |key: mclone_mesh::RenderSectionKey| {
            (key.chunk_x - runtime_stats.interest_center.x)
                .abs()
                .max((key.chunk_z - runtime_stats.interest_center.z).abs())
                <= target_distance
        };
        let target_rebuilt_section_count = section_update
            .rebuilt_sections
            .iter()
            .filter(|section| in_target(section.key))
            .count();
        let target_removed_section_count = section_update
            .removed_section_keys
            .iter()
            .filter(|key| in_target(**key))
            .count();
        let rebuilt_vertex_count = section_update.rebuilt_vertex_count;
        let rebuilt_index_count = section_update.rebuilt_index_count;
        let neighbor_ready_section_count = section_update.neighbor_ready_section_count;
        let near_exception_section_count = section_update.near_exception_section_count;
        let deferred_section_count = section_update.deferred_section_count;
        let submitted_compile_section_count = section_update.submitted_compile_section_count;
        let deadline_skipped_compile_request_count =
            section_update.deadline_skipped_compile_request_count;
        let accepted_compile_result_count = section_update.accepted_compile_result_count;
        let queued_completed_compile_result_count = if should_sync_render_sections {
            section_update.queued_completed_compile_result_count
        } else {
            slot.runtime.as_ref().map_or(0, |runtime| {
                runtime.pending_completed_compile_result_count()
            })
        };
        let completed_compile_section_count = section_update.completed_compile_section_count;
        let stale_compile_section_count = section_update.stale_compile_section_count;
        let mut pending_compile_jobs_after_sync = if should_sync_render_sections {
            section_update.pending_compile_jobs
        } else {
            slot.runtime
                .as_ref()
                .map_or(0, |runtime| runtime.render_compile_pending_job_count())
        };
        let visibility_graph_build_count = section_update.visibility_graph_stats.build_count;
        let visibility_graph_total_ms = section_update.visibility_graph_stats.total_ms;
        let visibility_graph_worst_ms = section_update.visibility_graph_stats.worst_ms;
        let resident_mesh_stats = section_update.resident_mesh_stats;
        if upload_frame_decision.should_apply_section_update_after_sync {
            let upload_start = policy.clock.now();
            let section_update_report = Self::apply_world_slot_section_update_uploads(
                slot,
                device,
                section_update,
                upload_frame_decision.upload_backpressured,
                policy,
                timing,
            )?;
            let upload_elapsed_ms = elapsed_ms(policy.clock.elapsed_since(upload_start));
            timing.runtime_gpu_upload_ms += upload_elapsed_ms;
            timing.runtime_gpu_upload_post_sync_ms += upload_elapsed_ms;
            accumulate_upload_report(&mut upload_report, section_update_report.upload);
            upload_phase.absorb(section_update_report.phase);
            Self::release_world_slot_render_compile_jobs(
                slot,
                section_update_report.release_compile_jobs,
            );
            pending_compile_jobs_after_sync = slot
                .runtime
                .as_ref()
                .map_or(0, |runtime| runtime.render_compile_pending_job_count());
        }
        // Removal-only drains already remove their keys from the draw ready set.
        // While the upload queue is still backpressured, avoid a full
        // traversal-ready recompute/publish until uploads or runtime work
        // introduce new ready candidates.
        let skip_ready_refresh = upload_frame_decision.upload_backpressured
            && upload_report.uploaded_section_count == 0
            && upload_report.removed_section_count > 0;
        let traversal_ready_section_count = Self::refresh_world_slot_traversal_ready_sections(
            slot,
            &policy.clock,
            camera_position,
            upload_frame_decision.upload_backpressured,
            skip_ready_refresh,
            timing,
        );
        let runtime = slot
            .runtime
            .as_ref()
            .expect("runtime presence checked before section sync");
        let compile_health = runtime.render_compile_queue_health();
        slot.render_stats.section_count = slot.draw.section_count();
        slot.render_stats.index_count = slot.draw.index_count();
        slot.render_stats.face_count = quad_face_count_from_indices(slot.render_stats.index_count);
        slot.render_stats.last_rebuilt_section_count = rebuilt_section_count;
        slot.render_stats.last_removed_section_count = removed_section_count;
        slot.render_stats.last_rebuilt_vertex_count = rebuilt_vertex_count;
        slot.render_stats.last_rebuilt_face_count =
            quad_face_count_from_indices(rebuilt_index_count);
        slot.render_stats.last_rebuilt_index_count = rebuilt_index_count;
        slot.render_stats.last_neighbor_ready_section_count = neighbor_ready_section_count;
        slot.render_stats.last_near_exception_section_count = near_exception_section_count;
        slot.render_stats.last_deferred_section_count = deferred_section_count;
        slot.render_stats.last_submitted_compile_section_count = submitted_compile_section_count;
        slot.render_stats.last_completed_compile_section_count = completed_compile_section_count;
        slot.render_stats.last_stale_compile_section_count = stale_compile_section_count;
        slot.render_stats.last_pending_compile_jobs = pending_compile_jobs_after_sync;
        slot.render_stats.last_visibility_graph_build_count = visibility_graph_build_count;
        slot.render_stats.last_visibility_graph_total_ms = visibility_graph_total_ms;
        slot.render_stats.last_visibility_graph_worst_ms = visibility_graph_worst_ms;
        if let Some(resident) = resident_mesh_stats {
            slot.render_stats.resident_cpu_mesh_section_count = resident.resident_section_count;
            slot.render_stats.resident_cpu_mesh_vertex_count = resident.resident_vertex_count;
            slot.render_stats.resident_cpu_mesh_face_count = resident.resident_face_count();
            slot.render_stats.resident_cpu_mesh_index_count = resident.resident_index_count;
            slot.render_stats.resident_cpu_mesh_owned_bytes = resident.resident_mesh_owned_bytes;
        }
        slot.render_stats.last_uploaded_section_count = upload_report.uploaded_section_count;
        slot.render_stats.last_upload_removed_section_count = upload_report.removed_section_count;
        slot.render_stats.last_uploaded_vertex_count = upload_report.uploaded_vertex_count;
        slot.render_stats.last_uploaded_face_count = upload_report.uploaded_face_count();
        slot.render_stats.last_uploaded_index_count = upload_report.uploaded_index_count;
        slot.render_stats.last_uploaded_grass_patch_count =
            upload_report.uploaded_grass_patch_count;
        slot.render_stats.last_removed_grass_patch_count = upload_report.removed_grass_patch_count;
        slot.render_stats.last_uploaded_grass_bytes = upload_report.uploaded_grass_bytes;
        let upload_queue = slot.section_uploads.stats();
        let pending_render_count_start = policy.clock.now();
        let pending_render_chunks_after = runtime.pending_render_chunk_count();
        timing.runtime_pending_render_count_ms +=
            elapsed_ms(policy.clock.elapsed_since(pending_render_count_start));
        Ok(XrTerrainUploadSummary {
            poll_changed,
            pending_render_chunks_before,
            pending_render_chunks_after,
            pending_compile_jobs_before,
            pending_compile_jobs_after: pending_compile_jobs_after_sync,
            max_pending_compile_jobs,
            available_compile_slots_before,
            available_compile_slots_after: runtime.render_compile_available_pending_job_slots(),
            dispatcher_compile_worker_count: compile_health.compile_worker_count,
            dispatcher_completed_compile_tasks: compile_health.completed_compile_tasks,
            dispatcher_total_compile_worker_busy_ms: micros_to_ms(
                compile_health.total_compile_worker_busy_us,
            ),
            dispatcher_max_compile_worker_task_ms: micros_to_ms(
                compile_health.max_compile_worker_task_us,
            ),
            rebuilt_section_count,
            removed_section_count,
            rebuilt_vertex_count,
            rebuilt_index_count,
            target_rebuilt_section_count,
            non_target_rebuilt_section_count: rebuilt_section_count
                .saturating_sub(target_rebuilt_section_count),
            target_removed_section_count,
            non_target_removed_section_count: removed_section_count
                .saturating_sub(target_removed_section_count),
            section_sync_timing,
            neighbor_ready_section_count,
            near_exception_section_count,
            deferred_section_count,
            submitted_compile_section_count,
            deadline_skipped_compile_request_count,
            accepted_compile_result_count,
            queued_completed_compile_result_count,
            completed_compile_section_count,
            stale_compile_section_count,
            uploaded_section_count: upload_report.uploaded_section_count,
            upload_removed_section_count: upload_report.removed_section_count,
            uploaded_vertex_count: upload_report.uploaded_vertex_count,
            uploaded_index_count: upload_report.uploaded_index_count,
            queued_upload_section_count: upload_queue.queued_upload_sections,
            queued_upload_removed_section_count: upload_queue.queued_removed_sections,
            queued_upload_lifecycle_item_count: upload_queue.queued_lifecycle_items,
            queued_upload_mesh_owned_bytes: upload_queue.queued_upload_mesh_owned_bytes,
            upload_phase_event_count: upload_phase.phase_event_count,
            upload_enqueued_lifecycle_item_count: upload_phase.queued_lifecycle_items,
            upload_superseded_lifecycle_item_count: upload_phase.superseded_lifecycle_items,
            upload_drained_lifecycle_item_count: upload_phase.drained_lifecycle_items,
            upload_released_compile_job_count: upload_phase.released_compile_jobs,
            upload_released_compile_jobs_on_enqueue: upload_phase.released_compile_jobs_on_enqueue,
            upload_released_compile_jobs_on_apply: upload_phase.released_compile_jobs_on_apply,
            upload_held_lifecycle_item_count: upload_queue.held_release_lifecycle_items,
            upload_held_compile_job_count: upload_queue.held_compile_jobs,
            upload_limited: upload_phase.upload_limited,
            upload_accept_limited: upload_phase.accept_limited,
            upload_backpressured: upload_frame_decision.upload_backpressured,
            traversal_ready_section_count,
            record_cache: slot.draw.record_cache_stats(),
            visibility_graph_build_count,
            visibility_graph_total_ms,
            visibility_graph_worst_ms,
            ..poll_summary
        })
    }

    fn apply_world_slot_section_update_uploads(
        slot: &mut DrawableWorldSlot,
        device: &wgpu::Device,
        section_update: RenderSectionCacheUpdate,
        upload_backpressured: bool,
        policy: &WorldPreparationPolicy,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadApplyReport> {
        if policy.upload_budget.is_none()
            && policy.accept_budget.is_none()
            && !slot.section_uploads.has_pending_work()
        {
            let release_compile_jobs =
                RenderSectionUploadCoordinator::direct_release_count(&section_update);
            let phase = RenderSectionUploadPhaseReport::direct(
                section_update.accepted_compile_result_count,
                section_update.rebuilt_sections.len(),
                section_update.removed_section_keys.len(),
                release_compile_jobs,
            );
            let apply_start = policy.clock.now();
            let report = slot
                .draw
                .apply_section_updates_with_context_timed(
                    device,
                    &section_update.rebuilt_sections,
                    &section_update.removed_section_keys,
                    upload_backpressured,
                )
                .context("upload prepared world render section updates");
            timing.runtime_upload_apply_ms += elapsed_ms(policy.clock.elapsed_since(apply_start));
            return report.map(|(upload, upload_timing)| {
                timing.absorb_upload_apply_timing(upload_timing);
                XrTerrainUploadApplyReport {
                    upload,
                    phase,
                    release_compile_jobs,
                }
            });
        }

        let enqueue_start = policy.clock.now();
        let mut phase = slot.section_uploads.enqueue_cache_update(section_update);
        timing.runtime_upload_enqueue_ms += elapsed_ms(policy.clock.elapsed_since(enqueue_start));
        let select_start = policy.clock.now();
        let drain = slot
            .section_uploads
            .drain_budgeted(policy.upload_budget, policy.accept_budget);
        phase.absorb(drain.phase_report);
        timing.runtime_upload_select_ms += elapsed_ms(policy.clock.elapsed_since(select_start));
        if drain.is_empty() {
            return Ok(XrTerrainUploadApplyReport {
                upload: TexturedSectionUploadReport::default(),
                phase,
                release_compile_jobs: phase.released_compile_jobs,
            });
        }
        let apply_start = policy.clock.now();
        let report = slot
            .draw
            .apply_section_updates_with_context_timed(
                device,
                &drain.rebuilt_sections,
                &drain.removed_section_keys,
                upload_backpressured,
            )
            .context("accept budgeted prepared-world render section updates");
        timing.runtime_upload_apply_ms += elapsed_ms(policy.clock.elapsed_since(apply_start));
        report.map(|(upload, upload_timing)| {
            timing.absorb_upload_apply_timing(upload_timing);
            let released_on_apply = slot
                .section_uploads
                .complete_applied_lifecycle_items(drain.lifecycle_item_count);
            phase.record_applied_release(released_on_apply, slot.section_uploads.stats());
            XrTerrainUploadApplyReport {
                upload,
                phase,
                release_compile_jobs: phase.released_compile_jobs,
            }
        })
    }

    fn release_world_slot_render_compile_jobs(slot: &mut DrawableWorldSlot, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        slot.runtime
            .as_mut()
            .map_or(0, |runtime| runtime.release_render_compile_jobs(count))
    }

    fn frozen_runtime_upload_summary(&self) -> XrTerrainUploadSummary {
        let pending_render_chunks = self
            .active_world
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.pending_render_chunk_count());
        let pending_compile_jobs = self
            .active_world
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.render_compile_pending_job_count());
        let max_pending_compile_jobs = self
            .active_world
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.render_compile_max_pending_job_count());
        let available_compile_slots = self.active_world.runtime.as_ref().map_or(0, |runtime| {
            runtime.render_compile_available_pending_job_slots()
        });
        let compile_health = self
            .active_world
            .runtime
            .as_ref()
            .map(|runtime| runtime.render_compile_queue_health());
        let upload_queue = self.active_world.section_uploads.stats();
        XrTerrainUploadSummary {
            host_mode: self.runtime_host_mode(),
            pending_render_chunks_before: pending_render_chunks,
            pending_render_chunks_after: pending_render_chunks,
            pending_compile_jobs_before: pending_compile_jobs,
            pending_compile_jobs_after: pending_compile_jobs,
            max_pending_compile_jobs,
            available_compile_slots_before: available_compile_slots,
            available_compile_slots_after: available_compile_slots,
            dispatcher_compile_worker_count: compile_health
                .map_or(0, |health| health.compile_worker_count),
            dispatcher_completed_compile_tasks: compile_health
                .map_or(0, |health| health.completed_compile_tasks),
            dispatcher_total_compile_worker_busy_ms: compile_health.map_or(0.0, |health| {
                micros_to_ms(health.total_compile_worker_busy_us)
            }),
            dispatcher_max_compile_worker_task_ms: compile_health.map_or(0.0, |health| {
                micros_to_ms(health.max_compile_worker_task_us)
            }),
            queued_upload_section_count: upload_queue.queued_upload_sections,
            queued_upload_removed_section_count: upload_queue.queued_removed_sections,
            queued_upload_lifecycle_item_count: upload_queue.queued_lifecycle_items,
            queued_upload_mesh_owned_bytes: upload_queue.queued_upload_mesh_owned_bytes,
            upload_held_lifecycle_item_count: upload_queue.held_release_lifecycle_items,
            upload_held_compile_job_count: upload_queue.held_compile_jobs,
            traversal_ready_section_count: self.active_world.draw.traversal_ready_section_count(),
            record_cache: self.active_world.draw.record_cache_stats(),
            ..XrTerrainUploadSummary::default()
        }
    }

    pub fn latest_budget_decision_panel(&self) -> BudgetDecisionPanelReport {
        let scheduler = self
            .active_world
            .runtime
            .as_ref()
            .map(|runtime| {
                runtime
                    .last_poll_diagnostics()
                    .scheduler_budget_decision_panel
            })
            .unwrap_or_default();
        merge_budget_decision_panels(scheduler, self.active_world.render_admission_policy.panel())
    }

    fn runtime_host_mode(&self) -> XrTerrainHostMode {
        self.active_world
            .runtime
            .as_ref()
            .map_or(XrTerrainHostMode::LocalIntegrated, |runtime| {
                runtime.host_mode().into()
            })
    }

    fn current_actor_instances(&mut self) -> Vec<ActorInstance> {
        if self.active_world.scene.skip_actors {
            return Vec::new();
        }
        let presentations = self
            .active_world
            .interpolated_actor_presentations(self.services.clock.now());
        let authoritative_eye = self.active_world.camera.snapshot().eye;
        let presentation_eye = self
            .active_world
            .local_participant
            .presentation_camera_snapshot()
            .eye;
        let local_presentation_offset =
            glam_vec3_from_vec3d(presentation_eye.subtract(authoritative_eye));
        self.active_world
            .runtime
            .as_ref()
            .map_or_else(Vec::new, |runtime| {
                let instances = actor_instances_from_presentations_near_observer(
                    &presentations,
                    runtime.client(),
                    presentation_eye,
                );
                let mut instances = if self.mono_ui_context.is_some() {
                    instances
                        .into_iter()
                        .chain(
                            local_player_actor_instance_for_view(
                                &self.active_world.camera,
                                runtime.client(),
                                actor_figure_id_for_player_model(self.active_world.player_model),
                            )
                            .map(|mut actor| {
                                actor.feet_position += local_presentation_offset;
                                actor
                            }),
                        )
                        .collect::<Vec<_>>()
                } else {
                    instances
                };
                instances.extend(self.active_world.mallard_tracks.iter().map(|track| {
                    let position = runtime
                        .client()
                        .topology()
                        .nearest_position_lift(track.cue.position, presentation_eye);
                    mclone_render::entity::ActorInstance::mallard_track(
                        glam_vec3_from_vec3d(position),
                        track.cue.y_rot_degrees,
                    )
                    .with_opacity(0.72)
                }));
                instances
            })
    }

    fn current_preview_actor_instances(&mut self) -> Option<PreviewActorInstances> {
        let (source_world, context) = self
            .embedded_world_preview
            .as_ref()
            .filter(|preview| preview.phase == EmbeddedWorldPreviewPhase::Visible)
            .map(|preview| (preview.source_world, preview.context))?;
        let slot = self
            .standby_world
            .as_mut()
            .filter(|slot| slot.id == source_world)?;
        if slot.scene.skip_actors {
            return None;
        }
        let presentations = slot.interpolated_actor_presentations(self.services.clock.now());
        let runtime = slot.runtime.as_ref()?;
        let client = runtime.client();
        let instances = actor_instances_from_presentations(&presentations, client);
        let entity_observations = presentations
            .iter()
            .zip(&instances)
            .filter_map(|(presentation, instance)| {
                let mclone_client::ActorPresentationId::Entity(entity_id) = presentation.id else {
                    return None;
                };
                let snapshot = client.entity(entity_id)?;
                Some(EmbeddedWorldPreviewActorObservation {
                    entity_id,
                    persistent_id: snapshot.persistent_id,
                    kind: snapshot.kind,
                    source_feet_position: snapshot.position,
                    composition_feet_position: context.source_to_composition(snapshot.position),
                    tick_count: snapshot.tick_count,
                    source_packed_light: instance.packed_light,
                })
            })
            .collect();
        let remote_player_observations = presentations
            .iter()
            .zip(&instances)
            .filter_map(|(presentation, instance)| {
                let mclone_client::ActorPresentationId::RemotePlayer(player_id) = presentation.id
                else {
                    return None;
                };
                let update = *client.remote_player(player_id)?;
                Some(EmbeddedWorldPreviewRemotePlayerObservation {
                    player_id,
                    appearance: update.appearance,
                    source_feet_position: update.position,
                    composition_feet_position: context.source_to_composition(update.position),
                    y_rot_degrees: update.y_rot_degrees,
                    x_rot_degrees: update.x_rot_degrees,
                    on_ground: update.on_ground,
                    walk_animation_distance: presentation.walk_animation_distance,
                    source_packed_light: instance.packed_light,
                })
            })
            .collect();
        let entity_count = client.entity_count();
        let remote_player_count = client.remote_player_count();
        Some(PreviewActorInstances {
            source_world,
            instances,
            entity_count,
            remote_player_count,
            source_local_player_count: 0,
            entity_observations,
            remote_player_observations,
        })
    }

    fn underwater_effect_dt_seconds(&mut self) -> f32 {
        let now = self.services.clock.now();
        let dt_seconds = self
            .last_underwater_update
            .replace(now)
            .map_or(0.0, |last| {
                now.saturating_duration_since(last).as_secs_f32()
            });
        dt_seconds.min(0.1)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_eye_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared_draw: &PreparedTexturedSectionStereoDraw,
        preview_prepared_draw: Option<&PreparedTexturedSectionStereoDraw>,
        preview_translucent_order: Option<&[TerrainTranslucentSubmission]>,
        target: XrTerrainEyeTarget<'_>,
        render_view: ChunkRenderView,
        diagnostic_panel: WorldGuiPanel,
        actor_instances: &[mclone_render::entity::ActorInstance],
        preview_actor_instances: Option<&PreviewActorInstances>,
        render_options: TexturedSectionRenderOptions,
        sky_clear_color: wgpu::Color,
        sky_state: mclone_render::sky::SkyRenderState,
        underwater_overlay: Option<UnderwaterOverlay>,
        label: &'static str,
        view_slot: PerViewSlot,
        wait_after_submit: bool,
        terrain_view_enabled: bool,
    ) -> Result<XrRenderedEye> {
        let collect_split_timing = self.render_split_timing_enabled;
        let encode_start = collect_split_timing.then(|| self.services.clock.now());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(match label {
                "left" => "mclone_xr_terrain_left_eye_encoder",
                "right" => "mclone_xr_terrain_right_eye_encoder",
                _ => "mclone_xr_terrain_eye_encoder",
            }),
        });
        let frame = RenderFrameContext::new(
            device,
            queue,
            &mut encoder,
            RenderFrameTarget::color(target.color_view, target.size),
        );
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        let ui_state = self.current_ui_render_state();
        let session_projection = self.session_projection();
        let panel_draw = prepare_xr_menu_panel_draw(
            &mut self.ui,
            &mut self.menu_overlay_cache,
            gui_scale,
            ui_state,
            session_projection.loading_progress_overlay.as_ref(),
            &session_projection.status_overlay,
        );
        let ui_active = self.ui.is_active();
        let mut summary_ui_draw = panel_draw.panel_draw.clone();
        summary_ui_draw.append(&panel_draw.overlay_draw);
        let selection_outline = self.current_xr_selection_outline();
        let mut render_stats = self.active_world.render_stats;
        #[cfg(not(target_arch = "wasm32"))]
        let opaque_world_gate = self
            .opaque_world_gate_renderer
            .as_ref()
            .zip(self.world_gate.as_ref().map(WorldGate::render_gate));
        #[cfg(target_arch = "wasm32")]
        let opaque_world_gate = None;
        let terrain_composition =
            preview_prepared_draw
                .zip(preview_translucent_order)
                .and_then(|(prepared, translucent_order)| {
                    let preview = self
                        .embedded_world_preview
                        .as_ref()
                        .filter(|preview| preview.phase == EmbeddedWorldPreviewPhase::Visible)?;
                    let standby = self
                        .standby_world
                        .as_mut()
                        .filter(|slot| slot.id == preview.source_world)?;
                    let preview_options =
                        self.render_options
                            .with_sky_darken(sky_state.sky_darken())
                            .with_grass_time_seconds(render_options.grass_time_seconds)
                            .with_grass_interactors(
                                preview_actor_instances
                                    .map_or_else(GrassInteractorSet::default, |actors| {
                                        grass_interactors_from_actors(None, &actors.instances)
                                    }),
                            )
                            .with_topology(
                                standby.runtime.as_ref().map_or(
                                    mclone_core::HorizontalTopology::UNBOUNDED,
                                    |runtime| runtime.client().topology(),
                                ),
                            );
                    let placed_actors = preview_actor_instances
                        .filter(|actors| actors.source_world == preview.source_world)
                        .and_then(|actors| {
                            standby.actors.as_mut().map(|draw| PlacedActorFrame {
                                draw,
                                instances: &actors.instances,
                                context: preview.context,
                                render_options: preview_options,
                            })
                        });
                    Some(TerrainCompositionFrame {
                        placed: PlacedTerrainFrame {
                            draw: &standby.draw,
                            renderer: &preview.renderer,
                            prepared: PlacedTerrainPrepared::Stereo(prepared),
                            context: preview.context,
                            render_options: preview_options,
                        },
                        actors: placed_actors,
                        translucent_order,
                    })
                });
        let full_frame_start = collect_split_timing.then(|| self.services.clock.now());
        let (summary, frame_timing) = if collect_split_timing {
            if let Some(terrain_composition) = terrain_composition {
                render_full_frame_for_view_with_prepared_stereo_draw_and_placed_terrain_timed_in_slot(
                    frame,
                    target.depth,
                    &self.sky,
                    &mut self.active_world.draw,
                    prepared_draw,
                    terrain_composition,
                    Some(
                        self.active_world
                            .actors
                            .as_mut()
                            .expect("active world owns actor draw state"),
                    ),
                    Some(&mut self.screen_effects),
                    None,
                    render_view,
                    actor_instances,
                    underwater_overlay,
                    sky_clear_color,
                    sky_state,
                    render_options,
                    FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                    |_| summary_ui_draw,
                    &self.services.clock,
                    &mut render_stats,
                    view_slot,
                )
            } else if terrain_view_enabled {
                render_full_frame_for_view_with_prepared_stereo_draw_terrain_backdrop_and_opaque_gate_timed_in_slot(
                    frame,
                    target.depth,
                    &self.sky,
                    &mut self.active_world.draw,
                    prepared_draw,
                    self.terrain_view
                        .as_mut()
                        .expect("enabled scene terrain view remains initialized"),
                    opaque_world_gate,
                    Some(
                        self.active_world
                            .actors
                            .as_mut()
                            .expect("active world owns actor draw state"),
                    ),
                    Some(&mut self.screen_effects),
                    None,
                    render_view,
                    actor_instances,
                    underwater_overlay,
                    sky_clear_color,
                    sky_state,
                    render_options,
                    FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                    |_| summary_ui_draw,
                    &self.services.clock,
                    &mut render_stats,
                    view_slot,
                )
            } else {
                render_full_frame_for_view_with_prepared_stereo_draw_and_opaque_gate_timed_in_slot(
                    frame,
                    target.depth,
                    &self.sky,
                    &mut self.active_world.draw,
                    prepared_draw,
                    opaque_world_gate,
                    Some(
                        self.active_world
                            .actors
                            .as_mut()
                            .expect("active world owns actor draw state"),
                    ),
                    Some(&mut self.screen_effects),
                    None,
                    render_view,
                    actor_instances,
                    underwater_overlay,
                    sky_clear_color,
                    sky_state,
                    render_options,
                    FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                    |_| summary_ui_draw,
                    &self.services.clock,
                    &mut render_stats,
                    view_slot,
                )
            }
        } else {
            if let Some(terrain_composition) = terrain_composition {
                render_full_frame_for_view_with_prepared_stereo_draw_and_placed_terrain_in_slot(
                    frame,
                    target.depth,
                    &self.sky,
                    &mut self.active_world.draw,
                    prepared_draw,
                    terrain_composition,
                    Some(
                        self.active_world
                            .actors
                            .as_mut()
                            .expect("active world owns actor draw state"),
                    ),
                    Some(&mut self.screen_effects),
                    None,
                    render_view,
                    actor_instances,
                    underwater_overlay,
                    sky_clear_color,
                    sky_state,
                    render_options,
                    FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                    |_| summary_ui_draw,
                    &mut render_stats,
                    view_slot,
                )
                .map(|summary| (summary, Default::default()))
            } else if terrain_view_enabled {
                render_full_frame_for_view_with_prepared_stereo_draw_terrain_backdrop_and_opaque_gate_in_slot(
                    frame,
                    target.depth,
                    &self.sky,
                    &mut self.active_world.draw,
                    prepared_draw,
                    self.terrain_view
                        .as_mut()
                        .expect("enabled scene terrain view remains initialized"),
                    opaque_world_gate,
                    Some(
                        self.active_world
                            .actors
                            .as_mut()
                            .expect("active world owns actor draw state"),
                    ),
                    Some(&mut self.screen_effects),
                    None,
                    render_view,
                    actor_instances,
                    underwater_overlay,
                    sky_clear_color,
                    sky_state,
                    render_options,
                    FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                    |_| summary_ui_draw,
                    &mut render_stats,
                    view_slot,
                )
                .map(|summary| (summary, Default::default()))
            } else {
                render_full_frame_for_view_with_prepared_stereo_draw_and_opaque_gate_in_slot(
                    frame,
                    target.depth,
                    &self.sky,
                    &mut self.active_world.draw,
                    prepared_draw,
                    opaque_world_gate,
                    Some(
                        self.active_world
                            .actors
                            .as_mut()
                            .expect("active world owns actor draw state"),
                    ),
                    Some(&mut self.screen_effects),
                    None,
                    render_view,
                    actor_instances,
                    underwater_overlay,
                    sky_clear_color,
                    sky_state,
                    render_options,
                    FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                    |_| summary_ui_draw,
                    &mut render_stats,
                    view_slot,
                )
                .map(|summary| (summary, Default::default()))
            }
        }
        .with_context(|| format!("render XR terrain {label} eye"))?;
        let full_frame_ms = full_frame_start.map_or(0.0, |start| {
            elapsed_ms(self.services.clock.elapsed_since(start))
        });
        let mut xr_fade_ms = 0.0;
        if let Some(overlay) = self.head_comfort.overlay() {
            let fade_start = collect_split_timing.then(|| self.services.clock.now());
            self.screen_effects.render_fade_in_slot(
                device,
                queue,
                &mut encoder,
                RenderFrameTarget::color(target.color_view, target.size),
                overlay,
                view_slot,
            );
            xr_fade_ms = fade_start.map_or(0.0, |start| {
                elapsed_ms(self.services.clock.elapsed_since(start))
            });
        }
        let selection_start = collect_split_timing.then(|| self.services.clock.now());
        let lens_mesh = if self.worldgen_lens.active_layer().is_some() {
            let topology = self
                .active_world
                .runtime
                .as_ref()
                .map_or(HorizontalTopology::UNBOUNDED, |runtime| {
                    runtime.client().topology()
                });
            let loaded_chunks = self
                .active_world
                .runtime
                .as_ref()
                .map(|runtime| {
                    runtime
                        .client()
                        .loaded_chunk_positions()
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .unwrap_or_default();
            self.worldgen_lens.prepare_mesh(
                self.active_world.scene.seed,
                self.active_world.scene.world_generation_profile,
                topology,
                render_view.camera_position,
                &loaded_chunks,
            )
        } else {
            None
        };
        self.worldgen_lens_renderer.render_in_slot(
            device,
            queue,
            &mut encoder,
            RenderFrameTarget::color(target.color_view, target.size),
            target.depth,
            render_view,
            lens_mesh,
            view_slot,
        );
        self.selection_outline.render_in_slot(
            device,
            queue,
            &mut encoder,
            RenderFrameTarget::color(target.color_view, target.size),
            target.depth,
            render_view,
            selection_outline.as_ref(),
            view_slot,
        );
        let xr_selection_ms = selection_start.map_or(0.0, |start| {
            elapsed_ms(self.services.clock.elapsed_since(start))
        });
        let mut world_lines = engine_debug_world_lines(
            &self.active_world.camera,
            EngineDebugVisualOptions::new(self.player_collision_box_visible),
        );
        if self.diagnostic_panel.debug_diagnostics_visible() {
            world_lines.extend(topology_debug_world_lines(
                self.active_world
                    .runtime
                    .as_ref()
                    .map_or(HorizontalTopology::UNBOUNDED, |runtime| {
                        runtime.client().topology()
                    }),
                render_view.camera_position,
            ));
        }
        if let Some(gameplay_ray) = self
            .xr_gameplay_controller_ray_line()
            .context("build XR gameplay controller ray visual")?
        {
            world_lines.push(gameplay_ray);
        }
        world_lines.extend(self.xr_blink_teleport_lines());
        let mut xr_world_lines_ms = 0.0;
        if !world_lines.is_empty() {
            let world_lines_start = collect_split_timing.then(|| self.services.clock.now());
            self.world_gui_renderer
                .render_lines_in_slot(
                    device,
                    queue,
                    &mut encoder,
                    RenderFrameTarget::color(target.color_view, target.size),
                    render_view,
                    &world_lines,
                    view_slot,
                )
                .with_context(|| format!("render XR world debug lines for {label} eye"))?;
            xr_world_lines_ms = world_lines_start.map_or(0.0, |start| {
                elapsed_ms(self.services.clock.elapsed_since(start))
            });
        }
        let mut ui_panel_stats = WorldGuiPanelRenderStats::default();
        let mut ui_draw_cache_stats = panel_draw.draw_cache;
        let mut xr_world_panel_ms = 0.0;
        let diagnostic_panel_start = collect_split_timing.then(|| self.services.clock.now());
        self.refresh_debug_diagnostics_overlay();
        let (diagnostic_panel_stats, diagnostic_draw_cache) = self
            .diagnostic_panel
            .render_in_slot(
                device,
                queue,
                &mut encoder,
                RenderFrameTarget::color(target.color_view, target.size),
                render_view,
                diagnostic_panel,
                view_slot,
            )
            .with_context(|| format!("render XR diagnostic panel for {label} eye"))?;
        ui_panel_stats.add(diagnostic_panel_stats);
        ui_draw_cache_stats.add(diagnostic_draw_cache);
        xr_world_panel_ms += diagnostic_panel_start.map_or(0.0, |start| {
            elapsed_ms(self.services.clock.elapsed_since(start))
        });
        let field_guide_notification = self
            .active_world
            .visible_field_guide_notification(self.services.clock.now());
        if !ui_active {
            let draw = field_guide_notification.map_or_else(GuiDrawList::new, |progress| {
                xr_field_guide_draw(
                    progress.mallard,
                    progress.deer,
                    progress.bee,
                    progress.rabbit,
                )
            });
            if !draw.commands().is_empty() {
                let guide_start = collect_split_timing.then(|| self.services.clock.now());
                ui_panel_stats.add(
                    self.world_gui_overlay_renderer
                        .render_panel_in_slot_report(
                            device,
                            queue,
                            &mut encoder,
                            RenderFrameTarget::color(target.color_view, target.size),
                            render_view,
                            XR_FIELD_GUIDE_PANEL_PIXELS,
                            [
                                XR_FIELD_GUIDE_PANEL_PIXELS[0] as f32,
                                XR_FIELD_GUIDE_PANEL_PIXELS[1] as f32,
                            ],
                            &draw,
                            xr_field_guide_panel_from_render_views([render_view; 2]),
                            &[],
                            view_slot,
                        )
                        .with_context(|| format!("render XR field-guide panel for {label} eye"))?,
                );
                xr_world_panel_ms += guide_start.map_or(0.0, |start| {
                    elapsed_ms(self.services.clock.elapsed_since(start))
                });
            }
        }
        if ui_active {
            if let Some(panel) = self.menu_panel_pose {
                let world_panel_start = collect_split_timing.then(|| self.services.clock.now());
                let controller_ray_lines = self
                    .xr_menu_controller_ray_lines(panel)
                    .context("build XR menu controller ray visuals")?;
                let mut menu_panel_stats =
                    if let Some(cache_revision) = panel_draw.cache_revision {
                        self.world_gui_renderer.render_panel_cached_in_slot(
                            device,
                            queue,
                            &mut encoder,
                            RenderFrameTarget::color(target.color_view, target.size),
                            render_view,
                            XR_MENU_PANEL_PIXELS,
                            [gui_scale.width, gui_scale.height],
                            &panel_draw.panel_draw,
                            panel,
                            if panel_draw.overlay_draw.commands().is_empty() {
                                &controller_ray_lines
                            } else {
                                &[]
                            },
                            view_slot,
                            cache_revision,
                        )
                    } else {
                        self.world_gui_renderer.render_panel_in_slot_report(
                            device,
                            queue,
                            &mut encoder,
                            RenderFrameTarget::color(target.color_view, target.size),
                            render_view,
                            XR_MENU_PANEL_PIXELS,
                            [gui_scale.width, gui_scale.height],
                            &panel_draw.panel_draw,
                            panel,
                            if panel_draw.overlay_draw.commands().is_empty() {
                                &controller_ray_lines
                            } else {
                                &[]
                            },
                            view_slot,
                        )
                    }
                    .with_context(|| format!("render XR menu panel for {label} eye"))?;
                if !panel_draw.overlay_draw.commands().is_empty() {
                    let overlay_stats =
                        if let Some(cache_revision) = panel_draw.overlay_cache_revision {
                            self.world_gui_overlay_renderer.render_panel_cached_in_slot(
                                device,
                                queue,
                                &mut encoder,
                                RenderFrameTarget::color(target.color_view, target.size),
                                render_view,
                                XR_MENU_PANEL_PIXELS,
                                [gui_scale.width, gui_scale.height],
                                &panel_draw.overlay_draw,
                                panel,
                                &controller_ray_lines,
                                view_slot,
                                cache_revision,
                            )
                        } else {
                            self.world_gui_overlay_renderer.render_panel_in_slot_report(
                                device,
                                queue,
                                &mut encoder,
                                RenderFrameTarget::color(target.color_view, target.size),
                                render_view,
                                XR_MENU_PANEL_PIXELS,
                                [gui_scale.width, gui_scale.height],
                                &panel_draw.overlay_draw,
                                panel,
                                &controller_ray_lines,
                                view_slot,
                            )
                        }
                        .with_context(|| format!("render XR menu panel overlay for {label} eye"))?;
                    menu_panel_stats.add(overlay_stats);
                }
                ui_panel_stats.add(menu_panel_stats);
                xr_world_panel_ms += world_panel_start.map_or(0.0, |start| {
                    elapsed_ms(self.services.clock.elapsed_since(start))
                });
            }
        }
        if let Some(overlay) = self.embedded_world_activation_fade_overlay() {
            self.screen_effects.render_fade_in_slot(
                device,
                queue,
                &mut encoder,
                RenderFrameTarget::color(target.color_view, target.size),
                overlay,
                view_slot,
            );
        }
        let finish_start = collect_split_timing.then(|| self.services.clock.now());
        let command_buffer = encoder.finish();
        let encoder_finish_ms = finish_start.map_or(0.0, |start| {
            elapsed_ms(self.services.clock.elapsed_since(start))
        });
        let encode_total_ms = encode_start.map_or(0.0, |start| {
            elapsed_ms(self.services.clock.elapsed_since(start))
        });
        let submit_start = collect_split_timing.then(|| self.services.clock.now());
        let submission = queue.submit(Some(command_buffer));
        let submit_ms = submit_start.map_or(0.0, |start| {
            elapsed_ms(self.services.clock.elapsed_since(start))
        });
        let poll_start = collect_split_timing.then(|| self.services.clock.now());
        let (submission, poll_wait_ms) = if wait_after_submit {
            Self::wait_for_xr_submission(
                device,
                submission,
                &format!("XR terrain {label}-eye render"),
            )?;
            (
                None,
                poll_start.map_or(0.0, |start| {
                    elapsed_ms(self.services.clock.elapsed_since(start))
                }),
            )
        } else {
            (Some(submission), 0.0)
        };
        if label == "left" {
            self.active_world.render_stats = render_stats;
        }
        let prepare_ms = frame_timing.terrain_prepare_ms;
        Ok(XrRenderedEye {
            summary,
            timing: XrTerrainEyeRenderTiming {
                full_frame_ms,
                sky_ms: frame_timing.sky_ms,
                terrain_opaque_ms: frame_timing.terrain_opaque_ms,
                terrain_backdrop_ms: frame_timing.terrain_backdrop_ms,
                terrain_translucent_ms: frame_timing.terrain_translucent_ms,
                prepare_ms,
                cull_ms: frame_timing.terrain_cull_ms,
                uniform_write_ms: frame_timing.terrain_uniform_write_ms,
                translucent_collect_ms: frame_timing.terrain_translucent_collect_ms,
                translucent_sort_ms: frame_timing.terrain_translucent_sort_ms,
                encode_ms: (encode_total_ms - prepare_ms).max(0.0),
                section_encode_ms: frame_timing.terrain_encode_ms,
                placed_cull_ms: frame_timing.placed_cull_ms,
                placed_draw_ms: frame_timing.placed_draw_ms,
                actor_ms: frame_timing.actor_ms,
                placed_actor_ms: frame_timing.placed_actor_ms,
                screen_effect_ms: frame_timing.screen_effect_ms,
                gui_ms: frame_timing.gui_ms,
                xr_fade_ms,
                xr_selection_ms,
                xr_world_lines_ms,
                xr_world_panel_ms,
                encoder_finish_ms,
                submit_ms,
                poll_wait_ms,
            },
            ui_panel: ui_panel_stats,
            ui_draw_cache: ui_draw_cache_stats,
            submission,
        })
    }

    fn wait_for_xr_submission(
        device: &wgpu::Device,
        submission: wgpu::SubmissionIndex,
        label: &str,
    ) -> Result<()> {
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .map(|_| ())
            .with_context(|| format!("wait for {label} submission"))?;
        Ok(())
    }

    fn commit_engine_camera_player_pose_timed(
        &mut self,
    ) -> Result<(bool, EngineCameraCommitTiming)> {
        let before = self.active_world.camera.snapshot();
        let before_feet = self.active_world.camera.feet_position();
        let Some(runtime) = self.active_world.runtime.as_mut() else {
            return Ok((false, EngineCameraCommitTiming::default()));
        };
        self.player_pose_sync
            .record_attempt(self.services.clock.now());
        let mut timing = EngineCameraCommitTiming::default();
        let changed = mclone_app_runtime::commit_engine_camera_player_pose(
            runtime,
            &mut self.active_world.local_participant.camera,
            XR_CAMERA_COMMIT_CONTEXT,
            &self.services.clock,
            Some(&mut timing),
        )
        .context("sync XR terrain player pose")?;
        let after = self.active_world.camera.snapshot();
        if before.eye != after.eye
            || before.yaw_radians != after.yaw_radians
            || before.pitch_radians != after.pitch_radians
            || before_feet != self.active_world.camera.feet_position()
        {
            self.active_world.local_participant.reset_movement();
        }
        Ok((changed, timing))
    }

    fn commit_engine_camera_player_pose_if_due_timed(
        &mut self,
    ) -> Result<Option<(bool, EngineCameraCommitTiming)>> {
        let report_rate_hz = self
            .active_world
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.client().session_configuration())
            .map(|configuration| configuration.body_pose_report_rate_hz)
            .unwrap_or(pose_sync::DEFAULT_PLAYER_POSE_SYNC_RATE_HZ);
        self.player_pose_sync.set_rate_hz(report_rate_hz);
        if !self.player_pose_sync.is_due(self.services.clock.now()) {
            return Ok(None);
        }
        if let Some(runtime) = self.active_world.runtime.as_mut() {
            runtime.core_mut().client_mut().advance_time_tick();
        }
        self.commit_engine_camera_player_pose_timed().map(Some)
    }

    fn play_local_movement_sounds(&mut self, before_feet: Vec3d) {
        let first_party = self
            .active_assets
            .selection
            .enabled_ids()
            .any(|id| id.as_str() == mclone_assets::AUTHORED_FIRST_PARTY_PACK_ID);
        let events = self.active_world.camera.take_landing_events();
        for event in events {
            let (legacy_sound, gain) = landing_playback_for_impact(event.impact_speed);
            let sound = if first_party {
                self.acoustic_material_below(event.position).landing_sound()
            } else {
                legacy_sound
            };
            self.services.audio.play_with(
                sound,
                PlaybackParams {
                    gain,
                    seed: spatial_sound_seed(event.position, 0x4c41_4e44),
                    ..PlaybackParams::default()
                },
            );
        }

        let after_feet = self.active_world.camera.feet_position();
        let grounded = self.active_world.camera.on_ground();
        if let Some(event) = self
            .active_world
            .footsteps
            .advance(before_feet, after_feet, grounded)
            && first_party
        {
            let sound = self
                .acoustic_material_below(event.position)
                .footstep_sound();
            self.services.audio.play_with(
                sound,
                PlaybackParams {
                    seed: spatial_sound_seed(event.position, event.sequence),
                    ..PlaybackParams::default()
                },
            );
        }
    }

    fn acoustic_material_below(&self, feet: Vec3d) -> AcousticMaterial {
        let below = BlockPos::containing(Vec3d::new(feet.x, feet.y - 0.05, feet.z));
        self.active_world
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.client().block_state_at_block_pos(below))
            .map_or(AcousticMaterial::Neutral, acoustic_material_for_state)
    }

    fn enqueue_interaction_sound(
        &mut self,
        intent: LocalInteractionSoundIntent,
        target: &BlockInteractionTarget,
    ) {
        let Some(runtime) = self.active_world.runtime.as_ref() else {
            return;
        };
        let client = runtime.client();
        let kind_and_material = match intent {
            LocalInteractionSoundIntent::Break => {
                let pos = target.hit.block_pos;
                let Some(before) = client.block_state_at_block_pos(pos) else {
                    return;
                };
                if before == AIR_BLOCK_STATE_ID {
                    return;
                }
                (
                    PendingInteractionSoundKind::Break { pos, before },
                    acoustic_material_for_state(before),
                )
            }
            LocalInteractionSoundIntent::Use => {
                let clicked = target.hit.block_pos;
                if let Some(before) = client.block_state_at_block_pos(clicked)
                    && (terrain_id::OAK_FENCE_GATE_STATE_START
                        ..=terrain_id::OAK_FENCE_GATE_STATE_END)
                        .contains(&before.0)
                {
                    return self.push_pending_interaction_sound(
                        PendingInteractionSoundKind::Toggle {
                            pos: clicked,
                            before,
                        },
                        AcousticMaterial::Wood,
                    );
                }
                let selected = usize::from(self.active_world.interaction.selected_hotbar_slot());
                let Some(material) = placement_acoustic_material(
                    self.active_world.interaction.hotbar_items()[selected],
                    client.player_inventory()[selected],
                ) else {
                    return;
                };
                let adjacent = clicked.relative(target.hit.direction);
                (
                    PendingInteractionSoundKind::Place {
                        candidates: [
                            (clicked, client.block_state_at_block_pos(clicked)),
                            (adjacent, client.block_state_at_block_pos(adjacent)),
                        ],
                    },
                    material,
                )
            }
        };
        self.push_pending_interaction_sound(kind_and_material.0, kind_and_material.1);
    }

    fn push_pending_interaction_sound(
        &mut self,
        kind: PendingInteractionSoundKind,
        material: AcousticMaterial,
    ) {
        if self.active_world.pending_interaction_sounds.len() == MAX_PENDING_INTERACTION_SOUNDS {
            self.active_world.pending_interaction_sounds.pop_front();
        }
        let sequence = self.active_world.interaction_sound_sequence;
        self.active_world.interaction_sound_sequence = sequence.wrapping_add(1);
        self.active_world
            .pending_interaction_sounds
            .push_back(PendingInteractionSound {
                kind,
                material,
                submitted_at: self.services.clock.now(),
                sequence,
            });
    }

    fn play_confirmed_interaction_sounds(&mut self) {
        let now = self.services.clock.now();
        let mut pending = std::mem::take(&mut self.active_world.pending_interaction_sounds);
        let Some(runtime) = self.active_world.runtime.as_ref() else {
            return;
        };
        let mut retained = VecDeque::with_capacity(pending.len());
        let mut confirmed = Vec::new();
        while let Some(sound) = pending.pop_front() {
            match sound.resolve(now, |pos| runtime.client().block_state_at_block_pos(pos)) {
                PendingInteractionResolution::Pending => retained.push_back(sound),
                PendingInteractionResolution::Rejected => {}
                PendingInteractionResolution::Confirmed { pos, sound: key } => {
                    confirmed.push((key, pos, sound.sequence));
                }
            }
        }
        self.active_world.pending_interaction_sounds = retained;
        for (sound, pos, sequence) in confirmed {
            let position = block_sound_position(pos);
            self.services.audio.play_with(
                sound,
                PlaybackParams {
                    pan: self.world_sound_pan(position),
                    seed: spatial_sound_seed(position, sequence),
                    ..PlaybackParams::default()
                },
            );
        }
    }

    fn play_mallard_calls_and_update_tracks(&mut self) {
        let now = self.services.clock.now();
        self.active_world
            .mallard_tracks
            .retain(|track| track.expires_at > now);
        let listener = self.active_world.camera.snapshot().eye;
        let Some(runtime) = self.active_world.runtime.as_mut() else {
            return;
        };
        let calls = runtime.drain_mallard_calls();
        let deer_sounds = runtime.drain_deer_sounds();
        let bee_sounds = runtime.drain_bee_sounds();
        let rabbit_sounds = runtime.drain_rabbit_sounds();
        let tracks = runtime.drain_mallard_tracks();
        let topology = runtime.client().topology();
        let positioned_calls = calls
            .into_iter()
            .map(|cue| (cue, topology.nearest_position_lift(cue.position, listener)))
            .collect::<Vec<_>>();
        for (cue, position) in positioned_calls {
            let distance = position.subtract(listener).length_sqr().sqrt();
            let gain = (1.0 - distance / f64::from(cue.audible_radius)).clamp(0.0, 1.0) as f32;
            self.services.audio.play_with(
                MALLARD_CALL,
                PlaybackParams {
                    gain,
                    pan: self.world_sound_pan(position),
                    seed: cue.sequence,
                    ..PlaybackParams::default()
                },
            );
        }
        for cue in deer_sounds {
            let position = topology.nearest_position_lift(cue.position, listener);
            let distance = position.subtract(listener).length_sqr().sqrt();
            let gain = (1.0 - distance / f64::from(cue.audible_radius)).clamp(0.0, 1.0) as f32;
            let key = match cue.kind {
                mclone_protocol::DeerSoundKind::Contact => DEER_CONTACT,
                mclone_protocol::DeerSoundKind::Alarm => DEER_ALARM,
                mclone_protocol::DeerSoundKind::Impact => DEER_IMPACT,
            };
            self.services.audio.play_with(
                key,
                PlaybackParams {
                    gain,
                    pan: self.world_sound_pan(position),
                    seed: cue.sequence,
                    ..PlaybackParams::default()
                },
            );
        }
        for cue in bee_sounds {
            let position = topology.nearest_position_lift(cue.position, listener);
            let distance = position.subtract(listener).length_sqr().sqrt();
            let gain = (1.0 - distance / f64::from(cue.audible_radius)).clamp(0.0, 1.0) as f32;
            self.services.audio.play_with(
                BEE_BUZZ,
                PlaybackParams {
                    gain,
                    pan: self.world_sound_pan(position),
                    seed: cue.sequence,
                    ..PlaybackParams::default()
                },
            );
        }
        for cue in rabbit_sounds {
            let position = topology.nearest_position_lift(cue.position, listener);
            let distance = position.subtract(listener).length_sqr().sqrt();
            let gain = (1.0 - distance / f64::from(cue.audible_radius)).clamp(0.0, 1.0) as f32;
            let key = match cue.kind {
                mclone_protocol::RabbitSoundKind::Thump => RABBIT_THUMP,
                mclone_protocol::RabbitSoundKind::Dig => RABBIT_DIG,
                mclone_protocol::RabbitSoundKind::Rustle => RABBIT_RUSTLE,
            };
            self.services.audio.play_with(
                key,
                PlaybackParams {
                    gain,
                    pan: self.world_sound_pan(position),
                    seed: cue.sequence,
                    ..PlaybackParams::default()
                },
            );
        }
        for cue in tracks {
            if self.active_world.mallard_tracks.len() == MAX_ACTIVE_MALLARD_TRACKS {
                self.active_world.mallard_tracks.pop_front();
            }
            self.active_world
                .mallard_tracks
                .push_back(ActiveMallardTrack {
                    cue,
                    expires_at: now.saturating_add(MALLARD_TRACK_LIFETIME),
                });
        }
    }

    fn world_sound_pan(&self, position: Vec3d) -> f32 {
        let listener = self.active_world.camera.snapshot();
        let delta = position.subtract(listener.eye);
        let horizontal_length = delta.x.hypot(delta.z);
        if horizontal_length <= 1.0e-6 {
            return 0.0;
        }
        let forward = mclone_client::view_vector(listener.yaw_radians, 0.0);
        let right_x = -forward.z;
        let right_z = forward.x;
        ((delta.x * right_x + delta.z * right_z) / horizontal_length).clamp(-1.0, 1.0) as f32 * 0.8
    }

    fn play_ui_action_sound(&self, action: GameUiAction) {
        self.services
            .audio
            .play_with(sound_for_ui_action(action), PlaybackParams::default());
    }

    fn play_ui_error_sound(&self) {
        self.services
            .audio
            .play_with(UI_ERROR, PlaybackParams::default());
    }

    pub fn season_preview_settings(&self) -> SeasonPreviewSettings {
        self.season_preview
    }

    pub fn set_season_preview_settings(&mut self, settings: SeasonPreviewSettings) {
        self.season_preview = settings;
    }

    fn day_time(&self) -> u64 {
        self.active_world.runtime.as_ref().map_or_else(
            || {
                self.active_world
                    .scene
                    .startup
                    .day_time_override
                    .unwrap_or(0)
            },
            |runtime| runtime.client().day_time(),
        )
    }

    fn solar_render_state(&self) -> SkyRenderState {
        let fixed = if self.active_world.scene.startup.world_generation_profile
            == mclone_server::WorldGenerationProfile::McloneOverworldV1
        {
            SkyRenderState::mclone_fixed(self.time_of_day(), self.sun_angle())
        } else {
            SkyRenderState::vanilla(self.time_of_day(), self.sun_angle())
        };
        self.solar_frame_diagnostics().map_or(fixed, |diagnostics| {
            SkyRenderState::SeasonalSolar(diagnostics.sample)
        })
    }

    pub fn solar_frame_diagnostics(&self) -> Option<SolarFrameDiagnostics> {
        if !self.season_preview.enabled
            || self.active_world.scene.startup.world_generation_profile
                != mclone_server::WorldGenerationProfile::McloneOverworldV1
        {
            return None;
        }

        let topology = self
            .active_world
            .runtime
            .as_ref()
            .map_or(self.active_world.scene.startup.world_topology, |runtime| {
                runtime.client().topology()
            });
        let policy = SolarCoordinatePolicy::mclone_for_topology(topology);
        let eye = self.active_world.camera.snapshot().eye;
        let world_latitude = match policy.latitude_at(eye.x, eye.z) {
            Ok(latitude) => latitude,
            Err(error) => {
                log::warn!("seasonal solar latitude fallback: {error}");
                return None;
            }
        };
        let effective_latitude_degrees = match self.season_preview.latitude_source {
            LatitudeSource::World => world_latitude.degrees,
            LatitudeSource::Manual => self.season_preview.manual_latitude.degrees(),
        };
        let solar_time_fraction = match self.season_preview.solar_time_source {
            SolarTimeSource::WorldClock => solar_time_fraction_from_day_time(self.day_time()),
            SolarTimeSource::Manual => self.season_preview.manual_solar_time.fraction(),
        };
        let sample = match SolarSample::compute(SolarInput {
            orbital_phase: self.season_preview.orbital_phase,
            effective_latitude_degrees,
            axial_tilt_degrees: MCLONE_AXIAL_TILT_DEGREES,
            solar_time_fraction,
        }) {
            Ok(sample) => sample,
            Err(error) => {
                log::warn!("seasonal solar sample fallback: {error}");
                return None;
            }
        };
        let observer_block = BlockPos::containing(eye);
        let observer_biome_id = self
            .active_world
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.client().biome_id_at_block_pos(observer_block))
            .unwrap_or(mclone_core::DEFAULT_BIOME_ID);
        let (mean_temperature, moisture) =
            mclone_mesh::seasonal_climate_for_biome(observer_biome_id);
        let local_season = EvaluatedLocalSeason::evaluate(LocalSeasonInput {
            orbital_phase: self.season_preview.orbital_phase,
            effective_latitude_degrees,
            mean_temperature,
            moisture,
            altitude_blocks: eye.y as f32,
        });
        Some(SolarFrameDiagnostics {
            settings: self.season_preview,
            policy,
            observer_world_x: eye.x,
            observer_world_z: eye.z,
            observer_world_y: eye.y,
            observer_biome_id,
            mean_temperature,
            moisture,
            world_latitude,
            effective_latitude_degrees,
            solar_time_fraction,
            sample,
            local_season,
        })
    }

    fn time_of_day(&self) -> f32 {
        self.active_world.runtime.as_ref().map_or_else(
            || time::time_of_day(self.active_world.scene.day_time_override.unwrap_or(0)),
            |runtime| runtime.time_of_day(),
        )
    }

    fn sun_angle(&self) -> f32 {
        self.active_world.runtime.as_ref().map_or_else(
            || time::sun_angle(self.active_world.scene.day_time_override.unwrap_or(0)),
            |runtime| runtime.sun_angle(),
        )
    }

    fn effective_render_options(
        &self,
        camera_position: Vec3,
        sky_state: SkyRenderState,
    ) -> TexturedSectionRenderOptions {
        let mut options = self.render_options;
        options.fog = self.open_air_fog(sky_state);
        options.grass_time_seconds = grass_presentation_time_seconds(self.services.clock.now());
        options.grass_interactors =
            grass_interactors_from_actors(Some(self.active_world.camera.feet_position()), &[]);
        options.topology = self
            .active_world
            .runtime
            .as_ref()
            .map_or(mclone_core::HorizontalTopology::UNBOUNDED, |runtime| {
                runtime.client().topology()
            });
        options.seasonal_appearance = self.solar_frame_diagnostics().map_or_else(
            SeasonalAppearanceRenderState::default,
            |diagnostics| {
                SeasonalAppearanceRenderState::evaluated(
                    true,
                    diagnostics.local_season,
                    self.season_preview.recent_snow,
                )
            },
        );
        if self.camera_inside_occluding_block(camera_position) {
            options.section_occlusion_culling = false;
        }
        options
    }

    fn open_air_fog(&self, sky_state: SkyRenderState) -> RenderFog {
        let settings = self.fog_settings.normalized();
        let mode = match settings.mode {
            mclone_ui::GameFogMode::Off => RenderFogMode::Off,
            mclone_ui::GameFogMode::Classic => RenderFogMode::Linear,
            mclone_ui::GameFogMode::Natural => RenderFogMode::Exponential,
            mclone_ui::GameFogMode::GroundHaze => RenderFogMode::GroundHaze,
        };
        let clear = sky_state.clear_color();
        let sky_color = [clear.r as f32, clear.g as f32, clear.b as f32];
        let color = match settings.color_mode {
            mclone_ui::GameFogColorMode::Sky => sky_color,
            mclone_ui::GameFogColorMode::Neutral => {
                let luminance =
                    sky_color[0] * 0.2126 + sky_color[1] * 0.7152 + sky_color[2] * 0.0722;
                [luminance; 3]
            }
            mclone_ui::GameFogColorMode::Warm => [
                (sky_color[0] * 1.12).min(1.0),
                sky_color[1] * 0.98,
                sky_color[2] * 0.82,
            ],
            mclone_ui::GameFogColorMode::Cool => [
                sky_color[0] * 0.85,
                sky_color[1],
                (sky_color[2] * 1.12).min(1.0),
            ],
            mclone_ui::GameFogColorMode::Custom => settings.custom_color,
        };
        let exact_corner_coverage =
            self.active_world.scene.render_distance as f32 * 16.0 * std::f32::consts::SQRT_2;
        let coverage_end = self.terrain_projection_far_distance(exact_corner_coverage.max(32.0));
        RenderFog::open_air(
            mode,
            color,
            settings.visibility_blocks,
            settings.classic_start,
            coverage_end,
            settings.coverage_guard,
            settings.guard_start,
            settings.ground_base_y,
            settings.ground_falloff_blocks,
            settings.max_opacity,
            settings.exponential_squared,
            settings.far_cull,
        )
    }
}

fn grass_interactors_from_actors(
    local_feet_position: Option<Vec3d>,
    actors: &[ActorInstance],
) -> GrassInteractorSet {
    let mut interactors = GrassInteractorSet::default();
    if let Some(position) = local_feet_position
        && let Some(interactor) = GrassInteractor::new(
            GrassInteractorIdentity::LocalPlayer,
            [position.x as f32, position.y as f32, position.z as f32],
            0.65,
        )
    {
        interactors.push(interactor);
    }
    for actor in actors {
        let Some(identity) = actor.id.map(|identity| match identity {
            ActorInstanceId::LocalPlayer => GrassInteractorIdentity::LocalPlayer,
            ActorInstanceId::RemotePlayer(id) => GrassInteractorIdentity::RemotePlayer(id),
            ActorInstanceId::Entity(id) => GrassInteractorIdentity::Entity(id),
        }) else {
            continue;
        };
        if let Some(interactor) = GrassInteractor::new(
            identity,
            actor.feet_position.to_array(),
            actor.width * 0.5 + 0.3,
        ) {
            interactors.push(interactor);
        }
    }
    interactors
}

fn render_options_with_actor_grass_interactors(
    mut options: TexturedSectionRenderOptions,
    actors: &[ActorInstance],
) -> TexturedSectionRenderOptions {
    for actor in grass_interactors_from_actors(None, actors).iter() {
        options.grass_interactors.push(actor);
    }
    options
}

fn grass_presentation_time_seconds(now: MonotonicInstant) -> f32 {
    (now.as_nanos() % 4_096_000_000_000) as f32 / 1_000_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_app_runtime::session::RemoteSessionEndpoint;
    use mclone_app_runtime::world_catalog::{
        LocalWorldId, LocalWorldSummary, WorldCatalogCapabilities, WorldCatalogRequest,
        WorldCatalogResponse,
    };

    #[test]
    fn grass_presentation_time_is_monotonic_and_safely_rebased() {
        assert_eq!(
            grass_presentation_time_seconds(MonotonicInstant::from_nanos(1_500_000_000)),
            1.5
        );
        assert_eq!(
            grass_presentation_time_seconds(MonotonicInstant::from_nanos(4_097_500_000_000)),
            1.5
        );
    }

    #[test]
    fn grass_interactors_keep_stable_actor_identity_and_local_player_priority() {
        let mut local = ActorInstance::local_player(Vec3::new(2.0, 64.0, 3.0), 0.0);
        local.width = 0.8;
        let mut remote = ActorInstance::remote_player(Vec3::new(4.0, 64.0, 5.0), 0.0);
        remote.id = Some(ActorInstanceId::RemotePlayer(7));
        let mut cow = ActorInstance::cow_model(Vec3::new(6.0, 64.0, 7.0), 0.0, 0.9, 1.4);
        cow.id = Some(ActorInstanceId::Entity(11));
        let anonymous = ActorInstance::cow_model(Vec3::new(8.0, 64.0, 9.0), 0.0, 0.9, 1.4);

        let interactors = grass_interactors_from_actors(
            Some(Vec3d::new(1.0, 64.0, 1.0)),
            &[local, remote, cow, anonymous],
        );
        let interactors = interactors.iter().collect::<Vec<_>>();

        assert_eq!(interactors.len(), 3);
        assert_eq!(
            interactors[0].identity,
            GrassInteractorIdentity::LocalPlayer
        );
        assert_eq!(interactors[0].feet_position, [2.0, 64.0, 3.0]);
        assert_eq!(
            interactors[1].identity,
            GrassInteractorIdentity::RemotePlayer(7)
        );
        assert_eq!(interactors[2].identity, GrassInteractorIdentity::Entity(11));
        assert!((interactors[2].footprint_radius - 0.75).abs() < f32::EPSILON);
    }

    use mclone_render_session::ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND;

    #[test]
    fn authoritative_entity_motion_requires_one_stable_id_and_changed_position() {
        let snapshot = |id, position, tick_count| EntitySnapshot {
            id: mclone_protocol::EntityId(id),
            persistent_id: mclone_protocol::EntityPersistentId::new(0, id),
            kind: mclone_protocol::EntityKind::Cow,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            deer: None,
            position,
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.9,
            height: 1.4,
            tick_count,
            animation: None,
        };
        let from = snapshot(7, Vec3d::new(6.5, 66.0, 8.5), 117);
        let stationary = snapshot(7, from.position, 118);
        let moved = snapshot(7, Vec3d::new(6.6, 66.0, 8.5), 118);
        let unrelated = snapshot(8, Vec3d::new(10.5, 66.0, 8.5), 118);
        let drifted = snapshot(7, Vec3d::new(6.500_000_1, 66.0, 8.5), 118);
        let meaningful = snapshot(8, Vec3d::new(10.6, 66.0, 8.5), 119);

        assert_eq!(changed_entity_motion(&[from], &[stationary]), None);
        assert_eq!(changed_entity_motion(&[from], &[unrelated]), None);
        assert_eq!(
            changed_entity_motion(&[from], &[unrelated, moved]),
            Some((from, moved))
        );
        assert_eq!(
            changed_entity_motion(&[from, unrelated], &[drifted, meaningful]),
            Some((unrelated, meaningful))
        );
    }

    #[test]
    fn authoritative_remote_player_motion_requires_one_stable_id_and_changed_position() {
        let update = |id, position| RemotePlayerUpdate {
            id: mclone_protocol::RemotePlayerId(id),
            appearance: mclone_protocol::PlayerAppearance {
                model: mclone_protocol::PlayerModelKind::UprightBear,
            },
            position,
            y_rot_degrees: -90.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        };
        let from = update(7, Vec3d::new(8.5, 66.0, 8.5));
        let stationary = update(7, from.position);
        let moved = update(7, Vec3d::new(8.6, 66.0, 8.5));
        let unrelated = update(8, Vec3d::new(10.5, 66.0, 8.5));

        assert_eq!(changed_remote_player_motion(&[from], &[stationary]), None);
        assert_eq!(changed_remote_player_motion(&[from], &[unrelated]), None);
        assert_eq!(
            changed_remote_player_motion(&[from], &[unrelated, moved]),
            Some((from, moved))
        );
    }

    #[test]
    fn translucent_composition_qualifies_worlds_and_reverses_with_view() {
        let active_world = WorldInstanceId::new(11);
        let preview_world = WorldInstanceId::new(22);
        let active = vec![
            TexturedSectionTranslucentRecord {
                key: RenderSectionKey::new(0, 4, -1),
                composition_center: Vec3::new(0.0, 65.0, -1.0),
            },
            TexturedSectionTranslucentRecord {
                key: RenderSectionKey::new(0, 4, 1),
                composition_center: Vec3::new(0.0, 65.0, 17.0),
            },
        ];
        let placed = vec![TexturedSectionTranslucentRecord {
            key: RenderSectionKey::new(0, 4, 0),
            composition_center: Vec3::new(0.0, 65.0, 8.0),
        }];
        let mut front =
            mclone_render::chunk::ChunkCamera::overview_for_chunk(0, 0).render_view(640, 480);
        front.camera_position = Vec3::new(0.0, 67.0, -6.0);
        front.camera_forward = Vec3::Z;
        let mut behind = front;
        behind.camera_position = Vec3::new(0.0, 67.0, 22.0);
        behind.camera_forward = Vec3::NEG_Z;

        let front_order = compose_translucent_terrain_order(
            active_world,
            active.clone(),
            preview_world,
            placed.clone(),
            &[front, front],
        );
        let behind_order = compose_translucent_terrain_order(
            active_world,
            active,
            preview_world,
            placed,
            &[behind, behind],
        );
        let front_snapshot =
            embedded_translucent_order_snapshot(active_world, preview_world, &front_order);
        let behind_snapshot =
            embedded_translucent_order_snapshot(active_world, preview_world, &behind_order);

        assert_eq!(front_snapshot.section_count, 3);
        assert_eq!(front_snapshot.active_section_count, 2);
        assert_eq!(front_snapshot.preview_section_count, 1);
        assert_eq!(front_snapshot.source_switch_count, 2);
        assert_eq!(front_snapshot.first.unwrap().section.chunk_z, 1);
        assert_eq!(front_snapshot.last.unwrap().section.chunk_z, -1);
        assert_eq!(behind_snapshot.first.unwrap().section.chunk_z, -1);
        assert_eq!(behind_snapshot.last.unwrap().section.chunk_z, 1);
    }

    fn point_in(rect: Rect) -> Point {
        Point {
            x: rect.center_x(),
            y: rect.y + rect.height * 0.5,
        }
    }

    fn test_loading_progress_overlay(
        ready_chunks: usize,
        center_status: mclone_ui::LoadingProgressCellStatus,
    ) -> LoadingProgressOverlay {
        LoadingProgressOverlay::new(
            1,
            ready_chunks,
            9,
            ready_chunks > 0,
            [
                mclone_ui::LoadingProgressCell::new(
                    -1,
                    -1,
                    mclone_ui::LoadingProgressCellStatus::Terrain,
                ),
                mclone_ui::LoadingProgressCell::new(0, 0, center_status).playable(true),
                mclone_ui::LoadingProgressCell::new(
                    1,
                    1,
                    mclone_ui::LoadingProgressCellStatus::TargetReady,
                ),
            ],
        )
    }

    #[test]
    fn default_scene_options_match_desktop_xr_smoke_defaults() {
        let options = McloneSceneHostOptions::default();
        assert_eq!(options.seed, 12_345);
        assert_eq!(options.center(), ChunkPos::new(0, 0));
        assert_eq!(options.render_distance, 5);
        assert_eq!(
            options.underwater_detection_mode,
            XrUnderwaterDetectionMode::Midpoint
        );
    }

    #[test]
    fn startup_view_pose_alignment_mode_is_explicit() {
        assert_eq!(XrViewAlignmentMode::PlayerSpawn.label(), "player-spawn");
        assert_eq!(XrViewAlignmentMode::ViewPose.label(), "view-pose");
    }

    #[test]
    fn xr_underwater_detection_mode_labels_parse() {
        assert_eq!(XrUnderwaterDetectionMode::Midpoint.label(), "midpoint");
        assert_eq!(XrUnderwaterDetectionMode::PerEye.label(), "per-eye");
        assert_eq!(
            XrUnderwaterDetectionMode::parse_label("--xr-underwater-mode", "both").unwrap(),
            XrUnderwaterDetectionMode::Midpoint
        );
        assert_eq!(
            XrUnderwaterDetectionMode::parse_label("--xr-underwater-mode", "per_eye").unwrap(),
            XrUnderwaterDetectionMode::PerEye
        );
        assert!(
            XrUnderwaterDetectionMode::parse_label("--xr-underwater-mode", "split")
                .unwrap_err()
                .to_string()
                .contains("midpoint or per-eye")
        );
    }

    #[test]
    fn xr_underwater_overlay_uses_view_forward() {
        let overlay = underwater_overlay_from_forward(Vec3::new(1.0, 0.0, 0.0), 0.5, 0.25);

        assert_eq!(overlay.water_vision, 0.5);
        assert_eq!(overlay.effect_strength, 0.25);
        assert_eq!(
            overlay.alpha,
            mclone_render::screen_effect::VANILLA_UNDERWATER_ALPHA * 0.25
        );
        assert!(overlay.uv_offset[0].is_finite());
        assert!(overlay.uv_offset[1].is_finite());
    }

    #[test]
    fn xr_head_comfort_residual_dead_zone_and_cap() {
        assert_eq!(
            xr_head_comfort_target_from_inputs(
                XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS * 0.5,
                false
            ),
            XrHeadComfortTarget::default()
        );

        let partial = xr_head_comfort_target_from_inputs(0.375, false);
        assert!(partial.blocked_residual);
        assert!(!partial.head_penetrating);
        assert!(partial.alpha > 0.0);
        assert!(partial.alpha < XR_HEAD_COMFORT_MAX_ALPHA);

        let capped = xr_head_comfort_target_from_inputs(2.0, false);
        assert_eq!(capped.alpha, XR_HEAD_COMFORT_MAX_ALPHA);
    }

    #[test]
    fn xr_head_comfort_vertical_offset_alone_does_not_fade() {
        let target = xr_head_comfort_target_from_inputs(0.0, false);

        assert_eq!(target.alpha, 0.0);
        assert!(!target.blocked_residual);
        assert!(!target.head_penetrating);
    }

    #[test]
    fn xr_head_comfort_head_penetration_sets_floor_alpha() {
        let target = xr_head_comfort_target_from_inputs(0.0, true);

        assert_eq!(target.alpha, XR_HEAD_COMFORT_HEAD_PENETRATION_ALPHA);
        assert!(!target.blocked_residual);
        assert!(target.head_penetrating);
    }

    #[test]
    fn xr_head_comfort_noclip_suppresses_collision_fade() {
        let target = xr_head_comfort_target_from_inputs_for_collision_mode(
            XR_HEAD_COMFORT_RESIDUAL_FULL_BLOCKS,
            true,
            EngineCameraCollisionMode::NoClip,
        );

        assert_eq!(target, XrHeadComfortTarget::default());
    }

    #[test]
    fn xr_head_comfort_state_smooths_in_and_out() {
        let mut state = XrHeadComfortState::default();
        state.update(
            XrHeadComfortTarget {
                alpha: XR_HEAD_COMFORT_MAX_ALPHA,
                blocked_residual: true,
                head_penetrating: false,
            },
            f64::from(XR_HEAD_COMFORT_FADE_IN_SECONDS) * 0.5,
        );

        assert!(state.alpha > 0.0);
        assert!(state.alpha < XR_HEAD_COMFORT_MAX_ALPHA);
        assert_eq!(state.target_alpha, XR_HEAD_COMFORT_MAX_ALPHA);
        assert!(state.blocked_residual);

        for _ in 0..8 {
            state.update(
                XrHeadComfortTarget::default(),
                XR_LOCOMOTION_MAX_FRAME_SECONDS,
            );
        }

        assert_eq!(state.alpha, 0.0);
        assert_eq!(state.target_alpha, 0.0);
        assert!(!state.blocked_residual);
    }

    #[test]
    fn xr_head_comfort_fade_uses_same_alpha_for_both_eyes() {
        let mut state = XrHeadComfortState::default();
        state.update(
            XrHeadComfortTarget {
                alpha: 0.5,
                blocked_residual: true,
                head_penetrating: false,
            },
            f64::from(XR_HEAD_COMFORT_FADE_IN_SECONDS),
        );

        let [left, right] = xr_head_comfort_fade_overlays(state);

        assert_eq!(left, right);
        assert!(left.expect("fade overlay").alpha > 0.0);
    }

    #[test]
    fn default_xr_locomotion_mode_is_headset_yaw() {
        assert_eq!(XrLocomotionMode::default(), XrLocomotionMode::HeadsetYaw);
        assert_eq!(XrLocomotionMode::HeadsetYaw.label(), "headset-yaw");
        assert_eq!(XrLocomotionMode::PlayerYaw.label(), "player-yaw");
    }

    #[test]
    fn xr_game_ui_starts_with_menu_open_for_local_session() {
        let ui = xr_game_ui_for_session(
            Some(&ActiveSessionDescriptor::new_seed_local_world(44)),
            12_345,
        );

        assert!(ui.is_active());
        assert_eq!(ui.screen(), Some(mclone_ui::GameScreen::Title));
        assert_eq!(ui.new_world_seed(), 44);
        assert_eq!(ui.join_remote_addr(), DEFAULT_JOIN_REMOTE_ADDR);
    }

    #[test]
    fn xr_game_ui_starts_with_menu_open_for_remote_session() {
        let ui = xr_game_ui_for_session(
            Some(&ActiveSessionDescriptor::Remote {
                endpoint: RemoteSessionEndpoint::new("10.0.0.5:25565"),
            }),
            12_345,
        );

        assert!(ui.is_active());
        assert_eq!(ui.screen(), Some(mclone_ui::GameScreen::Title));
        assert_eq!(ui.new_world_seed(), 12_345);
        assert_eq!(ui.join_remote_addr(), "10.0.0.5:25565");
    }

    #[test]
    fn xr_menu_panel_draw_cache_hits_until_panel_revision_changes() {
        let mut ui = GameUiHost::new_ingame();
        ui.set_screen(Some(GameScreen::Help {
            parent: mclone_ui::GameHelpParent::OptionsPause,
        }));
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        let state = GameUiRenderState::default();
        let status = StatusOverlay::hidden();
        let mut overlay_cache = XrMenuPanelOverlayCache::default();

        let first = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            None,
            &status,
        );
        assert_eq!(first.draw_cache, UiDrawCacheStats::rebuild());
        assert!(first.cache_revision.is_some());
        assert!(first.overlay_cache_revision.is_none());
        assert!(!first.panel_draw.commands().is_empty());
        assert!(first.overlay_draw.commands().is_empty());

        let second = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            None,
            &status,
        );
        assert_eq!(second.draw_cache, UiDrawCacheStats::cache_hit());
        assert_eq!(second.cache_revision, first.cache_revision);
        assert_eq!(second.panel_draw, first.panel_draw);
        assert!(second.overlay_draw.commands().is_empty());

        let snapshot = ui.v2_debug_snapshot().expect("Controls has debug data");
        let back = snapshot
            .widgets
            .iter()
            .find(|widget| widget.label == "Back")
            .expect("Back button")
            .rect;
        ui.pointer_move(point_in(back));

        let hovered = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            None,
            &status,
        );
        assert_eq!(hovered.draw_cache, UiDrawCacheStats::rebuild());
        assert_ne!(hovered.cache_revision, first.cache_revision);

        ui.pointer_move(Point {
            x: back.x + 2.0,
            y: back.y + 2.0,
        });
        let jitter = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            None,
            &status,
        );
        assert_eq!(jitter.draw_cache, UiDrawCacheStats::cache_hit());
        assert_eq!(jitter.cache_revision, hovered.cache_revision);
        assert_eq!(jitter.panel_draw, hovered.panel_draw);
        assert!(jitter.overlay_draw.commands().is_empty());
    }

    #[test]
    fn xr_menu_panel_draw_composes_transient_overlays_over_cached_panel() {
        let mut ui = GameUiHost::new_ingame();
        ui.set_screen(Some(GameScreen::Pause));
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        let state = GameUiRenderState::default();
        let visible_status = StatusOverlay::new("starting", true);
        let mut overlay_cache = XrMenuPanelOverlayCache::default();

        let first = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            None,
            &visible_status,
        );
        let second = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            None,
            &visible_status,
        );

        assert_eq!(
            first.draw_cache,
            UiDrawCacheStats {
                rebuild_count: 2,
                cache_hit_count: 0,
            }
        );
        assert_eq!(
            second.draw_cache,
            UiDrawCacheStats {
                rebuild_count: 0,
                cache_hit_count: 2,
            }
        );
        assert!(first.cache_revision.is_some());
        assert_eq!(second.cache_revision, first.cache_revision);
        assert!(first.overlay_cache_revision.is_some());
        assert_eq!(second.overlay_cache_revision, first.overlay_cache_revision);
        assert!(!first.panel_draw.commands().is_empty());
        assert_eq!(second.panel_draw, first.panel_draw);
        assert!(!first.overlay_draw.commands().is_empty());
        assert_eq!(second.overlay_draw, first.overlay_draw);

        let changed_status = StatusOverlay::new("loading", true);
        let changed = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            None,
            &changed_status,
        );
        assert_eq!(
            changed.draw_cache,
            UiDrawCacheStats {
                rebuild_count: 1,
                cache_hit_count: 1,
            }
        );
        assert_eq!(changed.cache_revision, first.cache_revision);
        assert_ne!(changed.overlay_cache_revision, first.overlay_cache_revision);
        assert_eq!(changed.panel_draw, first.panel_draw);
        assert_ne!(changed.overlay_draw, first.overlay_draw);
    }

    #[test]
    fn xr_menu_panel_draw_keeps_panel_cache_when_progress_changes() {
        let mut ui = GameUiHost::new_ingame();
        ui.set_screen(Some(GameScreen::Pause));
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        let state = GameUiRenderState::default();
        let status = StatusOverlay::hidden();
        let mut overlay_cache = XrMenuPanelOverlayCache::default();
        let progress =
            test_loading_progress_overlay(3, mclone_ui::LoadingProgressCellStatus::Features);

        let first = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            Some(&progress),
            &status,
        );
        assert_eq!(
            first.draw_cache,
            UiDrawCacheStats {
                rebuild_count: 2,
                cache_hit_count: 0,
            }
        );
        assert!(first.cache_revision.is_some());
        assert!(first.overlay_cache_revision.is_some());
        assert!(!first.panel_draw.commands().is_empty());
        assert!(!first.overlay_draw.commands().is_empty());

        let changed_progress =
            test_loading_progress_overlay(4, mclone_ui::LoadingProgressCellStatus::Light);
        let changed = prepare_xr_menu_panel_draw(
            &mut ui,
            &mut overlay_cache,
            gui_scale,
            state,
            Some(&changed_progress),
            &status,
        );
        assert_eq!(
            changed.draw_cache,
            UiDrawCacheStats {
                rebuild_count: 1,
                cache_hit_count: 1,
            }
        );
        assert_eq!(changed.cache_revision, first.cache_revision);
        assert_ne!(changed.overlay_cache_revision, first.overlay_cache_revision);
        assert_eq!(changed.panel_draw, first.panel_draw);
        assert_ne!(changed.overlay_draw, first.overlay_draw);
    }

    #[test]
    fn xr_locomotion_maps_left_stick_and_right_a_to_engine_input() {
        let frame = test_xr_input(Vec2::new(0.0, 1.0), Vec2::ZERO, [PlayerAction::Jump]);
        let input = xr_locomotion_input_from_controllers(&frame, 1.0 / 72.0, Some(0.25));

        assert_eq!(input.dt_seconds, 1.0 / 72.0);
        assert!(input.jump);
        assert!(!input.descend);
        assert_eq!(
            input.movement_impulse,
            Some(EngineCameraMovementImpulse::new(0.0, 1.0))
        );
        assert_eq!(input.movement_yaw_radians, Some(0.25));
        assert_eq!(input.mouse_delta_x, 0.0);
    }

    #[test]
    fn xr_locomotion_maps_left_stick_lateral_axis_to_strafe() {
        let frame = test_xr_input(Vec2::new(1.0, 0.0), Vec2::ZERO, []);
        let input = xr_locomotion_input_from_controllers(&frame, 1.0 / 72.0, None);

        assert_eq!(
            input.movement_impulse,
            Some(EngineCameraMovementImpulse::new(-1.0, 0.0))
        );
        assert_eq!(input.movement_yaw_radians, None);
    }

    #[test]
    fn xr_locomotion_dead_zone_filters_small_thumbstick_noise() {
        let frame = test_xr_input(
            Vec2::splat(XR_JOYPAD_DEAD_ZONE * 0.25),
            Vec2::splat(XR_JOYPAD_DEAD_ZONE * 0.25),
            [],
        );
        let input = xr_locomotion_input_from_controllers(&frame, 1.0, None);

        assert_eq!(input.movement_impulse, None);
        assert_eq!(input.mouse_delta_x, 0.0);
    }

    #[test]
    fn xr_blink_teleport_engages_only_past_left_stick_threshold() {
        assert_eq!(XR_BLINK_TELEPORT_STICK_THRESHOLD, 0.75);
        assert!(!xr_left_stick_blink_engaged(&test_xr_input(
            Vec2::new(XR_BLINK_TELEPORT_STICK_THRESHOLD, 0.0),
            Vec2::ZERO,
            [],
        )));
        assert!(xr_left_stick_blink_engaged(&test_xr_input(
            Vec2::new(XR_BLINK_TELEPORT_STICK_THRESHOLD + 0.01, 0.0),
            Vec2::ZERO,
            [],
        )));
        assert!(xr_left_stick_blink_engaged(&test_xr_input(
            Vec2::new(0.0, -(XR_BLINK_TELEPORT_STICK_THRESHOLD + 0.01)),
            Vec2::ZERO,
            [],
        )));
    }

    #[test]
    fn xr_travel_assist_off_does_not_suppress_left_stick_movement() {
        let input = test_xr_input(
            Vec2::new(XR_BLINK_TELEPORT_STICK_THRESHOLD + 0.01, 0.0),
            Vec2::ZERO,
            [],
        );

        let frame = xr_blink_teleport_disabled_frame(GameTravelAssistMode::Off, &input)
            .expect("travel assist off disables Blink");

        assert!(!frame.suppress_left_stick_movement);
        assert!(xr_blink_teleport_disabled_frame(GameTravelAssistMode::Blink, &input).is_none());
    }

    #[test]
    fn xr_blink_heading_only_changes_past_heading_threshold() {
        assert_eq!(XR_BLINK_TELEPORT_HEADING_STICK_THRESHOLD, 0.9);
        let base_yaw_degrees = 17.0;
        let mut activation_angle = None;

        let below_heading_threshold = xr_blink_teleport_target_yaw_degrees_from_stick(
            base_yaw_degrees,
            base_yaw_degrees,
            &mut activation_angle,
            Vec2::new(0.0, XR_BLINK_TELEPORT_HEADING_STICK_THRESHOLD),
        );

        assert!((below_heading_threshold - base_yaw_degrees).abs() < 1.0e-9);
        assert!(activation_angle.is_none());

        let armed_heading = xr_blink_teleport_target_yaw_degrees_from_stick(
            base_yaw_degrees,
            base_yaw_degrees,
            &mut activation_angle,
            Vec2::new(1.0, 0.0),
        );
        let rotated_heading = xr_blink_teleport_target_yaw_degrees_from_stick(
            base_yaw_degrees,
            armed_heading,
            &mut activation_angle,
            Vec2::new(0.0, 1.0),
        );
        let release_noise_heading = xr_blink_teleport_target_yaw_degrees_from_stick(
            base_yaw_degrees,
            rotated_heading,
            &mut activation_angle,
            Vec2::new(-0.6, 0.5),
        );

        assert!((armed_heading - base_yaw_degrees).abs() < 1.0e-9);
        assert!((rotated_heading + 73.0).abs() < 1.0e-5);
        assert!((release_noise_heading - rotated_heading).abs() < 1.0e-9);
    }

    #[test]
    fn xr_blink_teleport_intent_uses_left_controller_world_aim() {
        let mut left = test_controller(XrHand::Left, Vec2::new(0.8, 0.0), false);
        left.aim_position = Some(Vec3::new(0.25, 1.2, -0.5));
        left.aim_direction = Some(Vec3::NEG_Z);
        let transform = XrStageToWorld {
            origin_stage: Vec3::ZERO,
            origin_world: Vec3::new(10.0, 64.0, -4.0),
            stage_to_world_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        let camera = EngineCameraController::from_eye_pose(
            Vec3d::new(10.0, 65.62, -4.0),
            std::f64::consts::FRAC_PI_2,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );

        let input = test_xr_tracked_input([left]);
        let intent = xr_blink_teleport_intent(&camera, &input, transform, 37.0).expect("intent");

        assert_eq!(intent.start_feet, camera.player().pose().position);
        assert_vec3d_close(intent.aim_origin, Vec3d::new(9.5, 65.2, -4.25));
        assert_vec3d_close(intent.aim_direction, Vec3d::new(-1.0, 0.0, 0.0));
        assert_eq!(intent.current_yaw_degrees, 37.0);
    }

    #[test]
    fn xr_blink_heading_stick_delta_ignores_initial_stick_direction() {
        let base_yaw_degrees = 17.0;
        let start = std::f64::consts::PI;

        let target = target_yaw_degrees_from_stick_delta(base_yaw_degrees, start, start);

        assert!((target - base_yaw_degrees).abs() < 1.0e-9);
    }

    #[test]
    fn xr_blink_heading_stick_delta_rotates_from_left_to_top() {
        let left = std::f64::consts::PI;
        let top = std::f64::consts::FRAC_PI_2;

        let target = target_yaw_degrees_from_stick_delta(0.0, left, top);
        let forward = horizontal_forward_from_player_yaw_degrees(target);

        assert!((target - 90.0).abs() < 1.0e-9);
        assert_vec3d_close(forward, Vec3d::new(-1.0, 0.0, 0.0));
    }

    #[test]
    fn xr_blink_landing_yaw_matches_arrow_under_current_headset_pose() {
        let origin = XrTrackingOrigin::from_stage_view(
            Vec3::ZERO,
            Quat::IDENTITY,
            XrViewAlignmentMode::ViewPose,
        )
        .unwrap();
        let before_snapshot = EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(8.0, 72.0, -12.0),
            0.0,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let before_transform =
            XrStageToWorld::from_tracking_origin(origin, before_snapshot).unwrap();
        let left = test_xr_view(
            Vec3::new(-0.03, 0.0, 0.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        );
        let right = test_xr_view(
            Vec3::new(0.03, 0.0, 0.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        );
        let target_yaw_degrees = 0.0;

        let landing_yaw = xr_blink_teleport_landing_yaw_radians(
            before_snapshot.yaw_radians,
            &[left, right],
            before_transform,
            target_yaw_degrees,
        )
        .unwrap();
        let after_snapshot = EngineCameraSnapshot::from_eye_pose(
            before_snapshot.eye,
            landing_yaw,
            before_snapshot.pitch_radians,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let after_transform = XrStageToWorld::from_tracking_origin(origin, after_snapshot).unwrap();
        let headset_yaw = xr_headset_world_yaw_from_views(&[left, right], after_transform).unwrap();
        let desired_yaw = -target_yaw_degrees.to_radians() as f32;

        assert!(
            normalize_angle(headset_yaw - desired_yaw).abs() < 1.0e-6,
            "headset yaw {headset_yaw} did not match desired arrow yaw {desired_yaw}"
        );
    }

    #[test]
    fn xr_blink_heading_arrow_uses_player_view_direction() {
        assert_vec3d_close(
            horizontal_forward_from_player_yaw_degrees(0.0),
            Vec3d::new(0.0, 0.0, 1.0),
        );
        assert_vec3d_close(
            horizontal_forward_from_player_yaw_degrees(-90.0),
            Vec3d::new(1.0, 0.0, 0.0),
        );
    }

    #[test]
    fn xr_locomotion_default_snap_turn_does_not_emit_mouse_delta() {
        let frame = test_xr_input(Vec2::ZERO, Vec2::X, []);
        let input = xr_locomotion_input_from_controllers(&frame, 0.05, None);

        assert_eq!(input.mouse_delta_x, 0.0);
        assert_eq!(input.movement_impulse, None);
        assert!(!input.jump);
    }

    #[test]
    fn xr_locomotion_smooth_turn_policy_uses_mouse_turn_path() {
        let frame = test_xr_input(Vec2::ZERO, Vec2::X, []);
        let input = xr_locomotion_input_from_controllers_with_turn_policy(
            &frame,
            0.05,
            None,
            XrTurnPolicy::Smooth,
        );
        let expected_mouse_delta =
            mclone_input::ControllerSessionSettings::DEFAULT_LOOK_RATE_PER_SECOND as f64 * 0.05;

        assert!((input.mouse_delta_x - expected_mouse_delta).abs() < 1.0e-6);
        assert_eq!(input.movement_impulse, None);
        assert!(!input.jump);
    }

    #[test]
    fn xr_locomotion_maps_right_a_to_jump() {
        let frame = test_xr_input(Vec2::ZERO, Vec2::ZERO, [PlayerAction::Jump]);
        let input = xr_locomotion_input_from_controllers(&frame, 1.0 / 72.0, None);

        assert!(input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn xr_locomotion_maps_right_b_to_descend() {
        let frame = test_xr_input(Vec2::ZERO, Vec2::ZERO, [PlayerAction::Descend]);
        let input = xr_locomotion_input_from_controllers(&frame, 1.0 / 72.0, None);

        assert!(input.descend);
        assert!(!input.jump);
    }

    #[test]
    fn xr_locomotion_right_stick_up_no_longer_maps_to_jump() {
        let frame = test_xr_input(Vec2::ZERO, Vec2::Y, []);
        let input = xr_locomotion_input_from_controllers(&frame, 1.0 / 72.0, None);

        assert!(!input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn xr_locomotion_right_stick_down_no_longer_maps_to_descend() {
        let frame = test_xr_input(Vec2::ZERO, Vec2::NEG_Y, []);
        let input = xr_locomotion_input_from_controllers(&frame, 1.0 / 72.0, None);

        assert!(!input.descend);
        assert!(!input.jump);
    }

    #[test]
    fn xr_locomotion_pure_yaw_does_not_trigger_vertical_movement() {
        let frame = test_xr_input(Vec2::ZERO, Vec2::X, []);
        let input = xr_locomotion_input_from_controllers(&frame, 1.0 / 72.0, None);

        assert!(!input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn default_xr_turn_policy_is_snap_15() {
        assert_eq!(
            XrTurnPolicy::default(),
            XrTurnPolicy::Snap {
                degrees: XR_DEFAULT_SNAP_TURN_DEGREES
            }
        );
        assert_eq!(XrTurnPolicy::default().game_mode(), GameXrTurnMode::Snap15);
        assert_eq!(
            XrTurnPolicy::from_game_mode(GameXrTurnMode::Snap30),
            XrTurnPolicy::Snap { degrees: 30.0 }
        );
        assert_eq!(
            XrTurnPolicy::from_game_mode(GameXrTurnMode::Snap45),
            XrTurnPolicy::Snap { degrees: 45.0 }
        );
        assert_eq!(
            XrTurnPolicy::from_game_mode(GameXrTurnMode::Smooth),
            XrTurnPolicy::Smooth
        );
    }

    #[test]
    fn xr_snap_turn_state_emits_one_snap_per_deflection() {
        let mut state = XrSnapTurnState::default();
        let policy = XrTurnPolicy::Snap { degrees: 15.0 };

        let first = state.update(XR_SNAP_TURN_ENGAGE_THRESHOLD, policy);
        let held = state.update(1.0, policy);

        assert!((first.expect("snap") + 15.0_f64.to_radians()).abs() < 1.0e-12);
        assert_eq!(held, None);
    }

    #[test]
    fn xr_snap_turn_state_requires_recenter_before_next_snap() {
        let mut state = XrSnapTurnState::default();
        let policy = XrTurnPolicy::Snap { degrees: 30.0 };

        assert!(state.update(1.0, policy).is_some());
        assert_eq!(state.update(0.4, policy), None);
        assert_eq!(state.update(1.0, policy), None);
        assert_eq!(state.update(0.0, policy), None);
        let next = state.update(-1.0, policy).expect("snap after recenter");

        assert!((next - 30.0_f64.to_radians()).abs() < 1.0e-12);
    }

    #[test]
    fn xr_snap_turn_state_ignores_smooth_policy() {
        let mut state = XrSnapTurnState::default();

        assert_eq!(state.update(1.0, XrTurnPolicy::Smooth), None);
    }

    #[test]
    fn xr_automated_flight_moves_forward_without_vertical_input() {
        let input = xr_automated_flight_input(1.0 / 72.0, Some(0.25));

        assert_eq!(input.dt_seconds, 1.0 / 72.0);
        assert_eq!(
            input.movement_impulse,
            Some(EngineCameraMovementImpulse::new(0.0, 1.0))
        );
        assert_eq!(input.movement_yaw_radians, Some(0.25));
        assert!(!input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn xr_automated_orbit_advances_heading_by_speed_and_radius() {
        let elapsed = std::f64::consts::PI * XR_AUTOMATED_ORBIT_RADIUS_BLOCKS / 4.0;
        let input = xr_automated_orbit_input(1.0 / 72.0, 4.0, elapsed);

        assert_eq!(input.dt_seconds, 1.0 / 72.0);
        assert_eq!(
            input.movement_impulse,
            Some(EngineCameraMovementImpulse::new(0.0, 1.0))
        );
        let yaw = input.movement_yaw_radians.expect("orbit yaw");
        assert!((yaw - std::f64::consts::PI).abs() < 1.0e-9);
        assert!(!input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn xr_hand_push_input_uses_grip_poses_in_world_space() {
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.grip_position = Some(Vec3::new(-0.25, -1.0, -0.35));
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.grip_position = Some(Vec3::new(0.25, -1.0, -0.35));
        let views = [
            test_xr_view(Vec3::new(-0.03, 0.0, 0.0), Quat::IDENTITY),
            test_xr_view(Vec3::new(0.03, 0.0, 0.0), Quat::IDENTITY),
        ];
        let transform = XrStageToWorld {
            origin_stage: Vec3::ZERO,
            origin_world: Vec3::new(8.0, 72.0, -12.0),
            stage_to_world_rotation: Quat::IDENTITY,
        };

        let input = xr_hand_push_input_from_controllers(&[left, right], &views, transform)
            .unwrap()
            .expect("both hands available");

        assert_vec3d_close(input.head_position, Vec3d::new(8.0, 72.0, -12.0));
        assert_vec3d_close(input.left_hand_position, Vec3d::new(7.75, 71.0, -12.35));
        assert_vec3d_close(input.right_hand_position, Vec3d::new(8.25, 71.0, -12.35));
    }

    #[test]
    fn xr_menu_toggle_uses_left_select_only() {
        let active = test_xr_input(Vec2::ZERO, Vec2::ZERO, [PlayerAction::OpenMenu]);
        assert!(xr_menu_toggle_pressed(&active.actions));
        assert!(!xr_menu_toggle_pressed(&PlayerActionFrame::default()));
    }

    #[test]
    fn xr_game_ui_toggle_uses_left_thumbstick_click_only() {
        let active = test_xr_input(Vec2::ZERO, Vec2::ZERO, [PlayerAction::OpenBlockPalette]);
        assert!(xr_game_ui_toggle_pressed(&active.actions));
        assert!(!xr_game_ui_toggle_pressed(&PlayerActionFrame::default()));
    }

    #[test]
    fn xr_menu_panel_anchors_in_front_of_hmd_center() {
        let left = test_render_view(Vec3::new(-0.03, 64.0, 0.0));
        let right = test_render_view(Vec3::new(0.03, 64.0, 0.0));

        let panel = xr_menu_panel_from_render_views([left, right]);

        assert!(
            (panel.center - Vec3::new(0.0, 64.0, -XR_MENU_PANEL_DISTANCE_BLOCKS)).length() < 1.0e-6
        );
        assert!((panel.right - Vec3::X).length() < 1.0e-6);
        assert!((panel.up - Vec3::Y).length() < 1.0e-6);
        assert_eq!(panel.width, XR_MENU_PANEL_WIDTH_BLOCKS);
        assert_eq!(panel.height, xr_menu_panel_height_blocks());
    }

    #[test]
    fn xr_diagnostic_panel_anchors_centered_below_hmd() {
        let left = test_render_view(Vec3::new(-0.03, 64.0, 0.0));
        let right = test_render_view(Vec3::new(0.03, 64.0, 0.0));

        let panel = xr_diagnostic_panel_from_render_views([left, right]);

        assert!(
            (panel.center
                - Vec3::new(
                    XR_DIAGNOSTIC_PANEL_RIGHT_OFFSET_BLOCKS,
                    64.0 + XR_DIAGNOSTIC_PANEL_UP_OFFSET_BLOCKS,
                    -XR_DIAGNOSTIC_PANEL_DISTANCE_BLOCKS,
                ))
            .length()
                < 1.0e-6
        );
        assert!((panel.right - Vec3::X).length() < 1.0e-6);
        assert!((panel.up - Vec3::Y).length() < 1.0e-6);
        assert_eq!(panel.width, XR_DIAGNOSTIC_PANEL_WIDTH_BLOCKS);
        assert_eq!(panel.height, xr_diagnostic_panel_height_blocks());
    }

    #[test]
    fn xr_field_guide_panel_and_draw_project_discovery_progress() {
        let left = test_render_view(Vec3::new(-0.03, 64.0, 0.0));
        let right = test_render_view(Vec3::new(0.03, 64.0, 0.0));
        let panel = xr_field_guide_panel_from_render_views([left, right]);
        assert_eq!(panel.width, XR_FIELD_GUIDE_PANEL_WIDTH_BLOCKS);
        assert_eq!(panel.height, xr_field_guide_panel_height_blocks());

        assert!(
            xr_field_guide_draw(
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            )
            .commands()
            .is_empty()
        );
        let complete = mclone_protocol::MallardFieldGuideProgress::from_bits_retain(
            mclone_protocol::MallardFieldGuideProgress::KNOWN_MASK,
        );
        assert!(
            !xr_field_guide_draw(
                complete,
                Default::default(),
                Default::default(),
                Default::default(),
            )
            .commands()
            .is_empty()
        );
        let deer = mclone_protocol::DeerFieldGuideProgress::from_bits_retain(
            mclone_protocol::DeerFieldGuideProgress::KNOWN_MASK,
        );
        assert!(
            !xr_field_guide_draw(
                Default::default(),
                deer,
                Default::default(),
                Default::default(),
            )
            .commands()
            .is_empty()
        );
    }

    #[test]
    fn field_guide_notification_auto_dismisses_and_rearms_for_new_discoveries() {
        let start = MonotonicInstant::from_nanos(1_000_000_000);
        let one_mallard = FieldGuideProgressSnapshot::new(
            mclone_protocol::MallardFieldGuideProgress::from_bits_retain(1),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        let mallard_and_deer = FieldGuideProgressSnapshot::new(
            one_mallard.mallard,
            mclone_protocol::DeerFieldGuideProgress::from_bits_retain(1),
            Default::default(),
            Default::default(),
        );
        let mut initially_empty = FieldGuideNotificationState::default();
        assert_eq!(
            initially_empty.observe(start, FieldGuideProgressSnapshot::default()),
            None
        );
        assert_eq!(
            initially_empty.observe(start.saturating_add(Duration::from_secs(1)), one_mallard),
            Some(one_mallard)
        );

        let mut notification = FieldGuideNotificationState::default();

        assert_eq!(notification.observe(start, one_mallard), Some(one_mallard));
        assert_eq!(
            notification.observe(
                start.saturating_add(Duration::from_secs(5) - Duration::from_nanos(1)),
                one_mallard,
            ),
            Some(one_mallard)
        );
        assert_eq!(
            notification.observe(start.saturating_add(Duration::from_secs(5)), one_mallard),
            None
        );
        assert_eq!(
            notification.observe(
                start.saturating_add(Duration::from_secs(6)),
                mallard_and_deer
            ),
            Some(mallard_and_deer)
        );
        assert_eq!(
            notification.observe(
                start.saturating_add(Duration::from_secs(11) - Duration::from_nanos(1)),
                mallard_and_deer,
            ),
            Some(mallard_and_deer)
        );
        assert_eq!(
            notification.observe(
                start.saturating_add(Duration::from_secs(11)),
                mallard_and_deer
            ),
            None
        );

        let two_mallard_and_deer = FieldGuideProgressSnapshot::new(
            mclone_protocol::MallardFieldGuideProgress::from_bits_retain(3),
            mallard_and_deer.deer,
            Default::default(),
            Default::default(),
        );
        assert_eq!(
            notification.observe(
                start.saturating_add(Duration::from_secs(12)),
                two_mallard_and_deer,
            ),
            Some(two_mallard_and_deer)
        );
    }

    #[test]
    fn field_guide_notification_treats_progress_regression_as_a_new_baseline() {
        let start = MonotonicInstant::from_nanos(1_000_000_000);
        let one_mallard = FieldGuideProgressSnapshot::new(
            mclone_protocol::MallardFieldGuideProgress::from_bits_retain(1),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        let two_mallard = FieldGuideProgressSnapshot::new(
            mclone_protocol::MallardFieldGuideProgress::from_bits_retain(3),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        let mut notification = FieldGuideNotificationState::default();

        assert_eq!(notification.observe(start, one_mallard), Some(one_mallard));
        assert_eq!(
            notification.observe(start.saturating_add(Duration::from_secs(1)), two_mallard),
            Some(two_mallard)
        );
        assert_eq!(
            notification.observe(start.saturating_add(Duration::from_secs(2)), one_mallard),
            None
        );
    }

    #[test]
    fn xr_game_ui_panel_anchors_to_left_hand_and_faces_hmd() {
        let views = [
            test_render_view(Vec3::new(-0.03, 64.0, 0.0)),
            test_render_view(Vec3::new(0.03, 64.0, 0.0)),
        ];
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.grip_position = Some(Vec3::new(0.0, 64.0, -1.0));

        let panel = xr_game_ui_panel_from_controllers(&[left], test_stage_to_world(), views)
            .expect("hand panel");

        assert!(
            (panel.center
                - Vec3::new(
                    0.0,
                    64.0 + XR_GAME_UI_PANEL_UP_OFFSET_BLOCKS,
                    -1.0 + XR_GAME_UI_PANEL_FORWARD_OFFSET_BLOCKS
                ))
            .length()
                < 1.0e-6
        );
        assert!((panel.right - Vec3::X).length() < 1.0e-6);
        assert!((panel.up - Vec3::Y).length() < 1.0e-6);
        assert_eq!(panel.width, XR_GAME_UI_PANEL_WIDTH_BLOCKS);
        assert_eq!(panel.height, xr_game_ui_panel_height_blocks());
    }

    #[test]
    fn xr_menu_panel_pointer_hit_maps_world_ray_to_gui_point() {
        let panel = WorldGuiPanel::new(Vec3::new(0.0, 64.0, -2.0), Vec3::X, Vec3::Y, 2.0, 1.0);
        let scale = GuiScale::from_pixels(1000, 500);

        let (point, distance) =
            xr_menu_panel_pointer_hit(panel, scale, Vec3::new(0.25, 64.25, 0.0), Vec3::NEG_Z)
                .unwrap();

        assert_eq!(
            point,
            Point {
                x: scale.width * 0.625,
                y: scale.height * 0.25
            }
        );
        assert!((distance - 2.0).abs() < 1.0e-6);
        assert!(
            xr_menu_panel_pointer_hit(panel, scale, Vec3::new(2.0, 64.0, 0.0), Vec3::NEG_Z)
                .is_none()
        );
    }

    #[test]
    fn xr_menu_pointer_prefers_right_controller_hit() {
        let panel = WorldGuiPanel::new(Vec3::new(0.0, 64.0, -2.0), Vec3::X, Vec3::Y, 2.0, 1.0);
        let scale = GuiScale::from_pixels(1000, 500);
        let transform = test_stage_to_world();
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.aim_position = Some(Vec3::new(-0.25, 64.0, 0.0));
        left.aim_direction = Some(Vec3::NEG_Z);
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.aim_position = Some(Vec3::new(0.25, 64.0, 0.0));
        right.aim_direction = Some(Vec3::NEG_Z);
        let input = test_xr_pointer_input([left, right], 0.25, 0.75);

        let hit = xr_menu_pointer_hit_from_controllers(&input, transform, panel, scale)
            .expect("right controller hit");

        assert_eq!(hit.hand, XrHand::Right);
        assert_eq!(hit.trigger, 0.75);
        assert_eq!(hit.point.x, scale.width * 0.625);
    }

    #[test]
    fn xr_menu_controller_ray_line_clips_at_panel_hit() {
        let panel = WorldGuiPanel::new(Vec3::new(0.0, 64.0, -2.0), Vec3::X, Vec3::Y, 2.0, 1.0);
        let transform = test_stage_to_world();
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.aim_position = Some(Vec3::new(0.25, 64.0, 0.0));
        right.aim_direction = Some(Vec3::NEG_Z);

        let input = test_xr_pointer_input([right], 0.0, 0.0);
        let lines = xr_menu_controller_ray_lines_from_controllers(&input, transform, panel);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start, Vec3::new(0.25, 64.0, 0.0));
        assert!((lines[0].end - Vec3::new(0.25, 64.0, -2.0)).length() < 1.0e-6);
        assert_eq!(lines[0].color, XR_MENU_RIGHT_RAY_COLOR);
    }

    #[test]
    fn xr_menu_controller_ray_line_uses_full_length_when_panel_missed() {
        let panel = WorldGuiPanel::new(Vec3::new(0.0, 64.0, -2.0), Vec3::X, Vec3::Y, 2.0, 1.0);
        let transform = test_stage_to_world();
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.aim_position = Some(Vec3::new(2.0, 64.0, 0.0));
        left.aim_direction = Some(Vec3::NEG_Z);

        let input = test_xr_pointer_input([left], 0.0, 0.0);
        let lines = xr_menu_controller_ray_lines_from_controllers(&input, transform, panel);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start, Vec3::new(2.0, 64.0, 0.0));
        assert!(
            (lines[0].end - Vec3::new(2.0, 64.0, -XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS)).length()
                < 1.0e-6
        );
        assert_eq!(lines[0].color, XR_MENU_LEFT_RAY_COLOR);
    }

    #[test]
    fn xr_menu_controller_ray_color_highlights_trigger_press() {
        assert_eq!(
            xr_menu_controller_ray_color(XrHand::Right, 0.0),
            XR_MENU_RIGHT_RAY_COLOR
        );

        assert_eq!(
            xr_menu_controller_ray_color(XrHand::Right, XR_MENU_POINTER_TRIGGER_PRESS),
            XR_MENU_TRIGGER_RAY_COLOR
        );
    }

    #[test]
    fn xr_menu_trigger_uses_hysteresis() {
        assert!(!xr_menu_pointer_trigger_down(0.5, false));
        assert!(xr_menu_pointer_trigger_down(0.56, false));
        assert!(xr_menu_pointer_trigger_down(0.4, true));
        assert!(!xr_menu_pointer_trigger_down(0.3, true));
    }

    #[test]
    fn emulated_xr_desktop_profile_drives_catalog_session_flow_through_facade() {
        let mut harness = EmulatedXrDesktopProfileHarness::new();

        assert_eq!(harness.ui.screen(), Some(GameScreen::Title));
        assert_eq!(
            harness.click_widget("Singleplayer"),
            GameUiAction::OpenWorldList
        );
        harness.route_emulated_ui_action(GameUiAction::OpenWorldList);
        assert_eq!(harness.ui.screen(), Some(GameScreen::WorldList));
        assert!(harness.has_widget("Existing World"));

        assert_eq!(
            harness.click_widget("Existing World"),
            GameUiAction::SelectWorld(harness.existing_ui_id())
        );
        harness.route_emulated_ui_action(GameUiAction::SelectWorld(harness.existing_ui_id()));
        assert_eq!(
            harness.click_widget("Open"),
            GameUiAction::OpenWorld(harness.existing_ui_id())
        );
        harness.route_emulated_ui_action(GameUiAction::OpenWorld(harness.existing_ui_id()));
        assert_eq!(
            harness.started_sessions.pop(),
            Some(SessionStartRequest::open_local_world(
                harness.existing_world_id.clone()
            ))
        );

        harness.ui.set_screen(Some(GameScreen::Title));
        assert_eq!(
            harness.click_widget("Singleplayer"),
            GameUiAction::OpenWorldList
        );
        harness.route_emulated_ui_action(GameUiAction::OpenWorldList);
        assert_eq!(harness.ui.screen(), Some(GameScreen::WorldList));
        assert_eq!(
            harness.click_widget("Create"),
            GameUiAction::OpenWorldCreate
        );
        harness.route_emulated_ui_action(GameUiAction::OpenWorldCreate);
        assert_eq!(harness.ui.screen(), Some(GameScreen::WorldCreate));
        assert_eq!(harness.ui.new_world_seed(), EMULATED_XR_CREATE_SEED);

        assert_eq!(
            harness.click_widget("Use Homestead Showcase"),
            GameUiAction::ApplyHomesteadShowcasePreset
        );
        harness.route_emulated_ui_action(GameUiAction::ApplyHomesteadShowcasePreset);
        assert_eq!(harness.ui.new_world_seed(), 0);
        assert!(harness.has_widget("World: Mclone Overworld"));
        assert!(harness.has_widget("Start: Homestead Start"));

        assert_eq!(
            harness.click_widget("Create World"),
            GameUiAction::CreateCatalogWorld
        );
        harness.route_emulated_ui_action(GameUiAction::CreateCatalogWorld);

        let created = harness
            .started_sessions
            .pop()
            .expect("catalog create queued a session start");
        match created {
            SessionStartRequest::CreateLocalWorld { options } => {
                assert_eq!(options.seed, 0);
                assert_eq!(options.display_name, "New World");
                assert!(options.requested_id.is_some());
                assert_eq!(
                    options.world_generation_profile,
                    mclone_server::WorldGenerationProfile::McloneOverworldV1
                );
                assert_eq!(
                    options.starter_content,
                    mclone_server::StarterContentDescriptor::IntroHomesteadV1
                );
            }
            other => panic!("expected local world creation start, got {other:?}"),
        }

        harness.ui.set_screen(Some(GameScreen::Title));
        harness.route_emulated_ui_action(GameUiAction::OpenWorldList);
        let created_ui_id = harness.ui_id_for_world("New World");
        harness.route_emulated_ui_action(GameUiAction::SelectWorld(created_ui_id));
        harness.route_emulated_ui_action(GameUiAction::OpenWorld(created_ui_id));
        assert_eq!(
            harness.started_sessions.pop(),
            Some(SessionStartRequest::open_local_world(
                LocalWorldId::new("new-world").unwrap()
            ))
        );

        harness.ui.set_screen(Some(GameScreen::Title));
        harness.route_emulated_ui_action(GameUiAction::OpenWorldList);
        let delete_id = harness.existing_ui_id();
        assert_eq!(
            harness.click_widget("Existing World"),
            GameUiAction::SelectWorld(delete_id)
        );
        harness.route_emulated_ui_action(GameUiAction::SelectWorld(delete_id));
        assert_eq!(
            harness.click_widget("Delete"),
            GameUiAction::ConfirmDeleteWorld(delete_id)
        );
        harness.route_emulated_ui_action(GameUiAction::ConfirmDeleteWorld(delete_id));
        assert_eq!(
            harness.ui.screen(),
            Some(GameScreen::WorldDeleteConfirm { id: delete_id })
        );
        assert_eq!(
            harness.click_widget("Delete"),
            GameUiAction::DeleteWorld(delete_id)
        );
        harness.route_emulated_ui_action(GameUiAction::DeleteWorld(delete_id));
        assert_eq!(harness.ui.screen(), Some(GameScreen::WorldList));
        assert!(!harness.has_widget("Existing World"));
    }

    #[test]
    fn xr_profile_supports_frame_pipeline_overlay() {
        let profile = xr_native_client_experience_profile();

        assert!(profile.settings.frame_pipeline_overlay.is_supported());
    }

    #[test]
    fn xr_profile_supports_debug_diagnostics() {
        let profile = xr_native_client_experience_profile();

        assert!(profile.settings.debug_diagnostics.is_supported());
    }

    const EMULATED_XR_CREATE_SEED: i64 = 24_680;

    struct EmulatedXrDesktopProfileHarness {
        profile: EmulatedXrDesktopProfileFacts,
        ui: GameUiHost,
        overlay_cache: XrMenuPanelOverlayCache,
        client_experience: ClientExperienceController,
        worlds: Vec<LocalWorldSummary>,
        existing_world_id: LocalWorldId,
        started_sessions: Vec<SessionStartRequest>,
    }

    impl EmulatedXrDesktopProfileHarness {
        fn new() -> Self {
            let profile = EmulatedXrDesktopProfileFacts::default();
            let existing_world_id = LocalWorldId::new("existing-world").unwrap();
            let existing =
                LocalWorldSummary::new(existing_world_id.clone(), "Existing World", 98_765, 1_000)
                    .unwrap();
            let mut client_experience =
                ClientExperienceController::new(xr_native_client_experience_profile());
            client_experience
                .catalog_mut()
                .set_capabilities(WorldCatalogCapabilities::persistent_local());
            Self {
                profile,
                ui: GameUiHost::new(),
                overlay_cache: XrMenuPanelOverlayCache::default(),
                client_experience,
                worlds: vec![existing],
                existing_world_id,
                started_sessions: Vec::new(),
            }
        }

        fn existing_ui_id(&self) -> mclone_ui::WorldCatalogUiWorldId {
            self.ui_id_for_world("Existing World")
        }

        fn ui_id_for_world(&self, display_name: &str) -> mclone_ui::WorldCatalogUiWorldId {
            self.client_experience
                .catalog()
                .ui_state()
                .entries
                .iter()
                .flatten()
                .find(|entry| entry.display_name.as_str() == display_name)
                .unwrap_or_else(|| panic!("{display_name} world catalog row"))
                .id
        }

        fn render_state(&self) -> GameUiRenderState {
            GameUiRenderState {
                world_catalog: self.client_experience.catalog().ui_state(),
                xr_turn_mode: Some(GameXrTurnMode::Snap15),
                crosshair_visible: None,
                ..GameUiRenderState::default()
            }
        }

        fn prepare_panel(&mut self) {
            let render_state = self.render_state();
            let draw = prepare_xr_menu_panel_draw(
                &mut self.ui,
                &mut self.overlay_cache,
                self.profile.gui_scale,
                render_state,
                None,
                &StatusOverlay::hidden(),
            );
            assert!(
                !draw.panel_draw.commands().is_empty(),
                "active XR panel should render visible UI commands"
            );
        }

        fn has_widget(&mut self, label: &str) -> bool {
            self.prepare_panel();
            self.ui
                .v2_debug_snapshot()
                .expect("emulated XR panel has debug data")
                .widgets
                .iter()
                .any(|widget| widget.label == label)
        }

        fn click_widget(&mut self, label: &str) -> GameUiAction {
            self.prepare_panel();
            let rect = {
                let snapshot = self
                    .ui
                    .v2_debug_snapshot()
                    .expect("emulated XR panel has debug data");
                let widget = snapshot
                    .widgets
                    .iter()
                    .find(|widget| widget.label == label)
                    .unwrap_or_else(|| panic!("widget `{label}` not found in {snapshot:?}"));
                assert!(widget.enabled, "widget `{label}` should be enabled");
                widget.rect
            };
            self.click_gui_point(point_in(rect))
                .unwrap_or_else(|| panic!("widget `{label}` did not emit an action"))
        }

        fn click_gui_point(&mut self, point: Point) -> Option<GameUiAction> {
            let pressed = self
                .profile
                .controller_for_gui_point(point, XR_MENU_POINTER_TRIGGER_PRESS);
            let press_hit = xr_menu_pointer_hit_from_controllers(
                &pressed,
                self.profile.transform,
                self.profile.panel,
                self.profile.gui_scale,
            )
            .expect("pressed controller ray hits emulated XR panel");
            assert_eq!(press_hit.hand, XrHand::Right);
            assert!(xr_menu_pointer_trigger_down(press_hit.trigger, false));
            assert_point_close(press_hit.point, point);
            let rays = xr_menu_controller_ray_lines_from_controllers(
                &pressed,
                self.profile.transform,
                self.profile.panel,
            );
            assert_eq!(rays.len(), 1);
            assert!(
                (rays[0].end - self.profile.world_point_for_gui_point(point)).length() < 1.0e-5
            );
            assert!(self.ui.pointer_down(press_hit.point));

            let released = self.profile.controller_for_gui_point(point, 0.0);
            let release_hit = xr_menu_pointer_hit_from_controllers(
                &released,
                self.profile.transform,
                self.profile.panel,
                self.profile.gui_scale,
            )
            .expect("released controller ray hits emulated XR panel");
            assert!(!xr_menu_pointer_trigger_down(release_hit.trigger, true));
            assert_point_close(release_hit.point, point);
            let (_handled, action) = self.ui.pointer_up(release_hit.point);
            action
        }

        fn route_emulated_ui_action(&mut self, action: GameUiAction) {
            let new_world_seed = if matches!(action, GameUiAction::OpenWorldCreate) {
                EMULATED_XR_CREATE_SEED
            } else {
                self.ui.new_world_seed()
            };
            let next_new_world_seed = matches!(
                action,
                GameUiAction::OpenNewWorld | GameUiAction::RerollSeed
            )
            .then_some(EMULATED_XR_CREATE_SEED + 1);
            let effects = self.client_experience.apply_ui_action(
                action,
                ClientExperienceActionContext {
                    new_world_seed,
                    next_new_world_seed,
                    current_join_remote_addr: self.ui.join_remote_addr(),
                    fallback_remote_addr: Some(DEFAULT_JOIN_REMOTE_ADDR),
                },
            );
            let apply_ui_action = client_experience_should_apply_ui_projection(action)
                && effects.projection.is_empty();
            self.apply_client_experience_effects(effects);
            if apply_ui_action {
                self.ui.apply_action(action);
            }
        }

        fn apply_client_experience_effects(&mut self, effects: ClientExperienceEffects) {
            self.apply_catalog_effects(effects.catalog);
            self.apply_session_effects(effects.session);
            assert!(
                effects.settings.is_empty(),
                "emulated XR menu/session flow should not emit settings effects"
            );
            assert!(
                effects.gameplay.is_empty(),
                "emulated XR menu/session flow should not emit gameplay effects"
            );
            assert!(
                effects.scenario.is_empty(),
                "emulated XR catalog flow should not emit scenario effects"
            );
            assert!(
                effects.local_data.is_empty(),
                "emulated XR catalog flow should not emit local-data effects"
            );
            for effect in effects.projection {
                match effect {
                    ClientExperienceProjectionEffect::ApplyUiAction(action) => {
                        self.ui.apply_action(action);
                    }
                    ClientExperienceProjectionEffect::SuppressUiAction => {}
                }
            }
        }

        fn apply_session_effects(&mut self, effects: ClientSessionEffects) {
            if let Some(seed) = effects.new_world_seed {
                self.ui.set_new_world_seed(seed);
            }
            if let Some(addr) = effects.join_remote_addr {
                self.ui.set_join_remote_addr(addr);
            }
            assert!(
                effects.session_start.is_none(),
                "catalog create/open should own persistent-world session starts"
            );
            assert!(
                effects.host_action.is_none(),
                "emulated XR flow should not emit host process actions"
            );
        }

        fn apply_catalog_effects(&mut self, effects: ClientCatalogEffects) {
            for start in effects.session_starts {
                self.started_sessions.push(start.request);
            }
            for request in effects.catalog_requests {
                let response = self.catalog_response(request.request);
                let effects = self
                    .client_experience
                    .catalog_mut()
                    .apply_catalog_response(request.id, response);
                self.apply_catalog_effects(effects);
            }
        }

        fn catalog_response(&mut self, request: WorldCatalogRequest) -> WorldCatalogResponse {
            match request {
                WorldCatalogRequest::ListWorlds => WorldCatalogResponse::WorldList {
                    capabilities: WorldCatalogCapabilities::persistent_local(),
                    worlds: self.worlds.clone(),
                },
                WorldCatalogRequest::CreateWorld { options } => {
                    let id = options.requested_id.clone().unwrap_or_else(|| {
                        LocalWorldId::available_from_display_name(
                            &options.display_name,
                            self.worlds.iter().map(|world| &world.id),
                        )
                    });
                    let mut summary = LocalWorldSummary::new(
                        id,
                        options.display_name.clone(),
                        options.seed,
                        2_000 + self.worlds.len() as u64,
                    )
                    .unwrap();
                    summary.world_generation_profile = options.world_generation_profile;
                    summary.starter_content = options.starter_content;
                    self.worlds.push(summary.clone());
                    WorldCatalogResponse::WorldCreated { summary }
                }
                WorldCatalogRequest::OpenWorld { id } => {
                    let summary = self
                        .worlds
                        .iter()
                        .find(|world| world.id == id)
                        .unwrap_or_else(|| panic!("emulated catalog missing world `{id}`"))
                        .clone();
                    WorldCatalogResponse::WorldOpened { summary }
                }
                WorldCatalogRequest::RecordWorldPlayed { id } => {
                    let summary = self
                        .worlds
                        .iter_mut()
                        .find(|world| world.id == id)
                        .unwrap_or_else(|| panic!("emulated catalog missing world `{id}`"));
                    summary.last_played_unix_millis = Some(3_000);
                    WorldCatalogResponse::WorldPlayRecorded {
                        summary: summary.clone(),
                    }
                }
                WorldCatalogRequest::DeleteWorld { id } => {
                    let index = self
                        .worlds
                        .iter()
                        .position(|world| world.id == id)
                        .unwrap_or_else(|| panic!("emulated catalog missing world `{id}`"));
                    let summary = self.worlds.remove(index);
                    WorldCatalogResponse::WorldDeleted { id: summary.id }
                }
                WorldCatalogRequest::DeleteAllLocalWorlds { .. } => {
                    let deleted_count = self.worlds.len();
                    self.worlds.clear();
                    WorldCatalogResponse::AllLocalWorldsDeleted { deleted_count }
                }
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct EmulatedXrDesktopProfileFacts {
        panel: WorldGuiPanel,
        transform: XrStageToWorld,
        gui_scale: GuiScale,
    }

    impl Default for EmulatedXrDesktopProfileFacts {
        fn default() -> Self {
            let render_views = [
                test_render_view(Vec3::new(-0.03, 64.0, 0.0)),
                test_render_view(Vec3::new(0.03, 64.0, 0.0)),
            ];
            let panel = xr_menu_panel_from_render_views(render_views);
            assert!(
                (panel.center - Vec3::new(0.0, 64.0, -XR_MENU_PANEL_DISTANCE_BLOCKS)).length()
                    < 1.0e-6
            );
            Self {
                panel,
                transform: test_stage_to_world(),
                gui_scale: GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]),
            }
        }
    }

    impl EmulatedXrDesktopProfileFacts {
        fn world_point_for_gui_point(self, point: Point) -> Vec3 {
            let x = point.x / self.gui_scale.width * self.panel.width - self.panel.width * 0.5;
            let y = self.panel.height * 0.5 - point.y / self.gui_scale.height * self.panel.height;
            self.panel.center + self.panel.right * x + self.panel.up * y
        }

        fn controller_for_gui_point(self, point: Point, trigger: f32) -> XrInputFrame {
            let target = self.world_point_for_gui_point(point);
            let normal = self.panel.right.cross(self.panel.up).normalize();
            let origin = target + normal * 0.75;
            let mut controller = test_controller(XrHand::Right, Vec2::ZERO, false);
            controller.aim_position = Some(origin);
            controller.aim_direction = Some((target - origin).normalize());
            controller.grip_position = Some(origin + Vec3::new(0.0, -0.12, 0.0));
            test_xr_pointer_input([controller], 0.0, trigger)
        }
    }

    fn assert_point_close(actual: Point, expected: Point) {
        assert!(
            (actual.x - expected.x).abs() < 1.0e-4 && (actual.y - expected.y).abs() < 1.0e-4,
            "expected point {actual:?} to be close to {expected:?}"
        );
    }

    #[test]
    fn xr_controller_interaction_ray_prefers_right_hand() {
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.aim_position = Some(Vec3::new(-1.0, 0.0, 0.0));
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.aim_position = Some(Vec3::new(1.0, 0.0, 0.0));

        let (origin, direction) =
            xr_controller_interaction_ray_from_controllers(&[left, right], test_stage_to_world())
                .expect("right hand ray");

        assert_eq!(origin, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(direction, Vec3::NEG_Z);
    }

    #[test]
    fn xr_pose_less_actions_use_head_gaze_and_tracked_ray_wins() {
        let gaze = Some((Vec3::new(0.0, 65.6, 0.0), Vec3::NEG_Z));
        let empty = XrInputFrame::default();
        assert_eq!(
            xr_interaction_ray_with_head_fallback(&empty, test_stage_to_world(), gaze),
            gaze
        );

        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.aim_position = Some(Vec3::new(1.0, 64.0, 0.0));
        right.aim_direction = Some(Vec3::NEG_X);
        let tracked = test_xr_tracked_input([right]);
        assert_eq!(
            xr_interaction_ray_with_head_fallback(&tracked, test_stage_to_world(), gaze),
            Some((Vec3::new(1.0, 64.0, 0.0), Vec3::NEG_X))
        );

        let transformed = XrStageToWorld {
            origin_stage: Vec3::ZERO,
            origin_world: Vec3::new(10.0, 64.0, 5.0),
            stage_to_world_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        let (origin, direction) = xr_interaction_ray_with_head_fallback(
            &empty,
            transformed,
            Some((Vec3::Y, Vec3::NEG_Z)),
        )
        .expect("transformed head-gaze fallback");
        assert!((origin - Vec3::new(10.0, 65.0, 5.0)).length() < 1.0e-5);
        assert!((direction - Vec3::NEG_X).length() < 1.0e-5);
    }

    #[test]
    fn xr_controller_interaction_ray_applies_stage_to_world_transform() {
        let transform = XrStageToWorld {
            origin_stage: Vec3::new(1.0, 0.0, 0.0),
            origin_world: Vec3::new(10.0, 70.0, -4.0),
            stage_to_world_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.aim_position = Some(Vec3::new(1.0, 1.0, -2.0));
        right.aim_direction = Some(Vec3::NEG_Z);

        let (origin, direction) =
            xr_controller_interaction_ray_from_controllers(&[right], transform)
                .expect("transformed ray");

        assert!((origin - Vec3::new(8.0, 71.0, -4.0)).length() < 1.0e-6);
        assert!((direction - Vec3::NEG_X).length() < 1.0e-6);
    }

    #[test]
    fn xr_gameplay_controller_ray_line_clips_to_hit_distance() {
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.aim_position = Some(Vec3::new(1.0, 64.0, 0.0));
        right.aim_direction = Some(Vec3::NEG_Z);
        let mut input = test_xr_pointer_input([right], 0.0, 0.0);
        input
            .xr_specific
            .controllers
            .iter_mut()
            .find(|controller| controller.hand == Some(XrHand::Right))
            .expect("right XR extension")
            .squeeze_value = XR_MENU_POINTER_TRIGGER_PRESS;
        let line = xr_gameplay_controller_ray_line_from_controllers(
            &input,
            test_stage_to_world(),
            Some(2.5),
            5.0,
        )
        .expect("gameplay ray line");

        assert_eq!(line.start, Vec3::new(1.0, 64.0, 0.0));
        assert!((line.end - Vec3::new(1.0, 64.0, -2.5)).length() < 1.0e-6);
        assert_eq!(line.color, XR_MENU_TRIGGER_RAY_COLOR);
    }

    #[test]
    fn startup_view_pose_maps_stage_center_to_requested_world_pose() {
        let stage_center = Vec3::new(1.0, 1.6, -0.25);
        let stage_orientation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let view_pose = XrStartupViewPose {
            position: [8.0, 72.0, -12.0],
            yaw_degrees: 0.0,
        };

        let origin = XrTrackingOrigin::from_stage_view(
            stage_center,
            stage_orientation,
            XrViewAlignmentMode::ViewPose,
        )
        .unwrap();
        let snapshot = EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(8.0, 72.0, -12.0),
            0.0,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let transform = XrStageToWorld::from_tracking_origin(origin, snapshot).unwrap();
        let (world_position, world_orientation) =
            transform.transform_pose(stage_center, stage_orientation);
        let world_forward = world_orientation * Vec3::NEG_Z;

        assert!((world_position - Vec3::from_array(view_pose.position)).length() < 1.0e-5);
        assert!((world_forward - Vec3::NEG_Z).length() < 1.0e-5);
        assert_eq!(origin.mode_label(), "view-pose");
    }

    #[test]
    fn player_root_transform_preserves_physical_hmd_offset() {
        let origin = XrTrackingOrigin::from_stage_view(
            Vec3::new(1.0, 1.6, -0.25),
            Quat::IDENTITY,
            XrViewAlignmentMode::ViewPose,
        )
        .unwrap();
        let snapshot = EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(8.0, 72.0, -12.0),
            0.0,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let transform = XrStageToWorld::from_tracking_origin(origin, snapshot).unwrap();

        let (world_position, _) = transform.transform_pose(
            origin.origin_stage + Vec3::new(0.35, 0.0, -0.2),
            Quat::IDENTITY,
        );

        assert!((world_position - Vec3::new(8.35, 72.0, -12.2)).length() < 1.0e-5);
    }

    #[test]
    fn consumed_room_scale_movement_advances_stage_origin_without_double_counting_head() {
        let origin = XrTrackingOrigin::from_stage_view(
            Vec3::new(1.0, 1.6, -0.25),
            Quat::IDENTITY,
            XrViewAlignmentMode::ViewPose,
        )
        .unwrap();
        let before_snapshot = EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(8.0, 72.0, -12.0),
            0.0,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let before_transform =
            XrStageToWorld::from_tracking_origin(origin, before_snapshot).unwrap();
        let current_head_stage = origin.origin_stage + Vec3::new(0.4, 0.2, -0.3);
        let head_world_before = before_transform.transform_position(current_head_stage);

        let consumed_world = Vec3::new(0.4, 0.0, -0.1);
        let origin_after = origin.consume_world_movement(consumed_world, before_transform);
        let after_snapshot = EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(8.4, 72.0, -12.1),
            0.0,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let after_transform =
            XrStageToWorld::from_tracking_origin(origin_after, after_snapshot).unwrap();
        let head_world_after = after_transform.transform_position(current_head_stage);
        let body_eye_after = glam_vec3_from_vec3d(after_snapshot.eye);

        assert!((head_world_after - head_world_before).length() < 1.0e-5);
        assert!((head_world_after - body_eye_after - Vec3::new(0.0, 0.2, -0.2)).length() < 1.0e-5);
    }

    #[test]
    fn tracking_origin_rebase_preserves_headset_world_position_across_snap_turn() {
        let origin = XrTrackingOrigin::from_stage_view(
            Vec3::new(1.0, 1.6, -0.25),
            Quat::IDENTITY,
            XrViewAlignmentMode::ViewPose,
        )
        .unwrap();
        let before_snapshot = EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(8.0, 72.0, -12.0),
            0.0,
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let before_transform =
            XrStageToWorld::from_tracking_origin(origin, before_snapshot).unwrap();
        let current_head_stage = origin.origin_stage + Vec3::new(0.35, 0.15, -0.3);
        let head_world_before = before_transform.transform_position(current_head_stage);

        let after_snapshot = EngineCameraSnapshot::from_eye_pose(
            before_snapshot.eye,
            15.0_f64.to_radians(),
            0.0,
            ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND,
        );
        let origin_after = origin
            .rebase_for_stage_position_world_position(
                current_head_stage,
                head_world_before,
                after_snapshot,
            )
            .unwrap();
        let after_transform =
            XrStageToWorld::from_tracking_origin(origin_after, after_snapshot).unwrap();
        let head_world_after = after_transform.transform_position(current_head_stage);

        assert!((head_world_after - head_world_before).length() < 1.0e-5);
    }

    #[test]
    fn fixed_startup_render_views_pin_center_and_yaw() {
        let view_pose = XrStartupViewPose {
            position: [8.0, 72.0, -12.0],
            yaw_degrees: 90.0,
        };
        let fov = XrFov {
            angle_left: -0.5,
            angle_right: 0.5,
            angle_up: 0.5,
            angle_down: -0.5,
        };

        let render_views =
            fixed_startup_view_pose_render_views(view_pose, [fov, fov], XR_FAR).unwrap();
        let center = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let forward = average_unit_direction(
            render_views[0].camera_forward,
            render_views[1].camera_forward,
            Vec3::NEG_Z,
        );

        assert!((center - Vec3::from_array(view_pose.position)).length() < 1.0e-6);
        assert!((yaw_from_forward(forward).unwrap() - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
        assert!(forward.y.abs() < 1.0e-6);
        assert_eq!(render_views[0].z_far, XR_FAR);
        assert_eq!(render_views[1].z_far, XR_FAR);

        let extended =
            fixed_startup_view_pose_render_views_with_far(view_pose, [fov, fov], 140_000.0)
                .unwrap();
        assert_eq!(extended[0].z_far, 140_000.0);
        assert_eq!(extended[1].z_far, 140_000.0);
    }

    #[test]
    fn headset_locomotion_yaw_uses_engine_movement_forward_convention() {
        let transform = test_stage_to_world();
        let left = test_xr_view(Vec3::new(-0.03, 0.0, 0.0), Quat::IDENTITY);
        let right = test_xr_view(Vec3::new(0.03, 0.0, 0.0), Quat::IDENTITY);

        let yaw = xr_headset_world_yaw_from_views(&[left, right], transform).unwrap();

        assert!((yaw - std::f32::consts::PI).abs() < 1.0e-6);
    }

    #[test]
    fn headset_locomotion_yaw_follows_stage_to_world_transform() {
        let transform = XrStageToWorld {
            origin_stage: Vec3::ZERO,
            origin_world: Vec3::ZERO,
            stage_to_world_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        let left = test_xr_view(Vec3::new(-0.03, 0.0, 0.0), Quat::IDENTITY);
        let right = test_xr_view(Vec3::new(0.03, 0.0, 0.0), Quat::IDENTITY);

        let yaw = xr_headset_world_yaw_from_views(&[left, right], transform).unwrap();

        assert!((yaw + std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
    }

    fn test_controller(
        hand: XrHand,
        _thumbstick: Vec2,
        _jump_pressed: bool,
    ) -> TrackedControllerState {
        TrackedControllerState {
            hand,
            aim_position: Some(Vec3::ZERO),
            aim_direction: Some(Vec3::NEG_Z),
            grip_position: Some(Vec3::ZERO),
            grip_orientation: Some(Quat::IDENTITY),
        }
    }

    fn test_xr_input(
        movement_axis: Vec2,
        turn_axis: Vec2,
        held: impl IntoIterator<Item = PlayerAction>,
    ) -> XrInputFrame {
        let held = held.into_iter().collect::<std::collections::BTreeSet<_>>();
        let mut assembler = mclone_input::XrInputFrameAssembler::new();
        assembler.sample(
            mclone_input::XrActionSnapshot {
                movement_axis,
                turn_axis,
                attack_value: f32::from(held.contains(&PlayerAction::Attack)),
                use_value: f32::from(held.contains(&PlayerAction::Use)),
                jump: held.contains(&PlayerAction::Jump),
                sprint: held.contains(&PlayerAction::Sprint),
                sneak: held.contains(&PlayerAction::Sneak),
                descend: held.contains(&PlayerAction::Descend),
                open_menu: held.contains(&PlayerAction::OpenMenu),
                open_block_palette: held.contains(&PlayerAction::OpenBlockPalette),
            },
            Vec::new(),
            mclone_input::XrSpecificInput {
                controllers: vec![
                    mclone_input::XrControllerSpecificState {
                        hand: Some(XrHand::Left),
                        locomotion_axis: movement_axis,
                        ..Default::default()
                    },
                    mclone_input::XrControllerSpecificState {
                        hand: Some(XrHand::Right),
                        turn_axis,
                        ..Default::default()
                    },
                ],
            },
        )
    }

    fn test_xr_tracked_input(
        tracked: impl IntoIterator<Item = TrackedControllerState>,
    ) -> XrInputFrame {
        XrInputFrame {
            tracked: tracked.into_iter().collect(),
            ..Default::default()
        }
    }

    fn test_xr_pointer_input(
        tracked: impl IntoIterator<Item = TrackedControllerState>,
        left_select: f32,
        right_select: f32,
    ) -> XrInputFrame {
        XrInputFrame {
            tracked: tracked.into_iter().collect(),
            xr_specific: mclone_input::XrSpecificInput {
                controllers: vec![
                    mclone_input::XrControllerSpecificState {
                        hand: Some(XrHand::Left),
                        pointer_select_value: left_select,
                        ..Default::default()
                    },
                    mclone_input::XrControllerSpecificState {
                        hand: Some(XrHand::Right),
                        pointer_select_value: right_select,
                        ..Default::default()
                    },
                ],
            },
            ..Default::default()
        }
    }

    fn assert_vec3d_close(actual: Vec3d, expected: Vec3d) {
        assert!(
            actual.distance_to_sqr(expected) < 1.0e-10,
            "expected {actual:?} to be close to {expected:?}"
        );
    }

    fn test_xr_view(position: Vec3, orientation: Quat) -> XrView {
        XrView {
            pose: XrViewPose {
                position,
                orientation,
            },
            fov: XrFov {
                angle_left: -0.5,
                angle_right: 0.5,
                angle_up: 0.5,
                angle_down: -0.5,
            },
        }
    }

    fn test_stage_to_world() -> XrStageToWorld {
        XrStageToWorld {
            origin_stage: Vec3::ZERO,
            origin_world: Vec3::ZERO,
            stage_to_world_rotation: Quat::IDENTITY,
        }
    }

    fn test_render_view(camera_position: Vec3) -> ChunkRenderView {
        ChunkRenderView {
            view: glam::Mat4::IDENTITY,
            projection: glam::Mat4::IDENTITY,
            view_projection: glam::Mat4::IDENTITY,
            camera_position,
            camera_forward: Vec3::NEG_Z,
            camera_right: Vec3::X,
            camera_up: Vec3::Y,
            aspect: 1.0,
            fov_y_radians: 1.0,
            z_near: XR_NEAR,
            z_far: XR_FAR,
            projection_kind: ChunkProjectionKind::External,
        }
    }
}
pub use asset_replacement::*;
