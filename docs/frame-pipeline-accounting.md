# Frame Pipeline Accounting

Status: durable overview and measurement rulebook. This document ties together
frame pacing, terrain streaming, local integrated server work, remote-host
play, render-section admission, GPU upload, and Quest/OpenXR headroom. It is
not an implementation tactical; use it to decide what a benchmark or UI/debug
overlay must measure before changing pacing policy.

The executable checklist for the instrumentation build-out — per-slice
deliverables, non-goals, exit criteria, validation blocks, and the
implementing-agent contract — is tactical
[`tactical/144-frame-pipeline-accounting-instrumentation.md`](tactical/144-frame-pipeline-accounting-instrumentation.md).
Implementation sessions should work from that tactical; this document stays
the durable rulebook. If they ever disagree, stop and reconcile the documents
before writing more code.

Primary related docs:

- [`architecture.md`](architecture.md): runtime layers and clock boundaries.
- [`authoritative-host-scheduling.md`](authoritative-host-scheduling.md): host
  authority and chunk-job scheduling priorities.
- [`worker-ownership.md`](worker-ownership.md): worker/cache ownership and
  cross-thread invariants.
- [`topics/performance.md`](topics/performance.md): current performance priority
  map and measured baselines.
- [`performance-records.md`](performance-records.md): desktop/native benchmark
  records and lane definitions.
- [`quest-standalone-performance-records.md`](quest-standalone-performance-records.md):
  Quest/OpenXR measured rows.
- [`tactical/117-android-xr-rd10-gpu-floor-and-frame-overlap.md`](tactical/117-android-xr-rd10-gpu-floor-and-frame-overlap.md):
  CPU/GPU overlap and Quest GPU-floor work.
- [`tactical/119-android-xr-live-streaming-frame-pacing.md`](tactical/119-android-xr-live-streaming-frame-pacing.md):
  Quest live streaming frame-pacing checklist.
- [`tactical/128-terrain-render-pipeline-coordination.md`](tactical/128-terrain-render-pipeline-coordination.md):
  dirty-to-drawable terrain pipeline coordination.
- [`tactical/130-quest-thread-scheduling-and-streaming-tail-attribution.md`](tactical/130-quest-thread-scheduling-and-streaming-tail-attribution.md)
  and [`tactical/131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md`](tactical/131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md):
  Quest thread scheduling, blocked-vs-CPU attribution, and GPU sync findings.
- [`tactical/142-throughput-policy-with-quest-rd5-guardrail.md`](tactical/142-throughput-policy-with-quest-rd5-guardrail.md):
  current desktop-throughput / Quest-guardrail strategy.
- [`tactical/144-frame-pipeline-accounting-instrumentation.md`](tactical/144-frame-pipeline-accounting-instrumentation.md):
  executable instrumentation checklist for this document's measurement gaps.

## Purpose

Chunk streaming performance is not one bottleneck. The player sees one frame
stream, but the work spans an authoritative host, worldgen/light workers,
publication queues, client update application, render-section compile workers,
GPU upload, draw encoding, GPU execution, and platform presentation pacing.

The useful question is not "how many chunks per second can the machine do?" in
isolation. The useful question is:

```text
For this platform, host mode, render distance, and refresh rate, which stages are
on the current-frame critical path, which stages can run concurrently, which
queues are aging, and how much measured headroom is left after render/present?
```

This document defines the accounting shape we need for that answer.

## Core Model

Treat each displayed frame as a budgeted coordination point. Some work must
finish before the current frame can be submitted. Some work can run in parallel
on other threads. Some work can be prepared after submission for a later frame.
Some work is paid by the local device only in local integrated play.

The target shape is:

```text
current frame pose/input
  -> minimal current-frame update/apply needed for correctness
  -> encode and submit current frame early
  -> GPU/runtime/compositor work proceeds
  -> CPU spends bounded slack on pose-independent next-frame streaming work
  -> next frame consumes completed, validated, budgeted results
```

Terrain data may be one or more frames late. Head/camera pose, input sampling,
and presentation timing should stay as fresh as the platform allows.

## Stage Ownership

| Stage | Local integrated play | Remote/dedicated play | Current-frame risk | Primary counters |
|---|---|---|---|---|
| Input, controller pose, window/OpenXR events | client device | client device | must stay fresh; should not wait for bulk work | input/poll time, pose commit time, command send time |
| Host/session commands | same device, host lane/thread | remote host or local transport endpoint | local integrated can contend with render CPU | command queue age, host pump time, command drain/apply time |
| Terrain/features generation | same device, worldgen worker | server pays | CPU peer pressure in local mode; not client-frame work in remote mode | queued/running jobs, chunks/sec, worker busy/idle time |
| Light compute/status | same device, light/status lane | server pays | CPU peer pressure and publication dependency in local mode | light status queued/running/done, light compute time |
| Scheduler publication / snapshots / save enqueue | same device, host scheduler | server pays | can throttle throughput; can spike local host tick | publish counts, publish elapsed, pending publication, save queue |
| Transport/network/decode | local queue or loopback | client receives remote updates | receive/decode/apply can age or burst | inbound bytes, queue depth, oldest update age, decode time |
| Client update apply / dirty marking | client device | client device | can land on render/pose path if not paced | updates applied, dirty sections/chunks, apply elapsed |
| Render-section admission | client device | client device | should be deadline/headroom gated | pending dirty, submitted sections, deadline skips |
| CPU mesh compile | render compile workers | render compile workers | parallel, but workers can contend for CPU | worker slots, pending jobs, completed results, worker CPU time |
| Completed-result acceptance | client render/runtime path | client render/runtime path | can burst many sections after workers finish | accepted results, queued results, accepted sections, elapsed |
| GPU upload / draw-resource mutation | render thread / GPU queue | render thread / GPU queue | current-frame critical unless post-submit/pipelined | uploaded/removed sections, bytes, buffer creation/write time |
| Prepared draw records / traversal-ready state | render path | render path | can turn small accepts into broad CPU work | ready-set changes, record rebuilds, drawn sections |
| Draw encode | render thread | render thread | current-frame critical | per-eye/flat encode time, cull time, draw calls/indices |
| GPU execution / presentation wait | GPU/runtime/compositor | GPU/runtime/compositor | serial wait if CPU blocks on completion | GPU timestamps, poll wait, app work, headroom |

The local integrated and remote columns are not interchangeable. Backpressure
can decouple server production from client render work, but it does not make
server CPU free on Quest local integrated play. Remote play removes most
generation/light/publication CPU from the headset, then shifts attention to
network/decode/apply, mesh/upload, and render headroom.

## Critical Path Rules

Use these labels when adding measurements:

| Label | Meaning | Examples |
|---|---|---|
| current-frame critical | delays submitting the frame being displayed now | pose update, draw encode, pre-submit upload, blocking GPU wait |
| next-frame slack | can run after current submit and feed later frames | runtime prefetch, completed-result acceptance, dirty admission |
| parallel CPU peer | runs off the render thread but can steal cores/cache/thermal headroom | local worldgen, local light, render compile workers |
| remote host work | does not consume client CPU in remote play | terrain/features, light, server publication, persistence |
| queued/backpressured | not currently on the frame, but queue age can become visible | snapshots, completed meshes, uploads, inbound updates |

Optimizations should state which label they move work between. A change that
moves work off the render thread but creates a local integrated CPU peer can
still hurt Quest. A change that improves a synthetic full-drain benchmark can be
irrelevant if the desktop or headset frame loop cannot admit the results safely.

## Platform Timelines

### Desktop Flat

Desktop usually has more slack and less severe comfort risk. Coarse budgets often
show up as slow world fill rather than dropped frames.

Current desktop native window shape is mostly:

```text
event/redraw
  -> runtime poll and section sync/upload
  -> UI/frame setup
  -> render encode
  -> queue submit / present
  -> wait or schedule next redraw
```

The desired throughput shape is to submit earlier when practical and spend
bounded post-submit or next-frame slack on terrain streaming. Even without that
full shape, desktop should use measured elapsed budgets rather than fixed tiny
counts when the frame loop has clear headroom.

### Quest / Android XR

Quest has a smaller frame period and tighter thermal/headroom constraints. At
72 Hz the period is `13.889 ms`, and usable CPU slack may be much smaller than
that after pose, draw encode, GPU execution, runtime overhead, and compositor
timing are accounted for.

The useful signals are `app_work_*`, `headroom_*`, and `app_over_period_*`, not
legacy paced frame wall time. A controller that is harmless on desktop can be
unsafe on Quest if it turns a zero-or-one-unit decision into a current-frame
burst.

The opt-in XR frame-overlap path already demonstrates part of the target shape:
submit eye work, defer some wait, prefetch pose-independent runtime/render work,
then consume it on a later frame. That path is not yet the default policy and is
not a substitute for an adaptive budget controller.

### Headless / Benchmark Lanes

Headless lanes are valuable because they are repeatable, but they each measure a
specific shape:

- raw worldgen lanes measure algorithmic ceiling without scheduler/light/client.
- scheduler-loading lanes measure server loading/publication without client mesh
  or frame pacing.
- mesh CPU lanes measure snapshot-to-mesh work without `wgpu` or admission
  pacing.
- GPU-upload lanes measure prebuilt mesh upload without draw/frame pacing.
- startup-streaming lanes are closest to desktop-shaped local play.
- Quest/OpenXR lanes are the authority for headset frame pacing.

Do not optimize for any single isolation lane without rerunning the desktop-shaped
and Quest guardrail lanes that correspond to the changed stage.

## Measurement Requirements

A useful frame or streaming report should answer five questions:

1. What work ran on the render/main thread this frame?
2. What work ran on peer CPU threads during the same window?
3. What queues got older, and what queues drained?
4. Did the frame miss its platform headroom target?
5. Did the player-visible readiness state change?

Minimum stage accounting:

| Area | Required measurements |
|---|---|
| Frame pacing | target Hz, frame period, app work, wait time, headroom, over-period, over-2x/4x, dropped/stale frames where available |
| Render thread | input/pose, runtime poll, client update apply, render sync/admission, accept, upload, ready-record maintenance, cull, encode, submit/present/poll wait |
| GPU | timestamped terrain/eye pass where available, blocking poll wait, upload bytes, buffer creation/write time |
| Host scheduler | tick/poll time, publication elapsed, feature/light publish counts, pending publication, pending unloads, save enqueue time |
| Worldgen/light | queued/running/completed jobs, worker busy/idle time, per-status compute time, dependency/cache hit rates |
| Transport/update | inbound queue depth, bytes, oldest update age, updates applied per frame, decode/apply time |
| Render compile | dirty/deferred/actionable sections, submitted sections, worker slots, pending jobs, queued/completed/stale results, acceptance budget hits |
| UI/debug | current readiness gate, playable/full-view/render-idle markers, phase color/status map for chunk grid |

Wall time alone is not enough on Quest. For worst frames, record thread CPU time
and blocked/wait time where possible. A frame can be expensive because the render
thread was busy, because it was runnable but not scheduled, or because it blocked
on GPU/runtime synchronization. Those are different fixes.

## Measurement Architecture And Ownership

### One Shared Owner: `mclone-diagnostics`

Accounting math and the report vocabulary get exactly one shared owner: a
small leaf crate, working name `mclone-diagnostics`, with no engine
dependencies. Producers span the whole stack — `mclone-server` tick and
publication timing, `mclone-app-runtime` streaming stats, `mclone-render`
pass timing, `mclone-xr-scene` frame timing, app frame loops, and benchmark
binaries — so the owner must sit below all of them. `mclone-app-runtime` is
too high in the dependency graph to serve `mclone-server`, which is why this
noun gets a dedicated crate while the client-experience core stayed an
`mclone-app-runtime` module family.

As of July 2026 this machinery exists as three disconnected app-local copies:

- Quest/Android XR: `AndroidXrActivePerfProbe`
  (`native/apps/mclone-android-xr-client/src/lib.rs`) is the most mature —
  `app_work_*` as frame wall minus `xrWaitFrame`, headroom percentiles,
  over-period/2x/4x tiers, worst-frame capture, and the
  `CLOCK_THREAD_CPUTIME_ID` busy-vs-blocked split (`thread_cpu_time_ms`).
- Desktop flat runtime: `FrameTimingStats`
  (`native/apps/mclone-native-client/src/frame_pacing.rs`) — budget,
  over-budget tiers, worst frame; no percentiles.
- Desktop benchmarks: a third percentile/over-budget implementation in
  `native/apps/mclone-native-client/src/perf.rs`
  (`frame_budget_percentile_ms`).

These collapse into one implementation. The closest existing shared stats
types (`RenderStreamStats` and `FullFrameRenderTiming` in
`mclone-app-runtime/src/frame_render.rs`, and the report structs in
`mclone-server/src/timing.rs`) keep their producers and adopt the shared
stage vocabulary and report schema rather than moving wholesale.

### Sans-I/O Accounting Core

The accounting core follows the same execution rule as the client-experience
core in
[`client-experience-architecture.md`](client-experience-architecture.md): it
never reads clocks or entropy, never blocks, and never spawns. Timestamps and
thread-CPU samples enter as adapter-provided inputs; the crate's `clock`
module owns the platform samplers (monotonic now, per-thread CPU time via
`CLOCK_THREAD_CPUTIME_ID` on Linux/Android/macOS-unix paths and the Windows
equivalent) as the one adapter-shaped exception, and the core compiles for
`wasm32-unknown-unknown`. This is what makes the math trustworthy: synthetic
frame histories in, exact percentiles/headroom/tier decisions out,
deterministically, in CI, with no device attached.

### One Schema, Three Sinks

Reports are serde structs with a schema version, defined next to the core.
The `MCLONE_*` summary log lines, the benchmark JSON envelopes, and the debug
overlay all render the same structs. A statistic that exists only in one
sink's formatting code is a defect. The logs remain the benchmark authority
and the overlay stays a thin view because all three are projections of the
same data, not because of discipline alone.

As of Slice 7 of tactical 144, schema v5 carries the shared frame summary,
queue panel, local peer-thread panel, GPU timestamp panel, and latest-frame
fields consumed by the debug overlay. Desktop startup-streaming JSON,
Quest/Android XR perf markers, accounting-smoke JSON, and the desktop flat
overlay use the same report structs and render-admission/upload stage names for
completed-result acceptance, dirty/ready scan, request build, worker submit,
prepared-record maintenance, admission remainder, and upload apply.

### Per-Lane Time-Source Authority

| Lane | Frame budget source | CPU attribution | GPU time authority |
|---|---|---|---|
| Desktop flat | monitor refresh / FPS cap (exists) | `Instant` spans + thread-CPU clock (extend to macOS/Windows) | wgpu timestamp queries (new) |
| Quest / Android XR | `XR_FB_display_refresh_rate` (exists) | `xrWaitFrame` subtraction + `CLOCK_THREAD_CPUTIME_ID` (exists) | `XR_META_performance_metrics` (exists; promote from one-shot probe) |
| Headless / benchmarks | `target_hz` lane config (exists) | same as desktop | wgpu timestamp queries (new) |
| Web / WASM | rAF-derived | `performance.now` wall only | unsupported capability for now |

### GPU Time Is Currently Unmeasured

`optional_gpu_features` (`native/crates/mclone-render/src/gpu_util.rs`)
requests `wgpu::Features::TIMESTAMP_QUERY` when the adapter offers it, and
nothing consumes it: there are no query sets, `write_timestamp`, or
`timestamp_writes` sites outside vendored `wgpu-hal`. Every current "GPU"
number — `device_poll_ms`, submit/poll waits, present time — is a CPU-side
blocking measurement, which tactical 131 already flags as synchronization
stalls rather than GPU cost. The only true GPU-time source in the tree is the
Quest `XR_META_performance_metrics` probe
(`native/apps/mclone-android-xr-client/src/perf_metrics.rs`), opt-in and
one-shot.

Target shape:

- desktop-first wgpu timestamp layer: pass-boundary `timestamp_writes`
  (whole-pass, not per-draw), a query/resolve pool sized to frames in flight,
  asynchronous readback only — results are allowed to be one or two frames
  late, which the accounting model already tolerates. "Desktop" is two
  backends in practice — Metal on the macOS host and Vulkan/DX12 on the
  Windows host — and the layer is not done until both are validated, since
  timestamp support and granularity differ per backend;
- Quest keeps `XR_META_performance_metrics` as the GPU authority (tiled-GPU
  timestamp semantics and compositor ownership make runtime counters the
  truth there); wgpu timestamps on Quest are validation-only if enabled at
  all;
- absence of `TIMESTAMP_QUERY` degrades to a capability fact, never a panic
  or a silently zeroed column;
- per the XR render-path guardrail, pass timing covers both the per-eye and
  full-frame multiview paths, and per-frame query/resolve resources are never
  shared across frames in flight.

### Clock Domains

There are three time domains: CPU monotonic (`Instant`), GPU device ticks
(converted via `Queue::get_timestamp_period()`), and OpenXR `XrTime`.
Durations are computed within one domain and reported per domain; never
subtract timestamps across domains. Cross-domain alignment (a single unified
waterfall timeline) is explicitly out of scope until a measurement need
demands it — the frame report aligns stages structurally, by frame index and
stage label, not by a shared epoch.

### Measurement Overhead Policy

The always-on accounting set is measured like any other frame cost: the Quest
RD5 guardrail lane runs with accounting on and off, and the delta is
recorded. Provisional ceiling: 0.2 ms added app-work p95 on Quest RD5;
anything above moves behind an opt-in flag. Expensive sources follow the
existing perf-metrics probe pattern — warm up, sample a window, disable —
rather than running continuously.

Slice 4 adopted the provisional CPU calibration tolerance and overhead
ceiling unchanged on 2026-07-05: CPU busy-spin attribution must land within
`max(10%, 0.3 ms)`, and the always-on set must stay within `<= 0.2 ms`
added app-work p95 on the Quest RD5 guardrail lane. The recorded RD5 A/B on
`kmacbook` with Quest 3 measured `15.706 ms` app-work p95 with accounting on
versus `15.777 ms` with accounting off, with zero conservation violations, so
no source was demoted to opt-in.

## Validating The Instrumentation

Wrong measurements are worse than missing ones: they redirect optimization
work with false confidence. Every instrumentation slice ships with its trust
mechanism:

1. **Deterministic unit tests.** The sans-I/O core is exercised with
   synthetic frame timelines; percentiles, headroom, over-period tiers,
   worst-frame capture, and queue ages are asserted exactly. Cheapest layer;
   covers all of the math.
2. **Conservation invariants.** Debug builds assert: per-stage spans sum to
   no more than frame wall time; `app_work + wait` ≈ frame period; thread-CPU
   time ≤ wall time per span; `enqueued - dequeued` equals queue depth
   change; timestamps are monotonic. Release builds count violations into the
   report instead of asserting. Violations are instrumentation bugs to fix,
   never assertions to loosen without a recorded reason — double counting and
   missed span exits are the classic failure modes these catch.
3. **Calibration lanes.** A headless lane injects a busy-spin of known K ms
   into a chosen stage and asserts the report attributes K within
   `max(10%, 0.3 ms)` to that stage under the correct critical-path label. A GPU
   calibration draws N known fullscreen quads and asserts GPU pass time
   scales with N and stays within wall-clock bounds. These run in the
   standard smoke set.
4. **Cross-source agreement at bring-up.** On Quest, `XR_META` app GPU
   frametime is compared against wgpu timestamp totals (if enabled) and the
   frame wall breakdown, with agreement bounds recorded in
   [`quest-standalone-performance-records.md`](quest-standalone-performance-records.md).
   On desktop, wgpu timestamps get a one-time spot-check against an external
   GPU capture (Metal/Xcode on macOS, PIX or RenderDoc on Windows), recorded
   in [`performance-records.md`](performance-records.md). One-time bring-up
   evidence, not CI.
5. **Measure the meter.** The overhead A/B from the policy above, re-run
   whenever the always-on set grows.

## Debug UI And Logging Target

The debug UI should eventually show the same pipeline model that the logs record:

- frame budget bar: app work, wait, headroom, over-period status;
- stacked current-frame waterfall: input/pose, update apply, terrain runtime,
  upload, records, cull/encode, GPU/poll wait;
- queue panel: host publication, inbound updates, dirty sections, compile jobs,
  completed results, upload queue, oldest age;
- local integrated peer panel: worldgen/light/server runner thread activity and
  backlog;
- readiness panel: playable gate, target chunks ready, render actionable idle,
  dirty-but-not-actionable counts;
- chunk status grid: generation, light, published, client-applied, mesh-pending,
  uploaded/drawable, using distinct colors for phases.

The logs should remain the authority for benchmark comparison. The UI is for
interactive diagnosis and should be a thin view over the same counters.

## Budget Controller Direction

The durable controller should calculate budget from measured conditions, not from
a profile-specific fixed count.

Inputs:

- target frame period and platform lane;
- recent app work and headroom percentiles;
- recent missed/dropped/stale frames;
- queue age and backlog pressure;
- recent elapsed cost per phase;
- local integrated vs remote host mode;
- thermal/performance drift where available;
- minimum work needed to prevent starvation.

Behavior:

- reduce budget quickly after misses or negative headroom;
- increase budget slowly after sustained headroom;
- spend budget on smaller units than "one chunk" where possible: accepted
  results, sections, bytes, updates, publications, or elapsed microseconds;
- keep current-frame critical work conservative on Quest;
- allow desktop to ramp higher when frame headroom is consistently large;
- preserve vanilla/reference correctness gates: status order, light readiness,
  neighbor readiness, stale-result rejection, and atomic drawable publication.

This is compatible with Java's reference shape: deadline-driven render compile
admission, explicit dispatcher/upload queues, bounded resources, and old drawable
geometry retained until replacement output is ready.

## Current Known State

As of the July 2026 throughput passes:

- desktop fresh startup-streaming was throttled heavily by scheduler publication
  policy, not raw worldgen hardware ceiling;
- raising fixed publication counts proved the valve, but high constants are not
  the target policy;
- desktop persisted startup enters and reaches full target view quickly, while
  stable render actionable idle still takes many seconds under current render
  admission/mesh progression;
- desktop GPU upload of prebuilt RD10 meshes is much smaller than CPU mesh and
  admission tail, though still too large to dump into one Quest frame;
- Quest RD5 is the low-distance safety guardrail, RD7 is the pressure lane, and
  RD10 remains a stress lane;
- Quest local integrated evidence shows server/worldgen/light threads are real
  CPU peers, so client-only frame charts can miss contention;
- XR frame overlap is a measured opt-in win, but not a finished adaptive policy.

For exact numbers, use [`topics/performance.md`](topics/performance.md) and
[`performance-records.md`](performance-records.md), not this overview.

## Actionable Gaps

The next useful work is mostly measurement quality before broad policy
changes. Tactical
[`tactical/144-frame-pipeline-accounting-instrumentation.md`](tactical/144-frame-pipeline-accounting-instrumentation.md)
is the executable checklist for gaps 1-8, including sequencing relative to
tactical 143; gaps 9-10 are follow-on work:
[`tactical/149-remote-contrast-accounting-honesty.md`](tactical/149-remote-contrast-accounting-honesty.md)
(gap 9) and
[`tactical/150-adaptive-frame-budget-controller.md`](tactical/150-adaptive-frame-budget-controller.md)
(gap 10).

As of 2026-07-06, gaps 1-8 are landed in tactical 144. Gap 6 closed after Mac
Metal plus Windows Vulkan/DX12 timestamp validation, Windows RenderDoc capture,
and Quest periodic `XR_META_performance_metrics` sampling; Quest-side wgpu
timestamp agreement is not applicable until Android XR emits a wgpu timestamp
panel. Gap 8 closed with the desktop flat/offscreen debug overlay routed
through the shared client-experience facade.

1. Extract the shared accounting owner (`mclone-diagnostics`): one
   budget/percentile/over-period/headroom implementation and one versioned
   report schema, adopted by the desktop runtime, the Quest probe, and the
   benchmark reports that currently hold private copies.
2. Make desktop startup-streaming and Quest streaming reports emit the same
   stage vocabulary where possible.
3. Add or harden per-frame queue-age counters for inbound updates, completed
   render results, upload work, and host publication.
4. Split render admission/mesh tail into elapsed admission, dirty/ready scan,
   request build, worker submit, result acceptance, upload apply, and prepared
   record maintenance.
5. Add local integrated peer-thread activity to headset summaries: server
   runner, worldgen, light/status, render compile workers.
6. Build the GPU timing layer: desktop-first wgpu pass timestamps, promote the
   Quest perf-metrics probe to a periodic sampler, record cross-source
   agreement.
7. Land the instrumentation-validation harness: conservation invariants,
   CPU/GPU calibration lanes, and the meter-overhead A/B with a recorded
   ceiling.
8. Landed: add a UI/debug overlay view that mirrors the logging counters
   instead of inventing separate presentation-only state, routed through the
   shared client-experience facade.
9. Keep remote/dedicated contrast lanes honest by surfacing network/decode/apply
   counters and server-side scheduler counters separately. Follow-on tactical:
   [`tactical/149-remote-contrast-accounting-honesty.md`](tactical/149-remote-contrast-accounting-honesty.md).
10. Only then open policy levers through elapsed/headroom-aware budget
    calculation, not a large fixed publish or upload count. That successor
    tactical builds on
    [`tactical/142-throughput-policy-with-quest-rd5-guardrail.md`](tactical/142-throughput-policy-with-quest-rd5-guardrail.md)
    and the measured data this work produces. Follow-on tactical:
    [`tactical/150-adaptive-frame-budget-controller.md`](tactical/150-adaptive-frame-budget-controller.md).

## Update Policy

Update this document when the pipeline shape changes, when a stage moves between
critical path and slack/parallel work, or when a benchmark lane starts measuring
a new part of the accounting model. Put detailed experiment rows in
[`performance-records.md`](performance-records.md) or the relevant tactical; keep
this file focused on the model and measurement requirements.
