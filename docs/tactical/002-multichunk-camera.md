# 002: Multi-Chunk Render And Camera

Status: completed.

Render a small generated chunk area and add enough native camera control to inspect it without recompiling. This keeps the renderer useful while still preserving headless screenshot validation as the acceptance gate.

## Dependencies

- [`001-one-generated-chunk-render.md`](001-one-generated-chunk-render.md)
- [`../native-engine-architecture.md`](../native-engine-architecture.md)

Use `~/code/playbox` only as a reference for camera/input/render-loop patterns. Keep the renderer data path voxel-specific and Minecraft-coordinate-native.

## Coordinate Contract

Preserve the existing world-space contract:

- `+X` east
- `+Y` up
- `+Z` south
- right-handed basis: `+X x +Y = +Z`
- chunk-local X/Z coordinates are `0..15`
- block cells occupy `[x, x + 1]`, `[y, y + 1]`, `[z, z + 1]`

Camera controls may move/orbit a view, but mesh positions and chunk coordinates must not be flipped.

## Scope

1. Generate a chunk square around a center chunk.
2. Mesh that area with neighbor-aware culling across loaded chunk boundaries.
3. Keep outside-area boundary faces visible.
4. Add `--chunk-radius <n>` to the native client, defaulting to a small area.
5. Make `--headless-chunk` render the area with a fixed overview camera.
6. Add minimal window controls:
   - WASD: move on the camera plane
   - Q/E: move down/up
   - mouse drag: orbit
   - mouse wheel: zoom
   - Escape: quit

## Out Of Scope

- dynamic streaming
- async generation
- lighting
- greedy meshing
- texture atlas/model baking
- selection/raycasting
- UI overlays
- OpenXR

## Validation

Required for this slice:

```bash
cargo test --workspace
cargo check -p mclone-web-client --target wasm32-unknown-unknown
cargo run -p mclone-native-client -- --headless-chunk /tmp/mclone-native-chunk-area.png --width 960 --height 640 --chunk-radius 1
git diff --check
```

Then inspect `/tmp/mclone-native-chunk-area.png`. The image should show a nonblank, framed multi-chunk terrain area without artificial walls between loaded chunks.

Windowed validation:

```bash
cargo run -p mclone-native-client
```

## Next Slice

After this:

1. add a small chunk cache/runtime boundary instead of building all scene data in `main.rs`
2. split terrain mesh generation from render upload lifetime
3. add initial sunlight/skylight data only after unlit multi-chunk geometry remains visually stable
