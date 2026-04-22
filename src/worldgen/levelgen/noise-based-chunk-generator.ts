import { ChunkBiomeContainer } from "../biome/chunk-biome-container.ts";
import type { NoiseBiomeSource } from "../biome/noise-biome-source.ts";
import { PerlinNoise } from "../noise/perlin-noise.ts";
import { PerlinSimplexNoise } from "../noise/perlin-simplex-noise.ts";
import { BlendedNoise } from "../noise/blended-noise.ts";
import { SimplexNoise } from "../noise/simplex-noise.ts";
import type { LongSeed } from "../prng/simple-random-source.ts";
import { WorldgenRandom } from "../prng/worldgen-random.ts";
import { NoiseModifier } from "./noise-modifier.ts";
import { NoiseGeneratorSettings } from "./noise-generator-settings.ts";
import { NoiseSampler } from "./noise-sampler.ts";

const CHUNK_WIDTH = 16;
const SECTION_HEIGHT = 16;
const BLOCKS_PER_SECTION = CHUNK_WIDTH * CHUNK_WIDTH * SECTION_HEIGHT;
const SURFACE_NOISE_OCTAVES = [-3, -2, -1, 0] as const;
const DEPTH_NOISE_OCTAVES = [-15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0] as const;

export const TerrainBlockId = {
  AIR: 0,
  STONE: 1,
  WATER: 2,
  BEDROCK: 3,
} as const;

export type TerrainBlockId = (typeof TerrainBlockId)[keyof typeof TerrainBlockId];

export const TERRAIN_BLOCK_NAMES = [
  "minecraft:air",
  "minecraft:stone",
  "minecraft:water",
  "minecraft:bedrock",
] as const;

export type TerrainBlockName = (typeof TERRAIN_BLOCK_NAMES)[number];

export interface TerrainChunkSection {
  readonly y: number;
  readonly palette: readonly TerrainBlockName[];
  readonly blockOrder: "y-major,z-major,x-minor";
  readonly blocks: readonly number[];
}

export interface TerrainChunk {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly sections: readonly TerrainChunkSection[];
  readonly heightmaps: {
    readonly OCEAN_FLOOR: readonly number[];
    readonly WORLD_SURFACE: readonly number[];
  };
  readonly biomes: readonly number[];
}

function normalizeLongSeed(seed: LongSeed): bigint {
  if (typeof seed === "bigint") {
    return BigInt.asIntN(64, seed);
  }

  if (typeof seed === "number") {
    if (!Number.isSafeInteger(seed)) {
      throw new RangeError("number seeds must be safe integers; use bigint for full 64-bit seeds");
    }

    return BigInt(seed);
  }

  return BigInt.asIntN(64, BigInt(seed));
}

function clamp(value: number, minValue: number, maxValue: number): number {
  return Math.min(Math.max(value, minValue), maxValue);
}

function lerp(delta: number, start: number, end: number): number {
  return start + (delta * (end - start));
}

function lerp2(
  deltaX: number,
  deltaY: number,
  x0y0: number,
  x1y0: number,
  x0y1: number,
  x1y1: number,
): number {
  return lerp(deltaY, lerp(deltaX, x0y0, x1y0), lerp(deltaX, x0y1, x1y1));
}

function lerp3(
  deltaX: number,
  deltaY: number,
  deltaZ: number,
  x0y0z0: number,
  x1y0z0: number,
  x0y1z0: number,
  x1y1z0: number,
  x0y0z1: number,
  x1y0z1: number,
  x0y1z1: number,
  x1y1z1: number,
): number {
  return lerp(
    deltaZ,
    lerp2(deltaX, deltaY, x0y0z0, x1y0z0, x0y1z0, x1y1z0),
    lerp2(deltaX, deltaY, x0y0z1, x1y0z1, x0y1z1, x1y1z1),
  );
}

function isMotionBlocking(blockId: TerrainBlockId): boolean {
  return blockId === TerrainBlockId.STONE || blockId === TerrainBlockId.BEDROCK;
}

function blockBufferIndex(localX: number, localY: number, localZ: number): number {
  return (localY << 8) | (localZ << 4) | localX;
}

export function terrainBlockNameForId(blockId: TerrainBlockId): TerrainBlockName {
  return TERRAIN_BLOCK_NAMES[blockId]!;
}

export function buildTerrainSections(blocks: Uint8Array, minY: number, height: number): TerrainChunkSection[] {
  const expectedLength = height * CHUNK_WIDTH * CHUNK_WIDTH;
  if (blocks.length !== expectedLength) {
    throw new RangeError(`terrain block buffer length ${blocks.length} did not match expected ${expectedLength}`);
  }

  const sectionCount = height / SECTION_HEIGHT;
  const minSectionY = Math.floor(minY / SECTION_HEIGHT);
  const sections: TerrainChunkSection[] = [];

  for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
    const start = sectionOffset * BLOCKS_PER_SECTION;
    let usedMask = 0;
    for (let index = 0; index < BLOCKS_PER_SECTION; index++) {
      usedMask |= 1 << blocks[start + index]!;
    }

    if (usedMask === (1 << TerrainBlockId.AIR)) {
      continue;
    }

    const paletteIds: TerrainBlockId[] = [];
    const paletteIndexByBlockId = new Int8Array(TERRAIN_BLOCK_NAMES.length).fill(-1);
    for (let blockId = 0; blockId < TERRAIN_BLOCK_NAMES.length; blockId++) {
      if ((usedMask & (1 << blockId)) !== 0) {
        paletteIndexByBlockId[blockId] = paletteIds.length;
        paletteIds.push(blockId as TerrainBlockId);
      }
    }

    const paletteIndices = new Array<number>(BLOCKS_PER_SECTION);
    for (let index = 0; index < BLOCKS_PER_SECTION; index++) {
      paletteIndices[index] = paletteIndexByBlockId[blocks[start + index]!]!;
    }

    sections.push({
      y: minSectionY + sectionOffset,
      palette: paletteIds.map(terrainBlockNameForId),
      blockOrder: "y-major,z-major,x-minor",
      blocks: paletteIndices,
    });
  }

  return sections;
}

export function buildTerrainHeightmaps(blocks: Uint8Array, minY: number, height: number): TerrainChunk["heightmaps"] {
  const expectedLength = height * CHUNK_WIDTH * CHUNK_WIDTH;
  if (blocks.length !== expectedLength) {
    throw new RangeError(`terrain block buffer length ${blocks.length} did not match expected ${expectedLength}`);
  }

  const oceanFloor = new Array<number>(CHUNK_WIDTH * CHUNK_WIDTH).fill(minY);
  const worldSurface = new Array<number>(CHUNK_WIDTH * CHUNK_WIDTH).fill(minY);
  const maxY = minY + height - 1;

  for (let localZ = 0; localZ < CHUNK_WIDTH; localZ++) {
    for (let localX = 0; localX < CHUNK_WIDTH; localX++) {
      const columnIndex = (localZ << 4) | localX;
      let foundOceanFloor = false;
      let foundWorldSurface = false;

      for (let y = maxY; y >= minY; y--) {
        const blockId = blocks[blockBufferIndex(localX, y - minY, localZ)]! as TerrainBlockId;
        if (!foundWorldSurface && blockId !== TerrainBlockId.AIR) {
          worldSurface[columnIndex] = y + 1;
          foundWorldSurface = true;
        }

        if (!foundOceanFloor && isMotionBlocking(blockId)) {
          oceanFloor[columnIndex] = y + 1;
          foundOceanFloor = true;
        }

        if (foundWorldSurface && foundOceanFloor) {
          break;
        }
      }
    }
  }

  return {
    OCEAN_FLOOR: oceanFloor,
    WORLD_SURFACE: worldSurface,
  };
}

export function buildTerrainChunk(
  chunkX: number,
  chunkZ: number,
  minY: number,
  height: number,
  blocks: Uint8Array,
  biomes: readonly number[],
): TerrainChunk {
  return {
    chunkX,
    chunkZ,
    sections: buildTerrainSections(blocks, minY, height),
    heightmaps: buildTerrainHeightmaps(blocks, minY, height),
    biomes: [...biomes],
  };
}

export class NoiseBasedChunkGenerator {
  private readonly seed: bigint;
  private readonly cellHeight: number;
  private readonly cellWidth: number;
  private readonly cellCountX: number;
  private readonly cellCountY: number;
  private readonly cellCountZ: number;
  private readonly minY: number;
  private readonly height: number;
  private readonly minCellY: number;
  private readonly seaLevel: number;
  private readonly sampler: NoiseSampler;

  public constructor(
    private readonly biomeSource: NoiseBiomeSource,
    seed: LongSeed,
    private readonly settings = NoiseGeneratorSettings.overworld(),
  ) {
    if (
      settings.isAquifersEnabled() ||
      settings.isNoiseCavesEnabled() ||
      settings.isDeepslateEnabled() ||
      settings.isOreVeinsEnabled() ||
      settings.isNoodleCavesEnabled()
    ) {
      throw new RangeError("This terrain-only generator only supports the default 1.17.1 overworld flags");
    }

    this.seed = normalizeLongSeed(seed);
    const noiseSettings = settings.noiseSettings();
    this.minY = noiseSettings.minY();
    this.height = noiseSettings.height();
    this.cellHeight = noiseSettings.noiseSizeVertical() * 4;
    this.cellWidth = noiseSettings.noiseSizeHorizontal() * 4;
    this.cellCountX = CHUNK_WIDTH / this.cellWidth;
    this.cellCountY = this.height / this.cellHeight;
    this.cellCountZ = CHUNK_WIDTH / this.cellWidth;
    this.minCellY = Math.floor(this.minY / this.cellHeight);
    this.seaLevel = settings.seaLevel();

    const random = new WorldgenRandom(this.seed);
    const blendedNoise = new BlendedNoise(random);
    const surfaceNoise = noiseSettings.useSimplexSurfaceNoise()
      ? new PerlinSimplexNoise(random, SURFACE_NOISE_OCTAVES)
      : new PerlinNoise(random, SURFACE_NOISE_OCTAVES);
    void surfaceNoise;
    random.consumeCount(2620);
    const depthNoise = new PerlinNoise(random, DEPTH_NOISE_OCTAVES);

    let islandNoise: SimplexNoise | undefined;
    if (noiseSettings.islandNoiseOverride()) {
      const islandRandom = new WorldgenRandom(this.seed);
      islandRandom.consumeCount(17292);
      islandNoise = new SimplexNoise(islandRandom);
    }

    this.sampler = new NoiseSampler(
      biomeSource,
      this.cellWidth,
      this.cellHeight,
      this.cellCountY,
      noiseSettings,
      blendedNoise,
      islandNoise,
      depthNoise,
      NoiseModifier.PASSTHROUGH,
    );
  }

  public fillFromNoise(chunkX: number, chunkZ: number): TerrainChunk {
    const blocks = this.fillTerrainBlockBuffer(chunkX, chunkZ);
    this.setBedrock(blocks, chunkX, chunkZ);
    const biomes = new ChunkBiomeContainer(this.minY, this.height, chunkX, chunkZ, this.biomeSource).writeBiomes();
    return buildTerrainChunk(chunkX, chunkZ, this.minY, this.height, blocks, biomes);
  }

  private fillTerrainBlockBuffer(chunkX: number, chunkZ: number): Uint8Array {
    const noiseColumns = this.createNoiseColumns(chunkX, chunkZ);
    const blocks = new Uint8Array(this.height * CHUNK_WIDTH * CHUNK_WIDTH);

    for (let cellX = 0; cellX < this.cellCountX; cellX++) {
      for (let cellZ = 0; cellZ < this.cellCountZ; cellZ++) {
        for (let cellY = this.cellCountY - 1; cellY >= 0; cellY--) {
          const x0z0y0 = noiseColumns[cellX]![cellZ]![cellY]!;
          const x0z0y1 = noiseColumns[cellX]![cellZ]![cellY + 1]!;
          const x1z0y0 = noiseColumns[cellX + 1]![cellZ]![cellY]!;
          const x1z0y1 = noiseColumns[cellX + 1]![cellZ]![cellY + 1]!;
          const x0z1y0 = noiseColumns[cellX]![cellZ + 1]![cellY]!;
          const x0z1y1 = noiseColumns[cellX]![cellZ + 1]![cellY + 1]!;
          const x1z1y0 = noiseColumns[cellX + 1]![cellZ + 1]![cellY]!;
          const x1z1y1 = noiseColumns[cellX + 1]![cellZ + 1]![cellY + 1]!;

          for (let yOffset = this.cellHeight - 1; yOffset >= 0; yOffset--) {
            const yFraction = yOffset / this.cellHeight;
            const blockY = ((this.minCellY + cellY) * this.cellHeight) + yOffset;
            const localY = blockY - this.minY;

            for (let xOffset = 0; xOffset < this.cellWidth; xOffset++) {
              const xFraction = xOffset / this.cellWidth;
              const localX = (cellX * this.cellWidth) + xOffset;

              for (let zOffset = 0; zOffset < this.cellWidth; zOffset++) {
                const zFraction = zOffset / this.cellWidth;
                const density = lerp3(
                  yFraction,
                  xFraction,
                  zFraction,
                  x0z0y0,
                  x0z0y1,
                  x1z0y0,
                  x1z0y1,
                  x0z1y0,
                  x0z1y1,
                  x1z1y0,
                  x1z1y1,
                );
                const blockId = this.resolveTerrainBlock(blockY, density);
                if (blockId !== TerrainBlockId.AIR) {
                  const localZ = (cellZ * this.cellWidth) + zOffset;
                  blocks[blockBufferIndex(localX, localY, localZ)] = blockId;
                }
              }
            }
          }
        }
      }
    }

    return blocks;
  }

  private createNoiseColumns(chunkX: number, chunkZ: number): number[][][] {
    const columns: number[][][] = [];
    const cellMinX = chunkX * this.cellCountX;
    const cellMinZ = chunkZ * this.cellCountZ;
    const noiseSettings = this.settings.noiseSettings();

    for (let cellX = 0; cellX <= this.cellCountX; cellX++) {
      const xColumns: number[][] = [];
      columns.push(xColumns);
      for (let cellZ = 0; cellZ <= this.cellCountZ; cellZ++) {
        const noiseValues = new Array<number>(this.cellCountY + 1).fill(0);
        this.sampler.fillNoiseColumn(
          noiseValues,
          cellMinX + cellX,
          cellMinZ + cellZ,
          noiseSettings,
          this.seaLevel,
          this.minCellY,
          this.cellCountY,
        );
        xColumns.push(noiseValues);
      }
    }

    return columns;
  }

  private resolveTerrainBlock(y: number, noise: number): TerrainBlockId {
    let density = clamp(noise / 200, -1, 1);
    density = (density / 2) - ((density * density * density) / 24);
    if (density > 0) {
      return TerrainBlockId.STONE;
    }

    return y >= this.seaLevel ? TerrainBlockId.AIR : TerrainBlockId.WATER;
  }

  private setBedrock(blocks: Uint8Array, chunkX: number, chunkZ: number): void {
    const random = new WorldgenRandom();
    random.setBaseChunkSeed(chunkX, chunkZ);

    const floorY = this.minY + this.settings.getBedrockFloorPosition();
    const roofY = (this.height - 1) + this.minY - this.settings.getBedrockRoofPosition();
    const minBuildHeight = this.minY;
    const maxBuildHeight = this.minY + this.height;
    const hasRoof = ((roofY + 4) >= minBuildHeight) && (roofY < maxBuildHeight);
    const hasFloor = ((floorY + 4) >= minBuildHeight) && (floorY < maxBuildHeight);

    if (!hasRoof && !hasFloor) {
      return;
    }

    for (let localZ = 0; localZ < CHUNK_WIDTH; localZ++) {
      for (let localX = 0; localX < CHUNK_WIDTH; localX++) {
        if (hasRoof) {
          for (let offset = 0; offset < 5; offset++) {
            if (offset <= random.nextInt(5)) {
              const localY = (roofY - offset) - this.minY;
              if (localY >= 0 && localY < this.height) {
                blocks[blockBufferIndex(localX, localY, localZ)] = TerrainBlockId.BEDROCK;
              }
            }
          }
        }

        if (hasFloor) {
          for (let offset = 4; offset >= 0; offset--) {
            if (offset <= random.nextInt(5)) {
              const localY = (floorY + offset) - this.minY;
              if (localY >= 0 && localY < this.height) {
                blocks[blockBufferIndex(localX, localY, localZ)] = TerrainBlockId.BEDROCK;
              }
            }
          }
        }
      }
    }
  }
}
