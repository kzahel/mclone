# Tactical 243: LOD-Native Vegetation Exact Foundation

Status: active 2026-07-25.

Topic: `lod-native-vegetation`

Workstream: shared Rust Mclone Overworld vegetation semantics, bounded
deterministic planning, exact block realization, feature-stage migration,
periodic topology, worker equivalence, and rendered worldgen review.

## Objective

Replace the original Mclone profile's vanilla-shaped tree placement with one
worldgen-owned semantic record path:

```text
production structured terrain and forest intent
  -> bounded deterministic tree candidates
  -> local-priority spacing
  -> stable tree records
  -> clipped exact broadleaf/conifer/acacia blocks
```

The landed slice must make each natural tree's position, family, resolved
silhouette, variation, bounds, and identity independent of chunk request
order, batching, cache residency, thread/Worker execution, and the remaining
placed-feature random sequence.

This tactical establishes the exact source of truth required by later Terrain
Lab summaries/proxies and in-game Far LOD. It does not implement those
presentation consumers, player-grown trees, edit overlays, persistent record
storage, wind, falling-tree behavior, or a generic natural-feature enum.

## Binding Planner Decision

Planning uses deterministic local-priority inhibition:

1. each aligned planning cell exposes a fixed number of candidate slots;
2. position, density acceptance, conflict priority, family/archetype,
   silhouette, and residual variation use independent hash domains;
3. terrain and forest intent decide preliminary eligibility without consulting
   another candidate's survival;
4. conflicts use a symmetric family-aware spacing predicate over integer block
   coordinates and topology-aware displacement;
5. a candidate survives exactly when no preliminarily eligible conflicting
   candidate has a better lexicographic `(priority, id)`; and
6. the maximum conflict distance defines the complete finite neighbor halo.

The implementation must never replace this with greedy traversal over already
accepted candidates. The local rule intentionally permits an occasional extra
opening in exchange for partition-independent output and fixed work.

Initial cell size, slot count, spacing, density, and silhouette ranges are
measured tuning parameters. They may change before acceptance, but the query
and determinism contracts do not.

## Shared Ownership

- `mclone-worldgen::levelgen::mclone_overworld::vegetation` owns source
  identity, forest intent, seed domains, canonical IDs, records, planning,
  topology-aware lifted occurrences, exact realization, and focused tests.
- `McloneOverworldFeatureDependencyCache` owns optional bounded vegetation
  cache residency beside its existing structured-stream cache.
- `ChunkGenerationPlan` continues to declare the mutable Surface dependency
  footprint needed by retained low vegetation. Tree realization fills that
  complete footprint once before center-by-center low decoration.
- app crates, browser TypeScript, render crates, and the reference-locked
  `overworld` profile own none of the generation policy.

The planner samples the same production structured terrain semantics used by
surface generation and reuses bounded stream metadata. It does not materialize
`GeneratedChunk` values merely to choose records.

## Exact Realization Contract

`McloneTreeRecord` owns:

- canonical identity and base;
- family and archetype;
- trunk/crown dimensions and orientation;
- landmark rank and residual variant seed; and
- conservative canonical bounds.

A topology-aware query returns an occurrence carrying the canonical record and
the working X lift/bounds needed by the caller. Canonical identity is never
duplicated merely because a seam lift differs.

The realizer:

- writes only through the supplied mutable region and clips at its boundary;
- visits occurrences in stable ID order;
- never selects another family/base/dimension;
- never runs a configured decorator or shared decoration random stream;
- uses bounded original grammars for broadleaf, conifer, and acacia; and
- keeps every emitted block inside the record's conservative bounds.

Ordinary grass, flowers, ferns, tall grass, and berries remain on the current
placed-feature path. Their seed-index behavior must be deliberately preserved
or explicitly recorded as an intentional decoration revision change.

## Performance And Correctness Gates

- fixed candidate work per planning cell and a fixed conflict halo;
- coarse or large-area callers are not introduced in this exact tactical;
- one large record query equals every partitioned/reversed/randomized query;
- cold and warm caches return byte-identical sorted occurrences;
- negative coordinates and cell boundaries have direct fixtures;
- the 6,144-block cylinder has canonical IDs, shortest periodic conflict
  displacement, lifted seam bounds, and identical repeated exact chunks;
- native and `wasm32-unknown-unknown` builds exercise the same Rust planner;
- reference `overworld` oracle/random-order output remains unchanged; and
- current same-host Mclone cold/warm generation cost is measured before and
  after the migration.

More than 25 percent cold or warm generation regression requires focused
profiling and explanation. A twofold regression blocks acceptance.

## Slice Plan

### Slice 0: tactical and baseline

- [x] Record the bounded contract and explicit exclusions.
- [x] Pin current representative tree counts/fingerprints and same-host
  generation throughput.

Gate: the migration has a reproducible pre-change output/performance baseline.

Baseline record:

- commit `06eefecb`, seed `12345`, plane topology, center chunk `(-5,0)`,
  radius three, 49 targets, and three release iterations;
- representative final decoration counts remain oak logs `111`, spruce logs
  `621`, acacia logs `240`, grass `1,985`, fern `376`, large fern `6`, berry
  bush `11`, tall grass `42`, dandelion `73`, and poppy `54`;
- the representative decorated payload fingerprint remains
  `11352546092800642539`;
- Mclone cold feature generation produced 147 targets in `195.262 ms`, or
  `752.834` target chunks/s;
- Mclone warm feature generation produced 147 targets in `44.893 ms`, or
  `3,274.460` target chunks/s; and
- the complete machine-readable baseline is
  `/tmp/mclone-vegetation-baseline.json`, intentionally outside the repository.

### Slice 1: semantic records and bounded planner

- [x] Add typed source identity, forest intent, family/archetype, ID, record,
  canonical bounds, and lifted occurrence.
- [x] Implement independent hash domains and fixed planning cells.
- [x] Implement preliminary terrain/density/family/silhouette decisions.
- [x] Implement symmetric integer local-priority inhibition.
- [x] Prove single/partitioned/reversed queries, negative coordinates,
  boundaries, and periodic seam identity.
- [x] Add an optional bounded canonical-cell cache and prove cold/warm equality.

Gate: sorted semantic occurrences are a pure bounded function of source and
query bounds.

Execution record:

- `vegetation.rs` owns a typed source, quantized density boundary, 32-block
  cells, 32 fixed candidate slots, independent hash lanes, canonical IDs,
  conservative bounds, and explicit lifted occurrences;
- the conflict halo is derived from the seven-block maximum family spacing,
  and survival compares only preliminarily eligible candidates through
  topology-aware integer squared distance;
- structured candidate samples reuse the production stream-plan cache through
  the same landform reconstruction helper as exact surface diagnostics;
- the optional 4,096-cell FIFO cache retains preliminary semantic candidates
  only and reports request/hit/miss/retained counts;
- representative seed `12345` regions pin 149 records: 30 broadleaf, 79
  conifer, and 40 acacia, fingerprint `10101728880902621857`; and
- all 338 active `mclone-worldgen` tests pass, with one pre-existing ignored
  gauntlet, and `mclone-worldgen` checks for `wasm32-unknown-unknown`.

### Slice 2: exact broadleaf milestone

- [x] Implement clipped broadleaf realization from records.
- [x] Realize planned trees once before retained low vegetation.
- [x] Remove broadleaf selection from the Mclone placed-feature tables without
  changing reference `overworld`.
- [x] Bump vegetation and decoration revisions and update focused fixtures.
- [x] Capture and inspect meadow, woodland, transition, and stream-edge pixels.

Gate: every exact broadleaf block comes from a record and the first drawable
record/blocks agreement is inspected.

Execution record:

- `mclone-overworld-v1-vegetation-2` expands record bounds through the dirt
  support block and drives a rounded broadleaf grammar entirely from the
  record's base, resolved dimensions, and variant seed;
- one stable-ID-ordered occurrence union fills the complete retained feature
  footprint through clipped region reads/writes before center-local low
  vegetation runs;
- the Mclone plains and woodland tables no longer contain oak configured
  features, while their remaining plants retain feature index one and the
  reference `overworld` tables are unchanged;
- `mclone-overworld-v1-decoration-13` pins 199 oak logs in the representative
  exact receipt and a decorated payload fingerprint of
  `1565593964556829038`; the integration fixture also checks every planned
  broadleaf trunk block against its record;
- vegetation cell request/hit/miss/residency metrics now travel with the
  feature dependency cache report;
- inspected offscreen captures at
  `/tmp/mclone-vegetation-meadow.png`,
  `/tmp/mclone-vegetation-woodland.png`,
  `/tmp/mclone-vegetation-transition.png`, and
  `/tmp/mclone-vegetation-stream-edge.png` show sparse meadow cover, dense but
  traversable rounded crowns, a bounded density transition, and tree-free
  channel/bank space without terrain or crown clipping; and
- all 338 active `mclone-worldgen` tests pass, with one pre-existing ignored
  gauntlet, and `mclone-worldgen` checks for `wasm32-unknown-unknown`.

### Slice 3: conifer and acacia completion

- [x] Add layered conifer and forked/flat-crowned acacia realizers.
- [x] Remove the remaining Mclone tree configured features.
- [x] Prove every emitted tree family, base, dimension, orientation, and bound
  agrees with its record.
- [x] Capture and inspect conifer, steppe, mixed transition, and cylinder-seam
  pixels.

Gate: Mclone natural tree placement no longer calls the vanilla-shaped
decorator path.

Execution record:

- exact realization now visits every occurrence once in stable ID order and
  dispatches only its resolved rounded broadleaf, layered conifer, or oriented
  forked acacia grammar;
- the conifer grammar derives its complete tapered crown depth from the record,
  while the acacia grammar derives both connected forks and flat crowns from
  record height, radius, orientation, and variant seed;
- runtime bounds assertions cover every attempted tree block, and the
  representative integration receipt checks each family's vertical trunk plus
  every reviewed acacia main-fork block against the source record;
- all Mclone biome tables now contain only retained low vegetation, a focused
  test rejects any remaining basic tree, tree, or random tree selector, and
  `mclone-overworld-v1-decoration-14` preserves their vacated feature index;
- the final representative exact receipt contains oak/spruce/acacia log counts
  `199/768/294`, retained low-vegetation counts
  `1934/272/0/10/45/82/49`, and decorated payload fingerprint
  `1780731906598478474`;
- inspected `/tmp/mclone-vegetation-conifer.png` shows the mixed
  broadleaf/conifer transition and bounded tiered spruce crowns,
  `/tmp/mclone-vegetation-steppe-record.png` shows connected oriented forks
  and distinct flat crowns, and
  `/tmp/mclone-vegetation-cylinder-seam.png` shows canonical woodland trees at
  cylinder chunk `(383,-128)` without a seam discontinuity; and
- all 338 active `mclone-worldgen` tests pass, with one pre-existing ignored
  gauntlet, and `mclone-worldgen` checks for `wasm32-unknown-unknown`.

### Slice 4: worker, integration, and performance closeout

- [ ] Prove feature-batch order/partition/cache equivalence over representative
  multi-chunk regions.
- [ ] Prove native/browser Worker-equivalent chunk payloads and wasm checks.
- [ ] Re-run periodic profile, persistence/scheduler, and reference-overworld
  regression gates selected by the affected contracts.
- [ ] Record final cold/warm throughput and cache metrics.
- [ ] Update the living topic with landed types, revisions, evidence, and the
  concrete Terrain Lab handoff.

Gate: the exact foundation is complete, documented, measured, and ready for
the separate summary/proxy tactical.

## Commit Plan

1. Record this tactical and pre-change baseline.
2. Add semantic records, deterministic planner, and fixtures.
3. Integrate the broadleaf exact milestone and inspect it.
4. Complete conifer/acacia migration and exact output revisions.
5. Close worker/topology/performance evidence and reconcile documentation.

Every implementation commit uses:

```text
Topic: lod-native-vegetation
```

## Stop Conditions

Stop and correct the implementation if:

- survival depends on a competing candidate's survival or enumeration order;
- a query reads or creates complete generated chunks to select trees;
- exact and future LOD callers would need to derive family or dimensions
  independently;
- cache contents alter a record;
- canonical and lifted seam identity are conflated;
- tree writes escape conservative bounds or the owned mutable footprint;
- the reference `overworld` output changes; or
- the first exact caller requires app-, renderer-, WGSL-, or TypeScript-owned
  vegetation policy.
