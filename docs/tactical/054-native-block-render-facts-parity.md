# 054: Native Block Render Facts Parity

Status: completed first pass.

## Purpose

Continue lighting/render parity after
[`053-native-non-cubic-ao-render-parity.md`](053-native-non-cubic-ao-render-parity.md)
by replacing the terrain-MVP AO facts embedded in `TexturedMeshCatalog` with a
small Java-shaped render-facts module. The goal is to let model AO sample
neighbor blocks through facts that match `BlockStateBase.Cache` behavior for the
current native terrain block set, without growing `mclone-mesh/src/builder.rs`.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/state/BlockBehaviour.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Block.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Blocks.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LeavesBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/AbstractGlassBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/TintedGlassBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/IceBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/SnowLayerBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/BushBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/GlowLichenBlock.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/Material.java`

Important reference shape:

- `BlockBehaviour.BlockStateBase.Cache` caches `solidRender`,
  `propagatesSkylightDown`, `lightBlock`, and
  `isCollisionShapeFullBlock`.
- `BlockBehaviour.Properties.noCollission()` and `.noOcclusion()` both set
  `canOcclude=false`.
- `BlockBehaviour.getLightBlock(...)` returns full light blocking for solid
  render blocks, otherwise `0` when skylight propagates and `1` when it does
  not.
- Leaves override `getLightBlock(...)` to `1` and are marked
  `noOcclusion()`.
- `AbstractGlassBlock` propagates skylight and uses shade brightness `1.0`;
  tinted glass overrides light blocking back to `15`.
- `LiquidBlock` does not propagate skylight, so water/lava attenuate by `1`
  even though they do not view-block or occlude.

## Scope

Landed in this slice:

- Added `mclone-mesh/src/render_facts.rs`.
- Moved render/AO facts out of `catalog.rs` and kept the builder using catalog
  accessors only.
- Derived catalog `occludes` from Java-style `canOcclude && full occlusion
  shape`, so full-cube leaves and glass-like models no longer act as opaque
  culling neighbors.
- Matched current terrain-MVP facts for stone-like solids, leaves, air/cave
  air, water/lava, ice/packed ice, glass/tinted/stained glass, snow layer,
  plants, glow lichen, magma block, and lit redstone ore states.
- Added focused tests for Java `BlockStateBase.Cache` facts and a catalog test
  proving a full-cube leaf model remains non-occluding while keeping leaf light
  attenuation.

## Out Of Scope

- Full vanilla block registry generation. The native registry is still the
  terrain-MVP state set, so this module is a hand-maintained current-block
  bridge rather than the final block-state fact source.
- Exact vanilla voxel-shape tables for every future block family. The current
  module still uses baked model full-cube shape as a proxy except for known
  current special cases.
- Liquid mesh light sampling from `LiquidBlockRenderer`.
- Runtime option plumbing for disabling AO independently from lighting.
- Server-side `BlockState.getLightBlock(...)` replacement for all generated
  block opacity. This slice affects renderer/AO facts.

## Result

Validated screenshot:

```text
/tmp/mclone-render-facts-day.png
```

The capture rendered `960x540`, `166` cached sections, `26` drawn sections, and
nonblank terrain. Tree canopies remain visible and no blue-sky-only regression
was observed. Render pressure increased as expected because leaves no longer
cull adjacent hidden solid faces like opaque cubes.

## Performance

Completed on 2026-06-19:

| Lane | Result |
|---|---:|
| radius-5 lighting-enabled total | `1,947.392 ms` |
| radius-5 light-status compute | `456.666 ms` |
| radius-5 `LevelLightEngine.run_all_updates` | `403.441 ms` |
| radius-5 sky processed nodes | `794,963` |
| release movement-frame over budget | `0 / 240` |
| release movement-frame p95 / p99 / max | `4.128 / 5.889 / 8.250 ms` |
| release timedemo average / max frame | `2.739 / 13.450 ms` |
| release timedemo face count | `405,407` |

Interpretation: the render-facts change increases mesh face/index pressure but
does not regress the retained lighting graph startup lane or the release
movement-frame budget on the current host. Timedemo remains within the existing
render performance envelope, but the face-count increase is now part of the
new baseline for Java-style leaf non-occlusion.

## Validation

Completed on 2026-06-19:

- `cargo test --manifest-path native/Cargo.toml -p mclone-mesh`
- `cargo test --manifest-path native/Cargo.toml -p mclone-render`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- touched-file `rustfmt --edition 2024 --check`
- `pnpm --silent native:movement-frame:smoke`
- `pnpm --silent native:timedemo:smoke`
- release radius-5 scheduler lighting probe:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting`
- release movement-frame probe:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4`
- release timedemo:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --timedemo --timedemo-frames 240`
- native daytime screenshot:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --screenshot /tmp/mclone-render-facts-day.png --width 960 --height 540 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --lighting true --fullbright false --day-time 6000 --freeze-time`

Note: full-workspace `cargo fmt --all --check` still reports pre-existing
formatting drift in untouched `mclone-render` sky files. Touched files were
formatted and checked directly.

## Next

The next likely visual parity work is liquid renderer lighting:

- read and port the light-sampling portions of
  `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/LiquidBlockRenderer.java`
- keep liquid mesh/light sampling separate from block model AO
- add liquid-specific screenshot/perf checks because water/lava now have
  Java-style render facts but still do not use Java's liquid quad light rules
