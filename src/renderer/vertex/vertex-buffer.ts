import { BufferBuilder, BufferBuilderDrawState } from "./buffer-builder";
import { VertexFormat, VertexFormatIndexType, VertexFormatMode } from "./vertex-format";

const LITTLE_ENDIAN = true;

function roundToward(value: number, multiple: number): number {
  return Math.trunc((value + multiple - 1) / multiple) * multiple;
}

function topologyForMode(mode: VertexFormatMode): GPUPrimitiveTopology {
  switch (mode) {
    case VertexFormat.Mode.LINES:
    case VertexFormat.Mode.DEBUG_LINES:
      return "line-list";
    case VertexFormat.Mode.LINE_STRIP:
    case VertexFormat.Mode.DEBUG_LINE_STRIP:
      return "line-strip";
    case VertexFormat.Mode.TRIANGLES:
    case VertexFormat.Mode.QUADS:
      return "triangle-list";
    case VertexFormat.Mode.TRIANGLE_STRIP:
      return "triangle-strip";
    case VertexFormat.Mode.TRIANGLE_FAN:
      throw new Error("WebGPU does not support triangle-fan topology");
    default:
      throw new Error("Unsupported vertex format mode");
  }
}

function gpuIndexFormat(indexType: VertexFormatIndexType): GPUIndexFormat {
  // WebGPU: uint16 is the smallest supported index format; promote BYTE indices.
  return indexType === VertexFormat.IndexType.INT ? "uint32" : "uint16";
}

function readIndex(dataView: DataView, offset: number, indexType: VertexFormatIndexType): number {
  switch (indexType) {
    case VertexFormat.IndexType.BYTE:
      return dataView.getUint8(offset);
    case VertexFormat.IndexType.SHORT:
      return dataView.getUint16(offset, LITTLE_ENDIAN);
    case VertexFormat.IndexType.INT:
    default:
      return dataView.getUint32(offset, LITTLE_ENDIAN);
  }
}

function createQuadIndices(vertexCount: number, indexFormat: GPUIndexFormat): Uint16Array | Uint32Array {
  const quadCount = Math.trunc(vertexCount / 4);
  const indexCount = quadCount * 6;
  const indices = indexFormat === "uint32" ? new Uint32Array(indexCount) : new Uint16Array(indexCount);
  for (let quad = 0; quad < quadCount; quad++) {
    const vertexBase = quad * 4;
    const indexBase = quad * 6;
    indices[indexBase + 0] = vertexBase + 0;
    indices[indexBase + 1] = vertexBase + 1;
    indices[indexBase + 2] = vertexBase + 2;
    indices[indexBase + 3] = vertexBase + 0;
    indices[indexBase + 4] = vertexBase + 2;
    indices[indexBase + 5] = vertexBase + 3;
  }

  return indices;
}

function createCustomIndexData(drawState: BufferBuilderDrawState, indexBytes: Uint8Array): Uint16Array | Uint32Array | null {
  if (drawState.sequentialIndex()) {
    if (drawState.mode() !== VertexFormat.Mode.QUADS) {
      return null;
    }

    // WebGPU: QUADS must be expanded to indexed triangles because the topology is unavailable.
    return createQuadIndices(drawState.vertexCount(), gpuIndexFormat(drawState.indexType()));
  }

  const indexFormat = gpuIndexFormat(drawState.indexType());
  const dataView = new DataView(indexBytes.buffer, indexBytes.byteOffset, indexBytes.byteLength);
  const result =
    indexFormat === "uint32" ? new Uint32Array(drawState.indexCount()) : new Uint16Array(drawState.indexCount());
  for (let index = 0; index < drawState.indexCount(); index++) {
    result[index] = readIndex(dataView, index * drawState.indexType().bytes, drawState.indexType());
  }

  return result;
}

function uploadBuffer(
  device: GPUDevice,
  data: ArrayBufferView,
  usage: GPUBufferUsageFlags,
  previous: GPUBuffer | null,
): GPUBuffer {
  previous?.destroy();
  const buffer = device.createBuffer({
    size: roundToward(data.byteLength, 4),
    usage,
    mappedAtCreation: true,
  });
  new Uint8Array(buffer.getMappedRange()).set(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
  buffer.unmap();
  return buffer;
}

export class VertexBuffer {
  private vertexBuffer: GPUBuffer | null = null;
  private indexBuffer: GPUBuffer | null = null;
  private indexFormat: GPUIndexFormat | null = null;
  private indexCount = 0;
  private vertexCount = 0;
  private mode: VertexFormatMode = VertexFormat.Mode.QUADS;
  private format: VertexFormat | null = null;

  public constructor(private readonly device: GPUDevice) {}

  public upload(bufferBuilder: BufferBuilder): void {
    const { drawState, buffer } = bufferBuilder.popNextBuffer();
    this.uploadRaw(drawState, buffer);
  }

  public uploadRaw(drawState: BufferBuilderDrawState, buffer: Uint8Array): void {
    const actualBuffer = buffer.subarray(0, drawState.bufferSize());
    const vertexByteLength = drawState.vertexBufferSize();
    const vertexBytes = actualBuffer.subarray(0, vertexByteLength);
    const indexBytes = actualBuffer.subarray(vertexByteLength);
    this.indexCount = drawState.indexCount();
    this.vertexCount = drawState.vertexCount();
    this.mode = drawState.mode();
    this.format = drawState.format();

    if (!drawState.indexOnly()) {
      this.vertexBuffer = uploadBuffer(this.device, vertexBytes, GPUBufferUsage.VERTEX, this.vertexBuffer);
    } else if (this.vertexBuffer === null) {
      throw new Error("Index-only upload requires an existing vertex buffer");
    }

    const indexData = createCustomIndexData(drawState, indexBytes);
    if (indexData === null) {
      this.indexBuffer?.destroy();
      this.indexBuffer = null;
      this.indexFormat = null;
      return;
    }

    this.indexFormat = indexData instanceof Uint32Array ? "uint32" : "uint16";
    this.indexBuffer = uploadBuffer(this.device, indexData, GPUBufferUsage.INDEX, this.indexBuffer);
  }

  public draw(passEncoder: GPURenderPassEncoder): void {
    if (this.vertexBuffer === null || this.vertexCount === 0) {
      return;
    }

    passEncoder.setVertexBuffer(0, this.vertexBuffer);
    if (this.indexBuffer !== null && this.indexFormat !== null && this.indexCount !== 0) {
      passEncoder.setIndexBuffer(this.indexBuffer, this.indexFormat);
      passEncoder.drawIndexed(this.indexCount);
      return;
    }

    passEncoder.draw(this.vertexCount);
  }

  public getFormat(): VertexFormat | null {
    return this.format;
  }

  public getMode(): VertexFormatMode {
    return this.mode;
  }

  public getPrimitiveTopology(): GPUPrimitiveTopology {
    return topologyForMode(this.mode);
  }

  public close(): void {
    this.vertexBuffer?.destroy();
    this.vertexBuffer = null;
    this.indexBuffer?.destroy();
    this.indexBuffer = null;
  }
}
