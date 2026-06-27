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
   the compact `MCLONE_ANDROID_XR_PERF_*` marker-block numbers.

If a benchmark is captured from an uncommitted worktree, record that explicitly
and name the later commit that contains the same runtime code.

Current summaries are saved as a compact marker block:
`MCLONE_ANDROID_XR_PERF_SUMMARY`, `STAGES`, `TERRAIN`, `UPLOAD_MAX`,
`COMPILE_MAX`, `UPLOAD_LAST`, and `DRAW`. They include refresh fields
(`refresh_supported`, `current_hz`, `supported_hz`, `target_hz`, `budget_ms`),
max stage timings, terrain runtime poll/sync/GPU-upload timings, compile/upload
workload counters, and draw counts. Add extra columns or a secondary detail
table when those fields are relevant to the change being tracked.

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

### 2026-06-27 - Standalone Quest 3 Flight Sweep With Stage Attribution

Benchmarked code commit: `518d21da92b3850f9b8b9d75bad6c7cf1318fb49`
(`Add Quest XR perf timing attribution`).

Capture note: captured from a clean worktree at the commit above. This run used
the pre-split single summary line, which was long enough that logcat truncated
the trailing draw-count fields after `indices`; record those fields as missing
for this run. Later captures use the split marker block described above.

Device/runtime:

| Field | Value |
|---|---|
| Device | Meta Quest 3 |
| Android API | 34 |
| OpenXR runtime | Oculus `v204.201.0` |
| Stereo view config | `1680x1760` recommended per eye, `1x` sample |
| Supported refresh | `72.0,80.0,90.0,120.0 Hz` |
| Current/target refresh | `72.0 Hz` / `13.889 ms` |
| World | local integrated, seed `12345`, center chunk `(0, 0)`, noon, frozen time |
| Flight | no-clip, `4.3 blocks/s`, about `86 blocks` over the sample |

Summary:

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `518d21d` | `native:android-xr:perf:flight:rd1` | 1 | `20.005s` | `72.0` | 1,441 | 0 | `13.889ms` | `15.160ms` | `17.350ms` | `25.475ms` | `11.415ms` | 721 | 0 | 0 | 47 | 11 | 247,308 | `86.036` |
| 2026-06-27 | `518d21d` | `native:android-xr:perf:flight:rd5` | 5 | `20.011s` | `56.8` | 1,137 | 0 | `17.348ms` | `27.437ms` | `31.546ms` | `44.264ms` | `36.108ms` | 933 | 50 | 0 | 538 | 89 | 2,531,418 | `86.043` |
| 2026-06-27 | `518d21d` | `native:android-xr:perf:flight:rd10` | 10 | `20.005s` | `20.8` | 417 | 0 | `51.836ms` | `65.716ms` | `77.480ms` | `82.461ms` | `82.326ms` | 414 | 371 | 68 | 1,272 | 194 | 5,691,858 | `86.144` |

Stage attribution:

| RD | Max wait | Poll | Locate | Locomotion | Acquire L | Acquire R | Terrain frame | Render views | Upload | Left eye | Right eye | Release | End frame |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `22.087ms` | `0.429ms` | `0.078ms` | `1.524ms` | `4.848ms` | `0.040ms` | `11.182ms` | `0.045ms` | `4.561ms` | `6.731ms` | `4.315ms` | `0.141ms` | `0.594ms` |
| 5 | `17.410ms` | `0.672ms` | `0.189ms` | `4.163ms` | `5.229ms` | `0.052ms` | `35.783ms` | `0.047ms` | `29.300ms` | `6.105ms` | `6.831ms` | `0.353ms` | `1.053ms` |
| 10 | `0.388ms` | `0.832ms` | `0.181ms` | `4.504ms` | `15.198ms` | `0.078ms` | `79.015ms` | `0.077ms` | `66.443ms` | `13.458ms` | `10.015ms` | `0.170ms` | `0.931ms` |

Interpretation:

- Render distance 1 is still near the 72 Hz target. The app render bucket stays
  under budget, with tail frame time mostly reflecting OpenXR pacing/wait.
- Render distance 5 misses the 72 Hz budget often enough to feel uneven. The
  worst app frame is dominated by `runtime_upload` (`29.300ms`) inside the
  terrain frame, while per-eye rendering stays around `6-7ms`.
- Render distance 10 is a stress lane. It is heavily app/render limited, with
  `runtime_upload` peaking at `66.443ms` and terrain frame time at `79.015ms`.
- `skipped_delta=0` across the sweep means the runtime did not report skipped
  frames during the samples, but the app submitted far fewer frames at RD5/RD10.
  The current data points more toward upload/chunk-stream stalls than joystick
  motion or a simple 60 Hz vs 72 Hz interpolation mismatch.

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
