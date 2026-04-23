import { describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { buildChunkSnapshot, createBlockStateResolver, hydrateChunkFromSnapshot } from "../../src/world/level/chunk-snapshot";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";

describe("chunk snapshot scheduled ticks", () => {
  test("round-trips scheduled block and liquid ticks through chunk snapshots", () => {
    const { airState } = registerGeneratedRenderBlocks();
    const chunk = new LevelChunk(0, 0, airState);
    chunk.recordBlockTick(new BlockPos(1, 10, 2), "minecraft:magma_block", 0);
    chunk.recordLiquidTick(new BlockPos(3, 62, 4), "minecraft:water", 0);

    const snapshot = buildChunkSnapshot(chunk, [0], 0, 256);
    const hydrated = hydrateChunkFromSnapshot(snapshot, airState, createBlockStateResolver(airState));

    expect(snapshot.blockTicks).toEqual([
      { x: 1, y: 10, z: 2, target: "minecraft:magma_block", delay: 0 },
    ]);
    expect(snapshot.liquidTicks).toEqual([
      { x: 3, y: 62, z: 4, target: "minecraft:water", delay: 0 },
    ]);
    expect(hydrated.getScheduledBlockTicks()).toEqual(snapshot.blockTicks);
    expect(hydrated.getScheduledLiquidTicks()).toEqual(snapshot.liquidTicks);
  });
});
