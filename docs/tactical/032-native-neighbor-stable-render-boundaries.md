# 032: Native Neighbor-Stable Render Boundaries

Status: draft.

## Purpose

Reduce underground/x-ray cave popping near chunk boundaries by matching the
Java 1.17.1 render-chunk neighbor readiness contract.

This is the follow-up to the inside-solid visibility fix. Disabling the section
visibility graph while the camera is inside an occluding block is correct and
Java-shaped, but it does not address boundary instability. The remaining pop is
caused by rendering or meshing chunks as if missing neighboring chunks were air.
That can expose large artificial outside faces until the adjacent chunk arrives
or until traversal state changes.

The desired player-facing behavior is:

- free-roam/noclip underground still gives useful x-ray cave visibility
- normally hidden back faces stay culled by the GPU
- section visibility graph does not remove caves while the camera is embedded in
  solid terrain
- missing neighbor chunks do not create fake cave openings or large boundary
  slabs that pop in/out as the camera crosses chunk boundaries

## Reference Findings

### Smart culling is disabled inside solid spectator blocks

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java:881-904`

Java starts render traversal with `minecraft.smartCull`, then disables it when
the camera's block is solid-rendering:

- `LevelRenderer.java:881`: `boolean var10 = this.minecraft.smartCull`
- `LevelRenderer.java:902-904`: if `var3` is true and
  `level.getBlockState(var5).isSolidRender(...)`, set `var10 = false`

This supports our first fix: when the native camera is inside an occluding
block, force `TexturedSectionRenderOptions.section_occlusion_culling = false`
for that frame. The user's manual `O` toggle still controls the base option.

### Traversal refuses far render chunks without horizontal neighbors

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java:914-950`

Java traverses render chunks breadth-first. Even after visibility-graph checks,
it only steps into a neighbor if the neighbor render chunk exists and
`hasAllNeighbors()` is true:

- `LevelRenderer.java:920`: compute `var18 = getRelativeFrom(...)`
- `LevelRenderer.java:921-936`: optional smart-cull direction/visibility checks
- `LevelRenderer.java:938`: require `var18 != null && var18.hasAllNeighbors()`
- `LevelRenderer.java:944-949`: only visible and accepted neighbors are queued

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java:958-967`
also clamps relative traversal to the current render distance in X/Z and to the
world build-height range.

### Rebuilds also refuse far render chunks without horizontal neighbors

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java:281-295`

`RenderChunk.hasAllNeighbors()` uses the camera position and loaded full chunk
presence:

- `doesChunkExistAt(...)` calls `level.getChunk(sectionX, sectionZ,
  ChunkStatus.FULL, false)` and requires a non-null result.
- chunks within 24 blocks of the camera are allowed immediately
  (`getDistToPlayerSqr() <= 576.0`)
- farther chunks require west, north, east, and south full chunks

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java:458-467`
uses the same check during `RebuildTask.doTask`. If neighbors are not available,
the task is cancelled instead of compiling a mesh against fake air.

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java:598-604`
applies the same rule to transparency resort tasks.

### Compile regions use a one-block halo from real chunks

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java:403-408`
creates a `RenderChunkRegion` for origin `offset(-1, -1, -1)` through
`offset(16, 16, 16)` with a padding value of `1`.

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/RenderChunkRegion.java:27-48`
loads all chunks covering that expanded region before constructing the region.
`RenderChunkRegion.java:75-81` fills a block-state cache by asking those real
chunks for block states. This matters because face culling at the render-chunk
edge depends on real neighbor blocks, not on assumed air.

`reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java:67-75`
returns `null` for missing chunks when `create=false`. That is the path used by
`doesChunkExistAt`, so missing full chunks are detected instead of silently
substituted with an empty chunk.

### Face emission still relies on normal block culling

`reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java:512-548`
iterates block positions in the render chunk and calls
`BlockRenderDispatcher.renderBatched(..., true, ...)`. The final block-model
face decisions still come from the vanilla block/model face-culling path using
the real `RenderChunkRegion` block getter.

The reference engine does not intentionally generate outside faces against
missing chunks. It either has the neighbor data available, is close enough to
allow the near-camera exception, or skips/cancels the far render chunk work.

## Native Current State

Relevant native paths:

- `native/apps/mclone-native-client/src/main.rs`
  - default radius is currently `DEFAULT_CHUNK_RADIUS = 1`
  - `ChunkApp::effective_render_options` now disables section occlusion when
    `WindowSceneRuntime::camera_inside_occluding_block(...)` is true
  - dirty updates mark the changed chunk and cardinal neighbors dirty
- `native/crates/mclone-mesh/src/lib.rs:942-953`
  - textured face emission checks the cullface neighbor and skips the face when
    the neighbor occludes
- `native/crates/mclone-mesh/src/lib.rs:1292-1307`
  - `block_state_at_world_or_air` returns `AIR_BLOCK_STATE_ID` if no mesh input
    chunk contains the queried world coordinate
- `native/crates/mclone-render/src/chunk.rs:352-465`
  - section traversal uses records/frustum/visibility graph, but it does not yet
    encode Java's `hasAllNeighbors()` render-chunk readiness rule

The mesh fallback is the main suspected source of boundary artifacts:
if a solid block lies at a loaded chunk edge and the adjacent chunk is missing
from the mesh input set, the neighbor query becomes air, so an exposed face is
emitted. When the adjacent chunk arrives, that face disappears. When the camera
crosses a boundary, the set of loaded/visible sections can change enough for a
large underground region to pop.

The visibility graph can make the symptom more obvious, but it is not the whole
cause. With graph culling off, meshes compiled against missing-neighbor air can
still contain artificial boundary faces.

## Target Design

Introduce a render-neighbor readiness contract for native chunk rendering.

The contract should be Java-shaped:

- For render traversal and rebuild eligibility, a render chunk/section farther
  than 24 blocks from the camera needs west, north, east, and south chunk
  snapshots available.
- Chunks within 24 blocks of the camera may render/rebuild without the full
  horizontal neighbor set, matching Java's near-camera exception.
- Mesh building must not treat missing horizontal neighbor chunks as air for
  far render chunks. It should defer the rebuild or retain the last good mesh.
- Once a chunk becomes neighbor-ready, any deferred dirty state should be
  processed normally.

This should preserve underground x-ray cave usefulness: the camera being inside
solid terrain disables section visibility graph culling, but chunk-boundary
meshes are only trusted when the needed neighbor data is available.

## Recommended Implementation Slices

### 1. Pure readiness helpers

Add small tested helpers before changing renderer behavior:

- `render_chunk_center(pos, section_y)` or equivalent distance helper
- `render_chunk_distance_to_camera_sqr(...)`
- `has_horizontal_neighbor_snapshots(chunk_pos, loaded_snapshots)`
- `is_render_neighbor_ready(chunk_pos, camera_position, loaded_snapshots)`

Use the Java threshold exactly: distance squared `<= 576.0` means ready even
without all neighbors. The distance should be from the render chunk/section
center to the active camera position, as Java uses `bb.min + 8` in
`ChunkRenderDispatcher.java:322-327`.

Open detail: Java render chunks are 16x16x16 sections, but `doesChunkExistAt`
checks horizontal full chunks. Native should start with the same horizontal
chunk check and use the section center for distance.

### 2. Stop compiling far boundary meshes against fake air

Change the dirty render-section rebuild path so a target chunk that is not
neighbor-ready is deferred instead of rebuilt from a partial input set.

Preferred behavior:

- if a previous mesh exists, keep it visible and mark the chunk still dirty
- if no previous mesh exists, keep it absent instead of uploading an empty or
  boundary-exposed mesh
- expose counters for deferred chunks/sections so this is visible in the debug
  pane or frame-budget output

This is less poppy than deleting a mesh whenever readiness is temporarily lost,
and it matches Java's practical behavior better than creating a new mesh with
missing neighbors treated as air.

Implementation candidates:

- gate in `WindowSceneRuntime::sync_render_sections` before calling the mesh
  builder
- or pass readiness into the render-section cache update so
  `CachedTexturedRenderSections` can retain old records and deferred dirty keys

Keep the first slice simple: horizontal chunk snapshots are enough. Do not solve
partial chunk-status networking yet unless a missing status abstraction blocks
the implementation.

### 3. Make mesh input absence explicit

Longer-term, replace `block_state_at_world_or_air` for boundary-sensitive
queries with an API that distinguishes:

- loaded neighbor block state
- known air inside a loaded chunk/section
- missing chunk/section data

The rebuild gate should prevent most missing-neighbor queries for far chunks,
but explicit absence avoids future regressions and makes unit tests stronger.
For a target chunk deemed ready, a missing local section inside a loaded chunk
can still be treated as air if the snapshot encodes omitted all-air sections.

### 4. Apply neighbor readiness to traversal

Add a render-record flag or callback so `mclone-render` traversal can mirror
`LevelRenderer.java:938`: do not enqueue far neighbor sections whose horizontal
chunk neighbors are missing.

This should be separate from section visibility graph culling:

- when graph culling is disabled, frustum drawing should still not draw records
  that were never neighbor-ready/uploaded as valid meshes
- when graph culling is enabled, traversal should not step through
  neighbor-incomplete far sections

The mesh gating is the higher priority. Traversal gating should follow once the
record/cache can express readiness cleanly.

### 5. Instrument the behavior

Add debug stats so field testing is straightforward:

- loaded chunks
- neighbor-ready chunks
- deferred render chunks/sections
- retained stale meshes
- near-camera exception count

This is useful for verifying that underground pops correlate with deferred or
newly-ready boundary chunks instead of guessing from screenshots.

## Tests

Unit tests:

- `has_horizontal_neighbor_snapshots` returns false until W/N/E/S snapshots are
  loaded, and ignores diagonal-only neighbors.
- near-camera distance `<= 576.0` bypasses the missing-neighbor requirement.
- far-camera distance `> 576.0` requires all four horizontal neighbors.
- a dirty not-ready chunk keeps an existing render record and remains dirty.
- a dirty not-ready chunk with no previous record does not upload boundary
  geometry.

Mesh tests:

- construct a solid target chunk with a missing east neighbor; far rebuild is
  deferred, not meshed with east faces.
- construct the same case with the east neighbor loaded and solid; no east faces
  are emitted.
- construct the same case with the east neighbor loaded and air; east faces are
  emitted intentionally.

Integration/headless tests:

- move the camera underground across a chunk boundary with `--chunk-radius 1`
  and compare consecutive captures for large boundary-face pops.
- repeat with section occlusion on and with the camera embedded in rock; the
  effective option should disable graph culling while embedded.
- repeat with manual `O` toggled off; boundary readiness should still prevent
  fake missing-neighbor faces.

## Validation Commands

Use the native lanes:

```bash
cargo test --manifest-path native/Cargo.toml -p mclone-mesh -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client -- --nocapture
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm native:movement:smoke
pnpm native:timedemo:smoke
```

For rendered validation, save captures to `/tmp`, for example:

```bash
cargo run --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-neighbor-ready-underground.png \
  --width 960 --height 640 --chunk-radius 1 --screenshot-debug-pane true
```

Inspect the PNG before considering the slice complete.

## Open Questions

- Should the near-camera exception be exactly 24 blocks for native, or should
  native expose a debug knob while we tune underground noclip behavior? Default
  should be exact Java parity.
- Should not-ready chunks retain stale meshes forever while dirty, or only until
  they are outside render distance? Retain while in the cache; drop on normal
  unload/eviction.
- Should default native render radius increase above `1` for interactive use?
  That would reduce how often users sit near loaded-world boundaries, but it is
  not a correctness fix and should remain separate.
- How should this interact with future remote dedicated servers and partial
  chunk statuses? Initial implementation can treat a present `ChunkSnapshot` as
  Java `ChunkStatus.FULL`; later protocol work can make status explicit.
- Do vertical neighbors matter? Java `hasAllNeighbors()` checks only W/N/E/S.
  Start with horizontal parity and rely on build-height/section bounds for Y.

## Acceptance Criteria

- Inside-solid cameras still get x-ray cave visibility because section graph
  culling is effectively disabled for that frame.
- GPU back-face culling remains enabled, so backside tunnel/ground faces do not
  render.
- Far chunks are not newly rebuilt from missing-neighbor air.
- Crossing a chunk boundary underground no longer creates large fake chunk-face
  pops from missing W/N/E/S data.
- Debug stats can explain when a chunk is deferred for neighbor readiness.
