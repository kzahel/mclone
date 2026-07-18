# Tactical 194: Beta V1 World Generation

Status: complete 2026-07-18.

Topic: `beta-world-generation`

Workstream: Java oracle/reference only, shared native Rust world generation,
server scheduling and persistence, shared startup/catalog configuration,
desktop rendered validation first, then browser/WASM contract validation.

## Objective

Implement a playable end-to-end `beta-v1` profile based on Minecraft Java
Beta 1.7.3. Direct-port the climate, biome, terrain, surface, and cave core;
then add a deterministic representative population subset that reads as
old-Beta without pursuing exact historical load-order side effects.

The durable implementation contract lives in
[`../topics/beta-world-generation.md`](../topics/beta-world-generation.md).
The decompiled source map lives in
[`../topics/beta-1.7.3-reference.md`](../topics/beta-1.7.3-reference.md).

## Constraints

- Scope is the Beta 1.7.3 Overworld only; exclude Nether and Skylands.
- Shared Rust owns generation. App and browser code transport/select profiles
  but contain no Beta policy.
- Beta parity-sensitive code is a sibling implementation, not Alpha switches
  or a generalized legacy-generator abstraction.
- Existing profile identities, tags, outputs, and fixtures remain unchanged.
- Append one binary profile tag after Alpha's existing tags `5` and `6`.
- Active Beta output is Y `0..=127` inside the shared 256-block chunk.
- Compare semantic Beta block meaning, not raw numeric IDs.
- Population is deterministic across scheduling and request order.
- Capture and inspect pixels at the first drawable milestone and after
  population, keeping all screenshots under `/tmp`.

## Slice 0: Contract And Compatibility Lock

Status: complete.

- Initially reserve `beta-v1` as `planned-unallocated`, then allocate it as
  `internal-mutable` before runtime use.
- Record Overworld-only scope, parity stages, effective biome palette,
  deterministic population divergence, safe spawn, ownership, and deferred
  work in the implementation topic.
- Lock the next binary tag and unchanged existing tags in tests before live
  dispatch is added.

Gate: a Maintainer can distinguish exact targets, flavor targets, and excluded
whole-game Beta systems before generator code lands.

## Slice 1: Staged Beta Oracle

Status: complete.

- Add a deterministic Java probe against the pinned mapped merged jar.
- Expose climate/biome, terrain, surface, and cave stages without invoking the
  interactive client or save storage.
- Emit raw Beta X/Z/Y SHA-256, semantic block counts, biome/climate receipts,
  and useful height ranges.
- Commit small text fixtures for seed `12345` at origin and mixed-sign chunks.
- Add native block-axis normalization tests.

Gate: repeated probe runs agree and a wrong block map or axis order fails.

## Slice 2: Profile And Shared Dispatch

Status: complete.

- Add `WorldGenerationProfile::BetaV1` with JSON name `beta-v1` and the next
  append-only binary tag.
- Carry it through generation planning, resident worker/cache reset, encoded
  requests, native and browser persistence, catalog display/cycling, startup
  parsing, spawn, diagnostics, and help text.
- Allocate the safety-ledger row as `internal-mutable` with its exact update
  requirements before runtime use.

Gate: profile round trips and descriptor changes are tested without changing
any existing profile serialization or output.

## Slice 3: Climate, Biomes, And Density Terrain

Status: complete.

- Direct-port Beta `SimplexNoise` and `PerlinSimplexNoise` climate streams.
- Direct-port the Beta-specific `ImprovedNoise` `sizeY == 1` path.
- Reproduce temperature/downfall shaping and the 64 by 64 biome lookup.
- Port the eight noise banks, climate-modulated 5 by 17 by 5 density field,
  trilinear expansion, sea water, and local surface ice.
- Store native biome payloads corresponding to all effective Beta biomes.

Gate: representative climate, biome, and terrain receipts match exactly and
the first headless terrain capture is inspected.

## Slice 4: Surfaces And Caves

Status: complete.

- Port biome top/filler selection, sand/gravel masks, sandstone transition,
  water repair, and uneven bedrock.
- Port the Beta cave traversal with its corrected horizontal ellipse cursor
  behavior rather than calling Alpha's carver.
- Preserve Beta random construction and reseeding order.

Gate: representative surface and cave receipts match exactly, including at a
negative coordinate, while Alpha receipts remain unchanged.

## Slice 5: Deterministic Beta Population

Status: complete.

- Reuse only neutral feature-region and plan/cache contracts.
- Replay canonical population centers over declared mutable dependencies.
- Implement the highest-value Beta flavor: biome-weighted oak/birch/conifer
  trees, grass/fern/dead bush, flowers/mushrooms, cactus, sugar cane,
  pumpkins, clay and ores including lapis, springs, lakes, snow, and simple
  dungeons where practical.
- Preserve the original broad order and random draws within implemented
  features, while documenting omitted consumers that prevent exact final
  population parity.

Gate: population is deterministic across batch partition/order, creates
cross-chunk features safely, and produces expected biome/material diversity.

## Slice 6: Product And Visual Closeout

Status: complete.

- Create and reopen Beta worlds through shared catalog and metadata paths.
- Validate target generation, worker reset, scheduling, integrated host,
  and browser generator lanes.
- Capture at least two reviewed landscape/worldgen cards showing contrasting
  Beta terrain and climates.
- Run focused tests, the full native workspace, thin-adapter checks, desktop
  offscreen smoke, and web build.
- Update this tactical and the implementation/reference/profile topics with
  exact receipts, screenshot paths, validation evidence, and honest gaps.

Gate: `beta-v1` is selectable and playable, looks recognizably old-Beta, and
has reproducible evidence proportional to its stated parity boundary.

## Closeout Evidence

Completed all slices on 2026-07-18.

- Added the shared `BetaV1` profile with JSON name `beta-v1`, append-only
  binary tag `7`, shared catalog/startup selection, safe spawn, persistence,
  dedicated-server selection, worker framing, diagnostics, and deterministic
  3x3-over-5x5 dependency planning.
- Directly ported Beta's climate simplex streams, effective ten-biome lookup,
  Beta-specific 2D improved-noise path, climate-shaped 5 by 17 by 5 density
  lattice, surface replacement, and corrected cave traversal into a standalone
  sibling of Alpha.
- Implemented deterministic Beta-flavored population for lakes, simplified
  dungeons, clay and the full Beta ore set including lapis, biome-weighted
  oak/birch/conifer trees, grasses/ferns/dead bushes, flowers/mushrooms,
  sugar cane, pumpkins, cactus, springs, and local snow.

Exact Beta X/Z/Y SHA-256 receipts:

| Fixture | SHA-256 |
|---|---|
| origin biome grid | `62e467d88bd1a744bbe69de118c40f38720d2398f9b9b12b00d04b9eb5eb7f46` |
| origin temperature doubles | `0e6845828c0fc3b51155032c1b2401cf0665a8c1cd8b6f94477ffea4b3ccb991` |
| origin downfall doubles | `e5a00aa9991ac8a5ee3109844d84a55583bd20572ad3ffcd42792f3c36b183ad` |
| origin terrain | `790e4588b113757ea26e75a60dc6cb4b966e7732387f36e57753d2a894986a88` |
| origin surface | `a8d110df56e2cc10e57aa943427f9307be8f04e7a68e8fb0fa04f4e839487ce2` |
| origin caves | `15d17bb7e95e712fb64d797583a1a00a2f511422381f1f0f589e3b6ae3e0f930` |
| chunk -3,5 caves | `8eb348ee2438cbd02aa6ce58701cefe9b91db92d920154df1bacc7a949844a2c` |

Visually inspected screenshot receipts:

- `/tmp/mclone-beta-first/beta-v1-seed-12345-chunk-0-0-card.png`: desert
  highlands, terraced relief, cave mouths, water cuts, vegetation, and warm
  biome transitions;
- `/tmp/mclone-beta-cold-neg46/beta-v1-seed-neg46-chunk-0-0-card.png`: frozen
  water, snowfields, taiga conifers, and cold relief.

Validation passed:

- repeated Java terrain, surface, origin-cave, and mixed-sign-cave probes;
- focused climate/hash, population, profile/codec, persistence, startup,
  catalog, spawn, dedicated CLI, and order/partition tests;
- `cargo test --manifest-path native/Cargo.toml`;
- `pnpm native:thin-adapters:purity`;
- `pnpm native:desktop-offscreen:smoke`, with its output inspected;
- `pnpm native:web:build`; and
- `pnpm native:web:generator-smoke`, including Beta Web Worker rendering and
  IndexedDB profile/chunk reopen.

Accepted gaps remain limited to the implementation topic's deferred-work list.
The climate, biome, terrain, surface, and cave stages are exact for the pinned
receipts; final population is deliberately deterministic and flavor-close.
