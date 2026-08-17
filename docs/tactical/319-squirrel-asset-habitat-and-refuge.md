# Tactical 319: Squirrel Asset, Habitat, and Refuge

Status: implementation complete 2026-08-17; Review Gate D awaits one capable
physical-XR or exact full-frame multiview pixel run

Parent: [`315`](315-authoritative-seasonal-calendar-and-squirrel-ecology.md)
Phase D

Topic: `habitat-driven-creature-ecology`

Topic: `wildlife-ecology-state-model`

Topic: `semantic-figure-assets`

## Instruction Synthesis

Introduce squirrels as ordinary durable woodland wildlife after the existing
rabbit, deer, and mallard resource campaign is re-accepted. Reuse and refine
the approved canonical Red Squirrel source rather than authoring a second
competing squirrel. Promote that one semantic asset through the shared
first-party actor pipeline, add host-neutral entity and presentation
vocabulary, derive initial populations from coordinate-pure woodland evidence,
and implement observable ground forage, cover escape, and one honest bounded
tree-refuge route.

This phase establishes the animal, habitat, movement, and first runtime visual
review. It does not add carried mast, world cache records, cache knowledge,
seasonal reproduction, unloaded ecology, a general arboreal navigator, or a
showcase-only squirrel. Tactical 315 Phase E owns cache transfers and seasonal
lifecycle after Review Gate D accepts the animal and refuge behavior.

## Asset And Runtime Promotion

Refine `tools/asset-lab/examples/red_squirrel/figure.ts` in place. Preserve its
accepted deep haunches, tufted ears, compact forequarters, and large articulated
plume tail. Add explicit land/woodland animal metadata and authored clips for:

- relaxed idle/look;
- ordinary ground bound;
- fast escape;
- forage/eat;
- carry posture;
- cache deposit/dig and retrieve actions for later Phase E selection;
- trunk climb and refuge entry/exit; and
- an alarm/social reaction used by the bounded sound contract.

Phase D gameplay must select idle, bound, escape, forage, climb/refuge, and
alarm clips from authoritative state. Phase E may select the already-reviewed
carry and cache actions without reopening the figure. Action endpoints must
compose with their next idle or locomotion pose; ground and surface gates must
sample every clip.

Promote the exact generated JSON as `mclone:red_squirrel` through the existing
checked actor registry and required first-party pack. Shared asset preparation,
render-session mapping, and actor rendering own mono, per-eye, synthetic
stereo, and full-frame multiview presentation. No squirrel mesh, animation
policy, or platform mapping belongs in an app crate.

Add only a small provenance-locked sound family whose cues communicate alarm,
rapid escape, or digging/refuge work. Calls remain spatial, finite-range, and
suppressed across nearby squirrels. Silent cache actions are not acceptable in
Phase E, but a large ambient-sound campaign is outside this slice.

## Shared Entity Contract

Add a stable `Squirrel` entity kind and compact replicated biology/behavior
record through protocol, client replica, server persistence, collision,
presentation, and diagnostics. Phase D state includes:

```text
SquirrelState {
    sex,
    age_ticks,
    life_stage,
    condition,
    behavior,
    behavior_epoch,
    retained_intent,
    refuge,
}
```

Use the common persistent identity, life-stage, update, saved-entity, and named
clip contracts. Do not encode cache inventory before Phase E owns the complete
transfer boundary. Revision bumps are deliberate because current saves and
fixtures are internal and unshipped.

The initial authoritative behavior vocabulary is bounded to idle/look,
forage/eat, ground travel, alarm, escape, trunk approach, climb, refuge rest,
and refuge exit. Orientation derives from actual displacement. A blocked route
expires to an honest ground state rather than rewriting yaw, teleporting, or
claiming refuge success.

## Woodland Habitat And Initial Population

Add one production habitat query shared by planning and live validation. It
combines coordinate-pure generated facts with current loaded block evidence:

- woodland edge or broken-canopy cover rather than dense featureless interior;
- nearby mast-bearing mature woody vegetation;
- a live seed/soft-mast resource opportunity at the 64-by-64 forage cell;
- at least one bounded cavity/refuge candidate in connected woody cover;
- low herb/ground access for travel and ordinary collision-safe standing;
- slope and obstruction bounds; and
- current player distance/disturbance for natural materialization.

Prefer existing Mclone vegetation and forest-edge facts. If a cavity semantic
is missing, derive a deterministic sparse candidate from the same production
tree occurrence identity and prove that its live trunk/leaf support exists;
do not add a squirrel-only noise field or change heightfield shape.

Extend the coordinate-pure initial-population planner with bounded squirrel
families and stable IDs. Plane and cylindrical topology must agree at wrapped
coordinates, planning must be request/order independent, one owner chunk must
realize each identity, and an explicitly saved empty entity record must retain
local extinction. Natural materialization uses ordinary entity-load readiness,
creature cap, light, collision, and player-distance rules.

## Ground Forage, Escape, And Honest Refuge

Squirrels consume only compatible reached `SeedsAndSoftMast` stock in this
phase. Feeding uses the shared seasonal accessibility and active recovery
rules accepted by Tactical 318. No carried stock exists yet: a reached bite is
immediate intake, and every unit remains covered by the existing resource
ledger conservation accounting.

Alarm first selects nearby real cover. Continued pressure selects a refuge
candidate through one bounded neighborhood query. The specialized route is:

```text
ground position
  -> collision-valid trunk approach
  -> support-checked vertical trunk segments
  -> support-checked branch/cavity threshold
  -> retained refuge position
```

Every segment requires loaded block/collision/support facts and bounded
progress. Vertical motion is gradual authoritative displacement with a climb
clip and stable facing; entry succeeds only at the validated threshold. Missing
or changed support invalidates the route. Refuge exit reverses a validated
route or returns to the nearest still-valid support. This is a squirrel-focused
state machine over ordinary world facts, not a universal tree-home graph,
flight path, noclip mode, or teleport.

## Validation And Review Gate D

Required automated evidence:

- exact source-to-generated-JSON drift, metadata, clip vocabulary, action
  endpoints, geometry connectivity, sampled ground, and surface stability;
- required first-party resource preparation and actor lookup;
- protocol/update/entity-save round trips for every squirrel state;
- coordinate-pure plane/cylinder habitat and initial-population planning,
  owner realization, empty-record extinction, persistence, and order
  independence;
- habitat accept/reject fixtures for useful woodland edge, mast, cavity,
  dense interior, open plain, steep land, missing live tree facts, and player
  disturbance;
- reached seed/mast intake with exact shared-ledger conservation;
- bounded alarm-to-cover and complete approach/climb/refuge/exit sequences,
  plus support-loss and unavailable-chunk failure;
- rabbit/deer/mallard regression and thousand-animal graceful-overload tests
  including squirrel/refuge demand; and
- full workspace/Wasm checks with no app-local gameplay growth.

Pixel and behavior review must inspect the same ordinary runtime actor in Asset
Lab, native mono, headed browser WebGPU, synthetic stereo, and a real or exact
full-frame multiview lane. At least one bounded uncommanded window must show
ground forage, alarm/escape, and a physically completed refuge sequence.

Review Gate D accepts only if the promoted animal reads clearly as a squirrel
at ordinary game scale, uses woodland structure rather than rabbit behavior,
keeps the tail and action timing legible, and never claims a refuge without a
valid loaded route. Cache implementation begins only after that gate is
recorded.

## Implementation Order

1. Record the existing Red Squirrel source, single clip, and prepared counts.
2. Add metadata and the complete reviewed Phase D/Phase E clip vocabulary.
3. Promote the checked actor and prove shared preparation/presentation.
4. Add protocol, client, authoritative entity, persistence, and diagnostics.
5. Add the pure habitat/refuge query and coordinate-pure initial planner.
6. Materialize durable natural squirrels through ordinary population rules.
7. Add ground forage, cover escape, and the support-checked refuge route.
8. Add the bounded sound family and authoritative cue selection.
9. Run focused, overload, cross-platform, pixel, and behavior acceptance.
10. Record Review Gate D in the parent and living topics before Phase E.

## Completion Checklist

- [x] Refine and visually accept the canonical Red Squirrel asset and clips.
- [x] Promote exact generated JSON through the shared first-party actor path.
- [x] Add shared protocol, lifecycle, persistence, client, and render mapping.
- [x] Add bounded woodland/mast/cavity habitat sampling.
- [x] Add coordinate-pure initial population planning and ordinary spawning.
- [x] Add conserved reached ground forage.
- [x] Add alarm, cover escape, and honest tree refuge entry/exit.
- [x] Add and validate the restrained first-party sound family.
- [ ] Pass overload, native/WebGPU/stereo/multiview, and behavior gates.
- [ ] Record Review Gate D and hand Phase E the accepted actor vocabulary.

## Implementation And Review Record

Commits `f0b31a1a`, `1cbf9aff`, `f074970a`, and `4ded90b0` implement the
Phase D owner path. Commit `8e79ba41` adds the focused catalogue acceptance.
The promoted `mclone:red_squirrel` record is generated from the one canonical
Red Squirrel source and contains 18 parts, 7 materials, 3 textures, and 12
clips. Its SHA-256 is
`56ff729ce4ef312c4b70e0293bfb533437cea80244adf020f57b5bd5797fd92e`.

The server now persists and replicates squirrel sex, age, life stage,
condition, behavior, behavior epoch, retained intent, and refuge state. The
revision-2 initial-wildlife planner selects bounded woodland groups, and the
live realization path searches only the owner chunk for a real mast site and
a support-valid route sized to the 0.62-block animal. Realization defers while
its bounded 3-by-3 loaded evidence is unavailable. The pinned planner receipt
remains `(1,500,4,279,[750,254,151,186,159],1,536,547)`. A production-seed
fixture at seed `-98765`, center chunk `-45,-43`, radius 2 realizes four
squirrels.

Reached ground feeding conserves `SeedsAndSoftMast` through the shared
resource ledger. Threatened squirrels select cover, approach a real trunk,
climb through gradual support-checked displacement, enter only a validated
refuge threshold, rest, and reverse the route on exit. Losing support falls
back to an honest ground behavior. The expensive refuge query uses the shared
ecology-work admission class: one thousand continuously threatened squirrels
perform no more than 32 refuge queries per tick and every identity progresses
within 64 ticks. Spatial alarm, escape/rustle, and digging/refuge cues use the
bounded first-party sound path.

The data-only `squirrel-woodland` revision-1 showcase uses seed `17504` and
ordinary saved squirrel state. Its typed evidence names the production founder
path, and its authored tree passes the same support-valid refuge query as live
gameplay. Native flat and synthetic-stereo captures were inspected at
`/tmp/mclone-squirrel-woodland.png` and
`/tmp/mclone-squirrel-woodland-stereo.png`; the stereo layers contain 318,952
different pixels. Headed WebGPU observed all of `Idle`, `Bound`, `Forage`,
`Alarm`, `Flee`, `TrunkApproach`, `Climb`, `RefugeEnter`, and `RefugeIdle`
over 424 samples, including a squirrel moving from height 65 to 68. All
IndexedDB world-record stores remained empty. The inspected browser canvas is
`/tmp/mclone-native-web-showcase-squirrel-woodland-canvas.png`.

All 12 clean Asset Lab sheets were regenerated under
`/tmp/mclone-squirrel-gate-d/` and inspected together in `montage.png`.
Haunches, tufted ears, and the plume tail read clearly; action endpoints retain
ground/surface stability. The focused production-built Animal Catalogue tests
pass for desktop and 390-pixel mobile views. The inspected captures are
`/tmp/mclone-animal-catalogue-desktop.png` and
`/tmp/mclone-animal-catalogue-mobile.png`.

Focused server, protocol, client, asset, audio, render, world-generation, and
showcase tests pass. The server library run passed 777 tests; one unrelated
native-runner queue-depth case was flaky in the aggregate and passed on its
isolated rerun. `mclone-render` passed 206 tests with 11 ignored. The ignored
actor-composition GPU fixture exercised the actual squirrel and `bound` clip,
with 2,238 actor-pixel and 1,778 eye-pixel differences in mono/per-eye output.
The current Mac adapter does not expose `wgpu::Features::MULTIVIEW`, so the
same fixture truthfully reported its capable-device branch unavailable.

Review Gate D therefore remains open on exactly one external-device item. No
authorized Quest was attached on 2026-08-17. A Windows testbed booted but its
administration channel was unavailable; the ready Linux testbed had neither
the project toolchain nor a shared checkout. Both were parked after diagnosis.
Do not infer a multiview pixel pass from the compiled pipeline or from
synthetic per-eye stereo. Run the ordinary squirrel actor on a Quest or another
adapter exposing `MULTIVIEW`, inspect both layers, then record Gate D before
Phase E cache code begins.
