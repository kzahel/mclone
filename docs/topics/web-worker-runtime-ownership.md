# Web Worker Runtime Ownership

Topic: web-worker-runtime-ownership

Status: high-value isolated-actor campaign complete 2026-07-19. Production
keeps isolated Wasm heaps and the existing external `SharedArrayBuffer`
transports. Tactical
[`197`](../tactical/197-domain-blind-web-worker-broker.md) moved the render
coordinator and worker actor, integrated-server startup and authority policy,
server-job dispatch, and catalog descriptors into Rust. Its closeout found no
evidence that justifies shared Wasm linear memory or further browser-native
long-tail reduction without a new human decision.

## Scope

This topic owns the boundary among:

- shared Rust engine policy and browser-specific Rust adapters;
- browser `Worker` construction, event-loop, promise, and shutdown mechanics;
- worker-resident Rust state machines;
- explicit `SharedArrayBuffer` byte mailboxes;
- copies and codecs between independent Wasm instances; and
- the possible later use of one shared Wasm linear memory.

It complements the completed
[`web-scene-host-adoption`](web-scene-host-adoption.md) topic. Scene-host
adoption established one shared `McloneSceneHost` policy owner; this topic asks
how much lower-level worker protocol and lifecycle knowledge should remain in
TypeScript around that host.

This topic does not reopen browser cadence, DOM input, WebGPU presentation,
IndexedDB execution, WebSocket construction, or other browser-only mechanics.
It also does not require every TypeScript line to disappear. The target is
domain-blind TypeScript, not zero TypeScript.

## Accepted Direction

Three architectures are available:

| Shape | Bulk copies | Platform-divergence risk | Shared-state risk |
|---|---:|---:|---:|
| domain-aware TypeScript coordination over isolated Wasm instances | moderate | highest | low |
| isolated Rust actors plus a generic TypeScript broker | moderate | low | low |
| one shared Rust/Wasm heap across Workers | lowest | low | highest |

The middle shape is the default target.

Each browser Worker keeps its private Wasm linear memory and owns a
worker-resident Rust actor. Main Rust owns domain policy, request identity,
budgets, revisions, stale-result decisions, and the encoded command. TypeScript
owns browser mechanics and forwards opaque frames, shared buffers, wakeups, and
completion/failure notifications without interpreting engine concepts.

This separates two goals that had been conflated:

1. Prevent TypeScript from becoming a second engine-policy surface.
2. Eliminate serialization and copies by sharing the Rust heap.

The first goal does not require the second. Mclone should collect the first
goal's architectural reward before accepting the second goal's allocator,
lifetime, crash-recovery, toolchain, and synchronization risk.

## Current Production State

The browser already shares much more policy with native clients than the raw
TypeScript size suggests:

- production uses `mclone_scene::McloneSceneHost`;
- render dirty planning, revision acceptance, cache merge, and admission are
  Rust-owned;
- integrated authority is a Rust `RealmServer` inside a Web Worker;
- world generation and lighting algorithms are Rust-owned worker jobs;
- generator profile and managed-scenario ownership locks reject TypeScript
  implementations of those behaviors; and
- local Worker, IndexedDB local-world, and remote WebSocket modes enter the
  same scene/runtime contracts.

The browser worker topology nevertheless has substantial hand-authored
TypeScript coordination. The authored `.ts` inventory on 2026-07-19 is:

| Family | Lines | Main contents |
|---|---:|---|
| `mclone-web-app.ts` | 2,484 | rAF/platform assembly, browser async operations, and smoke/report coordination |
| server, job, remote, and provision Workers | 1,328 | IndexedDB servicing, opaque actor forwarding, WebSocket, provisioning, timers, and SAB mechanics |
| render compiler Worker, shim, and declarations | 124 | Wasm loading, opaque actor forwarding, and the render doorbell |
| input and touch | 886 | browser input mechanics |
| world catalog and settings | 862 | IndexedDB CRUD, opaque descriptor storage, browser settings |
| threading smoke Worker | 47 | shared-memory capability proof |
| generic Worker transport | 47 | opaque `post`/`poll`/`terminate` browser mechanics |
| **total** | **5,778** | authored TypeScript, excluding JavaScript smoke harnesses |

Line count is a warning signal, not a correctness metric. Input, touch,
IndexedDB transactions, and browser presentation can legitimately remain
large. The concern is the domain vocabulary and state machines that grow
inside the browser adapter: generation-profile unions, topology projection,
render work kinds, active/standby priority, per-world compiler sessions,
request retries, asset epochs, and hand-authored result bags.

Slice 0 locked the pre-cutover 7,417-line baseline. Slice 1 removed all four
registered server-job dispatch debts and seven net TypeScript lines by replacing
the worldgen/light selector with one opaque Rust actor call. The line delta is
incidental; the zero domain-selector counts are the acceptance evidence.

The subsequent Slice 1 validation cleanup corrected an independent semantic leak:
gameplay dispatch success had been reported as `changed` even though it proved
only command submission. The browser now establishes block changes causally by
observing the exact replica block state, section-update progress, and the
resulting remesh. This added three explanatory TypeScript lines, for a
7,413-line pre-Slice-2 inventory, without restoring any server-job domain
selector.

That stronger probe exposed the actual intermittent remote failure. The
dedicated server's nonblocking WebSocket writer disconnected on `WouldBlock`
instead of retrying Tungstenite's buffered frame. It now retains a
pending-flush state, has a deterministic regression test, and passed five
consecutive fresh remote interaction smokes. The baseline is therefore green;
that evidence authorized the bounded render cutover.

Slice 2 replaced the main-side TypeScript render compiler broker atomically.
`WebRenderWorkerCoordinator` now owns worker generation/readiness, globally
qualified request identity, active/standby admission, stale completion and
timeout handling, quiescent world release, asset candidate activation and
rollback, failure reconstruction, and compile diagnostics. `WebSceneHost`
owns exactly one coordinator, while each scene runtime carries a
world-qualified handle. The TypeScript boundary is a generic polled transport
that constructs one ordinary Worker and forwards opaque messages, transfer
handles, browser errors, and termination.

The Slice 2 cutover removed the TypeScript broker class, priority queue, pending
promises, compiler-wake relay, release policy, and asset-worker swap policy.
Authored TypeScript is now 6,484 lines: 933 below the Slice 0 baseline and 929
below the pre-Slice-2 tree. The render-worker family fell from 1,349 to 536
lines and `mclone-web-app.ts` from 2,650 to 2,487; the new generic transport is
47 lines. All main-side render ownership debt counters are zero. The eight-site
copy ledger, private Wasm heaps, external SAB ABI, and worker-side render actor
remain unchanged.

Slice 3 subsequently moved the worker-side render authority into
`WebRenderWorkerActor`. Worker Rust now owns the selected asset template,
per-world compiler sessions and release, render-section versus Far LOD
dispatch, target normalization, compile diagnostics, shared-input validation,
and atomic shared-result publication. The Worker TypeScript is a 69-line Wasm
loader and opaque frame forwarder; it no longer names asset selection, world
priority, work kinds, compiler methods, or per-world sessions.

The cut also removed one real production copy. Compiler internals now return
their packed Rust bytes to the actor, diagnostics inspect those bytes in place,
and worker Rust copies them directly into the external result SAB. The prior
Rust-to-JavaScript temporary array followed by JavaScript-to-SAB copy is gone.
The ownership inventory now reports 6,072 authored TypeScript lines, 124 render
worker lines, zero worker-side render debts, and a seven-site copy ledger.
Private Wasm heaps, one ordinary render Worker, the external SAB ABI, and
failure containment remain unchanged.

Post-cutover validation passed the complete Rust workspace and the ordinary,
movement, and desktop-lobby browser lanes. The lobby held two resident compiler
sessions across three qualified world identities while retaining one Worker,
one Worker-Wasm initialization, one asset load, and zero result overflow or
response transfers. Its embedded-preview capture remained coherent. The
broader lifecycle lane again stopped only at its recorded hard-coded actor-ID
fixture after 628 successful actor compiles; its assertion was not weakened.

The subsequent render/server-job shell comparison did not justify a common
broker. Only the one-shot Wasm import and error stringification are identical.
The render result mailbox is now Rust-owned, while the server-job shell still
owns a distinct transfer-or-SAB response protocol and its failure wakeup.
Factoring only the tiny loader would add indirection without reducing domain
ownership or copies, so the shells remain specialized until another Rust actor
makes a larger contract genuinely identical.

The first integrated-server cut is also complete. Main Rust now authors a
strict versioned opaque startup frame containing all generation, topology,
behavior, player/observer authority, debug, light, and identity configuration.
A worker-resident `WebIntegratedServerStartup` decodes it, constructs the
transient or external-load IndexedDB realm, honors stored metadata precedence,
and applies the authority configuration. TypeScript no longer names or defaults
any of those fields; it retains IndexedDB transactions, URLs, runner transport,
and timer mechanics. Its unreachable bulk-record constructor fallback was
removed.

This cut reduced the integrated-server Worker from 1,202 to 1,039 lines and the
authored TypeScript inventory from 6,072 to 5,909 lines. The ordinary browser
lane passed shared and transfer runner stress, IndexedDB mutation/reload passed,
and the desktop lobby passed protected interaction, warm standby, embedded
preview, and activation checks. The next high-value owner is the live
integrated-server authority/session actor, which the following cut completed;
the external SAB and IndexedDB platform contracts remained deliberately
unchanged.

The live authority/session cut is now complete as well. A resident
`WebIntegratedServerActor` owns operation admission, message-to-session
dispatch, command-only pending-job policy and poll limit, completion/failure
envelopes, and graceful domain shutdown. TypeScript retains the browser event
exclusion guard, IndexedDB batching, zero-delay yields, SAB views/publication,
message posting, timer cadence, and final Worker close. This is deliberately a
bounded actor rather than a Rust reimplementation of browser mechanics.

The integrated-server Worker is now 919 lines and total authored TypeScript is
5,789 lines. The cut added no copy, worker, heap, schema, cadence, or SAB-ABI
change. Shared/transfer runner stress, IndexedDB reload, and the desktop lobby
remain green, including protected behavior and observer/player transitions.

The browser catalog now persists a strict version-1 `MCWC` descriptor inside
the existing `{ id, descriptor }` IndexedDB key-path envelope. Rust owns every
persisted catalog fact, compatibility derivation, canonical ordering, and the
clear UI projection. TypeScript retains only CRUD/transaction mechanics, the
stable `id`, an opaque byte value, and browser timestamps. New writes contain
no clear generation-profile or persisted-world policy fields. Legacy clear
records remain readable and are rewritten in place on open or play-recording;
the database remains version 6 and no migration runs at startup.

Topology and behavior are intentionally not duplicated into this catalog
descriptor. They were never catalog-summary facts: the dimension record owns
topology, realm metadata owns behavior, and the integrated-server startup frame
carries the selected session facts. The periodic IndexedDB reopen proof still
recovers its cylinder descriptor and canonical edit from those existing Rust
owners. Managed scenario metadata remains in its separate store and its
isolation proof reports zero ordinary catalog rows before and after.

This cut reduced authored TypeScript to 5,778 lines. The catalog adapter's UI
summary type is now an opaque Rust projection with only `id` exposed to
TypeScript, its generation-profile union is gone, all catalog ownership debts
are zero, and the seven-site copy ledger is unchanged. The descriptor itself
round-trips full-width integers, but browser world creation deliberately keeps
its pre-cutover JavaScript-number seed semantics in this ownership-only slice.
An initial attempt to change that semantic selected a different large-seed
fixture world and was removed before landing.

Slice 6 closed the campaign at that boundary. The final inventory contains 15
authored TypeScript modules, six Worker entries, and two TypeScript Worker
construction sites. All 24 registered domain-ownership debts are zero. The
remaining 5,778 lines are concentrated in rAF/DOM assembly, input, IndexedDB,
WebSocket, timers/yields, typed SAB views and atomics, provisioning, settings,
and smoke/report exposure. Further reduction would primarily move
browser-native mechanics into Rust rather than remove a competing engine
owner.

The source copy ledger has seven explicit production sites: main Rust to the
render input SAB, that SAB into worker Rust, worker Rust to the render result
SAB, that SAB into main Rust, parent Rust to the server-job request SAB,
server-job JavaScript to its response SAB, and that SAB into parent Rust. The
movement probe retained 16 compiles with at most 574,641 input bytes and
1,085,108 result bytes. Their worker round trip averaged 18.9 ms and ranged
from 15.2 to 25.1 ms; main decode/finish/apply was below the browser timer's
resolution, the maximum observed frame gap was 20.4 ms, and no response
overflow or transfer fallback occurred. The telemetry does not separate codec,
copy, and compute time inside the Worker, so it cannot establish copy
dominance. Tactical 068's most recent decomposition remains the stronger
evidence: lighting was about 98% compute and cold world generation about 88%
generation after transport attribution.

The desktop lobby passed with a 9.925 ms p95 and 41.055 ms maximum frame gap;
the two-times CPU-throttled mobile lobby passed with 10.065 ms p95 and 100.985
ms maximum. Both used one render Worker, two resident compiler sessions, three
qualified world identities, and two active integrated-server Workers. Measured
external SAB capacity was 29.55 MB desktop and 29.07 MB mobile. Compatible
terrain resources had two owners with zero duplicated atlas base bytes. Known
actor retention was 52,436 bytes and actor-state allocation about 1.35 MB.
Exact main and Worker Wasm heap high-water and Worker CPU are not exported, so
the closeout does not pretend to have a complete duplicate-memory or copy-time
decomposition.

Failure containment remains an advantage of the chosen boundary. Rust tests
cover coordinator failure, stale generation/completion rejection, asset
rollback, restart, and idempotent termination. Browser runner stress reached
clean shutdown in shared and transfer modes. The lifecycle probe completed a
cancelled provision and an injected integrated-server Worker-construction
failure without changing the active world or leaking the failed standby, then
stopped at its separately recorded hard-coded actor-ID fixture. Five fresh
remote WebSocket interaction runs also completed with zero pending work.

Two complete pre-cutover control runs separated coordinator evidence from
unrelated browser-fixture debt:

- the lobby lifecycle probe hard-codes returned live actors `1`/`2`, while
  both trees correctly returned later live actors after simulation; and
- the Far LOD movement leg settled at exactly 936 desired, 899 visible, and
  1,225 resident tiles with zero pending work on both trees, but accumulated
  ten `suppressed_without_replacement` events.

The mobile smoke likewise fails its unchanged startup-camera ledger on both
trees (`112` hold versus `93.62` recorded minimum). Its added hidden/resume leg
did advance the background save and hidden-frame counters successfully. These
are confirmed baseline debts, not coordinator regressions or weakened checks.
The focused desktop/mobile lobby, asset replacement, IndexedDB, movement,
local, and five fresh remote interaction lanes are green.

The existing scene-host purity gate covers known competing scene-policy
owners in `mclone-web-app.ts`, and focused Rust tests forbid particular
generator and scenario implementations in TypeScript. Those are valuable but
do not yet inventory every worker module or establish one reusable
domain-blind worker contract.

## What `SharedArrayBuffer` Means Today

Production's `shared-memory` diagnostics describe explicit external byte
mailboxes. They do **not** mean that the production Rust heap is shared.

The main Wasm instance and each Worker Wasm instance have independent linear
memories. A typical render transaction is:

```text
main Rust value
  -> encode bytes
  -> copy into external SharedArrayBuffer
  -> publish atomic ready/status words
  -> worker copies bytes into its private Wasm heap
  -> decode and compute
  -> encode result
  -> copy into external SharedArrayBuffer
  -> publish atomic completion words
  -> main copies bytes into its private Wasm heap
  -> decode
```

The mailbox is a shared locker between isolated Rust heaps. Small atomic words
publish ownership and byte counts; the Rust objects themselves never cross the
worker boundary.

That isolation has real benefits:

- each worker has an independent allocator and mutable object graph;
- browser termination cannot strand a mutex in another Rust instance;
- data races cannot cross worker heaps;
- a failed worker can be reconstructed without trusting a shared heap; and
- browser event-loop ownership stays explicit and naturally asynchronous.

Its costs are explicit codecs, copies, duplicate worker-resident state, ABI
schemas, and a temptation to let TypeScript own more policy because it already
sees every message.

## Prior Evidence

Tactical
[`068`](../tactical/068-web-zero-copy-worker-lane-investigation.md) measured the
worldgen and lighting lanes before choosing against shared Wasm linear memory:

- lighting was approximately 98% worker computation;
- cold world generation was approximately 88% generation work; and
- warm world generation was serialization-dominated because a 529-chunk
  dependency neighbourhood bounced through main on every job.

Tactical
[`069`](../tactical/069-web-worldgen-lane-payload-reduction.md) then removed the
largest warm-worldgen transport tax without a shared heap. A resident Rust
worker session plus delta protocol reduced the representative warm request
from about 27 MB to 81 bytes and the response from about 29 MB to 3.2 MB while
keeping desktop's moved-value path untouched.

Tactical
[`067`](../tactical/067-shared-render-worker-architecture.md) made the same
choice for render compilation: one shared logical `RenderSectionCompiler`
contract, private platform transports, resident worker state, and delta-only
inputs. Its measured single-worker budget-one path remained below a frame on
the tested movement bursts.

These results establish an important precedent: payload reduction and Rust
policy convergence can capture most of the practical benefit without sharing
the heap.

Tactical 068's exact toolchain findings were made against its then-current
Rust/wasm-bindgen environment. Any future shared-heap proposal must revalidate
the current toolchain instead of copying those claims forward. Its measured
work/serialization decomposition and its preference for payload reduction
remain relevant evidence.

## Ownership Contract

### TypeScript may own

- `Worker` construction and module/bootstrap URLs;
- `postMessage`, `MessageEvent`, transfer lists, and browser error events;
- `SharedArrayBuffer` feature detection and typed views needed by an explicit
  mailbox ABI;
- promise settlement, `AbortSignal`, timers, and yielding to the browser event
  loop;
- IndexedDB requests and transactions;
- WebSocket construction and browser network events;
- rAF, DOM input, visibility, canvas/surface, and presentation mechanics; and
- generic byte counts, queue depths, and browser-failure forwarding.

### Rust owns

- engine message schemas and defaults;
- generation profiles, topology, realm/dimension, and scenario concepts;
- job kinds and domain dispatch;
- scheduling, admission, active/standby priority, budgets, and backpressure
  policy;
- request IDs, epochs, revisions, stale-result rejection, retries, and
  supersession;
- worker-resident domain sessions, caches, and mirror lifetimes;
- result interpretation and acceptance;
- graceful domain shutdown state; and
- authoritative diagnostics semantics.

A TypeScript broker may carry a generic request ID, byte view, buffer handle,
or opaque initialization frame. It should not enumerate every field of an
engine request or select a Rust algorithm by a domain string.

## Target Isolated-Actor Shape

```text
shared Rust owner
  -> constructs typed request
  -> web Rust adapter encodes an opaque actor frame
  -> generic browser broker forwards frame / SAB handles
  -> worker-resident Rust actor decodes and updates its private state
  -> generic browser broker forwards completion / error
  -> web Rust adapter accepts typed result
```

Native workers continue to move typed Rust values over native channels. The
symmetry is the Rust trait and actor lifecycle, not a requirement that desktop
use the browser byte ABI.

The generic broker does not need to be one universal Worker script on day one.
It is acceptable to prove a narrow broker for server jobs and another for
render compilation, provided both are domain-blind and converge on a small
common browser-mechanics module only when the common shape is real.

For the main-side render lane, the accepted browser-mechanics boundary is a
generic polled transport rather than asynchronous domain callbacks into Rust:

```text
Rust coordinator -> post opaque message / handles -> TypeScript Worker shell
Rust frame poll  <- queued opaque event / error  <- TypeScript Worker shell
```

The transport may own `new Worker`, `postMessage`, event capture, transfer
lists, SAB handles, and `terminate`. Rust owns worker generations, readiness,
request/world association, active/standby priority, timeouts, stale-result
policy, quiescent world release, asset-epoch replacement, and recovery. This
keeps the web mechanism explicit while making its coordinating semantics
testable Rust state instead of a second TypeScript runtime. It also better
matches native's nonblocking send/`try_recv` shape without forcing native to
serialize through the browser ABI.

## Shared Wasm Linear Memory: Separate Option

A future shared-heap runtime would instantiate multiple Wasm instances over
one `SharedArrayBuffer`-backed `WebAssembly.Memory`. Ordinary Rust accesses do
not automatically become atomic and do not receive implicit barriers;
synchronization is paid at explicit atomic, queue, lock, and reference-count
operations. The WebAssembly threads model distinguishes ordinary and atomic
memory operations; see the
[WebAssembly threads memory model](https://webassembly.github.io/threads/core/exec/relaxed.html).

The dangerous costs come from architecture rather than a global barrier tax:

- blocking or busy-waiting on the browser main thread;
- one contended shared allocator;
- false sharing and fine-grained atomic/`Arc` traffic;
- frame-time memory growth and stale JavaScript views;
- arbitrary JavaScript views violating Rust aliasing/lifetime assumptions;
- terminating a Worker while it owns locks or shared references;
- panics or crashes leaving the shared runtime's health ambiguous;
- thread-affine `JsValue`, DOM, and WebGPU handles crossing into shared jobs;
- oversubscribed worker pools; and
- a large shared heap retaining high-water allocations.

`wasm32-unknown-unknown` also does not turn browser Workers into ordinary
`std::thread::spawn` support; the
[Rust target documentation](https://doc.rust-lang.org/stable/rustc/platform-support/wasm32-unknown-unknown.html)
still records that limitation. A deliberate bootstrap, stack/thread-state,
allocator, cooperative-shutdown, and crash-recovery runtime is required.

No stage of Tactical 197 may silently introduce shared Wasm linear memory.
That requires a separate tactical and an explicit go decision after all of the
following are true:

1. Post-convergence profiling shows serialization/copy remains at least about
   20% of a user-felt path.
2. The result reproduces on a representative slower/mobile browser.
3. Payload reduction, resident worker state, and coarse batching are already
   exhausted.
4. Current Rust/wasm-bindgen/browser toolchain and support costs are proven.
5. Allocation, memory growth, atomic contention, queue wait, worker CPU, and
   heap high-water are instrumented.
6. Main-thread code never blocks or spins.
7. Cooperative shutdown and forced-worker-failure recovery are proven.
8. TypeScript cannot inspect or mutate live Rust object bytes.
9. A human architecture review accepts the remaining lifetime and deployment
   risk.

The 2026-07-19 closeout does not satisfy this gate. Payload reduction and
resident state are proven, and the capability smoke proves the browser can use
SAB, atomics, and shared-memory-capable Wasm. However, copy/serialization has
not been shown to consume 20% of a user-felt path on either desktop or
throttled mobile. Exact Worker CPU, private-heap high-water, allocator growth,
and atomic contention are also not instrumented. Cooperative recovery for a
hypothetical shared heap has not been designed or proven, and the required
human architecture review has not occurred. No shared-memory tactical is
therefore opened.

## Validation And Evidence

Worker-convergence slices must preserve:

- local Worker, IndexedDB local-world, and remote WebSocket behavior;
- runner, worldgen, light, and render-compiler worker kinds;
- current shared-buffer pool, fallback, overflow, and byte metrics;
- worldgen and render resident-mirror correctness;
- managed lobby with two world identities and active/standby compilation;
- asset-epoch replacement and worker restart behavior;
- mobile throttling and hidden/resume lifecycle behavior;
- native channel-backed worker paths; and
- rendered browser output inspected from `/tmp` whenever a slice can affect
  pixels.

Every material worker migration should record before/after:

- authored TypeScript lines by responsibility family;
- TypeScript-owned domain identifiers and state machines removed;
- encoded and copied bytes per lane;
- p50/p95 request and completion latency;
- maximum browser frame gap;
- worker creation/restart/shutdown counts; and
- final pending work and retained worker state.

Lower TypeScript line count is not sufficient acceptance. The executable
ownership checks and unchanged behavior/performance evidence are load-bearing.

## Code And Documentation Map

- [`../native-web.md`](../native-web.md): current browser operation and deploy
  commands.
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md): completed shared
  scene-policy adoption.
- [`platform-parity.md`](platform-parity.md): cross-platform capability and
  shared-contract ledger.
- [`../tactical/062-shared-threading-topology.md`](../tactical/062-shared-threading-topology.md):
  original native-thread/Web-Worker topology.
- [`../tactical/067-shared-render-worker-architecture.md`](../tactical/067-shared-render-worker-architecture.md):
  resident render compiler and explicit SAB mailbox.
- [`../tactical/068-web-zero-copy-worker-lane-investigation.md`](../tactical/068-web-zero-copy-worker-lane-investigation.md):
  measured shared-linear-memory decline.
- [`../tactical/069-web-worldgen-lane-payload-reduction.md`](../tactical/069-web-worldgen-lane-payload-reduction.md):
  landed isolated-actor payload reduction.
- [`../tactical/170-web-scene-host-adoption.md`](../tactical/170-web-scene-host-adoption.md):
  browser host/service cutover evidence.
- [`../tactical/197-domain-blind-web-worker-broker.md`](../tactical/197-domain-blind-web-worker-broker.md):
  completed implementation and closeout record.

Primary implementation surfaces:

- `native/apps/mclone-web-client/src/web_canvas.rs`
- `native/apps/mclone-web-client/src/web_render_worker.rs`
- `native/apps/mclone-web-client/src/web_render_worker_actor.rs`
- `native/apps/mclone-web-client/src/web_render_compiler_abi.rs`
- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/src/web_server_worker.rs`
- `native/apps/mclone-web-client/src/web_remote_session.rs`
- `native/apps/mclone-web-client/www/mclone-web-app.ts`
- `native/apps/mclone-web-client/www/mclone-render-compiler-shared.ts`
- `native/apps/mclone-web-client/www/mclone-render-compiler-worker.ts`
- `native/apps/mclone-web-client/www/mclone-worker-transport.ts`
- `native/apps/mclone-web-client/www/mclone-integrated-server-worker.ts`
- `native/apps/mclone-web-client/www/mclone-server-job-worker.ts`
- `scripts/check-web-scene-host-adoption.mjs`
- `scripts/check-web-worker-ownership.mjs`

## Recommended Next Work

Keep isolated heaps, the seven-site external SAB copy ledger, one ordinary
render Worker, browser-owned IndexedDB/timers/events, and native typed
channels. Treat the remaining TypeScript as an intentionally isolated browser
adapter, not an unfinished line-count target. Reopen this topic only when new
profiling identifies a concrete user-felt copy bottleneck or a domain-policy
owner appears in TypeScript. A new explicit human decision is required before
browser-native long-tail reduction or shared Wasm linear-memory work.
