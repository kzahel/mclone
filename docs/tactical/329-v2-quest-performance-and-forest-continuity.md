# Tactical 329: V2 Quest Performance And Forest Continuity

Status: **active 2026-08-21. Tactical committed before implementation. The
matched physical Quest baseline identifies an invalid settle gate, a 12.6x V2
cold-convergence gap, and forest coverage that collapses when proxy-tree
records stop.**

Topic: `procedural-horizon-clipmap`
Topic: `continental-ecoregion-planning`
Topic: `performance`

## Instruction Synthesis

Mclone Overworld V2 is becoming visually promising, but its usefulness now
depends on product behavior rather than another abstract terrain layer. At
large zoom, V2's extensive forests change character abruptly when individual
LOD trees disappear. A coarse sprite-, texture-, or similarly cheap forest
representation should preserve their broad mass. Performance also needs direct
attention because V2 is experimental and must not silently become much more
expensive than V1 on the Quest headset.

Create this tactical, then implement it end to end. Keep V1 selectable and the
default. Preserve the shared procedural-horizon architecture, exact/proxy tree
ownership, fixed clipmap residency, and mono/per-eye/multiview behavior. Measure
matched, fully rendered V1 and V2 workloads rather than accepting a fast result
from an incomplete scene.

## Product Question

Can a dense V2 forest retain a recognizable continuous canopy from exact
blocks through continental zoom while a matched Quest 3 run proves honest
readiness, bounded cost, and useful V1-relative performance?

## Baseline And First Finding

The baseline is physical Quest 3, release APK, 72 Hz, `1680x1760` per eye,
foveation Off, the production per-eye frame-overlap path, Low Distant Terrain,
render distance 8, seed `12345`, and the V2 humid-jungle review region near
block `(10240, -54784)`.

Fresh stationary convergence is not close:

| Profile | Full quiet settle | Exact columns | Ready exact sections |
|---|---:|---:|---:|
| V1 | `10.593 s` | `289` | `819` |
| V2 | `133.748 s` | `289` | `735` |

Once both views are genuinely complete, stationary app-work p50/p95 is
`10.077/10.721 ms` for V1 and `10.659/11.222 ms` for V2. Both spend `0.2%` of
frames over the 72 Hz period. Each fresh sample also contains one large render
encoding stall, `9.47 s` for V1 and `21.77 s` for V2; one occurrence per lane
is attribution evidence, not a stable percentile.

The existing moving result is invalid as a comparison. V1 starts its 30-second
orbit with all `289` exact columns and keeps the exact center ready. V2 starts
with only three exact columns, reports the center unavailable in 1,491 of 1,986
horizon sample frames, and draws only three or four exact sections. Its lower
reported frame time is therefore an under-rendered scene, not a performance
win. The settle detector currently treats queue quiet plus a ready horizon as
sufficient and does not require the requested exact footprint.

V2 proxy vegetation has another discontinuity. Complete individual records
exist through spacing 4, deterministic subsets survive at spacing 8 and 16,
and only continuous forest summaries remain beyond that. The terrain tint uses
those summaries, but there is no coarse geometric canopy consumer. Large V2
forests consequently lose most of their visible volume at the representation
boundary.

## Representation Decision

Use the existing semantic forest summary as one source across four perceptual
bands:

1. exact block trees in ready exact chunks;
2. near procedural proxy trees with whole-tree XOR ownership;
3. a middle/far world-oriented low-poly canopy carpet; and
4. the existing continental forest albedo at the broadest projected scale.

The first implementation adds band 3 without enumerating tree records. Each
committed coarse tile exposes at most a fixed `16 x 16` canopy-cell lattice.
Canopy vertices read the tile's already-resident forest coverage, family,
mean height, height variation, and opening influence. Neighboring cells derive
their corners from shared world-coordinate samples so coverage remains stable
across tile and LOD movement.

The canopy is world-oriented geometry, not camera-facing billboards. It must
use the same immutable per-view uniforms, visibility masks, fog, exact mask,
inner-hole ownership, and normal per-eye/full-frame multiview pipelines as the
existing horizon. It allocates no per-forest CPU records or GPU instance
buffer. Its maximum vertex work is a function of resident tiles, never forest
area or world size.

Individual proxy trees remain the near owner. The canopy starts only beyond
the current proxy-vegetation presentation bound and uses a bounded transition
band based on sample spacing and coverage, so it neither doubles dense nearby
trees nor produces an empty ring. Natural diagnostics report canopy tiles,
cells, and vertices separately from individual trees.

## Ownership

- `mclone-worldgen` owns stable forest summary semantics and direct coarse
  evaluation. This tactical may optimize their evaluation but must not add a
  fine-tree dependency to coarse requests.
- `mclone-terrain-view` owns canopy representation, clipmap-level selection,
  exact/inner-hole suppression, mono/per-eye/multiview shaders, fixed budgets,
  and diagnostics.
- `mclone-scene` owns the ready exact component and shared live presentation.
- The Android XR app owns only the diagnostic harness and must derive its
  expected exact footprint from the requested render distance.
- V1 and V2 authoritative generation remain in `mclone-server` and shared
  worldgen. No Quest- or profile-specific gameplay path is allowed.

## Phases And Commit Gates

### Phase 0: Tactical And Honest Measurement Contract

- Commit this tactical before implementation.
- Preserve the raw physical Quest baseline artifacts outside the repository.
- Make the settled benchmark require the full requested square of exact
  columns, a ready exact center, a target-ready horizon, drained bounded work,
  and the existing quiet-frame duration.
- Log expected and observed exact columns in settle progress, start, and final
  horizon receipts.
- Add focused tests that reject the observed three-column V2 state and accept
  a complete render-distance-8 state.
- Commit before using the corrected harness for performance claims.

### Phase 1: Fixed-Budget Canopy Continuity

- Add one procedural canopy pipeline in the shared renderer, including normal
  single-view/per-eye and full-frame multiview variants.
- Read existing packed forest summaries on GPU; do not add forest instance
  uploads or regenerate tree records.
- Use no more than 256 canopy cells per resident tile and expose exact per-level
  tile/cell/vertex counts plus zero incremental resident bytes.
- Preserve inner-hole, far/frustum culling, exact coverage, fog, diagnostic,
  color-transfer, and device recreation behavior.
- Prove that levels beyond the proxy-tree bound draw canopy at a dense V2
  forest while open/desert summaries produce no visible canopy.
- Capture and inspect the first native dense-forest frame before proceeding.
- Commit the independently useful representation.

### Phase 2: Measured V2 Cost Recovery

- Attribute fresh settle time across authoritative generation, dependency
  preparation, exact compilation/upload, horizon terrain, and horizon
  vegetation. Do not optimize from the earlier incomplete orbit.
- Remove repeated V2 work through bounded shared caches, compact batch reuse,
  or direct coarse evaluation at the actual measured owner. Do not reduce
  render distance, exact completeness, tree coverage, clipmap levels, or
  terrain semantics to improve the number.
- Alternate V1/V2 with identical release configuration and include a V1 repeat
  when host or headset variance is material.
- Commit each measured improvement separately and revert candidates that do
  not improve the binding metric or pixels.

### Phase 3: Cross-Platform And Pixel Acceptance

- Run focused worldgen, terrain-view, scene, Android XR, WGSL, and codec tests.
- Capture native exact/composed/broad V2 jungle pixels and inspect continuity
  across the proxy-to-canopy boundary.
- Run the headed WebGPU lane and inspect its captured pixels; do not use
  headless black output as evidence.
- Validate the shared Android build boundary and both XR render-path shader
  contracts.
- Confirm V1 appearance and V2 desert/open regions do not gain false forest.
- Update the LOD, continental-planning, and performance topics with measured
  status and remaining debt.

### Phase 4: Corrected Physical Quest A/B

- Build through `pnpm native:android-xr:apk` and use the public Quest testbed
  provider for target selection, wake/authorization, recovery, and sleep.
- Require all `289` exact columns before each RD8 measurement begins.
- Repeat stationary and moving V1/V2 samples at the same jungle coordinates.
- Reject a lane if the exact center becomes unavailable during a settled
  stationary sample; report every unavailable moving frame rather than hiding
  it in a frame percentile.
- Record cold settle, p50/p95/p99/max app work, over-period rate, Meta app GPU,
  exact sections drawn, tree and canopy work by level, and large stall
  attribution.
- Restore the headset state and keep all pulled logs and screenshots under
  `/tmp`.

## Acceptance Gates

### Correctness

- RD8 settle cannot start below `289` ready exact columns or with an unready
  exact center.
- V1/V2 profile, seed, topology, exact, horizon, and vegetation identities stay
  matched within each lane.
- Exact/proxy whole-tree ownership retains zero missing and zero dual-owned
  records.

### Forest Continuity

- A dense V2 forest retains elevated, family-colored canopy coverage beyond
  the last individual proxy-tree level instead of collapsing to bare ground.
- Coarse canopy coverage is derived from the same summary as nearby trees,
  preserves authored gaps, and does not create forest over desert, water, or
  open-range summaries.
- The visual handoff has no empty clipmap ring or obvious camera-facing sprite
  rotation in mono, stereo, or multiview review.
- Canopy work is at most 256 cells per resident tile and requires zero
  area-proportional records or incremental resident buffers.

### Performance

- Corrected settled samples draw the full requested exact footprint. An
  incomplete sample is a harness failure, not a performance result.
- The canopy adds no CPU vegetation jobs and no instance-upload bytes. Its GPU
  cost is reported independently.
- Target: fully ready V2 stationary p95 within 10% of matched V1 and no new
  stable over-period regression.
- Target: reduce fresh V2 full-view convergence from the `133.748 s` baseline
  to at most 2x matched V1. If that target cannot be met without changing
  product semantics, stop with attributed evidence and a bounded next owner;
  do not weaken readiness.
- Investigate any repeated foreground stall over 2x the frame budget. Keep
  isolated driver/runtime outliers explicit rather than averaging them away.

## Human Review F

Stop with one interactive V2 jungle location that can move continuously from
block-detail forest through broad canopy, plus a matched V1/V2 Quest receipt.
Ask:

1. Does the forest still read as a forest when individual crowns stop being
   legible?
2. Is the proxy-to-canopy handoff less distracting than the previous
   disappearance?
3. Does the canopy preserve authored gaps and terrain form rather than reading
   as one green sheet?
4. Is the corrected performance acceptable enough to keep V2 enabled as an
   experiment, or should the next slice remain performance-only?

V2 remains experimental and V1 remains the default regardless of this review.

## Implementation Progress: CPU Horizon Attribution And Recovery

The first measured owner was not proxy vegetation. V2's continental horizon
was compiling each `69 x 69` tile synchronously inside render encoding, using
the GPU lane's allowance of up to 16 refills per frame. The exact/procedural
frontier compounded that cost: every partial exact-coverage generation built
and synchronously compiled a fresh preferred support set before a later
generation immediately superseded it.

The native shared renderer now has a bounded CPU horizon compiler with one to
four named workers, reserving two reported logical CPUs and capping the pool at
four. Jobs and completions carry source generation, tile request, physical
slot, and slot generation. Admission rejects stale source, request, assignment,
or slot results; fixed GPU buffers and clipmap residency are unchanged. Frame
diagnostics report worker count, in-flight work, submissions, completions,
aggregate compile microseconds, and stale results. WASM keeps the same compiler
and admission contract with a one-tile inline fallback until the common browser
worker protocol grows a terrain payload.

Preferred CPU frontier resources now begin unready, as GPU frontier resources
already did. Partial exact generations coalesce while terrain is warming;
after terrain settles, at most one CPU support tile compiles per frame. The
zero-capacity synchronous fallback remains immediately complete, so no exact
boundary is exposed while preferred support warms. The engine also no longer
starts an unobserved origin-centered fill in its constructor. Its first actual
residency request plans the clipmap, including the valid `(0, 0)` case.

A matched release native High/jungle 480-frame diagnostic shows the bounded
work reduction independently of Quest timing:

| Step | Terrain submissions | Terrain CPU | Preferred commits | Process user time |
|---|---:|---:|---:|---:|
| async only, speculative origin retained | `320` | `3.447 s` | `31` | `10.28 s` |
| frontier generations coalesced | `320` | `3.444 s` | `1` | `6.54 s` |
| first real residency only | `160` | `1.847 s` | `3` | `4.99 s` |

The final run reached all 160 High slots, all 48 vegetation products, zero
in-flight or stale terrain results, ten drawn levels, and the same 17,920
canopy cells / 215,040 canopy vertices as the pre-optimization capture. These
are attribution and regression results, not the physical Quest acceptance;
the corrected RD8 A/B remains the governing gate.

The first corrected physical Quest V2 run then reached all 289 exact columns,
an exact-ready center, all 96 Low horizon slots, and quiet vegetation in
`10.632 s`, down from `133.748 s` and within 1.06x of its current `10.003 s` V1
control. Android reported one available CPU horizon worker, 124 completed tile
jobs, `5.562 s` aggregate worker time, and zero stale results. The result meets
the cold-convergence target without reducing RD8 completeness, clipmap levels,
or authored coverage.

That control also found an avoidable scope regression: enabling coarse canopy
for V1 raised its stationary p95 from the pre-canopy `10.721 ms` baseline to
`12.109 ms`. The severe forest-volume collapse belongs to V2/candidate, while
V1 already has its accepted presentation. Canopy eligibility is therefore
narrowed to V2/candidate so V1 remains a clean unchanged performance control;
a rebuilt V1 repeat remains required before final A/B acceptance.

The first corrected moving repeat revealed that Android logcat truncated the
aggregate horizon line before its late exact-center continuity fields. The XR
harness now emits a separate compact `PERF_HORIZON_READINESS` receipt with the
expected/start/latest exact footprint and the exact-center unavailable-frame
count. Moving acceptance uses that compact receipt rather than inferring
readiness from a truncated diagnostic.
