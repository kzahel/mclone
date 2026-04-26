import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import type { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { Biome } from "../biome/biome";
import { getLayeredBiomeByKey } from "../biome/biome-data";
import { ChunkBiomeContainer } from "../biome/chunk-biome-container";
import type { NoiseBiomeSource } from "../biome/noise-biome-source";
import {
  CHUNK_WIDTH,
  ChunkBlockId,
  MutableChunkBlockBuffer,
} from "../chunk/chunk-block-buffer";
import type { LongSeed } from "../prng/simple-random-source";
import { type BaseStoneSource, SingleBaseStoneSource } from "./base-stone-source";
import type { CooperativeGenerationYield } from "./cooperative-generation";
import type { BiomeDecorationProfiler } from "./decoration-profiler";
import type { GenerationStep } from "./generation-step";
import type { GenerationEntitySink, NaturalSpawnerOptions } from "./natural-spawner";
import type { WorldGenerator } from "./world-generator";

const MIN_BUILD_HEIGHT = 0;
const WORLD_HEIGHT = 256;
const DEFAULT_SEA_LEVEL = 62;
const STONE_LOCATION = new ResourceLocation("minecraft:stone");

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

function smoothstep(value: number): number {
  return value * value * (3.0 - (2.0 * value));
}

function lerp(delta: number, start: number, end: number): number {
  return start + (delta * (end - start));
}

function mix64(seed: bigint, x: number, z: number): bigint {
  let value = BigInt.asUintN(
    64,
    seed
      ^ (BigInt(x) * 0x9e3779b97f4a7c15n)
      ^ (BigInt(z) * 0xc2b2ae3d27d4eb4fn),
  );
  value = BigInt.asUintN(64, (value ^ (value >> 30n)) * 0xbf58476d1ce4e5b9n);
  value = BigInt.asUintN(64, (value ^ (value >> 27n)) * 0x94d049bb133111ebn);
  return BigInt.asUintN(64, value ^ (value >> 31n));
}

function hashUnit(seed: bigint, x: number, z: number): number {
  return Number(mix64(seed, x, z) & 0xffff_ffffn) / 0xffff_ffff;
}

function sampleInterpolatedNoise(seed: bigint, x: number, z: number, cellSize: number): number {
  const scaledX = x / cellSize;
  const scaledZ = z / cellSize;
  const minX = Math.floor(scaledX);
  const minZ = Math.floor(scaledZ);
  const fracX = smoothstep(scaledX - minX);
  const fracZ = smoothstep(scaledZ - minZ);

  const x0z0 = hashUnit(seed, minX, minZ);
  const x1z0 = hashUnit(seed, minX + 1, minZ);
  const x0z1 = hashUnit(seed, minX, minZ + 1);
  const x1z1 = hashUnit(seed, minX + 1, minZ + 1);

  return lerp(
    fracZ,
    lerp(fracX, x0z0, x1z0),
    lerp(fracX, x0z1, x1z1),
  );
}

function resolveStoneBlockState(): BlockState {
  const stone = Registry.BLOCK.get(STONE_LOCATION) as Block | undefined;
  if (stone === undefined) {
    throw new Error(`Missing registered block ${STONE_LOCATION}`);
  }

  return stone.defaultBlockState();
}

class SingleBiomeSource implements NoiseBiomeSource {
  public constructor(private readonly biome: Biome) {}

  public getNoiseBiome(_x: number, _y: number, _z: number): Biome {
    return this.biome;
  }
}

class SmallIslandBiomeSource implements NoiseBiomeSource {
  public constructor(private readonly seed: bigint) {}

  public getNoiseBiome(x: number, _y: number, z: number): Biome {
    const column = describeSmallIslandColumn(this.seed, x * 4, z * 4);
    if (column.biomeBand === "plains") {
      return getLayeredBiomeByKey("minecraft:plains");
    }
    if (column.biomeBand === "beach") {
      return getLayeredBiomeByKey("minecraft:beach");
    }
    return getLayeredBiomeByKey("minecraft:ocean");
  }
}

type SmallIslandBiomeBand = "plains" | "beach" | "ocean";

interface SmallIslandColumn {
  readonly biomeBand: SmallIslandBiomeBand;
  readonly signedDistance: number;
  readonly surfaceY: number;
  readonly oceanFloorY: number;
}

function describeSmallIslandColumn(seed: bigint, worldX: number, worldZ: number): SmallIslandColumn {
  const coastOffset =
    (((sampleInterpolatedNoise(seed ^ 0x51ed2705a8d3f19bn, worldX, worldZ, 18) * 2.0) - 1.0) * 10.0)
    + (((sampleInterpolatedNoise(seed ^ 0x3c79ac492ba7b653n, worldX, worldZ, 34) * 2.0) - 1.0) * 5.0);
  const stretchedDistance = Math.hypot(worldX * 0.92, worldZ * 1.08);
  const radius = 56.0 + coastOffset;
  const signedDistance = stretchedDistance - radius;
  const oceanFloorY =
    DEFAULT_SEA_LEVEL - 7
    + Math.floor(((sampleInterpolatedNoise(seed ^ 0x1c69b3f74ac4ae35n, worldX, worldZ, 20) * 2.0) - 1.0) * 2.0);

  if (signedDistance >= 0.0) {
    return {
      biomeBand: signedDistance < 6.0 ? "beach" : "ocean",
      signedDistance,
      surfaceY: oceanFloorY,
      oceanFloorY,
    };
  }

  const inland = -signedDistance;
  const inlandFactor = Math.min(1.0, inland / 28.0);
  const hillNoise = ((sampleInterpolatedNoise(seed ^ 0x2545f4914f6cdd1dn, worldX, worldZ, 14) * 2.0) - 1.0) * 2.5;
  const uplift = (inlandFactor * inlandFactor) * 17.0;
  const surfaceY = DEFAULT_SEA_LEVEL + 1 + Math.max(0, Math.floor(uplift + hillNoise));

  return {
    biomeBand: inland < 7.0 ? "beach" : "plains",
    signedDistance,
    surfaceY,
    oceanFloorY,
  };
}

abstract class DemoWorldGeneratorBase implements WorldGenerator {
  private baseStoneSource: BaseStoneSource | undefined;

  protected constructor(
    private readonly seed: bigint,
    private readonly biomeSource: NoiseBiomeSource,
  ) {}

  public getSeed(): bigint {
    return this.seed;
  }

  public getBiomeSource(): NoiseBiomeSource {
    return this.biomeSource;
  }

  public getBaseStoneSource(): BaseStoneSource {
    this.baseStoneSource ??= new SingleBaseStoneSource(resolveStoneBlockState());
    return this.baseStoneSource;
  }

  public buildSurfaceAndBedrock(_chunk: MutableChunkBlockBuffer): void {}

  public applyCarvers(_chunk: MutableChunkBlockBuffer, _step?: GenerationStep.Carving): void {}

  public async applyCarversCooperative(_chunk: MutableChunkBlockBuffer, _yieldStep: CooperativeGenerationYield): Promise<void> {}

  public applyBiomeDecoration(
    _level: WorldGenLevel,
    _chunkX: number,
    _chunkZ: number,
    _profiler?: BiomeDecorationProfiler,
  ): void {}

  public async applyBiomeDecorationCooperative(
    _level: WorldGenLevel,
    _chunkX: number,
    _chunkZ: number,
    _yieldStep: CooperativeGenerationYield,
    _profiler?: BiomeDecorationProfiler,
  ): Promise<void> {}

  public spawnOriginalMobs(
    _level: WorldGenLevel,
    _chunkX: number,
    _chunkZ: number,
    _sink: GenerationEntitySink,
    _options?: NaturalSpawnerOptions,
  ): void {}

  protected createChunkBuffer(chunkX: number, chunkZ: number): MutableChunkBlockBuffer {
    return new MutableChunkBlockBuffer(
      chunkX,
      chunkZ,
      MIN_BUILD_HEIGHT,
      WORLD_HEIGHT,
      new ChunkBiomeContainer(
        MIN_BUILD_HEIGHT,
        WORLD_HEIGHT,
        chunkX,
        chunkZ,
        this.biomeSource,
      ).writeBiomes(),
    );
  }

  public abstract fillFromNoise(chunkX: number, chunkZ: number): MutableChunkBlockBuffer;
}

export class FlatGrassWorldGenerator extends DemoWorldGeneratorBase {
  private static readonly TOP_Y = 63;

  public constructor(seed: LongSeed) {
    super(normalizeLongSeed(seed), new SingleBiomeSource(getLayeredBiomeByKey("minecraft:plains")));
  }

  public fillFromNoise(chunkX: number, chunkZ: number): MutableChunkBlockBuffer {
    const chunk = this.createChunkBuffer(chunkX, chunkZ);

    for (let localZ = 0; localZ < CHUNK_WIDTH; localZ++) {
      for (let localX = 0; localX < CHUNK_WIDTH; localX++) {
        chunk.setBlockAtY(localX, 0, localZ, ChunkBlockId.BEDROCK);
        for (let y = 1; y < FlatGrassWorldGenerator.TOP_Y - 3; y++) {
          chunk.setBlockAtY(localX, y, localZ, ChunkBlockId.STONE);
        }
        for (let y = FlatGrassWorldGenerator.TOP_Y - 3; y < FlatGrassWorldGenerator.TOP_Y; y++) {
          chunk.setBlockAtY(localX, y, localZ, ChunkBlockId.DIRT);
        }
        chunk.setBlockAtY(localX, FlatGrassWorldGenerator.TOP_Y, localZ, ChunkBlockId.GRASS_BLOCK);
      }
    }

    return chunk;
  }
}

export class SmallIslandWorldGenerator extends DemoWorldGeneratorBase {
  public constructor(seed: LongSeed) {
    const normalizedSeed = normalizeLongSeed(seed);
    super(normalizedSeed, new SmallIslandBiomeSource(normalizedSeed));
  }

  public fillFromNoise(chunkX: number, chunkZ: number): MutableChunkBlockBuffer {
    const chunk = this.createChunkBuffer(chunkX, chunkZ);
    const minWorldX = chunkX * CHUNK_WIDTH;
    const minWorldZ = chunkZ * CHUNK_WIDTH;

    for (let localZ = 0; localZ < CHUNK_WIDTH; localZ++) {
      for (let localX = 0; localX < CHUNK_WIDTH; localX++) {
        const worldX = minWorldX + localX;
        const worldZ = minWorldZ + localZ;
        const column = describeSmallIslandColumn(this.getSeed(), worldX, worldZ);

        chunk.setBlockAtY(localX, 0, localZ, ChunkBlockId.BEDROCK);

        if (column.signedDistance >= 0.0) {
          const floorTop = Math.max(1, column.oceanFloorY);
          for (let y = 1; y < floorTop - 1; y++) {
            chunk.setBlockAtY(localX, y, localZ, ChunkBlockId.STONE);
          }
          chunk.setBlockAtY(localX, Math.max(1, floorTop - 1), localZ, ChunkBlockId.SAND);
          chunk.setBlockAtY(localX, floorTop, localZ, ChunkBlockId.SAND);
          for (let y = floorTop + 1; y <= DEFAULT_SEA_LEVEL; y++) {
            chunk.setBlockAtY(localX, y, localZ, ChunkBlockId.WATER);
          }
          continue;
        }

        const topY = Math.max(1, column.surfaceY);
        const fillerDepth = column.biomeBand === "beach" ? 3 : 4;
        const stoneTopY = Math.max(1, topY - fillerDepth);
        const fillerBlock = column.biomeBand === "beach" ? ChunkBlockId.SAND : ChunkBlockId.DIRT;
        const surfaceBlock = column.biomeBand === "beach" ? ChunkBlockId.SAND : ChunkBlockId.GRASS_BLOCK;

        for (let y = 1; y < stoneTopY; y++) {
          chunk.setBlockAtY(localX, y, localZ, ChunkBlockId.STONE);
        }
        for (let y = stoneTopY; y < topY; y++) {
          chunk.setBlockAtY(localX, y, localZ, fillerBlock);
        }
        chunk.setBlockAtY(localX, topY, localZ, surfaceBlock);

        for (let y = topY + 1; y <= DEFAULT_SEA_LEVEL; y++) {
          chunk.setBlockAtY(localX, y, localZ, ChunkBlockId.WATER);
        }
      }
    }

    return chunk;
  }
}
