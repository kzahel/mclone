# Tactical 331: Exact Frontier And Forest Presentation

Status: **implementation and automated/platform acceptance complete as of
2026-08-22; awaiting Human Review H.**

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

## Phase 0 Evidence

A settled native V2 Low/RD2 frame at seed `12345` reports all 25 exact
columns, all 96 horizon slots, and all 16 vegetation products ready. Every one
of its 320 exposed exact segments has spacing-one procedural adjacency. Three
additional preferred support tiles are committed and drawn; no segment uses a
coarse fallback. The visible square is therefore not an absent finest ring or
an incomplete frontier certificate.

The new `appearance-transition` diagnostic renders the procedural field as
grayscale and suppresses proxy/canopy clutter. An inspected elevated capture
shows the intended 32-block white-to-black procedural halo around the exact
footprint. The field is uploaded and sampled in the V2 product path; it simply
does not cover the later forest-summary darkening, exact-side block lighting,
or the topology change. This replaces the initial suspicion of a dead field
with a narrower presentation diagnosis.

The matching headed Chrome app reproduction reaches V2 Low/RD2 with six drawn
levels, 25 exact columns, 16 resident vegetation tiles, 57 visible canopy
tiles, and 14,592 canopy cells. Its natural frame reproduces both the exact
rectangle and long grazing canopy-fan edges. No page, frontier, ownership, or
source-profile error is required to produce either defect.

## Phase 1 Evidence

The V2 product and the detached continental-planning candidate had both been
packed into preview flag `4`. That correctly selected their common
continental height evaluator, but it also made the renderer treat live V2 as
an untextured planning surface. Its final biome IDs were then passed through
the V1 grass-biome remap. Exact V2 chunks consequently used the active
material pack and final biome tint while their spacing-one continuation used
a flat planning color, producing the most conspicuous square in the report.

V2 now has a distinct product flag while retaining the shared continental
evaluator. The product path enables the active material pack and consumes its
final biome IDs directly. The existing 32-block appearance field also fades
coarse forest-summary darkening away at the exact edge, and spacing-one
vertices converge to the same downward block-height convention before
recovering the continuous surface through that field. This remains one smooth
procedural owner; it does not restore the retired voxel shell.

The inspected native V2 Low/RD2 correction has the same 25 exact columns, 320
spacing-one edge segments, three preferred support tiles, and bounded vertex
and residency counts as the diagnostic baseline. Exact and procedural land
now share texture frequency and characteristic green instead of forming a
bright rectangle inside a flat olive mesh. The remaining transition is the
intentional block-to-smooth topology change and ordinary tree representation,
which Phases 2 and 3 address independently.

## Phase 2 Evidence

Vegetation no longer falls through to a terrain presentation while product
tiles arrive individually. A complete cold or replacement level remains the
admission unit, but newly presented world tiles now begin with an independent
proxy/canopy reveal age. Retained semantic tiles preserve their age across a
clipmap shift; a reused physical slot cannot inherit its previous occupant's
age. A `0.65 s` time-based ramp is shared by native and Web clocks, and each
frame contributes at most `0.10 s`, so a suspend or hitch cannot skip the
transition in one update.

The native cold-frame receipt exposes all 16 proxy tiles and 96 canopy tiles
as revealing immediately after the target becomes ready. After 90 ordinary
settle frames, both counts are zero with the same 16 vegetation products, 96
terrain slots, 67 total proxy records, and fixed canopy budget. Inspected
first and settled pixels show the outer proxy population moving from low
opacity to full opacity instead of appearing as an opaque band.

## Phase 3 Evidence

The proxy/canopy complement now combines projected scale with eye altitude
relative to each represented crown. From six through 32 blocks above a crown,
the aerial canopy becomes smoothly eligible. Below that range a grazing X/Z
derivative cannot promote the canopy on its own, and individual proxies retain
the complementary weight. Mono and multiview use the same function, selecting
the physical camera for each eye.

An inspected settled ground capture inside the exact footprint shows ordinary
exact trees transitioning to procedural proxy crowns without the former long
translucent canopy edges or fuzzy sky halo. Elevated pixels retain the fixed
canopy population and broad forest-summary tint. This changes no canopy cells,
vertices, buffers, clipmap slots, tree records, or ownership rules.

## Phase 4 Movement Safety Finding

The first physical Quest RD8 acceptance attempt found a separate real crash
before a performance sample could begin. Exact coverage generation `263`
contained 263 of the requested 289 chunks. That ragged, still-filling
footprint temporarily needed more spacing-one frontier support than the fixed
pool could certify, while all procedural clipmap refills happened to be
settled. The renderer's old warming test considered only procedural work and
therefore terminated the session with `no complete frontier certificate`.

The live scene now supplies the missing typed fact: whether the current exact
coverage equals its complete topology-aware requested chunk view. While exact
coverage is still stabilizing, an uncertified generation suppresses optional
procedural composition and remains not-ready instead of becoming fatal. Once
the requested exact view is complete, an uncertified frontier is still an
error; this does not weaken settled certificate validation or enlarge the
support pool.

A native V2 Low/RD8 replay advances safely through generation `265` and all
289 exact chunks. Its complete topology selects 20 preferred spacing-one
support tiles within the existing capacity of 32 and prepares them under the
bounded dispatch budget. The corresponding physical Quest acceptance is
repeated after rebuilding the release APK below.

The initial stationary Quest comparison also exposed avoidable canopy cost in
the spacing-one ring. That ring already draws the detailed proxy
representation and broad forest tint, so drawing the coarse aerial canopy
there duplicated representation work nearest the observer. Canopy fans now
begin at spacing two. At V2 Low/RD8 this reduces the settled fixed canopy from
49 tiles / 12,544 cells to 33 tiles / 8,448 cells. The matched physical V2
stationary sample improves from `13.092/14.161 ms` app-work p50/p95 with 10.3%
over-period frames to `12.242/13.202 ms` and 0.0%. Its `6.586 ms` app GPU time
is 6.7% above the paired V1 control's `6.172 ms`, while app-work p95 is 5.6%
above V1's `12.497 ms`; both stay within the accepted 10% V2 envelope.

The first 12-block/second retained-motion replay was safe but exposed a much
larger V2-only render-thread tail: app-work reached about `99.249 ms` p95,
42.0% of frames exceeded the 72 Hz period, and only 27.4 frames/second were
submitted. Focused phase tracing showed that preferred spacing-one frontier
support tiles were compiling the continental CPU reference grid synchronously
inside each eye's encode. Deferring transient pool allocation alone did not
remove that cost.

Preferred V2 frontier tiles now use the existing bounded native horizon CPU
compiler. Each completion carries source, exact generation, presentation, and
fine-tile identities; a retired result is counted stale and cannot enter a
replacement support pool. The already-complete zero-capacity fallback remains
active while preferred tiles warm, and frontier jobs do not masquerade as
ordinary clipmap refills or block their dispatch.

The rebuilt physical Quest 3 V2 Low/RD8 replay starts with all 289 exact
columns and 96 horizon slots ready, moves `30.973` blocks over `30.014 s` at
12 blocks/second, and exits normally. It sustains 71.57 submitted frames per
second with app-work `10.890/13.566/15.857 ms` p50/p95/p99, 3.4% over-period
frames, and no 2x-budget worst frame. The paired unchanged V1 control measured
`12.609/16.738 ms` p50/p95 and 15.1% over-period frames. This removes the
V2-specific roughly 100 ms movement tail without enlarging the support pool or
weakening exact-frontier certification.

Native `mclone-terrain-view` passes 157 tests with one adapter-only test
ignored. The shared browser client compiles for `wasm32-unknown-unknown`, the
flat Android APK builds, and the Android XR release APK builds. The physical
XR replay used the already-staged headset assets because the unrelated legacy
`mclone-game-1.17.1` asset lock is stale; first-party pack generation and both
Android code builds succeed.

## Human Review H

Stop with one deployed V2 world at a broad flat forest/clearing and provide
two reproducible views: ground-level forward movement and elevated flight
across the same exact boundary. Ask whether the exact square, mass tree pop,
or canopy halo remains distracting. Keep V2 experimental and V1 default
regardless of acceptance.

Extreme high-aerial views can still expose faceted or crisscross edges in the
fixed canopy carpet. The ground halo is removed and the broad forest no longer
vanishes, but replacing that farthest forest-mass representation is a distinct
follow-up rather than a reason to reintroduce near spacing-one canopy cost.
