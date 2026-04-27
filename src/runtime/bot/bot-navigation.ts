import type { ClientWorld } from "../client/client-world";
import type { ClientPlayerState } from "../protocol/world-messages";
import type { BotMovementIntent } from "./bot-runtime";
import {
  queryBotStandableSurface,
  type BotMissingChunk,
  type BotStandableSurface,
} from "./bot-observation";

export interface BotPathPlannerOptions {
  readonly maxExpandedNodes?: number;
  readonly maxStepUp?: number;
  readonly maxDrop?: number;
}

export interface BotLoadedPathResult {
  readonly type: "loaded";
  readonly path: readonly BotStandableSurface[];
  readonly expandedNodeCount: number;
}

export interface BotBlockedPathResult {
  readonly type: "blocked";
  readonly reason: "no_path" | "invalid_start" | "invalid_goal" | "node_limit";
  readonly expandedNodeCount: number;
}

export interface BotMissingPathResult {
  readonly type: "missing";
  readonly reason: "missing_chunk";
  readonly missing: readonly BotMissingChunk[];
  readonly expandedNodeCount: number;
}

export type BotPathResult = BotLoadedPathResult | BotBlockedPathResult | BotMissingPathResult;

export interface BotWaypointSteeringOptions {
  readonly arrivalRadius?: number;
  readonly moveZ?: number;
}

interface PathNode {
  readonly surface: BotStandableSurface;
  readonly key: string;
  readonly parentKey?: string;
  readonly cost: number;
  readonly priority: number;
}

const DEFAULT_MAX_EXPANDED_NODES = 512;
const DEFAULT_MAX_STEP_UP = 1;
const DEFAULT_MAX_DROP = 3;

function surfaceKey(surface: Pick<BotStandableSurface, "x" | "y" | "z">): string {
  return `${surface.x},${surface.y},${surface.z}`;
}

function missingChunkKey(chunk: BotMissingChunk): string {
  return `${chunk.chunkX},${chunk.chunkZ}`;
}

function uniqueMissingChunks(missing: readonly BotMissingChunk[]): readonly BotMissingChunk[] {
  return [...new Map(missing.map((chunk) => [missingChunkKey(chunk), chunk] as const)).values()]
    .sort((left, right) => left.chunkX - right.chunkX || left.chunkZ - right.chunkZ);
}

function isMissingChunkList(value: unknown): value is readonly BotMissingChunk[] {
  return Array.isArray(value);
}

function heuristic(left: BotStandableSurface, right: BotStandableSurface): number {
  return Math.abs(left.x - right.x) + Math.abs(left.z - right.z) + Math.abs(left.y - right.y);
}

function reconstructPath(nodes: ReadonlyMap<string, PathNode>, goalKey: string): readonly BotStandableSurface[] {
  const path: BotStandableSurface[] = [];
  let key: string | undefined = goalKey;
  while (key !== undefined) {
    const node = nodes.get(key);
    if (node === undefined) {
      break;
    }
    path.push(node.surface);
    key = node.parentKey;
  }
  path.reverse();
  return path;
}

function popBest(open: PathNode[]): PathNode | undefined {
  if (open.length === 0) {
    return undefined;
  }

  let bestIndex = 0;
  for (let index = 1; index < open.length; index++) {
    if (open[index]!.priority < open[bestIndex]!.priority) {
      bestIndex = index;
    }
  }
  const [node] = open.splice(bestIndex, 1);
  return node;
}

function findStandableNear(
  clientWorld: ClientWorld,
  x: number,
  z: number,
  minY: number,
  maxY: number,
): BotStandableSurface | readonly BotMissingChunk[] | undefined {
  const missing: BotMissingChunk[] = [];
  for (let y = maxY; y >= minY; y--) {
    const query = queryBotStandableSurface(clientWorld, { x, y, z });
    if (query.type === "loaded") {
      return query.surface;
    }
    if (query.type === "missing") {
      missing.push(...query.missing);
    }
  }

  return missing.length > 0 ? uniqueMissingChunks(missing) : undefined;
}

function validateSurface(clientWorld: ClientWorld, surface: BotStandableSurface): "valid" | "blocked" | readonly BotMissingChunk[] {
  const query = queryBotStandableSurface(clientWorld, surface);
  if (query.type === "loaded") {
    return "valid";
  }
  if (query.type === "missing") {
    return query.missing;
  }
  return "blocked";
}

export function planPathBetweenSurfaces(
  clientWorld: ClientWorld,
  start: BotStandableSurface,
  goal: BotStandableSurface,
  options: BotPathPlannerOptions = {},
): BotPathResult {
  const startValidation = validateSurface(clientWorld, start);
  if (startValidation !== "valid") {
    return isMissingChunkList(startValidation)
      ? { type: "missing", reason: "missing_chunk", missing: startValidation, expandedNodeCount: 0 }
      : { type: "blocked", reason: "invalid_start", expandedNodeCount: 0 };
  }

  const goalValidation = validateSurface(clientWorld, goal);
  if (goalValidation !== "valid") {
    return isMissingChunkList(goalValidation)
      ? { type: "missing", reason: "missing_chunk", missing: goalValidation, expandedNodeCount: 0 }
      : { type: "blocked", reason: "invalid_goal", expandedNodeCount: 0 };
  }

  const goalKey = surfaceKey(goal);
  const maxExpandedNodes = options.maxExpandedNodes ?? DEFAULT_MAX_EXPANDED_NODES;
  const maxStepUp = options.maxStepUp ?? DEFAULT_MAX_STEP_UP;
  const maxDrop = options.maxDrop ?? DEFAULT_MAX_DROP;
  const open: PathNode[] = [{
    surface: start,
    key: surfaceKey(start),
    cost: 0,
    priority: heuristic(start, goal),
  }];
  const bestNodes = new Map<string, PathNode>([[surfaceKey(start), open[0]!]]);
  const closed = new Set<string>();
  const missing: BotMissingChunk[] = [];
  let expandedNodeCount = 0;

  while (open.length > 0) {
    if (expandedNodeCount >= maxExpandedNodes) {
      return { type: "blocked", reason: "node_limit", expandedNodeCount };
    }

    const current = popBest(open)!;
    if (closed.has(current.key)) {
      continue;
    }
    if (current.key === goalKey) {
      return {
        type: "loaded",
        path: reconstructPath(bestNodes, current.key),
        expandedNodeCount,
      };
    }

    closed.add(current.key);
    expandedNodeCount++;

    for (const [dx, dz] of [[1, 0], [-1, 0], [0, 1], [0, -1]] as const) {
      const neighbor = findStandableNear(
        clientWorld,
        current.surface.x + dx,
        current.surface.z + dz,
        current.surface.y - maxDrop,
        current.surface.y + maxStepUp,
      );
      if (neighbor === undefined) {
        continue;
      }
      if (isMissingChunkList(neighbor)) {
        missing.push(...neighbor);
        continue;
      }

      const nextKey = surfaceKey(neighbor);
      if (closed.has(nextKey)) {
        continue;
      }

      const stepCost = 1 + Math.max(0, neighbor.y - current.surface.y) + Math.max(0, (current.surface.y - neighbor.y) * 0.25);
      const cost = current.cost + stepCost;
      const existing = bestNodes.get(nextKey);
      if (existing !== undefined && existing.cost <= cost) {
        continue;
      }

      const nextNode: PathNode = {
        surface: neighbor,
        key: nextKey,
        parentKey: current.key,
        cost,
        priority: cost + heuristic(neighbor, goal),
      };
      bestNodes.set(nextKey, nextNode);
      open.push(nextNode);
    }
  }

  const uniqueMissing = uniqueMissingChunks(missing);
  if (uniqueMissing.length > 0) {
    return {
      type: "missing",
      reason: "missing_chunk",
      missing: uniqueMissing,
      expandedNodeCount,
    };
  }

  return {
    type: "blocked",
    reason: "no_path",
    expandedNodeCount,
  };
}

export function steerTowardWaypoint(
  playerState: ClientPlayerState,
  waypoint: Pick<BotStandableSurface, "x" | "y" | "z">,
  options: BotWaypointSteeringOptions = {},
): BotMovementIntent {
  const position = playerState.movementBody?.position ?? playerState.position;
  const targetX = waypoint.x + 0.5;
  const targetZ = waypoint.z + 0.5;
  const dx = targetX - position.x;
  const dz = targetZ - position.z;
  const distance = Math.hypot(dx, dz);
  const arrivalRadius = options.arrivalRadius ?? 0.35;
  const yaw = distance <= 1.0e-7
    ? playerState.rotation.yaw
    : normalizeDegrees(Math.atan2(-dx, dz) * 180.0 / Math.PI);

  return {
    moveX: 0.0,
    moveZ: distance <= arrivalRadius ? 0.0 : (options.moveZ ?? 1.0),
    yaw,
    pitch: playerState.rotation.pitch,
    buttons: 0,
    edgeButtons: 0,
  };
}

function normalizeDegrees(value: number): number {
  const normalized = value % 360.0;
  if (Object.is(normalized, -0.0)) {
    return 0.0;
  }
  return normalized < 0.0 ? normalized + 360.0 : normalized;
}
