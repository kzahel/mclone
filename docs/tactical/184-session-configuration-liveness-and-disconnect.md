# 184: Session Configuration, Liveness, And Disconnect

Status: active 2026-07-16

Topic: `multiplayer-networking`

Workstream: shared native Rust protocol, server-session, client-replica,
runtime, and UI behavior plus native TCP/direct WebSocket/browser-worker
transport mechanics. Platform adapters may open/close sockets, measure wall
time, and surface lifecycle events; they do not own session phases,
capability policy, disconnect semantics, or movement sequencing.

## Goal

Turn the current identity-bearing transport handshake into a bounded,
observable session lifecycle:

```text
transport connect
  -> version + identity + supported-capability handshake
  -> negotiated capabilities
  -> ordered server configuration
  -> PLAY ready
  -> remote keepalive challenge/echo
  -> explicit client or server disconnect reason
  -> clean close or visible failure state
```

Ride movement sequence numbers and the correction echo on the same coordinated
protocol bump. This preserves the future input-replay path without implementing
prediction replay or movement validation in this tactical.

## Current Problem

Protocol v23 validates one version and unauthenticated UUID/display name, then
immediately treats the connection as playable. Server cadence, view limits,
and optional debug behavior are implicit. TCP and WebSocket peers discover
normal closure through EOF/close events, with no logical reason. A half-open
remote connection can remain registered indefinitely. Debug commands are
ordinary protocol variants rather than negotiated behavior, and movement
corrections cannot identify the latest client movement the server accepted.

The shared title/session UI can show startup failure, but an already-active
session has no replica-owned configuring/disconnected state to project after
transport loss.

## Vanilla 1.17.1 Receipts

- `ServerGamePacketListenerImpl.tick()` sends a 64-bit keepalive challenge
  every 15 seconds and disconnects when the prior challenge remains pending:
  `server/network/ServerGamePacketListenerImpl.java:155-164,245-255`.
- `handleKeepAlive()` accepts only the pending matching challenge, smooths the
  measured latency, and disconnects a non-local player on an invalid echo:
  `ServerGamePacketListenerImpl.java:1416-1423`.
- `ClientPacketListener.handleKeepAlive()` immediately echoes the same id:
  `client/multiplayer/ClientPacketListener.java:1499-1501`.
- Native client/server Netty pipelines also install a 30-second read timeout:
  `network/Connection.java:290` and
  `server/network/ServerConnectionListener.java:94`.
- A play-state kick sends `ClientboundDisconnectPacket` before making the
  connection read-only and closing it:
  `ServerGamePacketListenerImpl.java:292-296`; the client applies its reason
  through `ClientPacketListener.handleDisconnect()` at lines 657-658.
- `ClientboundLoginPacket` carries server-owned play configuration including
  dimension/world facts, max players, and chunk radius before ordinary world
  packets: `network/protocol/game/ClientboundLoginPacket.java:16-105`.
- Vanilla movement packets do not carry input sequence numbers and corrections
  do not echo them. Mclone deliberately adds these neutral fields now so the
  stronger replay design remains cheap later; current behavior remains
  vanilla-shaped client authority plus teleport-id correction.

## Product And Ownership Contract

### Observable phases

The shared client replica owns:

```text
Connecting -> Configuring -> Playing -> Disconnected(reason)
```

`SessionConfiguration` must precede `SessionReady` in the ordered update
stream. World/chunk/player updates may follow `SessionReady`. The shared UI
projects Connecting, Configuring, timeout, server-close, version/capability
mismatch, and transport-failure text without app-local parsing.

This is a play-session state model, not Mojang's packet-state registry. The
transport handshake still completes before the normal command/update frames.

### Negotiated capabilities

The client handshake carries a `u64` supported-capability mask. The server
accepts the intersection of client support and server-enabled capabilities and
returns that mask in the acceptance response. Unknown optional bits are
ignored by the current server rather than crashing decoding; protocol v24
still uses strict version equality for core message compatibility.

The first optional capability is `DEBUG_ACTIONS`. It gates
`SetDebugHotbarSlot`, `ShootDebugPhysicsCube`, and
`PlayerActionKind::DebugInstantBreak` at the authoritative session boundary.
Current development hosts may enable it by default, but a client that did not
negotiate it cannot invoke those actions. Capability negotiation does not grant
permissions or authentication.

`SessionConfiguration` reports the negotiated mask, current 20 Hz gameplay and
publication rates, and authoritative render/tracking-distance ceilings. A
future variable-cadence slice owns live reconfiguration and rate-aware client
behavior; this tactical makes the initial facts explicit.

### Remote liveness

- Dedicated TCP and direct WebSocket sessions send one logical 64-bit
  `KeepAlive` challenge after 15 seconds.
- Native and browser transport actors echo it immediately on their outbound
  ordered command lane, independent of drawable-frame update budgets.
- A missing or wrong echo closes the session with typed `Timeout` reason. TCP
  also retains a 30-second read timeout for handshake/transport silence.
- Local integrated sessions use the same phase/configuration message model but
  disable half-open liveness: an in-process channel cannot become a silent
  remote socket.
- Keepalive frames never mutate gameplay state and do not count as player
  activity, movement, or persistence dirtiness.

The production defaults follow vanilla. Tests inject short intervals rather
than sleeping for real protocol time.

### Explicit close

Add typed logical disconnect messages in both directions. Server reasons cover
timeout, duplicate identity, protocol violation, shutdown/kick, internal
failure, and transport loss with bounded human-readable detail. Client clean
close carries `Quit`; EOF remains valid but is converted to an observable
transport/end-of-stream reason when no explicit reason arrived.

The server queues `ServerUpdate::Disconnect` before closing TCP/WebSocket.
Client transports preserve that ordered update for the shared replica before
reporting actor shutdown. If a later synthetic EOF follows an explicit reason,
the first authoritative reason wins.

### Movement sequence plumbing only

Every `MovePlayer` command gains a monotonically wrapping nonzero `u32`
sequence selected by `LocalPlayerMoveSync`. The server records the latest
accepted sequence per player. Every `PlayerPosition` correction echoes
`last_applied_move_sequence` alongside the existing teleport id.

Sequence zero is reserved for fixtures/legacy internal callers that have not
selected a production sequence. No replay buffer, server input simulation,
anti-teleport check, or correction replay lands here.

## Implementation Slices

### Slice 1: Protocol v24 session vocabulary

- Add capability masks to native/WebSocket handshakes and acceptance results.
- Add typed configuration, ready, keepalive, client/server disconnect, and
  bounded reason codecs.
- Add sequenced movement wrapper and correction echo.
- Extend strict round-trip, malformed-mask/reason/configuration, trailing-byte,
  and handshake compatibility tests.

Exit: all lifecycle facts have one transport-neutral, validated codec.

### Slice 2: Shared server/client session semantics

- Queue configuration then ready before ordinary join facts for local and
  dedicated players.
- Track client replica phases/configuration/disconnect reason.
- Select movement sequences client-side and record/echo accepted sequences
  server-side without changing movement authority.
- Reject unnegotiated debug actions in the dedicated session adapter.
- Project active-session lifecycle state into the shared status UI.

Exit: local integrated play proves ordering, state projection, capability
gating, and sequence echo without socket-specific policy.

### Slice 3: Native TCP liveness and clean close

- Add completion handshakes that return negotiated capabilities.
- Run 15-second challenge state in the dedicated session and a 30-second TCP
  read timeout.
- Echo keepalive from the native reader through the writer lane without
  drawable-frame involvement.
- Send typed disconnect before writer shutdown; convert unexpected read failure
  into one queued client-visible reason; send client `Quit` during clean drop.
- Add fake-clock liveness tests plus real TCP configuration, echo, explicit
  close, timeout, and sequence acceptance where practical.

Exit: native remotes cannot remain silently half-open and receive a reasoned
ordered close.

### Slice 4: Direct/browser WebSocket parity and closeout

- Apply the same dedicated-session challenge/gating policy to direct WebSocket
  peers.
- Echo keepalive inside the browser socket worker using Rust-owned codecs.
- Send client `Quit` before worker close and synthesize visible unexpected-close
  state without replacing an earlier server reason.
- Run wasm/typecheck/browser remote seams, update protocol/networking/hosting
  living docs, and close this tactical with evidence.

Exit: TCP, direct WebSocket, browser worker, and local integrated paths share
one lifecycle vocabulary and client state model.

## Required Evidence

- configuration precedes ready and reports truthful fixed rates/view ceilings;
- capability negotiation takes the client/server intersection and an
  unnegotiated debug command is rejected before gameplay mutation;
- matching keepalive clears pending state and records latency; wrong/missing
  echo produces typed timeout;
- clean client quit, duplicate UUID, server rejection, unexpected EOF, and
  server close remain distinguishable to the client;
- movement sequences are nonzero/monotonic in production and corrections echo
  the latest accepted sequence; fixtures may explicitly use zero;
- local integrated play never schedules a remote liveness timeout;
- native TCP and direct/browser WebSocket codec and end-to-end paths agree;
- formatting, protocol/net/server/client/app-runtime/scene/dedicated suites,
  wasm build, web typecheck/smoke, and scoped `git diff --check` pass.

## Explicit Non-Goals

- Mojang/Microsoft authentication, encryption, identity proof, permissions,
  bans, or multiple local profiles;
- automatic reconnect or retaining a replica across reconnect;
- vanilla movement validation, server-authoritative input simulation, client
  prediction replay, or anti-cheat;
- inventory synchronization/persistence or broader player state;
- compression, version ranges, required-capability negotiation, or dynamic
  protocol registries;
- variable gameplay/publication rates or live session reconfiguration;
- chat/kick administration UI or localization of disconnect text.

## Validation Commands

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-protocol
cargo test --manifest-path native/Cargo.toml -p mclone-net
cargo test --manifest-path native/Cargo.toml -p mclone-client
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-dedicated-server
cargo check --manifest-path native/Cargo.toml -p mclone-native-client \
  -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml \
  --target wasm32-unknown-unknown -p mclone-web-client
pnpm native:web:typecheck
git diff --check
```

This slice changes UI status text but not world pixels or render math. Capture
is required only if closeout changes layout/presentation beyond the existing
status overlay.

## Related

- [`183-authoritative-world-time-persistence.md`](183-authoritative-world-time-persistence.md)
- [`182-local-profile-and-player-persistence-proof.md`](182-local-profile-and-player-persistence-proof.md)
- [`176-dedicated-autonomous-push-runtime.md`](176-dedicated-autonomous-push-runtime.md)
- [`../topics/multiplayer-networking.md`](../topics/multiplayer-networking.md)
- [`../topics/client-prediction.md`](../topics/client-prediction.md)
- [`../topics/vanilla/networking.md`](../topics/vanilla/networking.md)
- [`../session-network-architecture.md`](../session-network-architecture.md)
- [`../protocol.md`](../protocol.md)
