import { SectionPos } from "../../core/section-pos";
import { MOVEMENT_COMMAND_BUTTONS, type MovementBody } from "../../runtime/movement";
import type { PlayerInputCommand, SetChunkViewRequest } from "../../runtime/protocol/world-messages";
import type { ClientPlayerState } from "../../runtime/protocol/world-messages";
import { Vec3 } from "../../world/phys/vec3";
import type { DebugInputFrame } from "./debug-input";

const MOUSE_SENS_DEG_PER_PIXEL = 0.15;
const JOYSTICK_LOOK_DEG_PER_SEC = 120.0;
const MAX_PITCH = 89.0;
const PLAYER_EYE_HEIGHT = 1.62;

export interface DebugCameraState {
  readonly position: Vec3;
  readonly xRot: number;
  readonly yRot: number;
}

export interface DebugInjectedInput {
  readonly heldKeys?: readonly string[];
  readonly mouseDeltaX?: number;
  readonly mouseDeltaY?: number;
  readonly locked?: boolean;
  readonly joystickX?: number;
  readonly joystickY?: number;
  readonly moveForward?: boolean;
  readonly moveBack?: boolean;
  readonly flyUp?: boolean;
  readonly flyDown?: boolean;
}

function clampPitch(value: number): number {
  return Math.max(-MAX_PITCH, Math.min(MAX_PITCH, value));
}

export function buildCameraDeltaWorld(
  yawDeg: number,
  pitchDeg: number,
  forwardAxis: number,
  rightAxis: number,
  upAxis: number,
): { readonly x: number; readonly y: number; readonly z: number } {
  const yawRad = (yawDeg * Math.PI) / 180.0;
  const pitchRad = (pitchDeg * Math.PI) / 180.0;
  const sinYaw = Math.sin(yawRad);
  const cosYaw = Math.cos(yawRad);
  const sinPitch = Math.sin(pitchRad);
  const cosPitch = Math.cos(pitchRad);

  const forwardX = -sinYaw * cosPitch;
  const forwardY = -sinPitch;
  const forwardZ = cosYaw * cosPitch;
  const rightX = -cosYaw;
  const rightZ = -sinYaw;

  return {
    x: forwardX * forwardAxis + rightX * rightAxis,
    y: forwardY * forwardAxis + upAxis,
    z: forwardZ * forwardAxis + rightZ * rightAxis,
  };
}

export function mergeDebugInputFrame(frame: DebugInputFrame, injectedInput: DebugInjectedInput | null): DebugInputFrame {
  if (injectedInput === null) {
    return frame;
  }

  return {
    heldKeys: new Set<string>([
      ...frame.heldKeys,
      ...(injectedInput.heldKeys ?? []),
    ]),
    mouseDeltaX: frame.mouseDeltaX + (injectedInput.mouseDeltaX ?? 0),
    mouseDeltaY: frame.mouseDeltaY + (injectedInput.mouseDeltaY ?? 0),
    locked: injectedInput.locked ?? frame.locked,
    joystickX: injectedInput.joystickX ?? frame.joystickX,
    joystickY: injectedInput.joystickY ?? frame.joystickY,
    moveForward: frame.moveForward || (injectedInput.moveForward ?? false),
    moveBack: frame.moveBack || (injectedInput.moveBack ?? false),
    flyUp: frame.flyUp || (injectedInput.flyUp ?? false),
    flyDown: frame.flyDown || (injectedInput.flyDown ?? false),
  };
}

export function createCameraStateFromPlayerState(playerState: ClientPlayerState): DebugCameraState {
  const position = playerState.movementBody?.position ?? playerState.position;
  return {
    position: new Vec3(
      position.x,
      position.y + PLAYER_EYE_HEIGHT,
      position.z,
    ),
    xRot: playerState.rotation.pitch,
    yRot: playerState.rotation.yaw,
  };
}

export function createCameraStateFromMovementBody(
  body: MovementBody,
  yaw: number,
  pitch: number,
): DebugCameraState {
  return {
    position: new Vec3(body.position.x, body.position.y + PLAYER_EYE_HEIGHT, body.position.z),
    xRot: pitch,
    yRot: yaw,
  };
}

export function reconcilePredictedCameraState(
  camera: DebugCameraState,
  playerState: ClientPlayerState,
  lastInputCommand: PlayerInputCommand | undefined,
): DebugCameraState {
  if (lastInputCommand !== undefined && playerState.acknowledgedInputSequence < lastInputCommand.sequence) {
    return camera;
  }

  return createCameraStateFromPlayerState(playerState);
}

export function applyPredictedCameraInput(
  camera: DebugCameraState,
  input: PlayerInputCommand,
  dtSeconds: number,
): DebugCameraState {
  const fixedCommandSeconds = input.commandQuantumUs !== undefined && input.stepCount !== undefined
    ? (input.commandQuantumUs * input.stepCount) / 1_000_000.0
    : dtSeconds;
  const move = buildCameraDeltaWorld(input.yaw, input.pitch, input.moveZ, input.moveX, input.moveY);
  const moveMagnitude = Math.hypot(move.x, move.y, move.z);
  const speedBlocksPerSecond = 8.0;
  const moveScale = moveMagnitude > 1.0
    ? (speedBlocksPerSecond * fixedCommandSeconds) / moveMagnitude
    : speedBlocksPerSecond * fixedCommandSeconds;

  return {
    position: new Vec3(
      camera.position.x + move.x * moveScale,
      camera.position.y + move.y * moveScale,
      camera.position.z + move.z * moveScale,
    ),
    xRot: input.pitch,
    yRot: input.yaw,
  };
}

export function getDebugPlayerButtonMask(frame: DebugInputFrame): number {
  let buttons = 0;
  if (frame.heldKeys.has("Space") || frame.flyUp) {
    buttons |= MOVEMENT_COMMAND_BUTTONS.JUMP;
  }
  if (frame.heldKeys.has("ShiftLeft") || frame.heldKeys.has("ShiftRight") || frame.flyDown) {
    buttons |= MOVEMENT_COMMAND_BUTTONS.CROUCH;
  }
  if (frame.heldKeys.has("ControlLeft") || frame.heldKeys.has("ControlRight")) {
    buttons |= MOVEMENT_COMMAND_BUTTONS.SPRINT;
  }
  return buttons;
}

export function createChunkViewRequestForCameraState(camera: DebugCameraState, radius: number): SetChunkViewRequest {
  return {
    type: "set_chunk_view",
    centerChunkX: SectionPos.posToSectionCoord(camera.position.x),
    centerChunkZ: SectionPos.posToSectionCoord(camera.position.z),
    radius,
  };
}

export function createChunkViewRequestForPlayerState(playerState: ClientPlayerState, radius: number): SetChunkViewRequest {
  return {
    type: "set_chunk_view",
    centerChunkX: SectionPos.posToSectionCoord(playerState.position.x),
    centerChunkZ: SectionPos.posToSectionCoord(playerState.position.z),
    radius,
  };
}

export function buildPlayerInputCommand(
  baseYaw: number,
  basePitch: number,
  frame: DebugInputFrame,
  dtSeconds: number,
  sequence: number,
  options: Partial<Pick<
    PlayerInputCommand,
    | "clientTimeUs"
    | "commandQuantumUs"
    | "stepCount"
    | "buttons"
    | "edgeButtons"
    | "physicsRevision"
    | "collisionRevision"
  >> = {},
): PlayerInputCommand {
  const yawDelta =
    frame.mouseDeltaX * MOUSE_SENS_DEG_PER_PIXEL
    + frame.joystickX * JOYSTICK_LOOK_DEG_PER_SEC * dtSeconds;
  const pitchDelta =
    frame.mouseDeltaY * MOUSE_SENS_DEG_PER_PIXEL
    - frame.joystickY * JOYSTICK_LOOK_DEG_PER_SEC * dtSeconds;

  const yaw = baseYaw + yawDelta;
  const pitch = clampPitch(basePitch + pitchDelta);
  const forwardAxis =
    (frame.heldKeys.has("KeyW") || frame.moveForward ? 1 : 0)
    - (frame.heldKeys.has("KeyS") || frame.moveBack ? 1 : 0);
  const strafeAxis =
    (frame.heldKeys.has("KeyA") ? 1 : 0)
    - (frame.heldKeys.has("KeyD") ? 1 : 0);

  return {
    sequence,
    moveX: strafeAxis,
    moveY: 0,
    moveZ: forwardAxis,
    yaw,
    pitch,
    buttons: getDebugPlayerButtonMask(frame),
    edgeButtons: 0,
    ...options,
  };
}

export function buildFreeCameraInputCommand(
  baseYaw: number,
  basePitch: number,
  frame: DebugInputFrame,
  dtSeconds: number,
  sequence: number,
): PlayerInputCommand {
  const yawDelta =
    frame.mouseDeltaX * MOUSE_SENS_DEG_PER_PIXEL
    + frame.joystickX * JOYSTICK_LOOK_DEG_PER_SEC * dtSeconds;
  const pitchDelta =
    frame.mouseDeltaY * MOUSE_SENS_DEG_PER_PIXEL
    - frame.joystickY * JOYSTICK_LOOK_DEG_PER_SEC * dtSeconds;
  const yaw = baseYaw + yawDelta;
  const pitch = clampPitch(basePitch + pitchDelta);
  const forwardAxis =
    (frame.heldKeys.has("KeyW") || frame.moveForward ? 1 : 0)
    - (frame.heldKeys.has("KeyS") || frame.moveBack ? 1 : 0);
  const rightAxis =
    (frame.heldKeys.has("KeyD") ? 1 : 0)
    - (frame.heldKeys.has("KeyA") ? 1 : 0);
  const upAxis =
    (frame.heldKeys.has("Space") || frame.flyUp ? 1 : 0)
    - (frame.heldKeys.has("ShiftLeft") || frame.heldKeys.has("ShiftRight") || frame.flyDown ? 1 : 0);
  const move = buildCameraDeltaWorld(yaw, pitch, forwardAxis, rightAxis, upAxis);
  return {
    sequence,
    moveX: move.x,
    moveY: move.y,
    moveZ: move.z,
    yaw,
    pitch,
  };
}

export function isSamePlayerInput(
  left: PlayerInputCommand | undefined,
  right: PlayerInputCommand,
): boolean {
  if (left === undefined) {
    return false;
  }

  return left.moveX === right.moveX
    && left.moveY === right.moveY
    && left.moveZ === right.moveZ
    && left.yaw === right.yaw
    && left.pitch === right.pitch
    && (left.buttons ?? 0) === (right.buttons ?? 0)
    && (left.edgeButtons ?? 0) === (right.edgeButtons ?? 0);
}
