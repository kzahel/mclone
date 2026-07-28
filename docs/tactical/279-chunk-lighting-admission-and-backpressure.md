# Tactical 279: Chunk Lighting Admission And Backpressure

Status: implementation complete and physical memory/convergence accepted
2026-07-28. The inherited Quest settled-orbit presentation-tail gate remains
open outside the scheduling owner and is not waived here.

Topics: `chunk-lighting-admission-and-backpressure`, `lighting`, `performance`

## Originating Request

Sustained high-altitude 8x travel on Quest made the procedural horizon
compelling, but exact terrain stopped becoming ready after returning to ground
level and Horizon OS eventually killed the app near its 6GB process limit.
Bounded persistence work removed one known unbounded owner and passed shorter
RD5 travel, but an extended RD7 soak reproduced the failure.

Investigate the shared concurrency model, compare it with Minecraft Java
1.17.1, record the durable direction, and implement a vanilla-shaped
player-ticket/status/light lifecycle with stronger explicit memory bounds for
Quest and other constrained runtimes.

The living evidence, analysis, and accepted contract are in
[`chunk-lighting-admission-and-backpressure.md`](../topics/chunk-lighting-admission-and-backpressure.md).

## Objective

Make exact-world work depend on a bounded, current-interest-prioritized
promotion pipeline:

```text
desired player positions
    -> compact keyed priority queue
    -> bounded active promotions
    -> Features
    -> compact keyed Light demand
    -> bounded shared-input Light work
    -> token-checked client-ready publication
```

Continuous travel must keep all transient ownership proportional to the active
working set, not the total distance travelled. Overload may delay or skip outer
exact chunks, but it must prioritize the current center, recover after movement
stops, and never retain obsolete copied neighborhoods until the platform kills
the process.

## Execution Record

### Landed series

The implementation was committed in reviewable slices under
`Topic: chunk-lighting-admission-and-backpressure`:

| Commit | Result |
|---|---|
| `ab7c8b21` | bound cold Player promotion to four and add Light request identity/tickets |
| `12a70f66` | share overlapping raw Light batch inputs |
| `f5ce7ac3` | add compact keyed Light demand, cancellation, and total mailbox ownership bounds |
| `ef8b5acb` | prune full completed jobs and retain a 64-entry summary ring |
| `cf07c84c` | preserve shared input ownership during worker materialization |
| `5f20601b` | update ticket distances incrementally and recover stationary throughput |
| `a56fa74d`, `1b69b23a`, `10ee9156` | expose and require complete Quest ownership evidence without log truncation |
| `7c1c08eb` | bound adjacent native-client deferred chunk payload ownership |

The mailbox defaults are the planned 18-status and 64 MiB limits. Capacity is
charged from admission through terminal drain. Player promotion defaults to
four active cold positions. Completed full jobs are removed once no live owner
references them; the diagnostic ring is fixed at 64.

Physical churn acceptance found one additional end-to-end owner after the
authority queues were bounded. The native client accepted only 16 deferred
unload records per poll but could receive far larger snapshots, retaining
millions of payloads and causing another 6.78 GB low-memory kill. Commit
`7c1c08eb` caps staging at 4,096 items, raises the normal handoff budget to the
same value, and applies overflow inline. Focused 64-snapshot burst tests prove
the cap and drain behavior.

### Correctness and platform closure

The final source passes:

- all 583 `mclone-server` tests;
- all 284 `mclone-app-runtime` tests;
- all 139 `mclone-client` tests;
- all 163 `mclone-scene` tests;
- native-client all-target and full workspace checks;
- the `wasm32-unknown-unknown` web-client check, browser build, and TypeScript
  typecheck;
- flat Android release APK;
- Android XR release APK; and
- focused promotion, demand, token, ticket, shared-input, mailbox-capacity,
  cancellation/unload, history-pruning, and client-staging stress tests.

No packed-light, mesh, shader, or rendered-pixel behavior changed, so this
slice did not require a new screenshot.

### Desktop throughput gates

The adjacent parent source was `01a221b8`; the final server-performance source
was `5f20601b`. Later commits only add diagnostics or bound downstream client
staging.

| Lane | Parent | Candidate | Disposition |
|---|---:|---:|---|
| RD10 scheduler first-view-ready | `8152.284ms` | `7511.308ms` | 7.9% faster |
| RD10 scheduler settled | `9078.914ms` | `8462.837ms` | 6.8% faster |
| RD10 view-ready throughput | `64.890/s` | `70.427/s` | 8.5% faster |
| RD10 first full view | `7688.148ms` | `8303.843 / 8325.184ms` | 8.0-8.3% slower; within 10% |
| RD10 render quiescent | `50424.710ms` | `47759.096 / 47948.029ms` | faster |
| RD10 frame-work p95 | `6.493ms` | `6.843 / 6.650ms` | +5.4% / +2.4%; within 10% |
| movement over-budget frames | `1` | `1` | unchanged |

The candidate RD15 promotion row reached all 1,089 chunks, first full view in
`17.348s`, and render quiescence in `52.926s`; all authority queues drained,
full completed-job retention ended at zero, and the recent ring stopped at 64.
A real 1280x900 Wayland/Vulkan/FIFO native window presented successfully.

A non-XR 200-center moving-authority lane completed in `20.774s`. RSS rose
from about 44.9 MiB to 62.9 MiB and plateaued, loaded snapshots settled at 85,
completed full jobs ended at zero, and recent summaries stayed fixed at 64.
This is the required proof that the correction is shared rather than XR-only.

### Quest 3 acceptance

The final Android XR APK contains code through `10ee9156`:

```text
SHA-256
1f68981866a1dda9b21d8a86c06630d0b939417e46d8ec42abbb40de203f7f48
```

It was installed and tested on Quest 3 `2G0YC1ZF93041Z`, API 34,
`arm64-v8a`.

The parent RD5 churn lane was killed at a 6,719,976 KiB footprint. An
intermediate candidate exposed the adjacent deferred-client owner and was
killed at 6,779,612 KiB. With both owners fixed, the exact final RD5 churn
rerun completed and passed every numeric guardrail:

| Metric | Final RD5 churn |
|---|---:|
| skipped / dropped | `0 / 16` |
| app-work p95 / p99 / max | `12.594 / 14.229 / 22.744ms` |
| average headroom | `7.528ms` |
| over-period | `1.1%` |
| promotion active / cancelled before admission | `4 / 96,148` |
| Light admitted / ownership high-water | `18 / 11,106,893 bytes` |
| full completed jobs retained | maximum `1`, final `0` |
| deferred client backlog / handoff | `2,043 / 4,096` |

The exact final RD7 flight row also passes: zero skipped, 15 dropped,
`8.499ms` app-work p95, `13.009ms` p99, `16.003ms` maximum, and `0.1%`
over-period.

The full fresh persisted-world reproduction then flew for `1,200.009s` at
34.4 blocks/second, travelled `41,277.132` blocks, and exited normally:

| Metric | Twenty-minute result |
|---|---:|
| Features / Light | `31,752 / 31,746` |
| Features / Light per second | `26.460 / 26.455` |
| skipped frames | `0` |
| app-work p95 / p99 / maximum | `8.021 / 8.868 / 19.947ms` |
| app-work over-period | `5` frames, `0.0%` |
| active promotions | maximum `4` |
| promotions cancelled before admission | `18,058` |
| Light mailbox | maximum `18` statuses / `10,690,118` bytes |
| full completed jobs | maximum `3`, final `0` |
| recent summaries | maximum/final `64` |
| retained Light chunks | maximum `693`, final `297` |
| deferred client backlog | maximum `549`, final `0` after stop |

Independent process RSS cycled rather than growing with distance: it started
near 510 MiB, spent most samples around 700-900 MiB, briefly reached
1,299,424 KiB, reclaimed to about 936 MiB at the next sample, and ended near
1.04 GiB. Late `dumpsys meminfo` total RSS was about 1.51 GiB including an
approximately 459 MiB fixed GL allocation. Available system memory remained
about 3.1-3.4 GiB, thermal status stayed zero, and swap remained negligible.

After the flight, a stopped-view run at chunk `(3,-2587)` reached all 225 RD7
chunks / 3,600 sections in `17.772s`. A subsequent 60-second sample had zero
Features or Light publications and ended with every promotion, scheduler,
mailbox, persistence, and client-deferred queue at zero.

The pulled database passes `PRAGMA integrity_check`, schema version 2, with
31,960 chunk records across `x=-6..13`, `z=-2592..-4`, 42 entity-chunk
records, and 1,573,417,146 bytes of chunk payload. This proves the full flight
corridor persisted cleanly.

### Remaining presentation-tail disposition

The scheduler and memory acceptance is complete, but the inherited settled
orbit absolute frame gate is not silently waived:

- the adjacent parent RD5 orbit already missed its p95 limit at `13.154ms`
  versus `13.0ms`;
- the parent RD7 orbit measured `14.206ms` p95 and `6.3%` over-period;
- the final cooled RD5 repeat passed every other limit but measured
  `13.201ms` p95 versus `13.0ms`, with four dropped frames, `1.4%`
  over-period, and a `20.991ms` maximum; and
- the final cooled RD7 repeat passed p95, dropped, and over-period limits at
  `13.131ms`, three frames, and `1.6%`, but one `33.763ms` app-work outlier
  exceeded twice the display period.

The passing churn, ordinary RD7 flight, and 20-minute 8x flight rows show no
scheduler-attributable frame regression. The remaining settled-orbit tail is a
pre-existing presentation/renderer release blocker and belongs with the Quest
renderer baseline and Tactical
[`278`](278-quest-procedural-horizon-multiview.md), not with another expansion
of Light ownership.

Post-closeout attribution strengthened that disposition. The original normal
RD5 comparison rendered nine actors on the parent and ten on the candidate.
With actors skipped and all other exact-only RD5 orbit settings matched, the
parent/final app-work average was `4.918/4.841ms`, app-work p95 was
`8.127/8.005ms`, thread-CPU average was `3.275/3.271ms`, and thread-CPU p95
was `4.888/4.887ms`. Stable foreground cost is unchanged.

That control also exposed a separate setup/throughput concern: parent/final
settle time was `19.627/33.186s` and Light requests were `37/70`. A forced
parent batch-size-four diagnostic produced 76 requests. Future Light batching
work should reduce request fragmentation without widening the accepted
promotion or mailbox lifetime bounds.

An explicit `mclone-overworld-v1` LOD-on orbit reported the horizon active and
target-ready throughout. With actors skipped it measured `10.723ms` average,
`12.889ms` p95, and `0.2%` over-period. With ten normal actors it measured
`12.252ms`, `15.151ms`, and `16.3%`. The latter is the binding product result;
the former is attribution evidence only. This localizes the current
presentation failure to the combined horizon/actor workload rather than the
bounded Light scheduler.

## Reference Source

Read and preserve the behavioral shape of:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueue.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`

Reference findings that constrain the implementation:

1. Player-ticket admission has four acquired positions.
2. Queued ticket work is keyed by chunk, priority-level sorted, re-sortable,
   and clearable before admission.
3. `ChunkHolder` status futures prevent unnecessary downstream scheduling after
   demotion.
4. LIGHT adds a temporary light ticket and releases it on the post-update path.
5. The light task operates on shared `ChunkAccess`, not copied 3x3 raw worlds.
6. `ThreadedLevelLightEngine` processes five active tasks per batch.
7. Vanilla does not provide a hard total light byte bound and does not preempt
   every already-started operation; Mclone must strengthen those two areas
   without obscuring the parity shape.

## Current Mclone Facts

- `ChunkDistanceManager::set_aggregate_interest_positions_with_priority`
  immediately installs Player tickets for the entire current simulation set.
- Background persistence-load admission permits up to 128 requests.
- One feature job runs at a time but may contain up to 128 target chunks.
- Feature publication eagerly constructs `PendingLightStatus` values.
- Each target copies its own raw chunk and up to eight neighbor raw chunks.
- Native light request and completion channels are unbounded.
- Batch priority is fixed at enqueue; old batches cannot be globally
  reprioritized or cancelled.
- Unload eviction uses the same FIFO after older compute work.
- Completion only checks current holder scheduling state; it has no exact
  request-generation match.
- completed `ChunkStatusJob` and timing records are not pruned.

The two-minute RD7 Quest diagnostic produced Features at `42.916/s`, completed
Light at `20.266/s`, and ended with 1,026 pending Light statuses. The
server-update queue stayed at zero and render compilation remained bounded.

## Non-Goals

- Do not change light propagation values, opacity/emission parity, packed-light
  bytes, shaders, AO, or mesh-light sampling.
- Do not add provisional/fullbright geometry publication in this slice.
- Do not solve the separate generated-clean `Store`/`Regenerate` policy.
- Do not make distant terrain, multiview, or XR rendering responsible for
  authoritative chunk admission.
- Do not hide the issue by lowering a platform's configured render distance,
  disabling actors, or disabling lighting. `--xr-skip-actors` is permitted
  only as a matched attribution control and never substitutes for the normal
  actor product gate.
- Do not parallelize overlapping light worlds before bounded ownership and
  request identity are correct.
- Do not expose player-ticket or light-mailbox limits as ordinary graphics
  settings.

## Required Invariants

1. Desired interest and admitted promotion are distinct states.
2. No more than four cold Player positions actively promote by default.
3. A ready or already-progressing position does not consume another admission
   slot.
4. A position that leaves desired interest before admission creates no load,
   worldgen, Light, or cache-save work.
5. Pending promotion and Light demand coalesce by position and generation.
6. Priority is derived from current ticket/center state, not frozen FIFO order.
7. Current-center demand can overtake obsolete or outer-ring demand.
8. Every admitted Light operation has an exactly matched request token and
   temporary Light ticket.
9. Each unique raw input chunk is owned at most once per admitted light batch.
10. Total Light input + active + completion ownership obeys both status-count
    and byte limits.
11. The scheduler never blocks its tick waiting to send Light work.
12. Cancellation and unload control cannot wait behind arbitrary compute
    history.
13. Stale completions cannot publish or leak tickets.
14. Completed job-history storage has a fixed bound.
15. Native-thread, inline WASM, and Web Worker backends report the same logical
    admission/cancellation metrics.

## Proposed Shared Design

### A. Player promotion admission

Add a scheduler-facing admission owner alongside
`ChunkDistanceManager`'s desired aggregate sets. Keep its policy in
`mclone-server`; app crates only configure test/diagnostic overrides.

Provisional state:

```rust
struct PlayerPromotionAdmission {
    desired: BTreeMap<ChunkPos, PromotionIntent>,
    queued: BTreeSet<PromotionPriorityKey>,
    active: BTreeMap<ChunkPos, PromotionToken>,
    max_active: usize,
    next_generation: u64,
}

struct PromotionIntent {
    generation: u64,
    desired_ticket_level: i32,
    nearest_center_distance: i32,
    first_desired_tick: u64,
}
```

The concrete queue representation may differ, but it must support keyed
replace/remove and deterministic priority updates without retaining duplicate
heap payloads.

Reconciliation:

1. Diff new desired simulation positions against existing intent.
2. Remove and count stale unadmitted intent immediately.
3. Preserve Player tickets for positions already ready or already active.
4. Rank cold positions by current center distance, then age/fairness and stable
   position order.
5. Admit until `max_active=4`.
6. Install the actual Player ticket only on admission.
7. Release the admission slot when the holder reaches current runtime-ready
   status, fails, or leaves interest.
8. Immediately refill released slots from the current priority queue.

For multiple priority centers, use a deterministic nearest-center base with a
fairness/age term so sustained work for one participant cannot permanently
starve another. Record center attribution in tests and diagnostics; do not add
platform-specific scheduling.

Observer/residency-only tickets retain their current dependency/residency role
and must not silently become full Light promotion requests.

### B. Bound feature admission to admitted demand

`enqueue_runtime_chunks` and
`enqueue_feature_job_for_missing_targets` may schedule full runtime promotion
only for:

- already-ready/currently-progressing desired positions; or
- newly admitted Player positions.

Dependencies may still schedule the status required by the generation plan.
They must not independently widen into complete client-ready promotion.

Retain one native worldgen job at a time initially. The existing target limit
of 128 ceases to be the effective moving-view admission because only admitted
targets are eligible. Do not replace it with an unrelated second throttle
unless automated diagnostics show dependency expansion can still exceed the
accepted memory envelope.

When an unadmitted desired position leaves interest, its compact intent is
removed before any persistence miss, generation plan, or generated-cache write
exists.

### C. Light request identity and tickets

Introduce an opaque monotonic `LightRequestId` or `LightRequestToken`:

```rust
struct LightRequestToken {
    id: u64,
    pos: ChunkPos,
    feature_revision: ChunkRevision,
}
```

Store the expected token in scheduler/holder Light status state. Every pending
request and completion carries it.

Before bounded admission:

- holder still requires `ChunkStatus::Light`;
- holder feature revision matches;
- token remains the current desired token; and
- required source/neighbor facts are available.

On admission add `ChunkTicketType::Light` keyed by the request token or a
collision-free equivalent. Remove it on accepted completion, cancellation,
stale completion, worker error, dimension teardown, and shutdown. Add a
conservation assertion:

```text
light tickets added
= accepted + cancelled + stale + failed + shutdown-released
```

Publication requires exact token equality, not merely
`slot.step == Scheduled`.

### D. Compact Light demand and bounded dispatch

Replace direct `enqueue_light_status_batch(statuses)` with:

1. scheduler records/replaces compact per-position Light demand;
2. scheduler reprioritizes that demand from current centers;
3. scheduler asks the mailbox for nonblocking capacity;
4. scheduler materializes only the admitted batch;
5. capacity remains charged through worker execution and completion drain.

Initial internal limits:

- at most two ordinary batches;
- at most `18` admitted statuses with current batch size `9`; and
- at most `64MiB` of conservatively estimated owned inputs/completions.

One individually oversized unit may be admitted only if necessary to avoid a
deadlock and must be reported distinctly, following the durable-persistence
oversize pattern. It must not permit a second unit until ownership returns
under the normal limit.

Use nonblocking `try_admit`/capacity reservation semantics. A full mailbox
returns “not admitted”; the scheduler retains compact priority intent and
continues its tick.

### E. Shared input carrier

Refactor `PendingLightStatusBatch` so the same raw block vector is not copied
once per target that references it as a neighbor.

Required ownership shape:

```text
batch:
  unique input chunks: ChunkPos -> shared immutable raw blocks
  target requests: token + feature snapshot/ticks + input-position references
```

Use `Arc<[RawBlockId]>`, a batch-owned unique map, or another measured
immutable sharing scheme. The invariant is one owned raw array per unique
input chunk per admitted batch, not a prescribed container.

The worker-owned `RetainedInitialLightState` should receive inserts/replacements
from this unique map before computing targets. Long term, move closer to
Java's shared chunk/light owner so repeated batches do not transfer unchanged
raw facts. Do not expand this P0 into a general chunk storage rewrite if
batch-level sharing plus bounded admission closes the device invariant.

Add exact byte estimation to the carrier and completion. Counts without bytes
are not sufficient acceptance evidence.

### F. Cancellation and unload control

Provide a control mechanism that the worker observes before each target:

- latest desired token/generation;
- cancelled token set or generation floor;
- unload/evict positions; and
- shutdown.

This may be a separate bounded control channel, a shared synchronized control
registry, or an equivalent design. It must not put control behind the compute
FIFO it is intended to invalidate.

The worker may finish the graph drain already in progress. Before starting the
next status in a batch it skips cancelled tokens and reports them so ownership
and Light tickets are released. Unload eviction is applied before later
unrelated target computation.

The completion path remains bounded. A worker must not deadlock while holding
the only capacity token and trying to send into a full completion channel.
Prefer mailbox-owned completion storage covered by the same total reservation,
or a protocol in which each admitted request has exactly one pre-reserved
terminal result slot.

### G. Global priority and overload behavior

Use current priority when selecting both:

- the next Player promotion; and
- the next compact Light demand to admit.

Priority tiers:

1. current player center(s);
2. guaranteed inner exact radius;
3. remaining requested exact view by distance;
4. age/fairness within comparable distance.

Do not continually reject new near work merely because an outer request was
older. Conversely, once movement settles, aging must let the outer ring fill.

Expose an overload diagnostic when estimated new exact demand exceeds observed
Light completion capacity. Do not silently change the configured render
distance. The user-visible contract remains requested RD7 with progressive
readiness; the scheduler only determines safe work order.

### H. Completed job-history pruning

After a feature job:

- has no pending worldgen publication;
- has no pending light-status batch keyed to it;
- is no longer referenced by a holder status slot; and
- has contributed diagnostics,

remove it from `jobs` and `job_timings`.

Keep cumulative counters and a fixed recent ring, provisionally 64 completed
job summaries, for diagnostics. Assert in long movement tests that full job
records plateau near live pipeline ownership.

## Diagnostics

Extend `ChunkSchedulerDiagnostics`, `ServerRunnerDiagnostics`,
`mclone-app-runtime`, the browser Worker bridge, and Quest periodic/final
markers with:

```text
player_promotion_desired
player_promotion_queued
player_promotion_active
player_promotion_max_active
player_promotion_admitted_total
player_promotion_cancelled_before_admission
player_promotion_oldest_age_ms

light_demand_queued
light_status_admitted
light_status_active
light_status_completed_undrained
light_status_cancelled
light_status_stale
light_status_failed
light_pending_owned_bytes
light_pending_owned_bytes_high_water
light_oversize_admissions
light_ticket_count
light_ticket_conservation_failures

current_center_ready
inner_radius_ready_chunks
completed_job_records_retained
recent_job_summaries_retained
```

Retain existing arrival/completion rates, mailbox batch timing, retained light
world count, persistence metrics, server-update queues, render compilation,
RSS sampling, and world-size evidence. This lets the device test distinguish a
new limiting stage rather than merely observing that the process survived.

## Implementation Slices

### Slice 1: admission and identity

- Add compact desired/queued/active Player promotion state.
- Preserve aggregate interest for presentation and unload decisions.
- Admit four cold Player positions and clear stale unadmitted intent.
- Add promotion metrics and deterministic multi-center tests.
- Add Light request tokens and exact holder-token publication checks.
- Use the existing Light ticket type with terminal-path conservation tests.

Gate: a 10,000-chunk synthetic interest jump schedules only the admitted
working set and leaves no work for departed unadmitted positions.

### Slice 2: bounded shared-input light mailbox

- Add compact scheduler Light demand.
- Add total lifecycle count/byte capacity and nonblocking admission.
- Refactor batch input to unique shared raw chunks.
- Add bounded completion ownership.
- Add cancellation/control priority and unload-before-next-target behavior.
- Carry diagnostics through native and browser authority boundaries.

Gate: a blocked light worker cannot grow admitted inputs or outputs beyond the
configured counts/bytes, while newer center demand replaces stale queued
intent.

### Slice 3: history pruning and platform build closure

- Prune full completed jobs and retain a fixed recent summary ring.
- Add long deterministic movement tests for promotion, Light, retained-world,
  job-history, and persistence plateaus.
- Run the shared native, web, Android flat, and Android XR build/test matrix.
- Update the living lighting, performance, and procedural-horizon topics with
  the new current truth.

Gate: every supported authority backend exposes the same logical bounds and no
platform adapter owns scheduling policy.

### Slice 4: physical Quest acceptance

- Run a two-minute RD7 8x diagnostic first and verify the new metrics.
- Run the full RD7 20-minute persisted-world flight.
- If it completes, stop movement and measure full-view convergence.
- Pull and integrity-check the disposable database.
- Compare process RSS, available memory, Light throughput, queue bytes,
  cancellations, center readiness, world growth, and thermals with the
  2026-07-28 failure.
- Repeat a flat native or browser-authority long movement lane.

Gate: all ownership plateaus, current center remains prioritized, the process
completes normally, and the outer exact view converges after stopping.

## Automated Validation

Focused correctness:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-client
```

Cross-platform contracts:

```bash
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml \
  -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:web:build
pnpm --silent native:web:typecheck
pnpm native:android:apk
pnpm native:android-xr:apk
```

Run the current authoritative platform matrix from
[`../platforms.md`](../platforms.md#validation-policy) if it changes before
implementation.

### Performance regression gates

This tactical inherits the throughput and Quest frame-safety policy from
[`142-throughput-policy-with-quest-rd5-guardrail.md`](142-throughput-policy-with-quest-rd5-guardrail.md).
Do not judge this scheduler change from one candidate run or from aggregate
generated chunks per second during continuous high-speed travel.

Cancellation intentionally reduces work completed for positions the player has
already left. A lower Features or Light completion count during 8x travel is
acceptable when it represents obsolete work that was never admitted or was
cancelled. The binding useful-throughput measures are:

- stationary cold-load first-view-ready and settled time;
- current-center and guaranteed-inner-radius readiness while moving;
- Light completion rate for requests that remain desired;
- convergence time for the current center and then the full requested view
  after movement stops; and
- presentation frame work while the pipeline is loaded.

Record the exact parent commit, candidate commit, build profile, configuration,
output files, and APK checksum for every comparison. Capture a same-host,
same-configuration parent control immediately before the candidate. Absolute
Quest frame-safety limits remain binding; the adjacent parent control detects
throughput changes without ratcheting those safety limits.

Desktop inner loop for every implementation slice that changes scheduling,
publication, completion, or capacity:

```bash
pnpm native:scheduler-loading:perf
pnpm native:startup-streaming:perf
pnpm native:movement-frame:perf
```

Run the scheduler-loading and RD10 startup-streaming lanes on the parent and
candidate. Reproduce the candidate RD10 result once before promotion.

Desktop gates:

- stationary scheduler-loading first-view-ready, settled throughput, and Light
  completion rate do not regress by more than `10%`;
- RD10 startup-streaming first-full-view-ready and first-render-quiescent do
  not regress by more than `10%`;
- startup-streaming p95 measured frame work does not grow by more than `10%`;
- movement-frame over-budget frame count does not grow; and
- deltas below `5%` are noise and require a rerun before attribution.

Before final promotion, also run RD15 startup streaming with enough frames to
leave completion headroom and one native-window desktop check. The headless
probe does not prove swapchain/present pacing.

The existing scheduler-loading lane is stationary and server-only. Add or
extend a deterministic moving-interest benchmark in this tactical so it also
reports:

- desired, queued, active, cancelled, stale, and accepted work;
- current-center and inner-radius ready latency;
- stop-to-center-ready and stop-to-full-view-ready time;
- useful Light completions versus obsolete cancellations; and
- ownership high-water marks and final retained counts.

Its gate is zero current-center starvation, bounded stop convergence, and
plateaued ownership. Total completed obsolete chunks is not a positive
throughput measure.

### Attached Quest hardware gates

When an authorized Quest is attached, physical acceptance is mandatory for
this tactical. A missing headset may defer physical acceptance, but it does not
turn the hardware gate into a build-only pass and the tactical must remain
incomplete until the device evidence exists.

Use the current attached serial explicitly in recorded commands. At planning
time on 2026-07-28, the available device is Quest 3
`2G0YC1ZF93041Z` (API 34, arm64-v8a).

Quest per-candidate inner loop:

```bash
pnpm native:android-xr:perf:orbit:rd5:metrics
pnpm native:android-xr:perf:churn:rd5:metrics
```

For changes to shared scheduling, publication, worker, or completion policy,
also alternate a 45-second exact-only RD5 orbit on the parent and candidate
with `--xr-skip-actors`. This lane normalizes dynamic scene composition and
answers whether foreground app/thread CPU changed. It is attribution-only:
normal-actor orbit, churn, and flight rows remain binding. Record
`actors/drawn_actors`, horizon active/ready/draw facts, settle time, worldgen
and Light request counts, app-work p50/p95/p99, thread-CPU p50/p95/p99, and
over-period frames.

Candidates changing shared admission, publication, worker, or completion
policy must additionally run:

```bash
pnpm native:android-xr:perf:flight:rd7:metrics
pnpm native:android-xr:perf:orbit:rd7:metrics
```

The validator proves install/launch, required marker completeness, requested
configuration, fatal-log absence, and normal sample completion. It does not
currently apply numeric performance thresholds automatically. Until a summary
comparison command is added, record and review the values explicitly.

Binding Quest RD5 frame-safety limits:

- `skipped_delta = 0`;
- `dropped_frames_delta <= 17`;
- `app_work_p95 <= 13.0ms`;
- `headroom_avg >= +2.5ms`;
- `app_over_period_pct <= 2%`; and
- frames over twice the display period `= 0`.

Binding Quest RD7 pressure limits:

- `skipped_delta = 0`;
- `dropped_frames_delta <= 20`;
- `app_work_p95 <= 13.5ms`;
- `app_over_period_pct <= 2%`; and
- frames over twice the display period `= 0`.

Those absolute limits were pinned on clean commit `bc55076c`. Because render
and horizon policy has changed since that capture, also run the same exact lane
on the current parent APK before the candidate. Keep the absolute safety limits
unless a separately documented intentional product change re-pins them.

Quest streaming comparison additionally gates:

- no material regression in app-work p95/p99 or runtime upload/apply tails;
- no growth in update queue depth or oldest-applied-update age attributable to
  burstier publication;
- current-center readiness remains prioritized;
- bounded Light ownership and completed history; and
- a stable memory plateau rather than growth proportional to distance.

Do not infer a scheduler frame regression from normal rows with different
actor counts. Conversely, do not use a passing actor-skipped control to accept
a normal-actor product failure. Report both and route the latter to the
renderer/actor owner while retaining the scheduler's memory and useful-
throughput gates.

Quest results within `0.5ms` of a threshold get one thermally separated rerun.
Do not run surprising rows back-to-back on a heat-soaked headset.

Required focused tests:

- admission cap and refill;
- stale-before-admission clearing;
- already-ready admission bypass;
- deterministic current-center and multi-center priority;
- promotion slot release on ready/failure/interest loss;
- Light demand coalescing and generation replacement;
- status and byte capacity under a blocked worker;
- unique raw input ownership for overlapping 3x3 halos;
- cancel-before-compute and stale-after-compute;
- exact token publication;
- Light ticket release on every terminal path;
- high-priority unload eviction;
- bounded completion ownership;
- completed job pruning; and
- long movement with stable counts/bytes.

This slice is scheduling/ownership-only and should preserve packed light and
rendered pixels. Run existing lighting oracle/fixture tests. A new screenshot
is not required unless implementation changes pixels; if it does, stop and
apply the repository's rendered-output validation policy before proceeding.

## Physical Quest Command Shape

Use the existing scripted Android XR lane, not a hand-built Android command.
The acceptance run should retain the prior stress configuration:

```bash
bash android-xr/validate-quest-openxr.sh \
  --release \
  --serial 2G0YC1ZF93041Z \
  --skip-build \
  --skip-assets \
  --world-dir /sdcard/Android/data/com.kzahel.mclone.xr/files/lighting-soak/rd7-8x-1200s-v2 \
  --perf-seconds 1200 \
  --perf-flight \
  --perf-flight-speed 34.4 \
  --perf-metrics-periodic \
  --wait-seconds 1500 \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 7 \
  --day-time 6000 \
  --freeze-time
```

Build/install through `pnpm native:android-xr:apk` and the validator as needed.
Use a fresh explicit disposable world, independent 15-second memory sampling,
and `/tmp` for logs, summaries, monitors, database pulls, and screenshots.
Record the exact APK checksum and commit. Do not delete retained forensic worlds
until their size/integrity evidence is captured.

## Acceptance Disposition

All scheduler, ownership, platform, throughput, long-soak, integrity, and
convergence criteria below pass. The settled-orbit portion of the inherited
Quest presentation gate remains explicitly open as recorded above; closing
this scheduling tactical does not waive that separate release blocker.

The completed checklist is:

- vanilla-shaped four-at-a-time Player promotion admission is live;
- unadmitted stale travel positions create no downstream work;
- Light demand is keyed, reprioritizable, token-checked, and ticket-owned;
- one unique block input is shared across overlapping targets;
- status and byte limits cover request, active, and completion ownership;
- cancellation/unload control cannot wait behind arbitrary compute history;
- completed scheduler jobs plateau;
- shared native/browser tests and platform builds pass;
- parent/candidate desktop scheduler-loading and startup-streaming gates pass;
- the candidate RD10 result reproduces and movement-frame does not regress;
- RD15 and one native-window desktop promotion check pass;
- attached-Quest RD5 churn, RD7 flight, and long-pressure gates pass, while
  the inherited RD5/RD7 settled-orbit exception remains recorded and owned by
  the Quest presentation/renderer follow-up;
- the two-minute diagnostic shows bounded Light ownership and center priority;
- the physical 20-minute RD7 Quest flight completes without low-memory kill;
- memory plateaus independently of distance travelled;
- the full requested view converges after movement stops; and
- a non-XR authority lane proves the behavior is shared.

Do not declare success merely because one shorter RD5 run completes or because
the generated-clean storage policy reduces disk writes.
