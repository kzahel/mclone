# 165: Native Feature-Parity Baseline And Exception Burn-Down

Status: active 2026-07-09. Slice 1 (baseline mechanism + flat-Android far LOD)
landed in `da719dda`. Slice 2a (shared frame-pipeline accountant owner) landed
2026-07-09; Slice 2c (XR/flat accountant convergence) landed through tactical
168 Slice 5 on 2026-07-10; the remaining exceptions are open burn-down slices.
Coordination update 2026-07-10: tactical 168 now owns the runtime/host plumbing
that clears the remaining native rows; do not grow the soon-to-be-deleted
Android or desktop loops to complete them independently.

Workstream: native Rust shared client-experience policy in `mclone-app-runtime`,
desktop flat, shared XR scene, flat Android, and Android XR. Web is the one
non-native target and is exempt from native feature parity, but only with
recorded reasons (never a silent gate). Extends
[`../client-experience-architecture.md`](../client-experience-architecture.md)
(the client-experience owner from Tacticals 141/143).

## Principle

All *native* platforms (desktop flat, desktop/Android XR, flat Android) expose
the **same set** of core engine features. Defaults may differ per platform;
**feature availability may not**. A feature that is available on one native
target but `Unsupported` on another is a bug and a refactor signal, not a
platform difference — unless the divergence is a genuine runtime-plumbing gap
being actively burned down, in which case it must be *explicit and reason-bearing*.

Web may diverge on features (different threading/runtime model), but every web
feature divergence must still carry an explicit reason.

## Root cause this fixes

Before this tactical, each of the four profile constructors
(`desktop_client_experience_profile`, `xr_client_experience_profile`,
`android_client_experience_profile`, `web_client_experience_profile`) lived in
its own app/XR crate and independently started from
`ClientExperienceSettingsProfile::all_supported()` then subtracted. There was no
shared "native baseline," so capability sets drifted per platform. The visible
symptom was the Far LOD graphics setting (Tacticals 121/162): wired through the
shared model and working on desktop flat, desktop XR, and Android XR, but
capability-gated **off** on flat Android with a bare per-app `Unsupported`, and
on web with a silent `PROFILE_UNSUPPORTED`. Reading the flat-Android effect
handler showed four *other* feature capabilities were silently gated the same
way (travel assist, frame-pipeline overlay, debug diagnostics, server simulation
cadence), and XR silently gated cadence — none for hardware reasons.

## Slice 1: Baseline Mechanism + Flat-Android Far LOD

Status: landed 2026-07-09 in `da719dda`.

Change:

- Relocate all four profile constructors into
  `mclone_app_runtime::client_experience` as
  `desktop_native_client_experience_profile`,
  `xr_native_client_experience_profile`,
  `android_flat_native_client_experience_profile`, and
  `web_client_experience_profile`, each derived from
  `native_client_experience_baseline()` (== `all_supported`). The XR crate keeps
  a thin `xr_client_experience_profile` alias for its several call sites; desktop,
  Android, and web repoint their single call sites and delete their locals.
- Add the **feature axis**: `ClientExperienceFeatureCapability` +
  `ClientExperienceSettingsProfile::feature_axis()` enumerate the 15 capabilities
  whose *availability* must be uniform across native. Input/surface-shaped
  capabilities (crosshair, turn, XR turn, touch look, touch controls, frame
  pacing, fps cap) are intentionally excluded and may vary per platform.
- Add `NativePlatform` and the `NATIVE_FEATURE_PARITY_EXCEPTIONS` ledger. Every
  entry is a genuine runtime-plumbing gap to burn down toward the baseline, not a
  permanent difference.
- Add three enforcement tests:
  - `native_targets_share_feature_capability_availability` — a feature-axis
    capability may only be `Unsupported` if it is reason-bearing (never a bare
    `PROFILE_UNSUPPORTED`) *and* declared in the ledger.
  - `native_feature_parity_exceptions_are_live` — a declared exception that has
    become `Supported` is stale and must be removed.
  - `web_feature_divergences_are_reason_bearing` — web may gate features but never
    silently.
- Promote `far_lod` into the shared `StartupSceneOptions` and thread it through
  desktop (`to/from_startup_scene`), flat Android, and Android XR startup
  builders, retiring the divergent per-app defaults for that field.
- Wire far LOD on flat Android: add shared
  `render_full_frame_for_view_with_far_lod`, give the Android renderer a
  `FarTerrainLodRenderer` + live `FarTerrainLodConfig`, mirror desktop's
  `SetFarLod`/`ClearFarLod` effect handling, prepare the mesh per frame, reflect
  live state in `GameUiRenderState`, and flip the flat-Android `far_lod`
  capability to `Supported` (dropping its ledger entry).

This converted two pre-existing *silent* drifts (XR `server_simulation_cadence`
and web `far_lod`/`travel_assist`/`server_simulation_cadence`) into
reason-bearing, tracked exceptions.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-scene
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:android:apk
pnpm native:android-xr:apk
# Pixel validation (temporary startup far-LOD toggle for the AVD capture, reverted after):
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --screenshot /tmp/mclone-desktop-farlod.png --render-distance 2 --far-lod true --day-time 6000 --freeze-time --startup-wait idle
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-smoke   # tablet AVD is arm64-v8a, not x86_64
git diff --check
```

Result on 2026-07-09: passed. 173 runtime + 173 desktop tests pass (incl. the 3
new enforcement tests). Desktop `--far-lod true` offscreen capture and the
flat-Android far-LOD tablet-AVD capture both render cleanly with no crash (the
seed-12345 ground-level forest occludes the distant LOD ring on both, same as
desktop). The wasm web check still reports the existing `mclone-server` warnings
for `TimingSample` and `with_unload_hysteresis_chunks`, and the native-client
build still reports the existing `DESKTOP_LOCAL_ARG_FLAGS` warning; no new
warnings were introduced.

## Exception Ledger (burn-down target)

Each row is a native feature that is not yet uniformly available. The goal is an
empty non-web ledger. Remove the `NATIVE_FEATURE_PARITY_EXCEPTIONS` entry and
flip the capability to `Supported` in the same slice that lands the runtime
plumbing — never before, or the profile advertises a feature the effect handler
still rejects.

| Platform | Feature | Current gap |
|---|---|---|
| Flat Android | `TravelAssist` | Effect handler is a silent no-op; needs camera travel-assist wiring. |
| Flat Android | `FramePipelineOverlay` | **Not a renderer gap** — the overlay renderer (`render_frame_pipeline_overlay`) is already shared and drives desktop *and* XR. The gap is a missing *data source*: the frame-pipeline accountant lives in the desktop app crate (`DesktopFramePipelineAccounting`), so Android has no `FramePipelineReport` producer to attach and stubs the effect + hard-codes visibility `false`. |
| Flat Android | `DebugDiagnostics` | Same shape: the draw path (`render_debug_overlay_at`) is shared, but the stats aggregator (`DebugPaneStats`) is desktop-app-local. Android already owns every input it aggregates but has no shared aggregator to build the `DebugOverlay`. |
| Flat Android | `ServerSimulationCadence` | Effect handler rejects; needs integrated-server cadence wiring. |
| XR (desktop + Android) | `ServerSimulationCadence` | Not yet wired on the XR scene path. |

### Diagnostics producer ownership (root cause behind the two overlay rows)

The `FramePipelineOverlay` and `DebugDiagnostics` rows are **not** independent
"write an Android panel" tasks — they share one root cause: the diagnostics
*content renderers* are correctly shared in `mclone-ui` (and consumed unchanged by
flat desktop, flat Android's HUD path, and XR world-quads alike), but the
diagnostics *data producers* are stranded in the desktop app crate. This is the
same "no shared owner" gravity that produced the original profile drift, one layer
down. Symptom of the drift: XR already hand-rolls a near-1:1 duplicate of the
desktop accountant (`XrFramePipelineReporter` vs `DesktopFramePipelineAccounting`),
so a naive Android port would create a *third* copy.

Owner decision: **`mclone-app-runtime`.** The crate graph makes this the only
non-cyclic home for code that needs `mclone-ui` and runtime types together
(`mclone-ui`, `mclone-diagnostics`, and `mclone-render-session` each already sit
*below* `mclone-app-runtime`, so hosting there would cycle). It also already owns
the accountant's inputs (`SingleViewRuntimeStats`, `RenderStreamStats`) and the
`client_experience` effect vocabulary — diagnostics production is runtime
orchestration, the same engine-policy class this tactical already keeps there.
The only genuinely app-local inputs are three frame-pacing POD structs
(`FramePacingMode`, `FramePacingDebugStats`, `FrameTimingStats`), which are pure
data merely co-located with the winit-bound `FramePacing` driver and can be split
out cleanly.

Web keeps `FarLod`, `TravelAssist`, `FramePipelineOverlay`, `DebugDiagnostics`,
and `ServerSimulationCadence` gated with explicit reasons; decided separately on
threading/perf grounds (far LOD "measure on web first", Tactical 162).

## Remaining Slices

**Coordination with tactical 168 (2026-07-10):** the items below remain the
capability acceptance requirements, but their implementation routing changed:

- travel assist and server cadence land through the shared scene host during
  168 Slices 7/8, not as Android-loop additions;
- accountant convergence (2c) is 168 Slice 5;
- frame-pacing POD/debug-aggregator promotion is coordinated with 168 Slices
  7a/8;
- Android frame-pipeline wiring is inherited when the old Android loop is
  deleted in 168 Slice 8. Do **not** execute the old "next — slice 2b" recipe by
  adding timing brackets to that app-local loop; it would create code that Slice
  8 immediately removes.

Capability flips and ledger-row removal still happen atomically with the shared
plumbing, as required by this tactical's enforcement tests.

1. **Flat-Android travel assist.** Wire `SetTravelAssistMode` into the Android
   camera/movement path (desktop `flat_client_driver` is the reference); drop the
   ledger entry and flip the capability.
2. **Promote the frame-pipeline accountant to a shared owner, then wire flat
   Android.** This is primarily a shared-owner refactor, not an Android renderer.
   - **[LANDED 2026-07-09 — slice 2a]** Relocated `DesktopFramePipelineAccounting`
     into `mclone_app_runtime::frame_pipeline_accounting` as the surface-neutral
     `FramePipelineAccountant` (shared `FrameAccumulator` + queue trackers +
     revision/latest-report bookkeeping; each surface still feeds its own measured
     stage/surface timings — that feeding is legitimately per-loop). Its inputs
     (`SingleViewRuntimeStats`, `RenderStreamStats`, `RenderSectionSyncTiming`)
     already lived in `mclone-app-runtime`, so the move added no new crate edges.
     Desktop `flat_client_driver` repoints to the shared type; the desktop-local
     module and its tests are deleted. Validated: `mclone-app-runtime` tests
     (incl. the two moved accountant tests) pass; desktop and wasm builds clean.
   - **[LANDED 2026-07-10 — slice 2c via tactical 168 Slice 5]** Removed
     `XrFramePipelineReporter`; desktop flat, desktop XR, Android XR, and desktop
     perf reconstruction now use the single shared `FramePipelineAccountant`.
     The accountant accepts `FrameAccountingConfig`, neutral queue-depth and
     peer-thread inputs, `record_prebuilt` / absolute-clock reconstruction, and
     optional peer/budget extras. XR keeps `NearestRank`, completed-result
     enqueue/dequeue age semantics, remote-lane availability, and its peer panel.
     A shared peer-window accumulator preserves the Quest/desktop perf max/sum
     semantics, and shared presentation preserves the parser-visible Quest labels.
   - **[subsumed by tactical 168 Slice 8 — do not implement app-locally]** On flat Android, instantiate the shared accountant,
     un-gate the profile, replace the `SetFramePipelineOverlayVisible(_)` no-op
     with a real `bool` handler, feed the flag through `current_ui_render_state`,
     and populate `hud.frame_pipeline`. The draw link already exists, but note:
     the Android render loop currently instruments **no** per-stage timings, so
     this slice must first add `Instant` brackets across its loop (frame-active
     wall, runtime-poll, the `_timed` section-sync variant, upload-apply, render,
     surface acquire/submit/present) and a target-frame-period source before the
     accountant has anything to report. Then drop the ledger entry and capture a
     tablet-AVD frame-pipeline overlay screenshot.
3. **Promote the debug-stats aggregator, then wire flat Android.** Same pattern.
   - Move the three frame-pacing POD structs out of the winit-bound
     `frame_pacing.rs` into a shared `mclone-app-runtime` location (leave the
     `FramePacing` driver in the desktop app), then relocate the
     `DebugPaneStats -> DebugOverlay` aggregator beside them.
   - On flat Android, feed the aggregator from its existing inputs, un-gate the
     profile, replace the `SetDebugDiagnosticsVisible(_)` no-op with a real `bool`
     handler, feed the flag through `current_ui_render_state`, and populate
     `hud.debug`. Drop the ledger entry. (XR may later converge its own
     `debug_diagnostics_overlay` stat assembly onto the shared aggregator, but
     that is optional and not required to clear this Android row.)
4. **Server simulation cadence on XR + flat Android.** Wire cadence into the
   integrated-server option path for the XR scene runtime and the Android
   renderer; drop both ledger entries. Cadence is only meaningful for
   local-integrated sessions, so gate on host mode rather than platform.
5. **Web far-LOD decision.** Separate from native parity: decide whether web's
   threading/runtime model can carry far LOD (Tactical 162) before removing its
   web reason.
6. **Settings persistence (orthogonal, same root cause).** Native graphics
   settings still reset to defaults every launch; only web `localStorage`
   persists two touch settings. The absence of a shared preferences owner is the
   same "no shared per-session options owner" gap that produced the profile drift.
   Track a shared preferences owner as a follow-up so `far_lod` and future scene
   settings persist across launches on all native targets.

## Guardrails

- Keep profile constructors and the feature-axis/exception ledger in
  `mclone-app-runtime`; they are engine policy, not platform glue.
- Never add a feature-axis divergence as a bare `PROFILE_UNSUPPORTED`. If a native
  target genuinely cannot honor a feature yet, add a reason-bearing `Unsupported`
  and a ledger entry in the same change (the enforcement test requires this).
- Do not flip a capability to `Supported` without the runtime plumbing landing in
  the same slice; validate pixel-producing features with a device/AVD capture.
- Keep input/surface-shaped capabilities (turn/touch/crosshair/pacing/fps) on the
  input axis; they are free to differ per platform and must not be forced uniform.
