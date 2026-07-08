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
| P0 | Unbounded pipeline/light memory under sustained movement | correctness / Quest stability | **highest** | **Fix A landed + verified on real hosts (2026-07-08)** — vanilla-shaped unload eviction; retained light plateaus (bounded by loaded set), acceptance gate flipped, parity byte-identical, wasm32 green; desktop RD20 300-step soak flat + Quest RD5/RD7/churn soak bounded (no OOM) |
| P1 | Movement-frame / 120 Hz over-2x attribution at `7/14` | unblock (promotes shipped win) | high | open |
| P2 | Sky-light graph hot path (fresh-startup ceiling) | new capability | medium (own tactical) | open |
| P3 | Mesh-worker clamp (`7`) rationale + cross-host evidence | anti-knob debt / docs | low | documented below |
| V | Close-condition verification backlog (RD20 memory, wasm32, Quest soak, parity) | evidence | folds into P0/promotions | **closed 2026-07-08** — RD20 300-step soak plateaus + flat RSS, wasm32 green, Quest RD5/RD7/churn soak bounded (no OOM), parity byte-identical; rows in perf-records + quest-records |

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

**Status: Fix A landed 2026-07-08.** Vanilla-shaped unload eviction now bounds
both retained stores: the movement-soak proxy plateaus at a fixed band (`180`
retained chunks, `second_half_growth = 0`) instead of the pre-fix `+9`/step
linear leak, against the same flat `961`-chunk loaded set. Parity byte-identical
(lighting fixtures + a contrasting seed + a re-light-after-eviction test);
`wasm32` green for the touched shared crates. See "P0 Investigation Log —
2026-07-08 Fix A".

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
  evidence for **P0**. **Verified 2026-07-08 (done).** Instrumented
  `scheduler_movement_smoke` (live `retained_light_world_chunks` gauge + per-step
  process RSS + a post-unload `wait_for_light_idle` flush) run at RD20 for `300`
  steps on two seeds. Retained fills to `2520` by step `11` then hard-plateaus
  (`second_half_growth = 0`) while the loaded set stays flat at `4489` and process
  RSS holds a flat ~60 MB band (~`1.5 GB` / ~`1.35 GB` by seed), not monotonic,
  across a 299-chunk traversal. Both seeds trace a byte-identical retained/holder
  trajectory (geometry-driven). Row in
  [`../performance-records.md`](../performance-records.md) (2026-07-08 Tactical
  155 P0 Fix A RD20 soak).
- **wasm32 checks green for every touched shared crate** (`mclone-server`,
  `mclone-light`, `mclone-frame-budget`, `mclone-app-runtime`). A stated 153
  close condition; confirm the `perf-diagnostics` gating + cost-derived grant
  changes still compile for `wasm32-unknown-unknown` and degrade to floors
  (simulation timing is `0` on wasm -> count-cap fallback; expected but
  unverified in the close-out). **Verified 2026-07-08 (done, crates Fix A
  touched).** `cargo check --target wasm32-unknown-unknown -p mclone-server -p
  mclone-light` green (re-confirmed after the harness instrumentation landed). The
  web *worker* light path is a no-op for eviction (rebuilds
  `RetainedInitialLightState` per job frame), so wasm degrades to the same bounded
  behaviour.
- **Quest RD5/RD7/churn + mixed soak** re-run under default config after any P0
  bound lands, to confirm no regression on the constrained platform. **Verified
  2026-07-08 (done, on Quest 3 `2G0YC1ZF93041Z`).** Fix A release APK built +
  installed; RD5 flight, RD5 chunk-view churn, and a `180s` RD7 sustained-flight
  soak (~48 fresh chunks) all ran to completion with `0` skipped frames / `0`
  conservation violations — no crash, no OOM. Memory is bounded: RD7 flight RSS
  holds a `~633–725 MB` band with the `845 MB` fill-time VmHWM never exceeded, and
  churn RSS returns exactly to its pre-churn baseline. RD5 flight is fully green;
  the churn/RD7 frame tails are within streaming-cost expectations, not a Fix A
  regression. Row in
  [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md)
  (2026-07-08 Tactical 155 P0 Fix A Quest soak).
- **Parity** (light byte-equality on fixtures + contrasting seed; mesh
  order-independence) re-checked for any touched execution path. **Verified
  2026-07-08 (done).** `cargo test -p mclone-server -p mclone-light` green
  (`375` + `55`), including the Java-oracle lighting fixtures
  (`provisional_{sky,block}_light_matches_java_*`,
  `generated_ocean_chunk_light_matches_persisted_java_oracle_fixture`), the
  contrasting-seed acceptance soak, and the eviction-parity tests
  (`evicting_a_chunk_column_..._leaves_neighbor_light_unchanged`,
  `relighting_a_chunk_after_eviction_is_byte_identical`).

**Status: closed 2026-07-08 — all four rows verified on real hosts (desktop RD20
+ Quest 3); see the P0 Investigation Log close entry below.**

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

### 2026-07-08 — diagnosis (code + vanilla reference)

**Conclusion: confirmed.** The retained-light block store and the light
engine's `DataLayer` storage grow by roughly `~64 KB + ~64 KB` per *unique chunk
ever visited*, with no eviction path, on both the native worker and the web
backend. It is unbounded by construction under sustained movement; 153 never saw
it because every lane used a fixed-center or short-churn (bounded) chunk set.

**mclone evidence (the accumulation chain):**

- The light worker owns a session-lifetime `RetainedInitialLightState`, created
  once before the receive loop (`light_mailbox.rs:266`, native thread) and once
  per backend on web (`light_mailbox.rs:142,156`). It holds a
  `RetainedLightWorld` (block copy) + a `LevelLightEngine` (light storage).
- Every batch calls `compute_batch` -> `RetainedLightWorld::upsert_chunk`
  (`light_world.rs:66,388`), which inserts a `Vec<RawBlockId>` into
  `chunks: BTreeMap<ChunkPos, Vec<RawBlockId>>` (`light_world.rs:359`).
  `RawBlockId = u8` (`mclone-worldgen/src/block.rs:3`), so each entry is
  `height * 16 * 16` bytes ~= `64 KB` for a 256-high column. There is **no**
  `self.chunks.remove` anywhere in the file.
- The `LevelLightEngine` stores sky + block light per section in
  `BTreeMap<SectionPosKey, DataLayer>` (`mclone-light/src/storage_map.rs:6-17`),
  ~`2 KB` per `DataLayer`, ~`64 KB`/chunk across both maps. Removal primitives
  exist (`remove_layer` `:82`, `remove_top_section` `:149`,
  `remove_marked_sections` `section_storage.rs:200`,
  `update_section_status(section, is_empty=true)` `section_storage.rs:283`) but
  are only ever driven by the *current batch's* changed chunks
  (`light_world.rs:104-113`) — never by unload.
- The unload event exists and is healthy but is disconnected from light: the
  scheduler drops holders in `process_pending_unloads` (`scheduler.rs:2429-2497`)
  and emits `ChunkSchedulerEvent::HolderUnloaded`; the handler
  (`integrated.rs:1514-1524`) frees block/liquid ticks and entities but never
  signals light. And it *cannot*: `LightStatusRequest` has a single variant,
  `ComputeBatch` (`light_mailbox.rs:403-405`) — there is no unload message the
  worker could receive.

**Magnitude.** ~`128 KB` x unique-chunks-visited. A bounded RD10 view (~`625`
chunks) is ~`80 MB` and drains; but free movement across, say, `5000` unique
chunks is ~`640 MB` and never releases. Quest is the RAM-constrained platform,
so it is where this OOMs first — directly relevant to the "didn't regress Quest"
close-out claim, which was never tested with a free-movement soak.

**Vanilla comparison (1.17.1, source-cited).** Read
`reference/minecraft-1.17.1/src/` on 2026-07-08:

- Vanilla bounds the loaded-chunk set with a **ticket-level flood-fill**
  (`DistanceManager.java` `ChunkTicketTracker` 247-289; `MAX_CHUNK_DISTANCE = 44`
  in `ChunkMap.java:98-99`). A `ChunkHolder` exists only while ticket level
  `<= 44`; movement lets the flood-fill drop unreachable chunks.
- Unload releases light through one hook: `ChunkMap.processUnloads` (391-409) ->
  `scheduleUnload` (411-438), which at **line 428** calls
  `lightEngine.updateChunkStatus(pos)`.
- `ThreadedLevelLightEngine.updateChunkStatus` (69-83) then does `retainData(…,
  false)`, `enableLightSources(…, false)`, `queueSectionData(…, null, …)`, and
  `updateSectionStatus(section, true)` per section. Marking empty drives the
  section's tracked level to EMPTY, which lands in `toRemove` and calls
  `DataLayerStorageMap.removeLayer` (= `map.remove`, `DataLayerStorageMap.java:60`)
  on the next light tick — the actual `2 KB` array free.
- **Vanilla keeps no persistent per-chunk block/opacity cache in the light
  engine.** `LayerLightEngine.getStateAndOpacity` (49-104) reads *through* to the
  live chunk via `getChunkForLighting`, with only a fixed 2-entry MRU
  (`CACHE_SIZE = 2`) cleared each batch. If the chunk is unloaded it substitutes
  bedrock/opacity-16. The only persistent light memory is the `DataLayer` arrays,
  and those are evicted per-section on unload as above.

**Divergence read.** `RetainedLightWorld` diverges from vanilla on two counts:
(a) it *is* a persistent per-chunk block cache vanilla deliberately does not keep
(it exists because mclone's light worker runs on a separate thread and needs a
private snapshot to avoid racing the scheduler's chunk store — a *defensible*
worker-thread divergence, unlike (b)); (b) it has no eviction hook analogous to
`updateChunkStatus`/`removeLayer`. **The bug is (b), the missing eviction, not
(a) the copy.** The light-engine storage maps have the same missing-eviction
problem despite already owning the removal primitives.

### 2026-07-08 — empirical measurement

Quantified with a committed movement-soak test,
`native/crates/mclone-server/src/tests/light_memory_soak.rs`
(`movement_soak_shows_unbounded_retained_light_memory`). It steps the view
center in +X for `24` steps at radius `2`, draining worldgen + light to
quiescence and `process_pending_unloads` each step, and records the scheduler's
loaded set (`holder_count`) against the retained-world proxy
(`total_light_status_inserted_chunks` — first-insert-only, never decremented, so
it equals the live retained-world entry count). Per-step trajectory:

| Metric | Step 0 | Step 23 | Behavior |
|---|---:|---:|---|
| `holder_chunks` (scheduler loaded set) | `961` | `961` | **flat** — bounded by tickets, `961 = 31x31` for the whole run |
| `retained_light_chunks` (retained world) | `81` | `288` | **+9.0/step, linear, no plateau** — `3.56x` in 24 steps |

The loaded set is dead flat while the retained light world grows by one movement
strip (`~9` chunks) every step with no eviction. Extrapolating the `+9`/step
slope: `~972` retained chunks by step 100, `~9000` (`~1.1 GB` at ~128 KB/chunk)
by step 1000 — unbounded in movement, exactly as the code/vanilla analysis
predicted. `~36 MB` was already retained after only 24 tiny steps here. This row
was the leak evidence; the same test is now the fix's before/after guard (see
Fix A below).

### Fix options

- **Fix A (recommended, minimal, vanilla-shaped eviction).** Keep the retained
  block snapshot (worker-thread necessity) but evict it on unload, matching
  vanilla's `scheduleUnload` line 428. Add a `LightStatusRequest::Unload`
  (single pos or a batched set), a `RetainedLightWorld::remove_chunk`, and drive
  the *existing* engine primitives (`update_section_status(section, true)` ->
  `remove_marked_sections`/`remove_layer`, `remove_top_section`,
  disable sky sources) for the unloaded chunk's sections. Wire it from the
  existing `HolderUnloaded` event (`integrated.rs:1514`) so light unload rides
  the same ticket-driven drop the scheduler already computes. Lives in the shared
  `mclone-server` light mailbox + `mclone-light` engine (host-neutral, rule F);
  the web backend gets the same message. Parity-neutral: it only frees data for
  chunks no longer loaded, so lighting fixtures + contrasting seed stay
  byte-identical (rule E).
- **Fix B (deeper; gated `062` candidate, not an inevitable end-state).**
  Eliminate the block copy entirely and have the light worker read opacity
  through a shared thread-safe view of the chunk store, exactly like vanilla's
  `getChunkForLighting`. Removes store (a) by touching the worker/scheduler
  ownership boundary. Out of scope for stopping the leak, and two verified facts
  make it *gated* rather than inevitable:
  - **No web generalization.** The web worker rebuilds
    `RetainedInitialLightState` per job frame
    (`compute_light_status_job_frame`) — no retention, no leak. A cross-worker
    `getChunkForLighting` there needs shared Wasm linear memory, the exact
    frontier `068` declined (light lane ~98% work-dominated, so the removed copy
    is ~2% of the lane). Fix B would diverge native/web, not converge them.
  - **No existing hook.** The worker holds no store handle today (the owned
    block copy is *moved* over an `mpsc` channel); the scheduler owns
    `holders: BTreeMap<ChunkPos, ChunkHolder>` by value on the tick thread with
    no `Arc<RwLock>`. Fix B introduces concurrent read access to a single-owner
    store — a new shared-live-state boundary, unlike `062`'s share-nothing
    frame passing.

  The reclaim is native-only and already bounded by Fix A (block copy ≈
  loaded-set × ~64 KB — order ~8-14 MB at Quest RD5-7, ~100 MB+ at desktop
  RD20, i.e. smallest where RAM is tightest). Sequence Fix B only if some other
  driver introduces a shared concurrent-read chunk-store view in `062`; absent
  that, Fix A's defensible worker-thread copy is the stable state.

### Recommendation and next steps

Implement **Fix A** as its own slice (it is an execution-path change, not an
investigation). Sequencing:

1. Land the unload message + `remove_chunk` + engine section-drop; assert (unit
   test) that after unloading a chunk the retained world and both storage maps no
   longer contain it, and that re-lighting a still-loaded neighbor is unchanged.
2. Parity gate: lighting fixtures + contrasting seed byte-identical (rule E);
   `wasm32` check for the touched shared crates (also clears a V item).
3. Validation: the deferred **RD20-long / free-movement soak** watching retained
   world entry count, storage-map section count, `pending_light_publications`,
   and process RSS — must be flat (not monotonic) after the fix (the V RD20
   watch). Re-run Quest RD5/RD7/churn to confirm no regression on the constrained
   platform.

Open sub-question for the implementer: confirm whether the scheduler already has
the unloaded chunk's section list handy at `HolderUnloaded` time, or whether the
light worker must derive sections from its own retained column before dropping it
(the retained world knows the column, so deriving worker-side is viable and keeps
the message a bare `ChunkPos`).

### 2026-07-08 — Fix A landed (implementation)

**Resolution: Fix A implemented and validated.** Vanilla-shaped unload eviction
now bounds both retained stores. The open sub-question resolved in favour of the
bare-`ChunkPos` message: the worker derives the column's light sections from the
retained world's own `min_y`/`height` before dropping the block copy, so the
scheduler ships only positions.

**Change shape (shared crates, host-neutral, rule F):**

- `mclone-light`: new direct-removal primitives mirroring the *end state* of
  vanilla `updateChunkStatus` -> `removeLayer` (the native engine sets section
  levels directly rather than flood-filling a `SectionTracker`, the pre-existing
  037-049 divergence, so eviction removes the layer from **both** the updating and
  visible maps and forgets the section/column tracking directly instead of
  queuing a graph pass). `LayerLightSectionStorage::remove_section` +
  `forget_retained_column`; `SkyLightSectionStorage::remove_section` (also drops
  the section's sky-source bookkeeping) + `forget_column` (drops
  `columns_with_sky_sources` + `top_sections`; `current_lowest_y` is intentionally
  left at its historical floor — safe and, for a uniform-height overworld,
  invariant); `LevelLightEngine::evict_chunk_column(column, light_section_range)`
  ties it together (`retainData(false)` + per-section block/sky removal + sky
  column forget).
- `mclone-server`: `RetainedLightWorld::remove_chunk` drops the `~64 KB` block
  snapshot; `RetainedInitialLightState::evict_chunks` drives it + the engine
  column drop over the light-section range (data range ±`LIGHT_SECTION_PADDING`);
  `retained_chunk_count` is the live gauge. Light mailbox gains
  `LightStatusRequest::Unload(Vec<ChunkPos>)` (rides the same FIFO request channel
  as `ComputeBatch`, so an unload always applies after any earlier compute for the
  chunk and before any later relight) and a `Sync` round-trip barrier so callers
  can read the post-unload gauge deterministically (unloads produce no
  completion). The web inline backend gets the same eviction; the web *worker*
  path is a no-op because it rebuilds `RetainedInitialLightState` per job frame
  (`compute_light_status_job_frame`) and retains nothing across batches — it never
  had the leak.
- Scheduler wiring: `process_pending_unloads` collects the ticket-driven
  `HolderUnloaded` positions and enqueues one `Unload` to the light mailbox
  (gated on `lighting_enabled`), so light eviction rides the exact same
  ticket-driven holder drop the scheduler already computes. A new
  `retained_light_world_chunks` metric surfaces the worker gauge.

**Key-risk resolution (only touches chunks nothing loaded depends on).** The
scheduler only unloads chunks outside the ticket/loaded set, and eviction removes
storage **only** for the evicted column (no neighbour data is read or written).
After a batch drains to quiescence the block/sky graph holds no per-node state
(`computed_levels` is emptied per node as the queue drains), so there are no
dangling nodes to fix up. A still-loaded boundary chunk that reads an evicted
neighbour later sees the same missing-neighbour substitutes as before —
`light_opacity` -> `None` -> opacity `16` (vanilla's bedrock substitute) for the
block world read, and `storing_light_for_section` -> `false` -> level `15`
(fullbright) for the light-data read. Proven by tests, not assumed:
`evicting_a_chunk_column_frees_its_sections_and_leaves_neighbor_light_unchanged`
(mclone-light, byte-identical neighbour light) and
`relighting_a_chunk_after_eviction_is_byte_identical` (mclone-server, revisit
under movement is parity-stable — eviction leaves no stale column state).

**Acceptance gate (the flipped soak, now green).**
`movement_soak_keeps_retained_light_memory_bounded` reads the live
`retained_light_world_chunks` gauge (flushed each step via `wait_for_light_idle`)
instead of the cumulative first-insert proxy. Trajectory, both seeds `12345` and
a contrasting `987654321`:

| Metric | Step 0 | Step 11 | Step 23 | Behavior |
|---|---:|---:|---:|---|
| `holder_chunks` (loaded set) | `961` | `961` | `961` | flat — ticket-bounded |
| `retained_light_chunks` | `81` | `180` | `180` | **fills then plateaus** — `second_half_growth = 0` |

The retained set fills for `loaded_radius - lit_radius` steps (its trailing edge
still inside the ticket set), then plateaus at `180` (one view widened by the
unload lag — a constant bounded by the loaded set, `2.22x` one view here, **not**
a function of distance travelled). The pre-fix run grew `+9`/step to `288` by
step 23 and diverged. Both seeds trace the identical bounded trajectory (retained
membership is geometry-driven, independent of terrain), which the test asserts —
a divergence would mean eviction freed a terrain-dependent set.

**Validation run.** `cargo fmt --check` clean; `cargo test -p mclone-server -p
mclone-light` green (`375` + `55`, including the Java-oracle lighting fixtures and
the new eviction/parity tests); `cargo check --target wasm32-unknown-unknown -p
mclone-server -p mclone-light` green (clears the V wasm32 item for the crates this
slice touched). Fix B (eliminate the block copy via a shared `getChunkForLighting`
view) is a *gated* `062` candidate, not an inevitable end-state — it has no web
generalization (needs the `068`-declined shared linear memory) and no existing
store hook, and its reclaim is native-only and bounded by Fix A (see "Fix
options"). Fix A stops the leak without touching the worker/scheduler ownership
boundary.

**V follow-ups (closed 2026-07-08 — see next entry).** The two host-measurement
items called out here — the desktop RD20-long RSS watch and the on-device Quest
RD5/RD7/churn soak — have now been run on real hardware and confirm the
shared-crate bound holds in practice.

### 2026-07-08 — Fix A verified on real hosts (V backlog closed)

**Resolution: verified.** The desktop RD20 free-movement soak and the on-device
Quest soak both confirm Fix A bounds retained-light memory in practice, with no
regression; all four "V" rows are closed.

- **Desktop RD20 / free-movement + RSS soak (primary P0 evidence).**
  `scheduler_movement_smoke` was extended to surface the live
  `retained_light_world_chunks` gauge and per-step process RSS, and to
  `wait_for_light_idle` after `process_pending_unloads` each step so the
  fire-and-forget unload evictions are observed in the same step (mirrors the
  acceptance gate). A pre-existing harness bug was also fixed so it runs at all
  with lighting enabled: the wait loop now polls first (job dispatch happens in
  `poll()`, not `apply_interest`) and drains to full quiescence
  (`pending_job_count == 0 && pending_publication_count == 0`) instead of exiting
  at zero worldgen jobs and publishing a partial view. Result (RD20, `300` steps,
  seeds `12345` + `987654321`): the loaded set stays flat at `4489`;
  `retained_light_world_chunks` fills to `2520` by step `11`, then hard-plateaus
  (`second_half_growth = 0`) across the remaining 288 steps while the center moves
  `299` chunks; process RSS holds a flat ~60 MB band (~`1.5 GB` / ~`1.35 GB` by
  seed), not monotonic. Both seeds trace a byte-identical retained/holder
  trajectory. This is the RD20 row the 153/155 close-out never measured; it now
  exists in [`../performance-records.md`](../performance-records.md).
- **Quest RD5/RD7/churn + soak (constrained platform, free-movement case).** A
  Fix A release APK was built and installed to Quest 3 `2G0YC1ZF93041Z`. RD5
  flight, RD5 chunk-view churn, and a `180s` RD7 sustained-flight soak (~`48`
  fresh chunks) ran to completion with `0` skipped frames and `0` conservation
  violations — no crash, no OOM. RD7 flight RSS holds a bounded `~633–725 MB` band
  with the `845 MB` initial-fill VmHWM never exceeded; churn RSS returns exactly to
  its pre-churn baseline (`574 MB`) — the eviction-working signature. RD5 flight is
  fully green; the churn/RD7 frame tails are within streaming-cost expectations
  (and the churn tail is inflated by the concurrent memory sampler), not a Fix A
  regression. Row in
  [`../quest-standalone-performance-records.md`](../quest-standalone-performance-records.md).
  This is the free-movement case that was never previously tested on the platform
  where the leak would OOM first.
- **wasm32 + parity (re-confirmed).** `cargo check --target
  wasm32-unknown-unknown -p mclone-server -p mclone-light` green; `cargo test -p
  mclone-server -p mclone-light` green (`375` + `55`, Java-oracle lighting fixtures
  and eviction/relight parity tests included) — re-run after the harness
  instrumentation landed.

With the desktop RD20 and Quest rows recorded, the tactical 155 P0 item and its
"V" close-condition backlog are fully evidenced on real hosts.
