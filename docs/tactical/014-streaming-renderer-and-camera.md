# 014: Streaming Renderer And Camera

Status: completed.

## Purpose

Move the native textured renderer off the single all-scene mesh from `013-vanilla-section-meshing.md` and onto keyed render-section ownership. This is the renderer runtime boundary needed for later frustum culling, dirty section replacement, async meshing, lighting, and movement-driven chunk-interest updates.

The durable shape is:

```text
ClientRuntime chunk snapshots
  -> TexturedChunkMeshInput per loaded chunk
  -> RenderSectionKey + TexturedRenderSectionMesh
  -> TexturedSectionDrawResources keyed GPU upload cache
  -> native window / headless scenario capture
```

## Scope

Landed:

- `RenderSectionKey` and `TexturedRenderSectionMesh` in `mclone-mesh`
- textured render-section generation split on 16-block vertical section boundaries
- neighbor occlusion still samples the full visible input area, including across vertical section boundaries
- compatibility combined area mesh now merges the section outputs
- `TexturedSectionDrawResources` in `mclone-render` with a keyed GPU buffer map and full-set visible-section update/removal
- headless textured-section capture path that exercises the same draw resources as the native window
- native client scene building now produces section sets from `ClientRuntime` snapshots
- `--headless-chunk-scenarios <dir>` for repeatable overview/orbit/close camera captures under `/tmp`
- native window mode uploads and draws the section set instead of one combined mesh

Kept out:

- true geometric frustum rejection
- dirty-only section upload diffs
- async mesh rebuild scheduling
- render-layer sorting, liquids, ambient occlusion, and packed light
- browser renderer consumption of these native textured section resources

## Architecture

The renderer now has two levels of textured draw API:

- `TexturedChunkDrawResources`: compatibility helper for one combined mesh
- `TexturedSectionDrawResources`: active native-client path, keyed by `RenderSectionKey`

The first implementation deliberately performs a full-set update when a scene is created. That is still useful because it establishes the correct ownership boundary: stale section keys can be removed, each section has independent GPU buffers, and the caller no longer treats the entire world view as one immutable mesh.

True frustum culling should build on this by filtering the section-key set before upload/draw. Dirty invalidation should replace only changed keys once chunk updates and light/liquid deltas start publishing section-level dirtiness.

## Divergence Notes

Vanilla Java owns compiled section rendering through `ChunkRenderDispatcher`, `RenderChunk`, `ViewArea`, and camera/frustum orchestration in `LevelRenderer`. This slice does not port those classes directly. It lands the native runtime boundary that matches their important ownership facts: render sections are independently rebuildable and independently drawable, while the client/world replica remains the source of chunk facts.

The current Rust renderer keeps world-space vertex positions in each section mesh, so no per-section model transform is needed yet. This keeps the first section cache small and easy to validate. A later renderer slice can add section-local coordinates if GPU precision, culling, or instancing pressure makes that useful.

## Validation

Required checks:

- `cargo test --workspace`
- `cargo check -p mclone-mesh --target wasm32-unknown-unknown`
- `cargo check -p mclone-render --target wasm32-unknown-unknown`
- `cargo check -p mclone-web-client --target wasm32-unknown-unknown`
- `cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-streaming-chunk.png --width 960 --height 640 --chunk-radius 1`
- `cargo run -p mclone-native-client -- --headless-chunk-scenarios /tmp/mclone-native-camera-scenarios --width 960 --height 640 --chunk-radius 1`
- inspect `/tmp/mclone-native-streaming-chunk.png` and at least one scenario image
- `pnpm native:web:smoke`

## Follow-Up

Proceed to `015-decoration-framework-foundation.md`: start the worldgen decoration framework and first visible feature families now that the renderer has a section-level runtime boundary. Fold true frustum rejection and dirty-only section replacement into the next renderer/runtime slice that introduces live chunk movement or section deltas.
