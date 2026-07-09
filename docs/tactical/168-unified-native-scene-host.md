# 168: Unified Native Scene Host

Status: approved direction 2026-07-09; Slice 0 (bug-grade parity pre-fixes)
landed 2026-07-09; Slice 2 (web-shaped runner-generic seam) landed 2026-07-09;
Slice 1 (OpenXR-free scene crate + `mclone-xr-scene` -> `mclone-scene` rename)
landed 2026-07-09. Slices 3+ not started; Slice 3 requires Slice 1 (done) and
Slice 4 requires Slice 1 (done). (Slices 0–2 are independent per the sequencing
guardrail, so Slice 2 landed ahead of Slice 1.)

Workstream: native Rust shared runtime convergence. This tactical collapses the
four native client runtimes (desktop flat, desktop XR, flat Android, Android
XR/Quest) plus the headless/offscreen lanes onto **one shared scene host** with
thin per-platform drivers. It is the frame-loop-and-everything-else sequel to
[`167`](167-shared-session-startup-contract.md) (which already unified startup)
and the structural completion of [`165`](165-native-feature-parity-baseline.md)
(which enforces feature parity but cannot see wiring-level forks).

## Principle: delete, don't port

The most mature orchestrator in the tree is the XR scene state
(`mclone-xr-scene::XrMcloneTerrainState`). Both XR apps are already thin over
it, which is why desktop XR and Quest cannot diverge on gameplay semantics.
The flat family has no equivalent: `FlatClientDriver` lives inside the desktop
app crate, and flat Android re-implements the whole loop by hand, worse.

The resolution is **not** to refactor each copy toward each other. It is to
generalize the mature XR host into a platform-neutral scene host and **delete**
the flat orchestrators outright. Flat rendering becomes the one-view case of
the same host. The only code that survives per platform is genuinely thin
glue: OS/activity/window/surface/swapchain ownership, raw input events, frame
cadence, asset roots, logging.

Target mental model:

```text
McloneSceneHost<S>   (one crate, no openxr, no winit, no android deps)
  owns: session runtime + startup pump (167), camera reconcile, interest,
        budgeted section sync/upload admission, client-experience effects,
        world catalog + session replacement, UI host + HUD assembly,
        locomotion/input application, audio events, frame accounting,
        lifecycle policy (background => save)
  renders: views-as-data — Mono(view) | Stereo([view;2]) | StereoMultiview

thin frame drivers (one per cadence source, not per product):
  OpenXrFrameDriver   (mclone-xr-host)  — xrWaitFrame cadence, xr::View -> neutral views
  WinitFrameDriver    (desktop app)     — redraw/vsync cadence, window swapchain
  AndroidSurfaceDriver(android app)     — AndroidApp events + surface
  OffscreenDriver     (tooling)         — texture target: headless, screenshot, perf

thin platform adapters: raw input -> mclone-input contract, lifecycle events,
  asset roots, logging, storage roots
```

A flat build can then feed **synthetic stereo poses** through the same host —
an XR-emulation lane for developing XR features without a headset. That is an
explicit acceptance test of this tactical (Slice 9), not a stretch goal.

## Evidence: the current duplication (audited 2026-07-09)

Five surfaces, four orchestrators, one of them shared:

- Desktop flat: `native/apps/mclone-native-client/src/flat_client_driver.rs`
  (~3,775 lines) + `app.rs`. Owns its own per-frame loop, effects wiring,
  session-replacement state machine, HUD assembly, adaptive render admission.
- Flat Android: `native/apps/mclone-android-client/src/lib.rs` (~3,372 lines).
  Re-implements the flat loop from scratch; **missing** the render-admission
  budget, all frame-pipeline instrumentation, travel assist, thruster/hand-push
  emulation, and uses raw `pick_block` where desktop uses `target_block`.
- Desktop XR: `native/apps/mclone-native-client/src/desktop_xr.rs` (~1,268
  lines) — thin over `mclone-xr-scene`, but hand-rolls the OpenXR session loop.
- Android XR/Quest: `native/apps/mclone-android-xr-client/src/lib.rs` (~7,661
  lines) — thin over `mclone-xr-scene` for gameplay, but hand-rolls the OpenXR
  session loop **five more times** (main + proof/perf harnesses) and carries
  ~2,500 lines of frame-report formatting duplicating shared builders.
- Headless/offscreen/perf (`headless.rs`, `offscreen_flat_client.rs`,
  `perf.rs`): partly on the desktop driver, partly third copies of frame-input
  assembly.
- Web (`native/apps/mclone-web-client`): a fourth copy of the host wiring in
  `web_canvas.rs` (~6,430 lines). Explicitly **not migrated** by this tactical
  (see Web posture), but the host must stay web-adoptable.

Concrete duplication clusters (file:line spot checks, verified 2026-07-09):

1. **Client-experience effect application** — ~24 identical
   `ClientExperienceSettingEffect::` match arms copied four ways:
   `flat_client_driver.rs:1195`, `mclone-android-client/src/lib.rs:1509`,
   `mclone-xr-scene/src/session.rs:1080`, `web_canvas.rs`. Capability
   projection is byte-identical in the same four places
   (`flat_client_driver.rs:1403`, `android lib.rs:1710`, `session.rs:1248`).
   UI-action orchestration, catalog-request wrappers, and session-effect
   plumbing follow the same 3–4-way copy pattern.
2. **OpenXR session loop** — the poll -> idle/exit -> `wait_begin_frame` ->
   render-or-skip -> end -> report skeleton exists six times: desktop
   `desktop_xr.rs:415` (`run_smoke_frames`), android-xr `lib.rs:3304`
   (`run_mclone_frame_loop`), `:2370`, `:2667`, `:3020` (harness loops), plus
   the per-frame `render_mclone_frame` twins (`desktop_xr.rs:972`,
   android-xr `lib.rs:7020` and `:7145`).
3. **Frame-report presentation** — stage-span/queue/peer-panel builders exist
   in shared `mclone-xr-scene/src/frame_pipeline_reporter.rs:119-349` and are
   re-implemented in android-xr `lib.rs:5502-5774` and again in desktop
   `perf.rs:2995-3284`. Two accountants (`FramePipelineAccountant` in
   app-runtime, `XrFramePipelineReporter` in xr-scene) emit the same
   `FramePipelineReport` (165 Slice 2c tracks their convergence).
4. **Remote session adapter** — `RemoteServerSession` +
   `remote_batch_from_native` copied verbatim three times:
   `mclone-native-client/src/remote_session.rs:9-108`,
   `mclone-android-client/src/lib.rs:2654-2749`,
   `mclone-android-xr-client/src/lib.rs:2271-2368`.
5. **SessionStartRequest dispatch** — the same 4-variant match hand-rolled in
   `desktop_xr.rs:903`, android-xr `lib.rs:2136`, android `lib.rs:2493`, while
   desktop flat uses an unrelated pending-start state machine
   (`app.rs:928-948`, `flat_client_driver.rs:1470-1740`).
6. **Input assembly** — `engine_camera_input_from_flat_frame` duplicated with
   semantic drift (`flat_client_driver.rs:3750` vs android `lib.rs:3303`;
   Android drops `hand_push_emulation`/`thruster_emulation` and `mouse_delta`).
   Key tables and bool->`FlatInputAction` tables duplicated and drifted
   (android `lib.rs:3184-3211`, `:3278-3286`). `mclone-input`'s
   `GamepadInputAdapter` and `TouchBindings` have **zero** users — Android
   hand-rolled its own touch layer instead (`lib.rs:276-398`).
7. **Camera reconcile** — shared `mclone_app_runtime::camera_reconcile` is used
   per-frame by flat surfaces only; XR runs a timed structural fork
   (`mclone-xr-scene/src/session.rs:1437-1540`) whose only difference is
   `XrCameraCommitTiming` instrumentation.

Behavior divergences that exist today because of the forks (fixed in Slice 0):

- **Sprint and sneak are unreachable on both XR surfaces**:
  `xr_locomotion_input_from_controllers`
  (`mclone-xr-scene/src/locomotion.rs:135-147`) builds
  `EngineCameraInput { .. ::default() }` and never sets `sprint`/`shift`. The
  shared player fully supports both. No capability flag exists, so 165's
  enforcement tests are blind to it.
- **Quest XR loses world edits on OS kill after Pause**: saving is Drop-driven
  only (`shutdown_persistence` -> `save_dirty_chunks` is the sole save path;
  there is no autosave anywhere). Flat Android drops the runtime on every
  `suspended()` (`android lib.rs:2798-2804`) and therefore saves; Android XR
  handles `MainEvent::Pause`/`Stop` with only a log line
  (`android-xr lib.rs:7511-7519`) and saves only on `Destroy`.
- **Flat Android block interaction bypasses targeting rules**: desktop uses
  `camera.target_block` (inside-solid suppression + outline check,
  `mclone-client/src/interaction.rs:81-106`); Android uses raw `pick_block`
  (`android lib.rs:1953`).
- **HandPush/Thruster flight emulation silently off on flat Android**
  (`flat_client_driver.rs:3771-3772` vs android `lib.rs:3320`).

### Frame-budget scoping (important correction — three layers, two shared)

Do not "port frame budgeting to XR" — most of it is already shared:

1. **Terrain-generation / chunk-publication budgeting** (tactical 150) lives in
   the **server scheduler** (`mclone-server/src/scheduler.rs:551,566`,
   `chunk_publication_budget_controller_config`). It runs inside the integrated
   server on every surface, including Quest. Out of scope here.
2. **Render compile capacity** is shared
   (`mclone-app-runtime/src/render_compile_capacity.rs`). Out of scope.
3. **Client-side render-section upload admission** is the forked layer:
   desktop flat has the adaptive `DesktopRenderBudgetController`
   (`flat_client_driver.rs:186-236`; EWMA cost estimator +
   `decision_for_family(RenderAdmission)` grants from measured frame headroom);
   XR has a **static** configured cap (`RenderSectionUploadFramePolicy`,
   `mclone-xr-scene/src/lib.rs:1757`, budget from
   `options.rs:195`) plus a pre-sync drain rule; flat Android has **nothing**
   (plain `sync_render_sections`). Slice 6 absorbs the adaptive controller
   into the host as the single policy; the static cap becomes an optional
   override knob, not a separate mechanism.

## Target architecture

### The host crate

Generalize `mclone-xr-scene` into **`mclone-scene`** (rename lands in Slice 1).
The crate must end this tactical with **no `openxr`, `winit`, or Android
dependencies**. Its OpenXR coupling is already rim-only: `render_frame` takes
`[xr::View; 2]` and immediately converts to internal render views
(`mclone-xr-scene/src/lib.rs:320-429`, `self.render_views(&views)`); ~17
signatures mention `xr::View`/`xr::Fovf` and nothing deeper does.

`McloneSceneHost<S: RemoteDedicatedServerSession>` (the generalized
`XrMcloneTerrainState`) owns, once:

- `NativeSessionRuntime<S>` + `NativeSessionStartupPump` consumption (167 —
  consume the shared pump; never fork it).
- Camera reconcile (absorbing the XR timed fork by letting the shared helpers
  emit optional timing).
- Per-frame orchestration: poll -> apply pending corrections -> budgeted
  section sync -> upload/apply -> compile-job release -> traversal-ready ->
  frame-input assembly (sky/time/sun/far-lod) -> render for the active view
  topology.
- View topology as data: `Mono(view)`, `Stereo([view; 2])`,
  `StereoMultiview`. Mono renders through the existing shared
  `render_full_frame_for_view*` (`mclone-app-runtime/src/frame_render.rs:873+`).
- Client-experience effect application: ONE copy of the setting-effect match,
  with a small `HostEffects` (name illustrative) trait for the few real
  platform hooks (mouse lock, present-mode/frame-pacing hint, quit). A new
  effect that a platform forgets to handle must be a compile error, not a
  silent desktop-only feature.
- World catalog + `SessionStartRequest` -> runtime factory (one copy of the
  dispatch; desktop's pending-start machine is deleted, not generalized).
- UI host + HUD assembly; screen-space vs world-quad UI is a presentation
  strategy on the host, not two stacks.
- Single frame accountant emitting `FramePipelineReport` (converge
  `FramePipelineAccountant` / `XrFramePipelineReporter` — coordinate with 165
  Slice 2c; do not do the same work twice).
- Adaptive render-admission policy (Slice 6).
- Lifecycle policy: `on_background()` saves/tears down consistently; a
  platform reports lifecycle events, it does not choose persistence policy.

### What is allowed to stay per platform (the thin-adapter budget)

Frame cadence source; surface/swapchain/eye-target acquisition; raw input
events -> `mclone-input` contract; pose source (tracked head vs engine camera);
asset/storage roots; logging; activity/window lifecycle event delivery;
graphics binding glue (`graphics_metal.rs`, `graphics_vulkan.rs`,
`perf_metrics.rs` for `XR_META_performance_metrics`). Nothing else. If a
behavior differs between surfaces it must be host-mode evidence, an explicit
named policy value, or a reason-bearing 165 ledger entry — never a second copy
of orchestration.

### Web posture: seams now, migration later (explicitly out of scope)

Web is the reason the runtime count exploded, and it is NOT migrated in this
tactical. But web already shares everything below orchestration
(`mclone-server`, `-client`, `-ui`, `-input`, `-render-session`,
`-app-runtime`; `WebIntegratedServerRunner` in `web_server_worker.rs:137`
already implements the shared `IntegratedServerRunner` trait from
`mclone-server/src/runner.rs:471`). The single structural seam that forked web
is that `NativeSessionRuntime` hard-codes the concrete
`NativeIntegratedServerRunner` (`native_session_runtime.rs:275`) instead of the
trait. Rules that keep the host web-adoptable at near-zero cost:

1. Local host mode becomes generic over `IntegratedServerRunner` (Slice 2).
2. The host's frame/startup API stays non-blocking and `step()`-based;
   blocking `drive_to_ready*` convenience loops live in native drivers only.
3. No bare `std::thread` or `std::time::Instant` in the host — use the
   existing wasm cfg patterns in `mclone-app-runtime` (`lib.rs:2881+`).
4. `cargo check -p mclone-web-client --target wasm32-unknown-unknown` stays in
   every slice's validation.
5. Web feature divergences remain reason-bearing via the 165 web ledger (e.g.
   a threads/SIMD-hungry physics engine would be a recorded web exception;
   rapier would need none).

Web adoption of the host (rAF driver, WebSocket `S`, worker runner,
`web_canvas.rs` collapse) is a follow-up tactical once the native shape has
settled.

## Non-goals

- No web migration (see above). Web keeps its duplicated orchestration for now.
- No new wire protocol / server-push work (owned by 133/151).
- No changes to server-side scheduler budgeting (150) or the resident-tile
  substrate direction (166).
- No re-introduction of resident CPU meshes (163) and no forking of the 167
  startup pump — the host consumes it.
- Do not move window/activity/canvas/OpenXR-session/socket ownership into the
  host crate.
- Do not force uniform input-axis capabilities (crosshair/turn/touch/pacing may
  vary per 165); this tactical unifies *ownership*, not input hardware.

## Implementation slices

Every slice must independently satisfy the **cross-slice guardrails** at the
bottom of this doc. Statuses below are all "not started".

### Slice 0: Bug-grade parity pre-fixes — DONE (2026-07-09)

Goal: fix the behavior divergences that exist today, before any structural
work, so later slices migrate correct behavior instead of enshrining bugs.
Each fix is small and independently landable.

Landed decisions (see deliverables below for the reasoning):

- **XR sprint/sneak bindings.** Sprint = left-hand **Y button** (`y_pressed`);
  the suggested left-thumbstick click was unavailable because it is already the
  game-UI toggle (`XR_GAME_UI_TOGGLE_HAND`). Sneak = **right thumbstick click**
  (`thumbstick_pressed`), which does not collide with snap-turn (the right-stick
  *axis*). `EngineCameraInput` in `xr_locomotion_input_from_controllers_*` is now
  spelled exhaustively (no `..default()`); three unit tests lock the bindings.
- **Quest save-on-pause.** Implemented as an explicit synchronous flush (option
  2), not session teardown/rebuild (option 1), to avoid regressing the
  in-headset resume experience. New `NativeRunnerControl::FlushPersistence`
  (ack-gated, blocking) → `NativeIntegratedServerRunner::flush_persistence` →
  `NativeSceneRuntime::flush_persistence` (remote host modes no-op) →
  `XrMcloneTerrainState::flush_persistence`; android-xr flushes on
  `MainEvent::Pause`/`Stop` from inside the frame loop
  (`poll_android_lifecycle`), logging `MCLONE_ANDROID_XR_PAUSE_SAVE`.

Validation status: full compile/test battery green (fmt, workspace check, 77
xr-scene tests + app-runtime/native-client/server, wasm check, `cargo ndk`
check for both Android apps). Device: Quest session-smoke drew terrain
(sections=123, drawn_sections=32) with no regression; flat Android AVD session +
touch smokes drew terrain and ran clean. The Quest Pause→force-stop→relaunch
retains-blocks acceptance still needs a **worn-headset manual pass** (the VR
compositor terminates the window when the headset is off-head, so the app cannot
be driven into the frame loop remotely); the flush path is compile- and
logic-verified and fires from the lifecycle pump.

Deliverables:

1. **XR sprint + sneak.** Wire `sprint` and `shift` into the
   `EngineCameraInput` built at `mclone-xr-scene/src/locomotion.rs:135-147`.
   This needs a controller-binding decision (suggested: left-thumbstick click
   = sprint, matching common VR Minecraft bindings; pick a sneak binding that
   does not collide with snap-turn on the right stick). If a binding is
   genuinely deferred, record a reason-bearing exception in the 165 ledger
   style instead of leaving the silent `..default()` drop. Additionally,
   change the construction so dropped fields can't recur silently: build
   `EngineCameraInput` with exhaustive field spelling or a shared constructor
   (no bare `..Default::default()` for gameplay-semantic fields).
2. **Quest XR save-on-pause.** World edits must survive activity
   Pause -> process kill on Android XR. Acceptable minimal implementations:
   trigger the same drop-driven save flat Android gets (teardown/rebuild the
   session on Pause/Resume), or add an explicit save/flush request on the
   shared runner and call it from `MainEvent::Pause`
   (`mclone-android-xr-client/src/lib.rs:7511-7519`). Prefer whichever is
   least likely to regress the in-headset resume experience; record the
   choice.
3. **Flat Android `target_block`.** Replace `pick_block` with `target_block`
   at `mclone-android-client/src/lib.rs:1953` to match desktop's inside-solid
   suppression and outline gating.
4. **Flat Android thruster/hand-push emulation flags.** Set
   `hand_push_emulation`/`thruster_emulation` in the Android copy of
   `engine_camera_input_from_flat_frame` (`lib.rs:3303-3322`) to match desktop
   (`flat_client_driver.rs:3771-3772`). (The copy itself dies in Slice 8; this
   keeps behavior right until then.)

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene -p mclone-app-runtime -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client -p mclone-android-xr-client
# Quest 3 is attached to this machine:
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke   # sprint/sneak + pause-save on device
# Flat Android tablet AVD (jstorrent-tablet):
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-session-smoke
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-touch-smoke
```

Exit criteria: sprint/sneak observable on Quest (log or on-device movement
speed), a Pause -> force-stop -> relaunch cycle on Quest retains placed/broken
blocks, Android AVD session smoke still draws terrain.

### Slice 1: OpenXR-free scene crate (`mclone-xr-scene` -> `mclone-scene`) — DONE (2026-07-09)

Goal: neutralize the rim. Behavior-free.

Landed shape:

- Host-neutral view types live in `mclone-xr-host` (which keeps the `openxr`
  rim): `XrFov` (four tangent half-angles) and `XrView` (a validated
  `XrViewPose` + `XrFov`), each with a `from_openxr` conversion. The scene
  crate's public render/locomotion/teleport/comfort signatures take
  `XrView`/`XrFov` instead of `xr::View`/`xr::Fovf`. `render_view_from_world_pose`,
  `xr_fov_to_projection_rh`, and `xr_fov_aspect` now take neutral `XrFov`.
- The `openxr` -> neutral conversion happens at the OpenXR rim: the two XR apps
  (`desktop_xr.rs`, `android-xr/lib.rs`) convert their located
  `XrStereoFrameViews` with `XrView::from_openxr` at the call boundary before
  handing views to the scene. Pose finiteness validation therefore moved from
  inside the scene functions to that boundary (behavior-equivalent).
- `openxr` removed from the scene crate's `Cargo.toml`. The scene crate still
  depends on `mclone-xr-host` for the neutral types and controller snapshots;
  fully shedding that transitive `openxr` link is a later-tactical (web
  adoption) concern, not a Slice 1 deliverable.
- Crate directory + package renamed `mclone-xr-scene` -> `mclone-scene` as a
  **separate mechanical commit** (imports/paths only): workspace member,
  dependency declarations in both consuming apps, and the `mclone_xr_scene`
  import paths. Two commits total: rim neutralization, then rename.

Tripwires (all pass): `grep -rn "openxr\|xr::" native/crates/mclone-scene/`
returns nothing; the scene test suite (77 tests) passes unchanged under the new
name; no `xr::` type appears in any `pub fn` signature of the scene crate.

Validation (all green 2026-07-09): `cargo fmt --all --check`, workspace
`cargo check`, `cargo test -p mclone-scene -p mclone-app-runtime
-p mclone-native-client` (77 scene + 185 + 179 + xr-host, 0 failed),
`cargo check -p mclone-web-client --target wasm32-unknown-unknown`, and
`cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
-p mclone-android-xr-client`. On-device Quest 3 session-smoke drew the seed
world identically to the Slice 0 baseline (terrain ready summary
`sections=123 drawn_sections=32 indices=599700 drawn_indices=265422 actors=2`),
with the log now emitting from `mclone_scene::session` — confirming the renamed
crate runs unchanged on-device.

Validation commands:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-scene -p mclone-app-runtime -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client -p mclone-android-xr-client
pnpm native:xr:desktop            # desktop XR smoke (see docs/platforms.md for runtime setup)
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke
```

Exit criteria (met): identical on-device behavior — same session-smoke seed
section counts as the pre-slice (Slice 0) baseline.

### Slice 2: Web-shaped seam — runner-generic local host mode — DONE (2026-07-09)

Goal: make `NativeSessionRuntime`'s local mode generic over the existing
`IntegratedServerRunner` trait instead of hard-coding
`NativeIntegratedServerRunner` (`native_session_runtime.rs:275`). Native
callers keep using the native runner via a default/alias; nothing about web is
migrated — this slice only removes the seam that would otherwise bake the fork
in permanently.

Landed shape:

- `LocalIntegratedConnection` and `LocalIntegratedSceneRuntime` are now generic
  over `R: IntegratedServerRunner` with `R = NativeIntegratedServerRunner` as
  the default type parameter, so every existing native caller
  (`LocalIntegratedSceneRuntime`, the `NativeSceneRuntime::Local` variant,
  `as_local`/`expect_local_scene`, the startup pump/prewarm) compiles unchanged
  — the default fills `R` and no `S`/`R` threading was needed through
  `NativeSceneRuntime<S>` / `NativeSessionRuntime<S>`.
- The native-runner-building constructors (`new`, `with_mesh_assets`) stay on
  `impl LocalIntegratedSceneRuntime<NativeIntegratedServerRunner>`; the shared
  scene wiring moved to a runner-generic
  `with_mesh_assets_and_runner(options, mesh_assets, runner: R)` that a web
  build can call with its own runner.
- Three methods the local connection needs (`refresh_fast_diagnostics`,
  `set_simulation_cadence`, `flush_persistence`) moved from inherent methods on
  `NativeIntegratedServerRunner` onto the `IntegratedServerRunner` trait, each
  with a behavior-preserving default (no-op counter refresh / cadence
  unsupported / no flush point) and a native override carrying the existing
  bodies. `WebIntegratedServerRunner` picks up the defaults for free; no web
  behavior changed.

Tripwires: app-runtime tests pass; no public API breakage for existing native
callers beyond mechanical type-parameter additions; wasm check clean.

Validation (all green 2026-07-09): `cargo fmt --all --check`, workspace
`cargo check`, `cargo test -p mclone-server -p mclone-app-runtime
-p mclone-native-client` (server 185 + app-runtime 179 + native-client 375,
0 failed), `cargo check -p mclone-web-client --target wasm32-unknown-unknown`,
`cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
-p mclone-android-xr-client`, and the desktop offscreen pixel canary
`pnpm native:desktop-offscreen:smoke` (64 sections / 11 drawn, terrain + mobs
render unchanged). No device smoke needed — behavior-preserving structural
slice.

### Slice 3: Mono view topology + first flat consumer (offscreen/headless)

Goal: the scene host can render a single flat view.

**Scope boundary (clarified 2026-07-09): this slice migrates only the
headless consumers that do NOT sit on `FlatClientDriver`.** The
`--screenshot` path and `offscreen_flat_client.rs` are `FlatClientDriver`
consumers; they stay on that driver until Slice 7, which is chartered to
repoint them ("CLI, screenshot, timedemo, perf harnesses repoint at the host
through the offscreen driver from Slice 3"). Slice 3 *builds* the
`OffscreenDriver` and proves it on the headless dual-view path; Slice 7
*adopts* it for everything that currently rides `FlatClientDriver`. Pulling
the `--screenshot` harness forward into this slice would start Slice 7's work
before Slices 4–6 land (violating the sequencing guardrail) and would force
either duplicated effects/admission wiring or a behavior change (violating
the no-behavior-smuggling guardrail).

Deliverables:

- Add `Mono` topology to the host, rendering through the existing shared
  `render_full_frame_for_view*` entry points
  (`mclone-app-runtime/src/frame_render.rs`), including far-LOD variant.
  The existing Stereo/Multiview paths are untouched; XR output must be
  byte-identical.
- Screen-space UI presentation strategy on the host, at **capture grade**
  for this slice: what the headless path renders today via
  `GameUiHost::new_ingame()` (HUD/crosshair/hotbar overlay on a mono frame).
  Full interactive menu/effects wiring on the mono topology arrives with
  Slice 7, not here.
- An `OffscreenDriver` that renders the host's Mono topology to a texture.
- Port the headless dual-view frame assembly (`headless.rs:844-891`,
  `write_headless_dual_view_frame`, which drives `WindowSceneRuntime`
  directly) onto the host + `OffscreenDriver`, deleting that third copy of
  frame-input assembly. Do **not** touch `offscreen_flat_client.rs` or the
  `--screenshot` path in this slice.

Tripwires: captures from the ported headless verbs are pixel-equivalent (or
reviewed-equivalent) to pre-slice baselines; `--screenshot` output is
byte-unchanged (it still rides `FlatClientDriver` — it is a canary here, not
a consumer); XR device smokes unchanged.

Validation: standard battery, plus:

- **Positive validation** — run the headless verb(s) that exercise the ported
  dual-view path, comparing `/tmp` captures against pre-slice baselines.
- **Canary** — `pnpm native:desktop-offscreen:smoke` and compare
  `/tmp/mclone-desktop-offscreen.png` against a pre-slice capture (must be
  unchanged; this path is not migrated in this slice).
- One Quest session-smoke to prove no XR regression.

> Line anchors in Slices 4–10 were re-verified 2026-07-09 after Slices 0–2
> landed. Expect small drift as slices land — anchor on symbol names first,
> line numbers second. The scene crate is now `mclone-scene`
> (`native/crates/mclone-scene/`); older references to `mclone-xr-scene` in
> the evidence section describe the pre-rename audit.

### Slice 4: Shared OpenXR frame driver

Goal: write the poll -> waitFrame -> locate -> render-or-skip -> end -> report
loop **once**, in `mclone-xr-host` (or a new thin `mclone-xr-runtime` crate if
dependency direction demands it), and remove the transitive `openxr` edge that
Slice 1 left behind.

Read first: `native/crates/mclone-xr-host/src/lib.rs` (the frame primitives:
`poll_openxr_events`, `wait_begin_frame`, `end_skipped_frame`,
`end_frame_with_layers`, `end_stereo_projection_frame`,
`end_multiview_projection_frame`, `acquire_eye_target`/`acquire_stereo_target`,
`locate_stereo_views`, the `XrEyeSwapchain`/`XrStereoSwapchain` traits, and the
neutral `XrViewPose`/`XrFov`/`XrView` types at `lib.rs:76-114`);
`desktop_xr.rs:415` (`run_smoke_frames`) as the simplest existing loop;
android-xr `lib.rs:3343` (`run_mclone_frame_loop`) as the fullest one; then the
three harness loops (`run_multiview_proof_loop` `:2370`,
`run_terrain_multiview_perf_loop` `:2667`, `run_terrain_multiview_proof_loop`
`:3053`).

Context: xr-host owns every frame *step* but not the loop that sequences them,
so the skeleton is copy-pasted six times. The only genuinely per-surface
pieces are: (a) the outer event pump (winit companion pump on desktop vs
`poll_android_events`/`wait_for_android_resume` on Android), (b)
frame-limit/timeout policy (desktop bounded smoke frames vs Android
unbounded), (c) the concrete graphics session (already abstracted by the
swapchain traits). All three become injected values/hooks on the driver.

Deliverables:

1. **Dependency-direction fix (discovered post-Slice-1, required here).** The
   neutral view types landed in `mclone-xr-host`, and `mclone-scene` depends
   on `mclone-xr-host` (`mclone-scene/Cargo.toml:24`,
   `mclone-scene/src/lib.rs:106` imports `XrControllerSnapshot, XrFov, XrHand,
   XrView`) — so the scene crate still carries a **transitive** `openxr`
   dependency, violating host purity (guardrail 5) the moment flat consumers
   arrive. Move the neutral pose/view/fov/controller-snapshot types to an
   openxr-free home so `mclone-scene` can drop its `mclone-xr-host` dep
   entirely. Preferred home: an existing openxr-free crate both already
   depend on (`mclone-render-session` beside `EngineCamera`, or
   `mclone-app-runtime`); a tiny new types crate is acceptable if neither
   fits cleanly. `mclone-xr-host` keeps only the `from_openxr` conversions.
   Record the placement choice in this doc. Exit check:
   `cargo tree --manifest-path native/Cargo.toml -p mclone-scene | grep -i openxr`
   returns nothing.
2. **`OpenXrFrameDriver`**, generic over the graphics session via the
   existing swapchain traits, taking: a per-frame render callback, a platform
   pump hook, frame-limit/timeout policy, and the idle poll interval.
   `SESSION_IDLE_POLL_INTERVAL` (declared identically at `desktop_xr.rs:103`
   and android-xr `lib.rs:150`) gets its single home here.
3. **Migrate all six loops** onto the driver: `desktop_xr.rs:415`, android-xr
   `lib.rs:3343` (main), `:2370`, `:2667`, `:3053` (harnesses). The harness
   loops become thin render-callback variants, not separate loops.
4. **Unify the `render_mclone_frame` twins** — `desktop_xr.rs:972`, android-xr
   `lib.rs:7065` (per-eye) and `:7205` (multiview) — into one scene-crate
   entry point with per-eye and multiview variants. Move the ~28-line
   automation-dispatch `match` (copy-pasted inside both android-xr variants)
   into `mclone-scene/src/locomotion.rs` next to the methods it calls.
5. **Small dedups while the files are open**: `log_openxr_host_event`
   (`desktop_xr.rs:294` / android-xr `lib.rs:7051`) moves beside the driver;
   `local_integrated_scene_options` (`mclone-scene/src/session.rs:1565`)
   becomes `pub` and the copies die (`android_xr_local_options`/
   `android_xr_host_options` in android-xr `lib.rs` ~2177,
   `xr_scene_options_from_desktop_scene` in `desktop_xr.rs` ~855-949).

Out of scope: report formatting (Slice 5), admission policy (Slice 6), any
flat surface, any gameplay/effects change. XR pixel output must be
byte-identical; this is loop plumbing only.

Baseline capture (before editing): save the full Quest session-smoke log and
a desktop XR smoke log to `/tmp` for post-slice line-by-line comparison. The
validation scripts grep for specific log markers (e.g.
`MCLONE_ANDROID_XR_REPLACEMENT_READY` in `android-xr/validate-quest-openxr.sh`)
— those exact strings must survive the migration.

Tripwires: `grep -rn "wait_begin_frame" native/apps` hits nothing (only the
shared driver calls it); Quest smoke ready-summary lines
(`sections=`/`drawn_sections=`/`drawn_indices=`) match pre-slice baselines;
the `cargo tree` openxr check on `mclone-scene` is clean.

Validation: standard battery + `pnpm native:xr:desktop` +
`MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke` +
`pnpm native:android-xr:multiview-proof` and
`pnpm native:android-xr:terrain-multiview-proof` (the harness loops must still
pass after migrating onto the driver).

Exit criteria: six loops replaced by one driver; both XR apps' loop code is
render-callback + platform pump only; `mclone-scene` has no openxr edge,
direct or transitive; device logs match baselines.

### Slice 5: Diagnostics presentation consolidation

Goal: one set of frame-report builders, one set of formatters, one accountant.

Read first: `mclone-scene/src/frame_pipeline_reporter.rs` (the canonical
builders: `xr_frame_pipeline_stage_spans` `:119`, the peer-panel builder, and
the `remote_host_queue_report`/`server_runner_peer_report`/
`worker_metrics_peer_report` helpers `:329-349`);
`mclone-app-runtime/src/frame_pipeline_accounting.rs` (the flat-side
`FramePipelineAccountant` relocated there by 165 Slice 2a); android-xr
`lib.rs:4519` (`log_summary`, ~700 lines) and `:5218` (`log_worst_frames`,
~300 lines); the android-xr duplicate builders
(`android_xr_render_stage_spans` `:5549`, `android_xr_queue_panel` `:5596`,
`android_xr_peer_thread_panel` `:5684`, plus its local
`remote_host_queue_report`/`server_runner_peer_report`/
`worker_metrics_peer_report`); desktop `perf.rs:2995-3284` (the third copy of
StageId/QueueId/PeerThreadId construction); and **the 165 tactical's Slice 2c
section**, which already scopes the accountant convergence.

Context: the *builders* are triplicated (shared crate / android-xr / desktop
perf). The *presentation* (text formatting of `FramePipelineReport`) was never
shared at all, so android-xr and desktop perf each wrote their own — that is
most of the remaining bulk of the 7,661-line Quest file. Separately, two
accountants emit the same `FramePipelineReport`
(`FramePipelineAccountant` in app-runtime; `XrFramePipelineReporter` in
mclone-scene).

Deliverables:

1. **Shared presentation module** (formatters: summary, worst-frames, the
   `*_label` helpers) living beside whichever accountant survives — prefer
   `mclone-app-runtime` next to `frame_pipeline_accounting` if flat consumers
   need it without the scene crate, otherwise the scene crate's reporter
   module. Delete the android-xr copies (`log_summary`/`log_worst_frames` +
   `*_label` + duplicate builders listed above) and the desktop
   `perf.rs:2995-3284` builder copy; both repoint at the shared module.
2. **Accountant convergence per 165 Slice 2c.** 165 records the exact
   semantics that must survive: the XR single-shot `record_frame` feed (vs
   the desktop push API), `NearestRank` percentile config, the
   `completed_results` enqueue/dequeue queue-age semantics (vs plain
   `reconcile_depth`), the XR-only peer-thread panel, and neutralizing the
   XR-local `XrTerrain*` input types. 165's recommended shape: generalize the
   shared accountant to accept a `FrameAccountingConfig` plus a
   `record_prebuilt(observation, queue_panel, extras)` entry point, and a
   neutral queue-depth input struct. **If 165-2c has already landed, consume
   its result; if not, land it here and mark it done in the 165 doc. Do not
   implement it twice.**

Out of scope: admission/budget behavior (Slice 6); changing what is measured
(only where the builders/formatters live); flat surfaces beyond the
`perf.rs` builder dedup.

Baseline capture: one full Quest `terrain-multiview-perf` summary and one
desktop perf/timedemo report saved to `/tmp` pre-slice. The formatters may be
unified, but any changed log line must be checked against what the validation
scripts parse (`android-xr/validate-quest-openxr.sh` greps specific markers)
and against the perf-summary comparison workflow.

Tripwires: per the XR render-path guardrail, any XR-pixel-visible change
requires desktop-XR + Quest capture validation (the diagnostic panel is
XR-visible); percentile/age semantics preserved exactly — compare a Quest
perf summary line-by-line pre/post; parsed log markers unchanged.

Validation: standard battery + Quest session-smoke +
`pnpm native:android-xr:terrain-multiview-perf` (report formatting is exactly
what this exercises) + one desktop `pnpm native:timedemo:smoke`.

Exit criteria: one builder set, one formatter set, one accountant type;
android-xr `lib.rs` loses roughly the `:4472-7020` region; 165 Slice 2c is
marked resolved in both docs.

### Slice 6: Shared adaptive render-admission policy

Goal: one client-side upload-admission policy, owned by the scene host.

Read first: `flat_client_driver.rs:186` (`DesktopRenderBudgetController`: the
`decide(...)` grant path building `BudgetControllerInput` +
`decision_for_family(BudgetDecisionFamily::RenderAdmission)`, and
`observe_sync(...)` feeding the `EwmaCostEstimator` from timed section
updates) plus its call sites (the `decide` call in the sync path ~`:2251`,
panel merge ~`:568`, reset on session change ~`:2727`);
`mclone-frame-budget` (`BudgetController`,
`render_frame_budget_controller_config`); the scene host's current static
policy (`RenderSectionUploadFramePolicy` + `should_drain_before_runtime_sync`
in `mclone-scene/src/lib.rs` ~`:1757-1765`, cap setter
`mclone-scene/src/options.rs:195`); and the **frame-budget scoping section of
this doc** — the server-scheduler budgeting (150) and render compile capacity
are already shared and must not be touched.

Context: three admission policies exist for one problem — desktop adaptive
(EWMA + headroom-based grants), XR static cap + drain rule, Android nothing.
The adaptive controller is the survivor; the other two become an override and
an inheritance respectively.

Deliverables:

1. Move the adaptive controller into the scene host as **the** admission
   policy, fed by the single frame accountant's `FramePipelineReport`
   (Slice 5). Keep the EWMA observation of timed section updates.
2. Per-driver input is only the **target frame period**: compositor refresh
   on XR (xr-host already queries display refresh), vsync/frame-pacing period
   on desktop flat; Android flat inherits in Slice 8 (fixed 60Hz per its
   current `ANDROID_FIXED_FPS_CAP` until a refresh query exists).
3. The XR static cap (`render_section_upload_budget`) becomes a **clamp on
   the adaptive grant** — an optional override knob, not a parallel
   mechanism. When set, the effective admission is
   `min(adaptive grant, configured cap)`. The pre-sync drain rule stays as
   shared policy.
4. Desktop's app-local controller struct and call sites are deleted.

Out of scope: `mclone-frame-budget` decision-family design changes (166 owns
that direction — this slice only *relocates* today's controller); server
scheduler config; compile-capacity config.

Baseline capture: `pnpm native:frame-budget:smoke` output and a Quest
`sky-terrain-multiview-perf` report pre-slice.

Tripwires: desktop probes show unchanged admission behavior (same decision
panels for the same inputs — the move must be behavior-preserving on
desktop); Quest shows no frame-time regression. On Quest the adaptive
controller must not admit more work per frame than the old static cap did —
the Quest smoke configs that set a cap keep it, so the clamp in deliverable 3
guarantees this; a Quest lane with no cap set should be compared explicitly
before relying on the adaptive grant alone.

Validation: standard battery + `pnpm native:frame-budget:smoke` +
`pnpm native:startup-streaming:smoke` + Quest
`pnpm native:android-xr:sky-terrain-multiview-perf`, comparing frame-time
percentiles against the baseline.

Exit criteria: `DesktopRenderBudgetController` gone from the app crate; one
policy type in the host consumed by both topologies; desktop probe output
matches baseline; Quest percentiles within noise of baseline.

### Slice 7: Desktop flat onto the host (delete `FlatClientDriver` orchestration)

Goal: desktop flat becomes `WinitFrameDriver` + host. Biggest slice — land it
as ordered sub-slices (7a–7e below), **each keeping desktop green and each
independently validated**. Do not attempt 7 as one change.

Read first: `flat_client_driver.rs` end to end (it is the thing being
deleted — know what it owns before deleting: per-frame orchestration
`poll_runtime`/`sync_runtime_sections`/`upload_runtime_sections`/
`prepare_frame_inputs`; effects `apply_client_experience_settings_effects`
~`:1195` + capability projection ~`:1403`; pending-start machine
`finish_pending_session_start` `:1467`, `start_pending_session` `:1676`,
`begin_pending_local_world_start` `:1781`; input assembly
`engine_camera_input_from_flat_frame` ~`:3750`); `app.rs` (winit handler,
`current_flat_hud` `:899`, `finish_pending_session_start` `:928`); the scene
host's existing wiring that desktop will adopt
(`mclone-scene/src/session.rs`: `apply_xr_settings_effects` `:1080`,
`execute_xr_catalog_request` `:1007`, the session-runtime factory /
`start_session_for_request` machinery, timed camera-reconcile fork
`:1437-1540`); `remote_session.rs:9` (the adapter being deduplicated); and
`offscreen_flat_client.rs` + the Slice 3 `OffscreenDriver`.

Recommended sub-slice order:

- **7a — Effects/HostEffects.** Generalize the host's effect applier
  (today's `apply_xr_settings_effects`) into the single applier with a
  `HostEffects` trait for the genuinely platform hooks. Known desktop-only
  arms to design for: mouse-lock arm/release, `CycleFramePacing`/`CycleFpsCap`
  (desktop host actions; Android/XR reject or ignore them today with visible
  reasons — that stays, expressed through the trait), quit-to-title/exit.
  Desktop and the XR session host both consume the shared applier; the
  desktop copy of the 24 arms + capability projection dies. The Android/web
  copies die in Slice 8 / the web follow-up. Frame-pacing POD stats structs
  move shared per **165 Slice 3** (coordinate, don't duplicate); the
  `FramePacing` winit driver itself stays app-local.
- **7b — Session start/replacement.** Desktop routes world create/open/join
  and mid-session replacement through the host's session-runtime factory;
  delete the pending-start state machine (`app.rs:928-948`,
  `flat_client_driver.rs:1467-1781`) and desktop's `SessionStartRequest`
  handling. Collapse the triplicated `RemoteServerSession` adapter into one
  label-parameterized shared type in `mclone-net` or `mclone-app-runtime`;
  desktop and both XR apps consume it (the flat-Android copy dies in
  Slice 8).
- **7c — Per-frame loop.** `WinitFrameDriver`: redraw-driven, builds the mono
  view from the engine camera, acquires the window surface texture, calls the
  host's Mono frame; poll/sync/upload/traversal-ready/frame-input assembly
  all delete from the app crate. HUD assembly (`current_flat_hud`,
  `app.rs:899`) moves onto the host's screen-space presentation (which
  Slice 3 built at capture grade; this is where it becomes fully
  interactive). Desktop input events keep flowing through `mclone-input` to
  the host's input application.
- **7d — Harnesses.** CLI `--screenshot`, timedemo, frame-budget/movement
  probes, and `perf.rs` consumers repoint at the host through the Slice 3
  `OffscreenDriver`; `offscreen_flat_client.rs` is deleted or reduced to a
  thin driver instantiation. `perf.rs` is large (~4,900 lines) — it keeps
  measuring the same stages via the hooks that moved in Slices 5/6; budget
  extra time here.
- **7e — Camera-reconcile timed fork.** Shared
  `mclone_app_runtime::camera_reconcile` grows optional timing output;
  delete the `*_for_runtime_timed` fork
  (`mclone-scene/src/session.rs:1437-1540`).

Out of scope: flat Android (Slice 8), any web code, XR behavior changes
(7a/7e touch shared XR paths — those need XR validation but not XR behavior
change).

Baseline capture: before 7c/7d, capture `--screenshot` local + remote,
timedemo, and frame-budget probe outputs to `/tmp` for comparison.

Tripwires: `flat_client_driver.rs` is deleted or reduced to winit/surface/
input glue (if what remains exceeds ~500 lines, something orchestration-
shaped survived — find it); grep — no per-frame `sync`/`upload`/frame-input
assembly outside the host; no `ClientExperienceSettingEffect` match in
`mclone-native-client`; desktop screenshot/timedemo/perf outputs match
baselines; XR smokes unchanged after 7a/7e.

Validation (full desktop suite, after each sub-slice as applicable) —

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-app-runtime -p mclone-scene
pnpm native:desktop-offscreen:smoke
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --screenshot /tmp/mclone-168-local.png --startup-wait playable
# remote join against a local dedicated server (see docs/platforms.md / native:remote:smoke)
pnpm native:remote:smoke
pnpm native:timedemo:smoke
pnpm native:frame-budget:smoke
pnpm native:xr:desktop
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke
```

### Slice 8: Flat Android rebuilt as thin glue (delete the loop)

Goal: `mclone-android-client/src/lib.rs` is deleted-and-replaced, not
refactored. Target size: activity/surface/lifecycle glue + input adapter +
driver instantiation. When in doubt about whether some Android code has value,
the default answer is: it is a worse copy of what the host now owns — delete
it. The three exceptions with real value are the touch controls, the
system-property startup config, and the surface/lifecycle glue.

Read first: `mclone-android-client/src/lib.rs` regions — touch layer
(`AndroidMovementTouch`/`AndroidTouchControls` `:276-398`, touch-look
`:690-707`); frame renderer (the `poll` -> reconcile -> `sync` -> apply ->
render sequence around `:2036-2089`); lifecycle (`suspended()` `~:2798` —
today's drop-driven save); effect arms `~:1509`; remote adapter `:2663`;
input assembly `~:3303` + key tables `~:3184`; startup/property config
(`debug.mclone.remote_addr` handling). Also `mclone-input/src/lib.rs:1257`
(`TouchBindings`, the dead shared contract this slice makes real) and
`android/validate-avd.sh` (what the AVD smokes actually assert).

Deliverables:

1. **`AndroidSurfaceDriver`**: AndroidApp events + surface acquire/resize +
   suspend/resume forwarded to host lifecycle calls. Suspend persistence now
   comes from the host's `on_background()` policy — the same policy Quest
   uses after Slice 0; the Android-specific full teardown/rebuild on suspend
   may remain as the driver's *implementation* of background, but the save
   decision is the host's.
2. **Touch controls move into `mclone-input`** as the real implementation of
   the currently-unused `TouchBindings`, replacing the hand-rolled layer —
   including the screen-normalized, configurable touch-look model
   (`touch_look_sensitivity`, clamped, resolution-independent), which becomes
   the shared touch-look semantics. Preserve current joystick geometry
   (`TOUCH_JOYSTICK_RADIUS_GUI`) and add sneak to the touch layout if the
   layout allows (it has jump/sprint/descend but no sneak today — if sneak
   doesn't fit, record it as an input-axis platform difference, which 165
   permits).
3. **Delete the Android copies** of: the per-frame loop, effect arms +
   capability projection, catalog/session wrappers,
   `engine_camera_input_from_flat_frame`, keyboard key tables (fold the
   Android-vs-desktop table drift — arrows/F5 — into one shared table with
   the shared adapter), the remote-session adapter (`:2663`, consume the
   Slice 7b shared type), and the startup-option duplication. Reconcile the
   two Android surfaces' remote-addr resolution (flat's
   `debug.mclone.remote_addr` system property vs android-xr's
   sentinel/normalization helpers) into one shared helper used by both apps.
4. **Android inherits from the host** (no Android-specific code): adaptive
   render admission (target period = 60Hz fixed, per Slice 6), frame-pipeline
   accounting + overlay data, travel assist, debug diagnostics. **165 ledger
   coordination**: flip each affected capability
   (`TravelAssist`, `FramePipelineOverlay`, `DebugDiagnostics`, and
   `ServerSimulationCadence` if the host wires cadence for local sessions by
   then) to `Supported` in the same change that lands its plumbing, and drop
   the ledger entries — 165's enforcement tests require exactly this
   ordering, and `native_feature_parity_exceptions_are_live` will fail if a
   stale entry remains. Note `DebugDiagnostics` also depends on the
   debug-stats aggregator move scoped in 165 Slice 3 — coordinate.
5. Present-mode policy (`PresentMode::Fifo`, fixed FPS cap with its visible
   "fixed on flat Android" status message) survives as the driver's
   `HostEffects` implementation detail, not as effect-arm copies.

Out of scope: Android XR (already thin after Slices 4–5); changing touch UX
beyond the sneak addition; web.

Baseline capture: all three AVD smoke screenshots + logcat outputs pre-slice.

Tripwires: `mclone-android-client` contains no `ClientExperienceSettingEffect`
match, no `sync_render_sections` call, no session-request dispatch, no
`EngineCameraInput` construction; AVD smoke log lines use the same
startup-ready wording and seed counters as desktop/XR; 165 ledger shrinks and
its three enforcement tests pass.

Validation:

```bash
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-smoke
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-touch-smoke
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-session-smoke
# plus the standard battery and one Quest session-smoke (shared-code regression)
# optional extra lane: Quest-as-flat-Android device smoke
pnpm native:android:quest-flat
```

Inspect the AVD screenshots (`/tmp/mclone-android-avd-*.png`) — terrain drawn,
touch controls responsive (the touch smoke swipes and asserts camera motion),
frame-pipeline overlay now populated when toggled. Exercise a
suspend/resume cycle on the AVD and verify the world persists (the host
`on_background()` path).

Exit criteria: new `lib.rs` is glue-only (target well under ~1,000 lines);
all three AVD smokes pass with unchanged-or-better output; the 165 ledger
rows for flat Android are gone.

### Slice 9: XR-emulation lane on desktop (acceptance proof)

Goal: prove the unification by running the stereo scene with a synthetic head
— no headset, no OpenXR runtime, no OpenXR loader initialization.

Context: because Slice 4 removed the openxr edge from `mclone-scene`, the
host's Stereo topology takes neutral view types. Feeding it two synthetic
views is therefore pure data construction. This lane doubles as a
headset-free way to develop and screenshot XR-visible features (menus,
world-quad UI, comfort effects) going forward.

Deliverables:

1. A desktop lane (CLI verb on `mclone-native-client`, e.g.
   `--xr-emulation-screenshot <path>` for offscreen and/or a windowed
   side-by-side mode) that constructs two synthetic neutral views — eye
   offsets from a fixed IPD (~64mm), symmetric FoVs (reuse the defaults the
   stereo config path produces) — and drives the host's Stereo topology
   through the Slice 3 `OffscreenDriver` (offscreen) or the winit driver
   (windowed side-by-side).
2. The synthetic head pose is driven by the engine camera; snap-turn and XR
   locomotion semantics run through the same `mclone-scene` locomotion path,
   fed from keyboard (and gamepad if the Slice 10 gamepad decision lands
   "adopt"), so XR input semantics are exercisable without controllers.
3. The lane must not initialize any OpenXR instance or require a runtime to
   be installed — if the desktop binary currently links the loader
   unconditionally, the emulation path simply must not call into it.

Out of scope: making the emulated lane a full interactive product surface;
head-tracking simulation beyond the engine camera; multiview (stereo per-eye
is sufficient for the proof — multiview stays validated on Quest).

Validation: capture a side-by-side stereo pair to `/tmp`
(e.g. `/tmp/mclone-xr-emulation.png`) and **inspect it**: both eyes render,
horizontally offset by the IPD (near geometry visibly shifts between eyes,
far geometry barely), world-quad UI panels appear in both eyes with their own
projections (per-eye view/projection invariant holds — never shared mutable
per-eye uniforms); standard battery; one Quest session-smoke to confirm the
real stereo path is untouched.

Exit criteria: the acceptance sentence from the top of this doc holds —
"desktop can run the stereo scene with a fake head" — demonstrated by a
committed-to-`/tmp` capture and a documented command (docs land in Slice 10).

### Slice 10: Cleanup, enforcement, docs

Goal: make the unified shape hard to regress, then document it.

Deliverables:

1. **Delete compatibility wrappers** that survived only for slice safety
   (sweep Slices 4–8 result notes for anything marked temporary).
2. **Constant consolidation.** One home each for: the 120s startup-readiness
   timeout (canonical: `DEFAULT_STARTUP_READINESS_TIMEOUT`,
   `mclone-app-runtime/src/native_session_runtime.rs:61`; delete the copies —
   originally `BLOCKING_STARTUP_TIMEOUT` in `flat_client_driver.rs`,
   `XR_STARTUP_READINESS_TIMEOUT` in the scene session,
   `ANDROID_STARTUP_READINESS_TIMEOUT` in the Android app, and the
   `remote_player_visual_smoke.rs` copy — several should already be gone via
   Slices 7/8; verify with grep) and the 25ms `SESSION_IDLE_POLL_INTERVAL`
   (should already be single-homed in the Slice 4 driver; verify).
3. **Tripwire enforcement.** Add a check that fails loudly when app crates
   regrow orchestration. Two acceptable shapes (pick one, follow repo
   precedent — 165 used tests, 167 used documented `rg` tripwires): a small
   script (e.g. `scripts/check-native-thin-adapters.sh`) run in validation,
   or unit tests. Forbidden in `native/apps/*`:
   `ClientExperienceSettingEffect` matches, per-frame
   `sync_render_sections`/section-upload calls, `wait_begin_frame`,
   `SessionStartRequest` dispatch matches, and `EngineCameraInput`
   field-by-field construction. Also enforce host purity:
   `cargo tree -p mclone-scene | grep -i openxr` empty (plus no
   winit/android-activity/jni edges).
4. **Gamepad decision.** `GamepadInputAdapter`/`GamepadBindings` in
   `mclone-input` still have zero users. Decide and record: adopt (wire it on
   desktop + the Slice 9 emulation lane) or keep-as-contract with a dated
   note, or delete. Do not leave it undecided — dead shared API misleads the
   next contributor.
5. **Docs.** Update [`../platforms.md`](../platforms.md) (validation matrix,
   the new XR-emulation lane command, host+driver entry points) and
   [`../native-engine-architecture.md`](../native-engine-architecture.md)
   (crate ownership: `mclone-scene` host, driver crates, what app crates may
   own). Add the host/driver boundary to the "Shared-first feature policy"
   guidance in `CLAUDE.md`/`AGENTS.md` if its wording still describes the
   pre-168 crate shape.
6. **Cross-tactical bookkeeping.** Update 165 (ledger end-state; Slices 2c/3
   resolution), 163 and 167 follow-up notes, and this doc's status +
   README index row to complete.

Final acceptance checklist (run everything on this machine):

- Enforcement greps/tests pass; host-purity `cargo tree` checks clean.
- Full standard battery + all device lanes: desktop offscreen screenshot,
  desktop XR smoke, Quest session-smoke + one multiview proof, all three
  `jstorrent-tablet` AVD smokes, `pnpm native:remote:smoke`, wasm
  `cargo check`.
- The XR-emulation capture from Slice 9 reproduces with the documented
  command.
- 165's non-web exception ledger is empty or every remaining row has a
  recorded reason unrelated to this tactical.

## Cross-slice guardrails

1. **Delete, don't port.** When migrating a surface onto the host, the old
   orchestration is removed in the same slice. No parallel "legacy path kept
   just in case" — that is how the fork count reached five.
2. **Every slice leaves every surface shippable.** Minimum per-slice battery:
   `cargo fmt --all --check`, workspace `cargo check`, `cargo test` for
   touched crates, wasm `cargo check` for `mclone-web-client`,
   `cargo ndk -t arm64-v8a --platform 28 check` for both Android apps.
3. **Pixels get eyes.** Any slice touching code that produces pixels validates
   on the affected devices before it is called done — desktop offscreen
   screenshot, `pnpm native:xr:desktop` for desktop XR, Quest 3 (attached)
   via `pnpm native:android-xr:session-smoke`, flat Android via the
   `jstorrent-tablet` AVD (`pnpm native:android:avd-session-smoke`;
   `MCLONE_ANDROID_ABIS=arm64-v8a` — the tablet AVD is arm64, not x86_64).
   Save captures to `/tmp`, never into the repo. Capture pre-slice baselines
   before starting a slice so "unchanged" is checkable.
4. **XR render-path guardrail** (repo-wide, restated): per-view features work
   in both per-eye and multiview paths; each eye/layer uses its own
   view/projection data; never share mutable per-eye uniforms across one
   submission. Run the multiview proofs after any change to XR frame
   sequencing (Slices 4, 5, 6).
5. **Host purity.** The scene host crate must never gain `openxr`, `winit`,
   `android-activity`, `jni`, or blocking-socket dependencies. Time via the
   existing wasm-compatible patterns; no bare `std::thread` in the host.
6. **No behavior smuggling.** Structural slices (1, 2, 4, 5 builders-move, 7,
   8) must be behavior-preserving; behavior fixes live in Slice 0 or are
   called out explicitly with their own validation. If a migration would
   change device-visible behavior, stop and split the slice.
7. **Platform differences must be named.** Any residual divergence is
   host-mode evidence, an explicit policy value with a test, or a
   reason-bearing 165 ledger/web-ledger entry. Bare platform `if`s and silent
   `..default()`s in gameplay-semantic struct construction are the failure
   mode this tactical exists to kill.
8. **Coordinate, don't duplicate, with 165.** Capability flips follow 165's
   rule (plumbing and flip in the same slice; ledger entry removed
   simultaneously). The accountant convergence (165-2c) and frame-pacing POD
   move (165-3) land once, in whichever tactical gets there first, and are
   marked done in both.
9. **Sequencing.** Slices 0–2 are independent and can land in any order.
   3 requires 1; 4 requires 1; 5 requires 4; 6 requires 5 (needs the single
   accountant); 7 requires 3+6 (and benefits from 2); 8 requires 7; 9 requires
   7; 10 is last. Do not start 7 while 4–6 are mid-flight in the same files.

## Validation resources on this machine (2026-07-09)

- Desktop flat + offscreen/headless: `pnpm native:desktop-offscreen:smoke`,
  `--screenshot` runs, `pnpm native:timedemo:smoke`,
  `pnpm native:frame-budget:smoke`, `pnpm native:remote:smoke`.
- Desktop XR: `pnpm native:xr:desktop` (macOS WiVRn lane; see
  [`../platforms.md`](../platforms.md#validation-policy) for runtime setup and
  the Windows lane note).
- Quest 3 headset: attached via USB. `pnpm native:android-xr:session-smoke`
  (add `MCLONE_ANDROID_XR_WAIT_SECONDS=60`), `:multiview-proof`,
  `:terrain-multiview-proof`, `:terrain-multiview-perf`.
- Flat Android: AVD **`jstorrent-tablet`** (arm64; the default in
  `android/validate-avd.sh` and the `pnpm native:android:avd-*` scripts; set
  `MCLONE_ANDROID_ABIS=arm64-v8a`). `jstorrent-dev`/`jstorrent-playstore`
  AVDs also exist but are not the validation target.
- Web: compile-only (`cargo check -p mclone-web-client --target
  wasm32-unknown-unknown`); behavioral web validation is out of scope.
- Android builds go through the scripts (`pnpm native:android*:apk`,
  `android/build-common.sh` auto-discovers SDK/NDK) — never hand-roll cargo
  invocations for Android or conclude the NDK is missing without running the
  script and reading its error.

## Relationship to other tacticals

| Doc | Relationship |
| --- | --- |
| [`167-shared-session-startup-contract.md`](167-shared-session-startup-contract.md) | Startup is already unified; the host consumes `NativeSessionStartupPump` unchanged. This tactical extends the same convergence to the whole frame loop and session lifecycle. |
| [`165-native-feature-parity-baseline.md`](165-native-feature-parity-baseline.md) | Same principle at feature level. Slices 5/7/8 here land 165's remaining plumbing (accountant convergence, frame-pacing PODs, Android overlay/diagnostics/travel-assist) and shrink its ledger; follow its flip-with-plumbing rule. |
| [`150-adaptive-frame-budget-controller.md`](150-adaptive-frame-budget-controller.md) | Server-side chunk-publication budgeting is shared already and untouched; Slice 6 reuses the same `BudgetController` machinery for the client-side admission layer. |
| [`163-render-section-cpu-mesh-eviction.md`](163-render-section-cpu-mesh-eviction.md) | Resident CPU mesh eviction invariants must survive all migrations (no resident CPU payloads reintroduced). |
| [`166-shared-resident-tile-substrate.md`](166-shared-resident-tile-substrate.md) | Orthogonal render-substrate direction; both tacticals touch upload admission — Slice 6 should not preempt 166's decision-family design, only relocate today's controller. |
| [`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md) / [`154-client-ingress-adapter-cleanup.md`](154-client-ingress-adapter-cleanup.md) | Remote ingress is shared; the triplicated `RemoteServerSession` adapter dedup (Slice 7) must use the existing ingress contracts, not new request/response helpers. |
| [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md) / [`062-shared-threading-topology.md`](062-shared-threading-topology.md) | Own the web worker/threading convergence that the deferred web-adoption tactical will build on; the seams in this doc (runner trait, step-based APIs) are prerequisites, not replacements. |

## How to begin (for the implementing agent)

Start with Slice 0. Read, in order:
`native/crates/mclone-xr-scene/src/locomotion.rs` (sprint/sneak site),
`native/apps/mclone-android-xr-client/src/lib.rs:7480-7573` (lifecycle pump),
`native/crates/mclone-server/src/runner.rs` +
`native/crates/mclone-server/src/integrated.rs` (Drop-driven save path),
`native/apps/mclone-android-client/src/lib.rs:1940-1980` and `:3280-3330`
(interaction + input-assembly fixes). Capture pre-fix baselines (Quest
session-smoke log, AVD session screenshot) before editing. Land the four fixes
as separate commits with the Slice 0 validation battery, then update this
doc's slice status and the README index row.
