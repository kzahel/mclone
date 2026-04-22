import { describe, expect, test } from "vitest";
import carvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json";
import { CHUNK_BLOCK_NAMES, ChunkBlockId } from "../../../src/worldgen/chunk/chunk-block-buffer.ts";

interface CarvedChunkOracleFixture {
  readonly module: string;
  readonly minecraftVersion: string;
  readonly generatorClass: string;
  readonly seed: string;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly minY: number;
  readonly height: number;
  readonly blockOrder: "y-major,z-major,x-minor";
  readonly palette: readonly string[];
  readonly blocks: readonly number[];
}

const carvedOracle = carvedFixture as CarvedChunkOracleFixture;

describe("carved-stage oracle fixture", () => {
  test("pins the committed carved-stage metadata and widened numeric palette", () => {
    expect(carvedOracle.module).toBe("carved-chunk");
    expect(carvedOracle.minecraftVersion).toBe("1.17.1");
    expect(carvedOracle.generatorClass).toBe("net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator");
    expect(carvedOracle.seed).toBe("12345");
    expect(carvedOracle.chunkX).toBe(0);
    expect(carvedOracle.chunkZ).toBe(0);
    expect(carvedOracle.minY).toBe(0);
    expect(carvedOracle.height).toBe(256);
    expect(carvedOracle.blockOrder).toBe("y-major,z-major,x-minor");
    expect(carvedOracle.palette).toEqual([...CHUNK_BLOCK_NAMES]);
    expect(carvedOracle.blocks.length).toBe(16 * 16 * 256);
  });

  test("contains air carving and lava-floor ids within the numeric model", () => {
    expect(carvedOracle.blocks).toContain(ChunkBlockId.AIR);
    expect(carvedOracle.blocks).toContain(ChunkBlockId.LAVA);

    for (const blockId of carvedOracle.blocks) {
      expect(blockId).toBeGreaterThanOrEqual(0);
      expect(blockId).toBeLessThan(carvedOracle.palette.length);
    }
  });
});
