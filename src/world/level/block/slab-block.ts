import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import { StateDefinition } from "./state/state-definition";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { EnumProperty } from "./state/properties/enum-property";
import { SlabType } from "./state/properties/slab-type";
import type { BlockState } from "./state/block-state";

export class SlabBlock extends Block {
  public static readonly TYPE: EnumProperty<SlabType> = BlockStateProperties.SLAB_TYPE;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.defaultBlockState().setValue(SlabBlock.TYPE, SlabType.BOTTOM));
  }

  public override useShapeForLightOcclusion(state: BlockState): boolean {
    return state.getValue(SlabBlock.TYPE) !== SlabType.DOUBLE;
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(SlabBlock.TYPE);
  }
}
