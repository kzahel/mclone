import { describe, expect, test } from "vitest";
import barrierSeed0Fixture from "../../fixtures/noise/normal-barrier-seed-0.json";
import barrierSeed1Fixture from "../../fixtures/noise/normal-barrier-seed-1.json";
import barrierSeed12345Fixture from "../../fixtures/noise/normal-barrier-seed-12345.json";
import barrierSeed2151901553968352745Fixture from "../../fixtures/noise/normal-barrier-seed-2151901553968352745.json";
import lavaSeed0Fixture from "../../fixtures/noise/normal-lava-seed-0.json";
import lavaSeed1Fixture from "../../fixtures/noise/normal-lava-seed-1.json";
import lavaSeed12345Fixture from "../../fixtures/noise/normal-lava-seed-12345.json";
import lavaSeed2151901553968352745Fixture from "../../fixtures/noise/normal-lava-seed-2151901553968352745.json";
import waterLevelSeed0Fixture from "../../fixtures/noise/normal-water-level-seed-0.json";
import waterLevelSeed1Fixture from "../../fixtures/noise/normal-water-level-seed-1.json";
import waterLevelSeed12345Fixture from "../../fixtures/noise/normal-water-level-seed-12345.json";
import waterLevelSeed2151901553968352745Fixture from "../../fixtures/noise/normal-water-level-seed-2151901553968352745.json";
import { NormalNoise } from "../../../src/worldgen/noise/normal-noise";
import { PerlinNoise } from "../../../src/worldgen/noise/perlin-noise";
import { SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source";

const INPUT_FACTOR = 1.0181268882175227;
const VALUE_FACTOR_SCALE = 0.16666666666666666;

interface NormalNoiseFixture {
  module: string;
  minecraftVersion: string;
  noiseClass: string;
  randomSourceClass: string;
  randomSourceAlias: string;
  noiseMethod: string;
  gridOrder: string;
  seed: string;
  sampleCount: number;
  firstOctave: number;
  amplitudes: number[];
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
    label: "barrier",
    expectedFirstOctave: -3,
    expectedAmplitudes: [1.0],
    fixtures: [barrierSeed0Fixture, barrierSeed1Fixture, barrierSeed12345Fixture, barrierSeed2151901553968352745Fixture],
  },
  {
    label: "water-level",
    expectedFirstOctave: -3,
    expectedAmplitudes: [1.0, 0.0, 2.0],
    fixtures: [waterLevelSeed0Fixture, waterLevelSeed1Fixture, waterLevelSeed12345Fixture, waterLevelSeed2151901553968352745Fixture],
  },
  {
    label: "lava",
    expectedFirstOctave: -1,
    expectedAmplitudes: [1.0, 0.0],
    fixtures: [lavaSeed0Fixture, lavaSeed1Fixture, lavaSeed12345Fixture, lavaSeed2151901553968352745Fixture],
  },
] as const satisfies ReadonlyArray<{
  label: string;
  expectedFirstOctave: number;
  expectedAmplitudes: readonly number[];
  fixtures: readonly NormalNoiseFixture[];
}>;

function expectedDeviation(octaveSpan: number): number {
  return 0.1 * (1 + (1 / (octaveSpan + 1)));
}

describe("NormalNoise", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixtureSet of fixtureSets) {
      for (const fixture of fixtureSet.fixtures) {
        expect(fixture.module).toBe("noise");
        expect(fixture.minecraftVersion).toBe("1.17.1");
        expect(fixture.noiseClass).toBe("net.minecraft.world.level.levelgen.synth.NormalNoise");
        expect(fixture.randomSourceClass).toBe("net.minecraft.world.level.levelgen.SimpleRandomSource");
        expect(fixture.randomSourceAlias).toBe("LegacyRandomSource");
        expect(fixture.noiseMethod).toBe("getValue(x,y,z)");
        expect(fixture.gridOrder).toBe("x-major,y-major,z-minor");
        expect(fixture.firstOctave).toBe(fixtureSet.expectedFirstOctave);
        expect(fixture.amplitudes).toEqual(fixtureSet.expectedAmplitudes);
        expect(fixture.wireFormat.coordinates).toBe("number");
        expect(fixture.wireFormat.values).toBe("number");
        expect(fixture.values).toHaveLength(fixture.sampleCount);
        expect(fixture.sampleCount).toBe(fixture.x.length * fixture.y.length * fixture.z.length);
      }
    }
  });

  for (const fixtureSet of fixtureSets) {
    for (const fixture of fixtureSet.fixtures) {
      test(`${fixture.seed} ${fixtureSet.label}: matches the Java oracle across the shared sample grid`, () => {
        const noise = NormalNoise.create(
          new SimpleRandomSource(BigInt(fixture.seed)),
          fixture.firstOctave,
          fixture.amplitudes,
        );

        let index = 0;
        for (const x of fixture.x) {
          for (const y of fixture.y) {
            for (const z of fixture.z) {
              const actual = noise.getValue(x, y, z);
              const oracle = fixture.values[index]!;
              if (!Object.is(actual, oracle)) {
                throw new Error(
                  `normal(seed=${fixture.seed}, config=${fixtureSet.label}) mismatch at index ${index} for (${x}, ${y}, ${z}): expected ${oracle}, got ${actual}`,
                );
              }

              index++;
            }
          }
        }
      });
    }
  }

  test("create matches the explicit two-Perlin chain and uses the non-zero amplitude span for valueFactor", () => {
    const seed = 12345n;
    const amplitudes = [1.0, 0.0] as const;
    const noise = NormalNoise.create(new SimpleRandomSource(seed), -1, amplitudes);

    const random = new SimpleRandomSource(seed);
    const first = PerlinNoise.create(random, -1, amplitudes);
    const second = PerlinNoise.create(random, -1, amplitudes);
    const octaveSpan = 0;
    const valueFactor = VALUE_FACTOR_SCALE / expectedDeviation(octaveSpan);

    expect(noise.getValue(0.25, -0.5, 0.75)).toBe(
      (
        first.getValue(0.25, -0.5, 0.75) +
        second.getValue(0.25 * INPUT_FACTOR, -0.5 * INPUT_FACTOR, 0.75 * INPUT_FACTOR)
      ) * valueFactor,
    );
  });
});
