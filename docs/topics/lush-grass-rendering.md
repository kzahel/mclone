# Lush Grass Rendering

Topic: `lush-grass-rendering`

Status: implementation active; static grass, quality/settings, wind, and
interaction are complete. Platform/performance closeout remains.

## Scope

This topic records the visual target, external research, local reference
snapshot, independent mclone architecture, platform policy, and recommended
implementation sequence for dense procedural grass. The motivating reference
is Leonardoinc22's **Grassier Grass** Minecraft mod, which places many short
blades on exposed grass surfaces, gives them biome-correct color and lighting,
moves them through coherent wind, and bends them around entities.

This is not a request to port or copy Grassier Grass. Its distributed metadata
marks it **All Rights Reserved**, and its upstream source is not published as of
2026-07-18. The intended result is an original Rust/WGSL implementation that is
inspired by the visible effect and broad rendering techniques while fitting
mclone's shared section compiler, render session, renderer, scene, and
mono/stereo/multiview contracts.

Bushy canopy geometry is a related but independent presentation concern.
[`bushy-leaf-rendering.md`](bushy-leaf-rendering.md) records the Better Leaves
research and the implemented separate `Leaf Detail: Blocky / Bushy` control.
Leaf Detail is now the first field in schema-1 `ClientGraphicsPreferences`;
Grass Detail should add its own field to that document rather than coupling
the two effects. Grass and leaves may eventually share world-space wind facts,
but they should not share eligibility, geometry caches, or a mandatory on/off
switch.

Implementation is governed by
[`../tactical/238-lush-grass-rendering.md`](../tactical/238-lush-grass-rendering.md).
The shared patch artifact, static renderer, and quality/setting path are
complete. Off remains the default on every host while the renderer accepts
explicit Sparse, Lush, and Ultra choices.

## 2026-07-24 Off Baseline

The baseline was captured from tactical commit `70a74978` before an enabled
grass path existed. Existing unrelated documentation edits made the worktree
dirty; the executable and source revision were otherwise fixed.

- `pnpm --silent native:desktop-offscreen:smoke`: `64` resident sections,
  `12` drawn sections, `0` GUI commands, `2` drawn actors. The inspected
  `/tmp/mclone-desktop-offscreen.png` showed the expected grass-block terrain
  with no presentation blades.
- `pnpm --silent native:xr-emulation:smoke`: `42` resident sections, `8` drawn
  sections, `249,588` differing eye pixels, `32` GUI commands, and `2` eye UI
  composites. The inspected side-by-side image had coherent terrain and
  ordinary binocular disparity.
- `pnpm --silent native:timedemo:smoke`: `1,936` compiled sections,
  `2,084,964` vertices, `521,241` faces, `664` loaded sections, `309.300`
  average drawn sections, and `3.539 ms` average frame time across `60`
  debug-optimized frames. This is a conservation baseline, not a release GPU
  performance claim.
- Grass patch, resident, upload, draw, template, wind, and interaction counts
  are all structurally zero because no such runtime resource existed.

The initial implementation preserves that Off result by carrying an explicit
request bit through both native and browser compilers. An Off request never
calls patch discovery and serializes no patch payload.

## 2026-07-24 Static Renderer Milestone

The request-gated patch artifact now has a shared GPU owner in
`mclone-render`. Each world draw store owns one lazy, range-managed patch
arena, while compatible worlds share six lazily materialized pipelines:
direct, direct multiview, placed, placed multiview, clipped placed, and
clipped placed multiview. Off builds contain no patches, allocate no patch
arena, and never materialize a grass pipeline.

The original static geometry derives six deterministic tapered blades from
each 32-byte patch instance. It reuses terrain view, periodic-topology,
lighting, fog, color-profile, placement, clip-plane, depth, and culling
contracts. Grass draws in the opaque phase after solid/cutout terrain, with
one instanced draw per visible resident section. Upload and render reports
expose patch, estimated blade, draw-call, and byte counts independently of
terrain faces.

Validation at this milestone:

- `cargo test -p mclone-render --lib`: `172` passed, `9` ignored.
- The ignored GPU pipeline test materialized and validated all six grass
  shader/pipeline variants on the local adapter.
- The ignored GPU visual test compared an Off image with a 35-patch enabled
  image and required more than 500 changed pixels.
- The inspected `/tmp/mclone-238-static-grass-on.png` showed deterministic
  green tapered blades rooted on the test surface; the paired
  `/tmp/mclone-238-static-grass-off.png` retained the exact bare surface.

## 2026-07-24 Quality And Settings Milestone

`Grass Detail: Off / Sparse / Lush / Ultra` is now an independent shared
Graphics setting. It projects through `mclone-ui`, client-experience effects,
`mclone-scene`, schema-1 graphics preferences, native file storage, and
browser localStorage. Missing fields restore Off. Crossing Off/non-Off
recompiles resident sections because patch discovery changes; enabled-tier
changes only select deterministic template prefixes and LOD bands.

Initial profiles use 64/128/192-block radii for Sparse/Lush/Ultra and
2/6/8 near blades respectively. Middle/far density falls to 1/1, 4/2, and
6/3. A four-block hysteresis margin stabilizes the near/middle/far
transitions. LOD state belongs to each world draw owner and observer; stereo
eyes and full-frame multiview share a head-center plan, while placed and
periodic worlds calculate source-space distance correctly.

The browser smoke uncovered and fixed a shared request-boundary omission:
the incremental browser sync path had bypassed native request decoration,
silently losing biome zoom seed, topology, and grass policy. Both paths now
decorate the neutral compile request through the same render-session hook.
Headed Wayland evidence restored Lush and reported `14,033` resident patches,
`5,277` drawn patches, about `27,078` blades, and `42` grass draws. The
inspected browser and native app captures showed dense grass rooted on exposed
grass blocks. The isolated GPU tier fixture also proved strictly increasing
Sparse/Lush/Ultra pixel coverage while Off retained the bare surface.

## 2026-07-24 Wind Milestone

Wind is now a shared, analytic source-world deformation rather than
camera-relative animation. The scene supplies one safely rebased monotonic
presentation time per frame. Both XR eyes share it, and placed worlds deform
before source-to-composition mapping. Each world owns a lazy 32-byte wind
uniform beside its patch arena, so Off retains no wind resource or update
work.

The blade template has two tapered vertical segments. Broad and clump-scale
fields combine with stable blade resistance, resting lean, and flutter;
quadratic height influence keeps roots fixed. Packed skylight attenuates
sheltered motion. Sparse/Lush/Ultra use 0.10/0.14/0.17-block wind amplitudes
without changing patch buffers.

The fixed-camera GPU fixture changed `3,979` pixels between time 0 and 3.25
seconds. All six direct/placed/clipped mono/multiview pipeline variants
validated. Native full-frame, synthetic stereo, and headed Wayland browser
captures passed and were inspected; the browser retained the established
Lush counts of `14,033` resident patches, `5,277` drawn patches, about
`27,078` blades, and `42` grass draws.

## 2026-07-24 Interaction Milestone

Grass interaction is presentation-only state shared by every host. The scene
collects the local player plus stable remote-player and entity identities into
a fixed, portable set of at most 24 source-world footprints. The renderer
checks those footprints against the actual resident grass-patch roots, so an
actor above or below the surface cannot bend unrelated grass.

Each grass-bearing world owns four 128x128 RGBA8 fields, enough for the four
supported presentation observers. Lush and Ultra activate a 0.5-block cell
field; Sparse leaves it disabled. Direction and strength accumulate under
radial footprints and interpolated motion samples, then recover
exponentially with a 0.85-second time constant. Camera motion shifts retained
cells, discontinuous movement and topology changes reset them, and periodic
worlds use the nearest lifted coordinates across their seams. Released fields
forget their observer, topology, trail, and clock before reuse.

Placed worlds consume source-world footprints. A bounded miniature centers
its interaction field on its source bounds rather than the distant
inverse-mapped physical camera, which keeps Local Play and embedded previews
interactive even at small placement scales. Direct, placed, clipped,
per-eye, and full-frame multiview shaders all sample the same source-world
deformation after wind and before placement. Stereo eyes share one field.

The GPU lifecycle and visual fixtures prove stamping, decay, recentering,
periodic seams, height rejection, contact pixels, and recovery toward an
unstamped control at the same wind time. The placed-terrain fixture rendered
and inspected interacting grass in mono and stereo, and exercised the
multiview pipeline on the local adapter. Headed Wayland browser WebGPU
reported one field, 54 active cells, eight stamps, and one aligned 65,536-byte
upload while rendering 14,033 resident grass patches.

## Attribution And External References

- **Mod:** [Grassier Grass on Modrinth](https://modrinth.com/mod/grassier-grass)
- **Author credit:** `Leonardoinc22` in the Modrinth/Fabric metadata; the linked
  GitHub account is `Leo-T22`.
- **License:** `LicenseRef-All-Rights-Reserved` in the pinned Modrinth project
  metadata and `All Rights Reserved` in `fabric.mod.json`.
- **Issue repository:**
  [Leo-T22/grass-mod](https://github.com/Leo-T22/grass-mod). Its README says it
  is an issue tracker and will host source later. The Modrinth project metadata
  has an `issues_url` but no `source_url`.
- **Versions:**
  [Modrinth version list](https://modrinth.com/mod/grassier-grass/versions).
- **Pinned artifact:** Fabric `1.4.5` for Minecraft `1.21.1`, Modrinth version
  id `CaNeiM0u`, published 2026-07-16.
- **Artifact URL:**
  [grassiergrass-fabric-1.4.5+mc1.21.1.jar](https://cdn.modrinth.com/data/3jCB9cn2/versions/CaNeiM0u/grassiergrass-fabric-1.4.5%2Bmc1.21.1.jar)
- **Author discussion:**
  [Dynamic grass mod I'm releasing soon](https://www.reddit.com/r/feedthebeast/comments/1uid0l7/dynamic_grass_mod_im_releasing_soon_grassier_grass/).
  The thread confirms the intended randomly varying wind and entity bending,
  and contains early discussion of skylight-based shelter. Treat comments as
  development-era context; the pinned JAR is the implementation evidence.
- **Decompiler:** [CFR 0.152](https://www.benf.org/other/cfr/).

Do not redistribute the pinned JAR, its extracted assets, or the reconstructed
Java. Do not copy shader text, textures, constants as a collection, or
decompiled control flow into tracked mclone code. Re-derive the behavior from
the principles and mclone's own requirements.

## Local Ignored Reference Snapshot

The repository-wide `reference/` directory is gitignored. The current workspace
contains this research snapshot:

```text
reference/grassier-grass/v1.4.5-mc1.21.1-fabric/
  artifact/
    grassiergrass-fabric-1.4.5+mc1.21.1.jar
  extracted/
    assets/grassiergrass/shaders/core/
    assets/grassiergrass/textures/
    assets/grassiergrass/lang/en_us.json
    com/leonardoinc22/shortgrass/**/*.class
    fabric.mod.json
    grassiergrass.mixins.json
  decompiled/
    com/leonardoinc22/shortgrass/**/*.java
    summary.txt
  metadata/
    modrinth-project.json
    modrinth-project-versions.json
  tools/
    cfr-0.152.jar
```

The files under `extracted/assets/**` are source-form resources shipped in the
JAR. The files under `decompiled/**` are CFR reconstructions of class files,
not upstream source. CFR did not receive the Minecraft dependency classpath or
Yarn named mappings, so Minecraft dependencies retain Fabric intermediary
names such as `class_2680`. The mod's own class and method names are still
mostly informative. If a later investigation needs fully named Minecraft
types, remap a copy of the pinned JAR from `intermediary` to `named` using a
pinned Minecraft 1.21.1 Yarn mapping release before running CFR again; retain
the original snapshot and record the mapping version and hashes.

Pinned SHA-256 values:

```text
762ee36b22eb05e24d4fad54af20cc47c271bfa15845a810cb6e3d311b33d872  grassiergrass-fabric-1.4.5+mc1.21.1.jar
f686e8f3ded377d7bc87d216a90e9e9512df4156e75b06c655a16648ae8765b2  cfr-0.152.jar
```

Useful local entry points:

- [`GrassRenderPass.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassRenderPass.java)
  integrates cache build, wind particles, normal rendering, Iris rendering,
  and invalidation.
- [`GrassSectionBuilder.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassSectionBuilder.java)
  discovers eligible blocks and plants and emits deterministic geometry,
  biome tint, light, snow, and per-blade variation.
- [`GrassSectionCache.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassSectionCache.java)
  owns asynchronous section builds, upload budgets, dirty/light invalidation,
  LOD refresh, and eviction.
- [`GrassGeometry.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassGeometry.java)
  owns blade dimensions and the three LOD tiers.
- [`GrassSectionBuildBuffers.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassSectionBuildBuffers.java)
  shows that the normal path bakes blade vertices into section VBOs.
- [`GrassDrawDispatcher.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassDrawDispatcher.java)
  performs section-frustum culling, per-section draws, trail binding, and the
  Iris compute compatibility path.
- [`GrassTrailField.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassTrailField.java)
  owns the camera-centered entity interaction field.
- [`GrassShaderUniforms.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassShaderUniforms.java)
  advances wind direction, noise scroll, flutter, and trail uniforms.
- [`GrassComputeAnimator.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/client/render/GrassComputeAnimator.java)
  contains the embedded compute shader used with Iris.
- [`GrassConfig.java`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled/com/leonardoinc22/shortgrass/config/GrassConfig.java)
  contains defaults and dynamic-wind policy.
- [`grass_blades.vsh`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/extracted/assets/grassiergrass/shaders/core/grass_blades.vsh)
  is the normal blade vertex shader.
- [`grass_blades.fsh`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/extracted/assets/grassiergrass/shaders/core/grass_blades.fsh)
  applies shape, color gradient, shimmer, cutout, brightness, and fog.
- [`grass_plant.vsh`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/extracted/assets/grassiergrass/shaders/core/grass_plant.vsh)
  animates re-rendered ordinary vegetation.
- [`en_us.json`](../../reference/grassier-grass/v1.4.5-mc1.21.1-fabric/extracted/assets/grassiergrass/lang/en_us.json)
  provides a compact inventory of user-facing settings.

If the ignored snapshot is absent, rehydrate it without changing tracked files:

```bash
mkdir -p reference/grassier-grass/v1.4.5-mc1.21.1-fabric/{artifact,metadata,tools}
curl -fL \
  'https://cdn.modrinth.com/data/3jCB9cn2/versions/CaNeiM0u/grassiergrass-fabric-1.4.5%2Bmc1.21.1.jar' \
  -o 'reference/grassier-grass/v1.4.5-mc1.21.1-fabric/artifact/grassiergrass-fabric-1.4.5+mc1.21.1.jar'
unzip -q \
  'reference/grassier-grass/v1.4.5-mc1.21.1-fabric/artifact/grassiergrass-fabric-1.4.5+mc1.21.1.jar' \
  -d reference/grassier-grass/v1.4.5-mc1.21.1-fabric/extracted
curl -fL \
  'https://repo1.maven.org/maven2/org/benf/cfr/0.152/cfr-0.152.jar' \
  -o reference/grassier-grass/v1.4.5-mc1.21.1-fabric/tools/cfr-0.152.jar
java -jar reference/grassier-grass/v1.4.5-mc1.21.1-fabric/tools/cfr-0.152.jar \
  'reference/grassier-grass/v1.4.5-mc1.21.1-fabric/artifact/grassiergrass-fabric-1.4.5+mc1.21.1.jar' \
  --outputdir reference/grassier-grass/v1.4.5-mc1.21.1-fabric/decompiled \
  --silent true
curl -fL 'https://api.modrinth.com/v2/project/grassier-grass' \
  -o reference/grassier-grass/v1.4.5-mc1.21.1-fabric/metadata/modrinth-project.json
curl -fL 'https://api.modrinth.com/v2/project/grassier-grass/version' \
  -o reference/grassier-grass/v1.4.5-mc1.21.1-fabric/metadata/modrinth-project-versions.json
```

Verify the JAR hash before relying on the reconstruction.

## Observed Grassier Grass Architecture

The observations below come from the pinned distributed artifact, not from
upstream source. Decompiled names and exact implementation details may change
in later releases.

### Frame Integration And Configuration

`GrassRenderPass.render` is a client render-stage integration point. Per frame
it:

1. resolves normal versus Iris rendering and optional compute compatibility;
2. notices geometry-affecting configuration changes and flushes the cache;
3. advances dynamic wind and optional grass-blade particles;
4. derives camera section coordinates and horizontal/vertical grass radius;
5. evicts sections outside the configured range;
6. submits and uploads a bounded number of section builds; and
7. draws cached blade sections and separately re-rendered plant sections.

The mod is client-only. It does not add grass to authoritative world state.
Block, chunk, and lighting mixins mark corresponding cached sections dirty.

User-facing configuration includes:

- blades per block and grass sparsity;
- blade height, height variation, and width;
- render radius;
- manual or dynamic wind speed/direction and a dynamic-wind limit;
- tapered or segmented blade style;
- replacement of short/tall grass plants with blades;
- dense flowers and wind-carried blade particles;
- grass through snow and shader-pack shadows;
- brightness, per-blade hue jitter, and base/tip gradient controls; and
- plant blacklists and whitelists.

### Section Discovery, Eligibility, And Caching

The section builder scans every block in a candidate 16x16x16 render section.
It emits blades for exposed grass blocks, an optional external grass slab, and
configured grass/fern/whitelisted plant replacements. Ordinary supported
plants can instead retain their baked-model quads and be re-rendered through a
separate swaying-plant path.

A grass-block patch is skipped when the top is not available, except for the
configured snow-layer case. The builder samples light at the exposed surface,
obtains block tint through Minecraft's block-color service, and stores stable
world-space noise coordinates. Per-block deterministic random placement avoids
frame-to-frame popping.

Section builds run on a single worker with bounded submission, in-flight, and
upload work. Cached meshes retain section bounds, LOD tier, vertex buffers,
light-patch information, build age, and optional Iris compute resources.
Sections are culled against the camera frustum and drawn independently.

### Geometry And The Important Non-Instancing Finding

The normal Grassier Grass path is **not hardware-instanced**. For every blade,
`GrassSectionBuilder` emits one quad per vertical segment into a section build
buffer. `GrassDrawDispatcher` later issues one vertex-buffer draw per visible
section. The render strategy is therefore:

- deterministic CPU placement;
- static per-section vertex buffers;
- section batching and culling;
- distance-dependent density/topology; and
- vertex-shader deformation without per-frame CPU vertex rebuilding.

The Iris path has additional input/output buffers. A compute shader transforms
vertices into buffers that Iris can render through its terrain pipeline. This
is a compatibility solution, not evidence that compute is required for the
ordinary wind effect.

Pinned `1.4.5` defaults and LOD facts:

| Fact | Value |
|---|---:|
| default blades per block | 48 |
| default render radius | 100 blocks |
| configured blade height | 0.35 blocks |
| common visual-height multiplier | 1.45 |
| default blade-width multiplier | 1.2 |
| LOD ring ends | 55%, 65%, 100% of radius |
| LOD density multipliers | 100%, 70%, 15% |
| resulting default densities | 48, 34, 7 blades/block |
| vertical blade segments | 5, 3, 2 |
| Iris compute animation tiers | nearest tier only |

These values characterize the reference; they are not proposed mclone
defaults.

### Wind And Blade Shape

The normal blade vertex shader receives a wind direction, wind strength,
scrolling noise offset, flutter phase, section offset, blade/style controls,
and the entity trail field. It uses a repeating world-space noise texture for
broad coherent gusts and stable hashes/baked noise channels for local blade
character.

The deformation keeps roots planted and reconstructs a curved spine from a
base, two control points, and a tip. Wind rotation, resting curvature, clump
inward lean, cross-wind flutter, and interaction displacement contribute to
the control points. Upper vertices move farther than lower vertices, producing
a convincing flexible blade rather than a rigid rotating card.

Other important visual devices are:

- broad stationary height patches plus stable per-blade height variation;
- shorter/wider and taller/thinner correlation;
- baked Voronoi-style clump character and inward resting lean;
- per-blade resistance, curve, and flutter phases;
- a limited near-camera facing correction to reduce edge-on disappearance;
- camera-distance fade on that correction to reduce far shimmer;
- rolling brightness modulation from the same coherent wind field;
- tapered and segmented shape textures;
- bottom-to-tip brightness gradients; and
- fog and Minecraft lightmap sampling.

Wind strength is multiplied by the packed skylight response. Because skylight
is a sky-access value rather than the current day/night brightness, sheltered
grass can be calmer without stopping all wind at night.

### Biome Color And Lighting

Biome matching is primarily a CPU mesh-build decision, not a shader guess.
The section builder asks Minecraft's block-color service for tint at each grass
position. Vanilla routes grass blocks, grass, ferns, and tall grass through
`BiomeColors.getAverageGrassColor`, including biome climate color, overrides,
swamp/dark-forest modifiers, and the configured biome blend.

The blade vertices carry that result and the packed surface light. A stable
per-blade hue perturbation breaks uniformity. The shaders then apply lightmap,
height gradient, wind shimmer, brightness, snow mixing, and fog.

Relevant Minecraft 1.17.1 reference files for mclone's own parity target are:

- [`BiomeColors.java`](../../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/BiomeColors.java)
- [`BlockColors.java`](../../reference/minecraft-1.17.1/src/net/minecraft/client/color/block/BlockColors.java)
- [`Biome.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/Biome.java)
- [`BiomeSpecialEffects.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/BiomeSpecialEffects.java)
- [`GrassColor.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/GrassColor.java)

### Entity Interaction Field

`GrassTrailField` maintains a camera-centered 2D field rather than touching
each blade on the CPU. Pinned facts are:

| Fact | Value |
|---|---:|
| texture size | 128x128 |
| covered world area | 48x48 blocks |
| cell size | 0.375 blocks |
| camera recenter quantum | 8 cells / 3 blocks |
| base stamp radius | 0.5 blocks |
| shader strength | 0.6 |
| per-frame retained strength | 0.998 |

Each active nearby entity is checked against the field bounds and against a
nearby grass/vegetation surface. Its footprint determines a stamp radius. The
stamp stores an outward horizontal direction and accumulated strength with a
smooth radial falloff. Active cells decay and are removed below a threshold;
the dirty texture region is uploaded.

The blade shader samples direction and strength at the blade root, suppresses
some wind/resting curve/flutter, and adds a strong bend away from the entity.
Because the field persists and decays, grass recovers behind an entity rather
than snapping upright immediately.

### Iris, Plants, Snow, Flowers, And Particles

The main visual effect does not require these features, but they explain the
larger code surface:

- Iris compatibility re-encodes blades as terrain and uses compute to animate
  the nearest tier. Farther tiers can be transformed once and baked static.
- Ordinary grass-like plants may have their vanilla model hidden and be
  re-rendered through a swaying plant shader.
- Short/tall grass and ferns can be converted to generated blades with height
  classes.
- Dense flower patches can add deterministic companion flowers.
- Snowy climates alter blade height/coverage and blend white toward tips or
  across short blades; grass-through-snow is separately configurable.
- Strong wind can spawn occasional detached blade particles.

These are later enhancements, not first-slice requirements for mclone.

## Why The Effect Is Convincing

The dramatic appearance is the combination of several modest techniques:

1. **Coverage:** enough blades exist to replace the visual reading of a flat
   grass texture with a field volume.
2. **Correct attachment:** blades appear only on appropriate exposed surfaces
   and remain fixed to stable world positions.
3. **Color continuity:** blades inherit the same biome blend and lighting as
   the underlying grass rather than looking like a foreign overlay.
4. **Correlated variation:** broad patches, small per-blade differences, and
   clumps create structure at multiple scales.
5. **Flexible motion:** roots remain fixed while tips and control points react
   progressively to wind.
6. **Coherent gusts:** neighboring blades share a moving low-frequency field,
   while resistance and flutter keep them from moving identically.
7. **Interaction memory:** the trail field bends many blades cheaply and lets
   the impression linger.
8. **Distance management:** density and topology fall with distance before
   individual blade detail becomes useful.

A shader alone can animate existing geometry, but it cannot decide which
surfaces receive grass, build/cull/cache that geometry, carry biome tint and
light, manage LOD, or preserve entity trails. The complete feature is a small
render subsystem.

## Existing Mclone Foundations

Mclone already has most of the surrounding contracts:

- [`mclone-mesh/src/tint.rs`](../../native/crates/mclone-mesh/src/tint.rs)
  implements Java-shaped grass, foliage, and water tint, including the biome
  blend and swamp/dark-forest behavior. Grass patches should reuse this result
  rather than invent a second biome-color path.
- [`mclone-mesh/src/data.rs`](../../native/crates/mclone-mesh/src/data.rs)
  defines section-keyed CPU mesh artifacts and resident metadata.
- [`mclone-render-session/src/mesh_inputs.rs`](../../native/crates/mclone-render-session/src/mesh_inputs.rs)
  builds section inputs from client snapshots with biome/light/topology facts.
- [`mclone-render-session/src/section_cache.rs`](../../native/crates/mclone-render-session/src/section_cache.rs)
  and
  [`upload.rs`](../../native/crates/mclone-render-session/src/upload.rs)
  own shared cache/upload policy.
- [`mclone-render/src/chunk.rs`](../../native/crates/mclone-render/src/chunk.rs)
  owns section GPU resources, culling, phase draws, per-view slots, and
  full-frame multiview terrain patterns.
- [`mclone-scene`](../../native/crates/mclone-scene/src/lib.rs) owns shared
  frame orchestration and should supply neutral effect/settings/interactor
  facts rather than rendering policy living in an app crate.
- [`performance.md`](performance.md) owns current desktop and Quest baselines.
  Its measured Quest RD10 GPU floor makes dense standalone-headset grass a
  poor default until a deliberately small quality tier proves otherwise.

No current renderer performs grass instancing. That is an opportunity rather
than a requirement to imitate the reference's section-VBO approach.

## Accepted Mclone Direction

### Patch Instancing, Not Blade Instancing

Use one compact instance per eligible **grass patch/block**, not one instance
per blade and not baked vertices for every blade.

A conceptual section-local record is:

```text
GrassPatchInstance
  local root position (quantized or compact floats)
  packed biome tint
  packed block/sky light
  stable world-derived seed or sufficient coordinates to derive it
  small flags (surface/style/snow capability as later needed)
```

The GPU owns a small reusable template for each quality/LOD tier. A template
contains the topology descriptors for many blades; its vertex data or
`vertex_index` identifies blade id, side, and height segment. The vertex shader
uses patch seed plus blade id to derive offset, angle, height, width, clump,
and phase. A visible section is rendered approximately as:

```text
draw_indexed(template_index_range, base_vertex, patch_instance_range)
```

This keeps resident instance bandwidth proportional to exposed grass blocks,
not blade count. Biome tint/light are naturally shared by the patch. Changing
the selected LOD template changes blade density and vertical segments without
rebuilding the section's patch buffer.

Retain section-granular frustum culling and one draw per visible grass-bearing
section for the first implementation. Measure before considering a shared
ring allocator, GPU culling, or indirect/multi-draw batching.

### Original Geometry And Shader Shape

Begin with actual tapered strip geometry and no alpha-cutout silhouette. It
avoids small discarded fragments and should be friendlier to mclone's GPU
floor. A later segmented/texture style is optional.

The original WGSL should provide:

- deterministic low-discrepancy or blue-noise-like blade placement within a
  patch, with controlled overlap across block boundaries to hide the grid;
- stable multi-scale height, width, hue, resting-lean, and clump variation;
- world-locked coherent wind translated along a configurable direction;
- root-fixed curved-spine deformation whose influence increases with height;
- packed-light and existing biome-tint integration;
- subtle base/tip brightness and wind-linked tonal modulation;
- optional skylight/shelter attenuation; and
- fog behavior matching the terrain pass.

A compute shader is not required initially. Static patch buffers plus a vertex
shader provide animated wind without per-frame CPU geometry work. Revisit
compute only for measured GPU culling, indirect generation, or interaction
simulation benefits.

### Interaction Field

Use a small, camera-centered presentation field with encoded horizontal bend
direction and strength. `mclone-scene` supplies neutral nearby interactors from
the local player and actor presentations; it or another shared layer filters
out actors that are not near a grass surface. `mclone-render` owns the dynamic
texture/buffer and GPU sampling.

A 128x128 RGBA8-equivalent full upload is only 64 KiB, so the first
implementation can prefer simple deterministic CPU stamping and a full or
dirty-region upload over a compute pipeline. Measure CPU cost and WebGPU row
alignment before optimizing. Reset presentation history on world/session
replacement.

The field is visual-only. It must never affect server collision, movement,
world state, protocol, or persistence.

### Shared Ownership

- **`mclone-mesh`:** eligibility rules and section-keyed patch records using
  the existing biome/light/topology inputs.
- **`mclone-render-session`:** patch artifact caching, dirty synchronization,
  upload admission, resident metadata, and per-world resource lifetime.
- **`mclone-render`:** templates, instance buffers, wind/trail GPU resources,
  pipelines, shaders, culling, mono/per-eye/multiview rendering, and counters.
- **`mclone-scene`:** shared quality/settings projection, time/wind facts,
  nearby visual interactors, lifecycle reset, and frame orchestration.
- **App/platform crates:** no grass policy; they provide their ordinary
  surface/session/cadence facts only.

Do not create a broad vegetation crate before dependency pressure proves that
the existing owners cannot express the feature cleanly.

### Stereo And Multiview

The renderer is world-space and must land with mono, ordinary per-eye, and
full-frame multiview paths. Per-eye uniforms must remain independently slotted.
Wind, blade identity, and interaction displacement must be identical in world
space for both eyes.

If a near-camera facing correction is needed, derive it from one shared
head-center direction for stereo. Do not rotate the blade differently for each
eye; that would change geometry/depth between views and can be uncomfortable.
The same common geometry must feed both multiview layers.

### Quality Profiles And Platform Policy

Exact values remain measurement-driven, but the product shape should be:

- **Off:** no compilation/upload/draw cost; default for Quest/mobile initially.
- **Sparse:** short radius, few blades, one or two segments, interaction
  optional or near-only.
- **Lush:** desktop-oriented three-tier radius and full wind/interaction.
- **Ultra:** deliberately dense, long-radius transformative presentation for
  high-end desktop GPUs.

Expose quality presets first. Expert sliders for radius, density, height,
width, wind, and interaction can follow only if they remain coherent and do
not multiply validation excessively. A desktop OpenXR host may select Lush or
Ultra based on the desktop GPU; “desktop-oriented” does not mean flat-only.

Keep this `Grass Detail` profile independent from bushy `Leaf Detail`. Their
cost scales differently, especially on Quest/mobile, so a player should be
able to retain bushy leaves while reducing or disabling dense ground blades.
A future overall Graphics Quality preset may project both only after Grass
Detail has a real runtime effect and its independent graphics-preference
field. The existing Leaf Detail implementation supplies the storage,
accepted-effect save, native/web adapter, and Factory Reset pattern; it does
not supply grass geometry, caches, defaults, or performance assumptions.

Quest should remain Off by default. Instancing reduces CPU and buffer traffic,
but does not remove stereo vertex work, fragment/overdraw pressure, or the
72/90 Hz frame budget. A Sparse Quest tier is acceptable only after physical
headset measurement; it must not be inferred from desktop or AVD results.

## Invariants

- Preserve the underlying vanilla-target terrain and biome-color behavior.
- Grass is presentation-only and deterministic for a given world position and
  quality profile; it must not shimmer or relocate across frames.
- Use shared section compilation and render-session lifetime. Do not create a
  parallel desktop cache/worker/upload scheduler.
- Patch eligibility reacts to block, neighbor/top-occlusion, biome/tint, and
  light changes through the existing dirty-section system.
- Quality/LOD transitions must use hysteresis or another stable rule.
- Render radius and section LOD must follow observer-local presentation facts
  and existing bounded/periodic topology lifts; do not assume one unbounded
  desktop world.
- No per-frame CPU blade mesh rebuilds or per-eye resource allocation.
- No shared mutable per-eye uniform reused within one submission.
- A full-frame multiview path is required even when a device profile defaults
  the feature Off.
- Preserve a true Off path with no hidden steady-state GPU draw or interaction
  upload cost.
- Do not copy Grassier Grass code, shaders, textures, or configuration tables.

## Recommended Implementation Sequence

### Slice 0: Tactical And Baseline

- Create a tactical that cites this topic and states an original-design rule.
- Capture fresh release desktop movement/timedemo and rendered-section
  baselines before adding the pass.
- Define counters before optimization: patch instances, estimated blades,
  template vertices/triangles, visible grass sections, draws, resident/upload
  bytes, interaction upload bytes, and grass CPU/GPU timings where available.

### Slice 1: Static Tinted Patch Instancing

- Extend the shared section artifact with exposed-grass patch records.
- Reuse the existing biome tint and packed-light sampling.
- Add one original tapered template and instanced mono renderer.
- Add per-eye and full-frame multiview variants in the same slice.
- At the first drawable milestone, capture an offscreen screenshot in
  `/tmp`, inspect it, and iterate before continuing.
- Include plains/forest/swamp or another strong biome-boundary fixture to prove
  tint continuity, plus exposed/covered grass eligibility.

### Slice 2: Stable LOD And Quality Profiles

- Add near/middle/far templates with distinct blade counts and segments.
- Select templates per section without rebuilding patch instances.
- Add stable radius/LOD hysteresis and Off/Sparse/Lush/Ultra policy.
- Compare Off versus each tier using counters, GPU timestamps, frame tails,
  resident bytes, and screenshots at transition distances.

### Slice 3: Wind And Clump Character

- Add world-locked coherent gusts, stable per-blade response, curved-spine
  deformation, multi-scale height/width/clump character, and shelter input.
- Prove roots remain fixed and camera movement does not alter blade identity.
- Capture at least two time-separated frames and an animation/video probe;
  still inspect representative screenshots manually.
- Validate mono, per-eye, and full-frame multiview world-space agreement.

### Slice 4: Entity Interaction And Recovery

- Add the camera-centered field, lifecycle reset, entity footprint stamps,
  decay, and wind/interaction composition.
- Test local player, another actor, different footprint radii, stationary
  recovery, camera recentering, world replacement, and actors above/below the
  grass surface.
- Record field-active-cell/upload counters and verify Off has no update cost.

### Slice 5: Optional Presentation Extensions

Only after the core effect and budgets are accepted, consider:

- generated replacements for short/tall grass and ferns;
- ordinary plant sway through a shared foliage path;
- snow tips or grass-through-snow;
- detached blade particles;
- dense flower companions;
- shader-friendly shadows; and
- a measured Sparse Quest profile.

Each extension needs the same mono/per-eye/multiview coverage if world-visible.

## Validation Expectations

Rendered-output work follows [`platforms.md`](../platforms.md) and the native
pixel-validation guardrail. Screenshots and probes go under `/tmp`, never the
repository.

Minimum evidence for the eventual core feature:

- deterministic unit tests for patch eligibility, packed records, and stable
  seed/variation;
- biome-tint parity using existing `mclone-mesh` fixtures;
- section dirty/invalidation and upload-lifetime tests;
- Off-path conservation and zero grass-draw/update counters;
- headless static captures at close, LOD-boundary, and biome-boundary views;
- movement-frame and timedemo A/B comparisons for each quality tier;
- desktop interactive inspection of wind, shimmer, moire, edge-on gaps, and
  interaction recovery;
- desktop OpenXR per-eye and full-frame multiview validation; and
- physical Quest evidence before enabling any non-Off default there.

Pay particular attention to alpha/coverage shimmer, subpixel grass at the far
radius, fragment overdraw, section draw count, interaction texture uploads,
stereo view agreement, and whether dense blades obscure gameplay targets.

## Open Decisions For The Future Tactical

- Exact first-pass patch record packing and whether section-local positions
  can use integers/fixed point.
- Whether templates use a small vertex buffer with blade descriptors or derive
  all descriptors from `vertex_index`.
- The original noise construction: sampled texture, analytic value noise, or a
  hybrid; choose by measured cache/ALU behavior and visual quality.
- Exact preset radii, densities, segments, and hysteresis thresholds.
- Which surface tags qualify beyond vanilla grass blocks.
- Whether skylight alone is an adequate shelter signal or whether exposed-sky
  and neighboring occlusion deserve a separate mesh fact.
- CPU versus compute interaction-field update after the simple path is
  measured on native and browser WebGPU.
- Whether crossed geometry or a shared head-center facing correction is needed
  to avoid edge-on gaps.
- Integration order relative to shadows, weather-driven wind, ordinary plant
  sway, and particles.

The next session should begin here and in the ignored pinned snapshot, then
write the bounded tactical. It should not restart by searching for upstream
source or reverse-engineering the JAR again unless a newer pinned version is
deliberately selected and recorded.
