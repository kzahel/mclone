import { describe, expect, test } from "vitest";
import seed0Fixture from "../../fixtures/noise/simplex-seed-0.json";
import seed1Fixture from "../../fixtures/noise/simplex-seed-1.json";
import seed12345Fixture from "../../fixtures/noise/simplex-seed-12345.json";
import seed2151901553968352745Fixture from "../../fixtures/noise/simplex-seed-2151901553968352745.json";
import { SimplexNoise } from "../../../src/worldgen/noise/simplex-noise";
import { SimpleRandomSource } from "../../../src/worldgen/prng/simple-random-source";

interface SimplexSampleSet2D {
  noiseMethod: string;
  gridOrder: string;
  sampleCount: number;
  x: number[];
  z: number[];
  values: number[];
}

interface SimplexSampleSet3D {
  noiseMethod: string;
  gridOrder: string;
  sampleCount: number;
  x: number[];
  y: number[];
  z: number[];
  values: number[];
}

interface SimplexFixture {
  module: string;
  minecraftVersion: string;
  noiseClass: string;
  randomSourceClass: string;
  randomSourceAlias: string;
  seed: string;
  wireFormat: {
    coordinates: string;
    values: string;
  };
  offsets: {
    xo: number;
    yo: number;
    zo: number;
  };
  samples2d: SimplexSampleSet2D;
  samples3d: SimplexSampleSet3D;
}

const fixtures = [
  seed0Fixture,
  seed1Fixture,
  seed12345Fixture,
  seed2151901553968352745Fixture,
] as SimplexFixture[];

describe("SimplexNoise", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixture of fixtures) {
      expect(fixture.module).toBe("noise");
      expect(fixture.minecraftVersion).toBe("1.17.1");
      expect(fixture.noiseClass).toBe("net.minecraft.world.level.levelgen.synth.SimplexNoise");
      expect(fixture.randomSourceClass).toBe("net.minecraft.world.level.levelgen.SimpleRandomSource");
      expect(fixture.randomSourceAlias).toBe("LegacyRandomSource");
      expect(fixture.wireFormat.coordinates).toBe("number");
      expect(fixture.wireFormat.values).toBe("number");

      expect(fixture.samples2d.noiseMethod).toBe("getValue(x,z)");
      expect(fixture.samples2d.gridOrder).toBe("x-major,z-minor");
      expect(fixture.samples2d.values).toHaveLength(fixture.samples2d.sampleCount);
      expect(fixture.samples2d.sampleCount).toBe(fixture.samples2d.x.length * fixture.samples2d.z.length);

      expect(fixture.samples3d.noiseMethod).toBe("getValue(x,y,z)");
      expect(fixture.samples3d.gridOrder).toBe("x-major,y-major,z-minor");
      expect(fixture.samples3d.values).toHaveLength(fixture.samples3d.sampleCount);
      expect(fixture.samples3d.sampleCount).toBe(fixture.samples3d.x.length * fixture.samples3d.y.length * fixture.samples3d.z.length);
    }
  });

  for (const fixture of fixtures) {
    test(`${fixture.seed}: constructor offsets match the Java oracle`, () => {
      const noise = new SimplexNoise(new SimpleRandomSource(BigInt(fixture.seed)));
      expect(noise.xo).toBe(fixture.offsets.xo);
      expect(noise.yo).toBe(fixture.offsets.yo);
      expect(noise.zo).toBe(fixture.offsets.zo);
    });

    test(`${fixture.seed}: matches the Java oracle on the 2D sample grid`, () => {
      const noise = new SimplexNoise(new SimpleRandomSource(BigInt(fixture.seed)));

      let index = 0;
      for (const x of fixture.samples2d.x) {
        for (const z of fixture.samples2d.z) {
          const actual = noise.getValue(x, z);
          const oracle = fixture.samples2d.values[index]!;
          if (!Object.is(actual, oracle)) {
            throw new Error(
              `simplex(seed=${fixture.seed}, dimensions=2d) mismatch at index ${index} for (${x}, ${z}): expected ${oracle}, got ${actual}`,
            );
          }

          index++;
        }
      }
    });

    test(`${fixture.seed}: matches the Java oracle on the 3D sample grid`, () => {
      const noise = new SimplexNoise(new SimpleRandomSource(BigInt(fixture.seed)));

      let index = 0;
      for (const x of fixture.samples3d.x) {
        for (const y of fixture.samples3d.y) {
          for (const z of fixture.samples3d.z) {
            const actual = noise.getValue(x, y, z);
            const oracle = fixture.samples3d.values[index]!;
            if (!Object.is(actual, oracle)) {
              throw new Error(
                `simplex(seed=${fixture.seed}, dimensions=3d) mismatch at index ${index} for (${x}, ${y}, ${z}): expected ${oracle}, got ${actual}`,
              );
            }

            index++;
          }
        }
      }
    });
  }
});
