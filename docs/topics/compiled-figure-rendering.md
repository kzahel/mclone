# Prepared Figure Rendering

Topic: `compiled-figure-rendering`

Status: first interactive production migration complete 2026-07-17. Tactical
[`181`](../tactical/181-compiled-figure-static-box-proof.md) is complete: the
canonical semantic handoff, shared startup compiler, immutable mono renderer,
visually approved Three.js/native comparison, inspected native per-eye stereo,
and inspected production browser WebGPU pixels are landed. The simpler native
proof lighting was accepted for this stage. Multiview execution remains
capability-skipped on the current Mac with shader/contract validation pending a
capable adapter. Semantic JSON remains the only persisted runtime format.
Tactical
[`186`](../tactical/186-prepared-figure-continuous-animation-proof.md) now owns
startup-compiled indexed tracks, cadence-independent local TRS/hierarchy
evaluation, resident mutable-palette rendering, and the human-approved
synchronized Three.js/native walk comparison. Tactical
[`189`](../tactical/189-interactive-prepared-actor-runtime.md) completed
explicit cuboid proxies for current solid-color non-box primitives, placeable
chicken and passive-goal mannequin fixtures, stable presentation identity,
continuous travel phase, prepared/legacy actor coexistence, browser WebGPU,
persistence, and the non-instanced thousand-chicken baseline. Exact rounded
tessellation is now closed by completed Tactical
[`200`](../tactical/200-box-only-figure-authoring.md), which makes boxes the
only canonical/promotable vocabulary and migrates the current catalog while
retaining explicit schema-v1 legacy compatibility. Instancing, box-part LOD,
GPU pose evaluation, and a disk cache remain deferred.

All source, generated-asset, pack, shared-workspace, thin-adapter, and WASM
build gates passed at Tactical 200 closeout. Canonical/legacy Asset Lab sheets
and movies were inspected throughout the migration. The 2026-07-20 Linux
follow-up regenerated and approved the complete canonical/rounded review set,
both Chicken semantic/prepared comparisons, and the native desktop offscreen
receipt. That evidence supersedes the earlier Mac Metal queue timeout. Browser
selection and provenance behavior also pass after serializing competing host
promises, but Chrome 147/150 on this displayless host yields only transparent
or monochrome presentation captures; the hardened smoke rejects those pixels,
so this follow-up does not claim a fresh browser screenshot.

## Scope

This topic tracks the evolution from the current CPU-baked combined
actor mesh to a startup-prepared figure pipeline with static GPU geometry,
rigid-part animation, shared instances, and generated figure LODs.

It covers the contract between the Asset Lab authoring format, persisted
semantic JSON, startup figure preparation, `mclone-assets`, actor
presentation, and `mclone-render`. It does not change authoritative entity
simulation, protocol identity, AI, or spawning. Those remain owned by the
entity/runtime architecture.

The architectural target and `mclone-assets` compiler ownership are selected,
while optimization thresholds and later LOD policy remain open. The purpose of
this document is to preserve current evidence and decisions, identify the
tradeoffs that need measurement, and keep small actor-cache work from
accidentally hardening an interim representation into the long-term figure
architecture.

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
  -> canonical schema-v1 JSON serialization
  -> parse and validate generated JSON
  -> Three.js semantic preview
  -> first-party asset-pack entry
  -> mclone-assets JSON loader
  -> mclone-render cuboid compiler
  -> CPU-baked world-space actor mesh
```

The authoring/export side preserves semantic primitives for schema-v1 legacy
compatibility, but canonical `figure()` sources now admit boxes only.
Deprecated `legacyFigure()` sources under `legacy-examples/` retain the rounded
A/B evidence. TypeScript is the only authored representation for promoted
figures. Every Asset Lab display
path now receives a parsed JSON document: a TypeScript input is executed,
serialized, reparsed, and validated before Three.js sees it, while a JSON
input is fetched and validated directly. Preview, sheet, smoke, and video no
longer render the live module object through a privileged shortcut.

The checked JSON remains the current Rust runtime handoff. Player and chicken
already regenerated byte-for-byte from their TypeScript sources. Upright bear
was an orphaned, directly edited JSON asset; it now has a DSL source, with its
geometry and explicit animation keys preserved and only obsolete locomotion
metadata normalized. `asset-lab:figures:check` covers all three promoted
figures, rejects stale output, rejects any promoted figure JSON without a
declared source, and rejects non-box promoted parts. All 20 canonical and 18
legacy Asset Lab examples also pass a discovered source-to-canonical-JSON
round-trip test; the other 17 canonical figures are authoring examples, not
checked runtime assets, so they have no second file to drift against.

Three.js uses `BoxGeometry` for canonical sources. Explicit legacy sources
still use `SphereGeometry`, `CapsuleGeometry`, and `CylinderGeometry`, including
their historical authored/default segment counts. Export writes semantic JSON;
it does not write the preview's final vertex/index data.

The native bridge was intentionally narrower:

- `sphere`, `capsule`, and `cylinder` compile to their cuboid bounds;
- a box ASCII face texture becomes one thin colored overlay cuboid per texture
  cell instead of ordinary UV-mapped texture data;
- the actor vertex contains baked world position, UV, color, and packed light;
- figure animation is sampled on the CPU and immediately baked into every
  affected vertex; and
- authored clip scale is parsed as source data but is not part of the current
  prepared/runtime transform;
- all visible actors share one mutable combined mesh per drawable world.

These choices proved asset ownership, figure selection, networked appearance,
clip import, first-person body filtering, and cross-platform actor rendering.
They were not intended to define final primitive tessellation or texture
ownership. Tactical
[`112`](../tactical/112-asset-lab-runtime-actor-geometry.md) records the early
bridge, while Tactical
[`118`](../tactical/118-entity-runtime-and-passive-mob-bringup.md#slice-5---chicken-asset-and-species-state)
explicitly calls true non-box rendering follow-up work.

The approximation historically distorted mesh size in both directions.
Legacy curved animals are much simpler in-engine than in Asset Lab, while a
textured box can
become much more expensive because texture cells become geometry. For example,
the authored player preview is twelve boxes and 288 Three.js vertices, but its
8x8 face texture adds 64 runtime overlay cuboids in the current bridge.

## Current Asset-Lab Geometry Evidence

The following representative counts were most recently refreshed on
2026-07-20 using the same Three.js geometry constructors and default segment
counts as Asset Lab. They describe intended preview geometry, not current
native cuboid approximations, and will change as source figures are revised.

| Figure | Parts | Vertices | Triangles |
|---|---:|---:|---:|
| player | 12 | 288 | 144 |
| piglet | 14 | 336 | 168 |
| piglet_rounded (legacy) | 14 | 809 | 1,052 |
| dog | 17 | 408 | 204 |
| dog_rounded (legacy) | 18 | 991 | 1,268 |
| fox | 18 | 432 | 216 |
| fox_rounded (legacy) | 32 | 1,565 | 1,900 |
| wolf | 16 | 384 | 192 |
| wolf_rounded (legacy) | 30 | 1,365 | 1,580 |
| sheep | 15 | 360 | 180 |
| sheep_rounded (legacy) | 17 | 1,139 | 1,512 |
| horse | 22 | 528 | 264 |
| horse_rounded (legacy) | 30 | 1,365 | 1,580 |
| bear | 17 | 408 | 204 |
| bear_rounded (legacy) | 41 | 1,801 | 1,968 |
| lion | 23 | 552 | 276 |
| lion_rounded (legacy) | 48 | 2,184 | 2,432 |
| bearfolk | 15 | 360 | 180 |
| bearfolk_rounded (legacy) | 17 | 1,612 | 2,316 |
| lionfolk | 20 | 480 | 240 |
| lionfolk_rounded (legacy) | 18 | 1,765 | 2,540 |
| cat | 16 | 384 | 192 |
| cat_rounded (legacy) | 20 | 1,103 | 1,340 |
| chicken | 14 | 336 | 168 |
| chicken_rounded (legacy) | 21 | 2,209 | 3,108 |
| cow | 22 | 528 | 264 |
| cow_rounded (legacy) | 38 | 1,946 | 2,208 |
| goat | 24 | 576 | 288 |
| goat_rounded (legacy) | 32 | 1,697 | 1,940 |
| rabbit | 18 | 432 | 216 |
| rabbit_rounded (legacy) | 25 | 3,050 | 4,640 |
| butterfly | 9 | 216 | 108 |
| butterfly_rounded (legacy) | 17 | 1,816 | 2,516 |
| elephant | 40 | 960 | 480 |
| elephant_rounded (legacy) | 38 | 2,894 | 3,756 |
| tiger | 20 | 480 | 240 |
| tiger_rounded (legacy) | 86 | 3,648 | 3,856 |

These are not extremely large meshes, but most rounded animals are already
well above vanilla cuboid-mob geometry. The cost also multiplies by visible
actor count and animation cadence. More importantly, the pipeline should not
make CPU world-space rebaking a prerequisite for later, richer figure assets.

### Box-only animal style A/B

All 18 retained rounded sources now have canonical box-only authoring pairs
for direct style review. Representative constructions include:

- `examples/elephant` uses 40 boxes exclusively, including its stepped
  articulated trunk, tusks, ears, eyes, feet, and secondary details, while
  `legacy-examples/elephant_rounded` retains the rounded interpretation;
- `examples/tiger` follows the Minecraft ocelot hierarchy with 20
  boxes and seven pixel textures carrying its face, stripes, paws, and tail
  rings, while `legacy-examples/tiger_rounded` retains the 86-part source;
- `examples/rabbit` reduces the most curve-heavy original animal to 18
  boxes while retaining its long ears, muzzle, large hind feet, tail, and
  synchronized hop;
- `examples/chicken` uses 14 boxes for a compact walking hen whose face,
  wing feathers, and toes move to pixel textures; and
- `examples/butterfly` uses nine boxes, including separate fore- and
  hindwing slabs whose border and spot detail is entirely texture-driven;
- `examples/cat` uses 16 boxes, with pixel tabby detail and a two-piece tail;
- `examples/cow` uses 22 boxes, with texture-painted Holstein patches and
  socks plus stepped horns, an udder, and a tail tuft; and
- `examples/goat` uses 24 boxes, with stepped swept horns, beard, narrow
  legs, a cloven-hoof texture, and a short upturned tail.

The Elephant pair uses the same `1.6`-second, `0.9`-unit `quadrupedWalk`
timing and matching independent trunk, ear, head, and tail motion. Clean
multi-angle sheets and three-cycle movies were rendered under `/tmp`,
inspected individually, and combined into direct side-by-side reviews. Human
review preferred the box-only version for its charm and stronger fit with the
voxel world's visual language.

The tiger pair uses identical `1.02`-second, `1.02`-unit walk timing. Direct
review found much less stylistic separation than the elephant pair because the
existing tiger already reads as blocky. The smaller box-only figure retains
the face, striped coat, gait, and articulated tail at both review-sheet and
thumbnail scale, but its useful comparison is authored complexity and
texture-driven detail rather than rounded-versus-blocky style. Review also
exposed a visibly detached tail root in the first capture; the corrected root
pivots from inside the rump, and its second segment attaches endpoint-to-
endpoint. Human review preferred the corrected 20-part version despite the
smaller stylistic difference.

The tiger comparison also supplies useful authoring-side cost evidence. The
box-only source has 4.3 times fewer parts, 7.6 times fewer preview vertices,
and 16.1 times fewer preview triangles than the existing tiger. Tiny cylinders
for whiskers and tessellated capsules for otherwise block-readable limbs or
tail bands add topology and authoring complexity without a visible fidelity
gain at the reviewed scale. Canonical and promoted figure work now requires
boxes and pixel face textures. Curved primitives remain only for explicitly
named legacy comparison sources.

The second conversion wave deliberately selected the three highest non-box
shares among existing animals and crossed three different body plans:

| Pair | Original non-box share | Box-only parts | Original / box-only triangles |
|---|---:|---:|---:|
| Rabbit | 20 / 25 (80%) | 18 | 4,640 / 216 (21.5x) |
| Butterfly | 13 / 17 (76%) | 9 | 2,516 / 108 (23.3x) |
| Chicken | 15 / 21 (71%) | 14 | 3,108 / 168 (18.5x) |

All three retain the original clip timing and locomotion metadata. Clean
multi-angle sheets, three-cycle movies, direct A/B sheets, and joined A/B
movies were rendered under `/tmp` and inspected across their sampled cycles.
The box-only rabbit has the strongest stylistic change; the chicken remains
immediately legible through its comb, beak, wattle, wings, tail, and feet; and
the butterfly preserves its colorful identity by replacing rounded spots and
antenna tips with pixel wing patterns and a simpler silhouette. The next
high-value conversion was the Cat, Cow, and Goat quadruped wave.

That wave retains every original walk duration, cycle distance, stance ratio,
body/head motion, and secondary track while reducing 90 mixed parts to 62
boxes. Cat drops from 1,340 to 192 exact preview triangles, Cow from 2,208 to
264, and Goat from 1,940 to 288. Together that is 5,488 versus 744 triangles,
a 7.4x authoring-preview reduction, plus a 31% part-count reduction relevant
to the current prepared cuboid path. Clean canonical/legacy sheets and all six
three-cycle movies were rendered under `/tmp/mclone-asset-lab` and inspected;
the sampled motion remains grounded and the sparse versions preserve the cat
tail, cow horn/udder, and goat horn/beard signatures. The anatomy-sharing Dog,
Fox, and Wolf wave followed.

The canid wave retains every archived trot duration, cycle distance, stance,
body/head motion, and secondary track while reducing 80 mixed parts to 51
boxes. Dog drops from 1,268 to 204 exact preview triangles, Fox from 1,900 to
216, and Wolf from 1,580 to 192. Together that is 4,748 versus 612 triangles,
a 7.8x authoring-preview reduction, plus a 36% part-count reduction relevant
to the prepared cuboid path. The shared anatomy remains intentionally varied:
the dog is broad, short-backed, floppy-eared, and collared; the fox is low and
narrow with oversized ears and a large white-tipped tail; and the wolf is tall
with a separate shoulder mass and heavy straight tail. Clean A/B sheets and
all six three-cycle movies were inspected. The Piglet, Sheep, and Horse farm
wave followed.

The farm wave retains every archived walk duration, cycle distance, stance,
body/head motion, and secondary track while reducing 61 mixed parts to 51
boxes. Piglet drops from 1,052 to 168 exact preview triangles, Sheep from 1,512
to 180, and Horse from 1,580 to 264. Together that is 4,144 versus 612
triangles, a 6.8x authoring-preview reduction, plus a 16% part-count reduction
relevant to the prepared cuboid path. The canonical piglet emphasizes its
square head and snout, the sheep replaces simulated round tufts with one
texture-edged wool mass, and the horse preserves its long-legged angled-neck
posture and animated mane. Clean A/B sheets and all six three-cycle movies
were inspected. The Bear, Lion, Bearfolk, and Lionfolk wave followed.

The final content wave retains every archived clip duration, cycle distance,
stance, body/head motion, and secondary track while reducing 124 mixed parts
to 75 boxes. Bear drops from 1,968 to 204 exact preview triangles, Lion from
2,432 to 276, Bearfolk from 2,316 to 180, and Lionfolk from 2,540 to 240.
Together that is 9,256 versus 900 triangles, a 10.3x authoring-preview
reduction, plus a 40% part-count reduction relevant to the prepared cuboid
path. Bear and Lion keep visibly different heavy and mane-led quadruped mass;
Bearfolk and Lionfolk now share the canonical player's cuboid body and joint
grammar. Clean A/B sheets and all eight three-cycle movies were inspected.
This completes the 13-source rounded-to-box migration queue.

These topology ratios do not by themselves prove an end-to-end frame-time
improvement. Material ranges, atlas residency, draw batching, animation
evaluation, actor count, and the selected renderer quality tier still matter.
The prepared box path keeps pixel textures in an atlas rather than treating
texture cells as the target geometry model, so texture-driven detail is the
compatible direction for later runtime measurement.

Keep both sources available as A/B pairs, but only the box-only source owns the
ordinary canonical name. Tactical
[`200`](../tactical/200-box-only-figure-authoring.md) makes this a repository-
wide production policy and migrates the remaining body plans in reviewed
waves. This is deliberately not a schema-v1 deletion: rounded sources remain
explicitly legacy and executable.

## Current Texture And UV Evidence

The current authored figures do not describe generally unwrapped characters.
An inventory taken on 2026-07-20 from both source roots found:

| Fact | Count |
|---|---:|
| figures | 47 |
| total parts | 1,057 |
| boxes | 852 |
| spheres / capsules / cylinders | 80 / 98 / 27 |
| ASCII textures | 113 |
| parts with any texture reference | 159 |
| individual texture applications | 267 |
| texture applications on curved primitives | 0 |

The policy split within that combined inventory is:

| Source class | Figures | Parts | Boxes | Spheres / capsules / cylinders |
|---|---:|---:|---:|---:|
| canonical `examples/` | 29 | 515 | 515 | 0 / 0 / 0 |
| deprecated `legacy-examples/` | 18 | 542 | 337 | 80 / 98 / 27 |

The first post-migration content batch adds three approved canonical box-only
examples without changing the promoted runtime set: Deer is a 25-part
white-tailed buck with a connected six-box antler assembly, Zebra is a 22-part
horse-derived rig with six pixel textures, and Panda is a 17-part bear-derived
rig with three pixel textures. Clean multi-angle sheets and four-cycle walk
videos were rendered under `/tmp/mclone-asset-lab/batch-1`, inspected for
identity, grounding, attachment continuity, and motion, and approved by the
user on 2026-07-20.

The second post-migration content batch adds Owl, Parrot, and Eagle as approved
15-part canonical box-only flying rigs. Their two-stage wing tips inherit the
animated wing-root transforms, while their facial discs, hooked beaks, tails,
talons, proportions, and 11 combined pixel textures keep the silhouettes
distinct. Clean multi-angle sheets and four-cycle flight videos were rendered
under `/tmp/mclone-asset-lab/batch-2`, inspected for identity, attachment
continuity, cadence, and full upstroke/downstroke motion, and approved by the
user on 2026-07-20.

The third post-migration content batch ships a shared `swim` authoring macro
that emits ordinary schema-v1 keys and locomotion metadata for body
counter-sway, primary and delayed tail motion, mirrored pectoral fins, and
optional vertical drift. Fish is a 10-part lateral-tail rig, Dolphin is a
13-part vertical-tail rig with horizontal flukes, and Shark is a 16-part
lateral-tail rig with a two-lobe caudal fin; all are canonical boxes and use
10 combined pixel textures. Clean multi-angle sheets and four-cycle swim
videos were rendered under `/tmp/mclone-asset-lab/batch-3`, inspected for axis
correctness, identity, attachment continuity, and complete tail/fin motion,
and approved by the user on 2026-07-20.

All 267 applications target an explicit face of a box. They range from single
front/north faces for eyes and muzzles to the butterfly wings and the blocky
tiger's body, leg, paw, and tail patterns. No part uses a whole-primitive
texture, and all 205 curved primitives currently use solid materials.

This distinction matters: GPU geometry may carry deterministic UV attributes
without requiring authors to unwrap a figure. The current content needs
automatic planar coordinates for a sparse set of box-face decals, not a
general character UV editor or continuity across parts.

## North Star

Keep the TypeScript DSL or another compact semantic representation as the
editable source. Keep its generated semantic JSON as the persisted runtime
contract, and compile it once at startup or first asset residency into an
in-memory prepared figure shared by actors and drawable worlds:

```text
figure.ts
  -> canonical semantic figure JSON
  -> parse and validate
  -> shared renderer-neutral startup compiler
  -> in-memory PreparedFigure
       static geometry + rig + materials + textures + clips + LODs
  -> immutable GPU resources shared by actor instances
```

The shared Rust engine implementation is the natural runtime authority because
native, web/WASM, Android, and XR already consume that code. `mclone-assets`
now owns renderer-neutral preparation beside semantic loading, while
`mclone-render` owns GPU residency and drawing. Asset Lab continues rendering
its parsed semantic JSON with Three.js; the native comparison diagnostic
exposes prepared output for review without creating a production file format.

`PreparedFigure` is an internal CPU representation, not a compatibility
format. It can evolve with the renderer while semantic JSON stays small,
typed, diffable, and pack-replaceable. Preparation happens once per resident
figure, never once per actor or presentation frame. A later disk cache is a
discardable optimization keyed by semantic content and compiler version, and
is justified only by measured startup, streaming, or compiler-size cost.

Three.js retains semantic preview authority even where it loses runtime-
production duty. Canonical figures now use only its exact `BoxGeometry`
semantics, guarded by pinned cross-language fixtures. Rounded Three.js
primitives remain useful solely for the retained legacy A/B archive; they are
not a future production quality tier.

Curved schema-v1 primitives are now orthogonal compatibility input. The shared
compiler continues translating them to explicitly diagnosed cuboid bounds so
legacy or replacement JSON fails neither silently nor gratuitously. Canonical
and promoted sources cannot create them, and no exact curved compiler work is
planned.

## Selected Direction And Deliberate Flexibility

The durable persisted contract is semantic figure JSON. Prepared geometry is
an internal runtime product, independent of a particular pose evaluator or
batching strategy:

- semantic Asset Lab source crosses the canonical JSON/validation boundary
  before startup compilation;
- every runtime lane uses the same shared compiler and prepared-figure
  contract;
- geometry stays in local part space and is uploaded once per resident figure
  and LOD;
- actor records and final part palettes remain the small mutable payload;
- actor movement, animation phase, and clip blending are evaluated
  continuously at presentation cadence; and
- the runtime may choose exact CPU pose evaluation or a measured GPU crowd
  path without changing the asset contract.

For the first proof, a renderer-neutral shared Rust compiler prepares boxes at
asset load. Three.js is a test oracle: equality fixtures against the
repository-pinned `BoxGeometry` output guard drift. Runtime preparation owns
coordinate conversion, topology, normals, UVs, material ranges, atlas data,
part indices, and bounds. `mclone-render` uploads that prepared data without
performing a second semantic compilation.

Legacy spheres, capsules, and cylinders enter the same compiler as an explicit
low-cost cuboid-proxy compatibility variant: sphere bounds are `2r` on each axis, capsule
bounds are `2r` by `length + 2r` by `2r`, and cylinder bounds use twice the
larger radius by `length` by twice the larger radius. This preserves the
existing acceptable in-engine silhouette while enabling the prepared runtime
contract. The source primitive kind and approximation count remain visible in
diagnostics. Exact curved tessellation is closed as a production direction;
removing the compatibility path would require a separate persisted-schema and
replacement-pack decision.
If interactive prepared-output review becomes valuable, expose the shared
compiler to Asset Lab through a native diagnostic or Rust/WASM rather than
creating a second implementation.

Likewise, a shared sampled phase palette is a runtime-derived cache, not
canonical authored animation data. Semantic JSON retains the clip tracks and
their semantics. A renderer may build GPU-resident sampled clips from them,
while a CPU evaluator can continue using the same tracks exactly. This avoids
making every actor count and every supported GPU pay for the crowd path.

## Static-Box Proof Evidence

The first box-only proof is now concrete rather than only architectural:

- `mclone-assets` loads the normal semantic JSON through `AssetSource` and
  deterministically prepares an internal `PreparedFigure`; there is no second
  persisted input, byte format, header, cache key, or compatibility promise.
- The semantic player hash is
  `3e5f885ed57fa86051e647cb98ca8c2238285e2fc14b511159601acb33f300fc`.
  It prepares as 12 parts, 288 local vertices, 432 `u16` indices, 72 face
  ranges, and one 13x10 atlas containing the 8x8 face texture without the old
  64-cuboid texture-cell expansion.
- `mclone-render` retains one immutable vertex buffer, index buffer, atlas,
  and rest palette. A three-view run reports four immutable uploads total and
  three view-uniform writes, rather than rebuilding topology for each panel.
- The one-command `pnpm asset-lab:compare` lane asks the semantic Three.js
  renderer to derive framing, passes that exact ephemeral contract to the
  native offscreen renderer, and emits raw panels, receipts, and a labeled
  paired sheet under `/tmp`.
- The inspected front, right, and three-quarter projections agree on scale,
  grounding, silhouette, box placement, and face orientation. First-draw
  inspection exposed and fixed incorrect mirrored winding. The native proof
  is darker because it intentionally uses a simpler fixed light/material
  approximation; exact RGB equality is not claimed.

This evidence makes the source/compiler/local-space/residency shape ready for
human review. It does not yet approve production actor replacement, animation,
stereo/multiview, browser WebGPU, or curved primitives.

## Candidate Prepared-Figure Contract

This is an in-memory capability sketch, not a frozen binary layout or public
compatibility format.

### Static geometry per LOD

- local-space positions;
- normals;
- deterministic UV attributes, generated mechanically and meaningful only to
  material ranges that actually sample a texture;
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
- validated clip tracks and locomotion/contact metadata;
- part identity preserved across every LOD; and
- an explicit policy for parts omitted from a coarse LOD.

The first runtime can continue sampling clips and resolving the parent
hierarchy on the CPU. The resulting final part matrices are the small dynamic
payload sent to the GPU. GPU clip evaluation is a separate possible future
optimization, not a prerequisite.

### Materials and textures

- solid-color materials as the ordinary path;
- generated RGBA atlas regions derived from the sparse ASCII sources;
- automatic planar `0..1` coordinates for each explicitly textured box face,
  remapped mechanically into its atlas region;
- nearest-filtered sampling where authored;
- ordinary box coordinates on cuboid proxies for legacy curved primitives,
  whose current solid materials do not sample them;
- a clear compile error for authored sphere, capsule, or cylinder textures;
  curved-surface texture authoring is closed with the production policy;
- a deliberately supported material subset; and
- stable material slots shared across LOD variants.

There is no whole-character unwrap, no cross-part texture continuity, and no
manual seam/rotation/scale authoring surface in the initial campaign. A solid
material may ignore UVs entirely or sample a compiler-generated solid atlas
swatch; that encoding choice does not become authoring work.

If later figures need broad fur variation, mottling, or noise, prefer
figure/object-space procedural mapping, generated vertex color, or additional
box-face textures over manually unwrapping every part. Curved-surface UV
authoring is not a production figure direction; changing that would require a
new policy and a concrete asset need.

Asset Lab currently previews roughness and metalness through Three.js while
the native actor path uses simpler baked face colors. The prepared contract
must either support a shared material interpretation or explicitly narrow the
preview to the runtime-supported subset. Silent material divergence should not
remain the default.

### Determinism and diagnostics

- semantic schema version;
- compiler implementation/version identity in diagnostics;
- semantic content hash;
- build settings, including LOD reduction settings;
- bounds and geometry statistics; and
- deterministic repeated preparation for the same semantic input and compiler
  version.

## Candidate Runtime Shape

Immutable prepared figure resources should be shareable across compatible
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

Prepared figures take one packed light value per actor, sampled at the
actor's position, instead of the current per-vertex baked world light. A
large figure straddling a light gradient will shade slightly differently
than the CPU-baked bridge; migration pixel comparisons must treat that as a
deliberate lighting-model change, not a regression.

Actors sharing a figure can later be instanced by indexing an actor record and
that actor's part-palette base. Instancing is desirable, but a first prepared
figure proof may issue one draw per actor or figure/material group while the
static-geometry and transform contracts settle. Do not couple correctness of
the prepared renderer contract to the first batching strategy.

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

The first animated prepared path evaluates final part matrices on the CPU each
presentation frame. If crowd measurements justify a GPU path, a compute pass
samples and blends the shared clip data, composes the hierarchy once per
actor/part, and writes final matrices for the instanced vertex pass. Sampling
and hierarchy work should not be repeated independently for every vertex.

## Performance Model

Startup-prepared figures and LOD solve different costs:

- static geometry plus part palettes reduces CPU vertex baking, serialization,
  and CPU-to-GPU upload;
- instancing reduces duplicate geometry residency and draw/setup overhead for
  actors sharing a figure; and
- LOD reduces the remaining GPU vertex processing, raster pressure, and
  potentially material cost for small projected actors.

As an illustration only, the current actor vertex is 40 bytes. Re-uploading
the canonical Chicken's 336 preview vertices would be about 13 KB before
indices, while 14 rigid 3x4 `f32` part matrices are 672 bytes. Its archived
rounded source would have required 2,209 preview vertices and 21 matrices.
The future prepared vertex layout and actor payload will differ, but the order
of magnitude explains both the prepared-path and box-only-authoring gains.

GPU part transforms do not reduce the number of vertices the GPU executes.
That is why LOD remains valuable after CPU/upload work is removed.

### Thousand-chicken design point

The canonical Asset Lab Chicken has 336 vertices, 168 triangles, and 14 rigid
parts. One thousand full-detail visible chickens would therefore submit about
336,000 vertices and 168,000 triangles per presentation frame. At high
presentation rates, vertex and raster work—not just pose upload—can still
become the limiting cost on low-end hardware.

Fourteen final 3x4 `f32` matrices for one thousand actors are approximately
672 KB per presented frame. As an accounting example, that is roughly 81 MB/s
at 120 Hz and 336 MB/s at 500 Hz, before actor records and alignment. Those are
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

- prepared vertices, indices, bytes, parts, materials, and texture bytes per
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

- **Compiler ownership:** the shared Rust engine gains deterministic
  tessellation, box-face projection, atlas, budget, and error-reporting
  responsibilities. Three.js fixtures, golden counts, and prepared-output
  diagnostics keep that cost controlled.
- **Startup work:** every newly resident semantic figure must be prepared once.
  Current meshes are small, so this is expected to be cheap, but startup and
  streaming time must be measured on low-end targets before dismissing a
  discardable cache.
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
- **LOD variants:** generated variants can pop, change silhouettes, or move
  grounding. Projected-error thresholds, hysteresis, stable part identity,
  conservative XR selection, and visual transition sheets are required.
- **Movement smoothness versus latency:** remote interpolation is smooth by
  buffering some history; prediction is responsive but needs corrections.
  Figure animation cannot paper over an undefined actor-motion contract.
- **Rigid-part ceiling:** this design fits current Minecraft-like figures but
  does not provide weighted skinning, morphs, cloth, or facial deformation.
  Those remain explicit future semantic/runtime extensions rather than hidden
  special cases.
- **CPU/GPU agreement:** floating-point ordering, quaternion interpolation,
  hierarchy composition, and transition timing can diverge. Shared semantic
  tests and tolerant pose/pixel comparisons must gate a GPU evaluator.

## Figure LOD Direction

Box-only source assets make deterministic generated LODs unusually tractable.
Candidate tiers are:

- LOD 0: all authored cuboid parts and full material/texture detail;
- LOD 1: omit or merge explicitly classified decorative cuboids while
  preserving the silhouette and all important animated parts;
- LOD 2: coarse cuboid silhouette with fewer rigid-part matrices and simplified
  face textures; and
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
  variants should already be prepared and resident or admitted through an
  explicit bounded residency path; and
- keep selection and residency policy shared across host adapters.

Open design work includes segment-reduction rules, treatment of tiny parts,
normal continuity, mip and filtering policy for the sparse box-face decals
(the proof's nearest-filtered unmipped atlas will shimmer on distant actors
and must be revisited alongside LOD), residency limits, transition policy,
shadows if added, and whether authored overrides are needed for silhouette-
critical features.

## Asset Lab Evolution

With this direction selected, Asset Lab grows from a disposable semantic
preview into an authoring and runtime-preparation review surface. Useful additions
include:

- the completed canonical serialize/reparse path for every semantic preview;
- direct JSON preview, sheet, smoke, and video input;
- checked first-party source/output mappings and deterministic drift checks;
- optional prepared-mesh diagnostics from the shared Rust compiler, exposed
  through a native review command or a later Rust/WASM Asset Lab bridge;
- a one-command, labeled side-by-side sheet pairing semantic Three.js and
  prepared engine views at the same camera, projection, dimensions, and pose;
- comparison framing derived once and shared by both renderers rather than
  independent auto-framing that could hide bounds, scale, or grounding drift;
- an optional aligned silhouette/edge overlay for geometry debugging, without
  treating raw RGB differences as failures while lighting and material models
  intentionally differ;
- box-face atlas/UV orientation visualization;
- inspection of compiler-preserved Three.js default coordinates on curved
  primitives without implying a curved-texture authoring workflow;
- normal and winding visualization;
- atlas and material-slot review;
- per-LOD static sheets and animated transition review;
- geometry/texture/part/clip budget reports with actionable part names;
- deterministic repeated-preparation checks; and
- measured startup/residency accounting before any disk-cache proposal.

The tool should continue making semantic source easy for agents and humans to
edit, targeting the familiar Three.js scene semantics described in the North
Star. Prepared data remains internal and may be optimized freely; authored
source and generated semantic JSON remain small, typed, diffable, and
regenerable.

## Relationship To The Immediate Actor-Cache Issue

The following bounded cleanup remains useful regardless of the prepared path:

1. distinguish topology, vertex, and index updates in diagnostics;
2. stop rebuilding and uploading unchanged indices on pose-only changes; and
3. preserve allocation-free unchanged frames and first-eye/second-eye reuse.

Stable per-actor CPU vertex spans may remain valuable for legacy cows, debug
cubes, item actors, unsupported figure features, and an incremental fallback.
They should not be treated as a prerequisite for prepared Asset Lab figures,
and the project should avoid a large CPU span-cache campaign before deciding
the prepared-figure contract.

The existing `ActorMeshCache` remains the production path until a prepared
path passes visual and performance gates. A migration must allow old and new
actor shapes to coexist without merging mutable caches across drawable worlds.

## Staged Implementation Campaign

This is the current staged direction. Each phase should become a bounded
tactical only when its contract and evidence are clear. After Phase 1,
animation and an explicit non-box proxy are the bounded bridge to an
interactive production proof. Exact curved parity is closed rather than an
optional quality tier. Neither choice changes the persistence decision.

### Semantic source/output prerequisite (complete)

- make `figure.ts` the only authored representation for promoted figures;
- make every Asset Lab display path serialize/reparse TypeScript output or
  parse JSON directly before rendering;
- restore an Asset Lab source for upright bear and normalize its legacy
  locomotion metadata;
- enforce exact generated JSON for player, chicken, and upright bear; and
- reject promoted runtime JSON without a declared source.

### Phase 0: accounting and low-risk cleanup

- split actor topology/vertex/index counters and uploaded bytes;
- reuse indices for pose-only CPU-baked updates;
- add representative per-figure geometry and pose-cost reports; and
- retain current pixels and all platform paths.

### Phase 1: startup-prepared static box proof (complete)

- establish the shared Rust startup compiler and in-memory `PreparedFigure`
  through Tactical
  [`181`](../tactical/181-compiled-figure-static-box-proof.md);
- compile one box-only figure with positions, normals, UVs, indices, part IDs,
  materials, and a real ASCII-derived texture atlas;
- upload and draw the prepared result through shared rendering; native mono,
  distinct-slot per-eye stereo, and production browser WebGPU are implemented
  and inspected, while full-frame multiview execution is capability-deferred;
- compare runtime pixels, bounds, winding, UVs, and counts against the
  semantic Three.js baseline; and
- record startup preparation time without creating a persisted geometry file.

### Capability 2A: explicit legacy non-box cuboid proxy (complete)

- compile spheres, capsules, and cylinders to their deterministic authored
  bounds while retaining their semantic primitive kinds in diagnostics;
- keep current curved primitives on solid materials and reject authored
  curved textures rather than inventing a UV authoring contract;
- label prepared comparisons as cuboid proxies instead of claiming Three.js
  silhouette parity;
- preserve deterministic legacy compatibility after promoted Chicken becomes
  box-only; and
- keep true rounded tessellation closed as a production direction.

### Capability 2B: rigid-part animation

- retain static local-space figure buffers;
- evaluate existing clips into per-actor part palettes continuously at every
  presentation frame, initially on the CPU;
- interpolate local translation/rotation/scale and clip transitions before
  composing the parent hierarchy;
- transform positions and normals in the vertex shader;
- support mono, per-eye stereo, and full-frame multiview; and
- validate first-person body filtering and world-local cache ownership.

### Phase 3: interactive migration with coexistence (complete)

- placeable chicken and mannequin debug-hotbar tools now use ordinary
  authoritative use-item placement and persistence;
- the mannequin now uses player dimensions/figure selection and the existing
  cow passive-goal family so continuous world movement is directly testable;
- stable presentation identity now reaches renderer-neutral actor instances,
  and world-local presentation state derives continuous travel phase from
  interpolated movement instead of authoritative update steps;
- chicken and mannequin now use asset/device-scoped immutable prepared
  geometry with stable-ID world-local model/light/palette records, while the
  same frame can retain legacy actors;
- direct, placed, clipped, per-eye, and multiview-aware paths are implemented;
  native mono/stereo composition pixels, no-second-eye-pose-write counters,
  the identical production browser WebGPU fixture, SQLite restart, semantic
  replacement, and the explicit thousand-chicken baseline pass; and
- retain `ActorMeshCache` as an explicit fallback for unsupported figure,
  debug, and item actors.

### Box-only canonical migration (complete)

- enforce boxes through canonical `figure()` and the first-party drift gate;
- retain deprecated rounded sources under `legacy-examples/` with explicit
  compatibility-only authoring and round-trip coverage;
- promote the approved box-only Chicken and existing animal comparisons to
  their ordinary names; and
- re-author the remaining catalog in inspected waves through Tactical
  [`200`](../tactical/200-box-only-figure-authoring.md).

### Phase 4: instancing and LOD

- batch actors sharing compatible figure/material/LOD state;
- generate and review bounded primitive LOD variants;
- implement projected-size selection, hysteresis, and bounded residency;
- validate movement, animation, composition, and XR eye consistency through
  transitions; and
- measure CPU, upload, GPU, memory, and draw-count effects independently.

### Optional Phase 5: measured GPU crowd pose evaluation

- establish a reproducible high-count crowd lane, including the thousand-
  chicken design point and lower-count controls;
- derive GPU-resident sampled clips from canonical semantic clip tracks with
  explicit error and loop-continuity bounds;
- retain per-actor phase, rate, blend, transform, light, and override state;
- expand final part matrices once per actor/part per presentation frame before
  instanced draws;
- keep independent animation channels composable without precomputed
  cross-product palettes; and
- promote the GPU path only on capabilities and actor/part thresholds where it
  beats the exact CPU evaluator without visible temporal degradation.

## Invariants

- Promoted `figure.ts` files are the only human/AI-authored figure source;
  semantic JSON is generated and never hand-edited.
- Asset Lab semantic display consumes only parsed, validated JSON, including
  when its input was TypeScript source.
- Semantic JSON and the in-memory prepared-figure contract remain
  platform-neutral.
- Desktop, web, Android, XR, and offscreen use the same startup compiler.
- No figure compilation, animation, material, or LOD policy moves into an app
  crate or TypeScript browser adapter.
- `mclone-assets` owns semantic asset loading and validation; a shared Rust
  asset/figure owner prepares renderer-neutral geometry; `mclone-render` owns
  GPU resources and drawing contracts. Presentation mapping remains in
  `mclone-render-session` and client/runtime owners.
- Immutable figure geometry may be shared; mutable actor/pose state remains
  per drawable world as established by Tactical 179.
- Actor world motion, animation phase, and clip blending are evaluated
  continuously at presentation cadence with no fixed maximum rate: every
  rendered frame samples its own presentation time, independent of
  authoritative update or derived clip-sample cadence.
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
- Startup preparation is deterministic, bounded, and attributable to its
  semantic source and compiler version.
- No prepared geometry, atlas, or LOD file is persisted in the initial
  campaign. Any later disk cache is discardable and measurement-gated.
- The current production path is removed only after direct pixels, animation,
  asset replacement, browser WebGPU, and relevant XR paths pass.

## Explicit Non-Goals For The First Static-Box Tactical

- arbitrary glTF or general triangle-mesh import;
- weighted multi-bone skinning;
- inverse kinematics;
- animation migration, GPU clip sampling, or instancing;
- runtime primitive tessellation on every host;
- arbitrary Three.js material/shader compatibility;
- general character UV unwrapping, cross-part texture continuity, or authored
  curved-surface seam/projection controls;
- automatic mesh simplification of unstructured artist meshes;
- impostors or billboards before ordinary primitive LODs are measured; and
- changing authoritative entity or collision shape from render LOD.

These can be revisited by later phases. They should not expand Tactical 181's
startup-prepared static-box proof.

## Closed Decisions

Closed 2026-07-16 during the pre-landing design review.

1. **Source and semantic-output authority.** Promoted `figure.ts` is the only
   authored representation. Schema-v1 JSON is its deterministic generated
   semantic snapshot, all Asset Lab displays consume the parsed snapshot, and
   the first-party drift gate covers every promoted JSON asset.
2. **Persistence.** Semantic `FigureAsset` JSON remains the persisted runtime
   contract. `PreparedFigure` is internal startup output, not an independent
   schema. No prepared-geometry file or cache is introduced until measurement
   justifies it. This deliberately accepts startup CPU work and shipping the
   shared compiler in exchange for avoiding another versioned representation,
   drift gate, and pack/replacement contract.
3. **Preview authority.** Authoring and acceptance are different roles. The
   semantic Three.js preview remains the authoring surface indefinitely, but
   it renders only the serialized/reparsed semantic snapshot. It serves as a
   comparison oracle through primitive/material parity. Runtime
   prepared-output diagnostics and native pixel captures become the acceptance
   surface as those paths land.
4. **Texture ownership.** One deterministic atlas per figure is the promoted
   answer in prepared memory. Pack-time shared atlas pages are demoted to an
   instancing/LOD optimization taken only if draw/bind profiling justifies
   them.
   Replacement packs continue supplying semantic JSON.
5. **First-person filtering.** Palette-level culling: a hidden part's matrix
   collapses to zero (or a per-part visibility bit read by the shader). This
   is topology-stable and needs no persisted change beyond the semantic part
   classification already stored. Separate prepared index ranges remain a
   later optimization only if the wasted vertex work measures.
6. **CPU span cache.** Phase 0's index-reuse cleanup is the ceiling for
   fallback optimization. No per-actor span-cache campaign happens unless the
   prepared path fails its gates.
7. **Sparse texture policy.** The initial campaign supports solid materials
   plus automatic planar mapping for explicit box-face decals. It does not
   introduce whole-character unwraps or cross-part continuity. Curved-surface
   UV controls are closed with the box-only production policy.
8. **Cross-renderer visual oracle.** Tactical 181 adds one command that renders
   corresponding semantic Three.js and prepared engine views and emits a
   labeled side-by-side sheet under `/tmp`. Both sides use one renderer-neutral
   review-camera and pose contract. This is a diagnostic product, not another
   persisted figure representation. Initial acceptance is human review of
   projection, silhouette, grounding, face placement, and UV orientation;
   exact RGB equality is not required across different lighting pipelines.
9. **Box-only production policy.** Canonical `figure()` sources and every
   promoted first-party figure use boxes exclusively. Deprecated
   `legacyFigure()` sources may retain sphere, capsule, and cylinder records
   for A/B and schema-v1 compatibility review. The shared compiler continues
   translating those legacy records to explicitly diagnosed cuboid bounds;
   exact curved topology is no longer a production quality tier.

## Open Decisions

1. **Shared compiler crate boundary.** Does renderer-neutral startup
   preparation belong in `mclone-assets`, a focused shared figure crate, or a
   neighboring asset/compiler crate? It must not become app-local or require
   Three.js at runtime.
2. **Optional cache trigger.** What measured startup, streaming, compiler-size,
   or replacement-pack cost would justify a discardable prepared-figure cache?
   If that trigger is reached, define invalidation from semantic content,
   compiler version, settings, and platform-neutral representation then—not
   before.
3. **Legacy primitive removal trigger.** What explicit schema-version and
   replacement-pack compatibility decision would justify removing sphere,
   capsule, and cylinder parsing after the first-party migration is complete?
   Do not conflate zero canonical use with permission to break schema-v1 input.
4. **Material expansion.** After Tactical 181's opaque color/box-face-texture
   subset, which of alpha mode, roughness, metalness, emissive behavior, and
   face shading belong in the shared shader contract?
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
8. **Legacy actors.** When do remaining debug/item shapes and newly promoted
   canonical figures join the prepared path, and how long does the CPU-baked
   fallback remain supported?
9. **Animation curves.** Are linear key tracks plus quaternion interpolation
   sufficient, or do authored tangents/interpolation modes become necessary
   to bound smooth-motion error?
10. **Remote motion.** What interpolation delay and correction policy belongs
    in the shared actor presentation contract, independently of figure pose?

## Validation Direction

Each pixel-producing slice must capture and inspect output at its first
drawable milestone. The eventual campaign should include:

- compiler determinism and malformed/budget-exceeding asset tests;
- exact prepared counts, bounds, part indices, winding, and normals, plus UV
  tests for explicitly textured box faces and diagnosed legacy cuboid proxies;
- Asset Lab semantic baseline sheets and native prepared-output captures;
- labeled side-by-side Three.js/engine sheets using the same review camera,
  projection, dimensions, grounding, and pose time;
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
- Canonical semantic JSON boundary:
  [`../../tools/asset-lab/src/figure-json.ts`](../../tools/asset-lab/src/figure-json.ts)
- First-party source/output drift gate:
  [`../../tools/asset-lab/src/sync-first-party.ts`](../../tools/asset-lab/src/sync-first-party.ts)
- Asset Lab preview geometry:
  [`../../tools/asset-lab/src/scene.ts`](../../tools/asset-lab/src/scene.ts)
- Asset Lab export:
  [`../../tools/asset-lab/src/export.ts`](../../tools/asset-lab/src/export.ts)
- Restored upright-bear source:
  [`../../tools/asset-lab/examples/upright_bear/figure.ts`](../../tools/asset-lab/examples/upright_bear/figure.ts)
- Figure source loading:
  [`../../native/crates/mclone-assets/src/figure.rs`](../../native/crates/mclone-assets/src/figure.rs)
- Prepared-figure startup compiler:
  [`../../native/crates/mclone-assets/src/prepared_figure.rs`](../../native/crates/mclone-assets/src/prepared_figure.rs)
- Prepared-figure GPU proof:
  [`../../native/crates/mclone-render/src/prepared_figure.rs`](../../native/crates/mclone-render/src/prepared_figure.rs)
- Native offscreen figure review:
  [`../../native/apps/mclone-figure-review/src/main.rs`](../../native/apps/mclone-figure-review/src/main.rs)
- Paired semantic/native comparison:
  [`../../tools/asset-lab/src/compare.ts`](../../tools/asset-lab/src/compare.ts)
- Current runtime figure compiler:
  [`asset_lab_figure.rs`](../../native/crates/mclone-render/src/asset_lab_figure.rs)
- Current actor cache/draw path:
  [`entity.rs`](../../native/crates/mclone-render/src/entity.rs)
- First bounded implementation proof:
  [`181`](../tactical/181-compiled-figure-static-box-proof.md)

## Recommended Next Work

Tactical [`200`](../tactical/200-box-only-figure-authoring.md) is complete: its
20-figure migration baseline is entirely sparse cuboid rigs, all 18 rounded
sources are isolated in the deprecated compatibility lane, and promoted
Chicken is re-baselined. The approved Deer/Zebra/Panda, Owl/Parrot/Eagle, and
Fish/Dolphin/Shark content batches extend the canonical authoring roster to 29
without changing the promoted runtime set. Instancing is now the strongest
independent performance candidate; box-part LOD and a measured sampled/GPU
pose path remain separate follow-ups.

Do not add a persisted compiled format or revive exact curved tessellation.
The existing actor-record boundary keeps later instancing, LOD, and GPU crowd
evaluation additive, while the real high-count fixture should establish their
crossover.
