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

  public encapsulate(pos: Vec3i): BoundingBox {
    this.minXValue = Math.min(this.minXValue, pos.getX());
    this.minYValue = Math.min(this.minYValue, pos.getY());
    this.minZValue = Math.min(this.minZValue, pos.getZ());
    this.maxXValue = Math.max(this.maxXValue, pos.getX());
    this.maxYValue = Math.max(this.maxYValue, pos.getY());
    this.maxZValue = Math.max(this.maxZValue, pos.getZ());
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

  public toString(): string {
    return `BoundingBox{minX=${this.minXValue}, minY=${this.minYValue}, minZ=${this.minZValue}, maxX=${this.maxXValue}, maxY=${this.maxYValue}, maxZ=${this.maxZValue}}`;
  }
}
