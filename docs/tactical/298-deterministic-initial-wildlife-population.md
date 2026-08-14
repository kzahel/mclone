# Tactical 298: Deterministic Initial Wildlife Population

Status: complete 2026-08-14

Topic: `habitat-driven-creature-ecology`

Topic: `wildlife-ecology-state-model`

## Instruction Synthesis

Replace Mclone Overworld's complicated rabbit-specific natural-spawn
criteria with a simpler wildlife-population model. Give the world a general
desired wildlife density, tune species ratios through broad terrain and biome
appropriateness, and establish animals when chunks are first populated in a
seed-deterministic, traversal-order-independent way. Add a Terrain Lab view
that runs the exact production planner so population density, species choice,
and habitat relationships are visible and shareable.

## Reference and Current-System Findings

Minecraft Java 1.17.1 has two distinct passive-mob producers:

- `NoiseBasedChunkGenerator.spawnOriginalMobs` performs a coordinate/world-
  seeded population pass while a chunk is first generated; and
- `ServerChunkCache.tickChunks` also admits passive natural-spawn work on a
  much slower 400-tick cadence inside shuffled ticking chunks.

Vanilla rabbit eligibility itself is intentionally small: the biome spawn
table selects rabbits, then the rabbit placement predicate accepts grass,
snow, or sand under sufficient light and produces a group of two or three.
Mclone's current live pass instead chains flowering, wetland, forest-edge,
rabbit-bank, and fallback farm-animal scans through generated blocks every
400 ticks. Rabbit admission alone counts browse, exposed banks, and open
columns in a radius. That is difficult to tune as one population, makes a
single species carry too much geography policy, and couples results to which
eligible live chunks are visited and chosen first.

The current entity persistence record already supplies the required durable
marker. `None` means that an entity chunk has never been realized; a saved
`Some(record)` may legitimately contain zero entities and means it has. A new
schema or parallel ecology database is therefore unnecessary.

## Binding Decisions

### One pure initial-population planner

`mclone-worldgen` owns a coordinate-pure Mclone wildlife planner. A fixed
64-by-64-block population cell (four by four chunks) receives an independent
seed domain and can plan at most one encounter group. Each plan exposes:

- the cell and owning chunk;
- bounded candidate/sample coordinates;
- broad terrain, climate, hydrology, and forest-cover evidence;
- a desired-density/productivity value;
- one suitability weight per supported species;
- the deterministic occupancy roll and weighted species roll; and
- the selected anchor and bounded group size, when occupied.

Every value is a function of profile revision, seed, topology, and cell
coordinates. There is no shared random stream, loaded-entity query, traversal
state, or caller-provided RNG. Querying cells or their member chunks in any
order must produce identical records.

The first supported mix is rabbit, deer, mallard, and bee. Rabbits receive the
largest general-land baseline so they are common without needing an exposed
bank count. Deer favor productive land with mixed cover; mallards strongly
favor watercourses, wetland pools, and banks; bees favor productive flowering
conditions near some woody cover. These are broad weights, not binary themed
boxes. Unsuitable ocean and alpine cells may remain empty.

One encounter is the budget unit. Group-size ranges remain species-owned and
small. This avoids treating one bee as ecologically equivalent to one deer,
prevents a quadratic entity census, and makes density and composition legible
over a map. Ratio tests cover a broad deterministic survey rather than
requiring every small window to match a quota.

### First entity-chunk realization

Only the population cell's selected owning chunk can materialize its group.
For a persistent world, the authoritative server considers that chunk after
its entity-record load resolves:

- `None`: run the pure plan once, place the ordinary live entities against
  the loaded chunk snapshot, mark the entity chunk dirty even if no group can
  be placed, and let the normal entity persistence lifecycle save it;
- `Some(record)`: hydrate the saved entities and never consult the seed plan
  to replace dead, moved, or absent wildlife.

Transient/null-store worlds use the same plan once per chunk per session.
They intentionally have no cross-session persistence claim. A population
decision never runs before the terrain snapshot needed for exact placement is
available.

Broad planning and exact placement stay separate. Worldgen decides whether,
what, and approximately where. The server validates ordinary collision,
surface, water, and species-specific live requirements in a small fixed set of
positions near the planned anchor. A failed exact placement still completes
that chunk's initial-population attempt; it does not search neighboring chunks
or retry forever.

The Mclone-specific periodic live habitat chain is retired by this tactical.
It must not continue adding cows, chickens, rabbits, deer, mallards, or bees
behind the deterministic plan. The Java 1.17.1 `overworld` profile retains its
separate reference-shaped live-spawn path. Later recolonization, migration,
predator response, or coarse population recovery requires an explicit
ecology producer and cannot be smuggled back in as generic random spawning.

### Terrain Lab is a production diagnostic

Add an optional Mclone-only `Wildlife` pane to Terrain Lab. A dedicated Worker
calls a Rust/Wasm facade over the same `mclone-worldgen` planner used by the
server. TypeScript owns only canvas presentation, input, labels, and colors.

The pane is a two-dimensional semantic map synchronized with the existing
seed, center, footprint, pan, zoom, and URL state. It must show:

- biome/landform habitat as a quiet map substrate;
- the fixed population-cell lattice;
- occupied encounter anchors and group sizes by species;
- empty versus unsuitable cells;
- a selected-cell inspector with density, habitat evidence, all species
  weights, rolls, and the chosen result; and
- aggregate visible-window encounter and species counts.

The URL must be sufficient to reproduce the view. The pane is diagnostics,
not a showcase or alternate simulation, and gains no authority to spawn or
edit a world.

## Performance and Bounds

- one independent plan per 64-by-64-block cell;
- a fixed, documented number of terrain/vegetation samples per plan;
- at most one encounter and one owning chunk per cell;
- a fixed exact-placement candidate limit;
- no entity-to-entity scan, active-player census, or habitat-radius block
  count in initial planning; and
- Worker compilation of only the cells intersecting the visible lab
  footprint, with a hard cell-count bound and stale-epoch cancellation.

The existing active AI scheduler and budgets continue to govern materialized
animals after creation. This tactical is not offscreen ecology or catch-up
simulation.

## Compatibility and Migration

The compatibility ledger classifies `mclone-overworld-v1` as
`internal-mutable`, explicitly including spawn rules. Existing entity records
remain authoritative. An old internal world can therefore receive initial
wildlife only in entity chunks that genuinely have no record; disposable test
worlds may be regenerated. No change is made to the reference-locked Java
1.17.1 `overworld` producer.

## Execution Slices

1. Land this tactical and update the living ecology direction.
2. Add the pure planner, pinned distribution evidence, negative-coordinate
   and randomized-query-order tests, and server-neutral species records.
3. Integrate first-realization materialization and remove the Mclone live
   habitat chain, with persistence/no-resurrection/order tests.
4. Add the Worker-backed Terrain Lab pane, URL state, inspector, ownership
   lock, and desktop/phone interaction tests.
5. Run targeted and workspace validation, inspect rendered native and browser
   evidence where affected, update living docs and this execution record, and
   commit the verified result in logical slices.

## Acceptance

- The same seed and population cell produce byte/equality-stable diagnostics
  regardless of query or chunk-load order, including negative coordinates.
- Each populated cell owns zero or one small group; aggregate density and
  species receipts remain inside explicit deterministic bounds.
- Persistent first visit survives unload/reload with stable entity IDs;
  killing or otherwise emptying the chunk does not seed-resurrect wildlife.
- A null-store session does not repeatedly populate the same chunk while it
  remains the same session.
- Mclone's 400-tick live scan cannot add a second generic population, while
  the reference profile's intended path remains covered.
- Terrain Lab's `panes=wildlife` URL displays the exact production decisions,
  supports pan/zoom and inspection, and is visually usable on desktop and
  phone.
- Rust, Wasm, TypeScript, ownership, focused server, and relevant workspace
  tests pass; rendered screenshots are inspected from `/tmp`.

## Execution Record

The implementation landed in five logical slices:

- `c22cbda7` recorded this binding plan and the living direction;
- `0923dfea` added the coordinate-pure revision-1 planner;
- `84fb2699` connected first entity-chunk realization and persistence;
- `1785aee4` added the Worker-backed Terrain Lab population map; and
- `57e41dc7` deleted the retired Mclone live habitat policy instead of
  retaining it as unreachable alternate spawn logic.

Revision 1 uses one 64-by-64-block cell, nine fixed stratified candidate
samples, and at most one owner chunk/group. Its pinned seed-`-98765` survey of
4,096 cells yields 1,519 occupied cells and 4,331 animals, with encounter
counts `[854 rabbit, 286 deer, 163 mallard, 216 bee]`. Randomized query order,
negative Euclidean coordinates, and periodic-X alias tests are exact.

Persistent realization waits for both the entity-record result and a usable
terrain snapshot. A missing record plans once; any present record, including
saved empty state, suppresses seed resurrection. Null-store worlds remember
attempted chunks once per session. Exact placement remains bounded to the
selected owner chunk and creates ordinary rabbit, deer, mallard, or bee-colony
state through the existing entity lifecycle.

Terrain Lab accepts `panes=wildlife` only for `mclone-overworld-v1`. The Rust
Wasm facade calls `McloneOverworldWildlifePlanner::plan_cell`; a dedicated
Worker builds only the bounded visible cell window; and TypeScript draws the
receipts. A source-ownership test rejects copied density/species arithmetic in
the canvas. The accepted review URL is:

<http://127.0.0.1:5180/terrain/?profile=mclone-overworld-v1&seed=-98765&x=0&z=-2048&blocks=1024&panes=wildlife&view=map>

Inspected acceptance captures are:

- [desktop UI](/tmp/mclone-terrain-lab-desktop-chrome-wildlife-ui.png);
- [phone UI](/tmp/mclone-terrain-lab-phone-chrome-wildlife-ui.png);
- [desktop canvas](/tmp/mclone-terrain-lab-desktop-chrome-wildlife.png); and
- [phone canvas](/tmp/mclone-terrain-lab-phone-chrome-wildlife.png).

Validation completed:

- `cargo test --manifest-path native/Cargo.toml` passed the full Rust
  workspace; after final live-policy deletion, all five focused live-planner
  tests and all 22 integrated entity tests passed;
- `pnpm terrain-lab:typecheck` and all 28 Terrain Lab state/ownership tests
  passed;
- the focused wildlife Playwright gate passed on desktop and phone, including
  deterministic reload, pan, inspection, and captures; and
- the production desktop Terrain Lab build/smoke passed.

The broad Playwright sweep also exposed two unrelated existing phone click
interception failures in the ordinary terrain and semantic-terrain controls;
both desktop cases pass, and the wildlife phone case passes. The broad mobile
smoke likewise waits for a status pill intentionally hidden by the existing
phone header. These adjacent harness issues do not exercise or block the
wildlife pane and remain separate Terrain Lab maintenance.

## Non-Goals

- season simulation, catch-up ecology, offscreen predation, or population
  recovery;
- a generalized ecology database or per-species framework;
- adding deer breeding, predators, stealth, scent, tracks, or new art;
- forcing exact regional ratios or preventing all local population variance;
- changing Java 1.17.1 worldgen parity; or
- turning Terrain Lab into a playable fixture or world editor.
