import { describe, expect, test } from "vitest";
import seed0Fixture from "../../fixtures/noise/blended-seed-0.json";
import seed1Fixture from "../../fixtures/noise/blended-seed-1.json";
import seed12345Fixture from "../../fixtures/noise/blended-seed-12345.json";
import seed2151901553968352745Fixture from "../../fixtures/noise/blended-seed-2151901553968352745.json";
import { BlendedNoise } from "../../../src/worldgen/noise/blended-noise";
import { PerlinNoise } from "../../../src/worldgen/noise/perlin-noise";
import { SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source";

interface BlendedSampleParameters {
  limitHorizontalScale: number;
  limitVerticalScale: number;
  mainHorizontalScale: number;
  mainVerticalScale: number;
}

interface BlendFactorRange {
  min: number;
  max: number;
}

interface BlendRegionCounts {
  belowOrEqualZero: number;
  interior: number;
  aboveOrEqualOne: number;
}

interface BlendedSampleSet {
  gridOrder: string;
  sampleCount: number;
  settingsKeys: string[];
  parameters: BlendedSampleParameters;
  x: number[];
  y: number[];
  z: number[];
  values: number[];
  blendFactorRange: BlendFactorRange;
  blendRegionCounts: BlendRegionCounts;
}

interface BlendedFixture {
  module: string;
  minecraftVersion: string;
  noiseClass: string;
  randomSourceClass: string;
  randomSourceAlias: string;
  noiseMethod: string;
  seed: string;
  octaves: {
    limit: number[];
    main: number[];
  };
  wireFormat: {
    coordinates: string;
    parameters: string;
    values: string;
  };
  sampleSets: {
    overworld: BlendedSampleSet;
    nether: BlendedSampleSet;
    end: BlendedSampleSet;
  };
}

const fixtures = [
  seed0Fixture,
  seed1Fixture,
  seed12345Fixture,
  seed2151901553968352745Fixture,
] as BlendedFixture[];

const sampleSetExpectations = {
  overworld: ["overworld", "amplified"],
  nether: ["nether", "caves"],
  end: ["end", "floating_islands"],
} as const;

describe("BlendedNoise", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixture of fixtures) {
      expect(fixture.module).toBe("noise");
      expect(fixture.minecraftVersion).toBe("1.17.1");
      expect(fixture.noiseClass).toBe("net.minecraft.world.level.levelgen.synth.BlendedNoise");
      expect(fixture.randomSourceClass).toBe("net.minecraft.world.level.levelgen.SimpleRandomSource");
      expect(fixture.randomSourceAlias).toBe("LegacyRandomSource");
      expect(fixture.noiseMethod).toBe(
        "sampleAndClampNoise(x,y,z,limitHorizontalScale,limitVerticalScale,mainHorizontalScale,mainVerticalScale)",
      );
      expect(fixture.octaves.limit).toEqual([-15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0]);
      expect(fixture.octaves.main).toEqual([-7, -6, -5, -4, -3, -2, -1, 0]);
      expect(fixture.wireFormat.coordinates).toBe("integer");
      expect(fixture.wireFormat.parameters).toBe("number");
      expect(fixture.wireFormat.values).toBe("number");

      for (const [preset, settingsKeys] of Object.entries(sampleSetExpectations)) {
        const sampleSet = fixture.sampleSets[preset as keyof typeof fixture.sampleSets];
        expect(sampleSet.gridOrder).toBe("x-major,y-major,z-minor");
        expect(sampleSet.settingsKeys).toEqual(settingsKeys);
        expect(sampleSet.values).toHaveLength(sampleSet.sampleCount);
        expect(sampleSet.sampleCount).toBe(sampleSet.x.length * sampleSet.y.length * sampleSet.z.length);

        const regionCount =
          sampleSet.blendRegionCounts.belowOrEqualZero +
          sampleSet.blendRegionCounts.interior +
          sampleSet.blendRegionCounts.aboveOrEqualOne;
        expect(regionCount).toBe(sampleSet.sampleCount);
        expect(sampleSet.blendFactorRange.min).toBeLessThanOrEqual(0);
        expect(sampleSet.blendFactorRange.max).toBeGreaterThanOrEqual(1);
        expect(sampleSet.blendRegionCounts.belowOrEqualZero).toBeGreaterThan(0);
        expect(sampleSet.blendRegionCounts.interior).toBeGreaterThan(0);
        expect(sampleSet.blendRegionCounts.aboveOrEqualOne).toBeGreaterThan(0);
      }
    }
  });

  for (const fixture of fixtures) {
    for (const [preset, sampleSet] of Object.entries(fixture.sampleSets)) {
      test(`${fixture.seed} ${preset}: matches the Java oracle across the shared cell grid`, () => {
        const noise = new BlendedNoise(new SimpleRandomSource(BigInt(fixture.seed)));

        let index = 0;
        for (const x of sampleSet.x) {
          for (const y of sampleSet.y) {
            for (const z of sampleSet.z) {
              const actual = noise.sampleAndClampNoise(
                x,
                y,
                z,
                sampleSet.parameters.limitHorizontalScale,
                sampleSet.parameters.limitVerticalScale,
                sampleSet.parameters.mainHorizontalScale,
                sampleSet.parameters.mainVerticalScale,
              );
              const oracle = sampleSet.values[index]!;
              if (!Object.is(actual, oracle)) {
                throw new Error(
                  `blended(seed=${fixture.seed}, preset=${preset}) mismatch at index ${index} for (${x}, ${y}, ${z}): expected ${oracle}, got ${actual}`,
                );
              }

              index++;
            }
          }
        }
      });
    }
  }

  test("random-source construction matches explicitly chained PerlinNoise instances", () => {
    const seed = 12345n;
    const direct = new BlendedNoise(new SimpleRandomSource(seed));

    const random = new SimpleRandomSource(seed);
    const explicit = new BlendedNoise(
      new PerlinNoise(random, [-15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0]),
      new PerlinNoise(random, [-15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0]),
      new PerlinNoise(random, [-7, -6, -5, -4, -3, -2, -1, 0]),
    );

    expect(direct.sampleAndClampNoise(4, -2, 8, 684.412, 684.412, 684.412 / 80, 684.412 / 160)).toBe(
      explicit.sampleAndClampNoise(4, -2, 8, 684.412, 684.412, 684.412 / 80, 684.412 / 160),
    );
  });
});
