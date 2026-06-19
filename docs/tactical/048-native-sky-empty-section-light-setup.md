# 048: Native Sky Empty-Section Light Setup

Status: completed first pass.

## Purpose

Continue from
[`047-native-light-graph-drain-instrumentation.md`](047-native-light-graph-drain-instrumentation.md)
by reducing sky graph queue churn at the source.

The measured P6.8 graph split showed sky propagation still processed roughly
`10M` graph nodes for a radius-5 cold startup batch. The Java reference does not
mark every vertical chunk section as active light storage during initial
lighting; it calls `updateSectionStatus(..., false)` only for non-empty
`LevelChunkSection`s and enables sky sources once per chunk column.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ThreadedLevelLightEngine.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightEngine.java`

Important reference shape:

- `ThreadedLevelLightEngine.lightChunk(...)` iterates chunk sections and calls
  `updateSectionStatus(section, false)` only when
  `!LevelChunkSection.isEmpty(section)`.
- Client chunk replacement passes the real `LevelChunkSection.isEmpty(...)`
  value for every section.
- `SkyLightSectionStorage.enableLightSources(...)` is column-owned and queues a
  source section from the current top storing section.
- `SkyLightEngine.checkNeighborsAfterUpdate(...)` can skip downward through
  missing light-storage sections when propagating sky changes.

## Scope

Landed in this slice:

- Retained initial light setup now computes a Java-shaped section-empty flag
  from raw generated blocks.
- `LevelLightEngine.update_section_status(section, is_empty)` is called with
  the real empty flag instead of unconditionally activating every vertical
  section.
- Light sources are enabled once per changed chunk column instead of once per
  section.
- Manual native sky-source seeding now starts from the top block row of the
  highest non-empty section in the chunk, instead of the build-limit top row
  that often belongs to an all-air, non-storing section.
- The test-only level-light bridge mirrors the retained path so fixtures keep
  using the same setup shape.
- Added focused retained-world tests for empty-section classification and sky
  source seed height.

## Out Of Scope

- Full Java `SkyLightSectionStorage` source-inconsistency queues:
  `sectionsWithSources`, `sectionsToAddSourcesTo`, and
  `sectionsToRemoveSourcesFrom`.
- Full Java `SkyLightEngine.checkNeighborsAfterUpdate(...)` skip-through
  propagation across missing vertical sections.
- Live block-change light deltas.
- Light ticket release, cancellation, and retained-state unload policy.
- Render `LightTexture` and ambient occlusion.

## Result

Radius-5 cold startup lighting cost dropped sharply because all-air sections no
longer become active sky light-storage sections:

| Metric | P6.8 baseline | P6.9 empty-section setup |
|---|---:|---:|
| total elapsed | `6,733.051 ms` | `1,931.110 ms` |
| light-status compute | `5,236.809 ms` | `485.407 ms` |
| `LevelLightEngine.run_all_updates` | `5,185.626 ms` | `415.744 ms` |
| block graph drain | `32.612 ms` | `29.855 ms` |
| sky graph drain | `5,152.656 ms` | `385.856 ms` |
| block processed nodes | `52,348` | `52,348` |
| sky processed nodes | `10,002,274` | `794,466` |
| run-update iterations | `614` | `52` |
| max sky queue before | `57,600` | `60,461` |

Interpretation: block-light work is unchanged, while sky work is much smaller.
The higher max sky queue is acceptable here because the important cost was
repeated processing of sky nodes, not peak queued node count.

## Validation

Completed on 2026-06-19:

- `cargo fmt --manifest-path native/Cargo.toml -p mclone-server -p mclone-light -- --check`
- `cargo test --manifest-path native/Cargo.toml -p mclone-light -p mclone-server`
- `cargo check --manifest-path native/Cargo.toml --workspace`
- `cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown`
- `git diff --check`
- native radius-5 lighting-enabled perf:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting`
- native full-frame screenshot:
  `/tmp/mclone-light-sky-empty-sections.png`

Screenshot result: `960x540`, `166` cached sections, `26` drawn sections,
nonblank terrain with visible tree canopy shadows; visually inspected with no
blue-sky-only failure.

## Next

The next likely lighting parity slice was Java sky source-section ownership,
now tracked by
[`049-native-sky-source-section-ownership.md`](049-native-sky-source-section-ownership.md):

- port `SkyLightSectionStorage` source-section sets and add/remove queues
- move manual sky-source seeding out of the retained setup and into sky storage
  inconsistencies, closer to Java
- port `SkyLightEngine.checkNeighborsAfterUpdate(...)` skip-through behavior for
  missing vertical sections
- re-run the same radius-5 graph metrics and screenshot validation

After sky source-section ownership matches Java more closely, return to live
`checkBlock` deltas and render-section dirtying by changed light sections.
