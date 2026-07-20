# Tactical 199: Unified Persistence Interface

Status: active 2026-07-20; full autonomous campaign authorized. Slices 0-2
are complete and Slice 3 is next.

Topic: `unified-persistence-interface`

Workstream: shared persistence coordinator, native SQLite, browser IndexedDB,
platform world-writer leases, lifecycle, and conformance. Durable direction
lives in
[`../topics/unified-persistence-interface.md`](../topics/unified-persistence-interface.md).

## Authorized Result

Complete the accepted unified persistence architecture end to end:

```text
authoritative Rust world host
            |
typed completion-based persistence port
            |
shared Rust coordinator and codecs
            |
generic record executor
      /           |           \
  SQLite      IndexedDB     memory/null
```

The core engine uses one typed Rust persistence interface. Native SQLite,
browser IndexedDB, and test/transient stores implement platform strategies
below it. Browser TypeScript executes generic IndexedDB/Web Locks mechanics but
does not understand chunk, entity, player, dimension, metadata, durability, or
save-policy meaning.

The campaign is authorized to continue through production browser cutover,
old-bridge deletion, lifecycle/error hardening, cross-platform validation, and
documentation closeout without routine human checkpoints. Stop only for an
unforeseen architectural choice, durable-data/schema risk, required semantic
weakening, unremovable material performance regression, or a platform result
which contradicts the accepted contract.

## Fixed Contracts

- Preserve IndexedDB version, object-store names, keys, existing records, and
  ordinary/managed deletion domains unless an explicit stop condition is hit.
- Preserve SQLite schema, record bytes, WAL/checkpoint behavior, threaded IO,
  and existing world-directory layout.
- Keep `WorldStoreRequest`/`WorldStoreCompletion`-shaped typed engine meaning;
  evolve the existing mailbox rather than add a competing engine API.
- No Rust blocking/spinning on browser promises and no mutable Rust borrow or
  JavaScript view across `await`.
- TypeScript may know generic namespace ids, physical stores/indices, keys,
  opaque bytes, transactions, Web Locks, promises, and browser errors. It must
  not know logical world record variants or persistence policy.
- One explicit writer lease per persistent world, not per process. Multiple
  clients may connect to one server, and different worlds may be hosted
  concurrently.
- Native writer leases use an OS-owned lifetime lock on one stable lock file;
  web uses an exclusive world-keyed Web Lock. No timestamp/PID/heartbeat/file-
  existence authority and no automatic stealing.
- A writer lease is owned by the live opened store/actor, acquired before a
  writer-capable database, and released only after durable drain/close.
- The old browser mirror/external-load/dirty-record path is removed in the same
  bounded cutover that makes the new backend production. No indefinite dual
  runtime path.
- Catalog, managed provisioning, and opened-world policy retain distinct Rust
  owners even if they later reuse the lower record executor.

## Current Implementation Baseline

### Shared/native

`native/crates/mclone-server/src/persistence.rs` currently owns:

- `WorldStoreRequest`, `WorldStoreCompletion`, and `PersistenceMailbox`;
- inline, external-load, and native-threaded actor modes;
- pending-write visibility, revision precedence, coalescing, `Cache` versus
  `Durable`, flush, close, and completion correlation;
- `NullWorldStore`, `MemoryWorldStore`, compatibility filesystem stores, and
  `SqliteWorldStore`; and
- typed metadata, dimension, chunk, entity-chunk, and player codecs.

`WorldStore` is synchronous beneath the actor. `SavedData` has a reserved load
request/key but no complete save path. `ChunkStoreError` currently exposes only
IO, invalid-data, and closed categories.

### Browser

`native/apps/mclone-web-client/src/web_server_worker.rs` currently owns a
`WebIndexedDbWorldStoreState` memory mirror and synchronous
`WebIndexedDbWorldStore`. Missing chunk/entity/player loads leave Rust through
the external-load actor; dirty metadata/dimension/chunk/entity/player records
are projected into operation results.

`www/mclone-integrated-server-worker.ts` understands those domain-specific
requests and results, performs IndexedDB reads/writes, and feeds completions
back to Rust. Tactical 198's ordinary catalog continuation already proves the
target generic plan/executor pattern, including read-dependent writes inside
one live transaction.

### Platform lifecycle

- Native SQLite is live on desktop, dedicated server, flat Android, and Quest.
- Flat Android calls shared `on_background()` during suspension.
- Android XR flushes through the shared Pause/Stop lifecycle path.
- Browser IndexedDB already has mutation/reopen and periodic-cylinder reopen
  smokes, but no cross-tab writer lease or quota-specific error state.

## Autonomous Stop Triggers

Stop and request human direction only if one of these becomes real:

1. Existing durable SQLite or IndexedDB data needs migration, reset, or a
   physical schema/version change.
2. The executor must expose engine record meaning to TypeScript or platform
   database primitives to the authoritative engine.
3. Correctness appears to require a long-lived old/new production fallback.
4. Flush, close, pending-write visibility, revision precedence, atomicity, or
   durability must be weakened.
5. Required browsers cannot provide a safe Worker-visible Web Lock and a
   fenced alternative materially changes the design.
6. A measured native tick/frame, browser frame/transaction, Android lifecycle,
   memory, or copy regression remains material after bounded investigation.
7. Native local-filesystem writer locks or Windows delete ordering cannot meet
   the world-scoped lease contract.
8. A destructive/reset UX, automatic lock steal, or new persistent-world
   failure policy beyond the accepted save-unhealthy boundary is required.

Routine compilation/test failures, refactors inside the accepted boundary,
test fixture updates that preserve formats, and ordinary performance repair do
not trigger a stop.

## Slice 0: Tactical And Frozen Baseline

Status: complete 2026-07-20.

Deliverables:

- author this tactical and mark the topic active;
- record current schema/version/store/key facts;
- run shared server/app-runtime/web unit controls;
- run current ordinary IndexedDB mutation/reopen, periodic reopen, catalog,
  managed isolation, and browser app smokes;
- capture current TypeScript ownership/source locks and relevant line counts;
- record current transaction/operation/frame timing exposed by the smokes; and
- commit the documentation plus baseline evidence before production changes.

Exit: behavior and physical formats are reproducible; any pre-existing red lane
is diagnosed rather than attributed to the refactor.

### Frozen facts and evidence

- SQLite `PRAGMA user_version` remains `2`. The durable tables are
  `world_metadata`, `dimension_records`, `chunk_records`,
  `entity_chunk_records`, `player_records`, and `saved_data_records`;
  coordinates and logical ids are primary-key material while codec version,
  revision, and opaque record bytes remain stored values.
- IndexedDB remains database `mclone-web-worlds` version `6`. Opened ordinary
  worlds use `worldMetadata`, `dimensions`, `dimensionChunks`,
  `dimensionEntityChunks`, and `players`, with the established compound keys
  and `worldId` indices. `worlds` and `managedWorlds` retain their separate
  catalog/managed roles. The legacy `chunks` and `entityChunks` migration
  names remain reserved.
- The authored browser baseline is 5,686 TypeScript lines in 15 modules.
  Ownership inventory reports 1,288 lines in `server-workers`, 2,420 in
  `web-app`, 874 in `catalog-settings`, and zero registered domain-debt
  violations. The current persistence concentration is 919 lines in
  `mclone-integrated-server-worker.ts`; the Rust browser worker is 3,913 lines
  and shared `persistence.rs` is 6,479 lines.
- `cargo test -p mclone-server -p mclone-app-runtime -p mclone-web-client`
  passed: 512 server tests, 293 app-runtime tests, 47 web-client unit tests,
  and all web ABI/scenario/catalog integration locks.
- `pnpm native:web:worker-ownership` passed all 34 domain locks and retained
  the seven-entry copy ledger.
- The first `pnpm native:web:typecheck` exposed a pre-campaign JavaScript
  annotation regression in `mclone-web-smoke.js`, introduced with the catalog
  timing helper. Adding explicit parameter and result-map types restored the
  lane; the rerun passed before persistence changes.
- `pnpm native:web:indexeddb-smoke` passed mutation, background save, reload,
  block-state recovery, day-time recovery, and record-count preservation. Both
  sides reported 121 chunks, two entity chunks, one dimension, one world
  metadata record, and 124 total records. The observed pre-reload maximum
  frame gap was 20.01 ms.
- `pnpm native:web:catalog-smoke` passed create/open/delete/non-resurrection;
  its two profiled world phases observed 20.69 ms and 74.66 ms maximum frame
  gaps, retained as noisy before/after comparison points rather than budgets.
- `pnpm native:web:managed-scenario-storage-smoke` passed cancellation,
  concurrent provision/reuse, partial/incompatible repair, corrupt refusal,
  reopen, and catalog isolation. The observed maxima were 64.48 ms materialize,
  72.58 ms IndexedDB write, and 0.145 ms main-thread submit.
- `pnpm native:web:app-smoke` passed the full app loop. The IndexedDB reload,
  catalog UI, and app canvas captures under `/tmp` were visually inspected and
  showed healthy textured world/UI output.

## Slice 1: Conformance And Typed Failure Contract

Status: complete 2026-07-20.

Add a reusable persistence behavior harness before moving backend ownership:

- hit, miss, corrupt record, and version compatibility;
- pending-write read visibility;
- revision precedence and superseded acknowledgements;
- cache/durable scheduling and bounded coalescing;
- explicit batch atomicity and injected failure;
- flush/close barriers and post-close rejection;
- every current durable record family across reopen;
- typed quota, unavailable/access, lease-conflict, corrupt/incompatible,
  closed/cancelled, and backend diagnostic errors; and
- deterministic fault injection for memory/test executors.

Extend SavedData only as required to remove a reserved incomplete contract;
do not broaden gameplay saved-data content in this campaign.

Exit: one contract describes every backend; current backends pass through
compatibility adapters before extraction.

Evidence and decisions:

- `PersistenceErrorKind` now classifies IO, invalid data, corruption,
  incompatibility, quota, unavailable access, writer-lease conflict, closed,
  cancellation, and backend failure without parsing diagnostics.
- The owned `PersistenceRecordRequest`/`PersistenceRecordResponse` vocabulary
  fixes stable namespace ids, generic key parts, opaque payloads, atomic
  put/delete batches, probes, flush, close, and request correlation.
- `MemoryRecordExecutor` and `NullRecordExecutor` prove the physical contract.
  The memory executor supplies deterministic read/probe/commit/flush/close
  fault injection; its failed multi-mutation commit proves apply-all-or-none.
- `RecordExecutorWorldStore` is the synchronous compatibility/codec adapter.
  Its conformance proof covers misses, all five live record families, flush,
  close/reopen, persisted revision precedence, and corrupt-byte
  classification. Existing actor tests continue to own pending visibility,
  durability-lane ordering, coalescing, superseded acknowledgements, and close
  behavior.
- `SavedData` remains an explicitly reserved unsupported engine family. Making
  it live in IndexedDB would require either a database-version/store change or
  an opaque-key hack in another family, both excluded by the frozen schema and
  not required by a consumer. The generic namespace remains reserved so a
  later schema decision does not change the executor ABI.
- `cargo test -p mclone-server -p mclone-app-runtime -p mclone-web-client`
  passed with 517 server tests (five new contract tests in that run), 293
  app-runtime tests, 47 web-client tests, and all integration locks. The
  focused adapter expansion then passed eight record-executor tests.

## Slice 2: Coordinator And Record-Executor Seam

Status: complete 2026-07-20.

Separate shared persistence semantics from physical calls under the existing
typed mailbox:

- Rust coordinator owns logical keys, codecs, revisions, pending visibility,
  coalescing, durability, flush, close, and errors;
- the executor accepts stable namespaces/keys, opaque bytes, bounded atomic
  write/delete batches, and owned completions;
- scans/conditional add/delete are introduced only for demonstrated consumers;
- memory/null run through the seam first; and
- no engine or platform app call site changes behavior.

Exit: memory/null conformance is green and the synchronous compatibility path
is no longer the only way to serve the coordinator.

Evidence:

- The existing `PersistenceActor` remains the single owner of pending-write
  visibility, cache/durable ordering, coalescing, flush, close, and typed
  engine completions. `RecordExecutorWorldStore` supplies physical record
  reads/commits beneath that actor; the owned request/response vocabulary is
  the nonblocking completion path for browser execution.
- `PersistenceMailbox::memory()` and `PersistenceMailbox::transient()` now run
  through memory/null record executors, respectively. The legacy public
  `MemoryWorldStore` and `NullWorldStore` remain compatibility types for test
  fixtures and callers not yet routed through the mailbox constructor.
- All 39 focused persistence actor/codec/SQLite tests passed after the live
  memory/null constructor cutover, including pending reads, durability lanes,
  close behavior, threaded behavior, and SQLite controls.

## Slice 3: SQLite Control And Native Writer Lease

Status: planned.

- adapt SQLite to the executor without changing schema or record bytes;
- keep the connection and world writer lease inside the threaded storage actor;
- acquire `world.writer.lock` with nonblocking `File::try_lock()` before a
  writer connection;
- return typed same-world conflict while allowing multiple clients and
  different-world servers;
- add an explicit read-only inspection path for live durability validation;
- acquire the same world lease for delete and close the Windows release/delete
  race with an explicit ordering protocol;
- test same-process, subprocess, crash, stale lock-file metadata, close/reopen,
  different-world, and multiple-client positive controls; and
- preserve native autosave, checkpoint, startup, and shutdown behavior.

Exit: memory and SQLite are full controls through the extracted seam; desktop
and dedicated same-world split brain is rejected.

## Slice 4: Generic IndexedDB Executor Proof

Status: planned.

Build the browser executor below the same coordinator without cutting
production yet:

- Rust emits generic store/key/byte transaction plans and owns continuations;
- TypeScript maps stable store ids to the existing schema and executes only
  requests/transactions;
- owned results return after promises without borrowed Rust/JS state;
- read-dependent writes stay inside the IndexedDB success callback;
- bounded batching/backpressure and byte/copy metrics are explicit;
- quota/abort/constraint/data-clone failures map to typed Rust categories; and
- the browser conformance harness runs against real IndexedDB.

Exit: an isolated real IndexedDB backend passes the same semantics as memory
and SQLite while production still uses the old bridge.

## Slice 5: Atomic Browser World Cutover

Status: planned.

- acquire a world-keyed exclusive Web Lock in the integrated-server Worker;
- drive metadata, dimensions, chunks, entity chunks, players, SavedData slot,
  write acknowledgements, flush, and close through the generic executor;
- preserve IndexedDB version, stores, keys, records, and startup behavior;
- preserve world catalog and managed-content isolation;
- delete `WebIndexedDbWorldStoreState`, `WebIndexedDbWorldStore`, external-load
  request projection, dirty-record result projection, and corresponding
  TypeScript record switchboard in the same production cutover; and
- retain only generic IndexedDB/Web Lock mechanics in TypeScript.

Exit: production ordinary and periodic local worlds reopen through the unified
backend, and no runtime selector can re-enter the old path.

## Slice 6: Lifecycle, Quota, And Lease Recovery

Status: planned.

- normal close drains all prior durable work before releasing either lock;
- worker/process termination releases locks and permits reacquisition;
- stale native diagnostic files never block a valid reacquire;
- rejected owners cannot save or delete;
- browser quota failure enters typed persistent save-unhealthy state and never
  reports successful autosave;
- browser persistent-storage availability/request/grant is observable while
  request timing remains a product-policy hook;
- Android/Quest autosave keeps the Pause/Stop drain bounded; and
- forced close reports possible save loss rather than silently pretending
  durability.

Exit: lifecycle failure is explicit and recoverable without split brain or
stale locks.

## Slice 7: Cross-Platform Acceptance And Performance

Status: planned.

Required evidence:

- full relevant Rust workspace tests and wasm target check;
- native SQLite create/mutate/flush/close/reopen and process-lock probes;
- browser ordinary IndexedDB mutation/reopen and periodic-cylinder reopen;
- browser two-tab/Worker same-world conflict and different-world concurrency;
- catalog constraint-abort/non-resurrection and managed storage isolation;
- browser app, movement/chunk, and lifecycle smokes;
- flat Android and Quest scripted APK/build validation through repository
  scripts, with shared second-reader/background regression;
- a batched Windows native lock and delete-ordering smoke when on that host;
- before/after storage latency, transaction/batch/copy/backlog, maximum browser
  frame gap, native tick impact, and Android pause-flush timing; and
- no unexplained TypeScript domain-knowledge regression.

Pixel output is inspected only if a rendered smoke is needed to prove loaded
world state; screenshots stay under `/tmp`.

Exit: behavior, durability, lifecycle, and performance are accepted across all
affected platform boundaries.

## Slice 8: Closeout

Status: planned.

- update the topic with final contracts, code map, evidence, and remaining
  deliberately deferred work;
- reconcile stale persistence/loading architecture statements;
- mark this tactical complete and update its index row;
- run final diff/source-lock checks; and
- commit the final execution record.

The campaign stops at the opened-world unification result. Reusing the executor
for managed provisioning or every catalog long-tail operation is a new
cost/benefit decision, not automatic continuation.

## Commit Plan

Use `Topic: unified-persistence-interface` throughout. Prefer one independently
green commit for each substantive slice:

1. tactical and baseline;
2. conformance/error contract;
3. coordinator/executor seam;
4. SQLite and native writer lease;
5. IndexedDB executor proof;
6. browser cutover and old-bridge deletion;
7. lifecycle/platform/performance hardening; and
8. closeout documentation.
