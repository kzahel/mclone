#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$REPO_ROOT/android/build-common.sh"

ANDROID_SDK_HOME="$(mclone_android_sdk_home_for_build)"
NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
REQUIRED_NDK_VERSION="$(mclone_gradle_ndk_version "$SCRIPT_DIR/app/build.gradle.kts")"
CARGO_NDK_PLATFORM="$(mclone_gradle_min_sdk "$SCRIPT_DIR/app/build.gradle.kts")"
CARGO_NDK_PLATFORM="${CARGO_NDK_PLATFORM:-28}"
BUILD_ABIS="${MCLONE_ANDROID_ABIS:-arm64-v8a}"

usage() {
    cat <<'USAGE'
Usage: android/build-apk.sh [options]

Build the flat Android APK.

Options:
  --release       Build the distribution APK (nightly signing when configured).
  --abi ABI       Build for one Android ABI. May be repeated.
                  Supported: arm64-v8a, x86_64.
  --abis LIST     Build for comma- or space-separated ABIs.
  -h, --help      Show this help.
USAGE
}

GRADLE_TASK=assembleDebug
APK_VARIANT=debug
EXPLICIT_ABIS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            GRADLE_TASK=assembleRelease
            APK_VARIANT=release
            shift
            ;;
        --abi)
            EXPLICIT_ABIS+=("$2")
            shift 2
            ;;
        --abis)
            BUILD_ABIS="$2"
            shift 2
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

if [[ "${#EXPLICIT_ABIS[@]}" -gt 0 ]]; then
    BUILD_ABIS="${EXPLICIT_ABIS[*]}"
fi

NDK_HOME="$(mclone_resolve_ndk_home "$ANDROID_SDK_HOME" "$NDK_HOME" "$REQUIRED_NDK_VERSION")"
NORMALIZED_ABIS="$(mclone_normalize_android_abis "$BUILD_ABIS")"
mclone_android_build_preflight "$ANDROID_SDK_HOME" "$NDK_HOME" "$REQUIRED_NDK_VERSION" "$NORMALIZED_ABIS"
mclone_export_android_build_env "$ANDROID_SDK_HOME" "$NDK_HOME"
mclone_prepare_first_party_asset_packs

NDK_PREBUILT="$(find "$NDK_HOME/toolchains/llvm/prebuilt" -maxdepth 1 -mindepth 1 -type d | head -1)"
if [[ -z "$NDK_PREBUILT" ]]; then
    echo "Could not find an LLVM prebuilt toolchain under $NDK_HOME" >&2
    exit 1
fi

for abi in arm64-v8a x86_64; do
    rm -f "$SCRIPT_DIR/jniLibs/$abi/libmclone_android_client.so" \
        "$SCRIPT_DIR/jniLibs/$abi/libc++_shared.so"
done

cd "$REPO_ROOT/native"
while IFS= read -r abi; do
    [[ -n "$abi" ]] || continue
    echo "Building Mclone Android shared library for $abi (API $CARGO_NDK_PLATFORM)..."
    cargo ndk -t "$abi" --platform "$CARGO_NDK_PLATFORM" -o ../android/jniLibs build --locked --release --package mclone-android-client --lib

    echo "Bundling libc++_shared.so for $abi..."
    libcxx_dir="$(mclone_android_libcxx_target_dir_for_abi "$abi")"
    mkdir -p "$SCRIPT_DIR/jniLibs/$abi"
    cp "$NDK_PREBUILT/sysroot/usr/lib/$libcxx_dir/libc++_shared.so" \
        "$SCRIPT_DIR/jniLibs/$abi/"
done <<< "$NORMALIZED_ABIS"

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
"$GRADLE" "$GRADLE_TASK"

echo "APK: android/app/build/outputs/apk/$APK_VARIANT/app-$APK_VARIANT.apk"
