import { afterEach, describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import {
  createGeneratedWorldSaveId,
  GENERATED_WORLD_STORAGE_VERSION,
  GeneratedWorldHost,
} from "../../src/runtime/host/generated-world-host";
import type { ChunkSnapshotMessage, WorldHostMessage } from "../../src/runtime/protocol/world-messages";
import { MemoryWorldStorage } from "../../src/runtime/storage/memory-world-storage";
import {
  buildChunkSnapshot,
  createBlockStateResolver,
  hydrateChunkFromSnapshot,
} from "../../src/world/level/chunk-snapshot";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import { packChunkSnapshot, unpackChunkSnapshot, type PackedChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";
import { LiquidBlock } from "../../src/world/level/block/liquid-block";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { ChunkBlockId } from "../../src/worldgen/chunk/chunk-block-buffer";

const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
} as const;

function chunkSnapshots(messages: readonly WorldHostMessage[]): ChunkSnapshotMessage[] {
  return messages.filter((message): message is ChunkSnapshotMessage => message.type === "chunk_snapshot");
}

function saveId(): string {
  return createGeneratedWorldSaveId(OPEN_WORLD_REQUEST.seed, OPEN_WORLD_REQUEST.preset);
}

function blockName(state: ReturnType<LevelChunk["getBlockState"]>): string | undefined {
  return Registry.BLOCK.getKey(state.getBlock() as unknown as object)?.toString();
}

describe("GeneratedWorldHost liquid simulation", () => {
  afterEach(() => {
    Registry.BLOCK.clear();
  });

  test("hydrates pending liquid ticks into the host queue and republishes dirty chunk snapshots", async () => {
    const blocks = registerGeneratedRenderBlocks();
    const resolveState = createBlockStateResolver(blocks.airState);
    const storage = new MemoryWorldStorage(() => 1_000);
    let nowMs = 0;
    const session = await storage.openWorld({
      saveId: saveId(),
      storageVersion: GENERATED_WORLD_STORAGE_VERSION,
      seed: OPEN_WORLD_REQUEST.seed.toString(),
      preset: OPEN_WORLD_REQUEST.preset,
      minBuildHeight: 0,
      height: 256,
      openedAtMs: 1_000,
    });

    const sourcePos = new BlockPos(0, 200, 0);
    const chunk = new LevelChunk(0, 0, blocks.airState);
    chunk.setBlockState(sourcePos, blocks.blockStateById[ChunkBlockId.WATER]!);
    chunk.recordLiquidTick(sourcePos, "minecraft:water", 0);
    await session.chunks.saveChunk(packChunkSnapshot(
      buildChunkSnapshot(chunk, [0], 0, 256),
      blocks.blockStateIds,
      resolveState,
    ));

    const host = new GeneratedWorldHost({
      seed: OPEN_WORLD_REQUEST.seed,
      airState: blocks.airState,
      blockStateById: blocks.blockStateById,
      blockStateIds: blocks.blockStateIds,
      lightingMode: "none",
      worldStorage: storage,
      worldTickIntervalMs: 1,
      nowMs: () => nowMs,
    });

    await host.openWorld(OPEN_WORLD_REQUEST);
    const initialMessages = await host.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    const initialCenter = chunkSnapshots(initialMessages).find((message) => message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0);
    expect(initialCenter?.snapshot.liquidTicks).toContainEqual({
      x: sourcePos.getX(),
      y: sourcePos.getY(),
      z: sourcePos.getZ(),
      target: "minecraft:water",
      delay: 0,
    });

    nowMs = 6;
    const updates = await host.pollUpdates({ type: "poll_world_updates" });
    const centerUpdate = chunkSnapshots(updates).find((message) => message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0);
    expect(centerUpdate).toBeDefined();

    const updatedChunk = hydrateChunkFromSnapshot(
      unpackChunkSnapshot(centerUpdate!.snapshot, blocks.blockStateIds),
      blocks.airState,
      resolveState,
    );
    const fallingWater = updatedChunk.getBlockState(sourcePos.below());
    expect(blockName(fallingWater)).toBe("minecraft:water");
    expect(fallingWater.getValue(LiquidBlock.LEVEL)).toBe(8);

    const saved: PackedChunkSnapshot | undefined = storage.getChunkRecord(saveId(), 0, 0)?.snapshot;
    expect(saved).toBeDefined();
    expect(unpackChunkSnapshot(saved!, blocks.blockStateIds).liquidTicks.length).toBeGreaterThan(0);
  });
});
