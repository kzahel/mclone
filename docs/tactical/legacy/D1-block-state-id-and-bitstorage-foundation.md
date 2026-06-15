# D1 — Block-state id and `BitStorage` foundation

Standing after the durable architecture consolidation in [`../runtime-data-model.md`](../runtime-data-model.md), [`../protocol.md`](../protocol.md), and [`../loading-persistence.md`](../loading-persistence.md). This is the first implementation tactical in the runtime data/protocol/loading arc.

## Goal

Introduce the minimum shared runtime substrate needed for vanilla-shaped packed chunk sections:

- stable numeric ids for full `BlockState` values
- a runtime `BitStorage` helper matching Minecraft Java 1.17.1 semantics
- focused unit coverage proving both pieces can support later packed-section work

At the end of `D1`, the repo should have reusable low-level primitives, but chunk snapshots, storage records, protocol messages, and browser worker ownership should still behave as they do today.

## Why this slice first

The current hot chunk paths still move section facts as name/property object graphs plus `number[]` palette indices. Before changing snapshots, storage, protocol, or worker topology, the engine needs two smaller shared facts:

1. a process-stable numeric identity for every full block state
2. a 1.17.1-compatible packed integer storage helper

Those are prerequisites for packed sections, but they are independently testable. Keeping them separate prevents the next slice from mixing data identity, bit packing, protocol migration, and browser performance into one change.

## Reference source

Read these before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Blocks.java` | block registration feeds global block-state identity |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunkSection.java` | section storage is `PalettedContainer<BlockState>`-backed |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/PalettedContainer.java` | palette width rules, section index order, global palette fallback |
| `reference/minecraft-1.17.1/src/net/minecraft/util/BitStorage.java` | exact no-cross-64-bit-word packed integer behavior |

The target is not a full `PalettedContainer` port in this slice. The target is the substrate that lets `D2` implement packed sections cleanly.

## Current TS context

Useful existing code:

| TS source | Current role |
|---|---|
| `src/core/registry.ts` | process-local block registry with insertion-order iteration |
| `src/world/level/block/state/state-definition.ts` | each block already exposes ordered possible states |
| `src/world/level/block/state/block-state.ts` | runtime full block-state object |
| `src/world/level/generated-render-blocks.ts` | registers the current generated/render block set and returns a reduced `ChunkBlockId -> BlockState` table |
| `src/world/level/chunk-snapshot.ts` | current object-heavy snapshot/hydration path |
| `src/oracle/anvil/chunk.ts` | oracle-local `paletteBitsFor(...)` and `unpackBitStorage(...)` helpers |
| `src/worldgen/chunk/chunk-section-serialization.ts` | existing section palette/index grouping for reduced worldgen ids |

Important distinction:

- the existing `ChunkBlockId -> BlockState` table is a reduced worldgen/render material table
- `D1` should introduce ids for full `BlockState` values

Do not collapse those concepts. Later migration can decide how the reduced table maps into the full table.

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Full block-state id table | runtime can map `BlockState -> BlockStateId` and `BlockStateId -> BlockState` for all registered block states |
| 2 | State-key diagnostics | tests can prove distinct property variants get distinct ids and stable roundtrips |
| 3 | Runtime `BitStorage` | non-oracle code can pack, get, set, get-and-set, expose raw 64-bit words, and unpack all values |
| 4 | Oracle helper reuse/alignment | oracle-side unpack behavior and runtime helper agree on representative vectors |
| 5 | Focused tests | unit tests lock down id roundtrips, property variants, boundary bit widths, and error handling |

## Explicit non-goals

- no packed `ChunkSectionSnapshot` rollout
- no protocol or HTTP codec changes
- no IndexedDB or file storage format changes
- no browser render-world worker ownership changes
- no `SharedArrayBuffer`
- no full `PalettedContainer` implementation
- no change to worldgen correctness or generated terrain contents
- no attempt to match vanilla numeric registry ids byte-for-byte yet

## Proposed module shape

Exact filenames can move if implementation context suggests a better fit, but keep the pieces shared and out of oracle-only code:

```text
src/
  util/
    bit-storage.ts
  world/
    level/
      block/
        state/
          block-state-id.ts
```

Likely public surface:

```ts
type BlockStateId = number;

interface BlockStateIdMap {
  readonly size: number;
  idFor(state: BlockState): BlockStateId;
  stateFor(id: BlockStateId): BlockState;
}

function buildBlockStateIdMap(blocks: Iterable<Block>): BlockStateIdMap;
```

```ts
class BitStorage {
  constructor(bits: number, size: number, raw?: BigInt64Array | readonly bigint[]);
  get(index: number): number;
  set(index: number, value: number): void;
  getAndSet(index: number, value: number): number;
  getRaw(): BigInt64Array;
  getSize(): number;
  getBits(): number;
  getAll(): number[];
}
```

Use `bigint` internally for 64-bit word behavior unless a cleaner typed-array/DataView implementation stays equally direct and tested.

## Format rules

### Block-state ids

- ids represent full block states, not blocks
- ids are stable within one process after block registration is complete
- ids are assigned by iterating registered blocks, then each block's `StateDefinition.getPossibleStates()`
- `idFor` must throw for states outside the table
- `stateFor` must throw for ids outside the table
- property-distinct states must get distinct ids

This intentionally follows vanilla's concept of a global block-state registry without requiring byte-for-byte vanilla registry ids in this slice.

### `BitStorage`

Match Minecraft Java 1.17.1 logical behavior:

- `bits` is inclusive `1..32`
- `valuesPerLong = floor(64 / bits)`
- entry bits do not span 64-bit words
- word count is `ceil(size / valuesPerLong)`
- values must be in `0..((1 << bits) - 1)`
- `get`, `set`, and `getAndSet` validate indices and values
- raw words preserve the same low-bit-first slot layout as vanilla

The existing oracle note in `04-integration-oracle-harness.md` is important: 1.17.1 does not use the older cross-long packing layout.

## Validation

### Unit tests

Block-state ids:

- build a table after `registerGeneratedRenderBlocks()`
- `state -> id -> state` roundtrips for all states
- variants from property-bearing blocks are distinct, for example snow layers, liquid levels, logs axis, leaves distance/persistent, bamboo age/leaves/stage, cocoa age/facing
- unknown states and invalid ids throw useful errors

`BitStorage`:

- roundtrips for bit widths `1`, `4`, `5`, `9`, `15`, and `32`
- size boundaries where `size` is not a multiple of `valuesPerLong`
- `getAndSet` returns the previous value and writes the new one
- invalid bits, indices, values, and raw array lengths throw
- raw vectors match oracle-style low-bit-first extraction

### Static checks

- `pnpm typecheck`
- `pnpm test`

No browser visual validation is required for `D1`; it should not change rendered output.

## Done when

- shared runtime code exposes full block-state id mapping
- shared runtime code exposes 1.17.1-compatible `BitStorage`
- oracle-only bit unpacking is either reused from the shared helper or covered by tests proving equivalence
- `ChunkBlockId` remains separate from full `BlockStateId`
- no chunk snapshot/protocol/storage behavior has changed
- `pnpm typecheck` and `pnpm test` pass

## Next

`D2`: packed section codecs and chunk snapshot model. That slice should use the `D1` primitives to build vanilla-shaped section records, still before changing browser worker ownership.
