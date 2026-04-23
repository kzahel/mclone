const FLOAT_BYTES = 4;
const SHADER_VISIBILITY = 0x1 | 0x2;
type ShaderUniformValues = ArrayLike<number>;

function alignTo(value: number, alignment: number): number {
  return Math.ceil(value / alignment) * alignment;
}

function expandValues(count: number, values: ShaderUniformValues): number[] {
  if (values.length !== count && values.length > 1) {
    throw new Error(`Invalid amount of values specified (expected ${count}, found ${values.length})`);
  }

  const expanded = Array.from(values);
  if (count > 1 && values.length === 1) {
    while (expanded.length < count) {
      expanded.push(values[0]!);
    }
  }

  return expanded;
}

function stringToBlendOperation(value: string): GPUBlendOperation {
  const normalized = value.trim().toLowerCase();
  switch (normalized) {
    case "subtract":
      return "subtract";
    case "reversesubtract":
    case "reverse_subtract":
      return "reverse-subtract";
    case "min":
      return "min";
    case "max":
      return "max";
    case "add":
    default:
      return "add";
  }
}

function stringToBlendFactor(value: string): GPUBlendFactor {
  let normalized = value.trim().toLowerCase();
  normalized = normalized.replaceAll("_", "");
  normalized = normalized.replaceAll("one", "1");
  normalized = normalized.replaceAll("zero", "0");
  normalized = normalized.replaceAll("minus", "-");
  switch (normalized) {
    case "0":
      return "zero";
    case "1":
      return "one";
    case "srccolor":
      return "src";
    case "1-srccolor":
      return "one-minus-src";
    case "dstcolor":
      return "dst";
    case "1-dstcolor":
      return "one-minus-dst";
    case "srcalpha":
      return "src-alpha";
    case "1-srcalpha":
      return "one-minus-src-alpha";
    case "dstalpha":
      return "dst-alpha";
    case "1-dstalpha":
      return "one-minus-dst-alpha";
    default:
      throw new Error(`Unsupported blend factor ${value}`);
  }
}

export interface ShaderSamplerJson {
  readonly name: string;
  readonly file?: string;
}

export interface ShaderUniformJson {
  readonly name: string;
  readonly type: string;
  readonly count: number;
  readonly values: readonly number[];
}

export interface ShaderBlendJson {
  readonly func?: string;
  readonly srcrgb?: string;
  readonly dstrgb?: string;
  readonly srcalpha?: string;
  readonly dstalpha?: string;
}

export interface ShaderProgramJson {
  readonly blend?: ShaderBlendJson;
  readonly vertex: string;
  readonly fragment: string;
  readonly attributes?: readonly string[];
  readonly samplers?: readonly ShaderSamplerJson[];
  readonly uniforms?: readonly ShaderUniformJson[];
}

type ShaderUniformKind =
  | "int1"
  | "int2"
  | "int3"
  | "int4"
  | "float1"
  | "float2"
  | "float3"
  | "float4"
  | "matrix2x2"
  | "matrix3x3"
  | "matrix4x4";

export interface ShaderUniformSpec {
  readonly name: string;
  readonly type: ShaderUniformKind;
  readonly count: number;
  readonly values: readonly number[];
}

export interface ShaderUniformLayoutEntry {
  readonly spec: ShaderUniformSpec;
  readonly offset: number;
  readonly byteSize: number;
  readonly alignment: number;
}

export interface ShaderSamplerBinding {
  readonly name: string;
  readonly samplerBinding: number;
  readonly textureBinding: number;
}

export interface ShaderBindGroupResources {
  readonly uniformBuffer?: GPUBuffer;
  readonly samplers?: Readonly<Record<string, GPUSampler>>;
  readonly textures?: Readonly<Record<string, GPUTextureView>>;
}

function getUniformTypeFromString(value: string): ShaderUniformKind {
  if (value === "int") {
    return "int1";
  }

  if (value === "float") {
    return "float1";
  }

  if (value === "matrix2x2") {
    return "matrix2x2";
  }

  if (value === "matrix3x3") {
    return "matrix3x3";
  }

  if (value === "matrix4x4") {
    return "matrix4x4";
  }

  throw new Error(`Unsupported uniform type ${value}`);
}

function resolveUniformKind(type: ShaderUniformKind, count: number): ShaderUniformKind {
  if (type === "int1" && count > 1 && count <= 4) {
    return `int${count}` as ShaderUniformKind;
  }

  if (type === "float1" && count > 1 && count <= 4) {
    return `float${count}` as ShaderUniformKind;
  }

  return type;
}

function getUniformAlignment(type: ShaderUniformKind): number {
  switch (type) {
    case "int1":
    case "float1":
      return 4;
    case "int2":
    case "float2":
    case "matrix2x2":
      return 8;
    case "int3":
    case "int4":
    case "float3":
    case "float4":
    case "matrix3x3":
    case "matrix4x4":
      return 16;
  }
}

function getUniformByteSize(type: ShaderUniformKind): number {
  switch (type) {
    case "int1":
    case "float1":
      return FLOAT_BYTES;
    case "int2":
    case "float2":
      return FLOAT_BYTES * 2;
    case "int3":
    case "float3":
      return FLOAT_BYTES * 3;
    case "int4":
    case "float4":
    case "matrix2x2":
      return FLOAT_BYTES * 4;
    case "matrix3x3":
      return FLOAT_BYTES * 12;
    case "matrix4x4":
      return FLOAT_BYTES * 16;
  }
}

function writeUniformValue(
  dataView: DataView,
  layout: ShaderUniformLayoutEntry,
  values: readonly number[],
): void {
  const { offset, spec } = layout;
  switch (spec.type) {
    case "int1":
    case "int2":
    case "int3":
    case "int4": {
      for (let index = 0; index < values.length; index++) {
        dataView.setInt32(offset + (index * FLOAT_BYTES), Math.trunc(values[index]!), true);
      }
      return;
    }
    case "float1":
    case "float2":
    case "float3":
    case "float4":
    case "matrix2x2":
    case "matrix4x4": {
      for (let index = 0; index < values.length; index++) {
        dataView.setFloat32(offset + (index * FLOAT_BYTES), values[index]!, true);
      }
      return;
    }
    case "matrix3x3": {
      for (let column = 0; column < 3; column++) {
        for (let row = 0; row < 3; row++) {
          const valueIndex = (column * 3) + row;
          dataView.setFloat32(offset + (column * 16) + (row * FLOAT_BYTES), values[valueIndex]!, true);
        }
      }
      return;
    }
  }
}

export class ShaderBlendState {
  public readonly operation: GPUBlendOperation;
  public readonly srcRgb: GPUBlendFactor;
  public readonly dstRgb: GPUBlendFactor;
  public readonly srcAlpha: GPUBlendFactor;
  public readonly dstAlpha: GPUBlendFactor;

  public constructor(json?: ShaderBlendJson) {
    this.operation = stringToBlendOperation(json?.func ?? "add");
    this.srcRgb = stringToBlendFactor(json?.srcrgb ?? "1");
    this.dstRgb = stringToBlendFactor(json?.dstrgb ?? "0");
    this.srcAlpha = stringToBlendFactor(json?.srcalpha ?? json?.srcrgb ?? "1");
    this.dstAlpha = stringToBlendFactor(json?.dstalpha ?? json?.dstrgb ?? "0");
  }
}

export class ShaderProgramDefinition {
  public readonly blend: ShaderBlendState;
  public readonly attributes: readonly string[];
  public readonly samplers: readonly ShaderSamplerBinding[];
  public readonly uniforms: readonly ShaderUniformLayoutEntry[];
  public readonly uniformBufferSize: number;

  public constructor(
    public readonly name: string,
    json: ShaderProgramJson,
  ) {
    this.blend = new ShaderBlendState(json.blend);
    this.attributes = [...(json.attributes ?? [])];
    this.samplers = (json.samplers ?? []).map((sampler, index) => ({
      name: sampler.name,
      samplerBinding: 1 + (index * 2),
      textureBinding: 2 + (index * 2),
    }));

    let offset = 0;
    this.uniforms = (json.uniforms ?? []).map((uniform) => {
      const baseType = getUniformTypeFromString(uniform.type);
      const spec: ShaderUniformSpec = {
        name: uniform.name,
        type: resolveUniformKind(baseType, uniform.count),
        count: uniform.count,
        values: expandValues(uniform.count, uniform.values),
      };
      const alignment = getUniformAlignment(spec.type);
      offset = alignTo(offset, alignment);
      const layout: ShaderUniformLayoutEntry = {
        spec,
        offset,
        byteSize: getUniformByteSize(spec.type),
        alignment,
      };
      offset += layout.byteSize;
      return layout;
    });
    this.uniformBufferSize = alignTo(offset, 16);
  }

  public getUniform(name: string): ShaderUniformLayoutEntry | undefined {
    return this.uniforms.find((uniform) => uniform.spec.name === name);
  }

  public writeUniformBufferBytes(bytes: Uint8Array, overrides: Readonly<Record<string, ShaderUniformValues>> = {}): void {
    if (bytes.byteLength < this.uniformBufferSize) {
      throw new Error(`Uniform byte buffer too small for shader ${this.name}`);
    }

    bytes.fill(0, 0, this.uniformBufferSize);
    const dataView = new DataView(bytes.buffer, bytes.byteOffset, this.uniformBufferSize);
    for (const uniform of this.uniforms) {
      const value = overrides[uniform.spec.name] ?? uniform.spec.values;
      writeUniformValue(dataView, uniform, expandValues(uniform.spec.count, value));
    }
  }

  public createUniformBufferBytes(overrides: Readonly<Record<string, ShaderUniformValues>> = {}): Uint8Array {
    const bytes = new Uint8Array(this.uniformBufferSize);
    this.writeUniformBufferBytes(bytes, overrides);
    return bytes;
  }

  public createBindGroupLayoutEntries(): GPUBindGroupLayoutEntry[] {
    const entries: GPUBindGroupLayoutEntry[] = [];
    if (this.uniforms.length > 0) {
      entries.push({
        binding: 0,
        visibility: SHADER_VISIBILITY,
        buffer: {
          type: "uniform",
        },
      });
    }

    for (const sampler of this.samplers) {
      entries.push({
        binding: sampler.samplerBinding,
        visibility: SHADER_VISIBILITY,
        sampler: {
          type: "filtering",
        },
      });
      entries.push({
        binding: sampler.textureBinding,
        visibility: SHADER_VISIBILITY,
        texture: {
          sampleType: "float",
          viewDimension: "2d",
          multisampled: false,
        },
      });
    }

    return entries;
  }

  public createBindGroup(
    device: GPUDevice,
    layout: GPUBindGroupLayout,
    resources: ShaderBindGroupResources,
  ): GPUBindGroup {
    const entries: GPUBindGroupEntry[] = [];
    if (this.uniforms.length > 0) {
      if (!resources.uniformBuffer) {
        throw new Error(`Shader ${this.name} requires a uniform buffer`);
      }

      entries.push({
        binding: 0,
        resource: {
          buffer: resources.uniformBuffer,
        },
      });
    }

    for (const sampler of this.samplers) {
      const samplerResource = resources.samplers?.[sampler.name];
      const textureResource = resources.textures?.[sampler.name];
      if (!samplerResource || !textureResource) {
        throw new Error(`Shader ${this.name} requires sampler resources for ${sampler.name}`);
      }

      entries.push({
        binding: sampler.samplerBinding,
        resource: samplerResource,
      });
      entries.push({
        binding: sampler.textureBinding,
        resource: textureResource,
      });
    }

    return device.createBindGroup({
      layout,
      entries,
    });
  }
}
