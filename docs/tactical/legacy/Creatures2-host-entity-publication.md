# Creatures2 - Host entity publication

Status: landed.

This slice follows [`Creatures1-generation-passive-spawning.md`](Creatures1-generation-passive-spawning.md). It wires generation-time passive entities into the real generated-world host lifecycle and publishes them through the shared host/client protocol as data.

## Goal

Make generated original mobs visible outside creature unit tests without rendering mobs yet:

- `GeneratedWorldHost` owns an `EntityRuntime<GeneratedMobEntity>`.
- chunk publication invokes `NoiseBasedChunkGenerator.spawnOriginalMobs(...)` through a host-owned sink.
- visible tracked generated entities are emitted as protocol `entity_snapshot` messages.
- local and remote clients cache entity snapshots as authoritative data and clear them when their chunk unloads.

This is an entity publication slice, not a behavior or rendering slice.

## Reference Source

No new creature parity algorithm was ported here. The parity-critical generation logic remains the Creatures1 port of:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java` | original mob generation entry point |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/NaturalSpawner.java` | generation-time passive spawn loop |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java` | full chunk status names used by entity visibility |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/entity/PersistentEntitySectionManager.java` | host-owned tracked/ticking lifecycle model |

The architectural divergence is narrow: `GeneratedWorldHost` currently invokes original mob spawning during chunk snapshot publication. Vanilla invokes it through `ChunkStatus.SPAWN`. The temporary host placement keeps entities authoritative and makes the future status-futures port easier than inventing a renderer-local sink.

## Landed Scope

- `WorldHostMessage` now includes `entity_snapshot`.
- `EntitySnapshot` carries id, UUID, type id, category, chunk, position, rotation, dimensions, on-ground, age, and small type-specific data such as sheep color.
- HTTP protocol version advanced to preserve remote wire compatibility.
- `WorldClient` exposes `getEntitySnapshots()`.
- `LocalWorldTransport` stores snapshots by entity id and clears snapshots for unloaded chunks.
- `GeneratedWorldHost` owns an `EntityRuntime<GeneratedMobEntity>`, promotes published chunks to `BORDER`, spawns original mobs once per chunk, and publishes sorted entity snapshots after the chunk snapshot.
- The Node remote service stores authoritative entity snapshots per shared world, forwards snapshots only to sessions that can see the entity chunk, replays them on resume, and clears them on chunk unload.
- `world-message-queue` treats `entity_snapshot` as chunk-interest bulk data so capped remote polls keep session/player state responsive.
- `test/runtime/generated-world-host-entities.test.ts` proves generated sheep-like snapshots appear through the host/client boundary and disappear when the chunk leaves view.

## Intentional Deferrals

- no `entity_delta`, remove-by-entity, or tick/revision stamped entity update stream
- no entity persistence adapter beyond the existing runtime memory path
- no live natural spawning, mob caps, player-distance eligibility, despawn, or AI
- no renderer ingestion, model loading, animation, sounds, particles, or selection UI
- no exact GeneratedWorldHost fixture comparison for the committed sheep positions until vanilla-shaped status futures and chunk scheduling are in place

The exact sheep fixture remains covered by `test/worldgen/levelgen/creature-generation.test.ts`, where the generation algorithm runs directly against the committed oracle fixture. The host publication test intentionally checks authoritative data flow and chunk lifecycle rather than exact vanilla count/order from the current cooperative host scheduler.

## Validation

Focused validation:

```bash
pnpm test -- test/runtime/generated-world-host-entities.test.ts test/runtime/world-message-queue.test.ts test/runtime/remote-world-transport.test.ts test/runtime/render-world-update-sink.test.ts test/runtime/generated-world-boundary.test.ts test/worldgen/levelgen/creature-generation.test.ts test/runtime/entity-runtime.test.ts
pnpm typecheck
```

No browser screenshot is required; this slice still does not produce pixels.

## Done Criteria

- done: generated-world host owns the entity sink used by generation-time passive spawning
- done: generated original mobs enter the host `EntityRuntime`
- done: entity snapshots flow through local and remote protocol paths
- done: client-side snapshot cache clears on chunk unload
- done: remote capped polling keeps player/session updates ahead of entity snapshot bulk data
- done: docs distinguish host publication from live spawning and rendering

## Next

The next useful creature tactical is `Creatures3-render-entity-placeholders`: consume `WorldClient.getEntitySnapshots()` in the browser presentation layer and draw simple debug placeholders at authoritative entity positions. Keep it deliberately visual-only: no AI, no local spawning, no prediction, and no client-owned entity mutation.

After that, choose between first real passive models and `Entities1` persistence/delta work based on what the placeholder path exposes.
