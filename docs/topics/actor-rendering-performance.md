# Actor Rendering Performance

Topic: `actor-rendering-performance`

Status: prepared-figure admission and per-figure instancing are implemented as
of 2026-07-28. The isolated 1,000-animated-cow lane improved from
`9.184ms` to `1.939ms` average and from 1,000 draws to one. The immediate
product gate is a physical Quest RD5 actor-on/actor-skipped rerun. The next
engine experiments are measured sparse bucket updates, capability-gated GPU
palette expansion, and projected-size actor LOD, in that order unless headset
attribution changes the priority.

## Ownership And Scope

This topic is the durable owner for actor-rendering performance evidence,
memory tradeoffs, benchmark controls, open optimization work, and acceptance
criteria. Future work in this stream should update this document rather than
growing parallel TODO lists elsewhere.

Related owners remain:

- [`compiled-figure-rendering.md`](compiled-figure-rendering.md) owns semantic
  figure preparation, rigid-part animation, prepared GPU contracts, and the
  generated-LOD architecture;
- [`performance.md`](performance.md) owns the cross-system priority queue and
  broader desktop/Quest performance posture;
- [`../performance-records.md`](../performance-records.md) owns dated immutable
  benchmark records; and
- [`../entity-architecture.md`](../entity-architecture.md) owns simulation,
  identity, replication, and actor presentation boundaries.

This topic does not own spawning, AI, persistence, network interpolation,
general XR frame orchestration, or procedural-horizon performance. It covers
those systems only where their output becomes actor render input or where a
shared benchmark must isolate actor cost.

## Current Prepared And Instanced Path

Commits `41fc9fae` and `32fcffaf` established the current path:

1. Every stable entity figure with an available prepared resource uses shared
   immutable local-space vertices, indices, atlas, and pass ranges.
2. Compatible actors are grouped by figure and rendered as instances. Figure,
   LOD, material/alpha pass, and pipeline state are valid bucket boundaries.
3. Every prepared vertex carries a rigid `part_id`.
4. Every actor instance carries a 64-byte affine world transform plus packed
   light, opacity, and a body-palette base.
5. Every actor retains its own evaluated part matrices. Affine 3x4 matrices
   are packed as three `Rgba32Float` texels in a shared palette texture.
6. The vertex shader loads
   `palette[instance.palette_base + vertex.part_id]`, then applies the actor
   world transform.

Animation phase is therefore not a batching boundary. One thousand cows can
have one thousand unrelated walk phases while using one cow draw per compatible
pass. The part palette is shared by mono, per-eye stereo, and full-frame
multiview submissions; it is not duplicated per eye.

Player-specific identities, anonymous actors, debug/item shapes, unsupported
figures, and any figure without a prepared resource retain the combined
CPU-baked fallback. Its coarse whole-list invalidation and repeated index
upload remain bounded follow-up concerns, but they no longer describe ordinary
stable cows.

## Current Performance Baseline

`actor_render_perf` is a deterministic offscreen rendering scene. It creates a
visible actor grid with stable entity identities, supports prepared/legacy,
animated/stationary, cow/chicken/mixed, and arbitrary-count controls, and
measures prepare, encode, submit, and synchronous GPU completion. It excludes
simulation, spawning, terrain, OpenXR, and swapchain presentation.

Five clean release runs on commit `32fcffaf`, Linux 7.0, Ryzen AI 9 365 with
Radeon 880M, used 120 measured frames after 30 warmup frames at `640x360`:

| Animated cows | Avg / mean p95 | Avg-run range | Pose evaluation | Upload | Device poll | Draws / frame |
|---:|---:|---:|---:|---:|---:|---:|
| 10 | `0.145 / 0.189ms` | `0.119–0.202ms` | `0.015ms` | `0.006ms` | `0.094ms` | `1` |
| 100 | `0.373 / 0.428ms` | `0.362–0.397ms` | `0.146ms` | `0.026ms` | `0.170ms` | `1` |
| 1,000 | `1.939 / 2.594ms` | `1.656–2.336ms` | `1.150ms` | `0.195ms` | `0.570ms` | `1` |
| 2,000 | `4.135 / 4.659ms` | `3.942–4.430ms` | `2.137ms` | `0.332ms` | `1.640ms` | `1` |

The immediately preceding non-instanced prepared path measured `0.208ms`,
`0.837ms`, and `9.184ms` average at 10, 100, and 1,000 cows. Instancing is
therefore `1.43x`, `2.24x`, and `4.74x` faster at those counts on this host.

The stationary 1,000-cow control measures `0.531/0.569ms` average/mean p95,
performs 120,000 exact-input reuse hits with zero pose or actor/palette writes,
and still submits one draw per frame. At full detail, 1,000 cows execute
528,000 vertices and 792,000 indices per frame. The isolated lane is suitable
for route attribution and scaling, not physical Quest acceptance.

Representative command:

```bash
native/target/release/actor_render_perf \
  --actors 1000 --frames 120 --warmup-frames 30 \
  --figure cow --path prepared --motion animated
```

## Memory And Bandwidth Tradeoff

At 1,000 cows, the prior prepared path allocated one 4,096-byte maximum-sized
palette buffer plus one 80-byte actor buffer per actor. The new path allocates
one grow-only actor instance buffer and one grow-only palette texture for the
whole cow bucket:

| Mutable GPU memory | Previous prepared | Instanced prepared |
|---|---:|---:|
| Actor-specific state | `4,176,000 B` | `2,162,688 B` |
| Fixed per-world view state | `1,350,016 B` | `1,350,016 B` |
| Snapshot total | `5,526,016 B` | `3,512,704 B` |

Actor-specific GPU memory falls by about 48%; total known mutable GPU memory
falls by about 36%. The instanced actor allocation is 65,536 bytes for 64,000
logical bytes. The cow palette contains 1,056,000 logical bytes but occupies a
2,097,152-byte power-of-two texture. That unused capacity is deliberate: it
keeps growth infrequent and the steady path allocation-free.

The resource-count reduction is larger than the byte table shows. One thousand
actors previously required roughly 2,000 actor/palette buffers and 2,000 bind
groups. The instanced cow bucket uses one actor buffer, one palette texture,
and one palette bind group. Driver descriptor and object overhead is not
included in the snapshot, so the table understates that benefit.

CPU memory is not yet reported as an exact aggregate. Each actor retains its
evaluated matrices, and each bucket retains reusable actor/palette upload
scratch. For 1,000 cows the dense upload scratch is about 1.12 MB before
allocator headroom. This avoids presentation-rate allocation but should be
included if actor CPU-memory telemetry is later added.

Per animated 1,000-cow frame, the new path performs:

- one palette write of `1,056,768` bytes including final row padding; and
- one actor write of `64,000` bytes.

The previous path performed 1,000 writes of each kind, totaling 1,408,000
palette bytes and 80,000 actor bytes. Affine packing therefore removes 25% of
matrix traffic and 20% of actor-record traffic in addition to collapsing queue
calls.

## Ordered Future Work

### P0: Physical Quest Product Gate

Install the current Android XR build and repeat the exact RD5 composed-orbit
normal-actor/`--xr-skip-actors` A/B that exposed the ten-cow cost. The
actor-skipped row is attribution-only; the normal-actor row must pass the
absolute product frame gates.

If the delta remains ambiguous, carry prepared/legacy counts, instance bucket
count, draws, pose evaluations, reuse hits, write counts/bytes, and actor GPU
timestamps into the Quest receipt. Do not infer a headset win solely from the
desktop offscreen lane.

### P1: Hybrid Sparse And Dense Bucket Uploads

Current invalidation is bucket-granular:

- a fully unchanged bucket performs no pose evaluation or GPU writes;
- an all-animated bucket performs two contiguous writes, which is the intended
  dense fast path; and
- one changed actor in a 1,000-actor bucket still rebuilds and writes the full
  1.12 MB bucket.

Before changing the implementation, extend `actor_render_perf` with a
deterministic sparse-motion control. Measure 100- and 1,000-actor buckets with
1, 4, 16, 64, and all actors changing, plus a mixed-figure control. Report
dirty actors, coalesced dirty ranges, queue writes, bytes, prepare time, and
total frame time.

The candidate hybrid path is:

1. Track changed instance indices while bucket order is stable.
2. Coalesce adjacent dirty actors into actor-buffer and row-safe
   palette-texture ranges.
3. Use partial writes only below a measured dirty-range/byte threshold.
4. Retain the two-write full-bucket path for dense mutation, any order change,
   or resource growth.

Acceptance requires zero writes for unchanged buckets, no dense-crowd
regression outside normal run variance, materially lower bytes and total work
for sparse mutation, allocation-free steady state, and unchanged mixed-figure
pixels. Do not replace two large writes with hundreds of tiny queue calls.

### P2: Capability-Gated GPU Palette Expansion

Exact CPU pose evaluation is now the largest stable animated preparation cost:
about `1.15ms` per 1,000 cows on the baseline host. It also produces roughly
1.06 MB of palette traffic per frame.

A GPU experiment should upload compact actor animation state—clip identity,
phase/time, rate, blends, transform, light, and bounded overrides—then expand
the final affine palette once per actor/part in a compute pass. The draw shader
continues consuming the current palette contract.

Do not evaluate the hierarchy independently in every vertex. At 1,000 cows
that would repeat actor/part work across 528,000 vertices instead of expanding
22,000 actor/part transforms once.

The GPU path trades CPU time and transfer bandwidth for:

- GPU-resident clip/channel data;
- compute dispatch and synchronization;
- palette output storage, potentially slotted for frames in flight; and
- more capability and correctness surface on browser/mobile adapters.

Keep the exact CPU evaluator as the ordinary and fallback path. Promote GPU
expansion only above measured actor/part thresholds where it wins on relevant
desktop, browser, and Quest hardware. Gate it with tolerant CPU/GPU palette
agreement, continuous-phase animation, hierarchy/override coverage,
mono/per-eye/multiview pixels, and real actor-pass GPU timestamps.

### P3: Projected-Size Actor LOD

Instancing and GPU palette expansion do not reduce vertex execution. The
1,000-cow lane still submits 528,000 vertices and 792,000 indices. Generated
figure LOD should be considered when actor-pass GPU time or projected-size
evidence shows geometry is binding.

LOD adds immutable geometry/material variants and may add palette variants, so
it can increase resident memory while reducing per-frame work. Use
projected-size selection, hysteresis, one conservative choice across XR eyes,
stable feet anchoring and animation phase, and bounded residency. Preserve the
accepted box-animal silhouette; the 1,000-actor stress case is not permission
to weaken ordinary presentation quality.

### P4: CPU-Baked Fallback Topology Reuse

Player-specific legacy routes, debug/items, anonymous actors, and unsupported
figures still rebuild one combined mesh when any input changes. Split topology
and pose/index invalidation only after counters show that fallback actors
remain material in a real workload. This work must not complicate or gate the
prepared path.

## Required Diagnostics And Gates

Retain or add:

- prepared and legacy actor counts;
- instance buckets, real draws, drawn instances, and maximum instances/draw;
- pose evaluations, exact-input reuses, and dirty actor/range counts;
- actor/palette write counts and logical/actual bytes;
- CPU evaluation, upload, encode, submit, and device-poll time;
- actor-pass GPU timestamps where supported;
- immutable and mutable GPU bytes plus reusable CPU scratch where practical;
- visible vertices/indices by figure and LOD; and
- physical Quest app-work/headroom evidence for product conclusions.

Validation for any renderer change includes native mono, per-eye stereo,
full-frame multiview, production browser WebGPU, and Android XR packaging.
Rendered-output changes require inspected pixels at the first drawable
milestone. Use `actor_render_perf` for isolated attribution and the physical
Quest composed orbit for headset acceptance.

## Decision Ledger

- **2026-07-28, `41fc9fae`:** removed the stale player/chicken whitelist so
  prepared-resource capability admits stable cows and upright bears.
- **2026-07-28, `32fcffaf`:** implemented per-figure instancing, batched affine
  palette textures, 64-byte actor records, and precomputed evaluation channels.
- **2026-07-28, `99ec3808`:** recorded clean scaling and selected physical
  Quest, sparse updates, GPU palette expansion, and actor LOD as distinct
  follow-ups.
