use mclone_core::Vec3d;
use mclone_protocol::MovePlayerCommand;

const JAVA_HORIZONTAL_MOVE_BOUND: f64 = 3.0e7;
const JAVA_VERTICAL_MOVE_BOUND: f64 = 2.0e7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ServerPlayerState {
    position: Vec3d,
    y_rot_degrees: f32,
    x_rot_degrees: f32,
    on_ground: bool,
}

impl Default for ServerPlayerState {
    fn default() -> Self {
        Self {
            position: Vec3d::ZERO,
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: false,
        }
    }
}

impl ServerPlayerState {
    pub(crate) fn apply_move_player(&mut self, command: MovePlayerCommand) -> bool {
        if !command.position.is_finite()
            || !command.y_rot_degrees.is_finite()
            || !command.x_rot_degrees.is_finite()
        {
            return false;
        }

        self.position = Vec3d::new(
            clamp_horizontal(command.position.x),
            clamp_vertical(command.position.y),
            clamp_horizontal(command.position.z),
        );
        self.y_rot_degrees = wrap_degrees(command.y_rot_degrees);
        self.x_rot_degrees = wrap_degrees(command.x_rot_degrees);
        self.on_ground = command.on_ground;
        true
    }

    pub(crate) const fn position(self) -> Vec3d {
        self.position
    }

    #[cfg(test)]
    pub(crate) const fn y_rot_degrees(self) -> f32 {
        self.y_rot_degrees
    }

    #[cfg(test)]
    pub(crate) const fn x_rot_degrees(self) -> f32 {
        self.x_rot_degrees
    }

    #[cfg(test)]
    pub(crate) const fn on_ground(self) -> bool {
        self.on_ground
    }
}

fn clamp_horizontal(value: f64) -> f64 {
    value.clamp(-JAVA_HORIZONTAL_MOVE_BOUND, JAVA_HORIZONTAL_MOVE_BOUND)
}

fn clamp_vertical(value: f64) -> f64 {
    value.clamp(-JAVA_VERTICAL_MOVE_BOUND, JAVA_VERTICAL_MOVE_BOUND)
}

fn wrap_degrees(mut degrees: f32) -> f32 {
    degrees %= 360.0;
    if degrees >= 180.0 {
        degrees -= 360.0;
    }
    if degrees < -180.0 {
        degrees += 360.0;
    }
    degrees
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_player_applies_java_packet_clamp_and_rotation_shape() {
        let mut player = ServerPlayerState::default();

        assert!(player.apply_move_player(MovePlayerCommand {
            position: Vec3d::new(4.0e7, 3.0e7, -4.0e7),
            y_rot_degrees: 181.0,
            x_rot_degrees: -181.0,
            on_ground: true,
        }));

        assert_eq!(
            player.position(),
            Vec3d::new(
                JAVA_HORIZONTAL_MOVE_BOUND,
                JAVA_VERTICAL_MOVE_BOUND,
                -JAVA_HORIZONTAL_MOVE_BOUND,
            )
        );
        assert_eq!(player.y_rot_degrees(), -179.0);
        assert_eq!(player.x_rot_degrees(), 179.0);
        assert!(player.on_ground());
    }

    #[test]
    fn move_player_rejects_non_finite_values_without_mutating_state() {
        let mut player = ServerPlayerState::default();
        assert!(player.apply_move_player(MovePlayerCommand {
            position: Vec3d::new(1.0, 2.0, 3.0),
            y_rot_degrees: 45.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }));
        let previous = player;

        assert!(!player.apply_move_player(MovePlayerCommand {
            position: Vec3d::new(f64::NAN, 2.0, 3.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: false,
        }));
        assert!(!player.apply_move_player(MovePlayerCommand {
            position: Vec3d::new(1.0, 2.0, 3.0),
            y_rot_degrees: f32::INFINITY,
            x_rot_degrees: 0.0,
            on_ground: false,
        }));

        assert_eq!(player, previous);
    }
}
