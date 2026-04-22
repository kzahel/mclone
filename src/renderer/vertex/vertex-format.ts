import { VertexFormatElement } from "./vertex-format-element";

export class VertexFormatIndexType {
  public static readonly BYTE = new VertexFormatIndexType(5121, 1);
  public static readonly SHORT = new VertexFormatIndexType(5123, 2);
  public static readonly INT = new VertexFormatIndexType(5125, 4);

  private constructor(
    public readonly asGLType: number,
    public readonly bytes: number,
  ) {}

  public static least(indexCount: number): VertexFormatIndexType {
    if ((indexCount & -65536) !== 0) {
      return VertexFormatIndexType.INT;
    }

    return (indexCount & 0xff00) !== 0 ? VertexFormatIndexType.SHORT : VertexFormatIndexType.BYTE;
  }
}

export class VertexFormatMode {
  public static readonly LINES = new VertexFormatMode(4, 2, 2);
  public static readonly LINE_STRIP = new VertexFormatMode(5, 2, 1);
  public static readonly DEBUG_LINES = new VertexFormatMode(1, 2, 2);
  public static readonly DEBUG_LINE_STRIP = new VertexFormatMode(3, 2, 1);
  public static readonly TRIANGLES = new VertexFormatMode(4, 3, 3);
  public static readonly TRIANGLE_STRIP = new VertexFormatMode(5, 3, 1);
  public static readonly TRIANGLE_FAN = new VertexFormatMode(6, 3, 1);
  public static readonly QUADS = new VertexFormatMode(4, 4, 4);

  private constructor(
    public readonly asGLMode: number,
    public readonly primitiveLength: number,
    public readonly primitiveStride: number,
  ) {}

  public indexCount(vertexCount: number): number {
    switch (this) {
      case VertexFormatMode.LINE_STRIP:
      case VertexFormatMode.DEBUG_LINES:
      case VertexFormatMode.DEBUG_LINE_STRIP:
      case VertexFormatMode.TRIANGLES:
      case VertexFormatMode.TRIANGLE_STRIP:
      case VertexFormatMode.TRIANGLE_FAN:
        return vertexCount;
      case VertexFormatMode.LINES:
      case VertexFormatMode.QUADS:
        return Math.floor(vertexCount / 4) * 6;
      default:
        return 0;
    }
  }
}

export class VertexFormat {
  public static readonly IndexType = VertexFormatIndexType;
  public static readonly Mode = VertexFormatMode;

  private readonly elements: VertexFormatElement[];
  private readonly offsets: number[] = [];
  private readonly vertexSize: number;

  public constructor(private readonly elementMapping: ReadonlyMap<string, VertexFormatElement>) {
    this.elements = [...elementMapping.values()];

    let offset = 0;
    for (const element of this.elements) {
      this.offsets.push(offset);
      offset += element.getByteSize();
    }

    this.vertexSize = offset;
  }

  public getIntegerSize(): number {
    return Math.floor(this.getVertexSize() / 4);
  }

  public getVertexSize(): number {
    return this.vertexSize;
  }

  public getElements(): readonly VertexFormatElement[] {
    return this.elements;
  }

  public getOffsets(): readonly number[] {
    return this.offsets;
  }

  public getElementAttributeNames(): string[] {
    return [...this.elementMapping.keys()];
  }

  public equals(other: unknown): boolean {
    if (!(other instanceof VertexFormat)) {
      return false;
    }

    if (this.vertexSize !== other.vertexSize || this.elementMapping.size !== other.elementMapping.size) {
      return false;
    }

    const thisEntries = [...this.elementMapping.entries()];
    const otherEntries = [...other.elementMapping.entries()];
    return thisEntries.every(([name, element], index) => {
      const otherEntry = otherEntries[index];
      return otherEntry !== undefined && name === otherEntry[0] && element.equals(otherEntry[1]);
    });
  }

  public toString(): string {
    const parts = [...this.elementMapping.entries()].map(([name, element]) => `${name}=${element.toString()}`);
    return `format: ${this.elementMapping.size} elements: ${parts.join(" ")}`;
  }
}
