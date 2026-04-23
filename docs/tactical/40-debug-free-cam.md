# Tactical 40 — Debug free-cam (temporary)

> **Status: temporary / debug tooling.** This slice is *not* a port of Minecraft's movement model and is *not* on the MVP path. It exists so a human can fly around the generated world during development instead of only seeing Playwright-captured frames. When real `Entity` / `LivingEntity` / `LocalPlayer` physics lands (see the "real path" note at the bottom), this slice should be deleted or gutted — don't build on it.

## Why "40"

The sequential arc (`00`–`26` today) tracks porting progress. Debug tooling that will be thrown away does not belong in that numbering. `40` is deliberately far ahead so nobody mistakes it for the next port slice and so grep'ing for `40-` surfaces all the temp scaffolding in one place.

## Goal

Open a page, click to capture the pointer, fly around the existing generated smoke world with WASD + mouse-look. That's it.

- No collision.
- No gravity.
- No entity / AABB / `VoxelShape` / `Pose` / fall-distance / tick separation.
- No HUD, crosshair, F3 overlay, block picker.
- No automated test coverage — visual only, open in Chrome manually.

## Non-goals (explicit, so future agents don't scope-creep)

- Do **not** port `net.minecraft.world.entity.Entity`, `LivingEntity`, `Player`, `LocalPlayer`, `Abilities`, `Input`, `KeyboardInput`, or `MouseHandler` in this slice. Those are the tactical 25b+ physics track.
- Do **not** introduce a tick loop, `partialTick` interpolation, or any fixed-step integration. Drive straight from `requestAnimationFrame`.
- Do **not** add block collision, step-assist, friction, slipperiness, or per-block speed factors.
- Do **not** tune numeric constants to match vanilla creative-fly (`abilities.flyingSpeed`, `walkDist`, etc.). Pick whatever feels OK for debugging.
- Do **not** add this to `pnpm test:browser`. The Playwright smoke stays headless and deterministic.

## Scope

A single parallel entry point that shares world setup with the existing smoke but swaps the 2-step `GENERATED_CAMERA_PATH` for an input-driven RAF loop.

| # | Module | Purpose |
|---|---|---|
| 1 | `src/renderer/debug/scene-setup.ts` | Extract the shared init block out of `main.ts` (atlas, models, world, dispatchers, `GameRenderer`, `LightTexture`) into one helper returning the assembled scene. `main.ts` and the new debug entry both consume it. |
| 2 | `src/renderer/debug/debug-input.ts` | Pointer-lock request on click, keydown/keyup → key-state set, mouse-delta → yaw/pitch accumulators. Plain object, no classes that mimic `Input`. |
| 3 | `src/renderer/debug/debug-free-cam.ts` | Browser entry. Calls scene-setup, installs input listeners, runs `requestAnimationFrame` loop: read input → update `position`/`xRot`/`yRot` → `level.ensureChunksForCamera(...)` → `gameRenderer.renderLevel(...)` → encode draws (ported from `main.ts`'s one-shot draw path). |
| 4 | `debug.html` | Sibling of `index.html`. `<canvas id="renderer">` + `<script type="module" src="/src/renderer/debug/debug-free-cam.ts">`. Instructions overlay ("click to capture, WASD + mouse, Esc to release"). |
| 5 | `package.json` script | `"dev:free-cam": "vite"` pointing at `debug.html` — or just document navigating to `http://localhost:5173/debug.html` under `pnpm dev:browser`. Prefer the latter (less surface). |

## Input → camera math

Plain per-frame update; no physics integrator.

```
const forward = [−sin(yaw)·cos(pitch), 0, cos(yaw)·cos(pitch)]   // ignore pitch for horizontal strafe
const right   = [ cos(yaw),             0, sin(yaw)            ]
velocity      = forward·(W−S) + right·(D−A) + [0,1,0]·(Space−Shift)
position     += normalize(velocity) · SPEED · dt
yaw          += mouseDeltaX · SENS
pitch         = clamp(pitch + mouseDeltaY · SENS, −89°, +89°)
```

Suggested starting values: `SPEED = 20` blocks/sec, `SENS = 0.15°/px`, Ctrl held → `SPEED × 4`. Tune by feel, not by vanilla parity.

## Chunk loading

The existing `GeneratedRenderLevel.ensureChunksForCamera(x, z, viewDistance)` already streams chunks around a camera. Call it every frame (or only when the camera crosses a chunk boundary, if that turns out to cost something). When it returns `true` (new chunks loaded), call `levelRenderer.allChanged()`.

Leave `GENERATED_VIEW_DISTANCE` at `1` initially so the user can watch chunks pop in while debugging decoration work — bump it later if wanted.

## Rendering

`main.ts` already has the full per-frame draw path (bind-group creation, pipeline cache, depth view, encode-draw-pass). Move that into a `renderFrame(scene, cameraState)` function in `scene-setup.ts` so both `main.ts` and `debug-free-cam.ts` call the same code. The one-shot readback + pixel assertion stays in `main.ts` only.

Depth view and readback texture get recreated per frame today (fine for a 2-step smoke). In the RAF loop, create them **once** at scene setup and reuse — otherwise GC churn will dominate the frame.

## Translation gotchas

- **Pointer lock requires a user gesture.** Request it inside a `click` handler on the canvas, not at load time.
- **Pointer-lock `movementX/movementY` are pixel deltas, not absolute.** Accumulate directly; don't try to read `clientX/clientY` — it's stale during lock.
- **Key repeat.** Use a `Set<string>` of held keys driven by `keydown`/`keyup`, not `keydown` events directly.
- **Canvas size vs. device-pixel-ratio.** Ignore DPR for this slice; the smoke harness already does.
- **Frame pacing.** Use `performance.now()` deltas for `dt`, not a fixed per-frame constant — Chrome may throttle to 30 Hz on battery.
- **Do not call `allChanged()` every frame.** That nukes the compiled-section cache and tanks perf. Only on new-chunk-load.

## Done when

- `pnpm dev:browser`, navigate to `http://localhost:5173/debug.html`, click the canvas, and fly around the generated world with WASD + mouse-look.
- Existing `pnpm test:browser` still passes (smoke harness untouched).
- `pnpm typecheck` and `pnpm test` stay green.
- `main.ts` no longer duplicates scene setup — it and `debug-free-cam.ts` both call `scene-setup.ts`.
- A one-line `> Debug tooling — see tactical 40` banner at the top of every file under `src/renderer/debug/`, so the temp scope is obvious at a glance.

## The real path (not this slice)

When we actually want Minecraft-exact walking/falling, that's a ~6–7 slice track: `Entity` + `AABB` + `move(MoverType, Vec3)`, then `VoxelShape` + per-block collision shapes, then `Player` + `LocalPlayer` + `Abilities` + `Pose`, then fluid/climb/friction, then tick/render decoupling with `partialTick`. This slice deliberately does none of that and should be deleted when any of it lands — don't graft physics onto the debug free-cam.
