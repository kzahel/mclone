use mclone_core::Vec3d;

const MIN_SPEED_SQR: f64 = 2.5000003E-7;
const MAX_MOVE_TURN_DEGREES: f32 = 90.0;
const DEFAULT_HEAD_TURN_DEGREES: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MoveControl {
    operation: MoveOperation,
}

impl Default for MoveControl {
    fn default() -> Self {
        Self {
            operation: MoveOperation::Wait,
        }
    }
}

impl MoveControl {
    pub(crate) fn has_wanted(&self) -> bool {
        matches!(self.operation, MoveOperation::MoveTo { .. })
    }

    pub(crate) fn set_wanted_position(&mut self, position: Vec3d, speed_modifier: f64) {
        self.operation = MoveOperation::MoveTo {
            position,
            speed_modifier,
        };
    }

    pub(crate) fn stop(&mut self) {
        self.operation = MoveOperation::Wait;
    }

    pub(crate) fn tick(
        &mut self,
        position: &mut Vec3d,
        y_body_rot_degrees: &mut f32,
        movement_speed: f64,
    ) -> bool {
        let MoveOperation::MoveTo {
            position: wanted,
            speed_modifier,
        } = self.operation
        else {
            return false;
        };

        let dx = wanted.x - position.x;
        let dy = wanted.y - position.y;
        let dz = wanted.z - position.z;
        let distance_sqr = dx * dx + dy * dy + dz * dz;
        if distance_sqr < MIN_SPEED_SQR {
            self.stop();
            return false;
        }

        let wanted_y_rot = yaw_to_target(dx, dz);
        *y_body_rot_degrees = rotlerp(*y_body_rot_degrees, wanted_y_rot, MAX_MOVE_TURN_DEGREES);

        let horizontal_distance = (dx * dx + dz * dz).sqrt();
        if horizontal_distance < f64::EPSILON {
            self.stop();
            return false;
        }

        let step = movement_speed * speed_modifier;
        if step >= horizontal_distance {
            *position = Vec3d::new(wanted.x, position.y, wanted.z);
            self.stop();
            return true;
        }

        let radians = (*y_body_rot_degrees as f64).to_radians();
        position.x += -radians.sin() * step;
        position.z += radians.cos() * step;
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MoveOperation {
    Wait,
    MoveTo {
        position: Vec3d,
        speed_modifier: f64,
    },
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
        let mut position = Vec3d::new(0.0, 64.0, 0.0);
        let mut y_rot = 0.0;
        control.set_wanted_position(Vec3d::new(0.0, 64.0, 4.0), 1.0);

        assert!(control.tick(&mut position, &mut y_rot, 0.2));

        assert_eq!(y_rot, 0.0);
        assert_eq!(position, Vec3d::new(0.0, 64.0, 0.2));
        assert!(control.has_wanted());
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
