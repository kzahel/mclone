import { describe, expect, test } from "vitest";
import integrationFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0.json";
import surfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json";
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

describe("surface-stage oracle fixture", () => {
  test("pins the committed seed/chunk metadata and widened block-id palette", () => {
    const chunk = fixture.chunks[0]!;

    expect(surfaceOracle.module).toBe("surface-chunk");
    expect(surfaceOracle.minecraftVersion).toBe("1.17.1");
    expect(surfaceOracle.generatorClass).toBe("net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator");
    expect(surfaceOracle.seed).toBe(fixture.seed);
    expect(surfaceOracle.chunkX).toBe(chunk.chunkX);
    expect(surfaceOracle.chunkZ).toBe(chunk.chunkZ);
    expect(surfaceOracle.minY).toBe(0);
    expect(surfaceOracle.height).toBe(256);
    expect(surfaceOracle.blockOrder).toBe("y-major,z-major,x-minor");
    expect(surfaceOracle.palette).toEqual([...CHUNK_BLOCK_NAMES]);
    expect(surfaceOracle.blocks.length).toBe(16 * 16 * 256);
  });

  test("contains surface-stage material ids while staying within the widened numeric model", () => {
    expect(surfaceOracle.blocks).toContain(ChunkBlockId.GRASS_BLOCK);
    expect(surfaceOracle.blocks).toContain(ChunkBlockId.DIRT);

    for (const blockId of surfaceOracle.blocks) {
      expect(blockId).toBeGreaterThanOrEqual(0);
      expect(blockId).toBeLessThan(surfaceOracle.palette.length);
    }
  });
});
