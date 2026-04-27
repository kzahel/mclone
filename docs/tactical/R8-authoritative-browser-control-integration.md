# R8: Authoritative Browser Control And Camera Integration

This slice follows `R0` through `R7` in the runtime/host arc from [`README.md`](README.md). The host/client boundary already carries authoritative chunk snapshots, resumable remote sessions, player input commands, authoritative player-state snapshots, and polled server-originated updates. The next requirement is making the live browser path actually consume that authority instead of leaving the real camera/control loop renderer-owned.

## Scope

Add:

- a browser/debug camera path derived from authoritative `player_state`
- browser input translation into shared `set_player_input` commands
- scheduled `poll_world_updates` in the live browser debug loop
- chunk-view updates derived from authoritative player position rather than a renderer-owned free-cam position
- browser coverage proving a real Playwright client can move the remote authoritative player and keep chunk interest aligned

Do not add:

- vanilla-parity movement, collision, or rollback/replay-grade prediction
- push-capable transports such as WebSocket or WebRTC
- new gameplay/entity systems beyond the existing single authoritative player state
- renderer ownership of player/world state

## Reference shape vs divergence

Minecraft Java already keeps camera/gameplay authority on the client/server gameplay loop rather than inside the chunk renderer. That remains the baseline design input.

The divergence in this slice is narrower than a full Minecraft client loop:

1. browser debug input is still a temporary engine-native control layer
2. the authoritative player model is still the simple fly-style movement introduced in `R7`
3. server-originated updates still arrive through HTTP polling rather than a persistent push transport

Even with those simplifications, the ownership rule now matches the intended architecture:

- browser input becomes a client-runtime concern
- authoritative player state stays on the host
- the renderer only consumes the resulting camera/world state

## Landed shape

### Authoritative live browser camera

The debug browser path no longer owns gameplay state. Instead it:

- polls the world client for authoritative updates on a fixed cadence
- keeps a presentation-only predicted camera for immediate local view response
- reconciles that camera from `player_state` once the latest local input is acknowledged
- derives chunk-view requests from the predicted camera position for interactive use, while fixed validation shots can still preserve their initial camera

That means the renderer still treats host state as authority, while local presentation can move smoothly between authoritative snapshots.

### Browser input translated at the client boundary

`DebugInput` remains the browser input source, but it no longer owns movement state.

The new `debug-player-controls` helper:

- converts mouse/touch/keyboard input into `set_player_input`
- applies yaw/pitch deltas relative to the predicted presentation camera
- converts camera-relative movement into world-space `moveX` / `moveY` / `moveZ`
- supports injected input so browser tests can drive the live page without special renderer hooks

### Browser and remote-host validation

The browser validation matrix now includes a live debug path against the dedicated Node host:

- Playwright opens `/?mode=debug` against the remote host transport
- the test injects forward input through the debug runtime controller
- the authoritative player tick advances
- authoritative player position changes
- authoritative chunk-view state follows the moved player

The existing remote smoke remains useful for render/boot validation, while the new debug test proves the browser control loop itself is now consuming host authority.

## Why this shape

This cut closes the biggest remaining ownership hole in the runtime split without overcommitting on transport:

- the renderer is no longer the thing deciding where the player or chunk window is
- browser singleplayer and remote browser clients now use the same input/state authority model
- the real browser path exercises the `R7` protocol instead of only the smoke harness doing so

The main intentional limitation is explicit:

- the update path is still HTTP polling
- the player loop is still baseline authoritative fly movement
- this slice proves ownership, not final gameplay parity

That is the right place to pause before deciding whether a push-capable transport is actually necessary.

## Validation

- `pnpm typecheck`
- `pnpm test`
- `pnpm test:browser`
- inspected `/tmp/mclone-browser-smoke.png`
- inspected `/tmp/mclone-debug-free-cam.png`

## Next step

`R9`: measure the live browser control path under the current poll-based remote transport and, if it is no longer sufficient, add a push-capable transport adapter that reuses the same world host/client contracts instead of reopening the authority split.
