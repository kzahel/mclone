# D2 — Packed section codecs and chunk snapshot model

Standing after [`D1-block-state-id-and-bitstorage-foundation.md`](D1-block-state-id-and-bitstorage-foundation.md). `D1` introduced the shared full `BlockStateId` table and 1.17.1-compatible `BitStorage`; this slice uses those primitives to create the packed chunk-section data model described in [`../runtime-data-model.md`](../runtime-data-model.md).

## Goal

Introduce vanilla-shaped packed section records and codecs:

- packed non-empty `16x16x16` chunk sections
- per-section palettes of full `BlockStateId` values
- packed local palette indices using the shared `BitStorage`
- conversion between today's object-heavy `ChunkSnapshot` and packed chunk facts
- focused roundtrip coverage against generated chunks and oracle fixture sections

`D2` landed the packed section representation as a shared codec layer. Existing protocol, storage, client-cache, and mesh-worker behavior still use compatibility adapters around the current `ChunkSnapshot` shape.

## Why this slice exists

The next broad arc wants chunk facts to stop moving as:

- `palette: BlockStateSnapshot[]`
- `blocks: number[]`
- hot-path `{ name, properties }` objects

But changing the whole runtime boundary at once would mix too many concerns:

- packed section format
- snapshot compatibility
- storage adapter migration
- worker transport encoding
- client-cache ownership
- meshing inputs

`D2` isolates the data format. Once the packed model roundtrips cleanly, `D3` can roll it through storage/protocol surfaces and `D4` can move ownership.

## Reference source

Read these before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunkSection.java` | vanilla section size, empty-section behavior, block-state storage owner |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/PalettedContainer.java` | section-local index order, palette width rules, NBT read/write behavior |
| `reference/minecraft-1.17.1/src/net/minecraft/util/BitStorage.java` | packed local-index layout |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Block.java` | global block-state registry concept used by palettes |

This slice is still not a full `PalettedContainer` port. It is a compact immutable snapshot/codec layer shaped by the same section model.

## Current TS context

Useful existing code:

| TS source | Current role |
|---|---|
| `src/world/level/chunk-snapshot.ts` | current authoritative snapshot and hydration shape |
| `src/world/level/chunk/snapshot-level-chunk.ts` | runtime view over snapshots for meshing/render reads |
| `src/world/level/client-chunk-cache.ts` | current client-side snapshot cache |
| `src/worldgen/chunk/chunk-section-serialization.ts` | reduced-id section palette/index grouping precedent |
| `src/oracle/anvil/chunk.ts` | decoded oracle sections and shared `BitStorage` usage |
| `src/world/level/block/state/block-state-id.ts` | full block-state id mapping from `D1` |
| `src/util/bit-storage.ts` | packed integer storage from `D1` |

The current `ChunkSnapshot` type should not disappear in this slice unless the compatibility migration stays small and low-risk. The safer default is to add packed equivalents and conversion helpers first.

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Packed section/chunk types | shared packed records exist for sections and chunk snapshots |
| 2 | Pack codec | current `ChunkSnapshot` sections can be encoded into packed section records using `BlockStateIdMap` |
| 3 | Unpack codec | packed records can decode back to current `ChunkSnapshot` shape or equivalent block-state reads |
| 4 | Oracle fixture bridge | decoded oracle sections can be packed/unpacked without palette-index drift |
| 5 | Roundtrip tests | generated chunks and fixture sections prove lossless conversion through packed records |

## Landed implementation

| Module | Result |
|---|---|
| `src/world/level/packed-chunk-snapshot.ts` | defines `PackedChunkSection`, `PackedChunkSnapshot`, `packChunkSection`, `unpackChunkSection`, `packChunkSnapshot`, `unpackChunkSnapshot`, and `bitsForLocalPalette` |
| `src/world/level/chunk-snapshot.ts` | exports the shared block order and block-state snapshot serializer needed by codecs and tests |
| `test/world/packed-chunk-snapshot.test.ts` | covers local palette widths, air/no-air sections, invalid packed inputs, snapshot hydration after unpack, and committed terrain fixture roundtrips |

No live protocol, storage, renderer, worker ownership, or mesh-input path moved in this slice.

## Explicit non-goals

- no remote HTTP wire-format migration
- no worker `postMessage` transfer-list work
- no IndexedDB or file storage physical layout migration
- no render-world worker ownership refactor
- no mesh-worker input redesign
- no `SharedArrayBuffer`
- no live protocol message rename or session lifecycle change
- no full mutable `PalettedContainer` implementation
- no byte-for-byte vanilla packet or NBT serialization requirement

## Implemented module shape

The packed model lives with shared world-level data, not renderer or oracle code:

```text
src/
  world/
    level/
      packed-chunk-snapshot.ts
```

Public surface:

```ts
interface PackedChunkSection {
  readonly y: number;
  readonly paletteStateIds: Uint32Array;
  readonly bitsPerBlock: number;
  readonly packedBlockIndices: BigInt64Array;
}

interface PackedChunkSnapshot {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly biomes: readonly number[];
  readonly sections: readonly PackedChunkSection[];
  readonly blockTicks: readonly ScheduledTickSnapshot[];
  readonly liquidTicks: readonly ScheduledTickSnapshot[];
}
```

Helpers:

```ts
function packChunkSnapshot(snapshot: ChunkSnapshot, ids: BlockStateIdMap, resolveState: BlockStateResolver): PackedChunkSnapshot;
function unpackChunkSnapshot(snapshot: PackedChunkSnapshot, ids: BlockStateIdMap): ChunkSnapshot;
function packChunkSection(section: ChunkSectionSnapshot, ids: BlockStateIdMap, resolveState: BlockStateResolver): PackedChunkSection;
function unpackChunkSection(section: PackedChunkSection, ids: BlockStateIdMap): ChunkSectionSnapshot;
function bitsForLocalPalette(paletteSize: number): number;
```

Avoid exposing mutable live chunk or renderer objects through the packed model.

## Format rules

### Section shape

- section dimensions are always `16x16x16`
- section cell order is `y-major,z-major,x-minor`, matching current snapshot order and vanilla `getIndex(x, y, z) = y << 8 | z << 4 | x`
- all-air sections may remain omitted
- non-empty sections should include air in the local palette when any air cells exist
- palette entries are full `BlockStateId` values
- section cell values are local palette indices, not global ids
- `paletteStateIds.length` determines local palette size
- `bitsPerBlock` should be the local bit width used by `BitStorage`

### Palette width

For the local packed snapshot path:

- use `max(4, paletteBitsFor(paletteSize))` for non-global local palettes
- follow 1.17.1 `BitStorage` no-cross-word layout
- keep global-palette fallback out of this slice unless implementation proves it is needed immediately

Reasoning: runtime snapshots in this project are not yet vanilla packet/NBT byte streams. Preserving local section palettes and 1.17.1 bit packing is the key compatibility target for `D2`; exact global-palette fallback can be added later if palette sizes make it necessary.

### Compatibility

Packed snapshots must preserve:

- chunk X/Z
- biome array contents and order
- section Y
- all block-state identities
- scheduled block/fluid ticks

If decoding back to `ChunkSnapshot`, decoded name/property snapshots must be stable enough for existing snapshot hydration tests and client-cache tests.

## Validation

### Unit tests

Packed section codecs:

- palette sizes `1`, `2`, `16`, `17`, and a larger mixed section
- section with air plus one non-air block
- section with no air and multiple block states
- invalid palette index or invalid block-state id throws
- packed raw word counts match `BitStorage` expectations

Packed chunk snapshots:

- `buildChunkSnapshot(...) -> pack -> unpack` preserves chunk coordinates, biomes, section count, section Y values, block ticks, liquid ticks, palettes, and all 4096 local cell identities per section
- generated chunk snapshot from `registerGeneratedRenderBlocks()` roundtrips through `PackedChunkSnapshot`
- hydration from the unpacked snapshot still produces equivalent block reads for representative positions

Oracle bridge:

- committed oracle fixture sections pack/unpack without palette-index drift
- 1.17.1 5-bit and 9-bit layout vectors still agree with `BitStorage`

### Static checks

- `pnpm typecheck`
- `pnpm test`

No browser visual validation is required for `D2` unless the implementation chooses to route any live renderer path through the packed codecs. Prefer not to do that yet.

## Done when

- shared packed section and packed chunk snapshot types exist
- current `ChunkSnapshot` can encode to packed form and decode back without data loss
- packed records use full `BlockStateId` palette entries and `BitStorage` local indices
- generated chunks and committed fixture sections have focused roundtrip coverage
- existing snapshot, oracle, client-cache, runtime, and renderer tests still pass
- protocol, storage, worker ownership, and meshing behavior remain unchanged except for optional internal tests/adapters

## Next

`D3`: storage/protocol rollout for packed chunk facts. That slice should choose how packed records move across local worker transport, remote transport, IndexedDB, and file storage while preserving the same logical protocol.
