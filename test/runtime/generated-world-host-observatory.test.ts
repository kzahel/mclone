import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldHostForRequest } from "../../src/runtime/host/generated-world-host-factory";
import type { GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import type {
  ChunkStorage,
  OpenWorldStorageRequest,
  WorldSaveMetadata,
  WorldStorage,
  WorldStorageSession,
} from "../../src/runtime/storage/world-storage";
import { createWorldSaveMetadata } from "../../src/runtime/storage/world-storage";
import type { PackedChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";
import type { GeneratedChunkStorageRecord } from "../../src/world/level/generated-proto-chunk";
import { GeneratedChunkStatus } from "../../src/world/level/generated-chunk-status";
import { FullChunkStatus } from "../../src/world/level/entity/full-chunk-status";
import type { WorldHostMessage } from "../../src/runtime/protocol/world-messages";
import {
  createChunkLifecycleRouteDelta,
  parseChunkRoute,
} from "../../scripts/chunk-lifecycle-observatory";

const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "flat_grass",
  storageMode: "none",
  config: {
    lightingMode: "none",
    liquidSimulationMode: "none",
  },
} as const;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function throwOnWorldError(messages: readonly WorldHostMessage[]): void {
  const error = messages.find((message) => message.type === "world_error");
  if (error !== undefined) {
    throw new Error(error.message);
  }
}

function createHost(options: { readonly worldStorage?: WorldStorage } = {}): GeneratedWorldHost {
  return createGeneratedWorldHostForRequest(OPEN_WORLD_REQUEST, {
    chunkViewScheduling: "cooperative",
    lightingMode: "none",
    liquidSimulationMode: "none",
    worldStorage: options.worldStorage,
  });
}

function getPublishRecords(host: GeneratedWorldHost) {
  return host.getDebugChunkLifecycleSnapshot().records.filter((record) => record.inPublishView);
}

function expectPublishViewConverged(host: GeneratedWorldHost, expectedCount: number): void {
  const snapshot = host.getDebugChunkLifecycleSnapshot();
  const publishRecords = snapshot.records.filter((record) => record.inPublishView);
  expect(publishRecords).toHaveLength(expectedCount);
  expect(publishRecords.every((record) => record.published)).toBe(true);
  expect(snapshot.counts.byPublicationBlocker.ready_to_publish ?? 0).toBe(0);
}

async function drainPublishedView(host: GeneratedWorldHost, expectedCount: number, timeoutMs = 60_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const publishRecords = getPublishRecords(host);
    if (publishRecords.length === expectedCount && publishRecords.every((record) => record.published)) {
      return;
    }

    await sleep(10);
    throwOnWorldError(await host.pollUpdates({ type: "poll_world_updates", maxMessages: 512 }));
  }

  const snapshot = host.getDebugChunkLifecycleSnapshot();
  const publishRecords = snapshot.records.filter((record) => record.inPublishView);
  const published = publishRecords.filter((record) => record.published).length;
  const blockers = JSON.stringify(snapshot.counts.byPublicationBlocker);
  throw new Error(`Expected ${expectedCount.toString()} published chunks, got ${published.toString()}; blockers=${blockers}`);
}

function clearPublishedChunkSnapshotsForTest(host: GeneratedWorldHost): void {
  (host as unknown as { readonly publishedChunkSnapshots: Set<string> }).publishedChunkSnapshots.clear();
}

class BlockingPreloadChunkStorage implements ChunkStorage {
  public generatedLoadStarted = 0;
  private firstGeneratedLoadResolver: (() => void) | undefined;
  private firstGeneratedLoadBlocked = false;

  public async loadChunk(_chunkX: number, _chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    return undefined;
  }

  public async saveChunk(_snapshot: PackedChunkSnapshot): Promise<void> {}

  public async loadGeneratedChunk(_chunkX: number, _chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined> {
    this.generatedLoadStarted++;
    if (!this.firstGeneratedLoadBlocked) {
      this.firstGeneratedLoadBlocked = true;
      await new Promise<void>((resolve) => {
        this.firstGeneratedLoadResolver = resolve;
      });
    }

    return undefined;
  }

  public async saveGeneratedChunk(_record: GeneratedChunkStorageRecord): Promise<void> {}

  public async evictChunk(_chunkX: number, _chunkZ: number): Promise<void> {}

  public releaseFirstGeneratedLoad(): void {
    this.firstGeneratedLoadResolver?.();
    this.firstGeneratedLoadResolver = undefined;
  }
}

class BlockingPreloadWorldStorageSession implements WorldStorageSession {
  public constructor(
    public readonly metadata: WorldSaveMetadata,
    public readonly chunks: BlockingPreloadChunkStorage,
  ) {}

  public async close(): Promise<void> {}
}

class BlockingPreloadWorldStorage implements WorldStorage {
  public readonly chunks = new BlockingPreloadChunkStorage();

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    return new BlockingPreloadWorldStorageSession(createWorldSaveMetadata(request), this.chunks);
  }
}

async function waitForGeneratedPreloadStart(storage: BlockingPreloadChunkStorage, timeoutMs = 10_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (storage.generatedLoadStarted > 0) {
      return;
    }

    await sleep(1);
  }

  throw new Error("Expected a generated chunk preload to start");
}

describe("GeneratedWorldHost chunk lifecycle observatory", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("reports settled generated status, publication, ticket, and holder counts", async () => {
    const host = createHost();
    await host.openWorld(OPEN_WORLD_REQUEST);
    throwOnWorldError(await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    }));
    await drainPublishedView(host, 25);

    const snapshot = host.getDebugChunkLifecycleSnapshot();
    expect(snapshot.currentChunkView).toEqual({ centerChunkX: 0, centerChunkZ: 0, radius: 0 });
    expect(snapshot.counts.inPublishView).toBe(25);
    expect(snapshot.counts.inAuthorityView).toBe(625);
    expect(snapshot.counts.published).toBe(25);
    expect(snapshot.counts.materialized).toBe(121);
    expect(snapshot.counts.byPublicationBlocker.already_published).toBe(25);
    expect(snapshot.counts.byGeneratedStatus[GeneratedChunkStatus.FULL]).toBe(49);
    expect(snapshot.counts.byGeneratedStatus[GeneratedChunkStatus.FEATURES]).toBe(32);
    expect(snapshot.counts.byGeneratedStatus[GeneratedChunkStatus.LIQUID_CARVERS]).toBe(40);

    const center = snapshot.records.find((record) => record.chunkX === 0 && record.chunkZ === 0);
    expect(center).toMatchObject({
      inPublishView: true,
      inAuthorityView: true,
      generatedStatus: GeneratedChunkStatus.FULL,
      holderFullStatus: FullChunkStatus.ENTITY_TICKING,
      published: true,
      publicationBlocker: { kind: "already_published" },
    });
    expect(center?.ticketSources.map((ticket) => ticket.source).sort()).toEqual([
      "entity",
      "generation_dependency",
      "player_view",
    ]);

    const metadataOnly = snapshot.records.find((record) =>
      record.ticketSources.some((ticket) => ticket.source === "generation_dependency")
      && !record.inPublishView
      && !record.hasBlockSections
    );
    expect(metadataOnly).toBeDefined();
    expect(metadataOnly?.publicationBlocker.kind).toBe("outside_publish_view");
  });

  test("computes coordinate-bearing route deltas for a one-chunk walk", async () => {
    expect(parseChunkRoute("0,0 -> 1,0")).toEqual([[0, 0], [1, 0]]);

    const host = createHost();
    await host.openWorld(OPEN_WORLD_REQUEST);
    throwOnWorldError(await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    }));
    await drainPublishedView(host, 25);
    const initial = host.getDebugChunkLifecycleSnapshot();

    throwOnWorldError(await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 1,
      centerChunkZ: 0,
      radius: 0,
    }));
    await drainPublishedView(host, 25);
    const moved = host.getDebugChunkLifecycleSnapshot();
    const delta = createChunkLifecycleRouteDelta(initial, moved, [1, 0]);

    expect(delta.newlyPublished).toEqual([[3, -2], [3, -1], [3, 0], [3, 1], [3, 2]]);
    expect(delta.unpublished).toEqual([[-2, -2], [-2, -1], [-2, 0], [-2, 1], [-2, 2]]);
    expect(delta.newlyFull).toEqual([[4, -3], [4, -2], [4, -1], [4, 0], [4, 1], [4, 2], [4, 3]]);
    expect(delta.newlyFeatures).toEqual([[5, -4], [5, -3], [5, -2], [5, -1], [5, 0], [5, 1], [5, 2], [5, 3], [5, 4]]);
    expect(delta.newlyMaterialized).toEqual([
      [6, -5],
      [6, -4],
      [6, -3],
      [6, -2],
      [6, -1],
      [6, 0],
      [6, 1],
      [6, 2],
      [6, 3],
      [6, 4],
      [6, 5],
    ]);
    expect(delta.stillBlocked).toEqual([]);
    expect(delta.asciiMap).toContain("legend:");
    expect(delta.asciiMap).toContain("C");
    expect(delta.asciiMap).toContain("N");
    expect(delta.asciiMap).toContain("U");
  });

  test("publishes chunks again after returning to an already generated view", async () => {
    const host = createHost();
    await host.openWorld(OPEN_WORLD_REQUEST);
    throwOnWorldError(await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 3,
    }));
    await drainPublishedView(host, 81);

    throwOnWorldError(await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 8,
      centerChunkZ: 0,
      radius: 3,
    }));
    await drainPublishedView(host, 81);

    throwOnWorldError(await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 3,
    }));
    await drainPublishedView(host, 81);

    expectPublishViewConverged(host, 81);
  });

  test("polling recovers idle publication debt without a chunk-view change", async () => {
    const host = createHost();
    await host.openWorld(OPEN_WORLD_REQUEST);
    throwOnWorldError(await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 3,
    }));
    await drainPublishedView(host, 81);

    clearPublishedChunkSnapshotsForTest(host);
    const ready = getPublishRecords(host).filter((record) => record.publicationBlocker.kind === "ready_to_publish");
    expect(ready).toHaveLength(81);

    throwOnWorldError(await host.pollUpdates({ type: "poll_world_updates", maxMessages: 512 }));
    expectPublishViewConverged(host, 81);
  });

  test("explains unpublished visible chunks while preload is blocked", async () => {
    const storage = new BlockingPreloadWorldStorage();
    const host = createHost({ worldStorage: storage });
    await host.openWorld(OPEN_WORLD_REQUEST);
    throwOnWorldError(await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    }));
    await waitForGeneratedPreloadStart(storage.chunks);

    const blocked = host.getDebugChunkLifecycleSnapshot();
    const publishRecords = blocked.records.filter((record) => record.inPublishView);
    expect(publishRecords).toHaveLength(25);
    expect(publishRecords.every((record) => !record.published)).toBe(true);
    expect(publishRecords.every((record) => record.publicationBlocker.kind !== "outside_publish_view")).toBe(true);
    expect(publishRecords.some((record) => record.publicationBlocker.kind === "missing_materialized_chunk")).toBe(true);
    expect(blocked.records.some((record) => record.preload?.state === "pending")).toBe(true);

    storage.chunks.releaseFirstGeneratedLoad();
    await drainPublishedView(host, 25);
  }, 60_000);
});
