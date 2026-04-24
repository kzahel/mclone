import { Direction } from "../../../../core/direction";
import type { Block } from "../../../../world/level/block/block";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import type { FeatureConfiguration } from "./feature-configuration";

export class GlowLichenConfiguration implements FeatureConfiguration {
  public readonly validDirections: readonly Direction[];

  public constructor(
    public readonly searchRange: number,
    public readonly canPlaceOnFloor: boolean,
    public readonly canPlaceOnCeiling: boolean,
    public readonly canPlaceOnWall: boolean,
    public readonly chanceOfSpreading: number,
    public readonly canBePlacedOnStates: readonly BlockState[],
  ) {
    const directions: Direction[] = [];
    if (canPlaceOnCeiling) {
      directions.push(Direction.UP);
    }

    if (canPlaceOnFloor) {
      directions.push(Direction.DOWN);
    }

    if (canPlaceOnWall) {
      directions.push(...Direction.Plane.HORIZONTAL.stream());
    }

    this.validDirections = directions;
  }

  public canBePlacedOn(block: Block): boolean {
    return this.canBePlacedOnStates.some((state) => state.is(block));
  }
}
