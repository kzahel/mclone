import { describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import type { ClientWorld } from "../../src/runtime/client/client-world";
import {
  BotSpatialIndex,
  GoodViewBotController,
  selectGoodViewTarget,
  type BotControllerTickContext,
  type BotObservation,
} from "../../src/runtime/bot";
import type { ClientPlayerState } from "../../src/runtime/protocol/world-messages";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { buildChunkSnapshot, createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { ChunkBiomeContainer } from "../../src/worldgen/biome/chunk-biome-container";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { ChunkBlockId } from "../../src/worldgen/chunk/chunk-block-buffer";

const GOOD_VIEW_TEST_OPTIONS = {
  scanRadius: 5,
  verticalScanBelow: 2,
  verticalScanAbove: 8,
  horizonRayLength: 2,
  opennessRadius: 1,
  maxCandidateCount: 16,
  rescanIntervalMs: 1000,
  pathPlanner: {
    maxExpandedNodes: 256,
  },
};

function createClientWorld(options: {
  readonly supportBlocks?: readonly { readonly x: number; readonly y: number; readonly z: number }[];
  readonly applyChunk?: boolean;
} = {}): ClientWorld {
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

  if (options.applyChunk !== false) {
    const sourceChunk = new LevelChunk(0, 0, blocks.airState, 0, 32);
    const stone = blocks.blockStateById[ChunkBlockId.STONE]!;
    for (const block of options.supportBlocks ?? []) {
      sourceChunk.setBlockState(new BlockPos(block.x, block.y, block.z), stone);
    }
    level.applyChunkSnapshot(buildChunkSnapshot(
      sourceChunk,
      new ChunkBiomeContainer(0, 32, 0, 0, biomeSource).writeBiomes(),
      0,
      32,
    ));
  }

  return {
    getSessionState: () => undefined,
    getLocalPlayerState: () => undefined,
    getEntitySnapshots: () => [],
    getPerformanceSnapshot: () => undefined,
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

function createViewWorld(): ClientWorld {
  return createClientWorld({
    supportBlocks: [
      { x: 5, y: 4, z: 8 },
      { x: 6, y: 4, z: 8 },
      { x: 7, y: 5, z: 8 },
      { x: 8, y: 6, z: 8 },
      { x: 9, y: 7, z: 8 },
      { x: 10, y: 7, z: 8 },
      { x: 11, y: 7, z: 8 },
      { x: 9, y: 7, z: 7 },
      { x: 10, y: 7, z: 7 },
      { x: 11, y: 7, z: 7 },
      { x: 9, y: 7, z: 9 },
      { x: 10, y: 7, z: 9 },
      { x: 11, y: 7, z: 9 },
    ],
  });
}

function playerStateAt(x: number, y: number, z: number, yaw = 0): ClientPlayerState {
  return {
    playerId: "player-1",
    position: { x, y, z },
    rotation: {
      yaw,
      pitch: 0,
    },
    acknowledgedInputSequence: 0,
    tick: 0,
    revision: 0,
  };
}

function observationFor(playerState: ClientPlayerState, loaded = true): BotObservation {
  return {
    playerState,
    presentation: {
      localPlayerState: playerState,
      entities: [],
      entityPresentation: [],
    },
    revisionFacts: {},
    loadedChunks: loaded ? [{ chunkX: 0, chunkZ: 0 }] : [],
    loadedChunkCount: loaded ? 1 : 0,
    entityCount: 0,
  };
}

function tickContext(
  clientWorld: ClientWorld,
  playerState: ClientPlayerState,
  nowMs: number,
  elapsedMs = 50,
  loaded = true,
): BotControllerTickContext {
  return {
    clientWorld,
    observation: observationFor(playerState, loaded),
    nowMs,
    elapsedMs,
  };
}

describe("good-view bot goal", () => {
  test("selects a reachable high surface from loaded client-world facts", () => {
    const world = createViewWorld();
    const selection = selectGoodViewTarget(
      world,
      playerStateAt(6.5, 5, 8.5),
      new BotSpatialIndex(),
      GOOD_VIEW_TEST_OPTIONS,
      0,
      "chunk-0",
    );

    expect(selection.type).toBe("selected");
    if (selection.type === "selected") {
      expect(selection.target.surface.y).toBe(8);
      expect(selection.target.path[0]).toMatchObject({ x: 6, y: 5, z: 8 });
      expect(selection.target.path.at(-1)).toMatchObject({ y: 8 });
      expect(selection.target.score.pathLength).toBe(selection.target.path.length);
    }
  });

  test("walks toward the selected target and keeps it stable between rescans", () => {
    const world = createViewWorld();
    const controller = new GoodViewBotController(GOOD_VIEW_TEST_OPTIONS);
    const player = playerStateAt(6.5, 5, 8.5);

    const first = controller.tick(tickContext(world, player, 0));
    const firstTarget = controller.getSnapshot().target;
    const second = controller.tick(tickContext(world, player, 50));

    expect(first.status).toContain("good-view selected target");
    expect(first.movement).toMatchObject({ moveZ: 1 });
    expect(firstTarget).toBeDefined();
    expect(controller.getSnapshot().target?.surface).toEqual(firstTarget?.surface);
    expect(second.status).toContain("good-view moving target");
  });

  test("stops and slowly looks around after arriving at the target", () => {
    const world = createViewWorld();
    const controller = new GoodViewBotController(GOOD_VIEW_TEST_OPTIONS);
    controller.tick(tickContext(world, playerStateAt(6.5, 5, 8.5), 0));
    const target = controller.getSnapshot().target;
    expect(target).toBeDefined();

    const arrived = controller.tick(tickContext(
      world,
      playerStateAt(target!.surface.x + 0.5, target!.surface.y, target!.surface.z + 0.5, 90),
      50,
    ));

    expect(arrived.status).toContain("good-view arrived at target");
    expect(arrived.movement).toMatchObject({ moveZ: 0 });
    expect(arrived.movement?.yaw).toBeGreaterThan(90);
  });

  test("reports missing chunks instead of inventing terrain", () => {
    const world = createClientWorld({ applyChunk: false });
    const controller = new GoodViewBotController(GOOD_VIEW_TEST_OPTIONS);

    const result = controller.tick(tickContext(world, playerStateAt(6.5, 5, 8.5), 0, 50, false));

    expect(result.status).toContain("good-view waiting missing current surface");
    expect(result.movement).toMatchObject({ moveZ: 0 });
  });

  test("fails the active target when movement makes no progress", () => {
    const world = createViewWorld();
    const controller = new GoodViewBotController({
      ...GOOD_VIEW_TEST_OPTIONS,
      stuckTimeoutMs: 100,
      progressEpsilon: 0.001,
    });
    const player = playerStateAt(6.5, 5, 8.5);

    controller.tick(tickContext(world, player, 0, 50));
    controller.tick(tickContext(world, player, 50, 50));
    const stuck = controller.tick(tickContext(world, player, 200, 150));

    expect(stuck.status).toContain("good-view stuck target");
    expect(stuck.movement).toMatchObject({ moveZ: 0 });
    expect(controller.getSnapshot().target).toBeUndefined();
  });
});
