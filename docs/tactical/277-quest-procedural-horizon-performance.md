# Tactical 277: Quest Procedural-Horizon Performance

Status: complete 2026-07-28; multiview follow-up sequenced separately.

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

## Results

The release Quest 3 baseline at commit `770ebe89` established that the
full-quality horizon was the limiting renderer:

| Stationary RD5 lane | FPS | App work avg / p95 | Headroom avg | App GPU | Over-period |
|---|---:|---:|---:|---:|---:|
| exact A | `72.00` | `5.397 / 6.175ms` | `8.492ms` | `2.330ms` | `0%` |
| composed | `51.91` | `19.157 / 20.313ms` | `-5.268ms` | `11.906ms` | `100%` |
| exact repeat | `72.00` | `5.382 / 6.404ms` | `8.506ms` | `2.267ms` | `0%` |

The exact repeat rules out run-order or short-term thermal drift. The composed
path submitted all 160 resident tiles to each eye and spent roughly four
milliseconds encoding the backdrop in each eye, while Meta app GPU time rose
by `9.6ms`.

Commit `99d6c5b6` adds conservative shared tile visibility:

- tiles wholly covered by a finer committed level are not submitted;
- tiles outside the physical eye frustum are not submitted;
- vegetation uses a 16-block crown margin;
- visibility storage is fixed and reused per resident slot; and
- terrain and vegetation counters expose each rejection class.

The same synthetic stereo scene is byte-identical before and after the change:

```text
f8f013e7a550554ab0fb43b58cc586c87e482c926fe6151f28ad9e4d63dc6d22
```

Post-change stationary repeats draw 27–28 terrain tiles:

| Stationary composed repeat | FPS | App work avg / p95 | Headroom avg | App GPU | Over-period |
|---|---:|---:|---:|---:|---:|
| first | `72.01` | `10.648 / 11.570ms` | `3.240ms` | `5.629ms` | `0%` |
| second | `72.01` | `10.781 / 11.770ms` | `3.108ms` | `5.711ms` | `0%` |

The moving lane is improved but not perfectly locked:

| RD5 lane | FPS | App work avg / p95 | Headroom avg | App GPU | Over-period |
|---|---:|---:|---:|---:|---:|
| exact orbit | `72.00` | `6.090 / 7.834ms` | `7.799ms` | `1.987ms` | `0%` |
| composed orbit repeat | `71.81` | `11.230 / 13.463ms` | `2.659ms` | `5.630ms` | `2.2%` |
| composed 300s 8x flight | `71.70` | `11.245 / 13.944ms` | `2.644ms` | `4.748ms` | `5.6%` |

The orbit result repeated within `0.04ms` average app work and `0.27ms` p95.
It is therefore a real moving-tail condition rather than one anomalous run.
The low Meta GPU time and duplicate per-eye backdrop encoding make a true
two-layer multiview horizon a justified next measurement. They do not prove it
will win: prior exact-only multiview had no average benefit, and the horizon
multiview pipeline does not exist yet.

## Persistent Travel Closeout

Commit `1d244c80` carries persistence queue facts and exact-center readiness
through the shared runtime/scene receipt, then extends the persisted-world
runner with composed orbit/flight modes.

Two release flights at `34.4` blocks/second completed:

- `180s`, `6,191` blocks, 12,916 horizon frames; and
- `300s`, `10,319` blocks, 21,510 horizon frames.

Both reported zero exact-center-not-ready frames. In the longer sample,
foreground persistence ownership peaked at 30 requests, cache writes peaked at
12 live / 15 lifetime-high-water requests and about `0.60MiB`, and durable
writes peaked at two requests / 720 bytes. The final live queues were zero
except one `15KiB` cache write. No cache write was skipped.

Quest `dumpsys meminfo` RSS samples during the longer run were approximately
`1.10`, `1.03`, `1.03`, `1.16`, and `1.22GiB`: bounded oscillation rather than
distance-proportional growth toward the prior multi-gigabyte kill. The process
completed normally. The disposable persisted guardrail world occupied
`504,496KiB`; generated-clean on-disk growth remains the already-separated
storage-policy question.

Raw evidence remains outside the repository:

```text
/tmp/mclone-277-stationary-exact-a2.txt
/tmp/mclone-277-stationary-composed-b2.txt
/tmp/mclone-277-stationary-exact-c.txt
/tmp/mclone-277-stationary-composed-cull.txt
/tmp/mclone-277-stationary-composed-cull-repeat.txt
/tmp/mclone-277-orbit-exact.txt
/tmp/mclone-277-orbit-composed-cull-repeat.txt
/tmp/mclone-277-persisted-flight.txt
/tmp/mclone-277-persisted-flight-long.txt
```

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

The evidence now justifies that conditional slice. Tactical
[`278`](278-quest-procedural-horizon-multiview.md) owns it and must preserve
the per-eye path as its comparator.

## Completion Gate

- [x] Horizon-aware settled benchmark receipts are implemented and tested.
- [x] Release Quest 3 exact/composed/exact stationary evidence is recorded.
- [x] Release Quest 3 exact/composed orbit evidence is recorded.
- [x] The composed Tactical 275 physical travel soak is recorded.
- [x] Any material regression is attributed before optimization.
- [x] A bounded supported optimization is implemented and remeasured, or the
      no-change conclusion is recorded.
- [x] The Quest performance ledger and living topic docs are updated.
- [x] The worktree is clean and all implementation/evidence commits identify
      both topic threads.
