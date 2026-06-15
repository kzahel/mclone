# Legacy TypeScript Engine Info

This document is only for explicit legacy TypeScript/browser engine work. The TypeScript implementation under `src/`, `test/browser/`, Playwright configs, and legacy tacticals under `docs/tactical/legacy/` are reference prior art for the native Rust rewrite, not the default implementation target.

Do not use this workflow for ordinary engine, renderer, input, movement, camera, worldgen, mesh, server, UI, or platform requests. Start from `AGENTS.md` and the native Rust workspace first.

Use this document when the user explicitly asks for legacy TypeScript/browser maintenance, when generating fixtures or behavior comparisons from the old engine, or when reading old orchestration context before applying a native Rust change.

## Legacy Porting Methodology

Every class ported in the legacy TypeScript engine has a 1:1 counterpart in `reference/minecraft-1.17.1/src/`. Always read the source file before writing the port. The default is direct translation: same field names, same method names, same logic flow. Diverge only when the browser, TypeScript, or WebGPU platform forces it.

### Sanctioned Legacy Divergences

| Layer | Java | TypeScript / WebGPU replacement |
|---|---|---|
| Renderer only | `GlStateManager` + `Uniform` | WebGPU pipeline descriptors + bind groups |
| Renderer only | GLSL core shaders (`assets/minecraft/shaders/core/`) | WGSL equivalents |
| Language | `ByteBuffer`, Java primitives, `int` bit-twiddling | `DataView` / typed arrays; preserve byte layout exactly |
| Language | Java generics / checked exceptions | TypeScript generics / unchecked throws |

Everything else - vertex formats, buffer builders, atlas stitching, model baking, chunk meshing, noise math, PRNG, biome layering - is a straight port. If you find yourself writing logic that is not in the source, stop and re-read the source.

## Legacy Architecture Review

Runtime architecture does not have to transliterate class-for-class from Minecraft, but architectural choices still need to be weighed against the reference source.

Before making an architectural divergence, explicitly determine:

1. What the reference Minecraft source does today
2. Why that shape is a poor fit for browser / WebGPU / worker / Node constraints
3. The exact scope of the divergence
4. Whether the divergence makes future parity work easier, neutral, or harder

Do not make architectural changes on the basis of convenience alone. Prefer the reference shape when it still fits. When diverging, keep the divergence intentional, narrow, and documented.

The main rule is:

- simulation/content parity is the default
- runtime orchestration may diverge when browser platform constraints require it
- any such divergence must preserve a clear path for future parity work instead of making it opaque or harder to recover

### Callout Convention

When a method diverges from its Java counterpart, add a one-line comment:

```ts
// WebGPU: GPUBuffer instead of GL VAO/VBO/IBO
```

Straight ports get no comment.

## Legacy Browser Validation

For any legacy TypeScript/browser slice that produces pixels, capture a screenshot and look at it before moving on. Do not finish a whole slice and then check. Check at the first drawable milestone - even a solid-color quad or a clear-color frame - then keep checking as complexity increases.

### Host Capability Check

Before choosing browser/WebGPU validation on an unfamiliar host, run `pnpm host:check`. The script reports whether the host has a display session, visible GPU devices, Chrome/Chromium, Playwright, and which validation lanes are expected to work.

- On a headless Linux host with no `DISPLAY` / `WAYLAND_DISPLAY`, Chrome/WebGPU Playwright lanes are expected to be unavailable. Do not treat failures from `pnpm test:browser`, `pnpm test:browser:integration`, or `pnpm probe:browser ...` as renderer regressions until they reproduce on a host with a working Chrome GPU/browser path.
- On this Linux Wayland host, the shell may have `WAYLAND_DISPLAY` unset even though `$XDG_RUNTIME_DIR/wayland-0` exists. For browser GPU validation here, run Playwright headed with an explicit Wayland display and a clean Vite port:
  - Browser smoke: `env VITE_PORT=5073 CI=1 WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland pnpm exec playwright test --config playwright.config.ts --headed`
  - Browser integration: `env VITE_PORT=5073 CI=1 WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland pnpm exec playwright test --config playwright.integration.config.ts --headed`
  - Browser probe: `env VITE_PORT=5073 CI=1 WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland pnpm exec playwright test --config playwright.probes.config.ts --headed test/browser/probes/<name>.probe.ts`
  - `CI=1` prevents Playwright from reusing an unrelated dev server, and `VITE_PORT=5073` avoids the user shell's `VITE_PORT=3402` override. Plain `pnpm test:browser` can reuse the wrong app on port 3402.
  - Headless Chrome on this host can fail WebGPU with `Instance dropped in popErrorScope` and black screenshots even when headed Wayland Chrome renders correctly. Prefer the headed Wayland command for browser-GPU validation; use Deno smokes as the headless WebGPU control.
- On macOS, keep browser screenshot probes on Playwright's Chrome channel and launch with `--enable-unsafe-webgpu --use-angle=metal`. Playwright's bundled Chromium can expose `navigator.gpu` and pass offscreen WebGPU readback while still screenshotting presented WebGPU canvases as black in headless mode unless `--use-angle=metal` is present. The shared Playwright config and smoke scripts include this default.
- On that kind of host, prefer display-independent validation: `pnpm test`, `pnpm typecheck`, Node headless host tests, and focused Deno WebGPU smokes such as `pnpm smoke:deno:webgpu`, `pnpm smoke:deno:pipeline`, `pnpm smoke:deno:world-assets`, or `pnpm smoke:deno:generated-world`.
- To run an actual Deno WebGPU capability probe through the checker, use `pnpm host:check -- --probe-deno-webgpu`. To confirm a Chrome/WebGPU browser path, use `pnpm host:check -- --probe-browser-webgpu`; the browser probe uses a localhost secure context and verifies both offscreen GPU readback and presented WebGPU canvas pixels in a screenshot, so it catches black-canvas headless failures.

Concretely:

- `pnpm test:browser` is the fast automatic WebGPU smoke lane. Run it for browser boot, WebGPU setup, remote-host smoke, and changes that could break the default browser entrypoint.
- `pnpm test:browser:integration` is the slower automatic browser integration lane. Run it when touching remote player state, debug camera controls, resize/backing-buffer logic, or chunk-interest movement.
- `pnpm probe:browser -- test/browser/probes/<name>.probe.ts` is the screenshot lane for human visual inspection. Run the smallest relevant probe when a change affects rendered pixels, terrain appearance, surface materials, vegetation, or camera framing. Do not run the full probe suite unless the user asks or the change broadly affects visual output.
- After wiring up a new draw path, run the smallest browser command that reaches the new path, capture a screenshot, read the image, and describe what you see. Does it look right? Are the shapes, colors, and positions what you expect?
- If the output looks wrong (blank canvas, wrong color, garbage geometry, WebGPU validation errors in the console), stop and fix it before adding more code on top.
- Unit tests (`pnpm test`) catch data-structure correctness. They do not catch GPU submission errors, wrong buffer layouts, or misconfigured pipelines. Actually looking at the rendered output is the only way to catch those.
- Always save debug, smoke, and probe screenshots to `/tmp` (for example `/tmp/mclone-debug-freecam.png`). Never write screenshots into the repo, into `test-results/`, or anywhere that risks getting committed.
