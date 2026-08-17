# Tactical 315: Authoritative Seasonal Calendar And Squirrel Ecology

Status: active coordinating parent 2026-08-17; Phase A implemented by
[`316`](316-authoritative-season-calendar-foundation.md), Phase B implemented by
[`317`](317-civil-time-discontinuity-and-sleep.md)

Topic: `seasons`

Topic: `habitat-driven-creature-ecology`

Topic: `wildlife-ecology-state-model`

Tactical role: **coordinating parent.** This record binds the connected product
contract and sequences six bounded implementation phases. Before changing code
for a phase, create and index one focused child tactical for that phase (or a
smaller child when a review gate divides it further). Do not implement this
entire parent as one undifferentiated code slice or commit.

## Instruction Synthesis

Make the existing Mclone seasonal model authoritative before asking wildlife
to depend on it. Preserve the current split between monotonic simulation work
and mutable civil time: `game_time` / `simulation_tick` advances once per real
authoritative tick, while persisted `day_time` supplies date and time of day
and may jump through sleep or an explicit calendar command. Derive one global
orbital phase from that civil clock, then evaluate local seasonal climate from
orbital phase, effective latitude, terrain climate, and altitude.

Treat sleep and administrative date changes as sharp authority changes, not
requests to replay missed simulation. A successful night skip moves civil time
to the next morning, wakes the players, and emits the new clock. It does not
run the skipped entity, crop, block, scheduled-tick, resource, pregnancy,
aging, hunger, weather, or hydrology work. Moving the calendar backward changes
current conditions without rewinding world state.

Let seasons affect wildlife populations through ordinary causes rather than a
population multiplier. Local seasonal climate changes resource accessibility
and loaded recovery or production rates. Animals still have to reach food,
consume persistent stock, maintain condition, find mates, reproduce, and die
through the existing individual lifecycle. No calendar transition directly
adds, removes, births, kills, heals, starves, or retargets an animal.

Promote squirrels as the next non-predator ecology pressure test. Use the
existing seeds-and-soft-mast resource stratum, woodland/edge terrain evidence,
bounded individual place knowledge, tree refuge or cavity use, and distributed
food caches. A squirrel physically gathers mast, carries it, deposits it at a
real bounded cache site, remembers only a small number of sites, and later
retrieves food under lean conditions. Cache transfers must conserve resource
units across terrain stock, carried stock, cache stock, consumption, death,
save/reload, and unavailable terrain.

Use compressed ecological chronology rather than literal real-world annual
waiting. Broad favorable seasonal windows may bias reproductive attempts, but
food, condition, safety, space, and mate access remain the gates. Gestation,
maturation, aging, hunger, and cooldowns remain active-simulation durations.
Do not make missing one short spring force the player or population to wait an
entire long year before another meaningful opportunity.

This tactical completes one connected calendar-to-resource-to-animal loop. It
does not implement migration, anonymous seasonal wildlife reconstruction,
unloaded population simulation, hibernation, a predator, or a general-purpose
ecology fact database.

## Starting Point

The repository already has most of the necessary ownership boundaries:

- `mclone-server::RealmServer` persists and advances `simulation_tick` and
  `day_time` separately;
- `simulation_tick` is the durable vanilla-shaped `game_time`, while
  `day_time` is the authoritative day/night clock replicated through
  `ServerUpdate::TimeUpdate`;
- `set_day_time` is currently a non-persistent Debug starting-time hook, and
  no ordinary gameplay operation changes the calendar discontinuously;
- `mclone-season` owns pure fixed-point `OrbitalPhase`, effective-latitude,
  local-season, appearance, and solar samples without owning a server,
  renderer, persistence backend, or platform host;
- Tactical [`306`](306-seasonal-appearance-preview.md) uses a provisional
  112-day Debug projection, explicitly not an authoritative year-length or
  persistence decision;
- Tactical [`307`](307-seasonal-solar-path-and-cyclical-latitude.md) supplies
  the accepted global orbital landmarks, `27`-degree tilt, and cyclical-plane
  latitude policy;
- the Seasonal Debug UI can show a manual preview but has no `World Calendar`
  source;
- wildlife resource cells persist five typed strata and recover only while
  their 64-by-64-block cells overlap the active domain;
- rabbit, deer, and mallard diets consume physically reached resource sites;
- the loaded lifecycle owns condition, maturation, reproduction, birth,
  lifespan, mortality, remains, and exact population conservation;
- rabbits already exercise bounded individual place knowledge and explicit
  available/unavailable/invalid outcomes;
- the deterministic initial-wildlife planner currently realizes rabbit,
  deer, mallard, and bee groups without post-initial refill;
- there is no squirrel figure, animation set, protocol kind, diet, habitat
  plan, cache record, tree-refuge behavior, call, or population report row;
  and
- there is no functional player bed, sleep command, sleep state, quorum, wake
  update, or ordinary next-morning clock operation.

Minecraft Java 1.17.1 supplies a narrow comparative sleep reference, not the
product design for Mclone seasons. In
[`ServerLevel.tick`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java),
once enough players are deeply asleep it assigns `dayTime` to the next
24,000-tick boundary, wakes the players, and clears weather when enabled. The
same ordinary server tick then advances `gameTime` once and performs one set
of current block/entity work. It does not execute every skipped nighttime
tick. Mclone retains that useful clock/simulation separation while making its
own decisions about quorum, weather, bed content, seasonal climate, and
wildlife.

## Objective

At one persisted original-Mclone world revision, the following sequence is
ordinary authoritative behavior:

1. The server advances one versioned world calendar from persisted civil time.
2. The client renders and labels the same world date and observer-local season
   without enabling Manual Preview.
3. A valid multiplayer sleep quorum advances civil time to the next morning
   while `simulation_tick` advances only through the one ordinary tick.
4. No skipped crop, scheduled block, animal, resource, cache, or lifecycle work
   is replayed.
5. Local seasonal climate changes accessible food and future loaded recovery
   continuously, without directly changing animal counts.
6. Woodland/edge seed planning may realize a small durable squirrel group.
7. A squirrel reaches mast, transfers units into carried stock, reaches a
   compatible cache site, deposits them, and later retrieves and consumes
   them when ordinary food is less accessible.
8. Leaving and returning restores the same squirrels, knowledge, carried food,
   caches, resource stock, calendar, and lifecycle state without inactive
   advancement.
9. Moving the date forward or backward immediately re-evaluates current local
   conditions but neither duplicates cache resources nor rewinds births,
   deaths, consumption, or player edits.
10. Native, browser, flat Android, and XR hosts consume the same authority,
    protocol, behavior, figure, animation, and render contracts.

The resulting population response must be emergent and explainable. Reports
may show that a lean season reduces intake, condition, or reproduction and
that caches buffer some squirrels, but no acceptance rule requires a chosen
final population count or species ratio.

## Binding Decisions

### Simulation time and civil time remain separate clocks

Retain one explicit clock vocabulary:

| Clock | Owner and purpose | Discontinuity policy |
|---|---|---|
| `game_time` / `simulation_tick` | server-owned count of executed simulation ticks; scheduled work, active aging, cooldowns, and deterministic tick seeds | monotonic; advances only for executed ticks and never jumps for sleep or date commands |
| `day_time` / civil time | server-owned persisted cumulative date plus time of day | normally advances with the daylight rule; may jump through typed sleep or calendar operations |
| orbital phase | pure `mclone-season` projection of civil time and the selected calendar policy | recomputed from current authority; never separately ticked or saved |
| local seasonal climate | pure position-dependent evaluation of orbital phase, latitude, annual climate, and altitude | recomputed on demand for active consumers and visible presentation |

Do not repurpose `game_time` as a date, move scheduled ticks when civil time
jumps, or add a second accumulating season counter. Do not store a mutable
orbital phase beside `day_time`; two independently writable clocks would
eventually disagree.

Use checked or saturating arithmetic at the civil-time boundary and test the
largest supported persisted values. Do not rely on accidental unsigned wrap
to begin a new epoch.

### The calendar is a versioned dimension/profile policy

Add shared dependency-leaf vocabulary equivalent to:

```text
SeasonCalendarPolicy =
    Disabled
    | Orbital {
          revision,
          days_per_year,
          phase_origin_day,
      }

AuthoritativeCalendarSample {
    civil_time_ticks,
    absolute_day,
    day_tick,
    year_index,
    day_of_year,
    orbital_phase,
}
```

`mclone-overworld-v1` opts into the orbital policy. Retained Java, Alpha,
Beta, Flat Grass, Small Island, Authored Only, and topology diagnostic
profiles remain unchanged unless a later explicit decision opts them in.
Topology alone does not enable seasons.

The policy revision and selected year length must be recoverable from durable
world/profile metadata. `day_time` remains the only advancing saved integer.
Changing code constants later must not silently reinterpret a persisted world
without an explicit internal migration or regeneration decision.

Use the existing global orbital landmarks:

```text
0.00  northward equinox
0.25  northern solstice
0.50  southward equinox
0.75  southern solstice
1.00  wraps to northward equinox
```

World day zero provisionally begins at the northward equinox. Record and test
the final phase origin rather than letting integer division accidentally
select it.

### Bind year length through play-scale calibration

The Debug-only 112-day projection is a useful candidate, not the answer by
inheritance. Before binding the authoritative `days_per_year`:

- compare at least one shorter, the current 112-day, and one longer candidate;
- report real unslept and sleep-every-night play time per season and year at
  the production tick/day cadence;
- compare those durations with ordinary walking, faster travel, crop cycles,
  current loaded wildlife maturation/cooldowns, squirrel caching, and likely
  session length;
- show how many favorable reproduction attempts a healthy loaded animal can
  receive in a broad local season;
- verify that a player can observe a cache-surplus-to-lean-retrieval story in
  a reasonable number of ordinary sessions;
- review opposite-hemisphere and weak tropical responses; and
- bind one exact integer year length through Human Review before resource or
  population tuning is accepted.

Do not shorten the year merely to make a test finish quickly. Long population
evidence already has an accelerated path proven against full authoritative
ticks. Conversely, do not select a literal real-world-feeling year that makes
the first seasonal animal mechanic practically unobservable.

### The world calendar is global and the season remains local

The authoritative server supplies one orbital phase. It does not replicate a
global `Spring` label. At an active position:

```text
authoritative orbital phase
  + generation profile and topology climate-coordinate policy
  + effective latitude
  + annual-mean temperature and moisture
  + altitude
  -> evaluated local seasonal climate
```

Northern and southern temperate positions at the same date must receive
opposite thermal seasons. Tropical regions may receive weak thermal response
and later wet/dry vocabulary. High elevations may be cold while nearby low
ground is mild. Every gameplay and presentation consumer uses the same shared
sample; none derives season from instantaneous sun elevation or a renderer
setting.

`mclone-server` owns when and where authoritative samples are requested.
`mclone-season` remains pure. Worldgen supplies annual climate and coordinate
facts without ticking a calendar. Platform and renderer crates do not acquire
calendar authority.

### World Calendar and Manual Preview are distinct sources

Extend Seasonal Debug with an explicit source selection:

```text
Season Source: World Calendar | Manual Preview
```

`World Calendar` displays the replicated absolute date, global milestone,
observer latitude, and evaluated local season. It is read-only from ordinary
client UI and drives the opted-in production appearance/gameplay paths.

`Manual Preview` retains the current client-local unsaved orbital, latitude,
solar-time, snow, and appearance controls. It never sends a server command or
changes gameplay. A developer/admin date command is a separate authorized
server operation with explicit diagnostics; dragging the preview slider must
never time-travel the world.

Exact feature-off tests must retain non-seasonal profiles and existing Debug
presentation. Do not make server authority depend on whether a client has the
Seasonal Debug screen open.

### Civil-time mutations are typed operations

Replace ambiguous whole-value mutation in gameplay with separate server-owned
operations:

```text
set_time_of_day_preserving_date(day_tick)
set_calendar_date(year_index, day_of_year, optional_day_tick)
advance_to_next_morning()
```

The existing startup/debug override may remain clearly named and non-durable.
Ordinary operations update persisted civil time, mark metadata dirty, and
publish one authoritative time/calendar update to every interested client.

`set_time_of_day_preserving_date` changes only `day_time % 24_000`.
`set_calendar_date` changes the absolute calendar and never claims to restore
the historical world that existed on that date. `advance_to_next_morning`
chooses the next valid morning boundary even near a year wrap.

Reject invalid day ticks, out-of-policy dates, unauthorized requests, and
arithmetic overflow. Record the reason in diagnostics. Do not silently clamp a
mistyped date into a different year.

### A discontinuity changes conditions, not history

When civil time jumps:

- the authoritative orbital and local climate samples change immediately;
- visual presentation may use a brief client-only blend to avoid a distracting
  pop, but gameplay consumes the new sample at once;
- `simulation_tick`, scheduled tick deadlines, cooldown origins, animal age,
  pregnancy progress, resource recovery remainders, crop age, and cache state
  remain unchanged;
- no missed sunrise, sunset, season boundary, bloom, mast, birth, death,
  starvation, or migration event is replayed;
- moving backward does not refund consumption, resurrect an animal, remove an
  offspring, refill a cache, or undo a player edit; and
- save/reload restores the post-jump civil clock and unchanged simulation
  history exactly.

Avoid boundary-crossing producers such as "entering autumn adds 100 mast."
They duplicate under repeated forward/backward changes and require historical
replay semantics. This tactical uses continuous current-condition functions
and active tick work. Later discrete annual encounters must carry explicit,
bounded event identities and receipts.

### Sleep performs one civil-time jump and no catch-up

Add a server-owned sleep state and quorum. The initial product rule is:

- only a living ordinary player can request sleep;
- the request must identify a loaded, valid, reachable sleep site in the
  player's current dimension;
- sleeping is admitted only during the accepted night window;
- observers, preview cameras, disconnected players, and dead/respawning
  players never count toward the quorum;
- all connected eligible realm players count, including an awake player in a
  dimension that shares the realm calendar, so one player cannot unexpectedly
  move everyone else's date;
- the initial default quorum is 100 percent, represented as a typed realm rule
  so a later multiplayer setting can lower it without changing the clock
  operation;
- moving, taking damage, losing the sleep site, changing dimension,
  disconnecting, or explicitly cancelling clears that player's sleep state;
  and
- sleep state is ephemeral session state and is never restored after relaunch.

When the quorum is satisfied at an authoritative tick boundary:

1. compute the next morning civil time;
2. assign and persist that civil time once;
3. clear every sleep state and send wake/time updates;
4. execute no loop proportional to skipped ticks or days; and
5. continue ordinary simulation with exactly one executed tick.

Pin the emitted time at an exact morning anchor. Do not leave acceptance
dependent on whether the ordinary `day_time += 1` happens immediately before
or after quorum resolution.

The first functional sleep site must be ordinary shared content rather than a
showcase-only command. If no production bed/bedroll exists when this tactical
begins, add the smallest original-Mclone asset, placement/state, interaction,
collision, persistence, and presentation contract needed for one functional
site. Its exact furniture form and recipe may stay deliberately small. Bed-set
respawn, hostile-nearby rules, villagers, bouncing, explosions, dreams,
comfort progression, and complete Java bed parity are outside this tactical.

Active weather does not yet exist. Do not add a fake weather clear solely
because Java sleep does it. When authoritative weather lands, its sleep policy
requires a separate decision.

### Seasonal resources change opportunity, not counts

For each active resource cell, preserve terrain-derived potential and durable
standing stock, then derive current opportunity from local seasonal climate:

```text
terrain potential
  + persisted standing stock
  + current local seasonal climate
  -> accessibility factor
  + loaded production/recovery rate
  -> physically reachable intake
```

The first seasonal response must remain a small fixed contract over the five
existing strata:

| Resource | First seasonal pressure |
|---|---|
| low herbaceous growth | stronger growth in favorable warm/moist conditions; reduced access under cold/snow tendency |
| woody browse | restrained seasonal change and useful relative winter availability rather than a global shutdown |
| seeds and soft mast | broad late-growth/decline-season abundance that supports squirrel surplus caching without a one-day spawn event |
| aquatic vegetation | warm/wet production response; no invented food where terrain potential is zero |
| aquatic invertebrates | temperature/moisture activity response while retaining real shallow-water site checks |

Use continuous response curves over orbital/local phase and climate. A global
four-season lookup table is insufficient because hemispheres, tropics,
altitude, and moisture differ.

Do not instantly overwrite standing stock when accessibility or recovery
changes. A winter jump may conceal or make some food inaccessible without
deleting its persisted biomass. Production may stop or slow while stock is
above a new effective ceiling; any actual decay must consume executed active
ticks and appear in conservation receipts. A spring jump may expose retained
stock and raise future recovery, but it must not refill prior depletion for
free.

Recovery remains limited to cells overlapping the active domain. Inactive
cells do not produce, decay, reconcile, or catch up. First activation samples
current climate once and continues from saved stock; it does not integrate all
missed seasons.

Extend resource diagnostics to expose at least terrain potential, standing
stock, current accessibility, effective accessible units, production/recovery
factor, recovered units, and per-consumer transfers. Reports must make it
possible to distinguish terrain scarcity, seasonal inaccessibility, ordinary
depletion, and failed physical access.

### Population effects remain individual and bottom-up

Seasonal suitability may influence an animal's priority or eligibility for a
reproductive attempt. It does not create a birth. The first shared direction
is:

- use broad favorable windows rather than one exact annual date;
- allow multiple condition-driven opportunities within a favorable window;
- let recent intake, energy, age, mate access, safety, habitat, crowding, and
  species cooldown remain mandatory;
- express unfavorable periods by reducing desire or opportunity, not by
  resetting a hidden annual appointment;
- keep pregnancy/incubation, maturation, aging, deficit, and cooldown progress
  on executed simulation ticks; and
- never cancel or complete an existing pregnancy merely because the date was
  changed administratively.

Do not add a seasonal carrying capacity, desired squirrel count, winter death
quota, spring birth quota, or population correction. The existing exact
identity remains:

```text
start + births + authorized arrivals - deaths - authorized departures = end
```

For this closed-domain tactical there are no arrivals or departures after the
initial population. Seasonal resource and condition curves explain the
resulting births and deaths.

### Squirrels consume the existing mast stratum

Add squirrel as a typed wildlife lifecycle consumer without creating a
private food ledger. Its first diet prefers `SeedsAndSoftMast` and may use a
small amount of another existing compatible stratum only if real behavior and
terrain evidence justify it. Do not add a `SquirrelFood` resource kind.

The terrain resource vocabulary may need to recognize additional ordinary
mast-producing tree or plant evidence. That remains a refinement of
`SeedsAndSoftMast`; it must not count one tree independently for every species
or invent mast in a site whose blocks and annual climate cannot support it.

Every transfer identifies source and destination:

```text
terrain stock -> carried mast -> cache stock -> squirrel intake
```

Eating directly from a reached forage site may remain
`terrain stock -> squirrel intake`. Reports preserve both paths.

### Squirrels are a woodland-edge animal, not uniform ambience

Extend the coordinate-pure initial-wildlife planner with a squirrel group only
after defining production habitat inputs. Favor a bounded combination of:

- mature woody cover and mast potential;
- woodland interiors, edges, clearings, and connected tree patches;
- tolerable slope and ground access;
- cavity/refuge opportunity;
- avoidance of treeless wetland, bare alpine, desert, and open-water cores;
  and
- sufficient separation from another seed-authored group to avoid ubiquitous
  noise.

Only the seed-selected owner chunk may realize the initial group. Materialized
squirrels become durable immediately. Saved empty entity state prevents seed
resurrection after local extinction. This tactical adds no live refill,
immigration, emigration, or anonymous ambient substitute.

Terrain Lab must display exact squirrel planning inputs, selected groups, and
rejection reasons through the same production planner used by the server.

### Squirrel behavior proves distributed knowledge and arboreal refuge

Use the common needs/knowledge/action model rather than a permanent home
pointer. A squirrel may know a bounded set of:

- recently productive mast sites;
- cache sites and last-known cache revisions;
- compatible tree cavities or refuge trees;
- immediate danger observations; and
- a small current social neighborhood if lifecycle evidence requires it.

Knowledge carries observation time, confidence, source, position or durable
ID, and the revision needed to detect a replaced cache. Unloaded is currently
unavailable, not destroyed. An unavailable remembered cache remains memory;
confirmed removal or incompatible terrain invalidates it.

At minimum, ordinary visible behavior must include:

- ground and low-cover foraging;
- carrying visible or inspectable mast state;
- selecting and physically reaching a compatible cache site;
- a cache-deposit action and later retrieval action;
- rapid escape toward real woody cover;
- one legible climb/refuge or cavity-use sequence rather than disappearing at
  the base of an arbitrary tree;
- rest/idle and alarm reactions; and
- stable movement-derived orientation and authored action animation.

The first arboreal path may be bounded and specialized, but it must use loaded
collision/support facts and fail honestly when no route exists. Do not install
a full unconstrained flying navigator, teleport squirrels into leaves, or
generalize a universal tree-home framework before another species needs it.

### Caches are durable places with bounded cardinality

A cache contains real transferred resource units and therefore needs
authoritative durable state independent of one animal's memory. Prefer a
bounded cache record owned beside the wildlife resource cell or another
focused server-owned ecological record rather than pretending it is an item
entity that stays loaded forever.

Each record needs at least:

```text
SquirrelCache {
    stable_id,
    position,
    revision,
    resource_kind,
    stored_units,
    created_simulation_tick,
    last_active_simulation_tick,
}
```

Only `SeedsAndSoftMast` is accepted initially. The squirrel's knowledge record
references the cache ID/revision but does not own or guarantee its contents.

Hard-bound both memory and world records before implementation. Begin review
with no more than eight remembered caches per squirrel and no more than
sixteen live cache records per 64-by-64 resource cell. Reuse a compatible
existing cache, evict contradicted memory, and refuse or choose another action
when a world-record bound is full. Tune downward if evidence shows fewer are
sufficient; do not raise the bounds without stress and persistence evidence.

A cache site requires loaded compatible substrate and a physically completed
deposit. Gathering removes units from terrain stock into persisted carried
stock. Deposit atomically removes carried units and adds cache units. Retrieval
atomically removes cache units and adds carried stock or immediate intake.
Empty cache records are removed through an idempotent lifecycle operation.

Another squirrel may discover and use a cache through current evidence; an
owner pointer must not make food magically inaccessible. Player digging,
cache loot, seedlings from forgotten nuts, spoilage, theft personality, and
long-term cache decay are valuable later mechanics but are not required here.
Do not age, spoil, or delete caches while their region is inactive.

If a squirrel dies while carrying mast, conservation must place those units in
an explicit remains/drop transfer or record them as a measured consumed/lost
sink. They may not silently duplicate on reload or vanish outside the report.

### Squirrel reproduction uses compressed seasonal opportunity

Give squirrels the common sex, age, condition, lifespan, deficit, mate,
cooldown, parentage, birth, mortality, and remains state. Species tuning begins
as an evidence hypothesis, not a real-world duration claim.

Use a broad local favorable-season response influenced by mast access and
refuge quality. It may raise courtship priority or lower the ordinary attempt
threshold, but a healthy pair still needs recent intake, loaded proximity, a
safe reachable place, and available lifecycle work. Lean conditions suppress
attempts through condition and opportunity.

Do not require one conception per exact year, simulate months of unloaded
gestation, or instantly mature young to fit the calendar. A player should be
able to observe surplus, caching, retrieval, condition, and at least one
meaningful reproductive opportunity within the accepted play-scale window,
while long population evidence still measures multiple generations.

### Figure, animation, sound, and rendering are part of runtime promotion

Author one original first-party squirrel figure through the canonical semantic
figure source. At minimum provide authored roles for:

- idle/look;
- ground locomotion;
- fast escape;
- forage/eat;
- carry;
- cache deposit/dig;
- cache retrieve;
- climb/refuge entry and exit; and
- alarm or social reaction where the sound/behavior contract uses it.

The tail and body silhouette must remain readable at ordinary game distance
without turning the squirrel into an oversized decorative actor. Review
prepared native rendering against the semantic Asset Lab source and add the
figure to the generated animal catalogue.

Add only a small provenance-locked first-party sound family that materially
communicates alarm, movement, or cache work. Do not delay the ecology behind a
large ambient sound campaign, and do not ship silent actions whose timing is
otherwise impossible to read.

The actor, carried state, and any cache trace must use existing render paths
that support mono, per-eye, synthetic stereo, and full-frame multiview. Do not
add a squirrel-only renderer or per-eye mutable animation state.

### Persistence and crash boundaries are explicit

Revise internal unshipped schemas deliberately for:

- world calendar policy metadata if needed;
- player sleep capability/state boundaries, while keeping active sleep
  ephemeral;
- wildlife resource seasonal diagnostics and standing-stock semantics;
- squirrel entity/lifecycle/carried-food state;
- cache records and revisions;
- initial-population planner revision; and
- protocol/entity/catalogue vocabulary.

One resource transfer spanning terrain stock, carried stock, cache stock, or
consumption must have one serialized authoritative owner and be retry-safe.
Do not save one side and defer the other indefinitely. A crash may roll back a
bounded recent atomic batch according to the existing lifecycle policy, but it
must not create resource units.

Save/reload and chunk unload/reload must preserve exact identity and stock.
Unavailable chunks do not cause cache deletion, knowledge contradiction,
animal aging, resource recovery, sleep progression, or calendar catch-up.

### Work remains proportional to active interest

The calendar sample is a constant-time pure query. Seasonal resource work is
performed only at the existing bounded active-cell cadence. Squirrel
observation, cache, refuge, and route queries use bounded spatial neighborhoods
and the shared staggered ecology work scheduler.

Do not:

- enumerate world climate cells at a season boundary;
- visit every saved resource or cache when civil time changes;
- tick or reconcile unloaded squirrels;
- retain one scheduler task per cache indefinitely;
- scan every tree in a chunk for every squirrel each tick;
- add a global cache nearest-neighbor search;
- let calendar cost scale with world age or number of elapsed years; or
- weaken entity, path, persistence, or protocol budgets to make the fixture
  pass.

The existing thousand-animal graceful-overload fixture must include squirrels
and cache/refuge demand before extracting any more general ecology API.

## Implementation Sequence And Review Gates

### Phase A: authoritative calendar foundation

Status: implemented 2026-08-17 by Tactical
[`316`](316-authoritative-season-calendar-foundation.md). The first internal
rule is a revision-1 56-day year beginning at the northward equinox. Metadata
codec 4 persists it, time updates replicate it, clients derive it from current
civil time, and shared Seasonal Debug defaults to read-only World Calendar.
Automated native/Wasm and exact preview-off pixel restoration pass; live pace
review remains open.

1. Add versioned calendar-policy and pure civil-time-to-orbital sampling in
   `mclone-season`.
2. Select it from the shared profile/dimension contract.
3. Persist or durably recover the selected policy with `day_time`.
4. Expose the authoritative sample in `mclone-server` without adding a second
   ticking clock;
5. replicate enough policy/clock state for clients to derive the same sample;
6. add `World Calendar | Manual Preview` to shared Seasonal Debug; and
7. pin native/Wasm sample equivalence, persistence, wrap, and opposite-local-
   season tests.

**Review Gate A:** compare year-length candidates with explicit real-time,
sleep, travel, crop, wildlife, and caching receipts. Human Review binds the
first authoritative year length and phase origin before Phase C tuning.

### Phase B: discontinuity and sleep foundation

Status: implemented 2026-08-17 by Tactical
[`317`](317-civil-time-discontinuity-and-sleep.md). Checked durable civil-time
operations, ordinary persistent sleeping-mat content, ephemeral multiplayer
sleep state, a typed 100-percent realm quorum, exact next-morning publication,
and shared client feedback now pass native, server, persistence, Wasm, and
headed-WebGPU gates.

1. Add typed time-of-day, date, and next-morning mutations.
2. Prove forward and backward mutations preserve simulation history.
3. Add an ordinary functional sleep site and shared interaction command.
4. Add server player sleep state, cancellation, realm quorum, wake, and clock
   publication.
5. Test local, couch/multi-participant, remote, disconnect, damage, dimension,
   invalid-site, and daylight-rule cases.
6. Prove the skipped-night operation executes no missed simulation work.

**Review Gate B:** one ordinary native and browser interaction visibly enters
sleep, crosses dawn, wakes at the exact accepted time, preserves nearby crop
and animal tick counts, and restores after save/reopen. Do not begin seasonal
resource tuning if sleep can duplicate or catch up world work.

Accepted 2026-08-17. The continuous browser run supplies the ordinary
place/use/wait/exact-dawn receipt; native separately proves ordinary shared
input and the transient presentation. Shared server tests prove exact
one-tick execution, unchanged scheduled/crop/resource/wildlife sentinels,
multiplayer cancellation, and exact save/reopen. The detailed receipt and the
reason native pixel capture stages the already-tested transient update are in
Tactical 317.

### Phase C: seasonal resource opportunity

1. Add pure local seasonal response for the five current resource strata.
2. Preserve static terrain potential and standing stock separately from
   accessibility and loaded production factors.
3. Thread the sample only through active-cell recovery and reached intake.
4. Extend saved records, snapshots, population reports, and invariants.
5. Run north/south/tropical, elevation, wet/dry, forward-jump, backward-jump,
   unload/reload, and full/accelerated equivalence fixtures.
6. Re-run the accepted rabbit/deer/mallard campaign windows before adding
   squirrel demand.

**Review Gate C:** resource curves must visibly differ for causal climate and
terrain reasons while exact conservation, inactive freeze, and current
three-species behavior remain intact. No population-target tuning is allowed.

### Phase D: squirrel asset, habitat, and initial population

1. Author and validate the semantic figure, animation roles, prepared render,
   catalogue entry, and bounded sound family.
2. Add shared protocol/entity/lifecycle/render vocabulary.
3. Define woodland-edge habitat fitness and mast/cavity evidence.
4. Extend coordinate-pure initial-population planning and Terrain Lab.
5. Prove exact plane/cylinder planning, owner-chunk realization, empty-record
   extinction, persistence, and order independence.
6. Add ordinary ground forage, cover escape, and one honest tree-refuge path.

**Review Gate D:** inspect Asset Lab, native game, headed WebGPU, synthetic
stereo, and at least one physical XR or exact multiview path before adding
cache complexity. The squirrel must read as an animal using woodland structure,
not a small rabbit with a different model.

### Phase E: cache transfers and seasonal lifecycle

1. Add persisted carried mast and bounded cache records.
2. Add knowledge, selection, gather, carry, deposit, retrieve, consume,
   invalidation, and empty-cache removal.
3. Prove exact resource conservation and crash-safe/idempotent transfers.
4. Add broad seasonal reproductive opportunity through the shared lifecycle.
5. Extend short fixtures, long closed-domain campaigns, reports, and the
   thousand-animal overload case.
6. Compare cache-enabled and cache-disabled diagnostic controls without making
   either a target population.

**Review Gate E:** an ordinary bounded-time window must show at least one
uncommanded gather/deposit/retrieve sequence and a meaningful lean-period
outcome such as cache-supported feeding or condition. First-frame pixels are
not behavioral acceptance.

### Phase F: cross-platform product acceptance

1. Create a bounded playable showcase only if it can declare compatible live
   instantiation evidence for every calendar, sleep, squirrel, cache, and
   habitat fact.
2. Run native and headed WebGPU behavioral windows sequentially on one GPU
   host and inspect their captures.
3. Prove SQLite and IndexedDB save/reopen plus transient-web no-persistence
   rules where applicable.
4. Build flat Android and Android XR through the repository scripts.
5. Run synthetic stereo and physical XR interaction/performance acceptance for
   the world-visible actor and sleep UI.
6. Re-run exact full/accelerated population equivalence at the accepted
   calendar/resource revision.

**Review Gate F:** Human Review accepts calendar pace, dawn discontinuity,
seasonal resource legibility, squirrel scale/personality, caching behavior,
and native/Web/XR presentation before the tactical is marked complete.

## Validation Matrix

### Calendar and discontinuity

- exact day zero, morning, noon, night, day wrap, year wrap, and maximum-value
  samples;
- north/south opposite local seasons and weak tropical response at one global
  date;
- native/Wasm bit- or tolerance-exact shared samples;
- persistence/relaunch with identical civil time, policy revision, year/day,
  and orbital phase;
- time-of-day mutation preserving the absolute date;
- date mutation preserving world state;
- backward date movement with no rewind or duplicate event;
- next-morning jump with one simulation tick and the exact bound civil jump;
  and
- non-seasonal profile exact-restoration controls.

### Sleep and multiplayer

- one-player success at night and rejection by day;
- two-player 100-percent quorum, one sleeper waiting, second sleeper
  completing, and both waking;
- local couch participants counted as ordinary realm players;
- remote player, disconnect, cancellation, movement, damage, death, invalid
  site, unload, and dimension-change cancellation;
- observer/preview exclusion;
- exact clock update ordering and no stale client interpolation across jump;
- no skipped crop growth, scheduled block/fluid tick, wildlife age, hunger,
  pregnancy, cache work, or resource recovery; and
- save/reopen after the jump with no persisted sleeping state.

### Resources and populations

- each stratum at favorable, neutral, and unfavorable local samples;
- zero terrain potential remaining exactly zero in every season;
- equal northern/southern sites producing phase-inverted response;
- stock conservation under accessibility changes and forward/backward jumps;
- active recovery and inactive freeze;
- rabbit/deer/mallard regression campaigns at the new resource revision;
- exact full-server/accelerated equivalence;
- no population floor, refill, cap, ratio correction, or acceptance target;
  and
- raw potential, stock, accessibility, recovery, intake, condition, birth, and
  death histories sufficient to explain outcomes.

### Squirrel and cache lifecycle

- coordinate-pure group selection and owner-chunk realization;
- exact identity after unload/reload and relaunch;
- saved-empty extinction without seed resurrection;
- mast-compatible and mast-incompatible habitat controls;
- bounded refuge/cache knowledge and graceful work admission;
- loaded/unloaded/invalid cache availability outcomes;
- ground-to-carried-to-cache-to-intake conservation;
- interruption before and after each transfer commit;
- death while carrying and cache persistence after creator death;
- another squirrel discovering and consuming a cache;
- cache bound saturation without overwrite or unbounded search;
- terrain replacement invalidating a cache exactly once;
- favorable-window reproduction plus lean-condition suppression;
- long population reports with exact identity conservation; and
- native/Web/stereo/multiview animation and carried-state consistency.

### Pixel and behavioral evidence

Store all captures and receipts under `/tmp`, never in the repository.
Inspect pixels at the first drawable squirrel and sleep milestone and after
each meaningful presentation addition.

Required final evidence includes:

- Asset Lab clean sheets and animation review for every new clip;
- animal-catalogue desktop and narrow/mobile inspection;
- native flat calendar/sleep/squirrel/cache captures;
- headed WebGPU flat captures and a bounded behavioral receipt;
- synthetic stereo with distinct per-eye view/projection and shared animation
  state;
- full-frame multiview or physical XR evidence for every visible feature;
- Android and Android XR script-driven builds; and
- one time-window receipt proving uncommanded caching and retrieval rather
  than an authored first-frame pose.

## Shared Ownership

- `mclone-season`: pure calendar policy/sample, orbital projection, local
  seasonal climate, and resource-response vocabulary.
- `mclone-core`: only genuinely universal clock primitives such as day length;
  no ecology policy.
- `mclone-server`: persisted civil clock, time mutation, sleep/quorum,
  authoritative local sampling, resource ledger, squirrel lifecycle,
  knowledge, caches, population planning, and persistence.
- `mclone-protocol`: neutral sleep commands/updates, calendar policy facts if
  required by clients, squirrel/cache presentation state, and diagnostics.
- `mclone-client`: replicated clock/player/entity state only; no authority or
  population policy.
- `mclone-assets`: canonical squirrel/bed content loading and provenance.
- `mclone-ui`: World Calendar display, sleep feedback, and Debug source
  selection.
- `mclone-render-session` / `mclone-render`: existing shared actor, prop, and
  surface paths across mono, stereo, and multiview.
- `mclone-scene`: frame-local assembly of replicated calendar, UI, and actor
  presentation without gameplay decisions.
- app/platform crates: raw interaction collection, cadence, surfaces,
  OpenXR/browser/native lifecycle, and presentation only.

Do not add season, sleep, squirrel, resource, cache, or breeding policy to
`mclone-native-client`, `mclone-web-client`, Android apps, or XR hosts.

## Explicit Deferrals

- migration waves, receipts, arrivals, departures, and anonymous seasonal
  wildlife reconciliation;
- unloaded population summaries or elapsed-time catch-up;
- hibernation, disease, literal annual gestation, or one-birth-per-year rules;
- predators, squirrel predation, fox adoption, and food-web balancing beyond
  the new consumer's measured pressure;
- discrete annual mast events that require boundary receipts;
- cache spoilage during inactivity, forgotten-cache germination, player cache
  digging, or a general buried-item system;
- arbitrary canopy navigation, universal animal homes, and a public generic
  ecology fact database;
- supplied feeders, zoo management, tagging, domestication, and sanctuary
  identity;
- bed respawn assignment, comfort systems, hostile-nearby parity, villagers,
  dreams, and full Java bed behavior;
- active weather clearing, seasonal hydrology, interactive snow, and
  light-engine/skylight gameplay changes;
- procedural-horizon seasonal appearance, which retains its separate LOD
  ownership; and
- new behavior for retained Java-shaped generation profiles.

## Completion Checklist

- [x] Bind a reviewed authoritative year length and phase origin.
- [x] Persist/recover one versioned calendar policy and derive orbital phase
      from civil `day_time` without a second ticking clock.
- [x] Show `World Calendar` and observer-local season through shared UI.
- [x] Land typed time-of-day, date, and next-morning authority operations.
- [x] Land ordinary functional sleep content, player state, cancellation,
      realm quorum, wake, and cross-client clock publication.
- [x] Prove sleep and date jumps execute no skipped simulation work.
- [ ] Add continuous local seasonal accessibility/recovery for all five
      resource strata without direct stock replacement.
- [ ] Preserve inactive resource and animal freeze plus exact population
      conservation.
- [ ] Re-accept rabbit/deer/mallard resource and population evidence.
- [ ] Author and catalogue the first-party squirrel figure, clips, and bounded
      sound family.
- [ ] Add squirrel habitat planning, durable realization, diet, lifecycle,
      tree-refuge behavior, and shared rendering.
- [ ] Add bounded durable caches, individual knowledge, carried mast, and
      atomic gather/deposit/retrieve/consume transfers.
- [ ] Prove cache conservation, persistence, invalidation, saturation, and
      overload behavior.
- [ ] Prove broad seasonal reproductive opportunity without annual waiting or
      population targeting.
- [ ] Complete exact native/Web/SQLite/IndexedDB/full-accelerated tests.
- [ ] Inspect native, headed WebGPU, stereo, Android, and XR evidence.
- [ ] Obtain staged Human Review for calendar pace, sleep discontinuity,
      seasonal food response, squirrel presentation, and caching behavior.
- [ ] Update all three living topic docs with final contracts, evidence,
      revisions, open gaps, and the next recommended species relationship.

## Code And Documentation Map

- `native/crates/mclone-core/src/time.rs`: shared day-length and ordinary
  time-of-day primitives.
- `native/crates/mclone-season/src/lib.rs`: pure orbital, latitude,
  local-season, calendar, and resource-response vocabulary.
- `native/crates/mclone-server/src/integrated.rs`: realm clocks, time updates,
  player commands, active tick orchestration, and persistence integration.
- `native/crates/mclone-server/src/players.rs` and `player.rs`: server player
  and future ephemeral sleep state boundary.
- `native/crates/mclone-server/src/wildlife_resources.rs`: typed stock,
  accessibility, recovery, diet, cache, and conservation accounting.
- `native/crates/mclone-server/src/wildlife_simulation.rs`: closed-domain
  reports and equivalence evidence.
- `native/crates/mclone-server/src/entity/`: squirrel behavior, lifecycle,
  knowledge, persistence, and actor state.
- `native/crates/mclone-worldgen/src/levelgen/mclone_overworld/wildlife.rs`:
  coordinate-pure initial squirrel geography.
- `native/crates/mclone-protocol/src/`: shared clock, sleep, player, squirrel,
  and cache presentation vocabulary.
- `native/crates/mclone-ui/src/`: World Calendar, Manual Preview, and sleep
  feedback.
- `assets/mclone/figures/`: canonical squirrel figure and animation source.
- [`../topics/seasons.md`](../topics/seasons.md): calendar, discontinuity,
  sleep, and seasonal-climate direction.
- [`../topics/habitat-driven-creature-ecology.md`](../topics/habitat-driven-creature-ecology.md):
  terrain/resource/creature loop and runtime promotion status.
- [`../topics/wildlife-ecology-state-model.md`](../topics/wildlife-ecology-state-model.md):
  knowledge, cache, lifecycle, work, and persistence architecture.
- [`../topics/animal-catalogue.md`](../topics/animal-catalogue.md): generated
  figure review and production catalogue evidence.
- [`../topics/playable-showcases.md`](../topics/playable-showcases.md): required
  live-instantiation and behavioral acceptance contract if a showcase is used.

## Final Report

Do not fill this section until implementation begins. Record:

- the accepted year length, phase origin, sleep window, morning anchor, quorum,
  and rejected timing candidates;
- exact calendar, discontinuity, persistence, and no-catch-up evidence;
- resource response curves and rabbit/deer/mallard before/after histories;
- squirrel habitat, asset, cache, lifecycle, conservation, overload, and
  population receipts;
- every schema/protocol/rule revision;
- inspected native, browser, Android, stereo, multiview, and physical-device
  artifacts;
- every Human Review decision and rejected correction; and
- remaining explicit gaps without claiming migration, unloaded ecology, or a
  stable ecosystem.
