# Multiplayer Hosting

How clients, hosts, transports, and storage are arranged. This is about runtime
packaging and connection modes, not worldgen parity — the simulation rules live
in the translated worldgen and host runtime. The logical messages are in
[`protocol.md`](./protocol.md); per-platform connect status is in
[`topics/platform-parity.md`](./topics/platform-parity.md).

> This doc originally described the retired TypeScript/Node hosting model
> (`pnpm host:dedicated`, a Deno harness, HTTP/query-param joins, a JSON server
> config). That is gone. The dedicated server is now the Rust
> `mclone-dedicated-server`. The "server config / world ids / asset hosting"
> shape under **Target Shape** is still aspirational and labeled as such.

## Goals

- Let the player choose local singleplayer or a server-hosted world with the
  same client.
- Keep dedicated server setup close to "run a server, clients join it."
- Keep WebSocket and TCP as compatibility transports while adding the accepted
  mixed-reliability paths.
- Support browser-hosted peer rooms without creating another world authority.
- Keep world authority, transport carrier, and asset hosting orthogonal.

## Non-Goals

- Do not let the renderer or a client generate authoritative chunks for a
  dedicated world.
- Do not require WebRTC for ordinary dedicated-server or native-host sessions.
- Do not tie hosting to a specific platform app shell.

## Separate Axes

Keep these independent. The logical host/client protocol does not care which
combination is used — a chunk snapshot or movement command means the same thing
over every carrier.

| Axis | Options |
|---|---|
| World authority | local integrated host (native runner or browser worker), native dedicated server |
| Transport carrier | in-process `LocalTransport`, worker channel, native TCP, WebSocket, WebTransport, browser WebRTC data channels |
| Asset hosting | local packed assets, dedicated-server static hosting (future), CDN/static host |
| Persistence | SQLite world store (native/Android), IndexedDB world store (browser singleplayer), export/import adapters (future) |

## Current Implementation

`mclone-dedicated-server` is a Rust headless host with no renderer. It
builds directly on `mclone-server::RealmServer` and drives clients through
the same protocol/client/server boundary as singleplayer.

- **Default listener:** native TCP on `127.0.0.1:25565`.
- **`--listen-ws HOST:PORT`** adds a WebSocket listener whose peers terminate
  directly in the same authoritative connection registry as TCP; there is no
  TCP loopback bridge.
- The host advances autonomously at the shared default 20/20/60 cadence,
  drains ready commands at host boundaries, and publishes each player's
  ordered stream through an independent bounded writer. Client traffic does
  not clock world time, worldgen publication, entity tracking, or autosave.
- Without a world argument, or with `--transient`, the server uses transient
  storage. `--world-dir PATH` opens a persistent SQLite-backed world;
  `--world-root ROOT --world-name NAME` selects a named directory. Dirty chunks,
  players, and typed world metadata autosave every 6000 gameplay ticks and save
  again during graceful shutdown. The first open stores the seed,
  generation/behavior profiles, target, clocks, and daylight rule; later opens
  validate supplied launch facts before generation and reject mismatches.
- Every peer has a nonblocking 64-publication-frame / 64 MiB exact-byte
  outbound queue. A saturated slow consumer is disconnected without delaying
  the authority loop; summary markers report queue high-water marks and
  pressure disconnects.
- **`--multi-client-smoke`** runs the multi-client integration check (two clients
  sharing a world with remote-player replication).
- `PROTOCOL_VERSION = 33` is negotiated with strict equality. The handshake
  also carries the client's unauthenticated stable local UUID/display name;
  persistent realms save pose, selected slot, XP, typed statistics, health,
  and a pending typed death cause under that UUID. Ordered owner life updates
  and explicit respawn use the same TCP/WebSocket streams. Time updates carry
  durable game/day clocks plus daylight running state (see
  [`protocol.md`](./protocol.md)).
- Browser singleplayer uses IndexedDB v6 with catalog, chunk, entity, player,
  and singleton world-metadata stores. Metadata is preloaded and validated
  before generation; background lifecycle and graceful worker shutdown queue
  the same completion-driven flush used by other records.

Client connect status (full grid in
[`topics/platform-parity.md`](./topics/platform-parity.md)):

- **Desktop flat + OpenXR** connect to a dedicated server over TCP via
  `--remote-addr HOST:PORT`.
- **Flat Android** can connect over TCP through the Android-owned
  `debug.mclone.remote_addr` property. There is not yet an in-app connect UI.
- **Android XR** can connect over TCP through launch-scoped
  `mclone.startup.argv` (`--remote-addr HOST:PORT`) and the shared XR scene
  runtime. The validator can start a local `mclone-dedicated-server` with
  `--start-server`. Quest remote smokes passed over direct LAN and through
  `adb reverse`; direct LAN still needs a headset-reachable host address and
  host firewall allow.
- **Web** can connect over WebSocket by launching the browser app with
  `?remoteWsUrl=ws://HOST:PORT`. `native:web:remote-smoke` starts a native
  dedicated WebSocket server, loads the normal playable app through that URL,
  and validates movement, block interaction, rendering, native UI, and canvas
  pixels. There is not yet an in-app connect UI.

Making remote-dedicated play reachable from every client is a tracked invariant,
not a desktop-only feature.

## Target Shape (aspirational)

The dedicated server should eventually be configured by a file, not by client
parameters — browser clients join a server-owned world, they do not redefine the
server seed. A future config would carry host/port, a save root, named worlds
(id → seed/preset/config/storage), and enabled transports, with persistence
wired so a world survives restarts. Optional single-process deployment could let
the dedicated server also serve the built web client and asset pack alongside the
WebSocket endpoint. Graceful, admin-only config reload/restart is a later server
lifecycle feature, not a client debug shortcut.

The next hosting lifecycle gaps are broader player state such as inventory,
authenticated login/capability negotiation, keepalive/timeouts, and explicit
disconnect reasons.
Those belong in the shared session/persistence layers rather than app-specific
TCP or WebSocket wrappers.

## Browser-Hosted Peer Rooms / WebRTC

Browser-hosted rooms are an accepted topology. The host browser runs the same
Rust `RealmServer` and IndexedDB persistence shape as singleplayer; guests
connect to it over WebRTC data channels. A small room-key signaling service
exchanges offers, answers, ICE candidates, and room metadata. It does not own
world state or carry gameplay after a direct connection succeeds.

Each guest connection uses two logical lanes:

- a reliable ordered data channel for session, world, gameplay, and
  cross-lane barrier facts;
- an unordered data channel with zero retransmissions for disposable,
  sequenced pose samples.

ICE may establish a direct peer path with the help of STUN. Restrictive NAT or
firewall combinations require TURN; in that case the TURN service relays
gameplay traffic and is more than a lightweight signaling service. The product
must expose whether a session is direct or relayed rather than promising that
room-key joins are always serverless.

The browser still cannot listen for inbound WebTransport. That limitation does
not prevent it from hosting authority through WebRTC. WebTransport is one
future browser-capable client/server carrier, while native clients can use a
TCP-plus-UDP profile and compatibility clients can use TCP or WebSocket.
Browser-hosted rooms use the browser's WebRTC stack and reuse the same logical
protocol and authority.
See
[`topics/browser-hosted-peer-sessions.md`](./topics/browser-hosted-peer-sessions.md)
for the ownership, lifecycle, and implementation contract.

## Guardrails

- Keep authority, transport, and asset hosting orthogonal.
- Keep TCP and WebSocket as compatibility transports; select the best
  negotiated ephemeral carrier independently. Start with dependency-free
  native UDP, retain WebTransport as a future browser-capable client/server
  carrier, and use WebRTC for browser-hosted peer rooms.
- Keep dedicated world configuration server-owned.
- Reuse the integrated-host authority in browser rooms; WebRTC changes the
  carrier, not the simulation or persistence owner.
- Keep any future reload/restart admin-only and testable.
