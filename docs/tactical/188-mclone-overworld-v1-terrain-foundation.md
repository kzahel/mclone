# Tactical 188: Mclone Overworld V1 Terrain Foundation

Status: Slices 0 through 4 and Reviews 1 and 2 complete 2026-07-18; Slice 5 is
next. The live internal profile now provides reviewed continuous terrain,
biomes, surfaces, and vegetation through typed dependency planning and a
two-caller shared Surface dependency cache. Durable direction lives in
[`mclone-overworld-generation`](../topics/mclone-overworld-generation.md).

Topic: `mclone-overworld-generation`

Workstream: shared native Rust world generation and server scheduling, with
desktop visual validation first and adoption through the existing profile
contract on every host.

## Result

Land a separately stored provisional `mclone-overworld-v1` profile with:

- unbounded oceans, coasts, lowlands, and rolling uplands;
- a deliberately small biome, surface, and decoration palette;
- safe spawn and deterministic native/browser output;
- production-backed field maps and reusable landscape review cards; and
- explicit pauses for reuse review and behavior-preserving refactoring.

Mountains, rivers, caves, custom biome registry content, and true structures
remain later tacticals. The topic document owns their sequencing and the
durable architecture; this document owns only the first executable slice.

## Product Gate

Before implementation, approve a concise visual target similar to:

> A temperate continuous world with broad oceans, readable sand coasts, open
> grass lowlands, wooded rolling uplands, occasional exposed stone, and enough
> large-scale variation that several seeds do not read as the same island.

Also approve the terrain scale and the seed/region review matrix. Do not land a
profile enum with placeholder terrain while those decisions remain open.

## Fixed Boundaries

- Keep `overworld` and all Java 1.17.1 output locks byte-exact.
- Keep scheduler admission, readiness, lighting, persistence, publication,
  and unload policy profile-neutral.
- Append the `mclone-overworld-v1` persisted tag without renumbering existing
  identities; persistence hits continue to win.
- Keep terrain rules in shared Rust. Apps and browser TypeScript transport and
  display the descriptor without interpreting it.
- Isolate terrain, biome, surface, and decoration seed domains.
- Treat new-profile fixtures as internal regression evidence, not a shipped
  compatibility promise.
- Add concrete profile modules only as behavior needs them. Do not introduce a
  plugin ABI, density DSL, node graph, or speculative trait hierarchy.

## Execution Checklist

### Slice 0: design, inventory, and baselines

- [x] Approve the product rule, scale vocabulary, and non-goals.
- [x] Select at least three contrasting seeds, including a negative seed, and
  positive/negative region centers.
- [x] Define only the structured field values and stable seed domains needed
  by the first terrain rule.
- [x] Classify relevant reference Overworld and Small Island mechanisms as
  reuse-as-is, extraction candidate with output locks, or profile-owned.
- [x] Identify affected compatibility-safety ledger rows.
- [x] Capture clean Overworld oracle/order results and Small Island
  fingerprints/cards before shared changes.
- [x] Define field hashes/ranges, terrain distributions, and a performance
  comparison command.

Gate: approve the aesthetic target, reuse inventory, evidence commands, and
review matrix. No generator identity lands in this slice.

Execution record 2026-07-18:

- approved the quoted temperate-world product rule above. The first scale
  vocabulary is broad land/ocean regions over roughly 768-2,048 blocks,
  coastline transitions over tens of blocks, rolling relief over roughly
  96-384 blocks, sea level 63, lowlands near 64-73, and first-slice uplands
  near 74-96. These are design ranges, not frozen output values;
- selected seeds `12345`, `-98765`, and `8675309`. The core review matrix is
  seed `12345` at chunk centers `(0,0)`, `(96,-64)`, and `(-128,80)`, plus the
  other two seeds at `(0,0)`. This distinguishes seed variation from spatial
  continuity without requiring a full seed-by-center cross product;
- limited the first production sample to signed `continentalness`, signed
  `relief`, and derived integer `surface_y`. One seeded sampler exposes pure
  point sampling and a bounded row-major region request/response using the
  same implementation. Temperature, moisture, ridges, rivers, biome choice,
  and 3D density remain absent;
- reserved independent stable domains for continentalness and relief. The
  exact domain constants land with tests in Slice 1 and then become part of
  the internal profile fingerprint;
- defined the regional evidence as stable field/grid hashes, min/max, height
  percentiles, land/water/shore column counts, slope counts at one- and
  three-block thresholds, and generation elapsed time. Slice 3 adds biome and
  feature proportions;
- the compatibility ledger rows in scope are reference-locked `overworld`,
  internal-mutable `small-island-v1`, and planned-unallocated
  `mclone-overworld-v1`. Flat Grass and authored-only behavior are
  non-regression checks but require no rule update in this slice.

Reuse inventory:

| Existing mechanism | Slice 1 decision | Evidence boundary |
|---|---|---|
| `SeedDomain` and `ValueNoise2d` | reuse as-is | pinned positive/negative coordinate tests |
| `GeneratedChunk`, mutable buffer, biome payload, heightmaps, ticks | reuse as-is | worldgen and server suites |
| `ChunkGenerationPlan::target_only` | reuse for the undecorated foundation | exact plan tests; scheduler remains generic |
| descriptor, worker frame, persistence, and catalog transport | extend closed identity dispatch | preserve old labels/tags/bytes |
| Small Island radial envelope, spawn patch, and material rules | keep profile-owned | existing island fingerprints/cards |
| reference `NoiseSampler`, Java PRNG, biome and surface tables | keep reference-owned | Java oracle and order locks |
| octave/region helper | do not extract yet | reconsider with two working callers in Slice 2 |
| feature-region execution and placed/configured features | defer reuse to Slice 3 | Small Island dependency/order locks |

Clean baseline at commit `9b9910b3` on arm64 macOS 26.5.1, Rust 1.92.0:

- `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen`: 226
  passed, zero failed, one known active-gauntlet test ignored;
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`: 470 passed,
  zero failed;
- release `worldgen_perf`, seed `12345`, center `(0,0)`, radius 1, three
  iterations: Surface 1,036.458 chunks/s, cold Features 149.023 target
  chunks/s, warm Features 492.162 target chunks/s. Use the same command after
  shared changes; add the mclone target-only phase when Slice 1 lands;
- inspected complete render-distance-16 Small Island cards for all three
  review seeds under `/tmp/mclone-worldgen-showcase`. Every receipt reported
  1,225/1,225 client-visible and target-ready chunks before capture. Warmup was
  1,388-1,422 frames and 7.79-7.95 seconds. The cards preserve the expected
  bounded island, surrounding water, coastline, relief, and decoration while
  visibly varying the shoreline between seeds.

### Slice 1: first continuous terrain caller

- [x] Add the profile label/tag and catalog, protocol, persistence, and
  descriptor round trips without changing old values.
- [x] Add pure absolute-coordinate single-point and bounded-region sampling
  through the same implementation used by chunk generation.
- [x] Generate continuous ocean, coast, lowland, and rolling-upland terrain
  with correct water, blocks, biomes, heightmaps, and ticks.
- [x] Add generator-aware safe spawn over dry traversable terrain.
- [x] Carry descriptor-keyed sessions through native workers and browser
  codecs.
- [x] Pin seam, negative-coordinate, repeated/reversed/partitioned batch, and
  native/browser-equivalence fingerprints.

Gate: the profile produces real continuous terrain, opens at a safe spawn, and
leaves reference Overworld output exact.

Execution record 2026-07-18:

- appended stored binary tag `4` for `mclone-overworld-v1` while preserving
  tags `0..=3`, routed the identity through catalogs, CLI startup, native
  jobs, WASM frames, and the production browser Worker, and added it to the
  shared procedural-profile cycle;
- added one profile-owned sampler with pure point requests and bounded
  row-major region requests. `continentalness`, `relief`, and `surface_y`
  use independent stable seed domains and the existing `SeedDomain` and
  `ValueNoise2d` mechanisms without changing those shared primitives;
- generated unbounded ocean, sand coast, grass lowland, and rolling upland
  columns with existing ocean/beach/plains biome IDs, canonical heightmaps,
  an empty tick payload, and a target-only generation plan. The profile has no
  decoration, carvers, structures, or generator prerequisites yet;
- added a deterministic profile-owned spawn search for dry grass at least
  five blocks above sea level. Native and browser startup both use the
  resulting authoritative spawn rather than interpreting terrain rules;
- pinned seam and negative-coordinate samples, point/region equivalence,
  repeated/reversed/partitioned requests, different-seed output, exact worker
  frames, and region fingerprints `3503757461131335250`,
  `13898607341532105566`, and `17278483164450982141` for the three review
  seeds;
- full validation passed: worldgen 232 passed with one known ignored test,
  server 473 passed, app runtime 290 passed plus integrations, native client
  170 passed, web client 11 passed plus ownership locks, WASM check,
  formatting, and TypeScript check. The production Web Worker app-loop smoke
  selected the profile reported by Rust, generated and rendered 49 resident
  chunks, and reached generated ground at seed `12345`;
- the release target-only probe produced 2,894.796 chunks/s at seed `12345`,
  center `(0,0)`, radius 1, three iterations. This dirty-tree measurement is a
  comparison receipt, not a performance promise;
- inspected the first complete native card at
  `/tmp/mclone-worldgen-showcase/mclone-overworld-v1-seed-12345-card.png`.
  It shows coherent broad rolling grass terrain at the spawn region. The
  sparse palette is intentional; the approved multi-seed/region review and
  production field maps remain Review 1 rather than being inferred from this
  first drawable milestone.

### Review 1: terrain before abstraction

- [x] Render the approved seed/center matrix before decoration breadth.
- [x] Map every live field and derived height using the production sampler.
- [x] Inspect terrain scale, repetition, lattice artifacts, shoreline noise,
  and spawn quality.
- [x] Record whether to accept, tune, or add one necessary field.

Gate: obtain human review before refactoring or adding content families.

Execution record 2026-07-18:

- added `pnpm native:worldgen:fields`, which requests a bounded strided region
  from the production sampler and writes continentalness, relief, and derived
  surface-height PNGs plus a schema-versioned quantitative receipt. Review
  requests use a 385-by-385 grid, 16-block spacing, and a 3,072-block radius;
- generalized card receipts with commit/dirty and bounded/unbounded coverage
  facts, renamed the low view to be profile-neutral, and included center chunk
  in output names. This prevented the three regions for seed `12345` from
  overwriting one another during the review itself;
- inspected the approved five-card matrix and added two cards at the actual
  production-selected spawn chunks, `(18,-21)` for seed `-98765` and `(30,-1)`
  for seed `8675309`. Every final card at commit `7a7b9247` reported all
  1,225 client-visible and target-ready chunks before capture. Warmup was
  1,342-1,379 frames and 7.84-8.09 seconds;
- the initial cards accepted the macro land/ocean scale and seed variation but
  rejected overly clean concentric relief terraces. One intentional tune added
  a low-weight 48-block octave to the existing `relief` field; no new exposed
  field or scheduler/runtime contract was added. Follow-up cards show more
  organic plains, slopes, and beach transitions without changing the broad
  continental shapes;
- clean field-revision-2 receipts span surface Y 45-89. Their P10/P50/P90
  heights are `50/64/78`, `55/70/83`, and `54/64/79`; water/shore/dry counts
  are `66533/21063/60629`, `50565/14161/83499`, and
  `63682/28238/56305` for seeds `12345`, `-98765`, and `8675309`.
  Broad-map sample fingerprints are `646172283875503124`,
  `11601619166430677986`, and `8202241311963224599`;
- accepted remaining first-slice defects: many intentionally selected matrix
  centers are open ocean, exposed block contours remain visible without
  decoration, the biome/material vocabulary is minimal, and there are no
  mountains, valleys, rivers, or local landmarks. No repeating macro tile or
  axis-aligned lattice seam was visible in the three broad field maps;
- release target-only throughput after the tune is 2,767.279 chunks/s versus
  the pre-tune 2,894.796 chunks/s probe, a 4.4% cost for one extra value-noise
  component per terrain sample with no new allocations, payload, or cache.
  Full worldgen
  (232 passed, one ignored) and server (473 passed) suites and the production
  browser Worker app-loop passed after the tune.

### Slice 2: first reuse and refactoring checkpoint

- [x] Compare working terrain code with Small Island and reference Overworld.
- [x] Extract only mechanisms with two concrete consumers or an already
  frozen boundary.
- [x] Keep radial island composition, mclone macro composition, rule tables,
  and seed domains profile-owned.
- [x] Separate behavior-preserving extraction from intentional tuning.
- [x] Re-run exact Overworld locks and both original-profile fingerprints
  after every shared extraction.
- [x] Reject helpers that add material allocation, payload, or cache cost.

Gate: every extraction names its consumers and evidence. Extracting nothing is
an acceptable review result.

Execution record 2026-07-18:

- extracted one mechanism: `sample_column_biome_payload` owns the canonical
  Y-major quart-section, Z-major, then X-major 2.5D chunk payload traversal.
  Its concrete consumers are Small Island and Mclone Overworld; each supplies
  its own absolute-coordinate biome rule. A direct boundary test pins sample
  coordinates, count, ordering, and vertical replication;
- preserved the original loop count and one exact-capacity `Vec` allocation.
  The helper adds no payload, cache, profile dispatch, or runtime state. The
  release target-only probe produced 2,934.743 chunks/s versus the 2,767.279
  pre-refactor receipt, which is sufficient evidence of no material
  throughput regression rather than a claim of improvement;
- deliberately did not extract the shared-looking biome thresholds, material
  columns, octave composition, interpolation helpers, or spawn searches.
  Those are profile policy, already differ at important edges, or need the
  Slice 3 surface/decoration caller before a useful boundary is visible. The
  reference Overworld retains its three-dimensional `NoiseBiomeSource`,
  Java-owned surface rules, and PRNG/order path;
- full worldgen validation passed 233 tests with the known gauntlet ignored;
  server validation passed 473 tests. This re-ran reference oracle locks,
  Small Island seam/order and seed fingerprints, Mclone field fingerprints,
  worker codec/partition locks, and scheduler plan tests without changing an
  expected value;
- inspected clean commit `3df1890a` RD16 cards for both consumers at seed
  `12345`. Each reached 1,225/1,225 visible and target-ready chunks with zero
  target stream or render work pending. Small Island warmed in 1,375 frames
  and 7.915 seconds; Mclone warmed in 1,356 frames and 7.805 seconds. Both
  retained their pre-refactor terrain, biome, and decoration appearance;
- no safety-ledger disposition changed. This was an output-identical shared
  implementation refactor across two `internal-mutable` profiles; the
  `reference-locked` path was validated but did not adopt the helper.

### Slice 3: first terrain language

Pre-slice inventory 2026-07-18:

| Candidate | Decision for this slice | Ownership/evidence |
|---|---|---|
| biome choice | add a concrete Mclone rule over the production terrain sample | Mclone owns ocean/beach/open-lowland/wooded-upland thresholds and uses existing biome IDs |
| surface material writing | keep recipes concrete over `MutableChunkBlockBuffer` | Mclone owns grass/soil, sand beach, gravel floor, and stone-exposure rules; do not generalize the similar Small Island writer yet |
| feature placement | reuse `FeatureRegion`, configured/placed features, and the ordered executor | shared mechanisms already have cross-chunk and PRNG/order locks |
| feature footprint | reuse `ChunkGenerationPlan::feature_region` through a named Mclone plan | exact 3x3 backend and 5x5 Surface-input facts; scheduler only consumes the declaration |
| feature recipes and random seed | add Mclone-owned lowland/upland tables and a new domain | do not invoke vanilla biome tables or Small Island's complete table/domain |
| dependency cache | add the smallest concrete Mclone cache beside the existing callers | compare all three caches in Slice 4 before extracting lifecycle policy |
| reference Overworld | do not modify its biome, surface, or decoration composition | full oracle, random-order, worker, and scheduler locks remain mandatory |

Land biome/surface output as one target-only commit and inspect pixels before
adding feature dependencies. Land decoration, worker/cache routing, and exact
plan changes as a separate commit. This keeps intentional material tuning
separate from the later execution-boundary change.

- [x] Distinguish open lowlands and wooded uplands with the smallest useful
  profile-owned biome rule and existing biome IDs.
- [x] Add mclone-owned grass/soil, beach, ocean-floor, and exposed-stone
  surface recipes through shared writing mechanisms.
- [x] Add an independent decoration domain and a small table of existing
  configured features such as trees, grass, and flowers.
- [x] Declare exact feature work and prerequisites through
  `ChunkGenerationPlan` without scheduler branches.
- [x] Prove cross-chunk placement, cache reuse, request-order independence,
  and safe-spawn preservation.
- [x] Pin field, surface, ordered-feature, and final-chunk fingerprints.

Gate: terrain, biome choice, surface, and decoration have separate ownership
and form a recognizable first original world.

Execution record 2026-07-18:

- added Mclone-owned ocean, beach, open-lowland, and wooded-upland biome
  classification over the production terrain sample. Surface selection is a
  separate Mclone rule with gravel ocean floor, sand beach, grass over dirt,
  and sparse high-relief exposed stone recipes;
- reused the existing configured/placed feature vocabulary, `FeatureRegion`,
  ordered executor, and typed planning contract. Mclone owns only its
  independent decoration seed domain and its lowland/upland tables for oak
  trees, grass, dandelions, and poppies;
- changed the profile plan from target-only output to an exact 3-by-3 feature
  work band and 5-by-5 Surface prerequisite band. The server scheduler and
  worker codecs consume those facts generically; no terrain rule or profile
  branch entered scheduling policy;
- added a concrete Mclone dependency cache with exact overlap evidence: a
  one-chunk move reuses 20 of 25 Surface inputs and generates five. Combined,
  reversed, native, encoded-worker, and partitioned target requests produce
  identical decorated chunks, and the existing shared region tests retain
  the cross-chunk write boundary;
- pinned the three broad field fingerprints, three surface/biome chunk
  fingerprints, a 9-by-9 ordered decoration distribution, and final payload.
  After Review 2 tuning, seed `12345` places 57 oak logs, 3,326 grass, 383
  dandelions, and 223 poppies across that region, with final fingerprint
  `6043725934403648447`;
- retained the generator-owned spawn query and proved the server still chooses
  a loaded dry spawn after features. Full validation passed 242 worldgen tests
  with one known gauntlet ignored and 473 server tests. The production browser
  Worker app-loop also completed the dependency-bearing plan and rendered the
  generated result;
- the clean release probe at commit `0dfc5148`, seed `12345`, radius one, and
  three iterations measured 2,787.828 target-only Surface chunks/s, 700.838
  cold decorated target chunks/s, and 12,682.010 warm decorated target
  chunks/s. Cold batches generated every declared input; warm batches hit all
  147 requested inputs.

### Review 2: complete foundation

- [x] Re-run all field maps and landscape cards.
- [x] Review biome proportions, feature density, coast readability, exposed
  stone, repetition, and performance.
- [x] Compare Small Island beside mclone output to find shared mechanisms
  without requiring similar results.
- [x] Bound defects for later mountain/valley or river work.

Gate: obtain human acceptance before the final refactor and host rollout.

Execution record 2026-07-18:

- extended `pnpm native:worldgen:fields` with production-backed biome and
  surface-recipe panels. Schema-2 receipts record the field and decoration
  revisions, per-rule distributions, and a deterministic terrain-language
  fingerprint without reconstructing the rules in the tool;
- inspected final clean broad maps for seeds `12345`, `-98765`, and `8675309`.
  Their ocean/beach/open-lowland/wooded-upland counts are respectively
  `66533/21063/31618/29011`, `50565/14161/21161/62338`, and
  `63682/28238/29898/26407` out of 148,225 samples. The resulting language
  fingerprints are `9809547655668137720`, `16182128992402164837`, and
  `15593051643100665937`;
- exposed stone is deliberately a local accent: 426, 2,129, and 310 sampled
  columns across the three seeds. Beaches remain broad and readable, and the
  biome maps show coherent upland forest bands rather than per-chunk noise;
- the first decorated card pass rejected flowers in every eligible chunk as
  uniformly noisy. Revision 2 retained the shared placed-feature executor but
  added Mclone-owned chance decorators. Final cards show localized flower
  patches, open grass, wooded uplands, readable coasts, and sparse stone;
- inspected the approved five-region matrix plus actual spawns `(18,-21)` for
  seed `-98765` and `(30,-1)` for seed `8675309`. Every clean commit
  `0dfc5148` RD16 receipt reports 1,225/1,225 visible and target-ready chunks,
  zero target stream/render work pending, 1,356-1,396 warmup frames, and
  7.979-8.503 seconds;
- comparison with Small Island confirms the right similarity boundary: both
  reuse column-biome traversal, typed feature planning, feature-region
  execution, and placed-feature vocabulary, while island radial shape,
  Mclone macro fields, surface recipes, seed domains, and feature tables stay
  intentionally different;
- accepted defects for later content slices are the limited four-biome
  vocabulary, height-threshold forest borders, one tree family, no climate
  fields, no mountains/valleys/rivers/wetlands, no caves, and no structures.
  Several intentionally selected matrix centers remain open ocean; that is
  useful coverage, not a spawn or generation failure.

### Slice 4: second reuse and boundary checkpoint

- [x] Compare surface writers, feature recipes, region setup, cache ownership,
  and spawn queries across the three procedural profiles.
- [x] Extract only data/mechanism boundaries now shared by real callers.
- [x] Reject abstractions that hide different profile rules behind switches.
- [x] Confirm vanilla rule tables/PRNG assumptions did not enter mclone code
  and terrain knowledge did not enter scheduler or platform code.
- [x] Re-run output locks and performance comparisons after refactors.

Gate: concrete profile modules remain clear, shared mechanisms stay small, and
the next terrain family will not copy known common policy.

Execution record 2026-07-18:

- compared the live Small Island, Mclone, and reference Overworld callers.
  Small Island and Mclone had the same seed reset, externally supplied input
  insertion, sorted Surface-input assembly, overlap reuse, plan-bounded
  retention, retained-input response, and empty-plan clearing lifecycle;
- extracted that exact two-caller lifecycle into the internal
  `SurfaceDependencyCache`. Each profile retains its public wrapper/report and
  supplies its own pure plan and Surface generator. Direct mechanism tests pin
  seed reset, 20-of-25 overlap reuse, five new inputs, seeded-input pruning,
  retained count, ordering, and empty-plan clearing;
- deliberately left reference Overworld on its existing cache. It reuses a
  heavyweight generator and biome source, produces liquid-carved inputs,
  records phase timing, has a three-dimensional biome payload, and preserves
  residency for an empty request. Pulling it into the new helper would require
  timing hooks and lifecycle switches that obscure rather than unify policy;
- rejected a generic surface-recipe writer. Small Island's radial stone floor,
  sand band, grass cap, and post-decoration spawn restoration are different
  rules from Mclone's gravel floor, wider beach, grass/soil recipe, and relief
  exposure. The common block-buffer setter is already the honest mechanism;
- kept configured feature implementations, `FeatureRegion`, ordered traversal,
  and typed plans shared, while retaining complete feature tables, decoration
  seed domains, biome resolution, target extraction, and spawn rules in each
  profile. Region construction is only a few neutral lines and did not warrant
  a callback abstraction;
- full validation after the output-identical extraction passed 244 worldgen
  tests with one known gauntlet ignored, 473 server tests, and workspace
  formatting. Existing reference oracle locks, both profile fingerprints,
  cache/order/partition tests, scheduler plans, and worker codecs remained
  unchanged;
- clean commit `55f4e13d` cards for Small Island and Mclone each reported
  1,225/1,225 visible and target-ready chunks with zero target stream/render
  work pending. Both retained their reviewed pixels. The production browser
  Worker app-loop also passed for Mclone after the extraction;
- the clean release probe measured 2,939.709 Mclone Surface chunks/s, 707.597
  cold decorated target chunks/s, and 12,877.588 warm target chunks/s versus
  2,787.828, 700.838, and 12,682.010 before extraction. Treat the small
  variation as evidence of no material regression, not an optimization claim;
- no compatibility-safety disposition changed. Both consumers remain
  internal-mutable, while the reference-locked path validated its output but
  did not adopt the helper.

### Slice 5: persistence, hosts, and closeout

- [ ] Cover shared world creation/catalog selection and descriptor display.
- [ ] Prove SQLite and IndexedDB reopen before generating unseen chunks.
- [ ] Prove native threads, production Web Workers, integrated/dedicated
  startup, remote authoritative consumption, dimension replacement, and warm
  previews.
- [ ] Validate desktop pixels first, then required browser, Android, and XR
  lanes affected by selection/startup changes.
- [ ] Record review receipts, distribution facts, commands, and performance.
- [ ] Update the safety ledger and living topic with the new internal state,
  accepted defects, shared extractions, and selected next tactical.

Gate: every host carries the same stored identity and authoritative output,
and the next content slice is explicitly chosen.

## Evidence And Validation

Screenshots are design evidence; deterministic field and chunk facts are the
regression oracle. Each review receipt records commit/dirty state, profile,
seed, dimension, center, field revision, distributions, generator plan,
worker mode, timing, readiness, and camera facts. Disposable maps and cards
stay under `/tmp`.

Minimum code validation after behavior lands:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --lib \
  -p mclone-worldgen -p mclone-server -p mclone-web-client \
  --target wasm32-unknown-unknown
pnpm native:web:typecheck
```

Run pixel captures at the first drawable milestone and after each visual slice.
Run live browser and platform smokes when profile selection or startup changes.

## Follow-Up Boundary

Use the closeout evidence to choose a mountain/valley tactical, a river/wetland
tactical, or a tooling/refactor tactical. Do not silently expand this one into
caves or structures.

## Related

- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`187-generator-profile-flat-grass-and-seeded-island.md`](187-generator-profile-flat-grass-and-seeded-island.md)
- [`191-guarded-generation-planning-refactor.md`](191-guarded-generation-planning-refactor.md)
- [`../worldgen-status.md`](../worldgen-status.md)
