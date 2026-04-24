import { afterEach, describe, expect, test } from "vitest";
import surfaceFixture from "../fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json";
import fullFixture from "../fixtures/integration/overworld-seed-12345-chunks-0-0.json";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { createGeneratedWorldSaveId, GENERATED_WORLD_STORAGE_VERSION, GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import { LocalWorldClient, LocalWorldTransport } from "../../src/runtime/transport/local-world-transport";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";

interface SurfaceChunkOracleFixture {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly minY: number;
  readonly height: number;
  readonly palette: readonly string[];
  readonly blocks: readonly number[];
}

interface FullChunkOracleSectionFixture {
  readonly y: number;
  readonly palette: readonly string[];
  readonly blocks: readonly number[];
}

interface FullChunkOracleFixture {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly sections: readonly FullChunkOracleSectionFixture[];
}

interface FullIntegrationOracleFixture {
  readonly seed: string;
  readonly chunks: readonly FullChunkOracleFixture[];
}

const oracle = surfaceFixture as SurfaceChunkOracleFixture;
const fullOracle = fullFixture as FullIntegrationOracleFixture;
const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
} as const;
const GENERATED_WORLD_LIGHTING_TIMEOUT_MS = 15_000;
const DECORATION_BLOCKS = new Set([
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

function oracleBlockNameAt(localX: number, y: number, localZ: number): string {
  const index = ((y - oracle.minY) << 8) | (localZ << 4) | localX;
  return oracle.palette[oracle.blocks[index]!]!;
}

function oracleSurfaceAt(localX: number, localZ: number): { readonly name: string; readonly y: number } {
  for (let y = oracle.minY + oracle.height - 1; y >= oracle.minY; y--) {
    const name = oracleBlockNameAt(localX, y, localZ);
    if (name !== "minecraft:air") {
      return { name, y };
    }
  }

  throw new Error(`oracle chunk (${oracle.chunkX}, ${oracle.chunkZ}) had no surface block at (${localX}, ${localZ})`);
}

function fullOracleChunk(): FullChunkOracleFixture {
  const chunk = fullOracle.chunks.find((candidate) => candidate.chunkX === 0 && candidate.chunkZ === 0);
  if (chunk === undefined) {
    throw new Error("full oracle fixture is missing chunk (0, 0)");
  }

  return chunk;
}

function fullOracleBlockNameAt(chunk: FullChunkOracleFixture, localX: number, y: number, localZ: number): string {
  const section = chunk.sections.find((candidate) => candidate.y === Math.floor(y / 16));
  if (section === undefined) {
    return "minecraft:air";
  }

  const index = ((y & 15) << 8) | (localZ << 4) | localX;
  return section.palette[section.blocks[index]!]!;
}

function fullOracleGroundSurfaceAt(chunk: FullChunkOracleFixture, localX: number, localZ: number): { readonly name: string; readonly y: number } {
  for (let y = 255; y >= 0; y--) {
    const name = fullOracleBlockNameAt(chunk, localX, y, localZ);
    if (name !== "minecraft:air" && name !== "minecraft:cave_air" && name !== "minecraft:water" && !DECORATION_BLOCKS.has(name)) {
      return { name, y };
    }
  }

  throw new Error(`full oracle chunk (${chunk.chunkX}, ${chunk.chunkZ}) had no ground block at (${localX}, ${localZ})`);
}

function runtimeGroundSurfaceAt(level: ClientChunkCache, worldX: number, worldZ: number): { readonly name: string; readonly y: number } {
  for (let y = level.getMaxBuildHeight() - 1; y >= level.getMinBuildHeight(); y--) {
    const state = level.getBlockState(new BlockPos(worldX, y, worldZ));
    if (!state.isAir()) {
      const name = Registry.BLOCK.getKey(state.getBlock() as unknown as object)?.toString();
      if (name !== undefined && DECORATION_BLOCKS.has(name)) {
        continue;
      }

      return { name: name ?? "unregistered", y };
    }
  }

  throw new Error(`runtime level had no surface block at (${worldX}, ${worldZ})`);
}

function createWorldClient(): LocalWorldClient {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);

  return new LocalWorldClient(
    new LocalWorldTransport(
      new GeneratedWorldHost({
        seed: 12345n,
        airState: blocks.airState,
        blockStateById: blocks.blockStateById,
        blockStateIds: blocks.blockStateIds,
      }),
    ),
    (worldOpened) => new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: worldOpened.minBuildHeight,
      height: worldOpened.height,
      biomeSource,
      biomeZoomSeed: 12345n,
      blockStateResolver: createBlockStateResolver(blocks.airState),
      blockStateIds: blocks.blockStateIds,
    }),
  );
}

describe("GeneratedWorld boundary", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("loads the oracle chunk through the local host/client boundary", async () => {
    const client = createWorldClient();

    await expect(client.openWorld(OPEN_WORLD_REQUEST)).resolves.toEqual({
      type: "world_opened",
      minBuildHeight: 0,
      height: 256,
      saveMetadata: {
        saveId: createGeneratedWorldSaveId(12345n, "default"),
        storageVersion: GENERATED_WORLD_STORAGE_VERSION,
        seed: "12345",
        preset: "default",
        minBuildHeight: 0,
        height: 256,
        createdAtMs: expect.any(Number),
        lastOpenedAtMs: expect.any(Number),
      },
    });
    expect(
      await client.setChunkView({
        type: "set_chunk_view",
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      }),
    ).toBe(true);

    const level = client.getLevel();
    expect(level.getLoadedChunkCount()).toBe(25);
    expect(level.getChunk(0, 0, false)).not.toBeNull();

    for (const [localX, localZ] of [
      [0, 0],
      [8, 8],
      [15, 15],
    ] as const) {
      expect(runtimeGroundSurfaceAt(level, localX, localZ)).toEqual(oracleSurfaceAt(localX, localZ));
    }
  }, GENERATED_WORLD_LIGHTING_TIMEOUT_MS);

  test("keeps chunk (0, 0) dry-land ground materials close to the full vanilla oracle", async () => {
    const client = createWorldClient();

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const level = client.getLevel();
    const chunk = fullOracleChunk();
    const mismatches: string[] = [];
    const unexpectedSand: string[] = [];

    for (let localZ = 0; localZ < 16; localZ++) {
      for (let localX = 0; localX < 16; localX++) {
        const expected = fullOracleGroundSurfaceAt(chunk, localX, localZ);
        const actual = runtimeGroundSurfaceAt(level, localX, localZ);
        if (actual.name !== expected.name) {
          mismatches.push(`${localX},${localZ}: expected ${expected.name}@${expected.y}, got ${actual.name}@${actual.y}`);
        }
        if (actual.name === "minecraft:sand" && expected.name !== "minecraft:sand") {
          unexpectedSand.push(`${localX},${localZ}: expected ${expected.name}@${expected.y}, got ${actual.name}@${actual.y}`);
        }
      }
    }

    expect(unexpectedSand).toEqual([]);
    expect(256 - mismatches.length, mismatches.slice(0, 10).join("\n")).toBeGreaterThanOrEqual(230);
  }, GENERATED_WORLD_LIGHTING_TIMEOUT_MS);

  test("slides the client chunk cache when the chunk view moves", async () => {
    const client = createWorldClient();

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const level = client.getLevel();
    expect(level.getChunk(-2, 0, false)).not.toBeNull();
    expect(level.getChunk(2, 0, false)).not.toBeNull();

    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 2,
      centerChunkZ: 0,
      radius: 1,
    });
    expect(level.getLoadedChunkCount()).toBe(25);
    expect(level.getChunk(-2, 0, false)).toBeNull();
    expect(level.getChunk(0, 0, false)).not.toBeNull();
    expect(level.getChunk(4, 0, false)).not.toBeNull();
  }, GENERATED_WORLD_LIGHTING_TIMEOUT_MS);

  test("preserves biome decoration when authoritative chunks are snapshotted to the client cache", async () => {
    const client = createWorldClient();

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 2,
      centerChunkZ: 2,
      radius: 1,
    });

    const level = client.getLevel();
    let foundTree = false;
    let foundGroundPlant = false;
    for (const chunk of level.getLoadedChunks()) {
      const minX = chunk.chunkX * 16;
      const minZ = chunk.chunkZ * 16;
      for (let y = level.getMinBuildHeight(); y < level.getMaxBuildHeight(); y++) {
        for (let localZ = 0; localZ < 16; localZ++) {
          for (let localX = 0; localX < 16; localX++) {
            const name = Registry.BLOCK.getKey(level.getBlockState(new BlockPos(minX + localX, y, minZ + localZ)).getBlock() as unknown as object)?.toString();
            if (name === "minecraft:spruce_log" || name === "minecraft:oak_log") {
              foundTree = true;
            } else if (name === "minecraft:grass" || name === "minecraft:fern") {
              foundGroundPlant = true;
            }
          }
        }
      }
    }

    expect(foundTree).toBe(true);
    expect(foundGroundPlant).toBe(true);
  }, GENERATED_WORLD_LIGHTING_TIMEOUT_MS);

  test("applies authoritative player input through the local host/client boundary", async () => {
    const client = createWorldClient();

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const initialPlayerState = client.getPlayerState();
    expect(initialPlayerState).toBeDefined();

    expect(await client.setPlayerInput({
      type: "set_player_input",
      input: {
        sequence: 1,
        moveX: 1,
        moveY: 0,
        moveZ: 0,
        yaw: 90,
        pitch: 15,
      },
    })).toBe(true);

    await new Promise((resolve) => setTimeout(resolve, 60));
    expect(await client.pollUpdates()).toBe(true);

    expect(client.getPlayerState()?.acknowledgedInputSequence).toBe(1);
    expect(client.getPlayerState()?.rotation).toEqual({
      yaw: 90,
      pitch: 15,
    });
    expect(client.getPlayerState()!.position.x).toBeGreaterThan(initialPlayerState!.position.x);
    expect(client.getPlayerState()!.revision).toBeGreaterThan(initialPlayerState!.revision);
  }, GENERATED_WORLD_LIGHTING_TIMEOUT_MS);
});
