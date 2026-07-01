# 121: Surface LOD First Slice

Status: active; Slice A landed. Shared native Rust renderer/runtime workstream.

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
- Use a deterministic placeholder height/color model keyed by seed and world
  position.
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

Status: landed first prototype.

Implementation targets:

- `mclone-render`: vertex-colored far surface mesh renderer.
- `mclone-app-runtime`: shared config, deterministic surface mesh builder, and
  cache.
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

Landed scope:

- `mclone-render` now has a vertex-colored far surface LOD renderer.
- `mclone-app-runtime` now has the opt-in config, deterministic placeholder
  surface mesh builder, and session-local cache.
- `mclone-native-client` now parses `--far-lod true|false`, keeps it disabled by
  default, and feeds the mesh through the shared flat render path.
- The shared Options UI now exposes a `Far LOD` checkbox for toggling the
  prototype at runtime.
- The current pass is single-view flat only; XR/multiview remains intentionally
  out of scope until this prototype has measured value.

Current hard-coded prototype distances:

- LOD starts at `render_distance + 2` chunks.
- LOD ends at `render_distance + 12` chunks.
- Effective LOD band width is therefore 10 chunks.
- Surface samples are spaced every 8 blocks.

This is intentionally not exposed as a slider yet. Once the source data is real
enough to evaluate, prefer a single `Far LOD Distance` control over separate
start/end sliders; keep the start margin/blend region hidden unless pop-in
testing proves it must be user-tunable.

## Follow-Up Slices

### B. Replace Placeholder Surface Source

Use a budgeted, shortcut worldgen sampler that waits for nearby real chunks and
skips nonessential features. Keep underwater/underground omission explicit. This
is the next implementation slice.

### C. Runtime Scheduling And Diagnostics

Expose mesh vertex/index counts, update cadence, and build time in diagnostics.
Add frame-budgeted rebuilds instead of one synchronous mesh build per cache miss.

### D. XR And Quest Measurement

Measure fragment cost, pop-in, and frame pacing with normal render distance near
5 chunks and LOD beginning just outside that range. Do not default-enable until
Quest headset captures show a product benefit.

### E. Persistence Decision

Only after the prototype proves value, decide whether to add persistent LOD
storage. If we do, document the data shape separately from Distant Horizons'
column/segment implementation so our surface-only constraints stay clear.
