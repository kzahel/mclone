import { CompositeRenderType } from "../render-type";
import { getShaderProgramDefinition, getShaderSource } from "../shader/shader-program-library";
import { type ShaderProgramDefinition } from "../shader/shader-program";
import { VertexFormat, VertexFormatMode } from "../vertex/vertex-format";
import { VertexFormatElement } from "../vertex/vertex-format-element";

const COLOR_WRITE_ALL = 0xf;

function colorWriteMask(writeColor: boolean): GPUColorWriteFlags {
  return writeColor ? COLOR_WRITE_ALL : 0;
}

function primitiveTopology(mode: VertexFormatMode): GPUPrimitiveTopology {
  switch (mode) {
    case VertexFormat.Mode.LINES:
      return "line-list";
    case VertexFormat.Mode.LINE_STRIP:
      return "line-strip";
    case VertexFormat.Mode.TRIANGLES:
    case VertexFormat.Mode.QUADS:
      return "triangle-list";
    case VertexFormat.Mode.TRIANGLE_STRIP:
      return "triangle-strip";
    default:
      throw new Error(`Unsupported primitive mode ${mode.asGLMode}`);
  }
}

function vertexFormatForElement(name: string, element: VertexFormatElement): GPUVertexFormat {
  switch (name) {
    case "Position":
      return "float32x3";
    case "Color":
      return "unorm8x4";
    case "UV":
    case "UV0":
      return "float32x2";
    case "UV1":
    case "UV2":
      return element.getType() === VertexFormatElement.Type.USHORT ? "uint16x2" : "sint16x2";
    case "Normal":
      return "snorm8x4";
    default:
      throw new Error(`Unsupported vertex element ${name}`);
  }
}

function createVertexBufferLayout(format: VertexFormat): GPUVertexBufferLayout {
  const names = format.getElementAttributeNames();
  const offsets = format.getOffsets();
  const elements = format.getElements();
  const attributes: GPUVertexAttribute[] = [];
  for (let index = 0; index < names.length; index++) {
    const element = elements[index]!;
    if (element.getUsage() === VertexFormatElement.Usage.PADDING) {
      continue;
    }

    attributes.push({
      shaderLocation: attributes.length,
      offset: offsets[index]!,
      format: vertexFormatForElement(names[index]!, element),
    });
  }

  return {
    arrayStride: format.getVertexSize(),
    attributes,
  };
}

export interface RenderPipelineDescriptorSpec {
  readonly primitive: GPUPrimitiveState;
  readonly depthStencil?: GPUDepthStencilState;
  readonly target: GPUColorTargetState;
  readonly vertexBufferLayout: GPUVertexBufferLayout;
}

export function createRenderPipelineDescriptorSpec(
  renderType: CompositeRenderType,
  colorFormat: GPUTextureFormat,
  depthFormat?: GPUTextureFormat,
): RenderPipelineDescriptorSpec {
  const state = renderType.state();
  const blend = state.transparencyState.getBlendState();
  return {
    primitive: {
      topology: primitiveTopology(renderType.mode()),
      cullMode: state.cullState.isEnabled() ? "back" : "none",
      frontFace: "ccw",
    },
    depthStencil: depthFormat
      ? {
          format: depthFormat,
          depthWriteEnabled: state.writeMaskState.writesDepth(),
          depthCompare: state.depthTestState.getCompareFunction(),
        }
      : undefined,
    target: {
      format: colorFormat,
      blend,
      writeMask: colorWriteMask(state.writeMaskState.writesColor()),
    },
    vertexBufferLayout: createVertexBufferLayout(renderType.format()),
  };
}

export interface CachedRenderPipeline {
  readonly pipeline: GPURenderPipeline;
  readonly bindGroupLayout: GPUBindGroupLayout;
  readonly shaderProgram: ShaderProgramDefinition;
  readonly spec: RenderPipelineDescriptorSpec;
}

function cacheKey(renderType: CompositeRenderType, colorFormat: GPUTextureFormat, depthFormat?: GPUTextureFormat): string {
  const state = renderType.state();
  return JSON.stringify({
    name: renderType.name,
    mode: renderType.mode().asGLMode,
    format: renderType.format().toString(),
    shader: state.shaderState.getShaderName(),
    cull: state.cullState.isEnabled(),
    depthWrite: state.writeMaskState.writesDepth(),
    colorWrite: state.writeMaskState.writesColor(),
    depthCompare: state.depthTestState.getCompareFunction(),
    blend: state.transparencyState.toString(),
    colorFormat,
    depthFormat: depthFormat ?? "none",
  });
}

export class RenderPipelineCache {
  private readonly cache = new Map<string, CachedRenderPipeline>();

  public constructor(private readonly device: GPUDevice) {}

  public getOrCreate(
    renderType: CompositeRenderType,
    colorFormat: GPUTextureFormat,
    depthFormat?: GPUTextureFormat,
  ): CachedRenderPipeline {
    const key = cacheKey(renderType, colorFormat, depthFormat);
    const cached = this.cache.get(key);
    if (cached) {
      return cached;
    }

    const shaderName = renderType.state().shaderState.getShaderName();
    if (!shaderName) {
      throw new Error(`RenderType ${renderType.name} has no shader state`);
    }

    const shaderProgram = getShaderProgramDefinition(shaderName);
    const bindGroupLayout = this.device.createBindGroupLayout({
      entries: shaderProgram.createBindGroupLayoutEntries(),
    });
    const pipelineLayout = this.device.createPipelineLayout({
      bindGroupLayouts: [bindGroupLayout],
    });
    const spec = createRenderPipelineDescriptorSpec(renderType, colorFormat, depthFormat);
    const module = this.device.createShaderModule({
      code: getShaderSource(shaderName),
    });
    const pipeline = this.device.createRenderPipeline({
      layout: pipelineLayout,
      vertex: {
        module,
        entryPoint: "vs_main",
        buffers: [spec.vertexBufferLayout],
      },
      fragment: {
        module,
        entryPoint: "fs_main",
        targets: [spec.target],
      },
      primitive: spec.primitive,
      depthStencil: spec.depthStencil,
    });

    const created: CachedRenderPipeline = {
      pipeline,
      bindGroupLayout,
      shaderProgram,
      spec,
    };
    this.cache.set(key, created);
    return created;
  }
}
