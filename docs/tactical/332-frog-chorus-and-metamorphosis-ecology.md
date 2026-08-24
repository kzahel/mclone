# Tactical 332: Frog Chorus And Metamorphosis Ecology

Status: **proposed 2026-08-23; selected as the next original creature chapter
after Tactical 315 closes its squirrel cache, seasonal lifecycle, and
cross-platform acceptance gates.**

Topics:

- `habitat-driven-creature-ecology`
- `wildlife-ecology-state-model`
- `playable-showcases`
- `semantic-figure-assets`
- `first-party-sound-effects`
- `seasons`

## Instruction Synthesis

Add frogs next. Promote the accepted Asset Lab frog into ordinary Mclone
Overworld gameplay through a complete, bounded ecology chapter rather than a
generic passive spawn. Frogs should make wetland edges legible through real
movement, feeding, calls, reproduction, persistence, and player discovery.
Reuse the authoritative calendar, seasonal resource opportunity, habitat
fitness, durable wildlife lifecycle, spatial sound, figure animation, and
showcase paths already established by mallards, rabbits, bees, deer, and
squirrels.

The chapter must remain deliberately smaller than fish or beaver. It does not
open general underwater navigation, dynamic seasonal hydrology, or arbitrary
animal-built terrain. Its new pressure tests are an animal that alternates
between ground and shallow water, an emergent local chorus without a permanent
social group, and one material aquatic brood whose loaded progression ends in
durable froglets without duplicating identities.

## Product Question

Can a player hear a real evening chorus, follow it to a suitable wetland,
watch frogs hop between cover and shallow water, find a developing brood, and
later observe froglets emerge, with every fact produced by the same ordinary
authoritative simulation that persists across native, Web, flat, and XR
clients?

## Decision

Select frogs as the sixth original creature-life chapter and the immediate
follow-up to Tactical
[`315`](315-authoritative-seasonal-calendar-and-squirrel-ecology.md).
Fox remains the first intended predator after bottom-up resource and prey
surplus are measured. Fish and beaver remain larger later water-system
pressure tests.

Frogs are a useful next step because the required foundations already exist:

- mallards prove water/shore habitat, shallow-water occupancy, calls, nests,
  offspring, and aquatic-resource consumption;
- rabbits prove small-animal hops, urgent player avoidance, bounded place
  queries, and durable family outcomes;
- the wildlife ledger already exposes `AquaticInvertebrates` and exact
  seasonal accessibility/recovery;
- the 56-day calendar provides authoritative time and local seasonal climate
  without another ticking clock; and
- the accepted 19-part Frog figure already has reviewed `hop` and one-shot
  `jump` clips, but is not yet a runtime asset.

The implementation must reuse those contracts without turning a frog into a
small mallard or a green rabbit. Chorus coordination, water-edge escape, and
the material spawn-to-froglet transition are the species-defining results.

## Objective

Deliver this ordinary live-world loop:

```text
generated shallow wetland + bank cover + aquatic invertebrates
  -> a small durable frog population
  -> land/shore foraging and shallow-water refuge
  -> dusk calls from actual nearby adults
  -> a bounded courtship and compatible shallow-water brood site
  -> visible spawn and tadpole-school phases while loaded
  -> idempotent release of durable froglets
  -> field observations that teach time, habitat, and life cycle
```

Population change remains an outcome of individual condition, physically
reached food, successful reproduction, loaded development, and mortality. The
tactical must not add a desired frog count, wetland refill rule, or invisible
unloaded catch-up simulation.

## Starting Point

- `tools/asset-lab/examples/frog/figure.ts` is an approved canonical box-only
  amphibious figure with explicit `hop` and `jump` animation roles.
- `mclone-server` already owns coordinate-pure initial wildlife geography,
  shared habitat fitness, five renewable resource strata, individual loaded
  lifecycles, remains, bounded ecology work, and immediate durable identity.
- `AquaticInvertebrates` is already a seasonal resource consumed at compatible
  water or shore positions. Frogs extend that shared stratum rather than add a
  species-named food ledger.
- Mallards already prove authoritative shallow-water movement, spatial calls,
  persistent nest progression, and field observations. The implementation
  should extract only genuinely shared mechanics and retain species policy in
  frog-owned state.
- Active weather is not yet an authoritative gameplay producer. Rain can
  become a later modifier, but the first frog loop must be complete from
  wetland geometry, civil time, local season, food opportunity, and current
  loaded state alone.
- Tactical 315 still owes squirrel caches, cache conservation, compressed
  seasonal reproduction, and full acceptance. Frog implementation begins
  after those gates close so population evidence and platform review do not
  overlap two unfinished creature chapters.

## Binding Decisions

### Frogs are durable individuals

A materialized frog immediately receives an ordinary persistent identity and
uses the same entity-chunk load, save, unload, and death rules as the existing
Mclone wildlife. Leaving and returning cannot reroll its sex, age, condition,
behavior epoch, reproductive cooldown, parentage, or life stage. A saved empty
entity record prevents coordinate-pure initial population planning from
resurrecting a depleted local population.

There is no separate ambient-frog system. Distant calls, visible bodies,
feeding, reproduction, and discovery originate from material entities in the
active world.

### One actor crosses a bounded land/water seam

The first frog may hop on supported ground and banks, occupy the existing
shallow-water movement envelope, and select reachable destinations on either
side of a compatible shore. It may not receive free 3D swimming, deep-water
pathfinding, flight, collision bypass, or a frog-only navigator.

Ground routes use the shared collision and path-request service. Water/shore
eligibility reuses or narrowly extracts the mallard occupancy facts. The
one-shot `jump` clip is presentation for a realized authoritative movement or
escape action; it does not teleport the actor over a blocked route. Failed or
invalidated destinations clear or replan through the existing bounded work
admission path.

### Chorus is local coordination, not ambience or a colony

An adult may call only when its current loaded state permits it: an eligible
dusk/evening window, compatible wetland habitat, sufficient condition, no
immediate threat, and a deterministic individual cooldown. One frog may call;
nearby eligible adults may answer through a bounded spatial query and staggered
admission. No permanent chorus entity, leader, shared blackboard, or global
wetland scan is created.

Every audible croak is an authoritative lossy cue carrying frog identity,
position, kind, sequence, and finite range. Local admission caps simultaneous
voices without synthesizing a detached biome soundtrack. Calls stop or change
when a nearby player causes a real flee response. Client audio failure cannot
change simulation state or discovery authority.

### Frogspawn is one material bounded brood

Reproduction creates a durable `FrogBrood` at one validated shallow-water site.
It is a visible material habitat feature, not a population summary or a hidden
list of tadpole entities. The first brood persists:

- stable identity, position, and site revision;
- parent persistent identities;
- current `Spawn`, `TadpoleSchool`, or `FrogletReady` stage;
- loaded development progress and habitat-deficit progress;
- a deterministic release serial and planned child identities; and
- terminal released or failed state needed to prevent replay.

Development advances only while the brood's entity chunk and required blocks
are active. Valid shallow water, support, cover, and local seasonal opportunity
gate progress. Temporary unsuitable loaded conditions pause or accumulate a
bounded failure deficit; unavailable chunks do neither. Removing or replacing
the required water/support invalidates the site exactly once when that state is
actually observed loaded.

The `TadpoleSchool` is presented as one bounded brood prop. This tactical does
not make every tadpole a separately ticking actor. At readiness, one
authoritative idempotent release creates a small hard-bounded number of durable
froglet entities using the recorded child identities, then marks or removes
the brood so hydration and retry cannot duplicate them. Froglets use the Frog
entity kind, a persisted life stage, a smaller reviewed presentation scale,
and loaded maturation into adults.

### Reproduction is seasonal opportunity, not a calendar event

Two compatible conditioned adults may form a temporary courtship near a valid
brood site during a broad locally warm reproductive opportunity. The exact
curve derives from the authoritative local seasonal sample and is calibrated
against the 56-day year; it is not a one-day annual boundary event.

Sleep, date changes, and other civil-time discontinuities change current
conditions but execute no skipped courtship, brood development, maturation,
feeding, birth, or resource recovery. No breeding receipt is scheduled for an
unloaded future date. True hibernation, overwintering below terrain, and
rain-triggered breeding remain deferred.

### Frogs consume the existing aquatic resource

Frogs use a species-owned diet beginning with `AquaticInvertebrates`. Intake
requires a physically reached compatible shallow-water or bank position and
the existing resource ledger's current accessible stock. A remote cell balance
cannot feed a frog.

Frogs therefore compete honestly with mallards in wet windows. The long
population campaign must measure both species, condition, suppression,
consumption, births, brood outcomes, and mortality. It may tune individual
maintenance, intake, cooldown, or brood size, but may not add a food floor,
species quota, forced refill, or acceptance target for a preferred population
ratio.

### Habitat comes from blocks and shared terrain facts

Add a frog habitat sample over existing fitness vocabulary. It reads current
loaded blocks and measures at least:

- stable shallow water and water/land adjacency;
- reachable low banks and supported landing cells;
- reeds, lilies, grass, logs, or other truthful nearby cover;
- aquatic-invertebrate potential and current accessibility;
- low local slope and connected short-range movement space;
- current disturbance and nearby frog/brood occupancy; and
- local seasonal thermal opportunity.

The coordinate-pure initial wildlife planner may select a small frog founder
group only from broad wetland and cover evidence, then first realization must
validate live blocks. A dry campaign control produces exactly zero frogs and
zero broods. Each opted-in original Mclone profile must consume its own
published terrain facts; do not hard-code V1 biome identities into V2 or alter
retained Java-shaped profiles. If ordinary Mclone wetlands lack readable
cover, improve the shared wetland/shore recipe through the world generator
rather than consult a hidden frog-only noise field or patch showcase terrain
into production logic.

### The player loop is observation and stewardship

Frogs do not need an extractive drop or taming mechanic in this chapter. Their
first player loop is to hear, locate, approach, disturb, observe, and protect a
wetland life cycle. Ordinary edits matter: removing cover reduces habitat,
blocking a shore route changes movement, and destroying brood water/support
can fail that brood.

Persist six frog field observations:

1. see an adult frog;
2. hear a call from a real frog;
3. hear two distinct nearby frogs participate in one chorus window;
4. witness a frog escape from land or bank into shallow water;
5. find a material brood in its spawn or tadpole stage; and
6. witness an idempotent froglet release or later froglet maturation.

The field-note HUD is supportive evidence. Calls, movement, scale, brood-stage
presentation, and actual froglets must communicate the loop without requiring
debug prose.

### Assets and sound are part of promotion

Promote the canonical figure as `mclone:frog` only after review. Retain the
accepted `hop` and `jump` clips, then add or refine the smallest set needed for
runtime behavior:

- `idle` for breathing and alert pauses;
- `hop` for distance-driven ground locomotion;
- `jump` for one-shot escape emphasis;
- `swim` for realized shallow-water displacement;
- `forage` for physically reached intake; and
- `call` for a clearly readable throat/body pulse.

Author one checked semantic brood presentation with legible spawn and tadpole
stages. Use a shared named-clip or semantic-variant path selected at an early
implementation checkpoint; do not add a frog-only renderer, raw mesh, or
per-eye mutable animation state.

Add a small rights-clean first-party croak family with individual and response
variants. Listen to isolated cues and a bounded multi-frog sequence before
runtime acceptance. The sound bank, asset inventory, provenance, first-party
pack, and missing-resource tests must remain strict.

### Persistence and work remain bounded

Frog state, brood state, field observations, resource transfers, and release
identities use shared server ownership and the ordinary persistence interface.
SQLite and IndexedDB must restore the same frogs and brood stage. A release
retry or save/reopen boundary cannot produce a second child with the same
planned identity or silently lose a completed release.

Per tick, chorus-neighbor queries, brood-site queries, feeding checks, and path
requests use bounded spatial neighborhoods and the existing fair work budgets.
Do not:

- scan every wetland or saved brood when dusk begins;
- retain a scheduler task per frog or brood;
- advance or reconcile frogs while their region is inactive;
- poll every candidate water block for every frog every tick; or
- let early unroutable frogs starve later identities of path or decision work.

The thousand-animal overload fixture must include frog movement, chorus, and
brood demand before extracting another general ecology API.

## Authoritative Behavior Vocabulary

Add a narrow replicated behavior vocabulary whose names select reviewed
presentation but do not move policy to clients:

| Behavior | Meaning and required outcome |
|---|---|
| `Idle` | supported rest, breathing, and bounded alert orientation |
| `Hop` | ordinary collision-checked ground or bank travel |
| `Forage` | approach and physically consume accessible aquatic invertebrates |
| `Swim` | realized displacement inside the supported shallow-water envelope |
| `Call` | one admitted authoritative croak with finite spatial range |
| `Courtship` | temporary proximity and site selection without permanent pairing |
| `FleeToWater` | urgent reachable route away from a threat toward compatible shallows |
| `LayBrood` | complete a validated courtship at a reached compatible shallow-water site |

`FleeToWater` remains an immediate safety action. A remembered or nearby brood
cannot override a reachable away-side escape, and a frog is not required to
return to one permanent pond. Parent identities are lineage and validation
facts, not an attendance requirement or a universal home pointer; adults
resume ordinary behavior after the brood is created.

## Playable Showcase Contract

Add a deny-unknown-fields `frog-chorus` recipe only after every composed fact
has an ordinary live producer and compatible typed evidence. It may establish
one readable shallow wetland, three or four durable adults, cover, and one
near-transition brood record. It may set a dusk civil time and bounded saved
progress so the normal loaded simulation can expose the full loop promptly.
It may not script calls, movement, feeding, threat response, stage transition,
release, observation, or sound playback.

Acceptance requires both composition and behavior:

- the first frame clearly shows frog scale, water edge, cover, and a readable
  brood without labels or debug overlays;
- a bounded authoritative window records calls from at least two distinct frog
  identities, meaningful frog displacement, one land-to-water transition,
  one physically reached feed, one brood-stage transition, and one
  non-duplicated froglet release;
- an ordinary player approach suppresses or interrupts a caller and produces
  material separation or water escape without commanding the subject;
- native and headed-WebGPU desktop/phone runs use the same recipe revision,
  seed, entry pose, civil time, and state receipt;
- synthetic stereo and a physical XR or exact full-frame multiview run show the
  adult, brood, and froglet presentation in every view; and
- the fresh Web showcase leaves every IndexedDB world-record store empty.

The recipe is a review save, not the first or only source of frogs, broods,
choruses, or froglets. Run headed WebGPU acceptance lanes sequentially on one
GPU host and inspect pixels at the first drawable actor/brood milestone and
again after behavior is complete.

## Phases And Review Gates

### Phase 0: contract and baselines

1. Record this tactical and update the living ecology state.
2. Pin the accepted Frog source, semantic JSON, animation sheets, and catalogue
   classification as the pre-promotion baseline.
3. Record current wet/dry resource and frog-candidate geography across the
   existing deterministic population windows without changing their accepted
   squirrel-era revisions.

**Review Gate A:** accept the scope, adult silhouette, brood-stage visual
direction, and the explicit exclusion of rain, deep-water navigation, and
individual tadpole actors.

### Phase 1: assets, actor, habitat, and movement

1. Refine the Frog animation set and author the brood presentation.
2. Add shared protocol/entity/life-stage/behavior state, first-party asset
   loading, client replication, named clip selection, and ordinary prepared
   rendering.
3. Add coordinate-pure frog geography, live habitat validation, durable
   founder realization, and Terrain Lab wildlife diagnostics.
4. Implement bounded ground/shore hopping, shallow-water occupancy, forage
   travel/intake, and player-triggered water escape.
5. Prove persistence, dry rejection, order independence, habitat edits,
   collision, bounded work, and native/Wasm shared ownership.

**Review Gate B:** inspect Asset Lab, native flat, headed WebGPU, and synthetic
stereo pixels. In an ordinary world, frogs must read as small amphibious
animals using a water edge, not rabbits recolored green or mallards standing
on water.

### Phase 2: chorus, sound, and discovery

1. Add authoritative individual call state and lossy spatial cues.
2. Add bounded local response admission, threat interruption, and deterministic
   cooldowns without a chorus entity or global scan.
3. Add the first four field observations from ordinary sight, sound, grouping,
   and escape evidence.
4. Generate or curate, provenance-lock, listen to, and integrate the croak
   family across shared audio paths.

**Review Gate C:** a bounded ordinary dusk window must produce a legible
multi-identity call-and-response sequence, spatially lead the player to the
wetland, and stop or scatter honestly under close disturbance. Muted playback
may skip audio output but must retain authoritative cue and observation tests.

### Phase 3: brood, seasonal lifecycle, and population evidence

1. Add temporary courtship, compatible-site selection, and one bounded brood
   creation path.
2. Add spawn/tadpole/readiness progression, habitat pause/failure, planned
   child identities, idempotent froglet release, maturation, and remains.
3. Extend the shared wildlife diet, lifecycle, resource, conservation,
   suppression, and population reports with frogs and brood outcomes.
4. Re-run wet and dry campaign windows through accepted short and long
   durations, including exact full/accelerated equivalence and inactive freeze.
5. Extend the thousand-animal graceful-overload fixture with frog and brood
   demand.

**Review Gate D:** exact identities must satisfy frog
`start + released froglets - deaths = end` and brood
`created - failed - released = active end` accounting. Wet windows must show
real aquatic-invertebrate pressure and at least one successful lifecycle
without requiring a target count; the dry control remains exactly frog-free.

### Phase 4: product acceptance and deployment

1. Register typed live-instantiation evidence and add the data-only
   `frog-chorus` recipe.
2. Add focused native and browser behavioral observers over ordinary
   replicated state rather than showcase commands.
3. Prove memory/null, SQLite, and IndexedDB save/reopen behavior plus transient
   Web zero-persistence semantics.
4. Build flat Android and Android XR through the repository scripts.
5. Run native flat, headed-WebGPU desktop/phone, synthetic stereo, physical XR
   or exact multiview, and affected performance gates sequentially where GPU
   contention matters.
6. Update the living topics and this execution record, commit and push the
   tested revision, verify the exact deployment, inspect public captures, and
   share the clean temporary review URL.

**Review Gate E:** Human Review accepts frog scale and motion, chorus cadence,
water-edge behavior, brood readability, froglet release, field-note clarity,
phone controls, and stereo/XR presentation before the tactical is complete.

## Validation Matrix

At minimum, cover:

- canonical Frog/brood generation, semantic validation, checked JSON drift,
  animation sheets, surface analysis, catalogue status, and strict first-party
  inventory/provenance;
- protocol codec round trips and malformed kind/payload rejection for Frog and
  FrogBrood snapshots and incremental updates;
- deterministic initial population plans across order, partition, negative
  coordinates, supported topology lifts, and dry controls;
- block-derived habitat qualification, live revalidation, land/shore paths,
  shallow-water occupancy, player avoidance, and failed-route recovery;
- real-entity call origin, spatial range, sequence, cooldown, local response
  cap, disturbance interruption, and no unloaded/global chorus work;
- resource reachability, aquatic-invertebrate conservation, condition,
  reproduction suppression, mortality, remains, and mallard competition;
- brood creation, stage pause/resume, loaded-only development, habitat loss,
  planned child identity, retry/reopen idempotence, release, and maturation;
- field observation source, idempotence, persistence, replication, and shared
  flat/stereo HUD projection;
- thousand-animal decision/path/chorus/brood work bounds and fair admission;
- memory/null, SQLite, and IndexedDB persistence plus full/accelerated campaign
  equivalence;
- shared renderer single-view, per-eye, and full-frame multiview actor/prop
  paths with one view's data never reused for another; and
- native, Web/Wasm, flat Android, Android XR, and dedicated-server compile or
  runtime gates selected from the current platform matrix.

Every pixel-producing milestone requires a capture under `/tmp` and visual
inspection before proceeding. Final behavioral evidence must include domain
outcomes; a first-frame screenshot alone is insufficient.

## Explicit Deferrals

- authoritative rain, storm-triggered calls, puddle spawning, seasonal creek
  discharge, and drying/re-wetting water reconciliation;
- deep-water or volumetric navigation, diving, underwater predation, fish,
  fishing, and beaver water mutation;
- one independently ticking entity per tadpole, large egg masses, disease,
  parasites, genetics, and exact real-world clutch sizes or calendars;
- true hibernation, buried winter shelters, inactive elapsed-time catch-up,
  migration, recolonization, and anonymous seasonal wildlife replacement;
- fox, owl, heron, fish, player, or other frog predation and food-web balancing
  beyond measured mallard resource competition;
- catching, carrying, releasing, feeding, taming, farming, poison, potions,
  cooking, drops, or a frog-specific collectible;
- permanent mates, colonies, pond ownership, universal animal homes, or a
  public generic ecology/behavior API;
- a frog-only renderer, navigator, audio mixer, app-local gameplay branch,
  scripted showcase behavior, or changes to retained Java-shaped profiles.

## Completion Checklist

- [ ] Tactical 315 squirrel cache and acceptance gates are closed before frog
      implementation begins.
- [ ] The reviewed Frog and brood assets are promoted through the shared
      first-party semantic pipeline.
- [ ] Ordinary Mclone habitat produces durable frogs through coordinate-pure
      planning plus live block validation, while dry controls remain empty.
- [ ] Frogs visibly hop, forage, swim in supported shallows, escape threats,
      call, and participate in a bounded chorus through authoritative state.
- [ ] One material brood progresses while loaded and releases non-duplicated
      durable froglets that later mature.
- [ ] Frog/mallard resource pressure and short/long population evidence pass
      exact conservation, full/accelerated equivalence, inactive freeze, and
      graceful overload without a population target.
- [ ] All six field observations arise only from ordinary evidence and persist
      through SQLite and IndexedDB reopen.
- [ ] Native, headed WebGPU desktop/phone, stereo, Android, and XR/multiview
      evidence is captured, inspected, and recorded.
- [ ] The exact pushed and deployed `frog-chorus` revision passes its bounded
      behavior gate and leaves all Web persistent-world stores empty.

## Shared Ownership

- `mclone-server`: authoritative frog/brood state, habitat sampling, initial
  geography, behavior, chorus admission, resource/lifecycle simulation,
  field observations, persistence, live evidence, and showcase compiler.
- `mclone-protocol`: neutral entity kinds, life stages, behaviors, call cues,
  brood state, observation progress, and stable wire encoding.
- `mclone-client`: replicated actor/brood facts only; no ecology policy.
- `mclone-assets`: checked Frog/brood figures, first-party inventory, pack
  loading, and provenance.
- `mclone-render-session` and `mclone-render`: shared named animation,
  semantic-prop, scale, single-view, stereo, and multiview presentation.
- `mclone-audio`: shared spatial croak cue mapping and bounded playback.
- `mclone-ui`: field-note and optional debug/report presentation from
  replicated models.
- `mclone-worldgen`: shared wetland/cover blocks and semantic terrain facts
  only where ordinary frog habitat exposes a real terrain gap.
- platform apps: cadence, target, raw input, storage, transport, and audio
  device adapters only.

## Code And Documentation Map

- `tools/asset-lab/examples/frog/figure.ts`: accepted source figure and
  animation starting point.
- `assets/mclone/figures/`: generated promoted Frog and brood semantic assets.
- `native/crates/mclone-server/src/entity/`: authoritative individual and brood
  behavior/state.
- `native/crates/mclone-server/src/entity/spawning/`: habitat qualification,
  coordinate-pure population planning, and first realization.
- `native/crates/mclone-server/src/wildlife_resources.rs`: frog diet and exact
  aquatic-invertebrate transfers.
- `native/crates/mclone-server/src/wildlife_simulation.rs`: lifecycle,
  conservation, population, and overload evidence.
- `native/crates/mclone-server/src/playable_showcase.rs`: typed live evidence
  and bounded review recipe compilation.
- `native/crates/mclone-protocol/src/`: shared replication and stable codec.
- `native/crates/mclone-assets/`, `mclone-client`, `mclone-render-session`,
  `mclone-render`, `mclone-audio`, and `mclone-ui`: shared consumer paths.
- [`../topics/habitat-driven-creature-ecology.md`](../topics/habitat-driven-creature-ecology.md):
  terrain, resources, admission, population, and species sequence.
- [`../topics/wildlife-ecology-state-model.md`](../topics/wildlife-ecology-state-model.md):
  durable individuals, bounded knowledge/social patterns, work, and inactive
  semantics.
- [`../topics/seasons.md`](../topics/seasons.md): authoritative seasonal
  opportunity and no-catch-up constraints.
- [`../topics/playable-showcases.md`](../topics/playable-showcases.md): bounded
  data-only review saves and live-instantiation evidence.

## Execution Record

No implementation has started. Phase 0 records the selected proposal and its
place after Tactical 315.
