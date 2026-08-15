# Tactical 308: Wildlife Population Stability Correction

Status: in progress 2026-08-15

Topic: `wildlife-ecology-state-model`

## Instruction Synthesis

Correct the first long-run rabbit and mallard population failures with a
small, evidence-led slice. Productive real Mclone Overworld windows should no
longer turn an initial reproductive pulse into universal extinction merely
because ordinary adults cannot resume feeding or because placeholder lifecycle
timings synchronize growth and death too aggressively.

Keep the individual loaded simulation authoritative. Do not add population
refill, immigration, a desired count, silent despawn, a hard ecological cap,
or campaign-only survival logic. Poor habitat may still lose a species, and a
closed finite population is never promised mathematical immortality.

## Diagnosis

The corrected 30-day Tactical 301 matrix exposes two different problems:

- Rabbit founders begin healthy, kits mature after one day, and parents may
  breed again after 18,000 ticks. The delayed four-day starvation threshold
  permits a large early overshoot. Default lifespan is only 20-25 loaded days,
  so the crash then combines starvation with a synchronized old-age cohort.
- Burrows are not the common cause. The dry control recorded no known or
  occupied rabbit refuge while still growing from 30 founders to more than 100
  rabbits before declining.
- A mallard's persisted `nest_target` currently means both "remembered usable
  site" and "steer here now." Successful hatching deliberately preserves it
  for site reuse, which unintentionally pins both parents to the old nest
  through their cooldown. Position-gated feeding can then starve otherwise
  healthy birds. In the meadow diagnostic, several successful parents stayed
  effectively stationary and five mallards starved around days four through
  six.

## Binding Decisions

### Remembered sites do not command movement

Separate mallard nest-site knowledge from active nesting intent. A successful
hatch may preserve one bounded valid remembered shore block, but both parents
must immediately resume ordinary alternating water/shore behavior. A currently
forming or incubating nest may establish an explicit runtime nest intent; nest
loss clears both active attendance and stale memory when the saved site is no
longer valid.

Persistence continues to store the remembered site in the existing bounded
mallard payload. Runtime intent remains reconstructible from a live nest and
current reproduction attempt, so loading memory alone must never pin an
animal, activate terrain, or imply an available route.

### Tune lifecycle pressure, not outcomes

Revise the versioned production lifecycle hypotheses conservatively:

- lengthen rabbit maturation and breeding intervals enough to remove the
  one-day near-exponential pulse;
- lengthen rabbit and mallard lifespans so old age remains visible in long
  campaigns without eliminating whole founder cohorts during an ordinary
  20-30-day observation;
- allow a somewhat longer sustained deficit than one resource recovery cycle;
  and
- adjust reproductive thresholds or costs only if same-seed sweeps show that
  timing changes alone still produce an unexplained surge.

Do not tune deer merely because the shared record changes revision. Do not
raise terrain resource potential or recovery until the movement/intent defect
is corrected and a same-seed campaign shows an actual production shortfall.

### Evidence before promotion

Use the exact Tactical 301 four-window, radius-8, 30-day matrix as the first
comparison. Inspect births, peak populations, deaths by cause, energy,
resource draw, and final age structure rather than accepting only a nonzero
last row. Then run the declared 120/200-day real-seed matrix. Productive wet
windows should retain mallards capable of repeated clutches and productive
rabbit windows should show bounded generations rather than universal
extinction. The dry control may remain mallard-free.

All identity, biomass, boundary, resource, deterministic receipt, and
full/accelerated-equivalence invariants remain mandatory. A result that survives
only because it reaches the 4,096 hard overload guard fails.

## Implementation Slices

1. Record this tactical and the diagnosed memory/intent distinction.
2. Split remembered mallard nest sites from active runtime attendance, with
   focused hatch, reuse, destruction, and resumed-foraging tests.
3. Run the 30-day matrix with the behavior fix and current timings to isolate
   its effect.
4. Sweep a small set of explicit tuning records, promote the least aggressive
   defaults that remove the diagnosed boom/crash failure, and bump the tuning
   revision.
5. Run focused server/persistence tests, the four-window diagnostic, and the
   long real-Overworld matrix; update this tactical and the living topic with
   exact evidence.

## Acceptance

- A remembered successful nest site does not create an active nest movement
  target after hatching or hydration.
- Parents with a remembered site resume ordinary water/shore travel, can feed,
  and may later reuse the same still-valid site through the normal condition,
  mate, cooldown, route, and nest path.
- Rabbit defaults reduce the early reproductive pulse and avoid universal
  extinction across productive accepted windows without a desired-population
  branch or resource refill shortcut.
- Productive wet windows retain a naturally reproducing mallard population;
  dry windows do not acquire synthetic aquatic support.
- All campaign conservation and determinism invariants pass, and population
  behavior remains explainable from individual histories.

## Non-Goals

- immigration, recolonization, moving tickets, catch-up simulation, predators,
  seasons, disease, genetics, or a coarse population actor;
- guaranteeing every seed/window retains every species forever;
- changing initial population geography, Java 1.17.1 worldgen, or resource
  cell geometry; and
- using a showcase, flat fixture, or aggregate equation as final population
  evidence.
