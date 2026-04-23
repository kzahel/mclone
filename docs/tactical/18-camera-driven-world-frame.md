# Tactical 18 — Camera-driven world frame

Port the first camera-owned world frame path from Minecraft's client renderer so chunk sections are selected, culled, lit, fogged, and submitted through `LevelRenderer` / `GameRenderer` instead of a bespoke smoke-scene matrix. This slice still uses the static smoke level, but the frame assembly now follows the translated renderer structure closely enough that the next step can swap in real generated chunks.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/renderer/LevelRenderer.java` (`prepareCullFrustum`, `setupRender`, `updateRenderChunks`, `renderChunkLayer`, `compileChunksUntil`) | `src/renderer/level-renderer.ts` |
| `reference/.../src/net/minecraft/client/renderer/GameRenderer.java` (`getProjectionMatrix`, `getDepthFar`, `getDarkenWorldAmount`, `renderLevel`) | `src/renderer/game-renderer.ts` |
| `reference/.../src/net/minecraft/client/renderer/culling/Frustum.java` | `src/renderer/culling/frustum.ts` |
| `reference/.../src/net/minecraft/client/renderer/LightTexture.java` | `src/renderer/light-texture.ts` |
| `reference/.../src/net/minecraft/client/renderer/FogRenderer.java` | `src/renderer/fog-renderer.ts` |
| `reference/.../src/net/minecraft/client/Camera.java` | `src/renderer/camera.ts` |
| `reference/.../src/net/minecraft/world/phys/AABB.java` | `src/world/phys/aabb.ts` |
| `reference/.../src/com/mojang/math/Matrix4f.java` (`transpose`, `perspective`, `orthographic`) | `src/renderer/math/matrix4f.ts` |
| `reference/.../src/com/mojang/math/Vector4f.java` | `src/renderer/math/vector4f.ts` |

## What landed

- `Frustum` now ports the vanilla plane-extraction and AABB visibility checks, and `RenderChunk` now tracks its section AABB for camera/frustum tests.
- `LevelRenderer` now owns `prepareCullFrustum(...)`, camera-driven `setupRender(...)`, BFS render-chunk discovery, visible-layer collection, and the WebGPU-facing frame description that replaces the old handwritten smoke draw list.
- `GameRenderer` now builds the projection matrix, owns a translated `Camera`, updates the lightmap, prepares the culling frustum, and asks `LevelRenderer` for a world frame instead of `main.ts` hand-authoring matrices and chunk offsets.
- `LightTexture` now uploads a real 16x16 WebGPU lightmap texture, and block draw bind groups now bind that texture on `Sampler2` instead of the old placeholder white texture.
- `FogRenderer` now drives clear color and fog uniforms from the translated frame path, and the browser smoke scene now renders through the camera-owned frame setup with camera-relative chunk offsets.

## Scope choice

- Landed here: camera transforms, projection/frustum math, visible-section selection, fog/light uniform plumbing, and the first `GameRenderer -> LevelRenderer` world frame.
- Explicitly deferred: the full sky pass, clouds, weather, entities, hand rendering, and post-processing targets. Those remain outside MVP and do not need to block chunk/world integration.

## Oracle / done-when

**Unit (Vitest):**

- `Frustum` accepts an AABB in front of the camera and rejects one behind it.
- The camera-driven `GameRenderer -> LevelRenderer` frame path yields solid chunk draws only in front of the camera for the smoke scene.
- `LightTexture` still exposes the vanilla packed-light constants and full-bright decode helpers.

**Browser smoke (Playwright + system Chrome):**

- The smoke scene renders through `GameRenderer -> LevelRenderer -> ChunkRenderDispatcher` with camera-relative chunk offsets, fog uniforms, and a real lightmap texture bound at `Sampler2`.
- The canvas center pixel still matches the expected shaded orange-wool texel.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 18 as done

## Next

Tactical 19 is now documented in [`19-generated-world-chunk-bridge.md`](19-generated-world-chunk-bridge.md). The next slice after that is tactical 20: surface-special blocks and biome tint so the generated-world bridge can stop dropping fluids/snow and start rendering grass with biome color.
