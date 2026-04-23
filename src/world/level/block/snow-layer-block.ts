import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { ResourceLocation } from "../../../core/resource-location";
import type { BlockGetter } from "../block-getter";
import type { WorldGenLevel } from "../world-gen-level";
import type { BlockState } from "./state/block-state";
import { StateDefinition } from "./state/state-definition";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";

const ICE_LOCATION = new ResourceLocation("minecraft:ice");
const PACKED_ICE_LOCATION = new ResourceLocation("minecraft:packed_ice");

export class SnowLayerBlock extends Block {
  public static readonly LAYERS = BlockStateProperties.LAYERS;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(this.getStateDefinition().any().setValue(SnowLayerBlock.LAYERS, 1));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(SnowLayerBlock.LAYERS);
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    const belowPos = pos.below();
    const belowState = level.getBlockState(belowPos);
    const belowLocation = belowState.getBlock().getLocation()?.toString();
    if (belowLocation === ICE_LOCATION.toString() || belowLocation === PACKED_ICE_LOCATION.toString()) {
      return false;
    }

    return belowState.isFaceSturdy(level, belowPos, Direction.UP) || (belowState.is(this) && belowState.getValue(SnowLayerBlock.LAYERS) === 8);
  }

  public override isSolidRender(_state: BlockState, _level: BlockGetter): boolean {
    return false;
  }

  public override useShapeForLightOcclusion(_state: BlockState): boolean {
    return true;
  }
}
