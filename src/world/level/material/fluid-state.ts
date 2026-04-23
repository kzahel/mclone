import { BlockPos } from "../../../core/block-pos";
import type { BlockGetter } from "../block-getter";
import type { Fluid } from "./fluid";
import { Vec3 } from "../../phys/vec3";
import type { BlockState } from "../block/state/block-state";

export class FluidState {
  public constructor(private readonly owner: Fluid) {}

  public getType(): Fluid {
    return this.owner;
  }

  public isSource(): boolean {
    return this.owner.isSource(this);
  }

  public isEmpty(): boolean {
    return this.owner.isEmpty();
  }

  public getHeight(level: BlockGetter, pos: BlockPos): number {
    return this.owner.getHeight(this, level, pos);
  }

  public getOwnHeight(): number {
    return this.owner.getOwnHeight(this);
  }

  public getAmount(): number {
    return this.owner.getAmount(this);
  }

  public shouldRenderBackwardUpFace(level: BlockGetter, pos: BlockPos): boolean {
    for (let offsetZ = -1; offsetZ <= 1; offsetZ++) {
      for (let offsetX = -1; offsetX <= 1; offsetX++) {
        const samplePos = pos.offset(offsetX, 0, offsetZ);
        const sampleFluid = level.getFluidState(samplePos);
        if (!sampleFluid.getType().isSame(this.getType()) && !level.getBlockState(samplePos).isSolidRender(level, samplePos)) {
          return true;
        }
      }
    }

    return false;
  }

  public getFlow(level: BlockGetter, pos: BlockPos): Vec3 {
    return this.owner.getFlow(level, pos, this);
  }

  public createLegacyBlock(): BlockState {
    return this.owner.createLegacyBlock();
  }
}
