use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, ActorPresentation, ActorPresentationKind,
    ClientInteractionController, ClientRuntime, LOCAL_PLAYER_STANDING_EYE_HEIGHT, PlayerInputKey,
};
use mclone_core::{BlockPos, Vec3d};
use mclone_mesh::quad_face_count_from_indices;
use mclone_protocol::EntityKind;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, TexturedSectionDrawResources,
    TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
use mclone_render::entity::{ActorDrawResources, ActorInstance};
use mclone_render::gui::{GuiRenderOptions, GuiRenderer};
use mclone_render::native::{NativeSurfaceContext, SurfaceFrameStatus};
use mclone_render::sky_render::SkyRenderer;
use mclone_render::target::RenderFrameContext;
use mclone_ui::{GuiScale, Point};
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::camera::{SpectatorCamera, chunk_camera_from_engine};
use crate::cli::SceneOptions;
use crate::frame_pacing::{
    FramePacing, FramePacingMode, FramePacingUiState, FrameTimingStats, RedrawSchedule, elapsed_ms,
    next_capped_redraw_deadline, redraw_schedule,
};
use crate::scene_runtime::{WindowSceneRuntime, poll_window_runtime_until_idle};
use crate::ui::{DebugPaneStats, NativeUi, NativeUiAction};
use crate::{MAX_RENDER_DISTANCE, MIN_RENDER_DISTANCE};
use mclone_render_session::{
    EngineCameraController, EngineCameraFrameState, EngineCameraMovementMode, EngineCameraSnapshot,
    RenderSectionCacheUpdate, render_camera_from_snapshot,
};

const NO_CLIP_TOGGLE_KEY: KeyCode = KeyCode::KeyN;
const PLAYER_SURFACE_FEET_OFFSET: f64 = 1.0;
const GROUND_PROBE_DISTANCE: f64 = 0.01;

pub(crate) fn run_window(
    scene: SceneOptions,
    render_options: TexturedSectionRenderOptions,
) -> Result<()> {
    let runtime = WindowSceneRuntime::new(&scene)?;
    log::info!(
        "native window runtime seed={} initial_center=({}, {}) render_distance={} chunk_tracking_radius={} lighting={} remote={:?} atlas={}x{}",
        scene.seed,
        scene.chunk_x,
        scene.chunk_z,
        scene.render_distance,
        runtime.chunk_tracking_radius,
        if scene.lighting_enabled {
            "enabled"
        } else {
            "disabled"
        },
        scene.remote_addr,
        runtime.mesh_assets.atlas.width,
        runtime.mesh_assets.atlas.height
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ChunkApp::new(
        runtime,
        SpectatorCamera::spawn_for_scene(&scene),
        render_options,
        scene.render_distance,
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RenderStreamStats {
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) face_count: u32,
    pub(crate) drawn_face_count: u32,
    pub(crate) index_count: u32,
    pub(crate) drawn_index_count: u32,
    pub(crate) last_rebuilt_section_count: usize,
    pub(crate) last_removed_section_count: usize,
    pub(crate) last_rebuilt_vertex_count: u32,
    pub(crate) last_rebuilt_face_count: u32,
    pub(crate) last_rebuilt_index_count: u32,
    pub(crate) last_neighbor_ready_section_count: usize,
    pub(crate) last_near_exception_section_count: usize,
    pub(crate) last_deferred_section_count: usize,
    pub(crate) last_submitted_compile_section_count: usize,
    pub(crate) last_completed_compile_section_count: usize,
    pub(crate) last_stale_compile_section_count: usize,
    pub(crate) last_pending_compile_jobs: usize,
    pub(crate) last_visibility_graph_build_count: usize,
    pub(crate) last_visibility_graph_total_ms: f64,
    pub(crate) last_visibility_graph_worst_ms: f64,
    pub(crate) last_uploaded_section_count: usize,
    pub(crate) last_upload_removed_section_count: usize,
    pub(crate) last_uploaded_vertex_count: u32,
    pub(crate) last_uploaded_face_count: u32,
    pub(crate) last_uploaded_index_count: u32,
    pub(crate) last_remesh_ms: f64,
    pub(crate) last_upload_ms: f64,
    pub(crate) last_frame_ms: f32,
    pub(crate) actor_count: usize,
    pub(crate) drawn_actor_count: usize,
    pub(crate) drawn_actor_index_count: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FullFrameRenderSummary {
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) index_count: u32,
    pub(crate) drawn_index_count: u32,
    pub(crate) gui_command_count: usize,
    pub(crate) actor_count: usize,
    pub(crate) drawn_actor_count: usize,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_full_frame(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    actors: &mut ActorDrawResources,
    gui: &mut GuiRenderer,
    camera: ChunkCamera,
    actor_instances: &[ActorInstance],
    sky_clear_color: wgpu::Color,
    time_of_day: f32,
    sun_angle: f32,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacingUiState,
    ui: &NativeUi,
    debug_stats: Option<DebugPaneStats>,
    render_stats: &mut RenderStreamStats,
) -> Result<FullFrameRenderSummary> {
    let ui_active = ui.is_active();
    let ui_covers_world = ui.covers_world();
    let debug_stats = (!ui_active).then_some(debug_stats).flatten();
    let gui_active = ui_active || debug_stats.is_some();
    let render_options =
        render_options.with_sky_darken(mclone_render::light_texture::sky_darken(time_of_day));
    let mut actor_stats = mclone_render::entity::ActorRenderStats::default();

    if !ui_covers_world {
        let render_view = camera.render_view(frame.target.size[0], frame.target.size[1]);
        sky.render(
            frame.queue,
            frame.encoder,
            frame.target.color_view,
            sky_clear_color,
            render_view.sky_view_projection(),
            time_of_day,
            sun_angle,
        );
        // The sky pass cleared and drew the background; the chunk pass loads it.
        let render_target = ChunkRenderTarget::from_frame_target(
            frame.target.with_depth(&depth.view),
            sky_clear_color,
        )?
        .with_loaded_color();
        let frame_stats = draw.render_with_options(
            frame.queue,
            frame.encoder,
            render_target,
            render_view,
            render_options,
        )?;
        render_stats.drawn_section_count = frame_stats.drawn_section_count;
        render_stats.drawn_face_count = frame_stats.drawn_face_count();
        render_stats.drawn_index_count = frame_stats.drawn_index_count;
        actor_stats = actors.render(
            frame.device,
            frame.queue,
            frame.encoder,
            frame.target.with_depth(&depth.view),
            render_view,
            render_options,
            actor_instances,
        )?;
    } else {
        render_stats.drawn_section_count = 0;
        render_stats.drawn_face_count = 0;
        render_stats.drawn_index_count = 0;
    }
    render_stats.actor_count = actor_instances.len();
    render_stats.drawn_actor_count = actor_stats.drawn_actor_count;
    render_stats.drawn_actor_index_count = actor_stats.index_count;

    let mut ui_draw = ui.render_draw_list(render_options, frame_pacing);
    if let Some(mut stats) = debug_stats {
        stats.render = *render_stats;
        ui.render_debug_pane(&mut ui_draw, &stats);
    }

    let gui_command_count = ui_draw.commands().len();
    if gui_active {
        gui.render(
            frame.device,
            frame.queue,
            frame.encoder,
            frame.target,
            [ui.scale.width, ui.scale.height],
            &ui_draw,
            if ui_covers_world {
                GuiRenderOptions::clear(mclone_render::default_clear_color())
            } else {
                GuiRenderOptions::overlay()
            },
        )?;
    }

    Ok(FullFrameRenderSummary {
        section_count: draw.section_count(),
        drawn_section_count: render_stats.drawn_section_count,
        index_count: draw.index_count(),
        drawn_index_count: render_stats.drawn_index_count,
        gui_command_count,
        actor_count: actor_instances.len(),
        drawn_actor_count: actor_stats.drawn_actor_count,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowCameraView {
    snapshot: EngineCameraSnapshot,
    eye: glam::Vec3,
}

impl WindowCameraView {
    fn from_snapshot(snapshot: EngineCameraSnapshot) -> Self {
        Self {
            snapshot,
            eye: glam_vec3_from_vec3d(snapshot.eye),
        }
    }

    fn block_column(self) -> (i32, i32) {
        (
            self.snapshot.eye.x.floor() as i32,
            self.snapshot.eye.z.floor() as i32,
        )
    }

    fn chunk_camera(self, render_distance: u32) -> ChunkCamera {
        chunk_camera_from_engine(render_camera_from_snapshot(self.snapshot, render_distance))
    }
}

struct ChunkApp {
    runtime: WindowSceneRuntime,
    spectator: SpectatorCamera,
    camera: EngineCameraController,
    actor_interpolation: ActorInterpolationState,
    interaction: ClientInteractionController,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacing,
    ui: NativeUi,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    depth: Option<ChunkDepthTarget>,
    sky: Option<SkyRenderer>,
    draw: Option<TexturedSectionDrawResources>,
    actors: Option<ActorDrawResources>,
    gui: Option<GuiRenderer>,
    mouse_locked: bool,
    mouse_lock_requested: bool,
    last_cursor: Option<(f64, f64)>,
    debug_visible: bool,
    last_frame: Instant,
    next_redraw_at: Option<Instant>,
    render_stats: RenderStreamStats,
    frame_timing: FrameTimingStats,
}

impl ChunkApp {
    fn new(
        runtime: WindowSceneRuntime,
        spectator: SpectatorCamera,
        render_options: TexturedSectionRenderOptions,
        render_distance: i32,
    ) -> Self {
        let camera = engine_camera_controller_from_spectator(&spectator);
        Self {
            runtime,
            spectator,
            camera,
            actor_interpolation: ActorInterpolationState::new(),
            interaction: ClientInteractionController::new(),
            render_options,
            frame_pacing: FramePacing::default(),
            ui: NativeUi::new_ingame(render_distance),
            window: None,
            surface: None,
            depth: None,
            sky: None,
            draw: None,
            actors: None,
            gui: None,
            mouse_locked: false,
            mouse_lock_requested: false,
            last_cursor: None,
            debug_visible: false,
            last_frame: Instant::now(),
            next_redraw_at: None,
            render_stats: RenderStreamStats::default(),
            frame_timing: FrameTimingStats::default(),
        }
    }

    fn schedule_next_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

        if self.frame_pacing.mode != FramePacingMode::Capped {
            self.next_redraw_at = None;
        }

        match redraw_schedule(self.frame_pacing.mode, Instant::now(), self.next_redraw_at) {
            RedrawSchedule::RequestNow => {
                event_loop.set_control_flow(ControlFlow::Poll);
                window.request_redraw();
            }
            RedrawSchedule::WaitUntil(deadline) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        }
    }

    fn finish_redraw(&mut self, event_loop: &ActiveEventLoop, frame_start: Instant) {
        if let Some(frame_duration) = self.frame_pacing.target_frame_duration() {
            let next_redraw_at = next_capped_redraw_deadline(
                frame_start,
                Instant::now(),
                self.next_redraw_at,
                frame_duration,
            );
            self.next_redraw_at = Some(next_redraw_at);
        } else {
            self.next_redraw_at = None;
        }
        self.schedule_next_redraw(event_loop);
    }

    fn window_camera_view(&self) -> WindowCameraView {
        WindowCameraView::from_snapshot(self.camera.snapshot())
    }

    fn update_camera_from_keys(&mut self, now: Instant) -> Result<()> {
        let frame_dt = now.duration_since(self.last_frame);
        let movement_dt = frame_dt.as_secs_f32().min(0.05);
        self.last_frame = now;
        self.render_stats.last_frame_ms = (frame_dt.as_secs_f64() * 1000.0) as f32;
        self.frame_timing.begin_frame(
            frame_dt.as_secs_f64() * 1000.0,
            self.frame_pacing.target_frame_ms(),
        );

        if self.ui.is_active() {
            return Ok(());
        }

        if self
            .camera
            .tick_movement(self.runtime.client(), f64::from(movement_dt))
        {
            self.commit_player_pose_change()?;
        }
        Ok(())
    }

    fn gui_scale(&self) -> Option<GuiScale> {
        self.surface.as_ref().map(|surface| {
            GuiScale::from_pixels(surface.config.width.max(1), surface.config.height.max(1))
        })
    }

    fn gui_point(&self, x: f64, y: f64) -> Option<Point> {
        self.gui_scale().map(|scale| scale.client_to_gui(x, y))
    }

    fn apply_ui_action(
        &mut self,
        action: NativeUiAction,
        event_loop: &ActiveEventLoop,
        from_pointer_click: bool,
    ) {
        let should_arm_mouse_lock = from_pointer_click
            && matches!(action, NativeUiAction::StartWorld | NativeUiAction::Resume);
        let preserve_pointer_state = matches!(action, NativeUiAction::SetRenderDistance(_));
        match action {
            NativeUiAction::ToggleSectionOcclusion => {
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
            NativeUiAction::ToggleFullbright => {
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
            NativeUiAction::CycleFramePacing => {
                self.frame_pacing.cycle_mode();
                self.next_redraw_at = None;
                if let Some(surface) = &mut self.surface {
                    self.frame_pacing.apply_to_surface(surface);
                }
                log::info!(
                    "frame pacing set to {} at cap {} fps",
                    self.frame_pacing.mode.label(),
                    self.frame_pacing.fps_cap
                );
            }
            NativeUiAction::CycleFpsCap => {
                self.frame_pacing.cycle_fps_cap();
                self.next_redraw_at = None;
                log::info!("fps cap set to {}", self.frame_pacing.fps_cap);
            }
            NativeUiAction::SetRenderDistance(render_distance) => {
                let render_distance =
                    render_distance.clamp(MIN_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
                let render_distance_chunks =
                    u32::try_from(render_distance).expect("clamped render distance must fit u32");
                match self.runtime.set_render_distance(render_distance_chunks) {
                    Ok(true) => {
                        log::info!(
                            "render distance set to {} (chunk tracking radius {})",
                            render_distance,
                            self.runtime.chunk_tracking_radius
                        );
                    }
                    Ok(false) => {}
                    Err(err) => {
                        log::error!("failed to set render distance to {render_distance}: {err:#}");
                        return;
                    }
                }
            }
            NativeUiAction::Quit => {
                event_loop.exit();
                return;
            }
            NativeUiAction::StartWorld
            | NativeUiAction::Resume
            | NativeUiAction::OpenOptions(_)
            | NativeUiAction::BackToTitle
            | NativeUiAction::BackToPause => {}
        }
        self.ui.apply_action(action);
        if should_arm_mouse_lock {
            self.mouse_lock_requested = true;
        }
        self.camera.clear_keys();
        if !preserve_pointer_state {
            self.last_cursor = None;
        }
        self.sync_mouse_lock();
        self.schedule_next_redraw(event_loop);
    }

    fn sync_mouse_lock(&mut self) {
        self.set_mouse_lock(self.mouse_lock_requested && !self.ui.is_active());
    }

    fn toggle_movement_mode(&mut self) {
        let movement_mode = self.camera.toggle_movement_mode();
        log::info!("player movement mode {}", movement_mode.label());
    }

    fn place_spectator_above_loaded_surface(&mut self) {
        let view = self.window_camera_view();
        let (world_x, world_z) = view.block_column();
        let Some(surface_y) = self
            .runtime
            .highest_non_air_block_y_at_world(world_x, world_z)
        else {
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
        if self.camera.movement_mode() == EngineCameraMovementMode::Walking {
            self.camera
                .probe_ground(self.runtime.client(), GROUND_PROBE_DISTANCE);
        }
        if let Err(err) = self.commit_player_pose_change() {
            log::warn!("failed to commit initial player pose: {err:#}");
        }
        log::info!(
            "placed player above loaded surface column ({world_x}, {world_z}) y={} -> feet_y={:.1} eye_y={:.1}",
            surface_y,
            self.camera.player().pose().position.y,
            self.window_camera_view().eye.y
        );
    }

    fn set_mouse_lock(&mut self, should_lock: bool) {
        let Some(window) = &self.window else {
            self.mouse_locked = false;
            return;
        };
        if should_lock == self.mouse_locked {
            return;
        }

        if should_lock {
            let grab_result =
                window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|locked_err| {
                        log::warn!(
                            "cursor lock unavailable ({locked_err}); trying confined cursor grab"
                        );
                        window.set_cursor_grab(CursorGrabMode::Confined)
                    });
            match grab_result {
                Ok(()) => {
                    window.set_cursor_visible(false);
                    self.mouse_locked = true;
                    self.last_cursor = None;
                }
                Err(err) => {
                    log::warn!("failed to grab cursor for mouse look: {err}");
                    window.set_cursor_visible(true);
                    self.mouse_locked = false;
                }
            }
        } else {
            if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                log::warn!("failed to release cursor grab: {err}");
            }
            window.set_cursor_visible(true);
            self.mouse_locked = false;
            self.last_cursor = None;
        }
    }

    fn update_interest_from_camera(&mut self) -> Result<bool> {
        let view = self.window_camera_view();
        let center = view.snapshot.chunk_pos;
        if self.runtime.set_interest_center(center)? {
            log::info!(
                "chunk interest moved to ({}, {}) at camera position ({:.1}, {:.1}, {:.1})",
                center.x,
                center.z,
                view.eye.x,
                view.eye.y,
                view.eye.z
            );
            return Ok(true);
        }
        Ok(false)
    }

    fn poll_runtime_and_upload(&mut self) -> Result<()> {
        let poll_start = Instant::now();
        let mut changed = self.runtime.poll()?;
        changed |= self.apply_pending_player_position_updates()?;
        self.frame_timing
            .record_runtime_poll(elapsed_ms(poll_start.elapsed()));
        if !changed
            && !self
                .runtime
                .has_pending_render_work(self.window_camera_view().eye)
        {
            return Ok(());
        }
        self.upload_runtime_sections()?;
        Ok(())
    }

    fn upload_runtime_sections(&mut self) -> Result<()> {
        let camera_view = self.window_camera_view();
        let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) else {
            return Ok(());
        };
        let remesh_start = Instant::now();
        let section_update = self.runtime.sync_render_sections(camera_view.eye)?;
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let upload_start = Instant::now();
        let upload_report = draw
            .apply_section_updates(
                &surface.device,
                &section_update.rebuilt_sections,
                &section_update.removed_section_keys,
            )
            .context("failed to upload streamed chunk section updates")?;
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let section_count = draw.section_count();
        let index_count = draw.index_count();
        let face_count = quad_face_count_from_indices(index_count);
        self.render_stats.section_count = section_count;
        self.render_stats.index_count = index_count;
        self.render_stats.face_count = face_count;
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.record_section_update_stats(&section_update, upload_report);
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        self.frame_timing.record_remesh_upload(remesh_ms, upload_ms);
        log::info!(
            "streamed chunks loaded={} sections={} faces={} indices={} rebuilt={} visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} uploaded={} uploaded_vertices={} uploaded_faces={} uploaded_indices={} removed={} remesh_ms={:.3} upload_ms={:.3}",
            self.runtime.client().loaded_chunk_count(),
            section_count,
            face_count,
            index_count,
            section_update.rebuilt_section_count(),
            section_update.visibility_graph_stats.build_count,
            section_update.visibility_graph_stats.total_ms,
            section_update.visibility_graph_stats.worst_ms,
            upload_report.uploaded_section_count,
            upload_report.uploaded_vertex_count,
            upload_report.uploaded_face_count(),
            upload_report.uploaded_index_count,
            upload_report.removed_section_count,
            remesh_ms,
            upload_ms
        );
        Ok(())
    }

    fn record_section_update_stats(
        &mut self,
        section_update: &RenderSectionCacheUpdate,
        upload_report: TexturedSectionUploadReport,
    ) {
        record_render_section_update_stats(&mut self.render_stats, section_update, upload_report);
    }

    fn effective_render_options(&self) -> TexturedSectionRenderOptions {
        effective_render_options_for_camera(
            self.render_options,
            self.runtime
                .camera_inside_occluding_block(self.window_camera_view().eye),
        )
    }

    fn debug_pane_stats(&self, render_options: TexturedSectionRenderOptions) -> DebugPaneStats {
        let camera_state = self.camera_frame_state();
        DebugPaneStats {
            position: self.window_camera_view().eye,
            speed: camera_state.camera.speed_blocks_per_second as f32,
            movement_mode: camera_state.movement_mode_label(),
            on_ground: camera_state.on_ground,
            runtime: self.runtime.stats(),
            render: self.render_stats,
            frame: self.frame_timing,
            pacing: self.frame_pacing.debug_stats(),
            section_occlusion: render_options.section_occlusion_culling,
            force_fullbright: render_options.force_fullbright,
        }
    }

    fn camera_frame_state(&self) -> EngineCameraFrameState {
        self.camera.frame_state(&self.interaction)
    }

    fn commit_player_pose_change(&mut self) -> Result<bool> {
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
        let server_changed = self.sync_server_player_pose()?;
        let interest_changed = self.update_interest_from_camera()?;
        Ok(server_changed || interest_changed)
    }

    fn interpolated_actor_instances(&mut self) -> Vec<ActorInstance> {
        self.actor_interpolation
            .reconcile_authoritative(self.runtime.client().actor_presentations());
        self.actor_interpolation.step(
            (self.render_stats.last_frame_ms * 0.001).min(0.1),
            ActorInterpolationConfig::default(),
        );
        actor_instances_from_presentations(
            &self.actor_interpolation.presentations(),
            self.runtime.client(),
        )
    }

    fn sync_server_player_pose(&mut self) -> Result<bool> {
        let changed = if let Some(command) = self.camera.next_move_player_command() {
            self.runtime
                .send_gameplay_command(command)
                .context("failed to sync player pose to server")?
        } else {
            false
        };
        Ok(changed || self.apply_pending_player_position_updates()?)
    }

    fn apply_pending_player_position_updates(&mut self) -> Result<bool> {
        let mut changed = false;
        for update in self.runtime.drain_player_position_updates() {
            let ack = self.camera.apply_player_position_update(update);
            sync_spectator_from_camera(&mut self.spectator, &self.camera);
            self.runtime
                .send_gameplay_command(ack)
                .context("failed to acknowledge player position correction")?;
            self.runtime
                .send_gameplay_command(self.camera.pos_rot_move_player_command())
                .context("failed to sync corrected player pose to server")?;
            let pose = self.camera.player().pose();
            log::warn!(
                "accepted server player position correction id={} feet=({:.2}, {:.2}, {:.2})",
                update.teleport_id,
                pose.position.x,
                pose.position.y,
                pose.position.z
            );
            changed = true;
        }
        if changed {
            changed |= self.update_interest_from_camera()?;
        }
        Ok(changed)
    }

    fn sync_carried_item(&mut self) -> Result<bool> {
        let Some(command) = self.interaction.ensure_has_sent_carried_item() else {
            return Ok(false);
        };
        self.runtime
            .send_gameplay_command(command)
            .context("failed to sync carried item to server")
    }

    fn handle_world_mouse_pressed(&mut self, button: MouseButton) -> Result<()> {
        self.sync_server_player_pose()?;
        self.sync_carried_item()?;
        let hit = self
            .camera
            .pick_block(self.runtime.client(), &self.interaction);
        let command = match button {
            MouseButton::Left => self.interaction.debug_instant_break_command(hit),
            MouseButton::Right => self.interaction.use_item_on_command(hit),
            _ => None,
        };
        let Some(command) = command else {
            return Ok(());
        };
        let changed = self
            .runtime
            .send_gameplay_command(command)
            .context("failed to send gameplay interaction command")?;
        log::info!(
            "gameplay interaction {:?} at ({}, {}, {}) face={:?} changed={}",
            button,
            hit.block_pos.x,
            hit.block_pos.y,
            hit.block_pos.z,
            hit.direction,
            changed
        );
        Ok(())
    }
}

fn effective_render_options_for_camera(
    mut render_options: TexturedSectionRenderOptions,
    camera_inside_occluding_block: bool,
) -> TexturedSectionRenderOptions {
    if camera_inside_occluding_block {
        render_options.section_occlusion_culling = false;
    }
    render_options
}

fn vec3d_from_glam(value: glam::Vec3) -> Vec3d {
    Vec3d::new(value.x as f64, value.y as f64, value.z as f64)
}

fn glam_vec3_from_vec3d(value: Vec3d) -> glam::Vec3 {
    glam::Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

pub(crate) fn actor_instances_from_presentations(
    presentations: &[ActorPresentation],
    client: &ClientRuntime,
) -> Vec<ActorInstance> {
    presentations
        .iter()
        .map(|actor| {
            let packed_light =
                client.packed_light_at_world_or_fullbright(actor_light_probe_block_pos(actor));
            match actor.kind {
                ActorPresentationKind::RemotePlayer => ActorInstance::remote_player(
                    glam_vec3_from_vec3d(actor.feet_position),
                    actor.y_rot_degrees,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Cow) => ActorInstance::cow_model(
                    glam_vec3_from_vec3d(actor.feet_position),
                    actor.y_rot_degrees,
                    actor.width,
                    actor.height,
                )
                .with_packed_light(packed_light),
                ActorPresentationKind::Entity(EntityKind::Chicken) => {
                    ActorInstance::chicken_placeholder(
                        glam_vec3_from_vec3d(actor.feet_position),
                        actor.y_rot_degrees,
                        actor.width,
                        actor.height,
                    )
                    .with_packed_light(packed_light)
                }
            }
        })
        .collect()
}

fn actor_light_probe_block_pos(actor: &ActorPresentation) -> BlockPos {
    BlockPos::containing(actor.feet_position.add(Vec3d::new(
        0.0,
        actor_light_probe_height(actor),
        0.0,
    )))
}

fn actor_light_probe_height(actor: &ActorPresentation) -> f64 {
    match actor.kind {
        ActorPresentationKind::RemotePlayer => LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        ActorPresentationKind::Entity(EntityKind::Cow) => 1.3,
        ActorPresentationKind::Entity(EntityKind::Chicken) => f64::from(actor.height) * 0.92,
    }
}

fn engine_camera_controller_from_spectator(spectator: &SpectatorCamera) -> EngineCameraController {
    EngineCameraController::from_eye_pose(
        vec3d_from_glam(spectator.position),
        f64::from(spectator.yaw),
        f64::from(spectator.pitch),
        f64::from(spectator.speed),
    )
}

fn sync_spectator_from_camera(spectator: &mut SpectatorCamera, camera: &EngineCameraController) {
    let snapshot = camera.snapshot();
    spectator.position = glam_vec3_from_vec3d(snapshot.eye);
    spectator.yaw = snapshot.yaw_radians as f32;
    spectator.pitch = snapshot.pitch_radians as f32;
    spectator.speed = snapshot.speed_blocks_per_second as f32;
}

pub(crate) fn record_render_section_update_stats(
    render_stats: &mut RenderStreamStats,
    section_update: &RenderSectionCacheUpdate,
    upload_report: TexturedSectionUploadReport,
) {
    render_stats.last_rebuilt_section_count = section_update.rebuilt_section_count();
    render_stats.last_removed_section_count = section_update.removed_section_count();
    render_stats.last_rebuilt_vertex_count = section_update.rebuilt_vertex_count;
    render_stats.last_rebuilt_face_count = section_update.rebuilt_face_count();
    render_stats.last_rebuilt_index_count = section_update.rebuilt_index_count;
    render_stats.last_neighbor_ready_section_count = section_update.neighbor_ready_section_count;
    render_stats.last_near_exception_section_count = section_update.near_exception_section_count;
    render_stats.last_deferred_section_count = section_update.deferred_section_count;
    render_stats.last_submitted_compile_section_count =
        section_update.submitted_compile_section_count;
    render_stats.last_completed_compile_section_count =
        section_update.completed_compile_section_count;
    render_stats.last_stale_compile_section_count = section_update.stale_compile_section_count;
    render_stats.last_pending_compile_jobs = section_update.pending_compile_jobs;
    render_stats.last_visibility_graph_build_count =
        section_update.visibility_graph_stats.build_count;
    render_stats.last_visibility_graph_total_ms = section_update.visibility_graph_stats.total_ms;
    render_stats.last_visibility_graph_worst_ms = section_update.visibility_graph_stats.worst_ms;
    render_stats.last_uploaded_section_count = upload_report.uploaded_section_count;
    render_stats.last_upload_removed_section_count = upload_report.removed_section_count;
    render_stats.last_uploaded_vertex_count = upload_report.uploaded_vertex_count;
    render_stats.last_uploaded_face_count = upload_report.uploaded_face_count();
    render_stats.last_uploaded_index_count = upload_report.uploaded_index_count;
}

impl ApplicationHandler for ChunkApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let initial_poll_start = Instant::now();
        let (initial_poll_count, initial_poll_ms) =
            match poll_window_runtime_until_idle(&mut self.runtime) {
                Ok(report) => report,
                Err(err) => {
                    log::error!("failed to load initial light-ready chunks: {err:#}");
                    event_loop.exit();
                    return;
                }
            };
        match self.apply_pending_player_position_updates() {
            Ok(true) => {}
            Ok(false) => self.place_spectator_above_loaded_surface(),
            Err(err) => {
                log::error!("failed to apply initial server player position: {err:#}");
                event_loop.exit();
                return;
            }
        }
        log::info!(
            "initial light-ready chunks loaded in {} polls poll_ms={:.3} elapsed_ms={:.3} loaded={}",
            initial_poll_count,
            initial_poll_ms,
            elapsed_ms(initial_poll_start.elapsed()),
            self.runtime.client().loaded_chunk_count()
        );

        let attrs = Window::default_attributes()
            .with_title("mclone native")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 900));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                log::error!("failed to create native window: {err}");
                event_loop.exit();
                return;
            }
        };
        let mut surface = match NativeSurfaceContext::new(window.clone()) {
            Ok(surface) => surface,
            Err(err) => {
                log::error!("failed to initialize native GPU: {err:#}");
                event_loop.exit();
                return;
            }
        };
        self.frame_pacing.update_monitor(&window);
        self.frame_pacing.apply_to_surface(&mut surface);
        let remesh_start = Instant::now();
        let initial_camera = self.window_camera_view();
        let section_update = match self.runtime.sync_all_render_sections(initial_camera.eye) {
            Ok(update) => update,
            Err(err) => {
                log::error!("failed to build initial chunk sections: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let sections = self.runtime.cached_sections();
        if sections.is_empty() {
            log::error!("initial chunk area produced no render sections after light-ready load");
            event_loop.exit();
            return;
        }
        let remesh_ms = elapsed_ms(remesh_start.elapsed());
        let upload_start = Instant::now();
        let depth =
            ChunkDepthTarget::new(&surface.device, surface.config.width, surface.config.height);
        let draw = match TexturedSectionDrawResources::new(
            &surface.device,
            &surface.queue,
            surface.config.format,
            &sections,
            self.runtime.mesh_assets.atlas.as_upload(),
        ) {
            Ok(draw) => draw,
            Err(err) => {
                log::error!("failed to initialize chunk draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let mut draw = draw;
        draw.set_traversal_ready_sections(
            &self
                .runtime
                .traversal_ready_render_section_keys(initial_camera.eye),
        );
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let sky = SkyRenderer::new(&surface.device, surface.config.format);
        let actors = match ActorDrawResources::new(
            &surface.device,
            &surface.queue,
            surface.config.format,
            self.runtime.actor_textures.atlas.as_upload(),
        ) {
            Ok(actors) => actors,
            Err(err) => {
                log::error!("failed to initialize actor draw resources: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let gui = GuiRenderer::new(&surface.device, surface.config.format);
        self.ui.set_scale(GuiScale::from_pixels(
            surface.config.width,
            surface.config.height,
        ));
        self.render_stats.section_count = draw.section_count();
        self.render_stats.index_count = draw.index_count();
        self.render_stats.face_count = quad_face_count_from_indices(self.render_stats.index_count);
        self.render_stats.drawn_section_count = 0;
        self.render_stats.drawn_face_count = 0;
        self.render_stats.drawn_index_count = 0;
        self.record_section_update_stats(
            &section_update,
            TexturedSectionUploadReport {
                uploaded_section_count: section_update.rebuilt_section_count(),
                removed_section_count: section_update.removed_section_count(),
                uploaded_vertex_count: section_update.rebuilt_vertex_count,
                uploaded_index_count: section_update.rebuilt_index_count,
            },
        );
        self.render_stats.last_remesh_ms = remesh_ms;
        self.render_stats.last_upload_ms = upload_ms;
        log::info!(
            "uploaded {} initial render sections with {} vertices / {} faces / {} indices visgraph_count={} visgraph_total_ms={:.3} visgraph_worst_ms={:.6} remesh_ms={:.3} upload_ms={:.3}",
            draw.section_count(),
            section_update.rebuilt_vertex_count,
            self.render_stats.face_count,
            draw.index_count(),
            section_update.visibility_graph_stats.build_count,
            section_update.visibility_graph_stats.total_ms,
            section_update.visibility_graph_stats.worst_ms,
            remesh_ms,
            upload_ms
        );
        self.depth = Some(depth);
        self.sky = Some(sky);
        self.draw = Some(draw);
        self.actors = Some(actors);
        self.gui = Some(gui);
        self.surface = Some(surface);
        self.window = Some(window);
        self.last_frame = Instant::now();
        self.frame_timing = FrameTimingStats::default();
        self.next_redraw_at = None;
        event_loop.listen_device_events(DeviceEvents::WhenFocused);
        self.sync_mouse_lock();
        self.schedule_next_redraw(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    surface.resize(size);
                }
                if let (Some(surface), Some(depth)) = (&self.surface, &mut self.depth) {
                    depth.resize(&surface.device, surface.config.width, surface.config.height);
                    self.ui.set_scale(GuiScale::from_pixels(
                        surface.config.width,
                        surface.config.height,
                    ));
                }
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    if let Some(scale) = self.gui_scale() {
                        self.ui.set_scale(scale);
                    }
                    if key_code == KeyCode::Backquote
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.debug_visible = !self.debug_visible;
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if event.state == ElementState::Pressed && self.ui.is_active() {
                        let (handled, action) = self.ui.key_pressed(key_code);
                        if let Some(action) = action {
                            self.apply_ui_action(action, event_loop, false);
                        } else if handled {
                            self.schedule_next_redraw(event_loop);
                        }
                        return;
                    }
                    if key_code == KeyCode::Escape
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.mouse_lock_requested = false;
                        self.ui.open_pause();
                        self.camera.clear_keys();
                        self.last_cursor = None;
                        self.sync_mouse_lock();
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == KeyCode::KeyO
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
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
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == KeyCode::KeyL
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.render_options.force_fullbright =
                            !self.render_options.force_fullbright;
                        log::info!(
                            "fullbright {}",
                            if self.render_options.force_fullbright {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if key_code == NO_CLIP_TOGGLE_KEY
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.toggle_movement_mode();
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if event.state == ElementState::Pressed
                        && !event.repeat
                        && let Some(slot) = hotbar_slot_from_key_code(key_code)
                    {
                        if self.interaction.select_hotbar_slot(slot) {
                            log::info!("selected hotbar slot {}", slot + 1);
                        }
                        self.schedule_next_redraw(event_loop);
                        return;
                    }
                    if let Some(input_key) = player_input_key_from_key_code(key_code) {
                        self.camera
                            .set_key(input_key, event.state == ElementState::Pressed);
                    }
                    self.schedule_next_redraw(event_loop);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.ui.is_active() {
                    if button == MouseButton::Left {
                        if let Some((x, y)) = self.last_cursor
                            && let Some(point) = self.gui_point(x, y)
                        {
                            match state {
                                ElementState::Pressed => {
                                    self.ui.pointer_down(point);
                                }
                                ElementState::Released => {
                                    let (_handled, action) = self.ui.pointer_up(point);
                                    if let Some(action) = action {
                                        self.apply_ui_action(action, event_loop, true);
                                        return;
                                    }
                                }
                            }
                        }
                        self.schedule_next_redraw(event_loop);
                    }
                    return;
                }
                if state == ElementState::Pressed {
                    let was_locked = self.mouse_locked;
                    self.mouse_lock_requested = true;
                    self.last_cursor = None;
                    self.sync_mouse_lock();
                    if was_locked
                        && matches!(button, MouseButton::Left | MouseButton::Right)
                        && let Err(err) = self.handle_world_mouse_pressed(button)
                    {
                        log::error!("failed to handle world mouse input: {err:#}");
                        event_loop.exit();
                        return;
                    }
                    self.schedule_next_redraw(event_loop);
                } else if button == MouseButton::Left || button == MouseButton::Right {
                    self.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let cursor = (position.x, position.y);
                if self.ui.is_active() {
                    self.last_cursor = Some(cursor);
                    if let Some(point) = self.gui_point(cursor.0, cursor.1) {
                        let (_handled, action) = self.ui.pointer_move(point);
                        if let Some(action) = action {
                            self.apply_ui_action(action, event_loop, true);
                            return;
                        }
                    }
                    self.schedule_next_redraw(event_loop);
                    return;
                }
                if self.mouse_lock_requested && !self.mouse_locked {
                    if let Some(previous) = self.last_cursor {
                        let dx = (cursor.0 - previous.0) as f32;
                        let dy = (cursor.1 - previous.1) as f32;
                        self.camera.turn_mouse_delta(f64::from(dx), f64::from(dy));
                        sync_spectator_from_camera(&mut self.spectator, &self.camera);
                        self.schedule_next_redraw(event_loop);
                    }
                }
                self.last_cursor = Some(cursor);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.ui.is_active() {
                    return;
                }
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.12,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.001,
                };
                self.camera.adjust_speed(f64::from(amount));
                sync_spectator_from_camera(&mut self.spectator, &self.camera);
                log::info!(
                    "no-clip speed {:.1} blocks/s",
                    self.camera.snapshot().speed_blocks_per_second
                );
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::Focused(false) => {
                self.mouse_lock_requested = false;
                self.camera.clear_keys();
                self.last_cursor = None;
                self.ui.clear_input();
                self.set_mouse_lock(false);
            }
            WindowEvent::Focused(true) => {
                self.sync_mouse_lock();
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                if let Some(window) = &self.window {
                    self.frame_pacing.update_monitor(window);
                }
                if let Err(err) = self.update_camera_from_keys(frame_start) {
                    log::error!("failed to update spectator camera: {err:#}");
                    event_loop.exit();
                    return;
                }
                if let Err(err) = self.poll_runtime_and_upload() {
                    log::error!("failed to poll chunk runtime: {err:#}");
                    event_loop.exit();
                    return;
                }
                let Some(gui_scale) = self.gui_scale() else {
                    return;
                };
                self.ui.set_scale(gui_scale);
                let camera_view = self.window_camera_view();
                let camera = camera_view.chunk_camera(self.runtime.render_distance);
                let sky_clear_color = self.runtime.sky_clear_color();
                let time_of_day = self.runtime.time_of_day();
                let sun_angle = self.runtime.sun_angle();
                let render_options = self.effective_render_options();
                let frame_pacing = self.frame_pacing.ui_state();
                let actor_instances = self.interpolated_actor_instances();
                let debug_stats = self
                    .debug_visible
                    .then(|| self.debug_pane_stats(render_options));
                let traversal_ready_sections = self
                    .runtime
                    .traversal_ready_render_section_keys(camera_view.eye);
                let mut render_stats = self.render_stats;
                let render_start = Instant::now();
                let result = {
                    let (
                        Some(surface),
                        Some(depth),
                        Some(sky),
                        Some(draw),
                        Some(actors),
                        Some(gui),
                    ) = (
                        &mut self.surface,
                        &self.depth,
                        &self.sky,
                        &mut self.draw,
                        &mut self.actors,
                        &mut self.gui,
                    )
                    else {
                        return;
                    };
                    draw.set_traversal_ready_sections(&traversal_ready_sections);
                    self.frame_pacing.apply_to_surface(surface);
                    surface.render_with_report(|frame| {
                        render_full_frame(
                            frame,
                            depth,
                            sky,
                            draw,
                            actors,
                            gui,
                            camera,
                            &actor_instances,
                            sky_clear_color,
                            time_of_day,
                            sun_angle,
                            render_options,
                            frame_pacing,
                            &self.ui,
                            debug_stats,
                            &mut render_stats,
                        )?;
                        Ok(())
                    })
                };
                match result {
                    Ok(report) => {
                        self.frame_timing.record_surface_frame(
                            elapsed_ms(render_start.elapsed()),
                            report.acquire_ms,
                            report.encode_ms,
                            report.submit_ms,
                            report.present_ms,
                        );
                        match report.status {
                            SurfaceFrameStatus::Presented | SurfaceFrameStatus::Skipped => {
                                self.render_stats = render_stats;
                                self.finish_redraw(event_loop, frame_start);
                            }
                            SurfaceFrameStatus::Reconfigured => {
                                self.schedule_next_redraw(event_loop);
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("render failed: {err:#}");
                        event_loop.exit();
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.ui.is_active() || !self.mouse_locked {
            return;
        }
        if let DeviceEvent::MouseMotion { delta } = event {
            let dx = delta.0 as f32;
            let dy = delta.1 as f32;
            self.camera.turn_mouse_delta(f64::from(dx), f64::from(dy));
            sync_spectator_from_camera(&mut self.spectator, &self.camera);
            self.schedule_next_redraw(event_loop);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.schedule_next_redraw(event_loop);
    }
}

fn player_input_key_from_key_code(key_code: KeyCode) -> Option<PlayerInputKey> {
    match key_code {
        KeyCode::KeyW => Some(PlayerInputKey::Forward),
        KeyCode::KeyS => Some(PlayerInputKey::Backward),
        KeyCode::KeyA => Some(PlayerInputKey::Left),
        KeyCode::KeyD => Some(PlayerInputKey::Right),
        KeyCode::Space => Some(PlayerInputKey::Jump),
        KeyCode::KeyX => Some(PlayerInputKey::Descend),
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Some(PlayerInputKey::Shift),
        KeyCode::ControlLeft | KeyCode::ControlRight => Some(PlayerInputKey::Sprint),
        _ => None,
    }
}

fn hotbar_slot_from_key_code(key_code: KeyCode) -> Option<u8> {
    match key_code {
        KeyCode::Digit1 => Some(0),
        KeyCode::Digit2 => Some(1),
        KeyCode::Digit3 => Some(2),
        KeyCode::Digit4 => Some(3),
        KeyCode::Digit5 => Some(4),
        KeyCode::Digit6 => Some(5),
        KeyCode::Digit7 => Some(6),
        KeyCode::Digit8 => Some(7),
        KeyCode::Digit9 => Some(8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_render_options_disable_section_occlusion_inside_occluding_block() {
        let enabled = TexturedSectionRenderOptions {
            section_occlusion_culling: true,
            ..TexturedSectionRenderOptions::default()
        };
        let disabled = TexturedSectionRenderOptions {
            section_occlusion_culling: false,
            ..TexturedSectionRenderOptions::default()
        };

        assert!(effective_render_options_for_camera(enabled, false).section_occlusion_culling);
        assert!(!effective_render_options_for_camera(enabled, true).section_occlusion_culling);
        assert!(!effective_render_options_for_camera(disabled, true).section_occlusion_culling);
    }

    #[test]
    fn actor_instances_follow_client_actor_presentations() {
        let integrated = mclone_client::ClientRuntime::local_integrated();
        assert!(integrated.actor_presentations().is_empty());
        assert!(
            actor_instances_from_presentations(&integrated.actor_presentations(), &integrated)
                .is_empty()
        );

        let mut remote =
            mclone_client::ClientRuntime::new(mclone_client::ClientHost::RemoteDedicated);
        remote.apply_update(mclone_protocol::ServerUpdate::RemotePlayerAdd(
            mclone_protocol::RemotePlayerUpdate {
                id: mclone_protocol::RemotePlayerId(9),
                position: Vec3d::new(1.0, 64.0, 2.0),
                y_rot_degrees: -90.0,
                x_rot_degrees: 0.0,
                on_ground: true,
            },
        ));

        let mut interpolation =
            ActorInterpolationState::from_authoritative(remote.actor_presentations());
        interpolation.step(1.0, ActorInterpolationConfig::default());
        let actors = actor_instances_from_presentations(&interpolation.presentations(), &remote);

        assert_eq!(actors.len(), 1);
        assert_eq!(actors[0].feet_position, glam::Vec3::new(1.0, 64.0, 2.0));
        assert!((actors[0].yaw_radians - 90.0_f32.to_radians()).abs() < 1.0e-6);
        assert_eq!(
            actors[0].shape,
            mclone_render::entity::ActorInstanceShape::Humanoid
        );
        assert_eq!(
            actors[0].packed_light,
            mclone_render::light_texture::FULL_BRIGHT
        );

        let mut entity_client =
            mclone_client::ClientRuntime::new(mclone_client::ClientHost::RemoteDedicated);
        entity_client.apply_update(mclone_protocol::ServerUpdate::EntitySnapshot(
            mclone_protocol::EntitySnapshot {
                id: mclone_protocol::EntityId(1),
                kind: EntityKind::Cow,
                position: Vec3d::new(3.0, 64.0, 4.0),
                y_rot_degrees: 45.0,
                x_rot_degrees: 0.0,
                on_ground: true,
                width: 0.9,
                height: 1.4,
                age_ticks: 0,
            },
        ));
        let entity_actors = actor_instances_from_presentations(
            &entity_client.actor_presentations(),
            &entity_client,
        );

        assert_eq!(entity_actors.len(), 1);
        assert_eq!(
            entity_actors[0].shape,
            mclone_render::entity::ActorInstanceShape::CowModel
        );
        assert_eq!(entity_actors[0].width, 0.9);
        assert_eq!(entity_actors[0].height, 1.4);
        assert_eq!(
            entity_actors[0].packed_light,
            mclone_render::light_texture::FULL_BRIGHT
        );
    }

    #[test]
    fn commit_player_pose_change_syncs_spectator_and_interest_center() {
        let scene = SceneOptions {
            render_distance: 1,
            ..SceneOptions::default()
        };
        let runtime = WindowSceneRuntime::new(&scene).unwrap();
        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let mut app = ChunkApp::new(
            runtime,
            spectator,
            TexturedSectionRenderOptions::default(),
            scene.render_distance,
        );
        let target_eye = Vec3d::new(16.25, 96.0, 8.0);
        app.camera.set_eye_pose(target_eye, 0.0, 0.0);

        assert!(app.commit_player_pose_change().unwrap());

        assert_eq!(
            app.spectator.position,
            glam::Vec3::new(
                target_eye.x as f32,
                target_eye.y as f32,
                target_eye.z as f32
            )
        );
        assert_eq!(
            app.camera_frame_state().camera.chunk_pos,
            mclone_core::ChunkPos::new(1, 0)
        );
        assert_eq!(
            app.runtime.stats().interest_center,
            mclone_core::ChunkPos::new(1, 0)
        );
    }

    #[test]
    fn window_camera_view_uses_controller_snapshot_not_spectator_mirror() {
        let scene = SceneOptions::default();
        let runtime = WindowSceneRuntime::new(&scene).unwrap();
        let spectator = SpectatorCamera::spawn_for_scene(&scene);
        let mut app = ChunkApp::new(
            runtime,
            spectator,
            TexturedSectionRenderOptions::default(),
            scene.render_distance,
        );
        let target_eye = Vec3d::new(32.25, 80.0, -0.25);

        app.camera.set_eye_pose(target_eye, 0.25, -0.125);
        let view = app.window_camera_view();

        assert_eq!(
            view.eye,
            glam::Vec3::new(
                target_eye.x as f32,
                target_eye.y as f32,
                target_eye.z as f32
            )
        );
        assert_eq!(view.snapshot.chunk_pos, mclone_core::ChunkPos::new(2, -1));
        assert_ne!(app.spectator.position, view.eye);
    }

    #[test]
    fn native_keys_map_to_platform_neutral_player_input() {
        assert_eq!(
            player_input_key_from_key_code(KeyCode::KeyW),
            Some(PlayerInputKey::Forward)
        );
        assert_eq!(
            player_input_key_from_key_code(KeyCode::KeyS),
            Some(PlayerInputKey::Backward)
        );
        assert_eq!(
            player_input_key_from_key_code(KeyCode::KeyA),
            Some(PlayerInputKey::Left)
        );
        assert_eq!(
            player_input_key_from_key_code(KeyCode::KeyD),
            Some(PlayerInputKey::Right)
        );
        assert_eq!(
            player_input_key_from_key_code(KeyCode::Space),
            Some(PlayerInputKey::Jump)
        );
        assert_eq!(
            player_input_key_from_key_code(KeyCode::KeyX),
            Some(PlayerInputKey::Descend)
        );
        assert_eq!(
            player_input_key_from_key_code(KeyCode::ShiftLeft),
            Some(PlayerInputKey::Shift)
        );
        assert_eq!(
            player_input_key_from_key_code(KeyCode::ControlLeft),
            Some(PlayerInputKey::Sprint)
        );
        assert_eq!(player_input_key_from_key_code(NO_CLIP_TOGGLE_KEY), None);
        assert_eq!(player_input_key_from_key_code(KeyCode::KeyO), None);
        assert_eq!(player_input_key_from_key_code(KeyCode::Digit1), None);
    }

    #[test]
    fn native_number_keys_map_to_hotbar_slots() {
        assert_eq!(hotbar_slot_from_key_code(KeyCode::Digit1), Some(0));
        assert_eq!(hotbar_slot_from_key_code(KeyCode::Digit5), Some(4));
        assert_eq!(hotbar_slot_from_key_code(KeyCode::Digit9), Some(8));
        assert_eq!(hotbar_slot_from_key_code(KeyCode::KeyW), None);
    }

    #[test]
    fn player_movement_mode_toggles_between_walking_and_no_clip() {
        assert_eq!(
            EngineCameraMovementMode::Walking.toggled(),
            EngineCameraMovementMode::NoClip
        );
        assert_eq!(
            EngineCameraMovementMode::NoClip.toggled(),
            EngineCameraMovementMode::Walking
        );
        assert_eq!(EngineCameraMovementMode::Walking.label(), "WALK");
        assert_eq!(EngineCameraMovementMode::NoClip.label(), "NOCLIP");
    }
}
