# 178: Shared Web Lobby Scenario Parity

Status: active 2026-07-14. Slices 0-7 locked the baseline, extracted portable
scenario content, removed the duplicate second-slot terrain shell, and moved
managed provisioning, slot-targeted startup, readiness, activation, and swap
policy into shared Rust. Catalog-excluded, Worker-backed IndexedDB provisioning
and dual browser runtime ownership through one namespaced compiler broker are
also complete. The production desktop and mobile browser title enters the
protected lobby, renders its live island preview, activates in both directions,
persists destination edits, and rejects late runtime starts across cancellation,
asset replacement, and resource rebuild. Performance and platform-parity
closeout is next. This tactical closes the browser exception left by Tacticals
174, 175, and 177 by refactoring their native-shaped startup seams into shared
contracts. It does not authorize a second browser scenario implementation.

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
- browser compiler asset data and the host-scoped worker execution pool.

The mutable per-slot draw store remains separate. Today
`TexturedSectionDrawResources::new` recompiles pipelines and uploads an atlas
for each slot. Slice 2 replaces that duplicate immutable shell with a shared
resource owner before browser product enablement. The resources listed above
are required shared owners. If implementation evidence shows that one cannot
be shared while preserving isolation, correctness, or frame budgets, stop and
report the boundary; do not silently reclassify it as an allowed duplicate in a
completion record.

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
| Browser compiles | one unqualified request-id map | stable-world-instance-qualified request ownership |
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
  <- ProvisionManagedWorld { operation token, role, content key }
  -> ProvisionedManagedWorld { role, stable managed key }

  primary and destination execute independently
  primary completion may start while destination is still provisioning

shared McloneSceneHost state machine
  -> StartScenarioWorld {
       operation token,
       stable WorldInstanceId,
       current role,
       managed key,
       start facts,
     }

platform slot-start executor
  native: NativeSessionStartupPump / integrated runner
  web: WebIntegratedServerRunner / Worker
  -> StartedWorldSlot {
       stable WorldInstanceId,
       role,
       SceneSessionRuntime,
       admission evidence,
     }

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

Persistence identity and live runtime identity are distinct. A stable
`WorldInstanceId` is assigned when a world-start operation is accepted and
remains attached to that runtime, compiler client, draw state, diagnostics,
and cancellation ownership through every whole-slot exchange. `Primary`,
`Destination`, `Active`, `Standby`, and physical slot A/B are changing roles,
not identities. They must never namespace compiler requests, promises,
persistence, or lifecycle ownership.

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
and stable reserved world ids. Lobby/island records reuse the existing chunk
and entity-chunk stores because those are already keyed by world id, but
managed worlds must not receive `worlds` catalog rows. This same-database shape
is the locked default; changing it requires migration evidence rather than a
second scenario database chosen inside an implementation slice.

Provision each managed world's metadata, chunks, and entity chunks atomically
in its own tokened transaction. The shared scenario manifest relates the two
world definitions, but there is no combined publication barrier: primary
publication can start the lobby while destination provisioning continues, and
destination failure never rolls back a valid primary. Shared Rust produces the
expected manifest and authored `ChunkRecord`s. TypeScript may execute the
IndexedDB transaction and carry opaque records, but may not reproduce fixture
generation or validation policy. Provisioning must not block the rAF thread;
use a short-lived Worker or another measured asynchronous path if record
materialization/serialization is not safely below budget.

### Browser compiler ownership

Two Rust runtime/compiler clients cannot share an unqualified JavaScript
`Map<request_id, ...>` because request ids may overlap. Introduce an explicit
stable `WorldInstanceId` namespace into request, timing, completion,
cancellation, and asset-epoch ownership. Slot or role names are forbidden
because activation exchanges them.

Use one host-scoped browser compiler broker and one shared worker execution
pool. Each world keeps its own namespaced queue/ring, cancellation state,
priority, and result accounting; immutable compiler asset data and Worker
execution capacity remain shared. Admission must preserve active-world
priority and prevent standby head-of-line blocking. If the existing resident
protocol cannot be multiplexed while preserving isolation and frame budgets,
stop with measurements and an interface analysis. Do not silently create a
second full compiler pool or duplicate asset payloads inside Slice 5.

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

## Unattended Execution Protocol

One explicit instruction to implement this tactical authorizes sequential work
through Slices 0-8. It does not waive any acceptance or stop condition.

For every slice, the implementing agent must:

1. update this tactical with the slice status and a concrete completion record;
2. implement only the bounded slice, splitting it into smaller verified commits
   if necessary rather than carrying an unverifiable large change;
3. run the focused tests, source/ownership gates, and performance checks named
   by the slice;
4. capture under `/tmp` and visually inspect every newly pixel-producing path
   before proceeding;
5. retain performance samples, reports, and any rejected sample with its
   measured external cause;
6. commit each green endpoint with `Topic: embedded-worlds`; and
7. continue automatically to the next slice when its exit criteria are met.

An unavailable optional device receipt is recorded honestly and does not by
itself pause host-neutral work when the existing compile, synthetic-stereo,
and available-device gates pass. A change to platform-specific XR behavior
still requires the relevant real-device gate before it can be called proven.

The Slice 6 interactive checklist is an unattended rendered-output checkpoint:
the agent runs the production browser path, inspects desktop and mobile
captures, and continues when the objective checks are unambiguous. It pauses
for human feedback only when visual/product judgment is genuinely required or
one of the stop rules below fires.

## Implementation Slices

### Slice 0: Lock The Parity Debt And Baselines

No intended product behavior change.

Deliverables:

- Add executable ownership inventories for every `cfg(wasm32)` scenario stub,
  native path-bearing request, native-only lifecycle state, active-only browser
  completion, and TypeScript compiler request map.
- Lock the shared owners listed above and reject TypeScript scenario policy,
  fixture ids, placement constants, readiness decisions, and activation state.
- Lock stable `WorldInstanceId` namespaces against active/standby, role, or
  slot-derived compiler and lifecycle identity.
- Lock independent primary/destination provisioning and reject a combined
  completion barrier that delays primary startup.
- Record the initial duplication ledger: classify per-slot mutable state,
  atlas/pipeline copies, compiler workers, server/job workers, retained bytes,
  and thread/Worker counts as required shared, inherently per-world, or pending
  removal. Leave no unclassified duplicate.
- Capture a fresh five-run native feature-off control and a five-run production
  browser control after idle-system preflight.
- Re-run native lobby A-to-B-to-A and inspect its flat/stereo captures as the
  behavior comparison.
- Add a disabled-web smoke that proves the current reason-bearing state so the
  later promotion is an explicit tested transition.

Exit criteria: the exact native assumptions to remove, every current duplicate
and its required disposition, and both performance controls are named before
refactoring.

#### Slice 0 completion record — 2026-07-14

The pre-refactor ownership lock now names all of the current exceptions rather
than allowing a later browser implementation to grow around them. It locks the
native-only `scenario_content` module, web capability reason, false WASM scene
effect, `PathBuf` standby request, native launch service, active/destination
operation separation, stable `WorldInstanceId` moving with the complete slot,
single unqualified TypeScript compiler map, and duplicate terrain constructor.
It also rejects scenario ids, fixture authorship, behavior policy, placement,
and activation constants in TypeScript.

The initial duplication ledger is:

| Resource/state | Current count with scenario | Disposition |
|---|---:|---|
| authoritative server/runtime, persistence identity, client replica, camera, traversal, upload coordinator, section buffers | two | inherently per world |
| browser integrated-server Worker | one today, two required | inherently per world once adopted |
| native server/worldgen/light and per-runtime compile/drop workers | one set per native runtime | server workers inherently per world; compiler execution is pending host-scoped sharing where practical |
| terrain atlas, sampler/bind ownership, shaders, layouts, compatible pipelines | two native copies | required shared; remove in Slice 2 |
| mutable terrain residency and GPU section buffers | two | inherently per world |
| browser compiler Worker, asset payload, SAB execution pool | one today | required singular host owner; qualify per-world queues in Slice 5 |
| managed content operation/service state | two independent native operations | operations inherently per world; lifecycle policy pending shared refactor |
| browser managed scenario records | none | pending thin IndexedDB adapter, never catalog rows |

All entries above are classified; no duplicate was accepted merely because the
native proof already owns it.

The controls were captured from already-built release/production artifacts on
an Apple M4 Pro with Chrome 150.0.7871.115. A first nominally idle preflight was
rejected because a repository-triggered Cloudflare deploy was still running.
The accepted preflight was 98.41% CPU idle with no Cargo, rustc, deploy,
Wrangler, or emulator work; postflight was 98.18% idle.

- Native 240-frame, 120 Hz release samples averaged 2.525, 2.455, 2.492,
  2.476, and 2.521 ms; P95 was 4.348, 4.309, 4.338, 4.187, and 4.350 ms.
  Medians were 2.492/4.338 ms. Average/P95 ranges were 2.85%/3.89%, with
  zero over-budget frames and zero accounting violations in every run.
- Production-browser movement samples reported compile averages of 13.0,
  12.6, 13.1, 11.7, and 13.2 ms and average maximum-frame-gap observations of
  8.4, 8.3, 8.3, 8.2, and 8.6 ms. The compile range was 12.8%, so this is a
  noisy characterization control rather than a narrow machine budget; the
  frame-gap range was 4.9%. All samples used one compiler Worker, one asset
  send, shared-result-buffer transport, and no generated fallback or overflow.
  No sample was discarded.

Native flat and stereo lobby smokes completed two switches. The inspected
receipts show the shared menu row, ground-level four-block table, live lobby
and island previews with depth/water ordering, and matching stereo eyes. The
new production-browser disabled smoke shows `ENTER LOBBY (UNAVAILABLE)` and
proves clicking it emits no action or session replacement. Updating that smoke
also corrected its old four-row title-menu coordinates for the shared five-row
menu.

Focused evidence:

```text
cargo test -p mclone-web-client --test scenario_parity_ownership_lock
cargo test -p mclone-app-runtime \
  lobby_scenario_is_a_shared_native_effect_and_reason_bearing_web_gap
pnpm native:web:lobby-scenario-disabled-smoke
pnpm native:lobby-scenario:smoke
pnpm native:lobby-scenario:stereo-smoke
```

Estimated effort: 0.5-1 day.

### Slice 1: Split Shared Scenario Content From Storage Execution

Refactor `mclone-app-runtime::scenario_content` into portable scenario
definitions/validation and platform executors.

Deliverables:

- Move manifests, ids, versions, expected roles, behavior profiles, and logical
  validation out of the native-only module.
- Define stable storage-neutral managed-world keys separately from stable live
  `WorldInstanceId`s.
- Keep the native filesystem/SQLite executor behind the same portable plan and
  preserve its staging, atomic publish, reuse, and corruption behavior.
- Build browser provisioning payloads from
  `authored_world_fixture_records`; add no TypeScript fixture builder.
- Add cross-target serialization/validation fixtures proving native and web
  consume the same manifest and record hashes.
- Keep native lobby launch and no-scenario construction byte/behavior stable.

Exit criteria: portable schema tests compile for WASM, native scenario tests
pass unchanged, and all filesystem types stay inside the native executor.

#### Slice 1 completion record — 2026-07-14

`mclone-app-runtime::scenario_content` now compiles on every target. It owns the
one lobby manifest, primary/destination roles, stable storage-neutral
`ManagedWorldKey`, logical validation errors, and shared provisioning payload.
The keys are `managed.lobby-preview-v1.lobby-v1` and
`managed.lobby-preview-v1.demo-island-v1`; neither is a live
`WorldInstanceId`, path, slot, or active/standby label.

The authored fixture record producer was made portable in `mclone-server`.
Both adapters therefore receive ordinary `ChunkRecord`s encoded by the shared
server persistence codec. The browser-facing ownership test consumes the same
Rust payload API and pins the same deterministic primary/destination receipts
as the app-runtime test: `8001097006086081343` and
`8764019107988679539`. No fixture id, block recipe, or serialization codec was
added to TypeScript.

The previous native module moved intact under `scenario_content::native` and
is re-exported only on non-WASM targets. All `fs`, `Path`, `PathBuf`, SQLite,
staging, rename, and background-thread types remain inside that executor. Its
versioned directory, JSON manifest shape, atomic publication, concurrent-winner
handling, corruption refusal, catalog exclusion, destination edit reuse, and
token cancellation tests remain green. The native lobby smoke still completes
five captures and two A-to-B-to-A switches. Ordinary construction still does
not call the payload generator or native executor.

Focused evidence:

```text
cargo test -p mclone-server
cargo test -p mclone-app-runtime
cargo test -p mclone-web-client --test scenario_parity_ownership_lock
cargo check -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:build
pnpm native:lobby-scenario:smoke
pnpm native:thin-adapters:purity
```

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

#### Slice 2 completion record — 2026-07-14

`TexturedSectionSharedResources` now owns the compatible direct-terrain
pipelines, per-view topology, texture layout, and GPU atlas behind one reference
counted resource owner. Each `TexturedSectionDrawResources` retains only its
queue handle, mutable section buffers/maps, traversal readiness, and culling
caches. The ordinary constructor creates exactly one shared owner and delegates
to the same mutable-store constructor; it does not create a standby resource or
add scenario polling.

Warm-world shell preparation clones the active slot's compatible resource
owner instead of compiling a second direct renderer or uploading a second
atlas. A managed lobby's replacement primary also adopts that prepared owner,
so replacing the pre-menu world cannot strand the standby as the sole owner.
The final lobby receipt reports two owners, an 8,388,608-byte shared base atlas,
and zero duplicated atlas bytes. Complete-slot exchange still contains no
renderer construction, materialization, compile, accept, or upload work.

The GPU characterization measured the original one-owner creation at 14.916 ms
for the 1024x2048 atlas and pipelines. Its five measured mip levels occupy
11,173,888 bytes. Constructing the second empty mutable store from that owner
measured below the timer's displayed 0.001 ms resolution, with no atlas bytes.
The product shell receipt was 0.0014 ms. Placed preview topology remains one
opt-in renderer owned by the one preview; it was not duplicated between slots.

Fresh flat, live-diorama activation, and synthetic-stereo captures were
inspected after the change. The table and miniature retain correct scale,
depth, water ordering, and matching eye composition; both A-to-B-to-A switches
retain ready first-uncovered terrain. The current macOS adapter lacks wgpu
`MULTIVIEW`, so full-frame materialization was source/test preserved but not
claimed as a capable-device receipt in this slice.

After a 98.10% idle preflight, the final five release direct-path samples
averaged 2.576, 2.519, 2.541, 2.508, and 2.494 ms; P95 was 4.408, 4.355,
4.402, 4.428, and 4.307 ms. Medians were 2.519/4.402 ms, 1.1%/1.5% above the
Slice 0 2.492/4.338 ms controls and below the 3% investigation threshold.
Within-batch average/P95 ranges were 3.3%/2.8%; every sample had zero
over-budget frames and zero accounting violations. Postflight was 98.36% idle.
No sample was rejected.

Focused evidence:

```text
cargo test -p mclone-render
cargo test -p mclone-scene
cargo test -p mclone-scene --test one_world_ownership_contract \
  empty_terrain_shell_reports_real_atlas_and_lazy_multiview_cost \
  -- --include-ignored --nocapture
cargo test -p mclone-web-client --test scenario_parity_ownership_lock
pnpm native:lobby-scenario:smoke
pnpm native:lobby-scenario:stereo-smoke
pnpm native:live-diorama:activation-smoke
pnpm native:web:build
pnpm native:thin-adapters:purity
```

Estimated effort: 1-2 days.

### Slice 3: Generalize Scenario And Slot-Targeted Startup

This is the largest shared-ownership refactor. Preserve native behavior before
adding browser execution.

Deliverables:

- Replace `PathBuf`-shaped scene requests with portable managed-world keys and
  start facts.
- Add tokened primary/destination start operations that complete into a named
  slot with a neutral `SceneSessionRuntime` aggregate while retaining the
  runtime's stable `WorldInstanceId` through later slot exchanges.
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

#### Slice 3 completion record — 2026-07-14

`ManagedScenarioLaunchState` now owns independent tokened primary and
destination provisioning operations and a separate tokened slot-start ledger
in portable scene code. Provisioning completions carry only deterministic
`ManagedWorldKey`s. Start requests carry a stable `WorldInstanceId`, role,
scene, descriptor, and optional destination presentation; neither contract
contains a filesystem path or derives identity from the slot that currently
owns the world.

`NativeManagedScenarioProvisionAdapter` is now the native rim around that
state machine. It retains key-to-path resolution, filesystem publication, and
background threads outside the shared contracts, then returns path-free
tokened completions. The direct diagnostic diorama also enters through an
explicit native adapter method instead of smuggling `PathBuf` through the
portable standby request.

External session completion is role-aware and installs its neutral runtime
aggregate into either the active managed-primary slot or the managed
destination slot. Native local startup and external-runtime startup converge
on the same prepared warm-slot installer and `Switchable` readiness policy.
The external path drains and acknowledges its own initial camera correction,
resolves terrain-relative placement, and advances through the ordinary bounded
compiler, accept, and upload path. Complete-slot activation, blink advancement,
preview retargeting, and stable instance identity through A-to-B-to-A exchange
no longer have false or no-op WASM policy substitutes.

No browser executor or second browser runtime was added in this slice. The web
menu remains explicitly unavailable until Slices 4-6 connect IndexedDB and
Worker adapters to these shared operations. Ordinary one-world construction
adds only inert ledgers, identifiers, and untaken branches; it performs no
scenario provisioning, renderer creation, runtime startup, or polling.

The native product smoke again cancelled a pending launch, relaunched, reused
managed content, completed five captures and two switches, denied mutation in
the lobby, and allowed mutation on the island. It reported two shared terrain
resource owners and zero duplicated atlas bytes. Flat lobby, return-island,
diagnostic activation, and synthetic-stereo captures were inspected; table
scale, shared-depth placement, water ordering, matching eye composition, and
ready first-uncovered destination terrain remain correct.

Focused evidence:

```text
cargo test -p mclone-app-runtime
cargo test -p mclone-scene
cargo test -p mclone-scene --test one_world_ownership_contract
cargo test -p mclone-server -p mclone-web-client
cargo check -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:lobby-scenario:smoke
pnpm native:lobby-scenario:stereo-smoke
pnpm native:live-diorama:activation-smoke
pnpm native:web:build
pnpm native:thin-adapters:purity
```

Estimated effort: 2-3 days.

### Slice 4: Provision Managed Scenarios In IndexedDB

Implement only the unavoidable browser storage adapter.

Deliverables:

- Upgrade the existing database schema with managed-scenario metadata without
  invalidating user catalog worlds.
- Use deterministic reserved world ids for lobby and island records; never add
  them to the user `worlds` catalog store.
- Commit each world's metadata, chunks, and entity chunks transactionally under
  its own operation token; primary publication never waits on destination
  publication.
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

#### Slice 4 completion record — 2026-07-14

The existing `mclone-web-worlds` database advanced from schema 2 to 3 by adding
one `managedWorlds` metadata store. Managed lobby and island chunks continue to
use the existing `chunks` and `entityChunks` stores under deterministic
`managed.*` world ids. They never receive `worlds` catalog rows, and the
ordinary title and Singleplayer paths do not open or populate managed content.
The ordinary user catalog migration and create/open/delete smoke remains green.

Shared Rust now owns the stored metadata contract and the exact
`missing`/`valid`/`partial`/`incompatible`/`corrupt` validation vocabulary. It
produces every fixture byte through the server persistence codec and validates
stored chunk/entity records by decoding them and checking their identities.
Valid mutable-world records may differ from the authored payload, so an edited
island is reused rather than regenerated. Corrupt records are refused rather
than silently overwritten; partial or version-incompatible records are replaced
transactionally.

The TypeScript adapter performs only generic IndexedDB mechanics. Each world is
published in its own transaction spanning metadata, chunks, and entity chunks.
First publication uses an exclusive metadata add, so concurrent duplicate
requests converge on one valid identity and the losing transaction can be
accepted only after shared revalidation. There is no staging key or combined
primary/destination barrier. An operation token is returned unchanged, and an
aborted short-lived Worker cannot publish a late completion.

The first direct measurement found that Rust materialization took 22-40 ms on
the browser main thread, which was too large for an uncovered frame. The final
adapter therefore initializes the same WASM policy and performs validation,
materialization, and IndexedDB work inside a short-lived provisioning Worker.
The final receipt measured 24-43 ms of fixture work inside Workers; IndexedDB
work ranged from 0-51 ms, including the intentionally losing concurrent
transaction. These are not rAF-thread costs. The main-thread submission receipt
was 0.155 ms at maximum in the final smoke.

The storage smoke proves cancellation after Worker creation leaves the primary
missing; concurrent primary requests publish/reuse one 49-chunk 927,561-byte
world; destination publication independently writes 49 chunks and 126,551
bytes; valid reuse preserves the complete byte digest; partial and incompatible
states repair; corrupt bytes remain corrupt after refusal; both worlds reopen
validly; and the user catalog count remains unchanged. The web menu remains
explicitly unavailable because no second runtime is started in this slice.

Focused evidence:

```text
cargo test -p mclone-app-runtime
cargo test -p mclone-web-client --test scenario_parity_ownership_lock
cargo check -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:typecheck
pnpm native:web:managed-scenario-storage-smoke
pnpm native:web:catalog-smoke
pnpm native:lobby-scenario:smoke
pnpm native:thin-adapters:purity
```

Estimated effort: 1-2 days.

### Slice 5: Start And Own Two Browser Runtime Workers

Connect provisioned managed keys to the shared slot-start service.

Deliverables:

- Extend `WebIntegratedServerRunnerConfig` and its Worker ABI with the shared
  world behavior profile and required authored-scene freeze settings.
- Start the protected lobby as primary and the mutable island as destination
  with independent Worker/runtime/persistence ownership.
- Qualify compiler requests, timings, results, cancellation, and asset epochs by
  stable `WorldInstanceId`, never current runtime role or slot.
- Install one host-scoped compiler broker/shared Worker pool with per-world
  queues/rings and active-world priority. Stop with evidence rather than create
  a second full pool if safe multiplexing fails.
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

Landed 2026-07-14. The browser adapter now consumes the same independently
tokened provision/start operations as native. An opaque WASM start ticket owns
Worker construction while awaiting, so destination startup does not borrow
`WebSceneHost` or pause the active animation-frame pump. The protected primary
and mutable destination use distinct IndexedDB keys, integrated-server Workers,
runtime services, cameras, replicas, and live `WorldInstanceId`s.

The one existing render-compiler Worker is now a host broker. Rust doorbells
carry stable instance id and active/standby priority; JavaScript assigns a
globally unique broker request id; the Worker forks independent mutable
snapshot mirrors from one parsed immutable asset template. Active requests are
selected ahead of queued standby requests. Runtime drop releases its compiler
namespace after already-submitted work drains. The accepted probe observed
three historical qualified ids (the replaced startup world plus primary and
destination), two live compiler sessions, and nine real local-request-id
collisions safely separated by the broker namespace. It still reported exactly
one compiler Worker, one WASM initialization, and one asset load.

The final browser receipt reached protected seed `17501` as active instance 2
and mutable seed `17502` as standby instance 3. Primary startup was playable;
the destination had 81 loaded chunks and its initial camera correction was
acknowledged. Worker instrumentation observed three integrated-server Workers
created (initial, primary, destination), exactly two live after replacement,
two short-lived provision Workers with zero retained, and one live compiler
Worker. Explicit shutdown reduced all instrumented Worker classes to zero with
no page error. Shared server tests retain the authoritative protected/mutable
mutation contract; Slice 4's reopen/digest proof retains persistence identity.

Browser cadence control remains the pre-existing typed gap:
`WebSceneRuntimeService::set_simulation_cadence` returns the explicit typed
worker-operation error, and the receipt records `standbyCadenceApplied=false`.
No ad-hoc Worker message was added. Scenario-on cost and the decision whether
to promote that typed operation remain measured work for Slices 6/8.

Making placed translucent composition execute in WASM also exposed one
unconditional `std::time::Instant`; it was replaced with the same target-aware
native/`Date.now()` timing shape already used by render code. The final capture
had 583 interior colors and 905,436 non-clear interior pixels; it was inspected
as a correctly playable flat authored lobby. Preview visual acceptance remains
Slice 6.

Focused evidence:

```text
cargo test -p mclone-server world_behavior_profile --lib
cargo test -p mclone-web-client --test scenario_parity_ownership_lock
cargo check -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:web:typecheck
pnpm native:web:managed-scenario-runtime-smoke
pnpm native:thin-adapters:purity
```

Estimated effort: 1.5-3 days.

### Slice 6: First Browser Lobby And Live Preview Pixels

This is the first required unattended rendered-output checkpoint.

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

Interactive and captured validation:

1. Open the production web app and choose `Enter Lobby` from the title.
2. Confirm control begins in the protected lobby before the island is ready.
3. Confirm the island appears on the four-block ground-level table without a
   visible frame hitch or page freeze.
4. Walk around the table and inspect shared-depth occlusion and water.
5. Attempt break/place in the lobby and confirm neither mutates.
6. Open ordinary Singleplayer afterward and confirm its startup remains normal.

Exit criteria: desktop and mobile browser show the same live scenario preview
as native through shared scene/render code, the agent records an unambiguous
visual/startup acceptance receipt, and no stop condition requires human input.

#### Slice 6 completion record — 2026-07-14

The production browser adapter now installs the complete managed provision,
role-targeted start, and completion boundary before it promotes the shared web
capability profile. Until that atomic assembly completes, the same profile
still reports the reason-bearing unsupported state. TypeScript does not choose
scenario ids, fixture coordinates, behavior, readiness, placement, or render
policy; the first title row dispatches the existing shared `Enter Lobby`
effect.

Both desktop and CPU-throttled mobile production smokes reach the protected
lobby while the destination is still warming, then render the fixed island at
shared 1:8 placement. The inspected sequence shows the real five-row title,
playable lobby, four-block brick warming table, and live grass/water island.
Shared depth correctly orders the table, island, water, lobby terrain, and a
foreground cow. There is no giant table, grass z-fighting, stale foreign
geometry, near-black frame, or transparent interior frame. Five consecutive
desktop preview captures were pixel-identical; desktop and mobile sequences
reported zero transparent interior pixels.

The accepted desktop receipt reached the lobby in 335 ms and the live preview
1,266 ms later. Its measured rAF maximum/P95 were 9.31/9.30 ms. The 2x
CPU-throttled mobile receipt reached the lobby in 337 ms and the preview 1,581
ms later with 9.31/9.22 ms maximum/P95. Both held two active integrated-server
Workers, one compiler Worker with one WASM/asset initialization and two live
world sessions, and no active provisioning Worker. Shutdown left zero active
scenario, server, or compiler Workers.

The destination retained 81 chunks and submitted two of three bounded sections
for 8,826 indices with zero out-of-region submissions. Its estimated mutable
GPU terrain was 270,664 bytes. Both slots referenced one 8,388,608-byte base
atlas owner with an owner count of two and zero duplicated atlas bytes. Direct
break and place attempts in the lobby hit terrain but were denied by the
authoritative world behavior without sending a mutation command.

Executing the composed frame with timing enabled exposed remaining native-only
`Instant` sites in the shared full-frame renderer. They now use the existing
target-aware timing clock. The WebGPU smoke also explicitly pauses the rAF loop
and renders one complete diagnostic frame before capture; this prevents a
screenshot from sampling the canvas between presentation states without
changing the product frame loop.

The five retained feature-off movement runs reported compile averages of 13.5,
13.5, 13.6, 13.6, and 14.3 ms, with average maximum-frame-gap observations of
8.3, 8.8, 8.3, 8.3, and 8.6 ms. Their 13.6 ms compile median was 4.6% above the
historical Slice 0 median and triggered the required investigation. An untouched
Slice 5 bundle was built separately and interleaved with the candidate. Candidate
averages were 13.9/14.1 ms versus control 14.0/13.7 ms: a 1.1% candidate mean
difference under the same drift, with effectively identical 8.4-8.6 ms frame
gaps. No sample was discarded. Source locks continue to prove the ordinary
path provisions no scenario and owns no standby resource.

Focused evidence:

```text
cargo test -p mclone-app-runtime \
  lobby_scenario_is_a_shared_effect_and_requires_complete_web_services
cargo test -p mclone-web-client --test scenario_parity_ownership_lock
pnpm native:web:typecheck
pnpm native:thin-adapters:purity
pnpm native:web:lobby-scenario-smoke
pnpm native:web:lobby-scenario-mobile-smoke
pnpm native:lobby-scenario:smoke
```

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

#### Slice 7 completion record — 2026-07-14

Production mouse right-click and touch `Use` now enter the same scene-owned
preview ray, blink, and complete-slot exchange. The lifecycle smoke completed
lobby A -> island B -> lobby A -> island B with five covered frames per
activation. Its first uncovered destination frame drew two sections with zero
compile submissions, compile accepts, section uploads, or renderer
materialization at the switch boundary. The native flat, synthetic-stereo, and
diagnostic A-to-B-to-A lanes retain the same invariant.

The browser broke island block `(0, 62, 1)`, waited for IndexedDB publication,
quit both runtimes, relaunched fresh runtime identities, and observed air at the
same coordinate. The protected lobby continued to deny break/place. A hidden
visibility transition advanced the shared background-save counter and the
visible transition resumed frame delivery. Managed worlds remained absent from
the catalog.

Late runtime completion is now guarded before installation by shared scene
policy. `ManagedScenarioLaunchState` checks the exact operation token, role,
and stable world-instance identity without consuming the later completion.
`complete_external_session_start` drops a runtime before either slot can be
mutated when that check fails. The browser adapter clears only its opaque
operation tickets and reports a stale-completion counter; it does not decide
whether a world is still admissible.

Deterministic browser lifecycle controls exercised all named races. Back while
both provisioning Workers were held left the original world untouched; a
repeated launch emitted no second scenario; destination Worker construction
failure left the protected lobby playable; and Quit with the destination start
held rejected that completion after release. Asset replacement and renderer
resource rebuild each cancelled a held destination, retained only the playable
lobby, then rejected the released completion. Those three late-start cases
advanced the stale counter from zero to three. The resource rebuild uses a new
host-neutral prepared-screen-effect entry, so browser recovery calls the same
scene rebuild implementation as native rather than maintaining a WASM copy.
The current smoke rebuilds on the live device; it is a resource-generation
recovery receipt, not a fabricated WebGPU device-loss claim.

The final browser run reported a 307 ms click-to-playable lobby and 1.237 s
from playable lobby to visible preview. Shutdown left zero active integrated
server, compiler, or managed-content Workers. Fresh island, return-lobby,
persisted-edit, asset-replacement, and resource-rebuild captures were inspected.
The Android arm64 release library and debug APK also built successfully through
the supported build script.

Focused evidence:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime
cargo test --manifest-path native/Cargo.toml -p mclone-scene
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm native:web:typecheck
pnpm native:web:lobby-scenario-lifecycle-smoke
pnpm native:lobby-scenario:smoke
pnpm native:lobby-scenario:stereo-smoke
pnpm native:live-diorama:activation-smoke
pnpm native:android:apk
pnpm native:thin-adapters:purity
```

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
   currently meet one unqualified JS timing/request map. Namespace by stable
   `WorldInstanceId`, use one broker/pool with per-world queues, and stop rather
   than duplicate the pool if bounded active-world priority cannot be proven.
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
- Stop if implementation would key persistent, compiler, promise, diagnostic,
  or lifecycle state by a changing active/standby role or physical slot.
- Stop if primary startup would be forced to await destination provisioning or
  if destination failure would invalidate an already valid primary lobby.
- Stop before a destructive IndexedDB migration or repair that could affect a
  user catalog world.
- Stop rather than add a second full compiler Worker pool, duplicate immutable
  compiler asset payloads, or introduce browser-only scenario policy.
- After Slice 6, continue on an unambiguous captured visual receipt; pause only
  if the result requires subjective product judgment or differs materially from
  the shared native scenario.
- Stop if browser resource limits would require changing the milestone from a
  live hosted destination to a static/baked preview or other fidelity tier.
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
