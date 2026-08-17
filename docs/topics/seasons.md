# Seasons And Seasonal Ecology

Topic: `seasons`

Status: active implementation record updated 2026-08-17. Tactical
[`316`](../tactical/316-authoritative-season-calendar-foundation.md) now gives
`mclone-overworld-v1` an authoritative revision-1 56-day calendar derived from
persisted cumulative civil `day_time`. Metadata codec 4 preserves the exact
policy, time updates replicate it, clients derive the same fixed-point phase,
and shared Seasonal Debug defaults to read-only `World Calendar` with year,
day, milestone, and observer-local season. Retained profiles report the
calendar as unavailable. The former 112-day date remains an explicit unsaved
`Manual Preview`; both visual consumers still default off. Live review of the
56-day pace remains open, but later changes require an explicit migration or
disposable-world regeneration decision.

Tactical
[`307`](../tactical/307-seasonal-solar-path-and-cyclical-latitude.md) is
implemented as a client-local solar proof. The default unbounded Mclone plane
has a provisional 98,304-block cyclical latitude wavelength, a 27-degree-tilt
pure solar model, one visible-sun/sky/rendered-light sample, and an accepted
Earth-like `0.53`-degree original-Mclone sun. Retained Minecraft-reference
profiles keep the Java 1.17.1 oversized sun; the optional cylinder stays
secondary.

Tactical [`306`](../tactical/306-seasonal-appearance-preview.md) is now
implemented through automated evidence. One continuous global preview date
drives observer-local exact-terrain, deciduous/evergreen, grass, seasonal-snow,
and bounded recent-snow appearance with zero mesh-payload growth or seasonal
remesh. The shared `Seasonal Debug` screen shows global date and evaluated
local season beside the solar, latitude, and snow controls. Its 22-case native
matrix, synthetic stereo capture, headed WebGPU off/active/exact-restoration
probe, and Android/Android-XR builds pass. The first palette review found
autumn grass too green and snow breakup too finely speckled. Follow-up revision
`741d5a74` gives mild/dry grass a stronger straw tint and dieback while wet or
warm regions retain more growth, and replaces half-block snow dithering with a
topology-aware broad field plus a smaller edge field. The refreshed native
matrix and headed WebGPU exact-restoration probe pass; final Human Review of
the adjusted palette and live menu remains open. No authorized Quest is
currently visible, so physical seasonal Debug interaction and
current-revision Quest performance also remain open. Procedural-horizon LOD
is still explicitly deferred.

A post-implementation control split now makes `Solar Preview` and `Ground
Appearance` independent. Date and latitude are shared climate inputs, but the
solar toggle alone admits the seasonal sun/sky/rendered-daylight sample and
the ground toggle alone admits exact-terrain tint, vegetation dormancy, and
derived snow. Both default off. This permits solar/date/latitude review with
an exactly neutral seasonal ground, or material review while retaining the
ordinary world-clock sky. The original `--season-preview` capture switch stays
a combined compatibility control; `--season-appearance` can override its
ground half explicitly.

Tactical
[`314`](../tactical/314-celestial-moon-stars-and-square-sun.md) is implemented
through automated and physical-XR evidence. Original Mclone now has the
accepted Earth-sized square solar core, a separate halo, a continuous square
phased moon, and a stable 2,048-entry latitude-aware star catalog. Retained
Java profiles preserve their moon atlas and seed-10842 star law. The shared
`Celestial Debug` screen independently disables every optional layer and
reports its exact submission/resource cost; all-off restores the deterministic
sky hash exactly. Quest held 72 Hz with no dropped frames, but Full stars cost
about `0.26` to `0.29 ms` app GPU, above Tactical 314's `0.20 ms` review
trigger. Quarter density cost essentially the same, so full quality remains
enabled and final Human Review must accept that fixed blended-draw exception
or request a later sky-compositing slice. None of this creates a gameplay
calendar or mutates the authoritative light engine.

Planned coordinating parent Tactical
[`315`](../tactical/315-authoritative-seasonal-calendar-and-squirrel-ecology.md)
owns the first authoritative gameplay-calendar and seasonal-ecology sequence.
It keeps monotonic executed `game_time` separate from mutable persisted civil
`day_time`, derives one versioned orbital calendar from the latter, treats
sleep/date changes as no-catch-up discontinuities, and lets local season affect
loaded food accessibility and recovery before promoting squirrels through
mast, bounded caches, tree refuge, and the ordinary durable lifecycle. It does
not make calendar transitions directly add or remove food or animals.

No authoritative season clock, terrain or biome season generation, active
weather event, migration producer, seasonal hydrology, wildlife tagging, or
managed-habitat infrastructure is implemented.
The selected direction is that unloaded terrain and entities receive no ticks
or elapsed-time catch-up. Climate and ambient seasonal opportunities may be
derived when an active region asks for them, while durable animals freeze.
The proposed distinction between replaceable anonymous wildlife and
tagged/managed animals conflicts with the current rule that every materialized
Mclone animal becomes durable immediately; that lifecycle change requires an
explicit later decision and coordinated revision of the existing ecology and
persistence contracts.

## Scope

This topic records the product and technical direction for seasons in the
original Mclone world. It covers:

- regional rather than globally uniform seasonal climate;
- latitude on the unbounded plane and X-periodic cylinder;
- snow cover, foliage, daylight, and seasonal water presentation;
- migration as active-world seasonal encounters rather than unloaded-world
  motion;
- hibernation and animal aging across inactive regions;
- tagged wildlife, livestock, zoos, and supplied food; and
- the macro-landscape scale needed to make seasonal movement legible.

It does not allocate every final crate/API, promise a complete ecosystem, or
make seasons part of the legacy Java-shaped profiles.
Vanilla 1.17.1 has no season system; any Mclone implementation is an explicit
original-product feature, not weather-parity work.

## Product Direction

A season should change the interpretation and activity of the world rather
than rewrite every affected block. The useful composition is:

```text
calendar phase
  + dimension climate-coordinate policy
  + effective latitude
  + macro temperature and moisture
  + altitude and terrain exposure
  + local habitat and active weather
  -> local seasonal climate
  -> appearance, resources, water, and active ecology
```

Regions must not all receive the same four-color global transition.

- Temperate regions may have recognizable spring, summer, autumn, and winter.
- Tropical regions may express wet and dry seasons with little temperature
  change.
- High elevations may accumulate snow at otherwise mild latitudes.
- Deserts may change little except for rainfall-driven blooms or water.
- Polar regions may retain snow and ice through a brief summer thaw.
- Signed northern and southern latitudes may be half a year out of phase.

Seasonal effects should remain understandable through landscape and animal
behavior. A shifting snow line, wet creek bed, spring arrival of ducks, autumn
departure call, or summer polar bloom is more valuable than a hidden numerical
season meter.

### Global calendar and evaluated local season

The calendar is global; the season name is local. One orbital date must not be
called Spring or Winter everywhere because opposite hemispheres disagree and
tropical regions may have weak thermal seasons or later wet/dry vocabulary.
The shared evaluation shape is:

```text
global calendar/orbital phase
  + effective latitude
  + local climate and altitude
  -> local seasonal phase, response strength, snow tendency, and display label
```

Sun path and material appearance are sibling outputs of those inputs. Do not
derive season from instantaneous sun elevation, and do not expose an
independent local-phase slider that can disagree with the sky.

Tactical 306's 112-day, four-by-28-day synthetic calendar remains only the
readable Manual Preview projection over continuous `OrbitalPhase`. Tactical
316's authoritative calendar is instead a 56-day year derived from civil time,
with day zero at the northward equinox. The dedicated Seasonal Debug screen
now selects `World Calendar | Manual Preview`: world mode shows its read-only
year/day and evaluated local result, while manual mode retains the scrubber.
A compact player-facing date plus local-season display remains a separate
slice.

### Debug time and simulation isolation

The current simulation is tick-driven but is not independent of time of day.
The authoritative server advances `game_time` and `day_time` from simulation
ticks, replicates them to clients, and may let gameplay schedules such as
animal activity read that value. Seasonal Debug does not mutate either clock.

`World Clock` is a one-way presentation input: the client reads replicated
`day_time` to place the seasonal sun. `Manual` date, latitude, and solar time
remain unsaved fields in client-local `SeasonPreviewSettings`; changing them
emits only a client-experience render-setting effect. It sends no server
command and performs no block, entity, crop, weather, scheduled-tick,
persistence, or light-engine update. The seasonal daylight factor changes the
rendered sky and material illumination only, so a manual preview may
intentionally disagree with authoritative mob schedules or propagated
skylight. A future gameplay calendar or true light-mechanics integration must
be an explicit server-owned feature rather than an extension of this Debug
override.

## Climate Coordinates And Topology

World topology and seasonal climate remain separate dimension facts. The
current Mclone cylinder is exact X-periodic geometry with unbounded Z. That
topology supplies a meaningful wrapped east/west direction, but it does not by
itself create latitude, poles, axial daylight, or seasons. A profile/dimension
must choose those climate semantics explicitly.

### Unbounded plane

The selected first plane policy uses a large, smooth cyclical effective
latitude:

```text
latitude_phase = (z - phase_origin_z) / climate_wavelength
latitude = sin(TAU * latitude_phase)
```

Travel along the latitude axis then passes through repeating equatorial,
temperate, northern-polar, temperate, equatorial, southern-polar, and
temperate belts. Travel roughly orthogonal to it remains within one climate
family while terrain, continentality, moisture, and regional ecology continue
to vary. Only the broad climate envelope repeats; landforms, rivers, habitats,
and content must not repeat with it.

The first solar proof deliberately omits coordinate warp so the navigational
rule and solar response remain legible. A later annual-climate/worldgen study
may propose a broad shared warp, but it must not erase the rule that one
direction mostly follows climate while the other crosses it. Tactical 307
provisionally selects a 98,304-block wavelength and Z origin zero, putting each
equator-to-pole interval at 24,576 blocks. Its map review rejected 49,152
blocks as too frequent and 196,608 blocks as too diffuse. Human Review may
still reject the provisional values after live travel, spawn, and headset
inspection; they are not a shipped compatibility promise.

This policy makes polar regions long belts rather than literal points. That is
acceptable only if the product describes climate regions honestly instead of
claiming spherical geography.

The solar law treats latitude as a scalar climate coordinate. Equal latitude
on opposite sides of a polar crest has equal sun path at the same global time;
crossing the crest does not rotate the compass frame or add a twelve-hour
longitude shift. Globe-like pole crossings and spatial time zones remain a
separate cosmology decision.

### X-periodic cylinder

The preferred cylinder exploration is a single broad habitable belt with
conditions becoming increasingly polar in unbounded positive and negative Z.
A bounded asymptotic coordinate is a useful starting hypothesis:

```text
latitude = tanh(z / climate_scale)
```

This avoids temperatures that decrease without limit. Far enough north or
south, the climate approaches a polar plateau. East/west travel circles the
cylinder in approximately the same climate; north/south travel becomes a
long, increasingly difficult expedition. The two signs may represent opposite
seasonal hemispheres.

The polar plateau should be seasonally quiet rather than permanently dead by
default. At extreme northern latitude, winter may approach polar night and
deep cold while summer brings very long daylight, partial thaw, migration,
flowers, insects, and brief water flow. The southern extreme experiences the
opposite phase. A permanently dark end requires a separate cosmology or world
boundary decision; ordinary axial seasons do not imply darkness all year.

An infinite, fully saturated polar tail eventually offers no new climatic
information. Human review must decide whether that quiet wilderness is a
desirable soft boundary, whether the axial extent should become finite, or
whether another climate mapping is better. Do not silently convert the exact
cylinder into a torus to solve this. Torus climate and lighting semantics
remain a separate later question.

### Seasonal forcing

A simple signed forcing can establish opposite hemispheres without a global
biome swap. With orbital phase zero at the northward equinox:

```text
solar_declination = axial_tilt * sin(TAU * orbital_phase)
seasonal_temperature_delta = latitude * sin(TAU * orbital_phase) * local_amplitude
```

Mean temperature still depends on macro climate and altitude. Seasonal
amplitude may increase with absolute latitude, while tropical moisture may
use a separate phase and response. Day length, precipitation tendency,
snowmelt, and biological cues should consume a shared local seasonal sample
rather than each inventing a calendar interpretation.

### Annual climate and world generation

Latitude and terrain generation should be independent layers with shared
climate meaning, not unrelated systems. Worldgen should eventually consume an
annual climate normal rather than the current orbital instant:

```text
annual climate normal =
    existing regional temperature and moisture
    + latitude mean-temperature bias
    + altitude cooling
    + continental and coastal moderation

current climate =
    annual climate normal
    + orbital seasonal forcing
    + active weather
```

Durable biome vocabulary, tree species, vegetation density, soils, perennial
snow/ice, drainage form, and habitat character may use the annual normal.
Reversible foliage, ordinary snow/frost, blooms, forage, temporary ice, and
seasonal activity belong to active seasonal response. Latitude should first
bias rather than replace existing regional noise, while continents, mountains,
and valleys remain mostly independent until map review supports stronger
coupling. Tactical 307 now defines and provisionally calibrates the shared
latitude policy but does not change generation.

## No Unloaded-World Simulation

World extent must not determine seasonal runtime cost. No system may enumerate
unbounded climate cells, tick unloaded entities, move migration cohorts through
inactive terrain, replay missed weather, or integrate hidden food, predation,
birth, death, aging, or hydrology.

The intended cost is proportional to active interest:

```text
season state: cheap pure query for requested positions/regions
surface response: visible or active chunks only
animals: ordinary entity tickets only
migration: events considered only where active interest intersects eligibility
managed food and reproduction: loaded simulation time only
```

Activation-time reconciliation is not unloaded simulation. A chunk may compare
its saved seasonal revision or last-active calendar with the current season
when loaded, choose current derived presentation, and decide which explicitly
ephemeral encounter opportunities are eligible. It must not replay every
missed day.

Durable entities freeze while unloaded. Aging, hunger, pregnancy, breeding,
disease, hibernation preparation, and resource consumption resume from stored
semantic state rather than advancing from wall-clock or world-calendar
elapsed time. This extends the existing no-offline-catch-up contract.

## Seasonal Surface And Snow

Do not represent the ordinary visual season by converting every base block to
a seasonal block variant. Broad block mutation would create persistence churn,
lighting work, fluid reactions, mesh invalidation, and expensive catch-up.

Prefer a derived surface layer:

- a compact per-column or coarse surface mask describes snow, frost, wetness,
  leaf litter, or related seasonal coverage;
- the base terrain mesh and saved block identity remain stable;
- a separate lightweight surface mesh, instance path, or material input owns
  presentation and can update independently;
- visible/active chunks approach the current climate target under bounded
  upload and work budgets; and
- distant terrain consumes the same seasonal climate semantics at its own
  representation scale.

A 16-by-16 one-byte surface mask would be only 256 bytes per chunk before
metadata, but the actual representation must be selected through renderer and
XR measurement. World-space seasonal presentation must support mono, per-eye,
and full-frame multiview paths from its first accepted renderer.

Permanent polar snow and stable tropical ground require little recurring
change. The moving temperate/subpolar frontier carries most of the seasonal
visual work. Foliage tint, dormant grass, blooms, falling leaves, ambience, and
surface wetness can follow the same climate sample without sharing one
all-purpose renderer.

Tactical 306 now proves a smaller representation before selecting that broader
surface layer. Exact meshes carry only compact static material and exposure
response; a continuous phase plus one bounded, topology-aware recent-snowfall
pulse is evaluated during ordinary rendering. The pulse can raise
texture-preserving coverage on eligible ground and exposed canopy locally,
then return to zero without remeshing, per-chunk masks, persistence, or inactive
work. A later active-world weather producer may drive its rise/hold/decay, but
the first proof controls intensity manually and makes no precipitation or
automatic-timing claim.

Interactive snow remains a later, separate authoritative layer. Shovelable
depth, collision, tracks, snowballs, buried plants, and meltwater may justify
sparse saved gameplay state. Cosmetic coverage must not pretend to be a block
that gameplay can collect, and sparse material snow must not force every
seasonal pixel into block persistence.

## Seasonal Water

Seasonal springs and creeks should consume persistent drainage and channel
facts rather than ask arbitrary fluid simulation to discover a watershed.
The terrain may always contain a creek bed while a shared seasonal query gives
its planned channel a current discharge class:

- dry bed;
- damp or pooled;
- shallow flowing water; or
- exceptional high water.

Snowmelt, wet seasons, and dry seasons can change discharge when the relevant
region is active. The channel/reach identity provides agreement across chunk
boundaries. Any real interactive fluid remains constrained by ordinary loaded
world authority; no upstream unloaded chunk is simulated to justify it.

This direction depends on the macro-planning and hydrology contracts. A local
season system must not invent disconnected water at each chunk boundary or
overwrite player changes simply because the calendar advanced.

## Migration As Seasonal Encounter

Migration does not require an offscreen population simulation. A migration
wave is a deterministic seasonal encounter opportunity, conceptually the
point where a coarse world fact enters active high-fidelity simulation:

```text
world seed + year + species + climate front + habitat/corridor + event slot
  -> no event, passage wave, or seasonal arrival
```

When eligible active interest intersects the event, animals or ambient
presentation may enter through an outer simulation/presentation ring with a
coherent heading. They behave normally while active. Once outside every
player's interest, their later lifecycle follows the explicitly selected
ambient-versus-durable policy; they are not advanced elsewhere.

A compact deterministic event identity or local receipt must prevent one
player, repeated chunk-boundary crossing, save/reload, or overlapping
multiplayer interest from duplicating the same passage indefinitely. Receipts
must be bounded by visited state and retention policy; an unbounded global
calendar job is forbidden.

Useful encounter classes are:

- **passage:** birds, butterflies, whales, or herds visibly travel through;
- **seasonal arrival:** waterfowl occupy wetlands, turtles use beaches, or
  salmon enter rivers for a bounded part of the year; and
- **resource-following movement:** grazing herds follow rainfall, forage, or
  snow retreat, while mountain animals move between summer and winter ranges.

Waterfowl are the strongest first visual pressure test: V-shaped flocks can
cross any terrain, calls can announce arrival, and actual landing requires
compatible lakes or wetlands. Altitudinal deer movement can test a smaller
terrestrial route without enormous continents. Salmon can later test whether
hydrology exposes connected mouths, reaches, and spawning habitat. Butterflies
or distant high flocks can begin as explicitly ambient presentation, while
interactive animals remain server-authoritative.

Migration direction should follow climate and habitat, not the player's view
vector. Materialization must occur far enough outside ordinary sight to read
as arrival rather than random spawning.

## Hibernation And Seasonal Reactivation

Hibernation should be a behavior conditioned on the current environment, not
a one-time autumn appointment that permanently breaks when the region was
inactive.

- An active animal may forage, select a den, prepare, and hibernate through
  ordinary behavior.
- An inactive durable animal performs none of those steps.
- On winter activation, a durable animal restores exactly and receives a
  safe, visible transition or grace period to seek shelter or enter its winter
  behavior.
- Missing preparation never causes retroactive unseen starvation or death.
- Anonymous seasonal wildlife may instead be reconciled to the currently
  eligible assemblage if the future lifecycle decision permits it.

No hidden simulation is needed to explain why a wild bear is absent in winter
or why a tagged zoo bear is still present. The former may be part of current
seasonal wildlife; the latter is a frozen persistent individual responding to
the newly active world.

## Wildlife Identity Decision To Resolve

The seasons discussion selected a promising distinction:

```text
anonymous wild population: reconstructed from current habitat and season
player-significant identity: persisted exactly and frozen while inactive
```

That is not the current implementation contract. Today a normal materialized
Mclone creature becomes durable immediately, a saved empty entity record
prevents seed-population refill, and killed animals do not reappear merely
because habitat remains suitable. That rule protects uncommon sightings,
local depletion, exact identity, and understandable Minecraft-shaped saves.

Seasonal wildlife creates real pressure to revisit it. Otherwise an unloaded
summer migratory duck may remain frozen until a winter visit, while a fully
ephemeral animal can be rerolled merely by crossing a load boundary. A future
tactical must choose and test one explicit lifecycle rather than mixing them.
A candidate model is:

1. ambient calls, distant flocks, and insect swarms have no gameplay identity;
2. anonymous gameplay wildlife restores across short/same-season unloads but
   may be reconciled after a material seasonal change;
3. tagging, taming, breeding, husbandry, or another explicit player investment
   promotes an animal to durable managed identity; and
4. livestock and managed animals always restore exactly and never advance
   while unloaded.

The second step is intentionally unresolved. It requires rules for deaths,
drops, nests, offspring, capture, transport, event receipts, multiplayer,
local depletion, and crash-safe persistence. Until that work lands, the
existing immediate-durability rule remains authoritative.

## Tagging, Sanctuaries, And Zoos

Tagging is the preferred explicit expression of player intent. A tag means:

> This animal has player-significant identity. Preserve it across seasonal
> wildlife reconciliation and unloading.

It does not imply immortality, free food, domestication, reproductive success,
or suppression of seasonal behavior. A tagged goose may still become restless
during migration season; a tagged bear may still seek winter shelter.

Automatic fence-containment inference is not a good first contract. Enclosures
can be irregular, open temporarily, span chunks, contain mixed species, or use
terrain rather than fences. Individual tags are legible and exact. A later
sanctuary/zoo marker may register nearby tagged animals, report conditions, or
reduce management tedium, but it should not manufacture persistence for every
animal that briefly walks through a radius.

Two player creations should remain distinct:

- A **wildlife sanctuary** persists through its terrain: water, shore, reeds,
  nesting features, cover, and food make seasonal ducks likely to return even
  when no individual wild duck is guaranteed.
- A **managed zoo** persists particular tagged animals and supports an active
  captive breeding population.

The first preserves a colony as an ecological relationship. The second
preserves individuals because the player explicitly invested in them.

### Supplied food and stable captive populations

A zoo footprint often cannot naturally produce enough forage for its animals.
Feeders, troughs, nests, dens, roosts, and shelter can let player logistics
supplement rather than replace habitat.

- Feeders consume appropriate stored seed, fruit, hay/browse, fish, meat, or
  other concrete inventory while active.
- Animals physically approach and consume supplied food; a remote ledger does
  not authorize feeding through unloaded terrain.
- Water, substrate, cover, space, social grouping, nesting, denning, and
  climate may remain independent needs that calories cannot erase.
- No food is consumed and no animal starves while the zoo is inactive.
- Breeding, maturation, aging, and death occur only through loaded simulation.

Imported feed raises the amount of animal demand a small site can support, but
must not become a hidden spawn quota or guaranteed carrying-capacity target.
The existing ecology direction keeps population support emergent from actual
resource access, condition, reproduction, and mortality. A management report
may estimate shortage or support; it must not create births or silently delete
surplus animals to hit a number.

A stable captive population should therefore emerge from compatible breeding
groups, suitable habitat, supplied and natural resources, spare space, and
active-time life cycles. Importing a missing breeder remains meaningful. A zoo
is a valuable stress test for persistence, dense-animal performance,
pathfinding, social requirements, seasonal behavior, feeding, breeding, and
predator/prey separation without requiring any unloaded work.

## Continental Scale And Migration Geography

The design discussion raises a hypothesis that current continents may be too
small for the intended grand seasonal geography. Do not treat that impression
as a measured worldgen finding. Review it through macro maps, journeys, climate
overlays, and travel-time calibration before changing production fields.

A useful scale ordering is:

```text
habitat patch
  << local animal range
  << seasonal migration route
  << major continent or connected habitat system
  << equator-to-pole climate distance
```

Large migrations need some landmasses or connected coasts to span multiple
climate regions, watersheds, and seasonal destinations. Small islands can
remain ecologically valuable, but they cannot be the only land vocabulary.
Larger geography should come from nested continental, regional, meso, and
local structure rather than merely stretching one noise frequency into bland
land blobs.

On the cylinder, the 6,144-block wrapped X circumference and unbounded Z create
different route opportunities and constraints. North/south climate movement
can be long even when east/west returns to its origin. Maps must test whether
continents, mountain barriers, wetlands, rivers, coasts, and habitat corridors
make those journeys plausible at the chosen year and player-travel scales.

## Shared Ownership Constraints

- The authoritative calendar and local seasonal-climate query belong in
  shared simulation/world state, not in a desktop, browser, Android, or XR
  adapter.
- World generation and macro planning expose topology-aware climate,
  drainage, habitat, and corridor facts; they do not tick seasons.
- The server owns gameplay weather, resource response, migration admission,
  material animals, tagging, hibernation, husbandry, and interactive water or
  snow.
- Shared rendering owns seasonal surface and atmosphere semantics across
  mono, stereo, and XR multiview paths.
- The procedural horizon consumes the same climate interpretation at a
  filtered scale; it must not invent a different snow line or hemisphere.
- Platform hosts supply cadence and presentation targets only.

Do not place the feature in one app crate because one target proves it first.
Do not hide original Mclone seasons inside retained vanilla-weather comparison
logic.

## Recommended Evidence Ladder

1. Review Tactical 306's final exact-terrain matrix and shared live menu, then
   complete current-revision physical Quest Seasonal Debug interaction and
   performance. Review Tactical 314's square sun, continuous moon phases,
   latitude-aware stars, stereo stability, and explicit Quest star-cost
   exception. Tactical 307's coordinate/solar behavior and Earth-like sun size
   are already accepted; its focused full-game WebGPU sun-path gate also
   remains available if the combined Tactical 306 Web proof is insufficient.
2. Review Tactical 316's implemented 56-day calendar pace against sleep, real
   play time, travel, farming, current wildlife cadence, and the squirrel
   surplus-to-lean cache story. Continue to compare it with climate
   wavelength/scale and continental travel in Terrain Lab or another shared
   review surface before declaring the timing shipped.
3. After Tactical 304/305 settle the exact/LOD frontier, give the procedural
   horizon a filtered version of the accepted seasonal appearance. Do not copy
   the exact fragment implementation or weaken the explicit LOD ownership.
4. Add a loaded active-weather producer for snowfall rise/hold/decay only after
   choosing whether visual coverage remains derived or gains sparse
   authoritative interactive snow.
5. Complete Tactical 315's no-catch-up sleep/date operations, narrow seasonal
   modifiers over the existing loaded habitat resources, and squirrel
   mast/cache lifecycle. Prove exact full/accelerated active-domain
   equivalence, conservation, and inactive freeze before migration.
6. Prove one ambient waterfowl passage before interactive migration. Then
   resolve the anonymous-wildlife persistence contract before allowing
   gameplay animals to seasonally disappear or refill.
7. Add tagging and one supplied-food husbandry loop independently of migration.
   Test unload/reload, active-only aging and consumption, dense enclosure
   performance, and native/browser persistence.
8. Add seasonal creek/spring discharge only after shared drainage reach
   identity can keep active chunk boundaries coherent.

Every pixel-producing milestone requires capture and inspection. Every
gameplay milestone must prove bounded work with inactive regions held frozen.

## Open Decisions

- Does live play retain the implemented 56-day year after sleep, ordinary
  travel, farming, breeding, and cache-surplus-to-lean review?
- Does live travel and Human Review retain Tactical 307's provisional
  98,304-block plane wavelength, Z origin zero, and 27-degree tilt?
- Does the cylinder keep asymptotic polar tails, gain finite axial limits, or
  use another mapping?
- How should day length affect actual skylight, spawning, crops, and player
  visibility without creating excessive lighting churn?
- Which seasonal surface facts are purely visual, and which become sparse
  authoritative material state?
- How are player edits protected from seasonal snow and water reconciliation?
- Can anonymous gameplay wildlife be seasonally reconstructed without
  violating local depletion, durable identity, and understandable saves?
- Which interactions promote persistence: explicit tag only, taming,
  breeding, feeding/trust, transport, or a managed-habitat marker?
- How long may an anonymous same-season animal snapshot survive inactivity?
- What bounded receipt prevents migration-event farming without accumulating
  an unbounded per-year history?
- Which first species best separates ambient passage, gameplay arrival, and
  persistent managed identity?
- Are current continental and regional scales actually too small when judged
  against the selected calendar and travel speeds?
- How should supplied food join the existing resource strata without becoming
  a remote feeding authorization or population target?

## Related

- [`../tactical/314-celestial-moon-stars-and-square-sun.md`](../tactical/314-celestial-moon-stars-and-square-sun.md)
- [`../tactical/306-seasonal-appearance-preview.md`](../tactical/306-seasonal-appearance-preview.md)
- [`../tactical/307-seasonal-solar-path-and-cyclical-latitude.md`](../tactical/307-seasonal-solar-path-and-cyclical-latitude.md)
- [`habitat-driven-creature-ecology.md`](habitat-driven-creature-ecology.md)
- [`wildlife-ecology-state-model.md`](wildlife-ecology-state-model.md)
- [`persistent-actor-identity.md`](persistent-actor-identity.md)
- [`bounded-world-topology.md`](bounded-world-topology.md)
- [`mclone-macro-landscape-planning.md`](mclone-macro-landscape-planning.md)
- [`world-generation-profiles.md`](world-generation-profiles.md)
- [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md)
- [`fog-atmosphere.md`](fog-atmosphere.md)
- [`vanilla/weather.md`](vanilla/weather.md)
