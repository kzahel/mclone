# Tactical 309: Procedural Horizon Lighting and Seam Convergence

Status: Phase 0 and Phase 1 accepted; Phase 2 correction implemented and
awaiting renewed Human Review 2 after the first review rejected an open
voxel-to-smooth geometry crack.

Topic: `procedural-horizon-clipmap`

Topic: `procedural-horizon-surface-appearance`

## Instruction Synthesis

Correct the most conspicuous remaining composed-terrain defect first: exact
terrain darkens with time of day while substantial portions of procedural
land, water, and vegetation retain daytime illumination. Then continue as one
focused visual-convergence campaign across both representation borders:

1. exact block terrain to the spacing-one blocky procedural shell; and
2. the spacing-one blocky shell to the coarser smooth procedural mesh.

Treat these as related but separately diagnosable borders. Do not hide them by
increasing exact radius, fading two opaque surfaces, or tuning one screenshot.
Each implementation phase ends with a stable review packet and must stop for
Human Review before the next phase begins. A rejected phase is corrected and
recaptured in place.

Tactical 304 remains the structural foundation: it established one solid
horizontal owner, the bounded exact-frontier curtain, the spacing-one
top-and-riser shell, worldgen-owned side strata, and active-pack near
materials. This tactical owns the unresolved visual-acceptance gate rather
than reopening the clipmap residency or terrain-ownership architecture.

## Observed Baseline

The initial live-scene review uses seed `12345`, local
`mclone-overworld-v1`, render distance `2`, Composed terrain, vanilla color,
ordinary lighting, frozen time, and no passive showcase. Two deterministic
cameras expose different parts of the problem:

| View | Eye | Target | Purpose |
|---|---|---|---|
| elevated | `8,105,8` | `8,65,-300` | Broad voxel/smooth rings, coast, water, and proxy vegetation |
| low | `8,82,8` | `8,67,-180` | Exact foreground, block shell, smooth hills, water, and trees in one frame |

The first review set captured dawn `0`, noon `6000`, dusk `12000`, and
midnight `18000`, plus an Exact Only midnight control. It establishes:

- exact foreground terrain responds strongly to the night lightmap;
- large procedural land regions remain near their daylight color at midnight;
- procedural water remains bright cyan at midnight;
- procedural tree proxies retain nearly daytime trunk and crown colors;
- the blocky-to-smooth change is visible in land light and water color even at
  noon; and
- the Exact Only control distinguishes the exact footprint from the dark and
  bright regions inside the procedural presentation.

The current shader shape explains the observation. The spacing-one shell
forces top geometric light to `1.0`, applies exact-style cardinal factors to
risers, and applies the shared full-sky lightmap only inside its near-material
branch. Smooth levels instead multiply material color by a fixed
slope-direction term clamped to `0.34..1.05`. The near-material weight fades
toward that far path. Sampled water and analytic water do not consistently
consume the near lightmap. Procedural trees use fixed family and height colors
plus fog, with no environmental-light term.

Ambient occlusion cannot explain that global day/night failure. It remains a
probable contributor at the exact/voxel boundary and must be measured later,
after environmental illumination is coherent.

## Objective

Produce a composed natural-terrain presentation in which:

- time of day affects exact terrain, every procedural terrain level, sampled
  and analytic water, and procedural vegetation through one shared
  environmental-light contract;
- representation topology controls only geometric shade, not whether
  daylight applies;
- the voxel/smooth boundary no longer appears as a stable annulus in land,
  water, texture detail, or light;
- the exact/voxel boundary is visually coherent under comparable full-sky
  natural-terrain conditions, with residual stored-light and AO differences
  explicitly identified rather than hidden;
- movement and clipmap rebases do not turn a spatial transition into shimmer
  or a moving square; and
- all results remain shared across mono, per-eye stereo, and full-frame
  multiview without increasing exact residency or forking platform policy.

Perfect equality is not required where the representations carry genuinely
different facts. Exact chunks know stored block light, stored sky light,
canonical neighbors, and model AO. The natural procedural horizon may assume
sky light `15` and block light `0` until a separately justified compact source
exists. The goal is a coherent approximation whose remaining differences are
semantic and visible in diagnostics, not accidental shader branches.

## Binding Decisions

### Human Review is a phase gate

This tactical is intentionally interactive. Each numbered implementation
phase produces the declared matched captures, records its automated evidence,
and stops. Do not begin the next phase until Human Review explicitly accepts
the current one.

Every review packet must keep seed, camera, exact radius, asset pack, color
profile, lighting mode, time, and output extent in its receipt or command
record. Do not compare images whose asynchronous vegetation, camera, or source
state differs. Rejected evidence is replaced, not supplemented by a more
favorable unrelated scene.

### Separate material, environment, geometry, and occlusion

Refactor the procedural appearance model conceptually as:

```text
active-pack/biome/water albedo
        x
shared environmental illumination
        x
representation-specific geometric shade
        x
bounded local occlusion
        -> fog -> target color transfer
```

Do not let a material-transition weight enable or disable time of day. Do not
bake slope light into base albedo. Do not use AO as a replacement for
environmental illumination. Diagnostic modes must be able to show at least
albedo, environmental illumination, geometric shade, and local occlusion
separately.

The exact renderer remains the reference for the shared clear-weather
lightmap curve. Procedural natural surfaces use the same scene `sky_darken`
input with full sky and zero block light. This tactical does not add distant
torch light, cave light, night vision, weather, or a shadow map.

### Fix time of day across all procedural surface classes first

The first implementation phase must apply environmental illumination to:

- spacing-one top faces and risers;
- smooth terrain at every coarser level;
- sampled ocean/river/wetland water;
- analytic river and pool overlays; and
- procedural tree trunks and crowns.

The shared environmental term must be constant across the voxel/smooth
transition for a given frame. Geometric face or slope shade may still differ.
Water may use its own material response, but it may not remain daytime-bright
at night. Tree proxies may retain their simplified family colors, but those
colors must participate in the same full-sky day/night response as natural
exact foliage.

Use the existing clear-weather time/lightmap contract. Do not pull Tactical
307's seasonal solar path, rotating orbital policy, latitude, or day-length
controls into this work. Vanilla-style fixed face shades are compatible with
a time-varying lightmap.

### Treat the voxel/smooth border as a parameter transition

The spacing-one shell and the coarser smooth mesh keep their current
single-owner topology and connector. Do not crossfade two opaque geometries.
Within the owning presentation, make the following inputs continuous or
deliberately compatible across the transition:

- environmental illumination;
- top-face versus slope-derived geometric shade;
- surface normal bandwidth;
- active-pack texture strength, derivatives, and mip selection;
- biome/material footprint selection;
- sampled and analytic water albedo, depth response, and texture weight; and
- procedural vegetation light response.

A spatial blend may evaluate both local shade models on one owning surface and
blend their parameters. It must be stable in absolute world space or committed
clipmap coordinates and must not breathe with camera sub-cell motion.

The accepted result may retain a visible intentional change from blocky tops
to a smooth silhouette. It may not add a square or annular change in daylight,
hue, saturation, or texture contrast on top of that geometric change.

### Treat the exact/voxel border as a semantic comparison

Use controlled full-sky natural-terrain fixtures before judging real scenes.
Compare:

- upward and cardinal active-pack sprites;
- biome tint and its neighborhood/footprint approximation;
- texture scale, filtering, mip choice, and output transfer;
- shared lightmap response and ordinary face shade;
- exact model AO versus any bounded height-neighborhood LOD occlusion;
- exact stored sky/block light versus the declared procedural assumption;
- the bounded frontier curtain's material and shade; and
- exact/proxy vegetation lighting on both sides of ownership arbitration.

Do not globally brighten or darken the procedural shell to match one exact
edge. First use diagnostics to determine whether a residual is albedo, light,
AO, texture filtering, fog, or unavailable semantic data.

A bounded height-neighborhood AO approximation is allowed only after the
unoccluded comparison agrees and only if it improves multiple terrain shapes.
It must be stable at tile edges and negative coordinates. Canonical block AO,
local artificial light, caves, overhangs, and edited structures are not
promised by the procedural heightfield.

### Keep water explicit

Tactical 304's single-visible-water-owner rule remains in force. This tactical
must normalize the visible procedural owner's illumination and color across
both borders. It must not restore coincident exact/procedural water surfaces.

Use one topology-independent environmental response for open water and one
declared, consistently filtered depth or bed-height input. Apply the same rule
to sampled water and analytic river/pool overlays. If ground-following water
geometry itself remains visibly unacceptable after shading converges, record
flat water geometry as a separate follow-up instead of expanding this
tactical into a hydrology rewrite.

### Diagnose before tuning

Add narrowly scoped diagnostic views or capture switches for:

- exact ownership and procedural clipmap level;
- voxel versus smooth topology;
- material/biome albedo before lighting;
- environmental illumination;
- geometric shade or normal;
- local occlusion/AO contribution;
- water classification and depth input; and
- texture contribution or mip/footprint selection.

Diagnostics may be developer-only, but they must use the same committed frame
facts as the rendered image. A screenshot plus a guessed explanation is not an
accepted diagnosis.

### Preserve fixed ownership and bounded cost

Do not change the ten-level, four-by-four toroidal clipmap, increase exact
radius, revive chunk-based Far LOD, or add a camera-sized painted texture.
Avoid new per-frame allocations and per-view terrain products. Any new
uniform, texture, sample field, fragment branch, or AO neighborhood work must
record fixed bytes and measured frame impact.

All shader changes must compile from one shared source for mono,
stereo/per-eye, and full-frame multiview. Exact-only must remain
horizon-allocation-free in the live game. World Explorer may retain its
existing switch-ready horizon.

## Phase 0: Reproducible Diagnostics and Review Contract

Status: implemented; Human Review 0 accepted 2026-08-15.

- Turn the elevated and low cameras above into reproducible capture commands
  or a small shared capture scenario, not hand-positioned screenshots.
- Capture Composed at ticks `0`, `6000`, `12000`, and `18000` and Exact Only
  controls at noon and midnight.
- Add a second seed and focused coast, forest, exposed-stone, and snow views so
  seed `12345` is not the only tuning target.
- Add the diagnostic channels listed above and record which shader term causes
  each visible border.
- Ensure all images report a settled, identical source/camera/vegetation state.

### Phase 0 execution record

Commits `34fd2e1f`, `3830479b`, `04ecde69`, `e5bcc456`, and
`94367622` implement the diagnostic and capture contract without changing
natural appearance. The shared `mclone-terrain-view` presentation now has
developer-only `ownership-level`, `topology`, `albedo`,
`environmental-illumination`, `geometric-shade`, `local-occlusion`, `water`,
and `texture` channels. One previously reserved uniform word carries the
selector, so the uniform size, bind groups, textures, sample records, and
fixed clipmap allocation are unchanged. The ordinary `natural` selector is
the default. Extra albedo atlas samples execute only in the albedo diagnostic;
the settled natural baseline remained byte-identical after that gate at SHA-256
`4c6c7f010b0a66d2489555d8852dd7e9f96ed466b809d6784c43c94474460391`.

The reproducible command is:

```bash
pnpm native:terrain-seam-review:capture -- \
  --output /tmp/mclone-terrain-seam-review-hr0-review
```

The schema-one receipt is
`/tmp/mclone-terrain-seam-review-hr0-review/receipt.json`. It records every
command, seed, chunk interest, camera, tick, terrain presentation, diagnostic,
render option, settle policy, PNG extent/size/hash, and observed terrain state.
The 26 1280-by-720 captures comprise:

- both accepted baseline cameras at ticks `0`, `6000`, `12000`, and `18000`;
- both cameras in Exact Only at noon and midnight;
- noon/midnight forest, exposed-stone, and snow views, with the elevated
  baseline also serving as the focused coast view;
- seed `12345` plus exposed-stone seed `-98765`; and
- all eight procedural diagnostic channels in the accepted low view.

The first strict campaign run exposed a false-ready forest capture with only
18 exact columns. The landed runner therefore requires the complete RD2
5-by-5 exact footprint and uses 180 paced post-settle frames. Every accepted
composed image reports 25 exact columns, 160 ready clipmap slots, target-ready
terrain, zero pending vegetation tiles, equal submitted/completed vegetation
jobs, and zero vegetation transport/job failures. Every comparison group has
an identical observed terrain-state object. Exact Only reports the terrain
horizon disabled, preserving its allocation boundary.

### Attributed problem inventory

| Visible discontinuity | Diagnostic attribution | Phase owning correction |
|---|---|---|
| Exact terrain darkens at midnight while distant land stays green | The midnight environmental channel is dark in the exact foreground and the weighted near-material band, but identity-white over the smooth procedural surface. Environmental illumination is conditionally attached to near material blending instead of being a topology-independent term. | Phase 1 |
| Procedural water remains bright cyan at midnight | The water channel confirms the sampled and analytic water owners, while the environmental channel shows that those visible water branches consume the identity multiplier. This is environmental response, not water ownership or depth classification. | Phase 1 |
| Proxy trees retain daytime color | The topology channel identifies proxy vegetation in pink. Its environmental channel is identity-white, while exact trees remain naturally dark. Proxy family color currently has geometric height shade and fog but no scene lightmap term. | Phase 1 |
| Voxel-to-smooth land forms a stable color/light ring | Topology cleanly locates green voxel tops/amber risers against blue smooth terrain. Geometric shade changes from fixed `1.0/0.6/0.8` faces to slope light, the texture channel changes exact-strength near material to footprint-reduced detail, and the albedo channel retains a biome/material color shift. These are separate contributors in addition to the night failure. | Phase 2 |
| Water character changes across procedural levels | The water channel shows continuous sampled/analytic classification, but the texture channel changes footprint response and the natural frame changes shade/color at the same topology boundary. Classification is not the primary current defect; environmental, texture, and geometry responses are. | Phases 1 and 2 |
| Exact-to-voxel frontier remains visible at noon | Ownership/level coloring confirms one exact owner and one spacing-one procedural owner rather than coincident solid surfaces. The procedural albedo, texture, and fixed face-shade terms differ from the exact foreground. Exact stored light and model AO remain additional semantic differences for controlled Phase 3 comparison. | Phase 3 |
| AO may contribute at the exact frontier | The procedural local-occlusion channel is identity-white for terrain and proxies. It therefore cannot explain the global day/night or voxel/smooth failures, but its absence can contribute next to exact model AO. No AO approximation is justified before the unoccluded terms converge. | Phase 3, only if multi-scene evidence warrants it |

The focused coast, forest, exposed-stone, and snow images were visually
inspected. They retain the intended exact/voxel/smooth material and vegetation
contrasts and reproduce the nighttime problem outside the original low view.
The diagnostic maps intentionally leave exact pixels natural and suppress fog
only on procedural diagnostic pixels so ownership and individual terms remain
legible; natural comparison frames retain ordinary fog and target transfer.

Automated evidence passes:

- `cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view --lib`
  (`96` passed, `1` GPU-only test ignored);
- the native screenshot CLI parsing tests for diagnostic and settle controls;
- shared Naga validation for single-view and generated multiview terrain/tree
  shaders; and
- the 26-capture campaign's PNG, complete-coverage, exact-only, vegetation,
  and within-group state gates.

Gate — Human Review 0: accept the cameras, scenarios, and attributed problem
inventory as the fixed comparison set. No appearance implementation begins
before this gate.

Accepted by the user on 2026-08-15.

## Phase 1: Shared Environmental Illumination

Status: implemented; Human Review 1 accepted 2026-08-16.

- Establish one renderer-neutral full-sky/zero-block-light environmental term
  from the scene time-of-day input.
- Apply it to every terrain level, both water paths, and procedural trees.
- Ensure near/far material weighting cannot remove or duplicate the term.
- Keep geometric face/slope shade separate and unchanged in this phase except
  where separation is necessary to remove double multiplication.
- Add focused WGSL/source tests and mono/multiview shader validation.
- Capture all Phase 0 scenes at the four frozen times.

### Phase 1 execution record

Commits `7608a317`, `028ccd94`, `787a7064`, and `c7edb61b` implement
and verify this phase. The CPU already computed the exact renderer's
full-sky/zero-block-light RGB through
`mclone_render::light_texture::lightmap_color(0, 15, sky_darken)` and packed
it in the shared terrain/tree uniform. The terrain shader now consumes that
value exactly once, after material texture, sampled and analytic water, cover,
and geometric shade are composed. Near-material weight and topology no longer
control environmental light. The tree shader multiplies the same value into
proxy family albedo and geometric height shade. The environmental diagnostic
now reports that one uniform term over every procedural surface class.

Face and slope shade are otherwise byte-for-byte unchanged: voxel tops and
risers retain `1.0/0.6/0.8`, smooth terrain retains its slope term, and tree
proxies retain their height shade. The change adds no uniform bytes, sample
fields, textures, bind groups, fixed allocations, per-view products, or
fragment branches. It adds one topology-independent RGB multiply to natural
procedural terrain/water and one to proxy vegetation; the removed near-only
multiply prevents double application.

The accepted command was:

```bash
pnpm native:terrain-seam-review:capture -- \
  --output /tmp/mclone-terrain-seam-review-hr1-final \
  --review-phase 1 \
  --settle-frames 300 \
  --skip-build
```

The schema-one receipt is
`/tmp/mclone-terrain-seam-review-hr1-final/receipt.json` and records revision
`c7edb61bdd8c8e554ae67d117042f9d4dee22837`. Its 32 1280-by-720 images
contain all four frozen times for the elevated/coast, low, forest,
exposed-stone, and snow scenes; four noon/midnight Exact Only controls; and
all eight surface diagnostics. It covers seeds `12345` and `-98765`.

Every composed image reports 25 exact columns, 160 ready clipmap slots,
target-ready terrain, zero pending vegetation, balanced submitted/completed
vegetation work, and zero transport or job failures. Settled presentation
facts are identical inside every comparison group. Fresh processes may use a
different exact-coverage upload generation or complete a different balanced
total of redundant vegetation jobs; both raw counters remain in the receipt
but are correctly excluded from visible-state identity. Exact Only reports
the horizon disabled.

The complete natural time grid, Exact Only controls, and diagnostic grid were
visually inspected, followed by the full-resolution forest, exposed-stone,
snow, low, and elevated checkpoints. A direct Phase 0/Phase 1 midnight
comparison shows the former daytime-green smooth land, bright-cyan water, and
bright proxy crowns replaced by the same dark, blue-biased night-lightmap
response seen at the exact foreground. Noon remains normally illuminated; it
now consistently receives the exact lightmap's slight sub-white clear-day
multiplier rather than identity white. The environmental diagnostic is one
constant RGB field across voxel land, smooth land, both water paths, and proxy
trees for the frame.

The review intentionally does not accept the still-visible voxel/smooth
material, texture, and geometric-shade ring or the exact/voxel frontier.
Those remain attributed Phase 2 and Phase 3 work. Blue night water and snow
are expected products of the shared exact lightmap tint, not retained daytime
brightness.

Automated evidence passes:

- `cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view --lib`
  (`97` passed, `1` GPU-only test ignored), including shared Naga validation
  for generated mono and multiview terrain/tree shaders;
- source-contract tests that require one topology-independent environment
  term in terrain, water, and proxy-tree composition;
- `node --check scripts/capture-terrain-seam-review.mjs`; and
- the 32-capture campaign's PNG, complete-coverage, exact-only, vegetation,
  and within-group settled-presentation gates.

Browser correction on 2026-08-15: the diagnostic shaders initially declared
a local named `diagnostic`. Native Naga accepted it, but Chrome reserves that
WGSL directive word and rejected both the terrain and proxy-tree modules,
leaving Terrain Lab runtime composition and the live Web Composed setting
black. The local is now `horizon_diagnostic`. A browser reserved-word lint
over every generated Terrain View shader variant rejects all three directive
words that Chromium reserves but Naga 25 accepts as identifiers. Terrain Lab
also captures uncaught WebGPU errors and validation-scopes runtime pipeline
construction so this failure class cannot report a ready black frame. Focused
Rust coverage, inspected desktop and Pixel 7 Terrain Lab runtime captures,
and the phone full-game Graphics toggle plus persisted reload smoke pass. The
game captures draw all ten levels and contain 37,750 and 38,735 distinct
interior colors.

Gate — Human Review 1: exact, voxel, smooth, water, and tree presentations
belong to the same time of day. Midnight contains no daytime-green horizon or
bright-cyan procedural water, while noon is not globally over-darkened.

Accepted by the user on 2026-08-16.

## Phase 2: Voxel-to-Smooth Procedural Convergence

Status: Human Review 2 rejected the first implementation; correction
implemented and awaiting renewed Human Review 2.

- Reconcile top-face and slope-derived geometric shade over a stable bounded
  transition without blending geometry owners.
- Compare normal bandwidth and the current fine/coarse connector under level
  diagnostics.
- Make material footprint, biome tint, texture strength, derivatives, and mip
  behavior continuous enough that the topology change does not create a color
  ring.
- Normalize sampled and analytic water depth/color/texture treatment across
  the boundary.
- Inspect procedural tree lighting and density presentation at the same ring.
- Exercise stationary, slow movement, orbit, rebase, and teleport captures.

### Phase 2 execution record

Commits `f533c1e0`, `2b8e2812`, `281a2121`, `6db9677b`,
`7de18bd5`, and `a519552b` implement and verify the first review packet. The
spacing-one voxel shell keeps sole ownership of its flat tops and cardinal
risers, but its fixed top/side shade now approaches the already-computed
slope shade over the existing 32-cell committed outer-footprint band. The
connector's bounded two-cell coarse normal footprint and fine/coarse geometry
weld remain unchanged; there is no second geometry owner or height crossfade.

Textured grass on both procedural representations now resolves biome color
from the same active-pack tint table. Untinted far materials retain their
low-frequency fallback as atlas detail recedes. Material texture treatment now
takes a continuous exact-style weight rather than a topology boolean. Land
uses the same existing transition as before, while analytic river water now
uses that weight instead of switching texture strength at the voxel/smooth
bit. Sampled water color/depth and water classification were already shared
and remain unchanged.

The forest topology, albedo, and geometric-shade diagnostics do not show a
separate proxy brightness or density annulus. Phase 1 already gave every proxy
the shared environmental response, and the whole-record vegetation admission
contract remains stable at the level boundary, so this phase deliberately adds
no tree-only fade, duplicate record, or density branch.

The accepted command was:

```bash
pnpm native:terrain-seam-review:capture -- \
  --output /tmp/mclone-terrain-seam-review-hr2 \
  --review-phase 2 \
  --settle-frames 300 \
  --skip-build
```

The schema-one receipt is
`/tmp/mclone-terrain-seam-review-hr2/receipt.json` and records revision
`a519552b13626a6ed2be976a1b579ec01ceeefa7`. Its 48 1280-by-720
captures include the complete Phase 1 time/scene/control grid, all eight low
diagnostics, focused voxel/smooth diagnostics over the elevated coast, forest,
exposed-stone, and snow scenes, and stationary, sub-cell, spacing-one-tile
rebase, second-orbit-angle, and far-teleport endpoints.

One rejected campaign attempt exposed the known fresh-process
`view-settled` race at 18 rather than 25 exact columns. The runner now rejects
and replaces an incomplete process up to a hard limit of three, records the
accepted attempt, and still fails on exhaustion. The final packet needed no
retry: all 48 accepted captures passed on attempt one. Every composed capture
has all 25 RD2 exact columns, all 160 clipmap slots, target-ready terrain,
drained failure-free vegetation, and identical settled facts inside each
comparison group. Exact Only continues to report no horizon allocation.

The complete natural, diagnostic, material/tree, and stability sheets were
visually inspected at full resolution. The topology change remains legible as
the intended blocky-to-smooth silhouette. It no longer adds a stable annulus
in land hue, geometric shade, water tint/classification, texture contrast, or
proxy brightness. The texture diagnostic shows a broad monotonic committed
transition instead of a topology-bit switch; the geometric diagnostic does
not show a square shade ring; and water classification is continuous through
the same terrain.

A clean headed traversal at the packet revision moved the low camera at four
blocks per second for 18 seconds, from X8 to X80, crossing a spacing-one tile
boundary while reconciling authoritative interest from chunk X0 to X4. All
2,155 frames presented with no surface skip or reconfigure. The final runtime
had 49/49 target chunks, zero pending jobs, publications, or unloads, and
terrain GPU duration at 0.132 ms median, 0.218 ms P95, and 1.153 ms P99. The
report is `/tmp/mclone-terrain-seam-phase2-slow-traversal-final.json`.
Existing focused clipmap tests also cover sub-cell retention, one-cell shifts,
long walks, and whole-level teleport rebases.

The implementation adds no uniform bytes, sample fields, additional atlas
samples, textures, bind groups, fixed allocations, per-view products, or
geometry. It adds a tint-table lookup for textured grass where the smooth path
formerly used its hard-coded approximation, and replaces boolean texture
selects with bounded scalar mixes. Automated evidence passes:

- `cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view --lib`
  (`100` passed, `1` GPU-only test ignored), including mono and generated
  multiview shader validation;
- `cargo build --manifest-path native/Cargo.toml -p mclone-native-client
  --bin mclone-native-client`;
- `node --check scripts/capture-terrain-seam-review.mjs`;
- the 48-capture PNG/readiness/vegetation/group-identity campaign; and
- the clean 18-second headed slow-traversal report above.

### Human Review 2 rejection: open voxel-to-smooth crack

The first packet substantially improved the lighting, material, water, and
proxy transition, but Human Review 2 rejected it on 2026-08-16 because exposed
sky remains visible as a blue line where the spacing-one voxel shell meets the
spacing-two smooth mesh. This is a geometric/topological crack, not a water
tint, environmental-light, fog, or ambient-occlusion mismatch.

The topology change made Tactical 304's original weld incomplete. Smooth
fine-level boundary vertices still interpolate the adjacent parent profile,
but the voxel shell rounds each flat top to a block height. Its outer cardinal
face currently joins that rounded top to another rounded fine-level neighbor
sample rather than to the continuous parent boundary. The two sole horizontal
owners therefore agree in plan view but can leave a sub-block vertical gap.

The considered corrections are:

1. **Selected: explicit parent-profile boundary curtain.** Keep the existing
   single horizontal owner and reserved outer cardinal faces. At both endpoints
   of each boundary segment, evaluate the same stitched parent profile consumed
   by the spacing-two mesh, then span the bounded vertical difference from the
   rounded voxel top. Preserve stable winding with endpoint lower/upper
   envelopes, ordinary side material and lighting, and the existing water
   owner. This adds no horizontal overlap or second terrain surface.
2. **Dedicated zipper transition ring.** Introduce a narrow topology whose
   inner edge exactly matches voxel steps and whose outer edge exactly matches
   parent triangles. This offers the most control if the curtain reads as a
   wall, but adds topology, vertex work, tests, and corner cases and is not the
   first correction.
3. **Morph the final voxel row.** Move its outer vertices onto the parent edge.
   This is small and watertight but creates ramps or wedges exactly where the
   blocky character is meant to remain, so it is rejected as the first choice.
4. **Adaptive safety skirt.** Extend the voxel edge just far enough vertically
   to cover numerical residuals. A tiny conservative skirt may remain a safety
   net, but a blind downward wall is not the primary solution because it can
   become conspicuous above water or on exposed ridges.

Opaque overlap, geometry crossfade, depth bias, fog, a color strip, and a
larger exact radius remain rejected: each hides the opening without making the
two owning boundaries watertight. The selected connector must remove exposed
sky at straight edges, corners, slopes, water, negative coordinates,
stationary cameras, rebases, and teleports without z-fighting or a horizontal
collar. Replacement evidence reopens Human Review 2; Phase 3 stays blocked.

### Phase 2 correction execution record

Commit `d2e81187` implements the selected connector. Every reserved outer
voxel cardinal-face vertex now evaluates the same stitched fine-edge endpoint
that defines the spacing-two parent's piecewise-linear boundary. The face
spans the lower/upper envelope of that continuous endpoint and the rounded
voxel top, so its existing counter-clockwise winding remains valid even when
the profiles cross within a one-block segment. Interior risers and the
exact-frontier fallback curtain remain unchanged.

This changes only the position of the already-submitted outer cardinal faces.
It adds no vertices, horizontal overlap, geometry owner, uniform bytes, sample
fields, buffers, bind groups, fixed allocations, or per-view products. One
additional stitched-height evaluation runs only for outer boundary-face
vertices. Mono, generated multiview, and the ordinary per-eye path continue to
consume the same WGSL source.

The replacement command was:

```bash
pnpm native:terrain-seam-review:capture -- \
  --output /tmp/mclone-terrain-seam-review-hr2-connector \
  --review-phase 2 \
  --settle-frames 300 \
  --skip-build
```

The schema-one receipt is
`/tmp/mclone-terrain-seam-review-hr2-connector/receipt.json` and records
revision `d2e81187`. All 48 1280-by-720 captures passed on their first attempt.
Every composed capture has 25 exact columns, 160 ready clipmap slots,
target-ready terrain, and drained failure-free vegetation; Exact Only reports
no horizon allocation. The full natural, topology, term, and stability sheets
were inspected at full resolution. No stable exposed-sky line remains along
the voxel/smooth boundary, including the coast and water crossings, and the
topology diagnostic shows no horizontal collar or second surface. The bounded
connector does not read as a persistent wall in these review cameras.

A fresh headed traversal moved the low camera from X8 to X80 at four blocks
per second over 18 seconds and crossed the spacing-one tile boundary. All
2,145 frames presented. The final interest center was chunk X4 with 49/49
target chunks ready and zero pending jobs, publications, unloads, or render
compile jobs. Terrain GPU duration was 0.128 ms median, 0.189 ms P95, and
0.983 ms P99. The report is
`/tmp/mclone-terrain-seam-phase2-connector-traversal.json`; its dirty-worktree
flag reflects pre-existing out-of-scope documentation edits, while the binary
and review receipt identify committed implementation revision `d2e81187`.

Automated evidence passes:

- `cargo test --manifest-path native/Cargo.toml -p mclone-terrain-view --lib`
  (`100` passed, `1` GPU-only test ignored), including generated mono and
  multiview WGSL validation and the parent-boundary connector contract;
- `cargo build --manifest-path native/Cargo.toml -p mclone-native-client
  --bin mclone-native-client`;
- the strict 48-capture readiness, PNG, vegetation, and group-identity gates;
  and
- the 18-second headed traversal above.

Gate — Human Review 2: the procedural topology becomes smoother with distance
without exposed sky or a stable square/annulus in land color, water tint,
lighting, texture contrast, or vegetation brightness.

## Phase 3: Exact-to-Voxel Frontier Convergence

Status: blocked on renewed Human Review 2.

- Compare controlled exact and voxel faces under equal atlas, biome, fog-off,
  transfer, full-sky, zero-block-light, and unoccluded inputs.
- Correct material, tint, texture, face-shade, and lightmap differences before
  adding any approximate AO.
- Attribute real-scene differences to stored light, AO, unavailable topology,
  connector material, water arbitration, or vegetation ownership.
- Add bounded height-neighborhood AO only if the controlled and multi-scene
  evidence justifies it.
- Inspect straight boundaries, corners, holes, slopes, water edges, trees,
  negative coordinates, admission, and eviction at noon and midnight.

Gate — Human Review 3: the exact/voxel frontier is coherent in representative
natural terrain. Any remaining discontinuity is tied to a documented fact the
procedural source does not carry, not a mismatched shader formula.

## Phase 4: Motion, Platform, and Performance Acceptance

Status: blocked on Human Review 3.

- Repeat the accepted scenario set during walking, orbit, clipmap rebase,
  exact admission/eviction, teleport, and source reset.
- Prove native and headed desktop/phone WebGPU pixels, synthetic stereo,
  ordinary per-eye XR, and full-frame multiview shader/render behavior.
- Build flat Android and Android XR; obtain physical pixels where the current
  platform matrix requires them.
- Record fixed/resident bytes, submitted vertices, refill/upload work, CPU
  frame summaries, and available GPU timing before and after.
- Verify exact-only allocation and pixels remain unchanged.

Gate — Human Review 4: accept the final composed frontier and both LOD border
types in still and moving evidence. Only then mark this tactical and Tactical
304's inherited visual-acceptance gate complete.

## Acceptance

- At ticks `0`, `6000`, `12000`, and `18000`, all procedural surface classes
  consume the same environmental time-of-day state as exact terrain.
- An unoccluded procedural natural surface using sky light `15` and block light
  `0` follows the exact lightmap response within the controlled fixture.
- Procedural water and tree proxies no longer retain daytime brightness at
  midnight.
- The voxel/smooth border exposes no sky-colored geometric crack and does not
  form a stable color, light, water, texture, or vegetation ring at stationary
  or moving cameras.
- The exact/voxel border agrees under controlled equal inputs and remains
  coherent in representative natural scenes.
- AO is either implemented from stable bounded neighborhood facts or retained
  as a measured, documented semantic limitation; it is never approximated by
  arbitrary border darkening.
- Water keeps one visible composition owner and a continuous appearance across
  both border types.
- Clipmap ownership, fine/coarse stitching, exact coverage, and whole-record
  vegetation arbitration remain correct through movement and delayed work.
- Mono, per-eye, and multiview use the same material/light/occlusion source.
- Exact-only remains horizon-allocation-free; composed memory and frame cost
  remain bounded and recorded.
- Human Review accepts every phase in order.

## Non-Goals

- Increasing exact chunk radius to hide either border.
- Replacing the toroidal clipmap or spacing-one voxel shell.
- Crossfading coincident opaque terrain or water surfaces.
- Distant artificial/block light, caves, overhangs, edited structures, or
  exact stored-light parity in approximate terrain.
- Shadow maps, global illumination, screen-space AO, reflections, water
  specular, or a general weather system.
- Seasonal sun paths, latitude, orbital controls, or altered day length from
  Tactical 307.
- Independent flat water geometry, translucent-water parity, or a hydrology
  rewrite.
- Changing worldgen, persisted terrain identity, or active-pack art to make a
  renderer seam less visible.
- Adding a second blocky clipmap level without a separate quality/performance
  decision.

## Code and Documentation Map

- `native/crates/mclone-terrain-view/src/shaders/terrain_preview_render.wgsl`
  — terrain material, water, near/far shade, and transition behavior.
- `native/crates/mclone-terrain-view/src/shaders/terrain_preview_tree.wgsl`
  — procedural tree color, lighting, fog, and multiview source.
- `native/crates/mclone-terrain-view/src/viewport_renderer.rs` — shared
  uniforms, lightmap input, pipelines, and diagnostics.
- `native/crates/mclone-terrain-view/src/composition.rs` — exact coverage and
  frontier ownership policy.
- `native/crates/mclone-scene/src/terrain_view.rs` — live scene time, coverage,
  and terrain-presentation orchestration.
- `native/crates/mclone-render/src/light_texture.rs` and
  `native/crates/mclone-render/src/shaders/chunk_textured*.wgsl` — exact
  lightmap reference and stored-light presentation.
- `native/crates/mclone-mesh/src/builder.rs` — exact face shade, AO, tint, and
  packed-light inputs.
- [`304-lod-frontier-and-near-field-voxel-convergence.md`](304-lod-frontier-and-near-field-voxel-convergence.md)
  — implemented ownership/topology/material foundation and inherited review
  gate.
- [`../topics/procedural-horizon-clipmap.md`](../topics/procedural-horizon-clipmap.md)
  — residency, transitions, exact handoff, and platform contract.
- [`../topics/procedural-horizon-surface-appearance.md`](../topics/procedural-horizon-surface-appearance.md)
  — material, water, light, and current performance contract.
- [`../topics/lighting.md`](../topics/lighting.md) — exact light solver,
  lightmap, AO, sky, and time-of-day reference status.
