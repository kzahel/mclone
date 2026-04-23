import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { FeatureConfiguration } from "./feature-configuration";

export class DiskConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly state: BlockState,
    public readonly radius: IntProvider,
    public readonly halfHeight: number,
    public readonly targets: readonly BlockState[],
  ) {}
}
