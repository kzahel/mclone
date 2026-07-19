# Tactical 198: Opaque WebSocket And IndexedDB Adapters

Status: approved and active 2026-07-19.

Topic: `web-worker-runtime-ownership`

Workstream: native web/WASM. Shared protocol and catalog policy remain in
their existing Rust crates. TypeScript retains browser WebSocket and IndexedDB
mechanics.

## Result

Continue Tactical 197's isolated-Rust-actor direction through the two clearest
remaining domain-aware browser adapters:

1. move remote WebSocket handshake, protocol, batch, backpressure, and shutdown
   semantics into a worker-resident Rust actor; and
2. move ordinary local-world catalog operation selection and record policy into
   a Rust-owned continuation while TypeScript executes only IndexedDB CRUD and
   transaction mechanics.

The campaign is ownership-only. Preserve private Wasm heaps, current Workers,
message transfers, IndexedDB version 6, store names and keys, transaction
boundaries, catalog serialization, error behavior, and runtime performance.

Managed-scenario provisioning and integrated-server persistence are out of
scope. Stop with a review package before either extension.

## Motivation

Tactical 197 removed every registered Worker-domain owner but deliberately
stopped before browser-native long-tail work. Human review subsequently found
two remaining surfaces where TypeScript still understands more than browser
mechanics require.

`mclone-remote-websocket-worker.ts` owns `handshakeComplete`, command/update
message selection, canonical protocol calls, keepalive response selection,
batch sequence and release accounting, two queue limits, and shutdown state.
The browser must own `WebSocket`; it need not own those protocol semantics.

`mclone-web-app.ts` receives a loose report bag, switches over
`listWorlds`/`createWorld`/`openWorld`/record/delete/reset, rebuilds create
options, and applies an `overworld` fallback before entering the IndexedDB
adapter. This makes the platform app a second catalog message decoder.

The target is not fewer lines by itself. TypeScript may remain substantial
where it owns DOM, rAF, input, IndexedDB upgrades and requests, WebSocket
events, Workers, promises, and diagnostics. It must not select engine
operations or reproduce engine defaults merely because those browser APIs are
asynchronous.

## Current Async Semantics To Preserve

### Ordinary catalog

The current production path is nonblocking:

1. shared Rust queues a typed tokened `WorldCatalogRequest`;
2. web Rust projects the operation and arguments into a JavaScript report;
3. `mclone-web-app.ts` serializes requests through
   `worldCatalogOperationTail`;
4. TypeScript opens IndexedDB, selects the operation, and performs one or more
   browser transactions;
5. TypeScript calls Rust synchronously for record policy before or during those
   transactions;
6. after the browser promises settle, TypeScript calls
   `applyWorldCatalogResponse` or `applyWorldCatalogError`; and
7. Rust validates the token and folds the result through shared catalog/UI
   policy.

Rust does not block on IndexedDB. No Rust borrow is held while JavaScript
awaits. The Wasm heap simply retains the pending token and scene state until a
later browser event calls Rust again.

Operations are serialized by the promise tail. Preserve these transaction
shapes exactly:

| Operation | Current browser transaction shape |
|---|---|
| list | one readonly `getAll` |
| create | read existing rows, then separate read/write `add` |
| open | readonly `get`, then separate read/write `put` |
| record played | `get` and `put` in one read/write transaction |
| delete | clear world records, then delete catalog row |
| delete all/reset | ordered per-world deletion, then existing store clears |

The record-played transaction is load-bearing: its existence check and recency
write share a transaction so a concurrent delete cannot be followed by a stale
write that resurrects the row.

### Remote WebSocket

Main Rust creates the browser Worker and awaits a readiness Promise. The Rust
future yields to the browser; it does not block the event loop. Worker
TypeScript dynamically loads its independent Wasm instance, creates the
WebSocket, receives browser callbacks, calls synchronous Rust codec exports,
and posts structured protocol reports to main Rust. Main Rust decodes those
reports into its ready-only client connection.

Preserve the one-Worker/private-heap topology, transferable update frames,
64-MiB worker and main update bounds, 8-MiB socket command-buffer bound,
ordered batch release, metrics, and graceful/forced shutdown behavior.

## Target Async Boundary

Rust must never synchronously wait, spin, or hold an exported mutable borrow
across `await`.

### Remote actor

A worker-resident Rust actor consumes opaque main frames plus browser events:

- initialized;
- socket opened;
- socket binary frame received;
- current `bufferedAmount`;
- socket error or close;
- main update-batch release; and
- shutdown.

It synchronously returns browser-mechanics actions:

- open this URL;
- send these opaque bytes;
- post this opaque actor report and transfer buffers;
- close; or
- remain idle.

TypeScript loads Wasm, constructs `WebSocket`, forwards events/bytes, executes
those actions, and closes. Rust owns handshake state, protocol decoding,
keepalive choice, command canonicalization, batch identity/accounting,
backpressure decisions, typed failure, and idempotent shutdown.

Main Rust should author and decode the actor frames. The Worker shell may know
generic request IDs, URLs, byte buffers, browser action kinds, and errors; it
must not know handshake or engine update/command variants.

### Catalog continuation

Use an owned browser-specific Rust execution object or equivalent tokened
continuation. Do not use an exported `async fn &mut WebSceneHost` that keeps the
host borrowed while IndexedDB runs.

The continuation produces storage-level actions only:

- read key/all/index range;
- add/put opaque record;
- delete key/index range;
- clear stores;
- commit or abort.

TypeScript maps stable storage identifiers to the existing IndexedDB schema,
starts requests, and returns opaque results. Rust owns the catalog operation,
arguments, timestamps' catalog meaning, record descriptors, active-world
validation, response variant, and failure mapping.

For read-dependent work inside one transaction, the IndexedDB success callback
calls Rust synchronously with the read result. Rust returns follow-up storage
actions synchronously, and TypeScript enqueues them before returning from the
callback. There is no `await` while that transaction must remain active.

Keep `worldCatalogOperationTail` for the first cut. Rust already owns token
epochs and completion acceptance; changing browser operation concurrency is
not part of this tactical.

## Slice 0: Contract And Baseline

Status: complete 2026-07-19.

- record this tactical and the human continuation decision;
- capture final pre-cutover TypeScript ownership and copy ledgers;
- run focused remote and catalog controls;
- identify any existing red fixture separately; and
- add source locks for the exact domain vocabulary each cut will remove.

Exit: current behavior is understood and reproducible before production code
changes.

### Slice 0 evidence

The baseline ownership checker reports 5,778 authored TypeScript lines across
15 modules, six Worker entries, two TypeScript Worker construction sites, all
24 previously registered domain debts at zero, and the unchanged seven-site
copy ledger. The remote ownership source lock passed.

The production remote browser smoke passed with the existing Worker-owned
WebSocket, 49 loaded chunks, authoritative break/place interaction, zero final
command/update queue depth, and no pending compile work. Its capture at
`/tmp/mclone-native-web-app-canvas.png` was inspected and showed coherent
remote terrain, actors, HUD, and diagnostics.

The catalog UI smoke passed create/list/open/record/delete behavior, two
distinct generated IDs, descriptor-backed rows, and the legacy rewrite path.
Its maximum frame gap was 20.015 ms. The capture at
`/tmp/mclone-native-web-catalog-ui-probe-canvas.png` was inspected and showed
the expected selected active world and deletion receipt.

No pre-cutover production behavior is red in the focused lanes. The separate
lifecycle actor-ID, Far LOD suppression, and startup-camera fixture debts
recorded by Tactical 197 remain unrelated and are not weakened here.

## Slice 1: Worker-Resident Remote Protocol Actor

Status: complete 2026-07-19.

Implement and test the Rust actor, keep the TypeScript WebSocket shell, cut
production over atomically, then delete the old TypeScript protocol state.

Required focused tests:

- command before readiness;
- valid and invalid handshake;
- keepalive/control response generation;
- socket and main-queue backpressure;
- update-batch sequence, byte accounting, and release;
- error/close during startup and after readiness;
- stale events after close;
- idempotent shutdown; and
- Worker construction or startup failure containment.

Browser acceptance repeats the production remote interaction smoke five times
and preserves Worker count, byte/queue metrics, zero pending work, and rendered
remote output.

### Slice 1 evidence

`WebRemoteSocketWorkerActor` now owns the handshake and readiness state,
command canonicalization, keepalive responses, socket-buffer admission,
update-batch sequence and byte accounting, release handling, typed failures,
and idempotent shutdown. Main Rust authors strict versioned `MCRW` frames for
start, command, release, and shutdown. The old five free remote codec exports
were deleted.

The TypeScript Worker is now a 161-line browser shell. It loads the independent
Wasm instance, constructs and closes `WebSocket`, forwards browser callbacks,
executes generic `open`/`send`/`post`/`close` actor actions, and retains only a
last-resort bootstrap/FFI error envelope. It contains no handshake state,
protocol codec call, keepalive choice, batch-accounting limit, release-message
vocabulary, or command/update selector.

Nine focused actor tests cover strict frame decoding, pre-handshake command
failure, valid and invalid handshake behavior, canonical commands and
acknowledgement, keepalive generation, batch release, actor-owned
backpressure, browser failure/close reports, measured decode time, and
idempotent shutdown. A tenth test proves stale socket events after shutdown are
ignored. The inverted remote source lock and the ownership checker register
five new remote debts at zero.

The Wasm check, generated-bindgen TypeScript gate, remote source lock, and
ownership self-test pass. Five fresh production remote smokes passed with 49
loaded chunks, authoritative break/place interaction, zero final command and
update queue depth, zero pending jobs and compile work, and maximum frame gaps
of 18.620, 19.260, 18.840, 18.735, and 18.830 ms. The capture at
`/tmp/mclone-native-web-app-canvas.png` was inspected after the cutover and
showed coherent remote terrain, actors, HUD, and diagnostics.

Authored TypeScript is now 5,738 lines. The 40-line reduction is incidental;
the acceptance result is that the remote Worker has become a browser-mechanics
executor while private Wasm heaps, one-Worker topology, transferable update
frames, queue limits, and main-side client semantics remain unchanged.

## Slice 2: Rust-Owned Ordinary Catalog Continuation

Status: pending.

Replace the operation switch and argument reconstruction in
`mclone-web-app.ts` with one opaque catalog execution. Keep IndexedDB opening,
upgrade, request, cursor, transaction, and close mechanics in TypeScript.

The first cut covers ordinary catalog create/list/open/record/delete/delete-all
and factory reset. It does not convert managed content or server persistence.

Required focused tests:

- exact storage-action and transaction trace for every operation;
- record-played/delete non-resurrection;
- duplicate create/add behavior;
- legacy clear-record rewrite;
- open/read/write/abort failure mapping;
- stale and duplicate completion rejection;
- one failed queued operation does not stall the next;
- active-world delete/reset refusal; and
- full-width descriptor/seed preservation within the existing browser input
  semantics.

Browser acceptance covers catalog create/list/open/record/delete, legacy
rewrite, ordinary IndexedDB mutation/reopen, periodic-world reopen, catalog
lobby selection/activation, and unchanged managed-store isolation.

## Slice 3: Closeout And Stop

Status: pending.

Remeasure:

- authored TypeScript lines and remaining domain vocabulary;
- Worker count and lifecycle;
- remote bytes, queue depth, latency, and repeated stability;
- catalog operation/transaction counts and latency;
- maximum browser frame gap;
- IndexedDB schema/version and persisted-record compatibility; and
- failure/cancellation containment.

Stop before managed-scenario provisioning or integrated-server persistence.
Recommend any follow-up with exact evidence; do not infer it from line count.

## Validation

Core gates:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml --workspace
cargo test --manifest-path native/Cargo.toml
cargo check --manifest-path native/Cargo.toml \
  -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:typecheck
pnpm native:web:worker-ownership
pnpm native:web:scene-host-adoption
pnpm native:web:remote-smoke
pnpm native:web:catalog-smoke
pnpm native:web:indexeddb-smoke
pnpm native:web:lobby-scenario-catalog-smoke
pnpm native:web:managed-scenario-storage-smoke
```

Run five fresh remote smokes after the production remote cutover. Capture
browser pixels to `/tmp` and inspect every changed rendered lane.

## Stop Conditions

Stop for a new human decision if either slice requires:

- IndexedDB version, store, key, index, or migration changes;
- different transaction boundaries, ordering, or catalog concurrency;
- a new production Worker or shared Wasm memory;
- Rust blocking/spinning on the browser main thread;
- a Rust borrow or JavaScript view held across an async gap unsafely;
- a long-lived old/new implementation or fallback;
- weaker race, failure, persistence, pixel, or performance assertions;
- managed-scenario or integrated-server persistence conversion; or
- a material frame-time, memory, queue, Worker-count, or visual regression.

## Non-Goals

- No shared Wasm heap or new SAB ABI.
- No IndexedDB schema or migration work.
- No browser WebSocket or IndexedDB APIs in shared engine crates.
- No managed-scenario provisioning conversion.
- No integrated-server chunk/entity/player persistence conversion.
- No input, touch, DOM, rAF, WebGPU, or diagnostics cleanup.
- No TypeScript line-count target.
