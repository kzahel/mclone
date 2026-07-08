# 163: Render Section CPU Mesh Eviction

Status: active 2026-07-08; Slice 0 byte accounting landed.

Workstream: native Rust shared render-session / render-resource boundary.
Desktop validation first, but the target shape must stay shared across flat,
XR, Android, headless, and native-web.

## Purpose

Stop retaining full CPU-side terrain mesh payloads after successful GPU upload.

The current render session cache stores `TexturedRenderSectionMesh` values per
resident section. Those meshes contain owned vertex and index vectors. The draw
path already uses GPU-resident `GpuTexturedChunkMesh` buffers, so the retained
CPU mesh payloads mostly support startup/full-upload convenience, diagnostics,
and renderer-resource rebuilds.

The target is more vanilla-shaped:

```text
client chunk snapshots / replica
  authoritative source for remesh

render-section metadata cache
  resident section keys, visibility, stats, dirty/revision state

transient mesh payload
  compile result, pending upload, then dropped

renderer draw resources
  GPU-resident section buffers used every frame
```

This is expected to reduce duplicate terrain memory and clone/copy pressure. It
is not expected to directly reduce steady-state draw cost, because unchanged
sections already stay GPU-resident.

## Vanilla Reference

Read before implementation:

- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/chunk/ChunkRenderDispatcher.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ChunkBufferBuilderPack.java`
- `reference/minecraft-1.17.1/src/com/mojang/blaze3d/vertex/BufferBuilder.java`
- `reference/minecraft-1.17.1/src/com/mojang/blaze3d/vertex/VertexBuffer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/ViewArea.java`
- `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java`

Key Java facts:

- `ViewArea` owns a fixed grid of `RenderChunk` slots, not an LRU cache.
- `RenderChunk` owns one `VertexBuffer` per chunk render layer. These are the
  resident drawable terrain buffers.
- `CompiledChunk` retains compact metadata: layer presence, empty flag,
  renderable block entities, `VisibilitySet`, and translucent sort state.
- `ChunkBufferBuilderPack` provides pooled CPU staging buffers for compile and
  upload. `clearAll()` / `discardAll()` reset draw state after work completes.
- Java does not retain full per-section CPU vertex/index mesh copies after
  upload.

## Current Native Shape

Relevant paths:

- `native/crates/mclone-render-session/src/section_cache.rs`
  - `CachedTexturedRenderSections` stores `CachedTexturedRenderSectionSlot`.
  - Each slot currently owns a full `TexturedRenderSectionMesh`.
  - `sections()` clones every resident mesh.
- `native/crates/mclone-mesh/src/data.rs`
  - `TexturedRenderSectionMesh` owns `TexturedVisibleChunkMesh`.
  - `TexturedVisibleChunkMesh` owns `Vec<TexturedChunkVertex>` and `Vec<u32>`.
- `native/crates/mclone-render-session/src/session.rs`
  - Dirty/revision/inflight state is already separate from draw resources.
  - `cached_sections()` leaks full mesh ownership to platform/startup paths.
- `native/crates/mclone-render-session/src/upload.rs`
  - `RenderSectionUploadCoordinator` already treats mesh payloads as pending
    upload lifecycle items.
- `native/crates/mclone-render/src/chunk.rs`
  - `TexturedSectionDrawResources` owns the GPU section map.
  - `apply_section_updates(...)` uploads changed sections and removes dropped
    sections.

Current callers that depend on full CPU mesh payloads:

- desktop full sync / cached runtime section upload paths
- Android startup draw-resource creation
- XR startup draw-resource creation
- web first upload / reporting
- headless screenshot and renderer-rebuild probes
- tests that inspect `cached_sections()` contents or counts

Normal interactive frame drawing does not need the retained CPU payloads.

## Target Ownership

Introduce a resident metadata type in `mclone-render-session`, for example:

```rust
pub struct TexturedRenderSectionMetadata {
    pub key: RenderSectionKey,
    pub visibility: VisibilitySet,
    pub stats: SectionMeshStats,
    pub drawable: bool,
}
```

Then make the resident cache store:

```rust
struct CachedTexturedRenderSectionSlot {
    metadata: TexturedRenderSectionMetadata,
    dirty: bool,
}
```

`RenderSectionCacheUpdate` may continue to carry full
`TexturedRenderSectionMesh` values, but only as a transient lifecycle payload:

```text
worker compile result
  -> accepted cache update
  -> optional upload coordinator
  -> renderer apply_section_updates
  -> payload dropped
```

After this change:

- the render-session cache should not clone or retain vertex/index vectors
- the renderer remains the owner of resident GPU buffers
- resource rebuild paths must explicitly recompile or receive a current upload
  batch, instead of pulling full meshes out of the cache

## Implementation Slices

### Slice 0: Baseline And Byte Accounting

Add diagnostic accounting before changing ownership:

- estimate CPU retained mesh bytes in `CachedTexturedRenderSections`
- estimate transient pending-upload bytes in `RenderSectionUploadCoordinator`
- expose the counters in the existing render/debug/probe reports where section
  counts and upload counts already appear
- record a desktop baseline at at least RD10 or the current standard render
  smoke, and if practical record a Quest/Android memory-sensitive baseline

This slice is allowed to be measurement-only. It provides proof that the cleanup
removes real duplicated memory instead of just reshuffling code.

Landed 2026-07-08:

- `TexturedVisibleChunkMesh` and `TexturedRenderSectionMesh` now expose
  estimated owned-byte helpers based on vertex/index vector capacity.
- `CachedTexturedRenderSections` reports resident CPU mesh section,
  vertex/index, face, and owned-byte pressure through
  `RenderSectionCacheUpdate`.
- `RenderSectionUploadCoordinator` reports transient queued upload mesh bytes.
- Desktop, web, XR, and Android XR diagnostics surface the new counters through
  existing render/upload report paths.
- Focused tests cover the mesh helper, resident cache accounting, and pending
  upload accounting.

### Slice 1: Split Resident Metadata From Mesh Payload

Refactor `CachedTexturedRenderSections` so `apply_build_report(...)` inserts
metadata derived from rebuilt sections instead of cloning and storing the full
section mesh.

Expected changes:

- add `TexturedRenderSectionMetadata`
- replace slot field `mesh: TexturedRenderSectionMesh` with metadata
- keep `section_keys_by_chunk`, generation, dirty flags, and revision handling
  behavior unchanged
- replace `sections()` with metadata/key/stat accessors
- keep `RenderSectionCacheUpdate.rebuilt_sections` unchanged so existing upload
  code can still receive transient mesh payloads
- update render-session tests to validate metadata, dirty keys, removals, and
  generation behavior without relying on CPU vertex/index retention

Important implementation detail: when applying a completed build report, borrow
`rebuilt_sections` to compute and insert metadata, then move the same
`rebuilt_sections` into the update. Do not clone the mesh payload back into the
cache.

### Slice 2: Make Draw Resources Creatable Empty

Add an explicit empty terrain draw-resource initialization path:

```rust
TexturedSectionDrawResources::new_empty(...)
```

or make `new(...)` cheap and clear when `sections` is empty.

Then route startup paths through:

```text
create empty draw resources
sync/compile until initial sections are ready
apply_section_updates(rebuilt_sections, removed_section_keys)
```

This removes the need for startup code to ask the runtime for all cached CPU
meshes.

Affected paths include:

- `native/apps/mclone-native-client/src/flat_client_driver.rs`
- `native/apps/mclone-android-client/src/lib.rs`
- `native/crates/mclone-xr-scene/src/session.rs`
- `native/apps/mclone-web-client/src/web_canvas.rs`
- `native/apps/mclone-native-client/src/headless.rs`

### Slice 3: Resource Rebuild Without CPU Mesh Retention

Replace "re-upload all cached sections" with an explicit rebuild policy.

Preferred first policy:

```text
renderer resources lost or intentionally rebuilt
  -> create empty draw resources
  -> mark all resident render sections dirty
  -> pump sync_all_render_sections(...)
  -> upload rebuilt sections as normal diffs
```

Add shared runtime API for this instead of platform-local logic, for example:

```rust
mark_all_render_sections_dirty_for_resource_rebuild()
```

or a higher-level:

```rust
rebuild_render_section_draw_resources(...)
```

Keep this path rare and explicit. It is acceptable for a renderer rebuild smoke
or surface-loss recovery to show a transient loading/sky-only frame while
sections recompile. It is not acceptable to silently depend on stale CPU meshes.

### Slice 4: Diagnostics And Tests Away From `cached_sections()`

Remove or demote APIs that return full resident CPU meshes.

Replace them with:

- `cached_section_count()`
- `cached_section_metadata()`
- `cached_section_stats()`
- `resident_render_section_keys()`
- renderer-side `section_count()` / `index_count()` for uploaded draw pressure
- render-session metadata counters for resident logical pressure

Update diagnostics that currently compute values from `cached_sections()`:

- web generated chunk reports
- startup/playable readiness checks
- native perf probes
- scene runtime tests
- headless captures

Prefer counts and metadata over full mesh access. If a diagnostic truly needs
mesh bytes, make it trigger a fresh compile explicitly.

### Slice 5: Delete The Old Mesh-Retention API

Once all production call sites are gone:

- remove `EngineRenderSession::sections()`
- remove runtime `cached_sections()` APIs that return `Vec<TexturedRenderSectionMesh>`
- keep only metadata/count/key accessors
- add tests that fail if the resident cache stores `TexturedRenderSectionMesh`
  or exposes full mesh payloads

## Non-Goals

- GPU buffer pooling or in-place suballocation. Current changed sections may
  still create replacement `wgpu::Buffer`s. Pooling is a separate follow-up if
  upload allocation shows up in probes.
- LRU terrain cache policy. The current live set remains driven by loaded
  client chunks/render sections.
- Changing the mesher output format.
- Dropping client chunk snapshots or the authoritative client replica.
- Adding a server/client mesh protocol.
- Reworking translucent sorting parity beyond preserving existing behavior.

## Risks And Mitigations

| Risk | Mitigation |
|---|---|
| Resource rebuild becomes slower because it must recompile sections | Keep rebuild explicit and rare; use loading/progress state; validate with renderer-rebuild smoke. |
| Startup paths briefly render sky-only while compile/upload catches up | This already happens during streaming; keep startup smokes pumping to ready before screenshots. |
| Diagnostics lose easy vertex/index totals | Store metadata stats at compile acceptance and use renderer draw-resource stats after upload. |
| Upload-budgeted XR path drops payload before upload | Keep `RenderSectionUploadCoordinator` as the transient payload owner until `complete_applied_lifecycle_items(...)`. |
| Web first-upload path expects all cached sections | Initialize draw resources empty and apply the current update batch, or force a startup full-sync batch. |
| Tests assert exact cached mesh contents | Move those assertions to mesher tests or explicit compile-result tests; render-session cache tests should assert metadata and dirty behavior. |

## Validation

Required checks for the implementation slice:

```bash
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-render-session -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-app-runtime -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-native-client -- --nocapture
cargo check --manifest-path native/Cargo.toml --workspace
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
pnpm --silent native:frame-budget:smoke
```

Rendered-output validation is required for ownership-changing slices because
they change render-resource initialization and rebuild behavior:

```bash
cargo run --quiet --manifest-path native/Cargo.toml -p mclone-native-client -- \
  --screenshot /tmp/mclone-cpu-mesh-eviction.png --width 960 --height 540 --chunk-radius 5
```

Inspect the PNG before considering the slice complete.

If the implementation touches web first-upload behavior, also run the current
native-web render smoke from `docs/native-web.md`.

If the implementation touches XR resource creation or the upload coordinator,
run the desktop XR/headless validation path available on the current host, and
batch Android/Quest validation with the next Android/XR pass.

Slice 0 validation run:

```bash
cargo fmt --manifest-path native/Cargo.toml -p mclone-mesh -p mclone-render-session -p mclone-app-runtime -p mclone-xr-scene -p mclone-native-client -p mclone-web-client -p mclone-android-xr-client -- --check
cargo test --manifest-path native/Cargo.toml -p mclone-mesh -- --nocapture
cargo test --manifest-path native/Cargo.toml -p mclone-render-session -- --nocapture
cargo check --manifest-path native/Cargo.toml -p mclone-native-client -p mclone-app-runtime -p mclone-xr-scene
cargo check --manifest-path native/Cargo.toml -p mclone-web-client --target wasm32-unknown-unknown
cargo check --manifest-path native/Cargo.toml -p mclone-android-xr-client
```

Workspace-wide `cargo fmt --manifest-path native/Cargo.toml --all -- --check`
currently reports unrelated `mclone-physics` rustfmt drift; it was left out of
this scoped commit.

## Close Criteria

- Resident render-session cache no longer stores `TexturedRenderSectionMesh` or
  owned vertex/index vectors.
- Normal frame upload still sends only rebuilt/removed section diffs.
- Startup, headless, web first-upload, Android, and XR initialization no longer
  require `cached_sections() -> Vec<TexturedRenderSectionMesh>`.
- Resource rebuild has an explicit recompile/dirty-all path.
- Diagnostics still report resident section counts, uploaded section counts,
  vertex/index/face pressure, and visibility-graph stats.
- At least one rendered screenshot is visually inspected after the change.
- Byte accounting shows retained CPU mesh payload bytes fall to zero or near
  zero outside transient compile/upload queues.

## Follow-Ups

1. Add GPU buffer reuse/pooling if replacement-buffer allocation becomes visible
   after CPU retention is gone.
2. Consider Java-shaped reusable CPU builder packs for native desktop compile
   workers if transient allocation remains material.
3. Revisit translucent sort-state parity separately; vanilla retains compact
   sort state for translucent resorting, not a full CPU mesh.
4. Feed retained CPU mesh byte counters into long-running movement soaks if they
   remain useful for catching regressions.
