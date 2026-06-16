# 031: Native Section Block Delta Updates

Status: active.

## Purpose

Replace runtime full-chunk snapshot publication for block mutations with
Java-shaped block/section delta updates.

Full `ChunkSnapshot` updates remain the right shape for initial chunk
publication, chunk reload, and recovery. They are the wrong shape for fluid
spread and later block edits: a cluster of water/lava mutations currently
rebuilds and publishes one full chunk snapshot per changed block.

The target short-term implementation is correct, not throwaway:

- server mutates live chunk block storage immediately
- server records changed block positions by chunk section
- server flushes section block deltas at the tick/publication boundary
- client patches its loaded chunk snapshot copy
- renderer marks affected chunks/sections dirty

Lighting deltas, persistence detail, single-block packet specialization, and
full Java networking parity can come later, but the block-delta contract should
be the permanent runtime mutation path.

## Triggering Finding

The 120 Hz frame-budget probe from `030` showed remaining hitches were dominated
by fluid mutation publication:

- max `poll_ms`: `31.635 ms`
- max `poll_fluid_tick_ms`: `31.066 ms`
- max `poll_fluid_set_block_ms`: `30.859 ms`
- max `poll_scheduler_report_ms`: `0.637 ms`
- max `poll_apply_updates_ms`: `0.015 ms`
- worst frame had `18` mutated fluid blocks, `18` snapshot events, and `18`
  full chunk snapshot updates

Frames without executed fluid ticks averaged `2.539 ms` and had no budget
misses. The expensive work is full snapshot rebuild/publication per block
mutation, not scheduler publication, GPU upload, mesh rebuild, or client update
application.

## Java 1.17.1 Reference Shape

Relevant source files:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`
  - owns `ServerTickList<Fluid> liquidTicks`
  - calls `liquidTicks.tick()` during the level tick
  - `sendBlockUpdated(...)` forwards changed block positions to
    `ServerChunkCache.blockChanged(...)`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/ServerTickList.java`
  - moves due ticks into a current-tick queue up to the 65,536 cap
  - executes due fluid/block ticks without publishing whole chunks for each
    mutation
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/material/FlowingFluid.java`
  - mutates blocks immediately with `LevelAccessor.setBlock(...)`
  - schedules follow-up liquid ticks and neighbor updates
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
  - routes `blockChanged(BlockPos)` to the visible chunk holder
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
  - records changed block positions in `changedBlocksPerSection`
  - `broadcastChanges(...)` emits `ClientboundBlockUpdatePacket` for one changed
    block or `ClientboundSectionBlocksUpdatePacket` for multiple changed blocks
    in a section
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
  - handles `ClientboundSectionBlocksUpdatePacket` by applying per-block
    updates to the client level
  - handles `ClientboundBlockUpdatePacket` by setting known block state
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
  - marks changed block/section neighborhoods dirty instead of remeshing a
    whole chunk immediately

We should match this contract, not necessarily the exact packet split. A single
section-delta update type that can carry one or many block changes is enough for
the first slice.

## MVP Scope

Implement one permanent runtime delta path:

- Add a protocol/core type for section-scoped block updates.
- Add `ServerUpdate::SectionBlockUpdates`.
- Add encode/decode coverage for the new update.
- Add client patching for loaded chunk snapshots.
- Change server runtime block mutation publication to record section deltas
  instead of emitting `ChunkSnapshot` from each mutation.
- Flush recorded section deltas during the same simulation tick after fluid
  mutations have run.
- Keep full snapshots for initial chunk publication, store load, generated chunk
  publication, and chunk resend/recovery.
- Mark render work dirty from delta updates exactly as snapshot updates do today
  for the affected chunk and its direct neighbors.

Out of scope for the first slice:

- Java's separate single-block packet optimization.
- Java's exact light update packets.
- Partial persistence format changes.
- Block entity delta packets.
- Cross-client interest filtering beyond the current integrated/visible chunk
  behavior.
- Threading changes.

## Data Shape

Recommended minimal protocol shape:

```rust
pub struct SectionBlockUpdate {
    pub local_x: u8,
    pub local_y: u8,
    pub local_z: u8,
    pub block_state: BlockStateId,
}

pub enum ServerUpdate {
    ChunkSnapshot(ChunkSnapshot),
    ChunkUnload { pos: ChunkPos },
    SectionBlockUpdates {
        pos: ChunkPos,
        section_y: i32,
        updates: Vec<SectionBlockUpdate>,
    },
}
```

`section_y` is render/chunk section coordinate, not local section index. Local
coordinates are `0..16` within that section. Reject invalid local coordinates
and empty update lists at decode/apply boundaries.

For duplicate writes to the same local block within one flush window, keep the
last state. The Java holder stores changed positions and reads the final section
state when broadcasting; the effective result is a final-state delta.

## Server Shape

Add a pending delta accumulator to the scheduler:

- key by `(ChunkPos, section_y, local_index)` or by section key plus local index
- record changed visible block states after live block buffers mutate
- do not allocate a `ChunkSnapshot` during each mutation
- mark holders dirty for persistence exactly as before
- advance chunk revision when a visible delta batch flushes, or when the
  published full snapshot is replaced
- emit one `SectionBlockUpdates` per changed section at flush time

The key implementation boundary is `ChunkScheduler::set_block_at_world`.
Today it mutates `live_blocks`, rebuilds a full snapshot with
`snapshot_from_mutable_buffer`, publishes that snapshot, and optionally emits
`SnapshotReady`. This should become:

- mutate `live_blocks`
- mark holder/chunk dirty
- update the holder's published snapshot enough to keep server-owned future
  full snapshot publication coherent
- record a section delta if the holder is client-visible
- return no immediate snapshot event for normal runtime block changes

Because our holder's `published_snapshot` is also used as a resend/source of
truth in several places, the first implementation may still patch the stored
snapshot data in memory. It must not rebuild the whole snapshot per block.

## Client Shape

`ClientRuntime` currently stores `BTreeMap<ChunkPos, ChunkSnapshot>`. Add a
patch method that:

- looks up the chunk by `ChunkPos`
- finds or creates the target packed section if the y range is valid
- unpacks that section, applies local block changes, and repacks it
- bumps or stores the received revision policy if provided later
- leaves light sections unchanged for the first slice

If a delta arrives for an unknown/unloaded chunk, ignore it. That matches the
pragmatic networking stance: full snapshots establish chunk presence; deltas
only patch known chunks.

## Renderer Shape

For the first slice, treat section deltas like current snapshot dirtying:

- mark the changed chunk and direct horizontal neighbors dirty
- keep the existing one-dirty-chunk mesh budget

Later, when render dirty keys become section-precise, use the delta section key
to dirty only the affected render section and its neighbor sections.

## Persistence Shape

No persistence format change is required for the first slice:

- runtime mutations continue to update server live block buffers
- holder/chunk dirty tracking remains responsible for later full snapshot save
- filesystem persistence can continue saving full snapshots

The important contract is that deltas are a network/client-publication concern,
not the persistence representation.

## Validation

Required tests:

- protocol roundtrip for section block update packets
- client applies section deltas to a loaded snapshot
- client ignores deltas for unloaded chunks
- server fluid tick emits section block deltas instead of per-block chunk
  snapshots
- existing fluid oracle/parity tests still pass
- frame-budget probe exposes lower `poll_fluid_set_block_ms`

Required commands:

```bash
cargo fmt --check
cargo test --manifest-path native/Cargo.toml -p mclone-protocol -p mclone-client -p mclone-server -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:frame-budget:smoke
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --frame-budget-probe --frame-budget-frames 120 --target-hz 120
```

For rendered-output validation, capture and inspect a native screenshot because
client chunk patching affects the render source:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-section-delta-verify.png --width 640 --height 360 --chunk-radius 1
```

## Status Log

- Planned: tactical created with Java reference shape and first-slice scope.
