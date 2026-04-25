# Tactical 49 — Vanilla status futures and partial chunks

Replace the generated host's flattened hidden authority terrain window with vanilla-shaped status futures and partial chunk records.

Status: proposed. This tactical should run before more screenshot-throughput tuning and before using decorated parity failures as signal.

## Source files

Read these before writing code:

| Java / reference source | Purpose |
|---|---|
| [`ChunkStatus.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java) | status chain, range values, `STATUS_BY_RANGE`, chunk type |
| [`ChunkMap.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | `schedule(...)`, `scheduleChunkGeneration(...)`, `getDependencyStatus(...)`, range futures |
| [`ChunkHolder.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | one future per status, ticket-level target status, full/ticking promotion |
| [`ChunkAccess.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkAccess.java) | shared proto/full interface and metadata methods |
| [`ProtoChunk.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java) | partial chunk state: status, sections, starts, references, masks, ticks, postprocessing, lights |
| [`LevelChunk.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java) | `FULL` chunk conversion and full-status behavior |
| [`ImposterProtoChunk.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ImposterProtoChunk.java) | proto-shaped wrapper for already-full chunks |
| [`WorldGenRegion.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java) | mixed-status input cache and write cutoff |

The canonical ordering contract is [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md). Do not duplicate or weaken it here.

## Current gap

The current generated runtime has explicit status names and finality gates, but it still prepares dependency inputs with a flattened authority terrain radius. For `viewDistance=1`, that means 625 chunks are advanced to the carved terrain boundary so 81 chunks can run `FEATURES`.

Vanilla does not do that. It requests exact statuses through `ChunkHolder` futures. A `FEATURES` task has range `8`, but its input statuses are mixed:

```text
offset radius 0: parent of FEATURES -> LIQUID_CARVERS
offset radius 1: STATUS_BY_RANGE[2] -> LIQUID_CARVERS
offset radius 2..8: STATUS_BY_RANGE[3..9] -> STRUCTURE_STARTS
```

The outer ring is metadata, not terrain. Promoting it to `LIQUID_CARVERS` is both slower and less faithful.

## Target model

Add host-owned generated chunk records that can represent:

- current requested/completed `ChunkStatus`
- one pending/completed future or job per status, shaped like `ChunkHolder`
- `ProtoChunk`-like partial data: structure starts, structure references, biome container when present, optional block sections, carving masks, ticks, postprocessing, light positions, dirty/unsaved flags
- `LevelChunk`/packed snapshot data only after the `FULL` conversion path

Status requests should be expressed as:

```text
ensureStatus(chunk, status)
  -> ensure parent for center
  -> gather getDependencyStatus(status, radius) for every dependency offset
  -> run the status task with the mixed-status ChunkAccess list
```

The browser/Node scheduler can stay cooperative and host-owned, but the requested statuses and input shapes must match vanilla.

## Implementation plan

1. **Introduce partial chunk records**
   - Keep current `LevelChunk`/snapshot path for full chunks.
   - Add a `GeneratedProtoChunk` or equivalent record for partial statuses and metadata.
   - Preserve current block storage only where a status has actually materialized sections.

2. **Port the status future graph**
   - Add per-status job/future slots on the host-side chunk record.
   - Implement `getDependencyStatus(requestedStatus, radius)` from `ChunkMap`.
   - Implement range-future gathering that returns mixed-status records, not a flat terrain window.

3. **Split existing stage methods behind `ensureStatus(...)`**
   - `STRUCTURE_STARTS` and `STRUCTURE_REFERENCES` remain no-op/metadata placeholders until real structures land, but they must be real statuses.
   - `BIOMES`, `NOISE`, `SURFACE`, `CARVERS`, and `LIQUID_CARVERS` advance only the chunks whose requested status requires them.
   - `FEATURES` receives a `WorldGenRegion`-style mixed-status input cache.

4. **Remove flattened authority terrain preparation**
   - Replace `ensureDecorationTerrainWindow(...)` with status dependency scheduling.
   - Keep 3x3 `FEATURES` stability and 3x3 `FULL` publish gates.
   - Keep cooperative yielding around status jobs, not around fake flattened phases.

5. **Measure again**
   - Run `pnpm perf:worldgen -- --radius 1`.
   - Run the smallest screenshot probe with lighting disabled.
   - Compare terrain and decoration chunk counts against the vanilla status graph.

## Tests

Add focused runtime tests for:

- `getDependencyStatus(FEATURES, 0..8)` returns `LIQUID_CARVERS`, `LIQUID_CARVERS`, then `STRUCTURE_STARTS`.
- A `FEATURES` request does not promote radius 2-8 chunks to terrain statuses.
- `STRUCTURE_REFERENCES` can complete from structure-start metadata without block sections.
- A far dependency chunk can exist as metadata-only and still satisfy a status dependency.
- `LIGHT` still waits for the 3x3 `FEATURES` gate.
- publication still waits for the 3x3 `FULL` gate.

## Validation

Required:

- `pnpm test -- test/runtime/generated-world-boundary.test.ts`
- `pnpm perf:worldgen -- --radius 1`
- `pnpm typecheck`
- `git diff --check`

Run the smallest relevant browser probe after the status graph changes the first visible-publish path.
