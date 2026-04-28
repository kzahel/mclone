export class Vec3 {
  public static readonly ZERO = new Vec3(0, 0, 0);

  public constructor(
    public readonly x: number,
    public readonly y: number,
    public readonly z: number,
  ) {}

  public add(x: number, y: number, z: number): Vec3;
  public add(other: Vec3): Vec3;
  public add(first: number | Vec3, second?: number, third?: number): Vec3 {
    if (first instanceof Vec3) {
      return new Vec3(this.x + first.x, this.y + first.y, this.z + first.z);
    }

    return new Vec3(this.x + first, this.y + (second ?? 0), this.z + (third ?? 0));
  }

  public subtract(other: Vec3): Vec3 {
    return new Vec3(this.x - other.x, this.y - other.y, this.z - other.z);
  }

  public distanceToSqr(other: Vec3): number {
    const dx = other.x - this.x;
    const dy = other.y - this.y;
    const dz = other.z - this.z;
    return (dx * dx) + (dy * dy) + (dz * dz);
  }

  public distanceTo(other: Vec3): number {
    return Math.sqrt(this.distanceToSqr(other));
  }

  public closerThan(other: Vec3, distance: number): boolean {
    return this.distanceToSqr(other) < distance * distance;
  }

  public dot(other: Vec3): number {
    return (this.x * other.x) + (this.y * other.y) + (this.z * other.z);
  }

  public multiply(x: number, y: number, z: number): Vec3 {
    return new Vec3(this.x * x, this.y * y, this.z * z);
  }

  public scale(value: number): Vec3 {
    return this.multiply(value, value, value);
  }

  public lengthSqr(): number {
    return (this.x * this.x) + (this.y * this.y) + (this.z * this.z);
  }

  public length(): number {
    return Math.sqrt(this.lengthSqr());
  }

  public horizontalDistanceSqr(): number {
    return (this.x * this.x) + (this.z * this.z);
  }

  public horizontalDistance(): number {
    return Math.sqrt(this.horizontalDistanceSqr());
  }
}
