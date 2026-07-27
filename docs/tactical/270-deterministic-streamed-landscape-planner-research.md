# Tactical 270: Deterministic Streamed Landscape Planner Research

Status: proposed and ready to begin 2026-07-27. Research only; no production
terrain or planner integration is authorized.

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

The first candidate specification should use 32-block semantic cells for
comparison. It may also test a second resolution when the dependency or
boundary hypothesis requires it. Region and halo sizes are experiment
parameters, not generator constants until selected.

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

- Expand the durable source ledger with primary work on tiled/parallel
  watershed processing, deterministic hierarchical procedural networks,
  boundary conditions, and maintained or shipped generator precedents.
- Record exact contributions, limitations, implementation inspection depth,
  and licensing/clean-room constraints.
- Specify each candidate's plan identity, ownership, dependency proof,
  boundary contract, topology behavior, and expected failure mode.
- Define the initial corpus and receipt schema before implementation.

**Human Review R0 — research brief**

Review the source ledger and candidate specifications. Confirm that the
research question is fair, the fallback is credible, important precedents
are not missing, and no candidate hides global mutable state or an undeclared
infinite dependency. Stop or revise before implementation if the brief is
weak.

### Phase 1: neutral invariance harness and controls

- Build candidate-neutral descriptor, semantic receipt, checksum, request
  permutation, cache, and topology test machinery.
- Run the fixed bounded planner, recentered negative control, and
  coordinate-pure fallback.
- Prove that the harness detects recenter disagreement and historical-style
  last-writer/discovery-state canaries.
- Record current native/Wasm agreement or mismatch before candidate tuning.

No subjective review is required. The outcome is a trustworthy falsification
instrument and quantified controls.

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
| source and candidate review | pending | pending | pending | pending |
| recentered-window negative control | pending | pending | pending | pending |
| fallback invariant control | pending | pending | pending | pending |
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
