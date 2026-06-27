use serde::{Deserialize, Serialize};

pub const FLAT_HOTBAR_SLOT_COUNT: u8 = 9;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FlatInputIntent {
    MoveDirection {
        direction: MovementDirection,
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
    pub look_delta: LookDelta,
    pub jump: bool,
    pub sprint: bool,
    pub sneak: bool,
    pub descend: bool,
    pub attack: bool,
    pub use_item: bool,
    pub open_menu: bool,
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
        }
    }
}

fn finite_axis(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn clamp_axis(value: f32) -> f32 {
    value.clamp(-1.0, 1.0)
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
            KeyboardMouseBinding::key(KeyboardKey::Space, InputBindingAction::Jump),
            KeyboardMouseBinding::key(KeyboardKey::KeyX, InputBindingAction::Descend),
            KeyboardMouseBinding::key(KeyboardKey::ShiftLeft, InputBindingAction::Sneak),
            KeyboardMouseBinding::key(KeyboardKey::ShiftRight, InputBindingAction::Sneak),
            KeyboardMouseBinding::key(KeyboardKey::ControlLeft, InputBindingAction::Sprint),
            KeyboardMouseBinding::key(KeyboardKey::ControlRight, InputBindingAction::Sprint),
            KeyboardMouseBinding::key(KeyboardKey::Escape, InputBindingAction::OpenMenu),
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardKey {
    KeyW,
    KeyA,
    KeyS,
    KeyD,
    KeyX,
    Space,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    Escape,
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
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
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
    DescendButton,
    AttackButton,
    UseButton,
    HotbarSlot(u8),
    MenuButton,
}

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
                GamepadBinding::new(GamepadControl::WestButton, InputBindingAction::Attack),
                GamepadBinding::new(GamepadControl::EastButton, InputBindingAction::Use),
                GamepadBinding::new(GamepadControl::NorthButton, InputBindingAction::OpenMenu),
                GamepadBinding::new(GamepadControl::StartButton, InputBindingAction::OpenMenu),
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
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputBindingAction {
    Move(MovementDirection),
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
