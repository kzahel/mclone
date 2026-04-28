import { BlockPos } from "../../../core/block-pos";
import { Vec3 } from "../../phys/vec3";
import { Node } from "./node";
import type { Target } from "./target";

export interface PathEntity {
  getBbWidth(): number;
}

export class Path {
  private openSet: Node[] = [];
  private closedSet: Node[] = [];
  private targetNodes: Set<Target> | undefined;
  private nextNodeIndex = 0;
  private readonly target: BlockPos;
  private readonly distToTarget: number;
  private readonly reached: boolean;

  public constructor(private readonly nodes: Node[], target: BlockPos, reached: boolean) {
    this.target = target;
    this.distToTarget = nodes.length === 0 ? Number.MAX_VALUE : this.nodes[this.nodes.length - 1]!.distanceManhattan(this.target);
    this.reached = reached;
  }

  public advance(): void {
    this.nextNodeIndex++;
  }

  public notStarted(): boolean {
    return this.nextNodeIndex <= 0;
  }

  public isDone(): boolean {
    return this.nextNodeIndex >= this.nodes.length;
  }

  public getEndNode(): Node | undefined {
    return this.nodes.length > 0 ? this.nodes[this.nodes.length - 1] : undefined;
  }

  public getNode(index: number): Node {
    return this.nodes[index]!;
  }

  public truncateNodes(length: number): void {
    if (this.nodes.length > length) {
      this.nodes.splice(length, this.nodes.length - length);
    }
  }

  public replaceNode(index: number, point: Node): void {
    this.nodes[index] = point;
  }

  public getNodeCount(): number {
    return this.nodes.length;
  }

  public getNextNodeIndex(): number {
    return this.nextNodeIndex;
  }

  public setNextNodeIndex(currentPathIndex: number): void {
    this.nextNodeIndex = currentPathIndex;
  }

  public getEntityPosAtNode(entity: PathEntity, index: number): Vec3 {
    const node = this.nodes[index]!;
    const offset = Math.trunc(entity.getBbWidth() + 1.0) * 0.5;
    return new Vec3(node.x + offset, node.y, node.z + offset);
  }

  public getNodePos(index: number): BlockPos {
    return this.nodes[index]!.asBlockPos();
  }

  public getNextEntityPos(entity: PathEntity): Vec3 {
    return this.getEntityPosAtNode(entity, this.nextNodeIndex);
  }

  public getNextNodePos(): BlockPos {
    return this.nodes[this.nextNodeIndex]!.asBlockPos();
  }

  public getNextNode(): Node {
    return this.nodes[this.nextNodeIndex]!;
  }

  public getPreviousNode(): Node | undefined {
    return this.nextNodeIndex > 0 ? this.nodes[this.nextNodeIndex - 1] : undefined;
  }

  public sameAs(pathentity: Path | undefined): boolean {
    if (pathentity === undefined) {
      return false;
    }
    if (pathentity.nodes.length !== this.nodes.length) {
      return false;
    }
    for (let i = 0; i < this.nodes.length; i++) {
      const left = this.nodes[i]!;
      const right = pathentity.nodes[i]!;
      if (left.x !== right.x || left.y !== right.y || left.z !== right.z) {
        return false;
      }
    }
    return true;
  }

  public canReach(): boolean {
    return this.reached;
  }

  public setDebug(openSet: Node[], closedSet: Node[], targetNodes: Set<Target>): void {
    this.openSet = openSet;
    this.closedSet = closedSet;
    this.targetNodes = targetNodes;
  }

  public getOpenSet(): readonly Node[] {
    return this.openSet;
  }

  public getClosedSet(): readonly Node[] {
    return this.closedSet;
  }

  public getTargetNodes(): ReadonlySet<Target> | undefined {
    return this.targetNodes;
  }

  public toString(): string {
    return `Path(length=${this.nodes.length})`;
  }

  public getTarget(): BlockPos {
    return this.target;
  }

  public getDistToTarget(): number {
    return this.distToTarget;
  }
}
