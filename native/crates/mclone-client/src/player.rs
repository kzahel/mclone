use mclone_core::Vec3d;

pub const NO_CLIP_BOOST_MULTIPLIER: f64 = 3.0;
pub const MOVING_SLOW_FACTOR: f32 = 0.3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerInputKey {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Descend,
    Boost,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerInputKeys {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub descend: bool,
    pub boost: bool,
}

impl PlayerInputKeys {
    pub fn set(&mut self, key: PlayerInputKey, down: bool) {
        match key {
            PlayerInputKey::Forward => self.forward = down,
            PlayerInputKey::Backward => self.backward = down,
            PlayerInputKey::Left => self.left = down,
            PlayerInputKey::Right => self.right = down,
            PlayerInputKey::Jump => self.jump = down,
            PlayerInputKey::Descend => self.descend = down,
            PlayerInputKey::Boost => self.boost = down,
        }
    }

    pub fn is_down(self, key: PlayerInputKey) -> bool {
        match key {
            PlayerInputKey::Forward => self.forward,
            PlayerInputKey::Backward => self.backward,
            PlayerInputKey::Left => self.left,
            PlayerInputKey::Right => self.right,
            PlayerInputKey::Jump => self.jump,
            PlayerInputKey::Descend => self.descend,
            PlayerInputKey::Boost => self.boost,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInput {
    pub left_impulse: f32,
    pub forward_impulse: f32,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub jumping: bool,
    pub shift_key_down: bool,
    pub descending: bool,
    pub boosting: bool,
}

impl PlayerInput {
    pub fn tick(&mut self, keys: PlayerInputKeys, moving_slowly: bool) {
        self.up = keys.forward;
        self.down = keys.backward;
        self.left = keys.left;
        self.right = keys.right;
        self.forward_impulse = axis(self.up, self.down);
        self.left_impulse = axis(self.left, self.right);
        self.jumping = keys.jump;
        self.shift_key_down = keys.boost;
        self.descending = keys.descend;
        self.boosting = keys.boost;
        if moving_slowly {
            self.left_impulse *= MOVING_SLOW_FACTOR;
            self.forward_impulse *= MOVING_SLOW_FACTOR;
        }
    }

    pub const fn move_vector(self) -> (f32, f32) {
        (self.left_impulse, self.forward_impulse)
    }

    pub fn has_forward_impulse(self) -> bool {
        self.forward_impulse > 1.0e-5
    }

    fn vertical_axis(self) -> f64 {
        axis(self.jumping, self.descending) as f64
    }

    fn right_axis(self) -> f64 {
        -self.left_impulse as f64
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoClipMovementStep {
    pub yaw_radians: f64,
    pub pitch_radians: f64,
    pub speed_blocks_per_second: f64,
    pub dt_seconds: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalPlayerController {
    keys: PlayerInputKeys,
    input: PlayerInput,
}

impl LocalPlayerController {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn keys(&self) -> PlayerInputKeys {
        self.keys
    }

    pub const fn input(&self) -> PlayerInput {
        self.input
    }

    pub fn set_key(&mut self, key: PlayerInputKey, down: bool) {
        self.keys.set(key, down);
    }

    pub fn clear_keys(&mut self) {
        self.keys.clear();
        self.input = PlayerInput::default();
    }

    pub fn tick_input(&mut self, moving_slowly: bool) -> PlayerInput {
        self.input.tick(self.keys, moving_slowly);
        self.input
    }

    pub fn tick_no_clip_movement(&mut self, step: NoClipMovementStep) -> Option<Vec3d> {
        let input = self.tick_input(false);
        no_clip_displacement(input, step)
    }
}

pub fn no_clip_displacement(input: PlayerInput, step: NoClipMovementStep) -> Option<Vec3d> {
    if !step.yaw_radians.is_finite()
        || !step.pitch_radians.is_finite()
        || !step.speed_blocks_per_second.is_finite()
        || !step.dt_seconds.is_finite()
        || step.dt_seconds <= 0.0
        || step.speed_blocks_per_second <= 0.0
    {
        return None;
    }

    let forward = view_vector(step.yaw_radians, step.pitch_radians);
    let right = normalize_or_zero(cross(forward, Vec3d::new(0.0, 1.0, 0.0)));
    let direction = right
        .scale(input.right_axis())
        .add(Vec3d::new(0.0, input.vertical_axis(), 0.0))
        .add(forward.scale(input.forward_impulse as f64));
    let direction = normalize_or_zero(direction);
    if direction == Vec3d::ZERO {
        return None;
    }

    let boost = if input.boosting {
        NO_CLIP_BOOST_MULTIPLIER
    } else {
        1.0
    };
    Some(direction.scale(step.speed_blocks_per_second * boost * step.dt_seconds))
}

pub fn view_vector(yaw_radians: f64, pitch_radians: f64) -> Vec3d {
    if !yaw_radians.is_finite() || !pitch_radians.is_finite() {
        return Vec3d::ZERO;
    }
    let (yaw_sin, yaw_cos) = yaw_radians.sin_cos();
    let (pitch_sin, pitch_cos) = pitch_radians.sin_cos();
    normalize_or_zero(Vec3d::new(
        yaw_sin * pitch_cos,
        pitch_sin,
        yaw_cos * pitch_cos,
    ))
}

fn axis(positive: bool, negative: bool) -> f32 {
    let positive = positive as i32;
    let negative = negative as i32;
    (positive - negative) as f32
}

fn cross(a: Vec3d, b: Vec3d) -> Vec3d {
    Vec3d::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn normalize_or_zero(value: Vec3d) -> Vec3d {
    let len_sqr = value.length_sqr();
    if len_sqr <= 1.0e-12 {
        Vec3d::ZERO
    } else {
        value.scale(1.0 / len_sqr.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_tick_matches_java_keyboard_impulses() {
        let mut keys = PlayerInputKeys::default();
        let mut input = PlayerInput::default();

        keys.set(PlayerInputKey::Forward, true);
        keys.set(PlayerInputKey::Left, true);
        input.tick(keys, false);

        assert_eq!(input.forward_impulse, 1.0);
        assert_eq!(input.left_impulse, 1.0);
        assert!(input.has_forward_impulse());

        keys.set(PlayerInputKey::Backward, true);
        keys.set(PlayerInputKey::Right, true);
        input.tick(keys, false);

        assert_eq!(input.forward_impulse, 0.0);
        assert_eq!(input.left_impulse, 0.0);
        assert!(!input.has_forward_impulse());
    }

    #[test]
    fn moving_slowly_scales_horizontal_impulses_only() {
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        keys.set(PlayerInputKey::Jump, true);
        let mut input = PlayerInput::default();

        input.tick(keys, true);

        assert_eq!(input.forward_impulse, MOVING_SLOW_FACTOR);
        assert!(input.jumping);
    }

    #[test]
    fn no_clip_movement_preserves_existing_forward_axis() {
        let mut input = PlayerInput::default();
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        input.tick(keys, false);

        let displacement = no_clip_displacement(
            input,
            NoClipMovementStep {
                yaw_radians: 0.0,
                pitch_radians: 0.0,
                speed_blocks_per_second: 10.0,
                dt_seconds: 0.5,
            },
        )
        .expect("movement");

        assert_eq!(displacement, Vec3d::new(0.0, 0.0, 5.0));
    }

    #[test]
    fn no_clip_movement_normalizes_diagonal_and_applies_boost() {
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        keys.set(PlayerInputKey::Jump, true);
        keys.set(PlayerInputKey::Boost, true);
        let mut input = PlayerInput::default();
        input.tick(keys, false);

        let displacement = no_clip_displacement(
            input,
            NoClipMovementStep {
                yaw_radians: 0.0,
                pitch_radians: 0.0,
                speed_blocks_per_second: 2.0,
                dt_seconds: 1.0,
            },
        )
        .expect("movement");

        assert!((displacement.length_sqr().sqrt() - 6.0).abs() < 1.0e-9);
        assert!(displacement.y > 0.0);
        assert!(displacement.z > 0.0);
    }

    #[test]
    fn controller_clears_keys_and_cached_input() {
        let mut controller = LocalPlayerController::new();
        controller.set_key(PlayerInputKey::Forward, true);
        controller.tick_input(false);

        controller.clear_keys();

        assert_eq!(controller.keys(), PlayerInputKeys::default());
        assert_eq!(controller.input(), PlayerInput::default());
    }

    #[test]
    fn no_clip_movement_rejects_non_finite_step_inputs() {
        let mut keys = PlayerInputKeys::default();
        keys.set(PlayerInputKey::Forward, true);
        let mut input = PlayerInput::default();
        input.tick(keys, false);

        assert_eq!(
            no_clip_displacement(
                input,
                NoClipMovementStep {
                    yaw_radians: f64::NAN,
                    pitch_radians: 0.0,
                    speed_blocks_per_second: 1.0,
                    dt_seconds: 1.0,
                },
            ),
            None
        );
        assert_eq!(view_vector(f64::NAN, 0.0), Vec3d::ZERO);
    }
}
