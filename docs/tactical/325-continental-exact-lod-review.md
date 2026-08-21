# Tactical 325: Continental Exact And LOD Review

Status: **planned 2026-08-21; implementation authorized through Human Review
C.**

Topic: `continental-ecoregion-planning`

## Instruction Synthesis

The continental candidate's macro organization is reasonable enough to
continue, but Human Review B did not accept the surface. World Explorer still
reads primarily as abstract colors and proxy cover, and its current reference
refill performance is poor. The next evidence must be ordinary exact voxel
terrain joined to the same procedural-horizon LOD, not another plan atlas.

Implement a bounded exact-to-LOD vertical slice at three contrasting journeys,
commit coherent milestones, and stop for Human Review C. Do not change the
product default or present the detached review source as a second long-lived
Overworld.

## Product Question

Does the accepted macro plan become memorable, plausible terrain when lowered
to real blocks, materials, water, vegetation, and walking-scale relief, while
the exact foreground and procedural horizon remain one recognizable geography
at usable streaming cost?

## Relationship To Mclone Overworld

The intended product outcome is a replacement of the macro backbone of
`mclone-overworld-v1`, not a replacement of every local generator subsystem
and not a permanent parallel planning world.

```text
future mclone-overworld-v1 descriptor
        -> continental/ecoregional plan
        -> shared surface, water, cover, and identity facts
             -> ordinary exact chunks near the player
             -> matching procedural-horizon LOD farther away
```

This tactical remains a promotion probe. It uses
`TerrainPreviewProfile::ContinentalEcoregionCandidate` to select one shared
worldgen source in the detached canonical exact compiler and World Explorer.
It does not add a persisted `WorldGenerationProfile`, a codec tag, a new-world
option, or a live server branch. If Human Review C accepts the result, a later
tactical may intentionally revise `mclone-overworld-v1` in place under the
compatibility safety ledger. If review rejects it, the detached source can be
removed without migrating worlds.

## Three Review Sites

Use the existing deterministic journey catalog and seed 12,345:

| Site | Required evidence |
|---|---|
| clearing-between-forest-cores | traversable open core, enclosing exact trees, quiet local relief, cover agreement into LOD |
| connected-water-country | exact owned water, bed/bank materials, flat lake or river surface, water agreement into LOD |
| upland-to-arid-basin | elevation transition, dry substrate strata, sparse exact cover, recognizable arid continuation in LOD |

The sites are review coordinates, not authored showcases or renderer recipes.
The same generator must work at arbitrary coordinates.

## Shared Exact Contract

`mclone-worldgen` owns candidate exact lowering. One generated column must use
the same `ContinentalSurfaceSample` that feeds LOD and publish:

- bounded solid terrain from bedrock through the quantized canonical surface;
- top and subsurface blocks derived from the typed substrate and water cause;
- explicit source water only where the shared sample owns water;
- an existing compatible biome ID for block tint and mesh presentation;
- candidate-owned deterministic vegetation records realized as ordinary tree
  blocks; and
- receipts that compare exact top, water, substrate, and cover facts with
  direct samples at the same coordinates.

Walking-scale refinement must be added to the shared surface source before
exact lowering if the current field is too smooth. Exact generation may
quantize continuous heights and realize bounded block strata; it may not add a
second unrelated heightfield, coastline, lake, route, or forest decision.

Surface and final-feature chunks use the ordinary `GeneratedChunk`, biome
payload, textured mesh, exact boundary profile, and exact/proxy ownership
formats. Candidate dependencies must be bounded and request-order independent.

## Exact-To-LOD Composition Contract

`mclone-terrain-view` continues to own composition. Extend the existing
canonical exact compiler and runtime exact renderer to preserve the selected
candidate source identity rather than hard-coding production. The existing
focus-connected exact coverage, discard mask, direct smooth frontier,
procedural water arbitration, and tree ownership must operate unchanged.

World Explorer may select journeys, exact radius, composition, and cameras. It
must not lower blocks, choose materials, synthesize trees, or special-case the
three sites. Journey selection should default to composed exact-plus-horizon
for this review, while explicit horizon-only comparison remains available.

## Performance Posture

The current 2.09-2.49 second complete 160-slot candidate refill is a measured
prototype limitation, not an accepted runtime budget. Separate:

1. direct surface sampling;
2. exact surface and feature generation;
3. exact meshing and upload;
4. candidate vegetation compilation;
5. cold clipmap completion;
6. retained movement/refill; and
7. browser Worker versus presentation time.

First make the exact/LOD result semantically and visually correct. Then remove
obvious redundant reconstruction and preserve retained work across movement.
Do not introduce a semantic cache whose contents affect geography, reduce
nearby quality based on clipmap level, or hide a full rebuild behind vague
"prototype" language.

## Phases And Commit Gates

### Phase 0: Tactical And Boundary

- Record Human Review B as conditional continuation rather than acceptance.
- Pin the detached exact compiler boundary and production non-change.
- Commit before implementation.

### Phase 1: Exact Surface Lowering

- Add candidate exact column/chunk generation in `mclone-worldgen`.
- Lower typed substrate and water facts into bounded block strata and biomes.
- Prove adjacent chunks, negative coordinates, traversal order, exact/direct
  agreement, flat owned water, and deterministic fingerprints.
- Connect Surface-stage canonical compilation and capture the first exact-only
  image. Inspect it before adding vegetation.
- Commit.

### Phase 2: Composed Exact And LOD

- Make native and browser exact executors source-qualified.
- Permit candidate exact, coverage, and composed modes without enabling them
  for incompatible sources.
- Exercise the ordinary frontier certificate at all three sites.
- Capture and inspect exact-only, coverage, and composed frames, including one
  frame where the exact boundary is plainly visible.
- Commit.

### Phase 3: Exact Candidate Cover

- Reuse the candidate's stable global vegetation records for exact blocks and
  LOD proxies.
- Preserve whole-tree exact/proxy ownership at the frontier.
- Add bounded local ground cover only if it is derived from shared candidate
  facts and materially improves walking-scale review.
- Inspect the clearing, water, and arid sites independently.
- Commit.

### Phase 4: Runtime Cost And Review Package

- Add sequential native and headed-browser three-site capture/smoke runners.
- Measure cold completion and retained movement with separate exact, clipmap,
  vegetation, mesh, upload, and frame receipts.
- Correct avoidable full-plan or full-coverage rebuilds exposed by the
  measurements without changing geography.
- Update living topics and this execution record, push, deploy the exact
  revision, verify public pixels, and stop at Human Review C.

## Acceptance Evidence

Human Review C receives, for each site:

- an exact-only walking/low-oblique frame;
- a coverage frame identifying exact ownership;
- a composed exact-plus-LOD low oblique;
- a broader composed horizon frame;
- a short retained movement step;
- direct-versus-exact height, water, material, and cover receipts; and
- cold and retained performance measurements.

Acceptance requires real terrain that reads beyond diagnostic colors,
recognizable continuity across the exact frontier, plausible water and cover,
materially different site stories, and a credible path to runtime performance.
Exact pixels alone do not authorize production integration.

## Non-Goals

This tactical does not:

- modify `mclone-overworld-v1`, the new-world default, persistence, or saves;
- add a new persisted generation profile or retain the candidate as a product
  mode;
- implement caves, ores, structures, global erosion, or complete decoration;
- implement animal simulation or migration;
- solve every terrain-character family or exact site;
- select a large cylinder period; or
- declare the macro plan or surface accepted without Human Review C.

## Human Review C

The decision is one of:

1. **accept** the exact/LOD vertical slice and authorize promotion planning for
   a substantial `mclone-overworld-v1` revision;
2. **revise** named macro, local terrain, water, material, vegetation,
   frontier, or performance behavior; or
3. **reject** the surface realization while retaining or separately revising
   the plan.

## Related

- [`324-continental-surface-world-explorer.md`](324-continental-surface-world-explorer.md)
- [`313-direct-exact-to-smooth-horizon-transition.md`](313-direct-exact-to-smooth-horizon-transition.md)
- [`../topics/continental-ecoregion-planning.md`](../topics/continental-ecoregion-planning.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/lod.md`](../topics/lod.md)
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
