# Tactical 193: Alpha V1 World Generation

Status: complete 2026-07-18.

Topic: `alpha-world-generation`

Workstream: shared native Rust world generation, server scheduling and
persistence, shared startup/catalog configuration, Java oracle/reference only,
desktop validation first, then browser/WASM contract validation.

## Objective

Implement a playable end-to-end `alpha-v1` generator based on Minecraft Java
Alpha v1.1.2_01. Preserve the characteristic terrain, surface, caves, sparse
features, and whole-world winter presentation while deliberately replacing
unseeded creation choices and load-order-sensitive population with explicit
state and deterministic planning.

The durable product and parity boundary lives in
[`../topics/alpha-world-generation.md`](../topics/alpha-world-generation.md).
The bytecode/source investigation lives in
[`../topics/alpha-era-reference.md`](../topics/alpha-era-reference.md).

## Constraints

- Shared Rust owns the generator. No app-local or TypeScript worldgen.
- Existing Overworld, Flat Grass, Small Island, Mclone Overworld, and
  Authored-only identities and fixtures remain unchanged.
- Append binary profile tags; never renumber existing tags `0..=4`.
- The winter bit is explicit, persisted, descriptor-visible, and selectable.
- Active Alpha terrain remains Y `0..=127` inside the existing 256-block chunk
  and client/runtime contract.
- Use semantic block normalization for oracle comparison; never equate Alpha
  numeric IDs with mclone raw IDs by accident.
- Population may diverge architecturally but must be deterministic across
  scheduling order and visually close.
- Pixel-producing milestones require immediate headless capture and visual
  inspection, with outputs under `/tmp`.

## Slice 0: Documentation And Contract Locks

Status: complete.

- Create the implementation topic and this tactical.
- Reserve `alpha-v1` as `planned-unallocated`, then allocate it as
  `internal-mutable` before its first runtime use.
- Record the parity boundary, semantic block mapping, winter contract,
  ownership, acceptance gates, and deferred exactness.
- Lock the next free binary tags and existing tag/JSON behavior in tests before
  changing dispatch.

Gate: the exact intended divergence from Alpha is reviewable before runtime
code lands.

## Slice 1: Oracle Stages And Semantic Mapping

Status: complete.

- Extend `AlphaTerrainProbe` with explicit terrain, surface, and cave stages.
- Keep winter an explicit probe argument.
- Emit raw Alpha-order SHA-256 and semantic block counts.
- Generate committed receipts for seed `12345`, origin plus a negative
  coordinate, and the relevant winter cases.
- Add a native mapping helper that reorders mclone storage into Alpha X/Z/Y
  order and converts native block IDs to the semantic Alpha vocabulary.

Gate: the reference produces repeatable stage receipts and tests detect a
wrong block mapping or axis order.

## Slice 2: Profile, Winter, Persistence, And Dispatch

Status: complete.

- Add `WorldGenerationProfile::AlphaV1 { winter }` with stable JSON and binary
  tags after existing tag `4`.
- Carry it through generation planning, worker execution, native/Web Worker
  frame codecs, resident cache reset, persistence, catalog records, spawn, and
  diagnostics.
- Add `--alpha-winter true|false` and `alphaWinter=` as shared startup inputs;
  apply them order-independently for Alpha and harmlessly ignore them for
  non-Alpha profiles.
- Add Alpha temperate/winter display text and product profile cycling.

Gate: both modes survive CLI/query parsing, catalog save/reopen, world metadata
save/reopen, worker round-trip, and descriptor-change reset tests.

## Slice 3: Density Terrain And Surface

Status: complete.

- Reuse the proven Java random implementation.
- Port the eight Alpha noise-bank construction order and octave combination.
- Port the 5 by 17 by 5 density lattice and 4 by 8 by 4 interpolation.
- Fill stone/water/air/ice in Y `0..=127`; leave Y `128..=255` air.
- Port sand, gravel, depth, grass/dirt, water repair, and uneven bedrock.
- Prime ordinary heightmaps and emit plains biomes throughout.

Gate: origin first drawable capture is inspected, then terrain/surface oracle
fixtures and negative-coordinate seam/determinism tests pass.

## Slice 4: Alpha Caves

Status: complete.

- Port the range-eight source-chunk traversal and seed derivation.
- Port room/tunnel movement, one-time branching, water rejection, target
  clipping, grass repair, and lava below Y=10.
- Keep cave mutation target-local so terrain dependencies remain deterministic.

Gate: cave-stage oracle receipts, neighbor seams, order independence, and a
cave-bearing focused fixture pass before feature work begins.

## Slice 5: Deterministic Alpha Population

Status: complete.

- Use generator-owned feature planning and the shared mutable feature region.
- Preserve Alpha attempt counts and broad Y distributions for clay, dirt,
  gravel, coal, iron, gold, redstone, diamond, trees, flowers, mushrooms,
  sugar cane, cactus, water/lava springs, and dungeon rooms.
- Use Alpha-like vein ellipsoids and compact oak geometry where practical.
- Apply whole-world snow/ice as an explicit final stage for winter profiles.
- Record substitutions for unavailable functional chest/spawner states.

Gate: features visibly occur, cross borders safely, and are byte-identical
under repeated, reversed, partitioned, cache-cold, and cache-warm generation.

## Slice 6: Product And Platform Closeout

Status: complete.

- Prove safe origin spawn and normal collision/lighting/meshing.
- Prove native SQLite and browser catalog/worker persistence.
- Capture temperate and winter three-view cards at the same seed and camera.
- Inspect every card and tune only when the change improves Alpha character
  without obscuring an oracle mismatch.
- Run focused crate tests, workspace tests, web build, and offscreen smoke.
- Update the topic, compatibility ledger, this execution record, and relevant
  reference/platform docs with exact receipts and remaining gaps.

Gate: one selectable profile works end to end through the shared engine and
the user-facing screenshots make the temperate/winter distinction obvious.

## Execution Record

Completed all slices on 2026-07-18.

- Added the shared `AlphaV1 { winter }` profile, stable binary tags `5` and
  `6`, JSON/catalog/startup selection, persistence, spawn, worker framing,
  diagnostics, and deterministic dependency planning.
- Directly ported Alpha's random/noise construction, 5 by 17 by 5 density
  lattice, surface replacement, and range-eight cave carver into shared Rust.
  The active world remains 128 blocks high inside the engine's 256-block
  chunks.
- Implemented deterministic Alpha-flavored population for ores, clay, trees,
  plants, reeds, cactus, springs, simplified dungeon shells, snow, and ice.
- Extended the decompiled Java oracle to expose terrain, surface, and cave
  stages and committed five receipts under
  `test/fixtures/alpha/a1.1.2_01/`.

Exact Alpha-order SHA-256 receipts:

| Fixture | SHA-256 |
|---|---|
| origin terrain | `7442e144fdf4819e3b960d1ece8e0a43ce427e785fc7117354fc1204e5464105` |
| origin surface | `e07275f18a1e42bce6a078b06f469c01663559bfe3d317413a177a995468ee6f` |
| origin caves | `947b3a034360da67c83baef0fc486fd05f8373d57080abc2fe952662db55e833` |
| chunk -3,5 caves | `84849bd3df13751903fd519b92eeff0db698fcefc0bb8f4989f38707173c0444` |
| chunk 5,5 winter terrain | `b6438418692cbe93c555ee442a17500455f8ab49e0db9348f66e89c736a0dc04` |

Visually inspected screenshot receipts:

- `/tmp/mclone-alpha-final/alpha-v1-seed-12345-chunk-0-0-card.png`
- `/tmp/mclone-alpha-final/alpha-v1-winter-seed-12345-chunk-0-0-card.png`

Validation passed:

- focused Alpha worldgen, profile/codec, persistence, startup, catalog, spawn,
  worker, and partition-independence tests;
- `cargo test --manifest-path native/Cargo.toml`;
- `pnpm native:thin-adapters:purity`;
- `pnpm native:web:build`; and
- `pnpm native:desktop-offscreen:smoke`, with its output visually inspected.

Accepted gaps are limited to the topic's deferred-exactness list. Terrain,
surface, and caves are exact for the pinned oracle receipts; population and
dungeon contents are intentionally deterministic and flavor-close.

## Stop Conditions

Stop and document rather than silently expanding scope if completion would
require:

- a new global block registry or widening the `u8` generated-block vocabulary;
- general structure-start or block-entity runtime architecture solely for
  Alpha dungeons;
- reproduction of load-order-dependent bugs;
- a client-specific Alpha renderer; or
- changing the reference-locked 1.17.1 Overworld output.
