#![forbid(unsafe_code)]

//! Deterministic, sans-I/O world-view navigation.
//!
//! Platform adapters translate their event types into [`WorldViewIntent`] or
//! neutral [`ContactEvent`] values. This crate owns navigation math and
//! gesture classification, but it has no renderer, window, browser, world, or
//! input-device dependency.

use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};
use std::time::Duration;

pub const MIN_PITCH_RADIANS: f64 = 0.12;
pub const MAX_PITCH_RADIANS: f64 = 1.25;
pub const DEFAULT_MIN_BLOCKS_ACROSS: f64 = 1.0;
pub const DEFAULT_MAX_BLOCKS_ACROSS: f64 = 131_072.0;
pub const WORLD_VIEW_HELD_PAN_FOOTPRINTS_PER_SECOND: f64 = 0.4;
pub const WORLD_VIEW_MAX_HELD_STEP_SECONDS: f64 = 0.1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldViewMode {
    Map,
    #[default]
    Orbit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldViewProjection {
    Orthographic,
    #[default]
    Perspective,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldViewState {
    pub mode: WorldViewMode,
    pub focus_x: f64,
    pub focus_z: f64,
    pub blocks_across: f64,
    pub yaw_radians: f64,
    pub pitch_radians: f64,
    pub projection: WorldViewProjection,
}

impl Default for WorldViewState {
    fn default() -> Self {
        Self {
            mode: WorldViewMode::Orbit,
            focus_x: 0.0,
            focus_z: 0.0,
            blocks_across: 4_096.0,
            yaw_radians: std::f64::consts::FRAC_PI_4,
            pitch_radians: 0.52,
            projection: WorldViewProjection::Perspective,
        }
    }
}

impl WorldViewState {
    pub fn center_x_i32(self) -> i32 {
        rounded_i32(self.focus_x)
    }

    pub fn center_z_i32(self) -> i32 {
        rounded_i32(self.focus_z)
    }

    pub fn blocks_across_u32(self) -> u32 {
        self.blocks_across.round().clamp(1.0, f64::from(u32::MAX)) as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldViewHeldDirection {
    Forward,
    Backward,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorldViewHeldMotion {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    last_elapsed: Option<Duration>,
}

impl WorldViewHeldMotion {
    pub fn set_direction(&mut self, direction: WorldViewHeldDirection, pressed: bool) -> bool {
        let was_active = self.is_active();
        let held = match direction {
            WorldViewHeldDirection::Forward => &mut self.forward,
            WorldViewHeldDirection::Backward => &mut self.backward,
            WorldViewHeldDirection::Left => &mut self.left,
            WorldViewHeldDirection::Right => &mut self.right,
        };
        let changed = *held != pressed;
        *held = pressed;
        let is_active = self.is_active();
        if was_active != is_active {
            self.last_elapsed = None;
        }
        changed
    }

    pub const fn is_active(self) -> bool {
        self.forward || self.backward || self.left || self.right
    }

    pub fn clear(&mut self) -> bool {
        let changed = self.is_active();
        *self = Self::default();
        changed
    }

    pub fn advance(&mut self, state: WorldViewState, elapsed: Duration) -> Option<WorldViewIntent> {
        if !self.is_active() {
            self.last_elapsed = None;
            return None;
        }
        let Some(previous) = self.last_elapsed.replace(elapsed) else {
            return None;
        };
        let delta_seconds = elapsed
            .saturating_sub(previous)
            .as_secs_f64()
            .min(WORLD_VIEW_MAX_HELD_STEP_SECONDS);
        let horizontal = f64::from(self.right as u8) - f64::from(self.left as u8);
        let vertical = f64::from(self.backward as u8) - f64::from(self.forward as u8);
        if (horizontal == 0.0 && vertical == 0.0) || delta_seconds <= 0.0 {
            return None;
        }
        let length = horizontal.hypot(vertical).max(1.0);
        let distance = finite_positive_or_one(state.blocks_across)
            * WORLD_VIEW_HELD_PAN_FOOTPRINTS_PER_SECOND
            * delta_seconds;
        Some(WorldViewIntent::PanWorld {
            delta_x: horizontal / length * distance,
            delta_z: vertical / length * distance,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldViewConstraints {
    pub min_blocks_across: f64,
    pub max_blocks_across: f64,
    pub min_pitch_radians: f64,
    pub max_pitch_radians: f64,
}

impl Default for WorldViewConstraints {
    fn default() -> Self {
        Self {
            min_blocks_across: DEFAULT_MIN_BLOCKS_ACROSS,
            max_blocks_across: DEFAULT_MAX_BLOCKS_ACROSS,
            min_pitch_radians: MIN_PITCH_RADIANS,
            max_pitch_radians: MAX_PITCH_RADIANS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportMetrics {
    pub width_pixels: f64,
    pub height_pixels: f64,
}

impl ViewportMetrics {
    pub fn new(width_pixels: f64, height_pixels: f64) -> Self {
        Self {
            width_pixels: finite_positive_or_one(width_pixels),
            height_pixels: finite_positive_or_one(height_pixels),
        }
    }

    pub fn aspect(self) -> f64 {
        self.width_pixels / self.height_pixels
    }

    pub fn normalized_anchor(self, point: ViewPoint) -> ViewPoint {
        ViewPoint {
            x: point.x / self.width_pixels - 0.5,
            y: point.y / self.height_pixels - 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewPoint {
    pub x: f64,
    pub y: f64,
}

impl ViewPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldViewIntent {
    SetMode(WorldViewMode),
    SetProjection(WorldViewProjection),
    Orbit {
        delta_pixels: ViewPoint,
        viewport: ViewportMetrics,
    },
    GrabPan {
        delta_pixels: ViewPoint,
        viewport: ViewportMetrics,
    },
    AnchoredZoom {
        log_delta: f64,
        normalized_anchor: ViewPoint,
        viewport: ViewportMetrics,
    },
    PinchPanZoom {
        log_delta: f64,
        normalized_anchor: ViewPoint,
        centroid_delta_pixels: ViewPoint,
        viewport: ViewportMetrics,
    },
    PanWorld {
        delta_x: f64,
        delta_z: f64,
    },
    FocusAt {
        world_x: f64,
        world_z: f64,
    },
    Recenter {
        world_x: f64,
        world_z: f64,
        blocks_across: Option<f64>,
    },
    Tap {
        position: ViewPoint,
    },
    DoubleTap {
        position: ViewPoint,
    },
    CancelContacts,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldViewSignal {
    Tap { position: ViewPoint },
    DoubleTap { position: ViewPoint },
    ContactsCancelled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldViewReduction {
    pub state: WorldViewState,
    pub signal: Option<WorldViewSignal>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WorldViewReducer {
    constraints: WorldViewConstraints,
}

impl WorldViewReducer {
    pub const fn new(constraints: WorldViewConstraints) -> Self {
        Self { constraints }
    }

    pub const fn constraints(self) -> WorldViewConstraints {
        self.constraints
    }

    pub fn normalize(self, state: WorldViewState) -> WorldViewState {
        let min_blocks = finite_positive_or_one(self.constraints.min_blocks_across);
        let max_blocks = self
            .constraints
            .max_blocks_across
            .max(min_blocks)
            .min(f64::from(u32::MAX));
        let min_pitch = finite_or(self.constraints.min_pitch_radians, MIN_PITCH_RADIANS);
        let max_pitch =
            finite_or(self.constraints.max_pitch_radians, MAX_PITCH_RADIANS).max(min_pitch);
        WorldViewState {
            mode: state.mode,
            focus_x: finite_or(state.focus_x, 0.0).clamp(f64::from(i32::MIN), f64::from(i32::MAX)),
            focus_z: finite_or(state.focus_z, 0.0).clamp(f64::from(i32::MIN), f64::from(i32::MAX)),
            blocks_across: finite_or(state.blocks_across, min_blocks).clamp(min_blocks, max_blocks),
            yaw_radians: wrap_radians(finite_or(state.yaw_radians, 0.0)),
            pitch_radians: finite_or(state.pitch_radians, min_pitch).clamp(min_pitch, max_pitch),
            projection: state.projection,
        }
    }

    pub fn reduce(self, state: WorldViewState, intent: WorldViewIntent) -> WorldViewReduction {
        let mut state = self.normalize(state);
        let signal = match intent {
            WorldViewIntent::SetMode(mode) => {
                state.mode = mode;
                None
            }
            WorldViewIntent::SetProjection(projection) => {
                state.projection = projection;
                None
            }
            WorldViewIntent::Orbit {
                delta_pixels,
                viewport,
            } => {
                orbit(&mut state, delta_pixels, viewport);
                None
            }
            WorldViewIntent::GrabPan {
                delta_pixels,
                viewport,
            } => {
                grab_pan(&mut state, delta_pixels, viewport);
                None
            }
            WorldViewIntent::AnchoredZoom {
                log_delta,
                normalized_anchor,
                viewport,
            } => {
                anchored_zoom(
                    &mut state,
                    log_delta,
                    normalized_anchor,
                    viewport,
                    self.constraints,
                );
                None
            }
            WorldViewIntent::PinchPanZoom {
                log_delta,
                normalized_anchor,
                centroid_delta_pixels,
                viewport,
            } => {
                let is_map = state.mode == WorldViewMode::Map;
                anchored_zoom(
                    &mut state,
                    log_delta,
                    if is_map {
                        normalized_anchor
                    } else {
                        ViewPoint::default()
                    },
                    viewport,
                    self.constraints,
                );
                let pan_delta = if is_map {
                    centroid_delta_pixels
                } else {
                    ViewPoint::new(-centroid_delta_pixels.x, -centroid_delta_pixels.y)
                };
                grab_pan(&mut state, pan_delta, viewport);
                None
            }
            WorldViewIntent::PanWorld { delta_x, delta_z } => {
                state.focus_x += finite_or(delta_x, 0.0);
                state.focus_z += finite_or(delta_z, 0.0);
                None
            }
            WorldViewIntent::FocusAt { world_x, world_z } => {
                state.focus_x = world_x;
                state.focus_z = world_z;
                None
            }
            WorldViewIntent::Recenter {
                world_x,
                world_z,
                blocks_across,
            } => {
                state.focus_x = world_x;
                state.focus_z = world_z;
                if let Some(blocks_across) = blocks_across {
                    state.blocks_across = blocks_across;
                }
                None
            }
            WorldViewIntent::Tap { position } => Some(WorldViewSignal::Tap { position }),
            WorldViewIntent::DoubleTap { position } => {
                Some(WorldViewSignal::DoubleTap { position })
            }
            WorldViewIntent::CancelContacts => Some(WorldViewSignal::ContactsCancelled),
        };
        WorldViewReduction {
            state: self.normalize(state),
            signal,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ContactPurpose {
    #[default]
    ViewDefault,
    Pan,
    Orbit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContactEvent {
    Down {
        id: u64,
        position: ViewPoint,
        purpose: ContactPurpose,
        time_seconds: f64,
        viewport: ViewportMetrics,
    },
    Moved {
        id: u64,
        position: ViewPoint,
        time_seconds: f64,
        viewport: ViewportMetrics,
    },
    Up {
        id: u64,
        position: ViewPoint,
        time_seconds: f64,
        viewport: ViewportMetrics,
    },
    CancelAll,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactGestureConfig {
    pub drag_threshold_pixels: f64,
    pub double_tap_interval_seconds: f64,
    pub double_tap_distance_pixels: f64,
}

impl Default for ContactGestureConfig {
    fn default() -> Self {
        Self {
            drag_threshold_pixels: 6.0,
            double_tap_interval_seconds: 0.35,
            double_tap_distance_pixels: 24.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ActiveContact {
    start: ViewPoint,
    previous: ViewPoint,
    purpose: ContactPurpose,
    dragged: bool,
    participated_in_multitouch: bool,
}

#[derive(Clone, Copy, Debug)]
struct CompletedTap {
    position: ViewPoint,
    time_seconds: f64,
}

#[derive(Debug, Default)]
pub struct ContactGestureReducer {
    config: ContactGestureConfig,
    contacts: BTreeMap<u64, ActiveContact>,
    last_tap: Option<CompletedTap>,
}

impl ContactGestureReducer {
    pub fn new(config: ContactGestureConfig) -> Self {
        Self {
            config: normalized_gesture_config(config),
            contacts: BTreeMap::new(),
            last_tap: None,
        }
    }

    pub fn active_contact_count(&self) -> usize {
        self.contacts.len()
    }

    pub fn handle(&mut self, state: WorldViewState, event: ContactEvent) -> Vec<WorldViewIntent> {
        match event {
            ContactEvent::Down {
                id,
                position,
                purpose,
                ..
            } => {
                self.contacts.insert(
                    id,
                    ActiveContact {
                        start: position,
                        previous: position,
                        purpose,
                        dragged: false,
                        participated_in_multitouch: false,
                    },
                );
                if self.contacts.len() >= 2 {
                    for contact in self.contacts.values_mut() {
                        contact.participated_in_multitouch = true;
                    }
                }
                Vec::new()
            }
            ContactEvent::Moved {
                id,
                position,
                viewport,
                ..
            } => self.moved(state, id, position, viewport),
            ContactEvent::Up {
                id,
                position,
                time_seconds,
                ..
            } => self.ended(id, position, time_seconds),
            ContactEvent::CancelAll => {
                self.contacts.clear();
                self.last_tap = None;
                vec![WorldViewIntent::CancelContacts]
            }
        }
    }

    fn moved(
        &mut self,
        state: WorldViewState,
        id: u64,
        position: ViewPoint,
        viewport: ViewportMetrics,
    ) -> Vec<WorldViewIntent> {
        let before = first_pair(&self.contacts);
        let Some(contact) = self.contacts.get_mut(&id) else {
            return Vec::new();
        };
        let previous = contact.previous;
        if contact.start.distance(position) >= self.config.drag_threshold_pixels {
            contact.dragged = true;
        }
        contact.previous = position;

        if let Some(((first_before, second_before), (first_after, second_after))) =
            before.zip(first_pair(&self.contacts))
        {
            let previous_distance = first_before.distance(second_before);
            let distance = first_after.distance(second_after);
            if previous_distance <= f64::EPSILON || distance <= f64::EPSILON {
                return Vec::new();
            }
            let previous_centroid = midpoint(first_before, second_before);
            let centroid = midpoint(first_after, second_after);
            return vec![WorldViewIntent::PinchPanZoom {
                log_delta: (previous_distance / distance).ln(),
                normalized_anchor: viewport.normalized_anchor(previous_centroid),
                centroid_delta_pixels: ViewPoint::new(
                    centroid.x - previous_centroid.x,
                    centroid.y - previous_centroid.y,
                ),
                viewport,
            }];
        }

        let contact = self
            .contacts
            .get(&id)
            .expect("moved contact remains active");
        if contact.participated_in_multitouch {
            return Vec::new();
        }
        let delta = ViewPoint::new(position.x - previous.x, position.y - previous.y);
        let purpose = match contact.purpose {
            ContactPurpose::ViewDefault if state.mode == WorldViewMode::Map => ContactPurpose::Pan,
            ContactPurpose::ViewDefault => ContactPurpose::Orbit,
            purpose => purpose,
        };
        match purpose {
            ContactPurpose::Pan => vec![WorldViewIntent::GrabPan {
                delta_pixels: delta,
                viewport,
            }],
            ContactPurpose::Orbit => vec![WorldViewIntent::Orbit {
                delta_pixels: delta,
                viewport,
            }],
            ContactPurpose::ViewDefault => unreachable!("default contact purpose was resolved"),
        }
    }

    fn ended(&mut self, id: u64, position: ViewPoint, time_seconds: f64) -> Vec<WorldViewIntent> {
        let Some(contact) = self.contacts.remove(&id) else {
            return Vec::new();
        };
        for remaining in self.contacts.values_mut() {
            remaining.participated_in_multitouch = true;
        }
        if contact.dragged
            || contact.start.distance(position) >= self.config.drag_threshold_pixels
            || contact.participated_in_multitouch
        {
            return Vec::new();
        }
        let time_seconds = finite_or(time_seconds, 0.0);
        let is_double_tap = self.last_tap.is_some_and(|last| {
            time_seconds >= last.time_seconds
                && time_seconds - last.time_seconds <= self.config.double_tap_interval_seconds
                && position.distance(last.position) <= self.config.double_tap_distance_pixels
        });
        if is_double_tap {
            self.last_tap = None;
            vec![WorldViewIntent::DoubleTap { position }]
        } else {
            self.last_tap = Some(CompletedTap {
                position,
                time_seconds,
            });
            vec![WorldViewIntent::Tap { position }]
        }
    }
}

fn orbit(state: &mut WorldViewState, delta: ViewPoint, viewport: ViewportMetrics) {
    state.yaw_radians += finite_or(delta.x, 0.0) / viewport.width_pixels * PI * 1.5;
    state.pitch_radians += finite_or(delta.y, 0.0) / viewport.height_pixels * PI;
}

fn grab_pan(state: &mut WorldViewState, delta: ViewPoint, viewport: ViewportMetrics) {
    let vertical_blocks = state.blocks_across / viewport.aspect().max(0.01);
    match state.mode {
        WorldViewMode::Map => {
            state.focus_x -= delta.x / viewport.width_pixels * state.blocks_across;
            state.focus_z -= delta.y / viewport.height_pixels * vertical_blocks;
        }
        WorldViewMode::Orbit => {
            let horizontal = delta.x / viewport.width_pixels * state.blocks_across;
            let depth = delta.y / viewport.height_pixels * vertical_blocks;
            state.focus_x += state.yaw_radians.sin() * horizontal + state.yaw_radians.cos() * depth;
            state.focus_z += state.yaw_radians.cos() * horizontal - state.yaw_radians.sin() * depth;
        }
    }
}

fn anchored_zoom(
    state: &mut WorldViewState,
    log_delta: f64,
    anchor: ViewPoint,
    viewport: ViewportMetrics,
    constraints: WorldViewConstraints,
) {
    let old_blocks = state.blocks_across;
    let factor = finite_or(log_delta, 0.0).clamp(-20.0, 20.0).exp();
    let new_blocks = (old_blocks * factor).clamp(
        finite_positive_or_one(constraints.min_blocks_across),
        constraints
            .max_blocks_across
            .max(constraints.min_blocks_across),
    );
    let old_height = old_blocks / viewport.aspect().max(0.01);
    let new_height = new_blocks / viewport.aspect().max(0.01);
    state.focus_x += finite_or(anchor.x, 0.0) * (old_blocks - new_blocks);
    state.focus_z += finite_or(anchor.y, 0.0) * (old_height - new_height);
    state.blocks_across = new_blocks;
}

fn first_pair(contacts: &BTreeMap<u64, ActiveContact>) -> Option<(ViewPoint, ViewPoint)> {
    let mut contacts = contacts.values();
    Some((contacts.next()?.previous, contacts.next()?.previous))
}

fn midpoint(first: ViewPoint, second: ViewPoint) -> ViewPoint {
    ViewPoint::new((first.x + second.x) * 0.5, (first.y + second.y) * 0.5)
}

fn normalized_gesture_config(config: ContactGestureConfig) -> ContactGestureConfig {
    ContactGestureConfig {
        drag_threshold_pixels: finite_or(config.drag_threshold_pixels, 6.0).max(0.0),
        double_tap_interval_seconds: finite_or(config.double_tap_interval_seconds, 0.35).max(0.0),
        double_tap_distance_pixels: finite_or(config.double_tap_distance_pixels, 24.0).max(0.0),
    }
}

fn rounded_i32(value: f64) -> i32 {
    finite_or(value, 0.0)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn wrap_radians(value: f64) -> f64 {
    (value + PI).rem_euclid(TAU) - PI
}

fn finite_positive_or_one(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: ViewportMetrics = ViewportMetrics {
        width_pixels: 1_000.0,
        height_pixels: 500.0,
    };

    fn assert_near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-8,
            "expected {expected}, got {actual}"
        );
    }

    fn apply(
        reducer: WorldViewReducer,
        state: WorldViewState,
        intent: WorldViewIntent,
    ) -> WorldViewState {
        reducer.reduce(state, intent).state
    }

    fn held_motion_after_steps(steps: u32) -> WorldViewState {
        let reducer = WorldViewReducer::default();
        let mut state = WorldViewState::default();
        let mut motion = WorldViewHeldMotion::default();
        assert!(motion.set_direction(WorldViewHeldDirection::Right, true));
        assert_eq!(motion.advance(state, Duration::ZERO), None);
        for frame in 1..=steps {
            let elapsed = Duration::from_secs_f64(f64::from(frame) / f64::from(steps));
            if let Some(intent) = motion.advance(state, elapsed) {
                state = apply(reducer, state, intent);
            }
        }
        state
    }

    #[test]
    fn held_motion_is_partition_independent_at_60_and_120_hz() {
        let at_60_hz = held_motion_after_steps(60);
        let at_120_hz = held_motion_after_steps(120);
        let expected =
            WorldViewState::default().blocks_across * WORLD_VIEW_HELD_PAN_FOOTPRINTS_PER_SECOND;

        assert_near(at_60_hz.focus_x, expected);
        assert_near(at_120_hz.focus_x, expected);
        assert_near(at_60_hz.focus_x, at_120_hz.focus_x);
        assert_eq!(at_60_hz.focus_z, 0.0);
        assert_eq!(at_120_hz.focus_z, 0.0);
    }

    #[test]
    fn held_diagonal_motion_is_normalized() {
        let state = WorldViewState {
            blocks_across: 1_000.0,
            ..WorldViewState::default()
        };
        let mut motion = WorldViewHeldMotion::default();
        motion.set_direction(WorldViewHeldDirection::Right, true);
        motion.set_direction(WorldViewHeldDirection::Forward, true);
        assert_eq!(motion.advance(state, Duration::ZERO), None);
        let WorldViewIntent::PanWorld { delta_x, delta_z } =
            motion.advance(state, Duration::from_millis(100)).unwrap()
        else {
            panic!("held motion must produce a world pan");
        };

        assert_near(delta_x.hypot(delta_z), 40.0);
        assert!(delta_x > 0.0);
        assert!(delta_z < 0.0);
    }

    #[test]
    fn held_motion_does_not_consume_idle_time_and_caps_delayed_frames() {
        let state = WorldViewState {
            blocks_across: 1_000.0,
            ..WorldViewState::default()
        };
        let mut motion = WorldViewHeldMotion::default();
        motion.set_direction(WorldViewHeldDirection::Forward, true);
        assert_eq!(
            motion.advance(state, Duration::from_secs(90)),
            None,
            "the first active frame establishes a fresh clock"
        );
        let WorldViewIntent::PanWorld { delta_x, delta_z } =
            motion.advance(state, Duration::from_secs(95)).unwrap()
        else {
            panic!("held motion must produce a world pan");
        };

        assert_eq!(delta_x, 0.0);
        assert_near(delta_z, -40.0);
    }

    #[test]
    fn held_motion_release_and_cancel_clear_every_direction() {
        let state = WorldViewState::default();
        let mut motion = WorldViewHeldMotion::default();
        assert!(motion.set_direction(WorldViewHeldDirection::Left, true));
        assert!(motion.set_direction(WorldViewHeldDirection::Backward, true));
        assert!(motion.is_active());
        assert!(motion.clear());
        assert!(!motion.is_active());
        assert_eq!(
            motion.advance(state, Duration::from_secs(10)),
            None,
            "cancelled motion remains idle"
        );
        assert!(!motion.clear());
    }

    #[test]
    fn normalization_clamps_scale_pitch_coordinates_and_wraps_yaw() {
        let state = WorldViewReducer::default().normalize(WorldViewState {
            focus_x: f64::INFINITY,
            focus_z: f64::from(i32::MIN) - 500.0,
            blocks_across: 1_000_000.0,
            yaw_radians: PI * 5.0,
            pitch_radians: -50.0,
            ..WorldViewState::default()
        });

        assert_eq!(state.focus_x, 0.0);
        assert_eq!(state.focus_z, f64::from(i32::MIN));
        assert_eq!(state.blocks_across, DEFAULT_MAX_BLOCKS_ACROSS);
        assert_near(state.yaw_radians, -PI);
        assert_eq!(state.pitch_radians, MIN_PITCH_RADIANS);
    }

    #[test]
    fn map_grab_moves_world_opposite_the_pointer() {
        let state = apply(
            WorldViewReducer::default(),
            WorldViewState {
                mode: WorldViewMode::Map,
                blocks_across: 1_000.0,
                ..WorldViewState::default()
            },
            WorldViewIntent::GrabPan {
                delta_pixels: ViewPoint::new(100.0, 50.0),
                viewport: VIEWPORT,
            },
        );

        assert_near(state.focus_x, -100.0);
        assert_near(state.focus_z, -50.0);
    }

    #[test]
    fn orbit_grab_uses_camera_relative_horizontal_and_depth_axes() {
        let state = apply(
            WorldViewReducer::default(),
            WorldViewState {
                mode: WorldViewMode::Orbit,
                blocks_across: 1_000.0,
                yaw_radians: 0.0,
                ..WorldViewState::default()
            },
            WorldViewIntent::GrabPan {
                delta_pixels: ViewPoint::new(100.0, 50.0),
                viewport: VIEWPORT,
            },
        );

        assert_near(state.focus_x, 50.0);
        assert_near(state.focus_z, 100.0);
    }

    #[test]
    fn orbit_clamps_pitch_and_wraps_yaw() {
        let state = apply(
            WorldViewReducer::default(),
            WorldViewState {
                yaw_radians: PI - 0.1,
                pitch_radians: MAX_PITCH_RADIANS - 0.01,
                ..WorldViewState::default()
            },
            WorldViewIntent::Orbit {
                delta_pixels: ViewPoint::new(1_000.0, 500.0),
                viewport: VIEWPORT,
            },
        );

        assert!(state.yaw_radians >= -PI && state.yaw_radians < PI);
        assert_eq!(state.pitch_radians, MAX_PITCH_RADIANS);
    }

    #[test]
    fn map_zoom_preserves_world_position_beneath_anchor() {
        let initial = WorldViewState {
            mode: WorldViewMode::Map,
            focus_x: -200.0,
            focus_z: 80.0,
            blocks_across: 1_000.0,
            ..WorldViewState::default()
        };
        let anchor = ViewPoint::new(0.25, -0.2);
        let old_world_x = initial.focus_x + anchor.x * initial.blocks_across;
        let old_world_z = initial.focus_z + anchor.y * initial.blocks_across / VIEWPORT.aspect();
        let state = apply(
            WorldViewReducer::default(),
            initial,
            WorldViewIntent::AnchoredZoom {
                log_delta: 0.5_f64.ln(),
                normalized_anchor: anchor,
                viewport: VIEWPORT,
            },
        );

        assert_eq!(state.blocks_across, 500.0);
        assert_near(state.focus_x + anchor.x * state.blocks_across, old_world_x);
        assert_near(
            state.focus_z + anchor.y * state.blocks_across / VIEWPORT.aspect(),
            old_world_z,
        );
    }

    #[test]
    fn pinch_combines_anchored_zoom_and_centroid_pan() {
        let state = apply(
            WorldViewReducer::default(),
            WorldViewState {
                mode: WorldViewMode::Map,
                blocks_across: 1_000.0,
                ..WorldViewState::default()
            },
            WorldViewIntent::PinchPanZoom {
                log_delta: 0.5_f64.ln(),
                normalized_anchor: ViewPoint::new(0.25, 0.0),
                centroid_delta_pixels: ViewPoint::new(100.0, 0.0),
                viewport: VIEWPORT,
            },
        );

        assert_eq!(state.blocks_across, 500.0);
        assert_near(state.focus_x, 75.0);
    }

    #[test]
    fn focus_recenter_and_signals_are_explicit() {
        let reducer = WorldViewReducer::default();
        let focused = reducer.reduce(
            WorldViewState::default(),
            WorldViewIntent::FocusAt {
                world_x: -12_000.0,
                world_z: 20_000.0,
            },
        );
        assert_eq!(focused.state.center_x_i32(), -12_000);
        assert_eq!(focused.state.center_z_i32(), 20_000);

        let recentered = reducer.reduce(
            focused.state,
            WorldViewIntent::Recenter {
                world_x: 10.0,
                world_z: 20.0,
                blocks_across: Some(64.0),
            },
        );
        assert_eq!(recentered.state.blocks_across_u32(), 64);
        assert_eq!(
            reducer
                .reduce(
                    recentered.state,
                    WorldViewIntent::DoubleTap {
                        position: ViewPoint::new(5.0, 7.0)
                    }
                )
                .signal,
            Some(WorldViewSignal::DoubleTap {
                position: ViewPoint::new(5.0, 7.0)
            })
        );
    }

    #[test]
    fn one_contact_maps_to_mode_default_gesture() {
        let mut gestures = ContactGestureReducer::default();
        gestures.handle(
            WorldViewState::default(),
            down(1, ViewPoint::new(10.0, 10.0), 0.0),
        );
        assert!(matches!(
            gestures
                .handle(
                    WorldViewState::default(),
                    moved(1, ViewPoint::new(20.0, 15.0), 0.1)
                )
                .as_slice(),
            [WorldViewIntent::Orbit { .. }]
        ));

        let mut gestures = ContactGestureReducer::default();
        let map = WorldViewState {
            mode: WorldViewMode::Map,
            ..WorldViewState::default()
        };
        gestures.handle(map, down(1, ViewPoint::new(10.0, 10.0), 0.0));
        assert!(matches!(
            gestures
                .handle(map, moved(1, ViewPoint::new(20.0, 15.0), 0.1))
                .as_slice(),
            [WorldViewIntent::GrabPan { .. }]
        ));
    }

    #[test]
    fn two_contacts_emit_one_simultaneous_pinch_intent() {
        let mut gestures = ContactGestureReducer::default();
        let state = WorldViewState::default();
        gestures.handle(state, down(1, ViewPoint::new(100.0, 100.0), 0.0));
        gestures.handle(state, down(2, ViewPoint::new(200.0, 100.0), 0.0));
        let intents = gestures.handle(state, moved(2, ViewPoint::new(220.0, 110.0), 0.1));

        let [
            WorldViewIntent::PinchPanZoom {
                log_delta,
                centroid_delta_pixels,
                ..
            },
        ] = intents.as_slice()
        else {
            panic!("expected one pinch intent, got {intents:?}");
        };
        assert!(*log_delta < 0.0);
        assert_near(centroid_delta_pixels.x, 10.0);
        assert_near(centroid_delta_pixels.y, 5.0);
    }

    #[test]
    fn tap_drag_double_tap_and_cancellation_are_classified() {
        let mut gestures = ContactGestureReducer::default();
        let state = WorldViewState::default();
        gestures.handle(state, down(1, ViewPoint::new(10.0, 10.0), 0.0));
        assert!(matches!(
            gestures
                .handle(state, up(1, ViewPoint::new(12.0, 10.0), 0.1))
                .as_slice(),
            [WorldViewIntent::Tap { .. }]
        ));
        gestures.handle(state, down(2, ViewPoint::new(11.0, 11.0), 0.2));
        assert!(matches!(
            gestures
                .handle(state, up(2, ViewPoint::new(11.0, 11.0), 0.3))
                .as_slice(),
            [WorldViewIntent::DoubleTap { .. }]
        ));

        gestures.handle(state, down(3, ViewPoint::new(10.0, 10.0), 1.0));
        gestures.handle(state, moved(3, ViewPoint::new(30.0, 10.0), 1.1));
        assert!(
            gestures
                .handle(state, up(3, ViewPoint::new(30.0, 10.0), 1.2))
                .is_empty()
        );
        gestures.handle(state, down(4, ViewPoint::new(10.0, 10.0), 2.0));
        assert!(
            gestures
                .handle(state, up(4, ViewPoint::new(30.0, 10.0), 2.1))
                .is_empty()
        );
        assert_eq!(
            gestures.handle(state, ContactEvent::CancelAll),
            vec![WorldViewIntent::CancelContacts]
        );
        assert_eq!(gestures.active_contact_count(), 0);
    }

    #[test]
    fn pointer_count_transition_does_not_jump_or_tap() {
        let mut gestures = ContactGestureReducer::default();
        let state = WorldViewState::default();
        gestures.handle(state, down(1, ViewPoint::new(100.0, 100.0), 0.0));
        gestures.handle(state, down(2, ViewPoint::new(200.0, 100.0), 0.0));
        gestures.handle(state, moved(2, ViewPoint::new(220.0, 100.0), 0.1));
        assert!(
            gestures
                .handle(state, up(2, ViewPoint::new(220.0, 100.0), 0.2))
                .is_empty()
        );
        assert!(
            gestures
                .handle(state, moved(1, ViewPoint::new(102.0, 100.0), 0.3))
                .is_empty()
        );
        assert!(
            gestures
                .handle(state, up(1, ViewPoint::new(102.0, 100.0), 0.4))
                .is_empty()
        );
    }

    fn down(id: u64, position: ViewPoint, time_seconds: f64) -> ContactEvent {
        ContactEvent::Down {
            id,
            position,
            purpose: ContactPurpose::ViewDefault,
            time_seconds,
            viewport: VIEWPORT,
        }
    }

    fn moved(id: u64, position: ViewPoint, time_seconds: f64) -> ContactEvent {
        ContactEvent::Moved {
            id,
            position,
            time_seconds,
            viewport: VIEWPORT,
        }
    }

    fn up(id: u64, position: ViewPoint, time_seconds: f64) -> ContactEvent {
        ContactEvent::Up {
            id,
            position,
            time_seconds,
            viewport: VIEWPORT,
        }
    }
}
