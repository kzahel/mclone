import { Direction } from "../../../core/direction";
import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "./state/block-state";
import { StateDefinition } from "./state/state-definition";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { Block } from "./block";
import { RenderShape } from "./render-shape";
import { BlockBehaviour } from "./state/block-behaviour";
import type { FluidState } from "../material/fluid-state";
import type { FlowingFluid } from "../material/flowing-fluid";
import type { WorldGenLevel } from "../world-gen-level";
import type { BlockGetter } from "../block-getter";
import { Fluids } from "../material/fluids";
import { PathComputationType, type PathComputationType as PathComputationTypeValue } from "../pathfinder/path-computation-type";

export class LiquidBlock extends Block {
  public static readonly LEVEL = BlockStateProperties.LEVEL;
  private readonly stateCache: FluidState[];

  public constructor(
    protected readonly fluid: FlowingFluid,
    properties: BlockBehaviour.Properties,
  ) {
    super(properties);
    this.stateCache = [this.fluid.getSourceState(false)];
    for (let level = 1; level < 8; level++) {
      this.stateCache.push(this.fluid.getFlowingState(8 - level, false));
    }
    this.stateCache.push(this.fluid.getFlowingState(8, true));
    this.registerDefaultState(this.getStateDefinition().any().setValue(LiquidBlock.LEVEL, 0));
  }

  public override getFluidState(state: BlockState): FluidState {
    return this.stateCache[Math.min(state.getValue(LiquidBlock.LEVEL), 8)]!;
  }

  public override propagatesSkylightDown(_state: BlockState, _level: BlockGetter, _pos: BlockPos): boolean {
    return false;
  }

  public override skipRendering(_state: BlockState, adjacentState: BlockState, _direction: Direction): boolean {
    return adjacentState.getFluidState().getType().isSame(this.fluid);
  }

  public override getRenderShape(_state: BlockState): RenderShape {
    return RenderShape.INVISIBLE;
  }

  public override isPathfindable(_state: BlockState, level: BlockGetter, pos: BlockPos, type: PathComputationTypeValue): boolean {
    switch (type) {
      case PathComputationType.LAND:
      case PathComputationType.WATER:
        return !this.fluid.isSame(Fluids.LAVA) && level.getFluidState(pos).getType().isSame(this.fluid);
      case PathComputationType.AIR:
        return false;
    }
  }

  public override onPlace(state: BlockState, level: WorldGenLevel, pos: BlockPos, _oldState: BlockState, _movedByPiston: boolean): void {
    level.getLiquidTicks().scheduleTick(pos, state.getFluidState().getType(), this.fluid.getTickDelay(level));
  }

  public override updateShape(
    state: BlockState,
    _direction: Direction,
    neighborState: BlockState,
    level: WorldGenLevel,
    pos: BlockPos,
    _neighborPos: BlockPos,
  ): BlockState {
    if (state.getFluidState().isSource() || neighborState.getFluidState().isSource()) {
      level.getLiquidTicks().scheduleTick(pos, state.getFluidState().getType(), this.fluid.getTickDelay(level));
    }

    return super.updateShape(state, _direction, neighborState, level, pos, _neighborPos);
  }

  public override neighborChanged(state: BlockState, level: WorldGenLevel, pos: BlockPos, _block: Block, _neighborPos: BlockPos, _movedByPiston: boolean): void {
    level.getLiquidTicks().scheduleTick(pos, state.getFluidState().getType(), this.fluid.getTickDelay(level));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(LiquidBlock.LEVEL);
  }
}
