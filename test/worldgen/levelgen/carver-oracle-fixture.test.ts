import { describe, expect, test } from "vitest";
import carvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json";
import oceanCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-117--128-carved-only.json";
import desertCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-96--64-carved-only.json";
import liquidOceanCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-117--128-liquid-carved.json";
import liquidFloorCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks--129--256-liquid-carved.json";
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
  readonly blockTicks: readonly {
    readonly x: number;
    readonly y: number;
    readonly z: number;
    readonly target: string;
    readonly delay: number;
  }[];
  readonly liquidTicks: readonly {
    readonly x: number;
    readonly y: number;
    readonly z: number;
    readonly target: string;
    readonly delay: number;
  }[];
}

const carvedOracle = carvedFixture as CarvedChunkOracleFixture;
const oceanCarvedOracle = oceanCarvedFixture as CarvedChunkOracleFixture;
const desertCarvedOracle = desertCarvedFixture as CarvedChunkOracleFixture;
const liquidOceanCarvedOracle = liquidOceanCarvedFixture as CarvedChunkOracleFixture;
const liquidFloorCarvedOracle = liquidFloorCarvedFixture as CarvedChunkOracleFixture;

function assertPinnedMetadata(
  oracle: CarvedChunkOracleFixture,
  chunkX: number,
  chunkZ: number,
  module: "carved-chunk" | "liquid-carved-chunk",
): void {
  expect(oracle.module).toBe(module);
  expect(oracle.minecraftVersion).toBe("1.17.1");
  expect(oracle.generatorClass).toBe("net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator");
  expect(oracle.seed).toBe("12345");
  expect(oracle.chunkX).toBe(chunkX);
  expect(oracle.chunkZ).toBe(chunkZ);
  expect(oracle.minY).toBe(0);
  expect(oracle.height).toBe(256);
  expect(oracle.blockOrder).toBe("y-major,z-major,x-minor");
  expect(oracle.palette).toEqual([...CHUNK_BLOCK_NAMES]);
  expect(oracle.blocks.length).toBe(16 * 16 * 256);
  expect(oracle.blockTicks).toBeDefined();
  expect(oracle.liquidTicks).toBeDefined();
}

describe("carved-stage oracle fixture", () => {
  test("pins the committed carved-stage metadata and widened numeric palette for the spawn oracle", () => {
    assertPinnedMetadata(carvedOracle, 0, 0, "carved-chunk");
  });

  test("pins the committed carved-stage metadata and widened numeric palette for the ocean oracle", () => {
    assertPinnedMetadata(oceanCarvedOracle, 117, -128, "carved-chunk");
  });

  test("pins the committed carved-stage metadata and widened numeric palette for the desert oracle", () => {
    assertPinnedMetadata(desertCarvedOracle, 96, -64, "carved-chunk");
  });

  test("pins the committed air-plus-liquid carved-stage metadata for the ocean oracle", () => {
    assertPinnedMetadata(liquidOceanCarvedOracle, 117, -128, "liquid-carved-chunk");
  });

  test("pins the committed air-plus-liquid carved-stage metadata for the underwater-floor oracle", () => {
    assertPinnedMetadata(liquidFloorCarvedOracle, -129, -256, "liquid-carved-chunk");
  });

  test("spawn carved oracle contains air carving and lava-floor ids within the numeric model", () => {
    expect(carvedOracle.blocks).toContain(ChunkBlockId.AIR);
    expect(carvedOracle.blocks).toContain(ChunkBlockId.LAVA);

    for (const blockId of carvedOracle.blocks) {
      expect(blockId).toBeGreaterThanOrEqual(0);
      expect(blockId).toBeLessThan(carvedOracle.palette.length);
    }
  });

  test("ocean carved oracle contains air carving, retained water, and lava-floor ids", () => {
    expect(oceanCarvedOracle.blocks).toContain(ChunkBlockId.AIR);
    expect(oceanCarvedOracle.blocks).toContain(ChunkBlockId.WATER);
    expect(oceanCarvedOracle.blocks).toContain(ChunkBlockId.LAVA);

    for (const blockId of oceanCarvedOracle.blocks) {
      expect(blockId).toBeGreaterThanOrEqual(0);
      expect(blockId).toBeLessThan(oceanCarvedOracle.palette.length);
    }
  });

  test("desert carved oracle contains sand, sandstone, air carving, and lava-floor ids", () => {
    expect(desertCarvedOracle.blocks).toContain(ChunkBlockId.SAND);
    expect(desertCarvedOracle.blocks).toContain(ChunkBlockId.SANDSTONE);
    expect(desertCarvedOracle.blocks).toContain(ChunkBlockId.AIR);
    expect(desertCarvedOracle.blocks).toContain(ChunkBlockId.LAVA);

    for (const blockId of desertCarvedOracle.blocks) {
      expect(blockId).toBeGreaterThanOrEqual(0);
      expect(blockId).toBeLessThan(desertCarvedOracle.palette.length);
    }
  });

  test("liquid-carved ocean oracle stays within the widened numeric model and captures the LIQUID-step water fill", () => {
    expect(liquidOceanCarvedOracle.blocks).toContain(ChunkBlockId.WATER);
    expect(liquidOceanCarvedOracle.blocks).not.toEqual(oceanCarvedOracle.blocks);
    expect(
      liquidOceanCarvedOracle.blocks.filter((blockId) => blockId === ChunkBlockId.WATER).length,
    ).toBeGreaterThan(
      oceanCarvedOracle.blocks.filter((blockId) => blockId === ChunkBlockId.WATER).length,
    );

    for (const blockId of liquidOceanCarvedOracle.blocks) {
      expect(blockId).toBeGreaterThanOrEqual(0);
      expect(blockId).toBeLessThan(liquidOceanCarvedOracle.palette.length);
    }

    expect(liquidOceanCarvedOracle.blockTicks).toEqual([]);
    expect(liquidOceanCarvedOracle.liquidTicks.length).toBeGreaterThan(0);
    expect(liquidOceanCarvedOracle.liquidTicks.every((tick) => tick.target === "minecraft:water")).toBe(true);
  });

  test("liquid-carved underwater-floor oracle commits obsidian, magma, and their scheduled tick consequences", () => {
    expect(liquidFloorCarvedOracle.blocks).toContain(ChunkBlockId.OBSIDIAN);
    expect(liquidFloorCarvedOracle.blocks).toContain(ChunkBlockId.MAGMA_BLOCK);
    expect(liquidFloorCarvedOracle.blockTicks.length).toBeGreaterThan(0);
    expect(liquidFloorCarvedOracle.blockTicks.every((tick) => tick.target === "minecraft:magma_block")).toBe(true);
    expect(liquidFloorCarvedOracle.liquidTicks.length).toBeGreaterThan(0);
    expect(liquidFloorCarvedOracle.liquidTicks.every((tick) => tick.target === "minecraft:water")).toBe(true);

    for (const blockId of liquidFloorCarvedOracle.blocks) {
      expect(blockId).toBeGreaterThanOrEqual(0);
      expect(blockId).toBeLessThan(liquidFloorCarvedOracle.palette.length);
    }
  });
});
