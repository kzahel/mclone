import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";

export class HugeMushroomBlock extends Block {
  public static readonly NORTH = BlockStateProperties.NORTH;
  public static readonly EAST = BlockStateProperties.EAST;
  public static readonly SOUTH = BlockStateProperties.SOUTH;
  public static readonly WEST = BlockStateProperties.WEST;
  public static readonly UP = BlockStateProperties.UP;
  public static readonly DOWN = BlockStateProperties.DOWN;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(
      this.stateDefinition
        .any()
        .setValue(HugeMushroomBlock.NORTH, true)
        .setValue(HugeMushroomBlock.EAST, true)
        .setValue(HugeMushroomBlock.SOUTH, true)
        .setValue(HugeMushroomBlock.WEST, true)
        .setValue(HugeMushroomBlock.UP, true)
        .setValue(HugeMushroomBlock.DOWN, true),
    );
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(
      HugeMushroomBlock.UP,
      HugeMushroomBlock.DOWN,
      HugeMushroomBlock.NORTH,
      HugeMushroomBlock.EAST,
      HugeMushroomBlock.SOUTH,
      HugeMushroomBlock.WEST,
    );
  }
}
