import { describe, expect, test } from "vitest";
import seed0WideFixture from "../../fixtures/noise/perlin-seed-0-oct-m15-0.json";
import seed0Fixture from "../../fixtures/noise/perlin-seed-0-oct-m7-0.json";
import seed1WideFixture from "../../fixtures/noise/perlin-seed-1-oct-m15-0.json";
import seed1Fixture from "../../fixtures/noise/perlin-seed-1-oct-m7-0.json";
import seed12345WideFixture from "../../fixtures/noise/perlin-seed-12345-oct-m15-0.json";
import seed12345Fixture from "../../fixtures/noise/perlin-seed-12345-oct-m7-0.json";
import seed2151901553968352745WideFixture from "../../fixtures/noise/perlin-seed-2151901553968352745-oct-m15-0.json";
import seed2151901553968352745Fixture from "../../fixtures/noise/perlin-seed-2151901553968352745-oct-m7-0.json";
import { PerlinNoise } from "../../../src/worldgen/noise/perlin-noise";
import { SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source";

interface PerlinFixture {
  module: string;
  minecraftVersion: string;
  noiseClass: string;
  randomSourceClass: string;
  randomSourceAlias: string;
  noiseMethod: string;
  gridOrder: string;
  seed: string;
  sampleCount: number;
  octaves: number[];
  wireFormat: {
    coordinates: string;
    values: string;
  };
  x: number[];
  y: number[];
  z: number[];
  values: number[];
}

const fixtureSets = [
  {
    rangeLabel: "[-7..0]",
    expectedOctaves: [-7, -6, -5, -4, -3, -2, -1, 0],
    fixtures: [seed0Fixture, seed1Fixture, seed12345Fixture, seed2151901553968352745Fixture],
  },
  {
    rangeLabel: "[-15..0]",
    expectedOctaves: [-15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0],
    fixtures: [seed0WideFixture, seed1WideFixture, seed12345WideFixture, seed2151901553968352745WideFixture],
  },
] as const satisfies ReadonlyArray<{
  rangeLabel: string;
  expectedOctaves: readonly number[];
  fixtures: readonly PerlinFixture[];
}>;

describe("PerlinNoise", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixtureSet of fixtureSets) {
      for (const fixture of fixtureSet.fixtures) {
        expect(fixture.module).toBe("noise");
        expect(fixture.minecraftVersion).toBe("1.17.1");
        expect(fixture.noiseClass).toBe("net.minecraft.world.level.levelgen.synth.PerlinNoise");
        expect(fixture.randomSourceClass).toBe("net.minecraft.world.level.levelgen.SimpleRandomSource");
        expect(fixture.randomSourceAlias).toBe("LegacyRandomSource");
        expect(fixture.noiseMethod).toBe("getValue(x,y,z)");
        expect(fixture.gridOrder).toBe("x-major,y-major,z-minor");
        expect(fixture.octaves).toEqual(fixtureSet.expectedOctaves);
        expect(fixture.wireFormat.coordinates).toBe("number");
        expect(fixture.wireFormat.values).toBe("number");
        expect(fixture.values).toHaveLength(fixture.sampleCount);
        expect(fixture.sampleCount).toBe(fixture.x.length * fixture.y.length * fixture.z.length);
      }
    }
  });

  for (const fixtureSet of fixtureSets) {
    for (const fixture of fixtureSet.fixtures) {
      test(`${fixture.seed} ${fixtureSet.rangeLabel}: matches the Java oracle across the shared sample grid`, () => {
        const noise = new PerlinNoise(new SimpleRandomSource(BigInt(fixture.seed)), fixture.octaves);

        let index = 0;
        for (const x of fixture.x) {
          for (const y of fixture.y) {
            for (const z of fixture.z) {
              const actual = noise.getValue(x, y, z);
              const oracle = fixture.values[index]!;
              if (!Object.is(actual, oracle)) {
                throw new Error(
                  `perlin(seed=${fixture.seed}, octaves=${fixtureSet.rangeLabel}) mismatch at index ${index} for (${x}, ${y}, ${z}): expected ${oracle}, got ${actual}`,
                );
              }

              index++;
            }
          }
        }
      });
    }
  }

  test("default getValue delegates to the explicit default arguments", () => {
    const noise = new PerlinNoise(new SimpleRandomSource(12345), [-7, -6, -5, -4, -3, -2, -1, 0]);
    expect(noise.getValue(0.5, -0.75, 1.25)).toBe(noise.getValue(0.5, -0.75, 1.25, 0, 0, false));
  });

  test("getSurfaceNoiseValue preserves Java's argument remapping", () => {
    const noise = new PerlinNoise(new SimpleRandomSource(12345), [-7, -6, -5, -4, -3, -2, -1, 0]);
    expect(noise.getSurfaceNoiseValue(0.5, -0.75, 1.25, 2.5)).toBe(noise.getValue(0.5, -0.75, 0, 1.25, 2.5, false));
  });

  test("create uses explicit amplitudes instead of octave presence", () => {
    const seed = 12345n;
    const explicit = PerlinNoise.create(new SimpleRandomSource(seed), -3, [1, 0, 2]);
    const manual = new PerlinNoise(new SimpleRandomSource(seed), -3, [1, 0, 2]);

    expect(explicit.getValue(0.25, -0.5, 0.75)).toBe(manual.getValue(0.25, -0.5, 0.75));
  });
});
