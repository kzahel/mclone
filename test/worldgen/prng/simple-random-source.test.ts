import { describe, expect, test } from "vitest";
import seed0Fixture from "../../fixtures/prng/seed-0.json";
import seed1Fixture from "../../fixtures/prng/seed-1.json";
import seed12345Fixture from "../../fixtures/prng/seed-12345.json";
import seed2151901553968352745Fixture from "../../fixtures/prng/seed-2151901553968352745.json";
import {
  longPartsToSignedDecimalString,
  SimpleRandomSource,
} from "../../../src/worldgen/prng/simple-random-source";

interface PrngFixture {
  module: string;
  minecraftVersion: string;
  randomSourceClass: string;
  randomSourceAlias: string;
  seed: string;
  count: number;
  sequenceMode: string;
  wireFormat: {
    nextInt: string;
    nextLong: string;
    nextDouble: string;
  };
  nextInt: number[];
  nextLong: string[];
  nextDouble: number[];
}

const fixtures = [
  seed0Fixture,
  seed1Fixture,
  seed12345Fixture,
  seed2151901553968352745Fixture,
] as PrngFixture[];

function expectNumberSequence(
  label: string,
  expected: readonly number[],
  nextValue: () => number,
): void {
  for (const [index, oracle] of expected.entries()) {
    const actual = nextValue();
    if (!Object.is(actual, oracle)) {
      throw new Error(`${label}: mismatch at index ${index}: expected ${oracle}, got ${actual}`);
    }
  }
}

function expectStringSequence(
  label: string,
  expected: readonly string[],
  nextValue: () => string,
): void {
  for (const [index, oracle] of expected.entries()) {
    const actual = nextValue();
    if (actual !== oracle) {
      throw new Error(`${label}: mismatch at index ${index}: expected ${oracle}, got ${actual}`);
    }
  }
}

describe("SimpleRandomSource", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixture of fixtures) {
      expect(fixture.module).toBe("prng");
      expect(fixture.minecraftVersion).toBe("1.17.1");
      expect(fixture.randomSourceClass).toBe("net.minecraft.world.level.levelgen.SimpleRandomSource");
      expect(fixture.randomSourceAlias).toBe("LegacyRandomSource");
      expect(fixture.sequenceMode).toBe("fresh-instance-per-method");
      expect(fixture.wireFormat.nextInt).toBe("number");
      expect(fixture.wireFormat.nextLong).toBe("signed-decimal-string");
      expect(fixture.wireFormat.nextDouble).toBe("number");
    }
  });

  for (const fixture of fixtures) {
    const seed = BigInt(fixture.seed);

    test(`${fixture.seed}: nextInt matches the Java oracle`, () => {
      const random = new SimpleRandomSource(seed);
      expectNumberSequence(`nextInt(seed=${fixture.seed})`, fixture.nextInt, () => random.nextInt());
    });

    test(`${fixture.seed}: nextLong matches the Java oracle`, () => {
      const random = new SimpleRandomSource(seed);
      expectStringSequence(`nextLong(seed=${fixture.seed})`, fixture.nextLong, () =>
        longPartsToSignedDecimalString(random.nextLong()),
      );
    });

    test(`${fixture.seed}: nextDouble matches the Java oracle`, () => {
      const random = new SimpleRandomSource(seed);
      expectNumberSequence(`nextDouble(seed=${fixture.seed})`, fixture.nextDouble, () => random.nextDouble());
    });
  }

  test("setSeed rewinds the sequence", () => {
    const random = new SimpleRandomSource(12345);
    const first = random.nextInt();
    random.nextInt();
    random.setSeed(12345);
    expect(random.nextInt()).toBe(first);
  });

  test("rejects non-positive bounds", () => {
    const random = new SimpleRandomSource(0);
    expect(() => random.nextInt(0)).toThrow("Bound must be positive");
    expect(() => random.nextInt(-1)).toThrow("Bound must be positive");
  });
});
