# 007: Chunk Interest Status Scheduler

Status: completed.

## Purpose

Replace the direct generate-on-interest shortcut with the first durable server-side chunk scheduling boundary.

The goal is not to clone Minecraft's full threaded chunk executor yet. The goal is to make chunk interest flow through holders, status slots, and scheduler events so later persistence, async workers, dedicated networking, lighting, and browser/WASM workers have a real runtime boundary to extend.

## Reference Source Read

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`

## Reference Shape

Minecraft routes chunk access through `ServerChunkCache` into `ChunkMap`, then through `ChunkHolder` instances. A holder owns per-status futures and `getOrScheduleFuture(...)` coalesces already scheduled or completed work. `ChunkStatus` is ordered and each status has dependency/range rules.

## Implemented Shape

Native keeps this slice synchronous but adopts the same ownership idea:

- `ChunkScheduler` owns `ChunkHolder`s.
- `ChunkHolder` owns per-`ChunkStatus` slots.
- `ChunkSchedulerEvent::StatusChanged` records scheduled/ready transitions.
- `ChunkSchedulerEvent::SnapshotReady` is the only scheduler event converted into `ServerUpdate::ChunkSnapshot`.
- duplicate interest requests return no new work once the target status is ready.
- interest movement unloads holders outside the view and publishes `ServerUpdate::ChunkUnload`.

Current generation only produces the MVP surface-stage chunk. The scheduler therefore targets `ChunkStatus::Surface` and records the collapsed `Terrain -> Surface` status path. Later slices can split actual terrain/surface/carver/feature work without changing the server/client protocol ownership boundary.

## Scope Boundaries

In scope:

- chunk holders keyed by `ChunkPos`
- per-status scheduled/ready slots
- deterministic status event ordering
- duplicate request coalescing
- unload publication when interest changes
- native app and web smoke continue through `IntegratedServer`

Out of scope:

- real async futures/executors
- native thread pools or WASM workers
- persistence-backed chunk load
- full vanilla status list
- neighbor dependency ranges beyond the collapsed MVP surface path
- lighting/full chunk promotion
- multi-client player ticket distance

## Validation

Required gates:

```text
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-scheduler-chunk.png --width 960 --height 640 --chunk-radius 1
pnpm native:web:smoke
git diff --check
```

Inspect `/tmp/mclone-native-scheduler-chunk.png`; it should remain a nonblank multi-chunk terrain capture fed by the client/server/runtime path.

## Done

- `IntegratedServer` delegates chunk interest to `ChunkScheduler`.
- `ChunkScheduler` has holders and status slots.
- duplicate interest coalesces without regenerating snapshots.
- scheduler status events prove deterministic `Terrain -> Surface` ordering.
- native and web smoke paths still publish the same protocol-shaped chunk snapshots.

## Next

Proceed to [`008-native-persistence-and-residency.md`](008-native-persistence-and-residency.md): add chunk residency, dirty/save queue shape, filesystem adapter scaffold, and a reload/resume hook.
