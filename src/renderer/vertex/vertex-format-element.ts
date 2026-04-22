class VertexFormatElementType {
  public static readonly FLOAT = new VertexFormatElementType(4, "Float", 5126);
  public static readonly UBYTE = new VertexFormatElementType(1, "Unsigned Byte", 5121);
  public static readonly BYTE = new VertexFormatElementType(1, "Byte", 5120);
  public static readonly USHORT = new VertexFormatElementType(2, "Unsigned Short", 5123);
  public static readonly SHORT = new VertexFormatElementType(2, "Short", 5122);
  public static readonly UINT = new VertexFormatElementType(4, "Unsigned Int", 5125);
  public static readonly INT = new VertexFormatElementType(4, "Int", 5124);

  private constructor(
    private readonly size: number,
    private readonly name: string,
    private readonly glType: number,
  ) {}

  public getSize(): number {
    return this.size;
  }

  public getName(): string {
    return this.name;
  }

  public getGlType(): number {
    return this.glType;
  }
}

class VertexFormatElementUsage {
  public static readonly POSITION = new VertexFormatElementUsage("Position");
  public static readonly NORMAL = new VertexFormatElementUsage("Normal");
  public static readonly COLOR = new VertexFormatElementUsage("Vertex Color");
  public static readonly UV = new VertexFormatElementUsage("UV");
  public static readonly PADDING = new VertexFormatElementUsage("Padding");
  public static readonly GENERIC = new VertexFormatElementUsage("Generic");

  private constructor(private readonly name: string) {}

  public getName(): string {
    return this.name;
  }
}

export class VertexFormatElement {
  public static readonly Type = VertexFormatElementType;
  public static readonly Usage = VertexFormatElementUsage;

  private readonly byteSize: number;

  public constructor(
    private readonly index: number,
    private readonly type: VertexFormatElementType,
    private readonly usage: VertexFormatElementUsage,
    private readonly count: number,
  ) {
    if (!this.supportsUsage(index, usage)) {
      throw new Error("Multiple vertex elements of the same type other than UVs are not supported");
    }

    this.byteSize = type.getSize() * this.count;
  }

  private supportsUsage(index: number, usage: VertexFormatElementUsage): boolean {
    return index === 0 || usage === VertexFormatElementUsage.UV;
  }

  public getType(): VertexFormatElementType {
    return this.type;
  }

  public getUsage(): VertexFormatElementUsage {
    return this.usage;
  }

  public getCount(): number {
    return this.count;
  }

  public getIndex(): number {
    return this.index;
  }

  public getByteSize(): number {
    return this.byteSize;
  }

  public isPosition(): boolean {
    return this.usage === VertexFormatElementUsage.POSITION;
  }

  public equals(other: unknown): boolean {
    if (!(other instanceof VertexFormatElement)) {
      return false;
    }

    return (
      this.count === other.count &&
      this.index === other.index &&
      this.type === other.type &&
      this.usage === other.usage
    );
  }

  public toString(): string {
    return `${this.count},${this.usage.getName()},${this.type.getName()}`;
  }
}
