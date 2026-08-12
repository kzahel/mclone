# Scripts

This directory contains developer and user-facing launch helpers. The entries
below are the click-to-run scripts intended for interactive app testing on this
Windows workstation.

## Linux Desktop OpenXR

On Linux, an attached authorized Quest can drive the official WiVRn server
headlessly over USB:

```bash
pnpm native:xr:linux:wivrn:check
pnpm native:xr:linux:wivrn:smoke
pnpm native:xr:linux:wivrn:mclone
```

The launcher prefers a native `wivrn-server` and falls back to the
`io.github.wivrn.wivrn` Flatpak. It discovers the Android SDK's `adb`, checks
that the Flatpak and selected Quest WiVRn client versions match, installs the
USB reverse tunnel, waits for the connection, and restores the headset
afterward. See [`../docs/linux-setup.md`](../docs/linux-setup.md) for setup and
[`../docs/topics/desktop-openxr-validation.md`](../docs/topics/desktop-openxr-validation.md)
for current evidence.

## Interactive Click-To-Test

Double-click `scripts\start-desktop-xr.bat` to start the desktop OpenXR mclone
terrain app through the existing `start-xr` launcher. This is the desktop XR
path, not the Quest standalone APK path. It defaults to Virtual Desktop/OpenXR,
uses the mclone XR scene, and runs until you close the app or exit the OpenXR
session.

Double-click `scripts\start-android-xr.bat` to refresh the local Minecraft asset
pack, build and install the Quest Android XR APK, push the packed assets to the
attached Quest, wake the headset, and launch Mclone XR. This is the Quest
standalone Android XR path. The app keeps running after the script exits so you
can test interactively in the headset.

Useful command-line variants:

```powershell
scripts\start-desktop-xr.bat --view-pose 0,78,-96,180
scripts\start-android-xr.bat --debug --view-pose 0,120,-96,180
scripts\start-android-xr.bat --skip-build --skip-asset-refresh
scripts\start-android-xr.bat --serial DEVICE_SERIAL
```

If the Android XR launcher cannot find Python because Windows resolves
`python3` to the Microsoft Store placeholder, set `MCLONE_PYTHON` to the real
interpreter path or make sure the Windows `py` launcher is installed. The
wrapper tries `py -3` before `python3` specifically to avoid the Store alias.

## What Not To Click For Interactive Testing

`scripts\start-xr.bat` is the lower-level desktop XR launcher. It can be used
directly, but it has smoke-test modes that may stop automatically unless you
pass `--forever`.

`android-xr\validate-quest-openxr.sh` is a smoke validator. It builds, installs,
launches, waits for readiness markers, then cleans up and sleeps the headset.
Use it for CI-style validation, not manual headset testing.

`android-xr\install-quest-openxr.sh` installs and optionally launches the Quest
APK. `scripts\start-android-xr.bat` is the user-facing wrapper that also
refreshes assets and wakes the headset.
