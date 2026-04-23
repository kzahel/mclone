import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { AbstractFlowerFeature } from "./abstract-flower-feature";
import { RandomPatchConfiguration } from "./configurations/random-patch-configuration";

export class DefaultFlowerFeature extends AbstractFlowerFeature<RandomPatchConfiguration> {
  public override isValid(level: WorldGenLevel, pos: BlockPos, config: RandomPatchConfiguration): boolean {
    return !config.blacklist.has(level.getBlockState(pos));
  }

  public override getCount(config: RandomPatchConfiguration): number {
    return config.tries;
  }

  public override getPos(random: SimpleRandomSource, pos: BlockPos, config: RandomPatchConfiguration): BlockPos {
    return pos.offset(
      random.nextInt(config.xspread) - random.nextInt(config.xspread),
      random.nextInt(config.yspread) - random.nextInt(config.yspread),
      random.nextInt(config.zspread) - random.nextInt(config.zspread),
    );
  }

  public override getRandomFlower(random: SimpleRandomSource, pos: BlockPos, config: RandomPatchConfiguration): BlockState {
    return config.stateProvider.getState(random, pos);
  }
}
