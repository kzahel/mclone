import { ResourceLocation } from "../../../../core/resource-location";
import { Goal, GoalFlag } from "./goal";
import type { MobAiLevel, PathfinderMob } from "../pathfinder-mob";
import type { BlockState } from "../../../level/block/state/block-state";

const EAT_ANIMATION_TICKS = 40;
const EAT_MUTATION_TICK = 4;
const ADULT_EAT_CHANCE = 1000;
const BABY_EAT_CHANCE = 50;
const AIR_LOCATION = new ResourceLocation("minecraft:air");
const DIRT_LOCATION = new ResourceLocation("minecraft:dirt");
const GRASS_LOCATION = new ResourceLocation("minecraft:grass");
const GRASS_BLOCK_LOCATION = new ResourceLocation("minecraft:grass_block");

export class EatBlockGoal extends Goal {
  private eatAnimationTick = 0;

  public constructor(private readonly mob: PathfinderMob) {
    super();
    this.setFlags([GoalFlag.MOVE, GoalFlag.LOOK, GoalFlag.JUMP]);
  }

  public canUse(): boolean {
    const level = this.mob.getAiLevel();
    if (level === undefined) {
      return false;
    }

    if (this.mob.getRandom().nextInt(this.mob.isBaby() ? BABY_EAT_CHANCE : ADULT_EAT_CHANCE) !== 0) {
      return false;
    }

    const pos = this.mob.blockPosition();
    return isBlock(level.getBlockState(pos), GRASS_LOCATION) || isBlock(level.getBlockState(pos.below()), GRASS_BLOCK_LOCATION);
  }

  public override start(): void {
    this.eatAnimationTick = EAT_ANIMATION_TICKS;
    this.mob.getNavigation().stop();
  }

  public override stop(): void {
    this.eatAnimationTick = 0;
  }

  public override canContinueToUse(): boolean {
    return this.eatAnimationTick > 0;
  }

  public getEatAnimationTick(): number {
    return this.eatAnimationTick;
  }

  public override tick(): void {
    this.eatAnimationTick = Math.max(0, this.eatAnimationTick - 1);
    if (this.eatAnimationTick !== EAT_MUTATION_TICK) {
      return;
    }

    const level = this.mob.getAiLevel();
    if (level === undefined) {
      return;
    }

    const pos = this.mob.blockPosition();
    if (isBlock(level.getBlockState(pos), GRASS_LOCATION)) {
      if (canMobGrief(level)) {
        if (level.destroyBlock !== undefined) {
          level.destroyBlock(pos, false);
        } else {
          const airState = level.getDefaultBlockState?.(AIR_LOCATION);
          if (airState !== undefined) {
            level.setBlock?.(pos, airState, 2);
          }
        }
      }
      this.mob.ate();
      return;
    }

    const below = pos.below();
    if (isBlock(level.getBlockState(below), GRASS_BLOCK_LOCATION)) {
      if (canMobGrief(level)) {
        const dirtState = level.getDefaultBlockState?.(DIRT_LOCATION);
        if (dirtState !== undefined) {
          level.setBlock?.(below, dirtState, 2);
        }
      }
      this.mob.ate();
    }
  }
}

function canMobGrief(level: MobAiLevel): boolean {
  // Runtime: game-rule storage is not modeled yet; default to vanilla's true mobGriefing setting.
  return level.getGameRuleMobGriefing?.() ?? true;
}

function isBlock(state: BlockState, location: ResourceLocation): boolean {
  return state.getBlock().getLocation()?.equals(location) ?? false;
}
