# 049: Native Sky Source-Section Ownership

Status: completed first pass.

## Purpose

Continue from
[`048-native-sky-empty-section-light-setup.md`](048-native-sky-empty-section-light-setup.md)
by moving sky source-section ownership into `SkyLightSectionStorage`, matching
the Java module boundary more closely.

The previous slice fixed the largest duplicate sky graph work by skipping
all-air sections, but retained initial light setup still manually scanned top
non-empty section rows and called `check_sky_source` block by block. Java owns
that work inside `SkyLightSectionStorage` using source-section add/remove
queues.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`

Important reference shape:

- `SkyLightSectionStorage` owns `sectionsWithSources`,
  `sectionsToAddSourcesTo`, `sectionsToRemoveSourcesFrom`, and
  `columnsWithSkySources`.
- `enableLightSources(...)` is column-owned and queues the current top storing
  section when a sky-enabled column appears.
- `onNodeAdded(...)` moves source ownership to a newly higher top section and
  queues removal of the previous source section.
- `markNewInconsistencies(...)` turns source-section changes into graph source
  edges before the graph drain.

## Scope

Landed in this slice:

- Added native `SkySourceUpdate` / `SkySourceUpdateKind`.
- Added source-section sets and add/remove queues to
  `mclone-light/src/sky_storage.rs`.
- `SkyLightSectionStorage.enable_light_sources(...)` now queues source-section
  add/remove work from the current top section.
- `SkyLightSectionStorage.on_node_added(...)` moves source ownership when a
  higher section becomes the top storing section.
- `SkyLightEngine.has_work()` now includes pending source-section work.
- `SkyLightEngine.run_updates_report(...)` drains source-section updates into
  graph source edges before graph propagation.
- Retained initial lighting and the test-only bridge no longer scan sky source
  blocks manually; they only set section statuses and enable sky source columns.
- Added focused storage tests for queueing a source section and moving source
  ownership to a higher top section.

## Out Of Scope

- Full Java `SkyLightSectionStorage.markNewInconsistencies(...)` behavior for
  `LIGHT_ONLY` sections that fills whole source sections and checks horizontal
  source boundaries. Native still keeps the current reduced opacity-filtered
  source-row behavior to preserve existing light values.
- Java `SkyLightEngine.checkNeighborsAfterUpdate(...)` skip-through behavior for
  missing vertical light-storage sections.
- Removal/unload correctness beyond the first queued source-section removal
  shape; retained light-world unload policy is still pending.
- Live `checkBlock` deltas and render-section dirtying.

## Result

The P6.9 performance win held while sky source ownership moved into the storage
module:

| Metric | P6.9 empty-section setup | P6.10 source storage |
|---|---:|---:|
| total elapsed | `1,931.110 ms` | `1,927.655 ms` |
| light-status compute | `485.407 ms` | `455.894 ms` |
| `LevelLightEngine.run_all_updates` | `415.744 ms` | `402.035 ms` |
| block graph drain | `29.855 ms` | `30.788 ms` |
| sky graph drain | `385.856 ms` | `371.212 ms` |
| sky source scan | `14.736 ms` | `0.000 ms` |
| sky source enqueue | `3.872 ms` | `0.000 ms` |
| block processed nodes | `52,348` | `52,348` |
| sky processed nodes | `794,466` | `794,466` |
| max sky queue before | `60,461` | `60,450` |

Interpretation: this is mainly a Java-shaped ownership improvement. It removes
manual retained-world source scanning without reintroducing the old sky graph
churn.

## Validation

Completed on 2026-06-19:

- `cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -- --check`
- `cargo test --manifest-path native/Cargo.toml -p mclone-light -p mclone-server`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
- native radius-5 lighting-enabled perf:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting`
- native full-frame screenshot:
  `/tmp/mclone-light-sky-source-storage.png`

Screenshot result: `960x540`, `166` cached sections, `26` drawn sections,
nonblank terrain with the same visible canopy shadows as the P6.9 capture; no
blue-sky-only failure.

## Next

The next likely lighting parity slice is Java sky neighbor skip-through
propagation:

- port `SkyLightEngine.checkNeighborsAfterUpdate(...)` downward skip behavior
  through missing vertical sections
- port the horizontal checks that use the skipped-down source node when side
  neighbors are in a lower storing section
- add focused synthetic fixtures for empty-section vertical gaps and side spread
  across a gap
- re-run radius-5 graph metrics and screenshot validation

After that, return to live `checkBlock` deltas and render-section dirtying by
changed light sections.
