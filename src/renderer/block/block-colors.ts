import { Registry } from "../../core/registry";
import { ResourceLocation } from "../../core/resource-location";
import type { BlockPos } from "../../core/block-pos";
import type { BlockAndTintGetter } from "../../world/level/block-and-tint-getter";
import { FoliageColor } from "../../world/level/foliage-color";
import { GrassColor } from "../../world/level/grass-color";
import type { Block } from "../../world/level/block/block";
import { DoublePlantBlock } from "../../world/level/block/double-plant-block";
import type { BlockState } from "../../world/level/block/state/block-state";
import { DoubleBlockHalf } from "../../world/level/block/state/properties/double-block-half";
import type { Property } from "../../world/level/block/state/properties/property";
import { BiomeColors } from "../biome-colors";

export type BlockColor = (state: BlockState, level: BlockAndTintGetter | null, pos: BlockPos | null, tintIndex: number) => number;

const EMPTY_PROPERTIES = new Set<Property<unknown>>();
const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");
const GRASS_LOCATION = new ResourceLocation("minecraft:grass");
const FERN_LOCATION = new ResourceLocation("minecraft:fern");
const LARGE_FERN_LOCATION = new ResourceLocation("minecraft:large_fern");
const TALL_GRASS_LOCATION = new ResourceLocation("minecraft:tall_grass");
const LILY_PAD_LOCATION = new ResourceLocation("minecraft:lily_pad");
const OAK_LEAVES_LOCATION = new ResourceLocation("minecraft:oak_leaves");
const ACACIA_LEAVES_LOCATION = new ResourceLocation("minecraft:acacia_leaves");
const SPRUCE_LEAVES_LOCATION = new ResourceLocation("minecraft:spruce_leaves");
const BIRCH_LEAVES_LOCATION = new ResourceLocation("minecraft:birch_leaves");
const DARK_OAK_LEAVES_LOCATION = new ResourceLocation("minecraft:dark_oak_leaves");
const JUNGLE_LEAVES_LOCATION = new ResourceLocation("minecraft:jungle_leaves");
const SUGAR_CANE_LOCATION = new ResourceLocation("minecraft:sugar_cane");
const VINE_LOCATION = new ResourceLocation("minecraft:vine");
const WATER_LOCATION = new ResourceLocation("minecraft:water");

export class BlockColors {
  private readonly blockColors = new WeakMap<Block, BlockColor>();
  private readonly coloringStates = new WeakMap<Block, ReadonlySet<Property<unknown>>>();

  public static createDefault(): BlockColors {
    const blockColors = new BlockColors();
    const grassBlock = Registry.BLOCK.get(GRASS_BLOCK_LOCATION) as Block | undefined;
    if (grassBlock !== undefined) {
      blockColors.register(
        (_state, level, pos) => level !== null && pos !== null ? BiomeColors.getAverageGrassColor(level, pos) : GrassColor.get(0.5, 1.0),
        grassBlock,
      );
    }

    const grass = Registry.BLOCK.get(GRASS_LOCATION) as Block | undefined;
    const fern = Registry.BLOCK.get(FERN_LOCATION) as Block | undefined;
    blockColors.register(
      (_state, level, pos) => level !== null && pos !== null ? BiomeColors.getAverageGrassColor(level, pos) : GrassColor.get(0.5, 1.0),
      ...[grass, fern].filter((block): block is Block => block !== undefined),
    );

    const largeFern = Registry.BLOCK.get(LARGE_FERN_LOCATION) as Block | undefined;
    const tallGrass = Registry.BLOCK.get(TALL_GRASS_LOCATION) as Block | undefined;
    const doubleGrassBlocks = [largeFern, tallGrass].filter((block): block is Block => block !== undefined);
    if (doubleGrassBlocks.length > 0) {
      blockColors.register(
        (state, level, pos) =>
          level !== null && pos !== null
            ? BiomeColors.getAverageGrassColor(level, state.getValue(DoublePlantBlock.HALF) === DoubleBlockHalf.UPPER ? pos.below() : pos)
            : -1,
        ...doubleGrassBlocks,
      );
      blockColors.addColoringState(DoublePlantBlock.HALF, ...doubleGrassBlocks);
    }

    const spruceLeaves = Registry.BLOCK.get(SPRUCE_LEAVES_LOCATION) as Block | undefined;
    if (spruceLeaves !== undefined) {
      blockColors.register((_state, _level, _pos) => FoliageColor.getEvergreenColor(), spruceLeaves);
    }

    const birchLeaves = Registry.BLOCK.get(BIRCH_LEAVES_LOCATION) as Block | undefined;
    if (birchLeaves !== undefined) {
      blockColors.register((_state, _level, _pos) => FoliageColor.getBirchColor(), birchLeaves);
    }

    const oakLeaves = Registry.BLOCK.get(OAK_LEAVES_LOCATION) as Block | undefined;
    const acaciaLeaves = Registry.BLOCK.get(ACACIA_LEAVES_LOCATION) as Block | undefined;
    const darkOakLeaves = Registry.BLOCK.get(DARK_OAK_LEAVES_LOCATION) as Block | undefined;
    const jungleLeaves = Registry.BLOCK.get(JUNGLE_LEAVES_LOCATION) as Block | undefined;
    const vine = Registry.BLOCK.get(VINE_LOCATION) as Block | undefined;
    const defaultFoliageBlocks = [oakLeaves, acaciaLeaves, darkOakLeaves, jungleLeaves, vine].filter((block): block is Block => block !== undefined);
    if (defaultFoliageBlocks.length > 0) {
      blockColors.register(
        (_state, level, pos) => level !== null && pos !== null ? BiomeColors.getAverageFoliageColor(level, pos) : FoliageColor.getDefaultColor(),
        ...defaultFoliageBlocks,
      );
    }

    const waterBlock = Registry.BLOCK.get(WATER_LOCATION) as Block | undefined;
    if (waterBlock !== undefined) {
      blockColors.register(
        (_state, level, pos) => level !== null && pos !== null ? BiomeColors.getAverageWaterColor(level, pos) : -1,
        waterBlock,
      );
    }

    const sugarCane = Registry.BLOCK.get(SUGAR_CANE_LOCATION) as Block | undefined;
    if (sugarCane !== undefined) {
      blockColors.register(
        (_state, level, pos) => level !== null && pos !== null ? BiomeColors.getAverageGrassColor(level, pos) : -1,
        sugarCane,
      );
    }

    const lilyPad = Registry.BLOCK.get(LILY_PAD_LOCATION) as Block | undefined;
    if (lilyPad !== undefined) {
      blockColors.register((_state, level, pos) => level !== null && pos !== null ? 2_129_968 : 7_455_580, lilyPad);
    }

    return blockColors;
  }

  public getColor(state: BlockState, level: BlockAndTintGetter | null, pos: BlockPos | null, tintIndex: number): number {
    const blockColor = this.blockColors.get(state.getBlock());
    return blockColor === undefined ? -1 : blockColor(state, level, pos, tintIndex);
  }

  public register(color: BlockColor, ...blocks: readonly Block[]): void {
    for (const block of blocks) {
      this.blockColors.set(block, color);
    }
  }

  public addColoringStates(states: ReadonlySet<Property<unknown>>, ...blocks: readonly Block[]): void {
    for (const block of blocks) {
      this.coloringStates.set(block, states);
    }
  }

  public addColoringState(state: Property<unknown>, ...blocks: readonly Block[]): void {
    this.addColoringStates(new Set([state]), ...blocks);
  }

  public getColoringProperties(block: Block): ReadonlySet<Property<unknown>> {
    return this.coloringStates.get(block) ?? EMPTY_PROPERTIES;
  }
}
