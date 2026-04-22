import { describe, expect, test } from "vitest";
import seed0Fixture from "../../fixtures/noise/seed-0.json";
import seed1Fixture from "../../fixtures/noise/seed-1.json";
import seed12345Fixture from "../../fixtures/noise/seed-12345.json";
import seed2151901553968352745Fixture from "../../fixtures/noise/seed-2151901553968352745.json";
import { ImprovedNoise } from "../../../src/worldgen/noise/improved-noise";
import { SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source";

interface NoiseFixture {
  module: string;
  minecraftVersion: string;
  noiseClass: string;
  randomSourceClass: string;
  randomSourceAlias: string;
  noiseMethod: string;
  gridOrder: string;
  seed: string;
  sampleCount: number;
  wireFormat: {
    coordinates: string;
    values: string;
  };
  offsets: {
    xo: number;
    yo: number;
    zo: number;
  };
  x: number[];
  y: number[];
  z: number[];
  values: number[];
}

const fixtures = [
  seed0Fixture,
  seed1Fixture,
  seed12345Fixture,
  seed2151901553968352745Fixture,
] as NoiseFixture[];

describe("ImprovedNoise", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixture of fixtures) {
      expect(fixture.module).toBe("noise");
      expect(fixture.minecraftVersion).toBe("1.17.1");
      expect(fixture.noiseClass).toBe("net.minecraft.world.level.levelgen.synth.ImprovedNoise");
      expect(fixture.randomSourceClass).toBe("net.minecraft.world.level.levelgen.SimpleRandomSource");
      expect(fixture.randomSourceAlias).toBe("LegacyRandomSource");
      expect(fixture.noiseMethod).toBe("noise(x,y,z)");
      expect(fixture.gridOrder).toBe("x-major,y-major,z-minor");
      expect(fixture.wireFormat.coordinates).toBe("number");
      expect(fixture.wireFormat.values).toBe("number");
      expect(fixture.values).toHaveLength(fixture.sampleCount);
      expect(fixture.sampleCount).toBe(fixture.x.length * fixture.y.length * fixture.z.length);
    }
  });

  for (const fixture of fixtures) {
    test(`${fixture.seed}: matches the Java oracle across the fixed grid`, () => {
      const noise = new ImprovedNoise(new SimpleRandomSource(BigInt(fixture.seed)));
      expect(noise.xo).toBe(fixture.offsets.xo);
      expect(noise.yo).toBe(fixture.offsets.yo);
      expect(noise.zo).toBe(fixture.offsets.zo);

      let index = 0;
      for (const x of fixture.x) {
        for (const y of fixture.y) {
          for (const z of fixture.z) {
            const actual = noise.getValue(x, y, z);
            const oracle = fixture.values[index]!;
            if (!Object.is(actual, oracle)) {
              throw new Error(
                `noise(seed=${fixture.seed}) mismatch at index ${index} for (${x}, ${y}, ${z}): expected ${oracle}, got ${actual}`,
              );
            }

            index++;
          }
        }
      }
    });
  }

  test("getValue delegates to the default noise path", () => {
    const noise = new ImprovedNoise(new SimpleRandomSource(12345));
    expect(noise.getValue(0.5, -0.75, 1.25)).toBe(noise.noise(0.5, -0.75, 1.25));
  });
});
