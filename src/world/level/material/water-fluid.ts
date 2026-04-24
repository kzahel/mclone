import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { BlockGetter } from "../block-getter";
import type { BlockState } from "../block/state/block-state";
import { BlockStateProperties } from "../block/state/properties/block-state-properties";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluid } from "./fluid";
import type { FluidState } from "./fluid-state";
import { FlowingFluid } from "./flowing-fluid";
import { Fluids } from "./fluids";

export abstract class WaterFluid extends FlowingFluid {
  public override getFlowing(): Fluid {
    return Fluids.FLOWING_WATER;
  }

  public override getSource(): Fluid {
    return Fluids.WATER;
  }

  protected override canConvertToSource(): boolean {
    return true;
  }

  protected override beforeDestroyingBlock(_level: WorldGenLevel, _pos: BlockPos, _state: BlockState): void {}

  public override getSlopeFindDistance(_level: WorldGenLevel): number {
    return 4;
  }

  public override createLegacyBlock(state: FluidState): BlockState {
    return super.createLegacyBlock(state).setValue(BlockStateProperties.LEVEL, FlowingFluid.getLegacyLevel(state));
  }

  public override isSame(fluid: Fluid): boolean {
    return fluid === Fluids.WATER || fluid === Fluids.FLOWING_WATER;
  }

  public override getDropOff(_level: WorldGenLevel): number {
    return 1;
  }

  public override getTickDelay(_level: WorldGenLevel): number {
    return 5;
  }

  public override canBeReplacedWith(_state: FluidState, _level: BlockGetter, _pos: BlockPos, fluid: Fluid, direction: Direction): boolean {
    return direction === Direction.DOWN && !fluid.isSame(Fluids.WATER);
  }
}

export namespace WaterFluid {
  export class Flowing extends WaterFluid {
    public constructor() {
      super();
    }

    public override getAmount(state: FluidState): number {
      return state.getAmountValue() ?? 8;
    }

    public override isSource(_state: FluidState): boolean {
      return false;
    }
  }

  export class Source extends WaterFluid {
    public constructor() {
      super();
    }

    public override getAmount(_state: FluidState): number {
      return 8;
    }

    public override isSource(_state: FluidState): boolean {
      return true;
    }
  }
}
