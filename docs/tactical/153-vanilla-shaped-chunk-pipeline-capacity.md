# 153: Vanilla-Shaped Chunk Pipeline Capacity

Status: Slice 3a publication max-units derivation landed after Slice 2 sparse
changed-block filtering, the vanilla comparison scout, and the publication-age
source split. The retained light path now enqueues replacement rechecks only
when current opacity/emission facts change, cutting clean RD10/RD15
changed-block recheck time by about `90%` and light compute by `54-56%` while
the lighting fixtures remain byte-identical. The cost-derived publication cap
then moved clean fresh startup to RD10/RD15 full view
`5106.574ms`/`9726.067ms` and target quiescence
`5598.029ms`/`10498.777ms`. Publication is no longer the visible limiter:
completed-publication ages fell to about `0.8-1.0s`, while the next pressure is
the serial light-status worker mailbox (`288` RD10 / `550` RD15 max pending
statuses).
This tactical absorbs 150's "per-stage render pipeline budgeting"
follow-up list and widens it to the real goal: raise the
end-to-end local-integrated chunk pipeline ceiling so desktop actually uses
its cores, by porting the vanilla 1.17.1 scheduling/capacity shape instead
of inventing constants.

Law doc: [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md).
Strategy lineage:
[`142-throughput-policy-with-quest-rd5-guardrail.md`](142-throughput-policy-with-quest-rd5-guardrail.md)
(gates, falsified levers),
[`150`](150-adaptive-frame-budget-controller.md) (controller, floors,
promoted queue-depth baseline),
[`062-shared-threading-topology.md`](062-shared-threading-topology.md)
(worker topology parent).

Workstream: native Rust shared performance/architecture.

Goal: after 150, the publication valve is measured-elapsed and the pipeline
runs at the structural ceiling of its single-threaded stages
(~`48-58` chunks/sec fresh at RD10/RD15/RD20 — a rate signature, not a
pacing one). Worldgen itself is not the bottleneck (raw `551-1243`
chunks/sec on one thread); the post-generation pipeline is: light
(~`60`/sec ceiling), CPU meshing (~`50-90`/sec at workers `1`), then the
publication unit cap (`80`/sec at 20 Hz). This tactical raises those stage
ceilings the way vanilla does — resource-derived capacity, serial actors
where vanilla is serial, parallel stages where vanilla is parallel, bounded
admission everywhere — while the Quest RD5/RD7/churn gate set stays green
and **no capacity constant is tuned to a specific host**. Desktop and Quest
resolve to different capacity numbers because their queried resources and
measured inputs differ, never because a constant was picked on one machine.

## Why Vanilla Shape, Stated Once

Vanilla 1.17.1 encodes hard-won answers to exactly the problems this
tactical touches:

1. **Capacity comes from resource formulas, not tuned counts.** The shared
   worker pool is `clamp(cores - 1, 1, 7)`; the mesh buffer pool is
   `max(1, min(is64bit ? cores : min(cores, 4), heapBudget/bufferSize))`.
   Both scale up on big hosts and degrade on small ones with no per-host
   profile.
2. **Serial where correctness demands, parallel where it is free.**
   Feature placement and lighting are single-consumer actors (serialization
   *is* the cross-chunk race-safety mechanism); noise fill and mesh
   compilation fork wide onto the pool.
3. **Bounded admission at every stage boundary** — buffer packs, light
   `taskPerBatch`, the compile frame deadline, server tick slack — so no
   stage can flood a frame regardless of how much capacity exists.
4. **One shared pool** so idle stages donate cores to busy ones instead of
   pinning one thread per stage.
5. **Priority by player relevance** (ticket-level sorter, distance-sorted
   compile queue) so capacity is spent near the player first.

150 already ported (3) as the budget controller. This tactical ports (1)
and (2), re-measures the promoted valves against the higher ceilings,
and makes an explicit evidence-gated decision on (4). (5) exists in the
compile queue (tactical 034) and is not re-opened.

## Sequencing

- Tactical 150 is closed; its re-pinned gate set and close-out baselines
  are the falsification anchors here.
- Coordinate with [`151-remote-inbound-update-pipeline.md`](151-remote-inbound-update-pipeline.md):
  151 owns remote/dedicated lanes and host-mode ingress. This tactical is
  local-integrated capacity only; remote sessions stay on fail-safe floors
  and their budget shaping remains 151-or-later business.
- [`152`](152-xr-room-scale-body-follow-hysteresis.md) is orthogonal.
- One slice per session, in order. Between slices the user reviews.
- Slice 0 is measurement-only (144 invariant 1 applies). Slices 1-4 are
  candidates: land behind flags/derived-default plumbing, promote through
  the gate loop, one policy family per measured comparison.

## Fixed Reference Numbers

All from the 150 close-out on clean commits `42d3e43a`/`c1ac218b` and the
142 ceiling split (Mac host `kmacbook`, Quest 3, seed `12345` unless
stated). If Slice 0 recapture disagrees materially, re-pin in Slice 0.

| Anchor | Value |
|---|---|
| Desktop RD10 fresh frozen-fluids, defaults (`workers=1`, `max-pending=4`) | Slice 0 recapture: full view `9699.848ms`, target quiescent `11186.276ms`, `0 / 0` over/over-2x |
| Desktop RD15 fresh frozen | Slice 0 recapture: full view `19930.189ms`, target quiescent `21998.543ms`, `0 / 0` over/over-2x |
| Desktop RD10 persisted frozen | Slice 0 recapture: full view `1022.791ms`, target quiescent `4940.262ms`, post-full-view target rebuilds `5696` |
| Contrasting seed `74739` RD10 frozen | full view `8157.360ms`, quiescent `9601.381ms` |
| RD20 long live (120 Hz backlog watch) | full view `34864.820ms`; max pending publication `208`, pump stalls `0` |
| Raw / server-only / server+light ceilings (142) | `551.5` (`1243.2` warm) / `126.7` / `40.6` chunks/sec |
| Implied startup stage costs | desktop fresh worldgen `1.56-1.70ms`/chunk, light `7.44-9.38ms`/status, render compile `0.181-0.188ms`/section; RD10 persisted render compile `0.217ms`/section; Quest churn render compile `1.173ms`/section |
| Effective fresh stage rates | RD10: feature `113.7`/s, light `105.0`/s, compile `639.4` sections/s, upload `195.2` sections/s; RD15: feature `130.9`/s, light `133.2`/s, compile `705.5` sections/s, upload `221.2` sections/s |
| Quest RD5 orbit (workers `2`, caps `2/16/64`) | skipped `0`, dropped delta `15`, app p95 `12.040ms`, headroom avg `+3.172ms`, over-period `0.0%` |
| Quest RD7 orbit (shipping defaults) | skipped `0`, dropped delta `16`, app p95 `12.755ms`, over-period `0.9%` |
| Quest RD5 churn (workers `2`, caps `2/16/64`) | skipped `0`, dropped delta `16`, app p95 `8.919ms`, over-period `0.0%`, compile queue age `581.685ms`, render compile busy `24722.366ms` |
| Quest churn publication cadence | `23.419` feature chunks/sec, `11.509` light statuses/sec, `34.928` units/sec |
| Movement-frame caveat (open from 150) | two clean repeats each with `1` over-budget / `1` over-2x legacy frame (`~18ms` max), unattributed |
| Quest 15-min churn soak (open from 150) | app-work safe but not clean: submitted FPS `41.79`, dropped deltas `219 -> 358`, compile queue age `1558.614ms`, battery `35.0C -> 43.0C` |

## Java Reference Anchors (1.17.1)

Verified against `reference/minecraft-1.17.1/src/` on 2026-07-07. Read the
listed source before implementing the consuming slice; drift from the
reference is a decision, not an accident. 150's anchors (compile deadline,
`compileChunksUntil` per-unit break, tick-slack `haveTime()`,
`processUnloads` floors/backlog, 3-tick task age, `FrameTimer`) carry over
unchanged and are not repeated.

| Mechanism | Reference | Shape |
|---|---|---|
| Shared worker pool sizing | `Util.java` `makeExecutor` (105-130), `backgroundExecutor` (136) | one ForkJoinPool `Mth.clamp(availableProcessors() - 1, 1, 7)`, threads `Worker-Main-N`; client and server share it in singleplayer |
| Serial actor primitive | `util/thread/ProcessorMailbox.java` (38-95, 112-124) | `SCHEDULED_BIT` CAS allows one in-flight `run()`; exactly one task per activation, then re-registers on the pool — serial semantics, fair interleave, no pinned thread |
| Chunk pipeline orchestration | `server/level/ChunkMap.java` (113-115, 150-158, 450-470, 512-543) | `worldgen`/`main`/`light` mailboxes wrapped by `ChunkTaskPriorityQueueSorter` (ticket-level priority, `Integer.MAX_VALUE` backlog); `scheduleChunkGeneration` assembles the neighbor-range future then runs `ChunkStatus.generate` **on the worldgen mailbox** |
| Serial generation bodies | `world/level/chunk/ChunkStatus.java` (39-163) | structure starts/references, biomes, surface, carvers, liquid carvers, **features**, spawn all run inline in the worldgen-mailbox task — globally serialized; FEATURES writes into neighbors via `WorldGenRegion` radius `1`, and the serialization is the race-safety mechanism |
| The one parallel gen stage | `world/level/levelgen/NoiseBasedChunkGenerator.java` `fillFromNoise` (317-349) | ignores the mailbox executor; `supplyAsync(doFill, Util.backgroundExecutor())` with per-section `acquire()/release()` — noise fills run concurrently across chunks |
| Dependency pyramid | `ChunkStatus.java` `STATUS_BY_RANGE` (164-176), `ChunkMap.getDependencyStatus` (555-564) | FULL needs r1 FEATURES, r2 LIQUID_CARVERS, r3-10 STRUCTURE_STARTS — the generation frontier is wide and status-staggered |
| Serial light actor | `server/level/ThreadedLevelLightEngine.java` (32, 117-196) | single mailbox; `scheduled` CAS; per activation: up to `taskPerBatch = 5` PRE_UPDATE chunk tasks (section statuses, enable sources, seed emissions), then `runUpdates(Integer.MAX_VALUE, true, true)` drains the whole propagation graph, then POST_UPDATE completions |
| Mesh capacity formula + admission | `client/renderer/chunk/ChunkRenderDispatcher.java` (71-103, 109-136) | pack count = `max(1, min(is64bit ? cores : min(cores, 4), (maxMemory*0.3)/(sum(bufferSizes)*4) - 1))`; serial admission mailbox pairs one queued task with one free `ChunkBufferBuilderPack` and **forks `doTask` onto the shared pool**; pack returns via mailbox and re-arms admission; `pC/pU/aB` counters (138-140) |
| Client meshing shares the pool | `client/renderer/LevelRenderer.java` (702-704) | dispatcher is constructed with `Util.backgroundExecutor()` — mesh compile contends with server worldgen/light on the same pool in singleplayer, bounded only by packs |
| Chunk NBT load | `ChunkMap.scheduleChunkLoad` (472-501) | runs on the server main-thread executor under tick slack in 1.17.1 — vanilla treats disk load as cheap; mclone's persisted path already beats this shape |

What vanilla does **not** have — the divergence ledger this tactical
maintains:

- **No parallel lighting.** The light engine is a serial actor. Any mclone
  light parallelism (Slice 2 option B) is a recorded divergence and must
  prove output equality against the serial engine, not just gate-greenness.
- **No parallel feature placement.** A feature-generation worker pool is
  explicitly *not* vanilla-shaped and is out of scope; mclone's single
  worldgen actor already matches the reference and is ~10x faster than the
  downstream stages.
- **No publication valve.** Vanilla's client applies everything; pacing
  lives at compile admission and tick slack. mclone's budgeted publication
  and update pump are previously recorded divergences (133/150) and stay.
- **No adaptation.** Vanilla's formulas are static resource formulas; 150's
  controller adds measured adaptation on top. The layering rule below keeps
  those roles separate.
- **Period constants.** The `7`-thread cap and the `is64bit` branch are
  2016-era host assumptions. We adopt the *formula shape* (clamped,
  resource-derived) and choose clamp bounds by cross-host measurement in
  Slice 1, recording the justification — see contract rule C.

## Contract For Implementing Agents

Rules 1-10 of 150's contract apply verbatim (one shared calculation;
fail-safe floors = today's shipped defaults; sans-I/O policy math;
traceable decisions; diagnostics-detour rule; definition of progress;
pinned absolute gates; A/B discipline; no adjacent workstreams; no new
platform-local budget policy). Read them there. Additional rules:

- **A. Capacity vs spend separation.** Structural capacity (worker counts,
  in-flight pools, backlog bounds) is derived from queried host resources
  and static footprint math — the vanilla formulas. Frame-time spending
  (what runs this frame/tick) is the 150 controller's business — elapsed
  grants from measured headroom. A capacity value must never be derived
  from frame headroom, and a budget must never be derived from core count.
  Keeping these separate is what lets weak hardware degrade to floors and
  future hardware scale without retuning.
- **B. No tuned capacity constants (anti-knob rule).** Every shipped
  capacity default must be traceable to: queried resources
  (`std::thread::available_parallelism`, measured buffer footprints),
  a structural reservation (threads the engine actually pins), a
  controller decision trace, or a fail-safe floor equal to today's shipped
  default. A bare integer default that is none of those is a defect. CLI
  flags remain as explicit overrides and benchmark/lane args only.
- **C. Clamp bounds need cross-host evidence.** Where a derivation needs a
  clamp (vanilla's `1..7`), the bounds are chosen where measured scaling
  flattens on **both** the desktop ladder and the Quest gate set, plus a
  written safety argument (memory bound, reserved-thread bound). Record
  the evidence next to the constant. A clamp bound justified by one host
  is rule-B defect.
- **D. Reference-first.** Each slice names its Java anchors; read those
  sources before writing the port. Divergences go to the ledger above with
  scope, reason, and the path back to parity.
- **E. Parity is falsifiable.** Any change to light execution must keep
  light output byte-identical on the existing lighting fixtures and a
  contrasting seed; any change to mesh scheduling must keep mesh output
  identical for identical inputs (order-independence proof). Perf gates do
  not substitute for output equality.
- **F. Host-neutral contracts.** New shared code compiles for
  `wasm32-unknown-unknown`. Resource inputs enter as parameters so the web
  adapter can supply browser values when 062/067 wiring lands; web ships
  on floors until then (behind on wiring is acceptable; forked on policy
  is not).

## Slice 0: Whole-Pipeline Attribution Row

Status: complete on clean runtime commit `1c8a0743`. Records:
[`../performance-records.md`](../performance-records.md) and
[`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md).

Why: every ceiling number above is stitched from rows measured on
different commits and lanes. Lever order must come from one table on one
commit, or this tactical tunes the wrong stage. This is the "understand
the whole pipeline" deliverable and it is measurement-only.

Deliverables:

- one clean-commit run set: RD10 fresh frozen, RD15 fresh frozen, RD10
  persisted frozen, plus one Quest RD5 churn row for contrast;
- a per-stage table covering worldgen job -> light -> publication ->
  client apply -> mesh admission -> mesh compile -> GPU upload -> visible,
  with, per stage: measured throughput (chunks or sections/sec), busy
  fraction of its owning thread, queue depth and oldest-age at the stage
  boundary, and per-unit cost (EWMA where the controller already tracks
  it);
- busy-fraction counters for the worldgen and light mailbox threads and
  the render compile worker(s) if not already reported — named consuming
  decision: the Slice 1-3 lever ranking below; this is 144-style
  measurement, no policy change;
- re-derive the implied stage costs in the anchor table (light ms/chunk,
  mesh ms/section) from this commit and re-pin if materially different;
- record the ranked lever order this table implies, in this section, with
  the expected ceiling after each lever (the falsification target for
  Slices 1-3).

Exit criteria: table recorded in
[`../performance-records.md`](../performance-records.md) (and the Quest row
in the Quest records doc); anchors re-pinned; lever ranking written here;
no engine behavior change.

Validation:

```bash
cargo fmt --manifest-path native/Cargo.toml --all --check
cargo test --manifest-path native/Cargo.toml -p mclone-server -p mclone-diagnostics
pnpm native:accounting:smoke
pnpm native:startup-streaming:perf
git diff --check
```

### Slice 0 Recorded Results

Desktop rows were captured from clean commit `1c8a0743`, `git_dirty=false`,
with explicit `--render-compile-workers 1 --render-compile-max-pending-jobs 4`.
Rates below divide by target render quiescence, matching the user-visible
startup completion window. Worker busy percentages are therefore load-window
busy fractions, not whole-process CPU utilization.

| Lane | Full view | Target quiescent | p95 / max frame | Over / over-2x | Feature rate | Light rate | Compile rate | Upload rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| RD10 fresh frozen | `9699.848ms` | `11186.276ms` | `11.924 / 15.384ms` | `0 / 0` | `113.7` chunks/s | `105.0` statuses/s | `639.4` sections/s | `195.2` sections/s |
| RD15 fresh frozen | `19930.189ms` | `21998.543ms` | `11.583 / 12.920ms` | `0 / 0` | `130.9` chunks/s | `133.2` statuses/s | `705.5` sections/s | `221.2` sections/s |
| RD10 persisted frozen | `1022.791ms` | `4940.262ms` | `11.947 / 14.663ms` | `0 / 0` | n/a | n/a | `1425.0` sections/s | `433.8` sections/s |

| Stage | RD10 fresh attribution | RD15 fresh attribution | RD10 persisted attribution | Boundary evidence |
|---|---:|---:|---:|---|
| worldgen job | `2161.554ms` busy, `19.3%`, `1.70ms`/chunk | `4482.406ms` busy, `20.4%`, `1.56ms`/chunk | idle | mailbox pending max `1`; not the current ceiling |
| light status | `11009.006ms` busy, `98.4%`, `9.38ms`/status | `21810.817ms` busy, `99.1%`, `7.44ms`/status | idle | light mailbox pending max `155` / `348`; fresh-startup ceiling |
| publication | host-publication max age `1546.781ms` | host-publication max age `2586.359ms` | idle | publication follows light backlog; controller cost traces stayed at cap |
| client apply | no inbound-update queue in local desktop lane | no inbound-update queue in local desktop lane | no inbound-update queue | still needs remote/151 attribution, not a local startup lever |
| mesh admission | compile-jobs max age `23.806ms`, pending `4` | max age `21.315ms`, pending `4` | max age `23.936ms`, pending `3` | admission is bounded and green on desktop |
| mesh compile | `1297.312ms` busy, `11.6%`, `0.181ms`/section | `2916.281ms` busy, `13.3%`, `0.188ms`/section | `1525.197ms` busy, `30.9%`, `0.217ms`/section | persisted lane is the clean desktop mesh-capacity signal |
| GPU upload -> visible | upload queue max age `0.000ms`; `2184` uploaded | max age `0.000ms`; `4866` uploaded | max age `0.000ms`; `2143` uploaded | upload not the desktop startup ceiling yet |

Quest RD5 chunk-view churn was captured on the same runtime commit with
workers `2`, max-pending `4`, accept/upload caps `2/16/64`, and
`45.007s` sample after `15.878s` settle:

| Field | Value |
|---|---:|
| submitted/runtime/skipped | `3239 / 3239 / 0` |
| Meta dropped-frame delta | `16` |
| app p95 / p99 / max | `8.919 / 9.678 / 15.518ms` |
| headroom avg / p05 / min | `+8.651 / +4.969 / -1.629ms` |
| app over-period frames / pct | `1 / 0.0%` |
| publication rates | feature `23.419`/s, light `11.509`/s, total `34.928`/s |
| queue max ages | inbound `133.188ms`, upload `254.848ms`, host-publication `1872.731ms`, render-compile `581.685ms` |
| peer busy totals | worldgen `15896.635ms`, light `57526.172ms`, render compile `24722.366ms` |
| render compile cost | `1.173ms`/completed section (`24722.366ms / 21085`) |

Meter-tax A/B follow-up on clean commit `30e80fbf` added
`--render-compile-worker-timing false`, which disables only the
render-compile worker busy timer/counter around each compile task while leaving
queueing, worker count, max-pending, and compile work unchanged. The timing-off
rows still completed compile work and reported zero render-compile peer busy
time, proving the switch removes the measured path.

| Lane | Timing on | Timing off | Delta | Frame result |
|---|---:|---:|---:|---|
| RD10 fresh frozen full view | `9699.018ms` | `9682.238ms` | `+16.780ms` / `+0.173%` | `0 / 0` over/over-2x both rows |
| RD10 fresh frozen target quiescent | `11078.662ms` | `11048.769ms` | `+29.893ms` / `+0.271%` | completed compile sections `7152` vs `7168` |
| RD10 persisted frozen full view | `1027.058ms` | `1016.075ms` | `+10.983ms` / `+1.081%` | `0 / 0` over/over-2x both rows |
| RD10 persisted frozen target quiescent | `4941.937ms` | `4921.454ms` | `+20.483ms` / `+0.416%` | completed compile sections `7040` both rows |

Conclusion: the render-compile worker timing meter is visible and measurable,
but the observed wall-clock tax is below the decision threshold for changing
Slice 0's lever ranking. Treat exact per-section compile costs as workload plus
meter tax, and keep the stage order: fresh startup is still light-bound, while
persisted/Quest churn still expose the render-compile tail.

Compile-feature follow-up on clean commit `7ec12fe3` put that worker busy
timer/counter behind the `perf-diagnostics` Cargo feature. Startup streaming now
reports requested, compiled, and effective timing states separately. Normal
release builds compile out the worker-task timer and metrics store; diagnostics
builds can still toggle the runtime branch.

| RD10 persisted frozen build | Requested / compiled / effective | Full view | Target quiescent | p95 / max frame | Over / over-2x | Render compile busy |
|---|---:|---:|---:|---:|---:|---:|
| compiled out | `true / false / false` | `1030.771ms` | `4929.208ms` | `10.793 / 13.455ms` | `0 / 0` | `0.000ms` |
| feature on, timing off | `false / true / false` | `1038.059ms` | `4920.899ms` | `10.913 / 17.923ms` | `1 / 0` | `0.000ms` |
| feature on, timing on | `true / true / true` | `1019.664ms` | `4926.331ms` | `10.732 / 33.613ms` | `1 / 1` | `1537.414ms` |

This distinguishes observability from budget policy: render-compile worker
busy time is a human attribution diagnostic and can compile out; submit/queue
timings that feed frame accounting stay always on.

Lever order implied by Slice 0:

1. **Slice 1: render compile capacity.** This remains next because it is the
   vanilla-shaped shared capacity slice, it has a clean persisted desktop
   signal (`4.940s` quiescence with `5696` post-full-view target rebuilds),
   and Quest churn now shows real render-compile work (`24.7s` busy,
   `581.7ms` queue age). Falsification target: RD10 persisted quiescence
   should materially fall without introducing frame misses, and Quest churn
   compile queue age must not regress.
2. **Slice 2: light/status throughput or light-publication boundary.** Fresh
   RD10/RD15 light is effectively saturated (`98-99%` busy) while worldgen
   is not. After Slice 1 removes the mesh-only tail, fresh-startup targets
   should be judged against this light ceiling first.
3. **Slice 3: publication/client-apply/upload pacing.** Host-publication age
   is already `1.55-2.59s` on desktop and `1.87s` on Quest churn, but desktop
   upload is not yet aged. Promote this only after upstream capacity makes
   publication/upload the visible tail.
4. **Shared pool topology remains evidence-gated.** The current row proves
   fixed per-stage threads leave worldgen underused while light is saturated,
   but it does not yet justify a pool rewrite ahead of the focused vanilla
   pack-pool render compile slice.

## Slice 1: Render Compile Capacity (Vanilla Pack-Pool Shape)

Why: CPU meshing is the client-side ceiling (~`1.3ms`/section, workers
`1`), and it is the stage vanilla explicitly scales with cores. mclone
already has the pack-bound half (promoted `max-pending=4`); the worker
half is a fixed `1`. 150's post-close-out thread named the worker ladder;
this slice lands it as a derived capacity, not a picked number.

Java anchors: `ChunkRenderDispatcher` (pack formula, admission mailbox,
fork-per-task), `LevelRenderer` deadline admission (150 table).

Deliverables:

- a shared capacity derivation (working home: a sans-I/O module in
  `mclone-frame-budget` next to the floors/caps config; record if review
  moves it) producing render-compile worker count and in-flight bound
  from: `available_parallelism`, a structural reservation for pinned
  engine threads (main, render, worldgen actor, light actor, XR loop
  where applicable), and a memory term from measured mesh buffer
  footprint — the vanilla pack formula with mclone's real numbers.
  Floors: today's `workers=1`, `max-pending=4`. Flags become overrides;
- desktop ladder measured at derived vs `1/2/4` (and `8` if the
  derivation can reach it): RD10/RD15 fresh frozen, RD10 persisted frozen,
  frame-budget probe, movement-frame probe. The persisted lane is the
  clean mesh-bound signal (`4.92s` quiescent baseline);
- Quest gate set with the same derivation active (its reservation-heavy
  formula should resolve low; if it does not and a gate regresses, the
  remedy is a corrected structural input or clamp bound with cross-host
  evidence — rule C — never a Quest-only constant);
- upload-drain decision: vanilla drains all pending uploads every frame;
  mclone keeps measured accept/upload seams. Record whether the higher
  compile rate makes upload/accept pacing measurable on desktop, feeding
  Slice 3;
- mesh output equality check across worker counts (rule E).

First implementation slice: the shared derivation now lives in
`mclone-frame-budget` and startup-streaming JSON surfaces it as
`render_compile_capacity_advisory`. It is deliberately non-applied
(`"applied": false`): active workers/max-pending remain the existing CLI/default
settings. The advisory logs the formula inputs and result: available
parallelism, structural reservation, measured compile request and uploaded
mesh-buffer footprint, memory budget, memory pack cap, derived worker count, and
derived max-pending jobs. Missing parallelism or missing memory/footprint input
falls back to today's `1/4` floors, with unit coverage for that rule.

Clean-commit release ladder on `67f47a53` tested explicit `1/4`, `2/4`,
`4/4`, and advisory `7/14` on RD10 persisted frozen. `7/14` dropped target
quiescence from `4934.921ms` to `1389.100ms` with `0 / 0` over/over-2x frames,
passing the persisted `<=3s` gate. Intermediate `2/4` and `4/4` barely moved
the tail, so the larger in-flight bound is part of the win.

Fresh rows did not materially improve: RD10 fresh target quiescence was
`10816.486ms` at `1/4` and `10840.326ms` at `7/14`; RD15 was `21645.497ms` at
`1/4` and `21792.767ms` at `7/14`, all with `0 / 0` over/over-2x. The 120 Hz
frame-budget and movement-frame probes both changed from `1 / 0` top-level
over/over-2x at `1/4` to `1 / 1` at `7/14`, while frame-accounting stage rows
stayed `0 / 0`. Decision: do not flip the global default from advisory data
alone. Next implementation should add an explicit opt-in or context-gated
derived-capacity path, then rerun these rows plus Quest.

Follow-up implementation added the explicit opt-in path without changing
defaults: `--render-compile-capacity derived` applies the shared derived
worker/max-pending result before startup, while omitted flags still use today's
`1/4`. Manual `--render-compile-workers` and
`--render-compile-max-pending-jobs` override the derived fields individually.
Because applied mode runs before frame reports can provide a measured mesh
footprint, the applied path uses a conservative preflight pack estimate; the
startup-streaming JSON now reports `render_compile_capacity_mode` plus the
advisory object's applied preflight result and measured post-run advisory result
separately. A short release RD2 persisted smoke on the dirty implementation
worktree resolved `derivedApplied` to `7/14`, with worker timing still compiled
out in the normal release build.

Clean-commit applied ladder on `b3147f4e` then reproduced the manual result
using only `--render-compile-capacity derived`. RD10 persisted frozen resolved
to `7/14` and reached target quiescence in `1395.463ms` with `0 / 0`
over/over-2x frames. RD10 fresh frozen was `10829.479ms` and RD15 fresh frozen
was `21611.805ms`, both neutral against the manual ladder and both `0 / 0`.
The 120 Hz frame-budget and movement-frame probes still showed `1 / 1`
top-level over/over-2x with `7/14`, while frame-accounting stayed `0 / 0`.
Decision is unchanged: explicit applied capacity is useful for persisted
local-integrated startup, but the default is not promoted.

Quest was not a fair applied-capacity row yet. The device was connected, but
Android XR startup parsing did not accept `--render-compile-capacity derived` at
the time of the row; it only accepted manual worker/max-pending flags.

Follow-up wiring moved the request into shared
`mclone-app-runtime::startup_args::StartupArgState`. Desktop, Android,
Android XR, and web now parse the same capacity request; each runtime resolves
it with its own host kind/resource inputs and preserves explicit
worker/max-pending overrides. Desktop and Android XR also have source-scan
tripwires that require new startup flags to be classified as shared startup
policy or explicit app-local/platform-local flags. The next measurement slice is
Quest RD5 orbit/churn with `--render-compile-capacity derived` active.

Quest applied-capacity rows on clean commit `ab582d3c` then proved that the
shared request reaches Android XR and resolves to Quest-local `1/4`, not the
desktop `7/14`. RD5 settled orbit repeated cleanly on app work (`0` skipped,
app p95 `11.994ms` then `12.088ms`, app over-period `0.0-0.1%`), and the
second repeat returned Meta dropped-frame delta to `15` after a first-repeat
`23`. RD5 churn was also app-work green (`0` skipped, dropped delta `17`, app
p95 `9.439ms`, app over-period `0.1%`, deadline skips `0`). Render-compile
queue age was `617.266ms`, slightly above Slice 0's `581.685ms` and below the
earlier queue-depth candidate's `678.520ms`; host-publication age rose to
`2344.678ms`, keeping publication/light as the watched downstream pressure.
Decision: the applied Quest path is validated, but global default promotion is
still not accepted because desktop 120 Hz still has one top-level over-2x
outlier at `7/14`. Keep derived capacity explicit/context-gated and move the
next Tactical 153 slice to light/status throughput.

Gates (inner loop per iteration; promotion before default flip):

- inner: RD10 persisted frozen + movement-frame probe + Quest RD5 orbit;
- promotion: RD10/RD15 fresh frozen reproduced, RD7 pressure, RD5 churn,
  one native-window `--window-frame-report` run, desktop-XR functional
  smoke, `pnpm native:xr:check`;
- acceptance: persisted RD10 target quiescent `<= 3s`; fresh lanes not
  worse; `0` over-budget frames at 60 Hz lanes; movement probe not worse
  than the pinned caveat (`1` over/over-2x); Quest envelopes green with
  compile queue ages not materially worse.

Exit criteria: derived defaults promoted for desktop and Quest
local-integrated (or falsified with the ladder evidence); derivation and
clamp-bound evidence recorded; 142 throttle inventory rows updated. Slice 1 is
currently in the "explicit/context-gated, not global default" state: desktop
persisted startup benefits strongly, Quest derives safely to floors, and
desktop 120 Hz prevents global default promotion.

## Slice 2: Light Stage Throughput

Why: light is the binding fresh-generation stage (~`17ms`/chunk implies
the ~`60`/sec pipeline ceiling the close-out lanes sit at). Vanilla offers
no parallel precedent — its light engine is a serial actor — so this
slice is explicitly: optimize first (parity-neutral), then make the
divergence decision with evidence, not by default.

Java anchors: `ThreadedLevelLightEngine` (batch shape, full-graph drain,
PRE/POST task split); mclone history 045-049 (initial light batch,
retained light world, graph drain instrumentation — prior large wins).

Deliverables:

- profile the per-chunk light cost on the Slice 0 commit into named
  sub-costs (sky column init, propagation drain, section storage,
  status/bridge serialization, publication handoff) — detour-rule scope:
  the consuming decision is option A vs B below;
- **option A (default): serial solver optimization.** Land the top
  measured parity-neutral wins (batching shape vs vanilla `taskPerBatch`,
  storage/allocation hot spots, redundant column work). Output must stay
  byte-identical on lighting fixtures + contrasting seed (rule E);
- **option B (divergence, only if A leaves light binding below the
  Slice 0 target rate):** parallel light across chunk-disjoint regions
  (tiled/checkerboard with halo ordering), N light workers derived by the
  Slice 1 capacity module, serial actor semantics preserved per region.
  Requires: ledger entry (scope, reason, parity-recovery path), byte-equal
  output vs the serial engine on fixtures + contrasting seed + a live
  RD10 world diff, and the same gate loop as every candidate;
- either way: light stage rate, busy fraction, and queue age re-measured
  into the Slice 0 table format.

First Slice 2 profiling landed in `db8a9e49` as shared instrumentation, not a
solver change. The clean release `scheduler_loading_perf` rows show the native
server-only light ceiling is dominated by changed-block rechecks, then sky graph
propagation:

| Lane | Light statuses / batches | Light compute | Compute / status | Changed-block recheck | Sky graph | Block graph | Storage swap | Publication handoff |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| RD10 server-only | `625 / 73` | `10500.835ms` | `16.801ms` | `6153.664ms` / `58.6%` | `3693.568ms` / `35.2%` | `203.333ms` / `1.9%` | `51.482ms` / `0.5%` | `61.974ms` / `0.6%` |
| RD15 server-only | `1225 / 144` | `21303.616ms` | `17.391ms` | `12831.490ms` / `60.2%` | `7116.344ms` / `33.4%` | `376.064ms` / `1.8%` | `200.390ms` / `0.9%` | `124.053ms` / `0.6%` |

Graph counters reinforce that ranking: RD10 processed `5428213` sky nodes and
`307814` block nodes; RD15 processed `9978387` sky nodes and `535659` block
nodes. Storage affected sections grew from `134490` to `253683`, but storage
swap time stayed below `1%` of light compute. The native-thread
`scheduler_loading_perf` lane has no status/bridge serialization; worker bridge
cost stays in `WorkerFrameMetrics` for web/worker paths. Decision: remain in
option A. The next code slice should reduce redundant changed-block rechecks
or make them sparse/dirty-set based while preserving byte-identical lighting
fixtures, before any option-B parallel-light divergence is considered.

The follow-up dirty RD10 classifier run on `999c78b0` added no solver behavior
change, only counters. It reported `4726` retained-light input chunks:
`729` inserted, `1224` replaced, and `2773` unchanged. The `1224` replacements
queued `5550167` raw changed-block checks; only `374161` (`6.7%`) changed
current opacity/emission facts, while `5176006` (`93.3%`) were raw-only. That
strongly supports a vanilla-shaped filter mirroring `ProtoChunk#setBlockState`'s
light-relevance gate as the next parity-neutral Slice 2 implementation.

The sparse filter then landed in `62b5d666`. It keeps counting all raw
replacement diffs, but only enqueues `check_block` for opacity/emission changes.
Clean release `scheduler_loading_perf` rows:

| Lane | View ready | Settled | Light compute | Compute / status | Raw checks | Enqueued checks | Changed-block recheck | Sky graph |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| RD10 server-only | `7896.782ms` | `8911.949ms` | `4807.892ms` | `7.693ms` | `5550167` | `374161` (`6.7%`) | `590.608ms` (`-90.4%`) | `3554.838ms` |
| RD15 server-only | `20865.423ms` | `23170.166ms` | `9386.362ms` | `7.662ms` | `10843350` | `684067` (`6.3%`) | `1177.668ms` (`-90.8%`) | `6857.777ms` |

This satisfies the first option-A win: changed-block recheck is no longer the
dominant light bucket. Remaining serial light time is now mostly sky graph drain,
so the next decision should be made from a fresh whole-pipeline re-rank rather
than assuming more retained-diff work is still the best lever.

The clean post-filter fresh startup-streaming re-rank on `0e5c2525` kept the
Slice 0 comparison knobs (`workers=1`, `max-pending=4`, frozen fluids):

| Lane | Full view | Target quiescent | Delta vs Slice 0 full / target | p95 / max frame | Over / over-2x | Host publication max age | Render compile queue max age |
|---|---:|---:|---:|---:|---:|---:|---:|
| RD10 fresh frozen | `7167.876ms` | `8201.012ms` | `-26.1%` / `-26.7%` | `10.340 / 18.045ms` | `1 / 0` | `1728.144ms` | `47.660ms` |
| RD15 fresh frozen | `14326.097ms` | `15726.106ms` | `-28.1%` / `-28.5%` | `10.475 / 13.096ms` | `0 / 0` | `2555.977ms` | `47.569ms` |

Re-rank interpretation: render compile did not become the fresh-startup
bottleneck; queue age is tiny and target render work is drained at quiescence.
The remaining visible pressure is the still-serial light lane (now mostly sky
graph drain) plus host publication age at roughly the same scale as Slice 0.
The next slice should scout those two narrow levers before selecting another
implementation change.

### Slice 2 Vanilla Comparison Scout

Verified against the 1.17.1 source on 2026-07-07 before selecting the next
lever. This is a shape comparison, not a new benchmark row.

| Topic | Vanilla 1.17.1 shape | Mclone shape after `62b5d666` | Interpretation |
|---|---|---|---|
| Light actor | `ThreadedLevelLightEngine` is serial: `checkBlock`, `updateSectionStatus`, `enableLightSources`, `queueSectionData`, and `retainData` enqueue through `ChunkTaskPriorityQueueSorter`; each activation runs up to `taskPerBatch = 5` PRE tasks, drains `runUpdates(Integer.MAX_VALUE, true, true)`, then runs POST tasks. | The retained light mailbox is also serial. `LevelLightEngine::run_updates_report` keeps vanilla's block/sky budget split, and `run_all_updates_report` drains repeated `16_384`-budget passes until no work remains. | Parallel light remains a divergence, not the next default. We are not missing a vanilla "parallel lighting" subsystem. |
| Initial sky setup | `lightChunk` marks non-empty sections, enables light sources, seeds block emitters, and lets `SkyLightSectionStorage` maintain source sections, top sections, and queued add/remove source sets. `SkyLightEngine` handles vertical empty-section skip and missing-layer sky fallback. | `SkyLightSectionStorage` has the same source/top-section queues and `SkyLightEngine` has the same graph-frontier shape. Source adds still fan out through graph seeding, but not from an obviously absent batch algorithm. | The remaining sky bucket is likely implementation cost or workload volume, not a missing vanilla stage. A sky slice should be a narrow graph/storage hot-path probe, not a rewrite. |
| Chunk priority and publication | `ChunkMap` wraps `worldgen`, `main`, and `light` mailboxes in `ChunkTaskPriorityQueueSorter`; queues are keyed by chunk ticket level and release/acquire state. Full-chunk conversion and player chunk send also run through the main mailbox with that chunk priority. | Runtime targets and feature jobs are center-priority sorted, but completed feature jobs and completed light statuses are published from FIFO pending queues under separate feature/light publication grants. The default floor is `1` unit each; adaptive caps currently top out at `4` units per gameplay tick. | This is the sharper structural divergence. Vanilla's priority sorter decides which chunk-lane task runs next; mclone's frame poll decides how many completed chunks/statuses to publish from each queue. |
| Publication age signal | Vanilla has no equivalent per-frame chunk-publication unit valve; pacing mainly sits in tick slack, mailbox priority, and client compile admission. | Post-filter startup still shows host-publication max age of `1728.144ms` RD10 and `2555.977ms` RD15 while render compile queue age is only about `48ms`. The reporter folds server pending publications, pending worldgen-publication chunks, and pending light publications into that age. | Before changing policy, split the age source. If the age is mostly pending light compute, keep working the sky graph. If it is mostly completed status/chunk publication, move the valve toward a priority/age-aware, cost-derived drain. |

Conclusion: the comparison rules out the broad concern that mclone is simply
applying initial light one block at a time instead of using vanilla's chunk
light path. The retained-diff path was doing redundant block rechecks and that
is fixed. The next implementation slice should add just enough age/source
attribution to decide whether the remaining `host-publication` age is real
publication backlog or a proxy for serial sky-graph work, then land the
corresponding smallest change.

The source split then landed as frame-pipeline schema v8. Startup-streaming,
desktop live accounting, shared XR scene accounting, and Android XR perf export
now keep the legacy aggregate `host-publication` row and add
`host-publication-runner`, `host-publication-worldgen`, and
`host-publication-light`. `SingleViewRuntimeStats` now carries the scheduler
worldgen/light publication depths so flat desktop reports the same vocabulary as
startup and XR. A short RD2 startup-streaming smoke wrote
`/tmp/mclone-153-publication-split-smoke.json` and verified schema `8` plus all
four publication rows; in that smoke the component depths were runner `41`,
worldgen `55`, and light `0`, proving the split can distinguish publication
valve backlog from light-publication backlog. This smoke is a wiring check, not
a replacement for the RD10/RD15 decision rows.

The clean RD10/RD15 decision rows on `bf4025a5` then selected the publication
valve. With the same frozen-fluid, workers `1/4`, 60 Hz knobs, RD10 reached full
view / target quiescence in `7157.870ms / 8248.762ms`, and RD15 in
`14320.163ms / 15715.883ms`, both `0 / 0` over/over-2x. Component queue ages
were:

| Lane | Aggregate | Runner | Worldgen publication | Light publication | Max pending worldgen pubs | Max pending light pubs | Max publish spent |
|---|---:|---:|---:|---:|---:|---:|---:|
| RD10 | `1626.560ms` | `1626.560ms` | `2027.595ms` | `2526.334ms` | `120` | `8` | `1.753ms` |
| RD15 | `2559.859ms` | `2287.251ms` | `3071.544ms` | `3049.842ms` | `175` | `9` | `1.807ms` |

Interpretation: light compute remains real (`5325.763ms` RD10,
`10269.710ms` RD15), but completed feature/light publication queues are aging
while each poll spends only about `1.8ms` against a `10ms` grant. The next code
slice should enter Slice 3 and make publication family `max_units` cost-derived
from elapsed grant / EWMA unit cost, leaving the old `4` as a cold-estimator
floor or safety bound. A sky-graph hot-path probe should wait until the count cap
is no longer the visible publication limiter.

Slice 3a landed in `8579fb18`. `PublicationGrant` now derives actual
`max_units` from the elapsed grant divided by the scheduler's EWMA unit cost;
the old controller cap remains the fallback only while the estimator is cold or
invalid. Clean RD10/RD15 rows on the same frozen-fluid, workers `1/4`, 60 Hz
knobs reported:

| Lane | Full view | Target quiescent | Delta vs split row full / target | p95 / max frame | Over / over-2x | Final feature/light grants |
|---|---:|---:|---:|---:|---:|---:|
| RD10 | `5106.574ms` | `5598.029ms` | `-2051.296ms` (`-28.7%`) / `-2650.733ms` (`-32.1%`) | `11.135 / 27.255ms` | `1 / 0` | `28` / `90` |
| RD15 | `9726.067ms` | `10498.777ms` | `-4594.096ms` (`-32.1%`) / `-5217.106ms` (`-33.2%`) | `10.469 / 13.450ms` | `0 / 0` | `31` / `98` |

Publication component ages after the change:

| Lane | Aggregate | Runner | Worldgen publication | Light publication | Max pending worldgen pubs | Max pending light pubs | Max publish spent |
|---|---:|---:|---:|---:|---:|---:|---:|
| RD10 | `790.833ms` | `279.265ms` | `995.863ms` | `0.000ms` | `67` | `0` | `10.005ms` |
| RD15 | `832.174ms` | `331.574ms` | `993.448ms` | `0.000ms` | `98` | `0` | `10.110ms` |

Interpretation: the publication valve is now using the elapsed grant instead of
the old count cap. The final grants match the observed per-unit estimates
(`0.345ms`/feature and `0.111ms`/light at RD10; `0.321ms`/feature and
`0.102ms`/light at RD15), and max publication spend is now roughly the `10ms`
grant. The RD10 single `27.255ms` frame is a watch item, but the worst-frame
stage spans do not attribute it to publication work and RD15 stayed `0 / 0`.
The visible queue has moved: completed light publication depth is gone, while
the light-status worker mailbox now reaches `288` RD10 / `550` RD15 pending
statuses. The next narrow slice should split and inspect that light-status
mailbox/worker pressure before changing sky-graph internals.

Gates: desktop RD10/RD15 fresh frozen (this is the slice that should move
them), Quest RD5 orbit + churn (light publication cadence and queue ages
watched), movement probe. Acceptance: light stage ceiling raised to at
least the Slice 0 ranked target with parity proofs green, or option-B
declined with the measured reason recorded and the tactical's close
conditions re-scoped accordingly.

Exit criteria: decision recorded (A sufficed / B landed / B declined with
evidence); ledger updated; anchors re-pinned.

## Slice 3: Count Knobs To Measured Valves

Why: with capacity raised, the remaining fixed counts become the valve.
The publication family cap is pinned at the sweep-knee `4`/tick (`80`
chunks/sec at 20 Hz) — a number swept on the *old* single-thread pipeline.
Vanilla has no unit cap at all at this seam: admission is elapsed
(`haveTime()`, compile deadline) with unit estimation only to bound
overshoot. The XR accept/upload lane args (`2/16/64`) are the same class
of knob, already refused promotion twice in 150.

Java anchors: `MinecraftServer.haveTime()` tick-slack drain;
`LevelRenderer.compileChunksUntil` per-unit break (both in 150's table).

Deliverables:

- publication family caps derived, not pinned: max units per grant =
  elapsed grant / EWMA per-unit cost (the controller already computes
  both; the fixed `4` becomes a floor-side safety only if the estimator is
  cold). Re-run the cadence-compatibility probe (150 Slice 3 shape) to
  prove the grant stays period-relative;
- backlog bound (`pending publication` cap) rederived from queue-age and
  memory terms instead of a fixed N; RD20-long watches peak memory as in
  150 Slice 5;
- adaptive render admission promotion decision: with Slice 1 capacity in
  place, re-A/B `--adaptive-render-admission-budget` default-on for
  desktop local-integrated against the fixed-floor path — promote or
  falsify with rows (this closes 150's deferred item);
- XR accept/upload: replace the static `2/16/64` lane args with controller
  outputs on Quest lanes, gated by RD5/RD7/churn; if the controller cannot
  beat unbounded defaults (the 150 queue-depth finding), record that and
  delete the lane args from guardrail configs rather than keeping a dead
  knob.

Gates: full inner/promotion loop as Slice 1, plus Quest churn publication
cadence `>=` Slice 0 baseline and update-queue oldest-age not materially
worse. Acceptance: no shipped count-shaped default remains in the
publication/admission/upload path that is not a floor — audited against
rule B; decision traces show caps tracking measured cost across desktop
vs Quest (the "different numbers from shared code" assertion from 150).

Exit criteria: promotions/falsifications recorded; 142 throttle inventory
updated; rule-B audit of the shipped defaults written into this section.

## Slice 4: Shared Pool Topology Decision

Why: vanilla runs one pool so idle stages donate cores; mclone pins one
thread per stage plus N mesh workers. After Slices 1-3, dedicated threads
may oversubscribe small hosts (Quest: pinned main/render/XR + gen + light
+ mesh workers) or leave desktop cores idle between stage bursts. But the
unification is a real refactor (062's parent scope), and it must be
pulled by evidence, not by symmetry with vanilla.

Deliverables:

- decision input, from the Slice 1-3 gate artifacts: named contention or
  idleness signals (Quest app-work p95 pressure with mesh workers active,
  scheduler/runner spike markers, desktop busy-fraction gaps while a
  neighbor stage is saturated);
- if the signals are present: a bounded first step only — move the
  worldgen and light actors onto the shared worker pool as serial actors
  (ProcessorMailbox semantics: CAS-guarded, one activation at a time,
  re-register), keeping every ordering and parity guarantee, under the
  full gate loop. The full 062 convergence (web workers, render worker
  trait unification) stays in 062;
- if not: record the measured "no contention at current capacity" result
  here and hand the topology work back to 062 as sequenced-later, with
  the Slice 0/1 tables as its starting evidence.

Exit criteria: decision recorded with the signal rows either way; no
half-migrated topology.

## Slice 5: Soak, Contrasting Seed, Close-Out

Deliverables:

- Quest mixed soak: add validator support to combine orbit + churn phases
  in one `>= 15 min` run (named blocker from 150 Slice 5), run it with
  the promoted defaults; record thermal note, dropped-frame drift,
  compile/update queue ages. The 150 partial-soak numbers are the
  comparison row;
- desktop RD20 long run re-checked for backlog/memory with the new
  capacity (peak pending publication vs the `208` baseline);
- contrasting seed RD10 row re-run (`74739` or ocean-heavy);
- movement-frame caveat: rerun the probe; if the `~18ms` spike persists,
  attribute it with the shared stage accounting now that worker counts
  changed the suspect set, or record it as a named open item with the
  fields consulted;
- docs closure: update 142 (throttle inventory, gate states), the law
  doc's follow-up paragraph, [`../topics/performance.md`](../topics/performance.md)
  priority queue, durable rows to both performance-records docs, and
  150's post-close-out pointer.

Exit criteria: close conditions below green or explicitly falsified with
evidence; tactical closed.

## Validating Capacity Derivations

How we know a derived capacity is right and not a laundered knob:

1. **Trace or formula for every value.** Each shipped capacity default is
   reproducible from logged inputs: queried parallelism, reservations,
   measured footprints — emitted once at startup into the existing report
   schema alongside the controller's decision panel. If a value cannot be
   recomputed from its logged inputs, that is a rule-B defect.
2. **Cross-host convergence assertion.** The same derivation code runs on
   desktop and Quest and resolves to materially different numbers from
   inputs alone (150's controller assertion, extended to capacity).
3. **Scaling-knee evidence for clamps.** Every clamp bound cites the
   ladder rows (both hosts) where scaling flattened plus the safety term
   that caps it.
4. **Floors under failure.** Missing/absurd resource inputs (unknown
   parallelism, zero memory estimate) degrade to today's shipped defaults
   exactly — asserted in unit tests, same pattern as the controller's
   cold-start floor.
5. **The gates.** RD5/RD7/churn and the desktop lane set stay green in
   default config on clean commits, reproduced per the 142 A/B rules.

## Desktop XR Role

Unchanged from 150: Quest standalone lanes are the numeric authority;
desktop XR (Mac WiVRn / Windows VDXR) is functional rot insurance — one
live smoke per promotion proving the XR frame loop with the new
capacity/valve defaults reaches `FOCUSED`, submits frames with `0`
skipped, and emits the decision panel. `pnpm native:xr:check` per
candidate.

## Non-Goals

- Parallel feature/worldgen workers (not vanilla-shaped; gen is not the
  bottleneck — the ledger records this explicitly).
- Web worker wiring for the new capacity paths (062/067 own it; web stays
  on floors behind the same contracts).
- Remote/dedicated budget shaping and host-mode treatment (151).
- Thread priority/niceness (130), CPU/GPU overlap internals (117/131),
  upload buffer/arena lifetime (128), cadence semantics (116), far-LOD
  budgets (121), XR HUD panels (148), wgpu upgrades.
- The flat-client frame-interior hoist (named follow-up in 150's
  non-goals; still deferred).
- Making Quest RD7 settled-orbit perfectly clean (render/draw-cost bound;
  117/119).
- Any new fixed global default as a "temporary" policy — floors only.

## Close Conditions

All measured on clean commits, default config (derived capacities and
valves on), reproduced per 142 A/B rules:

- desktop end-to-end fresh pipeline rate at RD10 `>= 2x` the Slice 0
  attribution row, with `0` over-budget frames at the 60 Hz target —
  the rate form is primary so the condition survives host changes;
  on `kmacbook` today that implies full view `<= ~5.5s`;
- desktop RD15 fresh full view `>= 2x` vs the `19429.505ms` anchor;
- desktop RD10 persisted target quiescent `<= 3s`;
- movement-frame probe not worse than the pinned caveat, with the spike
  either attributed or recorded as a named open item;
- Quest RD5 orbit, RD7 pressure, RD5 churn green against the 150
  close-out envelopes; churn publication cadence `>=` Slice 0 baseline;
  compile/update queue ages not materially worse; mixed soak recorded
  (clean, or falsified with attribution);
- rule-B audit green: no shipped capacity/valve default in the touched
  paths that is not formula-derived, controller-derived, or a floor;
- parity proofs green for every touched execution path (light
  byte-equality, mesh order-independence);
- wasm32 checks green for every touched shared crate; remote sessions
  still on floors.

If Slice 2 declines the light divergence and optimization alone cannot
reach the rate target, the close conditions re-scope to the measured
light-bound ceiling in the same PR that records the falsification — the
tactical then closes on honest numbers, not on stretch.

## Open Questions

- Clamp upper bounds for mesh workers on big desktops (rule C evidence
  needed at `8+`).
- Whether serial-light optimization alone clears the target rate, making
  the parallel-light divergence unnecessary.
- Whether the shared-pool topology signals appear at all at these
  capacities (Slice 4 gate), and if so whether Quest or desktop shows
  them first.
- Where the mixed-soak validator support should permanently live
  (validator script vs perf harness).

## Links

- [`150-adaptive-frame-budget-controller.md`](150-adaptive-frame-budget-controller.md)
  — controller, floors, promoted baseline, carried contract rules.
- [`142-throughput-policy-with-quest-rd5-guardrail.md`](142-throughput-policy-with-quest-rd5-guardrail.md)
  — gate discipline, throttle inventory, ceiling split.
- [`144-frame-pipeline-accounting-instrumentation.md`](144-frame-pipeline-accounting-instrumentation.md)
  — the meter; Slice 0 follows its measurement-only invariant.
- [`062-shared-threading-topology.md`](062-shared-threading-topology.md)
  — worker topology parent; Slice 4 hands off or draws from it.
- [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) —
  law doc.
- `reference/minecraft-1.17.1/src/` — all anchors verified against this
  tree; see the table above for files and line ranges.
