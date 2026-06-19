# 053: Native Non-Cubic AO Render Parity

Status: completed first pass.

## Purpose

Continue visual lighting parity after
[`052-native-model-ao-render-parity.md`](052-native-model-ao-render-parity.md)
by porting Java's non-cubic model AO shape weighting. The previous slice applied
`AmbientOcclusionFace` to full cube faces only; this slice lets partial boxes
use Java's `calculateShape(...)` flags and `SizeInfo` weights.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/ModelBlockRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/model/FaceBakery.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/FaceInfo.java`

Important reference shape:

- `calculateShape(...)` derives a 12-entry shape array from baked quad vertex
  bounds: six direction extents plus six flipped extents.
- `calculateShape(...)` sets one flag for face-neighbor sampling and one flag
  for non-cubic weighting.
- `AdjacencyInfo` owns the per-direction `SizeInfo` weight arrays.
- `AmbientOcclusionFace.calculate(...)` uses the weighted branch only when the
  shape flag is set.

## Scope

Landed in this slice:

- Added `AmbientOcclusionShape` and `calculate_ambient_occlusion_shape(...)` in
  `mclone-mesh::ambient_occlusion`.
- Ported Java's `SizeInfo` shape indices and all six `AdjacencyInfo` weight
  tables.
- Generalized `calculate_ambient_occlusion_face(...)` so full cube and partial
  faces share the same helper.
- Removed the builder-side `face.is_full_cube_side()` AO gate; AO-eligible
  partial faces now use shape-aware per-vertex brightness and packed-light
  blending.
- Made flat fallback use shape-derived face-neighbor sampling for unculled
  faces while keeping the existing cullface-neighbor path for culled flat
  faces.
- Added a mesh catalog `collision_shape_full_block` fact for Java's
  `calculateShape(...)` face-neighbor flag.
- Added unit tests for shape flags and Java `SizeInfo` weights, plus a mesh
  integration test proving partial faces receive per-vertex AO.

## Out Of Scope

- Full vanilla block-state render facts. The AO sampler still uses terrain-MVP
  facts from `TexturedMeshCatalog`.
- Liquid light sampling from `LiquidBlockRenderer`.
- Runtime option plumbing for disabling AO independently from lighting.
- Exact dynamic Java lightmap texture effects such as gamma, torch flicker, and
  potion/status overrides.

## Result

Validated screenshot:

```text
/tmp/mclone-light-ao-shape-day.png
```

The capture rendered `960x540`, `166` cached sections, `26` drawn sections, and
nonblank terrain. The scene remains visually stable after switching AO to the
shape-aware path.

## Validation

Completed on 2026-06-19:

- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- touched-file `rustfmt --edition 2024 --check`
- native daytime screenshot:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-light-ao-shape-day.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --lighting true --fullbright false --day-time 6000 --freeze-time`

## Next

The next likely visual parity work is block/fluid render facts:

- replace the terrain-MVP AO render facts with fuller Java block-state facts for
  `getLightBlock`, `isViewBlocking`, `isSolidRender`,
  `isCollisionShapeFullBlock`, and `getLightEmission`
- port liquid light sampling from `LiquidBlockRenderer`
- add targeted fixtures for leaves, glass/ice, snow layers, slabs, plants, and
  fluids as those assets enter the native MVP surface
- decide whether a dedicated `--disable-ao` diagnostic toggle is still useful
