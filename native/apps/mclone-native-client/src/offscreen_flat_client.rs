use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_app_runtime::frame_render::FullFrameRenderSummary;
use mclone_client::{ActorPresentationId, ClientHost, ClientRuntime};
use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId};
use mclone_input::{FlatInputAction, FlatInputFrame, FlatInputIntent};
use mclone_render::headless::{HeadlessFrameLoopOptions, run_headless_capture_loop, save_rgba_png};
use mclone_scene::{MonoUiPresentation, MonoWorldActionStatus};
use mclone_server::initial_spawn_center_for_seed;
use mclone_ui::{GameTravelAssistMode, Point};

use crate::camera::SpectatorCamera;
use crate::cli::{
    HeadlessScreenshotOptions, HeadlessScreenshotUi, SceneOptions, StartupWaitPolicy,
};
use crate::offscreen_scene_host::OffscreenDriver;
use crate::render_cache::load_asset_source;
use crate::scene_runtime::WindowSceneAssets;

const OFFSCREEN_BLINK_DEBUG_PREVIEW_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OffscreenFlatClientScreenshotReport {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) summary: FullFrameRenderSummary,
    pub(crate) remote_player_count: usize,
    pub(crate) remote_actor_figures: Vec<mclone_assets::ActorFigureId>,
    pub(crate) remote_actor_walk_animation_distances: Vec<f32>,
    pub(crate) entity_count: usize,
    pub(crate) underwater: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OffscreenFlatClientFrameOptions {
    pub(crate) hud: bool,
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
    InputFrame {
        frame: FlatInputFrame,
        require_changed_action: Option<FlatInputAction>,
    },
    #[allow(dead_code)]
    UiPointerClick {
        point: Point,
        require_action: Option<mclone_ui::GameUiAction>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OffscreenScriptReport {
    pub(crate) input_frame_count: usize,
    pub(crate) world_action_count: usize,
    pub(crate) ui_pointer_click_count: usize,
    pub(crate) ui_action_count: usize,
}

pub(crate) struct OffscreenFlatClientHost {
    driver: OffscreenDriver,
    camera: SpectatorCamera,
}

impl OffscreenFlatClientHost {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        target_size: [u32; 2],
        scene: &SceneOptions,
        render_options: mclone_render::chunk::TexturedSectionRenderOptions,
        assets: &WindowSceneAssets,
        asset_source: &impl mclone_assets::AssetSource,
        startup_camera: SpectatorCamera,
    ) -> Result<Self> {
        let driver = OffscreenDriver::new(
            device,
            queue,
            format,
            target_size,
            scene,
            render_options,
            assets,
            asset_source,
            Some(&startup_camera),
        )?;
        Ok(Self {
            driver,
            camera: startup_camera,
        })
    }

    pub(crate) fn start_scene_with_wait_policy(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        startup_wait: StartupWaitPolicy,
    ) -> Result<()> {
        self.driver
            .drive_to_wait_policy(device, queue, startup_wait)?;
        if matches!(
            startup_wait,
            StartupWaitPolicy::Playable | StartupWaitPolicy::Idle
        ) && self.driver.host().render_stats().section_count == 0
        {
            bail!("offscreen flat client reached readiness without render sections");
        }
        Ok(())
    }

    pub(crate) fn render_frame(
        &mut self,
        frame: mclone_render::target::RenderFrameContext<'_>,
        options: OffscreenFlatClientFrameOptions,
    ) -> Result<FullFrameRenderSummary> {
        Ok(self
            .driver
            .render(frame, MonoUiPresentation::ScreenSpaceHud, options.hud)?
            .render)
    }

    pub(crate) fn run_script(
        &mut self,
        script: &OffscreenScript,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<OffscreenScriptReport> {
        let mut report = OffscreenScriptReport::default();
        for step in &script.steps {
            match *step {
                OffscreenScriptStep::SetCameraLookAt { eye, target } => {
                    self.set_camera_look_at(eye, target);
                    self.driver.commit_camera()?;
                }
                OffscreenScriptStep::InputFrame {
                    frame,
                    require_changed_action,
                } => {
                    report.input_frame_count += 1;
                    let statuses = self.driver.apply_input_frame(frame)?;
                    report.world_action_count += statuses.len();
                    if let Some(action) = require_changed_action {
                        require_world_action_changed(
                            &statuses,
                            action,
                            "offscreen script input frame",
                        )?;
                    }
                }
                OffscreenScriptStep::UiPointerClick {
                    point,
                    require_action,
                } => {
                    report.ui_pointer_click_count += 1;
                    let action = self.driver.apply_ui_pointer_click(point, device, queue)?;
                    report.ui_action_count += usize::from(action.is_some());
                    if let Some(required) = require_action
                        && action != Some(required)
                    {
                        bail!(
                            "offscreen script UI pointer click at ({:.1}, {:.1}) emitted {:?}, expected {:?}",
                            point.x,
                            point.y,
                            action,
                            required
                        );
                    }
                }
            }
        }
        Ok(report)
    }

    fn force_day_time(&mut self, day_time: u64) {
        self.driver.host_mut().force_mono_day_time(day_time);
    }

    fn settle_remote_session(&mut self, remote_settle_ms: u64) -> Result<()> {
        if remote_settle_ms == 0
            || !self
                .driver
                .host()
                .mono_client()
                .is_some_and(|client| client.host() == ClientHost::RemoteDedicated)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(remote_settle_ms));
        self.driver.commit_camera()?;
        Ok(())
    }

    fn apply_scripted_interaction(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let target = self.scripted_interaction_target()?;
        let report = self.run_script(&scripted_interaction_script(target), device, queue)?;
        if report.input_frame_count != 2 || report.world_action_count != 1 {
            bail!(
                "scripted interaction expected 2 input frames and 1 world action, got {} input frames and {} world actions",
                report.input_frame_count,
                report.world_action_count
            );
        }
        Ok(())
    }

    fn scripted_interaction_target(&self) -> Result<ScriptedInteractionTarget> {
        self.driver
            .host()
            .mono_client()
            .context("offscreen scripted interaction requires an active runtime")?;
        let (base_x, base_z) = self.camera.block_column();
        if let Some(target) = clear_scripted_interaction_target(self.driver.host(), base_x, base_z)
        {
            return Ok(target);
        }
        let fallback_x = base_x;
        let fallback_z = base_z + 4;
        let surface_y = self
            .driver
            .host()
            .mono_highest_non_air_block_y_at_world(fallback_x, fallback_z)
            .with_context(|| {
                format!(
                    "no loaded surface for scripted interaction at ({fallback_x}, {fallback_z})"
                )
            })?;
        Ok(ScriptedInteractionTarget {
            x: fallback_x,
            y: surface_y,
            z: fallback_z,
        })
    }

    fn frame_first_actor(&mut self) {
        let Some(client) = self.driver.host().mono_client() else {
            return;
        };
        frame_first_actor(client, &mut self.camera);
        self.driver.set_camera(&self.camera);
    }

    fn set_eye_override(&mut self, eye: [f32; 3]) {
        self.camera.position = Vec3::from_array(eye);
        self.driver.set_camera(&self.camera);
    }

    fn set_camera_look_at(&mut self, eye: Vec3, target: Vec3) {
        aim_spectator_at(&mut self.camera, eye, target);
        self.driver.set_camera(&self.camera);
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
    let startup_camera = screenshot_startup_camera(&scene, startup_wait);
    let frame_count = if options.frame_pipeline_overlay {
        startup_wait.offscreen_capture_frame_count().max(2)
    } else {
        startup_wait.offscreen_capture_frame_count()
    };
    let (loop_report, frame_pixels, host) = run_headless_capture_loop(
        HeadlessFrameLoopOptions {
            width: options.width,
            height: options.height,
            frame_count,
            pace_frame_duration: None,
        },
        move |device, queue, format, size| {
            let mut host = OffscreenFlatClientHost::new(
                device,
                queue,
                format,
                size,
                &scene,
                render_options,
                &assets,
                &asset_source,
                startup_camera,
            )?;
            host.start_scene_with_wait_policy(device, queue, startup_wait)?;
            configure_screenshot_scene(&mut host, options, device, queue)?;
            Ok(host)
        },
        |_index, frame, host| {
            host.render_frame(frame, OffscreenFlatClientFrameOptions { hud: options.hud })?;
            Ok(())
        },
    )?;

    let pixels = frame_pixels
        .last()
        .context("offscreen flat client screenshot produced no captured frame")?;
    save_rgba_png(&options.path, loop_report.width, loop_report.height, pixels)?;
    let summary = host
        .driver
        .last_summary()
        .context("offscreen flat client screenshot rendered no frame summary")?
        .render;
    let client = host.driver.host().mono_client();
    let remote_player_count = client.map_or(0, ClientRuntime::remote_player_count);
    let remote_actor_presentations =
        client.map_or_else(Vec::new, ClientRuntime::actor_presentations);
    let remote_actor_figures = remote_actor_presentations
        .iter()
        .filter_map(|actor| {
            matches!(actor.id, ActorPresentationId::RemotePlayer(_))
                .then_some(actor.appearance.figure)
                .flatten()
        })
        .collect();
    let remote_actor_walk_animation_distances = remote_actor_presentations
        .iter()
        .filter_map(|actor| {
            matches!(actor.id, ActorPresentationId::RemotePlayer(_))
                .then_some(actor.walk_animation_distance)
        })
        .collect();
    let entity_count = client.map_or(0, ClientRuntime::entity_count);
    let underwater = host.driver.host().mono_underwater();

    Ok(OffscreenFlatClientScreenshotReport {
        path: options.path.clone(),
        width: loop_report.width,
        height: loop_report.height,
        byte_len: pixels.len(),
        summary,
        remote_player_count,
        remote_actor_figures,
        remote_actor_walk_animation_distances,
        entity_count,
        underwater,
    })
}

fn screenshot_startup_camera(
    scene: &SceneOptions,
    startup_wait: StartupWaitPolicy,
) -> SpectatorCamera {
    if scene.remote_addr.is_some() || matches!(startup_wait, StartupWaitPolicy::Idle) {
        return SpectatorCamera::spawn_for_scene(scene);
    }
    let spawn = initial_spawn_center_for_seed(scene.seed);
    let mut startup_scene = scene.clone();
    startup_scene.chunk_x = spawn.x;
    startup_scene.chunk_z = spawn.z;
    SpectatorCamera::spawn_for_scene(&startup_scene)
}

fn configure_screenshot_scene(
    host: &mut OffscreenFlatClientHost,
    options: &HeadlessScreenshotOptions,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<()> {
    if matches!(
        options.ui,
        HeadlessScreenshotUi::NewWorld | HeadlessScreenshotUi::WorldCreate
    ) {
        host.driver
            .host_mut()
            .set_mono_new_world_seed(options.scene.seed);
    }
    host.driver
        .host_mut()
        .set_mono_ui_screen(options.ui.game_screen());
    if let Some(day_time) = options.scene.day_time_override {
        host.force_day_time(day_time);
    }
    host.settle_remote_session(options.remote_settle_ms)?;
    if options.scripted_interaction {
        host.apply_scripted_interaction(device, queue)?;
    } else {
        host.frame_first_actor();
    }
    if let Some(eye) = options.eye {
        host.set_eye_override(eye);
    }
    if let Some(target) = options.target {
        host.set_camera_look_at(host.camera.position, Vec3::from_array(target));
    }
    host.driver
        .host_mut()
        .set_mono_player_collision_box_visible(options.player_collision_box);
    host.driver
        .host_mut()
        .set_mono_frame_pipeline_overlay_visible(options.frame_pipeline_overlay);
    host.driver
        .host_mut()
        .set_mono_debug_diagnostics_visible(options.debug_pane);
    host.driver
        .host_mut()
        .set_mono_camera_view(options.camera_view);
    if options.blink_debug {
        host.driver
            .host_mut()
            .set_mono_travel_assist_mode(GameTravelAssistMode::Blink);
        if !host.driver.host_mut().begin_mono_blink_debug() {
            bail!("offscreen Blink debug preview requires an active runtime");
        }
        let start = std::time::Instant::now();
        while !host.driver.host().mono_blink_preview_ready() {
            host.driver.host_mut().update_mono_blink_debug();
            if start.elapsed() >= OFFSCREEN_BLINK_DEBUG_PREVIEW_TIMEOUT {
                bail!("offscreen Blink debug preview worker timed out");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    Ok(())
}

fn clear_scripted_interaction_target(
    host: &crate::desktop_scene_host::DesktopSceneHost,
    base_x: i32,
    base_z: i32,
) -> Option<ScriptedInteractionTarget> {
    let client = host.mono_client()?;
    const X_OFFSETS: [i32; 7] = [0, -1, 1, -2, 2, -3, 3];
    for dz in 3..=10 {
        for dx in X_OFFSETS {
            let x = base_x + dx;
            let z = base_z + dz;
            let Some(y) = host.mono_highest_non_air_block_y_at_world(x, z) else {
                continue;
            };
            if is_clear_torch_surface(client, x, y, z) {
                return Some(ScriptedInteractionTarget { x, y, z });
            }
        }
    }
    None
}

fn is_clear_torch_surface(client: &ClientRuntime, x: i32, y: i32, z: i32) -> bool {
    let pos = BlockPos::new(x, y, z);
    let above = pos.relative(mclone_core::Direction::Up);
    let headroom = above.relative(mclone_core::Direction::Up);
    let Some(surface) = client.block_state_at_block_pos(pos) else {
        return false;
    };
    is_screenshot_surface_support(surface)
        && client.block_state_at_block_pos(above) == Some(AIR_BLOCK_STATE_ID)
        && client.block_state_at_block_pos(headroom) == Some(AIR_BLOCK_STATE_ID)
}

fn is_screenshot_surface_support(state: BlockStateId) -> bool {
    matches!(
        state.0,
        1 | 4 | 5 | 6 | 7 | 13 | 14 | 15 | 33 | 34 | 38 | 40 | 52 | 53 | 88
    )
}

fn frame_first_actor(client: &ClientRuntime, spectator: &mut SpectatorCamera) {
    let Some(actor) = client.actor_presentations().first().copied() else {
        return;
    };
    let target = Vec3::new(
        actor.feet_position.x as f32,
        actor.feet_position.y as f32 + 1.0,
        actor.feet_position.z as f32,
    );
    let eye = target + Vec3::new(-2.2, 1.4, -4.8);
    aim_spectator_at(spectator, eye, target);
}

fn aim_spectator_at(spectator: &mut SpectatorCamera, eye: Vec3, target: Vec3) {
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
        target.y as f32 + 6.0,
        target.z as f32 - 2.0,
    );
    let final_target = Vec3::new(
        target.x as f32 + 0.5,
        target.y as f32 + 1.2,
        target.z as f32 + 0.5,
    );
    OffscreenScript::from_steps([
        OffscreenScriptStep::SetCameraLookAt {
            eye: interaction_eye,
            target: interaction_target,
        },
        OffscreenScriptStep::InputFrame {
            frame: hotbar_input_frame(8),
            require_changed_action: None,
        },
        OffscreenScriptStep::InputFrame {
            frame: action_input_frame(FlatInputAction::Use),
            require_changed_action: Some(FlatInputAction::Use),
        },
        OffscreenScriptStep::SetCameraLookAt {
            eye: final_position,
            target: final_target,
        },
    ])
}

fn require_world_action_changed(
    statuses: &[(FlatInputAction, MonoWorldActionStatus)],
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
        MonoWorldActionStatus::Sent { changed: true, .. } => Ok(()),
        MonoWorldActionStatus::Sent { changed: false, .. } => {
            bail!("{label} sent a command but reported no world change")
        }
        MonoWorldActionStatus::NoRuntime => bail!("{label} had no active runtime"),
        MonoWorldActionStatus::NoTarget => bail!("{label} found no interaction target"),
        MonoWorldActionStatus::NoCommand => bail!("{label} produced no gameplay command"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_interaction_script_has_camera_input_and_final_framing_steps() {
        let script = scripted_interaction_script(ScriptedInteractionTarget { x: 1, y: 64, z: 2 });

        assert_eq!(script.steps().len(), 4);
        assert!(matches!(
            script.steps()[0],
            OffscreenScriptStep::SetCameraLookAt { .. }
        ));
        assert!(matches!(
            script.steps()[1],
            OffscreenScriptStep::InputFrame {
                frame: FlatInputFrame {
                    selected_hotbar_slot: Some(8),
                    ..
                },
                require_changed_action: None,
            }
        ));
        assert!(matches!(
            script.steps()[2],
            OffscreenScriptStep::InputFrame {
                require_changed_action: Some(FlatInputAction::Use),
                ..
            }
        ));
        assert!(matches!(
            script.steps()[3],
            OffscreenScriptStep::SetCameraLookAt { .. }
        ));
    }

    #[test]
    fn offscreen_script_can_store_ui_pointer_click_steps() {
        let point = Point { x: 12.0, y: 34.0 };
        let script = OffscreenScript::from_steps([OffscreenScriptStep::UiPointerClick {
            point,
            require_action: Some(mclone_ui::GameUiAction::ToggleCrosshair),
        }]);

        assert_eq!(script.steps().len(), 1);
        assert!(matches!(
            script.steps()[0],
            OffscreenScriptStep::UiPointerClick {
                point: stored_point,
                require_action: Some(mclone_ui::GameUiAction::ToggleCrosshair),
            } if stored_point == point
        ));
    }
}
