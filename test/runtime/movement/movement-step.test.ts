import { describe, expect, test } from "vitest";
import {
  createMissingCollisionWorld,
  createStandingMovementBody,
  createStaticCollisionWorld,
  DEFAULT_MOVEMENT_PHYSICS,
  NO_MOVEMENT_INTENT,
  blockGetterCollisionWorld,
  simulateMovementStep,
  shouldAutoJump,
  type MovementIntent,
  type MovementPhysicsParams,
} from "../../../src/runtime/movement";
import { BlockPos } from "../../../src/core/block-pos";
import { AirBlock } from "../../../src/world/level/block/air-block";
import { Block } from "../../../src/world/level/block/block";
import { BushBlock } from "../../../src/world/level/block/bush-block";
import { LeavesBlock } from "../../../src/world/level/block/leaves-block";
import { SlabBlock } from "../../../src/world/level/block/slab-block";
import { BlockBehaviour } from "../../../src/world/level/block/state/block-behaviour";
import type { BlockState } from "../../../src/world/level/block/state/block-state";
import type { BlockGetter } from "../../../src/world/level/block-getter";
import { Material } from "../../../src/world/level/material/material";
import type { FluidState } from "../../../src/world/level/material/fluid-state";
import { AABB } from "../../../src/world/phys/aabb";
import { Vec3 } from "../../../src/world/phys/vec3";

const TEST_PARAMS: MovementPhysicsParams = {
  ...DEFAULT_MOVEMENT_PHYSICS,
  maxGroundSpeedBlocksPerSecond: 4.0,
  maxAirSpeedBlocksPerSecond: 4.0,
  groundAccelerationBlocksPerSecondSq: 20.0,
  airAccelerationBlocksPerSecondSq: 10.0,
  groundFrictionPerSecond: 0.0,
  airDragPerSecond: 0.0,
  verticalDragPerSecond: 0.0,
  maxStepHeight: 0.6,
};

function intent(overrides: Partial<MovementIntent> = {}): MovementIntent {
  return {
    ...NO_MOVEMENT_INTENT,
    ...overrides,
  };
}

function floor(): AABB {
  return new AABB(-20, -1, -20, 20, 0, 20);
}

class MapBlockGetter implements BlockGetter {
  public constructor(
    private readonly blocks: ReadonlyMap<string, BlockState>,
    private readonly defaultState: BlockState,
  ) {}

  public getBlockState(pos: BlockPos): BlockState {
    return this.blocks.get(blockKey(pos.getX(), pos.getY(), pos.getZ())) ?? this.defaultState;
  }

  public getFluidState(pos: BlockPos): FluidState {
    return this.getBlockState(pos).getFluidState();
  }

  public getMaxLightLevel(): number {
    return 15;
  }
}

function blockKey(x: number, y: number, z: number): string {
  return `${x},${y},${z}`;
}

function airState(): BlockState {
  return new AirBlock(BlockBehaviour.Properties.of(Material.AIR).noCollission().noOcclusion().air()).defaultBlockState();
}

function movementBlockLevel(entries: readonly (readonly [number, number, number, BlockState])[]): BlockGetter {
  const states = new Map<string, BlockState>();
  const stone = new Block(BlockBehaviour.Properties.of(Material.STONE)).defaultBlockState();
  for (let x = -4; x <= 4; x++) {
    for (let z = -2; z <= 2; z++) {
      states.set(blockKey(x, -1, z), stone);
    }
  }
  for (const [x, y, z, state] of entries) {
    states.set(blockKey(x, y, z), state);
  }
  return new MapBlockGetter(states, airState());
}

function expectCloseVec3(actual: Vec3, expected: Vec3): void {
  expect(actual.x).toBeCloseTo(expected.x, 8);
  expect(actual.y).toBeCloseTo(expected.y, 8);
  expect(actual.z).toBeCloseTo(expected.z, 8);
}

describe("simulateMovementStep", () => {
  test("moves a grounded body from fixed intent and fixed dt", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });

    const result = simulateMovementStep(body, intent({ wishZ: 1.0 }), world, 0.1, TEST_PARAMS);

    expectCloseVec3(result.body.position, new Vec3(0, 0, 0.2));
    expectCloseVec3(result.body.velocity, new Vec3(0, 0, 2.0));
    expect(result.body.onGround).toBe(true);
    expect(result.collision.horizontal).toBe(false);
    expect(result.collision.onGround).toBe(true);
    expect(result.collision.missingCollision).toBe(false);
  });

  test("applies gravity and lands on full-block collision", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 1, 0),
      velocity: new Vec3(0, -10, 0),
      onGround: false,
    });

    const result = simulateMovementStep(body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);

    expectCloseVec3(result.body.position, new Vec3(0, 0, 0));
    expectCloseVec3(result.body.velocity, Vec3.ZERO);
    expect(result.body.onGround).toBe(true);
    expect(result.collision.vertical).toBe(true);
    expect(result.collision.onGround).toBe(true);
  });

  test("stops horizontal movement at full-block walls", () => {
    const world = createStaticCollisionWorld([
      floor(),
      new AABB(1, 0, -2, 2, 2, 2),
    ]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(12, 0, 0),
      onGround: true,
    });

    const result = simulateMovementStep(body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);

    expect(result.body.position.x).toBeCloseTo(0.7, 8);
    expect(result.body.velocity.x).toBe(0);
    expect(result.collision.x).toBe(true);
    expect(result.collision.horizontal).toBe(true);
    expect(result.body.bounds.maxX).toBeCloseTo(1.0, 8);
  });

  test("collides with non-occluding leaves from block collision shapes", () => {
    const leaves = new LeavesBlock(BlockBehaviour.Properties.of(Material.LEAVES).noOcclusion()).defaultBlockState();
    const world = blockGetterCollisionWorld(movementBlockLevel([[1, 0, 0, leaves]]));
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(12, 0, 0),
      onGround: true,
    });

    const result = simulateMovementStep(body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);

    expect(result.body.position.x).toBeCloseTo(0.7, 8);
    expect(result.body.bounds.maxX).toBeCloseTo(1.0, 8);
    expect(result.collision.horizontal).toBe(true);
  });

  test("passes through no-collision plants from block collision shapes", () => {
    const plant = new BushBlock(BlockBehaviour.Properties.of(Material.PLANT).noCollission().noOcclusion()).defaultBlockState();
    const world = blockGetterCollisionWorld(movementBlockLevel([[1, 0, 0, plant]]));
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(12, 0, 0),
      onGround: true,
    });

    const result = simulateMovementStep(body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);

    expect(result.body.position.x).toBeGreaterThan(1.0);
    expect(result.collision.horizontal).toBe(false);
  });

  test("clears grounded state after walking off an edge", () => {
    const world = createStaticCollisionWorld([
      new AABB(-2, -1, -2, 0.05, 0, 2),
    ]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(5, 0, 0),
      onGround: true,
    });

    const first = simulateMovementStep(body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);
    const result = simulateMovementStep(first.body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);

    expect(result.body.position.x).toBeGreaterThan(0);
    expect(result.body.onGround).toBe(false);
    expect(result.collision.onGround).toBe(false);
  });

  test("applies one jump impulse while jump is held", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });

    const first = simulateMovementStep(body, intent({ jump: true }), world, 0.05, TEST_PARAMS);
    const second = simulateMovementStep(first.body, intent({ jump: true }), world, 0.05, TEST_PARAMS);

    expect(first.body.velocity.y).toBeCloseTo(TEST_PARAMS.jumpSpeedBlocksPerSecond, 8);
    expect(second.body.velocity.y).toBeLessThan(first.body.velocity.y);
    expect(second.body.velocity.y).toBeGreaterThan(0);
  });

  test("steps up a low full-block obstacle without changing the caller protocol", () => {
    const world = createStaticCollisionWorld([
      floor(),
      new AABB(0.8, 0, -0.5, 1.8, 0.5, 0.5),
    ]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(8, 0, 0),
      onGround: true,
    });

    const result = simulateMovementStep(body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);

    expect(result.body.position.x).toBeGreaterThan(0.7);
    expect(result.body.position.y).toBeCloseTo(0.5, 8);
    expect(result.body.onGround).toBe(true);
  });

  test("steps onto slab collision shape height from a block-backed collision world", () => {
    const slab = new SlabBlock(BlockBehaviour.Properties.of(Material.STONE).noOcclusion()).defaultBlockState();
    const world = blockGetterCollisionWorld(movementBlockLevel([[1, 0, 0, slab]]));
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(8, 0, 0),
      onGround: true,
    });

    const result = simulateMovementStep(body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);

    expect(result.body.position.x).toBeGreaterThan(0.7);
    expect(result.body.position.y).toBeCloseTo(0.5, 8);
    expect(result.body.onGround).toBe(true);
  });

  test("uses the sprint intent to raise the grounded speed cap", () => {
    const world = createStaticCollisionWorld([floor()]);
    const params = {
      ...TEST_PARAMS,
      groundAccelerationBlocksPerSecondSq: 100.0,
    };
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });

    const walking = simulateMovementStep(body, intent({ wishZ: 1.0 }), world, 0.1, params);
    const sprinting = simulateMovementStep(body, intent({ wishZ: 1.0, sprint: true }), world, 0.1, params);

    expect(walking.body.velocity.z).toBeCloseTo(TEST_PARAMS.maxGroundSpeedBlocksPerSecond, 8);
    expect(sprinting.body.velocity.z).toBeCloseTo(
      TEST_PARAMS.maxGroundSpeedBlocksPerSecond * TEST_PARAMS.sprintSpeedMultiplier,
      8,
    );
  });

  test("detects full-block obstacles that should become an auto-jump edge", () => {
    const world = createStaticCollisionWorld([
      floor(),
      new AABB(-0.5, 0, 0.8, 0.5, 1.0, 1.8),
    ]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });

    expect(shouldAutoJump(body, intent({ wishZ: 1.0 }), world, TEST_PARAMS)).toBe(true);
  });

  test("does not auto-jump low step-up obstacles or blocked headroom", () => {
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });
    const lowStepWorld = createStaticCollisionWorld([
      floor(),
      new AABB(-0.5, 0, 0.8, 0.5, 0.5, 1.8),
    ]);
    const blockedHeadroomWorld = createStaticCollisionWorld([
      floor(),
      new AABB(-0.5, 0, 0.8, 0.5, 1.0, 1.8),
      new AABB(-0.5, 2.0, 0.8, 0.5, 3.0, 1.8),
    ]);

    expect(shouldAutoJump(body, intent({ wishZ: 1.0 }), lowStepWorld, TEST_PARAMS)).toBe(false);
    expect(shouldAutoJump(body, intent({ wishZ: 1.0 }), blockedHeadroomWorld, TEST_PARAMS)).toBe(false);
  });

  test("reports missing collision data instead of moving through it", () => {
    const world = createMissingCollisionWorld("chunk not loaded");
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(1, 0, 0),
      onGround: true,
    });

    const result = simulateMovementStep(body, NO_MOVEMENT_INTENT, world, 0.1, TEST_PARAMS);

    expect(result.body).toBe(body);
    expect(result.collision.missingCollision).toBe(true);
    expectCloseVec3(result.actualDisplacement, Vec3.ZERO);
  });

  test("is deterministic for the same body, intent, world, params, and fixed dt", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 2, 0),
      velocity: new Vec3(1, 0, 0.5),
      onGround: false,
    });
    const moveIntent = intent({ wishX: 0.2, wishZ: 1.0, yaw: 35.0 });

    const left = simulateMovementStep(body, moveIntent, world, 1 / 120, TEST_PARAMS);
    const right = simulateMovementStep(body, moveIntent, world, 1 / 120, TEST_PARAMS);

    expect(left).toEqual(right);
  });

  test("does not move from stale intent when no fixed step is requested", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(10, 0, 0),
      onGround: true,
    });

    const result = simulateMovementStep(body, intent({ wishX: 1.0, jump: true }), world, 0.0, TEST_PARAMS);

    expect(result.body).toBe(body);
    expectCloseVec3(result.actualDisplacement, Vec3.ZERO);
  });
});
