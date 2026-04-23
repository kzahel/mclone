import { BlockPos } from "../../core/block-pos";
import { Vec3 } from "./vec3";

export class AABB {
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
}
