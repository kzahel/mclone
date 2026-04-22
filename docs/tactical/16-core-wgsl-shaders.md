# Tactical 16 — Core WGSL shader ports

Port the first real Minecraft core shader set from GLSL to WGSL and wire those sources into the pipeline cache from tactical 11. This slice replaces the stub `rendertype_solid` path used during early renderer bring-up and validates the adjacent block-renderer variants that the next chunk-rendering slices will depend on.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/assets/minecraft/shaders/core/position_color.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/position_tex.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/rendertype_solid.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/rendertype_cutout.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/rendertype_cutout_mipped.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/rendertype_translucent.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/rendertype_translucent_no_crumbling.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/rendertype_translucent_moving_block.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/rendertype_tripwire.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/core/rendertype_lines.*` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/include/fog.glsl` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/assets/minecraft/shaders/include/light.glsl` | `src/renderer/shader/shader-program-library.ts` |
| `reference/.../src/net/minecraft/client/renderer/RenderStateShard.java` | `src/renderer/render-state-shard.ts` |
| `reference/.../src/net/minecraft/client/renderer/RenderType.java` | `src/renderer/render-type.ts` |

## What landed

- `shader-program-library.ts` now serves real WGSL sources for `position_color`, `position_tex`, `rendertype_solid`, `cutout`, `cutout_mipped`, `translucent`, `translucent_no_crumbling`, `translucent_moving_block`, `tripwire`, and `lines`, with the matching shader JSON resources checked into `src/renderer/resources/shaders/core/`.
- `RenderStateShards` now points each translated render type at its real shader key instead of aliasing everything back to `rendertype_solid`, and `RenderType.lines()` / `RenderType.lineStrip()` are available with the translated line state/output-state wiring.
- `RenderPipelineCache` now creates shader modules from the real WGSL registry instead of the old stub generator.
- The browser smoke boot path now precompiles every tactical-16 render type so Chrome validates the whole shader set, not just the solid block path.
- WebGPU validation scopes were added around pipeline creation and submission in `main.ts` so shader/bind-group mistakes surface as explicit test failures instead of silent black frames.

## Scope choice

- Landed here: the block/core shader set needed by current block rendering plus `lines`, because the next chunk renderer slices depend on those contracts.
- Explicitly deferred: the broader entity/item/GUI shader matrix. Those stay with the later renderer slices that actually exercise them.

## Oracle / done-when

**Unit (Vitest):**

- `RenderType.lines()` maps to a `line-list` pipeline with the translated `POSITION_COLOR_NORMAL` layout.
- `rendertype_lines` uniform offsets match the JSON-derived packing.
- The shader library resolves WGSL sources and JSON definitions for every tactical-16 shader name.

**Browser smoke (Playwright + system Chrome):**

- The smoke boot path compiles all tactical-16 pipelines in Chrome.
- The existing block scene still renders correctly through the real `rendertype_solid` shader path.
- The screenshot is inspected manually before continuing.

## Done when

- `pnpm typecheck` passes
- `pnpm test` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 16 as done

## Next

Tactical 17 is now documented in [`17-chunk-section-compilation.md`](17-chunk-section-compilation.md). The next slice after that is tactical 18: `LevelRenderer`, `GameRenderer`, `Frustum`, `LightTexture`, and `FogRenderer` so compiled sections can be culled and submitted through a real camera-driven world frame.
