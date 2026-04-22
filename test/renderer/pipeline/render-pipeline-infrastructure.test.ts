import { describe, expect, test } from "vitest";
import { RenderPipelineCache, createRenderPipelineDescriptorSpec } from "../../../src/renderer/pipeline/render-pipeline-cache";
import { RenderType } from "../../../src/renderer/render-type";
import { getShaderProgramDefinition } from "../../../src/renderer/shader/shader-program-library";

describe("Render pipeline infrastructure", () => {
  test("solid render type maps to a triangle-list pipeline with depth writes and block vertex layout", () => {
    const spec = createRenderPipelineDescriptorSpec(RenderType.solid(), "rgba8unorm", "depth24plus");
    expect(spec.primitive.topology).toBe("triangle-list");
    expect(spec.primitive.cullMode).toBe("back");
    expect(spec.depthStencil).toEqual({
      format: "depth24plus",
      depthWriteEnabled: true,
      depthCompare: "less-equal",
    });
    expect(spec.target.blend).toBeUndefined();
    expect(spec.vertexBufferLayout.arrayStride).toBe(32);
    expect(Array.from(spec.vertexBufferLayout.attributes, (attribute) => attribute.format)).toEqual([
      "float32x3",
      "unorm8x4",
      "float32x2",
      "sint16x2",
      "snorm8x4",
    ]);
  });

  test("translucent render type maps to alpha blending from the translated transparency shard", () => {
    const spec = createRenderPipelineDescriptorSpec(RenderType.translucent(), "rgba8unorm", "depth24plus");
    expect(spec.target.blend).toEqual({
      color: {
        operation: "add",
        srcFactor: "src-alpha",
        dstFactor: "one-minus-src-alpha",
      },
      alpha: {
        operation: "add",
        srcFactor: "one",
        dstFactor: "one-minus-src-alpha",
      },
    });
  });

  test("rendertype_solid uniform layout matches the expected packed offsets", () => {
    const definition = getShaderProgramDefinition("rendertype_solid");
    expect(
      definition.uniforms.map((uniform) => ({
        name: uniform.spec.name,
        offset: uniform.offset,
        size: uniform.byteSize,
      })),
    ).toEqual([
      { name: "ModelViewMat", offset: 0, size: 64 },
      { name: "ProjMat", offset: 64, size: 64 },
      { name: "ChunkOffset", offset: 128, size: 12 },
      { name: "ColorModulator", offset: 144, size: 16 },
      { name: "FogStart", offset: 160, size: 4 },
      { name: "FogEnd", offset: 164, size: 4 },
      { name: "FogColor", offset: 176, size: 16 },
    ]);
    expect(definition.uniformBufferSize).toBe(192);
  });

  test("shader JSON sampler parsing produces one uniform binding plus sampler-texture pairs", () => {
    const definition = getShaderProgramDefinition("rendertype_solid");
    expect(definition.samplers).toEqual([
      { name: "Sampler0", samplerBinding: 1, textureBinding: 2 },
      { name: "Sampler2", samplerBinding: 3, textureBinding: 4 },
    ]);
    expect(definition.createBindGroupLayoutEntries()).toHaveLength(5);
  });

  test("pipeline cache reuses the same pipeline object for identical render state", () => {
    let pipelineCreations = 0;
    const fakeDevice = {
      createBindGroupLayout(descriptor: object) {
        return { descriptor };
      },
      createPipelineLayout(descriptor: object) {
        return { descriptor };
      },
      createShaderModule(descriptor: object) {
        return { descriptor };
      },
      createRenderPipeline(descriptor: object) {
        pipelineCreations++;
        return { descriptor, id: pipelineCreations };
      },
    } as unknown as GPUDevice;

    const cache = new RenderPipelineCache(fakeDevice);
    const first = cache.getOrCreate(RenderType.solid(), "rgba8unorm", "depth24plus");
    const second = cache.getOrCreate(RenderType.solid(), "rgba8unorm", "depth24plus");

    expect(second).toBe(first);
    expect(second.pipeline).toBe(first.pipeline);
    expect(pipelineCreations).toBe(1);
  });
});
