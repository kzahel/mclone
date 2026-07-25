# 121: Surface LOD First Slice

Status: closed 2026-07-11. Slices A and B landed; the prototype proved its
value and evolved past this tactical's scope. The remaining follow-up slices
are dispositioned below: C (scheduling/diagnostics) and D (XR/Quest
measurement) landed through Tactical 166 — Shared Resident-Tile Substrate
(shared workers, budget families, counters, and Quest far-LOD gates); E
(persistence decision) stays deferred to Tactical 162 — Real-Chunk LOD
Reduction Draft Slice 6. Product hardening (settle contract, coverage
correctness, detail modes) now lives in Tactical 172 — Far LOD Settle
Contract And Detail Modes. Shared native Rust renderer/runtime workstream.

Retirement note 2026-07-25: the experiment is now rejected as a scalable
far-horizon architecture. Tactical
[`245`](245-retire-chunk-far-lod-runtime.md) owns removal of its runtime
surface. This file remains an execution record, not implementation guidance.

## Purpose

Add a deliberately small, opt-in far-terrain LOD path so we can measure whether
a coarse surface shell is useful for Quest and other low-power targets before
committing to Distant Horizons-style persistence, complete worldgen parity, or
multi-level voxel LOD.

This follows the investigation in [`docs/lod-architecture.md`](../lod-architecture.md):
Distant Horizons is the durable reference for column/segment persistence and
non-surface LOD, while this workstream starts with the simplest surface-only
experiment we can render and disable cleanly.

## Non-Goals For The First Slice

- No persistent LOD world storage.
- No vanilla worldgen parity for the far mesh.
- No caves, overhangs, interiors, structures, or underground/underwater geometry.
- No decoration pipeline beyond a future tiny whitelist.
- No default enablement on desktop, web, Android, or XR.

## First Slice Shape

The first implementation should prove the renderer and runtime boundary, not the
final terrain algorithm:

- Add an explicit `--far-lod true|false` switch, defaulting to `false`.
- Generate one in-memory coarse surface ring outside the normal render distance.
- Start with a deterministic placeholder height/color model keyed by seed and
  world position, then replace it with real surface chunk sampling once the
  renderer path is proven.
- Leave a square hole for real chunk geometry so near chunks remain authoritative.
- Render the LOD shell before normal terrain, with normal chunks depth-overwriting
  it where they overlap.
- Keep all generation and mesh state discardable on session/world replacement.

## Open Questions

- Persistence: do we eventually need a separate LOD world cache, or can this stay
  session-local for Quest-focused distances?
- Source data: when should LOD generation sample the real chunk pipeline instead
  of a shortcut surface model?
- Scheduling: should far LOD wait for nearby chunks to reach `Features`, `Light`,
  or client-ready status?
- Visual policy: how close can LOD begin before popping is unacceptable in flat
  and XR views?
- Materials: which decorations are worth whitelisting first, if any?
- Data shape: is a surface grid enough, or do we need a Distant Horizons-like
  column/segment model for player-built structures?

## Slice A: Opt-In Prototype

Status: Slice A and B landed.

Implementation targets:

- `mclone-render`: vertex-colored far surface mesh renderer.
- `mclone-app-runtime`: shared config, real surface mesh builder, and
  incrementally filled session-local cache.
- `mclone-native-client`: CLI flag and desktop/headless wiring for validation.

Validation targets:

- Passed: `cargo test -p mclone-render -p mclone-app-runtime -p
  mclone-native-client --manifest-path native/Cargo.toml`
- Passed: `cargo run -p mclone-native-client --manifest-path native/Cargo.toml
  -- --screenshot /tmp/mclone-far-lod.png --width 960 --height 540
  --startup-wait idle --seed 12345 --render-distance 5 --far-lod true
  --day-time 6000 --freeze-time --fullbright true`
- Passed: visibility capture with high camera and closer normal radius:
  `/tmp/mclone-far-lod-visible.png`
- Inspected: the visible capture shows the coarse far surface shell and the
  deliberate empty near hole.
- Passed: `cargo test -p mclone-app-runtime --manifest-path native/Cargo.toml
  far_terrain_lod`

Landed scope:

- `mclone-render` now has a vertex-colored far surface LOD renderer.
- `mclone-app-runtime` now has the opt-in config, a terrain-surface mesh
  builder backed by `mclone-worldgen::levelgen::generate_overworld_surface_chunk`,
  and a session-local cache that builds a few far chunks per frame.
- `mclone-native-client` now parses `--far-lod true|false`, keeps it disabled by
  default, and feeds the mesh through the shared flat render path.
- The shared Options UI now exposes a `Far LOD` checkbox for toggling the
  prototype at runtime, plus a `Far LOD Range` slider for the number of chunk
  rings rendered beyond the normal render distance.
- The current pass is single-view flat only; XR/multiview remains intentionally
  out of scope until this prototype has measured value.
- The current height source samples real 1.17.1-style overworld surface chunks,
  but intentionally skips feature decoration, structures, persistence, caves,
  interiors, and full material fidelity. Water surfaces are represented; terrain
  under water is not separately drawn.

Current hard-coded prototype distances:

- LOD starts at the first chunk ring outside the normal render distance.
- LOD ends at `render_distance + far_lod_range` chunks.
- The default `far_lod_range` is 12 chunks, adjustable in the Options UI.
- Surface samples are spaced every 4 blocks, producing 4x4-block coarse LOD
  cells.
- The LOD mesh is a blocky heightfield: each coarse cell has a flat top face,
  with vertical side faces where adjacent coarse cell heights differ. This
  intentionally looks more Minecraft-like than the first sloped-triangle pass,
  while staying much cheaper than one-to-one block terrain.
- Far terrain chunks are generated incrementally with a small per-frame budget,
  so the LOD mesh fills in over multiple frames instead of blocking startup or a
  camera move on a full-ring rebuild.

This intentionally exposes a single range control rather than separate start/end
sliders. The start boundary should remain tied to the normal render distance
unless pop-in testing proves we need a hidden overlap/blend margin.

## Follow-Up Slices

### B. Replace Placeholder Surface Source

Status: landed first real-surface version and the blocky heightfield revision.
The rebuild-from-scratch concern was resolved by retained tiles (Tactical 162
Slice 0A) and the shared substrate (Tactical 166). Texture-atlas material
mapping and feature whitelists remain unscheduled ideas; if picked up they
belong to the current LOD tactical of record, not here.

### C. Runtime Scheduling And Diagnostics

Status: landed via Tactical 166 — shared worker admission, `LodBuildAdmission`
/ `LodUpload` budget families, and producer/queue/upload counters exposed in
debug overlays and reports.

### D. XR And Quest Measurement

Status: landed via Tactical 166 Slices 3–4 — Quest far-LOD on/off orbit
comparisons, multiview proofs, and per-frame upload/draw accounting. Default
enablement remains a product decision tracked by Tactical 172.

### E. Persistence Decision

Status: deferred to Tactical 162 Slice 6 (discardable LOD persistence),
unchanged: decide only after reduced-real semantics, dirtying, and budget
behavior are stable.
