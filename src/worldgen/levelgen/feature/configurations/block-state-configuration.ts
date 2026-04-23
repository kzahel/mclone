import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { FeatureConfiguration } from "./feature-configuration";

export class BlockStateConfiguration implements FeatureConfiguration {
  public constructor(public readonly state: BlockState) {}
}
