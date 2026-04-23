# R5: Remote Browser Transport To Dedicated Host

This slice follows `R0` through `R4` in the runtime/host arc from [`README.md`](README.md). The authoritative host boundary already exists, browser singleplayer already runs through a worker, browser chunk meshing already lives behind a client worker boundary, browser and Node persistence both sit behind the same storage contracts, and the same generated-world host/runtime core now boots in Node. The next requirement is proving that browser clients can consume that Node-hosted authority over a real remote transport.

## Scope

Add:

- shared HTTP-safe serialization for the existing world host/client messages
- a remote browser transport adapter behind `WorldTransport`
- a dedicated Node HTTP host bootstrap with remote session management
- browser runtime selection between local worker transport and remote transport
- a two-client browser smoke proving two pages can render against one dedicated Node host process

Do not add:

- reconnect/resync/version-hardening work beyond what the current request/response transport needs
- shared chunk-interest management across multiple remote clients
- player/session gameplay state sync
- WebRTC
- a push-driven real-time transport layer

## Reference shape vs divergence

Minecraft Java has a dedicated server and a real network protocol. The architectural target from [`../../AGENTS.md`](../../AGENTS.md) is still correct:

1. simulation/content parity remains the default
2. runtime orchestration may diverge for browser / worker / Node constraints
3. those divergences must preserve a clear path for future parity work

This slice diverges in the smallest way that fits the current protocol surface.

The earlier architecture notes described WebSocket as the likely remote transport. That remains a plausible later shape, but the actual host/client protocol we have today only supports explicit request/response commands:

- `open_world`
- `set_chunk_view`

There are no unsolicited server pushes, player inputs, ticks, or reconnect semantics yet. For that current scope, HTTP JSON request/response is the narrowest honest transport. It reuses the same message model, proves the browser-to-Node boundary, and avoids pretending we already need a persistent socket transport before the higher-level protocol exists.

## Landed shape

### Shared wire serialization

Added `src/runtime/protocol/world-http-protocol.ts`, which:

- serializes `open_world` requests with bigint-safe seed strings
- keeps host message payloads in the same engine-native logical shape
- defines the HTTP session/open and chunk-view response envelopes used by both browser and Node

### Remote browser transport

Added `RemoteWorldTransport` under `src/runtime/transport/`, plus `RemoteWorldClient`, which:

- opens a remote world session over HTTP
- stores the returned session id locally
- sends later `set_chunk_view` requests back to that dedicated host session
- reuses the same `TransportWorldClient` chunk-cache application path as local worker transport

### Dedicated Node HTTP host

Added `src/runtime/node/generated-world-http-server.ts`, which:

- boots a dedicated Node host process
- reuses `createGeneratedWorldHostForRequest(...)`
- keeps `FileWorldStorage` behind the same persistence interfaces from `R3` and `R4`
- creates one authoritative generated-world host per remote client session
- exposes HTTP endpoints for `open_world`, `set_chunk_view`, and health checks

The repo now exposes that path through:

```bash
pnpm host:remote -- --host 127.0.0.1 --port 4173 --save-root /tmp/mclone-node-worlds
```

### Browser smoke path

The browser renderer boot path now supports runtime transport selection:

- default path: local worker host
- remote path: `?worldTransport=remote&worldHostUrl=http://127.0.0.1:4173`

The Playwright smoke now starts both the Vite browser server and the dedicated Node host, opens two browser pages against the remote transport, and verifies both pages render the same remote save id while still using worker-backed chunk meshing on the client.

## Why this shape

This cut reaches the first real browser-to-Node boundary without overclaiming multiplayer completeness:

- browser and Node now share the same authoritative message model
- renderer still consumes authoritative snapshots instead of owning worldgen
- transport changed without changing simulation/runtime core logic
- the two-client smoke proves one dedicated Node process can serve multiple browser sessions

The important limitation is also intentional:

- the current generated-world host is still single-view oriented
- the dedicated Node host therefore creates one authoritative host instance per remote client session today
- `R6` is where shared interest management, reconnect/resync, and baseline player/session state need to land

That is a better shape than hiding those missing pieces behind a premature “multiplayer server” claim.

## Validation

- `pnpm typecheck`
- `pnpm test`
- `pnpm test:browser`
- inspected `/tmp/mclone-browser-smoke.png`

## Next step

`R6`: harden the protocol and session model. Concretely: reconnect/resync rules, chunk-interest management, baseline player/session state sync, and the first transition away from one-host-per-remote-session toward real shared authority.
