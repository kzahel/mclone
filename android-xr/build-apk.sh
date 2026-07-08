#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$REPO_ROOT/android/build-common.sh"

ANDROID_SDK_HOME="$(mclone_android_sdk_home_for_build)"
NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
REQUIRED_NDK_VERSION="$(mclone_gradle_ndk_version "$SCRIPT_DIR/app/build.gradle.kts")"
BUILD_TYPE="${MCLONE_ANDROID_XR_BUILD_TYPE:-release}"

usage() {
    cat <<'USAGE'
Usage: android-xr/build-apk.sh [--release|--debug] [--perf-diagnostics]

Build the standalone Quest Android XR APK.

Options:
  --release           Build an optimized release APK. This is the default.
  --debug             Build a debug APK with Cargo's dev profile.
  --perf-diagnostics  Build Rust code with the perf-diagnostics feature.
  -h, --help          Show this help.
USAGE
}

CARGO_FEATURES="${MCLONE_ANDROID_XR_CARGO_FEATURES:-}"

append_cargo_feature() {
    local feature="$1"
    if [[ -z "$CARGO_FEATURES" ]]; then
        CARGO_FEATURES="$feature"
    else
        CARGO_FEATURES="${CARGO_FEATURES},${feature}"
    fi
}

if [[ "${MCLONE_ANDROID_XR_PERF_DIAGNOSTICS:-0}" == "1" ]]; then
    append_cargo_feature "perf-diagnostics"
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            BUILD_TYPE=release
            shift
            ;;
        --debug)
            BUILD_TYPE=debug
            shift
            ;;
        --perf-diagnostics)
            append_cargo_feature "perf-diagnostics"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

case "$BUILD_TYPE" in
    release|Release|RELEASE)
        BUILD_TYPE=release
        CARGO_PROFILE_ARGS=(--release)
        GRADLE_TASK=assembleRelease
        APK_PATH=android-xr/app/build/outputs/apk/release/app-release.apk
        ;;
    debug|Debug|DEBUG)
        BUILD_TYPE=debug
        CARGO_PROFILE_ARGS=()
        GRADLE_TASK=assembleDebug
        APK_PATH=android-xr/app/build/outputs/apk/debug/app-debug.apk
        ;;
    *)
        echo "MCLONE_ANDROID_XR_BUILD_TYPE must be 'release' or 'debug'" >&2
        exit 1
        ;;
esac

NDK_HOME="$(mclone_resolve_ndk_home "$ANDROID_SDK_HOME" "$NDK_HOME" "$REQUIRED_NDK_VERSION")"
CARGO_NDK_PLATFORM="$(mclone_gradle_min_sdk "$SCRIPT_DIR/app/build.gradle.kts")"
CARGO_NDK_PLATFORM="${CARGO_NDK_PLATFORM:-28}"
mclone_android_build_preflight "$ANDROID_SDK_HOME" "$NDK_HOME" "$REQUIRED_NDK_VERSION"
mclone_export_android_build_env "$ANDROID_SDK_HOME" "$NDK_HOME"

CARGO_FEATURE_ARGS=()
if [[ -n "$CARGO_FEATURES" ]]; then
    CARGO_FEATURE_ARGS=(--features "$CARGO_FEATURES")
fi

echo "Building Mclone Android XR shared library ($BUILD_TYPE, API $CARGO_NDK_PLATFORM, features=${CARGO_FEATURES:-default})..."
cd "$REPO_ROOT/native"
cargo ndk -t arm64-v8a --platform "$CARGO_NDK_PLATFORM" -o ../android-xr/jniLibs build "${CARGO_PROFILE_ARGS[@]}" "${CARGO_FEATURE_ARGS[@]}" --package mclone-android-xr-client --lib

echo "Bundling libc++_shared.so..."
NDK_PREBUILT="$(find "$NDK_HOME/toolchains/llvm/prebuilt" -maxdepth 1 -mindepth 1 -type d | head -1)"
if [[ -z "$NDK_PREBUILT" ]]; then
    echo "Could not find an LLVM prebuilt toolchain under $NDK_HOME" >&2
    exit 1
fi
mkdir -p "$SCRIPT_DIR/jniLibs/arm64-v8a"
cp "$NDK_PREBUILT/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" \
    "$SCRIPT_DIR/jniLibs/arm64-v8a/"

echo "Building Android XR APK ($BUILD_TYPE)..."
"$REPO_ROOT/android/gradlew" -p "$SCRIPT_DIR" "$GRADLE_TASK"

echo "APK: $APK_PATH"
