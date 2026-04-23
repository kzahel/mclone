import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { BlockStateProvider } from "../stateproviders/block-state-provider";
import type { FeatureConfiguration } from "./feature-configuration";

export class SimpleBlockConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly toPlace: BlockStateProvider,
    public readonly placeOn: readonly BlockState[] = [],
    public readonly placeIn: readonly BlockState[] = [],
    public readonly placeUnder: readonly BlockState[] = [],
  ) {}
}
