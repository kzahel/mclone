import type { NoiseBiome } from "./noise-biome";

export interface NoiseBiomeSource {
  getNoiseBiome(x: number, y: number, z: number): NoiseBiome;
}
