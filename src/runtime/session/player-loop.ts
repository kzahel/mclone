import { SectionPos } from "../../core/section-pos";
import type { ClientPlayerState, PlayerInputCommand, SessionChunkViewState } from "../protocol/world-messages";

export const PLAYER_TICK_INTERVAL_MS = 50;
export const PLAYER_MOVE_SPEED_BLOCKS_PER_TICK = 0.4;
export const PLAYER_MOVE_SPEED_BLOCKS_PER_SECOND =
  PLAYER_MOVE_SPEED_BLOCKS_PER_TICK * (1000.0 / PLAYER_TICK_INTERVAL_MS);
const DEFAULT_PLAYER_Y = 104.0;

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

export function createInitialPlayerState(playerId: string): ClientPlayerState {
  return {
    playerId,
    position: {
      x: 8.5,
      y: DEFAULT_PLAYER_Y,
      z: 8.5,
    },
    rotation: {
      yaw: 0.0,
      pitch: 0.0,
    },
    acknowledgedInputSequence: 0,
    tick: 0,
    revision: 0,
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
    tick,
    revision: playerState.revision + 1,
  };
}

export function tickPlayerState(
  playerState: ClientPlayerState,
  input: PlayerInputCommand | undefined,
  tick: number,
): ClientPlayerState {
  const moveX = clampAxis(input?.moveX ?? 0.0);
  const moveY = clampAxis(input?.moveY ?? 0.0);
  const moveZ = clampAxis(input?.moveZ ?? 0.0);
  const magnitude = Math.hypot(moveX, moveY, moveZ);
  const scale = magnitude > 1.0 ? PLAYER_MOVE_SPEED_BLOCKS_PER_TICK / magnitude : PLAYER_MOVE_SPEED_BLOCKS_PER_TICK;
  const nextPosition = {
    x: playerState.position.x + moveX * scale,
    y: playerState.position.y + moveY * scale,
    z: playerState.position.z + moveZ * scale,
  };
  const nextRotation = {
    yaw: normalizeDegrees(input?.yaw ?? playerState.rotation.yaw),
    pitch: clampPitch(input?.pitch ?? playerState.rotation.pitch),
  };
  const acknowledgedInputSequence = Math.max(playerState.acknowledgedInputSequence, input?.sequence ?? 0);

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
    tick,
    revision: playerState.revision + 1,
  };
}
