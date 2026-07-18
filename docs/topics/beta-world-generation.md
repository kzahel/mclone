# Beta V1 World Generation

Topic: `beta-world-generation`

Status: **Implementation authorized and active under Tactical
[`194`](../tactical/194-beta-v1-world-generation.md). `beta-v1` is reserved as
an internal-mutable Beta 1.7.3 Overworld profile; no runtime identity or output
exists yet.**

## Objective

Build a selectable, playable generator with the recognizable Minecraft Java
Beta 1.7.3 Overworld language:

- climate-shaped old-Beta hills, cliffs, overhangs, and oceans;
- the effective ten-biome palette with local desert, forest, taiga, tundra,
  swamp, and plains surfaces;
- Beta caves, water, local ice, and climate-derived snow;
- biome-weighted trees and ground cover; and
- representative old-Beta lakes, ores, plants, pumpkins, springs, and
  dungeons.

The source archaeology and measured Alpha comparison remain in
[`beta-1.7.3-reference.md`](beta-1.7.3-reference.md). This topic owns the live
native product contract and evidence.

## Scope And Parity Boundary

`beta-v1` means the Beta 1.7.3 Overworld, not every generator present in the
jar. The Nether and normally inaccessible Skylands generator are separate
future product decisions.

Exactness is staged:

- Java-compatible climate, biome lookup, density terrain, surface, and cave
  stages should match pinned semantic oracle receipts at representative
  chunks;
- native chunk storage may remain 256 blocks high while Beta output occupies
  Y `0..=127`;
- population is deterministic and scheduler-order-independent rather than a
  reproduction of Beta's load-order-dependent mutation history;
- representative population preserves Beta attempt counts, broad height
  distributions, biome weighting, and feature order where implemented;
- spawn uses mclone's deterministic safe-spawn policy instead of Beta's
  separately seeded sand-search walk; and
- block comparisons use a semantic mapping rather than assuming Beta numeric
  IDs equal native registry IDs.

The profile is `internal-mutable`. Oracle receipts constrain the intended
historical core, but no shipped or named retained world requires current
output.

## Ownership And Isolation

All Beta policy belongs in a sibling `mclone-worldgen` Beta module. It may use
general engine contracts for coordinates, block states, Java random, chunk
buffers, feature-region mutation, generation plans, and dependency-cache
lifecycle. It must not call the Alpha generator or the 1.17 terrain, biome,
surface, or cave pipeline.

Small identical-looking Alpha/Beta algorithms may remain duplicated until
both oracle suites prove an extraction safe. No broad configurable legacy
generator or Alpha/Beta mode switches should obscure historical differences.

Server, persistence, startup, catalog, and client code own only profile
selection and generic descriptor transport. They gain no Beta terrain policy.

## Effective Biomes

The generator exposes only biomes returned by Beta 1.7.3's climate decision
tree:

- Rainforest
- Swampland
- Seasonal Forest
- Forest
- Savanna
- Shrubland
- Taiga
- Desert
- Plains
- Tundra

Declared but unreachable Ice Desert is reference data, not an effective
Overworld output. Hell and Sky are dimension-specific and excluded.

## Acceptance

Completion requires:

1. repeatable Java oracle receipts for climate/biome, terrain, surface, and
   cave stages at origin and mixed-sign coordinates;
2. exact native semantic hashes for a useful representative subset, with any
   remaining nonmatching stages diagnosed and recorded rather than hidden;
3. deterministic target chunks across batch partition and request order;
4. profile JSON/binary, worker codec, persistence, spawn, catalog, CLI, and
   browser/WASM coverage;
5. unchanged Alpha and 1.17 reference fixtures;
6. focused and workspace tests plus the normal web build; and
7. inspected headless screenshots showing at least two seeds or contrasting
   climate regions, saved outside the repository.

## Known Deferred Work

- exact post-population parity under historical chunk-loading order;
- Beta chest loot, spawner behavior, and immediate fluid-tick side effects;
- every original tree geometry variant if representative shared/native trees
  already establish the requested flavor;
- weather, mobs, lighting quirks, metadata fidelity, and other whole-game Beta
  simulation behavior;
- Nether and Skylands generation; and
- Far Lands and extreme-coordinate floating-point conformance.

