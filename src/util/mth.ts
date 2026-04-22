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
