export function paletteBitsFor(paletteSize: number): number {
  if (!Number.isInteger(paletteSize) || paletteSize < 1) {
    throw new Error(`paletteBitsFor requires a positive integer palette size, got ${paletteSize}`);
  }
  if (paletteSize <= 1) {
    return 1;
  }

  let bits = 0;
  let value = paletteSize - 1;
  while (value > 0) {
    bits += 1;
    value >>>= 1;
  }
  return bits;
}

export class BitStorage {
  private readonly data: BigInt64Array;
  private readonly mask: bigint;
  private readonly valuesPerLong: number;

  public constructor(
    private readonly bits: number,
    private readonly size: number,
    raw?: BigInt64Array | readonly bigint[],
  ) {
    if (!Number.isInteger(bits) || bits < 1 || bits > 32) {
      throw new Error(`BitStorage bits must be an integer in [1, 32], got ${bits}`);
    }
    if (!Number.isInteger(size) || size < 0) {
      throw new Error(`BitStorage size must be a non-negative integer, got ${size}`);
    }

    this.mask = (1n << BigInt(bits)) - 1n;
    this.valuesPerLong = Math.floor(64 / bits);
    const expectedLongs = Math.ceil(size / this.valuesPerLong);
    if (raw !== undefined) {
      if (raw.length !== expectedLongs) {
        throw new Error(`bit-storage data length ${raw.length} != expected ${expectedLongs} for bits=${bits}, size=${size}`);
      }
      this.data = raw instanceof BigInt64Array ? new BigInt64Array(raw) : BigInt64Array.from(raw);
    } else {
      this.data = new BigInt64Array(expectedLongs);
    }
  }

  public get(index: number): number {
    this.validateIndex(index);
    const cellIndex = this.cellIndex(index);
    const bitIndex = BigInt((index - cellIndex * this.valuesPerLong) * this.bits);
    const word = BigInt.asUintN(64, this.data[cellIndex]!);
    return Number((word >> bitIndex) & this.mask);
  }

  public set(index: number, value: number): void {
    this.validateIndex(index);
    this.validateValue(value);
    const cellIndex = this.cellIndex(index);
    const bitIndex = BigInt((index - cellIndex * this.valuesPerLong) * this.bits);
    const word = BigInt.asUintN(64, this.data[cellIndex]!);
    const cleared = word & ~(this.mask << bitIndex);
    const updated = cleared | ((BigInt(value) & this.mask) << bitIndex);
    this.data[cellIndex] = BigInt.asIntN(64, updated);
  }

  public getAndSet(index: number, value: number): number {
    const previous = this.get(index);
    this.set(index, value);
    return previous;
  }

  public getRaw(): BigInt64Array {
    return this.data;
  }

  public getSize(): number {
    return this.size;
  }

  public getBits(): number {
    return this.bits;
  }

  public getAll(): number[] {
    const values = new Array<number>(this.size);
    let index = 0;
    for (let longIndex = 0; longIndex < this.data.length; longIndex++) {
      let word = BigInt.asUintN(64, this.data[longIndex]!);
      for (let slot = 0; slot < this.valuesPerLong && index < this.size; slot++) {
        values[index++] = Number(word & this.mask);
        word >>= BigInt(this.bits);
      }
    }
    return values;
  }

  private cellIndex(index: number): number {
    return Math.floor(index / this.valuesPerLong);
  }

  private validateIndex(index: number): void {
    if (!Number.isInteger(index) || index < 0 || index >= this.size) {
      throw new Error(`BitStorage index ${index} out of bounds for size ${this.size}`);
    }
  }

  private validateValue(value: number): void {
    if (!Number.isInteger(value) || value < 0 || BigInt(value) > this.mask) {
      throw new Error(`BitStorage value ${value} out of bounds for ${this.bits} bits`);
    }
  }
}

export function unpackBitStorage(packed: BigInt64Array | readonly bigint[], bits: number, entries: number): number[] {
  return new BitStorage(bits, entries, packed).getAll();
}
