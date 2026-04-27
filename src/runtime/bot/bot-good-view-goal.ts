import { BlockPos } from "../../core/block-pos";
import type { ClientWorld } from "../client/client-world";
import type { ClientPlayerState } from "../protocol/world-messages";
import {
  planPathBetweenSurfaces,
  steerTowardWaypoint,
  type BotPathPlannerOptions,
} from "./bot-navigation";
import {
  BotSpatialIndex,
  queryBotBlock,
  queryBotStandableSurface,
  type BotMissingChunk,
  type BotStandableSurface,
} from "./bot-observation";
import type {
  BotController,
  BotControllerTickContext,
  BotControllerTickResult,
  BotGoal,
  BotGoalEvaluation,
  BotMovementIntent,
} from "./bot-runtime";

export interface GoodViewGoalOptions {
  readonly scanRadius?: number;
  readonly verticalScanBelow?: number;
  readonly verticalScanAbove?: number;
  readonly maxCandidateCount?: number;
  readonly horizonRayLength?: number;
  readonly opennessRadius?: number;
  readonly rescanIntervalMs?: number;
  readonly targetSwitchMinScoreGain?: number;
  readonly waypointArrivalRadius?: number;
  readonly targetArrivalRadius?: number;
  readonly stuckTimeoutMs?: number;
  readonly progressEpsilon?: number;
  readonly moveZ?: number;
  readonly lookYawDegreesPerSecond?: number;
  readonly pathPlanner?: BotPathPlannerOptions;
}

export interface GoodViewSurfaceScore {
  readonly surface: BotStandableSurface;
  readonly score: number;
  readonly heightScore: number;
  readonly opennessScore: number;
  readonly horizonScore: number;
  readonly distanceCost: number;
  readonly pathCost: number;
  readonly pathLength?: number;
}

export interface GoodViewTarget {
  readonly surface: BotStandableSurface;
  readonly score: GoodViewSurfaceScore;
  readonly path: readonly BotStandableSurface[];
  readonly candidateCount: number;
  readonly selectedAtMs: number;
  readonly revisionKey: string;
}

export interface GoodViewGoalSnapshot {
  readonly target?: GoodViewTarget;
  readonly pathIndex: number;
  readonly status: string;
}

export interface GoodViewLoadedSurfaceScoreResult {
  readonly type: "loaded";
  readonly score: GoodViewSurfaceScore;
}

export interface GoodViewBlockedSurfaceScoreResult {
  readonly type: "blocked";
  readonly reason: "blocked_candidate" | "obstructed_sample";
}

export interface GoodViewMissingSurfaceScoreResult {
  readonly type: "missing";
  readonly missing: readonly BotMissingChunk[];
}

export type GoodViewSurfaceScoreResult =
  | GoodViewLoadedSurfaceScoreResult
  | GoodViewBlockedSurfaceScoreResult
  | GoodViewMissingSurfaceScoreResult;

export interface GoodViewSelectedTargetResult {
  readonly type: "selected";
  readonly target: GoodViewTarget;
  readonly currentSurface: BotStandableSurface;
  readonly missing: readonly BotMissingChunk[];
}

export interface GoodViewWaitTargetResult {
  readonly type: "wait";
  readonly status: string;
  readonly reason: "missing_player" | "missing_current_surface" | "missing_visibility" | "missing_path" | "no_loaded_surfaces";
  readonly missing: readonly BotMissingChunk[];
}

export interface GoodViewBlockedTargetResult {
  readonly type: "blocked";
  readonly status: string;
  readonly reason: "blocked_current_surface" | "no_reachable_target";
  readonly candidateCount: number;
  readonly missing: readonly BotMissingChunk[];
}

export type GoodViewTargetSelectionResult =
  | GoodViewSelectedTargetResult
  | GoodViewWaitTargetResult
  | GoodViewBlockedTargetResult;

export type GoodViewGoalEvaluation =
  | (BotGoalEvaluation & {
      readonly type: "activate";
      readonly target: GoodViewTarget;
      readonly currentSurface: BotStandableSurface;
    })
  | (BotGoalEvaluation & {
      readonly type: "wait";
      readonly reason: GoodViewWaitTargetResult["reason"];
      readonly missing: readonly BotMissingChunk[];
    })
  | (BotGoalEvaluation & {
      readonly type: "fail";
      readonly reason: GoodViewBlockedTargetResult["reason"];
      readonly candidateCount: number;
      readonly missing: readonly BotMissingChunk[];
    })
  | (BotGoalEvaluation & {
      readonly type: "complete";
      readonly target: GoodViewTarget;
    });

const DEFAULT_SCAN_RADIUS = 16;
const DEFAULT_VERTICAL_SCAN_BELOW = 8;
const DEFAULT_VERTICAL_SCAN_ABOVE = 32;
const DEFAULT_MAX_CANDIDATE_COUNT = 32;
const DEFAULT_HORIZON_RAY_LENGTH = 5;
const DEFAULT_OPENNESS_RADIUS = 1;
const DEFAULT_RESCAN_INTERVAL_MS = 3000;
const DEFAULT_TARGET_SWITCH_MIN_SCORE_GAIN = 12.0;
const DEFAULT_WAYPOINT_ARRIVAL_RADIUS = 0.45;
const DEFAULT_TARGET_ARRIVAL_RADIUS = 0.75;
const DEFAULT_STUCK_TIMEOUT_MS = 2500;
const DEFAULT_PROGRESS_EPSILON = 0.05;
const DEFAULT_MOVE_Z = 1.0;
const DEFAULT_LOOK_YAW_DEGREES_PER_SECOND = 18.0;

const HORIZON_DIRECTIONS = [
  [1, 0],
  [-1, 0],
  [0, 1],
  [0, -1],
  [1, 1],
  [1, -1],
  [-1, 1],
  [-1, -1],
] as const;

function optionOrDefault(value: number | undefined, fallback: number): number {
  return value ?? fallback;
}

function missingChunkKey(chunk: BotMissingChunk): string {
  return `${chunk.chunkX},${chunk.chunkZ}`;
}

function uniqueMissingChunks(missing: readonly BotMissingChunk[]): readonly BotMissingChunk[] {
  return [...new Map(missing.map((chunk) => [missingChunkKey(chunk), chunk] as const)).values()]
    .sort((left, right) => left.chunkX - right.chunkX || left.chunkZ - right.chunkZ);
}

function mergeMissingChunks(
  left: readonly BotMissingChunk[],
  right: readonly BotMissingChunk[],
): readonly BotMissingChunk[] {
  return uniqueMissingChunks([...left, ...right]);
}

function revisionKeyForObservation(context: Pick<BotControllerTickContext, "observation">): string {
  const loadedChunks = context.observation.loadedChunks
    .map((chunk) => `${chunk.chunkX},${chunk.chunkZ}`)
    .join(";");
  const facts = context.observation.revisionFacts;
  return [
    loadedChunks,
    facts.sessionRevision ?? "",
    facts.collisionRevision ?? "",
    facts.movementPhysicsRevision ?? "",
  ].join("|");
}

function playerPosition(playerState: ClientPlayerState): { readonly x: number; readonly y: number; readonly z: number } {
  return playerState.movementBody?.position ?? playerState.position;
}

function floorPlayerPosition(playerState: ClientPlayerState): { readonly x: number; readonly y: number; readonly z: number } {
  const position = playerPosition(playerState);
  return {
    x: Math.floor(position.x),
    y: Math.floor(position.y + 1.0e-5),
    z: Math.floor(position.z),
  };
}

function distanceToSurfaceXZ(
  playerState: ClientPlayerState,
  surface: Pick<BotStandableSurface, "x" | "z">,
): number {
  const position = playerPosition(playerState);
  return Math.hypot((surface.x + 0.5) - position.x, (surface.z + 0.5) - position.z);
}

function distanceBetweenSurfacesXZ(left: BotStandableSurface, right: BotStandableSurface): number {
  return Math.hypot(left.x - right.x, left.z - right.z);
}

function isSameSurface(left: BotStandableSurface, right: BotStandableSurface): boolean {
  return left.x === right.x && left.y === right.y && left.z === right.z;
}

function formatSurface(surface: Pick<BotStandableSurface, "x" | "y" | "z">): string {
  return `x=${surface.x.toString()} y=${surface.y.toString()} z=${surface.z.toString()}`;
}

function formatMissing(missing: readonly BotMissingChunk[]): string {
  return missing.map((chunk) => `${chunk.chunkX.toString()},${chunk.chunkZ.toString()}`).join(" ");
}

function normalizeDegrees(value: number): number {
  const normalized = value % 360.0;
  if (Object.is(normalized, -0.0)) {
    return 0.0;
  }
  return normalized < 0.0 ? normalized + 360.0 : normalized;
}

function stopIntent(playerState: ClientPlayerState | undefined, yaw?: number): BotMovementIntent {
  return {
    moveX: 0.0,
    moveZ: 0.0,
    yaw: yaw ?? playerState?.rotation.yaw ?? 0.0,
    pitch: playerState?.rotation.pitch ?? 0.0,
    buttons: 0,
    edgeButtons: 0,
  };
}

function isPassableForGoodView(clientWorld: ClientWorld, pos: { readonly x: number; readonly y: number; readonly z: number }): boolean | undefined {
  const query = queryBotBlock(clientWorld, pos);
  if (query.type === "missing") {
    return undefined;
  }

  const level = clientWorld.getRenderView().getRenderLevel();
  const blockPos = new BlockPos(query.pos.x, query.pos.y, query.pos.z);
  return query.state.getFluidState().isEmpty()
    && (query.state.isAir()
      || !query.state.getBlock().hasCollision
      || !query.state.isCollisionShapeFullBlock(level, blockPos));
}

function queryPassableForGoodView(
  clientWorld: ClientWorld,
  pos: { readonly x: number; readonly y: number; readonly z: number },
): { readonly type: "loaded"; readonly passable: boolean } | { readonly type: "missing"; readonly missing: readonly BotMissingChunk[] } {
  const query = queryBotBlock(clientWorld, pos);
  if (query.type === "missing") {
    return {
      type: "missing",
      missing: [{ chunkX: query.chunkX, chunkZ: query.chunkZ }],
    };
  }

  return {
    type: "loaded",
    passable: isPassableForGoodView(clientWorld, pos) ?? false,
  };
}

function evaluateOpenness(
  clientWorld: ClientWorld,
  surface: BotStandableSurface,
  options: GoodViewGoalOptions,
): { readonly type: "loaded"; readonly score: number } | { readonly type: "missing"; readonly missing: readonly BotMissingChunk[] } {
  const radius = optionOrDefault(options.opennessRadius, DEFAULT_OPENNESS_RADIUS);
  let passableCount = 0;
  let sampleCount = 0;
  let missing: readonly BotMissingChunk[] = [];

  for (let dz = -radius; dz <= radius; dz++) {
    for (let dx = -radius; dx <= radius; dx++) {
      for (let dy = 0; dy <= 2; dy++) {
        const query = queryPassableForGoodView(clientWorld, {
          x: surface.x + dx,
          y: surface.y + dy,
          z: surface.z + dz,
        });
        if (query.type === "missing") {
          missing = mergeMissingChunks(missing, query.missing);
          continue;
        }

        sampleCount++;
        if (query.passable) {
          passableCount++;
        }
      }
    }
  }

  if (missing.length > 0) {
    return { type: "missing", missing };
  }

  return {
    type: "loaded",
    score: sampleCount === 0 ? 0.0 : (passableCount / sampleCount) * 18.0,
  };
}

function evaluateHorizon(
  clientWorld: ClientWorld,
  surface: BotStandableSurface,
  options: GoodViewGoalOptions,
): { readonly type: "loaded"; readonly score: number } | { readonly type: "missing"; readonly missing: readonly BotMissingChunk[] } {
  const rayLength = optionOrDefault(options.horizonRayLength, DEFAULT_HORIZON_RAY_LENGTH);
  let clearCount = 0;
  let sampleCount = 0;
  let missing: readonly BotMissingChunk[] = [];

  for (const [dx, dz] of HORIZON_DIRECTIONS) {
    for (let step = 1; step <= rayLength; step++) {
      const query = queryPassableForGoodView(clientWorld, {
        x: surface.x + dx * step,
        y: surface.y + 1,
        z: surface.z + dz * step,
      });
      if (query.type === "missing") {
        missing = mergeMissingChunks(missing, query.missing);
        continue;
      }

      sampleCount++;
      if (!query.passable) {
        break;
      }
      clearCount++;
    }
  }

  if (missing.length > 0) {
    return { type: "missing", missing };
  }

  return {
    type: "loaded",
    score: sampleCount === 0 ? 0.0 : (clearCount / (HORIZON_DIRECTIONS.length * rayLength)) * 24.0,
  };
}

export function findBotCurrentSurface(
  clientWorld: ClientWorld,
  playerState: ClientPlayerState,
): { readonly type: "loaded"; readonly surface: BotStandableSurface }
  | { readonly type: "missing"; readonly missing: readonly BotMissingChunk[] }
  | { readonly type: "blocked"; readonly reason: string } {
  const base = floorPlayerPosition(playerState);
  let missing: readonly BotMissingChunk[] = [];
  let blockedReason = "blocked_current_surface";

  for (const dy of [0, -1, 1, -2, 2] as const) {
    const query = queryBotStandableSurface(clientWorld, {
      x: base.x,
      y: base.y + dy,
      z: base.z,
    });
    if (query.type === "loaded") {
      return query;
    }
    if (query.type === "missing") {
      missing = mergeMissingChunks(missing, query.missing);
      continue;
    }
    blockedReason = query.reason;
  }

  return missing.length > 0
    ? { type: "missing", missing }
    : { type: "blocked", reason: blockedReason };
}

export function scoreGoodViewSurface(
  clientWorld: ClientWorld,
  currentSurface: BotStandableSurface,
  candidate: BotStandableSurface,
  options: GoodViewGoalOptions = {},
  path?: readonly BotStandableSurface[],
): GoodViewSurfaceScoreResult {
  const candidateQuery = queryBotStandableSurface(clientWorld, candidate);
  if (candidateQuery.type === "missing") {
    return { type: "missing", missing: candidateQuery.missing };
  }
  if (candidateQuery.type !== "loaded") {
    return { type: "blocked", reason: "blocked_candidate" };
  }

  const openness = evaluateOpenness(clientWorld, candidate, options);
  if (openness.type === "missing") {
    return openness;
  }
  const horizon = evaluateHorizon(clientWorld, candidate, options);
  if (horizon.type === "missing") {
    return horizon;
  }

  const heightScore = ((candidate.y - currentSurface.y) * 12.0) + (candidate.y * 0.25);
  const opennessScore = openness.score;
  const horizonScore = horizon.score;
  const distanceCost = distanceBetweenSurfacesXZ(currentSurface, candidate) * 0.8;
  const pathCost = path === undefined ? 0.0 : Math.max(0, path.length - 1) * 0.45;
  const score = heightScore + opennessScore + horizonScore - distanceCost - pathCost;

  return {
    type: "loaded",
    score: {
      surface: candidate,
      score,
      heightScore,
      opennessScore,
      horizonScore,
      distanceCost,
      pathCost,
      pathLength: path?.length,
    },
  };
}

export function selectGoodViewTarget(
  clientWorld: ClientWorld,
  playerState: ClientPlayerState | undefined,
  spatialIndex = new BotSpatialIndex(),
  options: GoodViewGoalOptions = {},
  nowMs = 0,
  revisionKey = "",
): GoodViewTargetSelectionResult {
  if (playerState === undefined) {
    return {
      type: "wait",
      status: "good-view waiting for player state",
      reason: "missing_player",
      missing: [],
    };
  }

  const current = findBotCurrentSurface(clientWorld, playerState);
  if (current.type === "missing") {
    return {
      type: "wait",
      status: `good-view waiting missing current surface chunk=${formatMissing(current.missing)}`,
      reason: "missing_current_surface",
      missing: current.missing,
    };
  }
  if (current.type === "blocked") {
    return {
      type: "blocked",
      status: `good-view blocked current surface reason=${current.reason}`,
      reason: "blocked_current_surface",
      candidateCount: 0,
      missing: [],
    };
  }

  const scanRadius = optionOrDefault(options.scanRadius, DEFAULT_SCAN_RADIUS);
  const verticalScanBelow = optionOrDefault(options.verticalScanBelow, DEFAULT_VERTICAL_SCAN_BELOW);
  const verticalScanAbove = optionOrDefault(options.verticalScanAbove, DEFAULT_VERTICAL_SCAN_ABOVE);
  const level = clientWorld.getRenderView().getRenderLevel();
  const bounds = {
    minX: current.surface.x - scanRadius,
    maxX: current.surface.x + scanRadius,
    minY: Math.max(level.getMinBuildHeight() + 1, current.surface.y - verticalScanBelow),
    maxY: Math.min(level.getMaxBuildHeight() - 2, current.surface.y + verticalScanAbove),
    minZ: current.surface.z - scanRadius,
    maxZ: current.surface.z + scanRadius,
  };
  const scan = spatialIndex.scanStandableSurfaces(clientWorld, bounds);
  if (scan.surfaces.length === 0) {
    return scan.missing.length > 0
      ? {
          type: "wait",
          status: `good-view waiting missing scan chunk=${formatMissing(scan.missing)}`,
          reason: "no_loaded_surfaces",
          missing: scan.missing,
        }
      : {
          type: "blocked",
          status: "good-view blocked no loaded standable surfaces",
          reason: "no_reachable_target",
          candidateCount: 0,
          missing: [],
        };
  }

  let missing: readonly BotMissingChunk[] = scan.missing;
  const preliminary = scan.surfaces
    .map((surface) => scoreGoodViewSurface(clientWorld, current.surface, surface, options))
    .filter((result): result is GoodViewLoadedSurfaceScoreResult => {
      if (result.type === "missing") {
        missing = mergeMissingChunks(missing, result.missing);
      }
      return result.type === "loaded";
    })
    .sort((left, right) => right.score.score - left.score.score)
    .slice(0, optionOrDefault(options.maxCandidateCount, DEFAULT_MAX_CANDIDATE_COUNT));

  let bestTarget: GoodViewTarget | undefined;
  const pathPlanner = options.pathPlanner ?? {};
  for (const candidate of preliminary) {
    const pathResult = planPathBetweenSurfaces(clientWorld, current.surface, candidate.score.surface, pathPlanner);
    if (pathResult.type === "missing") {
      missing = mergeMissingChunks(missing, pathResult.missing);
      continue;
    }
    if (pathResult.type !== "loaded") {
      continue;
    }

    const finalScore = scoreGoodViewSurface(clientWorld, current.surface, candidate.score.surface, options, pathResult.path);
    if (finalScore.type === "missing") {
      missing = mergeMissingChunks(missing, finalScore.missing);
      continue;
    }
    if (finalScore.type !== "loaded") {
      continue;
    }

    if (bestTarget === undefined || finalScore.score.score > bestTarget.score.score) {
      bestTarget = {
        surface: candidate.score.surface,
        score: finalScore.score,
        path: pathResult.path,
        candidateCount: preliminary.length,
        selectedAtMs: nowMs,
        revisionKey,
      };
    }
  }

  if (bestTarget === undefined) {
    return missing.length > 0
      ? {
          type: "wait",
          status: `good-view waiting missing reachable path chunk=${formatMissing(missing)}`,
          reason: "missing_path",
          missing,
        }
      : {
          type: "blocked",
          status: `good-view blocked no reachable target candidates=${preliminary.length.toString()}`,
          reason: "no_reachable_target",
          candidateCount: preliminary.length,
          missing: [],
        };
  }

  return {
    type: "selected",
    target: bestTarget,
    currentSurface: current.surface,
    missing,
  };
}

export class GoodViewGoal implements BotGoal<GoodViewGoalEvaluation> {
  private readonly spatialIndex = new BotSpatialIndex();
  private target: GoodViewTarget | undefined;
  private pathIndex = 0;
  private lastScanMs = Number.NEGATIVE_INFINITY;
  private lastStatus = "good-view waiting";
  private lastProgressDistance: number | undefined;
  private stuckElapsedMs = 0;
  private lookYaw: number | undefined;

  public constructor(private readonly options: GoodViewGoalOptions = {}) {}

  public evaluate(context: BotControllerTickContext): GoodViewGoalEvaluation {
    if (this.target !== undefined && this.complete(context)) {
      return {
        type: "complete",
        status: `good-view arrived at target ${formatSurface(this.target.surface)}`,
        target: this.target,
      };
    }

    const selection = selectGoodViewTarget(
      context.clientWorld,
      context.observation.playerState,
      this.spatialIndex,
      this.options,
      context.nowMs,
      revisionKeyForObservation(context),
    );
    if (selection.type === "selected") {
      return {
        type: "activate",
        status: `good-view selected target ${formatSurface(selection.target.surface)} score=${selection.target.score.score.toFixed(1)} path=${selection.target.path.length.toString()}`,
        target: selection.target,
        currentSurface: selection.currentSurface,
      };
    }
    if (selection.type === "wait") {
      return {
        type: "wait",
        status: selection.status,
        reason: selection.reason,
        missing: selection.missing,
      };
    }

    return {
      type: "fail",
      status: selection.status,
      reason: selection.reason,
      candidateCount: selection.candidateCount,
      missing: selection.missing,
    };
  }

  public activate(_context: BotControllerTickContext, evaluation: GoodViewGoalEvaluation): void {
    if (evaluation.type !== "activate") {
      return;
    }

    this.target = evaluation.target;
    this.pathIndex = 0;
    this.lastScanMs = evaluation.target.selectedAtMs;
    this.lastProgressDistance = undefined;
    this.stuckElapsedMs = 0;
    this.lookYaw = undefined;
  }

  public tick(context: BotControllerTickContext): BotControllerTickResult {
    const playerState = context.observation.playerState;
    if (playerState === undefined) {
      return this.withStatus({
        movement: stopIntent(undefined),
        status: "good-view waiting for player state",
      });
    }

    if (this.target !== undefined && !this.isTargetStillUsable(context.clientWorld)) {
      const failed = this.target;
      this.fail(context, "target left loaded world");
      return this.withStatus({
        movement: stopIntent(playerState),
        status: `good-view target unavailable ${formatSurface(failed.surface)}`,
      });
    }

    if (this.target !== undefined && this.shouldRescan(context)) {
      const evaluation = this.evaluate(context);
      if (evaluation.type === "activate" && evaluation.target.score.score > this.target.score.score + optionOrDefault(this.options.targetSwitchMinScoreGain, DEFAULT_TARGET_SWITCH_MIN_SCORE_GAIN)) {
        this.activate(context, evaluation);
        return this.tickActiveTarget(context, evaluation.status);
      }
      this.lastScanMs = context.nowMs;
    }

    if (this.target === undefined) {
      const evaluation = this.evaluate(context);
      if (evaluation.type === "activate") {
        this.activate(context, evaluation);
        return this.tickActiveTarget(context, evaluation.status);
      }
      if (evaluation.type === "complete") {
        return this.tickArrival(context, evaluation.status);
      }

      return this.withStatus({
        movement: stopIntent(playerState),
        status: evaluation.status,
      });
    }

    return this.tickActiveTarget(context);
  }

  public complete(context: BotControllerTickContext): boolean {
    if (this.target === undefined || context.observation.playerState === undefined) {
      return false;
    }

    const position = playerPosition(context.observation.playerState);
    return distanceToSurfaceXZ(context.observation.playerState, this.target.surface) <= optionOrDefault(this.options.targetArrivalRadius, DEFAULT_TARGET_ARRIVAL_RADIUS)
      && Math.abs(position.y - this.target.surface.y) <= 1.25;
  }

  public fail(_context: BotControllerTickContext, reason: string): void {
    this.target = undefined;
    this.pathIndex = 0;
    this.lastProgressDistance = undefined;
    this.stuckElapsedMs = 0;
    this.lastStatus = `good-view failed ${reason}`;
  }

  public explain(): string {
    return this.lastStatus;
  }

  public getSnapshot(): GoodViewGoalSnapshot {
    return {
      target: this.target,
      pathIndex: this.pathIndex,
      status: this.lastStatus,
    };
  }

  private shouldRescan(context: BotControllerTickContext): boolean {
    if (this.target === undefined) {
      return true;
    }

    if (context.nowMs - this.lastScanMs < optionOrDefault(this.options.rescanIntervalMs, DEFAULT_RESCAN_INTERVAL_MS)) {
      return false;
    }

    return revisionKeyForObservation(context) !== this.target.revisionKey;
  }

  private isTargetStillUsable(clientWorld: ClientWorld): boolean {
    if (this.target === undefined) {
      return false;
    }

    const query = queryBotStandableSurface(clientWorld, this.target.surface);
    return query.type === "loaded" && isSameSurface(query.surface, this.target.surface);
  }

  private tickActiveTarget(context: BotControllerTickContext, statusOverride?: string): BotControllerTickResult {
    const playerState = context.observation.playerState;
    if (playerState === undefined || this.target === undefined) {
      return this.withStatus({
        movement: stopIntent(playerState),
        status: "good-view waiting for player state",
      });
    }

    if (this.complete(context)) {
      return this.tickArrival(context, `good-view arrived at target ${formatSurface(this.target.surface)}`);
    }

    const previousPathIndex = this.pathIndex;
    while (this.pathIndex < this.target.path.length
      && distanceToSurfaceXZ(playerState, this.target.path[this.pathIndex]!) <= optionOrDefault(this.options.waypointArrivalRadius, DEFAULT_WAYPOINT_ARRIVAL_RADIUS)) {
      this.pathIndex++;
    }
    if (this.pathIndex !== previousPathIndex) {
      this.lastProgressDistance = undefined;
      this.stuckElapsedMs = 0;
    }

    const waypoint = this.target.path[Math.min(this.pathIndex, this.target.path.length - 1)] ?? this.target.surface;
    const waypointDistance = distanceToSurfaceXZ(playerState, waypoint);
    if (this.lastProgressDistance === undefined || waypointDistance < this.lastProgressDistance - optionOrDefault(this.options.progressEpsilon, DEFAULT_PROGRESS_EPSILON)) {
      this.stuckElapsedMs = 0;
    } else {
      this.stuckElapsedMs += context.elapsedMs;
    }
    this.lastProgressDistance = waypointDistance;

    if (this.stuckElapsedMs >= optionOrDefault(this.options.stuckTimeoutMs, DEFAULT_STUCK_TIMEOUT_MS)) {
      const stuckTarget = this.target;
      this.target = undefined;
      this.pathIndex = 0;
      this.lastProgressDistance = undefined;
      this.stuckElapsedMs = 0;
      return this.withStatus({
        movement: stopIntent(playerState),
        status: `good-view stuck target ${formatSurface(stuckTarget.surface)}`,
      });
    }

    return this.withStatus({
      movement: steerTowardWaypoint(playerState, waypoint, {
        arrivalRadius: optionOrDefault(this.options.waypointArrivalRadius, DEFAULT_WAYPOINT_ARRIVAL_RADIUS),
        moveZ: optionOrDefault(this.options.moveZ, DEFAULT_MOVE_Z),
      }),
      status: statusOverride ?? `good-view moving target ${formatSurface(this.target.surface)}`,
    });
  }

  private tickArrival(context: BotControllerTickContext, status: string): BotControllerTickResult {
    const playerState = context.observation.playerState;
    this.lookYaw = normalizeDegrees(
      (this.lookYaw ?? playerState?.rotation.yaw ?? 0.0)
      + optionOrDefault(this.options.lookYawDegreesPerSecond, DEFAULT_LOOK_YAW_DEGREES_PER_SECOND) * (context.elapsedMs / 1000.0),
    );
    return this.withStatus({
      movement: stopIntent(playerState, this.lookYaw),
      status,
    });
  }

  private withStatus(result: BotControllerTickResult): BotControllerTickResult {
    this.lastStatus = result.status ?? this.lastStatus;
    return result;
  }
}

export class GoodViewBotController implements BotController {
  private readonly goal: GoodViewGoal;

  public constructor(options: GoodViewGoalOptions = {}) {
    this.goal = new GoodViewGoal(options);
  }

  public tick(context: BotControllerTickContext): BotControllerTickResult {
    return this.goal.tick(context);
  }

  public explain(): string {
    return this.goal.explain();
  }

  public getSnapshot(): GoodViewGoalSnapshot {
    return this.goal.getSnapshot();
  }
}
