use super::*;

const NANOS_PER_SECOND: u128 = 1_000_000_000;
const PRESENTATION_EYE_EPSILON_SQR: f64 = 1.0e-12;

pub const DEFAULT_PLAYER_MOVEMENT_RATE_HZ: u32 = 60;
pub const DEFAULT_PLAYER_MOVEMENT_MAX_CATCH_UP_STEPS: u32 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerMovementCadenceConfig {
    pub rate_hz: u32,
    pub max_catch_up_steps: u32,
}

impl PlayerMovementCadenceConfig {
    pub const fn new(rate_hz: u32, max_catch_up_steps: u32) -> Self {
        Self {
            rate_hz,
            max_catch_up_steps,
        }
    }

    pub const fn is_valid(self) -> bool {
        self.rate_hz > 0 && self.rate_hz <= 1_000 && self.max_catch_up_steps > 0
    }

    pub fn step_seconds(self) -> f64 {
        1.0 / f64::from(self.rate_hz)
    }
}

impl Default for PlayerMovementCadenceConfig {
    fn default() -> Self {
        Self::new(
            DEFAULT_PLAYER_MOVEMENT_RATE_HZ,
            DEFAULT_PLAYER_MOVEMENT_MAX_CATCH_UP_STEPS,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerMovementAdvance {
    pub steps: u32,
    pub dropped_steps: u32,
    pub interpolation_alpha: f64,
}

/// Scene-owned local-player command clock and presentation timeline.
///
/// Platform hosts contribute elapsed wall time and the latest semantic input
/// state. This owner turns that into bounded fixed quanta so render, browser
/// animation-frame, Android surface, and OpenXR cadence never become movement
/// integration `dt`.
#[derive(Clone, Debug)]
pub(crate) struct PlayerMovementState {
    config: PlayerMovementCadenceConfig,
    accumulator_rate_nanos: u128,
    latest_input: EngineCameraInput,
    pending_jump_edge: bool,
    previous_eye: Vec3d,
    current_eye: Vec3d,
    interpolation_alpha: f64,
    dropped_steps: u64,
}

impl PlayerMovementState {
    pub(crate) fn new(config: PlayerMovementCadenceConfig, initial_eye: Vec3d) -> Self {
        debug_assert!(config.is_valid());
        Self {
            config,
            accumulator_rate_nanos: 0,
            latest_input: EngineCameraInput::default(),
            pending_jump_edge: false,
            previous_eye: initial_eye,
            current_eye: initial_eye,
            interpolation_alpha: 0.0,
            dropped_steps: 0,
        }
    }

    pub(crate) fn observe_input(&mut self, mut input: EngineCameraInput) {
        self.pending_jump_edge |= input.jump;
        input.dt_seconds = 0.0;
        input.mouse_delta_x = 0.0;
        input.mouse_delta_y = 0.0;
        self.latest_input = input;
    }

    pub(crate) const fn config(&self) -> PlayerMovementCadenceConfig {
        self.config
    }

    pub(crate) const fn total_dropped_steps(&self) -> u64 {
        self.dropped_steps
    }

    pub(crate) fn advance_elapsed(&mut self, elapsed_seconds: f64) -> PlayerMovementAdvance {
        let elapsed_nanos = finite_elapsed_nanos(elapsed_seconds);
        self.accumulator_rate_nanos = self
            .accumulator_rate_nanos
            .saturating_add(elapsed_nanos.saturating_mul(u128::from(self.config.rate_hz)));
        let due_steps = self.accumulator_rate_nanos / NANOS_PER_SECOND;
        self.accumulator_rate_nanos %= NANOS_PER_SECOND;

        let admitted_steps = due_steps.min(u128::from(self.config.max_catch_up_steps)) as u32;
        let dropped_steps = due_steps
            .saturating_sub(u128::from(admitted_steps))
            .min(u128::from(u32::MAX)) as u32;
        self.dropped_steps = self.dropped_steps.saturating_add(u64::from(dropped_steps));
        self.interpolation_alpha = self.accumulator_rate_nanos as f64 / NANOS_PER_SECOND as f64;

        PlayerMovementAdvance {
            steps: admitted_steps,
            dropped_steps,
            interpolation_alpha: self.interpolation_alpha,
        }
    }

    pub(crate) fn next_step_input(&mut self) -> EngineCameraInput {
        let mut input = self.latest_input;
        input.dt_seconds = self.config.step_seconds();
        if self.pending_jump_edge {
            input.jump = true;
            self.pending_jump_edge = false;
        }
        input
    }

    pub(crate) fn synchronize_eye(&mut self, authoritative_eye: Vec3d) {
        if authoritative_eye.subtract(self.current_eye).length_sqr() > PRESENTATION_EYE_EPSILON_SQR
        {
            self.previous_eye = authoritative_eye;
            self.current_eye = authoritative_eye;
            self.interpolation_alpha = 0.0;
        }
    }

    pub(crate) fn record_step(&mut self, eye: Vec3d) {
        self.previous_eye = self.current_eye;
        self.current_eye = eye;
    }

    pub(crate) fn presentation_eye(&self, authoritative_eye: Vec3d) -> Vec3d {
        if authoritative_eye.subtract(self.current_eye).length_sqr() > PRESENTATION_EYE_EPSILON_SQR
        {
            return authoritative_eye;
        }
        let alpha = self.interpolation_alpha.clamp(0.0, 1.0);
        self.previous_eye
            .add(self.current_eye.subtract(self.previous_eye).scale(alpha))
    }

    pub(crate) fn reset(&mut self, eye: Vec3d) {
        self.accumulator_rate_nanos = 0;
        self.latest_input = EngineCameraInput::default();
        self.pending_jump_edge = false;
        self.previous_eye = eye;
        self.current_eye = eye;
        self.interpolation_alpha = 0.0;
    }
}

fn finite_elapsed_nanos(elapsed_seconds: f64) -> u128 {
    if !elapsed_seconds.is_finite() || elapsed_seconds <= 0.0 {
        return 0;
    }
    Duration::from_secs_f64(elapsed_seconds).as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PlayerMovementState {
        PlayerMovementState::new(
            PlayerMovementCadenceConfig::default(),
            Vec3d::new(0.0, 64.0, 0.0),
        )
    }

    #[test]
    fn default_player_movement_rate_is_sixty_hz() {
        let config = PlayerMovementCadenceConfig::default();
        assert_eq!(config.rate_hz, 60);
        assert!((config.step_seconds() - 1.0 / 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ten_fps_presentation_emits_six_movement_steps_without_loss() {
        let mut state = state();
        let mut total = 0;
        for _ in 0..10 {
            let advance = state.advance_elapsed(0.1);
            assert_eq!(advance.steps, 6);
            assert_eq!(advance.dropped_steps, 0);
            total += advance.steps;
        }
        assert_eq!(total, 60);
    }

    #[test]
    fn five_fps_presentation_reaches_the_default_catch_up_boundary() {
        let mut state = state();
        let advance = state.advance_elapsed(0.2);
        assert_eq!(advance.steps, 12);
        assert_eq!(advance.dropped_steps, 0);
    }

    #[test]
    fn held_input_produces_the_same_quanta_at_fifty_and_ten_fps() {
        fn command_timeline(presentation_rate_hz: u32) -> Vec<bool> {
            let mut state = state();
            let frame_seconds = 1.0 / f64::from(presentation_rate_hz);
            let mut timeline = Vec::new();
            for frame in 0..presentation_rate_hz {
                state.observe_input(EngineCameraInput {
                    forward: frame < presentation_rate_hz / 2,
                    ..EngineCameraInput::default()
                });
                let advance = state.advance_elapsed(frame_seconds);
                for _ in 0..advance.steps {
                    timeline.push(state.next_step_input().forward);
                }
            }
            timeline
        }

        let at_fifty_fps = command_timeline(50);
        let at_ten_fps = command_timeline(10);
        assert_eq!(at_fifty_fps.len(), 60);
        assert_eq!(at_fifty_fps, at_ten_fps);
        assert_eq!(at_fifty_fps.iter().filter(|held| **held).count(), 30);
    }

    #[test]
    fn pathological_stall_drops_only_work_beyond_the_bounded_debt() {
        let mut state = state();
        let advance = state.advance_elapsed(1.0);
        assert_eq!(advance.steps, 12);
        assert_eq!(advance.dropped_steps, 48);
        assert_eq!(state.total_dropped_steps(), 48);
    }

    #[test]
    fn jump_press_survives_a_zero_step_frame_and_release() {
        let mut state = state();
        state.observe_input(EngineCameraInput {
            jump: true,
            ..EngineCameraInput::default()
        });
        assert_eq!(state.advance_elapsed(0.00834).steps, 0);
        state.observe_input(EngineCameraInput::default());

        assert_eq!(state.advance_elapsed(0.00834).steps, 1);
        assert!(state.next_step_input().jump);
        assert!(!state.next_step_input().jump);
    }

    #[test]
    fn presentation_interpolates_only_when_camera_matches_timeline() {
        let mut state = state();
        state.record_step(Vec3d::new(1.0, 64.0, 0.0));
        state.interpolation_alpha = 0.25;
        assert_eq!(
            state.presentation_eye(Vec3d::new(1.0, 64.0, 0.0)),
            Vec3d::new(0.25, 64.0, 0.0)
        );
        assert_eq!(
            state.presentation_eye(Vec3d::new(8.0, 70.0, 9.0)),
            Vec3d::new(8.0, 70.0, 9.0)
        );
    }
}
