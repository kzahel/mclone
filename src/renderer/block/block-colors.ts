import type { BlockPos } from "../../core/block-pos";
import type { BlockAndTintGetter } from "../../world/level/block-and-tint-getter";
import type { Block } from "../../world/level/block/block";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { Property } from "../../world/level/block/state/properties/property";

export type BlockColor = (state: BlockState, level: BlockAndTintGetter | null, pos: BlockPos | null, tintIndex: number) => number;

export class BlockColors {
  private readonly blockColors = new WeakMap<Block, BlockColor>();
  private readonly coloringStates = new WeakMap<Block, ReadonlySet<Property<unknown>>>();

  public getColor(state: BlockState, level: BlockAndTintGetter | null, pos: BlockPos | null, tintIndex: number): number {
    const blockColor = this.blockColors.get(state.getBlock());
    return blockColor === undefined ? -1 : blockColor(state, level, pos, tintIndex);
  }

  public register(color: BlockColor, ...blocks: readonly Block[]): void {
    for (const block of blocks) {
      this.blockColors.set(block, color);
    }
  }

  public getColoringProperties(block: Block): ReadonlySet<Property<unknown>> {
    return this.coloringStates.get(block) ?? new Set<Property<unknown>>();
  }
}
