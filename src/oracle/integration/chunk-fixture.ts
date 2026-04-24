import type { DecodedChunk } from "../anvil/chunk.ts";
import { encodeChunkLightFixture, type ChunkLightFixture } from "./light-fixture.ts";

export interface ChunkFixtureSection {
  readonly y: number;
  readonly palette: readonly string[];
  readonly blockOrder: "y-major,z-major,x-minor";
  readonly blocks: readonly number[];
}

export interface ChunkFixtureEntry {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly status: string;
  readonly isLightOn: boolean;
  readonly sections: readonly ChunkFixtureSection[];
  readonly light?: ChunkLightFixture;
  readonly heightmaps: { readonly [name: string]: readonly number[] };
  readonly biomes: readonly number[];
}

export interface ChunkFixture {
  readonly module: "integration";
  readonly minecraftVersion: string;
  readonly dataVersion: number;
  readonly seed: string;
  readonly generator: string;
  readonly generateStructures: boolean;
  readonly wireFormat: {
    readonly blockOrder: "y-major,z-major,x-minor";
    readonly heightmapOrder: "z-major,x-minor";
    readonly biomeOrder: "y-major,z-major,x-minor";
    readonly paletteEntries: "resource-key";
    readonly lightData: "base64-encoded-2048-byte-datalayer";
  };
  readonly chunks: readonly ChunkFixtureEntry[];
}

export interface ChunkFixtureMetadata {
  readonly minecraftVersion: string;
  readonly seed: string;
  readonly generator: string;
  readonly generateStructures: boolean;
}

export function buildChunkFixture(metadata: ChunkFixtureMetadata, chunks: readonly DecodedChunk[]): ChunkFixture {
  if (chunks.length === 0) {
    throw new Error("buildChunkFixture requires at least one chunk");
  }

  const ordered = [...chunks].sort((a, b) => (a.chunkX - b.chunkX) || (a.chunkZ - b.chunkZ));
  const dataVersion = ordered[0]!.dataVersion;
  for (const chunk of ordered) {
    if (chunk.dataVersion !== dataVersion) {
      throw new Error(
        `mixed data versions in fixture: chunk (${chunk.chunkX}, ${chunk.chunkZ}) has ${chunk.dataVersion}, expected ${dataVersion}`,
      );
    }
    if (chunk.status !== "full") {
      throw new Error(
        `chunk (${chunk.chunkX}, ${chunk.chunkZ}) has status "${chunk.status}"; fixture requires "full"`,
      );
    }
  }

  return {
    module: "integration",
    minecraftVersion: metadata.minecraftVersion,
    dataVersion,
    seed: metadata.seed,
    generator: metadata.generator,
    generateStructures: metadata.generateStructures,
    wireFormat: {
      blockOrder: "y-major,z-major,x-minor",
      heightmapOrder: "z-major,x-minor",
      biomeOrder: "y-major,z-major,x-minor",
      paletteEntries: "resource-key",
      lightData: "base64-encoded-2048-byte-datalayer",
    },
    chunks: ordered.map(toFixtureEntry),
  };
}

function toFixtureEntry(chunk: DecodedChunk): ChunkFixtureEntry {
  const light = encodeChunkLightFixture(chunk.light);
  const entry: ChunkFixtureEntry = {
    chunkX: chunk.chunkX,
    chunkZ: chunk.chunkZ,
    status: chunk.status,
    isLightOn: chunk.isLightOn,
    sections: chunk.sections.map((section) => ({
      y: section.y,
      palette: section.palette.map((entry) => entry.name),
      blockOrder: "y-major,z-major,x-minor",
      blocks: section.blocks,
    })),
    heightmaps: chunk.heightmaps,
    biomes: chunk.biomes,
  };
  if (light.block.length > 0 || light.sky.length > 0) {
    return { ...entry, light };
  }
  return entry;
}
