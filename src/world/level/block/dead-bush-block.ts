import { BlockPos } from "../../../core/block-pos";
import type { BlockGetter } from "../block-getter";
import type { BlockState } from "./state/block-state";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockTags } from "../../../tags/block-tags";
import { BushBlock } from "./bush-block";

const VALID_LOCATIONS: ReadonlySet<string> = new Set([
  "minecraft:sand",
  "minecraft:red_sand",
  "minecraft:terracotta",
  "minecraft:white_terracotta",
  "minecraft:orange_terracotta",
  "minecraft:magenta_terracotta",
  "minecraft:light_blue_terracotta",
  "minecraft:yellow_terracotta",
  "minecraft:lime_terracotta",
  "minecraft:pink_terracotta",
  "minecraft:gray_terracotta",
  "minecraft:light_gray_terracotta",
  "minecraft:cyan_terracotta",
  "minecraft:purple_terracotta",
  "minecraft:blue_terracotta",
  "minecraft:brown_terracotta",
  "minecraft:green_terracotta",
  "minecraft:red_terracotta",
  "minecraft:black_terracotta",
] as const satisfies readonly string[]);

export class DeadBushBlock extends BushBlock {
  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
  }

  protected override mayPlaceOn(state: BlockState, _level: BlockGetter, _pos: BlockPos): boolean {
    const location = state.getBlock().getLocation()?.toString();
    return (location !== undefined && VALID_LOCATIONS.has(location)) || state.is(BlockTags.DIRT);
  }
}
