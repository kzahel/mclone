import type { FeatureConfiguration } from "./feature-configuration";
import type { BlockStateProvider } from "../stateproviders/block-state-provider";

export class HugeMushroomFeatureConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly capProvider: BlockStateProvider,
    public readonly stemProvider: BlockStateProvider,
    public readonly foliageRadius: number,
  ) {}
}
