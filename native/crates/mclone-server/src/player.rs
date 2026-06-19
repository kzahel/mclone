use mclone_core::Vec3d;
use mclone_protocol::{MovePlayerCommand, PlayerPositionRelativeFlags, PlayerPositionUpdate};

const JAVA_HORIZONTAL_MOVE_BOUND: f64 = 3.0e7;
const JAVA_VERTICAL_MOVE_BOUND: f64 = 2.0e7;
const JAVA_TOO_FAST_MOVE_THRESHOLD: f64 = 100.0;
const JAVA_MOVE_PACKET_BURST_LIMIT: u32 = 5;
const JAVA_MAX_TELEPORT_ID: u32 = i32::MAX as u32;

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
    teleport_id_counter: u32,
    has_accepted_position: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AwaitingTeleport {
    pub(crate) id: u32,
    pub(crate) tick: u64,
    pub(crate) position: Vec3d,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MovePlayerApplyResult {
    Accepted,
    RejectedInvalid,
    AwaitingTeleport,
    RejectedTooFast {
        attempted_position: Vec3d,
        first_good_position: Vec3d,
        movement_delta_sqr: f64,
        velocity_delta_sqr: f64,
        allowed_delta_sqr: f64,
        packet_count: u32,
    },
}

impl MovePlayerApplyResult {
    #[cfg(test)]
    pub(crate) const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }
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
            teleport_id_counter: 0,
            has_accepted_position: false,
        }
    }
}

impl ServerPlayerState {
    pub(crate) fn apply_move_player(
        &mut self,
        command: MovePlayerCommand,
    ) -> MovePlayerApplyResult {
        let position = command.position_or(self.position);
        let y_rot_degrees = command.y_rot_degrees_or(self.y_rot_degrees);
        let x_rot_degrees = command.x_rot_degrees_or(self.x_rot_degrees);
        if !position.is_finite() || !y_rot_degrees.is_finite() || !x_rot_degrees.is_finite() {
            return MovePlayerApplyResult::RejectedInvalid;
        }
        if self.awaiting_teleport.is_some() {
            return MovePlayerApplyResult::AwaitingTeleport;
        }

        let position = Vec3d::new(
            clamp_horizontal(position.x),
            clamp_vertical(position.y),
            clamp_horizontal(position.z),
        );
        let has_position = command.has_position();
        self.received_move_packet_count = self.received_move_packet_count.saturating_add(1);
        if has_position && self.has_accepted_position {
            let packet_count = self.effective_packet_count_since_tick();
            let movement_delta_sqr = position.distance_to_sqr(self.first_good_position);
            let velocity_delta_sqr = self.velocity_delta_sqr();
            let allowed_delta_sqr = JAVA_TOO_FAST_MOVE_THRESHOLD * f64::from(packet_count);
            if movement_delta_sqr - velocity_delta_sqr > allowed_delta_sqr {
                return MovePlayerApplyResult::RejectedTooFast {
                    attempted_position: position,
                    first_good_position: self.first_good_position,
                    movement_delta_sqr,
                    velocity_delta_sqr,
                    allowed_delta_sqr,
                    packet_count,
                };
            }
        }

        self.position = position;
        self.y_rot_degrees = wrap_degrees(y_rot_degrees);
        self.x_rot_degrees = wrap_degrees(x_rot_degrees);
        self.on_ground = command.on_ground();
        if has_position && !self.has_accepted_position {
            self.first_good_position = self.position;
            self.has_accepted_position = true;
        }
        self.last_good_position = self.position;
        MovePlayerApplyResult::Accepted
    }

    pub(crate) fn correction_update(&mut self, tick: u64) -> PlayerPositionUpdate {
        let id = self.next_teleport_id();
        self.awaiting_teleport = Some(AwaitingTeleport {
            id,
            tick,
            position: self.position,
        });
        PlayerPositionUpdate {
            position: self.position,
            y_rot_degrees: self.y_rot_degrees,
            x_rot_degrees: self.x_rot_degrees,
            relative: PlayerPositionRelativeFlags::ABSOLUTE,
            teleport_id: id,
            dismount_vehicle: false,
        }
    }

    pub(crate) fn accept_teleport(&mut self, id: u32) -> bool {
        let Some(awaiting) = self.awaiting_teleport else {
            return false;
        };
        if awaiting.id != id {
            return false;
        }
        self.position = awaiting.position;
        self.last_good_position = awaiting.position;
        self.has_accepted_position = true;
        self.awaiting_teleport = None;
        true
    }

    pub(crate) fn mark_tick_boundary(&mut self) {
        self.first_good_position = self.position;
        self.last_good_position = self.position;
        self.known_move_packet_count = self.received_move_packet_count;
    }

    fn next_teleport_id(&mut self) -> u32 {
        let mut id = self.teleport_id_counter.saturating_add(1);
        if id == JAVA_MAX_TELEPORT_ID {
            id = 0;
        }
        self.teleport_id_counter = id;
        id
    }

    fn effective_packet_count_since_tick(self) -> u32 {
        let packet_count = self
            .received_move_packet_count
            .saturating_sub(self.known_move_packet_count);
        if packet_count > JAVA_MOVE_PACKET_BURST_LIMIT {
            1
        } else {
            packet_count.max(1)
        }
    }

    fn velocity_delta_sqr(self) -> f64 {
        0.0
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

    #[cfg(test)]
    pub(crate) const fn has_accepted_position(self) -> bool {
        self.has_accepted_position
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

        assert!(
            player
                .apply_move_player(MovePlayerCommand::PosRot {
                    position: Vec3d::new(4.0e7, 3.0e7, -4.0e7),
                    y_rot_degrees: 181.0,
                    x_rot_degrees: -181.0,
                    on_ground: true,
                })
                .is_accepted()
        );

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
        assert_eq!(player.first_good_position(), player.position());
        assert_eq!(player.last_good_position(), player.position());
        assert_eq!(player.received_move_packet_count(), 1);
        assert_eq!(player.known_move_packet_count(), 0);
        assert!(player.has_accepted_position());
    }

    #[test]
    fn move_player_rejects_non_finite_values_without_mutating_state() {
        let mut player = ServerPlayerState::default();
        assert!(
            player
                .apply_move_player(MovePlayerCommand::PosRot {
                    position: Vec3d::new(1.0, 2.0, 3.0),
                    y_rot_degrees: 45.0,
                    x_rot_degrees: 0.0,
                    on_ground: true,
                })
                .is_accepted()
        );
        let previous = player;

        assert_eq!(
            player.apply_move_player(MovePlayerCommand::PosRot {
                position: Vec3d::new(f64::NAN, 2.0, 3.0),
                y_rot_degrees: 0.0,
                x_rot_degrees: 0.0,
                on_ground: false,
            }),
            MovePlayerApplyResult::RejectedInvalid
        );
        assert_eq!(
            player.apply_move_player(MovePlayerCommand::Rot {
                y_rot_degrees: f32::INFINITY,
                x_rot_degrees: 0.0,
                on_ground: false,
            }),
            MovePlayerApplyResult::RejectedInvalid
        );

        assert_eq!(player, previous);
    }

    #[test]
    fn move_player_variants_preserve_missing_position_or_rotation() {
        let mut player = ServerPlayerState::default();

        assert!(
            player
                .apply_move_player(MovePlayerCommand::PosRot {
                    position: Vec3d::new(1.0, 2.0, 3.0),
                    y_rot_degrees: 30.0,
                    x_rot_degrees: -15.0,
                    on_ground: true,
                })
                .is_accepted()
        );
        assert!(
            player
                .apply_move_player(MovePlayerCommand::Rot {
                    y_rot_degrees: 45.0,
                    x_rot_degrees: 10.0,
                    on_ground: false,
                })
                .is_accepted()
        );

        assert_eq!(player.position(), Vec3d::new(1.0, 2.0, 3.0));
        assert_eq!(player.y_rot_degrees(), 45.0);
        assert_eq!(player.x_rot_degrees(), 10.0);
        assert!(!player.on_ground());

        assert!(
            player
                .apply_move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(4.0, 5.0, 6.0),
                    on_ground: true,
                })
                .is_accepted()
        );
        assert_eq!(player.position(), Vec3d::new(4.0, 5.0, 6.0));
        assert_eq!(player.y_rot_degrees(), 45.0);
        assert_eq!(player.x_rot_degrees(), 10.0);
        assert!(player.on_ground());

        assert!(
            player
                .apply_move_player(MovePlayerCommand::StatusOnly { on_ground: false })
                .is_accepted()
        );
        assert_eq!(player.position(), Vec3d::new(4.0, 5.0, 6.0));
        assert_eq!(player.y_rot_degrees(), 45.0);
        assert_eq!(player.x_rot_degrees(), 10.0);
        assert!(!player.on_ground());
        assert_eq!(player.received_move_packet_count(), 4);
    }

    #[test]
    fn movement_tick_boundary_tracks_known_packet_count_and_good_positions() {
        let mut player = ServerPlayerState::default();

        assert!(
            player
                .apply_move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(1.0, 64.0, 1.0),
                    on_ground: true,
                })
                .is_accepted()
        );
        assert!(
            player
                .apply_move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(2.0, 65.0, 2.0),
                    on_ground: false,
                })
                .is_accepted()
        );
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

    #[test]
    fn first_position_packet_bootstraps_native_server_player_sync() {
        let mut player = ServerPlayerState::default();

        assert!(
            player
                .apply_move_player(MovePlayerCommand::Rot {
                    y_rot_degrees: 90.0,
                    x_rot_degrees: 0.0,
                    on_ground: false,
                })
                .is_accepted()
        );
        assert!(!player.has_accepted_position());

        assert!(
            player
                .apply_move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(80.0, 90.0, -80.0),
                    on_ground: true,
                })
                .is_accepted()
        );

        assert_eq!(player.position(), Vec3d::new(80.0, 90.0, -80.0));
        assert_eq!(player.first_good_position(), Vec3d::new(80.0, 90.0, -80.0));
        assert_eq!(player.last_good_position(), Vec3d::new(80.0, 90.0, -80.0));
        assert_eq!(player.received_move_packet_count(), 2);
        assert!(player.has_accepted_position());
    }

    #[test]
    fn move_player_rejects_too_fast_position_without_mutating_pose() {
        let mut player = ServerPlayerState::default();
        assert!(
            player
                .apply_move_player(MovePlayerCommand::PosRot {
                    position: Vec3d::new(0.0, 64.0, 0.0),
                    y_rot_degrees: 45.0,
                    x_rot_degrees: 10.0,
                    on_ground: true,
                })
                .is_accepted()
        );
        player.mark_tick_boundary();
        let previous = player;

        let result = player.apply_move_player(MovePlayerCommand::PosRot {
            position: Vec3d::new(11.0, 64.0, 0.0),
            y_rot_degrees: 90.0,
            x_rot_degrees: -30.0,
            on_ground: false,
        });

        assert_eq!(
            result,
            MovePlayerApplyResult::RejectedTooFast {
                attempted_position: Vec3d::new(11.0, 64.0, 0.0),
                first_good_position: Vec3d::new(0.0, 64.0, 0.0),
                movement_delta_sqr: 121.0,
                velocity_delta_sqr: 0.0,
                allowed_delta_sqr: 100.0,
                packet_count: 1,
            }
        );
        assert_eq!(player.position(), previous.position());
        assert_eq!(player.y_rot_degrees(), previous.y_rot_degrees());
        assert_eq!(player.x_rot_degrees(), previous.x_rot_degrees());
        assert_eq!(player.on_ground(), previous.on_ground());
        assert_eq!(player.last_good_position(), previous.last_good_position());
        assert_eq!(player.received_move_packet_count(), 2);
        assert_eq!(player.known_move_packet_count(), 1);
    }

    #[test]
    fn correction_update_tracks_awaiting_teleport_and_ack() {
        let mut player = ServerPlayerState::default();
        assert!(
            player
                .apply_move_player(MovePlayerCommand::PosRot {
                    position: Vec3d::new(1.0, 64.0, 2.0),
                    y_rot_degrees: 90.0,
                    x_rot_degrees: 10.0,
                    on_ground: true,
                })
                .is_accepted()
        );

        let update = player.correction_update(7);

        assert_eq!(
            update,
            PlayerPositionUpdate {
                position: Vec3d::new(1.0, 64.0, 2.0),
                y_rot_degrees: 90.0,
                x_rot_degrees: 10.0,
                relative: PlayerPositionRelativeFlags::ABSOLUTE,
                teleport_id: 1,
                dismount_vehicle: false,
            }
        );
        assert_eq!(
            player.awaiting_teleport(),
            Some(AwaitingTeleport {
                id: 1,
                tick: 7,
                position: Vec3d::new(1.0, 64.0, 2.0),
            })
        );

        assert_eq!(
            player.apply_move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(2.0, 64.0, 2.0),
                on_ground: true,
            }),
            MovePlayerApplyResult::AwaitingTeleport
        );
        assert_eq!(player.position(), Vec3d::new(1.0, 64.0, 2.0));
        assert_eq!(player.received_move_packet_count(), 1);

        assert!(!player.accept_teleport(2));
        assert_eq!(
            player.awaiting_teleport().map(|awaiting| awaiting.id),
            Some(1)
        );
        assert!(player.accept_teleport(1));
        assert_eq!(player.awaiting_teleport(), None);
        assert_eq!(player.last_good_position(), Vec3d::new(1.0, 64.0, 2.0));
    }

    #[test]
    fn move_player_uses_java_packet_count_threshold_and_burst_clamp() {
        let mut player = ServerPlayerState::default();
        assert!(
            player
                .apply_move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(0.0, 64.0, 0.0),
                    on_ground: true,
                })
                .is_accepted()
        );

        assert!(
            player
                .apply_move_player(MovePlayerCommand::Pos {
                    position: Vec3d::new(14.0, 64.0, 0.0),
                    on_ground: true,
                })
                .is_accepted()
        );
        assert_eq!(player.received_move_packet_count(), 2);
        assert_eq!(player.known_move_packet_count(), 0);

        player.mark_tick_boundary();
        for index in 0..JAVA_MOVE_PACKET_BURST_LIMIT {
            assert!(
                player
                    .apply_move_player(MovePlayerCommand::Pos {
                        position: Vec3d::new(15.0 + f64::from(index), 64.0, 0.0),
                        on_ground: true,
                    })
                    .is_accepted()
            );
        }

        assert_eq!(
            player.apply_move_player(MovePlayerCommand::Pos {
                position: Vec3d::new(32.0, 64.0, 0.0),
                on_ground: true,
            }),
            MovePlayerApplyResult::RejectedTooFast {
                attempted_position: Vec3d::new(32.0, 64.0, 0.0),
                first_good_position: Vec3d::new(14.0, 64.0, 0.0),
                movement_delta_sqr: 324.0,
                velocity_delta_sqr: 0.0,
                allowed_delta_sqr: 100.0,
                packet_count: 1,
            }
        );
        assert_eq!(player.position(), Vec3d::new(19.0, 64.0, 0.0));
    }
}
