# 168: Unified Native Scene Host

Status: approved direction 2026-07-09; Slice 0 (bug-grade parity pre-fixes)
landed 2026-07-09; Slice 2 (web-shaped runner-generic seam) landed 2026-07-09;
Slice 1 (OpenXR-free scene crate + `mclone-xr-scene` -> `mclone-scene` rename)
landed 2026-07-09; Slice 3 (mono view topology + screen-space HUD strategy +
offscreen driver) landed 2026-07-09; Slice 4 (shared OpenXR frame driver)
landed 2026-07-10; Slice 5 (shared diagnostics accounting and presentation)
landed 2026-07-10; Slice 6 (shared adaptive render admission) landed
2026-07-10; Slice 7a (shared effects/`HostEffects` and frame-pacing/debug POD
promotion) landed 2026-07-10; Slice 7b (shared session-start planning and
native remote-session adapter) landed 2026-07-10; Slice 7c (desktop live
per-frame loop onto the shared Mono host) landed 2026-07-10; Slice 7d
(offscreen, screenshot, timedemo, and perf harnesses onto the shared Mono
host) landed 2026-07-10; Slice 7e (shared optional camera-reconcile timing and
scene-local timed-fork deletion) landed 2026-07-10; Slice 8 (flat Android onto
the shared Mono host, shared touch semantics, and native capability-ledger
closeout) landed 2026-07-10; Slice 9 (headset-free synthetic stereo capture
and keyboard-fed XR semantics) landed 2026-07-10; Slice 10 (cleanup,
enforcement, neutral public names, and durable docs) completed 2026-07-10.
Tactical complete. (Slices 0–2 are
independent per the sequencing guardrail, so Slice 2 landed ahead of Slice 1.)
Post-Slice-3
review corrections landed
2026-07-10: truthful offscreen-settle failure, an exercised mono-HUD capture
lane, a shared lifecycle `on_background()` policy, durable-flush regression
coverage, the WASM-built runner connection adapter, and early completion of
Slice 4's dependency-direction/host-purity prerequisite.

Workstream: native Rust shared runtime convergence. This tactical collapses the
four native client runtimes (desktop flat, desktop XR, flat Android, Android
XR/Quest) plus the headless/offscreen lanes onto **one shared scene host** with
thin per-platform drivers. It is the frame-loop-and-everything-else sequel to
[`167`](167-shared-session-startup-contract.md) (which already unified startup)
and the structural completion of [`165`](165-native-feature-parity-baseline.md)
(which enforces feature parity but cannot see wiring-level forks).

## Principle: delete, don't port

The most mature orchestrator in the tree began as the XR scene state (now
`mclone-scene::McloneSceneHost`). Both XR apps are already thin over
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
  lines) — thin over the shared scene for gameplay, but hand-rolls the OpenXR
  session loop **four times** (main + three proof/perf harnesses) and carries
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
2. **OpenXR session loop** — the poll -> idle/exit -> wait/begin ->
   render-or-skip -> end -> report skeleton exists in five loop bodies: desktop
   `desktop_xr.rs:415` (`run_smoke_frames`), android-xr `lib.rs:3304`
   (`run_mclone_frame_loop`), `:2370`, `:2667`, `:3020` (harness loops), plus
   the per-frame `render_mclone_frame` twins (`desktop_xr.rs:972`,
   android-xr `lib.rs:7020` and `:7145`).
3. **Frame-report presentation** — stage-span/queue/peer-panel builders exist
   in shared `mclone-xr-scene/src/frame_pipeline_reporter.rs:119-349` and are
   re-implemented in android-xr `lib.rs:5502-5774` and again in desktop
   `perf.rs:2995-3284`. Two accountants (`FramePipelineAccountant` in
   app-runtime, `XrFramePipelineReporter` in xr-scene) emitted the same
   `FramePipelineReport`; Slice 5 removed the latter and the builder copies.
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
- Single frame accountant emitting `FramePipelineReport` (**done in Slice 5**;
  165 Slice 2c is resolved).
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

### Web posture: compile the seams now, migrate later (explicitly out of scope)

Web is the reason the runtime count exploded, and it is NOT migrated in this
tactical. `WebIntegratedServerRunner` already implements the shared
`IntegratedServerRunner`, but the post-Slice-3 review found that the runner was
not the only structural seam: the entire `native_session_runtime` module is
`cfg(not(wasm32))`, owns `NativeRenderSectionCompileDispatcher` and native
deferred-drop workers, and `NativeSceneRuntime` fixes its local variant back to
the native runner default. The scene also consumes native catalog/storage,
camera-reconcile, and teleport-preview worker types. Therefore the earlier
"near-zero cost" claim was too strong.

Corrections landed 2026-07-10:

1. `IntegratedRunnerConnection<R>` moved into the WASM-built
   `mclone-app-runtime::client_connection` module and has a worker-shaped
   regression test. `LocalIntegratedSceneRuntime<R>` consumes that same adapter.
   The command/update/diagnostics runner boundary is now compiled by the normal
   app-runtime WASM check instead of living inside a native-only file.
2. Neutral tracked-controller snapshots live in `mclone-input`; neutral
   view/FOV/projection contracts live in `mclone-render-session`. The OpenXR rim
   keeps only conversion functions. `mclone-scene` no longer depends on
   `mclone-xr-host`.
3. `mclone-render`'s winit surface module is behind the explicit
   `native-surface` feature, enabled only by the desktop app. The scene host's
   dependency tree is free of `openxr`, `winit`, Android activity, and JNI edges.

Rules for the remaining work:

1. The host's frame/startup API stays non-blocking and `step()`-based; blocking
   `drive_to_ready*` loops live in native drivers only.
2. `cargo check -p mclone-app-runtime --target wasm32-unknown-unknown` and the
   existing web-client WASM check stay green in every slice; these compile the
   shared runner connection seam.
3. A direct `cargo check -p mclone-scene --target wasm32-unknown-unknown` is the
   acceptance gate for the follow-up host-runtime split. It currently fails on
   the explicit open items below and must become green before web adopts the
   host. Do not hide the failure with a stub/empty WASM scene feature.
4. Time-dependent host code must move behind the existing WASM-compatible timing
   patterns as it is made reachable on WASM; no browser path may call native
   thread/blocking convenience loops.
5. Web feature divergences remain reason-bearing via the 165 web ledger.

Web adoption of the host (rAF driver, WebSocket `S`, worker runner,
`web_canvas.rs` collapse) remains a follow-up tactical once the native shape has
settled and the direct scene WASM gate is green.

### Open questions / follow-ups found by the 2026-07-10 review

These are tracked here so the native convergence does not accidentally close
over native-only types. Resolve them in a dedicated web-host-adoption tactical,
or pull a prerequisite earlier when a native slice naturally touches it:

1. **Runtime/compiler generics.** Decide whether `NativeSceneRuntime` /
   `NativeSessionRuntime` gain a second runner/compiler type parameter or are
   renamed and split into a platform-neutral session shell plus native/web local
   implementations. `NativeRenderSectionCompileDispatcher` and
   `DeferredChunkDropWorker` cannot remain hard-coded in the shared local variant.
2. **Catalog/storage adapters.** Split `NativeWorldCatalog` and native filesystem
   storage from the catalog/session policy so the browser can inject IndexedDB or
   its existing storage adapter without forking scene orchestration.
3. **Camera reconcile and teleport preview.** Make camera reconcile compile on
   WASM and inject the teleport-preview worker/executor; do not make the host own
   an OS thread.
4. **Clock/deadline abstraction.** Replace scene-local bare `Instant` use with a
   monotonic host clock/timing helper before the direct scene WASM gate is called
   complete. Preserve native perf timing while allowing browser `performance.now`.
5. **Audio backend.** Confirm that the shared audio owner can use the browser
   backend without blocking/thread assumptions; record a reason-bearing web
   exception if it cannot land with the first host adoption.
6. **Naming cleanup.** Rename `XrMcloneTerrainState` / `XrSceneOptions` to the
   neutral `McloneSceneHost` / scene-host options shape once compatibility aliases
   are no longer needed (Slice 10 at the latest).
7. **Quest extended perf-dump ownership.** Slice 5 shared the actual
   `FramePipelineReport` text/JSON presentation, labels, builders, and accountant.
   Android XR's much larger `log_summary` / `log_worst_frames` block remains an
   app-specific dump of raw OpenXR, eye, locomotion, upload, and Quest probe
   fields rather than a second frame-pipeline formatter. Do not move those
   Android-only PODs into `mclone-app-runtime` merely to shorten `lib.rs`; either
   delete the block with the old Android frame host in Slice 8 or extract a
   dedicated platform perf-probe module in a follow-up if it survives that host.

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
bottom of this doc. Slices 0–6 are complete; Slice 7 is the next implementation
slice and must land through its ordered sub-slices.

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
scene tests + app-runtime/native-client/server, wasm check, scripted Android
build gates for both apps). Device: Quest session-smoke drew terrain
(sections=123, drawn_sections=32) with no regression; flat Android AVD session +
touch smokes drew terrain and ran clean. The flush path is compile- and
logic-verified and fires from the lifecycle pump.

Post-review strengthening (2026-07-10): the lifecycle decision now lives on
the shared host as `on_background()` and the Quest adapter only reports the
Pause/Stop transition. A runner regression test keeps the original server alive,
calls the explicit flush, opens a second reader, and proves the edit is durable
before any Drop/shutdown save can mask the result. A worn-headset
Pause→force-stop→relaunch spot check is fully deferred outside this tactical's
acceptance and must not block or appear in subsequent slice handoffs.

Deliverables:

1. **XR sprint + sneak.** Wire `sprint` and `shift` into the
   `EngineCameraInput` built at `mclone-scene/src/locomotion.rs:135-147`.
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
cargo test --manifest-path native/Cargo.toml -p mclone-scene -p mclone-app-runtime -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:android:apk
pnpm native:android-xr:apk
# Quest 3 is attached to this machine:
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke   # sprint/sneak + pause-save on device
# Flat Android tablet AVD (jstorrent-tablet):
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-session-smoke
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-touch-smoke
```

Exit criteria (met): sprint/sneak observable on Quest (log or on-device
movement speed), durable flush covered by the shared runner regression test,
and Android AVD session smoke still draws terrain.

### Slice 1: OpenXR-free scene crate (`mclone-xr-scene` -> `mclone-scene`) — DONE (2026-07-09)

Goal: neutralize the rim. Behavior-free.

Landed shape:

- Host-neutral view types (`XrViewPose`, `XrFov`, `XrView`, `XrRenderView`) and
  pure projection helpers live in `mclone-render-session`. Neutral controller
  snapshots and hand identity live in `mclone-input`. The names retain `Xr`
  because they describe stereo/tracking data, but neither owner depends on
  OpenXR.
- The `openxr` -> neutral conversion happens at the OpenXR rim: the two XR apps
  call `mclone-xr-host` conversion functions at the boundary before handing
  views to the scene. Pose finiteness validation therefore lives at that
  boundary (behavior-equivalent).
- `openxr` and `mclone-xr-host` are absent from the scene crate's dependency
  graph. The post-review Slice 4 prerequisite also made `winit` an opt-in
  `mclone-render/native-surface` feature owned by the desktop app, so the scene
  graph is free of all platform-rim dependencies checked by
  `pnpm native:scene-host:purity`.
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
the scripted flat/XR Android build gates. On-device Quest 3 session-smoke drew the seed
world identically to the Slice 0 baseline (terrain ready summary
`sections=123 drawn_sections=32 indices=599700 drawn_indices=265422 actors=2`),
with the log now emitting from `mclone_scene::session` — confirming the renamed
crate runs unchanged on-device.

Validation commands:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-scene -p mclone-app-runtime -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:android:apk
pnpm native:android-xr:apk
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
- Post-review correction (2026-07-10): the generic connection adapter itself
  moved out of the native-only runtime file into the WASM-built
  `client_connection` module as `IntegratedRunnerConnection<R>`. This makes the
  runner seam real and continuously compiled, but does **not** claim the whole
  scene runtime is WASM-ready; the remaining compiler/catalog/worker seams are
  listed in the web follow-ups above.

Tripwires: app-runtime tests pass; no public API breakage for existing native
callers beyond mechanical type-parameter additions; wasm check clean.

Validation (all green 2026-07-09): `cargo fmt --all --check`, workspace
`cargo check`, `cargo test -p mclone-server -p mclone-app-runtime
-p mclone-native-client` (server 185 + app-runtime 179 + native-client 375,
0 failed), `cargo check -p mclone-web-client --target wasm32-unknown-unknown`,
the scripted flat/XR Android build gates, and the desktop offscreen pixel canary
`pnpm native:desktop-offscreen:smoke` (64 sections / 11 drawn, terrain + mobs
render unchanged). No device smoke needed — behavior-preserving structural
slice.

### Slice 3: Mono view topology + first flat consumer (offscreen/headless) — DONE (2026-07-09)

Goal: the scene host can render a single flat view.

Landed shape:

- **Mono view topology on the host.** New `mclone-scene/src/mono.rs` adds
  `XrMcloneTerrainState::render_mono_frame` (live) and
  `render_mono_frame_frozen_runtime` (frozen), rendering one flat
  `ChunkRenderView` through the shared
  `render_full_frame_for_view_with_far_lod` entry (far-LOD included). They
  reuse the host's existing frame orchestration unchanged — `live_upload_for_frame`
  (budgeted poll/sync/upload/compile-release), `effective_render_options`,
  `sky_clear_color`/`time_of_day`/`sun_angle`, `current_actor_instances`, and a
  new single-view `mono_underwater_overlay` mirroring the stereo midpoint case.
  Unlike the self-submitting stereo eye path, the mono entries encode into a
  caller-owned `RenderFrameContext` and leave submission to the driver (matching
  the shared `render_full_frame_for_view*` contract), so they compose with
  offscreen capture loops and any future surface driver. The Stereo/Multiview
  paths are untouched.
- **Screen-space UI presentation strategy.** `MonoUiPresentation::{None,
  ScreenSpaceHud}` selects the strategy; the stereo world-quad panel remains the
  other. `ScreenSpaceHud` draws the shared menu-panel list plus a capture-grade
  `FlatHud` crosshair/hotbar through the shared GUI slot at the target resolution,
  backed by a lazily-built `mono_gui: Option<GuiRenderer>` on the host (built on
  first HUD render; stays `None` on headsets). The dual-view consumer defaults to
  `None` (world capture only) and enables the pixel canary explicitly with
  `--headless-dual-view-hud true`.
- **Readiness seam.** New non-blocking `local_startup_complete()` and
  `pending_stream_work(camera_position)` host queries (drawable-target pending
  chunks/inflight sections + queued client uploads). Tracking-halo dirt outside
  the requested render distance does not block a capture; this matches the
  existing startup-streaming target-render quiescence distinction. The blocking
  drive-to-ready loop lives in the native driver, not the host (Web posture rule).
- **OffscreenDriver.** New `mclone-native-client/src/offscreen_scene_host.rs`
  `MonoOffscreenSceneHost` constructs the host for a local flat scene, streams
  the world in against a scratch target until target-aware
  `pending_stream_work(camera_position)` reaches
  zero (the step-based equivalent of the old cold full-sync), then renders flat
  cameras via the host mono topology. `write_headless_dual_view`
  (`--headless-dual-view`) now consumes it; the inline third copy of frame-input
  assembly (`headless.rs:844-891`, `write_headless_dual_view_frame`) is deleted.
  `offscreen_flat_client.rs` and the `--screenshot` path are untouched (Slice 7).
- **Dependency.** `mclone-scene` is now a non-optional dependency of
  `mclone-native-client` so the flat/offscreen lane links the openxr-free host.
  The transitive `mclone-xr-host -> openxr` link was removed by the 2026-07-10
  post-review dependency correction recorded under Slice 4.
- **Truthful readiness failure (post-review).**
  `drive_until_view_settled` now errors with server-view, client-residency,
  render-section, pending-render, compile, upload, and asset-replacement facts
  when the warmup cap is reached; it no longer freezes and captures a partial
  scene merely because one section happened to draw.
- **Exercised mono HUD (post-review).** `--headless-dual-view-hud true` routes
  the same capture through `MonoUiPresentation::ScreenSpaceHud`, so the new GUI
  renderer/atlas/draw-list path has a reproducible pixel gate before Slice 7.

Validation (all green 2026-07-09): `cargo fmt --all --check`, workspace
`cargo check`, `cargo test -p mclone-scene -p mclone-app-runtime
-p mclone-native-client` (77 scene + 185 app-runtime + 179 + native-client, 0
failed), `cargo check -p mclone-web-client --target wasm32-unknown-unknown`, and
the scripted flat/XR Android build gates. `--headless-dual-view` (seed 12345, rd 3) rendered
238 streamed sections through the host mono path (left/right offset overview
captures reviewed in `/tmp/mclone-168-dualview/`). The `--screenshot` canary
(`pnpm native:desktop-offscreen:smoke`) is **byte-identical** to the pre-slice
baseline (64 sections / 11 drawn / 2 actors — `FlatClientDriver` path untouched).
Desktop→Quest OpenXR session smoke over WiVRn-USB ran the full session lifecycle
(VISIBLE→SYNCHRONIZED→…→EXITING, frames submitted) clean with the changes
compiled in — no XR regression. A terrain-drawing worn-headset session-smoke
remains the deeper on-device check (same worn-headset limitation noted in Slice
0), but the stereo render path is byte-unchanged.

Post-review validation adds:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --headless-dual-view /tmp/mclone-168-mono-hud \
  --headless-dual-view-hud true --render-distance 3 --day-time 6000 --freeze-time
```

Inspect both captures for world pixels plus the screen-space crosshair/hotbar.
The 2026-07-10 run rendered 238/238 sections and 1,148,178 drawn indices per
view, emitted 98 GUI commands per view, and passed visual inspection. The
desktop offscreen canary still reports 64 sections / 11 drawn / 0 GUI commands.
The scripted flat and XR Android APK builds, Quest session-smoke (123 sections,
33 drawn; replacement world ready), and terrain multiview proof (2,355,570
different eye pixels) also pass. Desktop XR compilation succeeds, but its local
runtime smoke is environment-blocked when the WiVRn/Monado service is not
running; this is not a code acceptance failure and should be rerun with the
service active before Slice 4 baselines are recorded.

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

### Slice 4: Shared OpenXR frame driver — DONE (2026-07-10)

Goal: write the poll -> waitFrame -> locate -> render-or-skip -> end -> report
loop **once**, in `mclone-xr-host` (or a new thin `mclone-xr-runtime` crate if
dependency direction demands it). The transitive host-dependency cleanup that
was originally part of this slice landed early after the Slice 3 review.

Landed shape:

- `mclone-xr-host::OpenXrFrameDriver` owns the only live poll -> wait -> begin
  -> render/skip -> end sequence. Its policy carries frame limits, READY and
  submitted-progress deadlines, runtime-exit behavior, blend/view facts, and the
  single 25 ms idle interval.
- App-local handler implementations provide only platform event/lifecycle pumps,
  render callbacks, and observation hooks. Structured per-frame outcomes retain
  session state, predicted display time, should-render/disposition, separate
  wait/begin/frame-wall timings, cumulative stats, and the render result. The
  Android handler still starts thread-CPU accounting precisely after wait and
  before begin.
- `OpenXrRenderFrame` is the submission capability: callbacks may submit empty,
  stereo, or multiview projection frames exactly once. The driver closes an
  unsubmitted error frame safely. All low-level poll/end helpers are private to
  `mclone-xr-host`.
- Desktop, Android main, minimal multiview proof, terrain multiview proof, and
  terrain multiview perf now use the driver. `pnpm
  native:xr:frame-driver:purity` permanently rejects direct poll/wait/begin/end
  calls in either app crate.
- `mclone-scene::render_xr_scene_frame` owns the per-eye/multiview and
  live/frozen render selection. Shared locomotion owns the automation dispatch;
  shared scene options own startup projection plus local/remote runtime option
  projection. The duplicate event formatter and app-local mapping helpers died.

Read first: `native/crates/mclone-xr-host/src/lib.rs` (the frame primitives:
`poll_openxr_events`, `wait_begin_frame`, `end_skipped_frame`,
`end_frame_with_layers`, `end_stereo_projection_frame`,
`end_multiview_projection_frame`, `acquire_eye_target`/`acquire_stereo_target`,
`locate_stereo_views`, the `XrEyeSwapchain`/`XrStereoSwapchain` traits, and the
OpenXR→neutral conversion functions; neutral controller types now live in
`mclone-input` and view/FOV/projection types in `mclone-render-session`);
`desktop_xr.rs:415` (`run_smoke_frames`) as the simplest existing loop;
android-xr `lib.rs:3343` (`run_mclone_frame_loop`) as the fullest one; then the
three harness loops (`run_multiview_proof_loop` `:2370`,
`run_terrain_multiview_perf_loop` `:2667`, `run_terrain_multiview_proof_loop`
`:3053`).

Context: xr-host owns every frame *step* but not the loop that sequences them,
so the skeleton is copy-pasted across **five** loop bodies: desktop, Android
main, and three Android proof/perf harnesses. The Android main loop directly
calls `frame_wait.wait()` + `frame_stream.begin()` rather than the
`wait_begin_frame` helper, which is why the earlier count and grep tripwire were
misleading. The only genuinely per-surface
pieces are: (a) the outer event pump (winit companion pump on desktop vs
`poll_android_events`/`wait_for_android_resume` on Android), (b)
frame-limit/timeout policy (desktop bounded smoke frames vs Android
unbounded), (c) the concrete graphics session (already abstracted by the
swapchain traits). All three become injected values/hooks on the driver.

Deliverables:

1. **Dependency-direction fix — DONE early (2026-07-10).** Controller
   snapshots/hand identity moved to `mclone-input`; view pose/FOV/projected-view
   contracts and pure projection helpers moved to `mclone-render-session`.
   `mclone-xr-host` keeps OpenXR conversion functions and compatibility
   re-exports for app callers. `mclone-scene` dropped `mclone-xr-host` entirely.
   Separately, winit surface ownership in `mclone-render` is behind the explicit
   `native-surface` feature enabled by the desktop app. The scene cargo tree is
   empty for `openxr|mclone-xr-host|winit|android-activity|jni`; run
   `pnpm native:scene-host:purity` as the permanent check.
2. **`OpenXrFrameDriver`**, generic over the graphics session via the
   existing swapchain traits, taking: a per-frame render callback, a platform
   pump hook, frame-limit/timeout policy, and the idle poll interval.
   `SESSION_IDLE_POLL_INTERVAL` (declared identically at `desktop_xr.rs:103`
   and android-xr `lib.rs:150`) gets its single home here.
   The driver must return a structured frame outcome containing session state,
   predicted display time, should-render/submitted/skipped state, and measured
   wait/begin durations. It must provide before-render/after-frame hooks (or an
   equivalent observation callback) so Android perf accounting and smoke
   replacement timing survive without moving diagnostics policy back into the
   app loop.
3. **Migrate all five loops** onto the driver: `desktop_xr.rs:415`, android-xr
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

Tripwires (all pass): app crates contain no `wait_begin_frame`, direct `frame_wait.wait()` /
`frame_stream.begin()`, `poll_openxr_events`, or `end_*frame` calls (make the
low-level primitives crate-private to the driver if feasible rather than relying
only on grep); Quest smoke preserves the ready/replacement markers, render path,
seed, and nonzero terrain/actor output. Exact readiness-frame section counts are
not a stable gate: repeated pre-edit runs varied with asynchronous compile
scheduling, so the multiview GPU readback proofs carry the pixel/output invariant;
the `cargo tree` openxr check on `mclone-scene` is clean.

Validation: standard battery + `pnpm native:xr:desktop` +
`MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke` +
`pnpm native:android-xr:multiview-proof` and
`pnpm native:android-xr:terrain-multiview-proof` (the harness loops must still
pass after migrating onto the driver).

Validation (2026-07-10): full workspace tests, workspace/native and app-runtime
and web-client WASM checks, both scripted Android APK builds, both purity scripts,
and the offscreen pixel canary pass. Desktop WiVRn before/after runs both submit
120 frames with 121 runtime / 1 skipped frame and preserve 125 sections, 79
drawn sections, 598,740 indices, and 291,768 drawn indices. Quest session-smoke
preserves READY and replacement markers with drawn terrain; production
full-frame multiview, minimal multiview proof, terrain multiview proof, and the
terrain multiview perf harness all pass. The final terrain proof reports two
drawn sections/eye, 15,924 indices/eye, and 2,355,106 differing eye pixels.

Exit criteria (met): five loops replaced by one driver; both XR apps' loop code is
render-callback + platform pump only; `mclone-scene` has no openxr edge,
direct or transitive; device logs match baselines.

### Slice 5: Diagnostics presentation consolidation — DONE (2026-07-10)

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

1. **Shared presentation module** living in `mclone-app-runtime` beside the
   accountant: line-oriented native perf output, JSON-field embedding, and all
   `*_label` helpers now have one implementation. Android XR consumes the
   shared text lines without changing its parser-visible markers; desktop perf
   consumes the shared JSON presentation. The Android XR/desktop copies of
   stage/queue/peer construction are deleted. The raw Quest extended-probe dump
   is intentionally still platform-specific as recorded in follow-up 7 above;
   it is not a second `FramePipelineReport` presentation path.
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

Implementation (2026-07-10): `FramePipelineAccountant` now accepts a direct
`FrameAccountingConfig`, neutral queue and peer inputs, prebuilt observations,
absolute-clock reconstruction, and optional peer/budget extras. One shared
queue tracker preserves XR completed-result enqueue/dequeue ages; one shared
peer-window accumulator preserves Quest/desktop perf max/sum semantics.
`XrFramePipelineReporter` is deleted; `mclone-scene` only maps scene timing and
upload facts into neutral inputs. Desktop XR, Android XR, Quest perf
reconstruction, flat desktop, and desktop startup-streaming all use the same
accountant. Android XR `lib.rs` lost roughly 660 lines of duplicate builders,
labels, and report presentation; desktop `perf.rs` lost its diagnostic
enum/panel copy.

Validation (2026-07-10): pre-slice baselines are
`/tmp/mclone-slice5-before-quest-terrain-multiview-perf.log` and
`/tmp/mclone-slice5-before-desktop-timedemo.log`. Full workspace tests,
workspace check, app-runtime/web-client WASM checks, both Android APK scripts,
scene/frame-driver purity gates, and the inspected desktop offscreen capture
pass. Desktop startup-streaming emits schema v8 with the preserved 9-stage,
8-queue, 4-peer shape; the timedemo JSON key set is identical before/after.
Quest session-smoke reaches READY and replacement READY with drawn terrain; a
full Quest perf probe reports 721 frames, 8 queues, 4 peers, 12 XR stages, and
zero conservation violations while preserving every parsed marker family.
Terrain multiview perf/proof retain two drawn sections and 15,924 indices per
eye; the proof records 2,353,984 differing eye pixels. Desktop WiVRn submits
120/120 frames (121 runtime, one skipped) and retains 125 sections and 598,740
indices.

Exit criteria (met): one diagnostic builder set, one `FramePipelineReport`
formatter set, one accountant type; XR percentile, queue-age, remote-lane, and
peer-window semantics have regression coverage; 165 Slice 2c is resolved in
both docs.

### Slice 6: Shared adaptive render-admission policy — DONE (2026-07-10)

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

Implementation (2026-07-10): `mclone-scene::RenderAdmissionPolicy` now owns
the surviving `BudgetController`, EWMA cost estimator, frame-report pressure
window, grant conversion, and scheduler/render decision-panel merge. The host
consumes the preceding shared `FramePipelineReport`, observes the timed sync,
and applies the grant's elapsed deadline plus compile-request limit. The former
XR upload cap remains the upload-drain limit and additionally clamps the
adaptive compile-request grant; the existing pre-sync drain rule is unchanged.
Desktop flat uses the same policy type without its former local controller or
helper set, preserving its opt-in flag until Slice 7 moves the remaining flat
orchestration onto the host. Android XR and desktop XR supply compositor refresh
periods; desktop XR now enables/queries `XR_FB_display_refresh_rate` when the
runtime exposes it. Session replacement resets policy history while preserving
the selected host address.

Validation (2026-07-10): the pre-slice desktop frame-budget and Quest
sky/terrain multiview baselines are under `/tmp/mclone-slice6-before-*`. Full
workspace tests, web/WASM build, both Android APK scripts, scene/frame-driver
purity gates, startup-streaming smoke, frame-budget smoke, the inspected
desktop offscreen capture, and desktop WiVRn 120-frame mclone smoke pass. The
desktop probe retains its report shape and zero conservation violations. Quest
sky/terrain multiview retains 6 sections, 15,924 indices per eye, and nearly
identical p95 (`3.960` -> `3.963 ms`). The live uncapped Quest policy reaches
the adaptive ceiling of 4; an explicit cap of 2 emits and enforces
`max_units=2`. Both 10-second runs sustain about 72 submitted FPS with zero
accounting violations and no app-work frame-period overruns.

Exit criteria (met): `DesktopRenderBudgetController` is gone from the app
crate; one policy type is consumed by mono and XR topologies; desktop report
shape and controller semantics are preserved; Quest percentiles are within
noise of baseline.

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

- **7a — Effects/HostEffects. [LANDED 2026-07-10]** Generalize the host's effect applier
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

  Landed shape: one exhaustive shared dispatcher and capability/rejection
  projection in `mclone-scene`, plus a narrow `HostEffects` trait for mouse
  lock, pacing/cap requests, touch-mode forwarding, quit-to-title, and exit.
  Desktop and XR consume it; the desktop 24-arm match and capability-projection
  copy are gone. `FramePacingMode`, `FramePacingUiState`,
  `FramePacingDebugStats`, `FrameTimingStats`, and the
  `DebugPaneStats -> DebugOverlay` aggregator now live in
  `mclone-app-runtime`; only winit cadence/surface control remains in the app.

  Follow-ups intentionally retained: `ClientExperienceSettingsHost` has a
  temporary `FlatClientDriver` compatibility implementation until 7c deletes
  that orchestrator; Android's and web's old effect matches remain for Slice 8
  and the web follow-up; flat Android still needs to feed the now-shared debug
  aggregator in Slice 8. XR pacing/mouse/touch hooks remain harmless adapters
  behind the existing visible capability policy rather than changing XR
  behavior in 7a.
- **7b — Session start/replacement. [LANDED 2026-07-10]** Desktop routes world create/open/join
  and mid-session replacement through the host's session-runtime factory;
  delete the pending-start state machine (`app.rs:928-948`,
  `flat_client_driver.rs:1467-1781`) and desktop's `SessionStartRequest`
  handling. Collapse the triplicated `RemoteServerSession` adapter into one
  label-parameterized shared type in `mclone-net` or `mclone-app-runtime`;
  desktop and both XR apps consume it (the flat-Android copy dies in
  Slice 8).

  Landed shape: `plan_session_start` is the one exhaustive create/open/join/
  unknown classifier and produces a typed local/remote runtime plan plus the
  active-session descriptor. Both the scene host and desktop consume it. The
  desktop coordinator no longer stores a second platform payload: UI/catalog
  actions hand the plan directly to the `NativeSessionStartupPump` factory,
  the app-side after-frame finisher and `FlatClientPendingSessionStart` are
  gone, and local loading remains honestly step-driven through the existing
  startup pump. Active-session replacement is covered by a regression that
  verifies teardown followed by the new plan's local startup.

  `mclone_app_runtime::native_remote_session::NativeRemoteServerSession` now
  owns the native TCP/reconnect/update-batch adapter with a diagnostic host
  label. Desktop flat, desktop XR, and Android XR use it; the two former
  app-local adapters and their conversion copy are deleted. Flat Android's
  copy remains explicitly for Slice 8.

  Follow-ups intentionally retained: desktop still holds
  `GameSessionCoordinator<()>` and `FlatClientLocalStartup` while 7c moves
  session state and frame ownership wholesale onto `mclone-scene`; these are
  state/progress compatibility, not a second queued request dispatcher. The
  offscreen harness keeps one deferred typed plan only because scripted UI
  clicks occur before a render-frame device is available; Slice 7d deletes
  that harness staging slot. Web transport remains unchanged.
- **7c — Per-frame loop. [LANDED 2026-07-10]** `WinitFrameDriver`:
  redraw-driven, builds the mono
  view from the engine camera, acquires the window surface texture, calls the
  host's Mono frame; poll/sync/upload/traversal-ready/frame-input assembly
  all delete from the app crate. HUD assembly (`current_flat_hud`,
  `app.rs:899`) moves onto the host's screen-space presentation (which
  Slice 3 built at capture grade; this is where it becomes fully
  interactive). Desktop input events keep flowing through `mclone-input` to
  the host's input application.

  Landed shape: app-local `WinitFrameDriver` owns only winit target sizing,
  depth, render-scale presentation, surface cadence/accounting input, raw
  input delegation, and `HostEffects` outputs. The shared Mono host now owns
  live desktop local/remote runtime polling, startup/replacement, camera and
  input application, section admission/sync/upload, traversal records,
  selection/debug/Blink world overlays, actors, screen-space HUD/menu/status,
  and frame-pipeline feedback. `ChunkApp` no longer builds a
  `FlatClientUiFrame`, calls `current_flat_hud`, or performs runtime/render
  section orchestration. Its former test-only shadow `FlatClientDriver` was
  removed with the tests of that retired wiring.

  Desktop-to-host scene conversion explicitly carries render-worker timing,
  simulation cadence, first-person body visibility, adaptive render
  admission, startup LOD prewarm, storage roots, and the other shared startup
  facts; local and remote replacement conversions have regression coverage.
  The remaining `FlatClientDriver` is compatibility code used only by the
  offscreen/timedemo/perf harnesses scheduled for 7d, with dead live-app APIs
  intentionally unavailable.

  Validation: 171 native-client and 92 scene tests; workspace, desktop-XR
  feature, and web/WASM builds; scene-host and OpenXR-frame-driver purity
  gates; flat Android and Android XR APK scripts; visually inspected live
  title/in-world windows and `/tmp/mclone-desktop-offscreen.png`; 180/180
  presented live-window frames with all 81 RD3 target chunks ready; offscreen,
  timedemo, frame-budget, and two-client remote smokes; Quest new-world
  replacement ready; desktop WiVRn 120-frame smoke.

  Follow-up closed in 7d: non-default desktop render scale now presents the
  scaled world first and composes host-owned HUD/menu pixels at native output
  resolution, with size-split regressions for scaled and unscaled paths.
- **7d — Harnesses. [LANDED 2026-07-10]** CLI `--screenshot`, timedemo,
  frame-budget/movement probes, and `perf.rs` consumers repoint at the host
  through the Slice 3
  `OffscreenDriver`; `offscreen_flat_client.rs` is deleted or reduced to a
  thin driver instantiation. `perf.rs` is large (~4,900 lines) — it keeps
  measuring the same stages via the hooks that moved in Slices 5/6; budget
  extra time here.

  Landed shape: `OffscreenDriver` is the one native no-surface cadence/target
  driver over the Mono host. Screenshot/script input, dual-view capture,
  timedemo, startup-streaming, frame-budget, and movement-frame all use it;
  their former poll/sync/upload/frame-input copies are gone. The 3,619-line
  `flat_client_driver.rs` and its test-only `ui.rs` companion are deleted.
  `offscreen_flat_client.rs` is now scenario/script/PNG glue over the shared
  host, with no deferred session-start slot or render orchestration.

  The 7c render-scale follow-up is closed: scaled winit frames render the
  world to the scaled target, present it, then ask the same host to compose
  screen-space UI at native output resolution. Regression tests lock the
  world/UI size split at both scaled and unscaled resolutions. Desktop Mono
  construction also now distinguishes requested desktop scene centers from
  XR's initial-spawn-center policy; this preserved the screenshot camera,
  passive actors, and pixel composition during the migration.

  Validation: 148 native-client, 194 app-runtime, 126 render, and 92 scene
  tests; workspace, desktop-XR feature, and web/WASM checks; host-purity and
  no-app-orchestration tripwires; flat Android and Android XR APK builds;
  desktop offscreen, timedemo, frame-budget, and movement-frame smokes. The
  1280x800 HUD/debug screenshot retained its pre-slice 64/11 section counts,
  397 GUI commands, and 2/2 visible actors and was visually inspected with
  the dual-view pair. Timedemo and both frame-probe JSON schemas are identical
  to the pre-slice captures; the 60-frame probes had zero over-budget or
  accounting-conservation violations.

  Open follow-up: timedemo now measures the shared host's drawable target and
  resident GPU set instead of the retired static helper's extra tracking-halo
  batch. Its schema is stable, but the numeric baseline intentionally moved
  (debug 60-frame sample: 1,936 resident CPU sections / 664 drawable GPU
  sections versus the old 2,704 / 965 static batch). Capture and pin a fresh
  release-mode timedemo baseline before using old numeric thresholds. The
  broader old `WindowSceneRuntime` compatibility/test surface is no longer a
  frame consumer; prune it in Slice 10 rather than mixing that mechanical
  cleanup into 7d.
- **7e — Camera-reconcile timed fork. [LANDED 2026-07-10]** Shared
  `mclone_app_runtime::camera_reconcile` grows optional timing output;
  delete the `*_for_runtime_timed` fork
  (`mclone-scene/src/session.rs:1437-1540`).

  Landed shape: `EngineCameraCommitTiming` is a neutral optional output from
  the one shared camera commit. It retains the pose-command, queued-position,
  and interest-command attribution that XR folds into its existing locomotion
  fields; flat callers pass `None`. The scene-local timing record and four
  runtime wrapper/helper functions are deleted, while XR report field names
  and meanings remain stable.

  The merge exposed one behavior discrepancy hidden by the fork: the old
  untimed shared helper used a short-circuiting
  `pose_sent || apply_pending_corrections` expression, so flat callers could
  postpone a queued server correction when they also sent a pose. XR's timed
  path always performed both operations, matching the helper's documented
  contract. The unified path keeps XR behavior, applies both operations for
  every caller, and has a focused regression for this case. The reconciled
  startup regression now explicitly models the real order: accept the initial
  server correction, apply the physical startup pose, then commit and re-pump
  chunk interest.

  Validation: 195 app-runtime, 148 native-client, and 92 scene tests; workspace,
  web/WASM, desktop-XR feature, and scene-host-purity checks; flat Android and
  Android XR APK builds; visually inspected 2560x1600 offscreen terrain; a
  120-frame macOS WiVRn desktop-XR smoke that accepted the startup correction
  and submitted all requested frames; and the Quest new-world session smoke.

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

### Slice 8: Flat Android rebuilt as thin glue (delete the loop) — DONE (2026-07-10)

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
pnpm native:android:apk
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

Result:

- Replaced the 3,388-line app-local renderer/session/effects implementation
  with a 139-line entry module plus Android startup and surface/input adapters.
  `AndroidSurfaceDriver` owns winit/Android lifecycle, Vulkan surface targets,
  raw input translation, and fixed-FIFO cadence facts; the shared Mono host now
  owns runtime polling, camera reconciliation, section admission/upload,
  rendering, UI/session/catalog policy, diagnostics, travel assist, and cadence.
- Made `TouchBindings` real through `mclone-input::TouchInputAdapter`: bounded
  resolution-independent look sensitivity, the existing 50-GUI-point movement
  radius, multi-touch held/edge semantics, neutral overlay state, and a sneak
  button in the formerly empty action slot. Physical-key code parsing and
  `FlatInputFrame` merging are shared as well.
- Flat Android uses `NativeRemoteServerSession` and the shared remote-runtime
  constructor. Both Android apps now use the same legacy remote-address
  sentinel/normalization helper from `mclone-android-platform`.
- Android supplies `FrameHostKind::FlatAndroidWinit`, a fixed 60 Hz target, and
  Mono frame reports to the shared adaptive admission/accounting path. The
  native feature-exception ledger is empty: flat Android now exposes travel
  assist, frame-pipeline overlay, debug diagnostics, and server cadence; XR
  server cadence is also supported by the same host settings implementation.
- Pre-change and post-change AVD world, touch-swipe, and New World session
  captures were inspected. The post-change captures preserve terrain/HUD
  output, show the new `SNK` control, respond to the camera swipe, and complete
  shared session replacement. A separate options-driven capture shows a
  populated frame-pipeline waterfall/queue report. A background/foreground
  cycle destroys and rebuilds Vulkan, recreates the surface, and resumes
  rendering successfully.
- The full native workspace, desktop offscreen pixels, browser WASM build, both
  Android APKs, and the attached Quest New World session smoke pass after the
  shared code changes.
- Tripwires are clean: the Android app contains no app-local experience-effect
  match, render-section sync, session-request dispatch, `EngineCameraInput`
  construction, or remote transport adapter. The existing shared-host
  startup-ready log wording is now visible in Android logcat.

### Slice 9: XR-emulation lane on desktop (acceptance proof) — DONE (2026-07-10)

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

Implementation (2026-07-10): the default, non-`xr` desktop binary now exposes
`--xr-emulation-screenshot <path>`. `OffscreenDriver` selects Mono or Stereo
target ownership while the same `mclone-scene` host retains startup, runtime,
UI, locomotion, admission, and per-eye rendering. The Stereo path constructs
two host-neutral views at a fixed 0.064-block IPD with a symmetric 90-degree
vertical FoV, renders ordinary per-eye textures, reads them back, and stitches
one side-by-side PNG. It does not reference or initialize the OpenXR host,
loader, session, or swapchain code.

The optional repeatable `--xr-emulation-key <physical-code>` input goes through
`KeyboardMouseInputAdapter`, then the shared `mclone-input` flat-frame to
neutral-controller projection, then `mclone-scene::apply_frame_locomotion`.
WASD/arrow turn/jump/descend/sprint/sneak therefore exercise the real XR
left-stick, right-stick snap-turn, and button semantics rather than a second
camera path. The synthetic tracked head remains fixed in stage space; the
scene's tracking transform maps it to the engine camera/player root, so engine
locomotion and snap-turn drive the captured head. Gamepad adoption remains the
explicit Slice 10 decision; a full interactive window remains out of scope.

Reproduce the headset-free acceptance capture from the repository root:

```bash
pnpm native:xr-emulation:smoke
```

This writes `/tmp/mclone-xr-emulation.png`. `--width` and `--height` are
per-eye. To exercise keyboard-fed XR locomotion before the final capture, add
repeatable physical codes and a frame count to the underlying command, for
example `--xr-emulation-key KeyW --xr-emulation-key ArrowLeft
--xr-emulation-input-frames 8`.

Validation (2026-07-10): the inspected 1280x640 side-by-side capture has 166
resident sections, 24 drawn sections, 32 GUI commands, two world-panel eye
composites, and 268,570 differing eye pixels. Both eyes show terrain and the
same world-space menu through distinct projections; the near tree trunk and
foliage visibly shift while distant canopy geometry moves much less. A second
`KeyW` + `ArrowLeft` executable capture retained two UI composites and 140,148
differing eye pixels after running the shared locomotion/snap-turn path.

The full workspace tests, existing desktop offscreen pixel smoke, browser WASM
build, scene-host and OpenXR-frame-driver purity gates, flat Android APK, and
Android XR APK pass; the default native-client dependency graph contains no
OpenXR or XR-host/graphics crate. The attached Quest 3 session smoke reaches
OpenXR READY and draws the original world (123 sections / 33 drawn), then
reaches replacement
READY for seed 246813579 (43 sections / 10 drawn). No unresolved Slice 9
correctness issue remains; durable platform/architecture wording and the
gamepad decision are Slice 10 work.

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
   `sync_render_sections`/section-upload calls, `wait_begin_frame`, direct
   `frame_wait.wait`/`frame_stream.begin`, app-local OpenXR poll/end-frame calls,
   `SessionStartRequest` dispatch matches, and `EngineCameraInput`
   field-by-field construction. Also enforce host purity:
   `cargo tree -p mclone-scene | grep -Ei
   'openxr|mclone-xr-host|winit|android-activity|jni'` empty.
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
7. **Neutral public names.** Rename `XrMcloneTerrainState` and `XrSceneOptions`
   to `McloneSceneHost` and neutral scene-host options. Temporary aliases may
   protect slice-by-slice callers, but remove them here; mono consumers must not
   expose an XR-named core as the permanent architecture.

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

Slice 10 implementation (2026-07-10): the desktop compatibility
`WindowSceneRuntime`/`WindowSceneStartupPump` facade and its duplicate test
suite are deleted; headless/perf/rebuild harnesses now use
`NativeSceneRuntime` and the shared host directly. The public native core is
`McloneSceneHost<S>` with `McloneSceneHostOptions`; no compatibility aliases or
old XR-prefixed public names remain. Related desktop construction names are
neutral as well.

`DEFAULT_STARTUP_READINESS_TIMEOUT` is public and single-homed in
`mclone-app-runtime`; scene, desktop, and remote visual-smoke consumers import
it. `SESSION_IDLE_POLL_INTERVAL` remains single-homed in `mclone-xr-host`.
`pnpm native:thin-adapters:purity` now rejects native app-local settings or
session dispatch, render-section sync/upload policy, engine-camera literals,
and low-level OpenXR sequencing, then composes the existing scene-host and
OpenXR-driver purity gates.

The shared gamepad API is intentionally retained as a dated contract, not
synthetically enabled. No platform adapter currently provides real gamepad
events or advertises the capability; adoption requires a real desktop,
browser, or Android event source and device validation. `AGENTS.md` was audited
and already states the completed host/driver ownership boundary, so no guidance
edit was needed.

Slice 10 validation (2026-07-10): workspace tests, workspace check, direct
`wasm32-unknown-unknown` check, browser build, formatting, diff check, thin
adapter/scene-host/OpenXR-driver purity gates, and the native remote two-client
smoke pass. The inspected desktop offscreen capture renders 64 sections (11
drawn), and the inspected 1280x640 XR-emulation capture renders 166 sections
(24 drawn), two eye UI composites, and 268,570 differing eye pixels.

Real-lane coverage also passes: macOS WiVRn/Quest desktop OpenXR submitted 120
frames and rendered 125 sections (7 drawn); the arm64+x86_64 flat Android APK
build and all three `jstorrent-tablet` AVD gates pass, with touch and replacement
screenshots inspected; the Quest session-replacement smoke passes; and the
terrain multiview proof renders six sections (two drawn per eye) with 2,357,731
differing pixels. The native non-web feature-exception ledger is empty, the two
canonical timing constants each have one definition, and source scans find no
old host/options names or desktop compatibility wrappers.

## Cross-slice guardrails

1. **Delete, don't port.** When migrating a surface onto the host, the old
   orchestration is removed in the same slice. No parallel "legacy path kept
   just in case" — that is how the fork count reached five.
2. **Every slice leaves every surface shippable.** Minimum per-slice battery:
   `cargo fmt --all --check`, workspace `cargo check`, `cargo test` for
   touched crates, wasm `cargo check` for `mclone-web-client`,
   and the scripted Android build gates (`pnpm native:android:apk` and
   `pnpm native:android-xr:apk`; the scripts own NDK/toolchain discovery—do not
   hand-roll `cargo ndk`).
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
   existing wasm-compatible patterns; no bare `std::thread` in the host. The
   pre-existing bare `Instant` use is a recorded web-adoption blocker above:
   do not add more, and remove it behind the clock abstraction before declaring
   the direct scene WASM gate green.
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
- Web was compile-only for this tactical. Tactical 170 subsequently adopted
  the same host for all browser modes and added the full behavioral matrix.
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
| [`067-shared-render-worker-architecture.md`](067-shared-render-worker-architecture.md) / [`062-shared-threading-topology.md`](062-shared-threading-topology.md) | Own the web worker/threading convergence that [`170-web-scene-host-adoption.md`](170-web-scene-host-adoption.md) will build on; the seams in this doc (runner trait, step-based APIs) are prerequisites, not replacements. |

## How to continue (for the implementing agent)

Tactical 168 is complete through Slice 10. Preserve the executable native
thin-adapter and headset-free stereo gates when adding scene-host behavior.
Web adopted the runner/session seams prepared in Slice 2 and the shared host in
completed [`170-web-scene-host-adoption.md`](170-web-scene-host-adoption.md).
The default thin-adapter gate now covers its browser driver as well as native
apps; remaining browser feature gaps are parity work, not host ambiguity.
