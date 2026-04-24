import type { FeatureConfiguration } from "./feature-configuration";

export class SmallDripstoneConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly maxPlacements: number,
    public readonly emptySpaceSearchRadius: number,
    public readonly maxOffsetFromOrigin: number,
    public readonly chanceOfTallerDripstone: number,
  ) {}
}
