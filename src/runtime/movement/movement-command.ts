import type { CollisionWorld } from "./collision-world";
import type { MovementBody, MovementStepResult } from "./movement-body";
import type { MovementIntent } from "./movement-intent";
import type { MovementPhysicsParams } from "./movement-params";
import { simulateMovementStep } from "./movement-step";

export const MOVEMENT_COMMAND_BUTTONS = {
  JUMP: 1 << 0,
  CROUCH: 1 << 1,
  SPRINT: 1 << 2,
} as const;

export interface PlayerMoveCommand {
  readonly type: "player_move_command";
  readonly playerId: string;
  readonly sequence: number;
  readonly clientTimeUs: number;
  readonly commandQuantumUs: number;
  readonly stepCount: number;
  readonly buttons: number;
  readonly edgeButtons: number;
  readonly wishX: number;
  readonly wishZ: number;
  readonly yaw: number;
  readonly pitch: number;
  readonly physicsRevision: number;
  readonly collisionRevision?: number;
}

export interface MovementCommandSimulationResult {
  readonly body: MovementBody;
  readonly stepResults: readonly MovementStepResult[];
  readonly lastProcessedCommandSeq: number;
  readonly missingCollision: boolean;
}

export function movementCommandToIntent(command: PlayerMoveCommand): MovementIntent {
  return {
    wishX: command.wishX,
    wishZ: command.wishZ,
    jump: (command.buttons & MOVEMENT_COMMAND_BUTTONS.JUMP) !== 0 || (command.edgeButtons & MOVEMENT_COMMAND_BUTTONS.JUMP) !== 0,
    crouch: (command.buttons & MOVEMENT_COMMAND_BUTTONS.CROUCH) !== 0,
    sprint: (command.buttons & MOVEMENT_COMMAND_BUTTONS.SPRINT) !== 0,
    yaw: command.yaw,
    pitch: command.pitch,
  };
}

export function validatePlayerMoveCommand(command: PlayerMoveCommand): void {
  if (command.type !== "player_move_command") {
    throw new Error(`invalid player move command type: ${String(command.type)}`);
  }
  if (!Number.isSafeInteger(command.sequence) || command.sequence <= 0) {
    throw new Error(`command sequence must be a positive safe integer, got ${command.sequence}`);
  }
  if (!Number.isSafeInteger(command.commandQuantumUs) || command.commandQuantumUs <= 0) {
    throw new Error(`commandQuantumUs must be a positive safe integer, got ${command.commandQuantumUs}`);
  }
  if (!Number.isSafeInteger(command.stepCount) || command.stepCount <= 0) {
    throw new Error(`stepCount must be a positive safe integer, got ${command.stepCount}`);
  }
  for (const [name, value] of [
    ["clientTimeUs", command.clientTimeUs],
    ["buttons", command.buttons],
    ["edgeButtons", command.edgeButtons],
    ["wishX", command.wishX],
    ["wishZ", command.wishZ],
    ["yaw", command.yaw],
    ["pitch", command.pitch],
    ["physicsRevision", command.physicsRevision],
  ] as const) {
    if (!Number.isFinite(value)) {
      throw new Error(`${name} must be finite, got ${value}`);
    }
  }
}

export function simulatePlayerMoveCommand(
  body: MovementBody,
  command: PlayerMoveCommand,
  world: CollisionWorld,
  params: MovementPhysicsParams,
): MovementCommandSimulationResult {
  validatePlayerMoveCommand(command);
  const fixedDtSeconds = command.commandQuantumUs / 1_000_000.0;
  const intent = movementCommandToIntent(command);
  const stepResults: MovementStepResult[] = [];
  let currentBody = body;
  let missingCollision = false;

  for (let step = 0; step < command.stepCount; step++) {
    const result = simulateMovementStep(currentBody, intent, world, fixedDtSeconds, params);
    stepResults.push(result);
    currentBody = result.body;
    if (result.collision.missingCollision) {
      missingCollision = true;
      break;
    }
  }

  return {
    body: currentBody,
    stepResults,
    lastProcessedCommandSeq: command.sequence,
    missingCollision,
  };
}

export function simulatePlayerMoveCommands(
  body: MovementBody,
  commands: readonly PlayerMoveCommand[],
  world: CollisionWorld,
  params: MovementPhysicsParams,
): MovementCommandSimulationResult {
  const stepResults: MovementStepResult[] = [];
  let currentBody = body;
  let lastProcessedCommandSeq = 0;
  let previousSequence = 0;
  let missingCollision = false;

  for (const command of commands) {
    if (command.sequence <= previousSequence) {
      throw new Error(`commands must be strictly ordered by sequence: ${command.sequence} after ${previousSequence}`);
    }

    const result = simulatePlayerMoveCommand(currentBody, command, world, params);
    stepResults.push(...result.stepResults);
    currentBody = result.body;
    lastProcessedCommandSeq = result.lastProcessedCommandSeq;
    previousSequence = command.sequence;
    if (result.missingCollision) {
      missingCollision = true;
      break;
    }
  }

  return {
    body: currentBody,
    stepResults,
    lastProcessedCommandSeq,
    missingCollision,
  };
}
