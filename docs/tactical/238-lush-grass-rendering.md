# Tactical 238: Lush Grass Rendering

Status: active 2026-07-24

Topic: [`../topics/lush-grass-rendering.md`](../topics/lush-grass-rendering.md)

Related topics:

- [`../topics/graphics-video-settings.md`](../topics/graphics-video-settings.md)
- [`../topics/bushy-leaf-rendering.md`](../topics/bushy-leaf-rendering.md)
- [`../topics/performance.md`](../topics/performance.md)
- [`../topics/lod-native-vegetation.md`](../topics/lod-native-vegetation.md)

## Originating Direction

Implement the researched lush-grass effect end to end through the shared
section compiler, render-session, renderer, scene, settings, preference, and
platform contracts. The completed core includes static biome-tinted blades,
stable quality/LOD profiles, coherent wind, entity bending and recovery, and
mono, per-eye, full-frame multiview, browser, Android, and XR boundaries.

The implementation is original Rust and WGSL. Grassier Grass is an
All-Rights-Reserved research reference, not a source port. Do not copy its
code, shaders, textures, constants as a collection, or reconstructed control
flow. The ignored pinned artifact may be rehydrated and hash-verified for
research, but no artifact-derived file enters tracked or distributable
content.

## Goal

Turn exposed canonical grass-block surfaces into an optional dense
presentation volume while preserving the authoritative block world:

```text
resident snapshots + mesh policy
  -> section-keyed compact grass patches
  -> shared upload and per-world GPU instance residency
  -> observer-local quality/LOD draw plan
  -> original tapered blade templates
  -> stable wind and presentation-scoped interaction
  -> mono / stereo / multiview / placed-world pixels
```

The feature must be deterministic, biome- and light-correct, cheap to disable,
and independent from Leaf Detail. It does not change collision, raycasts,
lighting opacity, block state, simulation, world generation, protocol, or
world persistence.

## Scope

This tactical owns core Slices 0 through 4 from the living topic:

1. tactical, baselines, counters, and executable contracts;
2. static tinted patch instancing in every render topology;
3. Off/Sparse/Lush/Ultra quality, stable LOD, UI, and preference storage;
4. coherent world-space wind and clump character; and
5. entity interaction history and lifecycle recovery.

The following remain later optional work unless a core requirement exposes a
small prerequisite:

- generated replacements for short/tall grass and ferns;
- ordinary plant sway;
- snow tips or grass through snow;
- dense flower companions;
- detached blade particles;
- grass-specific shadow integration; and
- promotion of a non-Off Quest/mobile default.

`lod-native-vegetation` is complementary but separate. It owns semantic
natural-tree identity and far representations. Lush grass consumes exact
resident grass blocks and does not add individual blade geometry to terrain
Far LOD, Terrain Lab, world generation, or forest summaries.

## Locked Product Decisions

- Expose `Grass Detail: Off / Sparse / Lush / Ultra` as an independent
  Graphics row beside Leaf Detail.
- Default to `Off` on every platform for the initial implementation. Tests,
  captures, and explicit user choice exercise enabled profiles.
- Consider a desktop default only after complete measured closeout. Quest and
  mobile remain Off until physical device evidence supports a different
  profile.
- Persist the accepted choice in schema-1
  `ClientGraphicsPreferences`. Missing schema-1 fields decode as Off.
- Store only after the scene accepts the live effect. A renderer/pipeline
  preparation failure retains the last accepted value.
- Treat the quality as machine-local presentation state, not world, server,
  asset-pack, or gameplay state.
- Initially admit only `minecraft:grass_block` surfaces whose top is not
  occluded. Non-occluding flowers and ordinary plants may coexist; snow and
  partial solid cover are not special-cased in the core slice.
- Keep the underlying terrain top face and tint unchanged. Blades add volume;
  they do not replace the block texture.
- Use original untextured tapered strip geometry with no alpha-cutout blade
  silhouette. Back faces remain visible.
- Keep wind and interaction world-space and identical for both stereo eyes.
- Preserve a true Off path with zero patch discovery, patch result bytes, GPU
  grass residency, grass draw calls, wind work, and interaction uploads.
- Preserve the existing direct-path fast path when no grass is active.

## Updated Architecture Decisions

The topic predates shared terrain arenas/multi-draw, warm secondary worlds,
placed-world composition, live auxiliary views, and the Local Play preview.
This tactical folds those contracts into the first implementation.

### Patch artifact and compilation policy

Add a compact `GrassPatch` payload to `TexturedRenderSectionMesh`. The record
contains:

```text
world root position
packed RGB biome tint
packed block/sky light
stable canonical-position seed
reserved flags
```

The exact Rust/GPU layout is selected by compile-time size/alignment tests. A
24- or 32-byte world-position record is acceptable. Prefer a layout that can
live in one global instance arena and participate in later indirect
multi-draw; do not require a dynamic per-section origin uniform.

Patch discovery is request policy, not permanent catalog output:

- `RenderSectionCompileRequest` carries whether grass patches are enabled;
- native and browser compilers consume the same policy;
- Off requests do not call patch discovery;
- crossing Off/non-Off bumps resident section revisions and recompiles;
- switching Sparse/Lush/Ultra reuses the existing patch buffers; and
- returning Off immediately stops draw/update work, releases GPU grass
  residency, disables future patch discovery, and schedules metadata cleanup.

Extend the packed render-section result codec and its ABI tests. The browser
resident `SharedArrayBuffer` result lane must report patch bytes and retain
overflow behavior; no browser-only patch builder is permitted.

Patch eligibility and derived facts live in `mclone-mesh`:

- mark grass-block surface identity from the resolved block-state record;
- check the block above through the existing one-block section halo;
- reuse `block_tint(..., TexturedBlockTint::Grass, ...)`;
- sample packed light at the exposed surface;
- canonicalize seed coordinates through `HorizontalTopology`; and
- retain the lifted root used by the ordinary observer-local renderer.

Periodic aliases must share canonical blade identity and variation across the
seam. Dirty block, neighbor, biome, and light changes continue through the
ordinary section-revision system.

### GPU ownership and draw shape

`mclone-render` owns:

- immutable blade templates and grass pipelines in shared device resources;
- a per-world growable/range-managed patch instance arena;
- per-section instance ranges stored beside ordinary GPU section ranges;
- upload/removal/replacement integration with the existing section lifecycle;
- quality templates, draw planning, fog/light/color-profile parity, wind and
  interaction bindings, and counters; and
- direct, placed, clipped, per-eye, and full-frame multiview pipelines.

The first static path may issue one draw per visible grass-bearing section as
the topic recommends. Its arena and records must still support
`first_instance` and later LOD-grouped indirect batches. The Steam Deck
terrain result showed that per-section encode cost can dominate; if grass
draw encode becomes material, batch visible sections by template without
changing patch identity or buffers.

Render grass after opaque terrain and before actors. Use reversed-Z
`Depth32Float`, `GreaterEqual`, depth writes, no blending, and no face culling.
The grass pass loads existing color and depth. Transparent terrain remains in
the later composed translucent order.

Placed and clipped worlds use the same patch records and source-to-composition
mapping as terrain. Uniform placement scales blade geometry with its world.
Direct-world, embedded-world, warm-world, and world-replacement resource
lifetimes stay separate while immutable templates/pipelines are shared.

### Quality and LOD

Quality profiles select only presentation facts:

- enabled radius;
- near/middle/far blade count;
- vertical segment count;
- distance bands and hysteresis;
- blade height/width ranges;
- wind character; and
- whether interaction is enabled.

Exact values are measurement-driven. Begin conservatively and record every
accepted value in this tactical. Sparse is intentionally small enough for
evaluation on constrained devices but remains Off by default.

The accepted initial static profiles are:

| Tier | Radius | Near end | Middle end | Blades near/middle/far |
| --- | ---: | ---: | ---: | --- |
| Sparse | 64 blocks | 24 | 48 | 2 / 1 / 1 |
| Lush | 128 blocks | 48 | 96 | 6 / 4 / 2 |
| Ultra | 192 blocks | 64 | 128 | 8 / 6 / 3 |

All three profiles currently use the same one-segment tapered strip template.
They select a prefix of eight deterministic blades per patch, so tier and LOD
changes do not rebuild patch buffers. Band transitions use a four-block
hysteresis margin. Wind and interaction may add profile facts without changing
these static density values.

LOD selection is per presentation observer, not one mutable global section
tier:

- a mono/flat view has its own stable LOD history;
- both stereo eyes share one head-center draw plan;
- a full-frame multiview submission consumes that same shared plan;
- distinct split/auxiliary views cannot overwrite one another's state; and
- placed-world source views select distance after inverse placement.

Use hysteresis around section-distance band transitions. Patch buffers never
rebuild for an LOD change.

### Wind

Wind uses an original world-locked coherent field. Start without compute.
Choose analytic, generated sampled noise, or a hybrid only after comparing
shader cost and pixels. The final core must provide:

- stable broad gusts translated along a shared wind direction;
- deterministic per-blade resistance, phase, height, width, hue, and lean;
- root-fixed curved-spine deformation with increasing height influence;
- clump-scale correlation that does not reveal the block grid;
- skylight-based shelter attenuation if the packed signal is visually
  adequate; and
- identical source-world deformation in mono, both eyes, and multiview.

Camera motion must not relocate blades or change their resting character.
Time is shared scene presentation time and resets or rebases safely across
session replacement; it is not server gameplay time.

### Interaction history

Use a CPU-updated 128x128 RGBA8-equivalent field with world-space horizontal
bend direction and strength. A full row is 512 bytes, satisfying WebGPU's
common 256-byte texture-row alignment, so a first full upload remains simple
and bounded. Dirty-region uploads are an optional measured optimization.

History is keyed by world and presentation observer:

- stereo eyes share one head-centered field;
- separate split/auxiliary observers have independent fields;
- warm or embedded worlds cannot inherit the active world's trail;
- placed-world interactors are transformed into the source world;
- world/session replacement, resource rebuild, observer reassignment, or a
  discontinuous teleport clears the affected field; and
- periodic camera recentering shifts or wraps retained cells in canonical
  space rather than duplicating seam trails.

`mclone-scene` supplies neutral interactors containing position, footprint,
and stable presentation identity. Include the local player and presented
actors whose footprints are near a qualifying grass surface. Do not stamp
actors far above/below the surface. The renderer owns field pixels, decay,
upload, and shader sampling.

Off performs no field update. Sparse may disable interaction initially if its
measured cost or tiny radius makes the effect unhelpful.

## Instrumentation

Define counters before optimizing:

- compiled grass sections and patch instances;
- encoded/queued/uploaded patch bytes;
- resident patch ranges and bytes;
- visible grass-bearing sections;
- near/middle/far section and patch counts;
- estimated blades, template vertices, indices, and triangles;
- direct, indirect, and multi-draw call counts;
- grass CPU draw-plan/encode/update time;
- grass GPU pass time where timestamp queries exist;
- interaction fields, active cells, stamps, recenter events, resets, and
  uploaded bytes; and
- Off-path conservation counters, all exactly zero.

Project the useful subset through the existing scene/frame diagnostics rather
than adding app-local logs. Detailed one-off evidence belongs here or in the
performance records.

## Slice 0: Tactical, Baselines, and Contracts

- Add this tactical and link it from the tactical index.
- Rehydrate and hash-check the ignored pinned reference snapshot only if a
  later visual question requires it.
- Capture fresh release Off baselines:
  - deterministic offscreen section/face/resource counts;
  - frozen timedemo;
  - movement frame probe;
  - synthetic stereo/multiview resource proof; and
  - browser worker result-size baseline.
- Add focused failing/then-passing tests for patch eligibility, stable
  topology seed, packed layout, codec round trip, Off policy, and counters.

Exit: current performance and output are recorded before any enabled path can
draw, and the original-design rule is executable/documented.

## Slice 1: Patch Artifact and Static Cross-View Rendering

- Add request-gated section patch discovery and packed-result codec support.
- Integrate patch payload bytes into queue/admission estimates.
- Add shared templates, pipelines, per-world arena ranges, upload/removal, and
  direct draw planning.
- Implement direct mono, per-eye, full-frame multiview, placed, and clipped
  rendering together.
- Reuse terrain render options for packed light, sky darken, fog, color
  profile, topology periods, and composition mapping.
- Capture and inspect the first drawable static milestone before adding wind.

Required fixtures:

- plains/forest/swamp tint boundary;
- exposed versus covered grass;
- section and chunk boundary eligibility;
- periodic seam identity;
- direct and placed terrain;
- mono, side-by-side stereo, and multiview layers; and
- Off pixel/count invariance.

Exit: stable static blades attach to the right surfaces with correct tint,
light, depth, fog, culling, and cross-view geometry.

## Slice 2: Quality, LOD, UI, and Preference

- Add `GameGrassDetail`, UI state/action/effect, Graphics row, labels, and
  capability classification.
- Add the shared scene setting host and accepted-effect persistence.
- Extend schema-1 graphics preferences without changing its storage key or
  schema number; missing `grassDetail` defaults Off.
- Restore the choice on desktop flat/XR, flat Android, Android XR, and web
  through the existing storage adapters.
- Add Off/Sparse/Lush/Ultra profiles and per-observer hysteretic LOD state.
- Make Off release resources and stop compilation/update/draw work.
- Cover rapid choices and concurrent scene operations without leaking an
  error through strict desktop input handling.

Exit: one shared persisted setting changes real rendering on every host, and
each tier has stable measured transitions without patch rebuilds.

Implemented 2026-07-24:

- `GameGrassDetail`, the Graphics row, action/effect/capability projection,
  scene host, and schema-1 preference field are shared; the default and a
  missing field are Off.
- Off/non-Off changes control patch discovery and mark resident sections for
  one resource rebuild. Sparse/Lush/Ultra reuse the same arena and templates.
  Off immediately frees each world's grass arena and LOD histories.
- LOD histories are keyed by world draw owner and stable observer. Stereo and
  multiview share the head-center plan; placed worlds use inverse-mapped source
  positions; periodic worlds use shortest wrapped distance.
- Browser incremental compilation now decorates the same request as native
  with biome seed, topology, and grass policy. A diagnostic regression fence
  records the worker's decoded grass request bit.
- Shared render and frame reports expose resident/drawn patches, estimated
  blades, draw calls, bytes, uploads, and removals.

Accepted evidence:

- the render GPU tier fixture produced and inspected
  `/tmp/mclone-238-static-grass-{off,sparse,lush,ultra}.png`; changed-pixel
  counts increased strictly for Sparse, Lush, and Ultra;
- a native scene restored Lush from schema-1 preferences and the inspected
  `/tmp/mclone-desktop-offscreen.png` showed dense rooted grass across the
  ordinary app/runtime/scene path;
- headed Wayland browser WebGPU restored Lush, decoded an enabled worker
  request, retained `14,033` patches, drew `5,277` patches and an estimated
  `27,078` blades in `42` grass draws; the inspected
  `/tmp/mclone-native-web-app-canvas.png` showed the same effect; and
- focused UI, preference, scene, renderer, render-session, app-runtime, GPU
  pipeline, and GPU pixel tests passed. The complete affected package set and
  web build passed before the slice commit.

## Slice 3: Wind and Clump Character

- Add shared presentation time and wind facts.
- Implement and tune the original coherent wind field.
- Add stable blade/clump variation and root-fixed curved deformation.
- Add packed-skylight shelter response if accepted by pixels.
- Capture two fixed-camera times plus a short animation probe.
- Compare still-camera time change with moving-camera fixed-time output to
  distinguish intended motion from shimmer.
- Prove the same world points in per-eye and multiview layers.

Exit: the field reads as flexible, coherent grass rather than vibrating cards,
with no camera-relative popping or stereo disagreement.

Implemented 2026-07-24:

- `mclone-scene` supplies one monotonic presentation time, rebased every
  4,096 seconds for stable `f32` shader precision. Stereo and multiview use the
  same sampled time, and placed previews copy the active frame time while
  retaining their own source-world topology.
- Each world grass owner lazily allocates a 32-byte frame uniform only beside
  a non-empty patch arena. The uniform and all wind work disappear with the
  arena on Off or world release; terrain bind-group layouts are unchanged.
- Every direct, placed, clipped, per-eye, and multiview shader derives the
  broad and clump phases from canonical source-world coordinates. Placement
  happens after deformation, so miniature and translated worlds scale the
  same source motion instead of changing its character.
- The original blade template now has two tapered vertical segments. A
  deterministic resting lean, per-blade resistance and flutter combine with
  two spatially coherent gust bands. Deformation grows quadratically from a
  fixed root, and packed skylight attenuates wind to an 18-percent sheltered
  floor.
- Sparse/Lush/Ultra select conservative wind amplitudes of 0.10/0.14/0.17
  blocks without rebuilding patch data. Direction, field scales, speed, and
  flutter remain shared presentation facts.

Accepted evidence:

- focused render and scene suites passed with `176` render tests and `158`
  scene tests, plus the scene integration contracts;
- the ignored GPU gate materialized all six wind-aware pipelines, and the
  fixed-camera visual fixture changed `3,979` pixels between time 0 and
  3.25 seconds while retaining identical terrain;
- the inspected `/tmp/mclone-238-static-grass-lush.png` and
  `/tmp/mclone-238-wind-lush-t1.png` showed rooted coherent blades at the two
  fixed times;
- native full-frame and synthetic stereo captures passed and were inspected;
  the latter reported `250,370` differing eye pixels while showing the same
  wind state in both eyes; and
- headed Wayland `native:web:app-smoke -- --grass-detail lush` passed with
  `14,033` resident patches, `5,277` drawn patches, an estimated `27,078`
  blades, and `42` grass draws. Its inspected canvas showed wind-deformed
  blades through the browser/Web Worker/WebGPU path.

## Slice 4: Entity Bending and Recovery

- Add scene-neutral interactor collection and grass-surface filtering.
- Add per-world/per-observer interaction field ownership.
- Add footprint stamps, accumulated strength, decay, recentering, and reset.
- Compose interaction with wind without moving roots.
- Cover local player, another actor, different radii, stationary recovery,
  camera recenter, periodic seam, teleport, warm-world switch, placed world,
  auxiliary split, Local Play preview, and actors above/below grass.
- Verify Off and disabled-interaction tiers upload zero field bytes.

Exit: nearby bodies bend and leave recovering grass trails without affecting
gameplay state or another world/view's history.

Implemented 2026-07-24:

- `mclone-scene` supplies a bounded, host-neutral set containing the local
  player and stable remote-player/entity footprints. Anonymous presentation
  actors are ignored, and duplicate identities update their existing entry.
- Each grass world retains the real resident patch roots for a height-aware
  qualification check. A moving body contributes interpolated samples only
  near those roots, so actors well above or below grass do not stamp.
- Four renderer-owned 128x128 RGBA8 fields cover the maximum four
  presentation observers at 0.5 blocks per cell. Stereo and multiview use one
  head field; separate flat views use separate fields; separate world draw
  stores cannot share trails.
- Direction and strength combine radial footprint pressure with movement.
  Strength recovers exponentially with a 0.85-second time constant. Small
  camera moves shift retained cells, topology changes and large teleports
  reset them, and periodic worlds lift camera, interactor, and shader sample
  coordinates across seams.
- Lush and Ultra enable interaction. Sparse uses the shared empty binding and
  reports zero fields/uploads. Returning to a disabled tier clears active
  field history before reuse; Off still releases the complete grass owner.
- Bounded placed worlds center the field on their source bounds, while
  unbounded placed worlds use the inverse-mapped source observer. Preview
  actors already supplied in source coordinates therefore bend miniature
  grass even when physical placement scale would put the source camera beyond
  the 64-block field.
- All six direct/placed/clipped mono/multiview shaders sample the field in
  canonical source space, compose interaction with wind using the same
  quadratic root-fixed influence, and apply placement afterward.
- Render, frame, and browser reports expose field, active-cell, stamp,
  recenter, reset, and upload-byte counts. One active field uploads one
  aligned 65,536-byte image per distinct presentation time.

Accepted evidence:

- focused render, scene, and app-runtime suites passed; shader-source
  contracts cover every pipeline variant and the scene identity fixture
  covers local, remote, entity, duplicate, and anonymous actors;
- ignored GPU lifecycle tests passed for contact, recovery, recentering,
  periodic seams, height rejection, and all six pipeline layouts;
- the inspected `/tmp/mclone-238-interaction-contact.png` and
  `/tmp/mclone-238-interaction-recovery.png` show the stamped and recovered
  poses, with the latter verified against an unstamped time-matched control;
- the placed fixture reported 81 drawn patches, one interaction field, active
  cells, and a 65,536-byte upload. Its inspected mono and side-by-side stereo
  captures show miniature grass, and its full-frame multiview path passed on
  the local adapter; and
- headed Wayland `native:web:app-smoke -- --grass-detail lush` passed with one
  field, 54 active cells, eight stamps, 65,536 uploaded bytes, 14,033 resident
  patches, 4,190 drawn patches, about 21,788 blades, and 32 grass draws. The
  inspected canvas showed grass through the complete browser worker/WebGPU
  path.

## Slice 5: Platform and Performance Closeout

Run focused gates after every owning slice, then the affected matrix:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml \
  -p mclone-mesh \
  -p mclone-render-session \
  -p mclone-render \
  -p mclone-app-runtime \
  -p mclone-ui \
  -p mclone-scene
cargo test --manifest-path native/Cargo.toml
pnpm native:thin-adapters:purity
pnpm native:desktop-offscreen:smoke
pnpm native:xr-emulation:smoke
pnpm native:movement:smoke
pnpm native:timedemo:smoke
pnpm native:web:build
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
pnpm native:web:movement-perf
pnpm native:android:apk
pnpm native:android-xr:apk
git diff --check
```

Before browser pixels, run `pnpm host:check` and use headed Wayland WebGPU.
Never accept a black/transparent headless Chrome capture.

Save and inspect screenshots and animation frames only under `/tmp`. Minimum
pixel evidence:

- Off and every enabled tier from one fixed mono camera;
- biome/tint and LOD-boundary fixtures;
- two wind times;
- interaction contact and recovery;
- side-by-side stereo;
- full-frame multiview layer comparison;
- placed terrain;
- auxiliary/Local Play split;
- headed desktop and mobile-sized browser WebGPU; and
- the smallest Android/device capture available through documented lanes.

Compare Off/Sparse/Lush/Ultra with:

- frozen and movement average/p50/p95/tail frame time;
- grass GPU time;
- patch/template/resident/upload bytes;
- visible sections and draws;
- browser worker payload and compile time; and
- first-use pipeline/resource materialization.

Physical Quest evidence is required only before enabling a non-Off Quest
default. If no Quest is attached, Android XR build and synthetic/full-frame
multiview evidence can close the Off-default implementation while physical
Sparse acceptance remains explicit follow-up. Do not infer headset performance
from AVD, desktop, or synthetic stereo.

## Commit Plan

Use `Topic: lush-grass-rendering` on every feature commit.

1. Tactical and baseline contract.
2. Patch artifact, eligibility, codec, and Off-path tests.
3. Static shared GPU renderer and cross-view pixels.
4. Quality/LOD/settings/preference.
5. Wind and clump deformation.
6. Interaction field and lifecycle.
7. Platform/performance evidence and documentation closeout.

Each commit must be independently formatted and tested at the narrowest
affected boundary. Do not commit unrelated existing worktree changes.

## Completion Conditions

- Off compiles, uploads, draws, and updates zero grass work.
- Enabled sections contain one compact patch per eligible grass-block surface.
- Patch identity and variation are stable across frames, rebuilds, workers,
  negative coordinates, and periodic aliases.
- Biome tint and packed light reuse the existing Java-shaped mesh facts.
- Sparse/Lush/Ultra change templates and admission without patch rebuilds.
- LOD transitions are stable for mono, shared-head stereo, multiview, placed,
  and simultaneous split observers.
- Wind is coherent, root-fixed, camera-stable, and stereo-consistent.
- Player/actor trails bend and recover while remaining isolated by world and
  observer.
- Direct, placed, clipped, mono, per-eye, full-frame multiview, browser,
  Android, and XR compile/render boundaries consume shared policy.
- `Grass Detail` persists through schema-1 graphics preferences and remains
  independent from Leaf Detail.
- The implementation contains no copied Grassier Grass source or resources.
- Captured pixels are manually inspected at each drawable milestone.
- Measured costs and final default decisions are recorded in the living topic.

## Stop Conditions

Stop and correct the architecture if:

- gameplay/server/worldgen state acquires blade or trail data;
- Off still scans, serializes, uploads, draws, or updates grass;
- desktop, browser, Android, or XR gains private grass policy;
- one mutable LOD or trail state is shared by distinct split observers;
- stereo eyes generate different blade geometry or interaction displacement;
- periodic aliases reroll blade identity at the seam;
- patch quality changes rebuild per-blade CPU geometry;
- a parallel worker/cache/upload scheduler appears beside render-session;
- placed or warm worlds borrow the active world's GPU/trail lifetime;
- per-section draws materially undo the measured terrain batching gain and no
  batching correction is attempted; or
- optional plants/snow/particles delay core static/wind/interaction closeout.
