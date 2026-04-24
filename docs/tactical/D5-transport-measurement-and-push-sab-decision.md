# D5 - Transport measurement and push/SAB decision

Standing after [`D4-browser-render-world-ownership.md`](D4-browser-render-world-ownership.md). `D4` moved live browser chunk ownership and mesh-neighborhood reads into the render-world worker, and it tightened screenshot readiness so browser validation waits for a settled visible render queue before capture.

`D5` is the acceptance gate for the runtime data/protocol/loading arc. It is not automatically "completion" and it is not a license to start push transport or `SharedArrayBuffer` work speculatively.

## Goal

Measure the actual browser traversal path now that the D4 ownership boundary is live, then make a documented decision:

- accept the current architecture if traversal is smooth enough and remaining spikes are bounded GPU upload/render bookkeeping
- write `D6-push-transport` if HTTP polling is the measured bottleneck
- write `D6-shared-buffer-pool` if worker transfer/copy cost is the measured bottleneck
- write `D6-render-world-subworkers` if CPU mesh fan-out is the measured bottleneck
- write a different D6 only if the trace clearly identifies a different bottleneck, such as host generation/storage

At the end of `D5`, we should have a repeatable measurement harness, a checked-in summary of the measured run, and a concrete next decision. Do not decide from subjective playtesting alone.

## Why this slice exists

Earlier D slices removed major architectural uncertainty:

- `D1` gave runtime code stable block-state ids and `BitStorage`
- `D2` introduced packed section and packed chunk snapshot codecs
- `D3` moved storage/protocol boundaries onto packed chunk facts
- `D4` moved browser render-world ownership and mesh inputs off the main thread

That means the remaining performance question is no longer "should the main thread own chunk data?" It should not. The question is which cost now dominates while the player crosses chunk boundaries:

- host storage or generation
- snapshot serialization and wire decode
- HTTP polling latency or request overhead
- worker message transfer/copy
- render-world ingest and dirty-section fan-out
- mesh build CPU time
- main-thread GPU buffer upload and render bookkeeping
- host scheduler coupling that blocks player input, authoritative ticks, or polling behind chunk work

`D5` exists to identify that cost with enough evidence that the next tactical is obvious.

## Durable architecture references

Use these docs as constraints:

| Doc | D5 constraint |
|---|---|
| [`../architecture.md`](../architecture.md) | authority stays in the host; renderer remains presentation-only |
| [`../runtime-data-model.md`](../runtime-data-model.md) | packed vanilla-shaped chunk facts remain the model; `SharedArrayBuffer` is only a carrier optimization |
| [`../protocol.md`](../protocol.md) | local and remote play use one logical protocol; polling is a transport detail, not the protocol model |
| [`../loading-persistence.md`](../loading-persistence.md) | host owns create/open/join, chunk loading, generation, saving, eviction, and aggregate interest |
| [`../authoritative-host-scheduling.md`](../authoritative-host-scheduling.md) | input, player ticks, polling, and chunk-interest acknowledgement must not block behind chunk load/generation/snapshot jobs |

## Architecture divergence review

Minecraft Java can rely on a shared JVM heap, client-thread chunk ownership, executor-backed chunk compilation, and GL upload constraints embedded in the renderer. The browser cannot:

- remote clients cross an HTTP boundary today
- browser singleplayer crosses a worker boundary
- workers cannot share ordinary JS object graphs
- WebGPU presentation and GPU buffer ownership remain on the main thread
- `SharedArrayBuffer` requires cross-origin isolation and explicit lifetime/synchronization policy

D5 should not add a new divergence by convenience. It should only measure existing browser divergences and decide whether a further carrier/runtime change is justified.

The D5 divergence scope is instrumentation and harnessing:

- add timing marks, counters, and trace capture around existing boundaries
- add a repeatable traversal script
- add a report format that explains the bottleneck
- do not change protocol semantics, authority, persistence policy, chunk facts, or renderer ownership

## Current TS context

| TS source | Current role |
|---|---|
| `src/runtime/protocol/world-messages.ts` | logical host/client commands and updates |
| `src/runtime/protocol/world-http-protocol.ts` | remote HTTP envelopes and packed wire codec |
| `src/runtime/transport/remote-world-transport.ts` | browser remote HTTP transport and reconnect/resync path |
| `src/runtime/transport/worker-world-transport.ts` | browser local worker transport using structured clone and transferables |
| `src/runtime/transport/local-world-transport.ts` | `TransportWorldClient` applies host messages and forwards chunk updates to the render-world sink |
| `src/runtime/node/generated-world-http-server.ts` | dedicated Node host HTTP service and shared authoritative-world/session runtime |
| `src/runtime/host/generated-world-host.ts` | authoritative generated-world host, chunk view handling, storage/generation boundary |
| `src/renderer/chunk/render-world-worker-client.ts` | render-world worker client, ingest and mesh request counters |
| `src/renderer/chunk/render-world-worker.ts` | worker-owned client cache, packed ingest, dirty-section fan-out, mesh build |
| `src/renderer/chunk/chunk-render-dispatcher.ts` | main-thread chunk render scheduling, render-world mesh requests, GPU uploads |
| `src/renderer/scene-setup.ts` | live scene wiring, settled-frame helpers, render-world counters, render queue stats |
| `src/renderer/debug/debug-free-cam.ts` | authoritative browser control/debug page and current scripted input surface |
| `test/browser/debug-free-cam.test.ts` | current remote movement smoke with render-world counters |
| `test/browser/smoke.test.ts` | remote two-client browser boot smoke |

## Scope

| # | Module | Expected result |
|---|---|---|
| 1 | Measurement model | a typed report shape captures traversal config, browser/adapter info, frame metrics, phase timings, counters, artifacts, and decision |
| 2 | Browser frame metrics | `requestAnimationFrame` gaps, long tasks, p50/p95/p99/max frame gap, dropped-frame-like gaps, and traversal duration are recorded |
| 3 | Host/runtime timings | open/session, chunk-view update, storage load, generation, snapshot encode, player tick cadence, input/poll latency, and HTTP response timings are recorded or approximated |
| 4 | Transport timings | remote request duration, response bytes, decode time, reconnect count, poll cadence, and update batch sizes are recorded |
| 5 | Render-world timings | chunk ingest duration, dirty-section count, mesh request queue wait/build duration, not-ready count, mesh bytes, and mesh completion count are recorded |
| 6 | Main-thread render timings | visible queue stats, GPU upload count/bytes/time, frame encode/submit/wait time, and request-to-visible latency are recorded |
| 7 | Traversal harness | a Playwright-driven route crosses multiple chunk boundaries at fixed seed/preset/view distance and saves trace/report/screenshots under `/tmp` |
| 8 | Decision document | D5 doc is updated with the measured result and one explicit next action: complete arc or write the correct D6 |

## Explicit non-goals

- no WebSocket/WebTransport/WebRTC implementation
- no `SharedArrayBuffer` implementation
- no render-world subworker pool
- no new protocol semantics
- no host authority or persistence redesign
- no client-side prediction or client-authoritative chunk state
- no chunk data model changes
- no renderer culling overhaul
- no gameplay movement parity work
- no benchmark-only rendering shortcuts that bypass the live path

D5 may add low-overhead instrumentation and harness-only helpers. It should not optimize first and measure later.

## Measurement scenario

Use one primary traversal before adding variants.

Primary config:

| Field | Value |
|---|---|
| seed | `12345` |
| preset | `browser_smoke` |
| page | `/debug.html` |
| transport | `remote` dedicated Node host |
| host | per-test `GeneratedWorldHttpServer` on random localhost port |
| view distance | `6` |
| render distance | `192` |
| fog color | `8fb8ff` |
| start position | `965.5,168,3189.5` |
| start yaw/pitch | `225,55` |
| camera mode | follow authoritative `player_state` during traversal |
| warmup | wait for expected chunk ring and settled render queue |
| route | hold forward input on the fixed yaw long enough to cross at least 4 chunk boundaries |
| duration | target 30 seconds after warmup; extend only if fewer than 4 chunk-view changes occur |
| cooldown | release input, poll/render until queue settles, capture end screenshot |

Artifacts:

- `/tmp/mclone-d5-start.png`
- `/tmp/mclone-d5-end.png`
- `/tmp/mclone-d5-traversal-report.json`
- `/tmp/mclone-d5-traversal-trace.json` or the closest Chrome trace artifact the harness can reliably produce

The harness should assert:

- traversal starts from a settled frame
- traversal crosses at least 4 chunk boundaries
- loaded chunk count returns to the expected ring count after each chunk-view change or after the final cooldown
- render-world ingest, mesh build, mesh completion, and GPU upload counters increase during traversal
- final frame is settled before the end screenshot
- no browser page errors or WebGPU validation errors occur

## Metrics to record

### Environment

- commit hash
- OS and architecture
- Chrome version and WebGPU adapter info
- viewport and device pixel ratio
- view distance and render distance
- transport type and host mode
- whether cross-origin isolation is active
- whether `SharedArrayBuffer` is available, only as an environment fact

### Frame pacing

- total traversal duration
- frame count
- min/p50/p95/p99/max `requestAnimationFrame` gap
- count of gaps greater than 33.4 ms
- count of gaps greater than 50 ms
- count of gaps greater than 100 ms
- long task count and total long task time from `PerformanceObserver`
- longest long task, with nearest phase marker if available

### Host and loading

- `open_world` duration
- `set_chunk_view` request duration
- aggregate-interest computation time
- storage load count/time
- generation count/time
- snapshot serialization/encode time
- chunks returned per chunk-view response
- response byte size by route
- save/eviction work observed during traversal, if any
- authoritative `player_state.tick` gaps while chunk jobs are active
- `set_player_input` and `poll_world_updates` latency while chunk jobs are active
- `set_chunk_view` acknowledgement latency, tracked separately from chunk snapshot completion

If exact internal timing is awkward at first, add coarse marks at the host service boundary and refine only where the first trace points.

### Transport

- request count by command: `open_world`, `set_chunk_view`, `set_player_input`, `poll_world_updates`
- request duration p50/p95/p99/max by command
- response byte count by command
- decode/deserialization time by command
- poll interval observed, including missed/delayed polls
- reconnect/resync count
- time from authoritative `player_state` tick to client observation when available
- gaps in authoritative `player_state.tick` delivery during chunk-view changes

### Render-world worker

- ingest batch count
- chunks per ingest batch
- packed bytes transferred into the worker
- ingest duration p50/p95/p99/max
- dirty sections emitted per batch
- mesh build request count
- mesh not-ready response count
- mesh queue wait p50/p95/p99/max
- mesh build duration p50/p95/p99/max
- mesh bytes returned to main thread
- mesh completion count

### Main thread rendering

- visible rendered chunk count over time
- pending visible chunk compile count over time
- queued and active chunk build count over time
- GPU upload count
- uploaded bytes by render layer
- GPU upload duration p50/p95/p99/max
- frame encode duration
- queue submit to `onSubmittedWorkDone()` duration for measured frames
- request-to-visible latency for new chunks when possible:
  - chunk snapshot received
  - render-world ingest done
  - dirty section marked
  - mesh request started
  - mesh response received
  - GPU upload done
  - first settled frame containing the section submitted

## Report shape

The report should be machine-readable JSON and easy to summarize in docs.

Suggested top-level shape:

```ts
interface D5TraversalReport {
  readonly schemaVersion: 2;
  readonly commit: string;
  readonly config: D5TraversalConfig;
  readonly environment: D5Environment;
  readonly artifacts: {
    readonly startScreenshot: string;
    readonly endScreenshot: string;
    readonly trace?: string;
  };
  readonly traversal: {
    readonly durationMs: number;
    readonly chunkViewChanges: number;
    readonly startChunk: readonly [number, number];
    readonly endChunk: readonly [number, number];
  };
  readonly framePacing: D5FramePacingSummary;
  readonly hostResponsiveness: D5HostResponsivenessSummary;
  readonly transport: D5TransportSummary;
  readonly renderWorld: D5RenderWorldSummary;
  readonly mainThread: D5MainThreadSummary;
  readonly decision: D5Decision;
}
```

Do not commit raw traces. Commit only code, tests, docs, and a concise measured summary. Raw traces and screenshots stay under `/tmp`.

## Initial acceptance thresholds

These thresholds are the first D5 bar for local Chrome on the development machine. If they need to change, change them before the first measured run and explain why in this doc. Do not loosen them after seeing an inconvenient result.

| Metric | Acceptable |
|---|---|
| traversal chunk-view changes | `>= 4` |
| p95 frame gap | `<= 33.4 ms` |
| p99 frame gap | `<= 50 ms` |
| max frame gap during warmed traversal | `<= 100 ms` |
| long tasks over 50 ms during warmed traversal | `0`, or each explained and not correlated with chunk arrival |
| final render queue | pending visible, queued, and active build counts are all `0` |
| visual result | start/end screenshots show populated terrain, no obvious chunk holes caused by late async work |
| WebGPU validation | no validation errors |
| main-thread chunk ownership | no raw chunk snapshot decode or mesh-neighborhood gathering on the live browser path |

If the run misses a threshold, D5 can still complete, but only by identifying the bottleneck and writing the correct D6 tactical. A failed threshold with no bottleneck is not a completed D5.

## Decision rules

### Arc complete

Choose this only if:

- frame pacing meets the thresholds
- long tasks are absent or unrelated to chunk arrival
- request-to-visible latency is acceptable for traversal
- remaining spikes are bounded GPU upload/render bookkeeping
- no trace evidence points to polling, transfer/copy, or mesh fan-out as a meaningful bottleneck

Record the measured numbers in this doc and mark the runtime data/protocol/loading arc accepted for now.

### Write `D6-push-transport`

Choose this if:

- chunk/player updates spend material time waiting for the next poll
- request overhead or polling cadence dominates request-to-visible latency
- player-state delivery is visibly or measurably stale even when host generation/mesh work is not backed up
- the trace shows idle gaps between host availability and client receipt

`D6-push-transport` should reuse the same logical messages. It should not change chunk ownership or authority.

### Write `D6-shared-buffer-pool`

Choose this if:

- structured clone, transfer, decode, or buffer allocation/copy cost remains material after D4
- packed chunk or mesh payload movement dominates worker/main time
- GC pressure correlates with chunk payload transfer
- the same payload bytes are copied repeatedly across boundaries

The D6 must include cross-origin isolation headers, feature detection, buffer lifetime rules, fallback paths, and tests that prove non-SAB mode still works.

### Write `D6-render-world-subworkers`

Choose this if:

- render-world mesh build CPU time dominates traversal
- the render-world worker has a sustained mesh backlog
- main thread is mostly idle or upload-bound while the render-world worker is saturated
- not-ready responses are low enough that missing neighbors are not the issue

The D6 should keep authority out of the renderer and split only render-world CPU mesh work.

### Write a different D6

Choose this if the trace clearly points elsewhere:

- player/session work is blocked behind chunk jobs: write `D6-authoritative-host-scheduler`
- host generation dominates: write a host generation/loading scheduling tactical
- storage dominates: write a storage/cache tactical
- GPU upload dominates beyond thresholds: write a GPU upload/render bookkeeping tactical
- visibility/culling dominates: write a LevelRenderer scheduling tactical

Name the bottleneck directly. Do not force every result into push/SAB/subworkers.

## First slice landed - 2026-04-24

The first D5 slice adds the repeatable traversal harness and report schema, but it does **not** make the D5 architecture decision yet.

What landed:

- typed report data in `test/browser/d5-traversal-report.ts`
- targeted Playwright harness in `test/browser/d5-traversal.test.ts`
- `pnpm perf:d5` entrypoint, skipped from default `pnpm test:browser`
- `/tmp` artifacts:
  - `/tmp/mclone-d5-start.png`
  - `/tmp/mclone-d5-end.png`
  - `/tmp/mclone-d5-traversal-report.json`
  - `/tmp/mclone-d5-traversal-trace.json`
- browser `requestAnimationFrame` gap collection during the warmed traversal window
- browser long-task collection where Chrome exposes `PerformanceObserver` `longtask`
- existing render-world counters and render queue stats sampled through the debug page
- coarse remote HTTP request duration/byte summaries from Playwright network events
- Chrome-trace-compatible JSON containing traversal, marker, long-task, HTTP, and chunk-view events

Current harness note: the debug flight controls apply pitch to forward movement. The first harness holds `Space` with `KeyW` at yaw/pitch `225,55` so the camera follows authoritative `player_state` without diving below the world during the 30-second run. This is a harness-only measurement constraint, not an optimization or protocol change.

Single validation run from the landing slice on this machine:

| Metric | Observed |
|---|---:|
| traversal duration | `30226 ms` |
| chunk-view changes | `4` (`[60,199] -> [64,196]`) |
| frame gap p95 / p99 / max | `10.1 ms` / `11.5 ms` / `18.5 ms` |
| frame gaps > 33.4 / 50 / 100 ms | `0` / `0` / `0` |
| long tasks | `0` |
| final loaded chunks | `225` |
| final render queue | pending visible `0`, queued `0`, active `0` |
| render-world delta | ingest batches `84`, mesh builds `1450`, mesh completions `1450`, GPU uploads `2488` |

This run proves the harness and report path, but it is only one run and only uses existing counters plus coarse HTTP timings. The next D5 slice should add the internal phase timers called out below, then run the primary traversal at least three times before selecting `arc complete` or the correct D6.

## D6 scheduler measurement update - 2026-04-24

The D6 remote scheduler slice upgraded the D5 report to schema `2` and added client-observed host responsiveness fields:

- `set_chunk_view` acknowledgement latency during the warmed traversal window
- `set_player_input` latency during the warmed traversal window
- `poll_world_updates` latency during the warmed traversal window
- `poll_world_updates` latency when responses contain chunk snapshots
- chunk snapshots per poll response
- `player_state.tick` response gaps and sampled tick gaps
- response message counts and chunk/player message counts by command

The timing source is now a browser-side `fetch` wrapper installed by the harness. Playwright-side network events were too easily distorted because the test runner and per-test Node host share one process.

Single validation run after remote scheduler migration:

| Metric | Observed |
|---|---:|
| traversal duration | `30043 ms` |
| chunk-view changes | `10` (`[60,199] -> [65,194]`) |
| frame gap p95 / p99 / max | `10.3 ms` / `10.4 ms` / `10.7 ms` |
| frame gaps > 33.4 / 50 / 100 ms | `0` / `0` / `0` |
| long tasks | `0` |
| `set_chunk_view` ack p50 / p95 / max during traversal | `1.8 ms` / `10.0 ms` / `10.0 ms` |
| `set_player_input` latency during traversal | `1.1 ms` |
| `poll_world_updates` p50 / p95 / p99 / max during traversal | `0.9 ms` / `11.2 ms` / `256.9 ms` / `291.3 ms` |
| `poll_world_updates` with chunk snapshots p50 / p95 / max | `7.4 ms` / `256.9 ms` / `274.2 ms` |
| chunk snapshots per poll p50 / p95 / max | `2` / `3` / `3` |
| player tick response gap p50 / p95 / p99 / max | `56.4 ms` / `69.1 ms` / `309.2 ms` / `340.5 ms` |
| final loaded chunks | `225` |
| final render queue | pending visible `0`, queued `0`, active `0` |
| render-world delta | ingest batches `86`, mesh builds `2486`, mesh completions `975`, GPU uploads `1311` |

Artifact paths:

- `/tmp/mclone-d5-start.png`
- `/tmp/mclone-d5-end.png`
- `/tmp/mclone-d5-traversal-report.json`
- `/tmp/mclone-d5-traversal-trace.json`

Visual inspection: the start screenshot shows a populated stone/dirt cliff face; the end screenshot shows settled terrain across savanna/desert/mountain chunks with no obvious holes.

Interpretation: the D6 remote service migration removed chunk snapshots from `set_chunk_view` responses and made chunk-view acknowledgement cheap. Main-thread frame pacing remains clean. The next bottleneck is the host chunk-generation/snapshot quantum visible in chunk-bearing poll responses and p99 player tick delivery, not push transport, `SharedArrayBuffer`, or render-world subworkers.

## D6 poll payload split update - 2026-04-24

The next D6 slice capped streamed poll payloads and prioritized control/state messages ahead of bulk chunk snapshots when a capped response is drained. Remote browser polls cap at two messages; browser-worker scene setup caps at four messages to avoid regressing the parity-test boot path. Cooperative chunk jobs now build one packed snapshot and reuse it for both persistence and streaming.

Single validation run after the poll payload split:

| Metric | Observed |
|---|---:|
| traversal duration | `30407 ms` |
| chunk-view changes | `11` (`[60,199] -> [66,194]`) |
| frame gap p95 / p99 / max | `10.3 ms` / `10.4 ms` / `12.4 ms` |
| frame gaps > 33.4 / 50 / 100 ms | `0` / `0` / `0` |
| long tasks | `0` |
| `set_chunk_view` ack p50 / p95 / max during traversal | `0.9 ms` / `1.2 ms` / `1.2 ms` |
| `set_player_input` latency during traversal | `1.0 ms` |
| `poll_world_updates` p50 / p95 / p99 / max during traversal | `0.9 ms` / `7.5 ms` / `238.7 ms` / `254.9 ms` |
| `poll_world_updates` with chunk snapshots p50 / p95 / max | `1.2 ms` / `19.9 ms` / `246.1 ms` |
| chunk snapshots per poll p50 / p95 / max | `1` / `1` / `2` |
| player tick response gap p50 / p95 / p99 / max | `56.3 ms` / `60.0 ms` / `292.6 ms` / `304.0 ms` |
| final loaded chunks | `225` |
| final render queue | pending visible `0`, queued `0`, active `0` |
| render-world delta | ingest batches `162`, mesh builds `10451`, mesh completions `1091`, GPU uploads `1454` |

Artifact paths:

- `/tmp/mclone-d5-start.png`
- `/tmp/mclone-d5-end.png`
- `/tmp/mclone-d5-traversal-report.json`
- `/tmp/mclone-d5-traversal-trace.json`

Visual inspection: the start screenshot shows a filled cliff/cave face, and the end screenshot shows settled savanna/desert/mountain terrain with no blank chunks.

Interpretation: delivery-side splitting worked: chunk-bearing poll p50/p95 dropped sharply and capped polls now carry at most two snapshots. The remaining p99 spikes still occur with one chunk or even a player-only response, which means D6 needs to split or offload the synchronous per-chunk generation/decor/snapshot work inside the host rather than changing transport shape.

## D6 chunk status phase split update - 2026-04-24

The next D6 slice split cooperative host chunk work into terrain/load, decoration, and snapshot publication phases. It also added cooperative decoration yields inside decorated feature placement loops and suppressed automatic synchronous decoration while cooperative decoration is active, preventing feature placement reads from recursively generating/decorating neighboring chunks in one large host quantum.

Single validation run after the chunk status phase split:

| Metric | Observed |
|---|---:|
| traversal duration | `30136 ms` |
| chunk-view changes | `12` (`[60,199] -> [66,193]`) |
| frame gap p95 / p99 / max | `9.3 ms` / `9.4 ms` / `9.4 ms` |
| frame gaps > 33.4 / 50 / 100 ms | `0` / `0` / `0` |
| long tasks | `0` |
| `set_chunk_view` ack p50 / p95 / max during traversal | `1.3 ms` / `11.6 ms` / `11.6 ms` |
| `set_player_input` p50 / p95 / p99 / max during traversal | `1.0 ms` / `1.0 ms` / `1.0 ms` / `1.0 ms` |
| `poll_world_updates` p50 / p95 / p99 / max during traversal | `0.9 ms` / `10.6 ms` / `19.8 ms` / `26.4 ms` |
| `poll_world_updates` with chunk snapshots p50 / p95 / p99 / max | `1.3 ms` / `17.2 ms` / `23.8 ms` / `26.4 ms` |
| chunk snapshots per poll p50 / p95 / max | `1` / `1` / `2` |
| player tick response gap p50 / p95 / p99 / max | `50.1 ms` / `61.4 ms` / `100.0 ms` / `119.3 ms` |
| sampled tick gap p50 / p95 / p99 / max | `253.5 ms` / `382.9 ms` / `425.1 ms` / `441.7 ms` |
| final loaded chunks | `225` |
| final render queue | pending visible `0`, queued `0`, active `0` |
| render-world delta | ingest batches `180`, mesh builds `13119`, mesh completions `1273`, GPU uploads `1678` |

Artifact paths:

- `/tmp/mclone-d5-start.png`
- `/tmp/mclone-d5-end.png`
- `/tmp/mclone-d5-traversal-report.json`
- `/tmp/mclone-d5-traversal-trace.json`

Visual inspection: the start screenshot shows a filled cliff/cave face, and the end screenshot shows populated savanna/desert/mountain terrain with no blank chunks.

Interpretation: the measured bottleneck was host chunk scheduling, not push transport, `SharedArrayBuffer`, or render-world subworkers. The phase split brought traversal `poll_world_updates` p99 from the prior `238.7 ms` to `19.8 ms` and chunk-bearing poll p99 to `23.8 ms`. D6 should now be judged with manual low-view walking plus the remaining response max/player tick max, not by starting a transport or renderer topology change.

## Implementation sequence

1. Add a D5 measurement model.

   Add typed report/counter records near renderer/debug or test utilities. Keep them independent of business logic so they can be removed or changed without affecting protocol semantics.

2. Add browser frame and long-task collection.

   The debug page or harness should collect `requestAnimationFrame` timestamps during the warmed traversal and use `PerformanceObserver` for long tasks where supported.

3. Add phase markers around host/client boundaries.

   Start coarse:

   - remote request start/end/decode
   - host route start/end
   - host chunk-view work
   - render-world ingest start/end
   - mesh build request/response
   - GPU upload and frame submit

   Refine only after the first trace shows a coarse bucket is too large.

4. Add byte counts.

   Count response body bytes, packed chunk bytes, mesh payload bytes, and GPU upload bytes. Exact layer-by-layer accounting is useful, but coarse payload totals are enough for the first decision.

5. Add a Playwright D5 traversal harness.

   Prefer an explicit `pnpm perf:d5` or similarly named command over putting the full performance run into default `pnpm test:browser`. The harness should still use the existing browser test infrastructure and save artifacts under `/tmp`.

6. Capture Chrome trace data.

   Use Chrome DevTools Protocol tracing from Playwright if reliable. Include user timing marks and enough timeline categories to correlate JS tasks, network, workers, and WebGPU-adjacent main-thread work. If full trace capture is flaky, keep the JSON report as the authoritative artifact and document the fallback.

7. Assert the traversal is valid.

   The harness should fail if it does not cross enough chunks, if the final queue does not settle, if page errors occur, or if screenshots are blank/partial.

8. Run baseline measurements.

   Run the primary traversal at least three times on the same machine and report median plus worst observed values for the key frame/latency metrics. One run is not enough to decide a transport architecture.

9. Update this doc with results and decision.

   Add a "Measured result" section with concise numbers, artifact paths, and the selected next action. If a D6 is needed, name it and state why.

10. Keep the existing correctness gates green.

   D5 instrumentation must not break:

   - `pnpm typecheck`
   - `pnpm test`
   - `pnpm test:browser`

## Validation

Required before D5 can be called done:

- `pnpm typecheck`
- `pnpm test`
- `pnpm test:browser`
- D5 traversal harness run, with report and screenshots in `/tmp`
- start and end screenshots inspected manually
- trace or report reviewed enough to identify the bottleneck
- this doc updated with measured results and decision

The browser screenshot rule from `AGENTS.md` still applies. Save D5 screenshots to `/tmp`, inspect them, and describe what is visible before moving on.

## Done when

- D5 traversal harness exists and is repeatable
- traversal crosses at least 4 chunk boundaries under fixed config
- frame pacing, long-task, queue, transport, render-world, mesh, and GPU upload metrics are recorded
- report artifacts are written under `/tmp`
- start/end screenshots show populated terrain after settled-frame waits
- main thread still does not own raw chunk sections or mesh-neighborhood snapshots
- measurements either meet the acceptance thresholds or identify the next bottleneck
- this doc records the measured result and explicit decision
- if thresholds pass, the runtime data/protocol/loading arc is accepted for now
- if thresholds fail, the correct D6 tactical is written or queued as the next step
- `pnpm typecheck`, `pnpm test`, and `pnpm test:browser` pass

## Next

Use the D5 harness as the regression gate for the D6 scheduler work. Next, manually validate low-view walking on `debug.html?viewDistance=1`; if the remaining host max is still visible, continue D6 inside the host scheduler with snapshot/persistence or feature-placement job work. Do not start push transport, `SharedArrayBuffer`, or render-world subworkers unless a later D5 report selects that path.
