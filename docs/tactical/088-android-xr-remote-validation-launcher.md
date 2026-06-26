# 088: Android XR Remote Validation Launcher

Status: validated via Quest USB tunnel. Android XR remote selection now uses
the Playbox-style launch-scoped `mclone.startup.argv` intent extra, and the
Quest validator can build/start a local dedicated server for the duration of a
remote smoke.

## Purpose

Make the Android XR dedicated-server smoke executable from one validation
command while keeping runtime options launch-scoped. Remote connect is a game
startup option, not a sticky Android debug property.

## Current State

- Android XR still supports the legacy `debug.mclone.remote_addr` fallback, but
  scripts clear it before launch.
- `--remote-addr ADDR` is appended to the launch-scoped
  `mclone.startup.argv` JSON token array, matching the Playbox Android XR
  command-line intent argv style.
- `android-xr/validate-quest-openxr.sh --start-server` builds and starts
  `mclone-dedicated-server` locally, waits for the ready log line, launches the
  headset app, then stops the server during cleanup.
- `--start-server` intentionally requires `--remote-addr HOST:PORT`; the
  headset needs a routable host address, and guessing that from ADB/host
  network state is unreliable.
- On 2026-06-26, the Quest remote smoke passed through `adb reverse`
  (`127.0.0.1:25565` on the headset to the host server). Direct LAN reachability
  from the same Quest to `192.168.1.107:25565` timed out while the server was
  listening, so LAN validation is currently a host firewall/routing issue rather
  than an app-runtime blocker.

## Implementation Slice

- [x] Extend Android XR startup argv parsing with `--remote-addr`.
- [x] Prefer intent-argv remote selection over the legacy debug property.
- [x] Change install/validate scripts so `--remote-addr` is passed through
  `mclone.startup.argv`.
- [x] Add validator options:
  - `--start-server`
  - `--server-listen ADDR`
  - `--server-seed SEED`
  - `--server-log PATH`
- [x] Keep view pose as a debug property, consistent with Playbox's split
  between wrapper/runtime properties and launch-scoped argv.

## Validation

```bash
cargo fmt --manifest-path native/Cargo.toml --all
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
cargo build --manifest-path native/Cargo.toml -p mclone-dedicated-server
bash -n android-xr/startup-properties.sh android-xr/install-quest-openxr.sh android-xr/validate-quest-openxr.sh
bash android-xr/validate-quest-openxr.sh --help
bash android-xr/install-quest-openxr.sh --help
```

Device validation passed on an attached Quest 3 through a USB reverse tunnel:

```bash
adb reverse tcp:25565 tcp:25565
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:validate -- --debug --skip-build \
  --start-server \
  --server-listen 127.0.0.1:25565 \
  --remote-addr 127.0.0.1:25565 \
  --view-pose 0,120,-96,180
adb reverse --remove tcp:25565
```

Observed ready markers:

```text
Android XR remote dedicated address from mclone.startup.argv: 127.0.0.1:25565
MCLONE_ANDROID_XR_ASSETS_READY
MCLONE_ANDROID_XR_CONTROLLERS_READY
MCLONE_ANDROID_XR_SESSION_READY
MCLONE_ANDROID_XR_TERRAIN_READY sections=122 indices=580248 actors=0
MCLONE_ANDROID_XR_READY
```

Direct LAN validation still needs a host address reachable from the headset:

```bash
pnpm native:android-xr:validate -- --debug --skip-build \
  --start-server \
  --server-listen 0.0.0.0:25565 \
  --remote-addr HOST:25565 \
  --view-pose 0,120,-96,180
```

## Guardrails

- Do not infer `HOST` silently; a wrong `127.0.0.1` or USB-only address makes
  the headset connect to itself or nowhere.
- Do not move launch argv parsing into Android properties. Properties are for
  wrapper/runtime toggles such as view pose.
- Do not let the validator-owned server path change the shared client/server
  protocol or Android XR runtime path.
