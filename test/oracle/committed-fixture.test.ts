import { describe, expect, test } from "vitest";
import committed from "../fixtures/integration/overworld-seed-12345-chunks-0-0.json" with { type: "json" };
import { decodeChunkLightFixture } from "../../oracle/lib/integration/light-fixture.ts";

interface CommittedFixtureSection {
  readonly y: number;
  readonly palette: readonly string[];
  readonly blockOrder: string;
  readonly blocks: readonly number[];
}

interface CommittedLightSection {
  readonly y: number;
  readonly dataBase64: string;
}

interface CommittedChunkLight {
  readonly sky: readonly CommittedLightSection[];
  readonly block: readonly CommittedLightSection[];
}

interface CommittedFixtureChunk {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly status: string;
  readonly isLightOn: boolean;
  readonly sections: readonly CommittedFixtureSection[];
  readonly light?: CommittedChunkLight;
  readonly heightmaps: { readonly [name: string]: readonly number[] };
  readonly biomes: readonly number[];
}

interface CommittedFixture {
  readonly module: string;
  readonly minecraftVersion: string;
  readonly dataVersion: number;
  readonly seed: string;
  readonly generator: string;
  readonly generateStructures: boolean;
  readonly wireFormat: {
    readonly blockOrder: string;
    readonly heightmapOrder: string;
    readonly biomeOrder: string;
    readonly paletteEntries: string;
    readonly lightData?: string;
  };
  readonly chunks: readonly CommittedFixtureChunk[];
}

const fixture = committed as CommittedFixture;

describe("committed integration fixture", () => {
  test("metadata identifies the pinned seed + 1.17.1 target", () => {
    expect(fixture.module).toBe("integration");
    expect(fixture.minecraftVersion).toBe("1.17.1");
    expect(fixture.seed).toBe("12345");
    expect(fixture.generator).toBe("default");
    expect(fixture.generateStructures).toBe(false);
    expect(fixture.wireFormat.blockOrder).toBe("y-major,z-major,x-minor");
    expect(fixture.wireFormat.heightmapOrder).toBe("z-major,x-minor");
    expect(fixture.wireFormat.biomeOrder).toBe("y-major,z-major,x-minor");
    expect(fixture.wireFormat.paletteEntries).toBe("resource-key");
    expect(fixture.wireFormat.lightData).toBe("base64-encoded-2048-byte-datalayer");
  });

  test("contains exactly one fully-generated chunk at (0, 0)", () => {
    expect(fixture.chunks).toHaveLength(1);
    const chunk = fixture.chunks[0]!;
    expect(chunk.chunkX).toBe(0);
    expect(chunk.chunkZ).toBe(0);
    expect(chunk.status).toBe("full");
    expect(chunk.isLightOn).toBe(true);
  });

  test("biomes array is MC's native 4x64x4 grid (1024 entries)", () => {
    const chunk = fixture.chunks[0]!;
    expect(chunk.biomes.length).toBe(1024);
    for (const biome of chunk.biomes) {
      expect(Number.isInteger(biome)).toBe(true);
      expect(biome).toBeGreaterThanOrEqual(0);
    }
  });

  test("each heightmap row is 256 entries and non-negative", () => {
    const chunk = fixture.chunks[0]!;
    expect(Object.keys(chunk.heightmaps).length).toBeGreaterThan(0);
    for (const [name, data] of Object.entries(chunk.heightmaps)) {
      expect(data.length).toBe(256);
      for (const height of data) {
        expect(Number.isInteger(height)).toBe(true);
        expect(height).toBeGreaterThanOrEqual(0);
        expect(height).toBeLessThanOrEqual(383);
        expect(typeof name).toBe("string");
      }
    }
  });

  test("every section has a 4096-entry block array indexing into its palette", () => {
    const chunk = fixture.chunks[0]!;
    expect(chunk.sections.length).toBeGreaterThan(0);
    for (const section of chunk.sections) {
      expect(section.blockOrder).toBe("y-major,z-major,x-minor");
      expect(section.blocks.length).toBe(4096);
      expect(section.palette.length).toBeGreaterThan(0);
      for (const palette of section.palette) {
        expect(palette).toMatch(/^minecraft:[a-z0-9_]+$/);
      }
      for (const blockIndex of section.blocks) {
        expect(blockIndex).toBeGreaterThanOrEqual(0);
        expect(blockIndex).toBeLessThan(section.palette.length);
      }
    }
  });

  test("bedrock appears in the bottom-most section, establishing voxel ground truth", () => {
    const chunk = fixture.chunks[0]!;
    const bottom = chunk.sections.slice().sort((a, b) => a.y - b.y)[0]!;
    expect(bottom.palette).toContain("minecraft:bedrock");
  });

  test("includes vanilla persisted BlockLight and SkyLight DataLayers", () => {
    const chunk = fixture.chunks[0]!;
    expect(chunk.light).toBeDefined();
    const light = decodeChunkLightFixture(chunk.light!);

    expect(light.block.length).toBeGreaterThan(0);
    expect(light.sky.length).toBeGreaterThan(0);
    for (const section of [...light.block, ...light.sky]) {
      expect(Number.isInteger(section.y)).toBe(true);
      expect(section.data).toHaveLength(2048);
    }
  });
});
