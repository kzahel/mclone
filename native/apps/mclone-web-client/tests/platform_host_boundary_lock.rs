const WEB_SCENE_HOST: &str = include_str!("../src/web_scene_host.rs");
const WEB_APP: &str = include_str!("../www/mclone-web-app.ts");
const WEB_INPUT: &str = include_str!("../www/mclone-web-input.ts");
const WEB_SMOKE_OBSERVER: &str = include_str!("../www/mclone-web-smoke-observer.ts");
const WEB_TOUCH: &str = include_str!("../www/mclone-web-touch.ts");
const INPUT_PREFERENCES: &str =
    include_str!("../../../crates/mclone-app-runtime/src/input_preferences.rs");
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
    assert!(WEB_SMOKE_OBSERVER.contains("Object.assign(runtime.state, report)"));
    for forbidden in [
        "lastReport",
        "lastUiAction",
        "movementMode",
        "selectedHotbarSlot",
        "playerJumpStatistic",
        "currentTarget",
        "nativeUiScreen",
        "runtime.interactBlock =",
        "openNativeTitleUi",
        "handleNativeUiKey",
        "renderHalfSpaceTerrainProof",
    ] {
        assert!(
            !WEB_APP.contains(forbidden),
            "production browser adapter regained diagnostic semantics through {forbidden}"
        );
    }
    for removed_export in [
        "js_name = toggleMovementMode",
        "js_name = selectHotbarSlot",
        "js_name = uiStatus",
        "js_name = setTouchControlsOverlay",
        "js_name = setPauseMenu",
        "js_name = exerciseSettingsEffect",
        "js_name = exerciseBlockInteraction",
        "js_name = frameFirstActor",
        "js_name = simulateSurfaceLoss",
    ] {
        assert!(
            !WEB_SCENE_HOST.contains(removed_export),
            "unused direct diagnostic export survived: {removed_export}"
        );
    }
}

#[test]
fn browser_input_preferences_have_one_rust_policy_owner() {
    for required in [
        "pub trait PreferenceKeyValueStore",
        "ClientInputPreferences",
        "TOUCH_LOOK_SENSITIVITY_STORAGE_KEY",
        "TOUCH_CONTROLS_MODE_STORAGE_KEY",
        "TouchInputSettings::DEFAULT_LOOK_SENSITIVITY",
    ] {
        assert!(
            INPUT_PREFERENCES.contains(required),
            "shared Rust input preferences lost {required}"
        );
    }
    assert!(WEB_SCENE_HOST.contains("impl PreferenceKeyValueStore"));
    assert!(WEB_SCENE_HOST.contains("ClientInputPreferences::load"));
    assert!(WEB_SCENE_HOST.contains("persist_input_preferences_if_changed"));
    for (label, source) in [("app", WEB_APP), ("touch", WEB_TOUCH)] {
        for forbidden in [
            "mclone.web.lookSensitivity",
            "mclone.web.touchControlsMode",
            "DEFAULT_LOOK_SENSITIVITY",
            "clampLookSensitivity",
            "loadStoredSettings",
            "storeLookSensitivity",
            "storeTouchControlsMode",
            "TouchControlsMode",
            "setNativeTouchLookSensitivity",
            "setNativeTouchControlsMode",
        ] {
            assert!(
                !source.contains(forbidden),
                "production {label} TypeScript regained preference policy through {forbidden}"
            );
        }
    }
    assert!(WEB_TOUCH.contains("setTouchInputAvailable"));
}
