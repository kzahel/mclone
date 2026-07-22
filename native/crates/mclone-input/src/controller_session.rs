use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{
    ControllerLayoutFamily, FlatInputAction, FlatInputFrame, GamepadBindings, GamepadControl,
    InputBindingAction, InputSourceDescriptor, InputSourceId, KeyboardTurnDirection, LookDelta,
    MovementDirection, MovementImpulse, StandardGamepadButtonState, StandardGamepadSnapshot,
};

/// Shared binding context selected by scene/UI state rather than a platform
/// collector.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InputContext {
    #[default]
    Gameplay,
    Menu,
    TextEntry,
}

/// Device-neutral actions emitted by keyboard, touch, ordinary gamepads,
/// tracked controllers, and future action-based backends such as Steam Input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlayerAction {
    Jump,
    Sprint,
    Sneak,
    Descend,
    Attack,
    Use,
    OpenMenu,
    OpenBlockPalette,
    OpenHelp,
    ToggleCameraView,
    SelectHotbarSlot(u8),
    NextHotbarSlot,
    PreviousHotbarSlot,
    UiNavigateUp,
    UiNavigateDown,
    UiNavigateLeft,
    UiNavigateRight,
    UiConfirm,
    UiBack,
    UiNextPage,
    UiPreviousPage,
}

/// One presentation-boundary semantic action sample.
///
/// `look_rate` is expressed in the same virtual mouse-delta units used by the
/// existing flat camera, but per second. `pointer_delta` is an already-integrated
/// mouse/touch delta and must never be multiplied by frame time again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerActionFrame {
    pub movement: MovementImpulse,
    pub look_rate: LookDelta,
    pub pointer_delta: LookDelta,
    pub held: BTreeSet<PlayerAction>,
    pub pressed: BTreeSet<PlayerAction>,
    pub released: BTreeSet<PlayerAction>,
    /// Source with meaningful post-dead-zone activity in this sample only.
    pub activity_source: Option<InputSourceId>,
    pub active_source: Option<InputSourceId>,
    pub active_controller_layout: Option<ControllerLayoutFamily>,
}

impl PlayerActionFrame {
    pub fn is_idle(&self) -> bool {
        self.movement == MovementImpulse::default()
            && self.look_rate == LookDelta::default()
            && self.pointer_delta == LookDelta::default()
            && self.held.is_empty()
            && self.pressed.is_empty()
            && self.released.is_empty()
    }

    /// Compatibility projection used while scene/application call sites move
    /// from `FlatInputFrame` to semantic action state.
    pub fn to_flat_frame(&self, dt_seconds: f64) -> FlatInputFrame {
        let dt_seconds = normalized_dt(dt_seconds);
        let mut frame = FlatInputFrame::default();
        if self.movement != MovementImpulse::default() {
            frame.set_analog_movement_impulse(self.movement.left, self.movement.forward);
        }
        frame.add_look_delta(
            self.pointer_delta.x + self.look_rate.x * dt_seconds,
            self.pointer_delta.y + self.look_rate.y * dt_seconds,
        );
        for action in &self.held {
            match action {
                PlayerAction::Jump => frame.press_action(FlatInputAction::Jump),
                PlayerAction::Sprint => frame.press_action(FlatInputAction::Sprint),
                PlayerAction::Sneak => frame.press_action(FlatInputAction::Sneak),
                PlayerAction::Descend => frame.press_action(FlatInputAction::Descend),
                _ => {}
            }
        }
        for action in &self.pressed {
            match *action {
                PlayerAction::Attack => frame.press_action(FlatInputAction::Attack),
                PlayerAction::Use => frame.press_action(FlatInputAction::Use),
                PlayerAction::OpenMenu => frame.press_action(FlatInputAction::OpenMenu),
                PlayerAction::OpenBlockPalette => {
                    frame.press_action(FlatInputAction::OpenBlockPalette);
                }
                PlayerAction::OpenHelp => frame.press_action(FlatInputAction::OpenHelp),
                PlayerAction::ToggleCameraView => {
                    frame.press_action(FlatInputAction::ToggleCameraView);
                }
                PlayerAction::SelectHotbarSlot(slot) => {
                    frame.select_hotbar_slot(slot);
                }
                PlayerAction::NextHotbarSlot => frame.step_hotbar(1),
                PlayerAction::PreviousHotbarSlot => frame.step_hotbar(-1),
                _ => {}
            }
        }
        frame
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerSessionSettings {
    pub movement_deadzone: f32,
    pub look_deadzone: f32,
    pub movement_response_exponent: f32,
    pub look_response_exponent: f32,
    pub look_rate_per_second: f32,
    pub trigger_press_threshold: f32,
    pub trigger_release_threshold: f32,
    pub navigation_press_threshold: f32,
    pub navigation_release_threshold: f32,
    pub navigation_initial_repeat_ms: u64,
    pub navigation_repeat_ms: u64,
}

impl ControllerSessionSettings {
    pub const DEFAULT_MOVEMENT_DEADZONE: f32 = 0.2;
    pub const DEFAULT_LOOK_DEADZONE: f32 = 0.18;
    pub const DEFAULT_LOOK_RATE_PER_SECOND: f32 = 480.0;
    pub const DEFAULT_TRIGGER_PRESS_THRESHOLD: f32 = 0.55;
    pub const DEFAULT_TRIGGER_RELEASE_THRESHOLD: f32 = 0.45;
    pub const DEFAULT_NAVIGATION_PRESS_THRESHOLD: f32 = 0.6;
    pub const DEFAULT_NAVIGATION_RELEASE_THRESHOLD: f32 = 0.4;
    pub const DEFAULT_NAVIGATION_INITIAL_REPEAT_MS: u64 = 350;
    pub const DEFAULT_NAVIGATION_REPEAT_MS: u64 = 90;

    fn normalized(self) -> Self {
        let trigger_press_threshold = unit_interval(self.trigger_press_threshold, 0.55);
        let trigger_release_threshold =
            unit_interval(self.trigger_release_threshold, 0.45).min(trigger_press_threshold);
        let navigation_press_threshold = unit_interval(self.navigation_press_threshold, 0.6);
        let navigation_release_threshold =
            unit_interval(self.navigation_release_threshold, 0.4).min(navigation_press_threshold);
        Self {
            movement_deadzone: deadzone(self.movement_deadzone),
            look_deadzone: deadzone(self.look_deadzone),
            movement_response_exponent: positive_finite(self.movement_response_exponent, 1.0),
            look_response_exponent: positive_finite(self.look_response_exponent, 1.0),
            look_rate_per_second: positive_finite(
                self.look_rate_per_second,
                Self::DEFAULT_LOOK_RATE_PER_SECOND,
            ),
            trigger_press_threshold,
            trigger_release_threshold,
            navigation_press_threshold,
            navigation_release_threshold,
            navigation_initial_repeat_ms: self.navigation_initial_repeat_ms.max(1),
            navigation_repeat_ms: self.navigation_repeat_ms.max(1),
        }
    }
}

impl Default for ControllerSessionSettings {
    fn default() -> Self {
        Self {
            movement_deadzone: Self::DEFAULT_MOVEMENT_DEADZONE,
            look_deadzone: Self::DEFAULT_LOOK_DEADZONE,
            movement_response_exponent: 1.0,
            look_response_exponent: 1.0,
            look_rate_per_second: Self::DEFAULT_LOOK_RATE_PER_SECOND,
            trigger_press_threshold: Self::DEFAULT_TRIGGER_PRESS_THRESHOLD,
            trigger_release_threshold: Self::DEFAULT_TRIGGER_RELEASE_THRESHOLD,
            navigation_press_threshold: Self::DEFAULT_NAVIGATION_PRESS_THRESHOLD,
            navigation_release_threshold: Self::DEFAULT_NAVIGATION_RELEASE_THRESHOLD,
            navigation_initial_repeat_ms: Self::DEFAULT_NAVIGATION_INITIAL_REPEAT_MS,
            navigation_repeat_ms: Self::DEFAULT_NAVIGATION_REPEAT_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerInputError {
    UnknownSource(InputSourceId),
    DuplicateSample(InputSourceId),
}

impl fmt::Display for ControllerInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSource(source_id) => {
                write!(formatter, "unknown controller input source {source_id:?}")
            }
            Self::DuplicateSample(source_id) => {
                write!(formatter, "duplicate controller sample for {source_id:?}")
            }
        }
    }
}

impl Error for ControllerInputError {}

#[derive(Clone, Debug)]
struct ControllerSourceState {
    descriptor: InputSourceDescriptor,
    snapshot: StandardGamepadSnapshot,
    held_actions: BTreeSet<PlayerAction>,
    navigation_held: BTreeSet<PlayerAction>,
    repeat_deadlines: BTreeMap<PlayerAction, Duration>,
    left_trigger_pressed: bool,
    right_trigger_pressed: bool,
    suppress_until_neutral: bool,
}

impl ControllerSourceState {
    fn new(mut descriptor: InputSourceDescriptor) -> Self {
        descriptor.connected = true;
        Self {
            descriptor,
            snapshot: StandardGamepadSnapshot::default(),
            held_actions: BTreeSet::new(),
            navigation_held: BTreeSet::new(),
            repeat_deadlines: BTreeMap::new(),
            left_trigger_pressed: false,
            right_trigger_pressed: false,
            suppress_until_neutral: false,
        }
    }

    fn clear_held(&mut self) -> BTreeSet<PlayerAction> {
        let released = std::mem::take(&mut self.held_actions);
        self.snapshot = StandardGamepadSnapshot::default();
        self.navigation_held.clear();
        self.repeat_deadlines.clear();
        self.left_trigger_pressed = false;
        self.right_trigger_pressed = false;
        self.suppress_until_neutral = true;
        released
    }
}

#[derive(Clone, Debug, Default)]
struct ResolvedSource {
    movement: MovementImpulse,
    look_rate: LookDelta,
    held: BTreeSet<PlayerAction>,
    repeat_pressed: BTreeSet<PlayerAction>,
}

/// Shared, deterministic ordinary-controller reducer.
#[derive(Clone, Debug)]
pub struct ControllerInputSession {
    context: InputContext,
    settings: ControllerSessionSettings,
    bindings: GamepadBindings,
    sources: BTreeMap<InputSourceId, ControllerSourceState>,
    active_source: Option<InputSourceId>,
    pending_released: BTreeSet<PlayerAction>,
}

impl Default for ControllerInputSession {
    fn default() -> Self {
        Self {
            context: InputContext::Gameplay,
            settings: ControllerSessionSettings::default(),
            bindings: GamepadBindings::default(),
            sources: BTreeMap::new(),
            active_source: None,
            pending_released: BTreeSet::new(),
        }
    }
}

impl ControllerInputSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_settings(settings: ControllerSessionSettings) -> Self {
        Self {
            settings: settings.normalized(),
            ..Self::default()
        }
    }

    pub fn with_bindings(bindings: GamepadBindings) -> Self {
        Self {
            bindings,
            ..Self::default()
        }
    }

    pub const fn context(&self) -> InputContext {
        self.context
    }

    pub const fn active_source(&self) -> Option<InputSourceId> {
        self.active_source
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn source_descriptor(&self, source_id: InputSourceId) -> Option<&InputSourceDescriptor> {
        self.sources.get(&source_id).map(|state| &state.descriptor)
    }

    pub fn connect_source(
        &mut self,
        source_id: InputSourceId,
        descriptor: InputSourceDescriptor,
    ) -> bool {
        match self.sources.get_mut(&source_id) {
            Some(state) => {
                let was_connected = state.descriptor.connected;
                state.descriptor = descriptor;
                state.descriptor.connected = true;
                !was_connected
            }
            None => {
                self.sources
                    .insert(source_id, ControllerSourceState::new(descriptor));
                true
            }
        }
    }

    pub fn disconnect_source(
        &mut self,
        source_id: InputSourceId,
    ) -> Result<bool, ControllerInputError> {
        let Some(mut state) = self.sources.remove(&source_id) else {
            return Err(ControllerInputError::UnknownSource(source_id));
        };
        let had_held_state = state.snapshot.has_held_state();
        let released = state.clear_held();
        let changed = !released.is_empty() || had_held_state;
        self.pending_released.extend(released);
        if self.active_source == Some(source_id) {
            self.active_source = None;
        }
        Ok(changed)
    }

    pub fn clear_held(&mut self) {
        for state in self.sources.values_mut() {
            self.pending_released.extend(state.clear_held());
        }
        self.active_source = None;
    }

    /// Changes semantic context without replaying controls that were already
    /// held at the transition boundary.
    pub fn set_context(&mut self, context: InputContext) {
        if self.context == context {
            return;
        }
        for state in self.sources.values_mut() {
            self.pending_released
                .extend(std::mem::take(&mut state.held_actions));
            state.navigation_held.clear();
            state.repeat_deadlines.clear();
        }
        self.context = context;
        let settings = self.settings.normalized();
        for state in self.sources.values_mut() {
            let resolved = if state.suppress_until_neutral {
                ResolvedSource::default()
            } else {
                resolve_source(
                    state,
                    self.context,
                    &self.bindings,
                    settings,
                    Duration::ZERO,
                    false,
                )
            };
            state.held_actions = resolved.held;
            state.repeat_deadlines.clear();
        }
    }

    pub fn sample_frame(
        &mut self,
        now: Duration,
        samples: impl IntoIterator<Item = (InputSourceId, StandardGamepadSnapshot)>,
    ) -> Result<PlayerActionFrame, ControllerInputError> {
        let mut samples = samples
            .into_iter()
            .map(|(source_id, snapshot)| (source_id, snapshot.normalized()))
            .collect::<Vec<_>>();
        samples.sort_by_key(|(source_id, _)| *source_id);
        for pair in samples.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(ControllerInputError::DuplicateSample(pair[0].0));
            }
        }
        for (source_id, _) in &samples {
            if !self.sources.contains_key(source_id) {
                return Err(ControllerInputError::UnknownSource(*source_id));
            }
        }
        let settings = self.settings.normalized();
        let mut changed_samples = BTreeSet::new();
        for (source_id, snapshot) in samples {
            let state = self
                .sources
                .get_mut(&source_id)
                .expect("sample sources were validated");
            if state.snapshot != snapshot {
                changed_samples.insert(source_id);
            }
            state.snapshot = snapshot;
            if state.suppress_until_neutral && snapshot_is_neutral(snapshot, settings) {
                state.suppress_until_neutral = false;
            }
        }

        let mut resolved_sources = BTreeMap::new();
        let mut activity_candidates = BTreeSet::new();
        let mut pressed = BTreeSet::new();
        let mut released = std::mem::take(&mut self.pending_released);
        let mut held = BTreeSet::new();

        for (source_id, state) in &mut self.sources {
            let resolved = if state.suppress_until_neutral {
                ResolvedSource::default()
            } else {
                resolve_source(state, self.context, &self.bindings, settings, now, true)
            };
            let source_pressed = resolved
                .held
                .difference(&state.held_actions)
                .copied()
                .collect::<BTreeSet<_>>();
            let source_released = state
                .held_actions
                .difference(&resolved.held)
                .copied()
                .collect::<BTreeSet<_>>();
            if (changed_samples.contains(source_id)
                && (resolved.movement != MovementImpulse::default()
                    || resolved.look_rate != LookDelta::default()
                    || !resolved.held.is_empty()))
                || !source_pressed.is_empty()
                || !source_released.is_empty()
                || !resolved.repeat_pressed.is_empty()
            {
                activity_candidates.insert(*source_id);
            }
            pressed.extend(source_pressed);
            pressed.extend(resolved.repeat_pressed.iter().copied());
            released.extend(source_released);
            held.extend(resolved.held.iter().copied());
            state.held_actions = resolved.held.clone();
            resolved_sources.insert(*source_id, resolved);
        }

        let activity_source = activity_candidates.first().copied();
        if activity_source.is_some() {
            self.active_source = activity_source;
        } else if !self.active_source.is_some_and(|source_id| {
            resolved_sources
                .get(&source_id)
                .is_some_and(resolved_source_is_meaningful)
        }) && let Some(source_id) =
            resolved_sources.iter().find_map(|(source_id, resolved)| {
                resolved_source_is_meaningful(resolved).then_some(*source_id)
            })
        {
            self.active_source = Some(source_id);
        }

        let mut frame = PlayerActionFrame {
            held,
            pressed,
            released,
            activity_source,
            active_source: self.active_source,
            active_controller_layout: self.active_source.and_then(|source_id| {
                self.sources
                    .get(&source_id)
                    .map(|state| state.descriptor.controller_layout)
            }),
            ..PlayerActionFrame::default()
        };
        if let Some(resolved) = self
            .active_source
            .and_then(|source_id| resolved_sources.get(&source_id))
        {
            frame.movement = resolved.movement;
            frame.look_rate = resolved.look_rate;
        }
        Ok(frame)
    }
}

fn resolved_source_is_meaningful(resolved: &ResolvedSource) -> bool {
    resolved.movement != MovementImpulse::default()
        || resolved.look_rate != LookDelta::default()
        || !resolved.held.is_empty()
}

fn snapshot_is_neutral(
    snapshot: StandardGamepadSnapshot,
    settings: ControllerSessionSettings,
) -> bool {
    adjusted_stick(
        snapshot.left_stick,
        settings.movement_deadzone,
        settings.movement_response_exponent,
    ) == glam::Vec2::ZERO
        && adjusted_stick(
            snapshot.right_stick,
            settings.look_deadzone,
            settings.look_response_exponent,
        ) == glam::Vec2::ZERO
        && !snapshot
            .buttons
            .mapped_controls()
            .into_iter()
            .any(|(_, pressed)| pressed)
}

fn resolve_source(
    state: &mut ControllerSourceState,
    context: InputContext,
    bindings: &GamepadBindings,
    settings: ControllerSessionSettings,
    now: Duration,
    emit_repeat: bool,
) -> ResolvedSource {
    let movement_stick = adjusted_stick(
        state.snapshot.left_stick,
        settings.movement_deadzone,
        settings.movement_response_exponent,
    );
    let look_stick = adjusted_stick(
        state.snapshot.right_stick,
        settings.look_deadzone,
        settings.look_response_exponent,
    );
    state.left_trigger_pressed = hysteretic_trigger(
        state.left_trigger_pressed,
        state.snapshot.buttons.left_trigger,
        settings,
    );
    state.right_trigger_pressed = hysteretic_trigger(
        state.right_trigger_pressed,
        state.snapshot.buttons.right_trigger,
        settings,
    );

    match context {
        InputContext::Gameplay => {
            let mut held = BTreeSet::new();
            let mut movement = MovementImpulse::default();
            let mut look_rate = LookDelta::default();
            for binding in &bindings.bindings {
                match (binding.control, binding.action) {
                    (GamepadControl::LeftStick, InputBindingAction::MoveAnalog) => {
                        movement.left = -movement_stick.x;
                        movement.forward = movement_stick.y;
                    }
                    (GamepadControl::RightStick, InputBindingAction::Look) => {
                        look_rate.x = look_stick.x * settings.look_rate_per_second;
                        look_rate.y = -look_stick.y * settings.look_rate_per_second;
                    }
                    (control, action) if control_pressed(state, control) => {
                        apply_gameplay_binding(action, &mut movement, &mut look_rate, &mut held);
                    }
                    _ => {}
                }
            }
            ResolvedSource {
                movement,
                look_rate,
                held,
                repeat_pressed: BTreeSet::new(),
            }
        }
        InputContext::Menu | InputContext::TextEntry => {
            let held = resolve_navigation(state, movement_stick, settings);
            let mut repeat_pressed = BTreeSet::new();
            for action in &held {
                let was_held = state.held_actions.contains(action);
                if !is_navigation_action(*action) {
                    continue;
                }
                let deadline = state.repeat_deadlines.entry(*action).or_insert_with(|| {
                    now.saturating_add(Duration::from_millis(settings.navigation_initial_repeat_ms))
                });
                if emit_repeat && was_held && now >= *deadline {
                    repeat_pressed.insert(*action);
                    *deadline =
                        now.saturating_add(Duration::from_millis(settings.navigation_repeat_ms));
                }
            }
            state
                .repeat_deadlines
                .retain(|action, _| held.contains(action));
            ResolvedSource {
                movement: MovementImpulse::default(),
                look_rate: LookDelta::default(),
                held,
                repeat_pressed,
            }
        }
    }
}

fn apply_gameplay_binding(
    action: InputBindingAction,
    movement: &mut MovementImpulse,
    look_rate: &mut LookDelta,
    held: &mut BTreeSet<PlayerAction>,
) {
    let semantic = match action {
        InputBindingAction::Move(direction) => {
            match direction {
                MovementDirection::Forward => movement.forward += 1.0,
                MovementDirection::Backward => movement.forward -= 1.0,
                MovementDirection::Left => movement.left += 1.0,
                MovementDirection::Right => movement.left -= 1.0,
            }
            None
        }
        InputBindingAction::Turn(direction) => {
            look_rate.x += match direction {
                KeyboardTurnDirection::Left => -270.0,
                KeyboardTurnDirection::Right => 270.0,
            };
            None
        }
        InputBindingAction::MoveAnalog | InputBindingAction::Look => None,
        InputBindingAction::Jump => Some(PlayerAction::Jump),
        InputBindingAction::Sprint => Some(PlayerAction::Sprint),
        InputBindingAction::Sneak => Some(PlayerAction::Sneak),
        InputBindingAction::Descend => Some(PlayerAction::Descend),
        InputBindingAction::Attack => Some(PlayerAction::Attack),
        InputBindingAction::Use => Some(PlayerAction::Use),
        InputBindingAction::SelectHotbarSlot(slot) => Some(PlayerAction::SelectHotbarSlot(slot)),
        InputBindingAction::NextHotbarSlot => Some(PlayerAction::NextHotbarSlot),
        InputBindingAction::PreviousHotbarSlot => Some(PlayerAction::PreviousHotbarSlot),
        InputBindingAction::OpenMenu => Some(PlayerAction::OpenMenu),
        InputBindingAction::OpenBlockPalette => Some(PlayerAction::OpenBlockPalette),
        InputBindingAction::OpenHelp => Some(PlayerAction::OpenHelp),
        InputBindingAction::ToggleCameraView => Some(PlayerAction::ToggleCameraView),
    };
    if let Some(action) = semantic {
        held.insert(action);
    }
    movement.left = movement.left.clamp(-1.0, 1.0);
    movement.forward = movement.forward.clamp(-1.0, 1.0);
}

fn resolve_navigation(
    state: &mut ControllerSourceState,
    stick: glam::Vec2,
    settings: ControllerSessionSettings,
) -> BTreeSet<PlayerAction> {
    let mut held = BTreeSet::new();
    for (action, pressed) in [
        (
            PlayerAction::UiNavigateUp,
            state.snapshot.buttons.dpad_up.pressed
                || axis_held(
                    stick.y,
                    true,
                    state.navigation_held.contains(&PlayerAction::UiNavigateUp),
                    settings,
                ),
        ),
        (
            PlayerAction::UiNavigateDown,
            state.snapshot.buttons.dpad_down.pressed
                || axis_held(
                    stick.y,
                    false,
                    state
                        .navigation_held
                        .contains(&PlayerAction::UiNavigateDown),
                    settings,
                ),
        ),
        (
            PlayerAction::UiNavigateLeft,
            state.snapshot.buttons.dpad_left.pressed
                || axis_held(
                    stick.x,
                    false,
                    state
                        .navigation_held
                        .contains(&PlayerAction::UiNavigateLeft),
                    settings,
                ),
        ),
        (
            PlayerAction::UiNavigateRight,
            state.snapshot.buttons.dpad_right.pressed
                || axis_held(
                    stick.x,
                    true,
                    state
                        .navigation_held
                        .contains(&PlayerAction::UiNavigateRight),
                    settings,
                ),
        ),
        (
            PlayerAction::UiConfirm,
            state.snapshot.buttons.south.pressed,
        ),
        (
            PlayerAction::UiBack,
            state.snapshot.buttons.east.pressed || state.snapshot.buttons.start.pressed,
        ),
        (
            PlayerAction::UiPreviousPage,
            state.snapshot.buttons.left_shoulder.pressed,
        ),
        (
            PlayerAction::UiNextPage,
            state.snapshot.buttons.right_shoulder.pressed,
        ),
    ] {
        if pressed {
            held.insert(action);
        }
    }
    state.navigation_held = held
        .iter()
        .filter(|action| is_navigation_action(**action))
        .copied()
        .collect();
    held
}

fn is_navigation_action(action: PlayerAction) -> bool {
    matches!(
        action,
        PlayerAction::UiNavigateUp
            | PlayerAction::UiNavigateDown
            | PlayerAction::UiNavigateLeft
            | PlayerAction::UiNavigateRight
    )
}

fn axis_held(
    value: f32,
    positive: bool,
    was_held: bool,
    settings: ControllerSessionSettings,
) -> bool {
    let signed = if positive { value } else { -value };
    signed
        >= if was_held {
            settings.navigation_release_threshold
        } else {
            settings.navigation_press_threshold
        }
}

fn control_pressed(state: &ControllerSourceState, control: GamepadControl) -> bool {
    let buttons = state.snapshot.buttons;
    match control {
        GamepadControl::LeftStick | GamepadControl::RightStick => false,
        GamepadControl::SouthButton => buttons.south.pressed,
        GamepadControl::EastButton => buttons.east.pressed,
        GamepadControl::WestButton => buttons.west.pressed,
        GamepadControl::NorthButton => buttons.north.pressed,
        GamepadControl::LeftShoulder => buttons.left_shoulder.pressed,
        GamepadControl::RightShoulder => buttons.right_shoulder.pressed,
        GamepadControl::DPadLeft => buttons.dpad_left.pressed,
        GamepadControl::DPadRight => buttons.dpad_right.pressed,
        GamepadControl::DPadUp => buttons.dpad_up.pressed,
        GamepadControl::DPadDown => buttons.dpad_down.pressed,
        GamepadControl::StartButton => buttons.start.pressed,
        GamepadControl::SelectButton => buttons.select.pressed,
        GamepadControl::LeftTrigger => state.left_trigger_pressed,
        GamepadControl::RightTrigger => state.right_trigger_pressed,
        GamepadControl::LeftStickButton => buttons.left_stick.pressed,
        GamepadControl::RightStickButton => buttons.right_stick.pressed,
        GamepadControl::GuideButton => buttons.guide.pressed,
    }
}

fn hysteretic_trigger(
    was_pressed: bool,
    state: StandardGamepadButtonState,
    settings: ControllerSessionSettings,
) -> bool {
    state.pressed
        || if was_pressed {
            state.value > settings.trigger_release_threshold
        } else {
            state.value >= settings.trigger_press_threshold
        }
}

fn adjusted_stick(value: glam::Vec2, deadzone: f32, exponent: f32) -> glam::Vec2 {
    let value = glam::Vec2::new(finite(value.x), finite(value.y)).clamp_length_max(1.0);
    let magnitude = value.length();
    if magnitude <= deadzone || magnitude == 0.0 {
        return glam::Vec2::ZERO;
    }
    let scaled = ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
    value / magnitude * scaled.powf(exponent)
}

fn normalized_dt(value: f64) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 0.1) as f32
    } else {
        0.0
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn deadzone(value: f32) -> f32 {
    finite(value).clamp(0.0, 0.99)
}

fn unit_interval(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn positive_finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InputSourceIdAllocator, StandardGamepadButtonState, StandardGamepadButtons};
    use glam::Vec2;

    fn source() -> (InputSourceId, InputSourceDescriptor) {
        let mut allocator = InputSourceIdAllocator::new();
        (
            allocator.allocate().expect("first source"),
            InputSourceDescriptor::scripted_gamepad("test controller"),
        )
    }

    #[test]
    fn right_stick_is_frame_rate_invariant() {
        let (source_id, descriptor) = source();
        let mut session = ControllerInputSession::new();
        session.connect_source(source_id, descriptor);
        let frame = session
            .sample_frame(
                Duration::ZERO,
                [(
                    source_id,
                    StandardGamepadSnapshot {
                        right_stick: Vec2::new(1.0, 0.0),
                        ..StandardGamepadSnapshot::default()
                    },
                )],
            )
            .expect("sample");
        let sixty_hz = frame.to_flat_frame(1.0 / 60.0).look_delta.x * 60.0;
        let one_twenty_hz = frame.to_flat_frame(1.0 / 120.0).look_delta.x * 120.0;
        assert!((sixty_hz - one_twenty_hz).abs() < 0.001);
        assert!((sixty_hz - ControllerSessionSettings::DEFAULT_LOOK_RATE_PER_SECOND).abs() < 0.01);
    }

    #[test]
    fn drift_does_not_activate_and_button_edges_are_exact() {
        let (source_id, descriptor) = source();
        let mut session = ControllerInputSession::new();
        session.connect_source(source_id, descriptor);
        let drift = session
            .sample_frame(
                Duration::ZERO,
                [(
                    source_id,
                    StandardGamepadSnapshot {
                        left_stick: Vec2::splat(0.05),
                        ..StandardGamepadSnapshot::default()
                    },
                )],
            )
            .expect("drift sample");
        assert!(drift.is_idle());
        assert_eq!(drift.active_source, None);

        let pressed_snapshot = StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                south: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        let pressed = session
            .sample_frame(Duration::from_millis(1), [(source_id, pressed_snapshot)])
            .expect("pressed sample");
        assert!(pressed.pressed.contains(&PlayerAction::Jump));
        assert!(pressed.held.contains(&PlayerAction::Jump));
        assert_eq!(pressed.activity_source, Some(source_id));
        assert_eq!(pressed.active_source, Some(source_id));

        let held = session
            .sample_frame(Duration::from_millis(2), [(source_id, pressed_snapshot)])
            .expect("held sample");
        assert!(!held.pressed.contains(&PlayerAction::Jump));
        assert!(held.held.contains(&PlayerAction::Jump));
        assert_eq!(held.activity_source, None);

        let released = session
            .sample_frame(
                Duration::from_millis(3),
                [(source_id, StandardGamepadSnapshot::default())],
            )
            .expect("released sample");
        assert!(released.released.contains(&PlayerAction::Jump));
        assert!(!released.held.contains(&PlayerAction::Jump));
    }

    #[test]
    fn trigger_hysteresis_avoids_threshold_chatter() {
        let (source_id, descriptor) = source();
        let mut session = ControllerInputSession::with_bindings(GamepadBindings {
            bindings: vec![crate::GamepadBinding::new(
                GamepadControl::RightTrigger,
                InputBindingAction::Attack,
            )],
        });
        session.connect_source(source_id, descriptor);
        let trigger = |value| StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                right_trigger: StandardGamepadButtonState {
                    value,
                    pressed: false,
                },
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        assert!(
            session
                .sample_frame(Duration::ZERO, [(source_id, trigger(0.54))])
                .expect("below press")
                .pressed
                .is_empty()
        );
        assert!(
            session
                .sample_frame(Duration::from_millis(1), [(source_id, trigger(0.56))])
                .expect("press")
                .pressed
                .contains(&PlayerAction::Attack)
        );
        assert!(
            session
                .sample_frame(Duration::from_millis(2), [(source_id, trigger(0.50))])
                .expect("hysteresis hold")
                .held
                .contains(&PlayerAction::Attack)
        );
        assert!(
            session
                .sample_frame(Duration::from_millis(3), [(source_id, trigger(0.44))])
                .expect("release")
                .released
                .contains(&PlayerAction::Attack)
        );
    }

    #[test]
    fn context_change_does_not_replay_held_confirm() {
        let (source_id, descriptor) = source();
        let mut session = ControllerInputSession::new();
        session.connect_source(source_id, descriptor);
        let held = StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                south: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        session
            .sample_frame(Duration::ZERO, [(source_id, held)])
            .expect("gameplay press");
        session.set_context(InputContext::Menu);
        let menu = session
            .sample_frame(Duration::from_millis(1), [(source_id, held)])
            .expect("menu hold");
        assert!(menu.held.contains(&PlayerAction::UiConfirm));
        assert!(!menu.pressed.contains(&PlayerAction::UiConfirm));
        assert!(menu.released.contains(&PlayerAction::Jump));
    }

    #[test]
    fn navigation_repeat_uses_shared_monotonic_time() {
        let (source_id, descriptor) = source();
        let mut session = ControllerInputSession::new();
        session.connect_source(source_id, descriptor);
        session.set_context(InputContext::Menu);
        let down = StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                dpad_down: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        let first = session
            .sample_frame(Duration::ZERO, [(source_id, down)])
            .expect("first navigation");
        assert!(first.pressed.contains(&PlayerAction::UiNavigateDown));
        let early = session
            .sample_frame(Duration::from_millis(349), [(source_id, down)])
            .expect("early navigation");
        assert!(!early.pressed.contains(&PlayerAction::UiNavigateDown));
        let repeat = session
            .sample_frame(Duration::from_millis(350), [(source_id, down)])
            .expect("repeated navigation");
        assert!(repeat.pressed.contains(&PlayerAction::UiNavigateDown));
    }

    #[test]
    fn most_recent_meaningful_source_owns_continuous_axes() {
        let mut allocator = InputSourceIdAllocator::new();
        let first = allocator.allocate().expect("first");
        let second = allocator.allocate().expect("second");
        let mut session = ControllerInputSession::new();
        session.connect_source(first, InputSourceDescriptor::scripted_gamepad("first"));
        session.connect_source(second, InputSourceDescriptor::scripted_gamepad("second"));
        let first_move = StandardGamepadSnapshot {
            left_stick: Vec2::new(-1.0, 0.0),
            ..StandardGamepadSnapshot::default()
        };
        let frame = session
            .sample_frame(
                Duration::ZERO,
                [
                    (first, first_move),
                    (second, StandardGamepadSnapshot::default()),
                ],
            )
            .expect("first active");
        assert_eq!(frame.active_source, Some(first));
        assert!(frame.movement.left > 0.0);

        let second_move = StandardGamepadSnapshot {
            left_stick: Vec2::new(1.0, 0.0),
            ..StandardGamepadSnapshot::default()
        };
        let frame = session
            .sample_frame(
                Duration::from_millis(1),
                [
                    (first, StandardGamepadSnapshot::default()),
                    (second, second_move),
                ],
            )
            .expect("second active");
        assert_eq!(frame.active_source, Some(second));
        assert!(frame.movement.left < 0.0);
    }

    #[test]
    fn disconnect_clears_held_actions_on_next_frame() {
        let (source_id, descriptor) = source();
        let mut session = ControllerInputSession::new();
        session.connect_source(source_id, descriptor);
        session
            .sample_frame(
                Duration::ZERO,
                [(
                    source_id,
                    StandardGamepadSnapshot {
                        buttons: StandardGamepadButtons {
                            south: StandardGamepadButtonState::pressed(),
                            ..StandardGamepadButtons::default()
                        },
                        ..StandardGamepadSnapshot::default()
                    },
                )],
            )
            .expect("press");
        assert!(session.disconnect_source(source_id).expect("disconnect"));
        let frame = session
            .sample_frame(Duration::from_millis(1), [])
            .expect("post-disconnect frame");
        assert!(frame.released.contains(&PlayerAction::Jump));
        assert_eq!(frame.active_source, None);
    }

    #[test]
    fn lifecycle_clear_requires_neutral_before_controls_rearm() {
        let (source_id, descriptor) = source();
        let mut session = ControllerInputSession::new();
        session.connect_source(source_id, descriptor);
        let pressed = StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                south: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        session
            .sample_frame(Duration::ZERO, [(source_id, pressed)])
            .expect("initial press");

        session.clear_held();
        let suppressed = session
            .sample_frame(Duration::from_millis(1), [(source_id, pressed)])
            .expect("held after lifecycle clear");
        assert!(!suppressed.pressed.contains(&PlayerAction::Jump));
        assert!(!suppressed.held.contains(&PlayerAction::Jump));
        assert_eq!(suppressed.activity_source, None);

        session
            .sample_frame(
                Duration::from_millis(2),
                [(
                    source_id,
                    StandardGamepadSnapshot {
                        left_stick: Vec2::splat(0.05),
                        ..StandardGamepadSnapshot::default()
                    },
                )],
            )
            .expect("post-clear neutral drift");
        let rearmed = session
            .sample_frame(Duration::from_millis(3), [(source_id, pressed)])
            .expect("rearmed press");
        assert!(rearmed.pressed.contains(&PlayerAction::Jump));
        assert_eq!(rearmed.activity_source, Some(source_id));
    }
}
