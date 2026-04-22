import { Direction } from "../../../core/direction";
import type { BlockPlaceContext } from "../../item/context/block-place-context";
import { type Rotation } from "./rotation";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { EnumProperty } from "./state/properties/enum-property";
import { StateDefinition } from "./state/state-definition";
import { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";

export class RotatedPillarBlock extends Block {
  public static readonly AXIS: EnumProperty<Direction.Axis> = BlockStateProperties.AXIS;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.defaultBlockState().setValue(RotatedPillarBlock.AXIS, Direction.Axis.Y));
  }

  public override rotate(state: BlockState, rotation: Rotation): BlockState {
    return RotatedPillarBlock.rotatePillar(state, rotation);
  }

  public static rotatePillar(state: BlockState, rotation: Rotation): BlockState {
    switch (rotation) {
      case "counterclockwise_90":
      case "clockwise_90":
        switch (state.getValue(RotatedPillarBlock.AXIS)) {
          case Direction.Axis.X:
            return state.setValue(RotatedPillarBlock.AXIS, Direction.Axis.Z);
          case Direction.Axis.Z:
            return state.setValue(RotatedPillarBlock.AXIS, Direction.Axis.X);
          default:
            return state;
        }
      default:
        return state;
    }
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(RotatedPillarBlock.AXIS);
  }

  public override getStateForPlacement(context: BlockPlaceContext): BlockState | undefined {
    return this.defaultBlockState().setValue(RotatedPillarBlock.AXIS, context.getClickedFace().getAxis());
  }
}
