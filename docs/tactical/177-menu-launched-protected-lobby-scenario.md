# 177: Menu-Launched Protected Lobby Scenario

Status: active 2026-07-14. Slices 0-2 are landed; authoritative lobby
protection is next. The CLI now projects into the same scene-owned scenario
operation intended for the product menu, and renderer-shell preparation is
independent from retained-runtime attachment. A tokened native managed-content
executor now resolves the path-free intent into versioned, atomically
published lobby and island roots outside the user catalog. This is the
immediate productization milestone after Tactical 175 Slice 6. It deliberately
precedes Tactical 175's remote preview slice so the current local proof is
first expressed as one shared scenario contract rather than accumulating a
second menu-only implementation.

Topic: `embedded-worlds`

Workstream: shared native Rust client-experience, scenario provisioning,
scene/session orchestration, and authoritative server behavior. Desktop flat is
the first interactive validation lane, but the scenario action and policy are
shared. All native clients use the same logical implementation; platform apps
own only writable-root resolution and presentation/lifecycle execution.

## Decision

Promote the landed live-diorama proof into one built-in product scenario:

> From the title menu, **Enter Lobby** opens a protected authored lobby as the
> primary world. The paired demo island is provisioned and warmed
> asynchronously, appears as live geometry on the lobby table when ready, and
> can be entered and returned from through the existing blink activation.

This tactical does not treat the lobby as a special saved-world row. A world is
one persistent server/save. A scenario is a higher-level launch recipe that
selects two world sources, their behavior, presentation, lifecycle, and
authority relationship.

The first scenario remains intentionally fixed:

- primary: Tactical 175's authored grass/table lobby fixture;
- retained destination: Tactical 175's authored island fixture;
- primary behavior: protected against player block mutation;
- destination behavior: ordinary mutable debug-creative behavior;
- presentation: the landed bounded placed-terrain diorama at the current
  player-reviewed scale and anchors;
- activation: the landed 120/30/120 ms blink and complete-slot exchange;
- destination loading: background work that never blocks entering an already
  playable lobby.

The fixed demo is the smallest honest product milestone. Selecting the most
recent local world, starting in the lobby by default, remote destinations, and
multiplayer lobbies follow after this contract is proven.

## Milestone

The milestone is complete when all of the following are true:

- The shared title menu exposes one top-level `Enter Lobby` action distinct
  from `Singleplayer` and `Join Remote`.
- Selecting it launches the same scenario from desktop flat, native XR, flat
  Android, and Android XR through shared client-experience and scene policy.
  No native app contains a private lobby state machine.
- The menu path and the existing `--live-diorama-*` diagnostic path both
  project into one scene-owned scenario/startup seam. The CLI may still select
  explicit fixture roots, but it does not retain a parallel orchestration
  implementation.
- First-party lobby/island content is provisioned beneath stable platform
  application storage, never a product `/tmp` path and never the user-deletable
  local-world catalog.
- Provisioning is idempotent and versioned. Relaunching does not rebuild or
  overwrite a valid scenario database; destination edits survive reopen.
- The lobby becomes playable without waiting for island world startup,
  compilation, GPU admission, or preview readiness.
- The duplicate standby renderer shell and required multiview topology are
  created while startup/loading still covers the frame. Attaching the island
  later must not introduce the currently measured shell/materialization hitch
  into an interactive lobby frame.
- The preview is absent while warming and appears only at the existing exact
  readiness gate. A destination failure leaves the lobby playable and reports
  a nonfatal status.
- The authoritative lobby host rejects forged as well as ordinary client block
  break and placement commands. The island remains editable. Protection stays
  attached to the lobby runtime across both slot exchanges.
- A menu-driven deterministic smoke proves title -> lobby -> asynchronous
  preview -> island -> return, plus protected-lobby and mutable-island block
  behavior and independent persistence.
- Ordinary `Singleplayer`, `Join Remote`, direct CLI world startup, and the
  no-scenario frame path remain unchanged. Five clean release samples show no
  significant no-scenario regression.

## Current Ground Truth

The implementation is close, but its current assembly is diagnostic-shaped:

- `mclone-ui` already owns one shared title layout and typed `GameUiAction`s.
  The visible choices are `Singleplayer`, `Join Remote`, `Options`, and `Quit`.
- `ClientExperienceCore` already classifies and reduces shared menu actions for
  flat, XR, Android, web, offscreen, and emulated profiles. Adding an app-local
  desktop action dispatcher would regress the convergence closed by Tacticals
  143, 165, and 168.
- `SessionStartRequest` correctly describes one leaf session:
  `CreateLocalWorld`, `OpenLocalWorld`, or `JoinRemote`. It should remain
  single-world rather than gain optional lobby/table/standby fields.
- `McloneSceneHost` owns the complete active plus optional retained slot,
  placed preview, blink, authority handoff, lifecycle, and rendering. That is
  the correct owner for scenario execution after content has been resolved.
- `create_desktop_scene_host` currently reads an authored fixture marker from
  the CLI's `--live-diorama-world-dir`, constructs a
  `WarmWorldStandbyRequest`, and immediately calls
  `begin_warm_world_standby`. This is the assembly to extract, not copy into a
  menu handler.
- `WarmWorldStandbyRequest` is explicitly documented and shaped as a
  launch-only local diagnostic. It carries a native `PathBuf` and local seed,
  profile, cadence, and presentation. It is a useful leaf request, not yet a
  platform-neutral product intent.
- `begin_warm_world_standby` creates an empty second
  `TexturedSectionDrawResources`, uploads/owns its atlas resources, and eagerly
  materializes required topology before starting the detached CPU pump. The
  latest evidence measures roughly 15 ms for the empty shell on this Mac, with
  additional capable-device multiview materialization possible. CLI startup
  hides that cost before the interactive loop; a naive asynchronous menu start
  would move it into gameplay.
- `pnpm native:authored-world-fixtures` calls the shared fixture builder and
  writes `/tmp/mclone-live-diorama-fixtures/{table-a,island-b}`. The fixture
  builder is already shared server/content code, but
  `write_authored_world_fixture_dir` deliberately rebuilds its database. A
  product provisioner must not call that destructive behavior on every launch.
- The native user-world catalog already uses stable app-data storage and
  records `last_played_unix_millis`. Its `open_world` operation updates that
  fact immediately, which is wrong for a future background preview open. The
  first fixed managed island avoids prematurely changing catalog semantics.
- Server block interaction currently uses the debug-creative reach/height
  checks, then mutates through the authoritative integrated-server command
  handlers. There is no per-world behavior profile yet; suppressing only flat
  or XR input would be bypassable and would diverge local from dedicated
  authority.

## Experience Contract

### Title and entry

`Enter Lobby` is a top-level title action, provisionally placed before
`Singleplayer`. It is not a row in `Select World`: the lobby is application
content and must not look deletable, renameable, or interchangeable with a user
save.

On selection:

1. The UI leaves the title in a named `Preparing Lobby`/startup state.
2. The host resolves or provisions the primary lobby content without blocking
   the render/event thread on fixture generation, lighting, or filesystem I/O.
3. The ordinary local-session startup pump opens the lobby.
4. The second renderer shell is prepared under the startup/loading cover.
5. As soon as the lobby is playable, it is presented and accepts movement.
6. Island content resolution and retained runtime startup continue
   independently. They may have begun in parallel but are not part of the
   lobby's playable gate.
7. The table contains no preview submission until the island is fully
   switchable. No new loading billboard or render-to-texture placeholder is
   required for this milestone.
8. When ready, the live island appears. Use/right-click invokes the existing
   blink exchange; the paired island table returns to the lobby.

If primary provisioning/startup fails, remain in or return to the title with a
normal blocking session error. If only the island fails, keep the lobby active,
drop incomplete retained resources, keep the table empty, and expose a nonfatal
destination status. Repeated clicks must not create multiple operations,
runtimes, or renderer shells.

### Protected lobby

"Read-only lobby" means protected gameplay, not filesystem read-only:

- movement and camera updates are allowed;
- scene-owned diorama activation is allowed and is consumed before block Use;
- block breaking is denied authoritatively;
- held-item block placement is denied authoritatively;
- the first version may freeze time, scheduled fluid changes, and passive
  showcase activity through existing authored-scene controls;
- SQLite/runtime bookkeeping may still write and close normally.

The policy belongs to the world runtime/server. It is not a mutable global on
`McloneSceneHost`, because the whole-slot exchange must carry the protected
lobby and mutable island without swapping or recomputing their behavior.

Use a new server-owned behavior/interaction profile separate from
`WorldGenerationProfile`. Generation answers what a storage miss produces;
behavior answers which authoritative commands and simulation mutations are
allowed. Existing worlds and servers default to today's mutable behavior.

The exact Rust names remain implementation choices, but the contract is
equivalent to:

```rust
enum WorldBehaviorProfile {
    Mutable,
    ProtectedLobby,
}

impl WorldBehaviorProfile {
    fn allows_player_break(self) -> bool;
    fn allows_player_place(self) -> bool;
}
```

Do not trust a client-supplied protected flag when joining an unrelated remote
server. A future hosted lobby configures its own authoritative server profile.

## Request And Ownership Model

Keep three levels explicit rather than passing paths through UI actions or
turning `SessionStartRequest` into a scene graph.

```text
1. Product intent (shared, path-free)

   GameUiAction::EnterScenario(LobbyPreview)
     -> ScenarioLaunchIntent { id: LobbyPreview }

2. Provisioned content (executor/platform result)

   primary managed content identity + opened storage reference
   destination managed content identity + opened storage reference
   verified manifests/content versions

3. Scene execution (one shared owner)

   primary leaf SessionStartRequest
   prepared retained-world request
   placements/readiness/cadence/behavior
     -> McloneSceneHost scenario startup state machine
```

The path-free intent and built-in scenario definition belong in shared Rust.
Native provisioning may return filesystem-backed prepared content; a future
browser executor may return IndexedDB-backed completion data. That is an
executor/storage difference, like the world catalog, not a second scenario
policy implementation.

### Shared owners

- `mclone-ui`
  - owns the typed scenario menu action, title widget, and capability
    projection;
  - does not know fixture paths or construct runtime options.
- `mclone-app-runtime`
  - owns the path-free scenario intent, client-experience reduction/effects,
    native managed-content operation service, cancellation epochs, and common
    native provisioning executor;
  - does not own `wgpu` resources or draw slots.
- `mclone-server`
  - owns authored lobby/island content recipes and the authoritative world
    behavior policy enforced by command handlers;
  - does not own menu or scene transitions.
- `mclone-scene`
  - owns the prepared scenario state machine, renderer-shell timing,
    primary/standby startup, exact readiness, presentation, blink, and teardown;
  - retains the direct one-world path when no scenario exists.
- app/platform adapters
  - supply a writable managed-scenario root or async storage executor and apply
    host/device effects;
  - do not choose fixtures, placements, behavior, timing, or readiness.

### One execution seam

Extract an explicit scene operation for the already-landed local scenario. The
diagnostic CLI and product provisioning result both call it. Illustrative
shape:

```rust
struct PreparedEmbeddedWorldScenario {
    identity: ScenarioInstanceId,
    primary: PreparedWorldStart,
    destination: PreparedWorldStart,
    preview: EmbeddedPreviewDefinition,
    standby_cadence: Option<SimulationCadenceConfig>,
}

McloneSceneHost::begin_prepared_scenario(...)
```

The concrete prepared native storage representation may remain under
`cfg(not(wasm32))`; the path-free intent, behavior values, state transitions,
and UI policy must compile for wasm. Do not add `winit`, Android, OpenXR, or
browser dependencies to `mclone-scene`.

`WarmWorldStandbyRequest` may remain as the internal local leaf or be renamed
once it is no longer diagnostic-only. Avoid a broad source enum until the
remote slice needs one, but do not bake a fixture path into the shared product
intent.

## Managed Content And Storage

### Separate from user worlds

Scenario content lives beneath a platform-supplied managed root, for example:

```text
<app data>/mclone/scenarios/
  lobby-preview-v1/
    scenario.json
    lobby/
      mclone-authored-fixture.json
      world.sqlite...
    demo-island/
      mclone-authored-fixture.json
      world.sqlite...
```

This layout is illustrative. Shared code must not hard-code the macOS path.
Desktop, Android flat, and Android XR supply app-private roots through one
common native executor. A custom diagnostic root may be injected by tests and
CLI smokes.

Do not place managed scenario worlds under the ordinary `NativeWorldCatalog`
root. They must not appear in `Select World`, participate in rename/delete, or
distort the user's most-recent-world ordering.

### Version and overwrite rules

- Give the scenario and each authored content recipe an explicit stable id and
  content version.
- On absence, build into a staging directory, flush/close the store, write the
  marker/manifest, and publish the complete directory without exposing a
  half-built root.
- On a matching valid manifest, reuse the database unchanged. Do not run the
  current rebuild helper.
- On incompatible content, prefer a new versioned directory. Never overwrite
  an unrelated or user-supplied root.
- On partial/corrupt managed content, surface a reason-bearing repair/failure
  result. Automatic destructive repair is out of the first slice unless the
  staging directory is provably unpublished and scenario-owned.
- Lobby protection makes authored lobby blocks stable; island mutations are
  ordinary persistent edits and must survive scenario close/reopen.

Keep the existing `/tmp` fixture writer and explicit CLI roots for deterministic
smokes. They become projections into the common scenario seam, not product
storage.

## Startup State Machine

The exact type names are provisional; these states and transitions are not:

```text
Idle
  -> ResolvingPrimary
  -> StartingPrimary + PreparingRendererShell
  -> PrimaryPlayable
       + ResolvingDestination / StartingDestination / WarmingDestination
  -> PreviewReady
  -> Activating / ActiveWithReturnPreview

primary failure -> ScenarioFailed -> title/status
destination failure -> PrimaryPlayableWithoutPreview + nonfatal status
quit/replacement/asset epoch/device rebuild -> cancel epoch + ordered teardown
```

Primary readiness and destination readiness remain different gates. Do not
weaken the existing destination `Switchable` contract merely to make the table
populate earlier.

Split second-slot GPU-shell preparation from detached runtime startup. Shell
creation happens exactly once while the primary loading cover is active.
Destination provisioning/runtime completion later fills that prepared slot and
uses ordinary per-frame startup/compile/upload budgets. The first uncovered
lobby frame must not pay for shader compilation, atlas duplication, or lazy
multiview materialization for the destination.

Scenario operations need generation/epoch tokens. A stale completion after
Back, Quit, a second launch request, asset replacement, or device replacement
must be ignored and release its owned resources. Teardown continues using the
existing two-slot flush/cancel/drop ordering from Tacticals 174 and 175.

## Capability And Platform Contract

The lobby is an engine feature, not a desktop-only menu item.

- Desktop flat, desktop XR, flat Android, and Android XR share one supported
  native capability and the same scenario action/state machine.
- Desktop flat supplies the first manual interaction and screenshot evidence.
- Synthetic stereo proves the shared XR input/render behavior without a
  headset. Real native XR validation follows the platform matrix when a capable
  runtime/device is available.
- Web keeps compiling against the shared intent and state vocabulary. If
  managed authored-content provisioning is not implemented in this tactical,
  expose one reason-bearing web capability gap such as "Lobby scenarios need
  IndexedDB managed-content provisioning." The button must be hidden or
  visibly unavailable; it cannot emit a silent no-op.
- A later browser implementation supplies an IndexedDB content executor behind
  the same scenario policy. It must not recreate scenario selection, behavior,
  placements, or readiness in TypeScript.

Add the scenario to the native feature-parity enforcement so it cannot
silently disappear from Android or XR profiles after desktop bring-up.

## Explicit Non-Goals

- No automatic startup in the lobby. The title action is explicit.
- No most-recent-world selection or user destination picker.
- No background open of a user catalog world and no change to
  `last_played_unix_millis` semantics.
- No remote destination, remote observer identity, or multiplayer lobby.
- No scenario browser/catalog, downloadable scenario pack, or user-authored
  scenario format.
- No multiple tables and no N-world registry.
- No baked T0 preview, thumbnail cache, or placeholder render pass.
- No render-to-texture.
- No continuous shrink/fall animation or portal clipping.
- No editable/personal lobby and no lobby inventory/health transfer.
- No filesystem read-only SQLite mode.
- No broad game-mode parity port. The new behavior profile is the narrow
  authoritative break/place policy required by this scenario.
- No duplicate desktop menu/session implementation that is promised to be
  unified later.

## Implementation Slices

### Slice 0: Characterize The Productization Boundary

No intended behavior change.

Deliverables:

- Record every current live-diorama construction call site, direct CLI/path
  dependency, marker read, renderer-shell creation point, startup transition,
  and teardown path.
- Lock the current `GameUiAction`/client-experience action dispatch and title
  layout ownership so a platform-local lobby action cannot land.
- Lock `SessionStartRequest` as a one-world leaf and record the higher-level
  scenario intent boundary.
- Add a source/ownership characterization proving the current local diorama is
  assembled only in desktop launch glue and executed by `McloneSceneHost`.
- Re-run and retain Tactical 175's flat/stereo activation report, covered/first
  visible images, and CLI interactive command as the refactor comparison.
- Capture a clean five-run no-scenario release baseline on an idle host using
  the established variation/outlier policy.

Exit criteria: the exact code to extract, the unchanged path to preserve, and
the performance/pixel receipts are named before refactoring.

#### Slice 0 completion record — 2026-07-14

The pre-refactor ownership is now executable rather than implicit. One
native-client characterization test proves that `desktop_scene_host.rs`
contains exactly one launch-only live-diorama projection, reads the authored
fixture marker, constructs `WarmWorldStandbyRequest`, and delegates execution
to the sole scene-owned `begin_warm_world_standby` seam. The same test locks the
three single-world `SessionStartRequest` variants and confirms that the shared
title layout has no lobby/scenario action before Slice 1. The future refactor
must update this tripwire rather than leave the old projection beside the new
one.

The accepted pre-refactor `native:live-diorama:activation-smoke` retains schema
1 and the established A-to-B-to-A facts: flat first-uncovered frames draw 2/17
sections, stereo frames draw 2/15 sections for both eyes, and every switch and
first-uncovered boundary reports zero construction, compile submission/result
acceptance, or upload. Covered frames remain pixel-identical black. The five
flat image hashes remain `8ffd99d4...e8a9`, `1818d75d...a56e`,
`f8d22803...73a7`, `1818d75d...a56e`, and `f1bb5636...afc3` for initial,
outbound-covered, outbound-visible, return-covered, and return-visible.

The first release performance batch is retained but rejected: its fifth sample
jumped to 3.025 ms average / 5.744 ms P95 with two overruns, and the immediate
postflight found an unrelated Physbox/Cloudflare deployment consuming CPU. No
sample was silently discarded. After that process exited and aggregate CPU
returned to 93.3% idle, five replacement 240-frame runs measured averages of
2.639, 2.652, 2.734, 2.683, and 2.660 ms (2.660 ms median; 3.6% spread) and
P95s of 4.461, 4.524, 4.721, 4.686, and 4.554 ms (4.554 ms median; 5.7%
spread). All retained 1,936 sections with zero overruns and zero accounting
violations. The accepted reports are
`/tmp/mclone-lobby-slice0-perf-{6..10}.json`; the rejected reports remain
beside them as `{1..5}`.

Focused native-client validation and diff whitespace checks pass. This slice
changes no runtime behavior and produces no new pixels requiring review.

### Slice 1: One Scenario Execution Seam, CLI First

Refactor-only for the existing diagnostic behavior.

Deliverables:

- Add the path-free built-in scenario id/intent and the prepared scene request
  boundary without exposing a menu action yet.
- Extract current `desktop_scene_host` live-diorama assembly into one shared
  scene scenario operation.
- Split renderer-shell/topology preparation from destination runtime startup.
  Keep both steps before the interactive loop for the existing CLI path.
- Project `--live-diorama-*` options and fixture markers into the prepared
  scenario request, then delete the hand-expanded orchestration from desktop
  launch glue.
- Preserve fixture roots, scale, source bounds, cadence, readiness, blink,
  mutation, A-to-B-to-A identity, and diagnostics exactly.
- Add ownership tripwires forbidding a second app-local scenario executor.

Exit criteria: `pnpm native:live-diorama:activation-smoke` still produces the
same logical report and representative pixels through the new common seam; no
menu behavior exists yet.

#### Slice 1 completion record — 2026-07-14

Shared `mclone-app-runtime` now owns the serialized, path-free
`BuiltInScenarioId::LobbyPreview` and `ScenarioLaunchIntent`. The prepared
native leaf remains explicitly separate: `PreparedEmbeddedWorldScenario`
carries the storage-resolved retained-world request only after an executor has
resolved content. `SessionStartRequest` remains the unchanged single-world
leaf, and no menu action exists yet.

`McloneSceneHost` now exposes one embedded-world scenario executor used by the
diagnostic diorama projection. It splits duplicate terrain/atlas, placed
renderer, far-LOD, opaque-gate, and required multiview topology creation into
`prepare_*_shell`, then attaches storage/runtime startup through a distinct
`begin_prepared_*` operation. The compatibility warm-world leaf calls those
same two phases. A prepared shell is scene-owned, can exist before a standby
runtime, and is released by the established cancellation/resource-rebuild
path. The desktop CLI retains marker/path projection but no longer expands or
calls detached-world orchestration directly.

Focused app-runtime serialization, scene ownership/lifecycle, native-client
source-lock, and native-web compilation pass. The refactor activation smoke
retains the exact five comparison hashes from Slice 0: `8ffd99d4...e8a9`,
`1818d75d...a56e`, `f8d22803...73a7`, `1818d75d...a56e`, and
`f1bb5636...afc3`. Its flat/stereo A-to-B-to-A report remains switchable with
two destination sections, 8,826 destination indices, covered black frames,
different stereo eyes, and no boundary construction/upload work. Empty shell
creation measured 13.907-14.454 ms in this run and still happens before the
interactive loop for the CLI path. Slice 2 can therefore introduce managed
content without reopening renderer/runtime ownership.

### Slice 2: Versioned Managed Scenario Provisioning

Deliverables:

- Add a shared native managed-scenario content service/executor in
  `mclone-app-runtime` over the existing server-owned authored recipes.
- Define scenario/content manifests and stable ids for lobby-preview v1, lobby
  v1, and demo-island v1.
- Add ensure-if-absent behavior distinct from the current explicit rebuild
  helper. Prove valid content is reused byte-for-byte and destination mutation
  survives reopen.
- Build unpublished content in a staging location and reject unrelated,
  mismatched, corrupt, duplicate, stale, and cancelled completions safely.
- Supply native app-private scenario roots through shared native assembly for
  desktop flat/XR and Android flat/XR. Keep path derivation in adapters.
- Add test-only root injection and a smoke that never touches the user's real
  app-data directory.
- Keep scenario worlds absent from ordinary world-list/create/delete results.

Exit criteria: a path-free lobby intent resolves to two verified persistent
managed worlds on every native host shape without `/tmp`, user-catalog, or
render-thread provisioning policy.

#### Slice 2 completion record — 2026-07-14

`mclone-app-runtime` now owns the native managed-scenario content service and
its background token/epoch executor. `LobbyPreview` resolves to the explicit
`lobby-preview-v1/scenario.json` contract, with `lobby-v1` and
`demo-island-v1` content under separate persistent roots. The service builds
both server-owned authored recipes beneath a uniquely owned unpublished
staging directory, validates their fixture markers and SQLite headers, and
atomically publishes the complete scenario directory. Valid published content
is only read and reused; the destructive diagnostic fixture rebuild helper is
never called against a published product root.

Tests edit an island chunk in SQLite, resolve the scenario again, compare the
database byte-for-byte, and reopen the edited block as air. Corrupt JSON,
valid-but-mismatched content versions, invalid SQLite headers, and unrelated
pre-existing directories produce reason-bearing errors without overwrite.
Stale staging owned by another invocation remains untouched. Two simultaneous
background resolves converge on one valid published root, while the common
platform-operation ledger classifies cancelled-epoch completions as stale and
second completions as duplicates.

Desktop derives the managed root as a `scenarios` sibling of its stable
application `worlds` root. The shared Android platform adapter derives the
same separate internal-app-data child for both flat and XR clients, falling
back through the existing app-data policy. A catalog-separation test provisions
both worlds and proves `NativeWorldCatalog` still lists zero rows. All content
tests use unique injected temporary roots; no test or executor touches the
user's application data. Focused app-runtime, Android-platform, native-client,
format/diff, and native-web build validation pass. This slice creates no new
rendered output.

### Slice 3: Authoritative Protected Lobby Behavior

Deliverables:

- Introduce a default-preserving server-owned world behavior/interaction
  profile separate from generation profile.
- Carry it through local integrated runner configuration and the common server
  construction boundary. Dedicated server configuration may use the same value,
  but a remote client cannot choose it for someone else's server.
- Gate debug instant break and held-item placement at the authoritative command
  handlers. Preserve reach, height, inventory, delta publication, and mutable
  behavior when the profile is ordinary.
- Configure only the managed lobby as protected. Keep the managed island
  mutable.
- Project enough active-world capability state for flat and XR UI/input to
  avoid misleading mutation affordances where practical, while retaining the
  server as the security/correctness boundary.
- Test forged commands directly against the server, ordinary flat and XR
  inputs, both slot-selection directions, independent persistence, and the
  default behavior of existing local/dedicated worlds.

Exit criteria: lobby blocks cannot be destroyed or placed over before or after
a round trip; island edits work and persist; no client-only suppression is
claimed as protection.

### Slice 4: Shared Menu Action And Asynchronous Lobby Startup

This is the first required manual-review checkpoint.

Deliverables:

- Add the shared `Enter Lobby` title widget/action and classify it through
  `ClientExperienceCore`.
- Add scenario capability projection: supported uniformly across native
  profiles, reason-bearing and non-actionable on web until its storage executor
  exists.
- Route the action into one scenario-launch effect applied by thin platform
  hosts. Do not add per-platform scenario state machines.
- Resolve primary and destination content as independently cancellable
  operations. Start/present the lobby when the primary becomes playable;
  destination work remains outside that gate.
- Prepare the empty destination renderer shell and required topology while the
  startup/loading cover is still active. Assert it is not materialized again
  when the destination operation completes.
- Begin the existing budgeted detached startup when destination content is
  available. Keep the table empty until exact readiness, then publish the
  preview without changing active camera/input authority.
- Treat destination failure as nonfatal and primary failure as a normal session
  failure. Handle repeated click, Back/Quit, asset epoch, and device/resource
  replacement with tokened cancellation.
- Add an offscreen menu-input smoke and synthetic stereo action smoke covering
  menu -> lobby -> delayed island -> activate -> return.
- Capture and inspect title, lobby-before-preview, lobby-with-preview,
  island-with-return-preview, and returned-lobby images under `/tmp`.

Exit criteria: the user can launch an ordinary app with no diorama CLI flags,
select `Enter Lobby`, move in the protected lobby before B is ready, see B
appear, enter it, and return. Stop for feedback on the menu label/order,
first-entry loading, empty-table warming presentation, protection feedback,
blink feel, and arrival/return poses.

### Slice 5: Lifecycle, Parity, And Performance Closeout

Deliverables:

- Prove first launch, warm relaunch, island edit/reopen, primary failure,
  destination failure/retry, title cancellation, app exit, asset replacement,
  resource rebuild, and device-loss teardown without leaked operations,
  threads, runtimes, stores, compiler work, or GPU ownership.
- Prove `Singleplayer` catalog list/create/open/delete and `Join Remote` remain
  unchanged and scenario-managed worlds never appear in the catalog.
- Re-run the CLI live-diorama smoke to keep the diagnostic and product entry
  points on the same implementation.
- Run desktop flat, offscreen menu, synthetic stereo, native web compilation,
  and all native adapter/purity gates. Run Android builds through the repository
  scripts; run real XR/device lanes when available and record unavailable
  capable-device multiview evidence honestly.
- Compare five clean no-scenario release runs against Slice 0. Investigate a
  median average/P95 slowdown above 5%; do not accept above 10% without an
  explicit user decision. Verify the normal path allocates no scenario,
  provisioning, retained-slot, or placed-render state.
- Record scenario-on startup, shell, background CPU, memory, thread, and
  first-preview latency separately. Do not hide product cost inside the
  no-scenario number.
- Reconcile this tactical, Tactical 175, `docs/topics/embedded-worlds.md`, the
  tactical index, platform parity, and durable architecture/performance docs.

Exit criteria: menu-launched local lobby is a stable shared native product
scenario, the diagnostic CLI is merely another projection, protection is
authoritative, and ordinary play retains its direct behavior/performance path.

## Validation Matrix

Every slice runs focused tests plus the relevant shared floor:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-ui
cargo test --manifest-path native/Cargo.toml -p mclone-native-client
pnpm native:thin-adapters:purity
pnpm native:scene-host:purity
pnpm native:web:build
git diff --check
```

Pixel-producing/menu slices add new stable commands, provisionally:

```bash
pnpm native:lobby-scenario:smoke
pnpm native:lobby-scenario:stereo-smoke
pnpm native:lobby-scenario:run
pnpm native:live-diorama:activation-smoke
pnpm native:desktop-offscreen:smoke
```

Native platform closeout uses the current commands in `docs/platforms.md`.
Android validation must run through the repository APK scripts rather than bare
Cargo/NDK guesses. Screenshots, scenario roots, reports, and logs remain under
`/tmp` for automated validation; product launches use platform app data.

Performance checkpoints verify the host is mostly idle, reject concurrent
compiler/emulator activity, run at least five samples, and retain every sample
plus any justified outlier rejection. The no-scenario comparison is the
regression gate; scenario-on cost is a separately labeled product receipt.

## First Manual Review

At the end of Slice 4, verify interactively:

1. Launch without live-diorama CLI flags and select `Enter Lobby` from the
   title.
2. Confirm the lobby reaches control independently of island readiness.
3. Try to break and place on lobby terrain/table; neither may mutate.
4. Confirm the empty table gains the live island when readiness completes,
   without a visible shell-creation hitch.
5. Activate the island, edit an island block, return, and activate it again.
6. Quit to title and re-enter; the protected lobby remains authored and the
   island edit remains persistent.
7. Open ordinary Singleplayer and confirm its menu/startup behavior is
   unchanged.

The human decision is about product feel, not correctness substitution: menu
placement/name, initial loading cover, whether an empty warming table needs a
later affordance, protection feedback, and activation/arrival feel.

## Deferred Product Ladder

After this milestone, evolve the same scenario rather than fork it:

1. **Most recently actively played local world.** Resolve a compatible catalog
   world as the destination. Add preview-open semantics that do not update
   `last_played_unix_millis`; commit last-played only on real activation/play.
2. **Immediate cached preview.** Draw a T0 baked snapshot while the T3 live
   destination warms, then replace it transactionally.
3. **Remembered lobby state.** Persist chosen destination and intentional lobby
   state separately from user-world metadata.
4. **Start in Lobby preference.** Promote the stable explicit menu action to an
   opt-in startup preference, and consider a default only after failure and
   performance behavior are proven.
5. **In-lobby destination selection.** Let an in-world table choose saves,
   seeds, or servers. Multiple simultaneously live tables are the point to
   justify an explicit N-world registry and budget.
6. **Remote hosted destination.** Resume Tactical 175 Slice 7 through the same
   source/start seam. An ordinary joined connection comes first; observer
   subscriptions follow only if player-slot identity becomes unacceptable.
7. **Remote or multiplayer lobby.** Join another host's authoritative lobby and
   its shared scenario state. This is a much later networking/authority
   milestone, not a local managed-content extension.
8. **Personal/editable lobby.** Add explicit ownership, mutation, save, and
   migration rules rather than weakening `ProtectedLobby`.

## Expected End State

The app has a real, menu-launched lobby experience rather than a command-line
demo. Its authored lobby is stable, protected, and immediately useful as the
primary scene; its demo island loads and warms independently, appears as live
shared-depth geometry, and can be entered and returned from with no switch-frame
rebuild.

The implementation still has exactly one active and one optional retained
world. It has no automatic lobby startup, user-world binding, remote source,
multiplayer lobby, or N-world registry. Crucially, those later features extend
one shared scenario intent/provisioning/execution contract instead of replacing
a desktop-only prototype.
