import { describe, expect, test } from "vitest";
import { BlockPos } from "../../src/core/block-pos";
import { Registry } from "../../src/core/registry";
import { ResourceLocation } from "../../src/core/resource-location";
import type { Block } from "../../src/world/level/block/block";
import type { BlockState } from "../../src/world/level/block/state/block-state";
import { buildChunkSnapshot, createBlockStateResolver, hydrateChunkFromSnapshot } from "../../src/world/level/chunk-snapshot";
import { LevelChunk } from "../../src/world/level/chunk/level-chunk";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";

function getState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

describe("chunk snapshots", () => {
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

  test("preserves registered air-like block states distinct from default air", () => {
    const { airState } = registerGeneratedRenderBlocks();
    const caveAirState = getState("minecraft:cave_air");
    const pos = new BlockPos(1, 5, 2);
    const chunk = new LevelChunk(0, 0, airState);

    chunk.setBlockState(pos, caveAirState);

    const snapshot = buildChunkSnapshot(chunk, [0], 0, 16);
    const hydrated = hydrateChunkFromSnapshot(snapshot, airState, createBlockStateResolver(airState));

    expect(chunk.getBlockState(pos)).toBe(caveAirState);
    expect(chunk.isYSpaceEmpty(5, 5)).toBe(true);
    expect(snapshot.sections).toHaveLength(1);
    expect(snapshot.sections[0]!.palette).toContainEqual({ name: "minecraft:cave_air" });
    expect(hydrated.getBlockState(pos)).toBe(caveAirState);
    expect(hydrated.isYSpaceEmpty(5, 5)).toBe(true);
  });

  test("packs only stored sections in the requested build-height range", () => {
    const { airState } = registerGeneratedRenderBlocks();
    const stoneState = getState("minecraft:stone");
    const pos = new BlockPos(1, 33, 2);
    const chunk = new LevelChunk(0, 0, airState, 0, 64);

    chunk.setBlockState(pos, stoneState);

    const snapshot = buildChunkSnapshot(chunk, [0], 0, 64);

    expect(snapshot.sections.map((section) => section.y)).toEqual([2]);
    expect(snapshot.sections[0]!.palette).toContainEqual({ name: "minecraft:stone" });
  });
});
