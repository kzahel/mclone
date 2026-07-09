# 165: Native Feature-Parity Baseline And Exception Burn-Down

Status: active 2026-07-09. Slice 1 (baseline mechanism + flat-Android far LOD)
landed in `da719dda`; the remaining exceptions are open burn-down slices.

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
cargo fmt --manifest-path native/Cargo.toml --package mclone-app-runtime --package mclone-native-client --package mclone-xr-scene --package mclone-android-client --package mclone-android-xr-client --package mclone-web-client --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-xr-client
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
| Flat Android | `FramePipelineOverlay` | Effect handler rejects; needs an Android overlay renderer. |
| Flat Android | `DebugDiagnostics` | Effect handler rejects; needs an Android diagnostics panel. |
| Flat Android | `ServerSimulationCadence` | Effect handler rejects; needs integrated-server cadence wiring. |
| XR (desktop + Android) | `ServerSimulationCadence` | Not yet wired on the XR scene path. |

Web keeps `FarLod`, `TravelAssist`, `FramePipelineOverlay`, `DebugDiagnostics`,
and `ServerSimulationCadence` gated with explicit reasons; decided separately on
threading/perf grounds (far LOD "measure on web first", Tactical 162).

## Remaining Slices

1. **Flat-Android travel assist.** Wire `SetTravelAssistMode` into the Android
   camera/movement path (desktop `flat_client_driver` is the reference); drop the
   ledger entry and flip the capability.
2. **Flat-Android frame-pipeline overlay.** Route the shared frame-pipeline
   overlay through the Android GUI path; drop the ledger entry.
3. **Flat-Android debug diagnostics.** Route the shared debug-diagnostics panel
   through the Android GUI path; drop the ledger entry.
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
