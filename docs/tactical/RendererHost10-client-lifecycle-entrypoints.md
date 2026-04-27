# RendererHost10: Client Lifecycle and Entrypoints

Status: research / next implementation shape.

## Goal

Compare vanilla Minecraft's client lifecycle with the current mclone browser, headless, and smoke entrypoints, then define the next shape before adding more lifecycle code.

The desired end state is a straightforward main game client entrypoint with composable launch options, not a giant object and not a collection of unrelated smoke/debug programs. Smoke and test entrypoints should remain first-class harnesses, but they should reuse the same session lifecycle as the game client: open a world, attach presentation, run or drive frames, close cleanly, and return to a screen or process caller.

## Vanilla Source Review

Files read:

- `reference/minecraft-1.17.1/src/net/minecraft/client/Minecraft.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/server/IntegratedServer.java`

Relevant vanilla flow:

| Vanilla method | Lifecycle role |
|---|---|
| `Minecraft.loadLevel(...)` / `createLevel(...)` | Public open/create entrypoints that delegate to one internal load path |
| `Minecraft.doLoadLevel(...)` | Opens storage/resources, clears the previous level, starts the integrated server, shows `LevelLoadingScreen`, waits for readiness while ticking, then connects the client through the local memory channel |
| `Minecraft.setLevel(ClientLevel)` | Installs the client-world replica and updates render subsystems through `updateLevelInEngines(...)` |
| `Minecraft.clearLevel(Screen)` | Central leave-world path: cleans packet listener state, detaches the integrated server, resets renderer/game mode, switches to a progress screen and ticks once, waits for integrated-server shutdown, clears GUI/session state, nulls `level`, updates render subsystems with `null`, then nulls `player` |
| `Minecraft.updateLevelInEngines(...)` | Fans level attach/detach to `LevelRenderer`, particles, block entity renderer, and window title |
| `ClientLevel.disconnect()` / `ClientPacketListener.onDisconnect(...)` | Client-side quit/disconnect enters the same `clearLevel(...)` path, then chooses the next screen |
| `IntegratedServer.initServer()` / `tickServer(...)` | Integrated server is still a server lifecycle; it loads the world, can pause with the client, saves on pause, and shuts down independently from screen flow |

Important properties:

- Vanilla has one client shell that survives title, loading, world, pause, disconnect, and return-to-title transitions.
- Screens are state inside the client, not separate application entrypoints.
- Integrated singleplayer and remote multiplayer converge through a client connection and `ClientLevel` install path.
- World teardown is centralized. Rendering subsystems are explicitly detached from the old level before the next world is installed.
- `Minecraft` is a large object, but the lifecycle edges are still clear: screen, connection/server, client level, renderer subsystems, player, and session state are attached/detached in an ordered path.

## Current mclone Entrypoints

Current browser entrypoints:

| Entry | Current role |
|---|---|
| `index.html -> src/renderer/main.ts` | Canvas-only root shell. Boots GPU title by default, can start a generated world, and can also run direct smoke mode depending on URL/path. |
| `smoke.html -> src/renderer/main.ts` | Browser smoke harness. Bypasses title by default and runs generated-world smoke scenarios via `window.__mcloneReady`. |
| `debug.html -> src/renderer/main.ts` | Canvas-only debug launch alias. It auto-starts the same GPU title/world lifecycle with debug launch config, pointer lock enabled, and the `window.__mcloneDebug` machine hook. |

Current headless and host entrypoints:

| Entry | Current role |
|---|---|
| `scripts/deno-generated-world-smoke.ts` | Deno WebGPU generated-world harness. Creates a worker integrated server, runs shared generated-world boot/scenario logic, writes PNG artifacts, and closes the boot result per scenario. |
| Other `scripts/deno-*.ts` | Lower-level WebGPU, pipeline, texture, atlas, static-world, and vanilla-asset smoke capability probes. |
| `src/runtime/node/headless-generated-world-host.ts` | Node headless world host CLI. |
| `src/runtime/node/generated-world-http-server.ts` | Remote/generated-world HTTP host for browser clients and tests. |
| Playwright tests/probes | Test harness entrypoints that drive `smoke.html`, `index.html`, or `debug.html` and save screenshots under `/tmp`. |

There is no literal `main-headless.ts` today. Deno headless behavior lives in scripts, Node host behavior lives under `src/runtime/node/`, and browser smoke/debug are separate HTML/module entrypoints.

## What Is Ad Hoc Today

- `src/renderer/main.ts` mixes URL parsing, smoke mode, GPU title setup, loading screen updates, world boot, GUI overlay wiring, and live runtime start.
- `startGpuWorldRuntime(...)` owns the live RAF loop and can stop input/frame/depth resources, but it does not own full world-session teardown.
- `runGeneratedWorldBoot(...)` has a `close()` result, but browser scene creation does not currently provide a real scene close hook. Deno benefits from harness close because the headless adapter wraps `GeneratedWorldHeadlessHarness.close()`.
- `RendererScene` has `clientRuntime.close()`, render-world worker ownership, GPU resources, chunk dispatcher state, and texture resources, but no single scene/session disposal API.
- Browser smoke returns a result to tests and then depends mostly on page lifetime for cleanup.
- `main.ts` now also owns the debug launch alias, so the former `debug-free-cam.ts` lifecycle split is gone. The remaining cleanup is to keep thinning `main.ts` into a small shell around explicit client/session objects.
- Quit-to-title is not a real lifecycle transition yet. The GPU pause menu can return to game, but it cannot centrally close the world and restore title state.

## Target Shape

Keep the pieces small and composable:

| Layer | Owns | Does not own |
|---|---|---|
| `BrowserMain` | Browser platform bootstrap, URL/storage launch request parsing, machine hooks | World runtime details, smoke scenario sequencing |
| `GameClient` | Long-lived client shell: screen manager, active world session, title/loading/pause/error flow | Chunk generation, render-world worker internals, Deno filesystem details |
| `WorldSession` | One opened world: client runtime, renderer scene, presentation runtime, input attachment, close ordering | Title/menu policy, test scenario definitions |
| `LaunchRequest` | Seed/save/preset/position/transport/debug/scenario shortcuts from URL, screens, tests, or headless scripts | Runtime ownership |
| `ScenarioHarness` | Smoke/probe step sequencing and assertions | Application screen lifecycle |
| `PlatformHost` | Canvas or offscreen target, WebGPU device, assets, workers, storage, clocks, artifact writing | Game semantics |

The browser main path should become conceptually:

```text
index.html
  -> main.ts
    -> createBrowserPlatformHost(...)
    -> readLaunchRequest(location, storage)
    -> createGameClient(platformHost).start(launchRequest)
```

Smoke paths should become:

```text
smoke.html / Playwright / Deno script
  -> create platform host
  -> create world session through the same boot API
  -> run scenario harness
  -> close session
```

Debug/freecam is a launch mode/debug screen rather than a separate app:

```text
?mode=debug&seed=12345&preset=browser_smoke&movementMode=freecam
```

It still exposes `window.__mcloneDebug` for automation, but it does not own a parallel UI, loading, or lifecycle path.

## Lifecycle Contract

The next implementation should add an explicit close path with vanilla-inspired ordering:

1. Stop presentation input and RAF/frame loops.
2. Detach GUI/gameplay input from the world.
3. Detach render-world update sinks from the client runtime.
4. Close the client runtime, which closes worker/remote transports and the integrated-server facade when local.
5. Stop or terminate render-world workers.
6. Clear chunk dispatcher/view-area/level renderer state and detach the compatibility level.
7. Destroy owned GPU transient resources such as depth targets and disposable readback/render targets.
8. Leave shared device/adapter/browser canvas ownership with the platform host unless the host itself is closing.
9. Return to the next screen or headless caller with no active world session.

The important parity point is not copying vanilla's exact classes. It is preserving the explicit attach/detach shape: active world state is installed into render subsystems when a world opens and removed before the client returns to title or opens another world.

## Launch Shortcuts

The main client can still support direct shortcuts without becoming a test-only entrypoint:

- `seed`
- `saveId` or later savegame selector
- `preset`
- `worldTransport=worker|remote`
- `worldHostUrl`
- `position` / `cameraX,Y,Z`
- `cameraYaw` / `cameraPitch`
- `movementMode=player|freecam`
- `screen=title|world|debug`
- `scenario=<smoke scenario id>` for harnesses only
- `clearWorldStorage=true` behind a GUI confirm outside smoke mode

The rule should be: shortcuts produce a `LaunchRequest`; they do not create separate world-opening code paths.

## Proposed Implementation Slices

| Slice | Scope | Validation |
|---|---|---|
| Lifecycle1 | Add `closeRendererScene(...)` / `WorldSession.close()` and wire `runGeneratedWorldBoot(...).close()` for browser and Deno | focused unit tests, `pnpm smoke:deno:generated-world`, `pnpm test:browser`, `git diff --check` |
| Lifecycle2 | Introduce a small browser `GameClient` shell so `main.ts` delegates title/loading/world/quit-to-title policy | browser title probe, `pnpm test:browser`, screenshot inspection |
| Lifecycle3 | Add Save and Quit to Title to the GPU pause flow using the same close path, then restart a world in the same page | browser integration start -> pause -> quit -> title -> start, screenshot inspection |
| Lifecycle4 | Fold `debug-free-cam` behavior into launch requests/debug screens; reduce `debug.html` to a canvas-only alias or retire it | browser integration/probe coverage for debug movement/config hooks |
| Lifecycle5 | Make Deno headless run two independent world sessions in one process through the same lifecycle API, proving close/open parity outside browser pages | Deno generated-world smoke with repeated sessions and PNG artifacts |

## Done When

- `main.ts` is a thin browser app entrypoint, not a mix of app, smoke, and lifecycle internals.
- `debug-free-cam` no longer owns the primary game lifecycle.
- Browser, Deno headless, and tests open worlds through the same session lifecycle.
- Quit-to-title closes the active world without reloading the page.
- A second world can be opened in the same browser page or Deno process without stale workers, stale render-world state, or duplicate input loops.
- Smoke/test entrypoints remain direct and fast, but they use harness adapters over the shared session API rather than duplicating boot logic.

## Next

Implement Lifecycle1 first: a concrete `WorldSession`/scene close path that browser and Deno both use. This is the smallest useful step because every later title/debug/menu cleanup depends on being able to leave a world without relying on page/process teardown.
