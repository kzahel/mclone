# Cross-Platform Operation Execution

Topic: `cross-platform-operation-execution`

Status: reviewed and direction accepted 2026-07-20. The review verified the
current-state claims against the code, resolved most open questions in place
(see Questions For Review), and selected the sharing shape: a shared pure
decision core with narrow Rust platform strategies beneath one operation
port. This remains a directional architecture record, not an authorized
tactical. Managed-scenario provisioning is the motivating first consumer.

## Scope

Mclone needs one clear engine-facing model for bounded operations that may run:

- synchronously or on an ordinary Rust background thread on native hosts;
- through a Rust actor in an isolated browser Web Worker;
- around an asynchronous browser API such as IndexedDB or WebSocket; or
- through a later transport implementation without changing engine policy.

The shared engine should submit a typed, owned request and later receive a
typed, owned completion. It should not know whether the implementation used a
thread, channel, Worker, JavaScript promise, `postMessage`, transferable buffer,
external `SharedArrayBuffer`, or direct platform call.

Managed-scenario provisioning exposes the current gap most clearly. Shared
Rust already owns the scenario recipe, payload construction, validation, launch
policy, operation identity, and stale-completion rules. Native Rust provisions
through a background thread. The browser nevertheless lets TypeScript
orchestrate validation outcomes, record families, repair/publication behavior,
and concurrent-publication recovery. The desired result is not to make
managed provisioning part of the live `WorldStore` API. It is to give the
distinct managed-provisioning policy one shared Rust implementation driven by
platform executors beneath one target-neutral operation port.

This topic also records how that operation port relates to the existing
high-throughput render, worldgen, lighting, server, and remote-socket Worker
paths. It does not require one universal physical Worker protocol for every
kind of work.

## Motivation

### Keep the engine simple

The authoritative scene and application policy should say:

```text
submit ProvisionManagedScenarioWorld
receive Result<ProvisionedManagedScenarioWorld, Error>
```

It should not contain a browser-shaped continuation, await an IndexedDB
promise, inspect a SAB control word, or reproduce platform-specific teardown
rules. Nor should it contain parallel native and browser versions of the
provisioning decision tree.

One operation boundary makes request identity, cancellation epochs, stale and
duplicate completion rejection, error restoration, and shutdown behavior
ordinary shared Rust policy rather than conventions repeated by every host.

### Prevent TypeScript from becoming another engine

TypeScript is the correct owner for browser mechanics:

- creating and terminating Workers;
- loading Wasm modules;
- calling IndexedDB and WebSocket browser APIs;
- starting promises and browser transactions;
- constructing typed views over external SAB mailboxes;
- posting/transferring buffers; and
- forwarding browser errors and lifecycle events.

Those requirements do not imply that TypeScript should know what a chunk
means, distinguish managed primary and destination policy, decide whether a
stored world is valid or repairable, choose publication precedence, or
manufacture an engine completion. Keeping those decisions in TypeScript
creates a second policy surface whose behavior can drift from native.

The target is domain-blind TypeScript, not zero TypeScript. A browser database
adapter may know that stable physical namespace `2` maps to the existing
IndexedDB object store named `dimensionChunks`, just as the SQLite adapter may
know its table name and key columns. It must not decode the record, decide why
or when to write it, or branch on its gameplay meaning.

### Preserve native strengths

Cross-platform convergence must not mean forcing native through a browser
lowest common denominator. Native execution should retain:

- direct typed Rust calls inside a background thread;
- ordinary ownership moves and `Arc` sharing rather than serialization;
- direct SQLite/filesystem access without JavaScript or SAB intermediaries;
- no per-record hop through the simulation thread;
- no promise-shaped engine API; and
- platform-specific database optimizations behind the shared semantic port.

The shared abstraction is an owned operation/completion contract. Shared
memory is one implementation property of native threads, not part of that
contract. A native channel can move a `Vec` allocation into a worker without
copying its contents; a browser with isolated Wasm heaps may need an encoded
frame or external buffer. The engine should not be distorted by either fact.

### Make browser costs explicit and local

The accepted browser architecture currently uses private Wasm heaps per
Worker. A browser operation may therefore pay for an opaque control frame and
for bytes crossing the Wasm/JavaScript boundary required by IndexedDB. That
cost should stay inside the Worker that performs the operation.

For managed provisioning, worker Rust can materialize and validate the world,
the adjacent browser executor can write it, and only a small success/failure
completion needs to return to the main engine. Moving all materialized records
through the main Wasm instance would add cost without adding authority.

### Leave room for future transports

A completion port does not commit Mclone to today's implementation. A native
thread pool, an isolated Web Worker actor, a future shared-Wasm-heap worker, a
direct `web_sys` IndexedDB adapter, or another backend can implement the same
engine-facing contract. Transport experiments should not require rewriting
scene policy.

## Current State

### Shared operation vocabulary already exists

`mclone-app-runtime::platform_operation` already defines:

- `PlatformOperation<K>` with a target-neutral token and typed request;
- `PlatformOperationCompletion<T, E>` with an owned result;
- `PlatformOperationExecutor<K, T, E>` with `submit` and
  `try_recv_completion`;
- `PlatformOperationService` around a boxed executor;
- a deferred executor/handle for adapters that complete later; and
- `PlatformOperationLedger`, which owns epochs, monotonically increasing
  request IDs, pending requests, failure restoration, and stale, duplicate,
  or unknown completion classification.

No `JsValue`, promise, Worker, socket, path, or database handle enters that
ledger. This is close to the desired outer boundary.

Managed launch policy in `mclone-scene::warm_world` uses the same operation and
completion types, but currently owns a separate ledger and pending queues
instead of consistently entering through `PlatformOperationService`.

### Shared managed-content policy already exists

`mclone-app-runtime::scenario_content` owns storage-neutral:

- `ProvisionManagedScenarioWorld`;
- `ProvisionedManagedScenarioWorld`;
- scenario manifests, roles, and managed world keys;
- authored fixture materialization;
- expected payload fingerprints and metadata;
- validation of missing, partial, incompatible, corrupt, and valid stored
  worlds; and
- native and browser-neutral launch identities.

The policy is not fundamentally missing. Its browser execution is split at
the wrong layer, and its stored-state outcomes have already diverged between
hosts (see below).

### Native execution has the intended coarse shape

Native currently has two related execution shapes.

`NativeManagedScenarioContentOperationService` already wraps a real
`PlatformOperationService`. Its `BackgroundManagedScenarioExecutor` implements
`PlatformOperationExecutor<ScenarioLaunchIntent, NativeManagedScenarioContent,
String>`, starts a named Rust thread, and returns a typed completion through an
MPSC channel. This is concrete evidence that the target-neutral operation port
fits native background execution.

The live scene path instead uses `NativeManagedScenarioProvisionAdapter`. It
accepts a typed `PlatformOperation<ProvisionManagedScenarioWorld>`, starts a
named Rust thread, resolves the native content service, selects the requested
world role, and returns a typed completion. Its public `submit`/`poll` shape is
structurally the executor contract, but it does not implement the trait and the
scene treats it as a special adapter.

The physical native workflow also differs usefully from the browser workflow.
Native stages and atomically renames one versioned scenario directory
containing both worlds, manifest files, and per-world SQLite databases. The
browser publishes one managed world at a time into IndexedDB object stores.
The current live per-role adapter may therefore cause two concurrently issued
role operations to race through the whole-scenario native resolver, whose
atomic directory publication makes that safe. Convergence should clarify this
whole-scenario versus per-world mismatch rather than erase it accidentally.

Filesystem paths remain native and are resolved only when native assembly
starts an integrated server. Thread-per-operation versus a shared pool is an
executor tuning question, not a reason to copy policy into another platform.
`mclone-scene::session` also currently holds this adapter behind a
`#[cfg(not(target_arch = "wasm32"))]` field — a platform branch inside
shared scene code that injecting the executor through the operation port
removes.

### Browser execution still leaks the workflow

The browser scene host exposes loose reports for pending managed operations.
`mclone-web-app.ts` switches on `provision` versus `start`, reconstructs a
provision request containing scenario and role strings, creates an
`AbortController`, launches a one-shot Worker, interprets its result, and calls
back into Rust with a world ID or error string.

The one-shot provisioning Worker loads Wasm and calls TypeScript helpers in
`mclone-web-world-catalog.ts`. Those helpers:

- request a materialized payload from Rust;
- read managed metadata, chunk, and entity-chunk stores;
- ask Rust to classify the stored payload;
- decide whether to reuse, repair, publish, or reject it;
- clear and republish record families;
- implement conditional first publication;
- recover a concurrent publication race through revalidation; and
- construct the completion statistics consumed by the app.

Rust owns the individual materialization and validation functions, but
TypeScript owns the state machine connecting their results. This is the
remaining competing policy owner. The movable policy surface is small —
roughly 130–160 lines of decision logic and completion construction — while
the remaining several hundred lines of that module are IndexedDB mechanics
that stay in TypeScript regardless.

### Stored-state policy already diverges between hosts

Review surfaced a live behavioral divergence, not merely a structural one.
Native never repairs: when the published scenario directory exists but fails
validation, `ensure_current_lobby_preview` returns an error until the
directory is removed, and `validate_published_scenario` inspects only the
scenario manifest and SQLite headers. The browser classifies stored records
through the full shared validator, repairs `partial` and `incompatible`
worlds by clearing and republishing, and refuses only `corrupt`. The
browser's publication-conflict recovery is also a line-for-line TypeScript
analog of the native rename-race recovery — the dual-authoring drift this
topic predicts has already happened once. Convergence is therefore not a
pure refactor: the shared decision core must pick one behavior per
stored-state class and one validation depth, deliberately, rather than
inherit whichever platform is ported first.

### The lower storage seam is close but not complete for administration

Tactical 199 introduced `PersistenceRecordRequest` and
`PersistenceRecordResponse`, with stable namespaces, owned keys, opaque bytes,
atomic put/delete batches, probes, flush, close, and typed failures. Memory,
null, SQLite, and browser IndexedDB executors implement that opened-world
contract.

Managed provisioning needs additional administrative capabilities not needed
by the first opened-world cut:

- bounded scans or index reads for all records in a managed world;
- conditional insertion with explicit conflict reporting;
- bounded range or namespace deletion for repair; and
- an atomic transaction spanning managed metadata and the relevant record
  stores.

The persistence topic deliberately deferred these until a demonstrated
consumer existed. Managed provisioning is that consumer. Review found that
all four capabilities already exist in a second Rust-owned vocabulary: the
catalog storage-plan executor in
`mclone-web-client/src/web_catalog_execution.rs` defines `GetAll`,
conditional `AddCatalogRecord`, `DeleteIndexRange`, `Clear`, and multi-store
atomic `StorageTransaction` actions, driven by the Rust
`CatalogExecutionCore` state machine through a domain-blind TypeScript
runner. The browser therefore already has three storage vocabularies: the
opened-world record executor, the catalog storage-plan executor, and the
TypeScript provisioning machine. The accepted direction is to generalize the
catalog transaction-plan executor for the browser driver rather than extend
`PersistenceRecordExecutor` or stand up a fourth vocabulary, and to fold the
TypeScript provisioning machine into it. The fixed requirement is unchanged:
record meaning and workflow policy stay in Rust.

### Adjacent Worker convergence is already proven

The web Worker campaign established the middle-ground architecture this topic
builds on:

- worker-resident Rust actors own render, server-job, integrated-authority,
  and remote-WebSocket policy;
- main Rust owns operation identity, admission, stale-result handling, and
  typed failure;
- TypeScript owns browser construction, callbacks, timers, SAB mechanics, and
  opaque action execution; and
- independent Wasm heaps and failure containment remain intact.

The managed path should follow that ownership direction without assuming that
its storage-driven task has the same physical mailbox as a render compiler.

## Problem Statement

The system has most of the correct pieces at both ends:

```text
shared typed operation + ledger
shared scenario policy
native background execution
generic persistence records
browser Rust actor precedent
browser IndexedDB transaction mechanics
```

The missing middle is one shared Rust operation-policy/driver boundary that
lets native run directly in a background thread while browser Rust pauses
around asynchronous platform actions. Review weighed a fully shared
resumable task against native whole-directory publication versus browser
per-world record publication and selected a shared pure decision core with
Rust platform strategies (see Physical-representation variation). Without
some shared policy owner at this boundary, browser glue becomes the
continuation and accumulates domain meaning.

The goal is therefore semantic convergence with transport specialization:

1. one shared typed request and completion at the engine boundary;
2. one shared Rust owner for the operation's decision structure;
3. platform drivers that execute effects efficiently for their host; and
4. no requirement that native serialize, use SAB, or emulate promises.

## Proposed Architecture

```text
shared scene / application policy
       |
       | PlatformOperation<Request>
       v
shared operation service and ledger
       |
       | one shared Rust task/coordinator
       v
operation effects: storage reads, scans, atomic commits, cancellation
       |
       +------------------------------+
       |                              |
       v                              v
native driver                    browser Worker driver
background Rust thread           worker-resident Rust actor
direct Rust executor calls       generic browser actions
SQLite / filesystem              tiny TypeScript executor -> IndexedDB
       |                              |
       +------------------------------+
       |
       | PlatformOperationCompletion<Result>
       v
shared service / scene policy
```

### Layer ownership

| Layer | Shared responsibility | Platform responsibility |
|---|---|---|
| scene/application policy | when to request work; how success or failure affects the experience | none |
| operation service | tokens, epochs, pending work, stale/duplicate rejection, failure restoration | completion delivery mechanism |
| operation task | materialization, validation, reuse/repair/reject decisions, transaction intent | none |
| task driver | advance the task and return its typed completion | thread/channel versus Worker/event-loop integration |
| record/transaction executor | stable actions, opaque records, atomicity and typed outcomes | SQLite/filesystem/IndexedDB mechanics |
| physical transport | no engine meaning | channels, direct calls, promises, `postMessage`, transfer or SAB |

## Engine-Facing Operation Contract

The existing contract is the default starting point:

```rust
pub trait PlatformOperationExecutor<K, T, E> {
    fn submit(&mut self, operation: PlatformOperation<K>);
    fn try_recv_completion(
        &mut self,
    ) -> Option<PlatformOperationCompletion<T, E>>;
}
```

The exact trait shape may evolve, but its semantic properties should remain:

- requests and completions are typed and owned;
- request identity is unique for the host lifetime;
- scene teardown changes an epoch and rejects late work;
- duplicate and unknown completions are observable rather than silently
  applied;
- failure carries target-neutral restoration state;
- `submit` never makes the simulation thread wait on storage or browser work;
- polling is nonblocking; and
- platform resources remain behind the executor.

Cancellation and shutdown need an explicit review. Epoch invalidation already
defines the authoritative semantic result: late completion cannot affect the
new scene. A platform executor may additionally abort an IndexedDB transaction,
terminate a Worker, cancel queued native work, or let already-running coarse
work finish. The common contract should not promise immediate physical
cancellation unless every required backend can provide it.

Likewise, the contract need not expose shared memory. Native may use shared
address-space ownership internally; browser implementations may use transferred
frames or external SABs. Those are executor capabilities and metrics, not
request semantics.

## Shared Resumable Task

Managed provisioning is read-dependent: the correct write or completion
depends on stored state. A one-time write plan cannot express the whole
workflow without either moving policy into the executor or reading everything
up front through another special API.

The proposed shared owner is therefore an owned resumable task or coordinator.
Conceptually:

```text
start(ProvisionManagedScenarioWorld)
    -> Need(storage action batch)

resume(storage completion batch)
    -> Need(next action batch)
    -> ...
    -> Ready(Result<ProvisionedManagedScenarioWorld, Error>)
```

Review resolved the representation question. Under the selected sharing
shape (below), the shared owner is a pure decision core rather than one
normalized effect-emitting task. The web Rust strategy that suspends around
IndexedDB transactions should be an explicit enum phase machine, and the
native strategy runs to completion on its worker thread. Managed
provisioning needs at most a handful of suspensions — inspect, decide,
publish, and one re-inspect/re-decide round on conflict — so an internally
driven future adds machinery without value. Whichever Rust owner hosts the
workflow must satisfy these properties:

- the task contains no path, database handle, `JsValue`, promise, Worker, or
  socket;
- all inputs and intermediate completions are owned across suspension;
- no mutable Wasm borrow or JavaScript view crosses an `await`;
- every transition is deterministic from request, stored results, and shared
  policy;
- native and browser drivers cannot substitute their own validation or
  repair decisions;
- bounded action batches avoid one main-thread or cross-Worker message per
  record;
- suspension points align with whole storage transactions: an IndexedDB
  transaction auto-commits once control returns to the event loop, so the
  workflow may decide between transactions but never mid-transaction; and
- trace tests can compare exact transitions independently of a physical
  backend.

The engine does not drive these internal steps. The task lives inside the
platform executor's background context. The main scene still sees one request
and one final completion.

### Physical-representation variation

It may be unnatural for one low-level action enum to describe both an atomic
native directory rename and an IndexedDB multi-store transaction. The review
should not preserve a literally identical action trace by reducing native to a
record-by-record browser model.

Three degrees of sharing are plausible:

1. **One normalized task and storage-state model.** Both backends inspect their
   physical representation into shared stored-state observations; one task
   chooses reuse, publication, refusal, and conflict recovery. Drivers compile
   its abstract effects into directory or record transactions. This provides
   the strongest semantic lock if the abstract effects remain honest.
2. **One shared policy coordinator with Rust storage strategies.** Shared Rust
   owns recipes, acceptable outcomes, identity, validation requirements, and
   completion semantics. Native Rust owns staged-directory mechanics; web Rust
   owns the IndexedDB-oriented continuation. Both are exercised by one policy
   conformance suite, and TypeScript owns neither. This is acceptable if a
   single task would otherwise become a lowest-common-denominator storage DSL.
3. **Only the outer operation port is shared.** Native and web Rust each own
   their entire workflow. This still removes TypeScript policy and cleans the
   engine boundary, but it leaves the largest Rust divergence surface. It
   should be selected only with evidence that the stronger two shapes are
   materially more complex or harm native behavior.

Review resolved this choice. At the altitude that preserves native's atomic
staged-directory design, shapes 1 and 2 converge: a normalized task could
only emit high-level effects such as inspect and conditionally-publish, at
which point it is shape 2 with the sequencing inlined. The accepted form is
shape 2 built around a shared pure decision core: a small shared function
set mapping stored-state classification to the required action (valid reuse,
missing exclusive publication, partial or incompatible repair, corrupt
refusal) plus the conflict rule (on publication conflict, revalidate and
accept only a shared-valid winner). Native and web Rust strategies own
sequencing against their physical shapes and are held to the decision core
by a shared conformance suite. No normalized storage-effect language should
be built; the backends overlap only at the classification layer, which is
already shared Rust. The fixed requirement is unchanged: one engine
contract, shared policy facts and outcomes, no TypeScript decision owner,
and traceable equivalence where the physical representations overlap.

## Native Driver

The native executor should move the request into a background worker and
drive the native strategy, constrained by the shared decision core, to
completion there:

```text
receive typed operation once
loop task locally against direct record/filesystem executor
send typed completion once
```

Within that worker, task actions can call a generic, monomorphized Rust
executor directly. No record needs to round-trip through the game thread. A
`Vec<u8>` can move without copying its allocation; immutable materialized data
may use `Arc` where sharing is useful. SQLite transactions, filesystem staging,
WAL/checkpoint behavior, and native error detail remain native implementation
choices.

### Native performance contract

An implementation is unacceptable if convergence causes native to:

- encode or decode browser actor frames;
- copy every chunk through a generic byte mailbox;
- use atomics or SAB-shaped control words for ordinary thread work;
- perform synchronous storage on the simulation thread;
- dispatch each record through a separate cross-thread operation; or
- lose current atomic publication or filesystem/SQLite optimization.

One outer dynamic dispatch on submission and completion is insignificant for a
coarse storage operation. If measurement contradicts that expectation, the
service can be generic over its executor or use a platform enum without
changing the request/completion contract. The project should not complicate
the shared workflow speculatively to remove an unmeasured pair of virtual
calls.

The current thread-per-provision implementation is acceptable as a first
behavioral control because scenarios issue only bounded primary/destination
work. A later pool is an executor optimization, not part of the shared
semantic design.

## Browser Driver

The browser implementation should host the same Rust task in the one-shot
managed provisioning Worker or another explicitly chosen Worker lifetime.
Worker Rust should:

1. decode one opaque Rust-authored request frame;
2. construct and advance the web phase machine against the shared decision
   core;
3. emit generic browser storage actions when IndexedDB work is required;
4. accept owned action completions after browser promises settle;
5. continue until success or typed failure; and
6. return one opaque completion frame to main Rust.

TypeScript should:

- load the Worker Wasm instance;
- open the existing IndexedDB database;
- execute stable storage actions and transactions;
- pass opaque values or byte records without decoding them;
- translate browser API failures into stable storage-level error categories;
- honor abort/transaction/Worker lifetime mechanics; and
- post or transfer the final opaque frame.

TypeScript should not receive or switch on scenario IDs, managed roles,
validation status, record family meaning, repair mode, publication outcome, or
engine completion variants. Ideally the main browser app launches a Worker
from a Rust-authored opaque frame and forwards its final opaque frame back to
the host; it need not reconstruct `ProvisionedManagedScenarioWorld` from loose
strings.

The browser main thread must never block or spin. Rust must not hold an
exported mutable borrow across a promise. A storage callback returns an owned
completion to the worker-resident task and then lets it advance synchronously
until the next browser action or final completion. Storage actions are whole
transactions: TypeScript executes each transaction atomically without
yielding decisions back to Rust mid-transaction, because an IndexedDB
transaction auto-commits once control returns to the event loop. The
existing catalog runner already obeys this rule by enqueuing read-dependent
writes synchronously inside request callbacks.

## Storage And Transaction Boundary

The operation port and record executor are distinct abstractions:

- the operation port isolates the engine from where and how work runs;
- the operation task owns managed-provisioning meaning; and
- the record/transaction port isolates that task from physical storage.

Managed provisioning should not be forced into the live `WorldStore` API.
`WorldStore` owns simulation persistence, revisions, cache-versus-durable
scheduling, flush, and close. Managed provisioning is an administrative
installation workflow with validation, conditional publication, repair, and
reuse. They may share a lower record executor without sharing their upper
policy API.

### Acceptable physical knowledge

The IndexedDB adapter must manage the frozen browser schema. It may therefore
map stable numeric store/namespace/index identifiers to:

- object-store names;
- key paths;
- index names;
- transaction modes; and
- structured-clone value envelopes.

That is backend knowledge, analogous to a SQLite adapter knowing its tables.
The mapping should be isolated and mechanically testable. It should not imply
that TypeScript knows how an engine chunk is encoded or why a managed repair
deletes it.

A later refinement could let web Rust provide a versioned IndexedDB schema
descriptor that a fully generic TypeScript upgrader executes. That would
remove hard-coded store vocabulary from TypeScript, but it also creates a
schema/migration description language. It is optional and should be justified
separately from removing actual policy divergence.

### Candidate lower-port shapes

The review compared these variants and selected the third:

1. **Extend `PersistenceRecordExecutor`.** Add only demonstrated bounded scan,
   conditional-add/conflict, and range-delete operations, plus whatever stable
   physical namespace is needed for managed metadata. This maximizes lower
   reuse but must not contaminate live `WorldStore` policy.
2. **Add an administrative record companion.** Keep opened-world persistence
   minimal while exposing scan/publish/replace operations to catalog and
   managed coordinators. This preserves policy separation but risks two
   overlapping executor vocabularies.
3. **Compile both through a generic browser transaction plan.** Rust domain
   coordinators emit transaction actions to one IndexedDB executor. Native may
   still use a direct record executor. This can shrink TypeScript substantially
   but the shared cross-platform task must not become browser-plan-shaped.
   Review selected this shape: it already exists in embryo as the catalog
   storage-plan executor, and generalizing it avoids a fourth vocabulary.
4. **Use direct `web_sys` IndexedDB calls in web Rust.** The same outer
   operation contract still applies. This removes more TypeScript but moves
   browser API and async-lifetime complexity into web-specific Rust. It is a
   viable later adapter choice, not required by the ownership direction.

These lower-port choices apply most directly to the browser record workflow.
They do not require the native staged-directory publisher to pretend its
physical representation is a set of browser namespaces. A shared normalized
task may instead emit a higher-level conditional-publish or inspect effect
which native and browser Rust drivers implement through different lower
primitives.

The first implementation should preserve IndexedDB version 6, store names,
keys, record bytes, current managed metadata compatibility, transaction
atomicity, and ordinary/managed deletion isolation. A schema change is not
justified merely to make the seam prettier.

## Managed Provisioning Walkthrough

The intended shared sequence is approximately:

1. Shared scene policy issues a typed primary or destination provisioning
   request with an epoch-qualified token.
2. The platform executor places it in a native worker or browser Worker.
3. Shared Rust resolves the scenario manifest and materializes the expected
   storage-neutral payload in that background context.
4. The provisioning workflow requests the bounded stored state needed for
   validation.
5. Shared Rust classifies that state:
   - valid: reuse it;
   - missing: conditionally publish it;
   - partial or incompatible: atomically repair it according to shared policy;
   - corrupt: return the shared refusal; or
   - publication conflict: reread and accept only a shared-valid winner.
6. The physical executor performs the required transaction without decoding
   payload meaning.
7. The provisioning workflow returns a typed provisioned-world identity or
   typed failure.
8. The operation ledger accepts, rejects as stale/duplicate, or restores the
   launch state using the same policy on every host.

Only steps 2 and 6 differ physically between native and browser.

The per-class actions shown for step 5 follow the browser's current
behavior; question 13 records that native currently refuses instead of
repairing, and the decision core must fix the unified answer.

## Relationship To Compute Workers

Mclone should converge on one operation model, not necessarily one universal
physical Worker implementation.

| Work profile | Shared semantic shape | Likely optimized transport |
|---|---|---|
| managed provisioning, catalog, session start | coarse request, token, completion, cancellation epoch | native thread/channel; browser actor plus promise/API actions |
| opened-world persistence | typed mailbox requests, completions, flush/close barriers | native storage thread; browser actor plus IndexedDB executor |
| worldgen, lighting, render compilation | queued compute jobs, resident state, high-throughput byte results | native worker pool/shared heap; browser isolated actor with transfer or external SAB |
| remote socket session | commands, updates, backpressure and lifecycle | native socket runtime; browser Rust actor plus `WebSocket` callbacks |

These profiles can share:

- owned request and completion identity;
- nonblocking submission/polling;
- explicit cancellation and shutdown;
- bounded queues/backpressure;
- typed failures and metrics; and
- Rust ownership of domain state.

They need not share the same bulk-buffer ABI, Worker lifetime, scheduling
policy, or transaction vocabulary. Requiring a WebSocket, database operation,
and render compile to use one lowest-common-denominator mailbox would create
complexity rather than remove it.

The operation port is therefore a semantic family resemblance across these
systems. It is not permission to replace their proven specialized transports
with a single universal broker.

## Lifecycle And Failure Semantics

A tactical derived from this topic should make these cases explicit:

- submission failure before work begins;
- Worker/thread construction failure;
- storage open or lease failure;
- cancellation before and during a transaction;
- completion after the scene epoch was replaced;
- duplicate or malformed completion;
- platform executor panic, Worker error, or forced termination;
- failure after an atomic transaction commits but before acknowledgement;
- concurrent first publication;
- orderly shutdown with pending work; and
- retry after a failed operation without wedging the executor.

The operation ledger owns whether a completion may affect current shared
state. The task owns whether stored state is semantically acceptable. The
physical executor owns whether a transaction committed and how its backend
error is classified. None of those owners should infer another layer's result
from a timeout alone.

At-most-once physical execution is not always provable across a lost browser
acknowledgement. Therefore operations which may be retried must be idempotent
or validate committed state before republishing. Managed provisioning already
has a content-addressed/validated shape suitable for this; the shared
decision core should own that property.

## Advantages

- **One policy owner.** A shared task or shared coordinator/conformance contract
  prevents native and web from independently defining validation, repair,
  conflict, or completion behavior.
- **Clean engine call sites.** Scene code submits typed work and polls typed
  completions without platform branches.
- **Thin browser glue.** TypeScript performs browser APIs but does not become a
  scenario-content or persistence coordinator.
- **No meaningful native tax.** Native retains direct Rust execution inside a
  background thread and moves owned values rather than browser frames.
- **Testable semantics.** An in-memory executor can trace every workflow
  transition and inject deterministic failures without a browser or disk.
- **Replaceable transports.** Worker, direct `web_sys`, shared-Wasm, thread
  pool, and storage-backend experiments remain below a stable port.
- **Explicit lifecycle.** Tokens, epochs, cancellation and stale completion
  behavior become reusable contracts instead of adapter conventions.
- **Better performance placement.** Materialization and storage stay together
  in the background context, avoiding main-thread and main-Wasm payload hops.
- **Honest platform specialization.** Native and browser use their strongest
  mechanics without copying engine decisions.
- **Smaller divergence surface.** Source locks can reject domain vocabulary in
  TypeScript while Rust trace tests prove platform-independent policy.

## Costs And Risks

### Explicit continuation complexity

A resumable task can be more verbose than a synchronous function. Poorly
designed phase enums may expose incidental sequencing throughout the engine.
The task must be encapsulated behind the coarse operation port, with native
`run_to_completion` and browser actor drivers hiding its internal phases.

### Over-generalizing storage

Trying to anticipate SQL, IndexedDB, cloud sync, catalog, migration, and every
future administrative query could create an accidental database language. Add
only operations demonstrated by current consumers and keep exact transaction
traces under test.

### False universal-Worker abstraction

Forcing high-throughput compute, sockets, and browser storage into one physical
mailbox could lose specialized batching, backpressure, failure containment, or
native efficiency. Share lifecycle semantics and domain ownership; specialize
transport profiles where measured requirements differ.

### Cancellation mismatch

Native threads, browser Workers, and IndexedDB transactions have different
physical cancellation capabilities. The shared contract should promise epoch
invalidation and no stale state application. Best-effort resource cancellation
belongs below it and must be measured for leaks or shutdown delay.

### Large task state

Materialized records held while storage actions await can increase Worker heap
high-water. The task should batch deliberately, release buffers after
publication, and report retained bytes. It should not bounce bulk records
through the main engine merely to reduce the task object's local lifetime.

### Double abstraction

The project already has `PlatformOperationExecutor`, persistence mailboxes,
record executors, and Worker actors. A new layer must converge those existing
shapes rather than sit beside them permanently. The first tactical should
identify which current adapter and ledger paths are deleted by adoption.

### Error flattening

Returning only strings would preserve today's loose browser callback but lose
the typed error work from unified persistence. Today the browser completion
is a world-ID string whose empty error string means success, and the rich
provision outcome (reused versus published versus repaired, record counts,
timings) never reaches Rust at all. Shared failures should reuse the stable
`PersistenceErrorKind` categories from unified persistence rather than a new
taxonomy, and the typed success completion should carry the provisioning
outcome so hosts and smokes stop reading it from TypeScript state.

## Alternatives Considered

### Keep TypeScript as the browser workflow coordinator

This is operational today and keeps asynchronous IndexedDB code familiar, but
it leaves validation/result policy split between Rust and TypeScript. Every new
managed-content behavior increases the divergence and source-lock surface. It
does not meet the ownership goal.

### Make all engine storage and platform traits `async`

An async trait or future could express browser suspension naturally. Making
simulation and native call sites promise-shaped would spread scheduling,
borrowing, runtime, cancellation, and executor choices through the engine.
Native still needs background dispatch to avoid blocking. The existing
completion port fits the tick/poll host and keeps async mechanics behind the
executor.

An internal future inside a browser or native driver remains a possible
implementation of a platform strategy; the objection is to making the
engine-facing contract await a platform future directly.

### Share only request/result types and keep separate algorithms

This makes the outer API look uniform while retaining two provisioners. It
cannot prevent semantic drift and gives weak value over the current state.
The decision structure, not merely the DTOs, must have one Rust owner.

### Share policy and conformance, but retain Rust platform strategies

This is the selected direction. Native atomic directory publication and
browser per-world IndexedDB publication are physically different. A shared
coordinator defines recipes, validation requirements, acceptable conflict
outcomes, and typed completion semantics while delegating publication
mechanics to native and web Rust strategies.

Sequencing drift in platform Rust is the residual risk, but the workflow has
only a handful of steps and the conformance suite pins their outcomes; this
was judged cleaner than a universal storage-effect language. Conformance
fixtures and exact outcome traces must make the allowed physical variation
explicit, and TypeScript must remain a mechanics-only executor.

### Generate a complete write plan before touching storage

This works only when the operation does not depend on current stored state.
Managed reuse, corruption refusal, repair and conflict recovery are
read-dependent. A plan may describe each individual transaction, but a Rust
continuation still needs to choose subsequent plans from results.

### Run the native backend through encoded actor frames too

That would make transport artifacts superficially identical at the cost of
unnecessary encoding, copying and indirection on native. Shared semantics do
not require shared bytes. Native should move typed Rust values and call its
executor directly.

### Put all browser Workers in one shared Wasm heap

A shared heap could reduce selected copies, but it does not remove IndexedDB's
asynchronous browser API or provide the operation policy automatically. It adds
allocator, lifetime, lock, termination and crash-recovery risk. The isolated
actor design solves the ownership problem first and leaves shared heap as an
independent measured option.

### Move all IndexedDB mechanics into web-specific Rust immediately

This can make TypeScript smaller and may ultimately be attractive. It does not
change the necessary engine-facing operation port or shared provision task.
Using the existing generic TypeScript executor first is lower-risk and keeps
the direct-`web_sys` choice replaceable beneath the same contract.

## Directional Invariants

Any reviewed variant should preserve these requirements:

1. Shared scene/application code sees one typed request and one typed
   completion, never platform storage actions.
2. One shared Rust owner decides materialization, validation, reuse, repair,
   corruption refusal, conflict acceptance, and final result, or one shared
   policy coordinator constrains Rust platform strategies to demonstrably
   equivalent outcomes where their physical representations differ.
3. Native does not encode browser frames, copy each record across threads, or
   use browser synchronization mechanics.
4. The browser main thread never blocks or spins.
5. No mutable Rust borrow or JavaScript view lives across a browser `await`.
6. TypeScript executes browser mechanics without interpreting managed-world or
   engine-record meaning.
7. Physical IndexedDB schema knowledge is isolated from domain policy.
8. Ordinary user worlds and managed scenario worlds retain separate catalog,
   deletion, and authority domains.
9. The first cut preserves existing SQLite/filesystem and IndexedDB data,
   schema versions, keys, record bytes, and transaction atomicity.
10. Stale, duplicate, cancelled and unknown completions cannot mutate current
    scene state.
11. Large payloads stay in the background execution context rather than
    transiting the main engine unnecessarily.
12. A production cut deletes the old browser policy path rather than retaining
    an indefinite dual implementation.

## Questions For Review

The 2026-07-20 review resolved most of these; resolutions are recorded
below. The remainder stay open for the implementing tactical.

Resolved:

1. `WarmWorldLaunch` should adopt `PlatformOperationService` directly. Its
   specialized ledgers and pending queues exist only because the web side
   pulls requests, which is exactly what `DeferredPlatformOperationHandle`
   provides. Adoption deletes duplicate queue plumbing and is a good first
   slice.
2. `NativeManagedScenarioProvisionAdapter` should implement the existing
   `PlatformOperationExecutor` trait as-is. Its `submit`/`poll` already
   match, and `world_dir` remains an adapter-specific accessor; no trait
   changes are needed first.
3. An explicit enum phase machine is clearer. The workflow has at most a
   handful of transaction-aligned suspensions, so an internally driven
   future adds machinery without value; native runs to completion on its
   worker thread.
4. Administrative operations come from generalizing the catalog
   transaction-plan executor (`web_catalog_execution.rs`), which already
   provides scan, conditional add, range delete, clear, and multi-store
   atomic transactions, rather than extending `PersistenceRecordExecutor`
   or standing up a fourth vocabulary.
5. Largely already solved: TypeScript treats the `managedWorlds` metadata
   envelope as an opaque value today and passes it verbatim; only Rust
   decodes it. Preserve that property through the cutover.
9. Reuse `PersistenceErrorKind` across the outer port rather than a second
   taxonomy, and make the success completion typed so it carries the
   provisioning outcome.
11. The engine operation stays per-world, matching shared launch demand and
    the scene's conditional destination sequencing. Whole-scenario staging
    remains a native execution detail whose idempotent atomic rename makes
    concurrent per-role requests safe.
12. No. The backends overlap only at the classification layer, which is
    already shared Rust; the shared owner is the decision core plus
    conformance, not a normalized storage language.

Open for the implementing tactical:

6. Should the one-shot provisioning Worker remain one-shot for isolation, or
   should a resident service amortize Wasm initialization after measurement?
7. Is thread-per-operation still appropriate on native, or should adoption
   immediately use an existing bounded worker pool?
8. Should physical IndexedDB schema descriptors stay in the TypeScript adapter
   or become Rust-authored data executed by a generic upgrader?
10. Which common lifecycle vocabulary is useful to compute Workers and socket
    actors without forcing them behind the same physical executor?

New decision surfaced by review:

13. Native and browser stored-state policy already diverge (see Current
    State). The shared decision core forces one answer per stored-state
    class and one validation depth; the implementing tactical must choose
    them deliberately — including whether native gains deep record
    validation and repair — rather than inherit whichever platform is
    ported first.

These resolutions may change the exact implementation. They do not change
the motivation: one clean engine operation, one Rust policy owner, thin
platform glue, and no material native regression.

## Possible Future Tactical Sequence

No tactical is authorized by this document. A likely bounded sequence after
review is:

1. **Baseline and contract convergence.** Record native/browser transaction
   traces, timings, allocation/copy facts, TypeScript ownership, and current
   failure semantics. Route native provision submission/polling through the
   accepted operation executor shape without changing behavior.
2. **Shared decision-core laboratory.** Implement the shared decision core
   and conformance suite against in-memory fake strategies, resolving the
   repair-versus-refusal and validation-depth divergence explicitly. Prove
   valid reuse, missing publication, partial and incompatible repair, corrupt
   refusal, conditional conflict recovery, cancellation epochs, and exact
   outcome traces.
3. **Native driver proof.** Drive the native strategy under the shared
   decision core in the native background adapter using direct Rust storage.
   Preserve existing
   staged-directory/filesystem layout and compare latency, allocation, thread
   count and record copies.
4. **Generic browser action proof.** Add only the lower storage actions required
   by the shared traces. Prove current IndexedDB v6 records and transaction
   atomicity without touching production provisioning.
5. **Browser Rust actor cutover.** Put the constrained web Rust strategy in
   the provisioning Worker, reduce main and Worker TypeScript to
   opaque action/completion forwarding, and delete the old TypeScript state
   machine atomically.
6. **Lifecycle and performance hardening.** Exercise cancellation, concurrent
   publication, hidden/resume, Worker failure, quota/error classification,
   repeated launches, and background memory release.
7. **Closeout and reassessment.** Update source locks, line/ownership/copy
   ledgers, current architecture docs, and decide whether session-start or
   another coarse operation should adopt the pattern next.

The task-versus-strategy sharing degree and lower storage-port variation
were chosen in the 2026-07-20 review. Further human review should occur
before resolving the repair-versus-refusal divergence, and again if
implementation would require a physical schema migration, a new
shared-Wasm-memory topology, weaker atomicity, or measurable native
overhead.

## Validation Expectations

### Shared Rust

- exact transition traces for every validation/publication outcome;
- deterministic memory-executor fault injection at every action boundary;
- stale, duplicate, unknown and cancelled completion tests;
- idempotent conflict/lost-ack recovery;
- bounded batch and retained-byte assertions; and
- no platform types in task or operation contracts.

### Native

- unchanged managed primary/destination world identity and contents;
- reopen and corruption behavior;
- failure and retry without simulation-thread blocking;
- request/completion counts and thread lifecycle;
- bytes copied or serialized across the operation boundary;
- wall time and allocation compared with the current adapter; and
- desktop plus affected Android/Quest packaging and lifecycle gates if shared
  native provisioning or storage code changes.

### Browser

- current managed storage reuse, cancellation, concurrent publication, repair,
  corrupt refusal, and ordinary-world isolation smoke;
- lobby primary/destination launch, preview and activation behavior;
- hidden/resume and forced Worker failure containment;
- quota, unavailable storage, transaction abort and malformed completion;
- no domain vocabulary in production TypeScript;
- Worker/Wasm initialization, request/completion, bytes and retained-memory
  metrics;
- maximum frame gap and main-thread work during provisioning; and
- rendered output captured under `/tmp` and inspected at the first drawable
  cutover milestone.

### Cross-platform equivalence

The same logical fixtures should yield the same normalized policy outcome and
typed completion on native and browser simulated executors. Review selected
Rust platform strategies over one normalized task, so a shared conformance
trace should record the allowed physical differences while validation,
conflict, repair, failure and epoch semantics remain identical.

## Code And Documentation Map

Shared operation and scenario policy:

- `native/crates/mclone-app-runtime/src/platform_operation.rs`
- `native/crates/mclone-app-runtime/src/scenario_content.rs`
- `native/crates/mclone-app-runtime/src/scenario_content/native.rs`
- `native/crates/mclone-scene/src/warm_world.rs`
- `native/crates/mclone-scene/src/session.rs`

Current browser path:

- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/src/web_canvas.rs`
- `native/apps/mclone-web-client/www/mclone-web-app.ts`
- `native/apps/mclone-web-client/www/mclone-managed-scenario-provision-worker.ts`
- `native/apps/mclone-web-client/www/mclone-web-world-catalog.ts`
- `native/apps/mclone-web-client/www/mclone-web-persistence-executor.ts`
- `native/apps/mclone-web-client/src/web_catalog_execution.rs`

Lower persistence and Worker context:

- `native/crates/mclone-server/src/persistence/record_executor.rs`
- `native/crates/mclone-server/src/persistence.rs` (`PersistenceErrorKind`)
- [`unified-persistence-interface.md`](unified-persistence-interface.md)
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md)
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md)
- [`../tactical/197-domain-blind-web-worker-broker.md`](../tactical/197-domain-blind-web-worker-broker.md)
- [`../tactical/198-opaque-websocket-and-indexeddb-adapters.md`](../tactical/198-opaque-websocket-and-indexeddb-adapters.md)
- [`../tactical/199-unified-persistence-interface.md`](../tactical/199-unified-persistence-interface.md)

## Recommended Direction

Accepted by the 2026-07-20 review:

- retain `PlatformOperation` request/completion semantics as the engine-facing
  model, with per-world operations at the port;
- converge native and web managed provisioning on one shared pure decision
  core — stored-state classification to action, plus the conflict rule —
  with narrow Rust platform strategies held to it by a conformance suite;
- drive the native strategy directly in a background thread and the web
  strategy as an explicit enum phase machine in a browser Rust actor whose
  suspensions align with whole IndexedDB transactions;
- generalize the catalog transaction-plan executor as the browser's
  domain-blind storage executor instead of adding a fourth vocabulary;
- reuse `PersistenceErrorKind` and make the success completion carry the
  typed provisioning outcome;
- resolve the native/browser repair-versus-refusal and validation-depth
  divergence as an explicit decision in the decision-core laboratory;
- keep TypeScript responsible only for browser API mechanics; and
- prove that native retains direct typed execution with no material
  serialization, copy, scheduling, or latency penalty.

Do not begin by merely moving the current TypeScript branches line-for-line
into web-specific Rust. First establish the shared operation and decision
core boundary so the browser cut removes a competing policy implementation
rather than only changing its language.
