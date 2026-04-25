# ClientRuntime1 - Client world replica hydration

Standing after [`ClientRuntime0-integrated-server-client-world-boundary.md`](ClientRuntime0-integrated-server-client-world-boundary.md), which introduced the first facade names for `IntegratedServer`, `ClientRuntime`, `ClientWorld`, and `PredictionService`.

## Goal

Make `ClientWorld` the single protocol-facing hydration target for visible/interested client facts:

- chunk snapshots and unloads
- light deltas
- session and local-player state
- entity snapshots
- revision facts needed by render, interpolation, and future prediction views

Singleplayer and remote clients must hydrate it through the same logical host messages. This is a boundary and ownership slice, not a gameplay or renderer-feature slice.

## Key Safeguard

Canonical generation from seed is host-only.

`ClientWorld` must never call `NoiseBasedChunkGenerator`, run decoration, or synthesize authoritative chunk contents from seed. If a chunk, light section, entity, or collision window is missing, the client replica should represent missing authority data until a host message arrives. Presentation-only placeholders are allowed, but they must not become client-world snapshots, collision truth, or revisioned facts.

Memory duplication is acceptable for the bounded client-interest replica. Duplicated canonical generation is not.

## Source Review

Re-read these vanilla files before moving hydration code:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java`

Vanilla hydrates `ClientLevel` and `ClientChunkCache` from server packets, even in singleplayer. `mclone` should preserve that ownership shape while using engine-native worker and protocol carriers.

## Current Code To Inspect

- `src/runtime/client/client-world.ts`
- `src/runtime/client/client-runtime.ts`
- `src/runtime/transport/local-world-transport.ts`
- `src/runtime/transport/worker-world-transport.ts`
- `src/runtime/transport/remote-world-transport.ts`
- `src/world/level/client-chunk-cache.ts`
- `src/renderer/chunk/render-world-worker.ts`
- `src/runtime/protocol/world-messages.ts`

## Scope

| # | Work | Expected result |
|---|---|---|
| 1 | Move host-message application behind `ClientWorld` methods | chunk, light, unload, session, player, entity, and perf updates enter through client-world hydration APIs |
| 2 | Keep transports as carriers | local worker and remote HTTP still return logical messages, but do not own replica mutation semantics |
| 3 | Preserve render-world ingestion | render-world worker still receives chunk/light/unload updates as derived render input |
| 4 | Expose explicit missing-data surfaces | prediction/collision and render views can distinguish missing host facts from empty chunks |
| 5 | Preserve bounded interest | client-world contents remain scoped to host-published interest, not a whole-world cache |
| 6 | Add focused tests | cover singleplayer/remote message application through the same client-world path where practical |

## Do Not Add

- movement physics, correction smoothing, or new command semantics
- NPC AI or client-side spawning authority
- WebSocket/WebRTC/WebTransport
- fluid prediction
- client-side worldgen, decoration, or seed fallback for missing chunks
- broad renderer rewrites

## Validation

- `pnpm typecheck`
- focused unit tests for `ClientWorld` hydration if code changes make them useful
- `pnpm test:browser` if browser bootstrap or render-world message flow changes
- `git diff --check`

This slice affects ownership, not pixels. Run a browser screenshot probe only if the implementation changes rendered output or mesh invalidation behavior.

## Done When

- `ClientRuntime` applies host messages by hydrating `ClientWorld`.
- Local worker and remote HTTP clients use the same client-world hydration path.
- `ClientWorld` has no import path to host worldgen or decoration code.
- Missing chunks/collision facts remain explicit instead of being locally generated.
- Render-world and prediction views consume bounded client-world facts rather than host internals.

## Next Step

`ClientRuntime2-presentation-thread-boundary.md`: make UI/render consume compact presentation state and render-world mesh handles without owning raw client-world internals.
