import { BlockPos } from "../../../core/block-pos";
import { floor } from "../../../util/mth";
import type { BlockGetter } from "../block-getter";
import type { PathfinderMob } from "../../entity/ai/pathfinder-mob";
import { BlockPathTypes, type BlockPathTypes as BlockPathType } from "./block-path-types";
import { Node } from "./node";
import type { PathNavigationRegion } from "./path-navigation-region";
import type { Target } from "./target";

export abstract class NodeEvaluator {
  protected level: PathNavigationRegion | undefined;
  protected mob: PathfinderMob | undefined;
  protected readonly nodes = new Map<number, Node>();
  protected entityWidth = 0;
  protected entityHeight = 0;
  protected entityDepth = 0;
  protected canPassDoorsValue = false;
  protected canOpenDoorsValue = false;
  protected canFloatValue = false;

  public prepare(level: PathNavigationRegion, mob: PathfinderMob): void {
    this.level = level;
    this.mob = mob;
    this.nodes.clear();
    this.entityWidth = floor(mob.getBbWidth() + 1.0);
    this.entityHeight = floor(mob.getBbHeight() + 1.0);
    this.entityDepth = floor(mob.getBbWidth() + 1.0);
  }

  public done(): void {
    this.level = undefined;
    this.mob = undefined;
  }

  protected getNode(pos: BlockPos): Node;
  protected getNode(x: number, y: number, z: number): Node;
  protected getNode(first: BlockPos | number, second?: number, third?: number): Node {
    const x = first instanceof BlockPos ? first.getX() : first;
    const y = first instanceof BlockPos ? first.getY() : second!;
    const z = first instanceof BlockPos ? first.getZ() : third!;
    const hash = Node.createHash(x, y, z);
    let node = this.nodes.get(hash);
    if (node === undefined) {
      node = new Node(x, y, z);
      this.nodes.set(hash, node);
    }
    return node;
  }

  public abstract getStart(): Node;

  public abstract getGoal(x: number, y: number, z: number): Target;

  public abstract getNeighbors(output: Node[], node: Node): number;

  public abstract getBlockPathType(
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

  public abstract getBlockPathType(level: BlockGetter, x: number, y: number, z: number): BlockPathType;

  public setCanPassDoors(canEnterDoors: boolean): void {
    this.canPassDoorsValue = canEnterDoors;
  }

  public setCanOpenDoors(canOpenDoors: boolean): void {
    this.canOpenDoorsValue = canOpenDoors;
  }

  public setCanFloat(canSwim: boolean): void {
    this.canFloatValue = canSwim;
  }

  public canPassDoors(): boolean {
    return this.canPassDoorsValue;
  }

  public canOpenDoors(): boolean {
    return this.canOpenDoorsValue;
  }

  public canFloat(): boolean {
    return this.canFloatValue;
  }

  protected requireLevel(): PathNavigationRegion {
    if (this.level === undefined) {
      throw new Error("NodeEvaluator used before prepare()");
    }
    return this.level;
  }

  protected requireMob(): PathfinderMob {
    if (this.mob === undefined) {
      throw new Error("NodeEvaluator used before prepare()");
    }
    return this.mob;
  }
}

export function defaultPathfindingMalus(type: BlockPathType): number {
  return type.getMalus();
}

export const DEFAULT_BLOCK_PATH_TYPE = BlockPathTypes.BLOCKED;
