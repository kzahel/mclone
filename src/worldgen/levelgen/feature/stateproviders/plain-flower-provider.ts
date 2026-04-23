import { BlockPos } from "../../../../core/block-pos";
import { Registry } from "../../../../core/registry";
import { ResourceLocation } from "../../../../core/resource-location";
import type { Block } from "../../../../world/level/block/block";
import type { BlockState } from "../../../../world/level/block/state/block-state";
import { Biome } from "../../../biome/biome";
import type { SimpleRandomSource } from "../../../prng/simple-random-source";
import { BlockStateProvider } from "./block-state-provider";

const LOW_NOISE_FLOWER_LOCATIONS = [
  "minecraft:orange_tulip",
  "minecraft:red_tulip",
  "minecraft:pink_tulip",
  "minecraft:white_tulip",
] as const;
const HIGH_NOISE_FLOWER_LOCATIONS = [
  "minecraft:poppy",
  "minecraft:azure_bluet",
  "minecraft:oxeye_daisy",
  "minecraft:cornflower",
] as const;
const DANDELION_LOCATION = "minecraft:dandelion";

function getRequiredState(location: string): BlockState {
  const block = Registry.BLOCK.get(new ResourceLocation(location)) as Block | undefined;
  if (block === undefined) {
    throw new Error(`Missing registered block ${location}`);
  }

  return block.defaultBlockState();
}

function getRandom<T>(values: readonly T[], random: SimpleRandomSource): T {
  return values[random.nextInt(values.length)]!;
}

export class PlainFlowerProvider extends BlockStateProvider {
  public static readonly INSTANCE = new PlainFlowerProvider();

  private constructor() {
    super();
  }

  public override getState(random: SimpleRandomSource, pos: BlockPos): BlockState {
    const lowNoiseFlowers = LOW_NOISE_FLOWER_LOCATIONS.map((location) => getRequiredState(location));
    const highNoiseFlowers = HIGH_NOISE_FLOWER_LOCATIONS.map((location) => getRequiredState(location));
    const dandelion = getRequiredState(DANDELION_LOCATION);
    const noise = Biome.BIOME_INFO_NOISE.getValue(pos.getX() / 200.0, pos.getZ() / 200.0, false);
    if (noise < -0.8) {
      return getRandom(lowNoiseFlowers, random);
    }

    return random.nextInt(3) > 0 ? getRandom(highNoiseFlowers, random) : dandelion;
  }
}
