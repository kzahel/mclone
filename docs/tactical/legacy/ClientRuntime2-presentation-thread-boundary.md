# ClientRuntime2 - Presentation thread boundary

Standing after [`ClientRuntime1-client-world-replica-hydration.md`](ClientRuntime1-client-world-replica-hydration.md), which moved protocol message application behind `ClientWorld` hydration APIs while keeping transports as carriers.

Status: **done**.

Landed result: browser scene setup now owns a `ClientRuntime`, uses it for world open, chunk-interest, player commands, update draining, and presentation-state reads, while render-world update sink wiring stays behind the runtime facade.

## Goal

Make the browser UI/render path consume compact presentation surfaces from `ClientRuntime` and `ClientWorld` instead of reaching through transport compatibility methods or owning raw replica mutation.

The presentation thread should own input sampling, pointer lock, DOM/UI state, GPU resources, draw submission, debug camera controls, and compact read-only snapshots. It should not apply host protocol messages, synthesize client-world facts, or treat `ClientChunkCache` as a transport-facing model.

## Source Review

Re-read the current browser and debug presentation paths before changing ownership:

- `src/renderer/scene-setup.ts`
- `src/renderer/debug/debug-free-cam.ts`
- `src/runtime/client/client-runtime.ts`
- `src/runtime/client/client-world.ts`
- `src/renderer/chunk/render-world-worker-client.ts`
- `src/renderer/chunk/render-world-worker.ts`

Re-check these vanilla files for ownership shape, not literal WebGPU architecture:

- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/GameRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`

## Scope

| # | Work | Expected result |
|---|---|---|
| 1 | Route scene setup through `ClientRuntime` where practical | browser open/set-interest/input/poll paths use the runtime facade instead of calling transport clients directly |
| 2 | Define compact presentation state access | debug camera, UI counters, and renderer coordination read `ClientPresentationState` / `ClientWorld` views instead of scattered raw client methods |
| 3 | Preserve render-world ownership | render-world worker still receives derived chunk/light/unload updates from `ClientWorld`, not from transport-specific logic |
| 4 | Keep compatibility cache bounded and read-only to presentation | renderer can keep using `ClientChunkCache` for mesh compatibility, but does not mutate it or treat missing authority as generated data |
| 5 | Leave player authority unchanged | debug controls still submit commands to the host; no prediction, smoothing, or new movement semantics |

## Do Not Add

- movement prediction, reconciliation, or physics changes
- entity rendering/interpolation beyond preserving current snapshot publication
- new network transports
- client-side worldgen, decoration, or seed fallback
- renderer rewrites unrelated to presentation ownership
- GPU pipeline or meshing behavior changes

## Validation

- `pnpm typecheck`
- focused unit tests if new presentation helpers are introduced
- `pnpm test:browser`
- `pnpm test:browser:integration`
- `git diff --check`

`pnpm test:browser:integration` is required for this slice because debug camera controls, presentation-state consumption, update polling, and chunk-interest movement are in scope.

Run a screenshot probe only if the implementation changes camera framing, visible rendered pixels, mesh invalidation, or entity presentation.

## Done When

- [x] Browser scene setup owns a `ClientRuntime` and uses it for host/client lifecycle operations.
- [x] Presentation state publication is the normal path for UI/debug state reads.
- [x] Render-world ingestion remains derived from `ClientWorld` hydration.
- [x] No transport class owns client replica mutation semantics.
- [x] Current remote and worker browser smoke paths still render and debug free-cam integration still passes.

## Validation Run

- `pnpm typecheck`
- `pnpm test:browser`
- `pnpm test:browser:integration`
- `git diff --check`

## Next Step

`ClientRuntime3-integrated-server-flow.md`: give browser singleplayer an explicit `IntegratedServer` bootstrap/facade with local session lifecycle semantics while preserving the shared host/client protocol path.
