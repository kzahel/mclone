# Tactical 10 — Vertex buffer layer

Port the `com.mojang.blaze3d.vertex` CPU/GPU pipeline: vertex format definitions, CPU-side buffer accumulation (`BufferBuilder`), transformation stack (`PoseStack`), and GPU-side buffer management (`VertexBuffer`). This is the plumbing every renderer slice from 11 onward builds on.

## Source files (read before writing)

| Java source | TS target |
|---|---|
| `reference/…/src/com/mojang/blaze3d/vertex/VertexFormatElement.java` | `src/renderer/vertex/vertex-format-element.ts` |
| `reference/…/src/com/mojang/blaze3d/vertex/VertexFormat.java` | `src/renderer/vertex/vertex-format.ts` |
| `reference/…/src/com/mojang/blaze3d/vertex/DefaultVertexFormat.java` | `src/renderer/vertex/default-vertex-format.ts` |
| `reference/…/src/com/mojang/blaze3d/vertex/BufferBuilder.java` | `src/renderer/vertex/buffer-builder.ts` |
| `reference/…/src/com/mojang/blaze3d/vertex/VertexBuffer.java` | `src/renderer/vertex/vertex-buffer.ts` |
| `reference/…/src/com/mojang/blaze3d/vertex/PoseStack.java` | `src/renderer/vertex/pose-stack.ts` |
| `reference/…/src/com/mojang/math/Matrix4f.java` | `src/renderer/math/matrix4f.ts` |
| `reference/…/src/com/mojang/math/Matrix3f.java` | `src/renderer/math/matrix3f.ts` |

## What to port and what to skip

### Direct translation (no divergence)

- `VertexFormatElement` — Type enum (FLOAT/UBYTE/BYTE/SHORT/UINT/INT), Usage enum (POSITION/NORMAL/COLOR/UV/PADDING/GENERIC), `index`, `count`, `byteSize`. Skip the GL `setupBufferState`/`clearBufferState` lambdas.
- `VertexFormat` — `ImmutableList<VertexFormatElement> elements`, `IntList offsets`, `int vertexSize`. Mode enum (LINES/TRIANGLES/QUADS + `indexCount(vertexCount)`). IndexType enum (BYTE/SHORT/INT + `least(n)`). Skip GL object IDs and `setupBufferState`.
- `DefaultVertexFormat` — All static element and format instances verbatim. BLOCK is 32 bytes (pos 12 + color 4 + uv0 8 + uv2 4 + normal 3 + padding 1). Verify sizes in unit tests.
- `BufferBuilder` — `DataView`-backed buffer, `begin(mode, format)`, `vertex(...)`, `endVertex()`, `end()`, `popNextBuffer()`. Preserve the `fastFormat` optimization path for BLOCK and NEW_ENTITY (manual byte packing). Preserve `DrawState` inner type.
- `PoseStack` — `Pose` (Matrix4f + Matrix3f), push/pop/translate/scale/mulPose. Direct port of the normal-matrix update in `scale()`.
- `Matrix4f` / `Matrix3f` — Port as needed for PoseStack. 32-float and 9-float column-major arrays. Only the operations PoseStack uses: mul, translate, scale, fromQuaternion.

### WebGPU divergences (callout required)

- `VertexBuffer` owns a `GPUBuffer` instead of a GL VAO/VBO/IBO triple. `upload(bufferBuilder)` maps the GPUBuffer and memcpys the vertex bytes. `draw(passEncoder)` calls `passEncoder.draw()` or `passEncoder.drawIndexed()`.
- `VertexFormat.Mode.QUADS`: WebGPU has no QUADS primitive. Emit a sequential index buffer with the pattern `[0,1,2, 0,2,3]` per quad (identical semantics to MC's sequential IBO, but triangle-list). This happens inside `VertexBuffer.upload()`.
- No `RenderSystem` singleton. The `GPUDevice` is passed explicitly to `VertexBuffer` rather than accessed globally.

## Oracle / done-when

**Unit (Vitest, no browser):**
- `DefaultVertexFormat.BLOCK.getVertexSize() === 32`
- `DefaultVertexFormat.NEW_ENTITY.getVertexSize() === 36`
- `VertexFormat.Mode.QUADS.indexCount(4) === 6` (4 verts → 2 triangles → 6 indices)
- `BufferBuilder` with BLOCK format: `begin()` → one `vertex()` call → `endVertex()` → `end()` → `popNextBuffer()` yields a ByteBuffer whose bytes match hand-computed expected layout (position at offset 0, color at 12, uv0 at 16, uv2 at 24, normal at 28).
- PoseStack push/translate/pop round-trips identity.

**Smoke (Playwright / WebGPU):**
- Render a single solid-color quad (2 triangles) to the canvas using our `BufferBuilder` → `VertexBuffer` pipeline. Clear color differs from quad color. Canvas pixel at quad center matches quad color. This is the first non-trivial draw call.

## Done when

- All unit tests pass (`pnpm test`)
- Smoke test passes (`pnpm test:browser`)
- `VertexBuffer` uploads and draws without WebGPU validation errors in system Chrome

## Next

Tactical 11: `RenderType` + `RenderStateShard` → `GPURenderPipeline` cache + bind group layouts. Needs `VertexFormat` (this slice) to describe vertex buffer layouts to the pipeline descriptor.
