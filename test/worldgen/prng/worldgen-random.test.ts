import { describe, expect, test } from "vitest";
import { longPartsToSignedDecimalString, SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source";
import { WorldgenRandom } from "../../../src/worldgen/prng/worldgen-random";

describe("WorldgenRandom", () => {
  test("matches SimpleRandomSource for the base random sequence", () => {
    const worldgen = new WorldgenRandom(12345n);
    const simple = new SimpleRandomSource(12345n);

    expect(worldgen.nextInt()).toBe(simple.nextInt());
    expect(longPartsToSignedDecimalString(worldgen.nextLong())).toBe(longPartsToSignedDecimalString(simple.nextLong()));
    expect(worldgen.nextBoolean()).toBe(simple.nextBoolean());
    expect(worldgen.nextFloat()).toBe(simple.nextFloat());
    expect(worldgen.nextDouble()).toBe(simple.nextDouble());
  });

  test("tracks Java-style next(...) call counts", () => {
    const random = new WorldgenRandom(0);

    expect(random.getCount()).toBe(0);
    random.nextInt();
    expect(random.getCount()).toBe(1);
    random.nextLong();
    expect(random.getCount()).toBe(3);
    random.nextDouble();
    expect(random.getCount()).toBe(5);
    random.consumeCount(4);
    expect(random.getCount()).toBe(9);
  });

  test("seed derivation helpers mirror the Java formulas", () => {
    const random = new WorldgenRandom(0);
    expect(random.setBaseChunkSeed(2, -3)).toBe(285052294801n);
    expect(random.setLargeFeatureWithSalt(12345n, 4, -7, 20003)).toBe(437206634409n);
  });

  test("slime chunk seeding returns a random source initialized to the Java formula", () => {
    const seeded = WorldgenRandom.seedSlimeChunk(4, -7, 12345n, 987234911n);
    const formulaSeed = BigInt.asIntN(64, (12345n + (16n * 4_987_142n) + (4n * 5_947_611n) + (49n * 4_392_871n) + (-7n * 389_711n)) ^ 987234911n);
    const direct = new WorldgenRandom(formulaSeed);

    expect(seeded.nextInt()).toBe(direct.nextInt());
    expect(seeded.nextDouble()).toBe(direct.nextDouble());
  });
});
