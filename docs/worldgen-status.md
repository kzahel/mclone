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
genuinely periodic Mclone fields and features are planned in
[`tactical/196-periodic-mclone-terrain-fields.md`](tactical/196-periodic-mclone-terrain-fields.md)
after the first accepted Tactical 192 field set.

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
  networked 32-chunk periodic-X cylinder; other procedural profiles still
  reject non-Euclidean topology explicitly.
- Bounded `small-island-v1` world-coordinate terrain with seeded shoreline and
  relief, a guaranteed central spawn patch, plains/beach/ocean biomes,
  native/dedicated publication, seam/partition locks, and save/reopen coverage.
- Continuous `mclone-overworld-v1` ocean, coast, open grass lowland, and
  wooded rolling upland terrain from pure production point/region samples,
  with separate gravel/sand/grass/stone surface recipes, a deterministic dry
  spawn, dependency-bearing vegetation execution, and field, seam, order,
  partition, cache, codec, native, and browser regression evidence.
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
  terrain/material/biome/vegetation palette and no mountains, rivers, caves,
  or structures
- Mclone Overworld terrain and features remain planar until Tactical 196 makes
  their production samplers, dependency plans, and feature writes periodic;
  canonical chunk wrapping alone is intentionally rejected
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
- [`192-mclone-overworld-mountains-and-valleys.md`](tactical/192-mclone-overworld-mountains-and-valleys.md) for the next original relief family.
