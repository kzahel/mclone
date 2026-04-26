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
import {
  compareDecoratedChunkToOracle,
  DECORATED_GROUND_IGNORED_BLOCKS,
  decoratedOracleGroundSurfaceAt,
  findDecoratedOracleChunk,
  formatDecoratedChunkDiff,
  type DecoratedIntegrationOracleFixture,
} from "../../src/oracle/integration/decorated-chunk-fixture.ts";

interface SurfaceChunkOracleFixture {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly minY: number;
  readonly height: number;
  readonly palette: readonly string[];
  readonly blocks: readonly number[];
}

const oracle = surfaceFixture as SurfaceChunkOracleFixture;
const fullOracle = fullFixture as DecoratedIntegrationOracleFixture;
const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
} as const;
const GENERATED_WORLD_LIGHTING_TIMEOUT_MS = 30_000;
const GENERATED_WORLD_TICK_INTERVAL_MS = 1000;

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

function runtimeGroundSurfaceAt(level: ClientChunkCache, worldX: number, worldZ: number): { readonly name: string; readonly y: number } {
  for (let y = level.getMaxBuildHeight() - 1; y >= level.getMinBuildHeight(); y--) {
    const state = level.getBlockState(new BlockPos(worldX, y, worldZ));
    if (!state.isAir()) {
      const name = Registry.BLOCK.getKey(state.getBlock() as unknown as object)?.toString();
      if (name !== undefined && DECORATED_GROUND_IGNORED_BLOCKS.has(name)) {
        continue;
      }

      return { name: name ?? "unregistered", y };
    }
  }

  throw new Error(`runtime level had no surface block at (${worldX}, ${worldZ})`);
}

function runtimeBlockNameAt(level: ClientChunkCache, worldX: number, y: number, worldZ: number): string {
  const state = level.getBlockState(new BlockPos(worldX, y, worldZ));
  return Registry.BLOCK.getKey(state.getBlock() as unknown as object)?.toString() ?? "unregistered";
}

function createWorldClient(options: {
  readonly nowMs?: () => number;
  readonly worldTickIntervalMs?: number;
} = {}): LocalWorldClient {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);

  return new LocalWorldClient(
    new LocalWorldTransport(
      new GeneratedWorldHost({
        seed: 12345n,
        airState: blocks.airState,
        blockStateById: blocks.blockStateById,
        blockStateIds: blocks.blockStateIds,
        lightingMode: "none",
        worldTickIntervalMs: options.worldTickIntervalMs,
        nowMs: options.nowMs,
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
    const chunk = findDecoratedOracleChunk(fullOracle, 0, 0);
    const mismatches: string[] = [];
    const unexpectedSand: string[] = [];

    for (let localZ = 0; localZ < 16; localZ++) {
      for (let localX = 0; localX < 16; localX++) {
        const expected = decoratedOracleGroundSurfaceAt(chunk, localX, localZ);
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

  test("matches chunk (0, 0) full-block decorated parity after fixture-equivalent liquid ticks", async () => {
    let nowMs = 0;
    const client = createWorldClient({
      nowMs: () => nowMs,
      worldTickIntervalMs: GENERATED_WORLD_TICK_INTERVAL_MS,
    });

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const level = client.getLevel();
    const chunk = findDecoratedOracleChunk(fullOracle, 0, 0);
    const preTickDiff = compareDecoratedChunkToOracle(
      chunk,
      (localX, y, localZ) => runtimeBlockNameAt(level, localX, y, localZ),
    );

    expect(preTickDiff.matches, formatDecoratedChunkDiff(preTickDiff)).toBe(65_533);
    expect(preTickDiff.mismatchCount, formatDecoratedChunkDiff(preTickDiff)).toBe(3);

    nowMs += 10 * GENERATED_WORLD_TICK_INTERVAL_MS;
    expect(await client.pollUpdates()).toBe(true);

    const postTickDiff = compareDecoratedChunkToOracle(
      chunk,
      (localX, y, localZ) => runtimeBlockNameAt(level, localX, y, localZ),
    );

    expect(postTickDiff.matches, formatDecoratedChunkDiff(postTickDiff)).toBe(65_536);
    expect(postTickDiff.mismatchCount, formatDecoratedChunkDiff(postTickDiff)).toBe(0);
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
    expect(client.getPlayerState()!.position).not.toEqual(initialPlayerState!.position);
    expect(client.getPlayerState()!.revision).toBeGreaterThan(initialPlayerState!.revision);
  }, GENERATED_WORLD_LIGHTING_TIMEOUT_MS);
});
