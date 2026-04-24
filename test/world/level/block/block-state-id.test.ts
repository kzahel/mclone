import { describe, expect, test } from "vitest";
import { Registry } from "../../../../src/core/registry";
import { Direction } from "../../../../src/core/direction";
import { ResourceLocation } from "../../../../src/core/resource-location";
import { BambooBlock } from "../../../../src/world/level/block/bamboo-block";
import { CocoaBlock } from "../../../../src/world/level/block/cocoa-block";
import { HorizontalDirectionalBlock } from "../../../../src/world/level/block/horizontal-directional-block";
import { LeavesBlock } from "../../../../src/world/level/block/leaves-block";
import { LiquidBlock } from "../../../../src/world/level/block/liquid-block";
import { RotatedPillarBlock } from "../../../../src/world/level/block/rotated-pillar-block";
import { SnowLayerBlock } from "../../../../src/world/level/block/snow-layer-block";
import type { Block } from "../../../../src/world/level/block/block";
import { buildBlockStateIdMap } from "../../../../src/world/level/block/state/block-state-id";
import { registerGeneratedRenderBlocks } from "../../../../src/world/level/generated-render-blocks";
import { Fluids } from "../../../../src/world/level/material/fluids";

function requireBlock(location: string) {
  const block = Registry.BLOCK.get(new ResourceLocation(location));
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }
  return block as Block;
}

function registeredBlocks(): Iterable<Block> {
  return Registry.BLOCK as Iterable<Block>;
}

describe("BlockStateIdMap", () => {
  test("round-trips all registered generated block states", () => {
    registerGeneratedRenderBlocks();
    const idMap = buildBlockStateIdMap(registeredBlocks());
    const states = idMap.getStates();

    expect(idMap.size).toBe(states.length);
    for (const state of states) {
      expect(idMap.stateFor(idMap.idFor(state))).toBe(state);
    }
  });

  test("assigns distinct ids to property variants", () => {
    registerGeneratedRenderBlocks();
    const idMap = buildBlockStateIdMap(registeredBlocks());

    const snow = requireBlock("minecraft:snow") as SnowLayerBlock;
    const water = requireBlock("minecraft:water") as LiquidBlock;
    const oakLog = requireBlock("minecraft:oak_log") as RotatedPillarBlock;
    const oakLeaves = requireBlock("minecraft:oak_leaves") as LeavesBlock;
    const bamboo = requireBlock("minecraft:bamboo") as BambooBlock;
    const cocoa = requireBlock("minecraft:cocoa") as CocoaBlock;

    const variants = [
      snow.defaultBlockState().setValue(SnowLayerBlock.LAYERS, 1),
      snow.defaultBlockState().setValue(SnowLayerBlock.LAYERS, 8),
      water.defaultBlockState().setValue(LiquidBlock.LEVEL, 0),
      water.defaultBlockState().setValue(LiquidBlock.LEVEL, 15),
      oakLog.defaultBlockState().setValue(RotatedPillarBlock.AXIS, Direction.Axis.X),
      oakLog.defaultBlockState().setValue(RotatedPillarBlock.AXIS, Direction.Axis.Y),
      oakLeaves.defaultBlockState().setValue(LeavesBlock.DISTANCE, 1),
      oakLeaves.defaultBlockState().setValue(LeavesBlock.DISTANCE, 7).setValue(LeavesBlock.PERSISTENT, true),
      bamboo.defaultBlockState().setValue(BambooBlock.AGE, 0),
      bamboo.defaultBlockState().setValue(BambooBlock.AGE, 1).setValue(BambooBlock.STAGE, 1),
      cocoa.defaultBlockState().setValue(CocoaBlock.AGE, 0).setValue(HorizontalDirectionalBlock.FACING, Direction.NORTH),
      cocoa.defaultBlockState().setValue(CocoaBlock.AGE, 2).setValue(HorizontalDirectionalBlock.FACING, Direction.EAST),
    ];

    expect(new Set(variants.map((state) => idMap.idFor(state))).size).toBe(variants.length);
  });

  test("throws for unknown states and invalid ids", () => {
    const { airState } = registerGeneratedRenderBlocks();
    const idMap = buildBlockStateIdMap([]);

    expect(() => idMap.idFor(airState)).toThrow(/Unknown block state/);
    expect(() => idMap.stateFor(0)).toThrow(/out of bounds/);
    expect(() => idMap.stateFor(-1)).toThrow(/out of bounds/);
  });

  test("keeps reduced worldgen block ids separate from full block-state ids", () => {
    const { blockStateById } = registerGeneratedRenderBlocks();
    const idMap = buildBlockStateIdMap(registeredBlocks());
    const waterState = Fluids.WATER.defaultFluidState().createLegacyBlock();

    expect(blockStateById.includes(waterState)).toBe(true);
    expect(idMap.idFor(waterState)).not.toBe(blockStateById.indexOf(waterState));
  });
});
