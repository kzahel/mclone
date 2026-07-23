# Multiplayer Networking Topic

Topic: multiplayer-networking

Status: first production push milestone complete 2026-07-13; shared client
pose-publication follow-up complete 2026-07-14; stable local identity,
realm-scoped player persistence, safe resume, and the persistence-demo XP
proof complete 2026-07-16. Tactical
[`176`](../tactical/176-dedicated-autonomous-push-runtime.md) delivered
autonomous dedicated ticking, native TCP/direct WebSocket push,
native/browser consumer convergence, bounded pressure, conformance evidence,
and compatibility cleanup. Tactical
[`182`](../tactical/182-local-profile-and-player-persistence-proof.md) now
records the first session/persistence vertical slice: inspectable/resettable
client-global UUID identity, explicit local-data controls, durable world/player
records, identity-bearing join, safe resume, and a gated experience proof.
Tactical
[`183`](../tactical/183-authoritative-world-time-persistence.md) completed the
authoritative world-metadata and two-clock persistence/replication follow-up on
2026-07-16. Tactical
[`184`](../tactical/184-session-configuration-liveness-and-disconnect.md)
completed configured play admission, capability negotiation, remote liveness,
typed close, and movement sequence plumbing on 2026-07-16. Tactical 185
Slice 1 then removed the privileged local-player path: integrated, Web Worker,
TCP, and WebSocket hosts now share one `RealmServer` authority and ordinary
player registry, with local/TCP/WebSocket join/command trace coverage.
Tactical
[`190`](../tactical/190-player-health-lava-death-and-respawn.md) completed the
first survival lifecycle on 2026-07-17: ordered owner life state, persistent
lava death, dead-command gating, shared death UI, and explicit safe respawn
now use those same local and hosted paths.

Scope: the client/server wire protocol, transports, session lifecycle, server
tick/publication cadence, and the dependency ordering for making mclone
multiplayer behave like vanilla Minecraft (extended with configurable tick
rates). Movement authority and prediction have their own topic:
[`client-prediction.md`](client-prediction.md). Vanilla receipts live in
[`vanilla/networking.md`](vanilla/networking.md);
the client-replica topology argument lives in
[`../minecraft-client-replica-research.md`](../minecraft-client-replica-research.md).
The unified integrated/dedicated realm server, multi-dimension ownership,
realm-scoped player/statistics persistence, observer interest, and warm
transfer contract live in the focused
[`realm-dimension-runtime.md`](realm-dimension-runtime.md) topic; Tactical
[`185`](../tactical/185-realm-dimension-and-observer-runtime.md) owns the
implementation sequence.

## Current state (verified 2026-07-17)

Tactical 182's core proof is live. Each installation/browser origin owns one
unauthenticated local UUID profile. Integrated, native TCP, direct WebSocket,
and browser-worker joins carry that identity; a dedicated world rejects a
second live connection claiming the same UUID. Realm-scoped player records
persist accepted pose/rotation, selected slot, display name, total experience,
typed statistics, health, and an optional pending death cause in memory,
SQLite, and IndexedDB. Valid saved poses resume exactly, including airborne
poses, while blocked poses reuse the deterministic safe surface search.
Protocol v29 publishes owner-only experience, statistics, and ordered life
state; it accepts an explicit `Respawn` command. Dedicated TCP and SQLite
restart tests prove the same UUID resumes durable pose, XP, statistics, and
dead-or-respawned lifecycle state.

The opened world store now owns typed metadata v1: target version, seed,
generation/behavior profiles, creation/last-played timestamps, revision, both
world clocks, and the daylight-cycle rule. New worlds start both clocks at
zero; legacy mclone stores preserve the prior day-time 1000 convention once.
SQLite and IndexedDB load and validate those facts before generation, reject
later seed/profile reinterpretation, autosave every 6000 game ticks, and flush
on normal lifecycle close. Protocol v29 carries `game_time`, `day_time`, and
daylight running state; the shared client advances its replica at 20 Hz between
join/tick-1/20-tick authoritative corrections.

The shared Storage & Profile title panel exposes the UUID, profile backend,
and local-world count. Reset Identity preserves worlds and old server rows;
Delete All Local Worlds preserves the profile/preferences/managed content;
Factory Reset removes registered local preferences, catalog worlds, and
managed content while preserving remote server data. Clear Cache is disabled
until an actual rebuildable persistent cache family is registered.

Tactical 176 completed 2026-07-13. The dedicated server now runs
one 20/20/60 authoritative cadence independently of command traffic, drains
ordered commands at host boundaries, publishes every player's routed stream
through independent bounded TCP writers, and autosaves every 6000 gameplay
ticks. Worldgen completion and periodic time updates reach idle clients
without polling commands. Native client/server TCP readers and writers are
independent, and normal frame polling accepts unsolicited decoded batches.
Native runtime adapters now expose one ready-only stream with
transport-neutral frame/queue diagnostics and no pending-response state.
Desktop flat/XR and Android flat/XR select the same shared adapter under a
source purity gate. Browser remote now creates a module worker that owns its
WebSocket, handshake, command send, receipt, and full update decode validation;
ordered canonical update buffers cross to the same ready-only runtime pump.
Dedicated WebSocket peers terminate directly in the shared authoritative
connection registry rather than looping back through native TCP.

The production bounds are now explicit and observable. Each dedicated peer
has an independent 64-frame / 64 MiB exact-encoded-byte outbound queue;
saturation disconnects only that peer without waiting on its writer. Native
client ingress is capped at 256 decoded publication batches and reports depth,
bytes, oldest age, receive/drain conservation, read/decode time, sequence, and
overflow disconnects. Browser remote caps worker-unacknowledged batches at 64
MiB, main-thread transferable ingress at 4096 updates / 64 MiB, and socket
command buffering at 8 MiB. The common frame pump reports ordered apply,
deferred/stalled work, producer pressure, and per-frame drain/apply timings.
Sustained browser movement crossed three chunk boundaries with zero residual
queues and a 10.25 ms movement-window frame-gap maximum; native two-client
smokes prove an idle observer receives peer movement. Quest evidence remains
deferred because no device is currently available; shared Android builds, an
AVD frame, and synthetic stereo cover the available platform seams.

Client pose publication now has the same ownership discipline as inbound
push. `McloneSceneHost` owns one wall-clock-slipping, at-most-20-Hz
publication deadline for Mono and XR. Desktop, browser, flat Android, and XR
adapters advance their shared scene entry every presentation frame; they no
longer decide whether movement deserves a command. Stationary mouse look now
reaches `LocalPlayerMoveSync` and emits the existing vanilla-shaped `Rot`
variant, while the 20-publication-attempt position reminder corresponds to
about one second at the default rate. Immediate interaction, teleport, and
offscreen-diagnostic reconciles remain explicit scene-owned operations.

Protocol v29 negotiates optional capability bits during native/WebSocket
handshake and publishes `SessionConfiguration` then `SessionReady` before
ordinary world facts. The shared replica exposes Connecting, Configuring,
Playing, and first-reason-wins Disconnected state to the UI. Dedicated sessions
challenge remote peers every 15 seconds, native TCP retains a 30-second read
timeout, and native/browser I/O actors echo without drawable-frame polling.
Clean quit and server rejection are ordered protocol messages; unexpected EOF
is converted to a visible typed reason. `MovePlayer` carries a client sequence
and corrections echo the latest accepted value. Owner-only `PlayerLife`
updates carry a life epoch, health, and typed death cause; `Respawn` returns
through the same full-duplex command path. Debug actions are rejected unless
their capability was negotiated.

The movement sequence preserves pose-publication ordering and teleport
continuity; it is not a semantic-command acknowledgement or server-replay
contract. Routine player movement is deliberately client-authoritative. The
server rejects non-finite values, clamps coordinates, honors pending
teleports, and otherwise accepts the reported pose without collision, speed,
or floating replay. Integrated single-player therefore does not duplicate the
client's 60 Hz movement simulation.

This cadence controls when a pose is selected and enqueued; it does not define
what XR pose means. Current XR behavior still publishes the existing combined
player camera/body yaw and pitch, while locomotion may reference headset yaw
or player yaw. A future independent head pose should extend the shared pose
and protocol model rather than restore XR-specific publication scheduling.

The autonomous wire and first identity/player-durability proof now have their
intended production shape, but broader session and world durability work
remain:

- **Protocol**: hand-rolled, validated, little-endian binary codec, strict
  `PROTOCOL_VERSION = 29` equality check. The transport handshake now carries
  the local profile UUID/display name plus supported capabilities, and
  `PlayerExperience`, `PlayerStatistics`, and `PlayerLife` are owner-only
  updates alongside chunk view/snapshots/unloads, section block deltas,
  vanilla-shaped move/teleport-ack, remote players, entities, two-clock time,
  and obfuscated-seed world info. `Respawn` is an explicit client command. No
  serde; every decode validates and rejects trailing bytes. No compression,
  no varints.
- **Transports**: in-process mpsc channels (native local integrated,
  `mclone-server/src/runner.rs:830-991`), length-prefixed TCP
  (`mclone-net/src/lib.rs:347-1146`), direct dedicated WebSocket adapter
  (`mclone-dedicated-server/src/websocket_connection.rs`), Web Worker channels
  for browser singleplayer and remote play
  (`mclone-web-client/src/web_server_worker.rs` and
  `www/mclone-remote-websocket-worker.ts`). Real
  cross-machine play works today over TCP (desktop/Android/XR) and WS
  (browser), validated by dedicated-server smokes.
- **Client ingress is in good shape**: shared `ClientConnection` trait +
  ordered per-frame pump with budgets across local/remote/web
  (`mclone-app-runtime/src/client_connection.rs:240-296`), a dedicated client
  IO thread (`mclone-net/src/lib.rs:668-800`), and a shared startup pump
  (tactical 167). Strict receive-order apply is preserved.
- **Server internals are multiplayer-capable**: per-player chunk interest
  tracking (`mclone-server/src/player_chunk_tracking.rs`), remote-player
  visibility fanout (`mclone-server/src/remote_players.rs`), per-player
  entity routing, server-owned inventories.
- **Local integrated runner already has the right tick shape**: a dedicated
  server thread paced by wall clock at 20 Hz with a validated cadence config
  (host/gameplay/physics lanes, default 20/20/60, live-reconfigurable)
  (`mclone-server/src/runner.rs:1143-1258`,
  `mclone-server/src/cadence.rs:9-130`).

## Structural gaps (the reasons this topic exists)

1. **Session lifecycle is still partial beyond basic play admission.** Stable
   identity, duplicate-live-UUID rejection, negotiated optional capabilities,
   configuration/ready phases, keepalive/read timeout, and in-band disconnect
   are live. Authenticated login and reconnect-without-rebuilding-the-client-
   replica are not.
2. **World/player durability is still incomplete.** Authoritative metadata,
   seed/profiles, game/day clocks, the daylight rule, player pose/rotation,
   selected slot, display name, XP, typed statistics, health, and pending death
   cause persist. Inventory contents, personal spawn/bed state, hunger,
   abilities, effects, and advancement state do not yet persist.
3. **Debug vocabulary remains on the wire but is capability-gated**:
   `ShootDebugPhysicsCube`, `SetDebugHotbarSlot`, and debug instant-break are
   rejected unless `DEBUG_ACTIONS` was negotiated. `EntityKind::DebugCube` and
   debug showcase defaults remain development-oriented follow-up surface.

## Target architecture

Vanilla's shape, adapted to our runtime (receipts in the reference doc):

- **One session with explicit reliable and ephemeral classes.** Keep strict
  reliable-stream ordering as a protocol invariant for correctness-critical
  session and gameplay facts; vanilla leans on it everywhere (login
  sequencing, teleport acks, chunk-then-delta coherence). The preferred
  dedicated transport is WebTransport over HTTP/3/QUIC: reliable streams and
  unreliable datagrams share one authenticated, encrypted,
  congestion-controlled session. TCP and WebSocket remain the reliable
  compatibility profile. Ephemeral remote body/head/hand pose semantics and
  cross-lane barriers live in
  [`remote-player-presentation.md`](remote-player-presentation.md).
- **Full-duplex, push-based wire.** Commands flow up and updates flow down
  independently. Server publishes on its own tick cadence: chunk
  snapshots/unloads as interest changes, per-tick section-batched block
  deltas, entity/remote-player updates at tracker cadence, time on a period.
  This replaces the one-batch-per-command shape everywhere (TCP, WS bridge,
  and eventually the in-process runner channel semantics, which already come
  close).
- **Dedicated server adopts the runner's paced host loop.** The wall-clock
  cadence loop in `NativeIntegratedServerRunner` becomes the shared server
  loop; inbound commands are drained at tick boundaries (vanilla drains its
  packet queue on the game thread between ticks); command handling never
  blocks on worldgen or IO. Chunk generation completes asynchronously and
  publishes snapshots when ready.
- **Session lifecycle modeled on vanilla login**: handshake (version +
  capabilities) → login (player identity; later auth) → configuration
  (world info: real view-distance policy, gameplay tick rate, day time) →
  play (join sequence ordered like `placeNewPlayer`: world info, position
  teleport with id, chunk streaming, entity/remote-player adds). Keepalive
  ping/ack (vanilla: 15 s interval, 30 s read timeout) plus explicit
  disconnect messages.
- **Simulation stays on ordinary threads; transport mechanics stay
  contained.** Vanilla is Netty IO threads plus one game thread; our
  authoritative server and bounded channel handoff remain that shape.
  TCP/WebSocket may continue using blocking workers. A future-based
  QUIC/WebTransport library may own a private async runtime inside the
  transport adapter, but async types and scheduling do not spread into
  `mclone-server`, the simulation cadence, or the frame thread. Browser
  WebTransport remains worker-owned; browser singleplayer keeps the
  worker-owned integrated server.
- **The protocol stays transport-neutral logical messages** (the current
  `mclone-protocol` stance), but grows: session/login messages, keepalive,
  disconnect-with-reason, and a capability field so debug variants can be
  gated rather than baked in.

## Shared ownership and frame-thread contract

The push conversion must not create separate desktop, XR, Android, and web
networking policies. Shared owners are:

- `mclone-protocol`: logical commands, updates, codec rules, and ordered-stream
  semantics;
- `mclone-server`: autonomous cadence, command dispatch, global simulation,
  per-player routing, publication, and outbound pressure policy;
- `mclone-net`: transport-neutral framing and native TCP mechanics;
- `mclone-app-runtime`: `ClientConnection`, decoded update envelopes, common
  queue diagnostics, and the budgeted ordered update pump;
- `mclone-client`: authoritative update application to the client replica;
- `mclone-scene`: the frame-owned pump point and handoff into the existing
  terrain dirty/compile/upload lifecycle, plus shared flat/XR player-pose
  publication cadence.

Desktop flat, desktop XR, flat Android, and Android XR must instantiate the
same native remote connection implementation. Platform apps select and wire a
connection; they do not own ordering, drain budgets, command policies,
publication cadence, or backpressure. Web uses different browser mechanics
behind the same logical boundary, not a different engine policy.

The drawable/frame thread may only enqueue a command without waiting, drain
already-decoded ready updates, apply them in receive order under an explicit
budget, and hand dirty state to bounded render workers. It must never read or
write a socket, wait for queue capacity or a command acknowledgement, decode a
wire frame or chunk snapshot, wait for worldgen, run reconnect/connect work, or
build a chunk mesh. Startup may choose an unlimited *apply* budget, but it does
not move socket IO or payload decode onto the frame.

Native remote therefore uses independent blocking reader and writer ownership
around bounded command/update queues. Production web remote must put WebSocket
ownership plus frame decode in a Web Worker; a browser-main-thread callback or
inline integrated server may remain an explicit smoke/fallback path, not the
target production topology. Transferable buffers are an acceptable first
worker handoff; `SharedArrayBuffer` can replace that mechanism later without
changing `ClientConnection` semantics.

One adapter-conformance suite must exercise integrated runner, native TCP, and
web-worker/WebSocket implementations. It owns command/update ordering,
unsolicited delivery, ready-only nonblocking drain, queue saturation,
disconnect/reconnect state, and budgeted deferral without loss or reordering.
XR validation adds frame-accounting evidence rather than a forked semantic
suite because XR consumes the same native adapter.

Deliberate divergences from vanilla, each preserving a parity path:

- Variable tick rates via the existing cadence lanes (below) — vanilla is
  fixed 20 Hz; we carry the rate in the join handshake instead of assuming
  it.
- Wall-clock slip instead of vanilla's burst-catch-up when a tick overruns
  (already the runner's behavior; revisit if gameplay timing parity ever
  needs vanilla's catch-up semantics).
- No encryption/Mojang auth initially; identity is a client-supplied profile
  (name + stable UUID) until an auth story exists.
- Hand-rolled codec instead of registration-order packet ids — keep, but
  packet-id assignment discipline and the version gate serve the same role.

## Dependency ordering (what to do first)

Answering the standing questions — "persistence first? delete scaffolding
first?":

- **Don't do a deletion pass first.** The scaffolding worth removing (tick-
  per-command session loop, `wait_for_server_jobs`, the lockstep IO shape,
  the WS bridge's per-connection TCP loopback) is exactly what phases 1–2
  replace; deleting ahead of the replacement just breaks remote play. The
  only cheap pre-cleanup is quarantining debug protocol variants behind a
  handshake capability (or compile feature) so the protocol surface stops
  accreting debug tags.
- **The first persistence prerequisites are complete.** Player records are
  keyed by the durable local profile UUID. Typed world metadata owns and
  validates the seed/profiles before generation, and durable game/day clocks
  plus the daylight rule ride through SQLite and IndexedDB lifecycle saves.
- **Documentation cleanup is current.** `protocol.md` records version 29 and
  autonomous publication framing; `multiplayer-hosting.md` records persistent
  worlds and direct WebSocket hosting; platform docs record the one shared
  native/browser semantic boundary.

## Phased plan

Order matters; the first coordinated milestone has a focused tactical while
later phases remain topic-level direction.

1. **Dedicated autonomous tick plus server-push transport — complete
   2026-07-13.** Tactical
   [`176`](../tactical/176-dedicated-autonomous-push-runtime.md) treats these
   as one coordinated milestone because autonomous publication without a push
   writer strands updates, while a push-capable transport without an
   autonomous host leaves simulation command-clocked. Its staged commits
   first separate global simulation from per-player drains, prepare duplex
   lanes, then atomically switch the dedicated host to tick-boundary command
   drain and unsolicited publication. It removes `wait_for_server_jobs` and
   per-command saves, publishes snapshots as worldgen completes, bounds
   per-connection queues, converts production web remote to worker-owned
   WebSocket/decode, and proves one cross-adapter contract.
2. **Basic session lifecycle — complete 2026-07-16.** The local profile,
   identity-bearing join, duplicate-live rejection, rejoin-as-same-player
   record selection, configuration-before-ready ordering, optional
   capabilities, 15-second keepalive/30-second TCP timeout, typed disconnect,
   and movement sequence echo are live. Tactical
   [`182`](../tactical/182-local-profile-and-player-persistence-proof.md)
   owns the identity/player-record proof and tactical
   [`184`](../tactical/184-session-configuration-liveness-and-disconnect.md)
   owns configured admission and liveness. Auth, reconnect preservation, and
   broader player-state restore remain later work.
3. **Permissive movement authority — accepted 2026-07-23.** Cooperative
   clients own routine movement; the server validates representation and
   bounds but does not replay collision, speed, floating, or semantic input.
   This is the intended integrated and dedicated posture, not an interim
   anti-cheat milestone. Detailed in
   [`client-prediction.md`](client-prediction.md).
4. **Variable tick over the wire.** Carry gameplay/publication rates in the
   join handshake; make client-side tick-denominated behavior (move
   reminder, interpolation windows, day-time conversion) rate-aware; add the
   cadence knob to the dedicated server CLI/config. The remote-player topic
   separates body/tracked-pose report, server replication, and presentation
   rates instead of treating "publication" as one universal clock. See the
   next section for the gameplay semantics decision this forces.
5. **Mixed-reliability remote pose transport.** After sequenced snapshots,
   buffered interpolation, and loss simulation establish the logical pose
   contract, add one WebTransport/QUIC session for first-party native and
   browser clients. Carry critical facts on reliable streams and ephemeral
   pose on datagrams, with an independently configured 20 Hz reliable
   fallback profile. The implementation and platform-spike order lives in
   [`remote-player-presentation.md`](remote-player-presentation.md).
6. **Robustness/perf tail.** Threshold-based frame compression (vanilla:
   zlib over 256 bytes; we control both ends, so lz4/zstd are candidates —
   must build on wasm), capability-based protocol evolution instead of
   strict version equality, chunk-send pacing/budgets per player, metrics.

## Variable tick rate: what it actually costs

The cadence machinery is ready (`SimulationCadence`, 20/20/60 defaults,
live-reconfigurable, clean-ratio validation; `mclone-server/src/cadence.rs`),
and physics/movement already convert vanilla per-tick constants to wall-clock
units pinned at the 20 Hz baseline
(`mclone-server/src/physics_runtime.rs:18-32`,
`mclone-client/src/player.rs:20,1276`) — so a 60 Hz physics/host rate is
mostly transparent. The catch is the **gameplay lane**: vanilla gameplay
constants are tick-denominated (day length 24000 ticks, item lifetime 6000,
spawn interval 400, scheduled block/fluid tick delays), so raising the
gameplay lane to 60 Hz triples world pace. Decision to make in phase 5:

- (a) keep the gameplay lane at 20 Hz semantics forever and scale only
  host/physics/publication rates (cheapest, fully vanilla-parity), or
- (b) make tick-denominated constants rate-aware (divide-by-rate wall-time
  semantics) so gameplay itself can run at 60 Hz without changing pace.

Either way the protocol needs the rate in the join handshake, and a
**publication/network lane** should be split from the gameplay rate (the
cadence doc already declares they are not the same; no such lane exists in
code yet). Vanilla's per-tick publication batching (section deltas per tick,
entity tracker intervals, time every 20 ticks) then keys off the publication
lane, expressed in time units.

## Open questions

- Player identity source is decided for the first unauthenticated phase: one
  generated client-global UUID per native app installation or browser origin,
  outside world stores; local integrated and remote dedicated join through the
  same logical profile contract. Flat Android and Android XR have separate app
  identities today. Tactical 182 owns storage paths and reset semantics.
  Tactical 185 Slice 1 completed removal of the structurally special local
  player.
- The in-process adapter now carries the same logical join updates, ordinary
  player registration, command path, persistence state, and safe-resume logic
  as remote hosts without encoding wire bytes. Normalized local/TCP/WebSocket
  join and command traces guard that host-boundary choice.
- Compression codec choice and threshold once frames are measured
  post-push-wire.
- Whether entity transforms later join the accepted superseding/coalescing
  datagram lane. The current reliable stream keeps exact 64-frame / 64 MiB
  per-peer bounds and disconnects a slow consumer rather than dropping or
  reordering gameplay updates; entity spawn/despawn and correction barriers
  must be defined before moving transform samples.

## Code and doc map

- Protocol: `native/crates/mclone-protocol/src/lib.rs`
- Transports/framing: `native/crates/mclone-net/src/lib.rs`
- Server loop + cadence: `native/crates/mclone-server/src/runner.rs`,
  `cadence.rs`; dedicated app `native/apps/mclone-dedicated-server/src/`
- Client ingress: `native/crates/mclone-app-runtime/src/client_connection.rs`,
  `native_session_runtime.rs`, `host_mode.rs`
- Persistence: `native/crates/mclone-server/src/persistence.rs`,
  [`../persistence-architecture.md`](../persistence-architecture.md)
- Session/bus status quo: [`../session-network-architecture.md`](../session-network-architecture.md),
  [`../protocol.md`](../protocol.md), and
  [`../multiplayer-hosting.md`](../multiplayer-hosting.md)
- Tacticals: 009 (original wire shape), 133 (bus/pacing; Slice 6 handed to
  176),
  151 (inbound pipeline), 154 (ingress cleanup), 167 (startup contract),
  116 (cadence), 176 (autonomous dedicated push milestone), 182 (local
  profile/player persistence), 184 (session/liveness), and 185
  (realm/dimension/observer topology)
- Vanilla receipts: [`vanilla/networking.md`](vanilla/networking.md)
