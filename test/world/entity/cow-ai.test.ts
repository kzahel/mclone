import { describe, expect, test } from "vitest";

import { EntityRuntime } from "../../../src/runtime/host/entity-runtime";
import { EntityTypes, GeneratedMobEntity } from "../../../src/world/entity/entity-type";
import { WaterAvoidingRandomStrollGoal } from "../../../src/world/entity/ai/goal/water-avoiding-random-stroll-goal";
import { FullChunkStatus } from "../../../src/world/level/entity/full-chunk-status";

describe("cow AI foundation", () => {
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
    cow.setAiLevel({
      getMinBuildHeight: () => 0,
      getMaxBuildHeight: () => 256,
      isStableDestination: () => true,
      isWater: () => false,
      isSolid: () => false,
    });
    const goal = new WaterAvoidingRandomStrollGoal(cow, 1.0);
    goal.trigger();
    cow.goalSelector.addGoal(0, goal);

    cow.tickServerAi({ resetNoActionTime: true });

    expect(cow.tickCount).toBe(1);
    expect(cow.position).not.toEqual({ x: 0.5, y: 64, z: 0.5 });
  });
});
