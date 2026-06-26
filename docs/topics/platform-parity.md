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
> 2026-06-26. When a slice closes a gap, update the affected cell **and** link
> the tactical. If a cell and the code disagree, the code wins — fix the cell.

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
- **world-space HUD + menus** (crosshair reticle, options, server-connect) via a
  stereo/world-space UI path that does not exist yet
- local-integrated **and** remote-dedicated host modes

> **Straw-man, please confirm:** XR controller interaction and world-space menus
> are listed as *targets* but sequenced **after** flat-class parity lands, since
> they need new UI/input architecture (see Cross-Cutting Blockers). If you want
> XR interaction pulled earlier, move it up.

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
| Remote-player rendering | ✅ | ◐ (path, unspawned) | ✗ | ✗ | ✅ |
| Passive entities (cow/chicken) | ✅ | ◐ (path, unspawned) | ✗ | ✗ | ◐ (placeholder) |
| Fluids (server sim, renders as terrain) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Underwater / screen effects | ✅ | ✗ | ✗ | ✗ | ✗ |
| HUD (crosshair/debug/status) | ◐ (no in-world crosshair) | ✗ | ✗ | ✗ | ✅ |
| Hotbar (debug palette) | ✅ | ✗ | ✗ | ✗ | ✅ |
| Menus (title/pause/options) | ✅ | ✗ | ✗ | ✗ | ✅ |
| Connect / world-select UI | ✗ | ✗ | ✗ | ✗ | ✗ |
| Remote-dedicated connect (wired in app) | ✅ TCP | ✅ TCP | ✗ | ✗ | ◐ (transport exists, not wired) |
| Persistence (world save/load, in-app) | ✗ | ✗ | ✗ | ✗ | ✗ (cfg-excluded) |
| Audio | ✗ | ✗ | ✗ | ✗ | ✗ |

Reading the matrix:

- **desktop-flat** is the reference; the only flat-class gaps are connect-UI,
  persistence, audio, and an in-world crosshair.
- **web** is near desktop parity; gaps are connect-UI wiring, underwater FX,
  persistence (structurally impossible on `wasm32` today), audio.
- **desktop-XR** has render/locomotion parity but no interaction, no UI, no
  actors spawned.
- **Android-XR** trails desktop-XR by one row: actor rendering is not wired
  (assets are loaded then discarded).
- **flat-Android** is the thinnest lane and the largest gap to its (now full)
  target: it is a render demo with an orbit camera, no player, no interaction,
  no UI, no networking.

## Matrix 2 — Shared Contract × Consumer (reuse burn-down)

Does each app consume the shared boundary, or fork it? Legend: ✅ consumes
shared · ◐ partial / re-derives · ⚑ forks (parallel app-local copy) · ✗ does not
use (and should) · — n/a.

| Shared boundary | native-client | web-client | flat-Android | Android-XR | Sentinel gate |
|---|:--:|:--:|:--:|:--:|---|
| `mclone-protocol` / `mclone-net` | ✅ | ✅ | ✗ (no dep) | ✗ (no dep) | `cargo test -p mclone-net`; `--multi-client-smoke` |
| `mclone-server` / `IntegratedServer` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-server` |
| `mclone-client::ClientRuntime` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-client` |
| `mclone-render` (view/target draw) | ✅ | ⚑ (inline render path) | ✅ | ✅ | `native:desktop-chunk:smoke` |
| `mclone-render-session` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-render-session` |
| `app-runtime::SingleViewRuntime` | ✅ | ✅ | ✅ | ✅ (via xr-scene) | `cargo test -p mclone-app-runtime` |
| `app-runtime::frame_render` | ✅ | ⚑ (inline reimpl) | ✅ | ✅ (via xr-scene) | `native:desktop-chunk:smoke`; `native:web:smoke` |
| `app-runtime::local_single_view` (scene driver) | ◐ (WindowSceneRuntime re-derives) | — (wasm-gated) | ✅ | ⚑ (xr-scene re-derives) | `cargo test -p mclone-app-runtime` |
| `app-runtime::host_mode` (local vs remote) | ✅ | ⚑ (parallel async enum) | ✗ | ✗ | `cargo test -p mclone-app-runtime host_mode` |
| `app-runtime::render_assets` | ✅ | — (wasm has own) | ✅ | ✅ | `cargo test -p mclone-app-runtime` |
| `mclone-ui` (GuiDrawList) | ✅ | ✅ | ✗ (empty list) | ✗ (empty list) | `native:web:app-smoke` |
| `mclone-xr-{host,graphics,scene}` | ⚑ (app-local XrMcloneWorldState) | — | — | ✅ | `native:xr:*`; `native:android-xr:validate` |

The reuse story in one line: **the two Android crates are the only thin
adapters; `native-client` and `web-client` carry the forks.** Concretely:

- One "integrated-server scene driver" exists in **three** hand-maintained
  copies — `LocalSingleViewSceneRuntime` (shared), `WindowSceneRuntime`
  (`native-client`), `XrMcloneTerrainState` (`mclone-xr-scene`) — ~270 LoC of
  near-duplicate poll/sync/idle logic.
- `mclone-xr-scene` is too narrow (no actors, no remote host), so desktop XR
  keeps a parallel app-local `XrMcloneWorldState` plus ~130 LoC of byte-identical
  transform helpers instead of consuming it.
- `web-client` re-inlines the whole sky→chunk→actor render sequence instead of
  calling `render_full_frame_for_view`, and runs a parallel `WebRuntimeHost`
  enum instead of `host_mode` (see blocker #1).
- flat Android and Android XR do not depend on `mclone-net`/`mclone-protocol` at
  all, so remote play is unreachable from those apps.

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
  typing a server address is currently impossible.
- **Stereo / world-space UI — absent.** `GuiRenderer` is a flat-NDC quad pass; it
  cannot render world-space panels and there is no controller-ray pointer.
- **Real inventory / items, crafting, mob AI / spawning, chat, settings
  persistence — absent.** Only a fixed 7-block debug hotbar exists.

## Cross-Cutting Blockers (do these before more content features)

Every new feature added today gets forked across up to four app shells. The
highest-leverage work is the shared contracts that *stop* the forking. Land
these first:

1. **Reconcile the host-mode async/sync seam.** `RemoteDedicatedServerSession`
   has a synchronous `send_command`, but the browser WebSocket is async, so web
   forked into a parallel `WebRuntimeHost`. Until the trait has an async-friendly
   shape, "shared host mode" does not actually serve web/Android/XR. This unblocks
   remote-dedicated connect on every lane. (tactical 084 Slice 3 follow-on)
2. **Build the connect/menu flow + the missing UI widgets.** Title → singleplayer
   vs server → address entry (`EditBox`) → world/seed select → loading screen. No
   lane can join a server in-app without this. Needed by the whole flat class.
3. **Finish the shared input-intent layer.** Unify raw input → intent across
   keyboard/mouse, touch, pointer, and XR controllers, covering **menu-nav,
   pointer, and interact**, not just locomotion. Required for XR interaction and
   for flat Android to use the player controller. (tactical 076 follow-up)
4. **Widen `mclone-xr-scene` (actors + pluggable/remote host) and delete the
   desktop-XR fork.** Lets desktop XR consume the shared scene like Android XR
   does. (platforms.md alignment #4)
5. **Collapse the three scene-driver copies into one shared driver** that
   `WindowSceneRuntime` and `XrMcloneTerrainState` compose. (tactical 084 Slice 2)
6. **Add a stereo/world-space UI render path** so XR can show a reticle, menus,
   and a connect screen. (new; no doc owns this yet)
7. **Protocol: add server push and cross-version negotiation.** Today it is
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
