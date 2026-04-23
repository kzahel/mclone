import { Biome } from "../../biome/biome";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { BlockPos } from "../../../core/block-pos";
import { NoiseCountFactorDecoratorConfiguration } from "../feature/configurations/noise-count-factor-decorator-configuration";
import { RepeatingDecorator } from "./repeating-decorator";

export class CountNoiseBiasedDecorator extends RepeatingDecorator<NoiseCountFactorDecoratorConfiguration> {
  protected override count(_random: SimpleRandomSource, config: NoiseCountFactorDecoratorConfiguration, pos: BlockPos): number {
    const noise = Biome.BIOME_INFO_NOISE.getValue(pos.getX() / config.noiseFactor, pos.getZ() / config.noiseFactor, false);
    return Math.ceil((noise + config.noiseOffset) * config.noiseToCountRatio);
  }
}
