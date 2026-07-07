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
    TraversalReadySectionCache, debug_block_palette_overlay, elapsed_ms, micros_to_ms,
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
use mclone_diagnostics::{BudgetDecisionPanelReport, FramePipelineReport};
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

mod comfort;
mod diagnostic_panel;
mod frame_pipeline_reporter;
mod locomotion;
mod options;
mod session;
mod teleport;
mod timing;
mod tracking;
mod ui_panels;

pub use comfort::*;
pub use locomotion::*;
pub use options::*;
pub use session::*;
pub(crate) use teleport::*;
pub use timing::*;
pub use tracking::*;
pub use ui_panels::*;

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

impl<S> XrMcloneTerrainState<S>
where
    S: RemoteDedicatedServerSession,
{
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
            let compile_health = runtime.render_compile_queue_health();
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
        let compile_health = runtime.render_compile_queue_health();
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
        let compile_health = self
            .runtime
            .as_ref()
            .map(|runtime| runtime.render_compile_queue_health());
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
            upload_held_lifecycle_item_count: upload_queue.held_release_lifecycle_items,
            upload_held_compile_job_count: upload_queue.held_compile_jobs,
            traversal_ready_section_count: self.draw.traversal_ready_section_count(),
            record_cache: self.draw.record_cache_stats(),
            ..XrTerrainUploadSummary::default()
        }
    }

    pub fn latest_budget_decision_panel(&self) -> BudgetDecisionPanelReport {
        self.runtime
            .as_ref()
            .map(|runtime| {
                runtime
                    .last_poll_diagnostics()
                    .scheduler_budget_decision_panel
            })
            .unwrap_or_default()
    }

    fn runtime_host_mode(&self) -> XrTerrainHostMode {
        self.runtime
            .as_ref()
            .map_or(XrTerrainHostMode::LocalIntegrated, |runtime| {
                runtime.host_mode().into()
            })
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
    fn xr_locomotion_maps_right_b_to_descend() {
        let mut right = test_controller(XrHand::Right, Vec2::ZERO, false);
        right.b_pressed = true;
        let input = xr_locomotion_input_from_controllers(&[right], 1.0 / 72.0, None);

        assert!(input.descend);
        assert!(!input.jump);
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
            b_pressed: false,
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
