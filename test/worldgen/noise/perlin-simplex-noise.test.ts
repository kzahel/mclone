import { describe, expect, test } from "vitest";
import seed0Oct0Fixture from "../../fixtures/noise/perlin-simplex-seed-0-oct-0.json";
import seed0Fixture from "../../fixtures/noise/perlin-simplex-seed-0-oct-m3-0.json";
import seed1Oct0Fixture from "../../fixtures/noise/perlin-simplex-seed-1-oct-0.json";
import seed1Fixture from "../../fixtures/noise/perlin-simplex-seed-1-oct-m3-0.json";
import seed12345Oct0Fixture from "../../fixtures/noise/perlin-simplex-seed-12345-oct-0.json";
import seed12345Fixture from "../../fixtures/noise/perlin-simplex-seed-12345-oct-m3-0.json";
import seed2151901553968352745Oct0Fixture from "../../fixtures/noise/perlin-simplex-seed-2151901553968352745-oct-0.json";
import seed2151901553968352745Fixture from "../../fixtures/noise/perlin-simplex-seed-2151901553968352745-oct-m3-0.json";
import { PerlinSimplexNoise } from "../../../src/worldgen/noise/perlin-simplex-noise";
import { SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source";

interface PerlinSimplexSampleSet {
  noiseMethod: string;
  gridOrder: string;
  sampleCount: number;
  x: number[];
  z: number[];
  values: number[];
}

interface PerlinSimplexFixture {
  module: string;
  minecraftVersion: string;
  noiseClass: string;
  randomSourceClass: string;
  randomSourceAlias: string;
  seed: string;
  octaves: number[];
  wireFormat: {
    coordinates: string;
    values: string;
  };
  samplesWithoutOffsets: PerlinSimplexSampleSet;
  samplesWithOffsets: PerlinSimplexSampleSet;
}

const fixtureSets = [
  {
    rangeLabel: "[-3..0]",
    expectedOctaves: [-3, -2, -1, 0],
    fixtures: [seed0Fixture, seed1Fixture, seed12345Fixture, seed2151901553968352745Fixture],
  },
  {
    rangeLabel: "[0]",
    expectedOctaves: [0],
    fixtures: [seed0Oct0Fixture, seed1Oct0Fixture, seed12345Oct0Fixture, seed2151901553968352745Oct0Fixture],
  },
] as const satisfies ReadonlyArray<{
  rangeLabel: string;
  expectedOctaves: readonly number[];
  fixtures: readonly PerlinSimplexFixture[];
}>;

describe("PerlinSimplexNoise", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixtureSet of fixtureSets) {
      for (const fixture of fixtureSet.fixtures) {
        expect(fixture.module).toBe("noise");
        expect(fixture.minecraftVersion).toBe("1.17.1");
        expect(fixture.noiseClass).toBe("net.minecraft.world.level.levelgen.synth.PerlinSimplexNoise");
        expect(fixture.randomSourceClass).toBe("net.minecraft.world.level.levelgen.SimpleRandomSource");
        expect(fixture.randomSourceAlias).toBe("LegacyRandomSource");
        expect(fixture.octaves).toEqual(fixtureSet.expectedOctaves);
        expect(fixture.wireFormat.coordinates).toBe("number");
        expect(fixture.wireFormat.values).toBe("number");

        for (const sampleSet of [fixture.samplesWithoutOffsets, fixture.samplesWithOffsets]) {
          expect(sampleSet.gridOrder).toBe("x-major,z-minor");
          expect(sampleSet.values).toHaveLength(sampleSet.sampleCount);
          expect(sampleSet.sampleCount).toBe(sampleSet.x.length * sampleSet.z.length);
        }

        expect(fixture.samplesWithoutOffsets.noiseMethod).toBe("getValue(x,y,false)");
        expect(fixture.samplesWithOffsets.noiseMethod).toBe("getValue(x,y,true)");
      }
    }
  });

  for (const fixtureSet of fixtureSets) {
    for (const fixture of fixtureSet.fixtures) {
      test(`${fixture.seed} ${fixtureSet.rangeLabel}: matches the Java oracle without offsets`, () => {
        const noise = new PerlinSimplexNoise(new SimpleRandomSource(BigInt(fixture.seed)), fixture.octaves);

        let index = 0;
        for (const x of fixture.samplesWithoutOffsets.x) {
          for (const z of fixture.samplesWithoutOffsets.z) {
            const actual = noise.getValue(x, z, false);
            const oracle = fixture.samplesWithoutOffsets.values[index]!;
            if (!Object.is(actual, oracle)) {
              throw new Error(
                `perlin-simplex(seed=${fixture.seed}, octaves=${fixtureSet.rangeLabel}, useOffsets=false) mismatch at index ${index} for (${x}, ${z}): expected ${oracle}, got ${actual}`,
              );
            }

            index++;
          }
        }
      });

      test(`${fixture.seed} ${fixtureSet.rangeLabel}: matches the Java oracle with offsets`, () => {
        const noise = new PerlinSimplexNoise(new SimpleRandomSource(BigInt(fixture.seed)), fixture.octaves);

        let index = 0;
        for (const x of fixture.samplesWithOffsets.x) {
          for (const z of fixture.samplesWithOffsets.z) {
            const actual = noise.getValue(x, z, true);
            const oracle = fixture.samplesWithOffsets.values[index]!;
            if (!Object.is(actual, oracle)) {
              throw new Error(
                `perlin-simplex(seed=${fixture.seed}, octaves=${fixtureSet.rangeLabel}, useOffsets=true) mismatch at index ${index} for (${x}, ${z}): expected ${oracle}, got ${actual}`,
              );
            }

            index++;
          }
        }
      });
    }
  }

  test("getSurfaceNoiseValue delegates to getValue with offsets and the Java scale factor", () => {
    const noise = new PerlinSimplexNoise(new SimpleRandomSource(12345n), [-3, -2, -1, 0]);
    expect(noise.getSurfaceNoiseValue(0.5, -0.75, 123, 456)).toBe(noise.getValue(0.5, -0.75, true) * 0.55);
  });
});
