import { BlockPos } from "../../../core/block-pos";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../../world/level/world-gen-level";
import type { SimpleRandomSource } from "../../prng/simple-random-source";
import { Feature } from "./feature";
import type { FeaturePlaceContext } from "./feature-place-context";
import type { FeatureConfiguration } from "./configurations/feature-configuration";

export abstract class AbstractFlowerFeature<U extends FeatureConfiguration> extends Feature<U> {
  public override place(context: FeaturePlaceContext<U>): boolean {
    const random = context.random();
    const origin = context.origin();
    const level = context.level();
    const config = context.config();
    const flowerState = this.getRandomFlower(random, origin, config);
    let placedCount = 0;

    for (let index = 0; index < this.getCount(config); index++) {
      const pos = this.getPos(random, origin, config);
      if (level.isEmptyBlock(pos) && flowerState.canSurvive(level, pos) && this.isValid(level, pos, config)) {
        level.setBlock(pos, flowerState, 2);
        placedCount++;
      }
    }

    return placedCount > 0;
  }

  public abstract isValid(level: WorldGenLevel, pos: BlockPos, config: U): boolean;

  public abstract getCount(config: U): number;

  public abstract getPos(random: SimpleRandomSource, pos: BlockPos, config: U): BlockPos;

  public abstract getRandomFlower(random: SimpleRandomSource, pos: BlockPos, config: U): BlockState;
}
