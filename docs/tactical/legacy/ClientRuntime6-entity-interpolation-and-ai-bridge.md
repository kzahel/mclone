# ClientRuntime6 - Entity interpolation and AI bridge

Standing after [`ClientRuntime5-prediction-service-scaffold.md`](ClientRuntime5-prediction-service-scaffold.md), which exposed prediction services through `ClientRuntime` and bounded them to `ClientWorldPredictionView` facts.

Status: **done**.

Landed result: `ClientRuntime` now owns a client entity interpolation service. `ClientWorld` exposes an explicit entity view, raw authoritative entity snapshots remain available in presentation state, and visual-only `entityPresentation` records provide interpolated positions/rotations plus an `aiAuthority: "host"` boundary marker for future NPC presentation.

## Goal

Create the client-runtime bridge that later entity rendering and NPC presentation can consume without inventing a second client entity model.

The bridge must:

- read entity snapshots from `ClientWorld`
- keep interpolation buffers in client-runtime/presentation ownership
- publish visual-only presentation records
- preserve raw authoritative snapshots
- make AI authority explicit as host-owned

## Source Review

Reviewed current entity and presentation paths:

- `src/runtime/client/client-runtime.ts`
- `src/runtime/client/client-world.ts`
- `src/runtime/client/entity-interpolation-service.ts`
- `src/runtime/protocol/world-messages.ts`
- `src/runtime/transport/local-world-transport.ts`
- `src/runtime/transport/remote-world-transport.ts`
- `src/renderer/debug/debug-free-cam.ts`
- `test/runtime/remote-world-transport.test.ts`

## Scope

| # | Work | Result |
|---|---|---|
| 1 | Add entity view | `ClientWorld.getEntityView()` exposes bounded authoritative entity snapshots |
| 2 | Add interpolation service | `ClientEntityInterpolationService` keeps previous/current snapshot buffers and samples visual-only entity state |
| 3 | Attach to runtime | `ClientRuntime.getEntityInterpolationService()` exposes the service and `publishPresentationState(...)` publishes `entityPresentation` |
| 4 | Preserve authority | raw `entities` remain authoritative protocol snapshots; `entityPresentation` is presentation-only |
| 5 | Keep AI host-owned | presentation records include `aiAuthority: "host"` and no client AI behavior |

## Do Not Add

- entity models, animations, sounds, particles, or selection UI
- client-owned AI, pathfinding, goals, despawn, combat, breeding, or natural spawning
- entity delta protocol or removal-by-entity protocol
- client-side generation fallback
- movement prediction or physics changes
- transport protocol changes

## Architecture Divergence Review

Vanilla hydrates client entities from server packets and renders/interpolates them client-side while the server remains authoritative for AI and simulation. `mclone` does not yet have the full vanilla entity packet set, so this slice stays at the current snapshot protocol level. The runtime divergence is narrow: interpolation buffers are client presentation state, while authority, AI, spawning, and lifecycle stay with the host-owned entity runtime. This keeps future rendering and NPC work aligned with `ClientWorld` instead of reading host internals.

## Validation Run

- `pnpm -s vitest run test/runtime/client-entity-interpolation-service.test.ts test/runtime/client-prediction-service.test.ts test/runtime/render-world-update-sink.test.ts test/runtime/remote-world-transport.test.ts`
- `pnpm typecheck`
- `pnpm test:browser`
- `pnpm test:browser:integration`
- `git diff --check`

## Done When

- [x] `ClientWorld` has an explicit entity view.
- [x] `ClientRuntime` owns entity interpolation buffers.
- [x] presentation state includes raw authoritative snapshots and visual-only interpolated entity state.
- [x] AI authority is explicit and host-owned.
- [x] no renderer feature, AI behavior, movement physics, or transport changes were introduced.

## Next Step

[`Creatures3-render-entity-placeholders.md`](Creatures3-render-entity-placeholders.md): consume `ClientPresentationState.entityPresentation` in the browser renderer/debug presentation layer and draw simple authoritative entity placeholders.
