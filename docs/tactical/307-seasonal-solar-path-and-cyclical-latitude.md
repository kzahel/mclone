# Tactical 307: Seasonal Solar Path And Cyclical Latitude

Status: planned 2026-08-15

Topic: `seasons`

## Instruction Synthesis

Give the original Mclone Overworld a season-aware sun path driven by an actual
dimension climate-coordinate policy. The current product default is an
unbounded Euclidean plane, not a cylinder. Its first policy therefore uses a
large, smooth, repeating effective latitude along world Z:

```text
Equator -> North Polar -> Equator -> South Polar -> Equator
```

Combine observer latitude, a continuous orbital phase, and time of day into
one pure solar sample. Use that sample consistently for the visible sun,
sunrise/sunset glow, sky brightness, and rendered skylight. Expose typed Debug
overrides for orbital phase, latitude source/value, and solar time so the user
can inspect the result interactively in desktop flat, desktop OpenXR, and
Android XR. Reuse Tactical 306's client-local `Season Preview` master switch;
when it is off, retain the accepted fixed-path sky exactly.

This tactical establishes the latitude policy as shared climate vocabulary
but does not change terrain generation. A later tactical may combine the same
latitude with existing Mclone temperature/moisture, altitude, and continental
facts to form an annual climate normal. Current-season snow, foliage, and
ground response remain Tactical
[`306`](306-seasonal-appearance-preview.md); latitude-informed worldgen and
gameplay daylight remain separate follow-ups.

Do not turn the plane into a wrapped dimension, make the optional cylinder the
default, introduce longitude/time zones, or implement physically curved
terrain or radial gravity.

## Starting Point

The engine already has part of the required sky path:

- product startup defaults to `mclone-overworld-v1` with
  `HorizontalTopology::UNBOUNDED` on X and Z;
- Mclone Overworld separately supports an explicitly selected 384-chunk
  X-periodic cylinder, but that is not the ordinary product world;
- the server persists and advances one authoritative vanilla-shaped
  `day_time`, and clients interpolate it between updates;
- `mclone-core::time` exposes the vanilla `time_of_day` and one-dimensional
  `sun_angle` calculations;
- `mclone-render::sky_render` draws the sky disc and sunrise/sunset glow in
  mono, placed, per-eye, and multiview paths;
- sky clear color and the exact-terrain lightmap darkening follow the existing
  global celestial phase; and
- Tactical [`036`](036-native-sky-and-day-night-cycle.md) still owns its open
  Phase 3 baseline: the visible sun/moon textures and star field are not yet
  implemented.

The current Mclone generator is climate-aware but not latitude- or
season-aware. It already has broad/detail temperature and moisture fields,
altitude cooling, climate-selected biome/vegetation recipes, and static snow
surface decisions. None has an equator, hemisphere, orbital phase, or annual
climate normal.

Tactical 306 plans a temporary local material-preview phase. Its local
Spring/Summer/Autumn/Winter review landmarks must not become the global
calendar vocabulary. Once latitude exists, one orbital instant maps to
opposite local seasons in the two hemispheres.

## Objective

At fixed world revision and time of day, traveling through one selected plane
climate wavelength changes the sun law coherently:

```text
plane climate phase:  0.00       0.25       0.50       0.75       1.00
effective latitude:   0 deg     +90 deg      0 deg     -90 deg      0 deg
region:               equator   north pole   equator   south pole   equator
```

- equatorial belts retain approximately equal day and night across the year;
- temperate northern and southern belts receive opposite seasonal solar
  forcing;
- a polar belt can enter polar day in its summer and polar night in its
  winter;
- sun direction, elevation, twilight, sky brightness, and rendered skylight
  agree;
- the solar path changes continuously while approaching and leaving each
  polar crest;
- places with the same effective latitude use the same solar law even when
  they occur on opposite sides of a polar belt; and
- traveling along X stays in approximately the same latitude family.

The first implementation is visual presentation only. It does not change the
authoritative day clock, world blocks, light graph, biome, vegetation,
resources, spawning, sleep, crops, animals, persistence, or unloaded regions.

## Binding Decisions

### The unbounded Mclone plane is primary

Do not describe the default Mclone Overworld as a cylinder. Select a solar
coordinate policy from both generation profile and dimension topology:

```text
SolarCoordinatePolicy =
    VanillaFixed
    | McloneCyclicPlane { wavelength_blocks, phase_origin_z }
    | McloneAsymptoticCylinder { climate_scale_blocks }
```

`mclone-overworld-v1 + unbounded plane` selects `McloneCyclicPlane`. Retained
reference, Alpha, Beta, Flat Grass, Small Island, Authored Only, and topology
probe dimensions keep their current fixed/vanilla sky unless a later explicit
decision opts them in. Topology alone must not silently give every unbounded
dimension Mclone cosmology.

The optional `mclone-overworld-v1 + cylinder-x:384` may consume the secondary
asymptotic policy in pure tests and focused captures. It remains opt-in and is
not acceptance evidence for the product-default plane.

### Plane latitude is a smooth climate coordinate

The initial policy is equivalent to:

```text
latitude_phase = (world_z - phase_origin_z) / wavelength_blocks
effective_latitude = 90 deg * sin(TAU * latitude_phase)
```

Use stable finite `f64` coordinate math before converting the final render
sample to finite `f32`. The query is coordinate-pure, allocation-free, and
independent of chunk load order. It neither enumerates climate cells nor stores
per-chunk latitude.

The latitude is an intentionally fictional solar/climate coordinate on a flat
world. A polar maximum is a broad line-like crest, not a spherical point. The
first policy does not rotate the compass frame when crossing it and does not
add a twelve-hour time shift on the far side. Therefore two places with equal
effective latitude have equal sun paths at the same orbital phase and global
time even if their latitude derivatives have opposite signs.

This choice preserves one global day clock and a continuous simple sky law. A
globe-like pole crossing would require local frame parity and spatial solar
time; that is explicitly outside this tactical.

Do not add a renderer-private warp or noise field. Start with a legible Z-axis
policy. A later annual-climate/worldgen study may propose a broad shared warp
only if it retains the navigational rule that X mostly follows a latitude band
and Z mostly crosses climate.

### Wavelength and phase origin require calibration

Do not select the latitude wavelength by aesthetic guess or by stretching one
existing noise octave. Before binding constants:

- measure current major-continent and connected-habitat dimensions across
  representative Mclone seeds;
- compare candidate equator-to-pole travel distances against ordinary walking,
  faster travel, expected year length, and intended migration geography;
- render latitude, current temperature/moisture, altitude, coastline, and
  candidate annual-mean temperature overlays at a common scale;
- verify that multiple terrain regions and useful travel remain inside each
  temperate band; and
- choose a phase origin that gives the ordinary spawn search a deliberate
  climate family rather than accidentally making default spawns polar.

Record rejected candidates and the accepted constants in this tactical's
execution record. The latitude policy becomes `mclone-overworld-v1` behavior
and must follow that profile's internal-mutable fixture/regeneration policy.
Do not claim compatibility with an already generated internal world unless it
is explicitly migrated or regenerated.

### Orbital phase is global; local season names are derived

Use one normalized cyclic orbital value with these world-level landmarks:

```text
0.00  northward equinox
0.25  northern solstice
0.50  southward equinox
0.75  southern solstice
1.00  wraps to northward equinox
```

Do not label these global values simply Spring, Summer, Autumn, and Winter.
Those names are local interpretations: the northern solstice is northern
summer and southern winter.

For this tactical, orbital phase is client-local Debug presentation state. It
is not a saved calendar and does not advance automatically. Tactical 306's
material-review phase may remain a local-season override until a later
integration explicitly maps the shared orbital/latitude sample into its
appearance model. Do not create a second incompatible phase type.

### One pure solar sample owns the math

Place the dependency-leaf vocabulary and math in the same shared owner selected
by Tactical 306, preferably `mclone-season`. A value equivalent to this is the
target contract:

```text
SolarInput {
    orbital_phase,
    effective_latitude,
    axial_tilt,
    solar_time_fraction,
}

SolarSample {
    direction,
    elevation,
    azimuth,
    daylight_factor,
    twilight_factor,
    day_length_fraction,
    polar_state,
}
```

Use ordinary solar declination/elevation relationships, with exact coordinate
axis and hour-angle conventions pinned by tests:

```text
declination = axial_tilt * sin(TAU * orbital_phase)

sin(elevation) =
    sin(latitude) * sin(declination)
    + cos(latitude) * cos(declination) * cos(hour_angle)
```

Derive hour angle linearly from replicated `day_time`, not from the existing
vanilla-smoothed `time_of_day` curve. Preserve the Minecraft clock anchors while
displaying conventional solar time in Debug UI:

```text
day_time 0      -> 06:00 solar time
day_time 6000   -> 12:00 solar time / hour angle 0
day_time 12000  -> 18:00 solar time
day_time 18000  -> 00:00 solar time
```

The retained fixed/vanilla path continues using its current smoothing. The
Mclone seasonal path needs a linear hour angle so derived sunrise, sunset, and
day length are internally meaningful.

Derive a normalized direction rather than independently approximating sun
elevation in each renderer. Daylight and twilight are smooth functions of
elevation around the horizon. Polar day/night classification comes from the
same sample rather than a latitude threshold special case.

Compare an Earth-like tilt and one restrained stylized larger tilt through the
capture matrix, then bind one dimension constant. Axial tilt is not a Debug or
product slider in this tactical.

### One observer-local sky sample is sufficient

Resolve effective latitude at the shared player/observation root once per
frame. Both XR eyes and both XR render paths consume the identical solar
sample; never evaluate latitude from individual eye positions. The chosen
climate wavelength must be large enough that a single sky direction across one
ordinary visible region is coherent.

Ground and later ecology may query latitude per position. This tactical does
not add per-fragment latitude, scan loaded chunks, or update inactive terrain.

### Finish the ordinary sun before bending its path

Tactical 036 retains ownership of baseline sun/moon/star parity. Complete and
validate its visible sun disc in the ordinary fixed-path sky before seasonal
solar acceptance. Work may be sequenced in the same implementation series, but
the commits and execution records must make the boundary clear:

- Tactical 036 owns asset loading, the ordinary sun quad, moon phases, stars,
  baseline draw ordering, and unchanged behavior for non-Mclone profiles;
- Tactical 307 owns Mclone latitude/orbital sampling and projecting the shared
  sun through the resulting direction/elevation; and
- neither tactical may introduce a mono-only or per-eye-only sky path.

Do not fake acceptance with only the existing sunrise glow. The user must be
able to observe the actual sun path.

### Every daylight consumer uses the shared elevation

For opted-in Mclone seasonal solar presentation, route the same `SolarSample`
to:

- visible sun direction;
- sunrise/sunset glow orientation and strength;
- sky clear/dome brightness;
- exact-terrain and actor sky-light darkening;
- open-air fog/atmosphere sky color where that shared path is active; and
- stable diagnostics and capture receipts.

Do not move the sun disc while leaving sky color and terrain brightness on the
old fixed curve. Preserve existing block-light contribution and avoid light
graph recomputation: the sample changes the rendered sky-light multiplier,
not stored sky-light propagation.

Directional shadows do not exist and are not added here. Material response
should not pretend that a directional shadow map is present.

Non-Mclone profiles retain byte-/tolerance-equivalent current time-of-day and
lightmap behavior. This original-product divergence must not rewrite the
vanilla reference path globally.

### Debug controls are shared and temporary

Extend the shared Debug options path with typed controls equivalent to:

```text
Season Preview       Off | On
Orbital Phase       cyclic 0..100 percent
Latitude Source     World | Manual
Preview Latitude    -90..+90 degrees
Solar Time Source   World Clock | Manual
Preview Solar Time  0..24 hours
```

Reuse the same `Season Preview` value and owner planned by Tactical 306;
whichever tactical implements it first establishes the shared contract. Do not
add a second solar-only master switch. All solar rows remain visible but
disabled while the preview is off. The latitude and solar-time sliders also
remain disabled unless their manual source is selected. `World` latitude is
the default and is the primary acceptance path. `World Clock` follows
replicated `day_time`; Manual is a client-local visual override and must not
send a server time command.

Use the existing shared UI action, `ClientExperienceController`, and
`mclone-scene` effect ownership used by other Debug settings. Desktop flat,
desktop OpenXR, Android XR, synthetic stereo, offscreen capture, Web, and flat
Android must not create separate enums or interpret row text. The title-side
screen may disable world-dependent values.

New processes and scenes reset to preview off, world latitude, world clock,
and the northward-equinox Debug orbital phase. Do not save these controls in
world records or preferences. Preview off is an exact no-op for the accepted
fixed-path sky and rendered light.

### Slider changes do no world or mesh work

Enabling/disabling the preview or changing orbital phase, latitude
source/value, or preview solar time may update small shared per-frame/per-view
render state only. It must produce zero:

- block or biome mutation;
- light-engine work or stored light updates;
- chunk requests, snapshots, scans, compiles, or uploads;
- atlas replacement;
- entity/ecology work;
- server commands;
- persistence dirties; and
- unloaded-world work or catch-up.

The steady cost is constant with respect to loaded chunks and world extent.

### Worldgen consumes annual climate later

This tactical may expose latitude maps beside current Mclone climate fields,
but it does not alter generated terrain. The later intended composition is:

```text
annual climate normal =
    existing regional temperature/moisture
    + latitude mean-temperature bias
    + altitude cooling
    + continental/coastal moderation

current climate =
    annual climate normal
    + latitude/orbital seasonal forcing
    + active weather
```

Future worldgen should consume the annual normal for durable biome vocabulary,
tree species, vegetation density, soils, perennial snow/ice, and habitat
character. Runtime seasons should own reversible foliage, ordinary snow/frost,
blooms, forage, temporary ice, and seasonal activity. Latitude should initially
bias rather than replace existing noise, and terrain shape should remain mostly
independent until separate map review supports a stronger coupling.

## Implementation Sequence

### Slice 0: Baselines, ownership audit, and Tactical 036 prerequisite

Status: planned.

- Confirm default Mclone creation and existing review worlds use the unbounded
  plane; pin an explicitly created cylinder only as secondary evidence.
- Capture current noon, sunset, midnight, and sunrise sky/terrain output with
  frozen day time in mono and synthetic stereo.
- Inventory every `time_of_day`, `sun_angle`, sunrise-glow, sky-clear,
  lightmap-darkening, fog, placed-world, per-eye, and multiview consumer.
- Complete or explicitly sequence Tactical 036 Phase 3 so a visible baseline
  sun disc exists in all required render topologies.
- Record flat desktop and Quest sky-pass/lightmap frame costs before seasonal
  math.

Gate: the execution record identifies one owner for every celestial fact,
contains inspected visible-sun baseline pixels, and proves that non-Mclone
profiles retain their existing path.

### Slice 1: Pure latitude, orbit, and solar model

Status: planned.

- Add the shared coordinate-policy, orbital-phase, and solar-sample vocabulary.
- Add pure cyclic-plane and secondary asymptotic-cylinder latitude queries.
- Produce candidate wavelength/phase-origin maps against current Mclone
  terrain/climate and record Human Review selection.
- Bind one axial tilt after reviewing the comparison captures.
- Use fixed-point orbital Debug state and stable finite coordinate conversion.

Add table-driven tests for at least:

- plane equator/north-pole/equator/south-pole/equator landmarks;
- periodic equality across one full latitude wavelength;
- continuity on both sides of polar crests and orbital wrap;
- equal sun samples at equal effective latitude on opposite crest shoulders;
- linear solar-time conversion at `day_time` 0/6000/12000/18000;
- equinox day length across representative latitudes;
- opposite solstice forcing at positive and negative latitude;
- temperate summer/winter noon elevation and day-length ordering;
- polar day and polar night;
- normalized finite directions across dense phase/time/latitude samples;
- cylinder X-seam equality and asymptotic Z bounds; and
- invalid/non-finite input rejection or normalization.

Gate: pure tests and inspected maps establish the product-default plane policy
without rendering, loading a world, or changing generation.

### Slice 2: Shared sky and rendered-light integration

Status: planned.

- Resolve one observer-root latitude and `SolarSample` in shared scene/session
  ownership.
- Project the visible sun from its direction in mono, placed, per-eye, and
  multiview sky paths.
- Drive glow, sky dome/clear, exact terrain, actors, and supported atmosphere
  from the same elevation-derived daylight/twilight values.
- Preserve the current fixed path for every non-opted-in profile.
- Add CPU/shader parity fixtures for any packed or uniform representation.
- Inspect the first northern-solstice midlatitude sun pixel before adding polar
  and opposite-hemisphere cases.

Drag orbital phase and manual solar time after the view settles and assert zero
section builds/uploads, atlas uploads, block/light updates, server commands,
or persistence dirties.

Gate: the visible sun, glow, sky, and rendered terrain brightness agree in mono
and stereo, with no world work and no regression to retained profiles.

### Slice 3: Shared interactive Debug controls

Status: planned.

- Add controller-friendly rows for orbital phase, latitude source/value, solar
  time source/value, including correct master/source disabled-row behavior.
- Route exact typed values through shared action/effect/session ownership.
- Report policy, world/manual latitude, latitude phase, orbital phase,
  declination, solar time, elevation, day length, and polar state in stable
  diagnostics.
- Add focus, keyboard/controller, pointer, capability, reducer, effect, reset,
  and non-persistence tests.
- Validate live interaction in desktop flat, desktop OpenXR, and Android XR.

Gate: one desktop and one XR session can enable the shared preview, move from
equator to manual polar inspection, scrub a full day and orbit, restore
World/World Clock, disable the preview, and observe no menu or render-state
drift.

### Slice 4: Spatial/temporal matrix, performance, and Human Review

Status: planned.

Add a focused capture runner, such as
`pnpm native:seasons:solar-capture`, that records under `/tmp`:

- world-derived plane latitude maps for accepted and rejected wavelength
  candidates;
- equator, +45, +75, north-polar crest, -45, -75, and south-polar crest;
- both solstices and both equinoxes;
- local midnight, sunrise shoulder, noon, sunset shoulder, and the relevant
  polar-day/night samples;
- matched visible sun, sky/glow, and lit-terrain pixels;
- paired mono and synthetic-stereo output; and
- the optional cylinder at representative Z and across its X seam, clearly
  labeled secondary.

Record profile, topology, seed, world position, latitude policy/constants,
effective latitude, orbital phase, axial tilt, day time/source, complete solar
sample, render path, revision, and image paths.

Then validate native, headed WebGPU, flat Android, desktop OpenXR, Android XR,
and physical Quest per-eye/full-frame multiview pixels where available. Run
headed GPU lanes sequentially and inspect every captured image.

Compare fixed-path baseline and seasonal-solar Mclone output for sky-pass and
total frame CPU/GPU, uniform writes, draw count, memory, section compile/upload
counters, and light work. If representative Quest GPU p95 regresses by more
than five percent or 0.25 ms, whichever is larger, stop and attribute the cost.

Gate: Human Review accepts the wavelength/anchor/tilt, the sun makes the
equator-to-pole loop legible, opposite hemispheres read correctly, XR is
comfortable, and the result remains inside the bounded presentation-only
contract.

## Acceptance

### Coordinate and solar-semantic gates

- Default `mclone-overworld-v1` remains an unbounded plane and selects the
  accepted cyclic-plane solar policy.
- One full selected Z wavelength yields equator, north-polar crest, equator,
  south-polar crest, and equator in that order.
- X travel does not materially change unwarped initial latitude.
- Effective latitude and every solar output are finite, continuous, and
  deterministic across negative/large coordinates and phase wraps.
- Equal effective latitude produces equal solar law on both sides of a polar
  crest; no globe-frame or twelve-hour longitude shift is implied.
- North and south temperate regions have opposite solstice responses.
- Equatorial day length remains approximately stable, while accepted polar
  samples prove polar day and night.
- The optional cylinder is reported as opt-in secondary behavior and never as
  the current product default.

### Visual and renderer gates

- A visible sun disc exists before seasonal-path acceptance.
- Preview off reproduces the accepted fixed-path sky and rendered-light output.
- Sun position, sunrise/sunset glow, sky brightness, atmosphere where active,
  and rendered sky light derive from one `SolarSample`.
- No visible sun remains above the horizon while the same sample renders full
  night, and no polar-day sample uses midnight terrain darkness.
- Mono, placed, per-eye, and multiview paths agree; both XR eyes share the
  observation-root sample and retain distinct view/projection data.
- Non-Mclone profiles retain their accepted fixed/vanilla solar and lightmap
  behavior.
- No directional-shadow, cloud, god-ray, precipitation, or moon-orbit claim is
  made.

### Interaction and ownership gates

- Shared Debug UI exposes one `Season Preview` master, one typed orbital
  slider, World/Manual latitude, manual latitude, World Clock/Manual solar
  time, and manual solar time.
- Desktop flat, desktop OpenXR, and Android XR use one action/effect owner.
- World-derived latitude, not the manual slider, is the primary acceptance
  path.
- Debug values reset to preview off, remain unsaved, send no server command,
  and do not alter authoritative `day_time`.
- Diagnostics expose enough exact values to reproduce every capture without
  UI text scraping.

### Work and boundary gates

- Solar changes cause zero block/biome writes, light-graph work, persistence,
  chunk requests/snapshots, mesh compiles/uploads, atlas uploads, or ecology
  work.
- No inactive chunk or unbounded coordinate enumeration occurs.
- Steady CPU work is constant with respect to loaded chunks and world extent.
- Terrain generation output and fingerprints do not change in this tactical.
- Tactical 306 material appearance and procedural-horizon LOD are not silently
  expanded.

## Validation Matrix

Use the current commands in
[`../platforms.md`](../platforms.md#validation-policy) at execution time. The
expected focused core is:

```bash
cargo fmt --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-core time
cargo test --manifest-path native/Cargo.toml -p mclone-season
cargo test --manifest-path native/Cargo.toml -p mclone-render sky
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:seasons:solar-capture
pnpm native:xr-emulation:smoke
pnpm native:web:chunk-smoke
pnpm native:android:apk
pnpm native:android-xr:apk
```

If Tactical 306 selects another dependency-leaf crate instead of
`mclone-season`, use that same owner and update both tacticals. Do not create a
dummy package merely to match this draft.

Browser/WebGPU captures on Linux use the headed Wayland lane after
`pnpm host:check`; save all screenshots, maps, contact sheets, receipts, and
traces under `/tmp`. Use the public Quest testbed/provider contract for device
selection, authorization, leases, and sleep-after-use.

## Non-Goals

- an authoritative or persisted year/season calendar;
- automatic orbital-phase advancement or selection of year length;
- latitude-informed terrain, biome, vegetation, surface, spawn, or wildlife
  generation;
- changing the default Mclone plane into a cylinder, torus, finite world, or
  globe;
- longitude, time zones, spatial hour-angle offsets, or successive equators
  having different local times;
- compass-frame rotation or a literal pole crossing at latitude crests;
- curved terrain geometry, radial gravity/up, changing block metrics, or a
  shrinking polar circumference;
- gameplay daylight effects on sleeping, hostile spawning, crops, animals,
  navigation, visibility, or resources;
- stored-light propagation or light-solver changes;
- directional shadows, shadow maps, clouds, god rays, lens flare, weather,
  precipitation, or a seasonal moon orbit;
- Tactical 306 snow/foliage implementation, seasonal hydrology, or ecology;
- procedural-horizon/LOD terrain-material changes; or
- a persisted graphics preference or public product setting.

## Stop Conditions

Stop and request a narrower follow-up decision if the proof requires:

- changing terrain generation before wavelength/anchor maps receive Human
  Review;
- turning `day_time` into a position-dependent authoritative clock;
- mutating stored sky light or scheduling light propagation as the sun moves;
- rebuilding terrain/actor meshes while scrubbing solar values;
- a renderer-private latitude field or a copy of worldgen climate noise;
- changing retained reference-profile sky behavior to simplify Mclone logic;
- accepting sunrise glow without a visible sun path;
- omitting placed, per-eye, or full-frame multiview sky/light consumers;
- inferring physical cylinder or spherical geometry from periodic topology; or
- hiding a discontinuous pole/time jump inside the cyclical plane policy.

## Code And Documentation Map

- `native/crates/mclone-core/src/time.rs` — current vanilla day phase and
  one-dimensional sun angle.
- prospective shared dependency leaf, preferably `native/crates/mclone-season`
  — orbital, latitude-policy, and pure solar-sample vocabulary/math.
- `native/crates/mclone-render/src/sky.rs` — current sky color and sunrise glow
  math.
- `native/crates/mclone-render/src/sky_render.rs` — sky geometry and all
  mono/per-eye/multiview celestial rendering.
- `native/crates/mclone-render/src/light_texture.rs` — rendered sky-light
  multiplier that must consume shared solar daylight for opted-in Mclone.
- `native/crates/mclone-scene` — observer-root policy resolution and shared
  mono/XR frame state.
- `native/crates/mclone-ui` and `native/crates/mclone-app-runtime` — shared
  Debug controls, typed actions, capability, and effects.
- `native/crates/mclone-worldgen/src/levelgen/mclone_overworld/fields.rs` —
  existing annual-climate inputs for map comparison only in this tactical.
- [`036-native-sky-and-day-night-cycle.md`](036-native-sky-and-day-night-cycle.md)
  — visible baseline sun/moon/stars prerequisite and retained-profile owner.
- [`306-seasonal-appearance-preview.md`](306-seasonal-appearance-preview.md) —
  local material appearance proof and recent-snow pulse.
- [`../topics/seasons.md`](../topics/seasons.md) — durable regional seasons,
  latitude, active-only simulation, and future worldgen direction.
- [`../topics/bounded-world-topology.md`](../topics/bounded-world-topology.md) —
  current plane/cylinder facts and local-Euclidean presentation contract.
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
  — Mclone profile support and internal-mutable compatibility policy.
