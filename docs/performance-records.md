# Performance Records

This file records native performance baselines by date and commit so renderer, worldgen, scheduler, and culling changes can be compared against a durable trend instead of one-off terminal output.

Use release builds for final budgets. Optimized-dev smokes are still useful for day-to-day trend checks because `native/Cargo.toml` sets `profile.dev.opt-level = 2` while keeping debug assertions enabled.

## Benchmark Lanes

Primary three-lane smoke:

```bash
pnpm native:perf:smoke
```

Individual lanes:

```bash
pnpm native:worldgen:smoke
pnpm native:movement:smoke
pnpm native:movement-frame:smoke
pnpm native:timedemo:smoke
```

Release-oriented lanes:

```bash
pnpm native:worldgen:perf
pnpm native:movement:perf
pnpm native:movement-frame:perf
pnpm native:timedemo:perf
```

Lower-level scheduler isolation:

```bash
pnpm native:runtime:smoke
pnpm native:runtime:perf
```

### What Each Lane Measures

- `native:worldgen:*`: surface chunk generation plus cold/warm full `FEATURES` batch generation. Reports dependency generation, carvers, feature decoration, cache hits, and chunks/sec.
- `native:movement:*`: integrated native client/server movement path. Reports chunk load/unload, scheduler polling, remesh time, dirty render-section rebuilds, and visible-vs-loaded face pressure.
- `native:movement-frame:*`: headless live-frame walking probe. Moves at spectator speed without fully draining render work each step and reports frame-budget misses, poll/remesh/upload/render timing, and render compile queue counters.
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
