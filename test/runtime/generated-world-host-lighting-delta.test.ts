import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { SectionPos } from "../../src/core/section-pos";
import { Registry } from "../../src/core/registry";
import { GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import type {
  ConfigureLightingWorldRequest,
  LightBlockChangeBatchRequest,
  LightingResult,
  LightingResultBatch,
  LightingService,
  PollLightingResultsRequest,
  RemoveLightChunkRequest,
  RequestInitialLightRequest,
  SetLightingViewRequest,
  UpsertLightChunkRequest,
} from "../../src/runtime/lighting/lighting-protocol";
import type { ChunkLightDeltaMessage, ChunkSnapshotMessage, WorldHostMessage } from "../../src/runtime/protocol/world-messages";
import { DataLayer } from "../../src/world/level/chunk/data-layer";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import type { WorldGenLevel } from "../../src/world/level/world-gen-level";
import { ChunkBlockId } from "../../src/worldgen/chunk/chunk-block-buffer";

const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
} as const;

class RecordingLightingService implements LightingService {
  public readonly blockChangeBatches: LightBlockChangeBatchRequest[] = [];
  public readonly initialLightRequests: RequestInitialLightRequest[] = [];
  public readonly upsertRequests: UpsertLightChunkRequest[] = [];
  private readonly results: LightingResult[] = [];

  public configureWorld(_request: ConfigureLightingWorldRequest): Promise<void> {
    return Promise.resolve();
  }

  public setView(_request: SetLightingViewRequest): Promise<void> {
    return Promise.resolve();
  }

  public upsertChunk(request: UpsertLightChunkRequest): Promise<void> {
    this.upsertRequests.push(request);
    return Promise.resolve();
  }

  public removeChunk(_request: RemoveLightChunkRequest): Promise<void> {
    return Promise.resolve();
  }

  public requestInitialLight(request: RequestInitialLightRequest): Promise<void> {
    this.initialLightRequests.push(request);
    this.results.push({
      type: "chunk_light_ready",
      chunkViewRevision: request.chunkViewRevision,
      chunkX: request.chunkX,
      chunkZ: request.chunkZ,
      chunkRevision: request.chunkRevision,
      light: {
        sky: [],
        block: [],
        lightCorrect: true,
      },
    });
    return Promise.resolve();
  }

  public enqueueBlockChanges(request: LightBlockChangeBatchRequest): Promise<void> {
    this.blockChangeBatches.push(request);
    const changedChunks = new Set(request.changes.map((change) =>
      `${SectionPos.blockToSectionCoord(change.x)},${SectionPos.blockToSectionCoord(change.z)}`,
    ));
    for (const revision of request.chunkRevisions) {
      if (!changedChunks.has(`${revision.chunkX},${revision.chunkZ}`)) {
        continue;
      }

      const data = new Uint8Array(DataLayer.SIZE);
      data[0] = 15;
      this.results.push({
        type: "chunk_light_delta",
        chunkViewRevision: revision.chunkViewRevision,
        chunkX: revision.chunkX,
        chunkZ: revision.chunkZ,
        chunkRevision: revision.chunkRevision,
        light: {
          block: [{ y: 15, data }],
        },
      });
    }
    this.results.push({
      type: "block_light_update_complete",
      batchId: request.batchId,
      chunkViewRevision: request.chunkViewRevision,
      changeCount: request.changes.length,
      chunkRevisions: request.chunkRevisions,
    });
    return Promise.resolve();
  }

  public pollResults(request: PollLightingResultsRequest): Promise<LightingResultBatch> {
    const maxResults = request.maxResults ?? this.results.length;
    return Promise.resolve({
      type: "lighting_result_batch",
      results: this.results.splice(0, maxResults),
      pendingResultCount: this.results.length,
    });
  }
}

function chunkSnapshots(messages: readonly WorldHostMessage[]): ChunkSnapshotMessage[] {
  return messages.filter((message): message is ChunkSnapshotMessage => message.type === "chunk_snapshot");
}

function chunkLightDeltas(messages: readonly WorldHostMessage[]): ChunkLightDeltaMessage[] {
  return messages.filter((message): message is ChunkLightDeltaMessage => message.type === "chunk_light_delta");
}

describe("GeneratedWorldHost lighting deltas", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("routes live block mutations through worker block light batches before publishing snapshots", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const lightingService = new RecordingLightingService();
    let nowMs = 0;
    const host = new GeneratedWorldHost({
      seed: OPEN_WORLD_REQUEST.seed,
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingService,
      worldTickIntervalMs: 1,
      nowMs: () => nowMs,
    });

    await host.openWorld(OPEN_WORLD_REQUEST);
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const mutationLevel = (host as unknown as { readonly liquidLevel: WorldGenLevel }).liquidLevel;
    const lavaState = blocks.blockStateById[ChunkBlockId.LAVA]!;
    const pos = new BlockPos(0, 250, 0);
    expect(mutationLevel.setBlock(pos, lavaState)).toBe(true);

    nowMs = 2;
    const updates = await host.pollUpdates({ type: "poll_world_updates" });

    expect(lightingService.blockChangeBatches).toHaveLength(1);
    expect(lightingService.blockChangeBatches[0]!.changes).toContainEqual({
      x: pos.getX(),
      y: pos.getY(),
      z: pos.getZ(),
      oldBlockStateId: blocks.blockStateIds.idFor(blocks.airState),
      newBlockStateId: blocks.blockStateIds.idFor(lavaState),
    });
    expect(chunkLightDeltas(updates).filter((message) => message.chunkX === 0 && message.chunkZ === 0)).toEqual([]);

    const snapshot = chunkSnapshots(updates).find((message) => message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0);
    expect(snapshot).toBeDefined();
    expect(snapshot!.snapshot.light?.block).toContainEqual({
      y: 15,
      data: expect.any(Uint8Array),
    });
  }, 30_000);

  test("requests initial light only from feature-complete 3x3 inputs", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const lightingService = new RecordingLightingService();
    const host = new GeneratedWorldHost({
      seed: OPEN_WORLD_REQUEST.seed,
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingService,
    });

    await host.openWorld(OPEN_WORLD_REQUEST);
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    expect(lightingService.initialLightRequests.length).toBeGreaterThan(0);
    expect(lightingService.upsertRequests.every((request) => request.decorated)).toBe(true);

    const upserted = new Set(lightingService.upsertRequests.map((request) => `${request.chunkX},${request.chunkZ}`));
    const centerRequest = lightingService.initialLightRequests.find((request) => request.chunkX === 0 && request.chunkZ === 0);
    expect(centerRequest).toBeDefined();
    expect(centerRequest!.neighbors).toHaveLength(8);
    expect(upserted.has("0,0")).toBe(true);
    for (const neighbor of centerRequest!.neighbors) {
      expect(upserted.has(`${neighbor.chunkX},${neighbor.chunkZ}`)).toBe(true);
    }
  }, 30_000);
});
