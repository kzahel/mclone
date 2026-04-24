export class DataLayer {
  public static readonly LAYER_COUNT = 16;
  public static readonly LAYER_SIZE = 128;
  public static readonly SIZE = 2048;

  private data: Uint8Array | undefined;

  public constructor(data?: Uint8Array) {
    if (data !== undefined && data.length !== DataLayer.SIZE) {
      throw new Error(`DataLayer should be ${DataLayer.SIZE} bytes not: ${data.length}`);
    }
    this.data = data;
  }

  public get(x: number, y: number, z: number): number {
    return this.getByIndex(DataLayer.getIndex(x, y, z));
  }

  public set(x: number, y: number, z: number, value: number): void {
    this.setByIndex(DataLayer.getIndex(x, y, z), value);
  }

  private static getIndex(x: number, y: number, z: number): number {
    return y << 8 | z << 4 | x;
  }

  private getByIndex(index: number): number {
    if (this.data === undefined) {
      return 0;
    }

    const byteIndex = DataLayer.getByteIndex(index);
    const nibbleIndex = DataLayer.getNibbleIndex(index);
    return this.data[byteIndex]! >> (4 * nibbleIndex) & 15;
  }

  private setByIndex(index: number, value: number): void {
    if (this.data === undefined) {
      this.data = new Uint8Array(DataLayer.SIZE);
    }

    const byteIndex = DataLayer.getByteIndex(index);
    const nibbleIndex = DataLayer.getNibbleIndex(index);
    const clearMask = ~(15 << (4 * nibbleIndex));
    const valueBits = (value & 15) << (4 * nibbleIndex);
    this.data[byteIndex] = (this.data[byteIndex]! & clearMask) | valueBits;
  }

  private static getNibbleIndex(index: number): number {
    return index & 1;
  }

  private static getByteIndex(index: number): number {
    return index >> 1;
  }

  public getData(): Uint8Array {
    if (this.data === undefined) {
      this.data = new Uint8Array(DataLayer.SIZE);
    }
    return this.data;
  }

  public copy(): DataLayer {
    return this.data === undefined ? new DataLayer() : new DataLayer(new Uint8Array(this.data));
  }

  public toString(): string {
    let out = "";
    for (let index = 0; index < 4096; index++) {
      out += this.getByIndex(index).toString(16);
      if ((index & 15) === 15) {
        out += "\n";
      }
      if ((index & 0xFF) === 255) {
        out += "\n";
      }
    }
    return out;
  }

  public layerToString(_layer: number): string {
    let out = "";
    for (let index = 0; index < 256; index++) {
      out += this.getByIndex(index).toString(16);
      if ((index & 15) === 15) {
        out += "\n";
      }
    }
    return out;
  }

  public isEmpty(): boolean {
    return this.data === undefined;
  }
}
