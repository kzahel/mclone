# 035: Native Codebase Health Refactor Plan

Status: active parent.

## Purpose

Track preventative refactors for the native Rust workspace before the largest
files become harder to change safely.

The goal is not to rewrite behavior. The first pass for each area should be a
move-only or nearly move-only split that preserves public behavior, oracle
parity, native desktop validation, and the WASM compatibility gate.

## Triggering Finding

Several native Rust files have grown into multi-responsibility modules:

- `native/apps/mclone-native-client/src/main.rs` is over 6k lines and owns CLI
  parsing, headless modes, perf probes, scene/runtime orchestration, asset
  loading, render-section cache management, UI, frame pacing, camera controls,
  and the `winit` app handler.
- `native/crates/mclone-server/src/lib.rs` is over 6k lines and combines
  ticket distance management, chunk holders, chunk scheduling, fluid ticking,
  worldgen mailbox ownership, provisional lighting, integrated-server glue, and
  tests.
- `native/crates/mclone-worldgen/src/feature.rs` is over 4k lines and contains
  feature context, feature-region writes, configuration types, decorators,
  biome tables, lakes, springs, plants, trees, ores, glow lichen, freeze/snow,
  and tests.
- `native/crates/mclone-worldgen/src/levelgen.rs` contains generated-chunk data
  types, mutable chunk buffers, noise settings, `NoiseSampler`,
  `NoiseBasedChunkGenerator`, feature batch caching, timings, and generation
  entry points.
- `native/crates/mclone-mesh/src/lib.rs` and
  `native/crates/mclone-render/src/chunk.rs` are still compact enough to work
  with, but their visibility, culling, mesh-building, GPU-resource, and render
  camera responsibilities are starting to deserve separate modules.

The current shape is workable, but continued feature, lighting, streaming,
runtime mutation, and platform work will make these files harder to review and
more likely to accumulate accidental coupling.

## Reference Shape

Use the Java 1.17.1 source tree as a boundary guide for parity-critical systems:

- `server/level/DistanceManager.java`, `ChunkHolder.java`, `ChunkMap.java`, and
  `ServerChunkCache.java` separate ticket distance propagation, holder state,
  status scheduling, and server chunk publication concerns.
- `world/level/material/FlowingFluid.java`, `WaterFluid.java`,
  `LavaFluid.java`, and `world/level/block/LiquidBlock.java` keep fluid behavior
  separate from chunk scheduling.
- `world/level/levelgen/NoiseBasedChunkGenerator.java` and `NoiseSampler.java`
  are separate concepts; generated chunk data and runtime buffers are not
  merely terrain-generator internals.
- `world/level/levelgen/feature/*`, `feature/trunkplacers/*`,
  `feature/foliageplacers/*`, `feature/featuresize/*`, and
  `levelgen/placement/*` spread the feature pipeline across many small classes.
- `client/renderer/chunk/VisGraph.java`, `VisibilitySet.java`,
  `ChunkRenderDispatcher.java`, and `RenderChunkRegion.java` are useful guides
  for keeping visibility, compile queues, and render-region inputs distinct.

Do not copy Java architecture blindly where native/web/runtime constraints need
a different shape. Use it to avoid hiding separate parity concepts in one Rust
module.

## Refactor Principles

- Prefer move-only module extraction before logic edits.
- Keep public APIs stable unless a small API adjustment is required to make the
  split coherent.
- Do not combine refactors with parity ports, performance rewrites, or renderer
  behavior changes.
- Keep native desktop as the first validation lane, but preserve `mclone-web`
  compilation when shared crates are touched.
- Keep `winit`, native surfaces, and desktop-only frame concerns inside app or
  platform adapters.
- Avoid moving Java-parity math helpers into generic utility modules unless the
  semantic match is exact.
- Preserve oracle fixture tests and rendered-output validation expectations for
  any slice that can affect generated or rendered pixels.

## Tracking Checklist

- [ ] Split `mclone-native-client` into app-local modules:
  `cli` (done), `headless` (done), `perf` (done), `render_cache` (done),
  `camera` (done), `frame_pacing` (done), `ui` (done), `scene_runtime`, and `app`.
- [ ] Split `mclone-server` after active block-delta/fluid mutation work
  stabilizes: `types`, `tickets`, `distance_manager`, `holder`, `scheduler`,
  `worldgen_mailbox`, `fluid`, `integrated`, `lighting_seed`, and `timing`.
- [ ] Split `mclone-worldgen::feature` around Java-shaped concepts:
  `context`, `region`, `configured`, `placed`, `tables`, `lake`, `spring`,
  `ore`, `tree`, `patch`, `glow_lichen`, and `top_layer`.
- [ ] Split `mclone-worldgen::levelgen` into chunk data, settings, sampler,
  generator, feature-batch, and timing modules.
- [ ] Move truly shared chunk/block position primitives into `mclone_core`:
  world block positions, local/chunk coordinate conversion, section indexing,
  and dirty-neighborhood helpers where they are semantic engine contracts.
- [ ] Split `mclone-mesh` into visibility, mesh data, textured catalog, and
  mesh-builder modules.
- [ ] Split `mclone-render::chunk` into camera/view, culling, GPU resources,
  pipelines, depth target, and stats modules.
- [ ] Move large inline test suites into module-local test files where doing so
  improves navigation without weakening fixture coverage.

## Suggested Order

1. Start with `mclone-native-client`. It is the least parity-sensitive large
   file and already has clear internal structs/functions that can become
   modules.
2. Split `mclone-worldgen::feature` and `mclone-worldgen::levelgen` before more
   features, structures, or lighting statuses land.
3. Split `mclone-server` after the section block-delta path and render compile
   priority work are stable, because those active slices touch nearby mutation,
   scheduler, and diagnostics code.
4. Centralize shared primitives only after module extraction exposes repeated
   contracts clearly.
5. Split mesh/render modules before broader lighting, GPU upload budgeting,
   Android, or OpenXR work increases renderer surface area.

## Slice Shape

Each child refactor should have its own tactical or checklist update when it
becomes active. A good child slice should:

- name the module boundary being extracted
- list the files moved or created
- state whether public APIs changed
- state whether behavior is intended to be identical
- include the minimum command set used for validation
- leave performance or parity behavior changes for a separate follow-up

## Validation

Baseline validation for move-only refactors:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Additional validation:

- For worldgen module splits, run the relevant oracle-backed tests and
  `pnpm native:worldgen:smoke`.
- For runtime/server splits, run `pnpm native:movement:smoke` and any active
  fluid/block-delta fixtures.
- For renderer/mesh/app splits that produce pixels, capture and inspect a
  screenshot under `/tmp` before considering the slice complete.
