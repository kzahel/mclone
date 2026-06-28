use mclone_app_runtime::frame_render::RenderStreamStats;
use mclone_client::{
    ActorInterpolationConfig, ActorInterpolationState, BlockInteractionTarget,
    ClientInteractionController,
};
use mclone_core::Vec3d;
use mclone_input::{FlatInputAction, FlatInputFrame};
use mclone_render::chunk::{ChunkCamera, TexturedSectionRenderOptions};
use mclone_render::entity::ActorInstance;
use mclone_render::screen_effect::UnderwaterOverlay;
use mclone_render::selection_outline::SelectionOutline;
use mclone_render_session::{
    EngineCameraController, EngineCameraFrameState, EngineCameraInput, EngineCameraMovementImpulse,
    actor_instances_from_presentations, render_camera_from_snapshot,
};

use crate::camera::{SpectatorCamera, chunk_camera_from_engine};
use crate::cli::SceneOptions;
use crate::frame_pacing::FrameTimingStats;
use crate::scene_runtime::WindowSceneRuntime;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlatClientCameraView {
    pub(crate) snapshot: mclone_render_session::EngineCameraSnapshot,
    pub(crate) eye: glam::Vec3,
}

impl FlatClientCameraView {
    pub(crate) fn from_snapshot(snapshot: mclone_render_session::EngineCameraSnapshot) -> Self {
        Self {
            snapshot,
            eye: glam_vec3_from_vec3d(snapshot.eye),
        }
    }

    pub(crate) fn block_column(self) -> (i32, i32) {
        (
            self.snapshot.eye.x.floor() as i32,
            self.snapshot.eye.z.floor() as i32,
        )
    }

    pub(crate) fn chunk_camera(self, render_distance: u32) -> ChunkCamera {
        chunk_camera_from_engine(render_camera_from_snapshot(self.snapshot, render_distance))
    }
}

pub(crate) struct FlatClientDriver {
    pub(crate) runtime: Option<WindowSceneRuntime>,
    pub(crate) spectator: SpectatorCamera,
    pub(crate) camera: EngineCameraController,
    pub(crate) actor_interpolation: ActorInterpolationState,
    pub(crate) interaction: ClientInteractionController,
    pub(crate) render_options: TexturedSectionRenderOptions,
    pub(crate) render_stats: RenderStreamStats,
    pub(crate) frame_timing: FrameTimingStats,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FlatClientWorldActionStatus {
    NoRuntime,
    NoTarget,
    NoCommand,
    Sent {
        target: BlockInteractionTarget,
        changed: bool,
    },
}

impl FlatClientDriver {
    pub(crate) fn new(scene: &SceneOptions, render_options: TexturedSectionRenderOptions) -> Self {
        let spectator = SpectatorCamera::spawn_for_scene(scene);
        let camera =
            engine_camera_controller_from_spectator(&spectator, scene.movement_speed_multiplier);
        Self {
            runtime: None,
            spectator,
            camera,
            actor_interpolation: ActorInterpolationState::new(),
            interaction: ClientInteractionController::new(),
            render_options,
            render_stats: RenderStreamStats::default(),
            frame_timing: FrameTimingStats::default(),
        }
    }

    pub(crate) fn camera_view(&self) -> FlatClientCameraView {
        FlatClientCameraView::from_snapshot(self.camera.snapshot())
    }

    pub(crate) fn camera_frame_state(&self) -> EngineCameraFrameState {
        self.camera.frame_state(&self.interaction)
    }

    pub(crate) fn current_render_distance(&self, fallback_render_distance: i32) -> u32 {
        self.runtime.as_ref().map_or_else(
            || u32::try_from(fallback_render_distance).unwrap_or(0),
            WindowSceneRuntime::render_distance,
        )
    }

    pub(crate) fn tick_frame_timing(&mut self, frame_ms: f64, target_frame_ms: Option<f64>) {
        self.render_stats.last_frame_ms = frame_ms as f32;
        self.frame_timing.begin_frame(frame_ms, target_frame_ms);
    }

    pub(crate) fn apply_held_input_frame(
        &mut self,
        frame: FlatInputFrame,
        dt_seconds: f64,
    ) -> bool {
        let Some(runtime) = self.runtime.as_ref() else {
            return false;
        };
        let input = engine_camera_input_from_flat_frame(frame, dt_seconds);
        let before = self.camera.snapshot();
        let after = self.camera.apply_movement_input(runtime.client(), input);
        after != before
    }

    pub(crate) fn clear_camera_input(&mut self) {
        self.camera.clear_keys();
    }

    pub(crate) fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        self.interaction.select_hotbar_slot(slot)
    }

    pub(crate) fn apply_look_frame(&mut self, frame: FlatInputFrame) -> bool {
        if frame.look_delta.x == 0.0 && frame.look_delta.y == 0.0 {
            return false;
        }
        self.camera
            .turn_mouse_delta(f64::from(frame.look_delta.x), f64::from(frame.look_delta.y));
        self.sync_spectator_from_camera();
        true
    }

    pub(crate) fn adjust_camera_speed(&mut self, amount: f64) {
        self.camera.adjust_speed(amount);
        self.sync_spectator_from_camera();
    }

    pub(crate) fn commit_player_pose_change(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.commit_engine_camera_player_pose(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn sync_server_player_pose(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.sync_engine_camera_player_pose(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn apply_pending_player_position_updates(&mut self) -> anyhow::Result<bool> {
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        let changed = runtime.apply_pending_engine_camera_position_updates(&mut self.camera)?;
        self.sync_spectator_from_camera();
        Ok(changed)
    }

    pub(crate) fn sync_carried_item(&mut self) -> anyhow::Result<bool> {
        let Some(command) = self.interaction.ensure_has_sent_carried_item() else {
            return Ok(false);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(false);
        };
        runtime.send_gameplay_command(command).map_err(Into::into)
    }

    pub(crate) fn current_block_interaction_target(&self) -> Option<BlockInteractionTarget> {
        let runtime = self.runtime.as_ref()?;
        self.camera
            .target_block(runtime.client(), &self.interaction)
    }

    pub(crate) fn current_selection_outline(&self, ui_active: bool) -> Option<SelectionOutline> {
        if ui_active {
            return None;
        }
        self.current_block_interaction_target()
            .map(|target| SelectionOutline::new(target.outline_boxes))
    }

    pub(crate) fn handle_world_action(
        &mut self,
        action: FlatInputAction,
    ) -> anyhow::Result<FlatClientWorldActionStatus> {
        if self.runtime.is_none() {
            return Ok(FlatClientWorldActionStatus::NoRuntime);
        }
        self.sync_server_player_pose()?;
        self.sync_carried_item()?;
        let Some(target) = self.current_block_interaction_target() else {
            return Ok(FlatClientWorldActionStatus::NoTarget);
        };
        let command = match action {
            FlatInputAction::Attack => self.interaction.debug_instant_break_command(target.hit),
            FlatInputAction::Use => self.interaction.use_item_on_command(target.hit),
            _ => None,
        };
        let Some(command) = command else {
            return Ok(FlatClientWorldActionStatus::NoCommand);
        };
        let Some(runtime) = &mut self.runtime else {
            return Ok(FlatClientWorldActionStatus::NoRuntime);
        };
        let changed = runtime.send_gameplay_command(command)?;
        Ok(FlatClientWorldActionStatus::Sent { target, changed })
    }

    pub(crate) fn effective_render_options(&self) -> TexturedSectionRenderOptions {
        effective_render_options_for_camera(
            self.render_options,
            self.runtime.as_ref().is_some_and(|runtime| {
                runtime.camera_inside_occluding_block(self.camera_view().eye)
            }),
        )
    }

    pub(crate) fn underwater_overlay(
        &self,
        camera_view: FlatClientCameraView,
    ) -> Option<UnderwaterOverlay> {
        self.runtime
            .as_ref()?
            .camera_inside_water(camera_view.eye)
            .then(|| {
                UnderwaterOverlay::vanilla_from_native_camera(
                    camera_view.snapshot.yaw_radians as f32,
                    camera_view.snapshot.pitch_radians as f32,
                )
            })
    }

    pub(crate) fn interpolated_actor_instances(&mut self) -> Vec<ActorInstance> {
        let Some(runtime) = self.runtime.as_ref() else {
            return Vec::new();
        };
        self.actor_interpolation
            .reconcile_authoritative(runtime.client().actor_presentations());
        self.actor_interpolation.step(
            (self.render_stats.last_frame_ms * 0.001).min(0.1),
            ActorInterpolationConfig::default(),
        );
        actor_instances_from_presentations(
            &self.actor_interpolation.presentations(),
            runtime.client(),
        )
    }

    pub(crate) fn sync_spectator_from_camera(&mut self) {
        sync_spectator_from_camera(&mut self.spectator, &self.camera);
    }

    pub(crate) fn reset_world_state(&mut self, scene: &SceneOptions) {
        self.runtime = None;
        self.spectator = SpectatorCamera::spawn_for_scene(scene);
        self.camera = engine_camera_controller_from_spectator(
            &self.spectator,
            scene.movement_speed_multiplier,
        );
        self.actor_interpolation = ActorInterpolationState::new();
        self.interaction = ClientInteractionController::new();
        self.render_stats = RenderStreamStats::default();
        self.frame_timing = FrameTimingStats::default();
    }
}

pub(crate) fn effective_render_options_for_camera(
    mut render_options: TexturedSectionRenderOptions,
    camera_inside_occluding_block: bool,
) -> TexturedSectionRenderOptions {
    if camera_inside_occluding_block {
        render_options.section_occlusion_culling = false;
    }
    render_options
}

pub(crate) fn vec3d_from_glam(value: glam::Vec3) -> Vec3d {
    Vec3d::new(value.x as f64, value.y as f64, value.z as f64)
}

pub(crate) fn glam_vec3_from_vec3d(value: Vec3d) -> glam::Vec3 {
    glam::Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

pub(crate) fn engine_camera_controller_from_spectator(
    spectator: &SpectatorCamera,
    movement_speed_multiplier: f32,
) -> EngineCameraController {
    let mut camera = EngineCameraController::from_eye_pose(
        vec3d_from_glam(spectator.position),
        f64::from(spectator.yaw),
        f64::from(spectator.pitch),
        f64::from(spectator.speed),
    );
    camera.set_movement_speed_multiplier(f64::from(movement_speed_multiplier));
    camera
}

pub(crate) fn sync_spectator_from_camera(
    spectator: &mut SpectatorCamera,
    camera: &EngineCameraController,
) {
    let snapshot = camera.snapshot();
    spectator.position = glam_vec3_from_vec3d(snapshot.eye);
    spectator.yaw = snapshot.yaw_radians as f32;
    spectator.pitch = snapshot.pitch_radians as f32;
    spectator.speed = snapshot.speed_blocks_per_second as f32;
}

pub(crate) fn engine_camera_input_from_flat_frame(
    frame: FlatInputFrame,
    dt_seconds: f64,
) -> EngineCameraInput {
    EngineCameraInput {
        dt_seconds,
        mouse_delta_x: f64::from(frame.look_delta.x),
        mouse_delta_y: f64::from(frame.look_delta.y),
        forward: frame.forward,
        backward: frame.backward,
        left: frame.left,
        right: frame.right,
        jump: frame.jump,
        descend: frame.descend,
        shift: frame.sneak,
        sprint: frame.sprint,
        movement_impulse: frame
            .analog_movement
            .map(|movement| EngineCameraMovementImpulse::new(movement.left, movement.forward)),
        movement_yaw_radians: None,
    }
}
