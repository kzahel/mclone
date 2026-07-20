# Tactical 202: Web Scene Async Boundary Cleanup

Status: active 2026-07-20. Fresh post-Tactical-201 ownership review is
complete; implementation has not started.

Topic: `cross-platform-operation-execution`

Related topics:

- [`../topics/web-worker-runtime-ownership.md`](../topics/web-worker-runtime-ownership.md)
- [`../topics/unified-persistence-interface.md`](../topics/unified-persistence-interface.md)

## Goal

Finish the smallest ownership cleanup justified by the production system after
Tactical 201. Reuse the existing shared scene/session coordinators and
Worker-resident Rust actors. Remove the remaining places where TypeScript
selects an engine operation, exposes an internal world-start descriptor, or
reconstructs an engine readiness/result fact.

This is not a new actor framework and not a replacement managed-provisioning
system. It is a bounded cleanup behind owners that already exist.

## Fresh Review Result

The post-Tactical-201 inventory found 5,374 authored TypeScript lines across
16 modules, five Worker entries, one generic Worker-construction site, and zero
hits across the existing 44-entry domain-debt ledger. The ledger is useful but
not complete: code reading found four residual ownership seams that its
keyword locks do not currently express.

| Concern | Classification | Evidence and direction |
|---|---|---|
| render compiler, server jobs, remote socket | existing actor/mailbox; no work | Worker-resident Rust owns identity, dispatch, lifecycle, backpressure, and results; TypeScript executes Wasm loading, messages, SAB, and browser socket actions |
| opened-world persistence | existing actor/mailbox; no new actor | Rust owns record addresses, codecs, revisions, failure latching, and continuations; TypeScript owns namespace-to-store mapping and IndexedDB requests |
| ordinary catalog | existing Rust continuation; retain specialized adapter | Rust owns request meaning and read-dependent decisions; TypeScript owns transaction mechanics and stable store/index mapping |
| active scene-session start | bounded cleanup | `mclone-web-app.ts` still branches on `localWorld` versus `remote`, rebuilds seed/world/request arguments, and calls different Rust start methods even though `ExternalSceneSessionStart` already contains the decision |
| browser session lifecycle | bounded deletion | `WebSceneSessionLifecycle` mirrors starting/active/failed/reconnect/shutdown state, but production begins its start token only after the async runtime has already been built; its reconnect path is test-only and it does not protect the promise it claims to fence |
| lobby runtime start | bounded cleanup | Rust owns the pending start, storage source, role, and stale check, but TypeScript receives a clear descriptor and uses a request-id map before asking Rust to turn it back into an opaque start ticket |
| startup/streaming settle | bounded cleanup | Rust emits `streamingIdle` and owns the underlying queue facts, while TypeScript recomputes the predicate and adds its own six-frame initial-presentation rule |
| integrated-server completion envelope | bounded cleanup | Rust already authors the ready report; TypeScript redundantly rewrites `kind`, `requestId`, and `updates` after servicing generic persistence requests |
| DOM input, touch layout, browser settings, rAF, presentation | browser adapter; no actor work | These modules translate physical browser events, retain browser-only controls and preferences, and project diagnostics; they do not own authoritative gameplay state |

No surviving operation justifies a new shared actor. The shared
`GameSessionCoordinator`, `ExternalSceneSessionStart`, lobby operation ledger,
integrated-server actor, render actor, remote actor, persistence coordinator,
and catalog continuation are the owners to expose more faithfully.

## Fixed Contracts

1. TypeScript may construct Workers, load Wasm, fetch assets, drive promises,
   schedule rAF/timers/yields, execute IndexedDB/Web Locks/WebSocket calls, and
   move opaque messages or shared buffers.
2. TypeScript must not choose local versus remote session semantics, rebuild a
   `SessionStartRequest`, inspect a lobby storage source/role/behavior profile,
   or decide when an engine operation is complete.
3. Shared `McloneSceneHost` session and lobby coordinators remain the semantic
   owners. Browser Rust lowers their decisions into browser-executable start
   tickets and folds outcomes back through the same shared completion paths.
4. Active-session and retained-lobby startup behavior, cancellation, stale
   completion rejection, catalog selection, app-private fallback, and
   A-to-B-to-A activation remain unchanged.
5. The browser main thread remains nonblocking. No exported mutable Wasm borrow
   or JavaScript view crosses an `await` unless the existing generated async
   method deliberately owns that borrow for the entire operation.
6. The native path gains no browser-shaped frames, serialization, promise
   types, or extra indirection.
7. Worker count, private Wasm heaps, external SAB ABIs, IndexedDB version/store
   names/key paths, Web Locks, and storage transaction shapes remain unchanged.
8. TypeScript may retain a last-resort transport/bootstrap failure envelope
   when no Rust actor can exist because Wasm loading or FFI bootstrap failed.
   Ordinary actor failures remain Rust-authored.

## Existing IndexedDB Migration Exception

`mclone-web-world-catalog.ts` still performs the accepted v5-to-v6 cursor copy
and assigns legacy rows to `minecraft:overworld` during `onupgradeneeded`.
That is a real semantic exception to the otherwise domain-blind adapter, not a
newly discovered reason for an operation actor.

Tactical 185 made reopening v5 worlds an explicit compatibility contract.
Deleting or moving that migration changes durable-data behavior, while this
tactical promises no database-version or migration change. Keep the existing
exception quarantined and add a guard against new TypeScript migration policy.
If the product later decides old internal browser worlds are disposable, or
provides a pre-admission conversion tool, remove the exception in a separate
data-compatibility tactical. Do not generalize it into a runtime migration
framework here.

## Slice 0: Baseline And Ownership Locks

Status: complete 2026-07-20.

- record this fresh review and its three-way classification;
- freeze the current authored TypeScript/Worker/copy inventory;
- add planned source locks for the residual semantic branches before cutover;
- identify the v5-to-v6 migration as a retained compatibility exception; and
- run the focused shared/web controls before behavior changes.

Exit: the successor is justified by current code rather than the deleted
managed installer, and its boundary can be checked mechanically.

Evidence:

- `web_scene_async_boundary_lock` pins the six clear TypeScript session/lobby
  dispatch sites, the redundant web lifecycle, the streaming-settle
  reconstruction, the ready-envelope rewrite, and the single accepted legacy
  migration exception;
- the focused lock passed three tests;
- the Wasm staging/generated-bindgen TypeScript build and authored TypeScript
  typecheck passed; and
- the Worker ownership self-test passed at 5,374 lines, 16 modules, five Worker
  entries, one generic construction site, 44 zero-debt entries, and the
  unchanged seven-copy ledger.

## Slice 1: Delete The Redundant Web Session Lifecycle

Status: complete 2026-07-20.

- remove `WebSceneSessionLifecycle`, its browser-only operation/result/state
  enums, and the unused production reconnect state machine;
- retain clock projection and deferred catalog services in the web platform
  assembly;
- rely on shared `GameSessionCoordinator`/`McloneSceneHost` state for session
  UI and completion; and
- prove start, failure, shutdown, and replacement behavior remain unchanged.

Exit: there is one session lifecycle owner, not a shared coordinator plus a
browser-only mirror whose token begins after the asynchronous work.

Evidence:

- `WebSceneSessionLifecycle`, all four browser-only operation/result/state
  enums, and the test-only reconnect state machine are deleted;
- initial host construction, replacement completion, and shutdown now use the
  already-authoritative `McloneSceneHost` lifecycle directly;
- the unchanged five-bit adapter receipt now proves projected time, shared
  `GameSessionCoordinator` state, shared operation-ledger stale rejection,
  typed catalog completion, and catalog epoch invalidation instead of testing
  the deleted mirror; and
- the focused adapter, frame-policy, and source-lock tests passed, as did the
  `wasm32-unknown-unknown` package check.

## Slice 2: Rust-Owned Active Session Start Dispatch

- expose one browser-Rust entry that consumes the already classified pending
  `ExternalSceneSessionStart`;
- match local transient, local IndexedDB, and remote runtime construction in
  browser Rust;
- pass only browser resource URLs/capabilities from TypeScript;
- delete TypeScript parsing of session kind, seed, world id, display name, and
  create/open request kind for execution; and
- preserve the current async borrow/exclusion behavior unless a separate
  tokened-start change is demonstrably needed.

Exit: changing `SessionStartRequest` variants or source policy requires Rust
changes only; TypeScript invokes one mechanical start operation.

## Slice 3: Opaque Lobby Runtime Tickets

- let browser Rust turn a queued lobby start directly into an owned opaque
  runtime-start ticket;
- remove the clear lobby operation report, request-id map, and separate
  prepare call;
- let TypeScript only start the ticket, yield while it warms, and return it to
  Rust for completion;
- retain Rust-owned cancellation epoch and stale-result rejection; and
- keep concurrent active-world rendering and destination warmup unchanged.

Exit: lobby source, role, seed, behavior, instance identity, and storage choice
never enter TypeScript execution code.

## Slice 4: Rust-Authored Readiness And Envelopes

- make browser Rust expose the complete streaming/initial-presentation fact
  needed by the warmup driver;
- remove TypeScript reconstruction from server, render, and residency queue
  counters while retaining those counters as diagnostics;
- remove redundant TypeScript mutation of the Rust-authored integrated-server
  ready envelope;
- review generic continuation-loop bounds and keep only browser-mechanical
  safety limits in TypeScript; and
- extend the ownership checker so these semantic reconstructions cannot
  silently return.

Exit: TypeScript decides when to yield or post, but Rust decides what ready,
complete, stale, or failed means.

## Slice 5: Validation And Closeout

- focused shared session/lobby/catalog/persistence tests;
- web Rust unit/source locks, Wasm check, generated-bindgen TypeScript check,
  TypeScript typecheck, and Worker-ownership self-test;
- ordinary local, IndexedDB reopen, remote WebSocket, catalog CRUD, lobby,
  lifecycle, and mobile browser smokes affected by the changed seam;
- desktop/native controls proving no shared or native regression;
- rendered-output inspection at the first affected drawable milestone, with
  the current Linux Chromium transparency limitation reported honestly if it
  recurs; and
- Android/Quest build lanes only if shared code outside web-only adapters is
  changed materially.

Close when production TypeScript no longer selects session runtime meaning,
sees lobby start semantics, or recomputes readiness/completion policy, and no
replacement actor or generalized browser transaction language was added.

## Stop Conditions

Stop for review if:

- correct active-session cancellation requires a new shared token/epoch
  contract rather than the existing coordinator behavior;
- browser runtime construction cannot be selected in Rust without holding a
  mutable host borrow across unrelated event-loop work;
- removing the web lifecycle reveals a production reconnect requirement not
  represented in shared session policy;
- an IndexedDB version/schema or durable-data migration becomes necessary;
- any change would expose domain records or decisions to TypeScript; or
- browser or native behavior requires different session/lobby semantics.

Routine Rust/TypeScript refactoring, generated binding changes, and browser
adapter mechanics are not stop conditions.

## Non-Goals

- a managed or downloadable content installer;
- a new general actor, executor, mailbox ABI, or shared Wasm heap;
- moving IndexedDB, WebSocket, DOM, rAF, or Worker APIs wholesale into Rust;
- changing input bindings, touch layout, browser preferences, or UI design;
- deleting or redesigning the v5-to-v6 browser data migration;
- changing database schemas, storage sharding, or writer leases; and
- changing lobby product behavior, persistence, or rendering.
