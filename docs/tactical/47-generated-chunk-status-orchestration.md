# Tactical 47 — Generated chunk status orchestration

Replace the current generated/decorated/published lifecycle with an explicit vanilla-shaped chunk-status orchestration model, so later parity work can rely on deterministic generation order instead of ad hoc radius and readiness checks.

This tactical intentionally precedes [`46-full-decorated-spawn-chunk-parity.md`](46-full-decorated-spawn-chunk-parity.md). Tactical 46 should run on top of the `FEATURES`, initial lighting, and publication gates made explicit here.

Status: implemented for explicit status names, finality gates, and publish/light checks. Follow-up required: the current runtime still uses a flattened hidden authority terrain window for some dependency preparation. That must be replaced with vanilla-shaped status futures and partial `ProtoChunk`-like records before treating the scheduler as parity-complete.

Post-implementation finding, 2026-04-25: [`worldgen-deterministic-order.md`](../worldgen-deterministic-order.md) now documents the missing metadata-only path. In vanilla, a `FEATURES` range-8 dependency list is not all `LIQUID_CARVERS`; the center plus radius `1` are `LIQUID_CARVERS`, while radii `2..8` are only `STRUCTURE_STARTS`. The current `GeneratedRenderLevel.ensureDecorationTerrainWindow(...)` shape is therefore an over-generation shortcut and should be replaced, not optimized in place.

## Source files (read before writing)

| Java / reference source | TS target |
|---|---|
| [`ChunkStatus.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java) | new generated status model, likely near `src/world/level/generated-render-level.ts` or `src/runtime/host/` |
| [`ChunkMap.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) (`getChunkRangeFuture(...)`, `scheduleChunkGeneration(...)`, `prepareTickingChunk(...)`) | `src/runtime/host/generated-world-host.ts` |
| [`ChunkHolder.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | host-owned per-chunk job/status records |
| [`WorldGenRegion.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java) | `src/world/level/generated-decoration-region.ts` |
| [`ChunkGenerator.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java) | `src/worldgen/levelgen/noise-based-chunk-generator.ts`, `src/world/level/generated-render-level.ts` |
| [`ThreadedLevelLightEngine.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java) and `ChunkStatus.LIGHT` | `src/runtime/lighting/lighting-protocol.ts`, `src/runtime/lighting/lighting-worker.ts`, `src/runtime/host/generated-world-host.ts` |
| [`ClientChunkCache.java`](../../reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java), [`LevelRenderer.java`](../../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java) | snapshot publication and render-world readiness tests |

Read [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md) first. It owns the ordering contract this tactical implements. [`../authoritative-host-scheduling.md`](../authoritative-host-scheduling.md) owns responsiveness and queue policy; do not duplicate a second order model there.

## Current problem

The current generated-world path has useful pieces, but the lifecycle is still not vanilla-shaped:

- `GeneratedRenderLevel.generateChunkTerrain(...)` currently runs terrain, surface, and carvers as one terrain step.
- `GeneratedRenderLevel` tracks only `decoratedChunks` / `decoratingChunks`, not `ChunkStatus`.
- `GeneratedDecorationRegion` correctly models the `FEATURES` read radius `8` and write cutoff `1`, but only around the current decoration call.
- `GeneratedWorldHost` publishes chunks after local decoration plus accepted light, but it does not have an explicit `3x3 FULL` publication gate.
- Initial lighting requests require 3x3 light-input chunks, but the host does not yet express the stronger rule that those inputs correspond to completed `FEATURES` for the whole 3x3 neighborhood.
- Structure statuses do not exist yet, so future structures would have to retrofit the pipeline while also porting structure-specific logic.
- The post-47 implementation added status labels but still flattens part of the dependency graph into an eager terrain authority radius. Vanilla schedules mixed-status futures and permits metadata-only chunks in the outer dependency rings.

The practical risk is that tactical 46 can burn down mismatches against an ordering model that is still too weak. We need the order model first.

## Scope

In scope:

- add explicit generated chunk statuses for the vanilla 1.17.1 chain
- implement status progression for the stages already supported by the codebase
- add no-op placeholder stages for `STRUCTURE_STARTS`, `STRUCTURE_REFERENCES`, `SPAWN`, and `HEIGHTMAPS`
- make `FEATURES` completion a real per-chunk status
- make `decoration_stable(C)` mean `FEATURES` complete for C and its 8 immediate neighbors
- make initial lighting requests require `decoration_stable(C)`
- make publishability require the documented `3x3 FULL` gate, or explicitly encode and test a temporary divergence if full vanilla gating is too large for the current runtime path
- keep hidden authority chunks resident separately from chunks sent to clients
- replace flattened authority terrain generation with status requests whose dependency inputs match `ChunkMap.getDependencyStatus(...)`
- represent metadata-only `STRUCTURE_STARTS` / `STRUCTURE_REFERENCES` inputs without materializing block sections
- keep host-owned deterministic status advancement; workers may compute results but must not directly mutate neighboring authoritative chunks
- add tests that fail if light or publication happens from weaker inputs

Out of scope:

- exact decorated block parity for seed `12345`, chunk `(0, 0)`; that remains tactical 46
- porting real structure starts, references, pieces, templates, jigsaw, or `Beardifier`
- changing feature placement logic unless required to preserve status boundaries
- implementing vanilla chunk NBT status persistence or partial-status resume
- broad DistanceManager/player-ticket parity
- client-side movement prediction, push transport, `SharedArrayBuffer`, or render-world subworker changes

## Target model

Introduce a host-visible generated status model shaped like:

```text
EMPTY
  -> STRUCTURE_STARTS        // no-op placeholder in this slice
  -> STRUCTURE_REFERENCES    // no-op placeholder in this slice
  -> BIOMES                  // may be implicit while biome storage remains derived
  -> NOISE
  -> SURFACE
  -> CARVERS
  -> LIQUID_CARVERS
  -> FEATURES
  -> LIGHT
  -> SPAWN                  // no-op placeholder in this slice
  -> HEIGHTMAPS             // no-op placeholder in this slice
  -> FULL
```

The implementation can keep current chunk storage and generated-buffer details, but status transitions should be explicit enough that tests can ask:

```text
status(C) >= FEATURES
featuresStable(C) = status(N) >= FEATURES for every N in C's 3x3
status(C) >= LIGHT only after featuresStable(C)
status(C) >= FULL only after LIGHT plus local post-light no-op stages
publishable(C) = status(N) >= FULL for every N in C's 3x3
```

If we temporarily decide not to require `3x3 FULL` for normal browser publication, the code and tests must name that as a deliberate parity gap. It should not remain an accidental consequence of `decoratedChunks`.

## Implementation plan

1. **Add status vocabulary and chunk records**
   - Add a small generated-status enum/type matching the vanilla status names.
   - Add host-owned per-chunk records with current status, revision, loaded/generated source, and publication state.
   - Keep existing `LevelChunk` ownership in the generated-world host; do not move mutable chunk ownership into lighting or render workers.

2. **Split terrain generation into explicit stage advancement**
   - Advance `NOISE`, `SURFACE`, `CARVERS`, and `LIQUID_CARVERS` through named methods or a single method that records each status boundary.
   - Keep current `NoiseBasedChunkGenerator` calls in vanilla order.
   - Add `STRUCTURE_STARTS` and `STRUCTURE_REFERENCES` as no-op statuses before `BIOMES` / `NOISE`.

3. **Replace `decoratedChunks` with `FEATURES` status**
   - `decorateChunk(...)` / cooperative decoration should become `ensureStatus(chunk, FEATURES)` or equivalent.
   - Preserve `GeneratedDecorationRegion`'s range-8 input cache and write cutoff `1`.
   - Do not treat the whole range-8 input cache as terrain. For `FEATURES`, vanilla only requires the center and immediate neighbors at `LIQUID_CARVERS`; the outer ring is `STRUCTURE_STARTS` metadata.
   - Prevent recursive decoration caused by reads through `getChunk(...)`.

4. **Add dependency queries**
   - Add helpers for Chebyshev status windows:
     - `hasStatus(chunk, status)`
     - `hasStatusWindow(center, radius, status)`
     - `featuresStable(center)`
     - `publishable(center)`
   - Use these helpers instead of open-coded `decoratedChunks`, `publishedChunkSnapshots`, and lighting-neighbor checks where the logic is status-based.

5. **Gate initial lighting on `FEATURES` stability**
   - Before sending `request_initial_light`, require the target plus its 8 neighbors to be at `FEATURES`.
   - Upsert light input only from chunks whose current revision corresponds to at least `FEATURES`.
   - Preserve revision invalidation when live mutations affect chunks and neighboring light.

6. **Gate `FULL` and publication**
   - Mark a chunk `LIGHT` only after accepted current light exists for the chunk.
   - Advance through no-op `SPAWN` and `HEIGHTMAPS`, then mark `FULL`.
   - Publish a normal chunk snapshot only when the chosen publication policy is satisfied:
     - vanilla target: `3x3 FULL`
     - temporary divergence: explicit, named, tested, and documented in this tactical's final notes

7. **Keep storage honest**
   - Do not claim persisted partial-status resume in this slice.
   - Loaded generated-cache chunks can enter at a coarse "full snapshot loaded" status if that matches current storage, but the status record must make this explicit.
   - Do not persist trusted light unless the existing light-correct policy says it is safe.

8. **Update diagnostics and docs**
   - Rename progress stages from generic "Generating missing chunks" / "Decorating new chunks" where useful to status-oriented language.
   - Update [`../loading-persistence.md`](../loading-persistence.md), [`../worker-ownership.md`](../worker-ownership.md), and [`../worldgen-status.md`](../worldgen-status.md) if the implemented model differs from this draft.

## Suggested tests

Unit/runtime tests should cover the order, not just final pixels:

- A chunk can reach `FEATURES` with the vanilla mixed-status input window: center/radius-1 at `LIQUID_CARVERS`, outer radius 2-8 at `STRUCTURE_STARTS`.
- A `STRUCTURE_STARTS`-only dependency chunk is not silently promoted to terrain just because it sits in a `FEATURES` dependency square.
- A write outside `FEATURES_WRITE_RADIUS_CUTOFF` is rejected and does not mutate the far chunk.
- A chunk cannot request or accept initial light until its 3x3 `FEATURES` neighborhood is complete.
- A chunk with local `FEATURES` but incomplete neighboring `FEATURES` is not treated as `LIGHT`-eligible.
- A chunk with local `LIGHT` but incomplete neighboring `FULL` is not treated as vanilla-publishable.
- Hidden authority chunks can be retained and advanced without being sent to the client.
- Moving the view cancels or ignores stale status/light/publish results by revision.
- Storage-loaded chunks enter an explicit status path and do not silently bypass readiness checks.

Likely existing tests to extend:

- `test/runtime/generated-world-boundary.test.ts`
- `test/runtime/remote-world-transport.test.ts`
- lighting worker tests around `request_initial_light` readiness, if present
- any current generated-world host tests that assert snapshot counts or publish timing

## Validation

Required:

- `pnpm test -- test/runtime/generated-world-boundary.test.ts`
- targeted runtime/lighting tests added by this slice
- `pnpm typecheck`
- `git diff --check`

Run broader checks if touched paths require it:

- `pnpm test -- test/runtime/remote-world-transport.test.ts`
- `pnpm test:browser`
- `pnpm perf:d5` if chunk-view scheduling, polling cadence, or host responsiveness changes materially

Browser visual probes are not the primary validation for this slice. Run the smallest relevant probe only if publication timing changes rendered behavior.

## Done when

- generated chunks have explicit status records instead of only `decoratedChunks`
- `STRUCTURE_STARTS` and `STRUCTURE_REFERENCES` exist as no-op placeholders in the order
- status records can represent metadata-only partial chunks separately from materialized block terrain
- `FEATURES` completion, 3x3 `FEATURES` stability, initial `LIGHT`, `FULL`, and publishability are separate, testable states
- lighting cannot consume weaker-than-3x3-`FEATURES` inputs as final initial light
- publication cannot accidentally bypass the chosen `FULL`/neighbor policy
- existing runtime chunk streaming remains responsive under cooperative scheduling
- tactical 46 is updated or re-enabled only after this status foundation is landed

## Next

After this lands, return to [`46-full-decorated-spawn-chunk-parity.md`](46-full-decorated-spawn-chunk-parity.md). The exact decorated-chunk diff will then run on top of a deterministic order model that can support later structures and lighting parity.
