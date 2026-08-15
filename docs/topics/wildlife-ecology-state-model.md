# Wildlife Ecology State Model

Topic: `wildlife-ecology-state-model`

Status: **active architecture with its first concrete foundation landed under
Tactical
[`295`](../tactical/295-rabbit-refuge-memory-and-ecology-agent-foundation.md)
and urgent safety proof completed under Tactical
[`296`](../tactical/296-rabbit-player-avoidance.md) on 2026-08-14. Rabbits now
consume server-internal bounded place knowledge, explicit availability
outcomes, stable decision schedules, deterministic work admission, spatially
bounded refuge/neighbor queries, and fair budgeted open-ground escape. Deer
and fox remain the intended contrasting consumers before this becomes a
public generalized ecology API. Tactical
[`298`](../tactical/298-deterministic-initial-wildlife-population.md) has now
landed a separate seed-authored initial-population geography, shared with
Terrain Lab;
it does not tick unloaded animals or introduce a coarse offscreen population
simulation. Social memory, multi-record crash-atomic persistence, and any
coarse population layer remain deliberate later work.**

Tactical
[`299`](../tactical/299-closed-domain-wildlife-population-simulation.md) has
landed the first multi-day population foundation without weakening that model.
It keeps one real-seed entity-ticking domain immutable, treats its exterior as
currently unavailable, permits no immigration or post-initial spawning, and
requires exact `start + births - deaths = end` identity conservation. Its
forage, energy, reproduction, mortality, and remains state advances only while
loaded; it is not an unloaded population summary. Radius-8 runs prove
multiple generations and exact full/accelerated equivalence.

Tactical
[`301`](../tactical/301-typed-resource-strata-and-mallard-ecology.md) has now
landed the next concrete extraction: five fixed shared terrain-resource
strata, species-owned rabbit/deer/mallard diet profiles, and mallard adoption
of the loaded lifecycle and natural nest/hatch path. The gameplay model still
contains no population estimator or carrying-capacity target; raw resource,
condition, birth, and death histories are the calibration evidence.

The first corrected multi-seed 30-day matrix proves that extraction across
four ordinary radius-8 Overworld windows. Every exact invariant and
full/accelerated canary passes. Wet windows consume different mixtures of
shore seeds and aquatic invertebrates; the dry control retains zero aquatic
capacity and zero mallards. Successful attended hatching keeps a bounded
parent-owned memory of the still-valid shore site, enabling a later clutch
without turning nests into permanent homes or loading remote terrain.

## Scope

This topic owns the durable state and active-behavior model by which varied
wild animals can remember places, coordinate socially, use optional habitat
features, and remain correct when related entities or terrain are unloaded. It
also owns the near-term species list as a set of deliberate pressure tests for
that model.

It does not own habitat generation, creature art, individual tactical
execution, or a complete offscreen ecosystem. Those remain in
[`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md),
[`animal-catalogue.md`](animal-catalogue.md), and bounded tactical documents.
Rabbit-specific implemented facts remain in
[`rabbit-burrow-ecology.md`](rabbit-burrow-ecology.md).

## Direction

Use one execution and persistence model for materialized animals, then compose
a few bounded ecology patterns on top. Do not make every species a bespoke
simulation, but also do not generalize the rabbit warren until every animal
appears to own a permanent home.

The common animal model is:

```text
persisted animal identity and physical state
  + needs, relationships, and timestamped knowledge
  -> observe the currently available world
  -> prioritize one need
  -> select a species policy and candidate
  -> attempt an interruptible action
  -> learn from the outcome without inventing absent world facts
```

Species should primarily differ in which observations matter, how quickly
memories lose confidence, how needs are prioritized, how candidates are
scored, which actions are possible, and which durable or social relationships
they support. Persistence, availability, action, and evidence semantics should
remain common.

## Individual Knowledge

Each durable animal may remember relevant places, creatures, and events.
Knowledge facts should carry enough context to become stale or be contradicted
without silently becoming authoritative world truth:

- position or durable subject ID;
- observation time and confidence;
- individual, social, or direct-world source; and
- any revision needed to recognize a replaced durable feature.

Typical facts include forage, water, danger, prey, refuges, bedding sites,
travel corridors, and the last known position of a social group.

A remembered place is not necessarily a separate saved object. One deer's
preferred bed normally belongs in that deer's memory. A warren, den, hive,
nest, or shared watering feature merits its own record only when it remains a
meaningful gameplay fact independently of one animal.

## Unloaded Is Not Absent

Queries and actions must distinguish at least:

| Outcome | Meaning | Permitted reaction |
|---|---|---|
| available | active evidence proves the target and a usable local route | act on it |
| currently unavailable | the target or route crosses unloaded or inactive state | defer, remember, or choose a local alternative |
| confirmed unsuitable or unreachable | complete active evidence rejects this candidate | reduce its preference and choose another |
| confirmed absent, replaced, or destroyed | authoritative state invalidates the fact | forget or reconcile the relationship |

An animal may say, in effect, "there is no current path to that destination"
when the navigation envelope ends. It must not infer that the destination was
destroyed, find a replacement merely to repair a load race, or cause chunks to
load just to settle the question. Loaded object references are caches, not
authority.

Actions therefore remain interruptible. Navigating, entering a refuge,
feeding, digging, resting, hunting, or following may defer or fail without
corrupting persistent relationships. A later active tick can reconsider the
need using the evidence then available.

## Bounded Social Memory

A herd, flock, family, pair, or colony may own a compact persisted shared
memory or behavioral blackboard. This is an intentional "telepathic"
simulation abstraction, not literal communication or decentralized authority.
Loaded nearby members contribute timestamped, positioned observations such as
alarm, forage, refuge, or intended travel direction. Unloaded animals make no
new observations, and consumers still evaluate age, distance, confidence, and
their own circumstances.

The group record can be loaded on demand independently of terrain chunks, but
loading it only exposes persisted facts; it does not grant a simulation ticket
or advance dormant behavior. Stable IDs, revisions, serialized idempotent
updates, and one authoritative owner per fact keep group and animal records
safe across arbitrary save/load ordering.

## Bounded Spatial Patterns

The shared model must support these patterns without forcing them into one
`home_id` field:

| Pattern | Examples | Durable shape |
|---|---|---|
| fixed communal refuge | rabbit warren, bee colony | habitat feature with capacity or claims plus animal knowledge |
| fixed individual or family refuge | fox den, owl cavity | habitat feature important during relevant life stages |
| seasonal construction | bird nest | feature that may later be abandoned or decay |
| repeated replaceable sites | deer beds, roosting branches | scored individual or group memories |
| familiar range or territory | deer, fox, mountain lion | approximate spatial familiarity and resource/threat knowledge |
| nomadic movement | migrating or dispersing animals | no required home; needs and social intent drive travel |

Population geography is separate again. A later coarse population-region
record may describe establishment, density, dispersal, or recovery without
pretending to be a particular animal, home, or moment-to-moment AI actor.

## Active-Tick and On-Demand Semantics

Ordinary materialized animals simulate only while their entity ticket is
active. Inactive animals freeze rather than receive approximate offscreen
movement, predation, breeding, or catch-up. An active prey population may
therefore grow while a predator outside the active area remains inert. That
limitation is simple, predictable, and acceptable until observed gameplay
justifies a more elaborate population layer.

Habitat and social records may be resolved on demand without loading their
terrain chunks, but they answer semantic questions only: whether the record
exists, its revision, remembered membership or claims, capacity, and known
condition. Physical entry, collision, pathing, and current block suitability
still require active world evidence. Record availability must never be
confused with simulation activity.

The pure behavior test surface should run the
observe/update/select/attempt kernel without rendering, chunk storage, or
rigid-body physics. It can supply flat synthetic observations and inject
unavailable, delayed, duplicated, or contradictory resolutions. The live
runtime remains Minecraft-shaped and freezes inactive state even if the test
harness can advance many deterministic steps quickly.

### Initial population is not offscreen simulation

The landed revision-1 Mclone population planner is a seed-authored initial
condition, not a coarse ecology record. It plans each 64-by-64-block cell
independently from terrain semantics and assigns at most one encounter to one
owner chunk. First entity-chunk realization turns that encounter into ordinary
durable animals. After that moment, individual records, active-tick behavior,
birth, death, movement, and player action are authoritative.

The entity-record load result is also the realization marker. Missing means
the owner chunk may attempt its seed plan; present empty means the ecology was
already realized and is now empty. This keeps initial geography compatible
with the durable-animal model without pretending that a planner cell is a
living population or using it to refill losses. A later population summary,
if justified, needs its own identity, producer, cadence, and reconciliation
contract.

## Resource Competition and Diagnostic Expectations

The common lifecycle can share resource accounting without sharing species
policy. Terrain resources are persistent active-world facts indexed by bounded
spatial cells; an animal's diet describes which facts it can consume, while
its species behavior still owns sensing, destination choice, route, feeding
action, safety, mating, and birth.

Resource identity belongs to the habitat, not the consumer. Low herbaceous
growth consumed by a rabbit is no longer independently available to a deer.
Woody browse, shore seeds, aquatic vegetation, and aquatic invertebrates remain
distinct, so coexistence does not imply total competition. A feeding event
requires both ledger availability and complete local world evidence: the
animal must reach compatible geometry inside its active navigation envelope.
A cell record cannot authorize remote feeding or turn unloaded terrain into a
known meal.

This is a small fixed production vocabulary extracted from rabbit, deer, and
mallard needs, not a generalized nutrient graph. New strata require a concrete
consumer and ordinary terrain evidence. Species-specific crops, carrion,
inventories, nests, and prey identities retain their own authoritative state
instead of being flattened into generic resource units.

Population support remains emergent. For species `s` and resource `r`, a
report can describe the rough constraint that combined observed demand cannot
remain above loaded recovery indefinitely:

```text
sum over species(observed population_s * maintenance demand_s,r)
  approximately fits within observed loaded recovery_r
```

That expression is a calibration lens, not a simulation equation. Substitution
between foods, access failure, safety, age structure, reproduction, mortality,
and stochastic-looking individual histories make the feasible population a
region rather than one carrying-capacity constant. Any estimated support range
must serialize its assumptions, stay outside gameplay code, and be removable
without changing a result. Raw intake, recovery, condition, birth, suppression,
and death histories are the primary evidence.

Never use a calculated capacity or desired species ratio to spawn, suppress a
birth, kill, despawn, refill, steer migration, or declare an otherwise valid
run successful. Technical overload guards remain host safety and are failures
when reached by an ordinary ecology campaign.

## Cadence, Work Budgets, and Checkpoint Durability

Logical persistence, active simulation, AI reconsideration, and crash
durability are separate concerns. An animal becomes logically persistent as
soon as the authoritative host materializes it: it joins the saved lifecycle
and may not be seed-rerolled on revisit. This does not require synchronously
writing every position, memory, or intent change to storage.

### Current Baseline

Mclone currently gives an entity in an `ENTITY_TICKING` chunk one ordinary
entity update on every gameplay tick. `TICKING` and `BORDER` chunks may retain
tracked/queryable entities without ticking them. The shared default cadence is
60 Hz host, 20 Hz gameplay, and 60 Hz physics, but ordinary creature movement,
collision, goals, and species behavior still belong to the 20 Hz gameplay
entity update; the separate physics lane does not make animals run at 60 Hz.

The native runner currently autosaves every 6,000 gameplay ticks, or five
minutes at the default rate, in addition to dirty save on unload, explicit
flush, browser page-background handling, and clean close. Entity movement
marks its logical entity chunk dirty, but storage writes are already coalesced
rather than issued every tick. This is compatible with accepting bounded
crash rollback while a region remains loaded.

Minecraft Java 1.17.1 runs goal cleanup, new-goal eligibility, and running-goal
updates from `GoalSelector` on each mob tick. Expensive individual goals often
self-throttle: default random stroll has a 1-in-120 eligibility roll, while a
default nearest-attackable-target search has a 1-in-10 roll. Mclone should
preserve recognizable outcomes where parity matters without treating one
global full-rate AI pass as the only possible runtime schedule.

### Explicit Work Cadences

Do not introduce one vague "partial animal tick." Split work by semantic
obligation and let an active entity receive only the work currently due:

| Work class | Target cadence |
|---|---|
| damage, interaction, block-change, and alarm reactions | event-driven and immediate |
| movement, collision, and waypoint following | gameplay rate while moving |
| running-goal continuation | frequent enough to preserve responsive behavior |
| broad sensing and new goal selection | staggered and budgeted, initially around 2-5 Hz where suitable |
| hunger, fatigue, memory confidence, and other slow needs | deadlines or roughly 1 Hz |
| habitat/population reconsideration | bounded intervals of seconds or longer |
| network publication and rendered animation | independent of AI decision cadence |
| ambient entity checkpoint | coalesced over minutes, unload, flush, or close |

Exact rates remain species- and measurement-driven. Damage, player
interaction, a newly opened gate, a changed path block, or a fresh group alarm
must wake relevant work without waiting for a slow polling interval. Due work
should be staggered by persistent identity so a large population does not
reconsider on the same tick.

An active outer band may reduce expensive sensing and reconsideration, but it
must not move materialized individuals through a non-physics approximation.
Real movement still requires loaded block evidence, collision, and navigation.
A resident dormant band performs no individual movement or ecology. If coarse
offscreen migration or population change is later desired, it belongs to the
separate population-summary model rather than a hidden low-quality animal.

### Graceful Overload

When active work exceeds capacity, keep the authoritative world cadence and
allow nonurgent decision latency to rise. Do not finish an unbounded AI pass at
the cost of a long server tick, and do not accumulate missed idle decisions as
catch-up debt. An animal that missed ten forage reconsiderations should make
one decision from current evidence when admitted.

Budget deterministic work units first—decision checks, sensor queries, neighbor
candidates, and path-node expansions—with a wall-time emergency ceiling as a
host-safety backstop. A rotating cursor plus queue aging must prevent stable
entity ordering from starving later identities. Request generations must make
deferred path and sensing results harmless after an animal changes intent or
the relevant world evidence changes.

Admission priority is:

1. player interaction, damage, urgent threat, and safety reactions;
2. movement and collision for already moving entities;
3. combat, flight, refuge entry, and other running consequential actions;
4. player-observable new decisions;
5. new paths, broad perception, and social coordination; and
6. idle wandering and slow memory maintenance.

Urgent safety does not imply a fixed-home requirement. The first rabbit
correction under Tactical
[`296`](../tactical/296-rabbit-player-avoidance.md) treats an approaching
player as an immediate wake, then commits to a bounded complete path whose
endpoint is farther from the threat. Refuge memory remains an independent
later shelter input. This is the intended reusable distinction: danger can
change locomotion now without assuming a loaded home, a reachable habitat
record, or a species that owns either one.

A thousand densely housed chickens also stress contact, neighbor queries,
replication, and rendering, not only AI. Budgets limit spikes but do not excuse
quadratic algorithms. Use spatial buckets for neighbors and contact pairs,
bounded group blackboards for shared observations, cached habitat samples with
relevant invalidation, path reuse, event/deadline wakeups for stationary
animals, changed-only replication, and instanced rendering. Natural mob caps
cannot be the only defense because player breeding can exceed them.

### Bounded Crash Rollback

Recency is less important than consistency. It is acceptable for a crash to
restore an older animal position, intent, bedding preference, memory
confidence, fatigue value, or ambient cooldown. It is not acceptable for
independently aged records to duplicate or contradict a completed gameplay
transfer.

Classify persistent mutations:

| Persistence class | Examples | Target policy |
|---|---|---|
| ambient | pose, current intent, memory, ordinary need progress | coalesced periodic checkpoint |
| lifecycle | materialization, birth, death, hatch, refuge creation or collapse | prompt atomic batch or journaled operation |
| transfer | pickup, inventory consumption, harvest/drop, block/entity exchange | atomic across every affected record |

The invariant is that related facts recover either before or after one
operation, never from mixed sides of it. A deer must not return while its drops
remain in player inventory; an item must not return after pickup; a rabbit
mouth must not survive while its excavated block rolls back; and a duckling
must not coexist with an unhatched copy of its nest state.

The existing record-oriented persistence actor already provides revisions,
same-key coalescing, durable lanes, unload barriers, flush, SQLite WAL, and
IndexedDB transactions. A future ecology persistence slice should add the
narrow multi-record batch/checkpoint or idempotent operation boundary needed
for lifecycle and transfer facts rather than synchronously saving every entity
tick. The ambient checkpoint interval should remain measurement-driven.

### Quality and Diagnostics

Render entity distance is a client graphics choice. Simulation distance and
ecology activation are authoritative gameplay/server policy. AI admission
budget is primarily an automatic host-performance policy. Do not expose a
generic Low/Medium/High ecology slider that quietly makes predators slower,
gardens safer, or breeding less effective.

Any future simulation profile should preserve immediate near-player behavior
and vary explicit server-owned quantities such as active-band width, admitted
nonurgent work, path-node budget, and semantic-record cache size. Multiplayer
uses the highest fidelity required by any nearby player. Hardware-dependent
wall-clock timing must not decide deterministic ordering or ecological truth.

Required diagnostics include animals by spatial/activity state, moving and
sleeping counts, decisions and sensor queries admitted/deferred, maximum and
aged work debt, path requests and node expansions, neighbor candidates, cost
by species/work class, record-cache activity, dirty records, checkpoint age,
and lifecycle/transfer batch latency. A bounded thousand-animal stress fixture
should prove stable tick time, fair progress, no stale-result application, and
no persistence duplication before quality controls become a product setting.

## Rabbit Refuge Evolution

The warren remains the first durable-refuge pattern, not the owner of a
universal animal lifecycle. Tactical 295 preserves its one-cell excavation,
readable mouth, compact hidden interior, capacity, condition, and reuse while
landing the following response:

1. Try a suitable reachable remembered mouth.
2. Learn of or locally discover another mouth with capacity.
3. Excavate one qualifying dirt or grass cell when no adequate local refuge is
   available.
4. Use temporary cover or defer when digging is unsafe or unsuitable.

An unloaded familiar warren stays remembered rather than becoming destroyed.
The rabbit may use or establish a local alternative without forcing the old
chunk to load. Repeated use can change affinity and make dispersal legible.
Suitability, local density, cooldown, reuse, and collapse prevent uncontrolled
burrow spam. Energy cost and eventual abandonment remain later tuning.

### Landed rabbit foundation

The first implementation is intentionally internal to `mclone-server` and
contains only machinery rabbits use today:

- `WorldFactLocator` and up to three `KnownPlace` records preserve stable
  refuge identity, last-known block position, optional revision, observation
  time, and familiarity;
- `Availability` keeps available, currently unavailable, confirmed unsuitable,
  and confirmed gone outcomes distinct;
- `DecisionSchedule` makes stable-ID-staggered work due and rejects stale
  attempt generations, while `EcologyWorkBudget` admits bounded decision,
  habitat-query, and path-request units without replaying missed work;
- the entity codec is version 11. Legacy homes migrate to unresolved memories,
  old permanent warren residents cease to be authority, and current occupancy
  is derived from sheltered or in-flight claims;
- active unhomed rabbits forage normally, seek shelter only for rest or safety,
  reuse remembered or nearby active capacity first, and excavate one qualified
  cell only when no adequate active refuge exists;
- 16-block refuge cells and bounded rabbit-neighbor buckets replace hot-path
  broad candidate work. A 1,000-rabbit fixture proves hard admission limits,
  nonquadratic neighbor candidates, and fair eventual decisions;
- urgent player avoidance bypasses idle decision cadence, consumes the same
  fixed path budget ahead of habitat work, commits only to complete paths away
  from danger, and rotates admission so permanently unroutable identities do
  not starve the rest of a dense population; and
- revision 4's exact public desktop and phone gates prove one founder can
  retain an unsuitable familiar mouth, discover and reuse another mouth, hide
  and return without excavation, then survive exact collapse reconciliation
  while a transient showcase writes no persistent world records.

This is a common persistence/execution vocabulary, not a generic behavior
tree, public mod API, or universal home trait. The next extraction decision
must be based on deer replaceable bedding or fox prey/den behavior.

## Species As Model Pressure Tests

The candidate list is a behavior and habitat test matrix, not a promise to
promote every available Creature Lab figure. Prefer additions that prove one
new bounded pattern while participating in an ecology that already has enough
food, prey, shelter, or disturbance to make the behavior legible.

| Candidate | Model pressure | Terrain and interaction pressure |
|---|---|---|
| fish | bounded water-volume availability, schooling, spawning grounds without terrestrial paths | depth, flow, cover, aquatic food and angling |
| beaver | family memory, lodge/refuge use, persistent construction intent | woody food, banks, water connectivity and bounded dam terrain mutation |
| squirrel | distributed caches, cavities and arboreal escape knowledge | mature trees, connected canopy, nuts and forgotten caches |
| fox | prey memory, stalking, temporary den use, dispersal | rabbit pressure, cover, field edges |
| wild pig | loose sounder knowledge, rooting, crop raids | rootable soil, woodland mast, marsh and garden pressure |
| crow | shared observations, scavenging, mobbing, mixed flight/ground use | perches, carrion, fields and forest edges |
| coyote | solitary/pair/group transitions and coordinated predation | scrub, grassland edges, rabbits and deer young |
| bear | seasonal priorities, conditional denning, cubs and omnivory | caves, logs, berries, streams and bee colonies |
| owl | nocturnal hunting and replaceable roost fidelity | old forest, cavities and small prey |
| mountain lion | very large familiar range, concealment and ambush | cliffs, rocky shelves, cover and deer corridors |
| eagle | fixed seasonal nest plus landscape-scale aerial hunting | cliffs, tall snags, lakes and open terrain |
| bobcat | solitary cover-dependent territory and prey caching | brush, rocks, wetlands and fallen timber |
| horse | herd cohesion, grazing travel, water memory and panic cascades | broad grassland, trails and watering places |

The current bounded sequence has migrated mallards and now uses the resulting
resource histories to choose among fish, beaver, or squirrel as the next
non-predator pressure test. This is not a commitment to implement all three.
After bottom-up support and prey surplus are legible, fox remains the first
predator candidate, followed by whichever of wild pig, crow, coyote, bear,
owl, mountain lion, or eagle adds the most useful missing relationship.
Bobcat overlaps fox and mountain lion unless cover, wetland, snow, climbing,
or cache mechanics distinguish it. Wild or feral horses are mechanically
valuable but also invite taming, riding, breeding, and an implied human
history, so their product role should be decided before promotion.

Keep the food web from becoming predator-heavy. Rabbit, deer, pig, squirrel,
crops, mast, berries, insects, and carrion should make predators and
scavengers consequential rather than decorative.

## Adoption Sequence and Guardrails

1. Retain the landed rabbit-owned knowledge/availability foundation and shared
   loaded lifecycle as server-internal concrete machinery.
2. Retain the landed typed habitat resources and species-owned diet profiles,
   preserving physically reached feeding and inactive-world semantics.
3. Retain mallards as the landed contrasting amphibious lifecycle consumer,
   using the durable semantic nest and attended hatch rather than a universal
   home or abstract population birth.
4. Choose one of fish, beaver, or squirrel only after the three-species
   resource evidence identifies the most valuable missing spatial pattern.
5. Add fox pressure later through prey observations, interruptible pursuit,
   and life-stage-relevant den use rather than a universal permanent-home
   pointer.
6. Extract a shared public ecology owner only from concrete cross-crate need.
   Do not begin with a general actor platform or an unconstrained fact
   database.

Every transition from memory to world mutation must have one authoritative
owner. Missing loaded state cannot prove destruction. Updates spanning an
animal, group, or habitat feature must be serialized, revision-checked, and
idempotent where retries are possible. On-demand activation of a record does
not activate ecology simulation or force its underlying chunk.
