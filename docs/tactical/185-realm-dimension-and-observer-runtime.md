# Tactical 185: Realm, Dimension, and Observer Runtime

Status: proposed 2026-07-16; reference/current-state audit complete;
implementation not started

Workstream: native Rust, shared server/runtime/persistence/protocol first;
native web/WASM adapters in the same slices

Topics: `realm-dimension-runtime`, `multiplayer-networking`, `embedded-worlds`

## Objective

Make one authoritative realm capable of owning an open-ended registry of
dimensions and running more than one dimension concurrently. Different
players must be able to occupy different dimensions while sharing one
realm-scoped player identity, statistics record, and later inventory/health
state. Chunk, entity, tick, and persistence state must remain dimension-local.

Replace preview-only diorama membership with an explicit non-player observer
interest source. An observer may load, receive, render, and optionally keep a
bounded region ticking, but it must not create a player entity, load or save a
player record, mutate player statistics, participate as a remote-player
subject, or receive physical interaction authority.

The first realm remains hosted by one process. This tactical preserves stable
realm/dimension addresses but does not implement multi-process sharding,
leases, distributed entity migration, or MMO ownership.

## Why This Comes Before General Statistics

The persistence proof currently treats one opened mclone world store as both
the complete save and its only Overworld simulation. Adding a general
statistics map immediately would work, but its intended scope would remain
implicit. This tactical makes the boundary executable first:

```text
client-global profile UUID
  identifies the same person across hosts

realm
  owns player records and statistics for that UUID

dimension
  owns terrain, entities, scheduled ticks, local environment, and chunk
  interest for one independently addressed simulation space
```

After this boundary exists, jumps and successful block placements become a
small acceptance canary: the same counter follows a player between dimensions
inside one realm, while the same UUID starts with independent counters in a
different realm.

## Terminology and Identity Contract

The following names are semantic boundaries, not necessarily final Rust type
names:

- **Server host**: one process and its platform/network adapters. Process
  ownership is deployment topology, not persistent identity.
- **Realm**: one durable save/universe. It owns realm metadata, a dimension
  registry, realm player records, statistics, and named realm saved data.
- **`RealmId`**: stable opaque identity for a realm. A catalog world or managed
  world currently supplies this ownership domain even if the stored id is not
  yet represented by a first-class Rust type.
- **Dimension**: one independently addressed terrain/entity/tick namespace
  inside a realm. Overworld, Nether, End, planets, authored spaces, and custom
  generators all use this same primitive.
- **`DimensionKey`**: stable namespaced identity such as
  `minecraft:overworld`, `minecraft:the_nether`, or `mclone:mars`. There is no
  hard three-dimension limit.
- **`DimensionDefinition`**: persistent type/generator/environment facts for a
  dimension: seed or seed derivation, generation profile, build bounds,
  coordinate scale, sky/environment rules, and compatibility versions.
- **`DimensionRuntime`**: the currently loaded scheduler, chunks, entities,
  ticks, lighting, interest sources, and persistence scope for one dimension.
- **`WorldInstanceId`**: scene-local ephemeral identity for a drawable runtime.
  It must never substitute for `RealmId` or `DimensionKey` in persistence or
  protocol contracts.
- **Observer**: a non-player chunk/entity publication consumer. Do not call it
  a vanilla Spectator player; a vanilla spectator is still a real
  `ServerPlayer` and is the wrong ownership model for a diorama-only view.

`DimensionKey` identifies an instance, not merely a generator class. Multiple
dimensions may share one definition family while owning different durable
content:

```text
mclone:earth -> overworld-like definition, seed A
mclone:mars  -> overworld-like definition, seed B, environment B
mclone:moon  -> moon definition, seed C
```

The product may call these planets or worlds. The engine primitive remains a
dimension until spherical topology, orbital relationships, or another real
simulation distinction requires a separate concept.

## Vanilla 1.17.1 Reference Shape

Read before implementation:

- `MinecraftServer.createLevels` builds an Overworld `ServerLevel`, then
  iterates every other registered `LevelStem` into the server's
  `Map<ResourceKey<Level>, ServerLevel>`.
- `LevelStem` gives the three built-ins preferred ordering but copies arbitrary
  additional registry entries after them. The stable vanilla preset is the
  three built-ins; the registry/runtime is not structurally capped at three.
- `DimensionType.getStorageFolder` stores built-ins in their historical
  locations and custom dimensions below
  `dimensions/<namespace>/<path>`.
- Each `ServerLevel` owns its own `ChunkMap`/`DistanceManager` and therefore
  its own chunk coordinates and ticket set.
- `ServerPlayer.changeDimension` removes the same player entity from one
  `ServerLevel`, switches its level and pose, inserts it into the destination,
  and refreshes the existing connection. It does not create a second player
  record or statistics file.
- `PlayerList` loads player data and `stats/<uuid>.json` from the save root, so
  those facts are shared across dimensions but not across independent saves.

Primary source anchors:

- `reference/minecraft-1.17.1/src/net/minecraft/server/MinecraftServer.java`
  (`levels`, `createLevels`, `getAllLevels`)
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/dimension/LevelStem.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/dimension/DimensionType.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerPlayer.java`
  (`changeDimension`)
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/stats/ServerStatsCounter.java`

Intentional mclone extensions:

- dimension definitions may represent planets or arbitrary generated/authored
  spaces rather than only the built-in three;
- registered dimensions may load and unload lazily rather than all remaining
  resident for the full realm lifetime;
- a non-player observer interest source supports live previews without
  manufacturing a second player;
- realm and dimension identity remain independent of the process that happens
  to host them, while this tactical still uses exactly one host process.

## Verified Current Mclone State

### Server and simulation

- `IntegratedServer` owns exactly one `ChunkScheduler`, entity runtime,
  `PlayerChunkTracking`, time state, and opened `WorldStore`.
- All scheduler, entity, dirty-record, and publication maps use unqualified
  `ChunkPos`. Two dimensions at the same `(x, z)` would collide if placed in
  one current instance.
- `NativeIntegratedServerRunner` and the dedicated-server host each tick one
  `IntegratedServer`.
- `ServerPlayerList` is attached directly to that one simulation. The local
  player is structurally special (`ServerPlayerId::LOCAL`) while dedicated
  players occupy the same single dimension.
- `PlayerRecord.dimension` exists, but writers hard-code
  `minecraft:overworld` and resume validation rejects every other value.

### Persistence

- Native SQLite stores realm-like metadata and dimension-like records in one
  `world.sqlite3`, but `chunk_records` and `entity_chunk_records` primary keys
  contain only `(x, z)`.
- `player_records` is already keyed independently by profile UUID, which is
  the correct eventual realm-level shape.
- Browser IndexedDB keys records by catalog/managed `worldId`, then chunk
  coordinates or player UUID. It likewise has no dimension key.
- `WorldMetadataV1` owns one seed/generation profile and one two-clock state.
  It does not own a dimension registry or per-dimension environment records.

### Ticketing and interest

- `PlayerChunkTracking` owns a `BTreeMap<ServerPlayerId, ...>` and reduces all
  player views to one aggregate `BTreeSet<ChunkPos>`.
- `ChunkDistanceManager` converts that aggregate set into `Player` tickets and
  also supports other vanilla-shaped ticket types. Player ticket identity is
  the aggregate chunk position rather than the contributing source.
- The current union correctly protects overlapping player views, but it cannot
  describe a non-player observer, distinguish source policy, report source
  ownership, or route the same coordinate in multiple dimensions.

### Diorama preview

The current preview is not observer-only:

1. `McloneSceneHost::begin_prepared_warm_world_standby...` creates a complete
   second `LocalIntegratedStartupPump` or accepts a complete external runtime.
2. The integrated runner contains its normal built-in local player.
3. Startup sends the normal `SetChunkView`, resolves a player spawn/resume
   pose, and builds a complete `ClientRuntime`.
4. Scene policy suppresses physical movement/interaction while that slot is
   standby, but the server-side player still exists.
5. Presentation deliberately synthesizes the standby connection's local
   player body because clients normally do not receive themselves as remote.

This shape was useful for proving two complete warm runtimes and atomic slot
exchange. It must remain valid for genuinely independent realms until a
cross-realm observer/join lifecycle replaces it, but it is not the target for
same-realm dimension preview.

## Integrated and Hosted Topology Invariant

Mclone must have one authoritative realm-server implementation, not an
integrated-server emulation of dedicated behavior and a separate hosted
server. Native in-process single player, browser/Web Worker single player,
dedicated TCP, dedicated WebSocket, and deterministic tests all instantiate
the same `RealmServer` core and drive the same logical session protocol.

This follows vanilla's important topology even though mclone's implementation
details differ:

- vanilla `IntegratedServer` and `DedicatedServer` both extend
  `MinecraftServer`;
- shared `MinecraftServer.createLevels` owns the dimension map in both cases;
- an integrated client uses a memory connection, but still joins the server
  through the ordinary connection/player lifecycle;
- dimension transfer and player/statistics persistence therefore remain
  server mechanics rather than single-player approximations.

Mclone already has part of this shape: both the native integrated runner and
the dedicated host construct the type currently named `IntegratedServer`.
However, the shared core still contains a structurally privileged local
player (`ServerPlayerId::LOCAL`, `CommandTarget::Local`, parallel local-player
state), and the dedicated host compensates by calling
`disable_local_player()`. The name hides the useful sharing while the local
exception prevents real topology equivalence.

The target is:

- rename or extract the shared authority as `RealmServer`;
- make a local in-memory connection join an ordinary session and ordinary
  realm player through the same state machine as a remote connection;
- allow the in-memory adapter to skip byte serialization, but not logical
  commands, ordered updates, configuration/play state, player registration,
  disconnect cleanup, persistence, or restoration;
- run one realm host/tick boundary around the same server core in every mode;
- keep player statistics, current dimension, transfer, ticketing, autosave,
  and safe resume inside that shared core;
- host multiple dimensions for one realm in that one server. Start a second
  realm server only for a genuinely independent realm/save, never merely
  because a player changed dimensions.

Host adapters may differ only where deployment actually differs:

| Concern | Integrated host | Dedicated host |
| --- | --- | --- |
| Transport | in-memory ordered channel | TCP or WebSocket |
| Storage adapter/root | platform-selected local realm | operator-selected realm |
| Lifecycle | app close/background and optional pause | service shutdown/restart |
| Trust/auth | client-global local profile trust | configured remote admission |
| Operator control | owning client UI | console/admin surface |

They may not own different player lists, statistics stores, dimension maps,
chunk-ticket rules, transfer rules, simulation cadence, autosave semantics, or
gameplay behavior. A single-player owner exemption, such as pause policy or
local administrative permission, must be an explicit capability/policy input
to the shared server rather than a parallel code path.

Conformance tests must compare normalized session traces across in-memory,
TCP, and WebSocket adapters. Transport framing may differ; accepted commands,
state transitions, authoritative updates, persistence effects, and disconnect
cleanup must not.

## Warm Transition Is Client Presentation, Not Server Topology

Vanilla keeps all server dimensions under one `MinecraftServer`, but its
1.17.1 client handles a dimension-change respawn by replacing `ClientLevel`
and showing a receiving-level screen. Mclone's live warm swap is an intentional
client presentation improvement, not a reason to fork the authoritative
server topology.

For a same-realm destination:

- the one `RealmServer` keeps source and destination dimensions live according
  to their interest and unload policies;
- the client may retain the active player replica plus a bounded observer
  replica of the destination;
- activation still performs the same single authoritative player transfer
  whether the destination is cold or warm;
- warm state changes cover duration and drawable readiness only. It cannot
  create a second player, bypass transfer validation, or change persistence;
- the contract is identical for integrated and dedicated hosts.

For an independent realm preview, the second realm necessarily has its own
realm server and connection/session ownership. Warming that realm does not
merge its player record, statistics, dimension registry, or authority with the
active realm.

## Target Ownership Shape

```text
mclone-server::RealmServer
  RealmId
  RealmMetadata
  DimensionRegistry
  RealmPlayerList
  PlayerStatistics
  SessionRegistry
  BTreeMap<DimensionKey, DimensionRuntime>

DimensionRuntime
  DimensionDefinition / DimensionRecord
  ChunkScheduler
  Entity runtime
  scheduled block/fluid ticks
  lighting/worldgen workers
  DimensionInterestTracker
  dimension-scoped persistence handle

Host adapters (same RealmServer core)
  native in-process integrated host
  browser Web Worker integrated host
  dedicated TCP/WebSocket host
  deterministic test host
```

One host tick advances realm-global lifecycle once and each loaded dimension
once. A command resolves its session/player first, then its current
`DimensionKey`, and is dispatched to exactly that runtime. Global cadence must
not advance realm clocks once per loaded dimension.

At least these facts are realm-scoped:

- stable player/profile association;
- player statistics;
- current dimension plus current pose;
- XP and selected slot already present;
- inventory, health, hunger, abilities, effects, advancements, and scoreboard
  state when those systems exist;
- a realm-wide monotonic game clock if retained.

At least these facts are dimension-scoped:

- chunks, light, block entities, and ordinary entities;
- scheduled block/fluid ticks;
- generation/environment definition and compatibility;
- local spawn/portal destinations;
- dimension daylight/weather state when independent planet days land;
- chunk tickets, interest sources, and ticking eligibility.

Do not add `DimensionKey` to every hot inner-loop `ChunkPos`. A
`DimensionRuntime` provides the namespace internally. Cross-dimension APIs,
persistence keys, diagnostics, protocol multiplexing, and realm-level maps use
an explicit qualified address such as `DimensionChunkPos`.

## Persistence Target

Keep one deletable realm container while separating root and dimension record
families. A likely native schema is:

```text
realm_metadata
dimension_records(dimension_key, codec_version, revision, record_blob)
chunk_records(dimension_key, x, z, codec_version, revision, record_blob)
entity_chunk_records(dimension_key, x, z, codec_version, revision, record_blob)
player_records(player_key, codec_version, revision, record_blob)
player_statistics(player_key, codec_version, revision, record_blob)  # later
saved_data_records(data_key, ...)
```

Browser IndexedDB uses the same logical keys, with `worldId` continuing to
select the realm container and `dimensionKey` added to dimension-local record
keys. Platform adapters must not interpret the Rust-owned record blobs.

The current store migrates as one realm with one
`minecraft:overworld` dimension. Older player records default their dimension
to `minecraft:overworld`; older chunk/entity records are moved or interpreted
under that same key. Migration must be transactional or explicitly
recoverable, and newer unknown schemas remain a hard error.

`WorldStore` may evolve into a realm store that issues scoped
`DimensionStoreHandle`s. The handle automatically prefixes dimension-local
keys so callers cannot accidentally read another dimension's `(x, z)`. Player
and statistics APIs remain on the realm root.

## Interest and Ticket Contract

Generalize player-only view ownership into explicit sources:

```text
InterestSourceId
InterestSourceKind
  Player(ServerPlayerId)
  Observer(ObserverId)
  Forced(name)
  Spawn
  Portal

DimensionInterest
  source_id
  center/radius or bounded region
  required chunk status
  publication target, if any
  simulation policy
```

Required invariants:

- removing one source never removes coverage still required by another;
- equal chunk coordinates in different dimensions never share tickets,
  holders, jobs, priority centers, or diagnostics;
- player interest may drive player visibility, ticking, and outbound routing;
- observer interest never creates a player entity or player record;
- observation and simulation are separate decisions. A static preview can ask
  for client-visible chunks only; a live diorama can explicitly request block
  and entity ticking without pretending a player is present;
- every source has bounded radius/bytes and can be removed atomically on
  disconnect, preview cancellation, dimension unload, or realm close;
- diagnostics attribute active chunks and priority to dimension plus source
  kind/id rather than one anonymous aggregate set.

The first implementation may still reduce source views to an aggregate set
inside one dimension after preserving source ownership. Do not pass a single
realm-wide `BTreeSet<ChunkPos>` into multiple schedulers.

## Player Transfer Contract

Within one realm, a player exists in exactly one dimension at a time. A
dimension transfer is an atomic authoritative operation:

1. validate the destination definition/runtime and safe destination pose;
2. stop routing physical commands to the source dimension;
3. remove source player visibility and player interest;
4. move the same realm player/entity state to the destination;
5. update the realm player record's current dimension and pose;
6. establish destination player interest and send a dimension-change reset;
7. stream destination facts and resume physical commands after teleport/ready
   acknowledgement;
8. retain realm statistics and other realm player state unchanged.

There must never be two authoritative player entities for one realm player as
an implementation shortcut for a warm preview.

Cross-realm activation is different: save/despawn/leave realm A, then
load/join the independent player record in realm B. The same profile UUID does
not imply shared statistics or inventory across those realms.

## Observer Session Contract

An observer subscription is narrower than login as a player:

- authenticate/associate the connection or local consumer as needed for
  access control, but allocate an `ObserverId`, not `ServerPlayerId`;
- select a realm/dimension and bounded interest region;
- receive configuration, world/dimension facts, chunks, block deltas,
  entities, and real players visible in the region;
- do not receive an authoritative local player spawn, player position
  correction, inventory/XP/statistics, or interaction capabilities;
- reject movement, break/place, inventory, teleport-ack, and other
  player-only commands;
- never load, dirty, save, or delete a player record;
- cleanly remove its interest/tickets when closed.

For a same-realm live diorama, one client realm session may eventually own an
active-player stream plus one or more bounded observer subscriptions. Protocol
updates must then identify their dimension/subscription or arrive through
separate logically ordered substreams with explicit cross-stream rules.

Do not force that multiplexing into the first server-data-structure slice. A
local observer adapter can prove server ownership before the wire grows.

## Diorama Migration Rules

There are two distinct products:

1. **Same-realm dimension preview**: retain one realm connection/player;
   observe another dimension without spawning there; activation performs a
   dimension transfer.
2. **Independent/remote realm preview**: observe realm B without loading its
   player record; activation leaves/suspends A and explicitly joins B. Return
   preview performs the inverse operation.

When the current lobby/destination path adopts observers:

- remove the preview-only source-local player body; no such player exists;
- continue rendering actual entities and actual players published by the
  observed dimension;
- preserve current shared-depth terrain/actor placement and bounded-region
  rendering;
- activation may not reveal B until its real player join/transfer has accepted
  a safe pose and destination chunks are ready;
- A-to-B-to-A retains one physical player authority throughout transitions;
- current complete-runtime behavior stays behind an explicit compatibility
  path until native and browser observer activation both pass.

This is a gameplay/session ownership correction. It must reuse the existing
mono/per-eye/multiview renderers; no new preview-only rendering fork is needed.

## Implementation Slices

Only one slice is active at a time. Each slice must leave the ordinary
one-realm/one-Overworld path green and fast.

### Slice 0: Contracts and executable current-state locks

Status: architecture recorded; executable locks not started.

- Add focused source/behavior tests proving the current one-dimension keys,
  hard-coded Overworld player record, and full-player standby behavior before
  refactoring.
- Decide exact shared homes for `RealmId`, `DimensionKey`, qualified addresses,
  and dimension definitions without adding app/platform dependencies.
- Record v1 SQLite/IndexedDB migration fixtures.

Exit: every implicit singleton that must change is enumerated and guarded.

### Slice 1: Unified realm server core and ordinary local session

This is the first structural implementation slice after Slice 0's evidence
locks, before dimension, observer, or statistics features.

- Rename or extract the shared `IntegratedServer` authority as `RealmServer`.
- Make native integrated, browser/Web Worker integrated, dedicated TCP,
  dedicated WebSocket, and deterministic hosts instantiate the same core.
- Remove the structurally built-in local player, `ServerPlayerId::LOCAL`,
  `CommandTarget::Local`, `disable_local_player()`, and parallel local-player
  state from server policy.
- Join the owning local profile through an ordinary session/player registry
  path over an in-memory ordered adapter.
- Preserve logical configuration/play commands, updates, disconnect cleanup,
  player restore/save, and safe spawn behavior across every adapter; only byte
  framing may be bypassed locally.
- Express pause, owner trust/permissions, storage root, and lifecycle as host
  policy/adapters rather than alternative gameplay mechanics.
- Add normalized local/TCP/WebSocket session-trace conformance coverage and
  prove the one-world direct path has no gameplay or pixel change.

Exit: integrated and dedicated modes differ only at explicit host boundaries;
there is no server-owned special local player or local-only persistence path.

### Slice 2: Neutral realm/dimension identities

- Add validated namespaced `DimensionKey` and stable `RealmId` contracts.
- Add `DimensionDefinition`/record vocabulary with an Overworld default.
- Wrap existing single-world construction as one realm with one dimension.
- Keep hot scheduler internals dimension-scoped rather than mechanically
  qualifying every `ChunkPos`.
- Ensure `WorldInstanceId` cannot enter persistence/server identity APIs.

Exit: no gameplay or pixels change; existing worlds resolve exactly one
`minecraft:overworld` definition.

### Slice 3: Dimension-qualified persistence

- Add realm metadata/dimension record versions and explicit v1 migration.
- Qualify native SQLite and browser IndexedDB chunk/entity keys by dimension.
- Keep player records realm-scoped and accept a validated current dimension.
- Add scoped store handles and cross-dimension collision tests using identical
  chunk/entity ids.

Exit: two dimensions can persist `(0, 0)` independently; old worlds reopen as
one Overworld realm on native and web.

### Slice 4: Multi-dimension authoritative runtime

- Extract one `DimensionRuntime` owner from the shared `RealmServer`.
- Add a realm host map and tick each loaded dimension once per host boundary.
- Route players and commands through current dimension membership.
- Prove two real players in different dimensions can move, stream, mutate,
  tick, autosave, disconnect, and resume without state leakage.
- Keep one-process ownership and bounded dimension load/unload lifecycle.

Exit: concurrent players can occupy at least two generated dimensions under
one realm and one server host.

### Slice 5: Source-owned dimension interest

- Replace the anonymous aggregate player set with per-source interest inside
  each dimension.
- Preserve player routing while adding non-player `ObserverId` sources.
- Separate client-visible residency from optional block/entity simulation
  interest.
- Add overlap/removal, dimension isolation, saturation, cancellation, and idle
  unload tests plus source-attributed diagnostics.

Exit: an observer keeps and receives one bounded region without any player
entity or player persistence activity.

### Slice 6: Player dimension transfer

- Add server-owned transfer policy and protocol/client replica reset.
- Move one player between dimensions without disconnecting or replacing realm
  statistics/state.
- Reuse teleport id/ack and configured-session ordering where applicable.
- Prove other players remain active in both source and destination.

Exit: A-to-B-to-A dimension travel preserves one player record and never
duplicates the player entity.

### Slice 7: Diorama observer adoption

- Replace preview-only full-player startup with observer subscriptions for
  same-realm dimensions first.
- Add explicit independent-realm observer-to-join activation without sharing
  player records.
- Remove preview-only synthetic source-local player presentation.
- Preserve entities, real remote players, live mutation, bounded preview,
  A-to-B-to-A cover/readiness, persistence, mono, stereo, and multiview.
- Land native and production browser paths together behind the same shared
  policy.

Exit: inspection and tests prove no preview-only player exists server-side.

### Slice 8: Realm-scoped statistics canary

- Add a vanilla-shaped typed statistics map with
  `minecraft:custom/minecraft:jump` and an explicitly mclone-namespaced
  successful-block-placement counter.
- Increment only from accepted authoritative gameplay events.
- Persist and publish owner-only values through the existing save lifecycle.
- Prove counters follow the player between dimensions, survive realm restart,
  and remain independent in another realm using the same profile UUID.
- Retire or quarantine the non-vanilla jump-grants-XP proof once the statistics
  proof supersedes it.

Exit: the realm/dimension ownership boundary is visible and restart-tested.

## Validation Matrix

At minimum:

- shared unit tests for identifiers, definitions, scoped addresses, migration,
  interest ownership, overlap, and dimension transfer;
- normalized in-memory/TCP/WebSocket join, play, save, disconnect, reconnect,
  and dimension-transfer trace conformance;
- complete `mclone-server`, `mclone-protocol`, `mclone-client`,
  `mclone-app-runtime`, and `mclone-scene` suites;
- SQLite create/v1-migrate/reopen and IndexedDB create/v1-migrate/reopen;
- dedicated TCP and direct WebSocket tests with players in two dimensions;
- unexpected disconnect during transfer and observer cancellation;
- one observer plus one player overlapping the same chunks, removing either in
  both orders;
- same `(x, z)`, entity id, player-local id, and scheduled-tick coordinates in
  two dimensions without collision;
- native/browser lobby preview and A-to-B-to-A activation;
- first changed preview pixels inspected under `/tmp`, then per-eye and
  full-frame multiview validation under the existing XR guardrail;
- Android, Android XR, web/WASM, format, lint, and source-ownership gates.

Diagnostics must include realm id, dimension key, loaded dimension count,
players per dimension, observers per dimension, ticketed chunks by source
kind, ticking chunks, persistence queue depth by dimension, transfer phase,
and observer bytes/update pressure.

## Non-Goals

- hard-coding exactly Overworld, Nether, and End;
- implementing Nether/End terrain, portals, or planet content in this
  structural tactical;
- spherical voxel planets, orbital simulation, or cross-dimension physics;
- simultaneous physical authority for one player in multiple dimensions;
- sharing statistics or inventory across independent realms merely because
  they use the same profile UUID;
- multi-process realm sharding, dynamic region ownership, leases, failover,
  distributed transactions, or MMO orchestration;
- arbitrary N-world rendering in `mclone-scene`; the existing active plus
  optional preview presentation bound remains valid;
- adding a new preview renderer or bypassing shared mono/stereo/multiview
  paths;
- compression, protocol compatibility negotiation, or movement validation
  except where required to keep existing behavior intact.

## Stop Conditions

Pause before implementation if:

- a proposed identity uses `WorldInstanceId`, a filesystem path, or a process
  id as durable realm/dimension identity;
- v1 world migration cannot be made transactional/recoverable on both SQLite
  and IndexedDB;
- player records or statistics would become dimension-local;
- an observer implementation creates a hidden `ServerPlayerId` or loads a
  player record;
- removing one interest source can evict chunks still owned by another;
- realm-global clocks advance once per loaded dimension;
- native and browser require different realm/dimension policy;
- integrated and dedicated hosts require different player, persistence,
  ticketing, transfer, simulation, or autosave mechanics;
- a local profile must bypass ordinary session/player registration to restore
  or save correctly;
- a preview change introduces per-eye-only rendering or regresses the direct
  one-world fast path without attribution;
- multi-process concerns begin dictating the in-process gameplay model beyond
  stable opaque identities.

## Related

- [`../topics/realm-dimension-runtime.md`](../topics/realm-dimension-runtime.md)
- [`../topics/embedded-worlds.md`](../topics/embedded-worlds.md)
- [`../topics/multiplayer-networking.md`](../topics/multiplayer-networking.md)
- [`../persistence-architecture.md`](../persistence-architecture.md)
- [`174-warm-world-hot-swap.md`](174-warm-world-hot-swap.md)
- [`175-live-hosted-world-diorama.md`](175-live-hosted-world-diorama.md)
- [`179-composable-world-presentation-and-live-preview-actors.md`](179-composable-world-presentation-and-live-preview-actors.md)
- [`180-configurable-lobby-world-destinations.md`](180-configurable-lobby-world-destinations.md)
- [`182-local-profile-and-player-persistence-proof.md`](182-local-profile-and-player-persistence-proof.md)
- [`184-session-configuration-liveness-and-disconnect.md`](184-session-configuration-liveness-and-disconnect.md)
