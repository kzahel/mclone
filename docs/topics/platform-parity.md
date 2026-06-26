# Platform Parity

Durable cross-platform parity tracker for the five client lanes. This is the
"where are we now, and what is left to reach par" answer between sessions.

This doc owns three things no other doc owns:

1. a **target-state definition** per platform class, so "par" is a checklist and
   not a vibe;
2. a **feature × platform matrix** (current → target) — the burn-down for "every
   platform has every feature";
3. a **shared-contract × consumer matrix** — the burn-down for "proper platform
   interfaces and reuse," i.e. the platform contract matrix that
   [`../platforms.md`](../platforms.md), [`../architecture.md`](../architecture.md),
   [`../native-rewrite-roadmap.md`](../native-rewrite-roadmap.md), and
   [`../tactical/084-single-view-platform-alignment.md`](../tactical/084-single-view-platform-alignment.md)
   (Slice 4) all call for.

Related docs: [`../platforms.md`](../platforms.md) owns lane status and
validation policy; [`../architecture.md`](../architecture.md) owns the runtime
boundary; this doc owns the per-feature and per-contract grids and the rule that
keeps new features from re-forking.

> Status note: the current-state cells below were derived from a code audit on
> 2026-06-26 and refreshed after tactical 088's Android XR remote validation.
> When a slice closes a gap, update the affected cell **and** link the tactical.
> If a cell and the code disagree, the code wins — fix the cell.

## Platform Classes And Target State

Collapse the five lanes into two **classes** with an explicit feature target. A
feature is "at par" for a lane when it meets its class target (adapted for the
display/input shape), not when it is byte-identical to desktop.

### Flat class — desktop flat, flat Android, web/WASM

Target: **full game client.** Same player-facing feature set across all three.

- world render, lighting, day/night
- first-person player movement + collision
- block interaction (raycast, break, place, hotbar)
- remote-player and passive-entity rendering
- HUD (crosshair, hotbar, debug/status overlay)
- menus: title, pause, options, **server-connect, world/seed select**
- local-integrated **and** remote-dedicated host modes
- underwater/screen effects (desktop/web; mobile may stage later for perf)

> **Confirmed:** flat Android is a full client, not a viewer. Its current
> orbit-camera shell is interim and must converge on the shared player
> controller + touch overlay.

### Stereo-XR class — desktop OpenXR, Android XR / Quest

Target: **full game client adapted to stereo + controllers.** XR diverges only
where the display/input shape forces it (no flat 2D overlay; world-space panels
and controller-ray pointer instead).

- world render (per-eye), lighting, day/night — *at par now*
- controller locomotion + collision — *at par now*
- remote-player and passive-entity rendering
- **controller block interaction** (ray pick, break, place)
- **world-space HUD + menus** (crosshair reticle, options, server-connect). A
  shared pause/options menu now renders through a world-space panel path with a
  controller-ray pointer; headset validation and tuning are still open.
- local-integrated **and** remote-dedicated host modes

XR controller interaction and world-space menus are now an active alignment
slice, ahead of text-entry/server-connect UI. See tactical 089.

> **Straw-man, please confirm:** world **persistence** (server saves a world
> between sessions) is treated as in-target for every lane below. The engine has
> a filesystem store but no app wires it today. If persistent worlds are not in
> scope this cycle, drop that row from the target.

## Matrix 1 — Feature × Platform (parity burn-down)

Current state. Legend: ✅ working · ◐ partial/limited · ✗ absent (engine may
support it; the lane does not wire it) · — n/a for the class.

A cell is a **parity gap** when it is not ✅ and its class targets it above.

| Feature | desktop-flat | desktop-XR | flat-Android | Android-XR | web/WASM |
|---|:--:|:--:|:--:|:--:|:--:|
| World render (textured terrain) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Lighting (sky+block, render integ.) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Day/night + sky | ✅ | ✅ | ◐ (frozen) | ✅ | ✅ |
| Player movement + collision | ✅ | ✅ | ✗ (orbit cam) | ✅ | ✅ |
| Block interaction (break/place) | ✅ | ✗ | ✗ | ✗ | ✅ |
| Remote-player rendering | ✅ | ◐ (path, unspawned) | ✗ | ◐ (path, unspawned; device smoke pending) | ✅ |
| Passive entities (cow/chicken) | ✅ | ◐ (path, unspawned) | ✗ | ◐ (path, unspawned; device smoke pending) | ◐ (placeholder) |
| Fluids (server sim, renders as terrain) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Underwater / screen effects | ✅ | ✗ | ✗ | ✗ | ✗ |
| HUD (crosshair/debug/status) | ◐ (no in-world crosshair) | ✗ | ✗ | ✗ | ✅ |
| Hotbar (debug palette) | ✅ | ✗ | ✗ | ✗ | ✅ |
| Menus (title/pause/options) | ✅ | ◐ (world panel + pointer, unvalidated) | ◐ (shared touch menu, device pending) | ◐ (world panel + pointer, unvalidated) | ✅ |
| Connect / world-select UI | ✗ | ✗ | ✗ | ✗ | ✗ |
| Remote-dedicated connect (wired in app) | ✅ TCP | ✅ TCP | ✅ TCP property | ✅ TCP intent argv (LAN + --adb-reverse smokes passed) | ✅ WebSocket query param |
| Persistence (world save/load, in-app) | ✗ | ✗ | ✗ | ✗ | ✗ (cfg-excluded) |
| Audio | ✗ | ✗ | ✗ | ✗ | ✗ |

Reading the matrix:

- **desktop-flat** is the reference; the only flat-class gaps are connect-UI,
  persistence, audio, and an in-world crosshair.
- **web** is near desktop parity; gaps are connect-UI, underwater FX,
  persistence (structurally impossible on `wasm32` today), audio.
- **desktop-XR** has render/locomotion parity and a shared pause/options
  world-panel menu with controller-ray pointer, but no headset-validated menu
  comfort or spawned actor scenarios.
- **Android-XR** now has the same shared actor render path and pause-menu
  world-panel/pointer path wired through `mclone-xr-scene`, but device visual
  validation of menu presentation/interaction and actual spawned actor scenarios
  remain pending.
- **flat-Android** is still the thinnest full-client lane: it has terrain,
  lighting, shared local/remote host wiring, a shared menu/touch path in code,
  and a touch-orbit shell, but still lacks player movement, interaction, actors,
  HUD/hotbar, device-validated menu UX, and an in-app connect flow.

## Matrix 2 — Shared Contract × Consumer (reuse burn-down)

Does each app consume the shared boundary, or fork it? Legend: ✅ consumes
shared · ◐ partial / re-derives · ⚑ forks (parallel app-local copy) · ✗ does not
use (and should) · — n/a.

| Shared boundary | native-client | web-client | flat-Android | Android-XR | Sentinel gate |
|---|:--:|:--:|:--:|:--:|---|
| `mclone-protocol` / `mclone-net` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-net`; `--multi-client-smoke` |
| `mclone-server` / `IntegratedServer` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-server` |
| `mclone-client::ClientRuntime` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-client` |
| `mclone-render` (view/target draw) | ✅ | ⚑ (inline render path) | ✅ | ✅ | `native:desktop-chunk:smoke` |
| `mclone-render-session` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-render-session` |
| `app-runtime::SingleViewRuntime` | ✅ | ✅ | ✅ | ✅ (via xr-scene) | `cargo test -p mclone-app-runtime` |
| `app-runtime::frame_render` | ✅ | ⚑ (inline reimpl) | ✅ | ✅ (via xr-scene) | `native:desktop-chunk:smoke`; `native:web:smoke` |
| `app-runtime::local_single_view` (native scene driver) | ✅ (WindowSceneRuntime + desktop XR compose) | — (wasm-gated) | ✅ | ✅ (via xr-scene) | `cargo test -p mclone-app-runtime` |
| `app-runtime::host_mode` (local vs remote) | ✅ | ◐ (async enum, shared exchange/resync policy) | ✅ | ✅ (local/remote via xr-scene) | `cargo test -p mclone-app-runtime host_mode`; `pnpm native:web:build` |
| `app-runtime::render_assets` | ✅ | — (wasm has own) | ✅ | ✅ | `cargo test -p mclone-app-runtime` |
| `mclone-ui` (GuiDrawList) | ✅ | ✅ | ◐ (shared touch menu, device pending) | ◐ (XR world panel + pointer, unvalidated) | `native:web:app-smoke`; `cargo test -p mclone-ui`; `cargo test -p mclone-xr-scene` |
| `mclone-xr-{host,graphics,scene}` | ✅ | — | — | ✅ | `native:xr:*`; `native:android-xr:validate` |

The reuse story in one line: **desktop flat, flat Android, desktop XR, and
Android XR now share the native scene shells; web still carries the important
runtime/render fork.**
Concretely:

- The native flat single-view scene driver is shared:
  `WindowSceneRuntime` and flat Android both compose
  `NativeSingleViewSceneRuntime<S>`, while concrete TCP/property/config remains
  in the app crates. This landed in
  [`../tactical/084-single-view-platform-alignment.md`](../tactical/084-single-view-platform-alignment.md).
- The XR scene-driver fork is closed:
  `XrMcloneTerrainState<S>` composes `NativeSingleViewSceneRuntime<S>`, desktop
  XR passes the desktop TCP `RemoteServerSession`, Android XR can pass its
  Android-owned TCP session adapter, and app-local `XrMcloneWorldState` is
  gone. This landed across tactical 086 Slices 1-4, tactical 087, and tactical
  088.
- `web-client` re-inlines the whole sky→chunk→actor render sequence instead of
  calling `render_full_frame_for_view`. It still owns an async `WebRuntimeHost`
  enum, but command/update accounting and remote WebSocket reconnect/resync prep
  now flow through `mclone-app-runtime::host_mode`. The playable browser app can
  join a dedicated WebSocket server through `?remoteWsUrl=...`, covered by
  `native:web:remote-smoke` (see blocker #1).
- flat Android has a TCP remote-dedicated path through
  `debug.mclone.remote_addr`; Android XR now uses launch-scoped
  `mclone.startup.argv --remote-addr HOST:PORT` and can have the validator start
  a local dedicated server. Android XR reached `MCLONE_ANDROID_XR_READY` against
  a dedicated server over direct LAN and through `--adb-reverse`.

## Whole Systems That Do Not Exist Yet

Parity for these is blocked because there is nothing to port. Several are
host-adapter concerns that should be designed as interfaces *before* they are
built, exactly as transport/storage were:

- **Audio / sound — entirely absent.** No crate, no `play_sound`. A host-agnostic
  audio interface (native vs Web Audio vs Android) is unplanned.
- **Persistence wiring — absent in every app.** Engine has
  `FilesystemSnapshotStore`; all apps use `NullChunkSnapshotStore`, and the
  dedicated server has no `--world-dir`. Every launch regenerates from seed.
- **Connect / world-select UI + text-input widget — absent.** `mclone-ui` has no
  `EditBox`, no loading/progress screen, and a 1px-rect uppercase-only font, so
  typing a server address is currently impossible. This is separate from the
  shared menu-surface bring-up; CLI, Android launch argv/properties, and web
  query params remain acceptable connect surfaces for now.
- **Stereo / world-space UI — partial.** Desktop XR and Android XR now share a
  `mclone-ui` pause/options panel through `mclone-xr-scene`, toggled by left-hand
  select, rendered by `mclone-render::gui::WorldGuiRenderer`, and driven by
  controller-ray trigger clicks. The XR target still needs headset visual and
  interaction validation.
- **Real inventory / items, crafting, mob AI / spawning, chat, settings
  persistence — absent.** Only a fixed 7-block debug hotbar exists.

## Cross-Cutting Blockers (do these before more content features)

Every new feature added today gets forked across up to four app shells. The
highest-leverage work is the shared contracts that *stop* the forking. Land
these first:

1. **Finish the host-mode async/sync cleanup.** Native desktop, flat Android,
   and XR app shells use blocking TCP/session adapters, while browser
   worker/WebSocket mechanics stay async and still sit behind `WebRuntimeHost`.
   Shared exchange accounting, remote WebSocket reconnect/resync prep, and
   playable browser remote-connect wiring have landed; remaining work is naming
   cleanup plus clarifying the web render-section idle diagnostics. (tactical
   085)
2. **Finish the shared menu surface before adding more menu features.** XR now
   has a shared world-panel pause/options menu with pointer input, but it still
   needs headset visual/interaction validation. Flat Android now consumes
   `mclone-ui` in code for pause/options touch input, but still needs device
   validation and broader HUD/touch gameplay controls. Connect/world-select UI
   and `EditBox` should build on this surface later, not define the first slice.
   (tactical 089, tactical 090)
3. **Finish the shared input-intent layer.** Unify raw input → intent across
   keyboard/mouse, touch, pointer, and XR controllers, covering **menu-nav,
   pointer, and interact**, not just locomotion. Required for XR interaction and
   for flat Android to use the player controller. (tactical 076 follow-up)
4. **Keep Android XR remote validation first-class for both USB and LAN.** The
   adapter and Playbox-style launch argv option exist now (`--remote-addr` in
   `mclone.startup.argv`), and Quest smokes passed over direct LAN and through
   the `--adb-reverse` validator path. Keep the LAN route documented as
   firewall-sensitive.
5. **Validate and tune the stereo/world-space UI path** so XR can show a
   comfortable panel, reticle/pointer, and later a connect screen. The first
   panel renderer and pointer path landed; headset validation remains open.
   (tactical 089)
6. **Protocol: add server push and cross-version negotiation.** Today it is
   strict request/response (a client that stops polling stops seeing others move)
   with strict-equality version match (independently-deployed web/APK/desktop
   builds will skew). Real-time co-presence and mixed-build cross-play both
   depend on this. (tactical 009 deferred these)

## Definition Of Done (the rule that prevents re-forking)

A user-facing feature is **done** only when:

1. it lands behind a **shared crate contract**, not an app-local fork (if a new
   app-local copy is unavoidable, record it as ⚑ in Matrix 2 with a follow-up);
2. **Matrix 1** is updated for every lane the feature's class targets — a feature
   is not "done on desktop," it is done when its class is at par or the remaining
   lanes are tracked here as explicit gaps;
3. the **sentinel gate** for the touched boundary (Matrix 2) passes, plus the
   device/headset lane when the change touches platform glue, packaging, OpenXR
   session/swapchain/action code, or browser worker/ABI glue (per
   [`../platforms.md`](../platforms.md) validation policy).

This keeps coverage deliberate without running every device for every branch.

## Update Policy

- Update a cell when a slice changes the truth, and link the tactical that did it.
- Keep this doc to the matrices, the target definition, and the blocker list.
  Detailed per-slice work stays in `docs/tactical/`; subsystem internals stay in
  their own topic docs ([`lighting.md`](lighting.md), [`performance.md`](performance.md)).
- When the audit drifts from code, re-run the per-axis audit and refresh the
  current-state cells with a new date stamp.
