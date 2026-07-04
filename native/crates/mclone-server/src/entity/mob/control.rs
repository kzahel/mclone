use mclone_core::Vec3d;

const MIN_SPEED_SQR: f64 = 2.5000003E-7;
const MAX_MOVE_TURN_DEGREES: f32 = 90.0;
const DEFAULT_HEAD_TURN_DEGREES: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MoveControl {
    operation: MoveOperation,
    wanted_position: Vec3d,
    speed_modifier: f64,
}

impl Default for MoveControl {
    fn default() -> Self {
        Self {
            operation: MoveOperation::Wait,
            wanted_position: Vec3d::ZERO,
            speed_modifier: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MoveControlTick {
    pub(crate) moved: bool,
    pub(crate) jump_requested: bool,
    pub(crate) speed: f64,
}

impl MoveControl {
    pub(crate) fn has_wanted(&self) -> bool {
        matches!(self.operation, MoveOperation::MoveTo)
    }

    pub(crate) fn set_wanted_position(&mut self, position: Vec3d, speed_modifier: f64) {
        self.wanted_position = position;
        self.speed_modifier = speed_modifier;
        if self.operation != MoveOperation::Jumping {
            self.operation = MoveOperation::MoveTo;
        }
    }

    pub(crate) fn stop(&mut self) {
        self.operation = MoveOperation::Wait;
    }

    pub(crate) fn tick(
        &mut self,
        position: Vec3d,
        y_body_rot_degrees: &mut f32,
        movement_speed: f64,
        on_ground: bool,
        max_up_step: f64,
        entity_width: f32,
    ) -> MoveControlTick {
        match self.operation {
            MoveOperation::Wait => MoveControlTick::default(),
            MoveOperation::Jumping => {
                if on_ground {
                    self.stop();
                    MoveControlTick::default()
                } else {
                    MoveControlTick {
                        moved: true,
                        jump_requested: false,
                        speed: movement_speed * self.speed_modifier,
                    }
                }
            }
            MoveOperation::MoveTo => {
                self.operation = MoveOperation::Wait;
                self.tick_move_to(
                    position,
                    y_body_rot_degrees,
                    movement_speed,
                    max_up_step,
                    entity_width,
                )
            }
        }
    }

    fn tick_move_to(
        &mut self,
        position: Vec3d,
        y_body_rot_degrees: &mut f32,
        movement_speed: f64,
        max_up_step: f64,
        entity_width: f32,
    ) -> MoveControlTick {
        let dx = self.wanted_position.x - position.x;
        let dy = self.wanted_position.y - position.y;
        let dz = self.wanted_position.z - position.z;
        let distance_sqr = dx * dx + dy * dy + dz * dz;
        if distance_sqr < MIN_SPEED_SQR {
            self.stop();
            return MoveControlTick::default();
        }

        let horizontal_distance_sqr = dx * dx + dz * dz;
        let jump_requested =
            dy > max_up_step && horizontal_distance_sqr < f64::from(entity_width).max(1.0);
        if jump_requested {
            self.operation = MoveOperation::Jumping;
        }

        let moved = horizontal_distance_sqr >= f64::EPSILON;
        if moved {
            rotate_toward_wanted(self.wanted_position, position, y_body_rot_degrees);
        }

        MoveControlTick {
            moved,
            jump_requested,
            speed: movement_speed * self.speed_modifier,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MoveOperation {
    Wait,
    MoveTo,
    Jumping,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct JumpControl {
    jump: bool,
}

impl JumpControl {
    pub(crate) fn jump(&mut self) {
        self.jump = true;
    }

    pub(crate) fn tick(&mut self) -> bool {
        let jump = self.jump;
        self.jump = false;
        jump
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LookControl {
    wanted: Option<LookTarget>,
}

impl LookControl {
    pub(crate) fn set_look_at(&mut self, position: Vec3d) {
        self.wanted = Some(LookTarget {
            position,
            y_max_rot_speed: DEFAULT_HEAD_TURN_DEGREES,
        });
    }

    pub(crate) fn tick(
        &mut self,
        position: Vec3d,
        y_head_rot_degrees: &mut f32,
        y_body_rot_degrees: f32,
    ) -> bool {
        if let Some(target) = self.wanted.take() {
            let dx = target.position.x - position.x;
            let dz = target.position.z - position.z;
            if dx.abs() > f64::EPSILON || dz.abs() > f64::EPSILON {
                let wanted_y_rot = yaw_to_target(dx, dz);
                *y_head_rot_degrees =
                    rotate_towards(*y_head_rot_degrees, wanted_y_rot, target.y_max_rot_speed);
                return true;
            }
        }

        let previous = *y_head_rot_degrees;
        *y_head_rot_degrees = rotate_towards(
            *y_head_rot_degrees,
            y_body_rot_degrees,
            DEFAULT_HEAD_TURN_DEGREES,
        );
        *y_head_rot_degrees != previous
    }
}

impl Default for LookControl {
    fn default() -> Self {
        Self { wanted: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LookTarget {
    position: Vec3d,
    y_max_rot_speed: f32,
}

fn yaw_to_target(dx: f64, dz: f64) -> f32 {
    (dz.atan2(dx).to_degrees() as f32) - 90.0
}

fn rotate_toward_wanted(wanted_position: Vec3d, position: Vec3d, y_body_rot_degrees: &mut f32) {
    let dx = wanted_position.x - position.x;
    let dz = wanted_position.z - position.z;
    let wanted_y_rot = yaw_to_target(dx, dz);
    *y_body_rot_degrees = rotlerp(*y_body_rot_degrees, wanted_y_rot, MAX_MOVE_TURN_DEGREES);
}

fn rotlerp(current: f32, wanted: f32, max_delta: f32) -> f32 {
    wrap_degrees(current + degrees_difference(current, wanted).clamp(-max_delta, max_delta))
}

fn rotate_towards(current: f32, wanted: f32, max_delta: f32) -> f32 {
    current + degrees_difference(current, wanted).clamp(-max_delta, max_delta)
}

fn degrees_difference(current: f32, wanted: f32) -> f32 {
    wrap_degrees(wanted - current)
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
    fn move_control_turns_and_steps_toward_target() {
        let mut control = MoveControl::default();
        let position = Vec3d::new(0.0, 64.0, 0.0);
        let mut y_rot = 0.0;
        control.set_wanted_position(Vec3d::new(0.0, 64.0, 4.0), 1.0);

        let tick = control.tick(position, &mut y_rot, 0.2, true, 0.6, 0.9);

        assert!(tick.moved);
        assert!(!tick.jump_requested);
        assert_eq!(tick.speed, 0.2);
        assert_eq!(y_rot, 0.0);
        assert!(!control.has_wanted());
    }

    #[test]
    fn move_control_requests_jump_for_close_high_target() {
        let mut control = MoveControl::default();
        let position = Vec3d::new(0.5, 64.0, 0.5);
        let mut y_rot = 0.0;
        control.set_wanted_position(Vec3d::new(0.5, 65.0, 0.5), 1.0);

        let tick = control.tick(position, &mut y_rot, 0.2, true, 0.6, 0.9);

        assert!(!tick.moved);
        assert!(tick.jump_requested);
    }

    #[test]
    fn jump_control_pulses_once_per_request() {
        let mut control = JumpControl::default();
        control.jump();

        assert!(control.tick());
        assert!(!control.tick());
    }

    #[test]
    fn look_control_rotates_head_toward_target() {
        let mut control = LookControl::default();
        let mut y_head_rot = 0.0;
        control.set_look_at(Vec3d::new(-4.0, 65.0, 0.0));

        assert!(control.tick(Vec3d::new(0.0, 64.0, 0.0), &mut y_head_rot, 0.0));

        assert_eq!(y_head_rot, 10.0);
    }
}
