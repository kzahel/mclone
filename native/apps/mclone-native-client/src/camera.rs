use glam::Vec3;
use mclone_core::{CHUNK_WIDTH, ChunkPos};
use mclone_render::chunk::ChunkCamera;

use crate::cli::SceneOptions;

pub(crate) const SPECTATOR_BASE_SPEED: f32 = 32.0;
pub(crate) const SPECTATOR_MIN_SPEED: f32 = 2.0;
pub(crate) const SPECTATOR_MAX_SPEED: f32 = 256.0;
pub(crate) const SPECTATOR_MOUSE_SENSITIVITY: f32 = 0.0035;
pub(crate) const SPECTATOR_PITCH_LIMIT: f32 = 1.52;

#[derive(Clone, Debug)]
pub(crate) struct SpectatorCamera {
    pub(crate) position: Vec3,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) speed: f32,
}

impl SpectatorCamera {
    pub(crate) fn spawn_for_scene(scene: &SceneOptions) -> Self {
        let center_x = scene.chunk_x as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
        let center_z = scene.chunk_z as f32 * CHUNK_WIDTH as f32 + CHUNK_WIDTH as f32 * 0.5;
        Self {
            position: Vec3::new(center_x, 88.0, center_z),
            yaw: 0.55,
            pitch: -0.35,
            speed: SPECTATOR_BASE_SPEED,
        }
    }

    pub(crate) fn camera(&self, chunk_radius: u32) -> ChunkCamera {
        let forward = self.forward();
        ChunkCamera {
            eye: self.position.to_array(),
            target: (self.position + forward).to_array(),
            up: [0.0, 1.0, 0.0],
            fov_y_radians: 64.0_f32.to_radians(),
            z_near: 0.05,
            z_far: 700.0 + chunk_radius as f32 * 128.0,
        }
    }

    pub(crate) fn chunk_pos(&self) -> ChunkPos {
        ChunkPos::new(
            world_block_to_chunk_coord(self.position.x),
            world_block_to_chunk_coord(self.position.z),
        )
    }

    pub(crate) fn look(&mut self, yaw_delta: f32, pitch_delta: f32) {
        if yaw_delta.is_finite() {
            self.yaw += yaw_delta;
        }
        if pitch_delta.is_finite() {
            self.pitch =
                (self.pitch + pitch_delta).clamp(-SPECTATOR_PITCH_LIMIT, SPECTATOR_PITCH_LIMIT);
        }
    }

    pub(crate) fn adjust_speed(&mut self, wheel_amount: f32) {
        if !wheel_amount.is_finite() {
            return;
        }
        let multiplier = (1.0 + wheel_amount * 0.18).clamp(0.5, 1.8);
        self.speed = (self.speed * multiplier).clamp(SPECTATOR_MIN_SPEED, SPECTATOR_MAX_SPEED);
    }

    pub(crate) fn move_local(
        &mut self,
        right_axis: f32,
        up_axis: f32,
        forward_axis: f32,
        boosted: bool,
        dt: f32,
    ) -> bool {
        if dt <= 0.0 {
            return false;
        }

        let forward = self.forward();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let direction = right * right_axis + Vec3::Y * up_axis + forward * forward_axis;
        let Some(direction) = direction.try_normalize() else {
            return false;
        };
        let boost = if boosted { 3.0 } else { 1.0 };
        self.position += direction * self.speed * boost * dt;
        true
    }

    pub(crate) fn forward(&self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        Vec3::new(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos).normalize()
    }
}

pub(crate) fn world_block_to_chunk_coord(value: f32) -> i32 {
    (value / CHUNK_WIDTH as f32).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_block_to_chunk_coord_floors_negative_positions() {
        assert_eq!(world_block_to_chunk_coord(0.0), 0);
        assert_eq!(world_block_to_chunk_coord(15.99), 0);
        assert_eq!(world_block_to_chunk_coord(16.0), 1);
        assert_eq!(world_block_to_chunk_coord(-0.01), -1);
        assert_eq!(world_block_to_chunk_coord(-16.0), -1);
        assert_eq!(world_block_to_chunk_coord(-16.01), -2);
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
    fn spectator_pitch_is_clamped() {
        let mut spectator = SpectatorCamera::spawn_for_scene(&SceneOptions::default());
        spectator.look(0.0, 100.0);
        assert_eq!(spectator.pitch, SPECTATOR_PITCH_LIMIT);
        spectator.look(0.0, -200.0);
        assert_eq!(spectator.pitch, -SPECTATOR_PITCH_LIMIT);
    }
}
