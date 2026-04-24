import { Direction } from "../../../core/direction";
import type { BlockGetter } from "../block-getter";
import { Fluids } from "../material/fluids";
import type { WorldGenLevel } from "../world-gen-level";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { BlockState } from "./state/block-state";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { DripstoneThickness, type DripstoneThickness as DripstoneThicknessValue } from "./state/properties/dripstone-thickness";
import { DirectionProperty } from "./state/properties/direction-property";
import { EnumProperty } from "./state/properties/enum-property";
import { StateDefinition } from "./state/state-definition";

export class PointedDripstoneBlock extends Block {
  public static readonly TIP_DIRECTION: DirectionProperty = BlockStateProperties.VERTICAL_DIRECTION;
  public static readonly THICKNESS: EnumProperty<DripstoneThicknessValue> = BlockStateProperties.DRIPSTONE_THICKNESS;
  public static readonly WATERLOGGED = BlockStateProperties.WATERLOGGED;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(
      this.stateDefinition
        .any()
        .setValue(PointedDripstoneBlock.TIP_DIRECTION, Direction.UP)
        .setValue(PointedDripstoneBlock.THICKNESS, DripstoneThickness.TIP)
        .setValue(PointedDripstoneBlock.WATERLOGGED, false),
    );
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(PointedDripstoneBlock.TIP_DIRECTION, PointedDripstoneBlock.THICKNESS, PointedDripstoneBlock.WATERLOGGED);
  }

  public override canSurvive(state: BlockState, level: WorldGenLevel, pos: import("../../../core/block-pos").BlockPos): boolean {
    const direction = state.getValue(PointedDripstoneBlock.TIP_DIRECTION);
    const supportState = level.getBlockState(pos.relative(direction.getOpposite()));
    return supportState.canOcclude() || (supportState.is(this) && supportState.getValue(PointedDripstoneBlock.TIP_DIRECTION) === direction);
  }

  public override getFluidState(state: BlockState) {
    return state.getValue(PointedDripstoneBlock.WATERLOGGED) ? Fluids.WATER.defaultFluidState() : super.getFluidState(state);
  }

  public override propagatesSkylightDown(state: BlockState, _level: BlockGetter, _pos: import("../../../core/block-pos").BlockPos): boolean {
    return state.getFluidState().isEmpty();
  }
}
