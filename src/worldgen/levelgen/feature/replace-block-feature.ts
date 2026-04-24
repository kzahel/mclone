import { ReplaceBlockConfiguration } from "./configurations/replace-block-configuration";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";

export class ReplaceBlockFeature extends Feature<ReplaceBlockConfiguration> {
  public override place(context: FeaturePlaceContext<ReplaceBlockConfiguration>): boolean {
    const level = context.level();
    const origin = context.origin();
    const config = context.config();

    for (const targetState of config.targetStates) {
      if (targetState.target.test(level.getBlockState(origin), context.random())) {
        level.setBlock(origin, targetState.state, 2);
        break;
      }
    }

    return true;
  }
}
