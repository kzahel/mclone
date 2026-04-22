import { getBlockPositionBiome } from "../biome/biome-zoom.ts";
import type { Biome } from "../biome/biome.ts";
import type { NoiseBiomeSource } from "../biome/noise-biome-source.ts";
import { WorldgenRandom } from "../prng/worldgen-random.ts";
import {
  aboveBottom,
  absolute,
  biasedToBottomHeight,
  constantFloat,
  type CarverConfiguration,
  type CarverContext,
  type CaveCarverConfiguration,
  type CanyonCarverConfiguration,
  trapezoidFloat,
  uniformFloat,
} from "./carver-config.ts";
import { CanyonWorldCarver } from "./canyon-world-carver.ts";
import { CaveWorldCarver } from "./cave-world-carver.ts";
import { type ChunkPosLike, type WorldCarver } from "./world-carver.ts";
import { MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";

interface ConfiguredWorldCarver<C extends CarverConfiguration> {
  readonly worldCarver: WorldCarver<C>;
  readonly config: C;
}

const CAVE = configure(new CaveWorldCarver(), {
  probability: 0.14285715,
  y: biasedToBottomHeight(absolute(0), absolute(127), 8),
  yScale: constantFloat(0.5),
  lavaLevel: aboveBottom(10),
  aquifersEnabled: false,
  horizontalRadiusMultiplier: constantFloat(1.0),
  verticalRadiusMultiplier: constantFloat(1.0),
  floorLevel: constantFloat(-0.7),
} satisfies CaveCarverConfiguration);

const OCEAN_CAVE = configure(new CaveWorldCarver(), {
  probability: 0.06666667,
  y: biasedToBottomHeight(absolute(0), absolute(127), 8),
  yScale: constantFloat(0.5),
  lavaLevel: aboveBottom(10),
  aquifersEnabled: false,
  horizontalRadiusMultiplier: constantFloat(1.0),
  verticalRadiusMultiplier: constantFloat(1.0),
  floorLevel: constantFloat(-0.7),
} satisfies CaveCarverConfiguration);

const CANYON = configure(new CanyonWorldCarver(), {
  probability: 0.02,
  y: biasedToBottomHeight(absolute(20), absolute(67), 8),
  yScale: constantFloat(3.0),
  lavaLevel: aboveBottom(10),
  aquifersEnabled: false,
  verticalRotation: uniformFloat(-0.125, 0.125),
  shape: {
    distanceFactor: uniformFloat(0.75, 1.0),
    thickness: trapezoidFloat(0.0, 6.0, 2.0),
    widthSmoothness: 3,
    horizontalRadiusFactor: uniformFloat(0.75, 1.0),
    verticalRadiusDefaultFactor: 1.0,
    verticalRadiusCenterFactor: 0.0,
  },
} satisfies CanyonCarverConfiguration);

const OCEAN_BIOME_KEYS = new Set([
  "minecraft:ocean",
  "minecraft:deep_ocean",
  "minecraft:warm_ocean",
  "minecraft:deep_warm_ocean",
  "minecraft:lukewarm_ocean",
  "minecraft:deep_lukewarm_ocean",
  "minecraft:cold_ocean",
  "minecraft:deep_cold_ocean",
  "minecraft:frozen_ocean",
  "minecraft:deep_frozen_ocean",
]);

function configure<C extends CarverConfiguration>(
  worldCarver: WorldCarver<C>,
  config: C,
): ConfiguredWorldCarver<C> {
  return { worldCarver, config };
}

function getAirCarversForBiome(biome: Biome): readonly ConfiguredWorldCarver<CarverConfiguration>[] {
  return OCEAN_BIOME_KEYS.has(biome.getKey()) ? [OCEAN_CAVE, CANYON] : [CAVE, CANYON];
}

function biomeAccessor(seed: bigint, biomeSource: NoiseBiomeSource) {
  return (worldX: number, _worldY: number, worldZ: number): Biome =>
    getBlockPositionBiome(seed, worldX, worldZ, biomeSource) as Biome;
}

export function applyOverworldAirCarvers(
  seed: bigint,
  biomeSource: NoiseBiomeSource,
  chunk: MutableChunkBlockBuffer,
  context: CarverContext,
): void {
  const random = new WorldgenRandom();
  const chunkPos: ChunkPosLike = {
    chunkX: chunk.chunkX,
    chunkZ: chunk.chunkZ,
  };
  const carvingMask = new Uint8Array(chunk.height * 16 * 16);
  const biomeAtBlockPosition = biomeAccessor(seed, biomeSource);

  for (let offsetX = -8; offsetX <= 8; offsetX++) {
    for (let offsetZ = -8; offsetZ <= 8; offsetZ++) {
      const sourceChunkX = chunkPos.chunkX + offsetX;
      const sourceChunkZ = chunkPos.chunkZ + offsetZ;
      const sourceBiome = biomeSource.getNoiseBiome(sourceChunkX << 2, 0, sourceChunkZ << 2) as Biome;
      const carvers = getAirCarversForBiome(sourceBiome);

      for (let carverIndex = 0; carverIndex < carvers.length; carverIndex++) {
        const configuredCarver = carvers[carverIndex]!;
        random.setLargeFeatureSeed(seed + BigInt(carverIndex), sourceChunkX, sourceChunkZ);
        if (configuredCarver.worldCarver.isStartChunk(configuredCarver.config, random)) {
          configuredCarver.worldCarver.carve(
            context,
            configuredCarver.config,
            chunk,
            biomeAtBlockPosition,
            random,
            { chunkX: sourceChunkX, chunkZ: sourceChunkZ },
            carvingMask,
          );
        }
      }
    }
  }
}
