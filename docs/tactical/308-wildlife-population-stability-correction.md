# Tactical 308: Wildlife Population Stability Correction

Status: implementation landed 2026-08-15; accepted revision-5 120/200-day
campaign remains open after the user requested wrap-up

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

## Execution Record

The nest correction landed in `ecf7707b`. Persisted mallard site memory and
runtime nest attendance are now separate: hydration reconstructs attendance
only from a live compatible nest, successful hatching clears active intent,
and later reuse must pass the ordinary condition, mate, route, and nest path.
Focused hydration, hatch-release, and reuse tests pass.

Lifecycle revision 3 landed in `895744e4`. It changes no resource potential,
recovery, population target, or spawn path. Rabbit maturation and cooldown are
two loaded days, rabbit lifespan is 60-80 days, mallard maturation is two days,
mallard cooldown is four days, mallard lifespan is 90-110 days, both species
tolerate six days of sustained deficit, and a mallard pays one maintenance
unit per lifecycle cadence on shore or water. The same-seed revision-3
30-day diagnostic produced:

| radius-8 Overworld window | initial R/D/M | peak R/D/M | final R/D/M |
|---|---:|---:|---:|
| dry meadow/woodland | 30/11/0 | 94/43/0 | 88/43/0 |
| river meadow | 31/5/6 | 106/29/9 | 99/29/8 |
| river steppe | 16/4/4 | 50/31/5 | 38/31/5 |
| negative river/mountain | 20/13/12 | 68/47/14 | 63/47/6 |

Every exact invariant passed, and the preceding production defaults had ended
the same rabbit windows at 10, 42, 7, and 20 after much larger synchronized
birth/death pulses. However, the first longer revision-3 steppe diagnostic
showed another causal problem: by day 84 deer had grown from 4 to a peak of 70
while rabbits fell from a peak of 50 to 10 despite continued births. The deer
four-day breeding cadence was monopolizing their shared low-herbaceous forage.

Lifecycle revision 4 therefore changes only the deer breeding cooldown from
four to twelve loaded days. It landed in `f90493fe`; deer maturation, energy,
food, lifespan, mortality, and movement remain unchanged. Repeating the exact
30-day matrix at checksum
`f951d5859d999cdc5e074872ff2bbd529178b8e34ca54987f6acddcdd61e3007`
produced:

| radius-8 Overworld window | initial R/D/M | peak R/D/M | final R/D/M |
|---|---:|---:|---:|
| dry meadow/woodland | 30/11/0 | 108/26/0 | 107/26/0 |
| river meadow | 31/5/6 | 118/10/9 | 110/8/6 |
| river steppe | 16/4/4 | 51/10/6 | 36/10/6 |
| negative river/mountain | 20/13/12 | 73/26/14 | 66/26/6 |

Revision 4 retains ordinary scarcity, starvation, nest failure, and habitat
differences while removing the deer pulse that delayed rabbit collapse. All
identity, biomass, resource, boundary, no-refill, and overload invariants pass.

Its first 120-day steppe run exposed one more concrete mallard defect. The
flock established later nests and retained healthy mixed-sex adults, but a
failed parented nest remained alive after a parent died. Every later ready pair
was redirected to that sterile nest, so two early ducklings were the only
recruitment and the flock reached zero on day 108. The run ended on day 120 at
8 rabbits, 31 deer, and 0 mallards and was rejected.

Revision 5 landed in `bf537ade`. Authoritative natural death now removes any
nest that records the dead mallard as a parent, replicates that removal, and
clears surviving parent intent. It does not infer death from an unloaded or
unavailable parent. A focused regression proves that the failed nest is
released and a later conditioned pair creates an ordinary replacement nest;
all 15 mallard-focused server tests pass. The revision-5 steppe 30-day receipt
remains exactly 36 rabbits, 10 deer, and 6 mallards. The broader 30-day rerun
and long campaign were stopped when the user requested wrap-up, so the final
120/200-day acceptance remains deliberately open.

The exact rerun command is:

```text
pnpm native:wildlife:campaign --matrix \
  test/fixtures/creatures/wildlife-population-matrix-v2.json \
  --output <empty-temporary-directory> --jobs 5
```

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
