import { describe, expect, test } from "vitest";
import {
  deserializePackedChunkSnapshot,
  deserializePackedChunkLightDelta,
  serializePackedChunkLightDelta,
  serializePackedChunkSnapshot,
  type SerializedPackedChunkSnapshot,
} from "../../src/runtime/protocol/packed-chunk-wire";
import { clonePackedChunkSnapshot, type PackedChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";
import { DataLayer } from "../../src/world/level/chunk/data-layer";

const SKY_LIGHT = new Uint8Array(DataLayer.SIZE);
SKY_LIGHT[0] = 0xFF;
SKY_LIGHT[DataLayer.SIZE - 1] = 0x10;

const BLOCK_LIGHT = new Uint8Array(DataLayer.SIZE);
BLOCK_LIGHT[1] = 0x22;

const PACKED_SNAPSHOT: PackedChunkSnapshot = {
  chunkX: -2,
  chunkZ: 3,
  biomes: [1, 2, 3],
  sections: [{
    y: 4,
    paletteStateIds: new Uint32Array([0, 1, 255, 65_535]),
    bitsPerBlock: 4,
    packedBlockIndices: new BigInt64Array([0x0123456789ABCDEFn, -1n]),
  }],
  light: {
    sky: [{ y: -1, data: SKY_LIGHT }],
    block: [{ y: 0, data: BLOCK_LIGHT }],
    lightCorrect: true,
  },
  blockTicks: [{ x: 1, y: 2, z: 3, target: "minecraft:stone", delay: 4 }],
  liquidTicks: [{ x: 5, y: 6, z: 7, target: "minecraft:water", delay: 8 }],
};

describe("packed chunk wire codecs", () => {
  test("round-trips packed snapshots through the JSON-safe wire shape", () => {
    const serialized = serializePackedChunkSnapshot(PACKED_SNAPSHOT);
    const deserialized = deserializePackedChunkSnapshot(serialized);

    expect(serialized.sections[0]!.paletteStateIds).toEqual([0, 1, 255, 65_535]);
    expect(serialized.sections[0]!.packedBlockIndicesBase64).not.toContain("name");
    expect(serialized.light?.sky[0]?.dataBase64).toBeTruthy();
    expect(deserialized).toEqual(PACKED_SNAPSHOT);
  });

  test("clones packed snapshots without sharing typed-array backing stores", () => {
    const cloned = clonePackedChunkSnapshot(PACKED_SNAPSHOT);

    cloned.sections[0]!.paletteStateIds[0] = 99;
    cloned.sections[0]!.packedBlockIndices[0] = 0n;
    cloned.light!.sky[0]!.data[0] = 0;

    expect(PACKED_SNAPSHOT.sections[0]!.paletteStateIds[0]).toBe(0);
    expect(PACKED_SNAPSHOT.sections[0]!.packedBlockIndices[0]).toBe(0x0123456789ABCDEFn);
    expect(PACKED_SNAPSHOT.light!.sky[0]!.data[0]).toBe(0xFF);
  });

  test("round-trips light deltas with explicit empty sections", () => {
    const serialized = serializePackedChunkLightDelta({
      sky: [{ y: 5, data: SKY_LIGHT }],
      block: [{ y: 5 }],
    });

    expect(serialized.sky?.[0]?.dataBase64).toBeTruthy();
    expect(serialized.block?.[0]?.dataBase64).toBeUndefined();
    expect(deserializePackedChunkLightDelta(serialized)).toEqual({
      sky: [{ y: 5, data: SKY_LIGHT }],
      block: [{ y: 5 }],
    });
  });

  test("rejects packed word payloads that are not whole 64-bit words", () => {
    const serialized: SerializedPackedChunkSnapshot = {
      ...serializePackedChunkSnapshot(PACKED_SNAPSHOT),
      sections: [{
        ...serializePackedChunkSnapshot(PACKED_SNAPSHOT).sections[0]!,
        packedBlockIndicesBase64: "AA==",
      }],
    };

    expect(() => deserializePackedChunkSnapshot(serialized)).toThrow(/multiple of 8/);
  });

  test("rejects light payloads that are not whole DataLayers", () => {
    const serialized: SerializedPackedChunkSnapshot = {
      ...serializePackedChunkSnapshot(PACKED_SNAPSHOT),
      light: {
        ...serializePackedChunkSnapshot(PACKED_SNAPSHOT).light!,
        block: [{
          y: 0,
          dataBase64: "AA==",
        }],
      },
    };

    expect(() => deserializePackedChunkSnapshot(serialized)).toThrow(/2048/);
  });
});
