# Cross-Platform Operation Execution

Topic: `cross-platform-operation-execution`

Status: high-level actor/mailbox direction accepted 2026-07-20. The topic was
reframed after review to make the logical execution topology, rather than
managed-storage mechanics, the governing architecture. Managed-scenario
provisioning remains the motivating first consumer and a directional example,
not an authorized tactical. Its validation and recovery policy still requires
an explicit implementation review.

## Top-Level Frame

> What is Mclone's shared logical concurrency model, and how do native threads
> and browser Workers realize that model without duplicating engine policy,
> forcing browser transport costs onto native, or letting TypeScript become a
> second engine?

The answer is an actor/mailbox model whose semantic owner is shared Rust:

```text
caller
  |
  v
typed mailbox
  owned requests, completions, identity, backpressure, and shutdown
        |
        v
platform driver
  native: channel + OS thread
  web: Worker + message/SAB + browser callbacks
        |
        v
shared Rust actor
  owns subsystem state, sequencing, policy, and lifecycle
        |
        v
optional effect port -> platform Rust effect adapter
  native: direct SQLite / filesystem / socket calls
  web: lower high-level effects into generic browser actions
        |
        v
      TypeScript browser-API executor
        IndexedDB / WebSocket mechanics only
```

The actor is the semantic owner. The driver and platform Rust effect adapter
are platform implementations; TypeScript is only the final browser-API
executor. Native and web should have the same logical owners and
request/completion semantics even when they use different thread counts,
memory layouts, byte transports, or asynchronous APIs.

This is not a proposal for one universal Worker ABI, one shared database
service, or one trait that erases every workload difference. It is a common
ownership topology that specialized mailboxes and transports implement
faithfully.

Managed-scenario provisioning exposed the gap because its browser path still
places a domain state machine in TypeScript. The solution is not merely to move
those branches into web-specific Rust. Provisioning should become another
shared Rust actor whose native driver executes filesystem/SQLite effects
directly and whose browser driver suspends around a Worker-resident Rust effect
adapter plus domain-blind TypeScript IndexedDB execution.

## Vocabulary

| Term | Meaning |
|---|---|
| **Actor** | The single Rust owner of a subsystem's mutable domain state, sequencing, decisions, and lifecycle. |
| **Mailbox** | The typed, owned request/completion boundary around an actor. It defines semantic identity, admission, backpressure, and closure, not a universal byte encoding. |
| **Driver** | The platform mechanism that gives the actor execution turns and transports mailbox values: native thread/channel, browser Worker/message/SAB, or an inline test fallback. |
| **Effect port** | The narrow actions an actor may request from platform resources without learning paths, browser objects, promises, or physical schema names. |
| **Effect adapter** | Platform Rust that implements a high-level effect: direct native calls, or browser-specific lowering into generic browser actions. |
| **Browser-API executor** | Domain-blind TypeScript that executes the lowered IndexedDB, WebSocket, Worker, promise, or other browser mechanics. |
| **Assembly** | Platform code that binds one actor and mailbox to an appropriate driver, effect adapter, and browser-API executor where required. |

An actor may call a synchronous effect adapter while running on a native worker
thread. The same actor may emit an owned effect request and resume later in a
browser Worker. That physical difference must not create two semantic actors.

“Opaque TypeScript” means semantically opaque, not unable to inspect any
mechanical envelope. TypeScript may see a request ID, stable storage namespace,
buffer length, transaction mode, or SAB status word. It must not decide what a
scenario, chunk, validation result, retry, revision, or engine completion
means.

## Logical And Physical Topologies

### Shared logical roles

Mclone's important asynchronous roles are:

```text
client / scene / renderer authority
  |
  +-- integrated-server actor
  |     +-- worldgen job actor(s)
  |     +-- lighting job actor(s)
  |     `-- persistence actor
  |
  +-- render-compiler actor
  |
  +-- remote-socket actor
  |
  `-- coarse platform-operation actors
        catalog, managed provisioning, session start, future bounded work
```

These roles describe state ownership and communication. They do not require a
dedicated physical thread for every box. Assembly may co-locate an actor with
an adapter when doing so preserves nonblocking behavior and ownership.

### Current native topology

The production native shape is approximately:

```text
client/render thread
  |
  +-- render compiler worker thread(s)
  |
  `-- commands / updates
        |
        v
      integrated-server runner thread
        |
        +-- worldgen worker(s)
        +-- lighting worker(s)
        `-- persistence mailbox
              |
              v
            mclone-persistence thread
              `-- owns SQLite connection + world writer lease
```

`ThreadedPersistenceActor` moves the `WorldStore`, including the SQLite
connection and writer lease, into one named thread. The server sends typed
`WorldStoreRequest` values and polls typed completions. Arbitrary compute
workers do not open the writable database.

Native managed provisioning is currently another coarse thread-backed
operation. It stages a complete scenario directory, creates per-world SQLite
stores, and publishes the directory through atomic rename. That direct typed
execution is a strength to preserve.

### Current browser topology

The production browser shape is approximately:

```text
browser main thread
  TypeScript browser mechanics
  main Rust scene/client/render
  WebGPU presentation
  |
  +-- message/SAB <-> render Worker
  |                    `-- Rust render actor in a private Wasm heap
  |
  `-- message/SAB <-> integrated-server Worker
                       TypeScript timer/browser shell
                       Rust server actor in a private Wasm heap
                       |
                       +-- message/SAB <-> worldgen/light Workers
                       |                    `-- Rust job actors
                       |
                       `-- Rust record requests
                              |
                              v
                            TypeScript IndexedDB executor
                            in the same Worker realm
                              `-- IndexedDB
```

The lobby may retain two integrated-server Workers, one for each live world,
while one render-compiler Worker multiplexes qualified world identities.

The browser persistence adapter does not round-trip records through the main
browser thread. Worker-resident Rust emits generic record requests; adjacent
TypeScript executes IndexedDB transactions and returns completions to that
same Rust actor. The external `SharedArrayBuffer` mailboxes used by compute and
runner lanes are shared byte lockers between private Wasm heaps, not a shared
Rust object heap.

### Topology equivalence

| Logical role | Native realization | Browser realization |
|---|---|---|
| client/scene | app/render thread | browser main Rust plus rAF/WebGPU adapter |
| integrated authority | server runner OS thread | integrated-server Web Worker |
| worldgen/light jobs | Rust worker threads | Rust actors in job Web Workers |
| persistence owner | Rust persistence thread with direct SQLite | server-Worker Rust actor plus adjacent TypeScript IndexedDB executor |
| render compiler | Rust worker thread/pool | Rust actor in render Web Worker |
| coarse operation | Rust worker thread | Rust actor in a suitable Web Worker |

The persistence row is physically asymmetric but logically equivalent. Native
can block its dedicated storage thread while SQLite completes. Browser Rust
must return to the event loop while IndexedDB completes, then resume. Both
retain one Rust policy owner and one request/completion boundary.

## Vanilla Reference Shape

Minecraft Java 1.17.1 provides an instructive ownership model even though
Mclone should not copy its Java futures or executor classes literally.

`IOWorker` owns one `RegionFileStorage`, its pending-write map, coalescing,
foreground/background/shutdown priorities, flush, and close. Callers submit
work through a `ProcessorMailbox` and receive `CompletableFuture` results; they
do not all access the region files directly.

`ProcessorMailbox` owns a serialized queue independently of the `Executor`
that schedules it. The same logical mailbox can therefore be driven by a pool
without moving subsystem state or policy into the scheduler.

`ThreadedLevelLightEngine` follows the same principle for light work: it queues
pre- and post-update tasks through mailboxes and rejects direct execution of
methods that belong on the threaded path.

Relevant source:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/IOWorker.java`
- `reference/minecraft-1.17.1/src/net/minecraft/util/thread/ProcessorMailbox.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`

The lesson for Mclone is “mailbox-owned subsystem state,” not “use Java's
executor implementation.” Shared Rust defines each logical actor and mailbox;
platform assembly chooses the execution substrate.

## Scope

This topic governs bounded and resident asynchronous subsystem execution where
native and browser hosts need the same semantic owner:

- integrated server and server-job actors;
- persistence actors and physical record executors;
- render compilation;
- browser socket actors;
- world catalog operations;
- managed-scenario provisioning; and
- future coarse platform operations.

Managed provisioning is the first unresolved worked example. The topic does
not require existing proven actors to adopt `PlatformOperationExecutor`
literally. Persistence, compute, rendering, sockets, and coarse operations may
retain specialized mailbox types and transports while satisfying the same
ownership model.

Out of scope:

- one universal physical Worker, broker, mailbox ABI, or scheduling policy;
- shared Wasm linear memory as a prerequisite for semantic convergence;
- forcing native through encoded browser frames or SAB-shaped mechanics;
- allowing arbitrary compute workers to access writable world storage;
- folding catalog, managed provisioning, and opened-world policy into one
  catch-all API; and
- runtime storage-schema migration or upgrade semantics. The first cut keeps
  existing physical schemas unchanged; any future migration campaign requires
  separate pre-admission/offline review.

## Directional Invariants

1. Every asynchronous engine subsystem has one Rust owner for domain state,
   sequencing, decisions, and lifecycle.
2. Callers communicate with that owner through typed, owned requests and
   completions.
3. Native threads and browser Workers are drivers of the mailbox, not alternate
   policy owners.
4. Platform adapters may specialize transport, batching, copies, memory
   placement, and physical API mechanics.
5. TypeScript may execute browser mechanics but must not interpret engine
   meaning or advance a domain state machine.
6. Storage belongs to an owning world session or bounded administrative actor,
   not to arbitrary compute workers.
7. Native remains free to move typed values, share immutable Rust allocations,
   and call SQLite/filesystem code directly on a worker thread.
8. The browser main thread never blocks or spins, and no mutable Rust borrow or
   JavaScript view crosses an `await`.
9. Private Wasm heaps and failure containment may remain even when external
   SAB mailboxes carry bytes between them.
10. Request identity, epochs, stale/duplicate rejection, typed errors, and
    shutdown semantics remain Rust-owned.
11. Ordinary user worlds and managed scenario worlds retain separate catalog,
    deletion, and authority domains.
12. A production cut deletes the superseded policy path instead of keeping
    indefinite native/web or Rust/TypeScript dual implementations.

## Common Actor And Mailbox Semantics

The actor/mailbox family should converge on these observable properties where
the workload needs them:

- owned request and completion values;
- stable request identity;
- nonblocking submission from latency-sensitive callers;
- bounded queues and explicit backpressure;
- deterministic ordering where requests affect the same state;
- stale, duplicate, cancelled, and unknown completion handling;
- typed failure plus backend diagnostics;
- explicit flush/close or quiescence barriers;
- actor-owned retries, revisions, supersession, and result acceptance; and
- metrics for queue depth, bytes, latency, retained state, and failure.

Not every mailbox needs every feature. A render compiler and a storage actor do
not need identical durability or buffer semantics. Shared lifecycle vocabulary
is useful only where it describes the same observable contract honestly.

### Workload-specific transports

| Work profile | Shared semantic shape | Native driver | Browser driver |
|---|---|---|---|
| integrated authority | commands, updates, cadence, readiness, shutdown | server thread/channel | Worker actor plus message/SAB transport |
| worldgen/lighting | queued jobs, resident state, bounded results | worker threads/pool | job Workers plus transfer/SAB |
| render compilation | qualified jobs, revision acceptance, resident compiler state | thread/pool | render Worker plus external SAB |
| opened-world persistence | ordered records, revisions, barriers, one world owner | persistence thread with direct SQLite | server-Worker Rust plus IndexedDB callbacks |
| managed provisioning/catalog | coarse request, read-dependent state machine, final completion | worker thread with direct effects | Worker Rust actor plus browser actions |
| remote socket | commands, events, backpressure, close | native socket runtime | Worker Rust actor plus WebSocket callbacks |

The shared model is a family of actors and mailboxes, not a lowest-common-
denominator byte broker.

## Existing Engine-Facing Operation Port

`mclone-app-runtime::platform_operation` already provides a suitable outer
mailbox for coarse platform operations:

```rust
pub trait PlatformOperationExecutor<K, T, E> {
    fn submit(&mut self, operation: PlatformOperation<K>);
    fn try_recv_completion(
        &mut self,
    ) -> Option<PlatformOperationCompletion<T, E>>;
}
```

It also provides:

- target-neutral tokens;
- `PlatformOperationService`;
- deferred executors/handles for platform adapters;
- epochs and monotonically increasing request IDs;
- pending-request ownership and failure restoration; and
- stale, duplicate, or unknown completion classification.

No `JsValue`, promise, Worker, socket, path, or database handle enters this
ledger. Shared scene policy should submit one typed request and later receive
one typed completion. It must not drive internal storage steps.

This outer port is one concrete mailbox in the larger topology. It should not
replace the specialized persistence, compute, render, or socket actor contracts
where those carry different performance and lifecycle requirements.

## Managed Provisioning As The First Worked Actor

### Current shared policy

`mclone-app-runtime::scenario_content` already owns storage-neutral:

- `ProvisionManagedScenarioWorld` and its typed result;
- scenario recipes, manifests, roles, and managed-world keys;
- authored fixture materialization;
- expected payload fingerprints and metadata;
- stored-world classification; and
- path-free launch identities.

`mclone-scene::warm_world` already owns operation identity, admission, launch
ordering, destination deferral, and stale-completion behavior. The remaining
gap is the inner provisioning workflow.

### Current divergence

Native currently runs a coarse Rust workflow on a background thread. It stages
and atomically renames one versioned scenario directory containing both worlds,
manifests, and per-world SQLite databases. Existing invalid content is refused;
validation checks manifests, markers, and SQLite headers.

Browser provisioning runs in a one-shot Worker but TypeScript currently:

- requests payload materialization from Rust;
- reads managed metadata and record stores;
- asks Rust to classify the result;
- decides reuse, repair, publication, or refusal;
- clears and republishes record families;
- implements conditional first publication and conflict recovery; and
- constructs the rich loose success report retained by main-thread TypeScript.

Rust owns the individual facts while TypeScript owns the state machine that
connects them. Native and browser behavior has consequently diverged: browser
repairs `partial` and `incompatible` worlds and refuses `corrupt`; native does
not repair and validates less deeply. The rich browser outcome never reaches
Rust: main TypeScript forwards only `worldId` plus an empty error string on
success through `completeManagedScenarioProvision`.

The shared five-way `Missing`/`Valid`/`Partial`/`Incompatible`/`Corrupt`
classifier also has only a browser production caller today. Native does not
call `validate_managed_scenario_stored_world`; it independently validates
manifests, markers, and the 16-byte SQLite header. Convergence therefore needs
one actor-owned classification contract as well as one actor-owned response to
each classification.

### Target actor

One shared Rust provisioning actor should own this semantic phase sequence:

```text
Start
  -> resolve recipe and materialize expected content
  -> request stored-state inspection
  -> classify according to shared validation policy
  -> decide reuse / exclusive publish / authorized replacement / refusal
  -> request one physical publication effect when needed
  -> on conflict, re-inspect once and evaluate the winner
       accept a valid winner or fail; never issue a second replacement
  -> produce one typed completion
```

The actor, not a native or web strategy, owns phase ordering, maximum conflict
reinspection, validation requirements, repair authority, result construction,
and final failure semantics.

The actor contains no path, SQLite connection, `JsValue`, promise, Worker,
socket, or IndexedDB object. Every input and suspended intermediate value is
owned. Trace tests can advance it with in-memory observations and effect
results.

### Platform effect adapters

The actor requests high-level effects rather than one normalized database
language:

```text
InspectManagedWorld
PublishManagedWorldExclusively
ReplaceManagedWorldAtomically   // only when shared policy authorizes it
OpenOrResolveProvisionedWorld
```

Exact names may differ. The important split is that the actor decides which
effect is allowed and the adapter decides how to perform it.

Native effects may:

- inspect filesystem manifests and SQLite content directly;
- build both roles in one staging directory;
- publish both through one atomic rename even though the outer operation is
  per-world;
- report `committed`, `conflict`, or physical failure for that rename attempt
  without revalidating or accepting a competing publisher internally;
- use owned Rust values and `Arc` without serialization; and
- return the requested role's logical identity while retaining native path
  resolution in platform assembly.

Browser effects are lowered by Worker-resident Rust. That lowering may:

- compile one actor-requested high-level effect into a transaction plan;
- inspect the required IndexedDB records;
- conditionally add the managed metadata commit marker;
- atomically replace the authorized record families;
- report commit, constraint conflict, abort, or storage failure; and
- keep materialized bytes inside the provisioning Worker.

The lowering necessarily knows which record families, keys, metadata envelope,
and transaction grouping implement one managed-world effect. That is
platform-specific domain knowledge and therefore remains Rust-owned.
TypeScript receives only the resulting generic, store-addressed actions and
executes their IndexedDB mechanics.

Native whole-scenario publication and browser per-world publication are honest
physical differences. They do not justify separate semantic phase machines.
The current native `ensure_current_lobby_preview` resolves a rename loser by
revalidating the winner internally; actor adoption must split that behavior so
`PublishManagedWorldExclusively` reports the conflict and the shared actor
requests the one allowed re-inspection.

### Native driving

The native driver moves the typed request into a background worker and advances
the shared actor to completion against direct Rust effects:

```text
receive PlatformOperation once
advance shared actor
call native effects directly when requested
send PlatformOperationCompletion once
```

The native driver must not turn a publication conflict into success on its own.
It returns the conflict observation to the actor, which owns the re-inspection
and final acceptance decision just as it does on web.

Convergence is unacceptable if native must encode browser frames, copy every
record through a generic byte mailbox, use SAB-shaped atomics, perform storage
on the simulation thread, or lose staged-directory atomicity.

### Browser driving

The browser provisioning Worker hosts the same actor in its private Wasm heap:

```text
decode opaque operation frame
advance shared Rust actor until it requests an effect
lower that effect into generic actions in Worker-resident Rust
return the generic actions to TypeScript
await the whole IndexedDB transaction
return the mechanical transaction result to Worker Rust
translate it into an owned effect result and resume the actor
repeat until the actor produces its typed completion
post one opaque completion frame
```

An explicit enum phase representation is appropriate because provisioning has
only a few transaction-aligned suspension points. That enum belongs to the
shared actor, not to a web-only workflow. Native may hide it behind a
`run_to_completion` driver.

IndexedDB transactions auto-commit when control returns to the event loop, so
an actor suspension aligns with a whole transaction. Read-dependent actions
that must remain in one transaction are emitted synchronously from Rust inside
the relevant success callback, as the existing catalog continuation already
demonstrates.

## Storage Access And Resolution

The operation mailbox and record/transaction executor are distinct:

- the operation mailbox isolates callers from where and how the actor runs;
- the actor owns managed-provisioning meaning and sequencing; and
- the record/transaction executor isolates physical storage mechanics.

Writable storage is not a global service that every worker may access. It
belongs to an owning actor/session:

- a live world persistence actor owns its writer-capable store for the world
  lifetime;
- worldgen, lighting, and render workers return compute results and do not
  independently persist them;
- a managed provisioning actor owns a bounded administrative storage context
  before handing the world to a live session; and
- read-only inspection is an explicitly non-writing capability.

SQLite and IndexedDB may physically serialize concurrent transactions, but
that does not establish game-level ordering, revision precedence, or one
authoritative writer. Those remain actor/session policy.

### Logical world identity

Shared scene policy already carries a discriminated, path-free
`ScenarioWorldStorageSource`:

```text
Managed(ManagedWorldKey)
Catalog(LocalWorldId)
```

Shared code chooses the logical source but never maps it to a filesystem path,
IndexedDB object, or SQLite connection. Platform world-start/session assembly
consumes the discriminated source and resolves an appropriate physical storage
session. The managed-versus-catalog domain must remain visible until that final
adapter because their catalog, deletion, and authority rules differ.

The native provision adapter's current `ManagedWorldKey -> PathBuf` map may
remain an implementation cache, but should not become the cross-platform
contract. The browser may use the logical key as a physical record prefix, but
that is likewise an adapter mapping rather than shared policy.

### Live store versus administrative provisioning

Managed provisioning should not be added to the live `WorldStore` policy API.
`WorldStore` owns opened-world records, revisions, pending-write visibility,
flush, and close. Provisioning owns inspection, conditional publication,
authorized replacement, reuse, and conflict recovery. They may reuse a lower
record/transaction executor while retaining different actors and lifetimes.

## Browser TypeScript Boundary

### TypeScript may own

- Worker construction, module loading, timers, yields, and termination;
- `postMessage`, transfer lists, SAB typed views, atomics, and wakeups;
- IndexedDB open, object-store/index mapping, requests, cursors, transactions,
  promises, aborts, and structured-clone envelopes;
- WebSocket construction and browser callbacks;
- stable mechanical action tags, namespace identifiers, request IDs, byte
  buffers, and generic browser error forwarding; and
- physical schema names and key paths required by the unchanged IndexedDB
  adapter.

### TypeScript must not own

- scenario IDs, roles, recipes, validation states, or recovery authority;
- chunk, entity, dimension, gameplay, or render-job interpretation;
- actor phase sequencing, retries, conflict acceptance, or completion policy;
- request admission, active/standby priority, revisions, supersession, or stale
  result handling;
- construction of engine success/failure variants; or
- independent mirrors of Rust domain state.

A practical acceptance test is:

- adding a scenario, content version, validation rule, or recovery decision
  requires no TypeScript change;
- changing an IndexedDB store/index/key path may require TypeScript adapter
  work; and
- adding a genuinely new generic physical transaction primitive may require a
  mechanical change on both sides.

### Generic IndexedDB transaction executor

The existing catalog runner is the correct browser precedent. Rust owns the
catalog continuation and emits storage actions. TypeScript maps stable store
and index identifiers, executes transactions, returns reads, and reports
completion.

Managed provisioning should generalize that transaction-plan executor rather
than extend the opened-world `PersistenceRecordExecutor` indiscriminately or
create a fourth browser storage vocabulary. The generalized physical actions
should preserve the seven demonstrated mechanical kinds: `get-all`, `get`,
`add`, `put`, `delete-key`, `delete-index-range`, and `clear`, generalized over
opaque values rather than managed- or catalog-specific TypeScript helpers.

The tactical must also choose one stable addressing scheme for this generalized
executor. The catalog plan currently names stores and indexes with string
labels, while the opened-world record executor uses numeric namespaces plus
typed key parts. Reusing mechanics does not justify exposing two overlapping
address vocabularies indefinitely or casually treating one as the other.

The first cut keeps IndexedDB version 6, existing store/index names, key paths,
record bytes, and transaction atomicity. Physical schema migration and a
Rust-authored schema-upgrade language are out of scope.

## Managed Validation And Recovery Policy

Convergence cannot simply copy either current backend.

The browser currently reads and decodes every managed chunk and entity-chunk
record. That is bounded for a small authored fixture but not for the mutable
generated destination, whose initial payload has no authored records and whose
stored world may grow indefinitely. “Deep validation” therefore cannot mean a
full-world scan on every launch.

The shared recipe should distinguish two policy dimensions:

```text
ValidationDepth
  BoundedAuthoredFootprint
  IdentityAndLazyRecords

RecoveryAuthority
  Reconstructible
  PreserveRuntimeState
```

- `BoundedAuthoredFootprint` deeply validates the finite content the recipe is
  required to provide.
- `IdentityAndLazyRecords` validates compatible publication identity and
  physical availability, while ordinary record loads validate record codecs
  lazily.
- `Reconstructible` permits atomic replacement after an invalid observation.
- `PreserveRuntimeState` refuses destructive repair until a separately
  authorized recovery/reset action exists.

Recovery authority must be explicit recipe/storage policy, not inferred from a
gameplay behavior profile. A protected world and a mutable world may have
different likely defaults, but gameplay edit rules do not by themselves grant
permission to erase persisted state.

The shared actor's logical decision is then a function of stored-state class,
validation depth, and recovery authority:

| Observation | Shared action |
|---|---|
| missing | publish conditionally |
| valid | reuse |
| partial/incompatible/corrupt + reconstructible | atomically replace if the reviewed recipe permits it |
| partial/incompatible/corrupt + preserve runtime state | refuse without deleting |
| publication conflict | re-inspect once; accept a state valid under the same policy or fail, with no second replacement attempt |

The implementing tactical must explicitly classify the current primary and
destination recipes and decide whether `corrupt` is ever automatically
replaceable. The safe default is preservation/refusal unless a recipe opts
into reconstruction. Existing content keys are versioned, so incompatible
content at the current key is abnormal rather than the ordinary upgrade path.

## Lifecycle And Failure Semantics

The shared actor/mailbox contract owns semantic lifecycle; the driver owns
best-effort physical cancellation.

Epoch invalidation is authoritative: a late completion cannot affect a
replacement scene. A native worker may finish already-running work; a browser
adapter may abort an IndexedDB transaction or terminate a Worker. The shared
contract should not promise immediate physical cancellation where a backend
cannot provide it.

A tactical must cover:

- submission failure before work begins;
- thread/Worker construction failure;
- storage open or authority failure;
- cancellation before, during, and after a physical transaction;
- completion after the scene epoch changed;
- duplicate, unknown, or malformed completion;
- actor panic, Worker error, or forced termination;
- commit followed by lost acknowledgement;
- concurrent first publication;
- orderly shutdown with pending work; and
- retry without wedging the actor or applying stale state.

The operation ledger decides whether a completion may affect current scene
state. The actor decides whether an observed world is acceptable. The physical
adapter reports what its transaction did. None may infer another layer's state
from a timeout alone.

At-most-once physical execution is not always provable after a lost browser
acknowledgement. Retried effects must therefore be idempotent or followed by
actor-owned reinspection. Managed content's versioned identity and conditional
publication are suitable for that contract.

## Current Assets To Reuse

The architecture should converge existing components rather than introduce a
parallel framework:

- `PlatformOperationService` and `PlatformOperationLedger` for coarse operation
  identity and lifecycle;
- `PersistenceMailbox`, `PersistenceActor`, and
  `ThreadedPersistenceActor` for mailbox-owned world storage;
- native and Wasm server-job mailboxes for worldgen/light actors;
- `WebRenderWorkerCoordinator` and `WebRenderWorkerActor` for render ownership;
- `WebIntegratedServerActor` for browser authority/session ownership;
- `WebRemoteSocketWorkerActor` for socket protocol ownership;
- `WebCatalogExecution` for a Rust continuation over browser transactions; and
- the generic browser Worker transport and opaque frame conventions.

These implementations prove the actor/mailbox direction. They need not be
collapsed into one type hierarchy.

## Decisions Accepted By Review

1. The top-level architecture is shared Rust actors plus typed mailboxes,
   platform drivers, platform Rust effect adapters, and domain-blind
   browser-API executors where required.
2. Logical topology converges across native and web; physical thread counts,
   transports, private heaps, and API mechanics may differ.
3. Managed provisioning uses one shared Rust actor/state machine. Platform
   Rust effect adapters execute/lower effects and do not own phase sequencing;
   TypeScript executes only the resulting browser mechanics.
4. `ManagedScenarioLaunchState` should adopt `PlatformOperationService` for its
   provision and start operations rather than keep two specialized
   `PlatformOperationLedger`s and two pending `VecDeque`s.
5. `NativeManagedScenarioProvisionAdapter` can implement the existing
   `PlatformOperationExecutor` trait; its native storage resolution remains an
   adapter detail.
6. The outer managed operation remains per-world, matching shared launch
   demand. Native whole-scenario staging remains one physical effect whose
   atomic rename makes concurrent per-role requests safe.
7. The browser actor uses an explicit shared enum phase machine with suspension
   at whole-transaction boundaries; native drives it to completion directly.
8. Administrative browser actions come from generalizing the catalog
   transaction-plan executor, not from a fourth storage vocabulary.
9. TypeScript retains physical IndexedDB mapping and API mechanics but no
   managed-world decision state.
10. Reuse `PersistenceErrorKind` for stable storage failure categories and
    return a typed provisioning success outcome. Managed completions currently
    carry `String`, so this is a real operation type and call-site change, not
    merely improved error formatting.
11. No normalized low-level storage language or universal Worker ABI is
    required.
12. Native must retain direct typed execution without material serialization,
    copy, scheduling, or latency regression.

## Questions For The Implementing Tactical

1. Which current managed recipes are explicitly reconstructible, which preserve
   runtime state, and which validation depth does each require?
2. Is `corrupt` ever automatically replaceable, or always a refusal requiring
   an explicit recovery action?
3. What is the smallest high-level effect vocabulary that lets the shared actor
   express inspection, exclusive publication, authorized replacement, and
   resolution without becoming a storage DSL?
4. Which one stable addressing scheme should the generalized browser
   transaction executor expose: string store/index labels, numeric namespaces
   with typed key parts, or a deliberately defined replacement for both?
5. What platform assembly contract resolves a discriminated logical world
   source into the native or browser storage session without leaking paths or
   browser handles into shared scene code?
6. Should the one-shot provisioning Worker remain one-shot for isolation, or
   should a resident actor amortize Wasm initialization after measurement?
7. Is thread-per-provision still appropriate on native, or should later
   measurement justify a bounded pool?
8. Which lifecycle terms genuinely apply across persistence, compute, render,
   socket, and coarse-operation mailboxes without forcing one physical
   executor?

These are implementation-shape and product-policy questions under the accepted
actor/mailbox frame. They are not reasons to move sequencing into a platform
adapter.

## Alternatives Rejected

### Shared decision helpers with platform-owned sequencing

This was the earlier selected shape. It removes TypeScript policy but still
permits native and web Rust strategies to decide when to inspect, retry,
complete, or accept a conflict. A conformance suite can detect some drift but
does not create one policy owner. The revised direction keeps physical effects
platform-specific while moving semantic phases into one shared actor.

### One universal physical Worker or mailbox ABI

Persistence, render compilation, compute jobs, and sockets have materially
different batching, backpressure, buffer, lifetime, and failure needs. One
lowest-common-denominator broker would add complexity and can harm native
performance. Share actor ownership and mailbox semantics; specialize
transports.

### Let every worker access the database directly

SQLite or IndexedDB transaction serialization protects physical database
consistency, not engine revision ordering, pending-write visibility, actor
authority, or lifecycle barriers. Writable storage remains owned by a world
session or bounded administrative actor.

### Keep TypeScript as the browser workflow coordinator

This leaves a second policy surface. The existing native/browser validation and
repair divergence demonstrates the cost. TypeScript remains the right owner
for browser APIs, not scenario or persistence decisions.

### Make all engine storage and operation traits async

Browser suspension does not require promise-shaped engine call sites. Native
still needs background dispatch, and shared simulation should not inherit a
particular async runtime or borrowing model. Actor drivers contain async
mechanics below a nonblocking request/completion boundary.

### Run native through browser frames

Transport-byte equality is not semantic equality. Encoding typed native values
would add copies and indirection without reducing policy divergence.

### Require one shared Wasm heap across Workers

A shared heap may later reduce selected copies but introduces allocator,
lifetime, lock, crash-recovery, and termination coupling. Private Rust actors
plus semantically opaque transport already remove the competing TypeScript
engine.

### Move all IndexedDB mechanics into Rust immediately

Direct `web_sys` access remains a possible effect-adapter replacement. It does
not change the actor/mailbox architecture and is not required to remove domain
policy from TypeScript.

## Possible Tactical Sequence

No tactical is authorized by this topic. A bounded implementation sequence is:

1. **Topology and behavior baseline.** Record current native/web actor,
   mailbox, Worker/thread, copy, transaction, validation, and failure traces.
   Route the existing outer native provision adapter through
   `PlatformOperationExecutor` without changing behavior.
2. **Shared actor laboratory.** Implement the platform-free provisioning phase
   machine and exact trace tests over in-memory effect observations. Resolve
   validation depth and recovery authority explicitly.
3. **Native effect proof.** Drive the actor to completion on a native worker
   while preserving whole-scenario staging, atomic rename, logical identities,
   and direct typed execution.
4. **Browser transaction proof.** Generalize only the catalog transaction
   actions demonstrated by the actor traces while preserving IndexedDB v6 and
   existing records.
5. **Browser actor cutover.** Host the shared actor in the provisioning Worker,
   reduce TypeScript to generic effect execution and opaque completion
   forwarding, and delete the old TypeScript workflow atomically.
6. **Lifecycle and performance hardening.** Exercise cancellation, concurrent
   publication, hidden/resume, Worker failure, quota/error classification,
   repeated launches, memory release, and native overhead.
7. **Closeout.** Update topology diagrams, ownership/source locks, copy and
   allocation ledgers, validation evidence, and the recommendation for the
   next operation family.

Human review is required before destructive recovery policy is selected, and
again if implementation would weaken atomicity, change physical schemas,
introduce a shared Wasm heap, or measurably regress native execution.

## Validation Expectations

### Shared Rust actor

- exact phase traces for every observation, effect result, and conflict path;
- deterministic fault injection at every effect boundary;
- stale, duplicate, unknown, and cancelled completion tests;
- idempotent lost-acknowledgement recovery;
- validation-depth and recovery-authority fixtures;
- bounded batch and retained-byte assertions; and
- no platform types in the actor or outer operation contract.

### Native

- unchanged managed primary/destination identities and contents;
- preserved staged-directory atomic publication;
- reopen, invalid-content, conflict, failure, and retry behavior;
- no simulation-thread storage;
- request/completion and worker-lifecycle metrics;
- bytes copied or serialized across operation boundaries;
- wall time and allocation compared with the current adapter; and
- desktop plus affected Android/Quest gates when shared native code changes.

### Browser

- managed reuse, cancellation, concurrent publication, reviewed recovery,
  corrupt refusal/replacement policy, and ordinary-world isolation;
- primary/destination launch, preview, activation, and return;
- hidden/resume and forced Worker failure containment;
- quota, unavailable storage, transaction abort, and malformed completion;
- no managed-domain vocabulary or state machine in production TypeScript;
- Worker initialization, transaction, bytes, and retained-memory metrics;
- maximum frame gap and main-thread work during provisioning; and
- rendered output captured under `/tmp` and inspected at the first drawable
  cutover milestone.

### Cross-platform equivalence

The same logical observations must produce the same actor decisions and typed
completion on native and browser fake adapters. Physical traces may record
allowed differences such as native whole-scenario rename versus browser
per-world transactions. Validation, recovery authority, conflict acceptance,
failure, epoch, and result semantics must remain identical.

## Code And Documentation Map

Shared coarse operations and scenario policy:

- `native/crates/mclone-app-runtime/src/platform_operation.rs`
- `native/crates/mclone-app-runtime/src/scenario_content.rs`
- `native/crates/mclone-app-runtime/src/scenario_content/native.rs`
- `native/crates/mclone-scene/src/warm_world.rs`
- `native/crates/mclone-scene/src/session.rs`

Shared actor/mailbox precedents:

- `native/crates/mclone-server/src/persistence.rs`
- `native/crates/mclone-server/src/persistence/record_executor.rs`
- `native/crates/mclone-server/src/worldgen_mailbox.rs`
- `native/crates/mclone-server/src/light_mailbox.rs`
- `native/crates/mclone-server/src/job_codec.rs`

Browser actors and drivers:

- `native/apps/mclone-web-client/src/web_server_worker.rs`
- `native/apps/mclone-web-client/src/web_render_worker_actor.rs`
- `native/apps/mclone-web-client/src/web_catalog_execution.rs`
- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/www/mclone-integrated-server-worker.ts`
- `native/apps/mclone-web-client/www/mclone-server-job-worker.ts`
- `native/apps/mclone-web-client/www/mclone-render-compiler-worker.ts`
- `native/apps/mclone-web-client/www/mclone-web-persistence-executor.ts`
- `native/apps/mclone-web-client/www/mclone-web-world-catalog.ts`
- `native/apps/mclone-web-client/www/mclone-managed-scenario-provision-worker.ts`

Architecture and execution records:

- [`unified-persistence-interface.md`](unified-persistence-interface.md)
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md)
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md)
- [`../tactical/062-shared-threading-topology.md`](../tactical/062-shared-threading-topology.md)
- [`../tactical/067-shared-render-worker-architecture.md`](../tactical/067-shared-render-worker-architecture.md)
- [`../tactical/197-domain-blind-web-worker-broker.md`](../tactical/197-domain-blind-web-worker-broker.md)
- [`../tactical/198-opaque-websocket-and-indexeddb-adapters.md`](../tactical/198-opaque-websocket-and-indexeddb-adapters.md)
- [`../tactical/199-unified-persistence-interface.md`](../tactical/199-unified-persistence-interface.md)

Vanilla reference:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/IOWorker.java`
- `reference/minecraft-1.17.1/src/net/minecraft/util/thread/ProcessorMailbox.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`

## Recommended Direction

- Treat the shared Rust actor and typed mailbox as the logical concurrency
  model.
- Map that model onto native threads/direct calls and browser Workers/browser
  callbacks without requiring transport equality.
- Keep domain sequencing, identity, retries, revisions, validation, result
  acceptance, and lifecycle in Rust.
- Lower browser high-level effects into generic storage actions in
  Worker-resident Rust.
- Keep TypeScript responsible for browser mechanics and generic physical
  actions only.
- Give writable storage one owning world session or bounded administrative
  actor; compute workers return results rather than persisting independently.
- Implement managed provisioning as one shared actor with platform-specific
  effects, not shared decision helpers wrapped by two platform workflows.
- Require native and browser publication adapters to report conflicts to that
  actor; neither adapter may revalidate and accept a winner on its own.
- Preserve native direct execution and physical publication strengths.
- Use the vanilla `IOWorker`/`ProcessorMailbox` ownership lesson as a topology
  guide without copying its Java runtime shape.

Do not begin by moving the current TypeScript provisioning branches into
web-specific Rust. Establish the shared actor and effect boundary first so the
browser cut removes the competing workflow rather than merely changing its
language.
