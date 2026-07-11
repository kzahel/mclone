# Far LOD

Topic: `far-lod-settle-contract`

Living status for the synthetic far-terrain LOD system: the coarse,
non-authoritative surface shell drawn outside normal render distance.

Last reconciled: 2026-07-11.

## Current State

Mechanically complete, behaviorally unproven. Tactical 166 landed far LOD as a
producer on the shared resident-tile substrate: event-driven desired coverage,
builds on the shared render-compile worker pool under `LodBuildAdmission` /
`LodUpload` budget families, 16×16-chunk region arenas with per-region packed
index draws, per-eye + multiview paths, and 4/8/16-block multi-level rings
with 2-chunk hysteresis and replacement-before-suppress transitions. Proven on
desktop offscreen, Quest 3, and all three production browser lanes.

Known broken in real sessions: coverage gaps. Flying up after spawn shows
concentric blue-void rings between drawn real terrain and the LOD bands (user
captures 2026-07-11). Confirmed mechanisms are ledgered as D1–D8 in
[tactical 172](../tactical/172-far-lod-settle-contract-and-detail-modes.md);
the two load-bearing ones:

- **D1:** with the camera above all resident sections, the occlusion-traversal
  fallback seeds only the highest resident section layer, graph-culling loaded
  visible real terrain (`mclone-render/src/chunk.rs::traversal_start_keys`).
- **D2:** LOD suppression uses traversal-ready columns — a strict superset of
  what actually paints — so culled-but-ready columns get neither real terrain
  nor a LOD tile, and `suppressed_without_replacement` cannot see it.

There is no settle-state instrument yet: nothing asserts which chunks/tiles
are drawn at which levels once streaming settles. Tactical 172 Slice 1 builds
that harness first (settle contract C1–C8), then Slice 2 burns down the defect
ledger, then detail modes (auto/4/8/16, debug 1/2) and residency polish.

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
  (`startup_args.rs`), Options checkbox + range slider (`mclone-ui`).

## Contract

Real chunks stay authoritative; LOD never satisfies chunk interest, collision,
raycast, edits, or gameplay. Chunk-granular tiles only (166 locked invariant;
16-block spacing is the per-chunk ceiling). One budget owner, one worker pool,
no whole-world rebuild/upload paths. Far-LOD-off is byte-identical. The settle
contract (tactical 172, C1–C8) is the behavioral acceptance bar: deterministic
desired set, painted coverage completeness at settle from any camera pose,
set coherence, view-independent suppression, replacement-before-suppress,
no silent caps, bounded steady state.

## Validation

- Existing: `far_lod`/`lod_coverage` unit suites, offscreen far-LOD captures
  (RD4, seed 12345, playable + idle), web far-LOD probes
  (`native:web:far-lod-*-smoke`), Quest orbit far-LOD on/off metrics lanes.
- Planned (172 Slice 1): `native:lod-settle:smoke` / `native:lod-settle:probe`
  — waypoint scripts with settle assertions, per-chunk ledger dumps, coverage
  image probe, revisit determinism. Once landed, every far-LOD change must run
  the smoke lane and cite per-fixture results.

## Recommended Next Direction

Follow tactical 172 in order: harness (Slice 1, lands with the fly-up repro
red), correctness burn-down (Slice 2), detail modes (Slice 3), residency/perf
polish (Slice 4), debug modes (Slice 5), re-baseline + handoff (Slice 6).
Reduced-real LOD (tactical 162 Slice 4+) resumes only after 172 Slice 2.
Ordering authority: tactical 171's thread ledger.
