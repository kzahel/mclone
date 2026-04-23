import { Direction } from "../../../core/direction";
import type { BlockState } from "./state/block-state";
import { StateDefinition } from "./state/state-definition";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { Block } from "./block";
import { RenderShape } from "./render-shape";
import { BlockBehaviour } from "./state/block-behaviour";
import type { Fluid } from "../material/fluid";
import type { FluidState } from "../material/fluid-state";

export class LiquidBlock extends Block {
  public static readonly LEVEL = BlockStateProperties.LEVEL;

  public constructor(
    protected readonly fluid: Fluid,
    properties: BlockBehaviour.Properties,
  ) {
    super(properties);
    this.registerDefaultState(this.getStateDefinition().any().setValue(LiquidBlock.LEVEL, 0));
  }

  public override getFluidState(_state: BlockState): FluidState {
    return this.fluid.defaultFluidState();
  }

  public override skipRendering(_state: BlockState, adjacentState: BlockState, _direction: Direction): boolean {
    return adjacentState.getFluidState().getType().isSame(this.fluid);
  }

  public override getRenderShape(_state: BlockState): RenderShape {
    return RenderShape.INVISIBLE;
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(LiquidBlock.LEVEL);
  }
}
