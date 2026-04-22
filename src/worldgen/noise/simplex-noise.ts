import type { NoiseRandomSource } from "./improved-noise";

const GRADIENTS = [
  [1, 1, 0],
  [-1, 1, 0],
  [1, -1, 0],
  [-1, -1, 0],
  [1, 0, 1],
  [-1, 0, 1],
  [1, 0, -1],
  [-1, 0, -1],
  [0, 1, 1],
  [0, -1, 1],
  [0, 1, -1],
  [0, -1, -1],
  [1, 1, 0],
  [0, -1, 1],
  [-1, 1, 0],
  [0, -1, -1],
] as const;

const SQRT_3 = Math.sqrt(3.0);
const F2 = 0.5 * (SQRT_3 - 1.0);
const G2 = (3.0 - SQRT_3) / 6.0;
const F3 = 0.3333333333333333;
const G3 = 0.16666666666666666;

function dot(gradientIndex: number, x: number, y: number, z: number): number {
  const gradient = GRADIENTS[gradientIndex]!;
  return (gradient[0] * x) + (gradient[1] * y) + (gradient[2] * z);
}

export class SimplexNoise {
  private readonly p: Uint8Array;
  public readonly xo: number;
  public readonly yo: number;
  public readonly zo: number;

  public constructor(random: NoiseRandomSource) {
    this.xo = random.nextDouble() * 256;
    this.yo = random.nextDouble() * 256;
    this.zo = random.nextDouble() * 256;
    this.p = new Uint8Array(256);

    for (let index = 0; index < 256; index++) {
      this.p[index] = index;
    }

    for (let index = 0; index < 256; index++) {
      const offset = random.nextInt(256 - index);
      const current = this.p[index]!;
      this.p[index] = this.p[index + offset]!;
      this.p[index + offset] = current;
    }
  }

  public getValue(x: number, z: number): number;
  public getValue(x: number, y: number, z: number): number;
  public getValue(x: number, yOrZ: number, z?: number): number {
    if (z === undefined) {
      return this.getValue2D(x, yOrZ);
    }

    return this.getValue3D(x, yOrZ, z);
  }

  private permutation(index: number): number {
    return this.p[index & 0xff]!;
  }

  private getCornerNoise3D(gradientIndex: number, x: number, y: number, z: number, offset: number): number {
    let value = offset - (x * x) - (y * y) - (z * z);
    if (value < 0) {
      return 0;
    }

    value *= value;
    return value * value * dot(gradientIndex, x, y, z);
  }

  private getValue2D(x: number, z: number): number {
    const skew = (x + z) * F2;
    const cellX = Math.floor(x + skew);
    const cellZ = Math.floor(z + skew);
    const unskew = (cellX + cellZ) * G2;
    const cellOriginX = cellX - unskew;
    const cellOriginZ = cellZ - unskew;
    const localX = x - cellOriginX;
    const localZ = z - cellOriginZ;

    let offsetX = 0;
    let offsetZ = 0;
    if (localX > localZ) {
      offsetX = 1;
    } else {
      offsetZ = 1;
    }

    const secondCornerX = localX - offsetX + G2;
    const secondCornerZ = localZ - offsetZ + G2;
    const thirdCornerX = localX - 1 + (2 * G2);
    const thirdCornerZ = localZ - 1 + (2 * G2);
    const permX = cellX & 0xff;
    const permZ = cellZ & 0xff;
    const gradient0 = this.permutation(permX + this.permutation(permZ)) % 12;
    const gradient1 = this.permutation(permX + offsetX + this.permutation(permZ + offsetZ)) % 12;
    const gradient2 = this.permutation(permX + 1 + this.permutation(permZ + 1)) % 12;
    const corner0 = this.getCornerNoise3D(gradient0, localX, localZ, 0, 0.5);
    const corner1 = this.getCornerNoise3D(gradient1, secondCornerX, secondCornerZ, 0, 0.5);
    const corner2 = this.getCornerNoise3D(gradient2, thirdCornerX, thirdCornerZ, 0, 0.5);
    return 70 * (corner0 + corner1 + corner2);
  }

  private getValue3D(x: number, y: number, z: number): number {
    const skew = (x + y + z) * F3;
    const cellX = Math.floor(x + skew);
    const cellY = Math.floor(y + skew);
    const cellZ = Math.floor(z + skew);
    const unskew = (cellX + cellY + cellZ) * G3;
    const cellOriginX = cellX - unskew;
    const cellOriginY = cellY - unskew;
    const cellOriginZ = cellZ - unskew;
    const localX = x - cellOriginX;
    const localY = y - cellOriginY;
    const localZ = z - cellOriginZ;

    let offset0X = 0;
    let offset0Y = 0;
    let offset0Z = 0;
    let offset1X = 0;
    let offset1Y = 0;
    let offset1Z = 0;

    if (localX >= localY) {
      if (localY >= localZ) {
        offset0X = 1;
        offset1X = 1;
        offset1Y = 1;
      } else if (localX >= localZ) {
        offset0X = 1;
        offset1X = 1;
        offset1Z = 1;
      } else {
        offset0Z = 1;
        offset1X = 1;
        offset1Z = 1;
      }
    } else if (localY < localZ) {
      offset0Z = 1;
      offset1Y = 1;
      offset1Z = 1;
    } else if (localX < localZ) {
      offset0Y = 1;
      offset1Y = 1;
      offset1Z = 1;
    } else {
      offset0Y = 1;
      offset1X = 1;
      offset1Y = 1;
    }

    const secondCornerX = localX - offset0X + G3;
    const secondCornerY = localY - offset0Y + G3;
    const secondCornerZ = localZ - offset0Z + G3;
    const thirdCornerX = localX - offset1X + (2 * G3);
    const thirdCornerY = localY - offset1Y + (2 * G3);
    const thirdCornerZ = localZ - offset1Z + (2 * G3);
    const fourthCornerX = localX - 1 + 0.5;
    const fourthCornerY = localY - 1 + 0.5;
    const fourthCornerZ = localZ - 1 + 0.5;
    const permX = cellX & 0xff;
    const permY = cellY & 0xff;
    const permZ = cellZ & 0xff;
    const gradient0 = this.permutation(permX + this.permutation(permY + this.permutation(permZ))) % 12;
    const gradient1 =
      this.permutation(permX + offset0X + this.permutation(permY + offset0Y + this.permutation(permZ + offset0Z))) % 12;
    const gradient2 =
      this.permutation(permX + offset1X + this.permutation(permY + offset1Y + this.permutation(permZ + offset1Z))) % 12;
    const gradient3 = this.permutation(permX + 1 + this.permutation(permY + 1 + this.permutation(permZ + 1))) % 12;
    const corner0 = this.getCornerNoise3D(gradient0, localX, localY, localZ, 0.6);
    const corner1 = this.getCornerNoise3D(gradient1, secondCornerX, secondCornerY, secondCornerZ, 0.6);
    const corner2 = this.getCornerNoise3D(gradient2, thirdCornerX, thirdCornerY, thirdCornerZ, 0.6);
    const corner3 = this.getCornerNoise3D(gradient3, fourthCornerX, fourthCornerY, fourthCornerZ, 0.6);
    return 32 * (corner0 + corner1 + corner2 + corner3);
  }
}
