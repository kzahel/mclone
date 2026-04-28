import { BlockPos } from "../../core/block-pos";
import { Vec3 } from "./vec3";

export class AABB {
  private static readonly EPSILON = 1.0e-7;

  public readonly minX: number;
  public readonly minY: number;
  public readonly minZ: number;
  public readonly maxX: number;
  public readonly maxY: number;
  public readonly maxZ: number;

  public constructor(minX: number, minY: number, minZ: number, maxX: number, maxY: number, maxZ: number);
  public constructor(min: BlockPos, max: BlockPos);
  public constructor(min: Vec3, max: Vec3);
  public constructor(
    minXOrMin: number | BlockPos | Vec3,
    minYOrMax: number | BlockPos | Vec3,
    minZ?: number,
    maxX?: number,
    maxY?: number,
    maxZ?: number,
  ) {
    if (typeof minXOrMin === "number" && typeof minYOrMax === "number") {
      this.minX = Math.min(minXOrMin, maxX!);
      this.minY = Math.min(minYOrMax, maxY!);
      this.minZ = Math.min(minZ!, maxZ!);
      this.maxX = Math.max(minXOrMin, maxX!);
      this.maxY = Math.max(minYOrMax, maxY!);
      this.maxZ = Math.max(minZ!, maxZ!);
      return;
    }

    if (minXOrMin instanceof BlockPos && minYOrMax instanceof BlockPos) {
      this.minX = Math.min(minXOrMin.getX(), minYOrMax.getX());
      this.minY = Math.min(minXOrMin.getY(), minYOrMax.getY());
      this.minZ = Math.min(minXOrMin.getZ(), minYOrMax.getZ());
      this.maxX = Math.max(minXOrMin.getX(), minYOrMax.getX());
      this.maxY = Math.max(minXOrMin.getY(), minYOrMax.getY());
      this.maxZ = Math.max(minXOrMin.getZ(), minYOrMax.getZ());
      return;
    }

    const min = minXOrMin as Vec3;
    const max = minYOrMax as Vec3;
    this.minX = Math.min(min.x, max.x);
    this.minY = Math.min(min.y, max.y);
    this.minZ = Math.min(min.z, max.z);
    this.maxX = Math.max(min.x, max.x);
    this.maxY = Math.max(min.y, max.y);
    this.maxZ = Math.max(min.z, max.z);
  }

  public static unitCubeFromLowerCorner(pos: Vec3): AABB {
    return new AABB(pos.x, pos.y, pos.z, pos.x + 1.0, pos.y + 1.0, pos.z + 1.0);
  }

  public static block(pos: BlockPos): AABB {
    return new AABB(pos.getX(), pos.getY(), pos.getZ(), pos.getX() + 1, pos.getY() + 1, pos.getZ() + 1);
  }

  public static ofSize(center: Vec3, xSize: number, ySize: number, zSize: number): AABB {
    return new AABB(
      center.x - (xSize / 2.0),
      center.y - (ySize / 2.0),
      center.z - (zSize / 2.0),
      center.x + (xSize / 2.0),
      center.y + (ySize / 2.0),
      center.z + (zSize / 2.0),
    );
  }

  public contract(x: number, y: number, z: number): AABB {
    let minX = this.minX;
    let minY = this.minY;
    let minZ = this.minZ;
    let maxX = this.maxX;
    let maxY = this.maxY;
    let maxZ = this.maxZ;
    if (x < 0.0) {
      minX -= x;
    } else if (x > 0.0) {
      maxX -= x;
    }

    if (y < 0.0) {
      minY -= y;
    } else if (y > 0.0) {
      maxY -= y;
    }

    if (z < 0.0) {
      minZ -= z;
    } else if (z > 0.0) {
      maxZ -= z;
    }

    return new AABB(minX, minY, minZ, maxX, maxY, maxZ);
  }

  public expandTowards(vec: Vec3): AABB;
  public expandTowards(x: number, y: number, z: number): AABB;
  public expandTowards(first: number | Vec3, second?: number, third?: number): AABB {
    const x = first instanceof Vec3 ? first.x : first;
    const y = first instanceof Vec3 ? first.y : second!;
    const z = first instanceof Vec3 ? first.z : third!;
    let minX = this.minX;
    let minY = this.minY;
    let minZ = this.minZ;
    let maxX = this.maxX;
    let maxY = this.maxY;
    let maxZ = this.maxZ;
    if (x < 0.0) {
      minX += x;
    } else if (x > 0.0) {
      maxX += x;
    }

    if (y < 0.0) {
      minY += y;
    } else if (y > 0.0) {
      maxY += y;
    }

    if (z < 0.0) {
      minZ += z;
    } else if (z > 0.0) {
      maxZ += z;
    }

    return new AABB(minX, minY, minZ, maxX, maxY, maxZ);
  }

  public inflate(value: number): AABB;
  public inflate(x: number, y: number, z: number): AABB;
  public inflate(first: number, second?: number, third?: number): AABB {
    const x = first;
    const y = second ?? first;
    const z = third ?? first;
    return new AABB(this.minX - x, this.minY - y, this.minZ - z, this.maxX + x, this.maxY + y, this.maxZ + z);
  }

  public deflate(value: number): AABB;
  public deflate(x: number, y: number, z: number): AABB;
  public deflate(first: number, second?: number, third?: number): AABB {
    return second === undefined ? this.inflate(-first) : this.inflate(-first, -second, -(third ?? second));
  }

  public intersect(other: AABB): AABB {
    return new AABB(
      Math.max(this.minX, other.minX),
      Math.max(this.minY, other.minY),
      Math.max(this.minZ, other.minZ),
      Math.min(this.maxX, other.maxX),
      Math.min(this.maxY, other.maxY),
      Math.min(this.maxZ, other.maxZ),
    );
  }

  public minmax(other: AABB): AABB {
    return new AABB(
      Math.min(this.minX, other.minX),
      Math.min(this.minY, other.minY),
      Math.min(this.minZ, other.minZ),
      Math.max(this.maxX, other.maxX),
      Math.max(this.maxY, other.maxY),
      Math.max(this.maxZ, other.maxZ),
    );
  }

  public move(vec: Vec3): AABB;
  public move(pos: BlockPos): AABB;
  public move(x: number, y: number, z: number): AABB;
  public move(first: number | BlockPos | Vec3, second?: number, third?: number): AABB {
    const x = typeof first === "number" ? first : first instanceof Vec3 ? first.x : first.getX();
    const y = typeof first === "number" ? second! : first instanceof Vec3 ? first.y : first.getY();
    const z = typeof first === "number" ? third! : first instanceof Vec3 ? first.z : first.getZ();
    return new AABB(this.minX + x, this.minY + y, this.minZ + z, this.maxX + x, this.maxY + y, this.maxZ + z);
  }

  public intersects(other: AABB): boolean;
  public intersects(minX: number, minY: number, minZ: number, maxX: number, maxY: number, maxZ: number): boolean;
  public intersects(first: AABB | number, second?: number, third?: number, fourth?: number, fifth?: number, sixth?: number): boolean {
    if (first instanceof AABB) {
      return this.intersects(first.minX, first.minY, first.minZ, first.maxX, first.maxY, first.maxZ);
    }

    return this.minX < fourth!
      && this.maxX > first
      && this.minY < fifth!
      && this.maxY > second!
      && this.minZ < sixth!
      && this.maxZ > third!;
  }

  public contains(x: number, y: number, z: number): boolean;
  public contains(pos: Vec3): boolean;
  public contains(first: number | Vec3, second?: number, third?: number): boolean {
    const x = first instanceof Vec3 ? first.x : first;
    const y = first instanceof Vec3 ? first.y : second!;
    const z = first instanceof Vec3 ? first.z : third!;
    return x >= this.minX && x < this.maxX && y >= this.minY && y < this.maxY && z >= this.minZ && z < this.maxZ;
  }

  public getXsize(): number {
    return this.maxX - this.minX;
  }

  public getYsize(): number {
    return this.maxY - this.minY;
  }

  public getZsize(): number {
    return this.maxZ - this.minZ;
  }

  public getSize(): number {
    return (this.getXsize() + this.getYsize() + this.getZsize()) / 3.0;
  }

  public getCenter(): Vec3 {
    return new Vec3((this.minX + this.maxX) * 0.5, (this.minY + this.maxY) * 0.5, (this.minZ + this.maxZ) * 0.5);
  }

  public equals(other: AABB): boolean {
    return this.minX === other.minX
      && this.minY === other.minY
      && this.minZ === other.minZ
      && this.maxX === other.maxX
      && this.maxY === other.maxY
      && this.maxZ === other.maxZ;
  }

  public hasNaN(): boolean {
    return Number.isNaN(this.minX)
      || Number.isNaN(this.minY)
      || Number.isNaN(this.minZ)
      || Number.isNaN(this.maxX)
      || Number.isNaN(this.maxY)
      || Number.isNaN(this.maxZ);
  }

  public toString(): string {
    return `AABB[${this.minX}, ${this.minY}, ${this.minZ}] -> [${this.maxX}, ${this.maxY}, ${this.maxZ}]`;
  }

  public static epsilon(): number {
    return AABB.EPSILON;
  }
}
