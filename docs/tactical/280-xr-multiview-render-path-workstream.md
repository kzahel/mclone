# Tactical 280: XR Multiview Render-Path Workstream

Status: implementation and physical Quest/Linux-WiVRn comparison complete
2026-07-29. Multiview is retained as a live diagnostic mode; dual per-eye
remains the default. The parent remains open only for the physical Metal,
foveation, and lifecycle-recovery acceptance gaps owned by Tactical 281.

Parent: Tactical
[`261`](261-procedural-horizon-product-integration-roadmap.md), PH-8 stereo
and XR promotion.

Topics: `procedural-horizon-clipmap`, `performance`,
`graphics-video-settings`

## Originating Request

The optional full-frame XR multiview path exists, but it has not produced a
clear performance win and the newer procedural horizon does not render
through it. Keep the path testable instead of letting it silently rot:

- add procedural-horizon terrain and vegetation to full-frame multiview;
- expose a live XR setting so a headset user can switch between per-eye and
  multiview rendering without relaunching;
- use that setting for ordinary visual regression checks and controlled
  performance comparisons; and
- make an evidence-backed keep, promote, or remove decision without assuming
  that multiview must be faster.

This tactical is a coordination document. Child tacticals own bounded
implementation and evidence. No renderer, OpenXR, scene, or UI behavior
changes merely because this document exists.

## Why A Focused Parent Is Necessary

Tactical
[`278`](278-quest-procedural-horizon-multiview.md) correctly scopes the first
missing renderer work and Quest A/B measurement. A live setting is a separate
cross-boundary feature:

1. `mclone-terrain-view` needs real two-layer terrain and tree pipelines.
2. `mclone-scene` must compose that backdrop in the full-frame multiview pass.
3. the XR host must be able to change render-target topology at a safe frame
   boundary;
4. the shared UI needs a capability-gated typed setting and a request/result
   handshake with the host; and
5. the physical Quest comparison must distinguish renderer savings from
   target-topology changes and actor-population variance.

Putting all of that into Tactical 278 would mix a focused renderer experiment
with platform resource lifecycle and product-control policy. This parent owns
their order and combined acceptance. Tactical 278 remains the renderer and
Quest performance child.

## Expected Involvement

This is a medium cross-cutting renderer/platform slice, not a new XR renderer.
The likely shape is four to six focused commits across the shared terrain
renderer, scene composition, XR target abstractions, the Android XR adapter,
and shared UI/effect contracts, followed by physical Quest closeout.

The shader work is bounded because exact terrain, sky, actors, overlays, and
UI already provide working multiview patterns. The live control is the more
involved half because current target ownership is fixed at startup and the
preferred shared-array design may change per-eye frame-overlap behavior.

The critical path is:

```text
target feasibility
      |
      +--> horizon terrain + tree multiview --> scene composition
      |                                          |
      `--> safe target ownership ----------------+
                                                 |
                                                 v
                                      shared live XR control
                                                 |
                                                 v
                                      Quest regression + A/B
```

## Executive Direction

The planned control is:

```text
Graphics
  XR Render Path: Per-eye | Multiview (Experimental)
```

The row is visible only in an XR capability profile. On an XR host without
full-frame multiview support it remains read-only as
`Per-eye (Multiview unavailable)`. Flat clients must not gain an inert XR
option.

The initial policy is:

- per-eye remains the safe default;
- `--xr-full-frame-multiview` remains a launch-only override and automation
  input;
- a live menu selection applies at the next safe frame boundary;
- the choice is transient and is not added to
  `ClientGraphicsPreferences` yet;
- failure leaves the last working path active and reports the rejection; and
- neutral performance does not by itself remove the setting. A correct,
  low-maintenance path with negligible inactive cost is useful as an
  interactive regression lane.

Persistence is deliberately deferred. A stored experimental path could make
an affected headset relaunch into a black or failing renderer before the user
can recover through the menu. Persistence may be reconsidered only after the
live path, fallback, and physical-device evidence are accepted.

## Current Code Facts

These are the implementation facts observed on 2026-07-28.

### The existing multiview path is real but startup-only

- Android XR parses `--xr-full-frame-multiview` into a startup boolean.
- startup branches before the main frame loop:
  - per-eye creates two independent OpenXR eye swapchains; or
  - multiview creates one two-layer stereo-array swapchain and one
    `ChunkMultiviewDepthTarget`.
- `AndroidXrFrameTargets<'a>` then borrows exactly one of those target
  families for the lifetime of `AndroidXrMainFrameLoop`.
- `AndroidXrRenderPath` is consequently fixed when the loop is constructed.
- the per-eye variants also carry the accepted frame-overlap,
  eye-submit-overlap, and runtime-prefetch modes.

This makes a menu row easy to draw but impossible to honor correctly without
refactoring target ownership.

### Full-frame multiview already covers most of the scene

The multiview scene path already renders:

- sky;
- exact chunks, including placed/composed exact terrain;
- opaque world gates;
- prepared actors;
- translucent terrain ordering;
- underwater and comfort-fade effects;
- world diagnostics and selection outlines; and
- XR menu/world UI.

Those renderers provide the working shader, uniform, array-layer, reversed-Z,
and pass-order patterns. This workstream extends an established render path;
it does not invent multiview from scratch.

### The procedural horizon is the missing world layer

The normal per-eye path:

1. prepares one shared terrain-view presentation;
2. renders exact opaque/cutout chunks for each eye;
3. renders `SceneTerrainViewState` into each eye with that eye's physical
   `ChunkRenderView`; and
4. continues with actors and translucent work.

The full-frame multiview path prepares exact records and immediately enters
the exact multiview draw. It never prepares or encodes `SceneTerrainViewState`.
The terrain and tree pipelines in `mclone-terrain-view` are created with
`multiview: None`.

The absent pixels are therefore a renderer and scene-composition gap, not a
missing procedural LOD residency system.

### The current targets expose only one view shape each

- `XrAcquiredStereoTarget` exposes a `D2Array` color view.
- `ChunkMultiviewDepthTarget` exposes a `D2Array` depth view.
- neither currently exposes layer-zero and layer-one `D2` views suitable for
  ordinary per-eye passes.

That is the narrow target abstraction needed by the preferred live-switch
design.

## Terminology Boundary

“LOD” in this workstream means the current fixed-budget procedural horizon
owned by `mclone-terrain-view`. It does not mean the retired chunk-granular
Far LOD system removed by Tactical
[`245`](245-retire-chunk-far-lod-runtime.md).

Do not restore the retired renderer, residency, setting, worker, or
compatibility types while adding multiview.

## Required Product Contract

When complete, a capable XR session must support this sequence:

```text
launch in safe per-eye mode
        |
        v
open shared Graphics page
        |
        v
select Multiview (Experimental)
        |
        v
finish/release current frame -> switch at boundary -> render next frame
        |
        v
inspect ordinary exact + horizon + actors + overlays in both layers
        |
        v
select Per-eye -> switch back without world/session restart
```

The active world, committed horizon presentation, exact coverage generation,
controller state, menu state, and gameplay session remain live across the
switch. Only view encoding and target access change.

The control must distinguish:

- **requested path** — the current UI selection;
- **active path** — the path which produced the last completed frame;
- **capability** — whether this graphics/session combination can use
  full-frame multiview; and
- **transition result** — idle, pending, accepted, or rejected with a bounded
  reason.

The menu must never claim `Multiview` while the host has silently fallen back
to per-eye.

## Ownership Contract

Keep the established shared-first and XR boundaries:

- `mclone-ui` owns the neutral `Per-eye | Multiview` value, row, capability
  projection, and typed action.
- an existing shared scene/app-runtime effect boundary owns the
  request/result handshake. The implementation child must audit the current
  effect seam before selecting its exact type location.
- `mclone-scene` owns one committed world/horizon frame decision and composes
  the selected render path. It must not create or submit OpenXR swapchains and
  must not gain an OpenXR dependency.
- `mclone-terrain-view` owns procedural terrain/tree pipelines, shared
  residency, exact-painted masking, conservative stereo visibility, and
  renderer-neutral prepared draws.
- `mclone-render-session` and `mclone-render` own reusable render-target and
  GPU pipeline contracts where the implementation needs them.
- `mclone-xr-host` owns OpenXR acquire/wait/release/projection submission
  mechanics and the safe transition point.
- `mclone-xr-graphics` owns Vulkan/wgpu capability evidence and concrete
  OpenXR texture/swapchain views.
- the Android XR app wires concrete capabilities, targets, and effects. It
  does not decide UI policy, horizon semantics, or shader behavior.

`OpenXrFrameDriver` remains the exclusive owner of OpenXR
poll/wait/begin/render-or-skip/end sequencing. A live path switch must be an
effect consumed between completed frames, not a second frame loop hidden in
the app.

## Render-Target Topology Decision

### Preferred design: one stereo-array swapchain for both paths

The preferred steady-state design creates one two-layer OpenXR color
swapchain and uses it in either encoding mode:

```text
one acquired stereo-array image
       |
       +--> layer-0 D2 color/depth views --> left per-eye pass
       |
       +--> layer-1 D2 color/depth views --> right per-eye pass
       |
       `--> D2Array color/depth views ----> one multiview pass
```

Required target changes:

- extend `XrAcquiredStereoTarget` to expose non-overlapping layer-zero and
  layer-one `D2` color views as well as the existing `D2Array` view;
- extend the two-layer depth owner to expose matching `D2` layer views and
  its `D2Array` view;
- generalize stereo-array projection submission so its name and contract
  describe the OpenXR layout, not whether the image was produced by two
  per-eye passes or one multiview pass; and
- acquire and release the stereo-array image once per rendered frame in both
  modes.

This avoids two permanently resident target families and makes a live switch
an encoding decision rather than swapchain destruction/recreation.

### Mandatory feasibility gate

The single-array design changes the current per-eye target topology. In
particular, two independent eye swapchains can be submitted/released
independently, while one array image cannot release one layer early. The
accepted frame-overlap path therefore cannot be assumed unchanged.

Before adopting the design:

1. prove wgpu can create and render through both `D2` layer views of every
   acquired OpenXR array texture;
2. prove both OpenXR projection views submit the correct array indices;
3. prove the foveation profile applies correctly to both array layers;
4. compare original dual-swapchain per-eye and array-backed per-eye pixels;
5. compare their Quest stationary and moving frame behavior; and
6. account for any overlap mode which becomes ineffective or changes
   meaning.

The original per-eye path is the protected baseline, not disposable
scaffolding for multiview.

### Fallback if array-backed per-eye regresses

If the feasibility gate fails, introduce an owned XR target manager which can
transition between the original dual-eye family and the stereo-array family
only at a completed-frame boundary. Prefer one steady-state family at a time.
Creation failure must leave the old family active.

Permanently holding both families is the last resort. At a representative
Quest eye extent of `1680 x 1760`, an RGBA8 triple-buffered two-layer color
family is about `67.7 MiB` and a two-layer D32 depth target is about
`22.6 MiB`. A second complete family therefore costs roughly `90 MiB` before
driver bookkeeping and foveation resources. Do not accept that inactive cost
without physical memory-pressure evidence.

## Procedural-Horizon Multiview Contract

Tactical 278 owns implementation and detailed evidence for this section.

- One requested/staged/committed clipmap update is shared by both eyes.
- One exact-painted coverage snapshot and generation are shared by both
  layers.
- Procedural ground, water, skirts, and tree proxies all use pipelines with
  `multiview: NonZeroU32::new(2)`.
- Vertex projection selects the correct physical-eye view/projection through
  the multiview layer/view index.
- Stereo view data is immutable for the complete submission. Do not rewrite
  one mutable per-eye uniform between layers.
- Tile and tree visibility conservatively admits the union of both eye
  frusta. It may not omit geometry visible to either eye.
- Existing covered-tile rejection remains active.
- Exact-painted discard, biome/material color, water, fog, lighting, and
  proxy ownership remain pixel-equivalent to the per-eye path.
- The backdrop loads the exact opaque/cutout depth, then actors and
  translucent exact terrain retain their accepted ordering.
- Both layers use their own physical projection with the shared reversed-Z
  contract. A mono camera matrix duplicated into both layers is a failure.
- Mono, browser, and normal per-eye pipelines remain available and protected.

Terrain and tree work must land together. A multiview mode with distant
ground but missing proxy forests is not a complete horizon renderer.

## Live Control Contract

The implementation child for the live-control slice must define a neutral
handshake equivalent to:

```text
XR capability/status -> shared UI render state
shared typed action  -> pending render-path request
pending request      -> XR host at a completed-frame boundary
host result          -> active status shown by the same UI
```

Exact type names may follow the existing settings/effect architecture, but
these invariants are binding:

- no stringly typed command crosses the shared boundary;
- no OpenXR type enters `mclone-ui`, `mclone-scene`, or generic app runtime;
- a request is applied no earlier than the frame after it is accepted;
- requests coalesce safely if the user changes the selection twice before a
  boundary;
- session stopping, loss pending, pause, or target recreation either cancels
  the pending request or replays one normalized request after recovery;
- failure restores the last active selection and publishes a bounded status;
- the CLI flag chooses the initial requested path but is never written to
  preferences; and
- automation can select and observe both paths without scraping menu text.

The first version may eagerly materialize both procedural pipeline families
on a multiview-capable device if the cost is bounded. It must not eagerly
create a second OpenXR swapchain family merely to make the menu switch appear
instant.

## Diagnostics

Add stable observations sufficient to tell what was requested, rendered, and
measured:

- multiview capability and rejection reason;
- requested and active XR render paths;
- pending state and completed/failed switch counts;
- active target topology (`dual-eye` or `stereo-array`);
- array image count, layer count, extent, formats, and estimated owned bytes;
- active frame-overlap/prefetch behavior;
- horizon active/ready state and committed generation;
- resident, visible-union, covered-rejected, and drawn terrain tile counts;
- resident and drawn proxy-tree counts;
- left/right non-empty horizon evidence;
- render-stage CPU timing for horizon preparation, terrain, vegetation,
  exact terrain, actors, overlays, submit, and wait; and
- existing app-work, thread-CPU, headroom, over-period, cadence, and Meta GPU
  counters.

Diagnostics must report the active renderer rather than merely echoing the
launch flag or menu preference.

## Work Breakdown

### Slice 0: baseline and target feasibility

- [x] Pin the current dual-eye per-eye and startup multiview source contracts.
- [x] Record current target image/layer/format/byte facts.
- [x] Add an isolated stereo-array per-layer render/submission proof.
- [x] Compare original and array-backed per-eye pixels and Quest cadence.
- [x] Select single-array or bounded target-manager topology from evidence.

This slice must finish before the UI promises a live switch.

### Slice 1: procedural terrain and vegetation multiview

Owner: child Tactical
[`278`](278-quest-procedural-horizon-multiview.md).

- [x] Add immutable two-view terrain uniforms and shaders.
- [x] Add the two-layer terrain pipeline and union visibility.
- [x] Add matching tree-proxy uniforms, shader, pipeline, and union
  visibility.
- [x] Preserve coverage, fog, material, water, skirt, and reversed-Z
  semantics.
- [ ] Add two-layer synthetic/readback evidence.

The existing startup flag is sufficient for this first renderer milestone.

### Slice 2: scene composition

- [x] Prepare the terrain-view presentation once for a multiview scene frame.
- [x] Encode the horizon after exact opaque/cutout depth and before actors /
  translucent work.
- [x] Preserve the exact/proxy tree ownership contract.
- [x] Account for horizon timings and stats in the multiview frame summary.
- [ ] Cover device/resource rebuild and active asset-epoch replacement.

### Slice 3: live target manager and shared setting

Open a bounded child tactical before implementation so target lifecycle and UI
effects do not turn Tactical 278 into an app-wide execution record.

- [x] Refactor borrowed startup-only target selection into the accepted
  safe-boundary target contract.
- [x] Add the neutral value, capability/status projection, and typed UI
  action.
- [x] Add the XR-only Graphics row.
- [x] Add the request/result handshake and next-frame application.
- [x] Keep per-eye as default and retain the CLI initial override.
- [x] Add failure fallback and automated `per-eye -> multiview -> per-eye`
  switching.

### Slice 4: regression and physical Quest acceptance

- [ ] Inspect matched low-angle terrain/tree captures in both layers.
- [x] Switch both directions repeatedly in one live world and menu session.
- [ ] Exercise pause/resume and OpenXR stop/ready recovery.
- [ ] Alternate per-eye/multiview/per-eye stationary and settled-orbit rows
  with a fixed normal actor population.
- [ ] Repeat actor-skipped rows only as attribution controls.
- [x] Re-run the selected comparison after any target-topology refactor.
- [x] Record inactive resource cost and memory behavior.

### Slice 5: decision and closeout

- [x] Choose default, experimental opt-in, diagnostic-only, or removal from
  the decision matrix below.
- [x] Update Tactical 261 PH-8 and the living horizon/performance/settings
  topics.
- [x] Update the platform validation matrix and CLI documentation.
- [x] Close or explicitly assign every remaining renderer, host, or device
  gap.

## Validation Matrix

### Automated renderer gates

1. WGSL/pipeline compilation on a `wgpu::Features::MULTIVIEW` adapter.
2. Two distinct physical-eye matrices produce distinct, non-empty array
   layers.
3. Both layers contain procedural terrain and proxy vegetation.
4. Exact coverage removes the same stable procedural ownership in per-eye and
   multiview paths.
5. Reversed-Z ordering preserves exact foreground, procedural backdrop,
   actors, water, and translucent terrain.
6. Union culling contains the independent left and right visible sets.
7. Mono/per-eye snapshots remain unchanged where byte equality is practical.

Save captures under `/tmp` and inspect them at the first drawable milestone,
in accordance with the repository rendered-output policy.

### Shared UI and lifecycle gates

1. Flat capability profiles do not expose an actionable XR row.
2. Unsupported XR profiles cannot request multiview.
3. Capable profiles expose exactly the two intended values.
4. A request changes the active path only after a safe frame boundary.
5. `Per-eye -> Multiview -> Per-eye` completes without world/session restart,
   stale images, double release, or one-eye output.
6. rapid repeated selection coalesces to the final legal request.
7. a forced target/materialization failure leaves the old path active and the
   UI truthful.
8. pause/resume and session stop/ready retain a safe normalized state.
9. relaunch without a CLI override returns to per-eye while an explicit CLI
   override selects the initial multiview request.

### Platform gates

- shared/native tests for the new neutral value and effect handshake;
- native offscreen two-layer capture on a capable adapter;
- desktop XR compile and unsupported/capability projection as applicable;
- `pnpm native:android-xr:apk`;
- scripted Android XR validation for both startup selections and live
  switches; and
- physical Quest 3 pixels, switching, cadence, memory, and recovery.

Desktop OpenXR does not claim multiview merely because the shared renderer
exists. Each host must publish real graphics/runtime capability and provide
the accepted target contract.

### Performance comparison

Use Tactical 278's alternating comparison and Tactical 277's accepted RD5
scene:

- per-eye / multiview / per-eye;
- stationary;
- settled orbit with a fixed normal actor population;
- actor-skipped settled orbit as attribution only; and
- the same quality, render distance, world, camera path, foveation, refresh
  rate, render scale, and horizon readiness.

Compare app-work p50/p95/p99, thread-CPU p50/p95/p99, over-period frames,
headroom, app GPU, cadence, render-stage timings, target bytes, actors, exact
draws, horizon tiles, and proxy trees.

Do not credit multiview for a different actor count, lower LOD quality,
shorter projection, altered foveation, or a changed per-eye target baseline.

## Decision Matrix

| Result | Disposition |
|---|---|
| Material moving-tail improvement, correct pixels, stable lifecycle | Retain the live setting; consider a later default change only after broader runtime/device support. |
| Neutral performance, correct pixels, low maintenance, negligible inactive cost | Retain as `Multiview (Experimental)` for interactive regression testing; keep per-eye default. |
| Modest regression, but correct and isolated with no inactive cost | Retain only if the regression lane remains materially useful and is clearly experimental; otherwise keep the narrower launch/automation lane. |
| Per-eye regression caused by shared target topology | Reject that topology; use the bounded fallback manager or preserve the original per-eye target family. |
| Visual/depth mismatch, one-eye omission, unstable recovery, or material memory risk | Disable the live option and retain per-eye while the defect is open. |
| High maintenance with no product or diagnostic value | Remove the optional path deliberately and update the XR render guardrail rather than leaving dead conditional code. |

A positive performance result is necessary to consider promotion, but it is
not necessary to retain a safe experimental regression control.

## Risks And Review Traps

- **False A/B from target changes:** array-backed per-eye may change overlap
  behavior. Compare it with the original dual-swapchain baseline before using
  it as the control.
- **One shared mutable eye uniform:** overwriting the left eye before a
  submission completes can recreate the projection defect already closed by
  Tactical
  [`107`](107-xr-stereo-uniform-ownership-and-multiview.md).
- **Terrain-only completion:** proxy vegetation is part of the horizon and
  must not disappear in multiview.
- **Mono projection duplicated twice:** two non-empty layers are insufficient;
  the eyes must use distinct physical views and FOVs.
- **Per-eye cull reused as stereo cull:** one eye's frustum may omit geometry
  visible to the other. Use a conservative union.
- **Pass-order drift:** the backdrop must load exact depth and precede actors
  and translucent terrain exactly as the accepted per-eye composition does.
- **Sticky experimental preference:** do not persist the option before safe
  recovery is established.
- **Menu-only state:** the label must reflect the active host result, not just
  the last clicked value.
- **Permanent duplicate targets:** do not spend about 90 MiB plus driver
  overhead merely to avoid a small target abstraction.
- **Actor-population confound:** normal actors are the product row;
  actor-skipped results are diagnostic only.
- **App-crate gravity:** target/session glue belongs at the platform rim, but
  neutral values, effects, render preparation, and renderer behavior remain
  shared.

## 2026-07-29 Disposition

Tactical [`278`](278-quest-procedural-horizon-multiview.md#result) records the
completed terrain/tree renderer and Quest A/B. Tactical
[`281`](281-cross-platform-dynamic-xr-render-targets.md#implementation-evidence)
records the shared three-mode target manager, UI control, Quest standalone
cycle, and Linux Vulkan/WiVRn cycle.

The matched Quest result is a deliberate non-promotion: multiview reduces
average thread CPU from `8.523ms` to `5.771ms`, but increases reported app GPU
from `7.052ms` to `9.244ms` and app work from `14.384ms` to `17.053ms`.
It remains a live, transient diagnostic/optimization mode with no inactive
swapchain-family residency. Dual per-eye remains the default.

## Master Checklist

- [x] Investigate the current horizon, multiview, XR target, and settings
  boundaries.
- [x] Separate Tactical 278's renderer/performance scope from the live
  target/control scope.
- [x] Record the preferred topology and its protected-baseline feasibility
  gate.
- [x] Define transient initial setting policy and keep/promote/remove decision
  rules.
- [x] Complete stereo-array per-eye feasibility and select the target design.
- [x] Complete Tactical 278's terrain/tree renderer and initial Quest A/B.
- [x] Integrate the horizon into the scene multiview pass.
- [x] Implement the shared live setting and safe-boundary XR target switch.
- [ ] Pass automated, visual, lifecycle, and physical Quest validation.
- [x] Make and document the final multiview disposition.

## Closeout Rule

This parent closes only when:

- the procedural horizon is correct in both multiview layers;
- a capable headset can switch both directions interactively without a world
  or XR session restart;
- unsupported hosts remain safe and truthful;
- the protected per-eye path has not silently changed or regressed;
- normal-actor Quest evidence supports the recorded performance conclusion;
- inactive resource cost is bounded and documented; and
- the live topics, platform matrix, and child tacticals agree on whether
  multiview is default, experimental, diagnostic-only, or removed.

If implementation stops after only the terrain shader or only the menu row,
the workstream remains open.
