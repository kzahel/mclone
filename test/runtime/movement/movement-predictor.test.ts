import { describe, expect, test } from "vitest";
import {
  createStandingMovementBody,
  createStaticCollisionWorld,
  DEFAULT_MOVEMENT_PHYSICS,
  MOVEMENT_COMMAND_BUTTONS,
  PlayerMovementPredictor,
  simulatePlayerMoveCommand,
  simulatePlayerMoveCommands,
  type MovementAuthoritativeState,
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

function floor(): AABB {
  return new AABB(-20, -1, -20, 20, 0, 20);
}

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
    collisionRevision: 7,
    ...overrides,
  };
}

function authoritative(body: ReturnType<typeof createStandingMovementBody>, lastProcessedCommandSeq: number): MovementAuthoritativeState {
  return {
    body,
    lastProcessedCommandSeq,
    physicsRevision: 1,
    collisionRevision: 7,
  };
}

describe("PlayerMovementPredictor", () => {
  test("matches server command drain when all commands are acknowledged", () => {
    const world = createStaticCollisionWorld([floor()]);
    const initial = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });
    const commands = [command(1), command(2), command(3)];
    const server = simulatePlayerMoveCommands(initial, commands, world, PARAMS);
    const predictor = new PlayerMovementPredictor(initial);

    for (const current of commands) {
      predictor.applyLocalCommand(current, world, PARAMS);
    }
    const result = predictor.reconcile(authoritative(server.body, server.lastProcessedCommandSeq), world, PARAMS, {
      expectedCommandQuantumUs: QUANTUM_US,
      currentPhysicsRevision: 1,
      currentCollisionRevision: 7,
    });

    expect(result.body).toEqual(server.body);
    expect(predictor.pendingCommands).toEqual([]);
    expect(result.diagnostics.acknowledgedSequence).toBe(3);
    expect(result.diagnostics.replayCount).toBe(0);
  });

  test("snaps to authoritative state and replays unacknowledged commands", () => {
    const world = createStaticCollisionWorld([floor()]);
    const initial = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });
    const commands = [command(1), command(2), command(3)];
    const ackOne = simulatePlayerMoveCommand(initial, commands[0]!, world, PARAMS);
    const allCommands = simulatePlayerMoveCommands(initial, commands, world, PARAMS);
    const predictor = new PlayerMovementPredictor(initial);

    for (const current of commands) {
      predictor.applyLocalCommand(current, world, PARAMS);
    }
    const result = predictor.reconcile(authoritative(ackOne.body, 1), world, PARAMS, {
      expectedCommandQuantumUs: QUANTUM_US,
    });

    expect(result.body).toEqual(allCommands.body);
    expect(predictor.pendingCommands.map((current) => current.sequence)).toEqual([2, 3]);
    expect(result.diagnostics.replayCount).toBe(2);
    expect(result.diagnostics.replayFirstSequence).toBe(2);
    expect(result.diagnostics.replayLastSequence).toBe(3);
    expect(result.diagnostics.causeTags).toContain("replay_backlog");
    expect(result.diagnostics.correctionPositionError).toBeGreaterThan(0);
  });

  test("processes high-rate commands inside a lower-rate server tick without collapsing dt", () => {
    const world = createStaticCollisionWorld([floor()]);
    const initial = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });
    const tickCommands = [command(1), command(2)];
    const server = simulatePlayerMoveCommands(initial, tickCommands, world, PARAMS);
    const oneLargeCommand = simulatePlayerMoveCommand(
      initial,
      command(1, { commandQuantumUs: QUANTUM_US * 2 }),
      world,
      PARAMS,
    );
    const predictor = new PlayerMovementPredictor(initial);

    predictor.applyLocalCommand(tickCommands[0]!, world, PARAMS);
    predictor.applyLocalCommand(tickCommands[1]!, world, PARAMS);
    predictor.reconcile(authoritative(server.body, 2), world, PARAMS);

    expect(predictor.predictedBody).toEqual(server.body);
    expect(server.body.position.z).not.toBeCloseTo(oneLargeCommand.body.position.z, 8);
  });

  test("preserves quick jump taps during replay via edgeButtons", () => {
    const world = createStaticCollisionWorld([floor()]);
    const initial = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });
    const commands = [
      command(1, { wishZ: 0, edgeButtons: MOVEMENT_COMMAND_BUTTONS.JUMP }),
      command(2, { wishZ: 0 }),
    ];
    const predictor = new PlayerMovementPredictor(initial);
    const ackZero = authoritative(initial, 0);

    for (const current of commands) {
      predictor.applyLocalCommand(current, world, PARAMS);
    }
    const result = predictor.reconcile(ackZero, world, PARAMS);

    expect(result.body.velocity.y).toBeGreaterThan(0);
    expect(result.diagnostics.replayCount).toBe(2);
    expect(result.diagnostics.causeTags).toContain("replay_backlog");
  });

  test("classifies command dt, physics revision, collision revision, and missing collision diagnostics", () => {
    const loadedWorld = createStaticCollisionWorld([floor()]);
    const missingWorld = {
      queryBlockCollisions: () => ({ type: "missing" as const, reason: "chunk not loaded" }),
    };
    const initial = createStandingMovementBody({
      position: new Vec3(0, 0, 0),
      onGround: true,
    });
    const predictor = new PlayerMovementPredictor(initial);
    const current = command(1, {
      commandQuantumUs: QUANTUM_US * 2,
      physicsRevision: 2,
      collisionRevision: 8,
    });

    predictor.applyLocalCommand(current, loadedWorld, PARAMS);
    const result = predictor.reconcile(authoritative(initial, 0), missingWorld, PARAMS, {
      expectedCommandQuantumUs: QUANTUM_US,
      currentPhysicsRevision: 1,
      currentCollisionRevision: 7,
    });

    expect(result.missingCollision).toBe(true);
    expect(result.diagnostics.causeTags).toEqual(expect.arrayContaining([
      "replay_backlog",
      "command_dt_mismatch",
      "physics_revision_mismatch",
      "collision_revision_mismatch",
      "missing_collision",
    ]));
  });
});
