# Dynamic XR Render-Path Switching

Topic: `xr-render-path-switching`

Status: accepted direction as of 2026-07-28; implementation has not started.
All shipped OpenXR hosts must use one shared live-selection and target-lifecycle
contract. Three modes remain independently selectable and measurable. Only one
OpenXR target family may remain resident in steady state.

## Scope

This topic owns the durable contract for changing the XR render path and its
OpenXR target topology without restarting the world, gameplay session, or
OpenXR session. It records:

- the three retained render modes;
- the live request, transition, and active-state model;
- target-family lifetime and memory policy;
- cross-platform ownership and capability behavior;
- the relationship to XR render scale;
- failure and lifecycle recovery; and
- validation and performance-comparison requirements.

The following documents retain their narrower ownership:

- [`graphics-video-settings.md`](graphics-video-settings.md) owns the shared
  player-facing Graphics page and preference policy;
- [`performance.md`](performance.md) owns the broader performance priority
  queue;
- [`procedural-horizon-clipmap.md`](procedural-horizon-clipmap.md) owns the
  current distant-terrain renderer and its stereo correctness;
- [`../tactical/107-xr-stereo-uniform-ownership-and-multiview.md`](../tactical/107-xr-stereo-uniform-ownership-and-multiview.md)
  records the original multiview and per-view uniform work; and
- [`../tactical/281-cross-platform-dynamic-xr-render-targets.md`](../tactical/281-cross-platform-dynamic-xr-render-targets.md)
  owns the first implementation.

This topic does not choose the fastest default before matched measurements. It
also does not make an XR-only option appear on flat desktop, flat Android, or
browser clients.

## Product Modes

A capable XR host exposes three distinct modes:

| Product mode | Target family | Encoding |
|---|---|---|
| `Dual per-eye` | two independent single-layer OpenXR swapchains | one pass and target acquisition per eye |
| `Array per-eye` | one OpenXR swapchain whose images have two array layers | two passes into layer-specific `D2` views |
| `Array multiview` | the same two-layer OpenXR swapchain shape | one multiview pass into a `D2Array` view |

The names must remain distinct in diagnostics and automation. Calling both
first and second modes merely “per-eye” would hide the swapchain-topology
variable and make performance results ambiguous.

`Dual per-eye` remains a supported control and fallback. Supporting multiview
does not require deleting it. Its two unrelated eye images simply cannot be
used together as one multiview render attachment.

`Array per-eye` is both a useful product candidate and a necessary experimental
control: it isolates the cost of the stereo-array swapchain topology from the
cost or benefit of multiview command encoding.

## Accepted Live-Switch Contract

All three modes are live session choices. A player or automation lane can
request this sequence without relaunching:

```text
Dual per-eye
     |
     v
Array per-eye <----> Array multiview
     |
     v
Dual per-eye
```

The active world, scene host, actors, terrain residency, controller state,
menu state, audio, networking, and OpenXR session remain alive. Only the XR
target family and render encoding may change.

Selection is transactional:

```text
Stable
  -> Requested
  -> Quiescing at a completed-frame boundary
  -> Creating or reusing the required target shape
  -> Committing
  -> Stable

Creating failure -> old Stable mode remains active
```

The UI and diagnostics distinguish:

- **requested mode**: the latest normalized player or automation request;
- **pending mode**: a request accepted but not yet committed;
- **active mode**: the mode which produced the last successfully submitted
  frame;
- **active topology**: `dual-eye` or `stereo-array`;
- **capability**: which of the three modes this graphics/runtime combination
  can actually use; and
- **transition result**: idle, pending, committed, rejected, or recovered,
  with a bounded reason.

A menu selection is not active merely because the click was accepted.

## One-Resident-Family Memory Policy

Do not retain both the dual-eye and stereo-array swapchain families to make
future switches instant.

At a representative Quest extent of `1680 x 1760`, a triple-buffered,
two-layer RGBA8 color family is about `67.7 MiB`; a two-layer D32 depth target
is about `22.6 MiB`. Keeping a second inactive family can therefore consume
roughly another `90 MiB` before driver bookkeeping and foveation resources.
That is not an acceptable steady-state convenience cost.

The target manager owns exactly one active family in ordinary rendering:

```text
Dual-eye family XOR stereo-array family
```

A topology-changing transaction may briefly keep the old family alive while
the replacement is created. This bounded create-before-commit overlap permits
failure to leave the known-good mode active; it is not permission to cache the
inactive family across subsequent frames. The old family must be retired once
the replacement commits and before ordinary steady-state rendering resumes.

The implementation must report both steady-state owned bytes and peak
transition bytes. If physical memory pressure makes create-before-commit
unsafe, the manager may add a drain-retire-create-recover strategy, but it
must not silently turn failure into an OpenXR session or world restart.

## What Changes Cheaply And What Rebuilds

`Array per-eye` and `Array multiview` share a target shape. Switching between
them should normally retain the acquired-image family, depth allocation, and
layer views while changing the selected pipeline/pass encoding at a completed
frame boundary.

Switching between `Dual per-eye` and either array mode changes the OpenXR
swapchain topology. The manager must:

1. wait until the current OpenXR frame has ended;
2. ensure no old swapchain image remains acquired or waited;
3. ensure submitted GPU work no longer requires old derived views/resources;
4. create the replacement color target family and matching depth resources;
5. apply and verify backend-specific foveation or other swapchain state;
6. commit the new target family and active mode atomically;
7. submit the correct swapchain and array indices on the next frame; and
8. retire the old family.

This operation may cause a one-time settings-change hitch. It must not require
an application, world, gameplay-session, or OpenXR-session restart.

Switching every frame is not a product goal. Repeated scripted switching is a
lifecycle test; ordinary selection is stable until another explicit request.

## Render Scale Relationship

The desktop flat World Scale control is a useful control-flow precedent, not
the same resource operation. It keeps the platform presentation swapchain at
native extent and replaces engine-owned intermediate color/depth targets.

Current XR render scale is launch-only. It computes the eye extent before
creating the actual OpenXR swapchains, so changing it live would currently
require target recreation.

The XR target manager should therefore use a complete normalized target
specification rather than hard-code “multiview toggle”:

```text
target specification
  = extent
  + topology
  + color/depth formats
  + sample count
  + foveation state
  + required view capabilities
```

The first implementation need not add a live XR render-scale setting, but its
transaction must allow a later extent change to use the same safe lifecycle.
Keeping a maximum-size swapchain and changing only its submitted image
rectangle is a separate optimization experiment, not an assumed replacement
for this contract.

## Shared And Platform Ownership

The implementation remains shared-first:

- `mclone-ui` owns neutral labels, capability/status presentation, and typed
  actions. It does not know OpenXR or `wgpu` types.
- `mclone-scene` owns scene preparation and selection between per-eye and
  multiview encoding. It retains one world/actor/horizon decision across the
  switch and does not create swapchains.
- `mclone-render-session` and `mclone-render` own neutral render-mode and
  target-view contracts where those contracts are not OpenXR-specific.
- `mclone-xr-host` owns the transition state machine and the only legal
  completed-frame boundary for acquisition, release, target replacement, and
  projection submission.
- `mclone-xr-graphics` owns reusable Vulkan/OpenXR/wgpu target construction,
  texture wrapping, and capability evidence.
- the desktop Metal adapter owns only the corresponding Metal/OpenXR texture
  mechanics that cannot be shared with Vulkan.
- desktop XR and Android XR adapters provide platform events, concrete target
  factories, and capability facts. They do not independently invent
  transition or menu policy.

`OpenXrFrameDriver` remains the exclusive owner of
poll/wait/begin/render-or-skip/end sequencing. The transition manager must
integrate with that owner rather than create a second frame loop.

## Cross-Platform Contract

“All platforms” means every client remains truthful and every shipped OpenXR
host adopts the same infrastructure:

| Platform/backend | Required behavior |
|---|---|
| Android XR / Quest Vulkan | live three-mode selection when multiview is supported; truthful rejection otherwise |
| Desktop XR Vulkan | the same owned target manager, modes, transition, and status contract |
| Desktop XR Metal | the same manager and live topology transition; expose only modes proven by the Metal adapter and runtime |
| Synthetic stereo/offscreen | deterministic renderer tests for per-eye and multiview scene equivalence without pretending to own OpenXR swapchains |
| Flat desktop, flat Android, browser | compile against the neutral shared state, publish XR as not applicable, and show no inert XR row |

Cross-platform does not mean claiming `Array multiview` on hardware which lacks
the required graphics capability. It means the lack of capability is explicit,
tested, and does not fork the lifecycle design.

## Lifecycle And Failure Invariants

- Requests coalesce to the latest normalized value before a transition starts.
- A transition starts only between completed OpenXR frames.
- No old target image may remain acquired when its family is destroyed.
- Per-eye uniforms remain immutable for the complete submission; a switch
  cannot reintroduce mutable left/right uniform reuse.
- OpenXR stop, loss-pending, pause, or device recovery cancels or normalizes
  pending work and reconstructs the last requested supported mode through the
  same manager.
- A failed replacement leaves the known-good family active when it is still
  valid and publishes a bounded reason.
- If the old family is no longer valid, recovery recreates a safe supported
  mode without restarting the world.
- Unsupported requests do not partially allocate targets or change the active
  status.
- Pipeline warmup may be eager when its memory is bounded; inactive OpenXR
  swapchain families may not be.

## Validation And Evidence

The implementation is incomplete until it has:

1. deterministic state-machine tests, including coalescing, unsupported modes,
   creation failure, rollback, pause/resume, session loss, and recovery;
2. distinct non-empty left/right pixels with correct physical projections in
   all supported modes;
3. identical world, horizon, actor, overlay, UI, fog, depth, and foveation
   semantics across matched captures;
4. repeated
   `dual -> array-per-eye -> multiview -> array-per-eye -> dual` switching in
   one live world and OpenXR session;
5. desktop Vulkan, desktop Metal, and Android XR adapter coverage proportional
   to the capabilities each publishes;
6. diagnostics proving one steady-state target family, zero acquired images at
   topology commit, and bounded transition peak memory; and
7. matched performance rows which keep world, actor population, LOD quality,
   render scale, foveation, refresh rate, camera path, and horizon readiness
   fixed.

The performance result chooses a platform default; it does not erase the other
supported diagnostic modes. Any default change requires physical-device
evidence and must remain recoverable through the same live control.

## Next Work

Implement the contract through Tactical
[`281`](../tactical/281-cross-platform-dynamic-xr-render-targets.md). Update
this topic whenever implementation changes the mode inventory, lifecycle,
memory policy, platform capability matrix, validation evidence, or recommended
default.
