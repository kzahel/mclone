# Dynamic Point Lights

Topic: `dynamic-point-lights`

Status: design exploration; implementation has not started. The renderer has
no dynamic local-light or shadow path today. A bounded renderer experiment
should precede a production tactical.

## Scope

This topic owns presentation-side, finite-radius dynamic lights, with point
lights as the primary case. Torches, lanterns, carried lights, burning
entities, projectiles, magic effects, and authored local fixtures all fit this
shape. The desired result is especially visible underground: a torch should
add a restrained directional glow, nearby creatures should move through and
cast shadows in it, and a wall should reliably block its direct contribution.

This is deliberately separate from [`lighting.md`](lighting.md), which owns
the Java-shaped stored sky/block light field, chunk-light status, persistence,
mesh sampling, and the `LightTexture` color curve. Dynamic point lights are a
rendering extension and intentional vanilla-parity divergence. They do not
replace the authoritative stored-light facts or change gameplay rules such as
mob spawning, crop growth, melting, or other light-level queries.

A cone/spot light is a close extension of the same source and admission
contract: it adds a direction plus inner/outer cone angles and can use one
projected shadow view instead of six point-light faces. Flashlights are useful,
but point lights come first because ordinary local emitters are omnidirectional
and are the more important general case.

Directional sunlight, global illumination, reflection probes, emissive
materials, and hardware-ray-traced rendering are outside the first concern.
The design should not prevent them, but this topic must not become a general
lighting catch-all.

## Current Renderer Facts

There is no partial implementation to preserve:

- Terrain and legacy actor vertices carry position, UV, color, and Java-packed
  sky/block light. Their shaders evaluate the stored lightmap and fog; they do
  not consume normals or a runtime light list.
- Prepared figures already carry transformed normals and rigid-part matrices,
  but still use a fixed presentation light plus stored packed light rather
  than scene lights.
- `mclone-render` has no light atlas, depth-from-light pass, point-light
  cubemap, clustered/tiled light list, voxel-occupancy GPU view, or dynamic
  shadow mask.
- Shared world rendering uses reversed-Z `Depth32Float`. The current target has
  no stencil component, so stencil shadow volumes would require a deliberate
  depth-target and pipeline change.
- Mono, stereo per-eye, and full-frame XR multiview are live paths. Any point
  light visible in XR must shade both eyes with their own view data. A
  camera-independent point-light cubemap may be shared by both eyes; a
  view-dependent stencil or screen-space classification may not.
- Placed/embedded worlds already have composition-space transforms. A future
  neutral light contract should be composition-ready, but the first DDA proof
  may remain explicitly single-world because one world's occupancy grid does
  not automatically describe overlapping embedded worlds.

Relevant current seams:

- [`../lighting.md`](../lighting.md): stored-light architecture and vanilla
  reference map.
- [`lighting.md`](lighting.md): live stored-light status and render handoff.
- [`embedded-worlds.md`](embedded-worlds.md#what-is-free-vs-what-is-net-new):
  why cross-world light/shadow is a new dynamic-light subsystem.
- `native/crates/mclone-mesh/src/data.rs`: current terrain vertex contract.
- `native/crates/mclone-render/src/shaders/chunk_textured.wgsl`: mono terrain
  stored-light shading.
- `native/crates/mclone-render/src/shaders/chunk_textured_multiview.wgsl`:
  multiview twin.
- `native/crates/mclone-render/src/shaders/prepared_actor.wgsl`: prepared
  figure normals, placement, and current fixed face light.
- `native/crates/mclone-render/src/chunk.rs`: shared reversed-Z depth format,
  render views, terrain targets, and mono/multiview pipelines.
- `native/crates/mclone-scene/src/lib.rs`: shared frame orchestration and
  mono/XR render admission.

## Working Product Direction

The initial product direction is a hybrid rather than a replacement lighting
model:

1. Keep stored sky/block light as the stable low-frequency visibility and
   gameplay floor.
2. Interpret stored block light as ambient/indirect fill when the dynamic-light
   presentation profile is enabled.
3. Add finite-radius point lights as a directional direct-light term.
4. Let many relevant lights contribute without shadows, subject to spatial or
   tiled/clustered culling.
5. Give real-time shadows to only a small importance-selected budget.
6. Make terrain and entities both eligible to cast and receive the selected
   lights' shadows.
7. Degrade by dropping shadow updates, then shadow eligibility, then
   low-contribution lights before sacrificing frame cadence.

This split preserves Minecraft's readable light around corners while allowing
the direct torch component to produce dramatic silhouettes. A shadow removes
the local direct term, not the stored-light floor, so a gameplay-lit location
does not become visually pitch black merely because a creature crossed a
torch.

There is unavoidable energy overlap: the stored block-light field already
contains placed-torch contribution but does not retain per-source provenance.
Raw addition would double-light the room. The dynamic profile should therefore
tune stored block light as restrained ambient fill and use a bounded direct
term, for example by reducing the stored block-light weight and using
saturating or tone-mapped composition. Exact coefficients are a visual
decision to make from captures, not constants to select in this document.

Torch flicker should initially modulate intensity and perhaps color, not light
position. Moving the source every frame invalidates cached visibility and
causes distracting shadow swimming. A later soft-shadow experiment can jitter
samples around a small flame area with blue noise and temporal accumulation
without changing the stable logical source.

Whether the finished presentation profile is default, optional by quality
tier, or explicitly non-vanilla remains open. The diagnostic fullbright path
should continue to bypass both stored and dynamic shading so it remains a
useful isolation tool.

## Shared Ownership And Source Contract

Dynamic lighting is not app/platform glue. Desktop, web, Android, XR, and
offscreen hosts should consume the same contracts and policy:

| Concern | Shared owner |
|---|---|
| Replicated emissive block/entity facts | `mclone-client` and existing content facts |
| Neutral light source and shadow-policy data | `mclone-render-session` |
| Source collection, placement/topology resolution, admission, and frame budget | `mclone-scene` |
| GPU light lists, shading, shadow resources, and technique implementation | `mclone-render` |
| Window, browser, Android, and OpenXR targets/presentation | app/platform adapters only |

The client/render session should maintain an emitter index as chunks and
actors change. It must not scan every resident block every frame. Placed
emitters derive from replicated block/content facts; held and moving emitters
derive from shared actor/effect presentation. Neither changes the
authoritative server light solver merely by being rendered dynamically.

A first neutral point-light record will likely need the following semantics,
though names and packing are not yet selected:

- stable source identity, qualified by drawable world instance;
- world-local position plus enough placement context to resolve a composition
  position and periodic observer-local lift;
- linear RGB color, intensity, finite radius, and an explicit smooth falloff;
- source mobility (`static`, `revisioned`, or `dynamic`);
- shadow eligibility/priority, not a promise that a platform will allocate a
  shadow resource;
- source and occluder revisions for cache invalidation; and
- optional direction/cone fields only when the spot-light extension lands.

Finite support is a contract. The radius bounds source collection, cluster
membership, DDA traversal, shadow-caster collection, and cache invalidation.
The visual falloff should be smooth at the boundary and evaluated in linear
color space. Vanilla's Manhattan-distance light levels are not the point-light
attenuation curve.

## Many Lights And The Shadow Budget

A room containing 100 torches must not become either 100 shadow cubemaps or
three visibly popping lights. The intended policy is:

- gather only sources whose finite volumes can affect the drawable view;
- admit their inexpensive unshadowed contribution through per-tile or
  per-cluster lists, with a measured maximum and overflow counters;
- score shadow candidates using luminance, projected influence, distance,
  screen coverage, source motion, and product priority rather than distance
  alone;
- reserve a slot for an important held/moving light when that produces a
  better player experience;
- retain the selected set with hysteresis and fade shadow strength when
  ownership changes; and
- cache static visibility until an occluder revision inside the finite light
  volume invalidates it.

The exact platform budgets must be measured. A desktop allowance of a few
shadowed lights is not evidence that the same count fits WebGPU or Quest.
Quality profiles may change counts, resolution, update cadence, and technique,
but should not change source semantics or produce different gameplay facts.

## Shadow Technique Options

No technique is selected for all occluders yet. Point lights make the
tradeoffs unusually clear:

| Technique | Strengths | Costs / gaps | Best initial role |
|---|---|---|---|
| Raster shadow cubemap | Arbitrary rasterizable terrain and figures; familiar filtering; reusable by both XR eyes | Up to six depth views per updated light; finite angular resolution, bias, seams, atlas memory | General baseline and comparison |
| Direct voxel DDA | Stable framebuffer-resolution block shadows; no cubemap or seam; exact for full opaque voxels | Cost scales with affected pixels, lights, and traversed cells; needs GPU occupancy and shape policy | First block-shadow experiment |
| Analytical entity OBB/capsule rays | Framebuffer-resolution animated creature shadows; Minecraft-style rigid cuboids are unusually suitable | Per-ray entity/part tests, broad-phase lists, self-shadow rules, approximate curved/cutout silhouettes | First creature-shadow experiment |
| Entity-only cubemap | Accurate animated raster silhouette; combines naturally with DDA terrain occlusion; avoids redrawing chunks into the dynamic layer | Still six entity views per selected point light; filtering/bias remain | Strong hybrid fallback/comparison |
| Stencil shadow volumes | Screen-resolution hard silhouette; no shadow texture | View-dependent and repeated per eye; per-light passes, fill/stencil overdraw, closed/silhouette geometry, hard shadows only; current depth has no stencil | Bounded research experiment, not portable baseline |
| Hardware triangle ray queries | Direct visibility against detailed geometry | Not a shared WebGPU/Android/XR baseline and needs acceleration-structure ownership | Explicitly deferred |

### Voxel DDA

For every shaded fragment affected by a selected light, trace the finite
segment from a biased surface position to the light through a compact GPU view
of block occupancy. Stop on an opaque cell; otherwise admit the direct light.
The result has framebuffer rather than shadow-texture resolution.

The first proof should use full opaque voxels. Production questions include:

- page/section layout and residency for GPU occupancy;
- conservative behavior for missing or stale sections so unloaded data does
  not leak light;
- topology-aware stepping across finite/periodic dimension boundaries;
- opacity for slabs, stairs, fences, leaves, liquids, and cutout geometry;
- step limits, branch divergence, half-resolution masks, and temporal reuse;
- receiver bias without gaps at block contacts; and
- how a placed/embedded world transform maps a ray into the correct source
  grid.

A DDA occupancy view is a renderer acceleration structure, not a second world
authority. It must be revisioned from client-replica block facts and discarded
or rebuilt freely.

### Entity Shadows

Minecraft-shaped creatures make analytical tests credible. Prepared figures
already have rigid-part transforms and current assets have explicit cuboid
proxies for solid geometry. A light can maintain a short list of entities
inside its radius, reject most rays against whole-entity bounds, then test the
remaining ray segment against animated part OBBs or capsules.

This should first target entities casting onto terrain. Correct self-shadowing
and entity-on-entity receiving need primitive identity, start bias, and clear
rules for excluding the receiver surface. Curved primitives, thin texture
layers, and cutout detail may use coarse proxies or fall back to an entity-only
cubemap.

The leading hybrid candidate is therefore:

```text
surface -> point-light visibility
  block occlusion       = voxel DDA
  ordinary mob occlusion = animated OBB/capsule tests
  detailed fallback      = entity-only shadow cubemap
```

The two visibility results combine before applying the direct-light term. A
static terrain cache and a per-frame entity layer may also be compared against
fully direct queries.

### Cubemaps And Caching

A conventional point shadow map stores radial depth in six faces. Short torch
range makes `256x256` or `512x512` faces plausible candidates, but resolution
must be selected from inspected captures and measured memory/bandwidth rather
than assumed. A nearby figure can still expose bias, filtering, and cube-edge
artifacts even when the light falls off quickly.

Placed lights and ordinary blocks are mostly static. A useful split is a
cached static block layer invalidated only by relevant block revisions plus a
small dynamic entity layer updated for the selected lights. The shader may
compare against both layers or consume a merged nearest depth. Shadow updates
can be staggered, but a partially refreshed cubemap must never mix faces from
incompatible light positions or source revisions.

### Stencil Volumes

Doom 3-style shadow volumes remain interesting for cuboid mobs because they
produce crisp screen-resolution hard shadows without a cubemap. They are not
the leading shared path: classification is camera-dependent, must be repeated
for each XR eye, naturally composes one light at a time, and requires a stencil
target plus reliable closed silhouette geometry. They are appropriate for a
small comparative lab if DDA/OBB rays prove unexpectedly expensive or if a
single hero light benefits materially.

## Recommended Experimental Sequence

Do not begin by wiring a desktop-only torch into an app shader. Use the shared
renderer/offscreen fixture path and preserve a route to mono, per-eye, and
multiview consumers.

1. Add a renderer fixture with a dark room, one point light, opaque voxel
   occluders, and one prepared cuboid figure. Record the stored-light-only
   baseline.
2. Define the neutral finite-radius point-light record and diagnostics. Render
   one unshadowed diffuse point light in mono and multiview. Terrain currently
   lacks normals, so compare fragment-derived geometric normals with an
   explicit compact face-normal attribute before changing the mesh contract.
3. Add a fixture-local GPU occupancy buffer and a block-only DDA visibility
   query. Inspect first pixels before scaling the light list or world
   integration.
4. Add animated figure-part OBB occlusion onto terrain and measure ray/entity
   broad-phase pressure. Treat self-shadowing as a separate acceptance step.
5. Implement a small raster cubemap baseline, ideally with an entity-only
   option, against the same fixture. Compare stability, silhouette quality,
   GPU time, memory, and XR reuse rather than choosing from theory.
6. Integrate replicated emitter indexing, block-occupancy revisions, light
   admission, hysteresis, and static cache invalidation through the shared
   scene/render-session boundary.
7. Scale the fixture to dense torch layouts, moving entities, live block edits,
   periodic seams, and the available desktop/web/Android/XR matrix.

Clustered/tiled lists are the likely scalable destination, but the first
single/few-light proof may use a bounded uniform list behind the same neutral
contract. Do not make a small hard-coded shader array the source-selection or
quality-policy owner.

## Evidence And Validation

Every pixel-producing slice must capture and inspect output before adding the
next layer. Keep captures under `/tmp` as required by the repository policy.
The minimum fixture matrix is:

- one torch in a closed dark room;
- the same torch behind a one-block wall, proving no direct leak;
- a doorway/corner proving stored-light fill survives where direct light is
  occluded;
- a prepared creature walking between the torch and a wall/floor;
- a creature receiving terrain, self, and another-creature shadows as those
  stages become supported;
- a moving/held light with stable selection and no cache corruption;
- 100 visible torches, including shadow-budget handoff and overflow behavior;
- block placement/removal inside and outside a cached light volume;
- daylight/outdoor exposure showing the dynamic term does not wash out the
  stored sky-light presentation;
- mono and synthetic stereo captures, plus real multiview/device validation
  when a capable adapter is available; and
- web/WASM build and representative mobile/XR performance before accepting a
  technique as shared default.

Diagnostics should report at least visible/admitted lights, lights per
tile/cluster and overflow, selected shadow casters, selection churn, cache
hits/invalidations, cubemap faces rendered, DDA ray/step counts or estimates,
entity broad-phase/primitive tests, and GPU pass timings. Technique comparisons
must use the same fixture, camera, light records, output size, and quality goal.

## Open Decisions

- Exact stored-block-light/direct-light composition and exposure behavior.
- Default versus optional presentation profile and per-platform quality tiers.
- Terrain normal representation and compatibility with placed/periodic meshes.
- GPU occupancy layout, partial-block opacity, and topology-aware DDA.
- Direct per-fragment queries versus a half-resolution/temporal shadow mask.
- OBB/capsule accuracy, self-shadow rules, and entity-only cubemap fallback.
- Cubemap format, resolution, atlas allocation, static/dynamic layering, and
  cache update cadence.
- Shadow candidate scoring, hysteresis duration, fades, and reserved hero
  slots.
- Translucent receivers/casters, liquids, particles, and emissive materials.
- Soft flame sampling without visible temporal noise or cache instability.
- Cross-world lighting/occlusion when multiple embedded worlds overlap in
  composition space.
- Whether a spot-light extension should share the first production tactical or
  follow after the point-light contract is proven.

## Historical Reference Points

- Original Quake's offline `LIGHT` tool performed point-to-point BSP visibility
  traces from lights to lightmap samples. This is conceptually close to voxel
  DDA, but it was baked rather than a per-fragment runtime query:
  [`TRACE.C`](https://github.com/id-Software/Quake-Tools/blob/master/qutils/LIGHT/TRACE.C)
  and
  [`LTFACE.C`](https://github.com/id-Software/Quake-Tools/blob/master/qutils/LIGHT/LTFACE.C).
- GLQuake runtime dynamic lights marked affected surfaces and rebuilt their
  lightmap contribution, or used additive `gl_flashblend`; they did not provide
  dynamic entity-cast shadows:
  [`gl_rsurf.c`](https://github.com/id-Software/Quake/blob/master/WinQuake/gl_rsurf.c)
  and
  [`gl_rlight.c`](https://github.com/id-Software/Quake/blob/master/WinQuake/gl_rlight.c).
- Doom 3 created per-light mesh silhouette volumes and classified screen pixels
  through stencil, demonstrating crisp dynamic entity shadows without shadow
  textures while also demonstrating the per-light/view complexity:
  [`Interaction.cpp`](https://github.com/id-Software/DOOM-3/blob/master/neo/renderer/Interaction.cpp)
  and
  [`tr_stencilshadow.cpp`](https://github.com/id-Software/DOOM-3/blob/master/neo/renderer/tr_stencilshadow.cpp).
