import { BlockPos } from "../../../../core/block-pos";
import { Registry } from "../../../../core/registry";
import { ResourceLocation } from "../../../../core/resource-location";
import { clamp } from "../../../../util/mth";
import type { Block } from "../../../../world/level/block/block";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import { Biome } from "../../../biome/biome";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { BlockStateProvider } from "./block-state-provider";

const FLOWER_LOCATIONS = [
  "minecraft:dandelion",
  "minecraft:poppy",
  "minecraft:allium",
  "minecraft:azure_bluet",
  "minecraft:red_tulip",
  "minecraft:orange_tulip",
  "minecraft:white_tulip",
  "minecraft:pink_tulip",
  "minecraft:oxeye_daisy",
  "minecraft:cornflower",
  "minecraft:lily_of_the_valley",
] as const;

function getRequiredState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

export class ForestFlowerProvider extends BlockStateProvider {
  public static readonly INSTANCE = new ForestFlowerProvider();

  private constructor() {
    super();
  }

  public override getState(_random: SimpleRandomSource, pos: BlockPos): BlockState {
    const flowers = FLOWER_LOCATIONS.map((location) => getRequiredState(location));
    const noise = clamp((1.0 + Biome.BIOME_INFO_NOISE.getValue(pos.getX() / 48.0, pos.getZ() / 48.0, false)) / 2.0, 0.0, 0.9999);
    return flowers[Math.trunc(noise * flowers.length)]!;
  }
}
