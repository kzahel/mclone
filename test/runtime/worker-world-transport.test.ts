import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { createGeneratedWorldSaveId, GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import {
  connectWorldWorkerSession,
  WorkerWorldClient,
  WorkerWorldTransport,
  type WorldWorkerClientEndpoint,
  type WorldWorkerHostEndpoint,
  type WorldWorkerMessageListener,
  type WorldWorkerRequestEnvelope,
  type WorldWorkerResponseEnvelope,
} from "../../src/runtime/transport/worker-world-transport";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";

class TestMessageEndpoint<TOutgoing, TIncoming> {
  private peer?: TestMessageEndpoint<TIncoming, TOutgoing>;
  private readonly messageListeners = new Set<WorldWorkerMessageListener<TIncoming>>();
  private readonly errorListeners = new Set<(event: unknown) => void>();

  public connect(peer: TestMessageEndpoint<TIncoming, TOutgoing>): void {
    this.peer = peer;
  }

  public postMessage(message: TOutgoing): void {
    queueMicrotask(() => {
      this.peer?.dispatchMessage(message);
    });
  }

  public addEventListener(type: "message" | "error" | "messageerror", listener: ((event: unknown) => void) | WorldWorkerMessageListener<TIncoming>): void {
    if (type === "message") {
      this.messageListeners.add(listener as WorldWorkerMessageListener<TIncoming>);
      return;
    }

    this.errorListeners.add(listener as (event: unknown) => void);
  }

  public removeEventListener(type: "message" | "error" | "messageerror", listener: ((event: unknown) => void) | WorldWorkerMessageListener<TIncoming>): void {
    if (type === "message") {
      this.messageListeners.delete(listener as WorldWorkerMessageListener<TIncoming>);
      return;
    }

    this.errorListeners.delete(listener as (event: unknown) => void);
  }

  public emitError(error: unknown): void {
    for (const listener of this.errorListeners) {
      listener(error);
    }
  }

  private dispatchMessage(message: TIncoming): void {
    for (const listener of this.messageListeners) {
      listener({ data: message });
    }
  }
}

function createEndpointPair(): {
  readonly clientEndpoint: WorldWorkerClientEndpoint;
  readonly hostEndpoint: WorldWorkerHostEndpoint;
  readonly rawClientEndpoint: TestMessageEndpoint<WorldWorkerRequestEnvelope, WorldWorkerResponseEnvelope>;
} {
  const rawClientEndpoint = new TestMessageEndpoint<WorldWorkerRequestEnvelope, WorldWorkerResponseEnvelope>();
  const rawHostEndpoint = new TestMessageEndpoint<WorldWorkerResponseEnvelope, WorldWorkerRequestEnvelope>();
  rawClientEndpoint.connect(rawHostEndpoint);
  rawHostEndpoint.connect(rawClientEndpoint);
  return {
    clientEndpoint: rawClientEndpoint as unknown as WorldWorkerClientEndpoint,
    hostEndpoint: rawHostEndpoint as unknown as WorldWorkerHostEndpoint,
    rawClientEndpoint,
  };
}

describe("WorkerWorld transport", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("streams generated chunks through a message-based worker session", async () => {
    const generatedBlocks = registerGeneratedRenderBlocks();
    const { clientEndpoint, hostEndpoint } = createEndpointPair();

    connectWorldWorkerSession(
      hostEndpoint,
      (request) => new GeneratedWorldHost({
        seed: request.seed,
        airState: generatedBlocks.airState,
        blockStateById: generatedBlocks.blockStateById,
      }),
    );

    const biomeSource = new OverworldBiomeSource(12345n);
    const client = new WorkerWorldClient(
      new WorkerWorldTransport(clientEndpoint),
      (worldOpened) => new ClientChunkCache({
        airState: generatedBlocks.airState,
        minBuildHeight: worldOpened.minBuildHeight,
        height: worldOpened.height,
        biomeSource,
        biomeZoomSeed: 12345n,
        blockStateResolver: createBlockStateResolver(generatedBlocks.airState),
      }),
    );

    await expect(client.openWorld({
      type: "open_world",
      seed: 12345n,
      preset: "default",
    })).resolves.toEqual({
      type: "world_opened",
      minBuildHeight: 0,
      height: 256,
      saveMetadata: {
        saveId: createGeneratedWorldSaveId(12345n, "default"),
        storageVersion: 2,
        seed: "12345",
        preset: "default",
        minBuildHeight: 0,
        height: 256,
        createdAtMs: expect.any(Number),
        lastOpenedAtMs: expect.any(Number),
      },
    });

    expect(await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).toBe(true);

    const level = client.getLevel();
    expect(level.getLoadedChunkCount()).toBe(25);
    expect(level.getChunk(0, 0, false)).not.toBeNull();
  });

  test("rejects pending client requests when the worker endpoint errors", async () => {
    const generatedBlocks = registerGeneratedRenderBlocks();
    const { clientEndpoint, rawClientEndpoint } = createEndpointPair();
    const biomeSource = new OverworldBiomeSource(12345n);
    const client = new WorkerWorldClient(
      new WorkerWorldTransport(clientEndpoint),
      (worldOpened) => new ClientChunkCache({
        airState: generatedBlocks.airState,
        minBuildHeight: worldOpened.minBuildHeight,
        height: worldOpened.height,
        biomeSource,
        biomeZoomSeed: 12345n,
        blockStateResolver: createBlockStateResolver(generatedBlocks.airState),
      }),
    );

    const openPromise = client.openWorld({
      type: "open_world",
      seed: 12345n,
      preset: "default",
    });
    rawClientEndpoint.emitError(new Error("worker transport failed"));

    await expect(openPromise).rejects.toThrow("worker transport failed");
  });
});
