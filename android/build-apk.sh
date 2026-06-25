#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$REPO_ROOT/android/build-common.sh"

ANDROID_SDK_HOME="$(mclone_android_sdk_home_for_build)"
NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
REQUIRED_NDK_VERSION="$(mclone_gradle_ndk_version "$SCRIPT_DIR/app/build.gradle.kts")"

NDK_HOME="$(mclone_resolve_ndk_home "$ANDROID_SDK_HOME" "$NDK_HOME" "$REQUIRED_NDK_VERSION")"
mclone_android_build_preflight "$ANDROID_SDK_HOME" "$NDK_HOME" "$REQUIRED_NDK_VERSION"
mclone_export_android_build_env "$ANDROID_SDK_HOME" "$NDK_HOME"

echo "Building Mclone Android shared library..."
cd "$REPO_ROOT/native"
cargo ndk -t arm64-v8a -o ../android/jniLibs build --release --package mclone-android-client --lib

echo "Bundling libc++_shared.so..."
NDK_PREBUILT="$(find "$NDK_HOME/toolchains/llvm/prebuilt" -maxdepth 1 -mindepth 1 -type d | head -1)"
if [[ -z "$NDK_PREBUILT" ]]; then
    echo "Could not find an LLVM prebuilt toolchain under $NDK_HOME" >&2
    exit 1
fi
mkdir -p "$SCRIPT_DIR/jniLibs/arm64-v8a"
cp "$NDK_PREBUILT/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" \
    "$SCRIPT_DIR/jniLibs/arm64-v8a/"

cd "$SCRIPT_DIR"
if [[ -x ./gradlew ]]; then
    GRADLE=./gradlew
elif command -v gradle >/dev/null 2>&1; then
    GRADLE=gradle
else
    echo "Gradle is not installed and android/gradlew is not present." >&2
    echo "The Rust .so was built; install Gradle or add a wrapper to build the APK." >&2
    exit 1
fi

echo "Building APK..."
"$GRADLE" assembleDebug

echo "APK: android/app/build/outputs/apk/debug/app-debug.apk"
