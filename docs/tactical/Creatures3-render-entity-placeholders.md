# Creatures3 - Render entity placeholders

Status: **superseded** by [`EntityRender0-vanilla-entity-renderer-foundation.md`](EntityRender0-vanilla-entity-renderer-foundation.md).

Standing after [`ClientRuntime6-entity-interpolation-and-ai-bridge.md`](ClientRuntime6-entity-interpolation-and-ai-bridge.md), which made authoritative entity snapshots available as visual-only interpolated presentation state through `ClientRuntime`.

## Goal

Render simple browser-visible placeholders for authoritative entity snapshots.

This placeholder-first direction is no longer the active plan for player rendering. Entity presentation should now proceed through the vanilla-shaped renderer stack recorded in [`EntityRender0-vanilla-entity-renderer-foundation.md`](EntityRender0-vanilla-entity-renderer-foundation.md).

This is deliberately a presentation slice:

- consume `ClientPresentationState.entityPresentation`
- draw simple debug placeholders at interpolated entity positions
- make generated passive mobs visibly inspectable in browser smoke/probes
- keep entity authority, AI, spawning, and lifecycle on the host

## Scope

| # | Work | Expected result |
|---|---|---|
| 1 | Choose placeholder path | minimal debug geometry or overlay that can be visually validated without model loading |
| 2 | Consume runtime presentation state | browser presentation reads `entityPresentation`, not raw host/client internals |
| 3 | Preserve authority boundary | placeholders are visual-only and do not mutate `ClientWorld` or entity snapshots |
| 4 | Add focused validation | browser smoke or smallest relevant probe captures visible entity placeholders |

## Do Not Add

- entity model baking/loading
- animations, sounds, particles, selection UI, hit tests, or combat
- client-owned AI, pathfinding, goals, despawn, breeding, or spawning
- entity delta protocol
- movement prediction changes

## Validation

- `pnpm typecheck`
- focused unit tests if a reusable placeholder mapper is added
- `pnpm test:browser`
- smallest relevant `pnpm probe:browser -- test/browser/probes/<name>.probe.ts` with screenshot inspection if rendered pixels change
- `git diff --check`
