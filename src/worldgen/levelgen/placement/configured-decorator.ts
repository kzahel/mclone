import { BlockPos } from "../../../core/block-pos";
import { ConstantInt } from "../../../util/valueproviders/constant-int";
import type { IntProvider } from "../../../util/valueproviders/int-provider";
import { UniformInt } from "../../../util/valueproviders/uniform-int";
import { ChanceDecoratorConfiguration } from "../feature/configurations/chance-decorator-configuration";
import { CountConfiguration } from "../feature/configurations/count-configuration";
import { DecoratedDecoratorConfiguration } from "../feature/configurations/decorated-decorator-configuration";
import type { DecoratorConfiguration } from "../feature/configurations/decorator-configuration";
import { NoneDecoratorConfiguration } from "../feature/configurations/none-decorator-configuration";
import type { DecorationContext } from "./decoration-context";
import { FeatureDecorators } from "./feature-decorators";
import { FeatureDecorator } from "./feature-decorator";
import type { SimpleRandomSource } from "../../prng/simple-random-source";

export class ConfiguredDecorator<DC extends DecoratorConfiguration> {
  public constructor(
    private readonly decorator: FeatureDecorator<DC>,
    private readonly configValue: DC,
  ) {}

  public getPositions(context: DecorationContext, random: SimpleRandomSource, pos: BlockPos): Iterable<BlockPos> {
    return this.decorator.getPositions(context, random, this.configValue, pos);
  }

  public decorated(decorator: ConfiguredDecorator<DecoratorConfiguration>): ConfiguredDecorator<DecoratedDecoratorConfiguration> {
    return new ConfiguredDecorator(FeatureDecorators.DECORATED, new DecoratedDecoratorConfiguration(decorator, this));
  }

  public count(count: number | IntProvider): ConfiguredDecorator<DecoratedDecoratorConfiguration> {
    return this.decorated(FeatureDecorators.COUNT.configured(new CountConfiguration(typeof count === "number" ? ConstantInt.of(count) : count)));
  }

  public countRandom(maxInclusive: number): ConfiguredDecorator<DecoratedDecoratorConfiguration> {
    return this.count(UniformInt.of(0, maxInclusive));
  }

  public rarity(chance: number): ConfiguredDecorator<DecoratedDecoratorConfiguration> {
    return this.decorated(FeatureDecorators.CHANCE.configured(new ChanceDecoratorConfiguration(chance)));
  }

  public squared(): ConfiguredDecorator<DecoratedDecoratorConfiguration> {
    return this.decorated(FeatureDecorators.SQUARE.configured(NoneDecoratorConfiguration.INSTANCE));
  }

  public config(): DC {
    return this.configValue;
  }
}
