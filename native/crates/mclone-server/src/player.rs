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
    first_good_position: Vec3d,
    last_good_position: Vec3d,
    received_move_packet_count: u32,
    known_move_packet_count: u32,
    awaiting_teleport: Option<AwaitingTeleport>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AwaitingTeleport {
    pub(crate) id: u32,
    pub(crate) tick: u64,
    pub(crate) position: Vec3d,
}

impl Default for ServerPlayerState {
    fn default() -> Self {
        Self {
            position: Vec3d::ZERO,
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: false,
            first_good_position: Vec3d::ZERO,
            last_good_position: Vec3d::ZERO,
            received_move_packet_count: 0,
            known_move_packet_count: 0,
            awaiting_teleport: None,
        }
    }
}

impl ServerPlayerState {
    pub(crate) fn apply_move_player(&mut self, command: MovePlayerCommand) -> bool {
        let position = command.position_or(self.position);
        let y_rot_degrees = command.y_rot_degrees_or(self.y_rot_degrees);
        let x_rot_degrees = command.x_rot_degrees_or(self.x_rot_degrees);
        if !position.is_finite() || !y_rot_degrees.is_finite() || !x_rot_degrees.is_finite() {
            return false;
        }

        self.position = Vec3d::new(
            clamp_horizontal(position.x),
            clamp_vertical(position.y),
            clamp_horizontal(position.z),
        );
        self.y_rot_degrees = wrap_degrees(y_rot_degrees);
        self.x_rot_degrees = wrap_degrees(x_rot_degrees);
        self.on_ground = command.on_ground();
        self.received_move_packet_count = self.received_move_packet_count.saturating_add(1);
        self.last_good_position = self.position;
        true
    }

    pub(crate) fn mark_tick_boundary(&mut self) {
        self.first_good_position = self.position;
        self.last_good_position = self.position;
        self.known_move_packet_count = self.received_move_packet_count;
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

    #[cfg(test)]
    pub(crate) const fn first_good_position(self) -> Vec3d {
        self.first_good_position
    }

    #[cfg(test)]
    pub(crate) const fn last_good_position(self) -> Vec3d {
        self.last_good_position
    }

    #[cfg(test)]
    pub(crate) const fn received_move_packet_count(self) -> u32 {
        self.received_move_packet_count
    }

    #[cfg(test)]
    pub(crate) const fn known_move_packet_count(self) -> u32 {
        self.known_move_packet_count
    }

    #[cfg(test)]
    pub(crate) const fn awaiting_teleport(self) -> Option<AwaitingTeleport> {
        self.awaiting_teleport
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

        assert!(player.apply_move_player(MovePlayerCommand::PosRot {
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
        assert_eq!(player.last_good_position(), player.position());
        assert_eq!(player.received_move_packet_count(), 1);
        assert_eq!(player.known_move_packet_count(), 0);
    }

    #[test]
    fn move_player_rejects_non_finite_values_without_mutating_state() {
        let mut player = ServerPlayerState::default();
        assert!(player.apply_move_player(MovePlayerCommand::PosRot {
            position: Vec3d::new(1.0, 2.0, 3.0),
            y_rot_degrees: 45.0,
            x_rot_degrees: 0.0,
            on_ground: true,
        }));
        let previous = player;

        assert!(!player.apply_move_player(MovePlayerCommand::PosRot {
            position: Vec3d::new(f64::NAN, 2.0, 3.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            on_ground: false,
        }));
        assert!(!player.apply_move_player(MovePlayerCommand::Rot {
            y_rot_degrees: f32::INFINITY,
            x_rot_degrees: 0.0,
            on_ground: false,
        }));

        assert_eq!(player, previous);
    }

    #[test]
    fn move_player_variants_preserve_missing_position_or_rotation() {
        let mut player = ServerPlayerState::default();

        assert!(player.apply_move_player(MovePlayerCommand::PosRot {
            position: Vec3d::new(1.0, 2.0, 3.0),
            y_rot_degrees: 30.0,
            x_rot_degrees: -15.0,
            on_ground: true,
        }));
        assert!(player.apply_move_player(MovePlayerCommand::Rot {
            y_rot_degrees: 45.0,
            x_rot_degrees: 10.0,
            on_ground: false,
        }));

        assert_eq!(player.position(), Vec3d::new(1.0, 2.0, 3.0));
        assert_eq!(player.y_rot_degrees(), 45.0);
        assert_eq!(player.x_rot_degrees(), 10.0);
        assert!(!player.on_ground());

        assert!(player.apply_move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(4.0, 5.0, 6.0),
            on_ground: true,
        }));
        assert_eq!(player.position(), Vec3d::new(4.0, 5.0, 6.0));
        assert_eq!(player.y_rot_degrees(), 45.0);
        assert_eq!(player.x_rot_degrees(), 10.0);
        assert!(player.on_ground());

        assert!(player.apply_move_player(MovePlayerCommand::StatusOnly { on_ground: false }));
        assert_eq!(player.position(), Vec3d::new(4.0, 5.0, 6.0));
        assert_eq!(player.y_rot_degrees(), 45.0);
        assert_eq!(player.x_rot_degrees(), 10.0);
        assert!(!player.on_ground());
        assert_eq!(player.received_move_packet_count(), 4);
    }

    #[test]
    fn movement_tick_boundary_tracks_known_packet_count_and_good_positions() {
        let mut player = ServerPlayerState::default();

        assert!(player.apply_move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(1.0, 64.0, 1.0),
            on_ground: true,
        }));
        assert!(player.apply_move_player(MovePlayerCommand::Pos {
            position: Vec3d::new(2.0, 65.0, 2.0),
            on_ground: false,
        }));
        assert_eq!(player.received_move_packet_count(), 2);
        assert_eq!(player.known_move_packet_count(), 0);
        assert_eq!(player.last_good_position(), Vec3d::new(2.0, 65.0, 2.0));

        player.mark_tick_boundary();

        assert_eq!(player.first_good_position(), Vec3d::new(2.0, 65.0, 2.0));
        assert_eq!(player.last_good_position(), Vec3d::new(2.0, 65.0, 2.0));
        assert_eq!(player.received_move_packet_count(), 2);
        assert_eq!(player.known_move_packet_count(), 2);
        assert_eq!(player.awaiting_teleport(), None);
    }
}
