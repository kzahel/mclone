# Tactical 11 — Render pipeline infrastructure

Port the render-state/pipeline layer that sits on top of the vertex buffer stack from tactical 10: `RenderStateShard`, `RenderType`, shader JSON parsing, and a WebGPU pipeline cache. This slice replaces the ad hoc pipeline setup in `src/renderer/main.ts` with something shaped like Minecraft's real renderer state.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/client/renderer/RenderStateShard.java` | `src/renderer/render-state-shard.ts` |
| `reference/.../src/net/minecraft/client/renderer/RenderType.java` | `src/renderer/render-type.ts` |
| `reference/.../src/net/minecraft/client/renderer/ShaderInstance.java` | `src/renderer/shader/shader-program.ts` |
| `reference/.../src/com/mojang/blaze3d/shaders/Uniform.java` | `src/renderer/shader/shader-program.ts` |
| `reference/.../src/com/mojang/blaze3d/shaders/BlendMode.java` | `src/renderer/shader/shader-program.ts` |
| `reference/.../src/assets/minecraft/shaders/core/*.json` | `src/renderer/resources/shaders/core/*.json` |

## What to port and what to skip

### Direct translation (no divergence)

- `RenderStateShard` shape and nested shard types: shader, texture, transparency, depth test, cull, write mask, lightmap, overlay, layering, output, texturing, line.
- `RenderType` core structure: `CompositeState`, `CompositeStateBuilder`, `CompositeRenderType`, `OutlineProperty`, plus the chunk-layer render types (`solid`, `cutoutMipped`, `cutout`, `translucent`, `tripwire`) and one color-only type for smoke (`lightning`).
- Shader JSON parsing from `ShaderInstance`: attributes, samplers, uniforms, blend metadata, default uniform values.
- `Uniform.getTypeFromString(...)` semantics and the scalar/vector expansion logic from `ShaderInstance.parseUniformNode(...)`.

### WebGPU divergences (callout required)

- `RenderStateShard` setup/clear runnables do not toggle global `RenderSystem` state. `RenderType` state is lowered into `GPURenderPipelineDescriptor` fields instead.
- Shader JSON does not compile GLSL in this slice. It produces bind-group layouts, uniform buffer layouts, and selects stub WGSL modules with the same attribute/uniform surface as the Minecraft program.
- Samplers become WebGPU sampler/texture binding pairs in a bind group instead of GL uniform slots.

## Oracle / done-when

**Unit (Vitest, no browser):**

- `RenderType.solid()` maps to a pipeline descriptor with `triangle-list`, back-face culling, depth write enabled, and a `BLOCK` vertex layout.
- `RenderType.translucent()` maps to alpha blending from the translated transparency shard.
- `rendertype_solid.json` uniform layout matches the expected packed offsets for `ModelViewMat`, `ProjMat`, `ChunkOffset`, `ColorModulator`, `FogStart`, `FogEnd`, `FogColor`.
- Shader JSON sampler parsing produces one uniform-buffer binding plus sampler/texture bindings for each declared sampler.
- Pipeline cache returns the same cached pipeline object for the same render-type/shader/target tuple.

**Smoke (Playwright / WebGPU):**

- `src/renderer/main.ts` renders the quad through `RenderType` + shader-json-derived bind groups + WebGPU pipeline cache, not an ad hoc pipeline.
- Browser smoke still passes and the center pixel matches the quad color.
- A screenshot is captured and visually inspected before moving on.

## Done when

- `pnpm test` passes
- `pnpm typecheck` passes
- `pnpm test:browser` passes
- `docs/tactical/README.md` links this slice and marks tactical 10 as done

## Next

Tactical 12: texture ingestion and atlas plumbing (`NativeImage`, `TextureAtlas`, `TextureAtlasSprite`, `Stitcher`, mip generation, GPU texture upload).
