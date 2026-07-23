# Worldgen Status

Living status page for Rust world generation. Live procedural profiles
are the Minecraft Java 1.17.1-shaped overworld, the deliberately minimal
`flat-grass-v1` proof generator, the seeded `small-island-v1` proof generator,
and the first continuous `mclone-overworld-v1` terrain caller.

The current `overworld` profile's durable target is seed parity against vanilla
1.17.1 overworld output. The accepted follow-up direction is to preserve that
profile while adding versioned flat-grass, seeded-island, and later original
mclone generation profiles. See
[`topics/world-generation-profiles.md`](topics/world-generation-profiles.md)
and
[`tactical/187-generator-profile-flat-grass-and-seeded-island.md`](tactical/187-generator-profile-flat-grass-and-seeded-island.md).
The original terrain direction lives in
[`topics/mclone-overworld-generation.md`](topics/mclone-overworld-generation.md),
its first bounded foundation is complete in
[`tactical/188-mclone-overworld-v1-terrain-foundation.md`](tactical/188-mclone-overworld-v1-terrain-foundation.md),
and mountains/valleys continue in
[`tactical/192-mclone-overworld-mountains-and-valleys.md`](tactical/192-mclone-overworld-mountains-and-valleys.md).
The finite/periodic runtime proof is complete in
[`tactical/195-periodic-cylinder-topology-proof.md`](tactical/195-periodic-cylinder-topology-proof.md);
genuinely periodic Mclone fields and features are complete in
[`tactical/196-periodic-mclone-terrain-fields.md`](tactical/196-periodic-mclone-terrain-fields.md)
for the plane and exact 384-chunk X cylinder.
The selected campaign has now advanced through mountains/valleys, periodic
fields, rivers/wetlands, a coherent bounded stream, and the first climate
regions. Three-dimensional geology and rock formations are the recommended
next terrain phase before broad caves.
The active water slice is
[`tactical/220-mclone-overworld-rivers-and-wetlands.md`](tactical/220-mclone-overworld-rivers-and-wetlands.md).
Its broad periodic river corridor, terrain carving, banks, local water levels,
river biome/substrate, and sparse shallow wetland pools are live, but Human
Review 1 rejected the pointwise water surface on 2026-07-23. Transverse slopes
and uncontained source faces required a flat contained-reach correction.
Field revision 8 now limits water to constant-Y63 lowland reaches and passes
hydraulic closure, authoritative fluid wake, RD16 pixel, cold/warm generation,
and accelerated one-minute movement evidence. Human Review 2 rejected the
world language: one global river level produces no visible drops and abruptly
vanishes at the lowland gate, while large bodies lack shelf/deep-basin
bathymetry. Field revision 9 now adds size-aware shelf/deep-basin floors and
field revision 10 added local four-block flat reaches joined by a bounded
baked lip/fall/pool stencil. Interactive review rejected that corridor because
its locally downhill steps may later rise. Field revision 11 instead keeps
major rivers at Y63 and permits raised water only in sparse, short
source-pool/upper-tributary/fall/major-river-sink landmarks. Closure,
authoritative wake, hotspot performance, movement, map, and RD16 evidence
pass. Interactive review accepted revision 11's hydraulic stability but
rejected its terrain treatment: the local support band becomes a raised
grass-topped shelf with a tiny waterfall outlet. This is not a drainage graph:
general highland rivers, confluences, discharge, globally monotonic macro
drainage, and one integrated river-to-deep-basin outlet remain open. Tactical
[`222`](tactical/222-bounded-valley-stream-structures.md) is active to replace
it with a bounded multi-chunk stream that follows and carves an existing
valley through reusable procedural structure starts and clipped pieces.
Field revision 12 now realizes deterministic 91-96-block routes with
monotonic reaches, zero required fill, shallow carved valleys, fixed-point
drop stencils, matching far LOD, and exact SQLite reopen. Closure,
authoritative wake, partition, periodic seam, release performance, movement,
and fully warmed RD16 evidence pass. Human Review 1 accepted its peaceful
spring-fed-creek language, completing Tactical 222; general drainage
semantics remain explicitly out of scope.
Tactical
[`223`](tactical/223-mclone-climate-and-bookend-biomes.md) is
implementation-complete and awaiting Human Review 1. Field revision 13 adds
periodic temperature/moisture and altitude cooling; decoration revision 10
realizes cool-wet conifer, snowy alpine, and warm-dry steppe through
taiga/snowy-mountain/savanna-compatible biome IDs, alpine snow and exposed
rock, spruce/pine/fern/berry language, and sparse
acacia/tall-grass/flower language. Exact cylinder seams, far-LOD snow, SQLite
reopen, browser WASM compilation, generation performance, a 3,600-frame
mixed-climate movement route, and fully warmed RD16 cards pass.

Compatibility safety is recorded in the
[`world-generation-profiles` ledger](topics/world-generation-profiles.md#compatibility-safety-ledger).
The project is currently internal and unshipped: Flat Grass, Small Island,
Mclone Overworld, and authored-only fixtures are mutable proving surfaces,
while `overworld` remains locked because Java 1.17.1 parity is its external
correctness target.

The implementation lives primarily in `native/crates/mclone-worldgen`, with
scheduler/publication integration in `native/crates/mclone-server` and shared
chunk data in `native/crates/mclone-core`.

## Current Shape

Landed native coverage:

- Java-compatible PRNG and worldgen seed helpers.
- Noise primitives, octaved noise, blended noise, and `NoiseSampler`.
- `OverworldBiomeSource` and sampled biome fixtures.
- Terrain density fill, bedrock, and current surface material path.
- Classic overworld AIR and LIQUID carvers, including committed carved-stage oracle fixtures.
- Broad first decoration/feature framework slices, including trees,
  vegetation, ores, lakes, springs, ice, ocean plants, dripstone, and related
  biome palette coverage.
- Scheduler-owned `FEATURES` publication and clean fixture comparisons for the current target chunks.
- Generated scheduled tick carry-through for fluids.
- A stored, descriptor-driven `WorldGenerationProfile` boundary with
  `Overworld`, `FlatGrassV1`, `SmallIslandV1`, `McloneOverworldV1`, and
  `AuthoredOnly`; Flat Grass is target-only, Small Island and Mclone use typed
  feature dependencies, and authored-only maps true persistence misses to
  void.
- Exact `flat-grass-v1` bedrock/dirt/grass layers, plains biomes, empty tick
  payloads, origin spawn policy, native/dedicated publication, and save/reopen
  coverage. It now supports authoritative finite dimensions and a persisted,
  networked 32-chunk periodic-X cylinder. Mclone Overworld additionally
  supports its exact 384-chunk periodic-X cylinder; other procedural profiles
  still reject non-Euclidean topology explicitly.
- Bounded `small-island-v1` world-coordinate terrain with seeded shoreline and
  relief, a guaranteed central spawn patch, plains/beach/ocean biomes,
  native/dedicated publication, seam/partition locks, and save/reopen coverage.
- Continuous `mclone-overworld-v1` ocean, coast, open grass lowland, and
  wooded rolling upland terrain from pure production point/region samples,
  with separate gravel/sand/grass/stone surface recipes, a deterministic dry
  spawn, dependency-bearing vegetation execution, and field, seam, order,
  partition, cache, codec, native, and browser regression evidence.
- Its first mountain/valley family adds coherent ridge and ruggedness fields,
  derived slope/exposure, open valley and shoulder language, and exposed-stone
  treatment. Human review found field revision 4 too smooth and too
  large-scale. Field revision 5 adds mountain-gated 32/8-block detail and
  reduces broad ridge lift; the multiscale benchmark places it substantially
  closer to undecorated Java 1.17.1 terrain without changing the lowland
  control. Human review rejected its diagonal lattice-terrace artifact;
  field revision 6 replaces it with periodic-ready 32/8-block gradient detail,
  gentle independent domain warps, and a 0.70/0.30 band balance. The full RD16
  matrix removes the repeated chevrons while preserving the exact lowland
  control. Human Review 3 accepted the result as more natural and less
  geometric, and production browser Worker closeout passed. A local
  structure-tensor metric supplements, but does not replace, pixel review.
- Its first watercourse family adds a periodic warped-contour river field,
  analytic centerline distance/tangent sampling, locally graded beds and
  banks, river biome `7`, gravel beds, coastal
  sand transitions, dry-spawn exclusion, and sparse clay-bottomed wetland
  pools. Production maps and actual 384-chunk cylinder-seam cards agree with
  chunk output. The reviewed fixed-work implementation remains deliberately
  short of drainage-network, tributary, confluence, discharge, and true
  downstream-reach semantics. Its first smooth hydraulic realization was
  rejected because column-local water height could slope across a channel and
  expose source faces. Field revision 8 instead realizes only constant-Y63
  lowland reaches. Revision 9 adds size-aware ocean depth. Revision 10's local
  stepped corridor was hydraulically stable but globally non-monotonic.
  Revision 11 restores Y63 major rivers and reuses the stable four-block drop
  only in a complete bounded tributary landmark with an explicit source pool
  and river sink. A halo-aware block audit and real fluid-runtime wake tests
  prove the ordinary river and landmark are quiescent. Revision 12 replaces
  its rejected containment shelf with a reusable procedural start and a
  95-block reviewed valley stream: four calm reaches, three drops, a rounded
  headwater, grassed cut shoulders, and a widened Y63 confluence. Full chunks,
  far LOD, and persisted reopen agree. General highland networks,
  drainage-network identity, accumulated discharge, and arbitrary
  confluences remain absent.
- Its first climate family adds independent periodic temperature and moisture
  fields, derived altitude cooling, and broad conifer, alpine, steppe,
  woodland, and meadow recipes. New regions emit taiga `5`, snowy mountains
  `13`, and savanna `35`; alpine snow remains visible in synthetic far LOD,
  while conifer and steppe use Mclone-owned decoration density. Human Review
  1 is pending.
- Production-backed broad field maps and fully warmed seed/region/spawn cards;
  the first review accepted macro scale and coast variation after adding one
  fine relief octave to break up concentric local contour bands. The second
  review added biome/surface maps and tuned always-on flowers into occasional
  patches.
- Profile-qualified worker results: dependency-cache and generation-timing
  diagnostics are explicitly optional. Overworld reports cache and timing;
  Small Island and Mclone report their concrete caches without synthesizing
  Overworld timing; target-only Flat Grass has neither.
- Small Island and Mclone share the plan-bounded Surface dependency-cache
  lifecycle while retaining profile-owned surface generation, biome payloads,
  decoration domains/tables, target post-processing, and spawn guarantees.
  Reference Overworld retains its distinct timed, carved-stage cache path.

Important native entry points:

| Area | Native source |
|---|---|
| PRNG/noise/biomes/terrain/features | `native/crates/mclone-worldgen/src/` |
| Carver status detail | [`carver-status.md`](carver-status.md) |
| Shared chunk/snapshot data | `native/crates/mclone-core/src/chunk.rs` |
| Server scheduler/publication | `native/crates/mclone-server/src/scheduler.rs` |
| Native worldgen smoke | `pnpm native:worldgen:smoke` |
| Terrain characteristics | `pnpm native:worldgen:terrain-characteristics` |
| Shared oracle fixtures | `test/fixtures/` |
| Oracle generation tooling | `oracle/` |

## Oracle Inputs

The retained fixture root is `test/fixtures/`. These fixtures are generated from Java/oracle tooling under `oracle/` and are consumed directly by Rust tests.

Current fixture families include:

- `test/fixtures/prng/`
- `test/fixtures/noise/`
- `test/fixtures/biome/`
- `test/fixtures/integration/`
- `test/fixtures/scheduler/`
- `test/fixtures/liquid/`
- `test/fixtures/creatures/`

Do not move these fixtures without updating native consumers in `mclone-worldgen`, `mclone-server`, and `mclone-mesh`.

## Deferred Or Incomplete

Still not full vanilla parity:

- full decorated chunk parity remains an active gauntlet rather than a finished guarantee
- broad biome/decorator confidence still needs more targeted fixtures
- no true native structure-start/reference/piece runtime exists yet; the
  former buried-treasure implementation and the former desert-well,
  monster-room, and fossil feature work belonged to the retired TypeScript
  engine
- the first original island generator remains intentionally bounded; the
  continuous mclone overworld now exists but has only a deliberately narrow
  terrain/material/biome/vegetation palette. Its first broad river and sparse
  wetland family failed Human Reviews 1 and 2, and the global stepped-river
  attempt was rejected interactively. Corrective bathymetry and the first
  bounded valley stream now pass objective and human review. It still has no
  general highland stream/cascade family, drainage-network semantics, caves,
  or authored gameplay structures
- Mclone Overworld terrain, biome, surface, spawn, existing vegetation, broad
  rivers, and wetland pools are periodic for the exact 384-chunk X cylinder.
  All later content must join that sampler contract explicitly; canonical
  chunk wrapping alone remains intentionally insufficient
- full entity/natural-spawn parity is incomplete
- block-state breadth is intentionally narrower than exhaustive vanilla state coverage
- Caves & Cliffs Part 1 systems disabled in 1.17.1 vanilla overworld remain out of scope unless the target changes

## Validation

Use native validation lanes first:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
pnpm native:worldgen:smoke
pnpm test
```

For cross-system changes, include the fixture-consuming crates:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen -p mclone-server -p mclone-mesh
```

## Tactical Trail

Current native tacticals live under [`docs/tactical/`](tactical/README.md). Start with:

- [`003-native-ts-parity-roadmap.md`](tactical/003-native-ts-parity-roadmap.md) for the broad native parity horizon.
- [`015-decoration-framework-foundation.md`](tactical/015-decoration-framework-foundation.md) through the later worldgen tacticals for feature/decorator progress.
- [`017-full-decorated-chunk-parity-gauntlet.md`](tactical/017-full-decorated-chunk-parity-gauntlet.md) for the current full decorated chunk parity target.
- [`187-generator-profile-flat-grass-and-seeded-island.md`](tactical/187-generator-profile-flat-grass-and-seeded-island.md) for the accepted multi-generator refactor and first original terrain proof.
- [`188-mclone-overworld-v1-terrain-foundation.md`](tactical/188-mclone-overworld-v1-terrain-foundation.md) for the first original continuous-terrain profile and its explicit reuse/refactor reviews.
- [`192-mclone-overworld-mountains-and-valleys.md`](tactical/192-mclone-overworld-mountains-and-valleys.md) for the accepted original relief family.
- [`196-periodic-mclone-terrain-fields.md`](tactical/196-periodic-mclone-terrain-fields.md) for the completed seam-safe field phase before rivers and wetlands.
- [`220-mclone-overworld-rivers-and-wetlands.md`](tactical/220-mclone-overworld-rivers-and-wetlands.md) for the active bounded watercourse field and human visual gate.
