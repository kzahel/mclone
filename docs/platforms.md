# Platform Direction

This document owns Mclone's current platform posture. It is the entrypoint for
supported client targets, validation
commands, and the boundaries that keep shared engine crates platform-neutral.

The durable engine architecture lives in
[`native-engine-architecture.md`](native-engine-architecture.md). This page is
narrower: app hosts, surfaces, packaging, validation lanes, and cross-platform
contract health.

## Current Status

Mclone currently has five supported client/platform validation lanes, plus
offscreen flat and headset-free stereo validation hosts:

| Target | Status | Validation shape |
|---|---|---|
| Desktop flat | first-class desktop target | `native/apps/mclone-native-client` owns desktop `winit`, surface acquisition/presentation, keyboard/mouse input, frame pacing, and desktop diagnostics. Validated with integrated and remote sessions, locomotion, world/actor/UI rendering, persistence, and chunk loading/generation. |
| Desktop OpenXR | first-class desktop XR target | `mclone-native-client --features xr` owns desktop runtime selection and OpenXR startup. Two run intents share one frame loop: the real `--desktop-xr` verb is a first-class **persistent desktop XR run** (renders the mclone world until you quit, with a 2D companion window carrying status and an OS Close control for a non-headset quit; `--no-window` drops back to headless), while `--xr-clear-smoke` / `--xr-mclone-smoke --frames N` stay the frame-bounded headless CI/liveness gates. Both a companion-window Close and the headset system menu drive one graceful OpenXR shutdown (`tactical/164`). Shared XR crates provide host/session helpers, graphics wrapping, scene alignment, controller locomotion, shared world-panel menu/pointer UI, and shared New World / Join Remote scene replacement. Validated with real stereo mclone terrain on Quest 3 through VirtualDesktopXR (Windows VDXR) and the macOS WiVRn USB lane; user headset validation says the shared XR menu works mostly fine, while an automated replacement-menu headset click smoke remains pending. A headset mirror into the companion window is a deferred follow-up (must reuse an existing per-eye/multiview renderer, not a new per-view path). |
| Android XR / Quest standalone | first-class standalone XR target | `native/apps/mclone-android-xr-client` plus [`../android-xr/`](../android-xr/) own Quest package, Android OpenXR loader, activity glue, asset staging, launch-scoped remote-address argv, and validation. Embedded first-party packs stage under writable internal app data; external app data is a read-only discovery root for ADB-installed reference assets. Validated with all three packs, stereo terrain and actors, controller actions, basic locomotion, remote-dedicated play over direct LAN and through the `--adb-reverse` USB tunnel path, plus launch-scoped in-headset New World replacement smoke. User headset validation says the shared XR menu works mostly fine; an automated controller-click replacement-menu smoke remains pending. |
| Flat Android | first-class mobile target | `native/apps/mclone-android-client` plus [`../android/`](../android/) own the non-XR `NativeActivity`, Vulkan surface/lifecycle, startup properties, and raw touch/input translation over the shared Mono scene host. Validated on the arm64 `jstorrent-tablet` AVD with real terrain/HUD pixels, shared touch movement/look plus sneak, populated frame-pipeline overlay, New World replacement, and background/foreground surface rebuild. The three sentinel lanes are `native:android:avd-smoke`, `:avd-touch-smoke`, and `:avd-session-smoke`. |
| Web/WASM | first-class browser target | `native/apps/mclone-web-client` builds for `wasm32-unknown-unknown` and drives the shared `McloneSceneHost` through a thin rAF/canvas/DOM rim. Local worker, IndexedDB local-world, and remote WebSocket modes share that owner while browser workers, promises, WebGPU targets, mobile controls, and deployment remain platform glue. The full behavioral matrix is documented in [`native-web.md`](native-web.md). |

Additional host lane:

| Host | Status | Notes |
|---|---|---|
| Offscreen flat client | active cleanup | No-window flat-client host for full-frame validation, scripted/network/model input, PNG/video/network/model frame sinks, and future remote UI style use. Current public validation uses the full-frame `--screenshot` offscreen host path with shared `--startup-wait none\|playable\|idle\|frames:N` readiness policy; screenshots default to `idle`, while desktop defaults to `playable`. Older narrow `--headless-ui` / `--headless-chunk` native-client modes are retired. Long-lived host lifetime remains tracked in [`tactical/105-offscreen-flat-client-host.md`](tactical/105-offscreen-flat-client-host.md) and specified in [`offscreen-flat-client.md`](offscreen-flat-client.md). |
| Headset-free XR emulation | active acceptance lane | The default desktop binary feeds fixed-IPD synthetic Stereo views and optional keyboard-translated controller input through `OffscreenDriver` and the same `McloneSceneHost` used by OpenXR. `pnpm native:xr-emulation:smoke` writes a side-by-side capture to `/tmp/mclone-xr-emulation.png`; it does not initialize or depend on OpenXR. This is a render/input/host seam gate, not an OpenXR runtime substitute. |
| Dedicated server | active | `native/apps/mclone-dedicated-server` owns listener/CLI/process lifecycle around the shared authoritative host. TCP and direct WebSocket peers share one registry, autonomous cadence, and per-peer 64-frame / 64 MiB outbound policy. Transient and persistent SQLite-backed worlds are supported. It is not one of the five client display platforms, but it is part of the shared runtime contract. |

The shared warm-world diagnostic is validated in desktop flat and synthetic
per-eye stereo, including A-to-B-to-A selection, no-request performance, paired
process cost, persistence, and asset/device invalidation. Full-frame multiview
pipelines are implemented and eagerly materialized when supported, but the
current macOS adapter does not expose `wgpu::Features::MULTIVIEW` and no Quest
is attached. That execution receipt remains open; it is not replaced by the
headset-free stereo lane.

The first live-diorama composition uses the same offscreen flat and synthetic
stereo hosts. `pnpm native:live-diorama:smoke` rebuilds persistent authored A/B
fixtures, captures A-only plus three shared-depth A+B views and a stereo pair,
and writes its JSON receipt under `/tmp/mclone-live-diorama-smoke`. The
interactive desktop lane uses the same shared scene owner and launch options.
Capable-device multiview execution remains the same named gap.

The first shared product scenario is available from every supported title menu
as `Enter Lobby`. Desktop flat/XR, Android flat/XR, and Web/WASM profiles route
the same path-free action into `McloneSceneHost`, which launches a protected
transient Rust-authored lobby and asynchronously warms its destination diorama.
The destination is the most recent compatible catalog world or a stable
catalog-excluded app-private ordinary world. Native instantiates the lobby in
memory and opens destination SQLite normally; web does the same in two server
Workers with IndexedDB only for the persistent destination. One namespaced
compiler broker serves both draw stores, which reuse one immutable terrain
resource owner. Flat, synthetic-stereo, production-browser, and Android
package/AVD gates pass. No TypeScript scenario selection,
authorship, readiness, placement, or activation policy exists. Tactical
[`178-shared-web-lobby-scenario-parity.md`](tactical/178-shared-web-lobby-scenario-parity.md)
records the completed parity refactor and performance receipts. A new
capable-device multiview scenario receipt remains pending because no headset
was attached at closeout.

That scenario now has one shared live-actor and multiplayer acceptance shape.
The destination server publishes its persisted cow/chicken, the retained
connection's source-local body, and an ordinary dedicated remote player through
the same client replica and placed actor renderer on native and WebGPU. The
checked-in validation-only auxiliary-player script is authored in shared Rust
and drives normal server admission plus `MovePlayer`; browser TypeScript only
forwards its enable bit. `native:lobby-scenario:smoke`,
`native:lobby-scenario:stereo-smoke`, and the three browser lobby commands
assert stable world-qualified identity, exact source-to-composition movement,
walk/light facts, changed pixels, unchanged active actors, activation/return,
and relaunch. Final flat Android APK/AVD, Android XR APK, desktop XR compile,
synthetic stereo, and browser desktop/mobile gates pass. The current Metal
adapter still lacks `MULTIVIEW`, and no headset was attached for a new
capable-device receipt. Tactical
[`179-composable-world-presentation-and-live-preview-actors.md`](tactical/179-composable-world-presentation-and-live-preview-actors.md)
records the completed shared composition and cost evidence.

Linux native/offscreen bring-up was validated on headless Ubuntu 24.04 on
2026-07-12. The native client builds and the full-frame offscreen smoke renders
without X11, Wayland, or a window manager, using either Mesa llvmpipe or a real
Vulkan render node. See [`linux-setup.md`](linux-setup.md) for the reproducible
package, Rust, vanilla-reference, GPU-permission, and smoke sequence.

Asset-pack selection is now a shared cross-platform contract. Desktop,
offscreen, flat Android, desktop/Android XR, and web discover or fetch platform
bytes but use one shared catalog, UI, preference reconciliation, provenance,
and frame-boundary replacement policy. Native platforms persist logical ids in
`preferences/asset-packs.v1.json` beside their client-global world root; web
uses localStorage `mclone.assetPacks.v1`. Current clients still bootstrap epoch
0 from the local reference payload before restoring a first-party preference,
so active proprietary-free provenance does not imply the reference payload was
never installed or fetched.

The retired TypeScript/browser engine is gone from the live tree. Use Git
history only when old behavior context is explicitly needed; retained oracle
helpers and fixtures remain active reference assets.

## Current Direction

All five client targets are equal product surfaces over the shared engine. No
target is the default implementation or validation baseline. New behavior
belongs in shared contracts first, and validation should cover the specific
platform boundaries affected by the change.

Client platform and server host mode are separate axes. Local integrated play
is a useful default for bring-up and offline validation, but every supported
client lane should retain a path to dedicated-server play. Future P2P or
shared-session modes should reuse the same command/update protocol and runtime
contracts, with only the transport/session adapter changing.

Remote networking is one shared semantic lane. Desktop flat/XR and Android
flat/XR select `NativeRemoteServerSession`, whose independent TCP writer/reader
feeds the shared ready-only `ClientConnection` pump. Browser remote uses a
worker-owned WebSocket/decode adapter behind that same pump contract. Platform
rims own endpoint and lifecycle wiring only; they do not own command ordering,
publication polling, ingress budgets, or response bookkeeping.

The platform posture is now validation-backed across the five client targets.
New shared features should be designed against shared contracts first, then
checked through representative gates. Do not require every feature branch to
run every device and headset lane unless the change touches platform adapter,
renderer target/view ownership, OpenXR behavior, Android packaging, browser
worker/ABI glue, or another boundary where that platform can fail uniquely.

The accepted shared client-experience rulebook lives in
[`client-experience-architecture.md`](client-experience-architecture.md). Use
that document when deciding whether behavior belongs in a profile-neutral
client core, a flat/XR/offscreen projection, or a platform adapter.

Near-term product gaps are feature parity and codebase alignment, not more
platform bring-up:

- lighting correctness and render integration still need continued parity work
- menu/HUD/options/loading UI need enough shared Rust/WebGPU coverage to stop
  each platform inventing its own surface
- platform adapters should be thinner around shared runtime/render/session
  contracts
- desktop app gravity should go down, not up: code that heavily grows
  `mclone-native-client` should be treated as a refactor signal unless it is
  genuinely `winit`, desktop surface, keyboard/mouse, CLI, offscreen frame
  sink/source glue, perf harness, or desktop diagnostics glue
- validation should move toward contract tests plus targeted platform smokes
  instead of broad manual matrix checks for every change

## Reference Engine

Use `~/code/playbox` as a pattern library only. Do not depend on it directly,
and do not copy its PhysX/VaM runtime shape into Mclone.

The useful Playbox references are:

| Reference | Use |
|---|---|
| `~/code/playbox/Cargo.toml` | `cdylib` library build shape, `winit` `android-native-activity` feature, Android target dependencies, optimized debug profile policy. |
| `~/code/playbox/src/core.rs` | Shared single-view runtime pattern used by desktop, flat Android, headless, and debug paths. |
| `~/code/playbox/src/android.rs` | Flat Android `NativeActivity` host: lifecycle, Vulkan-only `wgpu`, resume/suspend teardown, resize, redraw loop, touch orbit input, and surface error handling. |
| `~/code/playbox/src/render/mod.rs` and `src/render/targets.rs` | Explicit render view and target boundaries that let desktop, flat Android, headless, and XR hosts feed renderer facts instead of desktop windows. |
| `~/code/playbox/android/` | Flat Android Gradle wrapper, manifest, `cargo ndk` build script, AVD validator, screenshot capture, and logcat fatal scanning. |
| `~/code/playbox/src/xr/` and `~/code/playbox/android-xr/` | OpenXR loader/session/swapchain ownership, per-eye target acquisition, Quest manifest features, startup property/intent validation, and device validation scripts. |

## Architecture Boundary

Platform hosts own:

- event loop and lifecycle
- desktop window, Android activity, browser canvas, or OpenXR session
- offscreen target creation, readback, encoding, and frame delivery for
  no-window hosts
- `wgpu` surface/swapchain acquisition and presentation pacing
- OpenXR swapchain image acquisition/release where applicable
- platform input collection and translation
- platform storage and asset-source selection
- app package scripts and device/headset validation

Shared engine crates own:

- protocol and client/server session facts
- authoritative runtime, chunk scheduling, and chunk publication
- host-mode-neutral client/server command and update contracts
- client replica, movement/input intent, and interaction state
- asset parsing and packed asset source abstractions
- render-section meshing, dirty/cache policy, and compile scheduling
- renderer resources and frame drawing from explicit view/target facts
- shared Rust/WebGPU UI model and draw list

Shared app/runtime boundary crates currently include:

- `mclone-app-runtime`: shared single-view runtime helpers used by desktop,
  flat Android, current headless captures, future offscreen flat client, and XR
  terrain runtime construction; it also owns the single client-side
  `FramePipelineAccountant`, neutral queue/peer report builders, and shared
  text/JSON presentation consumed by flat, XR, and perf hosts
- `mclone-render-session`: shared render-section dirty state, compile request,
  cache update, neighbor-readiness, and camera-controller contracts used by
  desktop and web, and consumed by XR scene code
- `mclone-xr-host`: shared OpenXR event/session/frame/action/view helpers used
  by desktop XR and Android XR. `OpenXrFrameDriver` exclusively owns
  poll/wait/begin/render-or-skip/end sequencing; app handlers supply only their
  platform event pump, concrete render targets, and observation hooks.
- `mclone-xr-graphics`: shared unsafe Vulkan/OpenXR/`wgpu` graphics bridge used
  by desktop Vulkan XR and Android XR
- `mclone-scene`: shared mono/stereo/multiview terrain/actor scene, startup
  view-pose alignment, controller-to-engine locomotion, and XR frame render
  topology selection. Its cross-platform core is `McloneSceneHost` configured
  by `McloneSceneHostOptions`; desktop, Android, XR, and web drivers all use it.

Core shared crates must not depend on:

- `winit`
- `android-activity`, JNI, or Android package paths
- DOM, `web_sys`, or browser workers
- OpenXR sessions, action sets, or swapchains
- platform filesystem locations such as Android app files

The app/platform XR crates may depend on OpenXR and `wgpu`, but they must not
own simulation rules, chunk scheduler policy, private renderers, or platform
activity/window glue. Android-specific activity/JNI/Horizon behavior stays in
the Android XR app. Desktop runtime selection and launch helpers stay in the
desktop app/scripts.

`mclone-native-client` is allowed to be larger than other app crates because it
owns the primary `winit` loop, desktop input, current headless/offscreen CLI
entrypoints, perf harnesses, CLI options, and desktop diagnostics. Size alone is
not a bug, but
new desktop-local gameplay, renderer policy, runtime startup policy, UI state,
session lifecycle policy, persistence behavior, or input semantics are bugs
unless they are temporary forks tracked in the platform parity matrix.
The live desktop redraw path is thin: its `WinitFrameDriver` owns the
surface/depth/presentation rim and delegates session, input application,
runtime/render orchestration, and screen-space UI assembly to `mclone-scene`.
The native desktop and offscreen lanes no longer have an app-local flat
orchestrator. `WinitFrameDriver` and `OffscreenDriver` both delegate session,
input application, runtime/render orchestration, and UI assembly to
`mclone-scene`. Flat Android's `AndroidSurfaceDriver` delegates the same policy
to the Mono host and retains only Android lifecycle/surface/raw-input work.
`OffscreenDriver` selects Mono or synthetic Stereo target ownership for
screenshot, perf, and headset-free XR-emulation lanes. OpenXR cadence and
session sequencing stay in `mclone-xr-host::OpenXrFrameDriver`.

The native app adapters are source-scanned by
`pnpm native:thin-adapters:purity`. The gate rejects app-local settings/session
dispatch, render-section synchronization/upload policy, engine-camera literals,
and low-level OpenXR frame sequencing, then runs the scene-host and XR-driver
dependency/ownership purity checks. Browser host adoption remains explicitly
deferred and is not included in this native-only gate.

`mclone-render` may depend on `wgpu` and own GPU resources, but host-facing
entry points should continue to accept explicit render target and view data.
This is already true through `RenderFrameContext`, `RenderFrameTarget`,
`ChunkRenderView`, `ChunkRenderTarget`, and the XR render-view descriptors.

## Host Shapes

Mono and synthetic-stereo hosts:

```text
platform input/lifecycle
  -> platform adapter
  -> shared client/runtime/render-session state
  -> explicit Mono(view) or Stereo([view; 2]) targets
  -> mclone-render
```

This includes desktop flat, offscreen flat, headset-free XR emulation, flat
Android, and the web canvas path. The app shells are not identical: desktop
owns native threads and
keyboard/mouse, offscreen owns synthetic/network/model input and frame sinks,
Android owns `NativeActivity` lifecycle and touch, and web owns browser workers
and canvas APIs. The convergence point is shared runtime/render state and
explicit frame facts. The server host mode is independent from that platform
shell: local integrated and remote dedicated should differ by session/transport
adapter, not by private client, simulation, or render-session logic.

Stereo XR hosts:

```text
OpenXR runtime/actions/swapchains
  -> platform XR adapter
  -> shared XR host/graphics/scene helpers
  -> shared client/runtime/render-session state
  -> explicit per-eye render views + targets
  -> mclone-render
```

Desktop XR and Android XR should continue to share OpenXR host/session/action
and terrain-scene behavior where possible. They should diverge only at runtime
discovery, Android loader/activity glue, packaging, headset wake/restore, and
other true platform concerns.

Both XR adapters query compositor refresh through `mclone-xr-host` and pass the
result to the shared `mclone-scene` render-admission policy. Static Quest upload
limits remain optional clamps on that shared adaptive grant, not a separate
platform controller.

`mclone-input::GamepadInputAdapter` and `GamepadBindings` are retained as a
shared contract as of 2026-07-10. No current native adapter advertises gamepad
capability or synthesizes support; adoption requires a real desktop, browser,
or Android event source plus device validation.

## Validation Policy

Every supported platform lane has an executable gate. Use `/tmp` for screenshots
and logs.

For broad pre/post-refactor sweeps across desktop flat, headless, desktop XR,
flat Android, Android XR, and web, use
[`platform-sanity-checklist.md`](platform-sanity-checklist.md). That checklist
is the in-depth runbook for proving the lanes are currently drivable before
attributing later failures to a refactor.

Recommended default gates:

```bash
cargo test --manifest-path native/Cargo.toml
pnpm native:thin-adapters:purity
pnpm native:desktop-offscreen:smoke
pnpm native:web:build
```

The thin-adapter command includes the enforced browser scene-host adoption
inventory; it is the default source-shape gate for every display client.

`native:desktop-offscreen:smoke` is the current full-frame offscreen
flat-client smoke. It runs the real offscreen host path and writes its screenshot
under `/tmp`.

Far-LOD changes must additionally run the checked-in four-waypoint settle gate:

```bash
pnpm native:lod-settle:smoke
```

It writes per-waypoint PNGs and `lod-settle-report.json` under
`/tmp/mclone-lod-settle-smoke`, requires zero pending lifecycle work and exact
set coherence. Its top-down assertions read GPU depth at the expected real or
LOD surface across all 441 RD4 + range-6 chunks; RGB/screenshot interpretation
is not coverage evidence. The high→spawn→high revisit must also remain
pixel-identical.

Run the full movement/configuration matrix when changing the harness, desired
LOD bands, toggle/range behavior, or lifecycle transitions:

```bash
pnpm native:lod-settle:probe
```

It produces 14 captures and three reports under
`/tmp/mclone-lod-settle-probe/{smoke,movement,mutations}`. The movement script
pins desired levels across one-chunk, hysteresis/band, and eight-chunk moves,
then interpolates those poses into a paced 220-frame flight plus a 120-frame
stationary tail. GPU depth is checked at the expected representation surface
for every configured chunk on every smooth frame, with first-failure lifecycle
rows and per-chunk missing age retained in the report; no transient missing
frame is allowed;
the mutation script pins complete off teardown, deterministic on
repopulation, and range-3/range-8 coverage counts.

Platform-specific gates:

```bash
# Desktop flat
pnpm native:movement:smoke
pnpm native:timedemo:smoke

# Headset-free synthetic stereo
pnpm native:xr-emulation:smoke

# Desktop OpenXR, headset/runtime required
pnpm native:xr:check
pnpm native:xr:windows:smoke:connected
pnpm native:xr:windows:mclone:connected
# Real persistent desktop XR run (companion window; quit via window Close or headset menu)
pnpm native:xr:mac:wivrn:desktop     # macOS WiVRn USB
# scripts\start-desktop-xr.bat        # Windows VDXR (pnpm native:xr:windows:interactive)

# Flat Android, SDK/AVD required
pnpm native:android:apk
pnpm native:android:apk:avd
pnpm native:android:avd-smoke -- --skip-build
pnpm native:android:avd-touch-smoke -- --skip-build
pnpm native:android:avd-session-smoke -- --skip-build

# Android XR / Quest, attached authorized Quest required
pnpm native:android-xr:apk
pnpm native:android-xr:validate --skip-build --view-pose 0,120,-96,180 --seed 12345 --chunk-x 0 --chunk-z 0 --render-distance 2 --day-time 6000 --freeze-time
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke
pnpm native:android-xr:terrain-multiview-proof
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:validate --debug --skip-build --adb-reverse --start-server --view-pose 0,120,-96,180
pnpm native:android-xr:validate --debug --skip-build --start-server --server-listen 0.0.0.0:25565 --remote-addr HOST:25565 --view-pose 0,120,-96,180

# Web/WASM
pnpm native:web:smoke
pnpm native:web:thread-smoke
pnpm native:web:canvas-smoke
pnpm native:web:chunk-smoke
pnpm native:web:app-smoke
pnpm native:web:catalog-smoke
pnpm native:web:mobile-smoke
pnpm native:web:asset-pack-smoke
pnpm native:web:block-edit-probe
pnpm native:web:movement-perf
pnpm native:web:remote-smoke
pnpm native:web:lobby-scenario-smoke
pnpm native:web:lobby-scenario-mobile-smoke
pnpm native:web:lobby-scenario-lifecycle-smoke

# Strict standalone first-party resolution audit
pnpm --silent assets:validate:first-party \
  > /tmp/mclone-first-party-provenance.json
```

Run the narrowest lane that can catch the bug class:

- simulation/content changes: shared Rust tests and oracle fixtures, plus each
  affected app or platform contract gate
- renderer/view/target changes: the smallest rendered-output gate that reaches
  the changed path, plus any uniquely affected browser, device, or headset lane
- shared runtime/render-session changes: shared tests plus the app targets whose
  cadence, lifecycle, worker, surface, or view contracts are affected
- Android activity/package changes: the relevant Android APK and validation
  lane
- OpenXR host/graphics/action changes: desktop XR and Android XR compile gates;
  run at least one real headset lane before treating the change as validated
- browser worker/ABI changes: web typecheck and smoke lanes; unrelated device
  gates are not required

## Adapter Footprint Audit

The platform parity tracker owns the semantic truth, but a quick footprint
audit helps catch desktop gravity before it becomes a design signal. Run this
audit when a slice adds substantial app-local code, touches
`mclone-native-client`, or introduces a temporary platform fork:

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

Use the output as a prompt, not a hard budget. Escalate to shared-contract
cleanup when:

- a feature adds logic to `mclone-native-client` that another display lane will
  need
- desktop startup/render/input code owns policy instead of adapting platform
  facts into shared contracts
- a blocking desktop helper also exists in `mclone-app-runtime` or XR scene
  startup and should become a shared progress/pump contract
- Matrix 2 in [`topics/platform-parity.md`](topics/platform-parity.md) would
  need a new `⚑` fork but no tactical records the convergence path

## Recommended Alignment Work

Highest-value next steps to keep features from requiring constant full-matrix
manual checks:

1. **Reduce desktop app gravity before adding more desktop-local behavior.**
   Treat new growth in `mclone-native-client` as suspicious unless it is true
   `winit`/surface/input/CLI/offscreen/perf/diagnostic glue. Startup loops,
   loading/progress state, session lifecycle, render policy, input semantics,
   HUD/menu behavior, persistence, and gameplay should move into shared owners
   before more features build on desktop-local versions.
2. **Keep the platform parity tracker current.** The feature/platform and
   shared-contract matrices live in
   [`topics/platform-parity.md`](topics/platform-parity.md). Update those cells
   when a slice changes platform capability or shared-boundary ownership.
3. **Make the contract matrix more executable.** For each shared crate boundary,
   keep the sentinel smoke/test close to scripts so platform coverage is
   deliberate instead of remembered manually.
4. **Finish connect/world-select UI and automate XR replacement/menu smoke.**
   All display lanes now have shared initial local/remote session identity and
   replacement code paths through `mclone-app-runtime::session`. Flat Android
   New World replacement is covered by an AVD touch-menu smoke, and Android XR
   New World replacement is covered by an in-headset launch smoke against the
   same shared XR replacement method. User headset validation says the shared
   XR menu/pointer works mostly fine. Remaining work is an automated XR
   controller-click replacement/menu smoke, a web Join Remote connect-screen
   smoke, and shared `mclone-ui` text input so users can choose endpoints/worlds
   in app instead of through CLI/properties/query params.
5. **Finish web host-lifecycle convergence and keep Android XR remote validation repeatable.**
   Native desktop, desktop XR, flat Android, and Android XR now share
   `mclone-app-runtime` host-mode and native scene-shell contracts where
   applicable. Browser remote networking is worker-owned and semantically
   converged, but web still has an async `WebRuntimeHost` lifecycle shell.
   Android XR remote works over LAN after host firewall allow and through the
   first-class `--adb-reverse` validator path.
6. **Keep XR scene convergence complete as features grow.** `mclone-scene`
   now owns shared terrain/actor rendering, startup pose, locomotion, and
   local/remote-capable host shape. Keep future UI, comfort, and interaction
   work behind that shared scene boundary instead of reintroducing app-local XR
   forks.
7. **Promote lighting and UI as shared feature contracts.** Lighting and
   menus/HUD/options/loading UI are the next user-visible parity blockers.
   Land them once through shared data/UI/render contracts instead of per
   platform paths.
8. **Add adapter conformance tests.** Prefer tests for render-target/view
   descriptors, asset-source discovery, input intent mapping, and render-section
   compile contracts over running every device for every feature branch.
9. **Keep device/headset smokes as boundary sentinels.** Run full Android,
   Quest, and desktop XR validation when touching platform glue, packaging,
   OpenXR session/swapchain/action code, graphics wrapping, or shared contracts
   they uniquely exercise.

## Tactical Links

- Flat Android: [`tactical/074-flat-android-build-smoke.md`](tactical/074-flat-android-build-smoke.md)
- Shared single-view runtime prerequisite: [`tactical/075-shared-single-view-runtime-prereq.md`](tactical/075-shared-single-view-runtime-prereq.md)
- XR frontload sequence: [`tactical/076-native-xr-frontload-plan.md`](tactical/076-native-xr-frontload-plan.md)
- Multi-view render contract: [`tactical/077-multiview-render-contract.md`](tactical/077-multiview-render-contract.md)
- Desktop OpenXR clear/frame/controller/locomotion: [`tactical/078-desktop-openxr-clear-smoke.md`](tactical/078-desktop-openxr-clear-smoke.md), [`tactical/079-desktop-openxr-mclone-frame.md`](tactical/079-desktop-openxr-mclone-frame.md), [`tactical/081-desktop-openxr-controller-actions.md`](tactical/081-desktop-openxr-controller-actions.md), [`tactical/082-desktop-xr-player-locomotion.md`](tactical/082-desktop-xr-player-locomotion.md)
- Android XR / Quest standalone: [`tactical/083-android-xr-quest-standalone.md`](tactical/083-android-xr-quest-standalone.md)
- Shared session coordinator: [`tactical/095-shared-session-coordinator.md`](tactical/095-shared-session-coordinator.md)
