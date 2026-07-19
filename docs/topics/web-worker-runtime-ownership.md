# Web Worker Runtime Ownership

Topic: web-worker-runtime-ownership

Status: first actor proof and validation baseline cleanup complete; the
main-side render coordinator cutover was approved 2026-07-19. The production
browser keeps isolated Wasm heaps and the existing external `SharedArrayBuffer`
transports while worker coordination moves toward Rust-owned actors behind
domain-blind TypeScript browser transports. The whole-worker baseline and
server-job Rust actor proof have landed. Tactical
[`197`](../tactical/197-domain-blind-web-worker-broker.md) owns the first
implementation slices. A shared Wasm linear-memory runtime remains a separate,
measurement-gated investigation rather than an implied destination of the
TypeScript reduction work.

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
| `mclone-web-app.ts` | 2,650 | rAF/platform assembly plus compiler wake, asset swap, async operation, smoke/report coordination |
| server, job, remote, and provision Workers | 1,611 | integrated-server lifecycle, IndexedDB servicing, opaque job-actor forwarding, WebSocket, provisioning |
| render compiler broker, Worker, and declarations | 1,349 | queueing, priority, request schemas, SAB doorbells, worker sessions, diagnostics |
| input and touch | 886 | browser input mechanics |
| world catalog and settings | 867 | IndexedDB mechanics, stored-record projection, browser settings |
| threading smoke Worker | 47 | shared-memory capability proof |
| **total** | **7,410** | authored TypeScript, excluding JavaScript smoke harnesses |

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

The subsequent validation cleanup corrected an independent semantic leak:
gameplay dispatch success had been reported as `changed` even though it proved
only command submission. The browser now establishes block changes causally by
observing the exact replica block state, section-update progress, and the
resulting remesh. This added three explanatory TypeScript lines, for a current
7,413-line inventory, without restoring any server-job domain selector.

That stronger probe exposed the actual intermittent remote failure. The
dedicated server's nonblocking WebSocket writer disconnected on `WouldBlock`
instead of retrying Tungstenite's buffered frame. It now retains a
pending-flush state, has a deterministic regression test, and passed five
consecutive fresh remote interaction smokes. The baseline is therefore green;
work remains stopped at the Slice 1 boundary review rather than proceeding to
render ownership.

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
  active implementation plan.

Primary implementation surfaces:

- `native/apps/mclone-web-client/src/web_canvas.rs`
- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/src/web_server_worker.rs`
- `native/apps/mclone-web-client/src/web_remote_session.rs`
- `native/apps/mclone-web-client/www/mclone-web-app.ts`
- `native/apps/mclone-web-client/www/mclone-render-compiler-shared.ts`
- `native/apps/mclone-web-client/www/mclone-render-compiler-worker.ts`
- `native/apps/mclone-web-client/www/mclone-integrated-server-worker.ts`
- `native/apps/mclone-web-client/www/mclone-server-job-worker.ts`
- `scripts/check-web-scene-host-adoption.mjs`
- `scripts/check-web-worker-ownership.mjs`

## Recommended Next Work

Review Tactical 197 Slice 1's actor/broker boundary. If accepted, begin Slice 2
by moving the main-side render compiler broker into browser-specific Rust while
preserving the external-SAB copy ledger and isolated heaps. Do not begin shared
Wasm linear-memory work as part of that continuation.
