import { BlockPos } from "../../../../core/block-pos";
import type { Vec3i } from "../../../../core/vec3i";

export class BoundingBox {
  public constructor(
    private minXValue: number,
    private minYValue: number,
    private minZValue: number,
    private maxXValue: number,
    private maxYValue: number,
    private maxZValue: number,
  ) {
    if (this.maxXValue < this.minXValue || this.maxYValue < this.minYValue || this.maxZValue < this.minZValue) {
      throw new Error(`Invalid bounding box data, inverted bounds for ${this}`);
    }
  }

  public static fromCorners(first: Vec3i, second: Vec3i): BoundingBox {
    return new BoundingBox(
      Math.min(first.getX(), second.getX()),
      Math.min(first.getY(), second.getY()),
      Math.min(first.getZ(), second.getZ()),
      Math.max(first.getX(), second.getX()),
      Math.max(first.getY(), second.getY()),
      Math.max(first.getZ(), second.getZ()),
    );
  }

  public static encapsulatingPositions(positions: Iterable<BlockPos>): BoundingBox | undefined {
    const iterator = positions[Symbol.iterator]();
    const first = iterator.next();
    if (first.done) {
      return undefined;
    }

    const firstPos = first.value;
    const box = new BoundingBox(
      firstPos.getX(),
      firstPos.getY(),
      firstPos.getZ(),
      firstPos.getX(),
      firstPos.getY(),
      firstPos.getZ(),
    );

    for (let next = iterator.next(); !next.done; next = iterator.next()) {
      box.encapsulate(next.value);
    }

    return box;
  }

  public static fromBlockPos(pos: BlockPos): BoundingBox {
    return new BoundingBox(pos.getX(), pos.getY(), pos.getZ(), pos.getX(), pos.getY(), pos.getZ());
  }

  public encapsulate(pos: Vec3i): BoundingBox {
    this.minXValue = Math.min(this.minXValue, pos.getX());
    this.minYValue = Math.min(this.minYValue, pos.getY());
    this.minZValue = Math.min(this.minZValue, pos.getZ());
    this.maxXValue = Math.max(this.maxXValue, pos.getX());
    this.maxYValue = Math.max(this.maxYValue, pos.getY());
    this.maxZValue = Math.max(this.maxZValue, pos.getZ());
    return this;
  }

  public move(x: number, y: number, z: number): BoundingBox;
  public move(vector: Vec3i): BoundingBox;
  public move(first: number | Vec3i, second?: number, third?: number): BoundingBox {
    if (typeof first !== "number") {
      return this.move(first.getX(), first.getY(), first.getZ());
    }

    this.minXValue += first;
    this.minYValue += second ?? 0;
    this.minZValue += third ?? 0;
    this.maxXValue += first;
    this.maxYValue += second ?? 0;
    this.maxZValue += third ?? 0;
    return this;
  }

  public isInside(pos: Vec3i): boolean {
    return (
      pos.getX() >= this.minXValue &&
      pos.getX() <= this.maxXValue &&
      pos.getY() >= this.minYValue &&
      pos.getY() <= this.maxYValue &&
      pos.getZ() >= this.minZValue &&
      pos.getZ() <= this.maxZValue
    );
  }

  public intersects(other: BoundingBox): boolean;
  public intersects(minX: number, minZ: number, maxX: number, maxZ: number): boolean;
  public intersects(first: BoundingBox | number, second?: number, third?: number, fourth?: number): boolean {
    if (first instanceof BoundingBox) {
      return (
        this.minXValue <= first.maxXValue &&
        this.maxXValue >= first.minXValue &&
        this.minYValue <= first.maxYValue &&
        this.maxYValue >= first.minYValue &&
        this.minZValue <= first.maxZValue &&
        this.maxZValue >= first.minZValue
      );
    }

    return this.minXValue <= third!
      && this.maxXValue >= first
      && this.minZValue <= fourth!
      && this.maxZValue >= second!;
  }

  public getXSpan(): number {
    return this.maxXValue - this.minXValue + 1;
  }

  public getYSpan(): number {
    return this.maxYValue - this.minYValue + 1;
  }

  public getZSpan(): number {
    return this.maxZValue - this.minZValue + 1;
  }

  public minX(): number {
    return this.minXValue;
  }

  public minY(): number {
    return this.minYValue;
  }

  public minZ(): number {
    return this.minZValue;
  }

  public maxX(): number {
    return this.maxXValue;
  }

  public maxY(): number {
    return this.maxYValue;
  }

  public maxZ(): number {
    return this.maxZValue;
  }

  public getCenter(): BlockPos {
    return new BlockPos(
      this.minXValue + Math.floor((this.maxXValue - this.minXValue + 1) / 2),
      this.minYValue + Math.floor((this.maxYValue - this.minYValue + 1) / 2),
      this.minZValue + Math.floor((this.maxZValue - this.minZValue + 1) / 2),
    );
  }

  public forAllCorners(consumer: (pos: BlockPos) => void): void {
    const pos = new BlockPos.MutableBlockPos();
    consumer(pos.set(this.maxXValue, this.maxYValue, this.maxZValue));
    consumer(pos.set(this.minXValue, this.maxYValue, this.maxZValue));
    consumer(pos.set(this.maxXValue, this.minYValue, this.maxZValue));
    consumer(pos.set(this.minXValue, this.minYValue, this.maxZValue));
    consumer(pos.set(this.maxXValue, this.maxYValue, this.minZValue));
    consumer(pos.set(this.minXValue, this.maxYValue, this.minZValue));
    consumer(pos.set(this.maxXValue, this.minYValue, this.minZValue));
    consumer(pos.set(this.minXValue, this.minYValue, this.minZValue));
  }

  public toString(): string {
    return `BoundingBox{minX=${this.minXValue}, minY=${this.minYValue}, minZ=${this.minZValue}, maxX=${this.maxXValue}, maxY=${this.maxYValue}, maxZ=${this.maxZValue}}`;
  }
}
