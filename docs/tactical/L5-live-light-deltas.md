# L5 - Live light deltas

Standing after [`L4-light-snapshot-consumption.md`](L4-light-snapshot-consumption.md). This slice carries changed sky/block light sections after the initial chunk snapshot path.

## Goal

Publish and consume section-level light changes:

- define `chunk_light_delta` as a whole-light-section replacement message
- represent explicit empty light sections separately from omitted sections
- have the generated-world host collect solver `onLightUpdate(...)` callbacks after propagation
- apply deltas in `ClientChunkCache` and render-world worker state
- dirty the changed render section plus neighboring render sections

## Reference source

Read these before changing this area:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLightUpdatePacket.java` | changed-section masks, empty masks, and byte payloads |
| `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java` | client queues section data and dirties render sections with neighbors |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java` | server collects dirty light-section bits and broadcasts light updates |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightSectionStorage.java` | dirty light sections are reported only after the visible section map is swapped |

## Implementation status

L5 is landed:

- `WorldHostMessage` now includes `chunk_light_delta`.
- `PackedLightSectionUpdate` uses `data?: Uint8Array`, where missing `data` means an explicit empty `DataLayer`, matching vanilla's empty light mask.
- worker transfer paths include light-delta buffers.
- JSON/HTTP wire codecs round-trip light deltas and the HTTP protocol version is bumped.
- `ClientChunkCache.applyChunkLightDelta(...)` updates stored `DataLayer`s and cached chunk light facts.
- render-world worker ingest applies light deltas and dirties a 3x3x3 render-section neighborhood around changed light sections.
- the generated-world host records solver `onLightUpdate(...)` callbacks, drains visible dirty light sections into `chunk_light_delta`, and clears dirty light sections covered by full chunk snapshots.
- live block mutations through the host call `updateSectionStatus(...)`, `checkBlock(...)`, and block-emission increase handling before light deltas are drained.

## Known limits

- Current liquid integration still republishes full chunk snapshots for changed blocks. `chunk_light_delta` mainly carries light-only neighbor updates and prepares the path for future block-delta messages.
- The host still runs lighting to idle for these MVP paths; budgeted cooperative propagation remains a later scheduler refinement.
- Browser pixel validation is still pending because the current workspace has unrelated asset/runtime changes that can block probes.

## Validation

Focused validation:

```bash
pnpm test -- test/world/client-chunk-cache.test.ts test/renderer/chunk/render-world-worker-client.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts test/runtime/render-world-update-sink.test.ts test/runtime/packed-chunk-wire.test.ts test/runtime/world-message-queue.test.ts
pnpm typecheck
```

## Next

`L6` should make the renderer visibly use the stored light facts:

- route mesh packed-light calculation through `ClientChunkCache.getBrightness(...)`
- remove remaining fullbright constants from block/liquid mesh emission in the vanilla render mode
- add a focused torch/interior or cave-mouth browser probe once the unrelated browser startup issues settle
