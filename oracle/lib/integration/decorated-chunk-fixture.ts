export interface DecoratedChunkOracleSectionFixture {
  readonly y: number;
  readonly palette: readonly string[];
  readonly blocks: readonly number[];
  readonly blockOrder?: "y-major,z-major,x-minor";
}

export interface DecoratedChunkOracleFixture {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly sections: readonly DecoratedChunkOracleSectionFixture[];
}

export interface DecoratedIntegrationOracleFixture {
  readonly seed: string;
  readonly chunks: readonly DecoratedChunkOracleFixture[];
}

export interface DecoratedChunkPosition {
  readonly localX: number;
  readonly y: number;
  readonly localZ: number;
}

export interface DecoratedChunkMismatch extends DecoratedChunkPosition {
  readonly expected: string;
  readonly actual: string;
  readonly bucket: DecoratedChunkMismatchBucket;
  readonly yBand: string;
}

export interface DecoratedChunkMismatchCount {
  readonly key: string;
  readonly count: number;
}

export interface DecoratedChunkDiff {
  readonly totalBlocks: number;
  readonly matches: number;
  readonly mismatchCount: number;
  readonly mismatches: readonly DecoratedChunkMismatch[];
  readonly bucketCounts: readonly DecoratedChunkMismatchCount[];
  readonly pairCounts: readonly DecoratedChunkMismatchCount[];
  readonly yBandCounts: readonly DecoratedChunkMismatchCount[];
}

export type DecoratedChunkBlockNameAt = (localX: number, y: number, localZ: number) => string;

export type DecoratedChunkMismatchBucket =
  | "Tree logs/leaves placement"
  | "Carver/fluid edge"
  | "Deep underground blobs/lava"
  | "Plants/snow decoration"
  | "Surface dirt/grass choice"
  | "Ores/glow lichen"
  | "Other";

export const DECORATED_CHUNK_MIN_Y = 0;
export const DECORATED_CHUNK_HEIGHT = 256;

export const DECORATED_GROUND_IGNORED_BLOCKS = new Set([
  "minecraft:air",
  "minecraft:cave_air",
  "minecraft:water",
  "minecraft:oak_log",
  "minecraft:oak_leaves",
  "minecraft:spruce_log",
  "minecraft:spruce_leaves",
  "minecraft:grass",
  "minecraft:fern",
  "minecraft:large_fern",
  "minecraft:oak_sapling",
  "minecraft:spruce_sapling",
  "minecraft:sweet_berry_bush",
  "minecraft:brown_mushroom",
  "minecraft:red_mushroom",
  "minecraft:sugar_cane",
  "minecraft:cactus",
  "minecraft:pumpkin",
  "minecraft:dandelion",
  "minecraft:poppy",
  "minecraft:snow",
]);

const TREE_BLOCKS = new Set([
  "minecraft:oak_log",
  "minecraft:oak_leaves",
  "minecraft:spruce_log",
  "minecraft:spruce_leaves",
  "minecraft:birch_log",
  "minecraft:birch_leaves",
  "minecraft:jungle_log",
  "minecraft:jungle_leaves",
  "minecraft:acacia_log",
  "minecraft:acacia_leaves",
  "minecraft:dark_oak_log",
  "minecraft:dark_oak_leaves",
]);

const PLANT_AND_SNOW_BLOCKS = new Set([
  "minecraft:grass",
  "minecraft:fern",
  "minecraft:large_fern",
  "minecraft:oak_sapling",
  "minecraft:spruce_sapling",
  "minecraft:sweet_berry_bush",
  "minecraft:brown_mushroom",
  "minecraft:red_mushroom",
  "minecraft:sugar_cane",
  "minecraft:cactus",
  "minecraft:pumpkin",
  "minecraft:dandelion",
  "minecraft:poppy",
  "minecraft:snow",
]);

const ORE_AND_GLOW_BLOCKS = new Set([
  "minecraft:coal_ore",
  "minecraft:iron_ore",
  "minecraft:copper_ore",
  "minecraft:gold_ore",
  "minecraft:redstone_ore",
  "minecraft:lapis_ore",
  "minecraft:diamond_ore",
  "minecraft:emerald_ore",
  "minecraft:deepslate_coal_ore",
  "minecraft:deepslate_iron_ore",
  "minecraft:deepslate_copper_ore",
  "minecraft:deepslate_gold_ore",
  "minecraft:deepslate_redstone_ore",
  "minecraft:deepslate_lapis_ore",
  "minecraft:deepslate_diamond_ore",
  "minecraft:deepslate_emerald_ore",
  "minecraft:glow_lichen",
]);

const CARVER_EDGE_BLOCKS = new Set([
  "minecraft:cave_air",
  "minecraft:water",
]);

const DEEP_UNDERGROUND_BLOCKS = new Set([
  "minecraft:deepslate",
  "minecraft:tuff",
  "minecraft:granite",
  "minecraft:diorite",
  "minecraft:andesite",
  "minecraft:gravel",
  "minecraft:lava",
]);

const SURFACE_DIRT_GRASS_BLOCKS = new Set([
  "minecraft:dirt",
  "minecraft:grass_block",
]);

export function findDecoratedOracleChunk(
  fixture: DecoratedIntegrationOracleFixture,
  chunkX: number,
  chunkZ: number,
): DecoratedChunkOracleFixture {
  const chunk = fixture.chunks.find((candidate) => candidate.chunkX === chunkX && candidate.chunkZ === chunkZ);
  if (chunk === undefined) {
    throw new Error(`decorated oracle fixture is missing chunk (${chunkX}, ${chunkZ})`);
  }

  return chunk;
}

export function decoratedOracleBlockNameAt(
  chunk: DecoratedChunkOracleFixture,
  localX: number,
  y: number,
  localZ: number,
): string {
  const section = chunk.sections.find((candidate) => candidate.y === Math.floor(y / 16));
  if (section === undefined) {
    return "minecraft:air";
  }

  const index = ((y & 15) << 8) | (localZ << 4) | localX;
  return section.palette[section.blocks[index]!]!;
}

export function decoratedOracleGroundSurfaceAt(
  chunk: DecoratedChunkOracleFixture,
  localX: number,
  localZ: number,
): { readonly name: string; readonly y: number } {
  for (let y = DECORATED_CHUNK_MIN_Y + DECORATED_CHUNK_HEIGHT - 1; y >= DECORATED_CHUNK_MIN_Y; y--) {
    const name = decoratedOracleBlockNameAt(chunk, localX, y, localZ);
    if (!DECORATED_GROUND_IGNORED_BLOCKS.has(name)) {
      return { name, y };
    }
  }

  throw new Error(`decorated oracle chunk (${chunk.chunkX}, ${chunk.chunkZ}) had no ground block at (${localX}, ${localZ})`);
}

export function compareDecoratedChunkToOracle(
  chunk: DecoratedChunkOracleFixture,
  actualBlockNameAt: DecoratedChunkBlockNameAt,
  minY = DECORATED_CHUNK_MIN_Y,
  height = DECORATED_CHUNK_HEIGHT,
): DecoratedChunkDiff {
  const mismatches: DecoratedChunkMismatch[] = [];

  for (let y = minY; y < minY + height; y++) {
    for (let localZ = 0; localZ < 16; localZ++) {
      for (let localX = 0; localX < 16; localX++) {
        const expected = decoratedOracleBlockNameAt(chunk, localX, y, localZ);
        const actual = actualBlockNameAt(localX, y, localZ);
        if (actual !== expected) {
          mismatches.push({
            localX,
            y,
            localZ,
            expected,
            actual,
            bucket: classifyDecoratedChunkMismatch(expected, actual, y),
            yBand: yBand(y),
          });
        }
      }
    }
  }

  const totalBlocks = height * 16 * 16;
  return {
    totalBlocks,
    matches: totalBlocks - mismatches.length,
    mismatchCount: mismatches.length,
    mismatches,
    bucketCounts: countBy(mismatches, (mismatch) => mismatch.bucket),
    pairCounts: countBy(mismatches, (mismatch) => `${mismatch.expected} -> ${mismatch.actual}`),
    yBandCounts: countBy(mismatches, (mismatch) => mismatch.yBand),
  };
}

export function formatDecoratedChunkDiff(diff: DecoratedChunkDiff, limit = 12): string {
  return [
    `matches ${diff.matches.toLocaleString("en-US")} / ${diff.totalBlocks.toLocaleString("en-US")}`,
    `mismatches ${diff.mismatchCount.toLocaleString("en-US")}`,
    "buckets:",
    ...diff.bucketCounts.map((entry) => `  ${entry.key}: ${entry.count}`),
    "top pairs:",
    ...diff.pairCounts.slice(0, limit).map((entry) => `  ${entry.key}: ${entry.count}`),
    "first mismatches:",
    ...diff.mismatches.slice(0, limit).map((entry) =>
      `  (${entry.localX},${entry.y},${entry.localZ}) ${entry.expected} -> ${entry.actual} [${entry.bucket}]`
    ),
  ].join("\n");
}

function classifyDecoratedChunkMismatch(expected: string, actual: string, y: number): DecoratedChunkMismatchBucket {
  if (TREE_BLOCKS.has(expected) || TREE_BLOCKS.has(actual)) {
    return "Tree logs/leaves placement";
  }
  if (PLANT_AND_SNOW_BLOCKS.has(expected) || PLANT_AND_SNOW_BLOCKS.has(actual)) {
    return "Plants/snow decoration";
  }
  if (ORE_AND_GLOW_BLOCKS.has(expected) || ORE_AND_GLOW_BLOCKS.has(actual)) {
    return "Ores/glow lichen";
  }
  if (CARVER_EDGE_BLOCKS.has(expected) || CARVER_EDGE_BLOCKS.has(actual)) {
    return "Carver/fluid edge";
  }
  if (y < 64 && (DEEP_UNDERGROUND_BLOCKS.has(expected) || DEEP_UNDERGROUND_BLOCKS.has(actual))) {
    return "Deep underground blobs/lava";
  }
  if (SURFACE_DIRT_GRASS_BLOCKS.has(expected) || SURFACE_DIRT_GRASS_BLOCKS.has(actual)) {
    return "Surface dirt/grass choice";
  }
  return "Other";
}

function countBy<T>(items: readonly T[], keyFor: (item: T) => string): readonly DecoratedChunkMismatchCount[] {
  const counts = new Map<string, number>();
  for (const item of items) {
    const key = keyFor(item);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }

  return [...counts.entries()]
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .map(([key, count]) => ({ key, count }));
}

function yBand(y: number): string {
  const start = Math.floor(y / 16) * 16;
  return `${start}-${start + 15}`;
}
