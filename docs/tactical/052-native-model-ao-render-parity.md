# 052: Native Model AO Render Parity

Status: completed first pass.

## Purpose

Continue visual lighting parity after
[`051-native-light-texture-render-parity.md`](051-native-light-texture-render-parity.md)
by moving Java block-model ambient occlusion into the mesh stage. The goal is
to stop treating every textured face as one flat brightness/light value.

This slice keeps neighbor sampling and packed-light blending in `mclone-mesh`
and leaves `mclone-render` responsible only for interpreting packed light
through the lightmap curve.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/ModelBlockRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/FaceBakery.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/FaceInfo.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/state/BlockBehaviour.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LeavesBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/AbstractGlassBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Blocks.java`

Important reference shape:

- Java gates AO with `Minecraft.useAmbientOcclusion()`,
  `state.getLightEmission() == 0`, and `model.useAmbientOcclusion()`.
- `AmbientOcclusionFace.calculate(...)` samples side and corner neighbors,
  averages shade brightness, and blends packed light per vertex.
- `ClientLevel.getShade(...)` uses clear-overworld directional shade constants:
  down `0.5`, up `1.0`, north/south `0.8`, west/east `0.6`.
- `FaceBakery` / `FaceInfo` define the baked-quad vertex order that
  `AmbientVertexRemap` writes into.
- Leaves are `noOcclusion`, `isViewBlocking(never)`, and `getLightBlock(...)`
  returns `1`; they must not be treated like stone by AO neighbor checks.

## Scope

Landed in this slice:

- Added `mclone-mesh/src/ambient_occlusion.rs` as a small Java-shaped helper for
  cubic/full block-model faces.
- Ported the core `AmbientOcclusionFace` side/corner neighbor sampling,
  directional shade, vertex remap, and packed-light blending for full cube
  faces.
- Preserved baked model `ambientocclusion` metadata, including parent-model
  inheritance.
- Added mesh catalog render facts for the AO sampler: light emission,
  light-block amount, view blocking, solid-render, and shade brightness.
- Kept existing flat rendering as the fallback for non-AO models, emissive
  blocks, and non-full-cube faces.
- Added unit and mesh integration tests for per-vertex AO brightness, model AO
  flag fallback, Java shade constants, and retained packed-light behavior.

## Out Of Scope

- Java's non-cubic `calculateShape(...)` shape-weight branch for partial boxes.
- Full vanilla block-state render facts; native derives a limited terrain-MVP
  fact set in `TexturedMeshCatalog` for now.
- Liquid light sampling from `LiquidBlockRenderer`.
- Runtime option plumbing for disabling AO independently from lighting.

## Result

Validated screenshot:

```text
/tmp/mclone-light-ao-day.png
```

The capture rendered `960x540`, `166` cached sections, `26` drawn sections,
and nonblank terrain. Daylight terrain now has visible per-face/corner shading
from mesh-side AO while still using the Java `LightTexture` shader curve.

## Validation

Completed on 2026-06-19:

- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- `cargo test --manifest-path native/Cargo.toml -p mclone-assets`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- touched-file `rustfmt --edition 2024 --check`
- native daytime screenshot:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-light-ao-day.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --lighting true --fullbright false --day-time 6000 --freeze-time`

## Next

The next likely visual parity slice is the remaining Java AO shape work:

- port `ModelBlockRenderer.calculateShape(...)` and the non-cubic
  `SizeInfo` weight arrays into `mclone-mesh::ambient_occlusion`
- add synthetic tests for slabs/partial boxes and unculled faces
- replace the terrain-MVP render facts with fuller Java block-state facts where
  visual parity needs them
- decide whether to add a dedicated `--disable-ao` diagnostic toggle

Keep this work inside the mesh-side AO helper and catalog facts. The builder
should stay as orchestration, not become the home for Java AO math.
