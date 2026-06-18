# 044: Native Light Status Worker And Disable Flag

Status: completed first pass.

## Purpose

Build on
[`043-native-loaded-light-hydration.md`](043-native-loaded-light-hydration.md)
by moving native initial `ChunkStatus::Light` payload computation off the
foreground scheduler poll path and adding a diagnostic command-line switch to
bypass lighting.

This slice addresses the desktop stutter observed after native light status
scheduling landed. It does not try to complete the full Java
`ThreadedLevelLightEngine` task model.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java`

Important reference shape:

- `ChunkMap.schedule(...)` gives `ChunkStatus.LIGHT` its own scheduling boundary
  and light ticket.
- `ThreadedLevelLightEngine` queues `PRE_UPDATE` and `POST_UPDATE` tasks through
  a mailbox instead of running propagation from the immediate scheduling caller.
- `lightChunk(...)` marks the chunk not light-correct, performs section/source
  setup as pre-update work, drains propagation, then marks the chunk
  light-correct in post-update work.
- `checkBlock(...)`, `queueSectionData(...)`, `retainData(...)`, and
  `updateSectionStatus(...)` are task-queued on the threaded wrapper.

## Scope

Land a narrow native worker boundary:

- add `mclone-server/src/light_mailbox.rs`
- compute `PendingLightStatus::compute_light_sections()` on a native worker
  thread for desktop targets
- keep a WASM inline backend until native-web worker plumbing exists
- keep scheduler-owned status publication and the existing completed-light
  publication budget
- expose integrated-server lighting mode controls
- add `--lighting true|false`, `--disable-lighting`, and `--enable-lighting`
  to the native client
- make `--disable-lighting` target `ChunkStatus::Features` for runtime chunks
  and publish feature snapshots without light sections
- default `--disable-lighting` to shader fullbright unless the caller
  explicitly passes a fullbright option
- place startup/headless spectator views above the loaded surface column so
  screenshots and first window frames do not start inside terrain

## Out Of Scope

- Java-equivalent priority queues, task-per-batch behavior, and light ticket
  release
- cancellation of stale light jobs
- a long-lived per-world threaded `LevelLightEngine`
- live `checkBlock` / block-emission update queues
- light delta publication and render-section dirtying
- Java face-shape occlusion
- full vanilla `BlockState.getLightBlock(...)` and emission tables
- render `LightTexture`
- ambient occlusion

## Result

Landed:

- `LightStatusMailbox` owns the async boundary for initial light status work.
- Native desktop starts a `mclone-light-status` worker and returns completed
  `ChunkStatus::Light` payloads to the scheduler for publication.
- WASM keeps an inline mailbox backend, preserving native-web compatibility.
- `ChunkScheduler` can run with lighting enabled or disabled.
- When lighting is disabled, runtime chunks target `ChunkStatus::Features`,
  no `Light` status slot is scheduled, and client-visible feature snapshots
  publish without light sections.
- The native CLI exposes the lighting mode and logs whether window runtime
  lighting is enabled.
- Full-frame headless screenshots and window startup now adjust the spectator
  camera to a loaded surface column before rendering.

Remaining parity gaps: this worker removes the foreground propagation stall, but
it is still only a first mailbox boundary. The Java threaded wrapper's task
priorities, batching, cancellation, tickets, and live-update queues remain
future work.

## Diagnostics

Lighting-enabled window path:

```bash
cargo run -p mclone-native-client
```

Lighting-disabled comparison path:

```bash
cargo run -p mclone-native-client -- --disable-lighting
```

Use this if you want to bypass server-side light status without forcing shader
fullbright:

```bash
cargo run -p mclone-native-client -- --disable-lighting --disable-fullbright
```

## Validation

Completed on 2026-06-18:

- `cargo fmt --manifest-path native/Cargo.toml -p mclone-server -p mclone-native-client -- --check`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- `cargo test --manifest-path native/Cargo.toml -p mclone-native-client`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
- native movement-frame probe:
  `cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- --movement-frame-probe --frame-budget-frames 30 --path-radius 1 --target-hz 60`
- native full-frame captures:
  `/tmp/mclone-light-worker-enabled-day.png`
  `/tmp/mclone-light-worker-disabled-day.png`

Movement-frame probe result:

| Metric | Value |
|---|---:|
| over-budget frames | `0` |
| over 2x budget frames | `0` |
| over 4x budget frames | `0` |
| p95 frame time | `7.824 ms` |
| p99 frame time | `9.280 ms` |
| max frame time | `9.280 ms` |
| runtime setup | `28,497.187 ms` |
| initial poll count | `14,279` |
| initial poll time | `10,544.738 ms` |
| initial sections | `400` |

The captures were inspected and both rendered nonblank daytime terrain with
`166` cached sections and `26` drawn sections.

## Next

The immediate desktop follow-up is startup responsiveness: the window path still
waits for the initial light-ready view before opening, which avoids a sky-only
first frame but makes launch latency high.

The next pure lighting subsystem remains live light deltas:

- detect runtime opacity/emission changes
- route changes through `LevelLightEngine.checkBlock(...)` /
  `onBlockEmissionIncrease(...)`
- publish changed light sections to clients
- dirty render sections and neighboring sections affected by changed light
