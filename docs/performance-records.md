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

## Near-Term Perf Questions

- Keep tracking whether graph culling is enabled for each camera lane. Movement and timedemo now both exercise the graph; outside-retained-section traversal seeding is still a separate Java-parity follow-up for ad-hoc camera paths.
- Add release records before enforcing budgets.
- Add larger radius/view-distance variants once radius `1` is stable enough to avoid hiding regressions in bootstrap noise.
