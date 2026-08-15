# Tactical 309: Procedural Horizon Lighting and Seam Convergence

Status: Phase 0 implemented 2026-08-15; awaiting Human Review 0. No
appearance correction has begun.

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

Status: implemented; Human Review 0 pending.

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

## Phase 1: Shared Environmental Illumination

Status: planned; highest priority.

- Establish one renderer-neutral full-sky/zero-block-light environmental term
  from the scene time-of-day input.
- Apply it to every terrain level, both water paths, and procedural trees.
- Ensure near/far material weighting cannot remove or duplicate the term.
- Keep geometric face/slope shade separate and unchanged in this phase except
  where separation is necessary to remove double multiplication.
- Add focused WGSL/source tests and mono/multiview shader validation.
- Capture all Phase 0 scenes at the four frozen times.

Gate — Human Review 1: exact, voxel, smooth, water, and tree presentations
belong to the same time of day. Midnight contains no daytime-green horizon or
bright-cyan procedural water, while noon is not globally over-darkened.

## Phase 2: Voxel-to-Smooth Procedural Convergence

Status: blocked on Human Review 1.

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

Gate — Human Review 2: the procedural topology becomes smoother with distance
without a stable square/annulus in land color, water tint, lighting, texture
contrast, or vegetation brightness.

## Phase 3: Exact-to-Voxel Frontier Convergence

Status: blocked on Human Review 2.

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
- The voxel/smooth border does not form a stable color, light, water, texture,
  or vegetation ring at stationary or moving cameras.
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
