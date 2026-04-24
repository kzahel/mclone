import { describe, expect, test } from "vitest";
import {
  connectRenderWorldWorkerSession,
  RenderWorldWorkerClient,
  RenderWorldWorkerUpdateSink,
  type RenderWorldWorkerClientEndpoint,
  type RenderWorldWorkerHostEndpoint,
  type RenderWorldWorkerMessageListener,
  type RenderWorldWorkerRequestEnvelope,
  type RenderWorldWorkerResponseEnvelope,
} from "../../../src/renderer/chunk/render-world-worker-client";
import type { SectionMeshResult } from "../../../src/renderer/chunk/chunk-mesh-protocol";
import type {
  RenderWorldRequest,
  RenderWorldResponse,
  RenderWorldSectionOrigin,
} from "../../../src/renderer/chunk/render-world-protocol";
import type { PackedChunkSnapshot } from "../../../src/world/level/packed-chunk-snapshot";

class TestMessageEndpoint<TOutgoing, TIncoming> {
  private peer?: TestMessageEndpoint<TIncoming, TOutgoing>;
  private readonly messageListeners = new Set<RenderWorldWorkerMessageListener<TIncoming>>();
  private readonly errorListeners = new Set<(event: unknown) => void>();
  private readonly messages: TOutgoing[] = [];
  private readonly transfers: Transferable[][] = [];

  public connect(peer: TestMessageEndpoint<TIncoming, TOutgoing>): void {
    this.peer = peer;
  }

  public postMessage(message: TOutgoing, transfer: readonly Transferable[] = []): void {
    this.messages.push(message);
    this.transfers.push([...transfer]);
    queueMicrotask(() => {
      this.peer?.dispatchMessage(message);
    });
  }

  public addEventListener(type: "message" | "error" | "messageerror", listener: ((event: unknown) => void) | RenderWorldWorkerMessageListener<TIncoming>): void {
    if (type === "message") {
      this.messageListeners.add(listener as RenderWorldWorkerMessageListener<TIncoming>);
      return;
    }

    this.errorListeners.add(listener as (event: unknown) => void);
  }

  public removeEventListener(type: "message" | "error" | "messageerror", listener: ((event: unknown) => void) | RenderWorldWorkerMessageListener<TIncoming>): void {
    if (type === "message") {
      this.messageListeners.delete(listener as RenderWorldWorkerMessageListener<TIncoming>);
      return;
    }

    this.errorListeners.delete(listener as (event: unknown) => void);
  }

  public emitError(error: unknown): void {
    for (const listener of this.errorListeners) {
      listener(error);
    }
  }

  public getMessages(): readonly TOutgoing[] {
    return this.messages;
  }

  public getTransfers(): readonly (readonly Transferable[])[] {
    return this.transfers;
  }

  private dispatchMessage(message: TIncoming): void {
    for (const listener of this.messageListeners) {
      listener({ data: message });
    }
  }
}

function createEndpointPair(): {
  readonly clientEndpoint: RenderWorldWorkerClientEndpoint;
  readonly hostEndpoint: RenderWorldWorkerHostEndpoint;
  readonly rawClientEndpoint: TestMessageEndpoint<RenderWorldWorkerRequestEnvelope, RenderWorldWorkerResponseEnvelope>;
  readonly rawHostEndpoint: TestMessageEndpoint<RenderWorldWorkerResponseEnvelope, RenderWorldWorkerRequestEnvelope>;
} {
  const rawClientEndpoint = new TestMessageEndpoint<RenderWorldWorkerRequestEnvelope, RenderWorldWorkerResponseEnvelope>();
  const rawHostEndpoint = new TestMessageEndpoint<RenderWorldWorkerResponseEnvelope, RenderWorldWorkerRequestEnvelope>();
  rawClientEndpoint.connect(rawHostEndpoint);
  rawHostEndpoint.connect(rawClientEndpoint);
  return {
    clientEndpoint: rawClientEndpoint as unknown as RenderWorldWorkerClientEndpoint,
    hostEndpoint: rawHostEndpoint as unknown as RenderWorldWorkerHostEndpoint,
    rawClientEndpoint,
    rawHostEndpoint,
  };
}

function createPackedChunkSnapshot(chunkX = 2, sectionCount = 1): PackedChunkSnapshot {
  return {
    chunkX,
    chunkZ: -3,
    biomes: [1, 2, 3],
    sections: Array.from({ length: sectionCount }, (_, sectionIndex) => ({
      y: sectionIndex,
      paletteStateIds: new Uint32Array([1, 2]),
      bitsPerBlock: 4,
      packedBlockIndices: new BigInt64Array([0n, 1n]),
    })),
    blockTicks: [],
    liquidTicks: [],
  };
}

function createMeshResult(buffer = new Uint8Array([1, 2, 3, 4])): SectionMeshResult {
  return {
    isCompletelyEmpty: false,
    hasBlocks: ["solid"],
    hasLayers: ["solid"],
    visibilitySet: Array.from({ length: 36 }, () => true),
    transparencyState: undefined,
    layers: [{
      renderType: "solid",
      drawState: {
        format: "block",
        vertexCount: 4,
        indexCount: 6,
        mode: "quads",
        indexType: "short",
        indexOnly: false,
        sequentialIndex: true,
      },
      buffer,
    }],
  };
}

function sectionOrigin(x = 0, y = 0, z = 0): RenderWorldSectionOrigin {
  return { x, y, z };
}

describe("RenderWorld worker client", () => {
  test("initializes over a fake endpoint and preserves request ids", async () => {
    const { clientEndpoint, hostEndpoint, rawClientEndpoint } = createEndpointPair();
    const received: RenderWorldRequest[] = [];

    connectRenderWorldWorkerSession(hostEndpoint, async (message) => {
      received.push(message);
      if (message.type === "initialize_render_world") {
        return { type: "render_world_ready" };
      }
      return {
        type: "render_world_stats",
        stats: { loadedChunkCount: 0 },
      };
    });

    const client = new RenderWorldWorkerClient(clientEndpoint);
    await client.initialize({
      type: "initialize_render_world",
      seed: 12345n,
      minBuildHeight: 0,
      height: 256,
    });
    await expect(client.getStats()).resolves.toEqual({ loadedChunkCount: 0 });

    expect(received).toEqual([
      {
        type: "initialize_render_world",
        seed: 12345n,
        minBuildHeight: 0,
        height: 256,
      },
      { type: "get_render_world_stats" },
    ]);
    expect(rawClientEndpoint.getMessages().map((message) => message.requestId)).toEqual([1, 2]);
  });

  test("ingests packed chunk updates with transferable buffers and returns dirty section metadata", async () => {
    const { clientEndpoint, hostEndpoint, rawClientEndpoint } = createEndpointPair();
    const snapshot = createPackedChunkSnapshot();

    connectRenderWorldWorkerSession(hostEndpoint, async (message): Promise<RenderWorldResponse> => {
      expect(message).toEqual({
        type: "ingest_render_world_updates",
        messages: [
          { type: "chunk_snapshot", snapshot },
          { type: "chunk_unload", chunkX: 5, chunkZ: 6 },
        ],
      });
      return {
        type: "render_world_dirty_sections",
        dirtySections: [sectionOrigin(32, 0, -48)],
        stats: { loadedChunkCount: 1 },
      };
    });

    const client = new RenderWorldWorkerClient(clientEndpoint);
    await expect(client.ingestUpdates({
      type: "ingest_render_world_updates",
      messages: [
        { type: "chunk_snapshot", snapshot },
        { type: "chunk_unload", chunkX: 5, chunkZ: 6 },
      ],
    })).resolves.toEqual({
      type: "render_world_dirty_sections",
      dirtySections: [sectionOrigin(32, 0, -48)],
      stats: { loadedChunkCount: 1 },
    });

    expect(rawClientEndpoint.getTransfers()).toHaveLength(1);
    expect(rawClientEndpoint.getTransfers()[0]).toEqual([
      snapshot.sections[0]!.paletteStateIds.buffer,
      snapshot.sections[0]!.packedBlockIndices.buffer,
    ]);
  });

  test("adapts world chunk updates into render-world worker ingest requests", async () => {
    const { clientEndpoint, hostEndpoint } = createEndpointPair();
    const snapshot = createPackedChunkSnapshot();

    connectRenderWorldWorkerSession(hostEndpoint, async (message): Promise<RenderWorldResponse> => {
      expect(message).toEqual({
        type: "ingest_render_world_updates",
        messages: [{ type: "chunk_snapshot", snapshot }],
      });
      return {
        type: "render_world_dirty_sections",
        dirtySections: [sectionOrigin(32, 0, -48)],
        stats: { loadedChunkCount: 9 },
      };
    });

    const sink = new RenderWorldWorkerUpdateSink(new RenderWorldWorkerClient(clientEndpoint));
    await expect(sink.ingestUpdates([{ type: "chunk_snapshot", snapshot }])).resolves.toEqual({
      chunkChanged: true,
    });
    expect(sink.getStats()).toEqual({ loadedChunkCount: 9 });
    expect(sink.drainDirtySections()).toEqual([sectionOrigin(32, 0, -48)]);
    expect(sink.drainDirtySections()).toEqual([]);
  });

  test("batches large render-world update ingests to keep transfer lists bounded", async () => {
    const { clientEndpoint, hostEndpoint, rawClientEndpoint } = createEndpointPair();
    const snapshots = Array.from({ length: 9 }, (_, index) => createPackedChunkSnapshot(index, 16));
    const receivedBatchSizes: number[] = [];

    connectRenderWorldWorkerSession(hostEndpoint, async (message): Promise<RenderWorldResponse> => {
      expect(message.type).toBe("ingest_render_world_updates");
      if (message.type !== "ingest_render_world_updates") {
        throw new Error(`Unexpected message ${message.type}`);
      }

      receivedBatchSizes.push(message.messages.length);
      return {
        type: "render_world_dirty_sections",
        dirtySections: [sectionOrigin(receivedBatchSizes.length, 0, 0)],
        stats: { loadedChunkCount: receivedBatchSizes.length },
      };
    });

    const sink = new RenderWorldWorkerUpdateSink(new RenderWorldWorkerClient(clientEndpoint));
    await expect(sink.ingestUpdates(snapshots.map((snapshot) => ({ type: "chunk_snapshot", snapshot })))).resolves.toEqual({
      chunkChanged: true,
    });

    expect(receivedBatchSizes).toEqual([8, 1]);
    expect(rawClientEndpoint.getMessages()).toHaveLength(2);
    expect(sink.getStats()).toEqual({ loadedChunkCount: 2 });
    expect(sink.drainDirtySections()).toEqual([
      sectionOrigin(1, 0, 0),
      sectionOrigin(2, 0, 0),
    ]);
  });

  test("builds section meshes and transfers mesh layer buffers back to the client", async () => {
    const { clientEndpoint, hostEndpoint, rawHostEndpoint } = createEndpointPair();
    const origin = sectionOrigin(0, 16, 0);
    const resultBuffer = new Uint8Array([9, 8, 7]);
    const result = createMeshResult(resultBuffer);

    connectRenderWorldWorkerSession(hostEndpoint, async (message): Promise<RenderWorldResponse> => {
      expect(message).toEqual({
        type: "build_render_section_mesh",
        origin,
        camera: { x: 8, y: 80, z: 8 },
      });
      return {
        type: "render_section_mesh_built",
        origin,
        result,
      };
    });

    const client = new RenderWorldWorkerClient(clientEndpoint);
    await expect(client.buildSectionMesh({
      type: "build_render_section_mesh",
      origin,
      camera: { x: 8, y: 80, z: 8 },
    })).resolves.toEqual({
      type: "render_section_mesh_built",
      origin,
      result,
    });

    expect(rawHostEndpoint.getTransfers()).toHaveLength(1);
    expect(rawHostEndpoint.getTransfers()[0]).toEqual([resultBuffer.buffer]);
  });

  test("returns mesh-not-ready responses without treating them as worker failures", async () => {
    const { clientEndpoint, hostEndpoint } = createEndpointPair();
    const origin = sectionOrigin(16, 0, 16);

    connectRenderWorldWorkerSession(hostEndpoint, async () => ({
      type: "render_world_mesh_not_ready",
      origin,
      reason: "missing_neighbors",
      missingChunks: [{ chunkX: 2, chunkZ: 1 }],
    }));

    const client = new RenderWorldWorkerClient(clientEndpoint);
    await expect(client.buildSectionMesh({
      type: "build_render_section_mesh",
      origin,
      camera: { x: 8, y: 64, z: 8 },
    })).resolves.toEqual({
      type: "render_world_mesh_not_ready",
      origin,
      reason: "missing_neighbors",
      missingChunks: [{ chunkX: 2, chunkZ: 1 }],
    });
  });

  test("rejects worker errors and endpoint errors", async () => {
    const session = createEndpointPair();
    connectRenderWorldWorkerSession(session.hostEndpoint, async () => {
      throw new Error("render world failed");
    });

    const client = new RenderWorldWorkerClient(session.clientEndpoint);
    await expect(client.getStats()).rejects.toThrow("render world failed");

    const endpointFailure = createEndpointPair();
    const pendingClient = new RenderWorldWorkerClient(endpointFailure.clientEndpoint);
    const pending = pendingClient.getStats();
    endpointFailure.rawClientEndpoint.emitError(new Error("endpoint failed"));

    await expect(pending).rejects.toThrow("endpoint failed");
  });
});
