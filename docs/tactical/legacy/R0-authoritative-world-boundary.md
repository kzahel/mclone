# R0 — Authoritative world boundary

Standing before the rest of the runtime/host arc in [`README.md`](README.md). This slice is **infrastructure only**: establish a real client/server boundary inside the current browser build before moving any work to a worker or introducing persistence/network transports. It deliberately does **not** add a browser worker yet — that is `R1`. It also does **not** add WebRTC; remote/browser-hosted transports stay deferred until the dedicated-host path is real.

## Goal

Replace the current renderer-owned generated-world path with an explicit authoritative world host boundary that can be consumed locally first and transported later.

At the end of `R0`, the browser smoke should still render generated terrain, but the render path should no longer call worldgen directly. Instead it should consume a read-only client view backed by:

- shared client/server message and snapshot shapes
- a `WorldHost` / `WorldClient` boundary
- a local in-process transport adapter
- an authoritative in-process world host that owns chunk generation and chunk residency

This is the smallest step that:

- starts matching the reference Minecraft client/server ownership split
- removes the renderer's direct dependency on `NoiseBasedChunkGenerator`
- keeps the change debuggable before a worker boundary is added
- preserves a clean path to `R1` browser singleplayer workers, `R4` dedicated Node hosts, and later optional WebRTC transports

## Current status

- `src/renderer/main.ts` calls `scene.level.ensureChunksForCamera(...)` directly before rendering the frame.
- `GeneratedRenderLevel` is both the authoritative world owner and the renderer-facing chunk cache.
- `GeneratedRenderLevel.getChunk(...)` synchronously runs `fillFromNoise(...)`, `buildSurfaceAndBedrock(...)`, `applyCarvers(...)`, and `applyBiomeDecoration(...)`.
- There is no shared client/server message model, no transport abstraction, and no dedicated client-side chunk cache abstraction.
- Browser singleplayer, multiplayer clients, and a headless Node host therefore cannot reuse the same runtime boundary today.

## Source files (read before writing)

### Reference Minecraft source

| Java source | Why it matters |
|---|---|
| `reference/.../src/net/minecraft/client/Minecraft.java` (`singleplayerServer = MinecraftServer.spin(...)`) | browser singleplayer should conceptually behave like a local client talking to a local authoritative host |
| `reference/.../src/net/minecraft/client/server/IntegratedServer.java` | local-authority/server ownership model |
| `reference/.../src/net/minecraft/server/MinecraftServer.java` (`Server thread`) | authority is off the render/client thread in vanilla |
| `reference/.../src/net/minecraft/server/level/ServerChunkCache.java` | authoritative chunk ownership and loading |
| `reference/.../src/net/minecraft/client/multiplayer/ClientChunkCache.java` | client-side chunk cache as a consumer, not an owner, of chunk data |
| `reference/.../src/net/minecraft/client/renderer/LevelRenderer.java` | renderer consumes chunk state; it does not generate terrain |

### Current TS code

| Current TS source | R0 relevance |
|---|---|
| `src/world/level/generated-render-level.ts` | current browser-only combined authority + client-cache bridge to split apart |
| `src/worldgen/levelgen/noise-based-chunk-generator.ts` | authoritative chunk generation stays here, but moves behind a host boundary |
| `src/renderer/scene-setup.ts` | current scene bootstrap wires renderer directly to generated-world ownership |
| `src/renderer/main.ts` | current smoke path triggers chunk generation from the render flow |
| `src/renderer/level-renderer.ts` | should become a consumer of client-side chunk state only |
| `src/world/level/chunk/level-chunk.ts` | existing runtime chunk representation to reuse on the client side where practical |

## Scope

| # | Module | Depends on | Expected result |
|---|---|---|---|
| 1 | Shared protocol/message shapes | none | explicit client→host commands and host→client chunk/update messages with serializable payloads |
| 2 | `WorldHost` / `WorldClient` contracts | item 1 | clear authority boundary independent of transport |
| 3 | Local in-process transport adapter | items 1–2 | singleplayer can exercise the same boundary without workers or sockets yet |
| 4 | Authoritative in-process generated-world host | items 1–3, existing generator | host owns chunk generation/residency and publishes chunk snapshots |
| 5 | Read-only client chunk cache | items 1–4 | renderer consumes chunk snapshots from a client-facing cache instead of generating chunks itself |
| 6 | Browser smoke path conversion | items 1–5 | generated scene still renders, but render code no longer imports or drives worldgen directly |

## Why this subset

Workerizing the current code immediately would be the wrong first move.

If we move today's `GeneratedRenderLevel` straight into a worker, we preserve the wrong ownership model:

- renderer-shaped world APIs
- browser-singleplayer-specific shortcuts
- no reusable local/remote protocol
- no clean path to a headless Node host

`R0` intentionally stops one layer earlier:

- define the boundary
- keep the first transport local and in-process
- preserve current behavior
- then move that boundary into a worker in `R1`

This is the same principle as the reference Minecraft source:

- the client renders and consumes chunk state
- the authoritative host/server owns chunk generation and residency

The exact runtime mechanism diverges, but the ownership split should not.

## Intentional divergence

This slice is a bounded runtime-architecture divergence from vanilla, and it should be treated as such.

What vanilla does:

- integrated singleplayer stands up an actual server
- client and server communicate through the existing client/server architecture
- client-side chunk storage is conceptually distinct from server-side chunk ownership

What `R0` does instead:

- introduces engine-native client/server contracts and a local in-process transport first
- defers worker transport to `R1`
- defers network transport to later runtime slices

Why that divergence is acceptable here:

- it preserves the authoritative ownership shape from vanilla
- it avoids overfitting the contracts to browser workers or a specific network transport
- it makes later `postMessage`, Node, and optional WebRTC adapters reuse the same boundary

What must remain parity-safe:

- worldgen logic remains in the translated simulation/core side
- renderer stops owning authority
- message and cache shapes stay factual and serializable, not renderer-specific object graphs

## Proposed module layout

Exact filenames can shift if the implementation teaches us something, but `R0` should land roughly this shape:

```
src/
  runtime/
    protocol/
      world-messages.ts         # command/update payload shapes
      world-host.ts             # WorldHost contract
      world-client.ts           # WorldClient contract
    transport/
      local-world-transport.ts  # in-process local transport adapter
    host/
      generated-world-host.ts   # authoritative generated-world host over current generator
  world/
    level/
      client-chunk-cache.ts     # read-only chunk cache for renderer/client consumption
      chunk-snapshot.ts         # serializable chunk snapshot shape if split from protocol module
  renderer/
    scene-setup.ts              # consumes WorldClient/client chunk cache, not generator
    main.ts                     # drives camera/view commands through WorldClient
```

The key choice is not the folder names; it is the separation:

- runtime boundary and transport in `src/runtime/`
- client-readable chunk state separate from authoritative host ownership
- no renderer module importing the generator directly

## Initial protocol shape

Do not over-design gameplay protocol here. `R0` only needs the minimum chunk/view boundary.

Client → host:

- `openWorld` or equivalent bootstrap/config request
- `setChunkView { centerChunkX, centerChunkZ, radius }`

Host → client:

- `worldOpened` or equivalent acknowledgment
- `chunkSnapshot { chunkX, chunkZ, ... }`
- `chunkUnload { chunkX, chunkZ }`

Optional in `R0` only if needed by implementation:

- `worldError`
- `stats` / debug counters for smoke visibility

Deliberately deferred:

- player/entity state
- block-interaction commands
- tick-driven deltas beyond chunk load/unload
- reliable networking concerns

## Snapshot shape guidance

Snapshots should be factual and transport-friendly.

Prefer:

- block-state IDs, palette data, biomes, heightmaps, and other engine-readable chunk facts
- immutable snapshot objects or typed-array-backed payloads
- data that can be consumed by both a browser client cache and a future Node/client boundary

Avoid:

- live `BlockState` instances crossing the host/client boundary
- renderer-owned GPU or meshing data in host messages
- browser-specific APIs in the authoritative host

The client side can reconstruct whatever richer runtime views it needs from the factual snapshot.

## Concrete steps

1. Define the minimal runtime protocol types for world bootstrap and chunk view updates.
2. Define `WorldHost` and `WorldClient` contracts around that protocol instead of around direct object references.
3. Implement a local in-process transport adapter that exercises the same protocol shape without workers.
4. Extract the authoritative responsibilities currently buried in `GeneratedRenderLevel` into a generated-world host:
   - chunk generation
   - chunk decoration
   - chunk residency / in-range management
   - publishing snapshots and unload events
5. Introduce a client-side chunk cache that receives snapshots/unloads and exposes the read path the renderer needs.
6. Rewire `scene-setup.ts` and `main.ts` so the browser smoke uses `WorldClient` + client chunk cache rather than `GeneratedRenderLevel`.
7. Remove renderer-side imports of `NoiseBasedChunkGenerator` and `GeneratedRenderLevel` from the smoke/render path.
8. Preserve the existing generated-scene visual result as closely as practical so this remains an ownership refactor, not a feature rewrite.
9. Document any runtime divergences from the reference source directly in the affected modules with the usual one-line platform comment where appropriate.
10. Update the runtime/host arc in [`README.md`](README.md) to mark `R0` as complete and point `R1` at the worker boundary.

## Validation

### Unit (Vitest)

- Protocol/message types cover the minimal chunk-view lifecycle without needing browser globals.
- The local transport adapter delivers commands and updates through the same contract shape the future worker transport will use.
- The authoritative generated-world host produces chunk snapshots for the pinned generated scene and emits unloads when the view moves out of range.
- The client chunk cache loads snapshots and unloads chunks correctly while preserving lookup behavior the renderer depends on.
- Existing worldgen oracle suites stay green.

### Browser smoke (Playwright + system Chrome)

- The generated-terrain smoke still renders through the translated renderer path.
- The renderer path no longer directly constructs or calls the generator-owned world bridge.
- Camera/view updates go through the `WorldClient` boundary.
- The final screenshot is inspected manually before continuing.

### Structural checks

- No browser-render-path module imports `NoiseBasedChunkGenerator` directly.
- No browser-render-path module imports `GeneratedRenderLevel` directly.
- The authoritative generated-world owner can be instantiated without renderer/WebGPU dependencies.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- The generated browser smoke still renders terrain through the new boundary
- The render path consumes a client-side chunk cache instead of an authority-owning generated level
- The first row of the runtime/host arc in [`README.md`](README.md) links this slice

## Out of scope for this doc

- browser worker transport — that is `R1`
- mesh worker offload — that is `R2`
- persistence adapters — that is `R3`
- Node/dedicated host bootstrap — that is `R4`
- remote multiplayer transport — later runtime slices
- gameplay/entity/session protocol beyond chunk view + chunk snapshots

## Next

`R1` is the next runtime slice: move the `R0` authoritative host behind a browser worker with a `postMessage` transport adapter, keeping the same contracts and client-side chunk cache in place.
