# 001: One Generated Chunk Render

Status: completed.

Render one generated overworld chunk from the native Rust pipeline. This is the first end-to-end path from seed-parity worldgen data to a GPU screenshot.

## Dependencies

- [`000-native-render-bringup.md`](000-native-render-bringup.md)
- [`../native-engine-architecture.md`](../native-engine-architecture.md)

Use `~/code/playbox` only as a reference for `wgpu` buffer/pipeline/depth/headless patterns. Keep this slice specific to voxel rendering and do not import Playbox code or architecture.

## Coordinate Contract

This slice must preserve the `000` Minecraft world-space contract:

- `+X` east
- `+Y` up
- `+Z` south
- right-handed basis: `+X x +Y = +Z`
- chunk-local X/Z coordinates are `0..15`
- block cell `(x, y, z)` occupies `[x, x + 1]`, `[y, y + 1]`, `[z, z + 1]`

The renderer may use a right-handed view/projection matrix, but mesh positions must stay in Minecraft world coordinates. Do not flip chunk data to make camera math convenient.

## Scope

1. Add a renderer-agnostic generated chunk artifact in `mclone_worldgen`.
2. Add simple visible-face meshing in `mclone_mesh`.
3. Add a flat-color chunk pipeline in `mclone_render`.
4. Make `mclone-native-client` render one generated chunk in windowed mode.
5. Add `--headless-chunk <path>` so the same mesh can be validated by PNG capture.

## Out Of Scope

- texture atlas loading
- Minecraft model/blockstate baking
- greedy meshing
- chunk-neighbor face culling
- dynamic chunk streaming
- lighting
- transparency sorting
- camera controls beyond a fixed overview camera
- OpenXR

## Rendering Rules

- Mesh positions are world-space floats derived directly from chunk/block coordinates.
- The first mesher may emit one quad per visible block face.
- Air is empty. Other generated block IDs are solid for this first pass.
- Face color is material color multiplied by a simple face-direction shade.
- The first renderer should use a depth buffer and no back-face culling until winding/camera assumptions are visually verified.

## Validation

Required for this slice:

```bash
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-chunk.png --width 640 --height 480
git diff --check
```

Then inspect `/tmp/mclone-native-chunk.png`. The image should show a nonblank, framed, flat-colored generated chunk with visible terrain shape.

Windowed validation is useful when a display is available:

```bash
cargo run -p mclone-native-client
```

## Next Slice

After one generated chunk renders:

- [`002-multichunk-camera.md`](002-multichunk-camera.md)
