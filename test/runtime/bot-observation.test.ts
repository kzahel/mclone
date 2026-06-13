import { describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import type { ClientWorld } from "../../src/runtime/client/client-world";
import {
  BotSpatialIndex,
  queryBotBlock,
  queryBotStandableSurface,
  scanStandableSurfaces,
} from "../../src/runtime/bot";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { buildChunkSnapshot, createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { ChunkBiomeContainer } from "../../src/worldgen/biome/chunk-biome-container";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../../src/worldgen/chunk/chunk-block-buffer";

function createClientWorldWithFloor(options: {
  readonly floorY?: number;
  readonly minX?: number;
  readonly maxX?: number;
  readonly minZ?: number;
  readonly maxZ?: number;
  readonly blockedFeet?: readonly { readonly x: number; readonly y: number; readonly z: number }[];
} = {}): ClientWorld {
  const floorY = options.floorY ?? 4;
  const minX = options.minX ?? 0;
  const maxX = options.maxX ?? 4;
  const minZ = options.minZ ?? 0;
  const maxZ = options.maxZ ?? 0;
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);
  const level = new ClientChunkCache({
    airState: blocks.airState,
    minBuildHeight: 0,
    height: 32,
    biomeSource,
    biomeZoomSeed: 12345n,
    blockStateResolver: createBlockStateResolver(blocks.airState),
    blockStateIds: blocks.blockStateIds,
  });
  const sourceChunk = new LevelChunk(0, 0, blocks.airState, 0, 32);
  const stone = blocks.blockStateById[ChunkBlockId.STONE]!;
  for (let z = minZ; z <= maxZ; z++) {
    for (let x = minX; x <= maxX; x++) {
      sourceChunk.setBlockState(new BlockPos(x, floorY, z), stone);
    }
  }
  for (const block of options.blockedFeet ?? []) {
    sourceChunk.setBlockState(new BlockPos(block.x, block.y, block.z), stone);
  }
  level.applyChunkSnapshot(buildChunkSnapshot(
    sourceChunk,
    new ChunkBiomeContainer(0, 32, 0, 0, biomeSource).writeBiomes(),
    0,
    32,
  ));

  return {
    getSessionState: () => undefined,
    getLocalPlayerState: () => undefined,
    getEntitySnapshots: () => [],
    getPerformanceSnapshot: () => undefined,
    getChunkLifecycleSnapshot: () => undefined,
    getChunkSnapshot: (chunkX, chunkZ) => level.getChunkSnapshot(chunkX, chunkZ),
    getRevisionFacts: () => ({}),
    getRenderView: () => ({
      getRenderLevel: () => level,
    }),
    getEntityView: () => ({
      getEntitySnapshots: () => [],
    }),
    getPredictionView: () => ({
      getAuthoritativeMovementState: () => undefined,
      getEntitySnapshots: () => [],
      createCollisionWorld: () => {
        throw new Error("not used");
      },
    }),
  };
}

describe("bot client-world observation", () => {
  test("distinguishes loaded blocks from missing chunks", () => {
    const world = createClientWorldWithFloor();

    const loaded = queryBotBlock(world, { x: 1, y: 4, z: 0 });
    expect(loaded.type).toBe("loaded");
    if (loaded.type === "loaded") {
      expect(loaded.state.isAir()).toBe(false);
    }

    expect(queryBotBlock(world, { x: 20, y: 4, z: 0 })).toMatchObject({
      type: "missing",
      chunkX: 1,
      chunkZ: 0,
      reason: "missing_chunk",
    });
    expect(queryBotBlock(world, { x: 1, y: -1, z: 0 })).toMatchObject({
      type: "missing",
      reason: "out_of_build_height",
    });
  });

  test("finds standable surfaces with body headroom", () => {
    const world = createClientWorldWithFloor({
      blockedFeet: [{ x: 2, y: 5, z: 0 }],
    });

    expect(queryBotStandableSurface(world, { x: 1, y: 5, z: 0 })).toMatchObject({
      type: "loaded",
      surface: {
        x: 1,
        y: 5,
        z: 0,
        supportY: 4,
      },
    });
    expect(queryBotStandableSurface(world, { x: 2, y: 5, z: 0 })).toMatchObject({
      type: "blocked",
      reason: "blocked_body",
    });
  });

  test("scans loaded standable surfaces and reports missing chunks", () => {
    const world = createClientWorldWithFloor({
      blockedFeet: [{ x: 2, y: 5, z: 0 }],
    });

    const result = scanStandableSurfaces(world, {
      minX: 0,
      maxX: 20,
      minY: 5,
      maxY: 5,
      minZ: 0,
      maxZ: 0,
    });

    expect(result.type).toBe("missing");
    expect(result.surfaces.map((surface) => surface.x)).toEqual([0, 1, 3, 4]);
    expect(result.missing).toEqual([{ chunkX: 1, chunkZ: 0 }]);
  });

  test("caches scan results until loaded chunk or bounds key changes", () => {
    const world = createClientWorldWithFloor();
    const index = new BotSpatialIndex();
    const bounds = {
      minX: 0,
      maxX: 4,
      minY: 5,
      maxY: 5,
      minZ: 0,
      maxZ: 0,
    };

    const first = index.scanStandableSurfaces(world, bounds);
    const second = index.scanStandableSurfaces(world, bounds);
    expect(second).toBe(first);

    const third = index.scanStandableSurfaces(world, { ...bounds, maxX: 3 });
    expect(third).not.toBe(first);
  });
});
