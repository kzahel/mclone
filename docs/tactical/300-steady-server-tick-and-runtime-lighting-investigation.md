# Tactical 300: Steady Server Tick and Runtime Lighting Investigation

Status: planned 2026-08-15

Topics: `performance`, `lighting`

## Instruction Synthesis

Use the large speed difference exposed by the closed-domain wildlife runner to
investigate ordinary Mclone server-tick overhead. Determine which costs are
real required simulation, which are idle bookkeeping, and whether runtime
lighting performs avoidable repeated work. Measure before changing policy.

The wildlife-only tick is an attribution control, not a proposed production
server mode. A normal server must continue to advance tickets, chunks,
scheduled and random blocks, fluids, spawning, players, persistence,
publications, lighting, physics, and every other applicable world system.

## Motivation and Current Evidence

Tactical
[`299`](299-closed-domain-wildlife-population-simulation.md) added a bounded
diagnostic tick over an immutable, already-generated and already-lit chunk set.
It retains ordinary creature AI, movement, collision/pathfinding, block
queries, lifecycle/resource rules, births, deaths, remains, rabbit digging and
crop raids, and persistence dirtying. It omits unrelated scheduler,
publication, block/fluid, generic spawning, player, network, and external
physics phases.

An early representative one-day radius-8 measurement took roughly 40 seconds
through the full authoritative tick and roughly 1.7 seconds through the
ecology-only tick. The matching canary produced identical ecology identities,
births, deaths, forage, and remains. That approximately 23x diagnostic
difference proves that substantial work lives outside creature ecology; it
does not identify lighting, prove any full-tick phase wasteful, or establish
equivalence for omitted world systems.

The ordinary full tick currently performs, for every loaded dimension:

- scheduler ticket/holder reconciliation, worker and persistence polling,
  completed feature/light publication, pending unload processing, and active
  block/entity chunk-set production;
- scheduled and random block ticks;
- scheduled fluid due-work and mutations;
- generic natural spawning;
- entity AI, movement, lifecycle, item collection, and gameplay cue routing;
- optional general physics-engine steps;
- time, player, interest, chunk, entity, and network-facing publication; and
- persistence dirtying and later checkpoint work.

Initial `ChunkStatus::Light` computation is demand-driven worker work. The
gameplay tick polls and budget-publishes completed light results; it does not
recompute every loaded chunk's lighting merely because a tick occurred.

Runtime block mutation has a different and still provisional shape.
`refresh_runtime_lighting_after_block_change` filters changes that do not alter
light properties, but a qualifying edit currently constructs a fresh retained
light computation, gathers the affected loaded status inputs, and may
recompute/publish a 3-by-3 chunk neighborhood synchronously. Multiple nearby
changes can invoke overlapping refreshes independently. This is not yet the
Java-shaped live `checkBlock`/revisioned light-delta path recorded as P7 in
[`../topics/lighting.md`](../topics/lighting.md), so any optimization must
coordinate with that direction rather than entrench the provisional bridge.

## Questions to Answer

1. In a fully loaded, fully lit, stationary world, what is the full server tick
   cost by phase and host mode?
2. Which nominally idle phases are constant-time, and which scale with loaded
   chunks, holders, entities, scheduled entries, dimensions, or uptime?
3. When no feature or light work is pending, how much does scheduler
   reconciliation and publication polling cost?
4. Does the scheduler rebuild block/entity ticking sets or holder plans when
   the ticket generation is unchanged, and are current cache-hit paths
   sufficient?
5. What costs come from empty scheduled block/fluid queues versus actual due
   work?
6. What are the count, affected-chunk overlap, CPU time, allocation/copying,
   and publication costs of runtime relighting under isolated and clustered
   block mutations?
7. Does batching light-affecting mutations by tick/section/chunk preserve
   exact results while avoiding repeated 3-by-3 work?
8. How do local-integrated and dedicated-server results differ once client
   rendering and transport are kept outside the server-tick measurement?
9. Does browser/WASM worker publication expose a materially different idle or
   active cost after the native owner is understood?
10. Which costs grow with animal population and belong to ecology budgeting,
    rather than general tick orchestration?

## Binding Investigation Rules

### Measure the full owner before proposing a fast path

Use `ServerSimulationTickTiming`, `ServerTickTiming`, scheduler publication
diagnostics, light-mailbox timing, fluid timing, entity ecology work counters,
and process CPU/memory evidence. Extend those owners only where an attribution
cannot currently be made.

Every timing row must also record semantic work counts. A faster row that
quietly executes fewer due ticks, publishes fewer required results, changes the
loaded domain, loses mutations, or defers work beyond the measurement window
is not an optimization result.

Do not infer a phase's cost by subtracting two broad modes when a narrower
counter or controlled gate can measure it directly. The ecology-only runner is
useful as a lower-bound control, not as a substitute for phase attribution.

### Preserve production semantics

Do not promote `try_closed_wildlife_ecology_tick` into ordinary gameplay. Do
not freeze lighting, fluids, crops, scheduled blocks, generic spawning,
players, persistence, or chunk lifecycle merely because a stationary
benchmark does not need them.

Acceptable optimizations include:

- caching immutable results until their explicit generation/revision changes;
- O(1) idle early-outs for empty due-work owners;
- keyed cancellation/coalescing of superseded work;
- batching equivalent mutations before one shared computation/publication;
- staggered species AI decisions behind unchanged immediate safety; and
- moving computation behind the established native-thread/Web-Worker job
  contract without changing authoritative ordering.

Any divergence from Minecraft Java 1.17.1 tick or live-light semantics must
name the reference behavior, platform reason, affected facts, and future parity
effect before implementation.

### Keep lighting questions separated

Measure three distinct lighting costs:

1. cold initial/status computation in the light worker;
2. foreground publication of completed light statuses; and
3. runtime response to live opacity/emission changes.

Do not label scheduler polling as light computation, or use cold chunk startup
to infer steady runtime-mutation cost. Do not tune initial-light graph/storage
work, publication grants, and runtime mutation batching as one undifferentiated
lever.

The runtime-mutation experiment must include a no-light-property control. Raw
block-state changes that preserve opacity/emission should not pay light-solver
work. Light-affecting changes must retain exact resulting sky/block sections,
chunk revisions, client deltas, and render-section dirtying required by the
accepted live-light contract.

### Avoid measurement-created overhead

Follow [`../topics/performance.md`](../topics/performance.md): use bounded live
accounting, finite exact capture windows, and construct rich percentile reports
after the measured interval. Instrumentation-on/off controls must establish
that the profiler itself does not materially change the tick.

Generated profiles, traces, CSV, and reports belong under `/tmp`. Do not add
large benchmark artifacts or machine-specific values to the repository.

## Required Benchmark Matrix

All rows use release builds, exact commands, fixed seeds/content, a warmup,
fixed measurement duration, and repeat alternation where variance matters.

### Steady-state rows

- fully generated/lit stationary domain with one stationary interest source;
- the same domain with no creatures;
- the same domain with a declared ordinary wildlife population;
- a fully persisted reload of the same already-lit records;
- radius-4 and radius-8 controls to expose chunk/holder scaling; and
- one versus multiple loaded dimensions if multi-dimension fixed overhead is
  material.

For each row record full tick total, scheduler, block, fluid, entity, physics,
event/publication application, work counts, loaded/ticking chunk counts,
holder updates/cache hits, queue depths, CPU, and memory.

### Controlled isolation rows

- normal lighting versus the existing lighting-disabled diagnostic;
- ordinary fluid ticks versus frozen-fluid diagnostic;
- empty scheduled queues versus a fixed due-work fixture;
- spawning enabled versus a diagnostic no-spawn control over the same initial
  identities;
- full authoritative versus ecology-only tick over the exact same immutable
  domain; and
- local-integrated versus dedicated server, excluding client/render time from
  the compared server window.

These controls attribute cost. They do not define shippable feature-off modes.

### Runtime lighting mutation rows

Use identical loaded, already-lit terrain and compare:

- one isolated light-neutral block-state mutation;
- one isolated opacity-changing mutation;
- one isolated emission-changing mutation;
- 16 and 64 mutations separated across non-overlapping neighborhoods;
- 16 and 64 clustered mutations whose 3-by-3 neighborhoods overlap heavily;
- fluid-produced mutation bursts; and
- representative player construction/destruction bursts.

Record block changes accepted, changes affecting light, refresh calls,
candidate and unique affected chunks/sections, input bytes cloned/read, graph
work, compute/publication time, light sections changed, client deltas, and
render sections dirtied. Compare current per-change refresh against an
offline/coalesced prototype only after the baseline is captured.

## Candidate Improvements After Attribution

These are hypotheses, not pre-approved implementation:

1. **Stable scheduler plan reuse:** skip holder/ticking-set reconstruction when
   ticket and relevant readiness generations are unchanged.
2. **Cheaper idle owners:** make empty scheduled block/fluid, persistence, and
   publication phases bounded O(1) without weakening due-work ordering.
3. **Runtime light mutation batch:** collect light-affecting changes for the
   authoritative tick, deduplicate affected sections/neighborhoods, run one
   revision-aware light update, and publish the exact combined delta.
4. **Persistent live light solver state:** replace provisional fresh-state
   recomputation only as part of the Java-shaped P7 live-delta direction.
5. **Ecology cadence/budget refinement:** retain per-tick movement safety while
   staggering slow decisions, social search, habitat queries, and path
   requests; keep this separately attributable from scheduler improvements.

If no phase is material under realistic steady or mutation workloads, record
that result and stop. Do not land complexity merely because the ecology-only
control is faster.

## Human Review Points

1. **Attribution review:** inspect the complete baseline matrix, phase totals,
   semantic work counts, variance, and native profiles before selecting an
   optimization.
2. **Lighting architecture review:** if runtime relighting is material, review
   whether the bounded correction belongs in the provisional bridge or must be
   the first P7 live-delta slice.
3. **Candidate review:** inspect matched correctness and performance evidence
   before promoting any changed scheduler, cadence, or batching behavior.

## Execution Slices

1. Add a focused headless steady-server benchmark around the existing full
   simulation tick and persisted real-world setup.
2. Fill missing bounded phase, idle-work, runtime-relight, and semantic-work
   diagnostics; prove instrumentation overhead is negligible.
3. Run native steady-state, controlled-isolation, and runtime-mutation
   baselines and retain reports under `/tmp`.
4. Profile the dominant row and update the performance/lighting topics with
   attribution, including a no-change conclusion if appropriate.
5. Stop for human review and select at most one bounded first correction.
6. Implement that correction in the shared owner with exact state/output
   fixtures and matched A/B evidence.
7. Validate native dedicated/local-integrated boundaries, browser/WASM when
   the changed owner affects it, and any client/render light-delta consumer.
8. Update living docs with accepted results, rejected hypotheses, and the next
   recommendation.

## Acceptance

The investigation phase is complete when:

- exact commands reproduce a phase-attributed full-tick report over stable,
  due-work, and runtime-light-mutation workloads;
- every timing row includes matching work/conservation counts and cannot hide
  deferred or skipped required work;
- idle scheduler and lighting publication cost is distinguished from active
  light compute and runtime mutation cost;
- the ecology-only/full difference is decomposed far enough to identify the
  dominant production phases or to show that no single phase dominates;
- instrumentation overhead is measured and bounded;
- local-integrated and dedicated server ownership is not conflated with
  client/render cost; and
- the performance and lighting topics record the evidence-backed next action.

Any later optimization is accepted only when exact gameplay/ticket/block/
fluid/light/entity/persistence results remain unchanged for its declared scope,
work and memory stay bounded, and matched release evidence shows a material
improvement without worse p95/p99 tick tails or cross-platform regressions.

## Non-Goals

- making the ecology-only diagnostic a gameplay or dedicated-server mode;
- freezing normal world systems or reducing simulation distance to win a
  benchmark;
- redesigning all server scheduling before attribution;
- combining initial-light throughput, runtime light correctness, and render
  lighting into one implementation slice;
- changing the wildlife population model or closing Tactical 299;
- optimizing client rendering, GPU terrain work, or network encoding unless a
  server-boundary measurement proves they were incorrectly attributed; and
- promising a specific speedup before the baseline matrix exists.
