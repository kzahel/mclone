import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { BlockGetter } from "../block-getter";
import type { BlockState } from "../block/state/block-state";
import { Material } from "./material";
import type { WorldGenLevel } from "../world-gen-level";
import { Fluid } from "./fluid";
import { FluidState } from "./fluid-state";
import { Fluids } from "./fluids";

const CACHE_COORD_OFFSET = 128;

function getCacheKey(origin: BlockPos, pos: BlockPos): number {
  const x = pos.getX() - origin.getX();
  const z = pos.getZ() - origin.getZ();
  return ((x + CACHE_COORD_OFFSET) & 0xff) << 8 | ((z + CACHE_COORD_OFFSET) & 0xff);
}

export abstract class FlowingFluid extends Fluid {
  public getFlowingState(amount: number, falling: boolean): FluidState {
    return new FluidState(this.getFlowing(), amount, falling);
  }

  public getSourceState(falling: boolean): FluidState {
    return falling ? new FluidState(this.getSource(), undefined, true) : this.getSource().defaultFluidState();
  }

  public abstract getFlowing(): Fluid;

  public abstract getSource(): Fluid;

  protected abstract canConvertToSource(): boolean;

  protected abstract beforeDestroyingBlock(level: WorldGenLevel, pos: BlockPos, state: BlockState): void;

  public abstract getSlopeFindDistance(level: WorldGenLevel): number;

  public abstract getDropOff(level: WorldGenLevel): number;

  protected spread(level: WorldGenLevel, pos: BlockPos, state: FluidState): void {
    if (!state.isEmpty()) {
      const blockState = level.getBlockState(pos);
      const belowPos = pos.below();
      const belowState = level.getBlockState(belowPos);
      const newBelowLiquid = this.getNewLiquid(level, belowPos, belowState);
      if (this.canSpreadTo(level, pos, blockState, Direction.DOWN, belowPos, belowState, level.getFluidState(belowPos), newBelowLiquid.getType())) {
        this.spreadTo(level, belowPos, belowState, Direction.DOWN, newBelowLiquid);
        if (this.sourceNeighborCount(level, pos) >= 3) {
          this.spreadToSides(level, pos, state, blockState);
        }
      } else if (state.isSource() || !this.isWaterHole(level, newBelowLiquid.getType(), pos, blockState, belowPos, belowState)) {
        this.spreadToSides(level, pos, state, blockState);
      }
    }
  }

  private spreadToSides(level: WorldGenLevel, pos: BlockPos, state: FluidState, blockState: BlockState): void {
    let amount = state.getAmount() - this.getDropOff(level);
    if (state.isFalling()) {
      amount = 7;
    }

    if (amount > 0) {
      const spread = this.getSpread(level, pos, blockState);
      for (const [direction, spreadState] of spread) {
        const targetPos = pos.relative(direction);
        const targetBlockState = level.getBlockState(targetPos);
        if (this.canSpreadTo(level, pos, blockState, direction, targetPos, targetBlockState, level.getFluidState(targetPos), spreadState.getType())) {
          this.spreadTo(level, targetPos, targetBlockState, direction, spreadState);
        }
      }
    }
  }

  public getNewLiquid(level: WorldGenLevel, pos: BlockPos, state: BlockState): FluidState {
    let amount = 0;
    let sources = 0;

    for (const direction of Direction.Plane.HORIZONTAL) {
      const neighborPos = pos.relative(direction);
      const neighborState = level.getBlockState(neighborPos);
      const neighborFluid = neighborState.getFluidState();
      if (neighborFluid.getType().isSame(this) && this.canPassThroughWall(direction, level, pos, state, neighborPos, neighborState)) {
        if (neighborFluid.isSource()) {
          sources++;
        }

        amount = Math.max(amount, neighborFluid.getAmount());
      }
    }

    if (this.canConvertToSource() && sources >= 2) {
      const belowState = level.getBlockState(pos.below());
      const belowFluid = belowState.getFluidState();
      if (belowState.getMaterial().isSolid() || this.isSourceBlockOfThisType(belowFluid)) {
        return this.getSourceState(false);
      }
    }

    const abovePos = pos.above();
    const aboveState = level.getBlockState(abovePos);
    const aboveFluid = aboveState.getFluidState();
    if (!aboveFluid.isEmpty() && aboveFluid.getType().isSame(this) && this.canPassThroughWall(Direction.UP, level, pos, state, abovePos, aboveState)) {
      return this.getFlowingState(8, true);
    }

    const newAmount = amount - this.getDropOff(level);
    return newAmount <= 0 ? Fluids.EMPTY.defaultFluidState() : this.getFlowingState(newAmount, false);
  }

  private canPassThroughWall(_direction: Direction, _level: BlockGetter, _fromPos: BlockPos, _fromState: BlockState, _toPos: BlockPos, _toState: BlockState): boolean {
    return true;
  }

  protected spreadTo(level: WorldGenLevel, pos: BlockPos, state: BlockState, _direction: Direction, fluidState: FluidState): void {
    if (!state.isAir()) {
      this.beforeDestroyingBlock(level, pos, state);
    }

    level.setBlock(pos, fluidState.createLegacyBlock(), 3);
  }

  protected getSlopeDistance(
    level: WorldGenLevel,
    pos: BlockPos,
    distance: number,
    fromDirection: Direction,
    state: BlockState,
    origin: BlockPos,
    stateCache: Map<number, readonly [BlockState, FluidState]>,
    holeCache: Map<number, boolean>,
  ): number {
    let best = 1000;

    for (const direction of Direction.Plane.HORIZONTAL) {
      if (direction !== fromDirection) {
        const targetPos = pos.relative(direction);
        const key = getCacheKey(origin, targetPos);
        let cached = stateCache.get(key);
        if (cached === undefined) {
          const targetState = level.getBlockState(targetPos);
          cached = [targetState, targetState.getFluidState()];
          stateCache.set(key, cached);
        }

        const [targetState, targetFluid] = cached;
        if (this.canPassThrough(level, this.getFlowing(), pos, state, direction, targetPos, targetState, targetFluid)) {
          let waterHole = holeCache.get(key);
          if (waterHole === undefined) {
            const belowPos = targetPos.below();
            waterHole = this.isWaterHole(level, this.getFlowing(), targetPos, targetState, belowPos, level.getBlockState(belowPos));
            holeCache.set(key, waterHole);
          }

          if (waterHole) {
            return distance;
          }

          if (distance < this.getSlopeFindDistance(level)) {
            const recursive = this.getSlopeDistance(level, targetPos, distance + 1, direction.getOpposite(), targetState, origin, stateCache, holeCache);
            if (recursive < best) {
              best = recursive;
            }
          }
        }
      }
    }

    return best;
  }

  private isWaterHole(level: BlockGetter, fluid: Fluid, pos: BlockPos, state: BlockState, belowPos: BlockPos, belowState: BlockState): boolean {
    if (!this.canPassThroughWall(Direction.DOWN, level, pos, state, belowPos, belowState)) {
      return false;
    }

    return belowState.getFluidState().getType().isSame(this) ? true : this.canHoldFluid(level, belowPos, belowState, fluid);
  }

  private canPassThrough(
    level: BlockGetter,
    fluid: Fluid,
    pos: BlockPos,
    state: BlockState,
    direction: Direction,
    targetPos: BlockPos,
    targetState: BlockState,
    targetFluid: FluidState,
  ): boolean {
    return !this.isSourceBlockOfThisType(targetFluid)
      && this.canPassThroughWall(direction, level, pos, state, targetPos, targetState)
      && this.canHoldFluid(level, targetPos, targetState, fluid);
  }

  private isSourceBlockOfThisType(state: FluidState): boolean {
    return state.getType().isSame(this) && state.isSource();
  }

  private sourceNeighborCount(level: WorldGenLevel, pos: BlockPos): number {
    let sources = 0;
    for (const direction of Direction.Plane.HORIZONTAL) {
      const fluid = level.getFluidState(pos.relative(direction));
      if (this.isSourceBlockOfThisType(fluid)) {
        sources++;
      }
    }
    return sources;
  }

  protected getSpread(level: WorldGenLevel, pos: BlockPos, state: BlockState): Map<Direction, FluidState> {
    let best = 1000;
    const spread = new Map<Direction, FluidState>();
    const stateCache = new Map<number, readonly [BlockState, FluidState]>();
    const holeCache = new Map<number, boolean>();

    for (const direction of Direction.Plane.HORIZONTAL) {
      const targetPos = pos.relative(direction);
      const key = getCacheKey(pos, targetPos);
      let cached = stateCache.get(key);
      if (cached === undefined) {
        const targetState = level.getBlockState(targetPos);
        cached = [targetState, targetState.getFluidState()];
        stateCache.set(key, cached);
      }

      const [targetState, targetFluid] = cached;
      const newLiquid = this.getNewLiquid(level, targetPos, targetState);
      if (this.canPassThrough(level, newLiquid.getType(), pos, state, direction, targetPos, targetState, targetFluid)) {
        const belowPos = targetPos.below();
        let waterHole = holeCache.get(key);
        if (waterHole === undefined) {
          waterHole = this.isWaterHole(level, this.getFlowing(), targetPos, targetState, belowPos, level.getBlockState(belowPos));
          holeCache.set(key, waterHole);
        }

        const distance = waterHole ? 0 : this.getSlopeDistance(level, targetPos, 1, direction.getOpposite(), targetState, pos, stateCache, holeCache);
        if (distance < best) {
          spread.clear();
        }

        if (distance <= best) {
          spread.set(direction, newLiquid);
          best = distance;
        }
      }
    }

    return spread;
  }

  private canHoldFluid(_level: BlockGetter, _pos: BlockPos, state: BlockState, _fluid: Fluid): boolean {
    const material = state.getMaterial();
    return material !== Material.PORTAL
      && material !== Material.STRUCTURAL_AIR
      && material !== Material.WATER_PLANT
      && material !== Material.REPLACEABLE_WATER_PLANT
      && !material.blocksMotion();
  }

  protected canSpreadTo(
    level: BlockGetter,
    pos: BlockPos,
    state: BlockState,
    direction: Direction,
    targetPos: BlockPos,
    targetState: BlockState,
    targetFluid: FluidState,
    fluid: Fluid,
  ): boolean {
    return targetFluid.canBeReplacedWith(level, targetPos, fluid, direction)
      && this.canPassThroughWall(direction, level, pos, state, targetPos, targetState)
      && this.canHoldFluid(level, targetPos, targetState, fluid);
  }

  protected getSpreadDelay(level: WorldGenLevel, _pos: BlockPos, _oldState: FluidState, _newState: FluidState): number {
    return this.getTickDelay(level);
  }

  public override tick(level: WorldGenLevel, pos: BlockPos, state: FluidState): void {
    let spreadState = state;
    if (!state.isSource()) {
      const newLiquid = this.getNewLiquid(level, pos, level.getBlockState(pos));
      const delay = this.getSpreadDelay(level, pos, state, newLiquid);
      if (newLiquid.isEmpty()) {
        spreadState = newLiquid;
        level.setBlock(pos, Fluids.EMPTY.createLegacyBlock(newLiquid), 3);
      } else if (!newLiquid.equals(state)) {
        spreadState = newLiquid;
        const legacyState = newLiquid.createLegacyBlock();
        level.setBlock(pos, legacyState, 2);
        level.getLiquidTicks().scheduleTick(pos, newLiquid.getType(), delay);
      }
    }

    this.spread(level, pos, spreadState);
  }

  public static getLegacyLevel(state: FluidState): number {
    return state.isSource() ? 0 : 8 - Math.min(state.getAmount(), 8) + (state.isFalling() ? 8 : 0);
  }

  private static hasSameAbove(state: FluidState, level: BlockGetter, pos: BlockPos): boolean {
    return state.getType().isSame(level.getFluidState(pos.above()).getType());
  }

  public override getHeight(state: FluidState, level: BlockGetter, pos: BlockPos): number {
    return FlowingFluid.hasSameAbove(state, level, pos) ? 1.0 : state.getOwnHeight();
  }

  public override getOwnHeight(state: FluidState): number {
    return state.getAmount() / 9.0;
  }
}
