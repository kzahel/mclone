use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use mclone_client::{
    ClientInteractionController, LOCAL_PLAYER_STANDING_EYE_HEIGHT, LocalPlayerController,
    LocalPlayerPose, NoClipMovementStep, PlayerInputKey, WalkingMovementStep,
};
use mclone_core::Vec3d;
use mclone_mesh::quad_face_count_from_indices;
use mclone_render::chunk::{
    ChunkCamera, ChunkDepthTarget, ChunkRenderTarget, TexturedSectionDrawResources,
    TexturedSectionRenderOptions, TexturedSectionUploadReport,
};
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

use crate::camera::{SPECTATOR_MOUSE_SENSITIVITY, SpectatorCamera};
use crate::cli::SceneOptions;
use crate::frame_pacing::{
    FramePacing, FramePacingMode, FramePacingUiState, FrameTimingStats, RedrawSchedule, elapsed_ms,
    next_capped_redraw_deadline, redraw_schedule,
};
use crate::render_cache::RenderSectionCacheUpdate;
use crate::scene_runtime::{WindowSceneRuntime, poll_window_runtime_until_idle};
use crate::ui::{DebugPaneStats, NativeUi, NativeUiAction};
use crate::{MAX_RENDER_DISTANCE, MIN_RENDER_DISTANCE};

const NO_CLIP_TOGGLE_KEY: KeyCode = KeyCode::KeyN;
const PLAYER_SURFACE_FEET_OFFSET: f64 = 1.0;
const GROUND_PROBE_DISTANCE: f64 = 0.01;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum PlayerMovementMode {
    #[default]
    Walking,
    NoClip,
}

impl PlayerMovementMode {
    const fn toggled(self) -> Self {
        match self {
            Self::Walking => Self::NoClip,
            Self::NoClip => Self::Walking,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Walking => "WALK",
            Self::NoClip => "NOCLIP",
        }
    }
}

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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FullFrameRenderSummary {
    pub(crate) section_count: usize,
    pub(crate) drawn_section_count: usize,
    pub(crate) index_count: u32,
    pub(crate) drawn_index_count: u32,
    pub(crate) gui_command_count: usize,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_full_frame(
    frame: RenderFrameContext<'_>,
    depth: &ChunkDepthTarget,
    sky: &SkyRenderer,
    draw: &mut TexturedSectionDrawResources,
    gui: &mut GuiRenderer,
    camera: ChunkCamera,
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
    } else {
        render_stats.drawn_section_count = 0;
        render_stats.drawn_face_count = 0;
        render_stats.drawn_index_count = 0;
    }

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
    })
}

struct ChunkApp {
    runtime: WindowSceneRuntime,
    spectator: SpectatorCamera,
    player: LocalPlayerController,
    movement_mode: PlayerMovementMode,
    interaction: ClientInteractionController,
    render_options: TexturedSectionRenderOptions,
    frame_pacing: FramePacing,
    ui: NativeUi,
    window: Option<Arc<Window>>,
    surface: Option<NativeSurfaceContext>,
    depth: Option<ChunkDepthTarget>,
    sky: Option<SkyRenderer>,
    draw: Option<TexturedSectionDrawResources>,
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
        let mut player = LocalPlayerController::new();
        player.set_pose(local_player_pose_from_spectator(&spectator));
        Self {
            runtime,
            spectator,
            player,
            movement_mode: PlayerMovementMode::default(),
            interaction: ClientInteractionController::new(),
            render_options,
            frame_pacing: FramePacing::default(),
            ui: NativeUi::new_ingame(render_distance),
            window: None,
            surface: None,
            depth: None,
            sky: None,
            draw: None,
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

        match self.movement_mode {
            PlayerMovementMode::Walking => {
                let pose = self.player.pose();
                if self
                    .player
                    .tick_walking_movement(
                        &self.runtime.client,
                        WalkingMovementStep {
                            y_rot_degrees: pose.y_rot_degrees,
                            dt_seconds: movement_dt as f64,
                        },
                    )
                    .is_some()
                {
                    sync_spectator_from_player_pose(&mut self.spectator, self.player.pose());
                    self.update_interest_from_spectator()?;
                }
            }
            PlayerMovementMode::NoClip => {
                let pose = self.player.pose();
                if self
                    .player
                    .tick_no_clip_movement(NoClipMovementStep {
                        yaw_radians: pose.native_yaw_radians(),
                        pitch_radians: pose.native_pitch_radians(),
                        speed_blocks_per_second: self.spectator.speed as f64,
                        dt_seconds: movement_dt as f64,
                        descending: false,
                        sprinting: false,
                    })
                    .is_some()
                {
                    sync_spectator_from_player_pose(&mut self.spectator, self.player.pose());
                    self.update_interest_from_spectator()?;
                }
            }
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
        self.player.clear_keys();
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
        self.movement_mode = self.movement_mode.toggled();
        self.player.clear_delta_movement();
        log::info!("player movement mode {}", self.movement_mode.label());
    }

    fn place_spectator_above_loaded_surface(&mut self) {
        let (world_x, world_z) = self.spectator.block_column();
        let Some(surface_y) = self
            .runtime
            .highest_non_air_block_y_at_world(world_x, world_z)
        else {
            log::warn!(
                "no loaded surface column found for initial spectator at ({world_x}, {world_z})"
            );
            return;
        };
        self.spectator.pitch = crate::camera::SPECTATOR_SURFACE_PITCH;
        let eye_position = Vec3d::new(
            self.spectator.position.x as f64,
            surface_y as f64 + PLAYER_SURFACE_FEET_OFFSET + LOCAL_PLAYER_STANDING_EYE_HEIGHT,
            self.spectator.position.z as f64,
        );
        self.player.set_pose(LocalPlayerPose::from_eye_position(
            eye_position,
            -(self.spectator.yaw as f64).to_degrees(),
            -(self.spectator.pitch as f64).to_degrees(),
            LOCAL_PLAYER_STANDING_EYE_HEIGHT,
        ));
        if self.movement_mode == PlayerMovementMode::Walking {
            self.player.move_colliding(
                &self.runtime.client,
                Vec3d::new(0.0, -GROUND_PROBE_DISTANCE, 0.0),
            );
        }
        sync_spectator_from_player_pose(&mut self.spectator, self.player.pose());
        log::info!(
            "placed player above loaded surface column ({world_x}, {world_z}) y={} -> feet_y={:.1} eye_y={:.1}",
            surface_y,
            self.player.pose().position.y,
            self.spectator.position.y
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

    fn update_interest_from_spectator(&mut self) -> Result<()> {
        let center = self.spectator.chunk_pos();
        if self.runtime.set_interest_center(center)? {
            log::info!(
                "chunk interest moved to ({}, {}) at spectator position ({:.1}, {:.1}, {:.1})",
                center.x,
                center.z,
                self.spectator.position.x,
                self.spectator.position.y,
                self.spectator.position.z
            );
        }
        Ok(())
    }

    fn poll_runtime_and_upload(&mut self) -> Result<()> {
        let poll_start = Instant::now();
        let changed = self.runtime.poll()?;
        self.frame_timing
            .record_runtime_poll(elapsed_ms(poll_start.elapsed()));
        if !changed
            && !self
                .runtime
                .has_pending_render_work(self.spectator.position)
        {
            return Ok(());
        }
        self.upload_runtime_sections()?;
        Ok(())
    }

    fn upload_runtime_sections(&mut self) -> Result<()> {
        let (Some(surface), Some(draw)) = (&self.surface, &mut self.draw) else {
            return Ok(());
        };
        let remesh_start = Instant::now();
        let section_update = self.runtime.sync_render_sections(self.spectator.position)?;
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
            self.runtime.client.loaded_chunk_count(),
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
                .camera_inside_occluding_block(self.spectator.position),
        )
    }

    fn debug_pane_stats(&self, render_options: TexturedSectionRenderOptions) -> DebugPaneStats {
        DebugPaneStats {
            position: self.spectator.position,
            speed: self.spectator.speed,
            movement_mode: self.movement_mode.label(),
            on_ground: self.player.on_ground(),
            runtime: self.runtime.stats(),
            render: self.render_stats,
            frame: self.frame_timing,
            pacing: self.frame_pacing.debug_stats(),
            section_occlusion: render_options.section_occlusion_culling,
            force_fullbright: render_options.force_fullbright,
        }
    }

    fn handle_world_mouse_pressed(&mut self, button: MouseButton) -> Result<()> {
        let pose = self.player.pose();
        let hit = self.interaction.pick_block(
            &self.runtime.client,
            pose.eye_position(),
            pose.view_vector(),
        );
        let command = match button {
            MouseButton::Left => self
                .interaction
                .debug_instant_break_command(hit, pose.position),
            MouseButton::Right => self
                .interaction
                .debug_place_block_command(hit, pose.position),
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

fn local_player_pose_from_spectator(spectator: &SpectatorCamera) -> LocalPlayerPose {
    LocalPlayerPose::from_eye_position(
        vec3d_from_glam(spectator.position),
        -(spectator.yaw as f64).to_degrees(),
        -(spectator.pitch as f64).to_degrees(),
        LOCAL_PLAYER_STANDING_EYE_HEIGHT,
    )
}

fn sync_spectator_from_player_pose(spectator: &mut SpectatorCamera, pose: LocalPlayerPose) {
    spectator.position = glam_vec3_from_vec3d(pose.eye_position());
    spectator.yaw = pose.native_yaw_radians() as f32;
    spectator.pitch = pose.native_pitch_radians() as f32;
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
        self.place_spectator_above_loaded_surface();
        log::info!(
            "initial light-ready chunks loaded in {} polls poll_ms={:.3} elapsed_ms={:.3} loaded={}",
            initial_poll_count,
            initial_poll_ms,
            elapsed_ms(initial_poll_start.elapsed()),
            self.runtime.client.loaded_chunk_count()
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
        let section_update = match self
            .runtime
            .sync_all_render_sections(self.spectator.position)
        {
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
                .traversal_ready_render_section_keys(self.spectator.position),
        );
        let upload_ms = elapsed_ms(upload_start.elapsed());
        let sky = SkyRenderer::new(&surface.device, surface.config.format);
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
                        self.player.clear_keys();
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
                    if let Some(input_key) = player_input_key_from_key_code(key_code) {
                        self.player
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
                        self.player.turn_native_radians(
                            (-dx * SPECTATOR_MOUSE_SENSITIVITY) as f64,
                            (-dy * SPECTATOR_MOUSE_SENSITIVITY) as f64,
                        );
                        sync_spectator_from_player_pose(&mut self.spectator, self.player.pose());
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
                self.spectator.adjust_speed(amount);
                log::info!("no-clip speed {:.1} blocks/s", self.spectator.speed);
                self.schedule_next_redraw(event_loop);
            }
            WindowEvent::Focused(false) => {
                self.mouse_lock_requested = false;
                self.player.clear_keys();
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
                let camera = self.spectator.camera(self.runtime.render_distance);
                let sky_clear_color = self.runtime.sky_clear_color();
                let time_of_day = self.runtime.time_of_day();
                let sun_angle = self.runtime.sun_angle();
                let render_options = self.effective_render_options();
                let frame_pacing = self.frame_pacing.ui_state();
                let debug_stats = self
                    .debug_visible
                    .then(|| self.debug_pane_stats(render_options));
                let traversal_ready_sections = self
                    .runtime
                    .traversal_ready_render_section_keys(self.spectator.position);
                let mut render_stats = self.render_stats;
                let render_start = Instant::now();
                let result = {
                    let (Some(surface), Some(depth), Some(sky), Some(draw), Some(gui)) = (
                        &mut self.surface,
                        &self.depth,
                        &self.sky,
                        &mut self.draw,
                        &mut self.gui,
                    ) else {
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
                            gui,
                            camera,
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
            self.player.turn_native_radians(
                (-dx * SPECTATOR_MOUSE_SENSITIVITY) as f64,
                (-dy * SPECTATOR_MOUSE_SENSITIVITY) as f64,
            );
            sync_spectator_from_player_pose(&mut self.spectator, self.player.pose());
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
    }

    #[test]
    fn player_movement_mode_toggles_between_walking_and_no_clip() {
        assert_eq!(
            PlayerMovementMode::Walking.toggled(),
            PlayerMovementMode::NoClip
        );
        assert_eq!(
            PlayerMovementMode::NoClip.toggled(),
            PlayerMovementMode::Walking
        );
        assert_eq!(PlayerMovementMode::Walking.label(), "WALK");
        assert_eq!(PlayerMovementMode::NoClip.label(), "NOCLIP");
    }
}
