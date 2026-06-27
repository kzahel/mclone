# Standalone Quest Performance Records

This file records standalone Quest / Android XR APK performance baselines by
date and commit. Keep one row per benchmark lane so regressions and
improvements are easy to compare over time.

Scope: these records are for the app running on the headset itself. They do not
cover desktop OpenXR hosted on the PC with Quest acting as a streaming client
through VirtualDesktopXR, WiVRn, Link, SteamVR, or similar runtimes.

## How To Add A Row

1. Commit the runtime code you want to benchmark.
2. Run one or more Quest benchmark scripts.
3. Add rows below with the benchmarked commit hash, device/runtime details, and
   the compact `MCLONE_ANDROID_XR_PERF_SUMMARY` numbers.

If a benchmark is captured from an uncommitted worktree, record that explicitly
and name the later commit that contains the same runtime code.

Current summaries include refresh fields (`refresh_supported`, `current_hz`,
`supported_hz`, `target_hz`, `budget_ms`) and max stage timings
(`max_locate_views_ms`, `max_locomotion_ms`, `max_acquire_*`,
`max_terrain_*`, `max_release_eyes_ms`, `max_end_frame_ms`). Add extra columns
or a secondary detail table when those fields are relevant to the change being
tracked.

## Current Standalone Quest Lanes

Automated no-clip flight at walking-like speed:

```bash
pnpm native:android-xr:perf:flight:rd1
pnpm native:android-xr:perf:flight:rd5
pnpm native:android-xr:perf:flight:rd10
pnpm native:android-xr:perf:flight:sweep
```

These run a 20 second local integrated sample after the first submitted terrain
frame. The validator force-stops the app and sleeps the headset during cleanup.

## Records

### 2026-06-27 - Standalone Quest 3 Automated Flight Sweep

Benchmarked code commit: `446d47349cdab2a84d2e7d05ccbf2516b496da21`
(`Add Quest XR flight performance probe`).

Capture note: these samples were captured from the implementation worktree
before it was committed. The runtime code used for the run is now committed as
the hash above.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Target used by probe | `72.0 Hz` / `13.889 ms` fallback |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Flight | no-clip, `4.3 blocks/s`, about `86 blocks` over the sample |

Schema note: this baseline predates the refresh/stage-attribution fields added
after `446d473`, so it records only the coarse `max_render_mclone_frame_ms`
bucket.

Summary:

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Drawn indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `446d473` | `native:android-xr:perf:flight:rd1` | 1 | `20.005s` | `72.0` | 1,441 | 0 | `13.887ms` | `15.070ms` | `15.696ms` | `27.190ms` | `11.343ms` | 719 | 0 | 0 | 47 | 11 | 248,940 | 65,868 | `86.035` |
| 2026-06-27 | `446d473` | `native:android-xr:perf:flight:rd5` | 5 | `20.017s` | `57.8` | 1,157 | 0 | `17.141ms` | `28.148ms` | `33.166ms` | `36.544ms` | `36.429ms` | 881 | 62 | 0 | 533 | 85 | 2,461,572 | 496,350 | `86.108` |
| 2026-06-27 | `446d473` | `native:android-xr:perf:flight:rd10` | 10 | `20.004s` | `22.2` | 444 | 0 | `48.764ms` | `56.691ms` | `74.015ms` | `83.101ms` | `82.951ms` | 442 | 377 | 41 | 1,265 | 186 | 5,633,760 | 1,138,038 | `86.029` |

Interpretation:

- Render distance 1 is close to the 72 Hz target. `Over 1x` is noisy because
  current `frame_wall_ms` includes OpenXR wait/pacing, but p95/p99 are still
  useful for tail comparison.
- Render distance 5 is already app/render limited: effective FPS drops to
  about 58 and max render time tracks max frame time.
- Render distance 10 is far over Quest budget in the current renderer path.
  Treat it as a stress lane, not a viable headset default.

Next comparable rows should keep the same seed, flight speed, sample duration,
view pose, and render-distance lanes unless the record explicitly says why the
lane changed.
