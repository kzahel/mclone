# Tactical 49 — Vanilla status futures and partial chunks

Replace the generated host's view-level status batching with vanilla-shaped status futures and partial chunk records.

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

The current generated runtime has explicit status names, finality gates, and a TypeScript copy of vanilla's dependency-status rule. It no longer needs to promote the full `FEATURES` radius-8 dependency square to terrain: a radius-1 performance run generated `121` terrain chunks and `81` `FEATURES` chunks to publish `25` chunks. That is the expected materialized write window for the current all-`5x5`-snapshots-are-normal-publication policy.

The remaining gap is not the radius-8 metadata ring. The remaining gap is that the host still schedules a whole view-level job list, keeps only coarse status records, and persists generated data only after a publishable snapshot is built. Vanilla requests exact statuses through `ChunkHolder` futures. A `FEATURES` task has range `8`, but its input statuses are mixed:

```text
offset radius 0: parent of FEATURES -> LIQUID_CARVERS
offset radius 1: STATUS_BY_RANGE[2] -> LIQUID_CARVERS
offset radius 2..8: STATUS_BY_RANGE[3..9] -> STRUCTURE_STARTS
```

The outer ring is metadata, not terrain. Promoting it to `LIQUID_CARVERS` would be both slower and less faithful; the current runtime should preserve the existing metadata-only behavior while replacing the view-level batch runner with vanilla-shaped holder/status futures.

## Boundary-crossing acceptance counts

The target is not "only five generated chunks" for the current radius-0 host policy. The current policy publishes a 5x5 square as normal chunk data. For a normal published radius `P = 2`, vanilla-shaped closure requires:

| State | Cold count | One-chunk axis move, warm count |
|---|---:|---:|
| published snapshots | `25` | `5` new |
| `FULL` chunks | `49` | `7` new |
| `FEATURES` chunks | `81` | `9` new |
| materialized terrain through `LIQUID_CARVERS` | `121` | `11` new |
| metadata `STRUCTURE_STARTS` records | `625` | `25` new |

The focused runtime test [`generated-world-host-chunk-crossing.test.ts`](../../test/runtime/generated-world-host-chunk-crossing.test.ts) records the current flat-world chunk-crossing shape. Tactical 49 should keep these closure counts when all 5x5 chunks are normal publications, while improving the reuse guarantees:

- each chunk/status has at most one in-flight job
- repeated requests coalesce to the existing pending/completed status future
- route changes do not discard partial status work earlier than the vanilla-equivalent unload/save boundary
- metadata-only records stay metadata-only instead of being promoted to terrain sections

Tactical [`57`](57-generated-chunk-holder-status-futures.md) has landed the first holder/coalescing layer around the existing host phases. The remaining Tactical 49 scheduler work is to invert view changes into recursive `ensureStatus(pos, status)` requests and then persist partial state through Tactical [`58`](58-generated-protochunk-partial-state-and-persistence.md).

If a later UX policy wants only the five newly visible chunks to materialize terrain on a one-chunk move, it must explicitly split normal published/ticking chunks from a weaker visual/cache halo. That is not the same acceptance target as vanilla publication parity.

## Vanilla reuse and coalescing

Vanilla's reuse model is `ChunkHolder`, not "current view job":

- one `CompletableFuture` slot per `ChunkStatus` lives on the holder ([`ChunkHolder.java:52`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkHolder.java:53`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java))
- `getOrScheduleFuture(status, chunkMap)` returns the existing future unless that future already completed as an unloaded/failure result; otherwise it schedules exactly one new status future and stores it back into the slot ([`ChunkHolder.java:247`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkHolder.java:249`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkHolder.java:253`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkHolder.java:259`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkHolder.java:261`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java))
- `ChunkMap.schedule(...)` recursively requests the parent status or schedules generation for the requested status; the task input is the mixed-status dependency list, not a prebuilt view batch ([`ChunkMap.java:450`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:459`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:467`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:512`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:514`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java))
- `chunkToSave` tracks the latest generated `ChunkAccess`, so partial `ProtoChunk` progress is eligible for later save/unload handling ([`ChunkHolder.java:59`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkHolder.java:260`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkHolder.java:268`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java), [`ChunkHolder.java:273`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java))
- when a holder is dropped, vanilla waits for `chunkToSave` to settle and saves the resulting chunk access if present; `ProtoChunk.setStatus(...)` marks the chunk unsaved, and `ChunkMap.save(...)` writes unsaved proto chunks except for vanilla's explicit empty/existing-full skip cases ([`ChunkMap.java:411`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:412`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:423`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ProtoChunk.java:265`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`ProtoChunk.java:267`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java), [`ChunkMap.java:627`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:636`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:648`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java))

Optimization rule: do not add a throughput shortcut unless it can be described as one of those vanilla mechanisms in browser/Node form. Per-chunk/status in-flight coalescing, status-request reprioritization, and partial `ProtoChunk` save/resume are valid. Discarding partial chunks for speed, weakening the `3x3 FULL` publication gate, or treating a normal published chunk as final after only a `7x7` `FEATURES` window are not valid.

## Target model

Add host-owned generated chunk records that can represent:

- current requested/completed `ChunkStatus`
- one pending/completed future or job per status, shaped like `ChunkHolder`
- `ProtoChunk`-like partial data: structure starts, structure references, biome container when present, optional block sections, carving masks, ticks, postprocessing, light positions, dirty/unsaved flags
- a `chunkToSave`-style latest generated partial/full record for save and unload
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
   - Track dirty/unsaved state and the latest generated partial/full record separately from "published snapshot".

2. **Port the status future graph**
   - Add per-status job/future slots on the host-side chunk record.
   - Implement `getDependencyStatus(requestedStatus, radius)` from `ChunkMap`.
   - Implement range-future gathering that returns mixed-status records, not a flat terrain window.
   - Reuse the same in-flight status job for duplicate requests instead of scheduling another view-level runner.

3. **Split existing stage methods behind `ensureStatus(...)`**
   - `STRUCTURE_STARTS` and `STRUCTURE_REFERENCES` remain no-op/metadata placeholders until real structures land, but they must be real statuses.
   - `BIOMES`, `NOISE`, `SURFACE`, `CARVERS`, and `LIQUID_CARVERS` advance only the chunks whose requested status requires them.
   - `FEATURES` receives a `WorldGenRegion`-style mixed-status input cache.

4. **Remove view-level batch preparation**
   - Replace view-level terrain/decorate batch preparation with status dependency scheduling.
   - Keep 3x3 `FEATURES` stability and 3x3 `FULL` publish gates.
   - Keep cooperative yielding around status jobs, not around fake flattened phases.

5. **Persist partial progress like vanilla**
   - Save unsaved generated partial chunks on unload, not only publishable packed snapshots.
   - Reload saved partial statuses and resume from the stored status.
   - Keep the current packed-publish cache as a client/durable transport optimization only after it is clearly separate from proto/full status storage.

6. **Measure again**
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
