# Unified Persistence Interface

Topic: `unified-persistence-interface`

Status: implemented 2026-07-20. Tactical
[`199`](../tactical/199-unified-persistence-interface.md) is the completed
execution record. This topic does not supersede the durable architecture in
[`persistence-architecture.md`](../persistence-architecture.md).

The engine now uses one typed Rust completion port over shared coordination and
generic memory/null, SQLite, and IndexedDB record executors. Native and browser
worlds hold world-scoped OS/Web Lock writer leases. Production browser worlds
have no domain-specific mirror/load/dirty TypeScript bridge or alternate old
runtime path. Typed quota failure, save-unhealthy latching, storage
observability, graceful retirement fencing, and platform acceptance are also
complete.

## Scope

Define one engine-facing Rust persistence interface while allowing each
platform to choose an appropriate physical storage strategy:

- SQLite on native desktop, dedicated-server, Android, and Quest hosts;
- IndexedDB in browsers;
- a raw-filesystem or region-file backend where useful;
- memory and null backends for tests and transient worlds; and
- future backends without changing authoritative engine behavior.

The proposal is deliberately narrower than all world lifecycle work. It covers
the boundary between authoritative persistence policy and physical record IO.
World-catalog policy, managed-content provisioning, migration UX, cloud saves,
and server administration remain separate higher-level concerns even when they
eventually reuse the same low-level storage machinery.

## Implemented Direction

> The engine owns persistence semantics through one Rust interface. Each
> platform injects a storage strategy beneath that interface. Platform adapters
> implement storage mechanics, never engine persistence policy.

The shared interface must be completion-based. “One interface” does not mean a
blocking trait call that returns an IndexedDB result synchronously. It means
that the authoritative engine always submits the same typed Rust requests and
integrates the same typed Rust completions, regardless of which backend serves
them.

```text
authoritative Rust server/runtime
  owns chunks, entities, players, dirty state, save policy, and lifecycle
                         |
                         v
typed Rust persistence port
  load/save records, durability, flush, close, request ids, completions
                         |
                         v
shared Rust persistence coordinator
  codecs, revisions, pending-write visibility, coalescing, batching,
  error semantics, flush barriers, and close semantics
                         |
                         v
platform record executor
  generic namespaces + keys + opaque bytes + transaction results
            /             |              |             \
       SQLite         IndexedDB      filesystem      memory/null
```

This result completes the direction already established in
[`persistence-architecture.md`](../persistence-architecture.md) and Tactical
[`134`](../tactical/134-shared-persistence-architecture.md), not a replacement
architecture. The live engine and browser path now share both the typed upper
port and the generic lower executor boundary.

## Ownership Contract

### Authoritative engine and typed Rust port

The engine-facing vocabulary should remain domain-meaningful and Rust-typed:

- world metadata;
- dimension records;
- chunk records;
- entity-chunk records;
- player records;
- world saved data;
- requested save durability;
- flush and close barriers; and
- typed success, miss, superseded-write, and failure completions.

The engine must not select SQLite tables, IndexedDB object stores, paths,
transactions, promises, cursors, or schema upgrade steps. It must not branch on
browser versus native when deciding how a chunk save behaves.

The existing `WorldStoreRequest`, `WorldStoreCompletion`, and
`PersistenceMailbox` in
[`mclone-server/src/persistence.rs`](../../native/crates/mclone-server/src/persistence.rs)
already approximate this port. The proposal does not require preserving every
current type or placing the final contract in that file, but a migration should
evolve this working boundary rather than create a competing engine API.

### Shared persistence coordinator

Rust should own semantics which must not vary by backend:

- record encoding and decoding;
- stable logical record addressing;
- request identity and completion correlation;
- same-key pending-write visibility to reads;
- revision precedence and write supersession;
- bounded write coalescing and batching;
- `Cache` versus `Durable` scheduling policy;
- flush as a barrier over prior required work;
- orderly close and rejection of post-close requests;
- consistent error classification and diagnostics; and
- backend capability checks where exact support differs.

These responsibilities are already substantially implemented by the current
persistence actors. They should not be reimplemented in TypeScript merely
because IndexedDB completes asynchronously.

### Platform record executor

The lower boundary should be record-oriented rather than SQL-shaped or
IndexedDB-shaped. Its smallest useful vocabulary is likely:

```text
read(namespace, key)
commit_batch(puts, deletes, requested_durability)
flush()
close()
```

Catalog and administrative consumers also need bounded scan/index operations,
conditional add or equivalent conflict reporting, and namespace deletion. Add
those operations from demonstrated consumers rather than constructing a
general database API in advance.

At this boundary:

- namespaces and key layouts are stable storage identifiers;
- values are opaque, versioned byte records authored by Rust;
- a batch has explicit atomicity requirements;
- results contain opaque records, misses, and storage-level errors; and
- the executor exposes a backend label, capabilities, and metrics without
  exposing engine policy.

The physical adapter may understand its own schema—for example which IndexedDB
store and index implement a stable namespace—but it should not decode a chunk,
choose a gameplay default, decide save precedence, or manufacture engine
responses.

## Async And Synchronization Model

The engine does not block while persistence runs.

On native hosts, a mailbox can dispatch synchronous SQLite or filesystem calls
to a storage thread and receive completions later. SQLite remains free to use
prepared statements, transactions, WAL, checkpointing, and other native
optimizations behind the interface.

In a browser Worker, Rust submits an owned storage plan and returns to the
event loop. A small IndexedDB executor starts the required requests. Browser
callbacks later return owned results to Rust, which advances the coordinator
and emits typed engine completions. No Rust mutable borrow or JavaScript view
may live across an `await`, and Rust must never spin or synchronously wait for a
promise.

IndexedDB sometimes requires read-dependent writes to be queued before the
current success callback returns so that the transaction remains active. That
does not require TypeScript to own the policy. The current ordinary-catalog
continuation already demonstrates the desired shape:

1. Rust emits a generic read action.
2. TypeScript executes it and calls Rust synchronously from `onsuccess`.
3. Rust evaluates the result and emits the follow-up storage action.
4. TypeScript queues that action before returning from the callback.
5. Transaction completion or abort is reported to Rust later.

This lets TypeScript do the small amount of browser-specific “legwork” that
IndexedDB requires without making it a second persistence engine.

## World Lifetime And Single-Writer Admission

Revision precedence inside one coordinator does not protect a world from two
independent coordinators. Two browser tabs, native processes, or retained
sessions can each hold an internally consistent view and still interleave
logically incompatible saves. SQLite transaction locking prevents physical
database corruption, but it does not by itself establish one authoritative
world owner.

The accepted product contract is therefore one writable authoritative session
per persistent world, enforced explicitly by every persistent backend. This is
a world-open/session-admission policy, not save precedence inside the record
executor:

- the Rust catalog/session layer requests an exclusive world lease when it
  admits the authoritative host;
- the concrete opened store/session retains the lease for its full lifetime,
  so direct `--world-dir`, dedicated-server, Android, and other paths cannot
  bypass a catalog-only preflight check;
- each platform strategy supplies the concrete lock/lease mechanism and
  reports loss or conflict through typed storage errors;
- the lease is released only after orderly persistence close, or automatically
  when the owning process/browser context dies where the platform permits;
- a second writer receives a typed `WorldAlreadyOpen`/lease-conflict result
  before starting simulation; and
- tests attempt a second open from a genuinely independent coordinator, not
  merely a second handle owned by the first one.

This is explicitly **not** a process-wide or application-wide singleton. The
lease key is the persistent world identity: for example, one native world
directory/database or one browser origin/database/world id. The following must
remain valid:

- multiple desktop client processes connect to one local or dedicated server;
- clients hold no persistence lease because they are not world authorities;
- separate server processes host different world directories concurrently;
- one process retains or serves multiple distinct worlds when the runtime
  supports it; and
- multiple browser tabs use different local worlds concurrently.

Only a second authoritative store/session targeting the same persistent world
conflicts.

The recommended first UX is to reject the second writable open with a clear
“This world is already open in another window or process” message and a retry
action. Do not silently use last-writer-wins, force takeover, or fall back to a
read-only simulation. Read-only previews and observer sessions are useful
features, but they need explicit authority and freshness semantics and should
not be invented as persistence-error recovery.

### Implemented lock mechanisms

Do not implement authority as a durable boolean, PID record, heartbeat, or
“lock file exists” check. Those schemes require stale-lock recovery and can
create split-brain writers. Use a platform-owned lifetime lock and treat any
stored owner information as diagnostics only.

On native hosts:

1. Create/open a stable `world.writer.lock` file inside the world directory
   without truncating or deleting it.
2. Take a nonblocking OS file lock before opening a writer-capable SQLite
   store. Desktop/dedicated hosts use `File::try_lock()`; Android uses the
   equivalent safe `fs4`/rustix adapter because Rust 1.92 returns
   `Unsupported` there. `WouldBlock` becomes typed `LeaseConflict`; other IO
   failures remain storage failures.
3. Move the uncloneable file handle into the persistence actor beside the
   SQLite connection. The actor is the `WorldWriteLease` owner.
4. Only after acquisition, optionally rewrite owner diagnostics such as a
   random session id, PID, process-start time, and backend label. Never use
   those fields to override the operating-system lock.
5. On orderly shutdown, stop accepting new work, drain prior durable writes,
   flush/checkpoint, close the database, explicitly unlock, and drop the file.
   If normal flush/close fails, retain the lease while retry/recovery remains
   possible; a separately explicit forced-abandon path may drop it after
   reporting potential save loss.

Closing or dropping the file handle releases the OS lock, including on process
crash; the inert lock file may remain forever without blocking anyone. Keeping
one stable file is important: deleting and recreating a locked path can produce
two different underlying files on some systems. Native world directories on
network filesystems need separate qualification because both SQLite and
advisory-lock behavior may differ from local app storage.

On web hosts:

1. The integrated-server Worker requests an exclusive Web Lock with a stable
   name derived from the storage namespace and world id, using `ifAvailable`
   so a conflict is reported immediately rather than queued indefinitely.
2. TypeScript performs only the Web Locks API mechanics. Rust requests the
   acquire/release actions, owns admission policy, and receives a typed
   acquisition result.
3. The lock callback returns an owned pending promise for the entire Rust world
   actor lifetime. Normal Rust close first drains every IndexedDB transaction;
   only its final release action settles that promise.
4. Worker or document termination releases its held Web Locks through the
   browser lock manager. Do not use the API's `steal` option: it can leave the
   previous code executing without exclusive authority.
5. If Web Locks are unavailable, fail the persistent-world capability honestly
   until a separately reviewed fenced fallback exists. Do not silently replace
   it with a race-prone IndexedDB/local-storage flag.

The Web Locks specification exposes the lock manager to Workers, holds a lock
until the callback's promise settles, and releases remaining locks when the
owning agent terminates. This is a substantially safer fit than an expiring
IndexedDB lease row. A durable lease row would require monotonically increasing
fencing tokens on every write so a delayed former owner cannot write after a
takeover; that complexity is not justified as the first path.

### Release and failure invariants

- The lock belongs to the writer-capable opened store, not a catalog preflight.
  Catalog listing and metadata inspection do not lock; catalog deletion must
  briefly acquire the same exclusive writer lease. Deletion needs an explicit
  platform ordering design: on Windows the deleter cannot remove the locked
  file until it releases its own handle, so release and directory removal must
  not create an unguarded recreate/open race.
- Network clients never open the authoritative store and therefore never
  acquire this lock.
- A read-only SQLite inspection/snapshot API may coexist with the writer where
  SQLite permits. It must be incapable of saving. Existing durability tests
  that open a second reader while the server is alive should migrate to this
  explicit read-only path rather than opening a second writer-capable
  `SqliteWorldStore`.
- Session close is a barrier: no executor request or IndexedDB transaction may
  remain live when the lease is released. Late completions carry the closed
  session/request epoch and are rejected.
- If the storage actor dies or loses its lease unexpectedly, the authoritative
  world enters a fatal/save-unhealthy state and stops durable mutation; it must
  not continue as an unlocked writer.
- A live but hung process or tab continues holding the lock. That is correct:
  automatically stealing from code that may resume would risk corruption. The
  recovery action is to close/terminate the old owner, after which the
  OS/browser releases the lifetime lock.
- Diagnostic owner metadata may be stale after a crash, PID reuse, or an
  abnormal exit. It can improve an error message but can never prove that a
  lock is held.

Risk and validation depth are platform-specific even though the invariant is
not. Cross-tab collision is a normal browser scenario and duplicate desktop or
dedicated-server authorities targeting one world are plausible, so those are
primary acceptance lanes. Two independent native servers on one Android device
are highly unlikely; Android should inherit the same native opened-store lock
and basic conflict tests without requiring an elaborate multi-process device
campaign.

## Backend Strategies

| Strategy | Platform mechanics retained behind the boundary | Engine-visible behavior |
|---|---|---|
| SQLite | connection/thread ownership, tables, prepared statements, transactions, WAL/checkpoints | typed requests and later completions |
| IndexedDB | object stores, indices, callbacks/promises, transaction-lifetime rules, schema upgrade mechanics | the same typed requests and later completions |
| Raw filesystem/region files | paths, temporary files, atomic rename or journal, file handles | the same typed requests and later completions |
| Memory/null | maps or deliberate discard, deterministic fault injection | the same typed requests and later completions |

One shared contract does not mean holding native backends to the least capable
backend. The contract specifies required semantics and capabilities; a backend
is free to implement them using its strongest practical mechanisms. Likewise,
`Durable` should retain the project’s defined engine meaning while each
platform documents what its strongest committed state can actually guarantee;
it must not pretend that an IndexedDB transaction completion is literally the
same hardware guarantee as a particular SQLite/fsync sequence.

## Implemented State

### Shared and native Rust

The live server owns the engine-facing shape and the physical executor seam in
`native/crates/mclone-server/src/persistence.rs`:

- `WorldStoreRequest` and `WorldStoreCompletion` carry typed, tokened logical
  operations;
- `PersistenceMailbox` and its actor own pending-write visibility, revision
  precedence, bounded coalescing, durability lanes, flush, close, and typed
  completion correlation;
- `PersistenceRecordRequest`/`PersistenceRecordResponse` define generic
  namespaces, owned key parts and payloads, atomic commits, probes, flush, and
  close;
- `RecordExecutorWorldStore` translates between typed records/codecs and the
  executor contract;
- memory/null mailboxes run through generic executors with deterministic fault
  injection; and
- `SqliteRecordExecutor` supplies the same contract on the threaded native
  path without changing schema version 2, keys, record bytes, WAL, or
  checkpoint behavior.

`SavedData` remains an explicit reserved unsupported engine family. Its generic
namespace and SQLite table remain reserved, but making it live in IndexedDB
would require a physical schema decision and no current consumer requires it.
This is deliberate capability honesty rather than an incomplete request path.

### Android and Quest persistence

SQLite is no longer merely a likely Android backend. Both Android application
lanes project app-private world roots through the shared native startup and
scene-service path. The flat Android client calls the shared `on_background()`
policy when its activity is suspended. Android XR observes `Pause`/`Stop` in
its lifecycle pump and synchronously drains the shared persistence path so
dirty world edits are committed before a possible OS kill. Tactical
[`168`](../tactical/168-unified-native-scene-host.md) records the save-on-pause
contract and its second-reader durability regression test.

The coordinator/executor extraction therefore affects live Android behavior.
Android is a required SQLite consumer and lifecycle validation lane, not a
future optional adapter. Pause/Stop must remain a final barrier over an already
bounded autosave backlog, not become the first time a long play session tries
to persist its durable work.

### Browser opened-world persistence

The browser now uses IndexedDB as the physical executor beneath the same Rust
record/coordinator boundary. Rust constructs logical addresses, owns codecs,
revisions, durability, bootstrap, and typed completions. The integrated-server
Worker services generic requests containing stable namespace ids, physical key
parts, opaque owned bytes, transaction boundaries, and browser errors.

[`mclone-web-persistence-executor.ts`](../../native/apps/mclone-web-client/www/mclone-web-persistence-executor.ts)
maps that vocabulary to the unchanged IndexedDB v6 stores and keys. It does
not distinguish chunks from entity chunks or metadata as engine concepts;
those are only physical namespace mappings. The former
`WebIndexedDbWorldStoreState`, `WebIndexedDbWorldStore`, external-load request
projection, dirty-record result projection, and TypeScript record switchboard
are gone, with no old/new runtime selector.

Each persistent Worker holds the Rust-named world Web Lock from before database
bootstrap through close. Ordinary catalog delete and delete-all acquire the
same lease around record clearing and catalog removal. A real same-profile
second-tab contention proof rejects the second lease, while graceful reload
and document/Worker destruction permit reacquisition.

On graceful replacement, the Worker releases its Web Lock before posting
`shutdown-complete`. A Rust-owned same-page retirement registry makes a new
Worker or catalog delete for that exact world await the acknowledgement with a
bounded timeout. Startup failures are not registered as retirements, fatal
Worker errors terminate the Worker, and independent tabs continue to receive
immediate conditional lock rejection.

### Existing browser proof of the thinner boundary

Tactical [`198`](../tactical/198-opaque-websocket-and-indexeddb-adapters.md)
moved ordinary world-catalog policy into the Rust continuation in
[`web_catalog_execution.rs`](../../native/apps/mclone-web-client/src/web_catalog_execution.rs).
TypeScript now executes storage-level transaction plans and returns results.
That path proves several important points with the current browser stack:

- Rust can retain operation state without holding a borrow across `await`;
- TypeScript can map stable store identifiers to IndexedDB mechanics without
  selecting catalog policy;
- read-dependent writes can be authored by Rust inside an IndexedDB success
  callback;
- real transaction aborts and constraint errors can be reported without
  manufacturing domain responses in TypeScript; and
- this division does not require shared Wasm memory or a new Worker topology.

The integrated-server store has more throughput and lifecycle pressure than
the catalog, so this is feasibility evidence rather than a sufficient
performance proof. It substantially reduces the architectural uncertainty.

The completed Tactical 198 browser evidence supplied the behavioral baseline
for the cutover. Ordinary IndexedDB mutation/reopen passed, and
the periodic `flat-grass-v1`/`cylinder-x:32` fixture reopened 121 chunks, two
entity chunks, and one dimension record while recovering the same block state
at canonical X 0 and lifted X 512. The catalog proof covered real constraint
abort recovery, concurrent record-played/delete non-resurrection, and record
cleanup. A replacement backend should preserve these outcomes rather than
define a smaller web contract.

### Browser durability and storage-policy limits

Cross-tab authority and quota mechanics are implemented. Every persistent
world has one conditional Web Lock, and `PersistenceErrorKind` distinguishes
quota, unavailable access, lease conflict, corrupt/incompatible data,
closed/cancelled work, and backend failure while retaining diagnostics. The
first durable executor failure latches the writer save-unhealthy so later work
cannot make it appear healthy.

The adapter exposes current usage/quota, `StorageManager.persisted()`, and an
explicit persistence-request hook. Startup deliberately does not request
persistent storage or claim a durability grant. Best-effort browser storage
can still be evicted outside a normal close; deciding when to request a grant
and what recovery UX to offer remains product policy above this executor.

### Implemented result

The reusable asynchronous record-executor middle is complete. The shared
coordinator drives IndexedDB directly through owned requests/completions, and
the web memory-mirror/external-load/dirty-record protocol no longer exists.

## Implementation Assessment

The result confirms the design was feasible without converting gameplay to
promises, changing SQLite or IndexedDB formats, adopting shared Wasm memory, or
retaining a dual runtime. Memory/SQLite controls and the ordinary catalog
continuation made the higher-risk ordering and IndexedDB transaction-lifetime
work testable before the production cut.

The main implementation risks were real: coordinator semantics had to survive
extraction, browser handoff needed an explicit retirement acknowledgement, and
Rust's standard file-lock API proved unsupported on Android. The campaign
resolved them inside the fixed contract. Android uses `fs4`'s safe rustix
adapter for the same OS-owned lifetime lock; no platform silently runs without
writer admission.

## Completed Migration Sequence

Tactical 199 completed the approved sequence:

1. **Freeze the semantic contract and evidence.** Record current request,
   completion, revision, durability, version compatibility, flush, close, and
   restart behavior. Add a backend conformance harness before moving ownership.
   Resolve the first-cut single-writer and quota-failure UX gates explicitly.
2. **Separate coordinator from executor.** Introduce the minimum generic
   asynchronous record-executor vocabulary under the existing typed mailbox.
   Keep current engine call sites unchanged.
3. **Adapt memory/null and SQLite first.** These are the control backends. The
   native SQLite format, threading, reopen behavior, and performance should
   remain unchanged.
4. **Build the IndexedDB executor behind the same seam.** Use a Rust-owned
   plan/continuation with a small TypeScript transaction executor, reusing the
   Tactical 198 pattern. Do not redesign Worker or Wasm-memory topology.
5. **Cut over one complete browser world.** Route metadata, dimensions,
   chunks, entity chunks, players, save acknowledgements, flush, and close
   through the new backend. Delete the mirror and dirty/external bridge in the
   same bounded cutover once restart evidence passes.
6. **Prove lifecycle and exclusive admission.** Validate Android background
   flush and browser page lifecycle, then reject a second independent writer
   consistently on native and web before calling the backend complete.
7. **Reassess catalog and managed storage.** Reuse the lower executor where it
   simplifies code, while retaining `WorldCatalog`, managed-provisioning, and
   opened-`WorldStore` policy as distinct Rust layers. This should be a later
   review decision rather than a prerequisite for the world-store cutover.
8. **Add filesystem/region strategies only from a real consumer.** The
   interface should permit them; the first campaign need not implement every
   possible backend.

Steps 1-6 are complete. Step 7 remains a deliberate later cost/benefit
decision, and step 8 remains consumer-driven. No long-lived old/new browser
fallback was retained.

## Acceptance Contract And Evidence

The following remains the durable acceptance contract. Tactical 199 satisfied
the applicable controls; its execution record owns exact commands, counts,
timings, screenshots, and platform caveats.

### Shared conformance

Run one behavioral suite against every backend that claims the capability:

- hit, miss, and corrupt-record handling;
- read-your-writes before physical completion;
- same-key revision precedence and superseded acknowledgements;
- cache/durable scheduling and bounded coalescing;
- atomic batch success and injected mid-batch failure;
- flush as a barrier;
- close with outstanding work and post-close rejection;
- reopen persistence for every durable record family;
- newer-than-code rejection, supported older-version migration, explicit
  refusal when no migration exists, and the documented distinction between
  discardable cache records and durable records which must never be silently
  dropped; and
- stable error classification and backend diagnostics.

### Exclusive-writer controls

- In the browser, attempt to open one world from two tabs/Workers and prove that
  exactly one obtains writable authority.
- On desktop/native, launch two independent store/session owners against the
  same world directory and prove that the second fails before simulation.
- As positive controls, connect multiple client processes to the first server
  and run independent server processes against different world directories;
  neither case may conflict.
- Prove that normal close releases admission and that crash/context loss does
  not leave a permanently stale lock.
- Prove that the presence of a stale native lock file or stale diagnostic
  metadata does not prevent reacquisition after its OS handle is gone.
- Prove that a rejected opener cannot write or delete the active world.
- Exercise the shared native lock/conflict mapping in Android tests, but do not
  require a multi-process Android device proof unless its runtime architecture
  later makes concurrent local authorities plausible.
- Database-level transaction serialization alone is not acceptance.
- Kill the native owner process and terminate the browser Worker while holding
  the lease, then prove a new owner can acquire it. Also prove that ordinary
  close does not release until all prior durable writes and transactions have
  completed.

### Native controls

- Preserve SQLite schema and record bytes unless a separately reviewed
  migration is necessary.
- Exercise threaded startup, autosave, shutdown, and reopen on desktop,
  dedicated-server, Android, and Quest paths.
- Measure storage-thread queue depth, batch size, latency, and tick impact
  before and after the extraction.

### Android and Quest controls

- Build both Android application lanes through the repository scripts.
- Open an app-private SQLite world, mutate it, trigger the real background
  lifecycle hook, and verify the mutation through a second reader before
  normal Drop/shutdown can mask a missing flush.
- Exercise Pause/Stop followed by process death and relaunch on device where
  practical. At minimum preserve the shared deterministic second-reader test;
  a device spot check is prudent before removing the old coordinator path.
- Bound and report pause-flush time. The activity may be killed soon after
  backgrounding, so an unbounded drain is not a sufficient lifecycle design.
  Also prove that normal autosave keeps the pre-pause durable backlog bounded.

### Browser proof

- Save, close, reopen, and verify metadata, dimensions, chunks, entity chunks,
  and players.
- Cover both ordinary and periodic-cylinder worlds so nontrivial dimension
  metadata and seam-adjacent chunks survive restart.
- Exercise simultaneous reads and writes, missing records, transaction abort,
  background autosave, graceful shutdown, and forced page/Worker termination.
- Attempt a second-tab writable open and verify the chosen conflict UX and
  absence of writes from the rejected session.
- Inject quota exhaustion deterministically and verify a typed failure,
  persistent save-unhealthy state, and no later false-success completion. A
  user-facing recovery flow remains separately reviewed product policy. Add a
  bounded isolated-origin fill-toward-quota probe where browser automation
  permits.
- Record whether persistent browser storage is available/requested/granted and
  prove that denial does not masquerade as a durable guarantee.
- Preserve catalog constraint-abort and non-resurrection coverage.
- Measure copied bytes, operation latency, bounded batch sizes, backlog, and
  maximum frame/tick gaps. Ownership cleanup alone does not prove a performance
  improvement.
- Keep managed scenario data isolated from ordinary local-world deletion.

## Tradeoffs And Risks

### Benefits

- The engine has one persistence model on every platform.
- SQLite remains a full native backend rather than being constrained by
  browser API details.
- IndexedDB mechanics remain testable and replaceable without duplicating
  engine record semantics in TypeScript.
- New record families are added once to the typed Rust port and coordinator,
  then supported mechanically by capable backends.
- Backend conformance tests become possible instead of relying mainly on
  platform-specific end-to-end behavior.
- The web Worker loses its current chunk/entity/player/dimension/metadata
  switchboard and dirty mirror.
- Batching can reduce crossings and transactions on web and improve SQLite
  throughput, provided batches remain bounded.

### Costs and footguns

- Extracting the coordinator/executor seam touches mature ordering and shutdown
  code. A superficially clean trait can still introduce save loss if barrier or
  pending-write semantics move incorrectly.
- Transaction atomicity must be explicit. A “batch” that can partially commit
  on one backend but not another is not a portable semantic unit.
- Browser page suspension and abrupt termination remain real limits; a better
  interface cannot make an ungranted shutdown interval reliable.
- Android Pause/Stop provides only a bounded opportunity to save before the OS
  may kill the process. A synchronous lifecycle drain preserves the current
  contract but must remain measured and bounded as world size grows.
- A database transaction lock is not a world-authority lease. Without explicit
  single-writer admission, independent processes or tabs can serialize
  individually valid but mutually stale saves.
- Browser quota exhaustion and storage eviction can invalidate optimistic
  durability assumptions. The engine needs a persistent save-unhealthy state;
  the UI needs honest recovery choices rather than a log-only error.
- IndexedDB still requires Wasm/JavaScript/structured-clone or view conversion
  around stored bytes. This design may enable fewer and larger transfers, but
  it does not guarantee zero copy.
- Oversized batches can trade call overhead for browser stalls and memory
  spikes. Backpressure and maximum byte/record counts need explicit bounds.
- Physical schema upgrades remain platform-specific. Shared Rust owns record
  codecs and compatibility policy; the adapter still owns how its stores or
  tables are upgraded safely.
- A boundary that is too high repeats domain policy in every backend; one that
  is too low becomes a generic database abstraction and leaks SQL/IndexedDB
  concerns upward. Typed Rust records over a minimal opaque-byte executor are
  the intended middle ground.
- Generic errors can hide useful backend details. Stable error categories
  should retain a diagnostic source, backend label, and relevant metrics.
- Collapsing world catalog, managed provisioning, and opened-world storage into
  one policy object would create a new catch-all. They may share the executor
  while keeping different lifetimes and authority rules.

## Alternatives Considered

### Keep incrementally shrinking the current web bridge

This has low per-slice risk and has already produced useful improvements, but
the mirror and two-way domain protocol remain structural sources of divergence.
Every new record family must be threaded through Rust dirty state, JavaScript
result fields, TypeScript normalization, store selection, and completion
decoding. It is a poor long-term endpoint.

### Implement all IndexedDB calls directly in Rust

This could minimize handwritten TypeScript and is a valid future option. It
does not remove IndexedDB callback/transaction-lifetime rules, and it would
move a substantial amount of browser API plumbing into Rust without changing
the engine-facing architecture. The recommended first direction is a small,
generic TypeScript executor controlled by Rust because the existing catalog
path already proves it. The executor can later move to `web_sys` without
changing the engine or coordinator contracts.

### Use OPFS-backed or Wasm-hosted SQLite in the browser

OPFS or a browser SQLite build could eventually offer a file-shaped or SQL
backend and may reduce the physical difference from native storage. It does
not eliminate browser quota, eviction, page lifecycle, cross-tab authority, or
Worker constraints, and it would introduce a new physical format and migration
path for existing IndexedDB worlds. IndexedDB remains the lower-risk first
cut because the schema, persisted test data, and Rust-owned transaction-plan
proof already exist.

The implemented executor seam makes this alternative cheaper rather than locking
it out. A measured OPFS/SQLite strategy could later replace or complement
IndexedDB without changing the authoritative engine port or persistence
coordinator.

### Use one lowest-common-denominator database API everywhere

A universal SQL-like or IndexedDB-like API would make the abstraction broad,
leaky, and likely to hold native storage back. The implemented interface unifies
engine semantics, not physical database features.

### Make the engine `async` over a storage trait

This would spread platform scheduling and cancellation choices into simulation
call sites and would not fit the existing poll/tick integration as naturally as
the proven mailbox. Native code also should not need promise-shaped call sites.
The completion port is the shared asynchronous abstraction.

## Non-Goals

- Literal Minecraft Anvil compatibility.
- Replacing SQLite or selecting a new native database.
- Shared Wasm linear memory, `SharedArrayBuffer`, or a new Worker topology.
- Eliminating all TypeScript, browser promises, or IndexedDB schema code.
- Making synchronous filesystem IO acceptable on the simulation thread.
- Unifying catalog, managed-content, and opened-world policy into one API.
- Designing cloud synchronization or cross-device conflict resolution.
- Changing record codecs or the IndexedDB schema merely to create the seam.
- Promising zero-copy persistence.

## Resolved Gate Decisions

1. The coordinator and mailbox were refactored in place in `mclone-server`;
   this campaign did not create a new shared crate.
2. The executor vocabulary is reads, probes, explicit atomic put/delete
   batches, flush, close, owned completions, and stable typed failures.
3. A commit batch is apply-all-or-none across the namespaces it contains.
   Memory fault injection, SQLite transactions, and one IndexedDB transaction
   prove that contract.
4. Physical IndexedDB v6 schema/version management remains TypeScript browser
   mechanics. Rust owns stable namespace ids, keys, codecs, and policy.
5. Catalog policy remains separate. It reuses the exact writer lease for
   deletion and same-page retirement fencing, but managed/catalog long-tail
   executor reuse is deferred.
6. The old bridge could be removed only after native controls and production
   browser reopen passed without a material frame/tick/storage regression.
7. Browser persistent-storage request timing remains an explicit product hook;
   startup does not prompt. Actual quota failure is typed and permanently
   latches that writer save-unhealthy.

## Completed Review Outcome

The architecture and full Tactical 199 cutover are accepted. Engine call sites,
SQLite schema/bytes, IndexedDB version/stores/keys, and private Wasm-memory
topology were preserved. The old browser bridge was deleted in the production
cut, and lifecycle/lease/quota/platform evidence passed. Further catalog or
managed-storage reuse is a new cost/benefit decision rather than unfinished
work in this campaign.

## Related Documents And Code

- [`persistence-architecture.md`](../persistence-architecture.md): durable
  persistence ownership, record families, and lifecycle design.
- [`loading-persistence.md`](../loading-persistence.md): chunk loading, dirty
  state, unload, and save-policy detail.
- Tactical [`134`](../tactical/134-shared-persistence-architecture.md): first
  shared actor, SQLite, and browser IndexedDB implementation record.
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md): accepted
  isolated Rust actor and domain-blind TypeScript boundary.
- Tactical [`198`](../tactical/198-opaque-websocket-and-indexeddb-adapters.md):
  completed Rust-owned ordinary-catalog continuation and generic IndexedDB
  executor proof.
- [`mclone-server/src/persistence.rs`](../../native/crates/mclone-server/src/persistence.rs):
  typed requests/completions, coordinator, generic record contract, SQLite
  executor, and native writer leases.
- [`web_server_worker.rs`](../../native/apps/mclone-web-client/src/web_server_worker.rs):
  Rust record translation, Worker lifecycle, and exact-world retirement fence.
- [`mclone-integrated-server-worker.ts`](../../native/apps/mclone-web-client/www/mclone-integrated-server-worker.ts):
  generic request service, executor event loop, and Web Lock lifetime.
- [`mclone-web-persistence-executor.ts`](../../native/apps/mclone-web-client/www/mclone-web-persistence-executor.ts):
  domain-blind IndexedDB namespace/key/opaque-byte executor.
- [`mclone-web-world-lease.ts`](../../native/apps/mclone-web-client/www/mclone-web-world-lease.ts):
  generic Web Lock acquisition and release mechanics.
- [`web_catalog_execution.rs`](../../native/apps/mclone-web-client/src/web_catalog_execution.rs)
  and
  [`mclone-web-world-catalog.ts`](../../native/apps/mclone-web-client/www/mclone-web-world-catalog.ts):
  existing Rust-continuation/thin-IDB-executor feasibility proof.
- [Rust `File::try_lock`](https://doc.rust-lang.org/stable/std/fs/struct.File.html#method.try_lock):
  standard native nonblocking lifetime lock; Android uses the equivalent safe
  `fs4`/rustix adapter because Rust 1.92 returns `Unsupported` on that target.
- [W3C Web Locks API](https://w3c.github.io/web-locks/): browser Worker lock
  scope, callback-promise lifetime, conditional acquisition, and termination
  release contract.
