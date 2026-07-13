# 174: Warm World Hot Swap

Status: proposed and architecture-review-reconciled 2026-07-13. Slice 0's
lower-level dual-integrated-host proof landed in commit `a31ac944`; Slices 1–7
are unimplemented.

Topic: `embedded-worlds`

Workstream: native Rust shared scene/runtime ownership in `mclone-scene`,
`mclone-app-runtime`, `mclone-render-session`, and `mclone-render`. Desktop flat
is the first interactive validation lane, followed by offscreen and synthetic
stereo through the same shared scene host. This is not a desktop-app feature.

## Milestone

Retain two complete local integrated worlds, including drawable GPU terrain,
while rendering exactly one. Warm the standby world incrementally under an
explicit background budget, then switch to it through an opaque world gate
without constructing a runtime, compiling a startup batch, or bulk-uploading
terrain on the switch frame. Keep the previous world ready for an immediate
return trip.

The milestone proves the practical value of multi-world ownership—smooth
Nether/sky/alternate-seed-style transitions—without taking on simultaneous
geometry composition:

```text
active world
  server + connection + replica + compiler + GPU terrain
       |
       | opaque gate / atomic selection
       v
standby world
  server + connection + replica + compiler + GPU terrain
```

Success means a user can walk through a shared world-space opaque gate, see the
other seed on the next presented frame, walk back, and encounter neither a
blank frame nor startup work charged to either switch.

## Current Evidence

`native/crates/mclone-app-runtime/tests/dual_integrated_hosts.rs` already proves
that two `NativeIntegratedServerRunner` → `IntegratedRunnerConnection` →
`SingleViewRuntime` stacks can coexist. The test uses different seeds and
disjoint chunk views, waits for both to become warm/idle, proves the replicas
do not cross-contaminate, changes logical active selection without rebuilding
either runtime, and shuts both down through ordinary ownership.

The remaining singleton assumption is scene ownership, not server/runtime
state. `McloneSceneHost` currently flattens one world's state into fields such
as:

- `runtime: Option<SceneSessionRuntime>`;
- `camera: EngineCameraController`;
- `draw: TexturedSectionDrawResources`;
- `traversal_ready_sections: TraversalReadySectionCache`;
- `section_uploads: RenderSectionUploadCoordinator`;
- `far_lod: FarTerrainLodRenderer`;
- startup/session/admission facts that currently replace the active world.

`StartedSceneRuntime` in `mclone-scene/src/session.rs` is a useful partial seam:
it already returns runtime, camera, draw resources, and render stats before the
caller installs them into the host. It is native-only and only some current
startup/install paths use it. Local and external/web completion still assign
flattened host fields directly. The eventual slot builder therefore must be a
target-neutral aggregate rather than an extension of this `cfg(not(wasm32))`
helper.

## Scope And Non-Goals

This tactical includes:

- two local integrated worlds using different seeds;
- one active and one standby drawable slot;
- one shared asset-pack selection and render configuration;
- a launch-known second seed for the first smoke, allowing duplicate immutable
  renderer/material shells to be created before the first interactive frame;
- independent transient storage first, then distinct persistent roots as an
  isolation test;
- background polling, compile acceptance, and GPU upload;
- an opaque shared world-space gate;
- paired entry poses and one-active-authority switching;
- desktop flat, offscreen, and synthetic-stereo validation;
- single-world and background-warm performance evidence.

This tactical does not include:

- creating a brand-new world after the first presented frame. The first smoke
  receives both seeds at launch; live lobby-driven world creation must first
  account for renderer-shell construction and its measured GPU cost;
- seeing the destination world through the gate;
- drawing two worlds in one frame;
- render-to-texture portals;
- world placement transforms, clip planes, stencil apertures, or cut caps;
- cross-world collision, lighting, translucency, entities, or block adjacency;
- simultaneous authority on two servers;
- inventory/health/entity transfer between unrelated hosts;
- remote observer subscriptions or remote-host federation;
- new Nether or sky generation algorithms. Different seeds exercise ownership;
  alternate generators become a later world-factory input once the slot exists;
- web, Android, or production UI adoption in this milestone. The owner remains
  shared so those platforms can adopt it without a second policy surface.

## Architectural Invariants

### Preserve Complete World Aggregates

Do not decouple a runtime from the render state derived from it. A drawable slot
must move or be selected as one unit. The exact audited field list lands in
Slice 1, but the target shape is approximately:

```rust
struct DrawableWorldSlot {
    id: WorldInstanceId,
    descriptor: ActiveSessionDescriptor,
    runtime: SceneSessionRuntime,
    camera: EngineCameraController,
    draw: TexturedSectionDrawResources,
    traversal: TraversalReadySectionCache,
    uploads: RenderSectionUploadCoordinator,
    far_lod: FarTerrainLodRenderer,
    render_stats: RenderStreamStats,
    readiness: WarmWorldReadiness,
}
```

Names are illustrative. Do not freeze this field list before Slice 1 classifies
asset epoch/replacement, admission policy, prefetched upload, startup admission,
underwater/effect state, interaction state, and session lifecycle facts.

Renderer resources that are stateless across worlds—actor pipelines, sky,
selection outline, GUI renderers, and screen-effect pipelines—should remain
host-owned unless evidence shows retained per-world mutable state. Prefer
duplicating uncertain state in the first correct slot over sharing it through a
new synchronization contract.

`TexturedSectionDrawResources` currently owns both per-world section state and
an uploaded atlas/pipeline/uniform renderer shell. Constructing a second empty
store is therefore not free: it duplicates immutable GPU resources before any
section upload. For the first launch-configured smoke, create that shell before
the interactive frame loop and report its startup/memory cost. Do not pretend it
is budgeted section warmup. At the current 4096×4096 atlas ceiling, base RGBA8
storage alone is about 67 MB (64 MiB) before mip overhead, so report actual
atlas dimensions and bytes rather than only a resource count.

The terrain multiview pipelines are created lazily on first XR multiview render,
so flat shell construction does not characterize the full XR cost. Measure flat
construction and first multiview materialization separately. Before an XR slot
can become switchable, explicitly materialize the terrain pipeline for the
topology that will present it; otherwise the first destination frame can still
compile a pipeline during the switch. If measured cost or later live lobby
creation requires it, split/share immutable terrain material/pipeline resources
in its own characterization-first slice; do not couple that refactor to slot
identity without evidence.

### Keep Leaf Types Single-World

`SceneSessionRuntime`, `SingleViewRuntime`, `EngineRenderSession`, client
replicas, render-section caches, and GPU terrain maps remain single-world leaf
types. Multi-world policy belongs in `mclone-scene` above them. Do not add
`WorldInstanceId` hashing to their per-section hot paths.

### Protect The One-World Fast Path

Ordinary play must remain a direct active-slot call:

```text
active.poll/sync/upload -> active.draw -> present
```

When no standby exists:

- allocate no registry or per-frame work list;
- start no extra server/compiler threads;
- do no background polling or uploads;
- evaluate no gate transition;
- add no world-id lookup to render traversal or draw loops;
- use the existing unmodified terrain shaders and render phases;
- preserve frame-pipeline accounting and screenshots.

A fixed field access such as `self.active_world.draw` is acceptable. A generic
N-world loop in the ordinary frame path is not required for this milestone.

### Account For Retained Runtime Cost

A second full native scene runtime currently brings its own integrated-server,
worldgen, light-status, render-compile dispatcher/worker, and deferred chunk-drop
threads. Disabling lighting prevents light jobs but does not remove the native
light-status worker. Treat roughly six or more OS threads per local scene as an
expected floor to verify, not a free background service.

There is no standby pause contract. The sanctioned background simulation lever
is the existing `SceneRuntimeService::set_simulation_cadence`; use it only with
measured policy and restore the active cadence on selection. A local chunk view
is sticky server state, so one initial `SetChunkView` retains interest without
synthetic keepalive traffic. The runtime must still be polled to drain ordered
updates and progress work.

## Refactor Risk Controls

- Land characterization before extraction and extraction before new behavior.
- Keep each commit or small series within one slice and use the exact
  `Topic: embedded-worlds` trailer.
- Move coherent fields mechanically; do not introduce traits merely to move a
  field across a struct boundary.
- Preserve the old one-active-world call graph while the slot count is one.
- Prefer whole-slot constructors and swaps over setters that can expose a
  partially installed runtime/draw pair.
- Partition `clear_transient_world_state` into audited per-world and physical
  presentation resets before using it during slot installation or selection;
  the current helper mixes both kinds of state.
- Keep the staged slot/install bundle target-neutral. Do not make the
  native-only `StartedSceneRuntime` the new architectural owner.
- Treat compiler queues, upload lifecycle conservation, asset epochs, and
  persistence roots as invariants with tests, not cleanup details.
- Do not share mutable per-world state to reduce memory until isolated ownership
  is measured and correct.
- Stop and split a new tactical if immutable GPU-resource sharing becomes a
  renderer architecture project rather than a narrow ownership extraction.

### One Physical Player Authority

Exactly one slot consumes physical movement and interactions and may reconcile
the physical camera. Standby worlds may retain their sticky chunk-view fact and
receive ordered server updates, but cannot correct the active camera.

The gate maps between paired endpoints rather than pretending unrelated worlds
share coordinates. On switch:

1. save the source slot's player/camera pose;
2. select the destination slot;
3. place its camera just beyond its paired gate endpoint;
4. commit that pose and interest through the normal runtime contract;
5. keep the source pose for the return trip.

The first smoke keeps inventory, health, entities, and persistence independent.

### Derive Entry Pose From Authoritative Spawn

The native integrated-runner configuration has no authored spawn-pose field,
and the current client protocol has no arbitrary client-to-server teleport. The
server derives its initial safe-surface spawn from the center of the first
`SetChunkView`, then refuses movement while its initial `PlayerPosition`
correction is awaiting acknowledgement. Standby startup must use that contract:

1. choose the intended gate-entry X/Z region before starting the standby;
2. make that region the standby's first chunk-view center so world generation,
   initial spawn, and warmup interest agree from the first command;
3. pump the standby until its initial `PlayerPosition` arrives;
4. apply it to the standby slot's camera through
   `apply_pending_engine_camera_position_updates`, including `AcceptTeleport`,
   corrected pose resync, and any resulting interest-center update;
5. derive the destination gate endpoint from that accepted safe-surface pose,
   including the seed-dependent Y, then pair it with the source endpoint.

This reconcile updates only the standby slot camera. It must never overwrite
the physically active camera while the active world remains selected. Do not
author paired endpoints at one absolute Y and assume unrelated seeds share a
surface height.

### Switch Only To GPU-Ready State

Runtime readiness alone is insufficient. `WarmWorldReadiness::Switchable` must
require at least:

- authoritative destination entry/view readiness;
- a client snapshot for the entry region;
- nonzero drawable traversal-ready coverage near the entry camera;
- initial seed meshes accepted into the standby upload lifecycle;
- no queued initial upload required to draw that coverage;
- the initial authoritative camera correction acknowledged and resynced, with no
  unresolved correction that would immediately invalidate entry interest;
- required terrain renderer pipelines already materialized for the active
  presentation topology, including multiview when that XR path is in use;
- an explicit failure/timeout state rather than an indefinitely passable gate.

The destination may continue streaming beyond the required entry coverage. It
does not need the entire requested render distance idle before switching.

## Implementation Slices

### Slice 0: Dual Runtime Ownership Proof — Landed

Evidence: commit `a31ac944`.

- [x] Start two native integrated-server runners concurrently.
- [x] Feed two independent connection adapters and client replicas.
- [x] Warm distinct seeds and chunk views.
- [x] Prove replica isolation and retained logical selection.
- [x] Add no production-frame branch or scene ownership change.

Validation:

```bash
cargo test --manifest-path native/Cargo.toml \
  -p mclone-app-runtime --test dual_integrated_hosts
```

### Slice 1: Ownership Audit And Characterization

No production behavior change.

Deliverables:

- Classify every mutable `McloneSceneHost` field as:
  - per-world retained state;
  - active-world transient state reset on selection;
  - physical client/presentation state;
  - immutable/shareable GPU/content resource;
  - global budget/scheduler state.
- Inventory all current startup/install/replacement sites and trace local,
  remote, external/web, and replacement assignments. Record which sites use
  `StartedSceneRuntime`, which assign inline, and which are available on wasm.
- Measure and classify what `TexturedSectionDrawResources::new(..., &[])`
  creates/uploads even with no section meshes: renderer pipelines, uniforms,
  atlas dimensions/bytes/mips, and any synchronous device/queue work. Measure
  first multiview-pipeline materialization separately from flat construction.
- Record the native thread topology and idle/background CPU floor of one full
  local scene, including the light-status worker when lighting is disabled.
  Characterize lower standby simulation cadence through the existing runtime
  control; do not design an unmeasured pause mechanism.
- Trace `clear_transient_world_state`, teardown, asset replacement, persistence
  flush, camera commit, interaction, actor projection, time/sky, underwater,
  Far LOD, diagnostics, and UI reads back to their world facts.
- Record the current `StartedSceneRuntime` and local-startup completion bundles.
- Record that local startup currently passes its complete render seed directly
  to `TexturedSectionDrawResources::new`; the budgeted upload coordinator does
  not yet accept that startup seed.
- Add characterization tests proving one-world startup/replacement preserves:
  descriptor, camera, asset epoch, cached/drawable section counts, traversal
  readiness, upload lifecycle conservation, and render stats.
- Capture a pre-refactor one-world baseline using the existing offscreen,
  timedemo/frame-budget, and synthetic-stereo lanes.

Exit criteria: every field moved in Slice 2 has a written owner and an existing
behavioral receipt. Ambiguous fields stay outside the slot until classified.

### Slice 2: Extract One Concrete Drawable Slot

Refactor only; retain exactly one world.

Deliverables:

- Introduce the concrete per-world aggregate from the Slice 1 audit.
- Move runtime/draw/traversal/upload/Far-LOD/camera facts atomically rather than
  adding independent optional fields.
- Make current frame methods operate directly on the single active slot.
- Make local, remote, external/web, and replacement startup construct/install
  the same aggregate without changing readiness or presentation behavior.
- Make the staged aggregate/builder compile on wasm even though multi-world web
  product adoption is out of scope. It must not depend on the native-only
  `StartedSceneRuntime` wrapper.
- Split the mixed `clear_transient_world_state` helper into characterized
  per-world reset and physical-presentation reset operations while preserving
  the exact existing one-world call sequence.
- Preserve asset replacement and surface/resource rebuild behavior.
- Keep `McloneSceneHost` as the shared product owner; add no app-local world
  manager and no `winit`, browser, Android, or OpenXR dependency.

Exit criteria:

- all current scene/session tests pass;
- native thin-adapter purity remains green;
- pre/post offscreen captures match within the existing deterministic contract;
- frame-accounting/timedemo evidence shows no significant one-world regression;
- no standby allocation, thread, poll, or render branch exists yet.

### Slice 3: Detached Standby Startup

Construct a second local world without replacing the active slot.

Deliverables:

- Generalize startup completion so it can return a detached
  `DrawableWorldSlot` or a staged slot builder rather than assigning host
  fields as a side effect.
- Retain the active session, camera, UI, and draw resources throughout standby
  startup.
- Give each slot a stable `WorldInstanceId`, descriptor, storage intent/root,
  and lifecycle state. Keep section/entity keys unqualified inside the slot.
- Add an `Option<DrawableWorldSlot>` standby owner; do not introduce an N-world
  registry yet.
- For the first smoke, take the second seed from harness configuration at
  launch and create any duplicate immutable draw-resource shell before the
  first interactive frame. Report that extra startup time and memory separately
  from background world/section warming.
- Center the standby's first chunk view on its intended gate-entry X/Z region.
  Drain and acknowledge the resulting safe-surface `PlayerPosition` correction
  into the standby camera during warmup, follow corrected interest, and retain
  the accepted seed-dependent pose as the destination endpoint basis.
- Send no synthetic standby keepalives: the first local chunk view is sticky.
  Continue polling the standby for ordered updates and startup progress.
- Start with the same active asset epoch/profile and render settings. For the
  first milestone, any asset replacement while a standby exists closes the
  gate and cancels/drops the standby before the active epoch commits. Do not
  attempt a dual-slot asset transaction, allow stale old-epoch uploads to
  survive, or rebuild the duplicate GPU shell inside the interactive loop. The
  launch-only smoke regains its standby on the next full session start; live
  standby reconstruction is a later milestone.
- Extend the integration proof to distinct persistent roots and verify edits or
  saves cannot cross-contaminate.
- Surface standby progress/failure diagnostics without replacing the active
  session status or opening a loading screen.

Exit criteria: two complete scene-owned runtimes coexist and active play
continues while standby reaches CPU startup readiness, acknowledges its initial
authoritative pose, and records an entry endpoint. No switch is exposed.

### Slice 4: Budgeted Standby Compile And GPU Warmup

Make the standby genuinely switchable without a switch-frame upload storm.

Deliverables:

- Split world preparation from drawing at the existing
  `poll_runtime_and_upload` seam. The active frame must call the same extracted
  logic with unchanged policy and accounting. Separate policy/context inputs
  currently read from the host (clock, scene descriptor/config, and budgets)
  from mutable slot state instead of turning the current large method into an
  opaque many-argument helper.
- Add an explicit startup-seed conversion path that represents the already
  compiled seed meshes as a `RenderSectionCacheUpdate`, with zero accepted
  compile jobs to release and accurate mesh/count metadata, then enqueue it in
  the standby's `RenderSectionUploadCoordinator` and drain it incrementally.
  This path does not exist today. Do not call
  `TexturedSectionDrawResources::new` with the entire standby startup seed on one
  active render frame.
- Test startup-seed lifecycle conservation: every queued mesh is applied or
  cancelled exactly once, no nonexistent compile grant is released, and the
  final section/index/face totals match the original seed.
- Do not count pre-created atlas/pipeline work as part of the incremental
  section-upload result. If the shell cannot be safely pre-created or its cost
  is unacceptable, pause here and extract/share the audited immutable terrain
  resources before continuing.
- Materialize any lazy terrain renderer required by the presentation topology
  before declaring the standby switchable. In particular, first XR multiview
  pipeline creation is warmup work, never switch-frame work.
- Give standby polling, completed-result acceptance, compile admission, and GPU
  uploads explicit caps. Start conservatively (for example one accepted/uploaded
  section per eligible frame) and make measured frame slack, not elapsed wall
  time alone, the escalation path.
- Keep active render-thread result acceptance and GPU upload ahead of standby
  work; standby cannot consume the active world's last shared render-thread
  budget. Compile dispatchers/pools are currently per-runtime, so their
  contention is OS-level CPU contention rather than competition for one compile
  grant. Bound that contention with worker counts and, if measurements require
  it, `set_simulation_cadence` rather than inventing a pause contract.
- Publish queue depth/bytes/age, drawable entry coverage, CPU mesh bytes, GPU
  section/index counts, warm duration, and worst active-frame contribution.
- Decide from evidence whether compiler worker pools and immutable atlas/
  pipeline resources must be shared now. Correct isolated ownership is the
  default; avoid premature shared queues.
- Define `WarmWorldReadiness` and lock the `Switchable` gate.

Exit criteria: standby reaches switchable GPU coverage while active frames
continue; a switch would require zero queued initial uploads for the admitted
entry coverage and no lazy terrain-pipeline creation for the current topology.

### Slice 5: Atomic Scripted Hot Swap

Prove the selection mechanism before adding world-gate visuals.

Deliverables:

- Add one shared scene command/effect for selecting the switchable standby.
  Platform adapters may trigger it, but may not implement selection policy.
- Select or swap the complete slot atomically at a frame boundary.
- Save source and restore/map destination camera poses through the existing
  camera reconcile and interest contracts.
- Route movement, interaction, time/sky, actors, effects, persistence flush,
  diagnostics, and active-session UI exclusively from the selected slot.
- Reset only audited active-world transient presentation state.
- Keep the previous slot as standby and support repeated A→B→A round trips.
- Restore the destination's active simulation cadence before it consumes
  movement, then apply the measured standby cadence to the retained source.
- Record switch frame index, wall/CPU time, GPU uploads, compile submissions,
  queue depths, camera corrections, and first drawable destination frame.
- Extend the existing `OffscreenScript` harness with a frame-advancing step and
  a dedicated CLI/package smoke hook. Its current one-shot camera/input actions
  do not advance and render a general walking/swap sequence. Slice 5 uses the
  hook to trigger selection at a deterministic frame; Slice 6 extends it to
  actual gate crossing.
- Add the offscreen scripted smoke and fail on blank output, runtime
  reconstruction, lazy renderer/pipeline creation, bulk switch-frame upload, or
  lost return-slot state.

Exit criteria: scripted swaps render the already-resident destination on the
next frame and round-trip without restarting either integrated host.

### Slice 6: Shared Opaque World Gate

Turn scripted selection into the interactive milestone.

Deliverables:

- Add a shared `WorldGate` model with paired endpoints, plane/volume geometry,
  normal, switch direction, readiness, and hysteresis state.
- Add a small shared `OpaqueWorldGateRenderer` in `mclone-render`; no existing
  renderer has the required opaque depth behavior. Its color target has no
  blending, its depth state uses the existing world depth format/compare and
  writes depth, and its quad/volume uses `cull_mode: None` so both faces render.
  Do not reuse `WorldGuiRenderer` (premultiplied-alpha with no depth
  attachment) or `SelectionOutlineRenderer` (alpha blend with depth writes
  disabled). Submit it in the world opaque phase before actors/translucency so
  its depth also occludes later phases behind the closed surface.
- Implement the gate renderer for ordinary mono/per-eye `render_in_slot` use
  and full-frame multiview, following the established per-view-slot plus lazy
  `RefCell<Option<MultiviewRenderer>>` pattern. A per-eye-only debug renderer is
  not acceptable. Keep the stateless renderer host-owned and the endpoint/
  hysteresis state in the shared gate model. Materialize the required gate
  pipeline before its first measured interactive frame.
- Place the destination endpoint from the standby's accepted pose recorded in
  Slice 3 and the source endpoint from the active slot's already accepted
  startup pose; do not assume unrelated seeds share coordinates or surface
  height.
- Use the mono eye or stereo eye midpoint for visual crossing and switch both
  XR eyes in the same frame. Never render one eye from each world.
- Use signed-distance enter/exit margins so head jitter cannot flap selection.
  Keep physical player/capsule authority separate from visual midpoint policy
  where XR locomotion requires it.
- Keep the gate visibly closed and non-passable until standby is switchable;
  show concise warm/failure state on the existing status/debug surface.
- Expose a harness-scoped desktop option for a second seed. Do not add a
  product startup field or duplicate policy in the desktop app.
- Validate desktop interaction first, then offscreen scripted crossing and
  synthetic stereo. Include occlusion cases proving nearer terrain hides the
  gate, the gate hides farther terrain, and both sides remain opaque. Capture
  and inspect the first drawable gate frame and both post-switch worlds under
  `/tmp`.

Exit criteria: a user can walk through the opaque gate A→B→A with no visible
blank frame, stereo split, reconstruction, or switch-frame upload burst.

### Slice 7: Performance And Lifecycle Closeout

Deliverables:

- Re-run the Slice 1 one-world baselines with standby disabled and report deltas
  in CPU frame stages, allocations if measurable, GPU submissions, section
  traversal/draw counts, and resident memory.
- Record active-frame impact while standby warms, total warm latency, CPU mesh
  queue bytes, GPU terrain bytes/counts, and switch-frame timing.
- Report thread counts by role, idle/background CPU at default and throttled
  standby cadence, flat renderer-shell creation, and first XR multiview
  materialization. Verify selecting the standby restores its active cadence.
- Test app pause/background lifecycle persistence flush for both retained local
  worlds without violating the rule that remote persistence is server-owned.
- Test standby failure, cancellation, replacement, app exit, surface rebuild,
  and device loss. No runner/compiler thread or GPU resource may outlive its
  slot.
- Add an
  `asset_replacement_during_standby_warm_cancels_without_mixing_epochs` test. It
  must prove the gate closes, the standby and its queued old-epoch uploads are
  cancelled before the active epoch commits, and no slot or renderer observes
  mixed epochs.
- Confirm the one-world path starts no standby services and remains the default
  on desktop, web, Android, and XR consumers.
- Run one real headset lane after synthetic stereo is green; retain screenshots,
  logs, and summaries under `/tmp`.
- Update `docs/topics/embedded-worlds.md`, `docs/native-engine-architecture.md`,
  and platform parity status with landed evidence and remaining composition
  work.

Exit criteria: the milestone is complete only with measured switch behavior,
background cost, memory cost, and no significant single-world regression.

## Validation Matrix

Per-slice focused gates:

```bash
cargo test --manifest-path native/Cargo.toml \
  -p mclone-app-runtime --test dual_integrated_hosts
cargo test --manifest-path native/Cargo.toml -p mclone-scene
pnpm native:thin-adapters:purity
git diff --check
```

Renderer/scene-ownership slices additionally run:

```bash
pnpm native:desktop-offscreen:smoke
pnpm native:timedemo:smoke
pnpm native:frame-budget:smoke
pnpm native:xr-emulation:smoke
pnpm native:web:build
```

Slice 5 must add a dedicated `pnpm native:warm-world-swap-smoke` wrapper and its
CLI hook over the extended frame-advancing `OffscreenScript` harness. Slice 6
extends the same lane from scripted selection to deterministic walking across
the gate; do not rely only on manual walking. The smoke must assert no renderer
or pipeline is lazily created on either switch frame.

Because the gate lane produces pixels, capture and inspect its nearer-terrain
occlusion case, first unobscured gate frame, A→B result, and B→A result before
proceeding. Use `/tmp`; never write captures into the repository. The wasm build
is also a source gate for the target-neutral slot builder even though web does
not expose multi-world product behavior in this milestone.

Final device validation follows `docs/platforms.md`. Batch the real desktop-XR
or Quest run after native/offscreen/synthetic-stereo gates are green.

## Stop Line Before Geometry Composition

Completing this tactical does not authorize a transparent or see-through
portal. The opaque gate renders one world at a time and exists specifically to
separate ownership/hot-swap work from multi-world rendering.

After closeout, reassess measured cost and ownership clarity. A later tactical
may build shared-coordinate composition from the retained `DrawableWorldSlot`
leaf, but must separately address placement transforms, transformed fog and CPU
culling, global render-phase/translucent ordering, clip/aperture policy,
boundary-aware meshing, and XR multiview correctness recorded in
`docs/topics/embedded-worlds.md`.
