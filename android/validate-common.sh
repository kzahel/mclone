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

mclone_android_sdk_home() {
    if [[ -n "${ANDROID_HOME:-}" ]]; then
        echo "$ANDROID_HOME"
    elif [[ -n "${ANDROID_SDK_ROOT:-}" ]]; then
        echo "$ANDROID_SDK_ROOT"
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

    mclone_collect_logcat "$serial" "$pid" "$log_path"
    if grep -E "FATAL EXCEPTION|Fatal signal|SIGSEGV|thread .* panicked|panicked at" "$log_path" >/dev/null 2>&1; then
        mclone_die "fatal Mclone logcat entries found in $log_path"
    fi

    mclone_capture_screenshot "$serial" "$screenshot_path"
    mclone_note "Screenshot: $screenshot_path"
    mclone_note "Logcat: $log_path"
    mclone_note "Android smoke validation passed"
}
