# 008: Native Persistence And Residency

Status: completed.

## Purpose

Put a storage-shaped boundary under the chunk scheduler before remote transport and broader streaming work. The server should no longer treat generated chunks as memory-only facts with no save/load lifecycle.

This is a scaffold, not final Anvil parity.

## Reference Source Read

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/IOWorker.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/RegionFileStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/ChunkSerializer.java`

## Reference Shape

Minecraft keeps scheduling and storage separate:

- `ChunkMap` owns holders, save decisions, and unload flow.
- `ChunkStorage` delegates actual disk IO to `IOWorker`.
- `RegionFileStorage` owns the filesystem container.
- `ChunkSerializer` owns the persisted chunk payload shape.

## Implemented Shape

Native keeps the same ownership split in a smaller form:

- `ChunkScheduler` owns resident holders, dirty flags, and a dirty save queue.
- `ChunkHolder` records `ChunkResidency`: `Generated`, `Saved`, or `LoadedFromStore`.
- `ChunkSnapshotStore` is the storage adapter trait.
- `NullChunkSnapshotStore` is the default for smoke paths and WASM compatibility.
- `FilesystemChunkSnapshotStore` is a native-only filesystem adapter.
- generated snapshots are dirty until `save_dirty_chunks()`.
- a new server over the same filesystem store reloads the saved snapshot and republishes it through the same `ServerUpdate::ChunkSnapshot` path.

The temporary filesystem format is a versioned little-endian chunk snapshot binary. It is intentionally not Anvil/NBT. It preserves the canonical native `ChunkSnapshot` facts from `mclone-core` so the next storage slices can replace the adapter or codec without changing client/server ownership.

## Scope Boundaries

In scope:

- holder residency state
- dirty save queue
- explicit save call and dirty cleanup
- native filesystem adapter scaffold
- load-before-generate hook
- save/load roundtrip through `IntegratedServer`
- WASM compile compatibility via default null store

Out of scope:

- Anvil `.mca` region files
- NBT chunk serialization
- async IO worker
- background save queue
- browser storage adapter
- chunk eviction policy beyond interest unload
- block entities, entities, ticks, lighting, or biome payloads

## Validation

Required gates:

```text
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-persistence-chunk.png --width 960 --height 640 --chunk-radius 1
pnpm native:web:smoke
git diff --check
```

Inspect `/tmp/mclone-native-persistence-chunk.png`; it should remain a nonblank multi-chunk terrain capture fed by the client/server/runtime path.

## Done

- scheduler holders distinguish generated/saved/loaded residency.
- generated chunks enter the dirty save queue.
- dirty chunks can be saved and marked clean.
- native filesystem store roundtrips a `ChunkSnapshot`.
- integrated server can reload a saved resident chunk and publish it through the existing protocol path.
- `mclone-web-client` still compiles and smokes with the default null store.

## Next

Proceed to [`009-dedicated-server-and-remote-transport.md`](009-dedicated-server-and-remote-transport.md): wire the dedicated server app and first remote transport around the same protocol/runtime shape.
