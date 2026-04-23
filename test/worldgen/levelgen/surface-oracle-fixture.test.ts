import { describe, expect, test } from "vitest";
import integrationFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0.json";
import surfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json";
import sandSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-5-115-surface-only.json";
import desertSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-96--64-surface-only.json";
import frozenSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks--247--247-surface-only.json";
import badlandsSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks--320-99-surface-only.json";
import giantTaigaSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks--9-68-surface-only.json";
import shatteredSavannaSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-60-199-surface-only.json";
import mushroomSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks--446-387-surface-only.json";
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
const frozenSurfaceOracle = frozenSurfaceFixture as SurfaceChunkOracleFixture;
const badlandsSurfaceOracle = badlandsSurfaceFixture as SurfaceChunkOracleFixture;
const giantTaigaSurfaceOracle = giantTaigaSurfaceFixture as SurfaceChunkOracleFixture;
const shatteredSavannaSurfaceOracle = shatteredSavannaSurfaceFixture as SurfaceChunkOracleFixture;
const mushroomSurfaceOracle = mushroomSurfaceFixture as SurfaceChunkOracleFixture;

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

function topColumnCounts(oracle: SurfaceChunkOracleFixture): Readonly<Record<string, number>> {
  const counts = new Map<string, number>();

  for (let localX = 0; localX < 16; localX++) {
    for (let localZ = 0; localZ < 16; localZ++) {
      let topBlock = "minecraft:air";

      for (let localY = oracle.height - 1; localY >= 0; localY--) {
        const blockName = oracle.palette[oracle.blocks[(localY << 8) | (localZ << 4) | localX]!]!;
        if (blockName !== "minecraft:air" && blockName !== "minecraft:water") {
          topBlock = blockName;
          break;
        }
      }

      counts.set(topBlock, (counts.get(topBlock) ?? 0) + 1);
    }
  }

  return Object.fromEntries([...counts.entries()].sort((left, right) => right[1] - left[1]));
}

describe("surface-stage oracle fixture", () => {
  test("pins the committed seed/chunk metadata and widened block-id palette", () => {
    const chunk = fixture.chunks[0]!;
    assertPinnedMetadata(surfaceOracle, chunk.chunkX, chunk.chunkZ, fixture.seed);
  });

  test("pins the committed widened palette for the desert sandstone oracle", () => {
    assertPinnedMetadata(desertSurfaceOracle, 96, -64, "12345");
  });

  test("pins the committed widened palette for the frozen-ocean surface oracle", () => {
    assertPinnedMetadata(frozenSurfaceOracle, -247, -247, "12345");
  });

  test("pins the committed widened palette for the badlands surface oracle", () => {
    assertPinnedMetadata(badlandsSurfaceOracle, -320, 99, "12345");
  });

  test("pins the committed widened palette for the giant-tree taiga surface oracle", () => {
    assertPinnedMetadata(giantTaigaSurfaceOracle, -9, 68, "12345");
  });

  test("pins the committed widened palette for the shattered-savanna surface oracle", () => {
    assertPinnedMetadata(shatteredSavannaSurfaceOracle, 60, 199, "12345");
  });

  test("pins the committed widened palette for the mushroom-fields surface oracle", () => {
    assertPinnedMetadata(mushroomSurfaceOracle, -446, 387, "12345");
  });

  test("giant-tree taiga surface oracle pins the podzol/coarse-dirt top-column mix", () => {
    expect(topColumnCounts(giantTaigaSurfaceOracle)).toEqual({
      "minecraft:podzol": 252,
      "minecraft:coarse_dirt": 4,
    });
  });

  test("shattered-savanna surface oracle pins the coarse-dirt/stone top-column mix", () => {
    expect(topColumnCounts(shatteredSavannaSurfaceOracle)).toEqual({
      "minecraft:coarse_dirt": 193,
      "minecraft:stone": 63,
    });
  });

  test("mushroom-fields surface oracle pins the full mycelium top-column cover", () => {
    expect(topColumnCounts(mushroomSurfaceOracle)).toEqual({
      "minecraft:mycelium": 256,
    });
  });

  test("contains surface-stage material ids while staying within the widened numeric model", () => {
    expect(surfaceOracle.blocks).toContain(ChunkBlockId.GRASS_BLOCK);
    expect(surfaceOracle.blocks).toContain(ChunkBlockId.DIRT);
    expect(sandSurfaceOracle.blocks).toContain(ChunkBlockId.SAND);
    expect(sandSurfaceOracle.blocks).toContain(ChunkBlockId.GRAVEL);
    expect(desertSurfaceOracle.blocks).toContain(ChunkBlockId.SAND);
    expect(desertSurfaceOracle.blocks).toContain(ChunkBlockId.SANDSTONE);
    expect(frozenSurfaceOracle.blocks).toContain(ChunkBlockId.PACKED_ICE);
    expect(frozenSurfaceOracle.blocks).toContain(ChunkBlockId.SNOW_BLOCK);
    expect(badlandsSurfaceOracle.blocks).toContain(ChunkBlockId.RED_SAND);
    expect(badlandsSurfaceOracle.blocks).toContain(ChunkBlockId.TERRACOTTA);
    expect(badlandsSurfaceOracle.blocks).toContain(ChunkBlockId.ORANGE_TERRACOTTA);
    expect(badlandsSurfaceOracle.blocks).toContain(ChunkBlockId.LIGHT_GRAY_TERRACOTTA);
    expect(giantTaigaSurfaceOracle.blocks).toContain(ChunkBlockId.PODZOL);
    expect(giantTaigaSurfaceOracle.blocks).toContain(ChunkBlockId.COARSE_DIRT);
    expect(shatteredSavannaSurfaceOracle.blocks).toContain(ChunkBlockId.COARSE_DIRT);
    expect(mushroomSurfaceOracle.blocks).toContain(ChunkBlockId.MYCELIUM);

    for (const oracle of [
      surfaceOracle,
      sandSurfaceOracle,
      desertSurfaceOracle,
      frozenSurfaceOracle,
      badlandsSurfaceOracle,
      giantTaigaSurfaceOracle,
      shatteredSavannaSurfaceOracle,
      mushroomSurfaceOracle,
    ]) {
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
