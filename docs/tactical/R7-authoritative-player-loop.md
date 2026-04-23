# R7: Authoritative Player Loop And Polled Session Updates

This slice follows `R0` through `R6` in the runtime/host arc from [`README.md`](README.md). The renderer already consumes authoritative chunk snapshots, browser singleplayer already runs through a worker host, browser clients already talk to a dedicated Node host, and `R6` already hardened the shared session protocol with resumable sessions, versioned envelopes, and shared chunk-interest management per save. The next requirement is putting the first real player/session loop on top of that boundary.

## Scope

Add:

- shared player-input command and authoritative player-state snapshot shapes
- a queued `poll_world_updates` flow for server-originated updates beyond `set_chunk_view`
- first authoritative player ticking on both the local singleplayer host and the dedicated Node host
- remote HTTP endpoints and client transport support for player input plus polled updates
- integration coverage for local boundary player updates, remote player-state polling, and reconnect/resync of player state

Do not add:

- vanilla-parity movement/collision/physics
- entity systems beyond the first session-owned player state
- camera binding to authoritative player state
- WebSocket/WebRTC/WebTransport
- gameplay interactions beyond basic position/rotation intent

## Reference shape vs divergence

Minecraft Java has a server-authoritative player loop and a push-capable network protocol. That reference shape is still the baseline design input.

The divergence in this slice is narrow and intentional:

1. the gameplay authority remains on the host side
2. the client still sends explicit input commands and consumes authoritative player snapshots
3. the remote transport stays on HTTP for now, with a polled update queue instead of jumping to sockets before the higher-level gameplay protocol exists

That choice keeps the transport change proportional to the actual new requirement:

- we needed server-originated updates
- we did not yet need a fully general push transport

If later gameplay traffic shows that HTTP polling is the wrong fit, the message model is now ready to move without reopening the host/client authority split again.

## Landed shape

### Shared player protocol

The world protocol now includes:

- `set_player_input`
- `poll_world_updates`
- `player_state`

`player_state` currently carries:

- authoritative `playerId`
- position
- rotation
- acknowledged input sequence
- authoritative tick
- player-state revision

This is intentionally baseline session gameplay state, not a claim of Minecraft movement parity.

### Local and worker host support

`GeneratedWorldHost` now owns a first singleplayer player/session loop:

- open-world responses include `session_state` and `player_state`
- `set_player_input` stores the latest local player command
- `poll_world_updates` advances the local player loop and drains queued authoritative updates

That keeps the browser worker path and the in-process test/runtime path on the same message model instead of making player-state delivery remote-only.

### Dedicated host shared-session ticking

`GeneratedWorldRemoteService` now owns per-session player state alongside the shared authoritative world:

- each session gets its own authoritative player state and latest input command
- the dedicated host ticks those session players independently while still sharing one authoritative generated world per save
- server-originated `player_state` messages are queued per session and drained through `poll_world_updates`
- reconnect/resume replays both session metadata and current player state

### Remote transport shape

The remote HTTP boundary now adds:

- `POST /api/world/session/:id/player-input`
- `POST /api/world/session/:id/updates/poll`

`RemoteWorldTransport` stores the last world-open, chunk-view, and player-input commands so it can restore a dropped session and replay the latest authoritative session shape before continuing.

This keeps the remote path honest:

- there is now a server-originated update flow
- it is still pull-driven over HTTP
- we have not overcommitted to sockets before measuring real need

### Browser smoke usage

The browser smoke now does more than render chunks:

- it sends an authoritative player input command
- it polls for authoritative updates
- it verifies that the boot result includes player-state sequence/revision/tick metadata from the host

The rendered scene itself stays on the same camera path, so this slice validates the runtime boundary without changing the existing smoke image.

## Why this shape

This cut lands the smallest real gameplay loop that exercises the hardened `R6` boundary:

- the client sends input commands instead of owning state
- the host ticks player state and originates updates
- the dedicated host shares one world while maintaining per-session player authority
- browser and local-worker paths still reuse the same protocol and client application path

The main intentional limitation is also explicit:

- movement is still a simple baseline fly-style authoritative intent model
- camera binding and full gameplay parity are not in this slice
- the remote update flow is poll-based, not push-based

That is a better place to pause than inventing a fake socket layer or tying the renderer to a half-finished player simulation.

## Validation

- `pnpm typecheck`
- `pnpm test`
- `pnpm test:browser`
- inspected `/tmp/mclone-browser-smoke.png`

## Next step

`R8`: bind browser control/camera flow to the new authoritative player loop. Concretely: drive the debug/browser camera from `player_state`, translate browser input into `set_player_input`, schedule regular `poll_world_updates` in the live browser loop, and only after that decide whether the HTTP poll path needs a push-capable transport.
