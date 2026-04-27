import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import type { ClientPresentationState, ClientRuntime } from "../client/client-runtime";
import type { ClientWorld, ClientWorldRevisionFacts } from "../client/client-world";
import type { ClientPlayerState, ClientSessionState } from "../protocol/world-messages";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { ClientChunkCache } from "../../world/level/client-chunk-cache";

export interface BotBlockPosition {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface BotLoadedChunkSummary {
  readonly chunkX: number;
  readonly chunkZ: number;
}

export interface BotMissingChunk {
  readonly chunkX: number;
  readonly chunkZ: number;
}

export interface BotObservation {
  readonly sessionState?: ClientSessionState;
  readonly playerState?: ClientPlayerState;
  readonly presentation: ClientPresentationState;
  readonly revisionFacts: ClientWorldRevisionFacts;
  readonly loadedChunks: readonly BotLoadedChunkSummary[];
  readonly loadedChunkCount: number;
  readonly entityCount: number;
}

export interface BotLoadedBlockQuery {
  readonly type: "loaded";
  readonly pos: BotBlockPosition;
  readonly state: BlockState;
}

export interface BotMissingBlockQuery {
  readonly type: "missing";
  readonly pos: BotBlockPosition;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly reason: "missing_chunk" | "out_of_build_height";
}

export type BotBlockQuery = BotLoadedBlockQuery | BotMissingBlockQuery;

export interface BotStandableSurface {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly supportY: number;
}

export interface BotLoadedStandableQuery {
  readonly type: "loaded";
  readonly surface: BotStandableSurface;
}

export interface BotBlockedStandableQuery {
  readonly type: "blocked";
  readonly pos: BotBlockPosition;
  readonly reason: "missing_support" | "blocked_body" | "liquid" | "not_solid";
}

export interface BotMissingStandableQuery {
  readonly type: "missing";
  readonly pos: BotBlockPosition;
  readonly missing: readonly BotMissingChunk[];
  readonly reason: "missing_chunk" | "out_of_build_height";
}

export type BotStandableQuery = BotLoadedStandableQuery | BotBlockedStandableQuery | BotMissingStandableQuery;

export interface BotSurfaceScanBounds {
  readonly minX: number;
  readonly maxX: number;
  readonly minY: number;
  readonly maxY: number;
  readonly minZ: number;
  readonly maxZ: number;
}

export interface BotSurfaceScanResult {
  readonly type: "loaded" | "missing";
  readonly bounds: BotSurfaceScanBounds;
  readonly surfaces: readonly BotStandableSurface[];
  readonly missing: readonly BotMissingChunk[];
}

export class BotSpatialIndex {
  private cachedKey: string | undefined;
  private cachedResult: BotSurfaceScanResult | undefined;

  public scanStandableSurfaces(clientWorld: ClientWorld, bounds: BotSurfaceScanBounds): BotSurfaceScanResult {
    const key = createSurfaceScanCacheKey(clientWorld, bounds);
    if (this.cachedKey === key && this.cachedResult !== undefined) {
      return this.cachedResult;
    }

    const result = scanStandableSurfaces(clientWorld, bounds);
    this.cachedKey = key;
    this.cachedResult = result;
    return result;
  }

  public clear(): void {
    this.cachedKey = undefined;
    this.cachedResult = undefined;
  }
}

function normalizeBlockPosition(pos: BotBlockPosition): BotBlockPosition {
  return {
    x: Math.trunc(pos.x),
    y: Math.trunc(pos.y),
    z: Math.trunc(pos.z),
  };
}

function missingChunkKey(chunk: BotMissingChunk): string {
  return `${chunk.chunkX},${chunk.chunkZ}`;
}

function uniqueMissingChunks(missing: readonly BotMissingChunk[]): readonly BotMissingChunk[] {
  return [...new Map(missing.map((chunk) => [missingChunkKey(chunk), chunk] as const)).values()]
    .sort((left, right) => left.chunkX - right.chunkX || left.chunkZ - right.chunkZ);
}

function getClientLevel(clientWorld: ClientWorld): ClientChunkCache {
  return clientWorld.getRenderView().getRenderLevel();
}

function isInBuildHeight(level: ClientChunkCache, y: number): boolean {
  return y >= level.getMinBuildHeight() && y < level.getMaxBuildHeight();
}

function isFullCollisionBlock(level: ClientChunkCache, pos: BlockPos, state: BlockState): boolean {
  return !state.isAir()
    && state.getBlock().hasCollision
    && state.getFluidState().isEmpty()
    && state.isCollisionShapeFullBlock(level, pos);
}

function isPassableForBot(level: ClientChunkCache, pos: BlockPos, state: BlockState): boolean {
  return state.getFluidState().isEmpty()
    && (state.isAir() || !state.getBlock().hasCollision || !state.isCollisionShapeFullBlock(level, pos));
}

export function createBotObservation(clientRuntime: ClientRuntime): BotObservation {
  const presentation = clientRuntime.publishPresentationState();
  const clientWorld = clientRuntime.getClientWorld();
  const loadedChunks = getClientLevel(clientWorld).getLoadedChunks()
    .map((chunk) => ({
      chunkX: chunk.chunkX,
      chunkZ: chunk.chunkZ,
    }))
    .sort((left, right) => left.chunkX - right.chunkX || left.chunkZ - right.chunkZ);

  return {
    sessionState: presentation.sessionState,
    playerState: presentation.localPlayerState,
    presentation,
    revisionFacts: clientWorld.getRevisionFacts(),
    loadedChunks,
    loadedChunkCount: loadedChunks.length,
    entityCount: presentation.entities.length,
  };
}

export function queryBotBlock(clientWorld: ClientWorld, pos: BotBlockPosition): BotBlockQuery {
  const normalized = normalizeBlockPosition(pos);
  const level = getClientLevel(clientWorld);
  const chunkX = SectionPos.blockToSectionCoord(normalized.x);
  const chunkZ = SectionPos.blockToSectionCoord(normalized.z);
  if (!isInBuildHeight(level, normalized.y)) {
    return {
      type: "missing",
      pos: normalized,
      chunkX,
      chunkZ,
      reason: "out_of_build_height",
    };
  }

  if (clientWorld.getChunkSnapshot(chunkX, chunkZ) === undefined) {
    return {
      type: "missing",
      pos: normalized,
      chunkX,
      chunkZ,
      reason: "missing_chunk",
    };
  }

  return {
    type: "loaded",
    pos: normalized,
    state: level.getBlockState(new BlockPos(normalized.x, normalized.y, normalized.z)),
  };
}

export function queryBotStandableSurface(clientWorld: ClientWorld, pos: BotBlockPosition): BotStandableQuery {
  const feet = normalizeBlockPosition(pos);
  const support = queryBotBlock(clientWorld, { x: feet.x, y: feet.y - 1, z: feet.z });
  const feetBlock = queryBotBlock(clientWorld, feet);
  const headBlock = queryBotBlock(clientWorld, { x: feet.x, y: feet.y + 1, z: feet.z });
  const missing = [support, feetBlock, headBlock].filter((query): query is BotMissingBlockQuery => query.type === "missing");
  if (missing.length > 0) {
    return {
      type: "missing",
      pos: feet,
      missing: uniqueMissingChunks(missing.map((query) => ({ chunkX: query.chunkX, chunkZ: query.chunkZ }))),
      reason: missing.some((query) => query.reason === "out_of_build_height") ? "out_of_build_height" : "missing_chunk",
    };
  }
  if (support.type !== "loaded" || feetBlock.type !== "loaded" || headBlock.type !== "loaded") {
    throw new Error("Bot standable query expected loaded block queries after missing checks");
  }

  const level = getClientLevel(clientWorld);
  const supportPos = new BlockPos(feet.x, feet.y - 1, feet.z);
  if (!isFullCollisionBlock(level, supportPos, support.state)) {
    return {
      type: "blocked",
      pos: feet,
      reason: support.state.isAir() ? "missing_support" : support.state.getFluidState().isEmpty() ? "not_solid" : "liquid",
    };
  }

  const feetPos = new BlockPos(feet.x, feet.y, feet.z);
  const headPos = new BlockPos(feet.x, feet.y + 1, feet.z);
  if (!isPassableForBot(level, feetPos, feetBlock.state) || !isPassableForBot(level, headPos, headBlock.state)) {
    return {
      type: "blocked",
      pos: feet,
      reason: !feetBlock.state.getFluidState().isEmpty() || !headBlock.state.getFluidState().isEmpty() ? "liquid" : "blocked_body",
    };
  }

  return {
    type: "loaded",
    surface: {
      x: feet.x,
      y: feet.y,
      z: feet.z,
      supportY: feet.y - 1,
    },
  };
}

export function scanStandableSurfaces(clientWorld: ClientWorld, bounds: BotSurfaceScanBounds): BotSurfaceScanResult {
  const surfaces: BotStandableSurface[] = [];
  const missing: BotMissingChunk[] = [];
  const minX = Math.min(bounds.minX, bounds.maxX);
  const maxX = Math.max(bounds.minX, bounds.maxX);
  const minY = Math.min(bounds.minY, bounds.maxY);
  const maxY = Math.max(bounds.minY, bounds.maxY);
  const minZ = Math.min(bounds.minZ, bounds.maxZ);
  const maxZ = Math.max(bounds.minZ, bounds.maxZ);

  for (let z = minZ; z <= maxZ; z++) {
    for (let x = minX; x <= maxX; x++) {
      for (let y = maxY; y >= minY; y--) {
        const query = queryBotStandableSurface(clientWorld, { x, y, z });
        if (query.type === "loaded") {
          surfaces.push(query.surface);
          break;
        }
        if (query.type === "missing") {
          missing.push(...query.missing);
          break;
        }
      }
    }
  }

  const uniqueMissing = uniqueMissingChunks(missing);
  return {
    type: uniqueMissing.length === 0 ? "loaded" : "missing",
    bounds: {
      minX,
      maxX,
      minY,
      maxY,
      minZ,
      maxZ,
    },
    surfaces,
    missing: uniqueMissing,
  };
}

function createSurfaceScanCacheKey(clientWorld: ClientWorld, bounds: BotSurfaceScanBounds): string {
  const revision = clientWorld.getRevisionFacts();
  const loadedChunks = getClientLevel(clientWorld).getLoadedChunks()
    .map((chunk) => `${chunk.chunkX},${chunk.chunkZ}`)
    .sort()
    .join(";");
  return [
    bounds.minX,
    bounds.maxX,
    bounds.minY,
    bounds.maxY,
    bounds.minZ,
    bounds.maxZ,
    revision.sessionRevision ?? "",
    revision.localPlayerRevision ?? "",
    revision.movementPhysicsRevision ?? "",
    revision.collisionRevision ?? "",
    loadedChunks,
  ].join("|");
}
