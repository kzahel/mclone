import { describe, expect, test } from "vitest";
import { getLayeredBiomeByKey } from "../../../src/worldgen/biome/biome-data";
import type { MutableChunkBlockBuffer } from "../../../src/worldgen/chunk/chunk-block-buffer";
import type { GeneratedMobEntity } from "../../../src/world/entity/entity-type";
import { ChunkBlockId } from "../../../src/worldgen/chunk/chunk-block-buffer";
import { FlatGrassWorldGenerator, SmallIslandWorldGenerator } from "../../../src/worldgen/levelgen/demo-world-generators";

function surfaceBlockAt(chunk: MutableChunkBlockBuffer, localX: number, localZ: number): { readonly y: number; readonly blockId: number } {
  for (let y = chunk.minY + chunk.height - 1; y >= chunk.minY; y--) {
    const blockId = chunk.getBlockAtY(localX, y, localZ);
    if (blockId !== ChunkBlockId.AIR) {
      return { y, blockId };
    }
  }

  throw new Error(`chunk (${chunk.chunkX.toString()}, ${chunk.chunkZ.toString()}) had no surface block at (${localX.toString()}, ${localZ.toString()})`);
}

describe("demo world generators", () => {
  test("flat grass generator produces a uniform grassy plane", () => {
    const generator = new FlatGrassWorldGenerator(12345n);
    const chunk = generator.fillFromNoise(0, 0);
    const plainsId = getLayeredBiomeByKey("minecraft:plains").getId();

    expect(new Set(chunk.biomes)).toEqual(new Set([plainsId]));
    expect(chunk.getBlockAtY(8, 0, 8)).toBe(ChunkBlockId.BEDROCK);
    expect(chunk.getBlockAtY(8, 59, 8)).toBe(ChunkBlockId.STONE);
    expect(chunk.getBlockAtY(8, 60, 8)).toBe(ChunkBlockId.DIRT);
    expect(chunk.getBlockAtY(8, 63, 8)).toBe(ChunkBlockId.GRASS_BLOCK);
    expect(chunk.getBlockAtY(8, 64, 8)).toBe(ChunkBlockId.AIR);
  });

  test("small island generator creates land near the origin and ocean farther away", () => {
    const generator = new SmallIslandWorldGenerator(12345n);
    const centerChunk = generator.fillFromNoise(0, 0);
    const farChunk = generator.fillFromNoise(12, 12);

    const centerSurface = surfaceBlockAt(centerChunk, 8, 8);
    const farSurface = surfaceBlockAt(farChunk, 8, 8);
    const centerBiomeIds = new Set(centerChunk.biomes);
    const farBiomeIds = new Set(farChunk.biomes);

    expect(centerSurface.y).toBeGreaterThan(62);
    expect([ChunkBlockId.GRASS_BLOCK, ChunkBlockId.SAND]).toContain(centerSurface.blockId);
    expect(centerBiomeIds.has(getLayeredBiomeByKey("minecraft:plains").getId()) || centerBiomeIds.has(getLayeredBiomeByKey("minecraft:beach").getId())).toBe(true);

    expect(farSurface.y).toBeLessThanOrEqual(62);
    expect(farSurface.blockId).toBe(ChunkBlockId.WATER);
    expect(farBiomeIds.has(getLayeredBiomeByKey("minecraft:ocean").getId())).toBe(true);
  });

  test("small island generator places visible starter farm animals near the origin", () => {
    const generator = new SmallIslandWorldGenerator(12345n);
    const spawned: GeneratedMobEntity[] = [];
    let nextId = 100;

    generator.spawnOriginalMobs({} as never, 0, 0, {
      addFreshEntityWithPassengers: (entity) => {
        spawned.push(entity);
      },
    }, {
      nextEntityId: () => nextId++,
      nextEntityUuid: (id) => `mclone:test/small-island-mob/${id.toString()}`,
    });

    expect(spawned).toHaveLength(8);
    expect(spawned.filter((entity) => entity.typeId === "minecraft:cow")).toHaveLength(4);
    expect(spawned.filter((entity) => entity.typeId === "minecraft:pig")).toHaveLength(4);
    expect(spawned.every((entity) => entity.onGround)).toBe(true);
    expect(spawned.map((entity) => [entity.position.x, entity.position.z])).toEqual([
      [6.5, 6.5],
      [10.5, 7.5],
      [7.5, 11.5],
      [12.5, 12.5],
      [3.5, 10.5],
      [4.5, 13.5],
      [13.5, 4.5],
      [14.5, 9.5],
    ]);
    expect(spawned.every((entity) => entity.position.y > 62)).toBe(true);

    const farSpawned: GeneratedMobEntity[] = [];
    generator.spawnOriginalMobs({} as never, 1, 0, {
      addFreshEntityWithPassengers: (entity) => {
        farSpawned.push(entity);
      },
    });
    expect(farSpawned).toEqual([]);
  });
});
