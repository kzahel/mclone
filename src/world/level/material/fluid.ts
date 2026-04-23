import type { BlockGetter } from "../block-getter";
import { BlockPos } from "../../../core/block-pos";
import { Vec3 } from "../../phys/vec3";
import { FluidState } from "./fluid-state";

export abstract class Fluid {
  private readonly defaultFluidStateValue: FluidState;

  protected constructor() {
    this.defaultFluidStateValue = new FluidState(this);
  }

  public defaultFluidState(): FluidState {
    return this.defaultFluidStateValue;
  }

  public isSame(other: Fluid): boolean {
    return this === other;
  }

  public isEmpty(): boolean {
    return false;
  }

  public isSource(_state: FluidState): boolean {
    return true;
  }

  public getHeight(state: FluidState, _level: BlockGetter, _pos: BlockPos): number {
    return this.getOwnHeight(state);
  }

  public getOwnHeight(state: FluidState): number {
    return state.isSource() ? 1.0 : state.getAmount() / 9.0;
  }

  public getAmount(_state: FluidState): number {
    return 8;
  }

  public getFlow(_level: BlockGetter, _pos: BlockPos, _state: FluidState): Vec3 {
    return Vec3.ZERO;
  }
}
