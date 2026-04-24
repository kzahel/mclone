import { describe, expect, test } from "vitest";
import { BitStorage, paletteBitsFor, unpackBitStorage } from "../../src/util/bit-storage";

function valuesFor(bits: number, size: number): number[] {
  const mask = bits === 32 ? 0xffff_ffff : (1 << bits) - 1;
  return Array.from({ length: size }, (_, index) => (index * 7 + 3) & mask);
}

describe("paletteBitsFor", () => {
  test.each([
    [1, 1],
    [2, 1],
    [3, 2],
    [4, 2],
    [5, 3],
    [15, 4],
    [16, 4],
    [17, 5],
    [256, 8],
    [257, 9],
  ])("palette size %i -> %i bits", (size, expected) => {
    expect(paletteBitsFor(size)).toBe(expected);
  });
});

describe("BitStorage", () => {
  test.each([1, 4, 5, 9, 15, 32])("round-trips %i-bit values", (bits) => {
    const values = valuesFor(bits, 73);
    const storage = new BitStorage(bits, values.length);
    for (let index = 0; index < values.length; index++) {
      storage.set(index, values[index]!);
    }

    expect(storage.getBits()).toBe(bits);
    expect(storage.getSize()).toBe(values.length);
    expect(storage.getAll()).toEqual(values);
    expect(unpackBitStorage(storage.getRaw(), bits, values.length)).toEqual(values);
  });

  test("preserves 1.17.1 no-cross-word layout with padding bits", () => {
    const storage = new BitStorage(5, 13);
    for (let index = 0; index < 13; index++) {
      storage.set(index, index + 1);
    }

    expect(storage.getRaw()).toHaveLength(2);
    expect(storage.getRaw()[0]).toBe(0x062d_4941_cc52_0c41n);
    expect(storage.getRaw()[1]).toBe(13n);
    expect(storage.getAll()).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
  });

  test("getAndSet returns the previous value", () => {
    const storage = new BitStorage(4, 4);
    storage.set(2, 9);

    expect(storage.getAndSet(2, 3)).toBe(9);
    expect(storage.get(2)).toBe(3);
  });

  test("copies raw input and validates raw length", () => {
    const source = new BitStorage(4, 16);
    source.set(0, 7);
    const copied = new BitStorage(4, 16, source.getRaw());

    source.set(0, 2);
    expect(copied.get(0)).toBe(7);
    expect(() => new BitStorage(4, 16, new BigInt64Array(2))).toThrow(/bit-storage data length/);
  });

  test("validates bits, indices, and values", () => {
    expect(() => new BitStorage(0, 1)).toThrow(/bits/);
    expect(() => new BitStorage(33, 1)).toThrow(/bits/);
    expect(() => new BitStorage(4, -1)).toThrow(/size/);

    const storage = new BitStorage(4, 2);
    expect(() => storage.get(-1)).toThrow(/out of bounds/);
    expect(() => storage.get(2)).toThrow(/out of bounds/);
    expect(() => storage.set(0, 16)).toThrow(/out of bounds/);
  });
});
