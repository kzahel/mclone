import type { Vec3i } from "../core/vec3i";

const MULTIPLY_DE_BRUIJN_BIT_POSITION = [
  0, 1, 28, 2, 29, 14, 24, 3, 30, 22, 20, 15, 25, 17, 4, 8, 31, 27, 13, 23, 21, 19, 16, 7, 26, 12, 18, 6, 11, 5, 10, 9,
] as const;

export function smallestEncompassingPowerOfTwo(value: number): number {
  let result = value - 1;
  result |= result >> 1;
  result |= result >> 2;
  result |= result >> 4;
  result |= result >> 8;
  result |= result >> 16;
  return result + 1;
}

export function isPowerOfTwo(value: number): boolean {
  return value !== 0 && (value & (value - 1)) === 0;
}

export function ceillog2(value: number): number {
  const powerOfTwo = isPowerOfTwo(value) ? value : smallestEncompassingPowerOfTwo(value);
  return MULTIPLY_DE_BRUIJN_BIT_POSITION[(Math.imul(powerOfTwo, 125613361) >>> 27) & 31]!;
}

export function log2(value: number): number {
  return ceillog2(value) - (isPowerOfTwo(value) ? 0 : 1);
}

export function lerp(delta: number, start: number, end: number): number {
  return start + (delta * (end - start));
}

export function equal(left: number, right: number): boolean {
  return Math.abs(right - left) < 1.0e-5;
}

export function positiveModulo(value: number, modulus: number): number {
  return ((value % modulus) + modulus) % modulus;
}

export function floor(value: number): number {
  return Math.floor(value);
}

export function intFloorDiv(dividend: number, divisor: number): number {
  return Math.floor(dividend / divisor);
}

export function clamp(value: number, minValue: number, maxValue: number): number {
  if (value < minValue) {
    return minValue;
  }

  return value > maxValue ? maxValue : value;
}

export function getSeed(x: number, y: number, z: number): bigint;
export function getSeed(pos: Vec3i): bigint;
export function getSeed(first: number | Vec3i, second?: number, third?: number): bigint {
  const x = typeof first === "number" ? first : first.getX();
  const y = typeof first === "number" ? second! : first.getY();
  const z = typeof first === "number" ? third! : first.getZ();
  let result = BigInt((x * 3129871) ^ (z * 116129781) ^ y);
  result = (result * result * 42317861n) + (result * 11n);
  return result >> 16n;
}
