# Realm and Dimension Runtime

Topic: `realm-dimension-runtime`

Status: **implemented 2026-07-17; Tactical 185 Slices 0-8 are complete. Every
host uses one `RealmServer`, the integrated player is ordinary,
realm/dimension persistence is qualified, and one realm can concurrently tick
players in multiple isolated `DimensionRuntime`s. Source-owned player and
non-player observer interest, A-to-B-to-A player transfer, observer-backed
retained dioramas, and realm-scoped typed statistics are live.**

This topic owns the durable server-topology contract for realms, dimensions,
players, persistence, interest, and warm destination presentation. Detailed
implementation order and exit criteria live in
[`185-realm-dimension-and-observer-runtime.md`](../tactical/185-realm-dimension-and-observer-runtime.md).

## Core Decision

Mclone has one authoritative `RealmServer` implementation. Native integrated
single player, browser/Web Worker integrated single player, dedicated TCP,
dedicated WebSocket, and deterministic tests are host adapters around that same
core. Integrated mode is not a separate simulation and does not emulate the
hosted server.

One `RealmServer` owns one durable realm/save and may run an open-ended set of
dimensions concurrently. Different players may occupy different dimensions
while sharing realm-scoped player state. A second server is created only for a
genuinely independent realm, not for another dimension in the same realm.

The first structural cleanup is now complete:

- the shared authority is `RealmServer` and has no implicit player;
- local-only command/state branches and the privileged local player id are
  gone;
- `LocalRealmSession` registers the local profile as an ordinary realm player
  through the same core APIs used by hosted sessions;
- normalized in-memory, TCP, and WebSocket join/command traces are locked by
  tests before multi-dimension behavior is added.

Multi-dimension runtime, observer, transfer, and statistics work can now build
on the ordinary-player topology and collision-safe persistence rather than
preserving the retired local-player exception.

## Concept Model

```text
process / server host
  platform transport, storage adapter, lifecycle and operator policy

RealmServer
  one durable realm/save
  one realm player/session/statistics domain
  one registry of DimensionRuntime owners

DimensionRuntime
  one terrain/entity/tick/environment namespace
  dimension-local chunks, tickets, interest and persistence

client realm session
  one active physical player stream
  zero or more bounded observer streams/replicas
```

- **Realm** means one durable universe/save. It owns realm metadata, the
  dimension registry, player records, statistics, and realm saved data.
- **Dimension** means one independently addressed terrain/entity/tick space
  within a realm. Overworld, Nether, End, planets, authored spaces, and custom
  generators use the same primitive.
- **`RealmId`** is the stable opaque identity of a realm.
- **`DimensionKey`** is a stable namespaced instance identity such as
  `minecraft:overworld` or `mclone:mars`. It identifies content ownership, not
  only a generator type, and is not capped at three values.
- **`DimensionRuntime`** is the loaded scheduler, chunks, entities, ticks,
  lighting, interest, and scoped persistence for one dimension.
- **`WorldInstanceId`** is an ephemeral client/scene identity and must never be
  used as persistent realm or dimension identity.
- **Observer** means a non-player bounded publication/interest consumer. It is
  not a vanilla Spectator player and must not allocate or save player state.

## Vanilla 1.17.1 Receipt

The relevant vanilla architecture is deliberately unified:

- `IntegratedServer` and `DedicatedServer` both extend `MinecraftServer`.
- `MinecraftServer.createLevels` builds the server's map of `ServerLevel`
  dimensions for either host mode.
- the integrated client uses a memory connection, but it still traverses the
  ordinary connection and player lifecycle;
- every `ServerLevel` owns dimension-local chunk/ticket state;
- `ServerPlayer.changeDimension` moves the same player between levels;
- player data and `stats/<uuid>.json` live at the save root, so they are shared
  by dimensions but isolated between independent saves;
- the 1.17.1 client replaces its `ClientLevel` and shows a receiving-level
  screen on dimension change even though the server can keep dimensions live.

Primary reference anchors:

- `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/server/IntegratedServer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/dedicated/DedicatedServer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/network/ServerConnectionListener.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerPlayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`

Mclone extends this shape with arbitrary dimension definitions, optional lazy
dimension residency, explicit non-player observers, and a warm destination
client replica. Those extensions do not change the shared server topology.

## Verified Current Mclone State

The host topology, persistence model, and live dimension ownership are
unified:

- native integrated and browser/Web Worker hosts wrap `RealmServer` in a thin
  `LocalRealmSession`; dedicated TCP/WebSocket hosts own `RealmServer`
  directly;
- `RealmServer` owns one ordinary player registry and starts empty;
- the local adapter allocates one normal `ServerPlayerId` and delegates to the
  same command, publication, persistence, and safe-resume APIs as hosted
  players;
- the server owns a loaded `DimensionRuntime` map; schedulers, holders,
  scheduled ticks, entities, lighting, physics, interest, and scoped store
  handles are dimension-local while the game/day clock advances once per
  realm host tick;
- `RealmServer` owns an explicit, persisted `RealmId` and one-entry
  `DimensionRegistry`; exact v1 metadata receives a stable generated UUID on
  first open;
- `mclone-protocol` owns validated `DimensionKey` and `DimensionChunkPos`
  boundary types, while hot scheduler code remains scoped by runtime and uses
  plain `ChunkPos`;
- player records remain realm-root records keyed by profile UUID and carry a
  validated current `DimensionKey`;
- SQLite and IndexedDB chunk/entity records are dimension-qualified, with
  exact v1/v5 rows migrated to `minecraft:overworld` and native collision
  coverage for equal coordinates in different dimensions;
- players carry one current dimension and all command/publication paths route
  through that runtime; players and realm-global `ObserverId`s retain
  source-owned views and outbound queues inside that dimension;
- aggregate client residency and block/entity simulation interest are separate:
  players contribute both, residency-only observers use a border-level ticket,
  and explicitly ticking observers contribute both;
- observers receive configuration, world/time, chunks, block deltas, entities,
  and real players without owning player state, commands, identity, inventory,
  experience, or persistence records;
- one authoritative transfer operation moves the same realm player between
  dimension runtimes, resets only the client's dimension-local replica and
  render cache, validates a safe destination, and completes through the
  existing absolute-position acknowledgement while retaining identity,
  inventory, experience, and one realm-root record;
- native and browser retained dioramas now start the independent destination
  realm as a non-player observer. The observer receives bounded terrain,
  entities, real remote players, time, and live deltas without loading or
  saving the local profile's player record;
- scene presentation no longer manufactures a source-local player body for a
  preview observer;
- covered activation promotes the destination observer to one ordinary
  identity player, demotes and saves the source player to an observer, waits
  for the destination's authoritative safe position and drawable readiness,
  then exchanges the retained slots. A-to-B-to-A repeats the inverse exchange
  without duplicate players or preview-only persistence;
- the server and scene resolve preview framing and resumed entry through the
  ordinary safe-surface rules. Unsupported saved poses fall back to a nearby
  safe spawn rather than admitting a falling return position;
- `PlayerStatistics` is a typed, bounded realm-player map. Accepted grounded
  jumps award `minecraft:custom/minecraft:jump`; successful authoritative
  placements award the explicitly mclone-specific aggregate
  `mclone:custom/mclone:successful_block_placements`;
- statistics publish only to their owning player, follow that player through
  A-to-B-to-A dimension transfer, survive SQLite and IndexedDB restart, and
  remain independent between realms for the same client-global profile UUID;
- player-record codec v2 currently carries the typed statistics map inside
  the realm-root player envelope, with codec v1 defaulting to an empty map.
  This differs physically from vanilla's separate `stats/<uuid>.json` while
  preserving the same realm scope and an uncomplicated future storage split;
- the retired persistence demo no longer grants XP for jumping. The shared HUD
  now exposes jump and placement canaries directly.

## Hard Topology Invariants

These are implementation stop conditions, not aspirations:

1. All host modes instantiate the same realm server, session registry, player
   list, dimension registry, persistence rules, interest/ticket rules,
   transfer rules, autosave behavior, and authoritative simulation.
2. A local in-process profile joins and leaves as an ordinary realm player.
   The adapter may avoid encoding bytes but may not skip logical protocol state
   or server lifecycle.
3. One realm server may tick multiple dimensions concurrently. Each loaded
   dimension advances once per host tick; realm-global clocks do not advance
   once per dimension.
4. One realm player has exactly one active physical dimension and player
   entity at a time.
5. Dimension transfer moves that same realm player. It does not disconnect,
   manufacture a replacement player, or fork statistics/state.
6. Player records and statistics are realm-scoped. Chunks, ordinary entities,
   scheduled ticks, environment state, tickets, and observers are
   dimension-scoped.
7. Equal chunk coordinates or entity ids in different dimensions cannot
   collide in runtime state, storage, protocol routing, or diagnostics.
8. An observer may receive bounded world facts but never creates a player
   entity, loads/saves a player record, changes statistics, or receives
   physical interaction authority.
9. Native and browser implementations share the same Rust-owned topology and
   persistence semantics. Platform adapters do not interpret record blobs.
10. Independent realms retain independent player records and statistics even
    when the client-global profile UUID is the same.

## Host Boundary

Only actual deployment concerns may vary by host:

| Concern | Integrated | Dedicated |
| --- | --- | --- |
| Ordered transport | in-memory channel | TCP/WebSocket |
| Storage selection | app/platform local realm | operator-selected realm |
| Lifecycle | app close/background, optional pause | service shutdown/restart |
| Trust/admission | trusted local profile | configured remote authentication |
| Operator surface | owning client UI | console/admin API |

Pause, owner privilege, or local trust are explicit policy/capability inputs to
the shared server. They do not justify alternate player lists, dimension maps,
persistence, ticketing, transfer, simulation, or autosave mechanics.

The browser's Web Worker is an integrated host adapter, not a reduced server.
The desktop in-process lane is likewise an adapter, not the canonical gameplay
owner. Shared server behavior belongs in `mclone-server`; platform startup,
transport, roots, and lifecycle wiring stay in app/platform crates.

## Realm and Dimension Persistence

One deletable realm container should expose two logical scopes:

| Realm-scoped | Dimension-scoped |
| --- | --- |
| realm metadata and dimension registry | dimension definition/runtime record |
| player identity and current dimension/pose | chunks and block entities |
| statistics | ordinary entities |
| XP, selected slot and later inventory/vitals | block/fluid scheduled ticks |
| realm saved data and monotonic clock | environment/day/weather when local |
| session/account association | chunk interest/tickets and local spawn data |

The native store should qualify chunk/entity records with `dimension_key` and
the browser store should add the same logical key beneath its realm `worldId`.
Realm-root player/statistics APIs stay independent of dimension. Existing
worlds migrate as one realm containing `minecraft:overworld`; old player
records default to that dimension. Migration must be transactional or
explicitly recoverable.

## Player and Statistics Lifecycle

The client-global UUID identifies a profile across hosts, but each realm owns
its record for that UUID. On every host mode the ordinary join path:

1. admits a session and associates the presented profile;
2. loads the realm player record and realm statistics once;
3. validates the saved dimension and finds a safe saved/nearby spawn pose;
4. creates one authoritative player in that dimension;
5. saves through the same autosave, disconnect, and realm-close policies.

A dimension transfer updates current dimension and pose while retaining the
loaded realm player/statistics state. A cross-realm transition leaves/saves
realm A and separately joins/loads realm B.

Jumps and successful block placements are the first statistics canary over
this topology. The counters follow one player across dimensions, survive realm
restart, and remain independent in another realm for the same profile UUID.
Vanilla stores a separate JSON statistics file at the save root; mclone
currently embeds the separately typed map in player-record codec v2 so SQLite
and IndexedDB can reuse their transactional player-save lifecycle. Neither
shape makes the statistics dimension-local.

## Warm Destination Contract

Warm switching is a client presentation optimization over the same server
mechanics:

- for a same-realm preview, one `RealmServer` owns both dimensions;
- the client keeps its active player replica and may maintain a bounded
  observer replica for the destination;
- activating a warm destination performs the same authoritative player
  transfer and safe-arrival validation as a cold destination;
- warmth affects drawable readiness and cover duration, not identity,
  persistence, ticket semantics, or authority;
- integrated and dedicated sessions use the same contract;
- an independent-realm preview uses a distinct connection/realm server and
  becomes a player only through an explicit leave/join transition.

The current product diorama is still implemented as two retained independent
realm connections, but the inactive connection is an observer rather than a
second joined player. Same-realm dimension previews can later reuse the
existing realm observer and transfer primitives without changing the client
presentation contract.

## Validation Contract

The topology cleanup is complete only when tests compare normalized
in-memory, TCP, and WebSocket traces for configuration, join, ordinary play,
save, disconnect, reconnect, and later dimension transfer. Framing and
platform lifecycle events may vary; accepted commands, state transitions,
authoritative updates, persistence effects, and cleanup must agree.

Multi-dimension validation must additionally prove:

- two players can remain active in different dimensions;
- equal coordinates and ids do not collide;
- observers and players can overlap without stealing each other's tickets;
- removing either interest source preserves the other;
- A-to-B-to-A moves one player and retains realm state;
- realm clocks, autosave, unload, and shutdown run once at the correct scope;
- native SQLite and browser IndexedDB migrate and reopen equivalently;
- warm preview/activation works through shared mono, stereo, and multiview
  presentation without a preview-only renderer.

## Code and Ownership Map

Current seams that the next slices must change or protect:

- `native/crates/mclone-server/src/integrated.rs`: multi-dimension
  `RealmServer` authority plus the thin role-aware `LocalRealmSession`
  adapter, player save, observer promotion/demotion, ticks, and publication;
- `native/crates/mclone-server/src/players.rs`: runtime player identities and
  realm player list;
- `native/crates/mclone-server/src/runner.rs`: native integrated runner,
  lifecycle save policy, and shared server cadence;
- `native/crates/mclone-server/src/player_chunk_tracking.rs`: source-owned
  player/observer views and residency/simulation ticket aggregation;
- `native/crates/mclone-server/src/persistence.rs`: record codecs, native
  SQLite schema, and store ownership;
- `native/apps/mclone-dedicated-server/src/main.rs`: dedicated TCP/WebSocket
  host around the same empty-at-construction `RealmServer`;
- `native/crates/mclone-app-runtime/src/client_connection.rs`: in-memory
  runner connection and client connection contract;
- `native/crates/mclone-app-runtime/src/native_service_assembly.rs`: local
  startup assembly and platform-selected storage;
- `native/crates/mclone-scene/src/session.rs`: active/warm client session
  ownership and preview startup;
- `native/crates/mclone-protocol`: shared commands, updates, configuration,
  play, transfer, and observer wire contracts as they evolve.

The shared owner remains `mclone-server`. App crates may assemble transports,
storage roots, event loops, browser workers, and process lifecycle, but may not
take ownership of realm/dimension gameplay policy.

Slice 0 pinned the former singleton state with executable receipts: an exact
SQLite v1 fixture, an IndexedDB v5 key-shape lock, an Overworld-only resume
test, and a source/behavior lock proving the old standby preview joined a full
local player. Later slices replaced those assumptions while retaining their
migration evidence. Shared identifiers and qualified cross-boundary addresses
live in `mclone-protocol`; authoritative definitions, persistence, runtime
membership, interest, and transfer remain owned by `mclone-server`.

## Completed Implementation Order

Tactical 185 followed this architectural order:

1. lock current behavior and migration fixtures;
2. unify the realm server name/ownership and ordinary local session path;
3. add neutral realm/dimension identities;
4. qualify persistence by dimension;
5. host multiple authoritative dimension runtimes;
6. generalize interest to player and observer sources;
7. transfer one player between dimensions;
8. adopt observers for dioramas;
9. add realm-scoped jump/place statistics as the acceptance canary.

All nine steps are complete. New gameplay systems can now use the executable
realm/dimension boundary rather than preserving a singleton assumption.
Inventory/vitals, real portal travel and richer statistics are separate
feature tacticals. Tactical 184 already closed the bounded session-liveness
work; further networking changes should use a new follow-up.

## Related

- [`embedded-worlds.md`](embedded-worlds.md)
- [`multiplayer-networking.md`](multiplayer-networking.md)
- [`../persistence-architecture.md`](../persistence-architecture.md)
- [`../tactical/174-warm-world-hot-swap.md`](../tactical/174-warm-world-hot-swap.md)
- [`../tactical/182-local-profile-and-player-persistence-proof.md`](../tactical/182-local-profile-and-player-persistence-proof.md)
- [`../tactical/184-session-configuration-liveness-and-disconnect.md`](../tactical/184-session-configuration-liveness-and-disconnect.md)
- [`../tactical/185-realm-dimension-and-observer-runtime.md`](../tactical/185-realm-dimension-and-observer-runtime.md)
