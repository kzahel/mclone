import { Biome } from "../biome/biome.ts";
import { CHUNK_WIDTH, ChunkBlockId, type ChunkBlockId as BlockId, MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";
import { WorldgenRandom } from "../prng/worldgen-random.ts";

interface SurfaceBuilderConfiguration {
  readonly topMaterial: BlockId;
  readonly underMaterial: BlockId;
  readonly underwaterMaterial: BlockId;
}

type SurfaceBuilderKind = "default" | "mountain" | "gravelly_mountain" | "swamp";

interface SurfaceBiomeDefinition {
  readonly builder: SurfaceBuilderKind;
  readonly baseTemperature: number;
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
        baseTemperature: 0.2,
        config: CONFIG_GRASS,
      };
    case "minecraft:gravelly_mountains":
    case "minecraft:modified_gravelly_mountains":
      return {
        builder: "gravelly_mountain",
        baseTemperature: 0.2,
        config: CONFIG_GRASS,
      };
    case "minecraft:desert":
    case "minecraft:desert_hills":
    case "minecraft:desert_lakes":
    case "minecraft:beach":
      return {
        builder: "default",
        baseTemperature: 2.0,
        config: CONFIG_DESERT,
      };
    case "minecraft:ocean":
    case "minecraft:deep_ocean":
    case "minecraft:cold_ocean":
    case "minecraft:deep_cold_ocean":
      return {
        builder: "default",
        baseTemperature: 0.5,
        config: CONFIG_GRASS,
      };
    case "minecraft:lukewarm_ocean":
    case "minecraft:deep_lukewarm_ocean":
      return {
        builder: "default",
        baseTemperature: 0.5,
        config: CONFIG_OCEAN_SAND,
      };
    case "minecraft:warm_ocean":
    case "minecraft:deep_warm_ocean":
      return {
        builder: "default",
        baseTemperature: 0.5,
        config: CONFIG_FULL_SAND,
      };
    case "minecraft:swamp":
    case "minecraft:swamp_hills":
      return {
        builder: "swamp",
        baseTemperature: 0.8,
        config: CONFIG_GRASS,
      };
    case "minecraft:frozen_ocean":
    case "minecraft:deep_frozen_ocean":
      throw new RangeError("Frozen-ocean surface mutation needs ice/snow-block support beyond the tactical 07 numeric model");
    case "minecraft:badlands":
    case "minecraft:wooded_badlands_plateau":
    case "minecraft:badlands_plateau":
    case "minecraft:eroded_badlands":
    case "minecraft:modified_wooded_badlands_plateau":
    case "minecraft:modified_badlands_plateau":
      throw new RangeError("Badlands surface mutation needs terracotta/red-sand materials beyond the tactical 07 numeric model");
    case "minecraft:giant_tree_taiga":
    case "minecraft:giant_tree_taiga_hills":
    case "minecraft:giant_spruce_taiga":
    case "minecraft:giant_spruce_taiga_hills":
    case "minecraft:shattered_savanna":
    case "minecraft:shattered_savanna_plateau":
      throw new RangeError("This surface builder needs podzol/coarse-dirt support beyond the tactical 07 numeric model");
    case "minecraft:mushroom_fields":
    case "minecraft:mushroom_field_shore":
      throw new RangeError("Mycelium surface mutation is still out of scope for tactical 07");
    case "minecraft:stone_shore":
      return {
        builder: "default",
        baseTemperature: 0.2,
        config: CONFIG_STONE,
      };
    case "minecraft:taiga":
    case "minecraft:taiga_hills":
    case "minecraft:taiga_mountains":
      return {
        builder: "default",
        baseTemperature: 0.25,
        config: CONFIG_GRASS,
      };
    default:
      return {
        builder: "default",
        baseTemperature: 0.8,
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
  worldX: number,
  worldZ: number,
  height: number,
  noise: number,
  seaLevel: number,
  minSurfaceLevel: number,
  baseTemperature: number,
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
            if (baseTemperature < 0.15) {
              throw new RangeError("Frozen surface replacement needs ice support beyond the tactical 07 numeric model");
            }
            replacement = DEFAULT_FLUID;
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
      throw new RangeError("Sand surface mutation needs sandstone support beyond the tactical 07 numeric model");
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
): void {
  const definition = resolveSurfaceBiomeDefinition(biome);

  switch (definition.builder) {
    case "default":
      applyDefaultSurface(
        random,
        chunk,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        definition.baseTemperature,
        definition.config,
      );
      return;
    case "mountain":
      applyDefaultSurface(
        random,
        chunk,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        definition.baseTemperature,
        noise > 1.0 ? CONFIG_STONE : CONFIG_GRASS,
      );
      return;
    case "gravelly_mountain":
      applyDefaultSurface(
        random,
        chunk,
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        definition.baseTemperature,
        noise < -1.0 || noise > 2.0 ? CONFIG_GRAVEL : noise > 1.0 ? CONFIG_STONE : CONFIG_GRASS,
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
        worldX,
        worldZ,
        height,
        noise,
        seaLevel,
        minSurfaceLevel,
        definition.baseTemperature,
        definition.config,
      );
      return;
  }
}
