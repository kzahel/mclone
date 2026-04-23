import type { Block } from "../../../../world/level/block/block";
import type { FluidState } from "../../../../world/level/material/fluid-state";
import type { FeatureConfiguration } from "./feature-configuration";

export class SpringConfiguration implements FeatureConfiguration {
  public constructor(
    public readonly state: FluidState,
    public readonly requiresBlockBelow: boolean,
    public readonly rockCount: number,
    public readonly holeCount: number,
    public readonly validBlocks: ReadonlySet<Block>,
  ) {}
}
