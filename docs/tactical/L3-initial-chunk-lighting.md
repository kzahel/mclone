# L3 - Initial chunk lighting

Standing after [`L2-light-solver-foundation.md`](L2-light-solver-foundation.md). This slice wires the vanilla-shaped solver into the authoritative generated-world host for initial chunk publication.

## Goal

Publish lit chunk snapshots from the host:

- activate light storage for generated non-empty sections
- enable sky sources for generated chunk columns
- scan generated chunk contents for emitting blocks
- run `LevelLightEngine` before chunk snapshot publication
- include sky/block light `DataLayer` bytes and `lightCorrect` in packed snapshots
- keep the lighting backend explicitly selectable, with vanilla 1.17 stored light enabled by default

At the end of `L3`, generated-world snapshots carry authoritative stored light facts. Client render-world ingestion and mesh packed-light lookup are still separate follow-up work.

## Reference source

Read these before changing this area:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java` | `lightChunk(...)` order: mark not correct, update section status, enable sources, scan emitters, run propagation, mark correct |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ProtoChunk.java` | records light-emitting positions during generation |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java` | rescans emitters when trusted stored light is not available |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java` | layer coordinator and light-section padding |

The host scheduling shape diverges from vanilla's threaded wrapper, but the initialization order and solver data remain vanilla-shaped.

## Scope

| # | Module | Result |
|---|---|---|
| 1 | `GeneratedWorldHost` | owns a `LevelLightEngine` behind `lightingMode?: "vanilla17" | "none"` |
| 2 | generated chunk publication | initializes and drains lighting before packing snapshots |
| 3 | chunk unload | clears light-correct tracking and queues light storage removal for evicted columns |
| 4 | `LevelChunk` | exposes `BlockGetter.getMaxLightLevel()` and iterable non-air block entries |
| 5 | tests | cooperative and synchronous host tests allow full generation+lighting time and assert light payload shape |

## Explicit non-goals

- no client/render-world light cache yet
- no mesh packed-light replacement yet
- no live block-edit light deltas yet
- no persisted-light trust path for loaded chunks yet
- no byte-exact official-server compare for generated chunks until fixture block-state properties and generated content are exact enough

## Implementation status

L3 is landed:

- `GeneratedWorldHost` can construct a vanilla 1.17 `LevelLightEngine` when `lightingMode: "vanilla17"` is explicitly requested; the current runtime default keeps lighting off until the worker-backed path is fast enough.
- Generated chunks are initialized by scanning existing non-air entries instead of every block coordinate.
- Initial lighting mirrors vanilla `lightChunk(...)`: active sections are marked non-empty, sky sources are enabled, emitting states call `onBlockEmissionIncrease(...)`, and propagation runs before snapshot packing.
- Packed chunk snapshots now include `light.sky[]`, `light.block[]`, and `lightCorrect: true` when the vanilla backend is active.
- Chunk unload clears local light-correct bookkeeping and queues the corresponding light sections/source column for removal.

Known limits:

- `mutateWorld`-style ad hoc host mutations are only covered before initial lighting. General live edits need `L5`-style dirty block checks and light deltas.
- Stored light bytes are authoritative-host facts now, but the client cache and mesh workers still ignore them.
- Full initial lighting makes the existing generated-world boundary tests cross the old 5s Vitest timeout; those tests now declare a larger timeout instead of silently disabling lighting.

## Validation

Focused validation:

```bash
pnpm test -- test/runtime/generated-world-host-scheduler.test.ts --reporter=verbose
pnpm test -- test/runtime/generated-world-boundary.test.ts --reporter=verbose
pnpm test -- test/runtime/generated-world-persistence.test.ts --reporter=verbose
pnpm test -- test/world/lighting/level-light-engine.test.ts test/world/packed-chunk-snapshot.test.ts test/runtime/packed-chunk-wire.test.ts --reporter=verbose
```

`pnpm typecheck` is currently blocked by unrelated in-progress work outside the lighting files.

## Next

`L4` should extend storage/protocol/render-world ingestion:

- preserve trusted stored light from saved packed chunks instead of always recomputing
- hydrate light snapshots into the client/render-world chunk cache
- expose light lookup on the client level implementation
- mark render sections dirty when light facts arrive
- when lighting is omitted, keep the runtime on the unlit path until the vanilla backend is performant enough to become the default again
