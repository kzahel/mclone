# 174: Warm World Hot Swap

Status: Slice 4 GPU-warm standby checkpoint ready for review 2026-07-13.
Slice 0's lower-level dual-integrated-host proof landed in commit `a31ac944`;
Slice 1's ownership audit, characterization locks, and one-world baselines are
recorded below. Slice 2 grouped the audited 13 per-world fields in one concrete
`DrawableWorldSlot`. Slice 3 adds stable world identity/storage/lifecycle facts,
one optional detached slot, a launch-only two-host harness, authoritative pose
acknowledgement, CPU seed retention, endpoint resolution, and explicit failure
diagnostics. Slice 4 converts that seed into conserved incremental upload work,
uses the active preparation path under hard standby caps, publishes exact
GPU/topology readiness, and reaches `Switchable` without drawing or selecting
the second world. Flat and synthetic-stereo two-host smokes pass. A
contemporaneous untouched-Slice-3 control clears the no-request performance
comparison after both binaries drifted above the older clean timing anchor.
Slices 5–7 are unimplemented. First-multiview-pipeline device timing remains a
named evidence gap because the current macOS adapter does not expose
`wgpu::Features::MULTIVIEW`; the capable-device path now materializes that
renderer before readiness, but still needs an XR/Windows receipt.

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

`native/crates/mclone-app-runtime/tests/dual_integrated_hosts.rs` proves
that two `NativeIntegratedServerRunner` → `IntegratedRunnerConnection` →
`SingleViewRuntime` stacks can coexist. The test uses different seeds and
disjoint chunk views, waits for both to become warm/idle, proves the replicas
do not cross-contaminate, changes logical active selection without rebuilding
either runtime, and shuts both down through ordinary ownership.

The lower-level runtime is instanceable; scene ownership is the remaining
product seam. Slice 2 replaces the 13 flattened per-world fields with exactly
one concrete `McloneSceneHost::active_world: DrawableWorldSlot`. Runtime,
camera, interaction/player state, draw/traversal/upload/Far-LOD state, render
statistics, admission policy, canonical scene options, and startup facts now
move as that direct aggregate. Physical presentation, shared renderer/content
resources, and global budgets remain host-owned.

Slice 3 now retains one `standby_world: Option<DrawableWorldSlot>` beside the
direct active slot. `--warm-world-standby-seed` constructs a second local
startup pump and empty terrain shell before presentation, advances it once per
scene frame, accepts its authoritative safe-surface pose into its own camera,
and retains its startup meshes as CPU data. It continues polling ordered
updates, but does not upload or draw standby terrain. A public diagnostic
snapshot reports phase, timing, memory, pose/endpoints, and failure state.

`StartedSceneRuntime` in `mclone-scene/src/session.rs` remains a useful partial
native seam: it already returns runtime, camera, draw resources, and render
stats before the caller installs them. Slice 2 adds the separate target-neutral
`DrawableWorldSlotInstall` aggregate and makes initial local/native,
provided-runtime, provided-scene-runtime, local completion, external/web
completion, and native replacement publish the same core cluster. Detached
standby GPU admission now reuses the same extracted preparation path as the
active slot. It remains deliberately invisible and unselectable until Slice 5
adds the atomic selection command.

## Slice 1 Architecture Checkpoint

This checkpoint changes no production behavior. It adds source-level
characterization locks for the flattened owner and records the behavioral and
performance receipts that Slice 2 must preserve.

### Complete Field Ownership Audit

`McloneSceneHost` has 81 mutable fields. The following is the complete primary
ownership classification; every field appears exactly once. "Shareable" means
one host-owned renderer may be reused serially by whichever world is selected.
It does not claim that its internal scratch buffers are immutable.

| Owner | Current fields |
|---|---|
| Per-world retained | `scene`, `runtime`, `local_startup`, `external_runtime_startup_pending`, `camera`, `interaction`, `player_model`, `draw`, `traversal_ready_sections`, `section_uploads`, `far_lod`, `render_stats`, `render_admission_policy` |
| Active-world transient, reset on selection | `underwater_effects`, `last_underwater_update`, `tracking_origin`, `prefetched_live_upload`, `last_locomotion_update`, `first_eye_summary`, `last_ui_panel_stats`, `last_ui_draw_cache_stats`, `rendered_frames` |
| Physical client/presentation | `services`, `session`, `session_runtime_factory`, `client_experience`, `initial_alignment_mode`, `render_options`, `player_collision_box_visible`, `crosshair_visible`, `travel_assist_mode`, `mono_ui_context`, `diagnostic_panel`, `ui`, `menu_overlay_cache`, `status_overlay`, `head_comfort`, `locomotion_mode`, `turn_policy`, `snap_turn_state`, `blink_teleport`, `mono_blink_debug`, `display_refresh_hz`, `per_view_uniform_frame`, `menu_toggle_down`, `game_ui_toggle_down`, `menu_pointer_down`, `gameplay_interaction_buttons`, `menu_panel_pose`, `menu_panel_anchor`, `menu_panel_recenter_pending`, `latest_controllers`, `seed_reroll` |
| Shareable GPU/content | `color_format`, `mesh_assets`, `active_assets`, `actors`, `selection_outline`, `world_gui_renderer`, `world_gui_overlay_renderer`, `mono_gui`, `sky`, `screen_effects` |
| Global content transaction/budget | `asset_replacement`, `asset_replacement_status`, `last_asset_replacement_commit`, `asset_replacement_started_at`, `asset_replacement_assets_ready_at`, `asset_pack_sources`, `asset_pack_preference`, `asset_pack_preference_storage`, `asset_pack_preference_error`, `pending_restored_asset_pack_selection`, `external_asset_pack_preparation`, `pending_external_asset_pack_selection`, `render_split_timing_enabled`, `defer_eye_waits_enabled`, `overlap_runtime_prefetch_enabled`, `render_section_upload_budget`, `render_section_accept_budget`, `render_completed_result_accept_budget` |

The extraction consequences are deliberate:

- `scene` is a mixed canonical startup DTO. For the mechanical one-world
  extraction, retain it whole with the slot rather than inventing a duplicate
  schema. Slice 3 copies the common presentation/settings values and varies
  only world identity, seed/generator, storage, and initial interest. Split the
  DTO only when two simultaneously retained values prove which fields differ.
- `interaction` is per-world because it owns inventory and the
  `ensure_has_sent_carried_item` synchronization fact, not merely a stateless
  ray cast.
- `draw` moves whole in Slice 2 even though it combines per-world section maps
  with a potentially shareable terrain renderer/atlas shell. Immutable shell
  sharing remains a later measured renderer refactor.
- `far_lod` is per-world: its GPU region map, visible-tile set, applied
  revision, and upload statistics are derived from one runtime.
- `render_admission_policy` is per-world because its adaptive telemetry is fed
  by that world's queues. The three explicit numerical budgets remain global
  host policy.
- actor instances are projected from the selected runtime every frame;
  `actors` is a host-owned renderer and content-keyed mesh cache. Sky and screen
  effects similarly consume selected-world inputs without retaining world
  geometry.
- the host `session` remains the user-facing start/selection workflow. Each
  slot gains its own descriptor/lifecycle fact; a standby must not replace the
  active session UI merely because it is starting.
- asset epoch/selection is one global transaction for this milestone. A live
  standby is cancelled before replacement commits, so no slot-local epoch
  transaction is introduced.

### Startup, Installation, And Replacement Inventory

There are six scene startup/installation shapes plus three resource/lifecycle
mutations that Slice 2 must preserve:

| Site | Target | Current installation shape |
|---|---|---|
| `start_local_async` | native | Builds an empty terrain shell, `runtime: None`, and a `SceneLocalStartup`; completion is deferred. |
| `with_runtime` | native | Calls `start_scene_runtime`, then installs all four fields from `StartedSceneRuntime`. |
| `with_scene_runtime` | native + wasm | Installs an already-started neutral runtime inline with an empty draw store and `external_runtime_startup_pending = true`. |
| `complete_external_session_start` | native + wasm | Replaces `scene`/runtime/camera/draw/stats inline and completes the session coordinator. |
| `complete_local_startup` | native | Drains the reconciled startup seed, uploads it directly into a new draw store, publishes traversal readiness, then installs fields inline. |
| `start_replacement_session` | native | Uses a runtime factory returning `StartedSceneRuntime`, then installs its runtime/camera/draw/stats. |
| `commit_asset_replacement` | native + wasm caller surface | Preserves the runtime/session/camera, replaces the runtime asset epoch and all content-bound renderers, then clears traversal/upload queues. |
| `rebuild_mono_render_resources` | native | Recreates render resources, clears draw/stream state, and marks all runtime sections dirty for a resource rebuild. |
| `teardown_world` | native + wasm | Drops startup/runtime ownership, creates an empty terrain store when needed, clears stats, and invokes the mixed transient reset. |

`StartedSceneRuntime` contains only `runtime`, `camera`, `draw`, and
`render_stats`; it is `cfg(not(target_arch = "wasm32"))`. Only `with_runtime`
and native replacement use it. The new Slice 2 staged aggregate must therefore
be target-neutral and must also carry the traversal/upload/Far-LOD/admission
facts that the four-field helper omits.

Both native startup helpers obtain `startup_sections`, pass the entire slice
directly to `TexturedSectionDrawResources::new`, then initialize render stats
from the resulting GPU store. They do not enqueue a
`RenderSectionCacheUpdate` or drain `RenderSectionUploadCoordinator`. Slice 4
therefore owns a real zero-compile-grant startup-seed conversion path, not a
call-site substitution.

### Lifecycle And Read-Site Trace

- `clear_transient_world_state` currently mixes slot-derived state
  (`underwater_effects`, upload/traversal/admission state) with physical
  presentation state (tracking origin, locomotion timing, comfort/blink,
  controller snapshots, eye/UI summaries, and frame count). Slice 2 must split
  the helper while retaining today's reset order.
- `flush_persistence` and `on_background` delegate only to the current
  `runtime`; remote runtime persistence is already a no-op. Multi-world
  lifecycle must iterate retained local slots explicitly, while normal slot
  drop continues to own runner/compiler shutdown.
- camera movement, pending authoritative correction/acknowledgement, pose
  commit, and interest update all pair `camera` with `runtime`. Neither member
  can be selected independently.
- block targeting and carried-item synchronization pair `interaction` with the
  selected runtime/client replica. Physical input edge latches remain
  host-owned and are cleared at selection boundaries.
- actor projection reads the selected client replica and camera, then submits
  transient `ActorInstance`s through the shared actor renderer. No retained
  actor instance list needs to move into the slot for this milestone.
- time, clear color, and sky darkening come from the selected runtime (or its
  slot's startup fallback); the shared `SkyRenderer` receives those values.
- underwater queries read the selected replica at the selected camera. Their
  smoothing accumulators are active-world presentation transients and restart
  on a handoff rather than leaking water state between seeds.
- Far LOD producer queues live in the runtime and the uploaded region/tile
  revision lives in `far_lod`; both are per-world. Diagnostics fold those facts
  into `render_stats` and the selected-world debug panel.
- UI/catalog/preferences remain physical product state. Active session labels,
  loading progress, debug seed/camera, actor counts, render counts, and queue
  depths must be projected from the selected slot; standby progress uses a
  separate concise diagnostic and never changes the active screen.

### Empty Terrain Renderer Shell

The explicit ignored GPU characterization test constructs
`TexturedSectionDrawResources::new(..., &[])` from the real default assets and
waits for queued device work. Even with zero sections it creates the ordinary
terrain shader, three render pipelines, per-view uniform storage/bind groups,
texture layout/sampler/bind group, generates the mip chain on CPU, creates the
GPU atlas, and issues one `queue.write_texture` per mip. Section vertex/index
buffers are the only part avoided by an empty slice.

On the 2026-07-13 macOS debug/optimized test lane:

```text
atlas                         1024 x 2048 RGBA8
base atlas bytes              8,388,608 (8.00 MiB)
five uploaded mip bytes       11,173,888 (10.66 MiB)
empty section count           0
flat shell + device wait      15.220-103.736 ms (warm/cold samples)
first multiview materialize   unavailable (adapter lacks MULTIVIEW)
```

The cache-sensitive timing range is characterization, not a stable budget. It
proves that a second empty store is startup work rather than background section
warmup. The test remains runnable as:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-scene \
  --test one_world_ownership_contract \
  empty_terrain_shell_reports_real_atlas_and_lazy_multiview_cost \
  -- --ignored --nocapture
```

Synthetic stereo on this Mac is the two-per-eye path and cannot substitute for
lazy multiview-pipeline evidence. Run the same first/steady empty multiview
measurement on a `MULTIVIEW` adapter before final XR closeout. Slice 4 now
materializes that renderer before publishing readiness on a capable device; no
switch frame may be the first materialization.

### Native Runtime And Cadence Cost

One local integrated scene with one render compiler worker owns six named Rust
threads in addition to the main/wgpu driver threads:

```text
mclone integrated server
mclone-worldgen
mclone-light-status
mclone-render-compile-dispatch
mclone-render-compile
mclone-chunk-drop
```

With lighting disabled, the light-status thread still exists but remained
parked and accepted zero jobs. After render distance 2 had settled, worldgen,
light-status, both compiler threads, and chunk-drop were parked; the integrated
server continued cadence ticks. A deliberately low 1 Hz render-loop probe with
scheduled fluid execution frozen sampled the whole debug process at about 7.4%
CPU for 20/20/60 and 1.1% for 10/10/10. A 60 Hz streaming comparison also
reduced average app frame work from 4.275 ms to 3.347 ms, but the default run
contained one unrelated 326.892 ms draw/device outlier.

These are directional single-run observations. They establish that
`set_simulation_cadence` is an effective sanctioned lever and that it does not
remove any of the six threads. Slice 4 retains the normal cadence because its
hard preparation caps held the measured active-frame contribution below one
millisecond; lowering cadence is not justified yet. If later background-cost
evidence does select it, Slice 5 must restore normal cadence before authority
handoff.

### Characterization Locks And Baseline Evidence

`one_world_ownership_contract.rs` now locks:

- the exact 81-field flattened inventory;
- the direct local startup seed/traversal/stats installation and its current
  upload-coordinator bypass;
- external and native replacement installation ordering;
- the native-only four-field `StartedSceneRuntime` seam;
- the mixed per-world/physical transient reset;
- asset replacement's epoch swap, streaming reset, and explicit
  session/camera/command/update preservation receipts.

Existing behavioral suites remain the stronger runtime receipts:

- `mclone-render-session` proves cached-section accounting, asset-epoch stale
  result retirement, and upload lifecycle conservation/supersession;
- `dual_integrated_hosts` proves two isolated local runtime stacks;
- the Mono and synthetic-stereo asset round trips prove an epoch replacement
  preserves the live session and camera while restoring drawable pixels.

Pre-refactor one-world samples on 2026-07-13:

```text
desktop offscreen
  2560x1600; 64 resident sections; 11 drawn; 2 drawn actors
timedemo (60 frames)
  average 2.520 ms; max 6.681 ms; 308.133 average drawn sections
120 Hz frame-budget smoke (60 frames)
  average 3.026 ms; p95 4.549 ms; max 5.285 ms
  0 over-budget frames; 0 accounting conservation violations
synthetic stereo
  640x640 per eye; 50 resident sections; 8 drawn
  249,679 differing eye pixels; UI composited in both eyes
Mono asset replacement
  epochs 0 -> 1 -> 2; restored pixel difference 0.393%
synthetic-stereo asset replacement
  epochs 0 -> 1 -> 2; replacement commit covered 96 sections
  249,679 differing eye pixels after restore; UI in both eyes
```

The production-equivalent clean `64505c01` and documentation-only `740fbcb4`
checkpoints also have desktop-flat release measurements from the same date.
They were recorded on an Apple M4 Pro MacBook Pro (`Mac16,7`, 14 CPU cores,
48 GiB memory) running macOS 26.5.1. These are headless engine measurements
rather than window-present FPS.

The initial clean single samples were:

```text
native:timedemo:perf (240 frames, release, 1280x720)
  scene build 2437.385 ms; render setup 2481.107 ms
  average 2.603 ms; max 5.482 ms
  664 loaded sections; 307.325 average drawn sections
native:frame-budget:perf (240 frames, release, 1280x720, 120 Hz)
  runtime setup 2477.601 ms
  average 2.329 ms; p95 3.884 ms; p99 4.076 ms; max 4.378 ms
  0 over-budget frames; 0 accounting conservation violations
```

A repeatability audit sampled the machine before each five-run batch. CPU was
85.9–94.4% idle with load average 2.1–2.3 on 14 cores, and no `cargo`, `rustc`,
Clang, linker, CMake, Ninja, or Xcode build process was active. Both probes ran
directly from the already-built release binary so compilation could not overlap
them.

```text
timedemo average frame ms
  runs     2.218, 3.400, 1.889, 2.884, 3.699
  median   2.884
  range    1.889-3.699 (1.810 ms; 62.8% of median)
  work     664 loaded; 307.325 average drawn in every run
frame-budget average frame ms
  runs     2.345, 2.317, 2.346, 2.374, 2.462
  median   2.346
  range    2.317-2.462 (0.145 ms; 6.2% of median)
frame-budget p95 frame ms
  runs     3.891, 3.844, 3.916, 3.943, 4.046
  median   3.916
  range    3.844-4.046 (0.202 ms; 5.2% of median)
frame-budget accounting
  0 conservation violations in all runs
  one isolated 9.765 ms maximum / over-budget frame in run 5
  other run maxima 4.148-4.467 ms
```

The timedemo workload is deterministic, but its frame timing is not repeatable
enough on this host to accept as a regression metric. Retain it for scene-build,
section-count, and draw-count characterization while investigating timing only
when a stable batch exists. The frame-budget average and p95 are the accepted
flat release anchors: their original single sample is within 1% of the five-run
median.

For every Slice 2-or-later change that touches scene ownership, frame polling,
upload, culling, or drawing, run five frame-budget release samples on the same
machine before accepting the checkpoint. Compare the median average/p95,
over-budget frames, accounting conservation, and work counts. A within-batch
average or p95 range greater than 10% of its median is unstable and cannot be
accepted; first recheck machine activity, then rerun or diagnose. A candidate
median more than 10% slower than the clean anchor is also an investigation
trigger rather than an automatic conclusion. Record every run: an isolated
maximum may be classified separately when average/p95 remain stable, but do not
silently delete it. Any new accounting violation or repeatable over-budget
behavior is a failure regardless of the median.

The desktop, ordinary stereo, Mono baseline/first-party/restored, and restored
stereo captures under `/tmp` were visually inspected. Terrain and actors were
drawable, the first-party middle frame visibly changed the material set, the
restored views returned to vanilla content, and the stereo capture retained
distinct eyes without a split-world frame. A maintainer also launched ordinary
desktop-flat play at `64505c01` and reported that basic single-world behavior
looked normal.

Checkpoint gates passed:

```text
mclone-scene                 102 passed; 0 failed; 1 GPU proof ignored
empty terrain GPU proof      passed explicitly; multiview unavailable
mclone-render-session        110 passed; 0 failed
dual_integrated_hosts        1 passed; 0 failed
native thin-adapter purity   passed
wasm32 web build             passed (pre-existing warnings only)
format + diff checks         passed
```

This is the review stop line. Do not begin Slice 2 until the field grouping and
the decision to keep `scene` canonical-but-slot-retained have maintainer
agreement. The first safe implementation after approval is a mechanical
one-slot extraction with no standby allocation or new frame branch.

## Scope And Non-Goals

This tactical includes:

- two local integrated worlds using different seeds;
- one active and one standby drawable slot;
- one shared asset-pack selection and render configuration;
- a launch-known second seed for the first smoke, allowing duplicate immutable
  renderer/material shells to be created before the first interactive frame;
- a provisional launch-only diagnostic activation and deterministic paired-gate
  placement contract;
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
- a world-creation checkbox, persisted portal/block/entity, catalog schema, or
  product startup preference for the smoke fixture;
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

### Provisional Smoke Activation And Gate Fixture

The first interactive proof is opt-in diagnostic behavior, not a property of a
saved world. Use this provisional activation contract:

- ordinary launch without a standby-seed request creates one world and no gate
  model, gate renderer, transition check, standby runtime, or extra worker
  threads;
- the desktop/offscreen harness accepts
  `--warm-world-standby-seed <i64>` alongside the ordinary active `--seed`;
- the platform adapter parses that diagnostic option once and passes a shared
  one-shot warm-world smoke request to `mclone-scene` before the first presented
  frame. Gate creation, readiness, placement, and switching policy remain
  shared; the app owns no duplicate policy;
- `pnpm native:warm-world-swap-smoke` supplies deterministic active/standby
  seeds and drives the automated lane;
- the request cannot add, replace, or recreate a world after presentation has
  begun.

The paired gate is a runtime scene fixture, not an authored block structure. It
is never written into either world's chunks or persistence root. Derive each
endpoint only after that slot has accepted its authoritative safe-surface spawn.
The provisional placement algorithm is:

1. start from the accepted feet pose and horizontal facing;
2. prefer a gate center six blocks forward, facing back toward the spawn-side
   approach;
3. anchor a three-block-wide, four-block-high opening to a walkable surface and
   require a clear approach/crossing volume on both sides;
4. if the preferred footprint is obstructed, search candidate surface columns
   in a deterministic expanding ring, initially bounded to 16 blocks;
5. if no candidate is valid, enter an explicit placement-failed state and keep
   switching disabled. Do not clear terrain, place a platform, or mutate saved
   blocks merely to make the diagnostic fixture fit.

Those distances are harness tuning constants, not durable portal gameplay
semantics. Captures and walking validation may adjust them without changing the
ownership architecture.

Gate availability is also explicit:

- `Disabled`: no smoke request, therefore no gate work at all;
- `Closed/Warming`: the opaque fixture may show progress, but the shared
  transition controller rejects locomotion across its volume;
- `Switchable`: the fixture remains visually opaque, but crossing performs the
  atomic slot handoff;
- `Failed/Cancelled`: crossing remains disabled and diagnostics explain why;
  dropping the standby may then remove the fixture entirely.

A future product may persist a portal link, expose a world-creation option, or
create destinations from a running lobby. None of those choices are implied by
this harness contract.

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

### Slice 1: Ownership Audit And Characterization — Checkpoint Ready

No production behavior change.

Evidence: `Slice 1 Architecture Checkpoint` above and
`mclone-scene/tests/one_world_ownership_contract.rs`. All architecture/code
exit criteria are met for maintainer review. The unavailable hardware
multiview timing does not block the one-world Slice 2 extraction, but remains a
hard pre-Slice-4 readiness gate.

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

### Slice 2: Extract One Concrete Drawable Slot — Checkpoint Ready

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

Checkpoint evidence (2026-07-13):

- `McloneSceneHost` now has 69 direct fields, including exactly one
  `active_world: DrawableWorldSlot`; the slot has exactly the 13 audited
  per-world fields. There is no standby field, collection, lookup, virtual
  dispatch, or selection branch.
- `DrawableWorldSlotInstall` is target-neutral and stages scene, runtime,
  startup state, external admission, camera, draw resources, and render stats
  before one coherent install. The native-only `StartedSceneRuntime` remains a
  leaf startup helper rather than the cross-target ownership boundary.
- All three initial construction paths and local, external/web, and native
  replacement completion install the same aggregate. Local startup deliberately
  retains its direct startup-seed upload; Slice 4 still owns the new budgeted
  seed-to-cache-update conversion.
- The old mixed reset is split into explicit physical-presentation and active
  world operations while preserving its existing physical-then-world call
  order. Asset replacement still preserves runtime/session/camera identity and
  clears the active slot's old-epoch stream state.
- `mclone-scene` passes 97 unit tests plus six ownership-contract tests (one GPU
  characterization test remains intentionally ignored); `mclone-render-session`
  passes 110 tests; the dual-integrated-host proof, web build, and native
  thin-adapter purity gate pass.
- Pre/post captures are byte-for-byte identical: desktop 2560×1600 with 64
  resident/11 drawn sections and two drawn actors; synthetic stereo 640×640 per
  eye with 42 resident/eight drawn sections, 249,679 differing eye pixels, and
  UI in both eyes. Both post-refactor captures were visually inspected.
- The release timedemo retains exactly 664 loaded and 307.325 average drawn
  sections. Its 3.697 ms average remains characterization only because Slice 1
  proved that timing unstable on this host.
- Five release frame-budget averages are 2.382, 2.430, 2.366, 2.392, and
  2.459 ms (median 2.392; 3.9% range). P95 values are 3.975, 4.027, 3.977,
  3.922, and 3.953 ms (median 3.975; 2.6% range). Those medians are 2.0% and
  1.5% above the clean 2.346/3.916 ms anchors. Every run retains 1,936 initial
  sections with zero over-budget frames and zero accounting violations; maxima
  are 4.331, 4.428, 4.340, 4.374, and 5.182 ms.
- Before the release batch, three CPU samples were 88.6%, 93.2%, and 94.8%
  idle. No Cargo, Rust, Clang, linker, CMake, Ninja, or Xcode build was active;
  the already-built release binary ran directly for all five samples.

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
- Add the provisional `--warm-world-standby-seed <i64>` harness option and
  project it once into a shared launch-only smoke request before presentation.
  With no request, construct neither the standby nor any gate state.
- For the first smoke, take the second seed from harness configuration at
  launch and create any duplicate immutable draw-resource shell before the
  first interactive frame. Report that extra startup time and memory separately
  from background world/section warming.
- Center the standby's first chunk view on its intended gate-entry X/Z region.
  Drain and acknowledge the resulting safe-surface `PlayerPosition` correction
  into the standby camera during warmup, follow corrected interest, and retain
  the accepted seed-dependent pose as the destination endpoint basis.
- Resolve and record both endpoint candidates using the provisional
  surface/clearance search. A placement failure is explicit standby diagnostic
  state and cannot become switchable; it never edits either world.
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
authoritative pose, and records a valid endpoint pair or explicit placement
failure. No switch is exposed.

Checkpoint evidence (2026-07-13):

- `McloneSceneHost` owns one direct active slot and one optional standby, not a
  registry. `DrawableWorldSlot` now has 20 fields: the original 13-field world
  cluster plus stable `WorldInstanceId`, descriptor, storage intent/root,
  lifecycle, asset epoch, accepted entry pose, and retained startup meshes.
  Every constructor initializes both standby options to `None`; without the
  CLI request there is no allocation, server, gate state, or poll work beyond
  the optional-state check.
- `--warm-world-standby-seed <i64>` is a desktop/offscreen launch harness and
  is rejected for remote sessions. The standby copies active render settings
  and asset epoch, uses transient storage, starts interest at the intended
  entry chunk, and never replaces the active session/UI/camera. Asset
  replacement and active teardown cancel/drop it before changing epochs or
  worlds.
- Startup advances one pump step per scene frame. It drains and acknowledges
  the initial `PlayerPosition`, follows corrected interest, observes a stable
  post-correction step, then converts the pump into a complete second runtime
  plus retained CPU startup meshes. Post-startup polling continues, while the
  standby draw store remains empty and no upload coordinator or draw path is
  entered.
- Progress and terminal state are exposed through
  `WarmWorldStandbySnapshot`; the existing flat debug pane adds warm phase,
  seed, loaded/mesh counts, elapsed/step/endpoint timing, and failure detail.
  The active session status and loading overlays are never replaced.
- Provisional endpoint resolution is deterministic, surface-relative, bounded
  to a 16-block ring, requires the documented 3×4 opening/approach clearance,
  and mutates no blocks. Seed `67890` produces a valid pair against active seed
  `12345`; standby seed `-98765` exercises explicit `placement-failed` state
  (`source=true`, `destination=false`) rather than becoming switchable.
- The first endpoint implementation exposed a 299.840 ms search. The cause was
  spatial probes allocating and unpacking an entire 4,096-block packed section
  per block lookup. `PackedChunkSection::block_state_id_at` now performs a
  direct allocation-free palette lookup and both client/app-runtime spatial
  accessors use it. The same representative flat smoke then measured 539.173 ms
  total standby warm time, 14.723 ms second-shell enqueue, 4,769,464 retained
  CPU seed bytes, 1.490 ms worst frame contribution, 0.102 ms worst startup
  step, 0.114 ms worst post-ready poll, and 1.441 ms endpoint-pair resolution.
- The synthetic-stereo two-host smoke measured 642.798 ms total, 15.597 ms
  shell enqueue, the same 64 seed/23 drawable sections and 4,769,464 CPU bytes,
  1.092 ms worst contribution, and 0.995 ms endpoint resolution. Its inspected
  640×640-per-eye capture retained 268,363 differing eye pixels, 166 active
  sections, 24 drawn sections, and UI in both eyes. The inspected flat standby
  capture retained ordinary active-world pixels, 96 active sections, 22 drawn
  sections, and two drawn actors; the standby itself remains invisible.
- The duplicate shell characterization reports a 1,024×2,048 atlas:
  8,388,608 base bytes and 11,173,888 bytes across five uploaded mip levels.
  A device-wait-inclusive standalone shell creation measured 129.528 ms; the
  already-initialized host's second-shell CPU enqueue is the 14–16 ms value
  above. Multiview shell materialization remains unavailable on this adapter;
  Slice 4 implements the capable-device path, but its hardware receipt is still
  open.
- The dual-host integration test now gives same-seed live hosts different
  SQLite roots, edits and flushes only the first, proves the second live replica
  is unchanged, reopens both roots, and proves only the first persisted the
  edit. The original different-seed/disjoint-view isolation test remains green.
- No-request desktop and synthetic-stereo captures retain the exact Slice 2
  counts (64/11 active resident/drawn sections and two actors; 42/8 stereo
  sections, 249,679 differing eye pixels, UI in both eyes) and were visually
  inspected. Web/WASM build, thin-adapter purity, focused/full tests, timedemo
  smoke, frame-budget smoke, and `git diff --check` pass.
- Before release measurement the host sampled 94.55% CPU idle with no active
  Cargo/Rust/compiler/linker build. Five direct-binary frame-budget averages
  were 2.388, 2.331, 2.387, 2.371, and 2.345 ms (median 2.371; 2.4% range).
  P95 values were 3.950, 3.895, 3.967, 3.955, and 3.971 ms (median 3.955;
  1.9% range). Those medians are 1.1% and 1.0% above the clean 2.346/3.916 ms
  anchors and slightly below Slice 2. All runs retained 1,936 initial sections
  with zero over-budget frames and zero accounting violations. The release
  timedemo retained 664 loaded and 307.325 average drawn sections at 3.971 ms;
  timing remains characterization-only on this host.

Exit result: Slice 3 is complete. Two scene-owned runtimes coexist and the
standby reaches CPU-ready with an acknowledged pose and endpoint pair (or an
explicit non-switchable placement failure). No standby terrain is GPU-resident,
no selection/switch/gate is exposed, and Slice 4 begins at CPU-seed conversion,
budgeted acceptance/upload, and topology-specific renderer materialization.

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

Checkpoint evidence (2026-07-13):

- `poll_runtime_and_upload` is now a small active policy wrapper around one
  slot-local `prepare_world_slot` implementation. The active policy preserves
  unlimited runtime polling, the host's existing upload/accept/completed-result
  budgets, adaptive admission, timing, and accounting. Only after that active
  call succeeds does the host offer remaining frame-deadline slack to the
  optional standby.
- The retained startup mesh vector becomes one ordinary
  `RenderSectionCacheUpdate`. Its exact vertex/index metadata is conserved and
  its accepted/submitted compile counts are zero. The upload coordinator drains
  one lifecycle item per eligible frame; a focused test covers populated and
  empty meshes and proves final totals with no held or released compile grant.
- Standby preparation has explicit 500 microsecond runtime-poll, one completed
  result, one compile request, one upload, one acceptance, and 750 microsecond
  work-deadline caps. It runs at most once per presented scene frame, skips when
  the active deadline has expired, and uses its own runtime/compiler pool. No
  shared worker-pool or immutable-renderer refactor is justified by this
  checkpoint's measurements.
- `WarmWorldReadiness` requires CPU/endpoints, startup-seed enqueue and complete
  initial drain, GPU-resident and traversal-ready support terrain below the
  admitted entry pose, and the renderer required by the device topology.
  Ordinary mono/per-eye pipelines are part of the pre-created shell. A device
  exposing `MULTIVIEW` now materializes the lazy terrain multiview renderer
  synchronously before the standby starts; `Switchable` cannot publish without
  it. The present macOS adapter cannot execute that capable-device branch, so
  its timing/visual receipt remains open for XR/Windows. The explicit GPU
  characterization test passes with a 16.185 ms flat empty shell and reports
  multiview unavailable.
- The diagnostic snapshot now exposes CPU seed bytes, initial lifecycle
  conservation and compile releases, queue sections/lifecycle/bytes/oldest age,
  accepted/released live compile work, GPU sections/vertices/indices, entry and
  topology readiness, total/GPU warm duration, advance counts, deadline skips,
  and last/worst GPU contribution. The flat debug panel exposes the compact
  phase, queue, coverage, and timing subset.
- Flat seed `12345` plus standby `67890` reached `Switchable` in 688.910 ms.
  The 64-item initial seed drained in exactly 64 GPU advances with zero compile
  releases; GPU warmup took 250.904 ms, left 23 sections/155,526 indices and no
  queued work at readiness, and measured a 0.670 ms worst GPU contribution.
  Entry coverage and topology readiness were both true. Its inspected
  1280x720 capture showed only the ordinary active world.
- Synthetic stereo reached readiness on GPU advance 64 and continued bounded
  streaming through advance 70. At capture it held 26 sections/158,130 indices,
  had 11 live lifecycle items queued, and retained true entry/topology
  readiness; the initial 64 items remained fully conserved with zero compile
  releases. Worst GPU contribution was 0.531 ms. The inspected 640x640-per-eye
  capture had 268,363 differing eye pixels, 166 active sections, 24 drawn
  sections, and UI in both eyes without standby leakage or a split-world frame.
- With no standby request, desktop and synthetic-stereo captures retain the
  Slice 3 deterministic counts: 64 resident/11 drawn sections and two actors;
  42 resident/eight drawn sections, 249,679 differing eye pixels, and UI in
  both eyes. Both were visually inspected. The full native package tests, web
  build, thin-adapter purity gate, focused conservation/ownership contracts,
  and `git diff --check` pass.
- The release timedemo retains exactly 664 loaded sections and 307.325 average
  drawn sections. Its 3.572 ms average remains characterization-only under the
  instability rule established in Slice 1.
- Before candidate timing, CPU samples were 86.26%, 92.65%, and 86.12% idle
  with no Cargo, Rust, compiler, linker, CMake, Ninja, or Xcode build active.
  Five direct release-binary averages were 2.709, 2.682, 2.571, 2.676, and
  2.643 ms (median 2.676; 5.2% range). P95 values were 4.560, 4.569, 4.509,
  4.638, and 4.602 ms (median 4.569; 2.8% range). All runs retained 1,936
  initial sections with zero over-budget frames and zero accounting violations.
- Those medians are 14.1%/16.7% above the older clean 2.346/3.916 ms anchors,
  so the required investigation built untouched Slice 3 commit `e7774e7c` in a
  separate worktree and ran five same-machine samples against the same assets.
  CPU was 90.78%, 96.89%, and 96.80% idle before the control. Its median
  average/p95 were 2.679/4.650 ms with 5.2%/7.8% ranges, zero over-budget
  frames, and zero accounting violations. Slice 4 is therefore 0.1% faster in
  median average and 1.7% faster in median p95 than the contemporaneous control;
  the older-anchor drift is environmental rather than attributable to this
  diff. Both complete five-run batches remain recorded under `/tmp`.

Exit criteria: standby reaches switchable GPU coverage while active frames
continue; a switch would require zero queued initial uploads for the admitted
entry coverage and no lazy terrain-pipeline creation for the current topology.

Exit result: Slice 4 is complete for the locally available flat and per-eye
stereo topologies. The retained standby is GPU-ready, but there is still no
selection command, slot swap, gate renderer, or simultaneous geometry. Slice 5
starts with atomic scripted A-to-B-to-A selection. Capable-device multiview
timing remains a named hardware receipt before final XR closeout.

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
  not acceptable. Keep the stateless renderer host-owned but construct it only
  for an active smoke request; keep endpoint/hysteresis state in the shared gate
  model. Materialize the required gate pipeline before its first measured
  interactive frame.
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
- Instantiate the runtime-only fixture from the endpoint candidates resolved in
  Slice 3. Follow the provisional dimensions/search contract and create no
  saved blocks, block entity, catalog record, or product startup preference.
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
- Test activation and fixture isolation: no flag means no standby/gate resource
  or per-frame transition work; the flag deterministically resolves paired
  endpoints; placement failure stays closed; neither success nor failure
  changes persisted chunk contents.
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

At each review checkpoint that touches the existing one-world hot path, also
run the release comparison pair against the clean Slice 1 evidence above. Run
the frame-budget command five times and use the documented stability rule;
timedemo timing remains characterization rather than a gate until it produces a
stable batch:

```bash
pnpm native:timedemo:perf
pnpm native:frame-budget:perf
```

Do not compare a release candidate against the shorter debug smoke numbers.

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
