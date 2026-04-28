import { BlockPos } from "../../../core/block-pos";
import { Vec3 } from "../../phys/vec3";
import { BlockPathTypes, type BlockPathTypes as BlockPathType } from "./block-path-types";

export class Node {
  public readonly x: number;
  public readonly y: number;
  public readonly z: number;
  private readonly hash: number;
  public heapIdx = -1;
  public g = 0.0;
  public h = 0.0;
  public f = 0.0;
  public cameFrom: Node | undefined;
  public closed = false;
  public walkedDistance = 0.0;
  public costMalus = 0.0;
  public type: BlockPathType = BlockPathTypes.BLOCKED;

  public constructor(x: number, y: number, z: number) {
    this.x = x;
    this.y = y;
    this.z = z;
    this.hash = Node.createHash(x, y, z);
  }

  public cloneAndMove(x: number, y: number, z: number): Node {
    const node = new Node(x, y, z);
    node.heapIdx = this.heapIdx;
    node.g = this.g;
    node.h = this.h;
    node.f = this.f;
    node.cameFrom = this.cameFrom;
    node.closed = this.closed;
    node.walkedDistance = this.walkedDistance;
    node.costMalus = this.costMalus;
    node.type = this.type;
    return node;
  }

  public static createHash(x: number, y: number, z: number): number {
    return (y & 0xff)
      | ((x & 32767) << 8)
      | ((z & 32767) << 24)
      | (x < 0 ? 0x80000000 : 0)
      | (z < 0 ? 32768 : 0);
  }

  public distanceTo(pathpoint: Node | BlockPos): number {
    const x = pathpoint instanceof Node ? pathpoint.x : pathpoint.getX();
    const y = pathpoint instanceof Node ? pathpoint.y : pathpoint.getY();
    const z = pathpoint instanceof Node ? pathpoint.z : pathpoint.getZ();
    const dx = x - this.x;
    const dy = y - this.y;
    const dz = z - this.z;
    return Math.sqrt((dx * dx) + (dy * dy) + (dz * dz));
  }

  public distanceToSqr(pathpoint: Node | BlockPos): number {
    const x = pathpoint instanceof Node ? pathpoint.x : pathpoint.getX();
    const y = pathpoint instanceof Node ? pathpoint.y : pathpoint.getY();
    const z = pathpoint instanceof Node ? pathpoint.z : pathpoint.getZ();
    const dx = x - this.x;
    const dy = y - this.y;
    const dz = z - this.z;
    return (dx * dx) + (dy * dy) + (dz * dz);
  }

  public distanceManhattan(pathpoint: Node | BlockPos): number {
    const x = pathpoint instanceof Node ? pathpoint.x : pathpoint.getX();
    const y = pathpoint instanceof Node ? pathpoint.y : pathpoint.getY();
    const z = pathpoint instanceof Node ? pathpoint.z : pathpoint.getZ();
    return Math.abs(x - this.x) + Math.abs(y - this.y) + Math.abs(z - this.z);
  }

  public asBlockPos(): BlockPos {
    return new BlockPos(this.x, this.y, this.z);
  }

  public asVec3(): Vec3 {
    return new Vec3(this.x, this.y, this.z);
  }

  public equals(other: unknown): boolean {
    return other instanceof Node
      && this.hash === other.hash
      && this.x === other.x
      && this.y === other.y
      && this.z === other.z;
  }

  public hashCode(): number {
    return this.hash;
  }

  public inOpenSet(): boolean {
    return this.heapIdx >= 0;
  }

  public toString(): string {
    return `Node{x=${this.x}, y=${this.y}, z=${this.z}}`;
  }
}
