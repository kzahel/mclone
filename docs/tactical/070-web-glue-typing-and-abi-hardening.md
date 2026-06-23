# 070: Web JS Glue Hardening — ABI Lock, Type-Checking, and Shrink

Status: **Stages 1–2 landed (server-worker SAB ABI single-source + lock test; no-emit type-check gate over the web glue); Stage 3 in progress (packed-frame codec moved into Rust; compile-timing instrumentation next).**

The web perf/refactor work across 064–069 grew the hand-written browser JS glue.
This is a consolidation/hardening pass over that glue. The framing matters: the
Rust↔JS boundary is **already well-drawn** — Rust owns the engine and the hot path
(WebGPU surface + render pass `web_canvas.rs`, `wgpu` device/queue, WebSocket frame
I/O `web_remote_session.rs`, and the `SharedArrayBuffer`+`Atomics` ring buffers and
worker polling `web_canvas.rs:1283-1451`), while JS owns the browser shell (module
load, `requestAnimationFrame`, canvas mount/resize, pointer-lock, mobile touch,
`localStorage`, menu/HUD DOM, `new Worker(...)`). That division is correct; this doc
is **not** about moving the engine into Rust (it already is). It is three things:

1. close an **unguarded ABI duplication** that mirrors a bug 067 Stage 5 already
   fixed for the render lane;
2. add a **no-emit type-check gate** over the ~4k lines of glue at the two
   boundaries where it silently breaks;
3. a few **targeted glue-shrink** moves.

Stable `wasm32-unknown-unknown`, no nightly/`build-std`, and the deploy pipeline
(raw `cp` of `www/`) stays unchanged. 062/067/069 lineage.

## Current state (measured)

Hand-written browser JS shipped **verbatim** (deploy is `cp "$WWW_DIR"/. "$DEPLOY_DIR"`
in `scripts/deploy-native-web.sh` — no bundle, transpile, or minify; `pkg/mclone_web_client.js`
is the only generated glue, from `wasm-bindgen --target web`):

| File | Lines | Role |
|---|---|---|
| `www/mclone-web-app.js` | 1930 | main app: loader, rAF loop, input, touch, menu/HUD, compile-timing instrumentation |
| `www/mclone-web-smoke.js` | 706 | smoke page logic |
| `www/mclone-render-compiler-shared.js` | 513 | render-compile consumer glue |
| `www/mclone-integrated-server-worker.js` | 376 | integrated-server worker |
| `www/mclone-render-compiler-worker.js` | 348 | render-compile worker (producer) |
| `www/mclone-server-job-worker.js` | 129 | stateless worldgen/light job worker |
| `www/mclone-thread-smoke-worker.js` | 38 | atomics smoke |
| `www/mclone-render-compiler-abi.js` | 33 | **single-source render-compile SAB ABI** (locked to Rust) |
| `scripts/browser-smoke.mjs` | 2079 | Node-side smoke harness (not shipped) |

**No type-checking covers any of it.** The root `tsconfig.json` `include` is
`src/**`, `test/**`, oracle integration, and the vite/vitest/playwright configs —
the legacy TypeScript engine only. The `package.json` `"typecheck": "tsc --noEmit"`
script (line 69) exists but never sees `native/apps/mclone-web-client/www`.

## Problem 1 (highest priority): the server-worker SAB ABI is duplicated and unguarded

067 Stage 5 already solved this exact hazard for the **render** lane:
`www/mclone-render-compiler-abi.js` is the single JS source (imported by the
producer worker and the consumer glue), the Rust reader keeps its own copy in
`web_canvas.rs`, and `tests/render_compiler_abi_lock.rs` parses the numeric
constants out of **both** files and fails the build on any drift — so a wrong
status-word index is a failing test instead of silent SAB corruption.

The **server-worker** SAB ring never got that treatment. The same control-word
layout is hand-synced across three places, with diverging names and **no lock**:

| Location | Prefix | Notes |
|---|---|---|
| `web_server_worker.rs:28-37` | `RUNNER_SHARED_*` | Rust reader; `CONTROL_SLOTS=4` (`×4 = CONTROL_BYTES`), `STATUS_PENDING=1`, `STATUS_COMPLETE=2` |
| `www/mclone-integrated-server-worker.js:9-16` | `RUNNER_SHARED_*` | `CONTROL_BYTES=16`, `STATUS_COMPLETE=2`, `STATUS_FAILED=-1` |
| `www/mclone-server-job-worker.js:10-14` | `SHARED_*` | renamed prefix; `STATUS_COMPLETE=2`, `STATUS_FAILED=-1` |

A concrete divergence already lives in this duplication: the **status enum is
asymmetric** — Rust defines `STATUS_PENDING=1`/`STATUS_COMPLETE=2` but no `_FAILED`;
both JS workers define `STATUS_FAILED=-1`/`STATUS_COMPLETE=2` but no `_PENDING`; and
control size is expressed as `CONTROL_BYTES=16` on the JS side vs `CONTROL_SLOTS=4 (×4)`
on the Rust side. Nothing enforces these agree. A wrong index or status code here
does not throw — it makes a server job silently never complete or decode garbage, in
a worker, in the browser. This is a latent bug, not a style nit.

## Problem 2: the two glue boundaries are stringly-typed and fail silently

- **wasm-return objects.** Every call into Rust returns an untyped JS object,
  checked with `frame?.ok` / `camera?.ok` / `interaction?.ok` / `report?.ok`
  (`mclone-web-app.js:398,513,563,577,607,624,769,1867,1910`) and defensively
  coerced — **103 `Number(...)` coercions** in that one file. Rename a field on the
  Rust side and every one of these goes quietly falsy; the feature stops working with
  no error and no stack.
- **postMessage payloads.** Stringly-typed `kind` tags plus manual byte layouts
  cross every worker boundary, with no schema and no versioning.

These are exactly what a type-check gate catches, and exactly where this glue is
weakest. They are also the spots most exercised by the recent 064–069 churn.

## Problem 3: shrinkable duplication / smells

- **Duplicated codec.** Packed-frame encode/decode and a hand-rolled `writeU32Le`
  little-endian writer are copy-pasted across both server workers
  (`mclone-integrated-server-worker.js`, `mclone-server-job-worker.js`). Rust already
  owns this codec.
- **Instrumentation in JS.** The compile-timing instrumentation in
  `mclone-web-app.js` (`createCompileTiming` / `pendingTimings` and a large share of
  the 103 coercions) is data-munging that Rust can own — it already produces the
  packed report summary via `mclone_web_packed_compile_report_summary`.
- **Smells.** `waitForSessionIdle()` busy-polls up to **10,000×** via `setTimeout(0)`
  (`mclone-web-app.js:642-646`), with `selectHotbarSlot`/`adjustCameraSpeed`
  self-rescheduling when busy; `globalThis.__mcloneWebApp` global mutable state
  (`:114`); and `mclone-web-app.js` at 1930 lines is loader + input + touch + menu +
  HUD + instrumentation in one file.

## Constraints (do not violate)

- **Deploy stays a raw `cp`** plus the `wasm-bindgen --target web` step — no bundler,
  no transpile. Type-checking is `tsc --noEmit` / `checkJs` **only**; nothing changes
  what ships. (The one option that *would* touch the pipeline — authoring `.ts` with an
  emit step — is explicitly deferred to follow-ups.)
- **Stable toolchain.** `wasm32-unknown-unknown`, no nightly / `build-std` (same
  posture as 068/069).
- **Desktop/native untouched.** All work is web-glue + `mclone-web-client` Rust,
  confined behind the existing web backends.
- Server workers are `WorkerType::Module` (`web_server_worker.rs:151-154`), so a
  static ES `import` of a shared ABI module works — same mechanism the render lane
  already uses.

## Staged implementation (land + validate + commit each)

### Stage 1 — server-worker SAB ABI single-source + lock test (highest priority, self-contained)

Copy the 067 Stage 5 pattern onto the server-worker lane.

- New `www/mclone-runner-shared-abi.js` single source, modeled on
  `www/mclone-render-compiler-abi.js` (carry over its **deploy cache-bust caveat
  comment** — a bare static `import` URL cannot carry `?v=<version>`).
- Reconcile both workers to import it: `mclone-integrated-server-worker.js`
  (`RUNNER_SHARED_*`) and `mclone-server-job-worker.js` (rename its `SHARED_*` to the
  shared names). **Normalize the status enum so `PENDING`/`COMPLETE`/`FAILED` all
  exist on both Rust and JS sides** so the lock can cover the full set.
- New `tests/runner_shared_abi_lock.rs` mirroring `render_compiler_abi_lock.rs`,
  locking the JS constants to `web_server_worker.rs`. **Parser caveat:** 067's
  `const_value` evaluates integer `A * B * C` products and plain ints but **not const
  *references*** — `RUNNER_SHARED_CONTROL_BYTES = RUNNER_SHARED_CONTROL_SLOTS * 4`
  will trip it. Simplest fix: express the locked value on the Rust side as an integer
  literal/product (e.g. lock `CONTROL_BYTES = 16` directly), or extend the parser to
  resolve the slots constant. Inlining is the path of least resistance.
- **Validate:** `cargo test` (new lock test + existing), `node --check` on both
  workers, `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
  (warning-clean), `pnpm native:web:app-smoke`, `pnpm native:web:chunk-smoke`. **Commit.**

### Stage 2 — type-check gate over the web glue (no emit, no pipeline change)

- **Type the Rust exports for free:** add `--typescript` to the `wasm-bindgen`
  invocation (`scripts/deploy-native-web.sh`, and the `native:web:build` path) so
  `mclone_web_client.d.ts` is emitted. This is the single biggest win — it makes the
  103-coercion wasm boundary checkable.
- New `native/apps/mclone-web-client/tsconfig.json`: `allowJs`, `checkJs`, `noEmit`,
  `strict`, `lib: ["ES2022","DOM","DOM.Iterable","WebWorker"]`,
  `types: ["@webgpu/types"]`, `include` `www/**/*.js` + `scripts/*.mjs`, referencing
  the generated `.d.ts`.
- Add JSDoc `@typedef`s for the postMessage payload shapes (one per worker boundary,
  plus the SAB doorbell objects) and annotate the wasm-return call sites against the
  `.d.ts`.
- New `native:web:typecheck` script (`tsc --noEmit -p native/apps/mclone-web-client/tsconfig.json`);
  add it to the web validation lanes (optionally also fold into root `typecheck`).
- **Iterate until clean.** This will surface real latent issues (the silent `?.ok`
  accesses, payload-shape mismatches) — fix them as found. This is the largest chunk;
  splitting the commit (scaffold + `.d.ts` + first files clean, then the rest) is fine.
  **Commit.**

### Stage 3 — shrink the glue (move duplicated codec + instrumentation into Rust)

- Move packed-frame encode/decode + `writeU32Le` into Rust, expose via
  `#[wasm_bindgen]`, and delete the JS copies in both server workers.
- Move the compile-timing instrumentation out of `mclone-web-app.js` into Rust (it
  already produces the packed report summary), shrinking app.js and removing coercion
  noise.
- **Correctness fence:** the deterministic chunk-smoke canvas PNG must be
  **byte-identical** (sha256) to pre-change — these are transport/diagnostics changes,
  not generation — plus `pnpm native:web:movement-perf` no-regression. **Commit.**

## Landed

### Stage 1 — server-worker SAB ABI single-source + lock test (landed)

The server-worker `SharedArrayBuffer` ring control-word ABI — formerly hand-synced
across three places with diverging names and no guard (Problem 1) — is now
single-sourced on the JS side and locked to the Rust reader by a host test, exactly as
067 Stage 5 did for the render lane. This closes the latent bug: a wrong status code or
byte-index here does not throw, it makes a server job silently never complete or decode
garbage, in a worker, in the browser.

- **Single JS source.** New `www/mclone-runner-shared-abi.js` exports the seven
  control-word constants (`RUNNER_SHARED_CONTROL_BYTES`, the three byte-indices, and the
  `STATUS_PENDING`/`COMPLETE`/`FAILED` enum), modeled on `mclone-render-compiler-abi.js`
  and carrying its deploy cache-bust caveat comment (a static `import` URL cannot carry
  `?v=<version>`, so a version bump relies on HTTP revalidation of the bare URL).
- **Both workers import it.** `www/mclone-integrated-server-worker.js` dropped its local
  `RUNNER_SHARED_*` block (keeping only the worker-local pool-tuning knobs
  `MAX_RUNNER_SHARED_POOL_SLOTS` / `DEFAULT_RUNNER_SHARED_RESPONSE_BYTES`, which are not
  part of the cross-boundary ABI), and `www/mclone-server-job-worker.js` had its
  divergent `SHARED_*` prefix renamed to the shared `RUNNER_SHARED_*` names. Both workers
  are `WorkerType::Module` (`web_server_worker.rs:152`,
  `mclone-server/src/wasm_job_worker.rs:77`), so a static ES `import` works.
- **Status enum normalized.** The enum was asymmetric — Rust had `PENDING=1`/`COMPLETE=2`
  but no `FAILED`; both JS workers had `COMPLETE=2`/`FAILED=-1` but no `PENDING`. Now all
  three exist on both sides so the lock covers the full set. The new Rust
  `RUNNER_SHARED_STATUS_FAILED` is used, not dead code: `shared_runner_response_frames`
  now reports a precise "worker reported a failure status" error on a FAILED control word
  instead of the generic "unexpected status -1" (`web_server_worker.rs`).
- **Lock test.** New `tests/runner_shared_abi_lock.rs` mirrors
  `render_compiler_abi_lock.rs`: it `include_str!`s both `src/web_server_worker.rs` and
  `www/mclone-runner-shared-abi.js`, parses the seven numeric `RUNNER_SHARED_*` constants
  out of each, and asserts they agree — so any drift is a failing `cargo test` instead of
  a silent SAB corruption. Verified non-vacuous (corrupting one JS value fails it with a
  precise drift message). The worker-local pool knobs are intentionally **not** locked
  (each side sizes its own buffer pool independently).

**Divergence / parser caveat.** 067's `const_value` parser evaluates integer literals and
`A * B` products but **not** const references, so the Rust `RUNNER_SHARED_CONTROL_BYTES` —
formerly the derived `RUNNER_SHARED_CONTROL_SLOTS * 4` — is now the inline literal `16`
(commented as 4 i32 control words × 4 bytes), and the now-unused
`RUNNER_SHARED_CONTROL_SLOTS` was removed (`web_server_worker.rs`). This is the doc's
recommended "inline" path; the parser is reused verbatim from the render lock test rather
than extended.

**Validation (all green):** `cargo test --manifest-path native/Cargo.toml` (full
workspace; the new `runner_shared_abi_lock` and existing `render_compiler_abi_lock` both
pass, no failures, no warnings), `cargo check -p mclone-web-client --target
wasm32-unknown-unknown` (warning-clean — `STATUS_FAILED` is now referenced), `node --check`
on the new ABI module + both touched workers, `pnpm native:web:build`,
`pnpm native:web:app-smoke` (integrated-server worker + worldgen/light job workers over
`shared-memory`, `failed: false`, zero fallback-response frames),
`pnpm native:web:chunk-smoke` (9 chunks loaded, terrain canvas inspected at
`/tmp/mclone-native-web-canvas.png` — grass/ore/lava column renders correctly),
`pnpm native:movement:smoke`, `pnpm native:timedemo:smoke` (desktop unaffected — the
server-worker ABI is wasm-only, cfg-gated out of the desktop build).

### Stage 2 — type-check gate over the web glue (landed)

A no-emit `tsc --checkJs`/`strict` gate now covers all of the hand-written web glue — every
`www/*.js` file plus the Node/Playwright harness `scripts/browser-smoke.mjs` — driven by a new
`pnpm native:web:typecheck`. Nothing about what ships changed: the deploy stays a raw `cp` of
`www/` plus the `wasm-bindgen` step, and the gate is `tsc --noEmit` only (Problem 2).

- **The Rust exports are typed for free.** `wasm-bindgen` now runs with `--typescript` in both
  invocations — `scripts/deploy-native-web.sh` and `browser-smoke.mjs`'s `buildBindgenBundle()` —
  so it emits `mclone_web_client.d.ts` next to the JS glue. This is the doc's "single biggest win":
  casting the dynamic `import()` of the bindgen module to `typeof import("mclone-web-client-wasm")`
  in `mclone-web-app.js`/`mclone-web-smoke.js`/the three worker producers makes the **live wasm
  call sites checkable against the real export signatures** — a renamed export or a wrong arg
  count (e.g. the 14-arg `advanceCameraFrame`) is now a `tsc` error, and `this.session` is typed
  `WebChunkRenderSession` (`mclone-web-app.js`).
- **New `native/apps/mclone-web-client/tsconfig.json`.** `allowJs`/`checkJs`/`noEmit`/`strict`,
  `lib: ["ES2022","DOM","DOM.Iterable","WebWorker"]`, `include` `www/**/*.js` + `scripts/*.mjs`. The
  bindgen `.d.ts` lives in the gitignored cargo target dir (a build artifact, never committed,
  never `cp`'d into the deploy bundle), so it is reached via a `paths` alias `mclone-web-client-wasm`
  → `../../target/.../mclone-web-client-bindgen/mclone_web_client` rather than the runtime
  `./pkg/...` URL (which does not exist on disk at type-check time). That bare specifier appears only
  inside JSDoc `import(...)` type positions, so it carries no runtime weight.
- **New `pnpm native:web:typecheck`.** `node browser-smoke.mjs --build-only && tsc --noEmit -p …`
  — a new `--build-only` flag on the harness compiles the wasm + runs `wasm-bindgen --typescript`
  and exits before launching a browser, so the gate cheaply materializes the path-mapped `.d.ts`
  before `tsc`. Added to the web validation lanes (kept separate from the legacy-TS root
  `typecheck`, which has no reason to build the wasm `.d.ts`).
- **JSDoc `@typedef`s at both stringly-typed boundaries.** Per-worker inbound postMessage payloads
  (`ServerJobWorkerMessage`, `IntegratedServerWorkerMessage`, `RenderCompileWorkerInbound`) and the
  consumer-side request/report/doorbell shapes (`RenderCompileDoorbell`, the SAB
  `RenderCompileSharedResultArena`/`…InputArena` doorbell objects, `RenderCompileWorkerRequest`) are
  authored as typedefs and the handler params annotated against them; the dynamic-import module is a
  shared `WasmModule` typedef in each file.

**Divergences / findings.**

- **`types: ["@webgpu/types", "node"]`, not just `["@webgpu/types"]`** as the doc sketched. One
  tsconfig spans browser-DOM glue *and* a Node harness, so the harness's `process`/`Buffer`/PNG
  decode needs the Node globals; `skipLibCheck` suppresses the benign DOM-vs-Node lib duplicate-global
  conflicts. The only browser-side friction this introduced was `setInterval` resolving to the Node
  overload, handled by typing the one timer field as `ReturnType<typeof setInterval>`
  (`mclone-integrated-server-worker.js`).
- **The wasm-return objects are typed as named permissive bags (`Record<string, any>`), not exact
  field lists.** `wasm-bindgen` types every `WebChunkRenderSession` method return as `any` in the
  `.d.ts` (it cannot describe the serde shape), so the doc's hoped-for "rename a field → `tsc`
  error" is **not achievable at the field level** through this boundary — that would need typed
  `#[wasm_bindgen]` getters (out of scope). The realized value is the *method/arg* boundary (above),
  internal-consistency checking within app.js, and the now-typed postMessage payloads. The named
  bags (`WasmReport`, `CompileTiming`) document the boundary and keep the `Number(...)`/`?.ok`
  coercion sites honest.
- **The app's global type leaks to the harness — usefully.** `globalThis.__mcloneWebApp = runtime`
  makes checkJs treat `__mcloneWebApp` as a global of type `AppRuntime`, so `browser-smoke.mjs`'s
  `page.evaluate` callbacks are now checked against the real runtime contract. The runtime's
  input/menu methods are installed by `boot()` (so they are optional on `AppRuntime`); the harness
  calls them only post-boot, so those sites use `?.()` (`mclone-web-app.js` exposes them;
  `browser-smoke.mjs` consumes them).
- **What the gate actually surfaced.** No live "silently falsy `?.ok`" bug (the `any` boundary
  precludes catching those statically — see above), but it forced explicit handling of real latent
  fragilities: DOM accesses that assumed non-null/HTML element types (`getElementById(...).value`
  needing `HTMLInputElement`, `querySelectorAll` → `Element` lacking `.dataset`,
  `getElementById("mclone-canvas")` possibly null), `server.address()`'s `string | AddressInfo | null`
  union read as `.port`, an `error?.code` access on an `unknown` catch binding in the dev server, and
  two SAB-path values (`message.sharedResultResponseBuffer`, `requestMessage.controlBuffer`) that the
  code already validates at runtime but TS could not see. All fixes are type-only — every removed line
  is an in-place cast/non-null-capture/annotation; no runtime logic changed (verified by diff).

**Validation (all green):** `pnpm native:web:typecheck` (0 errors across all 7 glue files),
`cargo test --manifest-path native/Cargo.toml` (full workspace, no failures, no warnings —
Stage 2 touched no Rust), `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
(warning-clean), `node --check` on all six touched `www/*.js` + the harness,
`pnpm native:web:build`, `pnpm native:web:app-smoke` + `native:web:chunk-smoke` +
`native:web:mobile-smoke` (booted, streamed, `appliedOk: true`, zero fallback frames),
`pnpm native:movement:smoke` + `native:timedemo:smoke` (desktop unaffected).

### Stage 3 — shrink the glue (in progress)

Two duplicated/coercion-heavy pieces of JS glue move into the `mclone-web-client` Rust crate
(Problem 3). These are transport/diagnostics changes, not generation, so the deterministic
chunk-smoke canvas stays **byte-identical** (sha256 fence) and movement-perf holds.

**Packed-frame codec → Rust (landed).** The runner-update packing codec — a `writeU32Le`
little-endian writer plus `packedUpdateByteLength` / `writePackedUpdates` — was hand-rolled in
`mclone-integrated-server-worker.js` and mirrored the Rust *decode* side
(`unpack_runner_update_frames` in `web_server_worker.rs`). The encode side now lives next to the
decode side in Rust, exported via wasm-bindgen as `mcloneWebPackedRunnerUpdateByteLength` and
`mcloneWebWritePackedRunnerUpdates` (`web_server_worker.rs:1343-1397`), so the SAB packing format
has a single owner. The worker captures the resolved module in `startServer`
(`mclone-integrated-server-worker.js`) and reaches the codec through a `requireWasmModule()`
guard; `postSharedUpdates` now sizes the SAB from the Rust byte-length function and lets Rust
pack the frames in, while JS still arms the doorbell (response byte count + status word). The
three JS codec functions (~45 lines) are deleted — the worker shrinks from 376 to ~348 lines.

- **Scope correction vs the doc.** The doc said the codec was "copy-pasted across both server
  workers." It is not: only `mclone-integrated-server-worker.js` packs *multiple* update frames
  into one SAB. `mclone-server-job-worker.js` writes a *single* response frame
  (`new Uint8Array(responseBuffer, 0, n).set(response)`) and never had `writeU32Le` or the
  packing helpers — verified by grep. So only the integrated-server worker changed.
- **Faithful port.** The Rust encoder copies each `Uint8Array` frame into the SAB view in one
  `TypedArray.set` (the same single copy the JS did), with the u32 headers written via
  `set_index`, so it carries no extra per-frame allocation and the perf fence holds. The
  decode-side trailing-byte check (`cursor == packed.len()`) still passes because the encoder
  writes exactly `packedBytes`.

**Validation (codec, all green):** baseline chunk-smoke canvas sha256
`0ba8251f…cbcb6a4c` captured from the committed state pre-change; after the codec move,
`pnpm native:web:chunk-smoke` reproduces it **byte-identical** (same sha256). `cargo test
--manifest-path native/Cargo.toml` (full workspace, no failures — both ABI lock tests pass),
`cargo check -p mclone-web-client --target wasm32-unknown-unknown` (warning-clean — the new
`#[wasm_bindgen]` fns are referenced), `pnpm native:web:typecheck` (0 errors; the `.d.ts` now
types `mcloneWebPackedRunnerUpdateByteLength` / `mcloneWebWritePackedRunnerUpdates` and the
worker call sites check against them), `node --check` on the worker, `pnpm native:web:build`,
`pnpm native:web:app-smoke` (`failed: false`, zero fallback frames) + `native:web:chunk-smoke`,
`pnpm native:web:movement-perf` (9 compiles, totalMs avg ~10.6 ms / workerRoundTrip avg ~6.4 ms,
no regression), `pnpm native:movement:smoke` + `native:timedemo:smoke` (desktop unaffected — the
codec is wasm-only).

## Acceptance & validation (every stage)

- `cargo test --manifest-path native/Cargo.toml` green (incl. the new lock test).
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown` warning-clean.
- `node --check` on touched `www/*.js`; `pnpm native:web:typecheck` clean (Stage 2 on).
- `pnpm native:web:build`, `pnpm native:web:app-smoke`, `pnpm native:web:chunk-smoke`;
  `pnpm native:web:mobile-smoke` if touch paths are touched.
- Stage 3: chunk-smoke canvas PNG sha256 byte-identical to pre-change; save probe PNGs
  to `/tmp` (never into the repo); `pnpm native:web:movement-perf` no regression.
- Desktop unaffected: `pnpm native:movement:smoke`, `pnpm native:timedemo:smoke`.

## Follow-ups (lower priority, separate work)

- **Split `mclone-web-app.js`** (1930 lines) into loader / input / touch / hud modules.
- **Replace `waitForSessionIdle` busy-poll** with a promise/signal from the session.
- **Optional minimal eslint** (typescript-eslint recommended set) if `tsc` strictness
  proves insufficient — quality polish, not essential.
- **Optional: graduate `mclone-web-app.js` to real `.ts`** with a `tsc`/`esbuild` emit
  step. This is the one change that touches the deploy pipeline (currently raw `cp`),
  so it is explicitly deferred and should be taken only if JSDoc DX hurts.

## Relationship to other tacticals

- [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md)
  — Stage 5's render-compile ABI single-source (`mclone-render-compiler-abi.js`) +
  `tests/render_compiler_abi_lock.rs` is the exact template Stage 1 copies to the
  server-worker lane.
- [`062-shared-threading-topology.md`](062-shared-threading-topology.md) — parent
  threading topology; the integrated-server / worldgen / light lanes whose SAB ABI
  Stage 1 hardens.
- [`069-web-worldgen-lane-payload-reduction.md`](069-web-worldgen-lane-payload-reduction.md)
  / [`068-web-zero-copy-worker-lane-investigation.md`](068-web-zero-copy-worker-lane-investigation.md)
  — recent web payload-reduction work that grew this glue; this is the hardening pass
  over it.
- [`064-native-web-mobile-controls-and-hud.md`](064-native-web-mobile-controls-and-hud.md)
  / [`065-native-web-mobile-streaming-performance.md`](065-native-web-mobile-streaming-performance.md)
  — added much of the `mclone-web-app.js` touch + HUD + compile-timing glue this typing
  pass would cover.
