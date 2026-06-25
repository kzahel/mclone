#!/usr/bin/env bash

mclone_build_die() {
    echo "error: $*" >&2
    exit 1
}

mclone_build_warn() {
    echo "warning: $*" >&2
}

mclone_windows_local_android_sdk() {
    if [[ -n "${LOCALAPPDATA:-}" && "$(uname -s 2>/dev/null || true)" =~ MINGW|MSYS|CYGWIN ]] && command -v cygpath >/dev/null 2>&1; then
        cygpath -u "$LOCALAPPDATA/Android/Sdk"
        return
    fi
    return 1
}

mclone_android_sdk_home_for_build() {
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

mclone_gradle_ndk_version() {
    local gradle_file="$1"

    [[ -f "$gradle_file" ]] || return 0
    sed -n 's/.*ndkVersion *= *"\([^"]*\)".*/\1/p' "$gradle_file" | head -1
}

mclone_resolve_ndk_home() {
    local sdk_home="$1"
    local explicit_ndk_home="$2"
    local required_ndk_version="$3"

    if [[ -n "$explicit_ndk_home" ]]; then
        echo "$explicit_ndk_home"
        return
    fi

    if [[ -n "$required_ndk_version" ]]; then
        if [[ -d "$sdk_home/ndk/$required_ndk_version" ]]; then
            echo "$sdk_home/ndk/$required_ndk_version"
        fi
        return
    fi

    if [[ -d "$sdk_home/ndk" ]]; then
        find "$sdk_home/ndk" -maxdepth 1 -mindepth 1 -type d | sort | tail -1
    fi
}

mclone_android_build_preflight() {
    local sdk_home="$1"
    local ndk_home="$2"
    local required_ndk_version="$3"
    local ndk_prebuilt

    command -v java >/dev/null 2>&1 || mclone_build_die "Java 17 is required but java was not found on PATH."
    command -v cargo >/dev/null 2>&1 || mclone_build_die "Rust cargo was not found on PATH."
    command -v rustup >/dev/null 2>&1 || mclone_build_die "rustup was not found on PATH; install the aarch64-linux-android target before building Android."
    if ! cargo ndk --version >/dev/null 2>&1; then
        mclone_build_die "cargo-ndk is not installed. Install it with: cargo install cargo-ndk"
    fi
    if ! rustup target list --installed | grep -qx 'aarch64-linux-android'; then
        mclone_build_die "Rust target aarch64-linux-android is not installed. Install it with: rustup target add aarch64-linux-android"
    fi

    [[ -d "$sdk_home" ]] || mclone_build_die "Android SDK not found at $sdk_home. Set ANDROID_HOME/ANDROID_SDK_ROOT or install the SDK there."
    [[ -d "$sdk_home/platforms/android-35" ]] || mclone_build_die "Android SDK platform android-35 is missing. Install it with: sdkmanager \"platforms;android-35\""
    [[ -d "$ndk_home" ]] || {
        if [[ -n "$required_ndk_version" ]]; then
            mclone_build_die "Android NDK $required_ndk_version is missing. Install it with: sdkmanager \"ndk;$required_ndk_version\""
        fi
        mclone_build_die "Android NDK not found. Set ANDROID_NDK_HOME/ANDROID_NDK_ROOT or install one under $sdk_home/ndk."
    }

    ndk_prebuilt="$(find "$ndk_home/toolchains/llvm/prebuilt" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | head -1 || true)"
    [[ -n "$ndk_prebuilt" ]] || mclone_build_die "Android NDK at $ndk_home is missing toolchains/llvm/prebuilt."

    if [[ -n "$required_ndk_version" && -z "${ANDROID_NDK_HOME:-}${ANDROID_NDK_ROOT:-}" && "$(basename "$ndk_home")" != "$required_ndk_version" ]]; then
        mclone_build_warn "using NDK $(basename "$ndk_home") but Gradle requests $required_ndk_version"
    fi
}

mclone_export_android_build_env() {
    local sdk_home="$1"
    local ndk_home="$2"

    export ANDROID_HOME="${ANDROID_HOME:-$sdk_home}"
    export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$sdk_home}"
    export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ndk_home}"
    export ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-$ndk_home}"
}
