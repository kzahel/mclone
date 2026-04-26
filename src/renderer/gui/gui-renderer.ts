import type { GuiClipRect, GuiDrawCommand, GuiDrawList, GuiTextureId } from "./gui-draw-list";
import {
  createGuiSolidPipeline,
  createGuiTexturedPipeline,
  type GuiSolidPipeline,
  type GuiTexturedPipeline,
} from "./gui-pipeline";
import { GuiTextureAtlas } from "./gui-texture-atlas";

export interface GuiRenderTarget {
  readonly view: GPUTextureView;
  readonly format: GPUTextureFormat;
  readonly pixelWidth: number;
  readonly pixelHeight: number;
  readonly guiWidth: number;
  readonly guiHeight: number;
}

export interface GuiScaledSize {
  readonly scale: number;
  readonly guiWidth: number;
  readonly guiHeight: number;
}

interface PreparedDraw {
  readonly kind: "solid" | "textured";
  readonly texture?: GuiTextureId;
  readonly clip?: GuiClipRect;
  readonly firstVertex: number;
  readonly vertexCount: number;
}

const SOLID_FLOATS_PER_VERTEX = 6;
const TEXTURED_FLOATS_PER_VERTEX = 8;
const QUAD_VERTEX_COUNT = 6;

export class GuiRenderer {
  private readonly solidPipelines = new Map<GPUTextureFormat, GuiSolidPipeline>();
  private readonly texturedPipelines = new Map<GPUTextureFormat, GuiTexturedPipeline>();
  private readonly bindGroups = new WeakMap<GPUBindGroupLayout, Map<GuiTextureId, GPUBindGroup>>();
  private solidVertexBuffer: GPUBuffer | undefined;
  private solidVertexBufferSize = 0;
  private texturedVertexBuffer: GPUBuffer | undefined;
  private texturedVertexBufferSize = 0;

  private constructor(
    private readonly device: GPUDevice,
    private readonly atlas: GuiTextureAtlas,
  ) {}

  public static async create(device: GPUDevice): Promise<GuiRenderer> {
    return new GuiRenderer(device, await GuiTextureAtlas.create(device));
  }

  public static calculateGuiSize(pixelWidth: number, pixelHeight: number): GuiScaledSize {
    let scale = 1;
    while (scale < 4 && pixelWidth / (scale + 1) >= 320 && pixelHeight / (scale + 1) >= 240) {
      scale++;
    }
    return {
      scale,
      guiWidth: Math.ceil(pixelWidth / scale),
      guiHeight: Math.ceil(pixelHeight / scale),
    };
  }

  public render(drawList: GuiDrawList, target: GuiRenderTarget, encoder: GPUCommandEncoder): void {
    const commands = drawList.getCommands();
    if (commands.length === 0) {
      return;
    }

    const prepared = this.prepare(commands, target);
    if (prepared.draws.length === 0) {
      return;
    }

    const solidBuffer = prepared.solidVertices.length === 0
      ? undefined
      : this.uploadSolidVertices(prepared.solidVertices);
    const texturedBuffer = prepared.texturedVertices.length === 0
      ? undefined
      : this.uploadTexturedVertices(prepared.texturedVertices);
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: target.view,
          loadOp: "load",
          storeOp: "store",
        },
      ],
    });

    for (const draw of prepared.draws) {
      this.applyScissor(pass, target, draw.clip);
      if (draw.kind === "solid") {
        if (solidBuffer === undefined) {
          continue;
        }
        pass.setPipeline(this.getSolidPipeline(target.format).pipeline);
        pass.setVertexBuffer(0, solidBuffer);
      } else {
        if (texturedBuffer === undefined || draw.texture === undefined) {
          continue;
        }
        const pipeline = this.getTexturedPipeline(target.format);
        pass.setPipeline(pipeline.pipeline);
        pass.setBindGroup(0, this.getBindGroup(draw.texture, pipeline));
        pass.setVertexBuffer(0, texturedBuffer);
      }
      pass.draw(draw.vertexCount, 1, draw.firstVertex);
    }

    pass.end();
  }

  public destroy(): void {
    this.solidVertexBuffer?.destroy();
    this.texturedVertexBuffer?.destroy();
    this.atlas.destroy();
  }

  private prepare(commands: readonly GuiDrawCommand[], target: GuiRenderTarget): {
    readonly solidVertices: Float32Array;
    readonly texturedVertices: Float32Array;
    readonly draws: readonly PreparedDraw[];
  } {
    const solid: number[] = [];
    const textured: number[] = [];
    const draws: PreparedDraw[] = [];
    for (const command of commands) {
      const clip = normalizeClipForTarget(command.clip, target);
      if (clip !== undefined && (clip.width <= 0 || clip.height <= 0)) {
        continue;
      }

      if (command.type === "solid_rect") {
        const firstVertex = solid.length / SOLID_FLOATS_PER_VERTEX;
        pushSolidQuad(
          solid,
          target,
          command.x0,
          command.y0,
          command.x1,
          command.y1,
          command.color,
          command.color,
        );
        draws.push({ kind: "solid", clip, firstVertex, vertexCount: QUAD_VERTEX_COUNT });
      } else if (command.type === "gradient_rect") {
        const firstVertex = solid.length / SOLID_FLOATS_PER_VERTEX;
        pushSolidQuad(
          solid,
          target,
          command.x0,
          command.y0,
          command.x1,
          command.y1,
          command.topColor,
          command.bottomColor,
        );
        draws.push({ kind: "solid", clip, firstVertex, vertexCount: QUAD_VERTEX_COUNT });
      } else {
        const firstVertex = textured.length / TEXTURED_FLOATS_PER_VERTEX;
        pushTexturedQuad(textured, target, command);
        draws.push({ kind: "textured", texture: command.texture, clip, firstVertex, vertexCount: QUAD_VERTEX_COUNT });
      }
    }
    return {
      solidVertices: new Float32Array(solid),
      texturedVertices: new Float32Array(textured),
      draws,
    };
  }

  private uploadSolidVertices(vertices: Float32Array): GPUBuffer {
    const size = Math.max(4, vertices.byteLength);
    this.solidVertexBuffer = this.ensureBuffer(this.solidVertexBuffer, this.solidVertexBufferSize, size, "gui-solid-vertices");
    this.solidVertexBufferSize = Math.max(this.solidVertexBufferSize, size);
    this.device.queue.writeBuffer(this.solidVertexBuffer, 0, vertices.buffer, vertices.byteOffset, vertices.byteLength);
    return this.solidVertexBuffer;
  }

  private uploadTexturedVertices(vertices: Float32Array): GPUBuffer {
    const size = Math.max(4, vertices.byteLength);
    this.texturedVertexBuffer = this.ensureBuffer(this.texturedVertexBuffer, this.texturedVertexBufferSize, size, "gui-textured-vertices");
    this.texturedVertexBufferSize = Math.max(this.texturedVertexBufferSize, size);
    this.device.queue.writeBuffer(this.texturedVertexBuffer, 0, vertices.buffer, vertices.byteOffset, vertices.byteLength);
    return this.texturedVertexBuffer;
  }

  private ensureBuffer(existing: GPUBuffer | undefined, existingSize: number, requiredSize: number, label: string): GPUBuffer {
    if (existing !== undefined && existingSize >= requiredSize) {
      return existing;
    }

    existing?.destroy();
    return this.device.createBuffer({
      label,
      size: nextPowerOfTwo(requiredSize),
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
  }

  private getSolidPipeline(format: GPUTextureFormat): GuiSolidPipeline {
    let pipeline = this.solidPipelines.get(format);
    if (pipeline === undefined) {
      pipeline = createGuiSolidPipeline(this.device, format);
      this.solidPipelines.set(format, pipeline);
    }
    return pipeline;
  }

  private getTexturedPipeline(format: GPUTextureFormat): GuiTexturedPipeline {
    let pipeline = this.texturedPipelines.get(format);
    if (pipeline === undefined) {
      pipeline = createGuiTexturedPipeline(this.device, format);
      this.texturedPipelines.set(format, pipeline);
    }
    return pipeline;
  }

  private getBindGroup(texture: GuiTextureId, pipeline: GuiTexturedPipeline): GPUBindGroup {
    let bindGroupsByTexture = this.bindGroups.get(pipeline.bindGroupLayout);
    if (bindGroupsByTexture === undefined) {
      bindGroupsByTexture = new Map<GuiTextureId, GPUBindGroup>();
      this.bindGroups.set(pipeline.bindGroupLayout, bindGroupsByTexture);
    }

    let bindGroup = bindGroupsByTexture.get(texture);
    if (bindGroup === undefined) {
      bindGroup = this.device.createBindGroup({
        label: `gui-${texture}-bind-group`,
        layout: pipeline.bindGroupLayout,
        entries: [
          {
            binding: 0,
            resource: this.atlas.sampler,
          },
          {
            binding: 1,
            resource: this.atlas.getView(texture),
          },
        ],
      });
      bindGroupsByTexture.set(texture, bindGroup);
    }
    return bindGroup;
  }

  private applyScissor(pass: GPURenderPassEncoder, target: GuiRenderTarget, clip: GuiClipRect | undefined): void {
    if (clip === undefined) {
      pass.setScissorRect(0, 0, target.pixelWidth, target.pixelHeight);
      return;
    }

    const scaleX = target.pixelWidth / target.guiWidth;
    const scaleY = target.pixelHeight / target.guiHeight;
    const x = Math.max(0, Math.floor(clip.x * scaleX));
    const y = Math.max(0, Math.floor(clip.y * scaleY));
    const width = Math.min(target.pixelWidth - x, Math.ceil(clip.width * scaleX));
    const height = Math.min(target.pixelHeight - y, Math.ceil(clip.height * scaleY));
    pass.setScissorRect(x, y, Math.max(0, width), Math.max(0, height));
  }
}

function pushSolidQuad(
  vertices: number[],
  target: GuiRenderTarget,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  topColor: number,
  bottomColor: number,
): void {
  const left = toClipX(x0, target.guiWidth);
  const right = toClipX(x1, target.guiWidth);
  const top = toClipY(y0, target.guiHeight);
  const bottom = toClipY(y1, target.guiHeight);
  const topRgba = colorToRgba(topColor);
  const bottomRgba = colorToRgba(bottomColor);
  pushSolidVertex(vertices, left, top, topRgba);
  pushSolidVertex(vertices, right, top, topRgba);
  pushSolidVertex(vertices, right, bottom, bottomRgba);
  pushSolidVertex(vertices, left, top, topRgba);
  pushSolidVertex(vertices, right, bottom, bottomRgba);
  pushSolidVertex(vertices, left, bottom, bottomRgba);
}

function pushTexturedQuad(vertices: number[], target: GuiRenderTarget, command: Extract<GuiDrawCommand, { readonly type: "textured_quad" }>): void {
  const left = toClipX(command.x, target.guiWidth);
  const right = toClipX(command.x + command.width, target.guiWidth);
  const top = toClipY(command.y, target.guiHeight);
  const bottom = toClipY(command.y + command.height, target.guiHeight);
  const rgba = colorToRgba(command.color);
  pushTexturedVertex(vertices, left, top, command.u0, command.v0, rgba);
  pushTexturedVertex(vertices, right, top, command.u1, command.v0, rgba);
  pushTexturedVertex(vertices, right, bottom, command.u1, command.v1, rgba);
  pushTexturedVertex(vertices, left, top, command.u0, command.v0, rgba);
  pushTexturedVertex(vertices, right, bottom, command.u1, command.v1, rgba);
  pushTexturedVertex(vertices, left, bottom, command.u0, command.v1, rgba);
}

function pushSolidVertex(vertices: number[], x: number, y: number, color: readonly [number, number, number, number]): void {
  vertices.push(x, y, color[0], color[1], color[2], color[3]);
}

function pushTexturedVertex(
  vertices: number[],
  x: number,
  y: number,
  u: number,
  v: number,
  color: readonly [number, number, number, number],
): void {
  vertices.push(x, y, u, v, color[0], color[1], color[2], color[3]);
}

function toClipX(x: number, guiWidth: number): number {
  return ((x / guiWidth) * 2.0) - 1.0;
}

function toClipY(y: number, guiHeight: number): number {
  return 1.0 - ((y / guiHeight) * 2.0);
}

function colorToRgba(color: number): readonly [number, number, number, number] {
  const value = color >>> 0;
  return [
    ((value >>> 16) & 0xff) / 255.0,
    ((value >>> 8) & 0xff) / 255.0,
    (value & 0xff) / 255.0,
    ((value >>> 24) & 0xff) / 255.0,
  ];
}

function normalizeClipForTarget(clip: GuiClipRect | undefined, target: GuiRenderTarget): GuiClipRect | undefined {
  if (clip === undefined) {
    return undefined;
  }

  const x0 = Math.max(0, Math.min(target.guiWidth, clip.x));
  const y0 = Math.max(0, Math.min(target.guiHeight, clip.y));
  const x1 = Math.max(0, Math.min(target.guiWidth, clip.x + clip.width));
  const y1 = Math.max(0, Math.min(target.guiHeight, clip.y + clip.height));
  return {
    x: x0,
    y: y0,
    width: Math.max(0, x1 - x0),
    height: Math.max(0, y1 - y0),
  };
}

function nextPowerOfTwo(value: number): number {
  let size = 4;
  while (size < value) {
    size *= 2;
  }
  return size;
}
