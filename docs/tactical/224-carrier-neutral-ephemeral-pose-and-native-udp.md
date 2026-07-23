# 224: Carrier-Neutral Ephemeral Pose And Native UDP

Status: active 2026-07-23.

Topic: `remote-player-presentation`

Topic: `multiplayer-networking`

## Goal

Implement the first complete mixed-reliability remote-player body-pose path
without making one socket API, browser API, or third-party transport library
the game architecture:

```text
decoded logical session
  reliable ordered lane
    -> memory / TCP / WebSocket / future reliable carrier
  ephemeral latest-wins lane
    -> memory / reliable fallback / native UDP / future datagram carrier
```

The first production datagram carrier is a dependency-free native
TCP-plus-UDP profile. Existing TCP establishes the session and carries every
correctness-critical fact. A companion `std::net::UdpSocket` carries bounded,
self-contained, sequenced body-pose samples in both directions after attaching
to that session. Browser clients retain the WebSocket reliable-compatibility
profile. WebTransport and WebRTC remain future carrier adapters, not protocol
owners.

## Motivation

Local movement now runs at a shared fixed 60 Hz independently of presentation
cadence, but local body-pose reporting is capped at 20 Hz and every shipping
remote-player update shares one reliable ordered stream with chunks, world
state, and gameplay facts. A lost TCP byte can therefore delay a newer pose
behind retransmission. The remote client also keeps only the newest pose and
eases toward it with one fixed half-life; it has no sample timeline with which
to absorb cadence jitter, loss, duplication, or reordering.

The accepted direction is deliberately not "replace networking with
WebTransport" or "build a reliable UDP protocol":

- server and client sessions negotiate capabilities and effective carriers;
- logical reliable and ephemeral classes exist independently of carriers;
- native clients may use TCP plus raw UDP;
- browser clients may use WebSocket compatibility, WebTransport datagrams, or
  WebRTC data channels according to topology and support;
- a future mature reliable-UDP or QUIC implementation can replace the carrier
  behind the same decoded boundary;
- no large C++ networking dependency is introduced by this tactical.

Routine movement remains permissively client-authoritative. This tactical
improves report transport and remote presentation; it does not add server
movement replay or anti-cheat.

## Ownership And Platform Contract

### Shared Rust owns meaning

- `mclone-protocol` owns reliable and ephemeral logical message types, codec
  validation, epochs, sequences, discontinuities, and negotiated profile
  facts.
- `mclone-client` owns local pose selection and the remote snapshot timeline.
- `mclone-server` owns accepted player state, pose coalescing, observer
  routing, barriers, and publication budgets.
- `mclone-app-runtime` owns the decoded client connection boundary, update
  pumping, queue diagnostics, and reliable fallback.
- `mclone-net` owns native TCP/UDP framing and socket actors.
- the dedicated app owns listener lifecycle and attaches decoded transport
  events to the ordinary `RealmServer` connection registry.
- `mclone-scene` owns frame-independent body report cadence and evaluates
  remote presentation at render time.

Desktop flat, desktop XR, flat Android, and Android XR instantiate the same
native remote implementation. OpenXR continues to sample runtime poses at the
OpenXR action/frame boundary; no free-running XR poller is added.

### TypeScript and JavaScript stay domain-blind

Browser TypeScript/JavaScript may:

- open a browser-owned socket or data channel;
- transfer bounded opaque byte buffers to and from a Rust worker actor;
- report generic open, close, error, pressure, and capability mechanics;
- execute Rust-authored opaque operations.

It must not:

- identify a player-pose message;
- parse an epoch, sequence, coordinate, heading, or gameplay command;
- select report cadence, reliability, fallback, interpolation, or barriers;
- maintain a parallel player/session model;
- decide which browser transport profile is semantically sufficient.

This tactical does not require TypeScript changes for native UDP. If browser
fallback plumbing changes, existing domain-blind byte brokers remain the only
JavaScript/TypeScript role and structural source gates pin that boundary.

## Logical Lane Contract

### Reliable ordered lane

The reliable lane remains the sole carrier for:

- handshake, identity, configuration, ready, keepalive, and disconnect;
- player add/remove, appearance, and presentation-epoch changes;
- teleports, transfer, death, respawn, and persistence barriers;
- chunks, world mutations, inventory, and gameplay interactions;
- a pose keyframe/barrier needed before an ordering-sensitive command;
- recovery when the ephemeral lane is unavailable.

### Ephemeral latest-wins lane

The first ephemeral schema carries a complete body sample:

- session-bound source identity supplied by the transport attachment, not
  trusted from an arbitrary payload;
- presentation epoch and wrapping nonzero sequence;
- bounded session-relative sample time;
- world-space feet/root position;
- view/body yaw and pitch in the current representation;
- grounded state;
- discontinuity/keyframe status where required.

Samples are independently decodable and do not depend on a missing predecessor.
The receiver rejects malformed, non-finite, wrong-session, stale-epoch,
duplicate, and older-sequence samples. Pressure supersedes older samples
instead of growing an ordered backlog.

Cross-lane arrival order is never meaningful. An interaction that depends on
current pose first sends a reliable self-contained pose barrier and then the
interaction on the same reliable lane. Teleport, transfer, death, respawn, and
representation changes advance the presentation epoch so older datagrams
become inert.

## Native UDP Attachment

The native dual-lane profile reuses the existing TCP session:

1. The reliable handshake negotiates ephemeral-pose support and publishes the
   effective profile plus UDP endpoint information.
2. The server creates an unpredictable, expiring attachment token associated
   with the accepted TCP connection.
3. The client sends a bounded UDP attach message containing that token.
4. The server binds the observed source endpoint to the reliable session and
   confirms attachment over the reliable lane.
5. Body samples flow client-to-server and server-to-observer over UDP.
6. Inactivity, malformed traffic, source change without reattachment, or
   socket failure disables UDP and returns the session to reliable fallback
   without disconnecting gameplay.

The UDP module uses `std::net::UdpSocket`; it does not add Tokio, QUIC, ENet,
Renet, GameNetworkingSockets, or another transport dependency. Initial packets
remain conservatively below 1200 bytes and never rely on IP fragmentation.
The lane has explicit packet/sample/byte limits and anti-amplification
behavior. An unpredictable session token prevents trivial off-path
cross-session injection; this first cooperative/LAN profile does not promise
confidentiality or public-Internet authentication.

The TCP and UDP listeners may use the same numeric port because their protocol
namespaces are independent. The dedicated CLI retains an explicit override and
can disable UDP for compatibility and conformance tests.

## Cadence And Presentation Contract

- Local semantic movement remains configurable, default 60 Hz.
- Mixed-reliability body selection is change-driven and capped at 60 Hz.
- Reliable compatibility is change-driven and capped at 20 Hz.
- An unchanged body sends only a wall-clock heartbeat, initially one second.
- Server ingest and per-observer replication budgets remain independently
  named even when a profile gives them equal numeric defaults.
- Late frames do not create publication catch-up bursts.
- Lifecycle close/persistence forces a reliable final body keyframe.

Remote players receive a bounded sequence-aware body snapshot track rather
than only a latest target. It retains sender-relative and local-arrival timing,
handles wrap/duplicates/reordering/gaps, and interpolates at a deliberately
delayed presentation time. Teleports and other discontinuities reset the
track. General entity smoothing does not change.

The initial interpolation delay is derived from negotiated sample interval and
measured jitter under explicit clamps. It is diagnostics-visible and
configurable so later human motion review tunes a policy rather than requiring
an architectural rewrite.

## Implementation Slices

### Slice 0: Documentation and tactical contract

- Correct topic and architecture documents that currently make WebTransport
  the common native/browser architecture.
- Record multiple carrier profiles, dependency-free native UDP first, and the
  domain-blind browser boundary.
- Preserve WebTransport for future browser/dedicated and Share-to-Browser
  support; preserve WebRTC for browser-hosted rooms.

Exit: implementation can be reviewed against one accepted, carrier-neutral
contract.

### Slice 1: Logical messages, capabilities, and reliable fallback

- Add client/server ephemeral body-sample types and strict codecs.
- Add presentation epoch/sequence/time and effective transport-profile facts.
- Extend client connection and server publication boundaries with distinct
  reliable and ephemeral methods.
- Map integrated memory directly and map TCP/WebSocket to bounded reliable
  fallback without changing JavaScript message semantics.
- Replace fixed publication-attempt reminders with time-based cadence and
  select the negotiated 60/20 Hz profile.

Exit: one logical pose trace is proven over integrated, TCP, and WebSocket
compatibility paths before a lossy production carrier is enabled.

### Slice 2: Snapshot timeline and loss conformance

- Add remote-player body snapshot tracks without changing generic entities.
- Interpolate position/topology/rotation against sample time.
- Reset correctly on add/remove, teleport epoch, respawn, transfer, and gaps.
- Add deterministic loss, delay, duplication, reordering, wrap, and pressure
  simulation.
- Add selected/suppressed/superseded/received/relayed/stale/gap/jitter/age
  diagnostics.

Exit: the decoded pose contract behaves correctly under datagram conditions
while still carried by reliable test adapters.

### Slice 3: Dependency-free native dual-lane carrier

- Add contained UDP packet codec, client/server socket actors, attachment
  tokens, endpoint binding, timeouts, and bounded latest-wins queues.
- Negotiate `NativeTcpUdp` versus `ReliableOnly`.
- Send client body samples and observer remote samples through UDP after
  attachment.
- Preserve reliable pose barriers and automatic fallback.
- Add real-loopback two-client, loss, wrong-token/source, timeout, fallback,
  pressure, and shutdown tests.

Exit: native clients and the dedicated server use UDP for ordinary body pose
while every critical fact and fallback remains on TCP.

### Slice 4: Cross-platform closeout

- Run shared protocol/net/server/client/app-runtime/scene/dedicated tests.
- Build desktop flat/XR, browser WASM, flat Android, and Android XR.
- Run native two-client and browser WebSocket compatibility smokes.
- Run domain-blind TypeScript/JavaScript structural gates.
- Record available high-refresh/LAN motion evidence and leave physical Quest
  or second-device checks explicit when hardware is unavailable.

Exit: the tactical closes only when all automated platform seams pass and
remaining human feel/device checks are distinguished from correctness.

## Automated Evidence

Required tests prove:

1. unchanged players do not publish at the maximum rate;
2. heartbeat duration is independent of 20/30/60/120 Hz presentation;
3. reliable fallback and native UDP decode to the same logical samples;
4. loss, duplication, reordering, sequence wrap, and pressure never apply an
   older pose after a newer pose;
5. teleport/respawn/transfer epochs make old datagrams inert;
6. interaction-after-pose barriers remain ordered on memory, TCP, WebSocket,
   and simulated datagram paths;
7. UDP attachment cannot claim another live TCP session with a wrong or
   expired token;
8. UDP disappearance returns to reliable fallback without losing session or
   gameplay updates;
9. one slow observer cannot stall critical reliable traffic or other peers;
10. no browser TypeScript/JavaScript source gains protocol or game-domain
    vocabulary.

## Human And Physical Evidence

Human review tunes, but does not define, the transport contract:

- compare 20 Hz reliable and 60 Hz mixed profiles on a high-refresh display;
- rotate while moving in circles and inspect perceived heading/trajectory lag;
- test two physical LAN devices through host firewalls and ordinary Wi-Fi;
- test Android lifecycle and Quest headset motion when hardware is available.

Diagnostics must make selected profile, report/relay rate, loss, jitter,
interpolation delay, fallback, and pose age visible during those checks.

## Explicit Non-Goals

- reliable-over-UDP, congestion-controlled bulk UDP, or replacing TCP;
- choosing ENet, Renet, Quinn, WebTransport, or GameNetworkingSockets;
- encryption, public-Internet security claims, Mojang auth, or anti-cheat;
- WebRTC signaling/TURN or Share-to-Browser product UI;
- XR head/hand component replication, IK, or body/view model redesign;
- delta-compressed or quantized poses;
- authoritative server movement replay or client input reconciliation;
- moving socket IO, codecs, or game semantics onto the render thread;
- adding game-domain meaning to JavaScript or TypeScript.

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
cargo check --manifest-path native/Cargo.toml \
  -p mclone-native-client -p mclone-android-client \
  -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml \
  --target wasm32-unknown-unknown -p mclone-web-client
pnpm native:web:typecheck
pnpm native:thin-adapters:purity
pnpm native:android:apk
pnpm native:android-xr:apk
git diff --check
```

Rendered pixels are not changed intentionally by the carrier slice. A static
screenshot is therefore not acceptance evidence for motion quality; automated
trajectory traces and a short motion capture are the relevant presentation
evidence if interpolation changes become drawable during closeout.

## Execution Record

Active. Append each landed slice, commit, validation result, measured default,
and explicit remaining hardware/human check here.
