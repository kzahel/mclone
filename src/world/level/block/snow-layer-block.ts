import type { BlockGetter } from "../block-getter";
import type { BlockState } from "./state/block-state";
import { StateDefinition } from "./state/state-definition";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";

export class SnowLayerBlock extends Block {
  public static readonly LAYERS = BlockStateProperties.LAYERS;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.getStateDefinition().any().setValue(SnowLayerBlock.LAYERS, 1));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(SnowLayerBlock.LAYERS);
  }

  public override isSolidRender(_state: BlockState, _level: BlockGetter): boolean {
    return false;
  }
}
