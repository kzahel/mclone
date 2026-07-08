# 161: Native Catalog / Storage Adapter Dedup Follow-Up

Status: active 2026-07-08. Slices 1-5 landed; follow-up slices remain open.

Workstream: native Rust shared client-experience/catalog/storage boundary,
desktop flat, shared XR scene, flat Android, and Android XR. Web remains
intentionally separate where browser IndexedDB or JS async plumbing requires it.

## Goal

Follow Tactical 160 by removing the smaller platform-local copies that still sit
around the shared native catalog executor and SQLite persistence backend. The
target is not to erase platform adapters; it is to keep only the parts that truly
own surface/session/GPU/Android/XR lifecycle and lift repeated catalog/storage
policy into shared native owners.

Current floor after 160:

- One native catalog request executor:
  `mclone_app_runtime::execute_world_catalog_request`.
- One native catalog backend:
  `NativeWorldCatalog` over SQLite world dirs.
- One native runtime persistence backend for opened local worlds:
  `SqliteWorldStore` through the shared native server runner.
- Web catalog and world storage stay async IndexedDB `+1` paths.

## Slice 1: Shared Catalog UI Refresh

Status: landed 2026-07-08 in `b530df74`.

Problem: desktop flat, XR, and flat Android each had the same
`refresh_world_catalog_ui` body: missing catalog -> default empty list; list
worlds; update `ClientCatalogController`; surface list errors as
`WorldCatalogUiStatus`.

Change:

- Extend native `WorldCatalog` with `list_worlds`.
- Add `mclone_app_runtime::refresh_world_catalog_controller`.
- Route desktop flat, XR, and flat Android refresh methods through it while
  preserving platform-specific log labels and caller-owned UI invalidation.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Result on 2026-07-08: passed. The wasm web check still reports the existing
`mclone-server` warnings for `TimingSample` and
`with_unload_hysteresis_chunks`, and the native-client XR feature check still
reports the existing `DESKTOP_LOCAL_ARG_FLAGS` warning; no new warnings were
introduced by this slice.

## Slice 2: Catalog Session-Start Scene Derivation

Status: landed 2026-07-08 in `59db7eb8`.

Problem: desktop flat, XR, and flat Android each derived local/catalog/remote
scene options by hand: transient local starts clear `remote_addr`/`world_dir`,
catalog starts set the opened world dir, and remote starts clear `world_dir`
while suppressing the desktop/XR adaptive local publication budget.

Change:

- Add `mclone_app_runtime::session::SessionStorageIntent` to encode the shared
  local transient, local catalog, and remote session storage intent.
- Route desktop flat, XR, and flat Android scene derivation through the shared
  intent while keeping concrete scene structs, render-distance refresh,
  OpenXR/session replacement, Android renderer ownership, and desktop mouse-lock
  lifecycle local to their adapters.
- Add shared unit coverage for the intent so future storage flags land in one
  place first.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --package mclone-app-runtime --package mclone-native-client --package mclone-xr-scene --package mclone-android-client --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Result on 2026-07-08: passed. The wasm web check still reports the existing
`mclone-server` warnings for `TimingSample` and
`with_unload_hysteresis_chunks`, and the native-client XR feature check still
reports the existing `DESKTOP_LOCAL_ARG_FLAGS` warning; no new warnings were
introduced by this slice.

## Slice 3: Shared Integrated-Session Storage Mapping

Status: landed 2026-07-08 in `50694e36`.

Problem: desktop flat, shared XR, flat Android, and Android XR each mapped their
scene `world_dir` into `LocalSingleViewSceneOptions::with_persistent_world_dir`
by hand. Desktop/XR/Android XR also set the adaptive chunk-publication flag near
that storage projection.

Change:

- Add `IntegratedWorldSessionStorage` in `mclone-app-runtime` as the shared
  host-neutral storage patch for local integrated sessions.
- Add `LocalSingleViewSceneOptions::with_integrated_world_session_storage` so
  persistent world dir and adaptive chunk-publication policy are applied in one
  shared place.
- Route desktop flat, shared XR, flat Android, and Android XR builders through
  the helper while preserving each platform's concrete scene/options struct.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --package mclone-app-runtime --package mclone-native-client --package mclone-xr-scene --package mclone-android-client --package mclone-android-xr-client --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Result on 2026-07-08: passed. The wasm web check still reports the existing
`mclone-server` warnings for `TimingSample` and
`with_unload_hysteresis_chunks`, and the native-client XR feature check still
reports the existing `DESKTOP_LOCAL_ARG_FLAGS` warning; no new warnings were
introduced by this slice.

## Slice 4: Android App-Data World-Root Helper

Status: landed 2026-07-08 in `da290e7c`.

Problem: flat Android and Android XR each resolved their default persistent
world catalog root with the same `internal_data_path().or_else(external_data_path()).join("worlds")`
logic and only differed in the log label.

Change:

- Add `mclone-android-platform` as a small Android-specific shared glue crate,
  separate from host-neutral `mclone-app-runtime`.
- Add `android_world_root_from_app_data_paths` with host unit tests for the
  internal-first fallback policy.
- Add Android-only `android_app_data_world_root` over `AndroidApp`, preserving
  the existing per-app log labels.
- Route flat Android and Android XR through the shared helper.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --package mclone-android-platform --package mclone-android-client --package mclone-android-xr-client --check
cargo test --manifest-path native/Cargo.toml -p mclone-android-platform
cargo check --manifest-path native/Cargo.toml -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml --target aarch64-linux-android -p mclone-android-platform
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-xr-client
git diff --check
```

Result on 2026-07-08: passed. The direct raw-target app checks were not used
because plain `cargo check --target aarch64-linux-android` does not set the NDK
compiler environment and failed before app compilation with missing
`aarch64-linux-android-clang`; the repo's `cargo ndk` path compiled both Android
apps for `arm64-v8a`. The wasm web check still reports the existing
`mclone-server` warnings for `TimingSample` and
`with_unload_hysteresis_chunks`; no new warnings were introduced by this slice.

## Slice 5: Startup Storage Projection And Validation

Status: landed 2026-07-08.

Problem: desktop, flat Android, and Android XR each repeated pieces of startup
world-storage policy: default-root eligibility, late platform default-root
injection, `--world-dir` rejection for remote sessions, and optional path
formatting for storage logs.

Change:

- Add `StartupWorldStorageProjection` in `mclone-app-runtime::startup_args` to
  project parsed storage into `world_root`/`world_dir` plus
  `default_world_root_enabled`.
- Add shared validation for `--world-dir applies only to local integrated
  worlds`.
- Add shared optional-path formatting with a caller-owned none label, preserving
  flat Android's `<none>` and Android XR's `none` log spelling.
- Route desktop CLI, flat Android, and Android XR through the projection. Flat
  Android and Android XR also validate again after legacy remote-address fallback
  so non-argv remote launches cannot combine with `--world-dir`.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --package mclone-app-runtime --package mclone-native-client --package mclone-android-client --package mclone-android-xr-client --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-xr-client
git diff --check
```

Result on 2026-07-08: passed. The wasm web check still reports the existing
`mclone-server` warnings for `TimingSample` and
`with_unload_hysteresis_chunks`, and the native-client XR feature check still
reports the existing `DESKTOP_LOCAL_ARG_FLAGS` warning; no new warnings were
introduced by this slice.

## Remaining Follow-Up Slices

1. **LocalSingleView naming cleanup.** `LocalSingleViewSceneOptions` and related
   `NativeSingleView*` names describe one logical local client/world runtime and
   chunk-view center, not one rendered eye or a flat-only presentation path.
   Shared XR and Android XR use these types today, so the name is easy to
   misread. Prefer a future mechanical rename toward names like
   `LocalIntegratedSceneOptions`, `LocalWorldRuntimeOptions`,
   `NativeLocalSessionOptions`, or `IntegratedWorldSessionOptions` once active
   storage/helper slices are settled.
2. **Android app-data asset-root helper audit.** Flat Android and Android XR
   both configure `MCLONE_ANDROID_ASSET_ROOT` from app data with similar
   logging, but flat Android currently prefers internal storage while Android XR
   prefers external storage. Decide whether that difference is intentional
   before moving the shared parts into `mclone-android-platform`.
3. **Seed reroll policy.** Desktop, XR, and flat Android all use the same LCG for
   new-world seed rerolls with separate initial-state helpers. Move this into
   shared client-experience/catalog policy.
4. **Remaining durable persistence gaps.** Tactical 160 unified catalog CRUD, not
   all Minecraft persistence. Native still lacks periodic autosave and reserved
   player/saved-data record implementations; those remain under 134/136-style
   persistence follow-up rather than this cleanup tactical.

## Guardrails

- Do not move Android activity ownership, OpenXR session/swapchain ownership, or
  desktop window/headless ownership into shared crates.
- Do not fold web's async IndexedDB catalog executor into the native synchronous
  executor.
- Every slice should reduce app-local policy duplication without hiding
  platform lifecycle effects.
