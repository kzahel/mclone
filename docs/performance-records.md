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
pnpm native:timedemo:smoke
```

Release-oriented lanes:

```bash
pnpm native:worldgen:perf
pnpm native:movement:perf
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
