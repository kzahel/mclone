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

if [[ -n "${ANDROID_DIR:-}" && -f "$ANDROID_DIR/startup-properties.sh" ]]; then
    source "$ANDROID_DIR/startup-properties.sh"
fi

mclone_require_arg() {
    local option="$1"
    local value="${2:-}"
    [[ -n "$value" ]] || mclone_die "$option requires a value"
}

MCLONE_ANDROID_STARTUP_ARGV=()
MCLONE_ANDROID_EFFECTIVE_STARTUP_ARGV=()

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

mclone_quest_testbed_cli() {
    local candidate

    if [[ -n "${QUEST_TESTBED_CLI:-}" ]]; then
        [[ -x "$QUEST_TESTBED_CLI" ]] || mclone_die "QUEST_TESTBED_CLI is not executable: $QUEST_TESTBED_CLI"
        printf '%s\n' "$QUEST_TESTBED_CLI"
        return 0
    fi
    for candidate in \
        "$REPO_ROOT/../quest-testbed/bin/quest" \
        "$HOME/code/quest-testbed/bin/quest" \
        "$HOME/Documents/code/quest-testbed/bin/quest"; do
        if [[ -x "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    mclone_die "quest-testbed was not found; clone https://github.com/kzahel/quest-testbed beside mclone or set QUEST_TESTBED_CLI"
}

mclone_quest_serial() {
    local adb_path="$1"
    local requested_serial="${2:-}"
    local quest_cli
    local -a command

    quest_cli="$(mclone_quest_testbed_cli)"
    command=("$quest_cli" --adb "$adb_path")
    if [[ -n "$requested_serial" ]]; then
        command+=(--serial "$requested_serial")
    fi
    "${command[@]}" serial
}

mclone_online_devices() {
    "$ADB" devices | tr -d '\r' | awk '$2 == "device" { print $1 }'
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
    local line_count="${MCLONE_ANDROID_LOGCAT_LINES:-400}"

    mkdir -p "$(dirname "$log_path")"
    if [[ -n "$pid" ]]; then
        "$ADB" -s "$serial" logcat -d "--pid=$pid" -t "$line_count" > "$log_path" 2>/dev/null \
            || "$ADB" -s "$serial" logcat -d -t "$line_count" > "$log_path" 2>/dev/null \
            || true
    else
        "$ADB" -s "$serial" logcat -d -t "$line_count" > "$log_path" 2>/dev/null || true
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

mclone_abs_path() {
    local path="$1"
    local dir
    local base

    dir="$(cd "$(dirname "$path")" && pwd -P)" || return 1
    base="$(basename "$path")"
    printf '%s/%s' "$dir" "$base"
}

mclone_check_default_asset_pack_lock() {
    local asset_pack_path="$1"
    local default_asset_pack_path="$REPO_ROOT/reference/minecraft-1.17.1/extracted.zip"
    local asset_pack_abs
    local default_asset_pack_abs

    [[ "${MCLONE_ANDROID_ASSET_LOCK_CHECK:-1}" == "1" ]] || return 0
    asset_pack_abs="$(mclone_abs_path "$asset_pack_path")" || return 0
    default_asset_pack_abs="$(mclone_abs_path "$default_asset_pack_path")" || return 0
    [[ "$asset_pack_abs" == "$default_asset_pack_abs" ]] || return 0

    command -v pnpm >/dev/null 2>&1 || {
        mclone_die "pnpm is required to check the default asset pack lock; install pnpm or set MCLONE_ANDROID_ASSET_LOCK_CHECK=0"
    }
    mclone_note "Checking default asset pack lock"
    (cd "$REPO_ROOT" && pnpm --silent assets:pack:check)
}

mclone_stage_asset_pack() {
    local serial="$1"
    local asset_pack_path="${MCLONE_ANDROID_ASSET_PACK:-$REPO_ROOT/reference/minecraft-1.17.1/extracted.zip}"
    local remote_dir="/sdcard/Android/data/$MCLONE_ANDROID_APP_ID/files/assets/packs"
    local remote_path="$remote_dir/extracted.zip"

    [[ -f "$asset_pack_path" ]] || {
        mclone_die "asset pack not found at $asset_pack_path; run pnpm assets:pack"
    }

    mclone_check_default_asset_pack_lock "$asset_pack_path"
    if [[ "${MCLONE_ANDROID_STAGE_INTERNAL_ASSETS:-0}" == "1" ]]; then
        mclone_stage_internal_asset_pack "$serial" "$asset_pack_path"
        mclone_stage_local_sound_assets "$serial"
        return 0
    fi

    mclone_note "Staging asset pack $asset_pack_path to $remote_path"
    "$ADB" -s "$serial" shell mkdir -p "$remote_dir" >/dev/null
    "$ADB" -s "$serial" push "$asset_pack_path" "$remote_path" >/dev/null
    mclone_stage_local_sound_assets "$serial"
    mclone_repair_external_asset_permissions "$serial"
    mclone_repair_emulator_asset_permissions "$serial"
}

mclone_run_as_app_shell() {
    local serial="$1"
    local command="$2"

    "$ADB" -s "$serial" shell "run-as $MCLONE_ANDROID_APP_ID sh -c '$command'"
}

mclone_reset_android_worlds_for_persist_smoke() {
    local serial="$1"

    [[ "${MCLONE_ANDROID_SESSION_SMOKE:-}" == "persist-restart" ]] || return 0
    mclone_note "Clearing app-owned Android worlds before persist-restart smoke"
    mclone_run_as_app_shell "$serial" "rm -rf files/worlds" >/dev/null 2>&1 || true
    "$ADB" -s "$serial" shell "rm -rf /sdcard/Android/data/$MCLONE_ANDROID_APP_ID/files/worlds" >/dev/null 2>&1 || true
}

mclone_stage_internal_asset_pack() {
    local serial="$1"
    local asset_pack_path="$2"
    local tmp_path="/data/local/tmp/${MCLONE_ANDROID_APP_ID//./_}-extracted.zip"

    mclone_note "Staging asset pack $asset_pack_path to internal app storage files/assets/packs/extracted.zip"
    "$ADB" -s "$serial" push "$asset_pack_path" "$tmp_path" >/dev/null
    mclone_run_as_app_shell "$serial" \
        "rm -rf files/assets/packs && mkdir -p files/assets/packs && cp $tmp_path files/assets/packs/extracted.zip && chmod 600 files/assets/packs/extracted.zip"
    "$ADB" -s "$serial" shell rm -f "$tmp_path" >/dev/null || true
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

    if [[ "${MCLONE_ANDROID_STAGE_INTERNAL_ASSETS:-0}" == "1" ]]; then
        mclone_stage_internal_sound_assets "$serial" "$source_dir"
        return 0
    fi

    mclone_note "Staging local sound assets $source_dir to $remote_dir"
    "$ADB" -s "$serial" shell rm -rf "$remote_dir" >/dev/null
    "$ADB" -s "$serial" shell mkdir -p "$remote_dir" >/dev/null
    mclone_push_asset_tree "$serial" "$source_dir" "$remote_dir"
}

mclone_stage_internal_sound_assets() {
    local serial="$1"
    local source_dir="$2"
    local tmp_dir="/data/local/tmp/${MCLONE_ANDROID_APP_ID//./_}-local-sounds"

    mclone_note "Staging local sound assets $source_dir to internal app storage files/assets/local-sounds"
    "$ADB" -s "$serial" shell rm -rf "$tmp_dir" >/dev/null
    "$ADB" -s "$serial" shell mkdir -p "$tmp_dir" >/dev/null
    mclone_push_asset_tree "$serial" "$source_dir" "$tmp_dir"
    mclone_run_as_app_shell "$serial" \
        "rm -rf files/assets/local-sounds && mkdir -p files/assets && cp -R $tmp_dir files/assets/local-sounds && chmod -R u+rwX,go-rwx files/assets/local-sounds"
    "$ADB" -s "$serial" shell rm -rf "$tmp_dir" >/dev/null || true
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

mclone_repair_external_asset_permissions() {
    local serial="$1"
    local remote_asset_dir="/sdcard/Android/data/$MCLONE_ANDROID_APP_ID/files/assets"

    [[ "${MCLONE_ANDROID_REPAIR_ASSET_PERMS:-1}" == "1" ]] || return 0

    mclone_note "Repairing staged external asset permissions for $MCLONE_ANDROID_APP_ID"
    "$ADB" -s "$serial" shell chmod -R u+rwX,g+rwX,o+rX "$remote_asset_dir" >/dev/null 2>&1 || {
        mclone_note "Could not repair staged external asset permissions; app may be unable to read shell-owned staged assets"
    }
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

mclone_android_display_size() {
    local serial="$1"
    local size

    size="$("$ADB" -s "$serial" shell wm size 2>/dev/null \
        | tr -d '\r' \
        | sed -n 's/.*size: \([0-9][0-9]*\)x\([0-9][0-9]*\).*/\1 \2/p' \
        | tail -1)"
    [[ -n "$size" ]] || mclone_die "could not determine Android display size with wm size"
    printf '%s\n' "$size"
}

mclone_android_gui_scale() {
    local width="$1"
    local height="$2"
    local scale=1
    local next

    while (( scale < 4 )); do
        next=$((scale + 1))
        if (( width / next >= 320 && height / next >= 240 )); then
            scale="$next"
        else
            break
        fi
    done
    printf '%s\n' "$scale"
}

mclone_android_tap_pixel() {
    local serial="$1"
    local x="$2"
    local y="$3"
    local label="$4"

    mclone_note "Injecting Android tap for $label at ${x},${y}"
    "$ADB" -s "$serial" shell input tap "$x" "$y" >/dev/null
    sleep "${MCLONE_ANDROID_UI_TAP_SECONDS:-0.7}"
}

mclone_run_session_smoke() {
    local serial="$1"
    local smoke="${MCLONE_ANDROID_SESSION_SMOKE:-}"
    local width
    local height
    local scale
    local center_x
    local center_y

    [[ -n "$smoke" ]] || return 0
    read -r width height <<<"$(mclone_android_display_size "$serial")"
    scale="$(mclone_android_gui_scale "$width" "$height")"
    center_x=$((width / 2))
    center_y=$((height / 2))
    mclone_note "Running Android session smoke '$smoke' on ${width}x${height} display at GUI scale $scale"

    case "$smoke" in
        new-world)
            mclone_android_tap_pixel "$serial" $((30 * scale)) $((30 * scale)) "touch menu"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 36 * scale)) "pause Quit To Title"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y - 12 * scale)) "title Singleplayer"
            mclone_android_tap_pixel "$serial" $((center_x - 46 * scale)) $((center_y + 125 * scale)) "world-list Create"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 48 * scale)) "new-world Create World"
            ;;
        persist-restart)
            mclone_android_tap_pixel "$serial" $((30 * scale)) $((30 * scale)) "touch menu"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 36 * scale)) "pause Quit To Title"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y - 12 * scale)) "title Singleplayer"
            mclone_android_tap_pixel "$serial" $((center_x - 46 * scale)) $((center_y + 125 * scale)) "world-list Create"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 48 * scale)) "new-world Create World"
            sleep "${MCLONE_ANDROID_PERSIST_CREATE_SETTLE_SECONDS:-6}"
            mclone_android_tap_pixel "$serial" $(((width / scale - 117) * scale)) $(((height / scale - 56) * scale)) "touch Use/place block"
            sleep "${MCLONE_ANDROID_PERSIST_AFTER_PLACE_SECONDS:-2}"
            mclone_android_tap_pixel "$serial" $((30 * scale)) $((30 * scale)) "touch menu after placement"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 36 * scale)) "pause Quit To Title after placement"
            sleep "${MCLONE_ANDROID_PERSIST_AFTER_QUIT_SECONDS:-2}"
            "$ADB" -s "$serial" shell am force-stop "$MCLONE_ANDROID_APP_ID" >/dev/null 2>&1 || true
            mclone_note "Relaunching $MCLONE_ANDROID_APP_ID/$MCLONE_ANDROID_ACTIVITY for persistence restart smoke"
            "$ADB" -s "$serial" shell am start -W -n "$MCLONE_ANDROID_APP_ID/$MCLONE_ANDROID_ACTIVITY" >/dev/null
            sleep "${MCLONE_ANDROID_PERSIST_RELAUNCH_SECONDS:-15}"
            mclone_android_tap_pixel "$serial" $((30 * scale)) $((30 * scale)) "touch menu after relaunch"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 36 * scale)) "pause Quit To Title after relaunch"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y - 12 * scale)) "title Singleplayer after relaunch"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y - 89 * scale)) "world-list first row after relaunch"
            mclone_android_tap_pixel "$serial" $((182 * scale)) $((325 * scale)) "world-list Open after relaunch"
            sleep "${MCLONE_ANDROID_PERSIST_REOPEN_SETTLE_SECONDS:-8}"
            ;;
        join-remote)
            mclone_android_tap_pixel "$serial" $((30 * scale)) $((30 * scale)) "touch menu"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 36 * scale)) "pause Quit To Title"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 12 * scale)) "title Join Remote"
            mclone_android_tap_pixel "$serial" "$center_x" $((center_y + 30 * scale)) "join-remote Connect"
            ;;
        *)
            mclone_die "unsupported Android session smoke '$smoke'; expected new-world, join-remote, or persist-restart"
            ;;
    esac
    sleep "${MCLONE_ANDROID_SESSION_SMOKE_SETTLE_SECONDS:-5}"
}

mclone_check_session_smoke_log() {
    local log_path="$1"
    local smoke="${MCLONE_ANDROID_SESSION_SMOKE:-}"

    [[ -n "$smoke" ]] || return 0
    case "$smoke" in
        new-world)
            grep -F "Mclone Android created local world seed=" "$log_path" >/dev/null 2>&1 \
                || mclone_die "Android new-world session smoke marker was not found in $log_path"
            ;;
        persist-restart)
            grep -F "Android gameplay interaction Use submitted at" "$log_path" >/dev/null 2>&1 \
                || mclone_die "Android persist-restart placement marker was not found in $log_path"
            if [[ "$(grep -F "Mclone Android created local world seed=" "$log_path" | wc -l | tr -d ' ')" -lt 2 ]]; then
                mclone_die "Android persist-restart reopen marker was not found in $log_path"
            fi
            ;;
        join-remote)
            grep -F "Mclone Android joined remote session" "$log_path" >/dev/null 2>&1 \
                || mclone_die "Android join-remote session smoke marker was not found in $log_path"
            ;;
        *)
            mclone_die "unsupported Android session smoke '$smoke'; expected new-world, join-remote, or persist-restart"
            ;;
    esac
}

mclone_install_launch_smoke() {
    local serial="$1"
    local screenshot_path="$2"
    local log_path="$3"
    local smoke_seconds="$4"
    local startup_argv_json=""
    local launch_component
    local remote_launch_command
    local launch_output
    local pid
    local focused_line
    local activity_block

    mclone_note "Using $(mclone_device_summary "$serial")"
    mclone_note "Installing $APK_PATH"
    "$ADB" -s "$serial" install -r "$APK_PATH"
    if [[ "${MCLONE_ANDROID_RESET_APP_DATA:-0}" == "1" ]]; then
        mclone_note "Clearing Android app data for cold-run validation"
        "$ADB" -s "$serial" shell pm clear "$MCLONE_ANDROID_APP_ID" >/dev/null
    fi
    mclone_reset_android_worlds_for_persist_smoke "$serial"

    if [[ "${STAGE_ASSETS:-1}" == "1" ]]; then
        mclone_stage_asset_pack "$serial"
    else
        mclone_note "Skipping Android asset-pack staging"
    fi

    mclone_configure_remote_addr "$serial"
    mclone_android_collect_startup_argv
    if ((${#MCLONE_ANDROID_EFFECTIVE_STARTUP_ARGV[@]} > 0)); then
        startup_argv_json="$(mclone_android_startup_argv_json "${MCLONE_ANDROID_EFFECTIVE_STARTUP_ARGV[@]}")"
        mclone_note "Startup argv intent extra: $startup_argv_json"
    fi

    "$ADB" -s "$serial" shell am force-stop "$MCLONE_ANDROID_APP_ID" >/dev/null 2>&1 || true
    "$ADB" -s "$serial" logcat -c || true

    mclone_note "Launching $MCLONE_ANDROID_APP_ID/$MCLONE_ANDROID_ACTIVITY"
    launch_component="$MCLONE_ANDROID_APP_ID/$MCLONE_ANDROID_ACTIVITY"
    remote_launch_command="am start -W -n $(mclone_android_shell_quote "$launch_component")"
    if [[ -n "$startup_argv_json" ]]; then
        remote_launch_command+=" --es $(mclone_android_shell_quote "$STARTUP_ARGV_INTENT_EXTRA")"
        remote_launch_command+=" $(mclone_android_shell_quote "$startup_argv_json")"
    fi
    launch_output="$("$ADB" -s "$serial" shell "$remote_launch_command" 2>&1 | tr -d '\r')"
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
    mclone_run_session_smoke "$serial"

    local collect_pid="$pid"
    if [[ "${MCLONE_ANDROID_SESSION_SMOKE:-}" == "persist-restart" ]]; then
        collect_pid=""
        MCLONE_ANDROID_LOGCAT_LINES="${MCLONE_ANDROID_LOGCAT_LINES:-3000}"
    fi
    mclone_collect_logcat "$serial" "$collect_pid" "$log_path"
    if grep -E "FATAL EXCEPTION|Fatal signal|thread .* panicked|panicked at" "$log_path" >/dev/null 2>&1; then
        mclone_die "fatal Mclone logcat entries found in $log_path"
    fi
    if [[ "${MCLONE_ANDROID_REQUIRE_RENDERED_FRAME:-1}" == "1" ]] \
        && ! grep -E "Mclone Android rendered .* frame" "$log_path" >/dev/null 2>&1; then
        mclone_die "no Mclone rendered-frame marker found in $log_path"
    fi
    if [[ "${MCLONE_ANDROID_REQUIRE_PACING_PERF:-0}" == "1" ]] \
        && ! grep -F "MCLONE_ANDROID_PACING_PERF_SUMMARY" "$log_path" >/dev/null 2>&1; then
        mclone_die "no Android pacing-perf summary marker found in $log_path"
    fi
    mclone_capture_screenshot "$serial" "$screenshot_path"
    mclone_note "Screenshot: $screenshot_path"
    mclone_note "Logcat: $log_path"
    mclone_check_session_smoke_log "$log_path"
    mclone_note "Android smoke validation passed"
}

mclone_configure_remote_addr() {
    local serial="$1"
    local property="${MCLONE_ANDROID_REMOTE_ADDR_PROPERTY:-${REMOTE_ADDR_PROPERTY:-debug.mclone.remote_addr}}"
    local remote_addr="${MCLONE_ANDROID_REMOTE_ADDR:-}"
    local none_sentinel="${REMOTE_ADDR_NONE_SENTINEL:-__mclone_none__}"

    if [[ -n "$remote_addr" && "${MCLONE_ANDROID_REMOTE_ADDR_VIA_PROPERTY:-0}" == "1" ]]; then
        mclone_note "Configuring legacy Android remote dedicated address $property=$remote_addr"
        "$ADB" -s "$serial" shell setprop "$property" "$remote_addr" >/dev/null
    else
        mclone_note "Clearing legacy Android remote dedicated address $property"
        "$ADB" -s "$serial" shell setprop "$property" "$none_sentinel" >/dev/null
    fi
}

mclone_android_collect_startup_argv() {
    local remote_addr="${MCLONE_ANDROID_REMOTE_ADDR:-}"

    MCLONE_ANDROID_EFFECTIVE_STARTUP_ARGV=("${MCLONE_ANDROID_STARTUP_ARGV[@]}")
    if [[ -n "$remote_addr" && "${MCLONE_ANDROID_REMOTE_ADDR_VIA_PROPERTY:-0}" != "1" ]]; then
        MCLONE_ANDROID_EFFECTIVE_STARTUP_ARGV+=(--remote-addr "$remote_addr")
    fi
}
