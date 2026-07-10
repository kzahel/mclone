# Multiplayer Networking Topic

Topic: multiplayer-networking

Status: planning — research pass completed 2026-07-10; no protocol slices
started under this topic yet.

Scope: the client/server wire protocol, transports, session lifecycle, server
tick/publication cadence, and the dependency ordering for making mclone
multiplayer behave like vanilla Minecraft (extended with configurable tick
rates). Movement authority and prediction have their own topic:
[`client-prediction.md`](client-prediction.md). Vanilla receipts live in
[`vanilla/networking.md`](vanilla/networking.md);
the client-replica topology argument lives in
[`../minecraft-client-replica-research.md`](../minecraft-client-replica-research.md).

## Current state (verified 2026-07-10)

What exists is better than "debug-only", but the wire model is scaffolding:

- **Protocol**: hand-rolled, validated, little-endian binary codec, strict
  `PROTOCOL_VERSION = 19` equality check
  (`native/crates/mclone-protocol/src/lib.rs:12`). ~9 `ClientCommand` and ~12
  `ServerUpdate` variants covering chunk view/snapshots/unloads, section block
  deltas, vanilla-shaped move/teleport-ack, remote players, entities, time,
  and obfuscated-seed world info. No serde; every decode validates and rejects
  trailing bytes. No compression, no varints.
- **Transports**: in-process mpsc channels (native local integrated,
  `mclone-server/src/runner.rs:830-991`), length-prefixed TCP
  (`mclone-net/src/lib.rs:347-1146`), WebSocket codec + bridge
  (`mclone-dedicated-server/src/websocket_bridge.rs`), Web Worker channel for
  browser singleplayer (`mclone-web-client/src/web_server_worker.rs`). Real
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

1. **Lockstep request/response wire.** The remote protocol is strictly one
   update batch per client command; there is no server push. An idle client
   receives nothing, and the client's 20-tick move reminder is the de facto
   poll clock (`mclone-net/src/lib.rs:727-729,765-800`,
   `mclone-app-runtime/src/host_mode.rs:93-99`). Time, entity, and remote
   player updates only arrive as reply batches. Tactical 133 Slice 6 already
   names server-push as open work.
2. **The dedicated server has no autonomous tick.** It blocks on
   `network.recv()` and runs exactly one simulation tick per inbound client
   command (`mclone-dedicated-server/src/main.rs:364-365`,
   `session.rs:77-106`); with zero clients the world freezes, with N chatty
   clients it ticks at the aggregate command rate. Command handling also
   synchronously waits up to 120 s for worldgen jobs and saves dirty chunks
   per command (`session.rs:205-228,88-90`), serializing all clients through
   one loop thread. This violates
   [`../authoritative-host-scheduling.md`](../authoritative-host-scheduling.md)
   (acks fast, snapshots later) and produced the 402-update giant-batch drain
   incident that motivated tactical 151.
3. **No session layer.** Connection = anonymous player slot; no identity, no
   join phase beyond a version handshake, no keepalive/timeouts (dead peers
   only detected by IO errors), no rejoin-as-same-player; reconnect wipes the
   whole client replica and re-syncs the view
   (`mclone-app-runtime/src/host_mode.rs:304-338`).
4. **Persistence gaps that block real multiplayer**: player state (position,
   rotation, inventory) is not persisted at all — `LoadPlayer` is hard-wired
   "reserved" (`mclone-server/src/persistence.rs:1069-1075`, no `SavePlayer`
   variant); the **seed is not stored in the world save** (dedicated server
   takes `--seed` per launch; a mismatch silently forks generation); and
   `day_time` resets to 1000 every process start
   (`mclone-server/src/integrated.rs:146,331`).
5. **Debug surface baked into the protocol**: `ShootDebugPhysicsCube`,
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
- **Docs cleanup is cheap and should ride the first slice**: `protocol.md`
  says version 16 (code is 19) and is missing newer variants;
  `multiplayer-hosting.md` still claims the dedicated server has no
  persistence (it has `--world-dir` + SQLite with a restart test);
  `platform-parity.md:299` copies the same stale claim.

## Phased plan

Each phase is a candidate tactical; order matters, sizes are rough.

1. **Dedicated server autonomous tick.** Move the dedicated loop onto the
   shared paced cadence loop (reuse `runner.rs` machinery); command handling
   becomes enqueue + drain-at-tick-boundary; delete `wait_for_server_jobs`
   and per-command saves (autosave on a tick period instead, vanilla: every
   6000 ticks); `SetChunkView` acks immediately, snapshots publish as
   worldgen completes. The world ticks with zero clients.
2. **Server-push wire.** Replace one-batch-per-command with independent
   command/update streams: per-connection reader + writer threads on TCP;
   WS sessions served natively by the server loop (retire the 1:1 TCP-
   loopback bridge shape); client IO actor splits its lockstep loop into
   send and receive lanes feeding the existing ordered pump. In-process and
   web-worker runners keep their channels but adopt the same push semantics.
   Add per-connection outbound queue bounds with a disconnect policy
   (replacing today's unbounded queues + diagnostics-only stance).
3. **Session lifecycle.** Login phase carrying player profile (name, stable
   UUID) and capabilities; join sequence ordered like vanilla
   `placeNewPlayer`; keepalive/timeout (15 s / 30 s to start); explicit
   disconnect messages; rejoin-as-same-player keyed on profile id (needs the
   persistence track's player records for position/inventory restore).
4. **Movement validation (basic anti-teleport).** Vanilla's checks server-
   side: packet-burst clamp, moved-too-quickly, collision replay +
   moved-wrongly, floating kick, with the local-integrated owner exempted
   like vanilla's singleplayer owner. Detailed in
   [`client-prediction.md`](client-prediction.md).
5. **Variable tick over the wire.** Carry gameplay/publication rates in the
   join handshake; make client-side tick-denominated behavior (move
   reminder, interpolation windows, day-time conversion) rate-aware; add the
   cadence knob to the dedicated server CLI/config. See the next section for
   the semantics decision this forces.
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
- Outbound backpressure policy: disconnect slow consumers (vanilla
  effectively does via TCP + timeouts) vs. drop/coalesce entity updates.

## Code and doc map

- Protocol: `native/crates/mclone-protocol/src/lib.rs`
- Transports/framing: `native/crates/mclone-net/src/lib.rs`
- Server loop + cadence: `native/crates/mclone-server/src/runner.rs`,
  `cadence.rs`; dedicated app `native/apps/mclone-dedicated-server/src/`
- Client ingress: `native/crates/mclone-app-runtime/src/client_connection.rs`,
  `native_session_runtime.rs`, `host_mode.rs`
- Persistence: `native/crates/mclone-server/src/persistence.rs`,
  [`../persistence-architecture.md`](../persistence-architecture.md)
- Session/bus status quo: [`../session-network-architecture.md`](../session-network-architecture.md)
  (accurate), [`../protocol.md`](../protocol.md) (stale: version, tables),
  [`../multiplayer-hosting.md`](../multiplayer-hosting.md) (stale:
  persistence claims)
- Tacticals: 009 (original wire shape), 133 (bus/pacing; Slice 6 = push),
  151 (inbound pipeline), 154 (ingress cleanup), 167 (startup contract),
  116 (cadence)
- Vanilla receipts: [`vanilla/networking.md`](vanilla/networking.md)
