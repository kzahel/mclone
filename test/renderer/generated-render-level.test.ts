import { afterEach, describe, expect, test } from "vitest";
import surfaceFixture from "../fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
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

function runtimeSurfaceAt(level: GeneratedRenderLevel, worldX: number, worldZ: number): { readonly name: string; readonly y: number } {
  for (let y = level.getMaxBuildHeight() - 1; y >= level.getMinBuildHeight(); y--) {
    const state = level.getBlockState(new BlockPos(worldX, y, worldZ));
    if (!state.isAir()) {
      const name = Registry.BLOCK.getKey(state.getBlock() as unknown as object)?.toString();
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

describe("GeneratedRenderLevel", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("loads the oracle chunk into the camera-centered cache with matching top surfaces", () => {
    const level = createGeneratedLevel();

    expect(level.ensureChunksForCamera(8.5, 8.5, 1)).toBe(true);
    expect(level.getLoadedChunkCount()).toBe(25);
    expect(level.getChunk(0, 0, false)).not.toBeNull();

    for (const [localX, localZ] of [
      [0, 0],
      [8, 8],
      [15, 15],
    ] as const) {
      const expected = oracleSurfaceAt(localX, localZ);
      const actual = runtimeSurfaceAt(level, localX, localZ);
      expect(actual).toEqual(expected);
    }
  });

  test("slides the generated chunk cache when the camera crosses chunk boundaries", () => {
    const level = createGeneratedLevel();

    expect(level.ensureChunksForCamera(8.5, 8.5, 1)).toBe(true);
    expect(level.getChunk(-2, 0, false)).not.toBeNull();
    expect(level.getChunk(2, 0, false)).not.toBeNull();

    expect(level.ensureChunksForCamera(40.5, 8.5, 1)).toBe(true);
    expect(level.getLoadedChunkCount()).toBe(25);
    expect(level.getChunk(-2, 0, false)).toBeNull();
    expect(level.getChunk(0, 0, false)).not.toBeNull();
    expect(level.getChunk(4, 0, false)).not.toBeNull();
  });
});
