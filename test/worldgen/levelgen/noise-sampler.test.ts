import { describe, expect, test } from "vitest";
import seed0Fixture from "../../fixtures/noise/noise-sampler-overworld-seed-0.json";
import seed1Fixture from "../../fixtures/noise/noise-sampler-overworld-seed-1.json";
import seed12345Fixture from "../../fixtures/noise/noise-sampler-overworld-seed-12345.json";
import seed2151901553968352745Fixture from "../../fixtures/noise/noise-sampler-overworld-seed-2151901553968352745.json";
import type { NoiseBiome } from "../../../src/worldgen/biome/noise-biome";
import type { NoiseBiomeSource } from "../../../src/worldgen/biome/noise-biome-source";
import { NoiseModifier } from "../../../src/worldgen/levelgen/noise-modifier";
import { NoiseSampler } from "../../../src/worldgen/levelgen/noise-sampler";
import { NoiseSamplingSettings } from "../../../src/worldgen/levelgen/noise-sampling-settings";
import { NoiseSettings } from "../../../src/worldgen/levelgen/noise-settings";
import { NoiseSlideSettings } from "../../../src/worldgen/levelgen/noise-slide-settings";
import { BlendedNoise } from "../../../src/worldgen/noise/blended-noise";
import { PerlinNoise } from "../../../src/worldgen/noise/perlin-noise";
import { SimplexNoise } from "../../../src/worldgen/noise/simplex-noise";
import { WorldgenRandom } from "../../../src/worldgen/prng/worldgen-random";

const DEPTH_NOISE_OCTAVES = Array.from({ length: 16 }, (_, index) => index - 15);

interface NoiseSamplerBiomePatternFixture {
  gridOrder: string;
  sampleCount: number;
  x: number[];
  z: number[];
  keys: string[];
  depths: number[];
  scales: number[];
}

interface NoiseSamplerColumnFixture {
  noiseMethod: string;
  gridOrder: string;
  columnValueCount: number;
  sampleCount: number;
  x: number[];
  z: number[];
  values: number[];
}

interface NoiseSamplerSampleSetFixture {
  biomePattern: NoiseSamplerBiomePatternFixture;
  columns: NoiseSamplerColumnFixture;
}

interface NoiseSamplerFixture {
  module: string;
  minecraftVersion: string;
  noiseClass: string;
  randomSourceClass: string;
  settingsPreset: string;
  noiseModifier: string;
  seed: string;
  cellWidth: number;
  cellHeight: number;
  cellCountY: number;
  biomeY: number;
  minCellY: number;
  columnValueCount: number;
  wireFormat: {
    coordinates: string;
    biomeKeys: string;
    biomeFactors: string;
    values: string;
  };
  noiseSettings: {
    minY: number;
    height: number;
    sampling: {
      xzScale: number;
      yScale: number;
      xzFactor: number;
      yFactor: number;
    };
    topSlide: {
      target: number;
      size: number;
      offset: number;
    };
    bottomSlide: {
      target: number;
      size: number;
      offset: number;
    };
    noiseSizeHorizontal: number;
    noiseSizeVertical: number;
    densityFactor: number;
    densityOffset: number;
    useSimplexSurfaceNoise: boolean;
    randomDensityOffset: boolean;
    islandNoiseOverride: boolean;
    isAmplified: boolean;
  };
  sampleSets: Record<string, NoiseSamplerSampleSetFixture>;
}

class TestBiome implements NoiseBiome {
  public constructor(
    private readonly depth: number,
    private readonly scale: number,
  ) {}

  public getDepth(): number {
    return this.depth;
  }

  public getScale(): number {
    return this.scale;
  }
}

class RepeatingPatternBiomeSource implements NoiseBiomeSource {
  private readonly width: number;
  private readonly height: number;

  public constructor(
    xAxis: readonly number[],
    zAxis: readonly number[],
    private readonly biomes: readonly NoiseBiome[],
  ) {
    this.width = xAxis.length;
    this.height = zAxis.length;
  }

  public getNoiseBiome(x: number, _y: number, z: number): NoiseBiome {
    const wrappedX = ((x % this.width) + this.width) % this.width;
    const wrappedZ = ((z % this.height) + this.height) % this.height;
    return this.biomes[(wrappedX * this.height) + wrappedZ]!;
  }
}

const fixtures = [seed0Fixture, seed1Fixture, seed12345Fixture, seed2151901553968352745Fixture] as const satisfies readonly NoiseSamplerFixture[];

function createNoiseSettings(fixture: NoiseSamplerFixture): NoiseSettings {
  return NoiseSettings.create(
    fixture.noiseSettings.minY,
    fixture.noiseSettings.height,
    new NoiseSamplingSettings(
      fixture.noiseSettings.sampling.xzScale,
      fixture.noiseSettings.sampling.yScale,
      fixture.noiseSettings.sampling.xzFactor,
      fixture.noiseSettings.sampling.yFactor,
    ),
    new NoiseSlideSettings(
      fixture.noiseSettings.topSlide.target,
      fixture.noiseSettings.topSlide.size,
      fixture.noiseSettings.topSlide.offset,
    ),
    new NoiseSlideSettings(
      fixture.noiseSettings.bottomSlide.target,
      fixture.noiseSettings.bottomSlide.size,
      fixture.noiseSettings.bottomSlide.offset,
    ),
    fixture.noiseSettings.noiseSizeHorizontal,
    fixture.noiseSettings.noiseSizeVertical,
    fixture.noiseSettings.densityFactor,
    fixture.noiseSettings.densityOffset,
    fixture.noiseSettings.useSimplexSurfaceNoise,
    fixture.noiseSettings.randomDensityOffset,
    fixture.noiseSettings.islandNoiseOverride,
    fixture.noiseSettings.isAmplified,
  );
}

function createBiomeSource(pattern: NoiseSamplerBiomePatternFixture): NoiseBiomeSource {
  return new RepeatingPatternBiomeSource(
    pattern.x,
    pattern.z,
    pattern.depths.map((depth, index) => new TestBiome(depth, pattern.scales[index]!)),
  );
}

function createNoiseSampler(fixture: NoiseSamplerFixture, pattern: NoiseSamplerBiomePatternFixture): NoiseSampler {
  const settings = createNoiseSettings(fixture);
  const random = new WorldgenRandom(BigInt(fixture.seed));
  const blendedNoise = new BlendedNoise(random);
  random.consumeCount(2620);
  const depthNoise = new PerlinNoise(random, DEPTH_NOISE_OCTAVES);

  return new NoiseSampler(
    createBiomeSource(pattern),
    fixture.cellWidth,
    fixture.cellHeight,
    fixture.cellCountY,
    settings,
    blendedNoise,
    undefined,
    depthNoise,
    NoiseModifier.PASSTHROUGH,
  );
}

describe("NoiseSampler", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixture of fixtures) {
      expect(fixture.module).toBe("noise");
      expect(fixture.minecraftVersion).toBe("1.17.1");
      expect(fixture.noiseClass).toBe("net.minecraft.world.level.levelgen.NoiseSampler");
      expect(fixture.randomSourceClass).toBe("net.minecraft.world.level.levelgen.WorldgenRandom");
      expect(fixture.settingsPreset).toBe("overworld");
      expect(fixture.noiseModifier).toBe("PASSTHROUGH");
      expect(fixture.cellWidth).toBe(4);
      expect(fixture.cellHeight).toBe(8);
      expect(fixture.cellCountY).toBe(32);
      expect(fixture.biomeY).toBe(63);
      expect(fixture.minCellY).toBe(0);
      expect(fixture.columnValueCount).toBe(33);
      expect(fixture.wireFormat.coordinates).toBe("integer");
      expect(fixture.wireFormat.biomeKeys).toBe("string");
      expect(fixture.wireFormat.biomeFactors).toBe("number");
      expect(fixture.wireFormat.values).toBe("number");
      expect(fixture.noiseSettings.useSimplexSurfaceNoise).toBe(true);
      expect(fixture.noiseSettings.randomDensityOffset).toBe(true);
      expect(fixture.noiseSettings.islandNoiseOverride).toBe(false);
      expect(fixture.noiseSettings.isAmplified).toBe(false);

      for (const [name, sampleSet] of Object.entries(fixture.sampleSets)) {
        expect(["constantPlains", "mixedOverworld"]).toContain(name);
        expect(sampleSet.biomePattern.gridOrder).toBe("x-major,z-minor");
        expect(sampleSet.biomePattern.sampleCount).toBe(sampleSet.biomePattern.x.length * sampleSet.biomePattern.z.length);
        expect(sampleSet.biomePattern.keys).toHaveLength(sampleSet.biomePattern.sampleCount);
        expect(sampleSet.biomePattern.depths).toHaveLength(sampleSet.biomePattern.sampleCount);
        expect(sampleSet.biomePattern.scales).toHaveLength(sampleSet.biomePattern.sampleCount);
        expect(sampleSet.columns.noiseMethod).toBe("fillNoiseColumn(noiseValues,cellX,cellZ,noiseSettings,biomeY,minCellY,cellCountY)");
        expect(sampleSet.columns.gridOrder).toBe("x-major,z-minor,y-minor");
        expect(sampleSet.columns.columnValueCount).toBe(fixture.columnValueCount);
        expect(sampleSet.columns.sampleCount).toBe(
          sampleSet.columns.x.length * sampleSet.columns.z.length * sampleSet.columns.columnValueCount,
        );
        expect(sampleSet.columns.values).toHaveLength(sampleSet.columns.sampleCount);
      }
    }
  });

  for (const fixture of fixtures) {
    for (const [sampleSetName, sampleSet] of Object.entries(fixture.sampleSets)) {
      test(`${fixture.seed} ${sampleSetName}: matches the Java oracle across the sampled cell columns`, () => {
        const sampler = createNoiseSampler(fixture, sampleSet.biomePattern);
        const settings = createNoiseSettings(fixture);
        const column = Array.from({ length: fixture.columnValueCount }, () => 0);

        let index = 0;
        for (const cellX of sampleSet.columns.x) {
          for (const cellZ of sampleSet.columns.z) {
            sampler.fillNoiseColumn(
              column,
              cellX,
              cellZ,
              settings,
              fixture.biomeY,
              fixture.minCellY,
              fixture.cellCountY,
            );

            for (let yIndex = 0; yIndex < fixture.columnValueCount; yIndex++) {
              const actual = column[yIndex]!;
              const oracle = sampleSet.columns.values[index]!;
              if (!Object.is(actual, oracle)) {
                throw new Error(
                  `noise-sampler(seed=${fixture.seed}, sampleSet=${sampleSetName}) mismatch at flattened index ${index} for cell (${cellX}, ${cellZ}) and column index ${yIndex}: expected ${oracle}, got ${actual}`,
                );
              }

              index++;
            }
          }
        }
      });
    }
  }

  test("NoiseSettings.create enforces Java's minY/height guard", () => {
    const sampling = new NoiseSamplingSettings(1, 1, 80, 160);
    const topSlide = new NoiseSlideSettings(-10, 3, 0);
    const bottomSlide = new NoiseSlideSettings(15, 3, 0);

    expect(() => NoiseSettings.create(0, 255, sampling, topSlide, bottomSlide, 1, 2, 1, -0.46875, true, true, false, false)).toThrow(
      "height has to be a multiple of 16",
    );
    expect(() => NoiseSettings.create(1, 256, sampling, topSlide, bottomSlide, 1, 2, 1, -0.46875, true, true, false, false)).toThrow(
      "min_y has to be a multiple of 16",
    );
    expect(() =>
      NoiseSettings.create(0, 2048 + 16, sampling, topSlide, bottomSlide, 1, 2, 1, -0.46875, true, true, false, false)
    ).toThrow("min_y + height cannot be higher than: 2032");
  });

  test("island noise override is rejected in this overworld-only slice", () => {
    const settings = createNoiseSettings(seed12345Fixture);
    const random = new WorldgenRandom(12345n);
    const blendedNoise = new BlendedNoise(random);
    random.consumeCount(2620);
    const depthNoise = new PerlinNoise(random, DEPTH_NOISE_OCTAVES);
    const islandNoise = new SimplexNoise(new WorldgenRandom(12345n));
    const sampler = new NoiseSampler(
      createBiomeSource(seed12345Fixture.sampleSets.constantPlains.biomePattern),
      seed12345Fixture.cellWidth,
      seed12345Fixture.cellHeight,
      seed12345Fixture.cellCountY,
      settings,
      blendedNoise,
      islandNoise,
      depthNoise,
      NoiseModifier.PASSTHROUGH,
    );

    expect(() =>
      sampler.fillNoiseColumn(
        Array.from({ length: seed12345Fixture.columnValueCount }, () => 0),
        0,
        0,
        settings,
        seed12345Fixture.biomeY,
        seed12345Fixture.minCellY,
        seed12345Fixture.cellCountY,
      )
    ).toThrow("NoiseSampler island noise override is out of scope for the 1.17.1 overworld target");
  });
});
