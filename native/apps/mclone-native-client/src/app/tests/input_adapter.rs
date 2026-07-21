use super::*;
use mclone_render_session::EngineCameraMovementMode;

#[test]
fn native_key_codes_map_to_shared_flat_input_controls() {
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::KeyW),
        Some(KeyboardKey::KeyW)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::KeyS),
        Some(KeyboardKey::KeyS)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::KeyA),
        Some(KeyboardKey::KeyA)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::KeyD),
        Some(KeyboardKey::KeyD)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::ArrowUp),
        Some(KeyboardKey::ArrowUp)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::ArrowDown),
        Some(KeyboardKey::ArrowDown)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::ArrowLeft),
        Some(KeyboardKey::ArrowLeft)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::ArrowRight),
        Some(KeyboardKey::ArrowRight)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::KeyE),
        Some(KeyboardKey::KeyE)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::KeyB),
        Some(KeyboardKey::KeyB)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::Space),
        Some(KeyboardKey::Space)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::KeyX),
        Some(KeyboardKey::KeyX)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::ShiftLeft),
        Some(KeyboardKey::ShiftLeft)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::ControlLeft),
        Some(KeyboardKey::ControlLeft)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::F1),
        Some(KeyboardKey::F1)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::F5),
        Some(KeyboardKey::F5)
    );
    assert_eq!(desktop_keyboard_key_from_key_code(NO_CLIP_TOGGLE_KEY), None);
    assert_eq!(
        desktop_keyboard_key_from_key_code(RENDER_RESOURCE_REBUILD_KEY),
        None
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(RENDER_SCALE_REBUILD_KEY),
        None
    );
    assert_eq!(desktop_keyboard_key_from_key_code(KeyCode::KeyO), None);
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::Digit1),
        Some(KeyboardKey::Digit1)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::Digit5),
        Some(KeyboardKey::Digit5)
    );
    assert_eq!(
        desktop_keyboard_key_from_key_code(KeyCode::Digit9),
        Some(KeyboardKey::Digit9)
    );
    assert_eq!(
        desktop_pointer_button_from_mouse_button(MouseButton::Left),
        Some(PointerButton::Primary)
    );
}

#[test]
fn desktop_adapter_normalizes_keyboard_facts_without_assigning_actions() {
    let mut input = DesktopFlatInputAdapter::new();

    assert_eq!(
        input.normalize_keyboard_input(KeyCode::KeyW, ElementState::Pressed, false),
        Some((KeyboardKey::KeyW, true, false))
    );
    assert_eq!(
        input.normalize_keyboard_input(KeyCode::Escape, ElementState::Pressed, false),
        Some((KeyboardKey::Escape, true, false))
    );
    assert_eq!(
        input.normalize_keyboard_input(KeyCode::Digit5, ElementState::Pressed, true),
        Some((KeyboardKey::Digit5, true, true))
    );
    assert_eq!(
        input.normalize_keyboard_input(KeyCode::KeyW, ElementState::Released, false),
        Some((KeyboardKey::KeyW, false, false))
    );
    assert!(input.capability_state.capabilities.keyboard);
}

#[test]
fn desktop_adapter_normalizes_pointer_motion_and_wheel_facts() {
    let mut input = DesktopFlatInputAdapter::new();

    assert_eq!(
        input.normalize_mouse_button(MouseButton::Left, ElementState::Pressed),
        Some((PointerButton::Primary, true))
    );
    assert_eq!(
        input.normalize_mouse_button(MouseButton::Right, ElementState::Released),
        Some((PointerButton::Secondary, false))
    );
    assert_eq!(input.normalize_mouse_motion(4.0, -2.0), (4.0, -2.0));
    assert_eq!(
        input.normalize_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0)),
        MouseWheelDirection::Up
    );
    assert_eq!(
        input.normalize_mouse_wheel(MouseScrollDelta::PixelDelta(
            winit::dpi::PhysicalPosition::new(0.0, -4.0)
        )),
        MouseWheelDirection::Down
    );
    assert!(input.capability_state.capabilities.mouse);
}

#[test]
fn desktop_touch_events_feed_shared_capability_resolution() {
    let mut input = DesktopFlatInputAdapter::new();

    input.note_touch_activity();
    let resolved = input
        .capability_state
        .resolve(mclone_input::InputPreferences::AUTO);
    assert_eq!(
        resolved.preferred_prompt,
        Some(mclone_input::InputPromptKind::Touch)
    );
    assert!(resolved.touch_controls_visible);
    assert!(resolved.accepts_touch);

    input.note_keyboard_activity();
    let resolved = input
        .capability_state
        .resolve(mclone_input::InputPreferences::AUTO);
    assert_eq!(
        resolved.preferred_prompt,
        Some(mclone_input::InputPromptKind::KeyboardMouse)
    );
    assert!(!resolved.touch_controls_visible);

    input.note_touch_activity();
    assert!(
        input
            .capability_state
            .resolve(mclone_input::InputPreferences::AUTO)
            .touch_controls_visible
    );
}

#[test]
fn player_movement_mode_cycles_through_shared_modes() {
    assert_eq!(
        EngineCameraMovementMode::Walking.toggled(),
        EngineCameraMovementMode::Fly
    );
    assert_eq!(
        EngineCameraMovementMode::Fly.toggled(),
        EngineCameraMovementMode::HandPush
    );
    assert_eq!(
        EngineCameraMovementMode::HandPush.toggled(),
        EngineCameraMovementMode::Thruster
    );
    assert_eq!(
        EngineCameraMovementMode::Thruster.toggled(),
        EngineCameraMovementMode::Walking
    );
    assert_eq!(EngineCameraMovementMode::Walking.label(), "WALK");
    assert_eq!(EngineCameraMovementMode::Fly.label(), "FLY");
    assert_eq!(EngineCameraMovementMode::HandPush.label(), "HAND");
    assert_eq!(EngineCameraMovementMode::Thruster.label(), "THRUST");
}
