import { describe, expect, test } from "vitest";
import { MultiBufferSource } from "../../src/renderer/multi-buffer-source";
import { RenderBuffers } from "../../src/renderer/render-buffers";
import { RenderType } from "../../src/renderer/render-type";
import { BufferBuilder } from "../../src/renderer/vertex/buffer-builder";
import { DefaultVertexFormat } from "../../src/renderer/vertex/default-vertex-format";
import type { VertexConsumer } from "../../src/renderer/vertex/vertex-consumer";
import { VertexFormat } from "../../src/renderer/vertex/vertex-format";

function emitBlockQuad(consumer: VertexConsumer): void {
  for (const [x, y, z] of [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]] as const) {
    consumer.vertex(x, y, z, 1, 1, 1, 1, 0, 0, 0, 0xf000f0, 0, 1, 0);
  }
}

function emitLineVertex(consumer: VertexConsumer): void {
  consumer.vertex(0, 0, 0).color(255, 0, 0, 255).normal(1, 0, 0).endVertex();
}

describe("MultiBufferSource", () => {
  test("switching away from the shared immediate builder flushes the previous render type", () => {
    const sharedBuilder = new BufferBuilder(256);
    const source = MultiBufferSource.immediate(sharedBuilder);

    emitBlockQuad(source.getBuffer(RenderType.solid()));
    emitLineVertex(source.getBuffer(RenderType.lines()));
    source.endBatch();

    const first = sharedBuilder.popNextBuffer().drawState;
    const second = sharedBuilder.popNextBuffer().drawState;
    expect(first.format()).toBe(DefaultVertexFormat.BLOCK);
    expect(first.mode()).toBe(VertexFormat.Mode.QUADS);
    expect(first.vertexCount()).toBe(4);
    expect(second.format()).toBe(DefaultVertexFormat.POSITION_COLOR_NORMAL);
    expect(second.mode()).toBe(VertexFormat.Mode.LINES);
    expect(second.vertexCount()).toBe(2);
  });

  test("endLastBatch flushes only the active non-fixed builder", () => {
    const sharedBuilder = new BufferBuilder(256);
    const fixedBuilder = new BufferBuilder(RenderType.solid().bufferSize());
    const source = MultiBufferSource.immediateWithBuffers(new Map([[RenderType.solid(), fixedBuilder]]), sharedBuilder);

    emitBlockQuad(source.getBuffer(RenderType.solid()));
    emitLineVertex(source.getBuffer(RenderType.lines()));
    source.endLastBatch();

    expect(sharedBuilder.building()).toBe(false);
    expect(fixedBuilder.building()).toBe(true);

    const fallbackDraw = sharedBuilder.popNextBuffer().drawState;
    expect(fallbackDraw.format()).toBe(DefaultVertexFormat.POSITION_COLOR_NORMAL);

    source.endBatch(RenderType.solid());
    expect(fixedBuilder.building()).toBe(false);
    const fixedDraw = fixedBuilder.popNextBuffer().drawState;
    expect(fixedDraw.format()).toBe(DefaultVertexFormat.BLOCK);
  });
});

describe("RenderBuffers", () => {
  test("bufferSource uses the renderer-owned fixed buffer pack for chunk layer render types", () => {
    const renderBuffers = new RenderBuffers();
    const source = renderBuffers.bufferSource();
    const solidBuilder = renderBuffers.fixedBufferPack().builder(RenderType.solid());

    expect(source.getBuffer(RenderType.solid())).toBe(solidBuilder);
    source.endBatch(RenderType.solid());
    solidBuilder.discard();
  });
});
