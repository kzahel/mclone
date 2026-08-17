# Tactical 319: Squirrel Asset, Habitat, and Refuge

Status: drafted 2026-08-17; implementation waits for Tactical 318 Review Gate C

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

- [ ] Refine and visually accept the canonical Red Squirrel asset and clips.
- [ ] Promote exact generated JSON through the shared first-party actor path.
- [ ] Add shared protocol, lifecycle, persistence, client, and render mapping.
- [ ] Add bounded woodland/mast/cavity habitat sampling.
- [ ] Add coordinate-pure initial population planning and ordinary spawning.
- [ ] Add conserved reached ground forage.
- [ ] Add alarm, cover escape, and honest tree refuge entry/exit.
- [ ] Add and validate the restrained first-party sound family.
- [ ] Pass overload, native/WebGPU/stereo/multiview, and behavior gates.
- [ ] Record Review Gate D and hand Phase E the accepted actor vocabulary.
