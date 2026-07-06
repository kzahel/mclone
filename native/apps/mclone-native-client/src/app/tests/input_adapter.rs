use super::*;

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
fn desktop_adapter_builds_shared_frame_from_held_keys() {
    let mut input = DesktopFlatInputAdapter::new();

    assert_eq!(
        input.handle_keyboard_input(KeyCode::KeyW, ElementState::Pressed, false),
        None
    );
    assert_eq!(
        input.handle_keyboard_input(KeyCode::KeyS, ElementState::Pressed, false),
        None
    );
    assert_eq!(
        input.handle_keyboard_input(KeyCode::Space, ElementState::Pressed, false),
        None
    );
    assert_eq!(
        input.handle_keyboard_input(KeyCode::ShiftLeft, ElementState::Pressed, false),
        None
    );
    assert_eq!(
        input.handle_keyboard_input(KeyCode::ArrowLeft, ElementState::Pressed, false),
        None
    );

    let frame = input.held_frame();
    assert!(frame.forward);
    assert!(frame.backward);
    assert!(!frame.left);
    assert!(!frame.right);
    assert_eq!(frame.keyboard_turn, 1.0);
    assert_eq!(frame.movement.forward, 0.0);
    assert!(frame.jump);
    assert!(frame.sneak);
    assert!(input.capability_state.capabilities.keyboard);

    let camera_input = engine_camera_input_from_flat_frame(frame, 0.016);
    assert_eq!(camera_input.dt_seconds, 0.016);
    assert!(camera_input.forward);
    assert!(camera_input.backward);
    assert!(camera_input.mouse_delta_x < 0.0);
    assert!(camera_input.jump);
    assert!(camera_input.shift);

    input.handle_keyboard_input(KeyCode::KeyW, ElementState::Released, false);
    let frame = input.held_frame();
    assert!(!frame.forward);
    assert!(frame.backward);
}

#[test]
fn desktop_adapter_emits_menu_and_hotbar_one_shots_without_repeat() {
    let mut input = DesktopFlatInputAdapter::new();

    let escape = input
        .handle_keyboard_input(KeyCode::Escape, ElementState::Pressed, false)
        .expect("escape should emit open-menu frame");
    assert!(escape.open_menu);

    let slot = input
        .handle_keyboard_input(KeyCode::Digit5, ElementState::Pressed, false)
        .expect("digit should emit hotbar frame");
    assert_eq!(slot.selected_hotbar_slot, Some(4));

    assert_eq!(
        input.handle_keyboard_input(KeyCode::Digit5, ElementState::Pressed, true),
        None
    );
}

#[test]
fn desktop_adapter_emits_mouse_action_and_look_frames() {
    let mut input = DesktopFlatInputAdapter::new();

    let attack = input
        .handle_mouse_button(MouseButton::Left, ElementState::Pressed)
        .expect("left click should emit attack frame");
    assert!(attack.attack);
    assert!(!attack.use_item);

    let use_item = input
        .handle_mouse_button(MouseButton::Right, ElementState::Pressed)
        .expect("right click should emit use frame");
    assert!(use_item.use_item);
    assert!(input.capability_state.capabilities.mouse);

    let look = input
        .mouse_look_frame(4.0, -2.0)
        .expect("finite mouse delta should emit look frame");
    assert_eq!(look.look_delta.x, 4.0);
    assert_eq!(look.look_delta.y, -2.0);
    assert!(input.mouse_look_frame(0.0, 0.0).is_none());
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
        EngineCameraMovementMode::NoClip
    );
    assert_eq!(
        EngineCameraMovementMode::NoClip.toggled(),
        EngineCameraMovementMode::HandPush
    );
    assert_eq!(
        EngineCameraMovementMode::HandPush.toggled(),
        EngineCameraMovementMode::Walking
    );
    assert_eq!(EngineCameraMovementMode::Walking.label(), "WALK");
    assert_eq!(EngineCameraMovementMode::NoClip.label(), "NOCLIP");
    assert_eq!(EngineCameraMovementMode::HandPush.label(), "HAND");
}
