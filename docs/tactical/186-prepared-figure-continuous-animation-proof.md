# 186: Prepared Figure Continuous Animation Proof

Status: active 2026-07-16. Slices 0-1 are complete and Slice 2 is next. Stop
for human clip review after Slice 2 before any production actor migration.

Topic: `compiled-figure-rendering`

Workstream: shared native Rust assets/rendering plus Asset Lab review tooling.
Desktop offscreen validation first. Production actor selection remains on the
CPU-baked `ActorMeshCache` throughout this tactical.

## Goal

Extend Tactical
[`181`](181-compiled-figure-static-box-proof.md)'s approved static player into
the smallest durable animated prepared path:

```text
canonical semantic walk keys
  -> startup-compiled indexed clip tracks
  -> continuous local TRS evaluation at presentation time
  -> parent-composed final part palette
  -> one mutable palette upload
  -> existing immutable geometry/atlas draw
```

Render the canonical player's walk through Three.js and the shared native
prepared renderer at corresponding times. Produce a reviewable animation and
frame sheet, then pause for human approval of motion, pivots, hierarchy,
grounding, loop continuity, and interpolation before changing gameplay.

Success is a continuously evaluated CPU palette over immutable topology. It is
not a production migration, crowd benchmark, fixed animation tick, GPU clip
evaluator, or persisted compiled format.

## Binding Decisions

- `mclone-assets` owns renderer-neutral clip compilation and evaluation beside
  `PreparedFigure`; apps and renderers do not reinterpret semantic keys.
- Evaluation accepts continuous `f64` presentation time. It has no 60, 120,
  500, or other fixed-Hz ceiling and does not advance by a frame counter.
- Looped clips wrap by duration; non-looped clips clamp. A presented frame
  samples its actual time even after an earlier frame was missed.
- Translation and scale interpolate linearly. Rotation endpoints are local
  quaternions and use normalized shortest-path interpolation. Already-composed
  matrices are never linearly blended.
- Sparse translation, rotation, and scale channels are sampled independently
  and hold their first/last authored values outside their own key ranges. A key
  for one channel never resets another channel.
- Key transforms remain additive to the authored base translation/Euler
  rotation, matching current Asset Lab semantics at keys. The startup compiler
  indexes tracks by part and preserves locomotion/contact metadata.
- Evaluation composes the part hierarchy in local space, including pivots and
  scale, and only then applies the established handedness/feet-grounding/unit-
  height normalization.
- One CPU evaluation produces final actor-local part matrices. The renderer
  writes only the mutable palette for an animated frame; vertex/index/atlas
  buffers and pipelines remain resident and unchanged.
- The first renderer still draws one prepared player. No instancing, actor
  record buffer, phase bucket, shared sampled phase palette, compute pass, or
  GPU hierarchy expansion belongs here.
- The semantic JSON remains the only persisted representation. Compiled tracks
  and palettes are in-memory Rust types without compatibility promises.
- The native and Three.js review paths share clip name, exact times, camera,
  projection, dimensions, and background. Lighting RGB remains non-authority,
  as accepted in Tactical 181.

## Explicit Non-Goals

- No replacement of `ActorMeshCache` in gameplay, actor review, browser, XR,
  chicken, bear, remote-player, or first-person rendering.
- No world-transform/network interpolation, prediction, reconciliation, or
  movement-authority change. This proof isolates figure pose.
- No animation blending, layers, head look, wing override, damage pose, IK,
  morph target, or weighted skinning.
- No curved primitive support, material expansion, mip chain, LOD, billboard,
  impostor, instancing, or thousand-chicken fixture.
- No sampled fixed-rate pose hold. Authored keys may be sparse, but displayed
  poses interpolate continuously between them.
- No disk cache, sidecar geometry, binary clip file, or generated palette
  asset.

## Implementation Slices

### Slice 0: shared clip and pose contract (complete 2026-07-16)

- Replace raw copied clips inside `PreparedFigure` with startup-compiled,
  part-indexed tracks while retaining locomotion/contact metadata.
- Retain enough per-part local base/pivot data and figure normalization to
  recompose the approved rest matrices exactly.
- Add allocation-reusing continuous evaluation into a caller-owned matrix
  palette, returning clip duration/local time and evaluation counters.
- Cover rest equivalence, hierarchy/pivot behavior, loop/clamp boundaries,
  sparse optional channels, scale, shortest quaternion path, deterministic
  repeated evaluation, and distinct results at sub-120/sub-500-Hz time steps.
- Keep the renderer static in this slice.

Gate: evaluating rest reconstructs every Tactical 181 rest matrix within a
documented tolerance, and arbitrary-time clip evaluation is deterministic,
finite, hierarchy-correct, and cadence-independent.

Gate evidence:

- `PreparedFigure` now retains startup-compiled part-indexed tracks, indexed
  locomotion contacts, source fps/duration/loop metadata, a parent-before-child
  evaluation order, semantic local base/pivot transforms, and the existing
  actor normalization. These are in-memory types; semantic JSON did not change.
- `evaluate_prepared_figure_clip_into` accepts `f64` presentation time and
  reuses a caller-owned matrix vector. It independently samples sparse local
  channels, linearly interpolates translation/scale, shortest-path slerps
  mirrored endpoint quaternions, composes parent content, and applies
  normalization once at roots.
- Recomposition of all 12 player rest matrices matches the approved static
  palette within `2e-5`. Player evaluations at `0.123s` and `0.125s` are
  distinct, repeat exactly at the same time without capacity growth, and wrap
  to the same palette one `0.9s` duration later.
- Focused fixtures prove non-loop clamp, `170` to `-170` degree shortest-path
  rotation through 180 degrees, scale-before-child composition even when the
  child precedes its parent in authored order, sparse-channel holds, unknown
  clip/non-finite-time rejection, and invalid locomotion rejection.
- All 58 `mclone-assets` tests, the prepared renderer shader/layout tests, and
  the figure-review app check pass. The renderer remains static in this slice.

### Slice 1: mutable palette over immutable topology (complete 2026-07-16)

- Change `PreparedFigureDrawResources` from a one-time rest-palette upload to
  one resident mutable palette buffer initialized from rest pose.
- Add an explicit palette update method with size/finite validation and
  separate immutable-upload versus mutable-palette-write counters.
- Render multiple player-walk frames through the existing mono path. Assert
  four initial residency uploads total, one palette write per changed
  presented pose, no topology/atlas upload growth, and visibly distinct
  non-keyframe pixels.
- Preserve ordinary per-eye and multiview resource ownership. Animation proof
  pixels may stay mono until the clip is approved; no app-local shader fork.

Gate: arbitrary presentation times change only the palette and view uniforms;
the prepared topology, atlas, draw count, and resource identities stay fixed.

Gate evidence:

- `PreparedFigureDrawResources` retains the same vertex, index, atlas,
  pipelines, and one palette buffer. `write_palette` validates exact part count
  and finite matrices, reuses serialization scratch, and writes only active
  matrices; mono, per-eye, and multiview bind the same buffer.
- The initial rest residency still performs four uploads: vertex, index,
  atlas, and the padded 4,096-byte palette initialization. Each animated player
  pose writes 12 active matrices, or 768 bytes; it does not upload the 14,976
  vertex bytes, 864 index bytes, or 520 atlas bytes again.
- `mclone-figure-review --animation-proof` captured eight front-view walk poses
  at `0.045`, `0.09`, `0.123`, `0.125`, `0.45`, `0.855`, `0.899`, and `0.901`
  seconds under `/tmp/mclone-prepared-animation/native-player/`. The first
  non-keyframe pixels and representative midpoint/wrap frames were inspected;
  limbs, sleeves, feet, torso, head, grounding, depth, and inherited motion are
  coherent in the native path.
- The receipt records four initial uploads, eight palette writes totaling
  6,144 bytes, eight view writes, and one unchanged 288-vertex/432-index draw
  per frame. The `0.123` to `0.125` second pair differs by 2,200 pixels despite
  only 2 ms of elapsed presentation time; `0.899` to wrapped `0.901` differs by
  2,053 pixels without a held 12 fps pose.
- Palette validation tests reject wrong-sized and non-finite matrices. Focused
  prepared-render tests and the figure-review check pass. No production actor,
  browser, or XR selector changed.

### Slice 2: synchronized Three.js/native clip review

- Extend the review contract with `walk`, duration, exact sample times, and an
  animation output cadence used only for capture—not runtime evaluation.
- Render corresponding raw frames through canonical-JSON Three.js and shared
  native preparation, preserving unscaled panels and receipts.
- Produce a labeled key/non-keyframe sheet and a side-by-side multi-cycle MP4
  under `/tmp`. Include at least one time that is not an authored key and
  adjacent time samples close enough to demonstrate continuous interpolation.
- Inspect the first native animated pixels immediately. Check limb direction,
  pivots, inherited sleeve/foot motion, torso bob, head motion, grounding,
  loop seam, silhouette, and frame-to-frame continuity.

Gate and stop condition: present the sheet/video and receipts to the human.
Pause work for approval or correction. Do not begin production migration based
only on numerical tests or self-inspection.

### Slice 3: post-review decision only

After approval, update the topic with accepted differences and write the next
bounded tactical. Likely choices are an opt-in production player path with
remote/world transform interpolation, or a second asset/curved primitive
proof. Do not implement either inside this tactical.

## Validation Matrix

| Gate | Required evidence |
|---|---|
| clip compiler | deterministic indexed tracks, invalid input bounds, exact duration/metadata |
| evaluator | rest equality, loop/clamp, shortest quaternion, scale, hierarchy, arbitrary-time tests |
| residency | immutable upload count unchanged; explicit bounded palette writes only |
| native pixels | first animated capture under `/tmp` inspected before growing the slice |
| cross-renderer | corresponding raw frames, labeled sheet, multi-cycle MP4, shared receipt |
| cadence | adjacent non-key times differ; no evaluator frequency constant or frame-step accumulator |
| fallback | production `ActorMeshCache` selection and pixels remain unchanged |
| hygiene | focused Rust tests/checks, Asset Lab test/typecheck, `git diff --check` |

## Review Questions

Human review should answer:

1. Do the same limbs move in the same directions and phases in both renderers?
2. Do sleeves, feet, hair, and shirt band inherit their parent motion without
   separating or drifting?
3. Are shoulder/hip pivots, torso bob, head motion, scale, and grounding
   credible at non-key times?
4. Is the loop seam continuous, including the frame immediately before and
   after wrap?
5. Does the animation remain smooth when captured above the authored key rate,
   without a visible hold at 12 fps or any renderer-imposed display limit?
6. Are the accepted lighting differences still the only obvious visual-model
   mismatch?

## Code And Documentation Map

- Durable direction:
  [`../topics/compiled-figure-rendering.md`](../topics/compiled-figure-rendering.md)
- Static predecessor:
  [`181`](181-compiled-figure-static-box-proof.md)
- Semantic clip schema:
  [`../../native/crates/mclone-assets/src/figure.rs`](../../native/crates/mclone-assets/src/figure.rs)
- Prepared compiler/evaluator owner:
  [`../../native/crates/mclone-assets/src/prepared_figure.rs`](../../native/crates/mclone-assets/src/prepared_figure.rs)
- Prepared GPU resources:
  [`../../native/crates/mclone-render/src/prepared_figure.rs`](../../native/crates/mclone-render/src/prepared_figure.rs)
- Native review app:
  [`../../native/apps/mclone-figure-review/src/main.rs`](../../native/apps/mclone-figure-review/src/main.rs)
- Asset Lab semantic evaluator/reference:
  [`../../tools/asset-lab/src/scene.ts`](../../tools/asset-lab/src/scene.ts)
- Paired review orchestration:
  [`../../tools/asset-lab/src/compare.ts`](../../tools/asset-lab/src/compare.ts)
