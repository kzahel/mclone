# World And Dimension Storage Layout

Topic: `world-dimension-storage-layout`

Status: accepted direction 2026-07-20. Native dimension-sharded SQLite is not
implemented and no tactical is authorized yet. The browser retains its current
consolidated IndexedDB layout unless evidence justifies a separate change.

## Scope

This topic owns the physical storage layout beneath the implemented logical
`WorldStore` and record-executor contracts:

- which records are realm-global or dimension-local;
- how native SQLite files are divided within one saved realm;
- why browser IndexedDB may use a different physical division;
- which atomicity, lifecycle, and recovery guarantees cross a shard boundary;
  and
- how a future layout change remains hidden from simulation and scene code.

It does not redefine persistence request/completion semantics, record codecs,
managed-world provisioning, schema-migration UX, or the cross-platform
actor/mailbox model. Those concerns live in
[`unified-persistence-interface.md`](unified-persistence-interface.md),
[`cross-platform-operation-execution.md`](cross-platform-operation-execution.md),
and tactical execution records.

## Accepted Logical Model

One `RealmServer` owns one durable realm/save and an open-ended set of
dimensions. The standard vanilla-shaped save will normally have Overworld,
Nether, and End dimensions, but neither the engine contract nor the physical
router may assume exactly three.

```text
one durable realm/save
  |
  +-- realm-global metadata, players, saved data, dimension registry
  |
  +-- minecraft:overworld
  +-- minecraft:the_nether
  +-- minecraft:the_end
  `-- arbitrary namespaced dimensions
```

`DimensionKey` remains present in every shared Rust request and record address.
It is a logical identity and routing key, not a promise that its text is stored
in every physical row. `WorldInstanceId` remains an ephemeral scene identity
and must never select durable storage.

The world writer lease and persistence actor remain realm-scoped. Physical
sharding does not authorize arbitrary compute workers to open dimension
databases or create multiple independent writers. One actor may initially own
and sequence several lazily opened SQLite connections; subordinate
dimension-I/O actors are a later measured optimization, not part of this
decision.

## Current Physical Layout

### Native

Each catalog or managed world has its own directory and one `world.sqlite3`.
That database contains realm-global records and every dimension's records.
Chunk and entity-chunk primary keys include `(dimension_key, x, z)`, while
dimension records use `dimension_key` directly.

This is logically correct and already indexed efficiently. The composite
primary key supports exact chunk lookup; dimension sharding is not a repair for
a missing index.

### Browser

All browser worlds currently share IndexedDB database `mclone-web-worlds`
version 6. Object-store keys partition records by logical identity:

```text
world metadata        worldId
dimension metadata    [worldId, dimensionKey]
chunks                 [worldId, dimensionKey, x, z]
entity chunks          [worldId, dimensionKey, x, z]
players                [worldId, playerKey]
```

Several Workers may hold IndexedDB connections, but the exclusive Web Lock is
world-scoped. Shared physical storage therefore does not imply shared writable
authority.

## Vanilla 1.17.1 Reference

Vanilla uses one save directory and directory lock for the durable world while
physically routing dimension-local data beneath separate paths:

```text
save/
  level.dat
  playerdata/
  region/ and entities/ and poi/       # Overworld
  DIM-1/...                            # Nether
  DIM1/...                             # End
  dimensions/<namespace>/<path>/...    # custom dimensions
```

Each `ServerLevel` receives its dimension path. Chunk, entity, and point-of-
interest storage use separate `IOWorker`/mailbox owners over the shared I/O
pool. Vanilla therefore demonstrates the distinction this topic preserves:
one logical save and authority boundary may contain several physical storage
shards and I/O owners.

Primary reference anchors:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/storage/LevelStorageSource.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/dimension/DimensionType.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/IOWorker.java`

Mclone does not need to reproduce vanilla's region-file format or one worker
per record family. The instructive precedent is independent physical routing
beneath one save-level lifecycle.

## Accepted Native Direction

Native durable worlds should move toward one realm-global SQLite database plus
one SQLite database per dimension:

```text
world/
  world.sqlite3                         # realm-global
  dimensions/
    <encoded dimension key>/
      dimension.sqlite3                 # dimension-local
```

The exact collision-safe filesystem encoding of a namespaced `DimensionKey`
is left to the tactical. The layout must support custom keys and must not
hard-code the three vanilla dimensions.

The realm-global database should own:

- world metadata;
- player records;
- realm-global saved data;
- the dimension registry and physical routing metadata; and
- any world-level recovery or coordination records later proven necessary.

Each dimension database should own:

- an identity marker binding it to its expected realm and `DimensionKey`;
- dimension metadata;
- block chunk records; and
- entity-chunk records.

Connections should open lazily and close beneath the one world lifecycle. A
normal three-dimension realm therefore has one global database and at most
three dimension databases, not a fixed requirement to keep four connections
open.

### Physical key policy

A dedicated dimension database should normally key chunk tables by `(x, z)`.
It should store its full `DimensionKey` once as validated database metadata
rather than repeating the string in every chunk and entity-chunk row.

The shared request still carries `DimensionKey`; the native router consumes it
to select and validate the shard. If a future backend groups multiple
dimensions in one database, that backend may use a compact local dimension id
or the current composite key without changing the shared API. Optional future
grouping is therefore not a reason to retain redundant dimension strings in a
dedicated shard.

### Expected benefits

- narrower chunk and entity-chunk B-tree keys and better page-cache density;
- failure and corruption isolation between dimensions;
- independent WAL, checkpoint, inspection, and future recovery boundaries;
- lazy physical residency for dimensions that are not opened; and
- a path to measured parallel dimension I/O without changing simulation
  requests.

The index-size benefit is real but likely modest relative to encoded chunk
payloads. Isolation and future scheduling freedom are the primary reasons for
the direction.

### Costs and semantic limits

- several files, WALs, and connections require coordinated flush, close,
  backup, deletion, and diagnostics;
- SQLite cannot provide one ordinary atomic transaction across independently
  committed dimension databases;
- a realm may be partially healthy when one dimension shard is unavailable;
- world publication and backup still operate on the complete directory; and
- introducing subordinate I/O actors too early would add scheduling and
  shutdown complexity without measured benefit.

The generic executor currently promises atomic application of one
`PersistenceRecordBatch`. Before sharding, the tactical must inventory whether
production batches can span realm-global and dimension-local records or more
than one dimension. The portable first contract should make one batch local to
one physical shard. A cross-shard batch must be rejected, split only when its
caller does not require atomicity, or represented by an explicit recoverable
world-level operation.

Dimension transfer should not depend casually on a distributed SQLite
transaction. If transfer eventually needs crash-recoverable changes in several
shards, use an idempotent realm-global intent/recovery record and explicit
completion protocol.

## Accepted Browser Direction

Do not reproduce native file sharding mechanically in IndexedDB.

The first direction remains one IndexedDB database with records partitioned by
`worldId` and `dimensionKey`, plus one writer lease per logical world. IndexedDB
database creation, version upgrades, open connections, blocked deletion, and
transaction scopes have different costs from opening a small set of SQLite
files. Repeating compact logical keys beside opaque chunk payloads is currently
the better tradeoff.

If browser measurements or product requirements later justify more isolation,
per-world IndexedDB databases are a more plausible first review than
per-dimension databases. That would still require a separate catalog and
careful open/delete/upgrade coordination, while browser quota and eviction
remain origin-scoped.

TypeScript continues to map stable Rust-authored namespaces and keys to
IndexedDB mechanics. It does not choose whether dimensions share a database.

## Compatibility And Migration

The accepted target is a physical-format change only. `WorldStore`,
`WorldStoreRequest`, `WorldStoreCompletion`, record codecs, revisions, and
logical `DimensionKey` addressing should remain stable.

Current internal worlds are disposable and are not a compatibility requirement
for the native sharding tactical. Prefer an intentional pre-release format
break with clear diagnostics over carrying forward the existing native
exact-v1 compatibility path. If a future shipped format genuinely requires
preservation, use an out-of-band or pre-start conversion with a staged result
and recoverable publication before a writable realm starts; do not introduce
live mid-session migration semantics.

The browser layout is unchanged by this direction. Its former v5-to-v6
Overworld cursor copy has been deleted, and production TypeScript contains no
record migration policy.

## Questions For The Implementing Tactical

1. What collision-safe path encoding maps arbitrary namespaced dimension keys
   to shard directories on every native filesystem?
2. Which current `DimensionRecord` fields belong in the root registry, the
   dimension identity metadata, or both?
3. Do any production record batches currently cross a shard boundary, and what
   atomicity does each caller actually require?
4. Which existing native worlds must survive the layout change, and what
   pre-start or offline conversion owns that responsibility?
5. How are read-only inspection, app-private fallback creation, backup,
   deletion, and directory publication coordinated across the complete shard
   set?
6. Does one corrupt or missing dimension prevent opening the realm, or can a
   typed degraded/recovery state preserve unaffected data?
7. What measurements would justify per-dimension child actors instead of one
   world persistence actor routing several connections?

## Recommended Tactical Shape

1. Record current single-file behavior, batch composition, write volume, page
   counts, index sizes, flush latency, and multi-dimension restart evidence.
2. Define the root and dimension schemas plus the stable native routing
   metadata without changing shared engine requests.
3. Implement a native sharding executor/router beneath `WorldStore`, initially
   owned by the existing single persistence actor.
4. Prove arbitrary dimension keys, identical coordinates in separate shards,
   player transfer restart, lazy open, flush/close, writer conflict, and
   isolated shard failure.
5. Add a pre-start converter only if the reviewed compatibility decision
   requires preserving existing single-file worlds.
6. Validate desktop, dedicated server, Android, and Quest lifecycle behavior;
   preserve browser conformance without changing IndexedDB layout.
7. Update `persistence-architecture.md` only after the new layout becomes the
   implemented durable system shape.

No actor-framework generalization, IndexedDB redesign, online migration
language, or per-dimension thread topology is required for the first slice.
