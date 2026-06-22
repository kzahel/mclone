# 063: Native Underwater Camera Effects

Status: completed fog slice.

## Purpose

Make native underwater frames read like Minecraft Java 1.17.1. This is camera
environment work, not water mesh work: when the camera/eye is inside water, the
renderer should apply a subtle underwater screen texture and, in the follow-up
fog slice, fade world geometry toward the water fog color.

The first implementation slice should make the screen visibly water-tinted while
preserving renderer boundaries for the fuller Java-shaped fog path.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ScreenEffectRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/FogRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/Camera.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/player/LocalPlayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/data/worldgen/biome/VanillaBiomes.java`

Important reference shape:

- `ScreenEffectRenderer.renderScreenEffect` renders the water overlay only when
  the player is not a spectator and `player.isEyeInFluid(FluidTags.WATER)`.
- `ScreenEffectRenderer.renderWater` binds `textures/misc/underwater.png`,
  enables normal alpha blending, applies player brightness as RGB, uses alpha
  `0.1`, tiles the quad over `0..4` UVs, and offsets UVs by yaw/pitch:
  `u = -player.getYRot() / 64`, `v = player.getXRot() / 64`.
- `Camera.getFluidInCamera` detects water from the camera block's `FluidState`
  and a fluid-height Y check. Lava and powder snow do extra near-plane sampling;
  this slice should stay focused on water.
- `FogRenderer.setupColor` uses the biome water fog color, transitions between
  water fog colors over `5000 ms`, applies water vision, and writes the clear
  color. Most 1.17.1 overworld biomes use water fog color `329011`
  (`0x050533`) as the fallback.
- `FogRenderer.setupFog` uses water fog start `-8.0`, base water fog distance
  `192.0`, and effective water fog end `distance * 0.5` (`96.0` before water
  vision or swamp adjustments).
- `LocalPlayer.getWaterVision` ramps from `0` to `1` over `600` ticks, with a
  fast first `100` ticks contributing `0.6`.

The vanilla underwater texture is already present locally at
`reference/minecraft-1.17.1/extracted/assets/minecraft/textures/misc/underwater.png`.

## Current Native State

- `native/apps/mclone-native-client/src/app.rs` has a shared
  `render_full_frame` path that renders sky, chunks, actors, then GUI/debug.
  The underwater screen effect belongs after world/actor rendering and before
  GUI/debug composition.
- `native/apps/mclone-native-client/src/scene_runtime.rs` already has
  `camera_inside_occluding_block` and private world block lookup helpers. This
  is the natural app-runtime place to derive camera fluid state from client
  snapshots.
- `native/crates/mclone-client/src/block_shapes.rs` has private terrain MVP
  fluid ID knowledge for collision/outline purposes. Do not duplicate those raw
  IDs in `app.rs`.
- `native/crates/mclone-render/src/shaders/chunk_textured.wgsl` and
  `native/crates/mclone-render/src/shaders/entity_actor.wgsl` currently carry
  lightmap options but no fog uniforms and no world-position output to fragment
  shaders.

## Target Shape

Introduce a small camera environment value owned by the client/runtime layer and
consumed by render composition, for example:

```rust
pub enum CameraFluid {
    None,
    Water {
        fog_color: [f32; 3],
        fog_start: f32,
        fog_end: f32,
        overlay_alpha: f32,
        overlay_uv_offset: [f32; 2],
    },
}
```

Exact names are flexible, but keep the ownership shape:

- Runtime/client snapshot code determines whether the camera is inside water.
- The app frame path passes camera environment facts into render composition.
- `mclone-render` draws screen effects and applies fog from explicit render
  inputs. It must not sample world/chunk state directly.
- Native desktop and native web/WASM consume the same render-facing types.

## First Implementation Slice

Landed in this slice:

- Added shared terrain-MVP fluid classification in `mclone-client`.
- Added snapshot-backed native camera water detection in `WindowSceneRuntime`.
- Added `mclone-render::screen_effect::ScreenEffectsRenderer` with the vanilla
  `textures/misc/underwater.png` fullscreen pass.
- Wired the underwater overlay through the shared full-frame path for native
  window, full-frame headless screenshots, and frame-budget probe rendering.
- Added `--screenshot-eye x,y,z` for deterministic full-frame validation
  captures, plus a headless screenshot `underwater=true|false` report flag.
- Kept the Java constants for the first pass: alpha `0.1`, UV tile size `4.0`,
  and Java-shaped yaw/pitch UV scroll.

Original plan:

1. Add shared fluid classification for current block-state IDs.
   - Expose a narrow helper from the crate that owns current client block facts,
     or move the existing private fluid ID knowledge into a small shared module.
   - Classify water separately from lava; source water and level-bearing water
     IDs must both count as water.
   - Avoid raw terrain ID checks in `app.rs`.

2. Add camera water detection in `WindowSceneRuntime`.
   - Reuse the existing snapshot-backed block lookup path.
   - Match Java's camera/eye semantics: test the camera position against the
     water block at that position and its fluid height.
   - If exact level fluid heights are not yet represented in native client block
     facts, treat current water IDs as full-height for the first pass and leave a
     clear follow-up to replace that with Java `FluidState.getHeight` parity.

3. Thread `CameraFluid` through `render_full_frame`.
   - The value should be computed once per frame near the other camera facts.
   - Keep UI-covering screens deterministic: if the UI covers the world, the
     underwater world effect does not need to draw behind the UI clear path.

4. Add `ScreenEffectsRenderer` in `mclone-render`.
   - Load `textures/misc/underwater.png` through the existing native/web asset
     source path rather than committing a copied texture.
   - Draw a fullscreen textured quad after chunks and actors, before GUI/debug.
   - Use vanilla first-pass constants: alpha `0.1`, UV tile size `4.0`, UV
     offsets `[-yaw / 64, pitch / 64]`.
   - Use full brightness initially if player/camera brightness is not exposed to
     the frame path yet; wire Java-shaped brightness as a follow-up rather than
     blocking the visible underwater effect.

5. Add focused tests.
   - Unit-test water classification for source and level water IDs.
   - Unit-test camera water detection at positions just below/above the water
     height boundary.
   - Unit-test underwater overlay UV offset math.

## Follow-Up Fog Slice

Landed in the fog slice:

- Added `mclone_render::fog::RenderFog` with Java-shaped fallback water fog
  constants: color `0x050533`, start `-8.0`, and end `96.0`.
- Added explicit fog state to textured section and actor render options.
- Extended textured chunk and actor uniforms with camera position, fog color,
  and fog distances.
- Extended chunk and actor WGSL to pass world position to the fragment shader
  and fog world geometry toward the water fog color by view distance.
- When underwater, the full-frame path clears the background to the water fog
  color instead of drawing the overworld sky dome, then draws chunks, actors,
  the vanilla underwater texture overlay, and GUI/debug as before.

Remaining follow-ups:

- Sample biome water fog colors and add Java's `5000 ms` fog-color transition.
- Replace terrain-MVP full-height fluid checks with Java `FluidState.getHeight`
  parity once exact fluid states are represented in native client block facts.
- Add water-vision ramp, swamp distance multiplier, night vision, conduit
  power, lava fog, powder snow fog, fire overlay, and view-blocking block
  overlay outside this slice.

## Validation

Completed for the first screen-effect slice:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-render -p mclone-native-client
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-waterline-above.png --width 960 --height 540 --seed 12345 --chunk-x -129 --chunk-z -256 --render-distance 2 --screenshot-ui none --screenshot-eye -2049.5,64.5,-4084.5 --fullbright true
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-waterline-under.png --width 960 --height 540 --seed 12345 --chunk-x -129 --chunk-z -256 --render-distance 2 --screenshot-ui none --screenshot-eye -2049.5,62.5,-4084.5 --fullbright true
pnpm native:timedemo:smoke
pnpm native:web:build
pnpm native:web:smoke
git diff --check
```

The waterline validation pair reported:

- `/tmp/mclone-waterline-above.png`: `underwater=false`
- `/tmp/mclone-waterline-under.png`: `underwater=true`

Both captures were inspected. The first-pass Java overlay is deliberately
subtle; the stronger "blue-ish underwater" read should come from the fog
follow-up below, not from increasing the overlay alpha beyond Java constants.

Completed for the fog slice:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-client -p mclone-render -p mclone-native-client
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-fog-above.png --width 960 --height 540 --seed 12345 --chunk-x -129 --chunk-z -256 --render-distance 2 --screenshot-ui none --screenshot-eye -2049.5,64.5,-4084.5 --fullbright true
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-fog-under.png --width 960 --height 540 --seed 12345 --chunk-x -129 --chunk-z -256 --render-distance 2 --screenshot-ui none --screenshot-eye -2049.5,62.5,-4084.5 --fullbright true
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-fog-distance.png --width 960 --height 540 --seed 12345 --chunk-x -129 --chunk-z -256 --render-distance 2 --screenshot-ui none --screenshot-eye -2049.5,58.5,-4084.5 --fullbright true
pnpm native:timedemo:smoke
pnpm native:web:build
pnpm native:web:smoke
git diff --check
```

The fog validation captures reported:

- `/tmp/mclone-fog-above.png`: `underwater=false`
- `/tmp/mclone-fog-under.png`: `underwater=true`
- `/tmp/mclone-fog-distance.png`: `underwater=true`

All three captures were inspected. Above-water rendering keeps the normal sky.
Underwater rendering clears to the water fog color, applies distance fog to
chunks and actors, and keeps the vanilla underwater screen texture overlay.

## Out Of Scope

- Changing water mesh translucency, sorting, culling, or liquid simulation.
- Biome water-color rendering for water surfaces.
- Lava, powder snow, fire, portal, or view-blocking block overlays.
- Full Java `FluidState` parity beyond what is needed to determine current
  camera-in-water status.
