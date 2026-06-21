use glam::Vec3;
use mclone_core::{ChunkPos, Vec3d, block_to_chunk_coord, chunk_middle_block_coord};
use mclone_render::chunk::ChunkCamera;
use mclone_render_session::{
    ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND, ENGINE_CAMERA_MAX_SPEED_BLOCKS_PER_SECOND,
    ENGINE_CAMERA_MIN_SPEED_BLOCKS_PER_SECOND, ENGINE_CAMERA_MOUSE_SENSITIVITY,
    ENGINE_CAMERA_SPAWN_PITCH_RADIANS, ENGINE_CAMERA_SPAWN_YAW_RADIANS, EngineCameraSnapshot,
    EngineRenderCamera, render_camera_from_snapshot,
};

use crate::cli::SceneOptions;

pub(crate) const SPECTATOR_BASE_SPEED: f32 = ENGINE_CAMERA_BASE_SPEED_BLOCKS_PER_SECOND as f32;
pub(crate) const SPECTATOR_MIN_SPEED: f32 = ENGINE_CAMERA_MIN_SPEED_BLOCKS_PER_SECOND as f32;
pub(crate) const SPECTATOR_MAX_SPEED: f32 = ENGINE_CAMERA_MAX_SPEED_BLOCKS_PER_SECOND as f32;
pub(crate) const SPECTATOR_MOUSE_SENSITIVITY: f32 = ENGINE_CAMERA_MOUSE_SENSITIVITY as f32;
pub(crate) const SPECTATOR_SURFACE_CLEARANCE: f32 = 8.0;
pub(crate) const SPECTATOR_SURFACE_PITCH: f32 = -0.45;

#[derive(Clone, Debug)]
pub(crate) struct SpectatorCamera {
    pub(crate) position: Vec3,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) speed: f32,
}

impl SpectatorCamera {
    pub(crate) fn spawn_for_scene(scene: &SceneOptions) -> Self {
        let center_x = chunk_middle_block_coord(scene.chunk_x) as f32;
        let center_z = chunk_middle_block_coord(scene.chunk_z) as f32;
        Self {
            position: Vec3::new(center_x, 88.0, center_z),
            yaw: ENGINE_CAMERA_SPAWN_YAW_RADIANS as f32,
            pitch: ENGINE_CAMERA_SPAWN_PITCH_RADIANS as f32,
            speed: SPECTATOR_BASE_SPEED,
        }
    }

    pub(crate) fn camera(&self, render_distance: u32) -> ChunkCamera {
        chunk_camera_from_engine(render_camera_from_snapshot(
            self.engine_snapshot(),
            render_distance,
        ))
    }

    pub(crate) fn engine_snapshot(&self) -> EngineCameraSnapshot {
        EngineCameraSnapshot {
            eye: Vec3d::new(
                f64::from(self.position.x),
                f64::from(self.position.y),
                f64::from(self.position.z),
            ),
            yaw_radians: f64::from(self.yaw),
            pitch_radians: f64::from(self.pitch),
            speed_blocks_per_second: f64::from(self.speed),
            chunk_pos: self.chunk_pos(),
        }
    }

    pub(crate) fn chunk_pos(&self) -> ChunkPos {
        ChunkPos::new(
            world_coord_to_chunk_coord(self.position.x),
            world_coord_to_chunk_coord(self.position.z),
        )
    }

    pub(crate) fn block_column(&self) -> (i32, i32) {
        (
            self.position.x.floor() as i32,
            self.position.z.floor() as i32,
        )
    }

    pub(crate) fn place_above_surface(&mut self, surface_y: i32) {
        self.position.y = surface_y as f32 + SPECTATOR_SURFACE_CLEARANCE;
        self.pitch = SPECTATOR_SURFACE_PITCH;
    }

    pub(crate) fn adjust_speed(&mut self, wheel_amount: f32) {
        if !wheel_amount.is_finite() {
            return;
        }
        let multiplier = (1.0 + wheel_amount * 0.18).clamp(0.5, 1.8);
        self.speed = (self.speed * multiplier).clamp(SPECTATOR_MIN_SPEED, SPECTATOR_MAX_SPEED);
    }

    #[cfg(test)]
    pub(crate) fn forward(&self) -> Vec3 {
        let camera = self.camera(0);
        (Vec3::from_array(camera.target) - Vec3::from_array(camera.eye)).normalize()
    }
}

fn chunk_camera_from_engine(camera: EngineRenderCamera) -> ChunkCamera {
    ChunkCamera {
        eye: camera.eye,
        target: camera.target,
        up: camera.up,
        fov_y_radians: camera.fov_y_radians,
        z_near: camera.z_near,
        z_far: camera.z_far,
    }
}

fn world_coord_to_chunk_coord(value: f32) -> i32 {
    block_to_chunk_coord(value.floor() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_coord_to_chunk_coord_floors_negative_positions() {
        assert_eq!(world_coord_to_chunk_coord(0.0), 0);
        assert_eq!(world_coord_to_chunk_coord(15.99), 0);
        assert_eq!(world_coord_to_chunk_coord(16.0), 1);
        assert_eq!(world_coord_to_chunk_coord(-0.01), -1);
        assert_eq!(world_coord_to_chunk_coord(-16.0), -1);
        assert_eq!(world_coord_to_chunk_coord(-16.01), -2);
    }

    #[test]
    fn spectator_spawn_starts_interest_in_scene_center_chunk() {
        let scene = SceneOptions {
            chunk_x: 3,
            chunk_z: -2,
            ..SceneOptions::default()
        };
        let spectator = SpectatorCamera::spawn_for_scene(&scene);

        assert_eq!(spectator.chunk_pos(), ChunkPos::new(3, -2));
    }

    #[test]
    fn spectator_crossing_chunk_boundary_changes_interest_center() {
        let scene = SceneOptions::default();
        let mut spectator = SpectatorCamera::spawn_for_scene(&scene);
        spectator.position.x = 16.25;
        spectator.position.z = -0.25;

        assert_eq!(spectator.chunk_pos(), ChunkPos::new(1, -1));
    }

    #[test]
    fn spectator_can_be_placed_above_loaded_surface() {
        let mut spectator = SpectatorCamera::spawn_for_scene(&SceneOptions::default());

        spectator.place_above_surface(93);

        assert_eq!(spectator.position.y, 101.0);
        assert_eq!(spectator.pitch, SPECTATOR_SURFACE_PITCH);
    }
}
