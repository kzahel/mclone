# 161: Native Catalog / Storage Adapter Dedup Follow-Up

Status: active 2026-07-08. Slices 1-3 landed; follow-up slices remain open.

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

Status: landed 2026-07-08.

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

## Remaining Follow-Up Slices

1. **LocalSingleView naming cleanup.** `LocalSingleViewSceneOptions` and related
   `NativeSingleView*` names describe one logical local client/world runtime and
   chunk-view center, not one rendered eye or a flat-only presentation path.
   Shared XR and Android XR use these types today, so the name is easy to
   misread. Prefer a future mechanical rename toward names like
   `LocalIntegratedSceneOptions`, `LocalWorldRuntimeOptions`,
   `NativeLocalSessionOptions`, or `IntegratedWorldSessionOptions` once active
   storage/helper slices are settled.
2. **Android app-data world-root helper.** Flat Android and Android XR duplicate
   `internal_data_path().or_else(external_data_path()).join("worlds")` with only
   log-label differences. Move this to Android-specific shared glue, not
   `mclone-app-runtime`.
3. **Startup storage validation and logging.** Desktop, flat Android, and
   Android XR repeat pieces of `--world-dir applies only to local integrated
   worlds`, default-root enablement, and storage logging. Keep argv/intent
   parsing platform-local, but centralize shared validation/projection.
4. **Seed reroll policy.** Desktop, XR, and flat Android all use the same LCG for
   new-world seed rerolls with separate initial-state helpers. Move this into
   shared client-experience/catalog policy.
5. **Remaining durable persistence gaps.** Tactical 160 unified catalog CRUD, not
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
