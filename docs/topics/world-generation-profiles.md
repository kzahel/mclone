# World Generation Profiles

Topic: `world-generation-profiles`

Status: **Tacticals 187, 188, 191, 193, 194, and 257 are complete.
`flat-grass-v1`, `small-island-v1`, `topology-probe-v1`, `alpha-v1`,
`beta-v1`, and the
first `mclone-overworld-v1` terrain language are live, persisted shared-Rust
generators beside the legacy Java-shaped Overworld; authored-only misses still produce
void. Small Island now
exercises the reusable value-noise primitive, typed scheduler/worker request
contract, dependency cache, mutable feature region, and a real cross-chunk
decoration stage.
Shared catalog/UI selection and desktop, browser, Android, XR, dedicated,
multi-dimension, and stored-reopen paths carry the same profile contract.
Generator-owned pure plans now declare exact outputs, backend work, and typed
prerequisites while the scheduler retains readiness, priority, admission,
publication, lighting, and persistence. Flat Grass now also supports finite
bounds and the first X-periodic cylinder; its generation plans retain coherent
target-relative work lifts while deduplicating canonical seam identities. The
alternate profiles remain
internal and unshipped, so their current names, tags, and fixtures are
regression guards rather than release compatibility promises. Tactical 188
completed two terrain reviews, one relief tune, one flower-density tune, two
reuse/refactor checkpoints, and host plus persistence closeout. Tactical 192
owns the next mountain/valley family. The creative terrain direction lives in
[`mclone-overworld-generation.md`](mclone-overworld-generation.md). Tactical
[`328`](../tactical/328-mclone-overworld-v2-regional-breadth.md) allocates a
new internal-unshipped `mclone-overworld-v2` sibling for the continental
candidate while preserving V1 as selectable and default. V2 is now live as
an explicitly experimental unbounded-plane profile with stored binary tag 9;
its mesa-desert and humid-jungle characteristic-domain slices are implemented
beside the retained temperate catchment. Native packaged live-game review and
shared/native/Wasm/Android/XR build boundaries are complete. Human Review E is
pending with the exact/procedural appearance frontier recorded as a revision
candidate. Tactical
[`329`](../tactical/329-v2-quest-performance-and-forest-continuity.md) moves
native CPU-authored V2 horizon compilation off the render thread, restores
complete Quest Low/RD8 cold settle to V1-scale timing, and adds a V2-only
fixed-budget coarse canopy. Browser terrain compilation and transient render
tails remain performance debts. The ordinary browser New World flow now has
an explicit V1-to-V2 handoff gate: the persisted V2 descriptor and active seed
must agree, the entry column must be loaded, and the settled V2 frame must
contain generated pixels without any render errors.**

This topic owns the current truth and durable decisions for selectable,
versioned world-generation profiles. Detailed refactoring and implementation
order lives in
[`187-generator-profile-flat-grass-and-seeded-island.md`](../tactical/187-generator-profile-flat-grass-and-seeded-island.md).

## Product Direction

Continue building Mclone Overworld as the original product world: its own
terrain, biome combinations, decoration, ecology, structures, landmarks, and
gameplay. It is already the product default; there is no future parity fork to
wait for. Minecraft Java 1.17.1 and the stored `overworld` profile are optional
comparative/legacy surfaces, not a reference lock on product work.

The accepted continental/ecoregional direction lives in
[`continental-ecoregion-planning.md`](continental-ecoregion-planning.md).
Its first exploration may change `mclone-overworld-v1` substantially. If
continental, province, ecoregion, landscape-mosaic, or migration scales become
world-selectable rather than one profile revision's constants, introduce a
persisted world-scale descriptor here before production depends on them. A
world-scale choice is generation identity, not an LOD or client preference.

The first alternate generators are intentionally smaller:

- flat grass proves a seed-independent, target-only procedural generator;
- seeded small island proves original, nontrivial, position-dependent terrain,
  shoreline continuity, biome output, guaranteed spawn, and neighbor-aware
  decoration;
- mclone overworld now proves unbounded continuous terrain, a deliberately
  small biome/surface language, and dependency-bearing vegetation through the
  same host contracts as the other profiles;
- the hidden topology probe proves adversarial plane, cylinder, and flat-torus
  seams through generation, worker, simulation, lighting, persistence, and
  native presentation without entering normal world creation; and
- Alpha and Beta prove that historical generation families can remain
  standalone siblings while sharing neutral chunks, planning, feature-region,
  persistence, and host contracts.

## Current Truth

The stored server-owned `WorldGenerationProfile` has nine values:

- `Overworld`: legacy procedural Java-1.17-shaped generation, retained as an
  internal development/reference selection rather than the product default;
- `FlatGrassV1`: exact bedrock/dirt/dirt/grass layers with plains biomes and no
  decoration, ticks, or generator neighbors;
- `SmallIslandV1`: a bounded original seeded radial/noise field with a safe
  central grass patch, sand shoreline, ocean, and a dependency-bearing
  oak/grass/flower feature stage;
- `McloneOverworldV1`: continuous original ocean, coast, grass lowland, and
  wooded rolling upland terrain from inspectable profile-owned fields, with
  gravel/sand/grass/stone surface recipes and a profile-owned oak, grass, and
  occasional-flower decoration language;
- `McloneOverworldV2`: an experimental unbounded continental profile with a
  retained temperate catchment plus mesa-desert and humid-jungle regional
  archetypes, generated from one exact/procedural surface source while V1
  remains available and default;
- `TopologyProbeV1`: a hidden adversarial plane/cylinder/torus conformance
  generator that guarantees terrain, water, bounded geometry, material,
  lighting, and persistence canaries at canonical seams;
- `AlphaV1 { winter }`: standalone Alpha v1.1.2_01-shaped density terrain,
  surface, caves, and compact deterministic population, with temperate and
  whole-world winter selections;
- `BetaV1`: standalone Beta 1.7.3 climate/biome terrain, exact staged surface
  and caves, and a deterministic Beta-flavored population subset;
- `AuthoredOnly`: persistence-backed content whose true misses become void.

The profile already crosses world catalogs, realm/dimension metadata,
integrated and dedicated startup, native and browser hosts, and persistence.
It is fixed before chunk scheduling starts.

Product world creation defaults to Mclone Overworld V1 and cycles eight
procedural selections—V1, experimental V2, legacy Overworld, Flat Grass,
Small Island, Alpha temperate, Alpha winter, and Beta—through shared catalog
policy and generator-agnostic UI text. Mclone
Overworld is the product creation default across catalog, startup, dedicated,
and integrated-runner entry points. Catalog records require an explicit
profile; the internal-unshipped project carries no compatibility policy for
obsolete saves. The hidden topology probe is available only through explicit
developer/test configuration and is not part of that cycle. Scene replacement,
warm-world startup, managed previews, and all host adapters copy the selected
descriptor. Native and Web local startup now consume one shared
profile-aware launch plan. Tactical
[`310`](../tactical/310-shared-local-session-launch-semantics.md) removed the
browser's inherited-center lowering, preserved signed 64-bit catalog seeds,
and locked the untouched default-Mclone-Wild menu flow at the same requested
and authority-accepted entry center as native. Native SQLite and browser
IndexedDB reopen preserve the profile; the browser Worker applies stored
metadata profiles before validating or scheduling the world.

A 2026-08-22 live New World regression exposed a separate presentation
lifecycle requirement. Creating V2 while a V1 world was active correctly
replaced exact chunks and the world descriptor, but attempted to reuse the V1
procedural-horizon renderer. Its source-identity check rejected the V2 exact
coverage, leaving a black scene. Procedural profile is now a construction-time
renderer fact: same-profile world and seed changes may reset in place, while a
V1/V2 profile change rebuilds the shared terrain-view engine before the
destination coverage is composed. The engine boundary rejects any silent
cross-profile reuse.

Scheduler and worker requests carry an immutable profile-plus-seed descriptor
through native messages, WASM codecs, responses, and diagnostics. The closed
shared-Rust executor selects the legacy Overworld cache, flat grass, the
Small Island, Mclone Overworld, Alpha, or Beta cache, and resident state resets
when either descriptor fact changes.

Transient native scene startup now builds its `SessionStartRequest` with the
selected profile instead of describing every direct world as Overworld. Local
periodic startup also preflights the exact tracking and unload-hysteresis view
against the topology, so an undersized period fails synchronously rather than
terminating a background runner and leaving startup pending.

`WorldGenerationProfile::plan_features` is the single closed planning entry.
It returns a deterministic `ChunkGenerationPlan` containing exact requested
outputs, generator backend-work chunks, and typed chunk/status prerequisites.
Overworld, Small Island, Mclone Overworld, Alpha, and Beta declare a 3x3
feature-center and 5x5 mutable Surface dependency footprint for one target;
the Topology Probe declares a one-chunk Surface input ring for its lighting
canary; Flat Grass declares target-only work. The scheduler consumes every
prerequisite generically, then applies its own view priority, deduplication,
job admission, and publication policy. Planning occurs once for scheduler
admission and is recomputed once by the worker as request validation; it adds
no worker round trip, trait-object dispatch, or general graph traversal and
never runs per poll or publication.

The scheduler/worker seam now has two explicit request layers:

- `GenerationPlanRequest` carries the immutable descriptor and exact requested
  outputs and deterministically returns the generator-owned plan;
- `GenerationExecutionRequest` carries that same plan request plus
  `GenerationInput` values, each pairing its exact
  `ChunkStatusRequirement` with a typed `ChunkBlocks` artifact.

The scheduler uses the first request for readiness and ordering, then sends the
second request over native messages or the WASM frame codec. The worker
recomputes the declared plan and rejects undeclared or duplicate typed inputs
before execution. Thus a bare chunk buffer can no longer silently lose the
status it is intended to satisfy while crossing the worker boundary. Missing
declared inputs remain legal cache misses and are generated deterministically.

Generator implementations remain concrete behind that contract:

- dependency-bearing profiles publish one generic cache report; detailed
  `OverworldFeatureBatchTiming` remains optional and Overworld-only;
- resident worker state contains separate Overworld, Small Island, Mclone,
  Alpha, and Beta caches, while the probe remains stateless;
- surface, carver, feature-biome, and feature-table internals remain specific
  to `OverworldBiomeSource` and the current Overworld case.

Small Island's terrain fields use public `SeedDomain` and `ValueNoise2d`
building blocks. The sampler is pinned across negative and positive absolute
coordinates, uses Euclidean lattice coordinates, and separates fields by
stable domains so adding one field need not perturb another. Decoration uses a
separate derived seed domain and a small profile-owned feature table while
reusing the existing `PlacedFeature`, `ConfiguredFeature`, and `FeatureRegion`
execution machinery.

The generic `NoiseBiomeSource` used by terrain sampling is only a partial seam.
Flat grass and seeded island intentionally bypass that machinery. Authored
island/table fixtures remain a separate persistence-backed content path.

There is now a minimal live native true-structure foundation: Tactical 274
Slice 1 added status-owned starts, references, persisted pieces/bounds, and
target-clipped placement for one opt-in original canary. It is not a vanilla
structure-family port. The old buried treasure, desert well, monster room, and
fossil history belonged to the retired TypeScript engine; current structure
docs must continue to distinguish that history from native status.

## Compatibility Safety Ledger

This is the authoritative metadata for deciding whether an intentional
world-generation change is safe. Review it before preserving an algorithm,
adding a new versioned profile, changing a fixture, or migrating a stored
world.

- **Reviewed:** 2026-08-10 at Tactical 274 Human Review R1; no freeze trigger
  or preservation consumer was added
- **Reviewed for Tactical 284:** 2026-08-12; the forest-edge query is derived
  from existing `mclone-overworld-v1` landform and vegetation intent and does
  not change generated blocks, fingerprints, or profile identity
- **Reviewed for Tactical 291:** 2026-08-13; pure profile output and
  fingerprints are unchanged. The `intro-homestead-v1` overlay intentionally
  replaces its disposable internal plan checksum with `5ea0fc55...` because
  `garden-v1` now projects the promoted working kitchen-garden record. Existing
  internal overlay worlds are disposable or require explicit rebuild.
- **Reviewed for Tactical 298:** 2026-08-14; Mclone wildlife spawn policy is
  explicitly mutable under the existing `mclone-overworld-v1` row. Terrain
  blocks and fingerprints are unchanged, while never-realized entity chunks
  now receive revision-1 seed-addressed rabbit, deer, mallard, or bee groups.
  Existing entity records, including empty records, remain authoritative;
  disposable internal worlds may be regenerated for a complete new
  population.
- **Reviewed for Tactical 301:** 2026-08-15; generated blocks, terrain
  fingerprints, and initial wildlife geography are unchanged. The internal
  wildlife resource saved-data codec intentionally advances from its scalar
  proof to revision 2 fixed typed strata and rejects obsolete scalar records;
  disposable development worlds regenerate them from current terrain. Entity
  saved data advances to revision 15 for mallard lifecycle and nest intent,
  with focused legacy mallard hydration and round-trip coverage.
- **Reviewed for default creation and inland spawn, 2026-08-15:** Mclone
  Overworld is now the product new-world default; obsolete catalog records
  without a profile are rejected rather than migrated. Mclone spawn selection
  now requires a dry grass candidate inside a land-dominant 129-by-129-block
  survey, and live admission resolves biome top material through the selected
  profile. Terrain blocks, field/decor fingerprints, and profile identity are
  unchanged; internal worlds remain disposable. The accepted
  `intro-homestead-v1` scout origin and realized seed-`0` plan checksum remain
  unchanged.
- **Reviewed for original-product direction, 2026-08-15:** Minecraft Java
  1.17.1 parity is no longer a project target. Mclone Overworld remains the
  product default, and legacy `overworld` joins the other internal-unshipped
  profiles as mutable rather than reference-locked. Existing oracle and
  regression fixtures remain useful evidence only when a task explicitly
  places that legacy surface in scope.
- **Project release state:** `internal-unshipped`
- **Known external world/save consumers:** none
- **Default fixture meaning:** refactor and determinism regression guard, not a
  release compatibility promise
- **Default internal-world policy:** disposable or explicitly migrated when an
  intentional generator change lands

Dispositions mean:

- `internal-mutable`: intentional output and identity changes are allowed in
  place because no shipped consumer depends on them;
- `planned-unallocated`: the identity is not live and may be redesigned before
  implementation.

| Surface | Disposition | Safe intentional changes | Why | Required update when changed |
|---|---|---|---|---|
| `overworld` | `internal-mutable` | Maintenance, output changes, identity changes, or retirement may proceed for an explicit project need | It is a legacy Java-shaped development/reference profile with no shipped consumers and no active parity target | Update affected fixtures/tests/docs and discard or explicitly migrate affected internal worlds; run oracle checks only when the scoped change still claims a Minecraft fact |
| `flat-grass-v1` | `internal-mutable` | Layers, biome, seed use, label, tag, planning shape, and implementation may change in place | It is an internal proof generator with no shipped worlds or external consumers | Update focused fixtures/tests/docs and discard or migrate affected internal worlds |
| `small-island-v1` | `internal-mutable` | Noise, terrain shape, materials, biomes, spawn, decoration, dependencies, label, tag, and implementation may change in place | It is an internal proving ground; current fingerprints protect accidental drift but do not prohibit intentional improvement | Update fingerprints, seam/order tests, captures, docs, and discard or migrate affected internal worlds |
| `authored-only` missing-void behavior | `internal-mutable` | Missing-chunk semantics and identity may change after auditing authored scenarios | No shipped consumer exists, although lobby/preview fixtures rely on the current void contract | Update persistence, embedded-world, catalog, and no-worldgen scenario coverage together |
| `mclone-overworld-v1` | `internal-mutable` | Identity, tag, fields, seed domains, world-scale facts, supported topology periods, terrain, biome/surface/decoration rules, spawn, ecology inputs, dependency plan, fixtures, and implementation may change in place | It is live only in internal builds; no shipped or named retained world requires current output | Update fingerprints, field/ecoregion maps, cards, tests, docs, and discard or explicitly migrate affected internal worlds |
| `mclone-overworld-v2` | `internal-mutable` | Identity, binary tag 9, continental rules, regional archetypes, topology support, performance policy, fixtures, and output may change in place | It is a selectable experimental sibling so V1's current charm and cost remain available while the continental generator matures; no shipped or named retained world requires current V2 output | Update shared native/Web codecs, exact/LOD witnesses, spawn and SQLite/IndexedDB reopen tests, captures, docs, and discard or explicitly migrate affected internal V2 worlds; keep V1 selectable and default unless a later review decides otherwise |
| `intro-homestead-v1` starter overlay | `internal-mutable` | Starter descriptor, scout revision, fit thresholds, score ordering, plan schema, promoted content, and materialization may change before a release freeze | It is an explicit identity orthogonal to the base profile. The seed-`0` compact composition is internally accepted and Tactical 291 intentionally replaces its decorative garden with working promoted content, but no shipped consumer requires the prior checksum | Keep pure-base fingerprints unchanged; update scout/plan witnesses, promoted-content and clipped-placement tests, review maps, starter/plan codecs, and discard or explicitly migrate affected internal overlay worlds |
| `topology-probe-v1` | `internal-mutable` | Identity, binary tag 8, minimum period, diagnostic terrain, plan geometry, materials, and fixtures may change in place | It is a hidden executable conformance instrument with no shipped or named retained world | Update exact conformance fixtures, worker/persistence tests, tactical evidence, and discard affected internal probe worlds |
| `alpha-v1` | `internal-mutable` | Profile shape, winter option, feature subset, planning shape, fixtures, and output may change while preserving or explicitly revising the documented Alpha flavor/parity boundary | It is live only in internal builds; no shipped or named retained world requires current output. Alpha v1.1.2_01 stage receipts constrain the close-parity core but do not make the whole profile a historical compatibility promise | Re-run the Alpha oracle hashes, mapping/order tests, scheduler/worker/persistence tests, temperate and winter captures, workspace tests, and web build; update fixtures/docs and discard or explicitly migrate affected internal worlds |
| `beta-v1` | `internal-mutable` | Identity, binary tag 7, parity boundary, population subset, planning shape, fixtures, and output may change while preserving or explicitly revising the documented Beta flavor/parity boundary | It is live only in internal builds; no shipped or named retained world requires current output. Beta 1.7.3 staged receipts constrain the close-parity climate/terrain/surface/cave core but do not make the whole profile a historical compatibility promise | Re-run the Beta oracle hashes, mapping/order tests, scheduler/worker/persistence tests, captures, workspace tests, and web build; update fixtures/docs and discard or explicitly migrate affected internal worlds |

For a proposed change, resolve every affected row before editing. The most
restrictive disposition wins when a shared primitive affects multiple rows. If
the affected surface has no row, add one with its concrete preservation
consumer—or state that none exists—rather than inferring safety from its name
or tests. Classify fixtures as regression guards or compatibility evidence,
then list the exact migrations and validation that an intentional change must
carry.

An immutable profile-plus-seed descriptor during a worker/scheduler session is
a runtime consistency rule, not a promise that a later build must preserve the
same algorithm.

A surface becomes release-frozen only when at least one concrete preservation
consumer is recorded here, for example:

- a distributed build whose users are expected to reopen generated worlds;
- a named family/test/server world that the user asks to retain across builds;
- an external fixture, tool, or protocol consumer that depends on the exact
  identity or output; or
- an explicit release/compatibility declaration.

When one of those triggers occurs, update this ledger first with the consumer,
freeze boundary, migration policy, and earliest compatible version. Do not
infer a freeze merely from a `v1` suffix, persisted discriminant, or committed
fingerprint.

## Binding Decisions

1. The existing stored `overworld` identity and serialized discriminant name a
   legacy Java-shaped path. They are not a parity promise or evidence of
   shipped save consumers and may be changed or retired under the safety
   ledger when an explicit need justifies it.
2. `flat-grass-v1` and `small-island-v1` are current internal identities, not
   release freezes. They may change in place under the safety ledger while no
   preservation consumer exists.
3. The product default uses `mclone-overworld-v1` so Mclone's original world
   remains distinct from legacy `overworld`; its exact compatibility promise
   begins only when the safety ledger records a concrete preservation consumer
   and freeze.
4. Persistence hits win for every profile. Profiles govern only what a true
   missing chunk produces.
5. Generator dependency footprints are separate from lighting and publication
   dependencies.
6. Shared Rust owns all algorithms. App crates select/display profiles, and
   browser TypeScript transports descriptors without interpreting them.
7. Closed enum dispatch is sufficient. Dynamic generator plugins and Mojang's
   codec/registry framework are deferred until a real requirement exists.
8. Flat and island initially emit existing biome IDs. Original biome registry
   work waits for the actual mclone fork.
9. Generator identity is stored per dimension and remains fixed during a live
   scheduling session. Cross-build preservation is required only for worlds
   recorded as compatibility consumers in the safety ledger.
10. Remote clients consume authoritative chunks and do not need the server's
    generator implementation.

## First Generator Proofs

### Flat grass v1

- Y `0`: bedrock
- Y `1..=2`: dirt
- Y `3`: grass
- above: air
- plains biome throughout
- no carvers, features, structures, or ticks
- origin spawn on the surface
- requested chunks are the entire generation dependency set

### Seeded small island v1

- original mclone seed domain and world-coordinate height field
- bounded distorted radial island centered at origin
- guaranteed dry, reasonably level central spawn patch
- stone/dirt/grass interior, sand shoreline, ocean floor and water
- existing plains/beach/ocean biome IDs
- oak trees, grass patches, dandelions, and poppies through the shared placed
  feature machinery
- 3x3 feature-center work and 5x5 Surface prerequisites for one target, with
  cache reuse and cross-chunk writes
- no caves, carvers, or structures
- exact seam, request-order, and partition determinism
- at least two pinned seeds with materially different valid islands

The exact island constants and formulas are recorded in Tactical 187's Slice 3
execution record. Two pinned seeds, X/Z seam checks, partition invariance,
save/reopen, dedicated spawn, and inspected overview/shoreline captures form a
strong accidental-regression baseline. They may be intentionally updated under
the safety ledger.

### Mclone overworld v1 terrain foundation

- independent stable continentalness and relief seed domains;
- pure absolute-coordinate point samples and bounded row-major region samples
  through the production sampler;
- continuous ocean, sand coast, grass lowland, and rolling upland terrain;
- existing ocean, beach, plains, and forest biome IDs with canonical
  heightmaps and empty ticks;
- profile-owned gravel, sand, grass/soil, and exposed-stone surface recipes;
- deterministic dry-upland spawn search;
- independent decoration domain and Mclone-owned oak, grass, and occasional
  flower tables through shared placed-feature and region execution;
- exact typed 3-by-3 feature work and 5-by-5 Surface prerequisites, with no
  terrain knowledge in scheduler or TypeScript;
- exact field, seam, negative-coordinate, request-order, partition, worker
  codec, and different-seed regression locks;
- no carvers, caves, mountains, rivers, climate fields, or structures yet.

These locks guard accidental drift while the profile is internal-mutable. The
first multi-seed/region field-map and landscape-card review accepted the macro
terrain after adding one fine relief octave to break up concentric local
contours. The second added biome/surface maps, accepted the first recognizable
terrain language, and changed flowers from every land chunk to occasional
patches. The next Tactical 188 gate compares all three procedural callers
before extracting any more shared mechanism.

### Reusable visual review card

`pnpm native:worldgen:card --seed 12345` renders three fixed views from one
fully warmed transient world: near-vertical top-down, a low landscape, and an
elevated opposing landscape. The chunk interest remains fixed on the requested
`--chunk-x`/`--chunk-z` center while detached diagnostic cameras render from
above or outside that region. It writes the labeled comparison card, the three
source PNGs, and a schema-versioned JSON receipt under
`/tmp/mclone-worldgen-showcase`. Output names include the profile, seed, and
interest-center chunk, so another seed or region accumulates a directly
comparable card instead of overwriting prior evidence.

The command defaults to `small-island-v1`, minimum render distance 16, frozen
daytime, disabled passive showcase actors, disabled lighting, and fullbright
rendering. Radius 16 covers the island's 192-block support radius plus 64
blocks of surrounding water on every axis. Smaller requested render distances
are raised to 16; this is also the current shared scene maximum, so larger
values are rejected by normal scene validation. Arguments appended to the
package command override seed, profile, dimensions, center, time, and other
rendering options. The underlying
`--worldgen-showcase-card <directory>` mode rejects remote and persistent worlds
so stored chunks cannot silently contaminate a generator review.

Before rendering any view, the capture requires the complete 35-by-35 tracked
region to be client-visible and ready, and requires all work for the 33-by-33
render target to be compiled and uploaded. Feature-dependency work outside that
capture target may still exist; the receipt reports it separately instead of
mistaking it for incomplete visible terrain. The receipt also records the fixed
interest center, coverage radius, warmup cost, camera poses/lenses, and per-view
drawn-section counts, commit, and dirty state. Its profile-aware coverage
metadata distinguishes the bounded Small Island support/water margin from
unbounded profiles. These cards are visual review evidence under the safety
ledger, not pixel-locked compatibility fixtures.

## Architecture Direction

```text
stored DimensionDefinition
  -> seed + stable generation profile/version
  -> persistence hit or profile-selected miss handling
  -> generator-specific plan and worker session
  -> canonical GeneratedChunk
  -> shared lighting, publication, persistence, and runtime mutation
```

The current Overworld cache is one dispatch case rather than the worker
protocol itself. Flat proves the zero-neighbor case; Small Island proves a
second dependency-bearing cache and feature-table caller. Future structure
profiles may request broader dependencies or new typed artifacts without
changing holder, light, publication, or client contracts.

Tactical
[`191`](../tactical/191-guarded-generation-planning-refactor.md) completed the
move of the existing Overworld footprint calculation to one generator-owned
pure plan. Exact schedule locks, host A/B probes, browser generator smokes, and
Android AVD canaries found no output, work, pacing, or policy regression. The
result is Quest-proxy-clean; physical Quest RD5 confirmation remains pending
and AVD evidence is not a substitute for it.

Spawn policy becomes generator-aware inside shared server/worldgen ownership.
Desktop, web, Android, and XR accept the authoritative spawn rather than
adding profile branches.

### Topology sequencing and support

World topology and generation profile remain separate dimension facts, but a
profile/topology pair must be explicitly supported. Tactical
[`195`](../tactical/195-periodic-cylinder-topology-proof.md) completed the
identity-topology baseline, finite-bound canary, and Flat Grass cylinder.
Tactical [`196`](../tactical/196-periodic-mclone-terrain-fields.md) completed
the first genuine periodic Mclone sampler after Tactical 192's accepted
mountain/valley fields.

Flat Grass explicitly admits finite and periodic-X topology, while Authored
Only admits fixture use. Mclone Overworld admits the plane and exactly
`cylinder-x:384`; different cylinder periods and finite axes remain explicit
errors. The hidden Topology Probe admits the unbounded plane, X-periodic
cylinders of at least eight chunks, and flat tori whose axes are each at least
eight chunks; it rejects finite axes and Z-only periodic worlds. Reference
Overworld, Small Island, Alpha, and Beta still reject
bounded or periodic topology during dimension registration. Canonical runtime
wrapping alone does not make a planar field periodic.

The 384-chunk/6,144-block Mclone cylinder is a conformance proof rather than a
continental size target. The ordinary product default remains the unbounded
plane. Preserve small cylinders for probes or deliberately compact profiles;
review larger continental and ecological scales before selecting any serious
large-cylinder period. Merely accepting a larger topology value is
insufficient: the stored profile/world-scale identity, periodic fields,
plans, habitat networks, features, summaries, and fixtures must all agree.

Current `mclone-overworld-v1` support is:

| Family | Plane | Finite | Cylinder | Torus | Cube atlas |
|---|---|---|---|---|---|
| Terrain fields | supported | unsupported | supported at 384 chunks | pending | design only |
| Features | supported | unsupported | supported at 384 chunks | pending | design only |
| Regional climate | periodic temperature/moisture, altitude cooling, and five land recipes | unsupported | same exact periodic fields at 384 chunks | pending | design only |
| Rivers/hydrology | size-aware ocean bathymetry, flat Y63 major rivers, and bounded Y67 source/tributary/fall/sink landmarks | unsupported | same periodic fields and fixed local stencils at 384 chunks | pending | design only |
| Mclone caves | absent | - | - | - | - |
| Mclone structures | one bounded valley-stream start/piece family | unsupported | exact periodic start/piece realization at 384 chunks | pending | design only |

Future Mclone content tacticals must classify each added family as
topology-neutral, plane/finite only, periodic-axis compatible, patch-atlas
compatible, or explicitly unsupported. This does not require every feature to
support every topology immediately; it prevents raw planar assumptions from
remaining invisible as the content surface grows.

## Acceptance Themes

- exact current world/profile decode;
- affected legacy-overworld regression fixtures, with oracle comparison only
  when the scoped work makes a Minecraft-behavior claim;
- deterministic output independent of request order/batching;
- worker rejection of undeclared or duplicate typed inputs;
- explicit profile/seed reset of worker-resident state;
- native and Web Worker equivalence;
- SQLite and IndexedDB save/reopen equivalence;
- safe spawn for every profile;
- a land-dominant Mclone spawn neighborhood for the untouched seed and the
  pinned review seeds;
- inspected flat, island, and Mclone desktop captures;
- profile identity visible in useful diagnostics;
- no app-local or TypeScript terrain implementation.

## Next Work

Tactical 188 is complete. Creation/catalog display, SQLite and IndexedDB
reopen, native and browser workers, dedicated/remote authority, independent
dimensions, and mono/stereo warm replacement all carry the Mclone descriptor
through existing shared contracts. The two checkpoints shared only the
output-identical columnar biome payload traversal and the Small Island/Mclone
Surface dependency-cache lifecycle; profile rule composition remains
concrete. The accepted terrain and reuse direction lives in
[`mclone-overworld-generation.md`](mclone-overworld-generation.md), and
[`Tactical 192`](../tactical/192-mclone-overworld-mountains-and-valleys.md),
[`195`](../tactical/195-periodic-cylinder-topology-proof.md), and
[`196`](../tactical/196-periodic-mclone-terrain-fields.md) are complete.

Human Review 1 rejected the first bounded rivers/wetlands hydraulic
realization because pointwise terrain-relative water height permitted
transverse slopes and uncontained source faces. Human Review 2 then rejected
field revision 8's one-level lowland language and shallow ocean floors. Field
revision 9 adds size-aware shelf/deep-basin bathymetry. Revision 10 proved a
bounded lip/fall/pool stencil but interactive review rejected its locally
quantized major corridor because it can descend and later rise. Revision 11
keeps major rivers hydrostatic at Y63 and uses the stable drop only in a short
Y67 source-pool/tributary/fall/Y63-river landmark. Authoritative wakes and a
waterfall-centered movement soak produce no fluid mutations or generated
fluid-tick tail. Human review now decides whether this compromise is visually
sufficient. General highland rivers, macro drainage identity, global
monotonicity, and one integrated river-to-deep-basin outlet remain open.
Production chunk-time simulation remains excluded.
True structure infrastructure remains ready as a separate concern but is
parked while this terrain campaign advances.

True native structure infrastructure and original mclone structures remain a
separate follow-up. Before adding it, extend the typed artifact vocabulary for
whatever structure metadata actually crosses planning/execution; do not encode
that metadata as an untyped chunk-buffer side channel.

## Related

- [`../worldgen-status.md`](../worldgen-status.md)
- [`../reference-minecraft.md`](../reference-minecraft.md)
- [`../structures.md`](../structures.md)
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md)
- [`embedded-worlds.md`](embedded-worlds.md)
- [`realm-dimension-runtime.md`](realm-dimension-runtime.md)
- [`../tactical/103-decorated-biome-fixture-matrix.md`](../tactical/103-decorated-biome-fixture-matrix.md)
- [`../tactical/135-overworld-biome-palette-matrix.md`](../tactical/135-overworld-biome-palette-matrix.md)
- [`../tactical/146-overworld-macro-terrain-geometry-parity.md`](../tactical/146-overworld-macro-terrain-geometry-parity.md)
- [`../tactical/188-mclone-overworld-v1-terrain-foundation.md`](../tactical/188-mclone-overworld-v1-terrain-foundation.md)
- [`../tactical/191-guarded-generation-planning-refactor.md`](../tactical/191-guarded-generation-planning-refactor.md)
- [`../tactical/192-mclone-overworld-mountains-and-valleys.md`](../tactical/192-mclone-overworld-mountains-and-valleys.md)
- [`../tactical/195-periodic-cylinder-topology-proof.md`](../tactical/195-periodic-cylinder-topology-proof.md)
- [`../tactical/196-periodic-mclone-terrain-fields.md`](../tactical/196-periodic-mclone-terrain-fields.md)
