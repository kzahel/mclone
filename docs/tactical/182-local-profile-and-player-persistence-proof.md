# 182: Local Profile And Player Persistence Proof

Status: core identity/player-persistence proof implemented 2026-07-16.
Slices 1, 2, the player-record portion of 3, the identity-bearing portion of
4, 5, and 6 are complete. World metadata/day-time durability, movement
sequence fields, keepalive/disconnect protocol, and physical-device lifecycle
closeout remain follow-ups rather than being implied complete.

Topic: `multiplayer-networking`

Workstream: shared native Rust session, persistence, client-profile, gameplay,
and UI contracts plus native web/WASM storage adapters. Desktop local and
remote validation first, then browser, flat Android, desktop XR, and Android
XR. Platform apps may provide app-private paths, browser storage calls,
cryptographic randomness, lifecycle events, and presentation targets; they do
not own identity semantics, player-record policy, safe-resume rules, or data-
management behavior.

## Goal

Deliver the smallest end-to-end proof that an unauthenticated local player
identity can join local or dedicated authority, mutate server-owned gameplay
state, leave, and later resume the same durable player:

```text
client-global stable UUID + display name
  -> identity-bearing local/remote join
  -> world-scoped authoritative PlayerRecord
  -> safe saved-position resume
  -> server-owned demo experience mutation
  -> owner-only replicated HUD value
  -> autosave/disconnect/shutdown persistence
  -> same UUID rejoins the same record
```

Before that identity affects sessions, expose shared UI and platform services
that make local state explicit and resettable. A developer or player must be
able to inspect the current UUID, clear only rebuildable caches, reset the
local identity, delete selected or all local user worlds, and deliberately
factory-reset known local data without confusing those operations.

This is the first persistence/session vertical slice after Tactical
[`176`](176-dedicated-autonomous-push-runtime.md). It narrows Follow-Ups A and
B from Tactical [`136`](136-world-catalog-and-crud-ui.md) into an executable
plan and supplies the identity/player-record prerequisite for the session
lifecycle in [`../topics/multiplayer-networking.md`](../topics/multiplayer-networking.md).

## Product Contract

### Identity is local and unauthenticated

- First launch generates one random UUID and stores it as a client-global
  profile, outside every world.
- The profile also carries a display name, schema version, and created time.
- The UUID is stable for that app installation/browser origin until the user
  explicitly resets it or clears all application/site data.
- The client presents the UUID when joining local integrated or remote
  dedicated authority. The server keys a world-scoped player record by UUID.
- This is identity, not authentication. A client that can claim another UUID
  can impersonate it. The UI and documentation must not call this secure login
  or imply Mojang/account verification.
- A later authenticated identity provider may prove or map the same logical
  player id without changing world player-record ownership.
- Flat desktop and desktop XR share the same native profile root. Flat Android
  and Android XR currently have different application ids and therefore
  separate profiles. Browser identity is scoped to browser profile + origin.

### Storage domains stay distinct

| Domain | Examples | Clear cache | Reset identity | Delete all worlds | Factory reset |
| --- | --- | ---: | ---: | ---: | ---: |
| Client profile | UUID, display name, created time | preserve | replace | preserve | delete |
| Client preferences | input, graphics, asset-pack selection | preserve | preserve | preserve | delete |
| Rebuildable cache | explicitly registered derived/downloaded artifacts | delete | preserve | preserve | delete |
| User local worlds | catalog rows, world metadata, chunks, entities, players | preserve | preserve | delete | delete |
| Managed scenario content | versioned first-party lobby/destination data | preserve unless registered as cache | preserve | preserve | delete/reprovision |
| Remote server records | server-owned player/world data | never local | never local | never local | never local |

The implementation must use an explicit storage-domain inventory. “Clear
cache” must not recursively delete a guessed application-data directory. If no
persistent rebuildable cache family exists on a platform, the action is shown
as unavailable or reports that there is nothing to clear.

### User-visible destructive actions

- **Clear Rebuildable Cache** removes only registered cache families. It does
  not change profile, preferences, local worlds, or remote records.
- **Reset Player Identity** creates a new UUID and keeps local worlds. Existing
  local and remote world records under the old UUID remain intact but are no
  longer selected by the new identity. The confirmation says this explicitly.
- **Delete Selected World** keeps the existing catalog behavior: only an
  inactive user world can be deleted after confirmation.
- **Delete All Local Worlds** deletes all inactive user-created/catalog worlds
  after one strong confirmation. It excludes managed scenario worlds and
  leaves profile/preferences/cache intact. It is unavailable while any local
  world is active; quit-to-title/close must complete first.
- **Factory Reset** closes all sessions and storage, deletes the client profile,
  preferences, registered caches, local user worlds, and managed local content,
  then requires/recommends app restart. It never sends a delete request to a
  remote server.
- Every operation reports its backend, progress, exact affected domains, and
  partial failure. Durable deletion must never silently degrade to an in-memory
  UI success.

Call the identity action **Reset Player Identity**, not Log Out. A future
multi-profile chooser may add Switch Profile or Log Out semantics when there
is an account/session concept.

## Platform Storage Map

The shared logical document is provisionally `player-profile.v1.json` on
native platforms and `mclone.playerProfile.v1` on web.

| Platform | Default client-profile storage |
| --- | --- |
| macOS | `~/Library/Application Support/mclone/preferences/player-profile.v1.json` |
| Windows | `%LOCALAPPDATA%/mclone/preferences/player-profile.v1.json` (fall back to `%APPDATA%`) |
| Linux | `$XDG_DATA_HOME/mclone/preferences/player-profile.v1.json`, then `~/.local/share/mclone/preferences/...` |
| Flat Android | internal app-private files for `com.kzahel.mclone`, under `preferences/player-profile.v1.json` |
| Android XR | internal app-private files for `com.kzahel.mclone.xr`, under `preferences/player-profile.v1.json` |
| Web | `localStorage["mclone.playerProfile.v1"]`, scoped to origin/browser profile |

Native roots reuse the existing world-root sibling preference convention from
`mclone-app-runtime::asset_pack_preferences`. `MCLONE_WORLD_ROOT` and test
roots derive a sibling `preferences` directory rather than placing identity in
one world. Android must prefer internal app data for identity even when an
external path is available for optional asset discovery. Browser private mode,
site-data clearing, or an origin change may remove/change identity; that is
expected and must be diagnosable.

Example logical profile document:

```json
{
  "schema": 1,
  "profileId": "92ffc98e-32c0-4be4-839e-e03a27652b92",
  "displayName": "Player",
  "createdAtUnixMs": 1784203200000
}
```

Malformed or unsupported durable profile data is an explicit error with Reset
as a user choice. Do not silently generate a different identity on parse/read
failure, because that makes an existing server player appear lost.

## Authoritative Record Contracts

### World metadata v1

The opened world store, not launch arguments alone, owns at least:

- record/schema version;
- seed;
- generation and behavior profiles;
- target/content compatibility stamps already required by persistence;
- created and last-played times;
- game time/day time; and
- initial/world spawn position once resolved.

Opening a new empty store initializes metadata. Opening an existing store
validates it before chunk generation. A dedicated `--seed` or profile mismatch
must fail clearly rather than silently generate new chunks under different
world facts. Catalog summaries may mirror metadata for listing, but the opened
world metadata is authoritative for simulation.

### Player record v1

Player records remain separate from entity chunks and are keyed by the stable
profile UUID within one world:

```text
PlayerRecordV1
  schema_version
  profile_id
  last_known_display_name
  dimension/world_key        # overworld-only value in this slice
  position: Vec3d
  y_rot_degrees
  x_rot_degrees
  on_ground
  selected_hotbar_slot
  total_experience_points
  last_seen_game_tick
  revision
```

Inventory contents, spawn/bed state, health, hunger, game mode, abilities,
effects, and advancement/statistic state are deliberately left for later
record versions. Fields added later must have explicit defaults/migration; do
not serialize a Rust enum ordinal or in-memory struct image.

Only the authoritative host mutates/saves the record. The client supplies
identity and commands, never saved position or experience. Native SQLite and
browser IndexedDB implement the same request/completion contract. Transient
worlds may use the same in-memory record behavior but promise no restart
durability.

### Save policy

- Accepted position/rotation, selected slot, and experience changes dirty the
  player record without writing per movement packet.
- Dirty player records join periodic autosave, clean disconnect, world close,
  and graceful process/page shutdown policy.
- Native storage work stays on the persistence actor/thread. IndexedDB remains
  completion-driven; the browser worker/render loop must not block.
- Disconnect removes the live player only after its required durable save has
  been queued/acknowledged according to the persistence lifecycle. A timeout
  or IO EOF is still a disconnect even before in-band disconnect packets land.
- Tests may force a flush barrier. Production may debounce frequent position
  updates, but the chosen maximum dirty interval must be explicit and measured.
- Save failures remain visible in server/session diagnostics and must not be
  reported as a successful clean exit.

## Join And Resume Contract

This tactical adds a minimal identity-bearing join, not an authenticated login
system. The exact Rust names may change, but the logical sequence is:

```text
transport version handshake
  -> client profile hello { UUID, display name, capabilities }
  -> server validates profile and reserves UUID
  -> async world metadata/player record load
  -> saved-position validation or spawn search
  -> server join result { disposition, world facts, player state }
  -> teleport-id-gated authoritative position
  -> chunk/entity/remote-player publication
  -> PLAY
```

- Local integrated and remote dedicated sessions consume the same logical
  profile/join contract. The in-process path may avoid byte encoding, but it
  must not retain a hard-wired unrelated local identity.
- Runtime `ServerPlayerId` stays session-local and distinct from the persistent
  UUID.
- A second concurrent connection claiming a UUID already active in that world
  is rejected with an explicit reason in this unauthenticated phase. Do not let
  two live sessions race one player record.
- Display name is presentation/last-seen metadata, not the durable key.
- Resetting local identity causes the next join to create/select another player
  record; it does not delete the old record from any server.
- Protocol/session fields land in one coordinated protocol-version bump. Ride
  a movement sequence number and correction `last_applied_sequence` on that
  bump as required by [`../topics/client-prediction.md`](../topics/client-prediction.md),
  but do not implement input replay or full movement validation here.

Join disposition is explicit in protocol, diagnostics, and tests:

```text
NewSpawn
ResumedSavedPosition
RecoveredNearSavedPosition
RecoveredAtWorldSpawn
```

## Safe Resume Policy

Do not blindly trust coordinates merely because the server previously wrote
them. Position selection waits until the required collision/chunk facts are
available and reuses shared spawn predicates rather than app-local checks.

Order:

1. Reject non-finite coordinates, the wrong dimension/world, positions below
   the recoverable world floor or outside world/build bounds, and a player AABB
   intersecting authoritative collision geometry.
2. If the exact saved pose is valid, resume it exactly. A valid airborne pose
   remains valid; disconnecting while jumping, falling, or flying must not by
   itself relocate the player.
3. If invalid, run a bounded deterministic center-out surface/clearance search
   around the saved X/Z location using the same block/height/clearance rules as
   initial spawning.
4. If no local recovery exists, use the existing deterministic initial world-
   spawn pipeline.
5. Deliver the selected pose through the existing teleport id/ack gate. Do not
   count resume/recovery teleports as jumps or experience events.

Refactor `mclone-server::spawn` only as needed to expose one shared safe-column
and clearance contract for both initial spawn and nearby recovery. Keep chunk
readiness asynchronous. Record the final recovery radius/order and relevant
Java 1.17.1 divergences in this tactical before implementation; do not invent
an unbounded world scan.

## Demo Experience Rule

Experience supplies a visible, easily repeated proof that server gameplay state
round-trips through protocol, client replica/HUD, and player persistence.

- Add a clearly named non-vanilla persistence-demo behavior flag. It is off in
  the vanilla behavior profile and not inferred from a debug client capability.
- While enabled, award one total experience point on one server-recognized,
  accepted transition from grounded to upward airborne movement.
- Packet duplicates, look-only/status noise, teleport correction, initial
  placement, resume, and safe recovery must not award experience.
- Saturate or explicitly handle the integer maximum; never wrap.
- Publish the authoritative total to the owning client. Other players do not
  need another player's experience in this slice.
- Show the total in a small shared HUD/debug row across mono, stereo, and web.
- Name the rule and diagnostics as a demo. Do not accidentally establish
  “jumping grants XP” as vanilla gameplay semantics.

The demo does not require a general XP level/progress curve, orbs, enchanting,
death loss, commands, sounds, or particles.

## Implementation Slices

Only one slice is active at a time. Pixel-producing UI/HUD slices must capture
and inspect the first drawable milestone before broadening platform coverage.

### Slice 0: Reference and storage-domain audit

Status: this document.

- Read the Java 1.17.1 player join/save/respawn sources before translating:
  `PlayerList`, `PlayerDataStorage`, `ServerPlayer`,
  `ServerGamePacketListenerImpl`, `PlayerRespawnLogic`, and the existing
  world-spawn sources listed by the spawn tactical/code.
- Inventory every durable client-global, cache, local-world, managed-content,
  and browser store currently used by each platform.
- Record exact delete/reset ownership and capability gaps; do not implement a
  generic recursive directory wipe.
- Confirm protocol tag/version allocation and current profile/session startup
  call graph before the first code edit.

Exit: this tactical and the tactical index are linked; storage domains have no
unclassified destructive target.

### Slice 1: Shared local-profile contract and platform stores

Status: complete 2026-07-16 (`32746127`).

- Add a shared `LocalPlayerProfile`/`PlayerProfileId` model and a small
  load/store/reset storage trait in `mclone-app-runtime` or a narrower shared
  preferences owner if one emerges during the audit.
- Generate UUIDs with platform-appropriate secure randomness. No MAC address,
  device serial, seed, timestamp-only id, or world id derivation.
- Add atomic native JSON storage beside existing preferences, internal-first
  Android storage, and the versioned web localStorage adapter.
- Add explicit ephemeral/fixture profile stores and profile-id overrides for
  tests, offscreen probes, and automation so they do not mutate a developer's
  real player record.
- Surface backend label, profile id, display name, and load/reset errors through
  a storage-neutral controller.

Validation:

- first load creates one UUID; repeated loads preserve it;
- native atomic replace and malformed/unsupported schema behavior;
- Android flat/XR roots are distinct and internal-first;
- browser reload preserves one origin-scoped profile;
- reset creates a different UUID without touching a fixture world catalog;
- test/offscreen roots do not touch the default profile file.

### Slice 2: Shared Storage & Profile UI and destructive operations

Status: complete 2026-07-16. Clear-cache is truthfully unavailable because no
persistent rebuildable cache family is currently registered.

- Add a shared title/options `Storage & Profile` screen and confirmation/
  progress/error states in `mclone-ui`.
- Show current display name, UUID, profile backend, world backend/count, and
  registered cache families/bytes where cheaply measurable.
- Extend shared catalog/platform-service contracts for delete-all-inactive-user-
  worlds and registered cache clearing. Reuse selected-world deletion.
- Implement Reset Player Identity and Factory Reset through shared policy with
  thin native/web/Android executors.
- Disable identity/world/factory mutations while a session or relevant storage
  handle is active; route through quit/close first rather than racing it.
- Keep managed scenario exclusion and remote-data non-ownership visible in
  confirmation copy.

Validation:

- reducer/hit-test/action tests for every confirmation and disabled state;
- selected and bulk world deletion preserve profile/preferences;
- cache clear preserves profile/world bytes;
- identity reset preserves worlds and reports a different UUID;
- factory reset removes all registered local domains and reprovisions cleanly;
- partial delete/backend failure remains visible and refreshes truthful state;
- inspect desktop/offscreen captures under `/tmp`, then web/mobile and stereo.

This slice is the prerequisite gate: do not make the UUID session-significant
until the user can inspect and reset it.

### Slice 3: World metadata and player-record persistence

Status: player-record portion complete 2026-07-16 (`836530a8`); the broader
world metadata/seed/day-time portion remains open.

- Activate world metadata and player load/save request/completion variants in
  the shared persistence actor.
- Add stable, versioned codecs and SQLite tables/queries without serializing
  in-memory layouts. Use the existing `player_records` placeholder only after
  verifying/migrating its schema deliberately.
- Add browser IndexedDB metadata/player stores behind the same logical contract.
- Initialize/validate seed and generation/behavior profile metadata before
  generation; persist day time.
- Add in-memory/transient behavior and deterministic fake stores for tests.
- Add dirty/revision/debounce, autosave, disconnect, flush, and close handling
  for PlayerRecordV1.

Validation:

- pending-write visibility and revision precedence for player/world records;
- SQLite and IndexedDB record codec round trips;
- seed/profile mismatch refuses open before generation;
- day time survives process/page reload;
- disconnect/close flushes a dirty player record;
- injected storage failures remain visible and do not claim clean shutdown.

### Slice 4: Identity-bearing join across local, TCP, and WebSocket paths

Status: stable identity handshake/join and duplicate-live-identity rejection
complete 2026-07-16 (`836530a8`). Movement sequence/correction echo and the
broader login/configuration lifecycle remain open.

- Add profile hello/join-result logical messages and the coordinated protocol
  bump in `mclone-protocol`/`mclone-net`.
- Pass the same profile through local integrated runner creation, native TCP,
  direct dedicated WebSocket, and the browser remote worker.
- Reserve UUID, load the player record asynchronously, and enter PLAY only
  after authoritative position/player-state selection.
- Separate persistent UUID from runtime player/remote entity ids.
- Reject malformed profile ids/names and duplicate concurrent UUIDs with an
  explicit reason.
- Add movement sequence/correction echo fields without changing movement
  authority in this tactical.

Validation:

- conformance tests cover local, native TCP, and WebSocket join ordering;
- two distinct profiles join one dedicated world independently;
- reconnecting one UUID selects its record;
- resetting the client identity selects a fresh player without deleting the
  old row;
- duplicate concurrent UUID is rejected without disturbing the live session;
- protocol mismatch/profile rejection occur before gameplay publication;
- platform frame threads remain free of IO/decode/wait work.

### Slice 5: Safe saved-position resume

Status: complete 2026-07-16 (`836530a8`). Exact valid airborne/grounded poses
resume; blocked poses use the bounded deterministic safe-surface fallback.

- Extract shared safe-position/clearance predicates from the current server
  spawn path.
- Implement exact resume, bounded nearby recovery, and initial-spawn fallback
  with explicit `JoinDisposition`.
- Wait for authoritative chunk/collision facts under the existing startup
  progress contract.
- Send the selected pose through teleport/ack and mark the final player record
  dirty only through server-owned state.

Validation fixtures:

- valid grounded saved pose resumes exactly;
- valid airborne saved pose resumes exactly;
- non-finite, below-floor, out-of-bounds, and inside-solid poses are rejected;
- invalid pose with a safe nearby column uses local recovery;
- no nearby result falls back to initial world spawn;
- deterministic repeated runs choose the same recovery;
- resume waits for unloaded chunks rather than treating missing facts as air;
- teleport ack completes and the recovered pose survives the next restart.

### Slice 6: Persistence-demo experience and shared HUD proof

Status: complete 2026-07-16 (`c26a8150`).

- Add `total_experience_points` to authoritative/player-replica state and the
  owner-only server update.
- Add the explicit non-vanilla demo behavior flag and one-point accepted-jump
  detector.
- Add a compact shared HUD/debug presentation on desktop, web, mono, and stereo.
- Dirty/save the player record on award and restore/publish it during join.

Validation:

- exactly one point per accepted jump transition;
- packet/status duplicates and teleport/resume paths award zero;
- demo rule off preserves vanilla-profile behavior;
- disconnect/reconnect and full process/page restart preserve the total;
- another UUID starts from its own total;
- inspect desktop/offscreen, web, and synthetic stereo captures under `/tmp`.

### Slice 7: Cross-platform lifecycle and closeout

Status: shared native/web code and desktop offscreen evidence complete;
Android/Quest physical lifecycle evidence and the broader metadata/session
follow-ups remain open.

- Prove the full UI/profile/join/save/resume flow on desktop local and remote,
  browser IndexedDB local and remote WebSocket, flat Android, desktop XR, and
  Android XR where a device is available.
- Verify app pause/stop/pagehide and clean shutdown do not lose acknowledged
  player changes or perform frame-thread storage work.
- Record all unavailable device evidence honestly; shared Android builds/AVD
  and synthetic stereo do not substitute for a claimed Quest persistence run.
- Update multiplayer, protocol, persistence, hosting, platform, and catalog
  docs to current truth. Close or narrow the superseded Follow-Ups A/B in
  Tactical 136.

End-to-end acceptance scenario:

1. Start from a clean test storage root and generate profile A.
2. Create/open a persistent world and record `NewSpawn`.
3. Move to a known safe position and earn three demo XP through three accepted
   jumps.
4. Quit cleanly, restart the client/server, and rejoin as profile A.
5. Observe `ResumedSavedPosition`, the exact accepted pose, selected slot, and
   three XP.
6. Corrupt the stored pose below the world floor; rejoin and observe one of the
   two explicit recovery dispositions plus a persisted safe pose.
7. Reset identity, rejoin as profile B, and observe a new spawn/zero XP while
   profile A's server row remains.
8. Clear cache and prove both profiles/world bytes are unchanged.
9. Delete all local user worlds and prove the local profile remains.
10. Factory reset and prove a clean reprovision creates profile C with no user
    worlds; no remote server records were deleted.

## Validation Commands

Exact targeted test filters may evolve with the landed module names. Minimum
gates for every relevant slice:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-protocol
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-dedicated-server
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:desktop-offscreen:smoke
pnpm native:web:build
pnpm native:web:catalog-smoke
git diff --check
```

Use the repository Android build scripts rather than bare Android cargo builds:

```bash
pnpm native:android:apk
pnpm native:android-xr:apk
```

Add focused automation for the end-to-end profile/player persistence scenario;
do not rely only on unit record round trips.

## Implementation Record

The landed core proof is split into four reviewable commits:

- `32746127 Add durable local player profiles`: shared UUID profile document,
  native atomic file storage, browser localStorage, platform-root wiring, and
  reset/error tests.
- `836530a8 Persist network players by stable identity`: protocol v21 identity
  handshake across integrated/TCP/WebSocket/browser-worker paths, duplicate
  live UUID rejection, versioned player records in memory/SQLite/IndexedDB,
  selected-slot/pose/XP storage, and safe resume.
- `c26a8150 Prove persisted experience end to end`: protocol v22 owner-only
  experience update, explicitly gated non-vanilla accepted-jump demo rule,
  client replica, persistence dirtying, and shared HUD presentation.
- the Storage & Profile closeout commit: shared read-only profile/backend/world
  inventory, strong confirmation screens, native/browser identity reset,
  delete-all catalog worlds, factory reset of registered preferences and
  managed content, truthful unavailable cache clearing, and a networked
  dedicated-restart acceptance test.

Evidence captured during the closeout:

- `/tmp/mclone-storage-profile.png`: visually inspected desktop/offscreen
  Storage & Profile panel with full UUID, backend, local-world count, disabled
  cache action, and title-only destructive controls.
- `/tmp/mclone-storage-factory-confirm.png`: visually inspected explicit
  factory-reset scope and remote-data preservation warning.
- a real TCP dedicated server is restarted against one SQLite world; the same
  UUID resumes the exact saved airborne pose and authoritative experience.
- native UI/app-runtime suites, native scene/client checks, wasm32 Rust check,
  and TypeScript web build/typecheck pass. Existing wasm-only warnings remain
  unrelated to this slice.

## Explicit Non-Goals

- Mojang/Microsoft accounts, passwords, tokens, encryption, online-mode auth,
  identity proof, bans, permissions, or secure impersonation prevention.
- Multiple selectable local profiles in this first slice. The contracts should
  not forbid them, but Reset creates one replacement profile.
- Deleting or resetting a remote server's player record from client UI.
- Full vanilla login/status/configuration protocol compatibility.
- Keepalive/timeouts and in-band graceful disconnect messages; those remain the
  next broader session-lifecycle slice, though EOF/transport disconnect must
  save correctly here.
- Full inventory/container sync, health, hunger, death/respawn, game mode,
  abilities, effects, advancements, statistics, chat, or commands.
- Vanilla XP sources/levels/orbs/enchanting. Jump XP is an explicit demo rule.
- Stage-1 movement anti-teleport validation or stage-2 input replay. Sequence
  fields only preserve that path during the coordinated protocol bump.
- Literal Anvil/player `.dat` compatibility or DataFixer support.
- Unbounded filesystem/site-data deletion, OS-level browser cache clearing, or
  claiming that factory reset deletes data outside registered Mclone stores.

## Guardrails

- Shared profile, data-management, join, player persistence, safe-spawn, and XP
  policy must not land in desktop, Android, XR, or web app glue.
- Client identity is global to the installation/origin; authoritative player
  state is world-scoped and server-owned. Never put position/XP in the client
  profile file.
- Never derive the UUID from device identifiers or world facts.
- Never silently replace an unreadable profile or incompatible durable record.
- Never delete an active world or mutate identity during a live session.
- Never claim local reset removed remote server data.
- Preserve one ordered reliable protocol stream and the ready-only client pump;
  no socket/storage IO or payload decode on drawable frame threads.
- Keep local integrated and remote dedicated behavior behind the same logical
  profile/join/player-record contracts.
- Reuse server spawn/collision facts and teleport ack; do not add app-local safe
  spawn heuristics or treat missing chunks as empty.
- Gate the non-vanilla jump-XP rule explicitly and keep it off in the vanilla
  profile.

## Related

- [`../topics/multiplayer-networking.md`](../topics/multiplayer-networking.md)
- [`../topics/client-prediction.md`](../topics/client-prediction.md)
- [`../topics/vanilla/networking.md`](../topics/vanilla/networking.md)
- [`../persistence-architecture.md`](../persistence-architecture.md)
- [`../protocol.md`](../protocol.md)
- [`../multiplayer-hosting.md`](../multiplayer-hosting.md)
- [`134-shared-persistence-architecture.md`](134-shared-persistence-architecture.md)
- [`136-world-catalog-and-crud-ui.md`](136-world-catalog-and-crud-ui.md)
- [`160-shared-native-world-catalog-executor.md`](160-shared-native-world-catalog-executor.md)
- [`161-native-catalog-storage-dedup-followup.md`](161-native-catalog-storage-dedup-followup.md)
- [`167-shared-session-startup-contract.md`](167-shared-session-startup-contract.md)
- [`176-dedicated-autonomous-push-runtime.md`](176-dedicated-autonomous-push-runtime.md)
