import { describe, expect, test } from "vitest";

import { buildChunkFixture } from "../../src/oracle/integration/chunk-fixture.ts";
import { decodeChunkLightFixture } from "../../src/oracle/integration/light-fixture.ts";
import type { DecodedChunk } from "../../src/oracle/anvil/chunk.ts";

function fakeChunk(overrides: Partial<DecodedChunk> = {}): DecodedChunk {
  return {
    dataVersion: 2730,
    chunkX: 0,
    chunkZ: 0,
    status: "full",
    isLightOn: true,
    sections: [
      {
        y: 0,
        palette: [{ name: "minecraft:air" }, { name: "minecraft:stone" }],
        blocks: Array.from({ length: 4096 }, (_, index) => (index < 1024 ? 1 : 0)),
      },
    ],
    light: { block: [], sky: [] },
    heightmaps: {
      WORLD_SURFACE: Array.from({ length: 256 }, (_, index) => index),
    },
    biomes: Array.from({ length: 1024 }, (_, index) => index % 32),
    ...overrides,
  };
}

describe("buildChunkFixture", () => {
  test("serializes all metadata and decoded fields in deterministic order", () => {
    const fixture = buildChunkFixture(
      {
        minecraftVersion: "1.17.1",
        seed: "12345",
        generator: "default",
        generateStructures: false,
      },
      [fakeChunk({ chunkX: 1, chunkZ: 0 }), fakeChunk({ chunkX: 0, chunkZ: 0 })],
    );

    expect(fixture.module).toBe("integration");
    expect(fixture.minecraftVersion).toBe("1.17.1");
    expect(fixture.dataVersion).toBe(2730);
    expect(fixture.seed).toBe("12345");
    expect(fixture.generator).toBe("default");
    expect(fixture.generateStructures).toBe(false);
    expect(fixture.wireFormat.blockOrder).toBe("y-major,z-major,x-minor");
    expect(fixture.wireFormat.heightmapOrder).toBe("z-major,x-minor");
    expect(fixture.wireFormat.biomeOrder).toBe("y-major,z-major,x-minor");
    expect(fixture.wireFormat.paletteEntries).toBe("resource-key");
    expect(fixture.wireFormat.lightData).toBe("base64-encoded-2048-byte-datalayer");

    expect(fixture.chunks.map((chunk) => [chunk.chunkX, chunk.chunkZ])).toEqual([
      [0, 0],
      [1, 0],
    ]);

    const section = fixture.chunks[0]!.sections[0]!;
    expect(fixture.chunks[0]!.isLightOn).toBe(true);
    expect(section.y).toBe(0);
    expect(section.palette).toEqual(["minecraft:air", "minecraft:stone"]);
    expect(section.blocks.length).toBe(4096);
    expect(section.blockOrder).toBe("y-major,z-major,x-minor");
    expect(fixture.chunks[0]!.light).toBeUndefined();
  });

  test("serializes light sections as sorted base64 DataLayer bytes", () => {
    const blockHigh = new Uint8Array(2048);
    blockHigh[0] = 255;
    blockHigh[2047] = 17;
    const blockLow = new Uint8Array(2048);
    blockLow[0] = 3;
    const sky = new Uint8Array(2048);
    sky[1] = 128;

    const fixture = buildChunkFixture(
      {
        minecraftVersion: "1.17.1",
        seed: "12345",
        generator: "default",
        generateStructures: false,
      },
      [
        fakeChunk({
          light: {
            block: [
              { y: 2, data: blockHigh },
              { y: -1, data: blockLow },
            ],
            sky: [{ y: 0, data: sky }],
          },
        }),
      ],
    );

    const light = fixture.chunks[0]!.light;
    expect(light?.block.map((section) => section.y)).toEqual([-1, 2]);
    expect(light?.sky.map((section) => section.y)).toEqual([0]);
    expect(light?.block[0]?.dataBase64.length).toBeGreaterThan(0);

    const decoded = decodeChunkLightFixture(light!);
    expect(Array.from(decoded.block[0]!.data.slice(0, 1))).toEqual([3]);
    expect(decoded.block[1]!.data[0]).toBe(255);
    expect(decoded.block[1]!.data[2047]).toBe(17);
    expect(decoded.sky[0]!.data[1]).toBe(128);
  });

  test("rejects chunks with mismatched data versions", () => {
    expect(() =>
      buildChunkFixture(
        { minecraftVersion: "1.17.1", seed: "12345", generator: "default", generateStructures: false },
        [fakeChunk({ chunkX: 0, chunkZ: 0 }), fakeChunk({ chunkX: 1, chunkZ: 0, dataVersion: 2731 })],
      )
    ).toThrow(/mixed data versions/);
  });

  test("rejects chunks whose status is not 'full'", () => {
    expect(() =>
      buildChunkFixture(
        { minecraftVersion: "1.17.1", seed: "12345", generator: "default", generateStructures: false },
        [fakeChunk({ status: "liquid_carvers" })],
      )
    ).toThrow(/requires "full"/);
  });

  test("requires at least one chunk", () => {
    expect(() =>
      buildChunkFixture(
        { minecraftVersion: "1.17.1", seed: "12345", generator: "default", generateStructures: false },
        [],
      )
    ).toThrow(/at least one chunk/);
  });
});
