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
});
