import { RenderShape } from "./render-shape";
import { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";

export class AirBlock extends Block {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  public override getRenderShape(_state: BlockState): RenderShape {
    return RenderShape.INVISIBLE;
  }
}
