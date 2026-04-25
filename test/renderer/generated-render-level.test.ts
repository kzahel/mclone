import { afterEach, describe, expect, test } from "vitest";
import surfaceFixture from "../fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../../src/worldgen/chunk/chunk-block-buffer";
import { NoiseBasedChunkGenerator } from "../../src/worldgen/levelgen/noise-based-chunk-generator";
import { GeneratedRenderLevel } from "../../src/world/level/generated-render-level";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";

interface SurfaceChunkOracleFixture {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly minY: number;
  readonly height: number;
  readonly palette: readonly string[];
  readonly blocks: readonly number[];
}

const oracle = surfaceFixture as SurfaceChunkOracleFixture;
const DECORATION_BLOCKS = new Set([
  "minecraft:oak_log",
  "minecraft:oak_leaves",
  "minecraft:spruce_log",
  "minecraft:spruce_leaves",
  "minecraft:grass",
  "minecraft:fern",
  "minecraft:large_fern",
  "minecraft:oak_sapling",
  "minecraft:spruce_sapling",
  "minecraft:sweet_berry_bush",
  "minecraft:brown_mushroom",
  "minecraft:red_mushroom",
  "minecraft:sugar_cane",
  "minecraft:cactus",
  "minecraft:pumpkin",
]);
const GENERATED_RENDER_LEVEL_TIMEOUT_MS = 30_000;

function oracleBlockNameAt(localX: number, y: number, localZ: number): string {
  const index = ((y - oracle.minY) << 8) | (localZ << 4) | localX;
  return oracle.palette[oracle.blocks[index]!]!;
}

function oracleSurfaceAt(localX: number, localZ: number): { readonly name: string; readonly y: number } {
  for (let y = oracle.minY + oracle.height - 1; y >= oracle.minY; y--) {
    const name = oracleBlockNameAt(localX, y, localZ);
    if (name !== "minecraft:air") {
      return { name, y };
    }
  }

  throw new Error(`oracle chunk (${oracle.chunkX}, ${oracle.chunkZ}) had no surface block at (${localX}, ${localZ})`);
}

function runtimeGroundSurfaceAt(level: GeneratedRenderLevel, worldX: number, worldZ: number): { readonly name: string; readonly y: number } {
  for (let y = level.getMaxBuildHeight() - 1; y >= level.getMinBuildHeight(); y--) {
    const state = level.getBlockState(new BlockPos(worldX, y, worldZ));
    if (!state.isAir()) {
      const name = Registry.BLOCK.getKey(state.getBlock() as unknown as object)?.toString();
      if (name !== undefined && DECORATION_BLOCKS.has(name)) {
        continue;
      }

      return { name: name ?? "unregistered", y };
    }
  }

  throw new Error(`runtime level had no surface block at (${worldX}, ${worldZ})`);
}

function createGeneratedLevel(): GeneratedRenderLevel {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);
  const generator = new NoiseBasedChunkGenerator(biomeSource, 12345n);
  return new GeneratedRenderLevel(blocks.airState, generator, biomeSource, 12345n, blocks.blockStateById);
}

function hasPublishedChunk(level: GeneratedRenderLevel, chunkX: number, chunkZ: number): boolean {
  return level.getLoadedChunks().some((chunk) => chunk.chunkX === chunkX && chunk.chunkZ === chunkZ);
}

describe("GeneratedRenderLevel", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("registers render states for widened frozen and badlands terrain ids", () => {
    const blocks = registerGeneratedRenderBlocks();
    const expected = [
      [ChunkBlockId.RED_SAND, "minecraft:red_sand"],
      [ChunkBlockId.TERRACOTTA, "minecraft:terracotta"],
      [ChunkBlockId.ORANGE_TERRACOTTA, "minecraft:orange_terracotta"],
      [ChunkBlockId.PACKED_ICE, "minecraft:packed_ice"],
      [ChunkBlockId.ICE, "minecraft:ice"],
      [ChunkBlockId.SNOW_BLOCK, "minecraft:snow_block"],
    ] as const;

    for (const [blockId, key] of expected) {
      const state = blocks.blockStateById[blockId];
      expect(state).toBeDefined();
      expect(state!.isAir()).toBe(false);
      expect(Registry.BLOCK.getKey(state!.getBlock() as unknown as object)?.toString()).toBe(key);
    }
  }, GENERATED_RENDER_LEVEL_TIMEOUT_MS);

  test("loads the oracle chunk into the camera-centered cache with matching top surfaces", () => {
    const level = createGeneratedLevel();

    expect(level.ensureChunksForCamera(8.5, 8.5, 1)).toBe(true);
    expect(level.getLoadedChunkCount()).toBe(25);
    expect(hasPublishedChunk(level, 0, 0)).toBe(true);

    for (const [localX, localZ] of [
      [0, 0],
      [8, 8],
      [15, 15],
    ] as const) {
      const expected = oracleSurfaceAt(localX, localZ);
      const actual = runtimeGroundSurfaceAt(level, localX, localZ);
      expect(actual).toEqual(expected);
    }
  }, GENERATED_RENDER_LEVEL_TIMEOUT_MS);

  test("keeps FEATURES stability and FULL publishability as separate status gates", () => {
    const level = createGeneratedLevel();

    expect(level.ensureChunksForCamera(8.5, 8.5, 1)).toBe(true);
    expect(level.isChunkDecorated(0, 0)).toBe(true);
    expect(level.isChunkFeaturesStable(0, 0)).toBe(true);
    expect(level.isChunkFull(0, 0)).toBe(false);
    expect(level.isChunkPublishable(0, 0)).toBe(false);

    for (let chunkZ = -1; chunkZ <= 1; chunkZ++) {
      for (let chunkX = -1; chunkX <= 1; chunkX++) {
        expect(level.markChunkLighted(chunkX, chunkZ)).toBe(true);
        expect(level.markChunkFull(chunkX, chunkZ)).toBe(true);
      }
    }

    expect(level.isChunkFull(0, 0)).toBe(true);
    expect(level.isChunkPublishable(0, 0)).toBe(true);
    expect(level.isChunkPublishable(1, 1)).toBe(false);
    expect(level.markChunkLighted(2, 2)).toBe(false);
  }, GENERATED_RENDER_LEVEL_TIMEOUT_MS);

  test("slides the generated chunk cache when the camera crosses chunk boundaries", () => {
    const level = createGeneratedLevel();

    expect(level.ensureChunksForCamera(8.5, 8.5, 1)).toBe(true);
    expect(hasPublishedChunk(level, -2, 0)).toBe(true);
    expect(hasPublishedChunk(level, 2, 0)).toBe(true);

    expect(level.ensureChunksForCamera(40.5, 8.5, 1)).toBe(true);
    expect(level.getLoadedChunkCount()).toBe(25);
    expect(hasPublishedChunk(level, -2, 0)).toBe(false);
    expect(level.getAuthorityChunk(-2, 0)).not.toBeNull();
    expect(hasPublishedChunk(level, 0, 0)).toBe(true);
    expect(hasPublishedChunk(level, 4, 0)).toBe(true);
  }, GENERATED_RENDER_LEVEL_TIMEOUT_MS);

  test("generated chunks gain biome-driven tree and ground vegetation blocks", () => {
    const level = createGeneratedLevel();

    expect(level.ensureChunksForCamera(40.5, 40.5, 1)).toBe(true);

    let foundTree = false;
    let foundGroundPlant = false;
    for (const chunk of level.getLoadedChunks()) {
      const minX = chunk.chunkX * 16;
      const minZ = chunk.chunkZ * 16;
      for (let y = level.getMinBuildHeight(); y < level.getMaxBuildHeight(); y++) {
        for (let localZ = 0; localZ < 16; localZ++) {
          for (let localX = 0; localX < 16; localX++) {
            const name = Registry.BLOCK.getKey(level.getBlockState(new BlockPos(minX + localX, y, minZ + localZ)).getBlock() as unknown as object)?.toString();
            if (name === "minecraft:spruce_log" || name === "minecraft:oak_log") {
              foundTree = true;
            } else if (name === "minecraft:grass" || name === "minecraft:fern") {
              foundGroundPlant = true;
            }
          }
        }
      }
    }

    expect(foundTree).toBe(true);
    expect(foundGroundPlant).toBe(true);
  }, GENERATED_RENDER_LEVEL_TIMEOUT_MS);
});
