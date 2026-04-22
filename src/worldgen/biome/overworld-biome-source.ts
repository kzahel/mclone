import type { LongSeed } from "../prng/simple-random-source";
import { OVERWORLD_LAYERED_BIOMES, getLayeredBiomeById } from "./biome-data";
import type { Biome } from "./biome";
import type { NoiseBiomeSource } from "./noise-biome-source";
import { buildOverworldBiomeArea } from "./layered/layers";

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

export class OverworldBiomeSource implements NoiseBiomeSource {
  private readonly seed: bigint;
  private readonly noiseBiomeArea;

  public constructor(
    seed: LongSeed,
    private readonly legacyBiomeInitLayer = false,
    private readonly largeBiomes = false,
  ) {
    this.seed = normalizeLongSeed(seed);
    this.noiseBiomeArea = buildOverworldBiomeArea(this.seed, this.legacyBiomeInitLayer, this.largeBiomes ? 6 : 4, 4);
  }

  public withSeed(seed: LongSeed): OverworldBiomeSource {
    return new OverworldBiomeSource(seed, this.legacyBiomeInitLayer, this.largeBiomes);
  }

  public getPossibleBiomes(): readonly Biome[] {
    return OVERWORLD_LAYERED_BIOMES;
  }

  public getNoiseBiomeId(x: number, _y: number, z: number): number {
    return this.noiseBiomeArea.get(x, z);
  }

  public getNoiseBiome(x: number, y: number, z: number): Biome {
    return getLayeredBiomeById(this.getNoiseBiomeId(x, y, z));
  }
}
