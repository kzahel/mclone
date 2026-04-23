import { BlockPos } from "../../../core/block-pos";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { NoiseBasedChunkGenerator } from "../noise-based-chunk-generator";
import type { ConfiguredFeature } from "./configured-feature";

export class WeightedConfiguredFeature {
  public constructor(
    public readonly feature: ConfiguredFeature<any, any>,
    public readonly chance: number,
  ) {}

  public place(level: WorldGenLevel, chunkGenerator: NoiseBasedChunkGenerator, random: SimpleRandomSource, pos: BlockPos): boolean {
    return this.feature.place(level, chunkGenerator, random, pos);
  }
}
