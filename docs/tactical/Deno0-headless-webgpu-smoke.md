# Deno0: Headless WebGPU Smoke

Status: done.

## Goal

Promote the Deno/WebGPU proof into a repo-owned smoke lane that renders to an offscreen GPU texture, reads pixels back, and writes a PNG under `/tmp` without Chrome, Playwright, Vite, DOM, or an HTML canvas.

This is a renderer-validation lane, not a native desktop rewrite. Chrome remains the interactive browser validation target until Deno can exercise real renderer paths.

## Scope

Add:

| # | Module | Expected result |
|---|---|---|
| 1 | `src/renderer/webgpu-target.ts` | done - shared WebGPU adapter/device setup plus browser-canvas and offscreen-texture target helpers |
| 2 | `scripts/deno-webgpu-smoke.ts` | done - Deno script that clears an offscreen `GPUTexture`, reads RGBA pixels, asserts a known pixel, and writes `/tmp/mclone-deno-webgpu-smoke.png` |
| 3 | `package.json` | done - pinned `pnpm smoke:deno:webgpu` command |
| 4 | `docs/deno-wgpu-native-spike.md` | done - durable sketch and observed Deno/WebGPU smoke result |
| 5 | `docs/native-target.md` / tactical README | done - link the official lane from the native and tactical docs |

Do not add:

- DOM emulation
- Playwright or Chrome dependencies
- asset atlas loading outside the browser
- chunk/world rendering in Deno
- Rust/wgpu or OpenXR host code

Those belong in follow-up tacticals if this smoke lane stays useful.

## Command

```bash
pnpm smoke:deno:webgpu
```

The package script uses:

```bash
npx -y deno@2.7.13 run --unstable-webgpu --allow-write=/tmp ./scripts/deno-webgpu-smoke.ts
```

Direct Deno is also valid when installed locally:

```bash
deno run --unstable-webgpu --allow-write=/tmp ./scripts/deno-webgpu-smoke.ts
```

Expected artifact:

```text
/tmp/mclone-deno-webgpu-smoke.png
```

Expected first pixel:

```text
[26, 102, 204, 255]
```

## Validation

Completed:

- `pnpm typecheck`
- `pnpm smoke:deno:webgpu` - passed after granting network access for the pinned Deno binary fetch through `npx`
- `pnpm test:browser` - passed after granting local-port access for the Vite web server
- inspected `/tmp/mclone-deno-webgpu-smoke.png`: solid blue 64x64 RGBA PNG, matching `[26, 102, 204, 255]`

`pnpm test:browser` remains required because this slice changes browser WebGPU setup plumbing.

## Done When

- Done: a repo-owned command renders a valid PNG through Deno WebGPU without Chrome.
- Done: the smoke validates readback pixels and fails on mismatch.
- Done: shared renderer target helpers are used by the browser setup path without changing browser behavior.
- Done: the tactical docs explain that Deno is a headless GPU lane, not a replacement for browser probes yet.

## Follow-Up

Next likely tactical: `Deno1` should import a small real renderer pipeline/shader path and draw known geometry through the offscreen target. It should still avoid DOM, browser asset decode, world/chunk rendering, and OpenXR.
