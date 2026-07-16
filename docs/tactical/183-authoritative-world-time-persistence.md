# 183: Authoritative World Time Persistence

Status: active 2026-07-16

Topic: `multiplayer-networking`

Workstream: shared native Rust server/persistence/protocol/client behavior plus
the native web/WASM IndexedDB adapter. Desktop and dedicated-server validation
first, then browser build and storage-contract evidence. Platform apps may
provide paths, IndexedDB transactions, and lifecycle notifications; they do
not own world-time policy.

## Goal

Make world time an authoritative durable world fact rather than process-local
server state:

```text
open world store
  -> load or initialize typed world metadata
  -> validate seed and world profiles before generation
  -> restore game time and day time
  -> advance the two clocks with vanilla semantics
  -> periodically replicate authoritative time
  -> autosave/flush/close one coalesced metadata revision
  -> restart at the saved clocks
```

This completes the world-metadata/day-time portion left open by Tactical
[`182`](182-local-profile-and-player-persistence-proof.md). It also fixes the
existing seed-authority hole: launch arguments may initialize a new store, but
must not silently reinterpret an existing persistent world's generated chunks.

## Current Problem

`IntegratedServer` currently owns `simulation_tick`, `day_time`, and the debug
`day_time_frozen` flag in memory. A fresh server always starts at simulation
tick `0` and day time `1000`, advances day time with gameplay ticks, and sends
only `day_time` on join and every 20 ticks. Persistent SQLite/IndexedDB worlds
store chunks, entity chunks, and players but do not load or save either clock.

Consequences:

- every process/page restart resets the sky and day count;
- the server tick used by scheduled work and spawn cadence restarts at zero;
- a dedicated server can be reopened with a different `--seed` or profile even
  though stored chunks were generated under the old facts;
- the browser client applies a new day-time sample only when a server update
  arrives, instead of advancing a local replica between corrections; and
- the catalog's listing summary is being asked to carry facts that belong to
  the opened authoritative world store.

## Vanilla 1.17.1 Receipts

- `PrimaryLevelData` owns separate `gameTime` and `dayTime` fields and writes
  them as `Time` and `DayTime` in the world-level `level.dat` record:
  `world/level/storage/PrimaryLevelData.java:50-51,177-192,251-252`.
- A new `PrimaryLevelData` initializes both clocks to zero:
  `PrimaryLevelData.java:137-171`.
- `ServerLevel.tickTime()` always advances game time and scheduled events, but
  advances day time only while `doDaylightCycle` is true:
  `server/level/ServerLevel.java:415-427`.
- `MinecraftServer.tickChildren()` sends `ClientboundSetTimePacket` every 20
  ticks, and `PlayerList.sendLevelInfo()` sends it during join:
  `server/MinecraftServer.java:865-875` and
  `server/players/PlayerList.java:668`.
- `ClientboundSetTimePacket` carries both clocks and encodes a stopped daylight
  cycle by negating day time: `network/protocol/game/
  ClientboundSetTimePacket.java:6-31`.
- `ClientLevel.tickTime()` advances its replica between authoritative updates,
  conditionally advancing day time from the decoded daylight rule:
  `client/multiplayer/ClientLevel.java:127-154`.
- Vanilla autosaves players and world data every 6000 server ticks and saves
  again through normal shutdown: `server/MinecraftServer.java:833-840`.

## Product And Ownership Contract

### Two clocks, one authority

- `game_time` is the durable count of authoritative gameplay ticks. It
  advances on every gameplay tick and supplies scheduled-tick/spawn/timer
  cadence.
- `day_time` is the durable celestial clock and day count. It advances only
  while the durable daylight-cycle rule is enabled.
- The existing `simulation_tick` field is promoted to `game_time` semantics.
  If process-local tick diagnostics are needed, they use a separately named
  non-durable counter rather than a third ambiguous world clock.
- The server is the only authority. Clients predict clock passage only between
  server corrections and never write world time.

### Debug overrides stay outside the save

`--day-time` and `--freeze-time` remain capture/diagnostic overrides. They may
replace the in-memory starting sample or stop effective advancement for the
current run, but they do not rewrite the durable daylight-cycle rule. Normal
world opens restore persisted clocks before applying an explicit debug
override.

### Catalog summaries are not authority

Native `world.json` and browser catalog rows remain list/create/open metadata.
They may mirror display-oriented facts later, but the opened `WorldStore`
record is authoritative for seed, generation/behavior profiles, and clocks.
World generation and player joins cannot begin before that record is loaded or
initialized successfully.

## World Metadata V1

Add a typed singleton record to the shared `mclone-server` persistence
contract:

```text
WorldMetadataV1
  codec_version
  revision
  target_minecraft_version     # "1.17.1"
  seed
  world_generation_profile
  world_behavior_profile
  created_unix_millis
  last_played_unix_millis
  game_time
  day_time
  do_daylight_cycle
```

The binary codec is explicit and versioned. It does not serialize Rust enum
ordinals or an in-memory struct image. Unknown versions, invalid profile tags,
trailing bytes, and incompatible target versions are errors.

`WorldStoreRequest`/`WorldStoreCompletion` gain typed load/save variants.
Metadata writes are durable and revision-coalesced with pending-write
visibility, the same completion-driven rule used by other record families.
Memory, null/transient, native SQLite, and the browser external-load bridge
implement the same logical behavior.

Native SQLite stores the encoded singleton in its existing world database.
Browser IndexedDB gains a world-metadata object store keyed by `worldId`; Rust
owns the bytes and TypeScript owns only the transaction/transport mechanics.

## Initialization And Migration

Opening has three explicit outcomes:

1. **New empty persistent store:** create metadata from requested seed/profiles,
   initialize `game_time = 0`, `day_time = 0`, and
   `do_daylight_cycle = true`, then durably save it before generation.
2. **Existing metadata:** decode and validate target, seed, generation profile,
   and behavior profile before any chunk is scheduled. A mismatch refuses the
   open with both stored and requested facts in the error.
3. **Legacy mclone store with records but no metadata:** initialize one
   migration record from the launch/catalog seed and profiles. Historical time
   cannot be reconstructed, so preserve the old mclone start convention
   (`game_time = 0`, `day_time = 1000`) exactly once and save it immediately.

The implementation must distinguish a genuinely empty store from a legacy
store. Native SQLite can query whether chunk/entity/player rows exist; the web
adapter already knows whether it loaded any records. Migration is never silent
about a conflicting seed because no old authoritative seed exists to compare;
the resulting initialized record becomes authoritative for every later open.

Transient stores initialize the same in-memory metadata semantics but promise
no restart durability. Existing screenshot defaults may continue to request an
explicit time; normal newly created persistent worlds use vanilla's zero clock.

## Save And Close Policy

- A gameplay tick dirties in-memory metadata without writing every tick.
- Explicit time/rule changes dirty it immediately.
- The 6000-gameplay-tick autosave queues one latest metadata revision alongside
  dirty chunks and player records.
- lifecycle flush, clean disconnect/world close, and graceful server/page
  shutdown queue metadata before the persistence barrier.
- Dirty metadata uses latest-revision-wins coalescing. A flush completion means
  all earlier durable metadata/chunk/entity/player writes are acknowledged.
- Save or decode failures remain visible and prevent a claimed clean open or
  close.

Like vanilla, an ungraceful process kill may lose clock progress since the last
autosave. Ordinary restart and acknowledged lifecycle flush must resume exactly.

## Replication Contract

Replace the current day-only update with:

```text
TimeUpdate
  game_time: u64
  day_time: u64
  daylight_cycle_running: bool
```

Use an explicit boolean rather than vanilla's negative-day-time wire encoding;
the semantics remain identical and the custom protocol stays type-safe. Send
the update during join, on tick 1, every 20 gameplay ticks, and immediately
after an explicit time/rule change.

The client replica stores both clocks. At the normal 20 Hz gameplay tick it
increments `game_time` and conditionally increments `day_time` between server
samples. Server updates replace both samples. Rendering continues to derive
celestial phase from replica `day_time`; presentation interpolation may add a
fractional tick without changing stored or protocol values.

This slice retains the current 20 Hz gameplay assumption. Tactical
[`176`](176-dedicated-autonomous-push-runtime.md) and the multiplayer topic
already track carrying variable gameplay/publication rates through the join
configuration before non-20-Hz gameplay becomes a product mode.

## Implementation Slices

### Slice 1: Typed persistence contract and native backends

- Add `WorldMetadata`, codec, record key, requests, completions, mailbox
  helpers, pending-write coalescing, and blocking test helpers.
- Implement memory/null and SQLite singleton load/save plus legacy-record
  detection.
- Add codec corruption/version/trailing-byte tests, actor visibility and
  revision tests, and SQLite reopen tests.

Exit: native persistence can round-trip one authoritative world record without
server lifecycle or protocol changes.

### Slice 2: Server restore, validation, autosave, and restart

- Add world-open metadata initialization/validation before scheduling.
- Promote simulation tick to durable game-time semantics and retain separate
  day-time/daylight-cycle state.
- Include latest metadata in native runner flush/shutdown and dedicated
  autosave.
- Prove exact SQLite restart, frozen-day behavior, and seed/profile rejection.

Exit: a native/dedicated persistent world resumes both clocks and cannot be
reinterpreted with incompatible launch facts.

### Slice 3: Vanilla-shaped time replication

- Extend `TimeUpdate`, bump the coordinated protocol version, and update strict
  codec tests/docs.
- Send both clocks plus running state on join and every 20 ticks.
- Add client-replica clock advancement between corrections without moving
  authority or timing policy into platform apps.

Exit: local/TCP/WebSocket clients observe the same two-clock stream and no
longer hold a one-second-stepped celestial clock.

### Slice 4: Browser IndexedDB and end-to-end closeout

- Add a `worldMetadata` IndexedDB store and migration-safe database bump.
- Route initial metadata bytes, external loads, dirty writes, autosave, and
  lifecycle flush through the existing worker bridge.
- Add browser contract/typecheck/build evidence and update Tactical 182 plus
  persistence/protocol/networking/hosting living docs.

Exit: native SQLite and browser IndexedDB implement the same metadata contract,
and all available automated platform seams are green.

## Required Evidence

- fresh persistent world starts with the chosen v1 clocks and writes metadata;
- after at least 25 ticks, flush/restart restores exact `game_time` and
  `day_time`, then both advance from the restored values;
- with daylight stopped, game time advances while day time remains fixed across
  save/restart;
- explicit debug freeze does not persist as the world rule;
- existing metadata plus a mismatched seed, generation profile, behavior
  profile, target, or codec version refuses open before chunk scheduling;
- a legacy store initializes exactly one metadata record and subsequent opens
  validate it;
- time updates carry both clocks and running state at join/tick-1/20-tick
  cadence;
- client replica advances between samples and corrects authoritatively;
- native TCP restart acceptance observes restored clocks after a real process
  boundary;
- SQLite and IndexedDB record bytes are Rust-codec compatible;
- native app/runtime/server/protocol suites, wasm check, web typecheck/build,
  formatting, and `git diff --check` pass.

## Explicit Non-Goals

- wall-clock catch-up while a world is closed;
- sleeping, `/time` command UI, gamerule UI, moon phases, weather, seasons, or
  calendar time;
- authenticated identity or remaining session-lifecycle packets;
- inventory persistence;
- cross-dimension clocks beyond the current overworld-only server;
- transactionally coupling every chunk row and metadata row into one physical
  database transaction; the ordered durable flush barrier is the required
  first contract;
- changing variable-tick-rate policy in this tactical.

## Validation Commands

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-protocol
cargo test --manifest-path native/Cargo.toml -p mclone-client
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-dedicated-server
cargo check --manifest-path native/Cargo.toml -p mclone-native-client \
  -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml \
  --target wasm32-unknown-unknown -p mclone-web-client
pnpm native:web:typecheck
git diff --check
```

Rendered-output validation is not required unless this slice changes celestial
render math or presentation. If pixels change, capture and inspect frozen dawn,
noon, dusk, and midnight images under `/tmp` before continuing.

## Related

- [`182-local-profile-and-player-persistence-proof.md`](182-local-profile-and-player-persistence-proof.md)
- [`176-dedicated-autonomous-push-runtime.md`](176-dedicated-autonomous-push-runtime.md)
- [`134-shared-persistence-architecture.md`](134-shared-persistence-architecture.md)
- [`036-native-sky-and-day-night-cycle.md`](036-native-sky-and-day-night-cycle.md)
- [`../topics/multiplayer-networking.md`](../topics/multiplayer-networking.md)
- [`../topics/vanilla/networking.md`](../topics/vanilla/networking.md)
- [`../persistence-architecture.md`](../persistence-architecture.md)
- [`../protocol.md`](../protocol.md)
