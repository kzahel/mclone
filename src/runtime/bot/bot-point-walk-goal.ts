import type { ClientPlayerState } from "../protocol/world-messages";
import { steerTowardWaypoint } from "./bot-navigation";
import { queryBotStandableSurface, type BotStandableSurface } from "./bot-observation";
import type {
  BotController,
  BotControllerTickContext,
  BotControllerTickResult,
  BotMovementIntent,
} from "./bot-runtime";

export interface BotWalkPoint {
  readonly x: number;
  readonly z: number;
}

export interface WalkToPointBotControllerOptions {
  readonly target?: BotWalkPoint;
  readonly randomSeed?: number;
  readonly minDistance?: number;
  readonly maxDistance?: number;
  readonly arrivalRadius?: number;
  readonly moveZ?: number;
}

export interface WalkToPointBotSnapshot {
  readonly target?: BotWalkPoint;
  readonly arrived: boolean;
  readonly status: string;
}

const DEFAULT_RANDOM_SEED = 0x51f15e;
const DEFAULT_MIN_DISTANCE = 4.0;
const DEFAULT_MAX_DISTANCE = 7.0;
const DEFAULT_ARRIVAL_RADIUS = 0.65;
const DEFAULT_MOVE_Z = 1.0;

function normalizeRandomSeed(seed: number | undefined): number {
  return (seed !== undefined && Number.isSafeInteger(seed) ? seed : DEFAULT_RANDOM_SEED) >>> 0;
}

function randomUnit(seed: number): number {
  let value = seed >>> 0;
  value ^= value << 13;
  value ^= value >>> 17;
  value ^= value << 5;
  return (value >>> 0) / 0xffff_ffff;
}

function playerPosition(playerState: ClientPlayerState): { readonly x: number; readonly y: number; readonly z: number } {
  return playerState.movementBody?.position ?? playerState.position;
}

function stopIntent(playerState: ClientPlayerState | undefined): BotMovementIntent {
  return {
    moveX: 0.0,
    moveZ: 0.0,
    yaw: playerState?.rotation.yaw ?? 0.0,
    pitch: playerState?.rotation.pitch ?? 0.0,
    buttons: 0,
    edgeButtons: 0,
  };
}

function distanceToTarget(playerState: ClientPlayerState, target: BotWalkPoint): number {
  const position = playerPosition(playerState);
  return Math.hypot(target.x - position.x, target.z - position.z);
}

function currentStandableSurface(context: BotControllerTickContext): BotStandableSurface | undefined {
  const playerState = context.observation.playerState;
  if (playerState === undefined) {
    return undefined;
  }

  const position = playerPosition(playerState);
  const query = queryBotStandableSurface(context.clientWorld, {
    x: Math.floor(position.x),
    y: Math.floor(position.y + 1.0e-5),
    z: Math.floor(position.z),
  });
  return query.type === "loaded" ? query.surface : undefined;
}

function chooseRandomTarget(playerState: ClientPlayerState, options: WalkToPointBotControllerOptions): BotWalkPoint {
  const position = playerPosition(playerState);
  const seed = normalizeRandomSeed(options.randomSeed);
  const angle = randomUnit(seed) * Math.PI * 2.0;
  const minDistance = options.minDistance ?? DEFAULT_MIN_DISTANCE;
  const maxDistance = Math.max(minDistance, options.maxDistance ?? DEFAULT_MAX_DISTANCE);
  const distance = minDistance + (randomUnit(seed ^ 0x9e3779b9) * (maxDistance - minDistance));
  return {
    x: position.x + Math.cos(angle) * distance,
    z: position.z + Math.sin(angle) * distance,
  };
}

export class WalkToPointBotController implements BotController {
  private target: BotWalkPoint | undefined;
  private arrived = false;
  private lastStatus = "walk-to-point waiting";

  public constructor(private readonly options: WalkToPointBotControllerOptions = {}) {}

  public tick(context: BotControllerTickContext): BotControllerTickResult {
    const playerState = context.observation.playerState;
    if (playerState === undefined) {
      return this.withStatus({
        movement: stopIntent(undefined),
        status: "walk-to-point waiting for player state",
      });
    }

    const surface = currentStandableSurface(context);
    if (surface === undefined && this.target === undefined) {
      return this.withStatus({
        movement: stopIntent(playerState),
        status: "walk-to-point waiting for standable surface",
      });
    }

    this.target ??= this.options.target ?? chooseRandomTarget(playerState, this.options);
    const arrivalRadius = this.options.arrivalRadius ?? DEFAULT_ARRIVAL_RADIUS;
    if (distanceToTarget(playerState, this.target) <= arrivalRadius) {
      this.arrived = true;
      return this.withStatus({
        movement: stopIntent(playerState),
        status: `walk-to-point arrived x=${this.target.x.toFixed(2)} z=${this.target.z.toFixed(2)}`,
      });
    }

    const waypoint = {
      x: Math.floor(this.target.x),
      y: surface?.y ?? Math.floor(playerPosition(playerState).y),
      z: Math.floor(this.target.z),
      supportY: (surface?.y ?? Math.floor(playerPosition(playerState).y)) - 1,
    };
    return this.withStatus({
      movement: steerTowardWaypoint(playerState, waypoint, {
        arrivalRadius,
        moveZ: this.options.moveZ ?? DEFAULT_MOVE_Z,
      }),
      status: `walk-to-point moving x=${this.target.x.toFixed(2)} z=${this.target.z.toFixed(2)}`,
    });
  }

  public getSnapshot(): WalkToPointBotSnapshot {
    return {
      target: this.target,
      arrived: this.arrived,
      status: this.lastStatus,
    };
  }

  private withStatus(result: BotControllerTickResult): BotControllerTickResult {
    this.lastStatus = result.status ?? this.lastStatus;
    return result;
  }
}
