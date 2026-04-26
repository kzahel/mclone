import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldHostForRequest } from "../../src/runtime/host/generated-world-host-factory";
import { createBlockStateResolver, hydrateChunkFromSnapshot } from "../../src/world/level/chunk-snapshot";
import { unpackChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";

const OPEN_FLAT_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "flat_grass",
  config: {
    lightingMode: "none",
    liquidSimulationMode: "none",
  },
} as const;

describe("GeneratedWorldHost factory presets", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("flat_grass preset publishes the demo grass plane through the host boundary", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const host = createGeneratedWorldHostForRequest(OPEN_FLAT_WORLD_REQUEST);

    await host.openWorld(OPEN_FLAT_WORLD_REQUEST);
    const messages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 0,
    });

    const centerSnapshot = messages.find(
      (message) => message.type === "chunk_snapshot" && message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0,
    );

    expect(centerSnapshot?.type).toBe("chunk_snapshot");

    const chunk = hydrateChunkFromSnapshot(
      unpackChunkSnapshot(centerSnapshot!.snapshot, blocks.blockStateIds),
      blocks.airState,
      createBlockStateResolver(blocks.airState),
    );

    expect(chunk.getBlockState(new BlockPos(8, 63, 8)).isAir()).toBe(false);
    expect(Registry.BLOCK.getKey(chunk.getBlockState(new BlockPos(8, 63, 8)).getBlock() as unknown as object)?.toString()).toBe("minecraft:grass_block");
    expect(Registry.BLOCK.getKey(chunk.getBlockState(new BlockPos(8, 64, 8)).getBlock() as unknown as object)?.toString()).toBe("minecraft:air");
  });
});
