const WEB_SCENE_HOST: &str = include_str!("../src/web_scene_host.rs");
const WEB_APP: &str = include_str!("../www/mclone-web-app.ts");
const WEB_INPUT: &str = include_str!("../www/mclone-web-input.ts");
const WEB_SMOKE_OBSERVER: &str = include_str!("../www/mclone-web-smoke-observer.ts");
const WEB_TOUCH: &str = include_str!("../www/mclone-web-touch.ts");
const FLAT_ANDROID: &str = include_str!("../../mclone-android-client/src/surface_driver.rs");

#[test]
fn browser_scene_host_owns_the_shared_raw_input_route() {
    for required in [
        "MonoInteractiveInputRouter",
        "TouchInputAdapter",
        "handleRawKey",
        "handleRawPointerButton",
        "handleRawPointerMove",
        "handleRawMouseMotion",
        "handleRawWheel",
        "handleRawTouch",
        ".advance_held_frame(",
    ] {
        assert!(
            WEB_SCENE_HOST.contains(required),
            "browser scene host no longer exercises {required}"
        );
    }
    assert!(WEB_APP.contains("return session.renderFrame(now);"));
    assert!(!WEB_APP.contains("mouseDeltaX"));
    assert!(!WEB_APP.contains("touchMovementImpulse"));
    assert!(!WEB_SCENE_HOST.contains("let input = FlatInputFrame"));
}

#[test]
fn production_typescript_input_modules_are_game_semantic_free() {
    for (label, source) in [("keyboard/mouse", WEB_INPUT), ("touch", WEB_TOUCH)] {
        for forbidden in [
            "\"forward\"",
            "\"backward\"",
            "\"turnLeft\"",
            "\"turnRight\"",
            "\"jump\"",
            "\"descend\"",
            "\"attack\"",
            "\"use\"",
            "\"break\"",
            "\"place\"",
            "selectHotbarSlot",
            "setInputKey",
            "setTouchKey",
            "interactBlock",
            "openNativePauseUi",
            "openNativeHelpUi",
            "handleNativeUiKey",
        ] {
            assert!(
                !source.contains(forbidden),
                "{label} TypeScript regained game semantics through {forbidden}"
            );
        }
    }
    for forbidden in [
        "TOUCH_JOYSTICK",
        "TOUCH_BUTTON",
        "TouchButtonKey",
        "touchButtonAt",
        "setTouchControlsOverlay",
    ] {
        assert!(
            !WEB_TOUCH.contains(forbidden),
            "touch TypeScript regained shared control policy through {forbidden}"
        );
    }
}

#[test]
fn flat_android_and_browser_share_touch_control_selection() {
    assert!(FLAT_ANDROID.contains("touch_control_at(scale, point)"));
    assert!(WEB_SCENE_HOST.contains("touch_control_at(scale, point)"));
    assert!(!FLAT_ANDROID.contains("fn touch_control_at("));
    assert!(!WEB_TOUCH.contains("function touchButtonRects("));
}

#[test]
fn semantic_smoke_registry_is_query_gated_outside_the_product_adapter() {
    assert!(!WEB_APP.contains("globalThis.__mcloneWebApp ="));
    assert!(!WEB_APP.contains("runtime.interactBlock ="));
    assert!(!WEB_APP.contains("runtime.openNativePauseUi ="));
    assert!(WEB_APP.contains("parameters.get(\"smokeObserver\") !== \"1\""));
    assert!(WEB_SMOKE_OBSERVER.contains("globalThis.__mcloneWebApp = runtime"));
    assert!(WEB_SMOKE_OBSERVER.contains("runtime.interactBlock ="));
    assert!(WEB_SMOKE_OBSERVER.contains("runtime.openNativePauseUi ="));
}
