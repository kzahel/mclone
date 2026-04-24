import type { BlockGetter } from "../block-getter";
import { Fluids } from "../material/fluids";
import { MultifaceBlock } from "./multiface-block";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { StateDefinition } from "./state/state-definition";
import { Block } from "./block";

export class GlowLichenBlock extends MultifaceBlock {
  public static readonly WATERLOGGED = BlockStateProperties.WATERLOGGED;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.defaultBlockState().setValue(GlowLichenBlock.WATERLOGGED, false));
  }

  public static emission(lightEmission: number): (state: BlockState) => number {
    return (state) => (MultifaceBlock.hasAnyFace(state) ? lightEmission : 0);
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    super.createBlockStateDefinition(builder);
    builder.add(GlowLichenBlock.WATERLOGGED);
  }

  public override getFluidState(state: BlockState) {
    return state.getValue(GlowLichenBlock.WATERLOGGED) ? Fluids.WATER.defaultFluidState() : super.getFluidState(state);
  }

  public override propagatesSkylightDown(state: BlockState, _level: BlockGetter, _pos: import("../../../core/block-pos").BlockPos): boolean {
    return state.getFluidState().isEmpty();
  }
}
