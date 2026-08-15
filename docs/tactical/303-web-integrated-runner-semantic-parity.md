# Tactical 303: Web Integrated Runner Semantic Parity

Status: completed 2026-08-15

Topic: `web-worker-runtime-ownership`

## Instruction Synthesis

Correct the reproducible browser-only chunk disappearance and reappearance by
removing the Web integrated server's command-scoped background-work drain.
Use the correction to make browser Worker orchestration obey the native
integrated-runner semantics: commands perform one short authoritative state
transition, background jobs advance independently, browser persistence is an
asynchronous platform continuation, and readiness diagnostics describe the
work that is actually outstanding.

Do not mask the defect with renderer retention, delayed chunk unloads, a larger
view radius, or Web-only `SetChunkView` coalescing. Preserve the existing
isolated-Wasm actor topology, domain-blind TypeScript boundary, external
`SharedArrayBuffer` transports, IndexedDB storage contract, simulation
cadence, and shared scene/runtime owners.

## Starting Evidence

An interactive investigation on 2026-08-15 reproduced large terrain holes in
the browser while moving through an otherwise healthy local integrated world.
The renderer was consuming real `ChunkUnload` and later `ChunkSnapshot`
updates; it was not independently evicting visible meshes.

The focused trace accelerated movement until the camera reached chunk `(0, 8)`
and then stopped. The client retained exactly 49 loaded chunks throughout, but
their identities changed in eight delayed batches. Each batch contained nine
unloads and nine snapshots, and the complete stale-view replay took about 3.3
seconds after movement stopped. A second run at the ordinary fly speed exposed
the same visible holes. No browser console or WebGPU error accompanied either
run.

The command path explains the exact shape:

1. shared camera reconciliation submits `SetChunkView` whenever the camera
   enters a new chunk;
2. main-side Web Rust posts every command as a distinct fire-and-forget Worker
   request;
3. the Worker shell admits one operation and holds it open while Rust reports
   any worldgen, lighting, publication, or persistence work pending;
4. the shell repeatedly yields and polls until all of that work is quiescent;
5. only then may the next already-stale view command enter the actor; and
6. each accepted historical center legitimately computes a new visible set,
   sending unloads and snapshots that make terrain disappear and later return.

The native runner does not impose that barrier. It decodes one command, calls
`LocalRealmSession::try_handle_command`, publishes the immediate updates, and
returns to its command/tick loop. New interest can therefore supersede old
interest while scheduler jobs remain active.

The browser also has a diagnostic blind spot. Its authoritative
`command_queue_depth` is incremented and decremented entirely inside the
synchronous `handleCommandFrame` call. Requests waiting in the Worker event
loop are invisible, so `streamingIdle` can become true between stale command
responses. The smoke observer then records the current camera center as the
loaded center without proving that the server has accepted it.

The command-only drain first entered with commit `df821749` (`Move WASM server
jobs to workers`) and was deliberately preserved by commit `542b0f84` (`Move
web server authority into a Rust actor`). Tactical
[`197`](197-domain-blind-web-worker-broker.md) correctly moved ownership of the
policy into Rust, but preserved behavior that this new evidence shows was not
semantically equivalent to native.

## Correctness Target

Native threads and browser Workers retain different scheduling mechanisms,
but they must implement the same authoritative runner contract:

```text
command arrival
  -> decode and validate
  -> apply exactly one authoritative command transition
  -> publish only immediately available ordered updates
  -> complete command admission

timer/job/persistence completion
  -> enter a separate bounded runner step
  -> advance simulation or completed background work
  -> publish newly available ordered updates
  -> complete that step
```

The following invariants are binding:

1. Ordinary command completion never requires global scheduler, worldgen,
   lighting, publication, or persistence quiescence.
2. No admitted Web actor operation remains open while JavaScript awaits a
   timer, IndexedDB request, Worker result, or other browser promise.
3. Authoritative mutations and publications remain ordered even though their
   production spans multiple runner steps.
4. A newer chunk view can be accepted while work requested by an older view is
   still pending; existing scheduler cancellation, reprioritization, and
   pending-unload rescue remain responsible for obsolete work.
5. Queue diagnostics count every accepted-but-not-completed command, including
   requests waiting outside worker Rust.
6. Streaming readiness cannot become true until the authority's accepted view
   matches the latest locally requested view wherever that fact is available.
7. Explicit persistence flush, lifecycle retirement, and shutdown remain
   durable fences even though ordinary commands are nonblocking.

Timing equality is not required. The browser event loop, IndexedDB, external
SAB mailboxes, and independent Wasm heaps remain platform facts. Semantic
parity means equivalent command admission, ordering, background progress,
readiness, failure, and durability contracts.

## Binding Architecture Decisions

### Command admission is one short state transition

`McloneWebIntegratedServerWorker::handle_command_frame` must end after decoding
and applying the command and packaging immediately available updates. Remove
its unconditional `try_poll` and command-triggered autosave. The
`WebIntegratedServerActor` must not expose command-only `hasPendingJobs` or
`pollPendingJobs` operations, and its operation record must not carry a
pending-job poll counter or global drain limit.

Do not add a second shared wrapper whose only implementation is another call
to `LocalRealmSession::try_handle_command`. The existing method is already the
shared semantic owner. Instead, add direct contract tests that prevent either
host adapter from surrounding it with a command-scoped background drain.

The TypeScript Worker may retain a short browser-event exclusion guard around
one synchronous Rust actor transition. It may not hold that guard across an
`await` or use it to turn an ordinary command into a transaction over all
future work caused by that command.

### Background work advances through ordinary runner steps

Worldgen, lighting, scheduler publication, and gameplay simulation continue
through the established timer tick and completion paths. A timer tick may
process the bounded work already owned by the shared server tick. It must not
loop until all work is gone merely to manufacture a drained response.

This tactical does not select a new scheduler budget or change chunk-interest
math. One-chunk local unload hysteresis, keyed scheduler cancellation,
publication budgets, and render compiler admission remain intact. Their normal
behavior becomes observable once stale view commands stop acting as barriers.

### Browser persistence becomes an independent continuation

The current `servicePersistenceResultForCurrentWorld` keeps the actor operation
open while it executes IndexedDB requests and repeatedly feeds completions back
into Rust. Split that lifetime:

1. a command or tick returns its immediate updates and any opaque browser
   persistence requests;
2. the actor operation finishes and releases admission;
3. TypeScript executes the IndexedDB batch without holding a domain operation;
4. the resulting typed completions enter Rust as a new bounded persistence-
   completion operation; and
5. any follow-on requests repeat as later continuations rather than one long
   command or tick RPC.

Keep at most the existing bounded number of browser persistence batches in
flight. Rust continues to own request IDs, expected addresses, stale or
duplicate completion rejection, and the authoritative pending state.
TypeScript may execute records, carry opaque request/completion payloads, and
schedule the next browser callback; it must not interpret chunk, player,
dimension, or save policy.

Explicit flush and shutdown are allowed to wait for durable completion because
their contract is a fence. Ordinary gameplay commands and timer ticks are not.

Remove command-triggered autosave as part of the semantic correction. Audit
the remaining browser tick-driven save frequency against the shared lifecycle
save policy and bounded-persistence topic before changing it. Preserve current
durability unless a separate parity correction is supported by reload,
page-lifecycle, and shutdown evidence; record any intentional browser-only
durability cadence explicitly rather than hiding it in command handling.

### Queue depth and accepted view are authoritative facts

Main-side Web Rust already records request IDs from post until response. Add a
command-specific outstanding set or equivalent typed accounting and overlay it
into `ServerRunnerDiagnostics.command_queue_depth`. Generic runner-frame
metrics may continue to count all request kinds, but ordinary command depth
must not be inferred from an update queue or a transient worker-local counter.

Expose the local authority's actually accepted `ChunkView` from existing
`PlayerChunkTracking` state through shared runner diagnostics. Do not create a
gameplay acknowledgement packet or infer acceptance from the camera solely for
a smoke test. Hosts without authoritative local diagnostics may retain their
transport-specific readiness contract, but must not fabricate an accepted
center.

`WebSceneHost::streaming_idle` must require all of the following for a local
integrated session:

- outstanding command depth is zero;
- accepted view equals the scene's latest requested interest view;
- server jobs, publications, persistence, and update queues are idle;
- scene stream work is empty; and
- render Worker requests and render compile jobs are empty.

The smoke observer's `loadedCenterX/Z` must come from that accepted view. If the
fact is unavailable, report it as unavailable instead of copying the camera
center.

### Do not introduce Web-only view coalescing

The first correction processes every command in order, just as native does.
Removing the global drain should make view admission cheap enough that the
Worker remains current under ordinary movement.

After the parity implementation, measure command depth and accepted-view lag.
If a meaningful backlog remains, stop and write a separate shared tactical for
latest-value chunk-interest semantics. Any future supersession must be
Rust-owned, apply consistently to native and Web local runners, preserve
ordering barriers around incompatible commands and lifecycle transitions, and
have exact protocol tests. Do not add a main-side Web-only replacement slot in
this tactical.

### Preserve the domain-blind Worker boundary

TypeScript may continue to own Worker construction, message callbacks,
`setTimeout`, IndexedDB transactions, SAB views, transfer lists, promise
settlement, browser failure forwarding, and final Worker close. Rust owns
operation kinds, request identity, admission, authoritative state,
supersession, budgets, diagnostics meaning, persistence completion validation,
and graceful shutdown.

Update `scripts/check-web-worker-ownership.mjs` so the deleted command-drain
policy cannot return in either TypeScript or browser-specific Rust. The gate
should reject command-scoped pending-job loops and actor operations held over a
browser yield without forbidding ordinary bounded tick or persistence work.

## Implementation Sequence

### Slice 0: Land the failing semantic probes

Add a deterministic browser movement probe before changing production
behavior. It must drive at least these paths against one fixed transient world:

- ground movement across multiple chunk boundaries under active streaming;
- ordinary fly speed forward and then back across at least four boundaries;
- accelerated travel across at least eight boundaries to amplify backlog; and
- a turn or reversal that would make stale historical views remove and later
  re-add the same chunks.

Record requested camera view, authority-accepted view, outstanding command
depth, server pending work, loaded chunk identities or a stable set hash,
snapshot/unload totals, render section count, and `streamingIdle` on each
transition. Save diagnostic screenshots under `/tmp` and inspect the first
visible failure; do not add captures to the repository.

Add a focused server/runner contract fixture that leaves worldgen or lighting
pending after one view command, submits a second view, and proves the second
view can be accepted without draining the first view's jobs. Use the same
normalized assertions for native and the worker-resident Rust actor wherever
the host boundary permits; retain browser execution for the actual event-loop
contract.

### Slice 1: Remove command-scoped quiescence

- simplify `WebIntegratedServerOperation` to operation kind and request ID;
- delete command-only pending-job predicates, poll exports, and drain limit;
- finish the actor response after the initial command result;
- remove `try_poll` and autosave from `handle_command_frame`;
- ensure the periodic tick continues to publish job completions; and
- update ownership locks and focused actor tests atomically.

Run the transient movement probe immediately and inspect pixels before
continuing. The accelerated path must no longer replay one settled view per
historical camera center after motion stops.

### Slice 2: Detach browser persistence waits

- represent IndexedDB request batches as external effects of a completed actor
  step;
- execute browser I/O only after releasing actor admission;
- admit typed completion batches as separate short actor operations;
- preserve exact request validation and ordered update publication;
- retain bounded continuation/failure handling without a global job drain;
- preserve explicit flush and shutdown durability fences; and
- prove transient and IndexedDB modes use the same command semantics.

The IndexedDB reload probe must demonstrate that edits and player/world facts
survive close and reopen. Injected read/write failure must remain typed and
must not leave the actor permanently admitted or falsely idle.

### Slice 3: Repair diagnostics and readiness

- count posted-but-uncompleted command requests in main-side Web Rust;
- publish the actual accepted local chunk view in runner/runtime diagnostics;
- make local streaming readiness compare requested and accepted views;
- remove the smoke observer's camera-as-loaded-center inference;
- expose accepted-view lag and maximum command depth in the movement report;
  and
- add false-idle regression assertions around movement and reversal.

Keep detailed diagnostics bounded and sampled according to the existing
performance topic. The fast facts required for readiness must remain cheap and
must not require cloning complete chunk sets every frame.

### Slice 4: Parity closeout

Run the full matrix below, compare native and Web normalized semantic traces,
and inspect final browser pixels. Measure after correctness is green. If Web
command admission stays current under accelerated movement, close this
tactical without coalescing. If it does not, report the residual depth/latency
and open a separate shared-interest tactical rather than extending this one.

Update [`../topics/web-worker-runtime-ownership.md`](../topics/web-worker-runtime-ownership.md)
with the landed contract, evidence, and any explicit persistence cadence
exception. The completed tactical becomes the execution record.

## Acceptance Gates

### Shared semantic contract

- Native and Web apply a command through one `try_handle_command` transition
  and do not require background quiescence before admitting the next command.
- A focused trace proves a second view is accepted while jobs from the first
  remain pending.
- Immediate command updates precede later tick/completion updates in both
  hosts.
- No command or tick actor operation spans a browser `await`.
- Explicit flush and shutdown still acknowledge only after their documented
  durable fence.

### Movement and chunk lifecycle

- Requested and accepted views converge before local streaming readiness.
- `streamingIdle` is never true while commands are outstanding or the accepted
  view differs from the requested view.
- After movement stops, no sequence of historical accepted centers continues
  to generate alternating unload/snapshot batches.
- Ground, ordinary fly, accelerated fly, and reversal probes retain bounded
  loaded/resident/render work and finish with coherent terrain.
- The existing one-chunk unload hysteresis and exact server visible-set tests
  remain unchanged.

### Persistence and failure

- Transient and IndexedDB worlds have the same nonblocking command semantics.
- IndexedDB requests execute outside actor admission and their completions are
  validated by Rust.
- Mutation/reload, explicit flush, graceful world replacement, writer-lease
  release, and shutdown preserve their existing durable facts.
- Worker, persistence, and codec failures release admission, surface a typed
  failure, and do not report a drained transport.

### Ownership and performance

- TypeScript remains domain-blind and no command-job predicate or gameplay
  scheduler policy moves into the Worker shell.
- No new Worker, Wasm heap, SAB ABI, persistence schema, copy site, or
  platform-local gameplay owner is introduced.
- Maximum outstanding command depth, accepted-view lag, frame gaps, worldgen
  and light pending work, update bytes, and render compile work are reported for
  the before/after accelerated trace.
- Correctness does not depend on a larger radius, longer unload delay, reduced
  movement speed, disabled lighting, reduced generation, or retained stale
  renderer geometry.

## Validation Matrix

Run the smallest focused gates after each slice, then the complete affected
matrix from [`../platforms.md`](../platforms.md#validation-policy):

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
pnpm native:web:typecheck
pnpm native:web:worker-ownership
pnpm native:web:scene-host-adoption
pnpm native:scheduler:smoke
pnpm native:movement:smoke
pnpm native:web:movement-perf
pnpm native:web:indexeddb-smoke
pnpm native:web:lobby-runtime-smoke
```

Before browser/WebGPU capture on Linux, run `pnpm host:check` and use the
headed Wayland lane required by the project instructions. Run GPU browser lanes
sequentially. Capture screenshots under `/tmp`, inspect them, and include the
exact commands, revision, requested/accepted view trace, queue maxima, update
counts, and image paths in the tactical execution record.

Exercise both external SAB and message-transfer runner transports. The
movement regression must run against the production worker path, not the
inline fallback. Repeat the native movement lane to ensure the shared
diagnostic or runner-contract changes did not alter native command ordering or
chunk lifecycle.

## Non-Goals

- changing chunk tracking radius, unload hysteresis, or server visible-set
  mathematics;
- retaining renderer meshes after an authoritative unload;
- Web-only `SetChunkView` coalescing or dropped protocol commands;
- replacing private Worker Wasm heaps with shared Wasm memory;
- moving IndexedDB, Worker, timer, DOM, input, WebGPU, or presentation
  mechanics into shared engine crates;
- changing worldgen, lighting, terrain, renderer, gameplay, or simulation
  results to make the probe pass;
- weakening persistence durability, writer-lease, shutdown, lifecycle, pixel,
  or failure assertions; or
- deploying a public build before the local semantic and pixel matrix is
  complete and explicitly requested.

## Stop Conditions

Stop and request a new decision if the correction requires:

- a persistence schema migration or weaker durability contract;
- a new Worker, shared Wasm heap, SAB ABI, or transport copy strategy;
- a protocol-visible view revision solely to repair local diagnostics;
- changing native command ordering to accommodate a browser mechanism;
- a broad general-purpose async runtime rather than bounded actor
  continuations; or
- weakening any accepted movement, IndexedDB reload, lifecycle, ownership, or
  rendered-output gate.

## Execution Record

Completed on 2026-08-15 without changing the Worker count, private-Wasm-heap
topology, external SAB ABI, persistence schema, simulation cadence, chunk
radius, unload hysteresis, gameplay results, or renderer retention policy.
No Web-only view coalescing was added.

### Landed contract

- `handle_command_frame` now performs one `try_handle_command` transition and
  returns its immediately available ordered updates. It no longer polls the
  scheduler or triggers autosave.
- The Worker actor no longer exposes command-scoped pending-job predicates,
  pending-job polls, yields, or drain limits. Periodic ticks and Worker job
  completions advance background work independently.
- Ordinary command and tick operations finish before IndexedDB begins.
  Browser record results enter Rust as separate typed persistence-completion
  operations. Explicit flush and shutdown still wait for durable completion.
- Main-side Web Rust retains every posted command request ID until its response
  arrives, so command depth includes messages waiting in the Worker event
  loop.
- Shared server and app-runtime diagnostics expose the accepted local
  `ChunkView`. Local streaming readiness requires the camera chunk, requested
  view, accepted view, server queues/jobs, scene stream work, and render work
  all to agree or be empty.
- The browser observer derives its loaded center only from accepted authority
  diagnostics and reports a stable hash of the loaded chunk identities.
- A source gate rejects reintroduction of command-scoped job drains or browser
  yields inside an ordinary actor operation.

The browser's existing tick-driven autosave cadence remains intentionally
unchanged. The retired command-triggered autosave was Web-only and was part of
the incorrect command transaction. Persistence schema, record validation,
request IDs, world-keyed writer leases, failure kinds, and shutdown durability
remain owned by the existing Tactical 199 boundary.

### Commits

- `ce47bb1c` — make Web server commands nonblocking;
- `f2b703a4` — detach Web persistence continuations;
- `f5ab3990` — make Web streaming readiness authoritative;
- `2f94707e` — gate Web readiness across deterministic view replay;
- `7bffe3b8` — lock nonblocking chunk-view admission in shared tests;
- `2d82f49a` — prove shared-memory and message-transfer runner parity;
- `f59e2a5e` — repair the exact persistence acceptance fixture; and
- `703fca5f` — restore the complete Web smoke typecheck gate.

All implementation commits use `Topic: web-worker-runtime-ownership`.

### Movement and transport evidence

`pnpm native:web:view-replay` ran the production integrated-server Worker and
passed all four required movement shapes:

- ground movement crossed two chunk boundaries;
- ordinary flight crossed four boundaries forward and four in reverse;
- accelerated flight crossed eight boundaries; and
- the accelerated path immediately reversed across eight boundaries before
  waiting for quiescence.

The 77 sampled transitions observed maximum outstanding command depth `2`,
maximum accepted-view lag `1` chunk, and zero false-idle samples. The final
requested, accepted, and loaded centers were all `(-2, 0)`. The scene retained
49 loaded chunks and 159 resident sections. Loaded-set hash
`44240cd7288b30cb`, 279 snapshot updates, and 216 unload updates remained exact
for the complete 1.5-second post-idle stability window. The final measured
maximum frame gap was `9.255 ms`.

The same lane ran fresh isolated transport actors after movement. The external
SAB runner processed six commands, reached maximum four pending frames,
exercised pool miss/drop/reuse, and settled. The separate message-transfer
runner processed its command and settled. Both reported `ok` with their exact
transport kinds and transitioned to final non-running diagnostics after
shutdown requests.

The final 1280 by 720 canvas contained 905,016 non-clear interior pixels and
1,384 distinct interior colors. The inspected capture showed coherent grass,
stone, and vegetation terrain without a missing-chunk hole. Evidence:

- [page capture](/tmp/mclone-native-web-view-replay.png)
- [canvas capture](/tmp/mclone-native-web-view-replay-canvas.png)
- [movement report](/tmp/mclone-native-web-view-replay.json)

### Persistence and lifecycle evidence

`pnpm native:web:indexeddb-smoke` passed the detached continuation and durable
fence boundary. Exact placed state `42` survived close and reopen, the
successful-placement statistic advanced from `0` to `1`, day time advanced
from `87` to `105`, and the reopened world contained 121 chunk records plus
one metadata record. The generic record executor passed.

The live world retained its Web Lock: direct and complete session contenders
both observed same-world rejection. A different IndexedDB world started and
reported clean shutdown. The Chromium quota injection returned the typed
`quota` failure without poisoning the live world. Evidence is recorded in the
[IndexedDB reload report](/tmp/mclone-native-web-indexeddb-reload-probe.json).

### Validation closeout

The complete affected matrix passed through `703fca5f`:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
pnpm native:web:typecheck
pnpm native:web:worker-ownership
pnpm native:web:scene-host-adoption
pnpm native:scheduler:smoke
pnpm native:movement:smoke
pnpm native:web:movement-perf
pnpm native:web:indexeddb-smoke
pnpm native:web:lobby-runtime-smoke
pnpm native:web:view-replay
```

One first full `mclone-server` run observed the existing homestead resident
test fail while the other 732 tests passed. The exact test then passed alone,
and the complete 733-test suite passed on immediate rerun without a source
change. It is retained here as flaky baseline evidence rather than hidden.
