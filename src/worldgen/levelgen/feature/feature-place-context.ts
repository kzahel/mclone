import { BlockPos } from "../../../core/block-pos";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import type { NoiseBasedChunkGenerator } from "../noise-based-chunk-generator";
import type { FeatureConfiguration } from "./configurations/feature-configuration";

export class FeaturePlaceContext<FC extends FeatureConfiguration> {
  public constructor(
    private readonly levelValue: WorldGenLevel,
    private readonly chunkGeneratorValue: NoiseBasedChunkGenerator,
    private readonly randomValue: SimpleRandomSource,
    private readonly originValue: BlockPos,
    private readonly configValue: FC,
  ) {}

  public level(): WorldGenLevel {
    return this.levelValue;
  }

  public chunkGenerator(): NoiseBasedChunkGenerator {
    return this.chunkGeneratorValue;
  }

  public random(): SimpleRandomSource {
    return this.randomValue;
  }

  public origin(): BlockPos {
    return this.originValue;
  }

  public config(): FC {
    return this.configValue;
  }
}
