# Vanilla Fast Macro Terrain Preview

Status: completed (2026-07-25).

Topic: `vanilla-terrain-lod`

## Objective

Turn the first correctness-oriented Java 1.17.1 Terrain Lab sampler into a
two-fidelity research workspace:

- **Sampled exact** preserves vanilla density interpolation at every requested
  point while avoiding density columns that have zero interpolation weight.
- **Fast macro** approximates the same terrain surface from a bounded subset of
  the vanilla density field and is explicitly optimized for broad,
  kilometer-scale review.

Terrain Lab must be able to show either product independently or compare both
at the same seed, coordinates, footprint, sample spacing, camera, and layer.
The fast product remains presentation-only and cannot satisfy canonical chunk,
collision, persistence, protocol, or gameplay obligations.

## Originating Direction

Human review of the first vanilla Terrain Lab slice found that a phone can take
about 20 seconds to refine a roughly 2-4 km view. The intended experience was a
quick approximation of recognizable vanilla macro heights, not an exact call
to the complete vertical density operation at every visible sample.

The existing implementation already excludes chunks, features, structures,
carvers, decoration, vegetation, lighting, and exact surface mutation. Its
remaining cost is the terrain-defining 3D density field itself. Java 1.17.1
does not expose a separate cheap two-dimensional height field, so the fast
product must be an explicit Mclone approximation whose error is measured
against the sampled-exact product.

Review also found an independent exact-path optimization. Terrain Lab tile
origins and every power-of-two spacing of four blocks or greater lie on
vanilla's four-block horizontal density lattice. At those coordinates the
horizontal interpolation fractions are both zero, but the direct translation
of `iterateNoiseColumn` still generates all four surrounding density columns.
Only the first column contributes to the result. Skipping the three
zero-weight columns preserves exact sampled output.

## Product Contract

For the global `overworld` profile, the procedural pane choices are:

- `Fast macro` — bounded approximate density evaluation;
- `Sampled exact` — direct vanilla density-column height sampling; and
- both — independently generated, progressively published comparison panes.

`Real terrain` remains the separate canonical Surface/Final chunk pane. The
Mclone profile retains its current CPU/GPU terminology and behavior.

Fast and exact products:

- use the same 65-by-65 aligned tile identities and viewport levels;
- have separate source revisions, cache readiness, Worker requests, timings,
  and stale-result rejection;
- publish a complete coarse parent before target detail;
- can be toggled without changing seed, center, camera, or footprint; and
- never masquerade as one another in labels, reports, or cache keys.

Terrain Lab comparison reports must include mean, P95, and maximum solid and
display height error plus water-presence agreement. The first accepted macro
algorithm is selected by measured speed/quality evidence, not by assuming that
fewer density evaluations remain recognizable.

## Algorithm Workstream

### Slice 1: Sampled-Exact Waste Removal

Generate only density columns with non-zero horizontal interpolation weight:

- one column at a density-lattice corner;
- two columns when exactly one horizontal fraction is zero; and
- four columns at an arbitrary interior block coordinate.

Existing full-column and chunk comparison fixtures remain byte-identical.
Tests pin the expected one/two/four generated-column counts.

### Slice 2: Fast Macro Candidates

Keep candidate algorithms behind one revisioned shared sampler and compare
them before locking the first product revision:

1. biome depth/scale plus vanilla's two-dimensional random density offset;
2. sparse full-density vertical samples with coarse interpolation; and
3. a biome-predicted bracket refined by a small bounded number of full-density
   probes.

The initial implementation should favor a simple deterministic baseline with
a clearly declared vertical probe count. It may reuse exact vanilla biome,
water-fill, and approximate top-material classification. It must not generate
complete chunks or call the canonical surface/features pipeline.

### Slice 3: Independent Browser Products

Expose a dedicated fast-macro Wasm compiler and Worker request kind. The
renderer owns independent exact and macro resident products rather than
calling a CPU approximation from app-local presentation code. Fast and exact
may progress concurrently, but bounded Worker and per-frame admission policy
must keep interaction responsive on phones.

### Slice 4: Evidence-Guided Refinement

Record fixed desktop and phone comparisons at local, 2 km, and broader
footprints. Improve the macro algorithm only when a change materially improves
height/water/silhouette evidence for an acceptable cost. A later GPU evaluator
is a follow-up after the CPU algorithm and revision are understood.

## Ownership

- `mclone-worldgen` owns exact optimization, macro semantics, revisions,
  samples, and exact-versus-macro comparison facts.
- `mclone-terrain-view` owns independent resident products, aligned comparison
  rendering, readiness, and cache accounting.
- `mclone-terrain-lab` owns the narrow Wasm compiler and typed transfer facade.
- `tools/terrain-lab` owns Workers, URL/pane state, responsive layout, labels,
  and browser evidence.

No app or renderer crate owns the approximation's terrain policy.

## Acceptance

- Aligned sampled-exact queries generate one density column and retain exact
  height, water, biome, and material output.
- Arbitrary sampled-exact queries preserve reference parity and use only the
  mathematically necessary one, two, or four density columns.
- Fast macro has a separate revision and deterministic cold/warm output.
- A fixed comparison receipt records timing, mean/P95/maximum height error,
  and water agreement for at least two seeds and negative coordinates.
- Terrain Lab exposes `Fast macro` and `Sampled exact` panes for `overworld`,
  including a simultaneous comparison view and truthful readiness labels.
- Mclone `CPU LOD` and `GPU LOD` behavior and URL normalization remain
  unchanged.
- Rust, TypeScript, Wasm, production build, headed desktop WebGPU, and headed
  phone WebGPU validation pass.
- Pixel evidence is captured outside the repository and inspected before the
  tactical is closed.

## Implementation Order

1. Record this contract and reopen the vanilla terrain LOD topic.
2. Land sampled-exact one/two/four-column selection and parity tests.
3. Implement and benchmark the first shared macro sampler.
4. Add exact-versus-macro comparison metrics and fixed fixtures.
5. Add independent resident/Worker products and UI pane vocabulary.
6. Run rendered-output validation and inspect desktop/phone captures.
7. Record the selected algorithm, performance/accuracy evidence, revisions,
   deployment receipt, and remaining experiments.

Commit each coherent slice with `Topic: vanilla-terrain-lod`.

## Selected Algorithm

The accepted `vanilla-1.17.1-sparse-density-column-lod-v1` macro sampler is a
hybrid of the first two candidates:

- select the center noise biome instead of evaluating vanilla's complete
  5-by-5 weighted biome neighborhood;
- evaluate the ordinary vanilla random density offset and blended 3D noise;
- retain 9 vertical density nodes per column, every fourth vanilla vertical
  cell, instead of all 33 nodes;
- linearly reconstruct the intervening 32-block vertical intervals;
- preserve vanilla solid/fluid resolution, block-position biome selection,
  water presence, and approximate top-material classification; and
- retain columns in the same bounded absolute-coordinate cache as sampled
  exact.

A five-node candidate using every eighth vertical cell was rejected. On the
two fixed 2 km fixtures it improved host speed only modestly beyond the
nine-node candidate but reduced water agreement to 84.8-87.8%, raised mean
solid error to 13.8-19.9 blocks, and raised P95 solid error to 42 blocks.
That changed coastlines and broad relief too aggressively.

## Performance And Error Receipt

The repeatable native command is:

```sh
cargo run -p mclone-worldgen --bin vanilla_lod_perf -- \
  --seed <seed> --center-x <x> --center-z <z> \
  --spacing 32 --cells 64
```

The optimized development-profile host results for 65-by-65, 2,048-block
grids were:

| Seed and center | Exact | Macro | Speedup | Solid mean / P95 / max | Display mean / P95 / max | Water |
|---|---:|---:|---:|---:|---:|---:|
| `12345`, `0,0` | 163.8 ms | 47.5 ms | 3.45x | 3.63 / 11 / 38 | 3.51 / 11 / 38 | 96.59% |
| `-98765`, `-304,336` | 289.7 ms | 74.8 ms | 3.87x | 4.92 / 16 / 58 | 1.68 / 8 / 58 | 94.18% |

The headed Chrome Pixel 7 viewport lane used the production Wasm Workers and
the same 2,048-block comparison:

- Fast macro reached target detail in 462.3 ms;
- Sampled exact reached target detail in 1,947.7 ms, 4.21x later;
- solid error was 3.72 mean, 12 P95, and 38 maximum blocks;
- display error was 3.26 mean, 11 P95, and 38 maximum blocks; and
- water-presence agreement was 96.30%.

The corresponding desktop viewport reached macro target in 2,526.2 ms and
exact target in 9,425.4 ms. Its comparison reported 4.22 / 13 / 69 solid
error, 2.92 / 11 / 69 display error, and 95.86% water agreement. Browser
target times include progressive viewport planning, Worker startup, transfer,
admission, and publication; the native command isolates cold grid compilation.

## Implemented Product

- Sampled exact now generates only the one, two, or four horizontal density
  columns with non-zero interpolation weight.
- Fast macro and Sampled exact have distinct shared samplers, revisions, Wasm
  compilers, Workers, queues, in-flight maps, resident buffers, timers, and
  stale-result validation.
- `overworld` exposes `Real terrain`, `Sampled exact`, and `Fast macro`.
  Selecting both procedural products uses the existing synchronized split
  presentation and exact-versus-candidate error layer.
- Comparison reports now include mean, P95, and maximum solid and display
  height error plus water agreement.
- `mclone-overworld-v1` retains its CPU/GPU labels, sources, normalization,
  compute dispatch, and comparison behavior.

## Validation Receipt

Passed:

```sh
cargo test -p mclone-worldgen --lib
cargo test -p mclone-terrain-view --lib
cargo test -p mclone-terrain-lab --lib
cargo check --target wasm32-unknown-unknown -p mclone-terrain-lab
pnpm --dir tools/terrain-lab test
pnpm --dir tools/terrain-lab typecheck
pnpm --dir tools/terrain-lab web:build
pnpm host:check -- --probe-browser-webgpu
pnpm --dir tools/terrain-lab exec playwright test \
  -c playwright.config.ts \
  --grep "switches the whole lab to worker-backed vanilla terrain"
```

The focused headed-WebGPU test passed for desktop Chrome and the Pixel 7
phone viewport. The ordinary Mclone comparison regression passed on desktop
under the normal cap and on the phone viewport with a 180-second cap; the
complete phone interaction/stress workflow took 1.6 minutes. The focused
vanilla phone acceptance path passed in 14.1 seconds.

Pixel evidence was captured and inspected at:

- `/tmp/mclone-terrain-lab-desktop-chrome-vanilla-workspace.png`
- `/tmp/mclone-terrain-lab-phone-chrome-vanilla-workspace.png`

The captures show matching broad land/water masses with the intended smoother
and locally shifted macro relief, correct independent readiness labels, and a
usable side-by-side desktop and stacked phone layout.

## Commit Receipt

- `92cd71cd` records this tactical and reopens the durable topic.
- `3b056495` removes zero-weight sampled-exact density columns.
- `44af6f29` adds the shared macro sampler, metrics, tests, and benchmark.
- `6182bb60` adds independent renderer, Wasm Worker, URL, and pane products.

## Remaining Experiments

The nine-node revision is the accepted first fast product, not the end of the
accuracy workstream. A bounded predicted-height bracket may improve peaks and
coast transitions without returning to the 33-node exact budget. Any
replacement must receive a new macro revision and beat the fixed timing/error
receipt. A GPU implementation remains optional follow-up work after the CPU
semantics are stable.
