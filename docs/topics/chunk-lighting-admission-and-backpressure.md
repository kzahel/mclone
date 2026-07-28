# Chunk Lighting Admission And Backpressure

Topic: `chunk-lighting-admission-and-backpressure`

Status: accepted P0 direction 2026-07-28; Tactical
[`279`](../tactical/279-chunk-lighting-admission-and-backpressure.md) planned,
implementation not started.

## Scope

This topic owns the exact-world scheduling and memory contract between player
interest, chunk-status promotion, initial lighting, client-ready publication,
and unload. It is intentionally narrower than the stored-light solver and
rendering progress tracked by [`lighting.md`](lighting.md).

It also remains separate from:

- bounded persistence request/write ownership in Tactical
  [`275`](../tactical/275-bounded-persistence-streaming.md);
- the per-world choice to store or regenerate deterministic clean chunks in
  [`generated-chunk-cache-policy.md`](generated-chunk-cache-policy.md);
- procedural horizon residency and rendering in
  [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md); and
- render-section compilation and GPU upload backpressure.

The incident was exposed by Android XR because Quest has a firm memory limit,
but the faulty owner is the shared authoritative chunk scheduler and native
light-status worker. The solution must apply to integrated native desktop,
flat Android, Android XR, desktop XR, dedicated-server, test/headless, and
browser authority paths through one host-neutral contract.

## Current Finding

Initial lighting is the measured bottleneck during sustained 8x travel, and
the current overload behavior retains obsolete work without a bound.

World generation continues publishing `ChunkStatus::Features` snapshots and
queuing cache persistence. A generated chunk becomes client-ready only after
the separate `ChunkStatus::Light` computation returns a `light_correct=true`
snapshot. Meshing is downstream of that boundary. The missing exact terrain in
the Quest incident was therefore generated and often stored, but had not
crossed the Light-to-client publication gate.

The current flow is:

```text
player interest
      |
      v
ChunkDistanceManager / ChunkScheduler
      |
      v
worldgen mailbox
      |
      +--> Features snapshot --> persistence mailbox --> SQLite/IndexedDB
      |
      +--> PendingLightStatus --> light mailbox --> retained light worker
                                                   |
                                                   v
                                             Light snapshot
                                                   |
                                                   v
                                            client replica
                                                   |
                                                   v
                                            mesh compilation
                                                   |
                                                   v
                                              GPU/render
```

“Lighting blocks publication” means it blocks client-ready exact publication,
not feature publication or persistence. The worldgen and storage branches can
therefore run far ahead while the client and mesh compiler remain starved.

## Physical Quest Evidence

### Earlier bounded-persistence closeout

Tactical
[`277`](../tactical/277-quest-procedural-horizon-performance.md) completed
three- and five-minute RD5 composed-horizon flights at `34.4` blocks/second.
Those runs travelled `6,191` and `10,319` blocks, retained an exact-ready
center, kept persistence ownership below about `0.61MiB`, and showed Quest RSS
oscillating around `1.03–1.22GiB`.

That evidence remains valid for Tactical 275's persistence bounds and the RD5
five-minute lane. It was not long enough or wide enough to prove that every
shared exact-world owner remained bounded.

### Extended RD7 reproduction

On 2026-07-28, the same physical Quest 3
`2G0YC1ZF93041Z` ran a fresh persisted-world pressure lane with:

- release Android XR build;
- normal actors and production per-eye frame-overlap rendering;
- render distance `7`;
- seed `12345`, frozen daytime;
- straight flight at `34.4` blocks/second, approximately 8x normal speed;
- a planned duration of 1,200 seconds; and
- independent process RSS, high-water, system-available-memory, world-size,
  battery, and thermal sampling every 15 seconds.

The app was killed after approximately `12m08s` of flight, before the planned
20-minute completion. The last recorded camera position was approximately
`(146.0, 73.6, -25136.5)`, or about `25,040` travelled blocks.

Horizon OS recorded:

```text
Process (3369) has memory footprint (6632088kB),
which is over the max memory threshold (6577152kB)

Kill 'com.kzahel.mclone.xr' ... to free 4179252kb rss,
and 1923008kB swap; reason: low watermark is breached and swap is low
```

External samples observed:

- maximum sampled RSS: about `4.17GiB`;
- process RSS high-water: about `4.22GiB`;
- minimum system `MemAvailable`: about `224MiB`;
- terminal low-memory-killer footprint: about `6.63GB` including swap; and
- stable 90% charging state, with sampled device temperature rising from about
  32°C to 40°C.

The world remained readable after the unclean kill:

- SQLite integrity: `ok`;
- `29,763` chunk records;
- chunk bounds `x=-5..18`, `z=-1566..-1`;
- most Z rows contained 19 chunks, matching the generated travel corridor; and
- about `572MiB` of world storage.

The database evidence matters: exact terrain generation and persistence did
not stop. The current-position client-ready path fell behind.

### Two-minute attribution run

A fresh 120-second run with the same RD7, speed, seed, and lighting settings
completed normally so final queue metrics could be emitted:

| Observation | Result |
|---|---:|
| feature chunks | `5,150`, `42.916/s` |
| completed light statuses | `2,432`, `20.266/s` |
| light mailbox pending statuses | `1,026` |
| light mailbox pending request frames | `141` final, `164` maximum |
| server update queue maximum | `0` |
| worldgen mailbox pending jobs | `1` |
| render-compile pending limit | `4` |
| completed scheduler jobs retained | `248` |

This rules out the server-to-client update queue as the active backlog. It also
shows that mesh compilation was not being flooded: chunks were waiting before
the client-ready boundary.

At RD7 the exact square is about 15 chunks wide. Straight movement at
`34.4` blocks/second crosses about `2.15` chunk columns/second, introducing
roughly `32` new exact chunks/second. The measured Quest light worker completed
about `20` statuses/second. Full RD7 exact convergence is therefore impossible
at this stress speed without more light throughput. A safe scheduler must
degrade gracefully instead of retaining the difference forever.

## Current Mclone Ownership Defect

The active defect is a queueing and ownership problem rather than a lost-pointer
allocator leak.

### Eager, duplicated light inputs

`PendingLightStatus::from_feature_publication` owns:

- the feature snapshot;
- `65,536` raw block bytes for the 256-block-tall target chunk;
- a copied raw block vector for each available neighbor in the 3x3 lighting
  halo, up to eight additional `65,536`-byte arrays; and
- scheduled block/fluid tick vectors and allocation overhead.

A fully surrounded request therefore carries `589,824` raw block bytes before
the snapshot and other allocations. At 1,026 queued statuses, those raw arrays
alone can account for about `577MiB`.

The same neighbor block array is copied into multiple target requests. The
retained light world eventually deduplicates state after the worker receives
the batch, but the mailbox retains the duplicated pre-compute carriers.

### Unbounded FIFO

The native `LightStatusMailboxBackend` uses unbounded request and completion
`std::sync::mpsc::channel`s. `enqueue_batch` always succeeds and increments a
pending count; it has no admission limit, owned-byte limit, keyed replacement,
or cancellation operation.

Priority sorting happens only within the newly created batch. Older batches
remain ahead in a global FIFO, so terrain around the latest player position
cannot overtake kilometers of old work.

### Late stale-result rejection

The scheduler checks holder state after a completed light status returns and
skips publication when Light is no longer scheduled. That protects some output
semantics but performs the rejection after the expensive input was retained
and the computation completed.

The completed value carries position and snapshot revision but no explicit
light-request generation tied to the holder slot. A future implementation must
not allow an obsolete completion for a position to satisfy a newer request for
that same position.

### Delayed retained-state eviction

Light-worker unload requests ride the same FIFO as compute batches. An unload
therefore cannot evict old retained block/light state until every earlier
compute request finishes. This makes overload retain both queued inputs and
worker-owned historical state longer than current interest requires.

### Secondary unbounded history

Completed `ChunkStatusJob` entries remain in the scheduler's `jobs` map after
`mark_job_complete`; timings are retained separately as well. The 120-second
run reached 248 completed jobs. This is much smaller than the copied lighting
payloads but is still distance-proportional history and belongs in the same
long-travel closure.

## Minecraft Java 1.17.1 Reference Research

The reference does not solve this with a simple bounded lighting channel.
Several cooperating lifecycle mechanisms prevent expensive light payloads
from being created without limit.

### Upstream player-ticket throttle

`DistanceManager` constructs a dedicated player-ticket
`ChunkTaskPriorityQueueSorter` with `maxTasks=4`. Its processor uses acquisition
markers, so only four newly requested player chunk positions actively progress
toward entity-ticking readiness at once.

When a desired player position leaves range before admission, the release path
uses `clearQueue=true` and removes the queued ticket task. Once an admitted
position reaches its entity-ticking future, the throttler slot is released and
the next priority request may enter.

Reference:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
  constructor and `PlayerTicketTracker.onLevelChange(...)`;
- `ChunkTaskPriorityQueueSorter.getProcessor(..., true)` acquisition; and
- `ChunkTaskPriorityQueue.release(..., fullClear)`.

This is the largest behavioral difference from Mclone. Mclone immediately adds
Player tickets for the complete simulation set, admits background load batches
up to 128, and may generate feature jobs containing up to 128 targets. Vanilla
keeps excess player demand as compact, removable ticket intent.

### Ticket-level priority and live resorting

`ChunkTaskPriorityQueue` groups work by chunk position and ticket level.
`ChunkHolder.updateFutures(...)` reports ticket-level changes through
`ChunkTaskPriorityQueueSorter.onLevelChange(...)`, which moves queued work to
the new priority bucket.

This is live global reprioritization. It is not equivalent to sorting only the
members of a batch at enqueue time.

### Status futures stop downstream scheduling

Each Java `ChunkHolder` owns one future per `ChunkStatus`.
`getOrScheduleFuture(...)` only schedules a status when the current ticket
level requires it. When a holder is demoted, no-longer-required status slots
become unloaded failures, preventing later status work from being introduced.

This does not preempt every already-running operation, but it makes demand
loss visible at each status boundary.

### Light ticket lifetime

`ChunkMap.schedule(..., ChunkStatus.LIGHT)` adds `TicketType.LIGHT` before the
asynchronous light operation. `ThreadedLevelLightEngine.lightChunk(...)`
marks the chunk not light-correct, queues pre-update work, and returns a future
whose post-update marks it light-correct, releases retained data, and calls
`ChunkMap.releaseLightTicket(...)`.

The light ticket pins the shared chunk graph for exactly the asynchronous
operation's lifetime. Unload is coordinated through ticket ownership rather
than by copying data into an independent queue and later sending an unrelated
FIFO eviction command.

### Shared chunks instead of copied neighborhoods

Java's light task captures the existing `ChunkAccess`. `LevelLightEngine`
queries shared chunk/section state through `LightChunkGetter`; neighboring
chunks are already retained by holder status dependencies. It does not clone a
3x3 raw-block neighborhood for every target.

### Small active light batches

`ThreadedLevelLightEngine` defaults `taskPerBatch` to five. It runs up to five
PRE_UPDATE tasks, drains propagation, then runs matching POST_UPDATE tasks.
The light sorter itself is not given a useful hard maximum in `ChunkMap`, so
this is an active-batch bound rather than proof of a globally bounded queue.

### Limits of the vanilla design

Java 1.17.1 can still fall behind, retain compact queued tasks, and finish
already-admitted lighting after player interest changes. The general chunk
worldgen/light sorter is constructed with `Integer.MAX_VALUE`, and there is no
hard byte budget for queued light work.

Mclone should port the behavioral invariants—upstream admission, keyed
priority, stale queued-ticket clearing, status gating, shared data, and light
ticket lifetime—without copying Java's lack of a hard memory ceiling. Quest
requires a stronger explicit bound.

## Accepted Direction

### 1. Separate desired interest from admitted promotion

`ChunkDistanceManager` must retain the complete current desired
resident/simulation sets, but Player tickets for newly cold exact chunks must
pass through a compact keyed admission queue.

Use vanilla's four active player promotions as the first parity-shaped default.
An already-ready or already-progressing chunk must not consume a second slot.
An admitted slot is released when the position reaches the runtime client-ready
status (`Light` with lighting enabled, otherwise `Features`), fails, or loses
interest.

This is an internal scheduling policy, not a graphics setting. Diagnostic and
test overrides may vary the value; product clients should share the same
authority-owned default.

### 2. Key and reprioritize queued demand

Pending promotion and pending Light demand must be keyed by chunk position and
request generation. Reconciliation must:

- remove queued positions that left interest;
- replace older demand for the same position/revision;
- rank current player centers before outer-ring work;
- update priority when centers or ticket levels move; and
- retain age/fairness so one player or outer ring cannot starve forever after
  movement settles.

### 3. Give Light work an explicit token and ticket

Every initial-light request and completion must carry a token containing at
least position, source feature revision, and a monotonic light-request
generation. The holder's Light slot records the expected token.

The scheduler adds the existing `ChunkTicketType::Light` when work is admitted
and removes it on accepted completion, stale completion, cancellation,
failure, or shutdown cleanup. Publication requires an exact token match.

### 4. Keep pending demand compact; materialize only admitted input

Do not build `PendingLightStatus` neighborhood copies merely because Light is
desired. Construct worker input only when bounded mailbox capacity is
available.

The cross-thread carrier must own each unique raw chunk array at most once per
admitted batch, preferably through immutable shared ownership. Target statuses
refer to that shared batch/world input. The long-term parity direction is a
worker-visible retained chunk owner analogous to Java's shared `ChunkAccess`,
not serialized 3x3 worlds per target.

### 5. Bound total light ownership

Capacity accounting must cover:

```text
admitted request inputs
+ worker-active input
+ completed output awaiting scheduler drain
```

The scheduler thread must never block on a full light queue. It retains compact
intent and retries later. Capacity is released only when the completion is
consumed or the request is definitively cancelled/dropped.

The first implementation should use both status-count and conservative
owned-byte limits. Tactical 279 proposes no more than two ordinary light
batches (`18` statuses with the current batch size `9`) and `64MiB` across the
admitted lifecycle, while keeping the constants internal and measured.

### 6. Prioritize cancellation and unload control

Cancellation/interest-generation changes and unload eviction must be visible
to the worker before it begins the next target, not queued behind the entire
compute FIFO. A separate control path, shared generation registry, or
equivalent design may satisfy this.

Cancellation granularity must include individual statuses inside a batch.
An already-running graph drain may finish, but the next target must observe the
latest desired generation.

### 7. Degrade exact radius gracefully

At a travel rate that exceeds measured Light throughput, the scheduler must
guarantee current-center and inner-radius priority rather than promise the full
requested outer exact radius.

The procedural horizon may cover distant presentation, but it is not required
for correctness. Once movement slows or stops, ordinary priority aging fills
the complete requested exact view.

A later geometry-first provisional-light fallback could publish feature
geometry before exact Light and remesh after lighting. That is a separate
visual/semantic decision and is not the P0 memory fix.

### 8. Prune completed scheduler history

Remove completed job metadata as soon as no pending publication, holder status
slot, or diagnostics consumer references it. Retain bounded aggregate timing
statistics and a small fixed recent-debug ring instead of all historical jobs.

## Platform Contract

The shared scheduler owns desired/admitted interest, tokens, tickets, priority,
and capacity. Platform adapters do not choose chunk admission.

- Native desktop, Android, XR, dedicated server, and headless use the native
  worker implementation behind that contract.
- Browser authority uses the same scheduler admission/token/capacity behavior
  whether Light is temporarily inline or runs through a Web Worker.
- Remote clients observe server-owned readiness and cannot change admission
  limits.
- Multiview, per-eye, and flat rendering consume the same client-ready
  snapshots and do not participate in server scheduling policy.

## Storage Policy Interaction

Upstream cancellation means a sufficiently fast player will not generate every
transient chunk crossed. No generated record exists to persist for work that
never passes admission.

For admitted chunks:

- `Store` may persist generated-clean Features or Light records under the
  accepted cache policy;
- `Regenerate` may suppress proven generated-clean cache writes; and
- durable edits, entities, players, metadata, and tombstone-like empty entity
  records remain mandatory.

The storage choice can reduce disk growth and write CPU. It must not weaken the
lighting bounds or be used as the explanation for their memory safety.

## Diagnostics And Acceptance

Required diagnostics:

- desired, queued, active, completed, cancelled, and stale player promotions;
- pending/admitted/active/completed/cancelled/stale Light statuses and batches;
- current and high-water estimated Light-owned bytes;
- oldest queued/admitted age;
- Light arrival and completion rates;
- temporary Light ticket count and leaked-ticket conservation failures;
- retained light-world chunks;
- current-center and inner-radius readiness;
- completed scheduler-job count and retained recent-history count; and
- per-platform backend/kind availability labels.

Automated movement tests must prove:

- four-at-a-time player promotion admission;
- stale unadmitted requests clear when interest jumps;
- current-center work overtakes obsolete outer work;
- total admitted Light ownership never exceeds either limit;
- duplicate requests coalesce;
- stale tokens cannot publish;
- every Light ticket is released on all terminal paths;
- unload eviction is not trapped behind arbitrary compute history;
- completed job history plateaus; and
- a long synthetic high-speed walk keeps all live counts and bytes bounded.

Physical acceptance repeats the exact extended Quest lane:

- RD7, `34.4` blocks/second, 20 minutes;
- process RSS/high-water and system available memory sampled independently;
- exact-center readiness sampled throughout and after stopping;
- promotion, Light, persistence, job-history, and retained-world metrics;
- normal completion without Android low-memory kill; and
- a memory plateau based on active working set rather than distance travelled.

The full RD7 outer exact ring need not remain complete at 8x. The center must
remain prioritized, and the full view must converge after movement stops.
A matching flat native or browser authority soak must show that the fix is
shared rather than XR-specific.

## Code And Reference Map

Current Rust owners:

- `native/crates/mclone-server/src/distance_manager.rs`
- `native/crates/mclone-server/src/scheduler.rs`
- `native/crates/mclone-server/src/holder.rs`
- `native/crates/mclone-server/src/light_status.rs`
- `native/crates/mclone-server/src/light_mailbox.rs`
- `native/crates/mclone-server/src/light_world.rs`
- `native/crates/mclone-server/src/runner.rs`
- `native/crates/mclone-app-runtime/src/lib.rs`

Minecraft Java 1.17.1 references:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueue.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java`

## Related Documents

- [`lighting.md`](lighting.md) — solver, status, persistence, and render-light
  parity index
- [`performance.md`](performance.md) — current performance priorities and Quest
  baselines
- [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md) — fixed
  distant-presentation residency and incident history
- [`generated-chunk-cache-policy.md`](generated-chunk-cache-policy.md) —
  separate clean generated storage decision
- Tactical [`042`](../tactical/042-native-light-status-scheduling.md) —
  initial client-ready Light status boundary
- Tactical [`046`](../tactical/046-native-retained-initial-light-world.md) —
  retained initial light owner and originally deferred priority/release work
- Tactical [`153`](../tactical/153-vanilla-shaped-chunk-pipeline-capacity.md) —
  earlier pipeline capacity and measured light-throughput handoff
- Tactical [`275`](../tactical/275-bounded-persistence-streaming.md) — bounded
  persistence queues and cancellation
- Tactical [`277`](../tactical/277-quest-procedural-horizon-performance.md) —
  valid RD5 five-minute device evidence
- Tactical [`279`](../tactical/279-chunk-lighting-admission-and-backpressure.md)
  — planned implementation and acceptance record
