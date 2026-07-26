# Tactical 258: Mclone Macro Terrain Performance Baseline

Status: active 2026-07-26.

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
