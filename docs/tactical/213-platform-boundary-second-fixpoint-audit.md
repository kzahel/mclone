# Tactical 213: Platform Boundary Second Fixpoint Audit

Status: audit complete 2026-07-21; **pass**. The parent concern is closed as of
that date under the Phase 9 scope defined here.

Topic: `platform-boundary-convergence`

Parent concern:
[`../topics/platform-boundary-convergence.md`](../topics/platform-boundary-convergence.md).
Audited implementation base: `6fc857af` (`Close boundary audit remediation
backlog`). The commit adding this handoff may contain documentation only; the
auditor must record the exact revision actually reviewed.

Reviewed revision: `6b1881de` (`Prepare independent boundary fixpoint audit`).
Its only changes from `6fc857af` are the audit handoff and associated
documentation; the product code is exactly the Tactical 212 result.

## Independence Contract

The audit must run in a separate session and commit series, performed by a
reviewer or agent that implemented none of Tactical 212. The auditor must read
the current code afresh rather than accepting Tactical 212's prose, counts, or
verdicts. If that independence condition cannot be met, stop without changing
the parent status and leave this tactical ready for another reviewer.

No product decision or user input is expected. In particular, do not revive a
legacy browser-world migration: the product is unreleased, there are no known
web-world preservation consumers, and Tactical 212 Slice 0 deliberately
accepted disposal of obsolete development profiles under the compatibility
safety ledger.

## Audit Scope

Start with the closure-protocol evidence that was missing at Phase 7, then
recheck every criterion rather than reviewing only the remediation diff.

1. Run and read the behavioral fixpoint test. Confirm that
   `TestOnlyRemote` is created, taken through the same operation-type-blind
   drain used by production, starts through the ordinary Promise capability,
   completes through that drain, and folds through
   `PlatformOperationLedger`. Confirm it required zero TypeScript changes and
   zero product exports.
2. Freshly read these owners and their direct callers:
   - `native/apps/mclone-web-client/src/web_scene_host.rs`;
   - `native/apps/mclone-web-client/src/web_catalog_execution.rs`;
   - all authored TypeScript under
     `native/apps/mclone-web-client/www/`;
   - `native/crates/mclone-app-runtime/src/platform_operation.rs` and
     `catalog_storage_plan.rs`;
   - `native/crates/mclone-scene/src/session.rs`, including catalog effect
     application, asset replacement, and deferred operation polling.
3. Verify the exact exported-method pins remain 37 / 3 / 6 / 6 / 8 for
   `WebSceneHost`, `WebSceneOperation`, `WebCatalogExecution`,
   `WebSceneSmokeHarness`, and `WebCatalogSmokeExecution`. Verify zero
   exported `async fn(&mut self)` methods remain.
4. Verify there is exactly one boundary-operation token/staleness family.
   The deleted browser-local catalog and asset in-flight guards must not have
   returned. Content and render-resource generations may remain only as
   domain compatibility facts, never as platform-completion identity.
5. Re-derive the full parent scoreboard with Tactical 212's measurement
   commands. Report every row, including any drift from unrelated commits,
   and explain ownership moves instead of treating shared-crate growth as a
   regression by itself.
6. Observe an active-world coarse browser operation under the headed Wayland
   lane. Record frame, render, and input counter movement, operation outcome,
   and the inspected capture. Do not accept a headless Chrome black or
   transparent capture.
7. Run the boundary locks, thin-adapter checks, full workspace tests, wasm
   build, and the smallest native rendered control. Follow
   `docs/platforms.md`: Android and headset lanes are required only if fresh
   reading finds changes to their unique activity, package, OpenXR, graphics,
   or action mechanics.

## Reproduction Commands

Run from the repository root. If the pinned wasm-bindgen runner directory is
absent, build or install the workspace-pinned matching runner; do not silently
substitute a version that disagrees with the generated test artifacts.

```bash
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=\
native/target/wasm-bindgen-cli-0.2.125/bin/wasm-bindgen-test-runner \
cargo test --manifest-path native/Cargo.toml \
  --target wasm32-unknown-unknown -p mclone-web-client --lib \
  test_only_coarse_operation_runs_through_generic_drain

cargo test --manifest-path native/Cargo.toml \
  -p mclone-web-client --test platform_boundary_convergence_debt
cargo test --manifest-path native/Cargo.toml

pnpm native:web:typecheck
pnpm --silent native:web:scene-adapters
pnpm --silent native:web:worker-ownership
pnpm --silent native:web:scene-host-adoption
pnpm native:thin-adapters:purity
pnpm native:web:build

pnpm host:check
pnpm native:web:catalog-smoke
node native/apps/mclone-web-client/scripts/browser-smoke.mjs \
  --lobby-asset-replacement-probe
pnpm native:desktop-offscreen:smoke
```

Inspect every produced pixel artifact. The catalog and focused asset lanes are
the minimum browser semantic controls; add narrower lanes if fresh code
reading identifies another affected contract.

## Known Separate Baseline Debt

The complete lobby lifecycle aggregate currently reaches and passes its asset
replacement and resource-rebuild subcases, but its aggregate verdict remains
false because reopened actors receive new IDs and reset ages. That fixture was
already recorded by Tacticals 197, 202, and 207. Do not count it as a platform
boundary failure unless fresh evidence causally connects it to the audited
operation machinery. Likewise, do not fix it in this audit; append or route a
separate owner if its status has changed.

## Verdict and Closeout

The audit must choose exactly one evidence-backed outcome:

- **Pass:** append a Phase 9 ledger row and full scoreboard column to the
  parent, mark it "closed as of <date> under <scope>", update this tactical
  and its index row to audit complete, and commit the audit separately.
- **Reopen:** leave the parent open, append precise findings with bounded exit
  gates, charter only the necessary follow-up implementation, and update this
  tactical and its index row to audit complete with the reopening verdict.

Neither outcome is predetermined by this handoff. Do not use implementation
closeout prose as a substitute for the fresh evidence above.

## Independent Audit Result

**Verdict: pass.** This audit ran in a separate session by an agent that
implemented none of Tactical 212. The exact handoff revision was checked out
in a detached temporary worktree, remained source-clean throughout the audit,
and was read without relying on Tactical 212's conclusions. Ignored generated
first-party packs and the pinned Minecraft 1.17.1 extracted archive were
hydrated only as runtime prerequisites.

Fresh reading established all of the following:

- `TestOnlyRemote` is test-only, carries a token issued by
  `PlatformOperationLedger`, is wrapped as an ordinary runtime effect, passes
  through the same operation-type-blind `WebSceneOperationDrain` used by the
  product host, starts through `WebSceneOperation::start`, completes through
  that drain, and folds to `Failed` through the shared ledger. Commit
  `5b6fce7f`, which introduced the proof, changed no TypeScript and added no
  product ABI exports.
- the product TypeScript has one generic mechanical scene-operation drain.
  Query-gated smoke observation remains an explicit test client, and the
  skipped physical IndexedDB schema mapping remains platform mechanics.
- `PlatformOperationEpoch`, `PlatformOperationRequestId`, and
  `PlatformOperationToken` are the one boundary-operation identity/staleness
  family. Asset/content and render generations are compatibility facts only.
  The browser-local catalog and asset in-flight guards remain absent.
- catalog policy lives in `mclone-app-runtime`; browser Rust only lowers plans
  to IndexedDB mechanics. Catalog effects use one shared application loop.
  The only platform fork there constructs the platform-specific scene.
- Tactical 212 changed no Android activity/package mechanics and no OpenXR,
  XR graphics, or action mechanics. Per `docs/platforms.md`, Android and
  headset validation lanes were therefore not required for this audit.

The widened ABI lock passed with exact method counts of **37 / 3 / 6 / 6 /
8** for `WebSceneHost`, `WebSceneOperation`, `WebCatalogExecution`,
`WebSceneSmokeHarness`, and `WebCatalogSmokeExecution`. Source review and the
lock both found zero exported `async fn(&mut self)` methods.

## Re-derived Scoreboard

Measured from `6b1881de` using Tactical 212's commands:

| Metric | Audit 213 |
|---|---:|
| authored web TypeScript lines | 3,593 |
| TypeScript gate lines / modules | 3,616 / 16 |
| `mclone-web-client/src` Rust lines | 19,636 |
| combined both-language boundary total | 23,229 |
| shared `mclone-scene` Rust lines | 24,730 |
| shared `mclone-app-runtime` Rust lines | 35,111 |
| `WebSceneHost` exported methods | 37 |
| async mutable wasm exports | 0 |
| boundary-operation identity/staleness systems | 1; rim guards 0 |
| wasm cfg forks (`mclone-scene` / `mclone-app-runtime`) | 70 / 98 |
| active world pauses during coarse web ops | no; headed trace passed |
| TS coordination residue | one opaque product drain; query-gated smoke observer; unchanged schema-table skip |

There is no drift from Remediation 212 because the audited revision contains
that exact product tree. The combined boundary remains 1,105 lines below the
clean campaign baseline. Growth in `mclone-app-runtime` reflects catalog
policy moving to its shared owner rather than new boundary code.

## Behavioral and Render Evidence

The wasm behavioral fixpoint and all four focused boundary locks passed. In
the headed Wayland asset-replacement probe, the destination worker was held
during active lobby warmup. The active lobby advanced frame, render, and input
counters by **+2 / +2 / +2** with no scene-borrow exclusion. The operation
completed exactly once, advanced the asset epoch **0 -> 1**, selected the
authored pack, retired the reference pack, returned replacement state to
`active`, and preserved active world instance `2` with the
`protected-lobby` profile.

The fresh captures were inspected, not merely generated:

- `/tmp/mclone-native-web-catalog-ui-probe.png` and its canvas capture show
  the coherent world-list result after create/open/delete operations.
- `/tmp/mclone-native-web-lobby-lifecycle-asset-replacement.png` shows a live
  authored-only lobby frame with HUD and debug state; its fallback/checker
  surfaces are expected from the deliberately incomplete authored-only pack,
  not a black or transparent capture.
- the generic headed web page/canvas captures are non-black, opaque, and
  internally consistent.
- `/tmp/mclone-desktop-offscreen.png` shows the native control with textured
  terrain, foliage, cow, and chicken.

Validation passed:

```text
cargo test ... test_only_coarse_operation_runs_through_generic_drain
cargo test ... --test platform_boundary_convergence_debt
cargo test --manifest-path native/Cargo.toml
pnpm native:web:typecheck
pnpm --silent native:web:scene-adapters
pnpm --silent native:web:worker-ownership
pnpm --silent native:web:scene-host-adoption
pnpm native:thin-adapters:purity
pnpm native:web:build
pnpm host:check
pnpm native:web:catalog-smoke
node .../browser-smoke.mjs --lobby-asset-replacement-probe
pnpm native:desktop-offscreen:smoke
```

An initial workspace run executed concurrently with other heavy validation
and exposed the unrelated server-test publication race in
`sqlite_restart_restores_mclone_profile_before_unseen_generation`. The focused
test and the entire workspace passed when rerun sequentially. Fresh reading
found no causal connection to Tactical 212's boundary machinery, so it does
not reopen this concern. The previously recorded actor-ID/age lifecycle
fixture likewise remains separate baseline debt.
