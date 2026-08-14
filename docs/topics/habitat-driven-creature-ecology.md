# Habitat-Driven Creature Ecology

Topic: `habitat-driven-creature-ecology`

Status: active direction with its biome-habitat foundation and four complete
original creature-life chapters landed through 2026-08-13. The continuing
architecture direction recorded 2026-08-14 keeps one durable animal and
active-tick model while composing a few bounded spatial, social, and habitat
patterns; it explicitly rejects a universal permanent-home abstraction and is
owned by
[`wildlife-ecology-state-model.md`](wildlife-ecology-state-model.md). Tactical
[`277`](../tactical/277-habitat-driven-creature-ecology-foundation.md) binds
natural cow/chicken habitat to generated chunk biomes and proves immediate
durability across persistent chunk unload/reload. Rich habitat fitness and
mechanics-led species promotion is now proven by Tactical
[`278`](../tactical/278-mallard-wetland-ecology-loop.md), beginning with a
mallard wetland loop, and Tactical
[`279`](../tactical/279-mallard-life-and-discovery.md), completing mallard life,
evidence, and discovery mechanics. Tactical
[`280`](../tactical/280-playable-showcase-links.md) now packages those ordinary
live mechanics as a bounded transient tiny save for matched screenshot and
interactive Web review. The continuing showcase contract lives separately in
[`playable-showcases.md`](playable-showcases.md).
Tactical [`282`](../tactical/282-mallard-behavioral-showcase.md) turns the
first rejected static review into shared retained habitat intent, stable
movement-derived orientation, a roomier wetland, and time-window evidence.
Tactical
[`284`](../tactical/284-deer-forest-edge-ecology-and-semantic-props.md)
completes deer as the contrasting second original chapter: generated
forest-edge intent, live habitat fitness, authored multi-state behavior,
durable sign, hunting/depletion, spatial calls, field notes, and semantic
props force new terrain and gameplay capabilities instead of cloning the
wetland loop.
Tactical [`285`](../tactical/285-bee-pollination-ecology.md) completes bees as
the third chapter: a shared habitat-fitness vocabulary, generated flowering
pockets, persistent colony-to-flower flight, real pollination, managed hotels,
renewable wax, field notes, and spatial buzz now make terrain both cause and
record ecological gameplay.
Tactical [`292`](../tactical/292-rabbit-burrow-ecology.md) completes the fourth
chapter: rabbits qualify and excavate one real shallow bank threshold, persist
as a warren family, emerge on world time, consume/raid real carrots, and make
ordinary fence/gate state into ecological gameplay.
Tactical
[`295`](../tactical/295-rabbit-refuge-memory-and-ecology-agent-foundation.md)
then replaces permanent warren ownership with rabbit-owned bounded refuge
knowledge, current derived occupancy, needs-driven reuse before excavation,
and deterministic bounded ecology work. The reusable state model remains
server-internal until deer or fox supplies a contrasting second consumer.

## Scope

This topic owns the continuing product and simulation contract that terrain
and creatures should develop together. It is not a promise to simulate a
complete ecosystem, nor a reason to put every Creature Lab figure into one
generic spawn pool.

The intended loop is:

```text
terrain and climate create habitats
  -> habitats support distinct creatures and flora
  -> creatures create needs, resources, risks, and traces
  -> players hunt, observe, collect, breed, farm, or protect them
  -> those mechanics reveal missing terrain and habitat vocabulary
  -> world generation gains better places, transitions, and history
  -> new places make more creature mechanics meaningful
```

The loop is valuable even when each implementation slice is small. A wetland
that exists because one animal needs shallow water, reeds, mud, and nesting
cover is more useful than a nominal biome label. An animal that leads the
player to that wetland, changes behavior there, and produces a habitat-specific
resource is more useful than another randomly distributed skin.

Reference spawning rules and vanilla-profile parity remain documented in
[`../creatures.md`](../creatures.md). Shared runtime ownership and persistence
remain in [`../entity-architecture.md`](../entity-architecture.md). The
original Mclone regional vocabulary remains in
[`mclone-overworld-breadth.md`](mclone-overworld-breadth.md). This topic owns
how those systems should reinforce one another as original game design.

## Product Principles

### Terrain is a gameplay content API

World generation should expose stable semantic facts that gameplay can query,
not only finished block columns. Useful habitat facts include climate,
landform, surface and substrate, water depth and flow, vegetation cover,
shelter, altitude, season, disturbance, and proximity to other habitats.

Biome identity is a useful coarse contract and the correct first input, but it
must not become the only ecological truth. A river bank, forest edge, sunny
rock face, deep pond, grazed meadow, and protected nesting island can differ
mechanically while sharing a nominal biome.

### Creatures justify places

Each promoted creature should answer more than “where can it randomly
appear?” A useful admission brief asks:

- what habitat makes it feel inevitable rather than scattered;
- what visible behavior communicates that habitat relationship;
- what the player can observe, collect, hunt, breed, farm, trade, protect, or
  be threatened by;
- what terrain features, flora, structures, or resources it motivates;
- what traces it leaves when the animal itself is absent; and
- what prevents the creature from becoming ubiquitous noise.

Creature Lab remains the broad authoring and review catalogue. Runtime
promotion is deliberate: figure and animation readiness are necessary, but
habitat, authoritative state, mechanics, persistence, sound, and validation
decide whether a creature belongs in the world.

### Habitats should form gradients and relationships

Hard biome membership alone tends to produce themed boxes. Prefer habitat
fitness assembled from continuous and local facts, then use thresholds and
rarity to create cores, shoulders, corridors, edges, and refuges. Examples:

- grazers prefer forage, open sight lines, water access, and tolerable slope;
- woodland animals prefer cover but may feed at edges and clearings;
- amphibious animals connect bank, shallows, and nearby land;
- predators depend on prey and cover rather than a biome name alone;
- migratory or seasonal animals depend on routes and time as well as place.

This makes terrain transitions mechanically legible and gives future macro
planning a reason to preserve connected habitat networks.

### Absence is content

Not every valid habitat should be occupied, and not every creature should be
common. Tracks, nests, calls, dung, browsed plants, burrows, feathers, or
predator remains can advertise a population without filling the active entity
budget. Population scarcity, local depletion, recolonization, and deliberate
reintroduction can create goals that uniform respawning cannot.

## Persistence Decision

A normal materialized creature in a persistent world becomes durable
immediately. Visiting a chunk is enough: once the authoritative server creates
the entity, leaving and returning must not reroll it merely because the player
did not edit a block or interact with the animal.

This follows the understandable Minecraft semantic and keeps one entity
lifecycle:

```text
eligible habitat + live spawn
  -> authoritative entity with stable persistent identity
  -> chunk-addressed entity record
  -> unload/save
  -> hydrate the same creature on return or reopen
```

The advantages of delaying persistence until interaction or a block edit do
not outweigh its hidden rules. A player could see an uncommon animal, walk out
of range, and return to a different roll; deaths and local depletion could be
silently undone; testing would depend on whether an unrelated edit crossed an
invisible threshold.

Laziness remains useful at a different layer. Explicit ambient swarms,
population summaries, migration plans, spoor, and other non-materialized
ecology may be reconstructed or advanced while unloaded. They must use a
separate data type and lifecycle rather than pretending a visible entity was
never real. Worlds backed by a null/transient store may also spawn deliberately
volatile entities, and diagnostics must say so.

The server must wait for a persistent entity chunk's load result before
natural spawning there. Otherwise a late entity record could replace or
duplicate a creature created during the load race.

## Population Layers

Keep these sources distinct even when they eventually share habitat tests:

| Layer | Purpose | Lifecycle |
|---|---|---|
| authored resident | named farm, settlement, quest, or showcase role | durable marker identity, realized once |
| generation-time population | seed-authored initial ecology where profile semantics require it | durable once materialized; never seed-resurrect killed animals |
| live natural spawn | local colonization under caps, cadence, habitat, light, distance, and chunk activity | durable entity in saved worlds |
| breeding/birth | player or simulation-created lineage | durable parentage and species state when those mechanics land |
| population summary | unloaded abundance, migration, or recovery model | compact ecological state, not an entity impostor |
| ambient presentation | bounded flock/insect/call effect with no individual gameplay identity | explicitly ephemeral |

The implemented slices use authored residents plus live natural cow, chicken,
and Mclone mallard spawns. They do not claim a population-summary simulation.

The continuing animal-record, knowledge, unavailable-world, social-memory,
and bounded spatial-pattern contract lives in
[`wildlife-ecology-state-model.md`](wildlife-ecology-state-model.md). This
topic supplies its terrain and habitat inputs rather than owning another AI or
persistence architecture.

## Mechanics Ladder

Creature breadth should grow by completing small loops rather than importing
many inert models:

1. **Discover and read:** habitat, silhouette, calls, tracks, and field-guide
   observations teach the player where and when to look.
2. **Approach and react:** flee, hide, flock, defend, hunt, graze, roost, swim,
   or seek shelter according to terrain facts.
3. **Collect and hunt:** species-specific renewable and nonrenewable resources
   carry tools, risk, season, quality, or ethical tradeoffs.
4. **Breed and domesticate:** food, space, social grouping, lineage, health,
   shelter, and temperament matter more than a single feed-to-clone action.
5. **Farm and steward:** enclosures, pasture rotation, crops, water, predators,
   disease pressure, manure, and habitat improvement connect animals back to
   building and terrain.
6. **Change the landscape:** trails, grazing pressure, nests, dams, burrows,
   pollination, seed movement, and local depletion create persistent evidence
   and new player decisions.

Not every creature needs every rung. Each should own at least one memorable
mechanical relationship, and related species should reuse shared behavior
without becoming palette swaps.

## Content Admission Contract

Before a Creature Lab figure joins ordinary spawning, record:

1. authoritative species/entity kind and durable species state;
2. habitat fitness inputs and at least one exclusion that preserves rarity;
3. spawn, birth, authored-placement, and removal sources;
4. one player-facing behavior or resource loop;
5. unloaded and persistence semantics;
6. animation, sound, drops/items, and interaction dependencies;
7. mob-cap/budget category and expected density; and
8. deterministic habitat tests plus rendered in-world evidence.

This contract should be data-driven enough that adding a creature exposes
missing capabilities instead of adding another species branch throughout the
server. Species may still need custom behavior modules when their mechanic is
genuinely distinct.

## First Vertical Slice

Tactical 277 intentionally uses the existing cows and chickens and now proves:

- live spawn candidates query the biome payload of the published generated
  chunk at the candidate surface position;
- the active world-generation profile therefore controls habitat, rather than
  a hidden Java-overworld biome source built only from the seed;
- terrain floor, headroom, collision, brightness, player distance, active
  chunks, creature cadence, and category cap remain enforced;
- saved worlds wait for entity-chunk load and create persistent records;
- null-store worlds preserve explicit volatile behavior for tests and
  transient sessions; and
- diagnostics distinguish biome rejection, missing biome payload, entity-load
  readiness, and durable versus volatile outcomes.

At the Tactical 277 foundation stage, the habitat table remained the existing
Java 1.17.1 farm-animal biome membership and only cow/chicken had implemented
protocol/runtime kinds. That slice remains a foundation proof rather than the
final original ecology design.

## Mallard Wetland Vertical Slice

Tactical 278 completes the first original terrain-creature-mechanics loop:

- Mclone generation places sparse lily pads and sugar cane only over verified
  inland wetland water and supported banks;
- one shared live-block habitat sample measures water columns, shallow beds,
  cover, and grass underfoot, and fails closed on missing data;
- only the Mclone profile can turn that sample into a bounded 2-4-member
  mallard flock, while Java farm-animal tables remain unchanged;
- eligible chunks are sampled without replacement, and generic farm animals
  wait as fallbacks until the bounded habitat scan finishes;
- mallards prefer verified shore destinations and fall back safely when edits
  remove the habitat;
- a due timer remains due away from wetlands and emits one distinct durable
  mallard egg on a qualifying shore; and
- mallards and eggs use the ordinary authoritative protocol, item, pickup,
  rendering, chunk persistence, and hydration paths.

## Mallard Life and Discovery Chapter

Tactical 279 closes the first mechanics ladder around that habitat:

- placing a carried mallard egg at a covered wetland shore creates an
  immediately durable nest instead of a decorative block or app-local prop;
- incubation advances only while the live habitat remains valid and two adult
  mallards attend, pauses when either condition fails, and resumes without
  losing progress;
- one nest produces at most one durable duckling whose parent identities, age,
  and growth survive unload/reload;
- mallards float and paddle in shallow water, retain non-local water and shore
  destinations, and apply bounded cohesion and separation around those intents
  without changing other passive mobs;
- body orientation follows realized horizontal travel at a bounded turn rate,
  so collision or arrival cannot create rapid stationary yaw jitter;
- authoritative water occupancy now survives incremental client updates, and
  shared actor composition sinks only the visual mallard figure by 24% of its
  presented height so feet paddle below the surface while the body rides above
  it; simulation, interaction, and persistence retain the original feet pose;
- real mallards emit spatially attenuated, flock-suppressed contact calls and
  shed collectible feathers under durable cooldowns;
- recent authoritative shore movement creates locally capped, expiring track
  evidence rather than world mutations; its current raised procedural visual
  is temporary pending the shared terrain-conforming path in
  [`terrain-surface-traces.md`](terrain-surface-traces.md); and
- seen, heard, feather, track, nest, and hatch observations form an idempotent
  per-player field-guide bitset, persisted and replicated to compact flat,
  stereo, and multiview presentation.

The design deliberately treats calls as information from actual creatures.
Their identity, position, finite radius, sequence, and cooldown originate on
the authoritative server; playback success is not required for the discovery
event, and no ambient soundtrack pretends that a flock exists.

## Current Evidence

- Canonical biome indexing covers X/Z quart cells, vertical quart layers,
  nonzero `min_y`, bounds, missing payloads, and topology-aware scheduler
  lookup.
- Live and dry-run planners distinguish missing generated biome data from a
  biome that honestly rejects farm animals.
- Persistent spawn candidates wait for entity-chunk load completion.
- A memory-world integration fixture naturally spawns four cows/chickens,
  unloads their chunks, and hydrates the same persistent identities with fresh
  runtime IDs on return.
- A persistent SQLite screenshot world with the debug showcase disabled
  naturally spawned and rendered eight actors; the inspected
  [in-world capture](</tmp/mclone-habitat-natural-creatures.png>) shows a cow
  occupying a grassy pond edge through the shared first-party actor path.
- A fresh persistent Mclone world naturally produced seven mallards with the
  debug showcase disabled. Four reloaded as four authoritative/drawn actors;
  the inspected [wetland capture](</tmp/mclone-mallard-natural-accepted.png>)
  shows the flock on grassy shallow-water margins.
- Generated-world integration proves a planned flock and its due egg survive
  full entity-chunk unload/reload with stable persistent IDs and fresh runtime
  IDs.
- Nest integration covers invalid placement, egg consumption, habitat and pair
  attendance, pause/resume, single hatch, parentage, growth, edited habitat,
  and entity-chunk hydration.
- Call, feather, track, and observation tests cover bounded emission, flock
  suppression, pickup, expiry/caps, every unlock source, idempotence,
  replication, and player-record persistence.
- The inspected [mallard ecology capture](</tmp/mclone-mallard-ecology.png>)
  now shows two adults, their smaller duckling, a visible near-hatch nest, and
  an incomplete field-guide entry across a multi-chunk wetland. The inspected
  [stereo capture](</tmp/mclone-mallard-ecology-stereo.png>) projects the guide
  legibly into both eyes through the shared world-GUI renderer.
- The `mallard-ecology` data recipe requires registered ordinary-game evidence
  for its lily pad, adults, duckling, nest, egg, feather, and observations. Its
  revision-1 local and deployed Web captures are pixel-identical, share the
  native seed, entry pose, entity composition, and guide receipt, and leave
  all IndexedDB world stores empty. The verified temporary play link is
  `https://mclone.kzahel.com/app.html?showcase=mallard-ecology`.
- A deterministic two-mallard 1,200-tick simulation proves retained non-local
  destinations, useful travel, water and actual dry-shore occupancy, bounded
  spacing and turning, and zero yaw mutations on stationary ticks.
- Local headed Web revision-2 acceptance observes the original three mallards
  move `3.33`, `2.21`, and `3.59` blocks over 80 authoritative ticks, then
  observes ordinary attended hatching and field-guide progress from 2/6 to
  5/6 while persistent browser world stores remain empty.
- The public revision-2 probe at pushed revision `25ea0d2e` reproduces those
  exact domain outcomes and displacements. Its inspected 1600x900 canvas shows
  four separated mallards across the wetland after the hatch, with the field
  guide visibly at 5/6 and an empty hotbar.
- Tactical
  [`283`](../tactical/283-mallard-waterline-presentation.md) corrects the
  accepted showcase's standing-on-water presentation. A replica regression
  covers incremental swim/life-stage/nest metadata, a render-session regression
  covers the species-specific model offset, and local flat, stereo, and headed
  Web pixels show submerged feet with the breast and body above water.
- The exact deployed revision `acee6205` reports three swimming mallards at
  both ends of its behavioral window. Its inspected public pixels retain the
  full accepted ecology sequence while placing the waterline through the lower
  legs instead of under the feet.
- Human Review 1 of Tactical 284 accepted authored mallard nest and feather
  props for live gameplay, but rejected rigid track figures as the wrong
  presentation abstraction. Nest entities and feather items now use required
  prepared semantic assets; terrain tracks retain their existing gameplay cue
  while a slope-conforming shared decal/surface path is developed.
- Human Review 2 accepted the articulated deer and its ten named clips. The
  exact checked asset is now a required shared `mclone:deer` runtime resource.
- The production Mclone vegetation planner now exposes a deterministic
  forest-edge intent above its existing coverage/density field. It samples
  four 64-block cardinal shoulders and reports local cover, minimum/maximum
  nearby cover, contrast, and explicit clearing/cover directions. This adds no
  deer-only noise and changes no generated blocks.
- A 97-by-97 seed-12345 atlas at 32-block spacing finds 840 transitional edge
  sites, 696 dense interiors, and 5,646 open interiors. Tests retain all three
  populations so later spawn tuning cannot silently make every forest or
  meadow an edge.
- Ordinary `mclone:deer` entities now enter the shared creature cap and
  immediate entity-chunk persistence path only when an Mclone forest-edge
  candidate also passes live grass, woody cover, browse, sight-line, slope,
  escape-cover, light, collision, distance, and current-disturbance checks.
  The planner emits bounded two-to-four-member groups rather than isolated
  decorative placements.
- Deer persistence and replication retain sex, fawn/adult stage, conditional
  antlers, behavior, behavior time, and health. The renderer hides semantic
  antler parts for females, fawns, and antlerless adult males, avoiding an
  asset/runtime mismatch while keeping one reviewed canonical figure.
- The first authoritative deer action loop now makes that forest edge
  legible: retained live-block browse targets drive walking and grazing,
  authored bedding transitions reach a stable rest pose, nearby players
  cause alert and then cover-seeking flight, and a fleeing herd member can
  propagate alarm. Separation/cohesion bands keep the group loose, while
  stale destination expiry and displacement-derived turning avoid the
  stationary-spin failure found in the first mallard showcase.
- Deer now choose dry walkable drinking banks beside water and bed only under
  measured woody cover. Repeated rest produces one durable semantic bed sign;
  persisted adult-antler timers can produce one collectible semantic shed
  antler without duplication across hydration.
- Contact, alarm, and impact cues originate from authoritative deer state,
  carry finite range, and suppress redundant local-herd events. Six original
  CC0 samples extend the reproducible first-party sound bank to 37 families
  and 127 samples.
- A normal starter hunting spear sends an authoritative targeted attack with
  reach, line-of-sight, and cadence validation. Accepted damage selects the
  authored `hit`, flee, and terminal `fall` clips before ordinary venison,
  hide, and conditional antler drops. A persisted local history keeps the
  killed identity absent and permits a different identity only after 60
  natural-spawn cycles.
- Six deer observations—seen, sign, alert, flee, shed antler, and harvest—are
  idempotent per-player facts persisted and rendered through the same flat,
  stereo, and multiview field-note surface as mallard discovery.
- The `deer-forest-edge` data recipe is the second bounded tiny-save review.
  Local and public headed WebGPU each observed one deer travel `11.2` blocks
  over 80 ticks, alert/flee clips, field-note progress to 3/6, four drawn
  semantic actors, and zero records in every browser world store. The verified
  temporary play link is
  `https://mclone.kzahel.com/app.html?showcase=deer-forest-edge`.
- Post-review interactive play found that fleeing deer translated while their
  legs stayed near the first locomotion frame. The shared client was resetting
  all derived animation state on every non-chicken authoritative reconcile.
  It now preserves identity-stable distance phase; the showcase gate requires
  both `11.2` blocks of entity displacement and more than `0.5` units of
  rendered deer locomotion advance. The local correction observed `11.1524`
  units of phase advance over its 80-tick behavior window.
- Phone review also found the spear unusable in practice because the larger
  touch hotbar hid its inventory contents behind slot numbers. Touch and
  desktop now present one canonical inventory through different-sized layouts;
  the inspected phone-sized deer capture shows selected `SP`, names it
  `Hunting spear`, and places it beside the visible `ATK` control. A dedicated
  mobile showcase smoke now prevents desktop-only review from accepting this
  interaction path again.
- Deer hoofprints are intentionally absent. Human review rejected rigid track
  figures and did not find tracks valuable enough to block the chapter. Any
  future tracking mechanic must justify itself first, then use the shared
  terrain-conforming surface-trace path rather than add raised deer geometry.
- Wetland, forest-edge, and flowering samples now share a bounded
  `HabitatFitness` record for forage, shelter, substrate, open movement space,
  continuity, and overall quality while retaining species-specific facts and
  thresholds. It remains server-internal until another consumer justifies a
  public cross-crate query API.
- Mclone decoration revision 16 creates deterministic clustered dandelion and
  poppy pockets through its ordinary vegetation pass. The Java-1.17.1
  reference Overworld remains unchanged. Bee spawning reads the published
  flowers, grass, woody cover, air clearance, and slope rather than a hidden
  bee-only noise field.
- A suitable unoccupied site creates one persistent nest and two or three
  individually persistent bees. Their retained state identifies the colony,
  flower, behavior, elapsed behavior time, and pollen load. `hover -> fly ->
  forage -> return -> at nest` uses realized 3D displacement for orientation
  and collision-aware travel; reaching home deposits bounded colony work and
  may spread one real supported flower under cooldown.
- A placeable semantic bee hotel consumes its starter item only when support,
  clearance, colony distance, and current flowering habitat pass. The ordinary
  habitat scan later colonizes an empty hotel; occupancy is persisted on the
  colony so unloaded resident bees cannot duplicate colonization.
- Server-validated colony use yields one collectible beeswax entity only at
  the work threshold, then resets that threshold. Six durable notes cover
  bee, nest, forage, pollen return, pollination, and wax. Spatial buzz events
  originate from actual bees and suppress redundant members of one colony.
- The transient `bee-pollination` review world proves those same mechanisms.
  Human review rejected revision 1's behavioral evidence: nearest-flower
  choice kept the colony clustered, repeated one flower, and straight
  collision clipping could leave a bee visibly stuck. Tactical
  [`286`](../tactical/286-bee-foraging-range-and-recovery.md) replaces that
  live behavior with stable colony-member near/middle/far forage bands,
  immediate-repeat avoidance, moving hover and elevated cruise waypoints,
  progress recovery, and bounded trip abandonment.
- Revision 2's local and exact-revision public desktop and phone gates sample
  every bee over 360 ticks and reproduce the same deterministic measurements.
  Each traveled `20.20`-`22.83` blocks; colony radii reached `7.42`, `15.39`,
  and `12.40` blocks; no bee remained nearly stationary for more than one
  20-tick sample; `hover`, `fly`, and `forage` all appeared; pollination
  changed a real block; and notes reached 5/6. The phone gate also taps the
  ninth item slot and creates a real hotel. Both leave zero IndexedDB records.
  The temporary play link remains
  `https://mclone.kzahel.com/app.html?showcase=bee-pollination`.
- Follow-up phone review found that bees could translate toward flowers while
  displaying one frozen wing pose. Tactical
  [`287`](../tactical/287-bee-wingbeat-presentation.md) replaces the incorrect
  distance-phased flight with an authoritative elapsed loop. Exact public
  desktop and phone gates now require and observe all three bees moving
  `14.57`-`20.84` rendered blocks while their uninterrupted flight phases
  advance `199`-`280` ticks; a clip-name-only claim can no longer pass.

## Rabbit Burrow and Garden Chapter

Tactical 292 adds the first creature-created durable terrain fact. A natural
Mclone founder requires a live dry supported bank plus forage, cover, open
movement space, and local occupancy headroom. It visibly digs, removes exactly
one qualifying dirt/grass cell, and creates a persistent semantic burrow mouth
whose compact warren stores capacity and stable resident identities. The
deeper space is intentionally not a tunnel graph.

Rabbits retain home, lineage, life stage, underground visibility, health,
breeding state, and raid cooldown through entity-chunk persistence. World time
drives entry/emergence; ordinary collision drives hop, flight, fence exclusion,
and open-gate traversal. Live mature carrots are raid targets, carried carrots
tempt and feed, and two compatible fed adults with warren capacity can create
one durable kit. Six field observations and bounded spatial cues arise from
real rabbit, terrain, crop, and family events.

The data-only `rabbit-burrow` review save proves this shared loop rather than
implementing it. Exact public desktop and phone gates observed all four
rabbits travel about 21–30 blocks, one founder-created second burrow, two
carrot-age mutations only after the gate opened, ordinary carrot feeding, 5/6
notes, and zero browser-world records. The temporary play link is
`https://mclone.kzahel.com/app.html?showcase=rabbit-burrow`.

## Known Gaps and Recommended Next Work

- Decide which consumer actually needs the now-shared habitat-fitness record
  outside server spawning before making it a public cross-crate API. Avoid a
  generic ecology framework without a concrete query or simulation owner.
- Evolve rabbits from one authoritative permanent-home relationship toward
  individual refuge knowledge and replaceable warrens. An inaccessible
  remembered warren should remain remembered; a rabbit needing shelter may
  locate a reachable mouth with capacity or excavate one suitable cell nearby.
  Density, suitability, cost/cooldown, reuse, abandonment, and collapse must
  prevent uncontrolled burrow spam.
- Use deer as the contrasting proof for replaceable bedding memories, familiar
  range, and bounded herd knowledge. Persistent player disturbance should
  lower a bed site's preference and encourage another site rather than leave a
  deer permanently unable to rest.
- Add fox predator pressure only after the shared unavailable-target and
  interruptible-action semantics are explicit. A den should matter during the
  appropriate life stages without making all fox behavior orbit a permanent
  home pointer.
- Build a first stewardship choice around mallards—food attraction, protected
  nesting cover, or restrained hunting—using the now-shared inventory,
  interaction, lineage, and persistence contracts.
- Explore compact unloaded population summaries after individual durable
  entities are correct; do not use them to weaken visible-entity continuity.
- Feed animal paths, grazing sites, nests, and water access back into terrain
  and structure planning as evidence-bearing landscape history.
- If tracking becomes a real player mechanic, replace the existing raised
  mallard track visual through the bounded shared path in
  [`terrain-surface-traces.md`](terrain-surface-traces.md). Do not add deer
  hoofprints merely as ambient decoration.
- Build the next chapter around either crop growth that lets bee pollination
  affect yield, a reusable food/storage consequence for gardens and hunting,
  predator pressure that connects prey and cover, or a cliff animal that
  requires connected steep-surface navigation. Each should consume the
  habitat vocabulary while adding a different player loop.

## Code and Documentation Map

- `native/crates/mclone-server/src/entity/spawning/`: caps, habitat tables,
  wetland sampling, fair bounded candidate selection, placement, and live
  request planning
- `native/crates/mclone-server/src/integrated.rs`: authoritative natural-spawn
  integration, generated-chunk habitat access, entity-load gating, and
  persistence selection
- `native/crates/mclone-server/src/entity/store.rs`: durable versus volatile
  entity records
- `native/crates/mclone-core/src/chunk.rs`: canonical chunk biome payload
- [`../creatures.md`](../creatures.md): Java 1.17.1 spawning reference and
  parity ledger
- [`../entity-architecture.md`](../entity-architecture.md): shared entity
  ownership and persistence architecture
- [`animal-catalogue.md`](animal-catalogue.md): Creature Lab catalogue status
- [`playable-showcases.md`](playable-showcases.md): bounded tiny-save review
  links and enforced live-instantiation evidence
- [`mclone-overworld-breadth.md`](mclone-overworld-breadth.md): original
  terrain/ecology breadth ledger
- [`wildlife-ecology-state-model.md`](wildlife-ecology-state-model.md): durable
  animal knowledge, availability semantics, social memory, spatial patterns,
  and candidate species pressure tests
