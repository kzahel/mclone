import { BlockPos } from "../../../../core/block-pos";
import { floor } from "../../../../util/mth";
import { MobAttribute } from "../../attribute";
import type { MobNavigation, PathfinderMob } from "../pathfinder-mob";
import { BlockPathTypes } from "../../../level/pathfinder/block-path-types";
import type { NodeEvaluator } from "../../../level/pathfinder/node-evaluator";
import { Path } from "../../../level/pathfinder/path";
import { PathFinder } from "../../../level/pathfinder/path-finder";
import { WalkNodeEvaluator } from "../../../level/pathfinder/walk-node-evaluator";
import { Vec3 } from "../../../phys/vec3";

const MAX_TIME_RECOMPUTE = 20;
const DEFAULT_FOLLOW_RANGE = 16.0;

function setContainsBlockPos(positions: ReadonlySet<BlockPos>, target: BlockPos | undefined): boolean {
  if (target === undefined) {
    return false;
  }
  for (const pos of positions) {
    if (pos.equals(target)) {
      return true;
    }
  }
  return false;
}

function bottomCenterOf(pos: BlockPos): Vec3 {
  return new Vec3(pos.getX() + 0.5, pos.getY(), pos.getZ() + 0.5);
}

export abstract class PathNavigation implements MobNavigation {
  protected path: Path | undefined;
  protected speedModifier = 0.0;
  protected tickCount = 0;
  protected lastStuckCheck = 0;
  protected lastStuckCheckPos = Vec3.ZERO;
  protected timeoutCachedNode = BlockPos.ZERO;
  protected timeoutTimer = 0;
  protected timeoutLimit = 0.0;
  protected maxDistanceToWaypoint = 0.5;
  protected hasDelayedRecomputationValue = false;
  protected timeLastRecompute = 0;
  protected nodeEvaluator!: NodeEvaluator;
  private targetPos: BlockPos | undefined;
  private reachRange = 0;
  private maxVisitedNodesMultiplier = 1.0;
  private readonly pathFinder: PathFinder;
  private stuck = false;

  protected constructor(protected readonly mob: PathfinderMob) {
    this.pathFinder = this.createPathFinder(floor(DEFAULT_FOLLOW_RANGE * 16.0));
  }

  public resetMaxVisitedNodesMultiplier(): void {
    this.maxVisitedNodesMultiplier = 1.0;
  }

  public setMaxVisitedNodesMultiplier(multiplier: number): void {
    this.maxVisitedNodesMultiplier = multiplier;
  }

  public getTargetPos(): BlockPos | undefined {
    return this.targetPos;
  }

  protected abstract createPathFinder(maxVisitedNodes: number): PathFinder;

  public setSpeedModifier(speed: number): void {
    this.speedModifier = speed;
  }

  public hasDelayedRecomputation(): boolean {
    return this.hasDelayedRecomputationValue;
  }

  public recomputePath(): void {
    const level = this.mob.getAiLevel();
    if (level === undefined) {
      return;
    }
    if (this.tickCount - this.timeLastRecompute > MAX_TIME_RECOMPUTE) {
      if (this.targetPos !== undefined) {
        this.path = undefined;
        this.path = this.createPath(this.targetPos, this.reachRange);
        this.timeLastRecompute = this.tickCount;
        this.hasDelayedRecomputationValue = false;
      }
    } else {
      this.hasDelayedRecomputationValue = true;
    }
  }

  public createPath(x: number, y: number, z: number, accuracy: number): Path | undefined;
  public createPath(pos: BlockPos, accuracy: number): Path | undefined;
  public createPath(first: BlockPos | number, second: number, third?: number, fourth?: number): Path | undefined {
    if (first instanceof BlockPos) {
      return this.createPathFromTargets(new Set([first]), 8, false, second);
    }

    return this.createPath(new BlockPos(first, second, third!), fourth!);
  }

  protected createPathFromTargets(
    targets: ReadonlySet<BlockPos>,
    regionOffset: number,
    offsetUpward: boolean,
    accuracy: number,
    maxRange = DEFAULT_FOLLOW_RANGE,
  ): Path | undefined {
    const level = this.mob.getAiLevel();
    if (targets.size === 0 || level === undefined || this.mob.getY() < level.getMinBuildHeight() || !this.canUpdatePath()) {
      return undefined;
    }
    if (this.path !== undefined && !this.path.isDone() && setContainsBlockPos(targets, this.targetPos)) {
      return this.path;
    }

    // Runtime: MobAiLevel is already the loaded navigation region; vanilla allocates a bounded PathNavigationRegion here.
    void regionOffset;
    void offsetUpward;

    const path = this.pathFinder.findPath(level, this.mob, targets, maxRange, accuracy, this.maxVisitedNodesMultiplier);
    if (path !== undefined && path.getTarget() !== undefined) {
      this.targetPos = path.getTarget();
      this.reachRange = accuracy;
      this.resetStuckTimeout();
    }

    return path;
  }

  public moveTo(x: number, y: number, z: number, speed: number): boolean {
    return this.moveToPath(this.createPath(x, y, z, 1), speed);
  }

  public moveToPath(path: Path | undefined, speed: number): boolean {
    if (path === undefined) {
      this.path = undefined;
      return false;
    }
    if (!path.sameAs(this.path)) {
      this.path = path;
    }
    if (this.isDone()) {
      return false;
    }

    this.trimPath();
    if (this.path === undefined || this.path.getNodeCount() <= 0) {
      return false;
    }

    this.speedModifier = speed;
    const pos = this.getTempMobPos();
    this.lastStuckCheck = this.tickCount;
    this.lastStuckCheckPos = pos;
    return true;
  }

  public getPath(): Path | undefined {
    return this.path;
  }

  public tick(): void {
    this.tickCount++;
    if (this.hasDelayedRecomputationValue) {
      this.recomputePath();
    }

    if (this.isDone()) {
      return;
    }

    if (this.canUpdatePath()) {
      this.followThePath();
    } else if (this.path !== undefined && !this.path.isDone()) {
      const mobPos = this.getTempMobPos();
      const nextPos = this.path.getNextEntityPos(this.mob);
      if (mobPos.y > nextPos.y
        && !this.mob.isOnGround()
        && floor(mobPos.x) === floor(nextPos.x)
        && floor(mobPos.z) === floor(nextPos.z)) {
        this.path.advance();
      }
    }

    if (!this.isDone() && this.path !== undefined) {
      const level = this.mob.getAiLevel();
      if (level === undefined) {
        return;
      }

      const nextPos = this.path.getNextEntityPos(this.mob);
      const blockPos = new BlockPos(nextPos.x, nextPos.y, nextPos.z);
      const wantedY = level.getBlockState(blockPos.below()).isAir()
        ? nextPos.y
        : WalkNodeEvaluator.getFloorLevel(level, blockPos);
      this.mob.getMoveControl().setWantedPosition(nextPos.x, wantedY, nextPos.z, this.speedModifier);
    }
  }

  protected followThePath(): void {
    if (this.path === undefined) {
      return;
    }

    const mobPos = this.getTempMobPos();
    this.maxDistanceToWaypoint = this.mob.getBbWidth() > 0.75
      ? this.mob.getBbWidth() / 2.0
      : 0.75 - (this.mob.getBbWidth() / 2.0);
    const nextNodePos = this.path.getNextNodePos();
    const dx = Math.abs(this.mob.getX() - (nextNodePos.getX() + 0.5));
    const dy = Math.abs(this.mob.getY() - nextNodePos.getY());
    const dz = Math.abs(this.mob.getZ() - (nextNodePos.getZ() + 0.5));
    const reached = dx < this.maxDistanceToWaypoint && dz < this.maxDistanceToWaypoint && dy < 1.0;
    if (reached || (this.mob.canCutCorner(this.path.getNextNode().type) && this.shouldTargetNextNodeInDirection(mobPos))) {
      this.path.advance();
    }

    this.doStuckDetection(mobPos);
  }

  private shouldTargetNextNodeInDirection(mobPos: Vec3): boolean {
    if (this.path === undefined || this.path.getNextNodeIndex() + 1 >= this.path.getNodeCount()) {
      return false;
    }

    const current = bottomCenterOf(this.path.getNextNodePos());
    if (!mobPos.closerThan(current, 2.0)) {
      return false;
    }

    const next = bottomCenterOf(this.path.getNodePos(this.path.getNextNodeIndex() + 1));
    const pathDirection = next.subtract(current);
    const mobDirection = mobPos.subtract(current);
    return pathDirection.dot(mobDirection) > 0.0;
  }

  protected doStuckDetection(position: Vec3): void {
    if (this.tickCount - this.lastStuckCheck > 100) {
      if (position.distanceToSqr(this.lastStuckCheckPos) < 2.25) {
        this.stuck = true;
        this.stop();
      } else {
        this.stuck = false;
      }

      this.lastStuckCheck = this.tickCount;
      this.lastStuckCheckPos = position;
    }

    if (this.path !== undefined && !this.path.isDone()) {
      const nextNode = this.path.getNextNodePos();
      if (nextNode.equals(this.timeoutCachedNode)) {
        this.timeoutTimer++;
      } else {
        this.timeoutCachedNode = nextNode;
        const distance = position.distanceTo(bottomCenterOf(this.timeoutCachedNode));
        const speed = this.mob.getAttributeValue(MobAttribute.MOVEMENT_SPEED) * Math.max(this.speedModifier, 0.0);
        this.timeoutLimit = speed > 0.0 ? (distance / speed) * 20.0 : 0.0;
        this.timeoutTimer = 0;
      }

      if (this.timeoutLimit > 0.0 && this.timeoutTimer > this.timeoutLimit * 3.0) {
        this.timeoutPath();
      }
    }
  }

  private timeoutPath(): void {
    this.resetStuckTimeout();
    this.stop();
  }

  private resetStuckTimeout(): void {
    this.timeoutCachedNode = BlockPos.ZERO;
    this.timeoutTimer = 0;
    this.timeoutLimit = 0.0;
    this.stuck = false;
  }

  public isDone(): boolean {
    return this.path === undefined || this.path.isDone();
  }

  public isInProgress(): boolean {
    return !this.isDone();
  }

  public stop(): void {
    this.path = undefined;
  }

  protected abstract getTempMobPos(): Vec3;

  protected abstract canUpdatePath(): boolean;

  protected isInLiquid(): boolean {
    return this.mob.isInWaterOrBubble();
  }

  protected trimPath(): void {
  }

  protected canMoveDirectly(_from: Vec3, _to: Vec3, _sizeX: number, _sizeY: number, _sizeZ: number): boolean {
    return false;
  }

  public isStableDestination(pos: BlockPos): boolean {
    const level = this.mob.getAiLevel();
    if (level === undefined) {
      return false;
    }

    const below = pos.below();
    return level.getBlockState(below).isSolidRender(level, below);
  }

  public getNodeEvaluator(): NodeEvaluator {
    return this.nodeEvaluator;
  }

  public setCanFloat(canSwim: boolean): void {
    this.nodeEvaluator.setCanFloat(canSwim);
  }

  public canFloat(): boolean {
    return this.nodeEvaluator.canFloat();
  }

  public recomputePathNear(pos: BlockPos): void {
    if (this.path === undefined || this.path.isDone() || this.path.getNodeCount() === 0) {
      return;
    }

    const endNode = this.path.getEndNode();
    if (endNode === undefined) {
      return;
    }

    const midpoint = new Vec3(
      (endNode.x + this.mob.getX()) / 2.0,
      (endNode.y + this.mob.getY()) / 2.0,
      (endNode.z + this.mob.getZ()) / 2.0,
    );
    if (new Vec3(pos.getX(), pos.getY(), pos.getZ()).closerThan(midpoint, this.path.getNodeCount() - this.path.getNextNodeIndex())) {
      this.recomputePath();
    }
  }

  public getMaxDistanceToWaypoint(): number {
    return this.maxDistanceToWaypoint;
  }

  public isStuck(): boolean {
    return this.stuck;
  }

  protected hasValidPathType(type: BlockPathTypes): boolean {
    return type !== BlockPathTypes.WATER && type !== BlockPathTypes.LAVA && type !== BlockPathTypes.OPEN;
  }
}
