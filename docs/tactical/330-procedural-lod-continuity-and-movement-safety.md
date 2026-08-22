# Tactical 330: Procedural LOD Continuity And Movement Safety

Status: **active 2026-08-22; procedural-only V2 WebGPU pixels reproduce a
large characteristic-color square between clipmap levels, and retained
composed movement can terminate on an incomplete frontier certificate.**

Topic: `procedural-horizon-clipmap`
Topic: `continental-ecoregion-planning`

## Instruction Synthesis

The continental-scale V2 direction is promising, but moving around for a
while can crash World Explorer and the terrain contains multiple visible
color discontinuities. The exact 5-by-5 chunk footprint is one distinct
handoff, but the more important defect is that procedural LOD levels do not
blend with one another. Procedural levels sample one geography and should read
as nearly one continuous surface, not nested square patches with different
characteristic color.

Use non-composed World Explorer views as the primary proof. Disable exact
terrain and the exact frontier, inspect stationary and moving horizon-only
views across multiple zooms, and fix the shared procedural owner before
judging exact-to-LOD appearance. Preserve V1 as the default, V2 as an
experiment, the fixed clipmap residency budget, and all shared native,
browser, flat, per-eye, and multiview paths.

## Baseline And Problem Separation

The reported browser failure is:

```text
exact terrain generation 1097 has no complete frontier certificate
```

Held keyboard movement currently advances inside horizon encoding, after
World Explorer has already derived and pumped the exact view for that frame.
That permits exact coverage, the requested procedural center, and the
frontier plan to describe two adjacent camera states.

Separately, the existing browser continental smoke at seed `12345`,
`composition=horizon`, and `blocksAcross=8192` reaches all 160 procedural
slots with zero exact chunks yet visibly contains a bright central clipmap
square surrounded by darker procedural rings. Exact composition therefore
does not cause that patch. The candidate source deliberately filters
unrepresentable height frequencies by sample spacing, but presentation must
not turn that resolution choice into a hard material or illumination domain.

These are two defects with one review context, not one seam:

1. retained composed movement must keep exact and procedural frame identity
   coherent and must never treat ordinary warming as a fatal render error;
2. procedural levels must transition between resolutions without a visible
   square in natural color, albedo, geometric shade, water, texture, or forest
   response; and
3. exact-to-procedural appearance remains a later, separately measurable
   transition once item 2 is clean.

## Ownership

- `mclone-worldgen` owns canonical V2 geography, spacing-aware frequency
  filtering, stable material/biome/ecology facts, and cross-spacing source
  invariants.
- `mclone-terrain-view` owns clipmap geometry transitions, level-independent
  appearance semantics, lighting, diagnostics, admission, and immutable frame
  certificates across mono, per-eye, and multiview rendering.
- `mclone-world-explorer` owns only host controls, exact-worker pumping,
  query-gated review instrumentation, and browser/native smoke orchestration.
- `mclone-scene` remains the live-game exact/procedural composition owner; it
  consumes the corrected shared terrain-view contract rather than receiving a
  V2-specific seam workaround.

## Phases And Commit Gates

### Phase 0: Procedural-Only Evidence

- Commit this tactical before implementation.
- Make World Explorer capable of selecting the shared horizon diagnostic in
  native and query-gated browser review paths.
- Capture horizon-only natural, ownership-level, albedo, geometric-shade,
  water, and texture views at walking, regional, and continental zooms.
- Add a deterministic horizon-only movement lane with zero desired, painted,
  or rendered exact chunks.
- Attribute each square to source semantics, geometry filtering, lighting,
  material response, vegetation response, or presentation ownership before
  selecting the correction.

### Phase 1: Coherent Retained Movement

- Advance held input before deriving either the exact view or horizon
  presentation, so one frame has one camera and residency identity.
- Preserve immediate pointer, touch, recenter, and non-held intent behavior.
- Exercise sustained forward/sideways movement, direction reversals, zoom,
  and release through a browser regression that records page and console
  errors.
- A recoverable staged or warming frontier state must retain or suppress
  optional composition coherently; it must not terminate the render loop.
- Commit the independently useful crash fix before visual convergence work.

### Phase 2: Procedural Ring Convergence

- Keep broad geography and typed landforms stable while retaining source-side
  removal of frequencies too small for the display lattice.
- Make adjacent levels agree at their shared boundary and transition any
  legitimately different filtered geometry over a bounded band.
- Derive material, biome, water, forest, texture, illumination, and fog from
  shared world facts or continuous projected-scale inputs, never from a
  categorical clipmap level style.
- Do not hide the defect by disabling fine levels, flattening V2 terrain,
  shrinking the view, changing the seed, or drawing exact terrain over it.
- Add cross-spacing semantic tests and a rendered seam metric that fails on a
  level-aligned characteristic-color step while allowing real geographic
  boundaries.
- Capture and inspect the first corrected horizon-only pixels before
  re-enabling composed acceptance.

### Phase 3: Shared And Composed Acceptance

- Re-run horizon-only stationary and movement captures on native and headed
  WebGPU at multiple V2 journeys and zooms.
- Confirm V1 natural pixels and terrain semantics do not regress.
- Re-run composed exact coverage only after the procedural-only gate passes;
  report exact-to-LOD appearance debt separately from any procedural ring.
- Run focused worldgen, terrain-view, World Explorer, scene, WGSL, and browser
  tests, plus the affected Android build boundary.
- Compare fixed resident bytes, refill work, and settled frame behavior with
  Tactical 329; the correction may not introduce area-proportional work or an
  unbounded transition cache.
- Update living LOD and continental-planning docs, commit, push, deploy the
  exact pushed revision, and inspect the deployed horizon-only and composed
  pixels.

## Acceptance Gates

### Procedural Continuity

- `composition=horizon` owns zero exact chunks and shows no level-aligned
  square or band in natural V2 terrain at the reviewed zooms.
- Ownership diagnostics may reveal the clipmap rings; natural, albedo,
  geometric-shade, water, and texture diagnostics must not reveal a comparable
  characteristic-color step at the same boundaries.
- Adjacent levels share boundary geometry without cracks and converge through
  a bounded transition where their filtered heights legitimately differ.
- Stable V2 material, biome, hydrology, and ecology identities do not change
  merely because a caller requests a different sample spacing.

### Movement Safety

- One frame derives exact, procedural, and frontier state from one advanced
  view identity.
- Sustained held movement and reversals produce no incomplete-frontier fatal
  error, panic, page error, or stale exact island.
- After input release, the requested procedural levels, exact footprint, and
  preferred frontier settle completely and report matching generations.

### Bounds And Platforms

- Clipmap logical/guard slot counts and fixed resident allocations stay
  bounded by the existing preset contract.
- Browser work remains incrementally admitted and native V2 compilation
  remains off the render thread.
- Single-view, per-eye, and full-frame multiview shaders use the same
  transition and appearance contract.
- V1 remains the normal new-world default and V2 remains selectable and
  experimental.

## Human Review G

Stop with two fresh interactive links to the exact deployed revision:

1. a procedural-only V2 view for judging ring continuity without exact
   terrain; and
2. the corresponding composed view for separately judging the exact handoff.

Ask whether any square or characteristic-color band remains while panning and
zooming, and whether sustained movement remains stable. Do not describe the
procedural seam as fixed merely because exact terrain obscures it.
