import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { ResourceLocation } from "../../../core/resource-location";
import type { BlockGetter } from "../block-getter";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluids } from "../material/fluids";
import type { FluidState } from "../material/fluid-state";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import type { BlockState } from "./state/block-state";

const KELP_LOCATION = new ResourceLocation("minecraft:kelp");
const KELP_PLANT_LOCATION = new ResourceLocation("minecraft:kelp_plant");
const MAGMA_BLOCK_LOCATION = new ResourceLocation("minecraft:magma_block");

function isAttachable(state: BlockState, level: BlockGetter, pos: BlockPos): boolean {
  const location = state.getBlock().getLocation()?.toString();
  return (
    location === KELP_LOCATION.toString() ||
    location === KELP_PLANT_LOCATION.toString() ||
    (state.isFaceSturdy(level, pos, Direction.UP) && location !== MAGMA_BLOCK_LOCATION.toString())
  );
}

export class KelpPlantBlock extends Block {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  public override canSurvive(_state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    const fluidState = level.getFluidState(pos);
    return fluidState.getType().isSame(Fluids.WATER) && fluidState.getAmount() === 8 && isAttachable(level.getBlockState(pos.below()), level, pos.below());
  }

  public override getFluidState(_state: BlockState): FluidState {
    return Fluids.WATER.defaultFluidState();
  }
}
