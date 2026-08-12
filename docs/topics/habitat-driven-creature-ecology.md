# Habitat-Driven Creature Ecology

Topic: `habitat-driven-creature-ecology`

Status: active direction with its biome-habitat foundation and first complete
original creature-life chapter landed through 2026-08-12. Tactical
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
- real mallards emit spatially attenuated, flock-suppressed contact calls and
  shed collectible feathers under durable cooldowns;
- recent authoritative shore movement creates locally capped, expiring track
  figures that remain presentation evidence rather than world mutations; and
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
  local and deployed Web captures are pixel-identical, share the native seed,
  entry pose, entity composition, and guide receipt, and leave all IndexedDB
  world stores empty. The verified temporary play link is
  `https://mclone.kzahel.com/app.html?showcase=mallard-ecology`.
- A deterministic two-mallard 1,200-tick simulation proves retained non-local
  destinations, useful travel, water and actual dry-shore occupancy, bounded
  spacing and turning, and zero yaw mutations on stationary ticks.
- Local headed Web revision-2 acceptance observes the original three mallards
  move `3.33`, `2.21`, and `3.59` blocks over 80 authoritative ticks, then
  observes ordinary attended hatching and field-guide progress from 2/6 to
  5/6 while persistent browser world stores remain empty.

## Known Gaps and Recommended Next Work

- Define a shared habitat-query/fitness record above raw biome IDs, backed by
  Mclone climate, landform, hydrology, vegetation, and substrate semantics.
- Add death/removal persistence coverage and prevent local population
  resurrection through seed-time decoration.
- Build a first stewardship choice around mallards—food attraction, protected
  nesting cover, or restrained hunting—using the now-shared inventory,
  interaction, lineage, and persistence contracts.
- Explore compact unloaded population summaries after individual durable
  entities are correct; do not use them to weaken visible-entity continuity.
- Feed animal paths, grazing sites, nests, and water access back into terrain
  and structure planning as evidence-bearing landscape history.
- Promote a contrasting second original species whose mechanic forces a new
  terrain capability instead of merely reusing the mallard wetland.

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
