# D3 — Packed chunk storage and protocol rollout

Standing after [`D2-packed-section-codecs.md`](D2-packed-section-codecs.md). `D2` introduced shared packed chunk-section records and lossless adapters to the current `ChunkSnapshot` shape. `D3` moves those packed records through the host/client protocol and storage adapters while leaving render-world ownership for `D4`.

## Goal

Make packed chunk facts the authoritative boundary payload for storage and protocol:

- `chunk_snapshot` host updates carry `PackedChunkSnapshot`
- `WorldStorage` chunk load/save contracts use `PackedChunkSnapshot`
- memory, file, and IndexedDB storage persist the same logical packed chunk record
- local direct, worker, and remote HTTP transports carry the same logical chunk payload through transport-specific codecs
- current client cache and hydration code can still unpack to `ChunkSnapshot` as a compatibility adapter

At the end of `D3`, object-heavy `{ name, properties }` block-state snapshots should no longer cross storage or protocol boundaries for hot chunk payloads.

## Why this slice exists

`D2` proved that current snapshots can pack and unpack without data loss, but the live runtime still sends and stores:

- `ChunkSnapshot`
- `BlockStateSnapshot[]`
- `number[]` section cells
- hot-path name/property object graphs

That keeps allocation and serialization costs in the path that will later feed browser workers, remote sessions, persistence, and meshing. `D3` removes that shape from the durable boundaries without also moving client chunk ownership or mesh fan-out. That keeps the migration testable before `D4` changes thread ownership.

## Reference source

Read these before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLevelChunkPacket.java` | vanilla sends chunk X/Z, available non-empty sections, biome data, heightmaps, block entities, and a binary section buffer |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunkSection.java` | section records own `PalettedContainer<BlockState>` data and skip empty sections |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/PalettedContainer.java` | local palette and packed `BitStorage` read/write behavior |
| `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java` | client cache consumes authoritative chunk packet data and replaces cached chunks |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java` | persistent chunk NBT writes section palettes and long-array block states |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/RegionFileStorage.java` | storage adapter owns the physical disk carrier, not the logical chunk meaning |
| `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java` | server owns chunk load/save and sends client chunk packets from authoritative state |

## Architecture divergence review

Minecraft Java keeps server, storage IO, packet encoding, and client chunk cache inside one JVM process with Netty byte buffers and Anvil NBT/region files. The browser/Node runtime cannot use that exact carrier:

- browser workers communicate through structured clone and transfer lists
- remote play currently uses HTTP JSON control envelopes
- browser persistence is IndexedDB
- Node persistence is file-backed JSON today

The divergence scope for `D3` is therefore the carrier, not the model:

- keep the vanilla-shaped logical facts: sparse sections, full block-state ids, local palettes, `BitStorage` words, section-local order
- do not require byte-for-byte vanilla packet or NBT serialization yet
- do not move authority into the client cache or renderer
- do not introduce `SharedArrayBuffer`

This divergence should make future parity easier: the logical section facts are closer to vanilla packets/NBT than the current name/property snapshot graph, and later byte-level codecs can be added behind the same logical model.

## Current TS context

| TS source | Current role |
|---|---|
| `src/world/level/packed-chunk-snapshot.ts` | shared packed chunk model and `ChunkSnapshot` adapters from `D2` |
| `src/runtime/protocol/world-messages.ts` | currently types `chunk_snapshot.snapshot` as `ChunkSnapshot` |
| `src/runtime/protocol/world-http-protocol.ts` | currently aliases remote serialized host messages to live host messages |
| `src/runtime/transport/local-world-transport.ts` | applies `chunk_snapshot` directly to `ClientChunkCache` |
| `src/runtime/transport/worker-world-transport.ts` | structured-clones live host messages without transfer lists |
| `src/runtime/transport/remote-world-transport.ts` | JSON-serializes/deserializes host messages through HTTP envelopes |
| `src/runtime/host/generated-world-host.ts` | builds `ChunkSnapshot` for send/save and hydrates stored snapshots |
| `src/runtime/node/generated-world-http-server.ts` | stores loaded snapshots for multi-session fan-out and sends JSON responses |
| `src/runtime/storage/world-storage.ts` | currently loads/saves `ChunkSnapshot` |
| `src/runtime/storage/memory-world-storage.ts` | stores `ChunkSnapshot` objects in memory |
| `src/runtime/storage/file-world-storage.ts` | writes chunk records as JSON containing `ChunkSnapshot` |
| `src/runtime/storage/indexeddb-world-storage.ts` | stores `ChunkSnapshot` through IndexedDB structured clone |
| `src/world/level/client-chunk-cache.ts` | currently accepts `ChunkSnapshot`; may stay as the D3 compatibility adapter |

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Protocol message types | `chunk_snapshot` carries `PackedChunkSnapshot`; control messages remain unchanged |
| 2 | Host snapshot production | authoritative generated chunks are packed before storage/save and before protocol send |
| 3 | Host storage loading | packed stored chunks unpack only at the point they must hydrate current `LevelChunk` objects |
| 4 | Client compatibility adapter | current client cache can consume packed snapshots by unpacking to existing snapshot/chunk views |
| 5 | Storage contracts | `WorldStorage` and all adapters load/save packed chunk records |
| 6 | Worker transport carrier | structured-clone path carries typed arrays and exposes transfer-list support for packed buffers |
| 7 | Remote HTTP carrier | JSON control envelopes use an explicit JSON-safe packed chunk wire codec |
| 8 | Remote fan-out cache | remote service stores loaded packed snapshots and filters them per session |
| 9 | Tests | storage, local, worker, remote, persistence, and boundary tests prove behavior is unchanged |

## Explicit non-goals

- no browser render-world worker ownership refactor
- no mesh-worker input redesign
- no `SharedArrayBuffer`
- no WebSocket/WebTransport or push-transport migration
- no chunk delta protocol
- no full mutable `PalettedContainer`
- no byte-for-byte vanilla packet or Anvil NBT serialization
- no heightmap, lighting, block-entity, or entity payload expansion
- no persistence policy rewrite beyond storing the packed logical record
- no compression requirement
- no global-palette fallback unless implementation discovers an immediate correctness need

## Target logical contract

The live logical protocol should still use the existing host update name:

```ts
interface ChunkSnapshotMessage {
  readonly type: "chunk_snapshot";
  readonly snapshot: PackedChunkSnapshot;
}
```

Keeping the message name avoids a broad semantic rename. In `D3`, "snapshot" means an authoritative baseline chunk payload, not the old object-heavy `ChunkSnapshot` type.

The storage contract should mirror that payload:

```ts
interface ChunkStorage {
  loadChunk(chunkX: number, chunkZ: number): Promise<PackedChunkSnapshot | undefined>;
  saveChunk(snapshot: PackedChunkSnapshot): Promise<void>;
  evictChunk(chunkX: number, chunkZ: number): Promise<void>;
}
```

`ChunkSnapshot` remains valid as a compatibility type inside:

- `buildChunkSnapshot(...)` before immediate packing
- `unpackChunkSnapshot(...)` before current hydration/client-cache application
- focused tests that prove the adapter path remains lossless

It should not remain the storage/protocol public payload after this slice.

## Carrier rules

### Direct local transport

`LocalWorldTransport` can pass `PackedChunkSnapshot` objects by reference inside tests and Node-local callers. It still exercises the same logical boundary; it does not need a wire codec.

### Worker transport

The worker transport should remain structured-clone based, but it should learn how to collect transferables from packed chunk messages:

- transfer `paletteStateIds.buffer`
- transfer `packedBlockIndices.buffer`
- do not transfer buffers still needed by the sender after posting
- clone or rebuild packed messages before transfer if the sender must retain a copy

This is not a buffer pool and not SAB. It is the normal transferable `ArrayBuffer` path. D5 will measure whether transfer/copy costs remain material.

The endpoint abstraction likely needs:

```ts
postMessage(message: TOutgoing, transfer?: readonly Transferable[]): void;
```

Tests should use a fake endpoint that records the transfer list so this behavior does not rely on a real browser worker.

### Remote HTTP transport

JSON cannot directly carry `BigInt64Array` or `bigint`. `D3` should add an explicit serialized packed-chunk wire shape instead of letting `JSON.stringify` guess:

```ts
interface SerializedPackedChunkSection {
  readonly y: number;
  readonly paletteStateIds: readonly number[];
  readonly bitsPerBlock: number;
  readonly packedBlockIndicesBase64: string; // little-endian signed 64-bit words
}

interface SerializedPackedChunkSnapshot {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly biomes: readonly number[];
  readonly sections: readonly SerializedPackedChunkSection[];
  readonly blockTicks: readonly ScheduledTickSnapshot[];
  readonly liquidTicks: readonly ScheduledTickSnapshot[];
}
```

Base64 is acceptable here because HTTP polling is still interim and the important `D3` goal is removing name/property object graphs from remote chunk payloads. Do not hide this behind vanilla packet names; it is a project wire codec for the packed logical model.

### File storage

File storage can use the same JSON-safe chunk record as remote HTTP:

- add a chunk record schema/version field
- store packed section words in base64
- keep save metadata adapter-independent
- bump the generated-world storage version so stale object-heavy chunk files are not silently mixed with packed records

### IndexedDB storage

IndexedDB can store structured records with typed arrays directly:

- keep `Uint32Array` palette ids
- keep `BigInt64Array` packed words if supported by the test/browser target
- fall back to the same JSON-safe section word encoding only if structured clone support is insufficient

The logical adapter contract remains `PackedChunkSnapshot` either way.

### Memory storage

Memory storage should store packed snapshots as records, preferably cloning typed arrays on save/load so tests do not accidentally depend on shared mutable references.

## Implementation sequence

1. Add protocol/storage codecs near the boundary.
   Suggested files:

   ```text
   src/runtime/protocol/packed-chunk-wire.ts
   src/runtime/storage/packed-chunk-record.ts
   ```

2. Update `WorldHostMessage` and `WorldStorage` types to use `PackedChunkSnapshot`.

3. Thread a `BlockStateIdMap` and `BlockStateResolver` through host/client construction where needed:

   - host packs `buildChunkSnapshot(...)` with `packChunkSnapshot(...)`
   - host unpacks stored records only before `hydrateChunkFromSnapshot(...)`
   - client unpacks protocol chunk snapshots before current `ClientChunkCache.applyChunkSnapshot(...)`, or adds `applyPackedChunkSnapshot(...)` as the adapter

   Use the full `BlockStateIdMap` from `D1`. Do not use the generated/render `ChunkBlockId -> BlockState` table as a substitute; that table is a reduced material table, not full block-state identity.

4. Update generated-world host persistence:

   - storage preload path loads packed, unpacks, hydrates
   - persistence path builds current snapshot, packs, saves
   - outgoing chunk messages carry packed snapshots
   - remote service `loadedSnapshots` stores packed snapshots

5. Update carriers:

   - direct local transport needs only type changes
   - worker transport adds transfer-list collection for packed chunk buffers
   - remote HTTP serialize/deserialize converts only at the HTTP boundary

6. Update storage adapters:

   - memory stores cloned packed records
   - file storage writes JSON-safe packed chunk records
   - IndexedDB stores logical packed records and validates roundtrip

7. Remove or narrow old object-heavy boundary uses.

   `ChunkSnapshot` references should remain in world-level compatibility adapters and tests, not in protocol/storage public contracts.

## Validation

### Unit tests

Packed wire/storage codecs:

- `PackedChunkSnapshot -> serialized HTTP/file record -> PackedChunkSnapshot` preserves typed-array contents exactly
- 64-bit packed words roundtrip through base64 with signed values and high bits intact
- invalid base64 word length, invalid bits, invalid ids, and invalid palette indices throw useful errors
- cloned packed snapshots do not share mutable typed-array backing stores unless explicitly transferred

Storage adapters:

- memory save/load roundtrips packed records and returns independent typed arrays
- file save/load roundtrips packed records and written JSON does not contain block-state `name`/`properties` section palettes
- IndexedDB save/load roundtrips packed records in the browser-like test environment
- incompatible storage metadata resets stale chunk records as today

Protocol/transports:

- `WorldHostMessage.chunk_snapshot.snapshot` is packed in type-level and runtime assertions
- worker transport posts packed chunk buffers through transfer lists when available
- remote HTTP responses serialize packed chunk records through the explicit JSON-safe codec
- remote deserialize hydrates the current client cache to the same visible block states as before

### Integration tests

Existing tests should continue to pass after being updated for packed payloads:

- `test/runtime/generated-world-boundary.test.ts`
- `test/runtime/generated-world-persistence.test.ts`
- `test/runtime/worker-world-transport.test.ts`
- `test/runtime/remote-world-transport.test.ts`
- `test/runtime/node-headless-generated-world-host.test.ts`
- `test/runtime/file-world-storage.test.ts`
- `test/world/client-chunk-cache.test.ts`

Add focused assertions where they prove the migration:

- local boundary receives packed snapshots before client unpacking
- remote service fan-out stores packed snapshots once and filters them per session
- file storage record is packed and schema-versioned
- worker fake endpoint sees transferable buffers for chunk messages

### Static checks

- `pnpm typecheck`
- `pnpm test`

No browser visual validation is required for `D3` if the renderer-facing behavior remains the current compatibility path. If the implementation changes live render cache ownership, stop and split that into `D4`.

## Done when

- protocol `chunk_snapshot` payloads are `PackedChunkSnapshot`
- storage `loadChunk`/`saveChunk` payloads are `PackedChunkSnapshot`
- memory, file, and IndexedDB adapters persist/load the packed logical record
- HTTP remote transport uses an explicit JSON-safe packed chunk codec, not raw `ChunkSnapshot`
- worker transport can transfer packed typed-array buffers without SAB
- host/client integration still hydrates equivalent visible chunks, scheduled block ticks, scheduled liquid ticks, and biome arrays
- old `ChunkSnapshot` object graphs remain only in compatibility adapters, not storage/protocol public contracts
- generated-world storage version is bumped or stale object-heavy records are otherwise rejected explicitly
- `pnpm typecheck` and `pnpm test` pass

## Next

`D4`: browser render-world ownership. After `D3`, the browser main thread may still unpack packed snapshots into the current `ClientChunkCache`. `D4` should move client chunk cache ownership and meshing inputs off the main thread while preserving the same packed chunk protocol.
