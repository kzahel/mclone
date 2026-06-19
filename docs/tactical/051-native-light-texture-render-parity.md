# 051: Native LightTexture Render Parity

Status: completed first pass.

## Purpose

Continue visual lighting parity after
[`050-native-leaf-sky-render-parity.md`](050-native-leaf-sky-render-parity.md)
by replacing the textured chunk shader's linear `max(block, sky) / 15`
brightness factor with Java's `LightTexture` lightmap curve.

This slice keeps packed light ownership in the mesh/client path and keeps the
lightmap interpretation in `mclone-render`.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LightTexture.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/dimension/DimensionType.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/ModelBlockRenderer.java`

Important reference shape:

- `LightTexture.pack(block, sky)` uses the same packed layout native already
  carries: block in bits `4..`, sky in bits `20..`.
- `DimensionType.fillBrightnessRamp(ambientLight)` maps raw light levels through
  `level / (4 - 3 * level)` for overworld ambient light `0`.
- `ClientLevel.getSkyDarken(...)` produces the clear-weather day/night sky
  darken term used by `LightTexture`.
- `ModelBlockRenderer.tesselateWithoutAO(...)` already uses one packed
  lightmap value per flat face; native still follows that flat path.

## Scope

Landed in this slice:

- Added `mclone-render/src/light_texture.rs` with a pure Rust first port of:
  - packed block/sky light extraction
  - overworld brightness ramp
  - clear-weather sky-darken curve
  - default `LightTexture` RGB multiplier, excluding dynamic effects
- Extended textured chunk render options/uniforms with `sky_darken`.
- The native full-frame app path now passes sky darken from current day time to
  chunk rendering.
- Replaced the textured WGSL shader's scalar linear brightness with the Java
  lightmap RGB curve.
- Kept fullbright as an explicit render override.

## Out Of Scope

- Exact Java 16x16 dynamic GPU lightmap texture allocation. The shader computes
  the same stable curve procedurally for now.
- Torch flicker, gamma, night vision, conduit power, boss-world darkening, rain,
  thunder, and lightning flash terms.
- `ModelBlockRenderer.AmbientOcclusionFace` per-vertex brightness and packed
  light blending.
- Liquid light sampling.

## Result

Validated screenshots:

```text
/tmp/mclone-light-lightmap-day.png
/tmp/mclone-light-lightmap-night.png
```

Both captures rendered `960x540`, `166` cached sections, `26` drawn sections,
and nonblank terrain. Daylight remains close to the prior fixed screenshot
because Java full sky is almost white. The nighttime capture visibly darkens
terrain using the Java sky-darken/lightmap path while preserving the same
packed light payloads.

## Validation

Completed on 2026-06-19:

- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- native daytime screenshot:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-light-lightmap-day.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --lighting true --fullbright false --day-time 6000 --freeze-time`
- native nighttime screenshot:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-light-lightmap-night.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --lighting true --fullbright false --day-time 18000 --freeze-time`

## Next

The next likely visual parity slice is Java model ambient occlusion:

- port `ModelBlockRenderer.AmbientOcclusionFace` into a small mesh-side module
- preserve the current flat path for non-AO blocks and debugging
- add synthetic mesh tests for per-vertex brightness/lightmap differences
- capture daylight screenshots that should finally differ meaningfully from
  forced fullbright on open terrain

Keep AO neighbor sampling out of the shader and out of a monolithic mesh
builder body; it belongs in a small Java-shaped helper used by textured mesh
construction.
