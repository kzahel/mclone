# Tactical 257: Topology Worldgen Conformance Probe

Status: complete 2026-07-26.

Topics:

- `bounded-world-topology`
- `world-generation-profiles`
- `mclone-overworld-generation`

Workstream: shared Rust world generation, topology-aware planning, worker
execution, persistence, and deterministic server fixtures.

## Objective

Add a deliberately adversarial topology-aware generator that makes seam
failures inevitable and easy to localize.

The probe complements two existing proofs:

- Flat Grass exercises authority, observation, meshing, lighting, fluids, and
  persistence without a coordinate-dependent terrain seam; and
- the real Mclone cylinder proves production fields, streams, vegetation,
  features, and stored chunks, but a natural seed does not guarantee that
  every future mechanism intersects the seam.

The result is a reusable conformance kernel plus a real hidden internal
`topology-probe-v1` profile. It runs through the ordinary dimension
descriptor, scheduler, worker codec, generated chunks, lighting, fluids, and
persistence, but it is omitted from the normal world-creation catalog.

This tactical does not change Mclone terrain output, add Mclone 3D noise,
enable periodic distant terrain, or promote the probe into a player-facing
terrain choice.

## Fixture Contract

The default interactive fixture is the existing 32-chunk X-periodic cylinder.
The conformance kernel also accepts:

- an unbounded plane as an explicit control;
- supported X-periodic cylinders with enough circumference for the complete
  fixture; and
- a two-axis periodic flat torus for direct server and generator tests.

Finite axes and mixed shapes the probe does not understand fail during
dimension validation. Periodic axes must be large enough that every bounded
probe formation has an influence diameter below half the period.

Interactive local sessions also retain the ordinary one-lift view contract.
Their accepted tracking radius plus unload hysteresis must fit inside the
period. For example, render distance 2 derives tracking radius 3 and unload
radius 4, so an eight-chunk period is valid for direct generator/server
fixtures but rejected before local runtime startup; the 32-chunk default is
valid.

The generated landscape is intentionally diagnostic rather than attractive.
It guarantees the following near the canonical seam:

- a smooth bounded ridge/valley response whose discrete values and slopes
  repeat across signed lifts;
- a flat contained channel crossing the seam;
- a split/rejoin reach enclosing one small island;
- a shallow seam-straddling pond;
- a bounded stone arch with one canonical owner and clipped pieces on both
  sides of the seam;
- material bands that make canonical phase and accidental duplicate output
  visible;
- one stable light source site; and
- source water suitable for an authoritative wake test.

The arch is bounded three-dimensional geometry, not a claim that a general
3D density or noise system exists. When a real density caller lands, it must
join the conformance suite with value, horizontal-derivative, interpolation,
and isosurface assertions.

## Identity And Work Contract

The existing `WorldGenerationDescriptor` remains the authoritative context:

```text
profile + seed + horizontal topology
```

No second process-global or thread-local topology context is introduced.
The probe source retains the compact descriptor facts it repeatedly needs.

Probe plans distinguish:

1. a canonical owner and stable plan identity;
2. a target-relative Euclidean work lift used to clip bounded geometry; and
3. final canonical chunk/block ownership.

The same plan queried from `-1`, `period - 1`, `period`, or another lift must
retain one owner and identity. Combined, reversed, and partitioned target
batches must produce identical canonical chunks.

The pure probe kernel remains target-independent, while its server generation
plan declares a one-chunk `Surface` input ring. That ring is the ordinary
initial-lighting input contract and lets a light source in a canonical seam
neighbor be unfolded beside the target before light sections are published.

## Profile And Product Boundary

`topology-probe-v1` is a compiled, persisted, internal-mutable profile so it
can exercise real worker and save/reopen paths. It receives a unique binary
codec tag and a CLI/debug label.

It is deliberately absent from
`LOCAL_WORLD_PROCEDURAL_GENERATION_PROFILES` and from the normal next-profile
cycle. UI code may display a diagnostic name if handed an already-created
probe world, but ordinary catalog creation does not offer it.

The compatibility ledger records no preservation consumer. Probe output may
change whenever its assertions, tactical record, and disposable internal
worlds are updated together.

## Defensive Mclone Boundary

This tactical changes no Mclone output. It adds defensive coverage around the
existing implementation:

- tests alternate plane and periodic descriptors through one resident Mclone
  executor and compare them with fresh executors, proving topology-qualified
  cache identity;
- a shared seam-coordinate corpus covers `-1`, `0`, `P - 1`, `P`, `P + 1`,
  signed laps, and the deterministic half-period tie where applicable;
- authoritative server and worker paths continue deriving Mclone topology
  from the stored descriptor; and
- ambiguous unbounded convenience constructors are named explicitly where
  doing so can be completed without a noisy compatibility shim.

The ordinary unbounded convenience APIs may remain as deliberate tool/test
entry points, but code that owns a dimension descriptor must not call them.

## Implementation Slices

### Slice 0: tactical and baseline

- [x] Record the fixture, hidden-profile, topology, and Mclone defensive
  contracts.
- [x] Confirm the current descriptor already carries profile, seed, and
  topology through scheduler and worker boundaries.
- [x] Confirm Flat Grass and real Mclone seam tests remain complementary
  rather than superseded.
- [x] Run focused green baselines before generator behavior changes.

Gate: the tactical adds no second context abstraction and names every intended
production boundary.

### Slice 1: reusable probe kernel

- [x] Add the topology-aware sample/plan/generation module in
  `mclone-worldgen`.
- [x] Validate supported plane, cylinder, and torus shapes and minimum
  periodic extents.
- [x] Generate the ridge, channel, island, pond, arch, material, light, and
  fluid canaries from one deterministic implementation.
- [x] Prove signed-lift equality, seam adjacency, stable plan ownership,
  batch order/partition independence, and torus-corner behavior.
- [x] Keep the module free of server, app, persistence, and renderer policy.

Gate: pure generation makes every selected seam event unavoidable and exact.

### Slice 2: hidden profile and worker execution

- [x] Add `topology-probe-v1` to the stored profile enum, labels, parsing,
  codec tags, planning, spawn, and closed worker dispatch.
- [x] Keep it out of normal catalog selection and prove that omission.
- [x] Round-trip plane, cylinder, and torus descriptors through the worker
  request/response codec.
- [x] Reject unsupported probe topology before scheduling generation.
- [x] Record the profile as internal-mutable in the compatibility ledger.

Gate: the probe is a real descriptor-selected generator without becoming a
normal product choice.

### Slice 3: authoritative runtime and persistence

- [x] Load seam views through the integrated server and inspect generated
  canonical/lifted block facts.
- [x] Wake the contained water body and prove no seam spill or mutation.
- [x] Recompute block light across the generated seam canary.
- [x] Save seam chunks and edits through SQLite, reopen, and prove only
  canonical chunk keys exist.
- [x] Exercise a small torus corner through canonical views and generated
  chunks.

Gate: generator, simulation, lighting, and persistence agree on the probe's
canonical topology.

### Slice 4: Mclone defensive corpus and closeout

- [x] Add the alternating plane/cylinder resident-cache isolation test.
- [x] Consolidate reusable seam coordinates without weakening existing
  field, feature, stream, or vegetation fixtures.
- [x] Run focused worldgen/server tests, all affected workspace test targets,
  and the browser/Wasm compile boundary.
- [x] Update topic status, support matrices, compatibility ledger, and this
  execution record with exact results and remaining exclusions.

Gate: future Mclone terrain families have a reusable topology acceptance lane,
while the probe and production terrain retain distinct responsibilities.

## Required Assertions

- exact profile/topology validation and codec round trips;
- canonicalization idempotence for generated targets;
- `chunk(c) == chunk(c + period)` across signed X and Z lifts;
- matching seam-side height differences;
- one canonical arch/plan identity across every lift;
- combined, reversed, and partitioned batch equality;
- no duplicate canonical output or persistence keys;
- flat contained source water remains quiescent when woken;
- light reaches the opposite seam neighbor;
- torus corner neighborhoods wrap and deduplicate on both axes;
- normal catalog cycling never selects the hidden probe;
- one resident Mclone executor isolates plane and cylinder cache state; and
- unchanged reference Overworld and existing Mclone regression locks.

## Validation Boundary

The minimum closeout is:

- `mclone-worldgen` focused tests;
- `mclone-server` profile, worker-codec, integrated topology, fluid, light,
  and SQLite tests;
- `mclone-app-runtime` catalog tests;
- all affected workspace test targets compiled;
- browser/Wasm compilation of the shared worker path; and
- an inspected native/offscreen probe seam capture if the existing
  profile-selectable capture path can exercise the hidden label without
  broadening product UI.

Screenshots remain under `/tmp`. Pixel evidence is diagnostic; exact generated
facts are the determinism oracle.

## Execution Record

The implementation landed as the following reviewable sequence:

- `3209c1e1` planned the hidden conformance profile and defensive Mclone
  boundary;
- `08318c8c` added the pure plane/cylinder/torus probe kernel;
- `206a5abb` integrated the hidden stored profile and worker codec;
- `94db3bec` added authoritative runtime, fluid, lighting, torus, and SQLite
  proofs;
- `65268c50` locked Mclone descriptor-qualified caches and the signed seam
  corpus;
- `ed4ca50f` added local-view topology preflight and a real period-32 startup
  pump proof; and
- `1e4c3b4e` preserved the selected profile in transient scene requests.

The runtime pass exposed two useful defects rather than merely confirming the
happy path:

- retained lighting lacked a light-only adjacent air section, so the generated
  torch could not propagate through empty seam-neighbor space; the light engine
  now activates a bounded envelope around retained nonempty sections; and
- a local period too small for unload hysteresis failed asynchronously inside
  the server thread and left startup waiting forever. Shared policy preflight
  now returns the exact duplicate-lift error before startup.

Final validation on 2026-07-26:

- `cargo test -p mclone-worldgen --lib`: 359 passed, 1 ignored;
- `cargo test -p mclone-light --lib`: 55 passed;
- `cargo test -p mclone-server --lib`: 560 passed;
- `cargo test -p mclone-app-runtime --lib`: 279 passed;
- `cargo test -p mclone-scene --lib`: 158 passed;
- `cargo check --workspace --all-targets`: passed with pre-existing warnings;
- `pnpm native:web:build`: passed for `wasm32-unknown-unknown` with
  pre-existing warnings; and
- native lit period-32, render-distance-10 captures at 1280 by 720 ran a
  fixed 4,096-frame warmup and were inspected at
  `/tmp/mclone-topology-probe-arch-frames.png` (929 sections, 189 drawn) and
  `/tmp/mclone-topology-probe-hydrology-frames.png` (929 sections, 307 drawn).

The arch capture shows the bounded stone ring and torch through the ordinary
renderer and lighting path. The hydrology capture shows the split/rejoin
channel and enclosed island. These pixels are presentation evidence only;
exact signed-lift, plan, fluid, light, torus-corner, and persistence assertions
remain the acceptance oracle.

Earlier playable/idle-gated frames were discarded because their hard chunk
edges showed that the complete server view had not reached the render cache.
The current offscreen `Idle` policy can observe zero pending render work for
already-admitted chunks before server-side view streaming completes; it is not
used as the capture-settlement claim here.

No Mclone terrain output changed. General 3D density, product torus authority,
periodic distant-terrain presentation, and multiple simultaneously visible
lifts remain outside this tactical.

## Stop Conditions

Split follow-up work rather than broadening this tactical if completion
requires:

- a general dynamic generator registry;
- a universal topology-aware feature framework without a second production
  caller;
- Mclone terrain, hydrology, or decoration output changes;
- production 3D density/noise implementation;
- periodic distant-terrain presentation;
- multiple simultaneously visible lifts; or
- app-local or TypeScript topology/generation policy.

## Related

- [`../topics/bounded-world-topology.md`](../topics/bounded-world-topology.md)
- [`../topics/world-generation-profiles.md`](../topics/world-generation-profiles.md)
- [`../topics/mclone-overworld-generation.md`](../topics/mclone-overworld-generation.md)
- [`../topics/mclone-macro-landscape-planning.md`](../topics/mclone-macro-landscape-planning.md)
- [`195-periodic-cylinder-topology-proof.md`](195-periodic-cylinder-topology-proof.md)
- [`196-periodic-mclone-terrain-fields.md`](196-periodic-mclone-terrain-fields.md)
