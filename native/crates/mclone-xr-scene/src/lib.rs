#![forbid(unsafe_code)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use glam::{Quat, Vec2, Vec3};
use mclone_app_runtime::frame_render::{
    FullFrameGui, FullFrameRenderSummary, RenderStreamStats, record_render_section_update_stats,
    render_full_frame_for_view_with_prepared_records,
    render_full_frame_for_view_with_prepared_records_timed,
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
use mclone_app_runtime::{RuntimePollDiagnostics, elapsed_ms};
use mclone_audio::{AudioEngine, landing_playback_for_impact};
use mclone_client::{BlockInteractionTarget, ClientInteractionController};
use mclone_core::{ChunkPos, Vec3d, time};
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::actor_assets::ActorTextureImage;
use mclone_render::chunk::{
    ChunkDepthTarget, ChunkProjectionKind, ChunkRenderView, PreparedTexturedSectionRecords,
    TexturedSectionDrawResources, TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::entity::ActorDrawResources;
use mclone_render::gui::{WorldGuiLine, WorldGuiPanel, WorldGuiRenderer};
use mclone_render::selection_outline::{SelectionOutline, SelectionOutlineRenderer};
use mclone_render::sky::overworld_clear_color;
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_render_session::{
    ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MAX_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MIN_MOVEMENT_SPEED_MULTIPLIER, ENGINE_CAMERA_MOUSE_SENSITIVITY,
    EngineCameraController, EngineCameraInput, EngineCameraMovementImpulse,
    EngineCameraMovementMode, EngineCameraSnapshot, actor_instances_from_presentations,
};
use mclone_ui::{
    DEFAULT_JOIN_REMOTE_ADDR, GameFramePacingMode, GameUi, GameUiAction, GameUiRenderState,
    GuiScale, Point, StatusOverlay, render_loading_progress_overlay, render_status_overlay,
};
use mclone_xr_host::{XrControllerSnapshot, XrHand};
use openxr as xr;

pub const DEFAULT_XR_SEED: i64 = 12_345;
pub const DEFAULT_XR_CHUNK_X: i32 = 0;
pub const DEFAULT_XR_CHUNK_Z: i32 = 0;
pub const DEFAULT_XR_RENDER_DISTANCE: u32 = 2;
pub const MAX_XR_RENDER_DISTANCE: u32 = 16;
pub const XR_NEAR: f32 = 0.05;
pub const XR_FAR: f32 = 700.0;
pub const XR_JOYPAD_DEAD_ZONE: f32 = 0.18;
pub const XR_JOYPAD_YAW_SPEED_RADIANS_PER_SECOND: f64 = 1.6;
/// Minimum right-stick vertical deflection required to ascend/descend. Keeps the
/// radial dead-zone cross-talk from nudging the player up/down while turning.
pub const XR_JOYPAD_VERTICAL_THRESHOLD: f32 = 0.5;
pub const XR_LOCOMOTION_MAX_FRAME_SECONDS: f64 = 0.1;
pub const XR_MENU_TOGGLE_HAND: XrHand = XrHand::Left;
pub const XR_UI_FPS_CAP: u32 = 90;
pub const XR_MENU_PANEL_PIXELS: [u32; 2] = [1024, 576];
pub const XR_MENU_PANEL_DISTANCE_BLOCKS: f32 = 2.2;
pub const XR_MENU_PANEL_WIDTH_BLOCKS: f32 = 1.75;
pub const XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS: f32 = 6.0;
pub const XR_MENU_POINTER_TRIGGER_PRESS: f32 = 0.55;
pub const XR_MENU_POINTER_TRIGGER_RELEASE: f32 = 0.35;
pub const XR_GAMEPLAY_INTERACTION_HAND: XrHand = XrHand::Right;
const XR_FIXED_RENDER_EYE_SEPARATION_BLOCKS: f32 = 0.064;
const XR_MENU_LEFT_RAY_COLOR: [f32; 4] = [0.18, 0.85, 1.0, 0.95];
const XR_MENU_RIGHT_RAY_COLOR: [f32; 4] = [0.2, 1.0, 0.45, 0.95];
const XR_MENU_TRIGGER_RAY_COLOR: [f32; 4] = [1.0, 0.42, 0.12, 1.0];

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrSceneOptions {
    pub seed: i64,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub render_distance: u32,
    pub movement_speed_multiplier: f32,
    pub day_time_override: Option<u64>,
    pub freeze_time: bool,
    pub lighting_enabled: bool,
}

impl Default for XrSceneOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_XR_SEED,
            chunk_x: DEFAULT_XR_CHUNK_X,
            chunk_z: DEFAULT_XR_CHUNK_Z,
            render_distance: DEFAULT_XR_RENDER_DISTANCE,
            movement_speed_multiplier: ENGINE_CAMERA_BASE_MOVEMENT_SPEED_MULTIPLIER as f32,
            day_time_override: None,
            freeze_time: false,
            lighting_enabled: true,
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
    pub ui_active: bool,
    pub local_startup_active: bool,
    pub actor_count: usize,
    pub drawn_actor_count: usize,
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
    pub runtime_gpu_upload_ms: f64,
    pub runtime_ready_sections_ms: f64,
    pub shared_records_ms: f64,
    pub left_eye_ms: f64,
    pub right_eye_ms: f64,
    pub left_eye_render: XrTerrainEyeRenderTiming,
    pub right_eye_render: XrTerrainEyeRenderTiming,
    pub stereo_finish_ms: f64,
    pub stereo_submit_ms: f64,
    pub stereo_poll_wait_ms: f64,
    /// E1: real GPU execution time for the whole stereo frame (both eyes plus
    /// sky/UI/selection passes), measured via wgpu timestamp queries bracketing
    /// the stereo command encoder. Zero unless GPU timestamps are enabled and
    /// the OpenXR Vulkan adapter supports `TIMESTAMP_QUERY`.
    pub gpu_stereo_total_ms: f64,
    pub gpu_left_eye_ms: f64,
    pub gpu_right_eye_ms: f64,
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
    pub rebuilt_section_count: usize,
    pub removed_section_count: usize,
    pub rebuilt_vertex_count: u32,
    pub rebuilt_index_count: u32,
    pub neighbor_ready_section_count: usize,
    pub near_exception_section_count: usize,
    pub deferred_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub completed_compile_section_count: usize,
    pub stale_compile_section_count: usize,
    pub uploaded_section_count: usize,
    pub upload_removed_section_count: usize,
    pub uploaded_vertex_count: u32,
    pub uploaded_index_count: u32,
    pub traversal_ready_section_count: usize,
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

struct XrRenderedEye {
    summary: FullFrameRenderSummary,
    timing: XrTerrainEyeRenderTiming,
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

// E1 (docs/tactical/106): GPU-timestamp split of the stereo frame.
//
// `render_prepared_frame` records both eyes into one encoder, submits once, then
// blocks on `device.poll(WaitForSubmissionIndex)`. That poll wait (~10-11ms at
// frozen RD10) fuses real GPU execution with submit/acquire overhead, leaving an
// unresolved gap against the ~7ms Meta `app/gpu_frametime` counter. Bracketing
// the encoder with timestamp writes (before the left eye, between eyes, after the
// right eye — all downstream sky/UI/selection passes share the encoder) yields
// the true on-device GPU time plus a per-eye split.
//
// The frame already blocks on the submission before we read back, so the readback
// buffer is guaranteed populated and a single extra `poll(Wait)` drives the map
// callback. This is opt-in (`set_gpu_timestamps_enabled`); the normal headset
// loop never allocates the query set or pays the readback cost.
const XR_GPU_TIMESTAMP_COUNT: u32 = 3;
const XR_GPU_TIMESTAMP_RESOLVE_SIZE: u64 = 64;
const XR_GPU_TIMESTAMP_MAP_TIMEOUT: Duration = Duration::from_millis(100);

struct XrGpuStereoTimestamps {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    timestamp_period_ns: f32,
}

impl XrGpuStereoTimestamps {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        let required = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !device.features().contains(required) {
            return None;
        }
        let timestamp_period_ns = queue.get_timestamp_period();
        if !timestamp_period_ns.is_finite() || timestamp_period_ns <= 0.0 {
            return None;
        }
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("mclone_xr_stereo_gpu_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: XR_GPU_TIMESTAMP_COUNT,
        });
        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_xr_stereo_gpu_timestamp_resolve"),
            size: XR_GPU_TIMESTAMP_RESOLVE_SIZE,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mclone_xr_stereo_gpu_timestamp_readback"),
            size: XR_GPU_TIMESTAMP_RESOLVE_SIZE,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(Self {
            query_set,
            resolve_buffer,
            readback_buffer,
            timestamp_period_ns,
        })
    }

    /// Write a single timestamp into the encoder timeline at `index`.
    fn write(&self, encoder: &mut wgpu::CommandEncoder, index: u32) {
        encoder.write_timestamp(&self.query_set, index);
    }

    /// Resolve the timestamps and stage them for readback. Call once after the
    /// last write and before `encoder.finish()`.
    fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(
            &self.query_set,
            0..XR_GPU_TIMESTAMP_COUNT,
            &self.resolve_buffer,
            0,
        );
        encoder.copy_buffer_to_buffer(
            &self.resolve_buffer,
            0,
            &self.readback_buffer,
            0,
            XR_GPU_TIMESTAMP_RESOLVE_SIZE,
        );
    }

    /// Read back the resolved timestamps after the frame's GPU work has
    /// completed. Returns `(total_ms, left_eye_ms, right_eye_ms)`.
    fn collect(&self, device: &wgpu::Device) -> Option<(f64, f64, f64)> {
        let slice = self.readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        if let Err(err) = device.poll(wgpu::PollType::Wait) {
            log::warn!("XR GPU timestamp readback poll failed: {err:?}");
            return None;
        }
        match receiver.recv_timeout(XR_GPU_TIMESTAMP_MAP_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                log::warn!("XR GPU timestamp readback map failed: {err:?}");
                return None;
            }
            Err(err) => {
                log::warn!("XR GPU timestamp readback timed out: {err:?}");
                return None;
            }
        }
        let result = {
            let data = slice.get_mapped_range();
            match (
                read_xr_timestamp(&data, 0),
                read_xr_timestamp(&data, 8),
                read_xr_timestamp(&data, 16),
            ) {
                (Some(start), Some(mid), Some(end)) => Some((
                    xr_timestamp_delta_ms(start, end, self.timestamp_period_ns),
                    xr_timestamp_delta_ms(start, mid, self.timestamp_period_ns),
                    xr_timestamp_delta_ms(mid, end, self.timestamp_period_ns),
                )),
                _ => None,
            }
        };
        self.readback_buffer.unmap();
        result
    }
}

fn read_xr_timestamp(data: &[u8], offset: usize) -> Option<u64> {
    let bytes = data.get(offset..offset + std::mem::size_of::<u64>())?;
    Some(u64::from_ne_bytes(bytes.try_into().ok()?))
}

fn xr_timestamp_delta_ms(start: u64, end: u64, period_ns: f32) -> f64 {
    if end <= start || !period_ns.is_finite() || period_ns <= 0.0 {
        return 0.0;
    }
    (end - start) as f64 * period_ns as f64 / 1_000_000.0
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
    draw: TexturedSectionDrawResources,
    actors: ActorDrawResources,
    selection_outline: SelectionOutlineRenderer,
    world_gui_renderer: WorldGuiRenderer,
    ui: GameUi,
    session_status: StatusOverlay,
    sky: SkyRenderer,
    render_stats: RenderStreamStats,
    tracking_origin: Option<XrTrackingOrigin>,
    locomotion_mode: XrLocomotionMode,
    display_refresh_hz: Option<f32>,
    render_split_timing_enabled: bool,
    gpu_timestamps_requested: bool,
    gpu_timestamps_init_attempted: bool,
    gpu_timestamps: Option<XrGpuStereoTimestamps>,
    last_locomotion_update: Option<Instant>,
    menu_toggle_down: bool,
    menu_pointer_down: bool,
    gameplay_interaction_buttons: XrGameplayInteractionButtons,
    menu_panel_pose: Option<WorldGuiPanel>,
    menu_panel_recenter_pending: bool,
    latest_controllers: Vec<XrControllerSnapshot>,
    first_eye_summary: Option<FullFrameRenderSummary>,
    rendered_frames: u32,
    audio: Option<AudioEngine>,
    seed_reroll_state: u64,
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
            draw,
            actors: ActorDrawResources::new(device, queue, color_format, actor_atlas.as_upload())
                .context("initialize XR terrain actor draw resources")?,
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            world_gui_renderer,
            ui,
            session_status: StatusOverlay::new(request.starting_message(), true),
            sky: SkyRenderer::new_with_color_profile(
                device,
                color_format,
                render_options.color_profile,
            ),
            render_stats: RenderStreamStats::default(),
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            display_refresh_hz: None,
            render_split_timing_enabled: false,
            gpu_timestamps_requested: false,
            gpu_timestamps_init_attempted: false,
            gpu_timestamps: None,
            last_locomotion_update: None,
            menu_toggle_down: false,
            menu_pointer_down: false,
            gameplay_interaction_buttons: XrGameplayInteractionButtons::default(),
            menu_panel_pose: None,
            menu_panel_recenter_pending: true,
            latest_controllers: Vec::new(),
            first_eye_summary: None,
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
            draw: started.draw,
            actors: ActorDrawResources::new(device, queue, color_format, actor_atlas.as_upload())
                .context("initialize XR terrain actor draw resources")?,
            selection_outline: SelectionOutlineRenderer::new(device, color_format),
            world_gui_renderer,
            ui,
            session_status: StatusOverlay::hidden(),
            sky: SkyRenderer::new_with_color_profile(
                device,
                color_format,
                render_options.color_profile,
            ),
            render_stats: started.render_stats,
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            display_refresh_hz: None,
            render_split_timing_enabled: false,
            gpu_timestamps_requested: false,
            gpu_timestamps_init_attempted: false,
            gpu_timestamps: None,
            last_locomotion_update: None,
            menu_toggle_down: false,
            menu_pointer_down: false,
            gameplay_interaction_buttons: XrGameplayInteractionButtons::default(),
            menu_panel_pose: None,
            menu_panel_recenter_pending: false,
            latest_controllers: Vec::new(),
            first_eye_summary: None,
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
        let upload = match runtime_mode {
            XrTerrainRuntimeUpdateMode::Live
                if self.local_startup.is_none() && self.runtime.is_some() =>
            {
                let runtime_upload_start = Instant::now();
                let upload = self.poll_runtime_and_upload(device, center_position, &mut timing)?;
                timing.runtime_upload_ms = elapsed_ms(runtime_upload_start.elapsed());
                upload
            }
            XrTerrainRuntimeUpdateMode::Live | XrTerrainRuntimeUpdateMode::Frozen => {
                self.frozen_runtime_upload_summary()
            }
        };
        let render_options = self.effective_render_options(center_position);
        let sky_clear_color = self.sky_clear_color();
        let time_of_day = self.time_of_day();
        let sun_angle = self.sun_angle();
        let actor_instances = self.runtime.as_ref().map_or_else(Vec::new, |runtime| {
            actor_instances_from_presentations(
                &runtime.client().actor_presentations(),
                runtime.client(),
            )
        });
        if self.gpu_timestamps_requested && !self.gpu_timestamps_init_attempted {
            self.gpu_timestamps_init_attempted = true;
            self.gpu_timestamps = XrGpuStereoTimestamps::new(device, queue);
            match self.gpu_timestamps.as_ref() {
                Some(ts) => log::info!(
                    "XR GPU timestamps enabled (timestamp_period={:.3}ns)",
                    ts.timestamp_period_ns
                ),
                None => log::warn!(
                    "XR GPU timestamps requested but TIMESTAMP_QUERY is unavailable on this OpenXR Vulkan device"
                ),
            }
        }
        // Move the profiler out of `self` for the duration of the encode so the
        // per-eye render calls can still borrow `&mut self`; restored after the
        // readback below.
        let gpu_timestamps = self.gpu_timestamps.take();
        let collect_split_timing = self.render_split_timing_enabled;
        let records_start = collect_split_timing.then(Instant::now);
        let prepared_records = self.draw.prepare_render_records();
        timing.shared_records_ms = records_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mclone_xr_terrain_stereo_encoder"),
        });
        if let Some(ts) = gpu_timestamps.as_ref() {
            ts.write(&mut encoder, 0);
        }
        let left_eye_start = Instant::now();
        let left_eye = self.render_eye_target(
            device,
            queue,
            &mut encoder,
            &prepared_records,
            left_target,
            render_views[0],
            &actor_instances,
            render_options,
            sky_clear_color,
            time_of_day,
            sun_angle,
            "left",
        )?;
        timing.left_eye_ms = elapsed_ms(left_eye_start.elapsed());
        timing.left_eye_render = left_eye.timing;
        if let Some(ts) = gpu_timestamps.as_ref() {
            ts.write(&mut encoder, 1);
        }
        let right_eye_start = Instant::now();
        let right_eye = self.render_eye_target(
            device,
            queue,
            &mut encoder,
            &prepared_records,
            right_target,
            render_views[1],
            &actor_instances,
            render_options,
            sky_clear_color,
            time_of_day,
            sun_angle,
            "right",
        )?;
        timing.right_eye_ms = elapsed_ms(right_eye_start.elapsed());
        timing.right_eye_render = right_eye.timing;
        if let Some(ts) = gpu_timestamps.as_ref() {
            ts.write(&mut encoder, 2);
            ts.resolve(&mut encoder);
        }
        let finish_start = collect_split_timing.then(Instant::now);
        let command_buffer = encoder.finish();
        timing.stereo_finish_ms = finish_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let submit_start = collect_split_timing.then(Instant::now);
        let submission = queue.submit(Some(command_buffer));
        timing.stereo_submit_ms = submit_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        let poll_start = collect_split_timing.then(Instant::now);
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .map(|_| ())
            .context("wait for XR terrain stereo render submission")?;
        timing.stereo_poll_wait_ms = poll_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
        // The submission has completed (the poll above blocked on it), so the
        // resolved timestamps are ready; collect them and restore the profiler.
        if let Some(ts) = gpu_timestamps.as_ref() {
            if let Some((total_ms, left_ms, right_ms)) = ts.collect(device) {
                timing.gpu_stereo_total_ms = total_ms;
                timing.gpu_left_eye_ms = left_ms;
                timing.gpu_right_eye_ms = right_ms;
            }
        }
        self.gpu_timestamps = gpu_timestamps;
        self.record_eye0_summary(left_eye.summary);
        Ok(self.frame_summary_with_timing(timing, upload))
    }

    pub fn set_locomotion_mode(&mut self, locomotion_mode: XrLocomotionMode) {
        self.locomotion_mode = locomotion_mode;
    }

    pub fn locomotion_mode(&self) -> XrLocomotionMode {
        self.locomotion_mode
    }

    pub fn set_display_refresh_hz(&mut self, display_refresh_hz: Option<f32>) {
        self.display_refresh_hz = display_refresh_hz.filter(|hz| hz.is_finite() && *hz > 0.0);
    }

    pub fn set_render_split_timing_enabled(&mut self, enabled: bool) {
        self.render_split_timing_enabled = enabled;
    }

    /// E1 (docs/tactical/106): opt into GPU-timestamp instrumentation of the
    /// stereo render. The query set is created lazily on the first rendered
    /// frame (when the device/queue are in hand) and is a no-op when the OpenXR
    /// Vulkan adapter does not advertise `TIMESTAMP_QUERY`.
    pub fn set_gpu_timestamps_enabled(&mut self, enabled: bool) {
        self.gpu_timestamps_requested = enabled;
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
        let ui_active = self.ui.is_active();
        let gameplay_interaction_edges = self.update_gameplay_interaction_buttons(controllers);
        let suppress_gameplay_interaction = ui_was_active != ui_active;
        if self.ui.is_active() {
            return Ok(());
        }
        if self.runtime.is_none() {
            return Ok(());
        }
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians(&views)
            .context("resolve XR locomotion frame")?;
        let input =
            xr_locomotion_input_from_controllers(controllers, dt_seconds, movement_yaw_radians);
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
        self.menu_panel_recenter_pending = false;
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

    pub fn apply_automated_stationary_input(&mut self) {
        self.latest_controllers.clear();
        if self.local_startup.is_some() || self.runtime.is_none() {
            return;
        }
        self.ui.close();
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_pose = None;
        self.menu_panel_recenter_pending = false;
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
                ui_active: self.ui.is_active(),
                local_startup_active: self.local_startup.is_some(),
                actor_count: summary.actor_count,
                drawn_actor_count: summary.drawn_actor_count,
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
            ui_active: self.ui.is_active(),
            local_startup_active: self.local_startup.is_some(),
            actor_count: self.render_stats.actor_count,
            drawn_actor_count: self.render_stats.drawn_actor_count,
            timing,
            upload,
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

    fn locomotion_movement_yaw_radians(&mut self, views: &[xr::View]) -> Result<Option<f64>> {
        match self.locomotion_mode {
            XrLocomotionMode::PlayerYaw => Ok(None),
            XrLocomotionMode::HeadsetYaw => {
                let tracking_origin = self.tracking_origin_for_views(views)?;
                let transform =
                    XrStageToWorld::from_tracking_origin(tracking_origin, self.camera.snapshot())?;
                xr_headset_world_yaw_from_views(views, transform).map(|yaw| Some(f64::from(yaw)))
            }
        }
    }

    fn poll_runtime_and_upload(
        &mut self,
        device: &wgpu::Device,
        camera_position: Vec3,
        timing: &mut XrTerrainFrameTiming,
    ) -> Result<XrTerrainUploadSummary> {
        let (pending_render_chunks_before, pending_compile_jobs_before) =
            if let Some(runtime) = self.runtime.as_ref() {
                (
                    runtime.pending_render_chunk_count(),
                    runtime.render_compile_pending_job_count(),
                )
            } else {
                return Ok(XrTerrainUploadSummary {
                    traversal_ready_section_count: self.draw.traversal_ready_section_count(),
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
        if !poll_changed && !runtime.has_pending_render_work(camera_position) {
            let ready_start = Instant::now();
            let ready_sections = runtime.traversal_ready_render_section_keys(camera_position);
            timing.runtime_ready_sections_ms = elapsed_ms(ready_start.elapsed());
            let traversal_ready_section_count = ready_sections.len();
            self.draw.set_traversal_ready_sections(&ready_sections);
            return Ok(XrTerrainUploadSummary {
                poll_changed,
                pending_render_chunks_before,
                pending_render_chunks_after: runtime.pending_render_chunk_count(),
                pending_compile_jobs_before,
                pending_compile_jobs_after: runtime.render_compile_pending_job_count(),
                traversal_ready_section_count,
                ..poll_summary
            });
        }
        let sync_start = Instant::now();
        let section_update = self
            .sync_render_sections(camera_position)
            .context("sync XR terrain render sections")?;
        timing.runtime_sync_ms = elapsed_ms(sync_start.elapsed());
        let upload_start = Instant::now();
        let upload_report = self
            .draw
            .apply_section_updates(
                device,
                &section_update.rebuilt_sections,
                &section_update.removed_section_keys,
            )
            .context("upload XR terrain render section updates")?;
        timing.runtime_gpu_upload_ms = elapsed_ms(upload_start.elapsed());
        let ready_start = Instant::now();
        let runtime = self
            .runtime
            .as_ref()
            .expect("runtime presence checked before section sync");
        let ready_sections = runtime.traversal_ready_render_section_keys(camera_position);
        timing.runtime_ready_sections_ms = elapsed_ms(ready_start.elapsed());
        let traversal_ready_section_count = ready_sections.len();
        self.draw.set_traversal_ready_sections(&ready_sections);
        self.render_stats.section_count = self.draw.section_count();
        self.render_stats.index_count = self.draw.index_count();
        self.render_stats.face_count = quad_face_count_from_indices(self.render_stats.index_count);
        record_render_section_update_stats(&mut self.render_stats, &section_update, upload_report);
        Ok(XrTerrainUploadSummary {
            poll_changed,
            pending_render_chunks_before,
            pending_render_chunks_after: runtime.pending_render_chunk_count(),
            pending_compile_jobs_before,
            pending_compile_jobs_after: runtime.render_compile_pending_job_count(),
            rebuilt_section_count: section_update.rebuilt_section_count(),
            removed_section_count: section_update.removed_section_count(),
            rebuilt_vertex_count: section_update.rebuilt_vertex_count,
            rebuilt_index_count: section_update.rebuilt_index_count,
            neighbor_ready_section_count: section_update.neighbor_ready_section_count,
            near_exception_section_count: section_update.near_exception_section_count,
            deferred_section_count: section_update.deferred_section_count,
            submitted_compile_section_count: section_update.submitted_compile_section_count,
            completed_compile_section_count: section_update.completed_compile_section_count,
            stale_compile_section_count: section_update.stale_compile_section_count,
            uploaded_section_count: upload_report.uploaded_section_count,
            upload_removed_section_count: upload_report.removed_section_count,
            uploaded_vertex_count: upload_report.uploaded_vertex_count,
            uploaded_index_count: upload_report.uploaded_index_count,
            traversal_ready_section_count,
            visibility_graph_build_count: section_update.visibility_graph_stats.build_count,
            visibility_graph_total_ms: section_update.visibility_graph_stats.total_ms,
            visibility_graph_worst_ms: section_update.visibility_graph_stats.worst_ms,
            ..poll_summary
        })
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
        XrTerrainUploadSummary {
            pending_render_chunks_before: pending_render_chunks,
            pending_render_chunks_after: pending_render_chunks,
            pending_compile_jobs_before: pending_compile_jobs,
            pending_compile_jobs_after: pending_compile_jobs,
            traversal_ready_section_count: self.draw.traversal_ready_section_count(),
            ..XrTerrainUploadSummary::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_eye_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        prepared_records: &PreparedTexturedSectionRecords,
        target: XrTerrainEyeTarget<'_>,
        render_view: ChunkRenderView,
        actor_instances: &[mclone_render::entity::ActorInstance],
        render_options: TexturedSectionRenderOptions,
        sky_clear_color: wgpu::Color,
        time_of_day: f32,
        sun_angle: f32,
        label: &'static str,
    ) -> Result<XrRenderedEye> {
        let collect_split_timing = self.render_split_timing_enabled;
        let encode_start = collect_split_timing.then(Instant::now);
        let frame = RenderFrameContext::new(
            device,
            queue,
            &mut *encoder,
            RenderFrameTarget::color(target.color_view, target.size),
        );
        let gui_scale = GuiScale::from_pixels(XR_MENU_PANEL_PIXELS[0], XR_MENU_PANEL_PIXELS[1]);
        self.ui.set_scale(gui_scale);
        let ui_state = self.current_ui_render_state();
        let ui_active = self.ui.is_active();
        let mut ui_draw = self.ui.render_draw_list(ui_state);
        if let Some(progress) = self
            .local_startup
            .as_ref()
            .and_then(|startup| startup.pump.progress_overlay())
        {
            render_loading_progress_overlay(gui_scale, &mut ui_draw, &progress);
        }
        render_status_overlay(gui_scale, &mut ui_draw, &self.session_status);
        let summary_ui_draw = ui_draw.clone();
        let selection_outline = self.current_xr_selection_outline();
        let mut render_stats = self.render_stats;
        let (summary, frame_timing) = if collect_split_timing {
            render_full_frame_for_view_with_prepared_records_timed(
                frame,
                target.depth,
                &self.sky,
                &mut self.draw,
                prepared_records,
                Some(&mut self.actors),
                None,
                None,
                render_view,
                actor_instances,
                None,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                |_| summary_ui_draw,
                &mut render_stats,
            )
        } else {
            render_full_frame_for_view_with_prepared_records(
                frame,
                target.depth,
                &self.sky,
                &mut self.draw,
                prepared_records,
                Some(&mut self.actors),
                None,
                None,
                render_view,
                actor_instances,
                None,
                sky_clear_color,
                time_of_day,
                sun_angle,
                render_options,
                FullFrameGui::new(false, false, [gui_scale.width, gui_scale.height]),
                |_| summary_ui_draw,
                &mut render_stats,
            )
            .map(|summary| (summary, Default::default()))
        }
        .with_context(|| format!("render XR terrain {label} eye"))?;
        self.selection_outline.render(
            device,
            queue,
            &mut *encoder,
            RenderFrameTarget::color(target.color_view, target.size),
            target.depth,
            render_view,
            selection_outline.as_ref(),
        );
        if ui_active {
            if let Some(panel) = self.menu_panel_pose {
                let controller_ray_lines = self
                    .xr_menu_controller_ray_lines(panel)
                    .context("build XR menu controller ray visuals")?;
                self.world_gui_renderer
                    .render_panel(
                        device,
                        queue,
                        &mut *encoder,
                        RenderFrameTarget::color(target.color_view, target.size),
                        render_view,
                        XR_MENU_PANEL_PIXELS,
                        [gui_scale.width, gui_scale.height],
                        &ui_draw,
                        panel,
                        &controller_ray_lines,
                    )
                    .with_context(|| format!("render XR menu panel for {label} eye"))?;
            }
        }
        let encode_total_ms = encode_start.map_or(0.0, |start| elapsed_ms(start.elapsed()));
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
                submit_ms: 0.0,
                poll_wait_ms: 0.0,
            },
        })
    }

    fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<mclone_render_session::RenderSectionCacheUpdate> {
        self.runtime
            .as_mut()
            .context("XR terrain runtime is not active")?
            .sync_render_sections(camera_position)
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
        GameUiRenderState {
            render_distance: (render_distance as i32).clamp(1, MAX_XR_RENDER_DISTANCE as i32),
            min_render_distance: 1,
            max_render_distance: MAX_XR_RENDER_DISTANCE as i32,
            section_occlusion_culling: self.render_options.section_occlusion_culling,
            force_fullbright: self.render_options.force_fullbright,
            fly_enabled: self.camera.movement_mode() == EngineCameraMovementMode::NoClip,
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
            touch_controls_mode: None,
            touch_settings: None,
            block_palette: Default::default(),
        }
    }

    fn apply_menu_toggle_input(&mut self, controllers: &[XrControllerSnapshot]) {
        let toggle_down = xr_menu_toggle_pressed(controllers);
        if toggle_down && !self.menu_toggle_down {
            if self.local_startup.is_some() {
                if !self.ui.is_active() {
                    self.ui.open_pause();
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
                self.menu_panel_recenter_pending = true;
                log::info!("XR menu opened");
            }
        }
        self.menu_toggle_down = toggle_down;
    }

    fn update_menu_panel_pose(&mut self, render_views: [ChunkRenderView; 2]) {
        if !self.ui.is_active() {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            self.menu_panel_pose = None;
            self.menu_panel_recenter_pending = false;
            return;
        }
        if self.menu_panel_pose.is_some() && !self.menu_panel_recenter_pending {
            return;
        }
        self.menu_panel_pose = Some(xr_menu_panel_from_render_views(render_views));
        self.menu_panel_recenter_pending = false;
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
        let ui_state = self.current_ui_render_state();
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
            self.ui.pointer_down(hit.point, ui_state);
            None
        } else if !trigger_down && self.menu_pointer_down {
            let (_handled, action) = self.ui.pointer_up(hit.point, ui_state);
            action
        } else {
            let (_handled, action) = self.ui.pointer_move(hit.point, ui_state);
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
        self.menu_panel_recenter_pending = true;
    }

    fn clear_menu_input_state(&mut self) {
        self.ui.clear_input();
        self.menu_pointer_down = false;
        self.menu_panel_pose = None;
        self.menu_panel_recenter_pending = false;
    }

    fn clear_transient_world_state(&mut self) {
        self.tracking_origin = None;
        self.last_locomotion_update = None;
        self.latest_controllers.clear();
        self.first_eye_summary = None;
        self.rendered_frames = 0;
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
            GameUiAction::ToggleFly => {
                let movement_mode = self.camera.toggle_movement_mode();
                log::info!("XR player movement mode {}", movement_mode.label());
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
            GameUiAction::CycleFramePacing
            | GameUiAction::CycleFpsCap
            | GameUiAction::SetTouchLookSensitivity(_)
            | GameUiAction::SetTouchControlsMode(_)
            | GameUiAction::AssignHotbarBlock { .. } => {}
            GameUiAction::BackToTitle | GameUiAction::QuitToTitle => {
                self.session_status = StatusOverlay::hidden();
            }
            GameUiAction::StartWorld
            | GameUiAction::Resume
            | GameUiAction::OpenBlockPalette
            | GameUiAction::OpenOptions(_)
            | GameUiAction::BackToPause => {}
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

fn xr_game_ui_for_session(session: Option<&ActiveSessionDescriptor>, seed: i64) -> GameUi {
    let mut ui = GameUi::new();
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
        .with_lighting_enabled(scene.lighting_enabled)
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
    let dt_seconds = if dt_seconds.is_finite() {
        dt_seconds.clamp(0.0, XR_LOCOMOTION_MAX_FRAME_SECONDS)
    } else {
        0.0
    };
    let left_axis = controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Left)
        .map(|controller| joypad_axis_after_dead_zone(controller.thumbstick))
        .unwrap_or(Vec2::ZERO);
    let right_axis = controllers
        .iter()
        .find(|controller| controller.hand == XrHand::Right)
        .map(|controller| joypad_axis_after_dead_zone(controller.thumbstick))
        .unwrap_or(Vec2::ZERO);
    // Right stick: X turns (look left/right), Y flies up/down. Push up to ascend,
    // pull down to descend.
    let jump = right_axis.y > XR_JOYPAD_VERTICAL_THRESHOLD;
    let descend = right_axis.y < -XR_JOYPAD_VERTICAL_THRESHOLD;
    let movement_impulse = (left_axis.length_squared() > f32::EPSILON)
        .then(|| xr_left_stick_movement_impulse(left_axis));
    let yaw_delta = f64::from(right_axis.x) * XR_JOYPAD_YAW_SPEED_RADIANS_PER_SECOND * dt_seconds;
    let mouse_delta_x = if ENGINE_CAMERA_MOUSE_SENSITIVITY > 0.0 {
        yaw_delta / ENGINE_CAMERA_MOUSE_SENSITIVITY
    } else {
        0.0
    };

    EngineCameraInput {
        dt_seconds,
        mouse_delta_x,
        jump,
        descend,
        movement_impulse,
        movement_yaw_radians,
        ..EngineCameraInput::default()
    }
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

pub fn xr_menu_toggle_pressed(controllers: &[XrControllerSnapshot]) -> bool {
    controllers
        .iter()
        .any(|controller| controller.hand == XR_MENU_TOGGLE_HAND && controller.select_pressed)
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

    #[test]
    fn default_scene_options_match_desktop_xr_smoke_defaults() {
        let options = XrSceneOptions::default();
        assert_eq!(options.seed, 12_345);
        assert_eq!(options.center(), ChunkPos::new(0, 0));
        assert_eq!(options.render_distance, 2);
    }

    #[test]
    fn startup_view_pose_alignment_mode_is_explicit() {
        assert_eq!(XrViewAlignmentMode::PlayerSpawn.label(), "player-spawn");
        assert_eq!(XrViewAlignmentMode::ViewPose.label(), "view-pose");
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
    fn xr_locomotion_maps_left_stick_and_right_stick_ascend_to_engine_input() {
        let input = xr_locomotion_input_from_controllers(
            &[
                test_controller(XrHand::Left, Vec2::new(0.0, 1.0), false),
                test_controller(XrHand::Right, Vec2::new(0.0, 1.0), false),
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
    fn xr_locomotion_maps_right_stick_to_desktop_mouse_turn_path() {
        let input = xr_locomotion_input_from_controllers(
            &[test_controller(XrHand::Right, Vec2::new(1.0, 0.0), false)],
            0.05,
            None,
        );
        let expected_mouse_delta =
            XR_JOYPAD_YAW_SPEED_RADIANS_PER_SECOND * 0.05 / ENGINE_CAMERA_MOUSE_SENSITIVITY;

        assert!((input.mouse_delta_x - expected_mouse_delta).abs() < 1.0e-6);
        assert_eq!(input.movement_impulse, None);
        assert!(!input.jump);
    }

    #[test]
    fn xr_locomotion_maps_right_stick_up_to_jump() {
        let right = test_controller(XrHand::Right, Vec2::new(0.0, 1.0), false);
        let input = xr_locomotion_input_from_controllers(&[right], 1.0 / 72.0, None);

        assert!(input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn xr_locomotion_maps_right_stick_down_to_descend() {
        let right = test_controller(XrHand::Right, Vec2::new(0.0, -1.0), false);
        let input = xr_locomotion_input_from_controllers(&[right], 1.0 / 72.0, None);

        assert!(input.descend);
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
