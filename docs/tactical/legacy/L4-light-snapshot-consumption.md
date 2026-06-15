# L4 - Light snapshot consumption

Standing after [`L3-initial-chunk-lighting.md`](L3-initial-chunk-lighting.md). This slice makes the client/render-world side retain authoritative light facts from chunk snapshots and expose them to mesh compilation.

## Goal

Consume stored sky/block light after the host publishes it:

- hydrate `ChunkSnapshot.light` into the client chunk cache
- answer `getBrightness(...)` and `getRawBrightness(...)` from light `DataLayer`s
- keep packed light buffers included in render-world worker transfers and batching limits
- preserve light facts in worker-facing mesh snapshots
- keep fullbright-style constants only as the fallback for snapshots that explicitly omit light

At the end of `L4`, render-world and mesh-worker builds can derive packed vertex light from authoritative chunk light snapshots.

## Reference source

Read these before changing this area:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java` | client-owned chunk/light data and render dirtying |
| `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLevelChunkWithLightPacket.java` | baseline chunks arrive with light data |
| `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLightUpdatePacket.java` | later section-level light updates |
| `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java` | packed light lookup for meshing |

## Scope

| # | Module | Result |
|---|---|---|
| 1 | `ClientChunkCache` | stores sky/block `DataLayer`s by light section and returns snapshot light values |
| 2 | mesh input path | existing `ChunkSnapshot` records retain `light` for mesh workers |
| 3 | render-world worker sink | light buffers count toward transfer batching and move with chunk updates |
| 4 | tests | client cache brightness lookup and light-buffer transfer coverage |

## Explicit non-goals

- no host-side live block-edit deltas yet
- no `chunk_light_delta` protocol message yet
- no browser light oracle/pixel diff automation yet
- no raytraced render backend
- no full voxel face-shape parity beyond the current L2 solver gap

## Implementation status

L4 is landed:

- `ClientChunkCache` copies light bytes from snapshots into `DataLayer`s keyed by chunk, section Y, and layer.
- `ClientChunkCache.getBrightness(...)` uses stored block light, stored sky light, and the vanilla layer-surrounding defaults when a lit snapshot omits a section.
- Sky lookups scan upward through missing light sections before falling back to full sky, matching the shape of vanilla's sparse sky-light reads.
- `getRawBrightness(...)` combines stored sky and block light instead of the previous sky-visibility approximation for lit chunks.
- Render-world worker ingest batching now counts light `DataLayer` buffers, so large lit snapshot batches do not exceed the intended transfer budget.
- Existing full-chunk ingest dirtying already marks affected render sections for rebuild when a lit snapshot arrives.

Known limits:

- Snapshots without `light` still use the old constant fallback. That keeps `lightingMode: "none"` usable as an explicit non-vanilla/debug mode.
- Host-side trusted persisted-light reuse is not separated from the current snapshot publication path yet; the packed light bytes remain durable and client-consumable.

## Validation

Focused validation:

```bash
pnpm test -- test/world/client-chunk-cache.test.ts test/renderer/chunk/render-world-worker-client.test.ts test/renderer/chunk/render-world-worker.test.ts test/renderer/chunk/chunk-render-infrastructure.test.ts
pnpm typecheck
```

The broader generated-world cooperative host scheduler tests are currently affected by unrelated dirty liquid-simulation work in the workspace and should be rerun after that work settles.

## Next

[`L5-live-light-deltas.md`](L5-live-light-deltas.md) adds live light updates. After that, the next slice should make visible meshing consume stored light:

- route packed-light calculation through `ClientChunkCache.getBrightness(...)`
- remove remaining fullbright constants from vanilla block/liquid mesh emission
- add a small torch/open-shaft browser probe that verifies visible light changes
