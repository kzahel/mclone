import type { NoiseBiomeSource } from "../biome/noise-biome-source";
import type { MutableChunkBlockBuffer } from "../chunk/chunk-block-buffer";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { CooperativeGenerationYield } from "./cooperative-generation";
import type { BiomeDecorationProfiler } from "./decoration-profiler";
import type { GenerationEntitySink, NaturalSpawnerOptions } from "./natural-spawner";
import type { BaseStoneSource } from "./base-stone-source";
import type { GenerationStep } from "./generation-step";

export interface WorldGenerator {
  getSeed(): bigint;
  getBiomeSource(): NoiseBiomeSource;
  getBaseStoneSource(): BaseStoneSource;
  fillFromNoise(chunkX: number, chunkZ: number): MutableChunkBlockBuffer;
  buildSurfaceAndBedrock(chunk: MutableChunkBlockBuffer): void;
  applyCarvers(chunk: MutableChunkBlockBuffer, step?: GenerationStep.Carving): void;
  applyCarversCooperative(chunk: MutableChunkBlockBuffer, yieldStep: CooperativeGenerationYield): Promise<void>;
  applyBiomeDecoration(
    level: WorldGenLevel,
    chunkX: number,
    chunkZ: number,
    profiler?: BiomeDecorationProfiler,
  ): void;
  applyBiomeDecorationCooperative(
    level: WorldGenLevel,
    chunkX: number,
    chunkZ: number,
    yieldStep: CooperativeGenerationYield,
    profiler?: BiomeDecorationProfiler,
  ): Promise<void>;
  spawnOriginalMobs(
    level: WorldGenLevel,
    chunkX: number,
    chunkZ: number,
    sink: GenerationEntitySink,
    options?: NaturalSpawnerOptions,
  ): void;
}
