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
`RUNTIME_MAX`, `QUEUE_MAX`, `COMPILE_MAX`, `UPLOAD_LAST`, and `DRAW`. They
include refresh fields (`refresh_supported`, `current_hz`, `supported_hz`,
`target_hz`, `budget_ms`), max stage timings, terrain runtime
poll/sync/GPU-upload timings, runtime poll sub-buckets, server/scheduler queue
state, compile/upload workload counters, and draw counts. Add extra columns or
a secondary detail table when those fields are relevant to the change being
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

### 2026-06-27 - Standalone Quest 3 Flight Sweep With Runtime Poll Attribution

Benchmarked code commit: `320bdd2610b45b9e9947f916f75db1cf84b96fd5`
(`Fix explicit empty light section packing`).

Capture note: captured from a detached clean worktree at the commit above, with
the local ignored `reference/` assets junctioned in for APK asset staging. The
marker block includes `RUNTIME_MAX` and `QUEUE_MAX`.

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

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Drawn indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `320bdd2` | `native:android-xr:perf:flight:rd1` | 1 | `20.007s` | `72.0` | 1,441 | 0 | `13.881ms` | `14.967ms` | `15.738ms` | `27.327ms` | `11.883ms` | 713 | 0 | 0 | 47 | 11 | 247,308 | 65,868 | `86.048` |
| 2026-06-27 | `320bdd2` | `native:android-xr:perf:flight:rd5` | 5 | `20.017s` | `56.6` | 1,132 | 0 | `17.480ms` | `27.737ms` | `33.756ms` | `44.455ms` | `40.183ms` | 909 | 56 | 0 | 538 | 89 | 2,531,364 | 537,858 | `86.040` |
| 2026-06-27 | `320bdd2` | `native:android-xr:perf:flight:rd10` | 10 | `20.047s` | `21.0` | 421 | 0 | `51.758ms` | `65.606ms` | `78.369ms` | `109.878ms` | `109.726ms` | 421 | 361 | 71 | 1,276 | 185 | 5,681,634 | 1,131,312 | `86.134` |

Terrain timing maxima:

| RD | Terrain frame | Runtime total | Runtime poll | Sync sections | GPU upload | Ready refresh | Left eye | Right eye |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `11.536ms` | `5.648ms` | `2.412ms` | `5.601ms` | `1.813ms` | `0.113ms` | `5.241ms` | `4.484ms` |
| 5 | `38.065ms` | `31.360ms` | `28.122ms` | `18.350ms` | `6.349ms` | `0.496ms` | `15.867ms` | `6.588ms` |
| 10 | `98.805ms` | `84.650ms` | `76.535ms` | `18.895ms` | `3.005ms` | `1.253ms` | `10.557ms` | `20.844ms` |

Runtime poll maxima:

| RD | Poll total | Drain updates | Apply updates | Dirty mark | Client apply | Diagnostics | Server tick | Scheduler tick | Updates | Snapshot | Section | Unload |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `2.410ms` | `0.004ms` | `0.003ms` | `0.001ms` | `0.002ms` | `2.409ms` | `3.684ms` | `3.670ms` | 2 | 0 | 0 | 0 |
| 5 | `28.120ms` | `0.004ms` | `0.002ms` | `0.001ms` | `0.001ms` | `28.119ms` | `12.494ms` | `12.471ms` | 2 | 0 | 0 | 0 |
| 10 | `76.533ms` | `0.002ms` | `0.002ms` | `0.001ms` | `0.001ms` | `76.531ms` | `29.737ms` | `29.693ms` | 2 | 0 | 0 | 0 |

Queue maxima:

| RD | Server cmd q | Server update q | Server jobs | Publications | Scheduler jobs | Completed jobs | Dirty chunks | Loaded chunks | Visible chunks | Ticket chunks | Player visible | Player outbound |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 0 | 5 | 4 | 5 | 5 | 65 | 65 | 9 | 841 | 9 | 0 |
| 5 | 4 | 0 | 5 | 4 | 5 | 4 | 265 | 265 | 121 | 1,369 | 121 | 0 |
| 10 | 3 | 0 | 5 | 4 | 5 | 4 | 499 | 499 | 289 | 1,849 | 289 | 0 |

Upload maxima:

| RD | Work frames | Rebuilt sections | Removed sections | Rebuilt indices | Uploaded sections | Upload removed | Uploaded indices | Ready sections |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 825 | 16 | 48 | 50,064 | 7 | 20 | 50,064 | 112 |
| 5 | 188 | 16 | 160 | 42,588 | 8 | 50 | 42,588 | 1,296 |
| 10 | 348 | 16 | 240 | 41,064 | 8 | 78 | 41,064 | 3,600 |

Compile / streaming maxima:

| RD | Pending chunks before | Pending chunks after | Pending jobs before | Pending jobs after | Deferred sections | Submitted sections | Completed sections | Stale sections | Visibility total | Visibility worst |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 12 | 9 | 1 | 1 | 16 | 16 | 16 | 1 | `0.890ms` | `0.286ms` |
| 5 | 59 | 49 | 1 | 1 | 16 | 16 | 16 | 16 | `0.703ms` | `0.394ms` |
| 10 | 98 | 85 | 1 | 1 | 16 | 16 | 16 | 16 | `0.732ms` | `0.327ms` |

Interpretation:

- Render distance 1 remains a viable 72 Hz lane. p95/p99 are close to budget
  and there are no over-2x frames.
- Render distance 5 remains uneven at about `56.6 FPS`. The worst app frame is
  dominated by runtime polling (`28.122ms`) and section sync (`18.350ms`), not
  raw GPU upload (`6.349ms`).
- Render distance 10 remains a stress lane at about `21 FPS`, with a worst
  frame over `100ms`. Runtime polling (`76.535ms`) dominates the terrain update
  bucket.
- The runtime poll split points at diagnostics collection: `poll_diagnostics_ms`
  is effectively equal to `poll_total_ms` in all three lanes, while
  `drain_updates_ms`, `apply_updates_ms`, dirty marking, and client application
  are near zero. The update payload was tiny (`2` updates, no snapshot or unload
  updates).
- The expensive diagnostics path scales with scheduler state: dirty/loaded
  chunks increase from `65` to `265` to `499`, and ticket chunks from `841` to
  `1,369` to `1,849`. The next optimization should make the diagnostics
  snapshot cheaper or less frequent on headset frames before changing mesh
  upload policy.

### 2026-06-27 - Standalone Quest 3 Flight Sweep With Upload Attribution

Benchmarked code commit: `2bd0e04b3e4ea64c8698b5e91571d40f19b1de39`
(`Split Quest XR perf markers and add upload counters`).

Capture note: captured from a clean worktree at the commit above. This run uses
the split marker block, so draw counts and upload/compile counters are not
truncated by logcat.

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

| Date | Commit | Lane | RD | Sample | FPS | Frames | Skipped | p50 | p95 | p99 | Max | Max render | Over 1x | Over 2x | Over 4x | Sections | Drawn sections | Indices | Drawn indices | Distance |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2026-06-27 | `2bd0e04` | `native:android-xr:perf:flight:rd1` | 1 | `20.008s` | `72.0` | 1,441 | 0 | `13.868ms` | `15.055ms` | `15.749ms` | `29.670ms` | `10.575ms` | 701 | 1 | 0 | 47 | 11 | 247,308 | 65,868 | `86.048` |
| 2026-06-27 | `2bd0e04` | `native:android-xr:perf:flight:rd5` | 5 | `20.026s` | `57.3` | 1,147 | 0 | `17.408ms` | `28.149ms` | `32.938ms` | `35.756ms` | `35.574ms` | 909 | 60 | 0 | 540 | 85 | 2,494,272 | 496,350 | `86.076` |
| 2026-06-27 | `2bd0e04` | `native:android-xr:perf:flight:rd10` | 10 | `20.034s` | `21.0` | 421 | 0 | `51.212ms` | `58.602ms` | `80.605ms` | `83.147ms` | `82.974ms` | 419 | 366 | 59 | 1,242 | 194 | 5,565,192 | 1,199,940 | `86.191` |

Terrain timing maxima:

| RD | Terrain frame | Runtime total | Runtime poll | Sync sections | GPU upload | Ready refresh | Left eye | Right eye |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `10.238ms` | `5.900ms` | `2.309ms` | `5.857ms` | `2.916ms` | `0.057ms` | `6.084ms` | `3.984ms` |
| 5 | `35.067ms` | `27.122ms` | `25.717ms` | `10.193ms` | `6.359ms` | `0.823ms` | `6.836ms` | `6.336ms` |
| 10 | `82.663ms` | `68.483ms` | `63.248ms` | `4.978ms` | `3.255ms` | `13.320ms` | `11.817ms` | `10.389ms` |

Upload maxima:

| RD | Work frames | Rebuilt sections | Removed sections | Rebuilt indices | Uploaded sections | Upload removed | Uploaded indices | Ready sections |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 815 | 16 | 48 | 50,064 | 7 | 20 | 50,064 | 112 |
| 5 | 196 | 16 | 160 | 42,588 | 8 | 50 | 42,588 | 1,296 |
| 10 | 350 | 16 | 240 | 41,064 | 8 | 78 | 41,064 | 3,600 |

Compile / streaming maxima:

| RD | Pending chunks before | Pending chunks after | Pending jobs before | Pending jobs after | Deferred sections | Submitted sections | Completed sections | Stale sections | Visibility total | Visibility worst |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 12 | 9 | 1 | 1 | 16 | 16 | 16 | 1 | `0.861ms` | `0.230ms` |
| 5 | 67 | 57 | 1 | 1 | 16 | 16 | 16 | 0 | `0.674ms` | `0.247ms` |
| 10 | 96 | 81 | 1 | 1 | 16 | 16 | 16 | 1 | `0.881ms` | `0.515ms` |

Interpretation:

- Render distance 1 still tracks 72 Hz closely. It had one over-2x frame, but
  p95/p99 are both close to budget and the app render bucket stays under
  `11ms`.
- Render distance 5 remains uneven at about `57 FPS`. The worst terrain frame
  is mostly the runtime/poll side (`25.717ms`) rather than GPU buffer upload
  (`6.359ms`) or per-eye drawing (`6-7ms`).
- Render distance 10 remains a stress lane at about `21 FPS`. The dominant max
  bucket is runtime polling (`63.248ms`), with traversal-ready refresh also
  visible (`13.320ms`), while GPU upload is only `3.255ms`.
- Max upload work is bounded to small batches (`7-8` uploaded non-empty
  sections, `16` submitted/completed sections), so the first optimization target
  is not raw `wgpu` upload bandwidth. The sharper target is runtime polling /
  chunk-stream application and traversal-ready work while flying.
- The last frame still had pending chunks (`4` / `40` / `71` for RD1/RD5/RD10)
  with zero pending compile jobs, which suggests backlog outside active mesh
  compilation.

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
