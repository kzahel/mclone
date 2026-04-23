import { BlockPos } from "../../core/block-pos";
import { Direction } from "../../core/direction";
import type { BlockAndTintGetter } from "./block-and-tint-getter";
import type { ColorResolver } from "./color-resolver";
import { LightLayer } from "./light-layer";
import { type BlockState } from "./block/state/block-state";
import type { FluidState } from "./material/fluid-state";

export class StaticBlockAndTintGetter implements BlockAndTintGetter {
  private readonly states = new Map<bigint, BlockState>();

  public constructor(
    private readonly airState: BlockState,
    private readonly skyLight = 15,
    private readonly blockLight = 15,
  ) {}

  public setBlock(pos: BlockPos, state: BlockState): void {
    this.states.set(pos.asLong(), state);
  }

  public getBlockState(pos: BlockPos): BlockState {
    return this.states.get(pos.asLong()) ?? this.airState;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public getMaxLightLevel(): number {
    return 15;
  }

  public getShade(direction: Direction, shade: boolean): number {
    if (!shade) {
      return 1.0;
    }

    switch (direction) {
      case Direction.DOWN:
        return 0.5;
      case Direction.UP:
        return 1.0;
      case Direction.NORTH:
      case Direction.SOUTH:
        return 0.8;
      case Direction.WEST:
      case Direction.EAST:
        return 0.6;
      default:
        return 1.0;
    }
  }

  public getBrightness(layer: LightLayer, _pos: BlockPos): number {
    return layer === LightLayer.SKY ? this.skyLight : this.blockLight;
  }

  public getBlockTint(_pos: BlockPos, _resolver?: ColorResolver): number {
    return -1;
  }
}
