# 071: Native Web Glue TypeScript Graduation

Status: **App/touch/input/HUD, render-compiler, and server-worker TypeScript slices landed; optional thread-smoke worker glue remains.**

## Context

070 hardened the native web glue without changing deploy shape: the browser still loads
plain ES modules from `native/apps/mclone-web-client/www/`, and `scripts/deploy-native-web.sh`
still copies that directory into `dist-native-web` before adding the wasm-bindgen bundle and
the Minecraft asset pack.

That was the right first step while `mclone-web-app.js` was oversized. 070 now has a cleaner
module split:

| File | Lines | Current role |
|---|---:|---|
| `www/mclone-web-app.js` | 962 | entry module, URL/versioning, runtime global, `WebChunkApp`, bootstrap |
| `www/mclone-web-touch.js` | 339 | touch controls and touch capability helpers |
| `www/mclone-web-input.js` | 297 | keyboard/mouse/hotbar binding |
| `www/mclone-web-hud.js` | 291 | HUD/menu/settings DOM glue |

The current `pnpm native:web:typecheck` gate uses `tsc --checkJs` plus JSDoc typedefs. That
keeps the code checkable, but the module split makes the remaining JSDoc machinery mostly
temporary scaffolding. This tactical graduates the authored browser glue to real TypeScript
while preserving the runtime module graph and smoke/deploy behaviour.

## Goals

- Author the native web app glue in `.ts` where real exported interfaces are materially better
  than file-scoped JSDoc typedefs.
- Keep the browser-loaded files as `.js` ES modules with the same public URLs and static import
  specifiers.
- Keep the wasm-bindgen output and generated `mclone_web_client.d.ts` flow from 070 Stage 2.
- Preserve the load-bearing `globalThis.__mcloneWebApp` contract used by the smoke harness.
- Preserve the Stage 3/first-follow-up correctness fence: chunk-smoke canvas PNG must remain
  byte-identical.

## Non-goals / guardrails

- This is **native web/WASM** glue only. Do not revive the retired browser engine.
- Do not change Rust engine/runtime behaviour, worker protocol semantics, render output, input
  behaviour, touch behaviour, or HUD/menu behaviour.
- Do not introduce a bundler unless a later tactical explicitly chooses one. Use `tsc` ES-module
  emit.
- Do not commit generated JavaScript. Emitted files should live under `native/target/...` and in
  `dist-native-web` during bundling/deploy.
- Do not ship `.ts` files. The deploy bundle should contain browser-loadable `.js`, `.html`,
  `.wasm`, asset-pack files, and other static assets only.
- Keep runtime imports as `.js` specifiers in TypeScript source, for example
  `import { TouchControls } from "./mclone-web-touch.js";`. The emitted graph should look like
  today's graph to the browser.
- Keep `versionedUrl(...)` in a `www/`-relative emitted module so worker/wasm URLs keep resolving
  as they do today.
- Do not convert the ABI lock single-source modules (`mclone-render-compiler-abi.js`,
  `mclone-runner-shared-abi.js`) in the first slice. Their Rust lock tests currently parse those
  JS files directly; converting them needs a separate lock-test update.

## Recommended Shape

Use the existing `www/` directory for authored web sources and static assets:

- Converted source files become `www/*.ts`.
- Unconverted shipped glue can remain `www/*.js` during the migration.
- `app.html` and `index.html` keep importing `.js` entrypoints.
- TypeScript source imports sibling modules using `.js` specifiers.

Add a staged web-root build step:

1. Clean a generated web-root under `native/target/`, for example
   `native/target/mclone-web-client-www`.
2. Copy `www/` into that generated root while excluding `.ts` source files.
3. Run `tsc` with a dedicated emit config that compiles `www/**/*.ts` into the same generated
   root, preserving file names such as `mclone-web-touch.js`.
4. Point the smoke harness and deploy script at the generated root instead of raw `www/`.

This keeps source control clean, lets converted and unconverted modules coexist, and keeps the
deployed URL graph stable.

Recommended files:

- `native/apps/mclone-web-client/scripts/build-web-glue.mjs`: shared staging helper for smoke and
  deploy.
- `native/apps/mclone-web-client/tsconfig.web.json`: emit config for `www/**/*.ts`, likely
  `target: ES2022`, `module: ESNext`, `moduleResolution: Bundler`, `strict: true`,
  `rootDir: "www"`, and `outDir: "../../target/mclone-web-client-www"`.
- `native/apps/mclone-web-client/tsconfig.json`: keep the no-emit check gate, but include both
  `www/**/*.js` and `www/**/*.ts` plus `scripts/*.mjs`.
- `package.json`: add a small `native:web:glue` script if useful, and keep
  `native:web:typecheck` as the no-emit gate after wasm-bindgen has produced the `.d.ts`.

## Implementation Checklist

### Stage 0 - Baseline and Inventory

- [ ] Confirm current workstream is native web/WASM.
- [ ] Run `pnpm native:web:chunk-smoke` before edits.
- [ ] Save the baseline hash with `shasum -a 256 /tmp/mclone-native-web-canvas.png`.
      Expected current fence:
      `0ba8251f6c0dba012782e80921b8d45ead799a216decf50e9041958babcb6a4c`.
- [ ] Inspect `scripts/deploy-native-web.sh`, `native/apps/mclone-web-client/tsconfig.json`,
      `native/apps/mclone-web-client/scripts/browser-smoke.mjs`, and the current `www/*.js`
      module graph.
- [ ] Record starting line counts for app/touch/input/HUD.

### Stage 1 - Emit Plumbing + First TS Module

First good slice: add the staged web-root build and convert only the most isolated module,
`mclone-web-touch.js` -> `mclone-web-touch.ts`.

- [ ] Add the staged web-root build helper.
- [ ] Add `tsconfig.web.json` for emit.
- [ ] Update `browser-smoke.mjs` to build/serve from the staged web-root.
- [ ] Update `scripts/deploy-native-web.sh` so deploy copies the staged web-root rather than raw
      `www/`.
- [ ] Update `package.json` scripts if needed.
- [ ] Convert `www/mclone-web-touch.js` to `www/mclone-web-touch.ts`.
- [ ] Replace JSDoc object typedefs in that module with real `interface` / `type` declarations.
- [ ] Keep emitted runtime imports and URLs unchanged.
- [ ] Verify `dist-native-web` / staged root contains `mclone-web-touch.js` and no `.ts` files.
- [ ] Update this doc's `## Landed` section and the 071 row in `docs/tactical/README.md`.
- [ ] Commit this slice before converting input/HUD/app.

### Stage 2 - Convert Input and HUD

Recommended as two commits if the diffs are large.

- [ ] Convert `www/mclone-web-input.js` to `.ts`.
- [ ] Convert `www/mclone-web-hud.js` to `.ts`.
- [ ] Replace cross-file JSDoc imports with exported interfaces/types.
- [ ] Keep callbacks and state-bag dependencies explicit so no app/runtime import cycle is
      introduced.
- [ ] Preserve touch/mobile menu probes and input runtime methods exactly.
- [ ] Update this doc and commit each good slice.

### Stage 3 - Convert App Core

- [ ] Convert `www/mclone-web-app.js` to `.ts`.
- [ ] Promote shared contracts to real exported types where useful: `AppRuntime`, runtime state,
      wasm-return bags, pending compile entries, render session aliases, and app-shaped module
      interfaces.
- [ ] Keep `globalThis.__mcloneWebApp = runtime` and the boot-time method installation shape
      intact.
- [ ] Keep `versionedUrl(...)`, `normalizedDeployAssetVersion(...)`, and all worker/wasm asset
      URLs resolving to the same values.
- [ ] Keep the smoke harness global type visibility clean. If needed, add a small ambient
      declaration file rather than hiding the global behind `any`.
- [ ] Update this doc and commit.

### Stage 4 - Optional Worker Glue Conversion

Only after app/touch/input/HUD are stable.

- [ ] Consider converting worker producer/consumer glue (`mclone-render-compiler-shared.js`,
      `mclone-render-compiler-worker.js`, `mclone-integrated-server-worker.js`,
      `mclone-server-job-worker.js`, `mclone-thread-smoke-worker.js`).
- [ ] Leave ABI lock modules as JS unless the lock tests are updated in the same commit.
- [ ] Keep worker `new Worker(...)` URLs and module type unchanged.
- [ ] Update this doc and commit each worker group separately.

### Stage 5 - Cleanup

- [ ] Remove obsolete JSDoc typedef blocks once their TS replacements exist.
- [ ] Update comments in deploy/typecheck scripts that still describe the old no-emit-only deploy
      shape.
- [ ] Refresh final line counts and any residual `.js`/`.ts` inventory in this doc.
- [ ] Decide whether optional eslint is still worth doing, or leave it as a separate 070/071
      follow-up.

## Validation

Run the full lane before every commit that changes build plumbing or converted source:

- `pnpm native:web:typecheck`
- `pnpm native:web:glue` if that script exists, or the equivalent staging helper directly
- `node --check` on emitted staged `.js` for touched converted modules, plus any touched
  unconverted `www/*.js`
- `cargo test --manifest-path native/Cargo.toml`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown --manifest-path native/Cargo.toml`
- `pnpm native:web:build`
- `pnpm native:web:bundle`
- Verify the bundle/staged root ships no `.ts` files
- `pnpm native:web:app-smoke`
- `pnpm native:web:chunk-smoke`
- `shasum -a 256 /tmp/mclone-native-web-canvas.png` and compare with the baseline
- `pnpm native:web:mobile-smoke`
- `pnpm native:web:movement-perf`
- `pnpm native:movement:smoke`
- `pnpm native:timedemo:smoke`

Expected output fence: `pnpm native:web:chunk-smoke` should keep producing
`0ba8251f6c0dba012782e80921b8d45ead799a216decf50e9041958babcb6a4c` for
`/tmp/mclone-native-web-canvas.png`. A mismatch means the migration changed runtime behaviour,
not just source format/build plumbing.

## Landed

### Stage 1 — staged emit root + touch TypeScript conversion (landed)

The first slice added the generated web-root plumbing and converted only
`www/mclone-web-touch.js` to authored TypeScript (`www/mclone-web-touch.ts`). Browser-loaded URLs
and static import specifiers stayed stable: `mclone-web-app.js`, `mclone-web-hud.js`, and
`mclone-web-input.js` still import `./mclone-web-touch.js`; `app.html` / `index.html` still load
`.js` entrypoints; `versionedUrl(...)` and the worker/wasm URL helpers remain in
`mclone-web-app.js`. The emitted staged root and deploy bundle both contain
`mclone-web-touch.js` and no `.ts` files.

Build/deploy shape changed from raw `www/` serving/copying to a staged generated root:

- New `scripts/build-web-glue.mjs` cleans `native/target/mclone-web-client-www`, copies `www/`
  while excluding `.ts`, runs `tsc -p tsconfig.web.json`, and fails if `.ts` leaks into the staged
  output.
- New `tsconfig.web.json` emits `www/**/*.ts` into that staged root with stable `.js` filenames.
- `browser-smoke.mjs` now builds and serves the staged root before launching Playwright; `--build-only`
  also stages it for `native:web:typecheck`.
- `scripts/deploy-native-web.sh` now copies the staged root into `dist-native-web` before adding the
  wasm-bindgen `pkg/` files and asset pack.
- `package.json` adds `pnpm native:web:glue` as the direct staging gate.

Line counts: starting app/touch/input/HUD was `962 / 339 / 297 / 291` lines. Ending source counts
are app `962`, touch TS `352`, input `297`, HUD `291`. The emitted staged/deploy
`mclone-web-touch.js` is `273` lines. The new staging helper is `88` lines and the emit tsconfig is
`31` lines.

Correctness and perf fences held. Baseline and final `pnpm native:web:chunk-smoke` both produced
`0ba8251f6c0dba012782e80921b8d45ead799a216decf50e9041958babcb6a4c` for
`/tmp/mclone-native-web-canvas.png`; the canvas screenshot was visually inspected. Movement perf
passed with 11 movement compiles over 3 chunk boundaries: `totalMs` avg `11.7` ms (min `8.0`, max
`17.1`), `workerRoundTripMs` avg `6.8` ms (min `5.8`, max `8.2`), `decodeFinishApplyMs` avg `1.6`
ms, and `maxFrameGapMs` avg `8.5` ms. Transport stayed `shared-result-buffer`; no shared-result
overflow and no generated-view fallback.

Validation (all green): `pnpm native:web:chunk-smoke` baseline, `pnpm native:web:glue`,
`pnpm native:web:typecheck`, `node --check` on the staging helper, browser-smoke harness, emitted
`mclone-web-touch.js`, and staged app/HUD/input JS, `bash -n scripts/deploy-native-web.sh`,
`cargo test --manifest-path native/Cargo.toml`, `cargo check -p mclone-web-client --target
wasm32-unknown-unknown --manifest-path native/Cargo.toml`, `pnpm native:web:build`,
`pnpm native:web:bundle`, explicit staged/deploy inventory checks (`mclone-web-touch.js` present,
no `.ts`), `pnpm native:web:app-smoke`, `pnpm native:web:chunk-smoke` plus final sha256,
`pnpm native:web:mobile-smoke` with mobile canvas inspection, `pnpm native:web:movement-perf`,
`pnpm native:movement:smoke`, and `pnpm native:timedemo:smoke`.

Implementation commit: `09c3f2ffe4afdc346e5a6d1a4a65674b25855984`
(`071: graduate touch glue through staged TS emit`).

### Stage 2a — input TypeScript conversion (landed)

The second slice converted only `www/mclone-web-input.js` to authored TypeScript
(`www/mclone-web-input.ts`). Runtime imports and URLs stayed stable: `mclone-web-app.js` still
imports `./mclone-web-input.js`, `app.html` / `index.html` still load `.js` entrypoints, and the
new input-to-touch dependency is a type-only `import type` from `./mclone-web-touch.js` that is
erased from the emitted runtime module. The staged root and deploy bundle both contain
`mclone-web-input.js` and no `.ts` files.

Line counts from the start of this slice were app/touch/input/HUD `962 / 352 / 297 / 291`.
Ending source counts are app `962`, touch TS `352`, input TS `289`, HUD `291`. The emitted staged
`mclone-web-input.js` is `232` lines.

Correctness and perf fences held. Pre-slice and final `pnpm native:web:chunk-smoke` both produced
`0ba8251f6c0dba012782e80921b8d45ead799a216decf50e9041958babcb6a4c` for
`/tmp/mclone-native-web-canvas.png`; the desktop chunk canvas and mobile app canvas were visually
inspected. Movement perf passed across 3 chunk boundaries, moving from center `[-1, -1]` to
`[-1, 2]` with final `cameraZ=33.95248`, `compileTimingCount=28`, last compile `8.4` ms total /
`6.2` ms worker round trip, `renderCount=263`, `frameCount=60`, and `maxFrameGapMs=10.215`.
Runner transport stayed `shared-memory` with runner frames `28 / 110`, worldgen frames `5 / 5`
(`maxRequestUs=702000`), and light-status frames `5 / 5` (`maxRequestUs=195000`).

Validation (all green): `pnpm native:web:chunk-smoke` baseline, `pnpm native:web:glue`,
`pnpm native:web:typecheck`, `node --check native/target/mclone-web-client-www/mclone-web-input.js`
plus staged app/HUD/touch JS, `bash -n scripts/deploy-native-web.sh`,
`cargo test --manifest-path native/Cargo.toml`, `cargo check -p mclone-web-client --target
wasm32-unknown-unknown --manifest-path native/Cargo.toml`, `pnpm native:web:build`,
`pnpm native:web:bundle`, explicit staged/deploy inventory checks (`mclone-web-input.js` present,
no `.ts`), `pnpm native:web:app-smoke`, `pnpm native:web:chunk-smoke` plus final sha256,
`pnpm native:web:mobile-smoke` with mobile canvas inspection, `pnpm native:web:movement-perf`,
`pnpm native:movement:smoke`, and `pnpm native:timedemo:smoke`.

Implementation commit: `e4843ed7e41fb0978f655f85661658b109021644`
(`071: graduate input glue to TypeScript`).

### Stage 2b — HUD TypeScript conversion (landed)

The third slice converted only `www/mclone-web-hud.js` to authored TypeScript
(`www/mclone-web-hud.ts`). Runtime imports and URLs stayed stable: `mclone-web-app.js` still
imports `./mclone-web-hud.js`, `app.html` / `index.html` still load `.js` entrypoints, and the
HUD module still imports the live `hasTouchInput` helper from `./mclone-web-touch.js`. Its
`TouchControls` dependency is now type-only and erased from runtime emit. The staged root and
deploy bundle both contain `mclone-web-hud.js` and no `.ts` files.

Line counts from the start of this slice were app/touch/input/HUD `962 / 352 / 289 / 291`.
Ending source counts are app `962`, touch TS `352`, input TS `289`, HUD TS `261`. The emitted
staged `mclone-web-hud.js` is `214` lines.

Correctness and perf fences held. Pre-slice and final `pnpm native:web:chunk-smoke` both produced
`0ba8251f6c0dba012782e80921b8d45ead799a216decf50e9041958babcb6a4c` for
`/tmp/mclone-native-web-canvas.png`; the desktop chunk canvas and mobile app canvas were visually
inspected. Mobile smoke covered the HUD/menu-specific surface: initial HUD closed on mobile, menu
open, debug HUD open/close, settings/menu state, and touch controls. Movement perf passed across
3 chunk boundaries, moving from center `[-1, -1]` to `[-1, 2]` with final `cameraZ=34.02544`,
`compileTimingCount=31`, last compile `16.6` ms total / `6.8` ms worker round trip,
`renderCount=270`, `frameCount=60`, and `maxFrameGapMs=10.305`. Runner transport stayed
`shared-memory` with runner frames `29 / 111`, worldgen frames `5 / 5` (`maxRequestUs=697000`),
and light-status frames `5 / 5` (`maxRequestUs=197000`).

Validation (all green): `pnpm native:web:chunk-smoke` baseline, `pnpm native:web:glue`,
`pnpm native:web:typecheck`, `node --check native/target/mclone-web-client-www/mclone-web-hud.js`
plus staged app/input/touch JS, `node --check dist-native-web/mclone-web-hud.js`,
`bash -n scripts/deploy-native-web.sh`, `cargo test --manifest-path native/Cargo.toml`,
`cargo check -p mclone-web-client --target wasm32-unknown-unknown --manifest-path
native/Cargo.toml`, `pnpm native:web:build`, `pnpm native:web:bundle`, explicit staged/deploy
inventory checks (`mclone-web-hud.js` present, no `.ts`), `pnpm native:web:app-smoke`,
`pnpm native:web:chunk-smoke` plus final sha256, `pnpm native:web:mobile-smoke` with mobile canvas
inspection, `pnpm native:web:movement-perf`, `pnpm native:movement:smoke`, and
`pnpm native:timedemo:smoke`.

Implementation commit: `db0a52726aa22e493c53b884d4c39bdc5e2a68ca`
(`071: graduate HUD glue to TypeScript`).

### Stage 3 — app core TypeScript conversion (landed)

The fourth slice converted only `www/mclone-web-app.js` to authored TypeScript
(`www/mclone-web-app.ts`). Runtime URLs and import specifiers stayed stable: `app.html` /
`index.html` still load `.js` entrypoints, the emitted app still imports
`./mclone-render-compiler-shared.js`, `./mclone-web-input.js`, `./mclone-web-hud.js`, and
`./mclone-web-touch.js`, and the dynamic wasm-bindgen import still goes through
`versionedUrl("./pkg/mclone_web_client.js")`. The boot-time global contract stayed intact:
`globalThis.__mcloneWebApp = runtime` and `globalThis.__mcloneNativeAppReady = boot()` are still
published by the app module, with global types now declared in TypeScript. A 21-line
`mclone-render-compiler-shared.d.ts` declaration shim types the existing JS render-compiler helper
without converting or shipping that worker glue.

The staged root and deploy bundle both contain emitted `mclone-web-app.js` and no `.ts` files.
Source `.d.ts` declarations are excluded by the staged copy filter and are not deployed.

Line counts from the start of this slice were app/touch/input/HUD `962 / 352 / 289 / 261`.
Ending source counts are app TS `932`, touch TS `352`, input TS `289`, HUD TS `261`. The emitted
staged app JS is `745` lines; emitted touch/input/HUD remain `273 / 232 / 214` lines.

Correctness and perf fences held. Pre-slice and final `pnpm native:web:chunk-smoke` both produced
`0ba8251f6c0dba012782e80921b8d45ead799a216decf50e9041958babcb6a4c` for
`/tmp/mclone-native-web-canvas.png`; the desktop chunk canvas and mobile app canvas were visually
inspected. Movement perf passed across 3 chunk boundaries, moving from center `[-1, -1]` to
`[-1, 2]` with `frameCountDelta=54`, `renderCountDelta=54`, and 9 movement compile timings:
`totalMs` avg `11.5` ms (min `8.2`, max `18.0`), `workerRoundTripMs` avg `6.9` ms (min `6.1`,
max `8.8`), `decodeFinishApplyMs` avg `1.8` ms, and `maxFrameGapMs` avg `8.5` ms. Transport
stayed `shared-result-buffer`; max packed/shared-result byte length was `1621772`; no transferred
request/response bytes, no shared-result overflow, and no generated-view fallback.

Validation (all green): `pnpm native:web:chunk-smoke` baseline, `pnpm native:web:glue`,
`pnpm native:web:typecheck`, `pnpm native:web:bundle`, `bash -n scripts/deploy-native-web.sh`,
`node --check` on emitted staged and deploy app/touch/input/HUD JS, explicit staged/deploy
inventory checks (`mclone-web-app.js` and `mclone-web-touch.js` present, no `.ts`), `cargo test
--manifest-path native/Cargo.toml`, `cargo check -p mclone-web-client --target
wasm32-unknown-unknown --manifest-path native/Cargo.toml`, `pnpm native:web:build`,
`pnpm native:web:app-smoke`, `pnpm native:web:chunk-smoke` plus final sha256,
`pnpm native:web:mobile-smoke` with mobile canvas inspection, `pnpm native:web:movement-perf`,
`pnpm native:movement:smoke`, `pnpm native:timedemo:smoke`, and `pnpm native:web:smoke`.

Implementation commit: `55b68f706ef470c986dacd739dba58cf7001d833`
(`071: graduate app glue to TypeScript`).

### Stage 4a — render compiler worker TypeScript conversion (landed)

The fifth slice converted only the render compiler consumer/producer glue:
`www/mclone-render-compiler-shared.js` is now authored as
`www/mclone-render-compiler-shared.ts`, and `www/mclone-render-compiler-worker.js` is now
authored as `www/mclone-render-compiler-worker.ts`. The previous temporary
`mclone-render-compiler-shared.d.ts` shim was removed because the shared module now exports real
interfaces/types. `mclone-render-compiler-abi.js` stayed JavaScript so
`render_compiler_abi_lock.rs` continues to parse the single source of truth directly; a small
`mclone-render-compiler-abi.d.ts` declaration shim gives the converted TS modules typed ABI
constants without changing the runtime module.

Runtime imports and URLs stayed stable: app and smoke still import
`./mclone-render-compiler-shared.js`; the app still constructs the render compiler worker from
`versionedUrl("./mclone-render-compiler-worker.js")`; the smoke harness still uses
`new URL("./mclone-render-compiler-worker.js", import.meta.url)`; the worker still imports
`./mclone-render-compiler-abi.js` and `./mclone-render-compiler-shared.js`; and the dynamic
wasm-bindgen import still uses the bindgen JS URL supplied by the app/smoke caller. Worker
construction remains `type: "module"`. The staged root and deploy bundle both contain emitted
`mclone-render-compiler-shared.js`, `mclone-render-compiler-worker.js`, and
`mclone-render-compiler-abi.js`, with no `.ts` files shipped.

Line counts from the start of this slice were render-compiler shared JS `660`, worker JS `417`,
shared declaration shim `21`, and ABI JS `33`. Ending source counts are shared TS `644`, worker
TS `400`, ABI declaration shim `16`, and ABI JS still `33`. The emitted staged JS is shared
`467` lines and worker `292` lines.

Correctness and perf fences held. Pre-slice and final `pnpm native:web:chunk-smoke` both produced
`0ba8251f6c0dba012782e80921b8d45ead799a216decf50e9041958babcb6a4c` for
`/tmp/mclone-native-web-canvas.png`; the desktop chunk canvas, mobile app canvas, and movement
perf canvas were visually inspected. Movement perf passed across 3 chunk boundaries, moving from
center `[-1, -1]` to `[-1, 2]`, with `compileTimingCount=29`, 16 accepted compile timings,
`renderCount=278`, `frameCount=59`, and `maxFrameGapMs=9.33`. Accepted compile `totalMs` averaged
`12.48` ms (min `8.1`, max `17.8`); `workerRoundTripMs` averaged `6.8` ms (min `5.8`, max `9.0`);
packed byte length ranged `936372..1607788`. The last compile was request `29`, `9.0` ms total /
`9.0` ms worker round trip, packed `1160484` bytes. Transport stayed `shared-result-buffer`, shared
result buffers stayed enabled, no shared-result overflow occurred, no generated-view fallback was
used, and compile/worker compile counts both reached `29`.

Validation (all green): `pnpm native:web:chunk-smoke` baseline, `pnpm native:web:glue`,
`pnpm native:web:typecheck`, `node --check` on emitted staged and deploy
`mclone-render-compiler-shared.js` / `mclone-render-compiler-worker.js`,
`bash -n scripts/deploy-native-web.sh`, `cargo test --manifest-path native/Cargo.toml`,
`cargo check -p mclone-web-client --target wasm32-unknown-unknown --manifest-path
native/Cargo.toml`, `pnpm native:web:build`, `pnpm native:web:bundle`, explicit staged/deploy
inventory checks (`mclone-render-compiler-shared.js`, `mclone-render-compiler-worker.js`, and
`mclone-render-compiler-abi.js` present, no `.ts`), `pnpm native:web:app-smoke`,
`pnpm native:web:chunk-smoke` plus final sha256, `pnpm native:web:mobile-smoke` with mobile canvas
inspection, `pnpm native:web:movement-perf`, `pnpm native:web:smoke`,
`pnpm native:movement:smoke`, and `pnpm native:timedemo:smoke`.

Implementation commit: `b9883a36d7cf6b1738317b94b4a40ce544fb25de`
(`071: graduate render compiler glue to TypeScript`).

### Stage 4b — server worker TypeScript conversion (landed)

The sixth slice converted the native-web integrated server worker pair:
`www/mclone-integrated-server-worker.js` is now authored as
`www/mclone-integrated-server-worker.ts`, and `www/mclone-server-job-worker.js` is now authored
as `www/mclone-server-job-worker.ts`. `mclone-runner-shared-abi.js` stayed JavaScript so
`runner_shared_abi_lock.rs` continues to parse the single source of truth directly; a small
`mclone-runner-shared-abi.d.ts` declaration shim gives the converted workers typed ABI constants
without changing the runtime module.

Runtime imports and URLs stayed stable: the app still constructs the integrated-server and
server-job workers through `versionedUrl("./mclone-integrated-server-worker.js")` and
`versionedUrl("./mclone-server-job-worker.js")`; the smoke harness still uses
`new URL("./mclone-integrated-server-worker.js", import.meta.url)` and
`new URL("./mclone-server-job-worker.js", import.meta.url)`; both emitted workers still import
`./mclone-runner-shared-abi.js`; and each worker's dynamic wasm-bindgen import still uses the
bindgen JS URL supplied by the app/smoke caller. Worker construction remains `type: "module"`.
The staged root and deploy bundle both contain emitted `mclone-integrated-server-worker.js`,
`mclone-server-job-worker.js`, and `mclone-runner-shared-abi.js`, with no `.ts` files shipped.

Line counts from the start of this slice were integrated-server worker JS `472`, server-job
worker JS `186`, and runner shared ABI JS `32`. Ending source counts are integrated-server
worker TS `435`, server-job worker TS `164`, runner shared ABI declaration shim `7`, and runner
shared ABI JS still `32`. The emitted staged JS is integrated-server worker `333` lines and
server-job worker `119` lines.

Correctness and perf fences held. Pre-slice and final `pnpm native:web:chunk-smoke` both produced
`0ba8251f6c0dba012782e80921b8d45ead799a216decf50e9041958babcb6a4c` for
`/tmp/mclone-native-web-canvas.png`; the desktop chunk canvas, app canvas, mobile app canvas, and
movement-perf canvas were visually inspected. Movement perf passed across 3 chunk boundaries,
moving from center `[-1, -1]` to `[-1, 2]`, with `compileTimingCount=27`, 16 accepted compile
timings, `renderCount=281`, `frameCount=66`, and `maxFrameGapMs=9.32`. Accepted compile
`totalMs` averaged `12.74` ms (min `7.6`, max `17.4`); `workerRoundTripMs` averaged `6.96` ms
(min `5.4`, max `8.6`); packed byte length ranged `936372..1621772`. The last compile was
request `27`, `16.8` ms total / `7.1` ms worker round trip, packed `1160484` bytes. Render
compiler transport stayed `shared-result-buffer`; no shared-result overflow occurred and no
generated-view fallback was used. Server runner metrics stayed on shared memory:
`runnerFrameMetrics` reported `29` request frames / `113` response frames with `0` shared-buffer
fallback response frames; `worldgenJobFrameMetrics` reported `5` request frames / `5` response
frames, `5901775` request bytes, `55106044` response bytes, and `0` shared-buffer fallback
response frames; `lightStatusJobFrameMetrics` reported `5` request frames / `5` response frames,
`38916115` request bytes, `1024056` response bytes, and `0` shared-buffer fallback response
frames.

Validation (all green): `pnpm native:web:chunk-smoke` baseline, `pnpm native:web:glue`,
`pnpm native:web:typecheck`, `node --check` on emitted staged and deploy
`mclone-integrated-server-worker.js` / `mclone-server-job-worker.js`,
`bash -n scripts/deploy-native-web.sh`, `cargo test --manifest-path native/Cargo.toml`,
`cargo check -p mclone-web-client --target wasm32-unknown-unknown --manifest-path
native/Cargo.toml`, `pnpm native:web:build`, `pnpm native:web:bundle`, explicit staged/deploy
inventory checks (`mclone-integrated-server-worker.js`, `mclone-server-job-worker.js`, and
`mclone-runner-shared-abi.js` present, no `.ts`), `pnpm native:web:app-smoke`,
`pnpm native:web:chunk-smoke` plus final sha256, `pnpm native:web:mobile-smoke` with mobile canvas
inspection, `pnpm native:web:movement-perf`, `pnpm native:web:smoke`,
`pnpm native:movement:smoke`, and `pnpm native:timedemo:smoke`.

Implementation commit: `880c0e0d3609decdc55952bb14fd3687799c75bb`
(`071: graduate server worker glue to TypeScript`).

## Relationship to Other Tacticals

- [`070-web-glue-typing-and-abi-hardening.md`](070-web-glue-typing-and-abi-hardening.md) -
  established the wasm-bindgen `.d.ts` typecheck gate, the static-import cache-bust caveat, the
  app/touch/input/HUD split, and the byte-identical chunk-smoke fence this tactical preserves.
- [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md) -
  render-worker architecture parent; do not blur the Rust-owned render/compiler boundary while
  migrating JS source format.
- [`062-shared-threading-topology.md`](062-shared-threading-topology.md) - parent native-thread /
  Web Worker topology; worker URLs and module boundaries must remain compatible with it.
