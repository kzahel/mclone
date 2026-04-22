const SIN_SCALE = Math.fround(10430.378);
const COS_OFFSET = Math.fround(16384.0);
const TWO_PI = Math.PI * 2.0;
const SIN_TABLE = new Float32Array(65536);

for (let index = 0; index < SIN_TABLE.length; index++) {
  SIN_TABLE[index] = Math.fround(Math.sin((index * TWO_PI) / SIN_TABLE.length));
}

export function f32(value: number): number {
  return Math.fround(value);
}

export function floor(value: number): number {
  const truncated = Math.trunc(value);
  return value < truncated ? truncated - 1 : truncated;
}

export function clamp(value: number, minValue: number, maxValue: number): number {
  if (value < minValue) {
    return minValue;
  }

  return value > maxValue ? maxValue : value;
}

export function sin(value: number): number {
  return SIN_TABLE[(Math.trunc(Math.fround(value) * SIN_SCALE)) & 0xffff]!;
}

export function cos(value: number): number {
  return SIN_TABLE[(Math.trunc((Math.fround(value) * SIN_SCALE) + COS_OFFSET)) & 0xffff]!;
}

export function randomBetween(random: { nextFloat(): number }, minValue: number, maxValue: number): number {
  return f32((random.nextFloat() * (maxValue - minValue)) + minValue);
}
