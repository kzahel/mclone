import type { BlockGetter } from "../block-getter";
import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { Vec3 } from "../../phys/vec3";
import { FluidState } from "./fluid-state";
import type { BlockState } from "../block/state/block-state";
import type { WorldGenLevel } from "../world-gen-level";

export abstract class Fluid {
  private readonly defaultFluidStateValue: FluidState;
  private legacyBlockValue: BlockState | undefined;

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

  public canBeReplacedWith(_state: FluidState, _level: BlockGetter, _pos: BlockPos, _fluid: Fluid, _direction: Direction): boolean {
    return false;
  }

  public getTickDelay(_level: WorldGenLevel): number {
    return 0;
  }

  public tick(_level: WorldGenLevel, _pos: BlockPos, _state: FluidState): void {}

  public setLegacyBlock(state: BlockState): void {
    this.legacyBlockValue = state;
  }

  public createLegacyBlock(_state: FluidState): BlockState {
    if (this.legacyBlockValue === undefined) {
      throw new Error(`No legacy block has been registered for fluid ${this.constructor.name}`);
    }

    return this.legacyBlockValue;
  }
}
