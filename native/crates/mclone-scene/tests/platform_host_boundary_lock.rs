const DESKTOP_APP: &str = include_str!("../../../apps/mclone-native-client/src/app.rs");
const DESKTOP_DRIVER: &str =
    include_str!("../../../apps/mclone-native-client/src/winit_frame_driver.rs");
const FLAT_ANDROID: &str =
    include_str!("../../../apps/mclone-android-client/src/surface_driver.rs");

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
