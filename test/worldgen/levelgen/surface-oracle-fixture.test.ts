import { describe, expect, test } from "vitest";
import integrationFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0.json";
import surfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json";
import sandSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-5-115-surface-only.json";
import desertSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-96--64-surface-only.json";
import { CHUNK_BLOCK_NAMES, ChunkBlockId } from "../../../src/worldgen/chunk/chunk-block-buffer.ts";

interface IntegrationChunkFixture {
  readonly chunkX: number;
  readonly chunkZ: number;
}

interface IntegrationFixture {
  readonly seed: string;
  readonly chunks: readonly IntegrationChunkFixture[];
}

interface SurfaceChunkOracleFixture {
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

const fixture = integrationFixture as IntegrationFixture;
const surfaceOracle = surfaceFixture as SurfaceChunkOracleFixture;
const sandSurfaceOracle = sandSurfaceFixture as SurfaceChunkOracleFixture;
const desertSurfaceOracle = desertSurfaceFixture as SurfaceChunkOracleFixture;

function assertPinnedMetadata(oracle: SurfaceChunkOracleFixture, chunkX: number, chunkZ: number, seed: string): void {
  expect(oracle.module).toBe("surface-chunk");
  expect(oracle.minecraftVersion).toBe("1.17.1");
  expect(oracle.generatorClass).toBe("net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator");
  expect(oracle.seed).toBe(seed);
  expect(oracle.chunkX).toBe(chunkX);
  expect(oracle.chunkZ).toBe(chunkZ);
  expect(oracle.minY).toBe(0);
  expect(oracle.height).toBe(256);
  expect(oracle.blockOrder).toBe("y-major,z-major,x-minor");
  expect(oracle.palette).toEqual([...CHUNK_BLOCK_NAMES]);
  expect(oracle.blocks.length).toBe(16 * 16 * 256);
}

describe("surface-stage oracle fixture", () => {
  test("pins the committed seed/chunk metadata and widened block-id palette", () => {
    const chunk = fixture.chunks[0]!;
    assertPinnedMetadata(surfaceOracle, chunk.chunkX, chunk.chunkZ, fixture.seed);
  });

  test("pins the committed widened palette for the desert sandstone oracle", () => {
    assertPinnedMetadata(desertSurfaceOracle, 96, -64, "12345");
  });

  test("contains surface-stage material ids while staying within the widened numeric model", () => {
    expect(surfaceOracle.blocks).toContain(ChunkBlockId.GRASS_BLOCK);
    expect(surfaceOracle.blocks).toContain(ChunkBlockId.DIRT);
    expect(sandSurfaceOracle.blocks).toContain(ChunkBlockId.SAND);
    expect(sandSurfaceOracle.blocks).toContain(ChunkBlockId.GRAVEL);
    expect(desertSurfaceOracle.blocks).toContain(ChunkBlockId.SAND);
    expect(desertSurfaceOracle.blocks).toContain(ChunkBlockId.SANDSTONE);

    for (const oracle of [surfaceOracle, sandSurfaceOracle, desertSurfaceOracle]) {
      expect(oracle.module).toBe("surface-chunk");
      expect(oracle.minecraftVersion).toBe("1.17.1");
      expect(oracle.palette).toEqual([...CHUNK_BLOCK_NAMES]);

      for (const blockId of oracle.blocks) {
        expect(blockId).toBeGreaterThanOrEqual(0);
        expect(blockId).toBeLessThan(oracle.palette.length);
      }
    }
  });
});
