# Tactical 281: Cross-Platform Dynamic XR Render Targets

Status: planned 2026-07-28; ready for implementation.

Topic: `xr-render-path-switching`

## Originating Request

Keep all three XR render modes available for matched testing and allow a
running headset session to change among them. Do not permanently retain both
the dual-eye and stereo-array swapchain families. Build the lifecycle as shared
infrastructure for every shipped OpenXR host rather than a Quest-local menu
branch.

The durable contract lives in
[`../topics/xr-render-path-switching.md`](../topics/xr-render-path-switching.md).
This tactical owns its first bounded implementation and evidence.

## Outcome

A supported XR session can perform:

```text
Dual per-eye
  -> Array per-eye
  -> Array multiview
  -> Array per-eye
  -> Dual per-eye
```

without restarting the world, gameplay session, or OpenXR session. The switch
is requested through one neutral typed action, committed by the XR host at a
completed-frame boundary, reflected through requested/pending/active status,
and exercised by desktop XR and Android XR adapters.

Only one target family remains resident in steady state. A topology transition
may briefly overlap old and replacement resources for transactional rollback,
then must retire the inactive family before ordinary rendering resumes.

## Current Starting Point

As of 2026-07-28:

- Android XR chooses `full_frame_multiview` at startup.
- Android XR constructs either two `OpenXrEyeState` targets or one
  `OpenXrStereoState`, then borrows that fixed family into the main frame loop.
- the current array target exposes a `D2Array` view but not both layer-specific
  `D2` views needed for array-backed per-eye rendering;
- desktop XR constructs only two eye swapchains;
- the shared `OpenXrFrameDriver` already owns legal OpenXR frame sequencing,
  but not an owned replaceable target family;
- Vulkan eye/stereo swapchain mechanics are partly shared through
  `mclone-xr-graphics`;
- desktop Metal has an app-local single-eye swapchain wrapper and no accepted
  stereo-array implementation;
- full-frame multiview already renders most scene layers, while its procedural
  horizon completion remains separate renderer work; and
- XR render scale is launch-only and creates the selected swapchains at the
  scaled extent.

The implementation must not encode these starting asymmetries into the final
shared contract.

## Mode And Capability Model

Add one neutral mode inventory equivalent to:

```rust
enum XrRenderMode {
    DualPerEye,
    ArrayPerEye,
    ArrayMultiview,
}
```

Exact names may follow existing conventions. The public contract must not
collapse the two per-eye modes.

Capability is a supported-mode set plus a bounded rejection reason, not one
`multiview: bool`. A backend may support:

- only `DualPerEye`;
- `DualPerEye` and `ArrayPerEye`; or
- all three modes.

The active target topology is a separate observation:

```rust
enum XrTargetTopology {
    DualEye,
    StereoArray,
}
```

Flat targets publish XR as not applicable and do not display an inert setting.

## Ownership

- `mclone-ui`: labels, capability/status row, and typed selection action.
- shared client-experience/effect seam: requested and host-confirmed state.
- `mclone-scene`: renderer selection and feature-equivalent scene
  composition.
- `mclone-render-session` / `mclone-render`: host-neutral mode and target-view
  contracts where appropriate.
- `mclone-xr-host`: target transition state machine integrated with
  `OpenXrFrameDriver`.
- `mclone-xr-graphics`: shared Vulkan eye/array target construction and view
  creation.
- desktop Metal graphics adapter: backend-specific equivalent construction.
- desktop XR and Android XR apps: concrete factories, runtime lifecycle, and
  capability publication only.

Do not move OpenXR types into shared UI, scene, or generic app-runtime crates.
Do not add independent low-level wait/begin/end calls to either app adapter.

## Target Specification

The owned target manager consumes a normalized specification containing at
least:

- requested render mode and derived topology;
- eye extent;
- color and depth formats;
- sample count;
- foveation state or backend equivalent; and
- required texture-view/multiview capabilities.

Although this tactical does not add a live XR render-scale control, extent must
be part of the target specification so a later scale change uses the same
transaction instead of creating a second lifecycle system.

## Transition State Machine

Implement and unit-test a state machine equivalent to:

```text
Stable(active)
    |
    v
Requested(latest)
    |
    v
Quiescing
    |
    +--> same topology --> Commit encoding mode
    |
    `--> new topology --> Create replacement --> Commit --> Retire old
                              |
                              `--> failure --> Roll back to old
```

Required rules:

1. consume requests only after the preceding OpenXR frame completes;
2. coalesce multiple unstarted requests to the latest supported mode;
3. never destroy a family with an acquired or waited image;
4. preserve the old active family until replacement creation succeeds when
   the platform memory policy permits transactional overlap;
5. commit target family, scene encoding, projection submission, and published
   active status as one logical transition;
6. retire the old family before resuming ordinary steady-state frames;
7. reject unsupported selections before allocating;
8. preserve or normalize the last request across pause, stop/ready, device
   recovery, and session-loss handling; and
9. expose bounded transition failure without crashing or restarting the world.

The manager must not cache the retired family for a faster future switch.

## Work Plan

### Slice 0: pin behavior and add neutral contracts

- [ ] Record current Android dual-eye, Android stereo-array, desktop Vulkan
  eye, and desktop Metal eye target facts.
- [ ] Add the three-mode value, supported-mode set, target topology, and
  requested/pending/active/result snapshot.
- [ ] Add reducer/state-machine tests for coalescing, unsupported modes,
  success, failure, rollback, and recovery.
- [ ] Add compile-time exhaustiveness gates so new XR modes cannot be silently
  omitted by a platform adapter.

### Slice 1: owned target manager and safe frame boundary

- [ ] Replace frame-loop-lifetime borrowed target selection with an owned,
  replaceable target family.
- [ ] Integrate one transition hook into `OpenXrFrameDriver` after a completed
  frame and before the next acquisition.
- [ ] Track acquired/waited/released counts and forbid a topology commit while
  old images remain outstanding.
- [ ] Support same-topology encoding changes without swapchain recreation.
- [ ] Support transactional topology replacement and immediate old-family
  retirement after commit.
- [ ] Add deterministic fake-target tests for creation failure and
  destruction ordering.

### Slice 2: shared Vulkan array and layer views

- [ ] Generalize `mclone-xr-graphics` Vulkan target construction so desktop
  Vulkan and Android XR use the same eye/array implementation.
- [ ] Expose one `D2Array` view plus non-overlapping layer-zero and layer-one
  `D2` views for every acquired stereo-array image.
- [ ] Add matching two-layer depth ownership and both view shapes.
- [ ] Submit two projection views against array indices zero and one for both
  array-backed encodings.
- [ ] Apply and validate Quest foveation state across both layers.
- [ ] Prove original dual-eye mode remains available after an array-family
  transition.

### Slice 3: desktop Metal adoption

- [ ] Move reusable target lifecycle behind the same manager without
  pretending Metal resources are Vulkan resources.
- [ ] Implement and validate a two-layer Metal/OpenXR swapchain plus layer
  views where the runtime supports it.
- [ ] Publish actual Metal adapter/runtime supported modes.
- [ ] Keep `DualPerEye` live and truthful when array or multiview capability is
  absent.
- [ ] Exercise topology replacement, failure recovery, and session lifecycle
  through the same state-machine contract.

### Slice 4: scene and shared live control

- [ ] Route all three modes through the existing shared XR scene host.
- [ ] Preserve distinct immutable physical-eye view/projection data for every
  submission.
- [ ] Add the XR-only Graphics row with distinct
  `Dual per-eye | Array per-eye | Array multiview` labels.
- [ ] Project requested, pending, active, and rejected state back to the same
  UI.
- [ ] Keep the initial CLI selection as an automation override.
- [ ] Keep the setting transient until recovery has physical acceptance.
- [ ] Ensure flat desktop, flat Android, and browser profiles show no XR row.

### Slice 5: automation and fault injection

- [ ] Add one-session automation for
  `dual -> array-per-eye -> multiview -> array-per-eye -> dual`.
- [ ] Inject replacement-creation failure and prove the old family remains
  active.
- [ ] Inject rapid coalesced requests before a boundary.
- [ ] Exercise pause/resume, stop/ready, skipped frames, and session loss.
- [ ] Report mode, topology, image count, layer count, extent, formats,
  foveation, estimated bytes, transition peak bytes, switch counts, failures,
  and outstanding image state.
- [ ] Assert no inactive target family survives the first stable frame after a
  topology commit.

### Slice 6: pixels and performance

- [ ] Capture and inspect both physical-eye layers in all supported modes.
- [ ] Verify exact terrain, procedural horizon, trees, actors, translucent
  ordering, selection, effects, world UI, fog, depth, and foveation.
- [ ] Run matched stationary and moving comparisons across all three modes.
- [ ] Keep world, actor population, render distance, horizon quality,
  foveation, render scale, refresh rate, camera path, and warmup fixed.
- [ ] Compare app-work and thread-CPU p50/p95/p99, GPU time, over-period rate,
  headroom, submission behavior, target bytes, and transition hitch.
- [ ] Select defaults per backend/device from evidence without removing the
  other supported diagnostic modes.

## Platform Acceptance Matrix

| Lane | Required gate |
|---|---|
| Shared/headless | reducer, state-machine, failure-injection, distinct stereo projection, and renderer-equivalence tests |
| Desktop XR Vulkan | build plus live three-mode switch on a multiview-capable runtime; truthful reduced capability otherwise |
| Desktop XR Metal | native build and live switch through every mode the runtime/adapter advertises; no false multiview claim |
| Android XR Vulkan | `pnpm native:android-xr:apk`, scripted live switching, and physical Quest pixels/performance/recovery |
| Flat desktop | native tests and proof that no XR-only row appears |
| Flat Android | APK/AVD compile or smoke gate appropriate to the neutral shared-control change; no XR-only row |
| Browser/WebGPU | shared UI/profile compile and browser smoke proving the XR-only row remains absent |

A compile-only desktop XR result does not establish target-lifecycle
correctness. If a physical/runtime lane is temporarily unavailable, leave that
acceptance item open rather than silently narrowing the implementation to
Quest.

## Memory Acceptance

The stable diagnostics must show:

- exactly one active OpenXR target family;
- zero inactive retained target families;
- no outstanding acquired images at topology commit;
- old-family retirement before the first ordinary stable frame after commit;
- steady-state bytes for the active family; and
- observed peak bytes and hitch duration during topology replacement.

Pipeline objects for supported encodings may remain resident when their cost is
bounded. This exception does not include swapchains, their wrapped images,
depth families, foveation attachments, or derived target views.

## Rendered-Output Milestones

Because this work changes presented pixels and target ownership, inspect output
at each first drawable milestone:

1. array-backed per-eye layer views;
2. first same-session switch into and out of array-backed per-eye;
3. first same-session multiview switch;
4. desktop Vulkan adoption;
5. desktop Metal adoption; and
6. Android XR / Quest adoption.

Save diagnostic captures outside the repository.

## Commit Plan

Use `Topic: xr-render-path-switching` throughout the series. Prefer bounded
commits in this order:

1. neutral modes, status, and transition tests;
2. owned XR target manager and frame-boundary hook;
3. shared Vulkan array/per-layer targets;
4. Android XR adoption and Quest switch proof;
5. desktop Vulkan and Metal adoption;
6. shared UI/effect projection;
7. pixels, performance evidence, and closeout.

Reorder platform-adoption commits if physical-runtime availability makes a
different sequence more efficient, but do not land a Quest-private lifecycle
which the desktop adapters must later reverse-engineer.

## Closeout

This tactical closes only when:

- all three modes remain represented and independently measurable;
- every shipped OpenXR adapter uses the shared target manager;
- supported modes switch live without a world or OpenXR session restart;
- one target family remains resident in steady state;
- unsupported platforms and modes remain truthful;
- failure and lifecycle recovery are proven;
- required screenshots have been inspected;
- matched performance evidence records the default recommendation per
  backend/device; and
- this topic, the platform matrix, Graphics settings topic, and relevant
  performance/horizon topics reflect the implemented state.
