#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use glam::{Quat, Vec2, Vec3};
use mclone_app_runtime::client_catalog_policy::{ClientCatalogEffects, ClientCatalogRequest};
use mclone_app_runtime::client_experience::{
    ClientExperienceActionContext, ClientExperienceCapabilityProjection,
    ClientExperienceCapabilityStatus, ClientExperienceController, ClientExperienceEffects,
    ClientExperienceGameplayEffect, ClientExperienceProfile, ClientExperienceProjectionEffect,
    ClientExperienceSettingEffect, ClientExperienceSettingsEffects,
    ClientExperienceSettingsProfile, ClientExperienceSettingsState,
    client_experience_should_apply_ui_projection,
};
use mclone_app_runtime::client_session_policy::{
    ClientSessionEffects, ClientSessionHostAction, ClientSessionStatusProjection,
    ClientSessionTransitionEffects, ClientSessionUiEffects, client_session_failed_start_ui_effects,
    client_session_quit_to_title_transition, client_session_should_clear_inactive_status,
    client_session_status_projection,
};
use mclone_app_runtime::far_lod::{
    FarTerrainLodConfig, MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
    MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
};
use mclone_app_runtime::frame_render::{
    FullFrameGui, FullFrameRenderSummary, RenderStreamStats, record_render_section_update_stats,
    render_full_frame_for_view_with_prepared_stereo_draw_in_slot,
    render_full_frame_for_view_with_prepared_stereo_draw_timed_in_slot,
    render_view_with_underwater_effect,
};
use mclone_app_runtime::host_mode::{RemoteDedicatedServerSession, SingleViewHostMode};
use mclone_app_runtime::local_single_view::{
    LocalSingleViewSceneOptions, LocalSingleViewStartupPump, LocalSingleViewStartupStep,
    NativeSingleViewSceneRuntime, NativeSingleViewSessionRuntime,
};
use mclone_app_runtime::render_assets::TexturedMeshAssets;
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, GameSessionCoordinator, GameSessionState, SessionFailure,
    SessionStartRequest,
};
use mclone_app_runtime::world_catalog::{
    LocalWorldId, LocalWorldSummary, NativeWorldCatalog, WorldCatalogCapabilities,
    WorldCatalogError,
};
use mclone_app_runtime::{
    GameplayCommandTiming, GameplayCommandUpdatePolicy, RuntimePollDiagnostics,
    TraversalReadySectionCache, debug_block_palette_overlay, elapsed_ms,
    set_player_appearance_command_for_ui_model,
};
use mclone_assets::AssetSource;
use mclone_audio::{AudioEngine, landing_playback_for_impact};
use mclone_client::{
    BlockInteractionTarget, ClientInteractionController, HAND_PUSH_DEFAULT_HEAD_RADIUS,
    NativeTeleportPreviewWorker, TeleportConfig, TeleportIntent, TeleportPreview,
    TeleportPreviewRequestId, TeleportPreviewResult, sphere_intersects_solid_blocks,
    view_vector_from_rot_degrees,
};
use mclone_core::{Aabb, BlockStateId, ChunkPos, Vec3d, time};
use mclone_diagnostics::FramePipelineReport;
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::actor_assets::ActorTextureImage;
use mclone_render::chunk::{
    ChunkDepthTarget, ChunkMultiviewDepthTarget, ChunkMultiviewRenderTarget, ChunkProjectionKind,
    ChunkRenderTarget, ChunkRenderView, PreparedTexturedSectionStereoDraw,
    TexturedSectionDrawResources, TexturedSectionRecordCacheStats,
    TexturedSectionRecordPrepareStats, TexturedSectionRenderOptions, TexturedSectionRenderPhase,
    TexturedSectionRenderStats, TexturedSectionUploadReport, TexturedSectionUploadTiming,
};
use mclone_render::entity::{ActorDrawResources, ActorFigureSet, ActorInstance, ActorRenderStats};
use mclone_render::far_lod::FarTerrainLodRenderer;
use mclone_render::fog::RenderFog;
use mclone_render::gui::{WorldGuiLine, WorldGuiPanel, WorldGuiPanelRenderStats, WorldGuiRenderer};
use mclone_render::screen_effect::{
    ScreenEffectsRenderer, ScreenFadeOverlay, UnderwaterEffectState, UnderwaterOverlay,
};
use mclone_render::selection_outline::{SelectionOutline, SelectionOutlineRenderer};
use mclone_render::sky::overworld_clear_color;
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_render::uniform::{
    LEFT_EYE_VIEW_SLOT, PER_VIEW_UNIFORM_FRAME_COUNT, PerViewSlot, RIGHT_EYE_VIEW_SLOT,
};
use mclone_render_session::{
    ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MOUSE_SENSITIVITY,
    EngineCameraCollisionMode, EngineCameraController, EngineCameraInput,
    EngineCameraMovementImpulse, EngineCameraMovementMode, EngineCameraSnapshot,
    EngineDebugVisualOptions, EngineHandPushInput, EngineRoomScaleReconciliation,
    RenderSectionCacheUpdate, RenderSectionUploadCoordinator, RenderSectionUploadFramePolicy,
    RenderSectionUploadPhaseReport, actor_instances_from_presentations, engine_debug_world_lines,
};
use mclone_server::WorkerFrameMetrics;
use mclone_ui::{
    Color, DEFAULT_JOIN_REMOTE_ADDR, DebugOverlay, GameCollisionMode, GameFramePacingMode,
    GameMovementMode, GamePlayerModel, GameScreen, GameTravelAssistMode, GameUiAction, GameUiHost,
    GameUiRenderState, GameXrTurnMode, GuiDrawList, GuiScale, LoadingProgressOverlay, Point, Rect,
    StatusOverlay, UiDrawCacheStats, UiPanelRevision, WorldCatalogUiStatus,
    render_loading_progress_overlay, render_status_overlay,
};
use mclone_xr_host::{XrControllerSnapshot, XrHand};
use openxr as xr;

mod diagnostic_panel;
mod frame_pipeline_reporter;

use diagnostic_panel::XrDiagnosticPanel;
pub use frame_pipeline_reporter::{XrFramePipelineHostTiming, XrFramePipelineReporter};

pub const DEFAULT_XR_SEED: i64 = 12_345;
pub const DEFAULT_XR_CHUNK_X: i32 = 0;
pub const DEFAULT_XR_CHUNK_Z: i32 = 0;
pub const DEFAULT_XR_RENDER_DISTANCE: u32 = 5;
pub const MAX_XR_RENDER_DISTANCE: u32 = 16;
pub const XR_NEAR: f32 = 0.05;
pub const XR_FAR: f32 = 700.0;
pub const XR_JOYPAD_DEAD_ZONE: f32 = 0.18;
pub const XR_JOYPAD_YAW_SPEED_RADIANS_PER_SECOND: f64 = 1.6;
pub const XR_DEFAULT_SNAP_TURN_DEGREES: f32 = 15.0;
pub const XR_SNAP_TURN_ENGAGE_THRESHOLD: f32 = 0.65;
pub const XR_SNAP_TURN_RECENTER_THRESHOLD: f32 = 0.25;
pub const XR_LOCOMOTION_MAX_FRAME_SECONDS: f64 = 0.1;
pub const XR_BLINK_TELEPORT_STICK_THRESHOLD: f32 = 0.75;
pub const XR_BLINK_TELEPORT_HEADING_STICK_THRESHOLD: f32 = 0.9;
pub const XR_AUTOMATED_ORBIT_RADIUS_BLOCKS: f64 = 16.0;
pub const XR_MENU_TOGGLE_HAND: XrHand = XrHand::Left;
pub const XR_UI_FPS_CAP: u32 = 90;
pub const XR_MENU_PANEL_PIXELS: [u32; 2] = [1024, 576];
pub const XR_MENU_PANEL_DISTANCE_BLOCKS: f32 = 2.2;
pub const XR_MENU_PANEL_WIDTH_BLOCKS: f32 = 1.75;
pub const XR_DIAGNOSTIC_PANEL_PIXELS: [u32; 2] = [336, 192];
pub const XR_DIAGNOSTIC_PANEL_DISTANCE_BLOCKS: f32 = 2.25;
pub const XR_DIAGNOSTIC_PANEL_WIDTH_BLOCKS: f32 = 1.2;
pub const XR_DIAGNOSTIC_PANEL_RIGHT_OFFSET_BLOCKS: f32 = 0.0;
pub const XR_DIAGNOSTIC_PANEL_UP_OFFSET_BLOCKS: f32 = -0.46;
pub const XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS: f32 = 6.0;
pub const XR_MENU_POINTER_TRIGGER_PRESS: f32 = 0.55;
pub const XR_MENU_POINTER_TRIGGER_RELEASE: f32 = 0.35;
pub const XR_GAMEPLAY_INTERACTION_HAND: XrHand = XrHand::Right;
pub const XR_GAME_UI_TOGGLE_HAND: XrHand = XrHand::Left;
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum XrLocomotionMode {
    #[default]
    HeadsetYaw,
    PlayerYaw,
}

impl XrLocomotionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::HeadsetYaw => "headset-yaw",
            Self::PlayerYaw => "player-yaw",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum XrTurnPolicy {
    Snap { degrees: f32 },
    Smooth,
}

impl Default for XrTurnPolicy {
    fn default() -> Self {
        Self::Snap {
            degrees: XR_DEFAULT_SNAP_TURN_DEGREES,
        }
    }
}

impl XrTurnPolicy {
    pub const fn snap(degrees: f32) -> Self {
        Self::Snap { degrees }
    }

    pub fn game_mode(self) -> GameXrTurnMode {
        match self {
            Self::Snap { degrees } if (degrees - 30.0).abs() < f32::EPSILON => {
                GameXrTurnMode::Snap30
            }
            Self::Snap { .. } => GameXrTurnMode::Snap15,
            Self::Smooth => GameXrTurnMode::Smooth,
        }
    }

    pub fn from_game_mode(mode: GameXrTurnMode) -> Self {
        match mode {
            GameXrTurnMode::Snap15 => Self::Snap { degrees: 15.0 },
            GameXrTurnMode::Snap30 => Self::Snap { degrees: 30.0 },
            GameXrTurnMode::Smooth => Self::Smooth,
        }
    }

    fn snap_radians(self) -> Option<f64> {
        match self {
            Self::Snap { degrees } if degrees.is_finite() && degrees > 0.0 => {
                Some(f64::from(degrees).to_radians())
            }
            Self::Snap { .. } => Some(f64::from(XR_DEFAULT_SNAP_TURN_DEGREES).to_radians()),
            Self::Smooth => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XrSnapTurnState {
    waiting_for_recenter: bool,
}

impl XrSnapTurnState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, axis_x: f32, policy: XrTurnPolicy) -> Option<f64> {
        let axis_x = if axis_x.is_finite() { axis_x } else { 0.0 };
        let magnitude = axis_x.abs();
        if self.waiting_for_recenter {
            if magnitude <= XR_SNAP_TURN_RECENTER_THRESHOLD {
                self.waiting_for_recenter = false;
            }
            return None;
        }
        if magnitude < XR_SNAP_TURN_ENGAGE_THRESHOLD {
            return None;
        }
        let snap_radians = policy.snap_radians()?;
        self.waiting_for_recenter = true;
        Some(-f64::from(axis_x.signum()) * snap_radians)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct XrGameplayInteractionButtons {
    attack: bool,
    use_item: bool,
}

impl XrGameplayInteractionButtons {
    const fn press_edges(self, current: Self) -> XrGameplayInteractionEdges {
        XrGameplayInteractionEdges {
            attack: current.attack && !self.attack,
            use_item: current.use_item && !self.use_item,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct XrGameplayInteractionEdges {
    attack: bool,
    use_item: bool,
}

impl XrGameplayInteractionEdges {
    const fn any(self) -> bool {
        self.attack || self.use_item
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XrGameplayInteractionAction {
    Attack,
    Use,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum XrUiPanelAnchor {
    #[default]
    Head,
    LeftHand,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum XrUnderwaterDetectionMode {
    #[default]
    Midpoint,
    PerEye,
}

impl XrUnderwaterDetectionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Midpoint => "midpoint",
            Self::PerEye => "per-eye",
        }
    }

    pub fn parse_label(flag: &str, value: &str) -> Result<Self> {
        match value.trim() {
            "midpoint" | "middle" | "center" | "both" => Ok(Self::Midpoint),
            "per-eye" | "per_eye" | "eye" | "eyes" => Ok(Self::PerEye),
            value => bail!("{flag} must be midpoint or per-eye, got `{value}`"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XrDebugUiScreen {
    Pause,
    Controls,
}

impl XrDebugUiScreen {
    pub fn parse_label(flag: &str, value: &str) -> Result<Self> {
        match value.trim() {
            "pause" => Ok(Self::Pause),
            "controls" | "help" => Ok(Self::Controls),
            value => bail!("{flag} must be pause or controls, got `{value}`"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct XrSceneOptions {
    pub seed: i64,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub render_distance: u32,
    pub render_compile_worker_count: usize,
    pub movement_speed_multiplier: f32,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub debug_passive_showcase: bool,
    pub lighting_enabled: bool,
    pub far_lod: FarTerrainLodConfig,
    pub underwater_detection_mode: XrUnderwaterDetectionMode,
    pub debug_ui_screen: Option<XrDebugUiScreen>,
    pub skip_actors: bool,
    pub world_root: Option<PathBuf>,
    pub world_dir: Option<PathBuf>,
}

impl Default for XrSceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_XR_SEED,
            chunk_x: DEFAULT_XR_CHUNK_X,
            chunk_z: DEFAULT_XR_CHUNK_Z,
            render_distance: DEFAULT_XR_RENDER_DISTANCE,
            render_compile_worker_count:
                mclone_app_runtime::render_assets::DEFAULT_RENDER_SECTION_COMPILE_WORKERS,
            movement_speed_multiplier: ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            day_time_override: None,
            freeze_time: false,
            debug_passive_showcase: true,
            lighting_enabled: true,
            far_lod: FarTerrainLodConfig::default(),
            underwater_detection_mode: XrUnderwaterDetectionMode::default(),
            debug_ui_screen: None,
            skip_actors: false,
            world_root: None,
            world_dir: None,
        }
    }
}

impl XrSceneOptions {
    pub fn center(&self) -> ChunkPos {
        ChunkPos::new(self.chunk_x, self.chunk_z)
    }

    pub fn validated(self) -> Result<Self> {
        if self.render_distance == 0 || self.render_distance > MAX_XR_RENDER_DISTANCE {
            bail!(
                "XR render distance must be between 1 and {MAX_XR_RENDER_DISTANCE}, got {}",
                self.render_distance
            );
        }
        if self.render_compile_worker_count == 0 {
            bail!("XR render compile worker count must be greater than zero");
        }
        let min = ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER as f32;
        let max = ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER as f32;
        if !self.movement_speed_multiplier.is_finite()
            || !(min..=max).contains(&self.movement_speed_multiplier)
        {
            bail!(
                "XR movement speed multiplier must be between {min} and {max}, got {}",
                self.movement_speed_multiplier
            );
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrStartupViewPose {
    pub position: [f32; 3],
    pub yaw_degrees: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct XrHeadComfortState {
    pub alpha: f32,
    pub target_alpha: f32,
    pub blocked_residual: bool,
    pub head_penetrating: bool,
}

impl XrHeadComfortState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn update(&mut self, target: XrHeadComfortTarget, dt_seconds: f64) {
        self.target_alpha = target.alpha;
        self.blocked_residual = target.blocked_residual;
        self.head_penetrating = target.head_penetrating;

        let dt_seconds = if dt_seconds.is_finite() {
            dt_seconds.clamp(0.0, XR_LOCOMOTION_MAX_FRAME_SECONDS) as f32
        } else {
            0.0
        };
        let fade_seconds = if target.alpha > self.alpha {
            XR_HEAD_COMFORT_FADE_IN_SECONDS
        } else {
            XR_HEAD_COMFORT_FADE_OUT_SECONDS
        };
        let t = if fade_seconds > 0.0 {
            (dt_seconds / fade_seconds).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.alpha += (target.alpha - self.alpha) * t;
        if target.alpha == 0.0 && self.alpha < 0.005 {
            self.alpha = 0.0;
        }
    }

    fn overlay(self) -> Option<ScreenFadeOverlay> {
        (self.alpha > 0.005).then(|| ScreenFadeOverlay::black(self.alpha))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct XrHeadComfortTarget {
    alpha: f32,
    blocked_residual: bool,
    head_penetrating: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct XrBlinkTeleportState {
    active: bool,
    preview: Option<TeleportPreview>,
    first_request_id: Option<TeleportPreviewRequestId>,
    preview_request_id: Option<TeleportPreviewRequestId>,
    base_yaw_degrees: f64,
    target_yaw_degrees: f64,
    activation_left_stick_angle_radians: Option<f64>,
    last_submitted_intent: Option<TeleportIntent>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct XrBlinkTeleportFrame {
    suppress_left_stick_movement: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XrViewAlignmentMode {
    PlayerSpawn,
    ViewPose,
}

impl XrViewAlignmentMode {
    pub const fn label(self) -> &'static str {
        match self {
            XrViewAlignmentMode::PlayerSpawn => "player-spawn",
            XrViewAlignmentMode::ViewPose => "view-pose",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct XrTerrainFrameSummary {
    pub rendered_frames: u32,
    pub section_count: usize,
    pub drawn_section_count: usize,
    pub index_count: u32,
    pub drawn_index_count: u32,
    pub gui_command_count: usize,
    pub ui_panel: WorldGuiPanelRenderStats,
    pub ui_draw_cache: UiDrawCacheStats,
    pub ui_active: bool,
    pub local_startup_active: bool,
    pub actor_count: usize,
    pub drawn_actor_count: usize,
    pub head_comfort: XrHeadComfortState,
    pub timing: XrTerrainFrameTiming,
    pub upload: XrTerrainUploadSummary,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrTerrainFrameTiming {
    pub render_views_ms: f64,
    pub menu_pointer_ms: f64,
    pub runtime_upload_ms: f64,
    pub runtime_poll_ms: f64,
    pub runtime_sync_ms: f64,
    pub runtime_sync_unattributed_ms: f64,
    pub runtime_result_accept_ms: f64,
    pub runtime_dirty_seed_ms: f64,
    pub runtime_prepare_ms: f64,
    pub runtime_submit_ms: f64,
    pub runtime_submit_snapshot_ms: f64,
    pub runtime_submit_handoff_ms: f64,
    pub runtime_submit_handoff_worst_ms: f64,
    pub runtime_submit_request_count: usize,
    pub runtime_submit_request_build_ms: f64,
    pub runtime_submit_compiler_ms: f64,
    pub runtime_submit_compiler_worst_ms: f64,
    pub runtime_submit_compiler_capacity_check_ms: f64,
    pub runtime_submit_compiler_capacity_check_worst_ms: f64,
    pub runtime_submit_compiler_command_send_ms: f64,
    pub runtime_submit_compiler_command_send_worst_ms: f64,
    pub runtime_submit_compiler_command_lock_wait_ms: f64,
    pub runtime_submit_compiler_command_lock_wait_worst_ms: f64,
    pub runtime_submit_compiler_command_slot_select_ms: f64,
    pub runtime_submit_compiler_command_slot_select_worst_ms: f64,
    pub runtime_submit_compiler_command_slot_write_ms: f64,
    pub runtime_submit_compiler_command_slot_write_worst_ms: f64,
    pub runtime_submit_compiler_command_queue_push_ms: f64,
    pub runtime_submit_compiler_command_queue_push_worst_ms: f64,
    pub runtime_submit_compiler_command_notify_ms: f64,
    pub runtime_submit_compiler_command_notify_worst_ms: f64,
    pub runtime_submit_compiler_command_post_enqueue_ms: f64,
    pub runtime_submit_compiler_command_post_enqueue_worst_ms: f64,
    pub runtime_submit_compiler_pending_mark_ms: f64,
    pub runtime_submit_compiler_pending_mark_worst_ms: f64,
    pub runtime_submit_mark_inflight_ms: f64,
    pub runtime_submit_apply_ready_plan_ms: f64,
    pub runtime_submit_ready_update_ms: f64,
    pub runtime_submit_ready_section_count: usize,
    pub runtime_submit_deferred_section_count: usize,
    pub runtime_submit_dirty_chunk_count_before: usize,
    pub runtime_submit_dirty_chunk_count_after: usize,
    pub runtime_submit_dirty_section_count_before: usize,
    pub runtime_submit_dirty_section_count_after: usize,
    pub runtime_submit_inflight_section_count_before: usize,
    pub runtime_submit_inflight_section_count_after: usize,
    pub runtime_submit_request_target_section_count: usize,
    pub runtime_submit_request_target_section_count_worst: usize,
    pub runtime_submit_request_snapshot_count: usize,
    pub runtime_submit_request_snapshot_section_count: usize,
    pub runtime_submit_request_snapshot_section_count_worst: usize,
    pub runtime_submit_request_light_section_count: usize,
    pub runtime_submit_request_light_section_count_worst: usize,
    pub runtime_submit_request_revision_count: usize,
    pub runtime_submit_request_estimated_payload_bytes: usize,
    pub runtime_submit_request_estimated_payload_bytes_worst: usize,
    pub runtime_dispatcher_pending_jobs: usize,
    pub runtime_dispatcher_max_pending_jobs: usize,
    pub runtime_dispatcher_available_job_slots: usize,
    pub runtime_dispatcher_queued_compile_tasks: usize,
    pub runtime_gpu_upload_ms: f64,
    pub runtime_gpu_upload_pre_sync_ms: f64,
    pub runtime_gpu_upload_post_sync_ms: f64,
    pub runtime_upload_enqueue_ms: f64,
    pub runtime_upload_select_ms: f64,
    pub runtime_upload_apply_ms: f64,
    pub runtime_upload_apply_dirty_mark_ms: f64,
    pub runtime_upload_apply_remove_ms: f64,
    pub runtime_upload_apply_section_state_ms: f64,
    pub runtime_upload_apply_vertex_bytes_ms: f64,
    pub runtime_upload_apply_vertex_buffer_ms: f64,
    pub runtime_upload_apply_index_bytes_ms: f64,
    pub runtime_upload_apply_index_buffer_ms: f64,
    pub runtime_upload_apply_mesh_insert_ms: f64,
    pub runtime_upload_apply_mesh_upload_worst_ms: f64,
    pub runtime_ready_sections_ms: f64,
    pub runtime_ready_publish_ms: f64,
    pub shared_records_ms: f64,
    pub record_cache_prepare: TexturedSectionRecordPrepareStats,
    pub left_eye_ms: f64,
    pub right_eye_ms: f64,
    pub left_eye_render: XrTerrainEyeRenderTiming,
    pub right_eye_render: XrTerrainEyeRenderTiming,
    pub stereo_finish_ms: f64,
    pub stereo_submit_ms: f64,
    pub stereo_poll_wait_ms: f64,
    pub overlap_runtime_prefetch_ms: f64,
    pub overlap_runtime_prefetch_poll_ms: f64,
    pub overlap_runtime_prefetch_sync_ms: f64,
    pub overlap_runtime_prefetch_gpu_upload_ms: f64,
    pub overlap_runtime_prefetch_ready_sections_ms: f64,
    pub multiview_sky_ms: f64,
    pub multiview_terrain_ms: f64,
    pub multiview_actor_ms: f64,
    pub multiview_screen_effect_ms: f64,
    pub multiview_world_overlays_ms: f64,
    pub multiview_submit_ms: f64,
    pub multiview_poll_wait_ms: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrLocomotionTiming {
    pub input_ms: f64,
    pub camera_apply_ms: f64,
    pub commit_ms: f64,
    pub commit_server_command_ms: f64,
    pub commit_server_command_send_ms: f64,
    pub commit_server_command_drain_updates_ms: f64,
    pub commit_server_command_apply_updates_ms: f64,
    pub commit_server_command_apply_dirty_mark_ms: f64,
    pub commit_server_command_apply_client_updates_ms: f64,
    pub commit_server_command_updates: usize,
    pub commit_server_command_snapshot_updates: usize,
    pub commit_server_command_section_block_updates: usize,
    pub commit_server_command_unload_updates: usize,
    pub commit_position_updates_ms: f64,
    pub commit_interest_ms: f64,
    pub commit_interest_command_send_ms: f64,
    pub commit_interest_command_drain_updates_ms: f64,
    pub commit_interest_command_apply_updates_ms: f64,
    pub commit_interest_command_apply_dirty_mark_ms: f64,
    pub commit_interest_command_apply_client_updates_ms: f64,
    pub commit_interest_command_updates: usize,
    pub commit_interest_command_snapshot_updates: usize,
    pub commit_interest_command_section_block_updates: usize,
    pub commit_interest_command_unload_updates: usize,
    pub gameplay_interaction_ms: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct XrCameraCommitTiming {
    server_command_ms: f64,
    server_command_send_ms: f64,
    server_command_drain_updates_ms: f64,
    server_command_apply_updates_ms: f64,
    server_command_apply_dirty_mark_ms: f64,
    server_command_apply_client_updates_ms: f64,
    server_command_updates: usize,
    server_command_snapshot_updates: usize,
    server_command_section_block_updates: usize,
    server_command_unload_updates: usize,
    position_updates_ms: f64,
    interest_ms: f64,
    interest_command_send_ms: f64,
    interest_command_drain_updates_ms: f64,
    interest_command_apply_updates_ms: f64,
    interest_command_apply_dirty_mark_ms: f64,
    interest_command_apply_client_updates_ms: f64,
    interest_command_updates: usize,
    interest_command_snapshot_updates: usize,
    interest_command_section_block_updates: usize,
    interest_command_unload_updates: usize,
}

impl XrCameraCommitTiming {
    fn record_interest_command_timing(&mut self, timing: GameplayCommandTiming) {
        self.interest_command_send_ms = timing.send_ms;
        self.interest_command_drain_updates_ms = timing.drain_updates_ms;
        self.interest_command_apply_updates_ms = timing.apply_updates_ms;
        self.interest_command_apply_dirty_mark_ms = timing.apply_dirty_mark_ms;
        self.interest_command_apply_client_updates_ms = timing.apply_client_updates_ms;
        self.interest_command_updates = timing.updates;
        self.interest_command_snapshot_updates = timing.snapshot_updates;
        self.interest_command_section_block_updates = timing.section_block_updates;
        self.interest_command_unload_updates = timing.unload_updates;
    }
}

impl XrLocomotionTiming {
    fn record_commit_timing(&mut self, timing: XrCameraCommitTiming) {
        self.commit_server_command_ms = timing.server_command_ms;
        self.commit_server_command_send_ms = timing.server_command_send_ms;
        self.commit_server_command_drain_updates_ms = timing.server_command_drain_updates_ms;
        self.commit_server_command_apply_updates_ms = timing.server_command_apply_updates_ms;
        self.commit_server_command_apply_dirty_mark_ms = timing.server_command_apply_dirty_mark_ms;
        self.commit_server_command_apply_client_updates_ms =
            timing.server_command_apply_client_updates_ms;
        self.commit_server_command_updates = timing.server_command_updates;
        self.commit_server_command_snapshot_updates = timing.server_command_snapshot_updates;
        self.commit_server_command_section_block_updates =
            timing.server_command_section_block_updates;
        self.commit_server_command_unload_updates = timing.server_command_unload_updates;
        self.commit_position_updates_ms = timing.position_updates_ms;
        self.commit_interest_ms = timing.interest_ms;
        self.commit_interest_command_send_ms = timing.interest_command_send_ms;
        self.commit_interest_command_drain_updates_ms = timing.interest_command_drain_updates_ms;
        self.commit_interest_command_apply_updates_ms = timing.interest_command_apply_updates_ms;
        self.commit_interest_command_apply_dirty_mark_ms =
            timing.interest_command_apply_dirty_mark_ms;
        self.commit_interest_command_apply_client_updates_ms =
            timing.interest_command_apply_client_updates_ms;
        self.commit_interest_command_updates = timing.interest_command_updates;
        self.commit_interest_command_snapshot_updates = timing.interest_command_snapshot_updates;
        self.commit_interest_command_section_block_updates =
            timing.interest_command_section_block_updates;
        self.commit_interest_command_unload_updates = timing.interest_command_unload_updates;
    }

    fn record_interest_command_timing(&mut self, timing: GameplayCommandTiming) {
        self.commit_interest_command_send_ms = timing.send_ms;
        self.commit_interest_command_drain_updates_ms = timing.drain_updates_ms;
        self.commit_interest_command_apply_updates_ms = timing.apply_updates_ms;
        self.commit_interest_command_apply_dirty_mark_ms = timing.apply_dirty_mark_ms;
        self.commit_interest_command_apply_client_updates_ms = timing.apply_client_updates_ms;
        self.commit_interest_command_updates = timing.updates;
        self.commit_interest_command_snapshot_updates = timing.snapshot_updates;
        self.commit_interest_command_section_block_updates = timing.section_block_updates;
        self.commit_interest_command_unload_updates = timing.unload_updates;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrTerrainEyeRenderTiming {
    pub full_frame_ms: f64,
    pub sky_ms: f64,
    pub far_lod_ms: f64,
    pub terrain_opaque_ms: f64,
    pub terrain_translucent_ms: f64,
    pub prepare_ms: f64,
    pub cull_ms: f64,
    pub uniform_write_ms: f64,
    pub translucent_collect_ms: f64,
    pub translucent_sort_ms: f64,
    pub encode_ms: f64,
    pub section_encode_ms: f64,
    pub actor_ms: f64,
    pub screen_effect_ms: f64,
    pub gui_ms: f64,
    pub xr_fade_ms: f64,
    pub xr_selection_ms: f64,
    pub xr_world_lines_ms: f64,
    pub xr_world_panel_ms: f64,
    pub encoder_finish_ms: f64,
    pub submit_ms: f64,
    pub poll_wait_ms: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrTerrainUploadSummary {
    pub host_mode: XrTerrainHostMode,
    pub poll_changed: bool,
    pub poll_total_ms: f64,
    pub poll_drain_updates_ms: f64,
    pub poll_producer_read_ms: f64,
    pub poll_producer_decode_ms: f64,
    pub poll_producer_response_sequence: Option<u64>,
    pub poll_client_deferred_chunk_drop_ms: f64,
    pub poll_client_deferred_chunk_drop_items: usize,
    pub poll_client_deferred_chunk_drop_backlog_items: usize,
    pub poll_apply_updates_ms: f64,
    pub poll_dirty_mark_ms: f64,
    pub poll_client_apply_updates_ms: f64,
    pub poll_snapshot_update_apply_ms: f64,
    pub poll_snapshot_update_dirty_mark_ms: f64,
    pub poll_snapshot_update_client_apply_ms: f64,
    pub poll_section_block_update_apply_ms: f64,
    pub poll_section_block_update_dirty_mark_ms: f64,
    pub poll_section_block_update_client_apply_ms: f64,
    pub poll_unload_update_apply_ms: f64,
    pub poll_unload_update_dirty_mark_ms: f64,
    pub poll_unload_update_client_apply_ms: f64,
    pub poll_other_update_apply_ms: f64,
    pub poll_other_update_dirty_mark_ms: f64,
    pub poll_other_update_client_apply_ms: f64,
    pub poll_mixed_update_apply_ms: f64,
    pub poll_mixed_update_dirty_mark_ms: f64,
    pub poll_mixed_update_client_apply_ms: f64,
    pub update_pump_stalled: bool,
    pub update_pump_stall_count: usize,
    pub server_update_applied_bytes: usize,
    pub server_update_oldest_applied_age_ms: f64,
    pub poll_diagnostics_ms: f64,
    pub poll_diagnostics_refreshed: bool,
    pub poll_diagnostics_cache_age_ms: f64,
    pub server_diagnostics_detail_refreshes: u64,
    pub server_diagnostics_detail_age_ms: f64,
    pub poll_server_tick_ms: f64,
    pub poll_server_reported_total_ms: f64,
    pub poll_scheduler_tick_ms: f64,
    pub poll_scheduler_completed_feature_jobs_drained: usize,
    pub poll_scheduler_feature_chunks_published: usize,
    pub poll_scheduler_feature_chunks_skipped: usize,
    pub poll_scheduler_feature_jobs_completed: usize,
    pub poll_scheduler_feature_snapshot_ready_events: usize,
    pub poll_scheduler_light_status_batches_enqueued: usize,
    pub poll_scheduler_completed_light_statuses_drained: usize,
    pub poll_scheduler_light_statuses_published: usize,
    pub poll_scheduler_light_statuses_skipped: usize,
    pub poll_scheduler_light_snapshot_ready_events: usize,
    pub poll_scheduler_pending_worldgen_publication_jobs: usize,
    pub poll_scheduler_pending_worldgen_publication_chunks: usize,
    pub poll_scheduler_pending_light_publications: usize,
    pub poll_scheduler_worldgen_mailbox_pending_jobs: usize,
    pub poll_scheduler_light_mailbox_pending_statuses: usize,
    pub poll_updates: usize,
    pub poll_snapshot_updates: usize,
    pub poll_section_block_updates: usize,
    pub poll_unload_updates: usize,
    pub poll_other_updates: usize,
    pub poll_mixed_updates: usize,
    pub server_command_queue_depth: usize,
    pub server_update_queue_depth: usize,
    pub server_update_queue_bytes: usize,
    pub server_pending_jobs: usize,
    pub server_pending_publications: usize,
    pub runner_frame_metrics: WorkerFrameMetrics,
    pub worldgen_job_frame_metrics: WorkerFrameMetrics,
    pub light_status_job_frame_metrics: WorkerFrameMetrics,
    pub scheduler_pending_jobs: usize,
    pub scheduler_completed_jobs: usize,
    pub scheduler_dirty_chunks: usize,
    pub scheduler_loaded_snapshot_chunks: usize,
    pub scheduler_client_visible_chunks: usize,
    pub scheduler_active_ticket_chunks: usize,
    pub player_visible_chunks: usize,
    pub player_outbound_queue_depth: usize,
    pub pending_render_chunks_before: usize,
    pub pending_render_chunks_after: usize,
    pub pending_compile_jobs_before: usize,
    pub pending_compile_jobs_after: usize,
    pub max_pending_compile_jobs: usize,
    pub available_compile_slots_before: usize,
    pub available_compile_slots_after: usize,
    pub rebuilt_section_count: usize,
    pub removed_section_count: usize,
    pub rebuilt_vertex_count: u32,
    pub rebuilt_index_count: u32,
    pub neighbor_ready_section_count: usize,
    pub near_exception_section_count: usize,
    pub deferred_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub deadline_skipped_compile_request_count: usize,
    pub accepted_compile_result_count: usize,
    pub queued_completed_compile_result_count: usize,
    pub completed_compile_section_count: usize,
    pub stale_compile_section_count: usize,
    pub uploaded_section_count: usize,
    pub upload_removed_section_count: usize,
    pub uploaded_vertex_count: u32,
    pub uploaded_index_count: u32,
    pub queued_upload_section_count: usize,
    pub queued_upload_removed_section_count: usize,
    pub queued_upload_lifecycle_item_count: usize,
    pub upload_phase_event_count: usize,
    pub upload_enqueued_lifecycle_item_count: usize,
    pub upload_superseded_lifecycle_item_count: usize,
    pub upload_drained_lifecycle_item_count: usize,
    pub upload_released_compile_job_count: usize,
    pub upload_released_compile_jobs_on_enqueue: usize,
    pub upload_released_compile_jobs_on_apply: usize,
    pub upload_held_lifecycle_item_count: usize,
    pub upload_held_compile_job_count: usize,
    pub upload_limited: bool,
    pub upload_accept_limited: bool,
    pub upload_backpressured: bool,
    pub traversal_ready_section_count: usize,
    pub record_cache: TexturedSectionRecordCacheStats,
    pub visibility_graph_build_count: usize,
    pub visibility_graph_total_ms: f64,
    pub visibility_graph_worst_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum XrTerrainHostMode {
    #[default]
    LocalIntegrated,
    RemoteDedicated,
}

impl XrTerrainHostMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::LocalIntegrated => "local-integrated",
            Self::RemoteDedicated => "remote-dedicated",
        }
    }

    pub const fn server_owned_lanes_are_remote(self) -> bool {
        matches!(self, Self::RemoteDedicated)
    }
}

impl From<SingleViewHostMode> for XrTerrainHostMode {
    fn from(value: SingleViewHostMode) -> Self {
        match value {
            SingleViewHostMode::LocalIntegrated => Self::LocalIntegrated,
            SingleViewHostMode::RemoteDedicated => Self::RemoteDedicated,
        }
    }
}

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

#[derive(Clone, Copy, Debug)]
pub struct XrTerrainMultiviewFrameSummary {
    pub rendered_frames: u32,
    pub section_count: usize,
    pub left: TexturedSectionRenderStats,
    pub right: TexturedSectionRenderStats,
    pub actor_count: usize,
    pub drawn_actor_count: usize,
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

#[derive(Clone, Debug, PartialEq)]
struct XrMenuPanelOverlayState {
    gui_scale: GuiScale,
    progress: Option<LoadingProgressOverlay>,
    status: Option<StatusOverlay>,
}

impl XrMenuPanelOverlayState {
    fn new(
        gui_scale: GuiScale,
        progress: Option<&LoadingProgressOverlay>,
        status: &StatusOverlay,
    ) -> Option<Self> {
        let progress = progress.cloned();
        let status = (status.visible && !status.message.is_empty()).then(|| status.clone());
        if progress.is_none() && status.is_none() {
            return None;
        }
        Some(Self {
            gui_scale,
            progress,
            status,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct XrMenuPanelOverlayDraw {
    draw: GuiDrawList,
    cache_revision: Option<UiPanelRevision>,
    draw_cache: UiDrawCacheStats,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct XrMenuPanelOverlayCache {
    state: Option<XrMenuPanelOverlayState>,
    draw: GuiDrawList,
    revision_content: u64,
}

impl XrMenuPanelOverlayCache {
    fn render(
        &mut self,
        gui_scale: GuiScale,
        progress: Option<&LoadingProgressOverlay>,
        status: &StatusOverlay,
    ) -> XrMenuPanelOverlayDraw {
        let Some(state) = XrMenuPanelOverlayState::new(gui_scale, progress, status) else {
            self.state = None;
            self.draw.clear();
            return XrMenuPanelOverlayDraw {
                draw: GuiDrawList::new(),
                cache_revision: None,
                draw_cache: UiDrawCacheStats::default(),
            };
        };

        if self.state.as_ref() == Some(&state) {
            return XrMenuPanelOverlayDraw {
                draw: self.draw.clone(),
                cache_revision: Some(UiPanelRevision::new(self.revision_content, 0)),
                draw_cache: UiDrawCacheStats::cache_hit(),
            };
        }

        let mut draw = GuiDrawList::new();
        if let Some(progress) = state.progress.as_ref() {
            render_loading_progress_overlay(gui_scale, &mut draw, progress);
        }
        if let Some(status) = state.status.as_ref() {
            render_status_overlay(gui_scale, &mut draw, status);
        }
        self.revision_content = self.revision_content.wrapping_add(1).max(1);
        self.state = Some(state);
        self.draw = draw.clone();
        XrMenuPanelOverlayDraw {
            draw,
            cache_revision: Some(UiPanelRevision::new(self.revision_content, 0)),
            draw_cache: UiDrawCacheStats::rebuild(),
        }
    }
}

struct XrMenuPanelDraw {
    panel_draw: GuiDrawList,
    cache_revision: Option<UiPanelRevision>,
    overlay_draw: GuiDrawList,
    overlay_cache_revision: Option<UiPanelRevision>,
    draw_cache: UiDrawCacheStats,
}

fn prepare_xr_menu_panel_draw(
    ui: &mut GameUiHost,
    overlay_cache: &mut XrMenuPanelOverlayCache,
    gui_scale: GuiScale,
    ui_state: GameUiRenderState,
    progress: Option<&LoadingProgressOverlay>,
    status: &StatusOverlay,
) -> XrMenuPanelDraw {
    ui.set_scale(gui_scale);
    let panel_draw = ui.render_v2_panel_draw_list(ui_state);
    let overlay_draw = overlay_cache.render(gui_scale, progress, status);
    let mut draw_cache = panel_draw
        .as_ref()
        .map_or_else(UiDrawCacheStats::default, |panel_draw| panel_draw.cache);
    draw_cache.add(overlay_draw.draw_cache);
    XrMenuPanelDraw {
        panel_draw: panel_draw
            .as_ref()
            .map_or_else(GuiDrawList::new, |panel_draw| panel_draw.draw.clone()),
        cache_revision: panel_draw.map(|panel_draw| panel_draw.revision),
        overlay_draw: overlay_draw.draw,
        overlay_cache_revision: overlay_draw.cache_revision,
        draw_cache,
    }
}

type XrSessionRuntimeFactory<S> = Box<
    dyn FnMut(
        SessionStartRequest,
        XrSceneOptions,
        TexturedMeshAssets,
    ) -> Result<NativeSingleViewSessionRuntime<S>>,
>;

struct StartedXrTerrainRuntime<S>
where
    S: RemoteDedicatedServerSession,
{
    runtime: NativeSingleViewSessionRuntime<S>,
    camera: EngineCameraController,
    draw: TexturedSectionDrawResources,
    render_stats: RenderStreamStats,
}

struct XrLocalStartup {
    request: SessionStartRequest,
    descriptor: Option<ActiveSessionDescriptor>,
    scene: XrSceneOptions,
    pump: LocalSingleViewStartupPump,
    camera: EngineCameraController,
    startup_view_pose: Option<XrStartupViewPose>,
}

#[derive(Clone, Debug, PartialEq)]
struct XrPendingSessionStart {
    scene: XrSceneOptions,
    descriptor: Option<ActiveSessionDescriptor>,
}

#[derive(Clone, Debug, PartialEq)]
enum XrSessionStartOutcome {
    LocalStartupQueued,
    Started(ActiveSessionDescriptor),
}

#[derive(Debug)]
pub enum XrLocalOnlyRemoteSession {}

impl RemoteDedicatedServerSession for XrLocalOnlyRemoteSession {
    fn send_command_only(&mut self, _command: mclone_protocol::ClientCommand) -> Result<()> {
        match *self {}
    }

    fn drain_command_updates(&mut self) -> Result<Vec<mclone_protocol::ServerUpdate>> {
        match *self {}
    }

    fn reconnect(&mut self) -> Result<()> {
        match *self {}
    }
}

pub struct XrMcloneTerrainState<S = XrLocalOnlyRemoteSession>
where
    S: RemoteDedicatedServerSession,
{
    scene: XrSceneOptions,
    color_format: wgpu::TextureFormat,
    mesh_assets: TexturedMeshAssets,
    runtime: Option<NativeSingleViewSessionRuntime<S>>,
    local_startup: Option<XrLocalStartup>,
    session: GameSessionCoordinator<XrPendingSessionStart>,
    session_runtime_factory: Option<XrSessionRuntimeFactory<S>>,
    client_experience: ClientExperienceController,
    world_catalog: Option<NativeWorldCatalog>,
    camera: EngineCameraController,
    interaction: ClientInteractionController,
    initial_alignment_mode: XrViewAlignmentMode,
    render_options: TexturedSectionRenderOptions,
    player_collision_box_visible: bool,
    travel_assist_mode: GameTravelAssistMode,
    player_model: GamePlayerModel,
    draw: TexturedSectionDrawResources,
    traversal_ready_sections: TraversalReadySectionCache,
    section_uploads: RenderSectionUploadCoordinator,
    actors: ActorDrawResources,
    far_lod: FarTerrainLodRenderer,
    selection_outline: SelectionOutlineRenderer,
    world_gui_renderer: WorldGuiRenderer,
    world_gui_overlay_renderer: WorldGuiRenderer,
    diagnostic_panel: XrDiagnosticPanel,
    ui: GameUiHost,
    menu_overlay_cache: XrMenuPanelOverlayCache,
    status_overlay: StatusOverlay,
    sky: SkyRenderer,
    screen_effects: ScreenEffectsRenderer,
    underwater_effects: XrUnderwaterEffectStates,
    last_underwater_update: Option<Instant>,
    head_comfort: XrHeadComfortState,
    render_stats: RenderStreamStats,
    tracking_origin: Option<XrTrackingOrigin>,
    locomotion_mode: XrLocomotionMode,
    turn_policy: XrTurnPolicy,
    snap_turn_state: XrSnapTurnState,
    blink_teleport: XrBlinkTeleportState,
    blink_teleport_worker: Option<NativeTeleportPreviewWorker>,
    display_refresh_hz: Option<f32>,
    render_split_timing_enabled: bool,
    defer_eye_waits_enabled: bool,
    overlap_runtime_prefetch_enabled: bool,
    prefetched_live_upload: Option<XrTerrainUploadSummary>,
    render_section_upload_budget: Option<usize>,
    render_section_accept_budget: Option<usize>,
    render_completed_result_accept_budget: Option<usize>,
    per_view_uniform_frame: u32,
    last_locomotion_update: Option<Instant>,
    menu_toggle_down: bool,
    game_ui_toggle_down: bool,
    menu_pointer_down: bool,
    gameplay_interaction_buttons: XrGameplayInteractionButtons,
    menu_panel_pose: Option<WorldGuiPanel>,
    menu_panel_anchor: XrUiPanelAnchor,
    menu_panel_recenter_pending: bool,
    latest_controllers: Vec<XrControllerSnapshot>,
    first_eye_summary: Option<FullFrameRenderSummary>,
    last_ui_panel_stats: WorldGuiPanelRenderStats,
    last_ui_draw_cache_stats: UiDrawCacheStats,
    rendered_frames: u32,
    audio: Option<AudioEngine>,
    seed_reroll_state: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct XrUnderwaterEffectStates {
    midpoint: UnderwaterEffectState,
    eyes: [UnderwaterEffectState; 2],
}

#[derive(Clone, Copy, Debug, Default)]
struct XrTerrainUploadApplyReport {
    upload: TexturedSectionUploadReport,
    phase: RenderSectionUploadPhaseReport,
    release_compile_jobs: usize,
}

impl XrTerrainFrameTiming {
    fn absorb_upload_apply_timing(&mut self, timing: TexturedSectionUploadTiming) {
        self.runtime_upload_apply_dirty_mark_ms += timing.dirty_mark_ms;
        self.runtime_upload_apply_remove_ms += timing.remove_ms;
        self.runtime_upload_apply_section_state_ms += timing.section_state_ms;
        self.runtime_upload_apply_vertex_bytes_ms += timing.vertex_bytes_ms;
        self.runtime_upload_apply_vertex_buffer_ms += timing.vertex_buffer_ms;
        self.runtime_upload_apply_index_bytes_ms += timing.index_bytes_ms;
        self.runtime_upload_apply_index_buffer_ms += timing.index_buffer_ms;
        self.runtime_upload_apply_mesh_insert_ms += timing.mesh_insert_ms;
        self.runtime_upload_apply_mesh_upload_worst_ms = self
            .runtime_upload_apply_mesh_upload_worst_ms
            .max(timing.mesh_upload_worst_ms);
    }
}

impl XrMcloneTerrainState<XrLocalOnlyRemoteSession> {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        scene: XrSceneOptions,
        render_options: TexturedSectionRenderOptions,
        mesh_assets: TexturedMeshAssets,
        actor_atlas: ActorTextureImage,
        actor_figures: ActorFigureSet,
        asset_source: &impl AssetSource,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        Self::start_local_async(
            device,
            queue,
            color_format,
            scene,
            render_options,
            mesh_assets,
            actor_atlas,
            actor_figures,
            asset_source,
            startup_view_pose,
        )
    }
}

impl<S> XrMcloneTerrainState<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn start_local_async(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        scene: XrSceneOptions,
        render_options: TexturedSectionRenderOptions,
        mesh_assets: TexturedMeshAssets,
        actor_atlas: ActorTextureImage,
        actor_figures: ActorFigureSet,
        asset_source: &impl AssetSource,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        let request = SessionStartRequest::new_seed_local_world(scene.seed);
        let descriptor = request.active_descriptor();
        let world_catalog = scene.world_root.clone().map(NativeWorldCatalog::new);
        let mut camera = EngineCameraController::spawn_for_chunk(scene.center());
        camera.set_movement_speed_multiplier(f64::from(scene.movement_speed_multiplier));
        if let Some(view_pose) = startup_view_pose {
            apply_xr_startup_view_pose(&mut camera, view_pose.position, view_pose.yaw_degrees)
                .context("apply initial XR local startup view pose")?;
        }
        let draw = TexturedSectionDrawResources::new(
            device,
            queue,
            color_format,
            &[],
            mesh_assets.atlas.as_upload(),
        )
        .context("initialize empty XR terrain draw resources")?;
        let mut world_gui_renderer = WorldGuiRenderer::new(device, color_format);
        world_gui_renderer
            .upload_texture_atlas(device, queue, mesh_assets.atlas.as_upload())
            .context("upload initial XR GUI atlas")?;
        let pump = LocalSingleViewStartupPump::with_mesh_assets(
            local_single_view_options(&scene),
            mesh_assets.clone(),
        )
        .context("create initial XR local world startup pump")?;
        let ui = xr_game_ui_for_session(None, scene.seed);
        let mut session = GameSessionCoordinator::new();
        session.begin_start(request.clone());
        let mut state = Self {
            scene: scene.clone(),
            color_format,
            mesh_assets,
            runtime: None,
            local_startup: Some(XrLocalStartup {
                request: request.clone(),
                descriptor,
                scene: scene.clone(),
                pump,
                camera: camera.clone(),
                startup_view_pose,
            }),
            session,
            session_runtime_factory: None,
            client_experience: ClientExperienceController::new(xr_client_experience_profile()),
            world_catalog,
            camera,
            interaction: ClientInteractionController::new(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            player_collision_box_visible: false,
            travel_assist_mode: GameTravelAssistMode::Off,
            player_model: GamePlayerModel::default(),
            draw,
            traversal_ready_sections: TraversalReadySectionCache::default(),
            section_uploads: RenderSectionUploadCoordinator::default(),
            actors: ActorDrawResources::new(
                device,
                queue,
                color_format,
                actor_atlas.as_upload(),
                Some(&actor_figures),
            )
            .context("initialize XR terrain actor draw resources")?,
            far_lod: FarTerrainLodRenderer::new(device, color_format),
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            world_gui_renderer,
            world_gui_overlay_renderer: WorldGuiRenderer::new(device, color_format),
            diagnostic_panel: XrDiagnosticPanel::new(device, color_format),
            ui,
            menu_overlay_cache: XrMenuPanelOverlayCache::default(),
            status_overlay: StatusOverlay::hidden(),
            sky: SkyRenderer::new_with_color_profile(
                device,
                color_format,
                render_options.color_profile,
            ),
            screen_effects: ScreenEffectsRenderer::new(device, queue, color_format, asset_source)
                .context("initialize XR screen effects renderer")?,
            underwater_effects: XrUnderwaterEffectStates::default(),
            last_underwater_update: None,
            head_comfort: XrHeadComfortState::default(),
            render_stats: RenderStreamStats::default(),
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            turn_policy: XrTurnPolicy::default(),
            snap_turn_state: XrSnapTurnState::default(),
            blink_teleport: XrBlinkTeleportState::default(),
            blink_teleport_worker: None,
            display_refresh_hz: None,
            render_split_timing_enabled: false,
            defer_eye_waits_enabled: false,
            overlap_runtime_prefetch_enabled: false,
            prefetched_live_upload: None,
            render_section_upload_budget: None,
            render_section_accept_budget: None,
            render_completed_result_accept_budget: None,
            per_view_uniform_frame: 0,
            last_locomotion_update: None,
            menu_toggle_down: false,
            game_ui_toggle_down: false,
            menu_pointer_down: false,
            gameplay_interaction_buttons: XrGameplayInteractionButtons::default(),
            menu_panel_pose: None,
            menu_panel_anchor: XrUiPanelAnchor::Head,
            menu_panel_recenter_pending: true,
            latest_controllers: Vec::new(),
            first_eye_summary: None,
            last_ui_panel_stats: WorldGuiPanelRenderStats::default(),
            last_ui_draw_cache_stats: UiDrawCacheStats::default(),
            rendered_frames: 0,
            audio: None,
            seed_reroll_state: initial_xr_seed_reroll_state(scene.seed),
        };
        state.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        Ok(state)
    }

    pub fn with_runtime(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        scene: XrSceneOptions,
        runtime: NativeSingleViewSessionRuntime<S>,
        render_options: TexturedSectionRenderOptions,
        actor_atlas: ActorTextureImage,
        actor_figures: ActorFigureSet,
        asset_source: &impl AssetSource,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        let world_catalog = scene.world_root.clone().map(NativeWorldCatalog::new);
        let started = start_xr_terrain_runtime(
            device,
            queue,
            color_format,
            runtime,
            scene.movement_speed_multiplier,
            startup_view_pose,
        )?;
        let active_session = started
            .runtime
            .active_session()
            .cloned()
            .context("XR runtime did not expose an active session")?;
        let ui = xr_game_ui_for_session(Some(&active_session), scene.seed);
        let mut session = GameSessionCoordinator::new();
        session.complete_start(active_session);
        let mesh_assets = started.runtime.mesh_assets().clone();
        let mut world_gui_renderer = WorldGuiRenderer::new(device, color_format);
        world_gui_renderer
            .upload_texture_atlas(device, queue, mesh_assets.atlas.as_upload())
            .context("upload XR GUI atlas")?;
        let mut state = Self {
            scene: scene.clone(),
            color_format,
            mesh_assets,
            runtime: Some(started.runtime),
            local_startup: None,
            session,
            session_runtime_factory: None,
            client_experience: ClientExperienceController::new(xr_client_experience_profile()),
            world_catalog,
            camera: started.camera,
            interaction: ClientInteractionController::new(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            player_collision_box_visible: false,
            travel_assist_mode: GameTravelAssistMode::Off,
            player_model: GamePlayerModel::default(),
            draw: started.draw,
            traversal_ready_sections: TraversalReadySectionCache::default(),
            section_uploads: RenderSectionUploadCoordinator::default(),
            actors: ActorDrawResources::new(
                device,
                queue,
                color_format,
                actor_atlas.as_upload(),
                Some(&actor_figures),
            )
            .context("initialize XR terrain actor draw resources")?,
            far_lod: FarTerrainLodRenderer::new(device, color_format),
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            world_gui_renderer,
            world_gui_overlay_renderer: WorldGuiRenderer::new(device, color_format),
            diagnostic_panel: XrDiagnosticPanel::new(device, color_format),
            ui,
            menu_overlay_cache: XrMenuPanelOverlayCache::default(),
            status_overlay: StatusOverlay::hidden(),
            sky: SkyRenderer::new_with_color_profile(
                device,
                color_format,
                render_options.color_profile,
            ),
            screen_effects: ScreenEffectsRenderer::new(device, queue, color_format, asset_source)
                .context("initialize XR screen effects renderer")?,
            underwater_effects: XrUnderwaterEffectStates::default(),
            last_underwater_update: None,
            head_comfort: XrHeadComfortState::default(),
            render_stats: started.render_stats,
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            turn_policy: XrTurnPolicy::default(),
            snap_turn_state: XrSnapTurnState::default(),
            blink_teleport: XrBlinkTeleportState::default(),
            blink_teleport_worker: None,
            display_refresh_hz: None,
            render_split_timing_enabled: false,
            defer_eye_waits_enabled: false,
            overlap_runtime_prefetch_enabled: false,
            prefetched_live_upload: None,
            render_section_upload_budget: None,
            render_section_accept_budget: None,
            render_completed_result_accept_budget: None,
            per_view_uniform_frame: 0,
            last_locomotion_update: None,
            menu_toggle_down: false,
            game_ui_toggle_down: false,
            menu_pointer_down: false,
            gameplay_interaction_buttons: XrGameplayInteractionButtons::default(),
            menu_panel_pose: None,
            menu_panel_anchor: XrUiPanelAnchor::Head,
            menu_panel_recenter_pending: false,
            latest_controllers: Vec::new(),
            first_eye_summary: None,
            last_ui_panel_stats: WorldGuiPanelRenderStats::default(),
            last_ui_draw_cache_stats: UiDrawCacheStats::default(),
            rendered_frames: 0,
            audio: None,
            seed_reroll_state: initial_xr_seed_reroll_state(scene.seed),
        };
        state.refresh_world_catalog_ui(WorldCatalogUiStatus::hidden());
        state.apply_debug_ui_screen();
        Ok(state)
    }

    pub fn set_audio_engine(&mut self, audio: Option<AudioEngine>) {
        self.audio = audio;
    }

    pub fn set_frame_pipeline_report(&mut self, report: Arc<FramePipelineReport>, revision: u64) {
        self.diagnostic_panel
            .set_frame_pipeline_report(report, revision);
    }

    pub fn clear_frame_pipeline_report(&mut self) {
        self.diagnostic_panel.clear_frame_pipeline_report();
    }

    pub fn set_session_runtime_factory<F>(&mut self, factory: F)
    where
        F: FnMut(
                SessionStartRequest,
                XrSceneOptions,
                TexturedMeshAssets,
            ) -> Result<NativeSingleViewSessionRuntime<S>>
            + 'static,
    {
        self.session_runtime_factory = Some(Box::new(factory));
    }

    pub fn render_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [xr::View; 2],
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

    pub fn render_frame_frozen_runtime(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [xr::View; 2],
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
        eye_fovs: [xr::Fovf; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        let mut timing = XrTerrainFrameTiming::default();
        let render_views_start = Instant::now();
        let render_views = fixed_startup_view_pose_render_views(view_pose, eye_fovs)?;
        timing.render_views_ms = elapsed_ms(render_views_start.elapsed());
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
        views: [xr::View; 2],
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
        eye_fovs: [xr::Fovf; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        let mut timing = XrTerrainFrameTiming::default();
        let render_views_start = Instant::now();
        let render_views = fixed_startup_view_pose_render_views(view_pose, eye_fovs)?;
        timing.render_views_ms = elapsed_ms(render_views_start.elapsed());
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
        views: [xr::View; 2],
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
        views: [xr::View; 2],
        target: XrTerrainMultiviewTarget<'_>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let render_views = self.render_views(&views)?;
        self.render_prepared_terrain_multiview_frame_frozen(device, queue, render_views, target)
    }

    pub fn render_sky_terrain_multiview_frame_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [xr::View; 2],
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
        views: [xr::View; 2],
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
        views: [xr::View; 2],
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
        views: [xr::View; 2],
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
        views: [xr::View; 2],
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
        views: [xr::View; 2],
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
        views: [xr::View; 2],
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
        views: [xr::View; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
        runtime_mode: XrTerrainRuntimeUpdateMode,
    ) -> Result<XrTerrainFrameSummary> {
        let mut timing = XrTerrainFrameTiming::default();
        let render_views_start = Instant::now();
        let mut render_views = self.render_views(&views)?;
        timing.render_views_ms += elapsed_ms(render_views_start.elapsed());
        self.update_menu_panel_pose(render_views);
        let menu_pointer_start = Instant::now();
        if self
            .apply_menu_pointer_input(device, queue)
            .context("apply XR menu pointer input")?
        {
            timing.menu_pointer_ms += elapsed_ms(menu_pointer_start.elapsed());
            let render_views_start = Instant::now();
            render_views = self.render_views(&views)?;
            timing.render_views_ms += elapsed_ms(render_views_start.elapsed());
            self.update_menu_panel_pose(render_views);
        } else {
            timing.menu_pointer_ms += elapsed_ms(menu_pointer_start.elapsed());
        }
        if self.advance_local_startup(device, queue)? {
            let render_views_start = Instant::now();
            render_views = self.render_views(&views)?;
            timing.render_views_ms += elapsed_ms(render_views_start.elapsed());
            self.update_menu_panel_pose(render_views);
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
        views: [xr::View; 2],
        target: XrTerrainMultiviewTarget<'_>,
        runtime_mode: XrTerrainRuntimeUpdateMode,
    ) -> Result<XrTerrainFrameSummary> {
        let mut timing = XrTerrainFrameTiming::default();
        let render_views_start = Instant::now();
        let mut render_views = self.render_views(&views)?;
        timing.render_views_ms += elapsed_ms(render_views_start.elapsed());
        self.update_menu_panel_pose(render_views);
        let menu_pointer_start = Instant::now();
        if self
            .apply_menu_pointer_input(device, queue)
            .context("apply XR menu pointer input for multiview frame")?
        {
            timing.menu_pointer_ms += elapsed_ms(menu_pointer_start.elapsed());
            let render_views_start = Instant::now();
            render_views = self.render_views(&views)?;
            timing.render_views_ms += elapsed_ms(render_views_start.elapsed());
            self.update_menu_panel_pose(render_views);
        } else {
            timing.menu_pointer_ms += elapsed_ms(menu_pointer_start.elapsed());
        }
        if self.advance_local_startup(device, queue)? {
            let render_views_start = Instant::now();
            render_views = self.render_views(&views)?;
            timing.render_views_ms += elapsed_ms(render_views_start.elapsed());
            self.update_menu_panel_pose(render_views);
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
        let render_options = self.effective_render_options(center_position);
        let sky_clear_color = self.sky_clear_color();
        let time_of_day = self.time_of_day();
        let sun_angle = self.sun_angle();
        let underwater_overlays = self.underwater_overlays(render_views);
        let actor_instances = self.current_actor_instances();
        let collect_split_timing = self.render_split_timing_enabled;
        let records_start = collect_split_timing.then(Instant::now);
        let (prepared_records, record_cache_prepare) =
            self.draw.prepare_render_records_with_stats();
        timing.record_cache_prepare = record_cache_prepare;
        timing.shared_records_ms = records_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let (terrain_views, terrain_options, _) =
            self.terrain_render_views_and_options(render_views);
        let (prepared_stereo_draw, stereo_draw_timing) = if collect_split_timing {
            self.draw
                .prepare_stereo_draw_timed(&prepared_records, terrain_views, terrain_options)
        } else {
            (
                self.draw
                    .prepare_stereo_draw(&prepared_records, terrain_views, terrain_options),
                Default::default(),
            )
        };
        let defer_eye_waits = self.defer_eye_waits_enabled;
        let uniform_frame = self.next_per_view_uniform_frame();
        let left_view_slot = LEFT_EYE_VIEW_SLOT.in_uniform_frame(uniform_frame);
        let right_view_slot = RIGHT_EYE_VIEW_SLOT.in_uniform_frame(uniform_frame);
        let diagnostic_panel = xr_diagnostic_panel_from_render_views(render_views);
        let left_eye_start = Instant::now();
        let left_eye = self.render_eye_target(
            device,
            queue,
            &prepared_stereo_draw,
            left_target,
            render_views[0],
            diagnostic_panel,
            &actor_instances,
            render_options,
            sky_clear_color,
            time_of_day,
            sun_angle,
            underwater_overlays[0],
            "left",
            left_view_slot,
            !defer_eye_waits,
        )?;
        timing.left_eye_ms = elapsed_ms(left_eye_start.elapsed());
        timing.left_eye_render = left_eye.timing;
        timing.left_eye_render.cull_ms += stereo_draw_timing.cull_ms;
        timing.left_eye_render.translucent_collect_ms += stereo_draw_timing.translucent_collect_ms;
        timing.left_eye_render.translucent_sort_ms += stereo_draw_timing.translucent_sort_ms;
        timing.left_eye_render.prepare_ms += stereo_draw_timing.prepare_ms;
        let right_eye_start = Instant::now();
        let mut right_eye = self.render_eye_target(
            device,
            queue,
            &prepared_stereo_draw,
            right_target,
            render_views[1],
            diagnostic_panel,
            &actor_instances,
            render_options,
            sky_clear_color,
            time_of_day,
            sun_angle,
            underwater_overlays[1],
            "right",
            right_view_slot,
            !defer_eye_waits,
        )?;
        timing.right_eye_ms = elapsed_ms(right_eye_start.elapsed());
        timing.right_eye_render = right_eye.timing;
        timing.stereo_submit_ms =
            timing.left_eye_render.submit_ms + timing.right_eye_render.submit_ms;
        if defer_eye_waits {
            if self.overlap_runtime_prefetch_enabled
                && matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live)
                && self.local_startup.is_none()
                && self.runtime.is_some()
            {
                let prefetch_start = Instant::now();
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
                timing.overlap_runtime_prefetch_ms = elapsed_ms(prefetch_start.elapsed());
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
            let poll_wait_start = Instant::now();
            Self::wait_for_xr_submission(device, submission, "deferred XR terrain stereo render")
                .context("wait for deferred XR terrain stereo render")?;
            timing.stereo_poll_wait_ms = elapsed_ms(poll_wait_start.elapsed());
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
        self.record_eye0_summary(left_eye.summary);
        Ok(self.frame_summary_with_timing(timing, upload))
    }

    fn live_upload_for_frame(
        &mut self,
        device: &wgpu::Device,
        center_position: Vec3,
        runtime_mode: XrTerrainRuntimeUpdateMode,
        frame_deadline: Option<Instant>,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadSummary> {
        if !matches!(runtime_mode, XrTerrainRuntimeUpdateMode::Live) {
            self.prefetched_live_upload = None;
            return Ok(self.frozen_runtime_upload_summary());
        }
        if self.local_startup.is_some() || self.runtime.is_none() {
            self.prefetched_live_upload = None;
            return Ok(self.frozen_runtime_upload_summary());
        }
        if let Some(upload) = self.prefetched_live_upload.take() {
            return Ok(upload);
        }
        let runtime_upload_start = Instant::now();
        let upload =
            self.poll_runtime_and_upload(device, center_position, frame_deadline, timing)?;
        timing.runtime_upload_ms = elapsed_ms(runtime_upload_start.elapsed());
        Ok(upload)
    }

    fn render_compile_frame_deadline(&self) -> Option<Instant> {
        self.display_refresh_hz
            .filter(|hz| hz.is_finite() && *hz > 0.0)
            .map(|hz| Instant::now() + Duration::from_secs_f64(1.0 / f64::from(hz)))
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
    ) -> (
        [ChunkRenderView; 2],
        [TexturedSectionRenderOptions; 2],
        [Option<UnderwaterOverlay>; 2],
    ) {
        let center_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let base_options = self
            .effective_render_options(center_position)
            .with_sky_darken(mclone_render::light_texture::sky_darken(self.time_of_day()));
        let underwater_overlays = self.underwater_overlays(render_views);
        let terrain_views = [
            render_view_with_underwater_effect(render_views[0], underwater_overlays[0]),
            render_view_with_underwater_effect(render_views[1], underwater_overlays[1]),
        ];
        let terrain_options = underwater_overlays.map(|overlay| {
            let fog = overlay
                .map(|overlay| RenderFog::underwater_with_water_vision(overlay.water_vision))
                .unwrap_or_default();
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
        let (terrain_views, terrain_options, _) =
            self.terrain_render_views_and_options(render_views);
        let actor_instances = if include_actors {
            self.current_actor_instances()
        } else {
            Vec::new()
        };
        let prepared_records = self.draw.prepare_render_records();
        let prepared_stereo_draw =
            self.draw
                .prepare_stereo_draw(&prepared_records, terrain_views, terrain_options);
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
        )?;

        self.render_stats.drawn_section_count = left.drawn_section_count;
        self.render_stats.drawn_face_count = left.drawn_face_count();
        self.render_stats.drawn_index_count = left.drawn_index_count;
        self.rendered_frames += 1;
        Ok(XrTerrainStereoFrameSummary {
            rendered_frames: self.rendered_frames,
            section_count: self.draw.section_count(),
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
            self.sky_clear_color(),
        );
        if include_sky {
            self.sky.render_in_slot(
                queue,
                &mut encoder,
                target.color_view,
                self.sky_clear_color(),
                render_view.sky_view_projection(),
                self.time_of_day(),
                self.sun_angle(),
                view_slot,
            );
            render_target = render_target.with_loaded_color();
        }
        let split_translucent_terrain = include_actors && !actor_instances.is_empty();
        let terrain_phase = if split_translucent_terrain {
            TexturedSectionRenderPhase::Opaque
        } else {
            TexturedSectionRenderPhase::All
        };
        let stats = self
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
        if include_actors {
            self.actors
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
            self.draw
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
        let upload = if self.local_startup.is_none() && self.runtime.is_some() {
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
        self.render_prepared_terrain_multiview_frame_with_upload_inner(
            device,
            queue,
            render_views,
            target,
            XrTerrainUploadSummary::default(),
            include_sky,
            include_actors,
            include_overlays,
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
        mut timing: Option<&mut XrTerrainFrameTiming>,
    ) -> Result<XrTerrainMultiviewFrameSummary> {
        let (terrain_views, terrain_options, underwater_overlays) =
            self.terrain_render_views_and_options(render_views);
        let actor_instances = if include_actors {
            self.current_actor_instances()
        } else {
            Vec::new()
        };
        let records_start = Instant::now();
        let (prepared_records, record_cache_prepare) =
            self.draw.prepare_render_records_with_stats();
        if let Some(timing) = timing.as_deref_mut() {
            timing.shared_records_ms = elapsed_ms(records_start.elapsed());
            timing.record_cache_prepare = record_cache_prepare;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_xr_terrain_multiview_encoder"),
        });
        let mut render_target = ChunkMultiviewRenderTarget::new(
            target.color_view,
            &target.depth.view,
            target.size,
            self.sky_clear_color(),
        );
        if include_sky {
            let sky_start = Instant::now();
            self.sky.render_multiview(
                device,
                queue,
                &mut encoder,
                target.color_view,
                self.sky_clear_color(),
                [
                    terrain_views[0].sky_view_projection(),
                    terrain_views[1].sky_view_projection(),
                ],
                self.time_of_day(),
                self.sun_angle(),
            )?;
            if let Some(timing) = timing.as_deref_mut() {
                timing.multiview_sky_ms = elapsed_ms(sky_start.elapsed());
            }
            render_target = render_target.with_loaded_color();
        }
        let terrain_start = Instant::now();
        let prepared_stereo_draw =
            self.draw
                .prepare_stereo_draw(&prepared_records, terrain_views, terrain_options);
        let split_translucent_terrain = include_actors && !actor_instances.is_empty();
        let terrain_phase = if split_translucent_terrain {
            TexturedSectionRenderPhase::Opaque
        } else {
            TexturedSectionRenderPhase::All
        };
        let stats = self
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
        if let Some(timing) = timing.as_deref_mut() {
            timing.multiview_terrain_ms = elapsed_ms(terrain_start.elapsed());
        }
        let actor_stats = if include_actors {
            let actor_start = Instant::now();
            let actor_stats = self
                .actors
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
                timing.multiview_actor_ms = elapsed_ms(actor_start.elapsed());
            }
            if split_translucent_terrain {
                let translucent_start = Instant::now();
                self.draw
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
                if let Some(timing) = timing.as_deref_mut() {
                    timing.multiview_terrain_ms += elapsed_ms(translucent_start.elapsed());
                }
            }
            actor_stats
        } else {
            ActorRenderStats::default()
        };
        let mut ui_panel_stats = WorldGuiPanelRenderStats::default();
        let mut ui_draw_cache_stats = UiDrawCacheStats::default();
        if include_overlays {
            let screen_effect_start = Instant::now();
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
                timing.multiview_screen_effect_ms = elapsed_ms(screen_effect_start.elapsed());
            }
            let overlays_start = Instant::now();
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
                timing.multiview_world_overlays_ms = elapsed_ms(overlays_start.elapsed());
            }
        }
        let submit_start = Instant::now();
        let submission = queue.submit(Some(encoder.finish()));
        if let Some(timing) = timing.as_deref_mut() {
            timing.multiview_submit_ms = elapsed_ms(submit_start.elapsed());
        }
        let poll_wait_start = Instant::now();
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .map(|_| ())
            .context("wait for XR terrain multiview submission")?;
        device
            .poll(wgpu::PollType::Wait)
            .map(|_| ())
            .context("wait for XR terrain multiview device idle")?;
        if let Some(timing) = timing.as_deref_mut() {
            timing.multiview_poll_wait_ms = elapsed_ms(poll_wait_start.elapsed());
        }

        self.render_stats.drawn_section_count = stats[0].drawn_section_count;
        self.render_stats.drawn_face_count = stats[0].drawn_face_count();
        self.render_stats.drawn_index_count = stats[0].drawn_index_count;
        self.last_ui_panel_stats = ui_panel_stats;
        self.last_ui_draw_cache_stats = ui_draw_cache_stats;
        self.rendered_frames += 1;
        Ok(XrTerrainMultiviewFrameSummary {
            rendered_frames: self.rendered_frames,
            section_count: self.draw.section_count(),
            left: stats[0],
            right: stats[1],
            actor_count: actor_instances.len(),
            drawn_actor_count: actor_stats.drawn_actor_count,
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
            &self.camera,
            EngineDebugVisualOptions::new(self.player_collision_box_visible),
        );
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
        if !self.ui.is_active() {
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

    pub fn set_locomotion_mode(&mut self, locomotion_mode: XrLocomotionMode) {
        self.locomotion_mode = locomotion_mode;
    }

    pub fn locomotion_mode(&self) -> XrLocomotionMode {
        self.locomotion_mode
    }

    pub fn set_turn_policy(&mut self, turn_policy: XrTurnPolicy) {
        self.turn_policy = turn_policy;
        self.snap_turn_state.reset();
    }

    pub fn turn_policy(&self) -> XrTurnPolicy {
        self.turn_policy
    }

    pub fn set_display_refresh_hz(&mut self, display_refresh_hz: Option<f32>) {
        self.display_refresh_hz = display_refresh_hz.filter(|hz| hz.is_finite() && *hz > 0.0);
    }

    pub fn set_render_split_timing_enabled(&mut self, enabled: bool) {
        self.render_split_timing_enabled = enabled;
    }

    pub fn set_defer_eye_waits_enabled(&mut self, enabled: bool) {
        self.defer_eye_waits_enabled = enabled;
    }

    pub fn set_overlap_runtime_prefetch_enabled(&mut self, enabled: bool) {
        self.overlap_runtime_prefetch_enabled = enabled;
        if !enabled {
            self.prefetched_live_upload = None;
        }
    }

    pub fn set_render_section_upload_budget(&mut self, budget: Option<usize>) {
        self.render_section_upload_budget = budget.filter(|budget| *budget > 0);
    }

    pub fn render_section_upload_budget(&self) -> Option<usize> {
        self.render_section_upload_budget
    }

    pub fn set_render_section_accept_budget(&mut self, budget: Option<usize>) {
        self.render_section_accept_budget = budget.filter(|budget| *budget > 0);
    }

    pub fn render_section_accept_budget(&self) -> Option<usize> {
        self.render_section_accept_budget
    }

    pub fn set_render_completed_result_accept_budget(&mut self, budget: Option<usize>) {
        self.render_completed_result_accept_budget = budget.filter(|budget| *budget > 0);
    }

    pub fn render_completed_result_accept_budget(&self) -> Option<usize> {
        self.render_completed_result_accept_budget
    }

    pub fn camera_snapshot(&self) -> EngineCameraSnapshot {
        self.camera.snapshot()
    }

    pub fn apply_locomotion_input(
        &mut self,
        controllers: &[XrControllerSnapshot],
        views: [xr::View; 2],
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = Instant::now();
        self.latest_controllers.clear();
        self.latest_controllers.extend_from_slice(controllers);
        let now = Instant::now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        let ui_was_active = self.ui.is_active();
        self.apply_menu_toggle_input(controllers);
        self.apply_game_ui_toggle_input(controllers);
        let ui_active = self.ui.is_active();
        let gameplay_interaction_edges = self.update_gameplay_interaction_buttons(controllers);
        let suppress_gameplay_interaction = ui_was_active != ui_active;
        if self.runtime.is_none() {
            self.head_comfort.reset();
            self.snap_turn_state.reset();
            self.clear_xr_blink_teleport();
            timing.input_ms = elapsed_ms(input_start.elapsed());
            return Ok(timing);
        }
        let mut transform = self
            .reconcile_room_scale_body_to_headset(&views)
            .context("reconcile XR room-scale body pose")?;
        self.update_head_comfort_state(transform, &views, dt_seconds)
            .context("update XR head comfort fade state")?;
        if self.ui.is_active() {
            self.snap_turn_state.reset();
            self.clear_xr_blink_teleport();
            timing.input_ms = elapsed_ms(input_start.elapsed());
            let commit_start = Instant::now();
            let (_, commit_timing) = self
                .commit_engine_camera_player_pose_timed()
                .context("sync XR room-scale player pose")?;
            timing.commit_ms = elapsed_ms(commit_start.elapsed());
            timing.record_commit_timing(commit_timing);
            return Ok(timing);
        }
        if let Some(yaw_delta_radians) = self.snap_turn_delta_from_controllers(controllers) {
            transform = self
                .apply_snap_turn_preserving_headset(&views, transform, yaw_delta_radians)
                .context("apply XR snap turn")?;
        }
        let blink_frame = self
            .update_xr_blink_teleport(controllers, &views, transform)
            .context("update XR Blink teleport")?;
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians_with_transform(&views, transform)
            .context("resolve XR locomotion frame")?;
        let mut input = xr_locomotion_input_from_controllers_with_turn_policy(
            controllers,
            dt_seconds,
            movement_yaw_radians,
            self.turn_policy,
        );
        if blink_frame.suppress_left_stick_movement {
            input.movement_impulse = None;
        }
        input.hand_push = xr_hand_push_input_from_controllers(controllers, &views, transform)
            .context("resolve XR hand-push input")?;
        timing.input_ms = elapsed_ms(input_start.elapsed());
        let camera_apply_start = Instant::now();
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        timing.camera_apply_ms = elapsed_ms(camera_apply_start.elapsed());
        self.play_landing_events();
        let commit_start = Instant::now();
        let (_, commit_timing) = self
            .commit_engine_camera_player_pose_timed()
            .context("sync XR locomotion player pose")?;
        timing.commit_ms = elapsed_ms(commit_start.elapsed());
        timing.record_commit_timing(commit_timing);
        if !suppress_gameplay_interaction {
            let interaction_start = Instant::now();
            self.apply_xr_gameplay_interaction_edges(gameplay_interaction_edges)?;
            timing.gameplay_interaction_ms = elapsed_ms(interaction_start.elapsed());
        }
        Ok(timing)
    }

    fn update_xr_blink_teleport(
        &mut self,
        controllers: &[XrControllerSnapshot],
        views: &[xr::View],
        transform: XrStageToWorld,
    ) -> Result<XrBlinkTeleportFrame> {
        if let Some(frame) = xr_blink_teleport_disabled_frame(self.travel_assist_mode, controllers)
        {
            self.clear_xr_blink_teleport();
            return Ok(frame);
        }

        let left_axis = xr_left_stick_raw_axis(controllers);
        let left_axis_active = left_axis.length() > XR_JOYPAD_DEAD_ZONE;
        let blink_engaged = xr_left_stick_blink_engaged(controllers);
        if blink_engaged {
            if !self.blink_teleport.active {
                self.begin_xr_blink_teleport(views, transform);
            }
            self.submit_xr_blink_teleport_request(controllers, transform)?;
            self.poll_xr_blink_teleport_worker();
            return Ok(XrBlinkTeleportFrame {
                suppress_left_stick_movement: true,
            });
        }

        if self.blink_teleport.active {
            self.commit_xr_blink_teleport(views, transform)?;
            return Ok(XrBlinkTeleportFrame {
                suppress_left_stick_movement: true,
            });
        }

        Ok(XrBlinkTeleportFrame {
            suppress_left_stick_movement: left_axis_active,
        })
    }

    fn begin_xr_blink_teleport(&mut self, views: &[xr::View], transform: XrStageToWorld) {
        if self.runtime.is_none() {
            self.clear_xr_blink_teleport();
            return;
        }
        let base_yaw_degrees = match xr_headset_player_yaw_degrees_from_views(views, transform) {
            Ok(yaw_degrees) => yaw_degrees,
            Err(error) => {
                log::warn!(
                    "failed to capture XR Blink headset heading, falling back to body yaw: {error:#}"
                );
                self.camera.player().pose().y_rot_degrees
            }
        };
        self.ensure_xr_blink_teleport_worker();
        self.blink_teleport.active = true;
        self.blink_teleport.preview = None;
        self.blink_teleport.first_request_id = None;
        self.blink_teleport.preview_request_id = None;
        self.blink_teleport.base_yaw_degrees = base_yaw_degrees;
        self.blink_teleport.target_yaw_degrees = base_yaw_degrees;
        self.blink_teleport.activation_left_stick_angle_radians = None;
        self.blink_teleport.last_submitted_intent = None;
        log::info!("XR Blink teleport preview armed");
    }

    fn clear_xr_blink_teleport(&mut self) {
        self.blink_teleport = XrBlinkTeleportState::default();
    }

    fn ensure_xr_blink_teleport_worker(&mut self) {
        if self.blink_teleport_worker.is_some() {
            return;
        }
        match NativeTeleportPreviewWorker::new() {
            Ok(worker) => {
                self.blink_teleport_worker = Some(worker);
            }
            Err(error) => {
                log::warn!("failed to start XR Blink teleport worker: {error}");
            }
        }
    }

    fn submit_xr_blink_teleport_request(
        &mut self,
        controllers: &[XrControllerSnapshot],
        transform: XrStageToWorld,
    ) -> Result<bool> {
        let target_yaw_degrees = self.xr_blink_teleport_target_yaw_degrees(controllers);
        self.blink_teleport.target_yaw_degrees = target_yaw_degrees;
        if let Some(preview) = self.blink_teleport.preview.as_mut() {
            preview.target_yaw_degrees = target_yaw_degrees;
        }
        let Some(intent) =
            xr_blink_teleport_intent(&self.camera, controllers, transform, target_yaw_degrees)
        else {
            return Ok(false);
        };
        if self
            .blink_teleport
            .last_submitted_intent
            .is_some_and(|last| teleport_intent_query_matches(last, intent))
        {
            return Ok(false);
        }
        self.ensure_xr_blink_teleport_worker();
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(false);
        };
        let Some(worker) = self.blink_teleport_worker.as_mut() else {
            return Ok(false);
        };
        match worker.submit_from_world(runtime.client(), intent, xr_blink_teleport_config()) {
            Ok(request_id) => {
                self.blink_teleport
                    .first_request_id
                    .get_or_insert(request_id);
                self.blink_teleport.last_submitted_intent = Some(intent);
                Ok(true)
            }
            Err(error) => {
                log::warn!("XR Blink teleport preview submit failed: {error}");
                self.blink_teleport_worker = None;
                Ok(false)
            }
        }
    }

    fn poll_xr_blink_teleport_worker(&mut self) -> bool {
        let Some(worker) = self.blink_teleport_worker.as_mut() else {
            return false;
        };
        match worker.try_recv_latest() {
            Ok(Some(result)) => self.accept_xr_blink_teleport_result(result),
            Ok(None) => false,
            Err(error) => {
                log::warn!("XR Blink teleport worker failed: {error}");
                self.blink_teleport_worker = None;
                false
            }
        }
    }

    fn accept_xr_blink_teleport_result(&mut self, result: TeleportPreviewResult) -> bool {
        if !self.blink_teleport.active {
            return false;
        }
        if self
            .blink_teleport
            .first_request_id
            .is_some_and(|first_request_id| result.id < first_request_id)
        {
            return false;
        }
        if self
            .blink_teleport
            .preview_request_id
            .is_some_and(|preview_request_id| result.id <= preview_request_id)
        {
            return false;
        }
        let mut preview = result.preview;
        preview.target_yaw_degrees = self.blink_teleport.target_yaw_degrees;
        self.blink_teleport.preview = Some(preview);
        self.blink_teleport.preview_request_id = Some(result.id);
        true
    }

    fn commit_xr_blink_teleport(
        &mut self,
        views: &[xr::View],
        transform: XrStageToWorld,
    ) -> Result<()> {
        self.poll_xr_blink_teleport_worker();
        let preview = self.blink_teleport.preview.take();
        self.clear_xr_blink_teleport();
        let Some(preview) = preview else {
            log::info!("XR Blink teleport release had no completed preview");
            return Ok(());
        };
        let Some(target_feet) = preview.target_feet.filter(|_| preview.is_valid()) else {
            log::info!(
                "XR Blink teleport release had no valid preview: {:?}",
                preview.validity
            );
            return Ok(());
        };

        let snapshot = self.camera.snapshot();
        let landing_yaw_radians = match xr_blink_teleport_landing_yaw_radians(
            snapshot.yaw_radians,
            views,
            transform,
            preview.target_yaw_degrees,
        ) {
            Ok(yaw_radians) => yaw_radians,
            Err(error) => {
                log::warn!(
                    "failed to solve XR Blink landing yaw from headset pose, falling back to body yaw: {error:#}"
                );
                -preview.target_yaw_degrees.to_radians()
            }
        };
        self.camera
            .set_player_feet_pose(target_feet, landing_yaw_radians, snapshot.pitch_radians);
        if let Some(runtime) = self.runtime.as_ref() {
            self.camera
                .probe_ground(runtime.client(), XR_BLINK_TELEPORT_GROUND_PROBE_DISTANCE);
        }
        log::info!(
            "XR Blink teleport committed feet=({:.2}, {:.2}, {:.2})",
            target_feet.x,
            target_feet.y,
            target_feet.z
        );
        Ok(())
    }

    fn xr_blink_teleport_target_yaw_degrees(
        &mut self,
        controllers: &[XrControllerSnapshot],
    ) -> f64 {
        let base_yaw_degrees = self.blink_teleport.base_yaw_degrees;
        let previous_target_yaw_degrees = self.blink_teleport.target_yaw_degrees;
        xr_blink_teleport_target_yaw_degrees_from_stick(
            base_yaw_degrees,
            previous_target_yaw_degrees,
            &mut self.blink_teleport.activation_left_stick_angle_radians,
            xr_left_stick_raw_axis(controllers),
        )
    }

    pub fn apply_automated_flight_input(
        &mut self,
        views: [xr::View; 2],
        speed_blocks_per_second: f64,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = Instant::now();
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            timing.input_ms = elapsed_ms(input_start.elapsed());
            return Ok(timing);
        }
        self.ui.close();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.gameplay_interaction_buttons = XrGameplayInteractionButtons::default();
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        let now = Instant::now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        self.camera.set_movement_mode(EngineCameraMovementMode::Fly);
        self.camera
            .set_collision_mode(EngineCameraCollisionMode::NoClip);
        self.camera
            .set_speed_blocks_per_second(speed_blocks_per_second);
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians(&views)
            .context("resolve XR automated flight locomotion frame")?;
        let input = xr_automated_flight_input(dt_seconds, movement_yaw_radians);
        timing.input_ms = elapsed_ms(input_start.elapsed());
        let camera_apply_start = Instant::now();
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        timing.camera_apply_ms = elapsed_ms(camera_apply_start.elapsed());
        let commit_start = Instant::now();
        let (_, commit_timing) = self
            .commit_engine_camera_player_pose_timed()
            .context("sync XR automated flight player pose")?;
        timing.commit_ms = elapsed_ms(commit_start.elapsed());
        timing.record_commit_timing(commit_timing);
        Ok(timing)
    }

    pub fn apply_automated_orbit_input(
        &mut self,
        speed_blocks_per_second: f64,
        elapsed_seconds: f64,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = XrLocomotionTiming::default();
        let input_start = Instant::now();
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            timing.input_ms = elapsed_ms(input_start.elapsed());
            return Ok(timing);
        }
        self.ui.close();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.gameplay_interaction_buttons = XrGameplayInteractionButtons::default();
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        let now = Instant::now();
        let dt_seconds = self
            .last_locomotion_update
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        self.camera.set_movement_mode(EngineCameraMovementMode::Fly);
        self.camera
            .set_collision_mode(EngineCameraCollisionMode::NoClip);
        self.camera
            .set_speed_blocks_per_second(speed_blocks_per_second);
        let input = xr_automated_orbit_input(dt_seconds, speed_blocks_per_second, elapsed_seconds);
        timing.input_ms = elapsed_ms(input_start.elapsed());
        let camera_apply_start = Instant::now();
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        timing.camera_apply_ms = elapsed_ms(camera_apply_start.elapsed());
        let commit_start = Instant::now();
        let (_, commit_timing) = self
            .commit_engine_camera_player_pose_timed()
            .context("sync XR automated orbit player pose")?;
        timing.commit_ms = elapsed_ms(commit_start.elapsed());
        timing.record_commit_timing(commit_timing);
        Ok(timing)
    }

    pub fn apply_automated_stationary_input(&mut self) -> XrLocomotionTiming {
        let mut timing = XrLocomotionTiming::default();
        let input_start = Instant::now();
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            timing.input_ms = elapsed_ms(input_start.elapsed());
            return timing;
        }
        if self.scene.debug_ui_screen.is_none() {
            self.ui.close();
            self.ui.clear_input();
            self.menu_pointer_down = false;
            self.menu_panel_pose = None;
            self.menu_panel_anchor = XrUiPanelAnchor::Head;
            self.menu_panel_recenter_pending = false;
        } else {
            self.apply_debug_ui_screen();
        }
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        self.last_locomotion_update = Some(Instant::now());
        timing.input_ms = elapsed_ms(input_start.elapsed());
        timing
    }

    pub fn apply_automated_chunk_view_churn(
        &mut self,
        center_x: i32,
        center_z: i32,
    ) -> Result<XrLocomotionTiming> {
        let mut timing = self.apply_automated_stationary_input();
        if self.local_startup.is_some() {
            return Ok(timing);
        }
        let Some(runtime) = self.runtime.as_mut() else {
            return Ok(timing);
        };
        let center = ChunkPos::new(center_x, center_z);
        let interest_start = Instant::now();
        let (changed, command_timing) = runtime
            .set_interest_center_with_update_policy_timed(
                center,
                GameplayCommandUpdatePolicy::SendOnly,
            )
            .context("set XR automated chunk-view churn interest center")?;
        timing.commit_interest_ms = elapsed_ms(interest_start.elapsed());
        timing.record_interest_command_timing(command_timing);
        if changed {
            log::info!(
                "MCLONE_ANDROID_XR_CHUNK_VIEW_CHURN center_x={} center_z={} changed=true",
                center.x,
                center.z
            );
        }
        Ok(timing)
    }

    pub fn frame_summary(&self) -> XrTerrainFrameSummary {
        self.frame_summary_with_timing(
            XrTerrainFrameTiming::default(),
            XrTerrainUploadSummary::default(),
        )
    }

    fn frame_summary_with_timing(
        &self,
        timing: XrTerrainFrameTiming,
        upload: XrTerrainUploadSummary,
    ) -> XrTerrainFrameSummary {
        if let Some(summary) = self.first_eye_summary {
            return XrTerrainFrameSummary {
                rendered_frames: self.rendered_frames,
                section_count: summary.section_count,
                drawn_section_count: summary.drawn_section_count,
                index_count: summary.index_count,
                drawn_index_count: summary.drawn_index_count,
                gui_command_count: summary.gui_command_count,
                ui_panel: self.last_ui_panel_stats,
                ui_draw_cache: self.last_ui_draw_cache_stats,
                ui_active: self.ui.is_active(),
                local_startup_active: self.local_startup.is_some(),
                actor_count: summary.actor_count,
                drawn_actor_count: summary.drawn_actor_count,
                head_comfort: self.head_comfort,
                timing,
                upload,
            };
        }
        XrTerrainFrameSummary {
            rendered_frames: self.rendered_frames,
            section_count: self.render_stats.section_count,
            drawn_section_count: self.render_stats.drawn_section_count,
            index_count: self.render_stats.index_count,
            drawn_index_count: self.render_stats.drawn_index_count,
            gui_command_count: 0,
            ui_panel: self.last_ui_panel_stats,
            ui_draw_cache: self.last_ui_draw_cache_stats,
            ui_active: self.ui.is_active(),
            local_startup_active: self.local_startup.is_some(),
            actor_count: self.render_stats.actor_count,
            drawn_actor_count: self.render_stats.drawn_actor_count,
            head_comfort: self.head_comfort,
            timing,
            upload,
        }
    }

    fn frame_summary_from_multiview(
        &self,
        summary: XrTerrainMultiviewFrameSummary,
        timing: XrTerrainFrameTiming,
    ) -> XrTerrainFrameSummary {
        XrTerrainFrameSummary {
            rendered_frames: summary.rendered_frames,
            section_count: summary.section_count,
            drawn_section_count: summary.left.drawn_section_count,
            index_count: self.render_stats.index_count,
            drawn_index_count: summary.left.drawn_index_count,
            gui_command_count: 0,
            ui_panel: summary.ui_panel,
            ui_draw_cache: summary.ui_draw_cache,
            ui_active: self.ui.is_active(),
            local_startup_active: self.local_startup.is_some(),
            actor_count: summary.actor_count,
            drawn_actor_count: summary.drawn_actor_count,
            head_comfort: self.head_comfort,
            timing,
            upload: summary.upload,
        }
    }

    pub fn start_session_for_request(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: SessionStartRequest,
    ) -> Result<bool> {
        self.request_session_start(request)?;
        self.start_pending_session(device, queue)
    }

    fn render_views(&mut self, views: &[xr::View]) -> Result<[ChunkRenderView; 2]> {
        if views.len() < 2 {
            bail!("OpenXR runtime returned fewer than two stereo views");
        }
        let tracking_origin = self.tracking_origin_for_views(views)?;
        let transform =
            XrStageToWorld::from_tracking_origin(tracking_origin, self.camera.snapshot())?;
        Ok([
            xr_view_to_chunk_render_view(&views[0], transform, XR_NEAR, XR_FAR)?,
            xr_view_to_chunk_render_view(&views[1], transform, XR_NEAR, XR_FAR)?,
        ])
    }

    fn tracking_origin_for_views(&mut self, views: &[xr::View]) -> Result<XrTrackingOrigin> {
        if let Some(origin) = self.tracking_origin {
            return Ok(origin);
        }
        let origin = XrTrackingOrigin::from_initial_views(views, self.initial_alignment_mode)?;
        let snapshot = self.camera.snapshot();
        log::info!(
            "mclone XR terrain player-root alignment: mode={} root_eye=({:.2}, {:.2}, {:.2}) root_yaw_degrees={:.1} stage_center=({:.3}, {:.3}, {:.3}) stage_yaw_degrees={:.1}",
            origin.mode_label(),
            snapshot.eye.x,
            snapshot.eye.y,
            snapshot.eye.z,
            snapshot.yaw_radians.to_degrees(),
            origin.origin_stage.x,
            origin.origin_stage.y,
            origin.origin_stage.z,
            origin.stage_yaw.to_degrees()
        );
        self.tracking_origin = Some(origin);
        Ok(origin)
    }

    fn reconcile_room_scale_body_to_headset(
        &mut self,
        views: &[xr::View],
    ) -> Result<XrStageToWorld> {
        let origin = self.tracking_origin_for_views(views)?;
        let transform = XrStageToWorld::from_tracking_origin(origin, self.camera.snapshot())?;
        let headset_stage_position = xr_headset_stage_position_from_views(views)?;
        let headset_world_position = transform.transform_position(headset_stage_position);
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(transform);
        };
        let reconciliation = self.camera.reconcile_room_scale_headset(
            runtime.client(),
            vec3d_from_glam(headset_world_position),
        );
        let consumed_world = glam_vec3_from_vec3d(reconciliation.consumed_body_movement);
        let origin = origin.consume_world_movement(consumed_world, transform);
        self.tracking_origin = Some(origin);
        XrStageToWorld::from_tracking_origin(origin, self.camera.snapshot())
    }

    fn snap_turn_delta_from_controllers(
        &mut self,
        controllers: &[XrControllerSnapshot],
    ) -> Option<f64> {
        let right_axis = xr_right_stick_axis(controllers);
        self.snap_turn_state.update(right_axis.x, self.turn_policy)
    }

    fn apply_snap_turn_preserving_headset(
        &mut self,
        views: &[xr::View],
        before_transform: XrStageToWorld,
        yaw_delta_radians: f64,
    ) -> Result<XrStageToWorld> {
        if !yaw_delta_radians.is_finite() || yaw_delta_radians.abs() <= f64::EPSILON {
            return Ok(before_transform);
        }
        let Some(origin_before) = self.tracking_origin else {
            return Ok(before_transform);
        };
        let headset_stage_position = xr_headset_stage_position_from_views(views)?;
        let headset_world_before = before_transform.transform_position(headset_stage_position);
        self.camera.turn_yaw_delta(yaw_delta_radians);
        let origin_after = origin_before.rebase_for_stage_position_world_position(
            headset_stage_position,
            headset_world_before,
            self.camera.snapshot(),
        )?;
        self.tracking_origin = Some(origin_after);
        XrStageToWorld::from_tracking_origin(origin_after, self.camera.snapshot())
    }

    fn update_head_comfort_state(
        &mut self,
        transform: XrStageToWorld,
        views: &[xr::View],
        dt_seconds: f64,
    ) -> Result<()> {
        let headset_stage_position = xr_headset_stage_position_from_views(views)?;
        let headset_world_position = transform.transform_position(headset_stage_position);
        let Some(runtime) = self.runtime.as_ref() else {
            self.head_comfort.reset();
            return Ok(());
        };
        let target = xr_head_comfort_target(
            self.camera.last_room_scale_reconciliation(),
            headset_world_position,
            runtime.client(),
        );
        self.head_comfort.update(target, dt_seconds);
        Ok(())
    }

    fn locomotion_movement_yaw_radians(&mut self, views: &[xr::View]) -> Result<Option<f64>> {
        let tracking_origin = self.tracking_origin_for_views(views)?;
        let transform =
            XrStageToWorld::from_tracking_origin(tracking_origin, self.camera.snapshot())?;
        self.locomotion_movement_yaw_radians_with_transform(views, transform)
    }

    fn locomotion_movement_yaw_radians_with_transform(
        &self,
        views: &[xr::View],
        transform: XrStageToWorld,
    ) -> Result<Option<f64>> {
        match self.locomotion_mode {
            XrLocomotionMode::PlayerYaw => Ok(None),
            XrLocomotionMode::HeadsetYaw => {
                xr_headset_world_yaw_from_views(views, transform).map(|yaw| Some(f64::from(yaw)))
            }
        }
    }

    fn refresh_traversal_ready_sections(
        &mut self,
        camera_position: Vec3,
        upload_backpressured: bool,
        skip_refresh: bool,
        timing: &mut XrTerrainFrameTiming,
    ) -> usize {
        let ready_start = Instant::now();
        if skip_refresh {
            timing.runtime_ready_sections_ms = elapsed_ms(ready_start.elapsed());
            let ready_publish_start = Instant::now();
            self.draw
                .record_traversal_ready_sections_skipped(upload_backpressured);
            timing.runtime_ready_publish_ms = elapsed_ms(ready_publish_start.elapsed());
            return self.draw.traversal_ready_section_count();
        }
        let draw_section_generation = self.draw.traversal_ready_source_generation();
        let refresh = {
            let runtime = self
                .runtime
                .as_ref()
                .expect("runtime presence checked before ready refresh");
            self.traversal_ready_sections.refresh(
                runtime.core(),
                camera_position,
                draw_section_generation,
            )
        };
        timing.runtime_ready_sections_ms = elapsed_ms(ready_start.elapsed());
        let ready_publish_start = Instant::now();
        if refresh.refreshed {
            self.draw.set_traversal_ready_sections_with_context(
                self.traversal_ready_sections.ready_sections(),
                upload_backpressured,
            );
        } else {
            self.draw
                .record_traversal_ready_sections_skipped(upload_backpressured);
        }
        timing.runtime_ready_publish_ms = elapsed_ms(ready_publish_start.elapsed());
        refresh.section_count
    }

    fn poll_runtime_and_upload(
        &mut self,
        device: &wgpu::Device,
        camera_position: Vec3,
        frame_deadline: Option<Instant>,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadSummary> {
        let (
            pending_render_chunks_before,
            pending_compile_jobs_before,
            max_pending_compile_jobs,
            available_compile_slots_before,
        ) = if let Some(runtime) = self.runtime.as_ref() {
            (
                runtime.pending_render_chunk_count(),
                runtime.render_compile_pending_job_count(),
                runtime.render_compile_max_pending_job_count(),
                runtime.render_compile_available_pending_job_slots(),
            )
        } else {
            let upload_queue = self.section_uploads.stats();
            return Ok(XrTerrainUploadSummary {
                host_mode: self.runtime_host_mode(),
                queued_upload_section_count: upload_queue.queued_upload_sections,
                queued_upload_removed_section_count: upload_queue.queued_removed_sections,
                queued_upload_lifecycle_item_count: upload_queue.queued_lifecycle_items,
                upload_held_lifecycle_item_count: upload_queue.held_release_lifecycle_items,
                upload_held_compile_job_count: upload_queue.held_compile_jobs,
                traversal_ready_section_count: self.draw.traversal_ready_section_count(),
                record_cache: self.draw.record_cache_stats(),
                ..XrTerrainUploadSummary::default()
            });
        };
        let poll_start = Instant::now();
        let poll_changed = self.poll().context("poll XR terrain runtime")?;
        timing.runtime_poll_ms = elapsed_ms(poll_start.elapsed());
        let (poll_summary, has_runtime_render_work) = {
            let runtime = self
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
        if !poll_changed && !has_runtime_render_work && !self.section_uploads.has_pending_work() {
            let traversal_ready_section_count =
                self.refresh_traversal_ready_sections(camera_position, false, false, timing);
            let runtime = self
                .runtime
                .as_ref()
                .expect("runtime presence checked before poll");
            let upload_queue = self.section_uploads.stats();
            return Ok(XrTerrainUploadSummary {
                poll_changed,
                pending_render_chunks_before,
                pending_render_chunks_after: runtime.pending_render_chunk_count(),
                pending_compile_jobs_before,
                pending_compile_jobs_after: runtime.render_compile_pending_job_count(),
                max_pending_compile_jobs,
                available_compile_slots_before,
                available_compile_slots_after: runtime.render_compile_available_pending_job_slots(),
                queued_completed_compile_result_count: runtime
                    .pending_completed_compile_result_count(),
                queued_upload_section_count: upload_queue.queued_upload_sections,
                queued_upload_removed_section_count: upload_queue.queued_removed_sections,
                queued_upload_lifecycle_item_count: upload_queue.queued_lifecycle_items,
                upload_held_lifecycle_item_count: upload_queue.held_release_lifecycle_items,
                upload_held_compile_job_count: upload_queue.held_compile_jobs,
                traversal_ready_section_count,
                record_cache: self.draw.record_cache_stats(),
                ..poll_summary
            });
        }
        let upload_frame_policy = RenderSectionUploadFramePolicy::new(
            self.render_section_upload_budget,
            self.render_section_accept_budget,
        );
        let mut upload_report = TexturedSectionUploadReport::default();
        let mut upload_phase = RenderSectionUploadPhaseReport::default();
        let drained_pending_uploads_before_sync = self
            .section_uploads
            .should_drain_before_runtime_sync(upload_frame_policy);
        if drained_pending_uploads_before_sync {
            let upload_start = Instant::now();
            let drained_report = self.apply_section_update_uploads(
                device,
                RenderSectionCacheUpdate::default(),
                false,
                timing,
            )?;
            let upload_elapsed_ms = elapsed_ms(upload_start.elapsed());
            timing.runtime_gpu_upload_ms += upload_elapsed_ms;
            timing.runtime_gpu_upload_pre_sync_ms += upload_elapsed_ms;
            accumulate_upload_report(&mut upload_report, drained_report.upload);
            upload_phase.absorb(drained_report.phase);
            self.release_render_compile_jobs(drained_report.release_compile_jobs);
        }
        let runtime_work_requested = poll_changed || has_runtime_render_work;
        let upload_frame_decision = self.section_uploads.frame_decision_after_pre_sync_drain(
            upload_frame_policy,
            runtime_work_requested,
            drained_pending_uploads_before_sync,
        );
        let should_sync_render_sections = upload_frame_decision.should_sync_render_sections;
        let sync_start = Instant::now();
        let timed_section_update = if should_sync_render_sections {
            if let Some(deadline) = frame_deadline {
                self.sync_render_sections_until_deadline_timed(camera_position, deadline)
                    .context("sync XR terrain render sections until deadline")?
            } else {
                self.sync_render_sections_timed(camera_position)
                    .context("sync XR terrain render sections")?
            }
        } else {
            mclone_app_runtime::TimedRenderSectionCacheUpdate::default()
        };
        timing.runtime_sync_ms = elapsed_ms(sync_start.elapsed());
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
        let section_update = timed_section_update.cache_update;
        let rebuilt_section_count = section_update.rebuilt_section_count();
        let removed_section_count = section_update.removed_section_count();
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
            self.runtime.as_ref().map_or(0, |runtime| {
                runtime.pending_completed_compile_result_count()
            })
        };
        let completed_compile_section_count = section_update.completed_compile_section_count;
        let stale_compile_section_count = section_update.stale_compile_section_count;
        let mut pending_compile_jobs_after_sync = if should_sync_render_sections {
            section_update.pending_compile_jobs
        } else {
            self.runtime
                .as_ref()
                .map_or(0, |runtime| runtime.render_compile_pending_job_count())
        };
        let visibility_graph_build_count = section_update.visibility_graph_stats.build_count;
        let visibility_graph_total_ms = section_update.visibility_graph_stats.total_ms;
        let visibility_graph_worst_ms = section_update.visibility_graph_stats.worst_ms;
        if upload_frame_decision.should_apply_section_update_after_sync {
            let upload_start = Instant::now();
            let section_update_report = self.apply_section_update_uploads(
                device,
                section_update,
                upload_frame_decision.upload_backpressured,
                timing,
            )?;
            let upload_elapsed_ms = elapsed_ms(upload_start.elapsed());
            timing.runtime_gpu_upload_ms += upload_elapsed_ms;
            timing.runtime_gpu_upload_post_sync_ms += upload_elapsed_ms;
            accumulate_upload_report(&mut upload_report, section_update_report.upload);
            upload_phase.absorb(section_update_report.phase);
            self.release_render_compile_jobs(section_update_report.release_compile_jobs);
            pending_compile_jobs_after_sync = self
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
        let traversal_ready_section_count = self.refresh_traversal_ready_sections(
            camera_position,
            upload_frame_decision.upload_backpressured,
            skip_ready_refresh,
            timing,
        );
        let runtime = self
            .runtime
            .as_ref()
            .expect("runtime presence checked before section sync");
        self.render_stats.section_count = self.draw.section_count();
        self.render_stats.index_count = self.draw.index_count();
        self.render_stats.face_count = quad_face_count_from_indices(self.render_stats.index_count);
        self.render_stats.last_rebuilt_section_count = rebuilt_section_count;
        self.render_stats.last_removed_section_count = removed_section_count;
        self.render_stats.last_rebuilt_vertex_count = rebuilt_vertex_count;
        self.render_stats.last_rebuilt_face_count =
            quad_face_count_from_indices(rebuilt_index_count);
        self.render_stats.last_rebuilt_index_count = rebuilt_index_count;
        self.render_stats.last_neighbor_ready_section_count = neighbor_ready_section_count;
        self.render_stats.last_near_exception_section_count = near_exception_section_count;
        self.render_stats.last_deferred_section_count = deferred_section_count;
        self.render_stats.last_submitted_compile_section_count = submitted_compile_section_count;
        self.render_stats.last_completed_compile_section_count = completed_compile_section_count;
        self.render_stats.last_stale_compile_section_count = stale_compile_section_count;
        self.render_stats.last_pending_compile_jobs = pending_compile_jobs_after_sync;
        self.render_stats.last_visibility_graph_build_count = visibility_graph_build_count;
        self.render_stats.last_visibility_graph_total_ms = visibility_graph_total_ms;
        self.render_stats.last_visibility_graph_worst_ms = visibility_graph_worst_ms;
        self.render_stats.last_uploaded_section_count = upload_report.uploaded_section_count;
        self.render_stats.last_upload_removed_section_count = upload_report.removed_section_count;
        self.render_stats.last_uploaded_vertex_count = upload_report.uploaded_vertex_count;
        self.render_stats.last_uploaded_face_count = upload_report.uploaded_face_count();
        self.render_stats.last_uploaded_index_count = upload_report.uploaded_index_count;
        let upload_queue = self.section_uploads.stats();
        Ok(XrTerrainUploadSummary {
            poll_changed,
            pending_render_chunks_before,
            pending_render_chunks_after: runtime.pending_render_chunk_count(),
            pending_compile_jobs_before,
            pending_compile_jobs_after: pending_compile_jobs_after_sync,
            max_pending_compile_jobs,
            available_compile_slots_before,
            available_compile_slots_after: runtime.render_compile_available_pending_job_slots(),
            rebuilt_section_count,
            removed_section_count,
            rebuilt_vertex_count,
            rebuilt_index_count,
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
            record_cache: self.draw.record_cache_stats(),
            visibility_graph_build_count,
            visibility_graph_total_ms,
            visibility_graph_worst_ms,
            ..poll_summary
        })
    }

    fn apply_section_update_uploads(
        &mut self,
        device: &wgpu::Device,
        section_update: RenderSectionCacheUpdate,
        upload_backpressured: bool,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadApplyReport> {
        if self.render_section_upload_budget.is_none()
            && self.render_section_accept_budget.is_none()
            && !self.section_uploads.has_pending_work()
        {
            let release_compile_jobs =
                RenderSectionUploadCoordinator::direct_release_count(&section_update);
            let phase = RenderSectionUploadPhaseReport::direct(
                section_update.accepted_compile_result_count,
                section_update.rebuilt_sections.len(),
                section_update.removed_section_keys.len(),
                release_compile_jobs,
            );
            let apply_start = Instant::now();
            let report = self
                .draw
                .apply_section_updates_with_context_timed(
                    device,
                    &section_update.rebuilt_sections,
                    &section_update.removed_section_keys,
                    upload_backpressured,
                )
                .context("upload XR terrain render section updates");
            timing.runtime_upload_apply_ms += elapsed_ms(apply_start.elapsed());
            return report.map(|(upload, upload_timing)| {
                timing.absorb_upload_apply_timing(upload_timing);
                XrTerrainUploadApplyReport {
                    upload,
                    phase,
                    release_compile_jobs,
                }
            });
        }

        let enqueue_start = Instant::now();
        let mut phase = self.section_uploads.enqueue_cache_update(section_update);
        timing.runtime_upload_enqueue_ms += elapsed_ms(enqueue_start.elapsed());
        let select_start = Instant::now();
        let drain = self.section_uploads.drain_budgeted(
            self.render_section_upload_budget,
            self.render_section_accept_budget,
        );
        phase.absorb(drain.phase_report);
        timing.runtime_upload_select_ms += elapsed_ms(select_start.elapsed());
        if drain.is_empty() {
            return Ok(XrTerrainUploadApplyReport {
                upload: TexturedSectionUploadReport::default(),
                phase,
                release_compile_jobs: phase.released_compile_jobs,
            });
        }
        let apply_start = Instant::now();
        let report = self
            .draw
            .apply_section_updates_with_context_timed(
                device,
                &drain.rebuilt_sections,
                &drain.removed_section_keys,
                upload_backpressured,
            )
            .context("accept budgeted XR terrain render section updates");
        timing.runtime_upload_apply_ms += elapsed_ms(apply_start.elapsed());
        report.map(|(upload, upload_timing)| {
            timing.absorb_upload_apply_timing(upload_timing);
            let released_on_apply = self
                .section_uploads
                .complete_applied_lifecycle_items(drain.lifecycle_item_count);
            phase.record_applied_release(released_on_apply, self.section_uploads.stats());
            XrTerrainUploadApplyReport {
                upload,
                phase,
                release_compile_jobs: phase.released_compile_jobs,
            }
        })
    }

    fn release_render_compile_jobs(&mut self, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        self.runtime
            .as_mut()
            .map_or(0, |runtime| runtime.release_render_compile_jobs(count))
    }

    fn frozen_runtime_upload_summary(&self) -> XrTerrainUploadSummary {
        let pending_render_chunks = self
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.pending_render_chunk_count());
        let pending_compile_jobs = self
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.render_compile_pending_job_count());
        let max_pending_compile_jobs = self
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.render_compile_max_pending_job_count());
        let available_compile_slots = self.runtime.as_ref().map_or(0, |runtime| {
            runtime.render_compile_available_pending_job_slots()
        });
        let upload_queue = self.section_uploads.stats();
        XrTerrainUploadSummary {
            host_mode: self.runtime_host_mode(),
            pending_render_chunks_before: pending_render_chunks,
            pending_render_chunks_after: pending_render_chunks,
            pending_compile_jobs_before: pending_compile_jobs,
            pending_compile_jobs_after: pending_compile_jobs,
            max_pending_compile_jobs,
            available_compile_slots_before: available_compile_slots,
            available_compile_slots_after: available_compile_slots,
            queued_upload_section_count: upload_queue.queued_upload_sections,
            queued_upload_removed_section_count: upload_queue.queued_removed_sections,
            queued_upload_lifecycle_item_count: upload_queue.queued_lifecycle_items,
            upload_held_lifecycle_item_count: upload_queue.held_release_lifecycle_items,
            upload_held_compile_job_count: upload_queue.held_compile_jobs,
            traversal_ready_section_count: self.draw.traversal_ready_section_count(),
            record_cache: self.draw.record_cache_stats(),
            ..XrTerrainUploadSummary::default()
        }
    }

    fn runtime_host_mode(&self) -> XrTerrainHostMode {
        self.runtime
            .as_ref()
            .map_or(XrTerrainHostMode::LocalIntegrated, |runtime| {
                runtime.host_mode().into()
            })
    }

    fn underwater_overlays(
        &mut self,
        render_views: [ChunkRenderView; 2],
    ) -> [Option<UnderwaterOverlay>; 2] {
        let dt_seconds = self.underwater_effect_dt_seconds();
        match self.scene.underwater_detection_mode {
            XrUnderwaterDetectionMode::Midpoint => {
                let center_position =
                    (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
                let underwater = self.camera_inside_water(center_position);
                let effect = self
                    .underwater_effects
                    .midpoint
                    .update(underwater, dt_seconds);
                let overlay = underwater.then(|| {
                    let forward = average_unit_direction(
                        render_views[0].camera_forward,
                        render_views[1].camera_forward,
                        Vec3::Z,
                    );
                    underwater_overlay_from_forward(
                        forward,
                        effect.water_vision,
                        effect.effect_strength,
                    )
                });
                [overlay, overlay]
            }
            XrUnderwaterDetectionMode::PerEye => {
                let left_underwater = self.camera_inside_water(render_views[0].camera_position);
                let right_underwater = self.camera_inside_water(render_views[1].camera_position);
                let left_effect =
                    self.underwater_effects.eyes[0].update(left_underwater, dt_seconds);
                let right_effect =
                    self.underwater_effects.eyes[1].update(right_underwater, dt_seconds);
                [
                    left_underwater.then(|| {
                        underwater_overlay_from_forward(
                            render_views[0].camera_forward,
                            left_effect.water_vision,
                            left_effect.effect_strength,
                        )
                    }),
                    right_underwater.then(|| {
                        underwater_overlay_from_forward(
                            render_views[1].camera_forward,
                            right_effect.water_vision,
                            right_effect.effect_strength,
                        )
                    }),
                ]
            }
        }
    }

    fn current_actor_instances(&self) -> Vec<ActorInstance> {
        if self.scene.skip_actors {
            return Vec::new();
        }
        self.runtime.as_ref().map_or_else(Vec::new, |runtime| {
            actor_instances_from_presentations(
                &runtime.client().actor_presentations(),
                runtime.client(),
            )
        })
    }

    fn underwater_effect_dt_seconds(&mut self) -> f32 {
        let now = Instant::now();
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
        target: XrTerrainEyeTarget<'_>,
        render_view: ChunkRenderView,
        diagnostic_panel: WorldGuiPanel,
        actor_instances: &[mclone_render::entity::ActorInstance],
        render_options: TexturedSectionRenderOptions,
        sky_clear_color: wgpu::Color,
        time_of_day: f32,
        sun_angle: f32,
        underwater_overlay: Option<UnderwaterOverlay>,
        label: &'static str,
        view_slot: PerViewSlot,
        wait_after_submit: bool,
    ) -> Result<XrRenderedEye> {
        let collect_split_timing = self.render_split_timing_enabled;
        let encode_start = collect_split_timing.then(Instant::now);
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
        let mut render_stats = self.render_stats;
        let far_lod_config = self.scene.far_lod;
        let far_lod_seed = self.scene.seed;
        let far_lod_center = self.camera.snapshot().chunk_pos;
        let far_lod_mesh = self.runtime.as_mut().and_then(|runtime| {
            runtime.prepare_far_lod_mesh(
                far_lod_config,
                far_lod_seed,
                far_lod_center,
                render_view.camera_position,
            )
        });
        let full_frame_start = collect_split_timing.then(Instant::now);
        let (summary, frame_timing) = if collect_split_timing {
            let far_lod = far_lod_mesh.map(|_| &mut self.far_lod);
            render_full_frame_for_view_with_prepared_stereo_draw_timed_in_slot(
                frame,
                target.depth,
                &self.sky,
                &mut self.draw,
                prepared_draw,
                Some(&mut self.actors),
                Some(&mut self.screen_effects),
                None,
                render_view,
                actor_instances,
                underwater_overlay,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                |_| summary_ui_draw,
                far_lod,
                far_lod_mesh,
                &mut render_stats,
                view_slot,
            )
        } else {
            let far_lod = far_lod_mesh.map(|_| &mut self.far_lod);
            render_full_frame_for_view_with_prepared_stereo_draw_in_slot(
                frame,
                target.depth,
                &self.sky,
                &mut self.draw,
                prepared_draw,
                Some(&mut self.actors),
                Some(&mut self.screen_effects),
                None,
                render_view,
                actor_instances,
                underwater_overlay,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                |_| summary_ui_draw,
                far_lod,
                far_lod_mesh,
                &mut render_stats,
                view_slot,
            )
            .map(|summary| (summary, Default::default()))
        }
        .with_context(|| format!("render XR terrain {label} eye"))?;
        let full_frame_ms = full_frame_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let mut xr_fade_ms = 0.0;
        if let Some(overlay) = self.head_comfort.overlay() {
            let fade_start = collect_split_timing.then(Instant::now);
            self.screen_effects.render_fade_in_slot(
                device,
                queue,
                &mut encoder,
                RenderFrameTarget::color(target.color_view, target.size),
                overlay,
                view_slot,
            );
            xr_fade_ms = fade_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        }
        let selection_start = collect_split_timing.then(Instant::now);
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
        let xr_selection_ms = selection_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let mut world_lines = engine_debug_world_lines(
            &self.camera,
            EngineDebugVisualOptions::new(self.player_collision_box_visible),
        );
        if let Some(gameplay_ray) = self
            .xr_gameplay_controller_ray_line()
            .context("build XR gameplay controller ray visual")?
        {
            world_lines.push(gameplay_ray);
        }
        world_lines.extend(self.xr_blink_teleport_lines());
        let mut xr_world_lines_ms = 0.0;
        if !world_lines.is_empty() {
            let world_lines_start = collect_split_timing.then(Instant::now);
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
            xr_world_lines_ms = world_lines_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        }
        let mut ui_panel_stats = WorldGuiPanelRenderStats::default();
        let mut ui_draw_cache_stats = panel_draw.draw_cache;
        let mut xr_world_panel_ms = 0.0;
        let diagnostic_panel_start = collect_split_timing.then(Instant::now);
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
        xr_world_panel_ms +=
            diagnostic_panel_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        if ui_active {
            if let Some(panel) = self.menu_panel_pose {
                let world_panel_start = collect_split_timing.then(Instant::now);
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
                xr_world_panel_ms +=
                    world_panel_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
            }
        }
        let finish_start = collect_split_timing.then(Instant::now);
        let command_buffer = encoder.finish();
        let encoder_finish_ms = finish_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let encode_total_ms = encode_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let submit_start = collect_split_timing.then(Instant::now);
        let submission = queue.submit(Some(command_buffer));
        let submit_ms = submit_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let poll_start = collect_split_timing.then(Instant::now);
        let (submission, poll_wait_ms) = if wait_after_submit {
            Self::wait_for_xr_submission(
                device,
                submission,
                &format!("XR terrain {label}-eye render"),
            )?;
            (
                None,
                poll_start.map_or(0.0, |start| elapsed_ms(start.elapsed())),
            )
        } else {
            (Some(submission), 0.0)
        };
        if label == "left" {
            self.render_stats = render_stats;
        }
        let prepare_ms = frame_timing.terrain_prepare_ms;
        Ok(XrRenderedEye {
            summary,
            timing: XrTerrainEyeRenderTiming {
                full_frame_ms,
                sky_ms: frame_timing.sky_ms,
                far_lod_ms: frame_timing.far_lod_ms,
                terrain_opaque_ms: frame_timing.terrain_opaque_ms,
                terrain_translucent_ms: frame_timing.terrain_translucent_ms,
                prepare_ms,
                cull_ms: frame_timing.terrain_cull_ms,
                uniform_write_ms: frame_timing.terrain_uniform_write_ms,
                translucent_collect_ms: frame_timing.terrain_translucent_collect_ms,
                translucent_sort_ms: frame_timing.terrain_translucent_sort_ms,
                encode_ms: (encode_total_ms - prepare_ms).max(0.0),
                section_encode_ms: frame_timing.terrain_encode_ms,
                actor_ms: frame_timing.actor_ms,
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

    fn sync_render_sections_timed(
        &mut self,
        camera_position: Vec3,
    ) -> Result<mclone_app_runtime::TimedRenderSectionCacheUpdate> {
        let completed_result_accept_budget = self.render_completed_result_accept_budget;
        self.runtime
            .as_mut()
            .context("XR terrain runtime is not active")?
            .sync_render_sections_with_completed_result_acceptance_timed(
                camera_position,
                completed_result_accept_budget,
            )
    }

    fn sync_render_sections_until_deadline_timed(
        &mut self,
        camera_position: Vec3,
        deadline: Instant,
    ) -> Result<mclone_app_runtime::TimedRenderSectionCacheUpdate> {
        let completed_result_accept_budget = self.render_completed_result_accept_budget;
        self.runtime
            .as_mut()
            .context("XR terrain runtime is not active")?
            .sync_render_sections_until_deadline_with_completed_result_acceptance_timed(
                camera_position,
                deadline,
                completed_result_accept_budget,
            )
    }

    fn poll(&mut self) -> Result<bool> {
        self.runtime
            .as_mut()
            .context("XR terrain runtime is not active")?
            .poll()
    }

    fn commit_engine_camera_player_pose_timed(&mut self) -> Result<(bool, XrCameraCommitTiming)> {
        let Some(runtime) = self.runtime.as_mut() else {
            return Ok((false, XrCameraCommitTiming::default()));
        };
        commit_engine_camera_player_pose_for_runtime_timed(
            runtime,
            &mut self.camera,
            "sync XR terrain player pose",
        )
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

    fn sky_clear_color(&self) -> wgpu::Color {
        self.runtime.as_ref().map_or_else(
            || overworld_clear_color(self.time_of_day()),
            |runtime| runtime.sky_clear_color(),
        )
    }

    fn time_of_day(&self) -> f32 {
        self.runtime.as_ref().map_or_else(
            || time::time_of_day(self.scene.day_time_override.unwrap_or(0)),
            |runtime| runtime.time_of_day(),
        )
    }

    fn sun_angle(&self) -> f32 {
        self.runtime.as_ref().map_or_else(
            || time::sun_angle(self.scene.day_time_override.unwrap_or(0)),
            |runtime| runtime.sun_angle(),
        )
    }

    fn effective_render_options(&self, camera_position: Vec3) -> TexturedSectionRenderOptions {
        let mut options = self.render_options;
        if self.camera_inside_occluding_block(camera_position) {
            options.section_occlusion_culling = false;
        }
        options
    }

    fn current_ui_render_state(&self) -> GameUiRenderState {
        let render_distance = self.local_startup.as_ref().map_or_else(
            || {
                self.runtime
                    .as_ref()
                    .map_or(self.scene.render_distance, |runtime| {
                        runtime.render_distance()
                    })
            },
            |startup| startup.scene.render_distance,
        );
        let block_palette = self.runtime.as_ref().map_or_else(
            || {
                self.local_startup
                    .as_ref()
                    .map_or_else(Default::default, |startup| {
                        debug_block_palette_overlay(
                            &startup.pump.runtime().mesh_assets().catalog,
                            self.interaction.selected_hotbar_slot(),
                        )
                    })
            },
            |runtime| {
                debug_block_palette_overlay(
                    &runtime.mesh_assets().catalog,
                    self.interaction.selected_hotbar_slot(),
                )
            },
        );
        GameUiRenderState {
            world_catalog: self
                .client_experience
                .catalog()
                .ui_state_with_active_world(self.active_local_world_id()),
            render_distance: (render_distance as i32).clamp(1, MAX_XR_RENDER_DISTANCE as i32),
            min_render_distance: 1,
            max_render_distance: MAX_XR_RENDER_DISTANCE as i32,
            section_occlusion_culling: self.render_options.section_occlusion_culling,
            force_fullbright: self.render_options.force_fullbright,
            far_lod_enabled: self.scene.far_lod.enabled,
            far_lod_range_chunks: self.scene.far_lod.extra_radius_chunks as i32,
            min_far_lod_range_chunks: MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
            max_far_lod_range_chunks: MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS as i32,
            player_collision_box_visible: self.player_collision_box_visible,
            first_person_player_visible: self.camera.first_person_player_visible(),
            crosshair_visible: None,
            frame_pipeline_overlay_visible: self.diagnostic_panel.frame_metrics_visible(),
            debug_diagnostics_visible: self.diagnostic_panel.debug_diagnostics_visible(),
            player_model: self.player_model,
            movement_mode: game_movement_mode(self.camera.movement_mode()),
            collision_mode: Some(game_collision_mode(self.camera.collision_mode())),
            travel_assist_mode: Some(self.travel_assist_mode),
            turn_mode: Some(self.turn_policy.game_mode().into()),
            xr_turn_mode: Some(self.turn_policy.game_mode()),
            fly_speed_multiplier: self.camera.fly_speed_multiplier() as f32,
            min_fly_speed_multiplier: ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER as f32,
            max_fly_speed_multiplier: ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER as f32,
            movement_speed_multiplier: self.camera.movement_speed_multiplier() as f32,
            min_movement_speed_multiplier: ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER as f32,
            max_movement_speed_multiplier: ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER as f32,
            frame_pacing_mode: GameFramePacingMode::Vsync,
            fps_cap: self
                .display_refresh_hz
                .map(|hz| hz.round().clamp(1.0, 999.0) as u32)
                .unwrap_or(XR_UI_FPS_CAP),
            server_cadence: None,
            touch_controls_mode: None,
            touch_settings: None,
            block_palette,
        }
    }

    fn refresh_debug_diagnostics_overlay(&mut self) {
        if !self.diagnostic_panel.debug_diagnostics_visible() {
            self.diagnostic_panel.clear_debug_overlay();
            return;
        }
        let overlay = self.debug_diagnostics_overlay();
        self.diagnostic_panel.set_debug_overlay(overlay);
    }

    fn debug_diagnostics_overlay(&self) -> DebugOverlay {
        let snapshot = self.camera.snapshot();
        let runtime_stats = self.runtime.as_ref().map(|runtime| runtime.stats());
        let render_distance =
            runtime_stats.map_or(self.scene.render_distance, |stats| stats.render_distance);
        let tracking_radius = runtime_stats.map_or(0, |stats| stats.chunk_tracking_radius);
        let interest_center =
            runtime_stats.map_or(snapshot.chunk_pos, |stats| stats.interest_center);
        let host = runtime_stats
            .map(|stats| stats.host_mode.label().to_ascii_uppercase())
            .unwrap_or_else(|| "STARTUP".to_owned());
        let runner = runtime_stats
            .and_then(|stats| stats.server_runner_kind)
            .map(|kind| kind.label().to_ascii_uppercase())
            .unwrap_or_else(|| "REMOTE".to_owned());
        let actor_indices = self.render_stats.drawn_actor_index_count;
        let lines = vec![
            format!(
                "POS {:.1} {:.1} {:.1}",
                snapshot.eye.x, snapshot.eye.y, snapshot.eye.z
            ),
            format!(
                "CHUNK {} {} SPEED {:.1}",
                snapshot.chunk_pos.x,
                snapshot.chunk_pos.z,
                self.camera.speed_blocks_per_second()
            ),
            format!(
                "MODE {}/{} GROUND {}",
                self.camera.movement_mode().label(),
                self.camera.collision_mode().label(),
                if self.camera.on_ground() { "Y" } else { "N" }
            ),
            format!(
                "VIEW R{} T{} C{} {}",
                render_distance, tracking_radius, interest_center.x, interest_center.z
            ),
            format!(
                "HOST {} {} SQ{} UQ{}",
                host,
                runner,
                runtime_stats.map_or(0, |stats| stats.server_command_queue_depth),
                runtime_stats.map_or(0, |stats| stats.server_update_queue_depth)
            ),
            format!(
                "CHUNKS L{} V{} P{}",
                runtime_stats.map_or(0, |stats| stats.loaded_chunks),
                runtime_stats.map_or(0, |stats| stats.client_visible_chunks),
                runtime_stats.map_or(0, |stats| stats.pending_jobs)
            ),
            format!(
                "DRAW S {}/{} F {}/{}",
                self.render_stats.drawn_section_count,
                self.render_stats.section_count,
                self.render_stats.drawn_face_count,
                self.render_stats.face_count
            ),
            format!(
                "ACTOR R {}/{} I{}",
                self.render_stats.drawn_actor_count, self.render_stats.actor_count, actor_indices
            ),
            format!(
                "MESH R{} U{} D{} SQ{} CQ{} X{}",
                self.render_stats.last_rebuilt_section_count,
                self.render_stats.last_uploaded_section_count,
                self.render_stats.last_deferred_section_count,
                self.render_stats.last_submitted_compile_section_count,
                self.render_stats.last_completed_compile_section_count,
                self.render_stats.last_stale_compile_section_count
            ),
            format!(
                "PENDING R{} C{}",
                runtime_stats.map_or(0, |stats| stats.pending_render_chunks),
                self.render_stats.last_pending_compile_jobs
            ),
            format!(
                "OPTIONS OCC {} FULL {} {}",
                if self.render_options.section_occlusion_culling {
                    "Y"
                } else {
                    "N"
                },
                if self.render_options.force_fullbright {
                    "Y"
                } else {
                    "N"
                },
                self.render_options.color_profile.label()
            ),
        ];
        DebugOverlay::new("XR DEBUG", lines)
    }

    fn client_experience_settings_state(&self) -> ClientExperienceSettingsState {
        ClientExperienceSettingsState::from(self.current_ui_render_state())
    }

    fn session_projection(&self) -> ClientSessionStatusProjection {
        client_session_status_projection(
            self.session.status(),
            self.status_overlay.clone(),
            self.startup_progress_overlay(),
        )
    }

    fn startup_progress_overlay(&self) -> Option<LoadingProgressOverlay> {
        self.local_startup
            .as_ref()
            .and_then(|startup| startup.pump.progress_overlay())
    }

    fn active_remote_addr(&self) -> Option<String> {
        match self.session.state() {
            GameSessionState::Active {
                session: ActiveSessionDescriptor::Remote { endpoint },
            } => Some(endpoint.address.clone()),
            GameSessionState::NoSession
            | GameSessionState::Starting { .. }
            | GameSessionState::Active {
                session: ActiveSessionDescriptor::LocalWorld { .. },
            }
            | GameSessionState::Failed { .. } => None,
        }
    }

    fn clear_inactive_session_status(&mut self) {
        if client_session_should_clear_inactive_status(self.session.state()) {
            self.status_overlay = StatusOverlay::hidden();
        }
    }

    fn apply_menu_toggle_input(&mut self, controllers: &[XrControllerSnapshot]) {
        let toggle_down = xr_menu_toggle_pressed(controllers);
        if toggle_down && !self.menu_toggle_down {
            if self.local_startup.is_some() {
                if !self.ui.is_active() {
                    self.ui.open_pause();
                    self.menu_panel_anchor = XrUiPanelAnchor::Head;
                    self.menu_panel_recenter_pending = true;
                }
                self.menu_toggle_down = toggle_down;
                return;
            }
            if self.ui.is_active() {
                self.ui.close();
                self.ui.clear_input();
                self.menu_pointer_down = false;
                self.menu_panel_pose = None;
                self.menu_panel_recenter_pending = false;
                log::info!("XR menu closed");
            } else {
                self.ui.open_pause();
                self.menu_panel_anchor = XrUiPanelAnchor::Head;
                self.menu_panel_recenter_pending = true;
                log::info!("XR menu opened");
            }
        }
        self.menu_toggle_down = toggle_down;
    }

    fn apply_game_ui_toggle_input(&mut self, controllers: &[XrControllerSnapshot]) {
        let toggle_down = xr_game_ui_toggle_pressed(controllers);
        if toggle_down && !self.game_ui_toggle_down {
            if self.local_startup.is_some() || self.runtime.is_none() {
                self.game_ui_toggle_down = toggle_down;
                return;
            }
            if self.ui.screen() == Some(GameScreen::BlockPalette)
                && self.menu_panel_anchor == XrUiPanelAnchor::LeftHand
            {
                self.ui.close();
                self.ui.clear_input();
                self.menu_pointer_down = false;
                self.menu_panel_pose = None;
                self.menu_panel_recenter_pending = false;
                log::info!("XR game UI closed");
            } else {
                self.ui.apply_action(GameUiAction::OpenBlockPalette);
                self.menu_panel_anchor = XrUiPanelAnchor::LeftHand;
                self.menu_pointer_down = false;
                self.menu_panel_pose = None;
                self.menu_panel_recenter_pending = true;
                log::info!("XR game UI opened");
            }
        }
        self.game_ui_toggle_down = toggle_down;
    }

    fn update_menu_panel_pose(&mut self, render_views: [ChunkRenderView; 2]) {
        if !self.ui.is_active() {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            self.menu_panel_pose = None;
            self.menu_panel_anchor = XrUiPanelAnchor::Head;
            self.menu_panel_recenter_pending = false;
            return;
        }
        match self.menu_panel_anchor {
            XrUiPanelAnchor::Head => {
                if self.menu_panel_pose.is_some() && !self.menu_panel_recenter_pending {
                    return;
                }
                self.menu_panel_pose = Some(xr_menu_panel_from_render_views(render_views));
                self.menu_panel_recenter_pending = false;
            }
            XrUiPanelAnchor::LeftHand => {
                let hand_panel = self.tracking_origin.and_then(|origin| {
                    let transform =
                        XrStageToWorld::from_tracking_origin(origin, self.camera.snapshot())
                            .ok()?;
                    xr_game_ui_panel_from_controllers(
                        &self.latest_controllers,
                        transform,
                        render_views,
                    )
                });
                if let Some(panel) = hand_panel {
                    self.menu_panel_pose = Some(panel);
                    self.menu_panel_recenter_pending = false;
                } else if self.menu_panel_pose.is_none() || self.menu_panel_recenter_pending {
                    self.menu_panel_pose = Some(xr_menu_panel_from_render_views(render_views));
                    self.menu_panel_recenter_pending = false;
                }
            }
        }
    }

    fn apply_menu_pointer_input(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if !self.ui.is_active() {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            return Ok(false);
        }
        let Some(panel) = self.menu_panel_pose else {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            return Ok(false);
        };
        let Some(origin) = self.tracking_origin else {
            return Ok(false);
        };
        let transform = XrStageToWorld::from_tracking_origin(origin, self.camera.snapshot())?;
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        self.ui.set_scale(gui_scale);
        let hit = xr_menu_pointer_hit_from_controllers(
            &self.latest_controllers,
            transform,
            panel,
            gui_scale,
        );
        let Some(hit) = hit else {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            return Ok(false);
        };
        let trigger_down = xr_menu_pointer_trigger_down(hit.trigger, self.menu_pointer_down);
        let action = if trigger_down && !self.menu_pointer_down {
            self.ui.pointer_down(hit.point);
            None
        } else if !trigger_down && self.menu_pointer_down {
            let (_handled, action) = self.ui.pointer_up(hit.point);
            action
        } else {
            let (_handled, action) = self.ui.pointer_move(hit.point);
            action
        };
        self.menu_pointer_down = trigger_down;
        if let Some(action) = action {
            return self.apply_xr_ui_action(action, device, queue);
        }
        Ok(false)
    }

    fn update_gameplay_interaction_buttons(
        &mut self,
        controllers: &[XrControllerSnapshot],
    ) -> XrGameplayInteractionEdges {
        let current = xr_gameplay_interaction_buttons_from_controllers(
            controllers,
            self.gameplay_interaction_buttons,
        );
        let edges = self.gameplay_interaction_buttons.press_edges(current);
        self.gameplay_interaction_buttons = current;
        edges
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
            .context("failed to sync XR carried item to server")
    }

    fn sync_player_appearance(&mut self) -> Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(set_player_appearance_command_for_ui_model(
                self.player_model,
            ))
            .context("failed to sync XR player appearance to server")
    }

    fn assign_debug_hotbar_slot(&mut self, slot: u8, block_state: BlockStateId) -> Result<bool> {
        let Some(command) = self
            .interaction
            .set_debug_hotbar_slot(slot, Some(block_state))
        else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime
            .send_gameplay_command(command)
            .context("failed to assign XR debug hotbar slot")
    }

    fn apply_xr_gameplay_interaction_edges(
        &mut self,
        edges: XrGameplayInteractionEdges,
    ) -> Result<()> {
        if !edges.any() {
            return Ok(());
        }
        self.sync_carried_item()?;
        let Some(target) = self.current_xr_block_interaction_target() else {
            return Ok(());
        };
        if edges.attack {
            self.send_xr_gameplay_interaction_command(
                XrGameplayInteractionAction::Attack,
                &target,
            )?;
        }
        if edges.use_item {
            self.send_xr_gameplay_interaction_command(XrGameplayInteractionAction::Use, &target)?;
        }
        Ok(())
    }

    fn send_xr_gameplay_interaction_command(
        &mut self,
        action: XrGameplayInteractionAction,
        target: &BlockInteractionTarget,
    ) -> Result<()> {
        let command = match action {
            XrGameplayInteractionAction::Attack => {
                self.interaction.debug_instant_break_command(target.hit)
            }
            XrGameplayInteractionAction::Use => self.interaction.use_item_on_command(target.hit),
        };
        let Some(command) = command else {
            return Ok(());
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(());
        };
        let changed = runtime
            .send_gameplay_command(command)
            .context("failed to send XR gameplay interaction command")?;
        log::info!(
            "XR gameplay interaction {:?} at ({}, {}, {}) face={:?} changed={}",
            action,
            target.hit.block_pos.x,
            target.hit.block_pos.y,
            target.hit.block_pos.z,
            target.hit.direction,
            changed
        );
        Ok(())
    }

    fn xr_menu_controller_ray_lines(&self, panel: WorldGuiPanel) -> Result<Vec<WorldGuiLine>> {
        let Some(origin) = self.tracking_origin else {
            return Ok(Vec::new());
        };
        let transform = XrStageToWorld::from_tracking_origin(origin, self.camera.snapshot())?;
        Ok(xr_menu_controller_ray_lines_from_controllers(
            &self.latest_controllers,
            transform,
            panel,
        ))
    }

    fn xr_gameplay_controller_ray_line(&self) -> Result<Option<WorldGuiLine>> {
        if self.ui.is_active() {
            return Ok(None);
        }
        let Some(origin) = self.tracking_origin else {
            return Ok(None);
        };
        let transform = XrStageToWorld::from_tracking_origin(origin, self.camera.snapshot())?;
        let Some((ray_origin, ray_direction)) =
            xr_controller_interaction_ray_from_controllers(&self.latest_controllers, transform)
        else {
            return Ok(None);
        };
        let hit_distance = self.runtime.as_ref().and_then(|runtime| {
            self.interaction
                .target_block(
                    runtime.client(),
                    vec3d_from_glam(ray_origin),
                    vec3d_from_glam(ray_direction),
                )
                .map(|target| {
                    let hit = glam_vec3_from_vec3d(target.hit.location);
                    (hit - ray_origin).dot(ray_direction.normalize_or_zero())
                })
        });
        Ok(xr_gameplay_controller_ray_line_from_controllers(
            &self.latest_controllers,
            transform,
            hit_distance,
            self.interaction.pick_range() as f32,
        ))
    }

    fn xr_blink_teleport_lines(&self) -> Vec<WorldGuiLine> {
        let Some(preview) = self.blink_teleport.preview.as_ref() else {
            return Vec::new();
        };
        xr_blink_teleport_lines(preview)
    }

    fn current_xr_block_interaction_target(&self) -> Option<BlockInteractionTarget> {
        if self.ui.is_active() {
            return None;
        }
        let runtime = self.runtime.as_ref()?;
        let origin = self.tracking_origin?;
        let transform =
            XrStageToWorld::from_tracking_origin(origin, self.camera.snapshot()).ok()?;
        let (ray_origin, ray_direction) =
            xr_controller_interaction_ray_from_controllers(&self.latest_controllers, transform)?;
        self.interaction.target_block(
            runtime.client(),
            vec3d_from_glam(ray_origin),
            vec3d_from_glam(ray_direction),
        )
    }

    fn current_xr_selection_outline(&self) -> Option<SelectionOutline> {
        self.current_xr_block_interaction_target()
            .map(|target| SelectionOutline::new(target.outline_boxes))
    }

    fn next_new_world_seed(&mut self) -> i64 {
        self.seed_reroll_state = self
            .seed_reroll_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.seed_reroll_state as i64
    }

    fn local_world_options(&self, seed: i64) -> XrSceneOptions {
        let mut scene = self.scene.clone();
        scene.seed = seed;
        scene.world_dir = None;
        if let Some(runtime) = &self.runtime {
            scene.render_distance = runtime.render_distance();
        }
        scene
    }

    fn remote_session_options(&self) -> XrSceneOptions {
        let mut scene = self.scene.clone();
        scene.world_dir = None;
        if let Some(runtime) = &self.runtime {
            scene.render_distance = runtime.render_distance();
        }
        scene
    }

    fn catalog_world_scene(
        &self,
        summary: &LocalWorldSummary,
        world_dir: PathBuf,
    ) -> XrSceneOptions {
        let mut scene = self.local_world_options(summary.seed);
        scene.world_dir = Some(world_dir);
        scene
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
                log::warn!("failed to refresh XR local world catalog: {error}");
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

    fn request_session_start(&mut self, request: SessionStartRequest) -> Result<()> {
        let payload = self.pending_session_payload_for_request(&request)?;
        self.status_overlay = StatusOverlay::hidden();
        self.session.request_start(request, payload);
        Ok(())
    }

    fn pending_session_payload_for_request(
        &self,
        request: &SessionStartRequest,
    ) -> Result<XrPendingSessionStart> {
        match request {
            SessionStartRequest::CreateLocalWorld { options } => Ok(XrPendingSessionStart {
                scene: self.local_world_options(options.seed),
                descriptor: request.active_descriptor(),
            }),
            SessionStartRequest::OpenLocalWorld { id } => {
                let catalog = self
                    .world_catalog
                    .as_ref()
                    .context("Persistent worlds unavailable")?;
                let opened = catalog.open_world(id)?;
                Ok(XrPendingSessionStart {
                    scene: self.catalog_world_scene(
                        &opened.summary,
                        catalog.world_dir(&opened.summary.id),
                    ),
                    descriptor: Some(ActiveSessionDescriptor::from_local_world_summary(
                        &opened.summary,
                    )),
                })
            }
            SessionStartRequest::JoinRemote { .. } => Ok(XrPendingSessionStart {
                scene: self.remote_session_options(),
                descriptor: request.active_descriptor(),
            }),
            SessionStartRequest::Unknown => bail!("unsupported unknown XR session start"),
        }
    }

    fn start_pending_session(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let Some(pending) = self.session.take_pending_start() else {
            return Ok(false);
        };
        let request = pending.request.clone();
        match self.start_pending_session_payload(device, queue, pending) {
            Ok(XrSessionStartOutcome::LocalStartupQueued) => Ok(false),
            Ok(XrSessionStartOutcome::Started(descriptor)) => {
                self.session.complete_start(descriptor.clone());
                self.status_overlay = StatusOverlay::hidden();
                self.apply_started_session_ui(&descriptor);
                Ok(true)
            }
            Err(error) => {
                log::error!("failed to start XR session {request:?}: {error:#}");
                self.session
                    .fail_start(SessionFailure::new(request.default_failure_message()));
                self.apply_xr_session_ui_effects(client_session_failed_start_ui_effects(
                    &request, false,
                ));
                self.menu_panel_anchor = XrUiPanelAnchor::Head;
                self.menu_panel_recenter_pending = true;
                Err(error)
            }
        }
    }

    fn start_pending_session_payload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pending: mclone_app_runtime::session::PendingSessionStart<XrPendingSessionStart>,
    ) -> Result<XrSessionStartOutcome> {
        let request = pending.request;
        let scene = pending.payload.scene;
        let descriptor = pending
            .payload
            .descriptor
            .or_else(|| request.active_descriptor());
        match request {
            request @ (SessionStartRequest::CreateLocalWorld { .. }
            | SessionStartRequest::OpenLocalWorld { .. }) => {
                self.begin_local_session_start(request, descriptor, scene)?;
                Ok(XrSessionStartOutcome::LocalStartupQueued)
            }
            request @ SessionStartRequest::JoinRemote { .. } => {
                let descriptor = self.start_replacement_session(device, queue, request, scene)?;
                Ok(XrSessionStartOutcome::Started(descriptor))
            }
            SessionStartRequest::Unknown => bail!("unsupported unknown XR session start"),
        }
    }

    fn begin_local_session_start(
        &mut self,
        request: SessionStartRequest,
        descriptor: Option<ActiveSessionDescriptor>,
        scene: XrSceneOptions,
    ) -> Result<()> {
        let scene = scene.validated()?;
        let mesh_assets = self.mesh_assets.clone();
        let pump = LocalSingleViewStartupPump::with_mesh_assets(
            local_single_view_options(&scene),
            mesh_assets,
        )
        .context("create XR local world startup pump")?;
        let mut camera = EngineCameraController::spawn_for_chunk(scene.center());
        camera.set_movement_speed_multiplier(f64::from(scene.movement_speed_multiplier));
        self.local_startup = Some(XrLocalStartup {
            request: request.clone(),
            descriptor,
            scene: scene.clone(),
            pump,
            camera,
            startup_view_pose: None,
        });
        self.status_overlay = StatusOverlay::hidden();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = true;
        log::info!(
            "XR local world startup queued seed={} center=({}, {}) render_distance={}",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.render_distance
        );
        Ok(())
    }

    fn advance_local_startup(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if self.local_startup.is_none() {
            return Ok(false);
        }

        let step = {
            let startup = self
                .local_startup
                .as_mut()
                .expect("startup presence checked");
            let camera_position = glam_vec3_from_vec3d(startup.camera.snapshot().eye);
            startup
                .pump
                .step(camera_position)
                .context("advance XR local world startup pump")
        };
        let step = match step {
            Ok(step) => step,
            Err(error) => {
                let startup = self
                    .local_startup
                    .take()
                    .expect("startup must exist after failed pump step");
                self.fail_local_startup(startup, error);
                return Ok(false);
            }
        };

        if !step.playable_ready {
            return Ok(false);
        }

        let startup = self
            .local_startup
            .take()
            .expect("startup must exist after playable step");
        let failed_request = startup.request.clone();
        match self.complete_local_startup(device, queue, startup, step) {
            Ok(()) => Ok(true),
            Err(error) => {
                log::error!("failed to complete XR local world startup: {error:#}");
                self.session.fail_start(SessionFailure::new(
                    failed_request.default_failure_message(),
                ));
                self.apply_xr_session_ui_effects(client_session_failed_start_ui_effects(
                    &failed_request,
                    false,
                ));
                self.menu_panel_anchor = XrUiPanelAnchor::Head;
                self.menu_panel_recenter_pending = true;
                Ok(false)
            }
        }
    }

    fn complete_local_startup(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        startup: XrLocalStartup,
        step: LocalSingleViewStartupStep,
    ) -> Result<()> {
        let XrLocalStartup {
            request,
            descriptor,
            scene,
            pump,
            mut camera,
            startup_view_pose,
        } = startup;
        let descriptor = descriptor
            .or_else(|| request.active_descriptor())
            .context("XR local startup request did not describe an active session")?;
        let mut runtime = NativeSingleViewSessionRuntime::<S>::from_active_runtime_with_descriptor(
            descriptor.clone(),
            NativeSingleViewSceneRuntime::Local(pump.into_runtime()),
        );
        let mut initial_pose_changed =
            apply_pending_engine_camera_position_updates_for_runtime(&mut runtime, &mut camera)
                .context("accept XR local startup player pose")?;
        if let Some(view_pose) = startup_view_pose {
            apply_xr_startup_view_pose(&mut camera, view_pose.position, view_pose.yaw_degrees)
                .context("apply XR local startup view pose")?;
            initial_pose_changed |= commit_engine_camera_player_pose_for_runtime(
                &mut runtime,
                &mut camera,
                "sync XR local startup view pose",
            )?;
        } else {
            initial_pose_changed |= commit_engine_camera_player_pose_for_runtime(
                &mut runtime,
                &mut camera,
                "sync XR local startup player pose",
            )?;
        }
        if initial_pose_changed {
            log::info!("XR local startup applied initial player pose correction");
        }

        let camera_position = glam_vec3_from_vec3d(camera.snapshot().eye);
        let sections = runtime.cached_sections();
        if sections.is_empty() {
            bail!(
                "XR local startup seed={} center=({}, {}) render_distance={} reached playable threshold without render sections",
                scene.seed,
                scene.chunk_x,
                scene.chunk_z,
                scene.render_distance
            );
        }
        let mut draw = TexturedSectionDrawResources::new(
            device,
            queue,
            self.color_format,
            &sections,
            runtime.mesh_assets().atlas.as_upload(),
        )
        .context("upload XR local startup render sections")?;
        draw.set_traversal_ready_sections(
            &runtime.traversal_ready_render_section_keys(camera_position),
        );

        let section_count = draw.section_count();
        let index_count = draw.index_count();
        let face_count = quad_face_count_from_indices(index_count);
        self.scene = scene;
        self.mesh_assets = runtime.mesh_assets().clone();
        self.runtime = Some(runtime);
        self.camera = camera;
        self.draw = draw;
        self.traversal_ready_sections.clear();
        self.render_stats = RenderStreamStats {
            section_count,
            index_count,
            face_count,
            ..RenderStreamStats::default()
        };
        self.sync_player_appearance()
            .context("sync XR local startup player appearance")?;
        self.clear_transient_world_state();
        self.session.complete_start(descriptor.clone());
        self.status_overlay = StatusOverlay::hidden();
        self.apply_started_session_ui(&descriptor);
        match descriptor {
            ActiveSessionDescriptor::LocalWorld { seed, .. } => {
                log::info!(
                    "XR local world playable seed={} polls={} poll_ms={:.3} sections={} target_ready={}/{}",
                    seed,
                    step.poll_count,
                    step.poll_ms,
                    section_count,
                    step.progress
                        .as_ref()
                        .map_or(0, |progress| progress.target_ready_chunks),
                    step.progress
                        .as_ref()
                        .map_or(0, |progress| progress.target_chunk_count)
                );
            }
            ActiveSessionDescriptor::Remote { .. } => {}
        }
        self.apply_debug_ui_screen();
        if !self.ui.is_active() {
            self.clear_menu_input_state();
        }
        Ok(())
    }

    fn apply_debug_ui_screen(&mut self) {
        let Some(screen) = self.scene.debug_ui_screen else {
            return;
        };
        let desired_screen = match screen {
            XrDebugUiScreen::Pause => GameScreen::Pause,
            XrDebugUiScreen::Controls => GameScreen::Help {
                parent: mclone_ui::GameHelpParent::OptionsPause,
            },
        };
        if self.ui.screen() != Some(desired_screen) {
            self.ui.set_screen(Some(desired_screen));
            self.ui.clear_input();
            self.menu_pointer_down = false;
            self.menu_panel_anchor = XrUiPanelAnchor::Head;
            self.menu_panel_recenter_pending = true;
        }
        if self.menu_panel_pose.is_none() {
            self.menu_panel_anchor = XrUiPanelAnchor::Head;
            self.menu_panel_recenter_pending = true;
        }
    }

    fn fail_local_startup(&mut self, startup: XrLocalStartup, error: anyhow::Error) {
        log::error!(
            "failed to start XR local world {:?}: {error:#}",
            startup.request
        );
        self.session.fail_start(SessionFailure::new(
            startup.request.default_failure_message(),
        ));
        self.apply_xr_session_ui_effects(client_session_failed_start_ui_effects(
            &startup.request,
            false,
        ));
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = true;
    }

    fn clear_menu_input_state(&mut self) {
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
    }

    fn clear_transient_world_state(&mut self) {
        self.tracking_origin = None;
        self.last_locomotion_update = None;
        self.underwater_effects = XrUnderwaterEffectStates::default();
        self.last_underwater_update = None;
        self.head_comfort.reset();
        self.clear_xr_blink_teleport();
        self.latest_controllers.clear();
        self.first_eye_summary = None;
        self.last_ui_panel_stats = WorldGuiPanelRenderStats::default();
        self.last_ui_draw_cache_stats = UiDrawCacheStats::default();
        self.rendered_frames = 0;
        self.prefetched_live_upload = None;
        self.traversal_ready_sections.clear();
        self.section_uploads.clear();
    }

    fn teardown_world(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
        self.local_startup = None;
        if self.runtime.take().is_some() {
            self.draw = TexturedSectionDrawResources::new(
                device,
                queue,
                self.color_format,
                &[],
                self.mesh_assets.atlas.as_upload(),
            )
            .context("reset XR terrain draw resources during session teardown")?;
        }
        self.render_stats = RenderStreamStats::default();
        self.clear_transient_world_state();
        Ok(())
    }

    fn start_replacement_session(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: SessionStartRequest,
        scene: XrSceneOptions,
    ) -> Result<ActiveSessionDescriptor> {
        let descriptor = request.active_descriptor().with_context(|| {
            format!("XR session start request did not describe an active session: {request:?}")
        })?;
        let mesh_assets = self.mesh_assets.clone();
        let Some(factory) = self.session_runtime_factory.as_mut() else {
            let error = anyhow!("XR session runtime factory is not installed");
            log::error!("{error:#}");
            return Err(error);
        };
        let scene = scene.validated()?;
        let runtime = match factory(request.clone(), scene.clone(), mesh_assets) {
            Ok(runtime) => runtime,
            Err(error) => {
                log::error!("failed to start XR session {request:?}: {error:#}");
                return Err(error);
            }
        };
        let movement_speed_multiplier = scene.movement_speed_multiplier;
        let started = match start_xr_terrain_runtime(
            device,
            queue,
            self.color_format,
            runtime,
            movement_speed_multiplier,
            None,
        ) {
            Ok(started) => started,
            Err(error) => {
                log::error!("failed to warm XR session {request:?}: {error:#}");
                return Err(error);
            }
        };

        self.scene = scene;
        self.mesh_assets = started.runtime.mesh_assets().clone();
        self.runtime = Some(started.runtime);
        self.camera = started.camera;
        self.draw = started.draw;
        self.render_stats = started.render_stats;
        self.sync_player_appearance()
            .context("sync XR session start player appearance")?;
        self.clear_transient_world_state();
        self.clear_menu_input_state();
        match &descriptor {
            ActiveSessionDescriptor::LocalWorld { seed, .. } => {
                log::info!("XR created local world seed={seed}");
            }
            ActiveSessionDescriptor::Remote { endpoint } => {
                log::info!("XR joined remote session {}", endpoint.address);
            }
        }
        Ok(descriptor)
    }

    fn apply_xr_ui_action(
        &mut self,
        action: GameUiAction,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if self.local_startup.is_some() {
            return Ok(false);
        }
        let settings_state = self.client_experience_settings_state();
        self.client_experience.set_settings_state(settings_state);

        let active_remote_addr = self.active_remote_addr();
        let current_join_remote_addr = normalized_xr_remote_addr(self.ui.join_remote_addr());
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
                current_join_remote_addr: &current_join_remote_addr,
                fallback_remote_addr: active_remote_addr.as_deref(),
            },
        );
        let apply_ui_action =
            client_experience_should_apply_ui_projection(action) && effects.projection.is_empty();
        let scene_replaced = self.apply_client_experience_effects(effects, device, queue)?;
        if apply_ui_action {
            self.apply_xr_projection_action(action);
        }
        if !self.ui.is_active() {
            self.clear_menu_input_state();
        }
        Ok(scene_replaced)
    }

    fn apply_client_experience_effects(
        &mut self,
        effects: ClientExperienceEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let catalog_scene_replaced =
            self.apply_xr_catalog_effects(effects.catalog, device, queue)?;
        let session_scene_replaced =
            self.apply_xr_session_effects(effects.session, device, queue)?;
        if !self.apply_xr_settings_effects(effects.settings)? {
            return Ok(catalog_scene_replaced || session_scene_replaced);
        }
        for effect in effects.gameplay {
            match effect {
                ClientExperienceGameplayEffect::AssignHotbarBlock { slot, block_state } => {
                    let changed = self.assign_debug_hotbar_slot(slot, BlockStateId(block_state))?;
                    log::info!(
                        "XR debug hotbar slot {} assigned block_state={} changed={}",
                        slot + 1,
                        block_state,
                        changed
                    );
                }
            }
        }
        for effect in effects.projection {
            match effect {
                ClientExperienceProjectionEffect::ApplyUiAction(action) => {
                    self.apply_xr_projection_action(action);
                }
            }
        }
        Ok(catalog_scene_replaced || session_scene_replaced)
    }

    fn apply_xr_catalog_effects(
        &mut self,
        effects: ClientCatalogEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let mut scene_replaced = false;
        for start in effects.session_starts {
            let Some(catalog) = self.world_catalog.clone() else {
                let error = WorldCatalogError::unsupported("Persistent worlds unavailable");
                log::warn!("XR catalog session start failed: {error}");
                continue;
            };
            let scene =
                self.catalog_world_scene(&start.summary, catalog.world_dir(&start.summary.id));
            self.status_overlay = StatusOverlay::hidden();
            self.session.request_start(
                start.request,
                XrPendingSessionStart {
                    scene,
                    descriptor: Some(start.descriptor),
                },
            );
            self.ui.clear_input();
            self.menu_pointer_down = false;
            scene_replaced |= self.start_pending_session(device, queue)?;
        }
        for request in effects.catalog_requests {
            scene_replaced |= self.execute_xr_catalog_request(request, device, queue)?;
        }
        Ok(scene_replaced)
    }

    fn execute_xr_catalog_request(
        &mut self,
        request: ClientCatalogRequest,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        let Some(catalog) = self.world_catalog.clone() else {
            let error = WorldCatalogError::unsupported("Persistent worlds unavailable");
            log::warn!("XR world catalog action failed: {error}");
            let effects = self
                .client_experience
                .catalog_mut()
                .apply_catalog_error(request.id, error);
            return self.apply_xr_catalog_effects(effects, device, queue);
        };
        let active_world = self.active_local_world_id().cloned();
        let response = match catalog.handle_request(request.request, active_world.as_ref()) {
            Ok(response) => response,
            Err(error) => {
                log::warn!("XR world catalog action failed: {error}");
                let effects = self
                    .client_experience
                    .catalog_mut()
                    .apply_catalog_error(request.id, error);
                return self.apply_xr_catalog_effects(effects, device, queue);
            }
        };
        let effects = self
            .client_experience
            .catalog_mut()
            .apply_catalog_response(request.id, response);
        self.apply_xr_catalog_effects(effects, device, queue)
    }

    fn apply_xr_session_effects(
        &mut self,
        effects: ClientSessionEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<bool> {
        if let Some(seed) = effects.new_world_seed {
            self.ui.set_new_world_seed(seed);
        }
        if let Some(addr) = effects.join_remote_addr {
            self.ui
                .set_join_remote_addr(normalized_xr_remote_addr(&addr));
        }
        if effects.clear_inactive_session_status {
            self.clear_inactive_session_status();
        }

        let mut scene_replaced = false;
        if let Some(request) = effects.session_start {
            scene_replaced = self.start_session_for_request(device, queue, request)?;
        }
        if let Some(host_action) = effects.host_action {
            match host_action {
                ClientSessionHostAction::QuitToTitle => {
                    let transition = client_session_quit_to_title_transition(self.session.state());
                    self.apply_xr_session_transition_effects(transition, device, queue)?;
                }
                ClientSessionHostAction::Quit => {
                    log::info!("XR menu quit action ignored by shared scene");
                }
            }
        }
        Ok(scene_replaced)
    }

    fn apply_xr_session_transition_effects(
        &mut self,
        effects: ClientSessionTransitionEffects,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        if effects.teardown_world {
            self.teardown_world(device, queue)?;
        }
        if effects.clear_session {
            self.session.clear();
        }
        self.status_overlay = StatusOverlay::hidden();
        self.apply_xr_session_ui_effects(effects.ui);
        Ok(())
    }

    fn apply_xr_settings_effects(
        &mut self,
        effects: ClientExperienceSettingsEffects,
    ) -> Result<bool> {
        let has_setting_effects = !effects.setting_effects.is_empty();
        let has_unavailable =
            !effects.capability_projection.is_empty() || !effects.rejections.is_empty();
        for effect in effects.setting_effects {
            match effect {
                ClientExperienceSettingEffect::SetSectionOcclusionCulling(enabled) => {
                    self.render_options.section_occlusion_culling = enabled;
                    log::info!(
                        "XR section occlusion culling {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                }
                ClientExperienceSettingEffect::SetFullbright(enabled) => {
                    self.render_options.force_fullbright = enabled;
                    log::info!(
                        "XR fullbright {}",
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
                        "XR far LOD {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                    log::info!(
                        "XR far LOD range set to {} chunks beyond render distance",
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
                        "XR player collision box debug {}",
                        if visible { "visible" } else { "hidden" }
                    );
                }
                ClientExperienceSettingEffect::SetFirstPersonPlayerVisible(visible) => {
                    self.camera.set_first_person_player_visible(visible);
                    log::info!(
                        "XR first-person player body {}",
                        if visible { "visible" } else { "hidden" }
                    );
                }
                ClientExperienceSettingEffect::SetCrosshairVisible(_) => {}
                ClientExperienceSettingEffect::SetFramePipelineOverlayVisible(visible) => {
                    self.diagnostic_panel.set_frame_metrics_visible(visible);
                    log::info!(
                        "XR frame pipeline overlay {}",
                        if visible { "visible" } else { "hidden" }
                    );
                }
                ClientExperienceSettingEffect::SetDebugDiagnosticsVisible(visible) => {
                    self.diagnostic_panel.set_debug_diagnostics_visible(visible);
                    log::info!(
                        "XR debug diagnostics {}",
                        if visible { "visible" } else { "hidden" }
                    );
                }
                ClientExperienceSettingEffect::SetPlayerModel(model) => {
                    self.player_model = model;
                    log::info!("XR player model set to {}", model.label());
                }
                ClientExperienceSettingEffect::SyncPlayerAppearance => {
                    if let Err(error) = self.sync_player_appearance() {
                        log::warn!("failed to sync XR player appearance: {error:#}");
                    }
                }
                ClientExperienceSettingEffect::SetMovementMode(movement_mode) => {
                    self.camera
                        .set_movement_mode(engine_movement_mode(movement_mode));
                    let movement_mode = self.camera.movement_mode();
                    log::info!("XR player movement mode {}", movement_mode.label());
                }
                ClientExperienceSettingEffect::SetCollisionMode(collision_mode) => {
                    self.camera
                        .set_collision_mode(engine_collision_mode(collision_mode));
                    let collision_mode = self.camera.collision_mode();
                    log::info!("XR player collision mode {}", collision_mode.label());
                }
                ClientExperienceSettingEffect::SetTravelAssistMode(travel_assist_mode) => {
                    self.travel_assist_mode = travel_assist_mode;
                    if self.travel_assist_mode != GameTravelAssistMode::Blink {
                        self.clear_xr_blink_teleport();
                    }
                    log::info!("XR travel assist {}", travel_assist_mode.label());
                }
                ClientExperienceSettingEffect::SetTurnMode(turn_mode) => {
                    let turn_mode = GameXrTurnMode::from(turn_mode);
                    self.set_turn_policy(XrTurnPolicy::from_game_mode(turn_mode));
                    log::info!("XR turn mode {}", turn_mode.label());
                }
                ClientExperienceSettingEffect::SetXrTurnMode(turn_mode) => {
                    self.set_turn_policy(XrTurnPolicy::from_game_mode(turn_mode));
                    log::info!("XR turn mode {}", turn_mode.label());
                }
                ClientExperienceSettingEffect::CycleFramePacing => {
                    log::info!("XR frame pacing cycle ignored by scene host");
                }
                ClientExperienceSettingEffect::CycleFpsCap => {
                    log::info!("XR FPS cap cycle ignored by scene host");
                }
                ClientExperienceSettingEffect::SetRenderDistance(render_distance) => {
                    if let Some(runtime) = &mut self.runtime {
                        if runtime
                            .set_render_distance(render_distance)
                            .context("set XR render distance from menu")?
                        {
                            log::info!(
                                "XR render distance set to {} (chunk tracking radius {})",
                                render_distance,
                                runtime.chunk_tracking_radius()
                            );
                        }
                    }
                    self.scene.render_distance = render_distance;
                }
                ClientExperienceSettingEffect::SetFlySpeedMultiplier(multiplier) => {
                    self.camera.set_fly_speed_multiplier(f64::from(multiplier));
                    log::info!(
                        "XR fly speed set to {:.1}x ({:.0} blocks/s)",
                        self.camera.fly_speed_multiplier(),
                        self.camera.speed_blocks_per_second()
                    );
                }
                ClientExperienceSettingEffect::SetMovementSpeedMultiplier(multiplier) => {
                    self.camera
                        .set_movement_speed_multiplier(f64::from(multiplier));
                    self.scene.movement_speed_multiplier =
                        self.camera.movement_speed_multiplier() as f32;
                    log::info!(
                        "XR movement speed multiplier set to {:.1}x",
                        self.camera.movement_speed_multiplier()
                    );
                }
                ClientExperienceSettingEffect::SetTouchLookSensitivity(_) => {}
                ClientExperienceSettingEffect::SetTouchControlsMode(_) => {}
                ClientExperienceSettingEffect::SetServerSimulationCadence(_) => {}
            }
        }
        self.apply_client_experience_capability_projection(effects.capability_projection);
        for rejection in effects.rejections {
            log::error!("{}", rejection.message);
            self.status_overlay = StatusOverlay::new(rejection.message, false);
            return Ok(false);
        }
        if has_setting_effects && !has_unavailable {
            self.status_overlay = StatusOverlay::hidden();
        }
        Ok(true)
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

    fn apply_xr_session_ui_effects(&mut self, effects: ClientSessionUiEffects) {
        if let Some(seed) = effects.new_world_seed {
            self.ui.set_new_world_seed(seed);
        }
        if let Some(addr) = effects.join_remote_addr {
            self.ui
                .set_join_remote_addr(normalized_xr_remote_addr(&addr));
        }
        if let Some(screen) = effects.screen {
            self.ui.set_screen(Some(screen));
        }
    }

    fn apply_started_session_ui(&mut self, descriptor: &ActiveSessionDescriptor) {
        match descriptor {
            ActiveSessionDescriptor::LocalWorld { seed, id, .. } => {
                self.ui.set_new_world_seed(*seed);
                if id.is_some() {
                    self.ui.close();
                } else {
                    self.ui.apply_action(GameUiAction::CreateWorld(*seed));
                }
            }
            ActiveSessionDescriptor::Remote { endpoint } => {
                self.ui.set_join_remote_addr(endpoint.address.clone());
                self.ui.apply_action(GameUiAction::JoinRemote);
            }
        }
    }

    fn apply_xr_projection_action(&mut self, action: GameUiAction) {
        if matches!(action, GameUiAction::OpenBlockPalette) {
            self.menu_panel_anchor = XrUiPanelAnchor::LeftHand;
            self.menu_panel_pose = None;
            self.menu_panel_recenter_pending = true;
        }
        self.ui.apply_action(action);
    }

    fn camera_inside_occluding_block(&self, position: Vec3) -> bool {
        let Some(runtime) = &self.runtime else {
            return false;
        };
        let Some(state_id) = runtime.block_state_at_position(position) else {
            return false;
        };
        runtime.mesh_assets().catalog.occludes(state_id)
    }

    fn camera_inside_water(&self, position: Vec3) -> bool {
        self.runtime
            .as_ref()
            .is_some_and(|runtime| runtime.camera_inside_water(position))
    }

    fn record_eye0_summary(&mut self, summary: FullFrameRenderSummary) {
        self.first_eye_summary = Some(summary);
        self.rendered_frames += 1;
    }
}

fn xr_poll_diagnostics_upload_summary(
    host_mode: XrTerrainHostMode,
    diagnostics: RuntimePollDiagnostics,
) -> XrTerrainUploadSummary {
    XrTerrainUploadSummary {
        host_mode,
        poll_total_ms: diagnostics.poll_total_ms,
        poll_drain_updates_ms: diagnostics.drain_updates_ms,
        poll_producer_read_ms: diagnostics.producer_read_ms,
        poll_producer_decode_ms: diagnostics.producer_decode_ms,
        poll_producer_response_sequence: diagnostics.producer_response_sequence,
        poll_client_deferred_chunk_drop_ms: diagnostics.client_deferred_chunk_drop_ms,
        poll_client_deferred_chunk_drop_items: diagnostics.client_deferred_chunk_drop_items,
        poll_client_deferred_chunk_drop_backlog_items: diagnostics
            .client_deferred_chunk_drop_backlog_items,
        poll_apply_updates_ms: diagnostics.apply_updates_ms,
        poll_dirty_mark_ms: diagnostics.dirty_mark_ms,
        poll_client_apply_updates_ms: diagnostics.client_apply_updates_ms,
        poll_snapshot_update_apply_ms: diagnostics.snapshot_update_apply_ms,
        poll_snapshot_update_dirty_mark_ms: diagnostics.snapshot_update_dirty_mark_ms,
        poll_snapshot_update_client_apply_ms: diagnostics.snapshot_update_client_apply_ms,
        poll_section_block_update_apply_ms: diagnostics.section_block_update_apply_ms,
        poll_section_block_update_dirty_mark_ms: diagnostics.section_block_update_dirty_mark_ms,
        poll_section_block_update_client_apply_ms: diagnostics.section_block_update_client_apply_ms,
        poll_unload_update_apply_ms: diagnostics.unload_update_apply_ms,
        poll_unload_update_dirty_mark_ms: diagnostics.unload_update_dirty_mark_ms,
        poll_unload_update_client_apply_ms: diagnostics.unload_update_client_apply_ms,
        poll_other_update_apply_ms: diagnostics.other_update_apply_ms,
        poll_other_update_dirty_mark_ms: diagnostics.other_update_dirty_mark_ms,
        poll_other_update_client_apply_ms: diagnostics.other_update_client_apply_ms,
        poll_mixed_update_apply_ms: diagnostics.mixed_update_apply_ms,
        poll_mixed_update_dirty_mark_ms: diagnostics.mixed_update_dirty_mark_ms,
        poll_mixed_update_client_apply_ms: diagnostics.mixed_update_client_apply_ms,
        update_pump_stalled: diagnostics.update_pump_stalled,
        update_pump_stall_count: diagnostics.update_pump_stall_count,
        server_update_applied_bytes: diagnostics.server_update_applied_bytes,
        server_update_oldest_applied_age_ms: diagnostics.server_update_oldest_applied_age_ms,
        poll_diagnostics_ms: diagnostics.poll_diagnostics_ms,
        poll_diagnostics_refreshed: diagnostics.diagnostics_refreshed,
        poll_diagnostics_cache_age_ms: diagnostics.diagnostics_cache_age_ms,
        server_diagnostics_detail_refreshes: diagnostics.server_diagnostics_detail_refreshes,
        server_diagnostics_detail_age_ms: diagnostics.server_diagnostics_detail_age_ms,
        poll_server_tick_ms: diagnostics.server_tick_ms,
        poll_server_reported_total_ms: diagnostics.server_reported_total_ms,
        poll_scheduler_tick_ms: diagnostics.scheduler_tick_ms,
        poll_scheduler_completed_feature_jobs_drained: diagnostics
            .scheduler_completed_feature_jobs_drained,
        poll_scheduler_feature_chunks_published: diagnostics.scheduler_feature_chunks_published,
        poll_scheduler_feature_chunks_skipped: diagnostics.scheduler_feature_chunks_skipped,
        poll_scheduler_feature_jobs_completed: diagnostics.scheduler_feature_jobs_completed,
        poll_scheduler_feature_snapshot_ready_events: diagnostics
            .scheduler_feature_snapshot_ready_events,
        poll_scheduler_light_status_batches_enqueued: diagnostics
            .scheduler_light_status_batches_enqueued,
        poll_scheduler_completed_light_statuses_drained: diagnostics
            .scheduler_completed_light_statuses_drained,
        poll_scheduler_light_statuses_published: diagnostics.scheduler_light_statuses_published,
        poll_scheduler_light_statuses_skipped: diagnostics.scheduler_light_statuses_skipped,
        poll_scheduler_light_snapshot_ready_events: diagnostics
            .scheduler_light_snapshot_ready_events,
        poll_scheduler_pending_worldgen_publication_jobs: diagnostics
            .scheduler_pending_worldgen_publication_jobs,
        poll_scheduler_pending_worldgen_publication_chunks: diagnostics
            .scheduler_pending_worldgen_publication_chunks,
        poll_scheduler_pending_light_publications: diagnostics.scheduler_pending_light_publications,
        poll_scheduler_worldgen_mailbox_pending_jobs: diagnostics
            .scheduler_worldgen_mailbox_pending_jobs,
        poll_scheduler_light_mailbox_pending_statuses: diagnostics
            .scheduler_light_mailbox_pending_statuses,
        poll_updates: diagnostics.updates,
        poll_snapshot_updates: diagnostics.snapshot_updates,
        poll_section_block_updates: diagnostics.section_block_updates,
        poll_unload_updates: diagnostics.unload_updates,
        poll_other_updates: diagnostics.other_updates,
        poll_mixed_updates: diagnostics.mixed_updates,
        server_command_queue_depth: diagnostics.server_command_queue_depth,
        server_update_queue_depth: diagnostics.server_update_queue_depth,
        server_update_queue_bytes: diagnostics.server_update_queue_bytes,
        server_pending_jobs: diagnostics.server_pending_jobs,
        server_pending_publications: diagnostics.server_pending_publications,
        runner_frame_metrics: diagnostics.runner_frame_metrics,
        worldgen_job_frame_metrics: diagnostics.worldgen_job_frame_metrics,
        light_status_job_frame_metrics: diagnostics.light_status_job_frame_metrics,
        scheduler_pending_jobs: diagnostics.scheduler_pending_jobs,
        scheduler_completed_jobs: diagnostics.scheduler_completed_jobs,
        scheduler_dirty_chunks: diagnostics.scheduler_dirty_chunks,
        scheduler_loaded_snapshot_chunks: diagnostics.scheduler_loaded_snapshot_chunks,
        scheduler_client_visible_chunks: diagnostics.scheduler_client_visible_chunks,
        scheduler_active_ticket_chunks: diagnostics.scheduler_active_ticket_chunks,
        player_visible_chunks: diagnostics.player_visible_chunks,
        player_outbound_queue_depth: diagnostics.player_outbound_queue_depth,
        ..XrTerrainUploadSummary::default()
    }
}

fn accumulate_upload_report(
    total: &mut TexturedSectionUploadReport,
    report: TexturedSectionUploadReport,
) {
    total.uploaded_section_count += report.uploaded_section_count;
    total.removed_section_count += report.removed_section_count;
    total.uploaded_vertex_count += report.uploaded_vertex_count;
    total.uploaded_index_count += report.uploaded_index_count;
}

fn start_xr_terrain_runtime<S>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color_format: wgpu::TextureFormat,
    mut runtime: NativeSingleViewSessionRuntime<S>,
    movement_speed_multiplier: f32,
    startup_view_pose: Option<XrStartupViewPose>,
) -> Result<StartedXrTerrainRuntime<S>>
where
    S: RemoteDedicatedServerSession,
{
    let center = runtime.interest_center();
    let render_distance = runtime.render_distance();
    let host_label = runtime.host_label();
    let session_label = active_session_label(runtime.active_session());
    let mut camera = EngineCameraController::spawn_for_chunk(center);
    camera.set_movement_speed_multiplier(f64::from(movement_speed_multiplier));
    let initial_poll_start = Instant::now();
    let (initial_poll_count, initial_poll_ms) = runtime
        .poll_until_idle()
        .context("wait for initial XR terrain chunks")?;
    let mut initial_pose_changed =
        apply_pending_engine_camera_position_updates_for_runtime(&mut runtime, &mut camera)
            .context("accept initial XR terrain player pose")?;
    if let Some(view_pose) = startup_view_pose {
        apply_xr_startup_view_pose(&mut camera, view_pose.position, view_pose.yaw_degrees)
            .context("apply XR terrain startup view pose")?;
        initial_pose_changed |= commit_engine_camera_player_pose_for_runtime(
            &mut runtime,
            &mut camera,
            "sync XR terrain startup view pose",
        )?;
    } else {
        initial_pose_changed |= commit_engine_camera_player_pose_for_runtime(
            &mut runtime,
            &mut camera,
            "sync initial XR terrain player pose",
        )?;
    }
    if initial_pose_changed {
        let _ = runtime
            .poll_until_idle()
            .context("wait for XR terrain chunks after player pose")?;
    }

    let camera_position = glam_vec3_from_vec3d(camera.snapshot().eye);
    let section_update = runtime
        .sync_all_render_sections(camera_position)
        .context("compile initial XR terrain render sections")?;
    let sections = runtime.cached_sections();
    if sections.is_empty() {
        bail!(
            "XR terrain host={} center=({}, {}) render_distance={} produced no render sections",
            host_label,
            center.x,
            center.z,
            render_distance
        );
    }
    let mut draw = TexturedSectionDrawResources::new(
        device,
        queue,
        color_format,
        &sections,
        runtime.mesh_assets().atlas.as_upload(),
    )
    .context("upload initial XR terrain render sections")?;
    draw.set_traversal_ready_sections(
        &runtime.traversal_ready_render_section_keys(camera_position),
    );

    let initial_upload = TexturedSectionUploadReport {
        uploaded_section_count: section_update.rebuilt_section_count(),
        removed_section_count: section_update.removed_section_count(),
        uploaded_vertex_count: section_update.rebuilt_vertex_count,
        uploaded_index_count: section_update.rebuilt_index_count,
    };
    let mut render_stats = RenderStreamStats {
        section_count: draw.section_count(),
        index_count: draw.index_count(),
        face_count: quad_face_count_from_indices(draw.index_count()),
        ..RenderStreamStats::default()
    };
    record_render_section_update_stats(&mut render_stats, &section_update, initial_upload);
    log::info!(
        "mclone XR terrain runtime: host={} session={} center=({}, {}) render_distance={} chunks={} sections={} faces={} indices={} initial_polls={} poll_ms={:.3} elapsed_ms={:.3}",
        host_label,
        session_label,
        center.x,
        center.z,
        render_distance,
        runtime.loaded_chunk_count(),
        render_stats.section_count,
        render_stats.face_count,
        render_stats.index_count,
        initial_poll_count,
        initial_poll_ms,
        elapsed_ms(initial_poll_start.elapsed())
    );
    Ok(StartedXrTerrainRuntime {
        runtime,
        camera,
        draw,
        render_stats,
    })
}

fn commit_engine_camera_player_pose_for_runtime<S>(
    runtime: &mut NativeSingleViewSessionRuntime<S>,
    camera: &mut EngineCameraController,
    context: &'static str,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    commit_engine_camera_player_pose_for_runtime_timed(runtime, camera, context)
        .map(|(changed, _)| changed)
}

fn commit_engine_camera_player_pose_for_runtime_timed<S>(
    runtime: &mut NativeSingleViewSessionRuntime<S>,
    camera: &mut EngineCameraController,
    context: &'static str,
) -> Result<(bool, XrCameraCommitTiming)>
where
    S: RemoteDedicatedServerSession,
{
    let (server_changed, mut timing) =
        sync_engine_camera_player_pose_for_runtime_timed(runtime, camera).context(context)?;
    let interest_start = Instant::now();
    let (interest_changed, interest_command_timing) =
        update_interest_from_engine_camera_for_runtime_timed(runtime, camera)?;
    timing.interest_ms = elapsed_ms(interest_start.elapsed());
    timing.record_interest_command_timing(interest_command_timing);
    Ok((server_changed || interest_changed, timing))
}

fn sync_engine_camera_player_pose_for_runtime_timed<S>(
    runtime: &mut NativeSingleViewSessionRuntime<S>,
    camera: &mut EngineCameraController,
) -> Result<(bool, XrCameraCommitTiming)>
where
    S: RemoteDedicatedServerSession,
{
    let mut timing = XrCameraCommitTiming::default();
    let command_start = Instant::now();
    let changed = if let Some(report) = camera.next_pose_sync_command() {
        let (changed, command_timing) = runtime
            .send_gameplay_command_with_update_policy_timed(
                report.command,
                GameplayCommandUpdatePolicy::SendOnly,
            )
            .context("failed to sync XR terrain player pose to server")?;
        timing.server_command_send_ms = command_timing.send_ms;
        timing.server_command_drain_updates_ms = command_timing.drain_updates_ms;
        timing.server_command_apply_updates_ms = command_timing.apply_updates_ms;
        timing.server_command_apply_dirty_mark_ms = command_timing.apply_dirty_mark_ms;
        timing.server_command_apply_client_updates_ms = command_timing.apply_client_updates_ms;
        timing.server_command_updates = command_timing.updates;
        timing.server_command_snapshot_updates = command_timing.snapshot_updates;
        timing.server_command_section_block_updates = command_timing.section_block_updates;
        timing.server_command_unload_updates = command_timing.unload_updates;
        changed
    } else {
        false
    };
    timing.server_command_ms = elapsed_ms(command_start.elapsed());
    let position_updates_start = Instant::now();
    let position_updates_changed =
        apply_pending_engine_camera_position_updates_for_runtime(runtime, camera)?;
    timing.position_updates_ms = elapsed_ms(position_updates_start.elapsed());
    Ok((changed || position_updates_changed, timing))
}

fn apply_pending_engine_camera_position_updates_for_runtime<S>(
    runtime: &mut NativeSingleViewSessionRuntime<S>,
    camera: &mut EngineCameraController,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    let mut changed = false;
    for update in runtime.drain_player_position_updates() {
        let accepted = camera.accept_position_update(update);
        runtime
            .send_gameplay_command(accepted.accept_command)
            .context("failed to acknowledge XR terrain player position correction")?;
        let resync = camera.corrected_pose_sync_command();
        runtime
            .send_gameplay_command(resync.command)
            .context("failed to sync corrected XR terrain player pose")?;
        log::warn!(
            "accepted XR terrain server player position correction id={} feet=({:.2}, {:.2}, {:.2})",
            accepted.update.teleport_id,
            accepted.feet_position.x,
            accepted.feet_position.y,
            accepted.feet_position.z
        );
        changed = true;
    }
    if changed {
        changed |= update_interest_from_engine_camera_for_runtime(runtime, camera)?;
    }
    Ok(changed)
}

fn update_interest_from_engine_camera_for_runtime<S>(
    runtime: &mut NativeSingleViewSessionRuntime<S>,
    camera: &EngineCameraController,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    update_interest_from_engine_camera_for_runtime_timed(runtime, camera)
        .map(|(changed, _)| changed)
}

fn update_interest_from_engine_camera_for_runtime_timed<S>(
    runtime: &mut NativeSingleViewSessionRuntime<S>,
    camera: &EngineCameraController,
) -> Result<(bool, GameplayCommandTiming)>
where
    S: RemoteDedicatedServerSession,
{
    let snapshot = camera.snapshot();
    let center = snapshot.chunk_pos;
    let (changed, timing) = runtime.set_interest_center_with_update_policy_timed(
        center,
        GameplayCommandUpdatePolicy::SendOnly,
    )?;
    if changed {
        log::info!(
            "XR terrain chunk interest moved to ({}, {}) at camera position ({:.1}, {:.1}, {:.1})",
            center.x,
            center.z,
            snapshot.eye.x,
            snapshot.eye.y,
            snapshot.eye.z
        );
    }
    Ok((changed, timing))
}

fn xr_game_ui_for_session(session: Option<&ActiveSessionDescriptor>, seed: i64) -> GameUiHost {
    let mut ui = GameUiHost::new();
    ui.set_new_world_seed(match session {
        Some(ActiveSessionDescriptor::LocalWorld { seed, .. }) => *seed,
        Some(ActiveSessionDescriptor::Remote { .. }) | None => seed,
    });
    ui.set_join_remote_addr(match session {
        Some(ActiveSessionDescriptor::Remote { endpoint }) => endpoint.address.clone(),
        Some(ActiveSessionDescriptor::LocalWorld { .. }) | None => {
            DEFAULT_JOIN_REMOTE_ADDR.to_owned()
        }
    });
    ui
}

fn normalized_xr_remote_addr(addr: &str) -> String {
    let addr = addr.trim();
    if addr.is_empty() {
        DEFAULT_JOIN_REMOTE_ADDR.to_owned()
    } else {
        addr.to_owned()
    }
}

fn xr_client_experience_profile() -> ClientExperienceProfile {
    let mut settings = ClientExperienceSettingsProfile::all_supported();
    settings.crosshair = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.frame_pacing = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.fps_cap = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.touch_look = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.touch_controls = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    settings.server_simulation_cadence = ClientExperienceCapabilityStatus::PROFILE_UNSUPPORTED;
    ClientExperienceProfile::new(settings)
}

fn initial_xr_seed_reroll_state(seed: i64) -> u64 {
    (seed as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0xD1B5_4A32_D192_ED03)
        .max(1)
}

fn local_single_view_options(scene: &XrSceneOptions) -> LocalSingleViewSceneOptions {
    let mut options =
        LocalSingleViewSceneOptions::new(scene.seed, scene.center(), scene.render_distance)
            .with_initial_spawn_center()
            .with_day_time(scene.day_time_override)
            .with_freeze_time(scene.freeze_time)
            .with_debug_passive_showcase(scene.debug_passive_showcase)
            .with_lighting_enabled(scene.lighting_enabled)
            .with_render_compile_worker_count(scene.render_compile_worker_count);
    if let Some(world_dir) = &scene.world_dir {
        options = options.with_persistent_world_dir(world_dir.clone());
    }
    options
}

fn active_session_label(session: Option<&ActiveSessionDescriptor>) -> String {
    match session {
        Some(ActiveSessionDescriptor::LocalWorld { seed, .. }) => {
            format!("local-world:{seed}")
        }
        Some(ActiveSessionDescriptor::Remote { endpoint }) => {
            format!("remote:{}", endpoint.address)
        }
        None => "none".to_owned(),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct XrTrackingOrigin {
    origin_stage: Vec3,
    stage_yaw: f32,
    mode: XrViewAlignmentMode,
}

#[derive(Clone, Copy, Debug)]
pub struct XrStageToWorld {
    origin_stage: Vec3,
    origin_world: Vec3,
    stage_to_world_rotation: Quat,
}

impl XrTrackingOrigin {
    pub fn from_initial_views(views: &[xr::View], mode: XrViewAlignmentMode) -> Result<Self> {
        let left_stage_pose = mclone_xr_host::view_pose(&views[0])?;
        let right_stage_pose = mclone_xr_host::view_pose(&views[1])?;
        let origin_stage = (left_stage_pose.position + right_stage_pose.position) * 0.5;
        Self::from_stage_view(origin_stage, left_stage_pose.orientation, mode)
    }

    pub fn from_stage_view(
        origin_stage: Vec3,
        left_stage_orientation: Quat,
        mode: XrViewAlignmentMode,
    ) -> Result<Self> {
        let stage_forward = left_stage_orientation * Vec3::NEG_Z;
        let stage_yaw = yaw_from_forward(stage_forward)
            .ok_or_else(|| anyhow!("OpenXR returned an invalid tracking-origin yaw"))?;
        Ok(Self {
            origin_stage,
            stage_yaw,
            mode,
        })
    }

    pub fn mode_label(self) -> &'static str {
        self.mode.label()
    }

    pub fn origin_stage(self) -> Vec3 {
        self.origin_stage
    }

    pub fn stage_yaw(self) -> f32 {
        self.stage_yaw
    }

    pub fn consume_world_movement(self, world_movement: Vec3, transform: XrStageToWorld) -> Self {
        if !world_movement.is_finite() || world_movement.length_squared() <= f32::EPSILON {
            return self;
        }
        let stage_movement = transform.stage_direction_from_world(world_movement);
        if !stage_movement.is_finite() {
            return self;
        }
        Self {
            origin_stage: self.origin_stage + stage_movement,
            ..self
        }
    }

    pub fn rebase_for_stage_position_world_position(
        self,
        stage_position: Vec3,
        world_position: Vec3,
        snapshot: EngineCameraSnapshot,
    ) -> Result<Self> {
        if !stage_position.is_finite() || !world_position.is_finite() {
            bail!("invalid XR tracking-origin rebase position");
        }
        let transform = XrStageToWorld::from_tracking_origin(self, snapshot)?;
        let origin_world = glam_vec3_from_vec3d(snapshot.eye);
        let stage_offset = transform.stage_direction_from_world(world_position - origin_world);
        if !stage_offset.is_finite() {
            bail!("invalid XR tracking-origin rebase offset");
        }
        Ok(Self {
            origin_stage: stage_position - stage_offset,
            ..self
        })
    }
}

impl XrStageToWorld {
    pub fn from_tracking_origin(
        origin: XrTrackingOrigin,
        snapshot: EngineCameraSnapshot,
    ) -> Result<Self> {
        let world_yaw = snapshot.yaw_radians as f32;
        if !world_yaw.is_finite() {
            bail!("invalid XR player root yaw {}", snapshot.yaw_radians);
        }
        Ok(Self {
            origin_stage: origin.origin_stage,
            origin_world: glam_vec3_from_vec3d(snapshot.eye),
            stage_to_world_rotation: Quat::from_rotation_y(normalize_angle(
                world_yaw - origin.stage_yaw,
            )),
        })
    }

    pub fn transform_pose(self, stage_position: Vec3, stage_orientation: Quat) -> (Vec3, Quat) {
        (
            self.origin_world
                + self
                    .stage_to_world_rotation
                    .mul_vec3(stage_position - self.origin_stage),
            (self.stage_to_world_rotation * stage_orientation).normalize(),
        )
    }

    pub fn transform_position(self, stage_position: Vec3) -> Vec3 {
        self.origin_world
            + self
                .stage_to_world_rotation
                .mul_vec3(stage_position - self.origin_stage)
    }

    pub fn transform_direction(self, stage_direction: Vec3) -> Vec3 {
        self.stage_to_world_rotation
            .mul_vec3(stage_direction)
            .normalize_or_zero()
    }

    pub fn stage_direction_from_world(self, world_direction: Vec3) -> Vec3 {
        self.stage_to_world_rotation
            .inverse()
            .mul_vec3(world_direction)
    }
}

pub fn xr_headset_stage_position_from_views(views: &[xr::View]) -> Result<Vec3> {
    if views.len() < 2 {
        bail!("OpenXR runtime returned fewer than two stereo views");
    }
    let left_pose = mclone_xr_host::view_pose(&views[0])?;
    let right_pose = mclone_xr_host::view_pose(&views[1])?;
    Ok((left_pose.position + right_pose.position) * 0.5)
}

pub fn xr_view_to_chunk_render_view(
    view: &xr::View,
    transform: XrStageToWorld,
    near: f32,
    far: f32,
) -> Result<ChunkRenderView> {
    let stage_pose = mclone_xr_host::view_pose(view)?;
    let (camera_position, camera_orientation) =
        transform.transform_pose(stage_pose.position, stage_pose.orientation);
    let render_view = mclone_xr_host::render_view_from_world_pose(
        mclone_xr_host::XrViewPose {
            position: camera_position,
            orientation: camera_orientation,
        },
        view.fov,
        near,
        far,
    )?;
    Ok(chunk_render_view_from_xr_render_view(render_view))
}

pub fn fixed_startup_view_pose_render_views(
    view_pose: XrStartupViewPose,
    eye_fovs: [xr::Fovf; 2],
) -> Result<[ChunkRenderView; 2]> {
    let yaw_radians = view_pose.yaw_degrees.to_radians();
    if !yaw_radians.is_finite() {
        bail!("invalid XR fixed render view yaw {}", view_pose.yaw_degrees);
    }
    let center = Vec3::from_array(view_pose.position);
    if !center.is_finite() {
        bail!(
            "invalid XR fixed render view position {:?}",
            view_pose.position
        );
    }
    let orientation = Quat::from_rotation_y(yaw_radians);
    let eye_right = orientation * Vec3::X;
    let half_eye_offset = eye_right * (XR_FIXED_RENDER_EYE_SEPARATION_BLOCKS * 0.5);
    let left =
        fixed_startup_view_pose_render_view(center - half_eye_offset, orientation, eye_fovs[0])?;
    let right =
        fixed_startup_view_pose_render_view(center + half_eye_offset, orientation, eye_fovs[1])?;
    Ok([left, right])
}

fn fixed_startup_view_pose_render_view(
    position: Vec3,
    orientation: Quat,
    fov: xr::Fovf,
) -> Result<ChunkRenderView> {
    let render_view = mclone_xr_host::render_view_from_world_pose(
        mclone_xr_host::XrViewPose {
            position,
            orientation,
        },
        fov,
        XR_NEAR,
        XR_FAR,
    )?;
    Ok(chunk_render_view_from_xr_render_view(render_view))
}

pub fn chunk_render_view_from_xr_render_view(
    view: mclone_xr_host::XrRenderView,
) -> ChunkRenderView {
    ChunkRenderView {
        view: view.view,
        projection: view.projection,
        view_projection: view.view_projection,
        camera_position: view.camera_position,
        camera_forward: view.camera_forward,
        camera_right: view.camera_right,
        camera_up: view.camera_up,
        aspect: view.aspect,
        fov_y_radians: view.fov_y_radians,
        z_near: view.z_near,
        z_far: view.z_far,
        projection_kind: ChunkProjectionKind::External,
    }
}

pub fn yaw_from_forward(forward: Vec3) -> Option<f32> {
    if !forward.is_finite() {
        return None;
    }
    let horizontal = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    if horizontal.length_squared() <= f32::EPSILON {
        return None;
    }
    Some((-horizontal.x).atan2(-horizontal.z))
}

pub fn normalize_angle(angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub fn glam_vec3_from_vec3d(value: Vec3d) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

pub fn vec3d_from_glam(value: Vec3) -> Vec3d {
    Vec3d::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
}

pub fn apply_xr_startup_view_pose(
    camera: &mut EngineCameraController,
    position: [f32; 3],
    yaw_degrees: f32,
) -> Result<()> {
    let yaw_radians = yaw_degrees.to_radians();
    if !yaw_radians.is_finite() {
        bail!("invalid XR startup view yaw {}", yaw_degrees);
    }
    camera.set_eye_pose(
        vec3d_from_glam(Vec3::from_array(position)),
        f64::from(yaw_radians),
        0.0,
    );
    Ok(())
}

pub fn xr_locomotion_input_from_controllers(
    controllers: &[XrControllerSnapshot],
    dt_seconds: f64,
    movement_yaw_radians: Option<f64>,
) -> EngineCameraInput {
    xr_locomotion_input_from_controllers_with_turn_policy(
        controllers,
        dt_seconds,
        movement_yaw_radians,
        XrTurnPolicy::default(),
    )
}

pub fn xr_locomotion_input_from_controllers_with_turn_policy(
    controllers: &[XrControllerSnapshot],
    dt_seconds: f64,
    movement_yaw_radians: Option<f64>,
    turn_policy: XrTurnPolicy,
) -> EngineCameraInput {
    let dt_seconds = if dt_seconds.is_finite() {
        dt_seconds.clamp(0.0, XR_LOCOMOTION_MAX_FRAME_SECONDS)
    } else {
        0.0
    };
    let left_axis = xr_left_stick_axis(controllers);
    let right_axis = xr_right_stick_axis(controllers);
    let jump = xr_right_a_pressed(controllers);
    let movement_impulse = (left_axis.length_squared() > f32::EPSILON)
        .then(|| xr_left_stick_movement_impulse(left_axis));
    let mouse_delta_x =
        if matches!(turn_policy, XrTurnPolicy::Smooth) && ENGINE_CAMERA_MOUSE_SENSITIVITY > 0.0 {
            let yaw_delta =
                -f64::from(right_axis.x) * XR_JOYPAD_YAW_SPEED_RADIANS_PER_SECOND * dt_seconds;
            -yaw_delta / ENGINE_CAMERA_MOUSE_SENSITIVITY
        } else {
            0.0
        };

    EngineCameraInput {
        dt_seconds,
        mouse_delta_x,
        jump,
        movement_impulse,
        movement_yaw_radians,
        ..EngineCameraInput::default()
    }
}

fn xr_left_stick_axis(controllers: &[XrControllerSnapshot]) -> Vec2 {
    controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Left)
        .map(|controller| joypad_axis_after_dead_zone(controller.thumbstick))
        .unwrap_or(Vec2::ZERO)
}

fn xr_left_stick_raw_axis(controllers: &[XrControllerSnapshot]) -> Vec2 {
    controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Left)
        .map(|controller| {
            if controller.thumbstick.is_finite() {
                controller.thumbstick
            } else {
                Vec2::ZERO
            }
        })
        .unwrap_or(Vec2::ZERO)
}

fn xr_left_stick_blink_engaged(controllers: &[XrControllerSnapshot]) -> bool {
    xr_left_stick_raw_axis(controllers).length() > XR_BLINK_TELEPORT_STICK_THRESHOLD
}

fn xr_right_stick_axis(controllers: &[XrControllerSnapshot]) -> Vec2 {
    controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Right)
        .map(|controller| joypad_axis_after_dead_zone(controller.thumbstick))
        .unwrap_or(Vec2::ZERO)
}

fn xr_right_a_pressed(controllers: &[XrControllerSnapshot]) -> bool {
    controllers
        .iter()
        .any(|controller| controller.hand == XrHand::Right && controller.a_pressed)
}

pub fn xr_hand_push_input_from_controllers(
    controllers: &[XrControllerSnapshot],
    views: &[xr::View],
    transform: XrStageToWorld,
) -> Result<Option<EngineHandPushInput>> {
    if views.len() < 2 {
        bail!("OpenXR runtime returned fewer than two stereo views");
    }
    let Some(left_hand_position) = xr_controller_hand_position(controllers, XrHand::Left) else {
        return Ok(None);
    };
    let Some(right_hand_position) = xr_controller_hand_position(controllers, XrHand::Right) else {
        return Ok(None);
    };
    let left_pose = mclone_xr_host::view_pose(&views[0])?;
    let right_pose = mclone_xr_host::view_pose(&views[1])?;
    let head_stage_position = (left_pose.position + right_pose.position) * 0.5;

    Ok(Some(EngineHandPushInput::new(
        vec3d_from_glam(transform.transform_position(head_stage_position)),
        vec3d_from_glam(transform.transform_position(left_hand_position)),
        vec3d_from_glam(transform.transform_position(right_hand_position)),
    )))
}

fn xr_controller_hand_position(controllers: &[XrControllerSnapshot], hand: XrHand) -> Option<Vec3> {
    controllers
        .iter()
        .find(|controller| controller.hand == hand)
        .and_then(|controller| controller.grip_position.or(controller.aim_position))
}

fn xr_blink_teleport_config() -> TeleportConfig {
    TeleportConfig {
        max_distance: XR_BLINK_TELEPORT_MAX_DISTANCE,
        arc_height: XR_BLINK_TELEPORT_ARC_HEIGHT,
        ..TeleportConfig::default()
    }
}

fn xr_blink_teleport_disabled_frame(
    travel_assist_mode: GameTravelAssistMode,
    _controllers: &[XrControllerSnapshot],
) -> Option<XrBlinkTeleportFrame> {
    (travel_assist_mode != GameTravelAssistMode::Blink).then_some(XrBlinkTeleportFrame {
        suppress_left_stick_movement: false,
    })
}

fn xr_blink_teleport_intent(
    camera: &EngineCameraController,
    controllers: &[XrControllerSnapshot],
    transform: XrStageToWorld,
    target_yaw_degrees: f64,
) -> Option<TeleportIntent> {
    let controller = controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Left)?;
    let aim_origin = transform.transform_position(controller.aim_position?);
    let aim_direction = transform.transform_direction(controller.aim_direction?);
    if !aim_origin.is_finite()
        || !aim_direction.is_finite()
        || aim_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    let pose = camera.player().pose();
    Some(TeleportIntent::new(
        pose.position,
        vec3d_from_glam(aim_origin),
        vec3d_from_glam(aim_direction.normalize_or_zero()),
        target_yaw_degrees,
    ))
}

fn teleport_intent_query_matches(left: TeleportIntent, right: TeleportIntent) -> bool {
    left.start_feet == right.start_feet
        && left.aim_origin == right.aim_origin
        && left.aim_direction == right.aim_direction
}

fn xr_headset_player_yaw_degrees_from_views(
    views: &[xr::View],
    transform: XrStageToWorld,
) -> Result<f64> {
    xr_headset_world_yaw_from_views(views, transform).map(|yaw| -f64::from(yaw).to_degrees())
}

fn xr_blink_teleport_target_yaw_degrees_from_stick(
    base_yaw_degrees: f64,
    previous_target_yaw_degrees: f64,
    activation_angle_radians: &mut Option<f64>,
    axis: Vec2,
) -> f64 {
    if !axis.is_finite() || axis.length() <= XR_BLINK_TELEPORT_HEADING_STICK_THRESHOLD {
        return previous_target_yaw_degrees;
    }
    let current_angle_radians = f64::from(axis.y.atan2(axis.x));
    let start_angle_radians = *activation_angle_radians.get_or_insert(current_angle_radians);
    target_yaw_degrees_from_stick_delta(
        base_yaw_degrees,
        start_angle_radians,
        current_angle_radians,
    )
}

fn target_yaw_degrees_from_stick_delta(
    base_yaw_degrees: f64,
    start_angle_radians: f64,
    current_angle_radians: f64,
) -> f64 {
    let delta_degrees =
        normalize_radians_180(current_angle_radians - start_angle_radians).to_degrees();
    normalize_degrees_180(base_yaw_degrees - delta_degrees)
}

fn xr_blink_teleport_landing_yaw_radians(
    current_root_yaw_radians: f64,
    views: &[xr::View],
    transform: XrStageToWorld,
    target_yaw_degrees: f64,
) -> Result<f64> {
    if !current_root_yaw_radians.is_finite() || !target_yaw_degrees.is_finite() {
        bail!("invalid XR Blink landing yaw input");
    }
    let current_headset_yaw_radians = f64::from(xr_headset_world_yaw_from_views(views, transform)?);
    let target_headset_yaw_radians = -target_yaw_degrees.to_radians();
    let yaw_delta = normalize_radians_180(target_headset_yaw_radians - current_headset_yaw_radians);
    Ok(normalize_radians_180(current_root_yaw_radians + yaw_delta))
}

fn normalize_radians_180(radians: f64) -> f64 {
    if !radians.is_finite() {
        return 0.0;
    }
    (radians + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

fn normalize_degrees_180(degrees: f64) -> f64 {
    if !degrees.is_finite() {
        return 0.0;
    }
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

fn xr_blink_teleport_lines(preview: &TeleportPreview) -> Vec<WorldGuiLine> {
    let mut lines = Vec::new();
    let arc_color = if preview.is_valid() {
        XR_BLINK_TELEPORT_VALID_ARC_COLOR
    } else {
        XR_BLINK_TELEPORT_INVALID_ARC_COLOR
    };
    for points in preview.arc_points.windows(2) {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(points[0]),
            glam_vec3_from_vec3d(points[1]),
            arc_color,
        ));
    }
    if let Some(feet) = preview.target_feet {
        push_xr_blink_teleport_cross(
            &mut lines,
            feet,
            XR_BLINK_TELEPORT_MARKER_RADIUS,
            XR_BLINK_TELEPORT_FEET_COLOR,
        );
        push_xr_blink_teleport_heading_arrow(
            &mut lines,
            feet,
            preview.target_yaw_degrees,
            XR_BLINK_TELEPORT_HEADING_COLOR,
        );
    }
    if let (Some(feet), Some(dot)) = (preview.target_feet, preview.marker_dot) {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(feet),
            glam_vec3_from_vec3d(dot),
            XR_BLINK_TELEPORT_DOT_COLOR,
        ));
        push_xr_blink_teleport_cross(
            &mut lines,
            dot,
            XR_BLINK_TELEPORT_DOT_RADIUS,
            XR_BLINK_TELEPORT_DOT_COLOR,
        );
    }
    lines
}

fn push_xr_blink_teleport_cross(
    lines: &mut Vec<WorldGuiLine>,
    center: Vec3d,
    radius: f64,
    color: [f32; 4],
) {
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

fn push_xr_blink_teleport_heading_arrow(
    lines: &mut Vec<WorldGuiLine>,
    feet: Vec3d,
    yaw_degrees: f64,
    color: [f32; 4],
) {
    let forward = horizontal_forward_from_player_yaw_degrees(yaw_degrees);
    if forward == Vec3d::ZERO {
        return;
    }
    let start = feet.add(Vec3d::new(0.0, 0.05, 0.0));
    let end = start.add(forward.scale(XR_BLINK_TELEPORT_HEADING_ARROW_LENGTH));
    lines.push(WorldGuiLine::new(
        glam_vec3_from_vec3d(start),
        glam_vec3_from_vec3d(end),
        color,
    ));

    let right = Vec3d::new(forward.z, 0.0, -forward.x);
    let head_angle = XR_BLINK_TELEPORT_HEADING_ARROW_HEAD_ANGLE_DEGREES.to_radians();
    let back = forward.scale(-head_angle.cos() * XR_BLINK_TELEPORT_HEADING_ARROW_HEAD_LENGTH);
    let side = right.scale(head_angle.sin() * XR_BLINK_TELEPORT_HEADING_ARROW_HEAD_LENGTH);
    for head in [back.add(side), back.subtract(side)] {
        lines.push(WorldGuiLine::new(
            glam_vec3_from_vec3d(end),
            glam_vec3_from_vec3d(end.add(head)),
            color,
        ));
    }
}

fn horizontal_forward_from_player_yaw_degrees(yaw_degrees: f64) -> Vec3d {
    if !yaw_degrees.is_finite() {
        return Vec3d::ZERO;
    }
    let forward = view_vector_from_rot_degrees(0.0, yaw_degrees);
    Vec3d::new(forward.x, 0.0, forward.z)
}

pub fn xr_automated_flight_input(
    dt_seconds: f64,
    movement_yaw_radians: Option<f64>,
) -> EngineCameraInput {
    let dt_seconds = if dt_seconds.is_finite() {
        dt_seconds.clamp(0.0, XR_LOCOMOTION_MAX_FRAME_SECONDS)
    } else {
        0.0
    };
    EngineCameraInput {
        dt_seconds,
        movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, 1.0)),
        movement_yaw_radians,
        ..EngineCameraInput::default()
    }
}

pub fn xr_automated_orbit_input(
    dt_seconds: f64,
    speed_blocks_per_second: f64,
    elapsed_seconds: f64,
) -> EngineCameraInput {
    let angular_speed = if speed_blocks_per_second.is_finite()
        && speed_blocks_per_second > 0.0
        && XR_AUTOMATED_ORBIT_RADIUS_BLOCKS > 0.0
    {
        speed_blocks_per_second / XR_AUTOMATED_ORBIT_RADIUS_BLOCKS
    } else {
        0.0
    };
    let yaw = if elapsed_seconds.is_finite() {
        (elapsed_seconds.max(0.0) * angular_speed).rem_euclid(std::f64::consts::TAU)
    } else {
        0.0
    };
    xr_automated_flight_input(dt_seconds, Some(yaw))
}

fn game_movement_mode(mode: EngineCameraMovementMode) -> GameMovementMode {
    match mode {
        EngineCameraMovementMode::Walking => GameMovementMode::Walk,
        EngineCameraMovementMode::Fly => GameMovementMode::Fly,
        EngineCameraMovementMode::HandPush => GameMovementMode::HandPush,
    }
}

fn engine_movement_mode(mode: GameMovementMode) -> EngineCameraMovementMode {
    match mode {
        GameMovementMode::Walk => EngineCameraMovementMode::Walking,
        GameMovementMode::Fly => EngineCameraMovementMode::Fly,
        GameMovementMode::HandPush => EngineCameraMovementMode::HandPush,
    }
}

fn game_collision_mode(mode: EngineCameraCollisionMode) -> GameCollisionMode {
    match mode {
        EngineCameraCollisionMode::Normal => GameCollisionMode::Normal,
        EngineCameraCollisionMode::NoClip => GameCollisionMode::NoClip,
    }
}

fn engine_collision_mode(mode: GameCollisionMode) -> EngineCameraCollisionMode {
    match mode {
        GameCollisionMode::Normal => EngineCameraCollisionMode::Normal,
        GameCollisionMode::NoClip => EngineCameraCollisionMode::NoClip,
    }
}

pub fn xr_menu_toggle_pressed(controllers: &[XrControllerSnapshot]) -> bool {
    controllers
        .iter()
        .any(|controller| controller.hand == XR_MENU_TOGGLE_HAND && controller.select_pressed)
}

pub fn xr_game_ui_toggle_pressed(controllers: &[XrControllerSnapshot]) -> bool {
    controllers.iter().any(|controller| {
        controller.hand == XR_GAME_UI_TOGGLE_HAND && controller.thumbstick_pressed
    })
}

fn xr_gameplay_interaction_buttons_from_controllers(
    controllers: &[XrControllerSnapshot],
    previous: XrGameplayInteractionButtons,
) -> XrGameplayInteractionButtons {
    let Some(controller) = controllers
        .iter()
        .find(|controller| controller.hand == XR_GAMEPLAY_INTERACTION_HAND)
    else {
        return XrGameplayInteractionButtons::default();
    };
    XrGameplayInteractionButtons {
        attack: xr_analog_button_down(controller.trigger, previous.attack),
        use_item: xr_analog_button_down(controller.squeeze, previous.use_item),
    }
}

fn xr_analog_button_down(value: f32, was_down: bool) -> bool {
    let value = if value.is_finite() { value } else { 0.0 };
    if was_down {
        value >= XR_MENU_POINTER_TRIGGER_RELEASE
    } else {
        value >= XR_MENU_POINTER_TRIGGER_PRESS
    }
}

pub fn xr_controller_interaction_ray_from_controllers(
    controllers: &[XrControllerSnapshot],
    transform: XrStageToWorld,
) -> Option<(Vec3, Vec3)> {
    [XrHand::Right, XrHand::Left].into_iter().find_map(|hand| {
        controllers
            .iter()
            .find(|controller| controller.hand == hand)
            .and_then(|controller| xr_controller_interaction_ray(controller, transform))
    })
}

fn xr_controller_interaction_ray(
    controller: &XrControllerSnapshot,
    transform: XrStageToWorld,
) -> Option<(Vec3, Vec3)> {
    let ray_origin = transform.transform_position(controller.aim_position?);
    let ray_direction = transform.transform_direction(controller.aim_direction?);
    if !ray_origin.is_finite()
        || !ray_direction.is_finite()
        || ray_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    Some((ray_origin, ray_direction.normalize()))
}

pub fn xr_gameplay_controller_ray_line_from_controllers(
    controllers: &[XrControllerSnapshot],
    transform: XrStageToWorld,
    hit_distance: Option<f32>,
    pick_range: f32,
) -> Option<WorldGuiLine> {
    let controller = controllers
        .iter()
        .find(|controller| controller.hand == XR_GAMEPLAY_INTERACTION_HAND)?;
    let (ray_origin, ray_direction) = xr_controller_interaction_ray(controller, transform)?;
    let pick_range = if pick_range.is_finite() && pick_range > 0.0 {
        pick_range
    } else {
        XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS
    };
    let distance = hit_distance
        .filter(|distance| distance.is_finite() && *distance >= 0.0)
        .unwrap_or(pick_range)
        .clamp(0.0, pick_range);
    Some(WorldGuiLine::new(
        ray_origin,
        ray_origin + ray_direction * distance,
        xr_gameplay_controller_ray_color(controller),
    ))
}

fn xr_gameplay_controller_ray_color(controller: &XrControllerSnapshot) -> [f32; 4] {
    let trigger_active =
        controller.trigger.is_finite() && controller.trigger >= XR_MENU_POINTER_TRIGGER_PRESS;
    let squeeze_active =
        controller.squeeze.is_finite() && controller.squeeze >= XR_MENU_POINTER_TRIGGER_PRESS;
    if trigger_active || squeeze_active {
        XR_MENU_TRIGGER_RAY_COLOR
    } else {
        xr_menu_controller_ray_color(controller)
    }
}

pub fn xr_menu_panel_from_render_views(render_views: [ChunkRenderView; 2]) -> WorldGuiPanel {
    let center_position = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
    let forward = average_unit_direction(
        render_views[0].camera_forward,
        render_views[1].camera_forward,
        Vec3::NEG_Z,
    );
    let up = average_unit_direction(
        render_views[0].camera_up,
        render_views[1].camera_up,
        Vec3::Y,
    );
    let right = average_unit_direction(
        render_views[0].camera_right,
        render_views[1].camera_right,
        Vec3::X,
    );
    WorldGuiPanel::new(
        center_position + forward * XR_MENU_PANEL_DISTANCE_BLOCKS,
        right,
        up,
        XR_MENU_PANEL_WIDTH_BLOCKS,
        xr_menu_panel_height_blocks(),
    )
}

pub fn xr_diagnostic_panel_from_render_views(render_views: [ChunkRenderView; 2]) -> WorldGuiPanel {
    let center_position = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
    let forward = average_unit_direction(
        render_views[0].camera_forward,
        render_views[1].camera_forward,
        Vec3::NEG_Z,
    );
    let up = average_unit_direction(
        render_views[0].camera_up,
        render_views[1].camera_up,
        Vec3::Y,
    );
    let right = average_unit_direction(
        render_views[0].camera_right,
        render_views[1].camera_right,
        Vec3::X,
    );
    WorldGuiPanel::new(
        center_position
            + forward * XR_DIAGNOSTIC_PANEL_DISTANCE_BLOCKS
            + right * XR_DIAGNOSTIC_PANEL_RIGHT_OFFSET_BLOCKS
            + up * XR_DIAGNOSTIC_PANEL_UP_OFFSET_BLOCKS,
        right,
        up,
        XR_DIAGNOSTIC_PANEL_WIDTH_BLOCKS,
        xr_diagnostic_panel_height_blocks(),
    )
}

pub fn xr_game_ui_panel_from_controllers(
    controllers: &[XrControllerSnapshot],
    transform: XrStageToWorld,
    render_views: [ChunkRenderView; 2],
) -> Option<WorldGuiPanel> {
    let controller = controllers
        .iter()
        .find(|controller| controller.hand == XR_GAME_UI_PANEL_HAND)?;
    let stage_anchor = controller.grip_position.or(controller.aim_position)?;
    let anchor = transform.transform_position(stage_anchor);
    if !anchor.is_finite() {
        return None;
    }

    let eye_center = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
    let view_up = average_unit_direction(
        render_views[0].camera_up,
        render_views[1].camera_up,
        Vec3::Y,
    );
    let view_right = average_unit_direction(
        render_views[0].camera_right,
        render_views[1].camera_right,
        Vec3::X,
    );
    let view_forward = average_unit_direction(
        render_views[0].camera_forward,
        render_views[1].camera_forward,
        Vec3::NEG_Z,
    );
    let mut normal = eye_center - anchor;
    if !normal.is_finite() || normal.length_squared() <= f32::EPSILON {
        normal = -view_forward;
    }
    let normal = normal.normalize_or_zero();
    if normal.length_squared() <= f32::EPSILON {
        return None;
    }
    let mut right = view_right - normal * view_right.dot(normal);
    if !right.is_finite() || right.length_squared() <= f32::EPSILON {
        right = view_up.cross(normal);
    }
    let right = right.normalize_or_zero();
    if right.length_squared() <= f32::EPSILON {
        return None;
    }
    let up = normal.cross(right).normalize_or_zero();
    if up.length_squared() <= f32::EPSILON {
        return None;
    }
    let center = anchor
        + up * XR_GAME_UI_PANEL_UP_OFFSET_BLOCKS
        + normal * XR_GAME_UI_PANEL_FORWARD_OFFSET_BLOCKS;
    Some(WorldGuiPanel::new(
        center,
        right,
        up,
        XR_GAME_UI_PANEL_WIDTH_BLOCKS,
        xr_game_ui_panel_height_blocks(),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrMenuPointerHit {
    pub hand: XrHand,
    pub point: Point,
    pub trigger: f32,
    pub distance: f32,
}

pub fn xr_menu_pointer_hit_from_controllers(
    controllers: &[XrControllerSnapshot],
    transform: XrStageToWorld,
    panel: WorldGuiPanel,
    gui_scale: GuiScale,
) -> Option<XrMenuPointerHit> {
    [XrHand::Right, XrHand::Left].into_iter().find_map(|hand| {
        controllers
            .iter()
            .find(|controller| controller.hand == hand)
            .and_then(|controller| {
                let ray_origin = transform.transform_position(controller.aim_position?);
                let ray_direction = transform.transform_direction(controller.aim_direction?);
                xr_menu_panel_pointer_hit(panel, gui_scale, ray_origin, ray_direction).map(
                    |(point, distance)| XrMenuPointerHit {
                        hand,
                        point,
                        trigger: controller.trigger,
                        distance,
                    },
                )
            })
    })
}

pub fn xr_menu_controller_ray_lines_from_controllers(
    controllers: &[XrControllerSnapshot],
    transform: XrStageToWorld,
    panel: WorldGuiPanel,
) -> Vec<WorldGuiLine> {
    [XrHand::Left, XrHand::Right]
        .into_iter()
        .filter_map(|hand| {
            controllers
                .iter()
                .find(|controller| controller.hand == hand)
                .and_then(|controller| xr_menu_controller_ray_line(controller, transform, panel))
        })
        .collect()
}

fn xr_menu_controller_ray_line(
    controller: &XrControllerSnapshot,
    transform: XrStageToWorld,
    panel: WorldGuiPanel,
) -> Option<WorldGuiLine> {
    let ray_origin = transform.transform_position(controller.aim_position?);
    let ray_direction = transform.transform_direction(controller.aim_direction?);
    if !ray_origin.is_finite()
        || !ray_direction.is_finite()
        || ray_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    let direction = ray_direction.normalize();
    let distance = xr_menu_panel_ray_distance(panel, ray_origin, direction)
        .unwrap_or(XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS)
        .clamp(0.0, XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS);
    Some(WorldGuiLine::new(
        ray_origin,
        ray_origin + direction * distance,
        xr_menu_controller_ray_color(controller),
    ))
}

pub fn xr_menu_controller_ray_color(controller: &XrControllerSnapshot) -> [f32; 4] {
    if controller.trigger.is_finite() && controller.trigger >= XR_MENU_POINTER_TRIGGER_PRESS {
        return XR_MENU_TRIGGER_RAY_COLOR;
    }
    match controller.hand {
        XrHand::Left => XR_MENU_LEFT_RAY_COLOR,
        XrHand::Right => XR_MENU_RIGHT_RAY_COLOR,
    }
}

pub fn xr_menu_panel_ray_distance(
    panel: WorldGuiPanel,
    ray_origin: Vec3,
    ray_direction: Vec3,
) -> Option<f32> {
    if !ray_origin.is_finite()
        || !ray_direction.is_finite()
        || ray_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    let direction = ray_direction.normalize();
    let normal = panel.right.cross(panel.up).normalize_or_zero();
    if normal.length_squared() <= f32::EPSILON {
        return None;
    }
    let denominator = direction.dot(normal);
    if denominator.abs() <= 1.0e-5 {
        return None;
    }
    let distance = (panel.center - ray_origin).dot(normal) / denominator;
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let hit = ray_origin + direction * distance;
    let local = hit - panel.center;
    let panel_x = local.dot(panel.right) + panel.width * 0.5;
    let panel_y = panel.height * 0.5 - local.dot(panel.up);
    if panel_x < 0.0 || panel_x > panel.width || panel_y < 0.0 || panel_y > panel.height {
        return None;
    }
    Some(distance)
}

pub fn xr_menu_panel_pointer_hit(
    panel: WorldGuiPanel,
    gui_scale: GuiScale,
    ray_origin: Vec3,
    ray_direction: Vec3,
) -> Option<(Point, f32)> {
    if !ray_origin.is_finite()
        || !ray_direction.is_finite()
        || ray_direction.length_squared() <= f32::EPSILON
    {
        return None;
    }
    let direction = ray_direction.normalize();
    let normal = panel.right.cross(panel.up).normalize_or_zero();
    if normal.length_squared() <= f32::EPSILON {
        return None;
    }
    let denominator = direction.dot(normal);
    if denominator.abs() <= 1.0e-5 {
        return None;
    }
    let distance = (panel.center - ray_origin).dot(normal) / denominator;
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let hit = ray_origin + direction * distance;
    let local = hit - panel.center;
    let panel_x = local.dot(panel.right) + panel.width * 0.5;
    let panel_y = panel.height * 0.5 - local.dot(panel.up);
    if panel_x < 0.0 || panel_x > panel.width || panel_y < 0.0 || panel_y > panel.height {
        return None;
    }
    Some((
        Point {
            x: panel_x / panel.width * gui_scale.width,
            y: panel_y / panel.height * gui_scale.height,
        },
        distance,
    ))
}

pub fn xr_menu_pointer_trigger_down(trigger: f32, was_down: bool) -> bool {
    xr_analog_button_down(trigger, was_down)
}

fn xr_menu_panel_height_blocks() -> f32 {
    XR_MENU_PANEL_WIDTH_BLOCKS * XR_MENU_PANEL_PIXELS[1] as f32 / XR_MENU_PANEL_PIXELS[0] as f32
}

fn xr_diagnostic_panel_height_blocks() -> f32 {
    XR_DIAGNOSTIC_PANEL_WIDTH_BLOCKS * XR_DIAGNOSTIC_PANEL_PIXELS[1] as f32
        / XR_DIAGNOSTIC_PANEL_PIXELS[0] as f32
}

fn xr_game_ui_panel_height_blocks() -> f32 {
    XR_GAME_UI_PANEL_WIDTH_BLOCKS * XR_MENU_PANEL_PIXELS[1] as f32 / XR_MENU_PANEL_PIXELS[0] as f32
}

fn underwater_overlay_from_forward(
    forward: Vec3,
    water_vision: f32,
    effect_strength: f32,
) -> UnderwaterOverlay {
    let forward = if forward.is_finite() && forward.length_squared() > f32::EPSILON {
        forward.normalize()
    } else {
        Vec3::Z
    };
    let yaw = engine_movement_yaw_from_forward(forward).unwrap_or(0.0);
    let pitch = forward.y.clamp(-1.0, 1.0).asin();
    UnderwaterOverlay::vanilla_from_native_camera(yaw, pitch)
        .with_effect(water_vision, effect_strength)
}

fn xr_head_comfort_target(
    reconciliation: Option<EngineRoomScaleReconciliation>,
    headset_world_position: Vec3,
    client: &mclone_client::ClientRuntime,
) -> XrHeadComfortTarget {
    let horizontal_residual = reconciliation
        .map(|reconciliation| reconciliation.residual_horizontal_length_sqr().sqrt())
        .unwrap_or(0.0);
    let head_penetrating = sphere_intersects_solid_blocks(
        client,
        vec3d_from_glam(headset_world_position),
        HAND_PUSH_DEFAULT_HEAD_RADIUS,
    );
    xr_head_comfort_target_from_inputs(horizontal_residual, head_penetrating)
}

fn xr_head_comfort_target_from_inputs(
    horizontal_residual: f64,
    head_penetrating: bool,
) -> XrHeadComfortTarget {
    let blocked_residual = horizontal_residual.is_finite()
        && horizontal_residual > XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS;
    let mut alpha = xr_head_comfort_residual_alpha(horizontal_residual);
    if head_penetrating {
        alpha = alpha.max(XR_HEAD_COMFORT_HEAD_PENETRATION_ALPHA);
    }
    XrHeadComfortTarget {
        alpha: alpha.min(XR_HEAD_COMFORT_MAX_ALPHA),
        blocked_residual,
        head_penetrating,
    }
}

fn xr_head_comfort_residual_alpha(horizontal_residual: f64) -> f32 {
    if !horizontal_residual.is_finite()
        || horizontal_residual <= XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS
    {
        return 0.0;
    }
    let span = XR_HEAD_COMFORT_RESIDUAL_FULL_BLOCKS - XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS;
    let t = ((horizontal_residual - XR_HEAD_COMFORT_RESIDUAL_DEAD_ZONE_BLOCKS) / span)
        .clamp(0.0, 1.0) as f32;
    let smooth = t * t * (3.0 - 2.0 * t);
    smooth * XR_HEAD_COMFORT_MAX_ALPHA
}

fn xr_head_comfort_fade_overlays(state: XrHeadComfortState) -> [Option<ScreenFadeOverlay>; 2] {
    let overlay = state.overlay();
    [overlay, overlay]
}

fn average_unit_direction(a: Vec3, b: Vec3, fallback: Vec3) -> Vec3 {
    let direction = (a + b) * 0.5;
    if direction.is_finite() && direction.length_squared() > f32::EPSILON {
        direction.normalize()
    } else {
        fallback
    }
}

pub fn xr_headset_world_yaw_from_views(
    views: &[xr::View],
    transform: XrStageToWorld,
) -> Result<f32> {
    if views.len() < 2 {
        bail!("OpenXR runtime returned fewer than two stereo views");
    }
    let left_pose = mclone_xr_host::view_pose(&views[0])?;
    let right_pose = mclone_xr_host::view_pose(&views[1])?;
    let left_forward = left_pose.orientation * Vec3::NEG_Z;
    let right_forward = right_pose.orientation * Vec3::NEG_Z;
    let stage_forward = average_unit_direction(left_forward, right_forward, Vec3::NEG_Z);
    let world_forward = transform.transform_direction(stage_forward);
    engine_movement_yaw_from_forward(world_forward)
        .ok_or_else(|| anyhow!("OpenXR returned an invalid headset locomotion yaw"))
}

fn engine_movement_yaw_from_forward(forward: Vec3) -> Option<f32> {
    if !forward.is_finite() {
        return None;
    }
    let horizontal = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    if horizontal.length_squared() <= f32::EPSILON {
        return None;
    }
    Some(horizontal.x.atan2(horizontal.z))
}

fn xr_left_stick_movement_impulse(axis: Vec2) -> EngineCameraMovementImpulse {
    EngineCameraMovementImpulse::new(-axis.x, axis.y)
}

fn joypad_axis_after_dead_zone(axis: Vec2) -> Vec2 {
    if !axis.is_finite() {
        return Vec2::ZERO;
    }
    let length = axis.length();
    if length <= XR_JOYPAD_DEAD_ZONE {
        return Vec2::ZERO;
    }
    let normalized = axis / length;
    let adjusted = ((length.min(1.0) - XR_JOYPAD_DEAD_ZONE) / (1.0 - XR_JOYPAD_DEAD_ZONE)).max(0.0);
    normalized * adjusted
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_app_runtime::session::RemoteSessionEndpoint;
    use mclone_app_runtime::world_catalog::{
        LocalWorldId, LocalWorldSummary, WorldCatalogCapabilities, WorldCatalogRequest,
        WorldCatalogResponse,
    };
    use mclone_render_session::{
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND, ENGINE_CAMERA_MOUSE_SENSITIVITY,
    };

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
        let options = XrSceneOptions::default();
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
        let input = xr_locomotion_input_from_controllers(
            &[
                test_controller(XrHand::Left, Vec2::new(0.0, 1.0), false),
                test_controller(XrHand::Right, Vec2::ZERO, true),
            ],
            1.0 / 72.0,
            Some(0.25),
        );

        assert_eq!(input.dt_seconds, 1.0 / 72.0);
        assert!(input.jump);
        assert_eq!(
            input.movement_impulse,
            Some(EngineCameraMovementImpulse::new(0.0, 1.0))
        );
        assert_eq!(input.movement_yaw_radians, Some(0.25));
        assert_eq!(input.mouse_delta_x, 0.0);
    }

    #[test]
    fn xr_locomotion_maps_left_stick_lateral_axis_to_strafe() {
        let input = xr_locomotion_input_from_controllers(
            &[test_controller(XrHand::Left, Vec2::new(1.0, 0.0), false)],
            1.0 / 72.0,
            None,
        );

        assert_eq!(
            input.movement_impulse,
            Some(EngineCameraMovementImpulse::new(-1.0, 0.0))
        );
        assert_eq!(input.movement_yaw_radians, None);
    }

    #[test]
    fn xr_locomotion_dead_zone_filters_small_thumbstick_noise() {
        let input = xr_locomotion_input_from_controllers(
            &[
                test_controller(XrHand::Left, Vec2::splat(XR_JOYPAD_DEAD_ZONE * 0.25), false),
                test_controller(
                    XrHand::Right,
                    Vec2::splat(XR_JOYPAD_DEAD_ZONE * 0.25),
                    false,
                ),
            ],
            1.0,
            None,
        );

        assert_eq!(input.movement_impulse, None);
        assert_eq!(input.mouse_delta_x, 0.0);
    }

    #[test]
    fn xr_blink_teleport_engages_only_past_left_stick_threshold() {
        assert_eq!(XR_BLINK_TELEPORT_STICK_THRESHOLD, 0.75);
        assert!(!xr_left_stick_blink_engaged(&[test_controller(
            XrHand::Left,
            Vec2::new(XR_BLINK_TELEPORT_STICK_THRESHOLD, 0.0),
            false,
        )]));
        assert!(xr_left_stick_blink_engaged(&[test_controller(
            XrHand::Left,
            Vec2::new(XR_BLINK_TELEPORT_STICK_THRESHOLD + 0.01, 0.0),
            false,
        )]));
        assert!(xr_left_stick_blink_engaged(&[test_controller(
            XrHand::Left,
            Vec2::new(0.0, -(XR_BLINK_TELEPORT_STICK_THRESHOLD + 0.01)),
            false,
        )]));
    }

    #[test]
    fn xr_travel_assist_off_does_not_suppress_left_stick_movement() {
        let controllers = [test_controller(
            XrHand::Left,
            Vec2::new(XR_BLINK_TELEPORT_STICK_THRESHOLD + 0.01, 0.0),
            false,
        )];

        let frame = xr_blink_teleport_disabled_frame(GameTravelAssistMode::Off, &controllers)
            .expect("travel assist off disables Blink");

        assert!(!frame.suppress_left_stick_movement);
        assert!(
            xr_blink_teleport_disabled_frame(GameTravelAssistMode::Blink, &controllers).is_none()
        );
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

        let intent = xr_blink_teleport_intent(&camera, &[left], transform, 37.0).expect("intent");

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
        let input = xr_locomotion_input_from_controllers(
            &[test_controller(XrHand::Right, Vec2::new(1.0, 0.0), false)],
            0.05,
            None,
        );

        assert_eq!(input.mouse_delta_x, 0.0);
        assert_eq!(input.movement_impulse, None);
        assert!(!input.jump);
    }

    #[test]
    fn xr_locomotion_smooth_turn_policy_uses_mouse_turn_path() {
        let input = xr_locomotion_input_from_controllers_with_turn_policy(
            &[test_controller(XrHand::Right, Vec2::new(1.0, 0.0), false)],
            0.05,
            None,
            XrTurnPolicy::Smooth,
        );
        let expected_mouse_delta =
            XR_JOYPAD_YAW_SPEED_RADIANS_PER_SECOND * 0.05 / ENGINE_CAMERA_MOUSE_SENSITIVITY;

        assert!((input.mouse_delta_x - expected_mouse_delta).abs() < 1.0e-6);
        assert_eq!(input.movement_impulse, None);
        assert!(!input.jump);
    }

    #[test]
    fn xr_locomotion_maps_right_a_to_jump() {
        let right = test_controller(XrHand::Right, Vec2::ZERO, true);
        let input = xr_locomotion_input_from_controllers(&[right], 1.0 / 72.0, None);

        assert!(input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn xr_locomotion_right_stick_up_no_longer_maps_to_jump() {
        let right = test_controller(XrHand::Right, Vec2::new(0.0, 1.0), false);
        let input = xr_locomotion_input_from_controllers(&[right], 1.0 / 72.0, None);

        assert!(!input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn xr_locomotion_right_stick_down_no_longer_maps_to_descend() {
        let right = test_controller(XrHand::Right, Vec2::new(0.0, -1.0), false);
        let input = xr_locomotion_input_from_controllers(&[right], 1.0 / 72.0, None);

        assert!(!input.descend);
        assert!(!input.jump);
    }

    #[test]
    fn xr_locomotion_pure_yaw_does_not_trigger_vertical_movement() {
        let right = test_controller(XrHand::Right, Vec2::new(1.0, 0.0), false);
        let input = xr_locomotion_input_from_controllers(&[right], 1.0 / 72.0, None);

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
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.select_pressed = true;
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.select_pressed = true;

        assert!(xr_menu_toggle_pressed(&[left]));
        assert!(!xr_menu_toggle_pressed(&[right]));
        assert!(xr_menu_toggle_pressed(&[right, left]));
        assert!(!xr_menu_toggle_pressed(&[]));
    }

    #[test]
    fn xr_game_ui_toggle_uses_left_thumbstick_click_only() {
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.thumbstick_pressed = true;
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.thumbstick_pressed = true;

        assert!(xr_game_ui_toggle_pressed(&[left]));
        assert!(!xr_game_ui_toggle_pressed(&[right]));
        assert!(xr_game_ui_toggle_pressed(&[right, left]));
        assert!(!xr_game_ui_toggle_pressed(&[]));
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
        left.trigger = 0.25;
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.aim_position = Some(Vec3::new(0.25, 64.0, 0.0));
        right.aim_direction = Some(Vec3::NEG_Z);
        right.trigger = 0.75;

        let hit = xr_menu_pointer_hit_from_controllers(&[left, right], transform, panel, scale)
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

        let lines = xr_menu_controller_ray_lines_from_controllers(&[right], transform, panel);

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

        let lines = xr_menu_controller_ray_lines_from_controllers(&[left], transform, panel);

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
        let mut controller = test_controller(XrHand::Right, Vec2::ZERO, false);
        assert_eq!(
            xr_menu_controller_ray_color(&controller),
            XR_MENU_RIGHT_RAY_COLOR
        );

        controller.trigger = XR_MENU_POINTER_TRIGGER_PRESS;
        assert_eq!(
            xr_menu_controller_ray_color(&controller),
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
                assert_eq!(options.seed, EMULATED_XR_CREATE_SEED);
                assert_eq!(options.display_name, "New World");
                assert!(options.requested_id.is_some());
            }
            other => panic!("expected local world creation start, got {other:?}"),
        }

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
        let profile = xr_client_experience_profile();

        assert!(profile.settings.frame_pipeline_overlay.is_supported());
    }

    #[test]
    fn xr_profile_supports_debug_diagnostics() {
        let profile = xr_client_experience_profile();

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
                ClientExperienceController::new(xr_client_experience_profile());
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
            self.client_experience
                .catalog()
                .ui_state()
                .entries
                .iter()
                .flatten()
                .find(|entry| entry.display_name.as_str() == "Existing World")
                .expect("existing world catalog row")
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
                &[pressed],
                self.profile.transform,
                self.profile.panel,
                self.profile.gui_scale,
            )
            .expect("pressed controller ray hits emulated XR panel");
            assert_eq!(press_hit.hand, XrHand::Right);
            assert!(xr_menu_pointer_trigger_down(press_hit.trigger, false));
            assert_point_close(press_hit.point, point);
            let rays = xr_menu_controller_ray_lines_from_controllers(
                &[pressed],
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
                &[released],
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
            for effect in effects.projection {
                match effect {
                    ClientExperienceProjectionEffect::ApplyUiAction(action) => {
                        self.ui.apply_action(action);
                    }
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
                    let summary = LocalWorldSummary::new(
                        id,
                        options.display_name.clone(),
                        options.seed,
                        2_000 + self.worlds.len() as u64,
                    )
                    .unwrap();
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
                WorldCatalogRequest::DeleteWorld { id } => {
                    let index = self
                        .worlds
                        .iter()
                        .position(|world| world.id == id)
                        .unwrap_or_else(|| panic!("emulated catalog missing world `{id}`"));
                    let summary = self.worlds.remove(index);
                    WorldCatalogResponse::WorldDeleted { id: summary.id }
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

        fn controller_for_gui_point(self, point: Point, trigger: f32) -> XrControllerSnapshot {
            let target = self.world_point_for_gui_point(point);
            let normal = self.panel.right.cross(self.panel.up).normalize();
            let origin = target + normal * 0.75;
            let mut controller = test_controller(XrHand::Right, Vec2::ZERO, false);
            controller.aim_position = Some(origin);
            controller.aim_direction = Some((target - origin).normalize());
            controller.grip_position = Some(origin + Vec3::new(0.0, -0.12, 0.0));
            controller.trigger = trigger;
            controller
        }
    }

    fn assert_point_close(actual: Point, expected: Point) {
        assert!(
            (actual.x - expected.x).abs() < 1.0e-4 && (actual.y - expected.y).abs() < 1.0e-4,
            "expected point {actual:?} to be close to {expected:?}"
        );
    }

    #[test]
    fn xr_gameplay_interaction_buttons_use_right_trigger_and_squeeze() {
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.trigger = 1.0;
        left.squeeze = 1.0;
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.trigger = XR_MENU_POINTER_TRIGGER_PRESS;
        right.squeeze = XR_MENU_POINTER_TRIGGER_PRESS - 0.01;

        let buttons = xr_gameplay_interaction_buttons_from_controllers(
            &[left, right],
            XrGameplayInteractionButtons::default(),
        );

        assert_eq!(
            buttons,
            XrGameplayInteractionButtons {
                attack: true,
                use_item: false
            }
        );
    }

    #[test]
    fn xr_gameplay_interaction_buttons_use_hysteresis() {
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.trigger = XR_MENU_POINTER_TRIGGER_PRESS;
        right.squeeze = XR_MENU_POINTER_TRIGGER_PRESS;
        let pressed = xr_gameplay_interaction_buttons_from_controllers(
            &[right],
            XrGameplayInteractionButtons::default(),
        );
        assert_eq!(
            pressed,
            XrGameplayInteractionButtons {
                attack: true,
                use_item: true
            }
        );

        right.trigger = XR_MENU_POINTER_TRIGGER_RELEASE + 0.01;
        right.squeeze = XR_MENU_POINTER_TRIGGER_RELEASE + 0.01;
        let held = xr_gameplay_interaction_buttons_from_controllers(&[right], pressed);
        assert_eq!(held, pressed);

        right.trigger = XR_MENU_POINTER_TRIGGER_RELEASE - 0.01;
        right.squeeze = XR_MENU_POINTER_TRIGGER_RELEASE - 0.01;
        let released = xr_gameplay_interaction_buttons_from_controllers(&[right], held);
        assert_eq!(released, XrGameplayInteractionButtons::default());
    }

    #[test]
    fn xr_gameplay_interaction_edges_fire_on_press_only() {
        let previous = XrGameplayInteractionButtons {
            attack: false,
            use_item: true,
        };
        let current = XrGameplayInteractionButtons {
            attack: true,
            use_item: true,
        };

        assert_eq!(
            previous.press_edges(current),
            XrGameplayInteractionEdges {
                attack: true,
                use_item: false
            }
        );
        assert!(!current.press_edges(current).any());
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
        right.squeeze = XR_MENU_POINTER_TRIGGER_PRESS;

        let line = xr_gameplay_controller_ray_line_from_controllers(
            &[right],
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
        let fov = xr::Fovf {
            angle_left: -0.5,
            angle_right: 0.5,
            angle_up: 0.5,
            angle_down: -0.5,
        };

        let render_views = fixed_startup_view_pose_render_views(view_pose, [fov, fov]).unwrap();
        let center = (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        let forward = average_unit_direction(
            render_views[0].camera_forward,
            render_views[1].camera_forward,
            Vec3::NEG_Z,
        );

        assert!((center - Vec3::from_array(view_pose.position)).length() < 1.0e-6);
        assert!((yaw_from_forward(forward).unwrap() - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
        assert!(forward.y.abs() < 1.0e-6);
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

    fn test_controller(hand: XrHand, thumbstick: Vec2, a_pressed: bool) -> XrControllerSnapshot {
        XrControllerSnapshot {
            hand,
            aim_position: Some(Vec3::ZERO),
            aim_direction: Some(Vec3::NEG_Z),
            grip_position: Some(Vec3::ZERO),
            trigger: 0.0,
            squeeze: 0.0,
            select_pressed: false,
            a_pressed,
            y_pressed: false,
            thumbstick,
            thumbstick_pressed: false,
        }
    }

    fn assert_vec3d_close(actual: Vec3d, expected: Vec3d) {
        assert!(
            actual.distance_to_sqr(expected) < 1.0e-10,
            "expected {actual:?} to be close to {expected:?}"
        );
    }

    fn test_xr_view(position: Vec3, orientation: Quat) -> xr::View {
        xr::View {
            pose: xr::Posef {
                orientation: xr::Quaternionf {
                    x: orientation.x,
                    y: orientation.y,
                    z: orientation.z,
                    w: orientation.w,
                },
                position: xr::Vector3f {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                },
            },
            fov: xr::Fovf {
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
