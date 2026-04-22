import { getLayeredBiomeById } from "./biome-data";
import type { Biome } from "./biome";
import type { NoiseBiomeSource } from "./noise-biome-source";

const WIDTH_BITS = 2;
const HORIZONTAL_MASK = (1 << WIDTH_BITS) - 1;
const HORIZONTAL_AREA = 1 << (WIDTH_BITS + WIDTH_BITS);

function quartFromBlock(value: number): number {
  return Math.floor(value / 4);
}

function ceilDiv(value: number, divisor: number): number {
  return Math.floor((value + divisor - 1) / divisor);
}

function clamp(value: number, minValue: number, maxValue: number): number {
  return Math.min(Math.max(value, minValue), maxValue);
}

export class ChunkBiomeContainer implements NoiseBiomeSource {
  private readonly biomes: Biome[];
  private readonly quartMinY: number;
  private readonly quartHeight: number;

  public constructor(
    minBuildHeight: number,
    height: number,
    chunkX: number,
    chunkZ: number,
    biomeSource: NoiseBiomeSource,
    biomeIds?: readonly number[],
  ) {
    this.quartMinY = quartFromBlock(minBuildHeight);
    this.quartHeight = quartFromBlock(height) - 1;
    this.biomes = new Array(HORIZONTAL_AREA * ceilDiv(height, 4));

    const minQuartX = quartFromBlock(chunkX * 16);
    const minQuartZ = quartFromBlock(chunkZ * 16);

    for (let index = 0; index < this.biomes.length; index++) {
      const biomeId = biomeIds?.[index];
      this.biomes[index] =
        biomeId === undefined
          ? ChunkBiomeContainer.generateBiomeForIndex(biomeSource, minQuartX, this.quartMinY, minQuartZ, index)
          : getLayeredBiomeById(biomeId);
    }
  }

  private static generateBiomeForIndex(
    biomeSource: NoiseBiomeSource,
    minQuartX: number,
    quartMinY: number,
    minQuartZ: number,
    index: number,
  ): Biome {
    const localX = index & HORIZONTAL_MASK;
    const localY = index >> (WIDTH_BITS + WIDTH_BITS);
    const localZ = (index >> WIDTH_BITS) & HORIZONTAL_MASK;
    return biomeSource.getNoiseBiome(minQuartX + localX, quartMinY + localY, minQuartZ + localZ) as Biome;
  }

  public writeBiomes(): number[] {
    return this.biomes.map((biome) => biome.getId());
  }

  public getNoiseBiome(x: number, y: number, z: number): Biome {
    const localX = x & HORIZONTAL_MASK;
    const localY = clamp(y - this.quartMinY, 0, this.quartHeight);
    const localZ = z & HORIZONTAL_MASK;
    return this.biomes[(localY << (WIDTH_BITS + WIDTH_BITS)) | (localZ << WIDTH_BITS) | localX]!;
  }
}
