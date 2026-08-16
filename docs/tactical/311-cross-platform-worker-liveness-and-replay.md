# Tactical 311: Cross-Platform Worker Liveness and Replay

Status: planned 2026-08-16; physical Android failure evidence complete

Topic: `web-worker-runtime-ownership`

## Instruction Synthesis

Investigate the persistent browser terrain holes with live tests on a real
Android device, add diagnostics only where necessary, distinguish proven
failure scenarios from hypotheses, and recommend a comprehensive correction.
Use the result to make browser Worker failure, retry, and completion semantics
match native worker semantics as closely as the hosts permit.

The implementation must correct worker lifecycle and lost-work behavior rather
than masking holes with a lower render distance, retained stale terrain,
disabled lighting, longer waits, or Web-only scheduler policy. Preserve the
isolated-Wasm actor architecture, domain-blind TypeScript boundary, external
`SharedArrayBuffer` mailboxes, shared worldgen and lighting algorithms, and
shared render-section planning.

This tactical follows completed Tactical
[`303`](303-web-integrated-runner-semantic-parity.md). Tactical 303 corrected
stale view-command replay, detached persistence continuations, independent
background progress, retained Web lighting state, and a saved-data update-loss
path. Those changes are valid but do not provide lifecycle recovery when a job
Worker stops responding, and the render Worker restart path can still consume
one failed compile without replaying it.

## Investigation Boundary

The investigation made no repository changes. It exercised the public deployed
build and used existing smoke diagnostics, Chrome DevTools Protocol target
control, Android process diagnostics, and physical screenshots. The controlled
faults affected only fresh transient probe tabs; the probe tab and ADB debugging
forward were closed afterward.

The primary test environment was:

- Google Pixel 7a, physical 1080 by 2400 display;
- Android Chrome `151.0.7922.137`;
- `navigator.deviceMemory = 8` and `hardwareConcurrency = 8`;
- `crossOriginIsolated = true`;
- public URL `https://mclone.kzahel.com/app`;
- deployed revision `0f0878067bd6-20260816091019`; and
- local source inspection at revision `46d49dd01b9f`, whose relevant worker
  lifecycle paths had not changed after the deployed revision.

The fault runs used a fresh transient world to isolate worker lifecycle from
IndexedDB:

```text
?smokeObserver=1
&startInWorld=1
&renderDistance=8
&movementMode=fly
&worldStorage=transient
&seed=0
```

The persistent-world observation used the real title-screen and options flow.
No conclusion below depends on a headless browser or a desktop mobile viewport.

## What Is Proven

The following statements are direct observations, not proposed explanations:

1. A clean physical-phone render-distance-8 run can produce all requested
   chunks and all exact drawable columns in ordinary time. Generation capacity
   is therefore sufficient for the test case.
2. Silently stopping either the worldgen or lighting Worker while it owns work
   produces a permanent server and client plateau with no error, no replacement
   Worker, and no recovery after more than one minute.
3. Throwing an explicit worldgen Worker error tears down the integrated server
   hierarchy and strands later commands instead of reconstructing the job
   actor or reporting a terminal session state.
4. The render Worker does reconstruct after its existing timeout, but the
   compile request interrupted by failure is removed from in-flight state and
   never requeued. The view can then report settled with one exact column
   permanently missing.
5. Ordinary persistent-world exploration can attempt to save a generated chunk
   whose record address was never read into the browser executor. The executor
   rejects that mutation as not revision-safe.
6. The existing mobile browser probes do not test physical Android Worker
   lifecycle, worker termination, worker generations, request replay, or exact
   coverage after recovery.

The investigation did not capture an unforced spontaneous Worker death during
its one clean baseline. Browser or Android memory pressure is a plausible
initiator, but it is not established as the initiating cause. The controlled
faults prove that once a Worker disappears or errors, the production lifecycle
cannot recover and produces the same indefinitely patchy or hard-boundary
terrain class reported by human review.

## Live Baseline: The Phone Can Complete the Work

The first real-input pass created a new world by dispatching touch events to
the actual canvas. At the default browser radius it advanced through:

| Elapsed sample | Requested client chunks | Exact drawable columns |
|---:|---:|---:|
| about 29.4 s from page start | `22 / 81` | `9 / 49` |
| about 31.5 s | `73 / 81` | `45 / 49` |
| about 33.5 s | `81 / 81` | `49 / 49` |

The same physical session then raised render distance to 8 and exercised
cardinal flight. It reached:

```text
requested client coverage: 361 / 361
exact drawable coverage:   289 / 289
server jobs:               0
worldgen mailbox:          0
lighting mailbox:          0
render compile jobs:       0
```

The render-distance-8 fill took roughly 14 seconds. A separate clean transient
baseline produced the same final coverage with worldgen request/inbound frames
`17 / 17` and lighting request/inbound frames `56 / 56`.

This falsifies the claim that the reported one-minute holes are simply the
expected time required to generate the view. A healthy run on this device can
finish considerably sooner.

### Input methodology warning

An early attempt invoked smoke-observer menu methods directly. It left two
commands queued while no render frame woke the application, and one forced
smoke frame plus `resumeRendering()` released them. The real DOM touch route
did not reproduce that pause and created the world normally.

That result is a probe defect, not production evidence: direct smoke methods
can bypass the wake normally caused by browser input. Future menu-flow gates
must use real pointer/touch dispatch or explicitly exercise and assert the
normal wake contract.

## Reproduction A: Silent Worldgen Worker Loss

### Procedure

1. Open a fresh transient render-distance-8 probe and wait for exact
   `361 / 361` requested and `289 / 289` drawable coverage.
2. Attach DevTools Protocol to the target titled `mclone-worldgen`.
3. Evaluate the following inside that Worker:

   ```javascript
   setTimeout(() => self.close(), 0)
   ```

4. Confirm that the `mclone-worldgen` target disappears and that no replacement
   target appears.
5. Move the player six chunks into new terrain.
6. Sample server, mailbox, delivery, and exact-coverage counters for at least
   60 seconds.

### Result

The counters plateaued unchanged for the complete 61-second observation:

```text
requested client coverage:      238 / 361
exact drawable coverage:        180 / 289
server missing chunks:          123
server pending jobs:            124
worldgen mailbox pending:       1
worldgen request/inbound frames: 18 / 17
lighting request/inbound frames: 64 / 64
runner last error:              empty
replacement worldgen Workers:   0
```

The single unmatched worldgen frame and retained pending count never became a
completion, failure, or retry. The visible result was a hard boundary into
empty terrain:

- [physical Android capture](/tmp/mclone-android-worldgen-worker-loss.png)

## Reproduction B: Silent Lighting Worker Loss

### Procedure

Repeat Reproduction A, but close the target titled `mclone-light-status` after
the initial exact view settles.

### Result

The counters again remained unchanged for the complete 61-second observation:

```text
requested client coverage:       238 / 361
exact drawable coverage:         180 / 289
server missing chunks:           123
server pending jobs:             163
lighting mailbox pending:        18
worldgen request/inbound frames: 19 / 19
lighting request/inbound frames: 65 / 55
runner last error:               empty
replacement lighting Workers:    0
```

Worldgen completed all of its frames, while lighting retained ten unmatched
requests and the server could not promote the new view. The visible hard
boundary is recorded in:

- [physical Android capture](/tmp/mclone-android-light-worker-loss.png)

## Reproduction C: Explicit Worldgen Worker Error

### Procedure

After a fresh exact render-distance-8 baseline, evaluate this inside the
`mclone-worldgen` target:

```javascript
setTimeout(() => {
  throw new Error("mclone-probe-worldgen-error");
}, 0)
```

Then move six chunks into new terrain and sample the actor targets, command
queue, and coverage.

### Result

- `runnerLastError` reported `Uncaught Error:
  mclone-probe-worldgen-error`.
- The integrated-server, worldgen, and lighting targets all disappeared.
- No replacement authority or job actor appeared.
- Coverage stopped at `208 / 361` requested chunks and `154 / 289` exact
  columns.
- The command queue grew from `69` to `100` during the following 30 seconds.
- Worldgen and light frame counts remained frozen at `17 / 17` and `56 / 56`.

This is not a silent stall, but it is equally unrecoverable. A job-actor error
escapes through an `expect`, destroys the authority hierarchy, and leaves the
main application accepting or retaining commands for a server that no longer
exists.

## Reproduction D: Incomplete Render Worker Recovery

### Procedure

1. Start a clean transient render-distance-8 world and wait for:

   ```text
   requested client coverage: 361 / 361
   exact drawable coverage:   289 / 289
   render generation:         1
   worker init count:         1
   completed compile count:   306
   ```

2. Close the target titled `mclone-render-compiler-app` with `self.close()`.
3. Move six chunks into fresh terrain while one compile is outstanding.
4. Wait for the existing render timeout and replacement generation.
5. Continue sampling exact coverage after all compile and streaming counters
   report idle.

### Result

The existing timeout detected the loss after about 20 seconds. A replacement
Worker appeared, generation and initialization counts advanced from 1 to 2,
and compile count advanced from 306 to 441. Server and client snapshot coverage
reached their complete `361 / 361` target.

Render recovery nevertheless stopped at:

```text
exact drawable coverage:        288 / 289
pending render compiles:        0
render Worker pending requests: 0
streamingSettled:               true in settled samples
missing-column count:           1
missing-column hash:            08a2ac6c839261bc
interest center:                (3, 6)
```

The singleton FNV diagnostic hash resolves to missing column `(0, 9)`, which
is inside the render-distance-8 target. It remained missing for more than a
minute even though the replacement Worker was healthy and all pending work was
zero.

- [physical Android capture](/tmp/mclone-android-render-worker-recovery-hole.png)

The source path proves why. `WebSharedRenderSectionCompiler::fail_in_flight`
creates an error `RenderSectionCompileResult`. The shared completion path then
removes every target section from `inflight_sections` before mapping the
compile error into an `Err`. The sections were already removed from dirty work
when submitted and are not requeued on this error. The replacement Worker can
therefore become perfectly idle while one target column has no accepted
result.

`ViewSettledStatus::ready()` independently omits exact coverage from its
predicate. It can declare the view settled when requested snapshots and work
queues are complete even though `exact_chunk_coverage_status()` still reports
a missing traversal-ready column.

## Reproduction E: Persistent Generated-Chunk Mutation Failure

During ordinary movement in the real newly created persistent world, the
runner recorded:

```text
browser record PersistenceRecordAddress {
  namespace: Chunk,
  key: [Text("minecraft:overworld"), I32(-9), I32(12)]
} must be read before revision-safe mutation
```

That particular run still reached exact terrain coverage, so this observation
is not claimed as the cause of its streaming result. It proves a remaining
revision-safe persistence defect distinct from Tactical 303's saved-data
bootstrap correction.

`WebPersistenceRecordExecutor::commit` rejects any mutation whose address is
absent from its main-side record map. A newly generated or evicted clean chunk
can legitimately require its first write without having been read from
IndexedDB. The current contract treats that valid state as an unavailable
error instead of asynchronously establishing known absence and retrying the
commit.

## Source-Correlated Failure Mechanics

### Server job Workers have no liveness lifecycle

In `native/crates/mclone-server/src/wasm_job_worker.rs`:

- lines 103-123 decrement `pending_count` only in `onmessage`;
- lines 167-178 make `onerror` store one error string;
- lines 200-217 record request identity, start time, and pending count, but do
  not attach a completion deadline;
- lines 324-346 surface a stored error only when the owner next drains or
  posts; and
- there is no generation replacement, timeout, heartbeat, replay, or terminal
  outcome for all in-flight requests.

Calling `DedicatedWorkerGlobalScope.close()` produces no response and no
`onerror`, so the coordinator retains the pending job forever. Browser process
loss can be observationally equivalent from this layer even when its initiating
cause differs.

### Worldgen mirror state becomes speculative truth

In `native/crates/mclone-server/src/worldgen_mailbox.rs`:

- lines 209-213 post with `expect`;
- lines 214-224 apply seeded inputs to `worker_mirror_shadow` immediately on
  submission; and
- lines 242-246 drain with `expect`.

The shadow therefore describes what the Worker is expected to retain after it
eventually processes the request, not what it has acknowledged retaining. If a
generation dies, reconstructing another Worker from that shadow can omit
dependencies the failed generation never accepted.

### Render failure consumes work before returning the error

In `native/crates/mclone-render-session/src/dirty.rs`, lines 86-96 remove
completed targets from `inflight_sections`. In
`native/crates/mclone-render-session/src/compile_queue.rs`, lines 281-301 call
that mutation before evaluating the result error. There is no failed-result
requeue. `native/apps/mclone-web-client/src/web_render_worker.rs` does provide
the 20-second generation timeout and reconstruction, so the transport returns
but its interrupted logical job does not.

### Readiness does not require drawable completeness

`native/crates/mclone-scene/src/mono.rs`, lines 126-136 define settled state
from startup, requested-view, and pending-work facts. Exact coverage is
computed separately at lines 2261-2288 from the intersection of target columns
and traversal-ready draw columns. The two predicates can disagree permanently.

### Generated browser records can be unknown at commit

`native/apps/mclone-web-client/src/web_server_worker.rs`, lines 2257-2271
reject mutation unless the address is already present in the executor map.
Tactical 303 preloaded a shared list of known realm bootstrap records, but that
finite list cannot enumerate every chunk generated later during exploration.

## Visual Morphology

The controlled worldgen and lighting failures produce a large hard frontier:
the entering view never advances beyond the last promotable work. The human
captures also contain cross-shaped holes. One absent client snapshot can cause
that shape because:

- `render_dirty_chunk_neighborhood` dirties the changed column and its four
  cardinal neighbors; and
- distant render-section readiness waits for all four horizontal neighbor
  snapshots.

This source relationship makes the human screenshot morphology consistent
with lost snapshot or compile work. It is an inference from the image and
shared readiness rules, not proof of which Worker spontaneously failed in that
specific human session.

## Memory Observation, Not Root-Cause Claim

During the multi-session fault campaign, one Chrome native-only sandboxed
renderer process reached roughly 1.67 GiB RSS. Reported shared buffer capacities
also reached approximately:

```text
worldgen request/response arena: 100,663,312 bytes
lighting request/response arena: 41,943,088 bytes
render arena:                    16 MiB
```

Each Worker also owns a separate Wasm heap. These values were collected after
several probe sessions and controlled Worker failures, not from a clean
single-world baseline. They justify memory and process-lifecycle stress tests;
they do not prove that Android killed a Worker for memory in the original
report.

## Correctness Target

Native OS-thread workers and browser Workers have different construction and
failure mechanisms, but they must implement one logical executor contract:

```text
logical job
  -> dispatch attempt { job id, worker generation, attempt, deadline }
  -> exactly one accepted terminal outcome
       Completed(result)
       RetryableFailure(reason)
       Cancelled(obsolete interest)
       FatalFailure(reason)

worker generation
  -> Starting
  -> Ready
  -> Busy
  -> Failed or TimedOut
  -> Reconstructing
  -> Ready
```

Execution may be at least once across a reconstruction. Acceptance must be
exactly once by logical job identity, revision, and generation. A late response
from an obsolete generation must never mutate retained actor state or satisfy
the replayed job.

The browser need not serialize native jobs or use the browser byte ABI on
desktop. Parity means both backends report the same logical outcomes to their
shared scheduler and obey the same replay, cancellation, stale-result,
readiness, and terminal-failure rules.

## Binding Architecture Decisions

### Shared Rust owns lifecycle semantics

Define the logical worker lifecycle and job-attempt state in the shared owner,
not in TypeScript and not in an app-only Web coordinator. The browser adapter
may construct, wake, terminate, and observe a physical Worker. The native
adapter may spawn and join an OS thread or observe channel disconnection. Both
must lower physical events into the shared lifecycle outcomes.

Every accepted job records at least:

- stable logical job ID;
- actor kind and worker generation;
- attempt number;
- submission and last-progress time;
- completion deadline or lease;
- request revision and retained-state generation where applicable; and
- scheduler cancellation state.

Track jobs until a terminal outcome is accepted. A successful `postMessage` or
channel send is not completion and must not make scheduler work disappear.

### Detect silent loss with deadlines

Browser APIs do not guarantee an observable event when Worker code calls
`close()` or a worker process disappears. Liveness therefore requires a
coordinator-owned deadline over the oldest in-flight request. The watchdog
must be driven by the ordinary scene/server pump, not by messages from the
possibly dead Worker.

Browser visibility is an explicit host input. Do not repeatedly kill healthy
Workers merely because a hidden page was suspended or timers were throttled.
On foreground resume, grant a bounded progress window, then reconstruct any
generation whose in-flight work still makes no progress. Native uses the same
logical deadline policy without browser visibility suspension.

At least one automatic replacement generation is required. Retry count and
backoff must be bounded and diagnosed; exhaustion must produce a visible typed
session failure rather than infinite restarts or a false settled view.

### Replay scheduler work, not raw browser messages

When a worldgen or lighting generation fails:

1. atomically fail every attempt assigned to that generation;
2. release or invalidate its SAB slots and pending counters;
3. terminate the physical Worker if it still exists;
4. construct and initialize a replacement generation;
5. return retryable outcomes to the shared scheduler; and
6. let the scheduler re-evaluate cancellation, priority, dependencies, and the
   current interest view before redispatch.

Do not blindly resend serialized frames from an obsolete view. The shared
scheduler remains authoritative about which logical jobs are still necessary.

### Reconstruct retained worker state from acknowledged truth

Worldgen and lighting need three distinct facts:

- desired authoritative retained state;
- state acknowledged by the active Worker generation; and
- speculative deltas assigned to in-flight attempts.

Only a valid completion may advance acknowledged state. On generation failure,
discard speculative deltas. Initialize the replacement with `reset = true`
and a complete bounded snapshot of the authoritative dependencies required by
the jobs selected for replay. The first response establishes the new
generation's acknowledged retained set.

Lighting must similarly reconstruct its retained light world and apply the
current unload set before replay. A replacement may not inherit coordinator
belief about state acknowledged only by a dead generation.

### Worker failures are values, not panics

Remove `expect` from runtime Worker post, drain, decode, and actor failure
paths. Transport failure, actor error, malformed response, timeout, and stale
generation response become typed executor outcomes.

Recoverable job failure must not destroy the integrated server. If bounded
actor reconstruction is exhausted, stop accepting unbounded commands, retain
the last coherent world state where possible, and surface a visible terminal
session error. A dead authority with a growing hidden command queue is not an
acceptable fallback.

### Failed render work returns to dirty state

Render completion handling must branch on success before consuming logical
work:

- success clears in-flight state and accepts revision-current output;
- stale success clears in-flight state and requeues any still-loaded current
  revision as already required;
- recoverable failure clears in-flight state and re-dirties every still-loaded
  target section at its current revision; and
- cancellation clears obsolete work without replay.

On Worker generation replacement, reset the input mirror and replay every
still-current interrupted compile. Existing valid GPU cache entries may remain
visible; a complete global mesh eviction is unnecessary. Readiness for an
affected column remains false until the replacement generation returns either
a valid build or an explicit valid-empty receipt.

### Settled means exact target coverage

Local `ViewSettledStatus::ready()` must require the exact target coverage
contract, not only empty queues. An all-air or otherwise mesh-empty column must
have an explicit accepted readiness receipt so exact coverage does not depend
on nonempty GPU geometry.

Expose a bounded list of missing target columns, their current state, last
attempt generation, and last failure reason in diagnostics. Hashes remain
useful for stable automated comparison, but a singleton hash should not need
offline inversion to reveal the coordinate.

### Unknown persistent records become continuations

Do not weaken revision-safe browser mutation. When a commit includes an address
whose current revision is unknown, return a typed read prerequisite, execute
that IndexedDB read outside actor admission, install either the observed record
or known-absent state, revalidate the batch, and retry the commit.

Generated chunk addresses must use this path automatically. A fixed bootstrap
key list is appropriate for bounded realm metadata but cannot be the solution
for an unbounded chunk namespace.

Already-produced ordered gameplay or chunk updates must not be discarded
because a detached autosave continuation fails. Persistence health and delivery
health remain separately diagnosable. Explicit flush and shutdown retain their
durable fence semantics.

## Required Diagnostics

Add the following bounded facts to native and Web normalized reports:

- actor kind, logical worker state, and active generation;
- physical construction, ready, termination, and reconstruction counts;
- restart reason and bounded restart history;
- in-flight logical jobs and attempts;
- oldest in-flight age, deadline, and last-progress age;
- submitted, completed, failed, timed-out, cancelled, replayed, stale, and
  accepted-result totals;
- current and high-water SAB capacity by actor;
- desired, acknowledged, and speculative retained-state counts/generations;
- queued, drained, runner-emitted, and client-applied snapshot/unload totals;
- requested, accepted, loaded, and exact drawable coverage;
- bounded missing-column coordinates and state; and
- whether readiness is blocked by authority, delivery, lighting, compilation,
  persistence, or exact draw admission.

Instrumentation must not clone complete chunk or retained-state sets per frame.
Keep fast counters and bounded samples in production diagnostics; detailed job
histories remain probe-only.

## Implementation Sequence

### Slice 0: Land deterministic failure probes

Create one scripted worker-liveness lane before changing behavior. It must use
the production Worker topology and provide smoke-only controls for:

- silent physical Worker stop;
- explicit actor exception;
- response suppression/no response;
- malformed response;
- delayed response from an obsolete generation; and
- forced SAB overflow or transport-post failure.

The real-browser control may ask the generic broker to terminate or fault a
specific physical Worker, but Rust selects the actor and owns the expected
semantic outcome. Do not expose a general production fault API.

Add an equivalent fault-injecting native executor that can disconnect, panic,
drop, delay, and reorder job results behind the same shared scheduler tests.

### Slice 1: Introduce the shared lifecycle contract

- add worker generation and attempt identity;
- retain logical jobs until terminal acceptance;
- map native disconnect/panic and browser errors/timeouts into typed outcomes;
- add the oldest-in-flight watchdog and visibility-aware resume policy;
- reject stale generation completions; and
- prove bounded replacement and terminal exhaustion independently of worldgen
  or lighting payload details.

Do not migrate browser bytes or native channels into a universal transport.
Share semantics, not mechanisms.

### Slice 2: Recover worldgen and lighting

- remove runtime `expect` paths;
- split desired, acknowledged, and speculative retained state;
- reconstruct each actor from authoritative state after generation loss;
- re-evaluate and replay still-current scheduler jobs;
- release failed-generation SAB slots and counters;
- apply current lighting unload state before replay; and
- prove exact target convergence after loss at idle and in flight.

Run the physical Android silent-close cases immediately after this slice and
inspect pixels before continuing.

### Slice 3: Make render recovery lossless

- requeue all current target sections from a failed compile;
- reset worker-side mirror state at generation replacement;
- preserve valid old cache entries while replacement work is pending;
- add valid-empty column receipts;
- require exact target coverage for settled state; and
- report bounded missing coordinates and failure provenance.

The controlled render-loss case must finish at `289 / 289`, not merely restart
the Worker or reach zero pending jobs.

### Slice 4: Complete generated-record persistence semantics

- return typed read prerequisites for unknown record addresses;
- resolve them through detached browser continuations;
- retry only after installing observed or known-absent revision state;
- preserve update delivery across autosave failure; and
- add first-write, evict-then-write, concurrent-revision, close/reopen, quota,
  flush, and shutdown tests for generated chunks.

### Slice 5: Cross-host closeout

Run the complete fault matrix, ordinary clean movement, persistence reload,
background/resume, and bounded memory soak on native desktop, headed desktop
Chrome, and physical Android Chrome. Record normalized semantic traces and
inspect browser pixels.

Update
[`../topics/web-worker-runtime-ownership.md`](../topics/web-worker-runtime-ownership.md)
with the landed lifecycle contract, measured retry policy, and final evidence.
Append implementation commits and exact deployed revision receipts to this
tactical.

## Acceptance Matrix

Every worldgen, lighting, and render executor must be tested at initialization,
idle, in-flight steady streaming, immediately after a radius increase, and
during cardinal movement.

| Fault | Native thread | Desktop Chrome | Physical Android Chrome |
|---|---:|---:|---:|
| silent stop/disconnect | required | required | required |
| explicit exception/panic | required | required | required |
| no response until deadline | required | required | required |
| malformed response | required | required | required |
| stale delayed response | required | required | required |
| buffer/post failure | required | required | required |
| hidden/background then resume | N/A | required | required |

Run both transient and IndexedDB worlds where persistence is relevant. At
least one browser lane must create the world and change Graphics settings
through real DOM pointer/touch input.

The canonical human-path movement sequence is:

1. create a fresh world;
2. raise render distance from 3 to 8;
3. move north three chunks;
4. move west three chunks;
5. fly upward 128 blocks;
6. move north another three chunks;
7. wait an ordinary maximum of 60 seconds; and
8. hold a stability window while checking exact counters and pixels.

For render distance 8, successful final acceptance requires:

```text
requested client coverage: 361 / 361
exact drawable coverage:   289 / 289
missing target columns:    0
server jobs/publications:  0
job and render mailboxes:  0
queued completed results:  0
requested view:            accepted view
snapshot delivery chain:   queued = drained = emitted = applied
unload delivery chain:     queued = drained = emitted = applied
```

After an injected recoverable fault, acceptance additionally requires:

- the affected worker generation increments;
- every failed logical job is completed, cancelled as obsolete, or represented
  by a visible terminal session failure;
- at least one replay occurs when the interrupted job remains current;
- stale old-generation results are rejected;
- all failed-generation SAB slots and pending counters return to zero;
- readiness never becomes true while exact target coverage is incomplete;
- no physical Worker from the retired generation remains after cleanup; and
- coverage remains exact through the stability window.

The physical Android route must converge within 60 seconds after movement
stops. Record faster phase targets from clean baselines, but do not disguise a
liveness failure by continually extending the timeout.

## Validation Commands

Exact command names may be introduced by Slice 0, but the closeout must include
at least:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-web-client \
  --target wasm32-unknown-unknown
pnpm native:web:typecheck
pnpm native:web:worker-ownership
pnpm native:web:scene-host-adoption
pnpm native:scheduler:smoke
pnpm native:movement:smoke
pnpm native:web:view-replay
pnpm native:web:cardinal-view-replay
pnpm native:web:indexeddb-smoke
pnpm native:web:worker-liveness
```

Browser fault lanes must run sequentially on a real GPU host. Save captures and
reports under `/tmp`, inspect every material pixel checkpoint, and repeat the
final deployed revision on the physical Android device. A desktop mobile
viewport is an additional deterministic lane, not a substitute for Android
Chrome.

## Non-Goals

- changing generation, lighting, or terrain results;
- reducing the user's chosen render distance;
- retaining authoritative-unloaded chunks or stale meshes to cover holes;
- disabling lighting or background work;
- increasing waits without detecting and recovering lost work;
- adding Web-only chunk-interest coalescing;
- adding more Workers as a throughput workaround;
- moving domain policy into TypeScript;
- introducing shared Wasm linear memory;
- adopting a broad async runtime; or
- claiming memory pressure as the spontaneous trigger without a clean
  controlled receipt.

## Stop Conditions

Stop and request a new decision if implementation requires:

- a persistence schema migration or weaker revision/durability contract;
- a shared Wasm heap or new browser Worker topology;
- changing native worldgen, lighting, render, or scheduler results to fit Web;
- replaying non-idempotent gameplay mutations without an acceptance identity;
- restarting the complete realm from durable storage and discarding newer
  in-memory state;
- weakening exact coverage, delivery-chain, shutdown, or pixel assertions; or
- treating repeated worker reconstruction as healthy steady-state behavior.

## Expected Commit Slices

Use `Topic: web-worker-runtime-ownership` for the implementation series.
Expected bounded commits are:

1. add shared native/Web worker fault probes and diagnostics;
2. add the shared lifecycle, generation, deadline, and replay contract;
3. recover worldgen and lighting actors without authority teardown;
4. requeue failed render work and make exact coverage load-bearing;
5. resolve unknown generated-record revisions through continuations; and
6. record physical Android, desktop Web, native, persistence, and deployed
   closeout evidence.

Do not mark this tactical complete because the clean route passes once. It is
complete only when every controlled failure reaches exact coverage or an
explicit terminal state, and the ordinary repeated phone route remains exact
without fault injection.
