# Tactical 56 - Worldgen performance investigation

Build a repeatable performance investigation harness before choosing Rust/C/WASM, worker pools, or more aggressive TypeScript optimization.

Status: proposed. This tactical should run before broad worldgen optimization work. It can run before or after [`49-vanilla-status-futures-and-partial-chunks.md`](49-vanilla-status-futures-and-partial-chunks.md), but the results should be interpreted with the known status-future and partial-state gap in mind.

## Goal

Separate the current stutter risk into measurable causes:

- raw chunk generation cost
- oversized cooperative work quanta that block host input/poll/player ticks
- normal-publication status closure size, duplicate in-flight work, and missing partial-status reuse
- snapshot packing, transfer, render-world ingest, mesh build, and GPU upload cost
- browser frame pacing cost on real browser/GPU/mobile hosts

The outcome should be a report that makes the next action obvious:

- keep TypeScript and tune scheduling/status orchestration
- split one host phase into smaller cooperative units
- move a narrow measured-hot kernel to WASM
- defer WASM because the bottleneck is transport/meshing/rendering
- require a real browser/mobile run before drawing browser-frame conclusions

## Current context

The repo already has the right architectural bias: TypeScript first, with a WASM escape hatch only for measured hot modules. Do not start a native rewrite from subjective phone stutter alone.

Existing evidence:

- `D5`/`D6` found that the old blocking `set_chunk_view` path was the first real stutter source.
- Cooperative chunk-view scheduling now acknowledges interest quickly and streams snapshots through `poll_world_updates`.
- `pnpm perf:worldgen` exists, but direct phase mode is stale against the current `GeneratedRenderLevel` constructor and only accepts `default`/`browser_smoke`.
- A local host-only check from this workspace showed `flat_grass` can publish a radius-1 view much faster than `default`, which is useful signal, but not enough to decide on WASM.

Known caveat:

[`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md) records the expected status-closure counts. For the current 5x5 normal-publication policy, `121` materialized terrain chunks and `81` `FEATURES` chunks are vanilla-shaped closure, not proof of duplicate generation. Treat the remaining performance question as: are we doing each required chunk/status once, are in-flight requests coalesced like `ChunkHolder`, and can partial `ProtoChunk`-like state survive route changes/unload/save instead of being recomputed?

## Host capability policy

This tactical has two lanes:

1. **Headless host lane**: required first. It must run on this kind of host with no display session. It measures host scheduling, worldgen phases, status counts, chunk publication, and synthetic player/chunk-interest traversal.
2. **Browser fly-by lane**: required before making browser/mobile conclusions. It measures actual frame pacing, long tasks, render-world ingestion, mesh upload, and screenshots. Run only when `pnpm host:check` reports a viable browser/WebGPU path.

Do not block the headless lane waiting for a browser host. Do not claim browser frame smoothness from the headless lane.

## Source files

Read these before implementation:

| Source | Purpose |
|---|---|
| [`scripts/worldgen-phase-baseline.ts`](../../scripts/worldgen-phase-baseline.ts) | current phase benchmark and report formatting |
| [`src/runtime/host/generated-world-host.ts`](../../src/runtime/host/generated-world-host.ts) | cooperative chunk-view scheduling and host tick/poll behavior |
| [`src/world/level/generated-render-level.ts`](../../src/world/level/generated-render-level.ts) | terrain/decor status advancement and chunk copy path |
| [`src/worldgen/levelgen/noise-based-chunk-generator.ts`](../../src/worldgen/levelgen/noise-based-chunk-generator.ts) | default terrain generation phases |
| [`src/worldgen/levelgen/demo-world-generators.ts`](../../src/worldgen/levelgen/demo-world-generators.ts) | `flat_grass` and `small_island` baseline generators |
| [`test/browser/d5-traversal.test.ts`](../../test/browser/d5-traversal.test.ts) | existing browser traversal and frame/report model |
| [`test/browser/d5-traversal-report.ts`](../../test/browser/d5-traversal-report.ts) | existing D5 report schema and gates |
| [`../authoritative-host-scheduling.md`](../authoritative-host-scheduling.md) | responsiveness requirements while chunk work is active |
| [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md) | vanilla status dependency constraints |

## Scope

### 1. Repair and widen `perf:worldgen`

Fix direct phase mode so it constructs `GeneratedRenderLevel` with the current constructor.

Add supported presets:

- `default`
- `browser_smoke`
- `flat_grass`
- `small_island`

Add phase toggles:

- `terrain-only`
- `terrain-surface-carvers`
- `features`
- `full-no-light`
- `snapshot-pack`
- `lighting`
- `all`

Report both total duration and max single cooperative quantum. A phase that takes 500 ms total but yields every 4-8 ms is a different problem from one 120 ms unbroken chunk copy.

### 2. Add host-only synthetic fly-by

Add a Node script, for example:

```text
scripts/worldgen-host-flyby.ts
pnpm perf:worldgen:flyby -- --preset default --radius 1 --steps 96 --speed-blocks-per-step 4
```

This script should exercise the real `GeneratedWorldHost` API, not call generator internals directly:

1. open a generated world
2. set an initial chunk view
3. wait for a publishable warmup ring
4. advance a scripted fly-by route across chunk boundaries
5. issue `set_chunk_view` as the route enters new chunks
6. send representative `set_player_input` commands if useful for player tick cadence
7. poll updates at a fixed cadence
8. stop when the route and final cooldown finish

This is a host scheduler benchmark, not a renderer benchmark. It should record whether the authoritative host stays responsive while chunks are generated and published.

Metrics:

- route duration and crossed chunk boundaries
- `set_chunk_view` ack p50/p95/p99/max
- `set_player_input` p50/p95/p99/max
- `poll_world_updates` p50/p95/p99/max
- authoritative `player_state.tick` gaps
- chunk snapshots published per poll
- generated terrain chunk count
- decorated chunk count
- full/published chunk count
- chunk unload count
- storage load/save/evict count and duration
- per-phase total duration
- per-phase max unbroken quantum
- pending message backlog max
- pending chunk job count by phase/status when available

### 3. Add generation instrumentation

Add low-overhead counters around host phases:

- storage preload
- `fillFromNoise`
- `buildSurfaceAndBedrock`
- air carvers
- liquid carvers
- chunk buffer to `LevelChunk` copy
- decoration dependency preparation
- biome decoration
- lighting request/wait/publish
- snapshot build and pack
- storage cache/dirty save
- publish queue insertion

Prefer structured counters returned through existing `world_perf` messages or a benchmark-only report object. Avoid logging inside hot loops.

Track status counts separately from chunk counts. For this repo, "625 authority chunks" and "25 published chunks" must not collapse into one vague number.

### 4. Add browser fly-by lane

When on a browser/WebGPU host, add or extend a Playwright performance test:

```text
pnpm perf:worldgen:browser-flyby
```

The browser lane should use the live debug/runtime path:

1. boot the world through the normal browser scene
2. wait for an initial settled frame
3. capture `/tmp/mclone-worldgen-flyby-start.png`
4. fly forward across several chunk boundaries
5. collect `requestAnimationFrame` gaps and long tasks
6. collect host/runtime/render-world/GPU counters
7. wait for final queue settle
8. capture `/tmp/mclone-worldgen-flyby-end.png`
9. write `/tmp/mclone-worldgen-flyby-report.json`

Run the browser lane for at least:

- `flat_grass`
- `small_island`
- `default`

If the test host has no display/browser path, the browser lane should skip with a clear host-capability reason rather than fail as a renderer regression.

### 5. Add phone/manual profile notes

The phone case is important enough to plan explicitly, but not to fake on a headless Linux host.

Document a manual mobile run procedure:

- run the dedicated host on the Mac or LAN server
- connect the phone browser to the remote host
- use the same seed/preset/route/view distance as the browser fly-by lane
- capture the generated JSON report from the client if available
- note device model, browser version, power mode, thermal state if obvious, and whether the page is remote-hosted or phone-hosted

Phone-hosted singleplayer and phone-client remote-host are different experiments. Keep them separate.

## Interpretation guide

Use this matrix before choosing implementation work:

| Observation | Likely next step |
|---|---|
| `flat_grass` has frame/poll stalls | investigate snapshot transfer, render-world ingest, mesh upload, or scheduling; not noise worldgen |
| `flat_grass` is smooth but `default terrain-only` stalls | inspect `fillFromNoise`, `NoiseSampler`, `BlendedNoise`, allocation in density fill |
| terrain is smooth but `features` stalls | split decoration units further, profile feature/decorator hot spots, check heightmap/block-read volume |
| status/job counters show duplicate same-status work, reruns after cancellation, or lost partial progress | prioritize Tactical 57/58 before low-level optimization |
| p95 is fine but max/p99 has a large single quantum | split the responsible phase or add a cooperative yield point |
| total work is high but quanta are bounded | consider prioritization, background prefetch, status orchestration, or a worker pool |
| one narrow numeric kernel dominates after status fixes | write a WASM spike for that module only, behind chunk-sized typed-array boundaries |
| browser frame gaps occur without host latency spikes | inspect render-world mesh/GPU upload/frame code, not worldgen |
| phone-hosted stalls but remote-host phone client is smooth | phone CPU worldgen is the issue; dedicated/integrated host selection may be enough |
| remote-host phone client still stalls | renderer/meshing/GPU upload/mobile browser path is the issue |

## WASM decision bar

Do not start Rust/C/WASM until all are true:

- the benchmark names the hot function or module
- the hot work remains hot after status-future coalescing and partial-state reuse work
- cooperative scheduling cannot solve the user-visible stall by itself
- the boundary can stay chunk-sized or section-sized
- the port does not obscure seed-parity testing against the Java oracle
- TS remains the reference implementation or has equivalent oracle coverage

Candidate WASM modules, only if measured:

- density/noise column fill
- block-buffer to packed-section conversion
- snapshot packing/bit storage
- a specific decoration helper with high call count and low object dependency

Non-candidates without much stronger evidence:

- the whole host
- the whole worldgen pipeline
- simulation authority
- renderer ownership

## Tests and validation

Headless required:

- `pnpm perf:worldgen -- --preset flat_grass --radius 1 --host-modes cooperative --direct-modes none`
- `pnpm perf:worldgen -- --preset small_island --radius 1 --host-modes cooperative --direct-modes none`
- `pnpm perf:worldgen -- --preset default --radius 1 --host-modes cooperative --direct-modes none`
- `pnpm perf:worldgen:flyby -- --preset flat_grass --radius 1`
- `pnpm perf:worldgen:flyby -- --preset default --radius 1`
- `pnpm typecheck`
- `git diff --check`

Browser required when host supports it:

- `pnpm host:check -- --probe-browser-webgpu`
- `pnpm perf:worldgen:browser-flyby -- --preset flat_grass`
- `pnpm perf:worldgen:browser-flyby -- --preset default`
- inspect the start/end screenshots saved under `/tmp`

Regression rule:

If a later worldgen, scheduling, storage, snapshot, render-world, or movement slice changes these numbers materially, update the report with before/after numbers instead of relying on subjective playtesting.

## Done when

- headless phase benchmarks run for `flat_grass`, `small_island`, and `default`
- host-only synthetic fly-by report exists and records responsiveness while crossing chunk boundaries
- generation counters identify terrain, decoration, status, and snapshot costs separately
- browser fly-by harness exists or is explicitly skipped by host-capability detection
- the tactical doc is updated with one measured result table and one recommended next action
- no WASM/native rewrite is started unless the report satisfies the WASM decision bar

## Measured result - 2026-04-27 headless host slice

Implemented:

- repaired `pnpm perf:worldgen` direct mode against the current `GeneratedRenderLevel` constructor
- added `flat_grass` and `small_island` preset support to `pnpm perf:worldgen`
- added worldgen phase/count counters to `world_perf`
- instrumented terrain, decoration, full-status, snapshot, storage, and publication phases
- added `pnpm perf:worldgen:flyby` for host-only synthetic chunk-boundary traversal

Host capability:

- `pnpm host:check` reports this host as headless Linux with no `DISPLAY` / `WAYLAND_DISPLAY` and no visible `/dev/dri`
- Chrome is installed, but Playwright Chrome/WebGPU lanes are expected unavailable here
- browser/mobile fly-by remains a follow-up on a real browser/GPU host

Radius-1 phase baseline, no lighting/liquid:

| Preset | Publish chunks | Total publish wait | Top measured phases |
|---|---:|---:|---|
| `flat_grass` | 25 | `234.8 ms` | terrain copy `134.0 ms`, snapshot pack/publish `30.6 ms`, decoration/no-op heightmaps `30.3 ms` |
| `small_island` | 25 | `413.2 ms` | terrain copy `136.6 ms`, terrain fill `127.0 ms`, snapshot pack/publish `64.4 ms` |
| `default` | 25 | `1742.2 ms` | terrain generation `971.9 ms`, decoration `667.9 ms`, snapshot pack/publish `62.2 ms` |

Important count signal:

- all three presets generated `121` terrain chunks and decorated `81` feature chunks to publish `25` chunks
- that confirms the normal-publication status closure is large; by itself it is expected for a 5x5 normal published square
- use future counters to separate expected closure from duplicate in-flight work, route-cancel reruns, or lost partial state

Vanilla dependency check for the `7x7` vs `9x9` question:

- vanilla sends normal chunk data from `ChunkMap.prepareTickingChunk(...)`, which waits for `getChunkRangeFuture(chunk, 1, FULL)` before calling `playerLoadedChunk(...)` ([`ChunkMap.java:599`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:601`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:610`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:1018`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java))
- `LIGHT` has parent `FEATURES` and range `1`, while `FULL` is after `LIGHT` through the status parent chain ([`ChunkStatus.java:135`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:137`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:138`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java), [`ChunkStatus.java:155`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java))
- therefore, if the published square has radius `P`, vanilla-shaped normal publication needs `FULL` through radius `P + 1` and `FEATURES` through radius `P + 2`
- current host radius helpers do exactly that: requested radius `1` becomes publish radius `2` (`5x5`), full radius `3` (`7x7`), and features radius `4` (`9x9`) ([`generated-world-host.ts:143`](../../src/runtime/host/generated-world-host.ts), [`generated-world-host.ts:147`](../../src/runtime/host/generated-world-host.ts), [`generated-world-host.ts:151`](../../src/runtime/host/generated-world-host.ts))
- a `7x7` feature set would be enough to make a `5x5` square decoration-stable if those `5x5` chunks are just terrain/snapshot/cache data and are not being treated as vanilla normal client publications; that would be a deliberate split between "normal published/ticking" chunks and "halo/cache snapshots"
- vanilla also inflates the configured server view distance by one internally, then tracks chunks against that internal distance ([`ChunkMap.java:682`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:683`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:823`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:826`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java), [`ChunkMap.java:923`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java)); the client radius packet still carries the configured value ([`PlayerList.java:787`](../../reference/minecraft-1.17.1/src/net/minecraft/server/players/PlayerList.java), [`PlayerList.java:789`](../../reference/minecraft-1.17.1/src/net/minecraft/server/players/PlayerList.java))

Current reuse/coalescing check after Tactical 57:

- completed chunks and status records are reused while they remain inside the generated level authority window: current-view targets request `FEATURES` and no-light `FULL`, then dependencies recurse through `getGeneratedChunkDependencyStatus(...)` instead of running separate view-level preload, terrain, and features batches ([`generated-world-host.ts`](../../src/runtime/host/generated-world-host.ts))
- chunks outside the new authority window are removed from the in-memory `StaticRenderLevel` and their generated status records are deleted ([`generated-render-level.ts`](../../src/world/level/generated-render-level.ts))
- generated chunks are cached to durable storage only when a publishable snapshot is built; partial terrain/features status is not persisted separately ([`generated-world-host.ts`](../../src/runtime/host/generated-world-host.ts))
- cooperative scheduling cancels by view revision at status-target boundaries, and per-chunk/status holder jobs coalesce repeated in-flight requests. Superseded work can still finish the current chunk/status quantum before observing cancellation, matching the host's cooperative scheduling granularity.

Host fly-by, radius 1, 32 steps, 8 crossed chunk boundaries, no lighting/liquid:

| Preset | Duration | Warmup | `set_chunk_view` p95/max | `poll_world_updates` p95/max | Player tick gap p95/max | Top measured phases during route |
|---|---:|---:|---:|---:|---:|---|
| `flat_grass` | `853.4 ms` | `226.6 ms` | `1.0 / 1.0 ms` | `0.2 / 0.7 ms` | `53.8 / 53.8 ms` | terrain copy `82.9 ms`, snapshot pack/publish `44.2 ms` |
| `default` | `2969.1 ms` | `1736.3 ms` | `1.0 / 1.0 ms` | `0.1 / 0.6 ms` | `56.8 / 56.8 ms` | terrain `712.1 ms`, decoration `526.7 ms`, feature placement `500.3 ms` |

Artifacts:

- `/tmp/mclone-worldgen-flyby-flat.json`
- `/tmp/mclone-worldgen-flyby-default.json`

Interpretation:

- Host command responsiveness is good in the headless synthetic lane; `set_chunk_view`, input, and poll calls stay low-latency while jobs run.
- Default worldgen cost is real, and the benchmark shows the substantial dependency closure of normal publication. The `9x9` feature count and `11x11` terrain count are vanilla-shaped for normal publication of all `5x5` snapshots, but they remain a performance-policy target if we explicitly split normal publication from cache/halo snapshots.
- Flat-world cost is not zero; chunk copy and snapshot packing are meaningful baseline costs even without noise or real decoration.
- The next implementation step should be Tactical 58's partial `ProtoChunk` state and persistence before a WASM rewrite. Rerun this fly-by after Tactical 58; if default still has large single quanta after that, inspect `features.apply_biome_decoration`, air carvers, and density fill as narrow candidates.
