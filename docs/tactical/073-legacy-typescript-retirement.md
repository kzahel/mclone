# 073: Legacy TypeScript Engine Retirement

Status: active high-priority parent. Slices 1-5 landed enough to remove the live legacy TypeScript engine; next recommended slice is documentation burn-down.

## Purpose

Delete the old TypeScript/browser engine, its tests, its browser app shell, its deploy path, and its tactical archive from the live tree, while preserving the native Rust engine, the native web/WASM glue, the Java oracle harness, and the oracle fixtures still used by native Rust tests.

The desired result is a repo that does not need defensive wording like "do not edit the legacy TypeScript engine" because the legacy engine is no longer present.

## Problem

The current tree still carries two different histories:

- the active native Rust engine under `native/`
- the legacy TypeScript engine under root-level `src/`, root browser/Vite entrypoints, Vitest tests, Playwright configs, Deno smoke scripts, and archived tacticals

That creates recurring cost:

- `README.md` and `AGENTS.md` spend attention routing agents away from old code.
- `package.json` exposes old commands next to native commands.
- root `index.html`, `smoke.html`, `vite.config.ts`, and `scripts/deploy-legacy.sh` still describe a deployable legacy browser app.
- `docs/tactical/legacy/` is large enough that search results routinely surface obsolete implementation plans.
- root `test/**/*.test.ts` and `test/browser/**` look authoritative even though native Rust tests are now the validation lane.
- `oracle/integration/*.ts` used to import helper modules from `src/oracle/**`, which blocked a clean `src/` deletion. Slice 1 moved those helpers to `oracle/lib/**`; the remaining `src/**` tree is now legacy engine code rather than oracle-owned tooling.

Git history is the archive. The live tree should only contain code, scripts, and docs that still shape the native engine or its oracle fixtures.

## Desired End State

- No root-level legacy engine source directory:
  - `src/` is gone.
- No root-level legacy browser app:
  - `index.html`, `smoke.html`, `vite.config.ts`, legacy Vite build scripts, and `scripts/deploy-legacy.sh` are gone.
- No legacy browser test surface:
  - root `playwright*.ts` configs are gone.
  - `test/browser/**` is gone.
  - root Playwright scripts are either deleted or explicitly retargeted to the native web app.
- No legacy Vitest suite:
  - `vitest.config.ts` and root `test/**/*.test.ts` are gone unless a specific oracle helper test survives in an oracle-owned location.
- No live legacy tactical archive:
  - `docs/tactical/legacy/**` is gone from the checkout.
  - durable information worth keeping is copied first into native tacticals, `docs/topics/**`, or oracle docs.
- Docs name the current architecture directly:
  - `README.md` describes native Rust, native web/WASM, reference MC source, and oracle tooling.
  - `AGENTS.md` routes directly to native Rust/native web and no longer contains legacy avoidance rules.
- Root `package.json` contains only commands that still make sense:
  - native Rust/native web commands
  - asset-pack commands
  - oracle generation commands
  - any small host capability or deployed-native-web smoke commands that are still actively useful
- TypeScript remains only where it is still part of the active path:
  - `native/apps/mclone-web-client/www/*.ts`
  - `native/apps/mclone-web-client/scripts/*.mjs`
  - `native/apps/mclone-web-client/tsconfig*.json`
  - optional oracle fixture tooling if it is clearly owned by `oracle/`, not by `src/`
- Native Rust tests either keep their existing oracle fixtures or use a renamed fixture root with all `include_str!` paths updated.

## Non-goals

- Do not delete native web/WASM TypeScript glue under `native/apps/mclone-web-client/`. That code is part of the Rust/WASM app path, not the legacy engine.
- Do not delete `test/fixtures/**` blindly. Native Rust crates currently include those JSON files directly.
- Do not delete Java oracle sources, decompile scripts, asset extraction scripts, the Cloudflare worker, or the native web deploy path.
- Do not use this cleanup to rewrite native engine behavior.
- Do not remove Playwright as a dependency merely because root Playwright configs go away; native web smokes still use `@playwright/test`.
- Do not remove TypeScript merely because root `src/` goes away; native web glue still uses `tsc`.

## Keep, Move, Delete

### Keep

These are active or likely active after retirement:

- `native/**`
- `native/apps/mclone-web-client/www/*.ts`
- `native/apps/mclone-web-client/www/*.js` when intentionally retained as ABI-lock or smoke glue
- `native/apps/mclone-web-client/scripts/**`
- `native/apps/mclone-web-client/tsconfig*.json`
- `oracle/java/**`
- `oracle/build.sh`
- `oracle/run.sh`
- `oracle/README.md`, after link/path updates
- `oracle/integration/run-server.sh`
- `oracle/integration/run-liquid-server.sh`
- `scripts/decompile-mc.sh`
- `scripts/fetch-server-jar.sh`
- `scripts/extract-assets.sh`
- `scripts/apply-parchment.py`
- `scripts/build-reference-asset-pack.py`
- `scripts/deploy-native-web.sh`
- `scripts/native-remote-client-smoke.mjs`
- `scripts/check-host-capabilities.mjs`, if still useful after browser-output wording is updated
- `worker/**`, because native deploy still uses the existing Cloudflare Worker
- `reference/**` as gitignored generated inputs

### Move Or Retarget Before Delete

These currently block a clean deletion or need an explicit keep/delete decision:

- `scripts/node-ts-loader.mjs`
- `test/fixtures/**`

Direction:

- Keep `scripts/node-ts-loader.mjs` only while oracle TypeScript CLIs still need it.
- Keep `test/fixtures/**` initially, then decide whether to leave it in place as the shared oracle fixture root or move it to `fixtures/oracle/**`.
- If fixture files move, update every native `include_str!` path in `mclone-worldgen`, `mclone-server`, and `mclone-mesh` in the same slice.

Cleared in Slice 1:

- `src/oracle/anvil/**` moved to `oracle/lib/anvil/**`.
- `src/oracle/integration/**` moved to `oracle/lib/integration/**`.
- the `src/util/bit-storage.ts` dependency was copied into `oracle/lib/util/bit-storage.ts`.
- `oracle/integration/*.ts` imports now target `oracle/lib/**`.

### Delete

Delete after the move/retarget blockers are cleared:

- `src/**`
- root legacy tests under `test/**/*.test.ts`
- root test support files that exist only for those tests
- `test/browser/**`
- root `playwright*.ts`
- `index.html`
- `smoke.html`
- `vite.config.ts`
- `vitest.config.ts`
- root `tsconfig.json`, unless it is replaced by an oracle-only config
- `scripts/deploy-legacy.sh`
- `scripts/dev-lan.mjs`
- legacy Deno smoke scripts that import `../src/**`
- legacy worldgen/perf observer scripts that import `../src/**`
- legacy root package scripts:
  - `test`, unless retargeted to native Rust validation
  - `test:watch`
  - `test:browser`
  - `test:browser:integration`
  - `probe:browser*`
  - `smoke:deno:*`
  - `dev:browser`
  - `dev:lan`
  - `host:node`
  - `host:dedicated`
  - `host:remote`
  - `bot:client`
  - `perf:worldgen`
  - `perf:worldgen:flyby`
  - `perf:worldgen:browser-flyby`
  - `observe:chunks`
  - `build`
  - `legacy-deploy`
  - `typecheck`, unless retargeted to native web typecheck
  - `perf:d5`
- dependencies/devDependencies that become unused after script removal:
  - `vite`
  - `vitest`
  - `unzipit`, if no retained oracle/asset script imports it
  - any root-only TS/browser package no longer referenced by native web or oracle tooling

## Implementation Slices

### Slice 0 - Inventory And Baseline

Goal: make the retirement measurable and reversible through ordinary Git history.

- [ ] Confirm workstream: native Rust/docs/repo cleanup.
- [ ] Record the current deletion candidates with `git ls-files`.
- [ ] Record every non-legacy path that imports or links to `src/**`, `test/browser/**`, `playwright*.ts`, `docs/tactical/legacy/**`, `vite`, and `vitest`.
- [ ] Record native fixture consumers:
  - `native/crates/mclone-worldgen`
  - `native/crates/mclone-server`
  - `native/crates/mclone-mesh`
- [ ] Run native baseline validation before deletion.
- [ ] Save any useful command output in the commit message or tactical landed notes, not in generated repo files.

Suggested inventory commands:

```bash
git ls-files 'src/**' 'test/**' 'docs/tactical/legacy/**' 'playwright*.ts' 'index.html' 'smoke.html' 'vite.config.ts' 'vitest.config.ts'
rg -n "src/|test/browser|playwright|docs/tactical/legacy|vite|vitest" README.md AGENTS.md docs package.json scripts oracle native
rg -n "test/fixtures|include_str!.*fixtures" native oracle docs
```

Validation:

```bash
cargo test --manifest-path native/Cargo.toml
pnpm native:web:build
pnpm native:web:smoke
```

### Slice 1 - Move Oracle TypeScript Out Of `src`

Status: landed on 2026-06-23.

Goal: make `src/` deletable without breaking official-server fixture generation.

- [x] Create an oracle-owned helper location, `oracle/lib/`.
- [x] Move the NBT/region/chunk/entity/liquid fixture helpers from `src/oracle/**` into that location.
- [x] Update `oracle/integration/*.ts` imports to use `oracle/lib/**`.
- [x] Update `oracle/README.md` references from `src/oracle/**` to the new location.
- [x] Keep the helper code shaped as tooling; do not let it depend on legacy engine runtime modules.
- [x] Inline or port the tiny non-oracle dependency into oracle tooling: `src/util/bit-storage.ts` became `oracle/lib/util/bit-storage.ts`.
- [x] Keep existing oracle helper tests pointed at the moved modules until the legacy Vitest suite is retired.

Validation:

```bash
./oracle/build.sh
./oracle/integration/gen-fixture.sh --seed 12345 --chunks 0,0 --out /tmp/mclone-oracle-fixture.json
./oracle/integration/gen-creature-fixture.sh --seed 12345 --scan --out /tmp/mclone-creature-scan.json
./oracle/integration/gen-liquid-fixture.sh --scenario test/fixtures/liquid-scenarios/water-slope.json --out /tmp/mclone-liquid-fixture.json
```

Landed validation:

```bash
pnpm exec vitest run test/oracle
node --disable-warning=ExperimentalWarning --experimental-transform-types --experimental-loader ./scripts/node-ts-loader.mjs --input-type=module -e 'await Promise.all([import("./oracle/lib/anvil/chunk.ts"), import("./oracle/lib/anvil/region.ts"), import("./oracle/lib/integration/chunk-fixture.ts"), import("./oracle/lib/integration/creature-fixture.ts"), import("./oracle/lib/integration/liquid-fixture.ts"), import("./oracle/lib/integration/liquid-scenario.ts")]); console.log("oracle lib imports ok")'
```

Full `gen-fixture.sh` / creature / liquid official-server generation was deferred to a later oracle-tooling slice because the moved modules were covered by the existing 72-test oracle helper suite plus direct Node loader import.

### Slice 2 - Preserve Or Relocate Shared Fixtures

Status: landed on 2026-06-23 with fixtures kept in place.

Goal: separate durable oracle data from the old TypeScript test suite.

- [x] Decide whether `test/fixtures/**` remains as the shared oracle fixture root or moves to `fixtures/oracle/**`.
- [x] If it stays, document that `test/fixtures/**` is not owned by the deleted Vitest test suite.
- [x] If it moves, update all native `include_str!` paths in the same commit. Not moved; native include paths stay stable.
- [x] Keep fixture generation docs in `oracle/README.md` aligned with the chosen path.
- [x] Defer fixture subset pruning until native and oracle consumers have a dedicated fixture-hygiene pass.

Current native fixture consumers include:

- PRNG/noise/biome/worldgen fixtures in `mclone-worldgen`
- carver/surface/full-decorated scheduler fixtures in `mclone-worldgen`
- liquid and lighting fixtures in `mclone-server`
- `render/visgraph-synthetic.json` in `mclone-mesh`

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-worldgen -p mclone-server -p mclone-mesh
```

### Slice 3 - Delete Legacy Engine Source And Vitest Suite

Status: landed on 2026-06-23.

Goal: remove the obsolete implementation and its unit tests.

- [x] Delete `src/**`.
- [x] Delete root `test/**/*.test.ts` and TS support files that exist only for the legacy implementation.
- [x] Keep `test/fixtures/**` or its replacement fixture root from Slice 2.
- [x] Delete `vitest.config.ts`.
- [x] Remove `vitest` from root devDependencies.
- [x] Remove or retarget root `test` / `test:watch` scripts.
- [x] Run `rg` to prove no retained file imports `../src/**`, `../../src/**`, or root `src/**`.

Validation:

```bash
rg -n "from [\"'].*src/|src/" scripts oracle docs native package.json
cargo test --manifest-path native/Cargo.toml
```

Expected remaining `src/` hits after this slice should be native Rust file paths, `reference/minecraft-1.17.1/src/**`, and ordinary words. There should be no root `src/**` imports.

### Slice 4 - Delete Legacy Browser App, Browser Tests, And Deploy Path

Status: landed on 2026-06-23.

Goal: remove the browser/Vite/Playwright surface that launched the TypeScript engine.

- [x] Delete root `index.html` and `smoke.html`.
- [x] Delete root `vite.config.ts`.
- [x] Delete root `playwright*.ts`.
- [x] Delete `test/browser/**`.
- [x] Delete `scripts/deploy-legacy.sh`.
- [x] Delete `scripts/dev-lan.mjs`.
- [x] Delete legacy Deno smoke scripts that import `../src/**`.
- [x] Audit root Playwright helpers:
  - deleted `scripts/smoke-deployed.mjs`, `scripts/smoke-mobile.mjs`, and `scripts/smoke-timelapse.mjs`
  - native web validation remains in `native/apps/mclone-web-client/scripts/browser-smoke.mjs`
- [x] Remove root package scripts for legacy browser/probe/perf paths.
- [x] Remove `vite` and root-only browser devDependencies if unused.

Validation:

```bash
pnpm native:web:build
pnpm native:web:smoke
pnpm native:web:app-smoke
pnpm native:web:mobile-smoke
```

For any retained deployed-site smoke helper, capture screenshots to `/tmp` and inspect them before keeping the script.

### Slice 5 - Package And Tooling Cleanup

Status: landed on 2026-06-23.

Goal: make root package scripts describe the current repo.

- [x] Keep native scripts:
  - `native:*`
  - `deploy` as alias for `native:web:deploy`
  - `assets:pack*`
  - `oracle:gen`
- [x] Decide whether `pnpm test` should become `cargo test --manifest-path native/Cargo.toml` or be removed to avoid a misleading Node test entrypoint.
- [x] Retarget `typecheck` to `pnpm native:web:typecheck` only if that is useful as a root alias.
- [x] Keep `@playwright/test` while native web smoke scripts import it.
- [x] Keep `typescript`, `@types/node`, and `@webgpu/types` while native web typecheck/build-glue uses them.
- [x] Remove unused `dependencies` and `devDependencies`.
- [x] Regenerate `pnpm-lock.yaml`.
- [x] Update `.gitignore` to remove legacy-only Playwright/Vite artifacts only if they are no longer produced by retained tooling. No `.gitignore` change needed in this slice.

Validation:

```bash
pnpm install --lockfile-only
pnpm native:web:typecheck
pnpm native:web:build
cargo test --manifest-path native/Cargo.toml
```

### Slice 6 - Documentation Burn-down

Goal: make docs speak from the native-first state instead of warning around old code.

- [ ] Update `README.md`:
  - remove the legacy TypeScript implementation paragraph
  - remove links to `docs/legacy-typescript-engine-info.md`
  - remove links to `docs/tactical/legacy/**`
  - replace TypeScript worldgen status links with native status or oracle fixture notes
- [ ] Update `AGENTS.md`:
  - remove the legacy routing section
  - keep the explicit native default target
  - keep native web/WASM guidance
  - keep oracle/reference-tree guidance
- [ ] Delete `docs/legacy-typescript-engine-info.md`.
- [ ] Delete `docs/tactical/legacy/**`.
- [ ] Audit durable docs with stale `src/**` links:
  - `docs/worldgen-status.md`
  - `docs/carver-status.md`
  - `docs/liquids.md`
  - `docs/runtime-data-model.md`
  - `docs/architecture.md`
  - `docs/protocol.md`
  - `docs/loading-persistence.md`
  - `docs/creatures.md`
  - `docs/entities.md`
  - `docs/gui.md`
  - `docs/lighting*.md`
  - `docs/worker-ownership.md`
- [ ] Delete obsolete durable docs that only describe the legacy implementation and have no native planning value.
- [ ] For docs that still matter, rewrite paths to native crates or oracle fixture tooling.
- [ ] Keep links to `reference/minecraft-1.17.1/src/**`; those are not legacy TypeScript links.

Validation:

```bash
rg -n "legacy TypeScript|TypeScript implementation|docs/tactical/legacy|docs/legacy-typescript-engine-info|test/browser|playwright|src/" README.md AGENTS.md docs
```

Expected remaining `src/` hits should primarily be:

- `reference/minecraft-1.17.1/src/**`
- native Rust paths such as `native/crates/.../src/**`
- native web paths such as `native/apps/mclone-web-client/src/**`

### Slice 7 - Final Negative Search And Native Validation

Goal: prove the live tree no longer carries the retired surface.

- [ ] Confirm these paths no longer exist:
  - `src/`
  - `docs/tactical/legacy/`
  - `docs/legacy-typescript-engine-info.md`
  - `test/browser/`
  - root `playwright*.ts`
  - root `vite.config.ts`
  - root `vitest.config.ts`
  - root `index.html`
  - root `smoke.html`
  - `scripts/deploy-legacy.sh`
- [ ] Confirm root package scripts no longer mention legacy commands.
- [ ] Confirm no retained script imports root `src/**`.
- [ ] Run the native validation suite.
- [ ] Run native web validation.
- [ ] Update this tactical with landed notes and any intentional survivors.

Validation:

```bash
test ! -d src
test ! -d docs/tactical/legacy
test ! -d test/browser
test -z "$(find . -maxdepth 1 -name 'playwright*.ts' -print)"
rg -n "legacy-deploy|dev:browser|probe:browser|test:browser|perf:d5|vite build|vitest|/src/renderer/main.ts" package.json scripts docs README.md AGENTS.md
cargo test --manifest-path native/Cargo.toml
pnpm native:web:build
pnpm native:web:smoke
pnpm native:web:app-smoke
```

## Risk Notes

- The main technical risk is accidentally deleting oracle fixtures or decoder helpers that native Rust tests still use. Handle `test/fixtures/**` and retained `oracle/lib/**` imports before the broad delete.
- The main workflow risk is leaving root `package.json` in a misleading half-state. Package scripts should either be current or gone.
- The main documentation risk is preserving obsolete tactical guidance under a different name. Copy only durable facts into current docs; let Git history preserve old plans.
- The native web path still uses TypeScript and Playwright. Retirement succeeds when legacy engine TypeScript is gone, not when every `.ts` file is gone.

## Landed Notes

- 2026-06-23: Slice 1 moved oracle-owned TypeScript helpers from `src/oracle/**` to `oracle/lib/**`, copied the required BitStorage utility into `oracle/lib/util/bit-storage.ts`, retargeted oracle integration CLIs and existing TS tests, and updated oracle/durable docs. Validation: `pnpm exec vitest run test/oracle`; direct Node TS-loader import of moved oracle libraries.
- 2026-06-23: Slices 2-5 kept `test/fixtures/**` as shared oracle fixture data with its own README, removed `src/**`, removed non-fixture root TypeScript tests, removed root browser app/config/test files, deleted legacy Deno/Vite/Playwright smoke and deploy helpers, retargeted root `test` / `typecheck` to native validation lanes, removed `vite` / `vitest` / `unzipit`, regenerated the pnpm lockfile, and simplified host capability checks around native web validation. Validation: `test ! -d src`; `test ! -d test/browser`; `test ! -f vite.config.ts`; `test ! -f vitest.config.ts`; no root `playwright*.ts`; `node -c scripts/check-host-capabilities.mjs`; negative searches for retired package/doc/import references; `cargo test --manifest-path native/Cargo.toml -p mclone-worldgen -p mclone-server -p mclone-mesh`; `pnpm native:web:typecheck`; `pnpm test`.
