# Android XR / Quest

Standalone Quest Android XR package for mclone.

This package is separate from the flat Android `NativeActivity` app under
`android/`. It follows the Playbox Android XR shape: a Quest VR activity,
Khronos' Android OpenXR loader package, a Rust `cdylib`, launch-scoped startup
argv through an intent extra, Android debug properties for wrapper settings,
validation scripts that wake/restore the headset, and packed asset staging into
the XR package external files directory.

Current status: Android OpenXR terrain-frame smoke with shared controller
actions wired. The app loads the staged Minecraft asset pack through
`mclone-app-runtime`, initializes the Android OpenXR loader, creates a
Vulkan-backed OpenXR session, creates per-eye color swapchains and
renderer-compatible depth targets, receives Horizon `nativeOnActivityReady`,
creates the shared OpenXR controller action set, builds the shared
`mclone-xr-scene` terrain runtime, feeds controller snapshots through the
shared XR locomotion mapper, submits the first real stereo mclone terrain
frame, and logs `MCLONE_ANDROID_XR_ASSETS_READY`,
`MCLONE_ANDROID_XR_CONTROLLERS_READY`, `MCLONE_ANDROID_XR_TERRAIN_READY`, and
`MCLONE_ANDROID_XR_READY`. It can run local integrated or remote dedicated
through the shared XR scene runtime; set `debug.mclone.remote_addr` (or use
`--remote-addr` in the scripts) to select TCP remote dedicated play.

## Build

```bash
bash android-xr/build-apk.sh --debug
bash android-xr/build-apk.sh --release
```

On Windows, use Git Bash so the script sees the Windows Rust and Android SDK
toolchains:

```powershell
& "C:\Program Files\Git\bin\bash.exe" -lc "cd /c/Users/sox/Documents/code/mclone && bash android-xr/build-apk.sh --debug"
```

The default is release. APK outputs:

```text
android-xr/app/build/outputs/apk/debug/app-debug.apk
android-xr/app/build/outputs/apk/release/app-release.apk
```

## Install

```bash
bash android-xr/install-quest-openxr.sh --debug
```

Install and stage a specific packed asset file:

```bash
bash android-xr/install-quest-openxr.sh --debug --asset-pack reference/minecraft-1.17.1/extracted.zip
```

Install and launch with startup config:

```bash
bash android-xr/install-quest-openxr.sh --debug --skip-build --launch \
  --view-pose 0,120,-96,180 \
  --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
```

Install and launch against a dedicated server reachable from the headset:

```bash
bash android-xr/install-quest-openxr.sh --debug --skip-build --launch \
  --remote-addr HOST:25565 \
  --view-pose 0,120,-96,180
```

The launch-scoped argv is passed as:

```text
mclone.startup.argv
```

The current wrapper properties are:

```text
debug.mclone.remote_addr
debug.mclone.xr_view_pose
```

## Validate

```bash
bash android-xr/validate-quest-openxr.sh --debug --view-pose 0,120,-96,180
```

The validator builds unless `--skip-build` is passed, installs the APK on an
attached Quest, stages `reference/minecraft-1.17.1/extracted.zip` to the XR app
external files directory, wakes the headset, launches the VR activity, waits
for `MCLONE_ANDROID_XR_ASSETS_READY`,
`MCLONE_ANDROID_XR_CONTROLLERS_READY`, `MCLONE_ANDROID_XR_TERRAIN_READY`, and
`MCLONE_ANDROID_XR_READY`, scans for fatal logcat entries, force-stops the app,
restores headset wake/proximity settings, and sleeps the headset. Use
`--asset-pack PATH` to stage a different pack, or `--skip-assets` to leave the
device copy unchanged.

For session/swapchain-only debugging:

```bash
bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180
```

`--session-only` accepts `MCLONE_ANDROID_XR_SESSION_READY` without requiring the
terrain runtime or first submitted stereo frame.

For remote dedicated validation, start `mclone-dedicated-server` on a host the
headset can reach, then pass:

```bash
bash android-xr/validate-quest-openxr.sh --debug --skip-build \
  --remote-addr HOST:25565 \
  --view-pose 0,120,-96,180
```

Logcat defaults to:

```text
/tmp/mclone-quest-openxr-logcat.txt
```

Use `--serial SERIAL` when multiple Android devices are attached.

## Quest Runtime Note

The package mirrors Playbox's Quest launch-policy surface closely:
manifest permissions/features for hand tracking, body tracking, render models,
spatial scene/anchors, passthrough, and controller/hand input metadata are
declared so the Horizon/OpenXR runtime path matches the known-good Playbox
shape. Mclone does not use those product features yet.

On the attached Quest 3, mclone reaches `MCLONE_ANDROID_XR_READY` after pinning
`android-activity` to Playbox's known-good NativeActivity glue version. Desktop
Vulkan XR and Android XR now share the OpenXR/Vulkan/wgpu graphics factory
through `mclone-xr-graphics`, and both desktop XR and Android XR consume
`mclone-xr-scene` for shared XR terrain constants/alignment/runtime shape. The
standalone headset path has been manually verified with awake Quest
controllers. The next implementation step is locomotion-frame tuning: decide
whether smooth movement should be player/body-yaw relative, HMD-yaw relative,
controller-hand relative, or configurable.
