export interface GuiSolidPipeline {
  readonly pipeline: GPURenderPipeline;
}

export interface GuiTexturedPipeline {
  readonly pipeline: GPURenderPipeline;
  readonly bindGroupLayout: GPUBindGroupLayout;
}

const GUI_SOLID_WGSL = /* wgsl */ `
struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) color: vec4f,
};

@vertex
fn vertexMain(
  @location(0) position: vec2f,
  @location(1) color: vec4f,
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4f(position, 0.0, 1.0);
  out.color = color;
  return out;
}

@fragment
fn fragmentMain(in: VertexOut) -> @location(0) vec4f {
  return in.color;
}
`;

const GUI_TEXTURED_WGSL = /* wgsl */ `
@group(0) @binding(0) var guiSampler: sampler;
@group(0) @binding(1) var guiTexture: texture_2d<f32>;

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
  @location(1) color: vec4f,
};

@vertex
fn vertexMain(
  @location(0) position: vec2f,
  @location(1) uv: vec2f,
  @location(2) color: vec4f,
) -> VertexOut {
  var out: VertexOut;
  out.position = vec4f(position, 0.0, 1.0);
  out.uv = uv;
  out.color = color;
  return out;
}

@fragment
fn fragmentMain(in: VertexOut) -> @location(0) vec4f {
  return textureSample(guiTexture, guiSampler, in.uv) * in.color;
}
`;

const ALPHA_BLEND: GPUBlendState = {
  color: {
    srcFactor: "src-alpha",
    dstFactor: "one-minus-src-alpha",
    operation: "add",
  },
  alpha: {
    srcFactor: "one",
    dstFactor: "one-minus-src-alpha",
    operation: "add",
  },
};

export function createGuiSolidPipeline(device: GPUDevice, format: GPUTextureFormat): GuiSolidPipeline {
  const module = device.createShaderModule({
    label: "gui-solid-shader",
    code: GUI_SOLID_WGSL,
  });
  return {
    pipeline: device.createRenderPipeline({
      label: `gui-solid-${format}`,
      layout: "auto",
      vertex: {
        module,
        entryPoint: "vertexMain",
        buffers: [
          {
            arrayStride: 24,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x2" },
              { shaderLocation: 1, offset: 8, format: "float32x4" },
            ],
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fragmentMain",
        targets: [{ format, blend: ALPHA_BLEND }],
      },
      primitive: {
        topology: "triangle-list",
        cullMode: "none",
      },
    }),
  };
}

export function createGuiTexturedPipeline(device: GPUDevice, format: GPUTextureFormat): GuiTexturedPipeline {
  const module = device.createShaderModule({
    label: "gui-textured-shader",
    code: GUI_TEXTURED_WGSL,
  });
  const bindGroupLayout = device.createBindGroupLayout({
    label: "gui-textured-bind-group-layout",
    entries: [
      {
        binding: 0,
        visibility: GPUShaderStage.FRAGMENT,
        sampler: { type: "filtering" },
      },
      {
        binding: 1,
        visibility: GPUShaderStage.FRAGMENT,
        texture: { sampleType: "float" },
      },
    ],
  });
  return {
    pipeline: device.createRenderPipeline({
      label: `gui-textured-${format}`,
      layout: device.createPipelineLayout({
        label: `gui-textured-layout-${format}`,
        bindGroupLayouts: [bindGroupLayout],
      }),
      vertex: {
        module,
        entryPoint: "vertexMain",
        buffers: [
          {
            arrayStride: 32,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x2" },
              { shaderLocation: 1, offset: 8, format: "float32x2" },
              { shaderLocation: 2, offset: 16, format: "float32x4" },
            ],
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fragmentMain",
        targets: [{ format, blend: ALPHA_BLEND }],
      },
      primitive: {
        topology: "triangle-list",
        cullMode: "none",
      },
    }),
    bindGroupLayout,
  };
}
