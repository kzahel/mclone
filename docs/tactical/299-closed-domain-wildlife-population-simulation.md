# Tactical 299: Closed-Domain Wildlife Population Simulation

Status: planned 2026-08-15

Topic: `habitat-driven-creature-ecology`

Topic: `wildlife-ecology-state-model`

## Instruction Synthesis

Turn the existing rabbit and deer gameplay chapters into measurable loaded-
world populations. Run ordinary authoritative ecology over real Mclone
Overworld seeds for many Minecraft days, record births, deaths, resources,
life stages, and population totals at a useful cadence, and expose deterministic
reports that can tune population growth and ceilings. Include natural mortality
and durable remains as the future food source for scavengers.

The first experiment is a closed loaded domain. After initial realization, one
synthetic central ticket owns an immutable set of entity-ticking chunks for the
entire run. There is no immigration, emigration, moving observer, post-initial
spawn, unloaded catch-up, or population-summary stand-in. An unavailable or
non-ticking chunk is not a lower-quality place to simulate an animal; it has no
current path.

## Current Evidence and Missing Proof

The existing chapters prove many individual mechanics but not population
ecology:

- rabbits can be tempted and player-fed with carrots, enter courtship, create
  one persistent kit with parent identities, mature, raid crops, use and create
  refuges, disperse, and consume bounded/fair AI work;
- rabbit love lasts 600 ticks, current kit maturation takes 2,400 ticks, and
  the parental breeding cooldown is 6,000 ticks, but grass forage does not
  supply reproductive energy and fleeing has no metabolic cost;
- warren capacity is current shelter occupancy, not landscape carrying
  capacity;
- deer persist sex, fawn/adult presentation, health, antlers, herd behavior,
  grazing, drinking, bedding, flight, hunting, and death, but have no age,
  maturation, mating, pregnancy, birth, or natural mortality;
- Tactical 298 supplies deterministic initial rabbit/deer/mallard/bee
  geography and explicitly supplies no refill or offscreen population
  simulation;
- the rabbit showcase observes at most 720 ticks, the deer showcase observes
  80 ticks, and the 1,000-rabbit fixture proves work admission rather than
  births, deaths, food pressure, or equilibrium.

A long run of the current rules would therefore be mostly flat: deer cannot
reproduce, and rabbits reproduce only after repeated player interaction. No
existing receipt demonstrates multi-day population growth, resource pressure,
stable population bands, or a meaningful ceiling.

The current entity tick list also demotes an entity based on its resulting
chunk position; it does not itself make the edge of the entity-ticking set a
physical wall. Loaded but non-ticking terrain can remain queryable. A mob that
crosses the boundary can therefore become stranded and inactive rather than
meaningfully emigrate. The closed-domain harness must expose and reject this
case instead of counting it as ecology.

## Binding Decisions

### One immutable loaded population domain

Each run declares a world profile, signed seed, central block/chunk, fixed
entity-ticking radius, terrain guard radius, duration, lifecycle-rule revision,
and report cadence. Setup proceeds in a fixed order:

1. generate the ordinary Mclone Overworld chunks required by the declared
   domain and guard;
2. install one synthetic central ticket and wait until the exact entity-ticking
   set is stable;
3. first-realize every entity chunk in that set through the production initial-
   wildlife path, including saved empty records;
4. record the immutable entity-ticking set and initial persistent identities;
5. disable every source of post-initial generic spawning for the run; and
6. advance the ordinary authoritative simulation without moving the ticket or
   changing the chunk set.

The terrain guard may remain loaded for stable boundary and surface queries,
but it is unavailable to animal navigation and ecology. Shared active-world
availability must reject path nodes, destinations, forage, water, refuges, and
social actions outside the immutable entity-ticking set. This is production
availability behavior exercised by the harness, not a harness-only wall or
alternate movement system.

No animal may cross from the ticking set into the guard. The runner records any
such result as a `boundaryViolation`, retains the durable identity for
diagnosis, and fails the run. It must not relabel a frozen entity as dead or
emigrated.

With no player or scripted mutation, every daily receipt must satisfy both
identity and count conservation:

```text
living_end = living_start + births - deaths
```

Maturation, hiding, bedding, movement, group change, and persistence hydration
do not change this total. Post-initial spawns, unexplained removals, duplicate
identities, missing birth parents, and boundary violations are hard failures.

Dynamic tickets, migration experiments, recovery from an exterior population,
and moving-player observations remain later work. They must not complicate the
first closed baseline.

### The long run uses authoritative gameplay

`mclone-server` owns a bounded `WildlifeSimulationSession` diagnostic facade
over the same world generation, entity records, physics, pathfinding, block
queries, lifecycle transitions, work budgets, and persistence codecs used by a
normal realm. A thin headless CLI owns argument parsing and report files. It
does not own ecology formulas.

The runner removes presentation cost only: no renderer, audio mixer, network
transport, client prediction, UI, or real-time sleep. It initially advances
the full server simulation loop as fast as the host permits. Benchmark a
representative ten-day run before extracting any faster path.

If full-tick execution cannot support the required matrix, extract only the
slow lifecycle/resource reducer into a shared production owner called by both
the live server and accelerated runner. Do not copy equations, invent daily
simulation-only transitions, bypass movement/resource reachability, or accept
an accelerated result without a shorter full-server equivalence canary.

Wall time and thread scheduling may not affect a receipt. Stable seed,
coordinates, rules, initial records, and commands must produce byte-stable
daily facts and a stable final checksum regardless of report formatting or
query order.

### Terrain-derived renewable forage

Add one bounded persisted forage-resource cell record on the same
64-by-64-block coordinate lattice as the initial wildlife population. The
initial potential is derived from ordinary generated/live facts such as grass,
flowers, low browse, woody cover, water access, and real crop blocks. The
record distinguishes:

- potential forage under the current terrain;
- currently available forage;
- loaded recovery accumulated at the production cadence;
- consumption by species and life stage; and
- the terrain/block revision used by its last bounded resample.

The cell is an accounting bound, not a remote feeding target. A rabbit or deer
must still reach and perform an ordinary forage/graze action at a valid local
site before consuming the cell's availability. Real carrots and other crops
remain block state and use their existing mutation/harvest paths. An animal
cannot consume a meadow on the opposite side of an impassable barrier merely
because both positions share a resource cell.

Loaded resource recovery is constant-condition baseline ecology. No resource
recovers while its owning domain is inactive, and the runner does not replay
missed days. Player clearing, planting, flooding, or obstruction invalidates
only bounded relevant samples. This tactical does not make every grass bite
destroy a block or introduce seasons.

### Shared needs with species-owned policy

Rabbit and deer provide the first two concrete consumers of a small common
loaded-lifecycle record:

- age and a stable identity-derived lifespan threshold;
- bounded energy/body condition;
- recent successful intake and sustained deficit;
- reproductive readiness/cooldown and parent identities; and
- death cause when life ends.

This is not a general behavior tree, genetic simulation, or public mod API.
Species continue to own mate choice, group/refuge requirements, food cost,
movement, reproduction interval, litter/offspring size, maturation, and
behavioral priorities.

For rabbits:

- successful wild forage gradually supplies the same underlying reproductive
  condition that a player-provided carrot accelerates;
- urgent flight and sustained travel consume more energy than resting or local
  foraging;
- two healthy mature compatible adults may court without player feeding when
  food, safety, and local pressure permit;
- one persistent birth creates bounded kits with durable parentage;
- shelter capacity remains an entry claim and cannot masquerade as a
  population cap; and
- food scarcity suppresses courtship before it causes dispersal pressure or
  prolonged-deficit mortality.

For deer:

- existing sex and fawn/adult state gains persistent age and maturation;
- actual reachable browse supplies energy;
- mature compatible herd members may produce a bounded fawn on a much slower
  cadence than rabbits;
- low condition suppresses breeding before sustained deficit becomes lethal;
  and
- herd cohesion, alarm, drinking, bedding, hunting, and drops retain their
  existing authoritative owners.

Starting timing values are explicit hypotheses, not acceptance constants. Use
one versioned `WildlifeLifecycleTuning` record for production defaults and
runner sweeps. Reports must serialize every effective value. The initial sweep
should explore approximately one-to-two-day rabbit maturation, food-dependent
half-to-two-day rabbit intervals, substantially slower deer maturation and
birth intervals, and lifespans long enough that natural death is uncommon in
ordinary short visits but visible across long loaded runs.

### Soft ecological ceiling and hard overload guard

Normal population stabilization must emerge from:

- renewable forage potential and recovery;
- individual intake and energy costs;
- reproductive thresholds and cooldowns;
- local crowding and disturbance;
- offspring maturation and survival;
- dispersal attempts within the available domain; and
- prolonged-deficit and age mortality.

Do not encode the desired result as `if rabbits > N, stop breeding` or delete
animals above a target. A bounded local density term may reduce mate/refuge
choice or encourage movement, but the report must expose every suppressed
birth reason.

The existing entity/AI budgets remain safety infrastructure. Add a separately
named hard overload ceiling only to protect the server against player-created
or pathological populations. Hitting it is a failed baseline run, not a valid
equilibrium.

### Mortality leaves future scavenger food

The first ecological causes are:

- stable identity-derived old age;
- starvation only after a persisted sustained deficit; and
- existing player damage/hunting.

Low energy first changes feeding priority, suppresses reproduction, and may
encourage local dispersal. It must not randomly kill an animal after one poor
day. No disease, seasonal cold, or predator cause is invented yet.

Natural ecological death creates one bounded persistent `WildlifeRemains`
record with source species/identity, position, biomass, cause, creation tick,
and loaded decay state. Use a restrained non-graphic semantic world prop
authored through the shared source-first asset pipeline. A source render is a
human review point before runtime promotion.

Existing direct player hunting may retain immediate drops in this tactical;
natural deaths use remains. A later scavenger consumes biomass from the same
record, and a later unified harvest interaction may convert remains into
ordinary drops. Remains decay only while loaded, are capped/indexed per
resource cell, and may merge into bounded biomass rather than create unbounded
entities.

The runner must conserve remains biomass independently:

```text
remaining_biomass_end
  = remaining_biomass_start
  + biomass_from_deaths
  - decayed_biomass
  - harvested_biomass
  - scavenged_biomass
```

Scavenged biomass is zero in this tactical. The field exists so a future
scavenger cannot require a second incompatible death model.

### Receipts, reports, and tuning workflow

Every run writes a machine-readable manifest and daily rows to a caller-owned
output directory, normally under `/tmp`. The manifest includes:

- exact git/build revision and report schema;
- world profile, seed, center, topology, ticking and guard chunk sets;
- initial-population and lifecycle/resource revisions;
- duration, sample cadence, commands, and every tuning value;
- initial persistent-identity checksum; and
- final state and daily-series checksum.

At every Minecraft day boundary, and optionally at a finer debug cadence,
record:

- living adults and young by species and sex;
- births with parent identities;
- deaths by cause and life stage;
- average and percentile age, energy, and sustained deficit;
- available/potential forage, recovery, and consumption by species;
- attempted, successful, and suppressed reproduction by reason;
- refuge, herd, and active/inactive counts;
- remains created, remaining, and decayed biomass;
- work admitted/deferred, path requests, neighbor candidates, and wall/CPU
  timing; and
- post-initial spawn, unexplained removal, duplicate identity, and boundary-
  violation counters.

Write compact JSON for exact receipts and CSV for analysis. Generate a
self-contained review report with aligned population, births/deaths, forage,
energy, and remains curves plus the run manifest and invariant failures. This
is a diagnostic report, not a scripted showcase or a second ecology runtime.
Terrain Lab remains the owner of static initial-population geography; the
multi-day report links its seed/center back to `panes=wildlife` rather than
turning Terrain Lab into a server simulator.

The CLI supports a declared parameter matrix and parallel independent runs,
but one run remains internally deterministic and bounded. Never aggregate away
individual failed invariants. The report must make seed/window variance visible
instead of presenting only a mean curve.

### Baseline matrix and acceptance bands

Use deterministic site selection rather than showcase cherry-picking. The
first accepted matrix spans multiple signed seeds and windows with meadow,
woodland/edge, wet, dry, and mixed habitat evidence. Establish the exact
ticking radius only after the ten-day benchmark, but require enough initial
rabbit and deer groups across the matrix to exercise both growth models.

The first tuning campaign should cover at least 120 loaded Minecraft days per
accepted window and retain a smaller set of 200-day stress runs. Acceptance is
not one magic final count. Across the declared matrix:

- every run is deterministic and satisfies living-count and biomass
  conservation exactly;
- post-initial spawns, duplicate identities, unexplained removals, and boundary
  violations remain zero;
- forage never underflows or exceeds its declared potential;
- ordinary populations remain below the hard overload guard;
- rabbits can grow and recover more quickly than deer without exponential
  runaway;
- deer mature and reproduce slowly enough to remain distinguishable from
  rabbits;
- productive sites generally approach bounded oscillating bands rather than
  universal extinction or monotonic explosion;
- intentionally poor habitat can remain sparse or lose population for an
  explainable resource reason; and
- per-tick work stays bounded as population changes.

Pin a compact set of exact daily/final fingerprints for regression and retain
the full campaign receipts as generated evidence outside the repository. A
short full-server equivalence canary must match the corresponding long-run
owner for births, deaths, resources, and final identities before accepting any
accelerated implementation.

## Persistence and Failure Semantics

Age, energy, reproductive state, sustained deficit, forage availability, and
remains are durable gameplay facts. Continue the accepted lazy checkpoint
posture for slow state: a crash may restore a recent checkpoint rather than
the last cosmetic tick. Birth, death, natural remains creation, player harvest,
and block/resource mutations dirty every affected record immediately.

The existing residual lack of crash-atomic multi-record entity batches must be
made explicit. The in-memory authoritative transition is serialized once;
checkpoint retries must be idempotent and must not duplicate a kit, revive a
death, or create two remains records. If this tactical cannot make those
transitions safe with current revision checks, land a bounded atomic ecology-
event batch rather than accepting conservation only in the no-crash harness.

An inactive or unavailable animal/resource record freezes. Loading one animal,
refuge, forage cell, or remains record does not activate its neighbors, force
terrain load, or advance missed time.

## Performance Bounds

- one immutable and explicitly serialized chunk set per run;
- no whole-world entity or resource scan at tick cadence;
- spatially indexed mate, forage, refuge, herd, remains, and neighbor queries;
- lifecycle/resource reconsideration at stable identity-staggered slow
  cadences, with urgent safety still immediate;
- fixed per-tick decision, habitat, path, neighbor, birth, death, and remains
  work budgets;
- at most bounded daily report work, with streaming JSON/CSV rather than a full
  in-memory event history;
- explicit peak living entities, remains records, loaded chunks, memory, tick
  time, and total run time in every receipt; and
- deterministic degradation under overload: defer low-priority courtship or
  habitat work before movement safety, death finalization, or player-visible
  mutation.

The runner must never accelerate by weakening collision, path reachability,
resource ownership, or persistence semantics. Performance tuning follows the
profile rather than silently shrinking the simulation.

## Human Review Points

1. **Untuned rabbit curves:** after the closed boundary, forage cells, energy,
   and natural rabbit reproduction work, inspect several 30-day runs before
   adding deer population rules. Review whether growth, resource drawdown, and
   suppression reasons are understandable rather than merely numerically
   bounded.
2. **Remains asset and lifecycle:** inspect the source-first non-graphic prop
   at rabbit/deer scale and one loaded decay sequence before promotion.
3. **Rabbit/deer contrast:** inspect untuned multi-seed curves after deer
   maturation and reproduction land; confirm the two species do not share one
   reskinned growth model.
4. **Final campaign:** review the 120/200-day matrix, worst windows, invariant
   table, runtime cost, and proposed production defaults before closing the
   tactical.

## Implementation Slices

1. Record this tactical and link the living ecology topics.
2. Add immutable entity-ticking availability, boundary rejection, exact
   conservation diagnostics, and a minimal real-seed headless session.
3. Add versioned tuning, persisted forage-resource cells, rabbit energy costs,
   wild feeding, reproduction, maturation timing, and suppressed-reason
   receipts.
4. Run and review untuned 30-day rabbit curves; correct model defects before
   tuning constants.
5. Add deer age, energy, maturation, compatible mating, fawn birth, and slower
   species policy through the same bounded lifecycle owner.
6. Add stable old age, prolonged-deficit mortality, persistent remains,
   source-first presentation, decay, and exact biomass conservation.
7. Add the deterministic CLI matrix, JSON/CSV receipts, self-contained curve
   report, performance accounting, and full-server equivalence canary.
8. Run the declared real-seed 120/200-day campaign, tune production defaults,
   inspect the review report and remains pixels, update living docs, and commit
   the exact accepted evidence.

## Acceptance

- A fixed seed/window/run command reproduces byte-stable daily and final
  receipts through the ordinary Mclone initial population and authoritative
  simulation owners.
- The entity-ticking chunk set never changes, every out-of-domain path or
  target resolves unavailable, and `boundaryViolation` remains zero.
- With no external commands, daily living identities equal initial identities
  plus births minus deaths; post-initial spawn, duplicate, and unexplained
  removal counters remain zero.
- Rabbits obtain reproductive condition through reachable wild forage as well
  as player carrots; energy costs, cooldowns, maturation, crowding, and
  resource pressure are persisted and observable.
- Deer fawns mature and compatible adults reproduce through a slower,
  independently tunable policy while retaining current herd, bedding, alarm,
  hunting, and presentation behavior.
- Natural old-age and prolonged-deficit deaths create one durable bounded
  remains record, and loaded decay satisfies exact biomass conservation.
- The full real-seed campaign demonstrates deterministic bounded population
  bands or explainable scarcity without normal runs hitting technical caps.
- Runtime cost and AI/resource work remain bounded across growth and decline;
  the focused dense-population fixtures still pass.
- A shorter full-authoritative canary matches any accelerated lifecycle path,
  and inspected population charts plus native/Web remains pixels are retained
  under `/tmp`.
- All affected Rust, persistence, protocol, asset, renderer, server, headless,
  and browser boundaries pass the proportional validation matrix.

## Non-Goals

- moving tickets, immigration, emigration, recolonization, migration between
  independently simulated regions, or unloaded catch-up;
- predators, scavenger AI, disease, seasons, weather mortality, genetics, or
  domestication;
- a hard ecological quota, silent despawn, seed resurrection, or periodic
  generic Mclone wildlife spawning;
- simulating the whole world, loading remote homes on demand, or treating a
  coarse resource/population record as a visible animal;
- one gameplay block mutation per grass bite, a separate simulation formula,
  a behavior-tree framework, or a public universal animal API;
- changing Java 1.17.1 reference-profile spawning or lifecycle parity; and
- turning the report into a playable showcase or accepting screenshots without
  exact multi-day receipts.
