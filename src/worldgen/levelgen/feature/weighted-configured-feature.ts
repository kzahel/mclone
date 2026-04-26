import { BlockPos } from "../../../core/block-pos";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { ConfiguredFeature } from "./configured-feature";
import type { WorldGenerator } from "../world-generator";

export class WeightedConfiguredFeature {
  public constructor(
    public readonly feature: ConfiguredFeature<any, any>,
    public readonly chance: number,
  ) {}

  public place(level: WorldGenLevel, chunkGenerator: WorldGenerator, random: SimpleRandomSource, pos: BlockPos): boolean {
    return this.feature.place(level, chunkGenerator, random, pos);
  }
}
