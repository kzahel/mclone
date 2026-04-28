import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import { floor } from "../../../util/mth";
import { BlockTags } from "../../../tags/block-tags";
import { CactusBlock } from "../block/cactus-block";
import { CocoaBlock } from "../block/cocoa-block";
import { LeavesBlock } from "../block/leaves-block";
import { SweetBerryBushBlock } from "../block/sweet-berry-bush-block";
import { WaterlilyBlock } from "../block/waterlily-block";
import type { BlockGetter } from "../block-getter";
import type { BlockState } from "../block/state/block-state";
import { Material } from "../material/material";
import { Fluids } from "../material/fluids";
import type { PathfinderMob } from "../../entity/ai/pathfinder-mob";
import { AABB } from "../../phys/aabb";
import { Vec3 } from "../../phys/vec3";
import { BlockPathTypes, type BlockPathTypes as BlockPathType } from "./block-path-types";
import { Node } from "./node";
import { NodeEvaluator } from "./node-evaluator";
import { PathComputationType } from "./path-computation-type";
import type { PathNavigationRegion } from "./path-navigation-region";
import { Target } from "./target";

function blockLocation(state: BlockState): string | undefined {
  return state.getBlock().getLocation()?.toString();
}

function isBlockLocation(state: BlockState, location: string): boolean {
  return blockLocation(state) === location;
}

function mutablePos(x: number, y: number, z: number): BlockPos.MutableBlockPos {
  return new BlockPos.MutableBlockPos(floor(x), floor(y), floor(z));
}

export class WalkNodeEvaluator extends NodeEvaluator {
  public static readonly SPACE_BETWEEN_WALL_POSTS = 0.5;
  protected oldWaterCost = 0.0;
  private readonly pathTypesByPosCache = new Map<bigint, BlockPathType>();
  private readonly collisionCache = new Map<string, boolean>();

  public override prepare(level: PathNavigationRegion, mob: PathfinderMob): void {
    super.prepare(level, mob);
    this.oldWaterCost = mob.getPathfindingMalus(BlockPathTypes.WATER);
  }

  public override done(): void {
    const mob = this.requireMob();
    mob.setPathfindingMalus(BlockPathTypes.WATER, this.oldWaterCost);
    this.pathTypesByPosCache.clear();
    this.collisionCache.clear();
    super.done();
  }

  public override getStart(): Node {
    const level = this.requireLevel();
    const mob = this.requireMob();
    const pos = new BlockPos.MutableBlockPos();
    let y = mob.getBlockY();
    let state = level.getBlockState(pos.set(floor(mob.getX()), y, floor(mob.getZ())));
    if (!mob.canStandOnFluid(state.getFluidState().getType())) {
      if (this.canFloat() && mob.isInWaterOrBubble()) {
        while (true) {
          if (!state.getFluidState().getType().isSame(Fluids.WATER) || !state.getFluidState().isSource()) {
            y--;
            break;
          }

          state = level.getBlockState(pos.set(floor(mob.getX()), ++y, floor(mob.getZ())));
        }
      } else if (mob.isOnGround()) {
        y = floor(mob.getY() + 0.5);
      } else {
        let blockPos = mob.blockPosition();

        while (
          (level.getBlockState(blockPos).isAir()
            || level.getBlockState(blockPos).isPathfindable(level, blockPos, PathComputationType.LAND))
          && blockPos.getY() > level.getMinBuildHeight()
        ) {
          blockPos = blockPos.below();
        }

        y = blockPos.above().getY();
      }
    } else {
      while (mob.canStandOnFluid(state.getFluidState().getType())) {
        state = level.getBlockState(pos.set(floor(mob.getX()), ++y, floor(mob.getZ())));
      }

      y--;
    }

    const blockPos = mob.blockPosition();
    const pathType = this.getCachedBlockType(mob, blockPos.getX(), y, blockPos.getZ());
    if (mob.getPathfindingMalus(pathType) < 0.0) {
      const box = mob.getBoundingBox();
      if (this.hasPositiveMalus(mutablePos(box.minX, y, box.minZ))
        || this.hasPositiveMalus(mutablePos(box.minX, y, box.maxZ))
        || this.hasPositiveMalus(mutablePos(box.maxX, y, box.minZ))
        || this.hasPositiveMalus(mutablePos(box.maxX, y, box.maxZ))) {
        const node = this.getNode(pos);
        node.type = this.getBlockPathTypeForMob(mob, node.asBlockPos());
        node.costMalus = mob.getPathfindingMalus(node.type);
        return node;
      }
    }

    const node = this.getNode(blockPos.getX(), y, blockPos.getZ());
    node.type = this.getBlockPathTypeForMob(mob, node.asBlockPos());
    node.costMalus = mob.getPathfindingMalus(node.type);
    return node;
  }

  private hasPositiveMalus(pos: BlockPos): boolean {
    const mob = this.requireMob();
    const type = this.getBlockPathTypeForMob(mob, pos);
    return mob.getPathfindingMalus(type) >= 0.0;
  }

  public override getGoal(x: number, y: number, z: number): Target {
    return new Target(this.getNode(floor(x), floor(y), floor(z)));
  }

  public override getNeighbors(output: Node[], node: Node): number {
    const mob = this.requireMob();
    let count = 0;
    let stepHeight = 0;
    const aboveType = this.getCachedBlockType(mob, node.x, node.y + 1, node.z);
    const currentType = this.getCachedBlockType(mob, node.x, node.y, node.z);
    if (mob.getPathfindingMalus(aboveType) >= 0.0 && currentType !== BlockPathTypes.STICKY_HONEY) {
      stepHeight = floor(Math.max(1.0, mob.getMaxUpStep()));
    }

    const floorLevel = this.getFloorLevel(new BlockPos(node.x, node.y, node.z));
    const south = this.findAcceptedNode(node.x, node.y, node.z + 1, stepHeight, floorLevel, Direction.SOUTH, currentType);
    if (this.isNeighborValid(south, node)) {
      output[count++] = south;
    }

    const west = this.findAcceptedNode(node.x - 1, node.y, node.z, stepHeight, floorLevel, Direction.WEST, currentType);
    if (this.isNeighborValid(west, node)) {
      output[count++] = west;
    }

    const east = this.findAcceptedNode(node.x + 1, node.y, node.z, stepHeight, floorLevel, Direction.EAST, currentType);
    if (this.isNeighborValid(east, node)) {
      output[count++] = east;
    }

    const north = this.findAcceptedNode(node.x, node.y, node.z - 1, stepHeight, floorLevel, Direction.NORTH, currentType);
    if (this.isNeighborValid(north, node)) {
      output[count++] = north;
    }

    const northWest = this.findAcceptedNode(node.x - 1, node.y, node.z - 1, stepHeight, floorLevel, Direction.NORTH, currentType);
    if (this.isDiagonalValid(node, west, north, northWest)) {
      output[count++] = northWest;
    }

    const northEast = this.findAcceptedNode(node.x + 1, node.y, node.z - 1, stepHeight, floorLevel, Direction.NORTH, currentType);
    if (this.isDiagonalValid(node, east, north, northEast)) {
      output[count++] = northEast;
    }

    const southWest = this.findAcceptedNode(node.x - 1, node.y, node.z + 1, stepHeight, floorLevel, Direction.SOUTH, currentType);
    if (this.isDiagonalValid(node, west, south, southWest)) {
      output[count++] = southWest;
    }

    const southEast = this.findAcceptedNode(node.x + 1, node.y, node.z + 1, stepHeight, floorLevel, Direction.SOUTH, currentType);
    if (this.isDiagonalValid(node, east, south, southEast)) {
      output[count++] = southEast;
    }

    return count;
  }

  protected isNeighborValid(node: Node | undefined, current: Node): node is Node {
    return node !== undefined && !node.closed && (node.costMalus >= 0.0 || current.costMalus < 0.0);
  }

  protected isDiagonalValid(current: Node, first: Node | undefined, second: Node | undefined, diagonal: Node | undefined): diagonal is Node {
    const mob = this.requireMob();
    if (diagonal === undefined || second === undefined || first === undefined) {
      return false;
    }
    if (diagonal.closed) {
      return false;
    }
    if (second.y > current.y || first.y > current.y) {
      return false;
    }
    if (first.type === BlockPathTypes.WALKABLE_DOOR
      || second.type === BlockPathTypes.WALKABLE_DOOR
      || diagonal.type === BlockPathTypes.WALKABLE_DOOR) {
      return false;
    }

    const fenceCorner = second.type === BlockPathTypes.FENCE && first.type === BlockPathTypes.FENCE && mob.getBbWidth() < 0.5;
    return diagonal.costMalus >= 0.0
      && (second.y < current.y || second.costMalus >= 0.0 || fenceCorner)
      && (first.y < current.y || first.costMalus >= 0.0 || fenceCorner);
  }

  private canReachWithoutCollision(node: Node): boolean {
    const mob = this.requireMob();
    let movement = new Vec3(node.x - mob.getX(), node.y - mob.getY(), node.z - mob.getZ());
    let box = mob.getBoundingBox();
    const steps = Math.ceil(movement.length() / box.getSize());
    movement = movement.scale(1.0 / steps);

    for (let i = 1; i <= steps; i++) {
      box = box.move(movement);
      if (this.hasCollisions(box)) {
        return false;
      }
    }

    return true;
  }

  protected getFloorLevel(pos: BlockPos): number {
    return WalkNodeEvaluator.getFloorLevel(this.requireLevel(), pos);
  }

  public static getFloorLevel(level: BlockGetter, pos: BlockPos): number {
    const below = pos.below();
    const shape = level.getBlockState(below).getCollisionShape(level, below);
    return below.getY() + (shape.isEmpty() ? 0.0 : shape.max(Direction.Axis.Y));
  }

  protected isAmphibious(): boolean {
    return false;
  }

  protected findAcceptedNode(
    x: number,
    y: number,
    z: number,
    stepHeight: number,
    floorLevel: number,
    direction: Direction,
    previousType: BlockPathType,
  ): Node | undefined {
    const level = this.requireLevel();
    const mob = this.requireMob();
    let node: Node | undefined;
    const pos = new BlockPos.MutableBlockPos();
    const targetFloorLevel = this.getFloorLevel(pos.set(x, y, z));
    if (targetFloorLevel - floorLevel > 1.125) {
      return undefined;
    }

    let type = this.getCachedBlockType(mob, x, y, z);
    let malus = mob.getPathfindingMalus(type);
    const halfWidth = mob.getBbWidth() / 2.0;
    if (malus >= 0.0) {
      node = this.getNode(x, y, z);
      node.type = type;
      node.costMalus = Math.max(node.costMalus, malus);
    }

    if (previousType === BlockPathTypes.FENCE && node !== undefined && node.costMalus >= 0.0 && !this.canReachWithoutCollision(node)) {
      node = undefined;
    }

    if (type !== BlockPathTypes.WALKABLE && (!this.isAmphibious() || type !== BlockPathTypes.WATER)) {
      if ((node === undefined || node.costMalus < 0.0)
        && stepHeight > 0
        && type !== BlockPathTypes.FENCE
        && type !== BlockPathTypes.UNPASSABLE_RAIL
        && type !== BlockPathTypes.TRAPDOOR
        && type !== BlockPathTypes.POWDER_SNOW) {
        node = this.findAcceptedNode(x, y + 1, z, stepHeight - 1, floorLevel, direction, previousType);
        if (node !== undefined && (node.type === BlockPathTypes.OPEN || node.type === BlockPathTypes.WALKABLE) && mob.getBbWidth() < 1.0) {
          const previousX = x - direction.getStepX() + 0.5;
          const previousZ = z - direction.getStepZ() + 0.5;
          const stepBox = new AABB(
            previousX - halfWidth,
            WalkNodeEvaluator.getFloorLevel(level, mutablePos(previousX, y + 1, previousZ)) + 0.001,
            previousZ - halfWidth,
            previousX + halfWidth,
            mob.getBbHeight() + WalkNodeEvaluator.getFloorLevel(level, new BlockPos(node.x, node.y, node.z)) - 0.002,
            previousZ + halfWidth,
          );
          if (this.hasCollisions(stepBox)) {
            node = undefined;
          }
        }
      }

      if (!this.isAmphibious() && type === BlockPathTypes.WATER && !this.canFloat()) {
        if (this.getCachedBlockType(mob, x, y - 1, z) !== BlockPathTypes.WATER) {
          return node;
        }

        while (y > level.getMinBuildHeight()) {
          type = this.getCachedBlockType(mob, x, --y, z);
          if (type !== BlockPathTypes.WATER) {
            return node;
          }

          node = this.getNode(x, y, z);
          node.type = type;
          node.costMalus = Math.max(node.costMalus, mob.getPathfindingMalus(type));
        }
      }

      if (type === BlockPathTypes.OPEN) {
        let fallDistance = 0;
        const originalY = y;

        while (type === BlockPathTypes.OPEN) {
          if (--y < level.getMinBuildHeight()) {
            const blocked = this.getNode(x, originalY, z);
            blocked.type = BlockPathTypes.BLOCKED;
            blocked.costMalus = -1.0;
            return blocked;
          }

          if (fallDistance++ >= mob.getMaxFallDistance()) {
            const blocked = this.getNode(x, y, z);
            blocked.type = BlockPathTypes.BLOCKED;
            blocked.costMalus = -1.0;
            return blocked;
          }

          type = this.getCachedBlockType(mob, x, y, z);
          malus = mob.getPathfindingMalus(type);
          if (type !== BlockPathTypes.OPEN && malus >= 0.0) {
            node = this.getNode(x, y, z);
            node.type = type;
            node.costMalus = Math.max(node.costMalus, malus);
            break;
          }

          if (malus < 0.0) {
            const blocked = this.getNode(x, y, z);
            blocked.type = BlockPathTypes.BLOCKED;
            blocked.costMalus = -1.0;
            return blocked;
          }
        }
      }

      if (type === BlockPathTypes.FENCE) {
        node = this.getNode(x, y, z);
        node.closed = true;
        node.type = type;
        node.costMalus = type.getMalus();
      }

      return node;
    }

    return node;
  }

  private hasCollisions(box: AABB): boolean {
    const key = box.toString();
    let collides = this.collisionCache.get(key);
    if (collides === undefined) {
      collides = !this.requireLevel().noCollision(this.requireMob(), box);
      this.collisionCache.set(key, collides);
    }
    return collides;
  }

  public getBlockPathType(
    level: BlockGetter,
    x: number,
    y: number,
    z: number,
    mob: PathfinderMob,
    xSize: number,
    ySize: number,
    zSize: number,
    canOpenDoors: boolean,
    canPassDoors: boolean,
  ): BlockPathType;
  public getBlockPathType(level: BlockGetter, x: number, y: number, z: number): BlockPathType;
  public getBlockPathType(
    level: BlockGetter,
    x: number,
    y: number,
    z: number,
    mob?: PathfinderMob,
    xSize?: number,
    ySize?: number,
    zSize?: number,
    canOpenDoors?: boolean,
    canPassDoors?: boolean,
  ): BlockPathType {
    if (mob === undefined) {
      return WalkNodeEvaluator.getBlockPathTypeStatic(level, new BlockPos.MutableBlockPos(x, y, z));
    }

    const types = new Set<BlockPathType>();
    let type = BlockPathTypes.BLOCKED;
    const pos = mob.blockPosition();
    type = this.getBlockPathTypes(level, x, y, z, xSize!, ySize!, zSize!, canOpenDoors!, canPassDoors!, types, type, pos);
    if (types.has(BlockPathTypes.FENCE)) {
      return BlockPathTypes.FENCE;
    }
    if (types.has(BlockPathTypes.UNPASSABLE_RAIL)) {
      return BlockPathTypes.UNPASSABLE_RAIL;
    }

    let best = BlockPathTypes.BLOCKED;
    for (const candidate of BlockPathTypes.values()) {
      if (!types.has(candidate)) {
        continue;
      }
      if (mob.getPathfindingMalus(candidate) < 0.0) {
        return candidate;
      }
      if (mob.getPathfindingMalus(candidate) >= mob.getPathfindingMalus(best)) {
        best = candidate;
      }
    }

    return type === BlockPathTypes.OPEN && mob.getPathfindingMalus(best) === 0.0 && xSize! <= 1 ? BlockPathTypes.OPEN : best;
  }

  public getBlockPathTypes(
    level: BlockGetter,
    x: number,
    y: number,
    z: number,
    xSize: number,
    ySize: number,
    zSize: number,
    canOpenDoors: boolean,
    canPassDoors: boolean,
    nodeTypes: Set<BlockPathType>,
    nodeType: BlockPathType,
    pos: BlockPos,
  ): BlockPathType {
    let result = nodeType;
    for (let xOffset = 0; xOffset < xSize; xOffset++) {
      for (let yOffset = 0; yOffset < ySize; yOffset++) {
        for (let zOffset = 0; zOffset < zSize; zOffset++) {
          const sampleX = xOffset + x;
          const sampleY = yOffset + y;
          const sampleZ = zOffset + z;
          let sampleType = this.getBlockPathType(level, sampleX, sampleY, sampleZ);
          sampleType = this.evaluateBlockPathType(level, canOpenDoors, canPassDoors, pos, sampleType);
          if (xOffset === 0 && yOffset === 0 && zOffset === 0) {
            result = sampleType;
          }

          nodeTypes.add(sampleType);
        }
      }
    }

    return result;
  }

  protected evaluateBlockPathType(
    level: BlockGetter,
    canOpenDoors: boolean,
    canPassDoors: boolean,
    pos: BlockPos,
    nodeType: BlockPathType,
  ): BlockPathType {
    let result = nodeType;
    if (result === BlockPathTypes.DOOR_WOOD_CLOSED && canOpenDoors && canPassDoors) {
      result = BlockPathTypes.WALKABLE_DOOR;
    }

    if (result === BlockPathTypes.DOOR_OPEN && !canPassDoors) {
      result = BlockPathTypes.BLOCKED;
    }

    // Runtime: rail block classes are not ported yet; tag/location checks keep the vanilla branch recoverable.
    if (result === BlockPathTypes.RAIL
      && !isBlockLocation(level.getBlockState(pos), "minecraft:rail")
      && !isBlockLocation(level.getBlockState(pos.below()), "minecraft:rail")) {
      result = BlockPathTypes.UNPASSABLE_RAIL;
    }

    if (result === BlockPathTypes.LEAVES) {
      result = BlockPathTypes.BLOCKED;
    }

    return result;
  }

  private getBlockPathTypeForMob(mob: PathfinderMob, pos: BlockPos): BlockPathType {
    return this.getCachedBlockType(mob, pos.getX(), pos.getY(), pos.getZ());
  }

  protected getCachedBlockType(mob: PathfinderMob, x: number, y: number, z: number): BlockPathType {
    const key = BlockPos.asLong(x, y, z);
    let type = this.pathTypesByPosCache.get(key);
    if (type === undefined) {
      type = this.getBlockPathType(
        this.requireLevel(),
        x,
        y,
        z,
        mob,
        this.entityWidth,
        this.entityHeight,
        this.entityDepth,
        this.canOpenDoors(),
        this.canPassDoors(),
      );
      this.pathTypesByPosCache.set(key, type);
    }
    return type;
  }

  public static getBlockPathTypeStatic(level: BlockGetter, pos: BlockPos.MutableBlockPos): BlockPathType {
    const x = pos.getX();
    const y = pos.getY();
    const z = pos.getZ();
    let type = WalkNodeEvaluator.getBlockPathTypeRaw(level, pos);
    if (type === BlockPathTypes.OPEN && y >= levelMinBuildHeight(level) + 1) {
      const belowType = WalkNodeEvaluator.getBlockPathTypeRaw(level, pos.set(x, y - 1, z));
      type = belowType !== BlockPathTypes.WALKABLE
        && belowType !== BlockPathTypes.OPEN
        && belowType !== BlockPathTypes.WATER
        && belowType !== BlockPathTypes.LAVA
        ? BlockPathTypes.WALKABLE
        : BlockPathTypes.OPEN;
      if (belowType === BlockPathTypes.DAMAGE_FIRE) {
        type = BlockPathTypes.DAMAGE_FIRE;
      }
      if (belowType === BlockPathTypes.DAMAGE_CACTUS) {
        type = BlockPathTypes.DAMAGE_CACTUS;
      }
      if (belowType === BlockPathTypes.DAMAGE_OTHER) {
        type = BlockPathTypes.DAMAGE_OTHER;
      }
      if (belowType === BlockPathTypes.STICKY_HONEY) {
        type = BlockPathTypes.STICKY_HONEY;
      }
    }

    if (type === BlockPathTypes.WALKABLE) {
      type = WalkNodeEvaluator.checkNeighbourBlocks(level, pos.set(x, y, z), type);
    }

    return type;
  }

  public static checkNeighbourBlocks(level: BlockGetter, centerPos: BlockPos.MutableBlockPos, nodeType: BlockPathType): BlockPathType {
    const x = centerPos.getX();
    const y = centerPos.getY();
    const z = centerPos.getZ();

    for (let xOffset = -1; xOffset <= 1; xOffset++) {
      for (let yOffset = -1; yOffset <= 1; yOffset++) {
        for (let zOffset = -1; zOffset <= 1; zOffset++) {
          if (xOffset !== 0 || zOffset !== 0) {
            centerPos.set(x + xOffset, y + yOffset, z + zOffset);
            const state = level.getBlockState(centerPos);
            if (state.getBlock() instanceof CactusBlock || isBlockLocation(state, "minecraft:cactus")) {
              return BlockPathTypes.DANGER_CACTUS;
            }

            if (state.getBlock() instanceof SweetBerryBushBlock || isBlockLocation(state, "minecraft:sweet_berry_bush")) {
              return BlockPathTypes.DANGER_OTHER;
            }

            if (WalkNodeEvaluator.isBurningBlock(state)) {
              return BlockPathTypes.DANGER_FIRE;
            }

            if (level.getFluidState(centerPos).getType().isSame(Fluids.WATER)) {
              return BlockPathTypes.WATER_BORDER;
            }
          }
        }
      }
    }

    return nodeType;
  }

  protected static getBlockPathTypeRaw(level: BlockGetter, pos: BlockPos): BlockPathType {
    const state = level.getBlockState(pos);
    const block = state.getBlock();
    const material = state.getMaterial();
    if (state.isAir()) {
      return BlockPathTypes.OPEN;
    }
    if (state.is(BlockTags.TRAPDOORS) || block instanceof WaterlilyBlock || isBlockLocation(state, "minecraft:big_dripleaf")) {
      return BlockPathTypes.TRAPDOOR;
    }
    if (material === Material.POWDER_SNOW || isBlockLocation(state, "minecraft:powder_snow")) {
      return BlockPathTypes.POWDER_SNOW;
    }
    if (block instanceof CactusBlock || isBlockLocation(state, "minecraft:cactus")) {
      return BlockPathTypes.DAMAGE_CACTUS;
    }
    if (block instanceof SweetBerryBushBlock || isBlockLocation(state, "minecraft:sweet_berry_bush")) {
      return BlockPathTypes.DAMAGE_OTHER;
    }
    if (isBlockLocation(state, "minecraft:honey_block")) {
      return BlockPathTypes.STICKY_HONEY;
    }
    if (block instanceof CocoaBlock || isBlockLocation(state, "minecraft:cocoa")) {
      return BlockPathTypes.COCOA;
    }

    const fluidState = level.getFluidState(pos);
    if (fluidState.getType().isSame(Fluids.LAVA)) {
      return BlockPathTypes.LAVA;
    }
    if (WalkNodeEvaluator.isBurningBlock(state)) {
      return BlockPathTypes.DAMAGE_FIRE;
    }
    if (isClosedWoodenDoor(state)) {
      return BlockPathTypes.DOOR_WOOD_CLOSED;
    }
    if (isClosedIronDoor(state)) {
      return BlockPathTypes.DOOR_IRON_CLOSED;
    }
    if (isOpenDoor(state)) {
      return BlockPathTypes.DOOR_OPEN;
    }
    if (isRail(state)) {
      return BlockPathTypes.RAIL;
    }
    if (block instanceof LeavesBlock || state.is(BlockTags.LEAVES)) {
      return BlockPathTypes.LEAVES;
    }
    if (!state.is(BlockTags.FENCES) && !state.is(BlockTags.WALLS) && !isClosedFenceGate(state)) {
      if (!state.isPathfindable(level, pos, PathComputationType.LAND)) {
        return BlockPathTypes.BLOCKED;
      }

      return fluidState.getType().isSame(Fluids.WATER) ? BlockPathTypes.WATER : BlockPathTypes.OPEN;
    }

    return BlockPathTypes.FENCE;
  }

  public static isBurningBlock(state: BlockState): boolean {
    return state.is(BlockTags.FIRE)
      || state.getFluidState().getType().isSame(Fluids.LAVA)
      || isBlockLocation(state, "minecraft:lava")
      || isBlockLocation(state, "minecraft:magma_block")
      || isBlockLocation(state, "minecraft:lava_cauldron")
      || isLitCampfire(state);
  }
}

function levelMinBuildHeight(level: BlockGetter): number {
  const maybeRegion = level as Partial<PathNavigationRegion>;
  return maybeRegion.getMinBuildHeight?.() ?? 0;
}

function isRail(state: BlockState): boolean {
  const location = blockLocation(state);
  return location !== undefined && location.endsWith("_rail") || location === "minecraft:rail";
}

function isClosedWoodenDoor(state: BlockState): boolean {
  const location = blockLocation(state);
  return location !== undefined && location.endsWith("_door") && !location.includes("iron") && !doorIsOpen(state);
}

function isClosedIronDoor(state: BlockState): boolean {
  const location = blockLocation(state);
  return location === "minecraft:iron_door" && !doorIsOpen(state);
}

function isOpenDoor(state: BlockState): boolean {
  const location = blockLocation(state);
  return location !== undefined && location.endsWith("_door") && doorIsOpen(state);
}

function doorIsOpen(_state: BlockState): boolean {
  // Runtime: door block/state properties are not ported yet, so registered door locations default closed.
  return false;
}

function isClosedFenceGate(state: BlockState): boolean {
  const location = blockLocation(state);
  // Runtime: fence-gate OPEN property is not ported yet, so registered fence gates default closed.
  return location !== undefined && location.endsWith("_fence_gate");
}

function isLitCampfire(state: BlockState): boolean {
  const location = blockLocation(state);
  // Runtime: campfire LIT property is not ported yet; treat registered campfires as burning for path malus.
  return location === "minecraft:campfire" || location === "minecraft:soul_campfire";
}
