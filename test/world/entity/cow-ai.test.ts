import { describe, expect, test } from "vitest";

import { BlockPos } from "../../../src/core/block-pos";
import { EntityRuntime } from "../../../src/runtime/host/entity-runtime";
import { EntityTypes, GeneratedMobEntity } from "../../../src/world/entity/entity-type";
import { MobAttribute } from "../../../src/world/entity/attribute";
import { WaterAvoidingRandomStrollGoal } from "../../../src/world/entity/ai/goal/water-avoiding-random-stroll-goal";
import type { MobAiLevel, PathfinderMob } from "../../../src/world/entity/ai/pathfinder-mob";
import { AirBlock } from "../../../src/world/level/block/air-block";
import { Block } from "../../../src/world/level/block/block";
import { BlockBehaviour } from "../../../src/world/level/block/state/block-behaviour";
import type { BlockState } from "../../../src/world/level/block/state/block-state";
import type { FluidState } from "../../../src/world/level/material/fluid-state";
import { Material } from "../../../src/world/level/material/material";
import { FullChunkStatus } from "../../../src/world/level/entity/full-chunk-status";
import { blockGetterPathNavigationRegion } from "../../../src/world/level/pathfinder";
import { BlockPathTypes } from "../../../src/world/level/pathfinder/block-path-types";
import type { AABB } from "../../../src/world/phys/aabb";

const AIR = new AirBlock(BlockBehaviour.Properties.of(Material.AIR).noCollission().noOcclusion().air()).defaultBlockState();
const STONE = new Block(BlockBehaviour.Properties.of(Material.STONE)).defaultBlockState();

class FlatMobAiLevel implements MobAiLevel {
  private readonly blocks = new Map<string, BlockState>();

  public constructor(
    private readonly minBuildHeight = 0,
    private readonly height = 256,
  ) {
    for (let x = -32; x <= 32; x++) {
      for (let z = -32; z <= 32; z++) {
        this.blocks.set(key(x, 63, z), STONE);
      }
    }
  }

  public getBlockState(pos: BlockPos): BlockState {
    return this.blocks.get(key(pos.getX(), pos.getY(), pos.getZ())) ?? AIR;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public getMaxLightLevel(): number {
    return 15;
  }

  public getMinBuildHeight(): number {
    return this.minBuildHeight;
  }

  public getHeight(): number {
    return this.height;
  }

  public getMaxBuildHeight(): number {
    return this.minBuildHeight + this.height;
  }

  public noCollision(_entity: PathfinderMob | undefined, box: AABB): boolean {
    return blockGetterPathNavigationRegion(this, this.minBuildHeight, this.height).noCollision(_entity, box);
  }

  public findStableStandingY(x: number, z: number, _nearY: number): number | undefined {
    return this.isStableDestination(new BlockPos(x, 64, z)) ? 64 : undefined;
  }

  public isStableDestination(pos: BlockPos): boolean {
    return !this.getBlockState(pos).getMaterial().blocksMotion()
      && !this.getBlockState(pos.above()).getMaterial().blocksMotion()
      && this.getBlockState(pos.below()).getMaterial().isSolid();
  }

  public isWater(_pos: BlockPos): boolean {
    return false;
  }

  public isSolid(pos: BlockPos): boolean {
    return this.getBlockState(pos).getMaterial().isSolid();
  }
}

function key(x: number, y: number, z: number): string {
  return `${x},${y},${z}`;
}

describe("passive mob AI foundation", () => {
  test("ticks generated cows only after their chunk reaches ENTITY_TICKING", () => {
    const cow = new GeneratedMobEntity({
      id: 1,
      uuid: "mclone:test/cow-ticking",
      entityType: EntityTypes.COW,
      x: 8.5,
      y: 64,
      z: 8.5,
      onGround: true,
      randomSeed: 0,
    });
    const runtime = new EntityRuntime<GeneratedMobEntity>({
      tickEntity: (entity) => entity.tickServerAi({ resetNoActionTime: true }),
    });

    runtime.updateChunkStatus(0, 0, FullChunkStatus.BORDER);
    runtime.addWorldGenChunkEntities([cow]);
    runtime.processLifecycle();
    runtime.tick();
    expect(cow.tickCount).toBe(0);

    runtime.updateChunkStatus(0, 0, FullChunkStatus.ENTITY_TICKING);
    runtime.processLifecycle();
    runtime.tick();
    expect(cow.tickCount).toBe(1);
  });

  test("forced WaterAvoidingRandomStrollGoal moves through navigation and move control", () => {
    const cow = new GeneratedMobEntity({
      id: 2,
      uuid: "mclone:test/cow-water-avoiding-stroll",
      entityType: EntityTypes.COW,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
    });
    cow.setAiLevel(new FlatMobAiLevel());
    const goal = new WaterAvoidingRandomStrollGoal(cow, 1.0);
    goal.trigger();
    cow.goalSelector.addGoal(0, goal);

    cow.tickServerAi({ resetNoActionTime: true });

    expect(cow.tickCount).toBe(1);
    expect(cow.position).not.toEqual({ x: 0.5, y: 64, z: 0.5 });
  });

  test("ground path navigation keeps generated mob Y on the stable standing surface", () => {
    const mob = new GeneratedMobEntity({
      id: 3,
      uuid: "mclone:test/mob-grounded-steering",
      entityType: EntityTypes.PIG,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
    });
    mob.setAiLevel(new FlatMobAiLevel());

    expect(mob.getAttributeValue(MobAttribute.MAX_HEALTH)).toBe(10.0);
    expect(mob.getAttributeValue(MobAttribute.MOVEMENT_SPEED)).toBe(0.25);
    expect(mob.getNavigation().moveTo(4.5, 70, 0.5, 1.0)).toBe(true);
    expect(mob.getNavigation().getPath()?.canReach()).toBe(true);
    expect(mob.getNavigation().getPath()?.getTarget()).toEqual(new BlockPos(4, 64, 0));

    mob.tickServerAi({ resetNoActionTime: true });

    expect(mob.position.x).not.toBe(0.5);
    expect(mob.position.y).toBe(64);
    expect(mob.position.z).toBe(0.5);
    expect(mob.isOnGround()).toBe(true);
  });

  test("generated sheep use vanilla passive attributes and shared ground navigation", () => {
    const sheep = new GeneratedMobEntity({
      id: 4,
      uuid: "mclone:test/sheep-ground-navigation",
      entityType: EntityTypes.SHEEP,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
    });
    sheep.setAiLevel(new FlatMobAiLevel());

    expect(sheep.getAttributeValue(MobAttribute.MAX_HEALTH)).toBe(8.0);
    expect(sheep.getAttributeValue(MobAttribute.MOVEMENT_SPEED)).toBe(0.23);
    expect(typeof sheep.data.Color).toBe("number");
    expect(sheep.getNavigation().moveTo(4.5, 64, 0.5, 1.0)).toBe(true);

    sheep.tickServerAi({ resetNoActionTime: true });

    expect(sheep.position.x).not.toBe(0.5);
    expect(sheep.position.y).toBe(64);
  });

  test("generated chickens use vanilla passive attributes and placeholder chicken state", () => {
    const chicken = new GeneratedMobEntity({
      id: 5,
      uuid: "mclone:test/chicken-ground-navigation",
      entityType: EntityTypes.CHICKEN,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
    });
    chicken.setAiLevel(new FlatMobAiLevel());

    expect(chicken.getAttributeValue(MobAttribute.MAX_HEALTH)).toBe(4.0);
    expect(chicken.getAttributeValue(MobAttribute.MOVEMENT_SPEED)).toBe(0.25);
    expect(typeof chicken.data.EggLayTime).toBe("number");
    expect(chicken.data.IsChickenJockey).toBe(false);
    expect(chicken.getPathfindingMalus(BlockPathTypes.WATER)).toBe(0.0);
    expect(chicken.getNavigation().moveTo(4.5, 64, 0.5, 1.0)).toBe(true);

    chicken.tickServerAi({ resetNoActionTime: true });

    expect(chicken.position.x).not.toBe(0.5);
    expect(chicken.position.y).toBe(64);
  });
});
