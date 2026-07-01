#!/usr/bin/env bash
set -euo pipefail

ANDROID_XR_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_XR_DIR/.." && pwd)"

REFRESH_ASSETS=1
LAUNCH_APP=1
WAKE_HEADSET=1
FORWARDED_ARGS=()

usage() {
    cat <<'USAGE'
Usage: android-xr/start-quest-openxr.sh [options]

Refresh the local Minecraft asset pack, build/install the standalone Quest
Android XR APK, stage assets to the attached Quest, wake the headset, and
launch Mclone XR for interactive testing. The app keeps running after this
script exits.

Script options:
  --skip-asset-refresh
                  Reuse reference/minecraft-1.17.1/extracted.zip as-is.
  --no-wake       Do not wake the headset before launch.
  --no-launch     Install and stage assets, but do not launch.
  -h, --help      Show this help.

Common delegated install options:
  --debug         Build/install the debug APK instead of release.
  --serial SERIAL Use a specific attached headset serial.
  --skip-build    Reuse the existing APK.
  --view-pose X,Y,Z,YAW_DEGREES
                 Set the startup XR view pose.
  --remote-addr ADDR
                 Connect to a remote dedicated server.
  --render-compile-workers N
                 Set native render compile worker count for terrain meshing.

All other options are forwarded to android-xr/install-quest-openxr.sh.

Set MCLONE_PYTHON to a Python 3 executable to override interpreter discovery.
USAGE
}

python_works() {
    "$@" --version >/dev/null 2>&1 \
        && "$@" -c 'import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)' >/dev/null 2>&1
}

resolve_python() {
    if [[ -n "${MCLONE_PYTHON:-}" ]]; then
        if python_works "$MCLONE_PYTHON"; then
            PYTHON_CMD=("$MCLONE_PYTHON")
            return 0
        fi
        echo "error: MCLONE_PYTHON does not run as Python 3: $MCLONE_PYTHON" >&2
        exit 1
    fi

    if python_works py -3; then
        PYTHON_CMD=(py -3)
        return 0
    fi
    if python_works python; then
        PYTHON_CMD=(python)
        return 0
    fi
    if python_works python3; then
        PYTHON_CMD=(python3)
        return 0
    fi

    echo "error: could not find a working Python 3. Install Python, or set MCLONE_PYTHON to a Python 3 executable." >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-asset-refresh|--no-asset-refresh)
            REFRESH_ASSETS=0
            shift
            ;;
        --asset-pack)
            REFRESH_ASSETS=0
            FORWARDED_ARGS+=("$1")
            shift
            [[ $# -gt 0 ]] || {
                echo "error: --asset-pack requires a value" >&2
                exit 1
            }
            FORWARDED_ARGS+=("$1")
            shift
            ;;
        --skip-assets)
            REFRESH_ASSETS=0
            FORWARDED_ARGS+=("$1")
            shift
            ;;
        --no-wake)
            WAKE_HEADSET=0
            shift
            ;;
        --no-launch)
            LAUNCH_APP=0
            WAKE_HEADSET=0
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            FORWARDED_ARGS+=("$1")
            shift
            ;;
    esac
done

cd "$REPO_ROOT"

if [[ "$REFRESH_ASSETS" == "1" ]]; then
    PYTHON_CMD=()
    resolve_python
    echo "==> Refreshing Minecraft reference asset pack using: ${PYTHON_CMD[*]}"
    "${PYTHON_CMD[@]}" "$REPO_ROOT/tools/minecraft_assets/asset_pack.py"
else
    echo "==> Reusing existing Minecraft reference asset pack"
fi

INSTALL_ARGS=(--release)
if [[ "$LAUNCH_APP" == "1" ]]; then
    INSTALL_ARGS+=(--launch)
fi
if [[ "$WAKE_HEADSET" == "1" ]]; then
    INSTALL_ARGS+=(--wake)
fi
INSTALL_ARGS+=("${FORWARDED_ARGS[@]}")

echo "==> Starting interactive Quest Android XR install/launch"
bash "$ANDROID_XR_DIR/install-quest-openxr.sh" "${INSTALL_ARGS[@]}"
