import { describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import type { ClientWorld } from "../../src/runtime/client/client-world";
import {
  planPathBetweenSurfaces,
  steerTowardWaypoint,
  type BotStandableSurface,
} from "../../src/runtime/bot";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { buildChunkSnapshot, createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { ChunkBiomeContainer } from "../../src/worldgen/biome/chunk-biome-container";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../../src/worldgen/chunk/chunk-block-buffer";

function createClientWorld(options: {
  readonly floor: readonly { readonly x: number; readonly z: number }[];
  readonly blockedFeet?: readonly { readonly x: number; readonly y: number; readonly z: number }[];
}): ClientWorld {
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
  for (const block of options.floor) {
    sourceChunk.setBlockState(new BlockPos(block.x, 4, block.z), stone);
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

function surface(x: number, z: number): BotStandableSurface {
  return {
    x,
    y: 5,
    z,
    supportY: 4,
  };
}

describe("bot navigation", () => {
  test("plans a bounded path over loaded standable surfaces", () => {
    const world = createClientWorld({
      floor: [0, 1, 2, 3, 4].map((x) => ({ x, z: 0 })),
    });

    const result = planPathBetweenSurfaces(world, surface(0, 0), surface(4, 0));

    expect(result.type).toBe("loaded");
    if (result.type === "loaded") {
      expect(result.path.map((step) => [step.x, step.y, step.z])).toEqual([
        [0, 5, 0],
        [1, 5, 0],
        [2, 5, 0],
        [3, 5, 0],
        [4, 5, 0],
      ]);
    }
  });

  test("reports blocked paths when no loaded route exists", () => {
    const world = createClientWorld({
      floor: [4, 5, 6, 7, 8].map((x) => ({ x, z: 8 })),
      blockedFeet: [{ x: 6, y: 5, z: 8 }],
    });

    expect(planPathBetweenSurfaces(world, surface(4, 8), surface(8, 8), {
      maxStepUp: 0,
      maxDrop: 0,
    })).toMatchObject({
      type: "blocked",
      reason: "no_path",
    });
  });

  test("reports missing chunks before planning through unavailable data", () => {
    const world = createClientWorld({
      floor: [0, 1, 2, 3, 4].map((x) => ({ x, z: 0 })),
    });

    expect(planPathBetweenSurfaces(world, surface(0, 0), {
      x: 20,
      y: 5,
      z: 0,
      supportY: 4,
    })).toMatchObject({
      type: "missing",
      missing: [{ chunkX: 1, chunkZ: 0 }],
    });
  });

  test("steers toward the next waypoint using normal player movement axes", () => {
    const playerState = {
      playerId: "player-1",
      position: {
        x: 0.5,
        y: 5,
        z: 0.5,
      },
      rotation: {
        yaw: 0,
        pitch: 0,
      },
      acknowledgedInputSequence: 0,
      tick: 0,
      revision: 0,
    };

    expect(steerTowardWaypoint(playerState, surface(0, 4))).toMatchObject({
      moveX: 0,
      moveZ: 1,
      yaw: 0,
    });
    expect(steerTowardWaypoint(playerState, surface(4, 0))).toMatchObject({
      moveX: 0,
      moveZ: 1,
      yaw: 270,
    });
    expect(steerTowardWaypoint(playerState, surface(0, 0))).toMatchObject({
      moveZ: 0,
    });
  });
});
