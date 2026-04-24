# Loading And Persistence

Durable guidance for world creation, joining, chunk loading, generation, saving, and eviction.

[`architecture.md`](./architecture.md) owns runtime boundaries. [`runtime-data-model.md`](./runtime-data-model.md) owns the chunk data shape. [`protocol.md`](./protocol.md) owns host/client messages. [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md) owns scheduler rules that keep player/session authority responsive while chunk jobs run.

## Core Rule

The authoritative host owns world lifecycle and persistence policy.

The client expresses interest and sends commands. The renderer consumes authoritative outputs. Storage adapters persist logical world/chunk records. None of those adapters become the simulation model.

## Create, Open, Join

Singleplayer and multiplayer should share the same conceptual flow:

1. host opens or creates the save
2. client joins or resumes a session
3. client declares chunk interest
4. host loads/generates chunks needed for aggregate interest
5. host sends authoritative snapshots and later deltas

Browser singleplayer differs only in host placement:

- local singleplayer host: browser worker
- dedicated host: Node process

The renderer should not call worldgen directly in either mode.

## Chunk Lifecycle

Authoritative chunks should move through explicit states:

```text
unloaded
  -> loading_from_storage
  -> generating
  -> loaded_clean
  -> loaded_dirty
  -> saving
  -> evictable
  -> unloaded
```

Important rules:

- storage lookup happens before generation
- generation is deterministic, but generated chunks become authoritative world state once loaded
- mutations make chunks dirty
- dirty chunks should be saved before eviction unless the active storage policy explicitly says otherwise
- clients receive snapshots after a chunk reaches loaded state
- clients receive unloads when their session interest no longer includes a chunk

This lifecycle can be implemented incrementally. The states should still guide naming and responsibilities.

## Loading Order

When chunk interest changes, the host should:

1. compute aggregate chunk interest across sessions
2. find newly required chunks
3. try storage for each newly required chunk
4. generate missing chunks through the simulation/worldgen core
5. apply pending generation-stage side effects recorded by the current pipeline
6. publish snapshots to sessions that need them
7. mark chunks as clean or dirty according to how they were obtained and mutated
8. identify chunks no longer required by any session
9. save dirty evictable chunks according to policy
10. evict chunks after persistence obligations are satisfied

The client render-world worker may have its own client-side cache lifecycle, but that cache is not authoritative.

Chunk lifecycle work must not make input or authoritative player ticks wait for the full view to finish loading. A chunk-interest change should update host residency requirements and queue chunk work promptly; snapshots should be published as chunk jobs complete. See [`authoritative-host-scheduling.md`](./authoritative-host-scheduling.md).

## Lazy Persistence

Lazy persistence is acceptable as an IO policy. It is not an ownership model.

Acceptable lazy policies:

- debounce saves for recently dirtied chunks
- batch chunk writes
- save on eviction, world close, or periodic flush
- track clean generated chunks separately from mutated chunks if the policy is explicit

Risky policies that need an explicit decision:

- never saving generated-but-unmodified chunks
- regenerating chunks on every load and only storing player mutations
- treating IndexedDB or file layout as the canonical world model

For vanilla-parity work, the safer default is:

- a loaded chunk is authoritative world state
- dirty chunks are persisted before eviction
- generated clean chunks may be persisted eagerly or lazily, but the policy should be documented and measurable

This keeps room for later optimization without hiding state ownership.

## Save Metadata

World metadata should be engine-native and adapter-independent.

Minimum metadata:

- save id
- storage schema version
- seed
- preset/profile
- min build height
- height
- created timestamp
- last opened timestamp

Opening a save with incompatible metadata should fail or reset through an explicit policy. It should not silently mix chunks from different world definitions.

## Storage Adapters

Storage contracts should remain logical:

- open world
- load chunk
- save chunk
- mark/observe eviction
- close/flush

Adapter choices:

- browser singleplayer: IndexedDB baseline, OPFS still possible later
- Node host: file-backed baseline, region-style or SQLite possible later
- tests: memory storage

Adapters can use different physical layouts. They should not require changes to worldgen, simulation, protocol semantics, or renderer code.

## Remote Sessions And Aggregate Interest

A dedicated host should share one authoritative world per save/profile and track per-session interest.

For each save:

- sessions have independent player/session state
- each session has its own chunk interest
- the host computes aggregate chunk residency from all active sessions
- chunks are generated/loaded once per authoritative world
- snapshots/deltas are filtered per session

This preserves the same ownership model as local singleplayer while allowing multiple clients.

## Future Parity Work

The current authoritative player loop is not vanilla movement parity yet. As gameplay grows, the same lifecycle still applies:

- player actions are commands
- server-side systems mutate authoritative world/entity state
- mutated chunks become dirty
- clients receive authoritative deltas
- persistence follows dirty state

Do not put gameplay truth in the renderer or in a client cache to avoid a host-side lifecycle problem.
