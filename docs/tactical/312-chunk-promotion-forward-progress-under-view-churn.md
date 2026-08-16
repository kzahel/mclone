# Tactical 312: Chunk Promotion Forward Progress Under View Churn

Status: in progress 2026-08-16; Slices 0-3 implementation and native/headed-
Web convergence gates complete; physical Android and memory closeout pending

Topic: `chunk-lighting-admission-and-backpressure`

## Instruction Synthesis

Correct the ordinary patchy-terrain failure reproduced in mobile Chrome after
creating a world, raising render distance, and moving across several chunks.
Preserve the shared four-at-a-time Player-promotion and bounded Light ownership
policy while making cancellation, re-entry, and rapid view churn guarantee
forward progress on native threads and browser Workers.

Do not attribute the clean-path failure to Worker death: the physical
reproduction completed every submitted worldgen and Light Worker request. Do
not mask it by increasing the promotion limit, reducing render distance,
retaining stale terrain, disabling lighting, adding Web-only resubmission, or
waiting longer. The fix belongs in the shared authoritative scheduler and must
retain the memory bounds accepted by Tactical
[`279`](279-chunk-lighting-admission-and-backpressure.md).

This tactical is separate from planned Tactical
[`311`](311-cross-platform-worker-liveness-and-replay.md). Tactical 311 owns
physical Worker loss, deadlines, reconstruction, and replay. Tactical 312 owns
healthy-executor scheduler progress when interest cancellation and re-entry
leave no Worker operation in flight. Implement this tactical first so Worker
recovery is not credited with fixing an unrelated ordinary-path stall.

## Evidence Boundary

The investigation used the deployed public build and a physical unlocked
Pixel 7a. It created a persistent world through the real title-screen flow,
changed Graphics render distance from 3 to 8 through real touch input, switched
to Fly, ascended, and moved approximately 12 to 18 chunks across multiple
cardinal directions. No Worker was stopped, faulted, delayed, replaced, or
otherwise manipulated.

Repository source was not changed during the reproduction. Chrome DevTools
Protocol was used only to read diagnostics and briefly inspect an ordinary
integrated-server tick after the plateau was already stable. The probe tab and
ADB forwarding were closed afterward.

The deployed revision was:

```text
0f0878067bd6-20260816091019
```

The natural reproduction is recorded in:

- [physical Android capture](/tmp/mclone-natural-rd8-stall.png)

Files under `/tmp` are investigation artifacts, not repository inputs or
durable acceptance evidence. The final implementation must produce a fresh
capture and structured report from the exact candidate revision.

## What Is Proven

### The ordinary phone route stalls without a Worker failure

The fresh world first settled exactly at the default browser radius:

```text
requested client coverage: 81 / 81
exact drawable coverage:   49 / 49
```

After changing to render distance 8, the same session also reached a healthy
exact baseline:

```text
requested client coverage: 361 / 361
exact drawable coverage:   289 / 289
```

After multi-direction movement and then releasing all input, the session
plateaued unchanged for the complete 90-second observation:

```text
accepted center:                    (7, 11)
requested client coverage:          265 / 361
server-ready coverage:              265 / 361
server-published visible coverage:  265 / 361
server missing published chunks:     96
exact drawable coverage:            197 / 289
missing exact columns:               92
server aggregate pending jobs:      171
server pending publications:          0
worldgen mailbox pending:              0
Light mailbox pending:                 0
render compile requests pending:       0
runner last error:                  empty
```

The authority, client, and renderer delivery chains were not hiding queued
payloads:

```text
snapshot queued/drained/emitted/applied: 854 / 854 / 854 / 854
unload queued/drained/emitted/applied:   758 / 758 / 758 / 758
server player outbound queue depth:       0
```

The page remained visible and the ordinary loops continued advancing. A
five-second comparison observed roughly 300 additional app frames and five
additional integrated-runner request/response exchanges, while the 171
aggregate pending jobs and coverage counts remained unchanged.

### Every submitted job-Worker request completed

The live worldgen Worker reported:

```text
requests / inbound responses: 134 / 134
mailbox pending:               0
transport:                     shared-memory
```

The live Light Worker reported:

```text
requests / inbound responses: 194 / 194
mailbox pending:               0
transport:                     shared-memory
```

Both physical Worker targets remained present. The render Worker remained at
generation 1 and had no request in flight. The integrated runner reported no
error and the main-side update pump reported `updatePumpStalled=false`.

This disproves Worker disappearance, an unmatched Worker response, authority
teardown, renderer retry, or sleeping browser cadence as necessary causes of
this reproduction. Tactical 311's controlled Worker-loss defects remain real,
but they do not explain this clean-path receipt.

### The aggregate pending count is not a Worker-job count

`ChunkScheduler::pending_job_count()` sums several distinct owners:

- queued Player promotions;
- active Player promotions;
- queued or running feature-status jobs;
- authored misses where applicable;
- pending Light demands;
- Light mailbox ownership; and
- pending Light publications.

The production Web report exposes only the aggregate plus mailbox and
publication counts. It does not expose the existing
`player_promotion_queued`, `player_promotion_active`, oldest-age, pending-Light-
demand, or live feature-job breakdown. Therefore `171` must not be described
as 171 stuck Worker requests.

## Source-Correlated Forward-Progress Hole

The source contains a complete cancellation/re-entry sequence capable of
creating the observed empty-mailbox plateau.

### 1. Player promotion has four completion-gated slots

`ChunkDistanceManager` admits at most
`DEFAULT_MAX_ACTIVE_PLAYER_PROMOTIONS = 4`. Remaining current player-view
positions stay in compact prioritized intent. A slot is refilled only through
`complete_player_promotion`, normally after the selected holder publishes a
client-ready Light snapshot.

This bound is intentional. It prevented sustained travel from converting the
complete requested view into unbounded Features and Light ownership on Quest.

### 2. Obsolete Light cancellation removes the only executable request

`ChunkScheduler::cancel_obsolete_initial_light_requests` finds Light tokens
whose positions are no longer runtime-required. `cancel_initial_light_request`
then:

1. removes the matching keyed `PendingLightDemand`, if it has not entered the
   mailbox;
2. asks the mailbox to cancel the token, if it has entered the mailbox;
3. removes the token from `active_light_tickets`;
4. clears the holder's `light_request_token`; and
5. releases the temporary Light ticket.

This correctly bounds obsolete work, but it does not transition the holder's
`ChunkStatus::Light` slot away from `ChunkStatusStep::Scheduled`. Cancellation
can therefore leave:

```text
Light status slot: Scheduled
Light request token: none
pending Light demand: none
Light mailbox request: none
pending Light publication: none
```

If cancellation removes a demand before mailbox admission, its Feature
snapshot and scheduled block/fluid tick record context are dropped with that
demand as well.

### 3. Re-entry treats slot existence as proof that scheduling already happened

`ChunkScheduler::enqueue_runtime_chunks` walks the status path for each runtime
target. If a status slot already exists, it normally continues without
rescheduling that status. It has a special repair path for a Features slot
that is `Scheduled` without a job ID, but no equivalent repair path for a
Light slot that is `Scheduled` without a live Light token or owned request.

A cancelled holder that remains resident as a dependency, survives pending-
unload processing, or is rescued by quick re-entry can therefore become
runtime-required again without recreating its Light demand. It is neither
ready nor represented in a Worker mailbox.

### 4. Four orphaned holders can stop all further promotion

If an orphaned holder is selected as an active Player promotion, it cannot
produce a client-ready snapshot. Its promotion slot is not released. Four
such positions can occupy the complete admission allowance while every
remaining position stays queued. The scheduler continues ticking and reports
promotion intent as aggregate pending work, but no worldgen or Light Worker
operation exists to advance it.

This source sequence matches all currently visible facts: nonzero aggregate
pending work, empty job mailboxes, no pending publication, live Workers,
healthy cadence, and permanently incomplete server coverage.

The deployed report does not expose the four active positions or their exact
blockers. The first implementation slice must capture that final per-position
receipt before changing behavior. Until then, this is a source-proven
forward-progress hole and the leading explanation of the natural plateau, not
a claim that the omitted live counters were somehow observed.

## Slice 0 Execution Evidence

Slice 0 adds a zero-default diagnostic delay immediately before shared initial-
Light mailbox admission. The delay is measured in scheduler polls rather than
wall time: it does not sleep or stop a thread or Worker, and native and WASM
execute the same delayed-demand, cancellation, and re-entry state machine.
Production behavior is unchanged unless
`debug_light_admission_delay_ticks`/`debugLightAdmissionDelayTicks` is
explicitly selected.

The scheduler diagnostics now split Player promotion into desired, queued,
active, oldest active age, and bounded active-blocker classes. They also count
all Light `Scheduled` slots without a token and, separately, those exact
orphans occupying active Player-promotion slots. The focused shared test
constructs a delayed Light demand, cancels it through ordinary interest
change, re-enters its still-resident holder, and proves the terminal blocker.

The native deterministic probe runs at render distance 8 with four promotion
slots and an 80-poll Light-admission delay. After a far-and-return view cycle,
187 stationary polls produced this stable pre-fix receipt:

```text
requested chunks:                         361
client-visible chunks:                     14
Player promotions queued / active:      357 / 4
active Light Scheduled without token:       4
pending publications:                       0
worldgen / Light mailbox pending:        0 / 0
```

The structured artifact is
[`/tmp/mclone-native-promotion-churn.json`](/tmp/mclone-native-promotion-churn.json).
`pnpm native:scheduler:promotion-churn` intentionally exits unsuccessfully
while the pre-fix view cannot converge.

The headed Chrome probe applies the same 80-poll shared delay through the real
integrated-server and retained worldgen/Light Worker topology. An explicit
smoke-only camera jump widens the interest-change window; it does not kill,
pause, replace, or bypass a Worker. The probe waited for all four delayed
demands, moved 32 chunks away, immediately returned to the original RD8 view,
and then held still for 30 seconds plus a stability window. It reproduced:

```text
requested / server-ready chunks:        361 / 172
exact ready / expected columns:         112 / 289
Player promotions queued / active:      236 / 4
active Light Scheduled without token:         4
Light demand / worldgen mailbox / Light mailbox: 0 / 0 / 0
pending publications:                         0
runner error / page errors:              empty / 0
```

All four blocker counts and the queued count remained unchanged through the
stationary window. The browser report and inspected capture are
[`/tmp/mclone-native-web-promotion-churn.json`](/tmp/mclone-native-web-promotion-churn.json)
and
[`/tmp/mclone-native-web-promotion-churn-canvas.png`](/tmp/mclone-native-web-promotion-churn-canvas.png).
The probe shuts down both browser Workers cleanly after recording the receipt.

This closes the earlier evidence gap: both native execution and the real WASM
Worker topology prove that four active promotions can be Light `Scheduled`
with no token or executable mailbox work. Slice 1 may proceed against the
source-correlated cancellation/re-entry blocker. The delay and smoke jump are
window-widening controls, not explanations of the natural phone failure; the
unchanged production build already supplied that ordinary-route evidence.

## Slice 1 Execution Evidence

The shared scheduler now owns one revision-keyed restart record from Features
publication until matching Light publication or authoritative holder teardown.
The holder remains the canonical owner of the Features snapshot; the restart
record owns only shared block/fluid scheduled-tick metadata and source-job
correlation. Queued, native-thread, and browser-Worker requests share that
metadata rather than moving its only copy into a cancellable demand.

Cancellation now releases the obsolete token and executor ownership while
leaving the matching restart record explicitly deferred. Ordinary ticket
reconciliation turns an eligible deferred record into a fresh revision-bound
token and keyed Light demand. Matching publication consumes the record once;
unload and deliberate lighting disable release it. A late result retains the
existing token/revision checks and cannot satisfy a replacement request.

Focused tests prove pre-mailbox cancellation and same-holder re-entry preserve
the exact block/fluid tick vectors, allocate a different token, and no longer
leave `Scheduled` without an owner. A matching publication test proves the
context and its byte accounting return to zero and the cached Light record
contains each scheduled-tick vector exactly once. The Light Worker frame codec
round trips the shared metadata on the browser boundary.

Slice 2 remains responsible for repairing an inconsistent bare orphan that
does not have a restart record, classifying every required holder on ordinary
polls, and proving the complete four-slot forward-progress matrix.

## Slice 2 Execution Evidence

One shared classifier now distinguishes prerequisite, deferred, queued-demand,
mailbox-owned, publication-pending, ready, scheduled-without-context, and
token-without-owner states. Ordinary scheduler polls and ticket reconciliation
audit every currently runtime-required holder. Deferred work is restarted in
current-center priority order, while a token that has no demand, mailbox, or
publication owner is cancelled and replaced through the same path.

The defensive bare-scheduled repair deliberately marks persistence metadata
incomplete. It may restore live Light progress, but successful publication
does not overwrite the earlier canonical Features cache record with invented
empty tick vectors. This preserves data rather than silently discarding tick
metadata in an invariant-violation path. Normal Features publication always
installs complete metadata before its first request.

Incremental diagnostics now separate explicit deferred count, restart-context
count/bytes, retries, repairs, and metadata-incomplete repairs from queued
demands and physical executor ownership. The compatibility pending-job total
includes deferred work, but repair never uses that aggregate as a Worker-health
signal.

Focused native tests cover cancellation before mailbox admission, cancellation
after mailbox admission, late completion racing a replacement token, quick
re-entry before unload, teardown plus re-entry after unload, newer Features
revision replacement, missing-owner repair, bare-scheduled repair, and all four
bounded Player-promotion slots becoming productive in one reconciliation.
All 45 scheduler tests pass. Slice 3 converts the deterministic native and Web
reproduction probes into bounded convergence gates and carries the normalized
metrics through their reports.

## Slice 3 Execution Evidence

The native reproduction is now a positive convergence gate. With the same RD8
view, four-promotion cap, and 80-poll admission delay, re-entry immediately
owned four queued Light demands rather than four inert Scheduled slots. It
completed 361/361 client-visible chunks in 1,011 settle polls with four retries
and zero active/queued promotions, deferred work, contexts, Light tickets,
scheduled-without-context holders, jobs, publications, or mailbox work. The
receipt is
[`/tmp/mclone-native-promotion-churn-fixed.json`](/tmp/mclone-native-promotion-churn-fixed.json).

The normalized diagnostics now cross the app-runtime and browser Worker
boundaries: active deferred blockers, restart-context count/bytes, deferred
count, retry/repair totals, and incomplete-metadata repair count appear beside
the established promotion and physical mailbox metrics.

The real headed Chrome/WebGPU gate uses the integrated-server Worker plus
worldgen and Light actors. After four delayed promotions were cancelled by the
far-and-return cycle, re-entry reported executable demand ownership and nine
retries. It converged in 10.94 seconds, then held unchanged loaded/exact hashes
for 60.00 seconds:

```text
requested server-ready coverage: 361 / 361
exact drawable coverage:         289 / 289
promotion queued / active:         0 / 0
Light deferred / demand:           0 / 0
restart contexts / tickets:        0 / 0
scheduled without context:         0
jobs / publications:               0 / 0
worldgen / Light mailbox:           0 / 0
delivery / compile target work:     0 / 0
```

The report and inspected mobile-size capture are
[`/tmp/mclone-native-web-promotion-churn.json`](/tmp/mclone-native-web-promotion-churn.json)
and
[`/tmp/mclone-native-web-promotion-churn-canvas.png`](/tmp/mclone-native-web-promotion-churn-canvas.png).
There were no page errors, and both browser Workers shut down cleanly.

The ordinary zero-delay cardinal route also passed after changing render
distance through the native UI, flying north three chunks, west three, climbing
128 blocks, flying north three more, reversing two, and waiting 30 seconds.
It ended at 361/361 server-ready and 289/289 exact coverage with zero pending,
deferred, demand, orphan, mailbox, publication, delivery, or compile work. Its
report and inspected capture are
[`/tmp/mclone-native-web-cardinal-view-replay.json`](/tmp/mclone-native-web-cardinal-view-replay.json)
and
[`/tmp/mclone-native-web-cardinal-view-replay-canvas.png`](/tmp/mclone-native-web-cardinal-view-replay-canvas.png).

Cancelled chunks that remain dependency-resident may retain a tiny dormant
restart record (seven records / 336 bytes in that route). They are not required
deferred jobs, do not contribute to pending work or scheduled-without-context
counts, and remain bounded by holder eviction. Slice 4 must confirm that bound
under the existing movement soak and physical Android route.

## Why Existing Tests Passed

The established scheduler movement smoke advances one chunk and then polls
until both `pending_job_count()` and `pending_publication_count()` reach zero
before moving again. That is valuable settled-throughput and memory evidence,
but it deliberately excludes view churn while promotion and Light work are
active.

The accepted Quest soak moves continuously, proves bounded memory, and waits
for final convergence. Its diagnostics prove the promotion cap and
cancellation totals, but its route and sampling do not assert that every
cancelled-and-reentered holder retains executable Light ownership. One long
route can also continually cancel stuck tail positions before they occupy the
stationary final view.

The Web cardinal-view replay checks requested versus accepted centers and
delivery ordering. It does not currently require a rapid non-quiescing
cancellation/re-entry cycle followed by exact stationary convergence on a
physical browser.

The missing test is therefore not another healthy settled load. It is a
shared scheduler state-machine test that changes interest again before the
previous view becomes Light-ready, revisits cancelled positions while their
holders remain resident, then demands bounded convergence after movement
stops.

## Correctness Target

Every current Player-promotion position must satisfy exactly one observable
state:

```text
Queued
  -> ActiveWaitingForStorage
  -> ActiveWaitingForFeatures
  -> ActiveWaitingForLightPrerequisite
  -> ActiveLightDeferred
  -> ActiveLightQueued
  -> ActiveLightInFlight
  -> ActiveLightCompletedPendingPublication
  -> Ready

or

Active* -> CancelledBecauseInterestDeparted
```

The names need not become one public enum, but the represented states and
transitions must be unambiguous. In particular, `Scheduled` is not evidence of
progress unless the status has an identifiable prerequisite or live executor
owner.

Binding invariants are:

1. An active promotion that is not client-ready has a specific blocker and a
   live path that can change that blocker.
2. A Light-scheduled holder has exactly one current request token or is
   explicitly deferred with sufficient retained context to create one.
3. A token belongs to exactly one of queued demand, mailbox ownership, or
   completed pending publication, except during one bounded atomic handoff.
4. Cancelling obsolete work preserves enough canonical Feature-stage context
   to restart Light if the same holder becomes required again.
5. Re-entry repairs deferred or orphaned required Light state during ordinary
   reconciliation; it does not depend on a new worldgen completion.
6. A promotion slot is released only for readiness or explicit obsolete-
   interest cancellation. Repair must not falsely mark a chunk ready or drop
   current intent.
7. After input stops and executors remain healthy, current requested coverage
   converges or diagnostics identify a terminal error. Indefinite inert
   pending state is invalid.
8. Native and Web use the same scheduler states, repair rules, ordering, and
   acceptance criteria. Their physical executors may differ.

## Binding Architecture Decisions

### Preserve the four-promotion bound

Do not raise the admission count to make an orphan less likely to occupy every
slot. That would weaken the memory correction physically accepted in Tactical
279 and would only postpone the same liveness failure at a larger footprint.

The fix must make each bounded slot productive or explicitly cancellable.
Throughput tuning is a later measured decision after correctness passes.

### Cancellation creates deferred work, not a fake scheduled operation

Cancellation must make the difference between these facts explicit:

- the holder still requires `ChunkStatus::Light` eventually;
- the old token is obsolete and owns no executor work; and
- the holder is not currently Light-ready.

The implementation may add an explicit deferred-Light state or derive it from
a stricter combination of holder facts. It may not leave a bare Scheduled slot
that suppresses future scheduling.

When current interest re-requires the position, reconciliation must construct
a fresh revision-bound token and keyed demand from the current Feature
snapshot. A response from the cancelled token remains stale and cannot satisfy
the replacement request.

### Retain canonical restart context with the holder lifecycle

`PendingLightDemand` currently owns the Feature snapshot plus scheduled block
and fluid tick record vectors needed by the Light publication/save path. A
pre-admission cancellation destroys that carrier.

Move or copy only the minimal canonical restart context into a holder-owned or
equivalent scheduler-owned Feature-to-Light record whose lifetime spans
cancellation and re-entry. Consume or release it when:

- a matching Light result publishes successfully;
- the holder is authoritatively unloaded after its save contract is handled;
- a newer Feature revision supersedes it; or
- lighting is deliberately disabled under the existing shared policy.

Do not duplicate scheduled gameplay tick admission on retry. Those events are
already emitted when Features publishes. The retained vectors exist to
preserve canonical record materialization, not to execute the same ticks a
second time.

### Reconciliation owns deterministic repair

Add one shared scheduler operation that validates Light progress for every
runtime-required Features-ready holder. It must distinguish:

- Light already ready;
- current token queued as demand;
- current token owned by the mailbox;
- current token completed and awaiting publication;
- explicit prerequisite not yet ready;
- deferred work eligible for a new request; and
- inconsistent orphan state.

Deferred work should be admitted through the ordinary keyed Light-demand
budget and current-center priority. An inconsistent orphan should be repaired
to the same path and increment a diagnostic counter; debug/test builds should
also assert the violated invariant at the smallest safe boundary.

Do not use a wall-clock timeout to repair this deterministic state hole.
Timeout and generation reconstruction belong to Tactical 311 when a live
executor owns work but stops responding.

### Promotion diagnostics must expose the blocker

Carry the existing promotion diagnostics through normalized native and Web
reports:

- desired, queued, active, and maximum active promotions;
- admitted and cancelled totals;
- oldest queued and oldest active age;
- active positions, bounded to the admission maximum;
- each active position's target status and blocker;
- Light deferred, queued-demand, mailbox, and publication counts;
- scheduled-without-token and token-without-owner counts;
- repair/retry totals; and
- current requested/server-ready/exact coverage.

The normal fast path should maintain counters incrementally. A bounded scan of
at most four active promotions is acceptable. Do not clone all holder or
requested-view state into every report.

Split `pending_job_count()` for diagnostics and readiness decisions. It may
remain as a compatibility aggregate, but probes and operators must not infer
Worker ownership from it.

### Tactical 311 consumes, but does not own, the repaired state machine

Worker failure recovery will eventually return retryable Light and worldgen
outcomes to the scheduler. Tactical 312 establishes what it means for a Light
request to be deferred, current, stale, restartable, or completed. Tactical
311 should reuse those states rather than add a second Worker-specific retry
queue.

Tactical 312 does not add Worker deadlines, generations, physical
reconstruction, or response replay. Those remain Tactical 311 scope.

## Implementation Sequence

### Slice 0: Prove the exact active blocker before changing behavior

Land diagnostics and failing tests first.

Add a focused shared scheduler test that:

1. requests a view large enough to leave queued promotions;
2. advances until at least one Features-ready target owns a pending initial-
   Light request;
3. changes interest so that request becomes obsolete while its holder remains
   resident or unload-rescuable;
4. re-enters the position before full quiescence;
5. stops movement and continues bounded scheduler polling; and
6. asserts the pre-fix state: Light Scheduled, no token owner, active promotion
   retained, empty Light mailbox, and no convergence.

Add a route-level probe that does not settle between chunk centers:

1. create a fresh world at render distance 8;
2. wait only for the playable gate, not the complete view;
3. move north three chunks;
4. move west three chunks;
5. ascend approximately 128 blocks;
6. move north another three chunks;
7. reverse across at least one recently cancelled boundary;
8. stop and poll for at most 60 seconds; and
9. hold an exact-coverage stability window.

Record the promotion decomposition and bounded active blockers at every view
change. Run the deterministic shared test on native first, then the real
headed browser topology. If the natural phone plateau does not contain the
source-correlated orphan state, stop before Slice 1 and update this tactical
with the actual blocker rather than implementing an assumed fix.

### Slice 1: Make Feature-to-Light context restartable

- add scheduler/holder ownership for the minimal restartable context;
- preserve it across demand cancellation and mailbox cancellation;
- distinguish deferred Light from live-token Light;
- create a new revision-bound token when current interest re-requires the
  holder;
- reject late results from the cancelled token;
- consume context exactly once on successful publication or authoritative
  unload; and
- prove scheduled block/fluid records and cache persistence are neither lost
  nor duplicated.

Commit this slice independently with focused state-transition tests.

### Slice 2: Enforce promotion forward progress

- add the bounded active-promotion blocker classifier;
- repair required deferred/orphan Light during normal reconciliation;
- ensure holder rescue and quick re-entry run the same repair;
- ensure departed positions release active promotion ownership immediately;
- separate compatibility aggregate counts from executor work counts;
- add debug assertions and production repair diagnostics; and
- prove four orphan candidates cannot exhaust promotion admission.

Test cancellation before mailbox admission, cancellation in flight,
completion racing cancellation, re-entry before unload, re-entry after unload,
and repeated revision replacement.

### Slice 3: Establish shared native/Web semantic traces

Run the same scripted view sequence against:

- the shared scheduler with native thread mailboxes;
- the native local integrated runner;
- the browser integrated-server Rust actor with real job Workers;
- desktop headed Chrome/WebGPU; and
- physical Android Chrome.

Normalize the trace around semantic states rather than physical timing. The
same view sequence must produce equivalent cancellation, deferred admission,
fresh-token, stale-result, publication, and final-ready transitions.

Add a regression lane to the normal Web smoke suite and a fast shared test to
the default server suite. Do not rely only on a long optional soak.

### Slice 4: Physical closeout and living-doc correction

Build and deploy the exact candidate revision, repeat the real title-screen
and Graphics-setting flow on the Pixel, inspect the physical screenshot, and
record:

- deployed revision and browser version;
- actual touch-driven route and final center;
- requested, server-ready, client, and exact coverage;
- promotion states and oldest ages;
- Worker request/response and mailbox counts;
- snapshot/unload conservation chains;
- time to convergence; and
- a stationary 60-second stability window.

Repeat a long bounded-memory movement lane on native/Quest or the current
accepted low-memory target before closure. Update
[`../topics/chunk-lighting-admission-and-backpressure.md`](../topics/chunk-lighting-admission-and-backpressure.md)
with the final lifecycle and evidence. Update Tactical 311 only where its
Worker-recovery integration consumes the new scheduler outcomes.

## Acceptance Matrix

| Scenario | Shared scheduler | Native integrated | Desktop Chrome | Android Chrome |
|---|---:|---:|---:|---:|
| cancel before Light mailbox admission | required | required | required | sampled |
| cancel while Light request is in flight | required | required | required | sampled |
| re-enter before holder unload | required | required | required | required |
| re-enter after holder unload | required | required | required | sampled |
| rapid multi-axis RD8 route | required | required | required | required |
| fresh persistent world and settings UI | N/A | required | required | required |
| sustained movement memory bound | required | required | required | required |

For render distance 8, the stopped route must converge within 60 seconds to:

```text
requested client coverage: 361 / 361
server-ready coverage:     361 / 361
exact drawable coverage:   289 / 289
missing target columns:      0
promotion queued/active:      0 / 0
feature jobs:                 0
Light deferred/demand:        0 / 0
job mailboxes:                0
pending publications:         0
delivery queues:              0
scheduled Light without owner: 0
```

During movement, bounded work is expected and exact coverage may lag. After
movement stops, every non-ready active promotion must change blocker or
complete within the bounded trace. Merely seeing counters fluctuate is not
acceptance.

The final coverage must remain exact through a 60-second stationary stability
window. Worker generation must remain unchanged in the clean route; a restart
would move the receipt into Tactical 311's failure matrix.

## Validation Commands

Exact new script names may be introduced by Slice 0. Closeout must include at
least:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo check --manifest-path native/Cargo.toml -p mclone-web-client \
  --target wasm32-unknown-unknown
pnpm native:scheduler:promotion-churn
pnpm native:scheduler:smoke
pnpm native:movement:smoke
pnpm native:web:cardinal-view-replay
pnpm native:web:typecheck
pnpm native:web:scene-host-adoption
pnpm native:web:promotion-churn
```

Browser/WebGPU lanes must follow the headed-host rules in
[`../platforms.md`](../platforms.md). Save screenshots and structured probe
reports under `/tmp`, inspect every material capture, and run headed GPU lanes
sequentially.

## Non-Goals

- increasing Player-promotion, Light-mailbox, or memory ceilings;
- making complete-view generation synchronous;
- changing world generation, lighting results, or chunk-interest geometry;
- changing the user's render-distance choice;
- retaining obsolete authoritative chunks or stale meshes to hide holes;
- adding a Web-only retry, timer, queue, or coalescing policy;
- weakening revision/token stale-result checks;
- treating `pending_job_count()` as a Worker-health signal;
- implementing Worker timeout/reconstruction from Tactical 311; or
- accepting eventual convergence without a bounded stationary deadline.

## Stop Conditions

Stop and request a new decision if implementation requires:

- raising a physically accepted memory bound;
- duplicating or discarding scheduled block/fluid ticks;
- weakening persistent record revision or durability semantics;
- replaying a non-idempotent gameplay mutation;
- moving scheduler state or retry policy into TypeScript;
- a Web-only change to shared chunk status or promotion behavior;
- changing exact coverage or client-ready definitions; or
- proceeding after Slice 0 disproves the cancellation/re-entry blocker.

## Expected Commit Slices

Use `Topic: chunk-lighting-admission-and-backpressure` for the implementation
series:

1. add promotion-blocker diagnostics and failing churn probes;
2. retain restartable Feature-to-Light context across cancellation;
3. repair deferred Light work and enforce promotion progress;
4. add normalized native/Web churn traces and smoke gates; and
5. record physical Android and bounded-memory closeout evidence.

Do not mark this tactical complete because one clean new world loads. Closure
requires the exact cancellation/re-entry tests, the non-quiescing route, native
and Web semantic agreement, physical Android convergence, and preservation of
the accepted sustained-movement memory ceiling.
