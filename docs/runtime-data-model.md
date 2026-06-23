# Runtime Data Model

Durable data-contract guidance for world/chunk state as it crosses simulation, storage, protocol, client workers, meshing, and rendering.

This document owns the shape of the data. [`architecture.md`](./architecture.md) owns the runtime boundaries. [`protocol.md`](./protocol.md) owns the message model. [`loading-persistence.md`](./loading-persistence.md) owns lifecycle and save policy.

## Core Rule

The canonical chunk-content model should be vanilla-shaped, not browser-storage-shaped and not renderer-shaped.

That means:

- full block states have stable numeric identities
- chunks are divided into sparse `16x16x16` sections
- non-empty sections use a local palette of full block-state ids
- section cells store packed local palette indices
- transport, persistence, and meshing consume this logical model through different codecs or views

The model should stay close to Minecraft Java 1.17.1 where that shape is still a good fit. Divergence belongs in the carrier, adapter, or runtime owner, not in the meaning of the chunk data.

## Reference Baseline

Relevant vanilla concepts:

| Java source | Concept |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Blocks.java` | global block-state registration |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunkSection.java` | section-owned block-state storage |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/PalettedContainer.java` | local palette plus packed indices |
| `reference/minecraft-1.17.1/src/net/minecraft/util/BitStorage.java` | packed integer storage |
| `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java` | client chunk cache consumes authoritative chunk state |
| `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java` | chunk section compilation is async client work |

The browser cannot use vanilla's shared JVM heap. It can still preserve vanilla's logical section model.

## Current Mismatch

The current runtime boundary still uses `ChunkSnapshot` records shaped around:

- `palette: BlockStateSnapshot[]`
- `blocks: number[]`
- name/property object graphs for hot block-state transport

That was useful for early correctness work, but it is now too loose for shared runtime use. It creates avoidable allocation and clone cost, and it blurs the model used by storage, protocol, client cache, and meshing.

## Block-State Identity

Numeric ids must represent full `BlockState` values, not just block types.

Examples that must remain distinct:

- slab half / waterlogging
- stair facing / shape
- snow layers
- crop age
- fence or pane connections
- liquid levels

The runtime should expose a shared block-state table with:

- `BlockState -> BlockStateId`
- `BlockStateId -> BlockState`
- stable ids within a process
- deterministic registration order for a fixed asset/content load

Do not let callers synthesize ids ad hoc from `{ name, properties }` on hot paths. Name/property records remain useful for debugging, JSON tooling, and compatibility adapters, not for the canonical runtime hot path.

## Packed Sections

The canonical section fact should be logically equivalent to:

```ts
interface PackedChunkSection {
  readonly y: number;
  readonly paletteStateIds: Uint32Array;
  readonly bitsPerBlock: number;
  readonly packedBlockIndices: ArrayBuffer;
}
```

Rules:

- section dimensions are `16x16x16`
- empty-air sections may be omitted
- palette entries are full block-state ids
- cell values are local palette indices
- cell order is section-local `y-major,z-major,x-minor`
- packing follows Minecraft 1.17.1 `BitStorage` semantics

For 1.17.1 `BitStorage`, entries do not span 64-bit words. The existing oracle decoder in `oracle/lib/anvil/chunk.ts` is the runtime behavior reference until the shared helper lands.

The exact TypeScript wrapper can change. The logical model should not.

## Chunk Snapshots And Deltas

Use snapshots for baseline chunk delivery and deltas for later mutation traffic.

Snapshot facts should include:

- chunk X/Z
- packed non-empty sections
- biome data
- scheduled block/fluid tick records where generation creates them
- future heightmaps, lighting, and block entities when those systems become live

Deltas should be additive to the same model:

- block update batches as positions plus block-state ids
- section replace/update messages where that is cheaper than many cell edits
- light deltas after lighting is authoritative
- block entity deltas after block entities exist

Mesh payloads are not chunk snapshots. Meshes are renderer-consumer products derived from chunk facts.
For the intended packed light-section shape and solver ownership, see [`lighting.md`](./lighting.md).

## Storage Records

Persistence adapters should store the same logical chunk facts, but each adapter may choose its physical encoding:

- browser IndexedDB can store structured records plus binary buffers
- Node file storage can start with JSON plus binary sidecars or a compact binary record
- a later region-file or SQLite backend can use a different layout

The simulation core should not know which adapter is active.

Storage records may contain adapter-local metadata such as timestamps, dirty status, or save schema version. That metadata must not become part of the authoritative chunk-content model.

## Client Render-World Ownership

The browser main thread should not own raw chunk sections.

Target browser-client ownership:

- authoritative host owns world state
- browser render-world worker owns client chunk cache and meshing inputs
- main thread owns input, UI, GPU resources, and draw submission
- main thread receives section mesh payloads and small presentation state, not packed chunk sections

Local singleplayer and remote multiplayer should share this browser-client shape. Only the authoritative source and transport differ.

## Client Replica And Prediction Ownership

Vanilla's singleplayer and multiplayer clients both use a client world replica rather than rendering directly from a server world; see [`minecraft-client-replica-research.md`](./minecraft-client-replica-research.md). Player prediction needs collision-relevant facts from that replica, but those facts should have their own client-side owner instead of leaking into the render thread.

Target ownership:

- authoritative host owns canonical chunk/entity/block/liquid state
- client replica/runtime owns visible client world facts, entity replicas, light/render facts, speculative overlays, and a bounded prediction view for local movement replay
- render-world/mesh worker owns meshing jobs and derived render products, or acts as part of the broader client replica/runtime
- main thread owns input/UI/GPU and receives small presentation poses

The client replica is a derived client mirror, not a server clone. It may contain visible chunks, entities, block entities, fluid states, light state, and presentation-time derived caches. The prediction view should contain only the authoritative facts needed to replay local movement commands: nearby block collision data, movement/collision revision maps, and dynamic colliders that can affect local prediction. The client replica should not run worldgen, decoration, AI authority, spawning authority, block ticks, liquid ticks, persistence, or chunk scheduling.

No client fallback may synthesize canonical chunk contents from seed. If `ClientWorld` lacks a chunk or collision window, the correct state is missing authority data, not locally generated authority. A temporary placeholder can be a presentation-only loading product, but it must not be stored as a `ClientWorld` chunk snapshot, collision truth, or revisioned fact.

Packed chunk snapshots/deltas may feed rendering, lighting, and prediction views, but no derived product should become another subsystem's source of truth. Meshes are not collision. Collision snapshots are not meshes. Client light caches are not host light authority. If sharing immutable packed buffers later becomes worthwhile, it remains a carrier optimization with explicit ownership and lifetime rules.

The first native code contract for this split is `native/crates/mclone-client/src/lib.rs`.
`ClientRuntime` owns the client replica and exposes render/prediction facts over
server-published snapshots instead of letting the renderer or UI synthesize
authority. The durable contract is the ownership split: the client replica
exposes render and prediction views over replica facts; the UI/render thread
consumes presentation state and mesh products; prediction consumes
collision/entity views plus revision facts.

## SharedArrayBuffer

`SharedArrayBuffer` is a carrier optimization, not a data model.

Do not design the engine around SAB first. Consider it only after:

- chunk ownership no longer flows through the main thread
- packed sections are the normal chunk-content payload
- measurements show transfer/copy cost is still material

SAB requires:

- cross-origin isolation headers in browser deployments
- explicit buffer lifetime rules
- synchronization discipline
- allocator or pool ownership
- a fallback path for environments where SAB is unavailable

The first SAB candidate should be a bounded buffer pool for packed chunk or mesh payload transfer, not shared mutable authoritative chunk state.

## Implementation Direction

The next implementation track should proceed in this order:

1. introduce shared `BlockStateId` and `BitStorage` runtime helpers
2. add packed section codecs and roundtrip tests
3. move protocol/storage chunk snapshots onto packed section facts
4. move browser-client chunk ownership and meshing off the main thread
5. measure transfer and upload costs
6. only then evaluate SAB or worker-to-worker channels

Each step should preserve the same logical chunk model.
