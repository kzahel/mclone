import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { WorldGenLevel } from "../world-gen-level";
import type { BlockGetter } from "../block-getter";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";
import { BooleanProperty } from "./state/properties/boolean-property";

export class VineBlock extends Block {
  public static readonly UP = BlockStateProperties.UP;
  public static readonly NORTH = BlockStateProperties.NORTH;
  public static readonly EAST = BlockStateProperties.EAST;
  public static readonly SOUTH = BlockStateProperties.SOUTH;
  public static readonly WEST = BlockStateProperties.WEST;
  public static readonly PROPERTY_BY_DIRECTION = new Map<Direction, BooleanProperty>([
    [Direction.UP, VineBlock.UP],
    [Direction.NORTH, VineBlock.NORTH],
    [Direction.EAST, VineBlock.EAST],
    [Direction.SOUTH, VineBlock.SOUTH],
    [Direction.WEST, VineBlock.WEST],
  ]);

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(
      this.stateDefinition
        .any()
        .setValue(VineBlock.UP, false)
        .setValue(VineBlock.NORTH, false)
        .setValue(VineBlock.EAST, false)
        .setValue(VineBlock.SOUTH, false)
        .setValue(VineBlock.WEST, false),
    );
  }

  public static getPropertyForFace(direction: Direction): BooleanProperty {
    const property = VineBlock.PROPERTY_BY_DIRECTION.get(direction);
    if (property === undefined) {
      throw new Error(`No vine property for face ${direction}`);
    }

    return property;
  }

  public static isAcceptableNeighbour(level: BlockGetter, pos: BlockPos, _direction: Direction): boolean {
    return level.getBlockState(pos).canOcclude();
  }

  public override canSurvive(state: BlockState, level: WorldGenLevel, pos: BlockPos): boolean {
    for (const direction of VineBlock.PROPERTY_BY_DIRECTION.keys()) {
      if (state.getValue(VineBlock.getPropertyForFace(direction)) && this.canSupportAtFace(level, pos, direction)) {
        return true;
      }
    }

    return false;
  }

  private canSupportAtFace(level: BlockGetter, pos: BlockPos, direction: Direction): boolean {
    if (direction === Direction.DOWN) {
      return false;
    }

    if (direction === Direction.UP) {
      return VineBlock.isAcceptableNeighbour(level, pos.above(), Direction.DOWN);
    }

    const neighborPos = pos.relative(direction);
    if (VineBlock.isAcceptableNeighbour(level, neighborPos, direction)) {
      return true;
    }

    const aboveState = level.getBlockState(pos.above());
    return aboveState.getBlock() === this && aboveState.getValue(VineBlock.getPropertyForFace(direction));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(VineBlock.UP, VineBlock.NORTH, VineBlock.EAST, VineBlock.SOUTH, VineBlock.WEST);
  }
}
