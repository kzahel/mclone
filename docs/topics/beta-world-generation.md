# Beta V1 World Generation

Topic: `beta-world-generation`

Status: **Complete under Tactical
[`194`](../tactical/194-beta-v1-world-generation.md). `beta-v1` is a live,
selectable internal-mutable Beta 1.7.3 Overworld profile with exact pinned
climate/biome, terrain, surface, and cave receipts, deterministic
Beta-flavored population, persisted tag `7`, shared host coverage, and
inspected warm/cold cards.**

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
internal historical-profile contract and evidence; it is not the product
worldgen direction.

## Scope And Reference Boundary

`beta-v1` means the Beta 1.7.3 Overworld, not every generator present in the
jar. The Nether and normally inaccessible Skylands generator are separate
future product decisions.

Exactness is staged:

- Java-compatible climate, biome lookup, density terrain, surface, and cave
  stages match the pinned semantic oracle receipts at origin and a mixed-sign
  cave chunk;
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

## Current Evidence

The implementation remains isolated under
`mclone-worldgen::levelgen::beta`: it calls neither Alpha nor the modern
Overworld pipeline. It reuses only neutral Java random, coordinate, chunk,
feature-region, generation-plan, and surface-dependency-cache contracts. The
server owns a separate resident Beta cache, and switching profile or seed
resets descriptor-bound state.

The exact origin receipts for seed `12345` are:

| Stage or field | SHA-256 |
|---|---|
| biome grid | `62e467d88bd1a744bbe69de118c40f38720d2398f9b9b12b00d04b9eb5eb7f46` |
| temperature doubles | `0e6845828c0fc3b51155032c1b2401cf0665a8c1cd8b6f94477ffea4b3ccb991` |
| downfall doubles | `e5a00aa9991ac8a5ee3109844d84a55583bd20572ad3ffcd42792f3c36b183ad` |
| terrain | `790e4588b113757ea26e75a60dc6cb4b966e7732387f36e57753d2a894986a88` |
| surface | `a8d110df56e2cc10e57aa943427f9307be8f04e7a68e8fb0fa04f4e839487ce2` |
| caves | `15d17bb7e95e712fb64d797583a1a00a2f511422381f1f0f589e3b6ae3e0f930` |

The mixed-sign seed-`12345` chunk `(-3, 5)` cave receipt is
`8eb348ee2438cbd02aa6ce58701cefe9b91db92d920154df1bacc7a949844a2c`.
Native semantic-order tests match all seven values exactly.

The visually inspected cards are:

- `/tmp/mclone-beta-first/beta-v1-seed-12345-chunk-0-0-card.png`;
- `/tmp/mclone-beta-cold-neg46/beta-v1-seed-neg46-chunk-0-0-card.png`.

The first shows warm desert highlands, cave mouths, water cuts, and biome
transitions. The second shows frozen water, snowfields, taiga conifers, and
cold relief. The integrated capture receipts each report all 1,225 target
chunks ready and three non-empty rendered views.

Closeout passed the repeated Java probes, focused native tests, full native
workspace, thin-adapter purity gate, desktop offscreen pixel smoke, WASM build,
and full browser generator gate. The browser gate rendered `beta-v1` through
the shared-memory Rust worker, saved 121 chunks, and reopened the stored tag-7
profile with the same safe Y=86.62 spawn.
