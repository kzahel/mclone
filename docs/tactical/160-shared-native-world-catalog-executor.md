# 160: Shared Native World Catalog Executor + Flat Android Persistence

Status: proposed 2026-07-08. Plan only; no implementation started.

Workstream: native Rust shared client-experience/catalog boundary
(`mclone-app-runtime`), shared XR scene driver (`mclone-xr-scene`), desktop flat
client, and flat Android client. Web/WASM is explicitly out of scope for the
shared executor (see Non-Goals).

## Goal

Collapse the three near-duplicate **native** world-catalog request executors into
one shared owner, and make that owner the mechanism that turns flat Android on to
real persistence instead of copying glue a fourth time.

Concretely:

1. Introduce a native `WorldCatalog` trait in `mclone-app-runtime` plus one
   shared `execute_world_catalog_request(...) -> ClientCatalogEffects` function.
2. Port the desktop flat and shared XR executors onto it with **zero behavior
   change** (deleting desktop's hand-expanded request match).
3. Wire flat Android to a real SQLite `NativeWorldCatalog` through the same
   shared executor, replacing its transient stubs. This is a **behavior change**
   (flat Android gains persistent worlds) and is the point of the slice.

The intended end state is the architectural "floor": **one** native catalog
executor + **one** native catalog backend (`SqliteWorldStore`), with web
remaining a separate asynchronous JS/IndexedDB executor over the same shared
request/response protocol. That web `+1` is permanent and accepted, not a gap to
close (see Rationale).

## Rationale: why this is the floor, not a universal owner

The persistence *backend* is already shared: `SqliteWorldStore` and
`WebIndexedDbWorldStore` are both impls of the shared `WorldStore` trait
(`mclone-server/src/persistence.rs:445`) feeding the shared `ChunkScheduler`
(dirty-marking + ticket-driven unload queue). Every non-web target reuses SQLite.

The *catalog policy* is also already shared: `ClientCatalogController`
(`mclone-app-runtime/src/client_catalog_policy.rs`) and the
`WorldCatalogRequest`/`WorldCatalogResponse`/`WorldCatalogCapabilities` value
types are used by all four platforms, including web.

What is **not** shared is the catalog *executor* — the code that takes a
`ClientCatalogRequest`, runs it against a catalog backend, and folds the result
back through the controller. Today there are four copies:

| # | Executor | Backend | Shape |
| --- | --- | --- | --- |
| 1 | `mclone-native-client/src/flat_client_driver.rs:1089` `execute_world_catalog_request` | SQLite | Hand-expands every `WorldCatalogRequest` variant into `list_worlds`/`create_world`/`open_world`/`delete_world`. |
| 2 | `mclone-xr-scene/src/session.rs:1032` `execute_xr_catalog_request` | SQLite | Delegates to `NativeWorldCatalog::handle_request`. Shared by Android XR **and** desktop XR smoke. |
| 3 | `mclone-android-client/src/lib.rs:1341` `execute_android_catalog_request` | none (transient) | Fabricates in-memory summaries; capabilities `transient_create_only()`. |
| 4 | `mclone-web-client/src/web_canvas.rs:3591` (+ JS re-entry) | IndexedDB | Marshals requests out to JS; answers arrive asynchronously via `applyWorldCatalogResponse`. |

Only #4 is forced to exist by wasm: it is asynchronous and lives in JavaScript
because `wasm32` is single-threaded, has no filesystem, and cannot block on
IndexedDB. `SqliteWorldStore` and `NativeWorldCatalog` are both
`#[cfg(not(target_arch = "wasm32"))]` and cannot compile there
(`world_catalog.rs:412`, `mclone-server/src/lib.rs:64`).

Executors #1–#3 are **native and synchronous** and could be one. They are three
copies only because the glue was never lifted. #2 is already the clean form; #1
re-implements the exact `match` that `NativeWorldCatalog::handle_request` already
performs (`world_catalog.rs:437`); #3 is a transient placeholder. Collapsing
#1–#3 to one shared executor is this tactical. #4 stays as the acknowledged
async `+1`.

The cost of shipping targets is therefore per-*executor*, not per-*target*:
desktop XR and Android XR already prove one executor spanning a desktop config
dir and an Android NDK path; headless and the dedicated server add zero catalog
glue. After this slice, native ships an unbounded number of targets through one
executor + one backend.

## Related Docs

| Doc | Relationship |
| --- | --- |
| [`134-shared-persistence-architecture.md`](134-shared-persistence-architecture.md) | Owns the shared host persistence contract/actor boundary, chunk dirty/unload integration, and the native SQLite backend this executor sits above. |
| [`136-world-catalog-and-crud-ui.md`](136-world-catalog-and-crud-ui.md) | Parent. Landed the shared catalog contract, native SQLite catalog backend, and shared UI v2 screens, and explicitly lists Android persistence as follow-up. 160 is the executable slice for that follow-up plus the native executor consolidation. |
| [`../client-experience-architecture.md`](../client-experience-architecture.md) | Owns the shared client-experience core (`ClientCatalogController`, `GameUiAction` classification, per-platform profiles). The shared executor is the missing native counterpart to the already-shared controller. |
| [`143-client-experience-convergence-burn-down.md`](143-client-experience-convergence-burn-down.md) | Closed burn-down that merged desktop/web/XR/Android onto the shared controller and added XR catalog CRUD. 160 removes the remaining native executor duplication that burn-down left behind. |
| [`145-native-rust-organization-refactor.md`](145-native-rust-organization-refactor.md) | Broad module-organization parent. 160 is a targeted convergence, not general reorg; feed only small findings back. |
| [`../platforms.md`](../platforms.md) | Current platform validation policy and platform ownership boundaries. |
| [`../native-web.md`](../native-web.md) | Web/WASM build and smoke context; relevant only to confirm web stays untouched and still compiles. |

## Design

Three parts. Only the middle one is shared; the two ends stay per-platform on
purpose.

### 1. `WorldCatalog` trait (native-only, in `mclone-app-runtime`)

Extracted directly from `NativeWorldCatalog`'s existing inherent methods — it is
already trait-shaped:

```rust
#[cfg(not(target_arch = "wasm32"))]
pub trait WorldCatalog {
    fn capabilities(&self) -> WorldCatalogCapabilities;
    fn handle_request(
        &self,
        request: WorldCatalogRequest,
        active_world: Option<&LocalWorldId>,
    ) -> WorldCatalogResult<WorldCatalogResponse>;
    fn world_dir(&self, id: &LocalWorldId) -> PathBuf;
}

#[cfg(not(target_arch = "wasm32"))]
impl WorldCatalog for NativeWorldCatalog { /* forwards to inherent methods */ }
```

`NativeWorldCatalog` keeps its name and stays the SQLite/filesystem impl. No
`TransientWorldCatalog` is introduced: flat Android is being turned **on** to
SQLite, and genuinely transient targets (headless, initial local session) already
work by passing `catalog: None` to the executor.

### 2. Shared executor (native-only, in `mclone-app-runtime`)

```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn execute_world_catalog_request(
    catalog: Option<&dyn WorldCatalog>,
    controller: &mut ClientCatalogController,
    active_world: Option<&LocalWorldId>,
    request: ClientCatalogRequest,
) -> ClientCatalogEffects {
    let Some(catalog) = catalog else {
        let error = WorldCatalogError::unsupported("Persistent worlds unavailable");
        log::warn!("world catalog action failed: {error}");
        return controller.apply_catalog_error(request.id, error);
    };
    match catalog.handle_request(request.request, active_world) {
        Ok(response) => controller.apply_catalog_response(request.id, response),
        Err(error) => {
            log::warn!("world catalog action failed: {error}");
            controller.apply_catalog_error(request.id, error)
        }
    }
}
```

This owns exactly the duplicated middle: missing-catalog guard →
`handle_request` → `apply_catalog_response`/`apply_catalog_error` → return
effects. It does **not** apply the effects; each app keeps its own
`apply_*_catalog_effects` tail (which drives session/GPU lifecycle and recurses
for nested `catalog_requests`).

Borrow note: apps hold the catalog on `self` (e.g. `self.world_catalog`) and the
controller via `self.client_experience.catalog_mut()`. `NativeWorldCatalog` is
`Clone` and cheap (a `PathBuf`), so each call site clones the catalog first, then
passes `Some(&clone as &dyn WorldCatalog)` alongside the mutable controller
borrow — the pattern XR already uses (`session.rs:1038`
`self.world_catalog.clone()`).

Per-app call sites collapse to:

```rust
let catalog = self.world_catalog.clone();
let active = self.active_local_world_id().cloned();
let effects = execute_world_catalog_request(
    catalog.as_ref().map(|c| c as &dyn WorldCatalog),
    self.client_experience.catalog_mut(),
    active.as_ref(),
    request,
);
self.apply_<platform>_catalog_effects(effects, /* platform ctx */)
```

### 3. Per-platform residue (stays per-app — correct, not a compromise)

- **Writable-root resolution** — desktop `default_native_world_root()`
  (`cli.rs:1457`); XR `android_xr_world_root(app)` NDK path
  (`mclone-android-xr-client/src/lib.rs:251`); flat Android needs a new
  `android_world_root(app)` helper (copy of the XR one). Web has none.
- **Scene-option construction** — desktop `SceneOptions`, XR `XrSceneOptions`,
  Android its own type. The executor never builds scenes; it emits
  `ClientCatalogSessionStart` effects the app turns into its scene.
- **Session-start application** — desktop queues a `FlatClientPendingSessionStart`
  with `arm_mouse_lock`; XR rebuilds the GPU scene inline with `&Device/&Queue`
  (`session.rs:1012`). Already modeled as an effect; stays per-app.

## Contract For Implementing Agents

- **Slices 1–2 are refactor-only.** Desktop flat, desktop XR, and Android XR
  catalog behavior, error text, effect ordering, session-start semantics, and
  diagnostics must be byte-for-byte preserved. The only desktop change is
  deleting the hand-expanded `match` in favor of `handle_request`, which is
  already proven equivalent (`world_catalog.rs:437` builds the identical
  `WorldList`/`WorldCreated`/`WorldOpened`/`WorldDeleted` responses, including
  `capabilities: self.capabilities()` for `ListWorlds`).
- **Slice 3 is the only intended behavior change** (flat Android gains SQLite
  persistence). It must not alter desktop/web/XR behavior.
- The `WorldCatalog` trait and `execute_world_catalog_request` are native-only
  (`#[cfg(not(target_arch = "wasm32"))]`). `mclone-app-runtime` and every touched
  shared crate must still compile for `wasm32-unknown-unknown`, and web catalog
  behavior must be untouched.
- Do not fold web into the shared executor. Do not move Android activity / NDK
  path ownership, OpenXR session ownership, or scene-option construction into
  `mclone-app-runtime`.
- Do not add the native periodic-autosave timer here (see Deferred Work).

## Validation Gates & Tripwires

`cargo check` proves compilation, not behavior. Slices 1–2 claim byte-for-byte
preservation across desktop flat and both XR clients, so the load-bearing gate is
a **behavior-equivalence tripwire**, not a build. Slice 3 is a deliberate
behavior change and needs a real persistence assertion, not just a render smoke.

Per [`../platforms.md`](../platforms.md) the policy is contract tests plus
targeted platform smokes, and device/headset lanes are run only when the change
touches a boundary where that platform can fail uniquely. This change touches the
shared native executor (desktop flat + desktop XR + Android XR), changes flat
Android behavior, and must keep web's async path untouched — so the targeted set
here spans every display lane. It does **not** require perf lanes: this is a
functional refactor, and no Quest/desktop perf smoke is a gate here.

### Behavior-equivalence tripwire (slices 1–2, mandatory)

- **Golden catalog-trace test** in `mclone-app-runtime`: drive a fixed sequence
  (`ListWorlds` empty → `CreateWorld` → `ListWorlds` → `OpenWorld` → `DeleteWorld`
  → error case: open missing id) through `execute_world_catalog_request` against a
  fake `WorldCatalog` and a real `ClientCatalogController`, and assert the exact
  `WorldCatalogResponse` **and** the resulting `ClientCatalogEffects`
  (`catalog_requests` + `session_starts`) at each step. Because all three native
  consumers route through this one function, this single test is the shared proof
  that list/create/open/delete/error semantics and effect ordering are unchanged.
- **Diff discipline**: slices 1–2 must be net-neutral-or-negative LOC on the
  executors and change no error strings (`"Persistent worlds unavailable"`), no
  effect ordering, and no `arm_mouse_lock`/`&Device`/`&Queue` threading. Reviewer
  reads the `flat_client_driver.rs` and `session.rs` diffs against this list.

### Tier 0 — hermetic gates (every slice, must be green; host/CI-runnable)

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime      # incl. golden trace + native_runner_persistent_world_dir_survives_restart
cargo test --manifest-path native/Cargo.toml -p mclone-server           # SqliteWorldStore + scheduler unload/dirty
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

### Tier 1 — host smokes, no device (run for the slices that touch each lane)

```bash
# Desktop flat catalog + persistence (slices 1–2):
pnpm native:desktop-offscreen:smoke            # renders a frame post-refactor
pnpm native:startup-streaming:persisted:smoke  # SQLite persisted world loads & streams
pnpm native:dedicated:smoke                    # server/SQLite boundary intact
# Web async catalog path untouched (all slices — proves wasm +1 unchanged):
pnpm native:web:catalog-smoke                  # --catalog-ui-probe
pnpm native:web:smoke
pnpm native:web:typecheck
```

### Tier 2 — emulator (slice 3, Android behavior change)

```bash
pnpm native:android:apk:avd
pnpm native:android:avd-session-smoke          # exercises new-world catalog session-start on x86_64 AVD
```

Gap to close in slice 3: `avd-session-smoke` covers *create/new-world* but not
*restart persistence*. Slice 3's exit criterion (a created world survives an app
restart) needs either a new `validate-avd.sh` mode (e.g.
`--session-smoke persist-restart`: create → place block → relaunch → assert block
present) or an explicit manual AVD restart pass recorded in this tactical. Do not
mark slice 3 done on `avd-session-smoke` alone.

### Tier 3 — device/headset (owner-run once before closeout; not per-commit)

```bash
pnpm native:xr:check && pnpm native:xr:mclone  # desktop XR shares the executor (slices 1/4)
pnpm native:android-xr:session-smoke           # Quest new-world through shared executor
pnpm native:android-xr:perf:orbit:rd5:persisted # Quest persisted world still round-trips
```

Tier 3 lanes are hardware-gated (XR runtime / Quest headset) and cannot be CI
tripwires. Because the shared executor is XR-consumed, at least one desktop-XR
and one Quest catalog pass must be run and their results recorded here before
Slice 4 closeout — matching how prior XR tacticals record headset validation.

### Per-slice gate summary

| Slice | Mandatory gates |
| --- | --- |
| 1 (trait + executor, port XR) | Tier 0 + golden-trace tripwire + `native:web:catalog-smoke` (untouched) |
| 2 (port desktop flat) | Tier 0 + golden-trace tripwire + desktop Tier 1 + diff discipline |
| 3 (turn on Android) | Tier 0 + Tier 2 + **new/manual restart-persistence assertion** |
| 4 (closeout) | Full Tier 0 + one desktop-XR + one Quest catalog/persist pass (Tier 3), results recorded |

## Slice 1: `WorldCatalog` trait + shared executor, port XR first

Status: proposed.

Goal: introduce the shared abstraction and prove it on the cleanest,
highest-coverage consumer (XR is already delegation-shaped and is shared by
Android XR **and** desktop XR smoke, so one port validates two targets).

Deliverables:

- Add the `WorldCatalog` trait + `NativeWorldCatalog` impl in
  `mclone-app-runtime/src/world_catalog.rs`; re-export from
  `mclone-app-runtime/src/lib.rs`.
- Add `execute_world_catalog_request` beside it (or in a small
  `catalog_executor` module). Unit-test the missing-catalog guard, the Ok path,
  and the Err path against a fake `WorldCatalog` impl and a real
  `ClientCatalogController`.
- Rewrite `mclone-xr-scene/src/session.rs:1032` `execute_xr_catalog_request` to
  call the shared executor and keep `apply_xr_catalog_effects` unchanged.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Exit criteria: the trait + executor exist with tests; XR routes through them; no
behavior change; wasm32 still compiles.

## Slice 2: Port desktop flat, delete the hand-expanded match

Status: proposed.

Goal: remove executor copy #1.

Deliverables:

- Rewrite `mclone-native-client/src/flat_client_driver.rs:1089`
  `execute_world_catalog_request` to call the shared executor, deleting the
  per-variant `list_worlds`/`create_world`/`open_world`/`delete_world` expansion
  (`flat_client_driver.rs:1104-1157`). Keep `apply_world_catalog_effects` and the
  `arm_mouse_lock` threading unchanged.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
# Persistence round-trip regression:
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime native_runner_persistent_world_dir_survives_restart
git diff --check
```

Exit criteria: desktop flat catalog CRUD + persistent-world restart test pass
with no behavior change; the hand-expanded match is gone.

## Slice 3: Turn on flat Android persistence

Status: proposed. This is the intended behavior change.

Goal: replace flat Android's transient stubs with a real SQLite
`NativeWorldCatalog` driven through the shared executor — collapsing executor
copy #3 instead of copying glue a fourth time.

Deliverables (all in `mclone-android-client/src/lib.rs` unless noted):

- Add `android_world_root(app)` (copy of `android_xr_world_root`,
  `internal_data_path()`/`external_data_path()` + `worlds`).
- Add `world_root`/`world_dir` to `AndroidSceneOptions`; populate in
  `android_scene_options_from_startup` and the session-start scene builder.
- In `local_options()`, add
  `if let Some(world_dir) = &self.world_dir { options = options.with_persistent_world_dir(world_dir.clone()); }`.
- Add a `world_catalog: Option<NativeWorldCatalog>` field to
  `AndroidFrameRenderer`, constructed from the resolved `world_root`.
- Change capabilities from `transient_create_only()` (`lib.rs:622`) to
  `persistent_local()`.
- Replace `execute_android_catalog_request` (`lib.rs:1341`) with a call to the
  shared executor plus a real `apply_android_catalog_effects` tail modeled on
  `apply_xr_catalog_effects` (Android already threads `device`/`queue`/`format`
  for GPU scene rebuild). Delete the transient short-circuits and the
  `android_transient_catalog_summary` / `android_transient_world_id` path.
- Set `world_dir = catalog.world_dir(&id)` on scene options at session start for
  opened/created worlds; refresh catalog UI from the store on list/open.
- Relax the startup storage-args rejection (`lib.rs:2270`) enough to honor
  `--world-dir`/`--world-root` as desktop/XR do (optional; in-app CRUD does not
  require it).

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo check --manifest-path native/Cargo.toml -p mclone-android-client
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
# Manual/AVD: create world -> place blocks -> quit -> relaunch -> blocks persist.
# Follow the flat Android AVD validation flow in docs/platforms.md.
```

Exit criteria: flat Android lists/creates/opens/deletes SQLite-backed worlds
through the shared executor; a created world survives an app restart on AVD;
desktop/web/XR behavior unchanged; wasm32 still compiles.

## Slice 4: Closeout + guardrail

Status: proposed.

Deliverables:

- Confirm exactly one native catalog executor remains
  (`execute_world_catalog_request`) and update the executor table above to show
  #1/#2/#3 collapsed, #4 (web) intentionally separate.
- Add a one-line grep guardrail note so future native catalog work starts from
  the shared executor rather than adding a fifth copy.
- Record the durable statement of the floor: one native executor, one native
  backend (`SqliteWorldStore`), web async `+1` permanent.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -p mclone-xr-scene -p mclone-native-client
cargo check --manifest-path native/Cargo.toml -p mclone-android-client -p mclone-android-xr-client
cargo check --manifest-path native/Cargo.toml -p mclone-native-client --features xr
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
git diff --check
```

Exit criteria: one native executor; all native targets green; wasm32 green.

## Non-Goals

- **Folding web into the shared executor.** Web's executor is asynchronous and
  lives in JavaScript/IndexedDB; it cannot implement a synchronous
  `WorldCatalog::handle_request`. Web stays on `web_canvas.rs` +
  `applyWorldCatalogResponse` over the already-shared request/response protocol.
- **Native periodic autosave timer.** Native (all targets) currently persists via
  save-on-unload + save-on-clean-shutdown only; there is no wall-clock autosave
  loop. This bites flat Android hardest because mobile OSes kill processes
  without a clean shutdown. It is a real gap but separable; see Deferred Work.
- **Player / saved-data record persistence.** `WorldStoreRequest::LoadPlayer` /
  `LoadSavedData` remain reserved-but-unimplemented
  (`persistence.rs:795`); schema tables exist but every backend returns
  "reserved but not implemented". Out of scope here; owned by 134/136 follow-up.
- Any `TransientWorldCatalog` abstraction, catalog UI changes, or rename of
  `NativeWorldCatalog`.

## Deferred Work

- **Native periodic autosave** — add a wall-clock/tick autosave to the
  integrated server so still-loaded dirty chunks flush without an unload or clean
  shutdown. Highest value on flat Android (Slice 3 consumer). Should land as its
  own slice under 134/136 and would benefit all native targets. Web already has
  an explicit IndexedDB autosave (`web_server_worker.rs:1942`).
- **Player/saved-data records** — implement the reserved
  `LoadPlayer`/`LoadSavedData` backends so player position/inventory and level
  saved-data persist. Owned by 134/136.
