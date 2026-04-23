import { describe, expect, test } from "vitest";
import integrationFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0.json";
import terrainFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json";
import surfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json";
import carvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0-carved-only.json";
import oceanCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-117--128-carved-only.json";
import liquidOceanCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-117--128-liquid-carved.json";
import liquidFloorCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks--129--256-liquid-carved.json";
import sandSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-5-115-surface-only.json";
import desertSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-96--64-surface-only.json";
import frozenSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks--247--247-surface-only.json";
import badlandsSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks--320-99-surface-only.json";
import giantTaigaSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks--9-68-surface-only.json";
import shatteredSavannaSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks-60-199-surface-only.json";
import mushroomSurfaceFixture from "../../fixtures/integration/overworld-seed-12345-chunks--446-387-surface-only.json";
import desertCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-96--64-carved-only.json";
import badlandsCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks--320-99-carved-only.json";
import giantTaigaCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks--9-68-carved-only.json";
import shatteredSavannaCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks-60-199-carved-only.json";
import mushroomCarvedFixture from "../../fixtures/integration/overworld-seed-12345-chunks--446-387-carved-only.json";
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
import { GenerationStep } from "../../../src/worldgen/levelgen/generation-step.ts";
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
  readonly blockTicks?: readonly {
    readonly x: number;
    readonly y: number;
    readonly z: number;
    readonly target: string;
    readonly delay: number;
  }[];
  readonly liquidTicks?: readonly {
    readonly x: number;
    readonly y: number;
    readonly z: number;
    readonly target: string;
    readonly delay: number;
  }[];
}

interface ScheduledTickFixture {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly target: string;
  readonly delay: number;
}

interface SurfaceChunkOracleFixture extends TerrainChunkOracleFixture {}
interface CarvedChunkOracleFixture extends TerrainChunkOracleFixture {}

const fixture = integrationFixture as IntegrationFixture;
const terrainOracle = terrainFixture as TerrainChunkOracleFixture;
const surfaceOracle = surfaceFixture as SurfaceChunkOracleFixture;
const carvedOracle = carvedFixture as CarvedChunkOracleFixture;
const oceanCarvedOracle = oceanCarvedFixture as CarvedChunkOracleFixture;
const liquidOceanCarvedOracle = liquidOceanCarvedFixture as CarvedChunkOracleFixture;
const liquidFloorCarvedOracle = liquidFloorCarvedFixture as CarvedChunkOracleFixture;
const sandSurfaceOracle = sandSurfaceFixture as SurfaceChunkOracleFixture;
const desertSurfaceOracle = desertSurfaceFixture as SurfaceChunkOracleFixture;
const frozenSurfaceOracle = frozenSurfaceFixture as SurfaceChunkOracleFixture;
const badlandsSurfaceOracle = badlandsSurfaceFixture as SurfaceChunkOracleFixture;
const giantTaigaSurfaceOracle = giantTaigaSurfaceFixture as SurfaceChunkOracleFixture;
const shatteredSavannaSurfaceOracle = shatteredSavannaSurfaceFixture as SurfaceChunkOracleFixture;
const mushroomSurfaceOracle = mushroomSurfaceFixture as SurfaceChunkOracleFixture;
const desertCarvedOracle = desertCarvedFixture as CarvedChunkOracleFixture;
const badlandsCarvedOracle = badlandsCarvedFixture as CarvedChunkOracleFixture;
const giantTaigaCarvedOracle = giantTaigaCarvedFixture as CarvedChunkOracleFixture;
const shatteredSavannaCarvedOracle = shatteredSavannaCarvedFixture as CarvedChunkOracleFixture;
const mushroomCarvedOracle = mushroomCarvedFixture as CarvedChunkOracleFixture;

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

function sortScheduledTicks(ticks: readonly ScheduledTickFixture[]): readonly ScheduledTickFixture[] {
  return [...ticks].sort(
    (left, right) =>
      left.x - right.x ||
      left.z - right.z ||
      left.y - right.y ||
      left.target.localeCompare(right.target) ||
      left.delay - right.delay,
  );
}

function assertScheduledTickParity(actualChunk: MutableChunkBlockBuffer, oracle: TerrainChunkOracleFixture): void {
  expect(sortScheduledTicks(actualChunk.getScheduledBlockTicks())).toEqual(sortScheduledTicks(oracle.blockTicks ?? []));
  expect(sortScheduledTicks(actualChunk.getScheduledLiquidTicks())).toEqual(sortScheduledTicks(oracle.liquidTicks ?? []));
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

  test("matches the widened sandstone surface oracle for chunk (96, -64)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(desertSurfaceOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(desertSurfaceOracle.seed));
    const actualChunk = generator.fillFromNoise(desertSurfaceOracle.chunkX, desertSurfaceOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);

    assertChunkParity(actualChunk, desertSurfaceOracle);
  });

  test("matches the widened frozen-ocean surface oracle for chunk (-247, -247)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(frozenSurfaceOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(frozenSurfaceOracle.seed));
    const actualChunk = generator.fillFromNoise(frozenSurfaceOracle.chunkX, frozenSurfaceOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);

    assertChunkParity(actualChunk, frozenSurfaceOracle);
  });

  test("matches the widened badlands surface oracle for chunk (-320, 99)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(badlandsSurfaceOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(badlandsSurfaceOracle.seed));
    const actualChunk = generator.fillFromNoise(badlandsSurfaceOracle.chunkX, badlandsSurfaceOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);

    assertChunkParity(actualChunk, badlandsSurfaceOracle);
  });

  test("matches the widened giant-tree taiga surface oracle for chunk (-9, 68)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(giantTaigaSurfaceOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(giantTaigaSurfaceOracle.seed));
    const actualChunk = generator.fillFromNoise(giantTaigaSurfaceOracle.chunkX, giantTaigaSurfaceOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);

    assertChunkParity(actualChunk, giantTaigaSurfaceOracle);
  });

  test("matches the widened shattered-savanna surface oracle for chunk (60, 199)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(shatteredSavannaSurfaceOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(shatteredSavannaSurfaceOracle.seed));
    const actualChunk = generator.fillFromNoise(shatteredSavannaSurfaceOracle.chunkX, shatteredSavannaSurfaceOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);

    assertChunkParity(actualChunk, shatteredSavannaSurfaceOracle);
  });

  test("matches the widened mushroom-fields surface oracle for chunk (-446, 387)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(mushroomSurfaceOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(mushroomSurfaceOracle.seed));
    const actualChunk = generator.fillFromNoise(mushroomSurfaceOracle.chunkX, mushroomSurfaceOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);

    assertChunkParity(actualChunk, mushroomSurfaceOracle);
  });

  test("matches the pinned carved-only oracle for chunk (0, 0)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(carvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(carvedOracle.seed));
    const actualChunk = generator.fillFromNoise(carvedOracle.chunkX, carvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk, GenerationStep.Carving.AIR);

    assertChunkParity(actualChunk, carvedOracle);
    assertScheduledTickParity(actualChunk, carvedOracle);
  });

  test("matches the pinned carved-only ocean oracle for chunk (117, -128)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(oceanCarvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(oceanCarvedOracle.seed));
    const actualChunk = generator.fillFromNoise(oceanCarvedOracle.chunkX, oceanCarvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk, GenerationStep.Carving.AIR);

    assertChunkParity(actualChunk, oceanCarvedOracle);
    assertScheduledTickParity(actualChunk, oceanCarvedOracle);
  });

  test("matches the pinned carved-only desert oracle for chunk (96, -64)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(desertCarvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(desertCarvedOracle.seed));
    const actualChunk = generator.fillFromNoise(desertCarvedOracle.chunkX, desertCarvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk, GenerationStep.Carving.AIR);

    assertChunkParity(actualChunk, desertCarvedOracle);
    assertScheduledTickParity(actualChunk, desertCarvedOracle);
  });

  test("matches the pinned carved-only badlands oracle for chunk (-320, 99)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(badlandsCarvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(badlandsCarvedOracle.seed));
    const actualChunk = generator.fillFromNoise(badlandsCarvedOracle.chunkX, badlandsCarvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk, GenerationStep.Carving.AIR);

    assertChunkParity(actualChunk, badlandsCarvedOracle);
    assertScheduledTickParity(actualChunk, badlandsCarvedOracle);
  });

  test("matches the pinned carved-only giant-tree taiga oracle for chunk (-9, 68)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(giantTaigaCarvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(giantTaigaCarvedOracle.seed));
    const actualChunk = generator.fillFromNoise(giantTaigaCarvedOracle.chunkX, giantTaigaCarvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk, GenerationStep.Carving.AIR);

    assertChunkParity(actualChunk, giantTaigaCarvedOracle);
    assertScheduledTickParity(actualChunk, giantTaigaCarvedOracle);
  });

  test("matches the pinned carved-only shattered-savanna oracle for chunk (60, 199)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(shatteredSavannaCarvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(shatteredSavannaCarvedOracle.seed));
    const actualChunk = generator.fillFromNoise(shatteredSavannaCarvedOracle.chunkX, shatteredSavannaCarvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk, GenerationStep.Carving.AIR);

    assertChunkParity(actualChunk, shatteredSavannaCarvedOracle);
    assertScheduledTickParity(actualChunk, shatteredSavannaCarvedOracle);
  });

  test("matches the pinned carved-only mushroom-fields oracle for chunk (-446, 387)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(mushroomCarvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(mushroomCarvedOracle.seed));
    const actualChunk = generator.fillFromNoise(mushroomCarvedOracle.chunkX, mushroomCarvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk, GenerationStep.Carving.AIR);

    assertChunkParity(actualChunk, mushroomCarvedOracle);
    assertScheduledTickParity(actualChunk, mushroomCarvedOracle);
  });

  test("matches the pinned air-plus-liquid carved ocean oracle for chunk (117, -128)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(liquidOceanCarvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(liquidOceanCarvedOracle.seed));
    const actualChunk = generator.fillFromNoise(liquidOceanCarvedOracle.chunkX, liquidOceanCarvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk);

    assertChunkParity(actualChunk, liquidOceanCarvedOracle);
    assertScheduledTickParity(actualChunk, liquidOceanCarvedOracle);
  });

  test("matches the pinned air-plus-liquid carved underwater-floor oracle for chunk (-129, -256)", () => {
    const biomeSource = new OverworldBiomeSource(BigInt(liquidFloorCarvedOracle.seed));
    const generator = new NoiseBasedChunkGenerator(biomeSource, BigInt(liquidFloorCarvedOracle.seed));
    const actualChunk = generator.fillFromNoise(liquidFloorCarvedOracle.chunkX, liquidFloorCarvedOracle.chunkZ);
    generator.buildSurfaceAndBedrock(actualChunk);
    generator.applyCarvers(actualChunk);

    assertChunkParity(actualChunk, liquidFloorCarvedOracle);
    assertScheduledTickParity(actualChunk, liquidFloorCarvedOracle);
  });
});
