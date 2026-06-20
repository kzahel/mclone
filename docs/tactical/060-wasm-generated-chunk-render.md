# 060: WASM Generated Chunk Render

Status: completed first pass.

## Purpose

Render one real server-generated chunk in the browser through the native Rust
runtime, mesh, and `wgpu` canvas path.

This builds directly on the clear-frame canvas bring-up from
[`059-wasm-canvas-render-bringup.md`](059-wasm-canvas-render-bringup.md). The
goal is not textured parity yet. The goal is to prove the end-to-end browser
path:

```text
browser page
  -> wasm-bindgen export
  -> WebRuntime loopback
  -> IntegratedServer generated ChunkSnapshot
  -> flat visible-face mesh
  -> mclone-render ChunkDrawResources
  -> WebGPU canvas pixels
```

## First Slice

Scope:

1. Keep the raw runtime report and clear-frame canvas export available.
2. Add a generated-chunk canvas export that:
   - requests chunk `(0, 0)` from the loopback runtime
   - confirms the client replica has that generated snapshot
   - rehydrates omitted all-air sections for mesh input
   - maps current generated `BlockStateId` values into the flat debug mesh ids
   - builds a `VisibleChunkMesh`
   - uploads it with `ChunkDrawResources`
   - renders through a depth target into the browser canvas
3. Extend the browser smoke JSON report with chunk mesh/render counts.
4. Extend Playwright validation to reject clear-only canvas screenshots.
5. Save screenshots under `/tmp`.

## Boundary Direction

Keep this slice asset-free and debug-rendered:

- Do not fetch or package vanilla textures yet.
- Do not move desktop app render-cache code wholesale into the web app.
- Keep the snapshot-to-flat-mesh conversion local and small until a shared
  runtime render-prep crate is justified.
- Continue to use `mclone-render` for GPU resources and draw calls.

## Deferred

- Textured block models and atlas upload in browser.
- Section-keyed streaming render cache in browser.
- Browser input and pointer lock.
- Continuous animation or requestAnimationFrame loop.
- Browser asset pack loading.

## Validation

Required checks:

```text
cargo test --manifest-path native/Cargo.toml -p mclone-web-client
pnpm --silent native:web:build
pnpm --silent native:web:smoke
pnpm --silent native:web:chunk-smoke
```

The strict chunk smoke saves:

- `/tmp/mclone-native-web-smoke.png`
- `/tmp/mclone-native-web-canvas.png`

## Next

After the flat generated chunk is stable, bring over a narrow textured path:
either a tiny generated atlas fixture or the existing baked atlas once web asset
loading is scoped. The first textured milestone should still be one generated
chunk, not continuous streaming.
