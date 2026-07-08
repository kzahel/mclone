# 155: Tactical 153 Follow-Up Investigations (Grab-Bag)

Status: open / active grab-bag opened 2026-07-08 after `b145c196` closed
[`153`](153-vanilla-shaped-chunk-pipeline-capacity.md) as a pipeline-capacity
pass. 153 raised the desktop chunk-pipeline ceiling (cost-derived publication
grants, sparse retained-light filtering, explicit render-compile capacity) and
tightened Quest measurement hygiene, but it deliberately handed several items
off rather than keeping the tactical open. This doc collects the reviewable
follow-ups from that close-out plus the code-review findings after it, ranks
them, and tracks each to a decision. Items graduate to their own tactical when
the work is larger than an investigation (sky-light graph, shared-pool
topology).

Workstream: native Rust shared performance/architecture.

## Working rules

- **Reference-first, every item.** Before changing any behavior, read the
  1.17.1 counterpart in `reference/minecraft-1.17.1/src/` and record what
  vanilla actually does. Divergence is a decision, not an accident. This is the
  same rule 153 ran under and it applies to every item below.
- **Capacity vs spend separation** (153 rule A) and **anti-knob** (153 rule B)
  carry over: shipped defaults must be formula-derived, controller-derived, or a
  fail-safe floor equal to today's shipped default.
- **Parity is falsifiable** (153 rule E): any change to light execution stays
  byte-identical on the lighting fixtures + a contrasting seed; mesh scheduling
  changes stay order-independent.
- One item per session where practical; update the item's Status line with the
  interpretation/result and commit before moving on.

## Priority Summary

| # | Item | Class | Priority | Status |
|---|---|---|---|---|
| P0 | Unbounded pipeline/light memory under sustained movement | correctness / Quest stability | **highest** | investigating |
| P1 | Movement-frame / 120 Hz over-2x attribution at `7/14` | unblock (promotes shipped win) | high | open |
| P2 | Sky-light graph hot path (fresh-startup ceiling) | new capability | medium (own tactical) | open |
| P3 | Mesh-worker clamp (`7`) rationale + cross-host evidence | anti-knob debt / docs | low | documented below |
| V | Close-condition verification backlog (RD20 memory, wasm32, Quest soak, parity) | evidence | folds into P0/promotions | open |

Priority note: P1 has the highest *unblock* value (it is the only thing gating
default promotion of the persisted RD10 `4.9s -> 1.39s` win), but P0 is a
latent stability risk on the memory-constrained platform (Quest), so P0 goes
first per the "do the most important thing first" call. P3 is low urgency
because derived render-compile capacity is explicit/context-gated, not a shipped
default; it is documented here so the `7` is not mistaken for a measured knee.

---

## P0: Unbounded pipeline/light memory under sustained movement

**Finding.** 153 made feature publication drain fast (cost-derived elapsed
grant) while light compute stayed serial and sky-graph-bound. That shifts the
accumulation point downstream of feature publication, and two structures on that
path have no visible bound:

1. **Retained light world has no eviction.**
   `RetainedLightWorld.chunks: BTreeMap<ChunkPos, Vec<RawBlockId>>`
   (`native/crates/mclone-server/src/light_world.rs:359`) is only ever inserted
   into (`upsert_chunk`, line ~412). There is no `self.chunks.remove` anywhere in
   the file (the only `remove`, line 176, is on a local temporary). Each entry is
   a full `height x 16 x 16` `RawBlockId` array. As the player moves through new
   terrain the map grows monotonically, even though the scheduler unloads the
   chunks themselves (`pending_unloads` / `process_pending_unloads`,
   `scheduler.rs:2412`). Scheduler state is bounded; the retained light block
   store is not wired into that unload path.

2. **Light-status backlog has no direct bound and no live controller feedback.**
   `pending_light_publications` is an unbounded `VecDeque`
   (`scheduler.rs:496`, extended at `:3007`), and the publication controller
   cannot throttle it: `chunk_publication_budget_controller_config()` ships
   `queue_age_limit_ms: None` and `queue_depth_limit: None`
   (`scheduler.rs:4568-4569`), and the per-tick input is fed synthetic
   `headroom_p05 = target_period`, `app_work_p95 = 0` (`scheduler.rs:~936`). Trace
   through `pressure()` (`mclone-frame-budget/src/lib.rs:727-762`): FrameMiss,
   NegativeHeadroom, and QueueAge all require inputs never set here, so
   `pressure()` returns `None` unconditionally and both publication families pin
   at their `0.20` cap. The `with_queue_age_ms(queue_depth, ...)` plumbing is
   therefore inert — light backlog exerts zero backpressure on the grant. The
   only real bound is the upstream worldgen-admission throttle
   (`feature_publication_backlog_blocks_job`, `scheduler.rs:2760`); the light
   side has no equivalent.

**Why it matters.** On a bounded settled/frozen world (RD10/RD15) this peaks and
drains — that is all 153 measured. Under sustained churn / long live movement
(RD20, real play), item 1 is a monotonic leak and item 2 has no cut path. Quest
is RAM-constrained, so this is the platform where it bites first. The 153
close-out asserts "didn't regress Quest" but never re-ran the deferred RD20-long
backlog/memory watch with the new grants active (see V), and the `208`
peak-pending baseline it would compare against was measured on the *old*
count-capped pipeline.

**Evidence to gather / vanilla reference to check.**
- Vanilla bound is the **loaded chunk set**, not a queue cap: `DistanceManager`
  / ticket levels decide which chunks are loaded, and `ChunkMap.processUnloads`
  drops chunk holders (and their `LightChunk` / `DataLayer` light storage) when
  tickets expire. `ChunkTaskPriorityQueueSorter` backlog is `Integer.MAX_VALUE`
  precisely because the ticket system upstream is the real bound. Read
  `DistanceManager`, `ChunkMap.processUnloads` (150 anchor), and how
  `ThreadedLevelLightEngine` / `LightChunk` light data is released on unload.
- Confirm whether mclone's `process_pending_unloads` reaches the retained light
  world at all, or whether the retained world is an mclone divergence
  (045-049 lineage) that was never wired into unloading. Record the divergence
  and the parity path.
- Empirical: RD20-long (and a churn variant) watching peak `RetainedLightWorld`
  entry count, peak `pending_light_publications`, peak light-status mailbox
  pending, and process RSS vs sample time. This is the V RD20 watch.

**What "done" looks like.** Either (a) the retained world + light backlog are
bounded by the loaded/ticket set the way vanilla bounds them, with an RD20-long
row showing flat (not monotonic) memory under churn; or (b) a recorded proof
that the structures are already bounded by an upstream mechanism I missed, with
the RD20 row as evidence. Any bound added must be vanilla-shaped (tied to the
loaded chunk set / unload path), not a fixed cap knob (rule B). Light parity
stays byte-identical (rule E).

**Status: investigating (2026-07-08).** See "P0 Investigation Log" below.

---

## P1: Movement-frame / 120 Hz over-2x attribution at `7/14`

**Finding.** Derived render-compile capacity (`7/14` on this host) took
persisted RD10 target quiescence from `~4.9s` to `~1.39s` with frame-accounting
stage rows `0/0`, but the 120 Hz frame-budget and movement-frame probes flipped
from `1/0` to `1/1` top-level over/over-2x (153 Slice 1). That single top-level
over-2x outlier is the *only* stated reason global default promotion is declined
— the shared derivation, the applied path, and the Quest floor-safe resolution
all validated. 153 handed this off unattributed; Slice 5 explicitly called for
rerunning and attributing it "now that worker counts changed the suspect set."

**Vanilla reference / hypothesis to test.** 153 Slice 1's own "upload-drain
decision" flagged that higher compile throughput may make upload/accept pacing
measurable on desktop (feeding Slice 3). Vanilla drains all pending uploads every
frame (`ChunkRenderDispatcher.uploadAllPendingUploads` / `LevelRenderer`); mclone
keeps measured accept/upload seams. Hypothesis: the `7/14` outlier is an
upload/accept burst (more sections compiled per unit time -> upload spike), not
publication (153 already found worst-frame spans do not attribute it to
publication). If confirmed, the fix is Slice-3 upload pacing and clearing it
promotes the whole capacity slice to default.

**Method.** Diff worst-frame stage spans at `1/4` vs `7/14` on the movement +
120 Hz probes; check upload queue age / uploaded-section count on the outlier
frame; test upload-drain pacing. Reference: `ChunkRenderDispatcher`,
`LevelRenderer` (153 anchor table).

**What "done" looks like.** The outlier is attributed to a named stage and
either fixed (unblocking default promotion of derived capacity for desktop
local-integrated) or recorded as an accepted, understood cost with the promotion
decision re-made on that basis.

**Status: open.**

---

## P2: Sky-light graph hot path (fresh-startup ceiling)

**Finding.** After 153's wins, fresh RD10/RD15 startup is light-bound, and
`~73-74%` of light compute is specifically sky-graph traversal (`run_updates`);
RD10 processes `~5.4M` sky nodes. 153's Slice 2 scout confirmed mclone has
vanilla's frontier *shape*, so the remaining cost is implementation/workload
volume, not a missing vanilla stage. This is the honest remaining ceiling and
153 explicitly re-scoped it to lighting work.

**Vanilla reference / probe.** Read `SkyLightEngine`, `SkyLightSectionStorage`
(vertical empty-section skip, source/top-section queues), and
`LayerLightEngine.checkNode` propagation. First probe is narrow, not a rewrite:
per-node cost and storage representation (array vs map section storage),
redundant re-propagation, and vertical empty-section-skip parity. mclone lineage
`026`, `037-049`.

**What "done" looks like.** A profiled attribution of the sky-node cost and a
selected parity-neutral optimization (or a recorded reason none is available).
This item should graduate to its own lighting tactical once P0 is settled;
parallel lighting stays declined by default (vanilla light engine is serial).

**Status: open (candidate own tactical).**

---

## P3: Mesh-worker clamp (`7`) rationale + cross-host evidence

**Documented rationale (why `7`).** The clamp upper bound
`VANILLA_RENDER_COMPILE_BACKGROUND_WORKER_CAP = 7`
(`native/crates/mclone-frame-budget/src/lib.rs:437`) is adopted directly from
vanilla 1.17.1 `Util.makeExecutor`, which sizes the shared background pool as
`Mth.clamp(Runtime.getRuntime().availableProcessors() - 1, 1, 7)` (verified in
153 anchor table, line 126). It is vanilla's 2016-era period constant: the
*formula shape* (clamped, `cores - 1`, resource-derived) is what 153 adopted;
the specific upper bound `7` is inherited, not measured on mclone hosts.

**This host, for the record.** `kmacbook` is an **Apple M4 Pro, 14 logical
cores** (`available_parallelism = 14`). Derivation on this host: usable
`= 14 - reservation ~= 13`, `cpu_worker_cap = clamp(13, 1, 7) = 7`;
`cpu_pack_cap = 14` (64-bit branch, `lib.rs:585-589`); memory pack cap `>= 14`,
so `worker_count = min(7, 14) = 7`, `max_pending = max(4, 7, 14) = 14` -> the
observed `7/14`. The clamp leaves `~6` usable cores unallocated to compile
workers on this machine.

**Rule-C status (open, low urgency).** Per 153 rule C, a clamp upper bound needs
cross-host scaling-knee evidence; 153's top open question is "clamp upper bounds
for mesh workers on big desktops (rule C evidence needed at `8+`)." The `8..13`
worker ladder was never run because the derivation caps at `7`. Since derived
capacity is explicit/context-gated (not a shipped default), this is low urgency
— but the `7` should not be cited as a measured knee until that ladder exists.

**What "done" looks like.** An `8..13` render-compile-worker ladder on this
14-core host (persisted RD10 the clean mesh-bound signal), recorded next to the
constant: either measured flattening justifies keeping `7`, or the knee is
higher and the clamp bound rises with the evidence and a written safety term
(memory bound). Only needed if/when default promotion of derived capacity is
reconsidered (couples to P1).

**Status: documented; ladder deferred (low urgency).**

---

## V: Close-condition verification backlog

153's close conditions and Slice 5 deliverables that were not evidenced in the
close-out. These are not new investigations; they are the measurement rows that
several items above depend on. Fold each into the relevant item rather than
running them cold.

- **RD20-long backlog + memory watch with cost-derived grants on** (peak pending
  publication vs the `208` baseline, plus RSS / retained-world size). Primary
  evidence for **P0**.
- **wasm32 checks green for every touched shared crate** (`mclone-server`,
  `mclone-light`, `mclone-frame-budget`, `mclone-app-runtime`). A stated 153
  close condition; confirm the `perf-diagnostics` gating + cost-derived grant
  changes still compile for `wasm32-unknown-unknown` and degrade to floors
  (simulation timing is `0` on wasm -> count-cap fallback; expected but
  unverified in the close-out).
- **Quest RD5/RD7/churn + mixed soak** re-run under default config after any P0
  bound lands, to confirm no regression on the constrained platform.
- **Parity** (light byte-equality on fixtures + contrasting seed; mesh
  order-independence) re-checked for any touched execution path.

**Status: open; scheduled with the items they support.**

---

## Links

- [`153-vanilla-shaped-chunk-pipeline-capacity.md`](153-vanilla-shaped-chunk-pipeline-capacity.md)
  — parent; close-out, hand-off list, contract rules, Java anchors.
- [`150-adaptive-frame-budget-controller.md`](150-adaptive-frame-budget-controller.md)
  — budget controller, floors, `processUnloads` anchor.
- [`062-shared-threading-topology.md`](062-shared-threading-topology.md)
  — shared-pool topology parent (Slice 4 hand-off from 153).
- [`../frame-pipeline-accounting.md`](../frame-pipeline-accounting.md) — law doc.
- `reference/minecraft-1.17.1/src/` — read the counterpart before each item.

## P0 Investigation Log

(Populated during the P0 investigation — findings, vanilla comparison, and the
bound decision land here.)
