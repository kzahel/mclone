use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_render::FullFrameRenderSummary;
use mclone_client::{ClientHost, ClientInteractionController, LOCAL_PLAYER_STANDING_EYE_HEIGHT};
use mclone_core::{HitResultType, Vec3d};
use mclone_protocol::{ClientCommand, MovePlayerCommand};
use mclone_render::color_profile::{DEFAULT_RENDER_SCALE, RenderConfig};
use mclone_render::headless::{HeadlessFrameLoopOptions, run_headless_capture_loop, save_rgba_png};
use mclone_render::target::RenderFrameContext;
use mclone_render_session::EngineCameraMovementMode;
use mclone_ui::GuiScale;

use crate::camera::SpectatorCamera;
use crate::cli::{HeadlessScreenshotOptions, HeadlessScreenshotUi, SceneOptions};
use crate::flat_client_driver::{
    FlatClientDebugFrame, FlatClientDriver, FlatClientUiFrame, FlatClientUiRenderOptions,
};
use crate::frame_pacing::{FramePacingDebugStats, FramePacingUiState};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowSceneAssets, WindowSceneRuntime, WindowSceneStartupPump};
use crate::ui::DebugPaneStats;

const DEFAULT_OFFSCREEN_FRAME_MS: f64 = 1000.0 / 60.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OffscreenFlatClientScreenshotReport {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) summary: FullFrameRenderSummary,
    pub(crate) remote_player_count: usize,
    pub(crate) entity_count: usize,
    pub(crate) underwater: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OffscreenFlatClientFrameClock {
    pub(crate) frame_ms: f64,
    pub(crate) target_frame_ms: Option<f64>,
}

impl Default for OffscreenFlatClientFrameClock {
    fn default() -> Self {
        Self {
            frame_ms: DEFAULT_OFFSCREEN_FRAME_MS,
            target_frame_ms: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OffscreenFlatClientFrameOptions {
    pub(crate) debug_pane: bool,
}

pub(crate) struct OffscreenFlatClientHost {
    assets: WindowSceneAssets,
    _asset_source: mclone_assets::AssetSourceChain,
    pub(crate) driver: FlatClientDriver,
    target_size: [u32; 2],
    clock: OffscreenFlatClientFrameClock,
    last_summary: Option<FullFrameRenderSummary>,
}

impl OffscreenFlatClientHost {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        target_size: [u32; 2],
        scene: &SceneOptions,
        render_options: mclone_render::chunk::TexturedSectionRenderOptions,
        assets: WindowSceneAssets,
        asset_source: mclone_assets::AssetSourceChain,
    ) -> Result<Self> {
        let render_config = RenderConfig::for_color_target(render_options.color_profile, format);
        let mut driver = FlatClientDriver::new(scene, render_options);
        driver.set_ui_scale(GuiScale::from_pixels(target_size[0], target_size[1]));
        driver.rebuild_render_resources(
            device,
            queue,
            target_size,
            render_config,
            assets.mesh_assets.atlas.as_upload(),
            assets.actor_textures.atlas.as_upload(),
            &asset_source,
        )?;
        Ok(Self {
            assets,
            _asset_source: asset_source,
            driver,
            target_size,
            clock: OffscreenFlatClientFrameClock::default(),
            last_summary: None,
        })
    }

    pub(crate) fn start_scene_now(
        &mut self,
        device: &wgpu::Device,
        scene: SceneOptions,
    ) -> Result<()> {
        let assets = &self.assets;
        self.driver
            .start_world_from_scene(scene, Some(device), &mut |scene| {
                WindowSceneRuntime::with_assets(scene, assets)
            })
            .context("failed to start offscreen flat client scene")?;
        self.require_uploaded_sections()?;
        Ok(())
    }

    pub(crate) fn render_frame(
        &mut self,
        index: usize,
        frame: RenderFrameContext<'_>,
        options: OffscreenFlatClientFrameOptions,
    ) -> Result<FullFrameRenderSummary> {
        self.driver
            .tick_frame_timing(self.clock.frame_ms, self.clock.target_frame_ms);
        self.driver.set_ui_scale(GuiScale::from_pixels(
            self.target_size[0],
            self.target_size[1],
        ));

        let assets = &self.assets;
        let session_update = self.driver.finish_pending_session_start(
            Some(frame.device),
            |scene| WindowSceneRuntime::with_assets(scene, assets),
            |scene| WindowSceneStartupPump::new_local(scene, assets),
        );
        let startup_update = self.driver.advance_local_world_startup(Some(frame.device));
        if session_update.mouse_lock_requested.is_some()
            || startup_update.mouse_lock_requested.is_some()
        {
            self.driver.clear_camera_input();
        }

        if self.driver.poll_runtime()?.needs_section_upload {
            self.driver
                .upload_runtime_sections(frame.device, |elapsed| elapsed.as_secs_f64() * 1000.0)?;
        }

        let debug_stats = self.debug_pane_stats(options.debug_pane);
        let debug_view_readiness_overlay = debug_stats
            .is_some()
            .then(|| {
                self.driver
                    .runtime
                    .as_ref()
                    .and_then(WindowSceneRuntime::view_readiness_overlay)
            })
            .flatten();
        let ui_frame = FlatClientUiFrame {
            render_options: FlatClientUiRenderOptions {
                render_distance: self
                    .driver
                    .current_render_distance(self.driver.scene.render_distance)
                    as i32,
                render_options: self.driver.render_options,
                frame_pacing: FramePacingUiState::default(),
                fly_enabled: self.driver.camera.movement_mode() == EngineCameraMovementMode::NoClip,
                fly_speed_multiplier: self.driver.camera.fly_speed_multiplier() as f32,
                movement_speed_multiplier: self.driver.camera.movement_speed_multiplier() as f32,
            },
            hud: None,
            loading_progress_overlay: self.driver.startup_progress_overlay(),
            debug: FlatClientDebugFrame {
                stats: debug_stats,
                view_readiness_overlay: debug_view_readiness_overlay,
            },
        };
        let summary = self.driver.render_full_frame_with_ui(
            frame,
            self.driver.scene.render_distance,
            ui_frame,
        )?;
        self.last_summary = Some(summary);
        log::trace!(
            "offscreen flat client rendered frame {} sections={} drawn_sections={}",
            index,
            summary.section_count,
            summary.drawn_section_count
        );
        Ok(summary)
    }

    fn debug_pane_stats(&self, enabled: bool) -> Option<DebugPaneStats> {
        if !enabled {
            return None;
        }
        let runtime = self.driver.runtime.as_ref()?;
        Some(DebugPaneStats {
            position: self.driver.spectator.position,
            speed: self.driver.spectator.speed,
            movement_mode: self.driver.camera.movement_mode().label(),
            on_ground: self.driver.camera.on_ground(),
            runtime: runtime.stats(),
            render: self.driver.render_stats,
            frame: self.driver.frame_timing,
            pacing: FramePacingDebugStats::default(),
            section_occlusion: self.driver.render_options.section_occlusion_culling,
            force_fullbright: self.driver.render_options.force_fullbright,
            color_profile: self.driver.render_options.color_profile.label(),
            render_scale: self.driver.current_render_scale(DEFAULT_RENDER_SCALE),
        })
    }

    fn require_uploaded_sections(&self) -> Result<()> {
        if self.driver.render_stats.section_count == 0 {
            bail!(
                "offscreen flat client seed={} center=({}, {}) render_distance={} produced no render sections",
                self.driver.scene.seed,
                self.driver.scene.chunk_x,
                self.driver.scene.chunk_z,
                self.driver.scene.render_distance
            );
        }
        Ok(())
    }

    fn force_day_time(&mut self, day_time: u64) {
        if let Some(runtime) = &mut self.driver.runtime {
            runtime.force_day_time(day_time);
        }
    }

    fn settle_remote_session(&mut self, remote_settle_ms: u64) -> Result<()> {
        if remote_settle_ms == 0 {
            return Ok(());
        }
        let Some(runtime) = &mut self.driver.runtime else {
            return Ok(());
        };
        if runtime.client().host() != ClientHost::RemoteDedicated {
            return Ok(());
        }

        std::thread::sleep(Duration::from_millis(remote_settle_ms));
        runtime
            .send_gameplay_command(ClientCommand::MovePlayer(MovePlayerCommand::StatusOnly {
                on_ground: false,
            }))
            .context("failed to poll remote offscreen session after settle delay")?;
        Ok(())
    }

    fn apply_scripted_interaction(&mut self) -> Result<()> {
        let initial_player_position = self.driver.camera.player().pose().position;
        let mut spectator = self.driver.spectator.clone();
        {
            let runtime = self
                .driver
                .runtime
                .as_mut()
                .context("offscreen scripted interaction requires an active runtime")?;
            apply_scripted_interaction(runtime, &mut spectator, initial_player_position)?;
        }
        self.driver
            .set_spectator_camera(spectator, self.driver.scene.movement_speed_multiplier);
        Ok(())
    }

    fn frame_first_actor(&mut self) {
        let mut spectator = self.driver.spectator.clone();
        if let Some(runtime) = self.driver.runtime.as_ref() {
            frame_first_actor(runtime, &mut spectator);
        }
        self.driver
            .set_spectator_camera(spectator, self.driver.scene.movement_speed_multiplier);
    }

    fn set_eye_override(&mut self, eye: [f32; 3]) {
        let mut spectator = self.driver.spectator.clone();
        spectator.position = Vec3::from_array(eye);
        self.driver
            .set_spectator_camera(spectator, self.driver.scene.movement_speed_multiplier);
    }
}

pub(crate) fn run_offscreen_flat_client_screenshot(
    options: &HeadlessScreenshotOptions,
) -> Result<OffscreenFlatClientScreenshotReport> {
    let assets = WindowSceneAssets::load()?;
    let asset_source = load_asset_source()?;
    let scene = options.scene.clone();
    let render_options = options.render_options;
    let (loop_report, frame_pixels, host) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: 1,
        },
        move |device, queue, format, size| {
            let mut host = OffscreenFlatClientHost::new(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                assets,
                asset_source,
            )?;
            host.start_scene_now(device, scene.clone())?;
            configure_screenshot_scene(&mut host, options)?;
            Ok(host)
        },
        |index, frame, host| {
            host.render_frame(
                index,
                frame,
                OffscreenFlatClientFrameOptions {
                    debug_pane: options.debug_pane,
                },
            )?;
            Ok(())
        },
    )?;

    let pixels = frame_pixels
        .last()
        .context("offscreen flat client screenshot produced no captured frame")?;
    save_rgba_png(&options.path, loop_report.width, loop_report.height, pixels)?;
    let summary = host
        .last_summary
        .context("offscreen flat client screenshot rendered no frame summary")?;
    let remote_player_count = host
        .driver
        .runtime
        .as_ref()
        .map_or(0, |runtime| runtime.client().remote_player_count());
    let entity_count = host
        .driver
        .runtime
        .as_ref()
        .map_or(0, |runtime| runtime.client().entity_count());
    let underwater = host.underwater();

    Ok(OffscreenFlatClientScreenshotReport {
        path: options.path.clone(),
        width: loop_report.width,
        height: loop_report.height,
        byte_len: pixels.len(),
        summary,
        remote_player_count,
        entity_count,
        underwater,
    })
}

fn configure_screenshot_scene(
    host: &mut OffscreenFlatClientHost,
    options: &HeadlessScreenshotOptions,
) -> Result<()> {
    if options.ui == HeadlessScreenshotUi::NewWorld {
        host.driver.set_new_world_seed(options.scene.seed);
    }
    host.driver.set_ui_screen(options.ui.game_screen());
    if let Some(day_time) = options.scene.day_time_override {
        host.force_day_time(day_time);
    }
    host.settle_remote_session(options.remote_settle_ms)?;
    if options.scripted_interaction {
        host.apply_scripted_interaction()?;
    }
    host.frame_first_actor();
    if let Some(eye) = options.eye {
        host.set_eye_override(eye);
    }
    Ok(())
}

impl OffscreenFlatClientHost {
    fn underwater(&self) -> bool {
        self.driver
            .underwater_overlay(self.driver.camera_view())
            .is_some()
    }
}

fn apply_scripted_interaction(
    runtime: &mut WindowSceneRuntime,
    spectator: &mut SpectatorCamera,
    initial_player_position: Vec3d,
) -> Result<()> {
    let (base_x, base_z) = spectator.block_column();
    let target_x = base_x;
    let target_z = base_z + 4;
    let surface_y = runtime
        .highest_non_air_block_y_at_world(target_x, target_z)
        .with_context(|| {
            format!("no loaded surface for scripted interaction at ({target_x}, {target_z})")
        })?;
    let interaction = ClientInteractionController::new();
    let eye_position = Vec3d::new(
        target_x as f64 + 0.5,
        surface_y as f64 + 3.0,
        target_z as f64 + 0.5,
    );
    let player_feet_position =
        eye_position.add(Vec3d::new(0.0, -LOCAL_PLAYER_STANDING_EYE_HEIGHT, 0.0));
    sync_scripted_player_position(runtime, initial_player_position, player_feet_position)?;
    let hit = interaction.pick_block(runtime.client(), eye_position, Vec3d::new(0.0, -1.0, 0.0));
    if hit.hit_type() != HitResultType::Block {
        bail!("scripted interaction ray missed target column");
    }
    let break_command = interaction
        .debug_instant_break_command(hit)
        .context("scripted interaction did not produce break command")?;
    let break_changed = runtime.send_gameplay_command(break_command)?;
    let place_command = interaction
        .use_item_on_command(hit)
        .context("scripted interaction did not produce place command")?;
    let place_changed = runtime.send_gameplay_command(place_command)?;
    if !break_changed || !place_changed {
        bail!(
            "scripted interaction did not mutate both blocks: break_changed={break_changed} place_changed={place_changed}"
        );
    }

    spectator.position = glam::Vec3::new(
        target_x as f32 + 0.5,
        surface_y as f32 + 5.0,
        target_z as f32 - 6.0,
    );
    spectator.yaw = 0.0;
    spectator.pitch = -0.7;
    Ok(())
}

fn sync_scripted_player_position(
    runtime: &mut WindowSceneRuntime,
    mut current: Vec3d,
    target: Vec3d,
) -> Result<()> {
    for _ in 0..64 {
        let delta = target.subtract(current);
        if delta.length_sqr() <= 64.0 {
            send_scripted_player_move(runtime, target)?;
            return Ok(());
        }
        let length = delta.length_sqr().sqrt();
        current = current.add(delta.scale(8.0 / length));
        send_scripted_player_move(runtime, current)?;
        runtime.poll()?;
    }
    bail!("timed out moving scripted player to interaction target");
}

fn send_scripted_player_move(runtime: &mut WindowSceneRuntime, position: Vec3d) -> Result<()> {
    runtime
        .send_gameplay_command(ClientCommand::MovePlayer(MovePlayerCommand::PosRot {
            position,
            y_rot_degrees: 0.0,
            x_rot_degrees: 90.0,
            on_ground: false,
        }))
        .context("failed to sync scripted player position")?;
    Ok(())
}

fn frame_first_actor(runtime: &WindowSceneRuntime, spectator: &mut SpectatorCamera) {
    let Some(actor) = runtime.client().actor_presentations().first().copied() else {
        return;
    };
    let target = glam::Vec3::new(
        actor.feet_position.x as f32,
        actor.feet_position.y as f32 + 1.0,
        actor.feet_position.z as f32,
    );
    let eye = target + glam::Vec3::new(-2.2, 1.4, -4.8);
    aim_spectator_at(spectator, eye, target);
}

fn aim_spectator_at(spectator: &mut SpectatorCamera, eye: glam::Vec3, target: glam::Vec3) {
    let direction = target - eye;
    let Some(direction) = direction.try_normalize() else {
        return;
    };
    spectator.position = eye;
    spectator.yaw = direction.x.atan2(direction.z);
    spectator.pitch = direction.y.clamp(-1.0, 1.0).asin();
}
