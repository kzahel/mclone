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
pnpm native:movement:smoke
pnpm native:movement-frame:smoke
pnpm native:startup-streaming:smoke
pnpm native:loading-settle:smoke
pnpm native:timedemo:smoke
```

Release-oriented lanes:

```bash
pnpm native:worldgen:perf
pnpm native:movement:perf
pnpm native:movement-frame:perf
pnpm native:startup-streaming:perf
pnpm native:startup-streaming:perf:rd20-long
pnpm native:loading-settle:perf
pnpm native:timedemo:perf
```

Quest/OpenXR guardrail lanes:

```bash
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
- `native:movement:*`: integrated native client/server movement path. Reports chunk load/unload, scheduler polling, remesh time, dirty render-section rebuilds, and visible-vs-loaded face pressure.
- `native:movement-frame:*`: headless live-frame walking probe. Moves at spectator speed without fully draining render work each step and reports frame-budget misses, poll/remesh/upload/render timing, and render compile queue counters.
- `native:startup-streaming:*`: desktop-shaped local startup and streaming probe. The default perf lane uses RD10 at a 60 Hz budget for faster iteration and easier comparison with Quest RD10 guardrails. Uses the same local startup pump to enter at the playable gate, then advances a paced headless frame loop that polls the runtime and syncs render sections under a frame deadline while the requested view fills in. Reports enter-playable time, first full-view-ready frame/time, first render-quiescent frame/time, frame-budget misses, runtime poll/remesh/upload/render timing, queue counters, and final readiness. RD20 is an explicit long-run lane, not the default iteration target.
- `native:android:*`: flat Android validation on AVD and Quest-as-panel. These lanes prove Android packaging, asset staging, NativeActivity startup, wgpu surface creation, touch UI, and first rendered-frame behavior. They are useful for Android startup timing, but they are not OpenXR frame-pacing proof and do not currently emit dropped-frame/headroom metrics.
- `native:android-xr:perf:*rd10*`: Quest/OpenXR RD10 frame-pacing guardrails. These currently report headset app-work/headroom, dropped/stale frames, runtime/render/upload/compile tails, and Meta performance metrics where enabled. They do not yet emit the same playable/full-view-ready/render-quiescent startup-streaming markers as the desktop startup-streaming lane.
- `native:loading-settle:*`: synthetic full-drain isolation probe. Creates fresh transient worlds at fixed render distances, spawns the player at the seed-derived spawn center, waits for all target chunks to become light-ready, then synchronously builds render sections. Reports runtime settle time, render mesh settle time, chunks/sec, simulation time, and pending queue counters. Use it to split server/light/runtime cost from mesh cost, not as the primary desktop startup policy target.
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
- Run Quest/OpenXR RD10 frame-pacing guardrails after any throughput policy
  changes. The expected result should be no submitted-frame drops and no stale
  frames; otherwise desktop throughput changes are not acceptable for Quest.

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
`120 Hz`. This is retained as an explicit long-run checkpoint. The default
desktop/Quest comparison lane should use RD10 for iteration.

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
