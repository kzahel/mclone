# 112 - Asset Lab Runtime Actor Geometry

Status: active; Slice 2 end-to-end actor figure registry, Slice 2b player
model selection, Slice 2c networked player appearance, and Slice 3a
first-person body toggle landed.

## Purpose

Promote one disposable asset-lab figure into the native runtime just far enough
to prove that AI-authored figure JSON can produce visible shared-engine actor
geometry.

This is not the final character asset pipeline. The first runtime bridge should
stay small, reviewable, and easy to delete if the figure DSL changes. It should
also avoid adding more policy to `mclone-native-client`: actor geometry belongs
in shared render/session boundaries, while platform apps keep owning only window,
surface, input, and lifecycle glue.

## Workstream

Shared implementation, desktop validation first.

Initial code target:

- `native/crates/mclone-render` for static actor mesh construction.
- `native/crates/mclone-render-session` only when actor selection or runtime
  presentation policy must change.
- `tools/asset-lab` remains the authoring and review lab, not a runtime
  dependency.

## Direction

Use the existing `FigureAsset` JSON contract from
[`111-ai-figure-asset-lab.md`](111-ai-figure-asset-lab.md) as input.

The early bridge may embed a generated player figure JSON inside the renderer to
avoid designing the full runtime asset registry too soon. That is intentionally
tactical. The durable path is:

1. asset-lab authoring file
2. exported figure JSON
3. native asset-pack or content-registry entry
4. renderer/session-resolved compiled actor model
5. optional animation clip sampling from exported clip metadata

## First Runtime Contract

Slice 1 should support only what the current player asset needs:

- schema version `1`
- named materials with hex colors
- named ASCII textures
- box primitives
- part parent relationships and local offsets
- per-face box texture overrides for face pixels
- model normalization to actor feet and actor height
- local asset-lab forward mapped to native actor forward

For the first slice, ASCII textures can be converted into tiny colored face
overlay quads instead of stitched into the actor texture atlas. This preserves
visible face details while deferring real atlas/import ownership.

## Non-Goals

- No general entity system work.
- No runtime animation sampling yet.
- No skinning, IK, glTF, or arbitrary mesh import.
- No app-local character definitions in `mclone-native-client`.
- No dependence on TypeScript or Three.js from the native runtime.

## Implementation Slices

### Slice 1 - Static Authored Player Mesh

- [x] Export or check in the current asset-lab `player` figure JSON as a small
      runtime fixture.
- [x] Add a native parser/compiler for the narrow box-only subset.
- [x] Render local and remote players with the compiled authored model.
- [x] Preserve the existing hardcoded humanoid as a fallback/debug shape.
- [x] Interpret ASCII face textures as colored front-face overlay geometry.
- [x] Add mesh tests for bounds, authored colors, and face detail placement.
- [x] Capture a native screenshot and inspect the result.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-asset-lab-player.png --width 1280 --height 720 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --screenshot-camera-view third-person
git diff --check
```

Landed validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-asset-lab-player.png --width 1280 --height 720 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --screenshot-camera-view third-person
git diff --check
```

Screenshot inspected:

```text
/tmp/mclone-asset-lab-player.png
```

The gameplay screenshot path shows the authored player from behind because the
runtime camera exposes first-person and third-person-back only. Front-face ASCII
eye/mouth placement is validated by mesh tests in this slice and visually by the
actor review sheet in Slice 1b.

### Slice 1b - Actor Review Sheet

- [x] Add a native `--actor-review-sheet` headless mode.
- [x] Render the authored player through the same actor renderer used by
      gameplay.
- [x] Capture front, side, and three-quarter views into one PNG sheet.
- [x] Use an actor-only neutral background so figure geometry is not hidden by
      terrain or UI.
- [x] Add CLI and pure sheet/camera tests.
- [x] Capture and inspect the review sheet.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --actor-review-sheet /tmp/mclone-actor-review-sheet.png --width 1152 --height 512
```

Review output inspected:

```text
/tmp/mclone-actor-review-sheet.png
```

### Slice 2 - Real Asset Ownership

- [x] Move figure JSON loading behind `mclone-assets` or a small shared
      content-registry contract.
- [x] Store the promoted player figure under first-party asset content.
- [x] Include first-party figure content in the asset pack lockfile.
- [x] Remove the runtime `include_str!` figure ownership from `mclone-render`.
- [x] Load the player figure through the same asset source chain as actor
      textures.
- [x] Define how actor presentations select a figure asset id.
- [x] Keep platform apps out of the model-selection policy.
- [x] Add missing asset diagnostics for unknown figure ids.

Slice 2 registry cleanup landed: platform resource constructors now pass
`ActorFigureSet` directly, `ActorTextureAssets` no longer exposes the legacy
single `player_figure`, and the temporary `AssetLabPlayer` instance shape was
removed. Runtime actor selection is now id-based against the loaded first-party
figure registry.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-assets -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-assets -p mclone-client -p mclone-render -p mclone-render-session -p mclone-native-client
pnpm assets:pack
pnpm assets:pack:write-lock
pnpm assets:pack:check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-xr-scene -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
pnpm native:web:build
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --actor-review-sheet /tmp/mclone-actor-review-sheet.png --width 1152 --height 512
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --actor-review-sheet /tmp/mclone-actor-figure-id-review-sheet.png --width 1152 --height 512
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --actor-review-sheet /tmp/mclone-actor-registry-review-sheet.png --width 1152 --height 512
```

Review output inspected:

```text
/tmp/mclone-actor-review-sheet.png
/tmp/mclone-actor-figure-id-review-sheet.png
/tmp/mclone-actor-registry-review-sheet.png
```

### Slice 2b - First-Party Player Model Selection

- [x] Add a second first-party authored figure (`mclone:upright_bear`) using
      the current box/material/ASCII-face subset.
- [x] Include the new figure in the first-party actor figure registry and asset
      pack lockfile.
- [x] Render every first-party figure in the native actor review sheet.
- [x] Add a shared Options menu `Player Model` cycle control.
- [x] Route the flat desktop setting into local-player actor figure selection.
- [x] Keep web, flat Android, and XR UI action/state handling coherent while
      those targets defer applying the selected model to their local actor
      render paths.
- [x] Fix the Android XR caller that still used the removed
      `ActorTextureAssets.player_figure` compatibility field.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-assets -p mclone-render -p mclone-render-session -p mclone-ui -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
pnpm assets:pack
pnpm assets:pack:write-lock
pnpm assets:pack:check
pnpm native:web:build
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --actor-review-sheet /tmp/mclone-actor-model-options-review-sheet.png --width 1152 --height 512
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-options-player-model.png --width 890 --height 1024 --screenshot-ui options-pause --startup-wait idle --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
git diff --check
```

Review output inspected:

```text
/tmp/mclone-actor-model-options-review-sheet.png
/tmp/mclone-options-player-model.png
```

### Slice 2c - Networked Player Appearance

- [x] Add first-party player appearance to the shared protocol without making
      `mclone-protocol` depend on asset registries.
- [x] Add a `SetPlayerAppearance` client command and publish appearance on
      remote-player add/update messages.
- [x] Store dedicated-player appearance on the authoritative server player
      entry.
- [x] Route appearance-only changes to visible remote-player observers.
- [x] Map remote-player protocol appearance back to first-party actor figure
      ids in `mclone-client`.
- [x] Send the selected Options model from desktop flat, web, flat Android, and
      XR clients when a runtime is available.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-protocol -p mclone-client -p mclone-server -p mclone-app-runtime -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-ui
cargo check --manifest-path native/Cargo.toml -p mclone-android-client --target aarch64-linux-android
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client --target aarch64-linux-android
pnpm native:web:build
```

### Slice 2d - Remote Player Visual Smoke

- [x] Add a repeatable native client smoke command that starts a loopback
      dedicated server, connects a synthetic upright-bear player, and captures
      an offscreen observer PNG.
- [x] Keep the smoke scoped to native client validation while exercising real
      native TCP protocol frames and server-side remote-player publication.
- [x] Assert that the capture contains a remote player, submitted/drawn actor
      geometry, and the `mclone:upright_bear` figure id.
- [x] Expose the smoke through `pnpm native:remote-player-visual-smoke`.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:remote-player-visual-smoke
git diff --check
```

Review output inspected:

```text
/tmp/mclone-remote-player-visual-smoke.png
```

### Slice 3 - Animation Metadata Import

- [x] Load exported walk clips and locomotion metadata.
- [x] Sample joint rotations on the native side.
- [x] Preserve gait cycle distance/contact metadata for future movement-speed
      matching.
- [x] Drive remote-player actor rendering from accumulated network movement
      distance so multiplayer players can visibly use imported walk clips.
- [x] Add an authored upright-bear walk clip and include it in the first-party
      asset lockfile.
- [x] Make the remote-player visual smoke assert a positive walk animation
      distance in addition to drawn actor geometry.
- [x] Add native side-view PNG strip and MP4 walk review captures for detailed
      gait critique.
- [x] Expose the walk review through `pnpm native:actor-walk-review`.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-assets -p mclone-render -p mclone-client -p mclone-render-session -p mclone-native-client
pnpm assets:pack
pnpm assets:pack:write-lock
pnpm assets:pack:check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:remote-player-visual-smoke
pnpm native:actor-walk-review
```

Review output inspected:

```text
/tmp/mclone-remote-player-visual-smoke.png
/tmp/mclone-actor-walk-review.png
/tmp/mclone-actor-walk-review.mp4
```

The remote smoke now reports a positive walk distance, for example
`walk distances=[0.55999994]`, which proves the observer received movement
updates and sampled the walk clip rather than only drawing a static figure.
The native walk review strip renders full side-view frames with a floor guide,
and the MP4 encodes the same native-rendered frames. The default script writes a
24-frame, 12 fps, 2-cycle review video.

### Slice 3a - First-Person Local Body Visibility

- [x] Add a default-off first-person local player visibility option.
- [x] Render the local actor in first person as body-only geometry.
- [x] Hide authored head-descendant figure parts and face overlays in body-only
      mode.
- [x] Preserve Java standing eye height alignment at `1.62` blocks above feet.
- [x] Add screenshot CLI coverage through `--first-person-player true|false`.
- [x] Expose the setting in the in-game Options menu as `First Person Body`.

Validation:

```powershell
cargo test --manifest-path native/Cargo.toml -p mclone-render -p mclone-render-session -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-ui -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
pnpm native:web:build
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-first-person-player.png --width 1280 --height 720 --startup-wait idle --screenshot-camera-view first-person --first-person-player true --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time --fullbright true
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-options-first-person-body.png --width 890 --height 1024 --screenshot-ui options-pause --startup-wait idle --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
```

Review output inspected:

```text
/tmp/mclone-first-person-player.png
/tmp/mclone-options-first-person-body.png
```

The default forward first-person camera does not show much body because the
local actor is below the view direction, but the render summary submits the
local body actor and the head-hidden mesh tests verify body-only geometry.

### Slice 4 - Broader Primitive Set

- [ ] Add capsule, sphere, and cylinder support only after the box-only path is
      useful in-engine.
- [ ] Decide whether non-box primitives are native mesh primitives or generated
      authoring-time meshes.
- [ ] Keep generated geometry bounded and deterministic for tests.

## Open Questions

- Should the first durable figure registry live in `mclone-assets`, a new small
  shared figure crate, or `mclone-render-session`?
- Should ASCII textures stay as source data in runtime assets, or should asset
  packing derive PNG/atlas regions while preserving ASCII only in authoring?
- How much animation state belongs in render-session versus gameplay/client
  actor presentation data?
