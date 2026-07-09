# 167: Shared Native Session Startup Contract

Status: Slice 4 complete 2026-07-09 (camera/interest startup reconciliation is
one shared helper in `mclone-app-runtime`, and the full clear-ready/re-pump
invariant lands where a large startup teleport is real — both XR paths and flat
Android drive `NativeSessionStartupPump::drive_to_ready_reconciled`, on-device
evidence showed a far XR view pose otherwise leaves the ready frame blank).
Slice 3 routed all native platform startup consumers through the shared
`NativeSessionStartupPump<S>` for both host modes; the divergent blocking
`poll_until_idle` + `sync_all_render_sections` startup paths are gone. Slice 2
host-neutral pump, Slice 1 local pump seed handoff, and Slice 0 audit + guard
comments landed first. Design/implementation tactical opened after the
docs/tactical/163 follow-up showed that startup render-section seeding is both
duplicated and platform-divergent.

Workstream: native Rust shared session/runtime/render startup boundary in
`mclone-app-runtime`, with desktop flat validation first and required adoption
by flat Android, XR, offscreen/perf, local integrated, and remote dedicated
native paths. This is host-mode convergence, not an app/platform feature.

## Purpose

Make native world startup one shared contract:

```text
construct NativeSessionRuntime for host mode
  -> drive shared startup pump with current camera position
  -> reconcile initial player/camera interest
  -> accumulate transient render-section seed meshes
  -> complete with { runtime, startup sections, final step }
  -> platform adapter creates draw resources from that seed
  -> normal frame streaming continues from the same runtime
```

The immediate trigger is the render-section CPU mesh eviction work in
[`163`](163-render-section-cpu-mesh-eviction.md). After that slice, the resident
render-session cache keeps only metadata. Startup paths that compiled sections
before draw-resource creation lost the transient CPU payload and some callers
worked around it by marking every resident section dirty and recompiling.

That workaround exposed a larger problem: startup is split by accident across
platforms and host modes. Desktop/XR local startup uses
`LocalIntegratedStartupPump`; Android local and all remote startup paths still
use older blocking `poll_until_idle` + `sync_all_render_sections` flows. The
goal is not a narrower XR optimization. The goal is to delete the divergent
startup contracts.

## Current Divergence

Known paths to audit and migrate:

- `native/crates/mclone-app-runtime/src/native_session_runtime.rs`
  - `LocalIntegratedStartupPump::step(...)` compiles render sections while
    polling to playable, but `LocalIntegratedStartupStep` carries counts only.
  - `compile_all_render_section_meshes(...)` marks every resident section dirty
    and recompiles to reconstruct a transient full batch.
- `native/crates/mclone-xr-scene/src/session.rs`
  - XR local startup drives `LocalIntegratedStartupPump`, then calls
    `compile_all_render_section_meshes(...)` in `complete_local_startup`.
  - XR remote/replacement startup uses `start_xr_terrain_runtime(...)`, which
    blocks on `poll_until_idle`, commits pose, then `sync_all_render_sections`.
- `native/apps/mclone-native-client/src/flat_client_driver.rs`
  - Desktop non-blocking local startup uses the pump, then
    `upload_cached_runtime_sections(...)` recompiles through
    `cached_runtime_sections(...)`.
  - Desktop remote/start-from-scene uses the older blocking
    `poll_window_runtime_until_idle(...)` + `upload_all_runtime_sections(...)`
    path.
- `native/apps/mclone-android-client/src/lib.rs`
  - `start_android_render_scene(...)` handles local and remote together with
    blocking `poll_until_idle`, optional pose poll, then
    `sync_all_render_sections`.
- `native/apps/mclone-native-client/src/perf.rs`
  - Startup-streaming perf drives the desktop startup pump, then recompiles
    with `compile_all_render_section_meshes(...)`.

Android local is not intentionally different from desktop local. Remote startup
is not intentionally different from local startup. The only real axis is host
mode: local integrated has loading-progress/playable diagnostics, while remote
dedicated has transport/update readiness facts. Both can feed the same startup
pump and render seed.

## Target Contract

Introduce a host-neutral startup pump over `NativeSessionRuntime<S>`:

```rust
pub struct NativeSessionStartupPump<S> {
    runtime: NativeSessionRuntime<S>,
    render_seed: StartupRenderSectionSeed,
    readiness: StartupReadinessPolicy,
    // timing/progress counters
}

pub struct NativeSessionStartupStep {
    pub host_mode: SingleViewHostMode,
    pub poll_count: usize,
    pub poll_ms: f64,
    pub changed: bool,
    pub startup_ready: bool,
    pub render_seed_section_count: usize,
    pub render_seed_drawable_section_count: usize,
    pub cached_section_count: usize,
    pub submitted_compile_section_count: usize,
    pub accepted_compile_result_count: usize,
    pub completed_compile_section_count: usize,
    pub pending_compile_jobs: usize,
    pub local_progress: Option<LoadingProgressOverlay>,
    // local-only LOD prewarm fields may stay optional/zeroed for remote
}

pub struct NativeSessionStartupCompletion<S> {
    pub runtime: NativeSessionRuntime<S>,
    pub startup_sections: Vec<TexturedRenderSectionMesh>,
    pub final_step: NativeSessionStartupStep,
}
```

Names are illustrative; the ownership shape is the contract.

### Startup Render Seed

`StartupRenderSectionSeed` is a short-lived keyed accumulator:

```text
map RenderSectionKey -> latest TexturedRenderSectionMesh
observe(RenderSectionCacheUpdate):
  remove every removed_section_key
  insert every rebuilt section, latest wins
drain_sections() -> Vec<TexturedRenderSectionMesh>
```

Rules:

- Do not use `RenderSectionCacheUpdate::merge` for this. It appends rebuilds and
  unions removals, which is correct for counters but wrong for reconstructing
  the final resident upload batch.
- Rebuild after removal wins; removal after rebuild removes. If one update ever
  carries the same key in both sets, removals are applied before rebuilds so a
  freshly accepted rebuild remains drawable.
- The seed may temporarily retain CPU mesh payloads only for the startup window.
  It is consumed at completion and must not become a resident cache.
- Compile-job release stays in the startup pump after accepting compile results.
  Do not accidentally hold render compile capacity until platform draw upload.

### Readiness

Use one `StartupReadinessPolicy::Playable` by default for every native startup
lane. Do not choose different thresholds because the caller is desktop, Android,
or XR.

Host modes provide different evidence:

- Local integrated readiness starts from today's honest playable gate:
  spawn-authority chunks ready, client snapshot for playable chunk present,
  render seed has at least one drawable section, and startup LOD prewarm is
  settled when enabled.
- Remote dedicated readiness should be equivalent or stricter than today's
  visible behavior at first: the active chunk view has produced client chunks,
  the startup seed has at least one drawable section, and the initial
  response/update backlog for the current response-paired protocol has drained.
  This is a host-mode fact, not a platform fact. When server-push broadening
  lands, remote readiness should not wait for "the stream is forever idle";
  it should wait for a drawable active view.

`StartupReadinessPolicy::Idle` may exist for diagnostics, screenshots, or
explicit tests, but it must be a named caller option. It must not be the hidden
reason Android or remote startup behaves differently.

### Camera And Interest Reconciliation

The shared startup pump owns runtime readiness and render-section seed meshes.
Platform adapters still own physical camera setup: flat spectator placement,
touch startup options, XR startup view pose, and OpenXR tracking origin.

Guardrail: startup is not complete until the final player/camera pose and
interest center have been reconciled. If accepting a server player-position
correction or applying a startup view pose changes chunk interest, the driver
must continue the shared pump at the new camera position before completion.

Do not complete startup by:

1. pumping to ready at camera A,
2. changing interest/camera to B,
3. seeding draw resources from A as if B were ready.

The seed can still contain sections from A; normal streaming/removals will fix
that. The readiness decision and traversal-ready publication must be based on
the final startup camera position.

## Non-Goals

- Do not reintroduce resident CPU mesh payloads in `CachedTexturedRenderSections`.
- Do not move window, Android activity, browser canvas, OpenXR session, or TCP
  socket ownership into `mclone-app-runtime`.
- Do not make startup draw resources "empty and hope streaming fills them" after
  the pump has already accepted and cleaned resident metadata. Without a seed or
  a dirty-all recompile, clean resident sections may never be re-emitted.
- Do not broaden the remote wire protocol or implement server-push here. That
  remains owned by [`133`](133-session-network-bus-and-update-pacing.md) and
  [`151`](151-remote-inbound-update-pipeline.md).

## Implementation Slices

### Slice 0: Startup Audit And Tripwire Baseline

No behavior change.

Deliverables:

- Record every native startup path that calls any of:
  - `LocalIntegratedStartupPump`
  - `poll_until_idle`
  - `sync_all_render_sections`
  - `compile_all_render_section_meshes`
  - `start_xr_terrain_runtime`
  - `start_android_render_scene`
  - `upload_cached_runtime_sections`
- Classify each hit as startup, resource rebuild, headless/probe, test, or live
  frame streaming.
- Add comments at `compile_all_render_section_meshes(...)` making it explicit
  that startup callers are not allowed to use it after this tactical; it is for
  renderer/surface resource rebuilds.
- Add focused unit tests for a new `StartupRenderSectionSeed` if introduced in
  this slice, or draft the tests in place if the type lands in Slice 1.

Tripwire validation:

```bash
rg -n "compile_all_render_section_meshes|sync_all_render_sections\\(|poll_until_idle\\(|LocalIntegratedStartupPump|start_xr_terrain_runtime|start_android_render_scene|upload_cached_runtime_sections" native/apps native/crates
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
git diff --check
```

Exit criteria: the old startup call graph is documented before it is changed,
and every post-slice migration can prove it removed a classified startup hit
without touching resource-rebuild/probe uses.

#### Slice 0 Result (audit, 2026-07-09)

Tripwire grep + `cargo test -p mclone-app-runtime` (175 passed) baseline is
green; `git diff --check` clean. Every current hit of the tracked startup
primitives, classified:

| Site | Primitive | Class | Notes |
| --- | --- | --- | --- |
| `mclone-app-runtime/native_session_runtime.rs:505,512` | `LocalIntegratedStartupPump` (def) | shared def | Local-only startup pump; Slice 2 generalizes to `NativeSessionStartupPump<S>`. |
| `…native_session_runtime.rs:989,1129,1156` | `poll_until_idle`/`sync_all`/`compile_all` (LocalIntegrated def) | shared def | `compile_all` now carries the 167 resource-rebuild-only guard comment. |
| `…native_session_runtime.rs:1504-1514,1630,1663` | enum-dispatch defs | shared def | `NativeSceneRuntime<S>` fan-out; `compile_all` guarded. |
| `…native_session_runtime.rs:2352,2488,2515` | RemoteDedicated core defs | shared def | `compile_all` guarded. |
| `…native_session_runtime.rs:2831-3043,3229-3420` | `poll_until_idle`/pump ctor | test | app-runtime unit tests. |
| `mclone-native-client/scene_runtime.rs:175,192` | `LocalIntegratedStartupPump` field/ctor | **startup** | Desktop `WindowSceneStartupPump`; migrate in Slice 1/3. |
| `…scene_runtime.rs:456,482` | `sync_all`/`compile_all` wrappers | shared def | Pass-through; `compile_all` guarded. |
| `…scene_runtime.rs:596` | `poll_until_idle` (`poll_window_runtime_until_idle`) | **startup** (blocking) | Desktop remote/probe blocking helper. |
| `mclone-native-client/flat_client_driver.rs:1819,2290,2425` | `upload_cached_runtime_sections`→`compile_all` | **startup** | Desktop non-blocking local playable seed; retire in Slice 1/3. |
| `…flat_client_driver.rs:1721,1747,2271` | `poll_window_runtime_until_idle`+`sync_all_runtime_sections` | **startup** (blocking) + resource-rebuild | `start_world_from_scene` remote/start-from-scene path; `sync_all` at 2271 is dual-use (also surface rebuild). |
| `mclone-native-client/scene_runtime/tests.rs:231-979` | `sync_all`/`poll_until_idle` | test | Desktop scene-runtime tests. |
| `mclone-native-client/headless.rs:805,966` | `sync_all` (cold seed) | headless/probe | Dual-view + render-resource-rebuild probe seeds. |
| `mclone-native-client/perf.rs:3349,3479,4301` | `sync_all` | perf probe | Per-step / frame-budget benchmark remesh. |
| `mclone-native-client/perf.rs:3828` | `compile_all` | **startup** | Startup-streaming perf seed; migrate in Slice 1. |
| `mclone-xr-scene/session.rs:27,128,550` | `LocalIntegratedStartupPump` field/ctor | **startup** | XR local pump; migrate in Slice 1/3. |
| `…xr-scene/session.rs:689` | `compile_all` (`complete_local_startup`) | **startup** | XR local seed; migrate in Slice 1. |
| `…xr-scene/session.rs:243,868,1311,1330,1352,1358` | `start_xr_terrain_runtime`→`poll_until_idle`+`sync_all` | **startup** (blocking) | XR remote/replacement; converge in Slice 3. |
| `mclone-android-client/lib.rs:606,1214,2133,2148,2157,2166` | `start_android_render_scene`→`poll_until_idle`+`sync_all` | **startup** (blocking) | Android local+remote combined; converge in Slice 3. |

Guard comments added this slice (no behavior change) at
`compile_all_render_section_meshes(...)` in
`mclone-app-runtime/native_session_runtime.rs` (LocalIntegrated def, enum
dispatch, RemoteDedicated def) and the `mclone-native-client/scene_runtime.rs`
wrapper, marking each as a resource-rebuild-only path forbidden to startup
callers. `StartupRenderSectionSeed` does not exist yet; its unit tests are
drafted against Slice 1 (see Slice 1 tripwires) rather than added here to keep
Slice 0 behavior-free.

Startup hits to retire by later slices: desktop local
(`flat_client_driver` `upload_cached_runtime_sections`/`compile_all`), desktop
remote (`start_world_from_scene` blocking path), XR local (`session.rs:689`),
XR remote (`start_xr_terrain_runtime`), Android local+remote
(`start_android_render_scene`), and startup-streaming perf (`perf.rs:3828`).
Resource-rebuild/probe/test hits (`headless.rs`, `perf.rs:3349/3479/4301`,
`scene_runtime/tests.rs`, `flat_client_driver.rs:2271` surface rebuild) stay.

### Slice 1: Local Pump Seed Handoff

Goal: fix the 163 redundant CPU remesh for existing pump users without yet
generalizing remote startup.

Deliverables:

- Add `StartupRenderSectionSeed` in `mclone-app-runtime`.
- Teach `LocalIntegratedStartupPump::step(...)` to consume each transient
  `RenderSectionCacheUpdate` into the seed before dropping the update.
- Add a consuming handoff such as:

  ```rust
  into_runtime_with_startup_sections(self)
      -> (LocalIntegratedSceneRuntime, Vec<TexturedRenderSectionMesh>)
  ```

  or a completion struct that carries the final `LocalIntegratedStartupStep`.
- Migrate XR local startup, desktop flat non-blocking local startup, offscreen
  playable startup, and startup-streaming perf away from
  `compile_all_render_section_meshes(...)`.
- Keep `into_runtime()` for callers that do not need a draw seed.

Tripwires:

- Unit tests: latest rebuild wins, removal deletes, removal+rebuild same update
  leaves the rebuild, no duplicate keys, drawable count reflects non-empty
  meshes.
- App-runtime startup tests assert that `playable_ready` implies a non-empty
  startup seed with at least one drawable section.
- Static grep: no local-startup completion path may call
  `compile_all_render_section_meshes(...)`.
- Resident CPU mesh accounting remains zero.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --package mclone-app-runtime --package mclone-native-client --package mclone-xr-scene --check
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml --workspace
```

Pixel/device validation:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --screenshot /tmp/mclone-startup-local.png --startup-wait playable
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke
```

Log tripwire: startup logs should report `sections=N` from the seed, with no
startup recompile path. The Quest smoke line to watch remains
`XR local world playable ... sections=N`.

#### Slice 1 Result (2026-07-09)

- Added `StartupRenderSectionSeed` in `mclone-app-runtime`
  (`startup_render_seed.rs`): a `BTreeMap<RenderSectionKey,
  TexturedRenderSectionMesh>` accumulator with `observe(&update)` (removals
  before rebuilds so a same-update rebuild stays drawable), `section_count`,
  `drawable_section_count`, `estimated_owned_bytes`, `is_empty`, and
  `drain_sections`. Five unit tests cover latest-rebuild-wins, removal deletes,
  removal+rebuild-same-update keeps the rebuild, no duplicate keys, drawable
  count, and drain-empties-to-zero.
- `LocalIntegratedStartupPump` now owns a `render_seed`, folds every transient
  `RenderSectionCacheUpdate` into it during `step(...)` (compile-job release
  stays in the pump, unchanged), surfaces `render_seed_section_count` /
  `render_seed_drawable_section_count` on `LocalIntegratedStartupStep`, and adds
  `into_runtime_with_startup_sections()` alongside the retained `into_runtime()`.
- Migrated all local seeders off `compile_all_render_section_meshes(...)`:
  desktop flat (`flat_client_driver` `complete_local_world_startup` →
  `upload_startup_seed_sections`; `cached_runtime_sections` deleted), offscreen
  playable startup (shares that driver path), XR local (`session.rs`
  `complete_local_startup`), and startup-streaming perf. The now-dead desktop
  `WindowSceneStartupPump::into_runtime` and
  `WindowSceneRuntime::compile_all_render_section_meshes` wrappers were removed.
- Tripwires green: app-runtime 180 tests (175 + 5 seed), render-session 103,
  native-client 179, xr-scene 74; workspace + `wasm32` web-client `cargo check`
  clean; local startup screenshot at `--startup-wait playable` drew terrain
  (`6 sections, 2 drawn sections`) from the seed with no recompile path.
- Deferred to later slices: XR remote (`start_xr_terrain_runtime`) and Android
  local+remote (`start_android_render_scene`) still use the blocking
  `poll_until_idle` + `sync_all_render_sections` path (Slice 3, once the
  host-neutral pump lands in Slice 2). The XR-local seed still reflects the
  pump's startup camera; interest drift from a startup pose correction is
  reconciled by normal streaming until Slice 4 makes reconciliation shared. The
  app-runtime `compile_all_render_section_meshes(...)` methods remain as the
  documented resource-rebuild path (currently callerless; renamed in Slice 5).

### Slice 2: Host-Neutral Startup Pump

Goal: replace local-only startup ownership with a pump over
`NativeSessionRuntime<S>`.

Deliverables:

- Introduce `NativeSessionStartupPump<S>` and
  `NativeSessionStartupCompletion<S>`.
- Move the seed accumulator and common poll/sync/release logic into the
  host-neutral pump.
- Express local playable readiness and remote playable readiness behind one
  readiness function on `NativeSessionRuntime<S>` or a small host-mode strategy.
- Preserve local loading-progress overlay fields as optional data on
  `NativeSessionStartupStep`; remote returns `None`.
- Keep local startup LOD prewarm as a policy on the shared pump, active only
  when the runtime is local integrated.
- Rebuild the old `LocalIntegratedStartupPump` as a thin compatibility wrapper
  or type alias only if that makes migration safer. Do not let it keep separate
  behavior.

Tripwires:

- Tests cover both `NativeSessionRuntime::Local` and `RemoteDedicated` with
  scripted sessions.
- A remote scripted-session test proves startup can reach ready through the same
  pump and seed contract, not through `poll_until_idle` + `sync_all`.
- A local high-render-distance test still reaches playable before the full view
  settles.
- Compile-job release counts are unchanged: accepted compile jobs do not remain
  held by the seed after `step(...)`.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-xr-scene -p mclone-android-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
```

#### Slice 2 Result (2026-07-09)

- Introduced the host-neutral `NativeSessionStartupPump<S>` in
  `mclone-app-runtime/native_session_runtime.rs`. It owns a
  `NativeSessionRuntime<S>`, the transient `StartupRenderSectionSeed`, a
  `StartupReadinessPolicy` (default `Playable`), the poll/sync/compile-job
  release loop, and startup LOD prewarm. `step(...)` returns a host-neutral
  `NativeSessionStartupStep` (`host_mode`, `startup_ready`, `host_ready`,
  seed/compile counters, `local_progress: Option<LoadingProgressOverlay>`, and
  local-only prewarm fields zeroed for remote). `complete(self)` drains the seed
  into `NativeSessionStartupCompletion<S> { runtime, startup_sections,
  final_step }`. Constructors: `local`/`local_with_mesh_assets` (on
  `NativeSessionStartupPump<LocalOnlySession>`) and
  `remote_dedicated`/`remote_dedicated_with_mesh_assets`, plus `with_readiness`.
- Expressed readiness once behind `StartupReadinessPolicy` with host-mode
  evidence supplied by `NativeSceneRuntime::startup_host_ready(policy, camera)`:
  local uses `LocalIntegratedSceneRuntime::startup_spawn_authority_ready`
  (playable chunk server-ready + client snapshot present), remote uses
  `RemoteDedicatedSceneRuntime::startup_host_ready` (active view produced client
  chunks + response/update backlog drained). The shared gate is
  `host_ready && render seed has ≥1 drawable section && prewarm settled`. `Idle`
  additionally waits for no pending render work at the final camera; no lane
  forks thresholds by platform. Local loading-progress stays optional step data;
  remote returns `None`.
- Startup LOD prewarm is a pump policy active only when the runtime is local
  integrated (remote carries a disabled, always-settled prewarm). Compile-job
  release stays in the pump after accepting results; the seed never holds compile
  capacity after `step`.
- Rebuilt `LocalIntegratedStartupPump` as a thin wrapper over
  `NativeSessionStartupPump<LocalOnlySession>` (keeps `with_mesh_assets`, `step`
  → `LocalIntegratedStartupStep`, `progress_overlay`, `runtime`, `into_runtime`,
  `into_runtime_with_startup_sections`). It holds no separate startup behavior;
  desktop/XR/perf local consumers are unchanged and migrate to the shared pump in
  Slice 3. The old `into_runtime()`/`playable_ready()` local-only helpers folded
  into the wrapper mapping.
- Tripwires green: app-runtime 183 tests (180 + 3 new — local reaches playable
  through the shared seed; a scripted `RemoteDedicated` session reaches ready
  through the shared pump+seed and proves it does **not** `poll_until_idle` (only
  the construction-time response is blocking-drained, `blocking_drain_count == 1`,
  backlog drained); readiness policy defaults to `Playable`). The existing
  high-render-distance local test still reaches playable before the full view
  settles (now via the wrapper over the shared pump). The local completion test
  asserts compile capacity is fully available after `complete()`, proving
  accepted jobs are released, not held by the seed. render-session 103 green;
  `mclone-native-client`/`mclone-xr-scene`/`mclone-android-client` and
  `wasm32` `mclone-web-client` `cargo check` clean; `git diff --check` clean;
  local `--startup-wait playable` screenshot drew terrain (`6 sections, 2 drawn`)
  from the seeded draw resources.
- Deferred to Slice 3: desktop remote (`start_world_from_scene` blocking path),
  flat Android (`start_android_render_scene`), and XR remote
  (`start_xr_terrain_runtime`) still use the blocking
  `poll_until_idle` + `sync_all_render_sections` sequence; migrating them onto the
  shared pump/completion is Slice 3. Shared camera/interest reconciliation stays
  Slice 4; `compile_all_render_section_meshes(...)` remains the documented
  (callerless) resource-rebuild path, renamed in Slice 5.

### Slice 3: Migrate Native Platform Startup Consumers

Goal: desktop flat, flat Android, XR, offscreen, and perf all consume the same
startup completion object for local and remote host modes.

Deliverables:

- Desktop flat:
  - route local and remote session start through `NativeSessionStartupPump`;
  - remove `cached_runtime_sections(...)` and `upload_cached_runtime_sections(...)`
    if they only served startup seeding;
  - keep full sync only for explicit resource rebuild/probe paths.
- XR:
  - replace `complete_local_startup(...)`'s startup recompile with the shared
    completion seed;
  - route `start_xr_terrain_runtime(...)` remote/replacement startup through the
    same pump or delete it in favor of a shared helper.
- Flat Android:
  - replace `start_android_render_scene(...)`'s local/remote
    `poll_until_idle` + `sync_all_render_sections` sequence with the shared
    pump driven synchronously during initialization;
  - use the same playable readiness policy as desktop/XR, not an Android-only
    idle wait.
- Startup-streaming perf and offscreen playable startup consume the same
  completion seed as the app paths.

Tripwires:

- Static grep: app startup code no longer calls
  `compile_all_render_section_meshes(...)` or `sync_all_render_sections(...)`.
  Remaining hits must be tests, headless/probes, live full-sync/rebuild paths,
  or explicitly documented exceptions.
- Native feature/platform parity tests should not need new per-platform startup
  exceptions.
- Android local and remote startup logs use the same startup-ready wording and
  seed section counters as desktop/XR.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
```

Rendered/device validation:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --screenshot /tmp/mclone-startup-flat-local.png --startup-wait playable
# Start a local dedicated server, then run a desktop remote screenshot/smoke.
# Use the repo's current remote smoke helper if one exists in platforms.md.
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-session-smoke
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke
```

Quest/Android XR remote validation should be batched with the next headset pass:
local new-world and remote join must both report nonzero startup seed sections
and draw terrain.

#### Slice 3 Result (2026-07-09)

Shared-pump additions in `mclone-app-runtime`
(`native_session_runtime.rs`):

- `NativeSessionStartupPump::local`/`local_with_mesh_assets` moved from the
  `LocalOnlySession`-only impl to the generic `impl<S>`, so desktop
  (`RemoteServerSession`), XR, and Android can build one pump type for both host
  modes; local-only callers infer `S = LocalOnlySession`.
- `from_runtime(NativeSessionRuntime<S>)` wraps an already-constructed runtime
  (prewarm disabled, `Playable`) for lanes that build the runtime up front.
- `drive_to_ready_with_timeout(camera, timeout)` / `drive_to_ready(camera)` are
  the shared synchronous blocking drive: step to `startup_ready` under the active
  policy, tiny-sleep when unchanged, then `complete()`. No lane calls
  `poll_until_idle` for startup anymore.

Per-consumer migration:

- **Desktop flat** (`flat_client_driver.rs`, `scene_runtime.rs`): `WindowSceneStartupPump`
  now wraps `NativeSessionStartupPump<RemoteServerSession>` and builds local
  *or* remote pumps (`from_scene` keeps the loading-screen spawn-center anchor;
  `from_scene_at_scene_center` matches the pre-tactical blocking `make_runtime`
  scene center for tests / offscreen idle / remote join). `start_world_from_scene`
  replaced its `poll_window_runtime_until_idle` + `upload_all_runtime_sections`
  path with a shared blocking drive at the runtime interest center + a
  `upload_startup_seed_sections` seed upload, and takes a `StartupReadinessPolicy`
  (`Playable` for real starts, `Idle` for screenshots). `finish_pending_session_start`
  dropped its `make_runtime` arg and drives both branches from one pump maker.
  The startup step is now the host-neutral `NativeSessionStartupStep`
  (`startup_ready`, `local_progress`). `upload_all_runtime_sections` /
  `sync_all_runtime_sections` stay only for the surface/resource-rebuild path.
- **XR** (`mclone-xr-scene/session.rs`): `start_xr_terrain_runtime` (remote and
  replacement) now `NativeSessionStartupPump::from_runtime(runtime).drive_to_ready(...)`,
  seeds draw resources from the completion seed, then commits the startup
  pose/view-pose after completion (unchanged reconciliation ordering). XR local
  `complete_local_startup` already consumed the seed (Slice 1) via the wrapper.
- **Flat Android** (`mclone-android-client/lib.rs`): `start_android_render_scene`
  replaced local+remote `poll_until_idle` + optional pose-poll +
  `sync_all_render_sections` with the same `from_runtime`/`drive_to_ready`
  (`Playable`) drive and seed, using the same startup-ready log wording and seed
  section counters as desktop/XR; no Android-only idle wait remains.
- **Perf / offscreen**: startup-streaming perf reads `step.startup_ready` /
  `step.local_progress` and completes via the shared `complete()` seed; offscreen
  playable/idle route through the same driver paths (idle → blocking pump with
  `Idle`).

Tripwires: `cargo test` — app-runtime 183, native-client 179, xr-scene 74
(unchanged from Slice 2). Workspace `cargo check`, `wasm32` `mclone-web-client`
`cargo check`, and `cargo ndk -t arm64-v8a --platform 28 check -p
mclone-android-client` all clean; `git diff --check` clean; `cargo fmt --all
--check` flags only pre-existing files outside this slice's diff (repo `main` is
not fmt-clean under the local toolchain — added lines verified clean). Static
grep: no app startup path calls `compile_all_render_section_meshes(...)` or
`sync_all_render_sections(...)`; remaining hits are app-runtime shared defs, the
desktop surface-rebuild + perf-probe `sync_all`, headless/perf
`poll_window_runtime_until_idle`, tests, and the `start_*_terrain_runtime` /
`start_android_render_scene` entry functions that now drive the shared pump.
Android local and remote both log `Mclone Android <host> runtime playable ...
sections=N drawable_sections=N`, matching the desktop/XR seed counters.

Rendered validation (desktop, this host): local new-world
`--startup-wait playable` drew terrain (6 sections, 2 drawn); local blocking
`--startup-wait idle` drew the full view (64 sections, 11 drawn); remote join
against a local dedicated server drew terrain at both `Playable` (27 sections, 9
drawn — the intended earlier drawable-active-view) and `Idle` (664 sections, 93
drawn). All four are non-blank terrain.

Device smokes: the Quest 3 XR `native:android-xr:session-smoke` ran on-device
this pass and passed. XR local new-world reached `XR local world playable
seed=12345 polls=78 sections=6 target_ready=9/9`, and the replacement session —
which drives the migrated `start_xr_terrain_runtime` through the shared pump —
reached `MCLONE_ANDROID_XR_REPLACEMENT_READY new-world seed=246813579 sections=19
drawn_sections=16 drawn_indices=124530` (nonzero seed sections, terrain drawn).
The flat Android AVD `native:android:avd-session-smoke` is deferred to the next
Android pass (the available AVDs are unrelated x86 images; the connected physical
device is the Quest 3, not a flat-Android phone). The flat Android code path is
validated by `cargo ndk check` and shares the exact `from_runtime` /
`drive_to_ready` contract exercised on-device by the XR lane and on desktop by
both host modes.

### Slice 4: Shared Camera/Interest Startup Reconciliation

Goal: avoid leaving camera/interest startup logic as the next platform fork.

Deliverables:

- Extract the common pieces of:
  - desktop `WindowSceneRuntime::commit_engine_camera_player_pose`,
  - Android `commit_engine_camera_player_pose`,
  - XR `commit_engine_camera_player_pose_for_runtime`,
  into a shared helper in `mclone-app-runtime` if crate boundaries allow it, or
  into a small shared adapter module with an explicit reason if not.
- Keep physical startup view-pose inputs platform-local:
  - flat spectator placement,
  - Android touch/startup camera options,
  - XR `XrStartupViewPose` and tracking-origin policy.
- Ensure any interest-center change during startup clears the ready flag and
  continues the shared pump at the new camera position before completing.

Tripwires:

- A test or scripted smoke applies a startup pose correction that changes chunk
  interest and proves completion still has nonzero seed sections for the final
  camera position.
- No platform path completes startup immediately after `set_interest_center`
  without another pump/sync pass.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client
```

#### Slice 4 Result (2026-07-09)

Consolidation — one shared `mclone-app-runtime::camera_reconcile` module now owns
the server pose-sync + position-correction acceptance + interest-follow that were
copied three ways (desktop `WindowSceneRuntime`, Android free functions, XR
`*_for_runtime`): `commit_engine_camera_player_pose`,
`sync_engine_camera_player_pose`, `apply_pending_engine_camera_position_updates`,
and `update_interest_from_engine_camera`, all generic over
`&mut NativeSceneRuntime<S>` + `EngineCameraController`. The one genuinely
divergent axis is carried by `EngineCameraCommitContext { lane, pose_sync_policy }`
— desktop/XR pose-sync is `SendOnly`, Android is `DrainImmediately` (an explicit
caller policy, not a hidden platform fork), and `lane` (`"desktop"`/`"Android"`/
`"XR terrain"`) drives the unified interest/correction log wording. Physical
view-pose inputs stay platform-local: flat spectator placement, Android
touch/startup camera options, and the XR `XrStartupViewPose` + tracking-origin
policy. The crate-boundary Open Question resolved in favor of
`mclone-app-runtime` (it already depends on `mclone-render-session`, which owns
`EngineCameraController`), so no narrower module was needed. The XR per-frame
`commit_engine_camera_player_pose_for_runtime_timed` chain stays in `mclone-xr-scene`
as an intentional profiling overlay (it interleaves `XrCameraCommitTiming` between
sub-steps); the now-unused non-timed `commit_engine_camera_player_pose_for_runtime`
was deleted.

Full re-pump invariant — **built, not descoped.** The prompt asked to first check
whether a large startup interest jump actually occurs before building the re-pump
machinery. It does: the standard Quest `native:android-xr:session-smoke` sets
`debug.mclone.xr_view_pose=0,120,-96,180`, i.e. a startup view pose ~6 chunks
(`z=-96`) from the loaded spawn center. Before this slice that shipped a seed built
at the spawn center and the ready frame drew nothing at the final camera
(`sections=6 drawn_sections=0 drawn_indices=0`). After it, the pump re-pumps at the
teleported camera and the same frame draws terrain (`sections=123 drawn_sections=32
drawn_indices=265422`). Remote desktop spawn corrections stay small (Slice 3), but
the XR view pose is a supported, unbounded, in-the-smoke input, so the defensive
guard is a real fix here.

Mechanism (`mclone-app-runtime`): `NativeSessionStartupPump::drive_to_ready_reconciled(
initial_camera, timeout, reconcile)` drives to base readiness, runs a platform
`reconcile` closure that applies the physical pose against the still-pump-owned
runtime and returns the resulting camera eye, and — when that moved chunk interest
— keeps pumping at the new camera until the render seed has drawable coverage near
it (`StartupRenderSectionSeed::drawable_section_count_near`, Chebyshev radius =
render distance), bounded by `MAX_STARTUP_RECONCILE_PASSES` for a correction
cascade and by the deadline. A coverage-only timeout completes with the best-effort
seed (a reconciled view over void has no drawable sections and must not hard-fail);
a base-readiness timeout still errors. `drive_to_ready_with_timeout` was refactored
onto the shared `drive_until_ready` step loop, and `LocalIntegratedStartupPump`
grew a matching `drive_to_ready_reconciled` for the XR-local finalize.

Wiring — the reconciled drive is used everywhere a far startup teleport is real and
the pose application is self-contained: XR remote/replacement (`start_xr_terrain_runtime`),
XR local finalize (`complete_local_startup`), and flat Android (`start_android_render_scene`),
all via the shared `reconcile_xr_startup_pose` / `commit_engine_camera_player_pose`
closures. Desktop (`start_world_from_scene`, `complete_local_world_startup`) keeps
the shared *post-completion* reconcile helper: its startup pose is spectator
placement that preserves the chunk column (no interest change) plus small remote
spawn corrections observed within a chunk in Slice 3, and its spectator placement
is entangled with the assembled `self.runtime`/`self.camera` (needed to probe the
surface column), which makes an inline reconciled drive awkward. This is the
recorded per-lane exception the Open Question anticipates; the shared
`drive_to_ready_reconciled` mechanism is ready to adopt if a far desktop remote
spawn is later observed.

Tripwires — both satisfied. (1) `reconciled_startup_repumps_seed_to_the_final_teleported_camera`
(app-runtime) drives to ready at spawn chunk (0,0), reconciles a teleport to (3,0)
(three chunks out, beyond render distance so the spawn seed cannot cover it), and
asserts the completion seed carries drawable sections within render distance of the
final chunk — proving the re-pump, not the spawn seed, feeds draw resources.
`drawable_count_near_filters_by_chunk_chebyshev_radius` covers the seed query.
(2) No platform re-pump path completes immediately after interest moves: the
reconciled loop only exits once base readiness *and* near-camera coverage hold (or
the deadline), and desktop/Android with no interest move complete with a single
reconcile pass, as before.

Tests — app-runtime 185 (183 + reconciled re-pump + seed proximity), render-session
103, native-client 179, xr-scene 74; workspace + `wasm32` web-client `cargo check`
clean; `cargo ndk -t arm64-v8a --platform 28 check -p mclone-android-client` clean;
`cargo fmt --all --check` flags only pre-existing files outside this diff; `git diff
--check` clean.

Rendered/device — desktop local and remote `--startup-wait playable` both drew
terrain (byte-identical to the Slice 3 captures; desktop path unchanged). The Quest 3
`native:android-xr:session-smoke` ran on-device this pass: the far-view-pose new-world
session reached `XR local world playable ... sections=112` and the ready summary drew
terrain (`drawn_sections=32 drawn_indices=265422`), up from `drawn_sections=0` before
the re-pump; the replacement session (interest moved to (1,1)) also drew. The flat
Android AVD session smoke is deferred to the next Android pass (the connected device
is the Quest 3, not a flat-Android phone; available AVDs are unrelated x86 images) —
the flat Android startup path is covered by `cargo ndk check` and shares the exact
`drive_to_ready_reconciled` contract exercised on-device by the XR lane.

### Slice 5: Cleanup, Naming, And Enforcement

Goal: make the new startup contract hard to bypass.

Deliverables:

- Rename or document `compile_all_render_section_meshes(...)` so the name makes
  its intended use explicit, for example
  `recompile_all_render_section_meshes_for_resource_rebuild(...)`.
- Delete compatibility wrappers that survived only for slice safety.
- Add a narrow enforcement test or CI grep script for forbidden startup calls if
  the repo has an existing place for such checks.
- Update `docs/platforms.md` validation notes if startup smoke commands/log
  lines change.
- Update [`163`](163-render-section-cpu-mesh-eviction.md) with the follow-up
  result once the redundant startup remesh is gone.

Tripwires:

- `resident_mesh_owned_bytes` remains zero in resident-cache diagnostics.
- Startup seed owned bytes are observable as transient startup pressure if a
  diagnostic surface is touched; they must drop to zero after completion.
- Renderer-resource rebuild smokes still pass, proving the renamed dirty-all
  recompile path remains available for real rebuilds.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-mesh
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
```

Rendered/device validation:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --screenshot /tmp/mclone-startup-final.png --startup-wait playable
cargo run --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --renderer-rebuild-smoke
MCLONE_ANDROID_ABIS=arm64-v8a pnpm native:android:avd-session-smoke
MCLONE_ANDROID_XR_WAIT_SECONDS=60 pnpm native:android-xr:session-smoke
```

## Implementation Guardrails

- Shared owner first: startup readiness, render seed accumulation, render sync,
  compile-job release, and startup diagnostics live in `mclone-app-runtime`.
- Platform adapters may own only lifecycle glue, physical camera/view-pose
  setup, surface/swapchain/draw-resource creation, and device validation.
- Do not split by platform. If behavior differs, it must be either:
  - host-mode evidence (`LocalIntegrated` vs `RemoteDedicated`), or
  - an explicit caller policy (`Playable` vs `Idle`) with a test name.
- Do not use empty draw resources as a substitute for the seed after a startup
  pump accepted resident metadata. Empty draw resources are fine before any
  section is accepted, but the completion seed must carry already-accepted
  startup meshes forward.
- Do not add an upload coordinator to the startup pump unless release ownership
  is redesigned at the same time. The first implementation should keep compile
  release in the pump and hand platform draw upload a plain seed vector.
- Keep XR multiview-safe by using existing terrain draw resources and normal
  traversal-ready publication. This tactical should not add a new XR-visible
  renderer path.
- When touching startup code that produces pixels, capture and inspect a
  screenshot or headset/AVD smoke before calling the slice done.

## Open Questions

- Remote readiness should initially match today's blocking behavior closely
  enough to avoid regressions, but the target is "drawable active view," not
  "remote stream idle forever." Revisit this when server-push/protocol work in
  tactical 133 broadens remote updates.
- Web currently creates draw resources empty and streams per frame. That is not
  the native double-compile wart, but the target contract is still useful for
  web if browser startup grows a blocking/playable phase. Do not force a web
  migration in the first native slices unless it removes duplication cleanly.
- The exact home for shared camera/interest reconciliation depends on crate
  boundaries around `EngineCameraController`. If moving it into
  `mclone-app-runtime` creates awkward dependencies, record the reason and keep
  the helper in the narrowest shared crate/module that can serve desktop,
  Android, and XR.

## Relationship To Other Tacticals

| Doc | Relationship |
| --- | --- |
| [`163-render-section-cpu-mesh-eviction.md`](163-render-section-cpu-mesh-eviction.md) | Source of the immediate redundant CPU remesh follow-up. This tactical completes the startup handoff cleanup without reverting resident CPU mesh eviction. |
| [`101-create-world-chunk-progress-screen.md`](101-create-world-chunk-progress-screen.md) | Existing local startup UI/pump work. This tactical generalizes the pump contract beyond local and beyond desktop/XR. |
| [`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md) | Remote normal-frame ingress is already shared. This tactical applies the same convergence pressure to startup/bootstrap. |
| [`154-client-ingress-adapter-cleanup.md`](154-client-ingress-adapter-cleanup.md) | Guardrails for old request/response-looking helpers. Startup should not use those helpers to justify a separate remote bootstrap path. |
| [`165-native-feature-parity-baseline.md`](165-native-feature-parity-baseline.md) | Same principle at feature level: native platforms should not silently diverge. Startup behavior should follow the same baseline mindset. |
| [`128-terrain-render-pipeline-coordination.md`](128-terrain-render-pipeline-coordination.md) | Ongoing terrain dirty-to-drawable coordination. Startup seed handoff is a special transient entry into the same render-section lifecycle. |
