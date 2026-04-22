import positionColorJson from "../resources/shaders/core/position_color.json" with { type: "json" };
import positionTexJson from "../resources/shaders/core/position_tex.json" with { type: "json" };
import rendertypeSolidJson from "../resources/shaders/core/rendertype_solid.json" with { type: "json" };
import type { ShaderProgramJson } from "./shader-program";
import { ShaderProgramDefinition } from "./shader-program";

const DEFINITIONS = new Map<string, ShaderProgramDefinition>([
  ["position_color", new ShaderProgramDefinition("position_color", positionColorJson as ShaderProgramJson)],
  ["position_tex", new ShaderProgramDefinition("position_tex", positionTexJson as ShaderProgramJson)],
  ["rendertype_solid", new ShaderProgramDefinition("rendertype_solid", rendertypeSolidJson as ShaderProgramJson)],
]);

function getPositionColorStub(): string {
  return `
struct Uniforms {
  ModelViewMat: mat4x4<f32>,
  ProjMat: mat4x4<f32>,
  ColorModulator: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(
  @location(0) position: vec3<f32>,
  @location(1) color: vec4<f32>,
) -> VertexOutput {
  var output: VertexOutput;
  output.position = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(position, 1.0);
  output.color = color;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return input.color * uniforms.ColorModulator;
}
`;
}

function getRenderTypeSolidStub(): string {
  return `
struct Uniforms {
  ModelViewMat: mat4x4<f32>,
  ProjMat: mat4x4<f32>,
  ChunkOffset: vec3<f32>,
  _padding0: f32,
  ColorModulator: vec4<f32>,
  FogStart: f32,
  FogEnd: f32,
  _padding1: vec2<f32>,
  FogColor: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
  @location(1) uv0: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var sampler0Sampler: sampler;
@group(0) @binding(2) var sampler0Texture: texture_2d<f32>;
@group(0) @binding(3) var sampler2Sampler: sampler;
@group(0) @binding(4) var sampler2Texture: texture_2d<f32>;

@vertex
fn vs_main(
  @location(0) position: vec3<f32>,
  @location(1) color: vec4<f32>,
  @location(2) uv0: vec2<f32>,
  @location(3) uv2: vec2<i32>,
  @location(4) normal: vec4<f32>,
) -> VertexOutput {
  let _unusedLight = vec2<f32>(uv2);
  let _unusedNormal = normal;
  var output: VertexOutput;
  let worldPosition = position + uniforms.ChunkOffset;
  output.position = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(worldPosition, 1.0);
  output.color = color;
  output.uv0 = uv0;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let baseColor = textureSample(sampler0Texture, sampler0Sampler, input.uv0);
  let _unusedLight = textureSample(sampler2Texture, sampler2Sampler, vec2<f32>(0.5, 0.5));
  return baseColor * input.color * uniforms.ColorModulator;
}
`;
}

export function getShaderProgramDefinition(name: string): ShaderProgramDefinition {
  const definition = DEFINITIONS.get(name);
  if (!definition) {
    throw new Error(`Unknown shader program ${name}`);
  }

  return definition;
}

export function getStubShaderSource(name: string): string {
  switch (name) {
    case "position_color":
      return getPositionColorStub();
    case "rendertype_solid":
      return getRenderTypeSolidStub();
    default:
      throw new Error(`No stub WGSL registered for shader ${name}`);
  }
}
