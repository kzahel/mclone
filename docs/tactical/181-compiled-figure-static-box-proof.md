# 181: Compiled Figure Static-Box Proof

Status: proposed 2026-07-16. Implementation has not started.

Topic: `compiled-figure-rendering`

Workstream: Asset Lab tooling plus shared native Rust assets/rendering. Desktop
offscreen validation first, then synthetic stereo/multiview and production
browser WebGPU validation. Platform apps may expose diagnostic selection only;
they do not own figure, animation, material, or renderer policy.

## Goal

Prove the smallest end-to-end version of the selected compiled-figure
architecture:

```text
player figure.ts
  -> validated semantic FigureAsset
  -> deterministic offline compiler
  -> one self-contained compiled figure artifact
  -> Asset Lab compiled-output preview
  -> mclone-assets validation/loading
  -> shared mclone-render static local-space draw
```

Use the box-only Asset Lab player because it exercises hierarchy, pivots,
per-face material selection, a real 8x8 ASCII face texture, first-person part
classification, and a clip-bearing rig without requiring curved primitives.
The compiled rest-pose target is 12 parts, 288 vertices, 432 indices, and 144
triangles. The current runtime bridge represents the same source as 12 base
cuboids plus 64 texture-cell overlay cuboids, or 1,824 cuboid vertices and
2,736 indices before actor-list duplication.

This is also representative of the actual texture scope, not merely a
convenient simplification. Across the current 18 figures and 516 parts, only
29 parts contain texture references and all 33 applications target explicit
box faces. All 183 spheres, capsules, and cylinders use solid materials. This
tactical therefore proves sparse planar box-face decals; it does not begin a
general UV-unwrapping system.

Success means the checked/generated artifact is deterministic, Asset Lab and
the Rust loader agree on its exact data, and the shared renderer visibly draws
its static rest pose with the face texture as UV-mapped pixels. It does not mean
the production player switches paths yet.

## Why This Is The First Slice

This proof settles the contracts that every later optimization depends on:

- one geometry authority instead of independent Three.js and Rust
  tessellators;
- exact coordinates, normals, winding, UV orientation, atlas sampling, part
  indices, bounds, and provenance;
- static local-space vertex/index buffers rather than world-space CPU baking;
- a rig/palette-shaped draw contract without dynamic animation yet; and
- one shared mono/stereo/multiview renderer that can later be instanced.

Starting with crowd compute, per-actor ranged uploads, or generated LOD before
these facts are stable would optimize the current approximation or freeze an
unreviewed vertex/asset layout.

## Selected Tactical Decisions

These decisions are provisional at the campaign level but binding inside this
tactical so implementation can be evaluated coherently.

### Compiler authority

- Add a deterministic offline compiler module under `tools/asset-lab`.
- The compiler owns box tessellation directly. A test asserts equality with
  the repository-pinned Three.js `BoxGeometry` attributes/groups, so Three.js
  is a drift oracle rather than the byte authority; extraction from pinned
  Three.js constructors is deferred to the curved-primitive phase. It must
  not serialize live Three.js objects or require Three.js in any runtime.
- The serialized artifact becomes geometry authority. Asset Lab's compiled
  preview reconstructs `BufferGeometry` from it; Rust consumes the same
  attributes and indices without retessellation.
- Record the compiler format/version, semantic source hash, compiler input
  hash, and resolved Three.js version or lock hash. A dependency change must
  make stale output fail, not silently drift.
- Revisit a Rust/WASM compiler core only after this proof exposes a concrete
  determinism, replacement-pack, or maintenance need.

### Artifact container

Emit one self-contained, little-endian artifact per figure so a layered asset
source cannot resolve metadata and payload from different packs. The exact
extension and Rust type names may change, but v1 has:

1. a fixed magic and container version;
2. a length-delimited canonical UTF-8 JSON header;
3. 4-byte-aligned binary buffer views; and
4. a payload/content hash covering all authoritative bytes.

The header carries:

- semantic and compiled schema versions;
- figure name, coordinate/normalization convention, source/compiler
  provenance, and byte/count budgets;
- stable part indices, names, parent indices, base local TRS/pivot,
  first-person visibility, and bounds;
- canonical clip tracks and locomotion metadata, even though this tactical
  does not evaluate them;
- LOD records, with only LOD 0 permitted here;
- typed buffer-view offset/count/stride/index-format descriptors;
- opaque material slots and draw ranges;
- one deterministic RGBA8-sRGB atlas description and sampler policy; and
- whole-figure and per-part bounds.

Reject unknown required features, overlapping/out-of-range buffer views,
invalid indices/part ids, hierarchy cycles, non-finite data, hash mismatch,
unsupported formats, and declared counts/bounds that disagree with payload
data. Put explicit conservative byte/vertex/index/part/material/texture limits
in `mclone-assets`; an untrusted replacement pack must not cause unbounded
allocation.

### Geometry and material subset

- Compile only box primitives for this tactical. Any sphere, capsule, or
  cylinder is a clear unsupported-feature error.
- Output runtime-ready Mclone actor-local coordinates: `+Y` up and `+Z`
  forward. The compiler owns the Asset Lab `-Z`-front conversion, normal
  conversion, triangle-winding reversal, feet grounding, and current unit-
  height normalization. Encode the convention in the header.
- Use interleaved or separately described static attributes for local position,
  normal, UV, and one rigid `part_id`. The logical contract must not assume a
  CPU-baked world position. UV storage does not imply authored unwrapping:
  untextured material ranges may ignore it.
- Select `u16` or `u32` indices explicitly per LOD and validate the choice.
- Build one deterministic opaque atlas. ASCII textures become ordinary RGBA
  texels; constant materials may occupy deterministic solid-color atlas
  regions. Sort source names before packing, define gutters and origin, use
  nearest filtering, and disable mip use in the proof.
- Generate canonical planar `0..1` coordinates independently for each box
  face and remap only explicitly textured faces into their atlas regions. The
  author supplies a face name and texture name, not UV vertices or seams.
- Preserve material slots/draw ranges in metadata even if the initial opaque
  atlas permits one draw. Roughness, metalness, emissive, alpha blending,
  normal maps, and arbitrary Three.js materials are unsupported.

### Runtime proof shape

- `mclone-assets` owns format parsing, validation, provenance, and typed CPU
  artifact data. It does not own GPU objects.
- `mclone-render` owns immutable atlas/vertex/index resources, the rest-pose
  part palette, shaders, and draw commands.
- Keep immutable compiled resources shareable and mutable draw/palette state
  per drawable world, matching Tactical 179.
- The proof vertex operation is already the durable shape:

  ```text
  world_position = actor_world * rest_part_matrix[part_id] * local_position
  ```

- Light the compiled figure from one per-actor packed light value sampled at
  the actor position, not per-vertex baked light. Comparison against the
  CPU-baked bridge treats any resulting gradient difference as a deliberate
  lighting-model change.
- Draw one proof actor at a time. Do not add instance buckets, sampled phase
  palettes, compute, or dynamic part uploads.
- Keep the current CPU-baked `ActorMeshCache` as the default production actor
  path. A diagnostic fixture may select the compiled player; absence or
  rejection of the artifact must leave ordinary gameplay behavior unchanged.

## Explicit Non-Goals

- No production player, bear, chicken, or entity migration.
- No animated part-palette updates or presentation-cadence interpolation work
  yet.
- No GPU clip sampling, compute pass, shared phase palette, or crowd fixture.
- No instancing, batching rewrite, per-actor ranged CPU mesh cache, or actor
  topology/index optimization.
- No sphere, capsule, cylinder, generated LOD, billboard, or impostor.
- No weighted skinning, morph targets, IK, glTF, or arbitrary mesh import.
- No transparent material, general PBR parity, mip chain, or texture-pack
  atlas merger.
- No whole-character unwrap, cross-part texture continuity, manual UV editor,
  or curved-surface texture controls.
- No protocol, authority, collision, AI, spawning, or player-appearance
  contract change.
- No app-local rendering implementation or platform-specific figure format.

## Implementation Slices

Only one slice is active at a time. Each pixel-producing slice captures and
inspects its first drawable result before proceeding.

### Slice 0: freeze baseline and acceptance data

- Capture current Asset Lab player sheet and native actor review sheet under
  `/tmp` with exact commands and dependency revisions recorded here.
- Add or expose current CPU bridge accounting for base/overlay cuboids,
  generated vertices/indices, CPU build bytes/time, and uploaded bytes. Do not
  broaden this into the HP-1 cache optimization.
- Record semantic player source hash, part/material/texture/clip counts,
  intended Three.js geometry counts, rest bounds, and visible face
  orientation.
- Record the repository-wide sparse-texture inventory: 516 parts, 29 textured
  parts, 33 explicit box-face applications, and zero curved applications.
- Define numerical tolerances for bounds/normals and an image comparison rule
  for semantic versus compiled Asset Lab preview.

Gate: the baseline must reproduce 12 parts, 288 intended box vertices, 432
indices, one 8x8 source texture, and the current 12+64-cuboid runtime
approximation. If it does not, update the topic evidence before coding.

### Slice 1: deterministic artifact compiler and validator fixture

- Add typed compiled-header and buffer-view definitions in Asset Lab.
- Generate box positions, normals, UVs, indices, and material groups directly
  in the compiler, with an oracle test against pinned Three.js `BoxGeometry`
  output; apply the declared coordinate/normalization conversion once.
- Resolve stable part indices and hierarchy, attach `part_id`, copy canonical
  clip/locomotion data, and compute exact bounds/counts.
- Compile ASCII textures and solid materials into the deterministic atlas.
- Serialize one self-contained artifact and add a CLI command plus package
  wrapper that writes it.
- Add byte-for-byte double-build tests, golden header/count tests, source-hash
  invalidation, malformed semantic input tests, and budget failures.
- Generate the first-party player artifact into its pack path and update the
  asset pack/lock through the normal scripts. Derived output is committed only
  with its reproducibility gate and source provenance.

Gate: two clean compiles produce identical bytes; the player artifact has 12
parts, 288 vertices, 432 valid indices, outward normals/winding, finite grounded
bounds, and an atlas whose face texels match the ASCII palette and orientation.

### Slice 2: Asset Lab compiled-output preview

- Add a loader that reconstructs the exact serialized buffers, atlas, parts,
  and rest palette without calling primitive constructors.
- Make a deliberate compiled preview/sheet mode and provide a semantic versus
  compiled A/B or overlay comparison during bring-up.
- Show source/artifact hashes, counts, bounds, atlas, material ranges, and
  stale status in the review output.
- Add UV orientation and normal/winding diagnostic modes.
- Capture and inspect front, side, and three-quarter static player output.

Gate: compiled preview displays the face texture on the intended north/front
face with correct orientation, no overlay geometry, correct grounding, and no
visible semantic-preview silhouette drift. Attribute/count/bounds agreement is
exact or within the recorded floating-point tolerance.

### Slice 3: Rust loading and first shared mono pixels

- Add the bounded artifact parser and typed data under `mclone-assets`, with
  fixture parity against the TypeScript header and malformed/container budget
  tests.
- Load through the ordinary `AssetSource`/provenance chain; never read a raw
  filesystem path from `mclone-render`.
- Add immutable compiled figure GPU resources and a distinct local-space
  vertex layout under `mclone-render`.
- Upload static vertex/index/atlas data once and build the rest part palette.
- Add an opaque compiled-figure shader and mono draw path using actor, part,
  and local transforms. Transform normals consistently.
- Expose the player only in an actor review/proof fixture; leave ordinary
  actor selection on `ActorMeshCache`.
- At the first drawable milestone, capture `/tmp` output and inspect face UV,
  winding, grounding, scale, depth, lighting, and front/side silhouettes.

Gate: a frozen compiled player is visible through shared rendering, reports
288 vertices/432 indices and one atlas, performs no texture-cell geometry
expansion, and reuses immutable GPU buffers across unchanged proof frames.
Feature-off actor review and gameplay captures remain unchanged.

### Slice 4: stereo, multiview, and browser portability

- Add the compiled-figure path to both ordinary per-view and full-frame
  multiview pipelines with each eye/layer using its own view/projection data.
- Reuse the same artifact, rest palette, atlas, and actor transform across
  paths; do not create an eye-local geometry cache.
- Exercise native synthetic stereo in per-eye and full-frame multiview modes
  and inspect both eyes/layers.
- Exercise the same shared proof renderer in production browser WebGPU. A thin
  browser diagnostic selector is acceptable; no browser-local figure or
  material policy is.
- Run normal native/web build and shader-validation gates, plus Android/Quest
  compile checks through the repository scripts where required by the current
  platform validation policy.

Gate: mono, stereo per-eye, multiview, and browser WebGPU all show the same
compiled player orientation/materials; no path independently recompiles
semantic primitives or uploads topology per eye/frame.

### Slice 5: close the proof and choose the next tactical

- Record artifact bytes, atlas bytes, retained GPU bytes, draw calls,
  vertices/indices, upload counts, and proof CPU/GPU timing beside the old
  bridge baseline. Treat one-player timing as accounting, not a claimed crowd
  win.
- Confirm stale artifact, replacement-pack provenance, missing artifact, and
  feature-off fallback behavior.
- Update
  [`../topics/compiled-figure-rendering.md`](../topics/compiled-figure-rendering.md),
  [`../topics/performance.md`](../topics/performance.md), this tactical, Asset
  Lab documentation, and the tactical index with landed evidence.
- Decide whether the container/atlas/coordinate contract is promoted as
  compiled schema v1 or revised before another asset uses it.

Stop after this decision. The next tactical should be true curved primitive
and material parity, not animation: this proof already draws through
`rest_part_matrix[part_id]`, so the vertex layout, palette indexing, and
shader contract are exercised statically, and animation adds evaluation
semantics rather than layout.

## Validation Matrix

Exact command names may be added as the CLI surface lands. Use existing
wrappers where available instead of hand-rolling platform builds.

| Gate | Required evidence |
|---|---|
| compiler | typecheck; byte-identical double build; golden header/counts; malformed/budget tests |
| Asset Lab pixels | semantic and compiled player sheets under `/tmp`, inspected; UV/normal diagnostics |
| assets | Rust parser/validation tests; source/artifact hash and pack-lock checks; replacement provenance |
| native mono | compiled actor review capture under `/tmp`, inspected; immutable upload counters |
| native stereo | synthetic per-eye and full-frame multiview captures/receipts, inspected |
| browser | production WebGPU proof pixels plus normal web build/smoke |
| fallback | normal actor review/gameplay pixels unchanged; missing/rejected artifact keeps old path |
| hygiene | relevant Rust tests/checks, Asset Lab tests/typecheck, pack checks, `git diff --check` |

Do not claim pixel completion from unit tests alone. Capture at Slice 2's first
compiled Asset Lab pixels, Slice 3's first native pixels, and Slice 4's first
stereo/browser pixels, inspecting each before the slice grows.

## Promotion And Rollback Rules

Promote the artifact contract only if:

- output is deterministic and source-attributable;
- the compiled preview, Rust loader, and GPU layout agree on every count and
  buffer range;
- UVs, normals, winding, bounds, and grounding pass direct review;
- all required view paths render correctly;
- malformed/untrusted assets are bounded and rejected; and
- the production CPU-baked path remains untouched and measurable.

If the proof fails, remove or leave the diagnostic renderer unselected and
retain the semantic JSON/current `ActorMeshCache` path. Do not add a second
runtime tessellator to work around artifact disagreement. Fix or revise the
compiler contract, bump the compiled schema if needed, and regenerate.

## Planned Follow-Up Sequence

Later work stays in separate tacticals so each optimization has its own visual
and performance claim:

1. true sphere/capsule/cylinder tessellation with Three.js positions, normals,
   indices, and default UV attributes preserved mechanically, while current
   curved parts remain solid-material and general curved-texture authoring
   stays deferred;
2. presentation-rate CPU rigid-part animation using static geometry and final
   actor palettes, including local TRS/quaternion interpolation and remote
   world-transform interpolation;
3. asset-by-asset production migration and deletion of texture-cell/non-box
   cuboid approximations only after parity gates;
4. per-figure/material/LOD instancing plus generated spatial LOD and
   hysteresis; and
5. only after a measured high-count fixture, optional shared sampled clips and
   GPU palette expansion with the exact CPU evaluator retained as fallback.

The thousand-chicken lane belongs to steps 4-5. It is a design point for
culling, LOD, batching, and evaluator crossover—not an acceptance burden for
this static player proof.

## Code And Documentation Map

- Durable direction:
  [`../topics/compiled-figure-rendering.md`](../topics/compiled-figure-rendering.md)
- Immediate actor-cache issue:
  [`../topics/performance.md`](../topics/performance.md#hp-1-split-actor-pose-updates-from-whole-mesh-rebuilds)
- Authoring contract:
  [`../../tools/asset-lab/src/dsl.ts`](../../tools/asset-lab/src/dsl.ts)
- Current semantic preview:
  [`../../tools/asset-lab/src/scene.ts`](../../tools/asset-lab/src/scene.ts)
- Current semantic exporter:
  [`../../tools/asset-lab/src/export.ts`](../../tools/asset-lab/src/export.ts)
- Proof source figure:
  [`../../tools/asset-lab/examples/player/figure.ts`](../../tools/asset-lab/examples/player/figure.ts)
- Current semantic Rust loader:
  [`../../native/crates/mclone-assets/src/figure.rs`](../../native/crates/mclone-assets/src/figure.rs)
- Current runtime approximation:
  [`../../native/crates/mclone-render/src/asset_lab_figure.rs`](../../native/crates/mclone-render/src/asset_lab_figure.rs)
- Current actor cache and draw path:
  [`../../native/crates/mclone-render/src/entity.rs`](../../native/crates/mclone-render/src/entity.rs)
- Existing native review harness:
  [`../../native/apps/mclone-native-client/src/headless.rs`](../../native/apps/mclone-native-client/src/headless.rs)
- Early bridge history:
  [`112`](112-asset-lab-runtime-actor-geometry.md)
- Mutable actor-world ownership:
  [`179`](179-composable-world-presentation-and-live-preview-actors.md)
- Validation policy:
  [`../platforms.md`](../platforms.md#validation-policy)
