# Liquid2 - Authoritative host integration

Implementation tactical for wiring the Liquid1 water simulation core into the generated-world host.

## Goal

Make pending liquid ticks authoritative runtime facts instead of inert chunk metadata.

At the end of Liquid2, the host should:

- hydrate chunk-local `liquidTicks` from generated or stored chunks into a host-owned `ServerTickList<Fluid>`
- advance due water ticks from the host world tick loop
- execute fluid mutation through loaded-chunk-safe block access and neighbor update scheduling
- dirty changed chunks and publish authoritative updates to clients
- persist remaining pending liquid ticks back into chunk snapshots with relative delays

## Implementation Status

Landed implementation:

- `GeneratedWorldHost` now owns host game time and a `ServerTickList<Fluid>` for liquid ticks.
- `LiquidSimulationLevel` adapts the generated render level into a runtime mutation view: reads do not generate chunks, writes only touch loaded chunks, and block placement triggers `onPlace`, `updateShape`, and `neighborChanged` scheduling.
- Loaded chunk `liquidTicks` are consumed from `LevelChunk` and scheduled into the host queue.
- Host snapshots merge pending queue entries back into `liquidTicks`, preserving pending updates through client snapshots and storage saves.
- Liquid block mutations dirty chunks; the current protocol republishes those chunks as replacement `chunk_snapshot` messages.
- `test/runtime/generated-world-host-liquid.test.ts` covers stored pending tick hydration, live water execution, dirty snapshot publication, and pending tick persistence.

## Reference Source

Read before extending this slice:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java` | `tickLiquid(...)` entry point and server tick ownership |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/ServerTickList.java` | due queue ordering, ticking-position checks, and per-tick cap |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunk.java` | chunk tick unpack/promotion shape |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java` | persisted `LiquidTicks` delays relative to save time |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/LiquidBlock.java` | neighbor scheduling hooks that runtime writes must trigger |

## Architectural Notes

Vanilla mutates liquid blocks on the server tick thread. `mclone` maps that authority lane to the generated-world host, which may be running in a browser worker or Node host. The simulation algorithm and tick queue shape stay vanilla-oriented; the thread/event-loop placement is the platform divergence.

The current protocol has no granular block-delta message. Liquid2 therefore publishes whole dirty chunk snapshots. This is intentionally coarse and keeps renderer/client behavior correct through the existing path; a later protocol slice can replace the payload with block deltas without moving simulation out of the host.

## Validation

Current validation:

- `pnpm typecheck`
- `pnpm test -- test/runtime/generated-world-host-liquid.test.ts`
- targeted Liquid0/Liquid1 oracle tests should remain green when touching flow or tick semantics

Recommended follow-up validation:

- browser probe framing a small source-water spill after several host ticks
- cross-chunk spill test, especially when the destination chunk is loaded but not the source chunk's original snapshot
- official-server oracle for source regeneration and falling water beyond the initial slope fixture

## Done When

- pending chunk `liquidTicks` are not inert after load
- due liquid ticks execute only from the authoritative host loop
- live water writes dirty authoritative chunks and reach clients through normal chunk update handling
- pending queue entries are present in saved/snapshotted chunks with relative delays
- no renderer or client cache code simulates water

## Follow-Up

`Liquid3`: broaden visible and oracle coverage before expanding the feature surface. Start with a browser liquid probe for the hill/spring case, add cross-chunk and source-regeneration fixtures, then choose between granular block-delta protocol work and lava/waterlogged follow-through based on the first real gaps those tests expose.
