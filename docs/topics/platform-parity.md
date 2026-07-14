# Platform Parity

Durable cross-platform parity tracker for the client lanes and offscreen flat
validation host. This is the "where are we now, and what is left to reach par"
answer between sessions.

This doc owns three things no other doc owns:

1. a **target-state definition** per platform class, so "par" is a checklist and
   not a vibe;
2. a **feature × platform matrix** (current → target) — the burn-down for "every
   platform has every feature";
3. a **shared-contract × consumer matrix** — the burn-down for "proper platform
   interfaces and reuse," i.e. the platform contract matrix that
   [`../platforms.md`](../platforms.md), [`../architecture.md`](../architecture.md),
   [`../native-engine-architecture.md`](../native-engine-architecture.md), and
   [`../tactical/084-single-view-platform-alignment.md`](../tactical/084-single-view-platform-alignment.md)
   (Slice 4) all call for.

Related docs: [`../platforms.md`](../platforms.md) owns lane status and
validation policy; [`../offscreen-flat-client.md`](../offscreen-flat-client.md)
owns the no-window flat-client target; [`../architecture.md`](../architecture.md)
owns the runtime boundary;
[`../client-experience-architecture.md`](../client-experience-architecture.md)
owns the draft shared client-experience core/profile/adapter target; this doc
owns the per-feature and per-contract grids and the rule that keeps new
features from re-forking. Tactical 171 — Convergence And Parity Closeout owns
the current cross-tactical ordering and exact browser feature-exception
burn-down.

> Status note: the current-state cells below were derived from a code audit on
> 2026-06-26 and refreshed on 2026-06-28 after tactical 095 Slice 4f, the
> existing audio foundation audit, user headset validation of the shared XR
> world-panel menu/pointer path, the offscreen flat-client target definition,
> the native `FlatClientDriver` headless screenshot/UI ownership work, and the
> shared desktop/offscreen `--startup-wait` readiness policy. Refreshed on
> 2026-07-05 after tactical 141 Slice 3 wired native web world catalog
> Create/Open/Delete through the shared flat catalog controller and IndexedDB
> adapter, then added `native:web:catalog-smoke` coverage for the menu flow and
> delete cleanup. Refreshed again after tactical 141 Slice 4a moved shared
> flat-client seed/new-world, join, start, back-to-title, and quit action
> effects into what is now `app-runtime::client_session_policy` for desktop and web adapters;
> Slice 4b added shared request-to-UI restoration, status overlay projection,
> and inactive-status clear policy for those adapters; Slice 4c added shared
> teardown-before-start and quit-to-title transition effects for those adapters;
> Slice 4d added shared session/startup projection for status plus optional
> loading progress. Refreshed again after tactical 143 Slice 2 made
> desktop/offscreen and web route catalog, session, settings/toggle, gameplay,
> and capability-projection action policy through
> `app-runtime::client_experience`; Slice 3 demoted web TypeScript world
> catalog code to an IndexedDB executor that delegates id validation, id
> generation, ordering, active-delete, and message text to Rust
> `world_catalog` wasm policy helpers. Refreshed again after tactical 143
> Slice 6 made flat Android consume the facade with transient create-only
> catalog capabilities and Slice 7 routed desktop XR / Android XR persistent
> catalog list/create/open/delete through the shared controller, using native
> and app-private storage adapters respectively. Refreshed again after closeout
> review found the XR underwater/screen-effect cells were stale: desktop XR and
> Android XR both use the shared `mclone-scene` multiview underwater path,
> and user device validation says it works pretty well; flat Android and
> web/WASM still need app-lane wiring.
> Refreshed on 2026-07-10 after tactical 168 Slices 7c–7e moved both the live
> desktop redraw path and all native offscreen/screenshot/timedemo/perf lanes
> onto the shared `mclone-scene` Mono host. `FlatClientDriver` is deleted;
> desktop/offscreen app code now owns only platform targets, cadence, raw input,
> CLI scenarios, and report/frame sinks. Non-default desktop render scale keeps
> world pixels scaled while composing host-owned UI at native output resolution.
> XR camera commits now use optional timing from the same shared reconcile path
> as flat clients; the scene-local timed policy fork is gone.
> Refreshed again after tactical 168 Slice 8 replaced flat Android's app-local
> loop with the shared Mono scene host. Touch input now uses the shared
> `TouchInputAdapter` (including sneak), all three AVD lanes and a lifecycle
> rebuild pass, Android inherits shared interaction/HUD/hotbar/screen effects,
> diagnostics, travel assist, and cadence, and the native feature-exception
> ledger is empty.
> Refreshed again after tactical 168 Slices 9–10 added a headset-free synthetic
> stereo capture through the same scene host, removed the last desktop
> compatibility runtime/startup wrappers, neutralized the public host/options
> names, consolidated startup and XR idle-poll constants, and added executable
> native thin-adapter and host-purity tripwires. The native non-web exception
> ledger remains empty. Refreshed on 2026-07-12 after tactical 173 made shared
> startup configuration canonical across desktop, offscreen, web, Android, and
> XR: browser query configuration stays behind an opaque Rust handle, camera
> startup/replacement uses one scene-owned factory, and the shared movement-mode
> launch canary reaches every lane without platform DTOs or setters.
> Refreshed on 2026-07-14 after the Tactical 176 pose-publication follow-up
> moved the at-most-20-Hz publication deadline and input/publication ordering
> into `McloneSceneHost`. Desktop, offscreen, web, flat Android, desktop XR,
> and Android XR now enter through shared Mono/XR frame methods; source-purity
> gates reject app-local packet-selection and publication policy.
> Refreshed again after the compact touchscreen action cluster replaced its
> lower-priority sprint and sneak slots with primary attack/break and secondary
> use/place. Native touch now routes those slots through the same shared actions
> as left and right mouse, and web touch invokes the corresponding existing
> block-interaction path; jump and descend retain the other two slots.
> Gamepad input is retained as a dated shared contract;
> no native adapter advertises it until a real platform event source and device
> validation land.
> Refreshed on 2026-07-14 after the lobby parity audit found that Tactical
> 177's web gap is not only an IndexedDB executor: managed scenario ownership,
> slot-targeted standby startup, dual-Worker/compiler identity, and WASM
> lifecycle adoption remain. Tactical 178 owns a shared-first refactor and
> forbids a TypeScript scenario implementation.
> Refreshed again after Tactical 178 completed Slices 0-8. Web now provisions
> the shared Rust-authored worlds into catalog-excluded IndexedDB records,
> retains two server Workers behind shared slot-targeted lifecycle policy,
> multiplexes them through one stable-identity compiler broker, renders the
> live preview, activates A-to-B-to-A, persists the island edit, and rejects
> stale lifecycle completions. Final native/browser feature-off performance and
> the available desktop/web/Android/XR build matrix pass. No headset was
> attached for a new capable-device multiview scenario receipt.
> Refreshed on 2026-07-11 after Tactical 170 — Web Scene-Host Adoption moved
> local-worker, IndexedDB, and remote-WebSocket browser modes onto
> `McloneSceneHost`, deleted the old web orchestrator, and made browser adapter
> purity executable. Tactical 169 — Runtime Asset Pack Selection also completed
> shared native/web transactional asset replacement, UI, persistence, and
> provenance. The structural matrix and prose below no longer describe the
> retired browser runtime/render-policy fork. Tactical 171 — Convergence And
> Parity Closeout Milestone D then promoted production browser Far LOD through
> the shared resident-tile worker/runtime/render path. It owns the exact
> remaining four-row browser feature ledger and active LOD sequence.
> When a slice closes a gap, update the affected cell **and** link the tactical.
> If a cell and the code disagree, the code wins — fix the cell.

## Platform Classes And Target State

Collapse the display/client lanes into two **classes** with an explicit feature
target. A feature is "at par" for a lane when it meets its class target
(adapted for the display/input shape), not when it is byte-identical to desktop.

### Flat class — desktop flat, offscreen flat, flat Android, web/WASM

Target: **full game client.** Same player-facing feature set across all flat
hosts. Offscreen flat is a validation/automation host rather than a user-facing
display lane, but it should run the same client lifetime and render the same
full-frame output into frame sinks.

- world render, lighting, day/night
- first-person player movement + collision
- block interaction (raycast, break, place, hotbar)
- remote-player and passive-entity rendering
- HUD (crosshair, hotbar, debug/status overlay)
- menus: title, pause, options, **server-connect, world/seed select**
- local-integrated **and** remote-dedicated host modes
- underwater/camera screen effects for flat clients
- no-window operation for offscreen flat: neutral input source, explicit render
  target, frame sink, and remote-dedicated play without a window manager

> **Confirmed:** flat Android is a full client, not a viewer. Its current
> shared touch player-camera/movement shell is interim and must gain
> interaction, HUD/hotbar, and device validation.

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
  controller-ray pointer; user headset validation says the current menu works
  mostly fine, while automated smoke coverage and comfort tuning remain open.
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

| Feature | desktop-flat | offscreen-flat | desktop-XR | flat-Android | Android-XR | web/WASM |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| World render (textured terrain) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Lighting (sky+block, render integ.) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Day/night + sky | ✅ | ✅ | ✅ | ◐ (frozen) | ✅ | ✅ |
| Player movement + collision | ✅ | ◐ (perf/scripted paths; no real host loop) | ✅ | ✅ (shared touch + AVD swipe) | ✅ | ✅ |
| Block interaction (break/place) | ✅ | ◐ (scripted `FlatInputFrame`, no real host loop) | ✗ | ◐ (shared path + touch controls; action smoke pending) | ✗ | ✅ |
| Remote-player rendering | ✅ | ◐ (render path, scenario coverage thin) | ◐ (path, unspawned) | ◐ (shared path, unspawned) | ◐ (path, unspawned; device smoke pending) | ✅ |
| Passive entities (cow/chicken) | ✅ | ◐ (render path, scenario coverage thin) | ◐ (path, unspawned) | ◐ (shared path, unspawned) | ◐ (path, unspawned; device smoke pending) | ◐ (placeholder) |
| Fluids (server sim, renders as terrain) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Underwater camera FX / screen effects | ✅ | ✅ | ✅ (multiview path + user device validation) | ✅ (shared Mono path) | ✅ (multiview path + user device validation) | ✅ (shared host/render path) |
| HUD (crosshair/debug/status) | ◐ (no in-world crosshair) | ◐ (debug/UI screenshots; no real HUD host) | ✗ | ✅ | ✗ | ✅ |
| Hotbar (debug palette) | ✅ | ✗ | ✗ | ✅ | ✗ | ✅ |
| Menus (title/pause/options) | ✅ | ◐ (screenshot scenarios; no real input host) | ◐ (world panel + pointer, user-validated; automation/tuning pending) | ✅ (shared touch menu, AVD session smoke) | ◐ (world panel + pointer, user-validated; automation/tuning pending) | ✅ |
| Built-in protected lobby scenario | ✅ | ✅ (scripted menu/activation) | ✅ (shared capability; real-device menu smoke pending) | ✅ (shared capability/APK; AVD app/new-world regression smokes pass, scenario-specific tap smoke pending) | ✅ (shared capability/APK; device scenario smoke pending) | ✅ (desktop/mobile preview, activation, persistence, lifecycle; Tactical 178) |
| Connect / world-select UI | ✗ | ✗ | ◐ (world catalog UI wired; connect-screen smoke pending) | ◐ (persistent world catalog + New World smoke; connect text/smoke pending) | ◐ (world catalog UI wired; connect-screen smoke pending) | ◐ (world catalog UI wired; connect-screen text/smoke pending) |
| Remote-dedicated connect (wired in app) | ✅ TCP | ◐ TCP screenshot/settle, no long-lived offscreen host | ✅ TCP | ✅ TCP property | ✅ TCP intent argv (LAN + --adb-reverse smokes passed) | ✅ WebSocket query param |
| Persistence (world save/load, in-app) | ◐ (filesystem/SQLite catalog + chunk/entity records; remaining metadata) | ◐ (persistent-world probes; not a full interactive host) | ◐ (shared native catalog/storage; remaining metadata) | ◐ (app-private catalog/storage; remaining metadata) | ◐ (app-private catalog/storage; remaining metadata) | ◐ (IndexedDB catalog + chunk/entity records; player/world metadata pending) |
| Basic audio (landing sound foundation) | ◐ (code; listen validation pending) | — (not targeted for no-window validation yet) | ◐ (code; listen validation pending) | ◐ (code; device audio pending) | ◐ (code; device audio pending) | ✗ |

Reading the matrix:

- **desktop-flat** is the reference; the remaining flat-class gaps are
  connect-UI, persistence, full audio validation/categories, and an in-world
  crosshair.
- **offscreen-flat** is the desired real no-window validation host. Today it has
  full-frame screenshots and scripted attack/use through neutral
  `FlatInputFrame`, plus shared startup readiness CLI with desktop window mode,
  but still lacks a long-lived client loop, broader input stream, and frame
  sinks. Tactical 105 owns that cleanup.
- **web** now uses the same scene-policy host as native. Product gaps include
  connect-screen text/smoke, full persistence lifecycle/metadata beyond the
  IndexedDB catalog plus chunk/entity records, audio, and the exact four
  reason-bearing feature-axis exceptions tracked below.
- **desktop-XR** has render/locomotion parity and a shared pause/options
  world-panel menu with controller-ray pointer. User headset validation says the
  menu works mostly fine; automated menu/replacement smoke, comfort tuning, and
  spawned actor scenarios remain open.
- **Android-XR** now has the same shared actor render path and pause-menu
  world-panel/pointer path wired through `mclone-scene`. User headset
  validation says the menu works mostly fine; automated menu/replacement smoke,
  comfort tuning, and actual spawned actor scenarios remain pending.
- **flat-Android** now runs the same Mono scene host as desktop/offscreen. Its
  AVD-validated touch movement, HUD/hotbar, menus, local world replacement,
  screen effects, interaction command path, diagnostics, travel assist, and
  cadence are shared. Remaining evidence gaps are spawned actor/remote-player
  scenarios, an actual break/place action smoke, and connect-screen text/input.
- **underwater camera effects** are not desktop-only by architecture. The shared
  renderer/app-runtime path exists, desktop native and current
  offscreen/headless screenshots compute and pass the camera-water overlay/fog,
  and `mclone-scene` now computes per-view/midpoint underwater overlays and
  renders the effect through the stereo/multiview path for desktop XR and
  Android XR. User device validation says the XR treatment works pretty well.
  Flat Android and web now consume the shared Mono host/render path.
- **basic audio** exists through `mclone-audio` on native desktop, desktop XR,
  flat Android, and Android XR. It is still a foundation slice: landing sounds
  only, no web audio yet, no automated audible validation, and no step/break/
  place/entity/music categories.

## Matrix 2 — Shared Contract × Consumer (reuse burn-down)

Does each app consume the shared boundary, or fork it? Legend: ✅ consumes
shared · ◐ partial / re-derives · ⚑ forks (parallel app-local copy) · ✗ does not
use (and should) · — n/a.

| Shared boundary | native-client desktop/offscreen | web-client | flat-Android | Android-XR | Sentinel gate |
|---|:--:|:--:|:--:|:--:|---|
| `mclone-protocol` / `mclone-net` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-net`; `--multi-client-smoke` |
| `mclone-server` / `IntegratedServer` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-server` |
| `mclone-client::ClientRuntime` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-client` |
| `mclone-render` (view/target draw) | ✅ | ✅ (canvas target through scene host) | ✅ | ✅ | `native:desktop-offscreen:smoke`; `native:web:smoke` |
| `mclone-render-session` | ✅ | ✅ | ✅ | ✅ | `cargo test -p mclone-render-session` |
| `app-runtime::SingleViewRuntime` | ✅ | ✅ | ✅ | ✅ (via scene host) | `cargo test -p mclone-app-runtime` |
| `app-runtime::frame_render` | ✅ | ✅ (via scene host) | ✅ | ✅ (via scene host) | `native:desktop-offscreen:smoke`; `native:web:smoke` |
| `app-runtime::local_single_view` (native scene runtime) | ✅ (composed by the scene host) | — (wasm-gated) | ✅ (composed by the scene host) | ✅ (composed by the scene host) | `cargo test -p mclone-app-runtime`; `cargo test -p mclone-scene` |
| `app-runtime::host_mode` (local vs remote) | ✅ | ✅ (browser connection services adapt local worker/IndexedDB/WebSocket mechanics) | ✅ | ✅ (local/remote via scene host) | `cargo test -p mclone-app-runtime host_mode`; `pnpm native:web:build`; `pnpm native:web:remote-smoke` |
| `app-runtime::session` (world-session coordinator) | ✅ (desktop flat dynamic; desktop XR dynamic via scene host, automated XR replacement-click smoke pending) | ✅ (initial local/remote plus menu New World restart; JoinRemote reconnect wired, connect-screen smoke pending) | ✅ (shared Mono host initial local/remote and New World / Join Remote replacement; AVD New World session smoke) | ✅ (initial local/remote plus shared scene replacement; Quest in-headset New World replacement smoke, automated controller replacement-click smoke pending) | `cargo test -p mclone-app-runtime`; `cargo test -p mclone-scene`; `pnpm native:web:app-smoke`; `pnpm native:web:remote-smoke`; `pnpm native:android:avd-session-smoke`; `pnpm native:android-xr:session-smoke` |
| `app-runtime::client_session_policy` (display-neutral session UI action policy) | ✅ (desktop flat/offscreen and desktop XR via `mclone-scene` execute shared seed/join/start/quit effects, status/restoration/startup projection, and teardown/quit-title transitions; native startup payloads remain host-local) | ✅ (web adapter executes shared seed/join/start/quit effects, status/restoration/startup projection, and teardown/quit-title transitions; JS async worker startup remains host-local) | ✅ (shared Mono host executes seed/join/start/quit effects and failed-start UI restoration; Android activity/surface remains local) | ✅ (scene host executes shared seed/join/start/quit effects, status/startup projection, failed-start UI restoration, and quit-title transitions; Android activity/session adapters remain host-local) | `cargo test -p mclone-app-runtime client_session_policy`; `cargo test -p mclone-native-client ui_action_routing`; `cargo test -p mclone-scene`; `pnpm native:desktop-offscreen:smoke`; `pnpm native:xr:mac:wivrn:smoke`; `pnpm native:android:avd-session-smoke`; `pnpm native:android-xr:session-smoke`; tactical 141 Slice 4d; tactical 143 Slice 4a; tactical 143 Slice 6; tactical 143 Slice 7 |
| `app-runtime::client_catalog_policy` (world catalog UI/action policy) | ✅ (desktop adapter and desktop XR scene execute native catalog effects) | ✅ (IndexedDB promise adapter executes controller effects; TS storage executor delegates id validation, id generation, ordering, active-delete, and message text to Rust wasm policy helpers) | ✅ (shared Mono host executes native catalog effects against the Android app-private world root; AVD New World flow covered) | ✅ (scene host executes shared catalog effects with an Android app-private world root) | `cargo test -p mclone-app-runtime client_catalog_policy`; `cargo test -p mclone-scene`; `pnpm native:web:typecheck`; `pnpm native:web:catalog-smoke`; `pnpm native:android:avd-session-smoke`; `pnpm native:xr:mac:wivrn:mclone`; `pnpm native:android-xr:apk`; tactical 141 Slice 3; tactical 143 Slice 3; tactical 143 Slice 6; tactical 143 Slice 7; tactical 143 Slice 7a |
| `app-runtime::client_experience` (catalog/session/settings facade) | ✅ (desktop/offscreen adapter and desktop XR scene execute facade effects; host window/runtime/pointer-lock/OpenXR work remains adapter-local) | ✅ (web adapter executes facade effects; IndexedDB promises, worker startup, and JS result writing remain adapter-local) | ✅ (shared Mono host executes facade effects; Android activity, raw touch translation, surface, and storage roots remain adapter-local) | ✅ (scene host executes facade catalog/session/settings/capability effects; Android activity/session adapters and app-private storage roots remain host-local) | `cargo test -p mclone-app-runtime`; `cargo test -p mclone-native-client ui_action_routing`; `cargo test -p mclone-native-client catalog_`; `cargo test -p mclone-scene`; `pnpm native:desktop-offscreen:smoke`; `pnpm native:web:typecheck`; `pnpm native:web:smoke`; `pnpm native:web:catalog-smoke`; `pnpm native:android:avd-smoke`; `pnpm native:android:avd-session-smoke`; `pnpm native:xr:mac:wivrn:smoke`; `pnpm native:android-xr:session-smoke`; tactical 143 Slice 2; tactical 143 Slice 4; tactical 143 Slice 6; tactical 143 Slice 7 |
| `app-runtime::render_assets` | ✅ | — (wasm has own) | ✅ | ✅ | `cargo test -p mclone-app-runtime` |
| `app-runtime::render_asset_data` (CPU mesh/atlas/far-LOD bundle) | ✅ | ✅ (production epoch replacement through scene host) | ✅ | ✅ | `cargo test -p mclone-app-runtime`; Tactical 169 — Runtime Asset Pack Selection; Tactical 170 — Web Scene-Host Adoption |
| `mclone-audio` | ✅ (desktop flat + desktop XR code wired; listen validation pending) | ✗ (web deferred) | ✅ (code wired; device audio validation pending) | ✅ (code wired; device audio validation pending) | `cargo test -p mclone-audio`; tactical 091 build gates |
| `mclone-ui` (GuiDrawList) | ✅ | ✅ | ✅ (shared touch menu+controls, AVD touch/session smoke) | ✅ (XR world panel + pointer, user-validated; automation/tuning pending) | `native:web:app-smoke`; `cargo test -p mclone-ui`; `cargo test -p mclone-scene`; `native:android:avd-session-smoke` |
| `mclone-scene` | ✅ | ✅ | ✅ | ✅ | `native:desktop-offscreen:smoke`; `native:web:smoke`; `native:xr-emulation:smoke`; `native:xr:*`; `native:android:*` |
| `mclone-xr-{host,graphics}` | ✅ (desktop XR) | — | — | ✅ | `native:xr:*`; `native:android-xr:validate` |

The reuse story in one line: **desktop flat, offscreen/screenshot/perf, flat
Android, desktop XR, Android XR, and web now share `mclone-scene`; the old web
runtime/render-policy fork is deleted, while browser mechanisms remain typed
services and offscreen still lacks an exposed long-lived source/sink mode.**
Concretely:

- `McloneSceneHost<S>` with `McloneSceneHostOptions` is the native shared owner;
  concrete TCP/property/config remains in app crates. `WinitFrameDriver`,
  `OffscreenDriver`, and `AndroidSurfaceDriver` adapt cadence and targets.
- `OffscreenDriver` and `WinitFrameDriver` drive the same scene host. Mono is
  the normal flat topology; the headset-free gate feeds synthetic Stereo views.
  Screenshot/script input, dual-view, timedemo, startup-streaming,
  frame-budget, and movement-frame therefore share runtime polling, section
  admission/upload, frame-input assembly, actors, `GameUi`, and full-frame
  composition. The remaining offscreen work is an exposed long-lived
  input-source/frame-sink mode (non-PNG sinks and backpressure), not another
  client or renderer. Tactical
  [`168`](../tactical/168-unified-native-scene-host.md) records the migration;
  [`105`](../tactical/105-offscreen-flat-client-host.md) remains the original
  product target.
- Flat and XR camera commits now share pose-sync, pending-correction, chunk
  interest, and one scene-owned at-most-20-Hz publication cadence. Mono hosts
  enter through `advance_mono_input_frame`; XR hosts enter through
  `apply_frame_locomotion`. Optional neutral timing keeps XR's existing
  locomotion attribution without a platform-local policy fork, while the pose
  model retains a seam for future tracked-head versus body-heading semantics.
- The XR scene-driver fork is closed:
  `McloneSceneHost<S>` composes `NativeSingleViewSessionRuntime<S>` around
  the shared native scene runtime, desktop XR passes the desktop TCP
  `RemoteServerSession`, Android XR can pass its Android-owned TCP session
  adapter, and app-local `XrMcloneWorldState` is gone. The scene-driver
  convergence landed across tactical 086 Slices 1-4, tactical 087, and tactical
  088; the startup session descriptor wrapper landed in tactical 095 Slice 4c.
- `web-client` now drives `McloneSceneHost` through a thin browser frame driver.
  Local worker, IndexedDB local-world, and remote WebSocket modes use typed
  browser connection/compiler/async-operation services while the shared host
  owns session, input/camera, render admission and synchronization, frame
  assembly, effects, actors, settings, UI/HUD, diagnostics, and accounting.
  The resident shared-memory render compiler and server/worldgen/light worker
  topology remain intact. JoinRemote reconnect is wired, with a dedicated
  connect-screen text/input smoke still pending.
- Flat Android drives the shared Mono host and shared native remote-session
  adapter. Its app crate retains only Android activity/lifecycle, Vulkan
  surface targets, raw touch/pointer/key translation, startup properties, and
  fixed presentation cadence. New World replacement is covered by
  `pnpm native:android:avd-session-smoke`.
- Desktop XR and Android XR route New World / Join Remote / Quit To Title menu
  actions through `mclone-scene`'s `GameSessionCoordinator` and
  `app-runtime::client_experience` facade. Each app host still owns its runtime
  factory: desktop maps back to `SceneOptions` and TCP `RemoteServerSession`,
  while Android XR maps launch-scoped options to `AndroidXrRemoteServerSession`.
  Shared session projection now owns XR status/progress UI and failed-start
  restoration; `SetXrTurnMode` is a capability-gated shared settings action
  that remains functional in XR. The code compiles through desktop XR and
  Android XR APK gates. Android XR New World replacement is covered on Quest by
  `pnpm native:android-xr:session-smoke`. User headset validation confirms the
  shared XR menu/pointer is usable; an automated controller-click replacement
  smoke is still pending.
- Android XR uses launch-scoped
  `mclone.startup.argv --remote-addr HOST:PORT`, can have the validator start a
  local dedicated server, and reports the local/remote session descriptor
  through the shared scene host. Android XR reached
  `MCLONE_ANDROID_XR_READY` against a dedicated server over direct LAN and
  through `--adb-reverse`.
- `mclone-audio` is a real shared native/XR/Android foundation now, not a
  placeholder. Native desktop, desktop XR, flat Android, and Android XR all
  construct `AudioEngine` and drain shared landing events; web/WASM audio is the
  remaining platform gap.

## Whole Systems Missing Or Not At Par

Parity for these is blocked either because the system is absent, or because the
shared foundation exists but is not yet product-complete on every lane. Several
are host-adapter concerns that should be designed as interfaces *before* they
are built, exactly as transport/storage were:

- **Audio / sound — partial, not absent.** `mclone-audio` exists and native
  desktop, desktop XR, flat Android, and Android XR are wired for landing
  sample playback. Web/WASM audio remains deferred, audible/device validation is
  still manual, and real sound parity still needs step/break/place/entity/music
  categories.
- **Persistence lifecycle — partial on every interactive app.** Native clients
  have filesystem/SQLite world catalogs and platform-private roots; web has an
  IndexedDB catalog plus chunk/entity records. Player/world metadata and full
  lifecycle parity remain incomplete, and offscreen is a probe rather than a
  long-lived interactive persistence host.
- **Connect / world-select UI + text-input widget — absent.** `mclone-ui` has no
  `EditBox`, no loading/progress screen, and a 1px-rect uppercase-only font, so
  typing a server address is currently impossible. This is separate from the
  shared menu-surface bring-up; CLI, Android launch argv/properties, and web
  query params remain acceptable connect surfaces for now.
- **Stereo / world-space UI — partial.** Desktop XR and Android XR now share a
  `mclone-ui` pause/options panel through `mclone-scene`, toggled by left-hand
  select, rendered by `mclone-render::gui::WorldGuiRenderer`, and driven by
  controller-ray trigger clicks. User headset validation says the current menu
  works mostly fine; the XR target still needs automated smoke coverage and
  comfort tuning.
- **Real inventory / items, crafting, mob AI / spawning, chat, settings
  persistence — absent.** Only a fixed 7-block debug hotbar exists.

## Shared-First Feature Checklist

Before adding a user-facing feature to one target, first decide where the
shared contract lives. App crates should only collect platform facts, adapt
them into shared contracts, and own platform resources. If a target needs a
temporary app-local implementation, mark it as a fork in Matrix 2 and add a
follow-up tactical to converge it.

Desktop flat is the fastest validation lane, not the default implementation
target. New feature work should be described as "shared implementation, desktop
validation first" unless the user explicitly asked for desktop platform glue.
Treat substantial new `mclone-native-client` logic as a warning sign: if the
same behavior will matter to flat Android, web/WASM, desktop XR, or Android XR,
move or extend the shared owner before building more desktop-local surface area.

| Feature area | Shared owner / next shared owner | App crates may own only |
|---|---|---|
| Input capabilities, bindings, and gameplay intents | `mclone-input`; flat/XR split is allowed only at pose/ray/comfort boundaries | raw OS/browser/Android/XR events, focus, pointer lock, sensor/controller polling |
| UI, HUD, menus, text widgets, and prompt presentation | `mclone-ui` | surface placement, world-space panel transforms, platform text/IME event capture |
| Audio and sound events | `mclone-audio`, with gameplay sound event production outside app crates | device creation, platform audio permissions, web/native output adapters |
| Network protocol and transport/session commands | `mclone-protocol`, `mclone-net`, `mclone-app-runtime` | socket/WebSocket construction, platform addresses, lifecycle reconnect triggers |
| Client prediction, replica state, interaction, and presentation events | `mclone-client` / `mclone-render-session` | raw input collection, final platform dispatch, device-specific view ownership |
| Authoritative simulation, world state, ticking, and scheduling | `mclone-server` plus domain crates (`mclone-worldgen`, `mclone-light`) | process/app startup, dedicated/integrated host construction |
| Assets, content loading, registries, and resource-pack shape | `mclone-assets` plus future shared content registries | platform file/package/HTTP access adapters |
| Rendering semantics and render-session data | `mclone-render-session`, `mclone-render` | swapchain/surface ownership, platform render target acquisition |
| Persistence, saves, player data, and durable settings | future shared persistence/settings contract; do not hide this in one app | filesystem/localStorage/app-storage adapters and migration entrypoints |
| Diagnostics, profiling, telemetry, and smoke reports | future shared diagnostics contract | platform counters that only exist in that backend, exported through shared report structs |
| Jobs, workers, priorities, cancellation, and budgets | future shared job/scheduler contract | native thread/Web Worker/Android worker creation and platform wakeups |
| Preferences/config, keybinds, graphics/audio/debug options | future shared preferences contract, consumed by `mclone-ui` and app adapters | platform persistence path and launch overrides |
| Text input, IME, clipboard, chat/commands/signs/server address entry | `mclone-ui` text model plus platform text adapters | IME composition events, clipboard permissions, soft-keyboard visibility |
| Inventory, items, crafting, containers, and item use | future shared gameplay/content contracts in client/server/content crates | platform input that selects or activates shared actions |
| Particles, block damage, transient effects, and animation events | future shared presentation/event contracts | target-specific rendering of shared effect records |
| Entity AI, spawning, pathing, damage, and interpolation policy | `mclone-server` for authority, `mclone-client`/`mclone-render-session` for presentation | platform-specific display/input only |
| Localization and font/text layout data | future shared localization/text contract | platform locale discovery and font asset access |

## Desktop Gravity And Adapter Footprint Audit

The matrices above are the source of truth, but this quick audit helps catch
platform code that is becoming an architectural signal. Run it when a slice
touches `mclone-native-client`, adds substantial app-local code, or introduces
a temporary fork:

```bash
for d in \
  native/apps/mclone-native-client/src \
  native/apps/mclone-web-client/src \
  native/apps/mclone-web-client/www \
  native/apps/mclone-android-client/src \
  native/apps/mclone-android-xr-client/src \
  native/crates/mclone-app-runtime/src \
  native/crates/mclone-render-session/src \
  native/crates/mclone-scene/src
do
  printf '%7s %s\n' \
    "$(find "$d" -maxdepth 1 -type f \( -name '*.rs' -o -name '*.ts' -o -name '*.js' \) -print0 2>/dev/null | xargs -0 wc -l 2>/dev/null | tail -1 | awk '{print $1}')" \
    "$d"
done

rg -n "poll_until_idle|sync_all_render_sections|std::thread::sleep|block_on" \
  native/apps native/crates/mclone-app-runtime native/crates/mclone-scene
```

Do not turn the counts into a hard budget. Use them to find cleanup candidates:

- `mclone-native-client` growth outside `winit`, desktop surface/input, CLI,
  offscreen source/sink glue, perf harnesses, or desktop diagnostics
- startup/loading/progress policy that blocks a platform loop instead of
  reporting incremental shared progress
- app-local runtime/session/render/UI/input behavior that another platform
  already needs or will obviously need
- a Matrix 2 `⚑` fork without a tactical convergence path

## Cross-Cutting Blockers (do these before more content features)

The shared scene host now prevents the former app-shell policy fork. The
highest-leverage remaining work is to preserve that boundary while closing
lower-level render residency and explicit product/evidence gaps:

1. **Reduce desktop app gravity before adding more desktop-local behavior.**
   `mclone-native-client` can own `winit`, desktop surface/input, CLI,
   offscreen source/sink glue, perf harnesses, and desktop diagnostics. It
   should not keep
   accumulating startup-loop policy, loading/progress state, session lifecycle,
   render policy, UI/HUD/menu behavior, input semantics, persistence, or
   gameplay. Move those into the shared owners in the checklist above, then use
   desktop flat as the first validation lane.
2. **Close the connect-UI gap and automate XR replacement/menu smoke.** Every
   lane now has shared local/remote session identity and a replacement code
   path. Flat Android New World replacement is covered by AVD touch-menu smoke,
   Android XR New World replacement is covered by an in-headset launch smoke,
   and user headset validation says the shared XR menu/pointer works mostly
   fine. Remaining work is an automated XR controller-click replacement/menu
   smoke, a dedicated web Join Remote connect-screen smoke, and the `mclone-ui`
   text-input/connect-world UI needed to choose endpoints in app instead of via
   CLI/properties/query params. World catalog selection now has focused web
   menu smoke coverage through tactical 141. (tactical 095)
3. **Finish shared resident-tile/LOD convergence, then burn down browser feature
   exceptions.** Browser worker/WebSocket/IndexedDB mechanisms are now typed
   services below `McloneSceneHost`; the old browser policy host is gone. The
   architectural fork is now closed: Tactical 166 — Shared Resident-Tile
   Substrate Slices 1–3 moved synthetic far LOD onto shared workers, admission,
   residency, and region-arena uploads, and Tactical 171 — Convergence And
   Parity Closeout Milestone D proved that path in production browser modes.
   Tactical 166 — Shared Resident-Tile Substrate Slice 4 then landed 4/8/16
   rings, hysteresis, replacement-before-suppress, and cross-level seam proof.
   Next resume Tactical 162 — Real-Chunk LOD Reduction Draft Slice 4, then the
   remaining exact browser feature promotions.
4. **Finish the shared menu surface before adding more menu features.** XR now
   has a shared world-panel pause/options menu with pointer input, and user
   headset validation says it works mostly fine. Flat Android consumes
   `mclone-ui` in code for pause/options touch input and uses the shared touch
   player movement path. Remaining work is automated XR menu coverage, comfort
   tuning, a flat-Android interaction action smoke, and the connect/world-select
   `EditBox` surface. (tactical 089, tactical 090)
5. **Finish the shared input-intent layer.** Unify raw input → intent across
   keyboard/mouse, touch, pointer, and XR controllers, covering **menu-nav,
   pointer, and interact**, not just locomotion. Required for XR interaction and
   for XR interaction and cross-platform menu/pointer parity. Flat Android now
   has shared touch movement/action/hotbar intents. (tactical 076 follow-up)
6. **Keep Android XR remote validation first-class for both USB and LAN.** The
   adapter and Playbox-style launch argv option exist now (`--remote-addr` in
   `mclone.startup.argv`), and Quest smokes passed over direct LAN and through
   the `--adb-reverse` validator path. Keep the LAN route documented as
   firewall-sensitive.
7. **Automate and tune the stereo/world-space UI path** so XR can keep a
   comfortable panel, reticle/pointer, and later a connect screen. The first
   panel renderer and pointer path landed and have user headset validation;
   automated coverage and comfort tuning remain open. (tactical 089)
8. **Protocol: add server push and cross-version negotiation.** Today it is
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
