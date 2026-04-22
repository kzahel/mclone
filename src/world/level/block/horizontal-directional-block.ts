import { getRotation, type Mirror } from "./mirror";
import { rotateDirection, type Rotation } from "./rotation";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { DirectionProperty } from "./state/properties/direction-property";
import { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";

export abstract class HorizontalDirectionalBlock extends Block {
  public static readonly FACING: DirectionProperty = BlockStateProperties.HORIZONTAL_FACING;

  protected constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  public override rotate(state: BlockState, rotation: Rotation): BlockState {
    return state.setValue(HorizontalDirectionalBlock.FACING, rotateDirection(rotation, state.getValue(HorizontalDirectionalBlock.FACING)));
  }

  public override mirror(state: BlockState, mirror: Mirror): BlockState {
    return this.rotate(state, getRotation(mirror, state.getValue(HorizontalDirectionalBlock.FACING)));
  }
}
