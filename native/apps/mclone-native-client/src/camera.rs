use glam::Vec3;
use mclone_core::{ChunkPos, block_to_chunk_coord, chunk_middle_block_coord};
use mclone_render::chunk::ChunkCamera;

use crate::cli::SceneOptions;

pub(crate) const SPECTATOR_BASE_SPEED: f32 = 32.0;
pub(crate) const SPECTATOR_MIN_SPEED: f32 = 2.0;
pub(crate) const SPECTATOR_MAX_SPEED: f32 = 256.0;
pub(crate) const SPECTATOR_MOUSE_SENSITIVITY: f32 = 0.0035;
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
            yaw: 0.55,
            pitch: -0.35,
            speed: SPECTATOR_BASE_SPEED,
        }
    }

    pub(crate) fn camera(&self, render_distance: u32) -> ChunkCamera {
        let forward = self.forward();
        ChunkCamera {
            eye: self.position.to_array(),
            target: (self.position + forward).to_array(),
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 64.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0 + render_distance as f32 * 128.0,
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

    pub(crate) fn forward(&self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        Vec3::new(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos).normalize()
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
