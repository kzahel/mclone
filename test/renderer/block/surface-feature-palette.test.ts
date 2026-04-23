import { beforeEach, describe, expect, test } from "vitest";
import { Registry } from "../../../src/core/registry";
import { ResourceLocation } from "../../../src/core/resource-location";
import { BlockColors } from "../../../src/renderer/block/block-colors";
import { ItemBlockRenderTypes } from "../../../src/renderer/item-block-render-types";
import { RenderType } from "../../../src/renderer/render-type";
import type { Block } from "../../../src/world/level/block/block";
import { FoliageColor } from "../../../src/world/level/foliage-color";
import { GrassColor } from "../../../src/world/level/grass-color";
import { registerGeneratedRenderBlocks } from "../../../src/world/level/generated-render-blocks";
import type { BlockState } from "../../../src/world/level/block/state/block-state";

const TEST_GRASS_COLOR = 0x336699;
const TEST_FOLIAGE_COLOR = 0x225544;

function initializeColorTables(): void {
  GrassColor.init(new Array<number>(65_536).fill(TEST_GRASS_COLOR));
  FoliageColor.init(new Array<number>(65_536).fill(TEST_FOLIAGE_COLOR));
}

function getState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing block ${location}`);
  }

  return block.defaultBlockState();
}

describe("Surface feature palette", () => {
  beforeEach(() => {
    Registry.BLOCK.clear();
    initializeColorTables();
  });

  test("BlockColors registers grass and foliage-tinted feature blocks", () => {
    registerGeneratedRenderBlocks();
    const colors = BlockColors.createDefault();

    expect(colors.getColor(getState("minecraft:grass"), null, null, 0)).toBe(TEST_GRASS_COLOR);
    expect(colors.getColor(getState("minecraft:fern"), null, null, 0)).toBe(TEST_GRASS_COLOR);
    expect(colors.getColor(getState("minecraft:tall_grass"), null, null, 0)).toBe(-1);
    expect(colors.getColor(getState("minecraft:lily_pad"), null, null, 0)).toBe(0x71c35c);
    expect(colors.getColor(getState("minecraft:spruce_leaves"), null, null, 0)).toBe(FoliageColor.getEvergreenColor());
    expect(colors.getColor(getState("minecraft:birch_leaves"), null, null, 0)).toBe(FoliageColor.getBirchColor());
    expect(colors.getColor(getState("minecraft:oak_leaves"), null, null, 0)).toBe(FoliageColor.getDefaultColor());
    expect(colors.getColor(getState("minecraft:sugar_cane"), null, null, 0)).toBe(-1);
  });

  test("generated feature blocks route through the expected render layers", () => {
    registerGeneratedRenderBlocks();

    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:oak_leaves"))).toBe(RenderType.cutoutMipped());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:grass"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:large_fern"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:tall_grass"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:oak_sapling"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:birch_sapling"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:sweet_berry_bush"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:dead_bush"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:seagrass"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:tall_seagrass"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:kelp"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:kelp_plant"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:blue_orchid"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:poppy"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:cornflower"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:lily_pad"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:brown_mushroom"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:cactus"))).toBe(RenderType.cutout());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:oak_log"))).toBe(RenderType.solid());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:birch_leaves"))).toBe(RenderType.cutoutMipped());
    expect(ItemBlockRenderTypes.getChunkRenderType(getState("minecraft:pumpkin"))).toBe(RenderType.solid());
  });
});
