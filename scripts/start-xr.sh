#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
host_build_dir="${HOST_BUILD_DIR:-${HOME}/code/wivrn-macos/build/wivrn}"

usage() {
    cat <<'EOF'
Usage: scripts/start-xr.sh [script-options] [mclone-xr-options]

Starts the native mclone desktop OpenXR smoke with the `xr` Cargo feature.

Script options:
  --release              Run the optimized release build.
  --build-only           Build the XR binary without launching it.
  --check-only           Check the XR feature without building an executable.
  --desktop-xr           Real persistent desktop XR run verb: renders the
                         mclone world until you quit (companion window Close or
                         headset menu), with a 2D companion window. Not a smoke;
                         does not inject --frames. Cannot combine with --smoke.
  --no-window            Suppress the desktop companion window (requires
                         --desktop-xr; the smoke gates are already windowless).
  --smoke clear|mclone   Select the smoke mode. Default: clear.
  --runtime wivrn|active|json|environment
                         Select the OpenXR runtime bootstrap. Default: wivrn
                         on macOS, environment elsewhere. WiVRn supports the
                         local macOS build and native/Flatpak Linux installs.
  --runtime-json PATH    Runtime manifest to use with --runtime json.
  --view-pose X,Y,Z,YAW  Map the first tracked headset pose to this mclone
                         world pose. Requires --smoke mclone.
  --xr-underwater-mode midpoint|per-eye
                         Select XR underwater detection. Requires --smoke
                         mclone. Default: midpoint.
  --xr-debug-ui none|pause|controls|graphics
                         Hold an XR debug UI panel open after startup for
                         headset UI validation. Requires --smoke mclone.
  --xr-render-mode dual-per-eye|array-per-eye|array-multiview
                         Select the initial XR target/encoding mode.
  --xr-render-mode-cycle Exercise all modes and both target topologies in one
                         live OpenXR/world session.
  --wivrn-usb            Start/reuse the local WiVRn host, install an ADB
                         reverse tunnel, and launch the matching Quest client.
  --frames N             Set the XR smoke frame budget (or bound a --desktop-xr
                         run). Default: 120 for smokes; unbounded for
                         --desktop-xr unless set.
  -h, --help             Show this help.

Examples:
  scripts/start-xr.sh --check-only
  scripts/start-xr.sh --frames 2
  scripts/start-xr.sh --smoke mclone --view-pose 0,72,0,0 --frames 120
  scripts/start-xr.sh --wivrn-usb --frames 2
  scripts/start-xr.sh --wivrn-usb --desktop-xr
  scripts/start-xr.sh --wivrn-usb --desktop-xr --frames 240

On macOS, this follows the Playbox WiVRn defaults:
  HOST_BUILD_DIR=$HOME/code/wivrn-macos/build/wivrn
  WIVRN_HOST_BIN=$HOST_BUILD_DIR/server/wivrn-server-headless
  MONADO_OPENXR_RUNTIME_PATH=$HOST_BUILD_DIR/_deps/monado-build/src/xrt/targets/openxr/libopenxr_wivrn.dylib
  XR_RUNTIME_JSON=$HOST_BUILD_DIR/openxr_wivrn-dev.json
  QUEST_WIVRN_PACKAGE=org.meumeu.wivrn.local
  QUEST_WIVRN_URI=wivrn+tcp://localhost:9757

On Linux, --wivrn-usb prefers WIVRN_HOST_BIN or a native wivrn-server,
then falls back to io.github.wivrn.wivrn from Flatpak. The default Quest
client package is org.meumeu.wivrn.github.
EOF
}

quote_command() {
    local arg
    for arg in "$@"; do
        printf ' %q' "${arg}"
    done
    printf '\n'
}

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required command: $1" >&2
        exit 1
    fi
}

resolve_adb() {
    if [ -n "${ADB:-}" ] && [ -x "${ADB}" ]; then
        printf '%s\n' "${ADB}"
        return
    fi
    if command -v adb >/dev/null 2>&1; then
        command -v adb
        return
    fi

    local candidate
    for candidate in \
        "${HOME}/Android/Sdk/platform-tools/adb" \
        "${HOME}/Library/Android/sdk/platform-tools/adb" \
        "${HOME}/AppData/Local/Android/Sdk/platform-tools/adb"
    do
        if [ -x "${candidate}" ]; then
            printf '%s\n' "${candidate}"
            return
        fi
    done

    return 1
}

first_wivrn_listener_pid() {
    if ! command -v lsof >/dev/null 2>&1; then
        return 1
    fi

    lsof -nP -tiTCP:9757 -sTCP:LISTEN 2>/dev/null | sed -n '1p'
}

has_wivrn_established_connection() {
    if ! command -v lsof >/dev/null 2>&1; then
        return 1
    fi

    lsof -nP -iTCP:9757 2>/dev/null | grep -q ESTABLISHED
}

wivrn_quest_lease_active=0
wivrn_quest_serial=""
wivrn_quest_package=""

quest_testbed_cli() {
    local candidate
    if [ -n "${QUEST_TESTBED_CLI:-}" ] && [ -x "${QUEST_TESTBED_CLI}" ]; then
        printf '%s\n' "${QUEST_TESTBED_CLI}"
        return 0
    fi
    for candidate in \
        "${repo_root}/../quest-testbed/bin/quest" \
        "${HOME}/code/quest-testbed/bin/quest" \
        "${HOME}/Documents/code/quest-testbed/bin/quest"; do
        if [ -x "${candidate}" ]; then
            printf '%s\n' "${candidate}"
            return 0
        fi
    done
    echo "quest-testbed was not found; clone https://github.com/kzahel/quest-testbed beside mclone or set QUEST_TESTBED_CLI" >&2
    return 1
}

wait_for_wivrn_host_port() {
    local pid="$1"
    local log_path="$2"

    for _ in $(seq 1 10); do
        if [ -n "$(first_wivrn_listener_pid || true)" ]; then
            return 0
        fi
        if ! kill -0 "${pid}" >/dev/null 2>&1; then
            echo "WiVRn host exited before opening TCP 9757. See ${log_path}" >&2
            return 1
        fi
        sleep 1
    done

    echo "WiVRn host did not open TCP 9757. See ${log_path}" >&2
    return 1
}

wait_for_wivrn_usb_connection() {
    local log_path="$1"

    for _ in $(seq 1 30); do
        if [ -n "${log_path}" ] && grep -q "Initial headset handshake completed" "${log_path}" 2>/dev/null; then
            return 0
        fi
        if has_wivrn_established_connection; then
            return 0
        fi
        sleep 1
    done

    echo "WiVRn USB connection did not become established." >&2
    if [ -n "${log_path}" ]; then
        echo "WiVRn host log: ${log_path}" >&2
    fi
    return 1
}

start_wivrn_usb_stack() {
    adb_bin="$(resolve_adb || true)"
    if [ -z "${adb_bin}" ]; then
        echo "Missing adb. Set ADB or install Android SDK platform-tools." >&2
        exit 1
    fi
    need_cmd lsof

    need_cmd adb
    local quest_cli
    local adb_path

    local host_bin="${WIVRN_HOST_BIN:-${host_build_dir}/server/wivrn-server-headless}"
    local quest_package="${QUEST_WIVRN_PACKAGE:-org.meumeu.wivrn.local}"
    local quest_uri="${QUEST_WIVRN_URI:-wivrn+tcp://localhost:9757}"
    local server_version=""
    local client_version=""
    local existing_pid
    local existing_cmd
    local host_bin=""
    local -a host_cmd

    if [ "${uname_s}" = "Darwin" ] || [[ "${uname_s}" == Darwin* ]]; then
        host_bin="${WIVRN_HOST_BIN:-${host_build_dir}/server/wivrn-server-headless}"
        quest_package="${QUEST_WIVRN_PACKAGE:-org.meumeu.wivrn.local}"
        if [ ! -x "${host_bin}" ]; then
            echo "WiVRn host binary not found or not executable: ${host_bin}" >&2
            exit 1
        fi
        host_cmd=("${host_bin}" --no-encrypt)
        wivrn_host_kind=native
    elif [ "${uname_s}" = "Linux" ] || [[ "${uname_s}" == Linux* ]]; then
        quest_package="${QUEST_WIVRN_PACKAGE:-org.meumeu.wivrn.github}"
        if [ -n "${WIVRN_HOST_BIN:-}" ]; then
            host_bin="${WIVRN_HOST_BIN}"
            if [ ! -x "${host_bin}" ]; then
                echo "WiVRn host binary not found or not executable: ${host_bin}" >&2
                exit 1
            fi
            host_cmd=(
                "${host_bin}"
                --no-fork
                --no-encrypt
                --early-active-runtime
            )
            wivrn_host_kind=native
        elif command -v wivrn-server >/dev/null 2>&1; then
            host_bin="$(command -v wivrn-server)"
            host_cmd=(
                "${host_bin}"
                --no-fork
                --no-encrypt
                --early-active-runtime
            )
            wivrn_host_kind=native
        elif command -v flatpak >/dev/null 2>&1 \
            && flatpak info io.github.wivrn.wivrn >/dev/null 2>&1
        then
            server_version="$(flatpak run --command=wivrn-server \
                io.github.wivrn.wivrn --version 2>/dev/null \
                | sed -n 's/^WiVRn version //p' \
                | sed -n '1p')"
            host_cmd=(
                flatpak run
                --command=wivrn-server
                io.github.wivrn.wivrn
                --no-fork
                --no-encrypt
                --early-active-runtime
            )
            wivrn_host_kind=flatpak
        else
            echo "WiVRn is not installed." >&2
            echo "Install a native wivrn-server or the io.github.wivrn.wivrn Flatpak." >&2
            exit 1
        fi
    else
        echo "--wivrn-usb is supported on macOS and Linux, not ${uname_s}." >&2
        exit 1
    fi

    quest_cli="$(quest_testbed_cli)"
    adb_path="$(command -v adb)"
    wivrn_quest_serial="$("${quest_cli}" --adb "${adb_path}" serial)"
    export ANDROID_SERIAL="${wivrn_quest_serial}"
    wivrn_quest_package="${quest_package}"
    echo "Starting recoverable Quest lease for WiVRn USB smoke"
    "${quest_cli}" --adb "${adb_path}" --serial "${wivrn_quest_serial}" begin \
        --owner-pid "$$" \
        --stop-package "${quest_package}" \
        --reverse tcp:9757=tcp:9757
    wivrn_quest_lease_active=1

    echo "Stopping Quest WiVRn client if it is already running: ${quest_package}"
    "${adb_bin}" shell am force-stop "${quest_package}" >/dev/null 2>&1 || true
    sleep 1

    existing_pid="$(first_wivrn_listener_pid || true)"
    if [ -n "${existing_pid}" ]; then
        existing_cmd="$(ps -p "${existing_pid}" -o command= 2>/dev/null || true)"
        echo "Using existing WiVRn host on TCP 9757: pid ${existing_pid}"
        case "${existing_cmd}" in
            *wivrn-server*--no-encrypt*) ;;
            *wivrn-server*)
                echo "Warning: existing WiVRn host command does not include --no-encrypt." >&2
                echo "The ADB USB URI omits a PIN, so pairing-mode hosts may not accept it." >&2
                ;;
        esac
    else
        local run_id
        local log_dir
        run_id="$(date +%Y%m%d_%H%M%S)"
        log_dir="${MCLONE_XR_LOG_DIR:-${TMPDIR:-/tmp}/mclone-xr}"
        mkdir -p "${log_dir}"
        wivrn_host_log="${log_dir}/wivrn-host-${run_id}.log"

        printf 'Starting WiVRn host:'
        quote_command "${host_cmd[@]}"
        "${host_cmd[@]}" >"${wivrn_host_log}" 2>&1 &
        wivrn_host_pid="$!"
        wait_for_wivrn_host_port "${wivrn_host_pid}" "${wivrn_host_log}"
    fi

    echo "Launching Quest WiVRn client: ${quest_package} ${quest_uri}"
    "${adb_bin}" shell am start -W -a android.intent.action.VIEW -d "${quest_uri}" "${quest_package}" >/dev/null

    wait_for_wivrn_usb_connection "${wivrn_host_log}"
    sleep "${WIVRN_POST_CONNECT_SETTLE_SECONDS:-2}"
}

cleanup() {
    local status=$?

    if [ -n "${wivrn_host_pid}" ] && kill -0 "${wivrn_host_pid}" >/dev/null 2>&1; then
        if [ "${wivrn_host_kind}" = "flatpak" ]; then
            flatpak kill io.github.wivrn.wivrn >/dev/null 2>&1 || true
        else
            kill "${wivrn_host_pid}" >/dev/null 2>&1 || true
        fi
        wait "${wivrn_host_pid}" >/dev/null 2>&1 || true
    fi
    if [ "${wivrn_quest_lease_active}" -eq 1 ]; then
        local quest_cli
        quest_cli="$(quest_testbed_cli || true)"
        if [ -n "${quest_cli}" ]; then
            "${quest_cli}" --serial "${wivrn_quest_serial}" end || true
        fi
        wivrn_quest_lease_active=0
    fi

    return "${status}"
}

release=0
build_only=0
check_only=0
wivrn_usb=0
smoke=clear
smoke_explicit=0
desktop_xr=0
no_window=0
runtime=auto
runtime_json=""
view_pose=""
frames=120
frames_explicit=0
adb_bin=""
wivrn_host_pid=""
wivrn_host_kind=""
wivrn_host_log=""
wivrn_reverse_installed=0
wivrn_reverse_preexisting=0
mclone_args=()

trap cleanup EXIT

while [ "$#" -gt 0 ]; do
    case "$1" in
        --release)
            release=1
            shift
            ;;
        --build-only)
            build_only=1
            shift
            ;;
        --check-only)
            check_only=1
            shift
            ;;
        --wivrn-usb)
            wivrn_usb=1
            shift
            ;;
        --smoke)
            if [ "$#" -lt 2 ]; then
                echo "--smoke requires clear or mclone" >&2
                exit 1
            fi
            case "$2" in
                clear|mclone)
                    smoke="$2"
                    smoke_explicit=1
                    ;;
                *)
                    echo "--smoke requires clear or mclone, got $2" >&2
                    exit 1
                    ;;
            esac
            shift 2
            ;;
        --desktop-xr)
            desktop_xr=1
            shift
            ;;
        --no-window)
            no_window=1
            shift
            ;;
        --runtime)
            if [ "$#" -lt 2 ]; then
                echo "--runtime requires wivrn, active, json, or environment" >&2
                exit 1
            fi
            case "$2" in
                wivrn|active|json|environment)
                    runtime="$2"
                    ;;
                *)
                    echo "--runtime requires wivrn, active, json, or environment, got $2" >&2
                    exit 1
                    ;;
            esac
            shift 2
            ;;
        --runtime-json)
            if [ "$#" -lt 2 ]; then
                echo "--runtime-json requires a path" >&2
                exit 1
            fi
            runtime_json="$2"
            runtime=json
            shift 2
            ;;
        --view-pose)
            if [ "$#" -lt 2 ]; then
                echo "$1 requires X,Y,Z,YAW_DEGREES" >&2
                exit 1
            fi
            view_pose="$2"
            shift 2
            ;;
        --frames)
            if [ "$#" -lt 2 ]; then
                echo "--frames requires a value" >&2
                exit 1
            fi
            frames="$2"
            frames_explicit=1
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            mclone_args+=("$@")
            break
            ;;
        *)
            mclone_args+=("$1")
            shift
            ;;
    esac
done

if [ "${desktop_xr}" -eq 1 ] && [ "${smoke_explicit}" -eq 1 ]; then
    echo "--desktop-xr is the real persistent run verb; it renders the mclone world and cannot be combined with --smoke." >&2
    exit 1
fi
if [ "${no_window}" -eq 1 ] && [ "${desktop_xr}" -ne 1 ]; then
    echo "--no-window requires --desktop-xr (the smoke gates are already windowless)." >&2
    exit 1
fi

need_cmd cargo

uname_s="$(uname -s 2>/dev/null || printf 'unknown')"

if [ "${runtime}" = "auto" ]; then
    if [ "${uname_s}" = "Darwin" ] || [[ "${uname_s}" == Darwin* ]]; then
        runtime=wivrn
    else
        runtime=environment
    fi
fi

case "${runtime}" in
    active)
        unset XR_RUNTIME_JSON
        unset MONADO_OPENXR_RUNTIME_PATH
        echo "Using active OpenXR loader/runtime discovery."
        ;;
    environment)
        if [ -n "${XR_RUNTIME_JSON:-}" ]; then
            echo "Using XR_RUNTIME_JSON=${XR_RUNTIME_JSON}"
        else
            echo "Using OpenXR loader/runtime discovery from the current environment."
        fi
        ;;
    json)
        if [ -z "${runtime_json}" ] && [ -z "${XR_RUNTIME_JSON:-}" ]; then
            echo "--runtime json requires --runtime-json PATH or XR_RUNTIME_JSON." >&2
            exit 1
        fi
        if [ -n "${runtime_json}" ]; then
            export XR_RUNTIME_JSON="${runtime_json}"
        fi
        echo "Using XR_RUNTIME_JSON=${XR_RUNTIME_JSON}"
        ;;
    wivrn)
        ;;
    *)
        echo "internal error: unsupported runtime ${runtime}" >&2
        exit 1
        ;;
esac

if [ "${runtime}" = "wivrn" ]; then
    if [ "${uname_s}" = "Linux" ] || [[ "${uname_s}" == Linux* ]]; then
        runtime_json="${XR_RUNTIME_JSON:-${XDG_CONFIG_HOME:-${HOME}/.config}/openxr/1/active_runtime.json}"
        if [ -z "${XR_RUNTIME_JSON:-}" ]; then
            export XR_RUNTIME_JSON="${runtime_json}"
        fi
        echo "Using Linux WiVRn runtime manifest: ${XR_RUNTIME_JSON}"
    elif [ "${uname_s}" = "Darwin" ] || [[ "${uname_s}" == Darwin* ]]; then
        runtime_dylib="${MONADO_OPENXR_RUNTIME_PATH:-${host_build_dir}/_deps/monado-build/src/xrt/targets/openxr/libopenxr_wivrn.dylib}"
        runtime_json="${XR_RUNTIME_JSON:-${host_build_dir}/openxr_wivrn-dev.json}"

        if [ -z "${MONADO_OPENXR_RUNTIME_PATH:-}" ] && [ -f "${runtime_dylib}" ]; then
            export MONADO_OPENXR_RUNTIME_PATH="${runtime_dylib}"
        fi
        if [ -z "${XR_RUNTIME_JSON:-}" ] && [ -f "${runtime_json}" ]; then
            export XR_RUNTIME_JSON="${runtime_json}"
        fi

        if [ -n "${MONADO_OPENXR_RUNTIME_PATH:-}" ]; then
            echo "Using MONADO_OPENXR_RUNTIME_PATH=${MONADO_OPENXR_RUNTIME_PATH}"
        else
            echo "No MONADO_OPENXR_RUNTIME_PATH set and default not found: ${runtime_dylib}" >&2
        fi
        if [ -n "${XR_RUNTIME_JSON:-}" ]; then
            echo "Using XR_RUNTIME_JSON=${XR_RUNTIME_JSON}"
        else
            echo "No XR_RUNTIME_JSON set and default not found: ${runtime_json}" >&2
        fi
    else
        echo "--runtime wivrn is supported on macOS and Linux, not ${uname_s}." >&2
        exit 1
    fi
fi

cargo_check_cmd=(cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
cargo_build_cmd=(cargo build --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
cargo_run_cmd=(cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client --features xr)
if [ "${release}" -eq 1 ]; then
    cargo_check_cmd=(cargo check --release --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
    cargo_build_cmd=(cargo build --release --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
    cargo_run_cmd=(cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client --features xr)
fi
if [ "${desktop_xr}" -eq 1 ]; then
    # Real desktop XR run verb: persistent by default (no --frames injection so
    # the run holds until quit), companion window on unless --no-window. An
    # explicit --frames still bounds it for a validation run; --xr-forever in
    # extra args is accepted (and redundant) by the CLI.
    app_args=(--desktop-xr)
    if [ "${no_window}" -eq 1 ]; then
        app_args+=(--no-window)
    fi
    if [ "${frames_explicit}" -eq 1 ]; then
        app_args+=(--frames "${frames}")
    fi
    if [ -n "${view_pose}" ]; then
        app_args+=(--view-pose "${view_pose}")
    fi
    app_args+=("${mclone_args[@]}")
elif [ "${smoke}" = "mclone" ]; then
    app_args=(--xr-mclone-smoke --frames "${frames}" "${mclone_args[@]}")
else
    app_args=(--xr-clear-smoke --frames "${frames}" "${mclone_args[@]}")
fi
if [ "${desktop_xr}" -ne 1 ] && [ -n "${view_pose}" ]; then
    if [ "${smoke}" != "mclone" ]; then
        echo "--view-pose requires --smoke mclone" >&2
        exit 1
    fi
    app_args=("${app_args[@]:0:3}" --view-pose "${view_pose}" "${app_args[@]:3}")
fi

if [ "${check_only}" -eq 1 ]; then
    echo "Checking mclone XR feature..."
    printf 'Command:'
    quote_command "${cargo_check_cmd[@]}"
    (
        cd "${repo_root}"
        "${cargo_check_cmd[@]}"
    )
    exit 0
fi

if [ "${build_only}" -eq 1 ]; then
    echo "Building mclone XR binary..."
    printf 'Command:'
    quote_command "${cargo_build_cmd[@]}"
    (
        cd "${repo_root}"
        "${cargo_build_cmd[@]}"
    )
    exit 0
fi

if [ "${wivrn_usb}" -eq 1 ]; then
    start_wivrn_usb_stack
fi

export RUST_LOG="${RUST_LOG:-warn,mclone_native_client=info}"

if [ "${desktop_xr}" -eq 1 ]; then
    if [ "${frames_explicit}" -eq 1 ]; then
        echo "Starting mclone desktop XR run (bounded to ${frames} frames)..."
    else
        echo "Starting mclone desktop XR run (persistent; close the companion window or headset menu to quit)..."
    fi
else
    echo "Starting mclone XR smoke..."
fi
printf 'Command:'
quote_command "${cargo_run_cmd[@]}" -- "${app_args[@]}"
(
    cd "${repo_root}"
    "${cargo_run_cmd[@]}" -- "${app_args[@]}"
)
