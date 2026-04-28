import type { BlockPos } from "../../../core/block-pos";
import { Block } from "./block";
import type { BlockGetter } from "../block-getter";
import { BlockBehaviour } from "./state/block-behaviour";
import { StateDefinition } from "./state/state-definition";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { EnumProperty } from "./state/properties/enum-property";
import { SlabType } from "./state/properties/slab-type";
import type { BlockState } from "./state/block-state";
import { Shapes, type VoxelShape } from "../../phys/shapes/voxel-shape";
import { Fluids } from "../material/fluids";
import { PathComputationType, type PathComputationType as PathComputationTypeValue } from "../pathfinder/path-computation-type";

const BOTTOM_SHAPE = Shapes.box(0.0, 0.0, 0.0, 1.0, 0.5, 1.0);
const TOP_SHAPE = Shapes.box(0.0, 0.5, 0.0, 1.0, 1.0, 1.0);

export class SlabBlock extends Block {
  public static readonly TYPE: EnumProperty<SlabType> = BlockStateProperties.SLAB_TYPE;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.defaultBlockState().setValue(SlabBlock.TYPE, SlabType.BOTTOM));
  }

  public override useShapeForLightOcclusion(state: BlockState): boolean {
    return state.getValue(SlabBlock.TYPE) !== SlabType.DOUBLE;
  }

  public override getShape(state: BlockState, _level: BlockGetter, _pos: BlockPos): VoxelShape {
    const type = state.getValue(SlabBlock.TYPE);
    if (type === SlabType.DOUBLE) {
      return Shapes.block();
    }
    return type === SlabType.TOP ? TOP_SHAPE : BOTTOM_SHAPE;
  }

  public override isPathfindable(_state: BlockState, level: BlockGetter, pos: BlockPos, type: PathComputationTypeValue): boolean {
    switch (type) {
      case PathComputationType.LAND:
      case PathComputationType.AIR:
        return false;
      case PathComputationType.WATER:
        return level.getFluidState(pos).getType().isSame(Fluids.WATER);
    }
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(SlabBlock.TYPE);
  }
}
