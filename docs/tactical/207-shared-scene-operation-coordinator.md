# Tactical 207: Shared Scene Operation Coordinator

Status: implementation active. Slices 0–2 landed on 2026-07-21; the mandatory
post-identity decision gate is next. The browser ABI cutover and one
boundary-operation token family are live. The original proposal framed
success as deleting the remaining named TypeScript pumps. The measured
two-sided audit showed the complexity mass sits on the Rust side, so the plan
now makes Rust-side consolidation the primary deliverable — one
boundary-operation token family, a poll-shaped browser ABI, and a smaller web
lowering layer — while TypeScript deletion is the corollary that verifies it.
The obsolete browser v5-to-v6 Overworld migration is no longer a prerequisite
decision.

Topic: `cross-platform-operation-execution`

Parent concern:
[`../topics/platform-boundary-convergence.md`](../topics/platform-boundary-convergence.md)
owns the campaign-wide pass ledger, the both-language scoreboard, and the
closure protocol. This tactical may close itself; it must not declare the
parent concern done, and its closeout must append a scoreboard row there.

Related topics:

- [`../topics/cross-platform-operation-execution.md`](../topics/cross-platform-operation-execution.md)
- [`../topics/platform-host-boundary.md`](../topics/platform-host-boundary.md)
- [`../topics/unified-persistence-interface.md`](../topics/unified-persistence-interface.md)

Predecessors:

- [`201-lobby-content-simplification.md`](201-lobby-content-simplification.md)
- [`202-web-scene-async-boundary-cleanup.md`](202-web-scene-async-boundary-cleanup.md)
- [`206-browser-preferences-and-bootstrap-policy.md`](206-browser-preferences-and-bootstrap-policy.md)

## Top-Level Goal

Make shared Rust own the sequencing and lifetime of coarse scene operations
through **one** identity/completion lifecycle, expose it through a
**poll-shaped** browser ABI that never suspends a mutable scene borrow, and
delete the parallel Rust owners and per-operation lowering that currently
compensate for both. TypeScript supplies only reusable browser machinery.

```text
shared McloneSceneHost operation coordinator
  owns admission, order, identity, cancellation, stale results, and completion
        |
        v
small platform effect ports
  native adapter                 browser-Rust adapter
  direct typed work              lowers to mechanical browser effects
        |                                  |
        v                                  v
thread/direct completion          domain-blind TypeScript executors
                                  Worker / IndexedDB / fetch / scheduling
        |                                  |
        +---------------+------------------+
                        v
              typed Rust completion
```

Browser promises and callbacks are a physical suspension mechanism, not an
alternate engine coordinator. TypeScript may await a Worker, transaction, or
fetch, but it must not know whether the work starts a lobby, replaces the
active world, updates the catalog, changes an asset selection, or satisfies a
scene readiness phase.

Success is measured by deletion **on both sides of the boundary**: fewer Rust
identity systems, fewer exported methods, a net-smaller
`mclone-web-client/src`, and the removal of the named TypeScript dispatch,
state, and reports. A cut that shrinks TypeScript while growing web-only Rust
by more is a failure of this tactical, not a partial success.

## Why The Original Framing Was Inverted

Tactical 202 correctly removed TypeScript selection of local versus remote
sessions, clear lobby descriptors, duplicated readiness predicates, and the
browser-only session lifecycle mirror. It stopped at opaque Rust tickets and
concluded no replacement provisioning actor was justified. A stricter audit
then observed that the surviving named pumps still form a platform-specific
operation scheduler even though Rust owns each individual decision.

The 2026-07-21 measured audit (recorded in the parent topic) added the facts
that change the plan:

- Over the 2026-07-18 → 07-21 campaign week, authored web TypeScript fell
  3,637 lines while web-only Rust grew 5,598. The TypeScript-only metric hid
  a cost shift into the web lowering layer, which is now larger than the
  TypeScript it replaced.
- The TypeScript coordination residue is ~300–360 dedicated lines plus a
  `sessionBusy` guard threaded through ~20 call sites — all of it mirroring
  Rust-owned state. It is derivative, not a source.
- Exactly 3 `async fn(&mut WebSceneHost)` exports (`startPendingSession`,
  `shutdownAsync`, `completeAssetPackSelection`) plus
  `WebLobbyRuntimeStart::start`, out of 48 exported methods, force the entire
  `sessionBusy`/spin/take-apply structure into existence. `sessionBusy` dates
  to 2026-06-24 and has been preserved by every pass since. While one of
  those borrows is live, rAF and raw input are excluded: coarse operations
  pause the active world on web only. This is the one user-visible defect in
  scope.
- At least four overlapping operation identity/staleness systems exist in Rust: the
  generic `PlatformOperationLedger` token (catalog fully, lobby/web session
  partially), `ExternalSceneSessionStart` currentness plus a hand-rolled
  stale-completion counter, the independent asset-replacement epoch, and a
  catalog `String` request-id carried alongside its own ledger token — plus
  `asset_epoch` tags threaded through warm-world slots. Three predate the
  generic ledger; the consolidation onto it was started and never finished.
  Some asset epochs also express real content-generation compatibility, so
  the implementation must separate that invariant from operation identity
  rather than deleting every field named `asset_epoch`.
- Native scene code still starts work directly while web Rust lowers it into
  per-operation tickets TypeScript must recognize and return.

The revised conclusion: finish the consolidation that already began. Merge
the parallel identity systems into the existing ledger — do not facade over
them — reshape the ABI so no operation suspends the scene borrow, and let the
TypeScript pumps collapse into one generic drain loop as a consequence.

## Prerequisite Compatibility Decision

Old internal worlds are not a compatibility requirement. The browser
v5-to-v6 cursor copy, legacy store labels, and hard-coded
`minecraft:overworld` assignment have been deleted. Source locks now reject
their return.

This tactical must not add replacement runtime migration machinery. If a
future shipped format needs conversion, it requires a separate pre-admission
or offline design. New schema work may intentionally reject or reset
unsupported internal formats.

## Measured Baseline (2026-07-21)

The clean Tactical 207 baseline is revision `8a3e9b12`. Reproduce the source
inventory with `scripts/platform-boundary-scoreboard.sh 8a3e9b12` (omit the
revision to inspect the worktree):

| Metric | Clean baseline |
|---|---:|
| authored web TypeScript | 3,757 lines / 14 modules |
| TypeScript inventory gate | 3,780 lines / 16 modules |
| web-only Rust | 20,577 lines |
| combined authored TypeScript + web-only Rust | 24,334 lines |
| shared `mclone-scene` Rust | 24,632 lines |
| shared `mclone-app-runtime` Rust | 33,877 lines |
| `WebSceneHost` exports | 48 |
| async mutable wasm exports | 4 |
| non-wasm cfg forks, scene / app-runtime | 71 / 110 |

The worktree may report lower TypeScript totals because unrelated storage
cleanup was already present when this series started. The immutable revision
above prevents that work from being silently credited to Tactical 207.

### Slice 0 operation evidence

The query-gated smoke observer now exposes only whether the scene host is
temporarily unavailable; product execution does not depend on this hook. The
catalog and asset UI probes inject a neutral unknown-key down/up pair during
that interval and record frame, render, and raw-input counters:

| Operation | Observed exclusion | Sample / total | Progress while excluded | Progress after release |
|---|---:|---:|---:|---:|
| first catalog-created local session | yes | 27.5 / 135.7 ms | frame 0, render 0, input 0 | input +2 |
| asset-pack preparation/activation | yes | 30.6 / 572.0 ms | frame 0, render 0, input 0 | input +3 |
| lobby runtime warmup control interval | no | 193.7 ms | frame +7, render +7, input +7 | n/a |

This establishes both sides of the defect: an async mutable scene borrow
stops ordinary work, while the same main loop advances all three counters
during independent runtime work that does not retain the scene borrow. The
existing native lobby scenario also completed with six captures and two
switches; the inspected title, settled-lobby, and destination images were
drawn correctly. Native has no global scene-borrow exclusion guard.

`shutdownAsync` is structurally in the debt count but performs no asynchronous
work internally: it calls synchronous `shutdown()` and returns on the next
promise turn. It therefore has no legitimate suspension to preserve.

The catalog, asset-pack, and lobby runtime semantic probes passed. Their web
page/canvas PNGs were fully black/transparent and failed the existing pixel
gate even though semantic counters and operation reports were healthy. Those
artifacts were inspected rather than counted as pixel passes. The broader
lobby lifecycle probe also timed out before preview activation, before the
new held-operation sample. Both are retained validation issues for the later
cutover matrix, not evidence that the measured borrow exclusion passed a
rendered-output gate.

### Identifier and generation classification

| Identifier | Classification and cutover treatment |
|---|---|
| `PlatformOperationToken` | Boundary-operation identity; keep as the sole family. |
| catalog `String request_id` | Duplicate boundary identity carried beside the ledger token; delete in Slice 2. |
| external-session request/currentness and stale lobby completion counter | Duplicate boundary identity/acceptance diagnostics; re-key to the ledger and delete in Slice 2. |
| asset replacement/selection epoch used to accept platform completion | Duplicate boundary identity; replace with the ledger token in Slice 2. |
| asset generation carried by prepared assets, active assets, compiler, and warm-world slots | Durable content-compatibility generation; retain, name explicitly, and never use as boundary completion identity. |
| render-worker generation | Worker/resource lifecycle generation; retain outside the coarse-operation token family. |
| render resource generation | Renderer resource-rebuild generation; retain outside the coarse-operation token family. |
| render compile request ID | Specialized render-actor/mailbox work identity, not a scene boundary-operation identity; retain. |
| world/realm instance IDs, storage IDs, role and slot IDs | Domain identity; retain and do not substitute for operation tokens. |

`platform_boundary_convergence_debt.rs` pins the current TypeScript
coordination fields/branches and the targeted Rust async/identity paths to
non-increasing source ceilings. Every deletion slice must lower the relevant
ceilings; zero remains the cutover target.

### Slice 1 cutover evidence

All long-running starts and asset preparation now use owned, Promise-backed
Rust tickets. Calling `start()` synchronously moves the effect into a
Promise-owned future; only later does a synchronous `completeRuntimeStart` or
`applyAssetPackPreparation` call fold the owned result back into the host.
Active local, active remote, lobby-primary, and lobby-destination starts use
the same `WebRuntimeStart` implementation. Asset preparation retains its
specialized render-worker effect and content-generation checks.

The cut deleted every exported async mutable borrow, `shutdownAsync`, the
TypeScript `sessionBusy` mutex, `waitForSessionIdle`, the animation-frame and
raw-input exclusions, and their setTimeout retry loops. A generic set of
outstanding scene-operation promises remains solely so observer shutdown can
wait for quiescence; it does not gate ordinary host calls. Catalog execution
no longer waits for an unrelated host mutex.

The same query-gated traces that measured exclusion now report:

| Operation | Sample | Scene callable | Frame / render / input progress |
|---|---:|---:|---:|
| catalog-created active local session | 116.5 ms | yes throughout | +4 / +4 / +4 |
| asset-pack preparation | 55.8 ms | yes throughout | +3 / +3 / +3 |
| lobby primary/destination warmup | 164.0 ms | yes throughout | +6 / +6 / +6 |

The base web smoke, threading/worker checks, synchronous shutdown path, lobby
runtime semantic probe, catalog semantic probe, asset replacement/reload
semantic probe, and IndexedDB reload semantics passed. The remote-session
smoke also reached its semantic result and stopped only at the same
transparent-black app screenshot gate. Catalog, asset, and IndexedDB headed
lanes likewise retain that pre-existing capture failure; no such pixel result
is counted as a pass.

Relative to the clean `8a3e9b12` baseline, authored TypeScript is 3,685 lines
(-72), web-only Rust is 20,702 (+125), and the combined boundary is 24,387
(+53). Relative to the Phase 0 evidence commit, the cut is -74 TypeScript,
+125 Rust, and +51 combined. The temporary Rust growth is the two owned ticket
states and their Promise-owned effect runners; the measured behavior gain
permits this phase-local increase, but Tactical 207 still owes a net-negative
combined closeout. `WebSceneHost` remains at 48 exports; async mutable exports
are 4 -> 0, and the wasm cfg counts remain 71 / 110.

### Slice 2 identity evidence

Active-session and lobby runtime starts now carry opaque
`PlatformOperationToken` values and are accepted through shared-ledger
resolution before either world slot can be installed. The parallel
`external_scene_start_is_current` predicate and browser-only
`stale_lobby_start_completion_count` are deleted. Replacement and teardown
advance the ledger epoch, so late runtimes and failures resolve as stale.

External asset preparation now has the same issue/completion identity. Its
opaque ticket retains the operation token while `content_generation` names
the distinct resource invariant passed to prepared assets and the render
worker. Retained warm-world `asset_epoch` fields are documented as content-
generation compatibility checks across CPU, compiler, GPU, active, standby,
and preview resources; none identify a platform completion.

Catalog execution no longer reports or accepts a string request ID. The
ledger token remains inside the opaque `WebCatalogExecution` ticket, so
TypeScript submits an exact success or failure without inspecting identity or
relying on FIFO completion order. Generic shared-ledger tests cover unique
ordering, concurrent out-of-order completion, duplicate and unknown results,
failure restoration, cancellation, executor replacement, teardown, and late
completion. Scene ownership locks prove ledger acceptance precedes active or
standby slot installation and world teardown cancels session-start and asset-
preparation work.

The catalog, asset-pack, and lobby runtime semantic probes passed. Their held-
operation traces still showed +4/+4/+4, +3/+3/+3, and +6/+6/+6 frame/render/
input progress respectively. The catalog probe again failed only its existing
transparent-black canvas pixel gate; the asset and lobby screenshots were
also inspected as black even where their commands accepted semantic results.

Relative to Phase 1, authored TypeScript is 3,686 lines (+1), web-only Rust is
20,695 (-7), and their combined boundary is 24,381 (-6). Relative to the
clean baseline the combined boundary remains +47, so later deletion is still
required. Shared `mclone-scene` is 24,720 lines (+88 in this slice) because it
now owns the session and asset ledgers; `mclone-app-runtime` remains 33,877.
Exports remain 48, async mutable exports remain zero, and cfg counts remain
71 / 110. Identity/staleness systems are 4 -> 1.

### Product TypeScript coordination state

`mclone-web-app.ts` currently owns:

- `sessionBusy`, which excludes rAF, raw input, resize, and other host calls
  while a wasm-bindgen `async &mut WebSceneHost` borrow is alive, and is
  threaded through ~20 otherwise-ordinary call sites;
- `pendingLobbyRuntimeStarts`, `lobbyOperationDrainActive`, and
  `worldCatalogOperationTail`;
- separate `dispatchSceneSessionOperation`, `dispatchWorldCatalogOperation`,
  and `dispatchAssetPackOperation` branches;
- a lobby-specific take/start/complete/free loop; and
- a warmup loop that reads `initialPresentationReady` and
  `renderWorkerPendingRequestCount`.

Rust already owns the pending semantic state behind all of those fields. The
TypeScript state is a consequence of how that state is exposed.

### Current Rust split

- `GameSessionCoordinator` and `ExternalSceneSessionStart` own session request
  meaning and pending state, with their own currentness check and a
  hand-rolled `stale_lobby_start_completion_count`.
- The lobby launch state and `PlatformOperationLedger` own role, slot, epoch,
  cancellation, and stale completion rejection.
- `WorldCatalogExecutor` uses `PlatformOperationService` and a Rust-owned
  IndexedDB continuation, but each web catalog operation carries a second
  `String` request identity across the JS boundary.
- Asset replacement owns a third independent epoch and acceptance state.
- Browser Rust already computes complete initial-presentation readiness.
- The web lowering layer (`web_scene_host.rs` 4,295 lines,
  `web_catalog_execution.rs` 1,496, `web_scene_protocol.rs` 242) exists to
  turn shared operations into JS-drivable tickets; native has no equivalent.
- `mclone-scene/src/session.rs` is 6,542 lines carrying ~60 wasm-related cfg
  forks between direct native starts and web ticket lowering.

The missing piece is not policy. It is one shared way to issue owned work,
release the scene borrow, and fold a later platform completion back through a
single identity system.

### Genuine browser constraints

The design must preserve these facts:

- the browser main thread cannot block;
- IndexedDB and Worker startup complete through browser callbacks/promises;
- independent Web Workers have private Wasm heaps;
- a wasm-bindgen `async fn(&mut WebSceneHost)` prevents safe re-entry until
  its promise settles; and
- rAF, DOM events, Worker construction, IndexedDB transactions, fetch, and
  presentation remain browser-owned mechanics.

None of those constraints requires TypeScript to know an operation's engine
meaning, and none requires any exported operation to hold the scene borrow
across an await.

## Fixed Contracts

1. Shared Rust owns operation admission, identity, ordering, priority,
   cancellation, stale/duplicate rejection, retries, semantic failure, and
   final state installation.
2. There is exactly **one boundary-operation token/staleness family** at
   cutover: the existing
   `PlatformOperationService`/`PlatformOperationLedger` family, extended where
   needed. Session currentness, asset-preparation request acceptance, the
   catalog `String` request-id, and the hand-rolled stale counter are merged
   into it and deleted — not wrapped by a facade that leaves them alive
   underneath. Asset/content generations may remain where they prove resource
   compatibility, but they do not cross the platform boundary as a second
   operation identity.
3. Long-running browser work does not retain a mutable `WebSceneHost` borrow.
   Issuing an effect is synchronous; an owned result returns later. At
   cutover, zero exported `async fn(&mut self)` methods remain.
4. Active-session and lobby starts use one logical runtime-start lifecycle.
   Active, primary, destination, standby, or preview placement remains Rust
   target state and never enters TypeScript.
5. TypeScript may retain a generic in-flight registry only when browser API
   mechanics require it. It may not maintain separate scene, lobby, catalog,
   or asset state machines.
6. Browser effect executors see only mechanical capability vocabulary such as
   Worker construction, opaque frames, byte fetch, IndexedDB transactions,
   transfer lists, request IDs, and generic failures.
7. Native uses the same logical request/completion owner with direct typed
   execution or existing worker threads. It does not adopt promises,
   `JsValue`, encoded browser frames, SABs, or extra hot-path allocation.
8. Adding a new coarse operation type after cutover, when it uses the existing
   mechanical capability vocabulary, requires zero TypeScript changes and zero
   new `WebSceneHost` exports. New operations are new Rust request variants
   lowered to those existing mechanical effects.
9. Existing integrated-server, persistence, render-compiler, server-job, and
   remote-socket actors remain specialized. This tactical does not put every
   workload behind one universal actor or executor.
10. Opened-world persistence remains adjacent to the integrated-server actor.
    High-frequency record traffic does not round-trip through the browser
    main thread or the coarse scene-operation coordinator.
11. Catalog storage meaning and continuation remain Rust-owned. TypeScript
    may execute a mechanical IndexedDB plan but may not own a
    catalog-specific promise tail or read-dependent policy.
12. Asset selection, generation, preparation state, and acceptance remain
    Rust-owned. TypeScript must not call a named asset-selection completion
    solely to wake Rust.
13. Rust supplies one readiness disposition. TypeScript may schedule another
    animation frame but may not reconstruct readiness from queue counts.
14. Shutdown and replacement have one Rust-owned quiescence barrier covering
    every issued operation. Physical Worker abort/termination remains
    best-effort adapter behavior.
15. Ordinary rendering and raw input remain available while independent
    standby work is in flight. Removing the global `sessionBusy` policy must
    not pause the active world.
16. The query-gated smoke observer remains an explicit semantic test client,
    but product operation execution must not depend on observer callbacks or
    named completion receipts.
17. Unsupported pre-release storage formats are rejected or reset; they are
    not migrated by production TypeScript.
18. Every slice reports the parent-topic scoreboard rows it moved and explains
   temporary growth. The completed tactical must be net-negative across
   authored TypeScript plus web-only Rust unless a separately reviewed
   behavioral gain changes that gate.

## Proposed Contract Shape

The exact names are reviewable, but the ownership should resemble:

```rust
struct SceneOperationCoordinator {
    // The existing session, lobby, catalog, and asset operation owners,
    // re-keyed onto PlatformOperationLedger identity — their private
    // token/epoch systems deleted, not delegated to.
}

enum SceneOperationRequest {
    StartRuntime(OwnedRuntimeStart),
    ExecuteCatalog(OwnedCatalogExecution),
    PrepareAssets(OwnedAssetPreparation),
}

struct SceneOperationCompletion {
    token: PlatformOperationToken,
    result: Result<OwnedPlatformResult, PlatformOperationError>,
}
```

This sketch does **not** mean TypeScript receives `SceneOperationRequest` or
its variants. Platform Rust consumes the semantic request:

- native Rust executes it directly or submits typed work to the existing
  runtime/catalog/render facilities;
- browser Rust lowers it to a capability-specific mechanical effect; and
- TypeScript executes that effect and returns its opaque token/result.

The browser ABI should prefer generated wasm-bindgen classes or opaque owned
frames over `Record<string, any>` flags. A small browser pump may drain
several independent effects, but Rust determines concurrency and backpressure
before they are issued.

## IndexedDB Addressing Direction

The current browser executors contain two mclone-specific addressing schemes:

- catalog string labels mapped to physical stores and indexes; and
- numeric persistence namespaces mapped to store/key/value layouts.

That is more engine awareness than a reusable IndexedDB executor needs. The
recommended direction is:

1. browser Rust owns the mapping from shared domain address to a physical web
   storage plan;
2. TypeScript receives physical store/index names, keys, transaction modes,
   and opaque values directly in a mechanical plan;
3. a Rust-authored physical schema descriptor is available before opening the
   database so `onupgradeneeded` can create stores synchronously; and
4. TypeScript contains no switches over `dimension-chunks`, players, managed
   worlds, or numeric namespace meanings.

This lowering is gated on net deletion: land it only if the combined
TypeScript-plus-browser-Rust line count decreases. Do not merge catalog and
opened-world continuations merely to make their envelopes look alike. They
may share the last-mile IndexedDB action runner while retaining separate Rust
owners.

## Implementation Slices

Slices are ordered by leverage: the ABI reshape first because four methods
cause most of the glue and the only user-visible defect; identity
consolidation second because it is the large Rust deletion; convergence and
addressing after a decision gate re-scopes them against what has already
evaporated.

### Slice 0: Exact traces, costs, and deletion ledger

Status: complete 2026-07-21. The reproducible source scoreboard, identifier
classification, non-increasing debt test, browser borrow/progress traces, and
native rendered baseline are recorded above. Pixel-capture and full-lifecycle
failures remain explicitly open for cutover validation.

- Capture one native and browser trace for active local start, remote start,
  lobby primary/destination warmup, catalog create/open/delete, asset
  replacement, initial presentation, replacement, and shutdown.
- Reuse existing smoke/lifecycle scenarios and prefer composite traces; do not
  create a new cross-platform test matrix solely for this baseline.
- Record which calls currently hold `&mut WebSceneHost` across an await,
  how long each borrow excludes rAF/input in practice, and which work can
  proceed concurrently with active rendering.
- Measure ordinary frame overhead, Worker count, startup time, lobby warmup,
  catalog latency, and asset replacement before changing the ABI.
- Record the parent-topic scoreboard baseline: authored TypeScript lines,
  `mclone-web-client/src` lines, `WebSceneHost` export count, async-borrow
  export count, identity-system count, and wasm cfg counts.
- Classify every request id, token, epoch, and generation named by the traced
  operations as either boundary-operation identity or durable domain/content
  version. Record the invariant for every version that should survive.
- Add source-inventory locks with current non-increasing ceilings and explicit
  zero cutover targets for every named product-TypeScript field, report flag,
  dispatch method, Rust stale counter, and parallel operation identity. Lower
  each ceiling as its owner is deleted; Phase 0 must still pass on the current
  baseline.

Exit: the refactor has exact behavioral and deletion evidence on both sides
of the boundary rather than a TypeScript-line goal.

### Slice 1: Retire the async-borrow ABI

Status: complete 2026-07-21. Owned Promise-backed runtime and asset tickets
are live; the source ceilings for all four async mutable exports,
`sessionBusy`, `waitForSessionIdle`, and the lobby-specific promise registry
are zero. The measured active-world progress and validation evidence are
recorded above.

- Use active-session start as the first end-to-end owned
  issue/effect/completion proof; do not introduce the final coordinator type
  merely to make that proof compile.
- Replace `startPendingSession`, `shutdownAsync`,
  `completeAssetPackSelection`, and `WebLobbyRuntimeStart::start` with
  synchronous issue, take-effect, and submit-completion turns; long-running
  work suspends in platform code holding owned values, never the scene
  borrow.
- Delete `sessionBusy`, `waitForSessionIdle`, the setTimeout retry loops, and
  the busy-mirroring into observer state — all ~20 guard sites.
- Preserve wasm-bindgen re-entry safety by construction: exported operations
  are synchronous, so no borrow can span an await.
- Prove with the Slice 0 trace that rAF and raw input continue during lobby
  warmup, asset replacement, and session start, and that shutdown still
  quiesces correctly.

Exit: zero exported `async fn(&mut self)` methods; ordinary input/render
calls are never excluded by a promise borrowing the whole scene host; the
active world no longer pauses during independent coarse operations.

### Slice 2: Consolidate operation identity onto the ledger

Status: complete 2026-07-21. Session, lobby, asset, and catalog completions
use opaque `PlatformOperationToken` values; the former currentness predicate,
stale counter, asset-generation completion identity, and catalog string ID
are deleted. Retained asset epochs are documented content generations.

- Migrate session-start currentness onto `PlatformOperationLedger` tokens and
  delete `external_scene_start_is_current` duplication and the hand-rolled
  `stale_lobby_start_completion_count`.
- Migrate asset-preparation request/completion acceptance onto the same tokens.
  Delete uses of replacement epochs as ad hoc platform-operation identity.
  Retain and clearly name asset/content generations where they prove that
  prepared assets, meshes, render workers, active assets, and warm worlds are
  compatible; `validate_replacement_epoch` may be narrowed to that invariant
  instead of being deleted by name.
- Delete the catalog `String` request-id; the ledger token is the only
  identity that crosses the boundary.
- Rationalize the warm-world `asset_epoch` slot tags against the unified
  identity where they duplicate it; keep them where they express the distinct
  content-generation invariant, with a comment stating which.
- Reuse `PlatformOperationService`/`PlatformOperationLedger` rules; do not
  create a second token system, and do not leave the old ones compiled in.
- Add shared tests for order, concurrency, cancellation, duplicate/unknown
  completion, replacement, failure restoration, and shutdown against the one
  identity system.

Exit: one identity/staleness vocabulary; the deleted-systems count on the
scoreboard reads 4 → 1; a platform adapter can take work and return
completion without learning why the scene requested it.

### Decision gate after Slice 2

Status: next. No Slice 3 implementation begins until the remaining pumps,
ticket types, exports, and combined line cost are freshly inventoried below.

Re-measure the remaining named TypeScript pumps and the per-operation ticket
types against the new ABI and identity system. If they have collapsed to
trivial forwarding, shrink or drop Slices 3–5 accordingly and record that in
this document. Decide separately whether storage-address lowering still has a
net-deletion case; it is optional and must not block operation convergence. Do
not execute the remaining slices merely because they were planned.

### Slice 3: Converge runtime startup

- Route active, lobby-primary, and lobby-destination starts through the same
  shared request lifecycle.
- Move native direct-start and browser ticket lowering behind platform Rust
  executors of that lifecycle.
- Preserve independent standby warmup while the active world continues to
  render.
- Delete `WebLobbyRuntimeStart`, `takeLobbyRuntimeStart`,
  `completeLobbyWorldStart`, and the separate active-session start ABI once
  the shared completion path is live.

Exit: TypeScript can start opaque Worker machinery without knowing lobby or
active-session identity, and native/web share the acceptance state machine.

### Slice 4: Adopt catalog and asset operations; one browser drain loop

- Feed the existing Rust catalog continuation through the same outer
  operation pump while keeping its specialized transaction semantics.
- Remove `catalogRequest`, `catalogRequestId`, and the product app's catalog
  promise tail; make the IndexedDB executor own only transaction mechanics.
- Submit asset preparation through the existing render-actor mailbox and poll
  its typed completion from Rust; remove `assetPackRequest` and the named
  TypeScript `completeAssetPackSelection` wakeup.
- Implement the smallest capability-specific TypeScript executors and one
  generic drain/wakeup loop; delete `pendingLobbyRuntimeStarts`,
  `lobbyOperationDrainActive`, and `worldCatalogOperationTail`.
- Let independent effects run concurrently only when Rust admission permits.

Exit: catalog and asset changes require no named branch in product
TypeScript; the five dispatch branches are one generic loop.

### Slice 5: Readiness and shutdown

- Return one Rust-authored frame/readiness disposition and stop reading
  render queue counts for product control flow.
- Make shutdown poll one Rust quiescence barrier and let the browser adapter
  mechanically terminate or release the resources named by final effects.
- Preserve the integrated-server Worker's adjacent IndexedDB path and its
  world writer lease.

Exit: product TypeScript contains browser mechanics and operational error
capture but no mclone scene-operation readiness or quiescence policy.

### Optional Slice 5b: Storage addressing

- Reassess the IndexedDB addressing direction after the operation cut has
  landed; stable store/index mapping may be legitimate platform mechanics.
- Move catalog or record namespace-to-physical-address lowering into browser
  Rust only when the result removes semantic TypeScript vocabulary and makes
  the combined authored-TypeScript-plus-web-Rust boundary net-smaller.
- Do not merge catalog and opened-world continuation owners merely to share an
  envelope, and do not delay Slice 6 if this optional cut lacks a deletion
  case.

Exit: either the smaller domain-blind addressing runner lands with measured net
deletion, or the retained platform mapping is documented as the cheaper owner.

### Slice 6: Cutover validation, deletion closeout, and ledger report

- Delete superseded reports, exported methods, state fields, helpers, tests,
  and historical compatibility branches rather than retaining two paths.
- Run the shared scene/app-runtime/session/persistence/render tests.
- Run native desktop, flat Android, Android XR, desktop OpenXR, and offscreen
  compile/control gates affected by shared types.
- Run Wasm checks, generated bindings, authored TypeScript typecheck, Worker
  ownership, thin-adapter, scene-host adoption, and source locks.
- Run headed local, remote, mobile, lobby, catalog, asset replacement,
  IndexedDB reload, lifecycle, and shutdown browser smokes and inspect their
  screenshots.
- Compare startup, steady-frame, lobby warmup, Worker count, memory/copy, and
  shutdown measurements with Slice 0.
- Demonstrate the fixpoint: add a trivial test-only coarse operation variant
  and show it requires zero TypeScript changes and zero new exports.
- Refresh the operation, host-boundary, persistence, platform-parity, and
  tactical records, and append the pass-ledger row with full scoreboard
  deltas and an honest "what remains" to
  [`platform-boundary-convergence.md`](../topics/platform-boundary-convergence.md).
  This tactical does not declare the parent concern complete.

## Acceptance Criteria

Rust-side (primary):

1. Exactly one boundary-operation token/staleness family remains; the session
   currentness duplication, hand-rolled stale counter, use of asset generation
   as platform-operation identity, and catalog `String` request-id are deleted.
   Any retained asset/content generation has a documented resource-
   compatibility invariant and is not used to identify a platform completion.
2. Zero exported `async fn(&mut self)` methods remain on `WebSceneHost` or
   its sibling exported classes.
3. The `WebSceneHost` export count is materially reduced from 48 and the
   final count is reported.
4. `mclone-web-client/src` ends net-smaller than its 20,577-line baseline,
   and the combined authored-TypeScript-plus-web-Rust total is net-negative
   for the tactical.
5. Adding a test-only coarse operation type that uses the established
   mechanical capability vocabulary requires zero TypeScript changes and zero
   new `WebSceneHost` exports, demonstrated in Slice 6.
6. Native hot paths gain no browser serialization, promises, SAB envelopes,
   or unjustified allocations.
7. Native and web execute the lifecycle through platform adapters without
   duplicating role, retry, acceptance, or completion decisions.

Behavioral:

8. Active rendering and input continue while permitted standby work is in
   flight; the active world never pauses for an independent coarse
   operation, proven by trace against the Slice 0 baseline.
9. Shutdown, replacement, cancellation, late completion, duplicate
   completion, and Worker failure are deterministic shared tests.
10. Opened-world persistence remains Worker-local and does not gain a
    browser main-thread round trip.

TypeScript-side (corollary):

11. Product TypeScript contains no lobby-specific runtime type,
    take/start/complete loop, pending set, or completion receipt.
12. Product TypeScript does not inspect `sessionStartPending`,
    `catalogRequest`, `catalogRequestId`, `assetPackRequest`, or
    `renderWorkerPendingRequestCount` for control flow.
13. Product TypeScript has no `sessionBusy`, `pendingLobbyRuntimeStarts`,
    `lobbyOperationDrainActive`, or `worldCatalogOperationTail` state.
14. Catalog meaning remains in its Rust continuation; TypeScript executes
    only mechanical IndexedDB actions. Any retained physical store/index
    mapping is classified as platform mechanics and contains no catalog flow,
    record-family policy, or dimension-specific compatibility decision.
15. Asset selection and preparation progress entirely through Rust-owned
    scene and render-actor state.
16. Rust alone decides initial presentation readiness; TypeScript schedules
    rAF according to a bounded mechanical disposition.
17. No production TypeScript contains `minecraft:overworld`, legacy chunk
    store labels, or runtime record migration code.

Process:

18. All affected product lanes and headed browser pixel gates pass.
19. The parent-topic pass ledger receives a row with before/after scoreboard
    values and an explicit "what remains."

## Questions For Independent Review

1. Should the scene facade extend `PlatformOperationService` directly, or use
   a small scene-specific enum over several existing services — and in either
   case, what proves the old token systems are deleted rather than wrapped?
2. Can active and standby runtime starts share one owned ticket type without
   retaining wgpu resources or coupling native to web construction details?
3. What is the smallest set of capability queues needed for Worker, IndexedDB,
   fetch, and scheduling mechanics while keeping domain operation variants out
   of TypeScript?
4. Does any operation truly require an async mutable scene borrow, or can all
   four current cases (session start, shutdown, asset selection, lobby
   warmup) become synchronous issue plus later completion?
5. Can asset preparation be completed entirely by polling the existing render
   coordinator, eliminating a browser effect altogether?
6. Should browser Rust emit direct physical IndexedDB names or opaque store
   IDs accompanied by one schema descriptor?
7. What is the smallest generic observer hook that removes named
   lobby/catalog/asset completion callbacks from the product driver without
   weakening integration tests?
8. Which current native `cfg(not(wasm32))` scene branches are semantic forks
   that should disappear, and which are legitimate direct effect adapters?
9. Which warm-world `asset_epoch` slot tags express a real invariant distinct
   from the unified identity, and which are duplication?

## Stop Conditions

Stop for renewed review if:

- the proposed coordinator becomes a universal actor for persistence,
  rendering, sockets, compute, input, and unrelated workloads;
- the consolidation turns into a facade: the old identity systems remain
  compiled in behind delegation rather than being deleted;
- a slice completes with a combined both-language line increase and no
  measured behavioral gain;
- native must serialize or allocate browser-shaped requests on a frame hot
  path;
- the design pauses active rendering while an independent standby operation
  is in flight;
- browser storage records or semantic operation variants must enter
  TypeScript to make progress;
- a physical IndexedDB change would silently preserve, migrate, or destroy a
  format not already declared disposable; or
- the cutover would retain old and new operation paths indefinitely.

## Non-Goals

- Restoring managed lobby installation, versioning, publication, or repair.
- One physical Worker or one byte ABI for every actor.
- Moving all DOM, rAF, Worker, IndexedDB, WebSocket, fetch, or promise calls
  into Rust merely to reduce TypeScript line count.
- Forcing XR pose/input or presentation sequencing through this coarse
  operation path.
- Changing lobby product behavior, world-generation behavior, asset content,
  or catalog UI semantics.
- Preserving unsupported pre-release worlds.
- Declaring the parent `platform-boundary-convergence` concern complete; only
  its standalone audit protocol may do that.
