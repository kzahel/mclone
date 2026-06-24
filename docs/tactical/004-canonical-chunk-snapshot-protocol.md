# 004: Canonical Chunk Snapshot Protocol

Status: completed.

Define the first real native chunk snapshot and protocol contract before wiring the local integrated client/server loop. The goal is to make the next slice render from authoritative client-replica facts without baking a temporary `mclone-native-client` cache or direct worldgen shortcut deeper into the app.

## Standing

Depends on:

- [`003-native-ts-parity-roadmap.md`](003-native-ts-parity-roadmap.md)
- [`../runtime-data-model.md`](../runtime-data-model.md)
- [`../protocol.md`](../protocol.md)
- [`../loading-persistence.md`](../loading-persistence.md)
- [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md)

Native implementation references:

- `native/crates/mclone-core/src/chunk.rs`
- `native/crates/mclone-core/src/bit_storage.rs`
- `native/crates/mclone-protocol/src/lib.rs`
- `native/crates/mclone-client/src/lib.rs`

Reference Minecraft source to read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/LevelChunkSection.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/PalettedContainer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/util/BitStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLevelChunkPacket.java`
- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLightUpdatePacket.java`

## Core Decision

Chunk snapshots are authoritative runtime facts. They are not renderer meshes, not worldgen buffers, and not storage-adapter records.

The first native snapshot shape should be small enough to land now, but it must preserve the future model:

- full `BlockStateId` values, not renderer material ids
- `16x16x16` chunk sections
- omitted all-air sections
- section-local cell order: y-major, z-major, x-minor, matching `(local_y << 8) | (local_z << 4) | local_x`
- palette entries are full block-state ids
- local palette indices are packed with 1.17.1 `BitStorage` semantics
- chunk position, status, and revision travel with the snapshot
- lighting, biomes, block entities, scheduled ticks, and entities are extension points, not renderer side channels

The current Rust worldgen still exposes generated `u8` block ids. This slice may add a minimal generated-block to `BlockStateId` adapter for the current terrain palette, but that adapter must be visibly temporary until the vanilla block registry lands in `011-block-registry-and-asset-source.md`.

## Scope

1. Add shared chunk data primitives in `mclone-core`:
   - section constants for width, volume, and index order
   - `BlockStateId`
   - `ChunkRevision`
   - `ChunkStatus` or a similarly explicit publish-status enum
   - `PackedChunkSection`
   - `ChunkSnapshot`
2. Add a small `BitStorage` helper with 1.17.1-compatible no-cross-word packing.
3. Add a worldgen-to-snapshot adapter in the narrowest crate that avoids dependency cycles.
4. Extend `mclone-protocol` with logical messages:
   - `ClientCommand::SetChunkInterest`
   - `ServerUpdate::ChunkSnapshot`
   - `ServerUpdate::ChunkUnload`
5. Keep `ChunkInterest` as an interest command, not a synchronous "return chunks now" call.
6. Add tests that prove snapshot packing, cloning, indexing, and message ownership.

## Out Of Scope

- local integrated server runtime
- native app rewiring
- remote sockets or wire codecs
- browser boot
- chunk-status scheduling
- persistence format
- full vanilla block registry
- blockstate/model/asset loading
- lighting payloads beyond reserved extension shape
- biome containers beyond a placeholder or explicit omission
- entities, block entities, liquid ticks, and block deltas

## Candidate Rust Shape

The exact names can move if implementation pressure demands it, but the ownership should stay like this:

```rust
// mclone-core
pub struct ChunkRevision(pub u64);

pub enum ChunkStatus {
    Terrain,
    Surface,
    Features,
    Light,
    Full,
}

pub struct PackedChunkSection {
    pub section_y: i32,
    pub palette_state_ids: Vec<BlockStateId>,
    pub bits_per_block: u8,
    pub packed_block_indices: Vec<u64>,
}

pub struct ChunkSnapshot {
    pub pos: ChunkPos,
    pub status: ChunkStatus,
    pub revision: ChunkRevision,
    pub min_y: i32,
    pub height: i32,
    pub sections: Vec<PackedChunkSection>,
}

// mclone-protocol
pub enum ClientCommand {
    SetChunkInterest(ChunkInterest),
}

pub enum ServerUpdate {
    ChunkSnapshot(ChunkSnapshot),
    ChunkUnload { pos: ChunkPos },
}
```

If `PackedChunkSection` needs to live outside `mclone-core` to keep `mclone-core` smaller, it may live in a new data-focused module or crate, but it must not live in `mclone-render`, `mclone-mesh`, `mclone-worldgen`, or an app crate.

## Implementation Order

1. Read the native implementation references and Java source listed above.
2. Add constants and indexing helpers to `mclone-core`.
3. Keep the small `palette_bits_for` and `BitStorage` behavior in `native/crates/mclone-core/src/bit_storage.rs`, cross-checking the Java source and the oracle-owned helper copy in `oracle/lib/util/bit-storage.ts`.
4. Add `PackedChunkSection` and `ChunkSnapshot`.
5. Add a minimal generated-terrain snapshot builder from current `GeneratedChunk` data.
6. Add protocol enums around chunk interest, snapshot, and unload.
7. Update any existing placeholder tests in `mclone-core` / `mclone-protocol` to assert the real contract.

## Validation

Required for the implementation slice:

```bash
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Focused tests should cover:

- `chunk_section_index(0, 0, 0) == 0`
- `chunk_section_index(15, 0, 0) == 15`
- `chunk_section_index(0, 0, 1) == 16`
- `chunk_section_index(0, 1, 0) == 256`
- `palette_bits_for` edge cases
- `BitStorage` roundtrip for 4096 entries
- empty-air sections are omitted
- non-air sections preserve block ids through pack/unpack
- protocol commands distinguish interest from host-originated updates
- no renderer, mesh, or native app dependency is introduced into the core/protocol data model

## Done When

- `mclone-core` has canonical chunk snapshot primitives.
- `mclone-protocol` has first real client/server message enums for chunk interest and chunk publication.
- Current generated chunks can be converted into chunk snapshots without involving renderer code.
- Tests pin the section order and packed palette behavior.
- `mclone-web-client` still checks for `wasm32-unknown-unknown`.

## Next Slice

After this lands, implement `005-local-integrated-client-server.md`:

- `IntegratedServer` owns seed/worldgen and publishes `ServerUpdate::ChunkSnapshot`
- `ClientRuntime` owns a client chunk replica and applies snapshots
- local in-process transport moves `ClientCommand` / `ServerUpdate`
- native headless/window rendering consumes client-replica chunks instead of direct worldgen output
