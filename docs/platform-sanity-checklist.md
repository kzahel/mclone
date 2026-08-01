# Platform Sanity Checklist

Status: active. Opened 2026-07-05 as the reusable, in-depth platform
preflight for broad client/runtime changes.

Use this checklist before and after work that can affect shared client
experience policy, session replacement, renderer target/view ownership,
platform adapters, OpenXR behavior, Android packaging, or device execution.
It complements the shorter validation matrix in [`platforms.md`](platforms.md);
that document owns supported lane posture, while this document owns the
runbook for proving the lanes are currently drivable.

## Rules

- Run this checklist before risky refactors when the next slice depends on
  platform health. Record the baseline result in the owning tactical before
  changing production code.
- Use `/tmp` for screenshots, logs, and device captures.
- Inspect screenshots or captured frames when a smoke produces pixels.
- Record every lane as `PASS`, `FAIL`, or `BLOCKED`. A blocked lane needs the
  exact missing prerequisite, such as unavailable headset runtime, missing AVD,
  unauthorized Quest, missing Android SDK, or platform-incompatible command.
- Do not claim a later refactor caused a failure unless this preflight proved
  the same lane passed beforehand.
- For headset/device commands, prefer the shortest smoke that exercises the
  changed behavior. For session refactors, include a session-replacement smoke.

## Core And Shared Policy

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
pnpm native:policy:wasm-check
git diff --check
```

## Desktop Flat And Headless

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:desktop-offscreen:smoke
pnpm native:movement:smoke
pnpm native:timedemo:smoke
```

Inspect `/tmp/mclone-desktop-offscreen.png` after the offscreen smoke.

## Desktop OpenXR

Run a compile/runtime check and a headset-backed smoke.

```bash
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
```

On Windows with VirtualDesktopXR:

```bash
pnpm native:xr:windows:check
pnpm native:xr:windows:smoke:connected
pnpm native:xr:windows:mclone:connected
```

On macOS/WiVRn:

```bash
pnpm native:xr:mac:wivrn:check
pnpm native:xr:mac:wivrn:smoke
pnpm native:xr:mac:wivrn:mclone
```

Use the platform/runtime lane that matches the machine being validated. For
session-replacement work, the `mclone` smoke is the important one because it
loads the real terrain session path instead of only proving OpenXR startup.
The macOS WiVRn launcher asks `~/code/quest-testbed` for a recoverable physical
headset lease. The provider saves/restores headset power settings, disables
proximity during the smoke, wakes and later sleeps the headset, and owns the
`adb reverse tcp:9757 tcp:9757` mapping. The project waits briefly after the
WiVRn handshake before launching Mclone. If connection still fails before
Mclone launch, inspect the printed WiVRn host log,
`adb devices -l`, `adb reverse --list`, and Quest logcat for
`org.meumeu.wivrn.local`.

## Flat Android

```bash
pnpm native:android:apk
pnpm native:android:apk:avd
pnpm native:android:avd-smoke -- --skip-build
pnpm native:android:avd-touch-smoke -- --skip-build
pnpm native:android:avd-session-smoke -- --skip-build
```

Inspect the screenshots under `/tmp`:

- `/tmp/mclone-android-avd-chunk.png`
- `/tmp/mclone-android-avd-touch.png`
- `/tmp/mclone-android-avd-session.png`

The session smoke drives the current shared flow:
`touch menu -> Quit To Title -> Singleplayer -> Create -> Create World`. It
captures the screenshot before asserting the log marker so failed runs still
leave visual evidence.

## Android XR / Quest

```bash
~/code/quest-testbed/bin/quest doctor
pnpm native:android-xr:apk
pnpm native:android-xr:validate --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke
```

For remote-session changes, also run the adb-reverse or direct LAN validation
from [`platforms.md`](platforms.md#validation-policy). For local session
replacement, `native:android-xr:session-smoke` is the sentinel. Do not insert
an extra `--` after `native:android-xr:validate`; the package script already
forwards arguments to `validate-quest-openxr.sh`.

The flat-Quest and standalone-XR validators run inside a transactional
`quest-testbed session`. On every ordinary success, failure, interrupt, or
termination, the provider force-stops only the declared Mclone package,
restores the saved Android settings, clears the Meta proximity override,
removes owned reverse ports, and sends `KEYCODE_SLEEP`. A hard-killed process
leaves an on-device recovery journal that `quest doctor` reports and the same
controller repairs before its next lease.

## Web/WASM

Web is not the primary risk lane for XR session-machine work, but it should be
included when a shared policy crate or wasm boundary changed:

```bash
pnpm native:web:typecheck
pnpm native:web:build
pnpm native:web:smoke
```

Inspect `/tmp/mclone-native-web-canvas.png` after `native:web:smoke`.

## Recording Template

```text
Platform sanity baseline, YYYY-MM-DD:

- Core/shared:
  - cargo fmt --manifest-path native/Cargo.toml --all --check: PASS/FAIL/BLOCKED
  - cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime: PASS/FAIL/BLOCKED
  - cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene: PASS/FAIL/BLOCKED
  - pnpm native:policy:wasm-check: PASS/FAIL/BLOCKED
  - git diff --check: PASS/FAIL/BLOCKED
- Desktop flat/headless:
  - cargo check --manifest-path native/Cargo.toml -p mclone-native-client: PASS/FAIL/BLOCKED
  - pnpm native:desktop-offscreen:smoke: PASS/FAIL/BLOCKED
  - pnpm native:movement:smoke: PASS/FAIL/BLOCKED
  - pnpm native:timedemo:smoke: PASS/FAIL/BLOCKED
- Desktop OpenXR:
  - cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr: PASS/FAIL/BLOCKED
  - check command: PASS/FAIL/BLOCKED
  - headset smoke command: PASS/FAIL/BLOCKED
  - mclone headset smoke command: PASS/FAIL/BLOCKED
- Flat Android:
  - pnpm native:android:apk: PASS/FAIL/BLOCKED
  - pnpm native:android:apk:avd: PASS/FAIL/BLOCKED
  - pnpm native:android:avd-smoke -- --skip-build: PASS/FAIL/BLOCKED
  - pnpm native:android:avd-touch-smoke -- --skip-build: PASS/FAIL/BLOCKED
  - pnpm native:android:avd-session-smoke -- --skip-build: PASS/FAIL/BLOCKED
- Android XR / Quest:
  - pnpm native:android-xr:apk: PASS/FAIL/BLOCKED
  - pnpm native:android-xr:validate --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time: PASS/FAIL/BLOCKED
  - MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke: PASS/FAIL/BLOCKED
- Web/WASM, if shared wasm policy changed:
  - pnpm native:web:typecheck: PASS/FAIL/BLOCKED
  - pnpm native:web:build: PASS/FAIL/BLOCKED
  - pnpm native:web:smoke: PASS/FAIL/BLOCKED
```
