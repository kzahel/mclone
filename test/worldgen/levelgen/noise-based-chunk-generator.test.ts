import { describe, expect, test } from "vitest";
import integrationFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0.json";
import terrainFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json";
import surfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json";
import carvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json";
import sandSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-5-115-surface-only.json";
import { OverworldBiomeSource } from "../../../src/worldgen/biome/overworld-biome-source.ts";
import {
  ChunkBlockId,
  CHUNK_BLOCK_NAMES,
  MutableChunkBlockBuffer,
  TERRAIN_STAGE_BLOCK_NAMES,
} from "../../../src/worldgen/chunk/chunk-block-buffer.ts";
import {
  buildTerrainChunk,
  NoiseBasedChunkGenerator,
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

interface SurfaceChunkOracleFixture extends TerrainChunkOracleFixture {}
interface CarvedChunkOracleFixture extends TerrainChunkOracleFixture {}

const fixture = integrationFixture as IntegrationFixture;
const terrainOracle = terrainFixture as TerrainChunkOracleFixture;
const surfaceOracle = surfaceFixture as SurfaceChunkOracleFixture;
const carvedOracle = carvedFixture as CarvedChunkOracleFixture;
const sandSurfaceOracle = sandSurfaceFixture as SurfaceChunkOracleFixture;

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

function terrainStageBlocksFromTerrainOracle(oracle: TerrainChunkOracleFixture): Uint8Array {
  // Tactical 06's oracle captured terrain after the old bottom-bedrock pass. For
  // the explicit 06a/07 staging boundary we compare the fill stage by removing
  // only that deterministic bottom overlay.
  return Uint8Array.from(oracle.blocks, (blockId) => (blockId === ChunkBlockId.BEDROCK ? ChunkBlockId.STONE : blockId));
}

function assertChunkParity(actualChunk: MutableChunkBlockBuffer, oracle: SurfaceChunkOracleFixture): void {
  const actual = buildTerrainChunk(actualChunk);
  const expected = buildTerrainChunk(
    new MutableChunkBlockBuffer(
      oracle.chunkX,
      oracle.chunkZ,
      oracle.minY,
      oracle.height,
      [],
      Uint8Array.from(oracle.blocks),
    ),
  );

  assertTerrainSections(actual.sections, expected.sections);
  assertHeightmap("WORLD_SURFACE", actual.heightmaps.WORLD_SURFACE, expected.heightmaps.WORLD_SURFACE);
  assertHeightmap("OCEAN_FLOOR", actual.heightmaps.OCEAN_FLOOR, expected.heightmaps.OCEAN_FLOOR);
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
    expect(terrainOracle.palette).toEqual([...TERRAIN_STAGE_BLOCK_NAMES]);
    expect(terrainOracle.height).toBe(256);
    expect(terrainOracle.minY).toBe(0);
  });

  test("fillFromNoise keeps bedrock out of the terrain stage until buildSurfaceAndBedrock runs", () => {
    const chunk = fixture.chunks[0]!;
    const biomeSource = new OverworldBiomeSource(BigInt(fixture.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(fixture.seed));
    const generated = generator.fillFromNoise(chunk.chunkX, chunk.chunkZ);

    expect(generated).toBeInstanceOf(MutableChunkBlockBuffer);
    expect(generated.blocks.includes(ChunkBlockId.BEDROCK)).toBe(false);

    generator.buildSurfaceAndBedrock(generated);

    expect(generated.blocks.includes(ChunkBlockId.BEDROCK)).toBe(true);
  });

  test("fills chunk (0, 0) with the terrain-only Java oracle once the old bottom-bedrock overlay is stripped", () => {
    const chunk = fixture.chunks[0]!;
    const biomeSource = new OverworldBiomeSource(BigInt(fixture.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(fixture.seed));
    const actualChunk = generator.fillFromNoise(chunk.chunkX, chunk.chunkZ);

    const actual = buildTerrainChunk(actualChunk);
    const expected = buildTerrainChunk(
      new MutableChunkBlockBuffer(
        terrainOracle.chunkX,
        terrainOracle.chunkZ,
        terrainOracle.minY,
        terrainOracle.height,
        chunk.biomes,
        terrainStageBlocksFromTerrainOracle(terrainOracle),
      ),
    );

    expect(actual.biomes).toEqual(expected.biomes);
    assertTerrainSections(actual.sections, expected.sections);
    assertHeightmap("WORLD_SURFACE", actual.heightmaps.WORLD_SURFACE, expected.heightmaps.WORLD_SURFACE);
    assertHeightmap("OCEAN_FLOOR", actual.heightmaps.OCEAN_FLOOR, expected.heightmaps.OCEAN_FLOOR);

    for (const section of actual.sections) {
      for (const blockName of section.palette) {
        expect(CHUNK_BLOCK_NAMES).toContain(blockName);
      }
    }
  });

  test("matches the pinned surface-only oracle for chunk (0, 0)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(surfaceOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(surfaceOracle.seed));
    const actualChunk = generator.fillFromNoise(surfaceOracle.chunkX, surfaceOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);

    assertChunkParity(actualChunk, surfaceOracle);
  });

  test("matches the narrow sand-and-gravel surface oracle for chunk (5, 115)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(sandSurfaceOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(sandSurfaceOracle.seed));
    const actualChunk = generator.fillFromNoise(sandSurfaceOracle.chunkX, sandSurfaceOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);

    assertChunkParity(actualChunk, sandSurfaceOracle);
  });

  test("matches the pinned carved-only oracle for chunk (0, 0)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(carvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(carvedOracle.seed));
    const actualChunk = generator.fillFromNoise(carvedOracle.chunkX, carvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk);

    assertChunkParity(actualChunk, carvedOracle);
  });
});
