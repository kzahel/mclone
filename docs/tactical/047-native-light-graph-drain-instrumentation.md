# 047: Native Light Graph Drain Instrumentation

Status: completed first pass.

## Purpose

Continue from
[`046-native-retained-initial-light-world.md`](046-native-retained-initial-light-world.md)
by making the remaining cold-start `LevelLightEngine.run_all_updates` cost
visible per light layer, then apply the first safe graph-drain optimization.

The reference Java engine uses `DynamicGraphMinFixedPoint` with primitive
fastutil long maps/sets and exposes graph queue size as the primary direct
diagnostic. The native port should preserve that module shape: graph internals
stay in `mclone_light`, while server/runtime code only consumes aggregate
timing and queue reports.

## Reference Source

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/DynamicGraphMinFixedPoint.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LayerLightEngine.java`

Important reference shape:

- `DynamicGraphMinFixedPoint` owns per-level queues and pending computed levels.
- Java's queues/maps are primitive long collections, not tree maps.
- `runUpdates(...)` consumes a node budget and updates `hasWork`.
- `getQueueSize()` is the exposed direct graph queue metric.
- `LayerLightEngine.runUpdates(...)` surrounds graph work with storage update
  and swap work; the graph remains a layer-local concern.

## Scope

Landed in this slice:

- Added `DynamicGraphRunReport` for per-drain budget, processed node count,
  and queue size before/after.
- Added `LightLayerRunReport` and `LevelLightRunReport` so native `LevelLightEngine`
  can report block-vs-sky graph calls, processed nodes, queue high water marks,
  and per-layer drain time.
- Threaded those reports through `BlockLightEngine`, `SkyLightEngine`,
  retained initial light computation, scheduler metrics, and
  `scheduler_movement_smoke` JSON.
- Replaced the graph's `BTreeMap` / `BTreeSet` backing collections with
  mixed integer-key `HashMap` / `HashSet` collections. Packed block-position
  keys clustered badly under identity hashing, so the retained version mixes
  keys before table lookup, matching the performance intent of Java's primitive
  fastutil maps.

## Out Of Scope

- Changing propagation semantics.
- Reducing sky-light duplicate queued work.
- Java-shaped task priority, cancellation, or light ticket release policy.
- Live `checkBlock` updates, light delta packets, or render-section dirtying.
- Rendering changes such as `LightTexture` or ambient occlusion.

## Result

The new metrics make the bottleneck concrete:

- Block graph drain is small: `52,348` processed nodes and `32.612 ms` after
  the hash-map optimization in the radius-5 run.
- Sky graph drain dominates: `10,002,274` processed nodes and `5,152.656 ms`
  after the hash-map optimization.
- Graph node counts were identical before and after the collection change,
  which keeps this slice a data-structure optimization rather than a propagation
  behavior change.

The raw identity-hash experiment was rejected because packed block-position
keys clustered and made the same run much slower.

## Validation

Completed on 2026-06-19:

- `cargo fmt --manifest-path native/Cargo.toml -p mclone-light -p mclone-server -- --check`
- `cargo test --manifest-path native/Cargo.toml -p mclone-light`
- `cargo test --manifest-path native/Cargo.toml -p mclone-server`
- native radius-5 lighting-enabled perf:
  `cargo run --release --quiet --manifest-path native/Cargo.toml -p mclone-server --bin scheduler_movement_smoke -- --radius 5 --steps 1 --poll-mode sleep --poll-sleep-ms 1 --max-polls 300000 --enable-lighting`

Radius-5 perf comparison:

| Lane | Total elapsed | Light compute | `run_updates` | Block graph | Sky graph |
|---|---:|---:|---:|---:|---:|
| Instrumented baseline | `10,614.161 ms` | `9,173.467 ms` | `9,115.460 ms` | `48.058 ms` | `9,067.052 ms` |
| Raw identity hash rejected | `35,665.194 ms` | `34,206.109 ms` | `33,919.280 ms` | `189.468 ms` | `33,729.449 ms` |
| Mixed hash kept | `6,733.051 ms` | `5,236.809 ms` | `5,185.626 ms` | `32.612 ms` | `5,152.656 ms` |

Mixed-hash graph counters:

| Metric | Value |
|---|---:|
| `run_update_iterations` | `614` |
| block run-update calls | `614` |
| sky run-update calls | `614` |
| block processed nodes | `52,348` |
| sky processed nodes | `10,002,274` |
| max block queue before | `27,663` |
| max sky queue before | `57,600` |
| final block queue after | `0` |
| final sky queue after | `0` |

Screenshot validation for the kept version:

```text
/tmp/mclone-light-graph-drain.png
```

The native full-frame capture used seed `12345`, chunk `(0,0)`, render distance
`2`, `960x540`, with server lighting enabled and shader fullbright disabled.
It was inspected and rendered nonblank terrain with `166` cached sections and
`26` drawn sections.

## Next

The next likely lighting throughput slice is sky graph duplicate-work
reduction:

- inspect sky-source enqueue and section-activation patterns against Java
  `SkyLightEngine`
- explain why the cold radius-5 batch processes roughly `10M` sky graph nodes
  for a `57,600` maximum queue size
- reduce redundant sky neighbor checks or section source checks without
  changing Java-visible light values
- keep the solution in `mclone_light` / retained light-world setup, not in
  `scheduler.rs`

After the sky graph node count is lower, re-run radius-5 cold startup and
multi-step movement before returning to live light deltas and render dirtying.
