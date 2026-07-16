# Compiled Figure Rendering

Topic: `compiled-figure-rendering`

Status: target direction selected 2026-07-16. Tactical
[`181`](../tactical/181-compiled-figure-static-box-proof.md) is proposed for
the first bounded static-box proof; instancing, LOD, and GPU pose evaluation
remain later measured stages.

## Scope

This topic tracks the evolution from the current CPU-baked combined
actor mesh to a compiled figure pipeline with static GPU geometry, rigid-part
animation, shared instances, and generated figure LODs.

It covers the contract between the Asset Lab authoring format, derived asset
artifacts, `mclone-assets`, actor presentation, and `mclone-render`. It does not
change authoritative entity simulation, protocol identity, AI, or spawning.
Those remain owned by the entity/runtime architecture.

The architectural target is selected, while artifact details, optimization
thresholds, and later LOD policy remain open. The purpose of this document is
to preserve current evidence and decisions, identify the tradeoffs that need
measurement, and keep small actor-cache work from accidentally hardening an
interim representation into the long-term figure architecture.

## Motivation

The immediate performance issue is documented as HP-1 in
[`performance.md`](performance.md#hp-1-split-actor-pose-updates-from-whole-mesh-rebuilds).
`ActorMeshCache` currently compares the complete actor list. When any actor
position, orientation, light, color, walk distance, or figure pose changes, it
CPU-bakes one combined world-space mesh and uploads all vertices and indices.
Stationary actors are rewritten because one actor moved.

Persistent grow-only GPU buffers and first-eye/second-eye reuse already fixed
the old per-eye allocation defect. The remaining problem is coarse CPU
invalidation and upload, not a GPU-resource leak.

A topology/pose split and index reuse are worthwhile bounded cleanup. However,
the Asset Lab source format supplies a stronger long-term opportunity:

- figures are named hierarchical rigid parts;
- every primitive belongs to exactly one part;
- pivots, parent relationships, base transforms, and clips are explicit;
- boxes, spheres, capsules, and cylinders have deterministic topology inputs;
- ASCII textures are compact source data under project control; and
- all supported runtime and tooling code is owned in this repository.

That is already the information needed to compile static local-space geometry
and animate it with one transform per rigid part. General weighted skinning is
not required for the current source format.

## Current Pipeline And Its Deliberate Approximations

The current pipeline is:

```text
tools/asset-lab/examples/<figure>/figure.ts
  -> Asset Lab semantic FigureAsset
  -> direct Three.js preview
  -> exported schema-v1 figure JSON
  -> first-party asset-pack entry
  -> mclone-assets JSON loader
  -> mclone-render cuboid compiler
  -> CPU-baked world-space actor mesh
```

The authoring/export side preserves semantic primitives. Asset Lab preview
uses Three.js `BoxGeometry`, `SphereGeometry`, `CapsuleGeometry`, and
`CylinderGeometry`, including authored/default segment counts. Export writes
the semantic JSON; it does not write the preview's final vertex/index data.

The native bridge was intentionally narrower:

- `sphere`, `capsule`, and `cylinder` compile to their cuboid bounds;
- a box ASCII face texture becomes one thin colored overlay cuboid per texture
  cell instead of ordinary UV-mapped texture data;
- the actor vertex contains baked world position, UV, color, and packed light;
- figure animation is sampled on the CPU and immediately baked into every
  affected vertex; and
- authored clip scale is parsed as source data but is not part of the current
  compiled runtime transform;
- all visible actors share one mutable combined mesh per drawable world.

These choices proved asset ownership, figure selection, networked appearance,
clip import, first-person body filtering, and cross-platform actor rendering.
They were not intended to define final primitive tessellation or texture
ownership. Tactical
[`112`](../tactical/112-asset-lab-runtime-actor-geometry.md) records the early
bridge, while Tactical
[`118`](../tactical/118-entity-runtime-and-passive-mob-bringup.md#slice-5---chicken-asset-and-species-state)
explicitly calls true non-box rendering follow-up work.

The approximation distorts mesh size in both directions. Curved animals are
currently much simpler in-engine than in Asset Lab, while a textured box can
become much more expensive because texture cells become geometry. For example,
the authored player preview is twelve boxes and 288 Three.js vertices, but its
8x8 face texture adds 64 runtime overlay cuboids in the current bridge.

## Current Asset-Lab Geometry Evidence

The following representative counts were taken on 2026-07-16 using the same
Three.js geometry constructors and default segment counts as Asset Lab. They
describe intended preview geometry, not current native cuboid approximations,
and will change as source figures are revised.

| Figure | Parts | Vertices | Triangles |
|---|---:|---:|---:|
| player | 12 | 288 | 144 |
| piglet | 14 | 809 | 1,052 |
| dog | 18 | 991 | 1,268 |
| chicken | 21 | 2,209 | 3,108 |
| cow | 38 | 1,946 | 2,208 |
| rabbit | 25 | 3,050 | 4,640 |
| tiger | 86 | 3,648 | 3,856 |

These are not extremely large meshes, but most rounded animals are already
well above vanilla cuboid-mob geometry. The cost also multiplies by visible
actor count and animation cadence. More importantly, the pipeline should not
make CPU world-space rebaking a prerequisite for later, richer figure assets.

## North Star

Keep the TypeScript DSL or another compact semantic representation as the
editable source, but introduce one deterministic compiled-figure artifact
consumed by both review tooling and every runtime lane:

```text
figure.ts
  -> validated semantic figure description
  -> deterministic figure compiler
  -> compiled figure artifact
       static geometry + rig + materials + textures + clips + LODs
  -> Asset Lab compiled preview
  -> native / web / Android / XR runtime asset loading
```

The long-term compiler owner and promoted artifact encoding remain
revisitable; Tactical 181 makes a provisional choice for the first proof. The
invariant is more important than the implementation language: Asset Lab and
the runtime should not independently interpret primitive tessellation, UVs,
winding, or material slots and then rely on screenshot luck to remain aligned.

The semantic source remains reviewable and regenerable. A compiled artifact is
derived content, not a replacement authoring format.

Three.js retains semantic authority in this pipeline even where it loses
byte-production duty. Asset Lab exists so that AI agents and humans can author
figures fluently, and that fluency comes from Three.js scene semantics being
deeply familiar: an author writing `figure.ts` knows what `CapsuleGeometry`
parameters produce without running anything. "What a figure looks like" is
therefore defined as what Three.js renders for the semantic description, the
authoring preview stays Three.js-shaped, and the compiler's job is to
reproduce those semantics deterministically — never to invent its own
primitive interpretation that authors would have to learn. Dropping the
Three.js dependency outright is not a goal of this direction.

## Selected Direction And Deliberate Flexibility

The durable contract is the compiled figure, not a particular pose evaluator
or batching strategy:

- semantic Asset Lab source compiles offline into one canonical, versioned
  artifact;
- Asset Lab compiled preview and every runtime lane consume the same geometry,
  UV, material, rig, clip, and LOD data;
- geometry stays in local part space and is uploaded once per resident figure
  and LOD;
- actor records and final part palettes remain the small mutable payload;
- actor movement, animation phase, and clip blending are evaluated
  continuously at presentation cadence; and
- the runtime may choose exact CPU pose evaluation or a measured GPU crowd
  path without changing the asset contract.

For the first proof, a deterministic compiler module under
`tools/asset-lab` is the provisional authority. For boxes the compiler owns
the trivial tessellation directly, and Three.js is a test oracle: an equality
test against the repository-pinned `BoxGeometry` output guards drift without
making artifact bytes depend on a dependency's internals. That test is also
the semantic-authority guarantee — it pins compiler output to what the author
targeted and previewed. Extracting topology from the pinned Three.js
constructors remains the plan for spheres, capsules, and cylinders in
Phase 2, where reimplementing tessellation is genuinely expensive and where
extraction is the most direct implementation of the Three.js-semantics
contract; there the artifact must record compiler and Three.js versions or
hashes so a dependency change fails loudly instead of silently altering
output. In both modes the compiler serializes the result and makes
compiled-output preview the acceptance surface. Runtime Rust must validate
and consume those bytes rather than independently retessellating the same
primitives.

That choice is intentionally revisitable. Moving a renderer-neutral compiler
core to Rust/WASM is justified later only if offline replacement-pack
compilation, deterministic maintenance, or duplicated tooling work makes it
valuable. It is not a prerequisite for proving the contract.

Likewise, a shared sampled phase palette is a runtime-derived cache, not
canonical authored animation data. The artifact retains the clip tracks and
their semantics. A renderer may build GPU-resident sampled clips from them,
while a CPU evaluator can continue using the same tracks exactly. This avoids
making every actor count and every supported GPU pay for the crowd path.

## Candidate Compiled Contract

This is a capability sketch, not a frozen binary layout.

### Static geometry per LOD

- local-space positions;
- normals;
- UVs;
- static indices;
- one rigid part index per vertex;
- material/texture slot or material-partitioned draw ranges;
- per-part and whole-figure bounds;
- first-person/body visibility classification; and
- deterministic vertex, index, and byte counts.

Tangents are not required until a supported material needs them. Weighted bone
indices and weights are not required while one primitive belongs rigidly to
one part.

### Rig and animation

- stable part names and indices;
- parent indices;
- base translation, rotation, pivot, and optional scale;
- compiled clip tracks and locomotion/contact metadata;
- part identity preserved across every LOD; and
- an explicit policy for parts omitted from a coarse LOD.

The first runtime can continue sampling clips and resolving the parent
hierarchy on the CPU. The resulting final part matrices are the small dynamic
payload sent to the GPU. GPU clip evaluation is a separate possible future
optimization, not a prerequisite.

### Materials and textures

- generated RGBA texture data or atlas regions derived from ASCII sources;
- nearest-filtered sampling where authored;
- explicit box face mapping;
- deterministic default projections for sphere, capsule, and cylinder;
- optional projection rotation, scale, offset, and seam controls when the
  defaults are not enough;
- a deliberately supported material subset; and
- stable material slots shared across LOD variants.

Asset Lab currently previews roughness and metalness through Three.js while
the native actor path uses simpler baked face colors. The compiled contract
must either support a shared material interpretation or explicitly narrow the
preview to the runtime-supported subset. Silent material divergence should not
remain the default.

### Provenance and compatibility

- semantic schema version;
- compiler/artifact schema version;
- source and compiler-input hash;
- build settings, including LOD reduction settings;
- bounds and geometry statistics; and
- enough provenance for asset-pack lock checking and stale-artifact rejection.

## Candidate Runtime Shape

Immutable compiled figure resources should be shareable across compatible
drawable worlds:

- static vertex and index buffers for each resident LOD;
- texture/atlas resources;
- pipeline/material topology;
- part hierarchy and clip data; and
- immutable bounds and draw ranges.

Mutable state remains world- and actor-local:

- actor identity and presentation state;
- selected figure and LOD;
- world transform and scale;
- sampled part-matrix palette;
- packed light and actor-specific color/palette data;
- visibility flags; and
- interpolation/animation phase.

The basic vertex operation becomes conceptually:

```text
world_position = actor_world * part_matrix[part_id] * local_position
```

Normal transformation follows the same rigid transforms. View/projection
remains per view, preserving the existing mono, per-eye, and full-frame
multiview ownership rules.

Compiled figures take one packed light value per actor, sampled at the
actor's position, instead of the current per-vertex baked world light. A
large figure straddling a light gradient will shade slightly differently
than the CPU-baked bridge; migration pixel comparisons must treat that as a
deliberate lighting-model change, not a regression.

Actors sharing a figure can later be instanced by indexing an actor record and
that actor's part-palette base. Instancing is desirable, but a first compiled
figure proof may issue one draw per actor or figure/material group while the
static-geometry and transform contracts settle. Do not couple correctness of
the compiled asset format to the first batching strategy.

### Presentation-rate pose evaluation

Animation sample spacing and display evaluation cadence are different things.
A sampled clip with 64 samples per second must not become a 64 Hz pose hold.
At every presented frame, at whatever cadence the host/display requests, the
evaluator uses the frame's presentation time and the actor's continuous phase
to select adjacent samples and a fractional blend:

```text
phase = fract(phase_origin + elapsed * rate / duration)
x = phase * sample_count
a = floor(x)
b = (a + 1) mod sample_count
alpha = fract(x)
```

Local translation and scale interpolate linearly. Local rotations use
shortest-path normalized quaternion interpolation or slerp. The evaluator then
composes the parent hierarchy; it must not linearly interpolate already
composed matrices. Loop endpoints, non-loop clamping, normal transforms, and
clip transition weights need explicit shared semantics.

Every actor retains its own phase origin, playback rate, clip selection, blend
weights, world transform, and overrides. Many actors can therefore share the
same immutable clip samples while appearing at unrelated phases. Long clip
duration alone does not require dense samples: sample density follows motion
error, curvature, and loop continuity. Resampling a coarse piecewise-linear
source more densely cannot reconstruct smooth authored velocity, so the
semantic schema may eventually need curve/tangent interpolation modes.

Independent channels such as locomotion, wing flap, head look, damage, and
procedural aiming should compose as a small set of layers or overrides. Do not
precompute their Cartesian product into phase-palette variants.

World movement interpolation is separate from figure pose. Remote actors need
previous/target authoritative transforms and timestamps, while local actors
normally use prediction plus reconciliation. Walk phase should derive from
continuous interpolated travel distance where available. A lower server or
simulation tick rate must not appear as 20 Hz or 60 Hz stepping on a higher-
refresh display, although remote interpolation may deliberately trade a small
amount of latency for smoothness.

There is no renderer-owned maximum animation cadence. A 120 Hz, 240 Hz, or
500 Hz presentation loop receives a newly evaluated continuous pose on every
frame if the host can render at that rate. Missing a performance budget may
drop or delay a frame, but the implementation must not respond by silently
capping ordinary pose evaluation to a lower fixed frequency. After a missed
frame, evaluation samples the current presentation time rather than advancing
one fixed animation step.

The first animated compiled path evaluates final part matrices on the CPU each
presentation frame. If crowd measurements justify a GPU path, a compute pass
samples and blends the shared clip data, composes the hierarchy once per
actor/part, and writes final matrices for the instanced vertex pass. Sampling
and hierarchy work should not be repeated independently for every vertex.

## Performance Model

Static compiled figures and LOD solve different costs:

- static geometry plus part palettes reduces CPU vertex baking, serialization,
  and CPU-to-GPU upload;
- instancing reduces duplicate geometry residency and draw/setup overhead for
  actors sharing a figure; and
- LOD reduces the remaining GPU vertex processing, raster pressure, and
  potentially material cost for small projected actors.

As an illustration only, the current actor vertex is 40 bytes. Re-uploading
the Asset Lab chicken's 2,209 preview vertices would be about 88 KB before
indices, while 21 rigid 3x4 `f32` part matrices are about 1 KB. The tiger's
3,648 vertices would be about 146 KB, while 86 such matrices are about 4 KB.
The future compiled vertex layout and actor payload will differ, but the order
of magnitude explains the opportunity.

GPU part transforms do not reduce the number of vertices the GPU executes.
That is why LOD remains valuable after CPU/upload work is removed.

### Thousand-chicken design point

One intended Asset Lab chicken has 2,209 vertices, 3,108 triangles, and 21
rigid parts. One thousand full-detail visible chickens would therefore submit
about 2.2 million vertices and 3.1 million triangles per presentation frame.
At high presentation rates, vertex and raster work—not just pose upload—can
become the limiting cost on low-end hardware.

Twenty-one final 3x4 `f32` matrices for one thousand actors are approximately
1 MB per presented frame. As an accounting example, that is roughly 120 MB/s
at 120 Hz and 500 MB/s at 500 Hz, before actor records and alignment. Those are
workload examples, not supported-cadence limits. GPU pose expansion can remove
most of that upload, while generated LODs, frustum/occlusion admission, and
conservative distance policies reduce the much larger geometry cost.

Instancing remains valuable despite animation because actors share immutable
geometry and clip data while indexing different actor records and palette
bases. It only batches compatible figure, LOD, material, alpha/pass, and
pipeline state, so the implementation should report bucket fragmentation
rather than promising one literal draw for every crowd.

The target is adaptive rather than GPU-only: CPU palettes should remain the
simple exact path for ordinary populations and unsupported devices; shared
sampled clips plus GPU palette expansion should activate only when measured
actor/part counts and capabilities make them a win.

The implementation should expose at least:

- compiled vertices, indices, bytes, parts, materials, and texture bytes per
  figure and LOD;
- retained immutable GPU bytes and strong-owner count;
- actor and part-matrix bytes uploaded per frame;
- full versus ranged/instanced actor update counts;
- visible actors by figure and LOD;
- LOD transitions, rejected transitions, and residency;
- actor CPU pose-sampling and upload time;
- actor GPU pass time where timestamp support exists; and
- draw calls, instances, vertices, and triangles submitted.

## Costs, Risks, And Mitigations

This direction trades a coarse but simple runtime bridge for a real asset and
rendering subsystem:

- **Compiler ownership:** Asset Lab gains deterministic tessellation, UV,
  atlas, versioning, budget, and error-reporting responsibilities. Compiled
  preview, golden counts, dependency provenance, and stale-artifact rejection
  are required to keep that cost controlled.
- **Sampled-animation approximation:** interpolation prevents temporal pose
  holds but sparse samples can still flatten fast or curved motion. Error-based
  sampling, loop tests, and preservation of canonical clip tracks keep the
  approximation optional and measurable.
- **Animation composition:** locomotion, flap, look, damage, and transitions
  cannot be baked as every combination. A bounded layering contract and final
  per-actor palette expansion avoid a combinatorial asset explosion.
- **Batch fragmentation:** figures, LODs, materials, transparency, and passes
  split instance buckets. Diagnostics must expose real draws and instances;
  correctness must not depend on one-draw crowd assumptions.
- **GPU overhead and limits:** compute dispatches, storage buffers, barriers,
  alignment, frames in flight, and browser/mobile limits can make a crowd path
  slower for small scenes. Capability checks and the CPU evaluator remain
  first-class.
- **Vertex indirection:** every vertex fetches its rigid-part transform. The
  upload and CPU savings are expected to dominate, but GPU time must be
  measured independently on low-end and XR hardware.
- **LOD artifacts:** generated variants can pop, change silhouettes, or move
  grounding. Projected-error thresholds, hysteresis, stable part identity,
  conservative XR selection, and visual transition sheets are required.
- **Movement smoothness versus latency:** remote interpolation is smooth by
  buffering some history; prediction is responsive but needs corrections.
  Figure animation cannot paper over an undefined actor-motion contract.
- **Rigid-part ceiling:** this design fits current Minecraft-like figures but
  does not provide weighted skinning, morphs, cloth, or facial deformation.
  Those remain explicit future format extensions rather than hidden special
  cases.
- **CPU/GPU agreement:** floating-point ordering, quaternion interpolation,
  hierarchy composition, and transition timing can diverge. Shared semantic
  tests and tolerant pose/pixel comparisons must gate a GPU evaluator.

## Figure LOD Direction

Primitive source assets make deterministic generated LODs unusually
tractable. Candidate tiers are:

- LOD 0: authored segment counts and full material/part detail;
- LOD 1: reduced sphere, capsule, and cylinder segments while preserving the
  silhouette and all important animated parts;
- LOD 2: coarse primitive or cuboid silhouette, with small decorative parts
  merged or omitted under an explicit rule; and
- later only if justified: billboards or impostors for very distant actors.

This is a direction, not a commitment to three exact tiers. Important
selection invariants are:

- use projected screen size or another view-aware error estimate rather than
  distance alone;
- apply hysteresis so an actor near a threshold does not thrash LODs;
- make one conservative choice across both XR eyes;
- preserve actor identity, animation phase, part semantics, materials, and
  feet/world anchoring across transitions;
- preserve ordinary presentation-rate motion and animation evaluation; LOD is
  primarily spatial simplification, not permission to hold poses for several
  display frames;
- never regenerate or upload topology every frame because selection changed;
  variants should already be compiled and resident or admitted through an
  explicit bounded residency path; and
- keep selection and residency policy shared across host adapters.

Open design work includes segment-reduction rules, treatment of tiny parts,
normal/UV continuity, mip and filtering policy (the proof's nearest-filtered
unmipped atlas will shimmer on distant actors and must be revisited alongside
LOD), residency limits, transition policy, shadows if added, and whether
authored overrides are needed for silhouette-critical features.

## Asset Lab Evolution

With this direction selected, Asset Lab grows from a disposable semantic
preview into an authoring and compilation review surface. Useful additions
include:

- a `compile` command that emits the exact runtime artifact;
- compiled-mesh preview using the same positions, normals, UVs, indices,
  materials, and LODs consumed by the runtime;
- optional semantic-Three.js versus compiled-output comparison while the
  compiler is brought up;
- UV/seam/projection visualization;
- normal and winding visualization;
- atlas and material-slot review;
- per-LOD static sheets and animated transition review;
- geometry/texture/part/clip budget reports with actionable part names;
- deterministic-output and stale-artifact checks; and
- batch compilation integrated with first-party asset packing.

The tool should continue making semantic source easy for agents and humans to
edit, targeting the familiar Three.js scene semantics described in the North
Star. Compiled data can be binary and optimized; authored source should remain
small, typed, diffable, and regenerable.

## Relationship To The Immediate Actor-Cache Issue

The following bounded cleanup remains useful regardless of the compiled path:

1. distinguish topology, vertex, and index updates in diagnostics;
2. stop rebuilding and uploading unchanged indices on pose-only changes; and
3. preserve allocation-free unchanged frames and first-eye/second-eye reuse.

Stable per-actor CPU vertex spans may remain valuable for legacy cows, debug
cubes, item actors, unsupported figure features, and an incremental fallback.
They should not be treated as a prerequisite for compiled Asset Lab figures,
and the project should avoid a large CPU span-cache campaign before deciding
the compiled-figure contract.

The existing `ActorMeshCache` remains the production path until a compiled
path passes visual and performance gates. A migration must allow old and new
actor shapes to coexist without merging mutable caches across drawable worlds.

## Staged Implementation Campaign

This ordering is the current staged direction. Each phase should become a
bounded tactical only when its contract and evidence are clear.

The ordering between curved primitives and animation is settled: the Phase 1
proof already draws through `rest_part_matrix[part_id]`, so the vertex
layout, palette indexing, and shader contract are exercised statically.
Phase 3 adds evaluation semantics, not layout, and Phase 2 does not need to
wait on an animation de-risk.

### Phase 0: accounting and low-risk cleanup

- split actor topology/vertex/index counters and uploaded bytes;
- reuse indices for pose-only CPU-baked updates;
- add representative per-figure geometry and pose-cost reports; and
- retain current pixels and all platform paths.

### Phase 1: compiled static box proof

- establish the provisional offline Asset Lab compiler authority and artifact
  versioning through Tactical
  [`181`](../tactical/181-compiled-figure-static-box-proof.md);
- compile one box-only figure with positions, normals, UVs, indices, part IDs,
  materials, and a real ASCII-derived texture atlas;
- render the exact compiled artifact in Asset Lab;
- load and draw the frozen artifact in shared native/web rendering; and
- compare compiled preview and runtime pixels, bounds, winding, and counts.

### Phase 2: true primitive and material parity

- compile spheres, capsules, and cylinders with deterministic segments;
- define UV projections, seams, normals, and the supported material subset;
- prove a rounded animal against Asset Lab; and
- add compiler budgets and deterministic pack validation.

### Phase 3: rigid-part animation

- retain static local-space figure buffers;
- evaluate existing clips into per-actor part palettes continuously at every
  presentation frame, initially on the CPU;
- interpolate local translation/rotation/scale and clip transitions before
  composing the parent hierarchy;
- transform positions and normals in the vertex shader;
- support mono, per-eye stereo, and full-frame multiview; and
- validate first-person body filtering and world-local cache ownership.

### Phase 4: migration and removal of approximations

- migrate player, upright bear, and chicken one at a time;
- preserve figure selection, remote appearance, walk review, and browser/XR
  behavior;
- remove texture-cell cuboids and non-box bounding-cuboid compilation only
  after the promoted assets no longer depend on them; and
- retain a deliberate fallback for non-figure debug/item actors.

### Phase 5: instancing and LOD

- batch actors sharing compatible figure/material/LOD state;
- generate and review bounded primitive LOD variants;
- implement projected-size selection, hysteresis, and bounded residency;
- validate movement, animation, composition, and XR eye consistency through
  transitions; and
- measure CPU, upload, GPU, memory, and draw-count effects independently.

### Optional Phase 6: measured GPU crowd pose evaluation

- establish a reproducible high-count crowd lane, including the thousand-
  chicken design point and lower-count controls;
- derive GPU-resident sampled clips from canonical artifact tracks with
  explicit error and loop-continuity bounds;
- retain per-actor phase, rate, blend, transform, light, and override state;
- expand final part matrices once per actor/part per presentation frame before
  instanced draws;
- keep independent animation channels composable without precomputed
  cross-product palettes; and
- promote the GPU path only on capabilities and actor/part thresholds where it
  beats the exact CPU evaluator without visible temporal degradation.

## Invariants

- The semantic source and compiled artifact remain platform-neutral.
- Desktop, web, Android, XR, and offscreen consume the same figure contract.
- No figure compilation, animation, material, or LOD policy moves into an app
  crate or TypeScript browser adapter.
- `mclone-assets` owns source/compiled asset loading and provenance;
  `mclone-render` owns GPU compilation/drawing contracts; presentation mapping
  remains in `mclone-render-session` and client/runtime owners.
- Immutable figure geometry may be shared; mutable actor/pose state remains
  per drawable world as established by Tactical 179.
- Actor world motion, animation phase, and clip blending are evaluated
  continuously at presentation cadence with no fixed maximum rate: every
  rendered frame samples its own presentation time, independent of
  authoritative update or compiled clip-sample cadence.
- Pose interpolation operates on local translation/quaternion/scale and
  composes the hierarchy afterward; CPU and GPU evaluators implement the same
  semantics.
- Every world-visible renderer and shader path is mono, per-eye, and
  full-frame-multiview aware, or explicitly documents a supported exception.
- LOD changes presentation detail only; they do not change authoritative
  entity state, dimensions, collisions, AI, or protocol identity.
- Ordinary LOD selection does not reduce temporal pose cadence. Any later
  extreme-distance held-pose or impostor policy requires separate evidence and
  an explicit visual-quality decision.
- Compiled output is deterministic, bounded, versioned, and attributable to
  its semantic source.
- The current production path is removed only after direct pixels, animation,
  asset replacement, browser WebGPU, and relevant XR paths pass.

## Explicit Non-Goals For The First Static-Box Tactical

- arbitrary glTF or general triangle-mesh import;
- weighted multi-bone skinning;
- inverse kinematics;
- animation migration, GPU clip sampling, or instancing;
- runtime primitive tessellation on every host;
- arbitrary Three.js material/shader compatibility;
- automatic mesh simplification of unstructured artist meshes;
- impostors or billboards before ordinary primitive LODs are measured; and
- changing authoritative entity or collision shape from render LOD.

These can be revisited by later phases. They should not expand Tactical 181's
compiled static-box proof.

## Closed Decisions

Closed 2026-07-16 during the pre-landing design review.

1. **Schema evolution.** Semantic `FigureAsset` remains schema v1; the
   compiled artifact starts at its own independent v1. Semantic v2 is
   revisited only when a concrete Phase 2 UV/material/LOD authoring need
   appears — the trigger tracked by the UV-policy and material-expansion
   questions below.
2. **Preview authority.** Authoring and acceptance are different roles. The
   live semantic Three.js preview remains the authoring surface indefinitely —
   it is what keeps the author's mental model and iteration loop cheap — and
   serves as a comparison oracle through Phase 2. Compiled-output preview
   becomes the default acceptance/verification surface once curved-primitive
   and material parity land.
3. **Texture ownership.** One deterministic atlas per figure is the promoted
   answer. Pack-time shared atlas pages are demoted to a Phase 5 optimization
   taken only if draw/bind profiling justifies them. Replacement packs follow
   the ordinary artifact provenance and pack-lock rules.
4. **First-person filtering.** Palette-level culling: a hidden part's matrix
   collapses to zero (or a per-part visibility bit read by the shader). This
   is topology-stable and needs no artifact change beyond the part
   classification already stored. Separate compiled index ranges remain a
   later optimization only if the wasted vertex work measures.
5. **CPU span cache.** Phase 0's index-reuse cleanup is the ceiling for
   fallback optimization. No per-actor span-cache campaign happens unless the
   compiled path fails its gates.

## Open Decisions

1. **Long-term compiler owner.** Does the provisional offline Asset Lab
   compiler remain sufficient, or does later replacement-pack or maintenance
   evidence justify a shared Rust/WASM compiler core?
2. **Promoted artifact form after the proof.** Does Tactical 181's
   self-contained header/binary container become the packed schema, or should
   pack-time layout change after its evidence? Which derived artifacts are
   committed versus generated reproducibly during packing, and which gate
   (pack validation, CI, or a commit-time script) enforces byte
   reproducibility on every change?
3. **UV policy.** What are the exact default orientations and seams for each
   primitive, and which author controls are necessary without making UV
   authoring cumbersome?
4. **Material expansion.** After Tactical 181's opaque color/texture subset,
   which of alpha mode, roughness, metalness, emissive behavior, and face
   shading belong in the shared shader contract?
5. **Palette storage and evaluator threshold.** Uniform, storage-buffer, or
   texture-backed part matrices; how are alignment, browser limits, frames in
   flight, and many instances handled, and when does GPU expansion beat CPU
   upload? WebGPU compatibility mode permits zero vertex-stage storage
   buffers, so browser reach likely requires a uniform- or texture-backed
   fallback regardless of desktop measurement.
6. **Batching after the proof.** When does the one-draw-per-actor proof become
   per-figure instancing and material/LOD buckets?
7. **LOD generation.** Fixed compiler tiers, authored overrides, screen-error
   targets, or a combination? Which variants stay resident?
8. **Legacy actors.** When do cow/debug/item shapes join compiled figures, and
   how long does the CPU-baked fallback remain supported?
9. **Animation curves.** Are linear key tracks plus quaternion interpolation
   sufficient, or do authored tangents/interpolation modes become necessary
   to bound smooth-motion error?
10. **Remote motion.** What interpolation delay and correction policy belongs
    in the shared actor presentation contract, independently of figure pose?

## Validation Direction

Each pixel-producing slice must capture and inspect output at its first
drawable milestone. The eventual campaign should include:

- compiler determinism and malformed/budget-exceeding asset tests;
- exact compiled counts, bounds, part indices, winding, normals, and UV tests;
- Asset Lab compiled-preview sheets and animation videos;
- native actor review sheets and direct gameplay screenshots;
- frozen and animated pixel comparisons for every migrated figure;
- first-person body filtering;
- asset-pack replacement and provenance checks;
- one moving actor among stationary actors;
- many actors sharing one figure;
- mono, synthetic/real stereo, and full-frame multiview where available;
- production browser WebGPU, not only a WASM build;
- LOD threshold/hysteresis and both-eye consistency tests;
- unchanged feature-off/direct-world performance; and
- release CPU, upload, GPU, memory, and draw-count comparisons.

## Code And Documentation Map

- Asset Lab direction:
  [`111`](../tactical/111-ai-figure-asset-lab.md)
- Runtime bridge history:
  [`112`](../tactical/112-asset-lab-runtime-actor-geometry.md)
- Entity/figure promotion:
  [`118`](../tactical/118-entity-runtime-and-passive-mob-bringup.md)
- Actor resource ownership:
  [`179`](../tactical/179-composable-world-presentation-and-live-preview-actors.md)
- Immediate performance issue:
  [`performance.md`](performance.md#hp-1-split-actor-pose-updates-from-whole-mesh-rebuilds)
- Durable entity boundaries:
  [`../entity-architecture.md`](../entity-architecture.md)
- Asset Lab DSL:
  [`../../tools/asset-lab/src/dsl.ts`](../../tools/asset-lab/src/dsl.ts)
- Asset Lab preview geometry:
  [`../../tools/asset-lab/src/scene.ts`](../../tools/asset-lab/src/scene.ts)
- Asset Lab export:
  [`../../tools/asset-lab/src/export.ts`](../../tools/asset-lab/src/export.ts)
- Figure source loading:
  [`../../native/crates/mclone-assets/src/figure.rs`](../../native/crates/mclone-assets/src/figure.rs)
- Current runtime figure compiler:
  [`asset_lab_figure.rs`](../../native/crates/mclone-render/src/asset_lab_figure.rs)
- Current actor cache/draw path:
  [`entity.rs`](../../native/crates/mclone-render/src/entity.rs)
- First bounded implementation proof:
  [`181`](../tactical/181-compiled-figure-static-box-proof.md)

## Recommended Next Work

Run Tactical [`181`](../tactical/181-compiled-figure-static-box-proof.md):
capture the existing player baseline, define and deterministically compile the
smallest self-contained box/UV artifact, make Asset Lab preview that artifact,
and draw the frozen result through a shared opt-in runtime proof. Stop before
animation, production actor migration, instancing, curved primitives, or LOD.

After that proof, decide artifact promotion and packing based on observed
authoring/runtime parity. Then open separate tacticals for true curved
primitives/material parity and presentation-rate rigid-part animation. Do not
schedule GPU crowd evaluation until the CPU-palette compiled path and a real
high-count fixture provide a measured crossover question.
