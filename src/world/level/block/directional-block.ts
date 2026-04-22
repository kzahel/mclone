import { BlockStateProperties } from "./state/properties/block-state-properties";
import { DirectionProperty } from "./state/properties/direction-property";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";

export abstract class DirectionalBlock extends Block {
  public static readonly FACING: DirectionProperty = BlockStateProperties.FACING;

  protected constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }
}
