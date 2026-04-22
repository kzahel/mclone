import { BufferVertexConsumer } from "./buffer-vertex-consumer";
import { DefaultVertexFormat } from "./default-vertex-format";
import { VertexFormat, VertexFormatIndexType, VertexFormatMode } from "./vertex-format";
import { VertexFormatElement } from "./vertex-format-element";
import { Vector3f } from "../math/vector3f";

const GROWTH_SIZE = 2_097_152;
const LITTLE_ENDIAN = true;

function roundUp(value: number): number {
  let size = GROWTH_SIZE;
  if (value === 0) {
    return size;
  }

  if (value < 0) {
    size *= -1;
  }

  const remainder = value % size;
  return remainder === 0 ? value : value + size - remainder;
}

function roundToward(value: number, multiple: number): number {
  return Math.trunc((value + multiple - 1) / multiple) * multiple;
}

function compareDescending(left: number, right: number): number {
  if (left > right) {
    return -1;
  }

  if (left < right) {
    return 1;
  }

  return 0;
}

export interface PoppedBuffer {
  readonly drawState: BufferBuilderDrawState;
  readonly buffer: Uint8Array;
}

export class BufferBuilder extends BufferVertexConsumer {
  private buffer: ArrayBuffer;
  private dataView: DataView;
  private bytes: Uint8Array;
  private readonly drawStates: BufferBuilderDrawState[] = [];
  private lastPoppedStateIndex = 0;
  private totalRenderedBytes = 0;
  private nextElementByte = 0;
  private totalUploadedBytes = 0;
  private vertices = 0;
  private currentElementValue: VertexFormatElement | null = null;
  private elementIndex = 0;
  private format = DefaultVertexFormat.BLOCK;
  private mode = VertexFormat.Mode.QUADS;
  private fastFormat = false;
  private fullFormat = false;
  private buildingValue = false;
  private sortingPoints: Vector3f[] | null = null;
  private sortX = Number.NaN;
  private sortY = Number.NaN;
  private sortZ = Number.NaN;
  private indexOnly = false;

  public constructor(initialCapacity: number) {
    super();
    this.buffer = new ArrayBuffer(initialCapacity * 6);
    this.dataView = new DataView(this.buffer);
    this.bytes = new Uint8Array(this.buffer);
  }

  private ensureVertexCapacity(): void {
    this.ensureCapacity(this.format.getVertexSize());
  }

  private ensureCapacity(size: number): void {
    if ((this.nextElementByte + size) <= this.buffer.byteLength) {
      return;
    }

    const oldSize = this.buffer.byteLength;
    const newSize = oldSize + roundUp(size);
    const newBytes = new Uint8Array(newSize);
    newBytes.set(this.bytes);
    this.buffer = newBytes.buffer;
    this.bytes = newBytes;
    this.dataView = new DataView(this.buffer);
  }

  public setQuadSortOrigin(x: number, y: number, z: number): void {
    if (this.mode !== VertexFormat.Mode.QUADS) {
      return;
    }

    if (this.sortX === x && this.sortY === y && this.sortZ === z) {
      return;
    }

    this.sortX = x;
    this.sortY = y;
    this.sortZ = z;
    if (this.sortingPoints === null) {
      this.sortingPoints = this.makeQuadSortingPoints();
    }
  }

  public getSortState(): BufferBuilderSortState {
    return new BufferBuilderSortState(this.mode, this.vertices, this.sortingPoints, this.sortX, this.sortY, this.sortZ);
  }

  public restoreSortState(state: BufferBuilderSortState): void {
    this.mode = state.mode;
    this.vertices = state.vertices;
    this.nextElementByte = this.totalRenderedBytes;
    this.sortingPoints = state.sortingPoints;
    this.sortX = state.sortX;
    this.sortY = state.sortY;
    this.sortZ = state.sortZ;
    this.indexOnly = true;
  }

  public begin(mode: VertexFormatMode, format: VertexFormat): void {
    if (this.buildingValue) {
      throw new Error("Already building!");
    }

    this.buildingValue = true;
    this.mode = mode;
    this.switchFormat(format);
    this.currentElementValue = format.getElements()[0] ?? null;
    this.elementIndex = 0;
  }

  private switchFormat(format: VertexFormat): void {
    if (this.format === format) {
      return;
    }

    this.format = format;
    const isNewEntity = format === DefaultVertexFormat.NEW_ENTITY;
    const isBlock = format === DefaultVertexFormat.BLOCK;
    this.fastFormat = isNewEntity || isBlock;
    this.fullFormat = isNewEntity;
  }

  private makeQuadSortingPoints(): Vector3f[] {
    const integerSize = this.format.getIntegerSize();
    const primitiveStride = integerSize * this.mode.primitiveStride;
    const primitiveCount = Math.trunc(this.vertices / this.mode.primitiveStride);
    const points = new Array<Vector3f>(primitiveCount);

    for (let index = 0; index < primitiveCount; index++) {
      const start = this.totalRenderedBytes + ((index * primitiveStride) * 4);
      const opposite = this.totalRenderedBytes + (((index * primitiveStride) + (integerSize * 2)) * 4);
      const x0 = this.dataView.getFloat32(start + 0, LITTLE_ENDIAN);
      const y0 = this.dataView.getFloat32(start + 4, LITTLE_ENDIAN);
      const z0 = this.dataView.getFloat32(start + 8, LITTLE_ENDIAN);
      const x1 = this.dataView.getFloat32(opposite + 0, LITTLE_ENDIAN);
      const y1 = this.dataView.getFloat32(opposite + 4, LITTLE_ENDIAN);
      const z1 = this.dataView.getFloat32(opposite + 8, LITTLE_ENDIAN);
      points[index] = new Vector3f((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2);
    }

    return points;
  }

  private writeIndex(position: number, indexType: VertexFormatIndexType, value: number): number {
    switch (indexType) {
      case VertexFormat.IndexType.BYTE:
        this.dataView.setInt8(position, value);
        return position + 1;
      case VertexFormat.IndexType.SHORT:
        this.dataView.setInt16(position, value, LITTLE_ENDIAN);
        return position + 2;
      case VertexFormat.IndexType.INT:
      default:
        this.dataView.setInt32(position, value, LITTLE_ENDIAN);
        return position + 4;
    }
  }

  private putSortedQuadIndices(indexType: VertexFormatIndexType): void {
    const distances = new Array<number>(this.sortingPoints!.length);
    const indices = new Array<number>(this.sortingPoints!.length);
    for (let index = 0; index < this.sortingPoints!.length; index++) {
      const point = this.sortingPoints![index]!;
      const deltaX = point.x() - this.sortX;
      const deltaY = point.y() - this.sortY;
      const deltaZ = point.z() - this.sortZ;
      distances[index] = ((deltaX * deltaX) + (deltaY * deltaY)) + (deltaZ * deltaZ);
      indices[index] = index;
    }

    indices.sort((left, right) => compareDescending(distances[left]!, distances[right]!));
    let position = this.nextElementByte;
    for (const index of indices) {
      position = this.writeIndex(position, indexType, (index * this.mode.primitiveStride) + 0);
      position = this.writeIndex(position, indexType, (index * this.mode.primitiveStride) + 1);
      position = this.writeIndex(position, indexType, (index * this.mode.primitiveStride) + 2);
      position = this.writeIndex(position, indexType, (index * this.mode.primitiveStride) + 2);
      position = this.writeIndex(position, indexType, (index * this.mode.primitiveStride) + 3);
      position = this.writeIndex(position, indexType, (index * this.mode.primitiveStride) + 0);
    }
  }

  public end(): void {
    if (!this.buildingValue) {
      throw new Error("Not building!");
    }

    const indexCount = this.mode.indexCount(this.vertices);
    const indexType = VertexFormat.IndexType.least(indexCount);
    let sequentialIndex: boolean;
    if (this.sortingPoints !== null) {
      const indexBufferSize = roundToward(indexCount * indexType.bytes, 4);
      this.ensureCapacity(indexBufferSize);
      this.putSortedQuadIndices(indexType);
      sequentialIndex = false;
      this.nextElementByte += indexBufferSize;
      this.totalRenderedBytes += (this.vertices * this.format.getVertexSize()) + indexBufferSize;
    } else {
      sequentialIndex = true;
      this.totalRenderedBytes += this.vertices * this.format.getVertexSize();
    }

    this.buildingValue = false;
    this.drawStates.push(
      new BufferBuilderDrawState(this.format, this.vertices, indexCount, this.mode, indexType, this.indexOnly, sequentialIndex),
    );
    this.vertices = 0;
    this.currentElementValue = null;
    this.elementIndex = 0;
    this.sortingPoints = null;
    this.sortX = Number.NaN;
    this.sortY = Number.NaN;
    this.sortZ = Number.NaN;
    this.indexOnly = false;
  }

  public putByte(index: number, value: number): void {
    this.dataView.setInt8(this.nextElementByte + index, value);
  }

  public putShort(index: number, value: number): void {
    this.dataView.setInt16(this.nextElementByte + index, value, LITTLE_ENDIAN);
  }

  public putFloat(index: number, value: number): void {
    this.dataView.setFloat32(this.nextElementByte + index, value, LITTLE_ENDIAN);
  }

  public endVertex(): void {
    if (this.elementIndex !== 0) {
      throw new Error("Not filled all elements of the vertex");
    }

    this.vertices++;
    this.ensureVertexCapacity();
    if (this.mode === VertexFormat.Mode.LINES || this.mode === VertexFormat.Mode.LINE_STRIP) {
      const vertexSize = this.format.getVertexSize();
      this.bytes.copyWithin(this.nextElementByte, this.nextElementByte - vertexSize, this.nextElementByte);
      this.nextElementByte += vertexSize;
      this.vertices++;
      this.ensureVertexCapacity();
    }
  }

  public nextElement(): void {
    const elements = this.format.getElements();
    this.elementIndex = (this.elementIndex + 1) % elements.length;
    this.nextElementByte += this.currentElement().getByteSize();
    this.currentElementValue = elements[this.elementIndex]!;
    if (this.currentElementValue.getUsage() === VertexFormatElement.Usage.PADDING) {
      this.nextElement();
    }

    if (this.defaultColorSet && this.currentElementValue.getUsage() === VertexFormatElement.Usage.COLOR) {
      super.color(this.defaultR, this.defaultG, this.defaultB, this.defaultA);
    }
  }

  public override color(r: number, g: number, b: number, a: number): this {
    if (this.defaultColorSet) {
      throw new Error("Default color already set");
    }

    return super.color(r, g, b, a);
  }

  public override vertex(x: number, y: number, z: number): this;
  public override vertex(
    x: number,
    y: number,
    z: number,
    r: number,
    g: number,
    b: number,
    a: number,
    u: number,
    v: number,
    overlay: number,
    light: number,
    normalX: number,
    normalY: number,
    normalZ: number,
  ): void;
  public override vertex(...args: number[]): this | void {
    if (args.length === 3) {
      return super.vertex(args[0]!, args[1]!, args[2]!) as this;
    }

    if (this.defaultColorSet) {
      throw new Error("Default color already set");
    }

    if (!this.fastFormat) {
      super.vertex(
        args[0]!,
        args[1]!,
        args[2]!,
        args[3]!,
        args[4]!,
        args[5]!,
        args[6]!,
        args[7]!,
        args[8]!,
        args[9]!,
        args[10]!,
        args[11]!,
        args[12]!,
        args[13]!,
      );
      return;
    }

    this.putFloat(0, args[0]!);
    this.putFloat(4, args[1]!);
    this.putFloat(8, args[2]!);
    this.putByte(12, Math.trunc(args[3]! * 255));
    this.putByte(13, Math.trunc(args[4]! * 255));
    this.putByte(14, Math.trunc(args[5]! * 255));
    this.putByte(15, Math.trunc(args[6]! * 255));
    this.putFloat(16, args[7]!);
    this.putFloat(20, args[8]!);
    let uvOffset: number;
    if (this.fullFormat) {
      this.putShort(24, args[9]! & 0xffff);
      this.putShort(26, (args[9]! >> 16) & 0xffff);
      uvOffset = 28;
    } else {
      uvOffset = 24;
    }

    this.putShort(uvOffset + 0, args[10]! & 0xffff);
    this.putShort(uvOffset + 2, (args[10]! >> 16) & 0xffff);
    this.putByte(uvOffset + 4, BufferVertexConsumer.normalIntValue(args[11]!));
    this.putByte(uvOffset + 5, BufferVertexConsumer.normalIntValue(args[12]!));
    this.putByte(uvOffset + 6, BufferVertexConsumer.normalIntValue(args[13]!));
    this.nextElementByte += uvOffset + 8;
    this.endVertex();
  }

  public popNextBuffer(): PoppedBuffer {
    const drawState = this.drawStates[this.lastPoppedStateIndex++]!;
    const start = this.totalUploadedBytes;
    this.totalUploadedBytes += roundToward(drawState.bufferSize(), 4);
    const buffer = this.bytes.slice(start, this.totalUploadedBytes);
    if (this.lastPoppedStateIndex === this.drawStates.length && this.vertices === 0) {
      this.clear();
    }

    return { drawState, buffer };
  }

  public clear(): void {
    if (this.totalRenderedBytes !== this.totalUploadedBytes) {
      console.warn(`Bytes mismatch ${this.totalRenderedBytes} ${this.totalUploadedBytes}`);
    }

    this.discard();
  }

  public discard(): void {
    this.totalRenderedBytes = 0;
    this.totalUploadedBytes = 0;
    this.nextElementByte = 0;
    this.drawStates.length = 0;
    this.lastPoppedStateIndex = 0;
  }

  public currentElement(): VertexFormatElement {
    if (this.currentElementValue === null) {
      throw new Error("BufferBuilder not started");
    }

    return this.currentElementValue;
  }

  public building(): boolean {
    return this.buildingValue;
  }
}

export class BufferBuilderDrawState {
  public constructor(
    private readonly formatValue: VertexFormat,
    private readonly vertexCountValue: number,
    private readonly indexCountValue: number,
    private readonly modeValue: VertexFormatMode,
    private readonly indexTypeValue: VertexFormatIndexType,
    private readonly indexOnlyValue: boolean,
    private readonly sequentialIndexValue: boolean,
  ) {}

  public format(): VertexFormat {
    return this.formatValue;
  }

  public vertexCount(): number {
    return this.vertexCountValue;
  }

  public indexCount(): number {
    return this.indexCountValue;
  }

  public mode(): VertexFormatMode {
    return this.modeValue;
  }

  public indexType(): VertexFormatIndexType {
    return this.indexTypeValue;
  }

  public vertexBufferSize(): number {
    return this.vertexCountValue * this.formatValue.getVertexSize();
  }

  private indexBufferSize(): number {
    return this.sequentialIndexValue ? 0 : this.indexCountValue * this.indexTypeValue.bytes;
  }

  public bufferSize(): number {
    return this.vertexBufferSize() + this.indexBufferSize();
  }

  public indexOnly(): boolean {
    return this.indexOnlyValue;
  }

  public sequentialIndex(): boolean {
    return this.sequentialIndexValue;
  }
}

export class BufferBuilderSortState {
  public constructor(
    public readonly mode: VertexFormatMode,
    public readonly vertices: number,
    public readonly sortingPoints: Vector3f[] | null,
    public readonly sortX: number,
    public readonly sortY: number,
    public readonly sortZ: number,
  ) {}
}
