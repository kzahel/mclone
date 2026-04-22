import { describe, expect, test } from "vitest";
import integrationFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0.json";
import terrainFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json";
import { OverworldBiomeSource } from "../../../src/worldgen/biome/overworld-biome-source.ts";
import {
  buildTerrainChunk,
  NoiseBasedChunkGenerator,
  TERRAIN_BLOCK_NAMES,
  type TerrainChunkSection,
} from "../../../src/worldgen/levelgen/noise-based-chunk-generator.ts";
import { NoiseGeneratorSettings } from "../../../src/worldgen/levelgen/noise-generator-settings.ts";

interface IntegrationChunkSectionFixture {
  readonly y: number;
  readonly palette: readonly string[];
  readonly blocks: readonly number[];
}

interface IntegrationChunkFixture {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly sections: readonly IntegrationChunkSectionFixture[];
  readonly biomes: readonly number[];
}

interface IntegrationFixture {
  readonly seed: string;
  readonly chunks: readonly IntegrationChunkFixture[];
}

interface TerrainChunkOracleFixture {
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
const terrainOracle = terrainFixture as TerrainChunkOracleFixture;

function blockNameAt(section: TerrainChunkSection, index: number): string {
  return section.palette[section.blocks[index]!]!;
}

function assertTerrainSections(actual: readonly TerrainChunkSection[], expected: readonly TerrainChunkSection[]): void {
  expect(actual.length).toBe(expected.length);

  for (let sectionIndex = 0; sectionIndex < expected.length; sectionIndex++) {
    const actualSection = actual[sectionIndex]!;
    const expectedSection = expected[sectionIndex]!;
    expect(actualSection.y).toBe(expectedSection.y);
    expect(actualSection.blockOrder).toBe(expectedSection.blockOrder);
    expect(actualSection.blocks.length).toBe(expectedSection.blocks.length);

    for (let index = 0; index < expectedSection.blocks.length; index++) {
      const actualBlock = blockNameAt(actualSection, index);
      const expectedBlock = blockNameAt(expectedSection, index);
      if (actualBlock !== expectedBlock) {
        const localX = index & 15;
        const localZ = (index >> 4) & 15;
        const localY = index >> 8;
        throw new Error(
          `section y=${actualSection.y} mismatch at local (${localX}, ${localY}, ${localZ}): expected ${expectedBlock}, got ${actualBlock}`,
        );
      }
    }
  }
}

function assertHeightmap(name: string, actual: readonly number[], expected: readonly number[]): void {
  expect(actual.length).toBe(expected.length);

  for (let index = 0; index < expected.length; index++) {
    if (actual[index] !== expected[index]) {
      const localX = index & 15;
      const localZ = index >> 4;
      throw new Error(
        `${name} mismatch at local (${localX}, ${localZ}): expected ${expected[index]}, got ${actual[index]}`,
      );
    }
  }
}

describe("NoiseBasedChunkGenerator", () => {
  test("default overworld settings keep all dormant C&C Part 1 toggles disabled", () => {
    const settings = NoiseGeneratorSettings.overworld();
    expect(settings.getDefaultBlock()).toBe("minecraft:stone");
    expect(settings.getDefaultFluid()).toBe("minecraft:water");
    expect(settings.getBedrockFloorPosition()).toBe(0);
    expect(settings.seaLevel()).toBe(63);
    expect(settings.isAquifersEnabled()).toBe(false);
    expect(settings.isNoiseCavesEnabled()).toBe(false);
    expect(settings.isDeepslateEnabled()).toBe(false);
    expect(settings.isOreVeinsEnabled()).toBe(false);
    expect(settings.isNoodleCavesEnabled()).toBe(false);
  });

  test("terrain-only oracle fixture stays pinned to the same seed/chunk and generator target", () => {
    const chunk = fixture.chunks[0]!;
    expect(terrainOracle.module).toBe("terrain-chunk");
    expect(terrainOracle.minecraftVersion).toBe("1.17.1");
    expect(terrainOracle.generatorClass).toBe("net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator");
    expect(terrainOracle.seed).toBe(fixture.seed);
    expect(terrainOracle.chunkX).toBe(chunk.chunkX);
    expect(terrainOracle.chunkZ).toBe(chunk.chunkZ);
    expect(terrainOracle.blockOrder).toBe("y-major,z-major,x-minor");
    expect(terrainOracle.palette).toEqual([...TERRAIN_BLOCK_NAMES]);
    expect(terrainOracle.height).toBe(256);
    expect(terrainOracle.minY).toBe(0);
  });

  test("fills chunk (0, 0) with the terrain-only Java oracle for the committed integration fixture seed/chunk", () => {
    const chunk = fixture.chunks[0]!;
    const biomeSource = new OverworldBiomeSource(BigInt(fixture.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(fixture.seed));

    const actual = generator.fillFromNoise(chunk.chunkX, chunk.chunkZ);
    const expected = buildTerrainChunk(
      terrainOracle.chunkX,
      terrainOracle.chunkZ,
      terrainOracle.minY,
      terrainOracle.height,
      Uint8Array.from(terrainOracle.blocks),
      chunk.biomes,
    );

    expect(actual.biomes).toEqual(expected.biomes);
    assertTerrainSections(actual.sections, expected.sections);
    assertHeightmap("WORLD_SURFACE", actual.heightmaps.WORLD_SURFACE, expected.heightmaps.WORLD_SURFACE);
    assertHeightmap("OCEAN_FLOOR", actual.heightmaps.OCEAN_FLOOR, expected.heightmaps.OCEAN_FLOOR);

    for (const section of actual.sections) {
      for (const blockName of section.palette) {
        expect(TERRAIN_BLOCK_NAMES).toContain(blockName);
      }
    }
  });
});
