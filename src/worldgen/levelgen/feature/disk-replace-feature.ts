import { BaseDiskFeature } from "./base-disk-feature";
import { Fluids } from "../../../world/level/material/fluids";
import type { DiskConfiguration } from "./configurations/disk-configuration";
import type { FeaturePlaceContext } from "./feature-place-context";

function isWaterFluid(context: FeaturePlaceContext<DiskConfiguration>): boolean {
  const fluid = context.level().getFluidState(context.origin()).getType();
  return fluid.isSame(Fluids.WATER) || fluid.isSame(Fluids.FLOWING_WATER);
}

export class DiskReplaceFeature extends BaseDiskFeature {
  public override place(context: FeaturePlaceContext<DiskConfiguration>): boolean {
    return isWaterFluid(context) && super.place(context);
  }
}
