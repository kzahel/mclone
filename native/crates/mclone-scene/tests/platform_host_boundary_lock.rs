const DESKTOP_APP: &str = include_str!("../../../apps/mclone-native-client/src/app.rs");
const DESKTOP_DRIVER: &str =
    include_str!("../../../apps/mclone-native-client/src/winit_frame_driver.rs");
const FLAT_ANDROID: &str =
    include_str!("../../../apps/mclone-android-client/src/surface_driver.rs");

fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source.find(marker).expect("item marker present");
    let body = &source[start..];
    let mut depth = 0usize;
    let mut opened = false;
    for (index, byte) in body.bytes().enumerate() {
        match byte {
            b'{' => {
                opened = true;
                depth += 1;
            }
            b'}' if opened => {
                depth -= 1;
                if depth == 0 {
                    return &body[..=index];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated braced item for {marker}")
}

#[test]
fn native_interactive_hosts_use_the_shared_router() {
    assert!(DESKTOP_DRIVER.contains("MonoInteractiveInputRouter"));
    assert!(FLAT_ANDROID.contains("MonoInteractiveInputRouter"));
    for method in [
        ".route_key(",
        ".route_pointer_button(",
        ".route_pointer_move(",
        ".route_mouse_motion(",
        ".route_wheel(",
        ".advance_held_frame(",
    ] {
        assert!(
            DESKTOP_DRIVER.contains(method) || FLAT_ANDROID.contains(method),
            "native adapters no longer exercise shared route {method}"
        );
    }
}

#[test]
fn desktop_gameplay_input_clear_preserves_ui_cursor_position() {
    let apply_host_effect_outcome = braced_item(DESKTOP_APP, "fn apply_host_effect_outcome(");
    assert!(
        apply_host_effect_outcome.contains("self.clear_flat_gameplay_input();"),
        "desktop host no longer clears held gameplay input when requested"
    );
    assert!(
        !apply_host_effect_outcome.contains("self.last_cursor = None;"),
        "desktop gameplay-input clear discarded the cursor position needed to route UI release"
    );
}

#[test]
fn native_apps_do_not_regain_final_frame_or_ui_dispatch() {
    for (label, source) in [
        ("desktop app", DESKTOP_APP),
        ("desktop frame driver", DESKTOP_DRIVER),
        ("flat Android", FLAT_ANDROID),
    ] {
        for forbidden in [
            "fn apply_flat_frame(",
            "fn apply_flat_keyboard_frame(",
            "fn apply_flat_world_action_frame(",
            "fn apply_flat_look_frame(",
            ".mono_ui_key_pressed(",
            ".mono_ui_pointer_down(",
            ".mono_ui_pointer_up(",
            ".mono_ui_pointer_move(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{label} regained shared input policy through {forbidden}"
            );
        }
    }
}
