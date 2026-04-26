import { getBlockPositionBiome } from "../biome/biome-zoom.ts";
import type { Biome } from "../biome/biome.ts";
import type { NoiseBiomeSource } from "../biome/noise-biome-source.ts";
import { GenerationStep } from "../levelgen/generation-step.ts";
import { WorldgenRandom } from "../prng/worldgen-random.ts";
import type { CarverContext } from "./carver-config.ts";
import type { ChunkPosLike } from "./world-carver.ts";
import { MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer.ts";

function biomeAccessor(seed: bigint, biomeSource: NoiseBiomeSource) {
  return (worldX: number, _worldY: number, worldZ: number): Biome =>
    getBlockPositionBiome(seed, worldX, worldZ, biomeSource) as Biome;
}

export function applyOverworldCarvers(
  seed: bigint,
  biomeSource: NoiseBiomeSource,
  chunk: MutableChunkBlockBuffer,
  context: CarverContext,
  step: GenerationStep.Carving,
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
      const carvers = sourceBiome.getGenerationSettings().carvers(step);

      for (let carverIndex = 0; carverIndex < carvers.length; carverIndex++) {
        const configuredCarver = carvers[carverIndex]!();
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

  chunk.setCarvingMask(step, carvingMask);
}

export function applyOverworldAirCarvers(
  seed: bigint,
  biomeSource: NoiseBiomeSource,
  chunk: MutableChunkBlockBuffer,
  context: CarverContext,
): void {
  applyOverworldCarvers(seed, biomeSource, chunk, context, GenerationStep.Carving.AIR);
}
