import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { ResourceLocation } from "../../src/core/resource-location";
import { createGeneratedWorldSaveId, GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import type { ChunkSnapshotMessage, WorldHostMessage, WorldOpenedMessage } from "../../src/runtime/protocol/world-messages";
import { MemoryWorldStorage } from "../../src/runtime/storage/memory-world-storage";
import { LocalWorldClient, LocalWorldTransport } from "../../src/runtime/transport/local-world-transport";
import { createBlockStateResolver, hydrateChunkFromSnapshot } from "../../src/world/level/chunk-snapshot";
import { clonePackedChunkSnapshot, type PackedChunkSnapshot, unpackChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";
import { GeneratedChunkStatus } from "../../src/world/level/generated-chunk-status";
import {
  GENERATED_PROTO_CHUNK_CONTENT_VERSION,
  cloneGeneratedChunkStorageRecord,
  type GeneratedChunkStorageRecord,
} from "../../src/world/level/generated-proto-chunk";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import type { WorldGenLevel } from "../../src/world/level/world-gen-level";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { createWorldSaveMetadata, type ChunkStorage, type OpenWorldStorageRequest, type WorldSaveMetadata, type WorldStorage, type WorldStorageSession } from "../../src/runtime/storage/world-storage";

const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
} as const;
const STORAGE_CHUNK_SNAPSHOT: PackedChunkSnapshot = {
  chunkX: 7,
  chunkZ: -2,
  biomes: [0],
  sections: [],
  blockTicks: [],
  liquidTicks: [],
};

function createWorldClient(storage: MemoryWorldStorage, mutateWorld?: (level: WorldGenLevel) => void): LocalWorldClient {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);

  return new LocalWorldClient(
    new LocalWorldTransport(
      new GeneratedWorldHost({
        seed: 12345n,
        airState: blocks.airState,
        blockStateById: blocks.blockStateById,
        blockStateIds: blocks.blockStateIds,
        worldStorage: storage,
        lightingMode: "none",
        mutateWorld,
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

function createWorldHost(storage: MemoryWorldStorage, mutateWorld?: (level: WorldGenLevel) => void): GeneratedWorldHost {
  const blocks = registerGeneratedRenderBlocks();
  return new GeneratedWorldHost({
    seed: 12345n,
    airState: blocks.airState,
    blockStateById: blocks.blockStateById,
    blockStateIds: blocks.blockStateIds,
    worldStorage: storage,
    lightingMode: "none",
    mutateWorld,
  });
}

function worldOpenedMessage(messages: readonly WorldHostMessage[]): WorldOpenedMessage {
  const message = messages.find((candidate): candidate is WorldOpenedMessage => candidate.type === "world_opened");
  if (message === undefined) {
    throw new Error("Expected world_opened message");
  }

  return message;
}

function chunkSnapshots(messages: readonly WorldHostMessage[]): ChunkSnapshotMessage[] {
  return messages.filter((message): message is ChunkSnapshotMessage => message.type === "chunk_snapshot");
}

function sleep(ms = 0): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

class BlockingChunkStorage implements ChunkStorage {
  public readonly records = new Map<string, PackedChunkSnapshot>();
  public readonly generatedRecords = new Map<string, GeneratedChunkStorageRecord>();
  public saveStarted = 0;
  private readonly pendingSaveResolvers: Array<() => void> = [];
  private savesReleased = false;

  public async loadChunk(_chunkX: number, _chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    return undefined;
  }

  public async saveChunk(snapshot: PackedChunkSnapshot): Promise<void> {
    this.saveStarted++;
    if (this.savesReleased) {
      this.records.set(`${snapshot.chunkX.toString()},${snapshot.chunkZ.toString()}`, clonePackedChunkSnapshot(snapshot));
      return;
    }

    await new Promise<void>((resolve) => {
      this.pendingSaveResolvers.push(() => {
        this.records.set(`${snapshot.chunkX.toString()},${snapshot.chunkZ.toString()}`, clonePackedChunkSnapshot(snapshot));
        resolve();
      });
    });
  }

  public async loadGeneratedChunk(_chunkX: number, _chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined> {
    return undefined;
  }

  public async saveGeneratedChunk(record: GeneratedChunkStorageRecord): Promise<void> {
    this.generatedRecords.set(`${record.chunkX.toString()},${record.chunkZ.toString()}`, cloneGeneratedChunkStorageRecord(record));
  }

  public async evictChunk(_chunkX: number, _chunkZ: number): Promise<void> {}

  public releaseOneSave(): void {
    this.pendingSaveResolvers.shift()?.();
  }

  public releaseAllSaves(): void {
    this.savesReleased = true;
    while (this.pendingSaveResolvers.length > 0) {
      this.releaseOneSave();
    }
  }
}

class BlockingWorldStorageSession implements WorldStorageSession {
  public constructor(
    public readonly metadata: WorldSaveMetadata,
    public readonly chunks: BlockingChunkStorage,
  ) {}

  public async close(): Promise<void> {}
}

class BlockingWorldStorage implements WorldStorage {
  public readonly chunks = new BlockingChunkStorage();
  public metadata: WorldSaveMetadata | undefined;

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    this.metadata = createWorldSaveMetadata(request);
    return new BlockingWorldStorageSession(this.metadata, this.chunks);
  }
}

class FirstSaveBlockingChunkStorage implements ChunkStorage {
  public readonly records = new Map<string, PackedChunkSnapshot>();
  public readonly generatedRecords = new Map<string, GeneratedChunkStorageRecord>();
  public saveStarted = 0;
  public loadChunkCalls = 0;
  public loadGeneratedChunkCalls = 0;
  public firstBlockedSnapshot: PackedChunkSnapshot | undefined;
  private firstSaveResolver: (() => void) | undefined;
  private firstSaveBlocked = false;

  public async loadChunk(_chunkX: number, _chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    this.loadChunkCalls++;
    return undefined;
  }

  public async saveChunk(snapshot: PackedChunkSnapshot): Promise<void> {
    this.saveStarted++;
    if (!this.firstSaveBlocked) {
      this.firstSaveBlocked = true;
      this.firstBlockedSnapshot = clonePackedChunkSnapshot(snapshot);
      await new Promise<void>((resolve) => {
        this.firstSaveResolver = () => {
          this.records.set(`${snapshot.chunkX.toString()},${snapshot.chunkZ.toString()}`, clonePackedChunkSnapshot(snapshot));
          resolve();
        };
      });
      return;
    }

    this.records.set(`${snapshot.chunkX.toString()},${snapshot.chunkZ.toString()}`, clonePackedChunkSnapshot(snapshot));
  }

  public async loadGeneratedChunk(_chunkX: number, _chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined> {
    this.loadGeneratedChunkCalls++;
    return undefined;
  }

  public async saveGeneratedChunk(record: GeneratedChunkStorageRecord): Promise<void> {
    this.generatedRecords.set(`${record.chunkX.toString()},${record.chunkZ.toString()}`, cloneGeneratedChunkStorageRecord(record));
  }

  public async evictChunk(_chunkX: number, _chunkZ: number): Promise<void> {}

  public releaseFirstSave(): void {
    this.firstSaveResolver?.();
    this.firstSaveResolver = undefined;
  }
}

class FirstSaveBlockingWorldStorageSession implements WorldStorageSession {
  public constructor(
    public readonly metadata: WorldSaveMetadata,
    public readonly chunks: FirstSaveBlockingChunkStorage,
  ) {}

  public async close(): Promise<void> {}
}

class FirstSaveBlockingWorldStorage implements WorldStorage {
  public readonly chunks = new FirstSaveBlockingChunkStorage();
  public metadata: WorldSaveMetadata | undefined;

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    this.metadata = createWorldSaveMetadata(request);
    return new FirstSaveBlockingWorldStorageSession(this.metadata, this.chunks);
  }
}

interface PendingWriteReadThroughDebugHost {
  readonly chunkViewJobRevision: number;
  readonly level: {
    removeChunk(chunkX: number, chunkZ: number): unknown;
  };
  preloadStoredChunk(
    chunkX: number,
    chunkZ: number,
    chunkViewJobRevision: number,
  ): Promise<"loaded" | "already_loaded" | "missing" | "stale">;
}

async function waitForCondition(
  predicate: () => boolean,
  message: string,
  timeoutMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) {
      return;
    }

    await sleep(1);
  }

  throw new Error(message);
}

describe("GeneratedWorld persistence", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("reopens the same saved chunks through shared world storage", async () => {
    const storage = new MemoryWorldStorage(() => 1_000);
    const firstClient = createWorldClient(storage, (level) => {
      const pumpkin = Registry.BLOCK.get(new ResourceLocation("minecraft:pumpkin"));
      if (pumpkin === undefined || !("defaultBlockState" in pumpkin)) {
        throw new Error("minecraft:pumpkin was not registered for persistence test");
      }

      level.setBlock(new BlockPos(0, 90, 0), (pumpkin as { defaultBlockState(): ReturnType<typeof level.getBlockState> }).defaultBlockState());
    });

    const opened = await firstClient.openWorld(OPEN_WORLD_REQUEST);
    expect(opened.saveMetadata.saveId).toBe(createGeneratedWorldSaveId(12345n, "default"));
    await firstClient.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const savedWorld = storage.getWorldMetadata(opened.saveMetadata.saveId);
    expect(savedWorld?.saveId).toBe(opened.saveMetadata.saveId);
    expect(storage.getChunkRecord(opened.saveMetadata.saveId, 0, 0)?.snapshot).toBeDefined();

    const secondClient = createWorldClient(storage);
    await secondClient.openWorld(OPEN_WORLD_REQUEST);
    await secondClient.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const restored = secondClient.getLevel().getBlockState(new BlockPos(0, 90, 0));
    expect(Registry.BLOCK.getKey(restored.getBlock() as unknown as object)?.toString()).toBe("minecraft:pumpkin");
  });

  test("records chunk eviction hooks when the authoritative view slides away", async () => {
    const storage = new MemoryWorldStorage(() => 2_000);
    const host = createWorldHost(storage);

    const opened = worldOpenedMessage(await host.openWorld(OPEN_WORLD_REQUEST));
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 32,
      centerChunkZ: 0,
      radius: 1,
    });
    await host.flushStorageSideEffects();

    expect(storage.getChunkRecord(opened.saveMetadata.saveId, -2, 0)?.lastEvictedAtMs).toBe(2_000);
  });

  test("queues generated-clean cache writes without blocking publication", async () => {
    const storage = new BlockingWorldStorage();
    const blocks = registerGeneratedRenderBlocks();
    const host = new GeneratedWorldHost({
      seed: 12345n,
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      worldStorage: storage,
    });
    await host.openWorld({
      type: "open_world",
      seed: 12345n,
      preset: "flat_grass",
    });

    const viewPromise = host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    const result = await Promise.race([
      viewPromise.then((messages) => ({ type: "returned" as const, messages })),
      sleep(100).then(() => ({ type: "blocked" as const, messages: [] as readonly WorldHostMessage[] })),
    ]);
    if (result.type === "blocked") {
      storage.chunks.releaseAllSaves();
      await viewPromise;
    }

    expect(result.type).toBe("returned");
    expect(chunkSnapshots(result.messages)).toHaveLength(25);
    expect(storage.chunks.records.size).toBe(0);
    expect(storage.chunks.saveStarted).toBeGreaterThan(0);

    storage.chunks.releaseAllSaves();
    await host.flushStorageSideEffects();
    expect(storage.chunks.records.size).toBeGreaterThan(0);
  });

  test("runs unrelated generated-cache writes while one chunk save is blocked", async () => {
    const storage = new FirstSaveBlockingWorldStorage();
    const blocks = registerGeneratedRenderBlocks();
    const host = new GeneratedWorldHost({
      seed: 12345n,
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      worldStorage: storage,
    });
    await host.openWorld({
      type: "open_world",
      seed: 12345n,
      preset: "flat_grass",
    });

    const messages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    expect(chunkSnapshots(messages)).toHaveLength(25);

    await waitForCondition(
      () => storage.chunks.saveStarted > 1 && storage.chunks.records.size > 0,
      "Expected an unrelated generated-cache save to complete while the first save is blocked",
    );
    expect(storage.chunks.saveStarted).toBeGreaterThan(1);
    expect(storage.chunks.records.size).toBeGreaterThan(0);

    storage.chunks.releaseFirstSave();
    await host.flushStorageSideEffects();
    expect(storage.chunks.records.size).toBe(25);
  });

  test("preloads from pending generated-cache writes before queued saves reach storage", async () => {
    const storage = new FirstSaveBlockingWorldStorage();
    const blocks = registerGeneratedRenderBlocks();
    const host = new GeneratedWorldHost({
      seed: 12345n,
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      worldStorage: storage,
    });
    await host.openWorld({
      type: "open_world",
      seed: 12345n,
      preset: "flat_grass",
    });

    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });
    await waitForCondition(
      () => storage.chunks.firstBlockedSnapshot !== undefined,
      "Expected first generated-cache save to block",
    );

    const pendingSnapshot = storage.chunks.firstBlockedSnapshot!;
    storage.chunks.loadChunkCalls = 0;
    storage.chunks.loadGeneratedChunkCalls = 0;
    const debugHost = host as unknown as PendingWriteReadThroughDebugHost;
    debugHost.level.removeChunk(pendingSnapshot.chunkX, pendingSnapshot.chunkZ);

    await expect(debugHost.preloadStoredChunk(
      pendingSnapshot.chunkX,
      pendingSnapshot.chunkZ,
      debugHost.chunkViewJobRevision,
    )).resolves.toBe("loaded");
    expect(storage.chunks.loadGeneratedChunkCalls).toBe(0);
    expect(storage.chunks.loadChunkCalls).toBe(0);

    storage.chunks.releaseFirstSave();
    await host.flushStorageSideEffects();
  });

  test("saves dirty authoritative chunks before eviction", async () => {
    let now = 4_000;
    const storage = new MemoryWorldStorage(() => now);
    const blocks = registerGeneratedRenderBlocks();
    const host = new GeneratedWorldHost({
      seed: 12345n,
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      worldStorage: storage,
      worldTickIntervalMs: 10_000,
      nowMs: () => now,
    });
    const opened = worldOpenedMessage(await host.openWorld(OPEN_WORLD_REQUEST));
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });

    const pumpkin = Registry.BLOCK.get(new ResourceLocation("minecraft:pumpkin"));
    if (pumpkin === undefined || !("defaultBlockState" in pumpkin)) {
      throw new Error("minecraft:pumpkin was not registered for persistence test");
    }

    const changedPos = new BlockPos(0, 90, 0);
    const mutationLevel = (host as unknown as { readonly liquidLevel: WorldGenLevel }).liquidLevel;
    expect(mutationLevel.setBlock(
      changedPos,
      (pumpkin as { defaultBlockState(): ReturnType<WorldGenLevel["getBlockState"]> }).defaultBlockState(),
    )).toBe(true);

    now = 4_001;
    await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 32,
      centerChunkZ: 0,
      radius: 1,
    });
    await host.flushStorageSideEffects();

    const savedRecord = storage.getChunkRecord(opened.saveMetadata.saveId, 0, 0);
    expect(savedRecord?.lastEvictedAtMs).toBe(4_001);
    const restoredChunk = hydrateChunkFromSnapshot(
      unpackChunkSnapshot(savedRecord!.snapshot, blocks.blockStateIds),
      blocks.airState,
      createBlockStateResolver(blocks.airState),
    );
    const restored = restoredChunk.getBlockState(changedPos);
    expect(Registry.BLOCK.getKey(restored.getBlock() as unknown as object)?.toString()).toBe("minecraft:pumpkin");
  });

  test("persists durable chunk facts without derived light", async () => {
    const storage = new MemoryWorldStorage(() => 3_000);
    const host = createWorldHost(storage);

    const opened = worldOpenedMessage(await host.openWorld(OPEN_WORLD_REQUEST));
    const messages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await host.flushStorageSideEffects();

    const publishedCenter = chunkSnapshots(messages)
      .find((message) => message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0);
    expect(publishedCenter?.snapshot.light).toBeUndefined();
    expect(storage.getChunkRecord(opened.saveMetadata.saveId, 0, 0)?.snapshot.light).toBeUndefined();
  });

  test("rejects stale generated chunk record writes", async () => {
    const storage = new MemoryWorldStorage(() => 5_000);
    const session = await storage.openWorld({
      saveId: "generated-cas-test",
      storageVersion: 1,
      seed: "12345",
      preset: "flat_grass",
      minBuildHeight: 0,
      height: 256,
      openedAtMs: 5_000,
    });
    const staleRecord: GeneratedChunkStorageRecord = {
      chunkX: 4,
      chunkZ: -3,
      type: "proto",
      status: GeneratedChunkStatus.STRUCTURE_STARTS,
      hasBlockSections: false,
      isUnsaved: false,
      contentVersion: GENERATED_PROTO_CHUNK_CONTENT_VERSION,
      writeVersion: 1,
    };
    const newerRecord: GeneratedChunkStorageRecord = {
      ...staleRecord,
      status: GeneratedChunkStatus.STRUCTURE_REFERENCES,
      writeVersion: 2,
    };

    await session.chunks.saveGeneratedChunk(newerRecord);
    await session.chunks.saveGeneratedChunk(staleRecord);

    const loaded = await session.chunks.loadGeneratedChunk(4, -3);
    expect(loaded?.status).toBe(GeneratedChunkStatus.STRUCTURE_REFERENCES);
    expect(loaded?.writeVersion).toBe(2);
  });

  test("ignores writes from superseded memory storage sessions", async () => {
    const storage = new MemoryWorldStorage(() => 6_000);
    const request: OpenWorldStorageRequest = {
      saveId: "memory-epoch-test",
      storageVersion: 1,
      seed: "12345",
      preset: "flat_grass",
      minBuildHeight: 0,
      height: 256,
      openedAtMs: 6_000,
    };
    const firstSession = await storage.openWorld(request);
    const secondSession = await storage.openWorld({
      ...request,
      openedAtMs: 6_001,
    });

    await firstSession.chunks.saveChunk(STORAGE_CHUNK_SNAPSHOT);
    expect(storage.getChunkRecord(request.saveId, STORAGE_CHUNK_SNAPSHOT.chunkX, STORAGE_CHUNK_SNAPSHOT.chunkZ)).toBeUndefined();

    await secondSession.chunks.saveChunk(STORAGE_CHUNK_SNAPSHOT);
    expect(storage.getChunkRecord(request.saveId, STORAGE_CHUNK_SNAPSHOT.chunkX, STORAGE_CHUNK_SNAPSHOT.chunkZ)?.snapshot).toEqual(STORAGE_CHUNK_SNAPSHOT);

    await secondSession.close();
    await secondSession.chunks.saveChunk({
      ...STORAGE_CHUNK_SNAPSHOT,
      chunkX: STORAGE_CHUNK_SNAPSHOT.chunkX + 1,
    });
    expect(storage.getChunkRecord(request.saveId, STORAGE_CHUNK_SNAPSHOT.chunkX + 1, STORAGE_CHUNK_SNAPSHOT.chunkZ)).toBeUndefined();
  });
});
