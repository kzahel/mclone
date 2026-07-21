# Tactical 211: Platform Boundary Fixpoint Audit

Status: audit complete 2026-07-21. Verdict: the parent concern remains
**open**. The Tactical 207 implementation claims verified under fresh code
reading — this was not another premature closeout — but the closure
protocol's fixpoint evidence is only partially met, and the audit found a
bounded remaining-work backlog. Per the parent's closure protocol, this
audit appends precise remaining work and hands implementation to
[`212-boundary-audit-cleanup-backlog.md`](212-boundary-audit-cleanup-backlog.md).
Reopening is the protocol's normal outcome, not a failure finding.

Topic: `platform-boundary-convergence`

Parent concern:
[`../topics/platform-boundary-convergence.md`](../topics/platform-boundary-convergence.md)
owns the campaign-wide pass ledger, the both-language scoreboard, and the
closure protocol. This tactical is the Phase 7 independent fixpoint audit
that protocol requires: performed in a separate session from the 197–207
implementation series, by an agent that implemented none of the audited
work, grounded in fresh code reading rather than keyword ledgers or prior
docs.

## Audit Method

- Fresh reads of `native/apps/mclone-web-client/src/web_scene_host.rs`,
  `web_catalog_execution.rs`, `web_scene_protocol.rs`, `lib.rs`, every
  authored `.ts` under `native/apps/mclone-web-client/www/`,
  `native/crates/mclone-scene/src/session.rs`,
  `native/crates/mclone-app-runtime/src/platform_operation.rs`,
  `catalog_executor.rs`, and the native client's scene-host usage. Three
  independent readers covered (a) the web Rust boundary, (b) shared-crate
  cfg forks and coordinator sharing, (c) TypeScript residue and the
  fixpoint locks; their counts were cross-checked against direct
  measurement.
- Ran the lock and gate suites on the audited tree:
  `cargo test -p mclone-web-client --test realm_dimension_persistence_lock
  --test web_scene_async_boundary_lock` (7 passed),
  `pnpm --silent native:web:scene-adapters`,
  `pnpm --silent native:web:worker-ownership`,
  `pnpm --silent native:web:scene-host-adoption` (all passed).
- Re-derived scoreboard values with the commands recorded in Tactical
  212's measurement section.
- The audited tree contained Tactical 207's then-uncommitted trailing
  cleanup (legacy IndexedDB migration deletion plus headed-Wayland
  capture guidance). Its lock tests pass. The migration deletion has
  since landed as `5bf2ffa2` (`Topic: world-dimension-storage-layout`)
  without resolving decision D1 below; D1 remains 212 Slice 0.
- Line references below are as of this audit; re-locate by symbol name if
  they have drifted.

## Closure-Criteria Verdicts

The parent's closure protocol lists five evidence requirements.

1. **Test-only coarse operation with zero TS changes and zero new
   exports — PARTIAL.** The demonstration exists but is compile-time and
   source-lock only. `WebRuntimeStartEffect::TestOnlyRemote`
   (`web_scene_host.rs:209`, `#[cfg(test)]`) reuses the existing Runtime
   Promise vocabulary, and
   `tests/platform_boundary_convergence_debt.rs:143-155` asserts zero
   TypeScript references and an unchanged 42-export `WebSceneHost`
   surface. However, nothing ever constructs the variant and drives it
   through `takeSceneOperation -> start -> completeSceneOperation`, so
   "requires zero TypeScript changes" is proven by compilation plus grep,
   not by observing the generic drain service it. Additionally the export
   pin covers only `impl WebSceneHost`; a future operation needing a new
   export on `WebSceneOperation` or `WebCatalogExecution` would pass the
   `== 42` lock silently.
2. **Zero `async fn(&mut self)` wasm exports — PASS.** Verified by direct
   grep and by reading the single `#[wasm_bindgen]` impl block
   (`web_scene_host.rs:469`, spanning 470–1815). The four remaining
   `async fn(&mut self)` in the crate are private methods of
   `web_server_worker.rs` (667, 720, 727, 749), not exports.
3. **Exactly one boundary-operation token/staleness family — PASS, with
   two rim caveats.** Catalog flows through `WorldCatalogOperationService`
   (a `PlatformOperationService` specialization,
   `catalog_executor.rs:75`); asset-pack and session starts issue
   `PlatformOperationToken` from the shared host; acceptance gates compare
   tokens (`web_scene_host.rs:1933`). Asset epochs are documented
   content-generation compatibility, never operation identity. Caveats:
   two hand-rolled in-flight guards in the web rim
   (`catalog_operation_in_flight`, `web_scene_host.rs:463`;
   `asset_pack_preparation_in_flight`, `:445`) re-implement
   pending/stale/duplicate bookkeeping the ledger already models. They
   reuse the family token, so they are duplication, not a second family.
4. **Scoreboard fully and honestly reported — PASS, with metric
   clarifications.** The "71 / 110 cfg forks" metric counts exactly
   `cfg(not(target_arch = "wasm32"))` occurrences (reproduced). The full
   fork-site census including positive `wasm32` gates and `cfg_attr` is
   95 / 133; both are dominated by legitimate platform mechanics (census
   below). The literal "zero domain-specific TypeScript" phrasing is not
   met: two domain schema tables remain TS-authored (the IndexedDB world
   schema in `mclone-web-world-catalog.ts:1-10, 229-245, 260-337` and
   `NAMESPACE_SPECS` in `mclone-web-persistence-executor.ts:83-97`).
   This is the consciously skipped Slice 5b of Tactical 207 — a recorded
   re-scope, not a hidden regression — but
   `realm_dimension_persistence_lock.rs` pins that leak in place rather
   than bounding it.
5. **Active-world rendering/input never excluded, with measured trace —
   ACCEPTED FROM PRIOR EVIDENCE.** Tactical 207 Slices 1/3/6 recorded
   frame/render/input progress counters advancing during catalog, asset,
   and lobby operations, re-validated under headed Wayland at closeout.
   The audit re-read the mechanism (no borrow-holding exports remain; the
   drain issues owned values) and accepts the recorded traces rather than
   re-measuring.

## Verified Positives Worth Recording

- Native and web drive the **same** shared coordinator. The desktop app
  aliases `McloneSceneHost` directly and contains no references to
  catalog-operation or external-session machinery of its own; platform
  variation is injected (`WorldCatalogOperationService::immediate` native,
  `::deferred` web), not forked. No parallel native coordinator exists.
- The TypeScript drain is genuinely opaque. The only branch in
  `executeSceneOperation` (`mclone-web-app.ts:379`) selects between two
  mechanical capabilities (IndexedDB action plan vs Promise), not domain
  kinds; runtime-start and asset-preparation are indistinguishable to
  TypeScript.
- `sessionBusy` is fully gone (zero hits under `www/`), and no campaign
  TODO/FIXME/shim markers remain in the audited files.
- The web-only Rust mass (~20.6k lines) is mostly the legitimate far side
  of the boundary (Worker-hosted server, IndexedDB executor, canvas/rAF
  driver, wasm ABI), with the exceptions listed as findings below.

## Findings (Remaining Work)

Ranked; implementation slices live in Tactical 212.

- **G1 — Behavioral fixpoint gap.** The closure protocol says
  "demonstrated, not asserted"; the current demonstration is
  compile/source-lock only (criterion 1 above). A wasm test must drive
  `TestOnlyRemote` through the full take/start/complete lifecycle.
- **G2 — Export-pin scope gap.** The `== 42` lock covers only
  `WebSceneHost`. Pins must extend to `WebSceneOperation`,
  `WebCatalogExecution`, and any future exported boundary type.
- **F1 — Storage-schema policy in web-only Rust (~800 lines).**
  `CatalogExecutionCore` and the storage-plan types
  (`web_catalog_execution.rs:24-868`) encode transaction ordering,
  read/write separation, duplicate-create rejection, active-world delete
  protection, and factory-reset ordering. That is policy, not browser
  mechanics; its owner is a shared crate, with only JsValue encode/decode
  remaining web-side.
- **F2 — Duplicated catalog apply loop with live drift.** The wasm branch
  of `poll_external_catalog_operations`
  (`mclone-scene/src/session.rs:5754-5790`) re-implements the native
  `apply_xr_catalog_effects` loop; the copies already differ
  (`commit_render_state` present in one, absent in the other). Only the
  per-start scene construction genuinely forks.
- **F3 — Web frame-timing accounting is silently dead.**
  `timing_start`/`timing_elapsed_ms`
  (`mclone-app-runtime/src/lib.rs:2981-3005`) return `None`/`0.0` on
  wasm, and `client_connection.rs:303-330` / `frame_render.rs:33-57`
  hand-roll the same Instant-vs-Date shim that `monotonic.rs` already
  abstracts and the web client already injects. Converging on
  `MonotonicClock` deletes an entire divergence category and fixes the
  measurement gap.
- **F4 — Test scaffolding on the production ABI.** Five `WebSceneHost`
  exports are smoke-only (`rebuildRenderResourcesForSmoke`,
  `renderHalfSpaceTerrainProof`, `renderPreparedFigureProof`,
  `renderActorCompositionProof`, `beginLobbySmokeWithChunkSpan`,
  `web_scene_host.rs:476-841, 1779-1793`; the last also bypasses the
  drain), and `WebCatalogExecution` carries a smoke-only constructor that
  forces an `Option` token and a dead-in-production error branch
  (`web_catalog_execution.rs:884, 900-903, 1007, 1018-1062`).
- **F5 — Rim duplication of ledger state.** The two in-flight guards from
  criterion 3, plus the vestigial `render_resource_generation` counter
  (`web_scene_host.rs:464`, written only by the smoke rebuild).
- **F6 — Minor policy and hygiene.** `lower_runtime_start` embeds
  descriptor-to-runner-config policy web-side
  (`web_scene_host.rs:1817-1871`); `session.rs` is 6,606 lines with one
  ~174-method impl block; small TS dead code and duplication
  (`finiteInteger`, dead `mclone-render-compiler-shared.ts` exports,
  six `stringifyError` copies, duplicated IndexedDB promise wrappers).
- **D1 — Pending decision from the uncommitted trailing cleanup.** The
  legacy-store migration was deleted without bumping
  `WORLD_DB_VERSION` past 6, so existing dev-browser databases keep
  orphaned `chunks`/`entityChunks` stores and factory reset no longer
  clears them. Acceptable under the disposable-internal-worlds ledger,
  but it must be an explicit choice (bump to v7 with `deleteObjectStore`
  cleanup, or record acceptance).

## What The Audit Recommends Against

- No new campaign around cfg-fork counts or web-Rust line mass. Roughly
  85% of the fork sites are unavoidable mechanics: no threads on wasm,
  and in-process-server-plus-filesystem (native) vs
  Worker-server-plus-IndexedDB (web).
- Slice 5b storage-address lowering stays skipped; it remains net growth
  without a schema-work motivation.
- Retiring native's synchronous session-start pump onto the neutral
  external-operation seam (`session.rs:931/942, 1555, 5183`, the
  `SceneLocalStartup` ZST at `:152/:174`) is possible — the seam is
  already target-neutral and the immediate-vs-deferred service pattern is
  proven — but unmotivated by any defect. Record as optional future work
  requiring a concrete driver.
- The actor-ID/age persistence fixture remains separately recorded
  baseline debt owned outside this boundary concern; it was verified
  unchanged, not weakened.

## Scoreboard At Audit

Measured on the audited tree (includes the uncommitted trailing cleanup;
methods in Tactical 212):

| Metric | 207 closeout | At audit |
|---|---:|---:|
| authored web TypeScript lines | 3,599 | 3,555 |
| `mclone-web-client/src` Rust lines | 20,574 | 20,566 |
| combined both-language boundary | 24,173 | 24,121 |
| shared `mclone-scene` Rust lines | 24,731 | 24,731 |
| shared `mclone-app-runtime` Rust lines | 33,877 | 33,877 |
| `WebSceneHost` exported methods | 42 | 42 |
| async mutable wasm exports | 0 | 0 |
| boundary-operation identity families | 1 | 1 |
| `cfg(not(wasm32))` forks scene / app-runtime | 71 / 110 | 71 / 110 |
| full cfg fork-site census scene / app-runtime | — | 95 / 133 |

## Handoff

Implementation of G1/G2, F1–F6, and D1 is chartered in
[`212-boundary-audit-cleanup-backlog.md`](212-boundary-audit-cleanup-backlog.md).
After 212 closes, a second, short fixpoint audit (parent Phase 9) is the
only path that may close the parent topic, and it must re-verify the
behavioral fixpoint and the widened export pins introduced by 212.
