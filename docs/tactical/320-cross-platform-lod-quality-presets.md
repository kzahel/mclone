# Tactical 320: Cross-Platform LOD Quality Presets

Status: completed and accepted 2026-08-20

Topic: `procedural-horizon-clipmap`

Topic: `graphics-video-settings`

## Instruction Synthesis

Turn the shared procedural-horizon LOD into a bounded, player-selectable
quality feature instead of introducing a second Quest-specific terrain
system. Replace the existing binary terrain-horizon control with named LOD
quality presets which apply live, persist, and appear in the ordinary shared
Graphics menu on desktop, Web, flat Android, desktop XR, and Android XR.

Every preset has the same meaning on every host. Platform profiles may choose
different defaults when no player preference exists, but a stored explicit
choice must win over that default and remain machine-local. Fog is an
independent optional presentation setting: changing LOD quality must never
enable, disable, or rewrite fog. When the player has independently selected a
fog configuration with a conservative opaque boundary, the renderer may use
that nearer boundary as an additional cull.

Keep the current shared toroidal geometry clipmap, terrain source, exact
replacement, and vegetation ownership. Bound its level count, visibility,
projection reach, and proxy-vegetation reach through one shared preset
descriptor. Do not recover the retired chunk-based Far LOD, create a
Web-specific or Quest-specific LOD implementation, or hide quality loss behind
an app-local default.

## Starting Point And Current Evidence

Tactical [`274`](274-all-client-distant-terrain-control.md) already provides
the required vertical path:

- `mclone-ui` exposes `Terrain Horizon: Exact Only | Composed` in the shared
  Graphics page;
- `mclone-app-runtime::ClientGraphicsPreferences` persists the binary choice
  through native, Android, XR, and browser storage adapters;
- `mclone-scene` applies the choice live and retains a stored composed request
  when the active world source is incompatible;
- flat and per-eye XR projections derive composed reach from the same shared
  clipmap contract; and
- explicit startup arguments remain unsaved developer/test overrides.

The live full-quality descriptor is ten levels, four-by-four tiles per level,
stride-one geometry, and procedural vegetation. It allocates 160 clipmap
slots. `TerrainClipmapConfig` already makes level count a validated setting,
and the reusable service deliberately does not require every consumer to use
the ten-level default. Render stride greater than one is not currently
quality-proven and must not become the shortcut for a low preset.

A matched release Quest 3 RD5 diagnostic at revision `20ec052d`, 72 Hz,
dual-per-eye rendering, scale 1.0, foveation off, a fixed pose, and normal
actors measured:

| Mode | Submitted FPS | App work avg / p95 | Meta app GPU | Over period |
|---|---:|---:|---:|---:|
| Exact A | `72.01` | `5.007 / 5.425ms` | `2.538ms` | `0.0%` |
| Full composed B | `68.75` | `14.386 / 15.828ms` | `8.058ms` | `70.1%` |

The composed row was fully settled: all 160 slots were ready, generation and
publication were idle, 55 terrain tiles were drawn, 93 were frustum-culled,
12 were inner-hole-culled, and zero were fog/far-culled. Proxy vegetation drew
612 tree instances from 35 tiles. Recovering average 72-Hz budget requires
about `0.497ms`, or only 5.3% of the measured incremental horizon app cost;
bringing p95 under budget requires about `1.939ms`, or 18.6% of that
incremental p95 cost.

Two exact repeats exposed a separate benchmark defect: actor pipelines may
compile just after the current five-second quiet gate and contaminate a sample
with one roughly 750ms startup frame. Their steady medians and Meta GPU samples
remained close to Exact A, ruling out material short-run thermal drift. Preset
acceptance must ensure all required render pipelines are warm before the
measurement window rather than treating that startup compile as LOD cost.

This evidence selects a smaller bound and shorter proxy range as the first
quality experiment. It does not justify a new generator or adaptive quadtree:
the settled generation path was already idle, while persistent per-eye draw,
encode, and GPU work exceeded the frame budget.

## Objective

Replace the binary Graphics row with:

```text
Graphics
  Distant Terrain: Off | Low | Medium | High
  Fog...                              # independent existing submenu
```

`Distant Terrain` is the player-facing name. The shared typed contract may use
`TerrainLodPreset` internally. The row must be present on every first-class
interactive client and must report an unavailable effective state when the
active source cannot compose procedural terrain without discarding the stored
choice.

At completion:

- `Off` retains the protected exact-only path and allocates no horizon;
- `Low`, `Medium`, and `High` select one shared, validated descriptor each;
- all non-Off descriptors retain the same terrain semantics, exact/procedural
  arbitration, four-by-four nested-ring topology, and stride-one surface
  quality;
- platform profiles choose an initial preset only when no explicit preference
  or launch override exists;
- menu changes apply in the active scene without restart and save only after
  successful application;
- fog can remain Off at every LOD quality; and
- lowering the preset actually reduces steady render work and total residency,
  rather than changing only the projection matrix or hiding distant pixels.

## Binding Decisions

### One shared preset descriptor, not separate implementations

Add one UI-neutral descriptor in `mclone-terrain-view`, equivalent to:

```text
TerrainLodPresetDescriptor {
    level_count,
    tiles_per_axis,
    base_sample_spacing,
    render_cell_stride,
    terrain_visibility_distance,
    vegetation_visibility_distance,
    vegetation_enabled,
}
```

The exact field shape may use derived distances or a maximum vegetation level
when that preserves stronger invariants. The descriptor, validation, clipmap
reach, projection reach, terrain admission, vegetation admission, and
diagnostics remain shared. Apps provide platform/capability facts and storage
mechanics only; they do not construct their own descriptor or reinterpret a
preset name.

Use the current four-by-four tile axis, base spacing, and render stride one for
the first preset family. The initial measurement candidates are:

| Preset | Initial level candidate | Slot bound | Initial intent |
|---|---:|---:|---|
| Off | 0 | 0 | Exact terrain only; no horizon engine or proxy work |
| Low | 6 | 96 | Bounded mobile horizon with a substantially shorter proxy-tree range |
| Medium | 8 | 128 | Broad constrained-device horizon with a shorter proxy-tree range |
| High | 10 | 160 | Current full-reach, stride-one terrain and vegetation behavior |

Six and eight are experiment inputs, not unmeasured visual promises. Human
review and matched performance may adjust the Low/Medium level or vegetation
bounds before their names become accepted product semantics. Once accepted, a
preset name must remain stable across platforms; platform tuning changes the
default selection, not what `Medium` means.

Removing two outer levels reduces maximum clipmap reach by approximately four
because sample spacing doubles per level. The current ten-level reach is far
beyond an ordinary ground-level view, so a materially smaller slot and draw
bound can still retain a convincing horizon. Diagnostics must report the
resolved descriptor, resident slots, per-level terrain draws, per-level proxy
draws and instances, and every cull class so that this is measured rather than
assumed.

### Fog is optional and orthogonal

The LOD preset must not contain a fog mode, opacity, color, visibility, weather
response, or `far_cull` preference. Changing LOD quality must leave
`GameFogSettings` byte-for-byte unchanged. Turning Fog Off must not change the
resolved LOD preset.

Terrain admission uses the preset's own finite clipmap bound whether fog is on
or off. Projection reach follows that same bound. When independently enabled
fog supplies a conservative fully opaque boundary, terrain and proxy
visibility may use the nearer of:

```text
preset visibility bound
opaque fog far-cull bound
```

Natural fog currently caps opacity below one and therefore supplies no safe
far cull. The LOD preset must still work in that mode and with Fog Off. Do not
relax the conservative whole-tile test or discard visible geometry merely
because it is heavily tinted.

Fog-off review is a first-class gate. Reduced outer rings must retain an
intentional horizon through their existing nested-ring edge/skirt and
background relationship. They may not expose clear-color holes, terrain
undersides, a floating square boundary, or forced haze. If a proposed Low
bound exposes the finite edge at an accepted play altitude, increase its reach
or add a shared terrain-owned outer closure; do not silently turn fog on.

### Proxy vegetation has its own shorter bound

Terrain and proxy vegetation share source and exact/procedural ownership but
do not require identical maximum distance. Low and Medium should stop planning,
retaining, and drawing far proxy records before the terrain edge. High begins
as the current full-quality control.

A shorter range must preserve whole-record exact/proxy XOR and stable tree
identity. Use a stable level/distance transition or deterministic identity
fade if direct removal sparkles during camera motion. Do not alter exact
natural trees, live entity distance, or general actor quality through this
setting.

### Platform defaults are fallback policy, not different presets

The initial default candidates are:

| Platform profile | Candidate default |
|---|---|
| Native desktop flat | High |
| SteamOS / handheld native | Medium |
| Browser / WebGPU | Medium |
| Flat Android | Low |
| Desktop OpenXR | Medium |
| Android XR / standalone Quest | Medium |

These defaults are completion-gated by pixels and frame evidence. A platform
may move to a lower or higher accepted preset before completion, but must not
fork the descriptor. Source incompatibility may force the effective mode Off
while retaining the desired platform default or explicit choice.

The shared startup resolver owns the mapping from a typed platform/capability
profile to the default preset. Platform apps identify their profile and legal
capabilities; they do not set an arbitrary enum in local rendering code.

There is no player-facing `Auto` value in this first slice. Internally, an
unset preference means "use the current platform-profile default." As soon as
the player selects a row value, that explicit preset is stored. Factory Reset
removes the explicit choice and returns to the platform default.

Apply precedence in the established graphics order:

1. platform capabilities and source compatibility define legal/effective
   behavior;
2. the shared platform profile supplies the default for an unset preference;
3. a stored explicit player preset replaces that default;
4. an explicit CLI/query/test override replaces both for one launch and is not
   saved; and
5. a successfully applied live menu edit becomes the stored explicit choice.

Preferences remain machine-local. A desktop `High` selection must not sync to
a phone, browser, Deck, or Quest.

### Legacy persistence preserves intent

Replace the stored binary `terrainPresentation` field with an optional typed
LOD preset while retaining a backward-compatible decoder:

```text
legacy ExactOnly -> explicit Off
legacy Composed  -> explicit High
missing field    -> unset; resolve the platform default
```

Mapping legacy Composed to High preserves the pixels and cost the player
explicitly selected. Do not reinterpret it as each platform's default.
Encoding may bump the graphics document schema or add a compatible optional
field, but must retain malformed/future-schema handling, native atomic writes,
browser `localStorage`, Factory Reset registration, and all-client reload
coverage.

Retain `--terrain-presentation exact-only|composed` as a deprecated test
compatibility alias mapping to Off/High. Add one canonical preset launch input
for Low/Medium/High and the equivalent Web query/startup path. Launch inputs
remain unsaved.

### Live switching reuses common levels and commits at a frame boundary

Do not build a second full engine beside the current one on memory-constrained
devices merely to change level count. Add shared in-place reconfiguration or
another bounded transition which reuses unchanged inner levels and source
identity:

- Off tears down procedural and proxy resources as today;
- lowering a non-Off preset retires outer levels and out-of-range vegetation
  at a frame boundary, updates both-eye/mono projection reach, and releases
  their fixed resources;
- raising a preset preserves ready common levels, admits new outer levels
  coarse-first under existing budgets, and keeps the prior committed horizon
  until the entering coverage is drawable; and
- rejected allocation or unsupported configuration leaves the previous
  effective preset active and does not save the requested value.

The UI must project desired, applying, effective, and unavailable/rejected
state when a transition is not immediate. It must not report `Medium` merely
because the button was pressed while High resources are still active or the
active world can only render exact terrain.

## Shared Ownership

- `mclone-terrain-view` owns `TerrainLodPreset`, validated descriptors,
  clipmap/vegetation bounds, in-place reconfiguration, visibility, resource
  release, and diagnostics.
- `mclone-ui` owns the host-neutral row, labels, desired/effective projection,
  and typed action.
- `mclone-app-runtime` owns platform-profile default resolution, startup and
  stored-preference precedence, the versioned preference migration, and
  storage-neutral codecs.
- `mclone-scene` owns active-source compatibility, live application,
  frame-boundary commit, projection reconciliation, persistence after accepted
  effect, and the exact-only fast path.
- `mclone-render` continues to own fog semantics and conservative opaque-bound
  derivation. The preset consumes only an optional cull distance and never
  mutates fog state.
- Desktop, Web, flat Android, desktop OpenXR, and Android XR adapters supply
  platform/capability identity, storage mechanics, surface/session facts, and
  cadence only.

Web TypeScript remains a domain-blind broker. It may load/store the shared JSON
and relay typed Rust startup/actions, but it must not define preset values,
level counts, vegetation ranges, or a browser-only fallback.

## Implementation Slices

### Slice 1: preset and evidence contract

- [x] Add the shared preset enum, descriptor table, validation, labels, and
      platform-profile default resolver.
- [x] Pin Off/Low/Medium/High semantic tests, including current High parity.
- [x] Add per-level terrain/vegetation and resolved-bound diagnostics.
- [x] Harden the Quest settled gate so required actor/horizon pipelines are
      warm before performance sampling.

### Slice 2: preference, menu, and all-client wiring

- [x] Replace the binary shared Graphics row with the four-value preset row.
- [x] Add unset-versus-explicit preference state and legacy migration.
- [x] Preserve source-unavailable desired/effective reporting.
- [x] Restore, apply, persist, reload, and Factory Reset the same choice on
      desktop, Web, flat Android, desktop XR, and Android XR.
- [x] Add the canonical CLI/Web startup override and retain legacy aliases.

### Slice 3: bounded live reconfiguration

- [x] Reconfigure shared clipmap level count without duplicating the full
      engine or invalidating unchanged common levels.
- [x] Coordinate terrain, proxy vegetation, exact coverage, projection reach,
      and mono/per-eye/multiview render targets at one frame boundary.
- [x] Bound/cancel obsolete vegetation work and release retired resources.
- [x] Keep the prior accepted preset active on allocation or apply failure.

### Slice 4: optional-fog and fog-off presentation

- [x] Prove every preset with Fog Off and confirm the fog preference is
      unchanged by preset transitions.
- [x] Retain conservative fog-derived far culling only when an independently
      selected mode reaches an opaque boundary.
- [x] Inspect outer edges at ground, hill, flight, stereo, and wrapped-topology
      views; correct shared closure or raise an inadequate bound rather than
      forcing haze.
- [x] Prove exact and proxy ownership across the shorter vegetation boundary.

### Slice 5: platform defaults and performance decision

- [x] Resolve an unset preference through each typed platform profile.
- [x] Measure Low/Medium/High using one exact build and identical scene inputs
      on the affected performance targets.
- [x] Select final defaults only from accepted pixels, startup/movement cost,
      steady frame evidence, and memory bounds.
- [x] Record the accepted descriptor table and defaults in the living LOD and
      graphics-settings topics.

## Validation Matrix

Shared automated coverage must include:

- descriptor validation, slot/reach math, High parity, and Off zero-allocation;
- legacy ExactOnly/Composed migration, missing-field platform default,
  explicit stored choice, launch override, malformed/future schema, save-after-
  apply, and Factory Reset;
- menu mouse, keyboard, controller, touch, and XR action dispatch through the
  same typed action;
- live High-to-Low-to-Off-to-Medium transitions with bounded allocation,
  cancellation, common-level preservation, source replacement, and failure
  rollback;
- exact/procedural coverage, connector, vegetation XOR, negative coordinates,
  plane/cylinder/torus topology, and both fog-off and opaque-fog visibility;
- mono, two-eye per-eye, synthetic stereo, array per-eye, and full-frame
  multiview projection/visibility invariants; and
- native and Wasm compilation without platform-local terrain policy.

Rendered and product evidence:

1. Inspect matched native desktop Off/Low/Medium/High pixels at ground level,
   hill height, and high flight with Fog Off, Natural fog, and one explicitly
   opaque far-cull configuration.
2. Exercise one-process menu switching and relaunch restoration on native
   desktop and headed WebGPU; verify Web `localStorage` and exact shared
   descriptor receipts.
3. Build and smoke flat Android, inspect an AVD or physical-device menu/pixel
   result, and verify the Low default when no preference exists.
4. Inspect synthetic stereo and at least one real OpenXR target with distinct
   per-eye projections and matching preset reach.
5. On physical Quest, run alternating High/Medium/Low/High stationary and
   settled-orbit samples at RD5, 72 Hz, scale 1.0, ordinary per-eye rendering,
   foveation off, fixed world/pose, and normal actors.
6. Compare app-work p50/p95/p99, thread CPU, Meta app GPU, over-period frames,
   horizon per-level draws, proxy instances, residency, settle time, and
   process memory. Exclude any window containing lazy pipeline compilation.
7. Repeat the selected Quest default with Fog Off and with optional opaque fog
   to separate preset/ring savings from fog-derived culling.

The Quest default should sustain the 72-Hz stationary gate with positive
average headroom and materially better p95 than current High. Moving evidence
must not be worse than the current full-quality path. Web and flat Android
defaults must remain responsive through startup and live switching; desktop
High must preserve current accepted full-quality appearance.

## Completion Evidence

The accepted descriptors keep a four-by-four ring, base spacing one, and
render stride one at every non-Off quality:

| Preset | Levels | Resident slots | Proxy vegetation levels |
|---|---:|---:|---:|
| Off | 0 | 0 | 0 |
| Low | 6 | 96 | 1 |
| Medium | 8 | 128 | 2 |
| High | 10 | 160 | 4 |

Native fixed-resource diagnostics measured approximately `81.6 MiB` for Low,
`107.4 MiB` for Medium, and `133.2 MiB` for High. Off constructs no horizon.
Live non-Off changes reuse common clipmap levels, admission state, GPU pools,
and compatible vegetation; lowering retires outer work, while raising admits
new levels under the existing bounded queues. Persistence commits only after
the scene accepts the effect.

Two physical Quest 3 fog-off RD5 orbit sequences used the same release APK,
world, pose, 72 Hz target, scale 1.0, foveation Off, normal actors, and
ordinary dual-per-eye renderer. The second sequence reversed the first one's
quality order:

| Preset | Submitted FPS | App work avg / p95 | Over period | Drawn terrain / proxy trees |
|---|---:|---:|---:|---:|
| Off | `72.00 / 72.00` | `6.129 / 7.071ms`; `6.123 / 7.086ms` | `0.0% / 0.0%` | `0 / 0` |
| Low | `71.83 / 71.87` | `12.671 / 13.878ms`; `12.649 / 13.662ms` | `4.8% / 2.0%` | `47 / 87` |
| Medium | `70.52 / 70.45` | `13.947 / 15.275ms`; `13.960 / 15.318ms` | `45.6% / 46.8%` | `51 / 342` |
| High | `63.98 / 63.78` | `15.489 / 16.988ms`; `15.534 / 17.134ms` | `94.4% / 93.7%` | `55 / 612` |

A final rebuilt-APK run with no LOD override resolved the Android XR profile
to Low and measured `71.86 FPS`, `12.643 / 13.649ms` average/p95 app work,
`1.246ms` average headroom, `2.3%` over-period frames, and `6.786ms` Meta app
GPU. The matched explicit High control measured `64.07 FPS`,
`15.450 / 17.015ms`, `-1.561ms` average headroom, `91.4%` over-period frames,
and `8.185ms` Meta app GPU. The six-level Low preset is therefore the accepted
standalone Quest default. Medium remains available as a player choice and as
the unset Web, SteamOS, and desktop-OpenXR default.

The final unset defaults are:

| Platform profile | Default |
|---|---|
| Native desktop flat | High |
| SteamOS / handheld native | Medium |
| Browser / WebGPU | Medium |
| Flat Android | Low |
| Desktop OpenXR | Medium |
| Android XR / standalone Quest | Low |

Fog remained explicitly Off throughout the native and Quest preset captures.
No preset changed that preference, no forced haze or clear-color terrain hole
was observed, and every device diagnostic reported zero fog/far-culled tiles.
Opaque-fog culling remains an optional independent nearer bound.

Acceptance also includes the full terrain-view, UI, and scene library suites;
native, Wasm, desktop-XR, flat-Android, and Android-XR compilation; headed
WebGPU menu switch plus persisted reload; inspected native fog-off captures;
an inspected synthetic-stereo frame; an inspected flat-Android AVD frame; and
inspected physical Quest captures. The AVD used a freshly generated ignored
development archive because an unrelated tracked asset lock was stale; this
tactical did not rewrite that lock.

## Human Review Result

The inspected native Off/Low/Medium/High fog-off captures, headed WebGPU live
toggle/reload captures, synthetic stereo frame, AVD frame, and physical Quest
captures accept the final preset family and defaults:

- Low reads as continuous distant terrain without a nearby finite plate,
  forced haze, or visible clear-color boundary at the accepted views.
- Medium preserves broader landscape reach but does not meet the standalone
  Quest frame target.
- terrain edges, tree range transitions, connectors, water, and exact/proxy
  ownership remain stable through the accepted static and moving lanes;
- High preserves the former full-quality descriptor and appearance;
- native, Web, Android, desktop-XR, and Quest defaults match their measured
  cost without changing preset semantics; and
- changing LOD leaves the selected Fog mode and appearance untouched.

## Explicit Non-Goals

- no new quadtree, chunk-granular Far LOD, or alternate terrain generator;
- no app-local Quest, Android, or Web LOD implementation;
- no mandatory fog, automatic haze, or LOD-driven fog mutation;
- no render stride greater than one without a separately proven stitching and
  normal-quality campaign;
- no automatic runtime frame-time quality controller in the first slice;
- no broad Low/Medium/High graphics bundle which also rewrites leaves, grass,
  world resolution, actors, particles, or foveation;
- no server simulation, exact chunk render distance, or world-save change; and
- no promotion of multiview as a performance fix without a new matched device
  win.

## Completion Checklist

- [x] Replace the binary horizon setting with shared Off/Low/Medium/High
      presets on every first-class client.
- [x] Preserve platform-independent preset semantics and select only the
      unset default per platform profile.
- [x] Migrate legacy preferences and preserve explicit player/launch
      precedence.
- [x] Apply and persist presets live without restart or unbounded duplicate
      resources.
- [x] Keep Fog independent and accept every preset with Fog Off.
- [x] Bound terrain residency, projection, rendering, and proxy vegetation
      through one shared descriptor.
- [x] Pass native, WebGPU, flat Android, per-eye, synthetic-stereo, multiview,
      and physical Quest gates.
- [x] Record final preset descriptors, platform defaults, pixels, and
      performance evidence in the living topics.
