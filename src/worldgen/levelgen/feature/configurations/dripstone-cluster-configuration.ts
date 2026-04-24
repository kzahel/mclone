import type { FloatProvider } from "../../../../util/valueproviders/float-provider";
import type { IntProvider } from "../../../../util/valueproviders/int-provider";
import type { FeatureConfiguration } from "./feature-configuration";

export class DripstoneClusterConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly floorToCeilingSearchRange: number,
    public readonly height: IntProvider,
    public readonly radius: IntProvider,
    public readonly maxStalagmiteStalactiteHeightDiff: number,
    public readonly heightDeviation: number,
    public readonly dripstoneBlockLayerThickness: IntProvider,
    public readonly density: FloatProvider,
    public readonly wetness: FloatProvider,
    public readonly chanceOfDripstoneColumnAtMaxDistanceFromCenter: number,
    public readonly maxDistanceFromEdgeAffectingChanceOfDripstoneColumn: number,
    public readonly maxDistanceFromCenterAffectingHeightBias: number,
  ) {}
}
