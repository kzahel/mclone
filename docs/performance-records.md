# Performance Records

This file records native performance baselines by date and commit so renderer, worldgen, scheduler, and culling changes can be compared against a durable trend instead of one-off terminal output.

Use release builds for final budgets. Optimized-dev smokes are still useful for day-to-day trend checks because `native/Cargo.toml` sets `profile.dev.opt-level = 2` while keeping debug assertions enabled.

## Benchmark Lanes

Primary smoke:

```bash
pnpm native:perf:smoke
```

Individual lanes:

```bash
pnpm native:worldgen:smoke
pnpm native:scheduler-loading:smoke
pnpm native:mesh-cpu:smoke
pnpm native:gpu-upload:smoke
pnpm native:movement:smoke
pnpm native:movement-frame:smoke
pnpm native:startup-streaming:smoke
pnpm native:loading-settle:smoke
pnpm native:timedemo:smoke
```

Release-oriented lanes:

```bash
pnpm native:worldgen:perf
pnpm native:scheduler-loading:perf
pnpm native:scheduler-loading:persisted-memory:perf
pnpm native:scheduler-loading:persisted-sqlite:perf
pnpm native:mesh-cpu:perf
pnpm native:gpu-upload:perf
pnpm native:movement:perf
pnpm native:movement-frame:perf
pnpm native:startup-streaming:perf
pnpm native:startup-streaming:perf:rd20-long
pnpm native:loading-settle:perf
pnpm native:timedemo:perf
```

Quest/OpenXR guardrail lanes:

```bash
pnpm native:android-xr:perf:orbit:rd5:metrics
pnpm native:android-xr:perf:orbit:rd7:metrics
pnpm native:android-xr:perf:orbit:rd7:frame-overlap
pnpm native:android-xr:perf:rd10:baseline
pnpm native:android-xr:perf:stationary:rd10:frame-overlap
pnpm native:android-xr:perf:flight:rd10:metrics
```

Flat Android validation lanes:

```bash
pnpm native:android:avd-smoke
pnpm native:android:quest-flat
```

Lower-level scheduler isolation:

```bash
pnpm native:runtime:smoke
pnpm native:runtime:perf
```

### What Each Lane Measures

- `native:worldgen:*`: surface chunk generation plus cold/warm full `FEATURES` batch generation. Reports dependency generation, carvers, feature decoration, cache hits, and chunks/sec.
- `native:scheduler-loading:*`: server-only loading ceiling probe. Applies one local chunk view to `ChunkScheduler`, hot-polls until the target view is client-visible and server queues drain, and reports target chunks/sec plus worldgen/light/publication counters. It includes current scheduler job admission, publication, light status work, and persistence queues, but no client update pump, mesh preparation, GPU upload, or frame pacing. `native:scheduler-loading:persisted-memory:perf` runs a prewarm pass into shared in-memory `ChunkRecord`s, then measures reload from already-generated/lit records with no disk. `native:scheduler-loading:persisted-sqlite:perf` repeats that shape through a temp SQLite world directory to price the normal record decode/load/storage path separately from generation/light.
- `native:mesh-cpu:*`: CPU-only render-section mesh probe. The default source prewarms a temp SQLite world, reloads already-generated/lit snapshots through `ChunkScheduler`, then builds all target-column render sections with the shared `mclone-render-session` mesh path. It includes asset catalog load, persisted server reload, snapshot-to-mesh conversion, ambient occlusion, light/tint sampling, and visibility-graph build. It excludes `wgpu`, GPU upload, frame pacing, render draw, and the runtime admission budget.
- `native:gpu-upload:*`: extends the mesh CPU probe with `--gpu-upload`. After CPU mesh build, it creates headless draw resources, uploads the atlas outside the measured section-upload phase, then times `TexturedSectionDrawResources::apply_section_updates_with_context_timed` for the prebuilt section meshes. It includes real `wgpu` buffer creation and renderer section bookkeeping, but no draw pass, frame loop, runtime upload budget, or Quest frame pacing.
- `native:movement:*`: integrated native client/server movement path. Reports chunk load/unload, scheduler polling, remesh time, dirty render-section rebuilds, and visible-vs-loaded face pressure.
- `native:movement-frame:*`: headless live-frame walking probe. Moves at spectator speed without fully draining render work each step and reports frame-budget misses, poll/remesh/upload/render timing, and render compile queue counters.
- `native:startup-streaming:*`: desktop-shaped local startup and streaming probe. The default perf lane uses RD10 at a 60 Hz budget for fast iteration; RD15 is the next stronger throughput signal before occasional RD20/RD30 long runs. Uses the same local startup pump to enter at the playable gate, then advances a paced headless frame loop that polls the runtime and syncs render sections under a frame deadline while the requested view fills in. Reports enter-playable time, first full-view-ready frame/time, stable first render-quiescent frame/time, frame-budget misses, runtime poll/remesh/upload/render timing, queue counters, and final readiness. `native:startup-streaming:persisted:*` first prewarms a temp SQLite world, reopens it through the same startup pump, and measures already-generated persisted startup/streaming without fresh generation/light noise. RD20 is an explicit long-run lane, not the default iteration target.
- `native:android:*`: flat Android validation on AVD and Quest-as-panel. These lanes prove Android packaging, asset staging, NativeActivity startup, wgpu surface creation, touch UI, and first rendered-frame behavior. They are useful for Android startup timing, but they are not OpenXR frame-pacing proof and do not currently emit dropped-frame/headroom metrics.
- `native:android-xr:perf:*rd5*`: Quest/OpenXR lower-distance control lanes. Use RD5 to distinguish fixed XR/render overhead from view-distance pressure; current records live in `docs/quest-standalone-performance-records.md`.
- `native:android-xr:perf:*rd7*`: Quest/OpenXR RD7 baseline guardrails. RD7 is the headset product-style frame-pacing lane; current records live in `docs/quest-standalone-performance-records.md`.
- `native:android-xr:perf:*rd10*`: Quest/OpenXR RD10 stress guardrails. These currently report headset app-work/headroom, dropped/stale frames, runtime/render/upload/compile tails, and Meta performance metrics where enabled. They do not yet emit the same playable/full-view-ready/render-quiescent startup-streaming markers as the desktop startup-streaming lane.
- `native:loading-settle:*`: synthetic full-drain isolation probe. Creates fresh transient worlds at fixed render distances, spawns the player at the seed-derived spawn center, waits for all target chunks to become light-ready, then synchronously builds render sections. Reports runtime settle time, render mesh settle time, chunks/sec, simulation time, and pending queue counters. Use it to split server/light/runtime cost from mesh cost, not as the primary desktop startup policy target. Use `native:mesh-cpu:*` and `native:gpu-upload:*` for already-loaded snapshot and prebuilt-mesh splits.
- `native:timedemo:*`: deterministic headless GPU render path over a fixed camera orbit. Reports scene build time, render setup, per-frame render time, and drawn section/index pressure. It does not read back PNGs per frame.
- `native:runtime:*`: lower-level server scheduler movement benchmark without client remesh/render work.

## Record Format

When adding a record, include:

- date
- commit hash
- whether the benchmark JSON reported `git_dirty`
- host/OS if relevant
- command and build mode
- concise numbers, not full raw JSON
- link or path to raw JSON only if it is intentionally preserved outside `/tmp`

The benchmark JSON includes `benchmark`, `recorded_unix_seconds`, `git_commit`, `git_dirty`, and `debug_assertions`.

## Records

### 2026-07-07 - Tactical 153 Render Compile Timing Meter-Tax A/B

Commit reported by benchmark JSON: `30e80fbf`, `git_dirty=false`.

Code change under test: `--render-compile-worker-timing false` disables the
render-compile worker busy timer/counter around each compile task. It does not
change render compile worker count, max-pending admission, queueing, or compile
work. Timing-off rows should still complete compile work while reporting zero
render-compile peer busy time.

Commands:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 1200 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-153-meter-rd10-fresh-timing-on.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 1200 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --render-compile-worker-timing false --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-153-meter-rd10-fresh-timing-off.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --startup-streaming-persisted-world --render-distance 10 --startup-streaming-frames 3600 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-153-meter-rd10-persisted-timing-on.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --startup-streaming-persisted-world --render-distance 10 --startup-streaming-frames 3600 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --render-compile-worker-timing false --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-153-meter-rd10-persisted-timing-off.json
```

| Lane | Timing | Full view | Target quiescent | p95 / max frame | Over / over-2x | Completed / uploaded sections | Render compile busy |
|---|---|---:|---:|---:|---:|---:|---:|
| RD10 fresh frozen | on | `9699.018ms` | `11078.662ms` | `11.114 / 13.026ms` | `0 / 0` | `7152 / 2184` | `1294.390ms` |
| RD10 fresh frozen | off | `9682.238ms` | `11048.769ms` | `10.370 / 14.053ms` | `0 / 0` | `7168 / 2190` | `0.000ms` |
| RD10 persisted frozen | on | `1027.058ms` | `4941.937ms` | `10.746 / 12.839ms` | `0 / 0` | `7040 / 2143` | `1550.942ms` |
| RD10 persisted frozen | off | `1016.075ms` | `4921.454ms` | `10.841 / 13.445ms` | `0 / 0` | `7040 / 2143` | `0.000ms` |

Measured timing-on deltas versus timing-off:

| Lane | Full-view delta | Target-quiescent delta | Decision |
|---|---:|---:|---|
| RD10 fresh frozen | `+16.780ms` / `+0.173%` | `+29.893ms` / `+0.271%` | below lever-ranking significance |
| RD10 persisted frozen | `+10.983ms` / `+1.081%` | `+20.483ms` / `+0.416%` | below lever-ranking significance |

Interpretation: the meter is not free, but its observed wall-clock tax is
small relative to the Slice 0 stage signals. The timing-off rows prove the
counter path can be removed for future A/B work. Keep using the Slice 0 lever
order; treat exact render compile per-section costs as including a small meter
tax.

### 2026-07-07 - Tactical 153 Slice 0 Whole-Pipeline Attribution

Commit reported by benchmark JSON: `1c8a0743`, `git_dirty=false`.

Commands:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 1200 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-153-slice0-rd10-fresh-frozen.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 15 --startup-streaming-frames 2400 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-153-slice0-rd15-fresh-frozen.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --startup-streaming-persisted-world --render-distance 10 --startup-streaming-frames 3600 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-153-slice0-rd10-persisted-frozen.json
```

Startup-streaming rows use frozen scheduled fluids, adaptive publication on,
render compile workers `1`, and max-pending render compile jobs `4`.

| Lane | Full view | Initial render complete | Target quiescent | p95 / max frame | Over / over-2x | Feature / light / compile / upload rate |
|---|---:|---:|---:|---:|---:|---:|
| RD10 fresh frozen | `9699.848ms` | `9699.848ms` | `11186.276ms` | `11.924 / 15.384ms` | `0 / 0` | `113.7` chunks/s / `105.0` statuses/s / `639.4` sections/s / `195.2` sections/s |
| RD15 fresh frozen | `19930.189ms` | `19930.189ms` | `21998.543ms` | `11.583 / 12.920ms` | `0 / 0` | `130.9` chunks/s / `133.2` statuses/s / `705.5` sections/s / `221.2` sections/s |
| RD10 persisted frozen | `1022.791ms` | `4940.262ms` | `4940.262ms` | `11.947 / 14.663ms` | `0 / 0` | n/a / n/a / `1425.0` sections/s / `433.8` sections/s |

Per-stage attribution:

| Stage | RD10 fresh | RD15 fresh | RD10 persisted | Boundary markers |
|---|---:|---:|---:|---|
| worldgen | `2161.554ms` busy, `19.3%`, `1.70ms`/chunk | `4482.406ms` busy, `20.4%`, `1.56ms`/chunk | idle | mailbox pending max `1` |
| light status | `11009.006ms` busy, `98.4%`, `9.38ms`/status | `21810.817ms` busy, `99.1%`, `7.44ms`/status | idle | mailbox pending max `155` / `348` |
| publication | host-publication max age `1546.781ms` | host-publication max age `2586.359ms` | idle | fresh-startup backlog follows light |
| client apply | no inbound update queue | no inbound update queue | no inbound update queue | local-integrated desktop lane |
| mesh admission | compile queue max age `23.806ms`, pending `4` | `21.315ms`, pending `4` | `23.936ms`, pending `3` | bounded and drained |
| mesh compile | `1297.312ms` busy, `11.6%`, `0.181ms`/section | `2916.281ms` busy, `13.3%`, `0.188ms`/section | `1525.197ms` busy, `30.9%`, `0.217ms`/section | persisted lane isolates mesh tail |
| GPU upload -> visible | upload queue max age `0.000ms`; `2184` sections | `0.000ms`; `4866` sections | `0.000ms`; `2143` sections | not the desktop ceiling yet |

Interpretation: fresh RD10/RD15 are light-status bound on this commit; the
light worker is effectively saturated while worldgen is not. The persisted lane
remains the clean mesh-capacity signal, with `5696` post-full-view target
rebuilds and `4.940s` target quiescence. That keeps Tactical 153 Slice 1 on the
vanilla-shaped render compile capacity work, followed by light/status
throughput and then publication/upload pacing if queue ages become the visible
tail.

### 2026-07-07 - Tactical 150 Slice 5 Clean Baseline Close-Out

Commit reported by benchmark JSON: `42d3e43a`, `git_dirty=false`.

Commands:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 1200 --target-hz 60 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-slice5-rd10-frozen-a.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 1200 --target-hz 60 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-slice5-rd10-frozen-b.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 15 --startup-streaming-frames 2400 --target-hz 60 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-slice5-rd15-frozen.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --startup-streaming-persisted-world --render-distance 10 --startup-streaming-frames 3600 --target-hz 60 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-slice5-rd10-persisted-frozen.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --movement-frame-probe --frame-budget-frames 240 --target-hz 120 > /tmp/mclone-150-slice5-movement-frame.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --movement-frame-probe --frame-budget-frames 240 --target-hz 120 > /tmp/mclone-150-slice5-movement-frame-repeat.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --seed 74739 --render-distance 10 --startup-streaming-frames 1200 --target-hz 60 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-slice5-rd10-seed74739-frozen.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 20 --startup-streaming-frames 30000 --target-hz 120 --debug-passive-showcase false > /tmp/mclone-150-slice5-rd20-long.json
```

Startup-streaming rows use the promoted default max-pending `4` with workers
`1`; the commands intentionally omit `--render-compile-max-pending-jobs`.

| Lane | Full view | Initial render complete | Target quiescent | p95 / p99 / max frame | Over / over-2x | Notes |
|---|---:|---:|---:|---:|---:|---|
| RD10 frozen A | `9183.072ms` | `9183.072ms` | `10744.258ms` | `7.191 / 7.583 / 10.332ms` | `0 / 0` | deadline skips `0`, post-full-view target rebuilds `0` |
| RD10 frozen B | `9137.523ms` | `9137.523ms` | `10853.328ms` | `7.004 / 7.626 / 8.726ms` | `0 / 0` | deadline skips `0`, post-full-view target rebuilds `0` |
| RD15 frozen | `19429.505ms` | `19429.505ms` | `21637.988ms` | `10.502 / 11.967 / 14.399ms` | `0 / 0` | `2.9x` faster than Slice 0 RD15 full-view baseline |
| RD10 persisted frozen | `1014.642ms` | `4924.696ms` | `4924.696ms` | `7.218 / 7.601 / 8.557ms` | `0 / 0` | already-generated reopen; post-full-view target rebuilds `5696` |
| RD10 frozen seed `74739` | `8157.360ms` | `8157.360ms` | `9601.381ms` | `6.363 / 6.918 / 7.478ms` | `0 / 0` | contrasting stone-shore seed |
| RD20 long live | `34864.820ms` | `34864.820ms` | `50780.778ms` | `14.163 / 16.315 / 26.256ms` | `28595 / 268` | 120 Hz backlog watch, not a pacing pass |

RD20 backlog markers: max pending worldgen-publication chunks `208`, max
pending light publications `6`, max server update queue depth `0`, update pump
stalls `0`, deadline-skipped compile requests `0`.

Movement-frame probe at 120 Hz:

| Run | p95 / p99 / max frame | Over / over-2x | Shared accounting |
|---|---:|---:|---|
| first | `2.876 / 3.065 / 18.038ms` | `1 / 1` | `240` observed frames, `0` conservation violations |
| repeat | `2.783 / 3.038 / 18.621ms` | `1 / 1` | `240` observed frames, `0` conservation violations |

Interpretation: desktop startup-streaming close-out is green for the promoted
queue-depth baseline. The movement-frame gate is not green on the legacy
over-budget counter: both repeats showed one isolated `~18ms` frame despite low
p99 and clean shared accounting. Record this as a follow-up caveat, not as a
reason to revert the queue-depth default. The RD20 row shows bounded
publication/update queues but also confirms that RD20 at a 120 Hz target is not
a frame-pacing acceptance lane.

### 2026-07-07 - Tactical 150 Default-On Queue Depth RD10 Check

Benchmarked worktree: dirty on `1815d7f0` while promoting
`DEFAULT_RENDER_SECTION_COMPILE_MAX_PENDING_JOBS = 4` and updating docs. This
is the default-on verification row: the command intentionally omits
`--render-compile-max-pending-jobs`.

Command:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 1200 --target-hz 60 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-defaulton-rd10-frozen.json
```

RD10 fresh startup-streaming, scheduled fluids frozen, adaptive publication
default on, workers `1`, default max-pending `4`, 60 Hz budget:

| Metric | Value |
|---|---:|
| startup playable | `525.847ms` |
| first full view ready | `9727.841ms` |
| first initial target render complete | `9727.841ms` |
| first target render quiescent | `11103.993ms` |
| p95 / p99 / max frame | `7.017 / 7.426 / 9.086ms` |
| over-budget / over-2x frames | `0 / 0` |
| submitted / completed compile sections | `7168 / 7168` |
| uploaded sections | `2190` |
| deadline-skipped compile requests | `0` |
| post-full-view target rebuilt sections | `0` |

Decision: default-on desktop behaves like the explicit queue-depth candidate and
proves the omitted flag path now reports `render_compile_max_pending_jobs=4`.

### 2026-07-07 - Tactical 150 Persisted RD10 Queue-Depth Follow-Up

Commit reported by benchmark JSON: `a89dc699`.

Note: `git_dirty=true` because this was captured while adding the shared
Android/XR max-pending startup plumbing and docs. Defaults are unchanged:
omitting `--render-compile-max-pending-jobs` still caps in-flight render
compile work at the worker count.

Commands:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --startup-streaming-persisted-world --render-distance 10 --startup-streaming-frames 3600 --target-hz 60 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-persisted-rd10-frozen-default.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --startup-streaming-persisted-world --render-distance 10 --startup-streaming-frames 3600 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-persisted-rd10-frozen-queue4.json
```

RD10 persisted startup-streaming, scheduled fluids frozen, adaptive publication
default on, workers `1`, 60 Hz budget:

| Metric | Frozen default | Frozen queue 4 |
|---|---:|---:|
| prewarm total / storage | `11591.353ms` / `34.4 MB` | `11500.504ms` / `34.4 MB` |
| startup playable | `125.045ms` | `123.775ms` |
| first full view ready | `1015.418ms` | `1028.074ms` |
| first initial target render complete | `19744.089ms` | `4889.199ms` |
| first target render quiescent | `19744.089ms` | `4889.199ms` |
| p95 / p99 frame | `7.072 / 7.467ms` | `7.181 / 7.593ms` |
| max frame | `8.028ms` | `10.465ms` |
| over-budget / over-2x frames | `0 / 0` | `0 / 0` |
| submitted / completed compile sections | `7040 / 7040` | `7024 / 7040` |
| uploaded sections | `2143` | `2143` |
| total remesh / upload / render | `177.960 / 179.835 / 2474.734ms` | `99.324 / 133.219 / 2610.277ms` |
| deadline-skipped compile requests | `0` | `0` |
| final target pending render chunks | `0` | `0` |

Decision: persisted-world render streaming confirms the queue-depth lever. With
the simulation tail frozen, the legacy quiescence marker equals
`first_initial_target_render_complete_ms`; queue depth `4` cuts persisted RD10
initial render completion from `19.744s` to `4.889s` without frame-budget
misses. This satisfies the desktop persisted slice gate, but it is still not a
shipping default decision until the Quest repeat/accept-upload loop is closed.

### 2026-07-07 - Tactical 150 Slice 4 Corrected Render Gate

Commit reported by benchmark JSON: `4ed7f77b`.

Note: `git_dirty=true` because this was captured while adding
`first_initial_target_render_complete_ms` and the benchmark-only
`--freeze-scheduled-fluid-ticks` startup-streaming flag. Defaults are unchanged:
scheduled fluid ticks run normally unless the benchmark flag is present, and the
render compile in-flight cap still equals `--render-compile-workers` unless
`--render-compile-max-pending-jobs` is passed.

Commands:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 6000 --target-hz 60 --debug-passive-showcase false > /tmp/mclone-150-corrected-rd10-live-default.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 3000 --target-hz 60 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-corrected-rd10-frozen-default.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 3000 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --freeze-scheduled-fluid-ticks --debug-passive-showcase false > /tmp/mclone-150-corrected-rd10-frozen-queue4.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 3000 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --debug-passive-showcase false > /tmp/mclone-150-corrected-rd10-live-queue4.json
```

RD10 startup-streaming, adaptive publication default on, workers `1`, 60 Hz
budget:

| Metric | Live default | Frozen fluids default | Frozen fluids queue 4 | Live queue 4 |
|---|---:|---:|---:|---:|
| startup playable | 537.478 ms | 537.314 ms | 532.369 ms | 524.323 ms |
| first full view ready | 9729.518 ms | 9227.596 ms | 9681.505 ms | 9671.935 ms |
| first initial target render complete | 23043.021 ms | 20058.747 ms | 9681.505 ms | 9671.935 ms |
| first target render quiescent | 50141.657 ms | 20058.747 ms | 10950.408 ms | 50135.632 ms |
| total fluid executed ticks | 8476 | 0 | 0 | 7746 |
| total fluid mutated blocks | 4419 | 0 | 0 | 4180 |
| p95 / p99 frame | 7.088 / 7.443 ms | 7.075 / 7.485 ms | 7.086 / 7.496 ms | 8.041 / 9.194 ms |
| max frame | 8.846 ms | 8.509 ms | 9.140 ms | 11.035 ms |
| over-budget frames | 0 | 0 | 0 | 0 |
| submitted / completed compile sections | 7395 / 7377 | 7136 / 7120 | 7168 / 7168 | 8212 / 8208 |
| uploaded sections | 2440 | 2173 | 2190 | 3230 |
| post-full-view target rebuilt sections | 4019 | 3952 | 0 | 604 |
| deadline-skipped compile requests | 0 | 0 | 0 | 0 |

Decision: the prior Candidate B queue-depth falsification was a measurement
failure of the old quiescence gate, not evidence that the lever is ineffective.
With fluids live, `first_target_render_quiescent_ms` remains ~`50.1s` because
scheduled fluid mutation continues to dirty render sections. With scheduled
fluids frozen, the old marker converges with the new initial-render marker in
the default lane (`20058.747ms`). Against the corrected
`first_initial_target_render_complete_ms` gate, queue depth `4` finishes at
`9671.935ms` live and `9681.505ms` frozen, under the RD10 `<= 14s` acceptance
target with no frame-budget misses. This is a desktop corrected-gate acceptance
row for the queue-depth lever, not a default promotion: Quest/persisted/XR
accept-upload gates still need their promotion loop before shipping defaults
change.

### 2026-07-07 - Tactical 150 Slice 4 Render Compile Queue-Depth Candidate

Commit reported by benchmark JSON: `72659d86`.

Note: `git_dirty=true` because this was captured while adding the opt-in
`--render-compile-max-pending-jobs` benchmark/config flag. The default remains
unchanged: when the flag is omitted, the compile in-flight cap equals
`--render-compile-workers`.

Command:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 6000 --target-hz 60 --render-compile-workers 1 --render-compile-max-pending-jobs 4 --debug-passive-showcase false > /tmp/mclone-150-slice4-rd10-queue4.json
```

RD10 startup-streaming, adaptive publication default on, workers `1`,
compile max-pending jobs `4`, 60 Hz budget:

| Metric | Value |
|---|---:|
| startup playable | 529.048 ms |
| first full view ready | 9722.828 ms |
| first render quiescent (legacy global suffix) | 50139.993 ms |
| first target render quiescent | 50139.993 ms |
| average frame | 5.732 ms |
| p95 / p99 frame | 7.525 / 8.468 ms |
| max frame | 11.238 ms |
| over-budget frames | 0 |
| over-2x-budget frames | 0 |
| submitted compile sections | 8161 |
| completed compile sections | 8156 |
| uploaded sections | 3178 |
| target rebuilt sections | 8156 |
| non-target rebuilt sections | 0 |
| post-full-view target rebuilt sections | 562 |
| post-full-view non-target rebuilt sections | 0 |
| deadline-skipped compile requests | 0 |

Superseded decision: this row used the old total target-render-quiescence gate
and is superseded by the corrected render-gate row above. It remains useful as
evidence that the old marker was fluid/world-settle dominated: queue depth `4`
kept frame pacing clean but could not move `first_target_render_quiescent_ms`
while scheduled fluids were still mutating the target view.

### 2026-07-07 - Tactical 150 Slice 4 Target Render Tail Follow-up

Commit reported by benchmark JSON: `962bcfc5`.

Note: `git_dirty=true` because this was captured while adding the target
render-work diagnostics split to the startup-streaming JSON. This is the one
allowed Candidate B diagnostics detour: the worker check below could not
distinguish target render work from non-target edge bookkeeping after the full
view became ready, and the consuming decision is whether Candidate B can be
promoted or remains falsified.

Command:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 6000 --target-hz 60 --render-compile-workers 1 --debug-passive-showcase false > /tmp/mclone-150-slice4-followup-rd10-target-split.json
```

RD10 startup-streaming, adaptive publication default on, workers `1`, 60 Hz
budget:

| Metric | Value |
|---|---:|
| startup playable | 537.341 ms |
| first full view ready | 9711.712 ms |
| first render quiescent (legacy global suffix) | 50148.741 ms |
| first target render quiescent | 50148.741 ms |
| average frame | 5.555 ms |
| p95 / p99 frame | 7.199 / 7.628 ms |
| max frame | 9.927 ms |
| over-budget frames | 0 |
| submitted compile sections | 7407 |
| completed compile sections | 7386 |
| uploaded sections | 2449 |
| target rebuilt sections | 7386 |
| non-target rebuilt sections | 0 |
| post-full-view target rebuilt sections | 4029 |
| post-full-view non-target rebuilt sections | 0 |
| deadline-skipped compile requests | 0 |
| final target pending render chunks | 0 |
| final global pending render chunks | 88 |

Interpretation: the long RD10 startup-streaming render tail is target render
work, not only edge-neighbor bookkeeping or a global quiescence-definition
artifact. The target-scoped quiescent marker lands on the same frame/time as
the legacy suffix marker, and all rebuilt sections after full-view readiness are
inside the render target (`4029` target, `0` non-target). The final `88` global
pending render chunks are still edge/non-target bookkeeping, but they are not
what delays the marker.

This keeps Candidate B falsified for promotion. The headless startup-streaming
lane already uses the existing deadline-shaped render sync path, has no
deadline-skipped compile requests, and still reaches target render quiescence
at ~50.1s. The live desktop opt-in render-admission controller remains useful
plumbing, but this gate cannot promote render admission, workers, XR
accept/upload, or Quest defaults until there is a measured lever for target
render mesh progression.

### 2026-07-07 - Tactical 150 Slice 4 Render Worker Check

Commit reported by benchmark JSON: `e48245db`.

Note: `git_dirty=true` because this was captured while adding the opt-in live
desktop render-admission controller wiring. The startup-streaming lane below is
used for the render-worker default decision; it does not exercise the live
window-only `--adaptive-render-admission-budget` flag.

Commands:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 6000 --target-hz 60 --render-compile-workers 1 --debug-passive-showcase false > /tmp/mclone-150-slice4-rd10-workers1.json
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mclone-native-client -- --startup-streaming-perf --render-distance 10 --startup-streaming-frames 6000 --target-hz 60 --render-compile-workers 2 --debug-passive-showcase false > /tmp/mclone-150-slice4-rd10-workers2.json
```

RD10 startup-streaming, adaptive publication default on, 60 Hz budget:

| Metric | Workers 1 | Workers 2 |
|---|---:|---:|
| startup playable | 527.186 ms | 545.541 ms |
| first full view ready | 9712.257 ms | 9221.628 ms |
| first render quiescent | 50136.512 ms | 50124.487 ms |
| average frame | 5.469 ms | 5.615 ms |
| p95 / p99 frame | 7.076 / 7.439 ms | 7.205 / 7.845 ms |
| max frame | 9.298 ms | 10.052 ms |
| over-budget frames | 0 | 0 |
| total remesh | 439.307 ms | 820.441 ms |
| total upload | 177.954 ms | 190.278 ms |
| total render | 4163.235 ms | 4235.862 ms |
| submitted compile sections | 7380 | 7759 |
| completed compile sections | 7393 | 7772 |
| uploaded sections | 2446 | 2804 |
| deadline-skipped compile requests | 0 | 0 |

Decision: workers `2` does not reproduce the earlier live quiescence win on
this clean RD10 startup-streaming check. Full-view readiness improves by about
5%, but render quiescence is unchanged at ~50.1s and remesh/frame tails are
worse. Do not promote the desktop render compile worker default from `1` to `2`
from this evidence.

Tail attribution note: the target-split follow-up above supersedes the earlier
interpretation from coarse queue samples. The tail is target render work after
full-view readiness, while the final `88` global pending render chunks are
edge/non-target bookkeeping.

### 2026-07-07 - Tactical 150 Slice 3 Promotion Closure Measurements

Benchmarked commit: `9da26f88` (`Fix app-runtime wasm all-target coverage bin`),
clean tree before doc edits. Host: `kmacbook`, Apple M4 Pro, macOS `26.5.1`
build `25F80`.

Raw artifacts were captured under `/tmp`:

- `/tmp/mclone-150-slice3-cadence-rd10.json`
- `/tmp/mclone-150-slice3-desktop-xr-smoke.log`
- `/tmp/mclone-150-slice3-native-window-smoke.log`
- `/tmp/mclone-150-slice3-native-window-present.json`
- `/tmp/mclone-150-slice3-native-window-control.json`

Full RD10 cadence-compatibility row:

```sh
cargo run --release --manifest-path native/Cargo.toml \
  -p mclone-native-client --bin mclone-native-client -- \
  --startup-streaming-perf --render-distance 10 \
  --startup-streaming-frames 6000 --target-hz 60 \
  --simulation-cadence 60/10/60 \
  --adaptive-chunk-publication-budget true \
  --debug-passive-showcase false
```

JSON reported `git_commit=9da26f88`, `git_dirty=false`,
`debug_assertions=false`.

| Lane | Cadence | Frames / Hz | Playable | Full view | Render quiescent | Over budget | Over 2x | p95 / p99 / max | Feature/light published | Max publish | Max pending publication chunks |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| RD10 cadence compatibility | `60/10/60` | `6000 / 60` | `1.265s` | `15.936s` | `113.925s` | `0` | `0` | `8.907 / 10.782 / 12.955ms` | `3264 / 3335` | `2.762ms` | `156` |

Decision trace:

| Family | Host/window/stage | Reason | Target period | Grant | Max units | Per-unit estimate |
|---|---|---|---:|---:|---:|---:|
| feature publication | integrated-server-runner / gameplay-tick / scheduler-publication | `hold-at-cap` | `100.0ms` | `20.000ms` | `4` | `0.356ms` |
| light publication | integrated-server-runner / gameplay-tick / scheduler-publication | `hold-at-cap` | `100.0ms` | `20.000ms` | `4` | `0.143ms` |
| feature-job admission | integrated-server-runner / gameplay-tick / terrain-generation | `hold-at-cap` | `100.0ms` | `0.000ms` | `1`, cap `256` pending units | n/a |

Interpretation: the full diagnostic cadence row now has a real populated
budget-decision trace and confirms the controller reads the configured
`100.0ms` gameplay period instead of using a baked `50ms` period. It stayed
inside the 60 Hz frame budget, but the lower gameplay rate materially changes
streaming semantics and render quiescence, so this row is compatibility
evidence, not a default-cadence acceptance row.

Desktop XR functional smoke:

```sh
bash ./scripts/start-xr.sh --runtime wivrn --wivrn-usb --smoke mclone \
  --frames 120 --view-pose 0,120,-96,180 \
  --adaptive-chunk-publication-budget true \
  --debug-passive-showcase false
```

Result: passed. WiVRn over USB selected the Quest 3 headset, submitted `120`
frames (`runtime_frames=121`, `skipped=1`), and printed
`desktop OpenXR mclone smoke complete: frames=120`. The final Mclone XR frame
summary reported `frames=71`, `sections=59`, `drawn_sections=41`,
`indices=258120`, `drawn_indices=119736`.

Native-window functional smoke:

```sh
cargo run --release --manifest-path native/Cargo.toml \
  -p mclone-native-client --bin mclone-native-client -- \
  --start-in-world true --transient --startup-wait playable \
  --render-distance 10 --adaptive-chunk-publication-budget true \
  --debug-passive-showcase false
```

The bounded live-window run reached playable startup and streamed to the RD10
target (`loaded=529` chunks) without logged render errors. The current
interactive window CLI does not emit quantitative present-path frame timing, so
this is a winit/swapchain functional smoke, not a full present-pacing gate row.

Native-window quantitative present-path A/B:

```sh
cargo run --release --manifest-path native/Cargo.toml \
  -p mclone-native-client --bin mclone-native-client -- \
  --window-frame-report /tmp/mclone-150-slice3-native-window-present.json \
  --window-frame-report-frames 3600 \
  --start-in-world true --transient --startup-wait playable \
  --render-distance 10 --simulation-cadence 60/10/60 \
  --adaptive-chunk-publication-budget true \
  --debug-passive-showcase false
```

Control used the same command with
`--window-frame-report /tmp/mclone-150-slice3-native-window-control.json` and
`--adaptive-chunk-publication-budget false`.

Both rows were measured on harness commit `c1ac218b`
(`Add native window frame report`), `git_dirty=false`, release build, VSync
`fifo` on a `120Hz` monitor (`8.333ms` target), and `3600` presented frames
with no skipped or reconfigured surface frames.

| Native-window row | Loaded / target chunks | Pending jobs / publications | Strict over budget | Over 2x / 4x | Frame p50 / p95 / p99 / max | Render p95 / p99 | Surface present p95 / p99 / max |
|---|---:|---:|---:|---:|---:|---:|---:|
| adaptive publication | `529 / 529` | `0 / 0` | `1802` | `3 / 1` | `8.335 / 9.394 / 9.600 / 37.736ms` | `8.289 / 8.479ms` | `0.006 / 0.011 / 0.030ms` |
| fixed publish floor control | `233 / 529` | `11 / 34` | `1794` | `7 / 1` | `8.333 / 8.694 / 9.504 / 36.618ms` | `8.250 / 8.918ms` | `0.008 / 0.028 / 0.066ms` |

Warmup-excluding the first `120` samples removed the startup-only `>4x` frame:
adaptive had `2` over-2x and `0` over-4x frames with frame
p95/p99/max `9.396 / 9.600 / 17.826ms`; the fixed-floor control had `6`
over-2x and `0` over-4x frames with `8.691 / 9.294 / 17.070ms`. The strict
single-budget count is not a useful pass/fail field for live 120Hz VSync
because half the samples sit a few microseconds over `8.333ms`; severe hitches
and present-call timing are the useful comparison. Adaptive reached the full
RD10 view in the finite window, left no server publication backlog, and did
not add severe present-path hitches relative to fixed-floor control.

### 2026-07-07 - Tactical 150 Slice 3 Budget Decision Panel Smoke

Commit: this diagnostics checkpoint commit. Host: `kmacbook`, Apple M4 Pro,
macOS `26.5.1` build `25F80`.

Raw JSON artifacts were captured under `/tmp`:
`/tmp/mclone-150-slice3-budget-panel-smoke.json` and
`/tmp/mclone-150-slice3-budget-panel-cadence-smoke.json`.

Both rows used
`--startup-streaming-perf --startup-streaming-frames 60 --render-distance 5 --target-hz 60 --adaptive-chunk-publication-budget true --debug-passive-showcase false`.
The second row also used `--simulation-cadence 60/10/60`.

| Lane | Cadence | Decisions | Feature decision | Light decision | Over budget | Max pending publication chunks |
|---|---:|---:|---|---|---:|---:|
| budget panel smoke | `60/20/60` | `3` | `hold-at-cap`, max units `4`, grant `10.000ms`, per-unit `0.352ms` | `hold-at-cap`, max units `4`, grant `10.000ms`, per-unit `0.108ms` | `0` | `108` |
| cadence smoke | `60/10/60` | `3` | `hold-hysteresis`, target `100.0ms`, grant `14.000ms`, max units `4`, per-unit `0.353ms` | `hold-hysteresis`, target `100.0ms`, grant `14.000ms`, max units `4`, per-unit `0.109ms` | `0` | `112` |

Interpretation: the startup-streaming JSON `frame_pipeline_accounting`
`budgetDecisionPanel` is now populated with the scheduler's real publication
decisions. The non-default cadence row verifies the trace records the
`100.0ms` gameplay period, but it is a short diagnostics smoke, not the full
RD10 cadence-compatibility promotion row.

### 2026-07-06 - Tactical 150 Slice 3 Adaptive Publication Desktop Checkpoint

Commit: this checkpoint commit (benchmarks captured from the same worktree
contents before the commit was created; base before edits was `704f6373`). Host: `kmacbook`, Apple
M4 Pro, macOS `26.5.1` build `25F80`.

Raw JSON artifacts were captured under `/tmp`:
`/tmp/mclone-rd10-adaptive-1.json`,
`/tmp/mclone-rd10-adaptive-2.json`,
`/tmp/mclone-rd15-adaptive.json`,
`/tmp/mclone-movement-frame-adaptive.json`, and
`/tmp/mclone-movement-frame-control.json`.

Startup-streaming rows used
`--adaptive-chunk-publication-budget true --debug-passive-showcase false`.

| Lane | Run | RD | Frames / Hz | Playable | Full view | Render quiescent | Over budget | Over 2x | p95 / p99 / max | Max publish | Max pending publication chunks |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| fresh startup adaptive | 1 | 10 | `6000 / 60` | `0.530s` | `9.730s` | `50.196s` | `0` | `0` | `7.010 / 7.450 / 8.961ms` | `1.847ms` | `124` |
| fresh startup adaptive | 2 | 10 | `6000 / 60` | `0.512s` | `9.700s` | `50.171s` | `0` | `0` | `6.806 / 7.320 / 8.485ms` | `1.851ms` | `124` |
| fresh startup adaptive | 1 | 15 | `9000 / 60` | `0.717s` | `20.022s` | `50.238s` | `24` | `2` | `10.594 / 12.252 / 39.042ms` | `2.021ms` | `179` |

Movement-frame same-commit control:

| Lane | Frames / Hz | Over budget | Over 2x | p95 / p99 / max | Accounting violations |
|---|---:|---:|---:|---:|---:|
| adaptive flag | `240 / 120` | `1` | `1` | `2.551 / 2.703 / 19.358ms` | `0` |
| control | `240 / 120` | `1` | `1` | `2.548 / 2.830 / 17.131ms` | `0` |

Interpretation:

- RD10 reproduced the publication win twice: full view reached `9.70-9.73s`
  with zero 60 Hz budget misses.
- RD15 full view reached `20.022s`, a `2.86x` improvement over the Slice 0
  `57.214s` baseline, but retained the known RD15 tail spikes (`24` over
  budget, `2` over 2x).
- Render quiescence stayed around `50s`, so Candidate A opens publication but
  does not close the render-admission/mesh tail. The frame-pipeline
  `budgetDecisionPanel` remained empty in these flat perf rows; full promotion
  still needs decision-trace/report wiring before default-on.

### 2026-07-06 - Tactical 150 Slice 0 Desktop Clean Baselines

Commit: `bc55076c` (`Fix throughput guardrail measurement gates`), clean tree
for every benchmark JSON row (`git_dirty=false`). Host: `kmacbook`, Apple M4
Pro, macOS `26.5.1` build `25F80`.

Raw JSON artifacts were captured under `/tmp/mclone-142-baselines/`.

Startup-streaming rows:

| Lane | Run | RD | Frames / Hz | Playable | Full view | Render quiescent | Over budget | Over 2x | p95 / p99 / max |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| fresh startup | 1 | 10 | `6000 / 60` | `1.066s` | `27.588s` | `56.651s` | `0` | `0` | `7.963 / 8.886 / 16.573ms` |
| fresh startup | 2 | 10 | `6000 / 60` | `1.074s` | `27.526s` | `56.746s` | `0` | `0` | `7.781 / 8.718 / 16.223ms` |
| fresh startup | 1 | 15 | `9000 / 60` | `1.267s` | `57.214s` | `82.718s` | `0` | `0` | `11.107 / 12.147 / 16.174ms` |
| fresh startup | 2 | 15 | `9000 / 60` | `1.196s` | `57.255s` | `82.779s` | `26` | `2` | `11.774 / 13.378 / 39.451ms` |
| persisted startup | 1 | 10 | `2400 / 60` | `0.125s` | `1.018s` | `25.141s` | `0` | `0` | `7.926 / 8.883 / 13.095ms` |
| persisted startup | 2 | 10 | `2400 / 60` | `0.112s` | `1.022s` | `25.146s` | `0` | `0` | `8.161 / 9.038 / 13.021ms` |

Frame probes:

| Lane | Run | Frames / Hz | Top-level over budget | Top-level over 2x | Accounting app over-period | p95 / p99 / max |
|---|---:|---:|---:|---:|---:|---:|
| frame-budget | 1 | `240 / 120` | `1` | `1` | `0` | `3.253 / 3.467 / 17.713ms` |
| frame-budget | 2 | `240 / 120` | `1` | `1` | `0` | `2.928 / 3.470 / 17.775ms` |
| movement-frame | 1 | `240 / 120` | `1` | `1` | `0` | `3.905 / 4.550 / 18.668ms` |
| movement-frame | 2 | `240 / 120` | `1` | `1` | `0` | `3.812 / 4.230 / 18.702ms` |

Interpretation:

- RD10 full-view readiness still pins the publication wall at `~27.5s`, but the
  current clean render-quiescent semantics settle at `~56.7s`, not the older
  `~33s` dirty-control row.
- RD15 full-view is stable at `~57.2s`; render quiescence is stable at
  `~82.7s`, but one row had a `39.451ms` tail frame and `26` app over-period
  frames.
- Persisted RD10 remains generation/light-free and stable: full view
  `~1.02s`, actionable render idle `~25.14s`, and `0/2400` over-budget frames.

### 2026-07-06 - Desktop Loopback Remote Contrast Smoke

Commit: `4f86dad3` (`Add shared debug diagnostics toggle`), clean tree before
capture. Host: Apple M4 Pro Mac, Darwin arm64.

Command:

```sh
pnpm native:remote:smoke
```

Result: pass. The smoke built `mclone-dedicated-server` and
`mclone-native-client`, started a loopback dedicated server, launched two
headless remote native clients, and saved rendered screenshots to:

- `/tmp/mclone-native-remote-client-smoke.png`
- `/tmp/mclone-native-remote-client-smoke-observer.png`

Desktop remote smoke summary:

| Client | Render distance | Sections / drawn sections | GUI commands | Remote players | Entities | Actors / drawn actors |
|---|---:|---:|---:|---:|---:|---:|
| actor | `2` | `166 / 12` | `12819` | `0` | `2` | `2 / 2` |
| observer | `2` | `166 / 37` | `13051` | `1` | `2` | `3 / 3` |

Dedicated server summary excerpts:

| Phase | Connection | Commands | Updates sent | Tick total | Scheduler tick | Active sessions |
|---|---:|---:|---:|---:|---:|---:|
| active | `#1` | `1` | `93` | `2.531ms` | `0.924ms` | `2` |
| active | `#2` | `2` | `149` | `3.318ms` | `1.701ms` | `2` |
| final | `#2` | `12` | `300` | `23.214ms` | `15.648ms` | `1` |
| final | `#1` | `14` | `307` | `25.462ms` | `17.679ms` | `0` |

Gap: there is still no desktop remote-host startup-streaming perf lane.
`--startup-streaming-perf` currently requires the local integrated server path,
so this record is a desktop-shaped remote rendering/connectivity contrast and
server-summary proof, not a desktop remote streaming baseline. Do not create a
one-off permanent lane just for this row; add a real remote startup/streaming
lane only if later policy work needs desktop remote startup numbers.

### 2026-07-05 - Persisted World Startup-Streaming Split

Commit reported by benchmark JSON: `14ceba0a`.

`git_dirty=true`: this record was captured while adding
`--startup-streaming-persisted-world`, hardening the startup-streaming
quiescent marker, fixing native-client package scripts to specify
`--bin mclone-native-client`, and editing docs. Treat it as a directional
baseline for the new desktop-shaped persisted lane, not a clean release gate.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Command:

```sh
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client \
  --bin mclone-native-client -- \
  --startup-streaming-persisted-world \
  --render-distance 10 \
  --startup-streaming-frames 2400 \
  --target-hz 60 \
  --debug-passive-showcase false \
  | tee /tmp/mclone-startup-streaming-persisted-rd10-release-2400.json
```

Raw local artifact:
`/tmp/mclone-startup-streaming-persisted-rd10-release-2400.json`.

The benchmark prewarms a temp SQLite world through the normal local integrated
server path, drops that runtime, then reopens the same world through
`WindowSceneStartupPump`. The measured startup/streaming phase therefore uses
the normal desktop pump, update queue, render compile workers, upload budget,
and paced 60 Hz headless frame loop, but it should not run fresh worldgen or
lighting. The report's stable render-quiescent marker is now post-processed
from the whole frame record and requires no actionable ready render work through
the end of the capture; transient idle frames no longer count as settled.

| Lane | Prewarm / storage | Playable | Full target view | Stable render idle | Frame pacing |
|---|---:|---:|---:|---:|---:|
| RD10 persisted startup-streaming, 60 Hz, workers 1 | `33.421s`, `529/529` target chunks, `34.4 MB` SQLite | `0.125s` | `1.031s` | `25.142s` | `0/2400` over budget, p95 `7.030ms`, max `11.020ms` |

Runtime totals during the measured frame loop:

| Measure | Value |
|---|---:|
| Total poll / scheduler publish-completed time | `105.824ms` / `953.132ms` |
| Total remesh / upload / render time | `313.664ms` / `169.804ms` / `1621.953ms` |
| Submitted / uploaded sections | `7131` / `2232` |
| Final target chunks / cached sections | `529/529` / `2149` |
| Final pending render chunks / actionable render work | `88` / `false` |

Interpretation:

- Already-generated local play enters quickly. The measured reopen reaches the
  playable gate in `0.125s` and the full target chunk view in `1.031s`.
- The many-second persisted tail is not server reload, generation, lighting, or
  GPU upload. Scheduler publish counters stay at `0` generated/light chunks,
  upload totals only `170ms`, and frame pacing stays clean.
- The remaining desktop-shaped persisted wall is render admission/mesh
  progression spread over frames: stable actionable render idle arrives at
  `25.142s`. That matches the earlier CPU-mesh evidence that mesh/admission is
  the next already-generated render tail, but this lane shows how it behaves
  under the real frame loop and budgets.
- `pending_render_chunks=88` at the end while `ready_render_work_pending=false`
  means dirty/boundary bookkeeping can remain after all currently actionable
  ready render work is drained. Optimization and regression checks should use
  both counters, not only one.

### 2026-07-05 - Prebuilt Mesh GPU Upload Split

Commit reported by benchmark JSON: `bbbb3a75`.

`git_dirty=true`: this record was captured while adding `--gpu-upload`,
package scripts, and docs. Treat as a first directional baseline for the
upload-only lane, not a clean release gate.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Command shape:

```sh
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mesh_cpu_perf -- \
  --render-distance N --gpu-upload --max-seconds 180 \
  > /tmp/mclone-gpu-upload-rdN-persisted-sqlite-20260705.json
```

Raw local artifacts:
`/tmp/mclone-gpu-upload-rd5-persisted-sqlite-20260705.json` and
`/tmp/mclone-gpu-upload-rd10-persisted-sqlite-20260705.json`.

The benchmark still prewarms/reloads persisted snapshots and builds CPU meshes
first, then times only the prebuilt section upload phase. Draw-resource setup
uploads the texture atlas separately; `update_sections_ms` is the measured
section mesh upload through `TexturedSectionDrawResources`.

| Lane | Prebuilt mesh output | Section upload | Uploaded bytes | Main upload subphases |
|---|---:|---:|---:|---|
| RD5 persisted prebuilt meshes | `965` non-empty sections, `739k` faces | `50.902ms` | `136.0 MB` | vertex bytes `21.624ms`, vertex buffers `18.927ms`, index buffers `7.749ms` |
| RD10 persisted prebuilt meshes | `2789` non-empty sections, `1.96M` faces | `133.113ms` | `360.4 MB` | vertex bytes `53.459ms`, vertex buffers `51.312ms`, index buffers `21.570ms` |

RD10 setup detail: headless device creation `15.000ms`, draw-resource setup
including atlas upload `17.032ms`, post-upload device poll `0.003ms`.

Interpretation:

- Desktop GPU upload is not the multi-second wall. RD10 uploads about
  `360 MB` of prebuilt section data in `133ms`, while the same run spends
  `10.236s` building CPU meshes.
- The measured upload cost is still too large to dump into one Quest frame and
  should remain budgeted, but it is an order-of-magnitude smaller than mesh CPU
  for the desktop RD10 already-generated case.
- The expensive upload subphase is not GPU completion wait; it is CPU-side
  vertex serialization plus buffer creation. `device.poll` after upload was
  effectively zero on this desktop run.
- The desktop-shaped persisted-world startup-streaming split now confirms the
  same already-generated tail inside the paced runtime: RD10 reaches full target
  view in `1.031s`, but stable actionable render idle takes `25.142s` with clean
  frame pacing.

### 2026-07-05 - Persisted Snapshot Mesh CPU Split

Commit reported by benchmark JSON: `4858ca1f`.

`git_dirty=true`: this record was captured while adding the new
`mesh_cpu_perf` binary, package scripts, and docs. Treat as a first directional
baseline for the mesh CPU-only lane, not a clean release gate.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Command shape:

```sh
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client --bin mesh_cpu_perf -- \
  --render-distance N --max-seconds 180 \
  > /tmp/mclone-mesh-cpu-rdN-persisted-sqlite-20260705.json
```

Raw local artifacts:
`/tmp/mclone-mesh-cpu-rd5-persisted-sqlite-20260705.json` and
`/tmp/mclone-mesh-cpu-rd10-persisted-sqlite-20260705.json`.

The benchmark prewarms a temp SQLite world, reloads already-generated/lit
snapshots through `ChunkScheduler`, then measures CPU mesh construction for all
target-column render sections. The mesh phase uses
`build_render_sections_from_snapshots_with_biome_zoom_seed`, the same shared
path used by the render compile worker. It does not create a `wgpu` device,
upload buffers, run the desktop frame loop, or apply the runtime's per-frame
admission budget.

| Lane | Target chunks / sections | Persisted reload ready / settled | Mesh CPU build | Mesh throughput | Mesh output |
|---|---:|---:|---:|---:|---:|
| RD5 persisted snapshots | `169` / `2704` | `0.079s` / `0.098s` | `1.624s` | `1665` sections/sec (`594` non-empty/sec) | `965` non-empty sections, `739k` faces |
| RD10 persisted snapshots | `529` / `8464` | `0.540s` / `0.626s` | `11.037s` | `767` sections/sec (`253` non-empty/sec) | `2789` non-empty sections, `1.96M` faces |

RD10 detail: asset catalog load `53.931ms`, `529` reloaded snapshots,
`625` stored chunk records, SQLite storage `35.5 MB`, visibility-graph total
`92.171ms`.

Interpretation:

- With persisted snapshots, server reload is sub-second at RD10, but CPU mesh
  construction alone is an `11.0s` wall for a full target-column build. This is
  now the largest measured already-generated startup cost.
- Mesh throughput drops from `1665` sections/sec at RD5 to `767` sections/sec
  at RD10, and non-empty throughput drops from `594` to `253` sections/sec.
  That scaling is consistent with the earlier render-quiescence tail and makes
  mesh CPU/admission a real optimization target after publication.
- This is still not a desktop-shaped frame-pacing result. The real runtime
  spreads compile/admit/upload work across frames and workers, so use this lane
  to price CPU work and output size, then validate changes with
  `startup-streaming` and Quest guardrails.
- The later GPU upload-only and desktop-shaped persisted startup-streaming
  records complete this split: upload is small on desktop, while the paced
  already-generated render tail remains many seconds.

### 2026-07-05 - Temp SQLite Persisted Server Reload Split

Commit reported by benchmark JSON: `b3950af8`.

`git_dirty=true`: this record was captured while adding
`--persisted-sqlite-reload`, package scripts, and docs. Treat as a first
directional baseline for the new temp-SQLite lane, not a clean release gate.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Command shape:

```sh
cargo run --release --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_loading_perf -- \
  --render-distance N --persisted-sqlite-reload --max-seconds 120 \
  > /tmp/mclone-scheduler-loading-rdN-persisted-sqlite-20260705.json
```

Raw local artifacts:
`/tmp/mclone-scheduler-loading-rd5-persisted-sqlite-20260705.json` and
`/tmp/mclone-scheduler-loading-rd10-persisted-sqlite-20260705.json`.

The benchmark performs a fresh prewarm pass through the normal SQLite
`open_world_dir` path in a temp world directory, then creates a fresh scheduler
over that same directory and measures reload. Reload validation fails if any
worldgen job, light status, or light compute runs. `sqlite_storage_bytes`
includes the main database plus WAL/SHM sidecar files when present.

| Lane | Target chunks | Stored chunk records | SQLite storage | Prewarm ready / settled | Reload ready / settled | Reload chunks/sec | Reload worldgen/light |
|---|---:|---:|---:|---:|---:|---:|---:|
| RD5 SQLite reload | `169` | `225` | `13.1 MB` | `3.599s` / `4.594s` | `0.076s` / `0.093s` | `2238.3` ready / `1810.3` settled | `0` jobs, `0` light statuses |
| RD10 SQLite reload | `529` | `625` | `35.5 MB` | `12.912s` / `14.934s` | `0.572s` / `0.721s` | `925.0` ready / `733.7` settled | `0` jobs, `0` light statuses |

RD10 reload detail: `625` loaded snapshots, `35` polls, `5113` scheduler
events, max poll call `30.172ms`.

Interpretation:

- Temp SQLite persisted reload is close to the in-memory reload lane at RD5 and
  modestly slower at RD10 (`0.572s` view-ready vs `0.523s` memory; `0.721s`
  settled vs `0.607s` memory). The real store path adds decode/load/poll cost,
  but not a multi-second wall.
- Persisted server reload remains far faster than fresh generation plus light.
  The many-second fresh path is still explained by generation/light/admission,
  not by loading already-generated records.
- The next split should leave the server-only lane and measure already-loaded
  snapshots through mesh CPU build and GPU upload separately. That answers how
  much of desktop render-quiescent tail is client render work once generation
  and light are removed.

### 2026-07-05 - In-Memory Persisted Server Reload Split

Commit reported by benchmark JSON: `4db978ed`.

`git_dirty=true`: this record was captured while adding
`--persisted-memory-reload`, package scripts, and docs. Treat as a first
directional baseline for the new persisted-memory lane, not a clean release
gate.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Command shape:

```sh
cargo run --release --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_loading_perf -- \
  --render-distance N --persisted-memory-reload --max-seconds 120 \
  > /tmp/mclone-scheduler-loading-rdN-persisted-memory-20260705.json
```

Raw local artifacts:
`/tmp/mclone-scheduler-loading-rd5-persisted-memory-20260705.json` and
`/tmp/mclone-scheduler-loading-rd10-persisted-memory-20260705.json`.

The benchmark performs a prewarm pass into a shared in-memory `WorldStore`, then
creates a fresh scheduler over the same store and measures reload. Reload
validation fails if any worldgen job, light status, or light compute runs.

| Lane | Target chunks | Stored chunk records | Prewarm ready / settled | Reload ready / settled | Reload chunks/sec | Reload worldgen/light |
|---|---:|---:|---:|---:|---:|---:|
| RD5 memory reload | `169` | `225` | `3.647s` / `4.628s` | `0.074s` / `0.092s` | `2284.2` ready / `1843.2` settled | `0` jobs, `0` light statuses |
| RD10 memory reload | `529` | `625` | `12.993s` / `14.986s` | `0.523s` / `0.607s` | `1012.1` ready / `871.4` settled | `0` jobs, `0` light statuses |

RD10 reload detail: `625` loaded snapshots, `35` polls, `5113` scheduler
events, max poll call `12.700ms`.

Interpretation:

- Already-generated/lit chunks are not expensive in the server-only in-memory
  path compared with fresh light generation. RD10 drops from about `13s`
  prewarm view-ready to `0.523s` reload view-ready.
- This lane isolates scheduler load/publication over resident `ChunkRecord`s.
  It intentionally excludes disk IO/decode beyond in-memory clone cost, client
  update pump, mesh preparation, GPU upload, and frame pacing.
- Next separation step is the temp SQLite persisted reload lane. That should
  price real record encode/decode/storage separately from generation/light,
  before moving to persisted desktop startup-streaming and Quest persisted-world
  guardrails.

### 2026-07-05 - Raw Worldgen vs Server-Only Loading Ceiling Split

Commit reported by benchmark JSON: `ff0e8f57`.

`git_dirty=true`: this record was captured while adding the
`scheduler_loading_perf` benchmark binary, raising `worldgen_perf`'s benchmark
radius cap, and editing docs/package scripts. Treat these as ceiling-orientation
numbers, not clean release gates.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Commands:

```sh
cargo run --release --manifest-path native/Cargo.toml -p mclone-worldgen --bin worldgen_perf -- \
  --radius 11 --iterations 1 \
  > /tmp/mclone-worldgen-r11-current-20260705.json

cargo run --release --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_loading_perf -- \
  --render-distance 10 --max-seconds 120 \
  > /tmp/mclone-scheduler-loading-rd10-current-20260705.json

cargo run --release --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_loading_perf -- \
  --render-distance 10 --lighting false --max-seconds 120 \
  > /tmp/mclone-scheduler-loading-rd10-lighting-false-current-20260705.json
```

Additional spot checks:
`/tmp/mclone-worldgen-r5-current-20260705.json`,
`/tmp/mclone-worldgen-r8-current-20260705.json`,
`/tmp/mclone-scheduler-loading-rd5-current-20260705.json`,
`/tmp/mclone-scheduler-loading-rd5-lighting-false-current-20260705.json`, and
`/tmp/mclone-scheduler-loading-rd10-lighting-false-spin-current-20260705.json`.

RD10 maps to tracking radius `11`, or `529` target chunks.

| Lane | Lighting | Target chunks | Ready / phase time | Target chunks/sec | Important exclusions |
|---|---:|---:|---:|---:|---|
| Raw `worldgen_perf --radius 11` features cold | N/A | `529` | `0.959s` | `551.5` | no scheduler, light, publication, client, mesh, or frame loop |
| Raw `worldgen_perf --radius 11` features warm | N/A | `529` | `0.426s` | `1243.2` | dependency cache already warm |
| Server-only `scheduler_loading_perf` RD10 | off | `529` | `4.175s` view-ready / `4.647s` settled | `126.7` ready / `113.8` settled | no client update pump, mesh, GPU upload, or frame pacing |
| Server-only `scheduler_loading_perf` RD10 | on | `529` | `13.034s` view-ready / `15.047s` settled | `40.6` ready / `35.2` settled | no client update pump, mesh, GPU upload, or frame pacing |

RD5 spot checks:

| Lane | Lighting | Target chunks | Ready / phase time | Target chunks/sec |
|---|---:|---:|---:|---:|
| Raw `worldgen_perf --radius 5` features cold | N/A | `121` | `0.305s` | `396.8` |
| Raw `worldgen_perf --radius 5` features warm | N/A | `121` | `0.120s` | `1006.5` |
| Server-only `scheduler_loading_perf` RD5 | off | `169` | `0.869s` view-ready / `0.964s` settled | `194.5` ready / `175.3` settled |
| Server-only `scheduler_loading_perf` RD5 | on | `169` | `3.652s` view-ready / `4.648s` settled | `46.3` ready / `36.4` settled |

RD10 server-only counters:

| Lane | Feature jobs | Loaded snapshots | Completed light statuses | Total light compute | Max poll call |
|---|---:|---:|---:|---:|---:|
| Lighting off | `6` | `625` | `0` | `0.000s` | `14.322ms` |
| Lighting on | `6` | `625` | `625` | `10.784s` | `14.027ms` |

Interpretation:

- `~50` chunks/sec is not a desktop hardware ceiling. Raw feature generation for
  the same `529`-target RD10 footprint is `551.5` chunks/sec cold and
  `1243.2` chunks/sec with warm dependencies.
- Current server-shaped loading is much slower than raw feature generation even
  with lighting disabled (`126.7` chunks/sec ready). That gap includes current
  scheduler job batching/admission, one native worldgen worker, publication,
  holder/status bookkeeping, and cache/persistence plumbing.
- Lighting is a major server-only cost in this lane: RD10 drops from `126.7` to
  `40.6` ready chunks/sec with lighting enabled, and the benchmark attributes
  `10.784s` to light-status compute. This does not make client mesh irrelevant;
  it means server ceiling, desktop startup streaming, and render quiescence must
  remain separate rows when judging future levers.
- `--poll-mode spin` with lighting disabled was flat (`125.5` chunks/sec ready)
  versus completion-wait mode (`126.7`), so the server-only result is not a
  wait-loop artifact.

### 2026-07-05 - Desktop RD10 Publish-Budget And Render-Worker Lever Sweep

Commit reported by native benchmark JSON: `2603981e`.

`git_dirty=true`: doc edits were present, and each publish-budget run used a
temporary two-line local prototype changing
`DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET` and
`DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET`. The constants were reverted after the
runs; no code change from this sweep is intended to land.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Common command shape:

```sh
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --startup-streaming-perf --render-distance 10 --startup-streaming-frames 3000 \
  --target-hz 60 --debug-passive-showcase false \
  > /tmp/mclone-rd10-publish-budget-N-current-20260705.json
```

The startup JSON's `startup_target_chunk_count` is the playable `3x3` gate, so
the chunks/sec below use `final.target_chunk_count=529`.

Publish-budget sweep, render workers `1`, lighting enabled:

| Feature/light publish budget | Playable | Full-view ready | Full-view chunks/sec | Render quiescent | Quiescent chunks/sec | Frame avg/p95/p99/max | Over budget | Max publish ms | Max pending publication chunks |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `1` | `1.067s` | `27.492s` | `19.2` | `32.540s` | `16.3` | `4.996 / 7.111 / 7.749 / 8.889ms` | `0` | `1.781` | `125` |
| `4` | `0.478s` | `9.681s` | `54.6` | `20.089s` | `26.3` | `5.486 / 7.349 / 7.787 / 8.830ms` | `0` | `2.639` | `120` |
| `8` | `0.375s` | `9.716s` | `54.4` | `19.639s` | `26.9` | `5.705 / 7.779 / 8.446 / 12.496ms` | `0` | `3.279` | `112` |
| `16` | `0.298s` | `9.728s` | `54.4` | `21.200s` | `25.0` | `5.174 / 7.091 / 7.630 / 9.803ms` | `0` | `5.658` | `96` |
| `32` | `0.299s` | `10.221s` | `51.8` | `18.161s` | `29.1` | `5.571 / 7.440 / 8.674 / 27.793ms` | `2` | `1.075` | `0` |

Attribution probes with publish budget `4`:

| Probe | Full-view ready | Full-view chunks/sec | Render quiescent | Quiescent chunks/sec | Frame avg/p95/p99/max | Over budget |
|---|---:|---:|---:|---:|---:|---:|
| workers `1`, lighting on | `9.681s` | `54.6` | `20.089s` | `26.3` | `5.486 / 7.349 / 7.787 / 8.830ms` | `0` |
| workers `1`, lighting off | `8.154s` | `64.9` | `23.303s` | `22.7` | `6.158 / 8.458 / 9.184 / 15.488ms` | `0` |
| workers `2`, lighting on | `9.690s` | `54.6` | `11.412s` | `46.4` | `6.423 / 8.699 / 9.787 / 12.031ms` | `0` |
| workers `4`, lighting on | `9.719s` | `54.5` | `11.639s` | `45.5` | `6.106 / 8.935 / 10.366 / 16.410ms` | `0` |

Raw local artifacts:
`/tmp/mclone-rd10-publish-budget-1-current-20260705.json`,
`/tmp/mclone-rd10-publish-budget-4-current-20260705.json`,
`/tmp/mclone-rd10-publish-budget-8-current-20260705.json`,
`/tmp/mclone-rd10-publish-budget-16-20260705.json`,
`/tmp/mclone-rd10-publish-budget-32-20260705.json`,
`/tmp/mclone-rd10-publish-budget-4-lighting-false-20260705.json`,
`/tmp/mclone-rd10-publish-budget-4-workers2-20260705.json`, and
`/tmp/mclone-rd10-publish-budget-4-workers4-20260705.json`.

Interpretation:

- The full-view throughput curve saturates early. Raising publish budget from
  `1` to `4` gives the large gain (`19.2` -> `54.6` chunks/sec); `8` and `16`
  are flat, while `32` is not better for full-view and produced two 60 Hz
  over-budget frames. The first production budget calculation should therefore
  target measured elapsed work/backlog pressure, not a large fixed count.
- Disabling lighting under budget `4` improves full-view by only about `1.5s`
  (`54.6` -> `64.9` chunks/sec). Light/status work is real, but it is not the
  main remaining full-view wall after opening publication.
- With publish budget `4`, render compile workers `2` do not change full-view
  readiness but cut render quiescence from `20.089s` to `11.412s`; workers `4`
  do not improve further. This sharpens Candidate B: isolation bulk-drain
  worker sweeps stayed flat, but desktop-shaped live streaming after publication
  opens has an obvious workers-2 render-tail win.

### 2026-07-05 - Desktop Startup-Streaming Waterfall With Publication Counters

Commit/reporting note: RD10 reported `2ff3273e` with `git_dirty=true` while
concurrent source changes were still uncommitted; those source changes were
then committed as `f5f90f83` before RD15. RD15 reported `f5f90f83` with
`git_dirty=true` from doc-only edits. Treat this as directional current-state
evidence; rerun RD10/RD15 from a clean tree before using the numbers as gates.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Commands:

```sh
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --startup-streaming-perf --render-distance 10 --startup-streaming-frames 6000 \
  --target-hz 60 --debug-passive-showcase false \
  > /tmp/mclone-startup-waterfall-rd10-20260705.json

cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --startup-streaming-perf --render-distance 15 --startup-streaming-frames 9000 \
  --target-hz 60 --debug-passive-showcase false \
  > /tmp/mclone-startup-waterfall-rd15-20260705.json
```

Summary:

| RD | Target chunks | Playable | Full-view ready | Render quiescent | Frame avg/p95/p99/max | Over budget | Update queue max | Oldest applied age max | Pending publication chunks max |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `10` | `529` | `1.076s` | `27.553s` | `32.834s` | `5.344 / 6.537 / 7.350 / 13.156ms` | `0 / 6000` | `0` | `24.387ms` | `125` |
| `15` | `1089` | `1.094s` | `57.190s` | `64.253s` | `7.721 / 9.946 / 11.003 / 13.966ms` | `0 / 9000` | `0` | `31.355ms` | `125` |

Stage totals:

| RD | Scheduler publish total/max | Apply/dirty/client apply total | Remesh total | Upload total | Render total | Completed sections | Uploaded sections |
|---:|---:|---:|---:|---:|---:|---:|---:|
| `10` | `739.283ms` / `1.582ms` | `50.613 / 15.091 / 35.136ms` | `625.207ms` | `221.743ms` | `3850.108ms` | `7747` | `2779` |
| `15` | `1435.515ms` / `1.313ms` | `102.425 / 35.990 / 65.862ms` | `1526.454ms` | `403.384ms` | `10279.895ms` | `16307` | `5683` |

Publication/queue counters:

| RD | Feature chunks published | Light statuses published | Snapshot-ready events | Max pending light publications | Max worldgen mailbox jobs | Max light mailbox statuses |
|---:|---:|---:|---:|---:|---:|---:|
| `10` | `1574` | `1597` | `1382` | `9` | `1` | `11` |
| `15` | `3173` | `3222` | `2863` | `9` | `1` | `11` |

Interpretation:

- Desktop full-view readiness still scales like the current publication valve:
  `529 / 27.55s` and `1089 / 57.19s` are both about `19` target chunks/sec.
- The client update queue did not back up in either run (`max depth 0`), and
  apply/dirty/client-apply totals were small relative to the paced frame
  window. The first visible wall is server-side readiness/publication, not
  client receive/apply.
- Frame pacing remained clean at 60 Hz in both runs. This is desktop evidence
  only; it does not prove a larger publish drain is safe on Quest.
- The pending-publication max stays at about one `128`-target feature job, which
  matches the current job serialization model. Candidate A still needs both an
  elapsed-budget publication drain and bounded job-admission overlap.

### 2026-07-05 - Publication Cost Instrumentation Smoke

Commit: captured before commit, then landed as `2ff3273e`.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Purpose: land no-behavior-change instrumentation before Candidate A. New fields
flow from `ChunkSchedulerPublicationDiagnostics` through runner/runtime
diagnostics into native startup-streaming JSON, frame-budget probe JSON, and
Quest `MCLONE_ANDROID_XR_PERF_QUEUE_MAX` / worst-frame upload logs.

Short JSON smoke:

```sh
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --startup-streaming-perf --render-distance 5 --startup-streaming-frames 120 \
  --target-hz 60 --debug-passive-showcase false \
  > /tmp/mclone-startup-instrument-smoke.json
```

Selected parsed fields from `/tmp/mclone-startup-instrument-smoke.json`:

- `startup_playable_ms=1122.983`
- `total_scheduler_feature_chunks_published=101`
- `total_scheduler_light_statuses_published=120`
- `total_scheduler_snapshot_ready_events=120`
- `max_scheduler_pending_worldgen_publication_chunks=124`
- `max_scheduler_pending_light_publications=8`
- `max_scheduler_worldgen_mailbox_pending_jobs=1`
- `max_scheduler_light_mailbox_pending_statuses=9`
- `queue_samples` length `3` for `120` frames (`0`, `60`, final)
- final sample: `target_ready_chunks=50 / 169`,
  `pending_publications=86`,
  `scheduler_pending_worldgen_publication_chunks=83`,
  `scheduler_pending_light_publications=4`,
  `scheduler_worldgen_mailbox_pending_jobs=0`,
  `scheduler_light_mailbox_pending_statuses=9`,
  `server_update_queue_depth=0`

Interpretation: the instrumentation is reporting the expected shape without
changing budgets: even the short RD5 smoke makes the publication backlog visible
while the client update queue stays drained. Full RD10/RD15 runs are still
required before making policy decisions.

Validation:

- `cargo check -p mclone-native-client -p mclone-android-xr-client` passed
  (existing Android XR unused-code warnings for
  `ANDROID_REMOTE_ADDR_NONE_SENTINEL` /
  `normalize_android_legacy_remote_addr`).
- `cargo test -p mclone-server chunk_scheduler_poll_slices_completed_publication`
  passed, including assertions for the new publication counters.
- `cargo test -p mclone-server --lib` compiled and ran, but one unrelated oracle
  test failed:
  `generated_origin_chunk_block_light_strict_matches_scheduler_light_oracle_fixture`
  (`expected 0x00, got 0x01`). Do not treat that as evidence about this
  diagnostic slice; it should be resolved with the concurrent worldgen/light
  workstream.

### 2026-07-04 - Publication Valve Attribution And Publish-Budget Prototype

Commit reported by native benchmark JSON: `4a3604cf`.

`git_dirty=true`; `debug_assertions=false`. Control runs carried only
uncommitted tactical/topic doc edits (no code changes). The two prototype runs
additionally carried a deliberate 2-line diff raising
`DEFAULT_COMPLETED_CHUNK_PUBLISH_BUDGET` and
`DEFAULT_COMPLETED_LIGHT_PUBLISH_BUDGET` from `1` to `32` in
`native/crates/mclone-server/src/scheduler.rs`; that diff was reverted after
measurement and is not landed.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Purpose: falsification pass for the tactical `142` bottleneck model. Question:
is desktop full-view streaming capped by the scheduler publication valve
(`1` feature chunk + `1` light status published per gameplay tick, with the
next feature job gated on full publication of the previous one)?

Startup-streaming RD10 (`6000` frames, `60 Hz`, workers `1`, seed `12345`):

| Config | Playable | Full-view ready | Render quiescent | Frame avg/p95/p99/max | Over budget |
|---|---:|---:|---:|---|---:|
| cadence `20/20/60`, publish `1` (control) | `1,078ms` | `27.54s` | `33.06s` | `5.41 / 7.14 / 7.70 / 10.83ms` | `0 / 6000` |
| cadence `60/20/60`, publish `1` | `1,498ms` | `33.78s` | `39.49s` | `5.39 / 7.25 / 7.95 / 10.88ms` | `0 / 6000` |
| cadence `20/20/60`, publish `32` (prototype) | `300ms` | `9.23s` | `21.04s` | `5.45 / 7.08 / 7.68 / 11.36ms` | `0 / 6000` |

Loading-settle isolation (`5,10,15`, cadence `20/20/60`):

| Config | RD5 runtime | RD10 runtime | RD15 runtime | RD5 mesh | RD10 mesh | RD15 mesh |
|---|---:|---:|---:|---:|---:|---:|
| workers `1`, publish `1` (control) | `12.42s` (`13.6/s`) | `33.38s` (`15.9/s`) | `64.83s` (`16.8/s`) | `0.92s` (`2093 sec/s`) | `7.94s` (`889 sec/s`) | `33.07s` (`465 sec/s`) |
| workers `1`, publish `32` | `3.87s` (`43.7/s`) | `11.12s` (`47.6/s`) | `22.03s` (`49.4/s`) | `0.91s` (`2134 sec/s`) | `7.77s` (`908 sec/s`) | `32.39s` (`475 sec/s`) |
| workers `2`, publish `32` | `3.97s` (`42.6/s`) | `11.32s` (`46.7/s`) | `22.19s` (`49.1/s`) | `0.95s` (`2044 sec/s`) | `7.96s` (`886 sec/s`) | `33.05s` (`465 sec/s`) |

Raw local artifacts: `/tmp/exp1-settle-control-cadence20.json`,
`/tmp/exp2-streaming-rd10-cadence20.json`,
`/tmp/exp2-streaming-rd10-cadence60.json`,
`/tmp/exp3-streaming-rd10-publish32.json`,
`/tmp/exp3-settle-publish32-workers1.json`,
`/tmp/exp4-settle-publish32-workers2.json`.

Interpretation:

- The publication valve is real and is the dominant desktop throughput limit.
  Raising the two publish budgets `1` → `32` (no other change) gave `3.0x`
  full-view-ready and `3.6x` playable entry at RD10, and `2.9x`-`3.2x` runtime
  settle at RD5-RD15, with mesh settle and desktop frame pacing unchanged
  (zero over-budget frames in both streaming runs).
- The valve is driven by the **gameplay tick**, not the host tick:
  `ChunkScheduler::tick_report_with_record_builders(...)` calls `poll()` once
  per gameplay tick. Raising only the host rate (`60/20/60`) left the cap in
  place and measured `~23%` worse, so the `140` cadence-sweep lever is retired.
- Compile workers `2` left the bulk mesh drain flat at every distance; the
  isolation mesh drain is admission/prepare-bound, not compile-bound. The
  superlinear per-section degradation (`2093` → `465` sec/s from RD5 to RD15)
  persists across all configs.
- After the prototype, the next limits are visible: server side settles near
  `50` chunks/sec (feature-job serialization bubble + single worldgen thread),
  and the streaming render quiescent tail (`9.2s` → `21.0s`) matches the
  one-chunk-per-frame mesh admission ceiling at 60 Hz.
- This is desktop evidence only. It shows that scheduler publication throttles
  client-visible readiness; it does not prove that publication is an important
  Quest bottleneck. The Quest question is whether opening this valve remains
  buffered by the client/render budgets or appears as server runner spikes,
  update queue age, client accept/store cost, dirty/prepare/upload tails, or
  dropped-frame deltas.
- Probe bug: `--loading-settle-perf` with `--simulation-cadence 60/20/60`
  false-idles in under a second (`target_ready_chunks=0`) because
  `runner_idle(...)` can be observed before the first interest command
  produces pending work. Harden before reusing that lane for cadence work.
- Follow-up implementation direction lives in tactical `142`: elapsed-budget
  publication drain serviced from the gameplay tick behind a throughput
  profile, job-admission overlap with a bounded pending-publication backlog, and
  first-class per-stage cost attribution before mesh admission work.

### 2026-07-04 - Flat Android RD5 Startup Timing Check

Flat Android validation code committed as `daef4977`.

The Android log timings below were captured from the worktree that became
`daef4977`; unrelated worldgen/texture-lab files were dirty and are not part of
this record.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64.

Scenario: seed `12345`, local integrated mode, render distance `5`, render
compile workers `1`, day time `6000`, freeze time enabled, lighting enabled,
section occlusion enabled, vanilla color profile. The flat Android validators
stage `reference/minecraft-1.17.1/extracted.zip` into internal app storage and
wait for the `Mclone Android rendered local integrated frame` marker.

Commands:

```bash
node ./scripts/run-native-bash.mjs ./android/validate-avd.sh \
  --avd jstorrent-tablet \
  --skip-build \
  --screenshot /tmp/mclone-android-avd-flat-internal-assets.png \
  --log /tmp/mclone-android-avd-flat-internal-assets-logcat.txt \
  --smoke-seconds 15

pnpm native:android:quest-flat -- \
  --skip-build \
  --serial 2G0YC1ZF93041Z \
  --screenshot /tmp/mclone-quest-flat.png \
  --log /tmp/mclone-quest-flat-logcat.txt
```

Raw local artifacts:

- `/tmp/mclone-android-avd-flat-internal-assets-logcat.txt`
- `/tmp/mclone-android-avd-flat-internal-assets.png`
- `/tmp/mclone-quest-flat-logcat.txt`
- `/tmp/mclone-quest-flat.png`

Comparable RD5 timing:

| Lane | Device/backend | Target chunks | Runtime ready | Render/upload ready | First rendered frame |
|---|---|---:|---:|---:|---:|
| Desktop loading-settle | Apple M4 Pro, synthetic full-drain | `169` | `12.473s` | `13.417s` full settle | N/A |
| Android AVD flat | `jstorrent-tablet`, arm64 AVD, SwiftShader Vulkan | `169` | `13.178s` | `14.518s` | `14.587s` |
| Quest 3 flat panel | Quest 3 `2G0YC1ZF93041Z`, Adreno 740 Vulkan | `169` | `18.074s` | `21.825s` | `21.973s` |

Android phase details, measured from `Mclone Android starting`:

| Lane | Asset pack loaded | Runtime idle | Player pose synced | Render sections uploaded | Adapter selected | First rendered frame |
|---|---:|---:|---:|---:|---:|---:|
| Android AVD flat | `0.325s` | `13.178s` | `13.228s` | `14.518s` | `14.583s` | `14.587s` |
| Quest 3 flat panel | `0.193s` | `18.074s` | `18.113s` | `21.825s` | `21.964s` | `21.973s` |

Interpretation:

- Flat Android now has a real validation lane on both AVD and Quest. The Quest
  screenshot shows the flat NativeActivity as a panel in the headset compositor
  with rendered gameplay and touch UI.
- Quest flat startup is slower than the AVD for the same RD5 target: roughly
  `22.0s` to first rendered frame versus `14.6s`.
- The flat Android path currently appears to render after the full RD5 target
  is ready and uploaded. That is good for first-frame validation, but it is not
  the same local-play shape as desktop startup streaming, where the app can
  enter at the under-foot `3x3` playable gate and stream the remaining view
  afterward.
- This is not Quest frame-pacing evidence. It proves first rendered frame and
  panel presentation, not dropped-frame/headroom behavior. Use the OpenXR
  guardrail lanes for VR frame pacing.

Follow-up gaps:

- Add flat Android startup markers equivalent to desktop
  playable/full-view-ready/render-quiescent if we want this lane to become a
  true Android startup-streaming benchmark.
- Run Quest/OpenXR RD5 settled-orbit guardrails after any throughput policy
  changes, plus RD7 for promising or shared-policy candidates. RD5 should remain
  clean; RD7 should not materially regress.

### 2026-07-04 - Desktop Startup-Streaming RD20 Long-Run Baseline

Commit reported by native benchmark JSON: `3adafc1e`.

`git_dirty=false`; `debug_assertions=false`.

Host: Apple M4 Pro Mac, Darwin `25.5.0` arm64, macOS `26.5.1`.

Release startup-streaming command:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --startup-streaming-perf \
  --render-distance 20 \
  --startup-streaming-frames 30000 \
  --target-hz 120 \
  --debug-passive-showcase false \
  --render-compile-workers 1 \
  --simulation-cadence 20/20/60
```

Raw output for this local run:
`/tmp/mclone-startup-streaming-rd20-workers1-cadence20.json`.

Benchmark options: seed `12345`, transient local integrated world, render
distance `20`, render compile workers `1`, simulation cadence `20/20/60`,
lighting enabled, debug passive showcase disabled, `30,000` paced frames at
`120 Hz`. This is retained as an explicit long-run checkpoint. The current
throughput loop should use desktop RD10/RD15 with Quest RD5 as the safety
guardrail and Quest RD7 as the pressure check.

Summary:

| Metric | Value |
|---|---:|
| Playable entry | `1,746.504 ms` |
| Playable entry frame | `140` |
| Cached sections at playable entry | `144` |
| Startup gate readiness | `9 / 9` chunks, `100%` |
| First full-view ready | frame `9,308`, `97,008.104 ms` after streaming start |
| First target render quiescent | frame `9,943`, `106,032.752 ms` after streaming start |
| Total streaming wall time | `334,273.465 ms` |
| Final target readiness | `1,849 / 1,849` chunks, `100%` |
| Final loaded chunks | `1,849` |
| Final cached sections | `8,320` |
| Final pending jobs/publications/compile/inflight | `0 / 0 / 0 / 0` |
| Final pending render chunks | `168` |

Frame-loop timing:

| Metric | Value |
|---|---:|
| Average measured frame work | `10.318 ms` |
| p95 measured frame work | `13.699 ms` |
| p99 measured frame work | `16.075 ms` |
| Max measured frame work | `18.971 ms` |
| Frames over `8.333 ms` target | `22,232 / 30,000` |
| Frames over `16.667 ms` | `224 / 30,000` |
| Frames over `33.333 ms` | `0 / 30,000` |

Streaming work totals:

| Bucket | Total |
|---|---:|
| Runtime poll | `262.906 ms` |
| Remesh/sync | `4,041.874 ms` |
| Upload | `697.657 ms` |
| Render callback | `52,645.397 ms` |
| Submitted compile sections | `28,995` |
| Completed compile sections | `29,004` |
| Uploaded sections | `10,377` |
| Deadline-skipped compile requests | `0` |
| Update-pump stalled frames | `0` |

Interpretation:

- The startup gate is fixed for this lane: RD20 enters playable in `1.747s`
  with only the under-foot `3x3` gate required, then continues streaming.
- Full target chunk readiness under the desktop-shaped pump arrives at
  `97.008s`, and the target render stream becomes quiescent at `106.033s`.
  That is the better local-play answer than the synthetic loading-settle
  full-drain number because it uses the same startup pump, runtime polling,
  render-work admission, and frame-deadline sync path as the offscreen client.
- This does not mean every possible edge render chunk is gone. The final
  `pending_render_chunks=168` matches the known edge-neighbor behavior where
  chunks outside the requested target square can still block edge render
  chunks. Target readiness, jobs, publications, compile queue, and inflight
  work are all drained.
- The frame-time numbers are conservative offscreen timing, not final native
  swapchain pacing. The headless loop waits for `wgpu::PollType::Wait` each
  frame, so use the readiness/quiescence timings as the primary streaming
  throughput signal and use native window/Quest runs for final frame pacing.
- Even with that caveat, the measured offscreen frame work is over a `120 Hz`
  budget for most frames. Before changing throughput policy, add a frame-loop
  timing breakdown or true native-window probe so we can separate runtime work,
  command encoding, GPU wait, and present/swapchain behavior.

### 2026-07-04 - Desktop Loading-Settle Synthetic Isolation Baseline

Commit reported by native benchmark JSON: `482f0d51`.

`git_dirty=false`; `debug_assertions=false`.

Host: Apple M4 Pro Mac, 48 GiB RAM, Darwin `25.5.0` arm64.

Release loading-settle command:

```bash
cargo run --release --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --loading-settle-perf \
  --loading-settle-distances 5,10,15,20 \
  --debug-passive-showcase false \
  --render-compile-workers 1 \
  --simulation-cadence 20/20/60
```

Benchmark options: seed `12345`, transient worlds, render compile workers `1`,
simulation cadence `20/20/60`, lighting enabled, debug passive showcase
disabled, benchmark idle timeout `600s` per distance. This is a clean
synthetic full-drain isolation baseline after the high-render-distance startup
scheduling work in tactical `139`; the target chunk counts include the
Java-shaped `requested + 1` tracking halo. It does not model desktop-native
entry followed by progressive streaming; use the startup-streaming lane for
that policy decision.

Summary:

| Render distance | Tracking radius | Target chunks | Runtime settle | Mesh settle | Full settle | Runtime chunks/sec | Full chunks/sec | Sim time | Cached sections | Pending render chunks |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `5` | `6` | `169` | `12,473.178 ms` | `943.591 ms` | `13,416.769 ms` | `13.549` | `12.596` | `12.050 s` | `1,936` | `48` |
| `10` | `11` | `529` | `33,325.249 ms` | `7,981.598 ms` | `41,306.846 ms` | `15.874` | `12.807` | `33.050 s` | `7,056` | `88` |
| `15` | `16` | `1,089` | `64,884.972 ms` | `33,042.018 ms` | `97,926.990 ms` | `16.784` | `11.121` | `64.600 s` | `15,376` | `128` |
| `20` | `21` | `1,849` | `108,077.834 ms` | `94,653.061 ms` | `202,730.895 ms` | `17.108` | `9.120` | `106.500 s` | `26,896` | `168` |

Counters at completion: `target_ready_chunks == target_chunk_count`,
`loaded_chunks == target_chunk_count`, `pending_jobs=0`,
`pending_publications=0`, and `pending_render_compile_jobs=0` for every
distance. The remaining `pending_render_chunks` are edge render chunks waiting
on outside-neighbor readiness after the requested target chunk square has
settled.

Interpretation:

- RD20 remains a multi-minute full-view drain in this synthetic isolation lane.
  The clean RD20 split is `108.078s` runtime/server settle plus `94.653s`
  render mesh settle.
- Runtime settle is still the larger RD20 share (`53.3%`), and simulation time
  (`106.500s`) tracks runtime wall time closely. That makes host cadence,
  scheduler publication, and server/worldgen/light pacing first-class suspects;
  this is not only a render compile problem.
- Mesh settle becomes nearly half the RD20 total (`46.7%`). Single-worker mesh
  throughput falls with distance, from roughly `2052` cached sections/sec at
  RD5 to `284` cached sections/sec at RD20, so render compile/mesh throughput
  also needs its own sweep.
- The next comparison should keep the phases separate: run a cadence sweep to
  test runtime throttling, and a render-compile worker sweep to test mesh
  throughput, before changing defaults or Quest backpressure policies.

### 2026-07-04 - Desktop Loading-Settle Synthetic Throughput

Commit reported by native benchmark JSON: `e0106245`.

Note: `git_dirty=true` because this was captured while adding the
loading-settle benchmark lane and with unrelated tactical-doc edits in the
worktree. Treat this as the first durable desktop-native loading-settle
isolation baseline, not a clean historical state of `e0106245` and not a
desktop-shaped startup streaming result.

Host: Apple M4 Pro Mac, 48 GiB RAM, Darwin `25.5.0` arm64.

Release loading-settle command:

```bash
pnpm native:loading-settle:perf
```

Benchmark options: seed `12345`, transient worlds, render compile workers `1`,
simulation cadence `20/20/60`, lighting enabled, debug passive showcase
disabled, benchmark idle timeout `600s` per distance.

Summary:

| Render distance | Target chunks | Runtime settle | Mesh settle | Full settle | Runtime chunks/sec | Full chunks/sec | Sim time | Cached sections | Pending render chunks |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `5` | `121` | `10,249.026 ms` | `547.563 ms` | `10,796.589 ms` | `11.806` | `11.207` | `9.850 s` | `1,296` | `40` |
| `10` | `441` | `31,117.749 ms` | `6,331.645 ms` | `37,449.394 ms` | `14.172` | `11.776` | `28.850 s` | `5,776` | `80` |
| `15` | `961` | `68,410.366 ms` | `28,736.760 ms` | `97,147.127 ms` | `14.048` | `9.892` | `57.700 s` | `13,456` | `120` |
| `20` | `1,681` | `127,947.261 ms` | `82,297.339 ms` | `210,244.600 ms` | `13.138` | `7.995` | `97.100 s` | `24,336` | `160` |

Counters at completion: `target_ready_chunks == target_chunk_count`,
`loaded_chunks == target_chunk_count`, `pending_jobs=0`,
`pending_publications=0`, and `pending_render_compile_jobs=0` for every
distance. The remaining `pending_render_chunks` are edge render chunks waiting
on outside-neighbor readiness after the requested target chunk square has
settled.

Render distances `25` and `30` are intentionally omitted from the default
release lane for now because distance `20` already makes the lane multi-minute
on this desktop. Add them only to an explicit long-run command until startup
throughput improves.

### 2026-06-19 - Block Render Facts Parity

Commit reported by native benchmark JSON: `1cdc9ed`.

Note: `git_dirty=true` because this was captured while implementing tactical
`054` after `1cdc9ed`. Treat the result as the measured state for tactical
`054`, not as the clean historical state of `1cdc9ed`.

Radius-5 lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Summary:

| Metric | Value |
|---|---:|
| total elapsed | `1,947.392 ms` |
| light-status compute | `456.666 ms` |
| `LevelLightEngine.run_all_updates` | `403.441 ms` |
| block graph drain | `30.506 ms` |
| sky graph drain | `372.905 ms` |
| block processed nodes | `52,348` |
| sky processed nodes | `794,963` |
| run-update iterations | `52` |

Release movement-frame command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4
```

Movement-frame probe, mode `movement_walk`, target `120 Hz`, `240` frames,
speed `32 blocks/sec`:

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 240` |
| p95 frame | `4.128 ms` |
| p99 frame | `5.889 ms` |
| max frame | `8.250 ms` |
| initial face count | `121,222` |
| initial index count | `727,332` |
| average headless frame | `2.887 ms` |

Release timedemo command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --timedemo --timedemo-frames 240
```

Timedemo, seed `12345`, render distance `2`, loaded render distance `4`,
`240` frames:

| Metric | Value |
|---|---:|
| scene build | `1,491.873 ms` |
| section count | `1,296` |
| face count | `405,407` |
| index count | `2,432,442` |
| visibility graph total | `19.404 ms` |
| render setup | `34.773 ms` |
| average frame | `2.739 ms` |
| max frame | `13.450 ms` |
| average drawn sections | `90.167` |
| max drawn sections | `125` |
| average drawn indices | `720,072.250` |
| max drawn indices | `986,070` |

Observation: moving AO/culling facts to Java `BlockStateBase.Cache` semantics
does not affect the server light graph counters. It does intentionally raise
render face pressure because full-cube leaves no longer act like opaque face
cullers, but the release movement-frame probe remains within the 120 Hz budget
and timedemo remains in the existing renderer envelope. This is the new render
baseline for Java-style leaf non-occlusion.

### 2026-06-19 - Leaf Sky Render Parity

Commit reported by native benchmark JSON: `451dedd`.

Note: `git_dirty=true` because this was captured while implementing tactical
`050` after `451dedd`. Treat the result as the measured state for tactical
`050`, not as the clean historical state of `451dedd`.

Radius-5 lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Summary:

| Lane | Total elapsed | Light compute | `run_updates` | Block graph | Sky graph |
|---|---:|---:|---:|---:|---:|
| P6.10 source storage | `1,927.655 ms` | `455.894 ms` | `402.035 ms` | `30.788 ms` | `371.212 ms` |
| P6.11 leaf/sky render parity | `1,945.648 ms` | `460.263 ms` | `405.848 ms` | `30.681 ms` | `375.135 ms` |

Graph counters:

| Metric | P6.10 source storage | P6.11 leaf/sky render parity |
|---|---:|---:|
| run-update iterations | `52` | `52` |
| block processed nodes | `52,348` | `52,348` |
| sky processed nodes | `794,466` | `794,963` |
| max block queue before | `27,663` | `27,663` |
| max sky queue before | `60,450` | `60,450` |

Interpretation: matching Java leaf opacity and Java sky-storage reads across
omitted all-air sky sections fixes the dark canopy-top render bug without
materially changing startup performance. The small sky-node increase is
expected because leaves now attenuate sky by one level instead of blocking it
completely. The next visible parity bottleneck is render-side Java lightmap/AO
rather than more cold-start graph tuning.

### 2026-06-19 - Sky Source-Section Ownership

Commit reported by native benchmark JSON: `e9cc842`.

Note: `git_dirty=true` because this was captured while implementing tactical
`049` after `e9cc842`. Treat the result as the measured state for tactical
`049`, not as the clean historical state of `e9cc842`.

Radius-5 lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Summary:

| Lane | Total elapsed | Light compute | `run_updates` | Block graph | Sky graph |
|---|---:|---:|---:|---:|---:|
| P6.9 empty-section setup | `1,931.110 ms` | `485.407 ms` | `415.744 ms` | `29.855 ms` | `385.856 ms` |
| P6.10 source storage | `1,927.655 ms` | `455.894 ms` | `402.035 ms` | `30.788 ms` | `371.212 ms` |

Graph counters:

| Metric | P6.9 empty-section setup | P6.10 source storage |
|---|---:|---:|
| run-update iterations | `52` | `52` |
| block processed nodes | `52,348` | `52,348` |
| sky processed nodes | `794,466` | `794,466` |
| max block queue before | `27,663` | `27,663` |
| max sky queue before | `60,461` | `60,450` |

Interpretation: moving source-section ownership into `SkyLightSectionStorage`
keeps the P6.9 graph-node reduction intact while removing retained-world manual
sky source scanning (`14.736 ms`) and enqueue timing (`3.872 ms`). This is
primarily a parity/module-boundary improvement; startup presentation is now the
larger desktop-feel issue than light graph drain cost.

### 2026-06-19 - Sky Empty-Section Light Setup

Commit reported by native benchmark JSON: `81b50f1`.

Note: `git_dirty=true` because this was captured while implementing tactical
`048` after `81b50f1`. Treat the result as the measured state for tactical
`048`, not as the clean historical state of `81b50f1`.

Radius-5 lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Summary:

| Lane | Total elapsed | Light compute | `run_updates` | Block graph | Sky graph |
|---|---:|---:|---:|---:|---:|
| P6.8 mixed hash baseline | `6,733.051 ms` | `5,236.809 ms` | `5,185.626 ms` | `32.612 ms` | `5,152.656 ms` |
| P6.9 empty-section setup | `1,931.110 ms` | `485.407 ms` | `415.744 ms` | `29.855 ms` | `385.856 ms` |

Graph counters:

| Metric | P6.8 baseline | P6.9 empty-section setup |
|---|---:|---:|
| run-update iterations | `614` | `52` |
| block run-update calls | `614` | `52` |
| sky run-update calls | `614` | `52` |
| block processed nodes | `52,348` | `52,348` |
| sky processed nodes | `10,002,274` | `794,466` |
| max block queue before | `27,663` | `27,663` |
| max sky queue before | `57,600` | `60,461` |
| final block queue after | `0` | `0` |
| final sky queue after | `0` | `0` |

Interpretation: the previous native setup marked every vertical section as
non-empty light storage, while Java `lightChunk(...)` only activates non-empty
sections. Passing real section-empty flags and seeding sky from the highest
non-empty section cuts repeated sky graph work by about `92%` on this lane.
The remaining performance issue is no longer the graph drain itself; the next
performance-facing desktop issue is startup presentation while the first scene
warms.

### 2026-06-19 - Light Graph Drain Instrumentation And Mixed Hash Map

Commit reported by native benchmark JSON: `4f7bc5e`.

Note: `git_dirty=true` because this was captured while implementing tactical
`047` after `4f7bc5e`. Treat the result as the measured state for tactical
`047`, not as the clean historical state of `4f7bc5e`.

Radius-5 lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Summary:

| Lane | Total elapsed | Light compute | `run_updates` | Block graph | Sky graph |
|---|---:|---:|---:|---:|---:|
| Instrumented baseline | `10,614.161 ms` | `9,173.467 ms` | `9,115.460 ms` | `48.058 ms` | `9,067.052 ms` |
| Raw identity hash rejected | `35,665.194 ms` | `34,206.109 ms` | `33,919.280 ms` | `189.468 ms` | `33,729.449 ms` |
| Mixed hash kept | `6,733.051 ms` | `5,236.809 ms` | `5,185.626 ms` | `32.612 ms` | `5,152.656 ms` |

Mixed-hash graph counters:

| Metric | Value |
|---|---:|
| run-update iterations | `614` |
| block run-update calls | `614` |
| sky run-update calls | `614` |
| block processed nodes | `52,348` |
| sky processed nodes | `10,002,274` |
| max block queue before | `27,663` |
| max sky queue before | `57,600` |
| final block queue after | `0` |
| final sky queue after | `0` |

Interpretation: the first graph-drain instrumentation split proves sky light is
the remaining cold-start blocker. Switching the graph's pending maps and queue
member sets from tree collections to mixed integer-key hash collections cuts
the radius-5 `run_updates` drain from `9.12s` to `5.09s` without changing graph
node counts. A raw identity hash was rejected because packed block-position
keys clustered and regressed the same workload badly. The next optimization
target is reducing redundant sky graph work, not block light or scheduler
publication.

### 2026-06-19 - Retained Initial Light World First Pass

Commit reported by native benchmark JSON: `fce2425`.

Note: `git_dirty=true` because this was captured while implementing the retained
initial light world after `fce2425`. Treat the result as the measured state for
tactical `046`, not as the clean historical state of `fce2425`.

Radius-5 lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Radius-5 summary:

| Metric | Value |
|---|---:|
| total elapsed | `11,292.102 ms` |
| feature batch timing | `1,148.652 ms` |
| completed light statuses | `169` |
| completed light batches | `1` |
| total light-status compute | `9,341.649 ms` |
| light `run_updates` time | `9,301.529 ms` |
| retained world upsert | `0.622 ms` |
| block source scan | `19.330 ms` |

Interpretation: the retained owner does not materially improve the cold
radius-5 startup case because that path was already one batch and remains one
large graph drain. This is expected; the purpose of this slice is to preserve
world light state across batches and make the worker shape closer to Java
`ThreadedLevelLightEngine`.

Radius-3 two-step lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 3 --steps 2 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Radius-3 two-step summary:

| Step | Total step time | Loaded snapshots | Light batches | Cumulative light compute | Incremental light compute |
|---|---:|---:|---:|---:|---:|
| `0` | `5,513.912 ms` | `81` | `1` | `4,652.194 ms` | `4,652.194 ms` |
| `1` | `510.729 ms` | `90` | `2` | `5,076.063 ms` | `423.869 ms` |

Interpretation: retained state helps subsequent movement batches: the second
step lights only newly retained work instead of rebuilding the whole initial
view's raw light world. The remaining high-priority lighting performance work
is inside `LevelLightEngine.run_all_updates` itself and its graph queue shape.

### 2026-06-18 - Shared Initial Light Batch First Pass

Commit reported by native benchmark JSON: `2207e64`.

Note: `git_dirty=true` because this was captured while implementing the shared
initial light batch slice after `2207e64`. Treat the result as the measured
state for tactical `045`, not as the clean historical state of `2207e64`.

Native `LIGHT`/lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Native `FEATURES`/lighting-disabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --disable-lighting
```

Summary:

| Lane | Target chunks | Total |
|---|---:|---:|
| Native scheduler, lighting disabled | `121` visible / `169` feature snapshots | `1,107.779 ms` |
| Native scheduler, lighting enabled before batch | `121` visible / `169` light snapshots | `47,782.677 ms` |
| Native scheduler, lighting enabled after batch | `121` visible / `169` light snapshots | `10,745.617 ms` |

Native lighting-enabled breakdown after batch:

| Metric | Value |
|---|---:|
| feature batch timing | `790.832 ms` |
| completed light statuses | `169` |
| completed light batches | `1` |
| total light-status batch compute | `9,323.850 ms` |
| max light-status batch compute | `9,323.850 ms` |
| light `run_updates` time | `9,281.490 ms` |
| block source scan | `21.637 ms` |
| sky source enqueue | `9.682 ms` |
| section setup | `4.498 ms` |
| collect sections | `0.439 ms` |

Interpretation: batching the initial light work at the completed feature-job
boundary removed most duplicate propagation work, dropping the radius-5
lighting-enabled scheduler run from `47.8s` to `10.7s`. Native worldgen remains
around `0.8s`; the remaining light bottleneck is the single large
`LevelLightEngine.run_all_updates` drain over the shared raw light world. The
next lighting throughput target should be the Java-shaped long-lived world light
state/task queue and the graph drain itself, not feature generation or renderer
frame pacing.

### 2026-06-18 - Radius-5 Scheduler/Oracle Lighting Investigation

Commit reported by native benchmark JSON: `48fdd70`.

Note: `git_dirty=true` because this was captured while adding the comparison
and timing instrumentation. Treat the result as the measured state after
`48fdd70` plus the local perf-instrumentation changes.

Native `FEATURES`/lighting-disabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --disable-lighting
```

Native `LIGHT`/lighting-enabled command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting
```

Java oracle command:

```bash
pnpm --silent oracle:gen scheduler-trace --seed 12345 --chunk-x 0 --chunk-z 0 --target-radius 5 --record-radius 5 --stop-status features --view-distance 5 --generate-structures false --timeout-seconds 240
```

Summary:

| Lane | Target chunks | Total |
|---|---:|---:|
| Java 1.17.1 oracle to `FEATURES` | `121` | `4,718.254 ms` |
| Native scheduler, lighting disabled | `121` visible / `169` feature snapshots | `1,106.492 ms` |
| Native scheduler, lighting enabled | `121` visible / `169` light snapshots | `47,782.677 ms` |

Native lighting-enabled breakdown:

| Metric | Value |
|---|---:|
| feature batch timing | `783.943 ms` |
| completed light statuses | `169` |
| total light-status compute | `46,983.813 ms` |
| max single light-status compute | `315.310 ms` |
| light `run_updates` time | `46,753.960 ms` |
| block source scan | `120.966 ms` |
| sky source enqueue | `49.613 ms` |
| section setup | `24.094 ms` |
| collect sections | `0.852 ms` |

Interpretation: native worldgen/features are not the desktop startup
regression. The regression is initial light propagation. The current native
path computes each `ChunkStatus::Light` payload independently over a temporary
3x3 raw-chunk light world, so radius 5 performs `169` separate graph drains.
The Java reference shape is a threaded, long-lived `LevelLightEngine` wrapper
that batches light tasks against shared world light state. The next optimization
target should be replacing per-chunk isolated initial light recomputation, not
renderer frame pacing or feature generation.

The Java oracle `LIGHT` and `FULL` stop-status runs timed out under the current
spawn-bootstrap trace scenario; `FEATURES` is the reliable oracle comparison
from this pass.

### 2026-06-16 - Render Compile Revisions And Priority Release Baseline

Commit reported by benchmark JSON: `3bc5e7d`.

Note: `git_dirty=true` because this was captured while implementing the render
compile revision/priority slice, before committing the slice. Treat it as the
release probe for that working-tree implementation, not as a clean historical
commit record.

Movement-frame command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4
```

Movement-frame probe, mode `movement_walk`, target `120 Hz`, `240` frames,
speed `32 blocks/sec`:

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 240` |
| over 2x budget frames | `0 / 240` |
| over 4x budget frames | `0 / 240` |
| p95 frame | `3.010 ms` |
| p99 frame | `4.156 ms` |
| max frame | `7.234 ms` |
| max `poll_ms` | `1.453 ms` |
| max `remesh_ms` | `0.130 ms` |
| max `upload_ms` | `0.400 ms` |
| max pending compile jobs | `1` |
| max in-flight sections | `16` |
| submitted compile sections | `107` |
| completed compile sections | `107` |
| stale compile sections | `0` |
| total snapshot updates | `5` |
| total section block update batches | `102` |
| total fluid mutated blocks | `299` |

Stress-orbit command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --frame-budget-probe --frame-budget-frames 120 --target-hz 120
```

Stress-orbit frame-budget probe, target `120 Hz`, `120` frames:

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 120` |
| p95 frame | `3.724 ms` |
| p99 frame | `4.361 ms` |
| max frame | `7.112 ms` |
| max `poll_ms` | `1.571 ms` |
| max `remesh_ms` | `0.080 ms` |
| max `upload_ms` | `0.342 ms` |
| max pending compile jobs | `1` |
| max in-flight sections | `16` |
| submitted compile sections | `115` |
| completed compile sections | `98` |
| stale compile sections | `14` |
| total section block update batches | `27` |
| total fluid mutated blocks | `80` |

Observation: per-section compile revisions removed stale worker output from the
normal movement-frame lane (`30` stale sections before, `0` after). The stress
orbit still stales active work during fast streaming/unload churn, but stale
sections dropped from `33` to `14`. Frame timing remains below the 120 Hz
budget.

### 2026-06-16 - Movement Frame Probe Release Baseline

Commit reported by benchmark JSON: `f8855ce`.

Note: `git_dirty=true` because this was captured while adding the movement-frame
probe itself, before committing the slice. Treat it as the release probe for
that working-tree implementation, not as a clean historical commit record.

Command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-frame-probe --frame-budget-frames 240 --target-hz 120 --path-radius 4
```

Movement-frame probe, mode `movement_walk`, target `120 Hz`, `240` frames,
speed `32 blocks/sec`:

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 240` |
| over 2x budget frames | `0 / 240` |
| over 4x budget frames | `0 / 240` |
| p95 frame | `3.317 ms` |
| p99 frame | `4.198 ms` |
| max frame | `7.066 ms` |
| headless average frame | `2.054 ms` |
| max `poll_ms` | `1.295 ms` |
| max `remesh_ms` | `0.094 ms` |
| max `upload_ms` | `0.399 ms` |
| max pending render chunks | `9` |
| max pending compile jobs | `1` |
| max in-flight sections | `16` |
| submitted compile sections | `91` |
| completed compile sections | `61` |
| stale compile sections | `30` |
| total snapshot updates | `5` |
| total section block update batches | `102` |
| total fluid mutated blocks | `299` |

Worst frame:

| Field | Value |
|---|---:|
| frame index | `0` |
| frame time | `7.066 ms` |
| budget multiple | `0.848x` |
| `poll_ms` | `0.322 ms` |
| `remesh_ms` | `0.000 ms` |
| `upload_ms` | `0.000 ms` |
| `render_ms` | `0.393 ms` |
| `device_poll_ms` | `6.102 ms` |

Observation: this movement-shaped headless lane does not reproduce the visible
120 Hz walking hitch on this host. Engine-side runtime, render compile, and GPU
upload work stayed below budget. The remaining investigative gap is likely
desktop present/wait/input pacing or a workload not represented by the headless
probe.

### 2026-06-16 - Async Render Section Compile Queue Release Probe

Commit reported by benchmark JSON: `ba23d22`.

Note: `git_dirty=true` because this was captured while implementing the async
render-section compile queue, before committing the slice. Treat it as the
release probe for that working-tree implementation, not as a clean historical
commit record.

Command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --frame-budget-probe --frame-budget-frames 120 --target-hz 120
```

Frame budget probe, target `120 Hz`, `120` frames:

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 120` |
| over 2x budget frames | `0 / 120` |
| over 4x budget frames | `0 / 120` |
| p95 frame | `4.030 ms` |
| p99 frame | `4.260 ms` |
| max frame | `6.968 ms` |
| headless average frame | `2.360 ms` |
| max `poll_ms` | `1.611 ms` |
| max `remesh_ms` | `0.060 ms` |
| max `upload_ms` | `0.326 ms` |
| max pending render chunks | `9` |
| max pending compile jobs | `1` |
| max in-flight sections | `16` |
| submitted compile sections | `78` |
| completed compile sections | `42` |
| stale compile sections | `33` |
| total section block update batches | `27` |
| total fluid mutated blocks | `80` |
| total rebuilt sections | `42` |
| max rebuilt sections on a frame | `16` |
| total uploaded sections | `31` |

Observation: CPU render-section compilation is no longer a material frame-path
cost in this probe. `remesh_ms` now reflects submission/drain bookkeeping.
The global compile epoch correctly discards stale worker output, but it is
conservative: continuous movement/fluid updates caused `33` stale sections out
of `78` submitted.

### 2026-06-16 - Section-Precise Dirtying Release Probe

Commit reported by benchmark JSON: `9078e8b`.

Note: `git_dirty=true` because this was captured while implementing
section-precise render dirtying, before committing the slice. Treat it as the
release probe for that working-tree implementation, not as a clean historical
commit record.

Command:

```bash
cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --frame-budget-probe --frame-budget-frames 120 --target-hz 120
```

Frame budget probe, target `120 Hz`, `120` frames:

| Metric | Value |
|---|---:|
| over-budget frames | `0 / 120` |
| over 2x budget frames | `0 / 120` |
| over 4x budget frames | `0 / 120` |
| p95 frame | `4.093 ms` |
| p99 frame | `5.146 ms` |
| max frame | `5.430 ms` |
| headless average frame | `2.442 ms` |
| max `poll_ms` | `1.589 ms` |
| max `remesh_ms` | `1.629 ms` |
| max `upload_ms` | `0.346 ms` |
| max pending render chunks | `9` |
| total section block update batches | `27` |
| total fluid mutated blocks | `80` |
| total rebuilt sections | `79` |
| max rebuilt sections on a frame | `16` |
| total uploaded sections | `52` |

Observation: section-delta frames now rebuild the affected render sections
instead of broad chunk neighborhoods. The largest frame in this probe was the
startup/device-poll frame, not a live section-delta frame. Nonzero pending
render chunks include retained deferred sections that are not actionable until
neighbor/camera readiness changes.

### 2026-06-15 - Optimized-Dev Smoke Baseline

Commit reported by benchmark JSON: `509c0e3`.

Note: `git_dirty=true` because this baseline was captured while adding the benchmark harness and this records document. Treat it as the first working-tree smoke baseline for the new lanes, not as a clean release budget.

Commands:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-worldgen --bin worldgen_perf -- --radius 1 --iterations 1
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-perf
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --timedemo --timedemo-frames 60
```

Worldgen, seed `12345`, radius `1`, `9` target chunks:

| Phase | Elapsed ms | Throughput | Notes |
|---|---:|---:|---|
| surface | 7.438 | 1209.989 chunks/s | terrain + surface/bedrock only |
| features cold | 357.560 | 25.171 target chunks/s | 441 dependency chunks generated |
| features warm | 22.929 | 392.510 target chunks/s | 441 dependency cache hits |

Cold feature timing:

| Component | ms |
|---|---:|
| dependency generation | 332.522 |
| feature decoration | 19.519 |

Movement/loading, seed `12345`, chunk radius `1`, 12-step circular path:

| Metric | Value |
|---|---:|
| total elapsed | 1538.349 ms |
| first step elapsed | 803.399 ms |
| later step elapsed range | 42.709-73.257 ms |
| later poll range | 8.760-20.570 ms |
| later remesh range | 8.766-11.787 ms |
| rebuilt sections range | 53-70 |
| later removed sections range | 30-55 |
| visible faces range | 6,863-22,315 |

Timedemo, seed `12345`, chunk radius `1`, 60 frames:

| Metric | Value |
|---|---:|
| scene build | 485.343 ms |
| render setup | 24.719 ms |
| average frame | 1.720 ms |
| min frame | 1.141 ms |
| max frame | 9.586 ms |
| average drawn sections | 54.733 |
| max drawn sections | 58 |
| average drawn indices | 281,794 |

### 2026-06-15 - VisGraph Implementation Smoke

Commit reported by benchmark JSON: `1edc3c9`.

Note: `git_dirty=true` because this was captured while implementing native `VisGraph` / `VisibilitySet` and render traversal. Treat it as an implementation-check record, not a clean budget.

Commands:

```bash
pnpm --silent native:movement:smoke
pnpm --silent native:timedemo:smoke
```

Movement/loading, seed `12345`, chunk radius `1`, 12-step circular path:

| Metric | Value |
|---|---:|
| total elapsed | 1565.930 ms |
| graph-cull-enabled steps | 12 / 12 |
| frustum sections range | 13-23 |
| graph-drawn sections range | 3-13 |
| graph-culled sections range | 3-18 |
| frustum faces range | 6,863-22,315 |
| graph-drawn faces range | 783-13,114 |
| graph-culled indices range | 9,432-77,820 |

Timedemo, seed `12345`, chunk radius `1`, 60 frames:

| Metric | Value |
|---|---:|
| loaded chunk radius | 4 |
| scene build | 1024.147 ms |
| render setup | 74.344 ms |
| average frame | 1.979 ms |
| min frame | 1.610 ms |
| max frame | 13.359 ms |
| graph-cull-enabled frames | 60 / 60 |
| average frustum sections | 393.717 |
| average drawn sections | 89.783 |
| average graph-culled sections | 303.933 |
| average frustum indices | 1,474,974.9 |
| average drawn indices | 440,756.4 |
| average graph-culled indices | 1,034,218.5 |

Observation: movement cameras exercise the graph on every step after retaining empty-section visibility records for traversal. Timedemo now loads an effective static scene radius of `max(chunk_radius, path_radius_chunks)`, so the default radius-4 orbit stays inside retained sections and exercises graph culling on every frame.

### 2026-06-15 - VisGraph Timing And Toggle Smoke

Commit reported by benchmark JSON: `07c0852`.

Note: `git_dirty=true` because this was captured while adding visibility graph build timing and the section-occlusion client toggle. Treat it as a paired implementation smoke, not a clean budget.

Commands:

```bash
pnpm --silent native:movement:smoke
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-perf --disable-section-occlusion
pnpm --silent native:timedemo:smoke
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --timedemo --timedemo-frames 60 --disable-section-occlusion
```

Movement/loading, seed `12345`, chunk radius `1`, 12-step circular path:

| Metric | Occlusion On | Occlusion Off |
|---|---:|---:|
| total elapsed | 1623.555 ms | 1582.340 ms |
| graph-cull-enabled steps | 12 / 12 | 0 / 12 |
| visibility graph builds per step | 144 | 144 |
| visibility graph total build range | 2.342-2.837 ms | 2.221-2.552 ms |
| visibility graph avg/section range | 0.016265-0.019703 ms | 0.015425-0.017726 ms |
| visibility graph worst section range | 0.051292-0.075875 ms | 0.049042-0.060667 ms |
| frustum sections range | 13-23 | 13-23 |
| drawn sections range | 3-13 | 13-23 |
| graph-culled sections range | 3-18 | 0 |
| frustum faces range | 6,863-22,315 | 6,863-22,315 |
| drawn faces range | 783-13,114 | 6,863-22,315 |
| graph-culled indices range | 9,432-77,820 | 0 |

Timedemo, seed `12345`, chunk radius `1`, loaded chunk radius `4`, 60 frames:

| Metric | Occlusion On | Occlusion Off |
|---|---:|---:|
| scene build | 967.136 ms | 1033.014 ms |
| visibility graph builds | 1,296 | 1,296 |
| visibility graph total build | 21.014 ms | 21.413 ms |
| visibility graph avg/section | 0.016214 ms | 0.016522 ms |
| visibility graph worst section | 0.055500 ms | 0.057833 ms |
| render setup | 69.390 ms | 67.782 ms |
| average frame | 2.056 ms | 3.575 ms |
| min frame | 1.647 ms | 3.213 ms |
| max frame | 13.258 ms | 16.071 ms |
| graph-cull-enabled frames | 60 / 60 | 0 / 60 |
| average frustum sections | 393.717 | 393.717 |
| average drawn sections | 89.783 | 393.717 |
| average graph-culled sections | 303.933 | 0 |
| average frustum indices | 1,474,974.9 | 1,474,974.9 |
| average drawn indices | 440,756.4 | 1,474,974.9 |
| average graph-culled indices | 1,034,218.5 | 0 |

Observation: the graph build itself is currently about `0.016 ms` per render section in optimized dev. The timedemo toggle control confirms section occlusion is reducing submitted draw pressure by roughly `3.35x` on this camera path, while leaving frustum pressure unchanged.

### 2026-06-15 - Provisional Sky Propagation Smoke

Commit reported by benchmark JSON: `2e97fc1`.

Note: `git_dirty=true` because this was captured while replacing direct-only provisional sky light with chunk-local propagated sky light. Treat it as an implementation-check record, not a clean budget.

Commands:

```bash
pnpm --silent native:movement:smoke
pnpm --silent native:timedemo:smoke
```

Movement/loading, seed `12345`, chunk radius `1`, 12-step circular path:

| Metric | Value |
|---|---:|
| total elapsed | 1804.757 ms |
| first step elapsed | 897.615 ms |
| later step elapsed range | 52.123-94.525 ms |
| later poll range | 19.362-44.411 ms |
| later remesh range | 12.420-16.336 ms |
| visibility graph total build range | 2.315-2.816 ms |
| graph-cull-enabled steps | 12 / 12 |
| graph-drawn sections range | 3-13 |
| graph-culled sections range | 3-18 |
| graph-drawn faces range | 783-13,114 |

Timedemo, seed `12345`, chunk radius `1`, loaded chunk radius `4`, 60 frames:

| Metric | Value |
|---|---:|
| scene build | 1172.352 ms |
| render setup | 83.020 ms |
| average frame | 2.097 ms |
| min frame | 1.619 ms |
| max frame | 15.524 ms |
| section count | 1,296 |
| face count | 328,036 |
| average drawn sections | 89.783 |
| average drawn indices | 440,756.4 |

Observation: draw pressure is effectively unchanged from the VisGraph lanes, but scene build/loading work is higher because the server now computes propagated sky `DataLayer`s instead of direct-only columns.

## Near-Term Perf Questions

- Keep tracking whether graph culling is enabled for each camera lane. Movement and timedemo now both exercise the graph; outside-retained-section traversal seeding is covered by render tests and headless overview captures.
- Add release records before enforcing budgets.
- Add larger radius/view-distance variants once radius `1` is stable enough to avoid hiding regressions in bootstrap noise.
