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
  --smoke clear|mclone   Select the smoke mode. Default: clear.
  --wivrn-usb            Start/reuse the local macOS WiVRn host, install an ADB
                         reverse tunnel, and launch the Quest WiVRn client.
  --frames N             Set the XR smoke frame budget. Default: 120.
  -h, --help             Show this help.

Examples:
  scripts/start-xr.sh --check-only
  scripts/start-xr.sh --frames 2
  scripts/start-xr.sh --smoke mclone --frames 120
  scripts/start-xr.sh --wivrn-usb --frames 2

On macOS, this follows the Playbox WiVRn defaults:
  HOST_BUILD_DIR=$HOME/code/wivrn-macos/build/wivrn
  WIVRN_HOST_BIN=$HOST_BUILD_DIR/server/wivrn-server-headless
  MONADO_OPENXR_RUNTIME_PATH=$HOST_BUILD_DIR/_deps/monado-build/src/xrt/targets/openxr/libopenxr_wivrn.dylib
  XR_RUNTIME_JSON=$HOST_BUILD_DIR/openxr_wivrn-dev.json
  QUEST_WIVRN_PACKAGE=org.meumeu.wivrn.local
  QUEST_WIVRN_URI=wivrn+tcp://localhost:9757
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
    if [ "${uname_s}" != "Darwin" ] && [[ "${uname_s}" != Darwin* ]]; then
        echo "--wivrn-usb is only implemented for the local macOS WiVRn host path." >&2
        exit 1
    fi

    need_cmd adb

    local host_bin="${WIVRN_HOST_BIN:-${host_build_dir}/server/wivrn-server-headless}"
    local quest_package="${QUEST_WIVRN_PACKAGE:-org.meumeu.wivrn.local}"
    local quest_uri="${QUEST_WIVRN_URI:-wivrn+tcp://localhost:9757}"
    local existing_pid
    local existing_cmd

    if [ ! -x "${host_bin}" ]; then
        echo "WiVRn host binary not found or not executable: ${host_bin}" >&2
        exit 1
    fi

    adb get-state >/dev/null

    echo "Stopping Quest WiVRn client if it is already running: ${quest_package}"
    adb shell am force-stop "${quest_package}" >/dev/null 2>&1 || true
    sleep 1

    echo "Installing ADB reverse tunnel: tcp:9757 -> tcp:9757"
    adb reverse tcp:9757 tcp:9757 >/dev/null

    existing_pid="$(first_wivrn_listener_pid || true)"
    if [ -n "${existing_pid}" ]; then
        existing_cmd="$(ps -p "${existing_pid}" -o command= 2>/dev/null || true)"
        echo "Using existing WiVRn host on TCP 9757: pid ${existing_pid}"
        case "${existing_cmd}" in
            *wivrn-server-headless*--no-encrypt*) ;;
            *wivrn-server-headless*)
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

        echo "Starting WiVRn host: ${host_bin} --no-encrypt"
        "${host_bin}" --no-encrypt >"${wivrn_host_log}" 2>&1 &
        wivrn_host_pid="$!"
        wait_for_wivrn_host_port "${wivrn_host_pid}" "${wivrn_host_log}"
    fi

    echo "Launching Quest WiVRn client: ${quest_package} ${quest_uri}"
    adb shell am start -W -a android.intent.action.VIEW -d "${quest_uri}" "${quest_package}" >/dev/null

    wait_for_wivrn_usb_connection "${wivrn_host_log}"
}

cleanup() {
    local status=$?

    if [ -n "${wivrn_host_pid}" ] && kill -0 "${wivrn_host_pid}" >/dev/null 2>&1; then
        kill "${wivrn_host_pid}" >/dev/null 2>&1 || true
        wait "${wivrn_host_pid}" >/dev/null 2>&1 || true
    fi

    return "${status}"
}

release=0
build_only=0
check_only=0
wivrn_usb=0
smoke=clear
frames=120
wivrn_host_pid=""
wivrn_host_log=""
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
                    ;;
                *)
                    echo "--smoke requires clear or mclone, got $2" >&2
                    exit 1
                    ;;
            esac
            shift 2
            ;;
        --frames)
            if [ "$#" -lt 2 ]; then
                echo "--frames requires a value" >&2
                exit 1
            fi
            frames="$2"
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

need_cmd cargo

uname_s="$(uname -s 2>/dev/null || printf 'unknown')"

if [ "${uname_s}" = "Darwin" ] || [[ "${uname_s}" == Darwin* ]]; then
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
fi

cargo_check_cmd=(cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
cargo_build_cmd=(cargo build --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
cargo_run_cmd=(cargo run --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
if [ "${release}" -eq 1 ]; then
    cargo_check_cmd=(cargo check --release --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
    cargo_build_cmd=(cargo build --release --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
    cargo_run_cmd=(cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --features xr)
fi
if [ "${smoke}" = "mclone" ]; then
    app_args=(--xr-mclone-smoke --frames "${frames}" "${mclone_args[@]}")
else
    app_args=(--xr-clear-smoke --frames "${frames}" "${mclone_args[@]}")
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

echo "Starting mclone XR smoke..."
printf 'Command:'
quote_command "${cargo_run_cmd[@]}" -- "${app_args[@]}"
(
    cd "${repo_root}"
    "${cargo_run_cmd[@]}" -- "${app_args[@]}"
)
