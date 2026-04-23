import { BlockPos } from "../../core/block-pos.ts";
import { Biome } from "../biome/biome.ts";
import { CHUNK_WIDTH, ChunkBlockId, type ChunkBlockId as BlockId, MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";
import { PerlinSimplexNoise } from "../noise/perlin-simplex-noise.ts";
import { WorldgenRandom } from "../prng/worldgen-random.ts";

interface SurfaceBuilderConfiguration {
  readonly topMaterial: BlockId;
  readonly underMaterial: BlockId;
  readonly underwaterMaterial: BlockId;
}

type SurfaceBuilderKind =
  | "default"
  | "mountain"
  | "gravelly_mountain"
  | "swamp"
  | "giant_tree_taiga"
  | "shattered_savanna"
  | "badlands"
  | "wooded_badlands"
  | "eroded_badlands"
  | "frozen_ocean";

interface SurfaceBiomeDefinition {
  readonly builder: SurfaceBuilderKind;
  readonly config: SurfaceBuilderConfiguration;
}

const DEFAULT_BLOCK = ChunkBlockId.STONE;
const DEFAULT_FLUID = ChunkBlockId.WATER;

const CONFIG_GRASS: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.GRASS_BLOCK,
  underMaterial: ChunkBlockId.DIRT,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const CONFIG_STONE: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.STONE,
  underMaterial: ChunkBlockId.STONE,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const CONFIG_GRAVEL: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.GRAVEL,
  underMaterial: ChunkBlockId.GRAVEL,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const CONFIG_DESERT: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.SAND,
  underMaterial: ChunkBlockId.SAND,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const CONFIG_OCEAN_SAND: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.GRASS_BLOCK,
  underMaterial: ChunkBlockId.DIRT,
  underwaterMaterial: ChunkBlockId.SAND,
};

const CONFIG_FULL_SAND: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.SAND,
  underMaterial: ChunkBlockId.SAND,
  underwaterMaterial: ChunkBlockId.SAND,
};

const CONFIG_BADLANDS: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.RED_SAND,
  underMaterial: ChunkBlockId.WHITE_TERRACOTTA,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const CONFIG_COARSE_DIRT: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.COARSE_DIRT,
  underMaterial: ChunkBlockId.DIRT,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const CONFIG_PODZOL: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.PODZOL,
  underMaterial: ChunkBlockId.DIRT,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const CONFIG_MYCELIUM: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.MYCELIUM,
  underMaterial: ChunkBlockId.DIRT,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const CONFIG_ICE_SPIKES: SurfaceBuilderConfiguration = {
  topMaterial: ChunkBlockId.SNOW_BLOCK,
  underMaterial: ChunkBlockId.DIRT,
  underwaterMaterial: ChunkBlockId.GRAVEL,
};

const BADLANDS_BAND_LENGTH = 64;
const BADLANDS_PILLAR_OCTAVES = [-3, -2, -1, 0] as const;
const BADLANDS_BAND_OFFSET_OCTAVES = [0] as const;
const FROZEN_OCEAN_ICEBERG_OCTAVES = [-3, -2, -1, 0] as const;
const FROZEN_OCEAN_ICEBERG_ROOF_OCTAVES = [0] as const;

interface BadlandsNoiseState {
  readonly clayBands: readonly BlockId[];
  readonly pillarNoise: PerlinSimplexNoise;
  readonly pillarRoofNoise: PerlinSimplexNoise;
  readonly clayBandsOffsetNoise: PerlinSimplexNoise;
}

interface FrozenOceanNoiseState {
  readonly icebergNoise: PerlinSimplexNoise;
  readonly icebergRoofNoise: PerlinSimplexNoise;
}

const BADLANDS_NOISE_BY_SEED = new Map<bigint, BadlandsNoiseState>();
const FROZEN_OCEAN_NOISE_BY_SEED = new Map<bigint, FrozenOceanNoiseState>();

function localCoord(worldCoord: number): number {
  return worldCoord & (CHUNK_WIDTH - 1);
}

function isInsideChunkY(chunk: MutableChunkBlockBuffer, y: number): boolean {
  return y >= chunk.minY && y < chunk.minY + chunk.height;
}

function getBlockAtYOrAir(chunk: MutableChunkBlockBuffer, localX: number, y: number, localZ: number): BlockId {
  if (!isInsideChunkY(chunk, y)) {
    return ChunkBlockId.AIR;
  }

  return chunk.getBlockAtY(localX, y, localZ);
}

function setBlockAtYIfInside(chunk: MutableChunkBlockBuffer, localX: number, y: number, localZ: number, blockId: BlockId): void {
  if (isInsideChunkY(chunk, y)) {
    chunk.setBlockAtY(localX, y, localZ, blockId);
  }
}

function resolveSurfaceBiomeDefinition(biome: Biome): SurfaceBiomeDefinition {
  switch (biome.getKey()) {
    case "minecraft:mountains":
    case "minecraft:mountain_edge":
    case "minecraft:wooded_mountains":
      return {
        builder: "mountain",
        config: CONFIG_GRASS,
      };
    case "minecraft:gravelly_mountains":
    case "minecraft:modified_gravelly_mountains":
      return {
        builder: "gravelly_mountain",
        config: CONFIG_GRASS,
      };
    case "minecraft:desert":
    case "minecraft:desert_hills":
    case "minecraft:desert_lakes":
    case "minecraft:beach":
    case "minecraft:snowy_beach":
      return {
        builder: "default",
        config: CONFIG_DESERT,
      };
    case "minecraft:ocean":
    case "minecraft:deep_ocean":
    case "minecraft:cold_ocean":
    case "minecraft:deep_cold_ocean":
      return {
        builder: "default",
        config: CONFIG_GRASS,
      };
    case "minecraft:lukewarm_ocean":
    case "minecraft:deep_lukewarm_ocean":
      return {
        builder: "default",
        config: CONFIG_OCEAN_SAND,
      };
    case "minecraft:warm_ocean":
    case "minecraft:deep_warm_ocean":
      return {
        builder: "default",
        config: CONFIG_FULL_SAND,
      };
    case "minecraft:swamp":
    case "minecraft:swamp_hills":
      return {
        builder: "swamp",
        config: CONFIG_GRASS,
      };
    case "minecraft:frozen_ocean":
    case "minecraft:deep_frozen_ocean":
      return {
        builder: "frozen_ocean",
        config: CONFIG_GRASS,
      };
    case "minecraft:badlands":
    case "minecraft:badlands_plateau":
    case "minecraft:modified_badlands_plateau":
      return {
        builder: "badlands",
        config: CONFIG_BADLANDS,
      };
    case "minecraft:wooded_badlands_plateau":
    case "minecraft:modified_wooded_badlands_plateau":
      return {
        builder: "wooded_badlands",
        config: CONFIG_BADLANDS,
      };
    case "minecraft:eroded_badlands":
      return {
        builder: "eroded_badlands",
        config: CONFIG_BADLANDS,
      };
    case "minecraft:giant_tree_taiga":
    case "minecraft:giant_tree_taiga_hills":
    case "minecraft:giant_spruce_taiga":
    case "minecraft:giant_spruce_taiga_hills":
      return {
        builder: "giant_tree_taiga",
        config: CONFIG_GRASS,
      };
    case "minecraft:shattered_savanna":
    case "minecraft:shattered_savanna_plateau":
      return {
        builder: "shattered_savanna",
        config: CONFIG_GRASS,
      };
    case "minecraft:mushroom_fields":
    case "minecraft:mushroom_field_shore":
      return {
        builder: "default",
        config: CONFIG_MYCELIUM,
      };
    case "minecraft:ice_spikes":
      return {
        builder: "default",
        config: CONFIG_ICE_SPIKES,
      };
    case "minecraft:stone_shore":
      return {
        builder: "default",
        config: CONFIG_STONE,
      };
    case "minecraft:taiga":
    case "minecraft:taiga_hills":
    case "minecraft:taiga_mountains":
      return {
        builder: "default",
        config: CONFIG_GRASS,
      };
    default:
      return {
        builder: "default",
        config: CONFIG_GRASS,
      };
  }
}

export function getOverworldSurfaceTopMaterial(biome: Biome): BlockId {
  return resolveSurfaceBiomeDefinition(biome).config.topMaterial;
}

function applyDefaultSurface(
  random: WorldgenRandom,
  chunk: MutableChunkBlockBuffer,
  biome: Biome,
  worldX: number,
  worldZ: number,
  height: number,
  noise: number,
  seaLevel: number,
  minSurfaceLevel: number,
  config: SurfaceBuilderConfiguration,
): void {
  const localX = localCoord(worldX);
  const localZ = localCoord(worldZ);
  const surfaceDepth = Math.trunc((noise / 3.0) + 3.0 + (random.nextDouble() * 0.25));

  if (surfaceDepth === 0) {
    let foundDefaultSurface = false;

    for (let y = height; y >= minSurfaceLevel; y--) {
      const blockId = getBlockAtYOrAir(chunk, localX, y, localZ);
      if (blockId === ChunkBlockId.AIR) {
        foundDefaultSurface = false;
      } else if (blockId === DEFAULT_BLOCK) {
        if (!foundDefaultSurface) {
          let replacement: BlockId = DEFAULT_BLOCK;
          if (y >= seaLevel) {
            replacement = ChunkBlockId.AIR;
          } else if (y === seaLevel - 1) {
            replacement = biome.getTemperature(new BlockPos(worldX, y, worldZ)) < 0.15 ? ChunkBlockId.ICE : DEFAULT_FLUID;
          } else if (y < seaLevel - (7 + surfaceDepth)) {
            replacement = config.underwaterMaterial;
          }

          setBlockAtYIfInside(chunk, localX, y, localZ, replacement);
        }

        foundDefaultSurface = true;
      }
    }

    return;
  }

  let underMaterial = config.underMaterial;
  let remainingDepth = -1;

  for (let y = height; y >= minSurfaceLevel; y--) {
    const blockId = getBlockAtYOrAir(chunk, localX, y, localZ);
    if (blockId === ChunkBlockId.AIR) {
      remainingDepth = -1;
      continue;
    }

    if (blockId !== DEFAULT_BLOCK) {
      continue;
    }

    if (remainingDepth === -1) {
      remainingDepth = surfaceDepth;
      let topMaterial: BlockId;

      if (y >= seaLevel + 2) {
        topMaterial = config.topMaterial;
      } else if (y >= seaLevel - 1) {
        underMaterial = config.underMaterial;
        topMaterial = config.topMaterial;
      } else if (y >= seaLevel - 4) {
        underMaterial = config.underMaterial;
        topMaterial = config.underMaterial;
      } else if (y >= seaLevel - (7 + surfaceDepth)) {
        topMaterial = underMaterial;
      } else {
        underMaterial = DEFAULT_BLOCK;
        topMaterial = config.underwaterMaterial;
      }

      setBlockAtYIfInside(chunk, localX, y, localZ, topMaterial);
      continue;
    }

    if (remainingDepth <= 0) {
      continue;
    }

    remainingDepth--;
    setBlockAtYIfInside(chunk, localX, y, localZ, underMaterial);
    if (remainingDepth === 0 && underMaterial === ChunkBlockId.SAND && surfaceDepth > 1) {
      remainingDepth = random.nextInt(4) + Math.max(0, y - seaLevel);
      underMaterial = ChunkBlockId.SANDSTONE;
    }
  }
}

function createBadlandsNoiseState(seed: bigint): BadlandsNoiseState {
  const clayBands = new Array<BlockId>(BADLANDS_BAND_LENGTH).fill(ChunkBlockId.TERRACOTTA);
  const bandRandom = new WorldgenRandom(seed);
  const clayBandsOffsetNoise = new PerlinSimplexNoise(bandRandom, BADLANDS_BAND_OFFSET_OCTAVES);

  for (let index = 0; index < BADLANDS_BAND_LENGTH; index++) {
    index += bandRandom.nextInt(5) + 1;
    if (index < BADLANDS_BAND_LENGTH) {
      clayBands[index] = ChunkBlockId.ORANGE_TERRACOTTA;
    }
  }

  const yellowBands = bandRandom.nextInt(4) + 2;
  for (let band = 0; band < yellowBands; band++) {
    const bandLength = bandRandom.nextInt(3) + 1;
    const start = bandRandom.nextInt(BADLANDS_BAND_LENGTH);
    for (let offset = 0; start + offset < BADLANDS_BAND_LENGTH && offset < bandLength; offset++) {
      clayBands[start + offset] = ChunkBlockId.YELLOW_TERRACOTTA;
    }
  }

  const brownBands = bandRandom.nextInt(4) + 2;
  for (let band = 0; band < brownBands; band++) {
    const bandLength = bandRandom.nextInt(3) + 2;
    const start = bandRandom.nextInt(BADLANDS_BAND_LENGTH);
    for (let offset = 0; start + offset < BADLANDS_BAND_LENGTH && offset < bandLength; offset++) {
      clayBands[start + offset] = ChunkBlockId.BROWN_TERRACOTTA;
    }
  }

  const redBands = bandRandom.nextInt(4) + 2;
  for (let band = 0; band < redBands; band++) {
    const bandLength = bandRandom.nextInt(3) + 1;
    const start = bandRandom.nextInt(BADLANDS_BAND_LENGTH);
    for (let offset = 0; start + offset < BADLANDS_BAND_LENGTH && offset < bandLength; offset++) {
      clayBands[start + offset] = ChunkBlockId.RED_TERRACOTTA;
    }
  }

  const whiteBands = bandRandom.nextInt(3) + 3;
  let start = 0;
  for (let band = 0; band < whiteBands; band++) {
    start += bandRandom.nextInt(16) + 4;
    for (let offset = 0; start + offset < BADLANDS_BAND_LENGTH && offset < 1; offset++) {
      clayBands[start + offset] = ChunkBlockId.WHITE_TERRACOTTA;
      if (start + offset > 1 && bandRandom.nextBoolean()) {
        clayBands[start + offset - 1] = ChunkBlockId.LIGHT_GRAY_TERRACOTTA;
      }

      if (start + offset < BADLANDS_BAND_LENGTH - 1 && bandRandom.nextBoolean()) {
        clayBands[start + offset + 1] = ChunkBlockId.LIGHT_GRAY_TERRACOTTA;
      }
    }
  }

  const pillarRandom = new WorldgenRandom(seed);
  return {
    clayBands,
    clayBandsOffsetNoise,
    pillarNoise: new PerlinSimplexNoise(pillarRandom, BADLANDS_PILLAR_OCTAVES),
    pillarRoofNoise: new PerlinSimplexNoise(pillarRandom, FROZEN_OCEAN_ICEBERG_ROOF_OCTAVES),
  };
}

function getBadlandsNoiseState(seed: bigint): BadlandsNoiseState {
  let state = BADLANDS_NOISE_BY_SEED.get(seed);
  if (state === undefined) {
    state = createBadlandsNoiseState(seed);
    BADLANDS_NOISE_BY_SEED.set(seed, state);
  }

  return state;
}

function getFrozenOceanNoiseState(seed: bigint): FrozenOceanNoiseState {
  let state = FROZEN_OCEAN_NOISE_BY_SEED.get(seed);
  if (state === undefined) {
    const random = new WorldgenRandom(seed);
    state = {
      icebergNoise: new PerlinSimplexNoise(random, FROZEN_OCEAN_ICEBERG_OCTAVES),
      icebergRoofNoise: new PerlinSimplexNoise(random, FROZEN_OCEAN_ICEBERG_ROOF_OCTAVES),
    };
    FROZEN_OCEAN_NOISE_BY_SEED.set(seed, state);
  }

  return state;
}

function isTerracotta(blockId: BlockId): boolean {
  switch (blockId) {
    case ChunkBlockId.TERRACOTTA:
    case ChunkBlockId.WHITE_TERRACOTTA:
    case ChunkBlockId.ORANGE_TERRACOTTA:
    case ChunkBlockId.MAGENTA_TERRACOTTA:
    case ChunkBlockId.LIGHT_BLUE_TERRACOTTA:
    case ChunkBlockId.YELLOW_TERRACOTTA:
    case ChunkBlockId.LIME_TERRACOTTA:
    case ChunkBlockId.PINK_TERRACOTTA:
    case ChunkBlockId.GRAY_TERRACOTTA:
    case ChunkBlockId.LIGHT_GRAY_TERRACOTTA:
    case ChunkBlockId.CYAN_TERRACOTTA:
    case ChunkBlockId.PURPLE_TERRACOTTA:
    case ChunkBlockId.BLUE_TERRACOTTA:
    case ChunkBlockId.BROWN_TERRACOTTA:
    case ChunkBlockId.GREEN_TERRACOTTA:
    case ChunkBlockId.RED_TERRACOTTA:
    case ChunkBlockId.BLACK_TERRACOTTA:
      return true;
    default:
      return false;
  }
}

function getBadlandsBand(state: BadlandsNoiseState, worldX: number, y: number, worldZ: number): BlockId {
  const offset = Math.round(state.clayBandsOffsetNoise.getValue(worldX / 512.0, worldZ / 512.0, false) * 2.0);
  return state.clayBands[(y + offset + BADLANDS_BAND_LENGTH) % BADLANDS_BAND_LENGTH]!;
}

function getBadlandsCeilingBlock(state: BadlandsNoiseState, worldX: number, y: number, worldZ: number, cosineBands: boolean): BlockId {
  if (y < 64 || y > 127) {
    return ChunkBlockId.ORANGE_TERRACOTTA;
  }

  return cosineBands ? ChunkBlockId.TERRACOTTA : getBadlandsBand(state, worldX, y, worldZ);
}

function applyBadlandsSurface(
  random: WorldgenRandom,
  chunk: MutableChunkBlockBuffer,
  worldX: number,
  worldZ: number,
  height: number,
  noise: number,
  seaLevel: number,
  minSurfaceLevel: number,
  config: SurfaceBuilderConfiguration,
  seed: bigint,
  wooded = false,
  eroded = false,
): void {
  const localX = localCoord(worldX);
  const localZ = localCoord(worldZ);
  const state = getBadlandsNoiseState(seed);
  let pillarHeight = 0.0;

  if (eroded) {
    const pillarNoise = Math.min(Math.abs(noise), state.pillarNoise.getValue(worldX * 0.25, worldZ * 0.25, false) * 15.0);
    if (pillarNoise > 0.0) {
      const roofNoise = Math.abs(state.pillarRoofNoise.getValue(worldX * 0.001953125, worldZ * 0.001953125, false));
      pillarHeight = pillarNoise * pillarNoise * 2.5;
      const maxPillarHeight = Math.ceil(roofNoise * 50.0) + 14.0;
      if (pillarHeight > maxPillarHeight) {
        pillarHeight = maxPillarHeight;
      }

      pillarHeight += 64.0;
    }
  }

  let topMaterial: BlockId = ChunkBlockId.WHITE_TERRACOTTA;
  const biomeUnderMaterial = config.underMaterial;
  const biomeTopMaterial = config.topMaterial;
  let underMaterial = biomeUnderMaterial;
  const surfaceDepth = Math.trunc((noise / 3.0) + 3.0 + (random.nextDouble() * 0.25));
  const cosineBands = Math.cos((noise / 3.0) * Math.PI) > 0.0;
  let remainingDepth = -1;
  let topPlaced = false;
  let stoneDepth = 0;

  for (let y = Math.max(height, Math.trunc(pillarHeight) + 1); y >= minSurfaceLevel; y--) {
    if (stoneDepth >= 15) {
      break;
    }

    const blockId = getBlockAtYOrAir(chunk, localX, y, localZ);
    if (blockId === ChunkBlockId.AIR) {
      if (eroded && y < pillarHeight) {
        setBlockAtYIfInside(chunk, localX, y, localZ, DEFAULT_BLOCK);
      }
      remainingDepth = -1;
      continue;
    }

    if (blockId !== DEFAULT_BLOCK) {
      continue;
    }

    if (remainingDepth === -1) {
      topPlaced = false;
      if (surfaceDepth <= 0) {
        topMaterial = ChunkBlockId.AIR;
        underMaterial = DEFAULT_BLOCK;
      } else if (y >= seaLevel - 4 && y <= seaLevel + 1) {
        topMaterial = ChunkBlockId.WHITE_TERRACOTTA;
        underMaterial = biomeUnderMaterial;
      }

      if (y < seaLevel && topMaterial === ChunkBlockId.AIR) {
        topMaterial = DEFAULT_FLUID;
      }

      remainingDepth = surfaceDepth + Math.max(0, y - seaLevel);
      if (y >= seaLevel - 1) {
        if (wooded && y > 86 + (surfaceDepth * 2)) {
          setBlockAtYIfInside(
            chunk,
            localX,
            y,
            localZ,
            cosineBands ? ChunkBlockId.COARSE_DIRT : ChunkBlockId.GRASS_BLOCK,
          );
        } else if (y <= seaLevel + 3 + surfaceDepth) {
          setBlockAtYIfInside(chunk, localX, y, localZ, biomeTopMaterial);
          topPlaced = true;
        } else {
          setBlockAtYIfInside(chunk, localX, y, localZ, getBadlandsCeilingBlock(state, worldX, y, worldZ, cosineBands));
        }
      } else {
        setBlockAtYIfInside(chunk, localX, y, localZ, underMaterial);
        if ((wooded && underMaterial === ChunkBlockId.WHITE_TERRACOTTA) || (!wooded && isTerracotta(underMaterial))) {
          setBlockAtYIfInside(chunk, localX, y, localZ, ChunkBlockId.ORANGE_TERRACOTTA);
        }
      }
    } else if (remainingDepth > 0) {
      remainingDepth--;
      setBlockAtYIfInside(
        chunk,
        localX,
        y,
        localZ,
        topPlaced ? ChunkBlockId.ORANGE_TERRACOTTA : getBadlandsBand(state, worldX, y, worldZ),
      );
    }

    stoneDepth++;
  }
}

function applyFrozenOceanSurface(
  random: WorldgenRandom,
  chunk: MutableChunkBlockBuffer,
  biome: Biome,
  worldX: number,
  worldZ: number,
  height: number,
  noise: number,
  seaLevel: number,
  minSurfaceLevel: number,
  config: SurfaceBuilderConfiguration,
  seed: bigint,
): void {
  let icebergHeight = 0.0;
  let icebergBaseY = 0.0;
  const noiseState = getFrozenOceanNoiseState(seed);
  const biomeTemperature = biome.getTemperature(new BlockPos(worldX, 63, worldZ));
  const icebergNoise = Math.min(Math.abs(noise), noiseState.icebergNoise.getValue(worldX * 0.1, worldZ * 0.1, false) * 15.0);
  if (icebergNoise > 1.8) {
    const roofNoise = Math.abs(noiseState.icebergRoofNoise.getValue(worldX * 0.09765625, worldZ * 0.09765625, false));
    icebergHeight = icebergNoise * icebergNoise * 1.2;
    const maxIcebergHeight = Math.ceil(roofNoise * 40.0) + 14.0;
    if (icebergHeight > maxIcebergHeight) {
      icebergHeight = maxIcebergHeight;
    }

    if (biomeTemperature > 0.1) {
      icebergHeight -= 2.0;
    }

    if (icebergHeight > 2.0) {
      icebergBaseY = seaLevel - icebergHeight - 7.0;
      icebergHeight += seaLevel;
    } else {
      icebergHeight = 0.0;
    }
  }

  const icebergHeightInt = Math.trunc(icebergHeight);
  const icebergBaseYInt = Math.trunc(icebergBaseY);

  const localX = localCoord(worldX);
  const localZ = localCoord(worldZ);
  let underMaterial = config.underMaterial;
  let topMaterial = config.topMaterial;
  const surfaceDepth = Math.trunc((noise / 3.0) + 3.0 + (random.nextDouble() * 0.25));
  let remainingDepth = -1;
  let snowLayersPlaced = 0;
  const maxSnowLayers = 2 + random.nextInt(4);
  const snowStartY = seaLevel + 18 + random.nextInt(10);

  for (let y = Math.max(height, icebergHeightInt + 1); y >= minSurfaceLevel; y--) {
    let blockId = getBlockAtYOrAir(chunk, localX, y, localZ);
    if (blockId === ChunkBlockId.AIR && y < icebergHeightInt && random.nextDouble() > 0.01) {
      setBlockAtYIfInside(chunk, localX, y, localZ, ChunkBlockId.PACKED_ICE);
      blockId = ChunkBlockId.PACKED_ICE;
    } else if (
      blockId === ChunkBlockId.WATER &&
      y > icebergBaseYInt &&
      y < seaLevel &&
      icebergBaseY !== 0.0 &&
      random.nextDouble() > 0.15
    ) {
      setBlockAtYIfInside(chunk, localX, y, localZ, ChunkBlockId.PACKED_ICE);
      blockId = ChunkBlockId.PACKED_ICE;
    }

    if (blockId === ChunkBlockId.AIR) {
      remainingDepth = -1;
      continue;
    }

    if (blockId === DEFAULT_BLOCK) {
      if (remainingDepth === -1) {
        if (surfaceDepth <= 0) {
          topMaterial = ChunkBlockId.AIR;
          underMaterial = DEFAULT_BLOCK;
        } else if (y >= seaLevel - 4 && y <= seaLevel + 1) {
          topMaterial = config.topMaterial;
          underMaterial = config.underMaterial;
        }

        if (y < seaLevel && topMaterial === ChunkBlockId.AIR) {
          topMaterial = biome.getTemperature(new BlockPos(worldX, y, worldZ)) < 0.15 ? ChunkBlockId.ICE : DEFAULT_FLUID;
        }

        remainingDepth = surfaceDepth;
        if (y >= seaLevel - 1) {
          setBlockAtYIfInside(chunk, localX, y, localZ, topMaterial);
        } else if (y < seaLevel - 7 - surfaceDepth) {
          topMaterial = ChunkBlockId.AIR;
          underMaterial = DEFAULT_BLOCK;
          setBlockAtYIfInside(chunk, localX, y, localZ, ChunkBlockId.GRAVEL);
        } else {
          setBlockAtYIfInside(chunk, localX, y, localZ, underMaterial);
        }
      } else if (remainingDepth > 0) {
        remainingDepth--;
        setBlockAtYIfInside(chunk, localX, y, localZ, underMaterial);
        if (remainingDepth === 0 && (underMaterial === ChunkBlockId.SAND || underMaterial === ChunkBlockId.RED_SAND) && surfaceDepth > 1) {
          remainingDepth = random.nextInt(4) + Math.max(0, y - 63);
          underMaterial = underMaterial === ChunkBlockId.RED_SAND ? ChunkBlockId.RED_SANDSTONE : ChunkBlockId.SANDSTONE;
        }
      }
    } else if (blockId === ChunkBlockId.PACKED_ICE && snowLayersPlaced <= maxSnowLayers && y > snowStartY) {
      setBlockAtYIfInside(chunk, localX, y, localZ, ChunkBlockId.SNOW_BLOCK);
      snowLayersPlaced++;
    }
  }
}

export function applyOverworldSurface(
  random: WorldgenRandom,
  chunk: MutableChunkBlockBuffer,
  biome: Biome,
  worldX: number,
  worldZ: number,
  height: number,
  noise: number,
  seaLevel: number,
  minSurfaceLevel: number,
  seed: bigint,
): void {
  const definition = resolveSurfaceBiomeDefinition(biome);

  switch (definition.builder) {
    case "default":
      applyDefaultSurface(
        random,
        chunk,
        biome,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        definition.config,
      );
      return;
    case "mountain":
      applyDefaultSurface(
        random,
        chunk,
        biome,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        noise > 1.0 ? CONFIG_STONE : CONFIG_GRASS,
      );
      return;
    case "gravelly_mountain":
      applyDefaultSurface(
        random,
        chunk,
        biome,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        noise < -1.0 || noise > 2.0 ? CONFIG_GRAVEL : noise > 1.0 ? CONFIG_STONE : CONFIG_GRASS,
      );
      return;
    case "giant_tree_taiga":
      applyDefaultSurface(
        random,
        chunk,
        biome,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        noise > 1.75 ? CONFIG_COARSE_DIRT : noise > -0.95 ? CONFIG_PODZOL : CONFIG_GRASS,
      );
      return;
    case "shattered_savanna":
      applyDefaultSurface(
        random,
        chunk,
        biome,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        noise > 1.75 ? CONFIG_STONE : noise > -0.5 ? CONFIG_COARSE_DIRT : CONFIG_GRASS,
      );
      return;
    case "swamp":
      if (Biome.BIOME_INFO_NOISE.getValue(worldX * 0.25, worldZ * 0.25, false) > 0.0) {
        const localX = localCoord(worldX);
        const localZ = localCoord(worldZ);
        for (let y = height; y >= minSurfaceLevel; y--) {
          const blockId = getBlockAtYOrAir(chunk, localX, y, localZ);
          if (blockId === ChunkBlockId.AIR) {
            continue;
          }

          if (y === 62 && blockId !== DEFAULT_FLUID) {
            setBlockAtYIfInside(chunk, localX, y, localZ, DEFAULT_FLUID);
          }
          break;
        }
      }

      applyDefaultSurface(
        random,
        chunk,
        biome,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        definition.config,
      );
      return;
    case "badlands":
      applyBadlandsSurface(random, chunk, worldX, worldZ, height, noise, seaLevel, minSurfaceLevel, definition.config, seed);
      return;
    case "wooded_badlands":
      applyBadlandsSurface(random, chunk, worldX, worldZ, height, noise, seaLevel, minSurfaceLevel, definition.config, seed, true);
      return;
    case "eroded_badlands":
      applyBadlandsSurface(random, chunk, worldX, worldZ, height, noise, seaLevel, minSurfaceLevel, definition.config, seed, false, true);
      return;
    case "frozen_ocean":
      applyFrozenOceanSurface(random, chunk, biome, worldX, worldZ, height, noise, seaLevel, minSurfaceLevel, definition.config, seed);
      return;
  }
}
