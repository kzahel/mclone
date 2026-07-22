use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, num::NonZeroU64, time::Duration};

mod controller_session;

pub use controller_session::{
    ControllerInputError, ControllerInputSession, ControllerSessionSettings, InputContext,
    PlayerAction, PlayerActionFrame,
};

pub const FLAT_HOTBAR_SLOT_COUNT: u8 = 9;
/// Mouse-delta units per second for held keyboard turning; intentionally slower than mouselook.
pub const KEYBOARD_TURN_MOUSE_DELTA_PER_SECOND: f64 = 270.0;

/// Host-neutral tracked-controller side. OpenXR is one producer, while desktop
/// XR emulation and future browser XR adapters can produce the same input data
/// without introducing an OpenXR dependency into shared scene policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XrHand {
    Left,
    Right,
}

/// Host-neutral tracked-controller input/pose snapshot.
///
/// Platform adapters translate their controller APIs into this shared input
/// contract; scene locomotion and interaction consume it.
#[derive(Clone, Copy, Debug)]
pub struct XrControllerSnapshot {
    pub hand: XrHand,
    pub aim_position: Option<Vec3>,
    pub aim_direction: Option<Vec3>,
    pub grip_position: Option<Vec3>,
    /// Full palm-relative grip orientation used by thruster/repulsor input.
    pub grip_orientation: Option<Quat>,
    pub trigger: f32,
    pub squeeze: f32,
    pub select_pressed: bool,
    pub a_pressed: bool,
    pub b_pressed: bool,
    pub y_pressed: bool,
    pub thumbstick: Vec2,
    pub thumbstick_pressed: bool,
}

/// Translate a flat input frame into the neutral controller facts consumed by
/// XR scene policy. This is intentionally an emulation adapter rather than a
/// second locomotion implementation: desktop headset-free tools can feed
/// keyboard input through the same stick/button semantics as a real headset.
///
/// Mouse look is not projected here. In XR, head orientation owns pitch and
/// the configured turn policy owns yaw; desktop emulation drives the synthetic
/// head from the engine camera and maps held keyboard turn bindings to the
/// right stick so snap-turn remains exercisable.
pub fn xr_emulation_controllers_from_flat_frame(
    frame: FlatInputFrame,
) -> [XrControllerSnapshot; 2] {
    let mut left = empty_xr_controller(XrHand::Left);
    left.thumbstick = Vec2::new(
        clamp_axis(-frame.movement.left),
        clamp_axis(frame.movement.forward),
    );
    left.select_pressed = frame.open_menu;
    left.thumbstick_pressed = frame.open_block_palette;
    left.y_pressed = frame.sprint;

    let mut right = empty_xr_controller(XrHand::Right);
    right.thumbstick = Vec2::new(clamp_axis(frame.keyboard_turn), 0.0);
    right.a_pressed = frame.jump;
    right.b_pressed = frame.descend;
    right.thumbstick_pressed = frame.sneak;
    right.trigger = f32::from(frame.attack);
    right.squeeze = f32::from(frame.use_item);
    [left, right]
}

fn empty_xr_controller(hand: XrHand) -> XrControllerSnapshot {
    XrControllerSnapshot {
        hand,
        aim_position: None,
        aim_direction: None,
        grip_position: None,
        grip_orientation: None,
        trigger: 0.0,
        squeeze: 0.0,
        select_pressed: false,
        a_pressed: false,
        b_pressed: false,
        y_pressed: false,
        thumbstick: Vec2::ZERO,
        thumbstick_pressed: false,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputDeviceKind {
    Keyboard,
    Mouse,
    Touch,
    Gamepad,
    XrController,
}

impl InputDeviceKind {
    pub const fn prompt_kind(self) -> InputPromptKind {
        match self {
            Self::Keyboard | Self::Mouse => InputPromptKind::KeyboardMouse,
            Self::Touch => InputPromptKind::Touch,
            Self::Gamepad => InputPromptKind::Gamepad,
            Self::XrController => InputPromptKind::XrController,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputPromptKind {
    KeyboardMouse,
    Touch,
    Gamepad,
    XrController,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputCapabilities {
    pub keyboard: bool,
    pub mouse: bool,
    pub touch: bool,
    pub gamepad: bool,
    pub xr_controller: bool,
}

impl InputCapabilities {
    pub const NONE: Self = Self {
        keyboard: false,
        mouse: false,
        touch: false,
        gamepad: false,
        xr_controller: false,
    };

    pub fn set_present(&mut self, kind: InputDeviceKind, present: bool) {
        match kind {
            InputDeviceKind::Keyboard => self.keyboard = present,
            InputDeviceKind::Mouse => self.mouse = present,
            InputDeviceKind::Touch => self.touch = present,
            InputDeviceKind::Gamepad => self.gamepad = present,
            InputDeviceKind::XrController => self.xr_controller = present,
        }
    }

    pub const fn is_present(self, kind: InputDeviceKind) -> bool {
        match kind {
            InputDeviceKind::Keyboard => self.keyboard,
            InputDeviceKind::Mouse => self.mouse,
            InputDeviceKind::Touch => self.touch,
            InputDeviceKind::Gamepad => self.gamepad,
            InputDeviceKind::XrController => self.xr_controller,
        }
    }

    pub const fn has_keyboard_mouse(self) -> bool {
        self.keyboard || self.mouse
    }

    pub const fn has_non_touch_flat_input(self) -> bool {
        self.keyboard || self.mouse || self.gamepad
    }

    pub const fn has_flat_input(self) -> bool {
        self.keyboard || self.mouse || self.touch || self.gamepad
    }

    pub const fn has_any(self) -> bool {
        self.has_flat_input() || self.xr_controller
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputCapabilityState {
    pub capabilities: InputCapabilities,
    pub last_active_device: Option<InputDeviceKind>,
}

impl InputCapabilityState {
    pub const fn new(capabilities: InputCapabilities) -> Self {
        Self {
            capabilities,
            last_active_device: None,
        }
    }

    pub fn set_present(&mut self, kind: InputDeviceKind, present: bool) {
        self.capabilities.set_present(kind, present);
        if !present && self.last_active_device == Some(kind) {
            self.last_active_device = None;
        }
    }

    pub fn note_activity(&mut self, kind: InputDeviceKind) {
        self.capabilities.set_present(kind, true);
        self.last_active_device = Some(kind);
    }

    pub fn clear_last_active_device(&mut self) {
        self.last_active_device = None;
    }

    pub fn last_active_prompt_kind(self) -> Option<InputPromptKind> {
        let kind = self.last_active_device?;
        self.capabilities
            .is_present(kind)
            .then_some(kind.prompt_kind())
    }

    pub fn resolve(self, preferences: InputPreferences) -> ResolvedFlatInput {
        resolve_flat_input(self, preferences)
    }
}

impl From<InputCapabilities> for InputCapabilityState {
    fn from(capabilities: InputCapabilities) -> Self {
        Self::new(capabilities)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputSchemePreset {
    #[default]
    Auto,
    KeyboardMouse,
    Touch,
    Gamepad,
    Custom,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TouchControlsMode {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputPreferences {
    pub preferred_scheme: InputSchemePreset,
    pub touch_controls: TouchControlsMode,
}

impl InputPreferences {
    pub const AUTO: Self = Self {
        preferred_scheme: InputSchemePreset::Auto,
        touch_controls: TouchControlsMode::Auto,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFlatInput {
    pub preferred_prompt: Option<InputPromptKind>,
    pub touch_controls_visible: bool,
    pub accepts_keyboard_mouse: bool,
    pub accepts_touch: bool,
    pub accepts_gamepad: bool,
    pub accepts_xr_controller: bool,
}

pub fn resolve_flat_input(
    state: InputCapabilityState,
    preferences: InputPreferences,
) -> ResolvedFlatInput {
    let capabilities = state.capabilities;
    ResolvedFlatInput {
        preferred_prompt: preferred_prompt_kind(state, preferences.preferred_scheme),
        touch_controls_visible: touch_controls_visible(state, preferences),
        accepts_keyboard_mouse: capabilities.has_keyboard_mouse(),
        accepts_touch: capabilities.touch,
        accepts_gamepad: capabilities.gamepad,
        accepts_xr_controller: capabilities.xr_controller,
    }
}

fn preferred_prompt_kind(
    state: InputCapabilityState,
    preferred_scheme: InputSchemePreset,
) -> Option<InputPromptKind> {
    match preferred_scheme {
        InputSchemePreset::Auto | InputSchemePreset::Custom => auto_prompt_kind(state),
        InputSchemePreset::KeyboardMouse => state
            .capabilities
            .has_keyboard_mouse()
            .then_some(InputPromptKind::KeyboardMouse)
            .or_else(|| auto_prompt_kind(state)),
        InputSchemePreset::Touch => state
            .capabilities
            .touch
            .then_some(InputPromptKind::Touch)
            .or_else(|| auto_prompt_kind(state)),
        InputSchemePreset::Gamepad => state
            .capabilities
            .gamepad
            .then_some(InputPromptKind::Gamepad)
            .or_else(|| auto_prompt_kind(state)),
    }
}

fn auto_prompt_kind(state: InputCapabilityState) -> Option<InputPromptKind> {
    state
        .last_active_prompt_kind()
        .or_else(|| fallback_prompt_kind(state.capabilities))
}

fn fallback_prompt_kind(capabilities: InputCapabilities) -> Option<InputPromptKind> {
    if capabilities.has_keyboard_mouse() {
        Some(InputPromptKind::KeyboardMouse)
    } else if capabilities.gamepad {
        Some(InputPromptKind::Gamepad)
    } else if capabilities.touch {
        Some(InputPromptKind::Touch)
    } else if capabilities.xr_controller {
        Some(InputPromptKind::XrController)
    } else {
        None
    }
}

fn touch_controls_visible(state: InputCapabilityState, preferences: InputPreferences) -> bool {
    if !state.capabilities.touch {
        return false;
    }
    match preferences.touch_controls {
        TouchControlsMode::Off => false,
        TouchControlsMode::On => true,
        TouchControlsMode::Auto => match preferences.preferred_scheme {
            InputSchemePreset::KeyboardMouse | InputSchemePreset::Gamepad => false,
            InputSchemePreset::Touch => true,
            InputSchemePreset::Auto | InputSchemePreset::Custom => {
                !state.capabilities.has_non_touch_flat_input()
                    || state.last_active_prompt_kind() == Some(InputPromptKind::Touch)
            }
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MovementDirection {
    Forward,
    Backward,
    Left,
    Right,
}

impl MovementDirection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Forward => "Move Forward",
            Self::Backward => "Move Backward",
            Self::Left => "Move Left",
            Self::Right => "Move Right",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardTurnDirection {
    Left,
    Right,
}

impl KeyboardTurnDirection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Left => "Turn Left",
            Self::Right => "Turn Right",
        }
    }

    const fn impulse(self) -> f32 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlatInputAction {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FlatInputIntent {
    MoveDirection {
        direction: MovementDirection,
        pressed: bool,
    },
    Turn {
        direction: KeyboardTurnDirection,
        pressed: bool,
    },
    MoveAnalog {
        left: f32,
        forward: f32,
    },
    LookDelta {
        x: f32,
        y: f32,
    },
    Action {
        action: FlatInputAction,
        pressed: bool,
    },
    SelectHotbarSlot(u8),
    StepHotbar(i8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovementImpulse {
    pub left: f32,
    pub forward: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookDelta {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatInputFrame {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub movement: MovementImpulse,
    pub analog_movement: Option<MovementImpulse>,
    #[serde(default)]
    pub keyboard_turn: f32,
    pub look_delta: LookDelta,
    pub jump: bool,
    pub sprint: bool,
    pub sneak: bool,
    pub descend: bool,
    pub attack: bool,
    pub use_item: bool,
    pub open_menu: bool,
    pub open_block_palette: bool,
    #[serde(default)]
    pub open_help: bool,
    #[serde(default)]
    pub toggle_camera_view: bool,
    pub selected_hotbar_slot: Option<u8>,
    pub hotbar_step: i8,
}

impl FlatInputFrame {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn apply_intent(&mut self, intent: FlatInputIntent) {
        match intent {
            FlatInputIntent::MoveDirection { direction, pressed } => {
                if pressed {
                    self.add_movement_direction(direction);
                }
            }
            FlatInputIntent::Turn { direction, pressed } => {
                if pressed {
                    self.add_keyboard_turn_direction(direction);
                }
            }
            FlatInputIntent::MoveAnalog { left, forward } => {
                self.set_analog_movement_impulse(left, forward);
            }
            FlatInputIntent::LookDelta { x, y } => self.add_look_delta(x, y),
            FlatInputIntent::Action { action, pressed } => {
                if pressed {
                    self.press_action(action);
                }
            }
            FlatInputIntent::SelectHotbarSlot(slot) => {
                self.select_hotbar_slot(slot);
            }
            FlatInputIntent::StepHotbar(step) => self.step_hotbar(step),
        }
    }

    /// Merge another producer's frame into this one without exposing the
    /// engine-camera representation to platform adapters.
    pub fn merge_from(&mut self, source: Self) {
        for (pressed, direction) in [
            (source.forward, MovementDirection::Forward),
            (source.backward, MovementDirection::Backward),
            (source.left, MovementDirection::Left),
            (source.right, MovementDirection::Right),
        ] {
            if pressed {
                self.add_movement_direction(direction);
            }
        }
        if let Some(movement) = source.analog_movement {
            self.set_analog_movement_impulse(movement.left, movement.forward);
        }
        self.add_keyboard_turn_impulse(source.keyboard_turn);
        self.add_look_delta(source.look_delta.x, source.look_delta.y);
        for (pressed, action) in [
            (source.jump, FlatInputAction::Jump),
            (source.sprint, FlatInputAction::Sprint),
            (source.sneak, FlatInputAction::Sneak),
            (source.descend, FlatInputAction::Descend),
            (source.attack, FlatInputAction::Attack),
            (source.use_item, FlatInputAction::Use),
            (source.open_menu, FlatInputAction::OpenMenu),
            (source.open_block_palette, FlatInputAction::OpenBlockPalette),
            (source.open_help, FlatInputAction::OpenHelp),
            (source.toggle_camera_view, FlatInputAction::ToggleCameraView),
        ] {
            if pressed {
                self.press_action(action);
            }
        }
        if let Some(slot) = source.selected_hotbar_slot {
            self.select_hotbar_slot(slot);
        }
        self.step_hotbar(source.hotbar_step);
    }

    pub fn add_movement_direction(&mut self, direction: MovementDirection) {
        let (left, forward) = match direction {
            MovementDirection::Forward => {
                self.forward = true;
                (0.0, 1.0)
            }
            MovementDirection::Backward => {
                self.backward = true;
                (0.0, -1.0)
            }
            MovementDirection::Left => {
                self.left = true;
                (1.0, 0.0)
            }
            MovementDirection::Right => {
                self.right = true;
                (-1.0, 0.0)
            }
        };
        self.add_movement_impulse(left, forward);
    }

    pub fn add_keyboard_turn_direction(&mut self, direction: KeyboardTurnDirection) {
        self.add_keyboard_turn_impulse(direction.impulse());
    }

    pub fn add_keyboard_turn_impulse(&mut self, turn: f32) {
        self.keyboard_turn = clamp_axis(self.keyboard_turn + finite_axis(turn));
    }

    pub fn set_analog_movement_impulse(&mut self, left: f32, forward: f32) {
        let movement = MovementImpulse {
            left: clamp_axis(finite_axis(left)),
            forward: clamp_axis(finite_axis(forward)),
        };
        self.analog_movement = Some(movement);
        self.add_movement_impulse(movement.left, movement.forward);
    }

    pub fn add_movement_impulse(&mut self, left: f32, forward: f32) {
        self.movement.left = clamp_axis(self.movement.left + finite_axis(left));
        self.movement.forward = clamp_axis(self.movement.forward + finite_axis(forward));
    }

    pub fn add_look_delta(&mut self, x: f32, y: f32) {
        if x.is_finite() {
            self.look_delta.x += x;
        }
        if y.is_finite() {
            self.look_delta.y += y;
        }
    }

    pub fn press_action(&mut self, action: FlatInputAction) {
        match action {
            FlatInputAction::Jump => self.jump = true,
            FlatInputAction::Sprint => self.sprint = true,
            FlatInputAction::Sneak => self.sneak = true,
            FlatInputAction::Descend => self.descend = true,
            FlatInputAction::Attack => self.attack = true,
            FlatInputAction::Use => self.use_item = true,
            FlatInputAction::OpenMenu => self.open_menu = true,
            FlatInputAction::OpenBlockPalette => self.open_block_palette = true,
            FlatInputAction::OpenHelp => self.open_help = true,
            FlatInputAction::ToggleCameraView => self.toggle_camera_view = true,
        }
    }

    pub fn select_hotbar_slot(&mut self, slot: u8) -> bool {
        if slot >= FLAT_HOTBAR_SLOT_COUNT {
            return false;
        }
        self.selected_hotbar_slot = Some(slot);
        true
    }

    pub fn step_hotbar(&mut self, step: i8) {
        self.hotbar_step = self.hotbar_step.saturating_add(step);
    }

    pub fn apply_binary_binding_action(&mut self, action: InputBindingAction, pressed: bool) {
        if !pressed {
            return;
        }
        match action {
            InputBindingAction::Move(direction) => self.add_movement_direction(direction),
            InputBindingAction::Turn(direction) => self.add_keyboard_turn_direction(direction),
            InputBindingAction::MoveAnalog | InputBindingAction::Look => {}
            InputBindingAction::Jump => self.press_action(FlatInputAction::Jump),
            InputBindingAction::Sprint => self.press_action(FlatInputAction::Sprint),
            InputBindingAction::Sneak => self.press_action(FlatInputAction::Sneak),
            InputBindingAction::Descend => self.press_action(FlatInputAction::Descend),
            InputBindingAction::Attack => self.press_action(FlatInputAction::Attack),
            InputBindingAction::Use => self.press_action(FlatInputAction::Use),
            InputBindingAction::SelectHotbarSlot(slot) => {
                self.select_hotbar_slot(slot);
            }
            InputBindingAction::NextHotbarSlot => self.step_hotbar(1),
            InputBindingAction::PreviousHotbarSlot => self.step_hotbar(-1),
            InputBindingAction::OpenMenu => self.press_action(FlatInputAction::OpenMenu),
            InputBindingAction::OpenBlockPalette => {
                self.press_action(FlatInputAction::OpenBlockPalette);
            }
            InputBindingAction::OpenHelp => {
                self.press_action(FlatInputAction::OpenHelp);
            }
            InputBindingAction::ToggleCameraView => {
                self.press_action(FlatInputAction::ToggleCameraView);
            }
        }
    }
}

fn finite_axis(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn clamp_axis(value: f32) -> f32 {
    value.clamp(-1.0, 1.0)
}

pub fn keyboard_turn_mouse_delta(turn: f32, dt_seconds: f64) -> f64 {
    let turn = f64::from(clamp_axis(finite_axis(turn)));
    let dt_seconds = if dt_seconds.is_finite() {
        dt_seconds.clamp(0.0, 0.1)
    } else {
        0.0
    };
    -turn * KEYBOARD_TURN_MOUSE_DELTA_PER_SECOND * dt_seconds
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBindings {
    pub keyboard_mouse: KeyboardMouseBindings,
    pub touch: TouchBindings,
    pub gamepad: GamepadBindings,
}

impl Default for InputBindings {
    fn default() -> Self {
        Self {
            keyboard_mouse: KeyboardMouseBindings::default(),
            touch: TouchBindings::default(),
            gamepad: GamepadBindings::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardMouseBindings {
    pub bindings: Vec<KeyboardMouseBinding>,
}

impl Default for KeyboardMouseBindings {
    fn default() -> Self {
        let mut bindings = vec![
            KeyboardMouseBinding::key(
                KeyboardKey::KeyW,
                InputBindingAction::Move(MovementDirection::Forward),
            ),
            KeyboardMouseBinding::key(
                KeyboardKey::KeyS,
                InputBindingAction::Move(MovementDirection::Backward),
            ),
            KeyboardMouseBinding::key(
                KeyboardKey::KeyA,
                InputBindingAction::Move(MovementDirection::Left),
            ),
            KeyboardMouseBinding::key(
                KeyboardKey::KeyD,
                InputBindingAction::Move(MovementDirection::Right),
            ),
            KeyboardMouseBinding::key(
                KeyboardKey::ArrowUp,
                InputBindingAction::Move(MovementDirection::Forward),
            ),
            KeyboardMouseBinding::key(
                KeyboardKey::ArrowDown,
                InputBindingAction::Move(MovementDirection::Backward),
            ),
            KeyboardMouseBinding::key(
                KeyboardKey::ArrowLeft,
                InputBindingAction::Turn(KeyboardTurnDirection::Left),
            ),
            KeyboardMouseBinding::key(
                KeyboardKey::ArrowRight,
                InputBindingAction::Turn(KeyboardTurnDirection::Right),
            ),
            KeyboardMouseBinding::key(KeyboardKey::Space, InputBindingAction::Jump),
            KeyboardMouseBinding::key(KeyboardKey::KeyX, InputBindingAction::Descend),
            KeyboardMouseBinding::key(KeyboardKey::ShiftLeft, InputBindingAction::Sneak),
            KeyboardMouseBinding::key(KeyboardKey::ShiftRight, InputBindingAction::Sneak),
            KeyboardMouseBinding::key(KeyboardKey::ControlLeft, InputBindingAction::Sprint),
            KeyboardMouseBinding::key(KeyboardKey::ControlRight, InputBindingAction::Sprint),
            KeyboardMouseBinding::key(KeyboardKey::Escape, InputBindingAction::OpenMenu),
            KeyboardMouseBinding::key(KeyboardKey::KeyE, InputBindingAction::OpenBlockPalette),
            KeyboardMouseBinding::key(KeyboardKey::KeyB, InputBindingAction::OpenBlockPalette),
            KeyboardMouseBinding::key(KeyboardKey::F1, InputBindingAction::OpenHelp),
            KeyboardMouseBinding::key(KeyboardKey::F5, InputBindingAction::ToggleCameraView),
            KeyboardMouseBinding::mouse_button(PointerButton::Primary, InputBindingAction::Attack),
            KeyboardMouseBinding::mouse_button(PointerButton::Secondary, InputBindingAction::Use),
            KeyboardMouseBinding {
                control: KeyboardMouseControl::MouseMotion,
                action: InputBindingAction::Look,
            },
            KeyboardMouseBinding {
                control: KeyboardMouseControl::MouseWheelUp,
                action: InputBindingAction::PreviousHotbarSlot,
            },
            KeyboardMouseBinding {
                control: KeyboardMouseControl::MouseWheelDown,
                action: InputBindingAction::NextHotbarSlot,
            },
        ];
        for slot in 0..FLAT_HOTBAR_SLOT_COUNT {
            let Some(key) = KeyboardKey::from_hotbar_slot(slot) else {
                continue;
            };
            bindings.push(KeyboardMouseBinding::key(
                key,
                InputBindingAction::SelectHotbarSlot(slot),
            ));
        }
        Self { bindings }
    }
}

impl KeyboardMouseBindings {
    pub fn shortcut_rows(&self) -> Vec<ShortcutHelpRow> {
        self.bindings
            .iter()
            .map(KeyboardMouseBinding::shortcut_row)
            .collect()
    }
}

pub fn default_keyboard_mouse_shortcut_rows() -> Vec<ShortcutHelpRow> {
    KeyboardMouseBindings::default().shortcut_rows()
}

pub fn flat_runtime_shortcut_rows() -> Vec<ShortcutHelpRow> {
    FLAT_RUNTIME_SHORTCUTS
        .iter()
        .map(RuntimeShortcutBinding::shortcut_row)
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutHelpRow {
    pub group: ShortcutHelpGroup,
    pub control: String,
    pub action: String,
    pub remappable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutHelpGroup {
    KeyboardMouse,
    RuntimeDebug,
}

impl ShortcutHelpGroup {
    pub const fn label(self) -> &'static str {
        match self {
            Self::KeyboardMouse => "Keyboard / Mouse",
            Self::RuntimeDebug => "Runtime / Debug",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeShortcutBinding {
    pub control: KeyboardMouseControl,
    pub action: RuntimeShortcutAction,
}

impl RuntimeShortcutBinding {
    pub const fn new(control: KeyboardMouseControl, action: RuntimeShortcutAction) -> Self {
        Self { control, action }
    }

    pub fn shortcut_row(&self) -> ShortcutHelpRow {
        ShortcutHelpRow {
            group: ShortcutHelpGroup::RuntimeDebug,
            control: self.control.label(),
            action: self.action.label().to_owned(),
            remappable: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeShortcutAction {
    ToggleMovementMode,
    IncreaseFlySpeed,
    DecreaseFlySpeed,
    ToggleDebugPane,
    ToggleSectionOcclusion,
    ToggleFullbright,
    ShootDebugPhysicsCube,
    RebuildRenderResources,
    CycleRenderScaleRebuild,
}

impl RuntimeShortcutAction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ToggleMovementMode => "Movement Mode",
            Self::IncreaseFlySpeed => "Increase Fly Speed",
            Self::DecreaseFlySpeed => "Decrease Fly Speed",
            Self::ToggleDebugPane => "Toggle Debug Pane",
            Self::ToggleSectionOcclusion => "Occlusion Toggle",
            Self::ToggleFullbright => "Toggle Fullbright",
            Self::ShootDebugPhysicsCube => "Shoot Debug Cube",
            Self::RebuildRenderResources => "Renderer Rebuild",
            Self::CycleRenderScaleRebuild => "Render Scale Cycle",
        }
    }
}

pub const FLAT_RUNTIME_SHORTCUTS: &[RuntimeShortcutBinding] = &[
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::Key(KeyboardKey::KeyN),
        RuntimeShortcutAction::ToggleMovementMode,
    ),
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::MouseWheelUp,
        RuntimeShortcutAction::IncreaseFlySpeed,
    ),
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::MouseWheelDown,
        RuntimeShortcutAction::DecreaseFlySpeed,
    ),
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::Key(KeyboardKey::Backquote),
        RuntimeShortcutAction::ToggleDebugPane,
    ),
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::Key(KeyboardKey::KeyO),
        RuntimeShortcutAction::ToggleSectionOcclusion,
    ),
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::Key(KeyboardKey::KeyL),
        RuntimeShortcutAction::ToggleFullbright,
    ),
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::Key(KeyboardKey::F7),
        RuntimeShortcutAction::ShootDebugPhysicsCube,
    ),
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::Key(KeyboardKey::F8),
        RuntimeShortcutAction::RebuildRenderResources,
    ),
    RuntimeShortcutBinding::new(
        KeyboardMouseControl::Key(KeyboardKey::F9),
        RuntimeShortcutAction::CycleRenderScaleRebuild,
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardMouseBinding {
    pub control: KeyboardMouseControl,
    pub action: InputBindingAction,
}

impl KeyboardMouseBinding {
    pub const fn key(key: KeyboardKey, action: InputBindingAction) -> Self {
        Self {
            control: KeyboardMouseControl::Key(key),
            action,
        }
    }

    pub const fn mouse_button(button: PointerButton, action: InputBindingAction) -> Self {
        Self {
            control: KeyboardMouseControl::MouseButton(button),
            action,
        }
    }

    pub fn shortcut_row(&self) -> ShortcutHelpRow {
        ShortcutHelpRow {
            group: ShortcutHelpGroup::KeyboardMouse,
            control: self.control.label(),
            action: self.action.label(),
            remappable: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardMouseControl {
    Key(KeyboardKey),
    MouseButton(PointerButton),
    MouseMotion,
    MouseWheelUp,
    MouseWheelDown,
}

impl KeyboardMouseControl {
    pub fn label(self) -> String {
        match self {
            Self::Key(key) => key.label().to_owned(),
            Self::MouseButton(button) => button.label().to_owned(),
            Self::MouseMotion => "Mouse Move".to_owned(),
            Self::MouseWheelUp => "Wheel Up".to_owned(),
            Self::MouseWheelDown => "Wheel Down".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardKey {
    KeyW,
    KeyA,
    KeyS,
    KeyD,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    KeyE,
    KeyB,
    KeyL,
    KeyN,
    KeyO,
    KeyX,
    Backquote,
    Space,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    Escape,
    F1,
    F5,
    F7,
    F8,
    F9,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
}

impl KeyboardKey {
    /// Parse the stable physical-key names used by winit and the browser
    /// `KeyboardEvent.code` API.
    pub fn from_code_name(code: &str) -> Option<Self> {
        match code {
            "KeyW" => Some(Self::KeyW),
            "KeyA" => Some(Self::KeyA),
            "KeyS" => Some(Self::KeyS),
            "KeyD" => Some(Self::KeyD),
            "ArrowUp" => Some(Self::ArrowUp),
            "ArrowDown" => Some(Self::ArrowDown),
            "ArrowLeft" => Some(Self::ArrowLeft),
            "ArrowRight" => Some(Self::ArrowRight),
            "KeyE" => Some(Self::KeyE),
            "KeyB" => Some(Self::KeyB),
            "KeyL" => Some(Self::KeyL),
            "KeyN" => Some(Self::KeyN),
            "KeyO" => Some(Self::KeyO),
            "KeyX" => Some(Self::KeyX),
            "Backquote" => Some(Self::Backquote),
            "Space" => Some(Self::Space),
            "ShiftLeft" => Some(Self::ShiftLeft),
            "ShiftRight" => Some(Self::ShiftRight),
            "ControlLeft" => Some(Self::ControlLeft),
            "ControlRight" => Some(Self::ControlRight),
            "Escape" => Some(Self::Escape),
            "F1" => Some(Self::F1),
            "F5" => Some(Self::F5),
            "F7" => Some(Self::F7),
            "F8" => Some(Self::F8),
            "F9" => Some(Self::F9),
            "Digit1" => Some(Self::Digit1),
            "Digit2" => Some(Self::Digit2),
            "Digit3" => Some(Self::Digit3),
            "Digit4" => Some(Self::Digit4),
            "Digit5" => Some(Self::Digit5),
            "Digit6" => Some(Self::Digit6),
            "Digit7" => Some(Self::Digit7),
            "Digit8" => Some(Self::Digit8),
            "Digit9" => Some(Self::Digit9),
            _ => None,
        }
    }

    pub const fn from_hotbar_slot(slot: u8) -> Option<Self> {
        match slot {
            0 => Some(Self::Digit1),
            1 => Some(Self::Digit2),
            2 => Some(Self::Digit3),
            3 => Some(Self::Digit4),
            4 => Some(Self::Digit5),
            5 => Some(Self::Digit6),
            6 => Some(Self::Digit7),
            7 => Some(Self::Digit8),
            8 => Some(Self::Digit9),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::KeyW => "W",
            Self::KeyA => "A",
            Self::KeyS => "S",
            Self::KeyD => "D",
            Self::ArrowUp => "Arrow Up",
            Self::ArrowDown => "Arrow Down",
            Self::ArrowLeft => "Arrow Left",
            Self::ArrowRight => "Arrow Right",
            Self::KeyE => "E",
            Self::KeyB => "B",
            Self::KeyL => "L",
            Self::KeyN => "N",
            Self::KeyO => "O",
            Self::KeyX => "X",
            Self::Backquote => "`",
            Self::Space => "Space",
            Self::ShiftLeft => "Left Shift",
            Self::ShiftRight => "Right Shift",
            Self::ControlLeft => "Left Ctrl",
            Self::ControlRight => "Right Ctrl",
            Self::Escape => "Esc",
            Self::F1 => "F1",
            Self::F5 => "F5",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::Digit5 => "5",
            Self::Digit6 => "6",
            Self::Digit7 => "7",
            Self::Digit8 => "8",
            Self::Digit9 => "9",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

impl PointerButton {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Primary => "LMB",
            Self::Secondary => "RMB",
            Self::Middle => "MMB",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MouseWheelDirection {
    Up,
    Down,
}

impl MouseWheelDirection {
    const fn control(self) -> KeyboardMouseControl {
        match self {
            Self::Up => KeyboardMouseControl::MouseWheelUp,
            Self::Down => KeyboardMouseControl::MouseWheelDown,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardMouseInputAdapter {
    pub bindings: KeyboardMouseBindings,
    held: KeyboardMouseHeldState,
}

impl Default for KeyboardMouseInputAdapter {
    fn default() -> Self {
        Self {
            bindings: KeyboardMouseBindings::default(),
            held: KeyboardMouseHeldState::default(),
        }
    }
}

impl KeyboardMouseInputAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bindings(bindings: KeyboardMouseBindings) -> Self {
        Self {
            bindings,
            held: KeyboardMouseHeldState::default(),
        }
    }

    pub fn clear_held(&mut self) {
        self.held = KeyboardMouseHeldState::default();
    }

    pub fn has_continuous_movement_input(&self) -> bool {
        self.held.has_continuous_movement_input()
    }

    pub fn handle_key(
        &mut self,
        key: KeyboardKey,
        pressed: bool,
        repeat: bool,
    ) -> KeyboardMouseInputEvent {
        let Some(action) = self.binding_action(KeyboardMouseControl::Key(key)) else {
            return KeyboardMouseInputEvent::default();
        };
        self.handle_binary_action(action, pressed, repeat)
    }

    pub fn handle_mouse_button(
        &mut self,
        button: PointerButton,
        pressed: bool,
    ) -> KeyboardMouseInputEvent {
        let Some(action) = self.binding_action(KeyboardMouseControl::MouseButton(button)) else {
            return KeyboardMouseInputEvent::default();
        };
        self.handle_binary_action(action, pressed, false)
    }

    pub fn mouse_motion_frame(&self, delta_x: f32, delta_y: f32) -> Option<FlatInputFrame> {
        let Some(InputBindingAction::Look) = self.binding_action(KeyboardMouseControl::MouseMotion)
        else {
            return None;
        };
        if (!delta_x.is_finite() || delta_x == 0.0) && (!delta_y.is_finite() || delta_y == 0.0) {
            return None;
        }
        let mut frame = FlatInputFrame::default();
        frame.apply_intent(FlatInputIntent::LookDelta {
            x: delta_x,
            y: delta_y,
        });
        Some(frame)
    }

    pub fn handle_mouse_wheel(&self, direction: MouseWheelDirection) -> KeyboardMouseInputEvent {
        let Some(action) = self.binding_action(direction.control()) else {
            return KeyboardMouseInputEvent::default();
        };
        Self::binary_action_frame(action)
            .map(KeyboardMouseInputEvent::frame)
            .unwrap_or_else(KeyboardMouseInputEvent::handled)
    }

    pub fn held_frame(&self) -> Option<FlatInputFrame> {
        self.has_continuous_movement_input().then(|| {
            let mut frame = FlatInputFrame::default();
            for direction in self.held.directions() {
                frame.apply_intent(FlatInputIntent::MoveDirection {
                    direction,
                    pressed: true,
                });
            }
            for direction in self.held.turn_directions() {
                frame.apply_intent(FlatInputIntent::Turn {
                    direction,
                    pressed: true,
                });
            }
            for action in self.held.actions() {
                frame.apply_intent(FlatInputIntent::Action {
                    action,
                    pressed: true,
                });
            }
            frame
        })
    }

    fn binding_action(&self, control: KeyboardMouseControl) -> Option<InputBindingAction> {
        self.bindings
            .bindings
            .iter()
            .find_map(|binding| (binding.control == control).then_some(binding.action))
    }

    fn handle_binary_action(
        &mut self,
        action: InputBindingAction,
        pressed: bool,
        repeat: bool,
    ) -> KeyboardMouseInputEvent {
        if self.held.set_binding_action(action, pressed) {
            return KeyboardMouseInputEvent::handled();
        }
        if !pressed || repeat {
            return KeyboardMouseInputEvent::handled();
        }
        Self::binary_action_frame(action)
            .map(KeyboardMouseInputEvent::frame)
            .unwrap_or_else(KeyboardMouseInputEvent::handled)
    }

    fn binary_action_frame(action: InputBindingAction) -> Option<FlatInputFrame> {
        match action {
            InputBindingAction::MoveAnalog | InputBindingAction::Look => None,
            _ => {
                let mut frame = FlatInputFrame::default();
                frame.apply_binary_binding_action(action, true);
                Some(frame)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardMouseInputEvent {
    pub handled: bool,
    pub frame: Option<FlatInputFrame>,
}

impl KeyboardMouseInputEvent {
    pub const fn handled() -> Self {
        Self {
            handled: true,
            frame: None,
        }
    }

    pub const fn frame(frame: FlatInputFrame) -> Self {
        Self {
            handled: true,
            frame: Some(frame),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyboardMouseHeldState {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    turn_left: bool,
    turn_right: bool,
    jump: bool,
    descend: bool,
    sneak: bool,
    sprint: bool,
}

impl KeyboardMouseHeldState {
    const fn has_continuous_movement_input(self) -> bool {
        self.forward
            || self.backward
            || self.left
            || self.right
            || self.turn_left
            || self.turn_right
            || self.jump
            || self.descend
            || self.sneak
            || self.sprint
    }

    fn set_binding_action(&mut self, action: InputBindingAction, pressed: bool) -> bool {
        match action {
            InputBindingAction::Move(direction) => {
                self.set_direction(direction, pressed);
                true
            }
            InputBindingAction::Turn(direction) => {
                self.set_turn_direction(direction, pressed);
                true
            }
            InputBindingAction::Jump => {
                self.jump = pressed;
                true
            }
            InputBindingAction::Descend => {
                self.descend = pressed;
                true
            }
            InputBindingAction::Sneak => {
                self.sneak = pressed;
                true
            }
            InputBindingAction::Sprint => {
                self.sprint = pressed;
                true
            }
            InputBindingAction::MoveAnalog
            | InputBindingAction::Look
            | InputBindingAction::Attack
            | InputBindingAction::Use
            | InputBindingAction::SelectHotbarSlot(_)
            | InputBindingAction::NextHotbarSlot
            | InputBindingAction::PreviousHotbarSlot
            | InputBindingAction::OpenMenu
            | InputBindingAction::OpenBlockPalette
            | InputBindingAction::OpenHelp
            | InputBindingAction::ToggleCameraView => false,
        }
    }

    fn set_direction(&mut self, direction: MovementDirection, pressed: bool) {
        match direction {
            MovementDirection::Forward => self.forward = pressed,
            MovementDirection::Backward => self.backward = pressed,
            MovementDirection::Left => self.left = pressed,
            MovementDirection::Right => self.right = pressed,
        }
    }

    fn set_turn_direction(&mut self, direction: KeyboardTurnDirection, pressed: bool) {
        match direction {
            KeyboardTurnDirection::Left => self.turn_left = pressed,
            KeyboardTurnDirection::Right => self.turn_right = pressed,
        }
    }

    fn directions(self) -> impl Iterator<Item = MovementDirection> {
        [
            (self.forward, MovementDirection::Forward),
            (self.backward, MovementDirection::Backward),
            (self.left, MovementDirection::Left),
            (self.right, MovementDirection::Right),
        ]
        .into_iter()
        .filter_map(|(pressed, direction)| pressed.then_some(direction))
    }

    fn turn_directions(self) -> impl Iterator<Item = KeyboardTurnDirection> {
        [
            (self.turn_left, KeyboardTurnDirection::Left),
            (self.turn_right, KeyboardTurnDirection::Right),
        ]
        .into_iter()
        .filter_map(|(pressed, direction)| pressed.then_some(direction))
    }

    fn actions(self) -> impl Iterator<Item = FlatInputAction> {
        [
            (self.jump, FlatInputAction::Jump),
            (self.descend, FlatInputAction::Descend),
            (self.sneak, FlatInputAction::Sneak),
            (self.sprint, FlatInputAction::Sprint),
        ]
        .into_iter()
        .filter_map(|(pressed, action)| pressed.then_some(action))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchBindings {
    pub bindings: Vec<TouchBinding>,
}

impl Default for TouchBindings {
    fn default() -> Self {
        let mut bindings = vec![
            TouchBinding::new(TouchControl::MovementStick, InputBindingAction::MoveAnalog),
            TouchBinding::new(TouchControl::LookDrag, InputBindingAction::Look),
            TouchBinding::new(TouchControl::JumpButton, InputBindingAction::Jump),
            TouchBinding::new(TouchControl::SprintButton, InputBindingAction::Sprint),
            TouchBinding::new(TouchControl::SneakButton, InputBindingAction::Sneak),
            TouchBinding::new(TouchControl::DescendButton, InputBindingAction::Descend),
            TouchBinding::new(TouchControl::AttackButton, InputBindingAction::Attack),
            TouchBinding::new(TouchControl::UseButton, InputBindingAction::Use),
            TouchBinding::new(TouchControl::MenuButton, InputBindingAction::OpenMenu),
        ];
        for slot in 0..FLAT_HOTBAR_SLOT_COUNT {
            bindings.push(TouchBinding::new(
                TouchControl::HotbarSlot(slot),
                InputBindingAction::SelectHotbarSlot(slot),
            ));
        }
        Self { bindings }
    }
}

impl TouchBindings {
    pub fn action_for(&self, control: TouchControl) -> Option<InputBindingAction> {
        self.bindings
            .iter()
            .find_map(|binding| (binding.control == control).then_some(binding.action))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchBinding {
    pub control: TouchControl,
    pub action: InputBindingAction,
}

impl TouchBinding {
    pub const fn new(control: TouchControl, action: InputBindingAction) -> Self {
        Self { control, action }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TouchControl {
    MovementStick,
    LookDrag,
    JumpButton,
    SprintButton,
    SneakButton,
    DescendButton,
    AttackButton,
    UseButton,
    HotbarSlot(u8),
    MenuButton,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchInputSettings {
    pub look_sensitivity: f32,
    pub movement_stick_radius: f32,
}

impl TouchInputSettings {
    pub const DEFAULT_LOOK_SENSITIVITY: f32 = 2.4;
    pub const MIN_LOOK_SENSITIVITY: f32 = 0.5;
    pub const MAX_LOOK_SENSITIVITY: f32 = 5.0;
    pub const DEFAULT_MOVEMENT_STICK_RADIUS: f32 = 50.0;

    pub fn normalized(self) -> Self {
        let look_sensitivity = if self.look_sensitivity.is_finite() {
            self.look_sensitivity
                .clamp(Self::MIN_LOOK_SENSITIVITY, Self::MAX_LOOK_SENSITIVITY)
        } else {
            Self::DEFAULT_LOOK_SENSITIVITY
        };
        let movement_stick_radius =
            if self.movement_stick_radius.is_finite() && self.movement_stick_radius > 0.0 {
                self.movement_stick_radius
            } else {
                Self::DEFAULT_MOVEMENT_STICK_RADIUS
            };
        Self {
            look_sensitivity,
            movement_stick_radius,
        }
    }
}

impl Default for TouchInputSettings {
    fn default() -> Self {
        Self {
            look_sensitivity: Self::DEFAULT_LOOK_SENSITIVITY,
            movement_stick_radius: Self::DEFAULT_MOVEMENT_STICK_RADIUS,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchLookDelta {
    pub yaw_radians: f32,
    pub pitch_radians: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchInputEvent {
    pub handled: bool,
    pub frame: Option<FlatInputFrame>,
    pub look_delta: Option<TouchLookDelta>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchJoystickState {
    pub base: Vec2,
    pub thumb: Vec2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchInputOverlayState {
    pub menu_pressed: bool,
    pub movement: Option<TouchJoystickState>,
    pub jump_pressed: bool,
    pub sprint_pressed: bool,
    pub sneak_pressed: bool,
    pub descend_pressed: bool,
    pub attack_pressed: bool,
    pub use_pressed: bool,
    pub hotbar_pressed_slot: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActiveTouchContact {
    id: u64,
    control: TouchControl,
    action: InputBindingAction,
    base: Vec2,
    position: Vec2,
    active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TouchInputAdapter {
    pub bindings: TouchBindings,
    pub settings: TouchInputSettings,
    viewport_size: Vec2,
    contacts: Vec<ActiveTouchContact>,
}

impl Default for TouchInputAdapter {
    fn default() -> Self {
        Self {
            bindings: TouchBindings::default(),
            settings: TouchInputSettings::default(),
            viewport_size: Vec2::ONE,
            contacts: Vec::new(),
        }
    }
}

impl TouchInputAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_settings(settings: TouchInputSettings) -> Self {
        Self {
            settings: settings.normalized(),
            ..Self::default()
        }
    }

    pub fn set_viewport_size(&mut self, size: Vec2) {
        self.viewport_size = Vec2::new(finite_positive(size.x), finite_positive(size.y));
    }

    pub fn set_look_sensitivity(&mut self, look_sensitivity: f32) {
        self.settings.look_sensitivity = TouchInputSettings {
            look_sensitivity,
            ..self.settings
        }
        .normalized()
        .look_sensitivity;
    }

    pub fn begin_contact(
        &mut self,
        id: u64,
        control: TouchControl,
        position: Vec2,
    ) -> TouchInputEvent {
        let Some(action) = self.bindings.action_for(control) else {
            return TouchInputEvent::default();
        };
        self.contacts.retain(|contact| contact.id != id);
        let position = finite_vec2(position);
        self.contacts.push(ActiveTouchContact {
            id,
            control,
            action,
            base: position,
            position,
            active: true,
        });
        let mut event = TouchInputEvent {
            handled: true,
            ..TouchInputEvent::default()
        };
        if is_one_shot_touch_action(action) {
            let mut frame = FlatInputFrame::default();
            frame.apply_binary_binding_action(action, true);
            event.frame = Some(frame);
        }
        event
    }

    pub fn move_contact(&mut self, id: u64, position: Vec2, active: bool) -> TouchInputEvent {
        let position = finite_vec2(position);
        let viewport_min = self.viewport_size.min_element().max(1.0);
        let sensitivity = self.settings.look_sensitivity;
        let Some(contact) = self.contacts.iter_mut().find(|contact| contact.id == id) else {
            return TouchInputEvent::default();
        };
        let delta = position - contact.position;
        contact.position = position;
        contact.active = active;
        let look_delta = (contact.action == InputBindingAction::Look
            && delta.length_squared() > 0.0)
            .then_some(TouchLookDelta {
                yaw_radians: -(delta.x / viewport_min) * sensitivity,
                pitch_radians: -(delta.y / viewport_min) * sensitivity,
            });
        TouchInputEvent {
            handled: true,
            frame: None,
            look_delta,
        }
    }

    pub fn end_contact(&mut self, id: u64, cancelled: bool) -> TouchInputEvent {
        let Some(index) = self.contacts.iter().position(|contact| contact.id == id) else {
            return TouchInputEvent::default();
        };
        let contact = self.contacts.remove(index);
        let frame = (!cancelled
            && contact.active
            && contact.action == InputBindingAction::OpenMenu)
            .then(|| {
                let mut frame = FlatInputFrame::default();
                frame.apply_binary_binding_action(contact.action, true);
                frame
            });
        TouchInputEvent {
            handled: true,
            frame,
            look_delta: None,
        }
    }

    pub fn clear(&mut self) {
        self.contacts.clear();
    }

    pub fn has_continuous_movement_input(&self) -> bool {
        self.contacts
            .iter()
            .any(|contact| is_continuous_touch_action(contact.action))
    }

    pub fn held_frame(&self) -> Option<FlatInputFrame> {
        let mut frame = FlatInputFrame::default();
        let mut has_input = false;
        for contact in &self.contacts {
            match contact.action {
                InputBindingAction::MoveAnalog => {
                    let radius = self.settings.movement_stick_radius;
                    let offset = (contact.position - contact.base).clamp_length_max(radius);
                    frame.set_analog_movement_impulse(-offset.x / radius, -offset.y / radius);
                    has_input = true;
                }
                action if is_continuous_touch_action(action) => {
                    frame.apply_binary_binding_action(action, true);
                    has_input = true;
                }
                _ => {}
            }
        }
        has_input.then_some(frame)
    }

    pub fn overlay_state(&self) -> TouchInputOverlayState {
        let mut overlay = TouchInputOverlayState::default();
        for contact in &self.contacts {
            match contact.control {
                TouchControl::MovementStick => {
                    let offset = (contact.position - contact.base)
                        .clamp_length_max(self.settings.movement_stick_radius);
                    overlay.movement = Some(TouchJoystickState {
                        base: contact.base,
                        thumb: contact.base + offset,
                    });
                }
                TouchControl::JumpButton => overlay.jump_pressed = true,
                TouchControl::SprintButton => overlay.sprint_pressed = true,
                TouchControl::SneakButton => overlay.sneak_pressed = true,
                TouchControl::DescendButton => overlay.descend_pressed = true,
                TouchControl::AttackButton => overlay.attack_pressed = true,
                TouchControl::UseButton => overlay.use_pressed = true,
                TouchControl::HotbarSlot(slot) => overlay.hotbar_pressed_slot = Some(slot),
                TouchControl::MenuButton => overlay.menu_pressed = contact.active,
                TouchControl::LookDrag => {}
            }
        }
        overlay
    }
}

fn finite_positive(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}

fn finite_vec2(value: Vec2) -> Vec2 {
    Vec2::new(
        if value.x.is_finite() { value.x } else { 0.0 },
        if value.y.is_finite() { value.y } else { 0.0 },
    )
}

fn is_continuous_touch_action(action: InputBindingAction) -> bool {
    matches!(
        action,
        InputBindingAction::MoveAnalog
            | InputBindingAction::Jump
            | InputBindingAction::Sprint
            | InputBindingAction::Sneak
            | InputBindingAction::Descend
    )
}

fn is_one_shot_touch_action(action: InputBindingAction) -> bool {
    matches!(
        action,
        InputBindingAction::Attack
            | InputBindingAction::Use
            | InputBindingAction::SelectHotbarSlot(_)
            | InputBindingAction::NextHotbarSlot
            | InputBindingAction::PreviousHotbarSlot
            | InputBindingAction::OpenBlockPalette
            | InputBindingAction::OpenHelp
            | InputBindingAction::ToggleCameraView
    )
}

/// Reserved shared gamepad contract (reviewed 2026-07-10 for tactical 168
/// Slice 10). It is intentionally retained because preferences, prompt
/// projection, remappable bindings, dead-zone policy, and HUD presentation
/// already share this vocabulary. No live platform adapter currently supplies
/// gamepad events or advertises the capability; adoption requires a real
/// desktop/browser/Android event source and device validation, not synthetic
/// enablement in one client lane.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadBindings {
    pub bindings: Vec<GamepadBinding>,
}

impl Default for GamepadBindings {
    fn default() -> Self {
        Self {
            bindings: vec![
                GamepadBinding::new(GamepadControl::LeftStick, InputBindingAction::MoveAnalog),
                GamepadBinding::new(GamepadControl::RightStick, InputBindingAction::Look),
                GamepadBinding::new(GamepadControl::SouthButton, InputBindingAction::Jump),
                GamepadBinding::new(GamepadControl::EastButton, InputBindingAction::Sneak),
                GamepadBinding::new(GamepadControl::NorthButton, InputBindingAction::OpenMenu),
                GamepadBinding::new(GamepadControl::RightTrigger, InputBindingAction::Attack),
                GamepadBinding::new(GamepadControl::LeftTrigger, InputBindingAction::Use),
                GamepadBinding::new(GamepadControl::LeftStickButton, InputBindingAction::Sprint),
                GamepadBinding::new(GamepadControl::StartButton, InputBindingAction::OpenMenu),
                GamepadBinding::new(
                    GamepadControl::SelectButton,
                    InputBindingAction::OpenBlockPalette,
                ),
                GamepadBinding::new(
                    GamepadControl::LeftShoulder,
                    InputBindingAction::PreviousHotbarSlot,
                ),
                GamepadBinding::new(
                    GamepadControl::RightShoulder,
                    InputBindingAction::NextHotbarSlot,
                ),
                GamepadBinding::new(
                    GamepadControl::DPadLeft,
                    InputBindingAction::PreviousHotbarSlot,
                ),
                GamepadBinding::new(
                    GamepadControl::DPadRight,
                    InputBindingAction::NextHotbarSlot,
                ),
                GamepadBinding::new(GamepadControl::DPadUp, InputBindingAction::Sprint),
                GamepadBinding::new(GamepadControl::DPadDown, InputBindingAction::Descend),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadBinding {
    pub control: GamepadControl,
    pub action: InputBindingAction,
}

impl GamepadBinding {
    pub const fn new(control: GamepadControl, action: InputBindingAction) -> Self {
        Self { control, action }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GamepadControl {
    LeftStick,
    RightStick,
    SouthButton,
    EastButton,
    WestButton,
    NorthButton,
    LeftShoulder,
    RightShoulder,
    DPadLeft,
    DPadRight,
    DPadUp,
    DPadDown,
    StartButton,
    SelectButton,
    LeftTrigger,
    RightTrigger,
    LeftStickButton,
    RightStickButton,
    GuideButton,
}

/// Maximum number of independently assigned humans/roles on one host.
///
/// This is a product bound, not a two-pane layout assumption. Presentation
/// may admit fewer or more views than assigned participants.
pub const MAX_LOCAL_PARTICIPANTS: usize = 4;

/// Default time during which a disconnected source keeps its participant
/// reservation. Hosts supply a session-monotonic timestamp, keeping platform
/// clocks and device handles out of shared assignment policy.
pub const DEFAULT_INPUT_RECONNECT_GRACE: Duration = Duration::from_secs(5);

/// Opaque identity allocated for one input-host session.
///
/// This type deliberately has no serialization implementation or constructor
/// from a backend number. Browser indices, GilRs IDs, Android device IDs, and
/// platform handles must remain in the collector that maps them to these IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InputSourceId(NonZeroU64);

/// Allocates monotonically ordered, process-local source identities.
#[derive(Clone, Debug)]
pub struct InputSourceIdAllocator {
    next: Option<NonZeroU64>,
}

impl Default for InputSourceIdAllocator {
    fn default() -> Self {
        Self {
            next: NonZeroU64::new(1),
        }
    }
}

impl InputSourceIdAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `None` only after exhausting every nonzero `u64` in a session.
    pub fn allocate(&mut self) -> Option<InputSourceId> {
        let id = self.next?;
        self.next = id.get().checked_add(1).and_then(NonZeroU64::new);
        Some(InputSourceId(id))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputSourceClass {
    KeyboardMouse,
    Touch,
    Gamepad,
    TrackedController,
    SteamInput,
    ScriptedTest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerLayoutFamily {
    XboxLike,
    PlayStationLike,
    NintendoLike,
    SteamDeckLike,
    Generic,
    Unknown,
}

/// Neutral source capabilities used by shared policy and presentation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputSourceCapabilities {
    pub standard_gamepad_axes: bool,
    pub standard_gamepad_buttons: bool,
    pub analog_button_values: bool,
    pub touch_surface_count: u8,
    pub motion_sensor_count: u8,
    pub haptic_channel_count: u8,
    pub action_origins: bool,
}

impl InputSourceCapabilities {
    pub const STANDARD_GAMEPAD: Self = Self {
        standard_gamepad_axes: true,
        standard_gamepad_buttons: true,
        analog_button_values: true,
        touch_surface_count: 0,
        motion_sensor_count: 0,
        haptic_channel_count: 0,
        action_origins: false,
    };
}

/// Shared source facts. The display label is session-only and must not become
/// a profile key or durable device identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputSourceDescriptor {
    pub source_class: InputSourceClass,
    pub controller_layout: ControllerLayoutFamily,
    pub capabilities: InputSourceCapabilities,
    pub connected: bool,
    pub display_label: Option<String>,
}

impl InputSourceDescriptor {
    pub fn standard_gamepad(
        controller_layout: ControllerLayoutFamily,
        display_label: Option<String>,
    ) -> Self {
        Self {
            source_class: InputSourceClass::Gamepad,
            controller_layout,
            capabilities: InputSourceCapabilities::STANDARD_GAMEPAD,
            connected: true,
            display_label,
        }
    }

    pub fn scripted_gamepad(display_label: impl Into<String>) -> Self {
        Self {
            source_class: InputSourceClass::ScriptedTest,
            controller_layout: ControllerLayoutFamily::Generic,
            capabilities: InputSourceCapabilities::STANDARD_GAMEPAD,
            connected: true,
            display_label: Some(display_label.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StandardGamepadButtonState {
    pub value: f32,
    pub pressed: bool,
}

impl StandardGamepadButtonState {
    pub const RELEASED: Self = Self {
        value: 0.0,
        pressed: false,
    };

    pub const fn pressed() -> Self {
        Self {
            value: 1.0,
            pressed: true,
        }
    }

    fn normalized(self) -> Self {
        Self {
            value: if self.value.is_finite() {
                self.value.clamp(0.0, 1.0)
            } else {
                0.0
            },
            pressed: self.pressed,
        }
    }
}

/// W3C-style standard gamepad buttons, named by position rather than vendor
/// glyph. Triggers retain both normalized analog values and pressed state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StandardGamepadButtons {
    pub south: StandardGamepadButtonState,
    pub east: StandardGamepadButtonState,
    pub west: StandardGamepadButtonState,
    pub north: StandardGamepadButtonState,
    pub left_shoulder: StandardGamepadButtonState,
    pub right_shoulder: StandardGamepadButtonState,
    pub left_trigger: StandardGamepadButtonState,
    pub right_trigger: StandardGamepadButtonState,
    pub select: StandardGamepadButtonState,
    pub start: StandardGamepadButtonState,
    pub left_stick: StandardGamepadButtonState,
    pub right_stick: StandardGamepadButtonState,
    pub dpad_up: StandardGamepadButtonState,
    pub dpad_down: StandardGamepadButtonState,
    pub dpad_left: StandardGamepadButtonState,
    pub dpad_right: StandardGamepadButtonState,
    pub guide: StandardGamepadButtonState,
}

impl StandardGamepadButtons {
    fn normalized(self) -> Self {
        Self {
            south: self.south.normalized(),
            east: self.east.normalized(),
            west: self.west.normalized(),
            north: self.north.normalized(),
            left_shoulder: self.left_shoulder.normalized(),
            right_shoulder: self.right_shoulder.normalized(),
            left_trigger: self.left_trigger.normalized(),
            right_trigger: self.right_trigger.normalized(),
            select: self.select.normalized(),
            start: self.start.normalized(),
            left_stick: self.left_stick.normalized(),
            right_stick: self.right_stick.normalized(),
            dpad_up: self.dpad_up.normalized(),
            dpad_down: self.dpad_down.normalized(),
            dpad_left: self.dpad_left.normalized(),
            dpad_right: self.dpad_right.normalized(),
            guide: self.guide.normalized(),
        }
    }

    fn mapped_controls(self) -> [(GamepadControl, bool); 17] {
        [
            (GamepadControl::SouthButton, self.south.pressed),
            (GamepadControl::EastButton, self.east.pressed),
            (GamepadControl::WestButton, self.west.pressed),
            (GamepadControl::NorthButton, self.north.pressed),
            (GamepadControl::LeftShoulder, self.left_shoulder.pressed),
            (GamepadControl::RightShoulder, self.right_shoulder.pressed),
            (GamepadControl::LeftTrigger, self.left_trigger.pressed),
            (GamepadControl::RightTrigger, self.right_trigger.pressed),
            (GamepadControl::SelectButton, self.select.pressed),
            (GamepadControl::StartButton, self.start.pressed),
            (GamepadControl::LeftStickButton, self.left_stick.pressed),
            (GamepadControl::RightStickButton, self.right_stick.pressed),
            (GamepadControl::DPadLeft, self.dpad_left.pressed),
            (GamepadControl::DPadRight, self.dpad_right.pressed),
            (GamepadControl::DPadUp, self.dpad_up.pressed),
            (GamepadControl::DPadDown, self.dpad_down.pressed),
            (GamepadControl::GuideButton, self.guide.pressed),
        ]
    }
}

/// One normalized ordinary-gamepad sample at the shared input boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StandardGamepadSnapshot {
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub buttons: StandardGamepadButtons,
}

impl StandardGamepadSnapshot {
    pub fn normalized(self) -> Self {
        Self {
            left_stick: normalize_standard_stick(self.left_stick),
            right_stick: normalize_standard_stick(self.right_stick),
            buttons: self.buttons.normalized(),
        }
    }

    /// A deliberate Start or position-neutral confirm edge may request a seat.
    pub fn join_held(self) -> bool {
        self.buttons.start.pressed || self.buttons.south.pressed
    }

    pub fn has_held_state(self) -> bool {
        self.left_stick != Vec2::ZERO
            || self.right_stick != Vec2::ZERO
            || self
                .buttons
                .mapped_controls()
                .into_iter()
                .any(|(_, pressed)| pressed)
            || self.buttons.left_trigger.pressed
            || self.buttons.right_trigger.pressed
            || self.buttons.select.pressed
            || self.buttons.left_stick.pressed
            || self.buttons.right_stick.pressed
            || self.buttons.guide.pressed
    }
}

fn normalize_standard_stick(value: Vec2) -> Vec2 {
    Vec2::new(finite_axis(value.x), finite_axis(value.y)).clamp_length_max(1.0)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalParticipantSlot(u8);

impl LocalParticipantSlot {
    pub fn from_index(index: usize) -> Option<Self> {
        (index < MAX_LOCAL_PARTICIPANTS).then_some(Self(index as u8))
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceConnectOutcome {
    Connected,
    Reconnected { slot: Option<LocalParticipantSlot> },
    AlreadyConnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceJoinOutcome {
    None,
    Assigned(LocalParticipantSlot),
    AlreadyAssigned(LocalParticipantSlot),
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GamepadSourceSampleReceipt {
    pub source_id: InputSourceId,
    pub join_edge: bool,
    pub join_outcome: SourceJoinOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceDisconnectReceipt {
    pub source_id: InputSourceId,
    pub reserved_slot: Option<LocalParticipantSlot>,
    pub held_state_cleared: bool,
    pub reconnect_until: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceReservationExpiry {
    pub source_id: InputSourceId,
    pub released_slot: Option<LocalParticipantSlot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalInputAssignmentError {
    UnknownSource(InputSourceId),
    DisconnectedSource(InputSourceId),
    DuplicateSample(InputSourceId),
}

#[derive(Clone, Debug)]
struct LocalGamepadSourceState {
    descriptor: InputSourceDescriptor,
    snapshot: StandardGamepadSnapshot,
    reconnect_until: Option<Duration>,
}

/// Bounded, deterministic source-to-seat reducer used by scripted tests now
/// and by future physical collectors without moving join policy into a host.
#[derive(Clone, Debug)]
pub struct LocalGamepadAssignmentReducer {
    reconnect_grace: Duration,
    sources: BTreeMap<InputSourceId, LocalGamepadSourceState>,
    assignments: [Option<InputSourceId>; MAX_LOCAL_PARTICIPANTS],
}

impl Default for LocalGamepadAssignmentReducer {
    fn default() -> Self {
        Self::new(DEFAULT_INPUT_RECONNECT_GRACE)
    }
}

impl LocalGamepadAssignmentReducer {
    pub fn new(reconnect_grace: Duration) -> Self {
        Self {
            reconnect_grace,
            sources: BTreeMap::new(),
            assignments: [None; MAX_LOCAL_PARTICIPANTS],
        }
    }

    pub fn connect_source(
        &mut self,
        source_id: InputSourceId,
        mut descriptor: InputSourceDescriptor,
        now: Duration,
    ) -> SourceConnectOutcome {
        self.expire_reconnect_grace(now);
        descriptor.connected = true;
        if self.sources.contains_key(&source_id) {
            let slot = self.slot_for_source(source_id);
            let source = self
                .sources
                .get_mut(&source_id)
                .expect("source presence was checked");
            if source.descriptor.connected {
                source.descriptor = descriptor;
                return SourceConnectOutcome::AlreadyConnected;
            }
            source.descriptor = descriptor;
            source.snapshot = StandardGamepadSnapshot::default();
            source.reconnect_until = None;
            return SourceConnectOutcome::Reconnected { slot };
        }
        self.sources.insert(
            source_id,
            LocalGamepadSourceState {
                descriptor,
                snapshot: StandardGamepadSnapshot::default(),
                reconnect_until: None,
            },
        );
        SourceConnectOutcome::Connected
    }

    /// Applies one logical sample boundary. Sorting by session-local source ID
    /// makes simultaneous joins deterministic even if a backend enumerates its
    /// devices in a different order on the next frame.
    pub fn sample_frame(
        &mut self,
        samples: impl IntoIterator<Item = (InputSourceId, StandardGamepadSnapshot)>,
    ) -> Result<Vec<GamepadSourceSampleReceipt>, LocalInputAssignmentError> {
        let mut samples = samples
            .into_iter()
            .map(|(source_id, snapshot)| (source_id, snapshot.normalized()))
            .collect::<Vec<_>>();
        samples.sort_by_key(|(source_id, _)| *source_id);

        for pair in samples.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(LocalInputAssignmentError::DuplicateSample(pair[0].0));
            }
        }
        for (source_id, _) in &samples {
            let Some(source) = self.sources.get(source_id) else {
                return Err(LocalInputAssignmentError::UnknownSource(*source_id));
            };
            if !source.descriptor.connected {
                return Err(LocalInputAssignmentError::DisconnectedSource(*source_id));
            }
        }

        let mut receipts = Vec::with_capacity(samples.len());
        for (source_id, snapshot) in samples {
            let source = self
                .sources
                .get_mut(&source_id)
                .expect("sample sources were validated");
            let join_edge = snapshot.join_held() && !source.snapshot.join_held();
            source.snapshot = snapshot;
            let join_outcome = if join_edge {
                self.assign_next_available(source_id)
            } else {
                SourceJoinOutcome::None
            };
            receipts.push(GamepadSourceSampleReceipt {
                source_id,
                join_edge,
                join_outcome,
            });
        }
        Ok(receipts)
    }

    pub fn disconnect_source(
        &mut self,
        source_id: InputSourceId,
        now: Duration,
    ) -> Result<SourceDisconnectReceipt, LocalInputAssignmentError> {
        let reserved_slot = self.slot_for_source(source_id);
        let Some(source) = self.sources.get_mut(&source_id) else {
            return Err(LocalInputAssignmentError::UnknownSource(source_id));
        };
        if !source.descriptor.connected {
            return Ok(SourceDisconnectReceipt {
                source_id,
                reserved_slot,
                held_state_cleared: false,
                reconnect_until: source
                    .reconnect_until
                    .expect("disconnected sources retain an expiry"),
            });
        }
        let held_state_cleared = source.snapshot.has_held_state();
        source.snapshot = StandardGamepadSnapshot::default();
        source.descriptor.connected = false;
        let reconnect_until = now.saturating_add(self.reconnect_grace);
        source.reconnect_until = Some(reconnect_until);
        Ok(SourceDisconnectReceipt {
            source_id,
            reserved_slot,
            held_state_cleared,
            reconnect_until,
        })
    }

    pub fn expire_reconnect_grace(&mut self, now: Duration) -> Vec<SourceReservationExpiry> {
        let expired = self
            .sources
            .iter()
            .filter_map(|(source_id, source)| {
                (!source.descriptor.connected
                    && source.reconnect_until.is_some_and(|until| now >= until))
                .then_some(*source_id)
            })
            .collect::<Vec<_>>();
        expired
            .into_iter()
            .map(|source_id| SourceReservationExpiry {
                source_id,
                released_slot: self.remove_source(source_id),
            })
            .collect()
    }

    pub fn forget_source(&mut self, source_id: InputSourceId) -> Option<LocalParticipantSlot> {
        self.remove_source(source_id)
    }

    pub fn source_descriptor(&self, source_id: InputSourceId) -> Option<&InputSourceDescriptor> {
        self.sources
            .get(&source_id)
            .map(|source| &source.descriptor)
    }

    pub fn source_snapshot(&self, source_id: InputSourceId) -> Option<StandardGamepadSnapshot> {
        self.sources.get(&source_id).map(|source| source.snapshot)
    }

    pub fn source_for_slot(&self, slot: LocalParticipantSlot) -> Option<InputSourceId> {
        self.assignments[slot.index()]
    }

    pub fn slot_for_source(&self, source_id: InputSourceId) -> Option<LocalParticipantSlot> {
        self.assignments
            .iter()
            .position(|assigned| *assigned == Some(source_id))
            .and_then(LocalParticipantSlot::from_index)
    }

    pub fn assigned_count(&self) -> usize {
        self.assignments.iter().flatten().count()
    }

    fn assign_next_available(&mut self, source_id: InputSourceId) -> SourceJoinOutcome {
        if let Some(slot) = self.slot_for_source(source_id) {
            return SourceJoinOutcome::AlreadyAssigned(slot);
        }
        let Some(index) = self.assignments.iter().position(Option::is_none) else {
            return SourceJoinOutcome::Full;
        };
        self.assignments[index] = Some(source_id);
        SourceJoinOutcome::Assigned(
            LocalParticipantSlot::from_index(index).expect("assignment array is product-bounded"),
        )
    }

    fn remove_source(&mut self, source_id: InputSourceId) -> Option<LocalParticipantSlot> {
        let slot = self.slot_for_source(source_id);
        if let Some(slot) = slot {
            self.assignments[slot.index()] = None;
        }
        self.sources.remove(&source_id);
        slot
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputBindingAction {
    Move(MovementDirection),
    Turn(KeyboardTurnDirection),
    MoveAnalog,
    Look,
    Jump,
    Sprint,
    Sneak,
    Descend,
    Attack,
    Use,
    SelectHotbarSlot(u8),
    NextHotbarSlot,
    PreviousHotbarSlot,
    OpenMenu,
    OpenBlockPalette,
    OpenHelp,
    ToggleCameraView,
}

impl InputBindingAction {
    pub fn label(self) -> String {
        match self {
            Self::Move(direction) => direction.label().to_owned(),
            Self::Turn(direction) => direction.label().to_owned(),
            Self::MoveAnalog => "Move".to_owned(),
            Self::Look => "Look".to_owned(),
            Self::Jump => "Jump".to_owned(),
            Self::Sprint => "Sprint".to_owned(),
            Self::Sneak => "Sneak".to_owned(),
            Self::Descend => "Descend".to_owned(),
            Self::Attack => "Break / Attack".to_owned(),
            Self::Use => "Use / Place".to_owned(),
            Self::SelectHotbarSlot(slot) => format!("Hotbar Slot {}", u16::from(slot) + 1),
            Self::NextHotbarSlot => "Next Hotbar Slot".to_owned(),
            Self::PreviousHotbarSlot => "Prev Hotbar Slot".to_owned(),
            Self::OpenMenu => "Open Menu".to_owned(),
            Self::OpenBlockPalette => "Open Block Palette".to_owned(),
            Self::OpenHelp => "Open Controls".to_owned(),
            Self::ToggleCameraView => "Toggle Camera View".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadInputSettings {
    pub movement_deadzone: f32,
    pub look_deadzone: f32,
    pub look_sensitivity: f32,
}

impl GamepadInputSettings {
    pub const DEFAULT_MOVEMENT_DEADZONE: f32 = 0.2;
    pub const DEFAULT_LOOK_DEADZONE: f32 = 0.18;
    pub const DEFAULT_LOOK_SENSITIVITY: f32 = 8.0;

    fn normalized(self) -> Self {
        Self {
            movement_deadzone: normalized_deadzone(self.movement_deadzone),
            look_deadzone: normalized_deadzone(self.look_deadzone),
            look_sensitivity: normalized_sensitivity(self.look_sensitivity),
        }
    }
}

impl Default for GamepadInputSettings {
    fn default() -> Self {
        Self {
            movement_deadzone: Self::DEFAULT_MOVEMENT_DEADZONE,
            look_deadzone: Self::DEFAULT_LOOK_DEADZONE,
            look_sensitivity: Self::DEFAULT_LOOK_SENSITIVITY,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadInputAdapter {
    pub bindings: GamepadBindings,
    pub settings: GamepadInputSettings,
    sticks: GamepadStickState,
    pressed: GamepadPressedControls,
}

impl Default for GamepadInputAdapter {
    fn default() -> Self {
        Self {
            bindings: GamepadBindings::default(),
            settings: GamepadInputSettings::default(),
            sticks: GamepadStickState::default(),
            pressed: GamepadPressedControls::default(),
        }
    }
}

impl GamepadInputAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bindings(bindings: GamepadBindings) -> Self {
        Self {
            bindings,
            ..Self::default()
        }
    }

    pub fn with_settings(settings: GamepadInputSettings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    pub fn clear_held(&mut self) {
        self.sticks = GamepadStickState::default();
        self.pressed = GamepadPressedControls::default();
    }

    /// Feeds the canonical snapshot through the existing shared bindings.
    /// Collectors should call this rather than translating gameplay actions.
    pub fn apply_standard_snapshot(
        &mut self,
        snapshot: StandardGamepadSnapshot,
    ) -> GamepadInputEvent {
        let snapshot = snapshot.normalized();
        self.set_left_stick(snapshot.left_stick.x, snapshot.left_stick.y);
        self.set_right_stick(snapshot.right_stick.x, snapshot.right_stick.y);
        let mut frame = FlatInputFrame::default();
        let mut emitted = false;
        for (control, pressed) in snapshot.buttons.mapped_controls() {
            if let Some(button_frame) = self.handle_button(control, pressed).frame {
                frame.merge_from(button_frame);
                emitted = true;
            }
        }
        if emitted {
            GamepadInputEvent::frame(frame)
        } else {
            GamepadInputEvent::handled()
        }
    }

    pub fn set_left_stick(&mut self, x: f32, y: f32) -> GamepadInputEvent {
        self.set_stick(GamepadControl::LeftStick, x, y)
    }

    pub fn set_right_stick(&mut self, x: f32, y: f32) -> GamepadInputEvent {
        self.set_stick(GamepadControl::RightStick, x, y)
    }

    pub fn handle_button(&mut self, control: GamepadControl, pressed: bool) -> GamepadInputEvent {
        if matches!(
            control,
            GamepadControl::LeftStick | GamepadControl::RightStick
        ) {
            return GamepadInputEvent::default();
        }
        let Some(action) = self.binding_action(control) else {
            return GamepadInputEvent::default();
        };
        let changed = self.pressed.set(control, pressed);
        if !pressed || !changed || is_continuous_binary_action(action) {
            return GamepadInputEvent::handled();
        }
        Self::binary_action_frame(action)
            .map(GamepadInputEvent::frame)
            .unwrap_or_else(GamepadInputEvent::handled)
    }

    pub fn has_continuous_input(&self) -> bool {
        self.sticks.left.is_some()
            || self.sticks.right.is_some()
            || self
                .pressed
                .controls
                .iter()
                .filter_map(|control| self.binding_action(*control))
                .any(is_continuous_binary_action)
    }

    pub fn held_frame(&self) -> Option<FlatInputFrame> {
        if !self.has_continuous_input() {
            return None;
        }
        let settings = self.settings.normalized();
        let mut frame = FlatInputFrame::default();
        let mut emitted = false;
        for binding in &self.bindings.bindings {
            match (binding.control, binding.action) {
                (GamepadControl::LeftStick, InputBindingAction::MoveAnalog) => {
                    if let Some(stick) = self.sticks.left.adjusted(settings.movement_deadzone) {
                        frame.apply_intent(FlatInputIntent::MoveAnalog {
                            left: -stick.x,
                            forward: stick.y,
                        });
                        emitted = true;
                    }
                }
                (GamepadControl::RightStick, InputBindingAction::Look) => {
                    if let Some(stick) = self.sticks.right.adjusted(settings.look_deadzone) {
                        frame.apply_intent(FlatInputIntent::LookDelta {
                            x: stick.x * settings.look_sensitivity,
                            y: -stick.y * settings.look_sensitivity,
                        });
                        emitted = true;
                    }
                }
                (control, action) if self.pressed.is_pressed(control) => {
                    if is_continuous_binary_action(action) {
                        frame.apply_binary_binding_action(action, true);
                        emitted = true;
                    }
                }
                _ => {}
            }
        }
        emitted.then_some(frame)
    }

    fn set_stick(&mut self, control: GamepadControl, x: f32, y: f32) -> GamepadInputEvent {
        let Some(action) = self.binding_action(control) else {
            return GamepadInputEvent::default();
        };
        match (control, action) {
            (GamepadControl::LeftStick, InputBindingAction::MoveAnalog) => {
                self.sticks.left = GamepadStick::new(x, y);
                GamepadInputEvent::handled()
            }
            (GamepadControl::RightStick, InputBindingAction::Look) => {
                self.sticks.right = GamepadStick::new(x, y);
                GamepadInputEvent::handled()
            }
            _ => GamepadInputEvent::default(),
        }
    }

    fn binding_action(&self, control: GamepadControl) -> Option<InputBindingAction> {
        self.bindings
            .bindings
            .iter()
            .find_map(|binding| (binding.control == control).then_some(binding.action))
    }

    fn binary_action_frame(action: InputBindingAction) -> Option<FlatInputFrame> {
        match action {
            InputBindingAction::Move(_)
            | InputBindingAction::Turn(_)
            | InputBindingAction::MoveAnalog
            | InputBindingAction::Look
            | InputBindingAction::Jump
            | InputBindingAction::Sprint
            | InputBindingAction::Sneak
            | InputBindingAction::Descend => None,
            _ => {
                let mut frame = FlatInputFrame::default();
                frame.apply_binary_binding_action(action, true);
                Some(frame)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadInputEvent {
    pub handled: bool,
    pub frame: Option<FlatInputFrame>,
}

impl GamepadInputEvent {
    pub const fn handled() -> Self {
        Self {
            handled: true,
            frame: None,
        }
    }

    pub const fn frame(frame: FlatInputFrame) -> Self {
        Self {
            handled: true,
            frame: Some(frame),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GamepadStickState {
    left: GamepadStick,
    right: GamepadStick,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GamepadStick {
    x: f32,
    y: f32,
}

impl GamepadStick {
    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn is_some(self) -> bool {
        self.x != 0.0 || self.y != 0.0
    }

    fn adjusted(self, deadzone: f32) -> Option<Self> {
        let x = clamp_axis(finite_axis(self.x));
        let y = clamp_axis(finite_axis(self.y));
        let magnitude = (x * x + y * y).sqrt().min(1.0);
        if magnitude <= deadzone || magnitude == 0.0 {
            return None;
        }
        let scaled = ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
        let scale = scaled / magnitude;
        Some(Self {
            x: x * scale,
            y: y * scale,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GamepadPressedControls {
    controls: Vec<GamepadControl>,
}

impl GamepadPressedControls {
    fn set(&mut self, control: GamepadControl, pressed: bool) -> bool {
        let index = self
            .controls
            .iter()
            .position(|pressed_control| *pressed_control == control);
        match (index, pressed) {
            (Some(_), true) | (None, false) => false,
            (None, true) => {
                self.controls.push(control);
                true
            }
            (Some(index), false) => {
                self.controls.swap_remove(index);
                true
            }
        }
    }

    fn is_pressed(&self, control: GamepadControl) -> bool {
        self.controls.contains(&control)
    }
}

fn is_continuous_binary_action(action: InputBindingAction) -> bool {
    matches!(
        action,
        InputBindingAction::Move(_)
            | InputBindingAction::Turn(_)
            | InputBindingAction::Jump
            | InputBindingAction::Sprint
            | InputBindingAction::Sneak
            | InputBindingAction::Descend
    )
}

fn normalized_deadzone(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 0.99)
    } else {
        0.0
    }
}

fn normalized_sensitivity(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        GamepadInputSettings::DEFAULT_LOOK_SENSITIVITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(capabilities: InputCapabilities) -> InputCapabilityState {
        InputCapabilityState::new(capabilities)
    }

    #[test]
    fn auto_touch_only_prefers_touch_and_shows_controls() {
        let resolved = state(InputCapabilities {
            touch: true,
            ..InputCapabilities::NONE
        })
        .resolve(InputPreferences::AUTO);

        assert_eq!(resolved.preferred_prompt, Some(InputPromptKind::Touch));
        assert!(resolved.touch_controls_visible);
        assert!(resolved.accepts_touch);
        assert!(!resolved.accepts_keyboard_mouse);
        assert!(!resolved.accepts_gamepad);
    }

    #[test]
    fn desktop_touchscreen_auto_tracks_last_active_device() {
        let mut input = state(InputCapabilities {
            keyboard: true,
            mouse: true,
            touch: true,
            ..InputCapabilities::NONE
        });

        let initial = input.resolve(InputPreferences::AUTO);
        assert_eq!(
            initial.preferred_prompt,
            Some(InputPromptKind::KeyboardMouse)
        );
        assert!(!initial.touch_controls_visible);

        input.note_activity(InputDeviceKind::Touch);
        let touch_active = input.resolve(InputPreferences::AUTO);
        assert_eq!(touch_active.preferred_prompt, Some(InputPromptKind::Touch));
        assert!(touch_active.touch_controls_visible);

        input.note_activity(InputDeviceKind::Mouse);
        let mouse_active = input.resolve(InputPreferences::AUTO);
        assert_eq!(
            mouse_active.preferred_prompt,
            Some(InputPromptKind::KeyboardMouse)
        );
        assert!(!mouse_active.touch_controls_visible);
        assert!(mouse_active.accepts_touch);
    }

    #[test]
    fn android_with_keyboard_mouse_keeps_touch_accepted() {
        let mut input = state(InputCapabilities {
            keyboard: true,
            mouse: true,
            touch: true,
            ..InputCapabilities::NONE
        });
        input.note_activity(InputDeviceKind::Keyboard);

        let resolved = input.resolve(InputPreferences::AUTO);
        assert_eq!(
            resolved.preferred_prompt,
            Some(InputPromptKind::KeyboardMouse)
        );
        assert!(resolved.accepts_keyboard_mouse);
        assert!(resolved.accepts_touch);
        assert!(!resolved.touch_controls_visible);

        input.note_activity(InputDeviceKind::Touch);
        assert!(input.resolve(InputPreferences::AUTO).touch_controls_visible);
    }

    #[test]
    fn explicit_preferences_change_presentation_not_accepted_inputs() {
        let input = state(InputCapabilities {
            keyboard: true,
            mouse: true,
            touch: true,
            gamepad: true,
            ..InputCapabilities::NONE
        });

        let keyboard_preferred = input.resolve(InputPreferences {
            preferred_scheme: InputSchemePreset::KeyboardMouse,
            touch_controls: TouchControlsMode::Auto,
        });
        assert_eq!(
            keyboard_preferred.preferred_prompt,
            Some(InputPromptKind::KeyboardMouse)
        );
        assert!(!keyboard_preferred.touch_controls_visible);
        assert!(keyboard_preferred.accepts_touch);
        assert!(keyboard_preferred.accepts_gamepad);

        let touch_forced = input.resolve(InputPreferences {
            preferred_scheme: InputSchemePreset::KeyboardMouse,
            touch_controls: TouchControlsMode::On,
        });
        assert!(touch_forced.touch_controls_visible);

        let touch_off = input.resolve(InputPreferences {
            preferred_scheme: InputSchemePreset::Touch,
            touch_controls: TouchControlsMode::Off,
        });
        assert_eq!(touch_off.preferred_prompt, Some(InputPromptKind::Touch));
        assert!(!touch_off.touch_controls_visible);
    }

    #[test]
    fn gamepad_preference_uses_gamepad_prompt_without_exclusive_input() {
        let input = state(InputCapabilities {
            keyboard: true,
            mouse: true,
            touch: true,
            gamepad: true,
            ..InputCapabilities::NONE
        });

        let resolved = input.resolve(InputPreferences {
            preferred_scheme: InputSchemePreset::Gamepad,
            touch_controls: TouchControlsMode::Auto,
        });

        assert_eq!(resolved.preferred_prompt, Some(InputPromptKind::Gamepad));
        assert!(resolved.accepts_keyboard_mouse);
        assert!(resolved.accepts_touch);
        assert!(resolved.accepts_gamepad);
        assert!(!resolved.touch_controls_visible);
    }

    #[test]
    fn removed_last_active_device_falls_back_to_remaining_capabilities() {
        let mut input = state(InputCapabilities {
            touch: true,
            gamepad: true,
            ..InputCapabilities::NONE
        });
        input.note_activity(InputDeviceKind::Gamepad);
        input.set_present(InputDeviceKind::Gamepad, false);

        let resolved = input.resolve(InputPreferences::AUTO);
        assert_eq!(resolved.preferred_prompt, Some(InputPromptKind::Touch));
        assert!(resolved.touch_controls_visible);
        assert!(!resolved.accepts_gamepad);
        assert!(resolved.accepts_touch);
    }

    #[test]
    fn frame_accumulates_movement_actions_hotbar_and_look() {
        let mut frame = FlatInputFrame::default();

        frame.apply_intent(FlatInputIntent::MoveDirection {
            direction: MovementDirection::Forward,
            pressed: true,
        });
        frame.apply_intent(FlatInputIntent::MoveDirection {
            direction: MovementDirection::Left,
            pressed: true,
        });
        frame.apply_intent(FlatInputIntent::Turn {
            direction: KeyboardTurnDirection::Left,
            pressed: true,
        });
        frame.apply_intent(FlatInputIntent::MoveAnalog {
            left: -0.25,
            forward: f32::INFINITY,
        });
        frame.apply_intent(FlatInputIntent::LookDelta { x: 3.5, y: -2.0 });
        frame.apply_intent(FlatInputIntent::Action {
            action: FlatInputAction::Attack,
            pressed: true,
        });
        frame.apply_intent(FlatInputIntent::Action {
            action: FlatInputAction::Use,
            pressed: false,
        });
        frame.apply_intent(FlatInputIntent::SelectHotbarSlot(4));
        frame.apply_intent(FlatInputIntent::StepHotbar(-1));

        assert_eq!(
            frame.movement,
            MovementImpulse {
                left: 0.75,
                forward: 1.0
            }
        );
        assert!(frame.forward);
        assert!(!frame.backward);
        assert!(frame.left);
        assert!(!frame.right);
        assert_eq!(
            frame.analog_movement,
            Some(MovementImpulse {
                left: -0.25,
                forward: 0.0
            })
        );
        assert_eq!(frame.keyboard_turn, 1.0);
        assert_eq!(frame.look_delta, LookDelta { x: 3.5, y: -2.0 });
        assert!(frame.attack);
        assert!(!frame.use_item);
        assert_eq!(frame.selected_hotbar_slot, Some(4));
        assert_eq!(frame.hotbar_step, -1);
    }

    #[test]
    fn frame_rejects_invalid_hotbar_slots() {
        let mut frame = FlatInputFrame::default();

        assert!(frame.select_hotbar_slot(FLAT_HOTBAR_SLOT_COUNT - 1));
        assert!(!frame.select_hotbar_slot(FLAT_HOTBAR_SLOT_COUNT));
        assert_eq!(frame.selected_hotbar_slot, Some(FLAT_HOTBAR_SLOT_COUNT - 1));
    }

    #[test]
    fn default_keyboard_mouse_bindings_match_current_desktop_controls() {
        let bindings = KeyboardMouseBindings::default();

        assert_eq!(KeyboardKey::from_hotbar_slot(0), Some(KeyboardKey::Digit1));
        assert_eq!(KeyboardKey::from_hotbar_slot(8), Some(KeyboardKey::Digit9));
        assert_eq!(KeyboardKey::from_hotbar_slot(9), None);
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::KeyW,
            InputBindingAction::Move(MovementDirection::Forward)
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::KeyS,
            InputBindingAction::Move(MovementDirection::Backward)
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::KeyA,
            InputBindingAction::Move(MovementDirection::Left)
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::KeyD,
            InputBindingAction::Move(MovementDirection::Right)
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::ArrowUp,
            InputBindingAction::Move(MovementDirection::Forward)
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::ArrowDown,
            InputBindingAction::Move(MovementDirection::Backward)
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::ArrowLeft,
            InputBindingAction::Turn(KeyboardTurnDirection::Left)
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::ArrowRight,
            InputBindingAction::Turn(KeyboardTurnDirection::Right)
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::KeyE,
            InputBindingAction::OpenBlockPalette
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::KeyB,
            InputBindingAction::OpenBlockPalette
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::F1,
            InputBindingAction::OpenHelp
        )));
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::F5,
            InputBindingAction::ToggleCameraView
        )));
        assert!(
            bindings
                .bindings
                .contains(&KeyboardMouseBinding::mouse_button(
                    PointerButton::Primary,
                    InputBindingAction::Attack
                ))
        );
        assert!(bindings.bindings.contains(&KeyboardMouseBinding::key(
            KeyboardKey::Digit9,
            InputBindingAction::SelectHotbarSlot(8)
        )));
    }

    #[test]
    fn default_keyboard_mouse_shortcut_rows_are_generated_from_bindings() {
        let bindings = KeyboardMouseBindings::default();
        let rows = bindings.shortcut_rows();

        assert_eq!(rows.len(), bindings.bindings.len());
        assert_eq!(default_keyboard_mouse_shortcut_rows(), rows);
        for (binding, row) in bindings.bindings.iter().zip(rows.iter()) {
            assert_eq!(row.group, ShortcutHelpGroup::KeyboardMouse);
            assert_eq!(row.control, binding.control.label());
            assert_eq!(row.action, binding.action.label());
            assert!(row.remappable);
            assert!(!row.control.is_empty());
            assert!(!row.action.is_empty());
        }
    }

    #[test]
    fn flat_runtime_shortcut_rows_cover_current_debug_keys() {
        let rows = flat_runtime_shortcut_rows();
        let controls = FLAT_RUNTIME_SHORTCUTS
            .iter()
            .map(|binding| binding.control)
            .collect::<Vec<_>>();

        assert_eq!(rows.len(), FLAT_RUNTIME_SHORTCUTS.len());
        assert!(controls.contains(&KeyboardMouseControl::Key(KeyboardKey::KeyN)));
        assert!(controls.contains(&KeyboardMouseControl::MouseWheelUp));
        assert!(controls.contains(&KeyboardMouseControl::MouseWheelDown));
        assert!(controls.contains(&KeyboardMouseControl::Key(KeyboardKey::Backquote)));
        assert!(controls.contains(&KeyboardMouseControl::Key(KeyboardKey::KeyO)));
        assert!(controls.contains(&KeyboardMouseControl::Key(KeyboardKey::KeyL)));
        assert!(controls.contains(&KeyboardMouseControl::Key(KeyboardKey::F7)));
        assert!(controls.contains(&KeyboardMouseControl::Key(KeyboardKey::F8)));
        assert!(controls.contains(&KeyboardMouseControl::Key(KeyboardKey::F9)));
        for row in rows {
            assert_eq!(row.group, ShortcutHelpGroup::RuntimeDebug);
            assert!(!row.remappable);
            assert!(!row.control.is_empty());
            assert!(!row.action.is_empty());
        }
    }

    #[test]
    fn keyboard_mouse_adapter_builds_shared_frames_from_default_bindings() {
        let mut adapter = KeyboardMouseInputAdapter::new();

        let w = adapter.handle_key(KeyboardKey::KeyW, true, false);
        assert!(w.handled);
        assert!(w.frame.is_none());
        adapter.handle_key(KeyboardKey::KeyA, true, false);
        adapter.handle_key(KeyboardKey::ArrowLeft, true, false);
        adapter.handle_key(KeyboardKey::Space, true, false);
        adapter.handle_key(KeyboardKey::ShiftLeft, true, false);

        let held = adapter
            .held_frame()
            .expect("held keys should produce frame");
        assert!(held.forward);
        assert!(held.left);
        assert_eq!(held.keyboard_turn, 1.0);
        assert!(held.jump);
        assert!(held.sneak);
        assert_eq!(
            held.movement,
            MovementImpulse {
                left: 1.0,
                forward: 1.0
            }
        );

        adapter.handle_key(KeyboardKey::KeyW, false, false);
        let held = adapter.held_frame().expect("left key should remain held");
        assert!(!held.forward);
        assert!(held.left);
        assert_eq!(held.keyboard_turn, 1.0);

        adapter.clear_held();
        assert!(adapter.held_frame().is_none());
    }

    #[test]
    fn xr_emulation_projects_keyboard_frames_to_controller_semantics() {
        let mut adapter = KeyboardMouseInputAdapter::new();
        adapter.handle_key(KeyboardKey::KeyW, true, false);
        adapter.handle_key(KeyboardKey::KeyA, true, false);
        adapter.handle_key(KeyboardKey::ArrowLeft, true, false);
        adapter.handle_key(KeyboardKey::Space, true, false);
        adapter.handle_key(KeyboardKey::ControlLeft, true, false);
        adapter.handle_key(KeyboardKey::ShiftLeft, true, false);
        let controllers = xr_emulation_controllers_from_flat_frame(
            adapter.held_frame().expect("held keyboard frame"),
        );

        assert_eq!(controllers[0].hand, XrHand::Left);
        assert_eq!(controllers[0].thumbstick, Vec2::new(-1.0, 1.0));
        assert!(controllers[0].y_pressed);
        assert_eq!(controllers[1].hand, XrHand::Right);
        assert_eq!(controllers[1].thumbstick, Vec2::X);
        assert!(controllers[1].a_pressed);
        assert!(controllers[1].thumbstick_pressed);
    }

    #[test]
    fn xr_emulation_projects_menu_and_world_actions() {
        let controllers = xr_emulation_controllers_from_flat_frame(FlatInputFrame {
            open_menu: true,
            open_block_palette: true,
            attack: true,
            use_item: true,
            descend: true,
            ..FlatInputFrame::default()
        });

        assert!(controllers[0].select_pressed);
        assert!(controllers[0].thumbstick_pressed);
        assert_eq!(controllers[1].trigger, 1.0);
        assert_eq!(controllers[1].squeeze, 1.0);
        assert!(controllers[1].b_pressed);
    }

    #[test]
    fn keyboard_mouse_adapter_emits_one_shot_frames() {
        let mut adapter = KeyboardMouseInputAdapter::new();

        let menu = adapter
            .handle_key(KeyboardKey::Escape, true, false)
            .frame
            .expect("escape should emit frame");
        assert!(menu.open_menu);

        let repeat = adapter.handle_key(KeyboardKey::Escape, true, true);
        assert!(repeat.handled);
        assert!(repeat.frame.is_none());

        let palette = adapter
            .handle_key(KeyboardKey::KeyE, true, false)
            .frame
            .expect("E should emit block palette frame");
        assert!(palette.open_block_palette);

        let repeat = adapter.handle_key(KeyboardKey::KeyE, true, true);
        assert!(repeat.handled);
        assert!(repeat.frame.is_none());

        let help = adapter
            .handle_key(KeyboardKey::F1, true, false)
            .frame
            .expect("F1 should emit help frame");
        assert!(help.open_help);

        let repeat = adapter.handle_key(KeyboardKey::F1, true, true);
        assert!(repeat.handled);
        assert!(repeat.frame.is_none());

        let view = adapter
            .handle_key(KeyboardKey::F5, true, false)
            .frame
            .expect("F5 should emit camera view frame");
        assert!(view.toggle_camera_view);

        let repeat = adapter.handle_key(KeyboardKey::F5, true, true);
        assert!(repeat.handled);
        assert!(repeat.frame.is_none());

        let slot = adapter
            .handle_key(KeyboardKey::Digit5, true, false)
            .frame
            .expect("digit should emit frame");
        assert_eq!(slot.selected_hotbar_slot, Some(4));

        let attack = adapter
            .handle_mouse_button(PointerButton::Primary, true)
            .frame
            .expect("primary button should emit frame");
        assert!(attack.attack);

        let use_item = adapter
            .handle_mouse_button(PointerButton::Secondary, true)
            .frame
            .expect("secondary button should emit frame");
        assert!(use_item.use_item);

        let previous = adapter
            .handle_mouse_wheel(MouseWheelDirection::Up)
            .frame
            .expect("wheel up should emit frame");
        assert_eq!(previous.hotbar_step, -1);

        let next = adapter
            .handle_mouse_wheel(MouseWheelDirection::Down)
            .frame
            .expect("wheel down should emit frame");
        assert_eq!(next.hotbar_step, 1);
    }

    #[test]
    fn keyboard_mouse_adapter_emits_look_frames() {
        let adapter = KeyboardMouseInputAdapter::new();

        let look = adapter
            .mouse_motion_frame(4.0, -2.0)
            .expect("finite mouse motion should emit frame");
        assert_eq!(look.look_delta, LookDelta { x: 4.0, y: -2.0 });
        assert!(adapter.mouse_motion_frame(0.0, 0.0).is_none());
        assert_eq!(
            adapter
                .mouse_motion_frame(f32::INFINITY, 3.0)
                .expect("finite y should still emit frame")
                .look_delta,
            LookDelta { x: 0.0, y: 3.0 }
        );
    }

    #[test]
    fn keyboard_code_names_are_shared_across_platform_adapters() {
        assert_eq!(KeyboardKey::from_code_name("KeyW"), Some(KeyboardKey::KeyW));
        assert_eq!(
            KeyboardKey::from_code_name("ArrowLeft"),
            Some(KeyboardKey::ArrowLeft)
        );
        assert_eq!(KeyboardKey::from_code_name("F5"), Some(KeyboardKey::F5));
        assert_eq!(KeyboardKey::from_code_name("Unidentified"), None);
    }

    #[test]
    fn flat_input_frames_merge_all_producer_intents() {
        let mut target = FlatInputFrame::default();
        target.add_movement_direction(MovementDirection::Forward);
        let mut source = FlatInputFrame::default();
        source.set_analog_movement_impulse(0.5, -0.25);
        source.press_action(FlatInputAction::Sneak);
        source.press_action(FlatInputAction::ToggleCameraView);
        source.add_look_delta(2.0, -1.0);
        source.step_hotbar(-1);

        target.merge_from(source);

        assert!(target.forward);
        assert!(target.sneak);
        assert!(target.toggle_camera_view);
        assert_eq!(
            target.analog_movement,
            Some(MovementImpulse {
                left: 0.5,
                forward: -0.25
            })
        );
        assert_eq!(target.look_delta, LookDelta { x: 2.0, y: -1.0 });
        assert_eq!(target.hotbar_step, -1);
    }

    #[test]
    fn touch_adapter_normalizes_look_and_clamps_movement_stick() {
        let mut adapter = TouchInputAdapter::with_settings(TouchInputSettings {
            look_sensitivity: f32::INFINITY,
            movement_stick_radius: 50.0,
        });
        assert_eq!(
            adapter.settings.look_sensitivity,
            TouchInputSettings::DEFAULT_LOOK_SENSITIVITY
        );
        adapter.set_viewport_size(Vec2::new(400.0, 200.0));
        adapter.begin_contact(1, TouchControl::LookDrag, Vec2::new(100.0, 100.0));
        let look = adapter
            .move_contact(1, Vec2::new(110.0, 95.0), true)
            .look_delta
            .expect("look drag should emit radians");
        assert!((look.yaw_radians + 0.12).abs() < 1.0e-6);
        assert!((look.pitch_radians - 0.06).abs() < 1.0e-6);

        adapter.begin_contact(2, TouchControl::MovementStick, Vec2::new(20.0, 30.0));
        adapter.move_contact(2, Vec2::new(120.0, 30.0), true);
        let held = adapter.held_frame().expect("movement touch should be held");
        assert_eq!(
            held.analog_movement,
            Some(MovementImpulse {
                left: -1.0,
                forward: 0.0
            })
        );
        let joystick = adapter.overlay_state().movement.expect("joystick overlay");
        assert_eq!(joystick.thumb, Vec2::new(70.0, 30.0));
    }

    #[test]
    fn touch_adapter_preserves_multitouch_and_edge_actions() {
        let mut adapter = TouchInputAdapter::new();
        assert!(
            adapter
                .begin_contact(1, TouchControl::JumpButton, Vec2::ZERO)
                .handled
        );
        adapter.begin_contact(2, TouchControl::SneakButton, Vec2::ZERO);
        let attack = adapter
            .begin_contact(3, TouchControl::AttackButton, Vec2::ZERO)
            .frame
            .expect("attack should emit on the press edge");
        assert!(attack.attack);
        let held = adapter
            .held_frame()
            .expect("held actions should produce a frame");
        assert!(held.jump);
        assert!(held.sneak);
        assert!(adapter.overlay_state().attack_pressed);

        adapter.begin_contact(4, TouchControl::MenuButton, Vec2::ZERO);
        assert!(adapter.move_contact(4, Vec2::ONE, false).handled);
        assert!(adapter.end_contact(4, false).frame.is_none());
        adapter.begin_contact(5, TouchControl::MenuButton, Vec2::ZERO);
        let menu = adapter
            .end_contact(5, false)
            .frame
            .expect("active menu release should emit the action");
        assert!(menu.open_menu);

        adapter.clear();
        assert!(adapter.held_frame().is_none());
    }

    #[test]
    fn gamepad_adapter_builds_shared_frames_from_sticks_and_held_buttons() {
        let mut adapter = GamepadInputAdapter::with_settings(GamepadInputSettings {
            movement_deadzone: 0.0,
            look_deadzone: 0.0,
            look_sensitivity: 8.0,
        });

        assert!(adapter.set_left_stick(0.5, 1.0).handled);
        assert!(adapter.set_right_stick(0.25, -0.5).handled);
        assert!(
            adapter
                .handle_button(GamepadControl::SouthButton, true)
                .handled
        );
        assert!(adapter.handle_button(GamepadControl::DPadUp, true).handled);

        let held = adapter
            .held_frame()
            .expect("sticks and held buttons should produce a frame");
        assert_eq!(
            held.analog_movement,
            Some(MovementImpulse {
                left: -0.5,
                forward: 1.0
            })
        );
        assert_eq!(
            held.movement,
            MovementImpulse {
                left: -0.5,
                forward: 1.0
            }
        );
        assert_eq!(held.look_delta, LookDelta { x: 2.0, y: 4.0 });
        assert!(held.jump);
        assert!(held.sprint);

        adapter.clear_held();
        assert!(adapter.held_frame().is_none());
    }

    #[test]
    fn gamepad_adapter_emits_one_shot_button_frames_on_press_edges() {
        let mut adapter = GamepadInputAdapter::new();

        let attack = adapter
            .handle_button(GamepadControl::RightTrigger, true)
            .frame
            .expect("right trigger should emit attack");
        assert!(attack.attack);
        assert!(
            adapter
                .handle_button(GamepadControl::RightTrigger, true)
                .frame
                .is_none()
        );
        assert!(
            adapter
                .handle_button(GamepadControl::RightTrigger, false)
                .handled
        );
        let attack_again = adapter
            .handle_button(GamepadControl::RightTrigger, true)
            .frame
            .expect("new press should emit attack again");
        assert!(attack_again.attack);

        let next = adapter
            .handle_button(GamepadControl::RightShoulder, true)
            .frame
            .expect("right shoulder should step hotbar");
        assert_eq!(next.hotbar_step, 1);

        let previous = adapter
            .handle_button(GamepadControl::DPadLeft, true)
            .frame
            .expect("d-pad left should step hotbar");
        assert_eq!(previous.hotbar_step, -1);

        let menu = adapter
            .handle_button(GamepadControl::StartButton, true)
            .frame
            .expect("start should open menu");
        assert!(menu.open_menu);
    }

    #[test]
    fn gamepad_adapter_filters_deadzone_and_non_finite_axes() {
        let mut adapter = GamepadInputAdapter::with_settings(GamepadInputSettings {
            movement_deadzone: 0.25,
            look_deadzone: 0.25,
            look_sensitivity: 8.0,
        });

        adapter.set_left_stick(0.1, 0.0);
        adapter.set_right_stick(f32::INFINITY, 0.1);
        assert!(adapter.held_frame().is_none());

        adapter.set_left_stick(0.625, 0.0);
        let held = adapter
            .held_frame()
            .expect("stick outside the deadzone should produce a frame");
        assert_eq!(
            held.analog_movement,
            Some(MovementImpulse {
                left: -0.5,
                forward: 0.0
            })
        );
    }

    #[test]
    fn standard_gamepad_snapshot_normalizes_and_feeds_shared_bindings() {
        let mut adapter = GamepadInputAdapter::with_settings(GamepadInputSettings {
            movement_deadzone: 0.0,
            look_deadzone: 0.0,
            look_sensitivity: 8.0,
        });
        let snapshot = StandardGamepadSnapshot {
            left_stick: Vec2::new(3.0, 4.0),
            right_stick: Vec2::new(f32::NAN, -0.5),
            buttons: StandardGamepadButtons {
                right_trigger: StandardGamepadButtonState::pressed(),
                left_trigger: StandardGamepadButtonState {
                    value: 2.0,
                    pressed: true,
                },
                ..StandardGamepadButtons::default()
            },
        };

        let normalized = snapshot.normalized();
        assert_eq!(normalized.left_stick, Vec2::new(0.6, 0.8));
        assert_eq!(normalized.right_stick, Vec2::new(0.0, -0.5));
        assert_eq!(normalized.buttons.left_trigger.value, 1.0);
        assert!(normalized.buttons.left_trigger.pressed);

        let event = adapter.apply_standard_snapshot(snapshot);
        assert!(event.frame.expect("west press should emit").attack);
        let held = adapter.held_frame().expect("sticks should remain held");
        assert_eq!(
            held.analog_movement,
            Some(MovementImpulse {
                left: -0.6,
                forward: 0.8,
            })
        );
        assert_eq!(held.look_delta, LookDelta { x: 0.0, y: 4.0 });
    }

    #[test]
    fn four_scripted_sources_assign_stably_without_backend_indices() {
        let mut allocator = InputSourceIdAllocator::new();
        let ids = std::array::from_fn::<_, 5, _>(|_| allocator.allocate().expect("test source ID"));
        let mut reducer = LocalGamepadAssignmentReducer::default();
        for (source_id, label) in ids.into_iter().zip([
            "backend-index-9",
            "backend-index-2",
            "backend-index-41",
            "backend-index-0",
            "backend-index-7",
        ]) {
            assert_eq!(
                reducer.connect_source(
                    source_id,
                    InputSourceDescriptor::scripted_gamepad(label),
                    Duration::ZERO,
                ),
                SourceConnectOutcome::Connected
            );
        }

        let join = StandardGamepadSnapshot {
            buttons: StandardGamepadButtons {
                start: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        let receipts = reducer
            .sample_frame([
                (ids[3], join),
                (ids[1], join),
                (ids[2], join),
                (ids[0], join),
            ])
            .expect("four joins");
        assert_eq!(
            receipts
                .iter()
                .map(|receipt| receipt.source_id)
                .collect::<Vec<_>>(),
            ids[..4]
        );
        for (index, source_id) in ids[..4].iter().copied().enumerate() {
            let slot = LocalParticipantSlot::from_index(index).expect("bounded slot");
            assert_eq!(reducer.source_for_slot(slot), Some(source_id));
            assert_eq!(reducer.slot_for_source(source_id), Some(slot));
        }
        assert_eq!(reducer.assigned_count(), MAX_LOCAL_PARTICIPANTS);

        let full = reducer
            .sample_frame([(ids[4], join)])
            .expect("fifth sample");
        assert_eq!(full[0].join_outcome, SourceJoinOutcome::Full);

        let release = StandardGamepadSnapshot::default();
        reducer
            .sample_frame([
                (ids[3], release),
                (ids[0], release),
                (ids[2], release),
                (ids[1], release),
                (ids[4], release),
            ])
            .expect("reordered releases");
        let repeated = reducer
            .sample_frame([
                (ids[2], join),
                (ids[0], join),
                (ids[3], join),
                (ids[1], join),
            ])
            .expect("reordered existing joins");
        assert!(
            repeated.iter().all(|receipt| matches!(
                receipt.join_outcome,
                SourceJoinOutcome::AlreadyAssigned(_)
            ))
        );
        assert_eq!(reducer.assigned_count(), MAX_LOCAL_PARTICIPANTS);
        for (index, source_id) in ids[..4].iter().copied().enumerate() {
            assert_eq!(
                reducer.source_for_slot(LocalParticipantSlot::from_index(index).unwrap()),
                Some(source_id)
            );
        }

        assert_eq!(
            reducer.sample_frame([(ids[0], release), (ids[0], join)]),
            Err(LocalInputAssignmentError::DuplicateSample(ids[0]))
        );
    }

    #[test]
    fn disconnect_clears_held_state_and_reconnect_reserves_only_until_grace() {
        let mut allocator = InputSourceIdAllocator::new();
        let reserved = allocator.allocate().unwrap();
        let waiting = allocator.allocate().unwrap();
        let mut reducer = LocalGamepadAssignmentReducer::new(Duration::from_secs(5));
        for source_id in [reserved, waiting] {
            reducer.connect_source(
                source_id,
                InputSourceDescriptor::scripted_gamepad("scripted pad"),
                Duration::ZERO,
            );
        }
        let held_join = StandardGamepadSnapshot {
            left_stick: Vec2::new(0.75, 0.0),
            buttons: StandardGamepadButtons {
                south: StandardGamepadButtonState::pressed(),
                ..StandardGamepadButtons::default()
            },
            ..StandardGamepadSnapshot::default()
        };
        reducer
            .sample_frame([(reserved, held_join)])
            .expect("initial join");
        let slot_zero = LocalParticipantSlot::from_index(0).unwrap();
        assert_eq!(reducer.source_for_slot(slot_zero), Some(reserved));

        let disconnected = reducer
            .disconnect_source(reserved, Duration::from_secs(10))
            .expect("disconnect");
        assert!(disconnected.held_state_cleared);
        assert_eq!(disconnected.reserved_slot, Some(slot_zero));
        assert_eq!(
            reducer.source_snapshot(reserved),
            Some(StandardGamepadSnapshot::default())
        );
        assert!(!reducer.source_descriptor(reserved).unwrap().connected);
        let repeated_disconnect = reducer
            .disconnect_source(reserved, Duration::from_secs(12))
            .expect("duplicate disconnect is idempotent");
        assert_eq!(repeated_disconnect.reconnect_until, Duration::from_secs(15));
        assert!(!repeated_disconnect.held_state_cleared);

        assert_eq!(
            reducer.connect_source(
                reserved,
                InputSourceDescriptor::scripted_gamepad("same session source"),
                Duration::from_secs(14),
            ),
            SourceConnectOutcome::Reconnected {
                slot: Some(slot_zero)
            }
        );
        assert_eq!(reducer.source_for_slot(slot_zero), Some(reserved));

        reducer
            .sample_frame([(reserved, held_join)])
            .expect("held state after reconnect starts from clear");
        reducer
            .disconnect_source(reserved, Duration::from_secs(20))
            .expect("second disconnect");
        let expired = reducer.expire_reconnect_grace(Duration::from_secs(25));
        assert_eq!(
            expired,
            vec![SourceReservationExpiry {
                source_id: reserved,
                released_slot: Some(slot_zero),
            }]
        );
        assert_eq!(reducer.source_for_slot(slot_zero), None);

        reducer
            .sample_frame([(waiting, StandardGamepadSnapshot::default())])
            .unwrap();
        let waiting_join = reducer.sample_frame([(waiting, held_join)]).unwrap();
        assert_eq!(
            waiting_join[0].join_outcome,
            SourceJoinOutcome::Assigned(slot_zero)
        );
        assert_eq!(
            reducer.connect_source(
                reserved,
                InputSourceDescriptor::scripted_gamepad("late source"),
                Duration::from_secs(26),
            ),
            SourceConnectOutcome::Connected
        );
        assert_eq!(reducer.slot_for_source(reserved), None);
        assert_eq!(reducer.source_for_slot(slot_zero), Some(waiting));
    }

    #[test]
    fn default_touch_and_gamepad_bindings_cover_flat_actions() {
        let touch = TouchBindings::default();
        assert!(touch.bindings.contains(&TouchBinding::new(
            TouchControl::MovementStick,
            InputBindingAction::MoveAnalog
        )));
        assert!(touch.bindings.contains(&TouchBinding::new(
            TouchControl::UseButton,
            InputBindingAction::Use
        )));
        assert!(touch.bindings.contains(&TouchBinding::new(
            TouchControl::SneakButton,
            InputBindingAction::Sneak
        )));
        assert!(touch.bindings.contains(&TouchBinding::new(
            TouchControl::HotbarSlot(8),
            InputBindingAction::SelectHotbarSlot(8)
        )));

        let gamepad = GamepadBindings::default();
        assert!(gamepad.bindings.contains(&GamepadBinding::new(
            GamepadControl::LeftStick,
            InputBindingAction::MoveAnalog
        )));
        assert!(gamepad.bindings.contains(&GamepadBinding::new(
            GamepadControl::RightStick,
            InputBindingAction::Look
        )));
        assert!(gamepad.bindings.contains(&GamepadBinding::new(
            GamepadControl::RightShoulder,
            InputBindingAction::NextHotbarSlot
        )));
    }
}
