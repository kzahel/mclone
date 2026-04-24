import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { SimpleRandomSource } from "../../../worldgen/prng/simple-random-source";
import { BlockPlaceContext } from "../../item/context/block-place-context";
import type { BlockGetter } from "../block-getter";
import { Fluids } from "../material/fluids";
import { Block } from "./block";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockStateProperties } from "./state/properties/block-state-properties";
import type { BooleanProperty } from "./state/properties/boolean-property";
import { StateDefinition } from "./state/state-definition";
import type { BlockState } from "./state/block-state";

const DIRECTIONS = Direction.values();

function shuffledDirections(directions: readonly Direction[], random: SimpleRandomSource): Direction[] {
  const shuffled = [...directions];
  for (let index = shuffled.length - 1; index > 0; index--) {
    const swapIndex = random.nextInt(index + 1);
    const value = shuffled[index]!;
    shuffled[index] = shuffled[swapIndex]!;
    shuffled[swapIndex] = value;
  }

  return shuffled;
}

export class MultifaceBlock extends Block {
  public static readonly PROPERTY_BY_DIRECTION = new Map<Direction, BooleanProperty>([
    [Direction.NORTH, BlockStateProperties.NORTH],
    [Direction.EAST, BlockStateProperties.EAST],
    [Direction.SOUTH, BlockStateProperties.SOUTH],
    [Direction.WEST, BlockStateProperties.WEST],
    [Direction.UP, BlockStateProperties.UP],
    [Direction.DOWN, BlockStateProperties.DOWN],
  ]);

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    this.registerDefaultState(MultifaceBlock.getDefaultMultifaceState(this.stateDefinition.any()));
  }

  protected isFaceSupported(_direction: Direction): boolean {
    return true;
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    for (const direction of DIRECTIONS) {
      if (this.isFaceSupported(direction)) {
        builder.add(MultifaceBlock.getFaceProperty(direction));
      }
    }
  }

  public override canBeReplaced(state: BlockState, _context: BlockPlaceContext): boolean {
    return DIRECTIONS.some((direction) => !MultifaceBlock.hasFace(state, direction));
  }

  public override canSurvive(state: BlockState, level: import("../world-gen-level").WorldGenLevel, pos: BlockPos): boolean {
    let supportedFace = false;

    for (const direction of DIRECTIONS) {
      if (!MultifaceBlock.hasFace(state, direction)) {
        continue;
      }

      const neighborPos = pos.relative(direction);
      if (!this.canAttachTo(level, direction, neighborPos, level.getBlockState(neighborPos))) {
        return false;
      }

      supportedFace = true;
    }

    return supportedFace;
  }

  public override getStateForPlacement(context: BlockPlaceContext): BlockState | undefined;
  public override getStateForPlacement(state: BlockState, level: BlockGetter, pos: BlockPos, direction: Direction): BlockState | undefined;
  public override getStateForPlacement(
    first: BlockPlaceContext | BlockState,
    second?: BlockGetter,
    third?: BlockPos,
    fourth?: Direction,
  ): BlockState | undefined {
    if (second !== undefined && third !== undefined && fourth !== undefined) {
      return this.getStateForPlacementWithState(first as BlockState, second, third, fourth);
    }

    const context = first as BlockPlaceContext;
    return this.defaultBlockState().setValue(MultifaceBlock.getFaceProperty(context.getClickedFace()), true);
  }

  private getStateForPlacementWithState(state: BlockState, level: BlockGetter, pos: BlockPos, direction: Direction): BlockState | undefined {
    if (!this.isFaceSupported(direction)) {
      return undefined;
    }

    let placedState: BlockState;
    if (state.is(this)) {
      if (MultifaceBlock.hasFace(state, direction)) {
        return undefined;
      }

      placedState = state;
    } else if (this.isWaterloggable() && state.getFluidState().isSource() && state.getFluidState().getType().isSame(Fluids.WATER)) {
      placedState = this.defaultBlockState().setValue(BlockStateProperties.WATERLOGGED, true);
    } else {
      placedState = this.defaultBlockState();
    }

    const neighborPos = pos.relative(direction);
    return this.canAttachTo(level, direction, neighborPos, level.getBlockState(neighborPos))
      ? placedState.setValue(MultifaceBlock.getFaceProperty(direction), true)
      : undefined;
  }

  protected canSpread(state: BlockState, level: BlockGetter, pos: BlockPos, direction: Direction): boolean {
    return DIRECTIONS.some((spreadDirection) => this.getSpreadFromFaceTowardDirection(state, level, pos, direction, spreadDirection) !== undefined);
  }

  public spreadFromFaceTowardRandomDirection(
    state: BlockState,
    level: BlockGetter & { setBlock(pos: BlockPos, state: BlockState, flags?: number): boolean },
    pos: BlockPos,
    direction: Direction,
    random: SimpleRandomSource,
    _markForPostProcessing: boolean,
  ): boolean {
    return shuffledDirections(DIRECTIONS, random).some((spreadDirection) =>
      this.spreadFromFaceTowardDirection(state, level, pos, direction, spreadDirection),
    );
  }

  public spreadFromFaceTowardDirection(
    state: BlockState,
    level: BlockGetter & { setBlock(pos: BlockPos, state: BlockState, flags?: number): boolean },
    pos: BlockPos,
    faceDirection: Direction,
    spreadDirection: Direction,
  ): boolean {
    const spread = this.getSpreadFromFaceTowardDirection(state, level, pos, faceDirection, spreadDirection);
    return spread !== undefined ? this.spreadToFace(level, spread.pos, spread.direction) : false;
  }

  public static getFaceProperty(direction: Direction): BooleanProperty {
    const property = MultifaceBlock.PROPERTY_BY_DIRECTION.get(direction);
    if (property === undefined) {
      throw new Error(`Missing multiface property for ${direction}`);
    }

    return property;
  }

  public static hasAnyFace(state: BlockState): boolean {
    return DIRECTIONS.some((direction) => MultifaceBlock.hasFace(state, direction));
  }

  private static hasFace(state: BlockState, direction: Direction): boolean {
    const property = MultifaceBlock.getFaceProperty(direction);
    return state.hasProperty(property) && state.getValue(property);
  }

  private static getDefaultMultifaceState(state: BlockState): BlockState {
    let current = state;
    for (const property of MultifaceBlock.PROPERTY_BY_DIRECTION.values()) {
      if (current.hasProperty(property)) {
        current = current.setValue(property, false);
      }
    }

    return current;
  }

  private canAttachTo(_level: BlockGetter, _direction: Direction, _neighborPos: BlockPos, neighborState: BlockState): boolean {
    return neighborState.canOcclude();
  }

  private isWaterloggable(): boolean {
    return this.stateDefinition.getProperties().includes(BlockStateProperties.WATERLOGGED);
  }

  private getSpreadFromFaceTowardDirection(
    state: BlockState,
    level: BlockGetter,
    pos: BlockPos,
    faceDirection: Direction,
    spreadDirection: Direction,
  ): { readonly pos: BlockPos; readonly direction: Direction } | undefined {
    if (spreadDirection.getAxis() === faceDirection.getAxis() || !MultifaceBlock.hasFace(state, faceDirection) || MultifaceBlock.hasFace(state, spreadDirection)) {
      return undefined;
    }

    if (this.canSpreadToFace(level, pos, spreadDirection)) {
      return { pos, direction: spreadDirection };
    }

    const spreadPos = pos.relative(spreadDirection);
    if (this.canSpreadToFace(level, spreadPos, faceDirection)) {
      return { pos: spreadPos, direction: faceDirection };
    }

    const diagonalPos = spreadPos.relative(faceDirection);
    const diagonalDirection = spreadDirection.getOpposite();
    return this.canSpreadToFace(level, diagonalPos, diagonalDirection)
      ? { pos: diagonalPos, direction: diagonalDirection }
      : undefined;
  }

  private canSpreadToFace(level: BlockGetter, pos: BlockPos, direction: Direction): boolean {
    const state = level.getBlockState(pos);
    if (!this.canSpreadInto(state)) {
      return false;
    }

    return this.getStateForPlacement(state, level, pos, direction) !== undefined;
  }

  private spreadToFace(
    level: BlockGetter & { setBlock(pos: BlockPos, state: BlockState, flags?: number): boolean },
    pos: BlockPos,
    direction: Direction,
  ): boolean {
    const state = level.getBlockState(pos);
    const placedState = this.getStateForPlacement(state, level, pos, direction);
    return placedState !== undefined ? level.setBlock(pos, placedState, 2) : false;
  }

  private canSpreadInto(state: BlockState): boolean {
    return state.isAir() || state.is(this) || (state.getFluidState().isSource() && state.getFluidState().getType().isSame(Fluids.WATER));
  }
}
