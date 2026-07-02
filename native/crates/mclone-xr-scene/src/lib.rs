#![forbid(unsafe_code)]

use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use glam::{Quat, Vec2, Vec3};
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
use mclone_app_runtime::host_mode::RemoteDedicatedServerSession;
use mclone_app_runtime::local_single_view::{
    LocalSingleViewSceneOptions, LocalSingleViewStartupPump, LocalSingleViewStartupStep,
    NativeSingleViewSceneRuntime, NativeSingleViewSessionRuntime,
};
use mclone_app_runtime::render_assets::TexturedMeshAssets;
use mclone_app_runtime::session::{
    ActiveSessionDescriptor, RemoteSessionEndpoint, SessionStartRequest,
};
use mclone_app_runtime::{
    RuntimePollDiagnostics, debug_block_palette_overlay, elapsed_ms,
    set_player_appearance_command_for_ui_model,
};
use mclone_assets::AssetSource;
use mclone_audio::{AudioEngine, landing_playback_for_impact};
use mclone_client::{
    BlockInteractionTarget, ClientInteractionController, HAND_PUSH_DEFAULT_HEAD_RADIUS,
    sphere_intersects_solid_blocks,
};
use mclone_core::{Aabb, BlockStateId, ChunkPos, Vec3d, time};
use mclone_mesh::{RenderSectionKey, TexturedRenderSectionMesh, quad_face_count_from_indices};
use mclone_render::actor_assets::ActorTextureImage;
use mclone_render::chunk::{
    ChunkDepthTarget, ChunkMultiviewDepthTarget, ChunkMultiviewRenderTarget, ChunkProjectionKind,
    ChunkRenderTarget, ChunkRenderView, PreparedTexturedSectionStereoDraw,
    TexturedSectionDrawResources, TexturedSectionRecordCacheStats,
    TexturedSectionRecordPrepareStats, TexturedSectionRenderOptions, TexturedSectionRenderPhase,
    TexturedSectionRenderStats, TexturedSectionUploadReport,
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
    EngineCameraController, EngineCameraInput, EngineCameraMovementImpulse,
    EngineCameraMovementMode, EngineCameraSnapshot, EngineDebugVisualOptions, EngineHandPushInput,
    EngineRoomScaleReconciliation, actor_instances_from_presentations, engine_debug_world_lines,
};
use mclone_ui::{
    Color, DEFAULT_JOIN_REMOTE_ADDR, GameFramePacingMode, GameMovementMode, GamePlayerModel,
    GameScreen, GameUiAction, GameUiHost, GameUiRenderState, GameXrTurnMode, GuiDrawList, GuiScale,
    LoadingProgressOverlay, Point, Rect, StatusOverlay, UiDrawCacheStats, UiPanelRevision,
    render_loading_progress_overlay, render_status_overlay,
};
use mclone_xr_host::{XrControllerSnapshot, XrHand};
use openxr as xr;

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
pub const XR_AUTOMATED_ORBIT_RADIUS_BLOCKS: f64 = 16.0;
pub const XR_MENU_TOGGLE_HAND: XrHand = XrHand::Left;
pub const XR_UI_FPS_CAP: u32 = 90;
pub const XR_MENU_PANEL_PIXELS: [u32; 2] = [1024, 576];
pub const XR_MENU_PANEL_DISTANCE_BLOCKS: f32 = 2.2;
pub const XR_MENU_PANEL_WIDTH_BLOCKS: f32 = 1.75;
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

#[derive(Clone, Copy, Debug, PartialEq)]
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
        }
    }
}

impl XrSceneOptions {
    pub fn center(self) -> ChunkPos {
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
    pub runtime_upload_enqueue_ms: f64,
    pub runtime_upload_select_ms: f64,
    pub runtime_upload_apply_ms: f64,
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
pub struct XrTerrainEyeRenderTiming {
    pub prepare_ms: f64,
    pub cull_ms: f64,
    pub uniform_write_ms: f64,
    pub translucent_collect_ms: f64,
    pub translucent_sort_ms: f64,
    pub encode_ms: f64,
    pub section_encode_ms: f64,
    pub submit_ms: f64,
    pub poll_wait_ms: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct XrTerrainUploadSummary {
    pub poll_changed: bool,
    pub poll_total_ms: f64,
    pub poll_drain_updates_ms: f64,
    pub poll_apply_updates_ms: f64,
    pub poll_dirty_mark_ms: f64,
    pub poll_client_apply_updates_ms: f64,
    pub poll_diagnostics_ms: f64,
    pub poll_diagnostics_refreshed: bool,
    pub poll_diagnostics_cache_age_ms: f64,
    pub server_diagnostics_detail_refreshes: u64,
    pub server_diagnostics_detail_age_ms: f64,
    pub poll_server_tick_ms: f64,
    pub poll_server_reported_total_ms: f64,
    pub poll_scheduler_tick_ms: f64,
    pub poll_updates: usize,
    pub poll_snapshot_updates: usize,
    pub poll_section_block_updates: usize,
    pub poll_unload_updates: usize,
    pub server_command_queue_depth: usize,
    pub server_update_queue_depth: usize,
    pub server_pending_jobs: usize,
    pub server_pending_publications: usize,
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
    pub traversal_ready_section_count: usize,
    pub record_cache: TexturedSectionRecordCacheStats,
    pub visibility_graph_build_count: usize,
    pub visibility_graph_total_ms: f64,
    pub visibility_graph_worst_ms: f64,
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

struct XrMenuPanelDraw {
    draw: GuiDrawList,
    cache_revision: Option<UiPanelRevision>,
    draw_cache: UiDrawCacheStats,
}

fn prepare_xr_menu_panel_draw(
    ui: &mut GameUiHost,
    gui_scale: GuiScale,
    ui_state: GameUiRenderState,
    progress: Option<&LoadingProgressOverlay>,
    status: &StatusOverlay,
) -> XrMenuPanelDraw {
    ui.set_scale(gui_scale);
    if progress.is_none() && !status.visible {
        if let Some(panel_draw) = ui.render_v2_panel_draw_list(ui_state) {
            return XrMenuPanelDraw {
                draw: panel_draw.draw,
                cache_revision: Some(panel_draw.revision),
                draw_cache: panel_draw.cache,
            };
        }
    }

    let ui_active = ui.is_active();
    let mut draw = ui.render_draw_list(ui_state);
    if let Some(progress) = progress {
        render_loading_progress_overlay(gui_scale, &mut draw, progress);
    }
    render_status_overlay(gui_scale, &mut draw, status);
    XrMenuPanelDraw {
        draw,
        cache_revision: None,
        draw_cache: if ui_active {
            UiDrawCacheStats::rebuild()
        } else {
            UiDrawCacheStats::default()
        },
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
    scene: XrSceneOptions,
    pump: LocalSingleViewStartupPump,
    camera: EngineCameraController,
    startup_view_pose: Option<XrStartupViewPose>,
}

#[derive(Debug)]
pub enum XrLocalOnlyRemoteSession {}

impl RemoteDedicatedServerSession for XrLocalOnlyRemoteSession {
    fn send_command(
        &mut self,
        _command: mclone_protocol::ClientCommand,
    ) -> Result<Vec<mclone_protocol::ServerUpdate>> {
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
    runtime: Option<NativeSingleViewSessionRuntime<S>>,
    local_startup: Option<XrLocalStartup>,
    session_runtime_factory: Option<XrSessionRuntimeFactory<S>>,
    camera: EngineCameraController,
    interaction: ClientInteractionController,
    initial_alignment_mode: XrViewAlignmentMode,
    render_options: TexturedSectionRenderOptions,
    player_collision_box_visible: bool,
    player_model: GamePlayerModel,
    draw: TexturedSectionDrawResources,
    pending_section_uploads: VecDeque<TexturedRenderSectionMesh>,
    pending_section_removals: BTreeSet<RenderSectionKey>,
    pending_compile_release_batches: VecDeque<XrTerrainCompileReleaseBatch>,
    actors: ActorDrawResources,
    far_lod: FarTerrainLodRenderer,
    selection_outline: SelectionOutlineRenderer,
    world_gui_renderer: WorldGuiRenderer,
    ui: GameUiHost,
    session_status: StatusOverlay,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XrTerrainCompileReleaseBatch {
    remaining_lifecycle_items: usize,
    compile_jobs: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct XrTerrainUploadApplyReport {
    upload: TexturedSectionUploadReport,
    release_compile_jobs: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct XrTerrainUploadEnqueueReport {
    queued_lifecycle_items: usize,
    superseded_lifecycle_items: usize,
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
        let request = SessionStartRequest::NewLocalWorld { seed: scene.seed };
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
            local_single_view_options(scene),
            mesh_assets,
        )
        .context("create initial XR local world startup pump")?;
        let ui = xr_game_ui_for_session(None, scene.seed);
        Ok(Self {
            scene,
            color_format,
            runtime: None,
            local_startup: Some(XrLocalStartup {
                request: request.clone(),
                scene,
                pump,
                camera: camera.clone(),
                startup_view_pose,
            }),
            session_runtime_factory: None,
            camera,
            interaction: ClientInteractionController::new(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            player_collision_box_visible: false,
            player_model: GamePlayerModel::default(),
            draw,
            pending_section_uploads: VecDeque::new(),
            pending_section_removals: BTreeSet::new(),
            pending_compile_release_batches: VecDeque::new(),
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
            ui,
            session_status: StatusOverlay::new(request.starting_message(), true),
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
        })
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
        let started = start_xr_terrain_runtime(
            device,
            queue,
            color_format,
            runtime,
            scene.movement_speed_multiplier,
            startup_view_pose,
        )?;
        let ui = xr_game_ui_for_session(started.runtime.active_session(), scene.seed);
        let mut world_gui_renderer = WorldGuiRenderer::new(device, color_format);
        world_gui_renderer
            .upload_texture_atlas(
                device,
                queue,
                started.runtime.mesh_assets().atlas.as_upload(),
            )
            .context("upload XR GUI atlas")?;
        let state = Self {
            scene,
            color_format,
            runtime: Some(started.runtime),
            local_startup: None,
            session_runtime_factory: None,
            camera: started.camera,
            interaction: ClientInteractionController::new(),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            player_collision_box_visible: false,
            player_model: GamePlayerModel::default(),
            draw: started.draw,
            pending_section_uploads: VecDeque::new(),
            pending_section_removals: BTreeSet::new(),
            pending_compile_release_batches: VecDeque::new(),
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
            ui,
            session_status: StatusOverlay::hidden(),
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
        Ok(state)
    }

    pub fn set_audio_engine(&mut self, audio: Option<AudioEngine>) {
        self.audio = audio;
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
        let left_eye_start = Instant::now();
        let left_eye = self.render_eye_target(
            device,
            queue,
            &prepared_stereo_draw,
            left_target,
            render_views[0],
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
        if !self.ui.is_active() {
            return Ok(XrWorldOverlayStats::default());
        }
        let Some(panel) = self.menu_panel_pose else {
            return Ok(XrWorldOverlayStats::default());
        };
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        let ui_state = self.current_ui_render_state();
        let progress = self
            .local_startup
            .as_ref()
            .and_then(|startup| startup.pump.progress_overlay());
        let panel_draw = prepare_xr_menu_panel_draw(
            &mut self.ui,
            gui_scale,
            ui_state,
            progress.as_ref(),
            &self.session_status,
        );
        let controller_ray_lines = self
            .xr_menu_controller_ray_lines(panel)
            .context("build XR menu controller ray visuals")?;
        let panel_stats = if let Some(cache_revision) = panel_draw.cache_revision {
            self.world_gui_renderer.render_panel_cached_multiview(
                device,
                queue,
                encoder,
                overlay_target,
                render_views,
                XR_MENU_PANEL_PIXELS,
                [gui_scale.width, gui_scale.height],
                &panel_draw.draw,
                panel,
                &controller_ray_lines,
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
                &panel_draw.draw,
                panel,
                &controller_ray_lines,
            )
        }
        .context("render XR menu panel multiview")?;
        Ok(XrWorldOverlayStats {
            panel: panel_stats,
            draw_cache: panel_draw.draw_cache,
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
    ) -> Result<()> {
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
            return Ok(());
        }
        let mut transform = self
            .reconcile_room_scale_body_to_headset(&views)
            .context("reconcile XR room-scale body pose")?;
        self.update_head_comfort_state(transform, &views, dt_seconds)
            .context("update XR head comfort fade state")?;
        if self.ui.is_active() {
            self.snap_turn_state.reset();
            self.commit_engine_camera_player_pose()
                .context("sync XR room-scale player pose")?;
            return Ok(());
        }
        if let Some(yaw_delta_radians) = self.snap_turn_delta_from_controllers(controllers) {
            transform = self
                .apply_snap_turn_preserving_headset(&views, transform, yaw_delta_radians)
                .context("apply XR snap turn")?;
        }
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians_with_transform(&views, transform)
            .context("resolve XR locomotion frame")?;
        let mut input = xr_locomotion_input_from_controllers_with_turn_policy(
            controllers,
            dt_seconds,
            movement_yaw_radians,
            self.turn_policy,
        );
        input.hand_push = xr_hand_push_input_from_controllers(controllers, &views, transform)
            .context("resolve XR hand-push input")?;
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        self.play_landing_events();
        self.commit_engine_camera_player_pose()
            .context("sync XR locomotion player pose")?;
        if !suppress_gameplay_interaction {
            self.apply_xr_gameplay_interaction_edges(gameplay_interaction_edges)?;
        }
        Ok(())
    }

    pub fn apply_automated_flight_input(
        &mut self,
        views: [xr::View; 2],
        speed_blocks_per_second: f64,
    ) -> Result<()> {
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            return Ok(());
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
        self.camera
            .set_movement_mode(EngineCameraMovementMode::NoClip);
        self.camera
            .set_speed_blocks_per_second(speed_blocks_per_second);
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians(&views)
            .context("resolve XR automated flight locomotion frame")?;
        let input = xr_automated_flight_input(dt_seconds, movement_yaw_radians);
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        self.commit_engine_camera_player_pose()
            .context("sync XR automated flight player pose")?;
        Ok(())
    }

    pub fn apply_automated_orbit_input(
        &mut self,
        speed_blocks_per_second: f64,
        elapsed_seconds: f64,
    ) -> Result<()> {
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            return Ok(());
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
        self.camera
            .set_movement_mode(EngineCameraMovementMode::NoClip);
        self.camera
            .set_speed_blocks_per_second(speed_blocks_per_second);
        let input = xr_automated_orbit_input(dt_seconds, speed_blocks_per_second, elapsed_seconds);
        let runtime = self.runtime.as_ref().expect("runtime presence checked");
        self.camera.apply_movement_input(runtime.client(), input);
        self.commit_engine_camera_player_pose()
            .context("sync XR automated orbit player pose")?;
        Ok(())
    }

    pub fn apply_automated_stationary_input(&mut self) {
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            return;
        }
        self.ui.close();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_pose = None;
        self.menu_panel_anchor = XrUiPanelAnchor::Head;
        self.menu_panel_recenter_pending = false;
        self.head_comfort.reset();
        self.snap_turn_state.reset();
        self.last_locomotion_update = Some(Instant::now());
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

    pub fn replace_session_for_request(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: SessionStartRequest,
    ) -> Result<()> {
        let scene = match &request {
            SessionStartRequest::NewLocalWorld { seed } => self.local_world_options(*seed),
            SessionStartRequest::JoinRemote { .. } => self.remote_session_options(),
            SessionStartRequest::Unknown => bail!("unsupported unknown XR replacement session"),
        };
        if matches!(request, SessionStartRequest::NewLocalWorld { .. }) {
            return self.begin_local_replacement_start(request, scene);
        }
        self.start_replacement_session(device, queue, request, scene)
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
            return Ok(XrTerrainUploadSummary {
                traversal_ready_section_count: self.draw.traversal_ready_section_count(),
                record_cache: self.draw.record_cache_stats(),
                ..XrTerrainUploadSummary::default()
            });
        };
        let poll_start = Instant::now();
        let poll_changed = self.poll().context("poll XR terrain runtime")?;
        timing.runtime_poll_ms = elapsed_ms(poll_start.elapsed());
        let runtime = self
            .runtime
            .as_ref()
            .expect("runtime presence checked before poll");
        let poll_summary = xr_poll_diagnostics_upload_summary(runtime.last_poll_diagnostics());
        let has_runtime_render_work = runtime.has_pending_render_work(camera_position);
        if !poll_changed && !has_runtime_render_work && !self.has_pending_section_upload_work() {
            let ready_start = Instant::now();
            let ready_sections = runtime.traversal_ready_render_section_keys(camera_position);
            timing.runtime_ready_sections_ms = elapsed_ms(ready_start.elapsed());
            let traversal_ready_section_count = ready_sections.len();
            let ready_publish_start = Instant::now();
            self.draw.set_traversal_ready_sections(&ready_sections);
            timing.runtime_ready_publish_ms = elapsed_ms(ready_publish_start.elapsed());
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
                queued_upload_section_count: self.pending_section_uploads.len(),
                queued_upload_removed_section_count: self.pending_section_removals.len(),
                traversal_ready_section_count,
                record_cache: self.draw.record_cache_stats(),
                ..poll_summary
            });
        }
        let budgeted_section_uploads = self.uses_budgeted_section_uploads();
        let mut upload_report = TexturedSectionUploadReport::default();
        let drained_pending_uploads_before_sync =
            budgeted_section_uploads && self.has_pending_section_upload_work();
        if drained_pending_uploads_before_sync {
            let upload_start = Instant::now();
            let drained_report = self.apply_section_update_uploads(
                device,
                mclone_render_session::RenderSectionCacheUpdate::default(),
                timing,
            )?;
            timing.runtime_gpu_upload_ms += elapsed_ms(upload_start.elapsed());
            accumulate_upload_report(&mut upload_report, drained_report.upload);
            self.release_render_compile_jobs(drained_report.release_compile_jobs);
        }
        let upload_backpressure_active =
            budgeted_section_uploads && self.has_pending_section_upload_work();
        let should_sync_render_sections =
            (poll_changed || has_runtime_render_work) && !upload_backpressure_active;
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
        if should_sync_render_sections || !drained_pending_uploads_before_sync {
            let upload_start = Instant::now();
            let section_update_report =
                self.apply_section_update_uploads(device, section_update, timing)?;
            timing.runtime_gpu_upload_ms += elapsed_ms(upload_start.elapsed());
            accumulate_upload_report(&mut upload_report, section_update_report.upload);
            self.release_render_compile_jobs(section_update_report.release_compile_jobs);
            pending_compile_jobs_after_sync = self
                .runtime
                .as_ref()
                .map_or(0, |runtime| runtime.render_compile_pending_job_count());
        }
        let ready_start = Instant::now();
        let runtime = self
            .runtime
            .as_ref()
            .expect("runtime presence checked before section sync");
        let ready_sections = runtime.traversal_ready_render_section_keys(camera_position);
        timing.runtime_ready_sections_ms = elapsed_ms(ready_start.elapsed());
        let traversal_ready_section_count = ready_sections.len();
        let ready_publish_start = Instant::now();
        self.draw.set_traversal_ready_sections(&ready_sections);
        timing.runtime_ready_publish_ms = elapsed_ms(ready_publish_start.elapsed());
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
            queued_upload_section_count: self.pending_section_uploads.len(),
            queued_upload_removed_section_count: self.pending_section_removals.len(),
            traversal_ready_section_count,
            record_cache: self.draw.record_cache_stats(),
            visibility_graph_build_count,
            visibility_graph_total_ms,
            visibility_graph_worst_ms,
            ..poll_summary
        })
    }

    fn has_pending_section_upload_work(&self) -> bool {
        !self.pending_section_uploads.is_empty() || !self.pending_section_removals.is_empty()
    }

    fn uses_budgeted_section_uploads(&self) -> bool {
        self.render_section_upload_budget.is_some() || self.render_section_accept_budget.is_some()
    }

    fn enqueue_section_update_uploads(
        &mut self,
        section_update: mclone_render_session::RenderSectionCacheUpdate,
    ) -> XrTerrainUploadEnqueueReport {
        let mut report = XrTerrainUploadEnqueueReport::default();
        for key in section_update.removed_section_keys {
            let before_uploads = self.pending_section_uploads.len();
            self.pending_section_uploads
                .retain(|section| section.key != key);
            report.superseded_lifecycle_items +=
                before_uploads - self.pending_section_uploads.len();
            if self.pending_section_removals.insert(key) {
                report.queued_lifecycle_items += 1;
            }
        }
        for section in section_update.rebuilt_sections {
            let before_uploads = self.pending_section_uploads.len();
            self.pending_section_uploads
                .retain(|pending| pending.key != section.key);
            report.superseded_lifecycle_items +=
                before_uploads - self.pending_section_uploads.len();
            if self.pending_section_removals.remove(&section.key) {
                report.superseded_lifecycle_items += 1;
            }
            self.pending_section_uploads.push_back(section);
            report.queued_lifecycle_items += 1;
        }
        report
    }

    fn apply_section_update_uploads(
        &mut self,
        device: &wgpu::Device,
        section_update: mclone_render_session::RenderSectionCacheUpdate,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadApplyReport> {
        if self.render_section_upload_budget.is_none()
            && self.render_section_accept_budget.is_none()
            && !self.has_pending_section_upload_work()
        {
            let release_compile_jobs = section_update.accepted_compile_result_count;
            let apply_start = Instant::now();
            let report = self
                .draw
                .apply_section_updates(
                    device,
                    &section_update.rebuilt_sections,
                    &section_update.removed_section_keys,
                )
                .context("upload XR terrain render section updates");
            timing.runtime_upload_apply_ms += elapsed_ms(apply_start.elapsed());
            return report.map(|upload| XrTerrainUploadApplyReport {
                upload,
                release_compile_jobs,
            });
        }

        let accepted_compile_result_count = section_update.accepted_compile_result_count;
        let enqueue_start = Instant::now();
        let enqueue_report = self.enqueue_section_update_uploads(section_update);
        timing.runtime_upload_enqueue_ms += elapsed_ms(enqueue_start.elapsed());
        let mut release_compile_jobs =
            self.complete_compile_release_work(enqueue_report.superseded_lifecycle_items);
        release_compile_jobs += self.queue_compile_release_batch(
            accepted_compile_result_count,
            enqueue_report.queued_lifecycle_items,
        );
        let select_start = Instant::now();
        let accept_budget = self.render_section_accept_budget.unwrap_or(usize::MAX);
        let upload_budget = self.render_section_upload_budget.unwrap_or(usize::MAX);
        let upload_count = upload_budget
            .min(accept_budget)
            .min(self.pending_section_uploads.len());
        let mut sections = Vec::with_capacity(upload_count);
        for _ in 0..upload_count {
            if let Some(section) = self.pending_section_uploads.pop_front() {
                sections.push(section);
            }
        }
        let removed = if self.render_section_accept_budget.is_some() {
            let remaining_accept_budget = accept_budget.saturating_sub(sections.len());
            self.take_budgeted_section_removals(remaining_accept_budget)
        } else {
            std::mem::take(&mut self.pending_section_removals)
        };
        timing.runtime_upload_select_ms += elapsed_ms(select_start.elapsed());
        if sections.is_empty() && removed.is_empty() {
            return Ok(XrTerrainUploadApplyReport {
                upload: TexturedSectionUploadReport::default(),
                release_compile_jobs,
            });
        }
        let apply_start = Instant::now();
        let report = self
            .draw
            .apply_section_updates(device, &sections, &removed)
            .context("accept budgeted XR terrain render section updates");
        timing.runtime_upload_apply_ms += elapsed_ms(apply_start.elapsed());
        report.map(|upload| {
            release_compile_jobs +=
                self.complete_compile_release_work(sections.len() + removed.len());
            XrTerrainUploadApplyReport {
                upload,
                release_compile_jobs,
            }
        })
    }

    fn take_budgeted_section_removals(&mut self, budget: usize) -> BTreeSet<RenderSectionKey> {
        if budget == 0 || self.pending_section_removals.is_empty() {
            return BTreeSet::new();
        }
        if budget >= self.pending_section_removals.len() {
            return std::mem::take(&mut self.pending_section_removals);
        }
        let keys: Vec<_> = self
            .pending_section_removals
            .iter()
            .copied()
            .take(budget)
            .collect();
        for key in &keys {
            self.pending_section_removals.remove(key);
        }
        keys.into_iter().collect()
    }

    fn queue_compile_release_batch(
        &mut self,
        compile_jobs: usize,
        lifecycle_items: usize,
    ) -> usize {
        if compile_jobs == 0 {
            return 0;
        }
        if lifecycle_items == 0 {
            return compile_jobs;
        }
        self.pending_compile_release_batches
            .push_back(XrTerrainCompileReleaseBatch {
                remaining_lifecycle_items: lifecycle_items,
                compile_jobs,
            });
        0
    }

    fn complete_compile_release_work(&mut self, mut lifecycle_items: usize) -> usize {
        let mut release_compile_jobs = 0;
        while lifecycle_items > 0 {
            let Some(front) = self.pending_compile_release_batches.front_mut() else {
                break;
            };
            if lifecycle_items < front.remaining_lifecycle_items {
                front.remaining_lifecycle_items -= lifecycle_items;
                break;
            }
            lifecycle_items -= front.remaining_lifecycle_items;
            release_compile_jobs += front.compile_jobs;
            self.pending_compile_release_batches.pop_front();
        }
        release_compile_jobs
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
        XrTerrainUploadSummary {
            pending_render_chunks_before: pending_render_chunks,
            pending_render_chunks_after: pending_render_chunks,
            pending_compile_jobs_before: pending_compile_jobs,
            pending_compile_jobs_after: pending_compile_jobs,
            max_pending_compile_jobs,
            available_compile_slots_before: available_compile_slots,
            available_compile_slots_after: available_compile_slots,
            queued_upload_section_count: self.pending_section_uploads.len(),
            queued_upload_removed_section_count: self.pending_section_removals.len(),
            traversal_ready_section_count: self.draw.traversal_ready_section_count(),
            record_cache: self.draw.record_cache_stats(),
            ..XrTerrainUploadSummary::default()
        }
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
        let progress = self
            .local_startup
            .as_ref()
            .and_then(|startup| startup.pump.progress_overlay());
        let panel_draw = prepare_xr_menu_panel_draw(
            &mut self.ui,
            gui_scale,
            ui_state,
            progress.as_ref(),
            &self.session_status,
        );
        let ui_active = self.ui.is_active();
        let summary_ui_draw = panel_draw.draw.clone();
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
        if let Some(overlay) = self.head_comfort.overlay() {
            self.screen_effects.render_fade_in_slot(
                device,
                queue,
                &mut encoder,
                RenderFrameTarget::color(target.color_view, target.size),
                overlay,
                view_slot,
            );
        }
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
        if !world_lines.is_empty() {
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
        }
        let mut ui_panel_stats = WorldGuiPanelRenderStats::default();
        if ui_active {
            if let Some(panel) = self.menu_panel_pose {
                let controller_ray_lines = self
                    .xr_menu_controller_ray_lines(panel)
                    .context("build XR menu controller ray visuals")?;
                ui_panel_stats = if let Some(cache_revision) = panel_draw.cache_revision {
                    self.world_gui_renderer.render_panel_cached_in_slot(
                        device,
                        queue,
                        &mut encoder,
                        RenderFrameTarget::color(target.color_view, target.size),
                        render_view,
                        XR_MENU_PANEL_PIXELS,
                        [gui_scale.width, gui_scale.height],
                        &panel_draw.draw,
                        panel,
                        &controller_ray_lines,
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
                        &panel_draw.draw,
                        panel,
                        &controller_ray_lines,
                        view_slot,
                    )
                }
                .with_context(|| format!("render XR menu panel for {label} eye"))?;
            }
        }
        let command_buffer = encoder.finish();
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
                prepare_ms,
                cull_ms: frame_timing.terrain_cull_ms,
                uniform_write_ms: frame_timing.terrain_uniform_write_ms,
                translucent_collect_ms: frame_timing.terrain_translucent_collect_ms,
                translucent_sort_ms: frame_timing.terrain_translucent_sort_ms,
                encode_ms: (encode_total_ms - prepare_ms).max(0.0),
                section_encode_ms: frame_timing.terrain_encode_ms,
                submit_ms,
                poll_wait_ms,
            },
            ui_panel: ui_panel_stats,
            ui_draw_cache: panel_draw.draw_cache,
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
        device
            .poll(wgpu::PollType::Wait)
            .map(|_| ())
            .with_context(|| format!("wait for {label} device idle"))?;
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

    fn commit_engine_camera_player_pose(&mut self) -> Result<bool> {
        let Some(runtime) = self.runtime.as_mut() else {
            return Ok(false);
        };
        commit_engine_camera_player_pose_for_runtime(
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
            player_model: self.player_model,
            movement_mode: game_movement_mode(self.camera.movement_mode()),
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
        let mut scene = self.scene;
        scene.seed = seed;
        if let Some(runtime) = &self.runtime {
            scene.render_distance = runtime.render_distance();
        }
        scene
    }

    fn remote_session_options(&self) -> XrSceneOptions {
        let mut scene = self.scene;
        if let Some(runtime) = &self.runtime {
            scene.render_distance = runtime.render_distance();
        }
        scene
    }

    fn begin_local_replacement_start(
        &mut self,
        request: SessionStartRequest,
        scene: XrSceneOptions,
    ) -> Result<()> {
        let scene = scene.validated()?;
        let mesh_assets = self
            .runtime
            .as_ref()
            .context("cannot replace XR local world before initial runtime is active")?
            .mesh_assets()
            .clone();
        let pump = LocalSingleViewStartupPump::with_mesh_assets(
            local_single_view_options(scene),
            mesh_assets,
        )
        .context("create XR local world startup pump")?;
        let mut camera = EngineCameraController::spawn_for_chunk(scene.center());
        camera.set_movement_speed_multiplier(f64::from(scene.movement_speed_multiplier));
        self.local_startup = Some(XrLocalStartup {
            request: request.clone(),
            scene,
            pump,
            camera,
            startup_view_pose: None,
        });
        self.session_status = StatusOverlay::new(request.starting_message(), true);
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
        let failure_message = startup.request.default_failure_message();
        let failure_seed = match &startup.request {
            SessionStartRequest::NewLocalWorld { seed } => Some(*seed),
            SessionStartRequest::JoinRemote { .. } | SessionStartRequest::Unknown => None,
        };
        match self.complete_local_startup(device, queue, startup, step) {
            Ok(()) => Ok(true),
            Err(error) => {
                log::error!("failed to complete XR local world startup: {error:#}");
                self.session_status = StatusOverlay::new(failure_message, false);
                if let Some(seed) = failure_seed {
                    self.ui.set_new_world_seed(seed);
                }
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
            scene,
            pump,
            mut camera,
            startup_view_pose,
        } = startup;
        let descriptor = request
            .active_descriptor()
            .context("XR local startup request did not describe an active session")?;
        let mut runtime = NativeSingleViewSessionRuntime::<S>::from_active_runtime(
            request.clone(),
            NativeSingleViewSceneRuntime::Local(pump.into_runtime()),
        )?;
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
        self.runtime = Some(runtime);
        self.camera = camera;
        self.draw = draw;
        self.render_stats = RenderStreamStats {
            section_count,
            index_count,
            face_count,
            ..RenderStreamStats::default()
        };
        self.sync_player_appearance()
            .context("sync XR local startup player appearance")?;
        self.clear_transient_world_state();
        self.session_status = StatusOverlay::hidden();
        match descriptor {
            ActiveSessionDescriptor::LocalWorld { seed } => {
                self.ui.set_new_world_seed(seed);
                self.ui.apply_action(GameUiAction::CreateWorld(seed));
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
        if !self.ui.is_active() {
            self.clear_menu_input_state();
        }
        Ok(())
    }

    fn fail_local_startup(&mut self, startup: XrLocalStartup, error: anyhow::Error) {
        log::error!(
            "failed to start XR local world {:?}: {error:#}",
            startup.request
        );
        self.session_status = StatusOverlay::new(startup.request.default_failure_message(), false);
        if let SessionStartRequest::NewLocalWorld { seed } = startup.request {
            self.ui.set_new_world_seed(seed);
        }
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
        self.latest_controllers.clear();
        self.first_eye_summary = None;
        self.last_ui_panel_stats = WorldGuiPanelRenderStats::default();
        self.last_ui_draw_cache_stats = UiDrawCacheStats::default();
        self.rendered_frames = 0;
        self.prefetched_live_upload = None;
        self.pending_section_uploads.clear();
        self.pending_section_removals.clear();
        self.pending_compile_release_batches.clear();
    }

    fn start_replacement_session(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        request: SessionStartRequest,
        scene: XrSceneOptions,
    ) -> Result<()> {
        self.session_status = StatusOverlay::new(request.starting_message(), true);
        let descriptor = request.active_descriptor().with_context(|| {
            format!("XR replacement request did not describe an active session: {request:?}")
        })?;
        let mesh_assets = self
            .runtime
            .as_ref()
            .context("cannot replace XR session before initial runtime is active")?
            .mesh_assets()
            .clone();
        let Some(factory) = self.session_runtime_factory.as_mut() else {
            let error = anyhow!("XR session runtime factory is not installed");
            log::error!("{error:#}");
            self.session_status = StatusOverlay::new(request.default_failure_message(), false);
            return Err(error);
        };
        let scene = scene.validated()?;
        let runtime = match factory(request.clone(), scene, mesh_assets) {
            Ok(runtime) => runtime,
            Err(error) => {
                log::error!("failed to start XR session {request:?}: {error:#}");
                self.session_status = StatusOverlay::new(request.default_failure_message(), false);
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
                self.session_status = StatusOverlay::new(request.default_failure_message(), false);
                return Err(error);
            }
        };

        self.scene = scene;
        self.runtime = Some(started.runtime);
        self.camera = started.camera;
        self.draw = started.draw;
        self.render_stats = started.render_stats;
        self.sync_player_appearance()
            .context("sync XR replacement player appearance")?;
        self.clear_transient_world_state();
        self.clear_menu_input_state();
        self.session_status = StatusOverlay::hidden();
        match descriptor {
            ActiveSessionDescriptor::LocalWorld { seed } => {
                self.ui.set_new_world_seed(seed);
                log::info!("XR created local world seed={seed}");
            }
            ActiveSessionDescriptor::Remote { endpoint } => {
                self.ui.set_join_remote_addr(endpoint.address.clone());
                log::info!("XR joined remote session {}", endpoint.address);
            }
        }
        Ok(())
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
        let mut scene_replaced = false;
        match action {
            GameUiAction::ToggleSectionOcclusion => {
                self.render_options.section_occlusion_culling =
                    !self.render_options.section_occlusion_culling;
                log::info!(
                    "XR section occlusion culling {}",
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
                    "XR fullbright {}",
                    if self.render_options.force_fullbright {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            GameUiAction::ToggleFarLod => {
                self.scene.far_lod.enabled = !self.scene.far_lod.enabled;
                if let Some(runtime) = &mut self.runtime {
                    runtime.clear_far_lod();
                }
                log::info!(
                    "XR far LOD {}",
                    if self.scene.far_lod.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            GameUiAction::SetFarLodRange(range_chunks) => {
                let range_chunks = u32::try_from(range_chunks)
                    .unwrap_or(MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS)
                    .clamp(
                        MIN_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                        MAX_FAR_TERRAIN_LOD_EXTRA_RADIUS_CHUNKS,
                    );
                self.scene.far_lod = self.scene.far_lod.with_extra_radius_chunks(range_chunks);
                if let Some(runtime) = &mut self.runtime {
                    runtime.clear_far_lod();
                }
                log::info!(
                    "XR far LOD range set to {} chunks beyond render distance",
                    self.scene.far_lod.extra_radius_chunks
                );
            }
            GameUiAction::TogglePlayerCollisionBox => {
                self.player_collision_box_visible = !self.player_collision_box_visible;
                log::info!(
                    "XR player collision box debug {}",
                    if self.player_collision_box_visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
            }
            GameUiAction::ToggleCrosshair => {}
            GameUiAction::ToggleFirstPersonPlayer => {
                let visible = !self.camera.first_person_player_visible();
                self.camera.set_first_person_player_visible(visible);
                log::info!(
                    "XR first-person player body {}",
                    if visible { "visible" } else { "hidden" }
                );
            }
            GameUiAction::SetRenderDistance(render_distance) => {
                let render_distance =
                    render_distance.clamp(1, MAX_XR_RENDER_DISTANCE as i32) as u32;
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
            GameUiAction::SetMovementMode(movement_mode) => {
                self.camera
                    .set_movement_mode(engine_movement_mode(movement_mode));
                let movement_mode = self.camera.movement_mode();
                log::info!("XR player movement mode {}", movement_mode.label());
            }
            GameUiAction::SetXrTurnMode(turn_mode) => {
                self.set_turn_policy(XrTurnPolicy::from_game_mode(turn_mode));
                log::info!("XR turn mode {}", turn_mode.label());
            }
            GameUiAction::SetFlySpeed(multiplier) => {
                self.camera.set_fly_speed_multiplier(f64::from(multiplier));
                log::info!(
                    "XR fly speed set to {:.1}x ({:.0} blocks/s)",
                    self.camera.fly_speed_multiplier(),
                    self.camera.speed_blocks_per_second()
                );
            }
            GameUiAction::SetMovementSpeed(multiplier) => {
                self.camera
                    .set_movement_speed_multiplier(f64::from(multiplier));
                self.scene.movement_speed_multiplier =
                    self.camera.movement_speed_multiplier() as f32;
                log::info!(
                    "XR movement speed multiplier set to {:.1}x",
                    self.camera.movement_speed_multiplier()
                );
            }
            GameUiAction::SetPlayerModel(model) => {
                self.player_model = model;
                log::info!("XR player model set to {}", model.label());
                if let Err(error) = self.sync_player_appearance() {
                    log::warn!("failed to sync XR player appearance: {error:#}");
                }
            }
            GameUiAction::Quit => {
                log::info!("XR menu quit action ignored by shared scene");
            }
            GameUiAction::OpenNewWorld => {
                let seed = self.next_new_world_seed();
                self.ui.set_new_world_seed(seed);
                self.session_status = StatusOverlay::hidden();
            }
            GameUiAction::OpenJoinRemote => {
                let remote_addr = self
                    .runtime
                    .as_ref()
                    .and_then(|runtime| runtime.active_session())
                    .and_then(|session| match session {
                        ActiveSessionDescriptor::Remote { endpoint } => {
                            Some(endpoint.address.clone())
                        }
                        ActiveSessionDescriptor::LocalWorld { .. } => None,
                    })
                    .unwrap_or_else(|| normalized_xr_remote_addr(self.ui.join_remote_addr()));
                self.ui.set_join_remote_addr(remote_addr);
                self.session_status = StatusOverlay::hidden();
            }
            GameUiAction::RerollSeed => {
                let seed = self.next_new_world_seed();
                self.ui.set_new_world_seed(seed);
                self.session_status = StatusOverlay::hidden();
                log::info!("XR new-world seed rerolled to {seed}");
            }
            GameUiAction::CreateWorld(seed) => {
                let request = SessionStartRequest::NewLocalWorld { seed };
                if self
                    .replace_session_for_request(device, queue, request)
                    .is_err()
                {
                    self.ui.set_new_world_seed(seed);
                    return Ok(false);
                }
                return Ok(false);
            }
            GameUiAction::JoinRemote => {
                let remote_addr = normalized_xr_remote_addr(self.ui.join_remote_addr());
                self.ui.set_join_remote_addr(remote_addr.clone());
                let request = SessionStartRequest::JoinRemote {
                    endpoint: RemoteSessionEndpoint::new(remote_addr),
                };
                if self
                    .replace_session_for_request(device, queue, request)
                    .is_err()
                {
                    return Ok(false);
                }
                scene_replaced = true;
            }
            GameUiAction::AssignHotbarBlock { slot, block_state } => {
                let changed = self.assign_debug_hotbar_slot(slot, BlockStateId(block_state))?;
                log::info!(
                    "XR debug hotbar slot {} assigned block_state={} changed={}",
                    slot + 1,
                    block_state,
                    changed
                );
            }
            GameUiAction::CycleFramePacing
            | GameUiAction::CycleFpsCap
            | GameUiAction::SetTouchLookSensitivity(_)
            | GameUiAction::SetTouchControlsMode(_)
            | GameUiAction::SetServerSimulationCadence(_) => {}
            GameUiAction::BackToTitle | GameUiAction::QuitToTitle => {
                self.session_status = StatusOverlay::hidden();
            }
            GameUiAction::StartWorld
            | GameUiAction::Resume
            | GameUiAction::OpenHelp(_)
            | GameUiAction::CloseHelp(_)
            | GameUiAction::OpenOptions(_)
            | GameUiAction::OpenServerSettings(_)
            | GameUiAction::BackToPause => {}
            GameUiAction::OpenBlockPalette => {
                self.menu_panel_anchor = XrUiPanelAnchor::LeftHand;
                self.menu_panel_pose = None;
                self.menu_panel_recenter_pending = true;
            }
        }
        self.ui.apply_action(action);
        if !self.ui.is_active() {
            self.clear_menu_input_state();
        }
        Ok(scene_replaced)
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
    diagnostics: RuntimePollDiagnostics,
) -> XrTerrainUploadSummary {
    XrTerrainUploadSummary {
        poll_total_ms: diagnostics.poll_total_ms,
        poll_drain_updates_ms: diagnostics.drain_updates_ms,
        poll_apply_updates_ms: diagnostics.apply_updates_ms,
        poll_dirty_mark_ms: diagnostics.dirty_mark_ms,
        poll_client_apply_updates_ms: diagnostics.client_apply_updates_ms,
        poll_diagnostics_ms: diagnostics.poll_diagnostics_ms,
        poll_diagnostics_refreshed: diagnostics.diagnostics_refreshed,
        poll_diagnostics_cache_age_ms: diagnostics.diagnostics_cache_age_ms,
        server_diagnostics_detail_refreshes: diagnostics.server_diagnostics_detail_refreshes,
        server_diagnostics_detail_age_ms: diagnostics.server_diagnostics_detail_age_ms,
        poll_server_tick_ms: diagnostics.server_tick_ms,
        poll_server_reported_total_ms: diagnostics.server_reported_total_ms,
        poll_scheduler_tick_ms: diagnostics.scheduler_tick_ms,
        poll_updates: diagnostics.updates,
        poll_snapshot_updates: diagnostics.snapshot_updates,
        poll_section_block_updates: diagnostics.section_block_updates,
        poll_unload_updates: diagnostics.unload_updates,
        server_command_queue_depth: diagnostics.server_command_queue_depth,
        server_update_queue_depth: diagnostics.server_update_queue_depth,
        server_pending_jobs: diagnostics.server_pending_jobs,
        server_pending_publications: diagnostics.server_pending_publications,
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
    let server_changed =
        sync_engine_camera_player_pose_for_runtime(runtime, camera).context(context)?;
    let interest_changed = update_interest_from_engine_camera_for_runtime(runtime, camera)?;
    Ok(server_changed || interest_changed)
}

fn sync_engine_camera_player_pose_for_runtime<S>(
    runtime: &mut NativeSingleViewSessionRuntime<S>,
    camera: &mut EngineCameraController,
) -> Result<bool>
where
    S: RemoteDedicatedServerSession,
{
    let changed = if let Some(report) = camera.next_pose_sync_command() {
        runtime
            .send_gameplay_command(report.command)
            .context("failed to sync XR terrain player pose to server")?
    } else {
        false
    };
    Ok(changed || apply_pending_engine_camera_position_updates_for_runtime(runtime, camera)?)
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
    let snapshot = camera.snapshot();
    let center = snapshot.chunk_pos;
    if runtime.set_interest_center(center)? {
        log::info!(
            "XR terrain chunk interest moved to ({}, {}) at camera position ({:.1}, {:.1}, {:.1})",
            center.x,
            center.z,
            snapshot.eye.x,
            snapshot.eye.y,
            snapshot.eye.z
        );
        return Ok(true);
    }
    Ok(false)
}

fn xr_game_ui_for_session(session: Option<&ActiveSessionDescriptor>, seed: i64) -> GameUiHost {
    let mut ui = GameUiHost::new();
    ui.set_new_world_seed(match session {
        Some(ActiveSessionDescriptor::LocalWorld { seed }) => *seed,
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

fn initial_xr_seed_reroll_state(seed: i64) -> u64 {
    (seed as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0xD1B5_4A32_D192_ED03)
        .max(1)
}

fn local_single_view_options(scene: XrSceneOptions) -> LocalSingleViewSceneOptions {
    LocalSingleViewSceneOptions::new(scene.seed, scene.center(), scene.render_distance)
        .with_initial_spawn_center()
        .with_day_time(scene.day_time_override)
        .with_freeze_time(scene.freeze_time)
        .with_debug_passive_showcase(scene.debug_passive_showcase)
        .with_lighting_enabled(scene.lighting_enabled)
        .with_render_compile_worker_count(scene.render_compile_worker_count)
}

fn active_session_label(session: Option<&ActiveSessionDescriptor>) -> String {
    match session {
        Some(ActiveSessionDescriptor::LocalWorld { seed }) => format!("local-world:{seed}"),
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
        EngineCameraMovementMode::NoClip => GameMovementMode::Fly,
        EngineCameraMovementMode::HandPush => GameMovementMode::HandPush,
    }
}

fn engine_movement_mode(mode: GameMovementMode) -> EngineCameraMovementMode {
    match mode {
        GameMovementMode::Walk => EngineCameraMovementMode::Walking,
        GameMovementMode::Fly => EngineCameraMovementMode::NoClip,
        GameMovementMode::HandPush => EngineCameraMovementMode::HandPush,
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
    use mclone_render_session::{
        ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND, ENGINE_CAMERA_MOUSE_SENSITIVITY,
    };

    fn point_in(rect: Rect) -> Point {
        Point {
            x: rect.center_x(),
            y: rect.y + rect.height * 0.5,
        }
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
            Some(&ActiveSessionDescriptor::LocalWorld { seed: 44 }),
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

        let first = prepare_xr_menu_panel_draw(&mut ui, gui_scale, state, None, &status);
        assert_eq!(first.draw_cache, UiDrawCacheStats::rebuild());
        assert!(first.cache_revision.is_some());
        assert!(!first.draw.commands().is_empty());

        let second = prepare_xr_menu_panel_draw(&mut ui, gui_scale, state, None, &status);
        assert_eq!(second.draw_cache, UiDrawCacheStats::cache_hit());
        assert_eq!(second.cache_revision, first.cache_revision);
        assert_eq!(second.draw, first.draw);

        let snapshot = ui.v2_debug_snapshot().expect("Controls has debug data");
        let back = snapshot
            .widgets
            .iter()
            .find(|widget| widget.label == "Back")
            .expect("Back button")
            .rect;
        ui.pointer_move(point_in(back));

        let hovered = prepare_xr_menu_panel_draw(&mut ui, gui_scale, state, None, &status);
        assert_eq!(hovered.draw_cache, UiDrawCacheStats::rebuild());
        assert_ne!(hovered.cache_revision, first.cache_revision);

        ui.pointer_move(Point {
            x: back.x + 2.0,
            y: back.y + 2.0,
        });
        let jitter = prepare_xr_menu_panel_draw(&mut ui, gui_scale, state, None, &status);
        assert_eq!(jitter.draw_cache, UiDrawCacheStats::cache_hit());
        assert_eq!(jitter.cache_revision, hovered.cache_revision);
        assert_eq!(jitter.draw, hovered.draw);
    }

    #[test]
    fn xr_menu_panel_draw_bypasses_revision_cache_for_transient_overlays() {
        let mut ui = GameUiHost::new_ingame();
        ui.set_screen(Some(GameScreen::Pause));
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        let state = GameUiRenderState::default();
        let visible_status = StatusOverlay::new("starting", true);

        let first = prepare_xr_menu_panel_draw(&mut ui, gui_scale, state, None, &visible_status);
        let second = prepare_xr_menu_panel_draw(&mut ui, gui_scale, state, None, &visible_status);

        assert_eq!(first.draw_cache, UiDrawCacheStats::rebuild());
        assert_eq!(second.draw_cache, UiDrawCacheStats::rebuild());
        assert_eq!(first.cache_revision, None);
        assert_eq!(second.cache_revision, None);
        assert!(!first.draw.commands().is_empty());
        assert!(!second.draw.commands().is_empty());
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
