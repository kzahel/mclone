# Android XR / Quest

Standalone Quest Android XR package for mclone.

This package is separate from the flat Android `NativeActivity` app under
`android/`. It follows the Playbox Android XR shape: a Quest VR activity,
Khronos' Android OpenXR loader package, a Rust `cdylib`, launch-scoped startup
argv through an intent extra, Android debug properties for wrapper settings,
and validation scripts that wake/restore the headset.

Current status: Android OpenXR first-frame smoke. The app initializes the
Android OpenXR loader, creates a Vulkan-backed OpenXR session, creates per-eye
color swapchains and depth targets, receives Horizon `nativeOnActivityReady`,
transitions to READY, submits the first stereo diagnostic clear frame, and logs
`MCLONE_ANDROID_XR_READY`.

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

Install and launch with startup config:

```bash
bash android-xr/install-quest-openxr.sh --debug --skip-build --launch \
  --view-pose 0,120,-96,180 \
  --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
```

The launch-scoped argv is passed as:

```text
mclone.startup.argv
```

The current wrapper property is:

```text
debug.mclone.xr_view_pose
```

## Validate

```bash
bash android-xr/validate-quest-openxr.sh --debug --view-pose 0,120,-96,180
```

The validator builds unless `--skip-build` is passed, installs the APK on an
attached Quest, wakes the headset, launches the VR activity, waits for
`MCLONE_ANDROID_XR_READY`, scans for fatal logcat entries, force-stops the app,
restores headset wake/proximity settings, and sleeps the headset.

For session/swapchain-only debugging:

```bash
bash android-xr/validate-quest-openxr.sh --debug --skip-build --session-only --view-pose 0,120,-96,180
```

`--session-only` accepts `MCLONE_ANDROID_XR_SESSION_READY` without requiring the
first submitted stereo frame.

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
`android-activity` to Playbox's known-good NativeActivity glue version. The
next implementation step is to reuse the shared XR host render-view descriptors
for a real mclone terrain frame on Quest.
