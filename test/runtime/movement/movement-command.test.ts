import { describe, expect, test } from "vitest";
import {
  createStandingMovementBody,
  createStaticCollisionWorld,
  DEFAULT_MOVEMENT_PHYSICS,
  MOVEMENT_COMMAND_BUTTONS,
  MovementCommandBuffer,
  MovementCommandClock,
  simulateMovementStep,
  simulatePlayerMoveCommand,
  simulatePlayerMoveCommands,
  splitCommandStepCounts,
  type PlayerMoveCommand,
} from "../../../src/runtime/movement";
import { AABB } from "../../../src/world/phys/aabb";
import { Vec3 } from "../../../src/world/phys/vec3";

const QUANTUM_US = 8_000;
const PARAMS = {
  ...DEFAULT_MOVEMENT_PHYSICS,
  groundFrictionPerSecond: 0.0,
  airDragPerSecond: 0.0,
  verticalDragPerSecond: 0.0,
};

function command(sequence: number, overrides: Partial<PlayerMoveCommand> = {}): PlayerMoveCommand {
  return {
    type: "player_move_command",
    playerId: "player-1",
    sequence,
    clientTimeUs: sequence * QUANTUM_US,
    commandQuantumUs: QUANTUM_US,
    stepCount: 1,
    buttons: 0,
    edgeButtons: 0,
    wishX: 0,
    wishZ: 1,
    yaw: 0,
    pitch: 0,
    physicsRevision: 1,
    ...overrides,
  };
}

function floor(): AABB {
  return new AABB(-20, -1, -20, 20, 0, 20);
}

describe("movement command clock", () => {
  test("converts elapsed time into bounded fixed command steps", () => {
    const clock = new MovementCommandClock({
      commandQuantumUs: QUANTUM_US,
      maxStepCountPerCommand: 3,
      maxCatchupStepCount: 12,
    });

    expect(clock.consumeElapsedUs(4_000)).toEqual([]);
    expect(clock.pendingRemainderUs).toBe(4_000);
    expect(clock.consumeElapsedUs(36_000)).toEqual([3, 2]);
    expect(clock.pendingRemainderUs).toBe(0);
  });

  test("splits large catch-up spans without creating variable-dt commands", () => {
    expect(splitCommandStepCounts(10, 4)).toEqual([4, 4, 2]);

    const clock = new MovementCommandClock({
      commandQuantumUs: QUANTUM_US,
      maxStepCountPerCommand: 5,
      maxCatchupStepCount: 9,
    });
    expect(clock.consumeElapsedUs(20 * QUANTUM_US)).toEqual([5, 4]);
    expect(clock.pendingRemainderUs).toBe(0);
  });
});

describe("movement command simulation", () => {
  test("stepCount simulates repeated fixed quanta, not one doubled dt", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });

    const repeated = simulatePlayerMoveCommand(body, command(1, { stepCount: 2 }), world, PARAMS);
    const separate = simulatePlayerMoveCommands(body, [command(1), command(2)], world, PARAMS);
    const oneLargeStep = simulateMovementStep(body, {
      wishX: 0,
      wishZ: 1,
      jump: false,
      crouch: false,
      sprint: false,
      yaw: 0,
      pitch: 0,
    }, world, (QUANTUM_US * 2) / 1_000_000, PARAMS);

    expect(repeated.body).toEqual(separate.body);
    expect(repeated.stepResults).toHaveLength(2);
    expect(repeated.body.position.z).not.toBeCloseTo(oneLargeStep.body.position.z, 8);
  });

  test("edgeButtons preserve quick jump taps", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });

    const tapped = simulatePlayerMoveCommand(
      body,
      command(1, { wishZ: 0, edgeButtons: MOVEMENT_COMMAND_BUTTONS.JUMP }),
      world,
      PARAMS,
    );
    const released = simulatePlayerMoveCommand(tapped.body, command(2, { wishZ: 0 }), world, PARAMS);

    expect(tapped.body.velocity.y).toBeGreaterThan(0);
    expect(tapped.body.jumpHeld).toBe(true);
    expect(released.body.velocity.y).toBeLessThan(tapped.body.velocity.y);
    expect(released.body.jumpHeld).toBe(false);
  });

  test("zero commands do not move from stale input", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      velocity: new Vec3(10, 0, 0),
      onGround: true,
    });

    const result = simulatePlayerMoveCommands(body, [], world, PARAMS);

    expect(result.body).toBe(body);
    expect(result.stepResults).toEqual([]);
    expect(result.lastProcessedCommandSeq).toBe(0);
  });

  test("requires strictly ordered command batches", () => {
    const world = createStaticCollisionWorld([floor()]);
    const body = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });

    expect(() => simulatePlayerMoveCommands(body, [command(2), command(1)], world, PARAMS)).toThrow(/strictly ordered/);
  });
});

describe("MovementCommandBuffer", () => {
  test("stores commands in sequence and drops acknowledged commands", () => {
    const buffer = new MovementCommandBuffer(3);

    buffer.add(command(1));
    buffer.add(command(2));
    buffer.add(command(3));
    buffer.add(command(4));

    expect(buffer.getUnacknowledged().map((current) => current.sequence)).toEqual([2, 3, 4]);
    expect(buffer.firstSequence).toBe(2);
    expect(buffer.lastSequence).toBe(4);

    buffer.dropAcknowledged(3);
    expect(buffer.getUnacknowledged().map((current) => current.sequence)).toEqual([4]);
  });

  test("rejects duplicate or out-of-order stores", () => {
    const buffer = new MovementCommandBuffer();
    buffer.add(command(1));

    expect(() => buffer.add(command(1))).toThrow(/greater than previous/);
  });
});
