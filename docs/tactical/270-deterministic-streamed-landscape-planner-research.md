# Tactical 270: Deterministic Streamed Landscape Planner Research

Status: **Human Review R0 accepted 2026-07-27; Phase 1 neutral invariance
harness and controls completed at `14d832b8`. Paused at the authorization
boundary before Phase 2.** Research only; no Candidate B/C/D implementation,
Terrain Lab integration, or production terrain integration is authorized.

Topics:

- `deterministic-streamed-landscape-planning`
- `mclone-macro-landscape-planning`
- `bounded-world-topology`

Workstream: primary-source research, canonical plan identity, streamed
relational landscape candidates, path/order/cache/window invariance,
topology-aware boundaries, bounded performance, and explicit fallback.

## Motivation

Tactical 267 showed that a bounded hybrid plan can replace much of Mclone's
repeated scalar-hill texture with persistent high/low axes, drainage
hierarchy, basins, divides, and quiet country. Tactical 268 made the plan
interactive and honestly exposed its fixed 6,144-block plane domain.

That review clarified the missing production proof. Rebuilding the current
solve around each viewport could make one absolute location change when the
window, exploration path, request order, cache, or Worker schedule changes.
Classic Alpha/Beta delayed population demonstrates why discovery-order writes
are unacceptable even when individual feature seeds are deterministic.

The visual promise warrants a research and experimental pass. It does not
warrant adapting the fixed-window algorithm directly into production.

## Objective

Determine whether a novel relational macro-landscape planner can satisfy all
of the following at once:

1. visibly stronger geographic structure than the coordinate-pure fallback;
2. exact independence from request, travel path, schedule, cache, viewport,
   chunk partition, and topology lift;
3. exact semantic agreement across independently requested plan regions;
4. bounded construction, lookup, dependency, memory, and far-summary cost;
5. plane, 6,144-block X-cylinder, and flat-torus semantics through one
   dimension-scoped topology contract; and
6. a maintainable identity, cache, revision, and diagnostic model.

Finish with one explicit decision:

- promote one representation to a separate production-integration proposal;
- narrow it to one bounded feature family;
- continue one precisely bounded experiment; or
- reject it and adopt coordinate-pure terrain plus bounded deterministic
  starts.

The fallback is a successful result, not a failed tactical.

## Non-Goals

This tactical does not:

- change Mclone Overworld field revision 21, rivers, lakes, coasts, surfaces,
  ecology, geology, exact chunks, or persistence;
- make the Tactical 267 reconstruction a production dependency;
- hide fixed-window disagreement through feathering or material transitions;
- implement a universal graph runtime;
- claim physically exact erosion or an exact infinite watershed;
- add selective 3D density;
- choose release compatibility for an experimental planner;
- deploy a game-facing planner; or
- treat a visually attractive screenshot as a determinism waiver.

## Binding Research Contract

The living methodology is
[`deterministic-streamed-landscape-planning.md`](../topics/deterministic-streamed-landscape-planning.md).
This tactical may add evidence and decisions to it but may not weaken its
invariants silently.

For a fixed stored descriptor, seed, dimension, topology, planner revision,
and canonical location, semantic plan facts must be byte-identical under:

- cold and repeated rebuild;
- every tested request permutation;
- raster, spiral, random-walk, reverse, teleport, and two-front paths;
- serial, reversed, and parallel completion;
- empty, warm, partial, and repeatedly evicted caches;
- neighboring region first/last;
- overlapping query windows and alternative chunk batches; and
- equivalent canonical/lifted plane, cylinder, and torus queries.

A fixed planner revision may prescribe a canonical internal traversal,
sorting order, and tie-breaker. Runtime discovery and execution order may not
choose them.

## Containment And Ownership

- `mclone-worldgen` may own pure research kernels, topology-neutral
  descriptors, semantic receipts, and reconstruction queries only when native
  and Wasm must share them. Production generation cannot call them.
- A standalone diagnostic owns corpus execution, permutation scheduling,
  artifact writing, and benchmark orchestration.
- Browser TypeScript may transport and present Rust-owned semantic facts; it
  cannot reimplement planner rules.
- Terrain Lab integration begins only after a candidate passes exact
  invariance and boundedness gates.
- Caches in experiments are disposable and descriptor-keyed. They cannot
  become implicit persistence or discovery state.
- Raw images and receipts stay under explicit `/tmp` directories. Small
  checked-in fixtures require a focused justification and stable schema.

Every implementation commit must retain the production terrain and
surface-chunk fingerprints until a later production tactical explicitly
changes them.

## Candidate Set

The first research review compares mechanisms before code selects a winner.

### Negative control: recentered bounded solve

Run the Tactical 267-style solve through overlapping centers and quantify
semantic disagreement. This is expected to fail window independence. It
provides a measured explanation of the problem and a regression canary
against accidentally shipping it.

### Candidate A: canonical supertile and owned interior

Solve one stable construction supertile with a finite halo. Publish only a
canonical interior. Test at least two plan extents or halo ratios chosen from
the 6,144-period divisor vocabulary.

Reject if a larger halo merely moves rather than resolves semantic
disagreement, or if important drainage has no bounded boundary rule.

### Candidate B: hierarchical shared boundary facts

Generate stable coarse facts—such as major outlet/sink identity, river ports,
high/low axes, and broad levels—then condition finer regional solves on them.

Reject if the hierarchy produces visible square lattices, lacks a finite
dependency proof, duplicates identity across levels, or needs exploration-time
reconciliation.

### Candidate C: feature-owned deterministic graph

Give stable owner cells responsibility for river, basin, ridge, or corridor
starts. Target queries enumerate every possible owner in a declared finite
neighborhood and reconstruct clipped influence locally.

Reject if convincing connected structure requires an unbounded owner search,
or if independently selected features cannot form coherent basin and level
semantics.

### Candidate D: bounded hybrid atlas

Combine coordinate-pure envelopes, finite coarse contracts, canonical
regional detail, and compact feature primitives only after the individual
mechanisms state their identity and dependency rules.

Do not begin here by default. It is the most expressive and easiest candidate
in which to conceal unboundedness or unnecessary complexity.

### Fallback control

Use coordinate-pure scalar/density terrain plus target-local bounded feature
starts and deterministic owner-neighborhood queries. Give it a fair quality
pass for the same river, basin, ridge, and negative-space goals rather than
comparing new candidates only with the current warped-river field.

## Phase 0 Architecture Specification

The source review changes the experiment from “stream an exact watershed” to
“generate bounded hydrography.” Exact tiled Priority-Flood and flow
accumulation still require a global meta-graph over the complete finite DEM.
Mclone's plane and cylinder have no complete last tile. A candidate may create
river, basin, divide, sink, and level relationships by rule, but it may not
call those facts the exact drainage analysis of an unbounded provisional
heightfield.

### Shared Identity And Execution Rules

Every candidate except the deliberate negative control uses the following
conceptual keys:

```text
PlanKey {
    stored_profile_revision,
    world_seed,
    dimension_id,
    topology_descriptor_hash,
    candidate_revision,
    plan_family,
    level,
    canonical_region_coordinate,
}

FactId {
    owner: PlanKey or canonical facet key,
    fact_kind_domain,
    stable_local_index,
}
```

The viewport center, request window, requested chunk, Worker, thread,
completion order, cache state, and prior exploration are never part of either
key. Negative coordinates use Euclidean division. Periodic coordinates are
canonicalized before identity is formed.

The initial research descriptor fixes:

- 32-block semantic cells;
- 1,024-block base plan regions;
- a maximum finite hierarchy of 1,024, 3,072, and 6,144 blocks for the
  hierarchical candidate;
- a maximum individual feature reach and graph-edge length of 1,536 blocks;
- integer or fixed-point semantic facts before floating reconstruction;
- stateless typed hashes from `FactId` and choice index rather than a mutable
  random stream; and
- canonical ascending `FactId` order for any conflict resolution or accepted
  serialization.

The numeric sizes are experiment revision 0, not production constants. They
divide the 6,144-block periodic corpus, and individual feature reach is less
than half its period. Changing them changes candidate identity and requires a
new receipt.

Dependencies form a finite directed acyclic graph declared by the descriptor.
A level may read immutable coordinate fields, a fixed set of coarser provider
regions, or a fixed owner neighborhood. It may not recursively discover
same-rank plans, climb an unbounded ancestor chain, or follow a river until an
outlet is found. Cache misses may reconstruct the same DAG; they may not add
geographic facts.

### Shared Boundary And Topology Rules

Half-open base regions own cells whose canonical centers fall inside their
owned extent. Cross-region semantics use either a feature owner or a canonical
facet key:

```text
FacetKey {
    topology_descriptor_hash,
    level,
    axis,
    canonical_facet_coordinate,
}
```

Both sides read the same facet fact. On a cylinder or torus, identified sides
canonicalize to the same key even when a 6,144-block root region is adjacent
to itself. Corners use a corresponding canonical vertex key. Ownership never
depends on which side asks first.

Every bounded feature records an anchor owner, stable bounds, and the maximum
distance by which it can affect output. A consumer asks for all features
overlapping its target extent expanded by that declared reach. Each individual
segment uses one target-relative topology lift; half-period ties use the
topology contract's stable rule. Revision 0 rejects an individual primitive
whose span reaches half a periodic extent.

Semantic facts do not blend at implementation boundaries. Residual scalar
height may use compact support after all possible owners have been enumerated.
On a torus, every directed water fact must reach an explicit ocean, lake,
wetland, or closed-basin sink inside its declared bounded graph; an identified
edge is not an outlet.

### Candidate Contract Matrix

| Candidate | Plan identity and ownership | Finite dependency claim | Boundary/topology construction | Expected failure or demotion |
|---|---|---|---|---|
| negative: recentered solve | window center and extent deliberately enter identity; publishes the current window interior | one bounded solve, but no world-indexed identity | overlap the same locations through shifted windows and compare full semantics | must fail window and partition independence; retained only to prove the harness can detect the original problem |
| A: canonical supertile | 1,024-block `PlanKey`; half-open core owns accepted cells; halo owns nothing | solve core plus declared 256- and 512-block halo controls; correctness is claimed only for operations whose proven effect distance fits the halo | coordinate fields wrap correctly and cores are canonical, but independently derived receivers have no shared semantic authority | expected to move rather than solve drainage disagreement; may survive only as a local realization mechanism |
| B: hierarchical boundary facts | `PlanKey` at 6,144-, 3,072-, and 1,024-block levels; canonical facets own ports; finer plans own interior elaboration | exactly three levels; root cell/facet facts are coordinate-pure and never request a parent; each child reads a fixed parent/facet neighborhood | root facets supply stable high/low tendency, sink/outlet permission, ports, and level intervals; children elaborate between the same shared facts; levels and facets wrap/deduplicate | may expose a square lattice, force implausible ports, or smuggle global drainage into root selection; it creates bounded hierarchy, not exact contributing area |
| C: feature-owned graph | 1,024-block anchor cells own starts; each start owns one complete bounded graph; an unordered endpoint pair owns an edge; reaches and basin primitives have stable IDs and bounds | every graph lies within 1,536 blocks of its anchor; consumers enumerate owners in target bounds expanded by that reach; construction examines one fixed candidate stencil and never traverses another graph | all targets reconstruct the same overlapping edge or primitive; standalone graphs end at an explicit sink/outlet; direction derives from a coordinate-pure potential plus stable ties; topology chooses canonical endpoints and work lifts | networks may look fragmented or noise-directed; exact whole-river component IDs, global basin IDs, and exact stream order are disallowed because they require cross-graph traversal |
| D: bounded hybrid atlas | B owns finite-scale ports/level permissions; C owns bounded features; A owns local raster realization | union of already-proved B and C DAGs plus compact-support reconstruction; no additional discovery step | shared hierarchy conditions feature graphs, and every target enumerates all overlapping owned primitives before realization | easiest place to hide complexity or an undeclared dependency; cannot be implemented until B and C pass separately |
| fallback | coordinate-pure field revision plus canonical bounded starts with Minecraft-style possible-owner enumeration | one fixed owner stencil per feature family; no cross-feature graph or derived watershed | topology-aware starts cross seams through one owner and target-relative lift; effects clip/reconstruct per target | may retain an obvious field character and weaker river/basin composition, but is the determinism, speed, and maintenance control |

### Candidate Audit At R0

- Candidate A is not a plausible standalone hydrology solution. It remains a
  measured control and possible fine realization tile.
- Candidate B has a dependency proof only because its hierarchy stops at a
  fixed 6,144-block level whose root facts are coordinate-pure. Adding a
  generated parent on demand would reopen an infinite-ancestor problem.
- Candidate C owns a complete graph inside one declared feature bound. A
  standalone graph ends at its own typed sink/outlet; only Candidate D may
  connect it to a B-owned port. It may not derive a globally stable
  connected-component name or exact upstream area across feature owners.
- Candidate D is a composition hypothesis, not the first implementation.
- The fallback is credible because its owner and possible-influence search
  match already inspected Minecraft structure/carver mechanisms and Mclone's
  existing periodic bounded-feature contract.

No candidate uses global mutable state, discovery-time reconciliation, or
exploration history. B and C achieve that by intentionally limiting the
semantic claim. Whether that limited claim still creates convincing geography
is the central research question.

## Fixed Research Corpus

The corpus begins with the Tactical 267 seeds:

- `12345`;
- `8675309`; and
- `-98765`.

Add adversarial seeds only through a recorded reason, such as no ocean,
boundary-aligned sinks, unusually flat provisional terrain, dense crossings,
or half-period ties. Do not replace difficult fixed seeds after seeing a
candidate.

Topologies:

- unbounded Euclidean plane;
- 6,144-block X-periodic cylinder with unbounded Z; and
- a pure 6,144-by-6,144 flat-torus planning kernel, even though no production
  Mclone torus profile exists yet.

Retain the Tactical 267 fixed 6,144-by-6,144, 32-block-cell result as a bounded
quality/performance control. Streamed candidates must additionally cover:

- an owned-region interior;
- every edge orientation;
- four-region corners;
- a cylinder seam;
- all torus edge/corner identifications;
- representative ocean, coast, highland, broad-low, quiet, and protected-sink
  locations; and
- journeys crossing several plan identities.

Revision 0 uses the 32-block cells and plan sizes specified above. A second
resolution or extent is a separately identified experiment when a dependency
or boundary hypothesis requires it. None of these research parameters is a
production generator constant.

### Revision 0 Canonical Target Set

Base plan coordinates below index the fixed 1,024-block regions. The set is
small enough for permutation tests and large enough to include ordinary
interiors, every boundary orientation, corners, negative division, periodic
identification, and multiple hierarchy parents.

- Plane: every region in `[-2, 2] x [-2, 2]`, plus teleport targets
  `(-23, 17)`, `(41, -29)`, `(-64, -64)`, and `(63, 63)`.
- X-cylinder: canonical X regions `[0, 5]` at every Z in `[-2, 2]`, plus
  equivalent X lifts `-6`, `6`, and `12` for the same canonical targets.
- Flat torus: all 36 canonical regions in `[0, 5] x [0, 5]`, plus negative
  and positive lifts of every edge and four corners.

The canonical boundary cases are:

- horizontal pair `(0, 0)` / `(1, 0)`;
- vertical pair `(0, 0)` / `(0, 1)`;
- four-region corner `(0, 0)`, `(1, 0)`, `(0, 1)`, `(1, 1)`;
- negative-division corner `(-1, -1)`, `(0, -1)`, `(-1, 0)`, `(0, 0)`;
- cylinder seam pair `(5, 0)` / `(0, 0)`;
- torus X seam, Z seam, and the four lifted representations of canonical
  corner `(0, 0)`; and
- 3,072- and 6,144-block hierarchy boundaries containing each base case.

Fixed journey request sequences are:

1. row-major and reverse over the plane 5-by-5 core;
2. center-out spiral and outside-in rings over the same core;
3. one fixed-seed permutation of every topology's canonical set;
4. alternating plane teleport targets and their adjacent regions;
5. two fronts approaching the `(0, 0)` / `(1, 0)` boundary;
6. cylinder regions `(-2, 0)` through `(8, 0)`, preserving lifted requests;
   and
7. torus diagonal `(0, 0)` through `(5, 5)`, then the same journey through
   one X and one Z lift.

Seed-dependent ocean, coast, highland, broad-low, quiet, sink, confluence, and
crossing examples are selected only after the complete fixed target set is
generated. Selection is deterministic: sort canonical facts by `(kind,
FactId)` and retain the first present example of each requested role. Missing
roles remain recorded as missing; a difficult seed is not replaced. These
selectors choose review artifacts, not pass/fail targets.

## Evidence Schema

Every candidate run writes a schema-versioned receipt containing:

- commit, schema, candidate, and planner revisions;
- stored seed/profile/topology facts;
- canonical plan identities and requested construction order;
- cell, owned-region, construction, halo, and hierarchy extents;
- deterministic sort/tie/random domains;
- maximum dependency fanout, traversal depth, and owner-neighborhood radius;
- complete semantic checksums per plan identity;
- shared-boundary crossing and ownership records;
- mismatch counts for every order/path/cache/window/lift comparison;
- graph completion, cycle, sink, spill, basin, divide, and channel facts;
- reconstruction height/water checksums where present;
- cold/warm construction, lookup, batch, summary, and cache timings;
- resident, transient, transferred, and optional persisted bytes;
- host, toolchain, iteration, warmup, and timing-boundary facts; and
- artifact paths and inspected/not-inspected state.

The schema must distinguish exact comparison from toleranced geometric
metrics. It must never summarize a nonzero semantic mismatch as “close.”

### Revision 0 Receipt Contract

The machine-readable receipt is small JSON. Large semantic arrays, timing
samples, and images remain separate files under the run's `/tmp` directory.
Paths may appear in the receipt but do not make those artifacts durable.

Required top-level records are:

```text
receipt_schema
run {
    experiment_id, source_commit, command, started_utc
    host, target, rustc, optimization, thread_count, worker_count
}
descriptor {
    candidate_revision, stored_profile_revision, seed, dimension_id
    topology, topology_descriptor_sha256
    semantic_cell_blocks, base_region_blocks
    hierarchy_blocks[], maximum_feature_reach_blocks
    numeric_policy, tie_domains[], random_domains[]
}
dependency_claim {
    layer_dag[], maximum_depth, maximum_provider_fanout
    maximum_owner_radius, maximum_traversal_steps
    undeclared_fetch_count
}
request_case {
    case_id, canonical_targets[], lifted_requests[]
    request_order[], completion_order[], cache_policy
}
plans[] {
    PlanKey, owned_extent, construction_extent, provider_keys[]
    semantic_record_count, semantic_sha256
}
boundaries[] {
    facet_or_vertex_key, participating_plans[]
    crossing_records_sha256, duplicate_count, omission_count
}
comparisons[] {
    property, left_case, right_case, record_count
    exact_mismatch_count, first_mismatch
}
geography {
    reach_count, crossing_count, confluence_count
    sink_count, cycle_count, basin_count, spill_count
    missing_review_roles[]
}
reconstruction {
    sample_descriptor, height_sha256, water_sha256
    exact_boundary_mismatches, geometric_metrics
}
cost {
    warmup_count, measured_iterations, timing_boundaries
    cold_plan_ns[], warm_point_ns[], warm_batch_ns[]
    summary_ns[], cache_rebuild_ns[]
    retained_bytes, peak_transient_bytes, transferred_bytes
}
artifacts[] {
    kind, path, byte_count, sha256, inspected
}
```

Semantic checksums use SHA-256 over a canonical little-endian byte stream:
records sorted by `FactId`; explicit type tags and lengths; fixed-width signed
integers; UTF-8 strings with byte lengths; no hash-map iteration order; and no
native padding. Exact semantic records do not contain unconstrained floating
point. A numeric policy may use fixed-point integers or explicitly normalized
IEEE bit patterns. Changing serialization changes `receipt_schema`.

`maximum_traversal_steps` is zero for ordinary point lookup unless a candidate
declares a small fixed loop over its own bounded primitive. A sentinel such as
“until outlet,” “until convergence,” or “all upstream cells” is invalid.
`undeclared_fetch_count` must be zero.

Each exact comparison records the complete canonical record count and mismatch
count. A checksum match is a fast equality witness, not a substitute for
retaining the first structured mismatch when a comparison fails. Geometric
metrics are labeled separately and cannot turn a semantic mismatch into a
pass.

## Boundary Experiments

### Semantic agreement

For each shared edge and corner:

- compare canonical ownership with no omissions or duplicate publication;
- match crossing feature identity, kind, direction, order, and level;
- match basin identity, sink/outlet, and spill semantics;
- match divide/ridge continuation and neighboring basin pair;
- conserve declared coarse-to-fine port relationships; and
- rebuild both sides in every order and cache state.

Semantic mismatch tolerance is zero.

### Geometric continuity

Reconstruct the same boundary strips through each relevant plan query and
record:

- exact same-coordinate height and water equality;
- cross-boundary first-difference/slope impulses;
- normal-angle discontinuity;
- curvature and feature-density distributions;
- boundary-versus-interior matched transects; and
- the maximum distance over which a boundary implementation measurably
  changes geometry.

Do not pick an arbitrary smoothness threshold before measuring the fallback,
fixed-domain reconstruction, and ordinary interior controls. Record the
distributions, then propose a gate before candidate selection.

### Visual boundary atlas

For candidates that pass exact semantic tests, create:

- plan maps with optional region, halo, owner, port, and hierarchy overlays;
- mismatch maps that remain empty for an accepted run;
- boundary and four-region-corner magnifiers;
- cylinder and torus seam-centered maps;
- reconstruction relief and oblique views with boundary guides on and off;
  and
- matched interior controls so ordinary plan texture is not mistaken for a
  seam.

## Path And Schedule Experiments

Generate the same canonical target set through:

1. row-major raster;
2. reverse raster;
3. center-out spiral;
4. outside-in rings;
5. fixed-seed random permutation;
6. teleport among disconnected targets;
7. two fronts meeting at a boundary;
8. target first, neighbors later;
9. neighbors first, target later;
10. bounded parallel completion permutations; and
11. repeated cache eviction between requests.

Compare final semantic receipts, per-target lookup facts, and reconstructed
terrain fingerprints. Also compare construction counts and peak residency so
an order-independent result cannot hide unbounded duplicate work.

## Performance Experiments

Measure candidates against Tactical 258 production sampling, Tactical 267's
bounded planner, and the fallback. Keep these clocks separate:

- cold canonical-plan construction;
- warm plan-cache hit;
- one point lookup;
- one chunk/region batch;
- exact terrain realization consuming plan facts;
- far-summary construction and lookup;
- serialization/transfer;
- cache insertion, eviction, and reconstruction;
- fixed-budget movement/streaming; and
- end-to-end diagnostic readiness.

Report medians and tail behavior after explicit warmup. Include worst-case
boundary/corner and dense-feature regions rather than averaging them into
quiet country.

Hard performance gates at this research stage are:

- no undeclared or unbounded query/construction traversal;
- no cost or memory growth with exploration history after bounded eviction;
- no Worker or platform dependency in semantic results; and
- a usable far summary that does not reconstruct exact regional terrain.

After measurements, propose explicit production budgets for a later decision
review. Do not retroactively redefine timing boundaries to make a candidate
pass.

## Phases And Human Review

### Phase 0: source and architecture research

- [x] Expand the durable source ledger with primary work on tiled/parallel
  watershed processing, deterministic hierarchical procedural networks,
  boundary conditions, and maintained or shipped generator precedents.
- [x] Record exact contributions, limitations, implementation inspection depth,
  and licensing/clean-room constraints.
- [x] Specify each candidate's plan identity, ownership, dependency proof,
  boundary contract, topology behavior, and expected failure mode.
- [x] Define the initial corpus and receipt schema before implementation.

**Human Review R0 — research brief**

Review the source ledger and candidate specifications. Confirm that the
research question is fair, the fallback is credible, important precedents
are not missing, and no candidate hides global mutable state or an undeclared
infinite dependency. Stop or revise before implementation if the brief is
weak.

**Decision 2026-07-27: accepted.** Proceed with Phase 1's neutral
falsification harness and controls. Stop before implementing Candidates B, C,
or D.

The Phase 0 proposal is:

1. Accept that exact analytical drainage over an unbounded provisional
   surface is outside this streamed experiment. Finite whole-domain profiles
   may still use it.
2. Test bounded **generative hydrography**: stable reaches, bounded basin and
   lake primitives, shared ports and levels, explicit sinks, and terrain
   realization informed by those facts.
3. Keep the recentered solve as the required failing canary and Candidate A
   as a halo/control mechanism rather than a likely winner.
4. Give minimal independent trials to Candidate B and Candidate C only after
   the neutral Phase 1 harness catches the known failures.
5. Compose Candidate D only if both mechanisms first prove exact invariance,
   finite dependencies, and enough distinct structural value.
6. Compare every survivor with the coordinate-pure plus bounded-start
   fallback.

R0 should explicitly answer:

- Is “bounded generative hydrography” still the intended novel question, or
  is exact derived watershed a requirement that should instead select a
  finite/offline world architecture?
- Are the three strongest precedent classes sufficient: exact finite tiled
  hydrology, deterministic contextual dependency frameworks, and real game
  generators at the finite-global and bounded-local extremes?
- Is a fixed maximum planning scale of 6,144 blocks a fair first experiment,
  with the understanding that it can fail quality review by exposing that
  scale?
- Is it acceptable that Candidate C promises stable reaches and bounded basin
  primitives, not global whole-river IDs or exact contributing area?
- Does the fallback receive a fair enough feature-quality pass to make
  rejection of the relational candidates meaningful?

An R0 acceptance authorizes only Phase 1's neutral falsification harness and
controls. It does not authorize a candidate implementation, Terrain Lab
integration, or production terrain change.

### Phase 1: neutral invariance harness and controls

- [x] Build candidate-neutral descriptor, semantic receipt, checksum, request
  permutation, cache, and topology test machinery.
- [x] Run the fixed bounded planner, recentered negative control, and
  coordinate-pure fallback.
- [x] Prove that the harness detects recenter disagreement and historical-style
  last-writer/discovery-state canaries.
- [x] Record current native/Wasm agreement or mismatch before candidate
  tuning.

No subjective review is required. The outcome is a trustworthy falsification
instrument and quantified controls.

**Outcome 2026-07-27:** accepted as an instrument. Commit `14d832b8`
contains the research-only shared kernel, standalone receipt runner, and Wasm
tests. The inspected native receipt is
`/tmp/mclone-streamed-plan-phase1/native-receipt.json` with file SHA-256
`e3d43b1deac7fb02dda65cdd2910c56eae667b8293b690fdd46cb88f441719a0`.
It records source commit `14d832b8a7785e2e10e14d1408ae2abc02e7bf49`,
schema `mclone-streamed-plan-phase1-receipt-v1`, three fixed seeds, and all
three topologies. The working tree was truthfully marked dirty because
unrelated vegetation/render work was present; only the named source commit
owns this harness.

Results:

- 99 expected-equality comparisons passed with zero exact mismatches and zero
  conflicting same-key publications;
- 15 expected-failure comparisons caught every recentered-window,
  recentered-last-writer, request-order discovery, schedule discovery, and
  cache discovery canary;
- recentering changed 965/1,024, 1,024/1,024, and 1,024/1,024 target-cell
  records for seeds `12345`, `8675309`, and `-98765`;
- each reversed recentered publication recorded one internal same-key
  conflict per direction, preventing silent last-writer acceptance;
- native and `wasm32-unknown-unknown` agree on coordinate-pure witness
  `e5e329515e9638044db802f67433ba40b8b1f4aaf179e11e45c04fc2e03a1fde`;
  and
- native and Wasm agree on complete 114-comparison witness
  `2f7bd2f9fb3e54fe9cf66900e32ca985a5de7fbba1a6d0ca379678b9c5eafc12`.

Validation commands:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-worldgen \
  --bin mclone_streamed_plan_harness -- \
  --output /tmp/mclone-streamed-plan-phase1/native-receipt.json
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen
STREAMED_PLAN_WASM_DIR="$PWD/native/target/wasm-bindgen-cli-0.2.125"
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="$STREAMED_PLAN_WASM_DIR/bin/wasm-bindgen-test-runner" \
  cargo test --manifest-path native/Cargo.toml -p mclone-worldgen \
  --target wasm32-unknown-unknown --test streamed_plan_wasm
```

The receipt explicitly marks geography, reconstruction, and cost as
unmeasured. The coordinate-pure control is not yet the quality-comparable
fallback promised for later phases. No candidate or production terrain
consumer was implemented.

### Phase 2: minimal candidate trials

- Implement the smallest semantic-only spike for every candidate that can
  state a finite dependency rule.
- Run rebuild, order, path, cache, window, boundary, cylinder, and torus
  suites before terrain reconstruction.
- Reject failures rather than smoothing their outputs.
- Compare code/identity/cache complexity and bounded cost.

Candidates with nonzero semantic mismatches, undeclared traversal, or
exploration-history growth do not advance.

### Phase 3: streamed structural atlas

- Select at most two exact-invariant candidates plus the fallback.
- Add streamed pan over canonical plan identities in Terrain Lab.
- Expose region, owner, halo, hierarchy, boundary-port, and cache diagnostics
  as independent overlays.
- Preserve checksums while panning, evicting, rebuilding, and approaching the
  same area by different paths.
- Generate the comparable structural and boundary corpus.

**Human Review R1 — structural planner**

Freely pan across ordinary boundaries, corners, cylinder seams, and torus
identifications. Judge hierarchy, long axes, basin/drainage relation, quiet
space, repetition, planning-grid artifacts, and whether implementation
boundaries become visible. Compare directly with the fallback. A candidate
may be objectively exact and still fail this review.

### Phase 4: reconstruction, summaries, and speed

- Reconstruct continuous terrain and water influence from the finalist and
  fallback without changing production generation.
- Add same-seed macro, oblique, journey, and walking-scale evidence.
- Add scale-aware far summaries and broad-preview comparisons.
- Measure construction, lookup, batch, cache, memory, transfer, and streaming
  costs on native and Wasm paths.
- Propose explicit production budgets and complexity tradeoffs.

**Human Review R2 — terrain and performance tradeoff**

Review ordinary and showcase terrain at regional and walking scales,
including boundaries, corners, coast arrivals, and periodic seams. Decide
whether the relational gain remains compelling after exact determinism,
bounded execution, local detail, and broad-preview constraints are enforced.

### Phase 5: decision record

- Summarize every accepted and rejected candidate with exact evidence.
- Record remaining risks, maintenance shape, selected budgets, and
  production-integration prerequisites.
- Update this tactical and the living topics with one outcome.
- Delete or clearly isolate experiments that could be mistaken for production.

**Human Review R3 — promote, narrow, continue, or reject**

Choose the terminal research outcome. Promotion creates a new production
tactical with new compatibility, persistence, exact-chunk, preview, and
platform gates. This tactical never performs that integration itself.

## Phase Outcomes

| Phase | Required durable outcome | Stop condition |
|---|---|---|
| 0 | primary-source ledger, candidate specs, dependency claims, corpus, receipt schema | R0 accepts the research brief |
| 1 | trusted permutation/cache/topology harness plus negative and fallback controls | harness catches known order/window failures |
| 2 | exact invariant and bounded-cost comparison for minimal candidates | zero-mismatch survivors identified, or all rejected |
| 3 | freely pannable streamed plan atlas for at most two survivors and fallback | R1 selects a finalist or fallback |
| 4 | reconstructed terrain, far summaries, native/Wasm costs, proposed budgets | R2 evaluates quality versus speed/complexity |
| 5 | complete decision and cleanup record | R3 selects promote, narrow, continue, or reject |

## Documentation And Commit Trail

- Commit this tactical before experimental code.
- Update the living topic's reference and experiment ledgers as evidence
  lands; do not leave source conclusions only in chat transcripts.
- Commit the neutral harness separately from candidate implementations.
- Give each material candidate trial its own reviewable commit or short
  threaded series using the exact topic trailer
  `deterministic-streamed-landscape-planning`.
- Record failed experiments with their commands, receipts, and reason for
  rejection before deleting or superseding code.
- At each human gate, commit the handoff state before pausing.
- Keep generated corpora under `/tmp`; commit schemas, small deterministic
  fixtures when justified, summary tables, and decision records.
- Production fingerprints remain a validation control throughout.

## Validation Ledger

This ledger begins empty. Fill it with exact commands and results during
execution.

| Experiment | Commit | Corpus/command | Result | Decision |
|---|---|---|---|---|
| source review | `0fa857db` | primary papers, public framework/source, local Minecraft source; docs only | exact tiled hydrology retains a finite global meta-problem; contextual streaming requires finite effect distance | reframe as bounded generative hydrography |
| candidate and corpus review | `d91ed5e4` | architecture contract and receipt schema; docs only | B and C state finite claims; A is a control; D is deferred; fallback remains credible | R0 accepted 2026-07-27; Phase 1 authorized |
| neutral Phase 1 harness | `14d832b8` | `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen`; 379 passed, 1 ignored, plus binary tests | canonical keys, exact integer facts, permutations, cache modes, topology lifts, and internal-publication conflict detection pass | accept the harness as the candidate-neutral instrument |
| recentered-window negative control | `14d832b8` | `cargo run --manifest-path native/Cargo.toml -p mclone-worldgen --bin mclone_streamed_plan_harness -- --output /tmp/mclone-streamed-plan-phase1/native-receipt.json` | shifted real solves mismatch 965, 1,024, and 1,024 of 1,024 cells; reversed publication also conflicts | retain as permanent failing window/last-writer canary |
| discovery-state negative control | `14d832b8` | same 114-comparison receipt | all request, schedule, and cache canaries fail as intended for all three seeds | reject discovery-time mutable facts |
| fallback invariant control | `14d832b8` | native runner plus `wasm-bindgen-test-runner` on `streamed_plan_wasm`; pinned comparison witness `2f7bd2...fc12` | 99 equality comparisons have zero mismatch/conflict; full native/Wasm witness agrees | accept as deterministic control, not yet a quality finalist |
| canonical supertile trial | pending | pending | pending | pending |
| hierarchical boundary trial | pending | pending | pending | pending |
| feature-owned graph trial | pending | pending | pending | pending |
| streamed structural atlas | pending | pending | pending | pending |
| reconstruction/performance finalist | pending | pending | pending | pending |

## Exit Condition

Tactical 270 is complete only when:

- the reference and experiment trail is durable and reproducible;
- the tested invariants have exact receipts rather than prose assurances;
- semantic boundary agreement and geometric boundary quality are reported
  separately;
- topology and performance results include the declared fixed corpus;
- human reviews R0-R3 have explicit outcomes or an earlier hard rejection
  makes later reviews unnecessary;
- production terrain remains unchanged; and
- the final decision is promote, narrow, continue with one bounded question,
  or reject in favor of the fallback.
