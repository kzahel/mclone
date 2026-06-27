# Flat Android

This directory owns the flat, non-XR Android package and validation lane for
the native Rust engine. It builds `mclone-android-client` as a `NativeActivity`
APK, stages the packed Minecraft asset source onto the device, launches the
app, captures screenshots under `/tmp`, and scans logcat for fatal failures.

Current status:

- The APK builds and launches on the `jstorrent-tablet` AVD.
- The Android host owns only NativeActivity lifecycle, `wgpu` surface/config
  resize, package paths, asset staging paths, and touch input.
- Runtime, render-section streaming, texture/mesh asset loading, and full-frame
  sky/terrain composition come from shared native Rust crates.
- The default host mode is local integrated. Launch-scoped startup arguments
  are passed as JSON argv through intent extra `mclone.startup.argv`; the flat
  Android app feeds those tokens into the shared startup parser.
- AVD validation builds an x86_64 emulator APK, stages assets, verifies the
  app-rendered frame marker, and captures a screenshot.
- Quest-flat validation is scripted, but still needs a machine with an attached
  authorized Quest headset.

## Prerequisites

- Android SDK with platform tools, emulator, and NDK installed.
- JDK 17 or newer.
- `cargo-ndk`.
- Rust target `aarch64-linux-android` for physical Android/Quest devices.
- Rust target `x86_64-linux-android` for local x86_64 AVD validation.
- Packed assets at `reference/minecraft-1.17.1/extracted.zip`; rebuild with
  `pnpm assets:pack` if needed.

The scripts discover Android tools through `ANDROID_HOME`, `ANDROID_SDK_ROOT`,
or `~/Android/Sdk`.

On Windows, the `pnpm native:android:*` scripts route through
`scripts/run-native-bash.mjs` so they use Git Bash with the native Windows Rust
toolchain instead of WSL `bash`.

## Build

```bash
pnpm native:android:apk
```

The default APK build targets `arm64-v8a`, which is the physical device/Quest
path. Build an emulator APK explicitly with:

```bash
pnpm native:android:apk:avd
```

or:

```bash
bash android/build-apk.sh --abi x86_64
```

The debug APK is written to:

```text
android/app/build/outputs/apk/debug/app-debug.apk
```

## AVD Validation

Run the full flat Android screenshot smoke:

```bash
pnpm native:android:avd-smoke
```

By default this uses the `jstorrent-tablet` AVD, builds the APK, stages
`reference/minecraft-1.17.1/extracted.zip`, repairs staged asset ownership on
rootable emulator images, launches the app, requires a rendered-frame log
marker, and captures:

```text
/tmp/mclone-android-avd-chunk.png
/tmp/mclone-android-avd-logcat.txt
```

To reuse an existing APK after a build:

```bash
pnpm native:android:avd-smoke -- --skip-build
```

Run the deterministic touch-orbit smoke:

```bash
pnpm native:android:avd-touch-smoke -- --skip-build
```

It captures:

```text
/tmp/mclone-android-avd-touch.png
/tmp/mclone-android-avd-touch-logcat.txt
```

Run against a dedicated server reachable from the AVD:

```bash
pnpm native:android:avd-smoke -- --skip-build --remote-addr 10.0.2.2:25565
```

The validator passes the address through intent extra `mclone.startup.argv`,
for example:

```bash
adb shell am start -W -n com.kzahel.mclone/android.app.NativeActivity \
  --es mclone.startup.argv '["--remote-addr","10.0.2.2:25565"]'
```

The legacy Android property `debug.mclone.remote_addr` is still read as a
fallback for manual launches, but validators clear it to the
`__mclone_none__` sentinel so stale process-global properties do not affect
normal local integrated smokes.

Other shared startup arguments use the same intent extra:

```bash
pnpm native:android:avd-smoke -- --skip-build \
  --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 \
  --movement-speed-multiplier 1.0 --day-time 6000 --freeze-time \
  --lighting false --fullbright true \
  --render-color-profile stylized-bright
```

Use the raw validator when you need custom paths, a visible emulator window, a
specific serial, or a different swipe:

```bash
bash android/validate-avd.sh --help
```

## Desktop Comparison

The desktop chunk smoke now uses the same integrated runtime section set as the
Android smoke:

```bash
pnpm native:desktop-chunk:smoke
```

It writes:

```text
/tmp/mclone-desktop-runtime-chunk.png
```

Use it when checking whether Android differs from the desktop runtime view, not
from an older renderer-only section fixture.

## Quest Flat Smoke

The Quest lane is still flat Android, not OpenXR. It validates the same
NativeActivity APK as a 2D app panel on headset hardware.

```bash
pnpm native:android:quest-flat -- --skip-build --screenshot /tmp/mclone-quest-flat.png --log /tmp/mclone-quest-flat-logcat.txt
```

Quest-flat remote dedicated smoke uses the same option when the server is
reachable from the headset network:

```bash
pnpm native:android:quest-flat -- --skip-build --remote-addr HOST:25565
```

On a machine without an attached authorized Quest, the expected blocker is:

```text
error: no attached Quest headset was found
```

Keep OpenXR, stereo swapchains, controller actions, hand tracking, passthrough,
and Quest standalone packaging out of this flat Android lane.

## Standard Checks

Use these after Android platform or shared runtime/render changes:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target x86_64-linux-android
pnpm native:android:apk
pnpm native:android:avd-smoke
```

If shared native render/runtime code changed, also run the affected desktop and
web gates, commonly:

```bash
pnpm native:desktop-chunk:smoke
pnpm native:web:build
```
