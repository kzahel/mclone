import { describe, expect, test } from "vitest";

import { paletteBitsFor, unpackBitStorage } from "../../src/oracle/anvil/chunk.ts";

function pack(entries: readonly number[], bits: number): BigInt64Array {
  if (bits <= 0 || bits > 32) {
    throw new Error(`invalid bits ${bits}`);
  }
  const valuesPerLong = Math.floor(64 / bits);
  const longCount = Math.ceil(entries.length / valuesPerLong);
  const mask = (1n << BigInt(bits)) - 1n;
  const longs = new BigInt64Array(longCount);
  for (let longIndex = 0; longIndex < longCount; longIndex++) {
    let long = 0n;
    for (let slot = 0; slot < valuesPerLong; slot++) {
      const entryIndex = longIndex * valuesPerLong + slot;
      if (entryIndex >= entries.length) {
        break;
      }
      const value = BigInt(entries[entryIndex]!) & mask;
      long |= value << BigInt(slot * bits);
    }
    // Convert unsigned long into signed for BigInt64Array storage.
    if (long >= (1n << 63n)) {
      long -= (1n << 64n);
    }
    longs[longIndex] = long;
  }
  return longs;
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

describe("unpackBitStorage", () => {
  test("round-trips 4-bit palette indices (16 slots per long)", () => {
    const values = Array.from({ length: 64 }, (_, index) => index % 16);
    const packed = pack(values, 4);
    expect(unpackBitStorage(packed, 4, values.length)).toEqual(values);
  });

  test("round-trips 5-bit palette indices (12 slots per long, padding bits)", () => {
    const values = Array.from({ length: 32 }, (_, index) => (index * 7) % 32);
    const packed = pack(values, 5);
    expect(unpackBitStorage(packed, 5, values.length)).toEqual(values);
  });

  test("round-trips 9-bit heightmap layout (7 slots per long)", () => {
    const values = Array.from({ length: 256 }, (_, index) => index % 384);
    const packed = pack(values, 9);
    const expectedLongs = Math.ceil(values.length / Math.floor(64 / 9));
    expect(packed.length).toBe(expectedLongs);
    expect(expectedLongs).toBe(37);
    expect(unpackBitStorage(packed, 9, values.length)).toEqual(values);
  });

  test("throws on undersized input", () => {
    const packed = new BigInt64Array(1);
    expect(() => unpackBitStorage(packed, 4, 32)).toThrow(/bit-storage data length/);
  });

  test("handles high-bit entries without sign extension", () => {
    const values = [31, 0, 31, 0];
    const packed = pack(values, 5);
    expect(unpackBitStorage(packed, 5, 4)).toEqual(values);
  });
});
