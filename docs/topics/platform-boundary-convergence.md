# Platform Boundary Convergence

Topic: `platform-boundary-convergence`

Status: open master tracker. Tactical
[`207`](../tactical/207-shared-scene-operation-coordinator.md) completed
Phases 0–6 on 2026-07-21. The Phase 7 independent fixpoint audit
([`211`](../tactical/211-platform-boundary-fixpoint-audit.md)) completed the
same day and appended a bounded remaining-work backlog. Phase 8, Tactical
[`212`](../tactical/212-boundary-audit-cleanup-backlog.md), completed its
mandatory Slices 0–6 on 2026-07-21 and deliberately skipped its optional
hygiene slice. The current pickup is the independent Phase 9 second fixpoint
audit, chartered as Tactical
[`213`](../tactical/213-platform-boundary-second-fixpoint-audit.md). This parent is
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
  the completed execution record for the operation-lifecycle workstream.
- Tactical [`211`](../tactical/211-platform-boundary-fixpoint-audit.md) is
  the completed Phase 7 audit record; its findings, verdicts, and evidence
  are authoritative over any implementing tactical's closing prose.
- Tactical [`212`](../tactical/212-boundary-audit-cleanup-backlog.md) is the
  completed execution record for the Phase 8 remediation workstream.
- Tactical [`213`](../tactical/213-platform-boundary-second-fixpoint-audit.md)
  is the prepared Phase 9 audit handoff. Its implementation-session authorship
  is not audit evidence; an independent reviewer must perform and record it.
- The child topic documents own their durable subsystem contracts and evidence.
- A standalone audit tactical, opened only after implementation stops, is
  the only document allowed to close this parent; the Phase 9 audit is the
  next such opportunity.

When resuming work, read the current phase row and its exit gate here, then use
the corresponding Tactical 212 slice for code-level details. Do not restart
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
| 207 shared scene operation coordinator | implementation through Gate A on 2026-07-21 | — | borrow-free ABI and one token family landed; one opaque operation drain remains before tactical closeout |
| 207 implementation closeout | 2026-07-21 | tactical complete; borrow-free ABI, one token family, and one opaque operation drain; combined boundary 24,334 → 24,173 | parent remains open for independent audit; unchanged actor-ID/age lifecycle fixture remains separately recorded baseline debt |
| 211 independent fixpoint audit | 2026-07-21 | 207 claims verified by fresh reading (same shared coordinator on native and web, opaque drain real, zero async-borrow exports, one token family); parent held open | fixpoint proof is compile/source-lock only, export pin covers only `WebSceneHost`, ~800-line catalog-storage policy in web Rust, duplicated catalog apply loop with drift, dead web frame timing, smoke exports on the production ABI, rim in-flight guards; backlog chartered → 212 |
| 212 audit-remediation closeout | 2026-07-21 | mandatory backlog complete: catalog policy moved shared, apply/timing paths converged, production/smoke ABI split, rim guards deleted, behavioral wasm fixpoint passed; combined boundary 24,121 → 23,229 | parent held open for independent Phase 9 audit; optional mechanical hygiene skipped; unchanged actor-ID/age lifecycle fixture remains separate baseline debt |

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
| 0. Evidence and deletion ledger | **complete 2026-07-21** | Re-capture a clean-revision baseline, trace the native and browser operation lifecycles, measure frame/input exclusion, and pin non-increasing counts for named duplicate paths | Baseline `8a3e9b12`; repeatable scoreboard and monotonic debt test; measured catalog/asset exclusion and lobby/native controls recorded in Tactical 207. Existing web pixel-capture and lifecycle-smoke failures remain explicit cutover validation issues. |
| 1. Borrow-free browser ABI | **complete 2026-07-21** | Replace every exported async mutable borrow with synchronous issue/take and later completion submission over owned values | Four async mutable exports and the global busy policy are gone. Catalog start advanced frame/render/input +4/+4/+4 during its sample; asset preparation +3/+3/+3; lobby warmup +6/+6/+6. |
| 2. One boundary-operation token family | **complete 2026-07-21** | Re-key session, lobby, catalog, and asset-preparation completions onto `PlatformOperationService`/`PlatformOperationLedger` and delete parallel request identities | Identity systems are 4 → 1. Catalog uses an opaque token-bearing execution ticket; assets retain a documented content generation; shared ledger and scene ownership tests cover rejection and teardown. |
| Gate A. Fresh inventory and re-scope | **complete 2026-07-21** | Measure what the first three phases already deleted and choose the smallest remaining cut | Runtime starts were already converged; former Phases 3–5 were folded into one opaque-operation drain; optional storage lowering was skipped as net growth |
| 3. Opaque operation drain, readiness, and quiescence | **complete 2026-07-21** | Replace runtime/catalog/asset-specific browser pumps and host exports with one opaque Rust-authored take/complete lifecycle while retaining mechanical executors | One product TypeScript drain; named pumps, report wakeups, and readiness reconstruction deleted; exports 48 → 42; clean combined boundary 24,334 → 24,148 |
| 4–5. Former candidate phases | **folded into Phase 3 at Gate A** | Avoid artificial phases now that runtime startup already shares one ticket/completion lifecycle | Catalog/asset adoption, readiness, and quiescence exits are enforced by Phase 3 |
| 5b. Storage-address lowering | **skipped at Gate A** | Retain the small stable physical-name mapping at its lower-total-cost owner | Reopen only if future schema work demonstrates combined net deletion |
| 6. Implementation closeout | **complete 2026-07-21** | Delete old paths, validate every affected platform boundary, demonstrate extensibility, and report final deltas | Tactical 207 closed itself and appended its ledger row; headed Wayland pixel gates and native compile/render controls passed; the parent remains open |
| 7. Independent fixpoint audit | **complete 2026-07-21** ([Tactical 211](../tactical/211-platform-boundary-fixpoint-audit.md)) | Fresh code review by a reviewer/agent outside the implementation series | Outcome (b) of the protocol: precise remaining work appended (fixpoint-evidence gaps G1/G2, findings F1–F6, decision D1) and implementation reopened as Phase 8 |
| 8. Audit-remediation backlog | **complete 2026-07-21** ([Tactical 212](../tactical/212-boundary-audit-cleanup-backlog.md)) | Land the trailing 207 cleanup, fix the audit findings (catalog-plan hoist, apply-loop dedup, timing convergence, smoke-ABI split, rim-guard collapse), and upgrade the fixpoint to a behavioral demonstration with widened export pins | Mandatory Slices 0–6 closed with their gates; scoreboard column appended; combined boundary 24,121 → 23,229 |
| 9. Second fixpoint audit | **next; independent pickup required** ([Tactical 213](../tactical/213-platform-boundary-second-fixpoint-audit.md)) | Short re-audit against the closure protocol, first checking the behavioral fixpoint test and widened pins from 212 | Closure protocol passes and the parent closes under it, or precise remaining work is appended again |

### Phase Boundaries

- Phases 0–2 are committed scope. They address a measured browser defect and
  remove known duplicate identity machinery.
- Gate A folded candidate Phases 3–5 into one opaque browser-operation drain.
  It must reuse the specialized session, catalog, asset, render, and
  persistence owners rather than replacing them with one universal actor.
- Gate A skipped Phase 5b because the small stable store/index mapping is
  legitimate platform mechanics and moving it would grow the combined
  boundary. Reopen it only with a measured net-deletion case.
- Phase 7 was a separately opened audit workstream, not an implementation
  closeout; it ran as Tactical 211 and reopened implementation as Phase 8.
  Phase 9 repeats that shape after Tactical 212 closes.

## Scoreboard

Any pass claiming progress on this concern must report before/after values for
all rows. The 2026-07-21 values are the immutable campaign baseline. Phase 0
must also record a clean-revision start baseline because unrelated work may
have changed the live counts since the audit.

| Metric | Clean baseline | Phase 0 | Phase 1 | Phase 2 | Phase 3 | Closeout | Audit 211 | Remediation 212 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| authored web TypeScript lines | 3,757 | 3,759 | 3,685 | 3,686 | **3,574** | **3,599** | 3,555 | **3,593** |
| TypeScript gate lines / modules | 3,780 / 16 | 3,782 / 16 | 3,708 / 16 | 3,709 / 16 | **3,597 / 16** | **3,622 / 16** | — | **3,616 / 16** |
| `mclone-web-client/src` Rust lines | 20,577 | 20,577 | 20,702 | 20,695 | **20,574** | **20,574** | 20,566 | **19,636** |
| combined both-language boundary total | 24,334 | 24,336 | 24,387 (+53 cumulative) | 24,381 (+47 cumulative; -6 phase-local) | **24,148 (-186 cumulative; -233 phase-local)** | **24,173 (-161 cumulative; +25 closeout)** | 24,121 | **23,229 (-1,105 campaign; -892 from audit)** |
| shared `mclone-scene` Rust lines | 24,632 | 24,632 | 24,632 | 24,726 | 24,731 | 24,731 | 24,731 | **24,730** |
| shared `mclone-app-runtime` Rust lines | 33,877 | 33,877 | 33,877 | 33,877 | 33,877 | 33,877 | 33,877 | **35,111** |
| `WebSceneHost` exported methods | 48 | 48 | 48 | 48 | **42** | **42** | 42 | **37** |
| async mutable wasm exports | 4 | 4 | **0** | **0** | **0** | **0** | 0 | **0** |
| boundary-operation identity/staleness systems | 4 | 4, classified | 4 | **1** | **1** | **1** | 1 (+2 rim in-flight guards noted) | **1; rim guards 0** |
| wasm cfg forks (`mclone-scene` / `mclone-app-runtime`) | 71 / 110 | 71 / 110 | 71 / 110 | 71 / 110 | 71 / 110 | 71 / 110 | 71 / 110 | **70 / 98** |
| active world pauses during coarse web ops | yes | measured yes | **no borrow exclusion; measured progress** | no; progress traces retained | no; semantic probes retain progress | **no; headed Wayland traces pass** | no; 207 traces accepted | **no; replacement/rebuild traces pass** |
| TS coordination residue | ~300–360 lines + ~20 guard sites | pinned | global borrow guard deleted; named dispatch remains | 259 lines inventoried; one drain is Phase 3 | one generic physical-Promise registry and drain; named state machines zero | same product shape; +25 query-gated smoke-capture lines | drain confirmed opaque; two TS-authored domain schema tables remain (skipped 5b) | **same opaque product drain; smoke observation query-gated; schema-table skip unchanged** |

Audit-column notes: measured on a tree that included Tactical 207's
then-uncommitted trailing cleanup (legacy IndexedDB migration deletion,
−44 TS / −8 web-Rust lines), which subsequently landed as `5bf2ffa2`;
the orphan-store decision it left open is Tactical 212 Slice 0. The
cfg-fork metric
counts exactly `cfg(not(target_arch = "wasm32"))` occurrences; the full
fork-site census including positive `wasm32` gates and `cfg_attr` is
95 / 133. Measurement commands are recorded in
[Tactical 212](../tactical/212-boundary-audit-cleanup-backlog.md).

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

Pick up
[Tactical 213](../tactical/213-platform-boundary-second-fixpoint-audit.md) in a
separate session and commit series, performed by a reviewer or agent that
implemented none of Tactical 212. Start with the passing wasm behavioral
fixpoint test and the widened exact export pins, then freshly check the rest of
the closure protocol. Only that audit may close this parent or append a new
bounded backlog.

Tactical 212 resolved the `WORLD_DB_VERSION` orphan-store decision without a
runtime migration because there are no web-world preservation consumers and
obsolete development profiles are disposable. It also closed the Phase 7
findings: catalog policy moved shared, the catalog apply loop and timing paths
converged, smoke ABI was separated, and both rim guards were deleted.

The full browser lifecycle aggregate still exposes the already reproduced
actor-ID/age persistence fixture from Tacticals 197 and 202. Tactical 207's
operation-specific cancellation, warmup replacement, resource rebuild,
quiescence, and shutdown checks pass; the unrelated actor assertion remains
unchanged and is not a hidden parent-boundary failure.

The optional storage-address move (skipped Slice 5b) remains out of this
workstream, and the audit recommended against new campaigns around cfg-fork
counts or web-Rust line mass; roughly 85% of the fork sites are legitimate
platform mechanics.
