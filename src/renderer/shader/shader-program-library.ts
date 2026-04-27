import positionColorJson from "../resources/shaders/core/position_color.json" with { type: "json" };
import positionTexJson from "../resources/shaders/core/position_tex.json" with { type: "json" };
import rendertypeCutoutJson from "../resources/shaders/core/rendertype_cutout.json" with { type: "json" };
import rendertypeCutoutMippedJson from "../resources/shaders/core/rendertype_cutout_mipped.json" with { type: "json" };
import rendertypeEntityCutoutJson from "../resources/shaders/core/rendertype_entity_cutout.json" with { type: "json" };
import rendertypeEntityCutoutNoCullJson from "../resources/shaders/core/rendertype_entity_cutout_no_cull.json" with { type: "json" };
import rendertypeEntitySolidJson from "../resources/shaders/core/rendertype_entity_solid.json" with { type: "json" };
import rendertypeEntityTranslucentJson from "../resources/shaders/core/rendertype_entity_translucent.json" with { type: "json" };
import rendertypeLinesJson from "../resources/shaders/core/rendertype_lines.json" with { type: "json" };
import rendertypeSolidJson from "../resources/shaders/core/rendertype_solid.json" with { type: "json" };
import rendertypeTranslucentJson from "../resources/shaders/core/rendertype_translucent.json" with { type: "json" };
import rendertypeTranslucentMovingBlockJson from "../resources/shaders/core/rendertype_translucent_moving_block.json" with { type: "json" };
import rendertypeTranslucentNoCrumblingJson from "../resources/shaders/core/rendertype_translucent_no_crumbling.json" with { type: "json" };
import rendertypeTripwireJson from "../resources/shaders/core/rendertype_tripwire.json" with { type: "json" };
import type { ShaderProgramJson } from "./shader-program";
import { ShaderProgramDefinition } from "./shader-program";

function createDefinition(name: string, json: ShaderProgramJson): ShaderProgramDefinition {
  return new ShaderProgramDefinition(name, json);
}

const DEFINITIONS = new Map<string, ShaderProgramDefinition>([
  ["position_color", createDefinition("position_color", positionColorJson as ShaderProgramJson)],
  ["position_tex", createDefinition("position_tex", positionTexJson as ShaderProgramJson)],
  ["rendertype_solid", createDefinition("rendertype_solid", rendertypeSolidJson as ShaderProgramJson)],
  ["rendertype_cutout", createDefinition("rendertype_cutout", rendertypeCutoutJson as ShaderProgramJson)],
  ["rendertype_cutout_mipped", createDefinition("rendertype_cutout_mipped", rendertypeCutoutMippedJson as ShaderProgramJson)],
  ["rendertype_entity_solid", createDefinition("rendertype_entity_solid", rendertypeEntitySolidJson as ShaderProgramJson)],
  ["rendertype_entity_cutout", createDefinition("rendertype_entity_cutout", rendertypeEntityCutoutJson as ShaderProgramJson)],
  [
    "rendertype_entity_cutout_no_cull",
    createDefinition("rendertype_entity_cutout_no_cull", rendertypeEntityCutoutNoCullJson as ShaderProgramJson),
  ],
  [
    "rendertype_entity_translucent",
    createDefinition("rendertype_entity_translucent", rendertypeEntityTranslucentJson as ShaderProgramJson),
  ],
  ["rendertype_translucent", createDefinition("rendertype_translucent", rendertypeTranslucentJson as ShaderProgramJson)],
  [
    "rendertype_translucent_moving_block",
    createDefinition("rendertype_translucent_moving_block", rendertypeTranslucentMovingBlockJson as ShaderProgramJson),
  ],
  [
    "rendertype_translucent_no_crumbling",
    createDefinition("rendertype_translucent_no_crumbling", rendertypeTranslucentNoCrumblingJson as ShaderProgramJson),
  ],
  ["rendertype_tripwire", createDefinition("rendertype_tripwire", rendertypeTripwireJson as ShaderProgramJson)],
  ["rendertype_lines", createDefinition("rendertype_lines", rendertypeLinesJson as ShaderProgramJson)],
]);

function getFogFunctions(): string {
  return `
fn linear_fog(
  inColor: vec4<f32>,
  vertexDistance: f32,
  fogStart: f32,
  fogEnd: f32,
  fogColor: vec4<f32>,
) -> vec4<f32> {
  if (vertexDistance <= fogStart) {
    return inColor;
  }

  var fogValue = 1.0;
  if (vertexDistance < fogEnd) {
    fogValue = smoothstep(fogStart, fogEnd, vertexDistance);
  }

  let fogBlend = fogValue * fogColor.a;
  let foggedRgb = inColor.rgb + ((fogColor.rgb - inColor.rgb) * fogBlend);
  return vec4<f32>(foggedRgb, inColor.a);
}

fn linear_fog_fade(vertexDistance: f32, fogStart: f32, fogEnd: f32) -> f32 {
  if (vertexDistance <= fogStart) {
    return 1.0;
  }

  if (vertexDistance >= fogEnd) {
    return 0.0;
  }

  return smoothstep(fogEnd, fogStart, vertexDistance);
}
`;
}

function getLightFunctions(): string {
  // WebGPU: vertex-stage lightmap sampling uses textureSampleLevel instead of GLSL texture.
  return `
const MINECRAFT_LIGHT_POWER: f32 = 0.6;
const MINECRAFT_AMBIENT_LIGHT: f32 = 0.4;

fn minecraft_mix_light(
  lightDir0: vec3<f32>,
  lightDir1: vec3<f32>,
  normal: vec3<f32>,
  color: vec4<f32>,
) -> vec4<f32> {
  let normalizedLightDir0 = normalize(lightDir0);
  let normalizedLightDir1 = normalize(lightDir1);
  let light0 = max(0.0, dot(normalizedLightDir0, normal));
  let light1 = max(0.0, dot(normalizedLightDir1, normal));
  let lightAccum = min(1.0, ((light0 + light1) * MINECRAFT_LIGHT_POWER) + MINECRAFT_AMBIENT_LIGHT);
  return vec4<f32>(color.rgb * lightAccum, color.a);
}

fn minecraft_sample_lightmap(
  lightMapTexture: texture_2d<f32>,
  lightMapSampler: sampler,
  uv: vec2<i32>,
) -> vec4<f32> {
  let clampedUv = clamp(vec2<f32>(uv) / 256.0, vec2<f32>(0.5 / 16.0), vec2<f32>(15.5 / 16.0));
  return textureSampleLevel(lightMapTexture, lightMapSampler, clampedUv, 0.0);
}
`;
}

function getPositionColorSource(): string {
  return `
struct Uniforms {
  ModelViewMat: mat4x4<f32>,
  ProjMat: mat4x4<f32>,
  ColorModulator: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertexColor: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(
  @location(0) position: vec3<f32>,
  @location(1) color: vec4<f32>,
) -> VertexOutput {
  var output: VertexOutput;
  output.position = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(position, 1.0);
  output.vertexColor = color;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let color = input.vertexColor;
  if (color.a == 0.0) {
    discard;
  }

  return color * uniforms.ColorModulator;
}
`;
}

function getPositionTexSource(): string {
  // WebGPU: sampler2D uniforms become separate sampler and texture bindings.
  return `
struct Uniforms {
  ModelViewMat: mat4x4<f32>,
  ProjMat: mat4x4<f32>,
  ColorModulator: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) texCoord0: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var sampler0Sampler: sampler;
@group(0) @binding(2) var sampler0Texture: texture_2d<f32>;

@vertex
fn vs_main(
  @location(0) position: vec3<f32>,
  @location(1) uv0: vec2<f32>,
) -> VertexOutput {
  var output: VertexOutput;
  output.position = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(position, 1.0);
  output.texCoord0 = uv0;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let color = textureSample(sampler0Texture, sampler0Sampler, input.texCoord0);
  if (color.a == 0.0) {
    discard;
  }

  return color * uniforms.ColorModulator;
}
`;
}

function getFoggedBlockShaderSource(alphaCutoff?: number): string {
  const alphaTest =
    alphaCutoff === undefined
      ? ""
      : `  if (color.a < ${alphaCutoff.toFixed(1)}) {
    discard;
  }
`;

  // WebGPU: sampler2D uniforms become separate sampler and texture bindings.
  return `
${getLightFunctions()}
${getFogFunctions()}

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
  @location(0) vertexDistance: f32,
  @location(1) vertexColor: vec4<f32>,
  @location(2) texCoord0: vec2<f32>,
  @location(3) normal: vec4<f32>,
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
  var output: VertexOutput;
  let worldPosition = position + uniforms.ChunkOffset;
  output.position = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(worldPosition, 1.0);
  output.vertexDistance = length((uniforms.ModelViewMat * vec4<f32>(worldPosition, 1.0)).xyz);
  output.vertexColor = color * minecraft_sample_lightmap(sampler2Texture, sampler2Sampler, uv2);
  output.texCoord0 = uv0;
  output.normal = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(normal.xyz, 0.0);
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let _unusedNormal = input.normal;
  let color = textureSample(sampler0Texture, sampler0Sampler, input.texCoord0) * input.vertexColor * uniforms.ColorModulator;
${alphaTest}  return linear_fog(color, input.vertexDistance, uniforms.FogStart, uniforms.FogEnd, uniforms.FogColor);
}
`;
}

function getEntityShaderSource(alphaCutoff?: number): string {
  const alphaTest =
    alphaCutoff === undefined
      ? ""
      : `  if (color.a < ${alphaCutoff.toFixed(1)}) {
    discard;
  }
`;

  // WebGPU: sampler2D uniforms become separate sampler and texture bindings.
  return `
${getLightFunctions()}
${getFogFunctions()}

struct Uniforms {
  ModelViewMat: mat4x4<f32>,
  ProjMat: mat4x4<f32>,
  ColorModulator: vec4<f32>,
  Light0_Direction: vec3<f32>,
  Light1_Direction: vec3<f32>,
  FogStart: f32,
  FogEnd: f32,
  FogColor: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertexDistance: f32,
  @location(1) vertexColor: vec4<f32>,
  @location(2) lightMapColor: vec4<f32>,
  @location(3) overlayColor: vec4<f32>,
  @location(4) texCoord0: vec2<f32>,
  @location(5) normal: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var sampler0Sampler: sampler;
@group(0) @binding(2) var sampler0Texture: texture_2d<f32>;
@group(0) @binding(3) var sampler1Sampler: sampler;
@group(0) @binding(4) var sampler1Texture: texture_2d<f32>;
@group(0) @binding(5) var sampler2Sampler: sampler;
@group(0) @binding(6) var sampler2Texture: texture_2d<f32>;

@vertex
fn vs_main(
  @location(0) position: vec3<f32>,
  @location(1) color: vec4<f32>,
  @location(2) uv0: vec2<f32>,
  @location(3) uv1: vec2<i32>,
  @location(4) uv2: vec2<i32>,
  @location(5) normal: vec4<f32>,
) -> VertexOutput {
  let _unusedOverlaySampler = sampler1Sampler;
  let _unusedLightSampler = sampler2Sampler;
  var output: VertexOutput;
  output.position = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(position, 1.0);
  output.vertexDistance = length((uniforms.ModelViewMat * vec4<f32>(position, 1.0)).xyz);
  output.vertexColor = minecraft_mix_light(uniforms.Light0_Direction, uniforms.Light1_Direction, normal.xyz, color);
  output.lightMapColor = textureLoad(sampler2Texture, uv2 / 16, 0);
  output.overlayColor = textureLoad(sampler1Texture, uv1, 0);
  output.texCoord0 = uv0;
  output.normal = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(normal.xyz, 0.0);
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let _unusedNormal = input.normal;
  var color = textureSample(sampler0Texture, sampler0Sampler, input.texCoord0);
${alphaTest}  color = color * input.vertexColor * uniforms.ColorModulator;
  color = vec4<f32>(mix(input.overlayColor.rgb, color.rgb, input.overlayColor.a), color.a);
  color = color * input.lightMapColor;
  return linear_fog(color, input.vertexDistance, uniforms.FogStart, uniforms.FogEnd, uniforms.FogColor);
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

function getTranslucentNoCrumblingSource(): string {
  // WebGPU: UV2 arrives as a signed-short vertex attribute, so the shader widens it before matching the vanilla varying.
  return `
struct Uniforms {
  ModelViewMat: mat4x4<f32>,
  ProjMat: mat4x4<f32>,
  ColorModulator: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertexColor: vec4<f32>,
  @location(1) texCoord0: vec2<f32>,
  @location(2) texCoord2: vec2<f32>,
  @location(3) normal: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var sampler0Sampler: sampler;
@group(0) @binding(2) var sampler0Texture: texture_2d<f32>;

@vertex
fn vs_main(
  @location(0) position: vec3<f32>,
  @location(1) color: vec4<f32>,
  @location(2) uv0: vec2<f32>,
  @location(3) uv2: vec2<i32>,
  @location(4) normal: vec4<f32>,
) -> VertexOutput {
  var output: VertexOutput;
  output.position = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(position, 1.0);
  output.vertexColor = color;
  output.texCoord0 = uv0;
  output.texCoord2 = vec2<f32>(uv2);
  output.normal = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(normal.xyz, 0.0);
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let _unusedTexCoord2 = input.texCoord2;
  let _unusedNormal = input.normal;
  let color = textureSample(sampler0Texture, sampler0Sampler, input.texCoord0) * input.vertexColor;
  return color * uniforms.ColorModulator;
}
`;
}

function getTranslucentMovingBlockSource(): string {
  // WebGPU: use textureLoad on the sampled texture binding in place of GLSL texelFetch.
  return `
struct Uniforms {
  ModelViewMat: mat4x4<f32>,
  ProjMat: mat4x4<f32>,
  ColorModulator: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertexColor: vec4<f32>,
  @location(1) texCoord0: vec2<f32>,
  @location(2) normal: vec4<f32>,
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
  let _unusedSampler = sampler2Sampler;
  var output: VertexOutput;
  output.position = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(position, 1.0);
  output.vertexColor = color * textureLoad(sampler2Texture, uv2 / 16, 0);
  output.texCoord0 = uv0;
  output.normal = uniforms.ProjMat * uniforms.ModelViewMat * vec4<f32>(normal.xyz, 0.0);
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let _unusedNormal = input.normal;
  let color = textureSample(sampler0Texture, sampler0Sampler, input.texCoord0) * input.vertexColor;
  return color * uniforms.ColorModulator;
}
`;
}

function getLinesSource(): string {
  // WebGPU: use @builtin(vertex_index) instead of GLSL gl_VertexID.
  return `
${getFogFunctions()}

const VIEW_SHRINK: f32 = 1.0 - (1.0 / 256.0);
const VIEW_SCALE: mat4x4<f32> = mat4x4<f32>(
  vec4<f32>(VIEW_SHRINK, 0.0, 0.0, 0.0),
  vec4<f32>(0.0, VIEW_SHRINK, 0.0, 0.0),
  vec4<f32>(0.0, 0.0, VIEW_SHRINK, 0.0),
  vec4<f32>(0.0, 0.0, 0.0, 1.0),
);

struct Uniforms {
  ModelViewMat: mat4x4<f32>,
  ProjMat: mat4x4<f32>,
  ColorModulator: vec4<f32>,
  LineWidth: f32,
  _padding0: f32,
  ScreenSize: vec2<f32>,
  FogStart: f32,
  FogEnd: f32,
  _padding1: vec2<f32>,
  FogColor: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) vertexDistance: f32,
  @location(1) vertexColor: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(
  @builtin(vertex_index) vertexIndex: u32,
  @location(0) position: vec3<f32>,
  @location(1) color: vec4<f32>,
  @location(2) normal: vec4<f32>,
) -> VertexOutput {
  let linePosStart = uniforms.ProjMat * VIEW_SCALE * uniforms.ModelViewMat * vec4<f32>(position, 1.0);
  let linePosEnd = uniforms.ProjMat * VIEW_SCALE * uniforms.ModelViewMat * vec4<f32>(position + normal.xyz, 1.0);
  let ndc1 = linePosStart.xyz / linePosStart.w;
  let ndc2 = linePosEnd.xyz / linePosEnd.w;
  let lineScreenDirection = normalize((ndc2.xy - ndc1.xy) * uniforms.ScreenSize);
  var lineOffset = vec2<f32>(-lineScreenDirection.y, lineScreenDirection.x) * uniforms.LineWidth / uniforms.ScreenSize;
  if (lineOffset.x < 0.0) {
    lineOffset = lineOffset * -1.0;
  }

  var output: VertexOutput;
  if ((vertexIndex % 2u) == 0u) {
    output.position = vec4<f32>((ndc1 + vec3<f32>(lineOffset, 0.0)) * linePosStart.w, linePosStart.w);
  } else {
    output.position = vec4<f32>((ndc1 - vec3<f32>(lineOffset, 0.0)) * linePosStart.w, linePosStart.w);
  }

  output.vertexDistance = length((uniforms.ModelViewMat * vec4<f32>(position, 1.0)).xyz);
  output.vertexColor = color;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  let color = input.vertexColor * uniforms.ColorModulator;
  return linear_fog(color, input.vertexDistance, uniforms.FogStart, uniforms.FogEnd, uniforms.FogColor);
}
`;
}

const SOURCES = new Map<string, string>([
  ["position_color", getPositionColorSource()],
  ["position_tex", getPositionTexSource()],
  ["rendertype_solid", getFoggedBlockShaderSource()],
  ["rendertype_cutout", getFoggedBlockShaderSource(0.1)],
  ["rendertype_cutout_mipped", getFoggedBlockShaderSource(0.5)],
  ["rendertype_entity_solid", getEntityShaderSource()],
  ["rendertype_entity_cutout", getEntityShaderSource(0.1)],
  ["rendertype_entity_cutout_no_cull", getEntityShaderSource(0.1)],
  ["rendertype_entity_translucent", getEntityShaderSource(0.1)],
  ["rendertype_translucent", getFoggedBlockShaderSource()],
  ["rendertype_translucent_moving_block", getTranslucentMovingBlockSource()],
  ["rendertype_translucent_no_crumbling", getTranslucentNoCrumblingSource()],
  ["rendertype_tripwire", getFoggedBlockShaderSource(0.1)],
  ["rendertype_lines", getLinesSource()],
]);

export function getShaderSource(name: string): string {
  const source = SOURCES.get(name);
  if (!source) {
    throw new Error(`No WGSL registered for shader ${name}`);
  }

  return source;
}
