# 168: Unified Native Scene Host

Status: approved direction 2026-07-09; implementation not started. Slice 0 is
ready to begin with no other context beyond this document and the referenced
code.

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

### Slice 0: Bug-grade parity pre-fixes

Goal: fix the behavior divergences that exist today, before any structural
work, so later slices migrate correct behavior instead of enshrining bugs.
Each fix is small and independently landable.

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

### Slice 1: OpenXR-free scene crate (`mclone-xr-scene` -> `mclone-scene`)

Goal: neutralize the rim. Behavior-free.

Deliverables:

- Introduce host-neutral view/pose types in the scene crate (the internal
  render-view representation that `render_views(&views)` already produces
  becomes the public API surface).
- Move `xr::View`/`xr::Fovf` -> neutral-view conversion into `mclone-xr-host`
  (or a tiny adapter module in the XR apps temporarily).
- Remove the `openxr` dependency from the scene crate's `Cargo.toml`.
- Rename the crate directory + package `mclone-xr-scene` -> `mclone-scene` as
  a **separate, purely mechanical commit** within the slice (imports only, no
  logic edits in the rename commit).

Tripwires: `grep -rn "openxr" native/crates/mclone-scene/` returns nothing;
xr-scene test suite (74 tests at time of writing) passes unchanged under the
new name; no `xr::` type appears in any `pub fn` signature of the scene crate.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-scene -p mclone-app-runtime -p mclone-native-client
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client -p mclone-android-xr-client
pnpm native:xr:desktop            # desktop XR smoke (see docs/platforms.md for runtime setup)
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke
```

Exit criteria: identical on-device behavior (same session-smoke log lines and
seed section counts as the pre-slice baseline — capture the baseline first).

### Slice 2: Web-shaped seam — runner-generic local host mode

Goal: make `NativeSessionRuntime`'s local mode generic over the existing
`IntegratedServerRunner` trait instead of hard-coding
`NativeIntegratedServerRunner` (`native_session_runtime.rs:275`). Native
callers keep using the native runner via a default/alias; nothing about web is
migrated — this slice only removes the seam that would otherwise bake the fork
in permanently.

Tripwires: app-runtime tests pass; no public API breakage for existing native
callers beyond mechanical type-parameter additions; wasm check clean.

Validation: the standard check/test battery from Slice 1 (no device smoke
needed — no behavior change — but run the desktop offscreen screenshot as a
cheap pixel canary: `pnpm native:desktop-offscreen:smoke`).

### Slice 3: Mono view topology + first flat consumer (offscreen/headless)

Goal: the scene host can render a single flat view.

Deliverables:

- Add `Mono` topology to the host, rendering through the existing shared
  `render_full_frame_for_view*` entry points
  (`mclone-app-runtime/src/frame_render.rs`), including far-LOD variant.
- Screen-space UI presentation strategy (HUD assembly on the host; the
  world-quad XR presentation remains the other strategy).
- Port the headless/offscreen screenshot path (`headless.rs` frame assembly,
  `offscreen_flat_client.rs`) to consume the host as an `OffscreenDriver`,
  deleting the third copy of frame-input assembly (`headless.rs:844-891`).

Tripwires: offscreen screenshots via the host are pixel-equivalent (or
reviewed-equivalent) to pre-slice captures; XR device smokes unchanged.

Validation: standard battery + `pnpm native:desktop-offscreen:smoke` +
compare `/tmp/mclone-desktop-offscreen.png` against a pre-slice capture, plus
one Quest session-smoke to prove no XR regression.

### Slice 4: Shared OpenXR frame driver

Goal: write the poll -> waitFrame -> locate -> render-or-skip -> end -> report
loop **once**, in `mclone-xr-host` (or a new thin `mclone-xr-runtime` crate if
dependency direction demands it).

Deliverables:

- `OpenXrFrameDriver` generic over the graphics session (the
  `XrEyeSwapchain`/`XrStereoSwapchain` traits already abstract Metal/Vulkan),
  taking a per-frame render callback and a platform pump hook (winit companion
  pump on desktop, `poll_android_events` on Android) and frame-limit/timeout
  policy as injected values.
- Migrate all six loops onto it: `desktop_xr.rs:415`, android-xr `lib.rs:3304`,
  `:2370`, `:2667`, `:3020`, and unify the `render_mclone_frame` twins
  (`desktop_xr.rs:972`, android-xr `lib.rs:7020`/`:7145`), moving the
  automation-dispatch `match` next to the locomotion methods it calls.
- Dedup `log_openxr_host_event` (`desktop_xr.rs:294` / android-xr `lib.rs:7006`)
  and the scene-option adapter triplication (make
  `local_integrated_scene_options`, `session.rs:1565`, public; delete
  `android_xr_local_options`/`android_xr_host_options` and
  `xr_scene_options_from_desktop_scene` copies).

Tripwires: grep — no app crate contains a `wait_begin_frame` call outside the
shared driver; Quest smoke log lines (session-smoke ready summaries,
`drawn_sections`/`drawn_indices`) match pre-slice baselines.

Validation: standard battery + `pnpm native:xr:desktop` +
`MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke` +
`pnpm native:android-xr:multiview-proof` and
`pnpm native:android-xr:terrain-multiview-proof` (the harness loops must still
pass after migrating onto the driver).

### Slice 5: Diagnostics presentation consolidation

Goal: one set of frame-report builders and formatters.

Deliverables:

- Move report *presentation* (`log_summary`/`log_worst_frames`, `*_label`
  formatters — android-xr `lib.rs:4472-7020`) next to the shared builders in
  the scene crate's `frame_pipeline_reporter`; delete the android-xr duplicate
  builders (`android_xr_render_stage_spans` `lib.rs:5502`,
  `android_xr_peer_thread_panel` `:5637`, `remote_host_queue_report` `:5769`,
  `server_runner_peer_report` `:5774`, `worker_metrics_peer_report` `:5738`,
  `android_xr_queue_panel` `:5549`) and the desktop `perf.rs:2995-3284`
  triplication.
- Converge the two accountants toward one host-owned accountant emitting
  `FramePipelineReport`. **Coordinate with 165 Slice 2c**, which already
  scopes the `XrFramePipelineReporter` collapse (single-shot `record_frame`
  feed, `NearestRank` percentiles, queue-age semantics, peer-thread panel must
  be preserved exactly). If 165-2c has landed, consume its result; if not,
  land it here and mark it done there. Do not implement it twice.

Tripwires: per the XR render-path guardrail, XR-pixel-visible output changes
require desktop-XR + Quest capture validation; percentile/age semantics must
be preserved exactly (compare a Quest perf summary line-by-line pre/post).

Validation: standard battery + Quest session-smoke +
`pnpm native:android-xr:terrain-multiview-perf` (report formatting is exactly
what this exercises).

### Slice 6: Shared adaptive render-admission policy

Goal: one client-side upload-admission policy on the host.

Deliverables:

- Move `DesktopRenderBudgetController` (`flat_client_driver.rs:186-236`) into
  the host as the shared admission policy, fed by the (now single) frame
  accountant; per-driver input is only the target frame period (compositor
  refresh on XR, vsync/pacing period on desktop flat).
- The XR static cap (`render_section_upload_budget`,
  `mclone-scene/src/options.rs:195`) becomes an optional override on the same
  policy, not a parallel mechanism; the pre-sync drain rule
  (`should_drain_before_runtime_sync`) stays as shared policy.

Tripwires: desktop frame-budget probes show unchanged admission behavior;
Quest perf smoke shows no frame-time regression (the adaptive controller must
not admit more work than the old static cap did on Quest — if it does, tune or
gate with the override before landing).

Validation: standard battery + `pnpm native:frame-budget:smoke` +
`pnpm native:startup-streaming:smoke` + Quest
`pnpm native:android-xr:sky-terrain-multiview-perf`.

### Slice 7: Desktop flat onto the host (delete `FlatClientDriver` orchestration)

Goal: desktop flat becomes `WinitFrameDriver` + host. Biggest slice; may land
as several PRs, each keeping desktop green.

Deliverables:

- Desktop per-frame loop consumes the host's Mono topology (poll/sync/upload/
  frame-input assembly all deleted from the app crate).
- Effects wiring through the host's single effect applier + `HostEffects`
  impl (mouse lock, present mode/frame pacing, quit). Desktop's frame-pacing
  driver (`frame_pacing.rs`) stays app-local as the cadence source but its
  POD stats structs move shared (165 Slice 3 already scopes this — coordinate,
  don't duplicate).
- Session replacement through the host's factory: delete the pending-start
  state machine (`app.rs:928-948`, `flat_client_driver.rs:1470-1740`) and the
  desktop copy of the `SessionStartRequest` dispatch.
- Collapse the triplicated `RemoteServerSession` adapter into one shared type
  (label-parameterized) in `mclone-net` or `mclone-app-runtime`; desktop
  consumes it (Android copies die in Slice 8 / already-thin XR apps repoint).
- HUD assembly (`app.rs:899-919 current_flat_hud`) moves onto the host's
  screen-space presentation.
- CLI, screenshot, timedemo, perf harnesses repoint at the host through the
  offscreen driver from Slice 3.
- Absorb the XR timed camera-reconcile fork: shared `camera_reconcile` grows
  optional timing output; delete `session.rs:1437-1540` equivalents in the
  renamed crate.

Tripwires: `flat_client_driver.rs` shrinks to (or is deleted in favor of)
winit/surface/input glue; grep — no per-frame `sync`/`upload`/frame-input
assembly outside the host; desktop screenshot/timedemo/perf baselines match.

Validation: full desktop suite —

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
driver instantiation.

Deliverables:

- `AndroidSurfaceDriver` (AndroidApp events + surface + suspend/resume ->
  host lifecycle calls). Suspend persistence now comes from the host's
  `on_background()` policy — the same policy Quest uses after Slice 0.
- Touch controls move into `mclone-input` as the real implementation of the
  currently-dead `TouchBindings` (`mclone-input/src/lib.rs:1257-1310`),
  replacing the hand-rolled `AndroidMovementTouch`/`AndroidTouchControls`
  (`lib.rs:276-398`) — including the screen-normalized configurable touch-look
  model (`lib.rs:690-707`), which becomes the shared touch-look semantics.
- Delete the Android copies of: the per-frame loop, effect arms, catalog/session
  wrappers, `engine_camera_input_from_flat_frame`, key tables, remote-session
  adapter, startup-option duplication. Reconcile the two Android surfaces'
  remote-addr resolution (`debug.mclone.remote_addr` property vs android-xr's
  sentinel normalization) into one shared helper.
- Android inherits from the host: adaptive render admission, frame-pipeline
  accounting + overlay data, travel assist, debug diagnostics — which should
  clear the corresponding 165 ledger rows. **Flip each 165 capability to
  `Supported` in the same change that lands its plumbing, and drop the ledger
  entries** (165's enforcement tests require exactly this ordering).

Tripwires: `mclone-android-client` contains no `ClientExperienceSettingEffect`
match, no `sync_render_sections` call, no session-request dispatch; AVD smoke
log lines use the same wording/counters as desktop/XR; 165 ledger shrinks.

Validation:

```bash
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-smoke
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-touch-smoke
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-session-smoke
# plus the standard battery and one Quest session-smoke (shared-code regression)
```

Inspect the AVD screenshots (`/tmp/mclone-android-avd-*.png`) — terrain drawn,
touch controls responsive, frame-pipeline overlay now populated when toggled.

### Slice 9: XR-emulation lane on desktop (acceptance proof)

Goal: prove the unification by running the stereo scene with a synthetic head.

Deliverables: a desktop lane (CLI flag or offscreen mode) that feeds synthetic
stereo poses through the host's Stereo topology and renders side-by-side or
offscreen captures — no OpenXR runtime involved. Snap-turn/locomotion driven
from keyboard/gamepad through the same locomotion path.

Validation: capture a side-by-side stereo screenshot to `/tmp` and inspect it;
both eyes show correctly-offset views (per-eye view/projection invariant
holds); standard battery.

### Slice 10: Cleanup, enforcement, docs

Deliverables:

- Delete compatibility wrappers that survived only for slice safety.
- Tripwire enforcement (tests or a grep script beside the existing 167-style
  tripwires): app crates must not contain `ClientExperienceSettingEffect`
  matches, per-frame section sync/upload calls, `wait_begin_frame`, or
  `SessionStartRequest` dispatches. Shared timeout/interval constants get one
  home (the 120s readiness timeout currently has five copies; the 25ms idle
  poll interval two).
- Delete or adopt the now-orphaned `GamepadInputAdapter` (decide: keep as the
  contract for a future gamepad slice with a note, or remove).
- Update [`../platforms.md`](../platforms.md) (validation matrix / entry
  points) and [`../native-engine-architecture.md`](../native-engine-architecture.md)
  (crate ownership shape) to describe the host + drivers model.
- Update 165 (ledger state) and 163/167 follow-up notes.

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
