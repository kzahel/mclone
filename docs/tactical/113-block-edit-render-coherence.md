# 113: Block Edit Render Coherence And Web Remesh Cost

Status: active; Phase 0 web update-kind telemetry and a solid-block edit probe
landed on 2026-06-30. The visual boundary-frame probe, deterministic repeated
walking-break target setup, live light deltas, and coherent local publication
remain pending.

## Purpose

Capture two bad current engine behaviors around live block edits and define the
recommended fix shape before code changes:

1. Destroying a block can reveal sky/background for one frame where the newly
   exposed adjacent face should appear.
2. In the web/WASM client, destroying even one block can cause visible frame
   drops, especially while walking and breaking several blocks.

These are related because both pass through live block mutation, light refresh,
render dirtying, and render-section rebuild/upload. They are not the same bug.
The visual gap is a render publication/coherence problem. The web hitch is
mostly a live-update granularity and scheduling problem.

Workstream: shared native Rust/rendering, desktop validation first, with
web/WASM performance as a first-class acceptance lane.

## Current Native Shape

Block deltas use `ServerUpdate::SectionBlockUpdates`, while full chunk-column
state uses `ServerUpdate::ChunkSnapshot`
(`native/crates/mclone-protocol/src/lib.rs`). The renderer has no live light
delta update type today.

For a section block update, render dirtying mirrors Java's local block dirty
neighborhood: `render_dirty_section_keys_for_block_update` expands the changed
world block by one block in each axis and marks the corresponding 3x3x3 render
sections dirty.

For a chunk snapshot, the render session marks the changed chunk and its four
horizontal cardinal neighbors dirty. For each loaded dirty chunk,
`render_section_keys_for_snapshot` targets every vertical render section in the
chunk column.

Web uses a narrow frame budget:

- `WEB_FRAME_UPDATE_DRAIN_BUDGET = 1`
- `WEB_RENDER_CHUNK_MESH_BUDGET = 1`
- one render compile in flight

The web compiler mirror tracks whole `ChunkSnapshot` revisions. When a snapshot
revision changes, `stage_streaming_delta` clones and sends the whole changed
chunk-column snapshot to the render worker as a `ChunkSnapshot` upsert.

### Visual Gap

The newly exposed face is normally absent from the old mesh because it was
occluded by the block that was just destroyed. If the destroyed block's render
section is rebuilt and published before the adjacent section that owns the
newly exposed face, the old block is gone but the replacement face is not yet in
any published mesh. The frame draws sky/background through that one-face hole.

This is most likely at render-section or chunk boundaries, where the destroyed
block and the newly exposed adjacent face belong to different render sections.
Within one render section, a single rebuild should remove the destroyed block
and add the exposed adjacent face together.

Keeping the destroyed block visible until neighbors catch up would hide the
symptom, but it is not the right primary contract. The better contract is:

- retain old render outputs until a replacement is ready
- for player-caused local edits, prioritize or synchronously rebuild the
  visible dirty dependency group
- avoid publishing a partial dirty group that creates impossible geometry

Showing the old block for a frame is much less bad than showing sky through a
solid neighbor face, but the target should be Java-style immediate local
coherence rather than a permanent ghost-block rule.

### Web Slowdown

A single solid-to-air break changes both block opacity and mesh visibility. The
native server path currently does this:

1. `set_block_at_world` patches live blocks and the published chunk snapshot,
   bumps the chunk revision, and records a `SectionBlockUpdates` delta.
2. `refresh_runtime_lighting_after_block_change` sees that the block change
   affects light and recomputes runtime light for the chunk.
3. `publish_runtime_light_sections` clones the published snapshot, replaces its
   light sections, bumps the snapshot revision again, and queues the clone in
   `pending_runtime_light_snapshots`.
4. Draining scheduler events emits the runtime light snapshot first as
   `SnapshotReady`, then emits the section block delta.

So one block removal can deliver a full updated `ChunkSnapshot` plus a smaller
section block delta. On web, that full snapshot revision is enough to clone and
ship the whole chunk column to the worker. Render dirtying for the snapshot then
targets every vertical section in the chunk and in the four cardinal neighbor
chunks. With a one-section mesh budget and one in-flight compile, repeated block
breaks while moving can quickly create dirty backlog, stale compile results,
and frame-time spikes.

This is not worldgen regeneration. It is coarse live-state publication,
coarse render invalidation, and limited web compile throughput.

## Java 1.17.1 Reference Shape

### Local Block Destruction

`MultiPlayerGameMode.destroyBlock` mutates the client world immediately with
`setBlock(pos, fluid.createLegacyBlock(), 11)`. In 1.17.1, `11` includes
`UPDATE_NEIGHBORS`, `UPDATE_CLIENTS`, and `UPDATE_IMMEDIATE`.

`LevelRenderer.blockChanged` turns the immediate flag into `playerChanged` and
marks the same 3x3x3 section neighborhood dirty. `ChunkRenderDispatcher`
retains a `playerChanged` bit on dirty render chunks.

During the render update, `LevelRenderer` synchronously rebuilds dirty chunks
that are near the camera or dirty from the player. Later async compilation also
sync-rebuilds `isDirtyFromPlayer()` chunks. Java therefore does not primarily
wait for a later background chunk rebuild before the local player's break is
reflected near the camera.

Java also keeps old render buffers live until upload completes and the compiled
chunk reference swaps. The relevant replacement is the render chunk output, not
the old block state.

### Live Server Block And Light Updates

Java does not resend a full chunk for an ordinary live block mutation:

- `ServerLevel.sendBlockUpdated` tells `ServerChunkCache.blockChanged`.
- `ChunkHolder.blockChanged` records changed block positions per section.
- `ChunkHolder.sectionLightChanged` records changed sky/block light section
  filters.
- `ChunkHolder.broadcastChanges` sends a `ClientboundLightUpdatePacket` for
  changed light sections, then sends either `ClientboundBlockUpdatePacket` for
  one changed block or `ClientboundSectionBlocksUpdatePacket` for multiple
  changed blocks in a section.

On the client:

- block packets apply block states with flags equivalent to `19` and dirty the
  affected block render neighborhood
- light packets update `DataLayer`s for only the changed light sections
- each changed light section dirties that section with neighbors

Initial chunk load/unload is different: Java dirties full chunk columns with
neighbors when chunk data arrives or disappears. Live block/light changes are
delta-shaped.

## Important Divergences

The current engine lacks the two reference behaviors that matter here:

- no Java-shaped live light delta packet/update path; live light refresh is
  republished as a full `ChunkSnapshot`
- no local-player/immediate dirty classification that forces a near visible
  dirty block-edit group to rebuild before the next presented frame

The first divergence explains most of the web hitch. The second divergence
explains why a partial dirty group can show sky for one frame instead of either
old geometry or the fully exposed new face.

## Recommended Phasing

### Phase 0: Repro And Telemetry

Add focused diagnostics before changing behavior.

For visual coherence:

- deterministic break of a block on a render-section boundary and chunk
  boundary
- capture consecutive frames or offscreen frame probes around the break
- assert that no frame contains sky/background where the exposed neighbor face
  should be visible

For web performance:

- a browser smoke/probe that removes N solid blocks while walking
- counters for update kinds received: `ChunkSnapshot`, `SectionBlockUpdates`,
  future light deltas
- counts for dirty chunks, dirty sections, target sections, stale compile
  sections, cloned snapshot columns, encoded compile bytes, worker compile time,
  and frame time

This phase should make the current bad behavior measurable and keep later
budget changes honest.

First pass landed:

- `SingleViewRuntime` now keeps cumulative `snapshotUpdateCount`,
  `sectionBlockUpdateCount`, and `unloadUpdateCount` counters.
- Web frame reports and web block-interaction reports expose cumulative and
  per-interaction update-kind counts.
- `pnpm native:web:block-edit-probe` runs a browser probe that places a dirt
  block, breaks that solid block, records update-kind deltas, captures a canvas
  screenshot, and writes `/tmp/mclone-native-web-block-edit-probe.json`.
- 2026-06-30 probe result: the solid dirt break produced
  `snapshotUpdateCountDelta: 1` and `sectionBlockUpdateCountDelta: 1`. The
  probe window ended with `renderDirtyChunkCount: 2`,
  `renderDirtySectionCount: 32`, `acceptedCompileSectionCount: 16`,
  `workerPackedByteLength: 969308`, and `maxFrameGapMs: 59.245`.

Remaining Phase 0 gaps:

- Add a deterministic boundary visual probe for the one-frame sky hole.
- Upgrade the web block-edit probe from one stable solid-block break to a
  deterministic repeated-break while-walking target. The current probe is
  parameterized with `MCLONE_NATIVE_WEB_BLOCK_EDIT_PROBE_BREAKS`, but the
  default remains one break because naive repeated breaks can look through air
  after the first removal.

### Phase 1: Split Live Light Deltas From Chunk Snapshots

Add a Java-shaped live light update to the shared protocol/runtime path. A
likely shape is a `ServerUpdate::LightSectionUpdates` carrying:

- chunk position
- changed section Y values
- optional sky and block packed light data for each changed section
- enough revision/generation data for the render worker mirror to reject stale
  input

Initial chunk snapshots should remain full snapshots. Runtime block edits that
only update live blocks/light should not republish a full chunk-column snapshot.

On the client/runtime side:

- patch the resident chunk snapshot light sections in place
- dirty render sections touched by changed light, using a section-neighbor halo
  comparable to Java `setSectionDirtyWithNeighbors`
- keep `SectionBlockUpdates` for block-state deltas

On web:

- stop using whole-snapshot revision changes as the only compiler mirror input
  for live light edits
- encode light deltas and block deltas into the worker mirror directly
- clone full chunk snapshots only for initial load, resync, and real full-column
  replacement

This phase should substantially reduce the cost of one block break and is also
the right prerequisite for better render coherence.

### Phase 2: Coherent Local Block-Edit Publication

Introduce an explicit immediate/player-caused render dirty path for local block
edits. It should be shared engine state, not desktop-only app glue.

Candidate contracts:

- mark the block edit's 3x3x3 dirty section neighborhood as a high-priority
  local-edit group
- synchronously rebuild/upload visible ready sections in that group before the
  next presented frame when feasible
- or build the group asynchronously but atomically publish the replacement
  sections, retaining old sections until the group is complete

The first option matches Java most directly for near/player edits. The second
option is useful as a web fallback when synchronous worker compilation is not
available or not cheap enough.

Do not implement a special "keep rendering the destroyed block" gameplay rule.
The render cache may retain old meshes while replacements are unavailable, but
client block state, selection, collision, and server reconciliation should stay
authoritative.

### Phase 3: Coalescing, Cancellation, And Budgets

After deltas and coherent publication exist, tune the scheduler:

- coalesce repeated edits by render section before compiling
- collapse multiple dirty marks for the same local-edit burst into one group
- reject stale worker results early and keep stale section counts bounded
- allow a temporary higher-priority local-edit budget without making movement
  streaming starve forever
- consider increasing web compile/update budgets only with telemetry proving
  the new cost per update is small

Budget increases alone are not a fix while live light still republishes full
snapshots.

### Phase 4: Validation

Required gates for the implementation slices:

- targeted Rust tests for light-delta encode/decode and client snapshot patching
- render-session tests for immediate/local-edit dirty priority and stale result
  handling
- native offscreen visual probe for section-boundary and chunk-boundary block
  breaks
- web smoke/probe showing block breaks no longer clone whole chunk snapshots for
  light-only updates
- browser performance comparison for repeated break while walking
- existing native and web build/smoke gates from `AGENTS.md`

## Acceptance Criteria

- Destroying a boundary block never presents a frame with sky/background where
  the newly exposed adjacent face should be visible.
- A single web block removal does not send or clone a full `ChunkSnapshot`
  solely because runtime light changed.
- Live block updates dirty the affected block/light section neighborhoods, not
  whole chunk columns except for true full snapshot load/unload/resync events.
- Repeated block removals while walking keep stale compile counts bounded and do
  not produce visible multi-frame hitches on the web smoke path.
- The fix is shared-first: desktop, web, Android, and XR consume the same
  protocol/runtime/render-session semantics, with only platform compile/upload
  mechanics differing.

## Non-goals

- Worldgen changes.
- Replacing the lighting solver wholesale.
- Greedy-meshing or mesh-format redesign.
- A desktop-only workaround.
- Making the old block visually persist as a gameplay/rendering policy.

## Code References

Native current shape:

- `native/crates/mclone-protocol/src/lib.rs`
- `native/crates/mclone-server/src/scheduler.rs`
- `native/crates/mclone-server/src/integrated.rs`
- `native/crates/mclone-render-session/src/lib.rs`
- `native/crates/mclone-app-runtime/src/lib.rs`
- `native/apps/mclone-web-client/src/web_canvas.rs`

Java 1.17.1 reference:

- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/MultiPlayerGameMode.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/Level.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/block/Block.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerLevel.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ServerChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/server/level/ChunkHolder.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientPacketListener.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientChunkCache.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/multiplayer/ClientLevel.java`

Related tactical:

- `097-native-lighting-leakage-hardening.md`
