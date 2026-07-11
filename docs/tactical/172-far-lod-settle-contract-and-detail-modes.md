# 172: Far LOD Settle Contract And Detail Modes

Status: active 2026-07-11. Slice 0 (documentation consolidation), Slice 1A
(pull-only exact-set accessors plus the per-chunk ledger), and Slice 1B (first
executable offscreen settle probe with the pinned fly-up repro) are complete.
Slice 1C1 (validated JSON waypoint scripts and the permanent smoke lane) is
also complete. Slice 1C2a (coverage pixels and revisit determinism) is
complete; Slice 1C2b (the movement/toggle/range matrix and full probe lane) is
next. This tactical owns the far-LOD
product-hardening series: a settle-state validation harness, the coverage-gap
correctness burn-down, the user-facing LOD detail modes (auto / 4 / 8 / 16,
debug 1 / 2), and residency/perf polish. It supersedes the remaining open
ends of Tactical 121 — Surface LOD First Slice and pauses Tactical 162 —
Real-Chunk LOD Reduction Draft Slice 4+ until its Slice 1–2 gates hold. Macro
ordering lives in Tactical 171 — Convergence And Parity Closeout.

Topic: `far-lod-settle-contract`

Workstream: native Rust shared runtime/render-session/render/UI plus the
native validation harness. Desktop validation first; behavior must stay shared
across flat, XR, Android, and web.

## Purpose

Tactical 166 landed the mechanism: far LOD is a producer on the shared
resident-tile substrate, with budget families, shared workers, region-arena
draws, and 4/8/16 multi-level rings. What it did not land is a **behavioral
contract** — a precise, machine-checkable statement of what the world must
look like once streaming settles — and the product surface users actually
touch.

Three motivating problems, in priority order:

1. **Verifiability.** LOD slices have been implemented fire-and-forget: agents
   land counters and captures, but nothing asserts "at settle, exactly these
   chunks are drawn, exactly these LOD tiles are visible at exactly these
   levels, and the frame contains no uncovered void." Without that instrument,
   every fix is a guess and every regression is invisible until a human flies
   up and looks.
2. **Correctness.** Real sessions show missing LOD coverage: fly up after
   spawn and concentric blue-void rings appear between the drawn real terrain
   and the LOD bands, and between bands (user captures 2026-07-11). The defect
   ledger below records confirmed mechanisms, found by code inspection, that
   produce exactly this family of artifacts.
3. **Product control.** The only user controls are a Far LOD checkbox and a
   range slider. The detail policy (distance-banded 4/8/16 spacing) is
   hard-coded. Users need an explicit detail mode: `auto` (current bands),
   fixed `4`, `8`, or `16` (one spacing for the whole shell), and later debug
   `1` / `2` (full/half resolution) for visual comparison and debugging.

Performance remains the highest-priority constraint throughout: every slice
carries the 166 perf tripwires, and no fix may trade steady-state frame health
for coverage.

## Current State (2026-07-11)

Landed (do not re-litigate):

- Producer/cache: `mclone-app-runtime/src/far_lod.rs` — event-driven desired
  coverage keyed on `FarTerrainLodBuildKey` (center, render distance, config,
  normal-chunk-set hash), band policy with 2-chunk hysteresis, retained tiles
  via `ResidentTileCache`, builds on the shared render-compile worker pool,
  budgeted by `LodBuildAdmission`/`LodUpload` families.
- Coordinator: `mclone-app-runtime/src/lod_coverage.rs` — per-chunk precedence
  (`normal drawable > reduced real > synthetic > nothing`), replacement
  counters including `suppressed_without_replacement`.
- Renderer: `mclone-render/src/far_lod.rs` — 16×16-chunk region arenas keyed
  by `{region, level}`, fixed 512-vertex/768-index tile slots, per-region
  packed index draws, per-eye and multiview paths.
- Multi-level rings: three equal distance bands over the extra radius map to
  4/8/16-block spacing; level flips are hysteresis-guarded and counted;
  replacement-before-suppress transitions with a 256 double-residency cap.
- Startup prewarm: `StartupLodPrewarmConfig` (default render distance + 5,
  2000 ms cap) drives the same producer before first playable.
- Config/UI: shared `--far-lod` flag (`startup_args.rs`), Options checkbox +
  range slider (1–64 extra chunks), `set_far_lod` full-reset propagation.

Key defaults: base spacing 4, extra radius 12, eviction margin 2 (retain
radius = render distance + extra + 2), retained-patch cap 4096, build floor
4/frame and upload floor 16/frame before the first frame report.

## Defect Ledger

Numbered so slices, fixtures, and commits can reference them. "Confirmed"
means the mechanism is proven by code reading; the Slice 1 harness converts
each to a reproducible red fixture before its fix lands in Slice 2.

### D1 — Real terrain graph-culled from high altitude (primary void source)

When the camera rises above all resident render sections, the occlusion
traversal fallback (`mclone-render/src/chunk.rs::traversal_start_keys`,
`outside_retained_section_start_keys`) seeds the visibility BFS **only from
resident sections at the single highest section layer** (`max_section_y`) that
are in-frustum and traversal-ready. Columns whose tops sit below that layer,
or that are not connected to a seed through resident+ready+visible sections,
are never reached — they are graph-culled and draw nothing, even though they
are loaded, compiled, and directly visible from above. This is a render
correctness bug independent of LOD; LOD makes it obvious because of D2.

### D2 — LOD suppression predicate is a superset of what actually paints

The set that removes columns from the LOD desired set and suppresses LOD
visibility is `traversal_ready_render_section_keys` projected to columns
(`mclone-app-runtime/src/lib.rs:2315`, consumed at
`native_service_assembly.rs::prepare_far_lod_frame` and
`FarTerrainNormalCoverage::covers_chunk`, `far_lod.rs:586`). That set has no
frustum or occlusion component, while drawn terrain is
`traversal_ready ∩ frustum ∩ graph-reachable`. Any column that is
traversal-ready but not painted (D1 makes this common at altitude) has **no
representation at all**: real terrain is culled and no LOD tile exists because
the column was deleted from the desired set. The result is the observed blue
moat/rings. The coordinator cannot even see it: `suppressed_without_replacement`
keys off client-loaded chunks, not painted state.

### D3 — Desired coverage silently truncated at the 4096-patch cap

`capped_far_lod_chunk_positions` (`far_lod.rs:1380`) truncates the
distance-sorted desired set to `max_retained_lod_patches` (≤ 4096). At larger
render distances or large Far LOD Range values the outermost coverage is
silently dropped (e.g. render distance 16 + range 64 caps out near
+19 chunks). The range slider offers 1–64, so the UI can request coverage the
producer silently refuses — a "no silent caps" guardrail violation and a
second void source (hard outer void ring).

### D4 — Prewarm and live band geometry disagree

Prewarm assigns levels using band ends computed from its own config
(`StartupLodPrewarmConfig::far_lod_config()`, extra = 5) while the live path
uses the live config (extra = 12 default). Bands of span 5 vs span 12 assign
different levels to the same chunk, so tiles prewarmed before first playable
either need immediate replacement builds at go-live or — worse — sit inside
the ±2-chunk hysteresis guard and **keep the wrong level indefinitely** (e.g.
distance rd+3..rd+4 prewarmed at level 2 stays level 2 in a live level-1
band). Mass go-live transitions also stack against D7.

### D5 — Hysteresis sticky zone never converges for static tiles

`far_lod_stabilized_level` (`far_lod.rs:1317`) keeps the previous level while
the tile sits within the previous band ± 2 chunks. A tile that ends up (via
D4, or a render-distance/range change) at a level whose band it sits 1–2
chunks outside will keep that level forever while stationary. Hysteresis
should damp oscillation during movement, not block convergence at rest.

### D6 — Cross-level seam cracks at band boundaries

Tile edges resolve against neighbors via single-point sampling plus one-sided
drop faces (`append_far_terrain_lod_chunk_patch`, `neighbor_lod_cell`); level
changes dirty only the four cardinal neighbors (`update_target`,
`far_lod.rs:1018`), not diagonals. Where a finer and coarser tile disagree on
the shared-edge height profile and neither emits a covering skirt, thin
vertical cracks show sky exactly along the concentric band boundaries,
compounding D1/D2 visually.

### D7 — Replacement head-of-line blocking under the double-residency cap

`submit_builds` (`far_lod.rs:1047`) handles a replacement build that exceeds
`MAX_FAR_TERRAIN_LOD_DOUBLE_RESIDENT_TILES` by pushing it back and **breaking
the whole admission loop**. Because pending builds are nearest-first, a wall
of level transitions (e.g. D4's go-live storm, or a band shift during flight)
stalls all fresh-coverage builds queued behind it for as long as the cap is
saturated. Fresh coverage should not starve behind blocked replacements.

### D8 — Churn discards under band/readiness movement (efficiency, bounded)

Completed builds are discarded when the desired level or neighbor spacings
changed in flight (`accept_completed` retain gate, `far_lod.rs:1122`), and
fresh uploads are discarded when desired changed during upload latency
(`drain_render_uploads`, `far_lod.rs:828`). Both are correct staleness
handling, but under sustained movement they can waste a large fraction of the
build budget (`stale_builds`). Needs measurement and bounding, not removal.

## Locked Settle Contract

These invariants define "correct" for this tactical and become permanent
harness assertions. **Settle** means: config stable, camera stationary, and
`pending_stream_work == 0` (which already includes far-LOD pending/inflight
builds and queued uploads) sustained for the stability window
(`STREAM_STABLE_FRAMES`).

- **C1 — Deterministic desired set.** The desired LOD tile set (chunk → level)
  is a pure function of (center chunk, render distance, far-LOD config,
  suppression set). The harness recomputes it independently and asserts
  equality with the producer's view.
- **C2 — Coverage completeness.** At settle, every column within the far-LOD
  outer radius is painted: it has drawn real-section geometry or a visible
  LOD tile. From an unoccluded viewpoint (top-down fixture), no in-coverage
  column may render as sky.
- **C3 — Set coherence.** At settle: every desired tile is resident, uploaded,
  and visible unless suppressed; nothing is pending, inflight, or queued;
  `visible ∪ suppressed == desired`; per-level visible counts match the band
  function (auto) or are single-level (fixed modes).
- **C4 — View-independent suppression.** The suppression predicate must be a
  view-independent, painted-capable property of the column (its real sections
  are uploaded and drawable), never a frustum- or traversal-dependent set.
  Suppressing a column that is not painted-capable is a defect (this is D2's
  contract form).
- **C5 — Replacement before suppress.** Across any move→settle cycle,
  `suppressed_without_replacement` delta is 0, and the painted-coverage probe
  (C2) holds at every settle point, not just the first.
- **C6 — No silent caps.** Any cap or denial that reduces coverage (patch cap,
  double-residency cap, budget denial, region slot exhaustion) must surface as
  a counter/skip reason, and user-facing controls must not offer ranges the
  producer will silently refuse.
- **C7 — Off is off.** `--far-lod false` remains byte-identical (the 166
  canary capture), and LOD work must never tax the default path.
- **C8 — Bounded steady state.** At settle, CPU/GPU resident tile counts and
  bytes are within declared caps; during steady movement, per-chunk-step LOD
  work is O(ring), never O(area).

## Proposed Sequence

Every slice carries the 166 regression tripwires (workspace tests, offscreen
smoke with inspected capture, far-LOD-off canary, desktop startup-streaming /
movement / timedemo within spread; Quest lanes for any slice touching upload,
admission, or the frame path) plus its own gate. A failed gate blocks the next
slice.

### Slice 1: Settle-state validation harness

The instrument comes first, and it lands red: the fly-up fixture must
reproduce D1/D2 before any fix exists.

#### Slice 1A — Pull-only exact sets and ledger (complete 2026-07-11)

- **Exact-set accessors landed.** `FarTerrainLodSettleSnapshot` exposes desired
  levels plus resident, uploaded, published, visible, pending, inflight, queued
  upload, and queued removal tile identities. `FarLodRuntimeSettleSnapshot`
  joins loaded chunks, traversal-ready sections, and the coordinator's actual
  suppression set across native and web runtime services. The renderer
  recomputes exact drawn real sections on explicit pull from its cached culling
  records; normal frame summaries remain count-only and retain no duplicate
  diagnostic state.
- **Per-chunk ledger landed.** `McloneSceneHost::mono_far_lod_settle_snapshot`
  exposes a deterministic debug dump keyed by chunk:
  `{loaded, traversal_ready, painted (drawn sections), lod_desired_level,
  lod_resident_levels, lod_visible_levels, suppressed,
  lod_pending_levels/lod_inflight_levels}` — plus uploaded/published/queued
  lifecycle levels. The snapshot directly classifies
  `culled_but_suppressed_chunks`, the D1/D2 failure signature that later probe
  reports emit on assertion failure and settle timeout.
- **Focused evidence:** app-runtime, render-session, scene, and renderer unit
  suites green (`224 + 110 + 94 + 121` passed; two renderer GPU tests remain
  intentionally ignored); direct `wasm32-unknown-unknown` web-client check
  green. This subsection changes no pixels or frame-path behavior, so no
  capture lane was required.

#### Slice 1B — First executable settle probe (complete 2026-07-11)

- **Executable mode landed.** `mclone-native-client --lod-settle-probe
  /tmp/mclone-lod-settle` runs on `OffscreenFlatClientHost` /
  `OffscreenSceneDriver`, reusing `pending_stream_work` plus
  `STREAM_STABLE_FRAMES`. It repeats the stable gate at full capture size after
  the normal 1×1 warmup so the first real frustum cannot reveal late work.
  Timeouts include pending work, the budget/skip panel, and the Slice 1A
  ledger.
- **Two fixed fixtures landed.** Both use seed 12345, RD4, range 6, frozen noon,
  section occlusion on, and 960×960 captures. Spawn-settle is the green
  set-coherence fixture; fly-up-high is the expected-red D1/D2 pin. The mode
  exits successfully only when both outcomes match their expectations.
- **Classifier tightened from first-draw evidence.** The renderer now exposes
  exact uploaded+traversal-ready sections inside the frustum as well as drawn
  sections on diagnostic pull. `culled_but_suppressed_chunks` therefore means
  a paintable in-frustum normal column was wholly graph-culled while still
  suppressing LOD; ordinary off-screen columns no longer count.
- **Pinned evidence:** spawn settled with `pending=0`, 360 desired/visible
  tiles, and no set-coherence failures. The high fixture settled with
  `pending=0`, 81 paintable-frustum normal columns, 78 painted columns, and
  exactly 3 D1/D2 culled-but-suppressed columns. It reported 467 frustum real
  sections, 173 drawn, and 294 graph-culled. Structured stdout plus
  `lod-settle-report.json`, `spawn-settle.png`, and `fly-up-high.png` were
  written under `/tmp/mclone-lod-settle-1b`; both PNGs were inspected.
- **Regression evidence:** native-client 133, renderer 126, and scene 94 tests
  passed (two intentional renderer GPU ignores); direct web/WASM check passed;
  `native:desktop-offscreen:smoke` passed and its far-LOD-off capture was
  inspected unchanged.

#### Slice 1C1 — Scripted smoke lane (complete 2026-07-11)

- **Validated script contract landed.** `--lod-settle-script <path.json>`
  accepts schema-1 scripts containing any 1–64 ordered `spawn-surface` or
  explicit eye/target waypoints. Schema version, unknown fields, safe unique
  names, finite coordinates, nondegenerate look vectors, and pinned `pass` /
  `fail-d1-d2` expectations are rejected before GPU startup. Omitting the flag
  preserves the built-in Slice 1B pair.
- **Arbitrary waypoint runner landed.** Every waypoint commits its camera,
  repeats both the 1×1 and full-output stable gates, captures a name-derived
  PNG, and emits its settle/render/set/ledger evidence. If the capture frame
  itself exposes follow-up streaming work, the runner settles and redraws
  before accepting it; this was exercised by the first permanent-lane run and
  closes with `pendingStreamWork=0`.
- **Permanent fast lane landed.** The checked-in
  `test/fixtures/far-lod/settle-smoke.json` script backs
  `pnpm native:lod-settle:smoke`, now listed in `docs/platforms.md`. Its RD4 /
  range-6 spawn set-coherence fixture matches `pass`; the altitude fixture
  matches the pinned `fail:d1-d2` outcome with 81 paintable columns, 78
  painted, and three culled-but-suppressed gaps. Both 960×960 PNGs under
  `/tmp/mclone-lod-settle-smoke` were inspected. Native-client tests are green
  (`137` passed).

#### Slice 1C2a — Image and revisit assertions (complete 2026-07-11)

- **Schema-2 assertions landed.** Optional `coverageProbe.planeY` and
  `matchesPixels` fields extend the validated waypoint contract; schema-1
  scripts remain accepted. Coverage is restricted to explicit near-vertical
  look-down cameras. Pixel references must name an earlier waypoint with the
  identical camera pose.
- **Coverage image probe (C2) landed.** For a top-down waypoint, the runner
  reverse-projects every capture pixel onto the configured world plane, masks
  it by the exact loaded-or-LOD-desired chunk set, and asserts that no masked
  pixel equals the transformed sky clear RGBA. This footprint test is stronger
  than the initially proposed center sample and directly detects small voids.
- **Revisit determinism landed.** `matchesPixels` performs exact whole-frame
  RGBA comparison (`count_pixel_mismatches`) after settle, without golden
  files. The checked-in smoke is now a four-waypoint high→spawn→high revisit
  around the original spawn/high pair.
- **Pinned evidence.** Both high captures report 113 projected columns and
  the same 165 sky pixels across seven coverage columns, while retaining the
  three-column D1/D2 ledger signature. The revisit differs by zero pixels from
  the first high capture. All four fixtures matched, closed with zero pending
  work, and their 960×960 PNGs were inspected. Focused tests cover schema-1
  compatibility, schema-2 reference validation, coverage masking, and
  pixel-granular mismatch counting.

#### Slice 1C2b — Movement/toggle/range matrix and full lane (next)

- **Fixtures.** Small/fast first: RD4 + range 6 spawn-settle; fly-up-high
  settle (the user repro, expected-fail pinning D1/D2); one-chunk-step move;
  8-chunk move; band-crossing walk; far-LOD toggle off/on; range change.
- **Full pnpm lane.** Add `native:lod-settle:probe` for the complete matrix;
  keep the landed `native:lod-settle:smoke` as its CI-suitable subset.

Gate:

- Harness runs green on fixtures the current build genuinely satisfies;
  the fly-up fixture is red and its failure report names the D1/D2 mechanism
  (culled-but-suppressed columns present in the ledger);
- accessors add no per-frame cost when unused (debug/probe pull only);
- far-LOD-off canary unchanged; existing suites green.

**Agent verification protocol (permanent, starts here):** any subsequent
change to far-LOD behavior — this tactical or later — must run
`native:lod-settle:smoke` and include the per-fixture results in its
validation evidence. A red fixture is only acceptable when the slice
explicitly lands it as the pinned repro for a ledger defect. This is the
antidote to fire-and-forget LOD work.

### Slice 2: Coverage-gap correctness burn-down

Drive every ledger defect to fixed-or-explicitly-deferred, each with its
fixture flipping red→green in the same change.

- **D1:** fix the outside-retained traversal seeding so a camera above the
  world seeds from the visible top surface per column (or disables graph cull
  when no valid seed layer covers the frustum), restoring drawn real terrain
  from altitude. This is a render fix with value independent of LOD; validate
  with a far-LOD-off altitude fixture too.
- **D2 / C4:** change the suppression input from traversal-ready columns to a
  view-independent painted-capable predicate (column has uploaded, drawable
  real sections), applied consistently to desired-set carving and the
  coordinator's `normal_drawable`. Decide and record whether covered columns
  stay out of the desired set (today's shape, cheapest) or become
  built-but-suppressed tiles; the contract only requires that suppression
  implies painted-capable. Extend diagnostics so a painted-coverage gap is
  countable (C2 as a counter, not only a probe assertion).
- **D3 / C6:** make the cap honest — surface a truncation counter/skip
  reason, clamp the UI range to what the cap can serve for the active render
  distance, and raise `MAX_FAR_TERRAIN_LOD_RETAINED_PATCHES` if the supported
  slider maximum requires it (recording the memory consequence).
- **D4:** prewarm assigns levels with the live band geometry (prewarm limits
  radius, never re-bands), eliminating go-live transition storms.
- **D5:** let stationary tiles converge: hysteresis yields to the raw level
  when the desired level has been stably different for N recomputes (or an
  equivalent rest rule); keep the oscillation tests green.
- **D6:** close band-boundary cracks (skirt/drop-face coverage for coarser
  neighbors, include diagonal neighbor dirtying if required); validate with a
  band-boundary capture fixture and the C2 probe along boundary rings.
- **D7:** replacement admission skips (defers) capped replacements instead of
  breaking the loop, so fresh coverage keeps building; add a skip-reason
  counter.
- **D8:** measure `stale_builds` share under the movement fixtures; bound it
  (e.g. re-check desired level at submit time; tolerate benign neighbor-spacing
  mismatches via seam-signature requeue only for edge tiles) if it exceeds a
  recorded threshold.

Gate:

- All Slice 1 fixtures green, including fly-up (C2 holds from altitude);
- `suppressed_without_replacement` and the new painted-gap counter stay 0
  across the movement fixtures;
- 166 perf tripwires within spread, including Quest orbit far-LOD-on lanes
  (D1 touches the real render path — treat as frame-path change);
- no new scheduler/queue types; budget families and ordering only.

### Slice 3: LOD detail modes (auto / 4 / 8 / 16)

Work:

- `FarLodDetailMode { Auto, Fixed { spacing_blocks } }` on
  `FarTerrainLodConfig`, folded into `FarTerrainLodSourceKey` so mode changes
  reset cleanly. Fixed mode: single level 1 at base spacing = 4/8/16; band
  ends, hysteresis, and level history bypassed (no transitions exist).
- Shared CLI: `--far-lod-detail auto|4|8|16` following the
  `--render-distance` value-arg pattern in `startup_args.rs` (all four
  platforms inherit). Debug values 1|2 parse but are rejected until Slice 5.
- Options UI: an "LOD Detail" cycle row (the Frame Pacing / FPS Cap `Cycle`
  widget precedent) next to the Far LOD checkbox, with the established
  action→effect→session→ui_panels plumbing chain.
- Harness fixtures per mode: C1/C3 with the expected single-level sets, C2
  probe, one movement fixture per mode; a runtime mode-switch fixture
  asserting a bounded clean reset (resident set converges to the new mode's
  desired set at settle, no leaked tiles or GPU regions).
- Renderer: no changes required — spacing ≥ 4 fits the existing 512/768 tile
  slots.

Notes: fixed-16 is the "whole chunk" mode (one sample per chunk, the
chunk-granular ceiling from 166). No options persistence exists in the engine;
like every other option, the mode resets each launch — persistence is a
separate concern, explicitly out of scope (see Non-Goals).

Gate: per-mode fixtures green; mode switching leak-free; far-LOD-off canary;
perf smokes within spread (fixed-16 should measurably lower build cost;
record it).

### Slice 4: Residency, memory, and movement-perf polish

Work:

- **Revisit retention.** Today tiles past retain radius (outer + 2) are fully
  evicted; backtracking rebuilds them. Keep evicted-from-desired tiles
  resident (CPU metadata + GPU slot) up to an explicit byte/tile budget with
  farthest-first eviction, so A→B→A needs index repacks, not rebuilds.
  Platform-tunable budget (Quest conservative); budget and evictions visible
  as counters (C6).
- **Movement cost bounds.** Harness movement fixtures assert per-chunk-step
  bounded builds/uploads (O(ring), C8) and bounded desired-set recompute
  frequency; verify the normal-set hash only changes when the painted-capable
  set changes (post-D2 it is no longer camera-noise-driven).
- **Region hygiene.** Empty-region reclamation, per-level region counts and
  arena slack in stats; confirm no unbounded region growth across long
  movement fixtures.
- **Queue-age trickle.** Assert the existing adaptive rule end-to-end: under
  healthy frames, queue age must not grow without bound (starvation fixture
  with a saturated compile queue).

Gate: A→B→A fixture shows ~0 rebuilds on return; steady-state resident bytes
within declared budget on desktop and Quest capture; movement/timedemo/Quest
lanes within spread.

### Slice 5: Debug detail modes 1 and 2

Work:

- Per-spacing tile slot capacity in the region arena (worst-case cells ×
  quads for the tile's spacing), or smaller regions (e.g. 8×8) for spacing
  ≤ 2, so slots stop assuming spacing ≥ 4 (`upload_tile` asserts today).
  Consider the 166 packed-vertex open question (u16 region-relative + rgba8)
  if memory pressure warrants.
- Producer/mesher already accept spacing 1–2 (`normalized()` clamps 1..16);
  extend the mode plumbing to accept 1|2 behind a debug gate (CLI accepted,
  UI shows them as debug entries or hides them outside a debug build/flag).
- Clamp the effective radius in debug modes to a recorded memory budget with
  an honest counter/UI hint (spacing-1 tiles are ~10× a spacing-4 tile's
  geometry; unbounded radius is not a goal — these modes exist for visual
  debugging and near-field comparison).
- Fixtures at RD2 + small range for both modes; C2 probe; no assert panics;
  memory counters within the declared debug budget.

Gate: spacing-1/2 fixtures green at bounded radius; default-mode and
far-LOD-off behavior unchanged; Quest explicitly out of scope for debug modes
(documented), desktop-only validation acceptable here.

### Slice 6: Platform re-baseline and closeout

Work: re-run the 166 perf lane matrix with the shipped defaults (auto mode)
and record fixed-mode deltas in `docs/performance-records.md` /
`docs/quest-standalone-performance-records.md`; re-pin baselines only as an
explicit recorded decision; update `docs/topics/far-lod.md` status; run the
web far-LOD probes (which should adopt the C3 set-coherence predicate where
the report exposes it); hand the LOD thread back to 162 Slice 4 (reduced-real
producer) via the 171 ledger.

Gate: matrix recorded; 171 ledger updated; 162 unpaused with its Slice 4
landing on a harness-validated substrate.

## Non-Goals

- Reduced-real LOD tiles, edit dirtying, LOD persistence — Tactical 162
  Slices 4–6, resuming after Slice 2 here (ordering via 171).
- Options persistence (no engine-wide preferences store exists; introducing
  one is its own tactical, not a LOD concern).
- Multi-chunk tiles / band-count redesign — 166's locked chunk-granular
  invariant and 4/8/16 shape stand; this tactical adds modes over that shape,
  not new tile geometry classes.
- Distant Horizons-style persistence or protocol work
  (`docs/lod-architecture.md` owns the long-range outlook).

## Guardrails

All 166 guardrails carry forward verbatim (one budget owner, no new
scheduler/queue types, one worker pool, chunk-granular tiles, substrate owns
admission/producers own representation, multiview parity, replacement-before-
suppress, no silent caps, far-LOD-off byte-identical, real sections never
regress). Additions:

- The settle contract C1–C8 is the acceptance bar for every LOD change from
  Slice 1 onward; a fix without its fixture does not land.
- Suppression must satisfy C4 (view-independent, painted-capable) — never
  reintroduce a frustum- or traversal-dependent suppression input.
- Debug modes must not weaken product defaults: mode plumbing may not add
  cost or behavior change when `Auto` is selected.
- Harness fixtures are shared vocabulary: new LOD work adds fixtures rather
  than bespoke one-off validation scripts.

## Validation Commands

- `pnpm native:lod-settle:smoke` — fast fixture subset (Slice 1+; the
  permanent pre-merge check for LOD changes).
- `pnpm native:lod-settle:probe` — full waypoint/fixture matrix with captures
  (lands in Slice 1C2b).
- Existing tripwires: `pnpm native:desktop-offscreen:smoke`,
  `native:movement:smoke`, `native:timedemo:smoke`,
  `native:startup-streaming:perf` (+RD15), `native:frame-budget:perf`,
  `native:web:build` / `native:web:smoke` + far-LOD web probes, Quest
  `native:android-xr:perf:orbit:*:metrics` per `docs/platforms.md`.

## Relationship To Other Tacticals

- [`121`](121-surface-lod-first-slice.md): closed by this tactical; its
  remaining follow-up slices are absorbed (C/D landed via 166 evidence, E
  deferred to 162 Slice 6).
- [`162`](162-real-chunk-lod-reduction-draft.md): paused at Slice 4; resumes
  after Slice 2 here so the reduced-real producer lands on a validated,
  instrumented substrate.
- [`166`](166-shared-resident-tile-substrate.md): complete; owns the substrate
  mechanism this tactical hardens. Its tripwires and locked invariants are
  incorporated by reference.
- [`171`](171-convergence-and-parity-closeout.md): owns macro ordering; its
  thread ledger points the LOD thread here until Slice 6 hands back to 162.
- [`128`](128-terrain-render-pipeline-coordination.md) /
  [`150`](150-adaptive-frame-budget-controller.md): vocabulary and budget
  owner; unchanged.

## Open Questions

- D2 shape: carve covered columns out of the desired set (today) vs
  build-but-suppress. Default position is keep carving (cheapest, no Quest
  overdraw risk); revisit only if the harness shows reveal-latency pops on
  chunk unload.
- Retention budget defaults per platform (Slice 4) — measure before choosing.
- Whether fixed-16 should unlock a larger range slider maximum once D3's
  honest cap lands (cheap tiles, coarse shell — the likely "see very far"
  product mode).
- Debug-mode gating: hidden behind a debug flag vs always-visible cycle
  entries labeled "debug".
