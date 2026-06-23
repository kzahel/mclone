import { describe, expect, test } from "vitest";

import { decodeChunk, paletteBitsFor, unpackBitStorage } from "../../oracle/lib/anvil/chunk.ts";
import { NBT_TAG_COMPOUND, type NbtCompound, type NbtList } from "../../oracle/lib/anvil/nbt.ts";

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

function sectionList(sections: readonly NbtCompound[]): NbtList {
  return { type: NBT_TAG_COMPOUND, values: sections };
}

function palette(names: readonly string[]): NbtList {
  return {
    type: NBT_TAG_COMPOUND,
    values: names.map((name) => ({ Name: name })),
  };
}

function rootWithSections(sections: readonly NbtCompound[], isLightOn: number | "absent" = 1): NbtCompound {
  const level: Record<string, unknown> = {
    xPos: 0,
    zPos: 0,
    Status: "full",
    Sections: sectionList(sections),
  };
  if (isLightOn !== "absent") {
    level.isLightOn = isLightOn;
  }
  return {
    DataVersion: 2730,
    Level: level as NbtCompound,
  };
}

function lightBytes(values: readonly number[]): Int8Array {
  const out = new Int8Array(2048);
  for (let index = 0; index < values.length; index++) {
    out[index] = values[index]!;
  }
  return out;
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

describe("decodeChunk light sections", () => {
  test("decodes the isLightOn validity flag", () => {
    expect(decodeChunk(rootWithSections([], 1)).isLightOn).toBe(true);
    expect(decodeChunk(rootWithSections([], 0)).isLightOn).toBe(false);
    expect(decodeChunk(rootWithSections([], "absent")).isLightOn).toBe(false);
  });

  test("decodes BlockLight and SkyLight DataLayers from raw section entries", () => {
    const blockLight = lightBytes([1, 2, 3]);
    const skyLight = lightBytes([15, 14, 13]);
    const chunk = decodeChunk(rootWithSections([
      {
        Y: 0,
        Palette: palette(["minecraft:air"]),
        BlockLight: blockLight,
        SkyLight: skyLight,
      },
    ]));

    expect(chunk.light.block).toHaveLength(1);
    expect(chunk.light.block[0]?.y).toBe(0);
    expect(Array.from(chunk.light.block[0]!.data.slice(0, 3))).toEqual([1, 2, 3]);
    expect(chunk.light.sky).toHaveLength(1);
    expect(chunk.light.sky[0]?.y).toBe(0);
    expect(Array.from(chunk.light.sky[0]!.data.slice(0, 3))).toEqual([15, 14, 13]);
  });

  test("retains light-only sections while keeping block sections palette-backed", () => {
    const chunk = decodeChunk(rootWithSections([
      {
        Y: -1,
        BlockLight: lightBytes([7]),
      },
      {
        Y: 0,
        Palette: palette(["minecraft:air"]),
      },
    ]));

    expect(chunk.sections.map((section) => section.y)).toEqual([0]);
    expect(chunk.light.block.map((section) => section.y)).toEqual([-1]);
    expect(chunk.light.sky).toEqual([]);
  });

  test("sorts light sections by signed section Y", () => {
    const chunk = decodeChunk(rootWithSections([
      { Y: 2, SkyLight: lightBytes([2]) },
      { Y: -1, SkyLight: lightBytes([255]) },
      { Y: 1, SkyLight: lightBytes([1]) },
    ]));

    expect(chunk.light.sky.map((section) => section.y)).toEqual([-1, 1, 2]);
    expect(chunk.light.sky.map((section) => section.data[0])).toEqual([255, 1, 2]);
  });

  test("missing light tags produce empty light layers", () => {
    const chunk = decodeChunk(rootWithSections([
      {
        Y: 0,
        Palette: palette(["minecraft:air"]),
      },
    ]));

    expect(chunk.light).toEqual({ block: [], sky: [] });
  });

  test("rejects light arrays that are not vanilla DataLayer length", () => {
    expect(() =>
      decodeChunk(rootWithSections([
        {
          Y: 0,
          BlockLight: new Int8Array(2047),
        },
      ]))
    ).toThrow(/BlockLight must contain 2048 bytes/);
  });

  test("preserves signed NBT bytes as unsigned fixture bytes", () => {
    const chunk = decodeChunk(rootWithSections([
      {
        Y: 0,
        BlockLight: lightBytes([0, -1, -128, 127]),
      },
    ]));

    expect(Array.from(chunk.light.block[0]!.data.slice(0, 4))).toEqual([0, 255, 128, 127]);
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
