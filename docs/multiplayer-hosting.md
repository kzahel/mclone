# Multiplayer Hosting Architecture

This document sketches the desired multiplayer and hosting shape for `mclone`. It is about runtime packaging and connection modes, not Minecraft worldgen parity. The simulation/content rules still live in the translated worldgen and host runtime; this document only decides how clients, hosts, transports, and assets are arranged.

## Goals

- Let the player choose local browser singleplayer or a server-hosted world.
- Keep dedicated server setup clear enough to resemble Minecraft's "run a server with a config and saves" model.
- Keep the default remote path WebSocket-backed and reliable.
- Leave a clean path to true browser-hosted peer-to-peer multiplayer from a fully static client deployment.
- Avoid a brittle proxy architecture. Separate services on separate ports are acceptable and preferred for development.
- Let a dedicated server optionally host the built browser client and asset pack for one-process deployment.

## Non-Goals

- Do not make the renderer or browser client generate authoritative chunks for a dedicated world.
- Do not require WebRTC before WebSocket remote play is mature.
- Do not require a dev proxy that combines Vite HMR, WebSocket world traffic, future WebRTC signaling, and static assets behind one clever router.
- Do not make Deno a persistent dedicated-server target. Deno remains a headless validation lane unless that target is explicitly revisited.

## Separate Axes

Keep these as separate concepts:

| Axis | Options |
|---|---|
| World authority | browser integrated worker, dedicated Node server, future browser P2P host |
| Transport carrier | `postMessage`, WebSocket, future WebRTC data channels |
| Asset hosting | Vite dev server, dedicated server static `dist/`, CDN/static host such as GitHub Pages |
| Persistence | IndexedDB for browser singleplayer, file storage for dedicated Node, future export/import adapters |

The logical host/client protocol should not care which combination is used. A chunk snapshot, player input command, session update, or player-state snapshot should mean the same thing over `postMessage`, WebSocket, or future WebRTC.

## Target Modes

### Browser Singleplayer

Default browser mode:

- the browser main thread owns input, GUI, presentation, and WebGPU submission
- a browser worker owns the authoritative local host
- render-world and mesh workers consume replicated chunk/light/entity facts
- IndexedDB is the default persistence adapter

This is a local integrated server. It is not a renderer shortcut. The browser UI may choose a seed and preset because it is creating the local world authority.

Current query shape:

```text
/?mode=debug
/?mode=debug&worldTransport=worker
```

Supported clearer aliases:

```text
/?mode=debug&worldAuthority=local
```

### Dedicated Server

Dedicated mode is a separate Node process that owns world authority and persistence.

Near-term runtime:

- `pnpm host:dedicated` should be the user-facing script name
- `pnpm host:remote` can remain as a compatibility alias while docs/tests migrate
- WebSocket is the default remote transport
- the browser connects only when explicitly configured for dedicated authority

Current query shape:

```text
/?mode=debug&worldTransport=remote&worldHostUrl=http://127.0.0.1:4173
```

Supported dedicated query aliases:

```text
/?server=https://mclone-host.graehlarts.com
/?mode=debug&worldAuthority=dedicated&dedicatedSocketUrl=ws://127.0.0.1:4173/api/world/socket&netTransport=websocket
/?mode=debug&startWorld=1&worldAuthority=dedicated&dedicatedSocketUrl=127.0.0.1:4173&netTransport=websocket
```

The short `server=` form is the mobile/shareable join URL. It implies `worldAuthority=dedicated`, defaults `netTransport` to WebSocket, maps `https://` to `wss://`, appends `/api/world/socket` when no socket path is provided, and auto-starts the world unless `startWorld=0` or `autoStartWorld=0` is explicitly present.

The old `worldTransport=remote` and `worldHostUrl` names keep working until all tests, docs, and saved localStorage state are migrated. `dedicatedHostUrl=http://...` is accepted as a compatibility spelling, but the user-facing dedicated gameplay transport is WebSocket. HTTP endpoints on the dedicated process are for health, static assets, and compatibility/testing surfaces.

The GPU Debug Settings screen exposes the same choice as a Local Singleplayer/Dedicated Server selector plus a dedicated socket URL field. Those GUI values are persisted locally; direct query params still override the stored authority and URL for shareable links and tests.

### Dedicated Server Config

The dedicated server should be configured by a file, not mainly by browser query params. Browser clients join a server-owned world; they do not casually redefine the server seed.

Current config support covers:

- `host`
- `port`
- `saveRoot`

Target config shape:

```json
{
  "schemaVersion": 1,
  "host": "127.0.0.1",
  "port": 4173,
  "saveRoot": "./worlds",
  "staticRoot": "./dist",
  "defaultWorld": "main",
  "worlds": [
    {
      "id": "main",
      "seed": "12345",
      "preset": "default",
      "config": {
        "lightingMode": "vanilla17",
        "liquidSimulationMode": "vanilla17"
      }
    }
  ],
  "transports": {
    "websocket": true,
    "webrtc": false
  }
}
```

Early implementation can keep the existing `open_world` message as the client request shape, but the dedicated-server direction should move toward server-owned world ids:

- clients request `defaultWorld` or a named `worldId`
- the server maps that id to seed/preset/config/storage
- optional world creation is an explicit server policy, not an accidental side effect of arbitrary client params

### Dedicated Server Asset Hosting

For production or LAN use, the dedicated server may serve the built browser client:

```text
pnpm native:web:bundle
cargo run --manifest-path native/Cargo.toml -p mclone-dedicated-server -- --config server.json
```

If `staticRoot` is present, the server can serve `dist/` and the asset pack alongside `/api/world/socket` and health endpoints. This is optional. Dev should still support separate ports:

```text
cargo run --manifest-path native/Cargo.toml -p mclone-dedicated-server -- --config /tmp/mclone-server.json
pnpm native:web:serve
```

A convenience script such as `pnpm dev:server+browser` should spawn those two processes and print a ready-to-open URL. It should not be a proxy.

### Future Static P2P

Static P2P should remain possible:

- the browser app can be hosted from GitHub Pages, R2, or any static host
- one browser becomes the authority by running the same integrated-server worker shape used by singleplayer
- guests connect to the host over WebRTC data channels
- a public or private signaling service only exchanges SDP/ICE/room metadata
- the signaling service does not own world state, chunks, entities, or assets

This is a future authority option, not a replacement for dedicated servers.

Important constraint: P2P should reuse the same logical host/client messages as local and dedicated play. The data carrier can be WebRTC, but gameplay authority should still be a `WorldHost`-like runtime.

### Future Dedicated WebRTC

Dedicated WebRTC should be deferred until the gameplay protocol needs unreliable or unordered lanes, such as high-rate entity/player snapshots.

Expected shape:

- WebSocket or HTTP endpoint for signaling
- WebRTC data channels for game traffic
- reliable ordered channel for world sync, chunks, config, chat, and commands
- optional unordered/unreliable channel for replaceable high-rate snapshots
- STUN/TURN configured separately from the game server

In Node, WebRTC usually means a native WebRTC binding such as `node-datachannel` or an equivalent. The application does not simply bind one stable "game UDP port" the way a hand-rolled UDP protocol would. ICE may use local UDP sockets, reflexive candidates, or TURN relays. TURN, if needed, is a separate service with its own ports and operational concerns.

## Dev Entrypoints

Desired scripts:

```json
{
  "native:web:serve": "node ./native/apps/mclone-web-client/scripts/browser-smoke.mjs --serve --app-loop",
  "native:dedicated:smoke": "cargo run --manifest-path native/Cargo.toml -p mclone-dedicated-server -- --multi-client-smoke",
  "dev:server+browser": "node ./scripts/dev-server-browser.mjs"
}
```

`dev:server+browser` should:

- start the dedicated server on its configured port
- start the native web server on its own port
- print a local dedicated-client URL
- shut both children down on exit
- avoid proxying traffic between them

## Runtime Reload And Admin Control

Reload support is useful but should be explicit and admin-controlled.

Deferred target:

- `--watch-config` mode watches the config file and marks the server config dirty
- health/admin status reports whether a reload is pending
- an authenticated admin command can request graceful reload
- graceful reload flushes persistence, stops accepting new joins briefly, closes/restarts affected worlds or restarts the process, then allows clients to reconnect/resume
- clients can show "server restarting" or "config reload pending" state, but ordinary clients must not be able to restart the server without an admin capability

This should be designed as a server lifecycle feature. Do not hide it in a browser-only debug shortcut.

## Deno Role

Deno is currently a validation/runtime harness:

- headless WebGPU smoke
- generated-world smoke
- render-world worker parity checks

It is useful because it exercises browser-free worker and renderer-host shapes. It is not the persistent dedicated server. Persistent server work should target Node unless the project deliberately adds a Deno server target later.

## Guardrails

- Keep authority, transport, and asset hosting orthogonal.
- Keep WebSocket as the default dedicated remote transport until measurements justify WebRTC.
- Keep old query params working while adding clearer names.
- Keep dedicated world configuration server-owned.
- Keep P2P static deployment possible by reusing the integrated-server worker authority in a browser host.
- Keep reload/restart admin-only and testable.
