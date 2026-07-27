# Tactical 272: Multiscale Semantic Refinement Witness

Status: **Human Review R1 ready as of 2026-07-27. The shared witness landed
at `fc844460`; the fourth Terrain Lab atlas view and performance receipt
landed at `2fc719d9`. Exact native/Wasm, topology, traversal, projection,
parent, containment, continuity, and terminal checks pass. Production terrain
integration, terrain reconstruction, and selective 3D realization remain
unauthorized.**

Topics:

- `multiscale-terrain-representation`
- `deterministic-streamed-landscape-planning`

## Motivation

Tactical 270 proves two useful but incomplete mechanisms:

- Candidate B reconstructs exact shared facts through a fixed
  6,144/3,072/1,024-block dependency DAG; and
- Candidate C reconstructs complete bounded features by enumerating canonical
  owners around a target.

Neither mechanism yet proves semantic refinement. Candidate B's root, middle,
and local values are independently hashed tendencies. Candidate C's short
graphs are independently owned complete objects. There is no feature whose
coarse identity and summary remain authoritative while regional and local
facts elaborate it.

The tailored precedent survey in
[`multiscale-terrain-representation.md`](../topics/multiscale-terrain-representation.md)
shows that semantic procedural refinement and sparse implicit 3D terrain are
established ideas. The useful Mclone question is narrower: can one canonical
feature hierarchy support direct coarse queries and exact local queries under
bounded random access, exact traversal independence, and plane/cylinder/torus
topology?

## Objective

Implement the smallest research-only witness that can answer that question:

```text
continental parent feature and conservative summary
    -> regional child segments tied to that parent
    -> local child segments tied to regional parents
```

The witness must prove:

1. the parent fact returned by a direct coarse query is byte-identical to the
   parent fact included in regional and local queries;
2. every child refers to one stable parent and stays inside the influence
   authorized by that parent;
3. exact local facts do not depend on request, path, batch, cache, completion
   order, thread, viewport, partition, or equivalent topology lift;
4. plane and cylinder queries construct only a bounded owner neighborhood,
   while the finite torus uses the same feature language and canonical
   identities;
5. coarse queries do not generate their regional or local children; and
6. Terrain Lab can show the parent, regional, and local representations
   independently without making presentation scale canonical state.

Pause for human review after the invariance evidence and interactive
diagnostic are complete.

## Non-Goals

This tactical does not:

- change Mclone Overworld field revision 21 or exact chunks;
- reconstruct height, water, materials, ecology, or structures;
- compose Tactical 270 Candidate D;
- claim that the witness is convincing geography;
- implement exact hydrology, erosion, or contributing area;
- add universal or selective 3D density;
- select production planning scales or feature frequencies;
- make camera LOD part of generation identity; or
- claim novelty.

The witness is deliberately allowed to look schematic. Its purpose is to make
semantic consistency, topology, boundedness, and cost inspectable before
terrain quality can obscure them.

## Shared Ownership

- `mclone-worldgen` owns canonical identities, topology-aware owner
  enumeration, feature construction, direct summary queries, invariance
  receipts, and typed atlas facts.
- A standalone `mclone-worldgen` research binary owns corpus execution,
  timing, and `/tmp` receipt output.
- `mclone-terrain-lab` owns only the existing Wasm serialization adapter.
- Terrain Lab TypeScript owns Worker transport, Canvas presentation, overlay
  toggles, and explanatory labels. It may not reconstruct features or infer
  parent relationships.
- Production worldgen must not import or call the witness.

## Witness Revision 1

### Fixed scales

Revision 1 uses the existing Tactical 270 scale vocabulary:

| Level | Extent | Role |
|---|---:|---|
| parent | 6,144 blocks | stable feature identity and conservative summary |
| regional | 3,072 blocks | bounded child elaboration |
| local | 1,024 blocks | final semantic detail for this witness |

The hierarchy has exactly three levels. A point, region, or window query may
stop at any level. No exact local query may climb an infinite ancestor chain.

Changing these sizes, subdivision counts, displacement limits, feature
families, or identity rules changes the witness revision and its pinned
checksums.

### Feature vocabulary

Revision 1 owns two intentionally small families:

- **range axis** — an undirected large-scale terrain corridor; and
- **basin route** — a directed source-to-explicit-sink corridor.

Each canonical parent owner has one feature of each family. A parent fact
contains:

- a stable feature ID;
- family and scale;
- canonical owner;
- coarse endpoints;
- conservative bounds containing every permitted descendant;
- maximum influence width;
- height or level interval; and
- an explicit terminal kind for the basin route.

Regional refinement splits the parent course into a fixed number of child
segments and adds bounded lateral displacement. Local refinement splits each
regional segment again. Children retain exact endpoint continuity, record
their parent feature ID, and cannot exceed the parent's conservative bounds.

These are generative semantic facts, not samples from a hidden fine
heightfield.

### Identity

Every feature identity derives from:

```text
stored profile revision
world seed
dimension ID
topology descriptor
witness revision
feature family
level
canonical owner coordinate
stable child slot
```

The camera, requested detail, viewport center, request window, Worker,
thread, batch, completion order, cache state, and prior exploration are not
identity inputs.

A detailed query includes the unchanged parent facts needed to interpret its
children. It does not replace a parent with a geometrically unrelated
fine-scale feature.

### Direct summaries

A parent query constructs parent facts and their conservative summaries only.
It must report zero regional and local constructions.

A regional query constructs parents and regional children only. A local query
constructs all three levels. Filtering a detailed result to parent level must
produce the exact direct-parent result. Filtering local output to parent plus
regional levels must produce the exact direct-regional result.

The receipt separately records requested plans, canonical owners examined,
parent facts, regional facts, local facts, canonical bytes, and elapsed time.
Timing is descriptive evidence, not a production budget in this tactical.

### Bounded owner enumeration

A target enumerates parent owners from its extent expanded by the declared
maximum parent influence. It never discovers owners by following another
feature.

The owner neighborhood, hierarchy depth, maximum children per parent, facts
per target, and maximum point-lookup traversal are fixed in the descriptor
and reported. A query that exceeds those declared bounds fails rather than
silently broadening its search.

### Topology

All canonicalization and target-relative lifts use the Tactical 270 topology
descriptor:

- plane: unbounded X and Z with lazy parent owners;
- cylinder: 6,144-block periodic X and unbounded Z; and
- torus: 6,144-block periodic X and Z.

The finite torus may deduplicate to one canonical parent owner. The plane and
cylinder may not require a complete domain. Both use the same feature
families, child rules, and identity structure.

Every primitive has reach strictly below half a periodic extent. Basin routes
terminate at an explicit sink and cannot treat a periodic seam as an outlet.

## Exact Validation

### Candidate-neutral harness

Run the detailed witness through Tactical 270's existing comparison corpus:

- cold rebuild;
- reverse and fixed-random request order;
- center-out, outside-in, teleport, and two-front paths;
- reversed completion;
- single and alternate batch partitions;
- warm and repeatedly evicted caches;
- viewport-center changes; and
- equivalent cylinder and torus lifts.

All equality comparisons must have zero semantic mismatches and zero
same-key publication conflicts.

### Cross-level consistency

For every seed and topology in the fixed corpus:

- direct parent facts equal the parent projection of regional and local
  queries;
- direct regional facts equal the parent-plus-regional projection of local
  queries;
- every child parent ID resolves exactly once;
- every child stays within its parent's conservative bounds;
- sibling endpoints meet where the parent subdivision requires continuity;
- every basin route has one explicit sink;
- no query constructs facts below its requested detail;
- independently partitioned target sets publish the same canonical records;
  and
- periodic lifts preserve identities, facts, and projections.

### Parallel and cross-host evidence

On native, construct the same corpus serially and on independent threads,
canonicalize the results, and require one exact checksum.

On `wasm32-unknown-unknown`, run the full deterministic corpus through
`wasm-bindgen-test-runner` and require the pinned native witness. Browser
Worker scheduling remains presentation transport, not a different semantic
implementation.

Retain the production terrain and surface-chunk fingerprint tests.

## Interactive Diagnostic

Extend the existing freely pannable **Planner atlas** rather than adding
another fixed-domain pane. Add a fourth synchronized panel:

```text
D · Multiscale semantic refinement
```

Its independent URL-addressed overlays are:

- parent features;
- regional children;
- local children; and
- conservative parent bounds.

Existing canonical-region identities and wrap-seam overlays remain shared.
The panel and evidence receipt show:

- canonical parent owners;
- parent, regional, and local fact counts;
- parent-projection checksum;
- full-detail checksum;
- unresolved-parent count;
- containment-failure count; and
- cache hits, misses, evictions, and query time.

Changing overlays must not rebuild or alter semantic checksums. Panning,
eviction, cold rebuild, and full-period cylinder/torus movement must preserve
the relevant canonical facts.

## Performance Evidence

Measure, without setting a production budget:

1. one direct parent query;
2. one direct regional query;
3. one direct local query;
4. a 6,144-block atlas window;
5. a cost-capped 65,536-block atlas window;
6. cold and warm cache behavior; and
7. native and Wasm/browser query time.

Record fact counts and owner counts beside time. A coarse result is not
accepted as cheap if it secretly constructs and discards its children.

No 500 km performance claim is required until this tiny witness earns the
semantic and interactive review.

## Human Review R1

Pause with plane, cylinder, and torus review links. The reviewer should be
able to pan and toggle scales while answering:

1. Does one coarse feature visibly remain the same feature as detail appears?
2. Are the parent bounds and child containment understandable?
3. Does coarse-only display tell a useful broad story rather than look like
   sparse exact sampling?
4. Do refinement and owner boundaries create unacceptable square structure?
5. Does the finite torus feel like the same language as the streamed plane
   and cylinder?
6. Is this mechanism clear and promising enough to justify reconstructing one
   terrain influence family?

The review may accept the mechanism while rejecting the particular witness
geometry. It may also stop the work and retain the coordinate-pure fallback.

## Outcome 2026-07-27

Revision 1 implements two deliberately schematic feature families in shared
Rust:

- one range axis per canonical 6,144-block parent owner; and
- one source-to-explicit-sink basin route per parent owner.

Each parent splits into two regional segments and four local segments.
Children retain stable parent slots, share exact subdivision endpoints, and
stay within conservative parent influence. Target queries enumerate parent
owners from a fixed 1,792-block influence expansion. The declared bound is
four owners and 57 facts; the fixed corpus observed at most four owners and 22
facts.

The pinned native/Wasm witness is:

```text
4ac9b52c6e04697378c6ff0dfc1b9bb25a3903f9138cf4f19e633bd54db6a78d
```

It covers 105 existing Tactical 270 equality comparisons plus nine
cross-level corpora: three seeds by plane, cylinder X, and torus. Every
corpus has zero:

- parent-projection mismatches;
- regional-projection mismatches;
- unresolved parent references;
- containment failures;
- sibling-continuity failures;
- explicit-terminal failures; and
- unexpected lower-detail construction.

The standalone release receipt at
`/tmp/mclone-multiscale-terrain-witness/receipt.json` recorded this run:

| Query | Snapshot facts including query receipt | Average |
|---|---:|---:|
| direct parent | 3 | 61.999 µs |
| direct regional | 7 | 62.494 µs |
| direct local | 15 | 63.164 µs |

These timings are descriptive single-host evidence. Their close values do not
mean the parent path constructs discarded children: exact construction
counters prove zero regional/local construction for the parent query and zero
local construction for the regional query. At this tiny hierarchy, fixed
owner enumeration and hashing dominate.

The same binary measures the complete four-view atlas compiler, not just the
new witness:

| Plane atlas | Canonical plans | Witness drawing facts | Cold | Warm |
|---|---:|---:|---:|---:|
| 6,144 blocks, 1.5 aspect | 24 | 21 | 3.841 ms | 0.824 ms |
| 65,536 blocks, 1.5 aspect | 256 | 154 | 37.231 ms | 8.676 ms |

The 65,536-block query is explicitly coverage-clipped to the existing
16-by-16 plan cap. It is not evidence for a complete 65 km solve and is not a
500 km performance claim.

Terrain Lab now presents four synchronized Planner-atlas panels. Panel D
draws purple parent facts, cyan regional children, green local children,
magenta basin terminals, and optional conservative bounds. Parent, regional,
local, and bounds overlays are independently URL-addressed. Overlay changes
do not rebuild facts or change checksums. The additive atlas schema is now
`mclone-streamed-plan-atlas-v2`; its pinned one-region checksums are:

- fallback:
  `d2e33dd8cb93c5ac15546b4bd639550ee6fb033aca19809caf831828777651e1`;
- hierarchy:
  `eb7eeafdbb31765c6bd5da2f87fecda79c18d524b92db8ef0cdd05695c9e7744`;
- graph:
  `6454698085bf76ac80161fd03c700c018da1c783b03272419dbb4e2992c53eb5`;
  and
- multiscale witness:
  `303a2e4d2e9a72eb0eb59c7a5b2c23d004fef7e1f0e37621c913527006e80d78`.

Headed desktop and phone tests inspect a broad 16,384-block plane view, toggle
regional/local presentation while preserving the multiscale checksum, and
report zero visible invariant failures. The existing torus test now also
preserves the fourth checksum and exercises cache reuse across one complete
period. Captures remain under:

```text
/tmp/mclone-terrain-lab-*-multiscale-refinement.png
/tmp/mclone-terrain-lab-*-multiscale-parent-only.png
/tmp/mclone-terrain-lab-phone-chrome-multiscale-review-viewport.png
/tmp/mclone-terrain-lab-*-streamed-atlas-torus*.png
```

This is a positive mechanism result, not a terrain-quality result. It proves
that bounded top-down semantic refinement can be directly queried,
order-independent, topology-aware, and cheap enough for interactive research.
The axes and routes remain schematic; they have not influenced a heightfield,
water, coasts, chunks, or selective density.

Review links when Terrain Lab is running locally:

```text
http://127.0.0.1:5180/terrain/?seed=-98765&x=0&z=0&blocks=16384&panes=atlas&view=map&atlasTopology=plane&atlasWitnessParent=1&atlasWitnessRegional=1&atlasWitnessLocal=1&atlasWitnessBounds=1
http://127.0.0.1:5180/terrain/?seed=-98765&x=0&z=0&blocks=6144&panes=atlas&view=map&atlasTopology=cylinder-x&atlasWitnessParent=1&atlasWitnessRegional=1&atlasWitnessLocal=1&atlasWitnessBounds=1
http://127.0.0.1:5180/terrain/?seed=-98765&x=0&z=0&blocks=6144&panes=atlas&view=map&atlasTopology=torus&atlasWitnessParent=1&atlasWitnessRegional=1&atlasWitnessLocal=1&atlasWitnessBounds=1
```

Validation completed:

- five focused native witness tests;
- five native atlas tests;
- all five pinned `streamed_plan_wasm` tests through
  `wasm-bindgen-test-runner`;
- seven `mclone-terrain-lab` library and seven browser-ownership tests;
- 23 Terrain Lab state/URL tests, TypeScript compilation, and production web
  build;
- headed desktop and phone torus and broad-plane tests; and
- the three pinned Mclone Overworld field, biome/surface-language, and surface
  chunk controls.

## Commit And Evidence Trail

1. Commit this tactical before code.
2. Commit the Rust witness, receipt, and exact tests independently of browser
   presentation.
3. Commit the Terrain Lab panel and validation separately.
4. Keep JSON receipts and screenshots under `/tmp`.
5. Inspect the first rendered desktop and phone pixels before closing the
   slice.
6. Update this tactical and the living topic with exact results and the human
   handoff state.
7. Use the exact trailer `Topic: multiscale-terrain-representation` for this
   series.

## Exit Condition

This tactical reaches its review gate only when:

- the parent and child contracts are implemented in shared Rust;
- every exact invariance and consistency test passes;
- native and Wasm witnesses agree;
- production fingerprints remain unchanged;
- the fourth atlas panel is freely pannable and its overlays are independent;
- broad, close, seam, desktop, and phone pixels have been inspected;
- costs and fact counts are reported honestly; and
- production terrain remains disconnected.
