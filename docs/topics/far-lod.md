# Far LOD

Topic: `far-lod-settle-contract`

Living status for the synthetic far-terrain LOD system: the coarse,
non-authoritative surface shell drawn outside normal render distance.

Last reconciled: 2026-07-11 (first moving-coverage class fixed; tactical active).

## Coverage Correctness Goal

Goal opened from interactive evidence on 2026-07-11: eliminate
persistent chunk-aligned and annular holes anywhere inside configured terrain
coverage after streaming settles, then bound the same failure during sustained
movement. Preserve the release-active invariant that real and synthetic
terrain never render for the same chunk in one frame.

The acceptance contract is representation-based rather than queue-based. For
every expected in-range column, a settled frame must select exactly one
painted representation:

```text
painted real terrain XOR visible far LOD
```

`pending_stream_work == 0`, desired/resident count equality, and an uploaded
tile are supporting lifecycle facts; none proves that a column painted. A
missing column must be classified at its first failing frame as undesired,
pending, inflight, resident-only, upload-queued, unpublished, or suppressed
without painted real replacement. Fix the owner named by that state instead
of adding another independent ticket or scheduler.

Architecture direction:

- keep normal terrain authoritative and Far LOD presentation-only;
- keep the shared compile worker/admission substrate and bounded resident-tile
  lifecycle;
- make sparse native section traversal preserve Minecraft's full-height
  `ViewArea` visibility semantics;
- base real-over-LOD arbitration on one explicit painted-capable contract, not
  a convenient loaded/readiness proxy;
- keep the synthetic target as a static distance shell independent of real-
  terrain loading/readiness, with only bounded handoff and movement guards;
- defer reduced-real/persisted sources until this pure-synthetic path is stable;
- validate a stationary teleport first, then a deterministic smooth path that
  records missing-column count and age without inventing a parallel movement
  or generation system.

First evidence under the active goal: the existing high-view fixture reached
zero pending work while its color probe reported 165 pixels equal to the sky
clear color across seven coverage columns. Interactive review established that
color equality is not a trustworthy hole oracle—water and other rendered
surfaces can be visually or numerically confounded with the clear color. The
settle executable had also overwritten parsed detail and the temporary
covered-build-culling diagnostic, and its coherence check incorrectly required
prebuilt-but-suppressed tiles to be visible. Those are harness contract defects,
not reasons to expand its schema. A renderer trace then found the concrete D1
mechanism: at Y=200 the sparse cache retained the empty camera section and
treated it as the sole traversal seed. Switching empty sparse camera sections
to per-column outermost visible drawable seeds changed the high ledger from
78/81 to 81/81 painted columns and removed all three
culled-but-suppressed columns. The later smooth lane uses explicit depth and
lifecycle evidence to classify and bound genuine missing representations.

The explicit oracle is now GPU depth readback, sampled at the exact expected
surface of the representation that owns each chunk: generated block-top
surfaces for real terrain and the sampled flat-cell height for synthetic LOD.
This deliberately does not use screenshot interpretation or RGB equality. A
full-height top-down run projects all 441 chunks in the RD4 + range-6 square;
the five settled movement positions, through an eight-chunk relocation, each
reported zero clear-depth representation samples after correcting the earlier
flat-Y attribution error.

Sustained movement had an architectural starvation path: new edge coverage
could sit behind a saturated wall of level replacements. D7 is now fixed:
bounded admission rotates blocked
replacement requests and continues scanning for fresh missing tiles, while
retaining the double-residency cap and replacement-before-suppress behavior.
That smooth path now exists. Before the fix it found the first desired-but-
pending missing LOD edge at frame 9 and then failed for all 331 remaining
frames; 201 chunks were absent at movement end and 122 were still absent after
the two-second tail. The shipped stable-shell policy now passes the same
48-block/s path for all 340 frames with zero missing chunks and zero maximum
missing age. This closes the reproduced streaming-throughput class; the wider
tactical remains active for silent range caps, seams, and performance closeout.

## Current State

The deterministic stationary and moving coverage cases are behaviorally
proven. Tactical 166 landed far LOD as a
producer on the shared resident-tile substrate: event-driven desired coverage,
builds on the shared render-compile worker pool under `LodBuildAdmission` /
`LodUpload` budget families, 16×16-chunk region arenas with per-region packed
index draws, per-eye + multiview paths, and 4/8/16-block multi-level rings
with 2-chunk hysteresis and replacement-before-suppress transitions. Proven on
desktop offscreen, Quest 3, and all three production browser lanes.

The motivating real sessions showed coverage gaps. Flying up after spawn showed
concentric blue-void rings between drawn real terrain and the LOD bands (user
captures 2026-07-11), and later movement testing captured large rectangular
empty areas among otherwise complete real and synthetic terrain. Confirmed
mechanisms are ledgered as D1–D8 in
[tactical 172](../tactical/172-far-lod-settle-contract-and-detail-modes.md);
the two load-bearing ones:

- **D1 (fixed in the active coverage goal):** a retained but empty camera
  section was accepted as the sole occlusion seed even when the sparse native
  graph had no continuous path down to every visible surface. Empty sparse
  camera records now fall back to the outermost visible drawable section in
  each column (`mclone-render/src/chunk.rs::traversal_start_keys`).
- **D2:** LOD suppression uses traversal-ready columns — a strict superset of
  what actually paints — so culled-but-ready columns get neither real terrain
  nor a LOD tile, and `suppressed_without_replacement` cannot see it.

The settle-state instrument has its data foundation and executable GPU probe.
Tactical 172 Slice 1A exposes exact desired/resident/uploaded/
published/visible/pending/inflight/queued LOD sets, loaded/readiness/
suppression sets, and render-view painted real columns through
`McloneSceneHost`, joined into a deterministic per-chunk ledger. Slice 1B adds
`--lod-settle-probe <directory>` with fixed RD4/range-6 spawn and high-view
fixtures, full-size stable gating, PNG captures, structured JSON, timeout
budget/ledger diagnostics, and expectation matching. The high fixture
originally pinned 81 paintable-frustum columns versus 78 painted and exactly
three in-frustum, graph-culled columns; D1 later flipped it green at 81/81.
This diagnostic path adds no normal per-frame state or work. Slice 1C1 adds
waypoint scripts, arbitrary ordered eye/target execution, capture-frame
resettling, and the permanent `native:lod-settle:smoke` lane backed by a
checked-in fixture. Slice 1C2a extends that fixture to four waypoints with a
schema-2 projected-footprint C2 probe and exact high→spawn→high pixel revisit.
Slice 1C2b1 adds the checked-in movement matrix with explicit desired-level
assertions. Slice 1C2b2 adds toggle/range mutations and the full probe lane,
completing the 14-waypoint Slice 1 harness;
Slice 2 next burns down the defect ledger before detail modes and residency
polish.

Slice 2 began with a temporary covered-build-culling A/B control rather than
more harness expansion. It helped isolate the original fly-up suppression
failure, but readiness-driven target carving is not a sound product policy.
The Graphics row, startup argument, shared config field, and probe-report field
have now been removed.

The persistent moving gap was then reproduced without screenshot
interpretation. The existing movement fixture is interpolated at 60 Hz and
48 blocks/s; depth is sampled at the expected representation surface over the
entire configured square every frame. Both culling-on and culling-off pre-fix
runs classified missing edge chunks as desired and pending rather than
render-culled. The fix uses the producer's existing retention margin as hidden
movement-guard coverage, prioritizes missing presentation before guard and
replacement work, prioritizes outer guards toward the direction of travel,
reuses bounded compressed surface facts per compile worker, and lazily
guarantees two shared compile workers/eight pending slots only when far LOD is
enabled. The current target is a pure-synthetic static shell: one drawable
handoff ring, one hidden inner guard, and two hidden outer guards.
Guard tiles never enter presentation arbitration until desired, and no target
membership depends on real-terrain readiness. Final real-over-LOD suppression
and the release-active overlap panic are unchanged.

That distinction is now a hard invariant. Interactive fixed-detail testing
captured coarse green LOD caps rendering over textured real terrain with
z-fighting. Final visible LOD tiles are now asserted disjoint from traversal-
ready real chunks—a superset of chunks eligible for a real draw—in both local
and remote runtime services. Any overlap panics in release builds with the
chunk coordinate. The build-culling option can no longer bypass render-time
suppression. The active coverage goal fixed D1 without weakening this rule;
D2 arbitration and any later boundary work may never trade a void for
co-rendered representations.

At user direction, the Slice 3 product chooser landed ahead of those remaining
fixes so it can be evaluated interactively. Graphics now cycles **LOD Detail**
through Auto, 4 blocks, 8 blocks, and 16 blocks; the shared startup equivalent
is `--far-lod-detail auto|4|8|16`. Auto preserves the existing three distance
bands. Each fixed mode uses one level and one spacing over the whole shell,
bypassing band hysteresis. Switching clears retained LOD and rebuilds from a
mode-specific source identity.

## Ownership

- Producer/cache, band policy, coordinator, prewarm:
  `native/crates/mclone-app-runtime/src/far_lod.rs`, `lod_coverage.rs`;
  per-frame orchestration in `native_service_assembly.rs`
  (`prepare_far_lod_frame`).
- Substrate (resident tiles, upload admission):
  `native/crates/mclone-render-session/src/resident_tile.rs`.
- Region arenas, draw pipelines: `native/crates/mclone-render/src/far_lod.rs`.
- Budget families: `mclone-frame-budget` panel via
  `mclone-scene/src/render_admission.rs` (`lod_grant`).
- Config/CLI/UI: `FarTerrainLodConfig` (`far_lod.rs`), shared `--far-lod`
  / `--far-lod-detail` (`startup_args.rs`), Options Far LOD, detail, and range controls
  (`mclone-ui`).

## Contract

Real chunks stay authoritative; LOD never satisfies chunk interest, collision,
raycast, edits, or gameplay. Chunk-granular tiles only (166 locked invariant;
16-block spacing is the per-chunk ceiling). One budget owner, one worker pool,
no whole-world rebuild/upload paths. Far-LOD-off is byte-identical. The settle
contract (tactical 172, C1–C9) is the behavioral acceptance bar: deterministic
desired set, painted coverage completeness at settle from any camera pose,
set coherence, view-independent suppression, replacement-before-suppress,
no silent caps, bounded steady state, and release-active real/LOD mutual
exclusion.

## Validation

- Existing: `far_lod`/`lod_coverage` unit suites, offscreen far-LOD captures
  (RD4, seed 12345, playable + idle), web far-LOD probes
  (`native:web:far-lod-*-smoke`), Quest orbit far-LOD on/off metrics lanes.
- Landed (172 Slice 1A): pull-only exact lifecycle/runtime/painted sets and the
  per-chunk D1/D2-classifying ledger; focused shared unit suites and direct web
  WASM check green.
- Landed (172 Slice 1B): direct `--lod-settle-probe <directory>` fixed
  spawn/fly-up executable probe; spawn set coherence green, fly-up D1/D2 pin
  red with three paintable-frustum gaps; JSON and inspected PNG evidence under
  `/tmp`.
- Landed (172 Slice 1C1): validated `--lod-settle-script` schema, arbitrary
  ordered waypoint runner, checked-in smoke script, and permanent
  `native:lod-settle:smoke`. Spawn set coherence and the pinned high-view
  three-column D1/D2 result match with zero pending work; both captures were
  inspected.
- Landed (172 Slice 1C2a): schema-2 `coverageProbe` / `matchesPixels`
  fields with schema-1 compatibility. The A→B→A exact-pixel revisit remains a
  determinism check. The original clear-color-equals-gap assertion is retired:
  its 165 matching pixels can include rendered water/surface color and do not
  prove missing geometry. Every waypoint still closes with zero pending work.
- Landed (172 Slice 1C2b1): schema-3 desired-level assertions plus a
  five-waypoint movement fixture. The anchor remains level 3 across the
  one-chunk/hysteresis guard and flips to level 1 at crossing. After the eight-
  chunk move the old anchor is correctly outside the shell while a new level-2
  anchor enters. Every waypoint currently settles at 392 desired, 360 visible,
  and 81 real-suppressed with zero pending work.
- Landed (172 Slice 1C2b2): schema-4 shared-policy toggle/range mutations,
  exact disabled teardown and enabled/range repopulation assertions, and
  `native:lod-settle:probe`. The full lane runs 14 waypoints across three
  scripts; all matched with zero pending work. Every far-LOD change must run
  the fast smoke lane and cite per-fixture results.
- Retired after the 172 Slice 2 diagnostic: the normal-terrain culling config,
  action/effect/UI path, startup flag, and report field. They served the A/B
  investigation but are not product settings. Far-LOD-off remains unchanged.
- Landed early from 172 Slice 3: shared Auto/fixed-4/fixed-8/fixed-16 policy,
  mode-specific source reset, Graphics cycle control, startup argument, and web
  reporting. Focused tests pin single-level fixed policy, mode cycling, source
  identity, and decreasing mesh work at coarser spacing. The menu and all four
  modes were captured and inspected without adding another fixture schema.
- Landed after interactive mode testing: unconditional final-frame real/LOD
  mutual exclusion. Covered-build culling may change generation only; the
  coordinator always suppresses visible LOD under traversal-ready real chunks,
  and a release-active assertion panics if an overlapping tile survives final
  admission. The supplied z-fighting capture is the motivating evidence.
- Landed in the active coverage goal: sparse empty camera sections no longer
  become sole visibility-graph seeds. The high fixture moved from 78/81 to
  81/81 painted real columns and its three culled-but-suppressed ledger rows
  disappeared. The settle executable now preserves parsed detail/build-culling
  policy, reports it, and treats desired prebuild hidden behind real terrain as
  coherent. Clear-color matches remain reportable diagnostics but are marked
  explicitly as non-evidence; the fixed high/revisit fixtures now pass on
  structural painted-coverage and lifecycle facts.
- Landed in the active coverage goal: the offscreen target exposes readable
  `Depth32Float` coverage after the submitted frame. Coverage samples are
  representation-aware rather than flat-plane or color based. At Y=500 the
  checked movement matrix covers all 441 expected chunks and passed at all
  five settled positions with zero clear-depth samples.
- Landed in the active coverage goal: D7 no longer lets a replacement blocked
  by the 256-tile double-residency allowance stop fresh-coverage admission.
  The bounded queue scan defers that replacement and admits eligible missing
  tiles behind it; a focused cache test saturates the allowance and proves the
  fresh tile is submitted.
- Landed in the active coverage goal: the existing five-waypoint movement
  script now also drives a 340-frame smooth lane (220 moving, 120 stationary)
  with per-frame exact-surface GPU depth and missing-age evidence. Its pre-fix
  run failed for 331 consecutive frames with desired-but-pending tiles; the
  stable-shell policy passes with zero missing frames/chunks. The probe now
  permits zero transient missing frames. The full 14-waypoint lane passes with
  392 desired / 360 visible / 81 real-suppressed tiles at range 6 and exact
  far-LOD lifecycle coherence. One hidden inner ring plus two outer rings gives
  600 settled resident tiles without generating the real-terrain interior.
- Browser-smoke debt confirmed 2026-07-19 during Tactical 197 validation: the
  production flight settled with 936 desired, 899 visible, and 1,225 resident
  tiles, zero pending/inflight/upload work, 68 level flips, and maximum
  double-residency one, but `suppressed_without_replacement` advanced by ten.
  The exact result reproduced on the pre-render-coordinator commit, so it is
  not attributed to Worker ownership. The browser probe now uses the current
  visible-plus-suppressed contract and preserves C5 as a red assertion; do not
  weaken it or confuse the idle lifecycle with painted-coverage correctness.
- Validation for the moving-coverage fix: full Rust workspace tests,
  thin-adapter purity, direct web/WASM build, far-LOD-off offscreen smoke,
  movement smoke, timedemo, and flat/Quest Android release APK builds passed.
  No ADB device was attached for a fresh on-device Quest run.
- Validation for the stable-shell correction: full Rust workspace tests,
  thin-adapter purity, direct web/WASM build, flat Android APK, and Android XR
  release APK passed. The complete 14-waypoint probe matched; the 340-frame
  flight passed three consecutive zero-allowance runs with no uncovered frame
  or missing chunk. This evidence comes from GPU depth and lifecycle state,
  not screenshot interpretation.

## Recommended Next Direction

First validate the stable shell in the original interactive high-flight repro.
If that agrees with the depth lane, continue the remaining Slice 2 correctness
burn-down with D3/C6: make large range requests honest instead of silently
truncating them at the 4096-patch cap. D2 arbitration across broader camera
shapes and D6 boundary coverage remain code-audit priorities;
lifecycle/scheduler changes must be justified by a missing column classified
in those states. Then continue tactical 172's remaining work and
residency/perf polish
(Slice 4), debug modes (Slice 5), and re-baseline + closeout (Slice 6).
Reduced-real LOD (tactical 162 Slice 4+) is explicitly deferred with no current
resume commitment. Ordering authority: tactical 171's thread ledger.
