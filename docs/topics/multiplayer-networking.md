# Multiplayer Networking Topic

Topic: multiplayer-networking

Status: first production push milestone complete 2026-07-13; shared client
pose-publication follow-up complete 2026-07-14. Tactical
[`176`](../tactical/176-dedicated-autonomous-push-runtime.md) delivered
autonomous dedicated ticking, native TCP/direct WebSocket push,
native/browser consumer convergence, bounded pressure, conformance evidence,
and compatibility cleanup. Session lifecycle and persistence metadata are the
next priorities.

Scope: the client/server wire protocol, transports, session lifecycle, server
tick/publication cadence, and the dependency ordering for making mclone
multiplayer behave like vanilla Minecraft (extended with configurable tick
rates). Movement authority and prediction have their own topic:
[`client-prediction.md`](client-prediction.md). Vanilla receipts live in
[`vanilla/networking.md`](vanilla/networking.md);
the client-replica topology argument lives in
[`../minecraft-client-replica-research.md`](../minecraft-client-replica-research.md).

## Current state (verified 2026-07-14)

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
offscreen-diagnostic reconciles remain explicit scene-owned operations. The
wire stays at protocol version 20 because `Rot` and remote rotation fanout
already existed.

This cadence controls when a pose is selected and enqueued; it does not define
what XR pose means. Current XR behavior still publishes the existing combined
player camera/body yaw and pitch, while locomotion may reference headset yaw
or player yaw. A future independent head pose should extend the shared pose
and protocol model rather than restore XR-specific publication scheduling.

The autonomous wire now has the intended first production shape, but session
and durability work remain:

- **Protocol**: hand-rolled, validated, little-endian binary codec, strict
  `PROTOCOL_VERSION = 20` equality check
  (`native/crates/mclone-protocol/src/lib.rs:12`). ~9 `ClientCommand` and ~12
  `ServerUpdate` variants covering chunk view/snapshots/unloads, section block
  deltas, vanilla-shaped move/teleport-ack, remote players, entities, time,
  and obfuscated-seed world info. No serde; every decode validates and rejects
  trailing bytes. No compression, no varints.
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

1. **No session layer.** Connection = anonymous player slot; no identity, no
   join phase beyond a version handshake, no keepalive/timeouts (dead peers
   only detected by IO errors), no rejoin-as-same-player; reconnect wipes the
   whole client replica and re-syncs the view
   (`mclone-app-runtime/src/host_mode.rs:304-338`).
2. **Persistence gaps that block real multiplayer**: player state (position,
   rotation, inventory) is not persisted at all — `LoadPlayer` is hard-wired
   "reserved" (`mclone-server/src/persistence.rs:1069-1075`, no `SavePlayer`
   variant); the **seed is not stored in the world save** (dedicated server
   takes `--seed` per launch; a mismatch silently forks generation); and
   `day_time` resets to 1000 every process start
   (`mclone-server/src/integrated.rs:146,331`).
3. **Debug surface baked into the protocol**: `ShootDebugPhysicsCube`,
   `SetDebugHotbarSlot`, `PlayerActionKind::DebugInstantBreak`,
   `EntityKind::DebugCube`, `debug_passive_showcase` defaulting on
   (`mclone-protocol/src/lib.rs:57-67`, `mclone-server/src/runner.rs:647`).

## Target architecture

Vanilla's shape, adapted to our runtime (receipts in the reference doc):

- **One ordered, reliable stream per client** (TCP native, WebSocket web —
  a faithful TCP analog with message framing). Keep strict ordering as a
  protocol invariant; vanilla leans on it everywhere (login sequencing,
  teleport acks, chunk-then-delta coherence). No UDP/QUIC/WebTransport until
  a measured need exists.
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
- **Threading stays std-threads + blocking IO.** Vanilla is Netty IO threads
  + one game thread; our equivalent (accept thread, reader/writer threads per
  connection, one authoritative server thread, mpsc handoff) is already the
  house style and is fine at our player counts. No async runtime unless the
  connection count ever demands it. On web, the browser's event-driven
  WebSocket already matches; browser singleplayer keeps the worker-owned
  integrated server.
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
- **Persistence is a parallel track, not a blocker** for the wire work, but
  two pieces should land before the session layer (phase 3) makes identity
  real: seed + world metadata in the save (correctness bug today), and
  player records keyed by a durable id (`player_records` table already
  exists as a placeholder). `day_time` durability rides along.
- **Documentation cleanup is complete.** `protocol.md` records version 20 and
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
2. **Session lifecycle.** Login phase carrying player profile (name, stable
   UUID) and capabilities; join sequence ordered like vanilla
   `placeNewPlayer`; keepalive/timeout (15 s / 30 s to start); explicit
   disconnect messages; rejoin-as-same-player keyed on profile id (needs the
   persistence track's player records for position/inventory restore).
3. **Movement validation (basic anti-teleport).** Vanilla's checks server-
   side: packet-burst clamp, moved-too-quickly, collision replay +
   moved-wrongly, floating kick, with the local-integrated owner exempted
   like vanilla's singleplayer owner. Detailed in
   [`client-prediction.md`](client-prediction.md).
4. **Variable tick over the wire.** Carry gameplay/publication rates in the
   join handshake; make client-side tick-denominated behavior (move
   reminder, interpolation windows, day-time conversion) rate-aware; add the
   cadence knob to the dedicated server CLI/config. See the next section for
   the semantics decision this forces.
5. **Robustness/perf tail.** Threshold-based frame compression (vanilla:
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

- Player identity source on each platform (generated UUID in config/world
  catalog? per-install?) and whether the local integrated player shares the
  identity path (today `ServerPlayerId::LOCAL` is structurally special —
  `mclone-server/src/integrated.rs:337-339` — and local play is hard-wired
  single-player).
- Whether the in-process runner channel should carry the full session
  handshake too (uniformity, cheap testing of login flow) or remain a
  trusted fast path.
- Compression codec choice and threshold once frames are measured
  post-push-wire.
- Whether later measured entity transform traffic warrants a separate
  superseding/coalescing lane. The current reliable stream keeps exact
  64-frame / 64 MiB per-peer bounds and disconnects a slow consumer rather
  than dropping or reordering gameplay updates; any future lane must define
  spawn/despawn and correction ordering first.

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
  116 (cadence), 176 (autonomous dedicated push milestone)
- Vanilla receipts: [`vanilla/networking.md`](vanilla/networking.md)
