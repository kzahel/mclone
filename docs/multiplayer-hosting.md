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
| Persistence | filesystem chunk store (native), browser storage for browser singleplayer (future), export/import adapters (future) |

## Current Implementation

`mclone-dedicated-server` is a native Rust headless host with no renderer. It
builds directly on `mclone-server::IntegratedServer` and drives clients through
the same protocol/client/server boundary as singleplayer.

- **Default listener:** native TCP on `127.0.0.1:25565`.
- **`--listen-ws HOST:PORT`** adds a WebSocket listener that bridges each browser
  connection into the same server loop, so TCP and WebSocket clients share one
  world.
- The server is **launched with a seed** and currently uses a non-persistent
  chunk store (`NullChunkSnapshotStore`). There is no `--world-dir`, world id, or
  save yet — every launch regenerates from seed. (A `FilesystemSnapshotStore`
  exists in the engine but no app wires it; see persistence note below.)
- **`--multi-client-smoke`** runs the multi-client integration check (two clients
  sharing a world with remote-player replication).
- `PROTOCOL_VERSION` is negotiated in the handshake (see [`protocol.md`](./protocol.md)).

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
- **Web** has a complete WebSocket client, but it is not yet wired into the
  playable browser app (reachable only from a smoke export).

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

Persistence is the nearest concrete gap: the engine has the chunk store and
`IntegratedServer::with_chunk_store`, but the dedicated server needs a
`--world-dir`/config path to actually save and reload a world.

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
