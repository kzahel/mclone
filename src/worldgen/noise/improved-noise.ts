const SHIFT_UP_EPSILON = Math.fround(1.0e-7);
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

export interface NoiseRandomSource {
  nextDouble(): number;
  nextInt(bound: number): number;
}

function lerp(delta: number, start: number, end: number): number {
  return start + (delta * (end - start));
}

function lerp2(
  deltaX: number,
  deltaY: number,
  x0y0: number,
  x1y0: number,
  x0y1: number,
  x1y1: number,
): number {
  return lerp(deltaY, lerp(deltaX, x0y0, x1y0), lerp(deltaX, x0y1, x1y1));
}

function lerp3(
  deltaX: number,
  deltaY: number,
  deltaZ: number,
  x0y0z0: number,
  x1y0z0: number,
  x0y1z0: number,
  x1y1z0: number,
  x0y0z1: number,
  x1y0z1: number,
  x0y1z1: number,
  x1y1z1: number,
): number {
  return lerp(
    deltaZ,
    lerp2(deltaX, deltaY, x0y0z0, x1y0z0, x0y1z0, x1y1z0),
    lerp2(deltaX, deltaY, x0y0z1, x1y0z1, x0y1z1, x1y1z1),
  );
}

function smoothstep(value: number): number {
  return value * value * value * ((value * ((value * 6) - 15)) + 10);
}

function gradDot(gradientIndex: number, xFactor: number, yFactor: number, zFactor: number): number {
  const gradient = GRADIENTS[gradientIndex & 15]!;
  return (gradient[0] * xFactor) + (gradient[1] * yFactor) + (gradient[2] * zFactor);
}

export class ImprovedNoise {
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

  public getValue(x: number, y: number, z: number): number {
    return this.noise(x, y, z);
  }

  public noise(x: number, y: number, z: number, yScale = 0, yMax = 0): number {
    const shiftedX = x + this.xo;
    const shiftedY = y + this.yo;
    const shiftedZ = z + this.zo;
    const gridX = Math.floor(shiftedX);
    const gridY = Math.floor(shiftedY);
    const gridZ = Math.floor(shiftedZ);
    const deltaX = shiftedX - gridX;
    const deltaY = shiftedY - gridY;
    const deltaZ = shiftedZ - gridZ;

    let yShift = 0;
    if (yScale !== 0) {
      const cappedY = yMax >= 0 && yMax < deltaY ? yMax : deltaY;
      yShift = Math.floor((cappedY / yScale) + SHIFT_UP_EPSILON) * yScale;
    }

    return this.sampleAndLerp(gridX, gridY, gridZ, deltaX, deltaY - yShift, deltaZ, deltaY);
  }

  private permutation(index: number): number {
    return this.p[index & 0xff]!;
  }

  private sampleAndLerp(
    gridX: number,
    gridY: number,
    gridZ: number,
    deltaX: number,
    weirdDeltaY: number,
    deltaZ: number,
    deltaY: number,
  ): number {
    const permX0 = this.permutation(gridX);
    const permX1 = this.permutation(gridX + 1);
    const permX0Y0 = this.permutation(permX0 + gridY);
    const permX0Y1 = this.permutation(permX0 + gridY + 1);
    const permX1Y0 = this.permutation(permX1 + gridY);
    const permX1Y1 = this.permutation(permX1 + gridY + 1);

    const x0y0z0 = gradDot(this.permutation(permX0Y0 + gridZ), deltaX, weirdDeltaY, deltaZ);
    const x1y0z0 = gradDot(this.permutation(permX1Y0 + gridZ), deltaX - 1, weirdDeltaY, deltaZ);
    const x0y1z0 = gradDot(this.permutation(permX0Y1 + gridZ), deltaX, weirdDeltaY - 1, deltaZ);
    const x1y1z0 = gradDot(this.permutation(permX1Y1 + gridZ), deltaX - 1, weirdDeltaY - 1, deltaZ);
    const x0y0z1 = gradDot(this.permutation(permX0Y0 + gridZ + 1), deltaX, weirdDeltaY, deltaZ - 1);
    const x1y0z1 = gradDot(this.permutation(permX1Y0 + gridZ + 1), deltaX - 1, weirdDeltaY, deltaZ - 1);
    const x0y1z1 = gradDot(this.permutation(permX0Y1 + gridZ + 1), deltaX, weirdDeltaY - 1, deltaZ - 1);
    const x1y1z1 = gradDot(this.permutation(permX1Y1 + gridZ + 1), deltaX - 1, weirdDeltaY - 1, deltaZ - 1);

    return lerp3(
      smoothstep(deltaX),
      smoothstep(deltaY),
      smoothstep(deltaZ),
      x0y0z0,
      x1y0z0,
      x0y1z0,
      x1y1z0,
      x0y0z1,
      x1y0z1,
      x0y1z1,
      x1y1z1,
    );
  }
}
