# R3: Browser Persistence Adapter

This slice follows `R0` through `R2` in the runtime/host arc from [`README.md`](README.md). The authority boundary already exists, browser singleplayer already runs behind a worker host, and chunk meshing already lives behind a client mesh-worker boundary. The next requirement is durability: browser singleplayer needs a real storage adapter on the authoritative side without turning IndexedDB into the canonical world model.

## Scope

Add:

- engine-native `WorldStorage` / `ChunkStorage` contracts
- engine-native save metadata shape for authoritative world saves
- a browser IndexedDB adapter behind those contracts
- authoritative-host load/save hooks around chunk view changes
- unit coverage for reopen/load and eviction hooks

Do not add:

- renderer-owned persistence
- IndexedDB-specific shapes in simulation code
- Node/file-backed persistence
- gameplay save data beyond current authoritative chunk snapshots and save metadata
- protocol reconnect/version-hardening beyond the metadata needed for this slice

## Reference shape vs divergence

Minecraft Java’s save system is not something we should transliterate class-for-class here. This is runtime architecture, not gameplay parity logic.

The relevant architectural reference from [`../../AGENTS.md`](../../AGENTS.md) is:

1. simulation/content parity remains the default
2. runtime orchestration may diverge for browser / worker / Node constraints
3. such divergences must preserve a clear path for future parity work

Persistence is one of those required divergences. The browser needs IndexedDB today; a dedicated host will need a different adapter later. The engine therefore needs logical world/chunk persistence records first, with browser storage as an adapter behind them.

## Landed shape

### Storage contracts

Added engine-facing storage types under `src/runtime/storage/`:

- `WorldStorage`
- `WorldStorageSession`
- `ChunkStorage`
- `WorldSaveMetadata`

These shapes are serializable, engine-native, and independent of IndexedDB layout.

### Browser adapter

Added `IndexedDbWorldStorage`, which:

- owns the IndexedDB schema/layout
- stores world metadata separately from chunk snapshot records
- records load/save/evict timestamps on chunk records as adapter-local policy hooks
- resets a save if stored metadata is incompatible with the requested authoritative world shape

### Host integration

`GeneratedWorldHost` now:

- opens a storage session during `open_world`
- returns `saveMetadata` in `world_opened`
- preloads stored chunk snapshots into the authoritative cache before generation on `set_chunk_view`
- persists authoritative chunk snapshots after view changes/mutations
- notifies chunk-storage eviction hooks when chunks leave the authoritative in-memory view

The browser worker now instantiates the IndexedDB adapter inside the authoritative worker, not on the renderer side.

## Why this shape

This cut keeps the authority and storage policy in the same place:

- renderer remains a consumer of authoritative chunk snapshots
- worker host remains the owner of generation, mutations, and persistence orchestration
- browser storage choice stays behind adapter interfaces

It also keeps the future `R4` path clean:

- browser worker host and Node host can share the same `WorldStorage` / `ChunkStorage` contracts
- only the adapter changes when we move from IndexedDB to file-backed persistence

## Validation

- `pnpm typecheck`
- `pnpm test`
- `pnpm test:browser`
- inspected `/tmp/mclone-browser-smoke.png`

## Next step

`R4`: stand up the same authoritative host/runtime core in a headless Node process with a file-backed storage adapter and a local integration harness.
