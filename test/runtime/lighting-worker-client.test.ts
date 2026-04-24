import { describe, expect, test } from "vitest";
import {
  LightingWorkerClient,
  connectLightingWorkerSession,
  type LightingWorkerClientEndpoint,
  type LightingWorkerHostEndpoint,
  type LightingWorkerMessageListener,
  type LightingWorkerRequestEnvelope,
  type LightingWorkerResponseEnvelope,
} from "../../src/runtime/lighting/lighting-worker-client";
import type { LightingResult } from "../../src/runtime/lighting/lighting-protocol";
import { createLightingWorkerHandler } from "../../src/runtime/lighting/lighting-worker";

class TestMessageEndpoint<TOutgoing, TIncoming> {
  private peer?: TestMessageEndpoint<TIncoming, TOutgoing>;
  private readonly messageListeners = new Set<LightingWorkerMessageListener<TIncoming>>();
  private readonly errorListeners = new Set<(event: unknown) => void>();
  private readonly transfers: Transferable[][] = [];

  public connect(peer: TestMessageEndpoint<TIncoming, TOutgoing>): void {
    this.peer = peer;
  }

  public postMessage(message: TOutgoing, transfer: readonly Transferable[] = []): void {
    this.transfers.push([...transfer]);
    queueMicrotask(() => {
      this.peer?.dispatchMessage(message);
    });
  }

  public addEventListener(type: "message" | "error" | "messageerror", listener: ((event: unknown) => void) | LightingWorkerMessageListener<TIncoming>): void {
    if (type === "message") {
      this.messageListeners.add(listener as LightingWorkerMessageListener<TIncoming>);
      return;
    }

    this.errorListeners.add(listener as (event: unknown) => void);
  }

  public removeEventListener(type: "message" | "error" | "messageerror", listener: ((event: unknown) => void) | LightingWorkerMessageListener<TIncoming>): void {
    if (type === "message") {
      this.messageListeners.delete(listener as LightingWorkerMessageListener<TIncoming>);
      return;
    }

    this.errorListeners.delete(listener as (event: unknown) => void);
  }

  public emitError(error: unknown): void {
    for (const listener of this.errorListeners) {
      listener(error);
    }
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
  readonly clientEndpoint: LightingWorkerClientEndpoint;
  readonly hostEndpoint: LightingWorkerHostEndpoint;
  readonly rawClientEndpoint: TestMessageEndpoint<LightingWorkerRequestEnvelope, LightingWorkerResponseEnvelope>;
} {
  const rawClientEndpoint = new TestMessageEndpoint<LightingWorkerRequestEnvelope, LightingWorkerResponseEnvelope>();
  const rawHostEndpoint = new TestMessageEndpoint<LightingWorkerResponseEnvelope, LightingWorkerRequestEnvelope>();
  rawClientEndpoint.connect(rawHostEndpoint);
  rawHostEndpoint.connect(rawClientEndpoint);
  return {
    clientEndpoint: rawClientEndpoint as unknown as LightingWorkerClientEndpoint,
    hostEndpoint: rawHostEndpoint as unknown as LightingWorkerHostEndpoint,
    rawClientEndpoint,
  };
}

async function collectLightingResultsUntil(
  client: LightingWorkerClient,
  predicate: (result: LightingResult) => boolean,
): Promise<readonly LightingResult[]> {
  const collected: LightingResult[] = [];
  for (let attempt = 0; attempt < 50; attempt++) {
    const batch = await client.pollResults({
      type: "poll_light_results",
      maxResults: 16,
    });
    collected.push(...batch.results);
    if (collected.some(predicate)) {
      return collected;
    }

    await new Promise((resolve) => {
      setTimeout(resolve, 1);
    });
  }

  return collected;
}

describe("Lighting worker client", () => {
  test("uses worker messages for lighting commands and drains results only through polling", async () => {
    const { clientEndpoint, hostEndpoint, rawClientEndpoint } = createEndpointPair();
    connectLightingWorkerSession(hostEndpoint, createLightingWorkerHandler());
    const client = new LightingWorkerClient(clientEndpoint);
    const blockStateIds = new Uint16Array(16 * 16 * 16);

    await client.configureWorld({
      type: "configure_light_world",
      seed: 12345n,
      minBuildHeight: 0,
      height: 256,
      blockRegistryVersion: 1,
    });
    await client.setView({
      type: "set_light_view",
      chunkViewRevision: 1,
      centerChunkX: 0,
      centerChunkZ: 0,
      loadRadius: 2,
      publishRadius: 1,
    });
    await client.upsertChunk({
      type: "upsert_light_chunk",
      chunkViewRevision: 1,
      chunkX: 0,
      chunkZ: 0,
      chunkRevision: 7,
      decorated: true,
      sections: [{ y: 0, blockStateIds }],
    });
    await client.requestInitialLight({
      type: "request_initial_light",
      chunkViewRevision: 1,
      chunkX: 0,
      chunkZ: 0,
      chunkRevision: 7,
      neighbors: [],
    });

    expect(rawClientEndpoint.getTransfers().some((transfer) => transfer.includes(blockStateIds.buffer))).toBe(true);

    const results = await collectLightingResultsUntil(client, (result) => result.type === "chunk_light_ready");
    expect(results).toContainEqual({
      type: "light_progress",
      stage: "configured",
      current: 1,
      total: 1,
    });
    const ready = results.find((result) => result.type === "chunk_light_ready");
    expect(ready).toMatchObject({
      type: "chunk_light_ready",
      chunkViewRevision: 1,
      chunkX: 0,
      chunkZ: 0,
      chunkRevision: 7,
      light: {
        lightCorrect: true,
      },
    });
  });

  test("rejects commands when the bounded lighting mailbox is full", async () => {
    const { clientEndpoint, hostEndpoint } = createEndpointPair();
    connectLightingWorkerSession(hostEndpoint, createLightingWorkerHandler({ maxQueuedCommands: 1 }));
    const client = new LightingWorkerClient(clientEndpoint);

    const first = client.configureWorld({
      type: "configure_light_world",
      seed: 12345n,
      minBuildHeight: 0,
      height: 256,
      blockRegistryVersion: 1,
    });
    const second = client.setView({
      type: "set_light_view",
      chunkViewRevision: 1,
      centerChunkX: 0,
      centerChunkZ: 0,
      loadRadius: 2,
      publishRadius: 1,
    });

    await expect(first).resolves.toBeUndefined();
    await expect(second).rejects.toThrow("lighting mailbox full");
  });

  test("rejects pending client requests when the lighting worker endpoint errors", async () => {
    const { clientEndpoint, rawClientEndpoint } = createEndpointPair();
    const client = new LightingWorkerClient(clientEndpoint);

    const request = client.configureWorld({
      type: "configure_light_world",
      seed: 12345n,
      minBuildHeight: 0,
      height: 256,
      blockRegistryVersion: 1,
    });
    rawClientEndpoint.emitError(new Error("lighting worker failed"));

    await expect(request).rejects.toThrow("lighting worker failed");
  });
});
