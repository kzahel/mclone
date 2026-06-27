#![forbid(unsafe_code)]

use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use glam::{Quat, Vec2, Vec3};
use mclone_app_runtime::elapsed_ms;
use mclone_app_runtime::frame_render::{
    FullFrameGui, FullFrameRenderSummary, RenderStreamStats, record_render_section_update_stats,
    render_full_frame_for_view,
};
use mclone_app_runtime::host_mode::RemoteDedicatedServerSession;
use mclone_app_runtime::local_single_view::{
    LocalSingleViewSceneOptions, NativeSingleViewSceneRuntime,
};
use mclone_app_runtime::render_assets::TexturedMeshAssets;
use mclone_audio::{AudioEngine, landing_playback_for_impact};
use mclone_core::{ChunkPos, Vec3d};
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::actor_assets::ActorTextureImage;
use mclone_render::chunk::{
    ChunkDepthTarget, ChunkRenderView, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
use mclone_render::entity::ActorDrawResources;
use mclone_render::gui::{WorldGuiLine, WorldGuiPanel, WorldGuiRenderer};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_render_session::{
    ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER, ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER,
    ENGINE_CAMERA_MOUSE_SENSITIVITY, EngineCameraController, EngineCameraInput,
    EngineCameraMovementImpulse, EngineCameraMovementMode, EngineCameraSnapshot,
    actor_instances_from_presentations,
};
use mclone_ui::{GameFramePacingMode, GameUi, GameUiAction, GameUiRenderState, GuiScale, Point};
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
pub const XR_LOCOMOTION_MAX_FRAME_SECONDS: f64 = 0.1;
pub const XR_MENU_TOGGLE_HAND: XrHand = XrHand::Left;
pub const XR_UI_FPS_CAP: u32 = 90;
pub const XR_MENU_PANEL_PIXELS: [u32; 2] = [1024, 576];
pub const XR_MENU_PANEL_DISTANCE_BLOCKS: f32 = 2.2;
pub const XR_MENU_PANEL_WIDTH_BLOCKS: f32 = 1.75;
pub const XR_MENU_CONTROLLER_RAY_LENGTH_BLOCKS: f32 = 6.0;
pub const XR_MENU_POINTER_TRIGGER_PRESS: f32 = 0.55;
pub const XR_MENU_POINTER_TRIGGER_RELEASE: f32 = 0.35;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XrSceneOptions {
    pub seed: i64,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub render_distance: u32,
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
    pub actor_count: usize,
    pub drawn_actor_count: usize,
}

#[derive(Clone, Copy)]
pub struct XrTerrainEyeTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth: &'a ChunkDepthTarget,
    pub size: [u32; 2],
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
    runtime: NativeSingleViewSceneRuntime<S>,
    camera: EngineCameraController,
    initial_alignment_mode: XrViewAlignmentMode,
    render_options: TexturedSectionRenderOptions,
    draw: TexturedSectionDrawResources,
    actors: ActorDrawResources,
    world_gui_renderer: WorldGuiRenderer,
    ui: GameUi,
    sky: SkyRenderer,
    render_stats: RenderStreamStats,
    tracking_origin: Option<XrTrackingOrigin>,
    locomotion_mode: XrLocomotionMode,
    last_locomotion_update: Option<Instant>,
    menu_toggle_down: bool,
    menu_pointer_down: bool,
    menu_panel_pose: Option<WorldGuiPanel>,
    menu_panel_recenter_pending: bool,
    latest_controllers: Vec<XrControllerSnapshot>,
    first_eye_summary: Option<FullFrameRenderSummary>,
    rendered_frames: u32,
    audio: Option<AudioEngine>,
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
        let runtime = NativeSingleViewSceneRuntime::local_with_mesh_assets(
            local_single_view_options(scene),
            mesh_assets,
        );
        Self::with_runtime(
            device,
            queue,
            color_format,
            runtime.context("start XR local integrated scene runtime")?,
            render_options,
            actor_atlas,
            startup_view_pose,
        )
    }
}

impl<S> XrMcloneTerrainState<S>
where
    S: RemoteDedicatedServerSession,
{
    pub fn with_runtime(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        runtime: NativeSingleViewSceneRuntime<S>,
        render_options: TexturedSectionRenderOptions,
        actor_atlas: ActorTextureImage,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let center = runtime.interest_center();
        let render_distance = runtime.render_distance();
        let host_label = runtime.host_label();
        let draw = TexturedSectionDrawResources::new(
            device,
            queue,
            color_format,
            &[],
            runtime.mesh_assets().atlas.as_upload(),
        )
        .context("create empty XR terrain draw resources")?;
        let mut state = Self {
            runtime,
            camera: EngineCameraController::spawn_for_chunk(center),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            draw,
            actors: ActorDrawResources::new(device, queue, color_format, actor_atlas.as_upload())
                .context("initialize XR terrain actor draw resources")?,
            world_gui_renderer: WorldGuiRenderer::new(device, color_format),
            ui: GameUi::new_ingame(),
            sky: SkyRenderer::new(device, color_format),
            render_stats: RenderStreamStats::default(),
            tracking_origin: None,
            locomotion_mode: XrLocomotionMode::default(),
            last_locomotion_update: None,
            menu_toggle_down: false,
            menu_pointer_down: false,
            menu_panel_pose: None,
            menu_panel_recenter_pending: false,
            latest_controllers: Vec::new(),
            first_eye_summary: None,
            rendered_frames: 0,
            audio: None,
        };

        let initial_poll_start = Instant::now();
        let (initial_poll_count, initial_poll_ms) = state
            .poll_until_idle()
            .context("wait for initial XR terrain chunks")?;
        let mut initial_pose_changed = state
            .apply_pending_engine_camera_position_updates()
            .context("accept initial XR terrain player pose")?;
        if let Some(view_pose) = startup_view_pose {
            state
                .apply_startup_view_pose(view_pose)
                .context("apply XR terrain startup view pose")?;
            initial_pose_changed |= state
                .commit_engine_camera_player_pose()
                .context("sync XR terrain startup view pose")?;
        } else {
            initial_pose_changed |= state
                .commit_engine_camera_player_pose()
                .context("sync initial XR terrain player pose")?;
        }
        if initial_pose_changed {
            let _ = state
                .poll_until_idle()
                .context("wait for XR terrain chunks after player pose")?;
        }

        let camera_position = glam_vec3_from_vec3d(state.camera.snapshot().eye);
        let section_update = state
            .sync_all_render_sections(camera_position)
            .context("compile initial XR terrain render sections")?;
        let sections = state.runtime.cached_sections();
        if sections.is_empty() {
            bail!(
                "XR terrain host={} center=({}, {}) render_distance={} produced no render sections",
                host_label,
                center.x,
                center.z,
                render_distance
            );
        }
        state.draw = TexturedSectionDrawResources::new(
            device,
            queue,
            color_format,
            &sections,
            state.runtime.mesh_assets().atlas.as_upload(),
        )
        .context("upload initial XR terrain render sections")?;
        state.draw.set_traversal_ready_sections(
            &state
                .runtime
                .traversal_ready_render_section_keys(camera_position),
        );

        let initial_upload = TexturedSectionUploadReport {
            uploaded_section_count: section_update.rebuilt_section_count(),
            removed_section_count: section_update.removed_section_count(),
            uploaded_vertex_count: section_update.rebuilt_vertex_count,
            uploaded_index_count: section_update.rebuilt_index_count,
        };
        state.render_stats = RenderStreamStats {
            section_count: state.draw.section_count(),
            index_count: state.draw.index_count(),
            face_count: quad_face_count_from_indices(state.draw.index_count()),
            ..RenderStreamStats::default()
        };
        record_render_section_update_stats(
            &mut state.render_stats,
            &section_update,
            initial_upload,
        );
        log::info!(
            "mclone XR terrain runtime: host={} center=({}, {}) render_distance={} chunks={} sections={} faces={} indices={} initial_polls={} poll_ms={:.3} elapsed_ms={:.3}",
            host_label,
            center.x,
            center.z,
            render_distance,
            state.runtime.loaded_chunk_count(),
            state.render_stats.section_count,
            state.render_stats.face_count,
            state.render_stats.index_count,
            initial_poll_count,
            initial_poll_ms,
            elapsed_ms(initial_poll_start.elapsed())
        );
        Ok(state)
    }

    pub fn set_audio_engine(&mut self, audio: Option<AudioEngine>) {
        self.audio = audio;
    }

    pub fn render_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [xr::View; 2],
        left_target: XrTerrainEyeTarget<'_>,
        right_target: XrTerrainEyeTarget<'_>,
    ) -> Result<XrTerrainFrameSummary> {
        let render_views = self.render_views(&views)?;
        let center_position =
            (render_views[0].camera_position + render_views[1].camera_position) * 0.5;
        self.update_menu_panel_pose(render_views);
        self.apply_menu_pointer_input()
            .context("apply XR menu pointer input")?;
        self.poll_runtime_and_upload(device, center_position)?;
        let render_options = self.effective_render_options(center_position);
        let sky_clear_color = self.sky_clear_color();
        let time_of_day = self.runtime.time_of_day();
        let sun_angle = self.runtime.sun_angle();
        let actor_instances = actor_instances_from_presentations(
            &self.runtime.client().actor_presentations(),
            self.runtime.client(),
        );
        let left_summary = self.render_eye_target(
            device,
            queue,
            left_target,
            render_views[0],
            &actor_instances,
            render_options,
            sky_clear_color,
            time_of_day,
            sun_angle,
            "left",
        )?;
        self.render_eye_target(
            device,
            queue,
            right_target,
            render_views[1],
            &actor_instances,
            render_options,
            sky_clear_color,
            time_of_day,
            sun_angle,
            "right",
        )?;
        self.record_eye0_summary(left_summary);
        Ok(self.frame_summary())
    }

    pub fn set_locomotion_mode(&mut self, locomotion_mode: XrLocomotionMode) {
        self.locomotion_mode = locomotion_mode;
    }

    pub fn locomotion_mode(&self) -> XrLocomotionMode {
        self.locomotion_mode
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
        self.apply_menu_toggle_input(controllers);
        if self.ui.is_active() {
            return Ok(());
        }
        let movement_yaw_radians = self
            .locomotion_movement_yaw_radians(&views)
            .context("resolve XR locomotion frame")?;
        let input =
            xr_locomotion_input_from_controllers(controllers, dt_seconds, movement_yaw_radians);
        self.camera
            .apply_movement_input(self.runtime.client(), input);
        self.play_landing_events();
        self.commit_engine_camera_player_pose()
            .context("sync XR locomotion player pose")?;
        Ok(())
    }

    pub fn frame_summary(&self) -> XrTerrainFrameSummary {
        if let Some(summary) = self.first_eye_summary {
            return XrTerrainFrameSummary {
                rendered_frames: self.rendered_frames,
                section_count: summary.section_count,
                drawn_section_count: summary.drawn_section_count,
                index_count: summary.index_count,
                drawn_index_count: summary.drawn_index_count,
                gui_command_count: summary.gui_command_count,
                ui_active: self.ui.is_active(),
                actor_count: summary.actor_count,
                drawn_actor_count: summary.drawn_actor_count,
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
            actor_count: self.render_stats.actor_count,
            drawn_actor_count: self.render_stats.drawn_actor_count,
        }
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
    ) -> Result<()> {
        let changed = self.poll().context("poll XR terrain runtime")?;
        if !changed && !self.has_pending_render_work(camera_position) {
            self.draw.set_traversal_ready_sections(
                &self
                    .runtime
                    .traversal_ready_render_section_keys(camera_position),
            );
            return Ok(());
        }
        let section_update = self
            .sync_render_sections(camera_position)
            .context("sync XR terrain render sections")?;
        let upload_report = self
            .draw
            .apply_section_updates(
                device,
                &section_update.rebuilt_sections,
                &section_update.removed_section_keys,
            )
            .context("upload XR terrain render section updates")?;
        self.draw.set_traversal_ready_sections(
            &self
                .runtime
                .traversal_ready_render_section_keys(camera_position),
        );
        self.render_stats.section_count = self.draw.section_count();
        self.render_stats.index_count = self.draw.index_count();
        self.render_stats.face_count = quad_face_count_from_indices(self.render_stats.index_count);
        record_render_section_update_stats(&mut self.render_stats, &section_update, upload_report);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_eye_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: XrTerrainEyeTarget<'_>,
        render_view: ChunkRenderView,
        actor_instances: &[mclone_render::entity::ActorInstance],
        render_options: TexturedSectionRenderOptions,
        sky_clear_color: wgpu::Color,
        time_of_day: f32,
        sun_angle: f32,
        label: &'static str,
    ) -> Result<FullFrameRenderSummary> {
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
        self.ui.set_scale(gui_scale);
        let ui_state = self.current_ui_render_state();
        let ui_active = self.ui.is_active();
        let ui_draw = self.ui.render_draw_list(ui_state);
        let summary_ui_draw = ui_draw.clone();
        let mut render_stats = self.render_stats;
        let summary = render_full_frame_for_view(
            frame,
            target.depth,
            &self.sky,
            &mut self.draw,
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
        .with_context(|| format!("render XR terrain {label} eye"))?;
        if ui_active {
            if let Some(panel) = self.menu_panel_pose {
                let controller_ray_lines = self
                    .xr_menu_controller_ray_lines(panel)
                    .context("build XR menu controller ray visuals")?;
                self.world_gui_renderer
                    .render_panel(
                        device,
                        queue,
                        &mut encoder,
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
        let submission = queue.submit(Some(encoder.finish()));
        device
            .poll(wgpu::PollType::WaitForSubmissionIndex(submission))
            .map(|_| ())
            .with_context(|| format!("wait for XR terrain {label}-eye render submission"))?;
        device
            .poll(wgpu::PollType::Wait)
            .map(|_| ())
            .with_context(|| format!("wait for XR terrain {label}-eye render device idle"))?;
        if label == "left" {
            self.render_stats = render_stats;
        }
        Ok(summary)
    }

    fn sync_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<mclone_render_session::RenderSectionCacheUpdate> {
        self.runtime.sync_render_sections(camera_position)
    }

    fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<mclone_render_session::RenderSectionCacheUpdate> {
        self.runtime.sync_all_render_sections(camera_position)
    }

    fn poll(&mut self) -> Result<bool> {
        self.runtime.poll()
    }

    fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        self.runtime.poll_until_idle()
    }

    fn has_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.runtime.has_pending_render_work(camera_position)
    }

    fn apply_startup_view_pose(&mut self, view_pose: XrStartupViewPose) -> Result<()> {
        apply_xr_startup_view_pose(&mut self.camera, view_pose.position, view_pose.yaw_degrees)
    }

    fn commit_engine_camera_player_pose(&mut self) -> Result<bool> {
        let server_changed = self.sync_engine_camera_player_pose()?;
        let interest_changed = self.update_interest_from_engine_camera()?;
        Ok(server_changed || interest_changed)
    }

    fn sync_engine_camera_player_pose(&mut self) -> Result<bool> {
        let changed = if let Some(report) = self.camera.next_pose_sync_command() {
            self.runtime
                .send_gameplay_command(report.command)
                .context("failed to sync XR terrain player pose to server")?
        } else {
            false
        };
        Ok(changed || self.apply_pending_engine_camera_position_updates()?)
    }

    fn apply_pending_engine_camera_position_updates(&mut self) -> Result<bool> {
        let mut changed = false;
        for update in self.runtime.drain_player_position_updates() {
            let accepted = self.camera.accept_position_update(update);
            self.runtime
                .send_gameplay_command(accepted.accept_command)
                .context("failed to acknowledge XR terrain player position correction")?;
            let resync = self.camera.corrected_pose_sync_command();
            self.runtime
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
            changed |= self.update_interest_from_engine_camera()?;
        }
        Ok(changed)
    }

    fn update_interest_from_engine_camera(&mut self) -> Result<bool> {
        let snapshot = self.camera.snapshot();
        let center = snapshot.chunk_pos;
        if self.runtime.set_interest_center(center)? {
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
        self.runtime.sky_clear_color()
    }

    fn effective_render_options(&self, camera_position: Vec3) -> TexturedSectionRenderOptions {
        let mut options = self.render_options;
        if self.camera_inside_occluding_block(camera_position) {
            options.section_occlusion_culling = false;
        }
        options
    }

    fn current_ui_render_state(&self) -> GameUiRenderState {
        GameUiRenderState {
            render_distance: (self.runtime.render_distance() as i32)
                .clamp(1, MAX_XR_RENDER_DISTANCE as i32),
            min_render_distance: 1,
            max_render_distance: MAX_XR_RENDER_DISTANCE as i32,
            section_occlusion_culling: self.render_options.section_occlusion_culling,
            force_fullbright: self.render_options.force_fullbright,
            fly_enabled: self.camera.movement_mode() == EngineCameraMovementMode::NoClip,
            fly_speed_multiplier: self.camera.fly_speed_multiplier() as f32,
            min_fly_speed_multiplier: ENGINE_CAMERA_MIN_FLY_SPEED_MULTIPLIER as f32,
            max_fly_speed_multiplier: ENGINE_CAMERA_MAX_FLY_SPEED_MULTIPLIER as f32,
            frame_pacing_mode: GameFramePacingMode::Vsync,
            fps_cap: XR_UI_FPS_CAP,
            touch_settings: None,
        }
    }

    fn apply_menu_toggle_input(&mut self, controllers: &[XrControllerSnapshot]) {
        let toggle_down = xr_menu_toggle_pressed(controllers);
        if toggle_down && !self.menu_toggle_down {
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

    fn apply_menu_pointer_input(&mut self) -> Result<()> {
        if !self.ui.is_active() {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            return Ok(());
        }
        let Some(panel) = self.menu_panel_pose else {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            return Ok(());
        };
        let Some(origin) = self.tracking_origin else {
            return Ok(());
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
            return Ok(());
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
            self.apply_xr_ui_action(action)?;
        }
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

    fn apply_xr_ui_action(&mut self, action: GameUiAction) -> Result<()> {
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
                if self
                    .runtime
                    .set_render_distance(render_distance)
                    .context("set XR render distance from menu")?
                {
                    log::info!(
                        "XR render distance set to {} (chunk tracking radius {})",
                        render_distance,
                        self.runtime.chunk_tracking_radius()
                    );
                }
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
            GameUiAction::Quit => {
                log::info!("XR menu quit action ignored by shared scene");
            }
            GameUiAction::CycleFramePacing
            | GameUiAction::CycleFpsCap
            | GameUiAction::SetTouchLookSensitivity(_) => {}
            GameUiAction::StartWorld
            | GameUiAction::OpenNewWorld
            | GameUiAction::OpenJoinRemote
            | GameUiAction::RerollSeed
            | GameUiAction::CreateWorld(_)
            | GameUiAction::JoinRemote
            | GameUiAction::Resume
            | GameUiAction::OpenOptions(_)
            | GameUiAction::BackToTitle
            | GameUiAction::BackToPause
            | GameUiAction::QuitToTitle => {}
        }
        self.ui.apply_action(action);
        if !self.ui.is_active() {
            self.ui.clear_input();
            self.menu_pointer_down = false;
            self.menu_panel_pose = None;
            self.menu_panel_recenter_pending = false;
        }
        Ok(())
    }

    fn camera_inside_occluding_block(&self, position: Vec3) -> bool {
        let Some(state_id) = self.runtime.block_state_at_position(position) else {
            return false;
        };
        self.runtime.mesh_assets().catalog.occludes(state_id)
    }

    fn record_eye0_summary(&mut self, summary: FullFrameRenderSummary) {
        self.first_eye_summary = Some(summary);
        self.rendered_frames += 1;
    }
}

fn local_single_view_options(scene: XrSceneOptions) -> LocalSingleViewSceneOptions {
    LocalSingleViewSceneOptions::new(scene.seed, scene.center(), scene.render_distance)
        .with_day_time(scene.day_time_override)
        .with_freeze_time(scene.freeze_time)
        .with_lighting_enabled(scene.lighting_enabled)
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
    let jump = controllers
        .iter()
        .any(|controller| controller.hand == XrHand::Right && controller.a_pressed);
    let descend = controllers
        .iter()
        .any(|controller| controller.hand == XrHand::Left && controller.y_pressed);
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

pub fn xr_menu_toggle_pressed(controllers: &[XrControllerSnapshot]) -> bool {
    controllers
        .iter()
        .any(|controller| controller.hand == XR_MENU_TOGGLE_HAND && controller.select_pressed)
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
    let trigger = if trigger.is_finite() { trigger } else { 0.0 };
    if was_down {
        trigger >= XR_MENU_POINTER_TRIGGER_RELEASE
    } else {
        trigger >= XR_MENU_POINTER_TRIGGER_PRESS
    }
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
    fn xr_locomotion_maps_left_stick_and_a_button_to_engine_input() {
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
    fn xr_locomotion_maps_right_a_button_to_jump() {
        let right = test_controller(XrHand::Right, Vec2::ZERO, true);
        let input = xr_locomotion_input_from_controllers(&[right], 1.0 / 72.0, None);

        assert!(input.jump);
        assert!(!input.descend);
    }

    #[test]
    fn xr_locomotion_maps_left_y_button_to_descend() {
        let mut left = test_controller(XrHand::Left, Vec2::ZERO, false);
        left.y_pressed = true;
        let input = xr_locomotion_input_from_controllers(&[left], 1.0 / 72.0, None);

        assert!(input.descend);
        assert!(!input.jump);
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
        }
    }
}
