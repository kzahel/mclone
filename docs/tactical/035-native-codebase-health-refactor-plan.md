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
- `native/apps/mclone-native-client/src/main.rs` has been reduced from over 6k
  lines, but still owns `ChunkApp`, `ApplicationHandler`, frame composition,
  render stream stats, input helpers, render upload stats, and benchmark
  metadata helpers.
- `native/apps/mclone-native-client/src/scene_runtime.rs` is now a useful
  boundary but is already large enough that later runtime/streaming growth
  should avoid turning it into the next app-local monolith.

The current shape is workable, but continued feature, lighting, streaming,
runtime mutation, and platform work will make these files harder to review and
more likely to accumulate accidental coupling.

## Current Audit Notes

After the first native-client module splits, the largest current Rust files are:

- `native/crates/mclone-server/src/lib.rs`: still over 6k lines.
- `native/crates/mclone-worldgen/src/feature.rs`: still over 4k lines.
- `native/crates/mclone-worldgen/src/levelgen.rs`: still over 3.5k lines.
- `native/crates/mclone-mesh/src/lib.rs` and
  `native/crates/mclone-render/src/chunk.rs`: both near or over 2k lines.
- `native/apps/mclone-native-client/src/main.rs`,
  `native/apps/mclone-native-client/src/scene_runtime.rs`, and
  `native/apps/mclone-native-client/src/perf.rs`: still large app-local modules
  even after the initial extraction pass.

The broad direction is right, but the codebase is not yet shaped like the
reference engine. The most important remaining issue is not just file size; it
is that separate engine concepts are still sharing files and some foundational
chunk/block coordinate contracts are duplicated instead of living in
`mclone_core`.

Concrete duplication found during the audit:

- `mclone_core` owns `CHUNK_WIDTH`, `SECTION_HEIGHT`,
  `CHUNK_SECTION_VOLUME`, `AIR_BLOCK_STATE_ID`, `ChunkPos`,
  `ChunkSnapshot`, and `chunk_section_index`, but not all consumers use it as
  the single source of truth.
- `mclone_mesh` still defines its own `CHUNK_WIDTH = 16` and
  `RENDER_SECTION_HEIGHT = 16`.
- `mclone_worldgen::feature` still defines a local `CHUNK_WIDTH = 16`.
- Server, worldgen, mesh, render, native camera, and scene runtime each have
  local variants of block-to-chunk, local-block, chunk-origin, section-center,
  or block-index helpers.
- Benchmark/reporting helpers such as `elapsed_ms`, `micros_to_ms`,
  `git_short_commit`, `git_dirty`, and `json_escape` are repeated across
  multiple binaries/modules.

Do not centralize every helper blindly. A shared helper should move only when
it represents the same semantic contract across call sites. Renderer-only GPU
layout constants, UI widget IDs, local tuning defaults, and Java-parity math
that intentionally mirrors a specific source class should remain local.

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

- [x] Split `mclone-native-client` into app-local modules:
  `cli` (done), `headless` (done), `perf` (done), `render_cache` (done),
  `camera` (done), `frame_pacing` (done), `ui` (done), `scene_runtime` (done), and `app` (done).
- [x] Move native-client app/window ownership into `app.rs`:
  `ChunkApp`, `ApplicationHandler`, native surface/depth/draw/gui ownership,
  mouse-lock handling, input-to-camera wiring, frame composition, and redraw
  scheduling. Keep CLI dispatch in `main.rs`.
- [ ] Move native-client frame/render reporting into a small app-local module
  after `app.rs`: `RenderStreamStats`, full-frame summaries, render update stat
  recording, and benchmark metadata helpers if they remain app-local.
- [ ] Centralize exact shared chunk/block primitives in `mclone_core`:
  block-to-chunk conversion, local block coordinate conversion, chunk-local
  block indexing, chunk/world origins, and world block position types where
  dependency direction allows it.
- [ ] Replace duplicate dimension constants with `mclone_core` exports where
  semantics match exactly: especially `mclone_mesh::CHUNK_WIDTH`,
  `mclone_mesh::RENDER_SECTION_HEIGHT`, and `mclone_worldgen::feature`'s local
  `CHUNK_WIDTH`.
- [ ] Extract shared benchmark/report helpers into an appropriate native utility
  module or crate if they stay useful across binaries: elapsed/micros
  conversion, git metadata, JSON string escaping, and square-count helpers.
- [ ] Split `mclone-server` after active block-delta/fluid mutation work
  stabilizes: `types`, `tickets`, `distance_manager`, `holder`, `scheduler`,
  `worldgen_mailbox`, `fluid`, `integrated`, `lighting_seed`, and `timing`.
- [ ] Split `mclone-worldgen::feature` around Java-shaped concepts:
  `context`, `region`, `configured`, `placed`, `tables`, `lake`, `spring`,
  `ore`, `tree`, `patch`, `glow_lichen`, and `top_layer`.
- [ ] Split `mclone-worldgen::levelgen` into chunk data, settings, sampler,
  generator, feature-batch, and timing modules.
- [ ] Split `mclone-mesh` into visibility, mesh data, textured catalog, and
  mesh-builder modules.
- [ ] Split `mclone-render::chunk` into camera/view, culling, GPU resources,
  pipelines, depth target, and stats modules.
- [ ] Move large inline test suites into module-local test files where doing so
  improves navigation without weakening fixture coverage.

## Suggested Order

1. Finish the native-client `app.rs` split. This completes the current
   app-local module extraction pass and leaves `main.rs` close to CLI dispatch.
2. Do a focused shared-core primitive pass. Move only exact shared
   chunk/block/section coordinate contracts into `mclone_core`, update the
   duplicated constants/helpers, and keep behavior identical.
3. Split `mclone-worldgen::feature` and `mclone-worldgen::levelgen` before more
   features, structures, or lighting statuses land.
4. Split `mclone-server` after the section block-delta path and render compile
   priority work are stable, because those active slices touch nearby mutation,
   scheduler, and diagnostics code.
5. Split mesh/render modules before broader lighting, GPU upload budgeting,
   Android, or OpenXR work increases renderer surface area.
6. Move large inline test suites into module-local test files as a navigation
   cleanup after the production module boundaries are clearer.

## High Priority Queue

1. `native-client app.rs` (done)
   - Mechanical extraction of `ChunkApp`, `ApplicationHandler`, surface/window
     ownership, input dispatch, frame redraw scheduling, and mouse-lock logic.
   - Expected behavior change: none.
   - Validation: native-client tests plus a full-frame screenshot under `/tmp`.

2. `mclone_core` shared coordinate primitives
   - Add exact shared helpers/types for chunk/block coordinate conversion and
     local block indexing.
   - Replace duplicate constants and helpers in mesh, worldgen feature, server,
     native camera, render, and scene runtime where semantics match exactly.
   - Expected behavior change: none.
   - Validation: full native workspace tests/checks, WASM check, and worldgen
     smoke because shared primitives touch parity-sensitive paths.

3. `worldgen::feature` first split
   - Start with low-risk move-only boundaries: `context`, `region`,
     `configured`, `placed`, and `tables`.
   - Leave individual feature behavior (`lake`, `spring`, `ore`, `tree`,
     `patch`, `glow_lichen`, `top_layer`) for follow-up slices.

4. `worldgen::levelgen` first split
   - Separate generated chunk data and mutable chunk buffers from settings,
     sampler, generator, feature-batch cache, and timing.

5. `mclone-server` first split
   - Split stable support modules first: `types`, `timing`, and
     `worldgen_mailbox`.
   - Then split ticket/distance/holder/scheduler/fluid once nearby active
     mutation work is stable.

## Follow-Up Queue

- Split `native-client perf.rs` into movement smoke, timedemo, frame-budget
  probe, reporting, and camera path helpers.
- Split `mclone_mesh` into visibility graph/set, mesh data, block lookup/input
  adapters, textured catalog, and builders.
- Split `mclone_render::chunk` into camera/view, traversal/culling, GPU mesh
  resources, pipelines, depth target, and stats/upload reporting.
- Consider a small benchmark/reporting utility only if JSON/git/time helpers
  remain duplicated after the app/perf/server/worldgen splits.
- Move large inline test suites into adjacent `tests` modules/files once
  production code boundaries are stable enough that test moves are not masking
  behavior changes.

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
