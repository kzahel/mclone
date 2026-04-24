import { Direction } from "../../../core/direction";
import type { BlockPos } from "../../../core/block-pos";
import type { BlockGetter } from "../block-getter";
import type { BlockState } from "../block/state/block-state";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluid } from "./fluid";
import type { FluidState } from "./fluid-state";
import { FlowingFluid } from "./flowing-fluid";
import { WaterFluid } from "./water-fluid";

class EmptyFluid extends Fluid {
  public constructor() {
    super();
  }

  public override isEmpty(): boolean {
    return true;
  }

  public override isSource(_state: FluidState): boolean {
    return false;
  }

  public override getAmount(_state: FluidState): number {
    return 0;
  }

  public override getOwnHeight(_state: FluidState): number {
    return 0.0;
  }

  public override canBeReplacedWith(_state: FluidState, _level: BlockGetter, _pos: BlockPos, _fluid: Fluid, _direction: Direction): boolean {
    return true;
  }
}

class LavaFluid extends FlowingFluid {
  public constructor(private readonly source: boolean) {
    super();
  }

  public override getFlowing(): Fluid {
    return Fluids.FLOWING_LAVA;
  }

  public override getSource(): Fluid {
    return Fluids.LAVA;
  }

  protected override canConvertToSource(): boolean {
    return false;
  }

  protected override beforeDestroyingBlock(_level: WorldGenLevel, _pos: BlockPos, _state: BlockState): void {}

  public override getSlopeFindDistance(_level: WorldGenLevel): number {
    return 2;
  }

  public override getDropOff(_level: WorldGenLevel): number {
    return 2;
  }

  public override getTickDelay(_level: WorldGenLevel): number {
    return 30;
  }

  public override canBeReplacedWith(_state: FluidState, _level: BlockGetter, _pos: BlockPos, fluid: Fluid, direction: Direction): boolean {
    return direction === Direction.DOWN && !fluid.isSame(this);
  }

  public override getAmount(state: FluidState): number {
    return this.source ? 8 : (state.getAmountValue() ?? 8);
  }

  public override isSource(_state: FluidState): boolean {
    return this.source;
  }
}

export class Fluids {
  public static readonly EMPTY: Fluid = new EmptyFluid();
  public static readonly FLOWING_WATER: FlowingFluid = new WaterFluid.Flowing();
  public static readonly WATER: FlowingFluid = new WaterFluid.Source();
  public static readonly FLOWING_LAVA: FlowingFluid = new LavaFluid(false);
  public static readonly LAVA: FlowingFluid = new LavaFluid(true);
}
