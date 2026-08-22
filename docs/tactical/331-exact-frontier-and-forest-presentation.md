# Tactical 331: Exact Frontier And Forest Presentation

Status: **active as of 2026-08-22. Baseline diagnosis is complete; pixel and
motion corrections are in progress.**

Topic: `procedural-horizon-clipmap`
Topic: `v2-forest-continuity-and-performance`

## Instruction Synthesis

Mclone Overworld V2 now exposes three composition defects clearly in the live
game, especially at low exact render distance and across its broad, relatively
flat forests:

1. the exact block footprint reads as a hard rectangular island against the
   smooth procedural horizon;
2. moving or waiting for streaming can make a large band of procedural trees
   appear in one conspicuous batch; and
3. a ground-level grazing view can select the coarse canopy carpet close
   enough that its translucent triangulated sheets read as a fuzzy halo.

Correct these in the shared procedural-horizon owner. Preserve V1 as the
default and V2 as selectable, keep one exact/procedural terrain owner, retain
bounded clipmap and frontier resources, and use the same mono, per-eye, and
full-frame multiview contracts. Commit and validate the work incrementally,
then push, deploy the exact revision, and provide an interactive review link.

## Baseline Diagnosis

The user's four live mobile captures separate the failures:

- the exact boundary changes terrain topology, albedo, texture frequency, and
  tree representation on the same axis-aligned line;
- the `10:59:02` and `10:59:10` captures show a distant forest band changing
  from sparse to dense without a comparable geographic or camera change; and
- the ground capture exposes long canopy-fan edges and overlapping sheets
  against the sky behind ordinary exact trees.

The remembered near-field "beard" existed historically as the spacing-one
procedural voxel shell. Tactical 313 deliberately deleted it after direct
exact-to-smooth review because it introduced a second conspicuous topology
handoff and about 40% more worst-case terrain vertex work. The current system
does still preserve full-resolution support: clipmap level zero samples every
block, and Tactical 321 adds a bounded spacing-one support belt wherever the
exact perimeter escapes the regular finest ring. Restoring the retired voxel
shell is therefore not the default remedy for this regression.

The current exact-side certificate guarantees closure, resolution, source
identity, and generation coherence. It does not guarantee perceptual
continuity. The 32-block appearance field modifies only the procedural
surface, while exact chunks retain ordinary block materials, discrete top and
side geometry, exact lighting, and exact vegetation. V2's flat forest makes
all of those changes visible on one rectangle even when the spacing-one belt
is present and watertight.

Vegetation admission has two pop paths. During cold fill,
`vegetation_presentations` falls back to the committed terrain presentation,
so individual ready buffers become visible as they arrive. During retained
movement, it preserves the old complete vegetation level and atomically swaps
to the replacement after every tile is ready. Both are bounded and ownership
safe, but neither gives a newly drawable tile or replacement strip a
presentation-age transition.

The V2 proxy/canopy cross-fade uses only fragment derivatives in world X/Z.
Those derivatives correctly describe projected scale from above, but grow at
a ground-level grazing angle. The shader consequently removes individual
proxies and raises the canopy carpet precisely where its elevated fan geometry
is most legible edge-on.

## Objective

Produce a live V2 composition in which:

- the exact footprint still meets one smooth spacing-one procedural owner, but
  does not read as a hard color, lighting, height, or texture rectangle;
- low render distance does not weaken the spacing-one collar or expose a
  coarser direct neighbor;
- cold and retained forest admission converges through a bounded smooth reveal
  rather than an all-at-once tile or strip pop;
- ground-level views do not select aerial canopy sheets at grazing angles;
- elevated and continental views retain continuous authored forest mass;
- tree and terrain ownership remain exact, deterministic, and duplicate-free;
  and
- Quest Low/RD8 cost remains within the accepted V2-relative performance
  envelope unless a measured, explicitly reviewed tradeoff changes it.

## Ownership And Constraints

- `mclone-worldgen` owns exact/spacing-one height, substrate, water, biome, and
  forest semantic agreement. A correction belongs there only if the same
  world coordinate disagrees before presentation.
- `mclone-terrain-view` owns exact-frontier appearance, spacing-one support,
  vegetation reveal, canopy view suitability, diagnostics, and every render
  path.
- `mclone-scene` owns the focus-connected ready exact component and one
  composition generation. Apps remain hosts and receive no V2 seam branch.
- Do not restore the retired voxel shell, add a coincident terrain surface,
  enlarge exact render distance to hide the line, use fog as the seam owner,
  or make forest work proportional to continent area.
- Any temporal reveal is presentation-only. It may delay opacity, but it may
  not delay authoritative ownership, retain stale tree identities, or vary
  worldgen output.

## Phases And Commit Gates

### Phase 0: Reproduction And Instrumentation

- Add a direct diagnostic for the exact appearance field and expose the
  spacing of the procedural terrain adjoining every visible exact edge.
- Add a deterministic forest-admission trace recording newly drawable proxy
  and canopy tiles per frame, including cold fill and one retained strip move.
- Capture V1 and V2 at a flat/clearing site with exact radius two from ground
  and elevated cameras. Separate natural, albedo, geometry, texture,
  appearance-weight, proxy, and canopy evidence.
- Fail if a settled radius-two exact edge lacks spacing-one procedural support;
  do not infer support merely from allocation.

### Phase 1: Exact-To-Spacing-One Perceptual Continuity

- Verify spacing-one V2 height quantization and visible material at the exact
  boundary against authoritative exact columns.
- Correct any source mismatch at the shared source. Otherwise converge the
  surviving exact and smooth render semantics through the existing transition
  contract without creating another geometry owner.
- Preserve direct smooth geometry and the complete frontier certificate.
- Add a rendered boundary metric that compares narrow inside/outside bands and
  rejects an axis-aligned characteristic step while allowing real geographic
  edges.
- Capture and inspect the first corrected low-distance natural pixels before
  proceeding.

### Phase 2: Stable Forest Admission

- Give newly drawable proxy and canopy resources a bounded presentation-age
  receipt independent of clipmap slot reuse.
- Cross-fade cold products and entering movement strips without fading retained
  stable world tiles or changing exact/proxy XOR ownership.
- Keep the reveal time-based and smooth under 30, 60, 72, 90, and 120 Hz frame
  cadence. Clamp long pauses so resume cannot skip the transition in one frame.
- Expose revealing tile/instance counts and reject a resource generation that
  inherits the prior occupant's reveal age.

### Phase 3: Ground/Aerial Canopy Suitability

- Combine projected scale with a smooth view-suitability term derived from the
  physical eye relative to canopy height. A grazing camera below the crowns
  must not promote the aerial carpet merely because X/Z derivatives diverge.
- Preserve aerial canopy continuity and the broadest terrain forest tint.
- Use one rule in mono, per-eye, and multiview; each eye evaluates its own
  camera facts.
- Inspect ground, low-flight, high-flight, and continental forest frames.

### Phase 4: Acceptance And Deployment

- Run focused worldgen, terrain-view, scene, WGSL, native capture, and headed
  WebGPU movement tests.
- Build the affected flat Android and Quest boundaries through their scripts.
- Re-run the physical Quest V1/V2 Low/RD8 stationary and retained-motion gate
  if the final shader or submitted vegetation work changes materially.
- Update the living LOD and continental-planning topics with the new contract,
  measured costs, and remaining exact water or volumetric limitations.
- Push and deploy the exact accepted revision, then inspect both the deployed
  ground view and an elevated V2 forest view before sharing the URL.

## Acceptance

- Every visible exact edge at radius two reports spacing-one support and the
  natural image has no conspicuous square representation boundary.
- No cold or retained frame introduces a complete forest tile/strip at full
  opacity. Slot reuse begins a fresh bounded reveal.
- A ground camera below the crown layer draws no visible coarse canopy fan
  edges or fuzzy elevated halo in the reviewed near/middle field.
- An elevated camera still sees continuous forest mass beyond individual
  proxy trees, with no empty ring.
- Exact/proxy ownership diagnostics remain zero missing and zero dual-owned.
- Clipmap slots, support capacity, canopy cells, tree records, resident bytes,
  and worker bounds do not grow.
- V1 pixels do not acquire V2 canopy or a V2-specific frontier treatment.
- Native, WebGPU, flat Android, per-eye XR, and multiview share the corrected
  shader and admission contracts.

## Human Review H

Stop with one deployed V2 world at a broad flat forest/clearing and provide
two reproducible views: ground-level forward movement and elevated flight
across the same exact boundary. Ask whether the exact square, mass tree pop,
or canopy halo remains distracting. Keep V2 experimental and V1 default
regardless of acceptance.

