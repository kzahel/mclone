# Tactical 194: Beta V1 World Generation

Status: active 2026-07-18.

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

- Reserve `beta-v1` as `planned-unallocated` in the compatibility ledger.
- Record Overworld-only scope, parity stages, effective biome palette,
  deterministic population divergence, safe spawn, ownership, and deferred
  work in the implementation topic.
- Lock the next binary tag and unchanged existing tags in tests before live
  dispatch is added.

Gate: a Maintainer can distinguish exact targets, flavor targets, and excluded
whole-game Beta systems before generator code lands.

## Slice 1: Staged Beta Oracle

Status: planned.

- Add a deterministic Java probe against the pinned mapped merged jar.
- Expose climate/biome, terrain, surface, and cave stages without invoking the
  interactive client or save storage.
- Emit raw Beta X/Z/Y SHA-256, semantic block counts, biome/climate receipts,
  and useful height ranges.
- Commit small text fixtures for seed `12345` at origin and mixed-sign chunks.
- Add native block-axis normalization tests.

Gate: repeated probe runs agree and a wrong block map or axis order fails.

## Slice 2: Profile And Shared Dispatch

Status: planned.

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

Status: planned.

- Direct-port Beta `SimplexNoise` and `PerlinSimplexNoise` climate streams.
- Direct-port the Beta-specific `ImprovedNoise` `sizeY == 1` path.
- Reproduce temperature/downfall shaping and the 64 by 64 biome lookup.
- Port the eight noise banks, climate-modulated 5 by 17 by 5 density field,
  trilinear expansion, sea water, and local surface ice.
- Store native biome payloads corresponding to all effective Beta biomes.

Gate: representative climate, biome, and terrain receipts match exactly and
the first headless terrain capture is inspected.

## Slice 4: Surfaces And Caves

Status: planned.

- Port biome top/filler selection, sand/gravel masks, sandstone transition,
  water repair, and uneven bedrock.
- Port the Beta cave traversal with its corrected horizontal ellipse cursor
  behavior rather than calling Alpha's carver.
- Preserve Beta random construction and reseeding order.

Gate: representative surface and cave receipts match exactly, including at a
negative coordinate, while Alpha receipts remain unchanged.

## Slice 5: Deterministic Beta Population

Status: planned.

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

Status: planned.

- Create and reopen Beta worlds through shared catalog and metadata paths.
- Validate target-only generation, worker reset, scheduling, integrated host,
  and browser generator lanes.
- Capture at least two reviewed landscape/worldgen cards showing contrasting
  Beta terrain and climates.
- Run focused tests, the full native workspace, thin-adapter checks, desktop
  offscreen smoke, and web build.
- Update this tactical and the implementation/reference/profile topics with
  exact receipts, screenshot paths, validation evidence, and honest gaps.

Gate: `beta-v1` is selectable and playable, looks recognizably old-Beta, and
has reproducible evidence proportional to its stated parity boundary.

