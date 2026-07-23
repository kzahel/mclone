# Remote Player Presentation Quality

Topic: `remote-player-presentation`

Status: Tactical
[`224`](../tactical/224-carrier-neutral-ephemeral-pose-and-native-udp.md)
active as of 2026-07-23. The current product
has a carrier-neutral, complete, sequenced body-pose sample and a distinct
ephemeral send boundary. Integrated, TCP, and WebSocket compatibility paths
currently carry that message through a bounded reliable fallback; remote
clients reject stale epoch/sequence samples and retain the newest state.
Buffered snapshot interpolation and the native UDP carrier remain active
Tactical 224 slices. Separate body, head, and hand poses remain later
representation work. The first datagram carrier is dependency-free raw UDP
beside existing native TCP, and WebTransport/WebRTC remain later adapters for
their supported browser and peer-hosted topologies.

Scope: the quality, timing, representation, relay, and presentation of other
players after local movement has produced a reportable state. This includes
flat and XR embodiment, pose-report and server-replication rates, change
thresholds and heartbeats, discontinuities, remote snapshot buffering,
interpolation/extrapolation, bandwidth policy, and transport semantics for
ephemeral pose data.

Local input collection and 60 Hz movement command materialization live in
[`input-observation-timeline.md`](input-observation-timeline.md). Local
movement authority and server pose acceptance live in
[`client-prediction.md`](client-prediction.md). Session lifecycle, durable
ordered commands, transports, and server publication infrastructure live in
[`multiplayer-networking.md`](multiplayer-networking.md). This topic does not
reopen authoritative movement replay or anti-cheat.

## Vocabulary

In this topic, a player **pose** is not a complete animated skeleton.

- **Body pose**: world-space feet/root position, body or locomotion heading,
  grounded state, and any compact motion facts needed for presentation.
- **View pose**: where a flat player is looking. Today this is the same
  yaw/pitch pair carried with the body.
- **Head pose**: optional tracked head position and orientation relative to
  the body/root, with tracking validity.
- **Hand pose**: optional left/right tracked aim and/or grip transforms
  relative to the body/root, with tracking validity.
- **Derived animation**: walk phase, limb swing, IK, and model-specific bone
  transforms reconstructed by the receiving client. These are not normally
  network facts.

The server uses the accepted body/root position for chunk interest,
persistence, and location-dependent gameplay. Head and hand offsets are
presentation facts and must not move server interest or become alternate
player locations.

## Current State (Verified 2026-07-23)

### Local reporting

- Local movement is simulated from fixed 60 Hz semantic commands.
- `mclone-scene` checks one shared negotiated player-pose publication
  deadline on Mono and XR paths: 20 Hz for reliable compatibility and 60 Hz
  for a mixed-reliability profile.
- The selector emits a complete body pose when position, rotation, or grounded
  state changes. An unchanged body emits a one-second heartbeat measured on
  the shared monotonic timeline, independent of frame or attempt rate.
- Attack/use and other ordering-sensitive paths can force the current pose
  before the gameplay command.
- The shared scene owns this policy; desktop, browser, Android, and XR
  adapters do not select movement packets.

### Wire and server relay

- Protocol v33 defines strict client body-pose and observer remote-body-pose
  codecs with nonzero epochs/sequences, finite values, bounded payloads, and
  wrap-aware sequence comparison.
- `ClientConnection` exposes a typed ephemeral send method. Its default maps
  to an explicit reliable-fallback command, so integrated, native TCP, and
  browser WebSocket sessions exercise the same decoded pose semantics without
  adding protocol awareness to JavaScript or TypeScript.
- The server accepts the permissively client-authored sample, applies normal
  movement validation, and routes a sequenced remote sample to observers.
- The receiving replica ignores unknown-player, stale-epoch, duplicate, and
  older-sequence samples. It still presents only the latest target through the
  existing smoothing path until Tactical 224 Slice 2 adds a bounded timeline.

`RemotePlayerUpdate` currently contains:

- `RemotePlayerId`;
- appearance/model selection;
- world-space feet position;
- one yaw and pitch pair;
- `on_ground`.

It has no remote-presentation epoch, pose sequence, sample time, velocity,
discontinuity marker, independent body heading, head transform, or hand
transforms. The incoming `MovePlayer` sequence is not retained in the remote
update.

The server accepts finite/clamped client movement under the permissive
movement-authority policy. Each accepted move reconciles the player as a
remote-player subject and routes an update to interested observers. Add,
update, and remove messages share the same reliable ordered per-client stream
as chunks, gameplay state, life state, teleports, and other protocol facts.

Integrated memory channels, native TCP, direct WebSocket, and browser worker
WebSocket paths therefore provide reliable ordered delivery today. No
shipping path offers an unreliable datagram side channel.

### Remote client and rendering

`mclone-client` retains only the latest `RemotePlayerUpdate` per player. It
derives cumulative horizontal walk distance when accepted updates arrive.

`ActorInterpolationState` then eases each rendered actor toward the newest
target with a frame-rate-independent exponential half-life of 80 ms. This is
better than frame-count-dependent smoothing, but it:

- has no history from which to interpolate at a deliberately delayed time;
- does not model source sample cadence, arrival jitter, loss, or reordering;
- never converges exactly in finite time;
- can trade visible lag against jitter only through one fixed half-life;
- applies the same presentation policy to remote players and other actors;
- cannot present head or hands because those facts do not exist.

Remote players render through the shared actor path on Mono, stereo, and XR
multiview surfaces. The current player figure uses feet position, the one
yaw/pitch pair, grounded state, and derived walk distance. There is no
networked XR embodiment path beside it.

### Cadence contract

The protocol already carries `SessionConfiguration.gameplay_rate_hz` and
`publication_rate_hz`, but server configuration currently populates the fixed
20/20 values and the scene's player-pose deadline remains a fixed 50 ms.
`publication_rate_hz` does not configure pose reporting or remote
interpolation.

The current effective lanes are:

| Lane | Current behavior |
| --- | --- |
| Local semantic movement | configurable, default 60 Hz |
| Keyboard/touch observation | platform event callbacks |
| Browser ordinary gamepad observation | one snapshot near each animation frame |
| OpenXR head/actions/hands | one observation at the OpenXR frame/action-sync boundary |
| Local body-pose report selection | change-driven, capped at 20 Hz |
| Server host/gameplay/physics | configurable profile, default 20/20/60 Hz |
| Remote-player relay | generated when accepted movement is processed |
| Remote actor presentation | render cadence with fixed 80 ms half-life |

These are distinct clocks even where their current numeric defaults happen to
match.

## Accepted Direction

### Keep every cadence independently named and configurable

Do not introduce one universal player, network, or publication tick. The
durable model distinguishes:

1. local physical-input observation cadence;
2. local semantic movement cadence;
3. body-pose report selection and maximum send cadence;
4. tracked head/hand report selection and maximum send cadence;
5. server pose-ingest/drain cadence;
6. per-observer remote-player replication cadence and budget;
7. remote snapshot presentation delay;
8. render/display cadence;
9. world/gameplay/AI cadences.

Configuration may choose equal values, but equality is a profile decision and
not an ownership rule.

Replace or precisely narrow the ambiguous session `publication_rate_hz`
contract. A future negotiated configuration should be able to express at
least:

- the maximum client body-pose report rate the server wants;
- the maximum tracked-pose report rate, when tracked components are supported;
- the server's intended remote-player replication rate or interval;
- protocol capabilities for component poses and mixed-reliability transport;
- the effective transport profile and its rate limits.

The local movement rate remains a local simulation profile. Because the server
does not replay movement, it does not need to force local movement to its
world or replication rate.

### Reports are change-driven and rate-capped

A maximum rate is not a requirement to send an unchanged packet every
interval.

- Select body reports when position, body heading, view heading, or grounded
  state crosses the relevant threshold.
- Select tracked reports when head/hand transforms or tracking validity cross
  their thresholds.
- Cap each component class independently.
- Express idle reminders as wall-clock durations, not a fixed count of
  publication attempts.
- Force a current body report before an ordering-sensitive gameplay command,
  lifecycle persistence barrier, or discontinuity.
- Do not emit catch-up bursts after a late frame.

A reasonable first body profile to measure is change-driven reporting at up
to 60 Hz with an approximately one-second stationary heartbeat. That is a
candidate baseline, not a claim that every server or connection must relay
60 updates per second. Tracked head/hands may justify another cap based on
measured visual benefit and bandwidth.

### Separate body, head, hands, and derived animation

The protocol/presentation model should support optional components without
making flat clients manufacture tracked hardware:

- Body/root remains the required component and the server's accepted location.
- Flat clients provide body/root and view orientation. Receivers derive a head
  pose suitable for the selected model.
- XR clients may add tracked head and left/right hand components in a neutral,
  body-relative coordinate frame.
- Tracking validity and loss are explicit. A lost hand expires or blends back
  to model animation rather than freezing forever.
- Body heading, locomotion reference, and head/view heading are not aliases.
- The receiver derives skeleton bones and IK from the compact component poses.

Body-relative head/hand transforms keep values small, make quantization
practical, and prevent a tracked offset from becoming another world-space
authority. The shared contract must define handedness, axes, units, pose
origins, validity, and maximum sane offsets. Non-finite or absurd
presentation offsets are rejected or dropped as data-safety checks, not
anti-cheat.

### Give the remote client a real snapshot timeline

Replace latest-target-only easing for remote players with a bounded,
sequence-aware snapshot track:

- one presentation epoch and monotonically ordered sample sequence per player;
- sender sample timing expressed as session-relative monotonic information,
  never a platform or wall-clock timestamp;
- arrival timing retained locally for jitter measurement;
- bounded history sufficient for the configured interpolation delay;
- shortest-path rotation and topology-aware position interpolation;
- explicit discontinuities for teleports, dimension changes, respawn, world
  transfer, representation changes, and unrecoverable gaps;
- deterministic handling of duplicates, reordering, loss, and sequence wrap;
- a bounded extrapolation policy, including the valid choice of no
  extrapolation initially.

Presentation normally evaluates at `now - interpolation_delay`, between two
known samples. The delay should be derived from the negotiated replication
interval and measured jitter rather than hidden inside a fixed smoothing
half-life. New samples do not retroactively change already presented history.

Large discontinuities snap or use a deliberately chosen transition effect;
they do not ease through walls or across a dimension. Periodic/looping world
topologies use the nearest valid lift before interpolation.

Walk phase and other derived animation advance from the interpolated body
trajectory. Head and hand interpolation uses component validity and
orientation-aware interpolation rather than the actor body's scalar
yaw/pitch path.

### Coalesce server work without losing ordering barriers

Under the accepted client-authoritative policy, the server does not need to
simulate every intermediate body pose. It may coalesce adjacent ephemeral
pose samples to the newest useful state before replication, provided it
preserves barriers around:

- explicit teleports and teleport acknowledgements;
- dimension/world transfer, death, respawn, join, and removal;
- interactions whose server interpretation depends on the current player
  location or view;
- persistence/lifecycle flushes;
- interest-boundary changes that require add/remove routing.

Coalescing policy must be observable. Counters should distinguish received,
superseded, relayed, dropped-for-pressure, and forced/keyframe samples.

## Transport-Neutral Ephemeral Pose Semantics

The current reliable stream remains the required foundation for all
correctness-critical session and gameplay messages. Remote pose quality should
nevertheless be designed so it does not depend on reliable stream ordering.

Define a logical ephemeral pose sample that can travel over today's reliable
stream first and then travel over the accepted unreliable sequenced channel.
The exact Rust schema is not fixed, but it needs enough information to
identify:

- session/player and presentation epoch;
- sample sequence;
- represented component set;
- body/head/hand values and validity;
- a self-contained baseline or dependency on a known baseline;
- discontinuity/keyframe status;
- bounded sender-relative sample timing.

The receiver must already tolerate loss, duplicates, and reordering in tests
even when the production adapter is reliable. That makes a later transport
change an adapter/capability decision rather than a rewrite of remote
presentation.

### Reliable and ephemeral classes remain separate

Keep these on the reliable ordered stream:

- login/session state and capability negotiation;
- player add/remove, identity, appearance, and representation changes;
- dimension transfer, respawn, death/life state, and explicit teleport;
- inventory, interactions, world changes, and persistence barriers;
- a recovery keyframe if an ephemeral stream cannot self-recover.

Body/head/hand samples between those barriers may be latest-wins and lossy.
They must never be the sole carrier of a critical lifecycle transition.

### Mixed-reliability transport is a target, not a measurement-gated option

The architecture is one decoded session with explicit reliable and ephemeral
lanes, not one preferred socket implementation. The server advertises
supported profiles and the client selects the best mutually supported shape:

| Profile | Reliable carrier | Ephemeral carrier | Initial use |
| --- | --- | --- | --- |
| Integrated | in-memory ordered queue | in-memory latest sample | singleplayer/tests |
| Native reliable | TCP | bounded reliable fallback | compatibility |
| Native mixed | TCP | dependency-free raw UDP | first production datagram slice |
| Browser reliable | WebSocket | bounded reliable fallback | current web clients |
| Browser mixed | WebTransport stream | WebTransport datagram | future client/server adapter |
| Browser peer | reliable data channel | unordered zero-retransmit channel | future browser-hosted rooms |

Every carrier decodes to the same logical pose sample and cross-lane barrier
contract. No platform API record enters the protocol. A future QUIC,
WebTransport, ENet, Renet, or other implementation can replace one physical
profile without changing client prediction, server authority, observer
routing, or remote interpolation.

Tactical 224 first adds `std::net::UdpSocket` beside the existing native TCP
session. TCP negotiates an unpredictable expiring attachment token; a bounded
UDP hello binds the observed endpoint to that reliable session. Ordinary
self-contained body samples then travel in both directions over UDP. Wrong
session/source, malformed, stale, duplicate, and oversized datagrams are
dropped. Timeout or carrier failure returns pose traffic to reliable fallback
without disconnecting the gameplay session. UDP does not carry chunks,
inventory, interactions, lifecycle transitions, or a home-grown reliable
protocol.

TCP and WebSocket remain supported compatibility transports. Browser
WebTransport support is feature-detected when that future adapter lands; older
browsers or networks retain WebSocket. WebRTC remains the accepted carrier for
browser-hosted peer rooms, where ICE, DTLS, SCTP, signaling, and possible TURN
relay are topology requirements rather than dedicated-server defaults. See
[`browser-hosted-peer-sessions.md`](browser-hosted-peer-sessions.md).

The initial candidate transport profiles are:

| Profile | Pose transport | Candidate body cap |
| --- | --- | ---: |
| Mixed reliability | sequenced datagrams | 60 Hz |
| Reliable compatibility | reliable stream | 20 Hz |

These are independently configurable starting profiles, not protocol
constants. The reliable profile's 20 Hz cap limits stale queued pose traffic;
it does not reduce local input observation or the client's 60 Hz movement
simulation. A clean TCP connection may carry 60 samples per second, but TCP
cannot prevent a lost byte from delaying newer pose data behind
retransmission. The compatibility profile therefore promises graceful,
interpolated motion rather than the same tail latency as the datagram profile.

The mixed-reliability implementation requires:

- session-bound attachment and rejection of wrong player/source IDs;
- congestion and send-budget behavior that yields before critical traffic;
- payloads bounded below the path's safe datagram size, with no reliance on IP
  fragmentation;
- recovery after loss without waiting indefinitely for a missing delta;
- native and browser fallback to the reliable-stream representation;
- the same decoded pose-sample contract above every transport;
- capability and effective-reliability negotiation rather than assumptions
  based only on the requested transport.

An unreliable channel does not justify sending raw OpenXR, browser, Android,
or controller API records. It carries neutral remote-player pose components.

### Cross-lane ordering is explicit

Reliable stream ordering cannot order a separate datagram against a gameplay
command. Any command whose interpretation depends on a current pose therefore
carries or names a reliable pose barrier. One workable first contract is:

1. send a self-contained pose keyframe on the reliable stream with its epoch
   and sequence;
2. send the dependent interaction after it on the same reliable stream;
3. let ordinary newer datagram samples continue independently;
4. ignore stale-epoch datagrams after a reliable teleport, respawn, transfer,
   or other discontinuity changes the epoch.

The exact encoding remains open, but correctness must never depend on arrival
order across reliable and unreliable lanes.

### Current external transport facts

As rechecked on 2026-07-23, the
[WebTransport specification](https://www.w3.org/TR/webtransport/) defines
reliable streams, unreliable datagrams, an effective reliability mode, worker
support, and a `requireUnreliable` option. Shipping support is more granular:
[MDN browser-compatibility data](https://raw.githubusercontent.com/mdn/browser-compat-data/main/api/WebTransport.json)
records base WebTransport and datagrams in Chromium 97, Firefox 114, and
Safari 26.4, but records the draft reliability-mode members as absent in
Chromium. WebTransport became
[Baseline Newly Available](https://developer.mozilla.org/en-US/docs/Web/API/WebTransport)
across current major browsers in March 2026; older browser versions still
require feature detection and fallback. Safari added it in
[Safari 26.4](https://webkit.org/blog/17862/webkit-features-for-safari-26-4/).
[RFC 9221](https://www.rfc-editor.org/rfc/rfc9221.html) defines QUIC
DATAGRAM frames as unreliable application data within the authenticated QUIC
connection.

Current pure-Rust candidates include
[WTransport](https://github.com/BiagioFesta/wtransport), which provides
native client/server WebTransport over Quinn/Tokio but still labels itself not
fully production-ready, and
[moq-dev/web-transport](https://github.com/moq-dev/web-transport), which
offers native Quinn and browser-WASM backends. Dependency choice remains an
implementation-spike decision for a future WebTransport carrier, not a
prerequisite for Tactical 224's standard-library native UDP path.

## Platform Considerations

### Desktop and flat Android

Body pose comes from the shared fixed movement owner. Mouse/right-stick look
may change view orientation without moving the body. A later body/view split
must preserve that fact instead of encoding one platform-specific camera
packet.

Android lifecycle backgrounding should force the final pose before the local
persistence barrier. Physical-controller rate and noise affect local semantic
input, not the remote wire shape.

### Browser

Browser Gamepad snapshot limitations are upstream of local movement and do not
require a browser-specific remote-player protocol. The browser uses the same
change thresholds, component samples, snapshot buffer, and fallback reliable
semantics as native clients.

WebSocket is the current browser compatibility carrier. A future
WebTransport-over-HTTP/3 adapter can add mixed reliability where supported,
with feature-detected WebSocket or reliable-only fallback. It must sit behind
the same connection/decoded-update boundary and cannot move socket, codec,
pose, cadence, or fallback policy onto the main/render thread. TypeScript and
JavaScript remain bounded opaque-byte brokers; Rust owns protocol and game
meaning. The negotiated server session profile is authoritative. Browser
conformance tests for a future WebTransport adapter must cover both reliability
API shapes described above.

### Desktop XR and Android XR

OpenXR views and actions remain sampled at the runtime's frame/action-sync
boundary. Do not add a free-running OpenXR poller.

The XR publisher projects runtime-local poses into the shared body-relative
head/hand contract. It preserves:

- body/root movement produced by stick, room-scale reconciliation, teleport,
  hand-push, or thruster locomotion;
- independent tracked head orientation and local offset;
- per-hand aim/grip choice and tracking validity;
- the distinction between locomotion heading and head/view heading;
- tracking-loss and lifecycle discontinuities.

Remote XR presentation must work in per-eye and full-frame multiview paths.
A flat observer should still see a sensible animated avatar when receiving XR
components; an XR observer should see a sensible derived head/body pose for a
flat subject.

## Observability and Validation

Record at least:

- selected, threshold-suppressed, rate-capped, and heartbeat reports;
- bytes and samples by body/head/hand component;
- server received, coalesced, relayed, and pressure-dropped samples;
- per-observer replication interval and queue age;
- client duplicate, stale, out-of-order, gap, discontinuity, and buffer
  underrun counts;
- interpolation delay, measured arrival jitter, and extrapolated time;
- pose-to-photon age where it can be measured honestly.

Automated validation should cover:

1. unchanged players do not send at the maximum rate;
2. a one-second heartbeat remains one second at 20, 30, 60, and 120 Hz;
3. equivalent motion observed at different render rates produces equivalent
   report samples and remote trajectories;
4. 20/30/60 Hz report and replication profiles remain independently
   configurable;
5. jitter, loss, duplication, reordering, and sequence wrap do not move a
   remote player backward through stale state;
6. teleport, transfer, death, respawn, and tracking loss reset the correct
   component tracks;
7. body/head/hand interpolation is finite, normalized, bounded, and
   topology-aware;
8. an interaction forced after a pose retains its ordering on integrated,
   TCP, WebSocket, and simulated datagram paths;
9. a slow or saturated observer cannot stall other clients or critical
   session traffic;
10. Mono, stereo per-eye, and XR multiview render the same remote embodiment
    facts.

Product evidence should include native and browser two-client motion captures
at representative 20/60 Hz report profiles, induced jitter/loss captures,
high-refresh displays, phone lifecycle transitions, and real desktop/Quest
headsets with head/hand tracking. Screenshots validate static representation;
motion traces or short captures are required for interpolation quality.

## Implementation Sequence

1. **Terminology and measurements.** Add remote-pose cadence, queue-age,
   smoothing-lag, and bandwidth diagnostics. Preserve today's wire and
   rendering while establishing a repeatable two-client baseline.
2. **Explicit cadence contract.** Replace/narrow ambiguous publication-rate
   configuration, make body report selection rate-aware and change-driven,
   make the heartbeat time-based, and force the final pose before lifecycle
   persistence.
3. **Sequenced body snapshots.** Add presentation epoch, pose sequence,
   bounded relative sample timing, and discontinuity to remote-player body
   updates. Keep them on the reliable stream initially.
4. **Buffered remote interpolation.** Give remote players a dedicated
   snapshot timeline and cadence/jitter-aware presentation delay. Do not
   change general entity smoothing accidentally.
5. **Loss-tolerant conformance.** Run the same logical sample stream through a
   simulator that injects loss, duplication, reordering, delay, and pressure.
   Decide full samples, component masks, quantization, keyframes, and recovery
   from measured results.
6. **Mixed-reliability transport — active Tactical 224.** Add capability and
   effective-profile negotiation, reliable fallback, and dependency-free
   native TCP-plus-UDP on desktop, Android, and Quest through the shared native
   adapter. Prove one logical pose stream over both profiles. WebTransport and
   WebRTC remain separate future carrier milestones.
7. **Component embodiment.** Add optional body/view separation, tracked head,
   and tracked hands; extend the shared actor/presentation/render contracts
   through Mono, per-eye XR, and multiview using the established pose lane.

Each stage is independently useful. In particular, cadence negotiation and
buffered interpolation are prerequisites for datagrams and improve the
TCP/WebSocket fallback. Do enough reliable-path work to prove the logical
contract, but do not treat high-cadence TCP tuning as the terminal quality
path.

## Accepted Decisions

- Remote-player presentation is separate from local movement authority.
- Local movement, pose reporting, server replication, and rendering remain
  independent clocks.
- Reports are change-driven, rate-capped, and heartbeat-based.
- Body/root is the required accepted location; head and hands are optional
  presentation components.
- Remote clients need a sequence-aware snapshot timeline rather than only a
  latest target plus fixed half-life.
- Critical lifecycle/gameplay facts remain reliable and ordered.
- Ephemeral pose semantics tolerate loss/reordering before the production
  datagram adapter is enabled.
- A mixed reliable-stream plus unreliable-datagram session is a supported
  dedicated-server and first-party-client target.
- Logical lanes and negotiated capabilities are the architecture; no one
  physical carrier is the common native/browser protocol.
- Dependency-free native TCP-plus-UDP is the first production mixed profile;
  TCP/WebSocket remains the reliable compatibility profile.
- WebTransport remains a future browser-capable client/server and
  Share-to-Browser carrier, while WebRTC remains the browser-hosted peer
  carrier.
- The candidate defaults are 60 Hz body reporting on mixed reliability and
  20 Hz on reliable compatibility, independently of 60 Hz local movement.
- WebRTC reliable and ephemeral data channels are the accepted carrier for
  browser-hosted peer rooms, not the primary dedicated-server transport.

## Open Decisions

- Default maximum body, tracked-pose, and server-replication rates after
  measurement.
- Fixed versus adaptive interpolation delay and the maximum extrapolation
  window.
- Exact body heading/view heading split for flat models.
- Aim versus grip hand transforms, quantization, and update thresholds.
- Whether ephemeral samples are always self-contained or use periodic
  keyframes plus deltas/component masks.
- Where server coalescing occurs relative to interest changes and per-observer
  budgets.
- Whether a future unified reliable-UDP/QUIC carrier provides enough measured
  benefit to replace native TCP-plus-UDP.
- Which Rust WebTransport implementation satisfies future
  Share-to-Browser/server-operation, maintenance, and binary-size gates.
- Certificate, port, reverse-proxy, and local-development ergonomics for
  self-hosted WebTransport.

## Code Map

- `native/crates/mclone-scene/src/pose_sync.rs`: current fixed 20 Hz local
  pose-publication deadline.
- `native/crates/mclone-client/src/player.rs`: local change selector,
  movement packet sequence, thresholds, and reminder count.
- `native/crates/mclone-protocol/src/lib.rs`: session configuration,
  `MovePlayer`, and current flat `RemotePlayerUpdate`.
- `native/crates/mclone-server/src/player.rs`: accepted client pose and
  teleport gate.
- `native/crates/mclone-server/src/remote_players.rs`: interest-aware
  remote-player add/update/remove routing.
- `native/crates/mclone-client/src/lib.rs`: latest remote-player replica state.
- `native/crates/mclone-client/src/actor.rs`: current fixed-half-life actor
  interpolation and derived walk animation.
- `native/crates/mclone-scene`: shared actor handoff and Mono/XR presentation.
- `native/crates/mclone-xr-host/src/actions.rs`: local OpenXR action and tracked
  pose collection.
- `native/crates/mclone-net` and browser worker adapters: current reliable
  native/WebSocket transports and future transport rims.
