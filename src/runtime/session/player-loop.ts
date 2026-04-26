import { SectionPos } from "../../core/section-pos";
import type { ClientPlayerState, PlayerInputCommand, SessionChunkViewState } from "../protocol/world-messages";
import type { CollisionWorld } from "../movement/collision-world";
import {
  DEFAULT_MOVEMENT_PHYSICS,
  createStandingMovementBody,
  simulatePlayerMoveCommand,
  validatePlayerMoveCommand,
  type MovementBody,
  type PlayerMoveCommand,
} from "../movement";
import { AABB } from "../../world/phys/aabb";
import { Vec3 } from "../../world/phys/vec3";

export const PLAYER_TICK_INTERVAL_MS = 50;
export const PLAYER_COMMAND_QUANTUM_US = 8_333;
export const PLAYER_MOVEMENT_PHYSICS_REVISION = 1;
export const PLAYER_COLLISION_REVISION = 0;
export const PLAYER_MAX_COMMAND_STEP_COUNT = 8;
export const PLAYER_MAX_COMMANDS_PER_TICK = 16;
const DEFAULT_PLAYER_Y = 168.0;
const PLAYER_WIDTH = 0.6;
const PLAYER_HEIGHT = 1.8;
const PLAYER_MAX_COMMAND_QUANTUM_US = PLAYER_TICK_INTERVAL_MS * 1000;

export interface QueuedPlayerMoveCommand {
  readonly command: PlayerMoveCommand;
}

export interface PlayerCommandQueue {
  commands: QueuedPlayerMoveCommand[];
  lastQueuedInputSequence: number;
}

function clampAxis(value: number): number {
  return Math.max(-1.0, Math.min(1.0, value));
}

function clampPitch(value: number): number {
  return Math.max(-90.0, Math.min(90.0, value));
}

function normalizeDegrees(value: number): number {
  const normalized = value % 360.0;
  return normalized < 0.0 ? normalized + 360.0 : normalized;
}

function finiteOr(value: number | undefined, fallback: number): number {
  return value === undefined || !Number.isFinite(value) ? fallback : value;
}

function boundedSafeInteger(value: number | undefined, fallback: number, min: number, max: number): number {
  if (value === undefined || !Number.isFinite(value)) {
    return fallback;
  }

  const integer = Math.trunc(value);
  if (!Number.isSafeInteger(integer)) {
    return fallback;
  }

  return Math.max(min, Math.min(max, integer));
}

function movementBodyFromPlayerState(playerState: ClientPlayerState): MovementBody {
  const snapshot = playerState.movementBody;
  if (snapshot === undefined) {
    return createStandingMovementBody({
      position: new Vec3(playerState.position.x, playerState.position.y, playerState.position.z),
      width: PLAYER_WIDTH,
      height: PLAYER_HEIGHT,
    });
  }

  return {
    position: new Vec3(snapshot.position.x, snapshot.position.y, snapshot.position.z),
    velocity: new Vec3(snapshot.velocity.x, snapshot.velocity.y, snapshot.velocity.z),
    bounds: new AABB(
      snapshot.bounds.minX,
      snapshot.bounds.minY,
      snapshot.bounds.minZ,
      snapshot.bounds.maxX,
      snapshot.bounds.maxY,
      snapshot.bounds.maxZ,
    ),
    onGround: snapshot.onGround,
    mode: snapshot.mode,
    jumpHeld: snapshot.jumpHeld,
  };
}

function createMovementBodySnapshot(
  body: MovementBody,
  lastProcessedCommandSequence: number,
  commandQuantumUs = PLAYER_COMMAND_QUANTUM_US,
): NonNullable<ClientPlayerState["movementBody"]> {
  return {
    position: {
      x: body.position.x,
      y: body.position.y,
      z: body.position.z,
    },
    velocity: {
      x: body.velocity.x,
      y: body.velocity.y,
      z: body.velocity.z,
    },
    bounds: {
      minX: body.bounds.minX,
      minY: body.bounds.minY,
      minZ: body.bounds.minZ,
      maxX: body.bounds.maxX,
      maxY: body.bounds.maxY,
      maxZ: body.bounds.maxZ,
    },
    onGround: body.onGround,
    mode: body.mode,
    jumpHeld: body.jumpHeld,
    physicsRevision: PLAYER_MOVEMENT_PHYSICS_REVISION,
    collisionRevision: PLAYER_COLLISION_REVISION,
    commandQuantumUs,
    lastProcessedCommandSequence,
  };
}

export function createInitialPlayerState(playerId: string): ClientPlayerState {
  const body = createStandingMovementBody({
    position: new Vec3(8.5, DEFAULT_PLAYER_Y, 8.5),
    width: PLAYER_WIDTH,
    height: PLAYER_HEIGHT,
    onGround: true,
  });
  return {
    playerId,
    position: {
      x: body.position.x,
      y: body.position.y,
      z: body.position.z,
    },
    rotation: {
      yaw: 0.0,
      pitch: 0.0,
    },
    acknowledgedInputSequence: 0,
    tick: 0,
    revision: 0,
    movementBody: createMovementBodySnapshot(body, 0),
  };
}

export function anchorPlayerStateToChunkView(
  playerState: ClientPlayerState,
  chunkView: SessionChunkViewState,
  tick: number,
): ClientPlayerState {
  const previousBody = movementBodyFromPlayerState(playerState);
  const body = createStandingMovementBody({
    position: new Vec3(
      SectionPos.sectionToBlockCoord(chunkView.centerChunkX) + 8.5,
      playerState.position.y,
      SectionPos.sectionToBlockCoord(chunkView.centerChunkZ) + 8.5,
    ),
    velocity: Vec3.ZERO,
    width: PLAYER_WIDTH,
    height: PLAYER_HEIGHT,
    onGround: previousBody.onGround,
    mode: previousBody.mode,
    jumpHeld: previousBody.jumpHeld,
  });
  const position = {
    x: body.position.x,
    y: body.position.y,
    z: body.position.z,
  };
  if (
    playerState.position.x === position.x
    && playerState.position.y === position.y
    && playerState.position.z === position.z
  ) {
    return playerState;
  }

  return {
    ...playerState,
    position,
    movementBody: createMovementBodySnapshot(body, playerState.acknowledgedInputSequence),
    tick,
    revision: playerState.revision + 1,
  };
}

export function createPlayerCommandQueue(): PlayerCommandQueue {
  return {
    commands: [],
    lastQueuedInputSequence: 0,
  };
}

export function playerInputToQueuedMoveCommand(
  playerId: string,
  input: PlayerInputCommand,
): QueuedPlayerMoveCommand {
  const commandQuantumUs = boundedSafeInteger(
    input.commandQuantumUs,
    PLAYER_TICK_INTERVAL_MS * 1000,
    1,
    PLAYER_MAX_COMMAND_QUANTUM_US,
  );
  const stepCount = boundedSafeInteger(input.stepCount, 1, 1, PLAYER_MAX_COMMAND_STEP_COUNT);
  const command: PlayerMoveCommand = {
    type: "player_move_command",
    playerId,
    sequence: boundedSafeInteger(input.sequence, 1, 1, Number.MAX_SAFE_INTEGER),
    clientTimeUs: boundedSafeInteger(input.clientTimeUs, input.sequence * commandQuantumUs, 0, Number.MAX_SAFE_INTEGER),
    commandQuantumUs,
    stepCount,
    buttons: boundedSafeInteger(input.buttons, 0, 0, Number.MAX_SAFE_INTEGER),
    edgeButtons: boundedSafeInteger(input.edgeButtons, 0, 0, Number.MAX_SAFE_INTEGER),
    wishX: clampAxis(finiteOr(input.moveX, 0.0)),
    wishZ: clampAxis(finiteOr(input.moveZ, 0.0)),
    yaw: normalizeDegrees(finiteOr(input.yaw, 0.0)),
    pitch: clampPitch(finiteOr(input.pitch, 0.0)),
    physicsRevision: boundedSafeInteger(
      input.physicsRevision,
      PLAYER_MOVEMENT_PHYSICS_REVISION,
      1,
      Number.MAX_SAFE_INTEGER,
    ),
    collisionRevision: boundedSafeInteger(
      input.collisionRevision,
      PLAYER_COLLISION_REVISION,
      0,
      Number.MAX_SAFE_INTEGER,
    ),
  };
  validatePlayerMoveCommand(command);
  return {
    command,
  };
}

export function enqueuePlayerInputCommand(
  queue: PlayerCommandQueue,
  playerState: ClientPlayerState,
  input: PlayerInputCommand,
): boolean {
  if (
    !Number.isSafeInteger(input.sequence)
    || input.sequence <= playerState.acknowledgedInputSequence
    || input.sequence <= queue.lastQueuedInputSequence
  ) {
    return false;
  }

  queue.commands.push(playerInputToQueuedMoveCommand(playerState.playerId, input));
  queue.lastQueuedInputSequence = input.sequence;
  return true;
}

interface PlayerCommandTickResult {
  readonly playerState: ClientPlayerState;
  readonly processedCommandCount: number;
}

function tickPlayerStateFromCommandsInternal(
  playerState: ClientPlayerState,
  commands: readonly QueuedPlayerMoveCommand[],
  tick: number,
  world: CollisionWorld,
): PlayerCommandTickResult {
  if (commands.length === 0) {
    return { playerState, processedCommandCount: 0 };
  }

  let nextBody = movementBodyFromPlayerState(playerState);
  let nextRotation = playerState.rotation;
  let acknowledgedInputSequence = playerState.acknowledgedInputSequence;
  let movementQuantumUs = playerState.movementBody?.commandQuantumUs ?? PLAYER_COMMAND_QUANTUM_US;
  let processedCommandCount = 0;

  for (const queuedCommand of commands) {
    validatePlayerMoveCommand(queuedCommand.command);
    const commandResult = simulatePlayerMoveCommand(nextBody, queuedCommand.command, world, DEFAULT_MOVEMENT_PHYSICS);
    if (commandResult.missingCollision) {
      break;
    }
    nextBody = commandResult.body;
    nextRotation = {
      yaw: normalizeDegrees(queuedCommand.command.yaw),
      pitch: clampPitch(queuedCommand.command.pitch),
    };
    acknowledgedInputSequence = Math.max(acknowledgedInputSequence, queuedCommand.command.sequence);
    movementQuantumUs = queuedCommand.command.commandQuantumUs;
    processedCommandCount++;
  }

  if (processedCommandCount === 0) {
    return { playerState, processedCommandCount };
  }

  const nextPosition = {
    x: nextBody.position.x,
    y: nextBody.position.y,
    z: nextBody.position.z,
  };
  const movementBody = createMovementBodySnapshot(nextBody, acknowledgedInputSequence, movementQuantumUs);
  if (
    nextPosition.x === playerState.position.x
    && nextPosition.y === playerState.position.y
    && nextPosition.z === playerState.position.z
    && nextRotation.yaw === playerState.rotation.yaw
    && nextRotation.pitch === playerState.rotation.pitch
    && acknowledgedInputSequence === playerState.acknowledgedInputSequence
  ) {
    return { playerState, processedCommandCount };
  }

  return {
    playerState: {
      ...playerState,
      position: nextPosition,
      rotation: nextRotation,
      acknowledgedInputSequence,
      movementBody,
      tick,
      revision: playerState.revision + 1,
    },
    processedCommandCount,
  };
}

export function tickPlayerStateFromCommands(
  playerState: ClientPlayerState,
  commands: readonly QueuedPlayerMoveCommand[],
  tick: number,
  world: CollisionWorld,
): ClientPlayerState {
  return tickPlayerStateFromCommandsInternal(playerState, commands, tick, world).playerState;
}

export function tickPlayerStateWithCommandQueue(
  playerState: ClientPlayerState,
  queue: PlayerCommandQueue,
  tick: number,
  world: CollisionWorld,
  maxCommands = PLAYER_MAX_COMMANDS_PER_TICK,
): ClientPlayerState {
  const commandCount = boundedSafeInteger(maxCommands, PLAYER_MAX_COMMANDS_PER_TICK, 1, PLAYER_MAX_COMMANDS_PER_TICK);
  const commands = queue.commands.slice(0, commandCount);
  const result = tickPlayerStateFromCommandsInternal(playerState, commands, tick, world);
  if (result.processedCommandCount > 0) {
    queue.commands.splice(0, result.processedCommandCount);
  }
  return result.playerState;
}

export function tickPlayerState(
  playerState: ClientPlayerState,
  input: PlayerInputCommand | undefined,
  tick: number,
  world: CollisionWorld,
): ClientPlayerState {
  if (input === undefined) {
    return playerState;
  }

  return tickPlayerStateFromCommands(playerState, [playerInputToQueuedMoveCommand(playerState.playerId, input)], tick, world);
}
