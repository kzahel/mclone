# R6: Protocol Hardening And Shared Dedicated-Host Authority

This slice follows `R0` through `R5` in the runtime/host arc from [`README.md`](README.md). The authoritative world boundary already exists, browser singleplayer already runs behind a worker host, browser chunk meshing already lives behind a client worker boundary, browser and Node persistence both sit behind the same storage contracts, and browser clients can already talk to a dedicated Node host over a remote HTTP transport. The next requirement is making that boundary robust enough that real shared sessions and future gameplay can accumulate on top of it.

## Scope

Add:

- shared session/player baseline message shapes on the existing world host/client protocol
- protocol versioning and structured remote transport error codes
- reconnect/resume support for remote sessions
- shared authoritative generated-world ownership per save on the dedicated Node host
- per-session chunk-interest tracking with aggregate authoritative chunk views
- integration coverage for two-client shared-world use, resume/resync, and unknown-session recovery

Do not add:

- a real gameplay tick loop
- client input commands beyond the existing world/chunk-view requests
- push-driven network transports such as WebSocket or WebRTC
- dedicated-host worker-thread pools
- entity or movement simulation

## Reference shape vs divergence

Minecraft Java already has long-lived remote sessions, per-player server authority, and a real push-driven protocol. The guidance from [`../../AGENTS.md`](../../AGENTS.md) is still the right one:

1. simulation/content parity remains the default
2. runtime orchestration may diverge for browser / worker / Node constraints
3. those divergences must preserve a clear path for future parity work

This slice keeps the divergence narrow.

We still do not have player movement, ticking, or unsolicited world updates. Because of that, the remote transport stays on the existing HTTP request/response session model for now. The hardening work happens in the shared protocol and host/session semantics rather than prematurely forcing a persistent socket transport before the higher-level update model exists.

## Landed shape

### Shared session-aware protocol

The runtime protocol now includes baseline session state:

- `session_state` messages carry `sessionId`, `playerId`, `playerProfile`, `saveId`, `revision`, `resumed`, and the current chunk-view state
- `TransportWorldClient` stores that state alongside the read-only client chunk cache
- browser boot results expose the active session/player ids in the smoke path

The HTTP transport is now explicitly versioned and structured:

- every open-world and chunk-view request/response carries `protocolVersion`
- remote errors now use stable error codes such as `unknown_session`, `world_request_mismatch`, and `protocol_version_mismatch`
- remote transport validation fails fast if the host speaks a different protocol version

### Shared dedicated-host authority

`GeneratedWorldRemoteService` no longer creates one authoritative generated-world host per remote session.

Instead it now:

- shares one `GeneratedWorldHost` per generated save id
- deduplicates concurrent `open_world` requests for the same save
- serializes stateful host mutations per save
- tracks per-session chunk-view state and per-session visible-chunk sets
- computes an aggregate authoritative chunk view covering all active remote sessions for that save
- serves each session only the chunk snapshots/unloads it should see

That is the first real move from “many isolated remote sessions” toward “one shared authoritative host with many clients.”

### Reconnect and resync

The remote path now supports resumable sessions:

- `open_world` accepts `resumeSessionId`
- a resumed session receives `world_opened`, `session_state(resumed: true)`, and its current visible chunks
- `RemoteWorldTransport` automatically reopens the world and replays `set_chunk_view` if the host reports `unknown_session`

This keeps the browser client on the same host/client boundary without forcing renderer code to know about remote-session lifecycle details.

### Validation coverage

The runtime test matrix now covers:

- single-client remote chunk streaming
- two concurrent remote clients sharing one authoritative generated world
- explicit session resume and chunk resync
- automatic reopen/resync after server-side session loss

The browser smoke still renders through the same remote Node host path and now verifies session metadata as part of boot.

## Why this shape

This cut hardens the existing boundary without pretending multiplayer gameplay is already finished:

- browser singleplayer, browser remote clients, and the dedicated Node host still share the same world host/client contracts
- the renderer still consumes read-only authoritative snapshots instead of owning world state
- remote reconnect and protocol mismatch handling now fail in predictable ways
- the dedicated host now owns real shared chunk-interest state instead of hiding behind one-host-per-session fanout

The main intentional limitation is unchanged:

- the remote protocol still only has explicit request/response world commands
- there is still no authoritative player movement/input/tick stream
- a push-capable transport should wait until there are actual server-initiated updates to deliver

## Validation

- `pnpm typecheck`
- `pnpm test`
- `pnpm test:browser`
- inspected `/tmp/mclone-browser-smoke.png`

## Next step

`R7`: add the first authoritative player/tick loop on top of this hardened boundary. Concretely: introduce player input commands and authoritative player-state snapshots, add a transport/update path that can carry server-initiated state changes instead of only request/response chunk-view sync, and make the dedicated host tick shared session state rather than only serving chunk snapshots on demand.
