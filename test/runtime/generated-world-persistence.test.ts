import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { ResourceLocation } from "../../src/core/resource-location";
import { createGeneratedWorldSaveId, GeneratedWorldHost } from "../../src/runtime/host/generated-world-host";
import { MemoryWorldStorage } from "../../src/runtime/storage/memory-world-storage";
import { LocalWorldClient, LocalWorldTransport } from "../../src/runtime/transport/local-world-transport";
import { createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import type { WorldGenLevel } from "../../src/world/level/world-gen-level";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";

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
        worldStorage: storage,
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
    }),
  );
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
    const client = createWorldClient(storage);

    const opened = await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 2,
      centerChunkZ: 0,
      radius: 1,
    });

    expect(storage.getChunkRecord(opened.saveMetadata.saveId, -2, 0)?.lastEvictedAtMs).toBe(2_000);
  });
});
