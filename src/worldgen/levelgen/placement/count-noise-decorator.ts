import { Biome } from "../../biome/biome";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { BlockPos } from "../../../core/block-pos";
import { NoiseDependantDecoratorConfiguration } from "../feature/configurations/noise-dependant-decorator-configuration";
import { RepeatingDecorator } from "./repeating-decorator";

export class CountNoiseDecorator extends RepeatingDecorator<NoiseDependantDecoratorConfiguration> {
  protected override count(_random: SimpleRandomSource, config: NoiseDependantDecoratorConfiguration, pos: BlockPos): number {
    const noise = Biome.BIOME_INFO_NOISE.getValue(pos.getX() / 200.0, pos.getZ() / 200.0, false);
    return noise < config.noiseLevel ? config.belowNoise : config.aboveNoise;
  }
}
