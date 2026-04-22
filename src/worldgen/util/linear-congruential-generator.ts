const MULTIPLIER = 6364136223846793005n;
const INCREMENT = 1442695040888963407n;

export function wrapLong(value: bigint): bigint {
  return BigInt.asIntN(64, value);
}

export function linearCongruentialGeneratorNext(left: bigint, right: bigint | number): bigint {
  const wrappedLeft = wrapLong(left);
  const wrappedRight = wrapLong(typeof right === "bigint" ? right : BigInt(right));
  const transformed = wrapLong((wrappedLeft * MULTIPLIER) + INCREMENT);
  return wrapLong((wrappedLeft * transformed) + wrappedRight);
}
