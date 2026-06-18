# 038: Native Light Graph And Block Engine Foundation

Status: completed first pass.

## Purpose

Build on
[`037-native-light-solver-storage-foundation.md`](037-native-light-solver-storage-foundation.md)
by porting Java's incremental fixed-point graph and using it for a first
block-light engine foundation.

This slice should prove that native lighting can brighten and darken through
the same graph repair shape as Java, without trying to wire
`ChunkStatus::Light`, sky-source storage, live deltas, or renderer lightmap
parity yet.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DynamicGraphMinFixedPoint.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/SectionTracker.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/BlockLightSectionStorage.java`

## Scope

Land the first graph-driven block-light path:

- port `DynamicGraphMinFixedPoint` as a reusable native module
- preserve Java's core structures:
  - level count
  - one ordered queue per internal level
  - pending computed levels
  - `firstQueuedLevel`
  - brighten/darken repair behavior
- connect graph callbacks to storage-facing methods without duplicating storage
  state
- add a small `BlockLightEngine` foundation that:
  - owns `LayerLightSectionStorage`
  - samples synthetic block input through a trait
  - uses Java's internal level convention (`0` full light, `15` dark)
  - supports source emission via the `Long.MAX_VALUE` source node shape
  - attenuates by `max(1, opacity)`
  - treats missing chunks/positions as opaque for this foundation
  - leaves face shape occlusion as an explicit later parity gap
- add synthetic fixtures for:
  - lava/torch-style source brightening through air
  - opaque block stopping propagation
  - source removal darkening and repair
  - cross-section propagation

## Out Of Scope

- `SkyLightEngine`
- `SkyLightSectionStorage` source-column bookkeeping
- full `LayerLightEngine` chunk cache and block-state shape hooks
- `LevelLightEngine`
- real server `ChunkStatus::Light`
- live light deltas
- Java face-occlusion shapes
- render `LightTexture`
- ambient occlusion

## Implementation Notes

Keep the graph generic and close to Java. The block engine should call into the
same graph rather than implementing a separate queue/flood-fill.

Expected native module additions:

```text
mclone-light/src/dynamic_graph.rs
mclone-light/src/block_engine.rs
```

Temporary parity gap for this slice: block opacity/emission can come from a
small trait-backed synthetic world. Do not route it through renderer or
worldgen-specific block IDs. A later slice should bridge real block-state
opacity, emission, and face occlusion into this same engine.

## Result

Landed:

- `mclone-light/src/dynamic_graph.rs` ports the Java fixed-point graph shape
  with per-level queues, pending computed levels, `firstQueuedLevel`, queue
  removal, and brighten/darken repair behavior.
- `mclone-light/src/block_engine.rs` adds a first block-light engine foundation
  that owns `LayerLightSectionStorage`, uses Java's inverted internal levels,
  treats `i64::MAX` as the source node, and delegates block emission/opacity to
  a small `BlockLightWorld` trait.
- The block engine propagates through active light sections, attenuates by
  `max(1, opacity)`, treats missing world input as opaque, and leaves
  shape-based face occlusion as an explicit later bridge.
- Synthetic fixtures cover source brightening, opaque section-wall blocking,
  source removal and repair from another source, and active-section boundary
  propagation.

This does not change rendered pixels yet. Runtime chunk publication still uses
the provisional light producer until the graph engine is wired into loaded
chunk/block-state data and then into `ChunkStatus::Light`.

## Validation

Fast focused gates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-light
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-mesh
cargo fmt --manifest-path native/Cargo.toml -p mclone-light -- --check
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Completed on 2026-06-18:

- `cargo test --manifest-path native/Cargo.toml -p mclone-light`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-mesh`
- `cargo fmt --manifest-path native/Cargo.toml -p mclone-light -- --check`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
