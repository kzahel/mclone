import type { Biome } from "../../worldgen/biome/biome";

export interface ColorResolver {
  getColor(biome: Biome, x: number, z: number): number;
}
