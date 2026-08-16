# Tactical 306: Seasonal Appearance Preview

Status: implementation and automated evidence complete 2026-08-16, including
independent solar and ground controls; Human Review plus current-revision
physical Quest interaction/performance remain pending because no authorized
headset is attached

Topic: `seasons`

Dependency update 2026-08-16: Tactical
[`307`](307-seasonal-solar-path-and-cyclical-latitude.md) landed the shared
dependency-leaf `mclone-season` crate, unsaved `SeasonPreviewSettings`, and one
typed `SetSeasonPreview` UI action/effect. Human Review accepts its global
orbital phase, effective-latitude policy, solar sample, and Earth-like
original-Mclone sun size. This tactical extends that owner for evaluated local
appearance and recent snow rather than create another action family or
independently controlled local phase. Solar and material response consume the
same global phase and latitude, but their render consumers can now be enabled
independently.

## Instruction Synthesis

Prove the first useful part of seasons as a visual-only, exact-terrain slice.
Use one global preview-calendar phase to drive both Tactical 307's sun and one
shared evaluated local seasonal-appearance model. Make the same fixed world
move gradually through the resulting local cycle and preview one bounded
recent-snowfall pulse without adding an authoritative calendar, gameplay
effects, unloaded-world simulation, block mutation, or seasonal persistence.

Move the growing seasonal inspection surface into one shared `Seasonal Debug`
screen reached from the existing Debug options category. It owns Tactical
307's existing preview, latitude, and solar-time controls plus this tactical's
calendar display, evaluated local season, and recent-snow control. The global
preview date and local evaluated result must be visible together. All rows use
the same shared UI actions and scene/render state in desktop flat, desktop
OpenXR, and Android XR; do not add a desktop shortcut or XR-only menu branch.

The resulting Debug surface must also support solar-only review: keep the
global date, latitude, sun path, sky, and rendered daylight active while
seasonal ground tint, vegetation dormancy, and derived snow remain exactly
neutral. Solar controls and ground appearance are independent consumers of
shared climate inputs, not one all-or-nothing season master.

Explicitly defer the procedural-horizon LOD. Tacticals
[`304`](304-lod-frontier-and-near-field-voxel-convergence.md) and
[`305`](305-fine-homestead-lod-overlay.md) already own active exact/LOD
frontier, material convergence, and fine-overlay work. This tactical changes
only exact near-field terrain and exact-associated grass/vegetation. Its pixel
acceptance runs in the existing `Exact Only` terrain presentation. It must not
add seasonal fields, uniforms, materials, or special cases to
`mclone-terrain-view`.

## Starting Point

The project already has most of the host and inspection shape needed for a
small proof:

- the original Mclone profile publishes topology-aware biome columns whose
  regional choices already reflect broad temperature, moisture, and altitude;
- exact render-section compilation already sees block/model facts, visible
  face direction, packed light, and blended biome tint inputs;
- exact chunk shaders already receive world position, vertex color, packed
  light, time-of-day darkening, fog inputs, and topology periods;
- mono, placed-world, per-eye, and full-frame multiview exact-terrain shader
  families share one renderer owner;
- the lush-grass renderer is a separate exact-associated world-space layer
  that must agree with visible ground response;
- the shared Options hub has a Debug category with controller-accessible cycle,
  checkbox, and slider rows;
- `GameUiAction` and `ClientExperienceController` already route Debug Pane,
  Frame Metrics, and Fullbright controls through flat and XR presenters; and
- the XR pause/options panel uses the same `mclone-ui` screen and supports
  pointer/controller interaction in both per-eye and multiview rendering.

No authoritative seasonal clock, material appearance sample, local snowfall
pulse, exact-terrain material response, appearance uniform, or appearance
capture fixture exists. Tactical 307's shared master and solar-specific Debug
values now exist, but do not affect terrain materials. Current biome tint is
compiled into vertex color. The exact textured vertex does not carry a
distinct seasonal material or upward/exposure flag, so the first slice must
audit whether the existing facts can be packed without a costly general
vertex expansion.

The current lifecycle contract is also intentionally useful here: the preview
is client-local presentation state. It does not ask the server to advance time,
does not affect wildlife, and does not reopen the anonymous-versus-tagged
animal decision in [`../topics/seasons.md`](../topics/seasons.md).

## Objective

At one fixed camera, fixed preview solar time/noon, fixed authoritative
weather, fixed world revision, and fixed exact-chunk set, the user can enable a
preview and scrub one global preview-calendar phase continuously through:

```text
Northward Equinox -> Northern Solstice -> Southward Equinox
  -> Southern Solstice -> Northward Equinox
```

At every date, the screen shows the global synthetic date and the observer's
evaluated local result. A northern temperate location may read Spring, Summer,
Autumn, and Winter across those landmarks; a southern temperate location reads
the opposite local seasons. Equatorial and weak-amplitude regions must not be
forced into a misleading four-season label.

The scene changes immediately and coherently:

- transitions between the named quarter-year landmarks are gradual, including
  the Winter-to-Spring wrap;
- the visible sun, day length, evaluated local season, and material response
  remain coherent because they consume the same orbital phase and latitude;
- deciduous foliage and biome-tinted grass show recognizable seasonal color;
- evergreen foliage responds less than deciduous foliage;
- cold/high/exposed ground can receive a texture-preserving seasonal snow
  blend;
- warm regions resist snow and show a much smaller winter response;
- dry regions remain visually distinct from productive green regions;
- lush grass agrees with the underlying ground and does not remain bright
  summer green through strong winter snow;
- a bounded `Recent Snow` pulse can temporarily raise ground and exposed
  canopy coverage around one anchored location, with smooth spatial falloff;
  and
- disabling both visual consumers restores the current renderer output
  exactly; disabling only `Ground Appearance` retains seasonal solar/sky/light
  while restoring exact neutral seasonal ground.

Changing preview state performs no authoritative mutation, persistence,
lighting, chunk scheduling, mesh rebuild, or atlas replacement. The proof
answers whether useful seasonal variety can be presented cheaply before any
authoritative calendar or gameplay system exists.

## Binding Decisions

### Preview state is typed, continuous, global, derived locally, and temporary

Use fixed-point shared values equivalent to:

```text
SeasonPreviewSettings {
    enabled,             // solar/sky/rendered daylight
    appearance_enabled,  // terrain/vegetation/derived snow
    orbital_phase: OrbitalPhase,
    latitude_source,
    manual_latitude,
    solar_time_source,
    manual_solar_time,
    recent_snow: Option<LocalSnowPulse>,
}

EvaluatedLocalSeason = evaluate(
    orbital_phase,
    effective_latitude,
    local_climate,
    altitude,
)

LocalSnowPulse {
    canonical_center_xz,
    radius_blocks,
    intensity: UnitU16,
}
```

Both enable flags are false by default on process start, world entry, and new
scene construction. The existing northward-equinox orbital default is
retained, and a new preview has no recent-snow pulse. Fixed point avoids
float-equality state drift; the renderer may receive normalized finite floats
derived from it. The evaluated local result is recomputed from the
preview/calendar input and observer-local facts; it is never a second stored
or independently editable phase. The value is developer presentation state:

- it is not written to world records or client preferences;
- it is not sent through gameplay protocol or server commands;
- it does not alter day time, authoritative weather, biome, climate, blocks,
  light, crops, resources, fluids, or entities;
- it is not described as the world's authoritative current season; and
- a future real calendar and active-weather producer may use the same pure
  appearance vocabulary without inheriting the Debug controls' ownership.

The existing Debug category exposes one `Seasonal Debug...` navigation row
with a compact `Off` or `Day N/112 · <local result> · S+/- G+/-` summary. The
dedicated shared screen exposes:

- `Solar Preview`, the sun/sky/rendered-daylight checkbox;
- `Ground Appearance`, the exact-terrain/vegetation/derived-snow checkbox;
- `Global Preview Date`, the existing orbital-phase value presented as a
  cyclic slider with a synthetic `Day N/112` label;
- `Global Milestone`, a read-only equinox/solstice interpretation;
- `Evaluated Local Season`, a read-only local interpretation;
- `Daylight`, a read-only duration from the shared solar sample;
- Tactical 307's `Latitude Source`, `Preview Latitude`, `Solar Time Source`,
  and `Preview Solar Time` rows;
- `Recent Snow`, a zero-to-full slider for one bounded local pulse; and
- `Seasonal LOD`, a read-only `Deferred` diagnostic in this tactical.

The synthetic calendar is only a review projection over `OrbitalPhase`:

```text
phase 0.00 -> Day 1   -> Northward Equinox
phase 0.25 -> Day 29  -> Northern Solstice
phase 0.50 -> Day 57  -> Southward Equinox
phase 0.75 -> Day 85  -> Southern Solstice
phase 1.00 -> Day 1   -> wrap
```

Keep the fixed-point orbital phase canonical and continuous; `Day N/112` is a
human-readable label, not a new simulation clock or a loss of sub-day review
precision. The 112-day display is a provisional Stardew-like four-by-28-day
review scale. It does not select the future authoritative year length, create
a year number, or advance automatically.

When a later tactical adds a real persisted calendar, evolve the control to
`Season Source: World Calendar | Manual Preview`. World mode shows a read-only
`Year N · Day N/total`; Manual Preview retains the scrubber. A compact
player-facing calendar may later reuse the same global date plus evaluated
local result, but Tactical 306 adds only the dedicated developer screen.

Northern temperate review landmarks map Spring `0.00`, Summer `0.25`, Autumn
`0.50`, and Winter `0.75`; southern temperate landmarks are shifted by half a
cycle. Pin northern late Winter at phase `0.875` / Day 99 and southern late
Winter at phase `0.375` / Day 43. Color, dormancy, and seasonal snow targets
interpolate through all landmarks without a snap. These presets are fixture
dates, not discrete render modes or extra seasons.

Do not call the date `Current` or imply a real calendar exists. The title-side
screen may show the navigation row disabled, but an active world's
pause/options path must make the dedicated screen interactive.

### Use one shared Seasonal Debug screen and action family

Extend Tactical 307's typed preview setting/action/effect through the same
owners as the existing Debug settings:

```text
mclone-ui Debug -> Seasonal Debug navigation and typed rows
  -> typed GameUiAction variants
  -> ClientExperienceController classification
  -> one scene/render setting effect family
  -> shared mono/XR render state
```

Desktop flat, desktop OpenXR, Android XR, synthetic stereo, offscreen capture,
and other capable consumers must observe the same values and local evaluation.
No app may translate menu text, intercept a platform-private action, or own a
second phase/pulse type. Moving Tactical 307's rows is a shared UI navigation
change, not a new state owner. Automation must be able to set and observe exact
fixed-point orbital phase, synthetic date, evaluated local result, pulse
intensity, center, and radius without pointer coordinates or text scraping.

Do not add a new keyboard shortcut. The shared Debug menu remains the entry
point, and the dedicated seasonal screen prevents the general Debug page from
becoming an unscannable collection of solar, calendar, climate, and weather
rows.

### Evaluate local season from the accepted solar coordinate policy

This tactical proves visual response, not new global climate geography. Reuse
Tactical 307's accepted orbital phase, cyclical-plane/asymptotic-cylinder
effective latitude, and solar day-length sample together with the biome
visual/climate facts, altitude, visible face direction, surface exposure, and
block/material semantics already available to exact mesh compilation. Do not
create another latitude mapping, axial tilt, hemisphere rule, year clock, or
persisted calendar.

The material model must not infer season from instantaneous sun elevation:
morning and evening are not winter. Sun and appearance are sibling evaluations
of the same global orbital phase and observer/position climate facts. A pure
local result may expose thermal phase, response strength, snow tendency, and a
typed display label while retaining the global phase in diagnostics.

Four-season labels are appropriate only where the local climate response makes
them meaningful. A southern temperate location shifts the northern sequence by
half a cycle. Equatorial/warm low-amplitude locations report a weak thermal
season rather than false Spring/Winter labels; wet/dry naming remains deferred
until moisture seasonality exists. Polar/high locations may report a local
season plus stronger snow/thaw tendency without inventing a second calendar.

The seasonal response must still vary spatially. A warm biome and a cold/high
biome must not receive the same coverage at the same global date merely because
their derived local thermal phase is similar. Existing topology-aware biome
identity and canonical position remain part of the evaluation. Any
deterministic breakup used near a threshold must reproduce across chunk
boundaries and the X-periodic cylinder seam.

Do not re-query the generator from the fragment shader or create a renderer
copy of Mclone worldgen. Mesh compilation may derive a compact static response
from the authoritative snapshot's biome, block/model, face, light/exposure,
and world-position facts. The seasonal preview then combines that static fact
with one compact per-frame orbital/local-evaluation/pulse input.

### Preview changes never rebuild terrain

Changing the preview enable, global calendar phase, or recent-snow intensity may
update already-written per-frame/per-view uniform data. It must not:

- mark a chunk or render section dirty;
- enqueue a render-section compile;
- replace a CPU or GPU mesh;
- upload a new block atlas;
- publish a block delta or chunk snapshot;
- enqueue light work;
- dirty persistence; or
- scan inactive chunks.

A one-time static mesh-format change is permitted only if the preflight proves
that the shader cannot distinguish the required material/face response from
existing attributes. Prefer a packed response key or otherwise reuse existing
storage. Report added CPU payload bytes, GPU vertex bytes, compile time, upload
bytes, and retained exact-terrain memory. Do not casually add several floats
per vertex.

If the smallest correct design adds more than four bytes per exact textured
vertex or more than ten percent to representative exact-terrain GPU mesh
bytes, stop and compare a packed bitfield, spare-channel encoding, per-section
data, and a bounded surface overlay before choosing a representation.

### Seasonal and recent snow compose as appearance

Snow in this tactical is a derived material response over eligible upward,
sky-exposed exact faces. The pure model combines two targets:

```text
snow coverage = clamp(
    seasonal baseline(evaluated local season, local climate, altitude, exposure)
    + recent snowfall(local pulse, retention, surface response),
    0, 1)
```

The recent-snow pulse represents material temporarily retained after a local
snowfall event, not a second global winter control. On the first Debug change
from zero to a positive intensity, anchor its center at the current canonical
player/camera XZ. Further intensity changes retain that center, so walking
away exposes the pulse boundary instead of dragging the weather with the
player. Returning the slider to zero clears it; the next positive adjustment
anchors a new pulse. Use one implementation-selected, hard-bounded radius with
a smooth radial falloff and report the center/radius in diagnostics.
In XR, use the shared player/observation root rather than either per-eye view
as the anchor, so both eyes and both render paths receive identical world-space
event geometry.

Distance to the pulse must use the world's topology-aware canonical
displacement so coverage is continuous across the X-periodic cylinder seam.
The pulse affects only exact surfaces drawn inside its footprint. It creates
no per-chunk mask, resident event list, inactive-chunk query, or work outside
ordinary rendering. Warm local climate and a warm evaluated local result reduce
retained snow even under a strong pulse; a late-Winter pulse in a cold or
temperate region should be the clearest proof.

Ground and canopy consume separate semantic response weights. Eligible
natural ground may receive the strongest coverage. Exposed upward deciduous
and evergreen canopy faces may receive a lighter dusting, while side faces and
sheltered foliage remain mostly unchanged. Canopy snow must not turn all leaf
pixels uniformly white or erase the evergreen/deciduous distinction.

For both seasonal and recent snow:

- preserve underlying atlas detail instead of replacing the texel with flat
  white;
- use local climate/altitude response and a seam-safe stable threshold breakup;
- exclude water, translucent surfaces, underground faces, and clearly
  incompatible blocks;
- make no height, side skirt, collision, track, shoveling, drop, meltwater, or
  inventory claim; and
- create no new geometry when the preview changes.

The proof may use narrow explicit natural-ground and canopy allowlists.
General snow on player-built roofs and complete block/material semantics are
later work. Do not use `snow` or `snow_layer` block identity unless the server
actually owns such a block.

The `Recent Snow` slider manually samples deposition and recession in this
visual proof. It does not run an automatic timer, precipitation particles, or
a weather scheduler. A later active-world weather event may drive the same
bounded intensity through a rise/hold/decay envelope without changing the
material contract; it must still do no unloaded catch-up.

### Vegetation response stays semantic

The mesh/compiler owns whether an exact visible surface is grass-tinted,
deciduous foliage, evergreen foliage, lush-grass ground, snow-eligible natural
surface, or seasonally inert. Do not infer these meanings from atlas UV
coordinates or texture colors in the shader.

The initial response should remain small. The columns below are quarter-year
review landmarks; the model interpolates continuously between them:

| Surface family | Spring | Summer | Autumn | Winter |
|---|---|---|---|---|
| biome-tinted grass/fern | fresh green | present baseline family | muted/dry | dormant; suppressed under snow |
| deciduous foliage | fresh green | present baseline family | strong warm hue | subdued/dormant; light exposed-canopy snow response |
| evergreen foliage | small freshening | near baseline | small desaturation | dark/cold tint; distinct exposed-canopy snow response |
| warm wet vegetation | modest phase change | lush | modest phase change | no automatic frost |
| warm dry ground/grass | limited green response | dry response | dry/muted | no automatic snow |
| cold/high exposed ground | thaw shoulder | mostly exposed where warm enough | frost shoulder | strongest snow blend |
| inert/artificial material | unchanged | unchanged | unchanged | unchanged in this proof |

Exact color constants are Human Review outputs, not architecture. Avoid one
global orange multiply or one global white fade.

### Exact-only owns acceptance; LOD remains visibly deferred

All visual acceptance captures and interactive A/B decisions use
`GameTerrainPresentation::ExactOnly`. The user can select that existing value
through `Graphics -> Terrain Horizon -> Exact Only` before examining seasons.
Changing the season must not silently change the terrain-presentation setting.

When composed terrain is enabled, exact near terrain may show the preview while
the procedural horizon retains its current appearance. That mismatch is an
explicit diagnostic limitation, not acceptance evidence. The Debug Pane or
other stable diagnostic state must report both the active preview and that
seasonal LOD response is deferred.

Do not edit:

- clipmap sampling or tile state;
- procedural ground, water, skirt, or vegetation shaders;
- exact-painted coverage/frontier behavior;
- fine homestead overlay inputs; or
- World Explorer/Terrain Lab appearance.

Seasonal LOD adoption begins only after Tacticals 304/305 settle the relevant
surface and overlay contracts and a later tactical can consume this proof's
accepted appearance semantics without parallel shader ownership.

If Tactical 304 is actively changing a shared exact-terrain vertex, uniform,
or shader file, sequence the overlapping implementation work rather than
mixing both tacticals in one commit. Tactical 306 should baseline after that
accepted exact-render change while retaining its independent no-LOD scope.

### Every exact render topology changes together

The seasonal response is world-space. The first drawable implementation must
cover:

- ordinary mono exact terrain;
- placed/composed exact terrain variants used by embedded worlds;
- normal XR per-eye terrain;
- full-frame multiview exact terrain; and
- lush-grass mono, placed, per-eye, and multiview variants where active.

Each eye/layer uses its own immutable view data for the full submission, but
both eyes consume the same season preview and canonical world response. A
mono-only shader patch is not a partial success. Actors, UI, sky, fog, and
procedural horizon remain unchanged.

### Disabled preview is an exact no-op

`Season Preview: Off` must retain the current visual and runtime result:

- the seasonal multiplier/blend is exactly neutral;
- exact terrain and grass use the same textures, tint, light, fog, depth,
  alpha, and color-transfer path as before;
- no seasonal allocation or per-chunk work occurs after setup; and
- fixed same-host before/after captures remain byte-identical wherever the
  existing deterministic offscreen lane permits it.

If adding a static response attribute necessarily changes buffer layout, the
payload may differ while preview-disabled pixels and draw ownership remain
exact. That cost must still be measured and justified.

## Implementation Sequence

### Slice 0: Baseline, data-path audit, and failing visual fixtures

Status: complete 2026-08-16 at pre-implementation revision `ca064e883607`.

- Capture one fixed temperate exact-only scene with the preview disabled
  under frozen noon light and save it under `/tmp`.
- Pin warm/wet, temperate/deciduous, dry/open, and cold/high exact capture
  locations from existing Mclone biome facts. Record seed, profile, topology,
  camera, loaded chunks, biome IDs, surface altitude, and presentation mode.
- Add pure fixture inputs that demonstrate the desired regional response
  matrix before changing shaders.
- Audit exact vertex layout, current color/alpha use, biome visual ownership,
  face direction, packed light/exposure, placed variants, lush-grass payload,
  and all mono/multiview shader copies.
- Record representative section vertex count, GPU mesh bytes, compile/upload
  counters, flat frame CPU/GPU, and Quest exact-only frame cost.
- Select the smallest static seasonal response encoding and record why it is
  preferable to the alternatives.

Gate: the execution record contains inspected preview-disabled pixels, pinned
regional fixtures, the complete affected shader list, and a measured
data-layout choice. No production seasonal effect lands before this gate.

Execution record:

- The inspected `960x600` exact-only, frozen-noon, preview-off baseline is
  `/tmp/mclone-seasonal-appearance-baseline-71fb5110/temperate-preview-off.png`
  with SHA-256
  `e8ec8e4a7e259293e339114a1564357eda0a0701515a0e50f39454308670caa4`.
  It drew 84 of 148 resident sections and retained the expected textured
  terrain, trees, flowers, actors, fog, and sky.
- Seed `12345` fixtures are pinned from the generated worldgen receipt at
  `/tmp/mclone-seasonal-appearance-baseline-71fb5110/worldgen/`:
  warm/dry lowland `(8, 68, 8)`, temperate woodland `(1032, 89, 1032)`,
  cool/wet conifer `(-632, 88, 136)`, and cold/high alpine
  `(-1720, 141, -776)`. Pure-model tests retain an additional warm/wet input
  because the bounded map's useful exact warm site is dry.
- The complete exact shader surface is `chunk_textured.wgsl`,
  `chunk_textured_multiview.wgsl`, `chunk_textured_placed.wgsl`,
  `chunk_textured_placed_multiview.wgsl`, and their generated clipped-placed
  forms, plus `grass.wgsl`, `grass_multiview.wgsl`, `grass_placed.wgsl`,
  `grass_placed_multiview.wgsl`, and their generated clipped forms.
- The RD3 CPU mesh baseline built 1,296 sections / 521 non-empty sections,
  1,671,148 vertices, 2,506,722 indices, and 417,787 faces in `571.345ms` in
  the debug optimized profile. At the existing 40-byte exact vertex stride,
  those vertices occupy `66,845,920` payload bytes; indices occupy
  `10,026,888` bytes. The run is recorded in
  `/tmp/mclone-seasonal-appearance-baseline-71fb5110/mesh-cpu.txt`.
- Physical Quest was not attached during this preflight. The latest accepted
  stationary RD5 exact-only reference remains Tactical 277's Quest 3
  `2.330ms` / `2.267ms` Meta app-GPU A/repeat at revision `770ebe89`.
  Current-revision physical A/B remains required in Slice 4.
- `TexturedChunkVertex::packed_light` is 40 bytes total and decodes only bits
  4..7 and 20..23 for block and sky light. Select its currently zero high byte
  for a compact seasonal key: three surface-family bits, one upward/exposed
  bit, two temperature-class bits, and two moisture-class bits. This adds zero
  CPU, GPU, or Worker-transfer bytes. `GrassPatch` retains its existing
  32-byte ABI and uses its explicit reserved word for the same key. Adding a
  new vertex word would cost `6,684,592` bytes (`10%`) in the measured corpus,
  so the zero-growth encoding is strictly preferable.

### Slice 1: Pure seasonal appearance model

Status: complete 2026-08-16 in `mclone-season`.

Extend the existing dependency-leaf `mclone-season` owner with the
presentation-independent vocabulary and pure response math. It may own:

- the 112-day synthetic display projection over existing `OrbitalPhase`;
- evaluation from global orbital phase, effective latitude, and compact local
  climate/altitude into typed local phase, response strength, snow tendency,
  day length, and display label;
- northern/southern temperate and weak-amplitude label semantics;
- compact local climate/region inputs;
- vegetation dormancy/color response;
- a bounded topology-aware local snowfall pulse;
- separate seasonal-baseline, recent-ground, and recent-canopy snow response;
  and
- finite/clamped validation.

The crate must not depend on `wgpu`, `winit`, OpenXR, browser APIs, server
runtime, persistence, or app UI. Mesh/world adapters provide existing biome,
altitude, exposure, and material inputs; the render layer consumes the compact
result. Do not create a second season crate or another orbital/latitude type.

Add table-driven tests for at least:

- global phase-to-Day-1/29/57/85 projection and exact wrap;
- opposite northern/southern temperate local evaluations at the same global
  date;
- equatorial weak-amplitude labeling without false four-season claims;
- day-length equality with Tactical 307's shared solar sample;
- warm/wet lowland at all four landmarks and intermediate phases;
- warm/dry lowland at all four landmarks and intermediate phases;
- temperate lowland at all four landmarks and intermediate phases;
- cold/high exposed ground at all four landmarks and intermediate phases;
- deciduous versus evergreen response;
- continuity on both sides of every landmark and the Winter-to-Spring wrap;
- snow threshold shoulders and monotonicity as pulse intensity rises;
- pulse center, radial falloff, outside-radius neutrality, and cylinder-seam
  equivalence;
- late-Winter recent snow on eligible ground and exposed canopy;
- low retention under warm climate/year-phase inputs;
- non-finite and out-of-range input normalization; and
- exact neutral output when the preview is disabled.

Gate: the pure model proves coherent global-date/local-season evaluation,
continuous regional differentiation, bounded local snowfall response, and
neutral disabled state without world loading, rendering, an authoritative
calendar, or a weather scheduler.

Execution record:

- `PreviewCalendarDate` projects fixed-point `OrbitalPhase` onto synthetic
  days 1..112, while `OrbitalMilestone` retains neutral global equinox and
  solstice names.
- `EvaluatedLocalSeason` consumes global phase, signed effective latitude,
  normalized local temperature/moisture, and altitude. It returns local phase,
  response strength, thermal forcing, snow tendency, the accepted solar model's
  day length, and a typed local label. Opposite temperate hemispheres disagree
  by half a cycle; weak equatorial response reports `Weak Thermal Cycle`.
- `StaticSeasonalResponse` owns the measured eight-bit family, exposure,
  temperature, and moisture key plus exact packed-light encode/decode.
- `LocalSnowPulse` uses fixed-point intensity, one 96-block bounded radius,
  canonical integer center, smooth radial falloff, and topology-aware shortest
  displacement. `evaluate_surface_appearance` composes continuous vegetation
  tint/dormancy with distinct seasonal and recent ground/canopy snow targets.
- Twenty-four focused crate tests cover calendar anchors/wrap, solar day-length
  agreement, hemispheres, equatorial weakness, regional climate and material
  matrices, deciduous/evergreen distinction, landmark continuity, packed-key
  round trips, recent-snow monotonicity/falloff/cylinder seam, warm retention,
  canopy/side semantics, non-finite normalization, and exact disabled output.

### Slice 2: Exact terrain and lush-grass rendering

Status: complete 2026-08-16.

- Derive the selected static response key during ordinary exact mesh/grass
  preparation from existing biome, block/model, face, light/exposure, and
  topology facts.
- Extend every affected mono, placed, per-eye, and multiview payload/pipeline
  together.
- Apply semantic vegetation response after atlas sampling while preserving
  texture detail, lightmap, fog, alpha/cutout, depth, and target color transfer.
- Apply seasonal/recent snow only to eligible exposed upward ground and canopy
  faces, retaining distinct response weights, and fade/suppress lush grass
  consistently where ground coverage is strong.
- Keep deterministic breakup canonical across section/chunk boundaries and
  exact at the X-periodic seam.
- Add CPU/WGSL parity fixtures for packed response decoding and representative
  material outputs.
- Capture and inspect the first active-preview native pixel immediately, then
  inspect a first late-Winter scene with and without recent snow before adding
  the full palette.

After the view is settled, drag `Global Preview Date` through a full cycle,
observe the sun and evaluated local material response move together, drag
recent snow `0 -> 1 -> 0`, exercise solar-only and ground-only states, and
disable both consumers. Assert zero new section
builds, section uploads, atlas uploads, block/light updates, persistence
dirties, or server commands. One compact existing per-frame uniform write may
carry the orbital/local evaluation, pulse center/radius, and intensity; do not
add per-section season uploads.

Gate: exact near-field terrain, canopy, and grass show useful gradual seasonal
and local recent-snow distinction in mono and stereo without any slider-driven
mesh or world churn.

Execution record:

- Ordinary exact mesh compilation classifies inert, natural-ground, grass,
  deciduous, and evergreen surfaces and packs family, upward exposure,
  temperature class, and moisture class into the unused high byte of
  `packed_light`. Lush grass uses its existing reserved word. The measured
  exact vertex remains 40 bytes and the grass patch remains 32 bytes.
- One shared `seasonal_appearance.wgsl` evaluates continuous vegetation tint,
  seasonal snow, bounded recent ground/canopy snow, deterministic world-space
  breakup, and grass suppression. All mono, placed, clipped-placed, per-eye,
  and full-frame multiview chunk and grass shaders include that owner.
- The existing per-frame exact-terrain uniform grows from 128 to 160 bytes and
  the grass frame uniform from 32 to 64 bytes. This is one constant-size write
  per renderer/frame; there is no per-section seasonal buffer, mask, upload,
  atlas replacement, or mesh mutation.
- Preview-disabled uniforms encode exact zero seasonal state. Two separately
  configured disabled captures at the final revision are byte-identical with
  SHA-256
  `8b3c0e5c27d7b83f3758d46d0df3f2d08c39b7bff2dc50d0581913dad1afd7a5`.
- The final native matrix and headed WebGPU transition both show gradual
  material response and texture-preserving ground/canopy snow. The Web probe
  holds mesh builds, commands, block updates, pending compiles, and accepted
  compile sections unchanged through preview off -> active -> off.

### Slice 3: Shared interactive Debug control

Status: complete for implementation and automated interaction contracts
2026-08-16; live Human Review is part of Slice 4.

- Add one controller-friendly `Seasonal Debug...` navigation row to the shared
  Debug options category, with a compact disabled/date/local-result summary.
- Add one shared `Seasonal Debug` screen containing independent solar and
  ground controls, global preview date, latitude source/value, solar-time
  source/value, recent snow, and read-only global milestone/local
  season/daylight/LOD rows. Group
  them as calendar/evaluation, location/sun, and weather-preview sections so
  the controller and XR scan order remains predictable. Move the existing
  Tactical 307 rows; do not duplicate them.
- Present the existing continuous orbital phase as `Global Preview Date: Day
  N/112`; retain exact phase in diagnostics and automation.
- Add typed screen navigation and recent-snow action/effect extensions through
  `ClientExperienceController`; retain Tactical 307's existing typed orbital,
  latitude, and solar-time setting path. The synthetic date and evaluated rows
  are derived displays and do not create reverse mutation actions.
- Apply the effect in shared `mclone-scene` mono/XR session ownership rather
  than desktop or OpenXR apps.
- On the first zero-to-positive recent-snow effect, anchor the bounded pulse at
  the current canonical player/camera XZ. Clear it at zero and do not silently
  recenter it while the slider remains positive.
- Reset both consumers off, northward-equinox phase / synthetic Day 1, and no
  recent snow for a new process/session; keep all preview state out of
  preference and world persistence.
- Expose enable, exact orbital phase, synthetic global date/milestone,
  effective latitude, typed evaluated local result, daylight, pulse
  intensity/center/radius, exact-only/composed presentation, and `LOD deferred`
  in stable diagnostics.
- Add focus, keyboard/controller navigation, pointer, capability, and effect
  tests for both the Debug entry and nested screen. Automation must set exact
  orbital phase, intensity, and explicit fixture pulse geometry and observe
  exact global/local evaluation through the typed contract.
- Validate the actual menu interaction in desktop flat, desktop OpenXR, and
  Android XR. The XR controls must be readable and operable through the
  existing world-panel pointer/controller path.

The controls may be available on Web and flat Android if their existing shared
UI and exact renderer consume the same actions without a new platform
mechanism. If a profile cannot present them, publish an explicit capability
reason and retain preview-off output; do not silently accept an action that has
no pixels.

Gate: one running desktop and one running XR session can enter the dedicated
screen, see global date and local evaluated season together, scrub the calendar
and watch sun/material response agree, scrub recent snow, inspect the anchored
pulse boundary, and return to exact preview-off output without restart, world
reload, or menu-state drift.

Execution record:

- The shared Debug category now has exactly one `Seasonal Debug...` entry. Its
  shared nested screen owns independent `Solar Preview` and `Ground
  Appearance` checkboxes, `Date: Day N/112`, milestone,
  evaluated local season/response/snow tendency, daylight/effective latitude,
  latitude source/value, solar-time source/value, `Recent Snow`, and the
  read-only `Seasonal LOD: Deferred` row.
- Existing orbital, latitude, and solar-time actions plus the typed recent-snow
  action flow through `ClientExperienceController` into one
  `SeasonPreviewSettings` value owned by `McloneSceneHost`. The pulse anchors
  once on the first zero-to-positive adjustment, remains fixed while scrubbed,
  clears at zero, and may anchor again on a later positive adjustment.
- Exact CLI controls and the structured
  `MCLONE_SEASONAL_APPEARANCE_STATE` receipt expose phase/date, observer biome
  and climate, effective latitude and solar sample, evaluated local result,
  exact fixed-point pulse intensity/center/radius, exact-terrain scope, and LOD
  deferral without menu text scraping.
- Desktop OpenXR and Android XR accept `seasonal-debug` as a direct shared
  validation target. The scene routes it to the same screen as flat UI; it is
  not an XR-only menu implementation. Focus, key/controller, pointer,
  capability, reducer/effect, CLI, and XR screen-routing tests cover the path.
- New process/session defaults remain preview off, Day 1/northward equinox,
  world latitude/time sources, and no pulse. No preference, world record,
  protocol, server command, or authoritative state was added.

Post-completion control-isolation record:

- Date and latitude remain available when either visual consumer is active.
  Solar time source/value is available only to `Solar Preview`; recent snow is
  available only to `Ground Appearance`.
- `World Clock` only reads replicated authoritative `day_time`. Manual solar
  time is an unsaved client-render input and never sets server `game_time` or
  `day_time`. The same action produces only `SetSeasonPreview`, with no server
  command, gameplay protocol, persistence, scheduled tick, or light-engine
  mutation.

### Slice 4: Visual matrix, performance, and Human Review

Status: automated native/WebGPU/XR-emulation/build evidence complete
2026-08-16. The first autumn/snow palette-review correction is complete in
`741d5a74`; final Human Review, live desktop/OpenXR interaction, and
current-revision physical Quest pixels/performance remain pending.

Add one focused capture runner, such as
`pnpm native:seasons:appearance-capture`, that saves under `/tmp`:

- preview off plus the four quarter-year landmarks and four intermediate
  global dates from one exact northern-temperate camera, recording each
  evaluated local result;
- matched northern/southern temperate cameras at the same global dates;
- the same evaluated late-Winter fixture at warm/wet, warm/dry, temperate, and
  cold/high locations;
- recent snow at zero, half, and full intensity from one late-Winter camera,
  plus matched inside-footprint, falloff, and outside-footprint views that show
  exposed ground and tree canopy; and
- paired mono and synthetic-stereo late-Winter recent-snow output.

Create a contact sheet only as a review convenience; retain the individual
full-resolution captures and inspect them. Record exact profile, seed,
topology, camera, exact orbital phase, synthetic global date/milestone,
effective latitude, evaluated local season/strength/snow tendency/daylight,
pulse intensity/center/radius, terrain presentation, day time, authoritative
weather, lighting mode, revision, and image paths.

Then run:

- a live desktop flat full preview-calendar scrub with global/local row
  inspection, recent-snow `0 -> 1 -> 0`, and preview-off restoration pass;
- a desktop OpenXR menu pass on an available headset/runtime;
- an Android XR build plus scripted menu/action validation;
- physical Quest pixels for normal per-eye and full-frame multiview exact
  terrain when that render path is available;
- native and headed WebGPU exact-terrain captures through shared renderer code;
- flat Android build/smoke for the affected shared crates; and
- stationary before/after performance rows on desktop and Quest.

Compare preview-off output with active calendar/pulse states for frame CPU/GPU,
exact mesh bytes, render-section compile/upload counters, uniform writes, draw
count, and memory. The steady active preview should add no recurring CPU work
proportional to loaded chunks or pulse radius. If representative Quest
exact-only GPU p95 regresses by more than five percent or 0.25 ms, whichever
is larger, stop and attribute the fragment/data cost before acceptance.

Gate: Human Review accepts coherent global-date/local-season presentation,
gradual local appearance change, regional late-Winter difference, and the
temporary local ground/canopy snowfall response; desktop and XR interaction
are comfortable; and the measured result stays inside the no-remesh/no-world-
work contract.

Execution record:

- `pnpm native:seasons:appearance-capture` produced 22 exact-revision captures
  at `67f91bd5d19b` under
  `/tmp/mclone-seasonal-appearance-67f91bd5d19b-1786893846646/`, including the
  four landmarks, four intermediate dates, opposite hemispheres, equatorial
  weakness, warm/dry, cool/wet, cold/high, half/full/falloff/outside snowfall,
  synthetic stereo, and the shared menu. The receipt is
  `capture-receipt.json`; the inspected review image is
  `seasonal-appearance-contact-sheet.png` in the same directory.
- The runner gates exact typed labels and pulse data: northern phase `0.875`
  / Day 99 is Winter, southern phase `0.375` is Winter, the equator reports
  `Weak Thermal Cycle`, and recent snow retains raw intensity, explicit center,
  and the fixed 96-block radius. It intentionally treats the Summer landmark
  as the neutral material baseline while Spring, Autumn, Winter, and
  intermediate dates prove continuous change.
- Headed Chrome/WebGPU on macOS/Metal captured the same pinned temperate exact
  scene at preview off, late Winter/manual 47.5 degrees/noon/full snow, and
  restored off. Active state changed 412,895 of 921,600 pixels; restored off
  changed zero. Off/restored SHA-256 is
  `2cc0b3f6e67dd39a219c5193446cdc322c771b566cbc046285d91a32f122929a`;
  active SHA-256 is
  `9332ad2118e01585df938362de8eb55b664f22d907d003ef53bf51107b88fafb`.
  Mesh builds stayed 120, commands 11, block updates zero, and pending/accepted
  compile counts zero across all three explicit frames.
- First Human Review found the autumn grass response too green and the
  half-block snow breakup too finely speckled. Revision `741d5a74` strengthens
  the autumn straw target and applies climate-sensitive lush-grass dormancy:
  mild/dry grass reaches the strongest dieback while wetter or warm regions
  retain more growth. Snow now uses a topology-aware 14-block interpolated
  field with a smaller 3.5-block edge field. The edge hash is arithmetic, not
  trigonometric, and the entire response remains fragment-only with no mesh,
  chunk, world, or persistence work.
- The refreshed 22-case exact matrix is under
  `/tmp/mclone-seasonal-appearance-741d5a74c379-1786897744853/`. Its inspected
  contact sheet shows a continuous green-to-straw-to-frost northern sequence,
  connected rather than half-block-speckled snow, snow-free warm/dry terrain,
  partial cool/wet retention, and strong alpine retention. Northern Autumn is
  SHA-256
  `bb8d377ed8e23349bedfa3a15b7a2f2460d91a3b227ffe778e0c2db4fb244487`;
  full recent snow is
  `39f2a859437a4ab8e332f71072aea917dc74d8d4b6457e8745546b3d450e027b`.
- The refreshed headed Chrome/WebGPU probe changes 413,969 of 921,600 pixels
  under full late-Winter recent snow, then restores Preview Off with zero
  changed pixels. Off/restored SHA-256 is
  `8e0e620fadd484750dd787d7157728feabc0c6ee9b685bb7bbbd67c568e68cd4`;
  active SHA-256 is
  `c134a70f5123471558cd3027f5730f10fedcb13fc38f42dec03e6d381773dd8e`.
  Mesh builds stayed 117, commands 11, block updates zero, and
  pending/accepted compile counts zero across all three frames.
- Repeating the preflight RD3 exact CPU mesh corpus after implementation built
  the identical 1,296 sections / 521 non-empty sections, 1,671,148 vertices,
  2,506,722 indices, and 417,787 faces in `570.405ms` versus `571.345ms`
  before (`-0.16%`). Exact vertex bytes remain `66,845,920`; index bytes remain
  `10,026,888`. Static vertex/payload growth is zero.
- Focused suites pass: `mclone-season` 25 tests, `mclone-mesh` 106,
  `mclone-render` 199 with 11 explicitly ignored GPU cases, `mclone-ui` 112,
  `mclone-app-runtime` 297 unit tests plus its integration targets, and
  `mclone-scene` 180 unit tests plus all integration/doc targets, including the
  new Seasonal Debug/XR routing cases (one unrelated GPU characterization is
  ignored). Explicit GPU validation covers every grass pipeline variant and
  the headless static grass artifact path. Native XR-feature and WASM checks
  plus Web TypeScript validation pass.
- `pnpm native:xr-emulation:smoke` passes, and the seasonal matrix's stereo
  case renders 480-by-480 per eye with exact terrain and recent snow. Current
  Android XR release and flat Android debug APK builds both pass after the
  final shared UI routing change.
- `quest doctor --json` currently returns
  `no attached, authorized Quest headset was found`. No current-revision
  physical Quest interaction, pixels, or GPU delta is claimed. The documented
  five-percent/0.25ms Quest stop gate therefore remains open for Human Review
  rather than being waived.

## Acceptance

### Visual and semantic gates

- One fixed northern-temperate camera reads distinctly at locally evaluated
  Spring, Summer, Autumn, and Winter landmarks without changing geometry,
  authoritative weather, or world state.
- Northern and southern temperate cameras report and render opposite local
  seasons at the same relevant global dates.
- Equatorial/warm weak-amplitude regions avoid false four-season labels and
  appropriately restrained material response.
- Intermediate phases change continuously, with no visual snap at a named
  landmark or at the Winter-to-Spring wrap.
- Warm/wet, warm/dry, temperate, and cold/high regions respond differently to
  the same global date and recent-snow intensity.
- Deciduous and evergreen foliage do not receive one identical autumn color.
- Seasonal and recent snow preserve underlying texture, remain on eligible
  exposed upward exact faces, and do not appear in caves or automatically
  cover warm regions.
- Raising recent snow at late Winter gradually dusts eligible ground and
  exposed canopy inside one anchored bounded footprint; falloff is smooth,
  outside terrain keeps its seasonal baseline, and clearing the pulse removes
  only its additional coverage.
- Deciduous and evergreen canopy retain distinct appearance under recent snow,
  with side and sheltered faces resisting the dusting.
- Lush grass agrees with ground dormancy/snow instead of floating as a summer
  layer.
- Preview off reproduces the accepted pre-tactical exact output.
- The result makes no collision, material depth, shoveling, persisted
  accumulation, melting, gameplay, authoritative-season, or authoritative-
  weather claim.

### Interaction and ownership gates

- The shared Debug category exposes exactly one `Seasonal Debug...` entry; one
  shared nested screen owns all seasonal preview, solar, latitude, evaluated
  local season, and recent-snow rows.
- The screen shows synthetic global calendar date and evaluated local season
  together, plus shared daylight and global milestone diagnostics.
- `Global Preview Date` is the only interactive annual-phase control. No
  independent local-season slider can disagree with the sun.
- Desktop flat, desktop OpenXR, and Android XR route those controls through one
  typed action/effect family.
- The controls work live and return to preview-off output without restarting
  or reloading the world.
- A zero-to-positive snow adjustment anchors once at the current canonical
  position; later scrubbing does not drag the pulse, zero clears it, and a new
  positive adjustment may anchor again.
- No desktop-only key, XR-only enum, string command, app-local renderer policy,
  or menu-text automation is added.
- The preview is absent from world saves, preferences, protocol, and server
  state and resets to off/northward-equinox/Day-1/no-pulse as documented.
- Diagnostics report exact orbital phase, synthetic date, effective latitude,
  local evaluation and pulse state and identify procedural-horizon seasonal
  response as deferred.

### Work and performance gates

- Enabling, disabling, or scrubbing calendar/recent snow produces zero block
  writes, light work, entity/ecology work, persistence dirties, server
  commands, chunk snapshots, mesh compiles, section uploads, and atlas uploads.
- Inactive chunks receive no query, update, mask, or catch-up work.
- Any static vertex/payload growth remains within the preflight bound or has a
  separately approved measured representation decision.
- Active-preview steady CPU cost is constant with respect to loaded chunk
  count and pulse radius beyond work already performed for ordinary rendering.
- Desktop and Quest GPU deltas are reported and remain within the Slice 4 gate.
- Mono, placed, per-eye, and multiview exact paths share the same response and
  preserve correct per-view projection.

### LOD deferral gates

- Acceptance runs with `Terrain Horizon: Exact Only` and records that setting.
- No `mclone-terrain-view`, procedural-horizon shader, clipmap, proxy
  vegetation, exact-painted frontier, World Explorer, Terrain Lab, or
  homestead-LOD behavior changes.
- Enabling composed terrain does not crash or corrupt exact terrain; its
  seasonal mismatch remains explicitly diagnostic and is not presented as
  complete season support.
- Tactical 304/305 files, ownership, and acceptance are neither copied nor
  weakened.

## Validation Matrix

Use the current commands in
[`../platforms.md`](../platforms.md#validation-policy) at execution time. The
expected focused core is:

```bash
cargo fmt --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-season
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:seasons:appearance-capture
pnpm native:xr-emulation:smoke
pnpm native:web:chunk-smoke
pnpm native:android:apk
pnpm native:android-xr:apk
```

If preflight selects an existing crate instead of `mclone-season`, replace the
crate command and record the decision; do not create a dummy package merely to
match this draft.

Browser/WebGPU captures on Linux must use the headed Wayland lane after
`pnpm host:check`, and headed GPU captures run sequentially. Save every image,
contact sheet, report, and temporary trace under `/tmp`. Inspect pixels at the
first drawable milestone and after every material expansion.

Use the public Quest testbed/provider contract for physical device selection,
authorization, leases, and sleep-after-use. Project scripts continue to own
the APK, launch arguments, exact-only selection, typed seasonal actions,
render-path selection, and acceptance assertions.

## Non-Goals

- authoritative or persisted season/calendar state;
- selection of authoritative year length, a new latitude mapping, new
  hemisphere semantics, or different axial daylight; the 112-day value is a
  synthetic Debug display only;
- changes to accepted day-length, sky, sun, moon, fog, authoritative weather,
  precipitation particles, weather scheduling, automatic pulse decay, or
  light-solver changes;
- block snow, snow geometry, collision, tracks, shoveling, persisted
  accumulation, melting, or water production;
- seasonal springs, streams, fluids, crops, resources, spawning, animals,
  migration, hibernation, aging, tagging, zoos, or husbandry;
- falling leaves, leaf removal, bloom/flower placement, or vegetation geometry
  replacement;
- player-built roof snow or a universal material taxonomy;
- procedural-horizon, clipmap, LOD vegetation, World Explorer, or Terrain Lab
  season rendering;
- a player-facing HUD calendar, named global seasons, or automatic date
  advancement;
- a new saved graphics preference or product-facing season setting; or
- public deployment or a showcase recipe.

## Stop Conditions

Stop and request a narrower follow-up decision if the proof requires:

- mutating blocks, lighting, persistence, or server simulation;
- rebuilding or re-uploading exact sections whenever the preview changes;
- adding more than the bounded static vertex/memory cost without comparing
  alternate representations;
- changing procedural-horizon/LOD code to make the exact-only proof look
  complete;
- a platform-private desktop or XR season owner;
- omitting per-eye or full-frame multiview from an XR-visible renderer;
- copying generator climate logic into the renderer;
- weakening existing texture, lightmap, fog, alpha, depth, color-transfer,
  topology-seam, or preview-off pixel contracts; or
- disguising a global color filter as regionally differentiated seasons.

## Code And Documentation Map

- `native/crates/mclone-mesh/src/builder.rs` — exact visible-face, biome-tint,
  light, and candidate static response derivation.
- `native/crates/mclone-mesh/src/tint.rs` — existing biome visual/climate and
  grass/foliage tint ownership to reuse rather than copy.
- `native/crates/mclone-mesh/src/data.rs` and `packed.rs` — exact mesh payload
  and any measured compact response encoding.
- `native/crates/mclone-render/src/chunk.rs` — exact mono/placed/multiview
  pipelines and per-view uniform ownership.
- `native/crates/mclone-render/src/grass.rs` — exact-associated lush-grass
  response.
- `native/crates/mclone-render/src/shaders/` — exact terrain/grass shader
  variants; procedural-horizon shaders are out of scope.
- `native/crates/mclone-ui/src/lib.rs` and `v2.rs` — typed Debug row, action,
  render state, and controller/pointer UI.
- `native/crates/mclone-app-runtime/src/client_experience.rs` — shared action
  classification, capability, temporary state, and setting effect.
- `native/crates/mclone-scene/` — shared mono/XR render-option application,
  diagnostics, and frame preparation.
- [`../topics/seasons.md`](../topics/seasons.md) — parent product and
  architecture direction.
- [`148`](148-xr-diagnostic-widget-panels.md) — shared Debug UI/XR diagnostic
  action precedent.
- [`304`](304-lod-frontier-and-near-field-voxel-convergence.md) and
  [`305`](305-fine-homestead-lod-overlay.md) — independent active LOD work that
  this tactical deliberately does not modify.
