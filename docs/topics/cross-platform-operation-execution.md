# Cross-Platform Operation Execution

Topic: `cross-platform-operation-execution`

Status: high-level actor/mailbox direction accepted 2026-07-20. Tactical
[`201`](../tactical/201-lobby-content-simplification.md) removed the accidental
managed installer and its TypeScript policy surface. The required fresh
post-cleanup review found no replacement provisioning actor, but did justify
the bounded existing-owner cleanup in active Tactical
[`202`](../tactical/202-web-scene-async-boundary-cleanup.md).

## Top-Level Frame

> What is Mclone's shared logical concurrency model, and how do native threads
> and browser Workers realize it without duplicating engine policy, forcing
> browser transport costs onto native, or letting TypeScript become a second
> engine?

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
  web: lower effects into generic browser actions
        |
        v
      TypeScript browser-API executor
        IndexedDB / WebSocket mechanics only
```

The actor is the semantic owner. The driver and effect adapter are platform
implementations. TypeScript is the final browser-API executor where the browser
does not expose the required API directly to the shared owner.

Native and web should therefore have the same logical owners and
request/completion semantics even when they use different thread counts,
memory layouts, byte transports, or asynchronous APIs. This is not a proposal
for one universal Worker ABI, one shared database service, or one trait that
erases every workload difference.

The managed lobby installer exposed the boundary problem because its browser
path contains a domain state machine in TypeScript. It also exposed a more
basic product-design problem: the lobby did not need to be installed as a
versioned persistent world at all. The accepted response is to simplify the
feature before generalizing its machinery. Moving the existing workflow into
a Rust actor would fix language ownership while preserving the wrong product
abstraction.

## Vocabulary

| Term | Meaning |
|---|---|
| **Actor** | The single owner of a subsystem's mutable domain state, sequencing, decisions, and lifecycle. “Actor” is a general concurrency term, not a Rust-specific facility; Mclone's actors are implemented in Rust. |
| **Mailbox** | The typed, owned request/completion boundary around an actor. It defines semantic identity, admission, backpressure, and closure, not a universal byte encoding. |
| **Driver** | The platform mechanism that gives the actor execution turns and transports mailbox values: native thread/channel, browser Worker/message/SAB, or an inline test fallback. |
| **Effect port** | Narrow actions an actor may request from platform resources without learning paths, browser objects, promises, or physical schema names. |
| **Effect adapter** | Platform Rust that implements an effect through direct native calls or lowers it into generic browser actions. |
| **Browser-API executor** | Domain-blind TypeScript that performs IndexedDB, WebSocket, Worker, promise, and related browser mechanics. |
| **Assembly** | Platform code that binds an actor and mailbox to a driver, effect adapter, and browser executor where required. |

An actor may call a synchronous adapter while running on a native worker
thread. The same actor may emit an owned effect request and resume later in a
browser Worker. That physical difference must not create two semantic owners.

“Opaque TypeScript” means semantically opaque, not unable to inspect a
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
  `-- bounded platform-operation actors where a real consumer needs one
        catalog, session start, future coarse work
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

The former managed lobby path also started native provisioning work on a
background thread, staged scenario directories, created SQLite stores, and
published through rename. Tactical 201 deleted that parallel installer. Native
lobby startup now uses a transient authored memory store plus an ordinary
catalog or app-private SQLite destination.

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

Opened-world persistence does not round-trip records through the browser main
thread. Worker-resident Rust emits generic record requests; adjacent
TypeScript executes IndexedDB transactions and returns completions to that same
Rust actor. External `SharedArrayBuffer` mailboxes are shared byte transports
between private Wasm heaps, not a shared Rust object heap.

The former one-shot managed-scenario Worker was separate from this ordinary
opened-world path. Tactical 201 deleted it and its TypeScript validation,
publication, repair, and conflict workflow rather than turning it into another
permanent Worker role.

### Topology equivalence

| Logical role | Native realization | Browser realization |
|---|---|---|
| client/scene | app/render thread | browser main Rust plus rAF/WebGPU adapter |
| integrated authority | server runner OS thread | integrated-server Web Worker |
| worldgen/light jobs | Rust worker threads | Rust actors in job Web Workers |
| persistence owner | Rust persistence thread with direct SQLite | server-Worker Rust actor plus adjacent TypeScript IndexedDB executor |
| render compiler | Rust worker thread/pool | Rust actor in render Web Worker |
| bounded operation | Rust worker thread or direct adapter | Rust actor in a suitable Worker when asynchronous browser effects require it |

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

The lesson is mailbox-owned subsystem state, not Java's executor
implementation. Shared Rust defines each logical actor and mailbox; platform
assembly chooses the execution substrate.

## Scope

This topic governs bounded and resident asynchronous subsystem execution where
native and browser hosts need the same semantic owner:

- integrated server and server-job actors;
- persistence actors and physical record executors;
- render compilation;
- browser socket actors;
- world catalog operations; and
- future coarse platform operations justified by a surviving consumer.

It does not require existing proven actors to adopt
`PlatformOperationExecutor` literally. Persistence, compute, rendering,
sockets, and coarse operations may retain specialized mailbox types and
transports while satisfying the same ownership model.

Managed-scenario provisioning is not a target actor. Tactical 201 removed that
special subsystem and retained the embedded-world behavior on ordinary
primitives. The completed post-cleanup inventory found no replacement
provisioning operation to standardize. It did find a few web-scene seams where
TypeScript still projects decisions already owned by Rust; Tactical 202 cleans
those seams without adding a new actor.

Out of scope:

- one universal physical Worker, broker, mailbox ABI, or scheduling policy;
- shared Wasm linear memory as a prerequisite for semantic convergence;
- forcing native through encoded browser frames or SAB-shaped mechanics;
- allowing arbitrary compute workers to access writable world storage;
- folding catalog, persistence, and unrelated operations into one catch-all
  API; and
- runtime storage-schema migration or upgrade semantics. Any future migration
  campaign requires separate pre-admission/offline review.

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
6. Writable storage belongs to one owning world session, not to arbitrary
   compute workers.
7. Native remains free to move typed values, share immutable Rust allocations,
   and call SQLite/filesystem code directly on a worker thread.
8. The browser main thread never blocks or spins, and no mutable Rust borrow or
   JavaScript view crosses an `await`.
9. Private Wasm heaps and failure containment may remain even when external
   SAB mailboxes carry bytes between them.
10. Request identity, epochs, stale/duplicate rejection, typed errors, and
    shutdown semantics remain Rust-owned.
11. Transient authored content, app-private persistent worlds, and catalog
    worlds retain explicit identity, deletion, and authority boundaries.
12. A production cut deletes the superseded policy path instead of keeping
    indefinite native/web or Rust/TypeScript dual implementations.
13. Do not create a general actor or executor merely to preserve machinery
    whose product requirement has disappeared.

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
not need identical durability or buffer semantics. The shared model is a family
of actors and mailboxes, not a lowest-common-denominator byte broker.

| Work profile | Shared semantic shape | Native driver | Browser driver |
|---|---|---|---|
| integrated authority | commands, updates, cadence, readiness, shutdown | server thread/channel | Worker actor plus message/SAB transport |
| worldgen/lighting | queued jobs, resident state, bounded results | worker threads/pool | job Workers plus transfer/SAB |
| render compilation | qualified jobs, revision acceptance, resident compiler state | thread/pool | render Worker plus external SAB |
| opened-world persistence | ordered records, revisions, barriers, one world owner | persistence thread with direct SQLite | server-Worker Rust plus IndexedDB callbacks |
| catalog/coarse operation | typed request, bounded continuation, final completion | direct adapter or worker thread | Worker Rust actor plus browser actions when needed |
| remote socket | commands, events, backpressure, close | native socket runtime | Worker Rust actor plus WebSocket callbacks |

## Existing Engine-Facing Operation Port

`mclone-app-runtime::platform_operation` provides an outer mailbox for coarse
platform operations:

```rust
pub trait PlatformOperationExecutor<K, T, E> {
    fn submit(&mut self, operation: PlatformOperation<K>);
    fn try_recv_completion(
        &mut self,
    ) -> Option<PlatformOperationCompletion<T, E>>;
}
```

It provides target-neutral tokens, `PlatformOperationService`, deferred
executors/handles, epochs, monotonically increasing request IDs,
pending-request ownership, failure restoration, and stale/duplicate/unknown
completion classification.

No `JsValue`, promise, Worker, socket, path, or database handle enters this
ledger. Shared scene policy may submit one typed request and later receive one
typed completion without driving internal browser steps.

This is one concrete mailbox, not a required base class. Keep it where a real
coarse operation benefits from it. Do not route persistence, compute, render,
or sockets through it solely for structural uniformity, and do not preserve the
managed lobby operation just to create a consumer.

## Lobby Simplification Confirms The Boundary

The pre-Tactical-201 implementation had two layers that should not be
conflated:

1. The embedded-world product and engine proof: two `RealmServer`s, observer
   warmup, retained preview, supported promotion, complete-slot exchange, and
   A-to-B-to-A return.
2. A managed-content installer: manifests, versions, fingerprints, stored-world
   classification, repair policy, native staging/publication, a one-shot web
   Worker, and a TypeScript provisioning workflow.

Only the first layer was a durable requirement. The landed replacement is:

```text
shared Rust lobby launch coordinator
  |
  +-- TransientAuthored(lobby fixture)
  |     -> fresh ordinary memory-backed WorldStore per launch
  |
  `-- destination
        +-- Catalog(LocalWorldId)
        |     -> ordinary persistent store
        |
        `-- AppPrivate(PrivateWorldKey)
              -> ordinary persistent generated store
```

The lobby source is immutable, but its session store may mutate and is simply
discarded at shutdown. The preferred destination remains the most recent
compatible catalog world. When there is none, a stable catalog-excluded
app-private world uses ordinary persistence.

This removes the need to inspect, publish, version, fingerprint, repair, or
migrate a persisted authored lobby. It also removes the reason for the browser
provisioning Worker and for a shared managed-provisioning actor.

The shared Rust launch coordinator still owns product sequencing: destination
selection, lobby-first admission, cancellation, stale completion rejection,
readiness, preview, and slot installation. That coordinator may use an actor or
operation mailbox where asynchronous startup requires one. It is not a
storage-installer actor.

The completed implementation and deletion sequence lives in
[`Tactical 201`](../tactical/201-lobby-content-simplification.md).

## Storage Access And Resolution

Writable storage is not a global service that every worker may access. It
belongs to the actor/session that owns the logical world:

- a live persistent world has one persistence actor and one writer-capable
  store for the session lifetime;
- worldgen, lighting, and render workers return compute results rather than
  independently persisting them;
- a transient authored lobby uses an in-memory store and has no durable storage
  identity; and
- an app-private fallback opens through the same ordinary persistence path as
  a catalog world but remains outside catalog listing and user deletion.

SQLite and IndexedDB may physically serialize concurrent transactions, but
that does not establish game-level ordering, revision precedence, pending-write
visibility, or one authoritative writer. Those remain actor/session policy.

The persistence owner remains one logical writable realm even when native
storage routes dimension-local records among several SQLite files. The
accepted native sharding and deliberately consolidated browser layout live in
[`world-dimension-storage-layout.md`](world-dimension-storage-layout.md).

Shared scene code should carry path-free source identity:

```text
TransientAuthored(AuthoredFixtureId)
Catalog(LocalWorldId)
AppPrivate(PrivateWorldKey)
```

Platform session assembly resolves the identity to an in-memory store, native
directory/SQLite set, or browser IndexedDB key. Paths, SQLite connections,
IndexedDB objects, and JavaScript values do not enter shared scene policy.

## Browser TypeScript Boundary

TypeScript may own:

- Worker construction, module loading, timers, yields, and termination;
- `postMessage`, transfer lists, SAB typed views, atomics, and wakeups;
- IndexedDB open, object-store/index mapping, requests, cursors, transactions,
  promises, aborts, structured-clone envelopes, schema names, and key paths;
- WebSocket construction and browser callbacks; and
- stable mechanical action tags, namespace identifiers, request IDs, byte
  buffers, and generic browser error forwarding.

TypeScript must not own:

- scenario IDs, roles, authored fixtures, gameplay behavior, or destination
  selection;
- chunk, entity, dimension, or render-job interpretation;
- actor phase sequencing, retries, conflict acceptance, or completion policy;
- request admission, active/standby priority, revisions, supersession, or stale
  result handling;
- construction of engine success/failure variants; or
- independent mirrors of Rust domain state.

The existing opened-world record executor and catalog runner are the relevant
browser precedents. Rust owns domain continuations and emits mechanical storage
requests. TypeScript maps stable namespaces to IndexedDB, executes the request,
and returns data or a generic failure.

Tactical 201 does not need a generalized administrative transaction language.
It deletes the only proposed consumer: managed publication and repair. Do not
merge the catalog string-label vocabulary and opened-world numeric namespace
vocabulary speculatively. If a later real consumer needs multi-action
transactions, choose one addressing scheme and the thinnest action set from
that consumer's measured requirements.

The practical boundary tests are:

- changing gameplay, authored content, destination policy, retry rules, or
  engine outcomes requires no TypeScript change;
- changing an IndexedDB store, index, key path, or browser lifecycle mechanism
  may require TypeScript adapter work; and
- production TypeScript contains no scenario role, content version,
  fingerprint, validation class, repair branch, or semantic result assembly.

## Lifecycle And Failure Semantics

The shared actor/mailbox contract owns semantic lifecycle; the driver owns
best-effort physical cancellation.

Epoch invalidation is authoritative: a late completion cannot affect a
replacement scene. A native worker may finish already-running work; a browser
adapter may abort an IndexedDB transaction or terminate a Worker. The shared
contract should not promise immediate physical cancellation where a backend
cannot provide it.

Every implementing tactical should select the relevant cases from:

- submission failure before work begins;
- thread/Worker construction failure;
- storage open or authority failure;
- cancellation before, during, and after a physical operation;
- completion after the scene epoch changed;
- duplicate, unknown, or malformed completion;
- actor panic, Worker error, or forced termination;
- commit followed by lost acknowledgement;
- orderly shutdown with pending work; and
- retry without wedging the actor or applying stale state.

The mailbox or operation ledger decides whether a completion may affect current
scene state. The actor decides whether a result is semantically acceptable.
The adapter reports what the platform operation did. None may infer another
layer's state from a timeout alone.

## Decisions Accepted By Review

1. Shared Rust actors plus typed mailboxes are the logical concurrency model.
2. Logical ownership converges across native and web; physical thread counts,
   transports, private heaps, storage APIs, and blocking behavior may differ.
3. Platform Rust adapters own physical effects; TypeScript owns browser API
   mechanics, not engine sequencing or meaning.
4. Specialized actors and mailboxes remain preferable to a universal executor
   or Worker ABI.
5. Writable world storage has one session owner. Compute workers do not write
   SQLite or IndexedDB directly.
6. Native retains direct typed calls, moves, and sharing without browser
   serialization or SAB-shaped indirection.
7. The managed lobby installer is accidental product machinery, not the first
   actor to standardize.
8. The authored lobby becomes a fresh transient Rust bootstrap. The fallback
   destination becomes a stable app-private ordinary persistent world.
9. Existing legacy managed records may remain inert; this work does not add
   runtime schema upgrades or destructive migration.
10. Tactical 201 landed without a new provisioning actor or generic
    administrative IndexedDB executor.
11. The post-cleanup TypeScript and coarse-operation inventory happens as a
    fresh review. New actors remain justified individually.
12. A production cut removes superseded code rather than maintaining parallel
    old/new paths.

## Tactical 201 Implementation Answers

The Tactical 201 questions now have concrete answers:

1. `LobbyWorldSource::TransientAuthored(AuthoredWorldFixtureKind)` is sufficient.
   Server assembly creates a fresh ordinary `MemoryWorldStore` directly from
   shared Rust fixture records before startup.
2. `AppPrivateWorldKey::LobbyFallback` preserves a path-free logical identity.
   Platform Rust resolves it to the existing native directory or stable
   IndexedDB world id without exposing that physical choice to scene policy.
3. Legacy native scenario directories and IndexedDB `managedWorlds` records
   remain inert. No live migration, schema upgrade, scan, or destructive reset
   was required.
4. `PlatformOperationService` still has a real production catalog consumer in
   `WorldCatalogExecutor`. The smaller `PlatformOperationLedger` also usefully
   owns web scene-session and lobby runtime-start identities. There is no
   surviving managed-administration consumer.
5. Product locks now assert transient fresh reset, protected authority,
   catalog/app-private selection, cancellation and stale completion, preview
   readiness, A-to-B-to-A activation, persistence, and direct-path isolation
   without naming installer phases or payloads.

These answers close the implementation questions, not the separate ownership
review. That fresh review must inventory all surviving production TypeScript
and native/web sequencing before deciding whether an existing actor is already
sufficient, a bounded cleanup is warranted, or a new shared actor has a real
consumer. A future durable-lobby, downloadable-content, or destructive-
migration requirement still requires renewed product review rather than
implicit restoration of the deleted installer.

## Alternatives Rejected

### Port the managed installer into one shared actor

This would correctly eliminate the native/browser sequencing fork, but it
would preserve versioning, classification, publication, conflict recovery, and
administrative storage machinery for a lobby that does not need durable
installation. It is the right pattern only if a future product really needs an
installed-content subsystem.

### One universal physical Worker or mailbox ABI

Persistence, render compilation, compute jobs, and sockets have materially
different batching, backpressure, buffer, lifetime, and failure needs. Share
ownership semantics and specialize transports.

### Let every worker access the database directly

SQLite or IndexedDB transaction serialization protects physical consistency,
not revision ordering, pending-write visibility, actor authority, or lifecycle
barriers. One world persistence actor remains the writable owner.

### Keep TypeScript as a workflow coordinator

This creates a second policy surface and permits native/web drift. TypeScript
remains the right owner for browser APIs, not scenario, gameplay, persistence,
or result decisions.

### Make all engine traits async

Browser suspension does not require promise-shaped engine call sites. Native
still needs background dispatch, and shared simulation should not inherit a
particular async runtime or borrowing model. Drivers contain async mechanics
below nonblocking request/completion boundaries.

### Run native through browser frames

Transport-byte equality is not semantic equality. Encoding typed native values
would add copies and indirection without reducing policy divergence.

### Require one shared Wasm heap across Workers

A shared heap may later reduce selected copies but introduces allocator,
lifetime, lock, crash-recovery, and termination coupling. Private Rust actors
plus semantically opaque transport already establish one policy owner.

### Move all IndexedDB mechanics into Rust immediately

Direct `web_sys` access remains a possible adapter replacement. It does not
change the actor/mailbox architecture and is not required to remove domain
policy from TypeScript.

## Completed Tactical Sequence

[`Tactical 201`](../tactical/201-lobby-content-simplification.md) completed this
sequence:

1. baseline the current dependencies and preserved product behavior;
2. add the shared transient authored bootstrap;
3. cut native and web primary launch over to it;
4. route the fallback through ordinary app-private persistence;
5. delete the managed Rust/TypeScript provisioning paths atomically; and
6. validate lifecycle, persistence, performance, pixels, and platform lanes.

[`Tactical 202`](../tactical/202-web-scene-async-boundary-cleanup.md) then
completed the fresh production ownership review and its bounded follow-up. The
review found no missing general actor. It reused the shared session and lobby
coordinators plus existing Worker-resident Rust owners to:

1. delete the redundant browser session-lifecycle mirror;
2. move active local/IndexedDB/remote start selection into browser Rust;
3. lower queued lobby starts directly into opaque Rust tickets;
4. move complete streaming and initial-presentation readiness into Rust; and
5. post the Rust-authored integrated-server ready envelope unchanged.

This is the intended application of the actor frame: first find the real state
owner, then remove duplicate platform sequencing without inventing another
framework. Apply it to another consumer only when a fresh review finds real
mutable state or sequencing that must remain consistent across native and web.

## Validation Expectations

### Tactical 201 and 202 boundary proof

- the lobby primary starts without filesystem or IndexedDB publication;
- two launches start from the same immutable authored source;
- catalog selection and app-private fallback persistence remain correct;
- lobby-first, preview, supported activation, return, and A-to-B-to-A behavior
  remain unchanged;
- cancellation, late completion, shutdown, relaunch, and Worker failure remain
  safe;
- ordinary catalog worlds are never adopted, overwritten, or deleted;
- production TypeScript contains no managed-domain workflow; and
- the direct one-world path retains its thread/Worker, memory, and frame-time
  envelope.

These checks passed in shared tests, native flat/stereo and catalog smokes,
browser semantic desktop/mobile, catalog, persistence and lifecycle probes,
Worker/type ownership gates, and flat Android plus Quest/OpenXR APK builds.
Native captures were inspected. On this Linux host, Chromium returned fully
transparent WebGPU screenshots for lobby and unrelated control probes despite
nonzero Rust draw receipts; headed Xvfb and both installed Chromium channels
reproduced it. That host capture defect is recorded in Tactical 201 rather than
weakening pixel assertions or treating transparent images as product evidence.

Tactical 202 additionally passed the complete web, shared scene, and shared app
runtime test suites, a desktop compile control, generated bindings/typecheck,
and the 56-entry Worker ownership gate. Browser local, IndexedDB, remote,
catalog, lobby, lifecycle, and mobile semantic paths passed to the extent
available on this host. Their only failures were the same recorded transparent
capture and invalid-device resource-rebuild limitations. The two-runtime lobby
probe passed outright.

### Continuing actor boundary

- platform types do not enter shared actor or operation contracts;
- native uses direct typed effects without browser serialization;
- browser Rust owns domain continuations around browser callbacks;
- TypeScript handles mechanical envelopes and browser API failures only;
- writable storage has one session owner; and
- stale, duplicate, unknown, cancelled, and shutdown cases are explicit where
  the workload needs them.

## Code And Documentation Map

Current operation and lobby owners:

- `native/crates/mclone-app-runtime/src/platform_operation.rs`
- `native/crates/mclone-app-runtime/src/scenario_content.rs`
- `native/crates/mclone-scene/src/warm_world.rs`
- `native/crates/mclone-scene/src/session.rs`
- `native/apps/mclone-web-client/src/web_scene_host.rs`
- `native/apps/mclone-web-client/www/mclone-web-world-catalog.ts`
- `native/apps/mclone-web-client/www/mclone-web-app.ts`

Shared actor/mailbox precedents:

- `native/crates/mclone-server/src/persistence.rs`
- `native/crates/mclone-server/src/persistence/record_executor.rs`
- `native/crates/mclone-server/src/worldgen_mailbox.rs`
- `native/crates/mclone-server/src/light_mailbox.rs`
- `native/crates/mclone-server/src/job_codec.rs`
- `native/apps/mclone-web-client/src/web_server_worker.rs`
- `native/apps/mclone-web-client/src/web_render_worker_actor.rs`
- `native/apps/mclone-web-client/src/web_catalog_execution.rs`

Related records:

- [`embedded-worlds.md`](embedded-worlds.md)
- [`unified-persistence-interface.md`](unified-persistence-interface.md)
- [`world-dimension-storage-layout.md`](world-dimension-storage-layout.md)
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md)
- [`../tactical/197-domain-blind-web-worker-broker.md`](../tactical/197-domain-blind-web-worker-broker.md)
- [`../tactical/198-opaque-websocket-and-indexeddb-adapters.md`](../tactical/198-opaque-websocket-and-indexeddb-adapters.md)
- [`../tactical/199-unified-persistence-interface.md`](../tactical/199-unified-persistence-interface.md)
- [`../tactical/201-lobby-content-simplification.md`](../tactical/201-lobby-content-simplification.md)
- [`../tactical/202-web-scene-async-boundary-cleanup.md`](../tactical/202-web-scene-async-boundary-cleanup.md)

Vanilla reference:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/storage/IOWorker.java`
- `reference/minecraft-1.17.1/src/net/minecraft/util/thread/ProcessorMailbox.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`

## Recommended Direction

- Treat Tactical 201 as the completed subtraction proof; do not rebuild its
  deleted installer as a managed-provisioning actor or generalized
  administrative IndexedDB executor.
- Treat shared Rust actors and typed mailboxes as the logical concurrency
  model for the core systems that remain.
- Map that model onto native threads/direct calls and browser Workers/callbacks
  without requiring transport equality.
- Keep domain sequencing, identity, retries, revisions, result acceptance, and
  lifecycle in Rust.
- Keep TypeScript responsible for browser APIs and generic physical actions.
- Give writable storage one owning world session; compute workers return
  results rather than persisting independently.
- Preserve native direct execution and use the vanilla
  `IOWorker`/`ProcessorMailbox` ownership lesson without copying Java's runtime
  shape.
- Treat Tactical 202's completed inventory as the current baseline: 5,275
  authored TypeScript lines, one generic Worker-construction site, and no
  identified need for another actor abstraction.
- Start another tactical only from a named remaining semantic owner or a
  concrete operation, not from a desire to make the topology look uniform.

The important result is that subtraction worked. The lobby now runs on a
transient authored store plus ordinary persistent destinations, and the fresh
review did conclude that no follow-up abstraction was warranted. The bounded
cleanup made the existing ownership lines faithful without changing the native
topology or building a replacement framework.
