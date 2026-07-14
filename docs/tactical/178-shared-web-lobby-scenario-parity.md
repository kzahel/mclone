# 178: Shared Web Lobby Scenario Parity

Status: proposed 2026-07-14. The audit is complete; Slices 0-8 are pending.
This tactical closes the browser exception left by Tacticals 174, 175, and
177 by refactoring their native-shaped startup seams into shared contracts.
It does not authorize a second browser scenario implementation.

Topic: `embedded-worlds`

Workstream: shared native Rust and native web/WASM scene/runtime ownership,
managed-content persistence adapters, browser Worker execution, renderer
resource ownership, and cross-platform performance validation. Desktop remains
the fastest refactor-control lane, but web is the product acceptance lane for
this tactical. Android and XR retain the same shared scenario implementation.

## Decision

Promote `Enter Lobby` from a native-supported scenario with a reason-bearing
web exception into one supported feature across all five client/platform
lanes.

This is a shared-first refactor:

- scenario identity, authored content recipes, protection policy, placement,
  lifecycle, readiness, preview composition, activation, and UI semantics have
  exactly one Rust implementation;
- native filesystem/SQLite/thread execution and browser
  IndexedDB/Promise/Worker execution remain thin platform adapters;
- the ordinary single-world path must not allocate, provision, start, poll, or
  render any scenario resource until the user explicitly requests the lobby;
- immutable renderer and asset resources are reused between world slots when
  their asset epoch and device generation match;
- inherently per-world mutable state remains separate and is measured rather
  than disguised as reusable state.

The browser result must be the same product scenario as native: the shared
title-menu action starts a protected authored lobby, a mutable island warms in
the background, the island appears as live shared-depth geometry on the table,
and Use activates A-to-B-to-A through the existing blink and complete-slot
exchange.

## Accountability And Starting Point

The gap is broader than the reason currently shown in the web UI.

- Tactical 174 deliberately kept multi-world web product adoption out of scope
  while extracting `DrawableWorldSlot` and implementing the local warm-world
  path.
- Tactical 175 kept the shared renderer and authored-world contracts portable,
  but permitted browser Worker adoption to remain deferred.
- Tactical 177 made the lobby a native product feature and explicitly left web
  unavailable.
- `web_client_experience_profile()` hard-codes
  `WEB_LOBBY_SCENARIO_REASON` as unsupported.
- `mclone-app-runtime::scenario_content` is entirely
  `cfg(not(target_arch = "wasm32"))` and mixes portable manifests with its
  filesystem executor.
- `mclone-scene` substitutes WASM no-op/false implementations for managed
  launch, standby startup, activation, and parts of lifecycle advancement.
- the detached startup path accepts native `PathBuf` storage and
  `LocalIntegratedStartupPump`, while browser session startup can currently
  complete only into the active slot.

The relevant series ran from the first warm-world ownership checkpoint on
2026-07-13 through the lobby product closeout on 2026-07-14. It spans about 26
commits, including documentation, characterization, performance, and validation
commits. Web compilation remained green, but web feature adoption was deferred
for the whole series rather than at one final menu call site.

The previous statement in `docs/native-web.md` that browser support needs
"only" an IndexedDB executor is therefore incorrect. IndexedDB provisioning is
one missing adapter; shared slot-targeted asynchronous startup, browser
dual-runtime ownership, compiler request isolation, and WASM lifecycle adoption
are also required.

## Milestone

The tactical is complete only when all of the following are true:

- `Enter Lobby` is actionable in the production desktop and mobile browser
  title menu, not hidden, disabled, or reported as unsupported.
- The production browser reaches the protected lobby before the island is
  ready, then attaches the live island preview through the same readiness gate
  as native.
- Browser Use/right-click/touch activation completes lobby -> island -> lobby
  with the same scene-owned blink and complete-slot exchange.
- The lobby rejects break/place authoritatively; the island accepts mutation;
  the island edit survives quit and relaunch through IndexedDB.
- Managed scenario worlds never appear in the user world catalog or update its
  last-played ordering.
- Native desktop, desktop XR, flat Android, and Android XR still consume the
  same scenario policy and retain their current behavior.
- No TypeScript code chooses the scenario worlds, authors blocks, defines
  placement, decides readiness, enforces protection, or performs activation.
- A browser scenario failure is tokened and cancellable. Late completions after
  Back, Quit, replacement, asset epoch change, device loss, or a second launch
  cannot install stale runtime or GPU state.
- The no-scenario single-world path creates no second server Worker, compiler
  namespace, renderer resources, persistence transaction, scenario state, or
  per-frame polling work.
- Five-run native and browser performance comparisons meet the performance
  contract below; all scenario-on CPU, GPU, memory, Worker/thread, storage, and
  startup costs are separately reported.

## Scope Boundary

This tactical includes only local managed content and local integrated browser
Workers. It does not include:

- a remote preview, remote destination, observer subscription, or multiplayer
  lobby;
- most-recent-world selection, automatic lobby startup, or a user destination
  chooser;
- multiple simultaneous previews or an N-world registry;
- render-to-texture, portal clipping, continuous shrink/fall, or recursive
  previews;
- browser WebXR, which is not currently one of the supported client lanes;
- a new web gameplay engine, TypeScript world model, or web-only scene host;
- broad IndexedDB/world-catalog redesign beyond the managed-content records
  required here;
- general scheduler or game-mode work not needed by the scenario.

Tactical 175's remote Slice 7 remains separate. It should later consume the
same slot-targeted start contract rather than broaden this parity milestone.

## Reuse And Duplication Contract

"Shared" means shared policy and shared implementation, not merely equal
behavior maintained in two places.

### Must remain singular

| Concern | Shared owner |
|---|---|
| Scenario id, roles, content versions, and launch recipe | `mclone-app-runtime` |
| Authored lobby/island block and chunk records | `mclone-server` |
| Authoritative mutable/protected behavior | `mclone-server` |
| Title action, status, and capability projection | `mclone-ui` / `mclone-app-runtime` |
| Primary/destination launch state machine and cancellation | `mclone-scene` |
| Playable and preview readiness | `mclone-scene` / `mclone-render-session` |
| Slot ownership, placement, preview composition, blink, and swap | `mclone-scene` |
| Mono, per-eye, and multiview terrain semantics | `mclone-render` / `mclone-scene` |
| Asset epoch and device-generation invalidation policy | `mclone-scene` |
| Performance counters and acceptance schema | shared Rust reporting contracts |

### Platform differences that are allowed

| Native adapter | Browser adapter |
|---|---|
| App-private scenario root | Stable managed IndexedDB keys |
| SQLite `WorldStore` | Existing IndexedDB chunk/entity stores |
| Background filesystem operation/thread | Promise or short-lived provisioning Worker |
| Native integrated-server runner | `WebIntegratedServerRunner` Worker |
| Native compile execution | Web Worker/SAB compile execution |
| Native lifecycle handles | browser Worker/promise/visibility handles |

These adapters may translate bytes, handles, errors, and cancellation tokens.
They must not make product or gameplay decisions.

### State that must remain per world

Two live hosted worlds inherently require two independent:

- authoritative simulations and persistence identities;
- client replicas, connections, cameras, interest centers, and correction
  state;
- traversal caches, upload coordinators, mutable section residency, and GPU
  section buffers;
- lifecycle and failure state.

A separate integrated-server Worker per live browser world is therefore an
expected cost. These are independent world instances, not duplicate feature
implementations.

### Resources that should not be duplicated

When asset epoch and device generation match, the slots should reuse:

- prepared asset bytes and mesh asset lookup data;
- atlas texture/view/sampler ownership;
- shader modules, bind-group layouts, and compatible render pipelines;
- immutable renderer topology/material resources;
- browser compiler asset data and, where safe, the worker execution pool.

The mutable per-slot draw store remains separate. Today
`TexturedSectionDrawResources::new` recompiles pipelines and uploads an atlas
for each slot. Slice 2 replaces that duplicate immutable shell with a shared
resource owner before browser product enablement. If a compiler Worker or other
resource cannot safely be shared, the completion record must name why, record
its retained cost, and show that sharing would violate isolation, correctness,
or frame budgets.

Any new long-lived Worker/thread, duplicate pipeline/atlas, or duplicate
allocation above 1 MiB must appear in a per-slice duplication ledger. No such
duplication is accepted merely because the native proof already did it.

## Current Ground Truth

### Already reusable

- `mclone_server::authored_world_fixture_records` returns a portable manifest
  and ordinary `ChunkRecord`s. Browser code does not need a second fixture
  author or block recipe.
- `WorldBehaviorProfile` already enforces protected lobby behavior in shared
  authoritative command handlers.
- `DrawableWorldSlot`, its install aggregate, `SceneSessionRuntime`, placed
  terrain rendering, water ordering, and blink/swap policy are shared Rust.
- Mono preview composition already submits active and placed world geometry
  through one depth target. Per-eye and full-frame multiview implementations
  use the same placement contract.
- The production browser already creates active local IndexedDB-backed runtime
  Workers and installs them into `McloneSceneHost` through the external scene
  completion seam.
- The existing browser database already stores chunk and entity-chunk records
  keyed by world id, and the world catalog is separated by its `worlds` store.
- `PlatformOperationService` already supplies tokens, epoch cancellation, late
  completion rejection, and deferred browser operation patterns.

### Missing seams

| Gap | Current shape | Required shape |
|---|---|---|
| Managed scenario schema | mixed into native filesystem module | portable plan/validation plus native and web executors |
| World storage identity | `PathBuf` in standby request | stable path-free managed-world key |
| Scenario lifecycle | owns native content service | shared tokened state over injected operations |
| Slot start | native `LocalIntegratedStartupPump` only | platform service completes a neutral runtime into a named slot |
| Browser completion | active-slot session replacement | explicit primary or destination completion |
| Startup readiness | native startup seed meshes | native seed or externally resident-runtime evidence |
| Web runner settings | generation profile only | generation plus behavior/freeze settings |
| Browser compiles | one unqualified request-id map | runtime/slot-qualified request ownership |
| WASM activation | false/no-op stubs | shared preview and swap advancement |
| Managed persistence | native versioned directories | versioned, catalog-excluded IndexedDB transaction |
| Renderer shell | duplicate atlas/pipelines per slot | shared immutable resources, separate mutable draws |
| Browser lifecycle | one active runtime | two-runtime cancellation/shutdown/flush/invalidation |

## Target Architecture

Keep product intent, content, runtime execution, and storage execution at
separate levels:

```text
shared title action
  -> ScenarioLaunchIntent { LobbyPreview }

shared scenario definition
  -> ManagedScenarioPlan
       primary role + content key + behavior + fixture
       destination role + content key + behavior + fixture
       placement + cadence + activation policy

platform managed-content executor
  native: versioned app-private SQLite roots
  web: versioned catalog-excluded IndexedDB records
  -> ProvisionedScenario { stable primary/destination keys }

shared McloneSceneHost state machine
  -> StartScenarioWorld { operation token, role, managed key, start facts }

platform slot-start executor
  native: NativeSessionStartupPump / integrated runner
  web: WebIntegratedServerRunner / Worker
  -> StartedWorldSlot { role, SceneSessionRuntime, admission evidence }

shared scene admission and composition
  -> active slot playable
  -> destination CPU ready
  -> destination GPU/placement ready
  -> preview visible
  -> shared blink and whole-slot activation
```

Exact type names may change, but the boundary may not collapse back into a
native path or a JavaScript scenario state machine.

### Storage-neutral identity

The scene should carry a stable `ManagedWorldKey` equivalent, not a path,
SQLite handle, IndexedDB object, or JS promise. Native and web adapters map the
key to their storage. The key must include enough scenario/content version
identity to prevent incompatible content reuse without exposing the storage
layout to scene policy.

### Slot-targeted asynchronous startup

Generalize the existing external active-session completion into a role-aware
operation. A completion installs into the primary or destination slot named by
its token; it must not replace the active world accidentally. The shared state
machine owns ordering and stale-result rejection. Platform adapters only start
and return services.

The completion aggregate should contain a target-neutral
`SceneSessionRuntime`, descriptor/camera authority facts, and admission
evidence. It should not expose a native runner type or browser Worker object to
`mclone-scene`.

### Two readiness sources, one policy

Native local startup currently supplies transient startup seed meshes. Browser
external runtimes begin with an empty draw store and become drawable from
resident runtime/compiler updates. Standby admission must accept either source
without creating two definitions of `Switchable`:

1. runtime/spawn authority established and the initial correction resolved;
2. entry interest and bounded source records available;
3. required compile/upload work admitted under budget;
4. asset epoch, device generation, placement, and renderer topology coherent;
5. entry coverage and activation endpoint ready.

The evidence producer differs; the readiness decision remains shared.

### Browser persistence

Upgrade the existing browser database with a managed-scenario metadata store
and stable reserved world ids. Lobby/island records may reuse the existing
chunk and entity-chunk stores because those are already keyed by world id, but
managed worlds must not receive `worlds` catalog rows.

Provision metadata, chunks, and entity chunks atomically. Shared Rust produces
the expected manifest and authored `ChunkRecord`s. TypeScript may execute the
IndexedDB transaction and carry opaque records, but may not reproduce fixture
generation or validation policy. Provisioning must not block the rAF thread;
use a short-lived Worker or another measured asynchronous path if record
materialization/serialization is not safely below budget.

### Browser compiler ownership

Two Rust runtime/compiler clients cannot share an unqualified JavaScript
`Map<request_id, ...>` because request ids may overlap. Introduce an explicit
runtime/slot namespace into request, timing, completion, cancellation, and
asset-epoch ownership.

Prefer one host-scoped browser compiler broker and shared worker pool over two
independent copies when the existing resident protocol can be multiplexed
without head-of-line blocking. Per-runtime queues/rings are valid mutable state;
duplicating asset payloads or a full worker pool requires the duplication
ledger and measurement described above.

## Performance Contract

Performance is a milestone gate, not a final polish slice.

### Feature-off invariant

Before `Enter Lobby` is requested, all clients—including web—must retain:

- one runtime and one authoritative host;
- one direct draw store and no placed-preview submission;
- no managed-content storage open or provisioning request;
- no scenario Worker/thread, second compiler namespace, or standby cadence;
- no duplicate atlas, pipeline set, renderer shell, upload coordinator, or
  section cache;
- no scenario-specific per-frame loop beyond an already-existing optional-state
  check shown insignificant by measurement.

Source/ownership tests must lock these absences. Timing alone is insufficient
because a dormant allocation or Worker may be hidden by noise.

### Measurement protocol

- Build before measuring; no Cargo, compiler, bundler, emulator, deploy hook,
  or unrelated high-CPU process may overlap a retained sample.
- Record system idle state immediately before and after each batch.
- Run at least five samples, retain every sample, and report median, range, and
  P95. Never silently discard an outlier.
- If a sample is rejected, retain it with the measured external cause and rerun
  the complete batch.
- For shared scene/render changes, use interleaved candidate/control runs from
  already-built binaries or bundles when machine drift exceeds the accepted
  within-batch range.
- A median average or P95 slowdown above 3% triggers investigation. Above 5%
  blocks slice closeout unless an untouched interleaved control attributes the
  movement to the environment and the candidate is not worse. Above 10%
  requires an explicit user decision even with an explanation.
- The existing deterministic scene/section/draw/accounting facts must remain
  equal. Zero budget overruns and zero accounting violations remain required.

The initial control anchors are Tactical 177's accepted native no-scenario
batch: 2.525 ms median average and 4.350 ms median P95 on the recorded M4 Pro.
They are historical context, not portable machine budgets. Slice 0 records a
fresh native control and a production-browser control on the current machine,
browser version, viewport, and asset selection.

### Scenario-on receipts

Record these separately from the feature-off regression gate:

- click-to-lobby-playable latency;
- lobby-playable-to-first-preview latency;
- first activation and return latency;
- maximum and P95 rAF main-thread contribution during provisioning, Worker
  startup, compile acceptance, and GPU upload;
- longest attributable frame gap/long task;
- server, job, compiler, and provisioning Worker counts and lifetimes;
- retained Rust/WASM, JS, SharedArrayBuffer, section CPU, estimated GPU, and
  IndexedDB bytes where observable;
- compile queue depth, result age, upload work, and per-slot admission tails;
- standby idle CPU and the effect of supported cadence throttling;
- mobile-browser CPU-throttled startup and steady-state evidence.

No filesystem/IndexedDB scan, fixture generation, large serialization, shader
compilation, atlas upload, or Worker initialization may execute synchronously
inside an uncovered interactive frame. Background work must use the existing
bounded compile/accept/upload grants and must not consume the active world's
last render-thread grant.

## Implementation Slices

### Slice 0: Lock The Parity Debt And Baselines

No intended product behavior change.

Deliverables:

- Add executable ownership inventories for every `cfg(wasm32)` scenario stub,
  native path-bearing request, native-only lifecycle state, active-only browser
  completion, and TypeScript compiler request map.
- Lock the shared owners listed above and reject TypeScript scenario policy,
  fixture ids, placement constants, readiness decisions, and activation state.
- Record the initial duplication ledger: per-slot mutable state, atlas/pipeline
  copies, compiler workers, server/job workers, retained bytes, and thread/Worker
  counts.
- Capture a fresh five-run native feature-off control and a five-run production
  browser control after idle-system preflight.
- Re-run native lobby A-to-B-to-A and inspect its flat/stereo captures as the
  behavior comparison.
- Add a disabled-web smoke that proves the current reason-bearing state so the
  later promotion is an explicit tested transition.

Exit criteria: the exact native assumptions to remove, every allowed duplicate,
and both performance controls are named before refactoring.

Estimated effort: 0.5-1 day.

### Slice 1: Split Shared Scenario Content From Storage Execution

Refactor `mclone-app-runtime::scenario_content` into portable scenario
definitions/validation and platform executors.

Deliverables:

- Move manifests, ids, versions, expected roles, behavior profiles, and logical
  validation out of the native-only module.
- Define stable storage-neutral managed-world keys.
- Keep the native filesystem/SQLite executor behind the same portable plan and
  preserve its staging, atomic publish, reuse, and corruption behavior.
- Build browser provisioning payloads from
  `authored_world_fixture_records`; add no TypeScript fixture builder.
- Add cross-target serialization/validation fixtures proving native and web
  consume the same manifest and record hashes.
- Keep native lobby launch and no-scenario construction byte/behavior stable.

Exit criteria: portable schema tests compile for WASM, native scenario tests
pass unchanged, and all filesystem types stay inside the native executor.

Estimated effort: 0.5-1.5 days.

### Slice 2: Share Immutable Second-Slot Renderer Resources

Remove the avoidable per-slot atlas/pipeline shell before adding browser cost.

Deliverables:

- Split immutable asset/device-generation terrain resources from mutable
  section draw residency.
- Let active and standby slots reference one compatible immutable owner while
  retaining independent section buffers and draw records.
- Preserve eager multiview materialization and asset/device invalidation.
- Prove a slot exchange performs no shader/pipeline construction, atlas upload,
  compile, or section upload.
- Update retained-byte and shell-creation accounting to distinguish shared base
  resources from per-slot mutable resources.
- Inspect flat and synthetic-stereo rendered output at the first drawable
  checkpoint.
- Run the five-sample native feature-off batch; this slice cannot close with an
  unresolved direct-path regression.

Exit criteria: the duplication ledger contains no second atlas or compatible
pipeline set, pixels/readiness match the control, and the ordinary single-world
path does not gain indirection measurable above the performance contract.

Estimated effort: 1-2 days.

### Slice 3: Generalize Scenario And Slot-Targeted Startup

This is the largest shared-ownership refactor. Preserve native behavior before
adding browser execution.

Deliverables:

- Replace `PathBuf`-shaped scene requests with portable managed-world keys and
  start facts.
- Add tokened primary/destination start operations that complete into a named
  slot with a neutral `SceneSessionRuntime` aggregate.
- Move managed launch state, renderer preparation, readiness advancement,
  activation requests, and slot swap out of native-only compilation where their
  dependencies are host-neutral.
- Keep native runner/pump construction in native assembly; adapt it to the new
  operation rather than retaining the old direct path.
- Generalize readiness evidence for native startup seeds and externally started
  runtimes while retaining one `Switchable` policy.
- Preserve camera-correction acknowledgement, terrain-relative endpoints,
  cadence restoration, asset epoch, device generation, and late completion
  cancellation.
- Delete superseded native-only launch/state-machine branches rather than leave
  compatibility implementations.
- Run native lobby, diagnostic diorama, cancellation, asset replacement,
  persistence, flat, stereo, and WASM compilation gates.

Exit criteria: native product and diagnostic paths use the new shared seam,
`mclone-scene` contains no false/no-op WASM scenario policy stubs, and no
browser runtime has been added yet.

Estimated effort: 2-3 days.

### Slice 4: Provision Managed Scenarios In IndexedDB

Implement only the unavoidable browser storage adapter.

Deliverables:

- Upgrade the existing database schema with managed-scenario metadata without
  invalidating user catalog worlds.
- Use deterministic reserved world ids for lobby and island records; never add
  them to the user `worlds` catalog store.
- Commit metadata, chunks, and entity chunks transactionally.
- Reuse valid matching content without rewriting the mutable island.
- Detect missing, partial, incompatible, and corrupt managed content through
  the shared validation result vocabulary.
- Execute materialization and writes off the uncovered rAF path; measure the
  main-thread serialization cost before selecting main-thread promise work or a
  short-lived Worker.
- Prove cancellation and concurrent duplicate requests publish at most one
  valid version and leak no staging identity.
- Prove ordinary title entry and Singleplayer never open or populate managed
  scenario content.

Exit criteria: a browser storage smoke provisions, reopens, excludes, corrupts,
and safely reports the two managed worlds without launching a second runtime.

Estimated effort: 1-2 days.

### Slice 5: Start And Own Two Browser Runtime Workers

Connect provisioned managed keys to the shared slot-start service.

Deliverables:

- Extend `WebIntegratedServerRunnerConfig` and its Worker ABI with the shared
  world behavior profile and required authored-scene freeze settings.
- Start the protected lobby as primary and the mutable island as destination
  with independent Worker/runtime/persistence ownership.
- Qualify compiler requests, timings, results, cancellation, and asset epochs by
  runtime/slot identity.
- Prefer a host-scoped compiler broker/shared pool; record and justify any
  unavoidable per-runtime compiler worker or asset duplication.
- Drain and acknowledge initial camera corrections for both runtimes.
- Apply existing standby cadence where supported; if browser cadence remains
  unavailable, measure the cost and keep the typed gap explicit rather than
  adding an untyped message.
- Prove independent chunk identity, block mutation, shutdown, and persistence
  with no cross-slot updates or request-id collisions.
- Prove Back/Quit/relaunch terminates both server Workers and any provisioning
  Worker without leaving rAF callbacks or promises able to reinstall state.

Exit criteria: two browser runtimes remain alive concurrently and independently
mutable, but the destination is not yet required to render.

Estimated effort: 1.5-3 days.

### Slice 6: First Browser Lobby And Live Preview Pixels

This is the first required human review checkpoint.

Deliverables:

- Enable the shared `Enter Lobby` capability for web only when the complete
  service assembly is installed; remove the unsupported reason.
- Drive destination runtime records through shared standby preparation,
  compile/accept/upload budgets, placement, and exact preview readiness.
- Make the lobby playable independently of island readiness.
- Add a production browser menu smoke that captures title, playable lobby,
  empty/warming table when observable, and first live-preview frame.
- Add a CPU-throttled mobile-browser version of the smoke.
- Inspect every new capture for correct table scale, depth, water ordering,
  lighting, and absence of single-frame stale/foreign geometry.
- Record first-preview latency, main-thread frame gaps, queue tails, worker
  counts, retained bytes, and shared-versus-per-slot GPU estimates.
- Run a five-sample browser feature-off comparison and an interleaved control if
  the 3% investigation threshold is crossed.

Manual review:

1. Open the production web app and choose `Enter Lobby` from the title.
2. Confirm control begins in the protected lobby before the island is ready.
3. Confirm the island appears on the four-block ground-level table without a
   visible frame hitch or page freeze.
4. Walk around the table and inspect shared-depth occlusion and water.
5. Attempt break/place in the lobby and confirm neither mutates.
6. Open ordinary Singleplayer afterward and confirm its startup remains normal.

Exit criteria: desktop and mobile browser show the same live scenario preview
as native through shared scene/render code, and the user accepts the basic
visual/startup feel before activation lifecycle work proceeds.

Estimated effort: 1-2 days.

### Slice 7: Activation, Persistence, And Adversarial Lifecycle

Complete behavioral parity rather than stopping at first pixels.

Deliverables:

- Route mouse/touch Use through the existing shared preview ray and activation
  request.
- Prove lobby -> island -> lobby with covered and first-uncovered frames,
  correct authority/behavior ownership, and no boundary construction/compile/
  upload work.
- Mutate the island, quit to title, relaunch, and prove the edit persists while
  lobby protection remains intact.
- Exercise Back during primary provisioning, Quit during destination startup,
  repeated Enter Lobby clicks, asset replacement while standby warms, device
  loss/resource rebuild, visibility suspension/resume, and failure of only the
  destination.
- Reject all stale completions by operation, asset, and device generation.
- Flush and terminate both stores/runtimes independently.
- Retain native flat/stereo A-to-B-to-A and Android build gates.

Exit criteria: the production browser scenario passes the same functional
contract as native, including return, persistence, and cancellation.

Estimated effort: 1-2 days.

### Slice 8: Performance And Platform-Parity Closeout

No new experience behavior.

Deliverables:

- Run final five-sample native and production-browser feature-off batches with
  idle-system pre/post checks and retained samples.
- Run desktop and CPU-throttled mobile browser scenario-on receipts, including
  startup tails, frame gaps, Worker lifetimes, CPU, memory, retained bytes,
  compile/upload queues, and activation boundaries.
- Run the complete native/web/platform validation matrix below.
- Confirm no second scenario implementation, compatibility branch, false WASM
  stub, unsupported capability, or undocumented duplication remains.
- Update `docs/platforms.md`, `docs/native-web.md`,
  `docs/topics/platform-parity.md`, `docs/topics/embedded-worlds.md`, and the
  Tactical 177 handoff/status.
- Record unavailable real-device receipts honestly; do not use synthetic stereo
  as a replacement for capable-device multiview execution.

Exit criteria: the web exception is removed, all five platform profiles support
the scenario, feature-off performance clears the contract, and scenario-on cost
is explicit enough to guide future lobby-default or cached-preview decisions.

Estimated effort: 1-2 days.

## Validation Matrix

Every slice runs its focused crate and source-ownership tests. Shared
scene/runtime or renderer slices additionally run:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-server
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-render-session
cargo test --manifest-path native/Cargo.toml -p mclone-render
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:thin-adapters:purity
pnpm native:scene-host:purity
pnpm native:web:build
git diff --check
```

Existing behavior gates remain:

```bash
pnpm native:lobby-scenario:smoke
pnpm native:lobby-scenario:stereo-smoke
pnpm native:live-diorama:activation-smoke
pnpm native:desktop-offscreen:smoke
pnpm native:xr-emulation:smoke
```

Add stable browser commands rather than one-off scripts:

```bash
pnpm native:web:lobby-scenario-smoke
pnpm native:web:lobby-scenario-mobile-smoke
pnpm native:web:lobby-scenario-lifecycle-smoke
```

Browser ABI/runtime changes also run the existing matrix:

```bash
pnpm native:web:thread-smoke
pnpm native:web:app-smoke
pnpm native:web:catalog-smoke
pnpm native:web:mobile-smoke
pnpm native:web:asset-pack-smoke
pnpm native:web:block-edit-probe
pnpm native:web:movement-perf
pnpm native:web:remote-smoke
```

Final native platform validation follows `docs/platforms.md` and uses the
repository Android scripts:

```bash
pnpm native:android:apk
pnpm native:android:avd-smoke -- --skip-build
pnpm native:android:avd-session-smoke -- --skip-build
pnpm native:android-xr:apk
pnpm native:xr:check
```

Use real device/headset lanes when available. Pixel-producing slices capture
under `/tmp` and inspect the image before proceeding. Browser screenshots,
reports, storage probes, and logs likewise stay outside the repository.

## Risk Register

1. **Compiler request collision or head-of-line blocking.** Two runtime clients
   currently meet one unqualified JS timing/request map. Namespace first; then
   measure whether a shared broker or separate execution pools give the safer
   bounded result.
2. **External standby readiness.** Browser startup has no native seed-mesh
   bundle. Readiness must consume resident-runtime evidence without weakening
   entry coverage or forking policy.
3. **IndexedDB atomicity and migration.** Managed metadata and chunk records
   must publish together without corrupting existing catalog worlds.
4. **Late promise/Worker completion.** Browser teardown and asset/device epochs
   make stale installation more likely than native thread completion. Tokens
   are mandatory at every boundary.
5. **Main-thread serialization.** Rust-authored records still cross a WASM/JS
   boundary. Measure and move the work off rAF if it threatens a frame.
6. **Two-world browser memory.** Independent simulations and section buffers
   are real costs. Share immutable resources, bound the preview region, and
   record retained bytes before considering broader scenarios.
7. **Mobile browser CPU.** A second Worker can contend for limited cores even
   when rAF remains light. Cadence and bounded preparation are sanctioned
   levers; silent quality divergence is not.
8. **Native regression from abstraction.** The largest shared refactor sits in
   a hot scene path. Source-level absence locks plus interleaved five-run
   controls guard the direct one-world case.

## Checkpoints And Stop Rules

- After Slice 2, stop if immutable renderer sharing regresses the one-world
  path or changes pixels; do not carry a questionable resource abstraction into
  browser work.
- After Slice 3, native lobby behavior must be fully restored through the new
  shared seam before any browser scenario runtime lands.
- After Slice 5, stop if two browser runtimes cannot shut down independently or
  compiler completions cannot be unambiguously attributed.
- After Slice 6, request human visual/startup review before activation closeout.
- At any slice, a feature-off regression above the performance contract blocks
  progression until attributed and resolved or explicitly accepted by the
  user.
- Never close a slice by adding a web-only policy implementation, a permanent
  compatibility branch, or a new reason-bearing exception that merely renames
  this one.

## Planning Estimate

Functional web parity was previously estimated at 7-12 focused engineering
days. This tactical adds an explicit immutable-renderer reuse checkpoint rather
than carrying the native duplicate shell forward, so plan around **8-14 focused
engineering days**, with **2-3 weeks** as the risk range if compiler
multiplexing, IndexedDB migration, or mobile-browser memory exposes deeper
work. Estimates are not acceptance criteria; each slice closes only on its
evidence.

## Expected End State

`Enter Lobby` is one cross-platform engine feature. Native and browser clients
choose the same shared action, resolve the same versioned Rust-authored worlds,
run the same protected/mutable authority policy, use the same scene lifecycle
and readiness gates, submit the same live shared-depth preview geometry, and
activate through the same blink and whole-slot exchange.

The only platform-specific code is the code that must be platform-specific:
filesystem/SQLite/thread handles on native and IndexedDB/Promise/Worker handles
in the browser. The single-world case stays direct and cheap. The two-world
case pays only for independent world state plus measured per-world mutable
render data, while immutable assets and renderer resources are shared.

That foundation supports later most-recent-world, remote destination, and
multiplayer-lobby milestones without replacing either a native implementation
or a browser implementation, because there is only one scenario
implementation to extend.
