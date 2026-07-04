# 140: Streaming Throughput And Frame Pacing Baselines

Status: active benchmark and attribution checkpoint. Slice A2 clean synthetic
loading-settle isolation baseline captured on 2026-07-04 at `482f0d51`; the
primary desktop-shaped startup-streaming baseline is still pending. No
optimization slices should start from this doc until the desktop-shaped baseline
matrix is captured and interpreted.
Workstream: native Rust performance, desktop throughput, Android XR / Quest
frame pacing, shared runtime/render scheduling policy.

## Purpose

Capture the current conflict between two valid performance goals:

- Desktop local play should generate, publish, mesh, and settle high render
  distances at much higher throughput than the current multi-minute RD20/RD30
  experience.
- Quest and other constrained XR/mobile lanes must keep chunk generation,
  update application, render-section compilation, GPU upload, and ready-state
  publication from creating frame-time tails.

The recent startup work fixed the entry gate: local play can enter as soon as
the center `3x3` publication gate is ready, even when a large render distance
continues streaming in the background. That does not solve full-view throughput.
The next work should measure the current steady and streaming behavior from
both ends before changing default backpressure or worker policies.

This tactical is intentionally about reproducible baselines and interpretation.
Concrete optimization plans should be written only after we can say which phase
is limiting each lane.

## Starting Evidence

Known desktop evidence:

- `docs/performance-records.md` has release loading-settle isolation baselines
  from 2026-07-04. The clean run recorded RD20 as multi-minute: roughly `108s`
  runtime settle, `95s` render mesh settle, and `203s` full drain. That is
  useful for splitting server/light/runtime cost from mesh cost, but it is
  synthetic: it waits for full target readiness and then drains render mesh
  work, instead of entering playable and streaming under the desktop frame loop.
- Follow-up local observations around the startup fix still show RD20 full
  settle on the order of minutes. Runtime settle and render mesh settle are both
  large enough that we need to split server/worldgen/light throughput from
  render compile/upload throughput.

Known Quest evidence:

- `120-vanilla-render-compile-backpressure.md` measured that more render compile
  workers can improve terrain throughput, but on Quest they can also produce
  larger runtime sync and upload tails.
- `128-terrain-render-pipeline-coordination.md` showed upload/accept budgets can
  reduce some spikes, but strict budgets can build backlog and move the tail
  into ready-set publication, upload selection, or eye encode.
- `130-quest-thread-scheduling-and-streaming-tail-attribution.md`,
  `131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md`, and
  `133-session-network-bus-and-update-pacing.md` show that Quest RD7 streaming
  remains close to the frame budget even after major tail fixes. The old severe
  tails were real, so the solution is not to remove backpressure globally.

The working assumption: the current defaults may be too conservative for
desktop throughput and still not perfectly shaped for Quest. The likely answer
is a measured policy boundary, not one global knob.

## Benchmark Principles

- Record exact commit hash and `git_dirty` state. Dirty runs are allowed for
  investigation, but the first durable baseline should be clean except for
  intentional benchmark-doc edits.
- Save raw command output under `/tmp`; summarize durable findings in
  `docs/performance-records.md` only after the run is interpreted.
- Treat desktop-shaped startup streaming as the primary local-play benchmark.
  Synthetic loading-settle remains useful for attribution, but it must not be
  the only target for throughput policy.
- Keep runtime settle and render mesh settle separate. A faster mesh path does
  not prove server generation improved.
- Keep average throughput and frame tails separate. A higher chunks/sec number
  is not acceptable if Quest p95/p99 or dropped frames regress.
- Compare profile-specific policies honestly. If desktop needs more workers or
  looser admission than Quest, make that an explicit profile decision rather
  than hiding it in platform-local code.
- Prefer release builds for final throughput claims. Optimized-dev smokes are
  useful only for trend checks.

## Baseline Matrix

### A. Desktop Startup Streaming Throughput

Goal: mirror local desktop play. Enter the world at the playable gate, then keep
polling, syncing render sections, uploading, and rendering through the same
paced desktop-shaped frame loop while the requested view streams in.

Primary clean baseline:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --startup-streaming-perf \
  --render-distance 20 \
  --startup-streaming-frames 30000 \
  --target-hz 120 \
  --debug-passive-showcase false \
  --render-compile-workers 1 \
  --simulation-cadence 20/20/60 \
  | tee /tmp/mclone-startup-streaming-rd20-workers1-cadence20.json
```

Smoke:

```bash
pnpm native:startup-streaming:smoke
```

Record:

- enter-playable frame/time and cached section count
- first full-view-ready frame/time
- first render-quiescent frame/time, if reached
- final target ready/count/percent and loaded chunk count
- frame avg/p95/p99/max and over-budget counts at target Hz
- total poll/remesh/upload/render time during the streaming window
- submitted/completed/uploaded render sections
- deadline-skipped compile requests
- pending jobs/publications/render chunks/render compile jobs/inflight sections
- update-pump stalled frames and server update queue depth

Interpretation:

- If playable entry is quick but first full-view-ready or quiescent is late,
  the startup gate is working and the remaining problem is streaming throughput.
- If frame tails are clean while backlog remains high, desktop throughput is
  likely being constrained by fixed admission/backpressure rather than raw
  frame safety.
- If frame tails are bad while backlog drains quickly, the desktop policy needs
  the same budget-awareness discipline as Quest, not simply more workers.

### A2. Desktop Loading-Settle Isolation

Goal: split full-view drain into runtime/server throughput and render mesh
throughput at fixed render distances. This is an auxiliary isolation probe, not
the primary local desktop startup model.

Primary clean baseline:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --loading-settle-perf \
  --loading-settle-distances 5,10,15,20 \
  --debug-passive-showcase false \
  --render-compile-workers 1 \
  --simulation-cadence 20/20/60 \
  | tee /tmp/mclone-loading-settle-rd5-20-workers1-cadence20.json
```

Worker-count sweep:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --loading-settle-perf \
  --loading-settle-distances 5,10,15 \
  --debug-passive-showcase false \
  --render-compile-workers 2 \
  --simulation-cadence 20/20/60 \
  | tee /tmp/mclone-loading-settle-rd5-15-workers2-cadence20.json
```

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --loading-settle-perf \
  --loading-settle-distances 5,10,15 \
  --debug-passive-showcase false \
  --render-compile-workers 4 \
  --simulation-cadence 20/20/60 \
  | tee /tmp/mclone-loading-settle-rd5-15-workers4-cadence20.json
```

Cadence sweep:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --loading-settle-perf \
  --loading-settle-distances 5,10,15 \
  --debug-passive-showcase false \
  --render-compile-workers 1 \
  --simulation-cadence 60/20/60 \
  | tee /tmp/mclone-loading-settle-rd5-15-workers1-cadence60.json
```

Optional isolation runs:

- `--lighting false` to separate light-status cost from terrain/features.
- RD20 with the best non-regressing worker/cadence candidate only, because RD20
  is already long enough to slow iteration.
- RD25/RD30 only as explicit long runs after RD20 is understood.

Record per sample:

- target chunks and tracking radius
- runtime settle ms and runtime chunks/sec
- render mesh settle ms and full chunks/sec
- simulation tick/seconds
- pending jobs/publications/render compile jobs at completion
- cached sections, rebuilt sections, rebuilt faces/indices

### B. Desktop Live Streaming Frame Budget

Goal: verify that desktop throughput improvements do not just move work into
interactive frame spikes.

Baseline:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --movement-frame-probe \
  --render-distance 10 \
  --render-compile-workers 1 \
  --frame-budget-frames 240 \
  --target-hz 120 \
  --path-radius 4 \
  --movement-frame-speed 32 \
  | tee /tmp/mclone-movement-frame-rd10-workers1.json
```

Repeat with `--render-compile-workers 2` and `4` if the loading-settle worker
sweep shows meaningful mesh-throughput gains.

Record:

- average/p95/p99/max frame time
- over-budget frames
- runtime poll/remesh/upload timing
- pending/max/available render compile slots
- loaded vs visible section/face pressure

### C. Lower-Level Server/Worldgen Isolation

Goal: decide whether the runtime settle limit is worldgen/light scheduling,
host cadence, publication, or client/render ingestion.

Run the existing lower-level lanes before adding new instrumentation:

```bash
pnpm native:worldgen:perf
pnpm native:runtime:perf
```

If these do not explain loading-settle runtime cost, add a focused follow-up
slice to expose per-status chunks/sec and pending mailbox depth during
`--loading-settle-perf`.

### D. Quest Guardrail

Goal: ensure any throughput policy candidate still respects headset frame
pacing and does not regress the backpressure story.

Use the current RD7 settled-orbit lane as the primary guardrail. The exact
command may evolve, but each comparable run should record workers, result
accept budget, section upload budget, section accept budget, render distance,
seed, duration, and whether actors are enabled.

Known useful guardrail command shape:

```bash
node ./scripts/run-native-bash.mjs ./android-xr/validate-quest-openxr.sh \
  --render-compile-workers 2 \
  --xr-render-completed-result-accept-budget 2 \
  --xr-render-section-upload-budget 16 \
  --xr-render-section-accept-budget 64 \
  --perf-seconds 45 \
  --perf-settled-orbit \
  --perf-orbit-speed 4.3 \
  --perf-metrics \
  --wait-seconds 270 \
  --perf-summary /tmp/mclone-quest-rd7-guardrail-2-16-64.txt \
  --log /tmp/mclone-quest-rd7-guardrail-2-16-64-logcat.txt \
  --view-pose 0,120,-96,180 \
  --seed 12345 \
  --chunk-x 0 \
  --chunk-z 0 \
  --render-distance 7 \
  --day-time 6000 \
  --freeze-time
```

For policy comparisons, run at least:

- current default lane
- current known guardrail lane
- the desktop candidate policy if it changes worker count or admission/upload
  defaults

Record:

- Meta dropped frames
- frame avg/p95/p99/max
- app work avg/p95/max and app over-period percentage
- thread CPU vs blocked summary when available
- terrain runtime sync/upload/ready/publish buckets
- compile worker pending/queued state and upload/result backlog

## Captured Results

### Slice A Implementation Smoke

Validated the startup-streaming benchmark path in optimized-dev mode while this
slice was dirty. This is not a durable throughput baseline.

Raw output:
`/tmp/mclone-startup-streaming-rd5-json-shape.json`.

Command:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --startup-streaming-perf \
  --render-distance 5 \
  --startup-streaming-frames 5 \
  --target-hz 120 \
  --debug-passive-showcase false
```

Result shape: playable at `1121.869 ms`, `128` cached sections at entry,
`average_frame_ms=3.916`, `p95_frame_ms=10.508`, final target readiness
`9/169` after only five paced streaming frames. This verifies the JSON and
desktop-shaped pump/frame-loop wiring; it intentionally does not wait for full
view readiness.

### Slice A2: Clean Desktop Loading-Settle Isolation Baseline

Captured on 2026-07-04 at commit `482f0d51`.

`git_dirty=false`; release build; `debug_assertions=false`.

This is retained as a synthetic attribution run. It does not model desktop
startup followed by progressive streaming under frame-loop backpressure.

Raw output for this local run:
`/tmp/mclone-loading-settle-rd5-20-workers1-cadence20.json`.

Command:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --loading-settle-perf \
  --loading-settle-distances 5,10,15,20 \
  --debug-passive-showcase false \
  --render-compile-workers 1 \
  --simulation-cadence 20/20/60
```

Summary:

| Render distance | Tracking radius | Target chunks | Runtime settle | Mesh settle | Full settle | Runtime chunks/sec | Full chunks/sec | Cached sections |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `5` | `6` | `169` | `12,473.178 ms` | `943.591 ms` | `13,416.769 ms` | `13.549` | `12.596` | `1,936` |
| `10` | `11` | `529` | `33,325.249 ms` | `7,981.598 ms` | `41,306.846 ms` | `15.874` | `12.807` | `7,056` |
| `15` | `16` | `1,089` | `64,884.972 ms` | `33,042.018 ms` | `97,926.990 ms` | `16.784` | `11.121` | `15,376` |
| `20` | `21` | `1,849` | `108,077.834 ms` | `94,653.061 ms` | `202,730.895 ms` | `17.108` | `9.120` | `26,896` |

Immediate interpretation:

- The current clean RD20 baseline is still multi-minute: `202.731s` full
  settle, split into `108.078s` runtime/server settle and `94.653s` render mesh
  settle.
- Runtime is still the larger RD20 share (`53.3%`), and simulation time
  (`106.500s`) closely tracks runtime wall time. That makes cadence,
  scheduler/publication pacing, and server/worldgen/light work first-class
  suspects.
- Mesh is already large enough to need its own sweep (`46.7%` of RD20 full
  settle). Single-worker cached-section throughput falls sharply with distance,
  from roughly `2052` sections/sec at RD5 to `284` sections/sec at RD20.
- The next pass should not tune one global backpressure knob. Run the cadence
  sweep and render-compile-worker sweep separately, then use a desktop
  movement-frame run and Quest RD7 guardrail before changing defaults.

## Interpretation Rules

Do not optimize from a single number. Use these reads:

- If `--simulation-cadence 60/20/60` improves runtime settle without worsening
  live frame probes, the current host tick cadence is likely throttling local
  integrated throughput.
- If more render compile workers improve mesh settle but not runtime settle, the
  bottleneck is render compilation, not generation.
- If more render compile workers improve desktop but regress Quest p95/p99 or
  dropped frames, keep worker count profile-specific or make admission adaptive.
- If Quest budgeted upload/accept policies improve p99 but build large backlog,
  do not make them default until ready-state publication and upload lifecycle
  are coordinated better.
- If `lighting=false` materially changes runtime settle, split light-status
  scheduling and propagation from terrain/features before touching render
  compile policy.
- If lower-level `worldgen_perf` is fast but loading-settle runtime is slow, the
  bottleneck is likely scheduling/publication/poll cadence rather than raw
  terrain generation.

## Acceptance Criteria For This Tactical

- [ ] A clean commit baseline exists for desktop startup-streaming RD20 with
  current defaults.
- [x] A clean synthetic isolation baseline exists for desktop loading-settle
  RD5/RD10/RD15/RD20 with current defaults.
- [ ] At least one worker-count sweep and one cadence sweep are recorded for
  desktop startup-streaming or, if needed for attribution, desktop
  loading-settle.
- [ ] At least one desktop live streaming frame-budget or startup-streaming run
  is recorded for the baseline and for any promising throughput candidate.
- [ ] At least one Quest RD7 guardrail run is recorded after the same candidate
  policy.
- [ ] The results are summarized in `docs/performance-records.md` with enough raw
  `/tmp` paths or copied summary tables to reproduce the interpretation.
- [ ] A follow-up tactical names the first concrete optimization target and why it
  should not make the opposite lane worse.

## Likely Follow-Up Directions

These are hypotheses, not plans:

- Add a named performance profile boundary for local desktop throughput versus
  headset/mobile frame pacing if one default cannot satisfy both.
- Make render compile worker defaults profile-aware after the matrix proves the
  split.
- Add adaptive admission that uses frame headroom/backlog rather than fixed
  worker and upload budgets.
- Add loading-settle per-status instrumentation if runtime settle remains opaque.
- Continue `128`'s resident terrain coordinator work if ready/publish/upload
  tails remain the Quest guardrail blocker.

## Cross-Links

- `docs/performance-records.md` owns durable baseline records.
- `139-vanilla-chunk-startup-scheduling.md` owns the high-render-distance entry
  gate and loading progress correctness.
- `120-vanilla-render-compile-backpressure.md` owns the Java-shaped render
  compile backpressure history.
- `128-terrain-render-pipeline-coordination.md` owns the long-term
  dirty-to-drawable terrain coordinator direction.
- `130-quest-thread-scheduling-and-streaming-tail-attribution.md`,
  `131-quest-cpu-gpu-overlap-and-frame-cost-hygiene.md`, and
  `133-session-network-bus-and-update-pacing.md` own the Quest RD7 frame-pacing
  evidence that prevents simply removing backpressure globally.
