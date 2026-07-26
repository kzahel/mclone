# Water Reflections

Topic: `water-reflections`

Status: design exploration; implementation has not started. Sky/environment
reflection is the preferred first drawable slice, followed by a bounded
screen-space reflection proof. Per-water-body planar rendering is not the
default direction.

## Scope

This topic owns above-water reflection policy for water surfaces:

- which liquid faces may reflect;
- the distinction between environment, screen-space, and planar reflections;
- quality tiers and player-facing graphics options;
- shared mesh, render, frame-orchestration, and preference ownership;
- mono, stereo, XR multiview, web, Android, and desktop constraints; and
- experiments and evidence needed before choosing a production technique.

It does not own fluid simulation, liquid geometry parity, refraction,
underwater fog, caustics, general transparent-object ordering, or a general
reflection system for arbitrary materials. Those systems may eventually share
render resources, but above-water reflection should not silently change their
contracts.

Minecraft Java 1.17.1 remains the geometry and visibility baseline.
[`LiquidBlockRenderer.java`](../../reference/minecraft-1.17.1/src/net/minecraft/client/renderer/block/LiquidBlockRenderer.java)
is the reference for liquid corner heights and exposed face classes. Reflective
water is an optional presentation extension rather than a vanilla-parity
requirement.

## Working Decision

Start with the following policy:

1. Make every visible, upward-facing water surface eligible, regardless of
   connected body size.
2. Give flat tops the strongest and sharpest result.
3. Fade reflection strength or increase roughness continuously on sloped and
   flowing top surfaces.
4. Initially exclude vertical waterfall sides, falling columns, bottom faces,
   and back faces from screen-space reflection. They may retain cheap
   sky/horizon reflection and specular light.
5. Fill screen-space misses with the same sky/horizon reflection so lakes and
   oceans do not turn black or abruptly lose their reflection near screen
   edges.
6. Keep the disabled path compatible with the current water rendering and
   avoid reflection-only passes and allocations when the option is off.

Do **not** gate screen-space reflection by connected water-body size. Body size
is expensive to maintain across chunks and changing fluids, creates visible
threshold popping, and is a poor proxy for cost: a nearby one-block puddle can
cover more pixels than a distant ocean. Prefer projected screen coverage,
visible reflective tiles or pixels, ray-step budget, ray distance, surface
slope, and confidence.

Body or plane selection matters only for a separate planar-reflection
experiment. If attempted, select at most one dominant horizontal water height
by projected coverage for a view. Disconnected surfaces at the same height can
share that plane. Do not render once per connected water body.

## Technique Comparison

### Sky or environment reflection

Sample a sky/horizon representation using the view direction and a water
normal, blend it with the existing water through a Fresnel term, and optionally
add a restrained sun highlight.

This is the cheapest and most stable first improvement:

- no scene-color ray traversal;
- no per-body work;
- naturally supports arbitrary top surfaces;
- provides a fallback for every screen-space miss; and
- is practical on lower-power, web, Android, and XR paths.

It does not reflect nearby banks, trees, buildings, actors, or overhangs.

### Screen-space reflection

SSR reconstructs or traverses the already-rendered opaque scene using the
current view's scene color and depth. Its cost is approximately:

```text
visible reflective screen pixels × average ray steps
```

The number of water blocks or connected bodies is not itself the multiplier.
One shader invocation can handle a flat ocean, a one-block puddle, or a
sloping stream. Shape mostly affects the water normal, hit confidence, and
visible pixel coverage.

SSR is therefore a good match for arbitrary top surfaces. Its inherent limits
must remain visible in the design:

- it can reflect only geometry present in the current eye's screen image;
- it misses objects behind the camera, outside the screen, or hidden behind
  nearer opaque geometry;
- screen edges and grazing rays are fragile;
- transparent objects are absent when only opaque scene depth/color is used;
  and
- long or rough reflections need more samples, filtering, or temporal work.

The first SSR proof should be deterministic, short range, and bounded. It
should emphasize useful contact reflections from shorelines, nearby terrain,
and actors, then fade by confidence into the environment fallback. It should
not begin with temporal accumulation or promise mirror-quality oceans.

### Planar reflection

A planar reflection mirrors the camera across a water plane and renders the
scene again into a texture. It gives off-screen information and can look very
clean on a large, flat lake, but cost scales with selected planes and views.
Many independent water heights can turn it into many extra scene renders.

This is the technique for which limiting reflections to a dominant lake or
ocean can make sense. A bounded proof may use:

- at most one horizontal plane per view;
- selection by projected coverage and importance rather than connectivity;
- a quarter- or half-resolution reflection target;
- aggressive distance and content culling; and
- environment fallback on every other surface.

In XR, the reflection is view-dependent and normally needs correct rendering
for each eye. Reusing one eye's planar result risks stereo disagreement.
Planar reflection is consequently a later experiment, not the baseline water
option.

### Cheap flipped-color approximation

A vertically warped or flipped scene-color sample can provide a very cheap
stylized result between sky-only and ray-marched SSR. It is worth a small
experiment only if it survives shores, camera motion, and uneven water heights
without looking worse than sky reflection. It should be labeled as a fast
approximation, not SSR.

## Surface Eligibility

| Surface | Initial treatment | Rationale |
|---|---|---|
| Flat exposed top | Full selected reflection tier | Stable normal and highest visual value |
| Sloped or flowing exposed top | Same tier with smooth roughness/strength reduction | Avoids a non-reflective ring around shores and streams |
| Vertical waterfall or falling column side | Sky/specular only | Turbulent normals and screen-space rays are unstable for the first slice |
| Bottom or back face | No above-water reflection | Not an air-facing reflective interface |
| Camera below water | Separate future treatment | Needs underwater-side Fresnel, refraction, fog, and per-eye surface handling |
| Lava | Not water reflection | Must remain materially distinct even though it shares liquid mesh code |
| General translucent blocks | Existing translucent treatment | Must not inherit water policy merely by sharing a render phase |

An exact “flat or not” cutoff should be avoided. A continuous slope or
roughness response is less prone to popping and handles the reference liquid
corner heights naturally.

For the earliest prototype, a fragment-space geometric normal can be derived
from world-position derivatives. A production path should investigate
mesh-authored water surface-role data and smooth height-gradient normals,
possibly combined with a small procedural wave normal. Care is needed around
the diagonal of non-planar liquid quads so the two triangles do not expose a
hard lighting seam.

## Player-Facing Quality Contract

The likely graphics row is:

```text
Water Reflections: Off / Sky / SSR Low / SSR High
```

The final inventory may include `Fast` for the flipped-color approximation if
captures justify it. An `Auto` platform profile can choose among explicit
tiers, but the stored preference must represent player intent rather than a
device-specific numeric budget.

Suggested meanings:

| Tier | Contract |
|---|---|
| Off | Current/vanilla-compatible water shading; no reflection resources |
| Sky | Fresnel environment, horizon, and optional sun highlight |
| SSR Low | Short bounded rays, top surfaces, conservative confidence fade, sky fallback |
| SSR High | Longer or hierarchical traversal and improved roughness/filtering, only after measurement |

This setting belongs in the shared
[`graphics-video-settings.md`](graphics-video-settings.md) contract and the
schema-versioned `ClientGraphicsPreferences`, not in a desktop launcher or
app-local toggle. Platform capability and automatic defaults may differ;
setting semantics may not.

## Current Renderer Facts

The existing renderer has useful liquid geometry but no reflection input:

- `native/crates/mclone-mesh/src/builder.rs` computes the four liquid top
  corner heights and emits exposed top, bottom, and side faces. Water and lava
  currently enter the general translucent mesh.
- `native/crates/mclone-mesh/src/data.rs` gives
  `TexturedChunkVertex` position, UV, color, and packed light only. It has no
  normal, liquid kind, material class, or face-role field.
- `native/crates/mclone-mesh/src/packed.rs` uses the `MCLMSH01` browser Worker
  wire format and ten 32-bit words per textured vertex. Expanding that vertex
  contract requires a deliberate packed-format migration.
- `native/crates/mclone-render/src/shaders/chunk_textured.wgsl` receives the
  atlas and camera/fog data, but no readable scene color or depth and no water
  classification.
- `native/crates/mclone-render/src/chunk.rs` shares terrain pipelines across
  opaque, cutout, and translucent phases. The translucent pipeline blends,
  disables depth writes, and currently uses the cutout fragment entry point.
- `ChunkDepthTarget` is `Depth32Float` with render-attachment and copy-source
  usage, but not texture-binding usage. `ChunkMultiviewDepthTarget` is a
  two-layer array and currently has render-attachment usage only.
- `FlatScaledColorTarget` is sampleable, but a pass cannot sample the same
  texture subresource it is simultaneously writing.
- Shared frame orchestration already places opaque terrain and opaque inserted
  worlds before actors, and translucent terrain after actors where the frame
  is split. That ordering is a promising seam for capturing the opaque scene
  that water should reflect.
- Mono, stereo per-eye, and full-frame two-layer multiview are live paths.
  A water feature must support all relevant paths rather than appearing in
  only one XR mode.

When no actors or opaque insertion requires a split, the current terrain path
may submit all phases together. Enabling SSR will need to force a stable
opaque/actor/reflection-input/translucent boundary even in that simpler case.

## Required Shared Architecture

### Material and face classification

The render path must distinguish at least:

- water from lava and other translucent materials; and
- exposed top faces from sides and bottom faces.

Two plausible representations should be compared before implementation:

1. a dedicated water submesh or explicit water draw ranges carrying surface
   role, which avoids enlarging every terrain vertex but may add draw and
   ordering complexity; or
2. compact material/face flags in the textured vertex contract, which simplify
   shader branching and ordering but add vertex bandwidth and require a Worker
   ABI migration.

The decision should account for transparency sorting, web Worker traffic,
memory, and future water refraction. It must not infer water from atlas UVs in
the shader.

### Readable opaque scene

The reflection pass needs color and depth from after sky, opaque world
geometry, placed/embedded opaque geometry, and actors, but before water and
screen-space UI. A target sequence should converge on:

```text
sky
→ opaque/cutout terrain and composed worlds
→ opaque actors
→ readable reflection input
→ translucent terrain and water
→ underwater/screen effects and UI
```

Sampling and writing the same color target is invalid. The shared renderer
will need either:

- an opaque scene target A that is sampleable, followed by presentation or
  copy into target B while water writes B and samples A; or
- a deliberate snapshot/copy into a separate sampleable reflection texture.

Depth needs an equivalent sampleable representation, or a resolved/encoded
depth copy appropriate for the supported `wgpu` backends. The ray shader also
needs correct inverse-view-projection or view-space reconstruction, including
the renderer's reversed-Z convention.

Reflection-only resources should be optional and recreated with the ordinary
surface/render-size lifecycle. Off and preferably Sky mode should not pay for
an SSR color/depth snapshot.

### Ownership

| Concern | Shared owner |
|---|---|
| Water/liquid face classification and optional normals | `mclone-mesh` |
| Neutral option/capability and per-view contracts | `mclone-render-session` |
| GPU targets, pipelines, shaders, and SSR statistics | `mclone-render` |
| Frame ordering, visibility/admission, mono/stereo preparation | `mclone-app-runtime` and `mclone-scene` |
| Durable graphics preference and shared settings UI | `mclone-app-runtime` and `mclone-ui` |
| Surface, browser, Android, and OpenXR presentation | app/platform adapters only |

Placed and embedded worlds that are visible before water must be represented
in the reflection input. Screen UI, menus, and HUD must not be reflected.

### XR invariants

- Each eye samples its own color, depth, view, and projection data.
- Full-frame multiview uses correctly indexed two-layer resources and the same
  quality semantics as per-eye rendering.
- No mutable uniform or snapshot from one eye is reused as the other eye's
  view-dependent reflection input.
- Near-surface behavior must be inspected in stereo; a mono capture cannot
  prove comfort or correctness.
- Automatic quality should be conservative because ray work and scene
  snapshots are paid across both eyes.

## Performance Policy

For SSR, spend work according to projected visibility rather than world
topology:

- skip the scene snapshot and SSR work when no reflective water is visible;
- bound ray count, step count, ray length, and thickness tolerance;
- fade low-confidence and edge hits into the environment fallback;
- prefer half-resolution or checkerboard reflection only if reconstruction
  looks acceptable in motion;
- investigate water-tile classification before connected-component body
  tracking; and
- treat hierarchical depth, temporal accumulation, and denoising as later
  optimizations justified by evidence.

The engine has no established temporal-AA history on which to casually depend.
A deterministic contact-SSR proof is therefore lower risk than beginning with
stochastic rays and a temporal denoiser.

Quest and other mobile/XR targets should not default to full-resolution SSR
until the current render-distance guardrails in
[`performance.md`](performance.md) have been remeasured with both eyes and
representative water coverage. Environment reflection should remain the
portable fallback.

For planar reflection, cap the number of admitted planes rather than connected
bodies. Even a single plane is an additional scene render per required view,
so it must be compared against SSR using actual GPU timing.

## Experiment Sequence

### Stage 1: classification and environment reflection

- Add an explicit water/surface-role path without changing fluid simulation.
- Shade all exposed top surfaces with Fresnel sky/horizon reflection.
- Reduce sharpness continuously on sloped/flowing surfaces.
- Give waterfall sides only the restrained environment/specular treatment.
- Prove the disabled path preserves current output and avoids SSR resources.

This is the first drawable milestone and should be captured and inspected
before scene-color work begins.

### Stage 2: bounded contact SSR

- Establish sampleable opaque color and depth in the shared frame path.
- Trace short deterministic rays for eligible water pixels.
- Reflect opaque terrain, composed worlds, and opaque actors.
- Add explicit screen-edge, distance, grazing-angle, and missing-depth
  confidence fades.
- Use environment reflection for every miss.
- Implement mono, per-eye stereo, and full-frame multiview together.

### Stage 3: quality and budget tuning

- Compare full, half, and classified-tile resolution.
- Tune slope roughness, thickness, step count, binary refinement, and maximum
  distance from captures and GPU timings.
- Test whether a small procedural normal improves water without masking hit
  stability.
- Decide whether `SSR Low` is viable for web/mobile/XR automatic profiles.

### Stage 4: measured optional work

Only after the bounded proof:

- hierarchical depth traversal;
- rough-reflection filtering;
- temporal accumulation and disocclusion handling;
- a cheap flipped-color `Fast` tier; or
- a single-dominant-plane planar reflection proof.

## Scene Matrix

The minimum visual and motion matrix is:

1. an isolated one-block source or puddle beside solid terrain;
2. a shallow pond with varied liquid corner heights, banks, foliage, and a
   nearby actor;
3. a large ocean viewed from above and at a grazing angle;
4. a stepped or flowing stream with several fluid levels;
5. a waterfall and a large falling water column;
6. cave water below an overhang, where environment fallback can be exposed;
7. camera pans that drive reflected objects through screen edges;
8. the camera crossing the surface and moving very close to it;
9. translucent objects near water, with their absence from the opaque SSR
   input treated explicitly; and
10. mono flat, stereo per-eye, and full-frame multiview views.

At every drawable stage, capture stills and inspect them. SSR also requires a
short motion sequence because edge popping, triangle seams, shimmer, and
temporal instability may be invisible in a screenshot. Browser pixel evidence
must use the headed Wayland WebGPU path described by the repository validation
policy.

Record at least:

- reflection preparation and water-pass GPU time;
- allocated reflection texture bytes and resolution;
- approximate visible water coverage or admitted tiles;
- traced pixel count;
- average and maximum ray steps;
- hit, miss, and low-confidence rates; and
- fallback pixel count.

## Acceptance Invariants

- `Off` preserves the current water path and does not allocate or execute SSR
  resources.
- Rendering does not require persistent connected-water-body identity.
- Small top-facing puddles remain eligible when they are visible.
- Flowing top surfaces degrade continuously rather than crossing an arbitrary
  flatness or body-size threshold.
- Waterfall sides are deliberately fallback-only until evidence supports a
  stable reflective treatment.
- Water, lava, and unrelated translucent materials cannot be confused.
- Every view uses its own camera and reflection inputs; multiview is not a
  second-class path.
- UI and screen-space overlays never appear in the water reflection.
- The feature does not change server fluid behavior, chunk visibility, or
  gameplay lighting.
- Fog, color-space conversion, render scaling, resize, and target recreation
  remain correct in every quality tier.

## External Technique References

These references inform technique vocabulary and experiment design; they do
not override local renderer evidence:

- [AMD FidelityFX Stochastic Screen Space Reflections](https://gpuopen.com/manuals/fidelityfx_sdk/techniques/stochastic-screen-space-reflections/)
  documents a higher-end path using tile classification, hierarchical depth,
  stochastic intersection, and denoising. It is a possible later direction,
  not the starting scope.
- [Godot screen-space reflections documentation](https://docs.godotengine.org/en/latest/tutorials/3d/environment_and_post_processing.html)
  describes the ordinary screen-space visibility limitations that require an
  environment fallback.
- [Complementary Shaders language/configuration](https://github.com/Miracle0565/ComplementaryShaders/blob/main/en_US.lang)
  provides a useful Minecraft shader-pack precedent for exposing reflection
  quality as off, sky-only, cheap, and higher-quality modes rather than as a
  water-body-size switch.
- [Photon shader pack](https://github.com/sixthsurge/photon) is a second
  Minecraft shader implementation worth inspecting when a code-level
  comparison becomes necessary.

## Open Questions

- Dedicated water ranges or compact material/face flags?
- Is fragment-derivative geometry normal adequate for the first proof, or does
  the reference liquid topology make mesh-authored normals necessary
  immediately?
- Can the existing flat scaled-color path evolve into the shared opaque scene
  target, or should reflection own an independent snapshot contract?
- Which depth-sampling representation is portable across the actual desktop,
  browser, Android, and XR backends?
- Does half-resolution SSR remain stable enough at Minecraft's hard-edged
  shores and in headset motion?
- Is a `Fast` tier visibly useful once sky fallback exists?
- Does one dominant planar plane ever beat bounded SSR strongly enough to
  justify the extra scene render and maintenance?
