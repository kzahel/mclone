# Tactical 277: Quest Procedural-Horizon Performance

Status: active 2026-07-28.

Topics: `procedural-horizon-clipmap`, `performance`

## Originating Request

Characterize and improve the procedural distant-terrain feature on a physical
Quest 3 standalone headset. Interactive review accepted its XR appearance but
did not feel confidently locked to the 72 Hz display rate.

The current product path is separate per-eye rendering with frame overlap.
Optional full-frame multiview does not render the procedural horizon. Earlier
Quest measurements found no average frame-work win from multiview for exact
terrain alone, although it modestly improved some tails. That historical result
must not be treated as an answer for a horizon renderer that is still encoded
once per eye.

## Objective

Establish an honest current-commit, same-device exact-only versus composed
baseline, attribute any material horizon cost, and land only a bounded
optimization supported by that evidence.

The accepted full-quality presentation is the comparison basis:

- Quest 3 standalone at 72 Hz;
- production per-eye frame-overlap path;
- default eye scale and foveation;
- render distance 5 as the product guardrail;
- identical seed, start pose, actors, fog, and exact-terrain settings; and
- the default ten clipmap levels, stride-one mesh, material appearance, and
  vegetation.

Do not reduce horizon quality before measuring it. Render-distance 7 remains a
pressure lane, not the product acceptance gate.

## Benchmark Validity

The existing settled Quest probe waits for exact chunk publication, render
compilation, upload, and UI activity to become quiet. It does not currently
wait for the composed horizon to become target-ready or for asynchronous
vegetation admission to drain. A composed sample can therefore start while
the feature under test is still changing.

Before recording the baseline:

- attach the scene's horizon diagnostics to the Android XR rendered-frame
  receipt;
- require `target_ready`, zero pending vegetation tiles, and matching submitted
  and completed vegetation job counts when a horizon is active;
- retain the exact-only settled behavior when no horizon exists; and
- print the horizon state in settle/start/end evidence so an invalid sample is
  conspicuous.

This is benchmark instrumentation, not a new product gate.

## Measurement Matrix

### M1: matched steady rendering

Run an alternating exact/composed/exact sequence from one release APK:

1. exact-only, settled stationary, 20 seconds;
2. composed, settled stationary, 20 seconds; and
3. exact-only repeat, settled stationary, 20 seconds.

Use `MCLONE_ANDROID_XR_PERF_HEADROOM` as the frame-budget authority and
`XR_META_performance_metrics` for compositor CPU/GPU evidence. Compare
`app_work` percentiles, headroom, over-period frames, submission cadence,
per-eye terrain encoding, draw counts, and device thermal/performance state.
The exact repeat bounds run-order and thermal drift.

### M2: matched continuous presentation

Repeat exact-only and composed with the settled orbit lane. This keeps the
exact footprint resident while changing both eye views and fractional horizon
presentation, exposing per-frame view/render work without making world
generation or persistence the primary variable.

### M3: streaming and memory health

Run the current composed build through a bounded persistent travel soak. This
is both a realistic horizon workload and the pending physical validation for
Tactical 275. It must show current-center exact reacquisition and plateaus in
process memory, persistence queue counts, and owned-byte high-water marks.
Report this separately from the steady renderer delta.

## Attribution And Optimization Policy

If composed rendering has a material steady cost, use the existing stage and
Meta GPU receipts to classify it before changing code:

- higher per-eye terrain encode time with little Meta GPU movement points to
  CPU command preparation or duplicate per-eye state work;
- higher Meta GPU time points to horizon geometry, fragment, depth, or
  vegetation load;
- orbit-only regression points to residency/presentation updates or
  vegetation admission; and
- memory or exact-streaming failure belongs to the Tactical 275 incident, not
  a frame-rate workaround.

Prefer a shared renderer improvement that preserves the accepted pixels and
benefits flat and XR clients. Do not add an app-local Quest-only terrain
implementation.

Full-frame multiview is a separate conditional slice. Implement it only if the
matched evidence says duplicate per-eye horizon work is material and a true
two-layer horizon path is the appropriate fix. That slice must cover terrain,
vegetation, distinct eye projections, exact-coverage discard, shared
reversed-Z depth, fog, and both-eye pixel validation. It must then be compared
against the same per-eye baseline; the old exact-only multiview result is
context, not proof.

## Human Review Checkpoint

No subjective visual review is required merely to accept benchmark
instrumentation or a numerically neutral diagnosis. Human review is required
after any optimization that changes rendered pixels, LOD admission,
vegetation density, fog, foveation, eye scale, or render topology.

The review handoff must state:

- exact-only and composed Quest 3 numbers;
- whether 72 Hz is met at render distance 5;
- the attributed limiting stage;
- what changed, if anything;
- matched before/after numbers; and
- whether full-frame multiview is now justified, rejected, or still
  unmeasured for the horizon.

## Completion Gate

- [ ] Horizon-aware settled benchmark receipts are implemented and tested.
- [ ] Release Quest 3 exact/composed/exact stationary evidence is recorded.
- [ ] Release Quest 3 exact/composed orbit evidence is recorded.
- [ ] The composed Tactical 275 physical travel soak is recorded.
- [ ] Any material regression is attributed before optimization.
- [ ] A bounded supported optimization is implemented and remeasured, or the
      no-change conclusion is recorded.
- [ ] The Quest performance ledger and living topic docs are updated.
- [ ] The worktree is clean and all implementation/evidence commits identify
      both topic threads.
