# Multiplayer Hosting

How clients, hosts, transports, and storage are arranged. This is about runtime
packaging and connection modes, not worldgen parity — the simulation rules live
in the translated worldgen and host runtime. The logical messages are in
[`protocol.md`](./protocol.md); per-platform connect status is in
[`topics/platform-parity.md`](./topics/platform-parity.md).

> This doc originally described the retired TypeScript/Node hosting model
> (`pnpm host:dedicated`, a Deno harness, HTTP/query-param joins, a JSON server
> config). That is gone. The dedicated server is now the native Rust
> `mclone-dedicated-server`. The "server config / world ids / asset hosting"
> shape under **Target Shape** is still aspirational and labeled as such.

## Goals

- Let the player choose local singleplayer or a server-hosted world with the
  same client.
- Keep dedicated server setup close to "run a server, clients join it."
- Keep WebSocket the default browser remote transport and TCP the native one.
- Leave a clean path to future browser-hosted peer-to-peer multiplayer.
- Keep world authority, transport carrier, and asset hosting orthogonal.

## Non-Goals

- Do not let the renderer or a client generate authoritative chunks for a
  dedicated world.
- Do not require WebRTC before WebSocket/TCP remote play is mature.
- Do not tie hosting to a specific platform app shell.

## Separate Axes

Keep these independent. The logical host/client protocol does not care which
combination is used — a chunk snapshot or movement command means the same thing
over every carrier.

| Axis | Options |
|---|---|
| World authority | local integrated host (native runner or browser worker), native dedicated server, future browser P2P host |
| Transport carrier | in-process `LocalTransport`, worker channel, native TCP, WebSocket, future WebRTC data channels |
| Asset hosting | local packed assets, dedicated-server static hosting (future), CDN/static host |
| Persistence | SQLite world store (native/Android), IndexedDB world store (browser singleplayer), export/import adapters (future) |

## Current Implementation

`mclone-dedicated-server` is a native Rust headless host with no renderer. It
builds directly on `mclone-server::IntegratedServer` and drives clients through
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
- `PROTOCOL_VERSION = 23` is negotiated with strict equality. The handshake
  also carries the client's unauthenticated stable local UUID/display name;
  persistent worlds save pose, selected slot, and XP under that UUID. Time
  updates carry durable game/day clocks plus daylight running state (see
  [`protocol.md`](./protocol.md)).
- Browser singleplayer uses IndexedDB v5 with catalog, chunk, entity, player,
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

## Future Static P2P / WebRTC

Static P2P should stay possible: the web app can be hosted from any static host,
one browser becomes authority by running the same integrated-host shape used for
singleplayer, and guests connect over WebRTC data channels with a signaling
service that only exchanges SDP/ICE/room metadata and never owns world state.
Dedicated WebRTC should wait until the gameplay protocol needs unreliable or
unordered lanes (e.g. high-rate entity/player snapshots). Either way, P2P and
WebRTC must reuse the same logical host/client messages — only the carrier
changes.

## Guardrails

- Keep authority, transport, and asset hosting orthogonal.
- Keep TCP (native) and WebSocket (browser) as the default remote transports
  until measurements justify WebRTC.
- Keep dedicated world configuration server-owned.
- Keep P2P static deployment possible by reusing the integrated-host authority
  in a browser host.
- Keep any future reload/restart admin-only and testable.
