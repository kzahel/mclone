import type { Block } from "../world/level/block/block";
import type { BlockState } from "../world/level/block/state/block-state";
import type { Fluid } from "../world/level/material/fluid";
import type { FluidState } from "../world/level/material/fluid-state";
import { RenderType } from "./render-type";

const TYPE_BY_BLOCK = new Map<Block, RenderType>();
const TYPE_BY_FLUID = new Map<Fluid, RenderType>();

export class ItemBlockRenderTypes {
  public static setRenderLayer(block: Block, renderType: RenderType): void {
    TYPE_BY_BLOCK.set(block, renderType);
  }

  public static setFluidRenderLayer(fluid: Fluid, renderType: RenderType): void {
    TYPE_BY_FLUID.set(fluid, renderType);
  }

  public static getChunkRenderType(state: BlockState): RenderType {
    return TYPE_BY_BLOCK.get(state.getBlock()) ?? RenderType.solid();
  }

  public static getRenderLayer(state: FluidState): RenderType {
    return TYPE_BY_FLUID.get(state.getType()) ?? RenderType.solid();
  }
}
