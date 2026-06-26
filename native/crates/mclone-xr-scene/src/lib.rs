#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use glam::{Quat, Vec3};
use mclone_app_runtime::frame_render::{
    FullFrameGui, FullFrameRenderSummary, RenderStreamStats, record_render_section_update_stats,
    render_full_frame_for_view,
};
use mclone_app_runtime::render_assets::{RenderSectionCompileWorker, TexturedMeshAssets};
use mclone_app_runtime::{RuntimeUpdateApplyReport, SingleViewRuntime, elapsed_ms};
use mclone_client::ClientRuntime;
use mclone_core::{ChunkPos, Vec3d};
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::chunk::{
    ChunkDepthTarget, ChunkRenderView, TexturedSectionDrawResources, TexturedSectionRenderOptions,
    TexturedSectionUploadReport,
};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_render_session::{EngineCameraController, EngineCameraSnapshot};
use mclone_server::{
    IntegratedServerRunner, NativeIntegratedServerRunner, NativeIntegratedServerRunnerConfig,
    ServerRunnerDiagnostics,
};
use mclone_ui::GuiDrawList;
use openxr as xr;

pub const DEFAULT_XR_SEED: i64 = 12_345;
pub const DEFAULT_XR_CHUNK_X: i32 = 0;
pub const DEFAULT_XR_CHUNK_Z: i32 = 0;
pub const DEFAULT_XR_RENDER_DISTANCE: u32 = 2;
pub const MAX_XR_RENDER_DISTANCE: u32 = 16;
pub const XR_NEAR: f32 = 0.05;
pub const XR_FAR: f32 = 700.0;

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
}

#[derive(Clone, Copy)]
pub struct XrTerrainEyeTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth: &'a ChunkDepthTarget,
    pub size: [u32; 2],
}

pub struct XrMcloneTerrainState {
    runtime: SingleViewRuntime,
    server_runner: NativeIntegratedServerRunner,
    render_compile_worker: RenderSectionCompileWorker,
    camera: EngineCameraController,
    initial_alignment_mode: XrViewAlignmentMode,
    render_options: TexturedSectionRenderOptions,
    draw: TexturedSectionDrawResources,
    sky: SkyRenderer,
    mesh_assets: TexturedMeshAssets,
    render_stats: RenderStreamStats,
    tracking_origin: Option<XrTrackingOrigin>,
    first_eye_summary: Option<FullFrameRenderSummary>,
    rendered_frames: u32,
}

impl XrMcloneTerrainState {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        scene: XrSceneOptions,
        render_options: TexturedSectionRenderOptions,
        mesh_assets: TexturedMeshAssets,
        startup_view_pose: Option<XrStartupViewPose>,
    ) -> Result<Self> {
        let scene = scene.validated()?;
        let render_compile_worker = RenderSectionCompileWorker::new(mesh_assets.catalog.clone())?;
        let chunk_tracking_radius =
            mclone_app_runtime::chunk_tracking_radius_for_render_distance(scene.render_distance);
        let mut runtime = SingleViewRuntime::new(
            ClientRuntime::local_integrated(),
            scene.center(),
            scene.render_distance,
            chunk_tracking_radius,
        );
        let mut server_runner = NativeIntegratedServerRunner::new(native_runner_config(scene))
            .context("start XR integrated server runner")?;
        set_chunk_view(&mut runtime, &mut server_runner, scene.center())?;
        let initial_day_time = server_runner
            .poll_diagnostics()
            .context("poll initial XR server diagnostics")?
            .day_time;
        runtime.force_day_time(initial_day_time);
        if let Some(day_time) = scene.day_time_override {
            runtime.force_day_time(day_time);
        }

        let mut state = Self {
            runtime,
            server_runner,
            render_compile_worker,
            camera: EngineCameraController::spawn_for_chunk(scene.center()),
            initial_alignment_mode: if startup_view_pose.is_some() {
                XrViewAlignmentMode::ViewPose
            } else {
                XrViewAlignmentMode::PlayerSpawn
            },
            render_options,
            draw: TexturedSectionDrawResources::new(
                device,
                queue,
                color_format,
                &[],
                mesh_assets.atlas.as_upload(),
            )
            .context("create empty XR terrain draw resources")?,
            sky: SkyRenderer::new(device, color_format),
            mesh_assets,
            render_stats: RenderStreamStats::default(),
            tracking_origin: None,
            first_eye_summary: None,
            rendered_frames: 0,
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
                "XR terrain seed={} center=({}, {}) render_distance={} produced no render sections",
                scene.seed,
                scene.chunk_x,
                scene.chunk_z,
                scene.render_distance
            );
        }
        state.draw = TexturedSectionDrawResources::new(
            device,
            queue,
            color_format,
            &sections,
            state.mesh_assets.atlas.as_upload(),
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
            "mclone XR terrain runtime: seed={} center=({}, {}) render_distance={} chunks={} sections={} faces={} indices={} initial_polls={} poll_ms={:.3} elapsed_ms={:.3}",
            scene.seed,
            scene.chunk_x,
            scene.chunk_z,
            scene.render_distance,
            state.runtime.client().loaded_chunk_count(),
            state.render_stats.section_count,
            state.render_stats.face_count,
            state.render_stats.index_count,
            initial_poll_count,
            initial_poll_ms,
            elapsed_ms(initial_poll_start.elapsed())
        );
        Ok(state)
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
        self.poll_runtime_and_upload(device, center_position)?;
        let render_options = self.effective_render_options(center_position);
        let sky_clear_color = self.sky_clear_color();
        let time_of_day = self.runtime.time_of_day();
        let sun_angle = self.runtime.sun_angle();
        let left_summary = self.render_eye_target(
            device,
            queue,
            left_target,
            render_views[0],
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
            render_options,
            sky_clear_color,
            time_of_day,
            sun_angle,
            "right",
        )?;
        self.record_eye0_summary(left_summary);
        Ok(self.frame_summary())
    }

    pub fn frame_summary(&self) -> XrTerrainFrameSummary {
        if let Some(summary) = self.first_eye_summary {
            return XrTerrainFrameSummary {
                rendered_frames: self.rendered_frames,
                section_count: summary.section_count,
                drawn_section_count: summary.drawn_section_count,
                index_count: summary.index_count,
                drawn_index_count: summary.drawn_index_count,
            };
        }
        XrTerrainFrameSummary {
            rendered_frames: self.rendered_frames,
            section_count: self.render_stats.section_count,
            drawn_section_count: self.render_stats.drawn_section_count,
            index_count: self.render_stats.index_count,
            drawn_index_count: self.render_stats.drawn_index_count,
        }
    }

    fn render_views(&mut self, views: &[xr::View]) -> Result<[ChunkRenderView; 2]> {
        if views.len() < 2 {
            bail!("OpenXR runtime returned fewer than two stereo views");
        }
        let tracking_origin = match self.tracking_origin {
            Some(origin) => origin,
            None => {
                let origin =
                    XrTrackingOrigin::from_initial_views(views, self.initial_alignment_mode)?;
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
                origin
            }
        };
        let transform =
            XrStageToWorld::from_tracking_origin(tracking_origin, self.camera.snapshot())?;
        Ok([
            xr_view_to_chunk_render_view(&views[0], transform, XR_NEAR, XR_FAR)?,
            xr_view_to_chunk_render_view(&views[1], transform, XR_NEAR, XR_FAR)?,
        ])
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
        let mut render_stats = self.render_stats;
        let summary = render_full_frame_for_view(
            frame,
            target.depth,
            &self.sky,
            &mut self.draw,
            None,
            None,
            None,
            render_view,
            &[],
            None,
            sky_clear_color,
            time_of_day,
            sun_angle,
            render_options,
            FullFrameGui::new(false, false, [1.0, 1.0]),
            |_| GuiDrawList::new(),
            &mut render_stats,
        )
        .with_context(|| format!("render XR terrain {label} eye"))?;
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
        self.runtime.sync_render_sections(
            &mut self.render_compile_worker,
            camera_position,
            |client, _compiler| client.chunk_snapshots().cloned().collect(),
        )
    }

    fn sync_all_render_sections(
        &mut self,
        camera_position: Vec3,
    ) -> Result<mclone_render_session::RenderSectionCacheUpdate> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut combined = mclone_render_session::RenderSectionCacheUpdate::default();
        loop {
            let update = self.runtime.sync_render_sections_with_budget(
                &mut self.render_compile_worker,
                camera_position,
                usize::MAX,
                |client, _compiler| client.chunk_snapshots().cloned().collect(),
            )?;
            let progressed = update.rebuilt_section_count() > 0
                || update.removed_section_count() > 0
                || update.submitted_compile_section_count > 0
                || update.completed_compile_section_count > 0
                || update.stale_compile_section_count > 0;
            combined.merge(update);
            if self.render_compile_worker.pending_job_count() == 0
                && !self.runtime.has_ready_pending_render_work(camera_position)
            {
                combined.pending_compile_jobs = 0;
                return Ok(combined);
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for XR terrain render section compile queue");
            }
            if !progressed {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn poll(&mut self) -> Result<bool> {
        let apply_report = drain_runner_updates_report(&mut self.runtime, &mut self.server_runner)?;
        let changed = apply_report.changed;
        let runner_diagnostics = self
            .server_runner
            .poll_diagnostics()
            .context("poll XR integrated server diagnostics")?;
        self.runtime
            .finish_poll_diagnostics(0.0, apply_report, Some(&runner_diagnostics));
        Ok(changed)
    }

    fn poll_until_idle(&mut self) -> Result<(usize, f64)> {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut polls = 0_usize;
        let mut poll_ms = 0.0_f64;
        loop {
            let poll_start = Instant::now();
            self.poll()?;
            poll_ms += elapsed_ms(poll_start.elapsed());
            polls += 1;
            if local_server_idle(&self.server_runner)? {
                return Ok((polls, poll_ms));
            }
            if Instant::now() >= deadline {
                bail!("timed out waiting for XR terrain worldgen jobs");
            }
            if self
                .server_runner
                .poll_diagnostics()
                .context("poll XR integrated server diagnostics")?
                .update_queue_depth
                == 0
            {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn has_pending_render_work(&self, camera_position: Vec3) -> bool {
        self.runtime.has_pending_render_work(
            self.render_compile_worker.pending_job_count(),
            camera_position,
        )
    }

    fn apply_startup_view_pose(&mut self, view_pose: XrStartupViewPose) -> Result<()> {
        let yaw_radians = view_pose.yaw_degrees.to_radians();
        if !yaw_radians.is_finite() {
            bail!("invalid XR startup view yaw {}", view_pose.yaw_degrees);
        }
        self.camera.set_eye_pose(
            vec3d_from_glam(Vec3::from_array(view_pose.position)),
            f64::from(yaw_radians),
            0.0,
        );
        Ok(())
    }

    fn commit_engine_camera_player_pose(&mut self) -> Result<bool> {
        let server_changed = self.sync_engine_camera_player_pose()?;
        let interest_changed = self.update_interest_from_engine_camera()?;
        Ok(server_changed || interest_changed)
    }

    fn sync_engine_camera_player_pose(&mut self) -> Result<bool> {
        let changed = if let Some(report) = self.camera.next_pose_sync_command() {
            dispatch_runner_command(&mut self.runtime, &mut self.server_runner, report.command)
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
            dispatch_runner_command(
                &mut self.runtime,
                &mut self.server_runner,
                accepted.accept_command,
            )
            .context("failed to acknowledge XR terrain player position correction")?;
            let resync = self.camera.corrected_pose_sync_command();
            dispatch_runner_command(&mut self.runtime, &mut self.server_runner, resync.command)
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
        if set_chunk_view(&mut self.runtime, &mut self.server_runner, center)? {
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

    fn sky_clear_color(&self) -> wgpu::Color {
        mclone_render::sky::overworld_clear_color(self.runtime.client().time_of_day())
    }

    fn effective_render_options(&self, camera_position: Vec3) -> TexturedSectionRenderOptions {
        let mut options = self.render_options;
        if self.camera_inside_occluding_block(camera_position) {
            options.section_occlusion_culling = false;
        }
        options
    }

    fn camera_inside_occluding_block(&self, position: Vec3) -> bool {
        let Some(state_id) = self.runtime.block_state_at_position(position) else {
            return false;
        };
        self.mesh_assets.catalog.occludes(state_id)
    }

    fn record_eye0_summary(&mut self, summary: FullFrameRenderSummary) {
        self.first_eye_summary.get_or_insert(summary);
        self.rendered_frames += 1;
    }
}

fn native_runner_config(scene: XrSceneOptions) -> NativeIntegratedServerRunnerConfig {
    NativeIntegratedServerRunnerConfig::new(scene.seed)
        .with_lighting_enabled(scene.lighting_enabled)
        .with_day_time(scene.day_time_override)
        .with_day_time_frozen(scene.freeze_time)
}

fn set_chunk_view(
    runtime: &mut SingleViewRuntime,
    runner: &mut NativeIntegratedServerRunner,
    center: ChunkPos,
) -> Result<bool> {
    let render_distance = runtime.render_distance();
    let chunk_tracking_radius = runtime.chunk_tracking_radius();
    let Some(command) =
        runtime.set_chunk_view_command(center, render_distance, chunk_tracking_radius)
    else {
        return Ok(false);
    };
    dispatch_runner_command(runtime, runner, command)
}

fn dispatch_runner_command(
    runtime: &mut SingleViewRuntime,
    runner: &mut NativeIntegratedServerRunner,
    command: mclone_protocol::ClientCommand,
) -> Result<bool> {
    runner
        .send_command(command)
        .context("failed to send command to XR integrated server runner")?;
    let _ = drain_runner_updates_report(runtime, runner)?;
    Ok(true)
}

fn drain_runner_updates_report(
    runtime: &mut SingleViewRuntime,
    runner: &mut NativeIntegratedServerRunner,
) -> Result<RuntimeUpdateApplyReport> {
    let updates = runner
        .drain_updates()
        .context("failed to drain XR integrated server runner updates")?;
    if updates.is_empty() {
        return Ok(RuntimeUpdateApplyReport::default());
    }
    Ok(runtime.apply_server_updates_report(updates))
}

fn local_server_idle(runner: &NativeIntegratedServerRunner) -> Result<bool> {
    let diagnostics = runner
        .poll_diagnostics()
        .context("poll XR integrated server diagnostics")?;
    Ok(server_diagnostics_idle(diagnostics))
}

fn server_diagnostics_idle(diagnostics: ServerRunnerDiagnostics) -> bool {
    diagnostics.command_queue_depth == 0
        && diagnostics.update_queue_depth == 0
        && !diagnostics.awaiting_tick
        && diagnostics.pending_jobs == 0
        && diagnostics.pending_publications == 0
}

#[derive(Clone, Copy, Debug)]
struct XrTrackingOrigin {
    origin_stage: Vec3,
    stage_yaw: f32,
    mode: XrViewAlignmentMode,
}

#[derive(Clone, Copy, Debug)]
struct XrStageToWorld {
    origin_stage: Vec3,
    origin_world: Vec3,
    stage_to_world_rotation: Quat,
}

impl XrTrackingOrigin {
    fn from_initial_views(views: &[xr::View], mode: XrViewAlignmentMode) -> Result<Self> {
        let left_stage_pose = mclone_xr_host::view_pose(&views[0])?;
        let right_stage_pose = mclone_xr_host::view_pose(&views[1])?;
        let origin_stage = (left_stage_pose.position + right_stage_pose.position) * 0.5;
        Self::from_stage_view(origin_stage, left_stage_pose.orientation, mode)
    }

    fn from_stage_view(
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

    fn mode_label(self) -> &'static str {
        self.mode.label()
    }
}

impl XrStageToWorld {
    fn from_tracking_origin(
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

    fn transform_pose(self, stage_position: Vec3, stage_orientation: Quat) -> (Vec3, Quat) {
        (
            self.origin_world
                + self
                    .stage_to_world_rotation
                    .mul_vec3(stage_position - self.origin_stage),
            (self.stage_to_world_rotation * stage_orientation).normalize(),
        )
    }
}

fn xr_view_to_chunk_render_view(
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

fn chunk_render_view_from_xr_render_view(view: mclone_xr_host::XrRenderView) -> ChunkRenderView {
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

fn yaw_from_forward(forward: Vec3) -> Option<f32> {
    if !forward.is_finite() {
        return None;
    }
    let horizontal = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    if horizontal.length_squared() <= f32::EPSILON {
        return None;
    }
    Some((-horizontal.x).atan2(-horizontal.z))
}

fn normalize_angle(angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn glam_vec3_from_vec3d(value: Vec3d) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

fn vec3d_from_glam(value: Vec3) -> Vec3d {
    Vec3d::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
