# Tactical 258: Mclone Macro Terrain Performance Baseline

Status: complete 2026-07-26.

Topics:

- `mclone-macro-landscape-planning`
- `mclone-overworld-generation`

Workstream: production Mclone terrain sampling, exact generation, and
procedural-horizon streaming evidence.

## Objective

Establish a reproducible performance baseline before adding macro terrain
planning, selective 3D density, or new review products.

This tactical measures the production paths that exist today. It does not
change terrain output, add a planning framework, define a composite review
map, or set pass/fail budgets from a single host run.

The result must answer four distinct questions:

1. how cheaply can the production two-dimensional Mclone field graph sample
   independent world positions;
2. how much additional work does the production preview compiler perform for
   a bounded grid and content stage;
3. how quickly do exact surface and decorated chunks generate with cold and
   warm dependency state; and
4. how quickly and steadily does the fixed-budget World Explorer horizon
   become ready and move through the same production source.

## Normalized Workloads

Every broad-preview result records all of:

- footprint blocks per axis;
- sample spacing in blocks;
- samples per axis and total lattice points;
- actual terrain sample evaluations;
- content stage;
- topology;
- seed and center; and
- iteration count and warm-up policy.

Distance alone is never a valid workload identifier. The initial matrix uses:

| Name | Footprint | Spacing | Samples per axis | Lattice points |
|---|---:|---:|---:|---:|
| regional-65k | 65,536 blocks | 1,024 | 65 | 4,225 |
| horizon-131k | 131,072 blocks | 1,024 | 129 | 16,641 |
| continental-500k-coarse | 499,712 blocks | 2,048 | 245 | 60,025 |
| continental-500k-medium | 499,712 blocks | 1,024 | 489 | 239,121 |
| continental-500k-fine | 499,712 blocks | 512 | 977 | 954,529 |

The non-power-of-two 245-, 489-, and 977-point grids deliberately benchmark
the production point sampler rather than pretending they are valid single
Terrain Lab tiles. Bounded preview-compiler receipts use its real
power-of-two cell limits and report their own footprints.

## Receipt Contract

Add a schema-versioned JSON receipt for the missing normalized sampler and
preview-compiler workloads. It must record:

- repository revision and dirty state;
- build profile, operating system, architecture, and logical CPU count;
- seed, center, topology, workload identity, extent, spacing, and point
  counts;
- minimum, median, and maximum elapsed time across measured iterations;
- points or evaluations per second;
- a deterministic output checksum so the optimizer cannot erase the work;
- allocation/output bytes where the path materializes a grid; and
- separate generation and packing timings where packing is exercised.

The sampler lane should time production `McloneOverworldSampler::sample`
calls directly without retaining a million rich sample structures. The
preview lane should time `TerrainPreviewReferenceGrid::compile` separately
from packing its production 32-float samples.

Exact-generation evidence continues to use `worldgen_perf`; this tactical may
extend its metadata or correctness only when required for a usable receipt.
It must retain distinct Mclone surface, cold decorated, and warm decorated
phases. The World Explorer smoke receipt remains the streaming owner and
must retain first-coarse, first-target, movement, settling, fixed residency,
refill, and rebase evidence.

## Measurement Policy

- Run release builds on one named host and one commit.
- Perform an unmeasured warm-up before reporting warm sampler or preview
  iterations.
- Use at least five measured iterations for the cheap sampler and preview
  lanes; use enough exact and streaming repetitions to expose obvious
  variance without turning this tactical into a lab campaign.
- Record raw receipts under `/tmp`; commit the command, workload definition,
  and a concise result table, not generated benchmark artifacts.
- Keep CPU generation, packing, transfer, validation, GPU execution, and
  presentation separate. A path that does not exercise one of those stages
  records it as absent rather than rolling in an estimate.
- Treat current numbers as baselines, not thresholds. A later regression
  budget requires repeated same-host evidence and an explicit product
  decision.

## Implementation Slices

### Slice 0: Workload and owner audit

- identify the production point sampler and preview compiler;
- verify the exact-generation and World Explorer receipt owners; and
- freeze the normalized workload and receipt contract above.

Acceptance:

- no proposed lane silently substitutes a synthetic noise kernel for a
  production Mclone path; and
- the tactical and index land before harness implementation.

### Slice 1: Normalized sampler and preview receipt

- add a small shared-worldgen benchmark binary;
- cover the five point-sampler workloads;
- cover bounded Base, Surface, and Cover preview compiler workloads that fit
  the production tile contract;
- time optional packing separately; and
- add argument and receipt-shape tests where practical.

Acceptance:

- one release command writes valid JSON with the complete workload identity;
- checksums are stable across repeated runs at the same seed and topology;
- point/evaluation counts match the declared grids; and
- the harness does not change production output.

### Slice 2: Exact and streaming receipts

- run the existing exact Mclone surface/cold/warm benchmark at a declared
  chunk footprint;
- run the native World Explorer movement smoke;
- inspect its rendered checkpoints because the streaming lane produces
  pixels; and
- preserve stage boundaries rather than aggregate unlike work.

Acceptance:

- exact receipts distinguish target chunks from generated dependencies;
- streaming finishes with fixed allocation, no pending work, and complete
  movement/settling summaries; and
- captured terrain is complete rather than an intermediate partially loaded
  frame.

### Slice 3: Baseline and closeout

- record the host, commit, commands, raw receipt locations, and concise
  results;
- update the macro-planning and Mclone-overworld topic docs;
- replace the vague “composite review first” direction with performance
  baselining as the completed prerequisite; and
- identify the next bounded terrain-planning question without inventing a
  generic framework.

Acceptance:

- a maintainer can rerun each lane without reconstructing chat context;
- the topic docs distinguish measured facts from future budgets; and
- the tactical closes with no terrain-output changes.

## Deferred Work

- composite terrain/water/coast review maps before their concrete consumers
  are selected;
- a 500 km rendered product or GPU presentation benchmark;
- active 3D density, caves, overhangs, arches, or geological formations;
- automatic regression thresholds;
- cross-host normalization; and
- a general macro-planning context or topology abstraction.

When selective 3D terrain work begins, extend this baseline with separate
ordinary-path, regional-hotspot, bounded-landmark-hotspot, and far-summary
lanes. Do not compare a dormant selector against a dense showcase and call
the difference the cost of “3D noise.”

## Execution Record

Implementation landed in `e1d0341b`:

- `mclone_macro_perf` samples the production point graph at all five declared
  grids;
- the same receipt measures bounded Base, Surface, and Cover preview
  compilation plus packing;
- every lane records raw iteration times, median/range, actual work counts,
  stable checksums, materialized bytes, host/git metadata, and absent stages;
  and
- `pnpm native:worldgen:macro-perf -- ...` is the durable release entry point.

The baseline ran on commit `e1d0341bff466bd52ec47c4ee45b1420c2a784e7`
using Ubuntu 24.04.4, Linux 7.0, and an AMD Ryzen AI 9 365 with 20 logical
CPUs. The repository dirty bit was true only because of a pre-existing,
unrelated formatting change in
`mclone-server/src/integrated/tests/player_state.rs`; benchmark sources matched
the named commit.

Raw receipts were written to `/tmp/mclone-macro-baseline-e1d0341b`. They are
deliberately not repository artifacts.

### Production point sampling

Each number is the median of five release iterations after one warm-up.

| Workload | Points | Plane | Cylinder X:384 | Cylinder / plane |
|---|---:|---:|---:|---:|
| regional 65 km | 4,225 | 3.229 ms | 4.277 ms | 1.32x |
| horizon 131 km | 16,641 | 9.833 ms | 11.452 ms | 1.16x |
| continental 500 km coarse | 60,025 | 33.283 ms | 40.071 ms | 1.20x |
| continental 500 km medium | 239,121 | 97.892 ms | 111.483 ms | 1.14x |
| continental 500 km fine | 954,529 | 392.000 ms | 448.237 ms | 1.14x |

The finest plane lane sustained about 2.44 million full terrain samples per
second; the periodic lane sustained about 2.13 million. The smallest lane is
more sensitive to fixed cost and host noise than the three continental
lanes. The topology cost is real, but this point-sampling evidence does not
support avoiding the seam or creating a special low-quality boundary zone.

### Production preview compilation

| Workload | Lattice / terrain evaluations | Plane compile | Cylinder compile | Plane pack |
|---|---:|---:|---:|---:|
| Base 65 km | 4,225 / 4,225 | 1.948 ms | 2.145 ms | 0.119 ms |
| Base 131 km | 16,641 / 16,641 | 7.746 ms | 8.464 ms | 0.477 ms |
| Surface 65 km | 4,225 / 4,225 | 1.904 ms | 2.107 ms | 0.070 ms |
| Cover 65 km | 4,225 / 21,125 | 9.563 ms | 10.645 ms | 0.070 ms |

Each 65 km product packs 540,800 bytes; the 131 km product packs 2,130,048
bytes. Cover performs five terrain evaluations per lattice point plus its
declared forest work. No transfer, GPU execution, validation beyond checksum,
or presentation time is hidden in the compile number.

### Exact generation

Three independent release invocations of `worldgen_perf` generated a centered
3-by-3 target footprint. Medians were:

| Topology | Surface, 9 chunks | Cold decorated, 9 targets / 49 dependencies | Warm decorated, 49 cache hits |
|---|---:|---:|---:|
| plane | 7.905 ms | 32.643 ms | 3.504 ms |
| cylinder X:384 | 13.243 ms | 55.748 ms | 3.928 ms |

The cylinder's cold exact path costs materially more than its point-preview
path, while the warm decorated path is close. That is a reason to retain
separate ordinary, cold-planning, and warm-cache lanes in future work rather
than summarize topology with one multiplier.

### Fixed-budget streaming

The World Explorer smoke exercised independent native-window and offscreen
sessions on the integrated Radeon GPU.

The native-window session reached first coarse terrain in 123.906 ms and the
complete target in 394.627 ms. Its 32 movement frames averaged 1.952 ms with a
3.659 ms p95; settling frames averaged 2.082 ms with a 2.888 ms p95. It ended
with all 160 slots ready, zero pending refills, 86,553,600 fixed resident
bytes, 811 total refills, and 26 rebases.

The offscreen control reached first coarse terrain in 34.176 ms and the target
in 484.793 ms. Movement averaged 2.868 ms with a 4.987 ms p95; settling
averaged 2.371 ms with a 3.345 ms p95. It also ended at 160 ready slots and
zero pending work.

All six native-window checkpoints were inspected together after capture.
Their receipts each reported 160 ready slots and zero pending refills, and
the 3D, movement, negative-coordinate, million-block teleport, map, and orbit
views showed complete continuous terrain rather than intermediate loading
holes.

### Reproduction commands

From the repository root:

```bash
pnpm native:worldgen:macro-perf -- \
  --iterations 5 \
  --warmup-iterations 1 \
  --output /tmp/mclone-macro-plane.json

pnpm native:worldgen:macro-perf -- \
  --topology cylinder-x:384 \
  --iterations 5 \
  --warmup-iterations 1 \
  --output /tmp/mclone-macro-cylinder.json

cargo run --release --manifest-path native/Cargo.toml \
  -p mclone-worldgen --bin worldgen_perf -- \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --radius 1 \
  --iterations 1 \
  --mclone-topology plane > /tmp/mclone-exact-plane.json

pnpm native:world-explorer:smoke /tmp/mclone-world-explorer-baseline
```

Run the exact command three times with distinct output paths, then repeat it
with `--mclone-topology cylinder-x:384`. Keep each receipt rather than hiding
cross-process variance inside one accumulated timer.

## Validation

Passed:

- `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen --bin
  mclone_macro_perf`;
- release macro receipts for plane and `cylinder-x:384`, five measured
  iterations after one warm-up;
- three independent plane and cylinder exact-generation receipts;
- native-window and offscreen World Explorer movement smoke; and
- visual inspection of the six native-window completed-frame captures.

This tactical sets no regression threshold. It supplies the workload names,
stage boundaries, and first same-host evidence from which a later threshold
can be chosen.
