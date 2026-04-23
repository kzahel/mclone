import { Registry } from "../../../core/registry";
import { ResourceLocation } from "../../../core/resource-location";
import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { BlockGetter } from "../block-getter";
import type { WorldGenLevel } from "../world-gen-level";
import { Block } from "./block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";
import { StateDefinition } from "./state/state-definition";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import type { FluidState } from "../material/fluid-state";
import { Fluids } from "../material/fluids";

const AIR_LOCATION = new ResourceLocation("minecraft:air");

function scanForWater(state: BlockState, level: BlockGetter, pos: BlockPos): boolean {
  if (state.getValue(BaseCoralPlantTypeBlock.WATERLOGGED)) {
    return true;
  }

  for (const direction of Direction.values()) {
    if (level.getFluidState(pos.relative(direction)).getType().isSame(Fluids.WATER)) {
      return true;
    }
  }

  return false;
}

export class BaseCoralPlantTypeBlock extends Block {
  public static readonly WATERLOGGED = BlockStateProperties.WATERLOGGED;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.stateDefinition.any().setValue(BaseCoralPlantTypeBlock.WATERLOGGED, true));
  }

  protected tryScheduleDieTick(state: BlockState, level: WorldGenLevel, pos: BlockPos): void {
    if (!scanForWater(state, level, pos)) {
      level.getBlockTicks().scheduleTick(pos, this, 60);
    }
  }

  public override updateShape(
    state: BlockState,
    direction: Direction,
    neighborState: BlockState,
    level: WorldGenLevel,
    pos: BlockPos,
    neighborPos: BlockPos,
  ): BlockState {
    if (state.getValue(BaseCoralPlantTypeBlock.WATERLOGGED)) {
      level.getLiquidTicks().scheduleTick(pos, Fluids.WATER, 1);
    }

    return direction === Direction.DOWN && !this.canSurvive(state, level, pos)
      ? this.airState()
      : super.updateShape(state, direction, neighborState, level, pos, neighborPos);
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    const belowPos = pos.below();
    return level.getBlockState(belowPos).isFaceSturdy(level, belowPos, Direction.UP);
  }

  public override getFluidState(state: BlockState): FluidState {
    return state.getValue(BaseCoralPlantTypeBlock.WATERLOGGED) ? Fluids.WATER.defaultFluidState() : super.getFluidState(state);
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(BaseCoralPlantTypeBlock.WATERLOGGED);
  }

  protected airState(): BlockState {
    const airBlock = Registry.BLOCK.get(AIR_LOCATION) as Block | undefined;
    if (airBlock === undefined) {
      throw new Error("Missing registered block minecraft:air");
    }

    return airBlock.defaultBlockState();
  }
}
