# 09a — Renderer browser harness

Standing before [`10-` blaze3d vertex layer + WebGPU bootstrap](README.md). This slice is **infrastructure only**: stand up a repeatable browser test harness so every renderer slice from `10` onward has a real-Chrome environment to load WebGPU code into and take screenshots from. It deliberately does **not** begin translating any renderer module — see [`AGENTS.md`](../../AGENTS.md).

## Goal

A `pnpm test:browser` workflow that:

- launches the user's installed **system Chrome** (not Playwright's bundled Chromium) via Playwright's `channel: "chrome"`, with an ephemeral temp profile per run
- serves TypeScript to the browser via a Vite dev server
- runs a WebGPU smoke test proving: adapter + device request succeeds, a canvas context configures, a clear pass produces the expected pixels
- leaves the existing Vitest (`pnpm test`) suite untouched

The harness has to exist before slice `10` because that slice's "done when" includes "a canvas that clears," and we cannot verify that claim in Node.

## Current status

- Tactical `09a` is complete.
- `vite`, `@playwright/test`, and `@webgpu/types` are installed as devDependencies.
- `pnpm dev:browser` serves TypeScript to the browser via Vite on `localhost:5173`.
- `pnpm test:browser` launches system Chrome (`channel: "chrome"`) with `--enable-unsafe-webgpu`, runs the WebGPU smoke, passes on macOS.
- `src/renderer/main.ts` is the browser entry point. Slice `10` will extend it rather than replace it.
- Vitest's `include` now excludes `test/browser/**`, so `pnpm test` and `pnpm test:browser` do not compete for the same files.

## Scope

| # | Module | Depends on | Oracle input → expected |
|---|---|---|---|
| 1 | Vite dev server | `vite`, `index.html`, `src/renderer/main.ts` | `pnpm dev:browser` serves `/` on `localhost:5173` with TS hot-reload |
| 2 | Browser entry point stub | item 1 | WebGPU adapter + device request, clear canvas to a known color, expose a `window.__mcloneReady` promise resolving with init result |
| 3 | Playwright config (system Chrome) | `@playwright/test`, item 1 | `playwright test` launches `channel: "chrome"` with `--enable-unsafe-webgpu`, ephemeral profile, serves via `webServer: "pnpm dev:browser"` |
| 4 | WebGPU smoke test | items 2 + 3 | assert `__mcloneReady` resolves `{ ok: true }`, adapter info is non-empty, preferred format is `bgra8unorm` or `rgba8unorm`, and no uncaught page errors fired |
| 5 | Script + gitignore plumbing | items 1–4 | `pnpm dev:browser`, `pnpm test:browser`; `dist/`, `test-results/`, `playwright-report/` ignored |

## Why browser-based

- WebGPU has no reliable Node execution target we can trust for parity work. Dawn's Node binding exists but the user will ship to a browser; testing through Dawn bypasses the WGSL pipeline cache, canvas-context semantics, and driver paths that actually matter.
- Playwright's bundled Chromium has historically lagged Chrome Stable on WebGPU. System Chrome Stable has shipped WebGPU on-by-default since M113 (April 2023), and `channel: "chrome"` takes advantage of whatever version the user has installed without our repo pinning a browser binary.
- Per-run ephemeral profile is Playwright's default — no `userDataDir` pinning, no interference with the user's personal Chrome profile.
- SSIM golden-screenshot comparison is **deferred to slice `14`**, where we will have a chunk mesh worth comparing. Slice `09a` only needs to *take* screenshots, not diff them.

## What lives where

```
index.html                         # canvas + <script type="module" src="/src/renderer/main.ts">
vite.config.ts                     # minimal: port 5173, strict port
playwright.config.ts               # channel: "chrome", webServer: pnpm dev:browser
src/
  renderer/
    main.ts                        # browser entry: requestAdapter → requestDevice → clear canvas
test/
  browser/
    smoke.test.ts                  # single WebGPU smoke, Playwright-driven
```

Browser code lives under `src/renderer/`. Node-only worldgen code stays under `src/worldgen/`. The two do not share a runtime today and shouldn't try to — slice `10`+ will figure out which shared utilities graduate to `src/common/`.

`test/browser/` is a sibling of the existing `test/` Vitest tree rather than a subfolder. Vitest's `include: ["test/**/*.test.ts"]` currently matches `test/browser/smoke.test.ts`, so we'll narrow the Vitest include to explicitly exclude `test/browser/` — the two runners must not compete for the same files.

## Tooling choices

- **`@playwright/test`** (the test runner), not Playwright-the-library. Fixtures, parallelism, reporting, artifacts are free.
- **`channel: "chrome"`**. Launches the user's installed Google Chrome. Alternatives considered and rejected:
  - *Bundled Chromium*: WebGPU support has lagged Chrome Stable.
  - *Puppeteer*: similar capability, smaller ecosystem around artifact comparison + fixture flows.
  - *Vitest browser mode*: couples Vitest's module graph to Playwright's, doubles config surface without new capability.
- **`--enable-unsafe-webgpu`**. Defensive: basic WebGPU is on-by-default in Chrome Stable, but experimental features (e.g. `shader-f16`, `timestamp-query`) that future slices may need live behind this flag.
- **Vite 5.x**. Canonical TS-to-browser dev server. No custom plugins.
- **No SSIM/pixelmatch**. Deferred to slice `14`.

## Smoke test design

The smoke test asserts four things:

1. No uncaught page errors fired during load (caught via Playwright's `pageerror` event).
2. `window.__mcloneReady` resolves `{ ok: true }`. The entry-point stub sets this promise from its `boot()`, so any step (canvas lookup, `navigator.gpu`, adapter, device, context) can short-circuit to `{ ok: false, reason }` and the test sees it as a concrete failure rather than a hang.
3. `result.adapterInfo` is non-empty — proves `adapter.info` returned real fields, which means a real driver bound, not a stub.
4. `result.format` is one of the expected canvas formats (`bgra8unorm` or `rgba8unorm`). Catches preferred-format regressions where the browser returns something unexpected.

Pixel-level validation (does the clear pass actually produce green pixels?) is **deferred to slice `14`**. Reading back WebGPU canvas contents reliably either needs a custom PNG decoder on the Node side or a `COPY_SRC`-capable canvas context with a readback buffer on the browser side — both carry cost that the smoke doesn't need to pay. Slice `14` will establish golden-screenshot infrastructure (baseline PNGs + SSIM) and pixel validation naturally lives there.

The clear pass still runs in `boot()` — not because the smoke checks pixels, but because it's the minimum surface-area proof that the render pipeline is wired end-to-end (encoder → pass → submit → queue flush). Clear color is `rgb(0, 128, 0)` so a future debugger opening `index.html` in a browser sees a green canvas and knows the harness is alive.

Screenshots of the page are captured as Playwright artifacts **only on test failure** via `trace: "retain-on-failure"`. `test-results/` is gitignored; passing runs leave no files behind.

## Translation gotchas

- **System Chrome must be installed.** On `darwin` this is near-universal; on Linux CI it requires an explicit install step. The smoke test will fail fast with Playwright's "Executable doesn't exist" message if Chrome is missing — clear enough for now. Revisit if CI lands.
- **Headless WebGPU has been unreliable historically.** Chrome's "new headless" (default in recent versions) runs a full Chrome off-screen and does support WebGPU on macOS. If the smoke test fails to acquire an adapter, set `headless: false` in `playwright.config.ts` as the first diagnostic.
- **Temp profile location is Playwright's decision.** Ephemeral by default under `~/Library/Caches/ms-playwright/` on darwin. Do **not** pass `userDataDir` — that switches Playwright into persistent-context mode, which is a different code path with different parallelism semantics.
- **WebGPU requires a secure context.** `localhost` counts; no TLS setup needed for dev. If we ever serve from an IP address, this changes.
- **Vitest and Playwright must not share test files.** Vitest's `include` currently globs `test/**/*.test.ts`. Narrow it to exclude `test/browser/` so Vitest doesn't try to import DOM-dependent code in Node.
- **`navigator.gpu.getPreferredCanvasFormat()` may return `bgra8unorm` or `rgba8unorm`** depending on platform. Never hardcode a format — always ask the API.
- **Command submission is async.** `device.queue.submit()` returns immediately; the clear may not be visible to `drawImage` until the next frame. Await `device.queue.onSubmittedWorkDone()` before resolving `__mcloneReady`.
- **Playwright browser-install noise.** `pnpm add -D @playwright/test` will *not* auto-download Chromium/Firefox/WebKit unless `npx playwright install` is run. Since we only use system Chrome, skip the install step. Document this in the slice for anyone who expects the usual `npx playwright install` step.
- **`channel: "chrome"` uses stable, not Canary.** If a future slice needs a specific Chrome-version feature, we may need to pin to `chrome-beta` or `chrome-canary` — the channel string is the only change needed.

## Concrete steps

1. `pnpm add -D vite @playwright/test`. Do **not** run `npx playwright install` — we use system Chrome via `channel: "chrome"`.
2. `vite.config.ts`: minimal — `server: { port: 5173, strictPort: true }`, no plugins.
3. `index.html` at repo root: `<canvas id="renderer" width="800" height="600">` + `<script type="module" src="/src/renderer/main.ts">`.
4. `src/renderer/main.ts`: `boot()` async that gets `navigator.gpu`, requests adapter + device, configures context with preferred format, submits a clear pass to `rgb(0, 128, 0)`, awaits `queue.onSubmittedWorkDone()`, returns result. Exposes `window.__mcloneReady`.
5. `playwright.config.ts`: `testDir: "test/browser"`, `use: { channel: "chrome", baseURL: "http://localhost:5173", launchOptions: { args: ["--enable-unsafe-webgpu"] } }`, `webServer: { command: "pnpm dev:browser", port: 5173, reuseExistingServer: !process.env.CI }`.
6. `test/browser/smoke.test.ts`: single `test()` that awaits `__mcloneReady`, asserts `result.ok`, non-empty `adapterInfo`, canonical `format`, and empty `pageerror` stream.
7. `package.json`: add `dev:browser` (`vite`), `test:browser` (`playwright test`).
8. `vitest.config.ts`: narrow `include` to exclude `test/browser/`.
9. `.gitignore`: add `dist/` (already present), `test-results/`, `playwright-report/`.
10. Run `pnpm test:browser` and confirm it passes. If it fails on adapter acquisition, flip `headless: false` to confirm the WebGPU stack works, then diagnose headless.
11. `docs/tactical/README.md`: add row for `09a-` in the renderer arc table (between `09` and `10`), note that slices `10`+ depend on this harness.

## Done when

- `pnpm test:browser` launches system Chrome, runs the WebGPU smoke test, passes on macOS.
- `pnpm test` (existing Vitest suite) stays green and does not try to pick up `test/browser/smoke.test.ts`.
- `pnpm typecheck` stays green with `src/renderer/main.ts` and `test/browser/smoke.test.ts` under its include globs.
- `docs/tactical/README.md` references `09a-` from the renderer arc table.
- A follow-up slice (`10`) can extend `src/renderer/main.ts` without the harness getting in its way.

## Out of scope for this doc

- SSIM or pixelmatch golden-screenshot diffs — deferred to slice `14` when we have a meshed chunk to diff.
- CI integration — harness must work locally first; GitHub Actions WebGPU support adds Xvfb/Vulkan-driver complexity that buys nothing until there's something worth gating on.
- Cross-browser coverage — Firefox/Safari WebGPU lags Chrome; add only when we ship.
- Hot-reload / HMR integration tests — Vite's defaults are fine.
- Asset-serving plumbing (textures, models) — that's slice `12` + `13`.
- Any actual renderer code beyond what proves the harness works — slice `10` picks that up.
