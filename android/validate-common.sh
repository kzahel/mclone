#!/usr/bin/env bash

mclone_die() {
    echo "error: $*" >&2
    exit 1
}

mclone_note() {
    echo "==> $*"
}

mclone_configure_msys_adb_path_conversion() {
    case "$(uname -s 2>/dev/null || echo unknown)" in
        MINGW*|MSYS*|CYGWIN*)
            case "${MSYS2_ARG_CONV_EXCL:-}" in
                "" )
                    export MSYS2_ARG_CONV_EXCL="/sdcard"
                    ;;
                "*"|*"/sdcard"* )
                    ;;
                * )
                    export MSYS2_ARG_CONV_EXCL="/sdcard;$MSYS2_ARG_CONV_EXCL"
                    ;;
            esac
            ;;
    esac
}

mclone_configure_msys_adb_path_conversion

mclone_windows_local_android_sdk() {
    if [[ -n "${LOCALAPPDATA:-}" && "$(uname -s 2>/dev/null || true)" =~ MINGW|MSYS|CYGWIN ]] && command -v cygpath >/dev/null 2>&1; then
        cygpath -u "$LOCALAPPDATA/Android/Sdk"
        return
    fi
    return 1
}

mclone_android_sdk_home() {
    local windows_sdk

    if [[ -n "${ANDROID_HOME:-}" ]]; then
        echo "$ANDROID_HOME"
    elif [[ -n "${ANDROID_SDK_ROOT:-}" ]]; then
        echo "$ANDROID_SDK_ROOT"
    elif windows_sdk="$(mclone_windows_local_android_sdk 2>/dev/null)" && [[ -d "$windows_sdk" ]]; then
        echo "$windows_sdk"
    else
        echo "$HOME/Android/Sdk"
    fi
}

mclone_android_tool() {
    local name="$1"
    local relative_path="$2"
    local sdk_home

    if command -v "$name" >/dev/null 2>&1; then
        command -v "$name"
        return
    fi

    sdk_home="$(mclone_android_sdk_home)"
    if [[ -x "$sdk_home/$relative_path" ]]; then
        echo "$sdk_home/$relative_path"
        return
    fi

    mclone_die "could not find $name on PATH or at $sdk_home/$relative_path"
}

mclone_online_devices() {
    "$ADB" devices | tr -d '\r' | awk '$2 == "device" { print $1 }'
}

mclone_unauthorized_devices() {
    "$ADB" devices | tr -d '\r' | awk '$2 == "unauthorized" { print $1 }'
}

mclone_report_no_quest_found() {
    local unauthorized_devices

    "$ADB" devices
    unauthorized_devices="$(mclone_unauthorized_devices | paste -sd, -)"
    if [[ -n "$unauthorized_devices" ]]; then
        mclone_die "attached Android device(s) are unauthorized: $unauthorized_devices. Put on the headset, accept the USB debugging RSA prompt, and rerun adb devices."
    fi
    mclone_die "no attached Quest headset was found"
}

mclone_wait_for_boot() {
    local serial="$1"
    local timeout_seconds="$2"
    local deadline=$((SECONDS + timeout_seconds))
    local boot_completed
    local boot_anim

    mclone_note "Waiting for $serial to finish booting"
    while (( SECONDS < deadline )); do
        boot_completed="$("$ADB" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || true)"
        boot_anim="$("$ADB" -s "$serial" shell getprop init.svc.bootanim 2>/dev/null | tr -d '\r' || true)"
        if [[ "$boot_completed" == "1" && "$boot_anim" != "running" ]]; then
            return 0
        fi
        sleep 1
    done

    mclone_die "$serial did not finish booting within ${timeout_seconds}s"
}

mclone_wait_for_any_emulator() {
    local timeout_seconds="$1"
    local deadline=$((SECONDS + timeout_seconds))
    local serial

    while (( SECONDS < deadline )); do
        serial="$(mclone_online_devices | awk '/^emulator-/ { print; exit }')"
        if [[ -n "$serial" ]]; then
            echo "$serial"
            return 0
        fi
        sleep 1
    done

    mclone_die "no emulator became available within ${timeout_seconds}s"
}

mclone_device_summary() {
    local serial="$1"
    local manufacturer
    local model
    local api
    local abi

    manufacturer="$("$ADB" -s "$serial" shell getprop ro.product.manufacturer | tr -d '\r' || true)"
    model="$("$ADB" -s "$serial" shell getprop ro.product.model | tr -d '\r' || true)"
    api="$("$ADB" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r' || true)"
    abi="$("$ADB" -s "$serial" shell getprop ro.product.cpu.abi | tr -d '\r' || true)"
    echo "$serial ${manufacturer:-unknown} ${model:-unknown} API ${api:-unknown} ${abi:-unknown}"
}

mclone_quest_like_device() {
    local serial="$1"
    local manufacturer
    local model
    local features

    manufacturer="$("$ADB" -s "$serial" shell getprop ro.product.manufacturer | tr -d '\r' || true)"
    model="$("$ADB" -s "$serial" shell getprop ro.product.model | tr -d '\r' || true)"
    features="$("$ADB" -s "$serial" shell pm list features 2>/dev/null | tr -d '\r' || true)"

    if printf '%s\n%s\n' "$manufacturer" "$model" | grep -Ei "Oculus|Meta|Quest" >/dev/null; then
        return 0
    fi

    if printf '%s\n' "$features" | grep -E "oculus.hardware.standalone_vr|oculus.software.xrsp" >/dev/null; then
        return 0
    fi

    return 1
}

mclone_detect_quest_serial() {
    local serial

    for serial in $(mclone_online_devices | awk '!/^emulator-/ { print }'); do
        if mclone_quest_like_device "$serial"; then
            echo "$serial"
            return 0
        fi
    done

    return 1
}

MCLONE_UNSET_MARKER="__mclone_unset__"
MCLONE_HEADSET_SETTINGS_SAVED=0
MCLONE_PREVIOUS_STAY_ON=""
MCLONE_PREVIOUS_SKIP_LAUNCH_CHECK=""
MCLONE_PREVIOUS_REQUIRE_CONTROLLERS=""

mclone_read_android_setting() {
    local serial="$1"
    local namespace="$2"
    local name="$3"
    local value

    value="$("$ADB" -s "$serial" shell settings get "$namespace" "$name" 2>/dev/null | tr -d '\r' || true)"
    if [[ -z "$value" || "$value" == "null" ]]; then
        echo "$MCLONE_UNSET_MARKER"
    else
        echo "$value"
    fi
}

mclone_restore_android_setting() {
    local serial="$1"
    local namespace="$2"
    local name="$3"
    local value="$4"

    if [[ "$value" == "$MCLONE_UNSET_MARKER" ]]; then
        "$ADB" -s "$serial" shell settings delete "$namespace" "$name" >/dev/null 2>&1 || true
    else
        "$ADB" -s "$serial" shell settings put "$namespace" "$name" "$value" >/dev/null 2>&1 || true
    fi
}

mclone_save_headset_power_settings() {
    local serial="$1"

    MCLONE_PREVIOUS_STAY_ON="$(mclone_read_android_setting "$serial" global stay_on_while_plugged_in)"
    MCLONE_PREVIOUS_SKIP_LAUNCH_CHECK="$(mclone_read_android_setting "$serial" secure skip_launch_check_requires_controllers_enabled)"
    MCLONE_PREVIOUS_REQUIRE_CONTROLLERS="$(mclone_read_android_setting "$serial" global require_controllers_for_vr_apps)"
    MCLONE_HEADSET_SETTINGS_SAVED=1
}

mclone_disable_headset_proximity_sensor() {
    local serial="$1"

    "$ADB" -s "$serial" shell setprop debug.oculus.disableProximity 1 >/dev/null 2>&1 || true
}

mclone_enable_headset_proximity_sensor() {
    local serial="$1"

    "$ADB" -s "$serial" shell setprop debug.oculus.disableProximity 0 >/dev/null 2>&1 || true
    "$ADB" -s "$serial" shell am broadcast -a com.oculus.vrpowermanager.prox_open --ei timeout 0 >/dev/null 2>&1 || true
}

mclone_wake_headset_for_test() {
    local serial="$1"

    if [[ "$MCLONE_HEADSET_SETTINGS_SAVED" != "1" ]]; then
        mclone_save_headset_power_settings "$serial"
    fi
    mclone_disable_headset_proximity_sensor "$serial"
    "$ADB" -s "$serial" shell settings put global stay_on_while_plugged_in 3 >/dev/null 2>&1 || true
    "$ADB" -s "$serial" shell settings put secure skip_launch_check_requires_controllers_enabled 1 >/dev/null 2>&1 || true
    "$ADB" -s "$serial" shell settings put global require_controllers_for_vr_apps 0 >/dev/null 2>&1 || true
    "$ADB" -s "$serial" shell input keyevent KEYCODE_WAKEUP >/dev/null 2>&1 || true
    "$ADB" -s "$serial" shell am broadcast -a com.oculus.vrpowermanager.prox_close --ei timeout 0 >/dev/null 2>&1 || true
}

mclone_dismiss_vr_system_dialogs() {
    local serial="$1"

    "$ADB" -s "$serial" shell input keyevent KEYCODE_BACK >/dev/null 2>&1 || true
    "$ADB" -s "$serial" shell input keyevent KEYCODE_ESCAPE >/dev/null 2>&1 || true
    sleep 1
    if "$ADB" -s "$serial" shell dumpsys activity activities 2>/dev/null \
        | grep -F "LaunchCheckControllerRequiredDialogActivity" >/dev/null 2>&1; then
        "$ADB" -s "$serial" shell am force-stop com.oculus.vrshell >/dev/null 2>&1 || true
        sleep 4
    fi
}

mclone_restore_headset_after_test() {
    local serial="$1"
    local package_name="${2:-}"

    if [[ -z "$serial" || "$MCLONE_HEADSET_SETTINGS_SAVED" != "1" ]]; then
        return
    fi

    if [[ -n "$package_name" ]]; then
        "$ADB" -s "$serial" shell am force-stop "$package_name" >/dev/null 2>&1 || true
    fi
    mclone_restore_android_setting "$serial" global stay_on_while_plugged_in "$MCLONE_PREVIOUS_STAY_ON"
    mclone_restore_android_setting "$serial" secure skip_launch_check_requires_controllers_enabled "$MCLONE_PREVIOUS_SKIP_LAUNCH_CHECK"
    mclone_restore_android_setting "$serial" global require_controllers_for_vr_apps "$MCLONE_PREVIOUS_REQUIRE_CONTROLLERS"
    mclone_enable_headset_proximity_sensor "$serial"
    "$ADB" -s "$serial" shell input keyevent KEYCODE_SLEEP >/dev/null 2>&1 || true
    "$ADB" -s "$serial" shell setprop debug.oculus.disableProximity 0 >/dev/null 2>&1 || true
}

mclone_build_apk() {
    if [[ "$SKIP_BUILD" == "1" ]]; then
        mclone_note "Skipping APK build"
    else
        mclone_note "Building APK"
        bash "$ANDROID_DIR/build-apk.sh"
    fi

    [[ -f "$APK_PATH" ]] || mclone_die "APK not found at $APK_PATH"
}

mclone_collect_logcat() {
    local serial="$1"
    local pid="$2"
    local log_path="$3"

    mkdir -p "$(dirname "$log_path")"
    if [[ -n "$pid" ]]; then
        "$ADB" -s "$serial" logcat -d "--pid=$pid" -t 400 > "$log_path" 2>/dev/null \
            || "$ADB" -s "$serial" logcat -d -t 400 > "$log_path" 2>/dev/null \
            || true
    else
        "$ADB" -s "$serial" logcat -d -t 400 > "$log_path" 2>/dev/null || true
    fi
}

mclone_capture_screenshot() {
    local serial="$1"
    local screenshot_path="$2"
    local remote_path="/sdcard/mclone-android-smoke.png"

    mkdir -p "$(dirname "$screenshot_path")"
    "$ADB" -s "$serial" shell screencap -p "$remote_path" >/dev/null
    "$ADB" -s "$serial" pull "$remote_path" "$screenshot_path" >/dev/null
    "$ADB" -s "$serial" shell rm -f "$remote_path" >/dev/null || true
    [[ -s "$screenshot_path" ]] || mclone_die "screenshot capture failed: $screenshot_path"
}

mclone_stage_asset_pack() {
    local serial="$1"
    local asset_pack_path="${MCLONE_ANDROID_ASSET_PACK:-$REPO_ROOT/reference/minecraft-1.17.1/extracted.zip}"
    local remote_dir="/sdcard/Android/data/$MCLONE_ANDROID_APP_ID/files/assets/packs"
    local remote_path="$remote_dir/extracted.zip"

    [[ -f "$asset_pack_path" ]] || {
        mclone_die "asset pack not found at $asset_pack_path; run pnpm assets:pack"
    }

    mclone_note "Staging asset pack $asset_pack_path to $remote_path"
    "$ADB" -s "$serial" shell mkdir -p "$remote_dir" >/dev/null
    "$ADB" -s "$serial" push "$asset_pack_path" "$remote_path" >/dev/null
    mclone_stage_local_sound_assets "$serial"
    mclone_repair_emulator_asset_permissions "$serial"
}

mclone_local_sound_asset_source() {
    if [[ -n "${MCLONE_ANDROID_SOUND_ASSET_ROOT:-}" ]]; then
        [[ -d "$MCLONE_ANDROID_SOUND_ASSET_ROOT/assets" ]] || {
            mclone_die "MCLONE_ANDROID_SOUND_ASSET_ROOT does not contain assets/: $MCLONE_ANDROID_SOUND_ASSET_ROOT"
        }
        printf '%s' "$MCLONE_ANDROID_SOUND_ASSET_ROOT"
        return 0
    fi

    if [[ -n "${MCLONE_SOUND_ASSET_ROOT:-}" ]]; then
        [[ -d "$MCLONE_SOUND_ASSET_ROOT/assets" ]] || {
            mclone_die "MCLONE_SOUND_ASSET_ROOT does not contain assets/: $MCLONE_SOUND_ASSET_ROOT"
        }
        printf '%s' "$MCLONE_SOUND_ASSET_ROOT"
        return 0
    fi

    local source="$REPO_ROOT/reference/minecraft-1.17.1/local-sounds"
    if [[ -d "$source/assets" ]]; then
        printf '%s' "$source"
        return 0
    fi

    source="$REPO_ROOT/reference/minecraft-1.17.1/sound-overlay"
    if [[ -d "$source/assets" ]]; then
        printf '%s' "$source"
        return 0
    fi

    return 1
}

mclone_stage_local_sound_assets() {
    local serial="$1"
    local source_dir
    local remote_dir="/sdcard/Android/data/$MCLONE_ANDROID_APP_ID/files/assets/local-sounds"

    if ! source_dir="$(mclone_local_sound_asset_source)"; then
        mclone_note "No local sound assets found; skipping Android sound staging"
        return 0
    fi

    mclone_note "Staging local sound assets $source_dir to $remote_dir"
    "$ADB" -s "$serial" shell rm -rf "$remote_dir" >/dev/null
    "$ADB" -s "$serial" shell mkdir -p "$remote_dir" >/dev/null
    mclone_push_asset_tree "$serial" "$source_dir" "$remote_dir"
}

mclone_push_asset_tree() {
    local serial="$1"
    local source_dir="$2"
    local remote_dir="$3"
    local source_prefix="$source_dir/"
    local local_dir
    local local_file
    local rel_path
    local remote_path

    while IFS= read -r -d '' local_dir; do
        rel_path="${local_dir#"$source_prefix"}"
        if [[ "$rel_path" == "$local_dir" ]]; then
            rel_path=""
        fi
        remote_path="$remote_dir"
        if [[ -n "$rel_path" ]]; then
            remote_path="$remote_path/$rel_path"
        fi
        "$ADB" -s "$serial" shell mkdir -p "$remote_path" </dev/null >/dev/null
    done < <(find "$source_dir" -type d -print0)

    while IFS= read -r -d '' local_file; do
        rel_path="${local_file#"$source_prefix"}"
        [[ "$rel_path" != "$local_file" ]] || mclone_die "failed to derive relative asset path for $local_file"
        "$ADB" -s "$serial" push "$local_file" "$remote_dir/$rel_path" </dev/null >/dev/null
    done < <(find "$source_dir" -type f -print0)
}

mclone_repair_emulator_asset_permissions() {
    local serial="$1"
    local remote_app_dir="/sdcard/Android/data/$MCLONE_ANDROID_APP_ID"
    local app_user
    local package_uid

    [[ "${MCLONE_ANDROID_REPAIR_ASSET_PERMS:-1}" == "1" ]] || return 0
    [[ "$serial" == emulator-* ]] || return 0

    if [[ "$("$ADB" -s "$serial" shell id -u 2>/dev/null | tr -d '\r' || true)" != "0" ]]; then
        mclone_note "Restarting emulator adbd as root for staged asset ownership repair"
        "$ADB" -s "$serial" root >/dev/null 2>&1 || {
            mclone_note "Emulator adbd root is unavailable; leaving staged asset ownership unchanged"
            return 0
        }
        "$ADB" -s "$serial" wait-for-device >/dev/null
    fi

    if [[ "$("$ADB" -s "$serial" shell id -u 2>/dev/null | tr -d '\r' || true)" != "0" ]]; then
        mclone_note "Emulator adbd did not become root; leaving staged asset ownership unchanged"
        return 0
    fi

    app_user="$("$ADB" -s "$serial" shell stat -c %U "/data/user/0/$MCLONE_ANDROID_APP_ID" 2>/dev/null | tr -d '\r' || true)"
    if [[ -z "$app_user" || "$app_user" == "UNKNOWN" ]]; then
        package_uid="$("$ADB" -s "$serial" shell cmd package list packages -U "$MCLONE_ANDROID_APP_ID" 2>/dev/null \
            | tr -d '\r' | sed -n 's/.* uid:\([0-9][0-9]*\).*/\1/p' | head -1)"
        app_user="$package_uid"
    fi

    if [[ -z "$app_user" ]]; then
        mclone_note "Could not resolve app owner for $MCLONE_ANDROID_APP_ID; leaving staged asset ownership unchanged"
        return 0
    fi

    mclone_note "Repairing staged asset ownership for $MCLONE_ANDROID_APP_ID on emulator"
    "$ADB" -s "$serial" shell chown -R "$app_user:ext_data_rw" "$remote_app_dir" >/dev/null 2>&1 \
        || "$ADB" -s "$serial" shell chown -R "$app_user:$app_user" "$remote_app_dir" >/dev/null 2>&1 \
        || true
    "$ADB" -s "$serial" shell chmod -R u+rwX,g+rwX "$remote_app_dir" >/dev/null 2>&1 || true
}

mclone_run_touch_swipe() {
    local serial="$1"
    local spec="${MCLONE_ANDROID_TOUCH_SWIPE:-}"
    local x1
    local y1
    local x2
    local y2
    local duration_ms
    local extra

    [[ -n "$spec" ]] || return 0
    IFS=, read -r x1 y1 x2 y2 duration_ms extra <<<"$spec"
    if [[ -z "${x1:-}" || -z "${y1:-}" || -z "${x2:-}" || -z "${y2:-}" || -z "${duration_ms:-}" || -n "${extra:-}" ]]; then
        mclone_die "invalid --touch-swipe '$spec'; expected x1,y1,x2,y2,duration_ms"
    fi

    mclone_note "Injecting Android touch swipe $spec"
    "$ADB" -s "$serial" shell input swipe "$x1" "$y1" "$x2" "$y2" "$duration_ms" >/dev/null
    sleep "${MCLONE_ANDROID_AFTER_TOUCH_SECONDS:-1}"
}

mclone_install_launch_smoke() {
    local serial="$1"
    local screenshot_path="$2"
    local log_path="$3"
    local smoke_seconds="$4"
    local launch_output
    local pid
    local focused_line
    local activity_block

    mclone_note "Using $(mclone_device_summary "$serial")"
    mclone_note "Installing $APK_PATH"
    "$ADB" -s "$serial" install -r "$APK_PATH"

    if [[ "${STAGE_ASSETS:-1}" == "1" ]]; then
        mclone_stage_asset_pack "$serial"
    else
        mclone_note "Skipping Android asset-pack staging"
    fi

    mclone_configure_remote_addr "$serial"

    "$ADB" -s "$serial" shell am force-stop "$MCLONE_ANDROID_APP_ID" >/dev/null 2>&1 || true
    "$ADB" -s "$serial" logcat -c || true

    mclone_note "Launching $MCLONE_ANDROID_APP_ID/$MCLONE_ANDROID_ACTIVITY"
    launch_output="$("$ADB" -s "$serial" shell am start -W -n "$MCLONE_ANDROID_APP_ID/$MCLONE_ANDROID_ACTIVITY" 2>&1 | tr -d '\r')"
    echo "$launch_output"
    if printf '%s\n' "$launch_output" | grep -F "Status: ok" >/dev/null; then
        :
    elif printf '%s\n' "$launch_output" | grep -F "Status: timeout" >/dev/null; then
        mclone_note "Launch reported Status: timeout; continuing with process/focus checks"
    else
        mclone_die "activity launch did not report Status: ok"
    fi

    sleep "$smoke_seconds"

    pid="$("$ADB" -s "$serial" shell pidof "$MCLONE_ANDROID_APP_ID" 2>/dev/null | tr -d '\r' || true)"
    if [[ -z "$pid" ]]; then
        mclone_collect_logcat "$serial" "" "$log_path"
        mclone_die "$MCLONE_ANDROID_APP_ID is not running after launch; logcat: $log_path"
    fi
    mclone_note "$MCLONE_ANDROID_APP_ID pid: $pid"

    if [[ "${MCLONE_ANDROID_REQUIRE_FOCUS:-1}" == "1" ]]; then
        focused_line="$("$ADB" -s "$serial" shell dumpsys window 2>/dev/null | tr -d '\r' \
            | grep -F "$MCLONE_ANDROID_APP_ID/$MCLONE_ANDROID_ACTIVITY" \
            | grep -E "mCurrentFocus|mFocusedApp|mFocusedWindow" \
            | head -1 || true)"
        if [[ -z "$focused_line" ]]; then
            mclone_collect_logcat "$serial" "$pid" "$log_path"
            mclone_die "$MCLONE_ANDROID_APP_ID is running but is not the focused activity; logcat: $log_path"
        fi
        mclone_note "Focused activity: $focused_line"
    else
        activity_block="$("$ADB" -s "$serial" shell dumpsys activity top 2>/dev/null | tr -d '\r' \
            | grep -A12 -F "ACTIVITY $MCLONE_ANDROID_APP_ID/$MCLONE_ANDROID_ACTIVITY" || true)"
        if ! printf '%s\n' "$activity_block" | grep -F "mResumed=true" >/dev/null; then
            mclone_collect_logcat "$serial" "$pid" "$log_path"
            mclone_die "$MCLONE_ANDROID_APP_ID is running but its NativeActivity is not resumed; logcat: $log_path"
        fi
        mclone_note "NativeActivity is resumed"
    fi

    mclone_run_touch_swipe "$serial"

    mclone_collect_logcat "$serial" "$pid" "$log_path"
    if grep -E "FATAL EXCEPTION|Fatal signal|SIGSEGV|thread .* panicked|panicked at" "$log_path" >/dev/null 2>&1; then
        mclone_die "fatal Mclone logcat entries found in $log_path"
    fi
    if [[ "${MCLONE_ANDROID_REQUIRE_RENDERED_FRAME:-1}" == "1" ]] \
        && ! grep -E "Mclone Android rendered .* frame" "$log_path" >/dev/null 2>&1; then
        mclone_die "no Mclone rendered-frame marker found in $log_path"
    fi

    mclone_capture_screenshot "$serial" "$screenshot_path"
    mclone_note "Screenshot: $screenshot_path"
    mclone_note "Logcat: $log_path"
    mclone_note "Android smoke validation passed"
}

mclone_configure_remote_addr() {
    local serial="$1"
    local property="${MCLONE_ANDROID_REMOTE_ADDR_PROPERTY:-debug.mclone.remote_addr}"
    local remote_addr="${MCLONE_ANDROID_REMOTE_ADDR:-}"

    if [[ -n "$remote_addr" ]]; then
        mclone_note "Configuring Android remote dedicated address $property=$remote_addr"
        "$ADB" -s "$serial" shell setprop "$property" "$remote_addr" >/dev/null
    else
        mclone_note "Clearing Android remote dedicated address $property"
        "$ADB" -s "$serial" shell setprop "$property" "__mclone_none__" >/dev/null
    fi
}
