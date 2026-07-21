# Tactical 213: Platform Boundary Second Fixpoint Audit

Status: ready for independent pickup; audit not started and no verdict
recorded. This file is a handoff prepared by the Tactical 212 implementation
session. It is not audit evidence and carries no authority to close the parent.

Topic: `platform-boundary-convergence`

Parent concern:
[`../topics/platform-boundary-convergence.md`](../topics/platform-boundary-convergence.md).
Audited implementation base: `6fc857af` (`Close boundary audit remediation
backlog`). The commit adding this handoff may contain documentation only; the
auditor must record the exact revision actually reviewed.

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
