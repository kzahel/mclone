# Tactical 190: Player Health, Lava Death, and Respawn

Status: active 2026-07-17; Slices 0-2 complete

Workstream: native Rust, shared protocol/server/client/UI/persistence first;
native web/WASM and platform adapters through the same contracts

Topics: `realm-dimension-runtime`, `multiplayer-networking`

## Objective

Add the first complete player-survival lifecycle without prematurely building
inventory, hunger, armor, combat, or the full vanilla damage catalog:

```text
server-owned player health
  -> authoritative lava-body intersection
  -> one lethal damage transition
  -> persisted dead player state and death statistic
  -> shared death screen
  -> explicit client respawn request
  -> safe realm-primary-dimension spawn
  -> same session, profile, realm record, and statistics continue
```

Lava contact is intentionally an instant kill in this tactical. It must still
flow through a real health and typed-damage contract so later nonlethal damage
does not require replacing a lava-specific teleport or boolean-only death
shortcut.

This tactical builds on:

- [`182`](182-local-profile-and-player-persistence-proof.md), which supplies
  stable profile identity and durable realm player records;
- [`184`](184-session-configuration-liveness-and-disconnect.md), which owns
  the surrounding session/play-state and disconnect contract; and
- [`185`](185-realm-dimension-and-observer-runtime.md), which supplies one
  shared `RealmServer`, concurrent dimensions, realm-scoped statistics,
  safe-spawn routing, transfer, and observer isolation.

## Agreed Product Contract

### Health and damage

- A living player starts with `20.0` health and `20.0` maximum health.
- Health is authoritative server state and is replicated only as needed for
  presentation. A client may predict effects later but cannot declare damage,
  death, or revival.
- Introduce an extensible typed damage cause. This tactical needs only
  `Lava`; it must not encode causes as user-facing strings.
- Intersecting lava applies lethal damage immediately. The damage pipeline
  clamps health to zero and emits one living-to-dead transition.
- Repeated contact or ticks while already dead must not emit another death,
  increment statistics again, or start a second respawn.

### Death is a player lifecycle, not a session lifecycle

- Death retains the connection, session, claimed profile UUID,
  server player id, realm player record, current realm, and chunk interest.
- The dead client continues receiving the current scene so the world can be
  displayed behind the death screen.
- Dead players have no physical authority. The server rejects movement,
  block break/place, carried-item selection, teleport/dimension-transfer, and
  later equivalent gameplay commands while dead.
- Session maintenance such as keepalive, orderly disconnect, bounded view
  maintenance, and the explicit respawn request remains valid.
- Client-side input suppression is required for good behavior, but is not the
  authority or the security boundary.
- Observers remain non-player interests and cannot acquire health, die,
  respawn, or receive owner-only vital updates.

### Death presentation

- Shared `mclone-ui` state owns a non-dismissible death screen, rather than
  desktop, browser, Android, or XR app code owning separate policy.
- The first screen contains:
  - `You Died!`;
  - a cause derived from the typed lava cause, initially the vanilla-shaped
    `<player> tried to swim in lava` wording;
  - `Respawn`; and
  - the existing quit-to-title path.
- Escape/back cannot silently return to gameplay while the server still
  considers the player dead.
- The ordinary HUD gains a minimal shared health display, preferably ten
  vanilla-style hearts. It intentionally has no armor, hunger, absorption,
  regeneration, or damage animation in this tactical.
- Flat, browser, Android, per-eye XR, and full-frame multiview consume the
  same screen/HUD model and action. Platform adapters only translate input and
  present the already-decided UI.

### Respawn

- Respawn requires an explicit client command. The server ignores or rejects
  it unless the requesting player is dead and prevents duplicate in-flight
  requests.
- Beds and respawn anchors do not exist yet. The destination is the realm's
  primary spawn dimension, initially `minecraft:overworld`, even when death
  occurred elsewhere.
- The destination pose comes from the existing safe surface-spawn search. It
  must have body/head clearance, support, and no intersecting fluid; neither
  the death pose nor an unsafe saved pose is a respawn fallback.
- If the required spawn area is not ready, the player remains dead and the UI
  may show `Respawning...` until the server can commit one authoritative
  result. It must not publish an unsafe provisional pose.
- Successful respawn restores health to `20.0`, clears the pending death
  cause and transient movement/teleport state, and resumes the same session
  and realm player record.
- Same-dimension respawn retains the client dimension replica and loaded
  chunks. Cross-dimension respawn uses the existing dimension boundary before
  publishing the new absolute pose.
- Do not repurpose `DimensionChange { keep_player_state: false }` if doing so
  resets realm statistics or the current placeholder experience/carried
  state. Respawn needs an explicit life-state/reset contract.

### State retained across death

- Realm statistics survive death. A first vanilla-keyed
  `minecraft:custom/minecraft:deaths` counter increments exactly once.
- Stable identity, display name, server player id, and realm membership
  survive.
- Until real inventory and experience-loss/drop rules exist, retain the
  current debug hotbar/carried selection and dormant experience state.
- Do not manufacture item entities, egg collection, inventory drops, or XP
  orbs as substitutes for systems the project does not have.
- A minimal remote-player behavior removes the dead player body from other
  clients and republishes the same player id on successful respawn. Corpse
  state and a death animation are later presentation features.

## Vanilla Minecraft 1.17.1 Reference Shape

Read these sources before implementing the corresponding slice:

- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/Entity.java`
  (`baseTick`, `isInLava`, `lavaHurt`)
- `reference/minecraft-1.17.1/src/net/minecraft/world/entity/LivingEntity.java`
  (health storage, `hurt`, death transition, save/load)
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerPlayer.java`
  (`die`, statistics and player cleanup)
- `reference/minecraft-1.17.1/src/net/minecraft/server/players/PlayerList.java`
  (`respawn`)
- `reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/DeathScreen.java`
- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ServerboundClientCommandPacket.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
  (`handleRespawn`)

Relevant vanilla behavior:

- vanilla players normally have 20 health;
- `lavaHurt` attempts 4 lava damage and sets the entity on fire rather than
  making contact intrinsically lethal;
- the connected client explicitly requests `PERFORM_RESPAWN`;
- the server creates/rebinds the live player entity while retaining the
  connection/profile and realm-level player data;
- a same-dimension client respawn need not replace the loaded client level,
  while a changed dimension does; and
- beds/anchors select a valid personal respawn before the Overworld fallback.

Mclone intentionally diverges only in the first ruleset breadth: lava is
lethal, there is no fire continuation, and only the realm-primary safe spawn
exists. The authority, explicit request, same-session identity, and
same-versus-cross-dimension boundaries stay vanilla-shaped.

## Current Mclone Seams to Reuse

- `ServerPlayerEntry` is the shared authority for a joined player's identity,
  current dimension, pose, selected debug slot, experience, typed statistics,
  and player-record revision. Vitals and live death state belong beside that
  state, not in an app or transport adapter.
- `PlayerRecord` is already realm-scoped and UUID-keyed. Its versioned native
  SQLite and browser IndexedDB codecs are the durable owner for health and a
  pending typed death cause.
- `RealmServer` has one realm tick and independently ticks each
  `DimensionRuntime`; this is the central stationary-hazard evaluation seam.
- Accepted movement already crosses server validation before the authoritative
  pose changes; this is the immediate moving-player hazard seam.
- `mclone-blocks` already exposes player/entity AABB helpers,
  `BlockFluidKind::Lava`, and fluid-height facts for source and flowing lava.
- The existing safe-spawn search rejects fluid spawn space and can be reused
  after a primary-dimension routing decision.
- `mclone-ui` already projects shared screen/action models into flat and XR
  presentation paths. Death policy belongs in that shared model.
- Existing dimension transfer and remote-player removal/publication provide
  boundaries to compose, but their reset semantics must not be overloaded.

Any duplicated player width/height constants encountered in hazard or spawn
code should converge on one shared collision/body contract during this work.

## Authority and Hazard Contract

Use the player's approximately `0.6 x 1.8` body AABB, deflated by a small
epsilon to avoid counting merely adjacent faces. For every intersected block:

1. read the authoritative block state from the owning `DimensionRuntime`;
2. classify its fluid through the shared block/fluid facts;
3. compare the player AABB against the actual fluid surface height; and
4. emit typed lava damage when there is a real overlap.

Do not infer lava from client reports or from missing/unloaded block data.
Hazard evaluation happens:

- immediately after an accepted authoritative pose change, preventing a burst
  of queued movement commands from stepping through lava without observation;
  and
- once at the realm/player simulation boundary, catching a stationary player
  when lava is placed, spreads, or otherwise changes around the body.

Both paths call one idempotent damage/death transition. World mutations may
request the same central check, but must not grow a second lava ruleset.

## Protocol and Replica Contract

Use typed, versioned protocol messages rather than inferring death from
position or a missing remote body. Exact final names may follow existing
protocol conventions, but the logical contract is:

```text
owner update:
  PlayerVitals { health, max_health }
  PlayerDied { cause }
  PlayerRespawned { dimension, pose, health }

client command:
  Respawn
```

The implementation may combine ordered owner messages when that simplifies
atomic application, provided these invariants remain visible in tests:

- zero health and the death cause become one ordered client transition;
- the client cannot leave the death screen before authoritative respawn;
- cross-dimension reset occurs before destination pose publication;
- same-dimension respawn does not clear valid terrain unnecessarily;
- stale death/vitals updates cannot re-kill a newer life epoch; and
- remote removal/republication cannot create duplicate bodies.

Add an explicit life epoch or equivalent ordering guard if existing ordered
session updates are insufficient to reject stale updates around respawn.

## Persistence Contract

Advance the versioned player-record schema rather than adding a sidecar:

```text
PlayerRecord
  existing identity/dimension/pose/experience/statistics fields
  health: finite f32, clamped to [0, max_health]
  pending_death_cause: Option<PlayerDamageCause>
```

- Old records migrate to living `20.0` health with no cause.
- New records reject or normalize non-finite, negative, over-maximum, or
  inconsistent health/cause combinations at the codec boundary.
- Zero health is the durable dead fact. The cause is present only while dead
  so reconnect/restart can reproduce a coherent death screen.
- A live record cannot retain a pending cause. An invalid zero-health record
  without a cause may migrate to a typed generic/unknown cause rather than
  silently granting life.
- Death and successful respawn immediately dirty the realm player record and
  then use the existing autosave, disconnect, and orderly-shutdown flush
  paths.
- Native SQLite and browser IndexedDB must decode the same logical versions
  and yield equivalent normalized records.

Rejoining a record persisted at zero health restores the connected dead state
and screen. It does not auto-respawn, generate a new UUID, or reuse the unsafe
saved death pose as a living spawn.

## Slices and Commit Boundaries

### Slice 0: Lock reference and current-state evidence

- Record the exact vanilla methods used for health, lava damage, death,
  explicit respawn, and same/cross-dimension client handling.
- Inventory current server player, command admission, player AABB, fluid,
  spawn, persistence, client replica, shared UI, and remote-player seams.
- Lock normalized integrated/dedicated/browser session traces before adding
  the lifecycle.
- Update this tactical if live code has moved any ownership boundary.

Commit outcome: documentation and executable contract tests only where useful;
no platform-local feature implementation.

### Slice 1: Shared health, damage, and player-record schema

- Add the shared finite/clamped vitals and typed damage-cause model.
- Extend server player state and realm `PlayerRecord` with health and pending
  cause.
- Version/migrate native SQLite and browser IndexedDB codecs.
- Add legacy, round-trip, malformed-record, independent-realm, and dead-record
  restoration tests.

Commit outcome: durable health exists and defaults to 20, but no environmental
source kills the player yet.

### Slice 2: Server-authoritative lava death

- Centralize the player body AABB contract.
- Add authoritative fluid-surface intersection for lava.
- Evaluate after accepted movement and at the realm player-tick boundary.
- Apply lethal typed damage through one idempotent transition.
- Increment `minecraft:custom/minecraft:deaths` exactly once, dirty the player
  record, cancel incompatible transient movement/transfer state, and remove
  the remote body.
- Gate all physical commands while dead and add adversarial forged-command
  tests.

Commit outcome: lava contact produces a durable dead server player without
disconnecting the session.

### Slice 3: Ordered owner replica and shared health HUD

- Add ordered owner-only vitals/death replication and stale-life protection.
- Apply it in the shared client replica.
- Add a minimal shared health HUD and its flat/per-eye/multiview projection.
- Prove observers and unrelated players never receive owner-only vitals.

Commit outcome: living health and the server death transition are visible on
every shared presentation path.

### Slice 4: Shared death screen and action routing

- Add shared death-screen state and cause-to-copy presentation.
- Add Respawn and quit-to-title actions; block escape/back and normal gameplay
  input while dead.
- Route the same action through scene/session dispatch for integrated,
  dedicated TCP, browser Worker/IndexedDB, and WebSocket paths.
- Keep the current world presentation and chunk interest alive behind the UI.

Commit outcome: the player can explicitly request respawn from one shared
death UI, but the server may still reject the command until Slice 5.

### Slice 5: Safe same- and cross-dimension respawn

- Admit exactly one respawn request only for a dead player.
- Resolve the realm-primary dimension and load/retain the spawn search area.
- Reuse safe non-fluid surface spawn; wait without publishing a provisional
  pose when it is not ready.
- Restore 20 health, clear the death cause and transient state, dirty the
  player record, and republish the same remote player id.
- Preserve statistics, placeholder experience, and debug carried selection.
- Keep same-dimension chunks; compose the existing dimension boundary only
  when the destination differs.
- Add duplicate-request, stale-update, duplicate-remote, and failure/retry
  tests.

Commit outcome: lava death has a complete same-session explicit respawn loop.

### Slice 6: End-to-end persistence, transport, and platform closeout

- Prove death/reconnect, process restart while dead, respawn/restart, and death
  statistic durability in native SQLite and browser IndexedDB.
- Compare normalized in-memory, TCP, native WebSocket, browser local Worker,
  and remote browser traces.
- Validate source/flowing lava, moving/stationary contact, and lava mutation
  through deterministic test content rather than changing ordinary worldgen.
- Capture and inspect flat native and browser living/death/respawn frames.
- Compile/run the established Android, desktop-XR synthetic stereo, and Quest
  multiview gates in accordance with `docs/platforms.md`; record unavailable
  physical-hardware validation honestly.
- Update this tactical, its index entry, and the relevant topic docs with
  landed status, evidence, deviations, and any narrow remaining follow-ups.

Commit outcome: the tactical is complete, documented, and leaves no silent
platform or transport exception.

## Required Tests and Acceptance Evidence

### Health and hazard

- New and migrated players load at exactly 20/20.
- Non-finite/malformed persisted health cannot enter simulation.
- A player AABB overlapping source lava dies.
- A player AABB overlapping each represented flowing-lava level dies.
- A body merely adjacent to lava does not die.
- Entering lava through accepted movement dies before a later movement command
  can escape it.
- Lava placed or spread into a stationary player kills on the server tick.
- Missing/unloaded block facts do not invent a death.
- Repeated contact produces one death event and one statistic increment.

### Dead authority

- Forged movement, break, place, carried-state, teleport, and transfer commands
  are rejected while dead without mutating world or player state.
- Keepalive, disconnect, bounded view maintenance, and one respawn request
  remain valid.
- A dead player remains connected and continues receiving allowed scene data.
- Observers never gain player vitals or a respawn path.

### Respawn and realm topology

- A same-dimension death respawns safely without clearing retained chunks.
- A death in another dimension respawns in the realm-primary Overworld through
  one correct dimension boundary.
- Respawn never selects lava, water, unsupported space, or obstructed body/head
  space.
- Duplicate requests cannot create duplicate players or remote bodies.
- Server player id, profile UUID, realm record, statistics, debug carried
  selection, and placeholder experience remain stable.
- Health returns to 20 and the pending cause is cleared.
- Two realms using the same profile UUID retain independent health/death
  records and statistics.

### Persistence and presentation

- Disconnect/rejoin and full restart while dead restore the death screen and
  cause without granting physical authority.
- Respawned state survives autosave, disconnect, and restart.
- Native SQLite and browser IndexedDB produce equivalent logical records.
- The health HUD and death screen render from the shared UI model on flat,
  browser, per-eye, and full-frame multiview paths.
- Cause text derives from typed state; protocol and persistence do not store
  localized display strings.
- Quit to title performs the normal orderly session/persistence close rather
  than implicitly respawning.

## Explicit Deferrals

- gradual lava damage, burning, fire ticks, fire resistance, and extinguishing;
- damage cooldowns, invulnerability frames, hurt animation, knockback, sounds,
  and particles;
- fall, void, drowning, suffocation, freezing, mob, projectile, PvP, command,
  and starvation damage;
- hunger, saturation, natural regeneration, armor, absorption, status effects,
  and difficulty scaling;
- inventory UI, item/egg collection, item drops, XP orbs/loss, and
  `keepInventory`;
- beds, respawn anchors, personal spawn positions, obstructed-spawn messaging,
  and dimension coordinate scaling;
- corpses, death animation, combat tracker/history, chat death messages,
  scoreboards, spectator-after-death, hardcore, and game-over deletion; and
- client damage prediction or reconciliation beyond ordered authoritative
  vitals/life transitions.

These are separate vertical slices. This tactical must leave typed extension
points for them, but must not implement placeholder versions merely to make
the first lava/respawn loop appear broader.

## Completion Gate

This tactical is complete only when a player can, through the same shared
realm/session contracts on local and hosted paths:

1. enter authoritative lava and die exactly once;
2. see zero health and the shared lava death screen;
3. remain connected but unable to mutate the physical world;
4. explicitly request respawn;
5. safely respawn in the realm-primary Overworld with 20 health;
6. retain identity and realm statistics, including exactly one death; and
7. disconnect/restart before or after respawn and recover the same durable
   lifecycle state on native SQLite and browser IndexedDB.

Platform adapters must remain thin. Any missing physical XR run may be
recorded as unavailable only after shared stereo/multiview validation passes;
it cannot justify a separate XR health, death, UI, or respawn implementation.
