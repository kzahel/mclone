# Alpha V1 World Generation

Topic: `alpha-world-generation`

Status: **Implemented and validated by Tactical
[`193`](../tactical/193-alpha-v1-world-generation.md). `alpha-v1` is a live
internal profile intentionally based on Minecraft Java Alpha v1.1.2_01 without
promising perfect historical parity.**

## Scope

`alpha-v1` is a small, playable, internal generator profile that preserves the
recognizable pre-biome Alpha terrain language:

- the original Java random and octave-noise construction order;
- the 5 by 17 by 5 density lattice expanded into 16 by 128 by 16 terrain;
- sea-level water, overhangs, floating terrain, and the old global surface
  vocabulary;
- branching Alpha-style caves with low lava;
- sparse hardcoded ores, trees, flowers, mushrooms, sugar cane, cactus,
  springs, clay, and dungeon rooms; and
- one explicit persisted temperate/winter choice.

The reference specimen and decompilation receipts remain in
[`alpha-era-reference.md`](alpha-era-reference.md). This topic owns the native
product contract, current implementation truth, deliberate divergences,
validation, and recommended follow-up.

## Product Identity And Compatibility

The persisted generator identity is `alpha-v1`. It is an
`internal-mutable` profile once implemented: its fixtures prevent accidental
drift, but the project has no shipped Alpha worlds or external save consumers.

The profile owns one explicit Boolean option:

```text
alpha-v1
  winter: false  -> temperate world
  winter: true   -> whole-world Alpha winter
```

Winter is never selected from wall-clock-seeded randomness. It is chosen by
the creator/CLI, included in the immutable generation descriptor, persisted in
world metadata, transported to native and browser workers, and displayed in
world-selection UI. A CLI/query flag may set it independently from the
`alpha-v1` profile selection and is order-independent. It is ignored when the
selected profile is not Alpha, so a shared launch URL can carry the preference
without changing another generator.

The profile name deliberately does not contain `a1.1.2_01`. The historical
version is the reference basis, while the native profile records intentional
architectural adaptations and may improve under the compatibility ledger.

## Parity Boundary

### Close or exact where practical

- `java.util.Random` bit behavior and signed overflow;
- noise-bank construction and sample order;
- density, interpolation, sea level, surface masks, layer depth, and bedrock;
- target-chunk cave shape, water avoidance, grass repair, and lava below Y=10;
- the original population attempt counts and height distributions where the
  corresponding mclone block exists; and
- the original 128-block active generation range inside mclone's 256-block
  chunk representation.

Terrain, surface, and cave stages use normalized Alpha oracle fingerprints.
If an implementation simplification changes a fingerprint, the tactical must
record the difference and visual evidence before accepting it.

The current native core exactly matches the decompiled probe's raw Alpha block
bytes for the five pinned receipts:

| Fixture | SHA-256 |
|---|---|
| seed 12345, chunk 0,0, terrain | `7442e144fdf4819e3b960d1ece8e0a43ce427e785fc7117354fc1204e5464105` |
| seed 12345, chunk 0,0, surface | `e07275f18a1e42bce6a078b06f469c01663559bfe3d317413a177a995468ee6f` |
| seed 12345, chunk 0,0, caves | `947b3a034360da67c83baef0fc486fd05f8373d57080abc2fe952662db55e833` |
| seed 12345, chunk -3,5, caves | `84849bd3df13751903fd519b92eeff0db698fcefc0bb8f4989f38707173c0444` |
| seed 12345, chunk 5,5, winter terrain | `b6438418692cbe93c555ee442a17500455f8ab49e0db9348f66e89c736a0dc04` |

### Deliberate adaptations

- Alpha's unseeded 1-in-4 winter choice becomes explicit persisted state.
- Spawn uses mclone's deterministic safe-surface search near origin rather
  than Alpha's unseeded random walk until sand.
- Population writes run through the deterministic generator-owned feature
  plan and mutable feature region. Alpha's chunk-load-order side effects are
  not reproduced.
- Modern mclone block-state IDs remain canonical. Oracle comparison normalizes
  both sides through a semantic Alpha block map.
- Missing functional chest/spawner state does not block the first profile;
  dungeon geometry uses available stone/mossy materials and records that
  runtime-content gap.
- Far Lands, exact floating-point behavior at extreme coordinates, Alpha save
  format, and historical lighting bugs are outside v1 acceptance.

## Shared Ownership

```text
stored DimensionDefinition
  WorldGenerationProfile::AlphaV1 { winter }
  seed
       |
       v
GenerationPlanRequest
  deterministic Surface prerequisites and population-center footprint
       |
       v
mclone-worldgen alpha module
  density -> surface -> caves -> deterministic population -> winter finish
       |
       v
ordinary GeneratedChunk
  scheduler -> lighting -> persistence -> native/web/Android/XR clients
```

- `mclone-server` owns the persisted profile value, worker descriptor, planning
  dispatch, scheduler, spawn admission, and storage codec.
- `mclone-worldgen` owns every Alpha algorithm, semantic mapping helper, and
  profile-specific cache/session.
- `mclone-app-runtime` owns creator-facing profile/winter selection and catalog
  display. App/platform crates only pass the shared startup schema.
- Renderer, mesh, lighting, physics, and clients consume ordinary chunks and
  gain no Alpha-specific policy.

## Semantic Block Mapping

The Java oracle emits Alpha numeric IDs because those are the bytecode-visible
reference values. Native comparison converts mclone blocks back to this small
semantic vocabulary before hashing:

| Alpha meaning | Alpha ID | Native block |
|---|---:|---|
| air | 0 | `AIR` |
| stone | 1 | `STONE` |
| grass block | 2 | `GRASS_BLOCK` |
| dirt | 3 | `DIRT` |
| cobblestone | 4 | `STONE` for initial dungeon geometry |
| bedrock | 7 | `BEDROCK` |
| water, flowing/still | 8/9 | `WATER` |
| lava, flowing/still | 10/11 | `LAVA` |
| sand | 12 | `SAND` |
| gravel | 13 | `GRAVEL` |
| gold/iron/coal ore | 14/15/16 | corresponding native ores |
| log/leaves | 17/18 | `OAK_LOG` / `OAK_LEAVES` |
| flowers and mushrooms | 37..40 | native dandelion, poppy, and mushrooms |
| mossy cobblestone | 48 | `MOSSY_COBBLESTONE` |
| diamond/redstone ore | 56/73 | corresponding native ores |
| snow layer/ice | 78/79 | `SNOW` / `ICE` |
| cactus/clay/reeds | 81/82/83 | `CACTUS` / `CLAY` / `SUGAR_CANE` |

Still/flowing fluid distinctions are intentionally collapsed because mclone's
generated source fluid plus scheduled-tick model is different. Oracle stage
fixtures state whether their hash uses raw Alpha IDs or normalized semantics.

## Oracle And Regression Contract

The Alpha Java probe must expose at least:

- density terrain;
- terrain plus surface replacement; and
- terrain plus surfaces plus caves.

Each receipt records version, stage, seed, chunk coordinate, winter bit,
semantic block counts, height range, and SHA-256 in Alpha's original X/Z/Y byte
order. Native tests reorder the chunk payload and map native blocks to Alpha
semantic IDs before comparison.

Population is accepted differently because deterministic planning is an
intentional architectural change. Its gates are:

- repeated, reversed-target, and partitioned generation equality;
- seams and cross-chunk feature writes;
- stable block-distribution fingerprints for representative seeds;
- visible presence of caves, ores, vegetation, and winter finishing; and
- inspected temperate and winter worldgen cards.

## Definition Of Done

The first end-to-end profile is complete when:

1. `alpha-v1` with explicit winter state round-trips through JSON, binary world
   metadata, native worker frames, browser worker frames, catalog records, CLI,
   and world reopen;
2. terrain, surface, and cave stage tests match or explicitly account for the
   pinned Alpha oracle receipts;
3. feature-complete chunks are deterministic across order, partition, cache,
   and native/browser execution;
4. origin spawn is safe in temperate and winter worlds;
5. ordinary lighting, meshing, collision, persistence, and remote chunk
   publication require no Alpha-specific client path;
6. desktop headless cards for both modes are captured under `/tmp`, visually
   inspected, and recorded in this topic and Tactical 193; and
7. focused Rust tests, workspace tests, web build, and relevant offscreen smoke
   gates pass.

## Known Deferred Exactness

- Alpha large-oak geometry and dungeon chest/spawner behavior;
- exact population collisions produced by historical chunk-load order;
- the unseeded spawn random walk;
- raw flowing-fluid ID parity and subsequent Alpha fluid simulation;
- Far Lands and extreme-coordinate floating-point archaeology; and
- historical client lighting, fog, and sky rendering.

These are possible follow-ups. They do not prevent `alpha-v1` from looking and
playing recognizably like the selected pre-biome Alpha reference.

## Current Evidence

Tactical 193 completed all seven slices on 2026-07-18. The implementation has
exact oracle equality through caves, deterministic generator-owned population,
stable binary tags `5` and `6` for temperate and winter, shared catalog/startup
selection, persistence and worker round trips, and safe origin spawn coverage.

The visually inspected seed-12345 cards are:

- `/tmp/mclone-alpha-final/alpha-v1-seed-12345-chunk-0-0-card.png`
- `/tmp/mclone-alpha-final/alpha-v1-winter-seed-12345-chunk-0-0-card.png`

Validation passed with the focused Alpha fixture and scheduling tests, the full
native workspace test suite, thin-adapter purity check, browser WASM build, and
desktop offscreen smoke. The deferred exactness list above remains intentional;
in particular, population is flavor-close rather than a promise of byte parity.
