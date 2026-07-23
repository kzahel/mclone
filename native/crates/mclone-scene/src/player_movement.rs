use super::*;
use std::collections::VecDeque;

const NANOS_PER_SECOND: u128 = 1_000_000_000;
const PRESENTATION_EYE_EPSILON_SQR: f64 = 1.0e-12;
const MAX_RECORDED_PLAYER_MOVEMENT_COMMANDS: usize = 1_024;
const MAX_PENDING_PLAYER_INPUT_OBSERVATIONS: usize = 2_048;

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

/// One deterministic fixed-rate local-player command.
///
/// Sequence numbers count movement quanta, including gaps explicitly dropped
/// by bounded catch-up. The command contains no platform controls or render
/// frame delta and is therefore the local record/replay and future network
/// replay boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerMovementCommand {
    pub epoch: u64,
    pub sequence: u64,
    pub input: EngineCameraInput,
}

#[derive(Clone, Copy, Debug)]
struct TimedMovementObservation {
    observed_at: MonotonicInstant,
    sequence: u64,
    input: Option<EngineCameraInput>,
    look_rate_mouse_delta_per_second: Option<[f64; 2]>,
    exact_movement_yaw_radians: Option<f64>,
    exact_movement_pitch_radians: Option<f64>,
    use_movement_pitch: Option<bool>,
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
    timeline_origin: Option<MonotonicInstant>,
    timeline_elapsed_nanos: u128,
    command_epoch: u64,
    next_command_sequence: u64,
    next_observation_sequence: u64,
    pending_observations: VecDeque<TimedMovementObservation>,
    dropped_input_observations: u64,
    pending_commands: VecDeque<PlayerMovementCommand>,
    recorded_commands: VecDeque<PlayerMovementCommand>,
    dropped_recorded_commands: u64,
    latest_input: EngineCameraInput,
    latest_look_rate_mouse_delta_per_second: [f64; 2],
    movement_yaw_radians: f64,
    movement_pitch_radians: f64,
    use_movement_pitch: bool,
    heading_time_nanos: u128,
    pending_jump_edge: bool,
    previous_eye: Vec3d,
    current_eye: Vec3d,
    interpolation_alpha: f64,
    dropped_steps: u64,
}

impl PlayerMovementState {
    pub(crate) fn new(config: PlayerMovementCadenceConfig, initial: EngineCameraSnapshot) -> Self {
        debug_assert!(config.is_valid());
        Self {
            config,
            timeline_origin: None,
            timeline_elapsed_nanos: 0,
            command_epoch: 0,
            next_command_sequence: 0,
            next_observation_sequence: 0,
            pending_observations: VecDeque::new(),
            dropped_input_observations: 0,
            pending_commands: VecDeque::new(),
            recorded_commands: VecDeque::new(),
            dropped_recorded_commands: 0,
            latest_input: EngineCameraInput::default(),
            latest_look_rate_mouse_delta_per_second: [0.0; 2],
            movement_yaw_radians: initial.yaw_radians,
            movement_pitch_radians: initial.pitch_radians,
            use_movement_pitch: true,
            heading_time_nanos: 0,
            pending_jump_edge: false,
            previous_eye: initial.eye,
            current_eye: initial.eye,
            interpolation_alpha: 0.0,
            dropped_steps: 0,
        }
    }

    pub(crate) fn observe_input_at(
        &mut self,
        input: EngineCameraInput,
        look_rate_mouse_delta_per_second: [f64; 2],
        observed_at: MonotonicInstant,
    ) {
        self.observe_input_internal(input, look_rate_mouse_delta_per_second, observed_at, false);
    }

    pub(crate) fn observe_input_without_heading_at(
        &mut self,
        input: EngineCameraInput,
        look_rate_mouse_delta_per_second: [f64; 2],
        observed_at: MonotonicInstant,
    ) {
        self.observe_input_internal(input, look_rate_mouse_delta_per_second, observed_at, true);
    }

    fn observe_input_internal(
        &mut self,
        mut input: EngineCameraInput,
        look_rate_mouse_delta_per_second: [f64; 2],
        observed_at: MonotonicInstant,
        preserve_heading: bool,
    ) {
        let exact_movement_yaw_radians = input.movement_yaw_radians.filter(|yaw| yaw.is_finite());
        let exact_movement_pitch_radians = input
            .movement_pitch_radians
            .filter(|pitch| pitch.is_finite());
        let use_movement_pitch = (!preserve_heading).then_some(
            exact_movement_yaw_radians.is_none() || exact_movement_pitch_radians.is_some(),
        );
        input.dt_seconds = 0.0;
        input.mouse_delta_x = 0.0;
        input.mouse_delta_y = 0.0;
        input.movement_yaw_radians = None;
        input.movement_pitch_radians = None;
        let sequence = self.allocate_observation_sequence();
        self.enqueue_observation(TimedMovementObservation {
            observed_at,
            sequence,
            input: Some(input),
            look_rate_mouse_delta_per_second: Some([
                finite_look_rate(look_rate_mouse_delta_per_second[0]),
                finite_look_rate(look_rate_mouse_delta_per_second[1]),
            ]),
            exact_movement_yaw_radians: if preserve_heading {
                None
            } else {
                exact_movement_yaw_radians
            },
            exact_movement_pitch_radians: if preserve_heading {
                None
            } else {
                exact_movement_pitch_radians
            },
            use_movement_pitch,
        });
    }

    pub(crate) fn observe_heading_at(
        &mut self,
        yaw_radians: f64,
        pitch_radians: f64,
        observed_at: MonotonicInstant,
    ) {
        if !yaw_radians.is_finite() || !pitch_radians.is_finite() {
            return;
        }
        let sequence = self.allocate_observation_sequence();
        self.enqueue_observation(TimedMovementObservation {
            observed_at,
            sequence,
            input: None,
            look_rate_mouse_delta_per_second: None,
            exact_movement_yaw_radians: Some(yaw_radians),
            exact_movement_pitch_radians: Some(pitch_radians),
            use_movement_pitch: Some(true),
        });
    }

    pub(crate) const fn config(&self) -> PlayerMovementCadenceConfig {
        self.config
    }

    pub(crate) const fn total_dropped_steps(&self) -> u64 {
        self.dropped_steps
    }

    pub(crate) const fn dropped_recorded_commands(&self) -> u64 {
        self.dropped_recorded_commands
    }

    pub(crate) const fn dropped_input_observations(&self) -> u64 {
        self.dropped_input_observations
    }

    pub(crate) fn recorded_commands(&self) -> impl Iterator<Item = PlayerMovementCommand> + '_ {
        self.recorded_commands.iter().copied()
    }

    pub(crate) fn advance_elapsed(
        &mut self,
        frame_end: MonotonicInstant,
        elapsed_seconds: f64,
    ) -> PlayerMovementAdvance {
        let elapsed_nanos = finite_elapsed_nanos(elapsed_seconds);
        if self.timeline_origin.is_none() {
            let elapsed_nanos_u64 = u64::try_from(elapsed_nanos).unwrap_or(u64::MAX);
            self.timeline_origin = Some(MonotonicInstant::from_nanos(
                frame_end.as_nanos().saturating_sub(elapsed_nanos_u64),
            ));
        }
        self.timeline_elapsed_nanos = self.timeline_elapsed_nanos.saturating_add(elapsed_nanos);
        let total_due_steps = self
            .timeline_elapsed_nanos
            .saturating_mul(u128::from(self.config.rate_hz))
            / NANOS_PER_SECOND;
        let due_steps = total_due_steps.saturating_sub(u128::from(self.next_command_sequence));

        let admitted_steps = due_steps.min(u128::from(self.config.max_catch_up_steps)) as u32;
        let dropped_steps = due_steps
            .saturating_sub(u128::from(admitted_steps))
            .min(u128::from(u32::MAX)) as u32;
        self.dropped_steps = self.dropped_steps.saturating_add(u64::from(dropped_steps));
        let accumulator_rate_nanos = self
            .timeline_elapsed_nanos
            .saturating_mul(u128::from(self.config.rate_hz))
            % NANOS_PER_SECOND;
        self.interpolation_alpha = accumulator_rate_nanos as f64 / NANOS_PER_SECOND as f64;

        for _ in 0..admitted_steps {
            let boundary_nanos = self.next_command_boundary_nanos();
            self.apply_observations_through(boundary_nanos);
            self.advance_heading_to(boundary_nanos);
            let mut input = self.latest_input;
            input.dt_seconds = self.config.step_seconds();
            input.movement_yaw_radians = Some(self.movement_yaw_radians);
            input.movement_pitch_radians = self
                .use_movement_pitch
                .then_some(self.movement_pitch_radians);
            if self.pending_jump_edge {
                input.jump = true;
                self.pending_jump_edge = false;
            }
            let command = PlayerMovementCommand {
                epoch: self.command_epoch,
                sequence: self.next_command_sequence,
                input,
            };
            self.pending_commands.push_back(command);
            self.record_command(command);
            self.next_command_sequence = self.next_command_sequence.saturating_add(1);
        }
        if dropped_steps > 0 {
            self.next_command_sequence = self
                .next_command_sequence
                .saturating_add(u64::from(dropped_steps));
            let boundary_nanos = quantum_boundary_nanos(
                self.next_command_sequence.saturating_sub(1),
                self.config.rate_hz,
            );
            self.apply_observations_through(boundary_nanos);
            self.advance_heading_to(boundary_nanos);
            self.pending_jump_edge = false;
        }

        PlayerMovementAdvance {
            steps: admitted_steps,
            dropped_steps,
            interpolation_alpha: self.interpolation_alpha,
        }
    }

    pub(crate) fn next_step_command(&mut self) -> Option<PlayerMovementCommand> {
        self.pending_commands.pop_front()
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

    pub(crate) fn reset(&mut self, camera: EngineCameraSnapshot) {
        self.timeline_origin = None;
        self.timeline_elapsed_nanos = 0;
        self.command_epoch = self.command_epoch.saturating_add(1);
        self.next_command_sequence = 0;
        self.next_observation_sequence = 0;
        self.pending_observations.clear();
        self.dropped_input_observations = 0;
        self.pending_commands.clear();
        self.recorded_commands.clear();
        self.dropped_recorded_commands = 0;
        self.latest_input = EngineCameraInput::default();
        self.latest_look_rate_mouse_delta_per_second = [0.0; 2];
        self.movement_yaw_radians = camera.yaw_radians;
        self.movement_pitch_radians = camera.pitch_radians;
        self.use_movement_pitch = true;
        self.heading_time_nanos = 0;
        self.pending_jump_edge = false;
        self.previous_eye = camera.eye;
        self.current_eye = camera.eye;
        self.interpolation_alpha = 0.0;
    }

    fn allocate_observation_sequence(&mut self) -> u64 {
        let sequence = self.next_observation_sequence;
        self.next_observation_sequence = self.next_observation_sequence.saturating_add(1);
        sequence
    }

    fn enqueue_observation(&mut self, observation: TimedMovementObservation) {
        if self.pending_observations.len() == MAX_PENDING_PLAYER_INPUT_OBSERVATIONS {
            self.pending_observations.pop_front();
            self.dropped_input_observations = self.dropped_input_observations.saturating_add(1);
            self.pending_jump_edge = false;
        }
        let position = self
            .pending_observations
            .iter()
            .position(|pending| {
                (pending.observed_at, pending.sequence)
                    > (observation.observed_at, observation.sequence)
            })
            .unwrap_or(self.pending_observations.len());
        self.pending_observations.insert(position, observation);
    }

    fn next_command_boundary_nanos(&self) -> u128 {
        quantum_boundary_nanos(self.next_command_sequence, self.config.rate_hz)
    }

    fn apply_observations_through(&mut self, boundary_nanos: u128) {
        let origin = self
            .timeline_origin
            .expect("movement timeline origin exists before commands");
        while let Some(observation) = self.pending_observations.front().copied() {
            let observation_nanos = observation
                .observed_at
                .saturating_duration_since(origin)
                .as_nanos();
            if observation_nanos > boundary_nanos {
                break;
            }
            self.pending_observations.pop_front();
            self.advance_heading_to(observation_nanos);
            if let Some(yaw_radians) = observation.exact_movement_yaw_radians {
                self.movement_yaw_radians = yaw_radians;
            }
            if let Some(pitch_radians) = observation.exact_movement_pitch_radians {
                self.movement_pitch_radians = pitch_radians;
            }
            if let Some(input) = observation.input {
                self.pending_jump_edge |= input.jump && !self.latest_input.jump;
                self.latest_input = input;
            }
            if let Some(look_rate) = observation.look_rate_mouse_delta_per_second {
                self.latest_look_rate_mouse_delta_per_second = look_rate;
            }
            if let Some(use_movement_pitch) = observation.use_movement_pitch {
                self.use_movement_pitch = use_movement_pitch;
            }
        }
    }

    fn advance_heading_to(&mut self, boundary_nanos: u128) {
        let boundary_nanos = boundary_nanos.max(self.heading_time_nanos);
        let elapsed_seconds =
            (boundary_nanos - self.heading_time_nanos) as f64 / NANOS_PER_SECOND as f64;
        self.movement_yaw_radians -= self.latest_look_rate_mouse_delta_per_second[0]
            * elapsed_seconds
            * ENGINE_CAMERA_MOUSE_SENSITIVITY;
        self.movement_pitch_radians = (self.movement_pitch_radians
            - self.latest_look_rate_mouse_delta_per_second[1]
                * elapsed_seconds
                * ENGINE_CAMERA_MOUSE_SENSITIVITY)
            .clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
        self.heading_time_nanos = boundary_nanos;
    }

    fn record_command(&mut self, command: PlayerMovementCommand) {
        if self.recorded_commands.len() == MAX_RECORDED_PLAYER_MOVEMENT_COMMANDS {
            self.recorded_commands.pop_front();
            self.dropped_recorded_commands = self.dropped_recorded_commands.saturating_add(1);
        }
        self.recorded_commands.push_back(command);
    }
}

fn finite_elapsed_nanos(elapsed_seconds: f64) -> u128 {
    if !elapsed_seconds.is_finite() || elapsed_seconds <= 0.0 {
        return 0;
    }
    Duration::from_secs_f64(elapsed_seconds).as_nanos()
}

fn finite_look_rate(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

fn quantum_boundary_nanos(sequence: u64, rate_hz: u32) -> u128 {
    let numerator = u128::from(sequence)
        .saturating_add(1)
        .saturating_mul(NANOS_PER_SECOND);
    numerator.saturating_add(u128::from(rate_hz).saturating_sub(1)) / u128::from(rate_hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PlayerMovementState {
        PlayerMovementState::new(
            PlayerMovementCadenceConfig::default(),
            EngineCameraSnapshot::from_eye_pose(Vec3d::new(0.0, 64.0, 0.0), 0.0, 0.0, 32.0),
        )
    }

    fn at_millis(millis: u64) -> MonotonicInstant {
        MonotonicInstant::from_nanos(millis.saturating_mul(1_000_000))
    }

    fn advance(
        state: &mut PlayerMovementState,
        frame_end_millis: u64,
        elapsed_seconds: f64,
    ) -> PlayerMovementAdvance {
        state.advance_elapsed(at_millis(frame_end_millis), elapsed_seconds)
    }

    fn drain_commands(state: &mut PlayerMovementState) -> Vec<PlayerMovementCommand> {
        std::iter::from_fn(|| state.next_step_command()).collect()
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
        for frame in 1..=10 {
            let advance = advance(&mut state, frame * 100, 0.1);
            assert_eq!(advance.steps, 6);
            assert_eq!(advance.dropped_steps, 0);
            assert_eq!(drain_commands(&mut state).len(), 6);
            total += advance.steps;
        }
        assert_eq!(total, 60);
    }

    #[test]
    fn five_fps_presentation_reaches_the_default_catch_up_boundary() {
        let mut state = state();
        let advance = advance(&mut state, 200, 0.2);
        assert_eq!(advance.steps, 12);
        assert_eq!(advance.dropped_steps, 0);
    }

    #[test]
    fn timestamped_input_produces_the_same_commands_at_fifty_and_ten_fps() {
        fn command_timeline(presentation_rate_hz: u32) -> Vec<bool> {
            let mut state = state();
            state.observe_input_at(
                EngineCameraInput {
                    forward: true,
                    ..EngineCameraInput::default()
                },
                [0.0; 2],
                MonotonicInstant::ZERO,
            );
            state.observe_input_at(EngineCameraInput::default(), [0.0; 2], at_millis(501));
            let mut timeline = Vec::new();
            for frame in 1..=presentation_rate_hz {
                let frame_end_nanos =
                    u64::from(frame) * 1_000_000_000 / u64::from(presentation_rate_hz);
                state.advance_elapsed(
                    MonotonicInstant::from_nanos(frame_end_nanos),
                    1.0 / f64::from(presentation_rate_hz),
                );
                timeline.extend(drain_commands(&mut state).into_iter().map(|command| {
                    assert_eq!(
                        command.input.dt_seconds,
                        PlayerMovementCadenceConfig::default().step_seconds()
                    );
                    command.input.forward
                }));
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
        let result = advance(&mut state, 1_000, 1.0);
        assert_eq!(result.steps, 12);
        assert_eq!(result.dropped_steps, 48);
        assert_eq!(state.total_dropped_steps(), 48);
        assert_eq!(drain_commands(&mut state).last().unwrap().sequence, 11);
        assert_eq!(
            advance(&mut state, 1_017, 0.017).steps,
            1,
            "the command stream resumes after the explicit sequence gap"
        );
        assert_eq!(state.next_step_command().unwrap().sequence, 60);
    }

    #[test]
    fn observation_overflow_recovers_to_the_newest_held_state() {
        let mut state = state();
        for index in 0..=MAX_PENDING_PLAYER_INPUT_OBSERVATIONS {
            state.observe_input_at(
                EngineCameraInput {
                    forward: index == MAX_PENDING_PLAYER_INPUT_OBSERVATIONS,
                    ..EngineCameraInput::default()
                },
                [0.0; 2],
                MonotonicInstant::ZERO,
            );
        }

        assert_eq!(state.dropped_input_observations(), 1);
        advance(&mut state, 17, 0.017);
        assert!(state.next_step_command().unwrap().input.forward);
    }

    #[test]
    fn reset_starts_a_new_command_epoch() {
        let mut state = state();
        advance(&mut state, 17, 0.017);
        let first = state.next_step_command().unwrap();
        state.reset(EngineCameraSnapshot::from_eye_pose(
            Vec3d::new(1.0, 65.0, 2.0),
            0.5,
            -0.25,
            32.0,
        ));
        advance(&mut state, 34, 0.017);
        let after_reset = state.next_step_command().unwrap();

        assert_eq!((first.epoch, first.sequence), (0, 0));
        assert_eq!((after_reset.epoch, after_reset.sequence), (1, 0));
        assert_eq!(after_reset.input.movement_yaw_radians, Some(0.5));
        assert_eq!(after_reset.input.movement_pitch_radians, Some(-0.25));
    }

    #[test]
    fn jump_press_and_release_between_frames_reaches_the_next_command() {
        let mut state = state();
        state.observe_input_at(
            EngineCameraInput {
                jump: true,
                ..EngineCameraInput::default()
            },
            [0.0; 2],
            at_millis(5),
        );
        state.observe_input_at(EngineCameraInput::default(), [0.0; 2], at_millis(8));
        assert_eq!(advance(&mut state, 8, 0.008).steps, 0);
        assert_eq!(advance(&mut state, 17, 0.009).steps, 1);
        assert!(state.next_step_command().unwrap().input.jump);
        assert!(state.next_step_command().is_none());
    }

    #[test]
    fn six_commands_follow_axis_changes_inside_a_hundred_millisecond_frame() {
        let mut state = state();
        for (millis, forward) in [(0, 0.0), (20, 0.2), (40, 0.4), (60, 0.6), (80, 0.8)] {
            state.observe_input_at(
                EngineCameraInput {
                    movement_impulse: Some(EngineCameraMovementImpulse::new(0.0, forward)),
                    ..EngineCameraInput::default()
                },
                [0.0; 2],
                at_millis(millis),
            );
        }

        assert_eq!(advance(&mut state, 100, 0.1).steps, 6);
        let forward = drain_commands(&mut state)
            .into_iter()
            .map(|command| command.input.movement_impulse.unwrap().forward)
            .collect::<Vec<_>>();
        assert_eq!(forward, vec![0.0, 0.2, 0.4, 0.6, 0.8, 0.8]);
    }

    #[test]
    fn look_rate_is_integrated_per_command_not_backdated_to_the_frame() {
        let mut state = state();
        state.observe_input_at(
            EngineCameraInput {
                forward: true,
                ..EngineCameraInput::default()
            },
            [120.0, 0.0],
            MonotonicInstant::ZERO,
        );
        assert_eq!(advance(&mut state, 100, 0.1).steps, 6);
        let yaws = drain_commands(&mut state)
            .into_iter()
            .map(|command| command.input.movement_yaw_radians.unwrap())
            .collect::<Vec<_>>();
        assert!(yaws.windows(2).all(|pair| pair[0] > pair[1]));
        let expected = -120.0 * 0.1 * ENGINE_CAMERA_MOUSE_SENSITIVITY;
        assert!((yaws[5] - expected).abs() < 1.0e-9);
    }

    #[test]
    fn action_history_does_not_backdate_a_later_heading_sample() {
        let mut state = state();
        state.observe_input_without_heading_at(
            EngineCameraInput {
                forward: true,
                ..EngineCameraInput::default()
            },
            [0.0; 2],
            at_millis(20),
        );
        state.observe_input_at(
            EngineCameraInput {
                forward: true,
                movement_yaw_radians: Some(1.0),
                ..EngineCameraInput::default()
            },
            [0.0; 2],
            at_millis(100),
        );

        advance(&mut state, 100, 0.1);
        let commands = drain_commands(&mut state);

        assert_eq!(commands.len(), 6);
        assert!(
            commands[..5]
                .iter()
                .all(|command| command.input.movement_yaw_radians == Some(0.0))
        );
        assert_eq!(commands[5].input.movement_yaw_radians, Some(1.0));
    }

    #[test]
    fn recorded_commands_replay_identically_in_sequence_order() {
        let mut state = state();
        state.observe_input_at(
            EngineCameraInput {
                movement_impulse: Some(EngineCameraMovementImpulse::new(0.25, 0.75)),
                sprint: true,
                ..EngineCameraInput::default()
            },
            [30.0, 0.0],
            MonotonicInstant::ZERO,
        );
        for frame in 1..=12 {
            advance(&mut state, frame * 25, 0.025);
            drain_commands(&mut state);
        }
        let recording = state.recorded_commands().collect::<Vec<_>>();
        assert_eq!(recording.len(), 18);
        assert_eq!(
            recording
                .iter()
                .map(|command| command.sequence)
                .collect::<Vec<_>>(),
            (0..18).collect::<Vec<_>>()
        );
        assert!(
            recording
                .iter()
                .all(|command| command.input.movement_impulse
                    == Some(EngineCameraMovementImpulse::new(0.25, 0.75)))
        );
        assert_eq!(state.dropped_recorded_commands(), 0);

        let replay = || {
            let client = ClientRuntime::local_integrated();
            let mut camera =
                EngineCameraController::from_eye_pose(Vec3d::new(0.0, 64.0, 0.0), 0.0, 0.0, 32.0);
            camera.set_movement_mode(EngineCameraMovementMode::Fly);
            camera.set_collision_mode(EngineCameraCollisionMode::NoClip);
            for command in &recording {
                camera.apply_movement_input(&client, command.input);
            }
            camera.snapshot()
        };
        assert_eq!(replay(), replay());
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
