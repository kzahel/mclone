import { SectionPos } from "../../core/section-pos";
import type { PlayerInputCommand, SetChunkViewRequest } from "../../runtime/protocol/world-messages";
import type { ClientPlayerState } from "../../runtime/protocol/world-messages";
import { Vec3 } from "../../world/phys/vec3";
import type { DebugInputFrame } from "./debug-input";

const MOUSE_SENS_DEG_PER_PIXEL = 0.15;
const JOYSTICK_LOOK_DEG_PER_SEC = 120.0;
const MAX_PITCH = 89.0;

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
  return {
    position: new Vec3(
      playerState.position.x,
      playerState.position.y,
      playerState.position.z,
    ),
    xRot: playerState.rotation.pitch,
    yRot: playerState.rotation.yaw,
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
  playerState: ClientPlayerState,
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

  const yaw = playerState.rotation.yaw + yawDelta;
  const pitch = clampPitch(playerState.rotation.pitch + pitchDelta);
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
    && left.pitch === right.pitch;
}
