# Platform Boundary Convergence

Topic: `platform-boundary-convergence`

Status: open. This topic is the parent record for the long-running campaign
toward an optimal shared/platform code split. It is deliberately **not
closeable by any implementing tactical**, including a tactical that completes
every slice it planned. See the closure protocol below.

Child topics owned by this concern:

- [`cross-platform-operation-execution.md`](cross-platform-operation-execution.md)
- [`platform-host-boundary.md`](platform-host-boundary.md)
- [`web-worker-runtime-ownership.md`](web-worker-runtime-ownership.md)
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md)
- [`unified-persistence-interface.md`](unified-persistence-interface.md)

## Top-Level Frame

> Every line of platform-facing code should live at its lowest-total-cost
> owner: engine policy in shared Rust, platform drivers in platform Rust,
> browser/OS mechanics in platform glue. The boundary is optimal when adding a
> new engine feature or coarse operation adds **zero** boundary code — no new
> glue branch, no new per-operation export, no new platform fork.

The campaign has historically been framed and measured as "reduce what
TypeScript knows." That framing is now known to be incomplete. The 2026-07-21
two-sided audit showed that TypeScript is thin residue and the remaining mass
is Rust-side: overlapping operation-identity systems, a large web-only
lowering layer, and a wasm-bindgen ABI shape that forces glue coordination
into existence. Progress must therefore be measured across **both languages
and all layers**, not by TypeScript line count alone.

## Why This Parent Topic Exists

Between 2026-06 and 2026-07-21 the same simplification target was attacked in
at least sixteen passes. Several of those passes explicitly declared the
direction finished, and each declaration was reopened — twice within one day.
No single pass was wrong to bound itself; the process failure was that each
closeout was a self-declaration at the end of its own work stream, measured
with a metric (TypeScript lines, keyword debt ledgers) that could not see the
remaining problem.

This topic exists to hold three things no single tactical holds:

1. the **pass ledger** — the honest history of attempts, declarations, and
   reopenings;
2. the **scoreboard** — the both-language metrics every pass must report; and
3. the **closure protocol** — the evidence bar that must be met before this
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
| 207 shared scene operation coordinator | proposed 2026-07-21; revised same day | — | reframed to two-sided accounting after the audit below |

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
- At least **four overlapping identity/staleness systems**: the generic
  `PlatformOperationLedger` token (covers catalog fully, lobby/web session
  partially), `ExternalSceneSessionStart` currentness plus a hand-rolled stale
  counter, the independent asset-replacement epoch, and a catalog `String`
  request-id carried alongside its own ledger token — plus `asset_epoch` tags
  threaded through warm-world slots. Three of the four predate the generic
  ledger; the consolidation onto it was started (catalog) and never finished.
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

## Scoreboard

Any pass claiming progress on this concern must report before/after values
for all rows. Baseline is the 2026-07-21 audit above.

| Metric | Baseline (2026-07-21) |
|---|---|
| authored web TypeScript lines | 3,757 (gate: 3,736/16 modules) |
| `mclone-web-client/src` Rust lines | 20,577 |
| combined both-language boundary total | trend must be net-negative per pass |
| `WebSceneHost` exported methods | 48 |
| `async fn(&mut self)` wasm exports | 3 + `WebLobbyRuntimeStart::start` |
| operation identity/staleness systems | 4 (+ warm-world `asset_epoch` tags) |
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
   - adding a new coarse operation type requires zero TypeScript changes and
     zero new `WebSceneHost` exports (demonstrated, not asserted);
   - zero `async fn(&mut self)` wasm exports remain;
   - exactly one operation identity/staleness system remains;
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

## Recommended Next Work

Tactical [`207`](../tactical/207-shared-scene-operation-coordinator.md)
(revised 2026-07-21) is the active program: retire the async-borrow ABI
first (small, ends the world-pausing defect), finish the identity
consolidation onto the existing ledger second (the large Rust simplification),
then collapse the per-operation lowering layer and evaluate against the
fixpoint. A decision gate after its Slice 2 re-scopes the remaining slices
against what has already evaporated.
