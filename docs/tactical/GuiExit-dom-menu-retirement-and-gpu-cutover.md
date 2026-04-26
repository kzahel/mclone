# GuiExit: DOM Menu Retirement and GPU Cutover

Status: active cutover target.

Durable architecture: [`../gui.md`](../gui.md). Foundation tactical: [`Gui0-webgpu-gui-foundation-and-dom-replacement.md`](Gui0-webgpu-gui-foundation-and-dom-replacement.md).

## Goal

Make WebGPU GUI the only visible game UI path. HTML remains only a platform shell for canvas, script loading, and browser APIs. All menus, loading/error status, debug/settings controls, and in-game overlays render through the vanilla-shaped GUI model.

## Exit Condition

This tactical is done only when all of the following are true:

- `/` and `/index.html` render a WebGPU `TitleScreen` directly in the canvas.
- `/smoke.html` remains a canvas-only test harness with no visible DOM controls.
- `/debug.html` is either removed, redirected to the same canvas shell, or reduced to a canvas-only debug entrypoint.
- No visible DOM menu/control/status nodes remain in game entry HTML: no `<button>`, `<form>`, `<input>`, `<select>`, `<textarea>`, visible `<details>`, debug overlays, CSS menus, CSS joystick, or HTML progress/status bars.
- `src/renderer/debug/debug-free-cam.ts` no longer mutates visible DOM for loading, error, config, performance, or progress UI.
- `src/renderer/debug/debug-input.ts` no longer creates or reads visible DOM joystick/fly buttons. Touch input is either removed or rendered as GPU widgets.
- `TitleScreen`, `WorldSetupScreen`, `ProgressScreen`, `ErrorScreen`, `PauseScreen`, `OptionsScreen`, `DebugSettingsScreen`, and required confirm/quit screens are GPU screens.
- Existing machine hooks remain available for automation: `window.__mcloneReady`, `window.__mcloneGui`, and, until its callers migrate, `window.__mcloneDebug`.
- Browser tests and probes drive the app through canvas/input events or machine hooks, not visible HTML controls.

## Current State

Already GPU-backed:

- `/` and `/index.html` canvas-only root shell
- default root `TitleScreen`
- opt-in smoke-harness title screen
- loading progress screen
- live generated world after title start
- `Esc` pause menu over the live world
- Back to Game input suppression/resume
- minimal GPU `OptionsScreen` reachable from root title and pause, with view/render distance sliders, lighting cycle, and water simulation checkbox persisted through the browser render config

Still DOM-backed:

- `debug.html` settings panel, progress overlay, text overlay, joystick, fly buttons
- debug-free-cam DOM progress/error/config plumbing
- debug input mobile controls

## Implementation Plan

| Step | Scope | Done when |
|---|---|---|
| 1 | Root shell cutover | **done** - `index.html` is canvas-only and boots GPU title by default; `/smoke.html` still direct-boots smoke scenarios |
| 2 | GPU world setup | seed, preset, movement mode, storage, and quickstart/continue behavior move from `index.html` JS into GPU screens |
| 3 | GPU options/debug settings | **partial** - `OptionsScreen` has GPU sliders/cycle/checkbox controls; Debug Settings screen is still pending |
| 4 | Debug harness migration | `debug.html` becomes canvas-only or aliases the root shell; `__mcloneDebug` remains as a machine API |
| 5 | Error/confirm/quit flow | loading errors, storage reset, save-and-quit, and return-to-title use GPU screens |
| 6 | DOM deletion sweep | remove obsolete HTML/CSS/DOM mutation code and update tests/probes to use GPU surfaces or hooks |

## Validation

Each implementation step must run the smallest relevant screenshot probe and inspect the output before continuing.

Baseline for code steps:

```bash
pnpm typecheck
pnpm test
pnpm test:browser
pnpm test:browser:integration
pnpm probe:browser -- test/browser/probes/<focused-gui-cutover-probe>.probe.ts
git diff --check
```

Use pinned `VITE_PORT` values when a local dev server is already running so Playwright does not reuse an unrelated app.

DOM retirement grep at the final deletion step:

```bash
rg -n "button|form|input|select|textarea|details|debug-overlay|debug-progress|debug-config|joystick|fly-btn|textContent|innerHTML|style\\.display|classList" index.html debug.html smoke.html src/renderer
```

Expected final grep result: only non-visible platform plumbing, test hooks, or comments remain, and each remaining hit is deliberately justified.

## Divergence Review

What vanilla does:

- The client owns all visible UI as `Screen` and widget objects rendered by the game renderer.
- Title, pause, options, confirm, progress, and error flows are not native OS controls.

Why browser/WebGPU diverges:

- HTML is needed for the canvas and browser APIs, but visible DOM controls create a second UI/input model.
- WebGPU command submission requires explicit render passes after the world pass.

Scope of divergence:

- Runtime shell and rendering backend only.
- GUI model stays vanilla-shaped; visible UI moves to GPU screens.

Parity impact:

- Positive. Removing DOM menus forces one Minecraft-shaped screen/widget path and keeps later vanilla screen ports from being split between web controls and GPU controls.
