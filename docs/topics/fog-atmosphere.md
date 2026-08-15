# Fog And Distance Atmosphere

Topic: `fog-atmosphere`

Status: active implementation. The first evaluation slice is defined below:
one shared Fog submenu, persisted experimental controls, linear, exponential,
and height-aware distance atmosphere, an explicit Off baseline, one
coverage-edge guard, matching exact/procedural presentation, conservative far
culling, and cross-platform rendered/performance evidence. The player will
evaluate the modes before one becomes a product default.

## Scope

This topic owns open-air distance atmosphere and its relationship to exact
render distance and procedural distant terrain:

- player-facing fog modes and interactive tuning controls;
- the shared per-view renderer contract and shader curves;
- fog color, sky, time-of-day, and future weather inputs;
- coverage-edge concealment with and without distant terrain;
- far-render culling which is safe only after atmosphere is effectively
  opaque;
- mono, per-eye, and multiview behavior;
- performance and rendered-output evaluation; and
- the decision boundary between an inexpensive product atmosphere and later
  volumetric effects.

Underwater, lava, powder-snow, blindness, and other gameplay/media visibility
effects are stronger environment overrides. Turning open-air Fog Off must not
turn those effects off. Vanilla weather state remains owned by
[`vanilla/weather.md`](vanilla/weather.md); the Graphics inventory and
preference lifecycle remain owned by
[`graphics-video-settings.md`](graphics-video-settings.md); procedural
residency and exact/LOD arbitration remain owned by
[`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md).

## Motivation

The procedural horizon can now project natural terrain roughly 140
kiloblocks from a composed flat or default per-eye XR view. Exact-only scenes
have a much shorter drawable frontier governed by chunk render distance.
Neither path should require perfectly clear air:

- a generous natural haze gives distant scale and depth;
- denser ground haze can make valleys and lowlands feel humid, smoky, or
  foggy while retaining high silhouettes;
- a final fade can conceal the finite drawable edge;
- the same controls must remain useful with distant terrain disabled; and
- Off must remain available so LOD seams, handoff problems, and real coverage
  limits cannot hide behind atmosphere during evaluation.

Fog is presentation, not terrain correctness. It must not replace stitched
clipmap seams, exact-painted coverage, stable vegetation ownership, or bounded
residency.

## Three Independent Distances

Do not collapse these controls:

1. **Exact Render Distance** selects expensive authoritative chunks and their
   render sections.
2. **Distant Terrain** enables or disables the procedural natural horizon.
3. **Atmospheric Visibility** controls optical contrast with distance.

The scene also resolves a fourth internal fact: conservative drawable
coverage for the current presentation. Exact-only coverage follows the usable
exact frontier; composed coverage follows the committed clipmap reach and
projection. Atmospheric visibility may be shorter than coverage. It may not
claim a clear view beyond coverage.

A narrow coverage guard therefore fades the final drawable interval even when
the selected natural atmosphere would remain clear. With Fog Off the guard is
also off so the mode remains an honest diagnostic. Linear mode consists
primarily of that guard. Natural and ground-haze modes combine their
atmospheric extinction with it.

## Evaluation Modes

The first menu exposes all four modes rather than prematurely selecting one:

| Mode | Optical model | Purpose | Expected fragment cost |
|---|---|---|---|
| Off | none in open air | clear diagnostic baseline | branch only |
| Classic | linear distance fade | hard-cutoff comparison | distance plus small ALU |
| Natural | homogeneous exponential extinction | smooth aerial haze without a visible start wall | distance plus one `exp2` |
| Ground Haze | exponential extinction modulated by a low atmospheric layer | smog, mist, and altitude-sensitive silhouettes | distance, height arithmetic, and up to two `exp2` operations |

### Off

Open-air fragments retain their shaded color. The sky remains normal and no
coverage guard is applied. Media/gameplay fog still applies.

### Classic

Classic uses the Minecraft Java 1.17.1 terrain shape as its reference:
ordinary terrain fog begins at `0.75 * far distance` and reaches full fog at
the far distance. Mclone generalizes the end point to conservative drawable
coverage so the mode remains meaningful with exact-only and composed terrain.
The menu exposes the start ratio for direct evaluation.

### Natural

Natural uses Beer-Lambert-shaped homogeneous extinction:

```text
transmittance = exp(-extinction * distance)
fog amount    = 1 - transmittance
```

The player-facing visibility value is the world distance at which retained
contrast reaches five percent. This definition gives the slider comparable
meaning across devices and avoids an arbitrary density coefficient in the UI.
The implementation may use `exp2` with a converted coefficient.

Natural begins gently near the viewer, preserves broad silhouettes, and
avoids the obvious radial wall of a late linear fade. It is the recommended
candidate for an eventual default, not the predetermined winner.

### Ground Haze

Ground Haze applies an exponential vertical density falloff around a
configurable base and falloff height, then integrates or conservatively
approximates density along the camera-to-fragment segment. It should:

- retain more visibility for an observer above the layer;
- thicken lowland and near-ground air;
- preserve mountain and tree silhouettes above dense lower air; and
- avoid camera-height discontinuities or a visible horizontal plane.

The initial implementation favors stable analytic arithmetic over noise or a
sampled volume. A later implementation may replace the approximation only
after matched pixel and performance evidence.

## First Fog Submenu

Graphics receives a nested **Fog** page. Evaluation takes priority over a
minimal final inventory, so the first page may be intentionally generous:

- Mode: Off / Classic / Natural / Ground Haze;
- Color: Sky Adaptive / Neutral / Warm / Cool / Custom RGB;
- Custom Red / Green / Blue sliders, enabled for Custom RGB;
- Visibility: logarithmic world-distance slider;
- Classic Start: fraction of effective coverage;
- Coverage Guard: Off / On;
- Guard Start: fraction of effective coverage;
- Ground Base: world Y at which the layer is densest;
- Ground Falloff: vertical distance over which density falls;
- Maximum Opacity: upper bound for natural atmosphere before the coverage
  guard;
- Double Exponential: Off / On, comparing homogeneous exponential extinction
  with the height-density extension/stronger curve;
- Far Cull: Off / On; and
- Weather Influence: Off / Subtle / Strong.

Rows which do not affect the selected mode remain visible but disabled or
clearly labeled. This makes comparisons repeatable without hiding parameters
or rearranging the page. The page must use the existing clipped scrolling and
controller/touch/XR focus behavior.

The persisted document stores typed, normalized values. Startup CLI/query
overrides remain launch-only and take precedence over stored values. Invalid
or future values must not make a client unlaunchable.

After evaluation, product settings may collapse to a smaller preset inventory.
The topic must record that decision instead of silently deleting the
experimental meanings.

## Color And Sky Contract

Open-air atmosphere fades world surfaces toward a horizon color derived from
the same shared sky facts as the background. It must not clear the entire
frame to one constant color:

- sky geometry, sunrise/sunset glow, and future clouds remain visible;
- distance color follows time of day;
- biome fog color and spatial blending can join when those renderer facts are
  available;
- rain and thunder can darken/desaturate the shared atmosphere input after the
  client replica exposes smooth levels; and
- color mixing occurs before the target-color transfer, matching exact-world
  shading.

Minecraft Java 1.17.1 is the reference baseline. Its ordinary fog color blends
biome fog and sky color, reacts to rain and thunder, and uses separate sky and
terrain fog ranges. Mclone's Natural and Ground Haze modes are explicit
non-vanilla presentation extensions.

Media fog is different. Underwater currently clears the distant background to
its medium color because open-air sky is not visible through that path.
The resolved per-view contract needs an explicit background policy rather than
using one `enabled` bit to mean both open-air atmosphere and opaque media.

## Shared Renderer Contract

`mclone-scene` resolves one atmosphere value per physical view from:

- stored live settings;
- camera position;
- current sky/time facts;
- effective exact/composed coverage;
- future smooth weather facts; and
- any stronger media/gameplay override.

The renderer-neutral value is passed through the ordinary frame compositor to:

- exact opaque, cutout, and translucent chunk shaders;
- grass;
- prepared and runtime actors;
- procedural terrain;
- procedural tree proxies; and
- future world-space particles, clouds, water, and overlays as applicable.

Mono and each XR eye receive the same policy with their own camera facts.
Existing multiview shaders must use each layer's view/camera data; no mutable
per-eye uniform may be shared within one submission. The procedural horizon
still lacks a full-frame multiview pipeline, which remains an explicit
pre-existing availability gap rather than permission for fog to diverge.

Keep one generated or source-shared WGSL fog function where practical. The
current exact shaders duplicate linear fog logic, so every new curve must not
be independently hand-maintained across chunk, actor, grass, placed, stereo,
and multiview variants.

## Coverage Guard And Far Culling

Atmosphere adds fragment work by itself. It becomes a performance feature only
when effectively opaque distance allows earlier work rejection.

The first culling rule is conservative:

1. derive the nearest distance at which combined atmosphere plus coverage
   guard reaches the configured cull-opacity threshold;
2. keep a safety margin for tall geometry, wide actors, stereo eye separation,
   and numerical/color error;
3. cull only whole draw records whose conservative bounds lie beyond that
   reach;
4. never cull translucent or emissive behavior whose contribution is not
   actually bounded by the same contract; and
5. expose a diagnostic count of atmosphere-culled records.

Far Cull Off preserves identical geometry for shader-cost comparisons. Far
Cull On demonstrates possible render savings. The first slice does not change
exact chunk loading, authoritative interest, procedural residency, or Worker
generation. Later evidence may justify omitting fully invisible coarse
clipmap draws or preparation, with hysteresis so weather and slider motion do
not churn residency.

Maximum Opacity below one disables atmosphere-only culling. The coverage guard
may still authorize culling beyond its fully opaque end.

## Weather

Vanilla weather is global clear/rain/thunder state with position-local
precipitation. Fog is not a separate vanilla weather event. Preserve that
boundary:

- weather supplies smooth presentation inputs to atmosphere;
- Subtle and Strong may multiply extinction and darken/desaturate the
  atmosphere color;
- optical values may interpolate every frame;
- work/residency decisions need hysteresis or a conservative stable reach; and
- non-vanilla fog banks, smoke, pollution, seasons, or biome-local mist must
  be explicit atmosphere systems, not hidden inside the vanilla rain cycle.

Until live smooth rain/thunder facts reach the renderer, Weather Influence is
a stored policy with neutral effective input. Developer/test overrides may
exercise the resolver without inventing authoritative weather state.

## Rejected First-Pass Techniques

### Full-screen depth fog

A post-process would give one implementation across opaque renderers, but it
adds a full color/depth read and color write, handles transparent geometry
poorly, complicates multisample and reversed-Z reconstruction, and adds
stereo/multiview bandwidth. Keep it as a later measured comparator, not the
first shared path.

### Ray-marched volumetric fog

Ray marching, froxel volumes, shadowed shafts, and local density lights require
multiple samples, history/reprojection, and larger XR/mobile budgets. They are
not needed to choose a distance atmosphere.

### Animated 3D noise

Noise can make banks and clouds, but texture sampling or procedural octaves,
temporal shimmer, eye disagreement, and swimming world-space patterns obscure
the initial curve comparison. Add it only as a separate atmospheric-volume
feature.

## First Implementation Sequence

1. Add the shared settings types, Fog submenu, typed actions/effects,
   persistence, normalization, and focused UI/controller tests.
2. Replace the underwater-only boolean shape with a resolved atmosphere/media
   contract while preserving existing underwater pixels and tests.
3. Implement Off, Classic, and Natural in one shared shader function across
   exact chunks, actors, and grass.
4. Add Ground Haze, Double Exponential, opacity, and coverage-guard parameters.
5. Pass the same resolved contract to procedural terrain and tree proxies.
6. Add conservative draw-record far culling and observable counts without
   changing loading or clipmap residency.
7. Capture and inspect pixels at the first drawable milestone and after every
   added curve/culling step.
8. Record matched GPU/frame, CPU, draw-count, and resident-memory evidence
   before choosing a default or using fog to reduce prepared work.

## Implemented Evaluation Pass

The first evaluation pass landed on 2026-07-28:

- Graphics > Fog exposes all modes, sky/neutral/warm/cool/custom RGB color
  choices, custom channels, visibility, linear onset, coverage guard, ground
  shaping, maximum opacity, exponential-squared, far cull, and weather policy.
- Settings normalize and persist through the host-neutral graphics document.
- One renderer-owned WGSL function evaluates linear, homogeneous
  exponential, exponential-squared, and height-shaped ground haze for chunks,
  grass, runtime and prepared actors, procedural terrain, and tree proxies.
  Mono, stereo, placed-world, and multiview shaders use the same function.
- Open-air fog keeps the sky pass and derives its recommended color from that
  sky. Underwater fog remains an opaque linear media override.
- Exact coverage uses the loaded chunk-corner distance. Composed coverage uses
  the clipmap's conservative reach. The guard reaches full opacity only at that
  boundary.
- Far Cull is opt-in. Exact-section and procedural-tile submission rejects
  geometry only after a conservative opaque boundary. A natural haze capped
  below full opacity cannot authorize atmosphere-only culling, and
  height-dependent Ground Haze can cull only behind its coverage guard.

The weather choice is persisted but currently has no live density input:
the client replica does not expose smooth rain/thunder levels yet. Keep the
row as an explicit future policy rather than faking weather in the renderer.

Validated evidence:

- shared renderer, terrain-view, runtime, and UI unit/integration tests;
- `/tmp/mclone-desktop-offscreen.png`, inspected recommended Natural
  exact-only output;
- `/tmp/mclone-fog-composed.png`, inspected recommended Natural composed
  output;
- `/tmp/mclone-fog-probe.png`, inspected aggressive custom-color low-visibility
  output across exact terrain, procedural hills, and procedural tree proxies;
  and
- `/tmp/mclone-xr-emulation.png`, inspected distinct left/right output.

No matched performance conclusion has been made. Existing section/tile draw
counts make Far Cull observable, but the decision gate still requires matched
timing with identical settings and geometry on representative desktop, mobile,
browser, and XR targets.

## Validation Matrix

The acceptance matrix crosses:

- exact-only and composed distant terrain;
- Off, Classic, Natural, and Ground Haze;
- Far Cull Off and On;
- low ground, valley, mountain, and high-altitude viewpoints;
- noon, dawn/dusk, and night;
- dry plus synthetic/live rain and thunder inputs when available;
- stationary, ordinary movement, fast flight, and teleport;
- native flat, headed browser/WebGPU, flat Android, synthetic stereo, desktop
  OpenXR where available, and Quest default per-eye; and
- underwater entry/exit to prove media override precedence.

Inspect:

- exact coverage edge and exact/procedural handoff;
- clipmap rings, water, and distant tree proxies;
- horizon-to-sky color continuity;
- sunrise/sunset preservation;
- opacity response while sliders move;
- stereo consistency and head-motion stability;
- transparent terrain/actor behavior; and
- whether Off exposes real defects hidden by other modes.

Measure identical geometry with Far Cull Off first to isolate shader cost.
Then compare Far Cull On using drawn record/section/proxy counts and GPU/frame
timing. Fog-colored screenshots are not performance evidence. Captures remain
under `/tmp`.

## Initial Code Map

- `native/crates/mclone-render/src/fog.rs` — renderer-neutral atmosphere/media
  contract, safe cull boundary, and shared WGSL injection.
- `native/crates/mclone-render/src/chunk.rs` and
  `native/crates/mclone-render/src/shaders/` — exact world, actor, grass,
  mono/stereo/multiview uniforms and the shared fog function.
- `native/crates/mclone-app-runtime/src/frame_render.rs` — sky/background,
  media override, frame composition, and terrain-backdrop handoff.
- `native/crates/mclone-scene/` — shared settings effects, coverage facts,
  per-view atmosphere resolution, diagnostics, and composed terrain ownership.
- `native/crates/mclone-terrain-view/` — procedural terrain/tree uniforms,
  shaders, and conservative level/draw boundaries.
- `native/crates/mclone-app-runtime/src/graphics_preferences.rs` — persisted
  machine-local graphics document.
- `native/crates/mclone-ui/src/lib.rs` and `src/v2.rs` — typed player values,
  Fog submenu, scrolling, and interaction.
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/FogRenderer.java`
  — optional color and range comparison specimen.

## Decision Gate After Evaluation

The first implementation deliberately does not choose the final product
inventory. The next decision should use inspected all-target pixels and
matched performance:

- choose Off, Classic, Natural, or Ground Haze as the default;
- retain advanced sliders, collapse them into quality/visibility presets, or
  move them to a developer panel;
- decide whether weather controls density only or can select local fog events;
- decide whether culling savings justify tighter integration with clipmap draw
  levels or preparation; and
- decide whether any post-process or volumetric experiment answers a visible
  gap that the analytic modes cannot.
