import { describe, expect, test } from "vitest";
import seed0Fixture from "../../fixtures/biome/overworld-seed-0.json";
import seed1Fixture from "../../fixtures/biome/overworld-seed-1.json";
import seed12345Fixture from "../../fixtures/biome/overworld-seed-12345.json";
import seed2151901553968352745Fixture from "../../fixtures/biome/overworld-seed-2151901553968352745.json";
import integrationFixture from "../../fixtures/integration/overworld-seed-12345-chunks-0-0.json";
import { OVERWORLD_LAYERED_BIOMES } from "../../../src/worldgen/biome/biome-data";
import { ChunkBiomeContainer } from "../../../src/worldgen/biome/chunk-biome-container";
import { OverworldBiomeSource } from "../../../src/worldgen/biome/overworld-biome-source";

interface PossibleBiomeFixture {
  id: number;
  key: string;
  depth: number;
  scale: number;
}

interface OverworldBiomeSamplesFixture {
  biomeMethod: string;
  gridOrder: string;
  sampleY: number;
  sampleCount: number;
  x: number[];
  z: number[];
  ids: number[];
  keys: string[];
  depths: number[];
  scales: number[];
}

interface OverworldBiomeSourceFixture {
  module: string;
  minecraftVersion: string;
  biomeSourceClass: string;
  seed: string;
  legacyBiomeInitLayer: boolean;
  largeBiomes: boolean;
  wireFormat: {
    coordinates: string;
    biomeIds: string;
    biomeKeys: string;
    biomeFactors: string;
  };
  possibleBiomes: PossibleBiomeFixture[];
  samples: OverworldBiomeSamplesFixture;
}

interface IntegrationChunkFixture {
  chunkX: number;
  chunkZ: number;
  biomes: number[];
}

interface IntegrationFixture {
  seed: string;
  chunks: IntegrationChunkFixture[];
}

const fixtures = [
  seed0Fixture,
  seed1Fixture,
  seed12345Fixture,
  seed2151901553968352745Fixture,
] as const satisfies readonly OverworldBiomeSourceFixture[];

const overworldIntegrationFixture = integrationFixture as IntegrationFixture;

describe("OverworldBiomeSource", () => {
  test("fixture metadata stays consistent", () => {
    for (const fixture of fixtures) {
      expect(fixture.module).toBe("biome");
      expect(fixture.minecraftVersion).toBe("1.17.1");
      expect(fixture.biomeSourceClass).toBe("net.minecraft.world.level.biome.OverworldBiomeSource");
      expect(fixture.legacyBiomeInitLayer).toBe(false);
      expect(fixture.largeBiomes).toBe(false);
      expect(fixture.wireFormat.coordinates).toBe("integer");
      expect(fixture.wireFormat.biomeIds).toBe("integer");
      expect(fixture.wireFormat.biomeKeys).toBe("string");
      expect(fixture.wireFormat.biomeFactors).toBe("number");
      expect(fixture.samples.biomeMethod).toBe("getNoiseBiome(x,0,z)");
      expect(fixture.samples.gridOrder).toBe("x-major,z-minor");
      expect(fixture.samples.sampleY).toBe(0);
      expect(fixture.samples.sampleCount).toBe(fixture.samples.x.length * fixture.samples.z.length);
      expect(fixture.samples.ids).toHaveLength(fixture.samples.sampleCount);
      expect(fixture.samples.keys).toHaveLength(fixture.samples.sampleCount);
      expect(fixture.samples.depths).toHaveLength(fixture.samples.sampleCount);
      expect(fixture.samples.scales).toHaveLength(fixture.samples.sampleCount);
    }
  });

  test("runtime biome registry matches the oracle's possible-biome list", () => {
    for (const fixture of fixtures) {
      expect(OVERWORLD_LAYERED_BIOMES).toHaveLength(fixture.possibleBiomes.length);
      fixture.possibleBiomes.forEach((oracleBiome, index) => {
        const biome = OVERWORLD_LAYERED_BIOMES[index]!;
        expect(biome.getId()).toBe(oracleBiome.id);
        expect(biome.getKey()).toBe(oracleBiome.key);
        expect(biome.getDepth()).toBe(oracleBiome.depth);
        expect(biome.getScale()).toBe(oracleBiome.scale);
      });
    }
  });

  for (const fixture of fixtures) {
    test(`${fixture.seed}: matches the Java oracle across the sampled quart grid`, () => {
      const biomeSource = new OverworldBiomeSource(BigInt(fixture.seed), fixture.legacyBiomeInitLayer, fixture.largeBiomes);
      let index = 0;

      for (const x of fixture.samples.x) {
        for (const z of fixture.samples.z) {
          const biome = biomeSource.getNoiseBiome(x, fixture.samples.sampleY, z);
          expect(biomeSource.getNoiseBiomeId(x, fixture.samples.sampleY, z)).toBe(fixture.samples.ids[index]!);
          expect(biome.getId()).toBe(fixture.samples.ids[index]!);
          expect(biome.getKey()).toBe(fixture.samples.keys[index]!);
          expect(biome.getDepth()).toBe(fixture.samples.depths[index]!);
          expect(biome.getScale()).toBe(fixture.samples.scales[index]!);
          index++;
        }
      }
    });
  }

  test("ChunkBiomeContainer writes the same native biome grid as the committed integration fixture", () => {
    const chunk = overworldIntegrationFixture.chunks[0]!;
    const biomeSource = new OverworldBiomeSource(BigInt(overworldIntegrationFixture.seed));
    const container = new ChunkBiomeContainer(0, 256, chunk.chunkX, chunk.chunkZ, biomeSource);
    expect(container.writeBiomes()).toEqual(chunk.biomes);
  });
});
