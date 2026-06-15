# Tactical 48 — Vanilla scheduler trace oracle

Build a source-backed, executable trace of vanilla Minecraft Java 1.17.1 chunk-status scheduling so decorated-chunk parity work can use the real cross-chunk `FEATURES` commit order instead of an inferred order.

Status: first bounded oracle implemented. Tactical [`47-generated-chunk-status-orchestration.md`](47-generated-chunk-status-orchestration.md) made status gates explicit in `mclone`; this tactical now has an executable vanilla spawn-bootstrap trace for the 3x3 `FEATURES` commit order around chunk `(0,0)`.

## Source files (read before writing)

| Java / reference source | Why it matters |
|---|---|
| [`ChunkStatus.java`](../../reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/ChunkStatus.java) | status parent chain, dependency ranges, `FEATURES` write cutoff, `LIGHT` range |
| [`ChunkMap.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkMap.java) | `getChunkRangeFuture(...)`, `getDependencyStatus(...)`, `schedule(...)`, `scheduleChunkGeneration(...)`, `prepareAccessibleChunk(...)`, `prepareTickingChunk(...)` |
| [`ChunkHolder.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java) | per-chunk futures, ticket-level to target-status mapping, full/ticking promotion |
| [`ServerChunkCache.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java) | player-facing chunk requests and ticket-driven scheduling entry points |
| [`DistanceManager.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/DistanceManager.java), [`TicketType.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/TicketType.java) | player/light tickets and target levels |
| [`ChunkTaskPriorityQueueSorter.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueueSorter.java), [`ChunkTaskPriorityQueue.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkTaskPriorityQueue.java), [`ProcessorMailbox.java`](../../reference/minecraft-1.17.1/src/net/minecraft/util/thread/ProcessorMailbox.java) | queue priority, dequeue order, and mailbox execution sequencing |
| [`WorldGenRegion.java`](../../reference/minecraft-1.17.1/src/net/minecraft/server/level/WorldGenRegion.java) | confirms which neighboring chunks a `FEATURES` task can mutate |

Read [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md) first. This tactical turns that source-backed ordering model into a measured trace fixture.

## Current problem

Tactical 47 now gives `mclone` explicit `FEATURES`, `LIGHT`, `FULL`, 3x3 feature-stability, and publication gates. That is necessary but not sufficient for exact decorated parity.

Ordinary biome decoration is not commutative. A `FEATURES` pass centered at chunk `A` can write into the 3x3 neighborhood around `A`. Therefore a target chunk can be mutated by its own `FEATURES` pass and by the 8 neighboring `FEATURES` passes. If two neighboring passes write overlapping blocks, the later committed write wins.

The existing Java `feature-order-trace` oracle records one center chunk's internal biome/feature seed order. It does not answer which center chunk's `FEATURES` pass commits first under vanilla loading.

## First trace result

The first bounded oracle path is `pnpm --silent oracle:gen scheduler-trace --seed 12345 --chunk-x 0 --chunk-z 0`. It runs a temporary deobfuscated 1.17.1 dedicated server and instruments `ChunkStatus.generate(...)` through an oracle-only shadow class.

Committed fixture:

- [`../../test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json`](../../test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json)

Observed `FEATURES` completion/commit order for the target 3x3 in that fixture:

```text
(-1,-1) -> (0,-1) -> (1,-1)
(-1, 0) -> (0, 0) -> (1, 0)
(-1, 1) -> (0, 1) -> (1, 1)
```

The current `GeneratedWorldHost` job ordering uses `sortChunkCoordinates(...)`, which is z-major then x-major. For this bounded fixture, that host order matches the observed vanilla `FEATURES` commit order. No runtime feature-generation changes are made in this tactical.

The default trace stops at the `FEATURES` gate because that is the order needed before tactical 46 resumes. The oracle child JVM pins `ActiveProcessorCount=2` and HotSpot `hashCode=3` so vanilla's background executor and identity-hashed scheduling collections are reproducible. Use `--stop-status full` for an extended trace that includes later `LIGHT`/`SPAWN`/`HEIGHTMAPS`/`FULL` completions; those phases are asynchronous and should not be used as the stable `FEATURES` commit-order fixture.

## Questions to answer

1. For a fixed load scenario, what exact order does vanilla make chunks dependency-ready, schedule, start, and complete for each status?
2. For the 3x3 neighborhood around chunk `(0, 0)`, what is the vanilla completion/commit order of `FEATURES` tasks?
3. Does vanilla's observed order differ between initial spawn bootstrap and an ongoing player-ticket load?
4. Which order should `mclone` use when applying cross-chunk feature write plans?
5. Is the current host order already compatible with the vanilla trace, or does it need an authoritative ordered commit queue?

## Scope

In scope:

- an executable vanilla 1.17.1 scheduler trace oracle
- a small, committed trace fixture or normalized expected trace for seed `12345`
- at least one fixed player/spawn load scenario that reaches chunk `(0, 0)` and its immediate neighbors
- event records for status dependency readiness, scheduling, task start, task completion, and chunk publication/send eligibility where practical
- focused comparison against the current `mclone` generated-world host order
- a documented deterministic commit-order rule for neighboring `FEATURES` side effects

Out of scope:

- exact decorated block mismatch burn-down; that returns to tactical 46
- broad DistanceManager parity beyond the load scenarios needed for this trace
- full official-server instrumentation if a smaller decompiled-source oracle can prove the same scheduling order
- changing feature placement internals except to add temporary diagnostics
- structures implementation beyond preserving their existing status slots in the trace

## Trace model

The trace should normalize each event to a stable JSON shape:

```json
{
  "sequence": 123,
  "phase": "dependency_ready | scheduled | task_start | task_complete | publish_ready | sent",
  "status": "FEATURES",
  "chunkX": 0,
  "chunkZ": 0,
  "ticketLevel": 33,
  "targetStatus": "FULL",
  "scenario": "spawn_bootstrap"
}
```

Minimum required statuses:

- `STRUCTURE_STARTS`
- `STRUCTURE_REFERENCES`
- `BIOMES`
- `NOISE`
- `SURFACE`
- `CARVERS`
- `LIQUID_CARVERS`
- `FEATURES`
- `LIGHT`
- `SPAWN`
- `HEIGHTMAPS`
- `FULL`

Minimum required chunk window:

- center `(0, 0)`
- its 3x3 `FEATURES` stability neighborhood
- any extra chunks needed to explain why those 9 chunks were dependency-ready, especially the `FEATURES` range-8 input window
- for that input window, record the requested dependency status per offset. This must prove the vanilla mixed-status rule: center/radius-1 at `LIQUID_CARVERS`, outer radius 2-8 at `STRUCTURE_STARTS`, not a flattened terrain window.

## Implementation plan

1. **Static source pass**
   - Re-read the scheduler files above.
   - Write down the exact event points that correspond to dependency readiness, scheduling, start, and completion.
   - Confirm whether a decompiled-source harness can instrument those points without replacing too much of the server.

2. **Add the trace oracle path**
   - Prefer a Java oracle under `oracle/java/` if it can execute the real 1.17.1 classes and record internal events.
   - If direct hooks are impossible through the jar classpath, use a narrow patched-source/shadow-class approach documented in the oracle README.
   - Do not treat a handwritten TypeScript scheduler simulation as the vanilla oracle.

3. **Trace one bounded scenario first**
   - Seed `12345`.
   - Chunk `(0, 0)`.
   - Small load radius sufficient to force `FULL`/send eligibility for the center.
   - Record all status events needed to order the center 3x3 `FEATURES` completions.
   - Record enough dependency-status events to catch accidental promotion of metadata-only outer-ring chunks to terrain statuses.

4. **Add a current-host trace**
   - Emit the analogous `mclone` status events from `GeneratedWorldHost` / `GeneratedRenderLevel`.
   - Keep this diagnostic disabled in normal operation.

5. **Compare traces**
   - Add a focused test or script that reports the first divergence in status order.
   - Pay special attention to the `FEATURES` completion order for the 3x3 neighborhood around `(0, 0)`.

6. **Define the host commit rule**
   - If current order matches vanilla, document the invariant and add regression coverage.
   - If it does not, add a follow-up implementation task: workers may compute feature write plans, but the host applies them in the vanilla traced order.

7. **Update downstream docs**
   - Done: tactical 46 references the scheduler trace and host commit rule before claiming exact parity.
   - Update [`../worldgen-deterministic-order.md`](../worldgen-deterministic-order.md) with the observed trace result and fixture path.

## Validation

Required:

- `./oracle/build.sh`
- the new scheduler trace oracle command for seed `12345`
- the current `feature-order-trace` command for seed `12345`, chunk `(0, 0)`, as a supporting sanity check
- `pnpm test -- test/runtime/generated-world-boundary.test.ts` if host trace code is added
- `pnpm typecheck`

If the oracle path needs official server or Mojang artifacts, follow the project bootstrap rules in [`../../AGENTS.md`](../../AGENTS.md) and request network escalation immediately if the sandbox blocks downloads.

## Done when

- there is an executable vanilla scheduler trace for at least one bounded 1.17.1 load scenario
- the trace identifies the `FEATURES` completion/commit order for the 3x3 neighborhood around chunk `(0, 0)`
- `mclone` has either been shown to match that order or has a documented follow-up to apply feature side effects in that order
- tactical 46 explicitly references the trace result before resuming full decorated mismatch burn-down
- the oracle README documents how to regenerate the trace

## Next

This trace has been consumed by [`46-full-decorated-spawn-chunk-parity.md`](46-full-decorated-spawn-chunk-parity.md): the bounded spawn fixture's observed `FEATURES` order matches the current host order, and the remaining spawn differences were generated-liquid tick timing, not source-sorted decoration jobs.

For future full-decorated fixture burn-downs, keep using these diagnostics together before changing runtime ordering:

1. the full decorated block diff
2. the per-chunk `feature-order-trace`
3. the vanilla scheduler trace from this tactical
