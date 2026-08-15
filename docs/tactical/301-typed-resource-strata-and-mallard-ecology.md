# Tactical 301: Typed Resource Strata and Mallard Ecology

Status: implementation landed 2026-08-15; typed resources, three-species
lifecycle, natural mallard nesting, and corrected 30-day evidence complete;
accepted long campaign and final native/Web remains review pending

Topic: `habitat-driven-creature-ecology`

Topic: `wildlife-ecology-state-model`

## Instruction Synthesis

Continue the predator-free loaded ecology through mallards before adding
predators or scavengers. Replace the first rabbit/deer forage scalar with a
small set of terrain-derived renewable resource strata shared by every
consumer. Give rabbits, deer, and mallards distinct, partially overlapping
diets so habitat, access, competition, condition, reproduction, and mortality
produce understandable population behavior without hard species quotas.

Use the long-run real-seed simulation from Tactical
[`299`](299-closed-domain-wildlife-population-simulation.md) to calibrate and
explain the model. A rough ecological support estimate may be useful as a
diagnostic comparison, but it must never become an authoritative population
target or simplify away the individual simulation. Natural, local interactions
remain the gameplay model; the estimate is optional evidence about whether the
observed result is surprising.

This tactical ends after mallards participate in the same loaded lifecycle,
resource accounting, mortality, conservation, and multi-day reports as rabbits
and deer. Fish, beavers, squirrels, predators, and scavenger behavior remain
later consumers chosen from the evidence this slice produces.

## Starting Point

Tactical 299 has landed the first production-shaped foundation:

- one immutable entity-ticking domain over ordinary Mclone Overworld chunks;
- deterministic initial rabbit/deer realization with no later refill;
- shared loaded age, energy, deficit, reproductive condition, birth, old-age
  death, starvation, and persistent remains mechanics;
- one persisted renewable forage scalar per 64-by-64-block cell, locally gated
  by an actual reachable feeding action;
- species-owned rabbit and deer feeding, mating, maturation, and birth policy;
- exact identity, boundary, resource, and biomass invariants;
- a deterministic real-seed campaign runner with JSONL, CSV, and HTML evidence;
  and
- a full-server/accelerated equivalence canary.

The scalar forage ledger is intentionally a first proof, not the final ecology.
It currently combines grass, dirt, flowers, and leaves into one amount that
rabbits and deer can consume interchangeably. That hides which terrain fact is
supporting which animal, exaggerates direct competition, and cannot represent
wetland feeding without turning all landscape productivity into generic food.

Mallards already have durable identity, duckling growth, parent IDs, water and
shore behavior, periodic eggs and feathers, calls, player-placed nests,
habitat-gated incubation, and attended hatching. They do not yet have the
shared wildlife life state, sex, feeding condition, spontaneous population
reproduction, natural mortality, or long-run receipts. Their current egg timer
is a collectible-resource timer rather than evidence of a complete wild
population lifecycle.

## Binding Decisions

### Resource strata are shared habitat facts

Replace the scalar forage value with a small closed enum of resource strata.
The first revision contains only types exercised by the three current
consumers:

| Resource stratum | Ordinary terrain evidence | First consumers |
|---|---|---|
| low herbaceous growth | grass, low plants, flowers, supported open ground | rabbit, deer |
| woody browse | reachable leaves and woody edge cover | deer |
| seeds and soft mast | seed-bearing low plants, flowers, and suitable woody margins | mallard |
| aquatic vegetation | shallow water with wetland plants and vegetated banks | mallard |
| aquatic invertebrates | shallow, supported, sheltered water as a habitat proxy | mallard |

These are renewable ecological accounting units, not inventory items and not
one invisible block per unit. The enum may later gain a resource only when a
real consumer or landscape mechanic requires it. Do not prebuild nectar,
pollen, carrion, fish prey, bark, roots, berries, or every imagined food type
in this slice.

No species owns a private copy of a landscape resource. Two animals drawing
from low herbaceous growth affect the same cell and therefore compete. Animals
using different strata can coexist without multiplying one meadow's energy by
the number of registered species. Diet overlap is explicit and partial rather
than all-or-nothing.

Keep the current 64-by-64-block resource-cell lattice unless profiling or
behavioral evidence shows it is too coarse. Each instantiated cell stores one
fixed-size value record per stratum: potential, available, recovery remainder,
recovered total, terrain revision, sample time, and bounded consumption totals.
Use fixed enum-indexed arrays or equivalent closed records rather than string
maps or per-block resource entries.

### Terrain sets potential; reachable behavior authorizes intake

Sampling continues to read ordinary generated and edited blocks around real
feeding sites. It must distinguish the strata rather than summing them and
must permit zero potential. A dry bare site cannot receive a synthetic minimum
of aquatic food, woody browse, or grass merely to keep a population alive.

Potential is a capacity signal. Available amount recovers only while the
owning resource cell overlaps the active domain and only at that stratum's
declared cadence/rate. A bounded terrain resample may raise or lower potential,
but it may not disguise depletion as a free refill. Reports expose both
terrain-caused capacity changes and biological recovery.

An accounting cell is never a remote feeding target. Every bite still requires
the species to:

1. choose a locally suitable food site through ordinary behavior;
2. obtain complete active terrain and route evidence;
3. physically reach and perform the feeding action; and
4. draw an appetite-sized amount from an eligible stratum in that cell.

Water, fences, terrain edits, collision, unavailable chunks, and failed paths
therefore remain meaningful. A duck cannot eat an aquatic resource while
standing at an unrelated dry corner of the same cell, and a deer cannot browse
leaves through an impassable barrier. Crops remain visible block state and use
their existing mutation and drop paths instead of becoming ledger units.

### Diet profiles express preference and substitution

Add one small, versioned diet profile per participating species. A profile
declares eligible strata, preference order or weights, intake conversion, and
the amount an ordinary feeding action may request. It does not decide when the
animal feeds, where it travels, whether it feels safe, or whether a mate is
acceptable; those remain species policy.

The first diets are:

- rabbits strongly prefer low herbaceous growth and retain real garden-crop
  raids and player carrots as separate high-value events;
- deer prefer woody browse near edge cover and may use low herbaceous growth,
  leaving real overlap with rabbits without making their diets identical; and
- mallards use aquatic invertebrates and vegetation in shallow water, with
  seeds/soft mast available from suitable shore feeding.

Diet substitution must preserve resource identity in receipts. If a deer uses
grass because browse is depleted, the report shows that choice. Conversion
values are hypotheses to tune from observed individual condition and
population curves, not statements that one unit is a literal calorie.

### The individual simulation remains authoritative

Do not add a carrying-capacity number to a cell or species record. Population
support continues to emerge from individual access to regenerating resources,
energy cost, safety, local crowding, mate opportunity, maturation, birth
cadence, sustained deficit, old age, and player disturbance.

No estimator may feed spawning, breeding permission, mortality, movement,
despawn, resource recovery, or acceptance bands. In particular, do not
implement any of these shortcuts:

```text
population < estimated_capacity -> spawn or guarantee a birth
population > estimated_capacity -> suppress, kill, or despawn
species ratio differs from target -> bias the next lifecycle event
```

The fixed seed-authored population remains only the initial condition. There
is still no post-initial spawn, immigration, emigration, unloaded catch-up, or
coarse population actor in the closed-domain run.

### A support estimate is optional diagnostic evidence

The report may include a clearly labeled `diagnosticSupportEnvelope` when it
can be derived without creating another simulation. Its purpose is to compare
rough expectation with observed outcomes and expose mistaken units, resource
bottlenecks, or missing pressures.

Prefer a modest range based on observed per-stratum recovery, configured intake
conversion, and observed maintenance demand. With substitutable diets and
shared resources, it may describe limiting strata or a feasible region rather
than produce one magic carrying-capacity value. Serialize every assumption and
show the residual between estimated animal-days and the actual run.

This diagnostic is not required to accept the resource or mallard mechanics.
If a useful envelope requires an optimizer, a mean-field population model, or
assumptions that obscure the actual histories, omit it. Raw recovery,
consumption, deficit, suppression, birth, and death curves are the primary
evidence. A poor estimate is information about the estimate, not a reason to
force the simulation toward it.

### Rabbit and deer migration preserves species behavior

Move the current rabbit/deer scalar intake onto diet profiles without changing
their locomotion, refuge, herd, alarm, player avoidance, crop, courtship, or
birth ownership. The migration is accepted only if short exact fixtures still
prove the established behaviors and if long-run changes are explainable by the
new resource composition.

Before tuning mallards, run the radius-8 predator-free rabbit/deer baselines
with typed resources. Compare them with the retained scalar-ledger receipts,
but do not require identical populations. The intended difference is visible:
deer should exert more pressure on woody browse, rabbits on low growth, and
their shared low-growth draw should remain measurable.

### Mallards become the third loaded lifecycle consumer

Extend the shared wildlife species/lifecycle vocabulary to mallards while
retaining mallard-owned movement, flock, waterline, call, feather, nest, and
incubation behavior.

Every mallard persistently records:

- stable sex, age, lifespan, energy, sustained deficit, recent intake,
  reproductive condition, and cooldown;
- existing parent identities, feather/call timers, and life-stage presentation;
  and
- the minimum nest or mate intent required for an interruptible reproduction
  attempt, without introducing a universal home field.

Founders receive stable identity-derived sex and lifespan. Duckling growth uses
the shared age source rather than a second capped age counter. Activity costs
distinguish swimming/flight-like urgent travel from rest and local feeding only
where existing movement evidence can support that distinction.

A mallard feeds only at a physically reached shallow-water or shore site that
matches the chosen stratum. Low condition first increases feeding priority and
suppresses reproduction; only sustained deficit causes natural death. Natural
death creates the same bounded persistent remains record with mallard source
species and biomass. Existing player-facing drops remain ordinary items.

### Natural nesting reuses the ordinary nest mechanic

Periodic collectible egg shedding and population reproduction are distinct
facts. Keep the current collectible egg/player-placement stewardship loop, but
do not let a wild loaded population depend on player item use to produce its
next generation.

A healthy mature female with a compatible mature male and recent successful
intake may attempt one natural clutch after its cooldown. The female must find
and reach a valid covered wetland shore through existing habitat and
availability rules. A successful attempt creates the same durable semantic
mallard-nest entity used by player placement, with parent identities and a
single authoritative lifecycle transition. It does not create showcase-only
state or a hidden abstract nest.

The existing habitat and adult-attendance rules continue to pause or advance
incubation. Successful hatching creates ordinary durable duckling identities
through the shared birth event and conservation accounting. Nest loss,
unavailable terrain, absent attendance, low condition, crowding, or stale
intent defers or suppresses the attempt without inventing a replacement in an
unloaded area.

Natural nest-site reuse and local density must bound nest proliferation. Do not
add a global mallard cap or make every periodic egg a new duckling. A player-
placed nest remains a meaningful way to improve the odds by selecting protected
habitat; it uses the same incubation and offspring rules as a natural nest.

### Persistence evolution is explicit

Revise the internal unshipped forage saved-data schema deliberately rather
than interpreting the old scalar as every new resource. Either migrate its
available fraction into only the low-herbaceous/woody evidence supported by a
fresh bounded sample, or reject the obsolete internal record with an actionable
diagnostic and regenerate disposable development worlds. Record the chosen
policy in the compatibility ledger and tests.

Mallard lifecycle, natural nest creation, hatch birth, natural death, and
remains creation follow the lifecycle/transfer durability classes in
[`../topics/wildlife-ecology-state-model.md`](../topics/wildlife-ecology-state-model.md).
A crash may roll ambient energy or intent back to a checkpoint, but it may not
duplicate a nest, duckling, death, or remains record. Codec fixtures cover the
new fields, legacy mallard hydration, unload/reload, and idempotent lifecycle
transitions.

## Diagnostics and Campaign Evidence

Extend the Tactical 299 receipts rather than creating a second runner. Each
daily row and final summary records:

- potential, available, recovered, and consumed units by resource stratum;
- consumption by species, life stage, and selected diet stratum;
- feeding attempts rejected for unavailable terrain, unreachable site, empty
  resource, or incompatible local geometry;
- energy, deficit, reproductive condition, births, maturation, and deaths for
  mallards alongside rabbits and deer;
- natural and player-established nests, failed/suppressed nest attempts,
  attendance days, hatches, and nest loss;
- living identities by species/sex/life stage and exact conservation;
- remains biomass by source species and cause; and
- optional diagnostic support ranges and their serialized assumptions.

Add deterministic site selection for wet mixed radius-8 windows containing
multiple initial rabbit/deer groups and at least one mallard flock. Retain dry,
meadow, and forest-edge windows to prove that absent wetland resources remain
zero and do not accidentally support mallards.

Use three evidence scales:

1. focused exact fixtures for sampling, competition, diet choice, lifecycle,
   nesting, persistence, and invariants;
2. several 30-day real-seed diagnostic runs after rabbit/deer migration and
   again after mallards join; and
3. the accepted 120-day matrix plus selected 200-day stress runs inherited
   from Tactical 299.

Do not define success as one final population. Inspect distributions, peaks,
troughs, resource pressure, non-predator mortality, recovery, and run-to-run
variance. A habitat may support sparse or no mallards if its aquatic strata are
honestly poor. Productive sites should avoid unexplained monotonic explosion,
but oscillation, local nest failure, and stochastic-looking individual outcomes
are desirable when they remain deterministic for the same receipt.

## Performance Bounds

- one fixed small resource-stratum enum and bounded record per instantiated
  64-by-64 cell;
- no per-block renewable ledger, whole-world scan, or per-species copy of a
  resource cell;
- stable staggered sampling/recovery and species decision schedules;
- spatially bounded feeding, mate, flock, nest, and neighbor queries;
- no quadratic cross-species matching or all-animal diet pass;
- no report-time history retained in server memory beyond the existing
  streaming receipt bounds; and
- no estimator solver in the gameplay tick or production server path.

The dense-rabbit work-budget proof remains a regression. Add a bounded mixed-
species fixture that reports resource and neighbor work by species while
preserving fair progress. A normal campaign hitting the hard overload guard is
an ecological failure, not a successful cap.

## Human Review Points

1. **Typed rabbit/deer resources:** inspect terrain/resource maps and several
   30-day curves before tuning constants. Confirm that browse, low growth, diet
   substitution, and shared pressure are legible rather than merely more
   counters.
2. **Diagnostic estimate boundary:** if the optional support envelope is
   useful, review its assumptions beside raw curves. Remove or demote it if it
   appears prescriptive, falsely precise, or easier to read than the actual
   causal evidence.
3. **Mallard feeding and natural nest:** inspect ordinary native and Web pixels
   plus a bounded behavior trace showing water/shore feeding, condition-driven
   nest establishment, attendance, and hatching without harness commands.
4. **Three-species campaign:** review the 120/200-day matrix, worst windows,
   resource bottlenecks, population distributions, invariant table, and runtime
   cost before promoting production defaults.

## Implementation Slices

1. Record this tactical, update the living ecology topics, and mark Tactical
   299's landed foundation versus its pending accepted campaign.
2. Introduce the fixed resource enum, versioned cell records, terrain-derived
   per-stratum potential/recovery, persistence evolution, exact accounting, and
   focused sampling tests.
3. Add versioned rabbit/deer diet profiles and migrate their feeding actions
   while retaining locomotion, refuge/herd, reproduction, and persistence
   regressions.
4. Run radius-8 30-day rabbit/deer comparisons; correct model defects and
   establish understandable resource-pressure evidence before changing
   production tuning.
5. Optionally add the report-only support envelope if the raw data supports a
   useful, honestly bounded estimate without a second simulation.
6. Extend shared lifecycle/remains state and codecs to mallards, unifying age
   and adding stable sex, condition, deficit, cooldown, and natural mortality.
7. Add mallard diet-driven shallow-water/shore feeding and condition-driven
   natural nest establishment through the ordinary nest entity and existing
   attended incubation path.
8. Extend exact conservation, resource, nest, hatch, and remains receipts; add
   focused mixed-species and full/accelerated equivalence tests.
9. Capture and inspect native/Web mallard behavior and remains pixels, then run
   and review the declared real-seed 30/120/200-day evidence.
10. Tune only from the observed histories, update the living topics and
    production defaults, and record the exact accepted commands/checksums.

## Acceptance

- Resource cells persist a bounded fixed set of distinct terrain-derived
  strata; zero-support terrain remains zero, recovery never exceeds potential,
  and every unit is conserved by stratum.
- Rabbits and deer use explicit different diets with measurable low-growth
  overlap, and no established refuge, herd, flight, garden, reproduction, or
  persistence behavior regresses.
- Every intake follows a physically reached compatible feeding action inside
  the active domain; a cell cannot authorize remote, blocked, dry-land aquatic,
  or unavailable-chunk feeding.
- Mallards persist shared lifecycle condition, feed from wetland/shore strata,
  reproduce naturally through the ordinary durable nest/hatch path, mature,
  die naturally after old age or sustained deficit, and leave conserved
  remains.
- Player egg collection and nest placement remain functional stewardship paths
  and share habitat, incubation, birth, and persistence semantics with natural
  nests.
- Daily identity conservation holds for all three species, biomass
  conservation includes mallards, and post-initial spawn, duplicate identity,
  unexplained removal, and boundary-violation counters remain zero.
- The accepted real-seed matrix shows explainable habitat/resource differences
  and bounded individual simulation without normal use of hard technical caps.
- Full and accelerated paths remain exact for resource, birth, death, nest,
  remains, and final-identity facts.
- Any support estimate is labeled diagnostic, absent from production decisions,
  reproducible from serialized assumptions, and removable without changing a
  single gameplay outcome.
- Focused Rust, persistence, protocol, asset, renderer, server, headless, Web,
  and campaign validation passes in proportion to the affected boundaries.

## Non-Goals

- predators, scavenger behavior, carrion consumption, disease, seasons,
  weather mortality, genetics, or domestication;
- fish, beavers, squirrels, wild pigs, crows, or their resource types and
  behaviors;
- a hard ecological carrying capacity, desired species ratio, population
  refill, silent despawn, or estimator-driven correction;
- replacing materialized individuals with equations, daily aggregate
  transitions, or a mean-field/offscreen population simulation;
- immigration, emigration, moving tickets, remote home loading, inactive
  catch-up, or world-scale ecology;
- visible grass destruction for every bite or conversion of crops into
  invisible resource units;
- a universal behavior tree, permanent-home trait, general-purpose ecology
  framework, or public mod API; and
- changing Java 1.17.1 reference-profile spawning or lifecycle behavior.
