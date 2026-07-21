# Platform Boundary Convergence

Topic: `platform-boundary-convergence`

Status: open master tracker. No coordinator cutover has landed. Tactical
[`207`](../tactical/207-shared-scene-operation-coordinator.md) is the active
implementation workstream; its first pickup is evidence/baseline capture,
followed by retirement of the async mutable-borrow browser ABI. This parent is
deliberately **not closeable by an implementing tactical**, including one that
completes every phase it planned. See the closure protocol below.

Child topics owned by this concern:

- [`cross-platform-operation-execution.md`](cross-platform-operation-execution.md)
- [`platform-host-boundary.md`](platform-host-boundary.md)
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md)
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md)
- [`unified-persistence-interface.md`](unified-persistence-interface.md)

## Document Role

This is the master tracking document for the campaign, not the file from which
an implementing agent should attempt the whole refactor in one pass. It owns:

1. the target boundary and non-negotiable invariants;
2. the phased program, dependency gates, and current pickup point;
3. the append-only history and both-language scoreboard; and
4. the independent-audit closure bar.

The documents below it have narrower jobs:

- Tactical [`207`](../tactical/207-shared-scene-operation-coordinator.md) is
  the executable plan for the current operation-lifecycle workstream. Its
  phases should land as separately reviewable commit series. A phase may move
  to a child tactical if code reading reveals an independent risk or owner,
  but it does not need a new tactical merely because it has several commits.
- The child topic documents own their durable subsystem contracts and evidence.
- A later standalone audit tactical, opened only after implementation stops,
  is the only document allowed to close this parent.

When resuming work, read the current phase row and its exit gate here, then use
the corresponding Tactical 207 slice for code-level details. Do not restart
from the historical pass ledger.

## Top-Level Frame

> Every line of platform-facing code should live at its lowest-total-cost
> owner: engine policy in shared Rust, platform drivers in platform Rust,
> browser/OS mechanics in platform glue. The boundary is optimal when adding
> engine policy or a coarse operation expressible with existing platform
> capabilities adds **zero** boundary code — no new glue branch, no new
> per-operation export, no new platform fork. A genuinely new browser/OS
> capability still justifies a new mechanical adapter.

The campaign has historically been framed and measured as "reduce what
TypeScript knows." That framing is now known to be incomplete. The 2026-07-21
two-sided audit showed that TypeScript is thin residue and the remaining mass
is Rust-side: overlapping operation-identity systems, a large web-only
lowering layer, and a wasm-bindgen ABI shape that forces glue coordination
into existence. Progress must therefore be measured across **both languages
and all layers**, not by TypeScript line count alone.

## Plan Review Decision

The direction is sound, with five clarifications that keep the implementation
from becoming another relocation pass:

1. **Fix the borrow shape before designing the final coordinator.** The
   `async fn(&mut WebSceneHost)` ABI causes the only known user-visible defect
   in this concern and forces `sessionBusy` throughout otherwise ordinary
   browser code. An owned issue/effect/completion turn is the first vertical
   proof.
2. **Unify boundary-operation identity, not every generation number.** One
   `PlatformOperationToken` family should identify issued work and accept or
   reject its completion. An asset/content epoch may remain where it proves
   that meshes, render workers, active assets, and warm worlds use the same
   content generation. It must not also serve as an ad hoc operation token.
3. **Do not pre-commit to a universal queue or god coordinator.** Shared Rust
   owns semantic admission and completion. Browser mechanics may still use
   separate Worker, IndexedDB, fetch, or scheduling executors when their
   lifetimes differ. The required convergence is zero domain-specific
   TypeScript scheduling, not one physical queue.
4. **Measure cumulative simplification, not line deletion in every commit.**
   Each phase reports both-language deltas and explains temporary growth. The
   completed Tactical 207 workstream must be net-negative across authored
   TypeScript plus web-only Rust unless a separately reviewed behavioral gain
   changes that gate.
5. **Re-plan after identity consolidation.** Phases after the mandatory
   decision gate are candidates, not work to perform because it was listed.
   Storage-address lowering in particular is adjacent and optional; it must
   not delay the borrow, identity, runtime-start, readiness, or shutdown fixes.

## Why This Parent Topic Exists

Between 2026-06 and 2026-07-21 the same simplification target was attacked in
at least sixteen passes. Several of those passes explicitly declared the
direction finished, and each declaration was reopened — twice within one day.
No single pass was wrong to bound itself; the process failure was that each
closeout was a self-declaration at the end of its own work stream, measured
with a metric (TypeScript lines, keyword debt ledgers) that could not see the
remaining problem.

This topic exists to hold four things no single tactical holds:

1. the **pass ledger** — the honest history of attempts, declarations, and
   reopenings;
2. the **scoreboard** — the both-language metrics every pass must report; and
3. the **program roadmap** — the ordered phases and explicit current pickup;
   and
4. the **closure protocol** — the evidence bar that must be met before this
   concern may ever be declared done.

## Pass Ledger

Append-only. Every tactical that claims to advance this concern must add a
row when it closes, including what it deliberately left open.

| Pass | Date | Declared at close | What reopened or remained |
|---|---|---|---|
| 070 web-glue typing/ABI hardening | 2026-06 | boundary "already well-drawn"; SAB ABI locked | TS graduation punted to 071 |
| 071 TypeScript glue graduation | 2026-06-23 | authored `.ts` inventory established | smoke JS retained |
| 072 native-UI DOM retirement | 2026-06 | duplicate DOM/CSS UI deleted | plumbing/input/worker glue remained |
| 073 legacy TS engine deletion | 2026-06-24 | entire legacy TS engine removed | native web glue remained — every later pass chips at it |
| 143 client-experience convergence | 2026-07-05 | TS catalog demotion | fed 154/170 |
| 154 client-ingress adapter cleanup | 2026-07-07 | legacy request/response helpers quarantined | native remote wrappers platform-local |
| 170 web scene-host adoption | 2026-07-11 | one scene-policy owner for browser | reason-bearing feature ledger seeded the 197 campaign |
| 197 domain-blind worker broker | 2026-07-19 | TS 7,417 → 5,778; "no automatic continuation recommended" | 198 followed within a day |
| 198 opaque socket/IndexedDB adapters | 2026-07-19 | TS → 5,686 | catalog store-label vocabulary retained |
| 199 unified persistence interface | 2026-07-20 | one typed Rust persistence port | TS **rose** +187 while deleting meaning — first proof the TS-line metric misleads |
| 201 lobby content simplification | 2026-07-20 | accidental installer deleted (−1.7k app-runtime) | ownership reassessment handed to 202 |
| 202 web scene async boundary cleanup | 2026-07-20 | TS → 5,275; "no replacement actor justified" | reopened **next day** by strict audit → 207 |
| 203 shared interactive router (native) | 2026-07-21 | desktop/Android final dispatch unified | browser adoption deferred to 204 |
| 204 browser raw-input adoption | 2026-07-21 | TS input semantics deleted | diagnostics/preferences deferred to 205/206 |
| 205 diagnostic observer isolation | 2026-07-21 | app.ts 2,263 → 1,261; observer split out | preferences/bootstrap deferred to 206 |
| 206 preferences/bootstrap policy | 2026-07-21 | app.ts → 1,057; host-boundary series closed | coarse-operation pumps remained → 207 |
| 207 shared scene operation coordinator | reviewed 2026-07-21; Phase 0 next | — | reframed to two-sided accounting and phased execution after the audit below |

## Measured State (2026-07-21 Audit)

Method: `git show` line counts at `f134b554` (2026-07-18, pre-197) and
`12a4b16f` (2026-07-21), authored `.ts` excluding `.d.ts`, crate `src/*.rs`.

The campaign week traded lines like this:

| Layer | 07-18 | 07-21 | Delta |
|---|---:|---:|---:|
| authored web TypeScript | 7,394 | 3,757 | **−3,637** |
| web-only Rust (`mclone-web-client/src`) | 14,979 | 20,577 | **+5,598** |
| shared `mclone-app-runtime/src` | 35,581 | 33,877 | −1,704 |
| shared `mclone-scene/src` | 24,417 | 24,632 | +215 |
| shared `mclone-input/src` | 2,978 | 2,978 | 0 |

Roughly line-neutral overall — but the policy that was supposed to converge
into shared Rust largely landed in **web-only Rust**, which is now larger than
the TypeScript it replaced. Part of that is legitimate (Worker-resident actor
shells are the web counterparts of native threads; shared routing/preferences
fixed real cross-platform drift). The waste is concentrated in the
per-operation ticket-lowering machinery and overlapping identity systems:

- `WebSceneHost` exports **48 methods** in one `wasm_bindgen` block; **3 are
  `async fn(&mut self)`** (`startPendingSession`, `shutdownAsync`,
  `completeAssetPackSelection`) plus `WebLobbyRuntimeStart::start`. Those four
  borrows are the entire reason the TypeScript `sessionBusy` mutex, spin
  loops, and take/apply export contortions exist. `sessionBusy` dates to
  2026-06-24 — the first day of shared web UI — and every pass since has
  preserved and worked around it. While any of those borrows is live, rAF and
  raw input are excluded: coarse operations pause the active world on web
  only. Never measured; no native equivalent.
- At least **four overlapping operation identity/staleness systems**: the generic
  `PlatformOperationLedger` token (covers catalog fully, lobby/web session
  partially), `ExternalSceneSessionStart` currentness plus a hand-rolled stale
  counter, the independent asset-replacement epoch, and a catalog `String`
  request-id carried alongside its own ledger token — plus `asset_epoch` tags
  threaded through warm-world slots. Three of the four predate the generic
  ledger; the consolidation onto it was started (catalog) and never finished.
  The audit did not establish that every `asset_epoch` is redundant: several
  also express content-generation compatibility and require classification
  before deletion.
- `mclone-scene/src/session.rs` is 6,542 lines with ~60 wasm-related cfgs;
  the web lowering layer (`web_scene_host.rs` 4,295 +
  `web_catalog_execution.rs` 1,496 + `web_scene_protocol.rs` 242) is ~6,000
  lines.
- Remaining TypeScript coordination residue: ~300–360 dedicated lines in
  `mclone-web-app.ts` plus `sessionBusy` threaded through ~20 call sites.
  All of it mirrors Rust-owned state; it is derivative, not a source.

## Structural Findings — Why Passes Kept Recurring

1. **Bounded-by-design plus mandatory fresh review is a treadmill.** Each
   tactical stops at a boundary and hands reassessment forward; the fresh
   review always finds a seam. The process guarantees a next pass unless a
   terminal condition exists. This topic supplies that terminal condition.
2. **Keyword debt ledgers cannot prove cleanliness.** 202's code reading found
   four seams its 44-entry all-zero ledger could not express. An all-zero
   ledger is necessary, never sufficient.
3. **Aggregate coordination survives per-decision fixes.** Every individual
   decision became Rust-owned, yet the named pumps still formed a
   platform-specific scheduler, because the ABI shape — not the decisions —
   dictates the glue.
4. **A single-language metric hides cost-shifting.** TypeScript-only
   accounting let ~5.6k lines of web-only Rust accrue invisibly in one week.

## Program Roadmap

Tactical 207 is an umbrella implementation tactical. The rows below are the
review and pickup units. Only one phase should be active at a time, and every
phase closes with its own tests, measurements, scoreboard delta, and explicit
remaining-work note.

| Phase | Status | Purpose | Required exit |
|---|---|---|---|
| 0. Evidence and deletion ledger | **next** | Re-capture a clean-revision baseline, trace the native and browser operation lifecycles, measure frame/input exclusion, and pin non-increasing counts for named duplicate paths | Reproducible traces and counters exist for session start, lobby warmup, catalog, asset replacement, readiness, replacement, and shutdown; the debt ceiling can only move toward zero; no product behavior changes |
| 1. Borrow-free browser ABI | pending on Phase 0 | Replace every exported async mutable borrow with synchronous issue/take and later completion submission over owned values | Zero exported `async fn(&mut self)` methods; `sessionBusy`, idle spin loops, retries, and ordinary frame/input guards are deleted; trace proves the active world continues during independent work |
| 2. One boundary-operation token family | pending on Phase 1 | Re-key session, lobby, catalog, and asset-preparation completions onto `PlatformOperationService`/`PlatformOperationLedger` and delete parallel request identities | A boundary completion crosses with one operation token; duplicate/unknown/late results have shared tests; content epochs that remain have documented non-identity invariants |
| Gate A. Fresh inventory and re-scope | mandatory after Phase 2 | Measure what the first two phases already deleted and choose the smallest remaining cut | Tactical 207 is revised before more implementation; unnecessary later phases are dropped or narrowed |
| 3. Runtime-start convergence | candidate after Gate A | Give active, lobby-primary, and lobby-destination starts one shared logical request/completion lifecycle while preserving target/priority policy in Rust | TypeScript constructs only opaque Worker/runtime machinery; lobby and active-session start exports and acceptance paths no longer differ semantically |
| 4. Domain-blind effect driving | candidate after Phase 3 | Remove named catalog and asset branches from the product browser driver; retain distinct mechanical executors where browser APIs require them | No product TypeScript state machine or branch names lobby, catalog, asset selection, or scene target; Rust controls admission and concurrency |
| 5. Readiness and quiescence | candidate after Phase 4 | Return one Rust-authored presentation disposition and one shutdown/replacement quiescence contract | Product TypeScript does not reconstruct readiness from queue counts; shutdown deterministically drains or cancels all issued operations |
| 5b. Storage-address lowering | **optional sibling** after Gate A | Evaluate moving record-family/store addressing into browser Rust | Land only with net combined deletion and a smaller semantic surface; never block Phases 1–5 |
| 6. Implementation closeout | pending | Delete old paths, validate every affected platform boundary, demonstrate extensibility, and report final deltas | Tactical 207 closes itself and appends a ledger row, but leaves this parent open |
| 7. Independent fixpoint audit | future separate tactical | Fresh code review by a reviewer/agent outside the implementation series | Closure protocol below passes or the audit appends precise remaining work and reopens implementation |

### Phase Boundaries

- Phases 0–2 are committed scope. They address a measured browser defect and
  remove known duplicate identity machinery.
- Phases 3–5 are the likely continuation, but Gate A controls their final
  shape. They must reuse the specialized session, catalog, asset, render, and
  persistence owners rather than replacing them with one universal actor.
- Phase 5b is deliberately outside the critical path. Stable store/index
  mapping can be legitimate platform mechanics; move it only when doing so
  makes the combined boundary smaller and clearer.
- Phase 7 is not the last implementation commit or a closeout paragraph. It is
  a later, separately opened audit workstream.

## Scoreboard

Any pass claiming progress on this concern must report before/after values for
all rows. The 2026-07-21 values are the immutable campaign baseline. Phase 0
must also record a clean-revision start baseline because unrelated work may
have changed the live counts since the audit.

| Metric | Baseline (2026-07-21) |
|---|---|
| authored web TypeScript lines | 3,757 (gate: 3,736/16 modules) |
| `mclone-web-client/src` Rust lines | 20,577 |
| combined both-language boundary total | report every phase; Tactical 207 cumulative result must be net-negative unless a reviewed behavioral gain changes the gate |
| `WebSceneHost` exported methods | 48 |
| `async fn(&mut self)` wasm exports | 3 + `WebLobbyRuntimeStart::start` |
| boundary-operation identity/staleness systems | 4; separately classify warm-world/render `asset_epoch` generation tags |
| wasm cfg forks (`mclone-scene` / `mclone-app-runtime`) | 71 / 110 |
| active world pauses during coarse web ops | yes (`sessionBusy` since 2026-06-24) |
| TS coordination residue in `mclone-web-app.ts` | ~300–360 lines + ~20 guard sites |

## Closure Protocol

1. **Implementing tacticals close themselves, never this topic.** A tactical
   may declare its own slices complete. On close it must append a pass-ledger
   row with scoreboard deltas and an explicit "what remains" — "nothing" is a
   claim the tactical is not allowed to make about this topic.
2. **Only a standalone audit may close this topic.** The audit must be its own
   tactical, started in a separate session/commit series from the last
   implementing work stream, performed by a reviewer or agent that did not
   implement the work it audits, grounded in fresh code reading — not keyword
   ledgers or prior docs alone.
3. **Closure requires the fixpoint evidence, not judgment:**
   - adding a test-only coarse operation type that uses the established
     mechanical capability vocabulary requires zero TypeScript changes and
     zero new `WebSceneHost` exports (demonstrated, not asserted);
   - zero `async fn(&mut self)` wasm exports remain;
   - exactly one boundary-operation token/staleness family remains; retained
     domain generations are documented and are not used to identify platform
     completions;
   - the scoreboard is fully reported and the combined both-language trend
     over the campaign is documented honestly; and
   - active-world rendering and input are never excluded by an independent
     coarse operation, with a measured trace.
4. **Reopening is normal, not a failure.** A passing audit records "closed as
   of <date> under <scope>." Later evidence reopens the topic by appending to
   the ledger; it does not invalidate the audit.
5. **No victory declarations in prose.** Status changes to this topic happen
   only through ledger rows and audit tacticals. A closing paragraph in an
   implementing tactical carries no authority here.

## Immediate Next Workstream

Start Tactical [`207`](../tactical/207-shared-scene-operation-coordinator.md)
at Phase 0; do not begin by creating the final `SceneOperationCoordinator`
type.

The first implementation work packet is:

1. choose and record the clean baseline revision after the current unrelated
   worktree changes are resolved;
2. inventory every operation from Rust admission through platform effect and
   completion, classifying each identifier as either an operation token or a
   durable domain/content generation;
3. add trace evidence for frame and raw-input progress while active-session
   start, lobby warmup, asset preparation, and catalog work are pending;
4. add non-increasing inventory locks for `sessionBusy`, its retry/spin
   helpers, the duplicate request IDs, and the exported async mutable-borrow
   methods, recording zero as the cutover target without making Phase 0 tests
   fail on the current baseline; and
5. use active-session start as the first owned issue/completion vertical proof,
   then migrate the remaining async exports and delete the global busy policy
   in the same Phase 1 series.

Phase 0 should be evidence-only. Reuse existing smoke/lifecycle scenarios and
capture a small number of composite traces; do not create a new platform test
matrix merely to establish the baseline. Phase 1 ends at the borrow-free ABI
and the measured no-pause result; it must not grow into identity consolidation,
catalog-address redesign, or a generic executor. Phase 2 begins only from that
green boundary.
