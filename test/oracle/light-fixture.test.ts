import { describe, expect, test } from "vitest";

import {
  base64ToLightBytes,
  compareChunkLight,
  lightBytesToBase64,
  type ChunkLightBytes,
} from "../../src/oracle/integration/light-fixture.ts";

function bytes(values: readonly number[], length = 2048): Uint8Array {
  const out = new Uint8Array(length);
  for (let index = 0; index < values.length; index++) {
    out[index] = values[index]!;
  }
  return out;
}

function light(overrides: Partial<ChunkLightBytes> = {}): ChunkLightBytes {
  return {
    sky: [],
    block: [],
    ...overrides,
  };
}

describe("light fixture base64", () => {
  test("round-trips exact DataLayer bytes", () => {
    const data = bytes([0, 255, 128, 127]);
    data[2047] = 42;

    const encoded = lightBytesToBase64(data);
    const decoded = base64ToLightBytes(encoded);

    expect(decoded).toHaveLength(2048);
    expect(Array.from(decoded.slice(0, 4))).toEqual([0, 255, 128, 127]);
    expect(decoded[2047]).toBe(42);
  });

  test("rejects non-DataLayer byte lengths", () => {
    expect(() => lightBytesToBase64(bytes([], 2047))).toThrow(/2048 bytes/);
    expect(() => base64ToLightBytes(lightBytesToBase64(bytes([])).slice(4))).toThrow(/2048 bytes/);
  });
});

describe("compareChunkLight", () => {
  test("identical light records produce no diffs", () => {
    const data = bytes([1, 2, 3]);
    const expected = light({ block: [{ y: 0, data }] });
    const actual = light({ block: [{ y: 0, data: bytes([1, 2, 3]) }] });

    expect(compareChunkLight(expected, actual)).toEqual([]);
  });

  test("reports missing and extra sections independently by layer", () => {
    const expected = light({
      sky: [{ y: 0, data: bytes([15]) }],
      block: [{ y: 1, data: bytes([9]) }],
    });
    const actual = light({
      sky: [{ y: 2, data: bytes([15]) }],
    });

    expect(compareChunkLight(expected, actual)).toEqual([
      { layer: "sky", y: 0, kind: "missing" },
      { layer: "sky", y: 2, kind: "extra" },
      { layer: "block", y: 1, kind: "missing" },
    ]);
  });

  test("reports the first mismatched byte", () => {
    const expected = light({ block: [{ y: -1, data: bytes([1, 2, 3]) }] });
    const actual = light({ block: [{ y: -1, data: bytes([1, 7, 3]) }] });

    expect(compareChunkLight(expected, actual)).toEqual([
      {
        layer: "block",
        y: -1,
        kind: "byte_mismatch",
        firstByteOffset: 1,
        expected: 2,
        actual: 7,
      },
    ]);
  });

  test("reports length mismatches after the shared prefix", () => {
    const expected = light({ sky: [{ y: 0, data: bytes([1, 2], 2048) }] });
    const actual = light({ sky: [{ y: 0, data: bytes([1, 2], 2047) }] });

    expect(compareChunkLight(expected, actual)).toEqual([
      {
        layer: "sky",
        y: 0,
        kind: "byte_mismatch",
        firstByteOffset: 2047,
        expected: 0,
        actual: undefined,
      },
    ]);
  });
});
