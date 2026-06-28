use mclone_app_runtime::frame_render::RenderStreamStats;
use mclone_client::{ActorInterpolationState, ClientInteractionController};
use mclone_core::Vec3d;
use mclone_input::FlatInputFrame;
use mclone_render::chunk::{ChunkCamera, TexturedSectionRenderOptions};
use mclone_render_session::{
    EngineCameraController, EngineCameraFrameState, EngineCameraInput, EngineCameraMovementImpulse,
    render_camera_from_snapshot,
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
