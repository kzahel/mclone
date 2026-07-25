# Far Terrain LOD Architecture

Status: retired experiment architecture 2026-07-25.

This document preserves the research and original experimental target shape
after reviewing Distant Horizons and Voxy. It is not current implementation
guidance. The resulting chunk-granular in-game system was removed by
[`docs/tactical/245-retire-chunk-far-lod-runtime.md`](tactical/245-retire-chunk-far-lod-runtime.md).
Current status lives in
[`docs/topics/far-lod.md`](topics/far-lod.md); future multiscale work starts
from Terrain Lab and
[`docs/topics/gpu-procedural-terrain.md`](topics/gpu-procedural-terrain.md).

## Historical Decision

Start with **minimal surface terrain LOD**, not a Distant Horizons clone.

The first version should answer one question:

```text
Can a cheap, non-authoritative terrain shell make small render distances feel
larger without destabilizing normal chunk generation, rendering, or Quest frame
pacing?
```

It should be possible to turn the system fully off. LOD should never block,
replace, or satisfy authoritative gameplay chunks.

## Reference Implementations

### Distant Horizons

Local reference clone:
[`reference/distant-horizons`](../reference/distant-horizons)

Distant Horizons solves a broader problem than our first slice:

- distant natural terrain
- trees/features
- player-built structures
- arbitrary vertical shapes
- caves and overhangs
- multiplayer/server-provided LOD data
- persistent reuse across sessions

The important design points:

- A lowest-level `FullDataSourceV2` tile is `64 x 64` block columns, or `4 x 4`
  normal Minecraft chunks because chunks are `16 x 16` columns:
  [`FullDataSourceV2.java`](../reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/dataObjects/fullData/sources/FullDataSourceV2.java:76)
- DH extracts vertical column spans from chunks, top-down, preserving block,
  biome, height, and light transitions:
  [`LodDataBuilder.java`](../reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/dataObjects/transformers/LodDataBuilder.java:64)
- Parent LOD columns are built by merging `2 x 2` child columns:
  [`FullDataSourceV2.java`](../reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/dataObjects/fullData/sources/FullDataSourceV2.java:722)
- DH can pre-generate LOD sections and persist them:
  [`PregenManager.java`](../reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/generation/PregenManager.java:39)
- DH has coarse distant generation modes. `SURFACE` generates surface only and
  excludes trees/structures, while `FEATURES` runs Minecraft's feature pass:
  [`EDhApiDistantGeneratorMode.java`](../reference/distant-horizons/coreSubProjects/api/src/main/java/com/seibel/distanthorizons/api/enums/worldGeneration/EDhApiDistantGeneratorMode.java:60)
- DH separately simplifies render data with cave culling, ignored blocks,
  non-colliding block skipping, and hidden-block compression:
  [`Config.java`](../reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/config/Config.java:669),
  [`Config.java`](../reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/config/Config.java:777),
  [`FullDataOcclusionCuller.java`](../reference/distant-horizons/coreSubProjects/core/src/main/java/com/seibel/distanthorizons/core/dataObjects/transformers/FullDataOcclusionCuller.java:11)

Use DH as the reference for durable concepts:

```text
semantic LOD data first
runtime renderer buffers second
generation and rendering are separate
LOD can be persisted and regenerated independently from final GPU meshes
```

Do not start by porting DH's full vertical column/span data model.

### Voxy

Local reference clone:
[`reference/voxy`](../reference/voxy)

Voxy is useful later for GPU-driven traversal, compact section storage, and
hierarchical culling ideas. Its OpenGL 4.6 and compute-heavy shape is a poor fit
for the first shared `wgpu`/Quest/Web/XR experiment, and its source license is
all-rights-reserved. Treat it as conceptual background only.

## Terms

### Normal Chunk

A normal gameplay chunk is the authoritative world representation:

```text
16 x 16 block columns horizontally
full vertical block sections
owned by the host/server
used for gameplay, collision, lighting, block edits, entities, and persistence
```

Normal chunks are still the only source of truth.

### Surface LOD Sample

A surface LOD sample is one cheap distant terrain record for a larger horizontal
area:

```text
surface_y
surface material or color
biome/tint identity
optional water surface_y
optional slope/normal
flags
```

It deliberately does not describe underground data, cave interiors, full
structures, exact vegetation, or player edits.

### LOD Tile

An LOD tile is a grid of surface LOD samples at one horizontal resolution.

Example top view:

```text
LOD level 0: each cell covers 1 x 1 block column

+---+---+---+---+
| a | b | c | d |
+---+---+---+---+
| e | f | g | h |
+---+---+---+---+

LOD level 1: each cell covers 2 x 2 original columns

+-------+-------+
|   A   |   B   |
+-------+-------+

LOD level 2: each cell covers 4 x 4 original columns

+---------------+
|       Q       |
+---------------+
```

The first `mclone` version may generate tiles directly at the target sample
spacing instead of recursively deriving each level from a finer tile. That keeps
the experiment small.

### Worldgen LOD Profile

The worldgen LOD profile answers:

```text
How much distant world content do we create in the first place?
```

Initial profiles:

```text
Off
  no LOD generation

SurfaceOnly
  generate terrain surface and biome/material color only
  no caves, no structures, no exact features

SurfacePlusApproxTrees
  SurfaceOnly plus deterministic coarse tree hints
  not exact Minecraft feature generation
```

Future profiles may include more major features, but the first implementation
should not run the full vanilla `FEATURES` pass for distant-only terrain.

### LOD Visibility Profile

The LOD visibility profile answers:

```text
Given distant generated facts, what is worth storing and drawing?
```

Initial profile:

```text
SurfaceShell
  keep top terrain surface
  keep water surface only if cheap and visually useful
  drop underwater terrain
  drop cave interiors
  drop underground blocks
  drop non-colliding details
  drop structures and block edits
```

These profiles must stay separate. Worldgen controls what facts exist; visibility
controls what facts are kept for distant rendering.

## Goals

- Make small normal render distances feel less constrained, especially on
  Quest-class hardware.
- Keep normal chunk generation, lighting, simulation, and publication higher
  priority than all LOD work.
- Keep implementation and validation shared across the affected target
  contracts; no target owns LOD policy.
- Keep the renderer path multiview-aware and compatible with desktop flat,
  desktop XR, Android XR, flat Android, and web/WASM.
- Keep LOD data non-authoritative and removable.
- Keep generation and rendering independently disableable.
- Learn from the smallest representative prototype before investing in durable
  persistence or exact correctness.

## Non-Goals For The First Slice

- No arbitrary structure LOD.
- No player-built structure LOD.
- No exact Minecraft feature generation for distant-only tiles.
- No cave or underground LOD.
- No underwater terrain shell.
- No persisted LOD database as a requirement.
- No LOD-derived collision, gameplay, AI, light propagation, or server truth.
- No DH-style vertical column-span port.
- No Voxy-style GPU-driven hierarchy.

## First Implementation Shape

The first implementation should look like this:

```text
authoritative host/server
  normal chunk scheduler
  normal chunk generation/light/publication
  low-priority LOD scheduler
    -> surface LOD generator
    -> in-memory LOD tile cache
    -> client/render-session visible LOD tile stream

renderer
  normal chunk render sections near the camera
  simple opaque far-terrain surface mesh beyond normal chunks
```

The LOD world is not a second gameplay world. It is a visual cache.

## Scheduling Policy

LOD generation is opportunistic.

Rules:

- Normal chunk work always wins.
- LOD generation starts only after the nearby normal chunk radius has reached
  the configured readiness gate.
- Missing LOD is acceptable.
- LOD jobs are cancellable or cheaply discardable when the player moves.
- Per-frame/per-tick generation work is budgeted.
- LOD work must not trigger normal chunk publication or save obligations.

Initial readiness gate:

```text
Do not enqueue far LOD jobs until normal chunks inside the active gameplay/render
interest radius are fully generated, lit, and publishable enough for the current
host policy.
```

This is intentionally conservative. It prevents the far-terrain experiment from
competing with the visible gameplay area while we learn the cost.

Example priority order:

```text
P0 player/session commands
P1 visible normal chunk load/generate/light/publish
P2 normal render-section compile/upload
P3 near-future normal chunk prefetch
P4 LOD tile generation
P5 LOD mesh rebuild/upload
```

## Data Model

Start with a small surface record, not a packed compatibility format.

Proposed logical record:

```text
LodSurfaceSample {
  surface_y
  surface_kind
  biome_id_or_tint
  water_y_optional
  flags
}
```

Proposed tile identity:

```text
dimension_id
world_seed_or_world_identity
lod_level
tile_x
tile_z
sample_spacing_blocks
tile_width_samples
generator_profile_version
visibility_profile_version
```

Do not bake GPU buffer layout into the logical cache. Renderer buffers are
derived products.

## Generation Algorithm

Initial algorithm:

1. Choose a tile outside the normal render distance.
2. For each sample point, run the cheapest available terrain-surface query for
   the current world profile.
3. Record the top visible terrain surface.
4. If water handling is enabled and the sample is water-covered, record only the
   water surface and do not record underwater terrain.
5. Derive a simple color/material from biome and surface block.
6. Store the tile in an in-memory cache.

The first version may use a simplified surface query rather than exact vanilla
status execution. This is an intentional visual-only divergence. The generated
LOD tile must not be used as a gameplay chunk or as a source for persisted
authoritative world data.

Approximate trees, if added, should be deterministic and coarse:

```text
seed + biome + low-frequency noise -> occasional tree marker
tree marker -> simple blob/billboard/low-poly silhouette
```

Do not call this vanilla feature parity.

## Rendering Algorithm

Start with one simple render path:

- opaque terrain mesh
- no alpha-tested grass/flowers
- no exact block atlas sampling if a vertex color path is enough
- cheap fog/haze to hide distance and LOD transitions
- optional skirts between tile edges/levels
- no underwater shell
- no translucent water beyond a simple surface if needed

The pass must preserve the XR guardrails:

- each view/layer gets its own view/projection data
- multiview support must be considered before adding a Quest-visible path
- LOD work must not become desktop-only app glue

Performance expectation:

```text
LOD is not primarily a fragment-shader optimization.
It may increase fragment work compared with drawing only sky past the normal
render distance. Its value is replacing expensive full chunks with cheap distant
world presence.
```

For Quest, prefer:

- low shader cost
- opaque early-Z-friendly terrain
- few textures or vertex colors
- small draw count
- no discard-heavy material path
- fogged transitions

## Transition Policy

Starting LOD immediately after a small normal render distance can pop visibly.
Treat this as an experiment, not a quality claim.

Initial tuning knobs:

```text
normal_render_distance_chunks
lod_start_chunks
lod_transition_chunks
lod_max_distance_chunks
lod_sample_spacing_by_level
lod_fog_start
lod_fog_end
```

For early Quest experiments, a plausible shape is:

```text
normal chunks: 5
transition band: 5..7
first visible LOD: 7 or 8
fog/haze: hides mesh simplification and missing features
```

Desktop can use wider distances to debug quality, but the architecture should
not assume high-end PC render budgets.

## Disable And Debug Controls

Required controls:

```text
lod.enabled
  hard off: no generation, no cache writes, no render pass

lod.generation.enabled
  keep renderer/cache readable, but do not generate new tiles

lod.render.enabled
  generate/cache tiles, but do not draw them
```

The first user-facing implementation may expose only `lod.enabled`, but internal
generation/render gates should be separate from the beginning.

Useful debug overlays:

- visible LOD tile boundaries
- LOD level color
- generated tile count
- queued tile count
- generation time budget and actual time
- mesh build/upload count
- transition band visualization

## Persistence Position

Do not require persistent LOD storage for the first slice.

Distant Horizons persists LOD data because its source data is expensive and
valuable across sessions: chunk reads/generation, vertical span extraction,
parent propagation, generation-step tracking, server-provided LODs, and large
view distances. That is the right shape for DH.

For `mclone`, start with:

```text
in-memory only
throw away on world close
no compatibility format
no invalidation problem
```

Later persistence options:

```text
Session cache
  in-memory only, useful for movement within one run

Ephemeral disk cache
  safe to delete at any time
  keyed by seed, world id, dimension, LOD profile, visibility profile, generator version

Durable LOD cache
  retained across sessions
  explicit invalidation and migration policy
  still non-authoritative
```

Open persistence questions:

- Should LOD tiles be host-side only, client-side only, or both?
- Should remote dedicated servers stream LOD tiles, or should clients generate
  visual-only LOD from shared seed/profile facts?
- How do edited chunks invalidate nearby surface tiles?
- How do custom worlds or non-vanilla profiles declare LOD compatibility?
- What is the minimum cache key that avoids stale visual lies?

Do not solve these before the minimal visual experiment proves useful.

## Multiplayer And Authority

LOD data is presentation data unless promoted by a later explicit design.

Rules:

- LOD does not satisfy block queries.
- LOD does not satisfy collision queries.
- LOD does not satisfy entity spawning or AI queries.
- LOD does not satisfy client interaction/raycast queries.
- LOD does not replace normal chunk interest near the player.
- LOD must disappear or be overwritten when real chunks arrive.

For remote servers, the safest first behavior is:

```text
LOD disabled unless local seed/profile facts are available and explicitly allowed.
```

Long term, the host may stream LOD tiles as a separate non-authoritative visual
channel, similar in spirit to DH server-side LOD transmission. That should use
the same host/client command/update boundary rather than becoming a platform
fork.

## Edits And Invalidations

First slice:

```text
ignore player edits outside normal chunks
normal chunks always draw over LOD when available
LOD tiles may be stale until regenerated
```

Future:

- mark tiles dirty when authoritative chunks are edited
- coalesce dirty regions
- rebuild surface samples from real chunk snapshots where available
- keep LOD cache revisions separate from authoritative chunk revisions

Do not let LOD invalidation block gameplay block edits.

## Validation Plan

Shared validation sequence:

1. Render normal chunks only.
2. Add generated surface LOD with a fixed seed and fixed camera.
3. Capture headless screenshots and inspect them.
4. Move the camera across tile boundaries and inspect pop/transition behavior.
5. Measure frame, generation, mesh build, and upload cost.
6. Add a disable flag and prove it removes generation and rendering.

Target-specific checks follow wherever the changed boundary requires them:

- Quest: check frame headroom, stereo correctness, and pop comfort.
- Web/WASM: check worker/budget shape and memory pressure.
- XR: verify per-eye/multiview view data ownership for all LOD draws.

For any slice that produces pixels, follow the native validation rule: capture a
screenshot and inspect it before moving on.

## Open Questions

- What first sample spacing gives acceptable silhouettes near a 5-chunk normal
  render distance?
- Should the first renderer use vertex colors, a tiny material palette, or the
  existing terrain atlas?
- Should water be absent, a flat biome-colored surface, or a simple transparent
  plane?
- Are coarse tree silhouettes worth the extra visual risk before terrain shell
  tuning?
- Should surface LOD use exact native worldgen surface queries or a cheaper
  approximation?
- What budget is small enough that LOD cannot hurt normal streaming?
- Should cache tiles align to chunks, 4x4 chunks like DH, or larger worldgen
  regions?
- How should fog and color grading hide the first LOD ring on Quest?
- When, if ever, do we need DH-style vertical column spans?

## Future Escalation Path

If surface LOD proves useful, the likely sequence is:

```text
1. surface-only in-memory LOD
2. better transition/fog and tile diagnostics
3. water surface
4. coarse deterministic trees
5. ephemeral cache
6. edit invalidation for nearby modified chunks
7. optional host-streamed LOD for remote play
8. evaluate DH-style vertical spans only if structures/overhangs become important
```

The important discipline is to keep the first slice cheap and reversible. We
want measured visual and performance evidence before committing to robust,
persistent, parity-aware far-world infrastructure.
