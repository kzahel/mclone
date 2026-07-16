# Tactical 187: Generator Profiles, Flat Grass, and Seeded Island

Status: active parent; product direction accepted 2026-07-16. Slices 0 through
4 are complete; the bounded fork-readiness inventory is next.

Topic: `world-generation-profiles`

Workstream: shared native Rust world generation and server scheduling, desktop
validation first, with native web/Web Worker, persistence, dedicated-server,
and dimension parity in the same contract.

## Objective

Introduce the smallest durable chunk-generator boundary needed to support more
than the current vanilla-shaped overworld path, then prove it with two bounded
generators:

1. a deterministic flat grass world; and
2. a seeded, nontrivial, explicitly non-vanilla small-island world.

The result must make the current overworld generator one implementation behind
a stored generation profile rather than the implicit definition of all
procedural world generation. It must not build a speculative general registry,
port Mojang's serialization framework, or begin the custom-biome/structure
campaign prematurely.

The intended product sequence is:

```text
current broad vanilla biome/decor agreement
  -> explicit generator/profile boundary
  -> flat-grass base case
  -> seeded original island proof
  -> frozen vanilla17 and mclone ruleset fork
  -> original biomes, decoration, and structures
```

## Product Decision

Mclone will continue using Minecraft Java 1.17.1 as the broad biome, terrain,
and ordinary-decoration reference while that work supplies engine breadth and
known-good validation. It will not treat exact vanilla expansion as the only
permanent product direction.

At an explicit future fork:

- the current oracle-tested path remains a frozen vanilla-1.17-shaped
  reference profile;
- a separately identified mclone profile may initially reuse its terrain
  primitives while owning biome selection, decoration tables, seed domains,
  structures, and later terrain changes;
- existing worlds retain the generator identity and behavior version with
  which they were created;
- vanilla oracle gates never silently become acceptance tests for the original
  mclone profile.

Flat grass and seeded island precede that fork. They prove the engine boundary
without requiring a new dynamic biome registry or the final mclone overworld
design.

## Verified Current Mclone State

### Product and persistence profile

`mclone-server::WorldGenerationProfile` is already a shared, serializable,
dimension-owned contract. It currently has only:

- `Overworld`, the backward-compatible default; and
- `AuthoredOnly { missing_chunk: Void }`, where a real persistence miss
  produces a deterministic empty chunk rather than invoking overworld
  generation.

The profile flows through local world metadata, dimension definitions,
integrated and dedicated startup, native and browser catalogs, and CLI/browser
selection. `ChunkScheduler::set_world_generation_profile(...)` forbids changing
it once chunk work begins.

This is the correct product/persistence seam. It is not yet a procedural
generator abstraction.

### Procedural worldgen path

The current procedural path is overworld-specific after the persistence miss:

- the scheduler enqueues feature jobs with a seed, target chunks, and retained
  lower-status dependencies;
- the native mailbox and Web Worker codec assume
  `OverworldFeatureDependencyCache`;
- the cache constructs `OverworldBiomeSource` and the current
  `NoiseBasedChunkGenerator`;
- surface generation, carvers, feature biome resolution, and placed-feature
  tables accept concrete `OverworldBiomeSource` values;
- the worker request has no generation profile or behavior version.

`NoiseBasedChunkGenerator<B>` is generic over the reduced
`NoiseBiomeSource` terrain-density interface, and tests use constant/repeating
sources. That generic is useful but does not make the full production pipeline
pluggable: surface, biome storage, carvers, decoration, job planning, and spawn
policy remain concrete overworld behavior.

### Flat and authored content

There is no selectable procedural flat-world generator. Current flat grass
appears only in focused feature tests and pre-authored fixture chunks. The
authored island/table worlds prove persistence-backed bounded content, not
seeded generation on storage misses.

### Structures

There is no live native true-structure runtime today. The current native
`ChunkStatus` vocabulary is `Terrain`, `Surface`, `Features`, `Light`, and
`Full`; generated chunks carry blocks, biomes, and scheduled ticks but no
structure starts, pieces, references, or bounding boxes. The former buried
treasure and desert-well implementations were in the retired TypeScript
engine.

This tactical does not implement structures. Its generator plan and result
contracts must leave room for future structure dependencies and metadata
without pretending those systems already exist.

## Minecraft 1.17.1 Reference Shape

Minecraft does not have only one chunk generator:

- `ChunkGenerator` is an abstract lifecycle boundary;
- registered implementations include `NoiseBasedChunkGenerator`,
  `FlatLevelSource`, and `DebugLevelSource`;
- `BiomeSource` is independently dispatched and includes overworld, fixed,
  checkerboard, multi-noise, and End implementations;
- `FlatLevelSource` fills configured vertical layers while reusing the common
  biome, structure, decoration, height, and spawn contracts;
- `WorldGenSettings` persists the generator inside a dimension definition.

Reference anchors:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkGenerator.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/FlatLevelSource.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/flat/FlatLevelGeneratorSettings.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/biome/BiomeSource.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/levelgen/WorldGenSettings.java`

The reusable lesson is the generator boundary, not Mojang's full codec and
registry stack. Mclone should retain a closed, versioned profile vocabulary
until user-authored generator plugins are a real requirement.

## Target Ownership Shape

```text
DimensionDefinition / stored world metadata
  seed
  WorldGenerationProfile (stable persisted identity)

DimensionRuntime / ChunkScheduler
  persistence hit always wins
  persistence miss consults profile
  generator-independent holder, light, publication, and save lifecycle

WorldgenMailbox / Web Worker session
  GenerationRequest { descriptor, targets, dependency delta }
  profile dispatch + profile-specific resident state
    Overworld -> OverworldFeatureDependencyCache
    FlatGrassV1 -> stateless layer generator
    SmallIslandV1 -> stateless seeded field generator

mclone-worldgen
  generator-specific planning and block/biome production
  shared GeneratedChunk result contract
```

Ownership rules:

- `mclone-server` owns the persisted profile vocabulary, missing-chunk policy,
  scheduling, worker lifecycle, lighting, publication, and persistence.
- `mclone-worldgen` owns procedural generation algorithms and their
  deterministic plans/results.
- app crates own only profile selection and display labels. They do not
  interpret generation algorithms or produce chunks.
- native and browser workers execute the same Rust generator dispatch. The
  TypeScript worker shell transports the descriptor but never implements a
  generator.

## Profile Identity and Compatibility

Do not rename or reinterpret existing worlds accidentally.

The first implementation should preserve the existing serialized
`Overworld` discriminant and `overworld` label. Its documented meaning becomes
"the current vanilla-1.17-shaped overworld rules." A source-level rename to
`Vanilla17` is optional later and must not change stored or external values.

Add stable profiles provisionally labeled:

| Persisted label | Meaning | Seed use | Behavior promise |
|---|---|---|---|
| `overworld` | current vanilla-1.17-shaped path | full | existing compatibility/oracle path |
| `flat-grass-v1` | fixed bedrock/dirt/grass layers | identity only | immutable v1 layers/biome |
| `small-island-v1` | original seeded island in ocean | full | immutable v1 field/material rules |
| `authored-only` | stored chunks or deterministic void | identity only | existing authored miss policy |

Version is part of profile identity for the new generators. Do not add a
mutable global "current generator version" whose meaning changes beneath an
old save. A later algorithm is `small-island-v2` or a new descriptor, while
old `small-island-v1` worlds continue to generate unseen chunks with v1.

Native binary persistence discriminants, serde JSON, browser catalog strings,
CLI parsing, dedicated startup, and test fixtures must all retain exact legacy
decoding. New discriminants append; existing `0`/`1` meanings do not move.

The future original overworld should likewise receive a versioned identity
such as `mclone-overworld-v1`. It is not added by this tactical unless the
vanilla/mclone fork begins in the same reviewed slice.

## Generator Dispatch Contract

Prefer a closed Rust enum dispatcher over premature trait-object and plugin
machinery. A profile-selected worker session may internally own something like:

```rust
enum ProceduralGeneratorSession {
    Overworld(OverworldFeatureDependencyCache),
    FlatGrassV1(FlatGrassGenerator),
    SmallIslandV1(SmallIslandGenerator),
}
```

The exact names may change, but the boundary must expose these facts:

```rust
struct GenerationRequest {
    descriptor: GenerationDescriptor,
    targets: Vec<ChunkPos>,
    seeded_dependencies: Vec<MutableChunkBlockBuffer>,
}

struct GenerationPlan {
    targets: BTreeSet<ChunkPos>,
    generator_dependencies: BTreeSet<ChunkPos>,
}

struct GenerationResult {
    chunks: BTreeMap<ChunkPos, GeneratedChunk>,
    retained_dependencies: BTreeMap<ChunkPos, MutableChunkBlockBuffer>,
    timing: GenerationTiming,
}
```

These are illustrative, not an instruction to add redundant wrapper types.
Reuse and rename current batch types when that keeps call sites smaller.

Hard contract points:

1. Generator dependencies are distinct from lighting and client-publication
   dependencies. A flat generator needing no neighboring generation does not
   remove the scheduler's normal light/readiness rules.
2. `Overworld` preserves the current dependency plan, resident cache, random
   order, and oracle output.
3. `FlatGrassV1` and initial `SmallIslandV1` require only their target chunks.
4. A persistence hit bypasses generation for every profile.
5. A worker session is keyed by descriptor and seed. Reusing retained state
   across a descriptor/seed change is forbidden; reset must be explicit and
   tested in native and web job sessions.
6. Every requested target produces exactly one feature-complete
   `GeneratedChunk`, or the entire job reports a typed failure. Silent target
   omission is not allowed.
7. Result publication, lighting, snapshot conversion, persistence, unload,
   and runtime mutation stay generator-independent.
8. Generator selection is fixed before the first chunk schedule, preserving
   the current liveness guard.

Do not force `AuthoredOnly` through procedural worker machinery merely to make
the diagram uniform. It is a storage-miss policy and may retain its cheap
scheduler-owned void path.

## Biome Contract for the First Proofs

This tactical does not require a dynamic biome registry.

- `FlatGrassV1` emits the existing plains biome ID throughout.
- `SmallIslandV1` emits existing ocean, beach, and plains biome IDs according
  to its generated column classes.
- both generators use the canonical chunk biome payload shape expected by
  snapshots, persistence, tinting, spawning, and rendering;
- neither generator routes through `OverworldBiomeSource` merely to obtain
  those IDs.

Using existing biome IDs keeps current tint, assets, and client contracts
valid while proving that biome fields can be generator-owned. Original biome
keys/IDs, their registry lifetime, mclone-specific surface rules, and
profile-selected feature tables belong to the later vanilla/mclone fork.

## Flat Grass V1 Specification

`FlatGrassV1` is a production-selectable base case with intentionally small
semantics:

- build range remains the current `0..256` world contract;
- Y `0` is bedrock;
- Y `1..=2` is dirt;
- Y `3` is grass block;
- Y `4..=255` is air;
- all biome samples are plains;
- no water, carvers, ordinary decoration, structures, block ticks, or liquid
  ticks are generated;
- the block and biome result is seed-independent, but the world's selected
  seed is still retained as realm/dimension identity;
- spawn is centered near block `(0.5, 4.0, 0.5)` through a shared
  profile-aware spawn contract, not an app special case.

The layers intentionally match the basic Minecraft flat-grass vocabulary, but
this profile does not claim full `FlatLevelSource` parity: vanilla flat
configuration, toggled decorations, villages, structures, presets, and codecs
are out of scope.

Acceptance invariants:

- origin, positive, negative, and far chunks have identical local layers;
- adjacent borders match exactly;
- every column's motion-blocking/world-surface height is correct;
- generated biome payload length and values are valid;
- no target or dependency work is scheduled outside the requested set;
- save/reopen and native/Web Worker generation reproduce identical chunks;
- a full-frame desktop capture shows a lit, collision-backed grass plane and
  is inspected before the slice closes.

## Small Island V1 Specification

`SmallIslandV1` is the first real seeded, nontrivial, non-vanilla procedural
terrain proof. It is deliberately a bounded test/product world, not the final
mclone overworld.

### Shape

- Generate a continuous height/material field from absolute world X/Z, never
  from per-chunk local coordinates.
- Use an mclone-owned seed domain and fixed v1 constants. Do not preserve
  Java/Minecraft random draw counts or call the vanilla biome layer stack.
- Combine a bounded radial island envelope around origin with low-frequency
  seeded distortion/noise so different seeds change the shoreline and relief
  without moving the guaranteed central land away from spawn.
- Guarantee a dry, reasonably level inner spawn patch. Spawn success must not
  depend on a lucky random seed or an unbounded search.
- Make terrain outside the bounded island envelope a stable shallow/deep ocean
  floor filled with water to the shared sea level.
- Keep the v1 height field single-valued per column: no caves, overhangs,
  carvers, or cross-chunk writes.

The first implementation slice must lock exact constants and formulas in the
tactical execution record before treating fixtures as compatibility promises.
Candidate acceptance ranges, to be finalized with the first inspected render:

- sea level `63`;
- ocean floor comfortably below sea level, provisionally around Y `48`;
- guaranteed central dry area at least `16x16` blocks;
- principal shoreline within roughly `64..128` blocks of origin;
- center relief visibly above sea level but below extreme-mountain height;
- no secondary land outside the declared bounded support in v1.

### Materials and biomes

At minimum:

- bedrock at Y `0`;
- stone interior/floor;
- a shallow dirt layer and grass top on inland dry columns;
- sand around the waterline and on shallow submerged columns;
- water up to sea level in submerged columns;
- ocean biome for submerged/open-water columns;
- beach biome for the shoreline band;
- plains biome for inland dry columns.

Do not add trees, ores, caves, flowers, mobs, structures, loot, special blocks,
or bespoke biomes in the generator proof. Those would hide whether the
generator boundary and base terrain are correct.

### Seed and continuity invariants

- repeated generation of any target set is byte-identical;
- target order and batch partitioning do not affect results;
- adjacent chunks agree because they sample the same world-coordinate field;
- at least two pinned seeds have materially different shoreline/height
  fingerprints;
- positive and negative coordinate sampling follows the same floor/modulo
  rules;
- origin always contains the guaranteed spawn patch;
- chunks beyond the support radius contain only the specified ocean column
  family;
- native worker, Web Worker, save/reopen, and dedicated generation are exact
  for the same descriptor/seed/chunk.

## Spawn Ownership

Current spawn selection is overworld-shaped. Generator work must not add
profile tests to desktop, web, XR, or Android startup code.

Add or extend one shared server/worldgen spawn contract that can answer at
least:

- the preferred initial chunk/column for a new world;
- whether a generated column is acceptable;
- the final safe feet Y after the relevant chunk is available.

`Overworld` preserves its current biome-friendly search. `FlatGrassV1` returns
the origin plane. `SmallIslandV1` returns its guaranteed central patch.
`AuthoredOnly` retains its stored/authored safe-spawn scan. Product clients
consume the accepted server pose and do not reinterpret generator rules.

## Diagnostics and Accounting

Do not reuse overworld-only labels for all profiles.

Promote only the diagnostics needed to retain honest accounting:

- profile/version and seed on every worldgen job report;
- target and generator-dependency chunk counts;
- cache hit/miss/retained counts where a profile has a cache;
- generation, serialization, publication, and lighting timing kept separate;
- profile-specific optional counters rather than fake zero-valued
  `OverworldFeatureBatchTiming` fields;
- native/Web Worker descriptor-reset counts.

Existing performance parsers for `Overworld` must remain compatible or receive
an intentional schema-version change with lock tests.

## Implementation Slices

### Slice 0: Current-state corrections and executable boundary locks

Status: complete 2026-07-16.

- Correct `docs/structures.md` and `docs/worldgen-status.md` so retired
  TypeScript structures/features are not described as live native coverage.
- Add a focused topic for generator profiles and link this tactical.
- Lock legacy profile labels/discriminants, old metadata decode, and the
  "profile cannot change after scheduling" rule before expanding the enum.
- Add source/contract tests proving app crates do not generate chunks and web
  TypeScript does not interpret generator algorithms.
- Record current `Overworld` fixture hashes or exact focused tests before
  moving dispatch so refactoring cannot silently change it.

Gate: documentation states current truth, legacy world/profile fixtures are
locked, and the exact current overworld canaries pass before dispatch changes.

Execution record:

- corrected the structures and worldgen status docs and created the focused
  `world-generation-profiles` topic;
- pinned the exact legacy profile labels, JSON forms, and binary tags `0` and
  `1`, while retaining the existing v1 metadata migration and scheduler
  immutability tests;
- added a browser ownership lock proving the TypeScript job shell delegates
  opaque frames to the shared Rust `WorldgenJobSession` and contains no terrain
  implementation;
- passed the exact committed terrain fixture
  `fills_chunk_zero_zero_with_terrain_only_java_oracle` before introducing
  dispatch.

### Slice 1: Descriptor propagation and closed generator dispatch

Status: complete 2026-07-16.

- Reserve the new stable profile values without changing existing serialized
  meanings. Add each public enum value in the same slice as its working
  generator so no selectable intermediate profile has placeholder behavior.
- Carry the selected descriptor through scheduler jobs, native mailbox
  requests, Web Worker request/delta codecs, worker session resets, responses,
  and diagnostics.
- Introduce generator-specific planning/dispatch while wrapping the existing
  overworld cache unchanged.
- Keep `AuthoredOnly` on its current direct void-miss path.
- Separate generator dependency facts from light/publication facts.
- Make profile/seed changes reset all worker-resident generator state in
  native and browser sessions.

Gate: `Overworld` remains byte-exact on all committed oracle fixtures and has
no material target/dependency/performance regression; legacy catalog/world
records still open; native and Web Worker reset tests pass.

Execution record:

- added the immutable shared `WorldGenerationDescriptor { profile, seed }` and
  carried it through scheduler jobs, native messages, full/delta worker frames,
  responses, and public job diagnostics;
- bumped the internal worker-frame codec to v3 and made the closed dispatcher
  reject `authored-only`, which retains its direct persistence-miss path;
- made the resident Web Worker session adopt a descriptor only on reset and
  reject seed/profile changes without one; the WASM mailbox likewise forces a
  new mirror generation when its descriptor changes;
- kept the Overworld dependency cache and planning path unchanged. The full
  438-test server suite, descriptor/reset codec tests, exact seed-0 terrain
  fixture, and server/web WASM library checks pass.

### Slice 2: Flat grass generator

Status: complete 2026-07-16.

- Implement `FlatGrassV1` in `mclone-worldgen` with the exact layer/biome
  specification above.
- Produce canonical `GeneratedChunk` outputs directly at feature-complete
  status with no generator dependencies.
- Integrate profile-aware spawn selection.
- Add native scheduler, persistence miss/hit, save/reopen, dedicated, and web
  worker coverage.
- Expose the profile through developer CLI/startup inputs first; add product
  new-world selection only after the shared path is proven.
- Capture and inspect the first desktop full-frame result before expanding
  platform UI.

Gate: flat grass is selectable, playable, lit, persistent, byte-deterministic,
and native/web worker-equivalent, with target-only generation demonstrated by
diagnostics.

Execution record:

- added persisted label `flat-grass-v1` and binary tag `2` together with the
  working shared `mclone-worldgen` implementation;
- emits Y 0 bedrock, Y 1-2 dirt, Y 3 grass, plains biome ID 1 throughout, and
  no scheduled ticks at all coordinates, independent of seed;
- scheduler plans only requested targets, publishes through the standard
  feature/light/save path, and uses center-first plains spawn resolution at
  the origin; stored chunks still win and a mutated flat chunk survives
  unload/reopen without regeneration;
- full/delta job-frame order tests and the dedicated native TCP test exercise
  the same dispatcher used by the browser WASM adapter and remote clients;
- inspected `/tmp/mclone-flat-grass-v1-lit.png` at 1280x720 from an elevated
  oblique camera. Six independently published chunks joined without cracks;
  grass, two dirt layers, and bedrock were visibly correct. The capture used
  lighting and the normal shared renderer.

### Slice 3: Seeded small-island generator core

Status: complete 2026-07-16.

- Implement and document the exact v1 seeded world-coordinate field.
- Lock material, biome, sea-level, support-radius, and guaranteed-spawn
  behavior with focused unit/property tests.
- Add seam probes across X and Z boundaries, including negative coordinates
  and chunks straddling the shoreline/support edge.
- Pin at least two contrasting seeds using compact height/material/biome
  fingerprints rather than large screenshot fixtures.
- Reuse the same target-only generation path as flat grass; do not add
  decoration or a dependency neighborhood.
- Capture the first drawable island milestone, inspect it, then tune only the
  documented v1 constants before freezing fixtures.

Gate: one seed visibly produces a coherent grass/sand island in ocean, a
second produces a materially different but valid island, both guarantee safe
spawn, and all deterministic/seam/profile gates pass.

Frozen `small-island-v1` field:

- build range is `0..256`, sea level is Y `63`, and the exact open-ocean floor
  is Y `48`;
- three SplitMix64-hashed lattice value-noise domains use cubic smoothstep
  interpolation at scales `64`, `24`, and `32`;
- distorted radial distance is
  `radius - 20 * shore_noise - 7 * detail_noise`;
- the principal envelope is `smoothstep(clamp(1 - distance / 152, 0, 1))`;
- pre-spawn-blend surface height is rounded from
  `48 + 36 * envelope + 5 * relief_noise * envelope`;
- all columns at radial distance `>= 192` use the exact Y `48` floor;
- world X/Z `-8..8` forms an exact 16-by-16 grass spawn patch at Y `80`,
  blended into the radial field with smoothstep over the next `20` blocks;
- Y `59..=66` surface columns use four sand layers, higher dry columns use
  two dirt layers and grass, deeper columns use stone, and submerged columns
  fill with water through Y `63`;
- biome IDs are ocean `0` through surface Y `61`, beach `16` through Y `66`,
  and plains `1` above that.

Execution record:

- added stable label `small-island-v1` and binary tag `3` together with the
  shared-Rust generator, target-only scheduler dispatch, center-first safe
  spawn, and dedicated-server support;
- pinned seed fingerprints `14357595377438549354` for seed `12345` and
  `12105951125863982310` for seed `-98765`; repeated, reversed, and
  independently partitioned jobs are exact, while the two seeds differ;
- X and Z border tests cover positive and negative chunk seams; the bounded
  support, material/biome families, exact central patch, empty tick payloads,
  scheduler dependency counts, persistence hit/reopen, and remote TCP spawn
  are locked;
- inspected the normally lit elevated view
  `/tmp/mclone-small-island-v1-seed-12345-lit.png`: 421 streamed sections
  produced one coherent asymmetric island with continuous grass terraces,
  sand beach, shallow water, and ocean and no chunk or lighting seams;
- inspected `/tmp/mclone-small-island-v1-shoreline-lit.png` with its streaming
  center moved to the east shore: the grass/dirt, four-layer sand, stone
  shelf, and waterline remain continuous across independently published
  chunks;
- inspected `/tmp/mclone-small-island-v1-seed-neg98765.png`: the same camera
  and compatibility rules produce a materially different shoreline and
  relief field;
- fixed the offscreen diagnostic camera to reapply an explicit requested
  eye/target through the whole warmup; authoritative spawn reconciliation had
  previously replaced it before the final capture.

### Slice 4: Cross-platform/product selection and lifecycle proof

Status: complete 2026-07-17.

- Add shared display metadata for the three procedural choices without making
  apps own behavior.
- Wire local-world creation/catalog selection on desktop and browser, then
  reuse the shared startup contract on flat Android and XR where their menus
  expose world creation.
- Prove dedicated startup and remote clients preserve the server-selected
  profile; clients must not need the generator to join an already-generated
  remote world.
- Prove world replacement, realm/dimension reopen, independent dimensions,
  warm preview/observer startup, and deletion retain correct descriptor/seed
  identity.
- Add a profile/version line to relevant debug/loading surfaces.

Gate: every host creates or opens the same stored profile, remote clients are
generator-agnostic, and no platform contains a private profile-to-algorithm
switch.

Execution record:

- added shared procedural-profile display metadata and a stable
  Overworld/flat/island cycle to the catalog controller; desktop, browser,
  flat Android, and XR consume the same generator-agnostic UI action and
  catalog text instead of owning terrain switches;
- catalog and transient create requests now carry the selected profile, world
  rows show stored profile/version identity, and native SQLite catalog reopen
  retains `small-island-v1` exactly. The live browser catalog smoke creates a
  flat world, then an island world, reopens the flat world, and deletes the
  island without descriptor loss or a late recency update resurrecting it;
- made scene startup, replacement, managed preview/destination startup, native
  perf paths, and browser worker runtime configuration copy the selected
  profile before resolving the profile-owned spawn center;
- made browser IndexedDB startup adopt stored generation/behavior profiles
  before metadata validation. The real Worker smoke exposed and then locked
  this rule by creating and reloading `small-island-v1` through IndexedDB;
- added a realm test with a flat overworld dimension and an independently
  seeded island dimension; both load their own exact generated surface at the
  same chunk coordinate without descriptor leakage;
- added `GEN <stable profile>` to flat/XR diagnostics and inspected
  `/tmp/mclone-world-generation-profile-create-idle.png`, the flat Worker
  frame, and the post-reload island Worker frame. The latter reports
  `GEN SMALL-ISLAND-V1` at the guaranteed Y `82.62` spawn;
- `pnpm native:web:generator-smoke` passes a real flat local Worker run plus an
  island Worker/IndexedDB reload, and `pnpm native:web:catalog-smoke` passes the
  alternate-profile product flow; the existing dedicated remote test continues
  to prove clients consume authoritative chunks without generator logic;
- shared Rust/UI suites, WASM check/typecheck, native Android APK, and Quest XR
  APK build lanes pass after the startup and UI wiring changes.

### Slice 5: Fork-readiness cleanup

Status: planned; do only evidence-backed extraction.

- Inventory the remaining concrete `OverworldBiomeSource` and
  overworld-feature-table dependencies after flat/island land.
- Extract the smallest biome/rules contracts needed for a future
  `mclone-overworld-v1`; do not generalize code unused by a second caller.
- Keep current `overworld` oracle tests explicitly scoped to that profile.
- Define the first original-profile compatibility fixture vocabulary and seed
  domain without implementing custom biome content in this tactical.
- Write the follow-up tactical for the vanilla/mclone biome and decoration
  fork using the evidence from two real alternate generators.

Gate: adding a future mclone profile has a named owner and bounded seams, while
the current flat/island implementations remain simple and the overworld path
retains exact parity gates.

## Validation Matrix

### Focused Rust gates

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
```

Add focused tests for:

- legacy and new profile serialization/labels/discriminants;
- generator plan target/dependency sets;
- repeated, reordered, and differently batched generation;
- seed/profile worker reset;
- flat layers and biome payloads at positive/negative/far coordinates;
- island seam, support, material, biome, seed-contrast, and spawn invariants;
- persistence hit precedence and save/reopen identity;
- one realm with different profiles in distinct dimensions;
- unchanged committed overworld oracle fixtures.

### Web/WASM gates

```bash
cargo check --manifest-path native/Cargo.toml \
  -p mclone-worldgen -p mclone-server -p mclone-web-client \
  --target wasm32-unknown-unknown
pnpm native:web:build
```

Run browser worker smokes for flat and island after each profile becomes live.
They must exercise the real worker codec/session, not an inline test-only
generation call.

### Pixel gates

At the first drawable milestone for each generator, save captures under
`/tmp`, inspect them, and record the command/result in this tactical:

- flat grass at a low oblique camera with enough distance to expose seams;
- island from an elevated oblique view showing center, shoreline, and ocean;
- a shoreline close view checking material transitions and waterline;
- one contrasting island seed.

Pixel inspection is an integration gate, not the deterministic terrain oracle.

### Cross-system gates

Before closing product adoption:

- local integrated startup and replacement;
- dedicated server plus remote desktop client;
- browser local Web Worker and remote WebSocket paths;
- SQLite and IndexedDB save/reopen;
- profile-qualified multi-dimension runtime;
- no-window/offscreen startup to playable and idle;
- relevant flat Android/Android XR builds after shared startup/UI wiring changes.

## Non-Goals

- No dynamic/user-authored generator plugin ABI.
- No Mojang codec or registry-system port.
- No general superflat preset editor.
- No new block, biome, item, mob, or texture vocabulary.
- No custom biome placement, mclone overworld, or biome registry in the flat or
  island proof.
- No ordinary decoration, caves, ores, carvers, structures, loot, or scheduled
  terrain mutation in `SmallIslandV1`.
- No native structure-start/reference/template/jigsaw implementation.
- No claim that flat grass exactly matches every vanilla `FlatLevelSource`
  option.
- No change to the 1.17.1 oracle target for the existing `overworld` profile.
- No app-local generator logic and no TypeScript generator implementation.
- No generator-wide trait hierarchy added solely for aesthetic symmetry; enum
  dispatch is acceptable and preferred until a real third-party extension
  requirement exists.

## Stop Conditions

Stop and re-scope if any slice:

1. changes existing overworld oracle output without an independently explained
   parity correction;
2. requires app/platform code to understand terrain, biome, or material rules;
3. reuses retained worker state across different descriptors or seeds;
4. changes an existing stored discriminant/label or strands old worlds;
5. couples generator dependencies to lighting/publication dependencies;
6. makes remote clients run the server's generator to consume snapshots;
7. introduces dynamic registries before flat and island prove a concrete need;
8. adds island decoration before base terrain, seams, spawn, worker parity,
   persistence, and inspected pixels are green;
9. routes original-profile changes through the vanilla oracle profile;
10. describes retired TypeScript structure/feature work as live native code.

## Code and Documentation Map

Current native seams:

- `native/crates/mclone-server/src/world_generation_profile.rs`
- `native/crates/mclone-server/src/scheduler.rs`
- `native/crates/mclone-server/src/worldgen_mailbox.rs`
- `native/crates/mclone-server/src/job_codec.rs`
- `native/crates/mclone-server/src/dimension.rs`
- `native/crates/mclone-server/src/persistence.rs`
- `native/crates/mclone-server/src/spawn.rs`
- `native/crates/mclone-worldgen/src/biome.rs`
- `native/crates/mclone-worldgen/src/levelgen/generator.rs`
- `native/crates/mclone-worldgen/src/levelgen/feature_batch.rs`
- `native/crates/mclone-worldgen/src/feature/placed.rs`
- `native/crates/mclone-worldgen/src/carver.rs`
- `native/apps/mclone-web-client/src/web_server_worker.rs`
- `native/apps/mclone-web-client/www/mclone-server-job-worker.ts`

Related docs:

- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`../worldgen-status.md`](../worldgen-status.md)
- [`../reference-minecraft.md`](../reference-minecraft.md)
- [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md)
- [`../structures.md`](../structures.md)
- [`103-decorated-biome-fixture-matrix.md`](103-decorated-biome-fixture-matrix.md)
- [`135-overworld-biome-palette-matrix.md`](135-overworld-biome-palette-matrix.md)
- [`146-overworld-macro-terrain-geometry-parity.md`](146-overworld-macro-terrain-geometry-parity.md)
- [`175-live-hosted-world-diorama.md`](175-live-hosted-world-diorama.md)
- [`185-realm-dimension-and-observer-runtime.md`](185-realm-dimension-and-observer-runtime.md)
