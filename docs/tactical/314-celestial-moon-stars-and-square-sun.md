# Tactical 314: Celestial Moon, Stars, And Square Sun

Status: implemented 2026-08-16; automated and physical-XR evidence complete,
with staged Human Review still open for final pixels and the measured Quest
star-cost exception

Topic: `seasons`

## Instruction Synthesis

Complete the ordinary Mclone sky with one square pixel-art moon, a bounded
latitude-aware star field, and an original-Mclone square sun. Keep the accepted
Earth-like `0.53`-degree apparent size for the solid sun core; do not recover
the previously rejected oversized sun. A separately rendered subtle halo may
extend beyond that core without changing its measured body size.

Use one coherent celestial-coordinate model. Sun, moon, and stars consume the
same global orbital phase, observer-root effective latitude, and solar-time
conventions when Seasonal Solar Preview is active. The moon has continuous
orbital/illumination state with eight named phase labels. Stars rotate around
the correct celestial pole, change orientation with latitude, and fade from
actual solar elevation so polar day and polar night work without clock-window
special cases.

Add one shared `Celestial Debug` screen with independent live controls for the
sun body, sun halo, horizon glow, moon body, rendered moonlight, and stars.
Stars must support `Off`, `Quarter`, `Half`, and `Full` density. Disabled
features skip their draw and per-frame write work so native, WebGPU, desktop
OpenXR, and Android XR can measure real incremental cost. Keep these controls
client-local and unsaved; they must not mutate authoritative time, lighting,
weather, spawning, entities, blocks, persistence, or unloaded regions.

Complete Tactical [`036`](036-native-sky-and-day-night-cycle.md)'s retained
Java 1.17.1 moon/star baseline while making the original Mclone appearance an
explicit divergence. Retained reference profiles preserve Java's oversized
sun/moon, eight-frame moon atlas, deterministic star field, celestial rig, and
brightness law. Do not make the original square bodies or cyclical-latitude
cosmology leak into those profiles.

## Starting Point

The shared sky renderer already provides the right host-neutral foundation:

- `mclone-render::sky_render::SkyRenderer` draws the sky disc, directional or
  vanilla horizon glow, and one textured sun quad before world geometry;
- the sun quad already supports mono, placed/per-eye, synthetic stereo, and
  full-frame multiview paths with per-view uniform slots;
- the visible sun direction, sky color, glow, fog input, and rendered daylight
  consume one `SkyRenderState` selected by `mclone-scene`;
- original Mclone fixed and seasonal paths use an accepted `0.53`-degree sun,
  while retained Java-shaped worlds use the approximately `33.398`-degree
  Java quad;
- Tactical [`307`](307-seasonal-solar-path-and-cyclical-latitude.md) supplies
  the shared solar sample and cyclical-plane/asymptotic-cylinder latitude
  policies;
- Tactical [`306`](306-seasonal-appearance-preview.md) separates `Solar
  Preview` from `Ground Appearance`, so celestial review does not require
  seasonal terrain tint or snow;
- the server owns tick-derived `game_time` and `day_time`, and clients already
  receive the authoritative clock;
- the current original fallback `sun.png` is generated as a soft circular
  alpha disc even though its renderer geometry is a quad;
- there is no moon render resource, lunar sample, star catalog, star shader,
  moonlight input, celestial Debug screen, or feature-specific sky cost
  receipt; and
- Tactical 036's open Phase 3 still lists the Java 4-by-2 moon atlas and 1,500
  attempted deterministic star quads.

Minecraft Java 1.17.1 remains the retained-profile reference:

- `LevelRenderer.renderSky` draws the sun quad, opposite moon quad selected
  from `moon_phases.png`, then the static star buffer;
- `DimensionType.moonPhase(day_time)` advances one of eight frames per 24,000
  ticks;
- `LevelRenderer.drawStars` uses seed `10842`, 1,500 candidate directions,
  and static quads; and
- `ClientLevel.getStarBrightness` fades the field from the smoothed vanilla
  time-of-day curve and suppresses it under rain.

That baseline is intentionally simple and visually valuable. Original Mclone
may diverge toward continuous lunar geometry and real latitude-aware stellar
motion, but must retain an exact, testable reference branch rather than
silently rewriting it.

## Objective

At one fixed world/camera with ground appearance disabled, the user can open
`Celestial Debug` and independently inspect:

- a crisp square original-Mclone sun whose solid side measures `0.53` degrees;
- a subtle halo and the existing horizon glow as distinct, separately
  disableable effects;
- a square moon whose position, waxing/waning state, illuminated fraction,
  and lit-limb orientation agree with the sun;
- eight readable phase labels over one continuous lunar phase;
- a static, culturally stable Mclone star catalog that rotates without
  per-frame regeneration;
- equatorial, temperate, and polar star paths under the accepted flat-world
  latitude law;
- date-dependent constellation orientation at equal local solar time;
- star and moon visibility during polar night, and their suppression during
  midnight sun; and
- exact feature-off restoration with measured CPU, GPU, draw, write, and
  resource costs for each incremental celestial layer.

The product result is a physically coherent sky expressed through a block-world
visual language. It is not an astronomical simulator, a new world topology, or
an authoritative gameplay calendar.

## Binding Decisions

### The sun and moon are sky quads, not nearby world spheres

Do not add large world-space sphere meshes, translation parallax, collision,
chunk ownership, or floating-origin behavior. Astronomical bodies are
effectively at infinity and use camera-translation-free sky projection.

The current sun is already a quad. Its circular appearance comes from alpha in
the selected or generated texture. For original Mclone:

- the solid sun silhouette is a square pixel-art core;
- its side subtends `0.53` degrees in both fixed and seasonal solar paths;
- angular size is measured by side, not diagonal;
- texel filtering and antialiasing must keep it stable on desktop and at Quest
  per-eye pixel densities without inflating its solid silhouette;
- its celestial-plane basis is stable in world sky coordinates rather than
  being a permanently screen-aligned HUD square; and
- an optional soft halo has its own opacity/extent and Debug enable, and never
  counts toward the accepted solid-body diameter.

The moon uses a `0.52`-degree square outer silhouette for original Mclone. Its
pixel-stepped terminator may curve inside that square, but the full-moon outer
limb remains visibly square. Human Review must compare the full, crescent, and
quarter silhouettes at native desktop and headset resolution before binding
the final pixel grid.

Retained Java-shaped profiles keep their current sun texture, approximately
`33.398`-degree sun side, approximately `22.620`-degree moon side, and Java
celestial-plane orientation. Selected asset packs may supply compatible
colors/texels, but the dimension sky profile owns the body shape and angular
law. Missing original assets resolve to bounded proprietary-free square
fallbacks; missing retained-reference assets retain an explicit provenance
fallback without borrowing original Mclone semantics.

### One continuous lunar phase has eight named labels

Add fixed-point dependency-leaf vocabulary in `mclone-season` equivalent to:

```text
LunarPhase: cyclic [0, 1)

0.000  New Moon
0.125  Waxing Crescent
0.250  First Quarter
0.375  Waxing Gibbous
0.500  Full Moon
0.625  Waning Gibbous
0.750  Last Quarter
0.875  Waning Crescent
1.000  wraps to New Moon
```

The labels are display landmarks, not discrete simulation states. Direction,
illumination, and terminator orientation remain continuous across every label
and the wrap. At representative anchors:

```text
illuminated_fraction = (1 - cos(TAU * lunar_phase)) / 2
waxing = lunar_phase in (0, 0.5)
waning = lunar_phase in (0.5, 1)
```

Use a provisional `29.5`-day visual synodic cycle (`59/2` Minecraft days) when
`Moon Phase Source: World Clock` is selected. Derive it as a pure read of
authoritative cumulative `day_time`; do not add a moon tick, saved lunar state,
or catch-up work. Record an explicit phase origin so fixture results are
stable. This is a presentation cycle, not selection of the future season year
length or a promise that lunar gameplay uses the same period.

`Manual Preview` exposes the same continuous phase through Debug without
mutating `day_time`. Retained Java profiles keep `day_time / 24000 mod 8` and
the reference atlas ordering when their reference mode is active.

### Moon position and phase come from one orbit

Do not make the moon an arbitrary texture opposite the sun while separately
sliding its phase. Place both behind one pure shared sample, equivalent to:

```text
LunarInput {
    orbital_phase,
    lunar_phase,
    effective_latitude,
    solar_time_fraction,
    orbital_inclination,
    node_phase,
}

LunarSample {
    direction,
    elevation,
    elongation,
    illuminated_fraction,
    waxing,
    lit_limb_tangent,
    named_phase,
    moonlight_factor,
}
```

The moon's elongation from the sun advances with lunar phase. A new moon stays
near the sun, a full moon stays approximately opposite it, and quarter moons
occupy intermediate paths. Project the sun direction into the moon's tangent
plane to orient the illuminated limb; do not hard-code left/right crescents in
screen space. Northern/southern and rising/setting views must therefore rotate
the apparent phase coherently.

Use one restrained roughly Earth-like orbital inclination, initially five
degrees, so conjunction does not imply a visible eclipse every month. This
tactical does not render solar/lunar eclipses, node precession, libration,
distance-driven angular-size change, solar phases, or multiple moons. The
no-eclipse rule is explicit rather than an accidental consequence of bad
geometry.

### Stars live in one stable celestial catalog

Original Mclone uses one world-independent catalog in equatorial coordinates.
Do not seed constellations from the world seed, camera, chunk, session, or
calendar. A stable sky can support navigation, stories, and later ecology.

Start with a hard-bounded catalog of at most 4,096 stars. Prefer approximately
2,048 entries at full density, including a sparse set of manually authored
constellation anchors plus deterministic filler. Each static entry needs only
direction/right ascension, declination, size class, brightness class, and a
small color class. Keep the full GPU-resident catalog under 128 KiB unless
measurement justifies more.

Render one instanced square or compact quad family in one draw. Construct and
upload the catalog only when the renderer/resource epoch changes. Do not:

- hash the full screen per fragment to discover stars;
- regenerate or sort the catalog each frame;
- allocate one object or issue one draw per star;
- introduce camera-position parallax;
- apply aggressive temporal twinkle; or
- let star work scale with chunks, render distance, terrain LOD, or world
  extent.

Retained Java mode reproduces seed `10842`, its accepted candidate rejection,
quad size range, fixed buffer, and brightness curve. It may use the same shared
GPU mechanism if exact generated directions, sizes, orientation, and draw
ordering remain fixture-locked.

### Latitude controls the celestial pole without inventing longitude

Store original Mclone stars in global equatorial coordinates and transform
them to the observer's local horizon from:

```text
effective latitude
  + local sidereal angle
  + global orbital phase
  -> local star direction
```

Local sidereal angle advances once per solar day and receives the expected
annual offset, so a constellation appears at different solar times across the
orbital year. The exact sign/zero conventions must be pinned against visual
fixtures rather than inferred independently in shaders.

Reuse Tactical 307's scalar effective latitude and no-time-zone decision:

- equal latitudes on opposite sides of a polar crest see equal star laws;
- crossing a crest does not flip the compass frame or add twelve hours;
- travel along X does not create longitude or spatial time-of-day change;
- the equator puts celestial poles on the horizon;
- high latitudes produce circumpolar stars; and
- a polar crest puts the appropriate celestial pole overhead.

Resolve one observer-root celestial sample. Both XR eyes consume the same
directions and phase state with their own view/projection matrices; never
derive latitude, sidereal angle, or moon phase separately per eye.

When `Solar Preview` is off, original Mclone's existing fixed sky remains the
baseline and uses a fixed-rig celestial transform. Latitude/orbital star and
moon motion becomes active with the same seasonal solar evaluation that moves
the sun. Retained reference profiles always use their reference rig. This
avoids a latitude-aware moon orbit disagreeing with a fixed sun.

### Visibility comes from celestial conditions

Do not show stars during a hard-coded `18:00..06:00` interval. Compute their
base visibility from the same sun elevation/daylight/twilight sample that owns
the sky:

- stars smoothly emerge through evening twilight and fade through dawn;
- polar night permits stars during the ordinary clock's nominal daytime;
- midnight sun suppresses them despite a nominal midnight clock;
- the moon below the horizon contributes no visible body or moonlight;
- a bright visible moon modestly suppresses the dimmest nearby/global stars;
  and
- future rain/cloud opacity may attenuate all bodies through one atmosphere
  input, while this tactical retains the existing clear-weather baseline.

Rendered moonlight is deliberately restrained. It may lift the visual
night-sky and material illumination enough for full/new moon distinction, but
it must not make full-moon night resemble daylight or erase block-light
contrast. It consumes illuminated fraction and elevation from `LunarSample`.

Moonlight remains presentation-only. It does not change propagated sky light,
the light graph, mob spawning, sleep, crop growth, animal schedules, weather,
or any authoritative gameplay query.

### Celestial controls are independent, shared, and temporary

Add one controller-friendly `Celestial Debug...` entry to the shared Debug
options category. Do not crowd the existing `Seasonal Debug` screen or add
desktop/XR-specific shortcuts. The nested screen owns at least:

- `Sun Body: On | Off`;
- `Sun Halo: On | Off`;
- `Horizon Glow: On | Off`;
- `Moon Body: On | Off`;
- `Moonlight: On | Off`;
- `Moon Phase Source: World Clock | Manual Preview`;
- `Moon Phase`, a continuous cyclic slider when manual;
- `Stars: Off | Quarter | Half | Full`;
- read-only named phase and illuminated percentage;
- read-only effective latitude and sidereal angle; and
- read-only visible/submitted star counts plus celestial draw/write counters.

Use one typed settings/action/effect family, such as:

```text
CelestialDebugSettings {
    sun_body_enabled,
    sun_halo_enabled,
    horizon_glow_enabled,
    moon_body_enabled,
    moonlight_enabled,
    moon_phase_source,
    manual_lunar_phase,
    star_density,
}

GameUiAction::SetCelestialDebug(settings)
ClientExperienceSettingEffect::SetCelestialDebug(settings)
```

All product visual layers default on when implemented; phase source defaults
to `World Clock`, and full star density is the target quality. The Debug
settings are reset on process/session construction and are never written to
world records or preferences. A later measured accessibility/graphics setting
may expose star density, but this tactical adds only review/performance state.

Stable CLI/smoke controls must set every field without pointer automation, for
example `--celestial-sun`, `--celestial-sun-halo`,
`--celestial-horizon-glow`, `--celestial-moon`,
`--celestial-moonlight`, `--celestial-stars`,
`--moon-phase-source`, and `--moon-phase`. Final names may follow existing CLI
style, but diagnostics must report exact typed values and not scrape menu text.

### Off means no submitted feature work

Feature toggles are cost controls, not opacity controls. For every disabled
layer:

- issue no feature draw;
- write no feature-specific per-frame vertex, instance, or uniform payload;
- run no per-star CPU transform;
- dispatch no compute work;
- emit no hidden moonlight material adjustment; and
- report zero submitted instances/indices for that feature.

The sky disc/clear remains because it owns the frame background. A live-off
toggle may retain small static buffers and pipelines for instant re-enable;
diagnostics must separately report their exact CPU/GPU resident bytes. If cold
startup can cheaply avoid allocating disabled optional resources, measure that
path separately rather than coupling it to live interaction.

Star density selects a deterministic prefix or stable brightness-stratified
subset of one catalog. `Quarter`, `Half`, and `Full` must nest exactly so A/B
pixel and cost comparisons differ only by submitted stars. Density changes do
not rebuild or upload a new catalog.

### Shared renderer ownership includes multiview from the first pixel

Extend `mclone-render::sky_render` rather than building moon/stars in an app.
Every new buffer, pipeline, shader, uniform, and draw path must support:

- ordinary mono;
- placed/embedded mono where the sky is intentionally present;
- per-eye stereo with independent view/projection slots;
- synthetic stereo capture; and
- full-frame two-layer multiview.

The scene supplies one immutable celestial frame sample; renderers consume it.
OpenXR, winit, Android, and browser hosts remain target/session adapters. Do
not add `winit` or OpenXR dependencies to shared sky or scene ownership.

## Implementation Slices

### Slice 0: Baseline, assets, and cost receipts

Before changing pixels:

- capture current original Mclone fixed/seasonal sun at noon, twilight, and
  polar conditions in mono and synthetic stereo;
- capture retained Java-profile sun pixels and receipt its texture source and
  angular size;
- record current sky draw count, per-frame writes, CPU encode time, GPU time,
  and renderer resource bytes;
- add typed celestial visibility/density settings, diagnostics, and CLI smoke
  plumbing without yet changing default output; and
- prove all new disabled fields preserve the exact pre-slice screenshot.

Gate: the implementation can attribute existing sky cost and express every
later A/B state through typed shared controls before moon/star complexity
lands.

### Slice 1: Pure lunar and sidereal model

Implement in `mclone-season`:

- fixed-point `LunarPhase`, eight typed labels, and exact wrap;
- provisional 29.5-day world-clock projection;
- continuous illumination and waxing/waning classification;
- moon celestial direction, elevation, elongation, limb tangent, and
  moonlight factor;
- stable equatorial-to-local-horizon star transforms;
- local sidereal angle from solar time and orbital phase; and
- finite/clamped input validation.

Table-driven tests cover every named phase, intermediate continuity, wrap,
world-clock anchors, new/full opposition, quarter separation, five-degree
inclination, northern/southern limb orientation, equal-latitude polar
shoulders, equator/temperate/pole star paths, annual sidereal offset, polar
day/night visibility, and non-finite rejection.

Gate: pure tests can explain every moon/star direction and brightness without
loading a world, GPU, texture, UI, or authoritative simulation service.

### Slice 2: Square original sun and separable halo

- Add or promote the original first-party square sun asset through the existing
  pack/provenance pipeline and make the proprietary-free generated original
  fallback square.
- Keep reference sun asset selection and pixels unchanged.
- Preserve exactly `0.53` degrees for the original solid side.
- Add a bounded soft halo as a separate render contribution or clearly
  isolated shader region with independent enable and cost counters.
- Keep horizon glow separately disableable.
- Add shader/CPU geometry fixtures for side length, celestial basis, opacity,
  mono/multiview parity, and no submitted work when disabled.

Capture and inspect the first square-sun pixel before continuing. Gate: Human
Review accepts the square identity at desktop and XR scale and confirms that
the halo has not made the body feel oversized again.

### Slice 3: Continuous square moon

- Add original and retained-reference moon assets with explicit source and
  fallback receipts.
- Render the original square moon from `LunarSample`, including continuous
  pixel-stepped terminator, lit-limb orientation, horizon visibility, and
  `0.52`-degree side.
- Render retained Java mode from the exact 4-by-2 atlas frame ordering and
  opposite fixed rig.
- Route optional visual moonlight through the same material/daylight owner as
  other presentation-only sky darkening, with no light-engine mutation.
- Support mono, per-eye, synthetic stereo, and multiview in the initial moon
  commit.

Capture New, both Crescents, both Quarters, both Gibbous phases, Full, and at
least one intermediate between every pair. Gate: position and limb orientation
remain coherent while scrubbing continuously and turning the moon off removes
its draw, write, and moonlight contribution exactly.

### Slice 4: Bounded celestial star field

- Add the fixed Mclone catalog and exact retained Java generator fixtures.
- Upload one static catalog per renderer/resource epoch.
- Render square stars through one bounded instanced draw in mono and
  multiview-aware paths.
- Apply shared horizon, twilight, solar-elevation, and moon visibility factors
  without CPU per-star updates.
- Make density states exact nested subsets and skip the draw entirely at
  `Off`.
- Expose catalog count, selected count, visible approximation/submitted count,
  buffer bytes, draw count, per-frame writes, and resource generation.

Capture equator, northern/southern temperate, and both polar regimes at two
orbital dates and several solar times. Gate: stars move as a celestial sphere,
not a camera effect; polar behavior reads correctly; no twinkle or subpixel
instability causes stereo discomfort.

### Slice 5: Shared Celestial Debug interaction

- Add the shared Debug navigation row and nested screen.
- Route every setting through `mclone-ui`, `ClientExperienceController`, and
  `mclone-scene` into one shared render-state owner.
- Make conditional rows explicit: manual phase is enabled only in Manual
  Preview; moonlight is enabled only when the moon model is active; the star
  density selector remains operable when its current value is `Off`.
- Add keyboard, controller, pointer, focus, scroll/layout, effect-dispatch,
  direct-screen, CLI, browser observer, and XR routing tests.
- Show global date, local latitude, solar time, lunar label/illumination, and
  submitted star/draw counters together without claiming an authoritative
  gameplay calendar.

Gate: desktop flat and one XR session can toggle each cost layer, scrub the
moon, change star density, and return to the exact all-off state without world
reload or menu drift.

### Slice 6: Cross-platform pixels and performance

Add one deterministic capture runner under `/tmp` with a receipt containing:

- exact revision, profile, seed, topology, camera, render path, and resolution;
- authoritative `game_time`/`day_time` plus manual overrides;
- solar orbital phase, effective latitude, solar time/elevation;
- lunar phase/source/label/direction/elevation/illumination/limb orientation;
- star catalog/density/submitted count and sidereal angle;
- enabled state for sun, halo, horizon glow, moon, moonlight, and stars;
- per-feature draw/instance/index/write counts and resource bytes;
- section mesh/build/upload/block/light/persistence counters proving no world
  churn; and
- frame CPU/GPU samples for the selected A/B lane.

Run an alternating, stationary, fixed-pixel matrix:

```text
sky disc only
+ square sun
+ sun halo
+ horizon glow
+ moon body
+ moonlight
+ stars quarter
+ stars half
+ stars full
full celestial -> all optional layers off -> exact baseline restoration
```

Repeat the meaningful boundaries on native desktop, headed WebGPU, synthetic
stereo, Android/flat build, and Android XR build. On available hardware, run
desktop OpenXR and Quest per-eye/full-frame-multiview pixels and performance.

For steady state:

- sun and moon each add at most one ordinary draw unless batching proves
  measurably better;
- the full star field adds at most one draw;
- star CPU work has no per-star frame loop, allocation, sort, or upload;
- disabled features add zero submitted draws and feature writes;
- exact terrain mesh/build/upload, block/light update, and persistence counts
  remain unchanged; and
- no cost grows with render distance, loaded chunks, climate wavelength, or
  star catalog history.

If full stars regress representative Quest app-GPU p95 by more than `0.20 ms`
or two percent, whichever is larger, stop and attribute fill, blending,
instance bandwidth, overdraw, and multiview behavior before acceptance. Also
stop if any optional layer introduces repeat per-frame allocation/upload or a
new CPU tail above `0.10 ms` p95 in the focused sky encode attribution.

Human Review must accept:

- square sun identity without recovering the oversized body;
- readable square moon phases and correct limb direction;
- restrained full-moon illumination versus genuinely dark new-moon night;
- recognizable but non-distracting constellations;
- equatorial/temperate/polar motion under the flat-world law;
- comfortable stereo stability; and
- no visible snap at phase, day, orbital-year, or cyclical-latitude wraps.

## Execution Record

Implemented on 2026-08-16 as the `Topic: seasons` series from `fdea647c`
through `aae3dfe1`.

- `mclone-season` now owns the fixed-point 29.5-day lunar cycle, eight labels,
  continuous illumination, waxing/waning state, inclined orbit and limb
  direction, local sidereal transform, and solar-elevation star visibility.
- `mclone-render` now draws the original `0.53`-degree square sun plus a
  separate halo, a continuous `0.52`-degree square moon, and one stable
  2,048-star catalog. The retained Java profile keeps its oversized sun,
  4-by-2 atlas phases, seed-10842 candidate stream, 780 accepted stars, and
  Java brightness law.
- Mono, per-eye, synthetic-stereo, and full-frame-multiview paths share the
  implementation. Stars use one indexed instanced draw; invariant transforms
  are precomputed, invisible stars are rejected before rasterization, and no
  per-frame CPU star regeneration, allocation, sort, or upload exists.
- The shared Debug navigation now exposes `Celestial Debug` on desktop, Web,
  flat Android, desktop XR, and Android XR. It independently controls sun,
  halo, glow, moon, moonlight, phase source, and nested star density, while
  showing global date, evaluated latitude, solar time, lunar phase, sidereal
  angle, and exact draw/write/resource receipts.
- All-off performs zero optional celestial draws and feature writes. The final
  deterministic matrix restored the sky-only SHA-256 exactly to
  `eac7448a135a069fb1e9caa0f24639ef7025021e9eabf5d393c15354deffc8b8`.
  Full density submits 2,048 stars in one draw. The original star buffer is
  73,728 bytes; total resident celestial resources, including both profile
  catalogs, textures, vertices, indices, and uniforms, are 143,304 bytes.
- The final 31-capture matrix at revision `aae3dfe18dc5` covers the incremental
  cost ladder, sixteen continuous-phase samples, retained Java, synthetic
  stereo, and all-off restoration. Native renderer tests report 206 passed
  and 11 hardware-only ignored tests. The headed WebGPU build and app-loop
  smoke pass with inspected pixels, and final flat Android and Android-XR APKs
  build through the repository scripts.
- A physical Quest full-frame-multiview session exercised the direct Debug
  screen and every launch override. The device held 72 Hz with no dropped
  frames and roughly 6 ms of app headroom.

The Quest star measurement crossed the tactical's review trigger. A paired
final 2,048-star observation measured about `4.389 ms` app GPU versus
`4.130 ms` all-off, or `+0.259 ms`. A longer five-sample attribution run at a
temporary 1,536-star density measured `+0.286 ms` mean. Quarter density cost
essentially the same as Full, identifying the remaining cost as the fixed
blended multiview submission rather than catalog bandwidth or CPU star work.
Indexed quads, invariant-transform precomputation, and pre-raster visibility
culling did not reduce that fixed cost. Full 2,048-star quality was therefore
restored instead of accepting a visual downgrade that did not buy performance.
Human Review must explicitly accept this measured exception or request a later
sky-compositing/batching slice; it is not concealed as a passed `0.20 ms`
budget.

## Explicit Deferrals

This tactical does not implement:

- authoritative season/year state or a player-facing calendar;
- eclipse rendering, eclipse gameplay, node precession, or libration;
- tides, lunar crops, spawning rules, sleep rules, animal schedules, or magic
  moon events;
- multiple moons, comets, meteors, aurora, planets, or a Milky Way layer;
- weather clouds, rain attenuation, thunder darkening, or lightning flashes
  beyond retaining compatible inputs;
- temporal star twinkle beyond a possible later restrained accessibility-safe
  effect;
- propagated moonlight or light-graph updates;
- sky reflections in water;
- procedural-horizon seasonal appearance; or
- a persisted player Graphics setting for star density.

Each is a later tactical only after the base sky pixels and costs are accepted.

## Completion Gate

Tactical 314 is complete only when:

1. Original Mclone has an accepted `0.53`-degree square sun and separately
   disableable halo without changing retained Java sun behavior.
2. One continuous square moon produces coherent position, eight labels,
   illumination, and limb orientation in fixed and seasonal sky paths.
3. One bounded fixed star catalog produces correct equatorial, temperate, and
   polar motion and solar-elevation visibility.
4. Retained Java mode passes exact sun/moon/star generation, atlas, brightness,
   and draw-order fixtures against 1.17.1 source.
5. Mono, per-eye, synthetic stereo, and full-frame multiview own complete
   celestial paths with per-view projection correctness.
6. Independent Debug toggles and density levels skip real draw/write work,
   report exact resource and submission costs, and restore the all-off baseline.
7. Native and headed WebGPU pixels are inspected, Android/Android-XR builds
   pass, and available physical XR lanes complete visual/performance review.
8. No server clock, gameplay light, block/entity state, persistence, terrain
   mesh, unloaded region, or procedural-horizon behavior changes.

All engineering gates and available platform lanes are complete. Formal
product acceptance remains staged because gates 1, 2, 3, 5, and 7 require the
human pixel/stereo review, including the explicit Quest star-cost exception
recorded above.

## Related

- [`036-native-sky-and-day-night-cycle.md`](036-native-sky-and-day-night-cycle.md)
- [`306-seasonal-appearance-preview.md`](306-seasonal-appearance-preview.md)
- [Tactical 307](307-seasonal-solar-path-and-cyclical-latitude.md)
- [`../topics/seasons.md`](../topics/seasons.md)
- [`../topics/lighting.md`](../topics/lighting.md)
- [`../topics/fog-atmosphere.md`](../topics/fog-atmosphere.md)
