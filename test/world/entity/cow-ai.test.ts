import { describe, expect, test } from "vitest";

import { BlockPos } from "../../../src/core/block-pos";
import { ResourceLocation } from "../../../src/core/resource-location";
import { EntityRuntime } from "../../../src/runtime/host/entity-runtime";
import { EntityTypes, GeneratedMobEntity } from "../../../src/world/entity/entity-type";
import { MobAttribute } from "../../../src/world/entity/attribute";
import { EatBlockGoal } from "../../../src/world/entity/ai/goal/eat-block-goal";
import { LookAtPlayerGoal } from "../../../src/world/entity/ai/goal/look-at-player-goal";
import { WaterAvoidingRandomStrollGoal } from "../../../src/world/entity/ai/goal/water-avoiding-random-stroll-goal";
import type { MobAiLevel, MobLookTarget, PathfinderMob } from "../../../src/world/entity/ai/pathfinder-mob";
import { AirBlock } from "../../../src/world/level/block/air-block";
import { Block } from "../../../src/world/level/block/block";
import { BushBlock } from "../../../src/world/level/block/bush-block";
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
const DIRT = namedState(new Block(BlockBehaviour.Properties.of(Material.DIRT)), "minecraft:dirt");
const GRASS_BLOCK = namedState(new Block(BlockBehaviour.Properties.of(Material.GRASS)), "minecraft:grass_block");
const SHORT_GRASS = namedState(
  new BushBlock(BlockBehaviour.Properties.of(Material.REPLACEABLE_PLANT).noCollission().noOcclusion()),
  "minecraft:grass",
);

class FlatMobAiLevel implements MobAiLevel {
  private readonly blocks = new Map<string, BlockState>();
  private readonly defaultStates = new Map<string, BlockState>([
    ["minecraft:air", AIR],
    ["minecraft:dirt", DIRT],
    ["minecraft:grass", SHORT_GRASS],
    ["minecraft:grass_block", GRASS_BLOCK],
  ]);
  private nearestPlayer: MobLookTarget | undefined;

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

  public getDefaultBlockState(location: ResourceLocation): BlockState | undefined {
    return this.defaultStates.get(location.toString());
  }

  public getGameRuleMobGriefing(): boolean {
    return true;
  }

  public setBlock(pos: BlockPos, state: BlockState, _flags = 3): boolean {
    this.blocks.set(key(pos.getX(), pos.getY(), pos.getZ()), state);
    return true;
  }

  public destroyBlock(pos: BlockPos, _dropBlock: boolean): boolean {
    return this.setBlock(pos, AIR);
  }

  public setNearestPlayer(player: MobLookTarget | undefined): void {
    this.nearestPlayer = player;
  }

  public getNearestPlayer(x: number, y: number, z: number, range: number): MobLookTarget | undefined {
    if (this.nearestPlayer === undefined) {
      return undefined;
    }

    const dx = this.nearestPlayer.getX() - x;
    const dy = this.nearestPlayer.getEyeY() - y;
    const dz = this.nearestPlayer.getZ() - z;
    return (dx * dx) + (dy * dy) + (dz * dz) <= range * range ? this.nearestPlayer : undefined;
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

function namedState(block: Block, location: string): BlockState {
  block.setLocation(new ResourceLocation(location));
  return block.defaultBlockState();
}

function livingTarget(x: number, y: number, z: number, eyeY: number): MobLookTarget {
  return {
    getX: () => x,
    getY: () => y,
    getZ: () => z,
    getEyeY: () => eyeY,
    isAlive: () => true,
  };
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

  test("generated mooshrooms reuse cow baseline movement with mooshroom data", () => {
    const mooshroom = new GeneratedMobEntity({
      id: 16,
      uuid: "mclone:test/mooshroom-ground-navigation",
      entityType: EntityTypes.MOOSHROOM,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
    });
    mooshroom.setAiLevel(new FlatMobAiLevel());

    expect(mooshroom.getAttributeValue(MobAttribute.MAX_HEALTH)).toBe(10.0);
    expect(mooshroom.getAttributeValue(MobAttribute.MOVEMENT_SPEED)).toBe(0.2);
    expect(mooshroom.data.Type).toBe("red");
    expect(mooshroom.getNavigation().moveTo(4.5, 64, 0.5, 1.0)).toBe(true);

    mooshroom.tickServerAi({ resetNoActionTime: true });

    expect(mooshroom.position.x).not.toBe(0.5);
    expect(mooshroom.position.y).toBe(64);
  });

  test("generated rabbits use vanilla baseline attributes and slower stroll speed", () => {
    const rabbit = new GeneratedMobEntity({
      id: 17,
      uuid: "mclone:test/rabbit-ground-navigation",
      entityType: EntityTypes.RABBIT,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
    });
    rabbit.setAiLevel(new FlatMobAiLevel());

    expect(rabbit.getAttributeValue(MobAttribute.MAX_HEALTH)).toBe(3.0);
    expect(rabbit.getAttributeValue(MobAttribute.MOVEMENT_SPEED)).toBe(0.3);
    expect(rabbit.data.RabbitType).toBe(0);
    expect(rabbit.getNavigation().moveTo(4.5, 64, 0.5, 0.6)).toBe(true);

    rabbit.tickServerAi({ resetNoActionTime: true });

    expect(rabbit.position.x).not.toBe(0.5);
    expect(rabbit.position.y).toBe(64);
  });

  test("generated wolves use vanilla baseline attributes and passive look/stroll goals", () => {
    const wolf = new GeneratedMobEntity({
      id: 18,
      uuid: "mclone:test/wolf-ground-navigation",
      entityType: EntityTypes.WOLF,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
    });
    wolf.setAiLevel(new FlatMobAiLevel());

    expect(wolf.getAttributeValue(MobAttribute.MAX_HEALTH)).toBe(8.0);
    expect(wolf.getAttributeValue(MobAttribute.MOVEMENT_SPEED)).toBe(0.3);
    expect(wolf.data.Tame).toBe(false);
    expect(wolf.data.CollarColor).toBe(14);
    expect(wolf.getNavigation().moveTo(4.5, 64, 0.5, 1.0)).toBe(true);

    wolf.tickServerAi({ resetNoActionTime: true });

    expect(wolf.position.x).not.toBe(0.5);
    expect(wolf.position.y).toBe(64);
  });

  test("EatBlockGoal converts grass block below sheep to dirt and regrows wool data", () => {
    const level = new FlatMobAiLevel();
    const sheep = new GeneratedMobEntity({
      id: 14,
      uuid: "mclone:test/sheep-eat-grass-block",
      entityType: EntityTypes.SHEEP,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
      data: {
        Color: 0,
        Sheared: true,
      },
    });
    sheep.setAiLevel(level);
    level.setBlock(new BlockPos(0, 63, 0), GRASS_BLOCK);
    const goal = new EatBlockGoal(sheep);

    goal.start();
    for (let tick = 0; tick < 36; tick++) {
      goal.tick();
    }

    expect(goal.getEatAnimationTick()).toBe(4);
    expect(level.getBlockState(new BlockPos(0, 63, 0)).getBlock().getLocation()?.toString()).toBe("minecraft:dirt");
    expect(sheep.getSnapshotData().Sheared).toBe(false);
  });

  test("EatBlockGoal destroys short grass at the sheep position", () => {
    const level = new FlatMobAiLevel();
    const sheep = new GeneratedMobEntity({
      id: 15,
      uuid: "mclone:test/sheep-eat-short-grass",
      entityType: EntityTypes.SHEEP,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
    });
    sheep.setAiLevel(level);
    level.setBlock(new BlockPos(0, 64, 0), SHORT_GRASS);
    const goal = new EatBlockGoal(sheep);

    goal.start();
    for (let tick = 0; tick < 36; tick++) {
      goal.tick();
    }

    expect(level.getBlockState(new BlockPos(0, 64, 0)).isAir()).toBe(true);
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

  test("generated chickens tick vanilla flap state and egg timer into snapshot data", () => {
    const chicken = new GeneratedMobEntity({
      id: 6,
      uuid: "mclone:test/chicken-live-flap",
      entityType: EntityTypes.CHICKEN,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
      data: {
        Flap: 0.0,
        FlapSpeed: 0.0,
        OFlap: 0.0,
        OFlapSpeed: 0.0,
        Flapping: 1.0,
        EggLayTime: 3,
        IsChickenJockey: false,
      },
    });

    chicken.tickServerAi({ resetNoActionTime: true });
    const firstSnapshot = chicken.getSnapshotData();
    expect(firstSnapshot.OFlap).toBe(0.0);
    expect(firstSnapshot.OFlapSpeed).toBe(0.0);
    expect(firstSnapshot.FlapSpeed).toBe(0.0);
    expect(firstSnapshot.Flapping).toBeCloseTo(0.9);
    expect(firstSnapshot.Flap).toBeCloseTo(1.8);
    expect(firstSnapshot.EggLayTime).toBe(2);

    chicken.tickServerAi({ resetNoActionTime: true });
    const secondSnapshot = chicken.getSnapshotData();
    expect(secondSnapshot.OFlap).toBeCloseTo(1.8);
    expect(secondSnapshot.OFlapSpeed).toBe(0.0);
    expect(secondSnapshot.Flapping).toBeCloseTo(0.81);
    expect(secondSnapshot.Flap).toBeCloseTo(3.42);
    expect(secondSnapshot.EggLayTime).toBe(1);
  });

  test("generated chickens reset egg timer without spawning item entities yet", () => {
    const chicken = new GeneratedMobEntity({
      id: 7,
      uuid: "mclone:test/chicken-egg-reset",
      entityType: EntityTypes.CHICKEN,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
      data: {
        EggLayTime: 1,
        IsChickenJockey: false,
      },
    });

    chicken.tickServerAi({ resetNoActionTime: true });
    const eggLayTime = chicken.getSnapshotData().EggLayTime;
    expect(typeof eggLayTime).toBe("number");
    expect(eggLayTime).toBeGreaterThanOrEqual(6000);
    expect(eggLayTime).toBeLessThan(12000);
  });

  test("generated chicken jockeys do not decrement egg timers", () => {
    const chicken = new GeneratedMobEntity({
      id: 8,
      uuid: "mclone:test/chicken-jockey-egg-timer",
      entityType: EntityTypes.CHICKEN,
      x: 0.5,
      y: 64,
      z: 0.5,
      onGround: true,
      randomSeed: 0,
      data: {
        EggLayTime: 3,
        IsChickenJockey: true,
      },
    });

    chicken.tickServerAi({ resetNoActionTime: true });

    expect(chicken.getSnapshotData().EggLayTime).toBe(3);
    expect(chicken.getSnapshotData().IsChickenJockey).toBe(true);
  });

  test("generated passive mobs expose vanilla standing eye heights for shared look control", () => {
    expect(new GeneratedMobEntity({
      id: 9,
      uuid: "mclone:test/cow-eye-height",
      entityType: EntityTypes.COW,
      x: 0.5,
      y: 64,
      z: 0.5,
    }).getEyeY()).toBeCloseTo(65.3);

    expect(new GeneratedMobEntity({
      id: 10,
      uuid: "mclone:test/sheep-eye-height",
      entityType: EntityTypes.SHEEP,
      x: 0.5,
      y: 64,
      z: 0.5,
    }).getEyeY()).toBeCloseTo(65.235);

    expect(new GeneratedMobEntity({
      id: 11,
      uuid: "mclone:test/chicken-eye-height",
      entityType: EntityTypes.CHICKEN,
      x: 0.5,
      y: 64,
      z: 0.5,
    }).getEyeY()).toBeCloseTo(64.644);
  });

  test("LookControl rotates generated mob head and publishes render rotation data", () => {
    const cow = new GeneratedMobEntity({
      id: 12,
      uuid: "mclone:test/cow-look-control",
      entityType: EntityTypes.COW,
      x: 0.5,
      y: 64,
      z: 0.5,
      yaw: 0,
      pitch: 0,
      randomSeed: 0,
    });

    cow.getLookControl().setLookAt(10.5, cow.getEyeY(), 0.5);
    cow.getLookControl().tick();

    expect(cow.getYBodyRot()).toBe(0);
    expect(cow.getYHeadRot()).toBeCloseTo(-10);
    expect(cow.getXRot()).toBe(0);
    expect(cow.getSnapshotData().YHeadRot).toBeCloseTo(-10);
  });

  test("LookAtPlayerGoal acquires the nearest player through the mob level", () => {
    const level = new FlatMobAiLevel();
    const cow = new GeneratedMobEntity({
      id: 13,
      uuid: "mclone:test/cow-look-at-player",
      entityType: EntityTypes.COW,
      x: 0.5,
      y: 64,
      z: 0.5,
      yaw: 0,
      pitch: 0,
      randomSeed: 0,
    });
    level.setNearestPlayer(livingTarget(4.5, 64, 0.5, 65.62));
    cow.setAiLevel(level);
    const goal = new LookAtPlayerGoal(cow, 6.0, 1.0);

    expect(goal.canUse()).toBe(true);
    goal.start();
    goal.tick();
    cow.getLookControl().tick();

    expect(cow.getYHeadRot()).toBeCloseTo(-10);
    expect(cow.getXRot()).toBeLessThan(0);
  });
});
