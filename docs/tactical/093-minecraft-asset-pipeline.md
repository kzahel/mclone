# 093 - Minecraft Asset Pipeline

Status: active; Slice 1 landed.

## Goal

Make mclone's Minecraft asset pipeline Playbox-shaped: repo-owned tools live
under `tools/`, checked freshness locks live beside those tools, generated and
Mojang-owned payloads stay ignored under `reference/`, and platform staging uses
the same declared asset contract instead of one-off file pushes.

This replaces the old `scripts/asset-locks/` placement and closes the first
Android audio gap: local sound files must be staged to Quest/Android without
being baked into the locked vanilla render pack.

## Current Problem

The existing render pack tooling is functional but poorly owned:

- `scripts/build-reference-asset-pack.py` owns both pack generation and lock
  freshness.
- `scripts/asset-locks/mclone-vanilla-1.17.1.lock.json` puts a checked metadata
  lock under generic script wrappers.
- Package scripts call `python3`, which resolves to the Microsoft Store
  placeholder on this Windows machine.
- Android staging pushes only
  `reference/minecraft-1.17.1/extracted.zip`.
- Landing OGG files live in a gitignored local filesystem tree and are visible
  to desktop, but are not staged to Quest and are not discoverable on-device.

The lock itself is not the audio bug. The design bug is that local sound assets
are neither part of the staged Android asset tree nor represented as a first
class local asset group.

## Target Shape

Use one game asset set for now:

```text
mclone-game-1.17.1
```

Canonical repo-owned tooling:

```text
tools/minecraft_assets/asset_pack.py
tools/minecraft_assets/locks/mclone-game-1.17.1.lock.json
```

Generated local payloads remain ignored:

```text
reference/minecraft-1.17.1/extracted/
reference/minecraft-1.17.1/extracted.zip
reference/minecraft-1.17.1/extracted.zip.json
reference/minecraft-1.17.1/local-sounds/
```

Compatibility:

- Keep `scripts/build-reference-asset-pack.py` as a thin wrapper for old notes
  and agent muscle memory.
- Keep `reference/minecraft-1.17.1/sound-overlay/` as a transitional read/stage
  fallback, but use `local-sounds` in new docs and scripts.

## Policy

- Do not put Mojang OGGs in `extracted/`, `extracted.zip`, the APK, or git.
- Do not make a "critical assets" split yet. The current game asset set is
  simply the assets needed by the game.
- Keep pack and local sound staging separate. The render pack is locked; local
  sounds are personal validation payloads.
- Platform adapters may stage or discover assets differently, but runtime asset
  lookup should converge on the same `AssetSource` chain.

## Implementation Plan

### Slice 1 - Tool Ownership And Android Local Sounds

- Unignore root `tools/` for repo-owned tooling.
- Move the pack/lock implementation to `tools/minecraft_assets/asset_pack.py`.
- Move the lock to `tools/minecraft_assets/locks/`.
- Rename the asset set from `mclone-vanilla-1.17.1` to
  `mclone-game-1.17.1`.
- Add `scripts/run-python.mjs` and route `pnpm assets:*` through it.
- Keep a compatibility wrapper at `scripts/build-reference-asset-pack.py`.
- Change `scripts/fetch-sound-assets.ps1` to default to `local-sounds`.
- Stage local sound assets to Android/Quest at:

```text
/sdcard/Android/data/<package>/files/assets/local-sounds/
```

- Load on-device local sounds from `MCLONE_ANDROID_ASSET_ROOT/assets/local-sounds`
  when present.

### Slice 2 - Manifest-Driven Local Asset Contract

- Add `tools/minecraft_assets/asset_sets.json` with the single
  `mclone-game-1.17.1` set.
- Represent render pack input and local sound assets as separate groups.
- Have staging code read the manifest rather than hard-coding `extracted.zip`
  and local-sound paths.
- Write a local ignored sound stamp/manifest that records expected sound paths,
  Mojang SHA-1s, and fetch time.

### Slice 3 - Stronger Freshness Checks

- Split quick/full checks like Playbox:
  - quick: contract, pack existence, sidecar equality, stage existence
  - full: normalized JSON/text hashes and pack manifest hash
  - parity: advisory raw payload report for every packed file
- Include tool fingerprints over every `tools/minecraft_assets/*.py|*.json`
  file.
- Fail clearly when generated local assets are stale.

### Slice 4 - Platform And Web Cleanup

- Make native web deploy/sync consume the same manifest outputs.
- Add startup logging of the resolved asset source chain.
- Add an Android validator assertion that local sounds were staged when the
  local sound source exists.
- Remove the transitional `sound-overlay` fallback after local machines have
  migrated.

## Status

2026-06-26 Slice 1 landed:

- Added this tactical.
- Moved canonical pack tooling to `tools/minecraft_assets/asset_pack.py`.
- Moved the checked lock path to
  `tools/minecraft_assets/locks/mclone-game-1.17.1.lock.json`.
- Added `scripts/run-python.mjs` and routed `pnpm assets:*` through it.
- Left `scripts/build-reference-asset-pack.py` as a compatibility wrapper.
- Updated Android asset staging to push local sounds when present.
- Updated runtime asset discovery to load local sounds from on-device staged
  paths and to keep the old `sound-overlay` directory as fallback.
- Rebuilt `reference/minecraft-1.17.1/extracted.zip` and refreshed
  `tools/minecraft_assets/locks/mclone-game-1.17.1.lock.json`.

## Validation

Slice 1 validation:

```powershell
node scripts/run-python.mjs tools/minecraft_assets/asset_pack.py --dry-run
node scripts/run-python.mjs scripts/build-reference-asset-pack.py --dry-run
pnpm assets:pack
pnpm assets:pack:write-lock
pnpm assets:pack:check
cargo check --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-android-client -p mclone-android-xr-client --target x86_64-linux-android
node --check scripts/run-python.mjs
node -e "JSON.parse(require('fs').readFileSync('package.json','utf8'))"
node ./scripts/run-native-bash.mjs -n ./android-xr/start-quest-openxr.sh
node ./scripts/run-native-bash.mjs -n ./android-xr/install-quest-openxr.sh
git diff --check
```

Android/Quest runtime audio remains manual until the next headset run:

```powershell
scripts\start-android-xr.bat --skip-build
```
