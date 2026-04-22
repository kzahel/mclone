import { describe, expect, test } from "vitest";
import { BufferBuilder } from "../../../src/renderer/vertex/buffer-builder";
import { BufferVertexConsumer } from "../../../src/renderer/vertex/buffer-vertex-consumer";
import { DefaultVertexFormat } from "../../../src/renderer/vertex/default-vertex-format";
import { PoseStack } from "../../../src/renderer/vertex/pose-stack";
import { VertexFormat } from "../../../src/renderer/vertex/vertex-format";

const LITTLE_ENDIAN = true;

function bytesOf(view: Uint8Array | Float32Array): number[] {
  return Array.from(view);
}

describe("Vertex buffer layer", () => {
  test("default vertex formats preserve Minecraft byte sizes", () => {
    expect(DefaultVertexFormat.BLOCK.getVertexSize()).toBe(32);
    expect(DefaultVertexFormat.NEW_ENTITY.getVertexSize()).toBe(36);
  });

  test("quad index count expands to two triangles", () => {
    expect(VertexFormat.Mode.QUADS.indexCount(4)).toBe(6);
  });

  test("BufferBuilder BLOCK fast path packs the expected byte layout", () => {
    const builder = new BufferBuilder(256);
    builder.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.BLOCK);
    builder.vertex(
      1,
      2,
      3,
      0x11 / 255,
      0x22 / 255,
      0x33 / 255,
      0x44 / 255,
      0.5,
      0.25,
      0x11223344,
      0x77885566,
      1,
      -1,
      0,
    );
    builder.end();

    const { drawState, buffer } = builder.popNextBuffer();
    expect(drawState.vertexBufferSize()).toBe(32);
    expect(drawState.indexCount()).toBe(0);
    expect(drawState.sequentialIndex()).toBe(true);

    const expected = new Uint8Array(32);
    const dataView = new DataView(expected.buffer);
    dataView.setFloat32(0, 1, LITTLE_ENDIAN);
    dataView.setFloat32(4, 2, LITTLE_ENDIAN);
    dataView.setFloat32(8, 3, LITTLE_ENDIAN);
    expected[12] = 0x11;
    expected[13] = 0x22;
    expected[14] = 0x33;
    expected[15] = 0x44;
    dataView.setFloat32(16, 0.5, LITTLE_ENDIAN);
    dataView.setFloat32(20, 0.25, LITTLE_ENDIAN);
    dataView.setInt16(24, 0x5566, LITTLE_ENDIAN);
    dataView.setInt16(26, 0x7788, LITTLE_ENDIAN);
    expected[28] = BufferVertexConsumer.normalIntValue(1);
    expected[29] = BufferVertexConsumer.normalIntValue(-1);
    expected[30] = BufferVertexConsumer.normalIntValue(0);
    expected[31] = 0;

    expect(bytesOf(buffer.subarray(0, 32))).toEqual(bytesOf(expected));
  });

  test("PoseStack push, translate, and pop returns to identity", () => {
    const stack = new PoseStack();
    const poseBefore = bytesOf(stack.last().pose().toFloat32Array());
    const normalBefore = bytesOf(stack.last().normal().toFloat32Array());

    stack.pushPose();
    stack.translate(2, -3, 4);
    stack.popPose();

    expect(bytesOf(stack.last().pose().toFloat32Array())).toEqual(poseBefore);
    expect(bytesOf(stack.last().normal().toFloat32Array())).toEqual(normalBefore);
    expect(stack.clear()).toBe(true);
  });
});
