import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import type { BlockGetter } from "../block-getter";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluids } from "../material/fluids";
import type { FluidState } from "../material/fluid-state";
import { BushBlock } from "./bush-block";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { BlockState } from "./state/block-state";
import { StateDefinition } from "./state/state-definition";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { IntegerProperty } from "./state/properties/integer-property";
import { BooleanProperty } from "./state/properties/boolean-property";

const AIR_LOCATION = new ResourceLocation("minecraft:air");

export class SeaPickleBlock extends BushBlock {
  public static readonly PICKLES: IntegerProperty = BlockStateProperties.PICKLES;
  public static readonly WATERLOGGED: BooleanProperty = BlockStateProperties.WATERLOGGED;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(SeaPickleBlock.PICKLES, 1).setValue(SeaPickleBlock.WATERLOGGED, true));
  }

  protected override mayPlaceOn(state: BlockState, level: BlockGetter, pos: BlockPos): boolean {
    return state.isFaceSturdy(level, pos, Direction.UP);
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    const belowPos = pos.below();
    return this.mayPlaceOn(level.getBlockState(belowPos), level, belowPos);
  }

  public override updateShape(
    state: BlockState,
    direction: Direction,
    neighborState: BlockState,
    level: WorldGenLevel,
    pos: BlockPos,
    neighborPos: BlockPos,
  ): BlockState {
    if (!this.canSurvive(state, level, pos)) {
      return this.airState();
    }

    if (state.getValue(SeaPickleBlock.WATERLOGGED)) {
      level.getLiquidTicks().scheduleTick(pos, Fluids.WATER, 1);
    }

    return super.updateShape(state, direction, neighborState, level, pos, neighborPos);
  }

  public override getFluidState(state: BlockState): FluidState {
    return state.getValue(SeaPickleBlock.WATERLOGGED) ? Fluids.WATER.defaultFluidState() : super.getFluidState(state);
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(SeaPickleBlock.PICKLES, SeaPickleBlock.WATERLOGGED);
  }

  private airState(): BlockState {
    const airBlock = Registry.BLOCK.get(AIR_LOCATION) as Block | undefined;
    if (airBlock === undefined) {
      throw new Error("Missing registered block minecraft:air");
    }

    return airBlock.defaultBlockState();
  }
}
