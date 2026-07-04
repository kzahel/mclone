# 142: Throughput Policy With Quest RD5 Guardrail

Status: proposed active implementation plan.
Workstream: native Rust performance, desktop streaming throughput, Android XR /
Quest frame-pacing guardrails.

## Decision

Optimize throughput against desktop-shaped startup streaming, not against small
Quest render-distance controls and not against synthetic full-drain benchmarks
alone.

Use this current validation split:

- **Primary throughput target:** desktop native startup-streaming at RD10 and
  RD15.
- **Quest safety guardrail:** Quest OpenXR RD5 settled-orbit metrics. This
  should stay clean while throughput work changes worker/admission policy.
- **Quest pressure check:** Quest OpenXR RD7 settled-orbit metrics. RD7 is not
  the thing to perfect right now, but it must not materially regress.
- **Long-run checkpoint:** desktop RD20/RD30 only when the question is
  explicitly high-distance full-view behavior.

RD5 is too small to prove throughput. It is useful because it is currently clean
enough to expose fixed XR/render regressions: the 2026-07-04 budgeted RD5 orbit
reported `skipped_delta=0`, `11.677ms` average app work, `13.434ms` p95 app
work, and `2.212ms` average headroom. RD7, by contrast, had zero runtime skipped
frames but averaged `15.460ms` app work and was over-period for `81.9%` of the
sample. That points the current RD7 problem at view-distance terrain pressure,
not fixed XR overhead alone.

## Non-Goals

- Do not spend this workstream trying to make Quest RD7 perfect before improving
  desktop throughput.
- Do not tune for RD5 throughput. RD5 is a guardrail/control lane.
- Do not remove Quest backpressure globally just to improve desktop numbers.
- Do not use synthetic loading-settle as the only success metric. It remains an
  attribution probe, not the local-play target.

## Required Measurements

Desktop startup-streaming records must include:

- time to playable gate,
- time to full requested view ready,
- time to target render quiescent,
- runtime/server queue depth and pending publication counters,
- render compile pending/inflight counters,
- upload/remesh/render timing,
- sections/sec or chunks/sec where available,
- frame-budget misses under the configured desktop frame budget.

Quest records must include:

- `skipped_delta`,
- submitted/runtime FPS,
- `app_work_avg_ms`, `app_work_p95_ms`, `app_work_p99_ms`,
- `headroom_avg_ms`, `headroom_p05_ms`, `headroom_min_ms`,
- `app_over_period_pct`,
- terrain runtime/upload/ready buckets,
- compile/upload backlog counters,
- draw pressure: drawn sections and indices.

The compositor dropped-frame story is still incomplete because current
PerfMetrics logging records an absolute counter rather than a measured
before/after delta. Add that delta before using Meta dropped frames as a strict
pass/fail field.

## Benchmark Loop

For any throughput candidate:

1. Run desktop RD10 startup-streaming before/after.
2. Run desktop RD15 startup-streaming before/after if RD10 improves.
3. Run Quest RD5 settled-orbit metrics after the candidate.
4. Run Quest RD7 settled-orbit metrics after promising candidates or when a
   candidate changes shared worker, upload, or admission policy.
5. Record any meaningful result in `docs/performance-records.md` or
   `docs/quest-standalone-performance-records.md`.

Minimum acceptance for a candidate:

- desktop RD10/RD15 full-view-ready or render-quiescent time improves
  materially, or the candidate clearly improves an attributed subphase;
- Quest RD5 remains clean: `skipped_delta=0`, no material p95/p99 regression,
  and average headroom remains positive or near-positive;
- Quest RD7 does not materially worsen if the candidate touches shared pacing,
  worker count, upload, or render compile admission.

## Candidate Areas

Start with measured policy levers rather than broad rewrites:

- render compile worker count and admission limits by profile,
- frame-budget-aware compile/result/upload admission,
- runtime publication cadence versus render compile backlog,
- upload queue draining and ready-set publication coordination,
- desktop throughput profile that can be more aggressive than Quest while still
  sharing the same scheduler contracts.

If the measurements show runtime/server settle dominates, pivot to host cadence,
worldgen/light scheduling, and publication pacing before changing render compile
policy. If render mesh/quiescence dominates, focus on compile workers, queue
admission, and upload/ready lifecycle.

## First Slice

Create the repeatable desktop throughput matrix:

- add or document release startup-streaming RD10 and RD15 commands with stable
  output paths,
- capture current-default baseline rows,
- summarize playable/full-view-ready/render-quiescent timing and queue counters,
- then run a small render-compile worker/admission sweep only after the baseline
  is recorded.

The first optimization candidate should not be selected until that matrix tells
us whether the limiting phase is runtime publication, render compile/mesh, or
upload/ready publication.

## Links

- [`140-streaming-throughput-frame-pacing-baselines.md`](140-streaming-throughput-frame-pacing-baselines.md)
  keeps the setup-era evidence and baseline history.
- [`../performance-records.md`](../performance-records.md) owns desktop and
  flat Android performance records.
- [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
  owns standalone Quest/OpenXR records.
- [`139-vanilla-chunk-startup-scheduling.md`](139-vanilla-chunk-startup-scheduling.md)
  owns the playable gate and Java-shaped startup scheduling model.
- [`128-terrain-render-pipeline-coordination.md`](128-terrain-render-pipeline-coordination.md)
  owns the longer-term dirty-to-drawable coordinator direction.
