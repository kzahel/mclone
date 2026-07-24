//! Offscreen target/cadence driver over the shared scene host's Mono topology.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mclone_app_runtime::frame_pacing::{
    FramePacingDebugStats, FramePacingUiState, FrameTimingStats,
};
use mclone_app_runtime::frame_pipeline_accounting::FramePipelineAccountant;
use mclone_diagnostics::{BudgetDecisionPanelReport, FrameHostKind};
use mclone_input::{
    FlatInputAction, FlatInputFrame, TouchControlsMode, xr_emulation_input_from_flat_frame,
};
use mclone_mesh::VisibilityGraphBuildStats;
use mclone_render::chunk::{ChunkCamera, ChunkDepthTarget, TexturedSectionRenderOptions};
use mclone_render::target::{RenderFrameContext, RenderFrameTarget};
use mclone_render_session::XrView;
use mclone_scene::{
    FlatPresentationFrameSummary, FlatPresentationView, HostEffects, MonoSceneFrameSummary,
    MonoUiContext, MonoUiPresentation, MonoWorldActionStatus, WarmWorldStandbyPhase,
    WarmWorldStandbySnapshot, XrStartupViewPose, XrTerrainEyeTarget, XrTerrainFrameSummary,
    record_mono_frame_pipeline, xr_frame_pipeline_accounting_config,
};
use mclone_ui::{
    GameFlatPresentationState, GameUiAction, GameUiHost, GameWorldRenderScaleMode, GuiScale, Point,
};

use crate::camera::SpectatorCamera;
use crate::cli::{SceneOptions, StartupWaitPolicy};
use crate::desktop_scene_host::{
    DesktopSceneHost, DesktopSceneHostOverrides, create_desktop_scene_host_with_overrides,
};
use crate::scene_runtime::WindowSceneAssets;

const MAX_WARMUP_FRAMES: usize = 65_536;
const STREAM_STABLE_FRAMES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OffscreenFrameClock {
    pub(crate) frame_ms: f64,
    pub(crate) target_frame_ms: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum OffscreenViewTopology {
    #[default]
    Mono,
    FlatAuxiliary,
    Stereo,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OffscreenDriverOptions {
    pub(crate) freeze_scheduled_fluid_ticks: bool,
    view_topology: OffscreenViewTopology,
}

impl OffscreenDriverOptions {
    pub(crate) fn mono_with_frozen_scheduled_fluid_ticks(freeze: bool) -> Self {
        Self {
            freeze_scheduled_fluid_ticks: freeze,
            ..Self::default()
        }
    }
}

impl Default for OffscreenFrameClock {
    fn default() -> Self {
        Self {
            frame_ms: 1_000.0 / 60.0,
            target_frame_ms: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct OffscreenWarmupReport {
    pub(crate) frame_count: usize,
    pub(crate) elapsed_ms: f64,
    pub(crate) poll_ms: f64,
    pub(crate) sync_ms: f64,
    pub(crate) upload_ms: f64,
    pub(crate) visibility_graph: VisibilityGraphBuildStats,
    pub(crate) last_summary: Option<MonoSceneFrameSummary>,
}

impl OffscreenWarmupReport {
    fn observe(&mut self, summary: MonoSceneFrameSummary) {
        self.frame_count += 1;
        self.poll_ms += summary.timing.runtime_poll_ms;
        self.sync_ms += summary.timing.runtime_sync_ms;
        self.upload_ms += summary.timing.runtime_gpu_upload_ms;
        self.visibility_graph.build_count += summary.upload.visibility_graph_build_count;
        self.visibility_graph.total_ms += summary.upload.visibility_graph_total_ms;
        self.visibility_graph.worst_ms = self
            .visibility_graph
            .worst_ms
            .max(summary.upload.visibility_graph_worst_ms);
        self.last_summary = Some(summary);
    }
}

#[derive(Default)]
struct OffscreenHostEffects;

impl HostEffects for OffscreenHostEffects {
    fn request_mouse_lock(&mut self, _requested: bool) -> Result<()> {
        Ok(())
    }

    fn cycle_frame_pacing(&mut self) -> Result<()> {
        Ok(())
    }

    fn cycle_fps_cap(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_world_render_scale_mode(
        &mut self,
        _mode: mclone_ui::GameWorldRenderScaleMode,
    ) -> Result<()> {
        Ok(())
    }

    fn set_touch_controls_mode(&mut self, _mode: TouchControlsMode) -> Result<()> {
        Ok(())
    }

    fn quit_to_title(&mut self) -> Result<()> {
        Ok(())
    }

    fn exit(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Thin offscreen driver. Runtime/session/input/UI/render orchestration stays
/// on `mclone-scene`; this type owns only target sizing, deterministic cadence,
/// readiness loops, and frame-accounting feedback.
pub(crate) struct OffscreenDriver {
    host: DesktopSceneHost,
    depth: ChunkDepthTarget,
    right_depth: Option<ChunkDepthTarget>,
    color_format: wgpu::TextureFormat,
    size: [u32; 2],
    view_topology: OffscreenViewTopology,
    clock: OffscreenFrameClock,
    frame_timing: FrameTimingStats,
    frame_pipeline_accounting: FramePipelineAccountant,
    last_summary: Option<MonoSceneFrameSummary>,
}

impl OffscreenDriver {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        size: [u32; 2],
        scene: &SceneOptions,
        render_options: TexturedSectionRenderOptions,
        assets: &WindowSceneAssets,
        asset_source: &impl mclone_assets::AssetSource,
        startup_camera: Option<&SpectatorCamera>,
    ) -> Result<Self> {
        Self::new_with_options(
            device,
            queue,
            color_format,
            size,
            scene,
            render_options,
            assets,
            asset_source,
            startup_camera,
            OffscreenDriverOptions::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_options(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        size: [u32; 2],
        scene: &SceneOptions,
        render_options: TexturedSectionRenderOptions,
        assets: &WindowSceneAssets,
        asset_source: &impl mclone_assets::AssetSource,
        startup_camera: Option<&SpectatorCamera>,
        options: OffscreenDriverOptions,
    ) -> Result<Self> {
        let startup_view_pose = startup_camera.map(|camera| XrStartupViewPose {
            position: camera.position.to_array(),
            yaw_degrees: camera.yaw.to_degrees(),
        });
        let mut host = create_desktop_scene_host_with_overrides(
            device,
            queue,
            color_format,
            scene,
            render_options,
            assets,
            asset_source,
            startup_view_pose,
            DesktopSceneHostOverrides {
                freeze_scheduled_fluid_ticks: options.freeze_scheduled_fluid_ticks,
            },
        )?;
        if options.view_topology != OffscreenViewTopology::Stereo {
            let mut ui = GameUiHost::new_ingame();
            ui.set_join_remote_addr(
                scene
                    .remote_addr
                    .clone()
                    .unwrap_or_else(|| mclone_ui::DEFAULT_JOIN_REMOTE_ADDR.to_owned()),
            );
            host.configure_mono_ui(ui, MonoUiContext::default());
        }
        host.set_frame_host_kind(FrameHostKind::HeadlessOffscreenPerf);
        host.set_display_refresh_hz(
            (options.view_topology == OffscreenViewTopology::Stereo).then_some(60.0),
        );
        if let Some(camera) = startup_camera {
            set_host_camera(&mut host, camera);
        }
        let size = [size[0].max(1), size[1].max(1)];
        if options.view_topology != OffscreenViewTopology::Stereo {
            host.set_mono_ui_scale(GuiScale::from_pixels(size[0], size[1]));
        }
        Ok(Self {
            host,
            depth: ChunkDepthTarget::new(device, size[0], size[1]),
            right_depth: (options.view_topology != OffscreenViewTopology::Mono)
                .then(|| ChunkDepthTarget::new(device, size[0], size[1])),
            color_format,
            size,
            view_topology: options.view_topology,
            clock: OffscreenFrameClock::default(),
            frame_timing: FrameTimingStats::default(),
            frame_pipeline_accounting: FramePipelineAccountant::new(
                xr_frame_pipeline_accounting_config(None),
            ),
            last_summary: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_stereo_emulation(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        size: [u32; 2],
        scene: &SceneOptions,
        render_options: TexturedSectionRenderOptions,
        assets: &WindowSceneAssets,
        asset_source: &impl mclone_assets::AssetSource,
    ) -> Result<Self> {
        Self::new_with_options(
            device,
            queue,
            color_format,
            size,
            scene,
            render_options,
            assets,
            asset_source,
            None,
            OffscreenDriverOptions {
                view_topology: OffscreenViewTopology::Stereo,
                ..OffscreenDriverOptions::default()
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_flat_auxiliary(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        size: [u32; 2],
        scene: &SceneOptions,
        render_options: TexturedSectionRenderOptions,
        assets: &WindowSceneAssets,
        asset_source: &impl mclone_assets::AssetSource,
    ) -> Result<Self> {
        Self::new_with_options(
            device,
            queue,
            color_format,
            size,
            scene,
            render_options,
            assets,
            asset_source,
            None,
            OffscreenDriverOptions {
                view_topology: OffscreenViewTopology::FlatAuxiliary,
                ..OffscreenDriverOptions::default()
            },
        )
    }

    pub(crate) fn host(&self) -> &DesktopSceneHost {
        &self.host
    }

    pub(crate) fn host_mut(&mut self) -> &mut DesktopSceneHost {
        &mut self.host
    }

    pub(crate) fn stereo_ui_is_active(&self) -> bool {
        self.host.mono_ui_is_active()
    }

    pub(crate) fn set_clock(&mut self, clock: OffscreenFrameClock) {
        self.clock = clock;
        self.frame_pipeline_accounting =
            FramePipelineAccountant::new(xr_frame_pipeline_accounting_config(
                clock
                    .target_frame_ms
                    .filter(|period_ms| period_ms.is_finite() && *period_ms > 0.0)
                    .map(|period_ms| 1_000.0 / period_ms),
            ));
    }

    pub(crate) fn last_summary(&self) -> Option<MonoSceneFrameSummary> {
        self.last_summary.clone()
    }

    pub(crate) fn far_lod_settle_snapshot(&self) -> Result<mclone_scene::FarLodSettleSnapshot> {
        let render_view = self.host.mono_render_view(self.size)?;
        self.host
            .mono_far_lod_settle_snapshot(render_view)
            .context("offscreen far LOD settle snapshot requires an active runtime")
    }

    pub(crate) fn set_camera(&mut self, camera: &SpectatorCamera) {
        set_host_camera(&mut self.host, camera);
    }

    pub(crate) fn commit_camera(&mut self) -> Result<bool> {
        self.host.force_mono_player_pose_reconcile_for_diagnostics()
    }

    pub(crate) fn render(
        &mut self,
        frame: RenderFrameContext<'_>,
        ui: MonoUiPresentation,
        hud_visible: bool,
    ) -> Result<MonoSceneFrameSummary> {
        self.render_inner(frame, ui, hud_visible, false)
    }

    pub(crate) fn render_frozen(
        &mut self,
        frame: RenderFrameContext<'_>,
        ui: MonoUiPresentation,
        hud_visible: bool,
    ) -> Result<MonoSceneFrameSummary> {
        self.render_inner(frame, ui, hud_visible, true)
    }

    pub(crate) fn depth_target(&self) -> &ChunkDepthTarget {
        &self.depth
    }

    pub(crate) fn render_stereo(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        left_color_view: &wgpu::TextureView,
        right_color_view: &wgpu::TextureView,
    ) -> Result<XrTerrainFrameSummary> {
        let right_depth = self
            .right_depth
            .as_ref()
            .context("stereo render requested from a mono offscreen driver")?;
        self.host.render_frame(
            device,
            queue,
            views,
            XrTerrainEyeTarget {
                color_view: left_color_view,
                depth: &self.depth,
                size: self.size,
            },
            XrTerrainEyeTarget {
                color_view: right_color_view,
                depth: right_depth,
                size: self.size,
            },
        )
    }

    pub(crate) fn render_flat_views_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        views: &[FlatPresentationView<'_>],
        hud_visible: bool,
    ) -> Result<FlatPresentationFrameSummary> {
        if self.view_topology != OffscreenViewTopology::FlatAuxiliary {
            bail!("flat presentation render requested from a non-flat driver");
        }
        self.frame_timing
            .begin_frame(self.clock.frame_ms, self.clock.target_frame_ms);
        self.host.set_mono_ui_context(MonoUiContext {
            frame_pacing: FramePacingUiState::default(),
            pacing_debug: FramePacingDebugStats {
                target_frame_ms: self.clock.target_frame_ms,
                ..FramePacingDebugStats::default()
            },
            frame_timing: self.frame_timing,
            render_scale: 1.0,
            hud_visible,
            ..MonoUiContext::default()
        });
        let primary_size = views
            .first()
            .context("flat presentation render requires at least one view")?
            .target
            .size;
        let frame_start = Instant::now();
        self.host
            .set_mono_ui_scale(GuiScale::from_pixels(primary_size[0], primary_size[1]));
        let summary = self
            .host
            .render_flat_presentation_frame_frozen(device, queue, encoder, &views)?;
        let frame_ms = frame_start.elapsed().as_secs_f64() * 1_000.0;
        self.frame_timing
            .record_runtime_poll(summary.timing.runtime_poll_ms);
        self.frame_timing.record_remesh_upload(
            summary.timing.runtime_sync_ms,
            summary.timing.runtime_gpu_upload_ms,
        );
        self.frame_timing.record_surface_frame(
            summary.timing.render_views_ms,
            0.0,
            frame_ms,
            0.0,
            0.0,
        );
        Ok(summary)
    }

    pub(crate) fn render_stereo_frozen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
        left_color_view: &wgpu::TextureView,
        right_color_view: &wgpu::TextureView,
    ) -> Result<XrTerrainFrameSummary> {
        let right_depth = self
            .right_depth
            .as_ref()
            .context("stereo render requested from a mono offscreen driver")?;
        self.host.render_frame_frozen_runtime(
            device,
            queue,
            views,
            XrTerrainEyeTarget {
                color_view: left_color_view,
                depth: &self.depth,
                size: self.size,
            },
            XrTerrainEyeTarget {
                color_view: right_color_view,
                depth: right_depth,
                size: self.size,
            },
        )
    }

    pub(crate) fn apply_stereo_input_frame(
        &mut self,
        frame: FlatInputFrame,
        views: [XrView; 2],
    ) -> Result<()> {
        let input = xr_emulation_input_from_flat_frame(frame);
        self.host
            .apply_frame_locomotion(&input, views, None)
            .context("apply synthetic stereo locomotion")?;
        Ok(())
    }

    pub(crate) fn drive_stereo_until_streamed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        views: [XrView; 2],
    ) -> Result<()> {
        if self.right_depth.is_none() {
            bail!("stereo warmup requested from a mono offscreen driver");
        }
        let output_size = self.size;
        self.resize_target(device, [1, 1]);
        let make_scratch = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.color_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let left = make_scratch("mclone_offscreen_stereo_warmup_left");
        let right = make_scratch("mclone_offscreen_stereo_warmup_right");
        let left_view = left.create_view(&wgpu::TextureViewDescriptor::default());
        let right_view = right.create_view(&wgpu::TextureViewDescriptor::default());
        let result = (|| {
            let mut stable = 0usize;
            for _ in 0..MAX_WARMUP_FRAMES {
                self.apply_stereo_input_frame(FlatInputFrame::default(), views)?;
                let summary = self.render_stereo(device, queue, views, &left_view, &right_view)?;
                let eye = self.host.camera_snapshot().eye;
                let pending = self.host.pending_stream_work(glam::Vec3::new(
                    eye.x as f32,
                    eye.y as f32,
                    eye.z as f32,
                ));
                if self.host.local_startup_complete()
                    && self.host.has_runtime()
                    && summary.drawn_section_count > 0
                    && pending == 0
                    && self
                        .host
                        .warm_world_standby_snapshot()
                        .is_none_or(|snapshot| snapshot.phase.terminal())
                {
                    stable += 1;
                } else {
                    stable = 0;
                }
                if stable >= STREAM_STABLE_FRAMES {
                    if let Some(snapshot) = self.host.warm_world_standby_snapshot() {
                        if snapshot.phase != WarmWorldStandbyPhase::Switchable {
                            bail!(
                                "warm-world standby ended in phase {} during stereo warmup: {}",
                                snapshot.phase.label(),
                                snapshot.failure.as_deref().unwrap_or("no failure detail"),
                            );
                        }
                        // GPU advances include ordinary compile/poll gaps; the
                        // lifecycle counters are the conservation invariant.
                        if !snapshot.readiness.switchable
                            || snapshot.initial_upload_applied_lifecycle_items
                                != snapshot.initial_upload_lifecycle_items
                            || snapshot.initial_upload_released_compile_jobs != 0
                        {
                            bail!(
                                "warm-world standby violated stereo switchable conservation: readiness={} initial={}/{} initial_releases={} ready_advances={}",
                                snapshot.readiness.switchable,
                                snapshot.initial_upload_applied_lifecycle_items,
                                snapshot.initial_upload_lifecycle_items,
                                snapshot.initial_upload_released_compile_jobs,
                                snapshot.gpu_ready_advance_count,
                            );
                        }
                        eprintln!(
                            "warm_world_standby_stereo id={} seed={} phase={} elapsed_ms={:.3} shell_ms={:.3} multiview_ms={:.3} polls={} loaded_chunks={} seed_sections={} drawable_sections={} seed_bytes={} initial_uploads={}/{} initial_releases={} gpu_advances={}/{} gpu_ms={:.3} gpu_sections={} gpu_indices={} queue={} queue_bytes={} entry_resident={} topology_ready={} worst_advance_ms={:.3} worst_startup_step_ms={:.3} worst_runtime_poll_ms={:.3} worst_gpu_ms={:.3} endpoint_ms={:.3} skipped_no_slack={}",
                            snapshot.instance_id.get(),
                            snapshot.seed,
                            snapshot.phase.label(),
                            snapshot.elapsed_ms,
                            snapshot.renderer_shell_create_ms,
                            snapshot.renderer_multiview_create_ms,
                            snapshot.poll_count,
                            snapshot.loaded_chunks,
                            snapshot.startup_seed_sections,
                            snapshot.startup_seed_drawable_sections,
                            snapshot.startup_seed_owned_bytes,
                            snapshot.initial_upload_applied_lifecycle_items,
                            snapshot.initial_upload_lifecycle_items,
                            snapshot.initial_upload_released_compile_jobs,
                            snapshot.gpu_advance_count,
                            snapshot.gpu_ready_advance_count,
                            snapshot.gpu_warm_ms,
                            snapshot.gpu_section_count,
                            snapshot.gpu_index_count,
                            snapshot.queued_upload_lifecycle_items,
                            snapshot.queued_upload_mesh_owned_bytes,
                            snapshot.readiness.entry_section_gpu_resident
                                && snapshot.readiness.entry_section_traversal_ready,
                            snapshot.readiness.renderer_topology_ready,
                            snapshot.worst_advance_ms,
                            snapshot.worst_startup_step_ms,
                            snapshot.worst_runtime_poll_ms,
                            snapshot.worst_gpu_advance_ms,
                            snapshot.endpoint_resolution_ms,
                            snapshot.gpu_skipped_no_slack_count,
                        );
                    }
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            bail!("offscreen stereo scene host did not settle after {MAX_WARMUP_FRAMES} frames")
        })();
        self.resize_target(device, output_size);
        result
    }

    fn render_inner(
        &mut self,
        frame: RenderFrameContext<'_>,
        ui: MonoUiPresentation,
        hud_visible: bool,
        frozen: bool,
    ) -> Result<MonoSceneFrameSummary> {
        let view = self.host.mono_render_view(frame.target.size)?;
        self.render_view_inner(frame, view, ui, hud_visible, frozen)
    }

    fn render_view_inner(
        &mut self,
        frame: RenderFrameContext<'_>,
        view: mclone_render::chunk::ChunkRenderView,
        ui: MonoUiPresentation,
        hud_visible: bool,
        frozen: bool,
    ) -> Result<MonoSceneFrameSummary> {
        let output_size = frame.target.size;
        self.frame_timing
            .begin_frame(self.clock.frame_ms, self.clock.target_frame_ms);
        let target_hz = self
            .clock
            .target_frame_ms
            .filter(|period_ms| period_ms.is_finite() && *period_ms > 0.0)
            .map(|period_ms| (1_000.0 / period_ms) as f32);
        self.host.set_display_refresh_hz(target_hz);
        self.host.set_mono_ui_context(MonoUiContext {
            frame_pacing: FramePacingUiState::default(),
            pacing_debug: FramePacingDebugStats {
                target_frame_ms: self.clock.target_frame_ms,
                ..FramePacingDebugStats::default()
            },
            frame_timing: self.frame_timing,
            render_scale: 1.0,
            flat_presentation: Some(GameFlatPresentationState::new(
                output_size,
                output_size,
                1.0,
                GameWorldRenderScaleMode::Automatic,
            )),
            hud_visible,
            ..MonoUiContext::default()
        });
        let frame_start = Instant::now();
        let summary = if frozen {
            self.host
                .render_mono_scene_frame_frozen(frame, &self.depth, view, ui)?
        } else {
            self.host
                .render_mono_scene_frame(frame, &self.depth, view, ui)?
        };
        let frame_ms = frame_start.elapsed().as_secs_f64() * 1_000.0;
        self.frame_timing
            .record_runtime_poll(summary.timing.runtime_poll_ms);
        self.frame_timing.record_remesh_upload(
            summary.timing.runtime_sync_ms,
            summary.timing.runtime_gpu_upload_ms,
        );
        self.frame_timing.record_surface_frame(
            summary.timing.render_views_ms,
            0.0,
            frame_ms,
            0.0,
            0.0,
        );
        let update = record_mono_frame_pipeline(
            &mut self.frame_pipeline_accounting,
            frame_ms,
            true,
            Some(summary.clone()),
            self.host.latest_budget_decision_panel(),
        );
        self.host
            .set_frame_pipeline_budget_signal(update.budget_signal);
        if let Some((report, revision)) = update.published_report {
            self.host.set_frame_pipeline_report(report, revision);
        }
        self.last_summary = Some(summary.clone());
        Ok(summary)
    }

    pub(crate) fn render_chunk_camera_frozen(
        &mut self,
        frame: RenderFrameContext<'_>,
        camera: ChunkCamera,
        ui: MonoUiPresentation,
    ) -> Result<MonoSceneFrameSummary> {
        let spectator = spectator_from_chunk_camera(camera);
        self.set_camera(&spectator);
        let view = camera.render_view(frame.target.size[0], frame.target.size[1]);
        let summary = self.render_view_inner(frame, view, ui, true, true)?;
        Ok(summary)
    }

    /// Render an explicit diagnostic view without moving the scene camera or
    /// its server-side chunk interest. This keeps capture framing independent
    /// from the region being generated and retained.
    pub(crate) fn render_detached_chunk_camera_frozen(
        &mut self,
        frame: RenderFrameContext<'_>,
        camera: ChunkCamera,
        ui: MonoUiPresentation,
        hud_visible: bool,
    ) -> Result<MonoSceneFrameSummary> {
        let view = camera.render_view(frame.target.size[0], frame.target.size[1]);
        let summary = self.render_view_inner(frame, view, ui, hud_visible, true)?;
        Ok(summary)
    }

    pub(crate) fn drive_to_wait_policy(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        startup_wait: StartupWaitPolicy,
    ) -> Result<OffscreenWarmupReport> {
        match startup_wait {
            StartupWaitPolicy::None | StartupWaitPolicy::Frames(_) => {
                Ok(OffscreenWarmupReport::default())
            }
            StartupWaitPolicy::Progress => self.drive_until(device, queue, |driver, _| {
                driver
                    .host
                    .mono_loading_progress_overlay()
                    .is_some_and(|progress| !progress.cells.is_empty())
            }),
            StartupWaitPolicy::Playable => self.drive_until(device, queue, |driver, summary| {
                let far_lod = driver.host.far_lod_stats();
                let far_lod_ready = !driver.host.scene_options().far_lod.enabled
                    || (far_lod.visible_tiles > 0 && far_lod.queued_uploads == 0);
                driver.host.local_startup_complete()
                    && driver.host.has_runtime()
                    && summary.render.section_count > 0
                    && far_lod_ready
            }),
            StartupWaitPolicy::Idle => {
                let mut stable = 0usize;
                self.drive_until(device, queue, move |driver, summary| {
                    let camera_position = driver.host.camera_snapshot().eye;
                    let pending = driver.host.pending_stream_work(glam::Vec3::new(
                        camera_position.x as f32,
                        camera_position.y as f32,
                        camera_position.z as f32,
                    ));
                    if pending == 0
                        && !driver.host.asset_replacement_in_progress()
                        && summary.render.drawn_section_count > 0
                    {
                        stable += 1;
                    } else {
                        stable = 0;
                    }
                    stable >= STREAM_STABLE_FRAMES
                })
            }
        }
    }

    pub(crate) fn drive_until_streamed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<OffscreenWarmupReport> {
        self.drive_to_wait_policy(device, queue, StartupWaitPolicy::Idle)
    }

    pub(crate) fn drive_until_warm_world_standby_ready(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<WarmWorldStandbySnapshot> {
        self.drive_until(device, queue, |driver, _| {
            driver
                .host
                .warm_world_standby_snapshot()
                .is_some_and(|snapshot| snapshot.phase.terminal())
        })?;
        let snapshot = self
            .host
            .warm_world_standby_snapshot()
            .context("warm-world standby readiness requested without a standby")?;
        if snapshot.phase != WarmWorldStandbyPhase::Switchable {
            bail!(
                "warm-world standby ended in phase {} before switchable readiness: {}",
                snapshot.phase.label(),
                snapshot.failure.as_deref().unwrap_or("no failure detail"),
            );
        }
        // GPU advances include ordinary compile/poll gaps; the lifecycle
        // counters are the conservation invariant.
        if !snapshot.readiness.switchable
            || snapshot.initial_upload_applied_lifecycle_items
                != snapshot.initial_upload_lifecycle_items
            || snapshot.initial_upload_released_compile_jobs != 0
        {
            bail!(
                "warm-world standby violated switchable conservation: readiness={} initial={}/{} initial_releases={} ready_advances={}",
                snapshot.readiness.switchable,
                snapshot.initial_upload_applied_lifecycle_items,
                snapshot.initial_upload_lifecycle_items,
                snapshot.initial_upload_released_compile_jobs,
                snapshot.gpu_ready_advance_count,
            );
        }
        Ok(snapshot)
    }

    pub(crate) fn drive_until_embedded_preview_idle(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<OffscreenWarmupReport> {
        let mut stable_frames = 0_usize;
        self.drive_until(device, queue, move |driver, _| {
            let idle = driver
                .host
                .embedded_world_preview_snapshot()
                .is_some_and(|preview| {
                    let preparation = preview.preparation;
                    preparation.pending_compile_jobs == 0
                        && preparation.queued_upload_lifecycle_items == 0
                        && preparation.last_submitted_compile_section_count == 0
                        && preparation.last_accepted_compile_result_count == 0
                        && preparation.last_uploaded_section_count == 0
                });
            stable_frames = if idle {
                stable_frames.saturating_add(1)
            } else {
                0
            };
            stable_frames >= STREAM_STABLE_FRAMES
        })
    }

    /// Re-run the idle gate at the actual capture size. The normal warmup path
    /// intentionally uses a 1x1 target; probes call this once afterward so the
    /// first full-size frustum cannot reveal one last unit of stream work.
    pub(crate) fn drive_until_streamed_at_output_size(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<OffscreenWarmupReport> {
        let mut stable = 0usize;
        self.drive_until_with_target_size(
            device,
            queue,
            move |driver, summary| {
                let camera_position = driver.host.camera_snapshot().eye;
                let pending = driver.host.pending_stream_work(glam::Vec3::new(
                    camera_position.x as f32,
                    camera_position.y as f32,
                    camera_position.z as f32,
                ));
                if pending == 0 && summary.render.drawn_section_count > 0 {
                    stable += 1;
                } else {
                    stable = 0;
                }
                stable >= STREAM_STABLE_FRAMES
            },
            false,
        )
    }

    pub(crate) fn drive_until_target_complete(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<OffscreenWarmupReport> {
        let mut stable = 0usize;
        self.drive_until(device, queue, move |driver, summary| {
            let eye = driver.host.camera_snapshot().eye;
            let camera_position = glam::Vec3::new(eye.x as f32, eye.y as f32, eye.z as f32);
            let target_loaded = driver.host.runtime_stats().is_some_and(|stats| {
                let diameter = stats.render_distance.saturating_mul(2).saturating_add(1);
                stats.loaded_chunks >= (diameter as usize).saturating_pow(2)
            });
            let view_ready = driver
                .host
                .mono_view_readiness_overlay()
                .is_none_or(|progress| {
                    progress.target_chunk_count > 0
                        && progress.target_ready_chunks == progress.target_chunk_count
                });
            if target_loaded
                && view_ready
                && driver.host.pending_stream_work(camera_position) == 0
                && summary.render.drawn_section_count > 0
            {
                stable += 1;
            } else {
                stable = 0;
            }
            stable >= STREAM_STABLE_FRAMES
        })
    }

    fn drive_until(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ready: impl FnMut(&Self, &MonoSceneFrameSummary) -> bool,
    ) -> Result<OffscreenWarmupReport> {
        self.drive_until_with_target_size(device, queue, ready, true)
    }

    fn drive_until_with_target_size(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut ready: impl FnMut(&Self, &MonoSceneFrameSummary) -> bool,
        shrink_target: bool,
    ) -> Result<OffscreenWarmupReport> {
        let output_size = self.size;
        if shrink_target {
            self.resize_target(device, [1, 1]);
        }
        let scratch = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mclone_offscreen_driver_warmup"),
            size: wgpu::Extent3d {
                width: self.size[0],
                height: self.size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let scratch_view = scratch.create_view(&wgpu::TextureViewDescriptor::default());
        let result = (|| {
            let start = Instant::now();
            let mut report = OffscreenWarmupReport::default();
            for _ in 0..MAX_WARMUP_FRAMES {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mclone_offscreen_driver_warmup_encoder"),
                });
                let summary = self.render(
                    RenderFrameContext::new(
                        device,
                        queue,
                        &mut encoder,
                        RenderFrameTarget::color(&scratch_view, self.size),
                    ),
                    MonoUiPresentation::None,
                    false,
                )?;
                queue.submit(std::iter::once(encoder.finish()));
                device
                    .poll(wgpu::PollType::Wait)
                    .context("poll device during offscreen warmup")?;
                report.observe(summary.clone());
                if ready(self, &summary) {
                    report.elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;
                    return Ok(report);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let last = report.last_summary.as_ref();
            bail!(
                "offscreen scene host did not reach requested readiness after {MAX_WARMUP_FRAMES} frames: runtime={} sections={} drawn={} pending={}",
                self.host.has_runtime(),
                last.map_or(0, |summary| summary.render.section_count),
                last.map_or(0, |summary| summary.render.drawn_section_count),
                {
                    let eye = self.host.camera_snapshot().eye;
                    self.host.pending_stream_work(glam::Vec3::new(
                        eye.x as f32,
                        eye.y as f32,
                        eye.z as f32,
                    ))
                }
            )
        })();
        if shrink_target {
            self.resize_target(device, output_size);
        }
        result
    }

    fn resize_target(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        self.size = [size[0].max(1), size[1].max(1)];
        self.depth = ChunkDepthTarget::new(device, self.size[0], self.size[1]);
        if self.right_depth.is_some() {
            self.right_depth = Some(ChunkDepthTarget::new(device, self.size[0], self.size[1]));
        }
        if self.view_topology != OffscreenViewTopology::Stereo {
            self.host
                .set_mono_ui_scale(GuiScale::from_pixels(self.size[0], self.size[1]));
        }
    }

    pub(crate) fn apply_input_frame(
        &mut self,
        frame: FlatInputFrame,
    ) -> Result<Vec<(FlatInputAction, MonoWorldActionStatus)>> {
        if frame.open_menu {
            self.host.open_mono_pause_menu();
            self.host.clear_mono_camera_input();
            return Ok(Vec::new());
        }
        self.host
            .advance_mono_input_frame(frame, self.clock.frame_ms / 1_000.0)?;
        if self.host.mono_ui_is_active() {
            return Ok(Vec::new());
        }
        if let Some(slot) = frame.selected_hotbar_slot {
            self.host.select_mono_hotbar_slot(slot);
        }
        if frame.hotbar_step != 0 {
            self.host.step_mono_hotbar_slot(frame.hotbar_step);
        }
        let mut statuses = Vec::new();
        if frame.attack {
            statuses.push((
                FlatInputAction::Attack,
                self.host
                    .handle_mono_world_action(FlatInputAction::Attack)?,
            ));
        }
        if frame.use_item {
            statuses.push((
                FlatInputAction::Use,
                self.host.handle_mono_world_action(FlatInputAction::Use)?,
            ));
        }
        Ok(statuses)
    }

    pub(crate) fn apply_ui_pointer_click(
        &mut self,
        point: Point,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Option<GameUiAction>> {
        self.host.mono_ui_pointer_down(point);
        let (_, action) = self.host.mono_ui_pointer_up(point);
        if let Some(action) = action {
            let mut effects = OffscreenHostEffects;
            self.host
                .apply_mono_ui_action(action, true, device, queue, &mut effects)?;
        }
        Ok(action)
    }

    pub(crate) fn apply_ui_action(
        &mut self,
        action: GameUiAction,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let mut effects = OffscreenHostEffects;
        self.host
            .apply_mono_ui_action(action, false, device, queue, &mut effects)?;
        Ok(())
    }

    pub(crate) fn latest_budget_decision_panel(&self) -> BudgetDecisionPanelReport {
        self.host.latest_budget_decision_panel()
    }
}

fn set_host_camera(host: &mut DesktopSceneHost, camera: &SpectatorCamera) {
    host.set_mono_capture_camera(
        mclone_core::Vec3d::new(
            f64::from(camera.position.x),
            f64::from(camera.position.y),
            f64::from(camera.position.z),
        ),
        f64::from(camera.yaw),
        f64::from(camera.pitch),
        f64::from(camera.speed),
    );
}

fn spectator_from_chunk_camera(camera: ChunkCamera) -> SpectatorCamera {
    let eye = glam::Vec3::from_array(camera.eye);
    let direction = (glam::Vec3::from_array(camera.target) - eye).normalize_or_zero();
    SpectatorCamera {
        position: eye,
        yaw: direction.x.atan2(direction.z),
        pitch: direction.y.clamp(-1.0, 1.0).asin(),
        speed: crate::camera::SPECTATOR_BASE_SPEED,
    }
}
