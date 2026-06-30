use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::{debug_block_palette_overlay, frame_render::FullFrameRenderSummary};
use mclone_client::ClientHost;
use mclone_input::{FlatInputAction, FlatInputFrame, FlatInputIntent};
use mclone_protocol::{ClientCommand, MovePlayerCommand};
use mclone_render::color_profile::{DEFAULT_RENDER_SCALE, RenderConfig};
use mclone_render::headless::{HeadlessFrameLoopOptions, run_headless_capture_loop, save_rgba_png};
use mclone_render::target::RenderFrameContext;
use mclone_server::initial_spawn_center_for_seed;
use mclone_ui::GuiScale;

use crate::camera::SpectatorCamera;
use crate::cli::{
    HeadlessScreenshotOptions, HeadlessScreenshotUi, SceneOptions, StartupWaitPolicy,
};
use crate::flat_client_driver::{
    FlatClientDebugFrame, FlatClientDriver, FlatClientUiFrame, FlatClientUiRenderOptions,
    FlatClientWorldActionStatus, game_movement_mode,
};
use crate::frame_pacing::{FramePacingDebugStats, FramePacingUiState};
use crate::render_cache::load_asset_source;
use crate::scene_runtime::{WindowSceneAssets, WindowSceneRuntime, WindowSceneStartupPump};
use crate::ui::DebugPaneStats;

const DEFAULT_OFFSCREEN_FRAME_MS: f64 = 1000.0 / 60.0;
const MAX_OFFSCREEN_PLAYABLE_STARTUP_STEPS: usize = 65_536;

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

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OffscreenScript {
    steps: Vec<OffscreenScriptStep>,
}

impl OffscreenScript {
    pub(crate) fn from_steps(steps: impl IntoIterator<Item = OffscreenScriptStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    #[cfg(test)]
    fn steps(&self) -> &[OffscreenScriptStep] {
        &self.steps
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum OffscreenScriptStep {
    SetCameraLookAt {
        eye: Vec3,
        target: Vec3,
    },
    SetCameraPose {
        position: Vec3,
        yaw: f32,
        pitch: f32,
    },
    InputFrame {
        frame: FlatInputFrame,
        require_changed_action: Option<FlatInputAction>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OffscreenScriptReport {
    pub(crate) input_frame_count: usize,
    pub(crate) world_action_count: usize,
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
            Some(&assets.actor_textures.player_figure),
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

    pub(crate) fn start_scene_with_wait_policy(
        &mut self,
        device: &wgpu::Device,
        scene: SceneOptions,
        startup_wait: StartupWaitPolicy,
    ) -> Result<()> {
        match startup_wait {
            StartupWaitPolicy::Idle => self.start_scene_idle(device, scene),
            StartupWaitPolicy::None
            | StartupWaitPolicy::Playable
            | StartupWaitPolicy::Frames(_) => self.start_scene_playable(device, scene),
        }
    }

    pub(crate) fn start_scene_idle(
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

    fn start_scene_playable(&mut self, device: &wgpu::Device, scene: SceneOptions) -> Result<()> {
        self.anchor_local_playable_startup_camera(&scene);
        self.driver.scene = scene;
        self.driver.request_current_scene_start(false, false);

        let assets = &self.assets;
        let session_update = self.driver.finish_pending_session_start(
            Some(device),
            |scene| WindowSceneRuntime::with_assets(scene, assets),
            |scene| WindowSceneStartupPump::new_local(scene, assets),
        );
        if session_update.mouse_lock_requested.is_some() {
            self.driver.clear_camera_input();
        }

        for _ in 0..MAX_OFFSCREEN_PLAYABLE_STARTUP_STEPS {
            if self.driver.startup.is_none() {
                break;
            }
            let startup_update = self.driver.advance_local_world_startup(Some(device));
            if startup_update.mouse_lock_requested.is_some() {
                self.driver.clear_camera_input();
            }
            if self.driver.startup.is_some() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if self.driver.startup.is_some() {
            bail!(
                "offscreen flat client did not reach playable startup after {MAX_OFFSCREEN_PLAYABLE_STARTUP_STEPS} steps"
            );
        }
        if self.driver.runtime.is_none() {
            bail!("offscreen flat client startup did not create a runtime");
        }
        self.require_uploaded_sections()?;
        Ok(())
    }

    fn anchor_local_playable_startup_camera(&mut self, scene: &SceneOptions) {
        if scene.remote_addr.is_some() {
            return;
        }
        let spawn = initial_spawn_center_for_seed(scene.seed);
        let mut startup_scene = scene.clone();
        startup_scene.chunk_x = spawn.x;
        startup_scene.chunk_z = spawn.z;
        let spectator = SpectatorCamera::spawn_for_scene(&startup_scene);
        self.driver
            .set_spectator_camera(spectator, scene.movement_speed_multiplier);
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
                movement_mode: game_movement_mode(self.driver.camera.movement_mode()),
                fly_speed_multiplier: self.driver.camera.fly_speed_multiplier() as f32,
                movement_speed_multiplier: self.driver.camera.movement_speed_multiplier() as f32,
            },
            block_palette: debug_block_palette_overlay(
                &self.assets.mesh_assets.catalog,
                self.driver.interaction.selected_hotbar_slot(),
            ),
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

    pub(crate) fn apply_input_frame(
        &mut self,
        frame: FlatInputFrame,
    ) -> Result<Vec<(FlatInputAction, FlatClientWorldActionStatus)>> {
        if frame.open_menu {
            self.driver.open_pause_menu();
            self.driver.clear_camera_input();
            return Ok(Vec::new());
        }
        if self.driver.ui_is_active() {
            return Ok(Vec::new());
        }

        if let Some(slot) = frame.selected_hotbar_slot {
            self.driver.select_hotbar_slot(slot);
        }
        if frame.hotbar_step != 0 {
            self.driver.step_hotbar_slot(frame.hotbar_step);
        }

        let dt_seconds = self.clock.frame_ms / 1000.0;
        if self.driver.apply_held_input_frame(frame, dt_seconds) {
            self.driver.commit_player_pose_change()?;
        }

        let mut statuses = Vec::new();
        if frame.attack {
            statuses.push((
                FlatInputAction::Attack,
                self.driver.handle_world_action(FlatInputAction::Attack)?,
            ));
        }
        if frame.use_item {
            statuses.push((
                FlatInputAction::Use,
                self.driver.handle_world_action(FlatInputAction::Use)?,
            ));
        }
        Ok(statuses)
    }

    pub(crate) fn run_script(&mut self, script: &OffscreenScript) -> Result<OffscreenScriptReport> {
        let mut report = OffscreenScriptReport::default();
        for step in &script.steps {
            match *step {
                OffscreenScriptStep::SetCameraLookAt { eye, target } => {
                    self.set_camera_look_at(eye, target);
                }
                OffscreenScriptStep::SetCameraPose {
                    position,
                    yaw,
                    pitch,
                } => {
                    self.set_camera_pose(position, yaw, pitch);
                }
                OffscreenScriptStep::InputFrame {
                    frame,
                    require_changed_action,
                } => {
                    report.input_frame_count += 1;
                    let statuses = self.apply_input_frame(frame)?;
                    report.world_action_count += statuses.len();
                    if let Some(action) = require_changed_action {
                        require_world_action_changed(
                            &statuses,
                            action,
                            "offscreen script input frame",
                        )?;
                    }
                }
            }
        }
        Ok(report)
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
        let target = self.scripted_interaction_target()?;
        let report = self.run_script(&scripted_interaction_script(target))?;
        if report.input_frame_count != 3 || report.world_action_count != 2 {
            bail!(
                "scripted interaction expected 3 input frames and 2 world actions, got {} input frames and {} world actions",
                report.input_frame_count,
                report.world_action_count
            );
        }
        Ok(())
    }

    fn scripted_interaction_target(&self) -> Result<ScriptedInteractionTarget> {
        let runtime = self
            .driver
            .runtime
            .as_ref()
            .context("offscreen scripted interaction requires an active runtime")?;
        let (base_x, base_z) = self.driver.spectator.block_column();
        let target_x = base_x;
        let target_z = base_z + 4;
        let surface_y = runtime
            .highest_non_air_block_y_at_world(target_x, target_z)
            .with_context(|| {
                format!("no loaded surface for scripted interaction at ({target_x}, {target_z})")
            })?;
        Ok(ScriptedInteractionTarget {
            x: target_x,
            y: surface_y,
            z: target_z,
        })
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
        self.set_camera_position(Vec3::from_array(eye));
    }

    fn set_camera_position(&mut self, position: Vec3) {
        let mut spectator = self.driver.spectator.clone();
        spectator.position = position;
        self.driver
            .set_spectator_camera(spectator, self.driver.scene.movement_speed_multiplier);
    }

    fn set_camera_pose(&mut self, position: Vec3, yaw: f32, pitch: f32) {
        let mut spectator = self.driver.spectator.clone();
        spectator.position = position;
        spectator.yaw = yaw;
        spectator.pitch = pitch;
        self.driver
            .set_spectator_camera(spectator, self.driver.scene.movement_speed_multiplier);
    }

    fn set_camera_look_at(&mut self, eye: Vec3, target: Vec3) {
        let mut spectator = self.driver.spectator.clone();
        aim_spectator_at(&mut spectator, eye, target);
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
    let startup_wait = options.startup_wait;
    let (loop_report, frame_pixels, host) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count: startup_wait.offscreen_capture_frame_count(),
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
            host.start_scene_with_wait_policy(device, scene.clone(), startup_wait)?;
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
    if !options.scripted_interaction {
        host.frame_first_actor();
    }
    if let Some(eye) = options.eye {
        host.set_eye_override(eye);
    }
    host.driver.camera.set_view_mode(options.camera_view);
    Ok(())
}

impl OffscreenFlatClientHost {
    fn underwater(&self) -> bool {
        self.driver
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.camera_inside_water(self.driver.camera_view().eye))
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScriptedInteractionTarget {
    x: i32,
    y: i32,
    z: i32,
}

fn action_input_frame(action: FlatInputAction) -> FlatInputFrame {
    let mut frame = FlatInputFrame::default();
    frame.apply_intent(FlatInputIntent::Action {
        action,
        pressed: true,
    });
    frame
}

fn hotbar_input_frame(slot: u8) -> FlatInputFrame {
    let mut frame = FlatInputFrame::default();
    frame.apply_intent(FlatInputIntent::SelectHotbarSlot(slot));
    frame
}

fn scripted_interaction_script(target: ScriptedInteractionTarget) -> OffscreenScript {
    let interaction_eye = Vec3::new(
        target.x as f32 + 0.5,
        target.y as f32 + 3.0,
        target.z as f32 - 1.5,
    );
    let interaction_target = Vec3::new(
        target.x as f32 + 0.5,
        target.y as f32 + 0.5,
        target.z as f32 + 0.5,
    );
    let final_position = Vec3::new(
        target.x as f32 + 0.5,
        target.y as f32 + 5.0,
        target.z as f32 - 6.0,
    );
    OffscreenScript::from_steps([
        OffscreenScriptStep::SetCameraLookAt {
            eye: interaction_eye,
            target: interaction_target,
        },
        OffscreenScriptStep::InputFrame {
            frame: hotbar_input_frame(7),
            require_changed_action: None,
        },
        OffscreenScriptStep::InputFrame {
            frame: action_input_frame(FlatInputAction::Attack),
            require_changed_action: Some(FlatInputAction::Attack),
        },
        OffscreenScriptStep::InputFrame {
            frame: action_input_frame(FlatInputAction::Use),
            require_changed_action: Some(FlatInputAction::Use),
        },
        OffscreenScriptStep::SetCameraPose {
            position: final_position,
            yaw: 0.0,
            pitch: -0.7,
        },
    ])
}

fn require_world_action_changed(
    statuses: &[(FlatInputAction, FlatClientWorldActionStatus)],
    action: FlatInputAction,
    label: &str,
) -> Result<()> {
    let Some((_, status)) = statuses
        .iter()
        .find(|(status_action, _)| *status_action == action)
    else {
        bail!("{label} did not run");
    };
    match status {
        FlatClientWorldActionStatus::Sent { changed: true, .. } => Ok(()),
        FlatClientWorldActionStatus::Sent { changed: false, .. } => {
            bail!("{label} sent a command but reported no world change")
        }
        FlatClientWorldActionStatus::NoRuntime => bail!("{label} had no active runtime"),
        FlatClientWorldActionStatus::NoTarget => bail!("{label} found no interaction target"),
        FlatClientWorldActionStatus::NoCommand => bail!("{label} produced no gameplay command"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_interaction_script_has_camera_input_and_final_pose_steps() {
        let script = scripted_interaction_script(ScriptedInteractionTarget { x: 1, y: 64, z: 2 });

        assert_eq!(script.steps().len(), 5);
        assert!(matches!(
            script.steps()[0],
            OffscreenScriptStep::SetCameraLookAt { .. }
        ));
        assert!(matches!(
            script.steps()[1],
            OffscreenScriptStep::InputFrame {
                frame: FlatInputFrame {
                    selected_hotbar_slot: Some(7),
                    ..
                },
                require_changed_action: None,
            }
        ));
        assert!(matches!(
            script.steps()[2],
            OffscreenScriptStep::InputFrame {
                require_changed_action: Some(FlatInputAction::Attack),
                ..
            }
        ));
        assert!(matches!(
            script.steps()[3],
            OffscreenScriptStep::InputFrame {
                require_changed_action: Some(FlatInputAction::Use),
                ..
            }
        ));
        assert!(matches!(
            script.steps()[4],
            OffscreenScriptStep::SetCameraPose { .. }
        ));
    }
}
