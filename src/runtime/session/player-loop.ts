import { SectionPos } from "../../core/section-pos";
import type { ClientPlayerState, PlayerInputCommand, SessionChunkViewState } from "../protocol/world-messages";
import { validatePlayerMoveCommand, type PlayerMoveCommand } from "../movement/movement-command";

export const PLAYER_TICK_INTERVAL_MS = 50;
export const PLAYER_COMMAND_QUANTUM_US = 8_333;
export const PLAYER_MOVE_SPEED_BLOCKS_PER_TICK = 0.4;
export const PLAYER_MOVE_SPEED_BLOCKS_PER_SECOND =
  PLAYER_MOVE_SPEED_BLOCKS_PER_TICK * (1000.0 / PLAYER_TICK_INTERVAL_MS);
export const PLAYER_MOVEMENT_PHYSICS_REVISION = 1;
export const PLAYER_COLLISION_REVISION = 0;
export const PLAYER_MAX_COMMAND_STEP_COUNT = 8;
export const PLAYER_MAX_COMMANDS_PER_TICK = 16;
const DEFAULT_PLAYER_Y = 104.0;
const PLAYER_WIDTH = 0.6;
const PLAYER_HEIGHT = 1.8;
const PLAYER_MAX_COMMAND_QUANTUM_US = PLAYER_TICK_INTERVAL_MS * 1000;

export interface QueuedPlayerMoveCommand {
  readonly command: PlayerMoveCommand;
  readonly legacyMoveY: number;
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

function createMovementBodySnapshot(
  position: ClientPlayerState["position"],
  velocity: NonNullable<ClientPlayerState["movementBody"]>["velocity"],
  lastProcessedCommandSequence: number,
  commandQuantumUs = PLAYER_COMMAND_QUANTUM_US,
): NonNullable<ClientPlayerState["movementBody"]> {
  const halfWidth = PLAYER_WIDTH / 2.0;
  return {
    position,
    velocity,
    bounds: {
      minX: position.x - halfWidth,
      minY: position.y,
      minZ: position.z - halfWidth,
      maxX: position.x + halfWidth,
      maxY: position.y + PLAYER_HEIGHT,
      maxZ: position.z + halfWidth,
    },
    onGround: false,
    mode: "flying",
    jumpHeld: false,
    physicsRevision: PLAYER_MOVEMENT_PHYSICS_REVISION,
    collisionRevision: PLAYER_COLLISION_REVISION,
    commandQuantumUs,
    lastProcessedCommandSequence,
  };
}

export function createInitialPlayerState(playerId: string): ClientPlayerState {
  const position = {
    x: 8.5,
    y: DEFAULT_PLAYER_Y,
    z: 8.5,
  };
  return {
    playerId,
    position,
    rotation: {
      yaw: 0.0,
      pitch: 0.0,
    },
    acknowledgedInputSequence: 0,
    tick: 0,
    revision: 0,
    movementBody: createMovementBodySnapshot(position, { x: 0.0, y: 0.0, z: 0.0 }, 0),
  };
}

export function anchorPlayerStateToChunkView(
  playerState: ClientPlayerState,
  chunkView: SessionChunkViewState,
  tick: number,
): ClientPlayerState {
  const position = {
    x: SectionPos.sectionToBlockCoord(chunkView.centerChunkX) + 8.5,
    y: playerState.position.y,
    z: SectionPos.sectionToBlockCoord(chunkView.centerChunkZ) + 8.5,
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
    movementBody: createMovementBodySnapshot(
      position,
      { x: 0.0, y: 0.0, z: 0.0 },
      playerState.acknowledgedInputSequence,
    ),
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
    legacyMoveY: clampAxis(finiteOr(input.moveY, 0.0)),
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

function nextPositionForCommandStep(
  position: ClientPlayerState["position"],
  command: QueuedPlayerMoveCommand,
): {
  readonly position: ClientPlayerState["position"];
  readonly velocity: NonNullable<ClientPlayerState["movementBody"]>["velocity"];
} {
  const moveX = clampAxis(command.command.wishX);
  const moveY = clampAxis(command.legacyMoveY);
  const moveZ = clampAxis(command.command.wishZ);
  const magnitude = Math.hypot(moveX, moveY, moveZ);
  const speedScale = magnitude > 1.0 ? PLAYER_MOVE_SPEED_BLOCKS_PER_SECOND / magnitude : PLAYER_MOVE_SPEED_BLOCKS_PER_SECOND;
  const velocity = {
    x: moveX * speedScale,
    y: moveY * speedScale,
    z: moveZ * speedScale,
  };
  const dtSeconds = command.command.commandQuantumUs / 1_000_000.0;
  return {
    position: {
      x: position.x + velocity.x * dtSeconds,
      y: position.y + velocity.y * dtSeconds,
      z: position.z + velocity.z * dtSeconds,
    },
    velocity,
  };
}

export function tickPlayerStateFromCommands(
  playerState: ClientPlayerState,
  commands: readonly QueuedPlayerMoveCommand[],
  tick: number,
): ClientPlayerState {
  if (commands.length === 0) {
    return playerState;
  }

  let nextPosition = playerState.position;
  let nextVelocity: NonNullable<ClientPlayerState["movementBody"]>["velocity"] = { x: 0.0, y: 0.0, z: 0.0 };
  let nextRotation = playerState.rotation;
  let acknowledgedInputSequence = playerState.acknowledgedInputSequence;
  let movementQuantumUs = playerState.movementBody?.commandQuantumUs ?? PLAYER_COMMAND_QUANTUM_US;

  for (const queuedCommand of commands) {
    validatePlayerMoveCommand(queuedCommand.command);
    for (let step = 0; step < queuedCommand.command.stepCount; step++) {
      const stepResult = nextPositionForCommandStep(nextPosition, queuedCommand);
      nextPosition = stepResult.position;
      nextVelocity = stepResult.velocity;
    }
    nextRotation = {
      yaw: normalizeDegrees(queuedCommand.command.yaw),
      pitch: clampPitch(queuedCommand.command.pitch),
    };
    acknowledgedInputSequence = Math.max(acknowledgedInputSequence, queuedCommand.command.sequence);
    movementQuantumUs = queuedCommand.command.commandQuantumUs;
  }

  const movementBody = createMovementBodySnapshot(nextPosition, nextVelocity, acknowledgedInputSequence, movementQuantumUs);
  if (
    nextPosition.x === playerState.position.x
    && nextPosition.y === playerState.position.y
    && nextPosition.z === playerState.position.z
    && nextRotation.yaw === playerState.rotation.yaw
    && nextRotation.pitch === playerState.rotation.pitch
    && acknowledgedInputSequence === playerState.acknowledgedInputSequence
  ) {
    return playerState;
  }

  return {
    ...playerState,
    position: nextPosition,
    rotation: nextRotation,
    acknowledgedInputSequence,
    movementBody,
    tick,
    revision: playerState.revision + 1,
  };
}

export function tickPlayerStateWithCommandQueue(
  playerState: ClientPlayerState,
  queue: PlayerCommandQueue,
  tick: number,
  maxCommands = PLAYER_MAX_COMMANDS_PER_TICK,
): ClientPlayerState {
  const commandCount = boundedSafeInteger(maxCommands, PLAYER_MAX_COMMANDS_PER_TICK, 1, PLAYER_MAX_COMMANDS_PER_TICK);
  const commands = queue.commands.splice(0, commandCount);
  return tickPlayerStateFromCommands(playerState, commands, tick);
}

export function tickPlayerState(
  playerState: ClientPlayerState,
  input: PlayerInputCommand | undefined,
  tick: number,
): ClientPlayerState {
  if (input === undefined) {
    return playerState;
  }

  return tickPlayerStateFromCommands(playerState, [playerInputToQueuedMoveCommand(playerState.playerId, input)], tick);
}
