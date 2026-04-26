import { OverworldBiomeSource } from "../biome/overworld-biome-source";
import type { LongSeed } from "../prng/simple-random-source";
import { FlatGrassWorldGenerator, SmallIslandWorldGenerator } from "./demo-world-generators";
import { NoiseBasedChunkGenerator } from "./noise-based-chunk-generator";
import type { WorldGenerator } from "./world-generator";

export type WorldGeneratorPreset = "default" | "browser_smoke" | "flat_grass" | "small_island";

export function createWorldGeneratorForPreset(preset: WorldGeneratorPreset, seed: LongSeed): WorldGenerator {
  switch (preset) {
    case "flat_grass":
      return new FlatGrassWorldGenerator(seed);
    case "small_island":
      return new SmallIslandWorldGenerator(seed);
    case "browser_smoke":
    case "default":
    default:
      return new NoiseBasedChunkGenerator(new OverworldBiomeSource(seed), seed);
  }
}
