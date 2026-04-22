import type { Block } from "../world/level/block/block";
import type { BlockState } from "../world/level/block/state/block-state";
import { RenderType } from "./render-type";

const TYPE_BY_BLOCK = new Map<Block, RenderType>();

export class ItemBlockRenderTypes {
  public static setRenderLayer(block: Block, renderType: RenderType): void {
    TYPE_BY_BLOCK.set(block, renderType);
  }

  public static getChunkRenderType(state: BlockState): RenderType {
    return TYPE_BY_BLOCK.get(state.getBlock()) ?? RenderType.solid();
  }
}
