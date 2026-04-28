import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import { ChunkBiomeContainer } from "../../worldgen/biome/chunk-biome-container";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import type { NoiseBiomeSource } from "../../worldgen/biome/noise-biome-source";
import { NoiseBasedChunkGenerator } from "../../worldgen/levelgen/noise-based-chunk-generator";
import {
  buildChunkSnapshot,
  createBlockStateResolver,
  hydrateChunkFromSnapshot,
  type ChunkSnapshot,
} from "../../world/level/chunk-snapshot";
import {
  applyPackedChunkLightDelta,
  clonePackedChunkSnapshot,
  packChunkSnapshot,
  unpackChunkSnapshot,
  type PackedChunkLight,
  type PackedChunkSnapshot,
} from "../../world/level/packed-chunk-snapshot";
import {
  FEATURES_CHUNK_DEPENDENCY_RADIUS,
} from "../../world/level/generated-decoration-region";
import {
  GeneratedChunkStatus,
  getGeneratedChunkDependencyStatus,
} from "../../world/level/generated-chunk-status";
import {
  GENERATED_CHUNK_BORDER_LEVEL,
  GENERATED_CHUNK_ENTITY_TICKING_LEVEL,
  GeneratedChunkTicketSet,
  type GeneratedChunkTicketDebugRecord,
} from "../../world/level/generated-chunk-tickets";
import {
  GENERATED_PROTO_CHUNK_CONTENT_VERSION,
  advanceGeneratedChunkAccessStatus,
  createGeneratedProtoChunk,
  createGeneratedChunkAccessFromStorageRecord,
  createGeneratedChunkStorageRecord,
  cloneGeneratedChunkStorageRecord,
  type GeneratedChunkAccess,
  type GeneratedChunkAccessDebugRecord,
  type GeneratedChunkStorageRecord,
} from "../../world/level/generated-proto-chunk";
import { GeneratedRenderLevel, type GeneratedChunkStatusDebugRecord } from "../../world/level/generated-render-level";
import type { LevelChunk } from "../../world/level/chunk/level-chunk";
import { LEVEL_CHUNK_SECTION_SIZE } from "../../world/level/chunk/level-chunk-section";
import { FullChunkStatus } from "../../world/level/entity/full-chunk-status";
import { AABB } from "../../world/phys/aabb";
import { Vec3 } from "../../world/phys/vec3";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { BlockStateIdMap } from "../../world/level/block/state/block-state-id";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { Fluid } from "../../world/level/material/fluid";
import { Fluids } from "../../world/level/material/fluids";
import type { GeneratedMobEntity } from "../../world/entity/entity-type";
import type { MobLookTarget } from "../../world/entity/ai/pathfinder-mob";
import { collectLivingEntityPushDeltas, type LivingEntityPushParticipant } from "../../world/entity/entity-push";
import type {
  ChunkLightDeltaResult,
  ChunkLightReadyResult,
  LightErrorResult,
  LightProgressResult,
  LightingNeighborRevision,
  LightingService,
} from "../lighting/lighting-protocol";
import {
  createScheduledTickSnapshot,
  resolveFluidTickTarget,
  serializeFluidTickTarget,
  type ScheduledTickSnapshot,
} from "../../world/level/scheduled-tick";
import { ServerTickList, type TickNextTickData } from "../../world/level/server-tick-list";
import type { WorldGenerator } from "../../worldgen/levelgen/world-generator";
import type { WorldHost } from "../protocol/world-host";
import { drainWorldHostMessages } from "../protocol/world-message-queue";
import {
  DEFAULT_PLAYER_PROFILE,
  normalizeWorldEngineConfig,
  type ClientPlayerState,
  type ClientSessionState,
  type EntitySnapshot,
  type EntitySnapshotCategory,
  type EntitySnapshotMessage,
  type EntityUpdate,
  type EntityUpdateMessage,
  type OpenWorldPreset,
  type OpenWorldRequest,
  type PollWorldUpdatesRequest,
  type SetChunkViewRequest,
  type SetPlayerInputRequest,
  type SessionChunkViewState,
  type WorldEngineConfig,
  type WorldEngineLightingMode,
  type WorldEngineLiquidSimulationMode,
  type NormalizedWorldEngineConfig,
  type WorldHostMessage,
  type WorldOpenedMessage,
  type WorldPerformanceMessage,
  type WorldgenPerformanceCounters,
  type WorldgenPhasePerformanceCounters,
  type WorldProgressMessage,
} from "../protocol/world-messages";
import {
  anchorPlayerStateToChunkView,
  createPlayerCommandQueue,
  createInitialPlayerState,
  enqueuePlayerInputCommand,
  movementBodyFromPlayerState,
  PLAYER_TICK_INTERVAL_MS,
  pushPlayerStateWithCollision,
  tickPlayerStateWithCommandQueue,
  type PlayerCommandQueue,
} from "../session/player-loop";
import { blockGetterCollisionWorld, type CollisionWorld } from "../movement/collision-world";
import {
  createWorldSaveMetadata,
  type OpenWorldStorageRequest,
  type WorldStorage,
  type WorldStorageSession,
} from "../storage/world-storage";
import { EntityRuntime } from "./entity-runtime";
import { LiquidSimulationLevel } from "./liquid-simulation-level";
import { floor } from "../../util/mth";

const MOB_STABLE_STANDING_SCAN_UP = 1;
const MOB_STABLE_STANDING_SCAN_DOWN = 4;
const PLAYER_STANDING_EYE_HEIGHT = 1.62;

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

type MutableEntityUpdate = { -readonly [K in keyof EntityUpdate]: EntityUpdate[K] };

function createEntityUpdate(previous: EntitySnapshot, next: EntitySnapshot): EntityUpdateMessage | undefined {
  const update: MutableEntityUpdate = { id: next.id };
  let changed = false;

  if (previous.chunkX !== next.chunkX) {
    update.chunkX = next.chunkX;
    changed = true;
  }
  if (previous.chunkZ !== next.chunkZ) {
    update.chunkZ = next.chunkZ;
    changed = true;
  }
  if (!vec3RecordEquals(previous.position, next.position)) {
    update.position = next.position;
    changed = true;
  }
  if (previous.rotation.yaw !== next.rotation.yaw || previous.rotation.pitch !== next.rotation.pitch) {
    update.rotation = next.rotation;
    changed = true;
  }
  if (previous.width !== next.width) {
    update.width = next.width;
    changed = true;
  }
  if (previous.height !== next.height) {
    update.height = next.height;
    changed = true;
  }
  if (previous.onGround !== next.onGround) {
    update.onGround = next.onGround;
    changed = true;
  }
  if (previous.age !== next.age) {
    update.age = next.age;
    changed = true;
  }
  if (!entityDataEquals(previous.data, next.data)) {
    update.data = next.data;
    changed = true;
  }
  if (changed) {
    update.tick = next.tick;
  }

  return changed ? { type: "entity_update", update } : undefined;
}

function vec3RecordEquals(
  left: { readonly x: number; readonly y: number; readonly z: number },
  right: { readonly x: number; readonly y: number; readonly z: number },
): boolean {
  return left.x === right.x && left.y === right.y && left.z === right.z;
}

function entityDataEquals(
  left: Readonly<Record<string, number | boolean | string>> | undefined,
  right: Readonly<Record<string, number | boolean | string>> | undefined,
): boolean {
  const leftKeys = Object.keys(left ?? {});
  const rightKeys = Object.keys(right ?? {});
  if (leftKeys.length !== rightKeys.length) {
    return false;
  }

  for (const key of leftKeys) {
    if (left?.[key] !== right?.[key]) {
      return false;
    }
  }
  return true;
}

// Bump when persisted generated-world meaning changes: terrain/decor algorithms,
// biome/block-state/tick encodings, metadata compatibility, or future trusted light.
export const GENERATED_WORLD_STORAGE_VERSION = 5;

function createGeneratedWorldConfigSaveSuffix(config: WorldEngineConfig | undefined): string {
  const normalized = normalizeWorldEngineConfig(config);
  if (normalized.liquidSimulationMode === "vanilla17") {
    return "";
  }

  return `-liquid-${normalized.liquidSimulationMode}`;
}

export function createGeneratedWorldSaveId(seed: bigint, preset: OpenWorldPreset, config?: WorldEngineConfig): string {
  return `generated-world-v${GENERATED_WORLD_STORAGE_VERSION.toString()}-${preset}-${seed.toString()}${createGeneratedWorldConfigSaveSuffix(config)}`;
}

function createStorageOpenRequest(
  request: OpenWorldRequest,
  minBuildHeight: number,
  height: number,
  openedAtMs: number,
  config: WorldEngineConfig | undefined,
): OpenWorldStorageRequest {
  return {
    saveId: createGeneratedWorldSaveId(request.seed, request.preset, config),
    storageVersion: GENERATED_WORLD_STORAGE_VERSION,
    seed: request.seed.toString(),
    preset: request.preset,
    minBuildHeight,
    height,
    openedAtMs,
  };
}

export function getGeneratedWorldViewChunkRadius(radius: number): number {
  return Math.max(1, radius) + 1;
}

export function getGeneratedWorldEntityChunkRadius(radius: number): number {
  return Math.max(0, getGeneratedWorldViewChunkRadius(radius) - 2);
}

export function getGeneratedWorldPlayerViewTicketLevel(radius: number): number {
  return GENERATED_CHUNK_BORDER_LEVEL - getGeneratedWorldViewChunkRadius(radius);
}

export function getGeneratedWorldFullChunkRadius(radius: number): number {
  return getGeneratedWorldViewChunkRadius(radius) + 1;
}

export function getGeneratedWorldFeaturesChunkRadius(radius: number): number {
  return getGeneratedWorldFullChunkRadius(radius) + 1;
}

function getGeneratedWorldAuthorityChunkRadius(radius: number): number {
  return getGeneratedWorldFeaturesChunkRadius(radius) + FEATURES_CHUNK_DEPENDENCY_RADIUS;
}

export type GeneratedWorldLightingMode = WorldEngineLightingMode;
export type GeneratedWorldLiquidSimulationMode = WorldEngineLiquidSimulationMode;

export interface GeneratedWorldHostOptions {
  readonly seed: bigint;
  readonly generator?: WorldGenerator;
  readonly airState: BlockState;
  readonly blockStateById: readonly BlockState[];
  readonly blockStateIds: BlockStateIdMap;
  readonly lightingMode?: GeneratedWorldLightingMode;
  readonly liquidSimulationMode?: GeneratedWorldLiquidSimulationMode;
  readonly chunkViewScheduling?: "synchronous" | "cooperative";
  readonly worldTickIntervalMs?: number;
  readonly nowMs?: () => number;
  readonly mutateWorld?: (level: WorldGenLevel) => void;
  readonly worldStorage?: WorldStorage;
  readonly lightingService?: LightingService;
}

const LOCAL_WORLD_SESSION_ID = "local-session";
const LOCAL_PLAYER_ID = "local-player";
const COOPERATIVE_CHUNK_PHASE_BUDGET_MS = 8;
const LIGHTING_RESULT_BATCH_SIZE = 64;
const LIQUID_TICK_READ_RADIUS_BLOCKS = 4;
const STORAGE_SIDE_EFFECT_MAX_CONCURRENCY = 4;
const GLOBAL_STORAGE_SIDE_EFFECT_KEY = "global";
const CHUNK_HOLDER_UNLOADS_PER_PASS = 200;
const CHUNK_HOLDER_UNLOAD_BACKLOG_THRESHOLD = 2_000;
const GENERATED_PLAYER_VIEW_TICKET_SOURCE = "player_view";
const GENERATED_DEPENDENCY_TICKET_SOURCE = "generation_dependency";
const GENERATED_ENTITY_TICKET_SOURCE = "entity";

type GeneratedChunkStatusJobState = "pending" | "fulfilled" | "rejected";

interface GeneratedChunkStatusJob {
  readonly status: GeneratedChunkStatus;
  readonly chunkViewJobRevision: number;
  readonly promise: Promise<boolean>;
  state: GeneratedChunkStatusJobState;
  result: boolean | undefined;
}

interface GeneratedChunkPreloadJob {
  readonly chunkViewJobRevision: number;
  readonly promise: Promise<StoredChunkPreloadResult>;
  state: GeneratedChunkStatusJobState;
  result: StoredChunkPreloadResult | undefined;
}

interface GeneratedChunkHolder {
  readonly chunkX: number;
  readonly chunkZ: number;
  fullStatus: FullChunkStatus;
  preloadJob: GeneratedChunkPreloadJob | undefined;
  readonly statusJobs: Map<GeneratedChunkStatus, GeneratedChunkStatusJob>;
  chunkToSave: GeneratedChunkAccess | undefined;
}

interface GeneratedPendingUnloadHolder {
  readonly holder: GeneratedChunkHolder;
  readonly record: GeneratedChunkStorageRecord;
  readonly savePromise: Promise<void>;
  cancelled: boolean;
}

interface QueuedChunkHolderUnload {
  readonly key: string;
  readonly holder: GeneratedChunkHolder;
  sourceChunk: GeneratedLevelChunk | undefined;
}

interface GeneratedChunkStatusTarget {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly status: GeneratedChunkStatus;
}

export interface GeneratedEntityChunkStatusDebugRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly status: FullChunkStatus;
}

export interface GeneratedChunkFullStatusDebugRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly status: FullChunkStatus;
}

interface EnsureLightingOptions {
  readonly isCancelled?: () => boolean;
  readonly onInitialLightReady?: (chunk: GeneratedLevelChunk) => Promise<void>;
  readonly onInitialLightBatchReady?: () => Promise<void>;
  readonly onLightProgress?: (progress: LightProgressResult) => void;
}

interface PublishReadyChunksOptions {
  readonly targetMessages?: WorldHostMessage[];
  readonly awaitStorage?: boolean;
  readonly onChunkPublished?: (publishedInPass: number) => void;
}

type StoredChunkPreloadResult = "loaded" | "already_loaded" | "missing" | "stale";

type GeneratedLevelChunk = LevelChunk;

type StorageSideEffectTask = () => Promise<void>;

interface QueuedStorageSideEffect {
  readonly task: StorageSideEffectTask;
  readonly key: string;
  readonly resolve: () => void;
}

interface QueuedGeneratedChunkAccessSave {
  readonly record: GeneratedChunkStorageRecord;
  readonly promise: Promise<void>;
}

interface PendingStorageWrite {
  readonly key: string;
  readonly snapshot: PackedChunkSnapshot | undefined;
  readonly generatedRecord: GeneratedChunkStorageRecord | undefined;
  readonly isCurrent: () => boolean;
}

interface RegisteredPendingStorageWrite {
  readonly snapshot: PackedChunkSnapshot | undefined;
  readonly generatedRecord: GeneratedChunkStorageRecord | undefined;
  clear: () => void;
}

interface PendingLightBlockChange {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly oldBlockStateId: number;
  readonly newBlockStateId: number;
}

interface MutableWorldgenPhasePerformanceCounters {
  count: number;
  totalMs: number;
  maxMs: number;
}

interface MutableWorldgenPerformanceCounters {
  readonly phases: Map<string, MutableWorldgenPhasePerformanceCounters>;
  readonly counts: Map<string, number>;
  maxPendingMessages: number;
}

function monotonicNowMs(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

function yieldToEventLoop(): Promise<void> {
  if (typeof process !== "undefined" && process.versions?.node !== undefined) {
    return new Promise((resolve) => {
      setTimeout(resolve, 1);
    });
  }

  if (typeof MessageChannel !== "undefined") {
    return new Promise((resolve) => {
      const channel = new MessageChannel();
      channel.port1.onmessage = () => {
        channel.port1.close();
        channel.port2.close();
        resolve();
      };
      channel.port2.postMessage(undefined);
    });
  }

  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

function withoutPersistedLight(snapshot: PackedChunkSnapshot): PackedChunkSnapshot {
  if (snapshot.light === undefined) {
    return snapshot;
  }

  return {
    chunkX: snapshot.chunkX,
    chunkZ: snapshot.chunkZ,
    biomes: snapshot.biomes,
    sections: snapshot.sections,
    blockTicks: snapshot.blockTicks,
    liquidTicks: snapshot.liquidTicks,
  };
}

function createCooperativeYield(): () => Promise<void> {
  let lastYieldAtMs = monotonicNowMs();
  return async () => {
    const nowMs = monotonicNowMs();
    if (nowMs - lastYieldAtMs < COOPERATIVE_CHUNK_PHASE_BUDGET_MS) {
      return;
    }

    await yieldToEventLoop();
    lastYieldAtMs = monotonicNowMs();
  };
}

function createSessionChunkViewState(request: SetChunkViewRequest | undefined): SessionChunkViewState | undefined {
  if (request === undefined) {
    return undefined;
  }

  return {
    centerChunkX: request.centerChunkX,
    centerChunkZ: request.centerChunkZ,
    radius: request.radius,
  };
}

function createGeneratedLevelCollisionWorld(level: GeneratedRenderLevel): CollisionWorld {
  const loadedWorld = blockGetterCollisionWorld(level);
  return {
    queryBlockCollisions(bounds: AABB) {
      const epsilon = 1.0e-7;
      const minChunkX = SectionPos.blockToSectionCoord(Math.floor(bounds.minX - epsilon));
      const maxChunkX = SectionPos.blockToSectionCoord(Math.floor(bounds.maxX + epsilon));
      const minChunkZ = SectionPos.blockToSectionCoord(Math.floor(bounds.minZ - epsilon));
      const maxChunkZ = SectionPos.blockToSectionCoord(Math.floor(bounds.maxZ + epsilon));
      for (let chunkZ = minChunkZ; chunkZ <= maxChunkZ; chunkZ++) {
        for (let chunkX = minChunkX; chunkX <= maxChunkX; chunkX++) {
          if (level.getAuthorityChunk(chunkX, chunkZ) === null) {
            return {
              type: "missing",
              reason: `missing_authority_chunk:${chunkX.toString()},${chunkZ.toString()}`,
            };
          }
        }
      }

      return loadedWorld.queryBlockCollisions(bounds);
    },
  };
}

export class GeneratedWorldHost implements WorldHost {
  private readonly biomeSource: NoiseBiomeSource;
  private readonly generator: WorldGenerator;
  private readonly level;
  private readonly lightingService: LightingService | undefined;
  private readonly liquidLevel: LiquidSimulationLevel;
  private readonly liquidTicks: ServerTickList<Fluid>;
  private readonly entityRuntime: EntityRuntime<GeneratedMobEntity>;
  private readonly engineConfig: NormalizedWorldEngineConfig;
  private readonly liquidSimulationEnabled: boolean;
  private readonly nowMs: () => number;
  private readonly resolveBlockState;
  private storageSession: WorldStorageSession | undefined;
  private worldOpened: WorldOpenedMessage | undefined;
  private sessionState: ClientSessionState | undefined;
  private playerState: ClientPlayerState | undefined;
  private readonly playerCommandQueue: PlayerCommandQueue = createPlayerCommandQueue();
  private currentChunkView: SetChunkViewRequest | undefined;
  private pendingMessages: WorldHostMessage[] = [];
  private gameTime = 0;
  private lastWorldTickAtMs: number;
  private lastPlayerTickAtMs: number;
  private playerAnchoredToChunkView = false;
  private readonly publishedChunkSnapshots = new Set<string>();
  private readonly publishedEntitySnapshots = new Map<number, EntitySnapshot>();
  private readonly entityChunkStatuses = new Map<string, FullChunkStatus>();
  private readonly chunkRevisions = new Map<string, number>();
  private readonly sentLightInputRevisions = new Map<string, number>();
  private readonly acceptedChunkLight = new Map<string, PackedChunkLight>();
  private readonly acceptedChunkLightRevisions = new Map<string, number>();
  private readonly pendingLightBlockChanges = new Map<bigint, PendingLightBlockChange>();
  private readonly dirtyChunksForPublication = new Set<string>();
  private readonly dirtyDurableChunks = new Set<string>();
  private readonly spawnedOriginalMobChunks = new Set<string>();
  private readonly chunkHolders = new Map<string, GeneratedChunkHolder>();
  private readonly chunkResidencyTickets = new GeneratedChunkTicketSet();
  private readonly chunkHolderUnloadQueue = new Map<string, QueuedChunkHolderUnload>();
  private readonly pendingUnloadChunkHolders = new Map<string, GeneratedPendingUnloadHolder>();
  private readonly generatedCacheWriteVersions = new Map<string, number>();
  private readonly generatedChunkRecordWriteVersions = new Map<string, number>();
  private readonly pendingStorageWrites = new Map<string, PendingStorageWrite>();
  private readonly storageSideEffectKeyTails = new Map<string, Promise<void>>();
  private readonly readyStorageSideEffects: QueuedStorageSideEffect[] = [];
  private readonly storageSideEffectIdleResolvers: Array<() => void> = [];
  private pendingStorageSideEffects = 0;
  private activeStorageSideEffects = 0;
  private readonly worldgenPerformance: MutableWorldgenPerformanceCounters = {
    phases: new Map(),
    counts: new Map(),
    maxPendingMessages: 0,
  };
  private opened = false;
  private chunkViewJobRevision = 0;
  private activeChunkViewJobRevision: number | undefined;
  private nextLightBlockChangeBatchId = 1;
  private nextGeneratedEntityId = 1;

  public constructor(private readonly options: GeneratedWorldHostOptions) {
    this.engineConfig = normalizeWorldEngineConfig({
      lightingMode: options.lightingMode,
      liquidSimulationMode: options.liquidSimulationMode,
    });
    this.nowMs = options.nowMs ?? (() => Date.now());
    this.lastWorldTickAtMs = this.nowMs();
    this.lastPlayerTickAtMs = this.nowMs();
    this.generator = options.generator ?? new NoiseBasedChunkGenerator(new OverworldBiomeSource(options.seed), options.seed);
    this.biomeSource = this.generator.getBiomeSource();
    this.resolveBlockState = createBlockStateResolver(options.airState);
    this.level = new GeneratedRenderLevel(
      options.airState,
      this.generator,
      options.blockStateById,
    );
    this.entityRuntime = new EntityRuntime<GeneratedMobEntity>({
      tickEntity: (entity) => this.tickGeneratedEntity(entity),
    });
    this.level.setPhaseRecorder((phase, elapsedMs) => this.recordWorldgenDuration(phase, elapsedMs));
    this.lightingService = this.engineConfig.lightingMode === "none" ? undefined : options.lightingService;
    if (this.engineConfig.lightingMode !== "none" && this.lightingService === undefined) {
      throw new Error("GeneratedWorldHost requires a worker-backed LightingService when lightingMode is vanilla17");
    }
    this.liquidSimulationEnabled = this.engineConfig.liquidSimulationMode !== "none";
    this.liquidTicks = new ServerTickList<Fluid>(
      (fluid) => fluid === Fluids.EMPTY,
      () => this.gameTime,
      (tick) => this.tickLiquid(tick),
      (pos) => this.isLiquidPositionTicking(pos),
    );
    this.liquidLevel = new LiquidSimulationLevel({
      level: this.level,
      airState: options.airState,
      liquidTicks: this.liquidTicks,
      onBlockChanged: (pos, oldState, newState) => this.markHostBlockChanged(pos, oldState, newState),
    });
  }

  public async openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    if (request.seed !== this.options.seed) {
      return [{ type: "world_error", message: `open_world seed ${request.seed} did not match host seed ${this.options.seed}` }];
    }

    if (this.worldOpened === undefined) {
      const openedAtMs = Date.now();
      const storageRequest = createStorageOpenRequest(
        request,
        this.level.getMinBuildHeight(),
        this.level.getHeight(),
        openedAtMs,
        this.engineConfig,
      );
      if (this.options.worldStorage !== undefined) {
        this.storageSession = await this.options.worldStorage.openWorld(storageRequest);
      }

      this.worldOpened = {
        type: "world_opened",
        minBuildHeight: this.level.getMinBuildHeight(),
        height: this.level.getHeight(),
        saveMetadata: this.storageSession?.metadata ?? createWorldSaveMetadata(storageRequest),
      };
      await this.lightingService?.configureWorld({
        type: "configure_light_world",
        seed: this.options.seed,
        minBuildHeight: this.level.getMinBuildHeight(),
        height: this.level.getHeight(),
        blockRegistryVersion: this.options.blockStateById.length,
      });
    }

    this.opened = true;
    this.ensureLocalSessionState();
    return [
      this.worldOpened,
      this.createSessionStateMessage(),
      this.createPlayerStateMessage(),
    ];
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    if (!this.opened) {
      return [{ type: "world_error", message: "set_chunk_view received before open_world" }];
    }

    this.ensureLocalSessionState();
    this.tickPlayerLoop();
    await this.tickWorldLoop();
    if (this.options.chunkViewScheduling === "cooperative") {
      return this.setChunkViewCooperative(request);
    }

    this.updateChunkResidencyTickets(request);
    const update = this.level.updateChunkView(request.centerChunkX, request.centerChunkZ, request.radius);
    const chunkViewChanged = this.currentChunkView === undefined
      || this.currentChunkView.centerChunkX !== request.centerChunkX
      || this.currentChunkView.centerChunkZ !== request.centerChunkZ
      || this.currentChunkView.radius !== request.radius;
    this.currentChunkView = request;
    if (chunkViewChanged) {
      this.chunkViewJobRevision++;
      this.updateSessionState();
    }
    const chunkViewJobRevision = this.chunkViewJobRevision;
    await this.setLightingView(request, chunkViewJobRevision);
    if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
      this.incrementWorldgenCount("chunk_view_jobs_cancelled");
      return [this.createSessionStateMessage()];
    }
    this.queueChunkHolderUnloadsOutsideCurrentAuthority(update.removedChunks);

    const messages: WorldHostMessage[] = [this.createSessionStateMessage()];
    if (!this.playerAnchoredToChunkView) {
      this.playerState = anchorPlayerStateToChunkView(this.playerState!, createSessionChunkViewState(request)!, this.playerState!.tick);
      this.playerAnchoredToChunkView = true;
      messages.push(this.createPlayerStateMessage());
    }

    if (!update.changed) {
      return messages;
    }

    await this.ensureStatusTargetsForCurrentView(undefined, chunkViewJobRevision);
    if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
      this.incrementWorldgenCount("chunk_view_jobs_cancelled");
      return messages;
    }

    this.options.mutateWorld?.(this.liquidLevel);
    await this.ensureFullStatusesForCurrentView(chunkViewJobRevision);
    if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
      this.incrementWorldgenCount("chunk_view_jobs_cancelled");
      return messages;
    }

    for (const chunk of update.removedChunks) {
      await this.saveDirtyChunkBeforeUnload(chunk);
      this.queueStorageSideEffect(() => this.evictStoredChunk(chunk.chunkX, chunk.chunkZ), chunkKey(chunk.chunkX, chunk.chunkZ));
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
      this.incrementWorldgenCount("chunks_removed");
    }

    for (const chunk of update.unloadedChunks) {
      messages.push(...this.createEntityRemoveMessagesForChunk(chunk.chunkX, chunk.chunkZ));
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
      messages.push({
        type: "chunk_unload",
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
      });
      this.incrementWorldgenCount("chunk_unloads_sent");
    }

    await this.publishReadyChunksForCurrentView(this.chunkViewJobRevision, undefined, {
      targetMessages: messages,
      awaitStorage: true,
    });

    return messages;
  }

  private async setChunkViewCooperative(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    this.updateChunkResidencyTickets(request);
    const update = this.level.updateChunkView(request.centerChunkX, request.centerChunkZ, request.radius);
    const chunkViewChanged = this.currentChunkView === undefined
      || this.currentChunkView.centerChunkX !== request.centerChunkX
      || this.currentChunkView.centerChunkZ !== request.centerChunkZ
      || this.currentChunkView.radius !== request.radius;
    this.currentChunkView = request;
    if (chunkViewChanged) {
      this.updateSessionState();
    }

    const messages: WorldHostMessage[] = [this.createSessionStateMessage()];
    if (!this.playerAnchoredToChunkView) {
      this.playerState = anchorPlayerStateToChunkView(this.playerState!, createSessionChunkViewState(request)!, this.playerState!.tick);
      this.playerAnchoredToChunkView = true;
      messages.push(this.createPlayerStateMessage());
    }

    this.queueChunkHolderUnloadsOutsideCurrentAuthority(update.removedChunks);

    for (const chunk of update.removedChunks) {
      this.queueStorageSideEffect(() => this.queueEvictAfterDirtySave(chunk), chunkKey(chunk.chunkX, chunk.chunkZ));
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
      this.incrementWorldgenCount("chunks_removed");
    }

    for (const chunk of update.unloadedChunks) {
      messages.push(...this.createEntityRemoveMessagesForChunk(chunk.chunkX, chunk.chunkZ));
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
      messages.push({
        type: "chunk_unload",
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
      });
      this.incrementWorldgenCount("chunk_unloads_sent");
    }

    const chunkViewJobRevision = chunkViewChanged ? ++this.chunkViewJobRevision : this.chunkViewJobRevision;
    await this.setLightingView(request, chunkViewJobRevision);
    if (!update.changed) {
      return messages;
    }

    this.incrementWorldgenCount("chunk_view_jobs_scheduled");
    const chunkViewJob = this.runChunkViewJobs(chunkViewJobRevision);
    chunkViewJob.catch((error: unknown) => {
      this.enqueueWorldError(error);
    });
    return messages;
  }

  public async setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    if (!this.opened) {
      return [{ type: "world_error", message: "set_player_input received before open_world" }];
    }

    this.ensureLocalSessionState();
    this.tickPlayerLoop();
    await this.tickWorldLoop();
    if (enqueuePlayerInputCommand(this.playerCommandQueue, this.playerState!, request.input)) {
      this.updateSessionState();
    }
    return [this.createSessionStateMessage()];
  }

  public async pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    if (!this.opened) {
      return [{ type: "world_error", message: "poll_world_updates received before open_world" }];
    }

    this.ensureLocalSessionState();
    this.tickPlayerLoop();
    if (this.pendingMessages.length > 0) {
      return this.drainPendingMessages(request.maxMessages);
    }

    await this.tickWorldLoop();
    return this.drainPendingMessages(request.maxMessages);
  }

  public close(): void {
    void this.flushStorageSideEffects()
      .then(() => this.storageSession?.close())
      .catch((error: unknown) => {
        this.enqueueWorldError(error);
      });
    this.lightingService?.close?.();
  }

  public async flushStorageSideEffects(): Promise<void> {
    this.processChunkHolderUnloadQueue({ drain: true });
    this.queueSaveResidentGeneratedChunkAccesses();
    while (this.pendingStorageSideEffects > 0) {
      await new Promise<void>((resolve) => {
        this.storageSideEffectIdleResolvers.push(resolve);
      });
    }
  }

  public getDebugChunkStatusRecords(): readonly GeneratedChunkStatusDebugRecord[] {
    return this.level.getDebugChunkStatusRecords();
  }

  public getDebugGeneratedChunkAccessRecords(): readonly GeneratedChunkAccessDebugRecord[] {
    return [...this.chunkHolders.values()]
      .map((holder) => holder.chunkToSave)
      .filter((access): access is GeneratedChunkAccess => access !== undefined)
      .map((access) => ({
        chunkX: access.chunkX,
        chunkZ: access.chunkZ,
        type: access.type,
        status: access.status,
        hasBlockSections: access.hasBlockSections,
        isUnsaved: access.isUnsaved,
        contentVersion: access.contentVersion,
      }))
      .sort((left, right) => left.chunkZ - right.chunkZ || left.chunkX - right.chunkX);
  }

  public getDebugEntityChunkStatusRecords(): readonly GeneratedEntityChunkStatusDebugRecord[] {
    return [...this.entityChunkStatuses]
      .map(([key, status]) => {
        const [chunkXRaw, chunkZRaw] = key.split(",");
        return {
          chunkX: Number(chunkXRaw),
          chunkZ: Number(chunkZRaw),
          status,
        };
      })
      .sort((left, right) => left.chunkZ - right.chunkZ || left.chunkX - right.chunkX);
  }

  public getDebugChunkFullStatusRecords(): readonly GeneratedChunkFullStatusDebugRecord[] {
    return [...this.chunkHolders.values()]
      .map((holder) => ({
        chunkX: holder.chunkX,
        chunkZ: holder.chunkZ,
        status: holder.fullStatus,
      }))
      .sort((left, right) => left.chunkZ - right.chunkZ || left.chunkX - right.chunkX);
  }

  public getDebugChunkTicketRecords(): readonly GeneratedChunkTicketDebugRecord[] {
    return this.chunkResidencyTickets.getDebugRecords();
  }

  private updateChunkResidencyTickets(request: SetChunkViewRequest): void {
    const previousSignature = this.chunkResidencyTickets.getSignature();
    this.chunkResidencyTickets.replaceSource(GENERATED_PLAYER_VIEW_TICKET_SOURCE, [{
      source: GENERATED_PLAYER_VIEW_TICKET_SOURCE,
      centerChunkX: request.centerChunkX,
      centerChunkZ: request.centerChunkZ,
      radius: getGeneratedWorldViewChunkRadius(request.radius),
      level: getGeneratedWorldPlayerViewTicketLevel(request.radius),
    }]);
    this.chunkResidencyTickets.replaceSource(GENERATED_ENTITY_TICKET_SOURCE, [{
      source: GENERATED_ENTITY_TICKET_SOURCE,
      centerChunkX: request.centerChunkX,
      centerChunkZ: request.centerChunkZ,
      radius: getGeneratedWorldEntityChunkRadius(request.radius),
      level: GENERATED_CHUNK_ENTITY_TICKING_LEVEL,
    }]);
    this.chunkResidencyTickets.replaceSource(GENERATED_DEPENDENCY_TICKET_SOURCE, [{
      source: GENERATED_DEPENDENCY_TICKET_SOURCE,
      centerChunkX: request.centerChunkX,
      centerChunkZ: request.centerChunkZ,
      radius: getGeneratedWorldAuthorityChunkRadius(request.radius),
    }]);
    const nextSignature = this.chunkResidencyTickets.getSignature();
    if (nextSignature !== previousSignature) {
      this.incrementWorldgenCount("chunk_residency_tickets_updated");
    }
    this.noteChunkResidencyTicketCount();
    this.syncChunkHolderFullStatuses(request);
  }

  private syncChunkHolderFullStatuses(request: SetChunkViewRequest): void {
    const keys = new Set(this.entityChunkStatuses.keys());
    for (const [key, holder] of this.chunkHolders) {
      if (holder.fullStatus !== FullChunkStatus.INACCESSIBLE) {
        keys.add(key);
      }
    }
    this.addChunkSquareKeys(
      keys,
      request.centerChunkX,
      request.centerChunkZ,
      getGeneratedWorldViewChunkRadius(request.radius),
    );
    this.addChunkSquareKeys(
      keys,
      request.centerChunkX,
      request.centerChunkZ,
      getGeneratedWorldEntityChunkRadius(request.radius),
    );

    let changed = false;
    for (const key of [...keys].sort()) {
      const [chunkXRaw, chunkZRaw] = key.split(",");
      const chunkX = Number(chunkXRaw);
      const chunkZ = Number(chunkZRaw);
      const status = this.getCurrentChunkHolderFullStatus(chunkX, chunkZ);
      const holder = this.chunkHolders.get(key) ?? (status === FullChunkStatus.INACCESSIBLE ? undefined : this.getChunkHolder(chunkX, chunkZ));
      changed = holder === undefined
        ? this.applyEntityChunkStatus(chunkX, chunkZ, FullChunkStatus.INACCESSIBLE) || changed
        : this.updateChunkHolderFullStatus(holder, status) || changed;
    }

    if (changed) {
      this.entityRuntime.processLifecycle();
    }
    this.noteChunkFullStatusCounts();
    this.noteEntityChunkStatusCounts();
  }

  private addChunkSquareKeys(keys: Set<string>, centerChunkX: number, centerChunkZ: number, radius: number): void {
    for (let chunkZ = centerChunkZ - radius; chunkZ <= centerChunkZ + radius; chunkZ++) {
      for (let chunkX = centerChunkX - radius; chunkX <= centerChunkX + radius; chunkX++) {
        keys.add(chunkKey(chunkX, chunkZ));
      }
    }
  }

  private getCurrentChunkHolderFullStatus(chunkX: number, chunkZ: number): FullChunkStatus {
    return this.chunkResidencyTickets.getFullStatus(chunkX, chunkZ);
  }

  private updateChunkHolderFullStatus(holder: GeneratedChunkHolder, status: FullChunkStatus): boolean {
    if (holder.fullStatus === status) {
      return false;
    }

    holder.fullStatus = status;
    this.incrementWorldgenCount(`chunk_full_status_updates.${status}`);
    return this.applyEntityChunkStatus(holder.chunkX, holder.chunkZ, status);
  }

  private applyEntityChunkStatus(chunkX: number, chunkZ: number, status: FullChunkStatus): boolean {
    const key = chunkKey(chunkX, chunkZ);
    const previous = this.entityChunkStatuses.get(key);
    if (previous === status) {
      return false;
    }
    if (previous === undefined && status === FullChunkStatus.INACCESSIBLE) {
      return false;
    }

    if (status === FullChunkStatus.INACCESSIBLE) {
      this.entityChunkStatuses.delete(key);
    } else {
      this.entityChunkStatuses.set(key, status);
    }
    this.entityRuntime.updateChunkStatus(chunkX, chunkZ, status);
    this.incrementWorldgenCount(`entity_chunk_status_updates.${status}`);
    return true;
  }

  private getChunkHolder(chunkX: number, chunkZ: number): GeneratedChunkHolder {
    const key = chunkKey(chunkX, chunkZ);
    const existing = this.chunkHolders.get(key);
    if (existing !== undefined) {
      if (this.isChunkInCurrentAuthorityView(chunkX, chunkZ)) {
        this.cancelQueuedChunkHolderUnload(key, true);
      }
      return existing;
    }

    const pendingUnload = this.pendingUnloadChunkHolders.get(key);
    if (pendingUnload !== undefined) {
      pendingUnload.cancelled = true;
      this.pendingUnloadChunkHolders.delete(key);
      this.chunkHolders.set(key, pendingUnload.holder);
      this.restoreStoredGeneratedChunkIntoLevel(pendingUnload.record);
      this.incrementWorldgenCount("chunk_holders_pending_unload_resurrected");
      this.notePendingUnloadChunkHolderCount();
      this.noteChunkHolderCount();
      this.noteChunkFullStatusCounts();
      return pendingUnload.holder;
    }

    const holder: GeneratedChunkHolder = {
      chunkX,
      chunkZ,
      fullStatus: FullChunkStatus.INACCESSIBLE,
      preloadJob: undefined,
      statusJobs: new Map(),
      chunkToSave: undefined,
    };
    this.chunkHolders.set(key, holder);
    this.noteChunkHolderCount();
    this.noteChunkFullStatusCounts();
    return holder;
  }

  private queueChunkHolderUnloadsOutsideCurrentAuthority(removedChunks: readonly GeneratedLevelChunk[] = []): void {
    const removedChunkByKey = new Map(removedChunks.map((chunk) => [chunkKey(chunk.chunkX, chunk.chunkZ), chunk] as const));
    let queued = 0;
    for (const [key, holder] of this.chunkHolders) {
      if (this.isChunkInCurrentAuthorityView(holder.chunkX, holder.chunkZ)) {
        this.cancelQueuedChunkHolderUnload(key, true);
        continue;
      }

      const sourceChunk = removedChunkByKey.get(key);
      const queuedUnload = this.chunkHolderUnloadQueue.get(key);
      if (queuedUnload !== undefined) {
        if (sourceChunk !== undefined && queuedUnload.sourceChunk === undefined) {
          queuedUnload.sourceChunk = sourceChunk;
        }
        continue;
      }

      this.chunkHolderUnloadQueue.set(key, {
        key,
        holder,
        sourceChunk,
      });
      queued++;
    }

    if (queued > 0) {
      this.incrementWorldgenCount("chunk_holders_unload_queued", queued);
    }
    this.noteChunkHolderUnloadQueueCount();
    this.processChunkHolderUnloadQueue();
  }

  private processChunkHolderUnloadQueue(options: { readonly drain?: boolean } = {}): void {
    let processed = 0;
    let skippedStale = 0;
    while (
      this.chunkHolderUnloadQueue.size > 0
      && (
        options.drain === true
        || processed < CHUNK_HOLDER_UNLOADS_PER_PASS
        || this.chunkHolderUnloadQueue.size > CHUNK_HOLDER_UNLOAD_BACKLOG_THRESHOLD
      )
    ) {
      const next = this.chunkHolderUnloadQueue.entries().next().value as [string, QueuedChunkHolderUnload] | undefined;
      if (next === undefined) {
        break;
      }

      const [key, queuedUnload] = next;
      this.chunkHolderUnloadQueue.delete(key);
      if (this.isChunkInCurrentAuthorityView(queuedUnload.holder.chunkX, queuedUnload.holder.chunkZ)) {
        this.incrementWorldgenCount("chunk_holders_unload_cancelled");
        this.restoreQueuedChunkHolderUnload(queuedUnload);
        continue;
      }

      if (this.chunkHolders.get(key) !== queuedUnload.holder) {
        skippedStale++;
        continue;
      }

      if (this.updateChunkHolderFullStatus(queuedUnload.holder, FullChunkStatus.INACCESSIBLE)) {
        this.entityRuntime.processLifecycle();
        this.noteEntityChunkStatusCounts();
      }
      this.schedulePendingUnloadChunkHolder(key, queuedUnload.holder, queuedUnload.sourceChunk);
      this.chunkHolders.delete(key);
      this.generatedCacheWriteVersions.delete(key);
      processed++;
    }

    if (processed > 0) {
      this.incrementWorldgenCount("chunk_holders_unload_processed", processed);
      this.incrementWorldgenCount("chunk_holders_pruned_outside_authority", processed);
    }
    if (skippedStale > 0) {
      this.incrementWorldgenCount("chunk_holders_unload_skipped_stale", skippedStale);
    }
    this.noteChunkHolderUnloadQueueCount();
    this.noteChunkHolderCount();
    this.noteChunkFullStatusCounts();
  }

  private cancelQueuedChunkHolderUnload(key: string, restore: boolean): boolean {
    const queuedUnload = this.chunkHolderUnloadQueue.get(key);
    if (queuedUnload === undefined) {
      return false;
    }

    this.chunkHolderUnloadQueue.delete(key);
    this.incrementWorldgenCount("chunk_holders_unload_cancelled");
    this.noteChunkHolderUnloadQueueCount();
    if (restore) {
      this.restoreQueuedChunkHolderUnload(queuedUnload);
    }
    return true;
  }

  private restoreQueuedChunkHolderUnload(queuedUnload: QueuedChunkHolderUnload): void {
    const access = queuedUnload.holder.chunkToSave;
    if (access === undefined) {
      return;
    }

    const record = this.createGeneratedChunkStorageRecordForAccess(
      access,
      queuedUnload.sourceChunk,
      this.generatedChunkRecordWriteVersions.get(queuedUnload.key) ?? 0,
    );
    if (record === undefined) {
      this.incrementWorldgenCount("chunk_holders_unload_restore_skipped");
      return;
    }

    if (this.restoreStoredGeneratedChunkIntoLevel(record)) {
      this.incrementWorldgenCount("chunk_holders_unload_restored");
    } else {
      this.incrementWorldgenCount("chunk_holders_unload_restore_rejected");
    }
  }

  private schedulePendingUnloadChunkHolder(
    key: string,
    holder: GeneratedChunkHolder,
    sourceChunk?: GeneratedLevelChunk,
  ): void {
    if (this.updateChunkHolderFullStatus(holder, FullChunkStatus.INACCESSIBLE)) {
      this.entityRuntime.processLifecycle();
      this.noteEntityChunkStatusCounts();
    }
    const queuedSave = this.queueSaveGeneratedChunkAccess(holder.chunkToSave, sourceChunk, holder);
    if (queuedSave === undefined) {
      return;
    }

    const pendingUnload: GeneratedPendingUnloadHolder = {
      holder,
      record: queuedSave.record,
      savePromise: queuedSave.promise,
      cancelled: false,
    };
    this.pendingUnloadChunkHolders.set(key, pendingUnload);
    this.incrementWorldgenCount("chunk_holders_pending_unload_scheduled");
    this.notePendingUnloadChunkHolderCount();
    void queuedSave.promise.finally(() => {
      if (this.pendingUnloadChunkHolders.get(key) !== pendingUnload || pendingUnload.cancelled) {
        return;
      }

      this.pendingUnloadChunkHolders.delete(key);
      this.incrementWorldgenCount("chunk_holders_pending_unload_completed");
      this.notePendingUnloadChunkHolderCount();
    });
  }

  private recordGeneratedChunkAccessStatus(chunkX: number, chunkZ: number): void {
    if (!this.isChunkInCurrentAuthorityView(chunkX, chunkZ)) {
      return;
    }

    const holder = this.getChunkHolder(chunkX, chunkZ);
    const access = holder.chunkToSave ?? createGeneratedProtoChunk(chunkX, chunkZ);
    const previousType = access.type;
    const previousStatus = access.status;
    const previousHasBlockSections = access.hasBlockSections;
    const previousIsUnsaved = access.isUnsaved;
    holder.chunkToSave = advanceGeneratedChunkAccessStatus(
      access,
      this.level.getChunkStatus(chunkX, chunkZ),
      this.level.hasMaterializedChunk(chunkX, chunkZ),
    );
    if (
      holder.chunkToSave.type !== previousType
      || holder.chunkToSave.status !== previousStatus
      || holder.chunkToSave.hasBlockSections !== previousHasBlockSections
      || holder.chunkToSave.isUnsaved !== previousIsUnsaved
    ) {
      this.incrementWorldgenCount(`chunk_to_save_updated.${holder.chunkToSave.status}`);
    }
  }

  private async ensureGeneratedChunkStatus(
    chunkX: number,
    chunkZ: number,
    status: GeneratedChunkStatus,
    run: () => Promise<boolean>,
    chunkViewJobRevision = this.chunkViewJobRevision,
  ): Promise<boolean> {
    this.incrementWorldgenCount(`status_requests.${status}`);
    if (this.level.hasChunkStatus(chunkX, chunkZ, status)) {
      this.recordGeneratedChunkAccessStatus(chunkX, chunkZ);
      this.incrementWorldgenCount(`status_reused_completed.${status}`);
      return true;
    }

    if (!this.isChunkInCurrentAuthorityView(chunkX, chunkZ)) {
      this.incrementWorldgenCount(`status_skipped_outside_authority.${status}`);
      return false;
    }

    const holder = this.getChunkHolder(chunkX, chunkZ);
    const existing = holder.statusJobs.get(status);
    if (existing !== undefined) {
      if (existing.state === "pending" && existing.chunkViewJobRevision === chunkViewJobRevision) {
        this.incrementWorldgenCount(`status_coalesced_pending.${status}`);
        return existing.promise;
      }

      if (existing.state === "pending") {
        this.incrementWorldgenCount(`status_not_coalesced_revision_changed.${status}`);
      }

      if (existing.state === "fulfilled" && existing.result === true && this.level.hasChunkStatus(chunkX, chunkZ, status)) {
        this.incrementWorldgenCount(`status_reused_completed.${status}`);
        return true;
      }

      holder.statusJobs.delete(status);
    }

    let resolveJob!: (result: boolean) => void;
    let rejectJob!: (error: unknown) => void;
    const promise = new Promise<boolean>((resolve, reject) => {
      resolveJob = resolve;
      rejectJob = reject;
    });
    const job: GeneratedChunkStatusJob = {
      status,
      chunkViewJobRevision,
      promise,
      state: "pending",
      result: undefined,
    };
    holder.statusJobs.set(status, job);

    void (async () => {
      this.incrementWorldgenCount(`status_jobs_started.${status}`);
      try {
        const result = await run();
        job.state = "fulfilled";
        job.result = result;
        if (result) {
          this.recordGeneratedChunkAccessStatus(chunkX, chunkZ);
        }
        this.incrementWorldgenCount(result ? `status_jobs_completed.${status}` : `status_jobs_skipped.${status}`);
        if (!result && holder.statusJobs.get(status) === job) {
          holder.statusJobs.delete(status);
        }
        resolveJob(result);
      } catch (error) {
        job.state = "rejected";
        job.result = false;
        if (holder.statusJobs.get(status) === job) {
          holder.statusJobs.delete(status);
        }
        this.incrementWorldgenCount(`status_jobs_failed.${status}`);
        rejectJob(error);
      }
    })();

    return promise;
  }

  private async ensureFullStatusWithoutLighting(
    chunk: GeneratedLevelChunk,
    chunkViewJobRevision = this.chunkViewJobRevision,
  ): Promise<boolean> {
    return this.ensureGeneratedChunkStatus(
      chunk.chunkX,
      chunk.chunkZ,
      GeneratedChunkStatus.FULL,
      async () => this.markChunkFullWithoutLighting(chunk),
      chunkViewJobRevision,
    );
  }

  private async ensureFullStatusWithoutLightingCooperative(
    chunk: GeneratedLevelChunk,
    yieldStep: () => Promise<void>,
    chunkViewJobRevision = this.chunkViewJobRevision,
  ): Promise<boolean> {
    return this.ensureGeneratedChunkStatus(
      chunk.chunkX,
      chunk.chunkZ,
      GeneratedChunkStatus.FULL,
      async () => {
        await yieldStep();
        return this.markChunkFullWithoutLighting(chunk);
      },
      chunkViewJobRevision,
    );
  }

  private async runTerrainStatus(chunkX: number, chunkZ: number): Promise<boolean> {
    const generated = this.recordWorldgenPhase("host.generate_chunk_terrain", () => this.level.generateChunkTerrain(chunkX, chunkZ));
    if (generated !== null) {
      this.incrementWorldgenCount("terrain_chunks_generated");
    }

    return this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS);
  }

  private async runTerrainStatusCooperative(
    chunkX: number,
    chunkZ: number,
    yieldStep: () => Promise<void>,
  ): Promise<boolean> {
    const generated = await this.recordWorldgenPhaseAsync("host.generate_chunk_terrain", () =>
      this.level.generateChunkTerrainCooperative(chunkX, chunkZ, yieldStep)
    );
    if (generated !== null) {
      this.incrementWorldgenCount("terrain_chunks_generated");
    }

    return this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS);
  }

  private async runFeaturesStatus(chunkX: number, chunkZ: number): Promise<boolean> {
    this.recordWorldgenPhase("host.decorate_chunk", () => this.level.decorateChunk(chunkX, chunkZ));
    if (this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)) {
      this.incrementWorldgenCount("feature_chunks_decorated");
      return true;
    }

    return false;
  }

  private async runFeaturesStatusCooperative(
    chunkX: number,
    chunkZ: number,
    yieldStep: () => Promise<void>,
  ): Promise<boolean> {
    await this.recordWorldgenPhaseAsync("host.decorate_chunk", () =>
      this.level.decorateChunkCooperative(chunkX, chunkZ, yieldStep)
    );
    if (this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)) {
      this.incrementWorldgenCount("feature_chunks_decorated");
      return true;
    }

    return false;
  }

  private collectCurrentStatusTargets(): readonly GeneratedChunkStatusTarget[] {
    if (this.currentChunkView === undefined) {
      return [];
    }

    const targets: GeneratedChunkStatusTarget[] = [];
    const featuresRadius = getGeneratedWorldFeaturesChunkRadius(this.currentChunkView.radius);
    for (let chunkZ = this.currentChunkView.centerChunkZ - featuresRadius; chunkZ <= this.currentChunkView.centerChunkZ + featuresRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - featuresRadius; chunkX <= this.currentChunkView.centerChunkX + featuresRadius; chunkX++) {
        if (!this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)) {
          targets.push({ chunkX, chunkZ, status: GeneratedChunkStatus.FEATURES });
        }
      }
    }

    if (this.lightingService !== undefined) {
      return targets;
    }

    const fullRadius = getGeneratedWorldFullChunkRadius(this.currentChunkView.radius);
    for (let chunkZ = this.currentChunkView.centerChunkZ - fullRadius; chunkZ <= this.currentChunkView.centerChunkZ + fullRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - fullRadius; chunkX <= this.currentChunkView.centerChunkX + fullRadius; chunkX++) {
        if (!this.level.isChunkFull(chunkX, chunkZ)) {
          targets.push({ chunkX, chunkZ, status: GeneratedChunkStatus.FULL });
        }
      }
    }

    return targets;
  }

  private async ensureStatusTargetsForCurrentView(
    yieldStep?: () => Promise<void>,
    chunkViewJobRevision = this.chunkViewJobRevision,
  ): Promise<number> {
    let completed = 0;
    for (const target of this.collectCurrentStatusTargets()) {
      await yieldStep?.();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return completed;
      }

      if (await this.ensureChunkStatusRecursive(target.chunkX, target.chunkZ, target.status, yieldStep, chunkViewJobRevision)) {
        completed++;
      }
    }

    return completed;
  }

  private async ensureChunkStatusRecursive(
    chunkX: number,
    chunkZ: number,
    status: GeneratedChunkStatus,
    yieldStep?: () => Promise<void>,
    chunkViewJobRevision = this.chunkViewJobRevision,
  ): Promise<boolean> {
    return this.ensureGeneratedChunkStatus(chunkX, chunkZ, status, async () => {
      switch (status) {
        case GeneratedChunkStatus.EMPTY:
          return true;
        case GeneratedChunkStatus.STRUCTURE_STARTS:
        case GeneratedChunkStatus.STRUCTURE_REFERENCES:
        case GeneratedChunkStatus.BIOMES:
          return this.level.markMetadataStatusAtLeast(chunkX, chunkZ, status);
        case GeneratedChunkStatus.NOISE:
        case GeneratedChunkStatus.SURFACE:
        case GeneratedChunkStatus.CARVERS:
        case GeneratedChunkStatus.LIQUID_CARVERS:
          return this.generateTerrainStatusRecursive(chunkX, chunkZ, yieldStep, chunkViewJobRevision);
        case GeneratedChunkStatus.FEATURES:
          return this.generateFeaturesStatusRecursive(chunkX, chunkZ, yieldStep, chunkViewJobRevision);
        case GeneratedChunkStatus.LIGHT:
        case GeneratedChunkStatus.SPAWN:
        case GeneratedChunkStatus.HEIGHTMAPS:
        case GeneratedChunkStatus.FULL:
          return this.generateFullStatusRecursive(chunkX, chunkZ, yieldStep, chunkViewJobRevision);
      }
    }, chunkViewJobRevision);
  }

  private async generateTerrainStatusRecursive(
    chunkX: number,
    chunkZ: number,
    yieldStep?: () => Promise<void>,
    chunkViewJobRevision = this.chunkViewJobRevision,
  ): Promise<boolean> {
    const preloadResult = await this.preloadStoredChunkCoalesced(chunkX, chunkZ, chunkViewJobRevision);
    if (preloadResult === "stale") {
      return false;
    }
    if (
      (preloadResult === "loaded" || preloadResult === "already_loaded")
      && this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS)
    ) {
      return this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS);
    }

    if (yieldStep === undefined) {
      return this.runTerrainStatus(chunkX, chunkZ);
    }

    return this.runTerrainStatusCooperative(chunkX, chunkZ, yieldStep);
  }

  private async generateFeaturesStatusRecursive(
    chunkX: number,
    chunkZ: number,
    yieldStep?: () => Promise<void>,
    chunkViewJobRevision = this.chunkViewJobRevision,
  ): Promise<boolean> {
    for (let neighborChunkZ = chunkZ - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ <= chunkZ + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX <= chunkX + FEATURES_CHUNK_DEPENDENCY_RADIUS; neighborChunkX++) {
        await yieldStep?.();
        const dependencyRadius = Math.max(Math.abs(neighborChunkX - chunkX), Math.abs(neighborChunkZ - chunkZ));
        const requiredStatus = getGeneratedChunkDependencyStatus(GeneratedChunkStatus.FEATURES, dependencyRadius);
        if (!await this.ensureChunkStatusRecursive(neighborChunkX, neighborChunkZ, requiredStatus, yieldStep, chunkViewJobRevision)) {
          return false;
        }
      }
    }

    if (this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)) {
      return true;
    }

    if (yieldStep === undefined) {
      return this.runFeaturesStatus(chunkX, chunkZ);
    }

    return this.runFeaturesStatusCooperative(chunkX, chunkZ, yieldStep);
  }

  private async generateFullStatusRecursive(
    chunkX: number,
    chunkZ: number,
    yieldStep?: () => Promise<void>,
    chunkViewJobRevision = this.chunkViewJobRevision,
  ): Promise<boolean> {
    for (let neighborChunkZ = chunkZ - 1; neighborChunkZ <= chunkZ + 1; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - 1; neighborChunkX <= chunkX + 1; neighborChunkX++) {
        await yieldStep?.();
        if (!await this.ensureChunkStatusRecursive(neighborChunkX, neighborChunkZ, GeneratedChunkStatus.FEATURES, yieldStep, chunkViewJobRevision)) {
          return false;
        }
      }
    }

    const chunk = this.level.getChunk(chunkX, chunkZ, false);
    if (chunk === null) {
      return false;
    }

    if (yieldStep === undefined) {
      return this.markChunkFullWithoutLighting(chunk);
    }

    await yieldStep();
    return this.markChunkFullWithoutLighting(chunk);
  }

  private async preloadStoredChunk(
    chunkX: number,
    chunkZ: number,
    chunkViewJobRevision: number,
  ): Promise<StoredChunkPreloadResult> {
    if (this.level.getChunk(chunkX, chunkZ, false) !== null) {
      this.incrementWorldgenCount("storage_preload_already_loaded");
      return "already_loaded";
    }

    if (!this.shouldAcceptStoredChunkPreload(chunkX, chunkZ, chunkViewJobRevision)) {
      return "stale";
    }

    if (this.storageSession === undefined) {
      this.incrementWorldgenCount("storage_preload_missing");
      return "missing";
    }

    const pendingGeneratedRecord = this.loadPendingGeneratedChunkRecord(chunkX, chunkZ);
    const generatedRecord = pendingGeneratedRecord ?? await this.recordWorldgenPhaseAsync("storage.preload_generated_chunk", () =>
      this.storageSession!.chunks.loadGeneratedChunk(chunkX, chunkZ)
    );
    if (!this.shouldAcceptStoredChunkPreload(chunkX, chunkZ, chunkViewJobRevision)) {
      return "stale";
    }

    if (generatedRecord !== undefined) {
      if (this.restoreStoredGeneratedChunk(generatedRecord)) {
        if (pendingGeneratedRecord !== undefined) {
          this.incrementWorldgenCount("storage_preload_loaded_pending_generated_chunk");
        }
        this.incrementWorldgenCount("storage_preload_loaded_generated_chunk");
        this.incrementWorldgenCount("storage_preload_loaded");
        return "loaded";
      }

      this.incrementWorldgenCount("storage_preload_generated_chunk_rejected");
    }

    const pendingSnapshot = this.loadPendingChunkSnapshot(chunkX, chunkZ);
    const snapshot = pendingSnapshot ?? await this.recordWorldgenPhaseAsync("storage.preload", () => this.storageSession!.chunks.loadChunk(chunkX, chunkZ));
    if (!this.shouldAcceptStoredChunkPreload(chunkX, chunkZ, chunkViewJobRevision)) {
      return "stale";
    }

    if (snapshot === undefined) {
      this.incrementWorldgenCount("storage_preload_missing");
      return "missing";
    }

    this.level.setChunk(
      hydrateChunkFromSnapshot(
        unpackChunkSnapshot(snapshot, this.options.blockStateIds),
        this.options.airState,
        this.resolveBlockState,
      ),
      true,
    );
    this.rememberStoredGeneratedChunkAccess({
      chunkX,
      chunkZ,
      type: "level",
      status: GeneratedChunkStatus.FULL,
      hasBlockSections: true,
      isUnsaved: false,
      contentVersion: GENERATED_PROTO_CHUNK_CONTENT_VERSION,
      writeVersion: 0,
      snapshot,
    });
    if (pendingSnapshot !== undefined) {
      this.incrementWorldgenCount("storage_preload_loaded_pending_chunk");
    }
    this.incrementWorldgenCount("storage_preload_loaded");
    return "loaded";
  }

  private shouldAcceptStoredChunkPreload(chunkX: number, chunkZ: number, chunkViewJobRevision: number): boolean {
    let accepted = true;
    if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
      this.incrementWorldgenCount("storage_preload_skipped_revision_changed");
      accepted = false;
    }
    if (!this.isChunkInCurrentAuthorityView(chunkX, chunkZ)) {
      this.incrementWorldgenCount("storage_preload_skipped_outside_authority");
      accepted = false;
    }
    if (!accepted) {
      this.incrementWorldgenCount("storage_preload_skipped_stale");
    }

    return accepted;
  }

  private loadPendingGeneratedChunkRecord(chunkX: number, chunkZ: number): GeneratedChunkStorageRecord | undefined {
    const pending = this.currentPendingStorageWrite(chunkX, chunkZ);
    if (pending?.generatedRecord === undefined) {
      return undefined;
    }

    this.incrementWorldgenCount("storage_pending_writes_read_generated_chunk");
    return cloneGeneratedChunkStorageRecord(pending.generatedRecord);
  }

  private loadPendingChunkSnapshot(chunkX: number, chunkZ: number): PackedChunkSnapshot | undefined {
    const pending = this.currentPendingStorageWrite(chunkX, chunkZ);
    if (pending?.snapshot === undefined) {
      return undefined;
    }

    this.incrementWorldgenCount("storage_pending_writes_read_chunk");
    return clonePackedChunkSnapshot(pending.snapshot);
  }

  private currentPendingStorageWrite(chunkX: number, chunkZ: number): PendingStorageWrite | undefined {
    const key = chunkKey(chunkX, chunkZ);
    const pending = this.pendingStorageWrites.get(key);
    if (pending === undefined) {
      return undefined;
    }
    if (pending.isCurrent()) {
      return pending;
    }

    this.pendingStorageWrites.delete(key);
    this.incrementWorldgenCount("storage_pending_writes_dropped_stale");
    this.notePendingStorageWriteCount();
    return undefined;
  }

  private async preloadStoredChunkCoalesced(
    chunkX: number,
    chunkZ: number,
    chunkViewJobRevision: number,
  ): Promise<StoredChunkPreloadResult> {
    if (this.level.getChunk(chunkX, chunkZ, false) !== null) {
      this.incrementWorldgenCount("storage_preload_already_loaded");
      return "already_loaded";
    }

    const holder = this.getChunkHolder(chunkX, chunkZ);
    const existing = holder.preloadJob;
    if (existing !== undefined) {
      if (existing.state === "pending" && existing.chunkViewJobRevision === chunkViewJobRevision) {
        this.incrementWorldgenCount("storage_preload_coalesced_pending");
        return existing.promise;
      }

      if (existing.state === "pending") {
        this.incrementWorldgenCount("storage_preload_not_coalesced_revision_changed");
      }

      if (existing.state === "fulfilled" && existing.result !== undefined && existing.result !== "stale") {
        this.incrementWorldgenCount(`storage_preload_reused_${existing.result}`);
        return existing.result;
      }

      holder.preloadJob = undefined;
    }

    let resolveJob!: (result: StoredChunkPreloadResult) => void;
    let rejectJob!: (error: unknown) => void;
    const promise = new Promise<StoredChunkPreloadResult>((resolve, reject) => {
      resolveJob = resolve;
      rejectJob = reject;
    });
    const job: GeneratedChunkPreloadJob = {
      chunkViewJobRevision,
      promise,
      state: "pending",
      result: undefined,
    };
    holder.preloadJob = job;

    void (async () => {
      try {
        const result = await this.preloadStoredChunk(chunkX, chunkZ, chunkViewJobRevision);
        job.state = "fulfilled";
        job.result = result;
        resolveJob(result);
      } catch (error) {
        job.state = "rejected";
        job.result = undefined;
        if (holder.preloadJob === job) {
          holder.preloadJob = undefined;
        }
        rejectJob(error);
      }
    })();

    return promise;
  }

  private restoreStoredGeneratedChunk(record: GeneratedChunkStorageRecord): boolean {
    if (!this.restoreStoredGeneratedChunkIntoLevel(record)) {
      return false;
    }

    this.rememberStoredGeneratedChunkAccess(record);
    return true;
  }

  private restoreStoredGeneratedChunkIntoLevel(record: GeneratedChunkStorageRecord): boolean {
    if (record.snapshot !== undefined && (record.chunkX !== record.snapshot.chunkX || record.chunkZ !== record.snapshot.chunkZ)) {
      return false;
    }
    if (record.contentVersion !== GENERATED_PROTO_CHUNK_CONTENT_VERSION) {
      return false;
    }
    if (record.hasBlockSections && record.snapshot === undefined) {
      return false;
    }
    if (!record.hasBlockSections && record.snapshot !== undefined) {
      return false;
    }

    if (record.snapshot !== undefined) {
      this.level.setChunk(
        hydrateChunkFromSnapshot(
          unpackChunkSnapshot(record.snapshot, this.options.blockStateIds),
          this.options.airState,
          this.resolveBlockState,
        ),
        record.status,
      );
    } else if (!this.level.restoreGeneratedChunkStatusRecord(record.chunkX, record.chunkZ, record.status, false)) {
      return false;
    }

    return true;
  }

  private rememberStoredGeneratedChunkAccess(record: GeneratedChunkStorageRecord): void {
    this.noteGeneratedChunkRecordWriteVersion(chunkKey(record.chunkX, record.chunkZ), record.writeVersion);
    this.getChunkHolder(record.chunkX, record.chunkZ).chunkToSave = createGeneratedChunkAccessFromStorageRecord({
      ...record,
      isUnsaved: false,
    });
  }

  private queueSaveResidentGeneratedChunkAccesses(): void {
    for (const holder of this.chunkHolders.values()) {
      this.queueSaveGeneratedChunkAccess(holder.chunkToSave, this.level.getChunk(holder.chunkX, holder.chunkZ, false), holder);
    }
  }

  private queueSaveGeneratedChunkAccess(
    access: GeneratedChunkAccess | undefined,
    sourceChunk?: GeneratedLevelChunk | null,
    holder?: GeneratedChunkHolder,
  ): QueuedGeneratedChunkAccessSave | undefined {
    if (access === undefined || !access.isUnsaved) {
      return undefined;
    }

    const key = chunkKey(access.chunkX, access.chunkZ);
    const writeVersion = this.bumpGeneratedChunkRecordWriteVersion(key);
    const record = this.createGeneratedChunkStorageRecordForAccess(access, sourceChunk ?? undefined, writeVersion);
    if (record === undefined) {
      return undefined;
    }

    const pending = this.registerPendingGeneratedChunkRecordWrite(record);
    const promise = this.queueStorageSideEffect(async () => {
      try {
        await this.saveGeneratedChunkRecord(record);
        if (
          holder?.chunkToSave === access
          && access.type === record.type
          && access.status === record.status
          && access.hasBlockSections === record.hasBlockSections
          && access.contentVersion === record.contentVersion
        ) {
          access.isUnsaved = false;
        }
      } finally {
        pending.clear();
      }
    }, key);
    return promise === undefined ? undefined : { record, promise };
  }

  private createGeneratedChunkStorageRecordForAccess(
    access: GeneratedChunkAccess,
    sourceChunk?: GeneratedLevelChunk,
    writeVersion = 0,
  ): GeneratedChunkStorageRecord | undefined {
    let snapshot: PackedChunkSnapshot | undefined;
    if (access.hasBlockSections) {
      const chunk = sourceChunk ?? this.level.getChunk(access.chunkX, access.chunkZ, false);
      if (chunk === null || chunk === undefined) {
        this.incrementWorldgenCount("storage_generated_chunk_saves_skipped_missing_sections");
        return undefined;
      }

      snapshot = withoutPersistedLight(this.buildPackedChunkSnapshot(chunk));
    }

    return createGeneratedChunkStorageRecord(access, snapshot, writeVersion);
  }

  private createFullGeneratedChunkStorageRecord(
    snapshot: PackedChunkSnapshot,
    writeVersion: number,
  ): GeneratedChunkStorageRecord {
    return createGeneratedChunkStorageRecord(
      {
        type: "level",
        chunkX: snapshot.chunkX,
        chunkZ: snapshot.chunkZ,
        status: GeneratedChunkStatus.FULL,
        hasBlockSections: true,
        isUnsaved: false,
        contentVersion: GENERATED_PROTO_CHUNK_CONTENT_VERSION,
      },
      snapshot,
      writeVersion,
    );
  }

  private registerPendingGeneratedChunkRecordWrite(
    record: GeneratedChunkStorageRecord,
    isCurrent: () => boolean = () => true,
  ): RegisteredPendingStorageWrite {
    return this.registerPendingStorageWrite({
      key: chunkKey(record.chunkX, record.chunkZ),
      snapshot: undefined,
      generatedRecord: cloneGeneratedChunkStorageRecord(record),
      isCurrent,
    });
  }

  private registerPendingChunkSnapshotWrite(
    snapshot: PackedChunkSnapshot,
    generatedRecordWriteVersion: number,
    isCurrent: () => boolean = () => true,
  ): RegisteredPendingStorageWrite {
    const snapshotWithoutLight = clonePackedChunkSnapshot(withoutPersistedLight(snapshot));
    return this.registerPendingStorageWrite({
      key: chunkKey(snapshot.chunkX, snapshot.chunkZ),
      snapshot: snapshotWithoutLight,
      generatedRecord: this.createFullGeneratedChunkStorageRecord(snapshotWithoutLight, generatedRecordWriteVersion),
      isCurrent,
    });
  }

  private registerPendingStorageWrite(write: PendingStorageWrite): RegisteredPendingStorageWrite {
    if (this.storageSession === undefined) {
      return {
        snapshot: write.snapshot,
        generatedRecord: write.generatedRecord,
        clear: () => {},
      };
    }

    this.pendingStorageWrites.set(write.key, write);
    this.incrementWorldgenCount("storage_pending_writes_registered");
    this.notePendingStorageWriteCount();
    return {
      snapshot: write.snapshot,
      generatedRecord: write.generatedRecord,
      clear: () => {
        if (this.pendingStorageWrites.get(write.key) !== write) {
          return;
        }

        this.pendingStorageWrites.delete(write.key);
        this.incrementWorldgenCount("storage_pending_writes_cleared");
        this.notePendingStorageWriteCount();
      },
    };
  }

  private async saveGeneratedChunkRecord(record: GeneratedChunkStorageRecord): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    await this.recordWorldgenPhaseAsync("storage.save_generated_chunk", () =>
      this.storageSession!.chunks.saveGeneratedChunk(record)
    );
    this.incrementWorldgenCount("storage_generated_chunk_saves");
  }

  private async writeChunkSnapshotByPolicy(snapshot: PackedChunkSnapshot): Promise<void> {
    const key = chunkKey(snapshot.chunkX, snapshot.chunkZ);
    if (this.dirtyDurableChunks.has(key)) {
      await this.saveDirtyChunkSnapshot(key, snapshot);
      return;
    }

    const writeVersion = this.bumpGeneratedChunkRecordWriteVersion(key);
    const pending = this.registerPendingChunkSnapshotWrite(snapshot, writeVersion);
    try {
      await this.cacheGeneratedChunkSnapshot(pending.snapshot ?? snapshot, writeVersion);
    } finally {
      pending.clear();
    }
  }

  private async cacheGeneratedChunkSnapshot(snapshot: PackedChunkSnapshot, generatedRecordWriteVersion: number): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    await this.recordWorldgenPhaseAsync("storage.cache_generated_chunk", () =>
      this.storageSession!.chunks.saveChunk(withoutPersistedLight(snapshot), { generatedRecordWriteVersion })
    );
    this.incrementWorldgenCount("storage_cache_saves");
  }

  private queueGeneratedCacheChunkSnapshot(snapshot: PackedChunkSnapshot): void {
    const key = chunkKey(snapshot.chunkX, snapshot.chunkZ);
    const version = this.bumpGeneratedCacheWriteVersion(key);
    const generatedRecordWriteVersion = this.bumpGeneratedChunkRecordWriteVersion(key);
    const pending = this.registerPendingChunkSnapshotWrite(
      snapshot,
      generatedRecordWriteVersion,
      () => this.generatedCacheWriteVersions.get(key) === version && !this.dirtyDurableChunks.has(key),
    );
    this.queueStorageSideEffect(async () => {
      try {
        if (this.generatedCacheWriteVersions.get(key) !== version || this.dirtyDurableChunks.has(key)) {
          this.incrementWorldgenCount("storage_cache_saves_skipped_stale");
          return;
        }

        await this.cacheGeneratedChunkSnapshot(pending.snapshot ?? snapshot, generatedRecordWriteVersion);
      } finally {
        pending.clear();
      }
    }, key);
  }

  private bumpGeneratedCacheWriteVersion(key: string): number {
    const version = (this.generatedCacheWriteVersions.get(key) ?? 0) + 1;
    this.generatedCacheWriteVersions.set(key, version);
    return version;
  }

  private bumpGeneratedChunkRecordWriteVersion(key: string): number {
    const version = (this.generatedChunkRecordWriteVersions.get(key) ?? 0) + 1;
    this.generatedChunkRecordWriteVersions.set(key, version);
    return version;
  }

  private noteGeneratedChunkRecordWriteVersion(key: string, writeVersion: number): void {
    if ((this.generatedChunkRecordWriteVersions.get(key) ?? 0) < writeVersion) {
      this.generatedChunkRecordWriteVersions.set(key, writeVersion);
    }
  }

  private async saveDirtyChunkSnapshot(key: string, snapshot: PackedChunkSnapshot): Promise<void> {
    this.bumpGeneratedCacheWriteVersion(key);
    const generatedRecordWriteVersion = this.bumpGeneratedChunkRecordWriteVersion(key);
    if (this.storageSession === undefined) {
      this.dirtyDurableChunks.delete(key);
      return;
    }

    const pending = this.registerPendingChunkSnapshotWrite(snapshot, generatedRecordWriteVersion);
    try {
      await this.recordWorldgenPhaseAsync("storage.save_dirty_chunk", () =>
        this.storageSession!.chunks.saveChunk(pending.snapshot ?? withoutPersistedLight(snapshot), { generatedRecordWriteVersion })
      );
      this.incrementWorldgenCount("storage_dirty_saves");
      this.dirtyDurableChunks.delete(key);
    } finally {
      pending.clear();
    }
  }

  private async saveDirtyChunkBeforeUnload(chunk: GeneratedLevelChunk): Promise<void> {
    const key = chunkKey(chunk.chunkX, chunk.chunkZ);
    if (!this.dirtyDurableChunks.has(key)) {
      return;
    }

    await this.ensureLightingForLoadedChunks([chunk], this.chunkViewJobRevision);
    await this.saveDirtyChunkSnapshot(key, this.buildPackedChunkSnapshot(chunk));
  }

  private async queueEvictAfterDirtySave(chunk: GeneratedLevelChunk): Promise<void> {
    const key = chunkKey(chunk.chunkX, chunk.chunkZ);
    try {
      await this.saveDirtyChunkBeforeUnload(chunk);
    } finally {
      this.dirtyDurableChunks.delete(key);
    }

    if (this.storageSession !== undefined) {
      await this.evictStoredChunk(chunk.chunkX, chunk.chunkZ);
    }
  }

  private async evictStoredChunk(chunkX: number, chunkZ: number): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    await this.recordWorldgenPhaseAsync("storage.evict_chunk", () => this.storageSession!.chunks.evictChunk(chunkX, chunkZ));
    this.incrementWorldgenCount("storage_evicts");
  }

  private async runChunkViewJobs(
    chunkViewJobRevision: number,
  ): Promise<void> {
    this.activeChunkViewJobRevision = chunkViewJobRevision;
    this.incrementWorldgenCount("chunk_view_jobs_started");
    try {
      await this.runChunkViewJobsInternal(chunkViewJobRevision);
      if (this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        this.incrementWorldgenCount("chunk_view_jobs_completed");
      } else {
        this.incrementWorldgenCount("chunk_view_jobs_cancelled");
      }
    } finally {
      if (this.activeChunkViewJobRevision === chunkViewJobRevision) {
        this.activeChunkViewJobRevision = undefined;
      }
    }
  }

  private async runChunkViewJobsInternal(
    chunkViewJobRevision: number,
  ): Promise<void> {
    const yieldStep = createCooperativeYield();
    const statusTargets = this.collectCurrentStatusTargets();
    if (statusTargets.length === 0) {
      return;
    }

    let statusDone = 0;
    this.enqueueWorldProgress("Advancing statuses", statusDone, statusTargets.length, chunkViewJobRevision);
    for (const target of statusTargets) {
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      await this.ensureChunkStatusRecursive(target.chunkX, target.chunkZ, target.status, yieldStep, chunkViewJobRevision);
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      statusDone++;
      this.enqueueWorldProgress("Advancing statuses", statusDone, statusTargets.length, chunkViewJobRevision);
    }

    const lightingChunks = this.collectFullStatusChunksForCurrentView();
    let lightingDone = 0;
    let publishDone = 0;
    let publishProgressStarted = false;
    this.enqueueWorldProgress("Computing light", lightingDone, lightingChunks.length, chunkViewJobRevision);
    const publishTotal = this.countUnpublishedChunksInCurrentPublishView();
    const enqueueLightingProgress = (): void => {
      if (!publishProgressStarted) {
        this.enqueueWorldProgress("Computing light", lightingDone, lightingChunks.length, chunkViewJobRevision);
      }
    };
    const enqueuePublishingProgress = (current: number, force = false): void => {
      if (!force && current <= 0 && publishTotal > 0) {
        return;
      }

      publishProgressStarted = true;
      this.enqueueWorldProgress("Publishing chunks", current, publishTotal, chunkViewJobRevision);
    };
    let pendingLightReadyForPublish = false;
    const publishReadyLightingBatch = async (): Promise<void> => {
      if (!pendingLightReadyForPublish) {
        return;
      }

      pendingLightReadyForPublish = false;
      publishDone += await this.publishReadyChunksForCurrentView(chunkViewJobRevision, yieldStep, {
        onChunkPublished: (publishedInPass) => {
          enqueuePublishingProgress(publishDone + publishedInPass);
        },
      });
      enqueuePublishingProgress(publishDone);
    };
    if (this.lightingService !== undefined && lightingChunks.length > 0) {
      this.options.mutateWorld?.(this.liquidLevel);
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      await this.ensureLightingForLoadedChunksCooperative(lightingChunks, chunkViewJobRevision, yieldStep, {
        isCancelled: () => !this.isCurrentChunkViewJob(chunkViewJobRevision),
        onInitialLightReady: async () => {
          lightingDone++;
          pendingLightReadyForPublish = true;
          enqueueLightingProgress();
        },
        onInitialLightBatchReady: publishReadyLightingBatch,
        onLightProgress: (progress) => {
          if (publishProgressStarted) {
            return;
          }

          this.enqueueWorldProgress(
            "Computing light",
            lightingDone,
            lightingChunks.length,
            chunkViewJobRevision,
            `lit ${lightingDone.toString()} / ${lightingChunks.length.toString()}, ${progress.stage} ${progress.current.toString()} / ${progress.total.toString()}`,
          );
        },
      });
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }
    } else {
      this.enqueueWorldProgress("Computing light", lightingChunks.length, lightingChunks.length, chunkViewJobRevision);
      for (const chunk of lightingChunks) {
        await yieldStep();
        if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
          return;
        }

        await this.ensureFullStatusWithoutLightingCooperative(chunk, yieldStep, chunkViewJobRevision);
      }
    }
    await publishReadyLightingBatch();
    publishDone += await this.publishReadyChunksForCurrentView(chunkViewJobRevision, yieldStep, {
      onChunkPublished: (publishedInPass) => {
        enqueuePublishingProgress(publishDone + publishedInPass);
      },
    });
    enqueuePublishingProgress(publishDone);
    lightingDone = lightingChunks.length;
    enqueueLightingProgress();
    enqueuePublishingProgress(publishTotal, true);
    this.enqueueDirtyLightDeltas();
  }

  private isCurrentChunkViewJob(chunkViewJobRevision: number): boolean {
    return chunkViewJobRevision === this.chunkViewJobRevision;
  }

  private enqueueWorldProgress(
    stage: string,
    current: number,
    total: number,
    chunkViewJobRevision: number,
    detail?: string,
  ): void {
    if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
      return;
    }

    const progress: WorldProgressMessage = detail === undefined
      ? {
        type: "world_progress",
        stage,
        current,
        total,
      }
      : {
        type: "world_progress",
        stage,
        detail,
        current,
        total,
      };
    this.pendingMessages = this.pendingMessages.filter((message) => message.type !== "world_progress");
    this.pendingMessages.push(progress);
    this.notePendingMessageBacklog();
  }

  private isChunkInCurrentPublishView(chunkX: number, chunkZ: number): boolean {
    if (this.currentChunkView === undefined) {
      return false;
    }

    const viewRadius = getGeneratedWorldViewChunkRadius(this.currentChunkView.radius);
    return Math.abs(chunkX - this.currentChunkView.centerChunkX) <= viewRadius
      && Math.abs(chunkZ - this.currentChunkView.centerChunkZ) <= viewRadius;
  }

  private isChunkInCurrentFullView(chunkX: number, chunkZ: number): boolean {
    if (this.currentChunkView === undefined) {
      return false;
    }

    const viewRadius = getGeneratedWorldFullChunkRadius(this.currentChunkView.radius);
    return Math.abs(chunkX - this.currentChunkView.centerChunkX) <= viewRadius
      && Math.abs(chunkZ - this.currentChunkView.centerChunkZ) <= viewRadius;
  }

  private isChunkInCurrentAuthorityView(chunkX: number, chunkZ: number): boolean {
    return this.chunkResidencyTickets.contains(chunkX, chunkZ);
  }

  private collectFullStatusChunksForCurrentView(): readonly GeneratedLevelChunk[] {
    if (this.currentChunkView === undefined) {
      return [];
    }

    const chunks: GeneratedLevelChunk[] = [];
    const viewRadius = getGeneratedWorldFullChunkRadius(this.currentChunkView.radius);
    for (let chunkZ = this.currentChunkView.centerChunkZ - viewRadius; chunkZ <= this.currentChunkView.centerChunkZ + viewRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - viewRadius; chunkX <= this.currentChunkView.centerChunkX + viewRadius; chunkX++) {
        if (!this.isChunkInCurrentFullView(chunkX, chunkZ)) {
          continue;
        }

        const chunk = this.level.getChunk(chunkX, chunkZ, false);
        if (chunk === null || !this.level.isChunkFeaturesStable(chunkX, chunkZ)) {
          continue;
        }

        if (this.level.isChunkFull(chunkX, chunkZ) && (this.lightingService === undefined || this.getAcceptedCurrentChunkLight(chunkX, chunkZ) !== undefined)) {
          continue;
        }

        chunks.push(chunk);
      }
    }

    return chunks;
  }

  private countUnpublishedChunksInCurrentPublishView(): number {
    if (this.currentChunkView === undefined) {
      return 0;
    }

    const viewRadius = getGeneratedWorldViewChunkRadius(this.currentChunkView.radius);
    let count = 0;
    for (let chunkZ = this.currentChunkView.centerChunkZ - viewRadius; chunkZ <= this.currentChunkView.centerChunkZ + viewRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - viewRadius; chunkX <= this.currentChunkView.centerChunkX + viewRadius; chunkX++) {
        if (!this.publishedChunkSnapshots.has(chunkKey(chunkX, chunkZ))) {
          count++;
        }
      }
    }

    return count;
  }

  private async ensureFullStatusesForCurrentView(chunkViewRevision: number): Promise<void> {
    const chunks = this.collectFullStatusChunksForCurrentView();
    if (this.lightingService === undefined) {
      for (const chunk of chunks) {
        if (!this.isCurrentChunkViewJob(chunkViewRevision)) {
          return;
        }
        await this.ensureFullStatusWithoutLighting(chunk, chunkViewRevision);
      }
      return;
    }

    await this.ensureLightingForLoadedChunks(chunks, chunkViewRevision);
  }

  private markChunkFullWithoutLighting(chunk: GeneratedLevelChunk): boolean {
    if (!this.level.isChunkFeaturesStable(chunk.chunkX, chunk.chunkZ)) {
      return false;
    }

    if (this.level.isChunkFull(chunk.chunkX, chunk.chunkZ)) {
      return true;
    }

    this.level.markChunkLighted(chunk.chunkX, chunk.chunkZ);
    this.level.markChunkFull(chunk.chunkX, chunk.chunkZ);
    this.recordGeneratedChunkAccessStatus(chunk.chunkX, chunk.chunkZ);
    this.incrementWorldgenCount("chunks_marked_full_without_lighting");
    return this.level.isChunkFull(chunk.chunkX, chunk.chunkZ);
  }

  private markChunkFullAfterAcceptedLight(chunkX: number, chunkZ: number): void {
    if (!this.level.isChunkFeaturesStable(chunkX, chunkZ)) {
      return;
    }

    this.level.markChunkLighted(chunkX, chunkZ);
    this.level.markChunkFull(chunkX, chunkZ);
    this.recordGeneratedChunkAccessStatus(chunkX, chunkZ);
    this.incrementWorldgenCount("chunks_marked_full_after_light");
  }

  private async publishReadyChunksForCurrentView(
    chunkViewJobRevision: number,
    yieldStep?: () => Promise<void>,
    options: PublishReadyChunksOptions = {},
  ): Promise<number> {
    if (this.currentChunkView === undefined) {
      return 0;
    }

    let published = 0;
    const viewRadius = getGeneratedWorldViewChunkRadius(this.currentChunkView.radius);
    for (let chunkZ = this.currentChunkView.centerChunkZ - viewRadius; chunkZ <= this.currentChunkView.centerChunkZ + viewRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - viewRadius; chunkX <= this.currentChunkView.centerChunkX + viewRadius; chunkX++) {
        await yieldStep?.();
        const chunk = this.level.getChunk(chunkX, chunkZ, false);
        if (chunk === null) {
          continue;
        }

        if (await this.publishChunkSnapshotIfReady(
          chunk,
          chunkViewJobRevision,
          undefined,
          options.targetMessages,
          options.awaitStorage ?? false,
        )) {
          published++;
          options.onChunkPublished?.(published);
        }
      }
    }

    return published;
  }

  private isLiquidPositionTicking(pos: BlockPos): boolean {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    if (!this.isChunkTicking(chunkX, chunkZ)) {
      return false;
    }

    // Host scheduling: defer fluid ticks whose water read footprint crosses unloaded chunks instead of synchronously loading them.
    const minChunkX = SectionPos.blockToSectionCoord(pos.getX() - LIQUID_TICK_READ_RADIUS_BLOCKS);
    const maxChunkX = SectionPos.blockToSectionCoord(pos.getX() + LIQUID_TICK_READ_RADIUS_BLOCKS);
    const minChunkZ = SectionPos.blockToSectionCoord(pos.getZ() - LIQUID_TICK_READ_RADIUS_BLOCKS);
    const maxChunkZ = SectionPos.blockToSectionCoord(pos.getZ() + LIQUID_TICK_READ_RADIUS_BLOCKS);
    for (let neighborChunkZ = minChunkZ; neighborChunkZ <= maxChunkZ; neighborChunkZ++) {
      for (let neighborChunkX = minChunkX; neighborChunkX <= maxChunkX; neighborChunkX++) {
        if (!this.isChunkTicking(neighborChunkX, neighborChunkZ)) {
          return false;
        }
      }
    }

    return true;
  }

  private isChunkTicking(chunkX: number, chunkZ: number): boolean {
    const key = chunkKey(chunkX, chunkZ);
    return this.publishedChunkSnapshots.has(key)
      && this.isChunkInCurrentPublishView(chunkX, chunkZ)
      && this.level.getChunk(chunkX, chunkZ, false) !== null
      && this.level.isChunkFull(chunkX, chunkZ);
  }

  private markChunkSnapshotPublished(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    this.publishedChunkSnapshots.add(key);
    this.dirtyChunksForPublication.delete(key);
  }

  private getChunkRevision(chunkX: number, chunkZ: number): number {
    return this.chunkRevisions.get(chunkKey(chunkX, chunkZ)) ?? 1;
  }

  private incrementChunkRevision(chunkX: number, chunkZ: number): number {
    const key = chunkKey(chunkX, chunkZ);
    const revision = this.getChunkRevision(chunkX, chunkZ) + 1;
    this.chunkRevisions.set(key, revision);
    for (let neighborChunkZ = chunkZ - 1; neighborChunkZ <= chunkZ + 1; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - 1; neighborChunkX <= chunkX + 1; neighborChunkX++) {
        this.acceptedChunkLightRevisions.delete(chunkKey(neighborChunkX, neighborChunkZ));
      }
    }
    return revision;
  }

  private markHostBlockChanged(pos: BlockPos, oldState: BlockState, newState: BlockState): void {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    const key = chunkKey(chunkX, chunkZ);
    this.dirtyDurableChunks.add(key);
    this.dirtyChunksForPublication.add(key);
    this.bumpGeneratedCacheWriteVersion(key);
    this.incrementChunkRevision(chunkX, chunkZ);
    this.recordPendingLightBlockChange(pos, oldState, newState);
  }

  private recordPendingLightBlockChange(pos: BlockPos, oldState: BlockState, newState: BlockState): void {
    if (this.lightingService === undefined || oldState === newState) {
      return;
    }

    const oldBlockStateId = this.options.blockStateIds.idFor(oldState);
    const newBlockStateId = this.options.blockStateIds.idFor(newState);
    if (oldBlockStateId === newBlockStateId) {
      return;
    }

    const posKey = pos.asLong();
    const existing = this.pendingLightBlockChanges.get(posKey);
    const coalescedOldBlockStateId = existing?.oldBlockStateId ?? oldBlockStateId;
    if (coalescedOldBlockStateId === newBlockStateId) {
      this.pendingLightBlockChanges.delete(posKey);
      return;
    }

    this.pendingLightBlockChanges.set(posKey, {
      x: pos.getX(),
      y: pos.getY(),
      z: pos.getZ(),
      chunkX: SectionPos.blockToSectionCoord(pos.getX()),
      chunkZ: SectionPos.blockToSectionCoord(pos.getZ()),
      oldBlockStateId: coalescedOldBlockStateId,
      newBlockStateId,
    });
  }

  private tickLiquid(tick: TickNextTickData<Fluid>): void {
    const fluidState = this.liquidLevel.getFluidState(tick.pos);
    if (fluidState.getType() === tick.getType()) {
      fluidState.tick(this.liquidLevel, tick.pos);
    }
  }

  private async tickWorldLoop(nowMs = this.nowMs()): Promise<void> {
    const tickIntervalMs = Math.max(1, this.options.worldTickIntervalMs ?? PLAYER_TICK_INTERVAL_MS);
    const dueTicks = Math.floor((nowMs - this.lastWorldTickAtMs) / tickIntervalMs);
    if (dueTicks <= 0) {
      return;
    }
    this.processChunkHolderUnloadQueue();
    if (
      this.publishedChunkSnapshots.size === 0
      || this.activeChunkViewJobRevision !== undefined
    ) {
      this.lastWorldTickAtMs += dueTicks * tickIntervalMs;
      return;
    }

    if (this.liquidSimulationEnabled) {
      this.hydratePublishedLiquidTicks();
    }
    for (let tickIndex = 0; tickIndex < dueTicks; tickIndex++) {
      this.gameTime++;
      if (this.liquidSimulationEnabled) {
        this.liquidTicks.tick();
      }
      this.entityRuntime.tick();
      this.pushOverlappingLivingEntities();
    }
    if (this.liquidSimulationEnabled) {
      this.hydratePublishedLiquidTicks();
    }
    this.lastWorldTickAtMs += dueTicks * tickIntervalMs;
    if (this.liquidSimulationEnabled) {
      await this.flushDirtyPublishedChunks(
        this.options.chunkViewScheduling === "cooperative" ? createCooperativeYield() : undefined,
      );
      this.enqueueDirtyLightDeltas();
    }
    this.enqueueDirtyEntityUpdates();
  }

  private hydratePublishedLiquidTicks(): void {
    for (const key of this.publishedChunkSnapshots) {
      const [chunkXRaw, chunkZRaw] = key.split(",");
      const chunk = this.level.getChunk(Number(chunkXRaw), Number(chunkZRaw), false);
      if (chunk === null) {
        continue;
      }

      this.hydrateLiquidTicksFromChunk(chunk);
    }
  }

  private hydrateLiquidTicksFromChunk(chunk: GeneratedLevelChunk): void {
    for (const tick of chunk.consumeScheduledLiquidTicks()) {
      this.liquidTicks.scheduleTick(
        new BlockPos(tick.x, tick.y, tick.z),
        resolveFluidTickTarget(tick.target),
        Math.max(0, tick.delay),
      );
    }
  }

  private withPendingLiquidTicks(
    snapshot: ChunkSnapshot,
    chunk: GeneratedLevelChunk,
  ): ChunkSnapshot {
    if (!this.liquidSimulationEnabled) {
      return snapshot;
    }

    const pendingLiquidTicks = this.collectPendingLiquidTicksForChunk(chunk);
    if (pendingLiquidTicks.length === 0) {
      return snapshot;
    }

    return {
      ...snapshot,
      liquidTicks: [...snapshot.liquidTicks, ...pendingLiquidTicks],
    };
  }

  private collectPendingLiquidTicksForChunk(
    chunk: GeneratedLevelChunk,
  ): ScheduledTickSnapshot[] {
    return this.liquidTicks.getPendingTicks()
      .filter((tick) => (
        SectionPos.blockToSectionCoord(tick.pos.getX()) === chunk.chunkX
        && SectionPos.blockToSectionCoord(tick.pos.getZ()) === chunk.chunkZ
      ))
      .map((tick) => createScheduledTickSnapshot(
        tick.pos,
        serializeFluidTickTarget(tick.getType()),
        Math.max(0, tick.triggerTick - this.gameTime),
      ));
  }

  private async flushDirtyPublishedChunks(yieldStep?: () => Promise<void>): Promise<void> {
    const dirtyKeys = [...this.dirtyChunksForPublication].sort();
    const dirtyChunks = dirtyKeys
      .map((key) => {
        const [chunkXRaw, chunkZRaw] = key.split(",");
        return this.level.getChunk(Number(chunkXRaw), Number(chunkZRaw), false);
      })
      .filter((chunk): chunk is GeneratedLevelChunk => chunk !== null);
    if (yieldStep === undefined) {
      await this.ensureLightingForLoadedChunks(dirtyChunks, this.chunkViewJobRevision);
    } else {
      await this.ensureLightingForLoadedChunksCooperative(dirtyChunks, this.chunkViewJobRevision, yieldStep);
    }

    for (const key of dirtyKeys) {
      await yieldStep?.();
      const [chunkXRaw, chunkZRaw] = key.split(",");
      const chunkX = Number(chunkXRaw);
      const chunkZ = Number(chunkZRaw);
      const chunk = this.level.getChunk(chunkX, chunkZ, false);
      if (chunk === null) {
        this.dirtyChunksForPublication.delete(key);
        continue;
      }

      const snapshot = this.recordWorldgenPhase("snapshot.build_pack_dirty", () => this.buildPackedChunkSnapshot(chunk));
      await this.writeChunkSnapshotByPolicy(snapshot);
      if (!this.isChunkInCurrentPublishView(chunkX, chunkZ) || !this.publishedChunkSnapshots.has(key)) {
        this.dirtyChunksForPublication.delete(key);
        continue;
      }

      this.pendingMessages.push({
        type: "chunk_snapshot",
        snapshot,
      });
      this.incrementWorldgenCount("dirty_chunk_snapshots_published");
      this.notePendingMessageBacklog();
      this.dirtyChunksForPublication.delete(key);
    }
  }

  private queueStorageSideEffect(
    task: StorageSideEffectTask | undefined,
    key = GLOBAL_STORAGE_SIDE_EFFECT_KEY,
  ): Promise<void> | undefined {
    if (task === undefined) {
      return undefined;
    }

    this.pendingStorageSideEffects++;
    this.incrementWorldgenCount("storage_side_effects_queued");
    this.noteStorageSideEffectQueueDepth();
    let resolveQueued!: () => void;
    const ready = new Promise<void>((resolve) => {
      resolveQueued = resolve;
    });
    const previousKeyTail = this.storageSideEffectKeyTails.get(key) ?? Promise.resolve();
    const queued = previousKeyTail.then(() => {
      this.readyStorageSideEffects.push({
        task,
        key,
        resolve: resolveQueued,
      });
      this.pumpStorageSideEffects();
      return ready;
    });
    this.storageSideEffectKeyTails.set(key, queued);
    this.noteStorageSideEffectKeyCount();
    void queued.finally(() => {
      if (this.storageSideEffectKeyTails.get(key) === queued) {
        this.storageSideEffectKeyTails.delete(key);
        this.noteStorageSideEffectKeyCount();
      }
    });
    return queued;
  }

  private pumpStorageSideEffects(): void {
    while (
      this.activeStorageSideEffects < STORAGE_SIDE_EFFECT_MAX_CONCURRENCY
      && this.readyStorageSideEffects.length > 0
    ) {
      const queued = this.readyStorageSideEffects.shift()!;
      this.activeStorageSideEffects++;
      this.noteActiveStorageSideEffectCount();
      void this.runStorageSideEffect(queued);
    }
  }

  private async runStorageSideEffect(queued: QueuedStorageSideEffect): Promise<void> {
    try {
      await queued.task();
      this.incrementWorldgenCount("storage_side_effects_completed");
    } catch (error) {
      this.incrementWorldgenCount("storage_side_effects_failed");
      this.enqueueWorldError(error);
    } finally {
      this.activeStorageSideEffects--;
      this.pendingStorageSideEffects--;
      queued.resolve();
      this.noteActiveStorageSideEffectCount();
      this.noteStorageSideEffectQueueDepth();
      this.pumpStorageSideEffects();
      this.resolveStorageSideEffectIdleIfNeeded();
    }
  }

  private resolveStorageSideEffectIdleIfNeeded(): void {
    if (this.pendingStorageSideEffects > 0) {
      return;
    }

    while (this.storageSideEffectIdleResolvers.length > 0) {
      this.storageSideEffectIdleResolvers.shift()?.();
    }
  }

  private async publishChunkSnapshotIfReady(
    chunk: GeneratedLevelChunk,
    chunkViewJobRevision: number,
    yieldStep?: () => Promise<void>,
    targetMessages?: WorldHostMessage[],
    awaitStorage = false,
  ): Promise<boolean> {
    const key = chunkKey(chunk.chunkX, chunk.chunkZ);
    if (
      !this.isCurrentChunkViewJob(chunkViewJobRevision)
      || !this.isChunkInCurrentPublishView(chunk.chunkX, chunk.chunkZ)
      || this.publishedChunkSnapshots.has(key)
      || !this.level.isChunkPublishable(chunk.chunkX, chunk.chunkZ)
    ) {
      return false;
    }

    if (this.lightingService !== undefined && this.getAcceptedCurrentChunkLight(chunk.chunkX, chunk.chunkZ) === undefined) {
      return false;
    }

    await yieldStep?.();
    if (
      !this.isCurrentChunkViewJob(chunkViewJobRevision)
      || !this.isChunkInCurrentPublishView(chunk.chunkX, chunk.chunkZ)
      || this.publishedChunkSnapshots.has(key)
      || !this.level.isChunkPublishable(chunk.chunkX, chunk.chunkZ)
    ) {
      return false;
    }

    this.ensureOriginalMobsForChunk(chunk);
    const snapshot = this.recordWorldgenPhase("snapshot.build_pack_for_publish", () => this.buildPackedChunkSnapshot(chunk));
    const messages = targetMessages ?? this.pendingMessages;
    messages.push({
      type: "chunk_snapshot",
      snapshot,
    });
    this.markChunkSnapshotPublished(chunk.chunkX, chunk.chunkZ);
    this.incrementWorldgenCount("chunk_snapshots_published");
    this.notePendingMessageBacklog();
    this.publishEntitySnapshotsForChunk(chunk.chunkX, chunk.chunkZ, messages);
    if (this.dirtyDurableChunks.has(key)) {
      await this.writeChunkSnapshotByPolicy(snapshot);
    } else {
      if (awaitStorage) {
        this.incrementWorldgenCount("storage_cache_saves_deferred_from_sync_publish");
      }
      this.queueGeneratedCacheChunkSnapshot(snapshot);
    }
    return true;
  }

  private buildPackedChunkSnapshot(chunk: GeneratedLevelChunk): PackedChunkSnapshot {
    this.incrementWorldgenCount("chunk_snapshots_built");
    if (this.liquidSimulationEnabled) {
      this.hydrateLiquidTicksFromChunk(chunk);
    }
    const snapshot = this.recordWorldgenPhase("snapshot.build_logical", () => this.withPendingLiquidTicks(
      buildChunkSnapshot(
        chunk,
        new ChunkBiomeContainer(
          this.level.getMinBuildHeight(),
          this.level.getHeight(),
          chunk.chunkX,
          chunk.chunkZ,
          this.biomeSource,
        ).writeBiomes(),
        this.level.getMinBuildHeight(),
        this.level.getHeight(),
      ),
      chunk,
    ));
    const light = this.getAcceptedCurrentChunkLight(chunk.chunkX, chunk.chunkZ);

    return this.recordWorldgenPhase("snapshot.pack", () => packChunkSnapshot(
      light === undefined ? snapshot : { ...snapshot, light },
      this.options.blockStateIds,
      this.resolveBlockState,
    ));
  }

  private ensureOriginalMobsForChunk(chunk: GeneratedLevelChunk): void {
    const key = chunkKey(chunk.chunkX, chunk.chunkZ);
    if (this.spawnedOriginalMobChunks.has(key)) {
      return;
    }

    // Runtime: generated-world host owns the entity sink until status futures can invoke ChunkStatus.SPAWN directly.
    this.generator.spawnOriginalMobs(
      this.level,
      chunk.chunkX,
      chunk.chunkZ,
      {
        addFreshEntityWithPassengers: (entity) => {
          this.entityRuntime.addWorldGenChunkEntities([entity]);
        },
      },
      {
        nextEntityId: () => this.nextGeneratedEntityId++,
        nextEntityUuid: (id) => `mclone:generated/${this.options.seed.toString()}/${chunk.chunkX.toString()}/${chunk.chunkZ.toString()}/${id.toString()}`,
      },
    );
    this.spawnedOriginalMobChunks.add(key);
    this.entityRuntime.processLifecycle();
  }

  private publishEntitySnapshotsForChunk(chunkX: number, chunkZ: number, messages: WorldHostMessage[]): void {
    const snapshots = this.entityRuntime.manager.getEntityGetter().getAll()
      .filter((entity) => SectionPos.blockToSectionCoord(entity.blockPosition().getX()) === chunkX
        && SectionPos.blockToSectionCoord(entity.blockPosition().getZ()) === chunkZ)
      .sort((left, right) => left.id - right.id)
      .map((entity) => this.createEntitySnapshotMessage(entity));

    messages.push(...snapshots);
    for (const snapshot of snapshots) {
      this.publishedEntitySnapshots.set(snapshot.entity.id, snapshot.entity);
    }
    if (snapshots.length > 0) {
      this.incrementWorldgenCount("entity_snapshots_published", snapshots.length);
      this.notePendingMessageBacklog();
    }
  }

  private createEntitySnapshotMessage(entity: GeneratedMobEntity): EntitySnapshotMessage {
    const snapshotData = entity.getSnapshotData();
    const data = Object.keys(snapshotData).length === 0 ? undefined : snapshotData;
    const snapshot: EntitySnapshot = {
      id: entity.id,
      uuid: entity.uuid,
      typeId: entity.typeId,
      category: entity.entityType.category as EntitySnapshotCategory,
      chunkX: SectionPos.blockToSectionCoord(entity.blockPosition().getX()),
      chunkZ: SectionPos.blockToSectionCoord(entity.blockPosition().getZ()),
      position: { ...entity.position },
      rotation: { ...entity.rotation },
      width: entity.entityType.width,
      height: entity.entityType.height,
      onGround: entity.onGround,
      age: entity.age,
      tick: entity.tickCount,
      ...(data === undefined ? {} : { data }),
    };

    return {
      type: "entity_snapshot",
      entity: snapshot,
    };
  }

  private createEntityRemoveMessagesForChunk(
    chunkX: number,
    chunkZ: number,
  ): Extract<WorldHostMessage, { type: "entity_remove" }>[] {
    return this.entityRuntime.manager.getEntityGetter().getAll()
      .filter((entity) => SectionPos.blockToSectionCoord(entity.blockPosition().getX()) === chunkX
        && SectionPos.blockToSectionCoord(entity.blockPosition().getZ()) === chunkZ)
      .sort((left, right) => left.id - right.id)
      .map((entity) => ({
        type: "entity_remove",
        entityId: entity.id,
        uuid: entity.uuid,
        reason: "unloaded_to_chunk",
      }));
  }

  private enqueueDirtyEntityUpdates(): void {
    const messages = this.collectEntityPublicationUpdates();
    if (messages.length === 0) {
      return;
    }

    this.pendingMessages.push(...messages);
    this.incrementWorldgenCount("entity_updates_published", messages.length);
    this.notePendingMessageBacklog();
  }

  private collectEntityPublicationUpdates(): WorldHostMessage[] {
    const messages: WorldHostMessage[] = [];
    const currentEntityIds = new Set<number>();
    const entities = [...this.entityRuntime.manager.getEntityGetter().getAll()]
      .sort((left, right) => left.id - right.id);

    for (const entity of entities) {
      currentEntityIds.add(entity.id);
      const snapshot = this.createEntitySnapshotMessage(entity).entity;
      const inPublishedView = this.isEntitySnapshotInPublishedView(snapshot);
      const previous = this.publishedEntitySnapshots.get(entity.id);

      if (!inPublishedView) {
        if (previous !== undefined) {
          messages.push({
            type: "entity_remove",
            entityId: entity.id,
            uuid: entity.uuid,
            reason: "untracked",
          });
          this.publishedEntitySnapshots.delete(entity.id);
        }
        continue;
      }

      if (previous === undefined) {
        messages.push({ type: "entity_snapshot", entity: snapshot });
        this.publishedEntitySnapshots.set(entity.id, snapshot);
        continue;
      }

      const update = createEntityUpdate(previous, snapshot);
      if (update !== undefined) {
        messages.push(update);
        this.publishedEntitySnapshots.set(entity.id, snapshot);
      }
    }

    for (const [entityId, previous] of [...this.publishedEntitySnapshots]) {
      if (currentEntityIds.has(entityId)) {
        continue;
      }

      messages.push({
        type: "entity_remove",
        entityId,
        uuid: previous.uuid,
        reason: "discarded",
      });
      this.publishedEntitySnapshots.delete(entityId);
    }

    return messages;
  }

  private isEntitySnapshotInPublishedView(snapshot: EntitySnapshot): boolean {
    const key = chunkKey(snapshot.chunkX, snapshot.chunkZ);
    return this.publishedChunkSnapshots.has(key) && this.isChunkInCurrentPublishView(snapshot.chunkX, snapshot.chunkZ);
  }

  private tickGeneratedEntity(entity: GeneratedMobEntity): void {
    entity.setAiLevel({
      getMinBuildHeight: () => this.level.getMinBuildHeight(),
      getHeight: () => this.level.getHeight(),
      getMaxBuildHeight: () => this.level.getMaxBuildHeight(),
      getBlockState: (pos) => this.getMobPathfindingBlockState(pos),
      getFluidState: (pos) => this.getMobPathfindingBlockState(pos).getFluidState(),
      getMaxLightLevel: () => 15,
      noCollision: (_entity, collisionBox) => this.noMobPathCollision(collisionBox),
      getNearestPlayer: (x, y, z, range) => this.getNearestLocalPlayerLookTarget(x, y, z, range),
      findStableStandingY: (x, z, nearY) => this.findStableMobStandingY(x, z, nearY),
      isStableDestination: (pos) => this.isStableMobDestination(pos),
      isWater: (pos) => this.isWaterMobPosition(pos),
      isSolid: (pos) => this.isSolidMobPosition(pos),
    });
    entity.tickServerAi({
      resetNoActionTime: this.shouldResetMobNoActionTime(entity),
    });
  }

  private getNearestLocalPlayerLookTarget(x: number, y: number, z: number, range: number): MobLookTarget | undefined {
    if (this.playerState === undefined) {
      return undefined;
    }

    const body = movementBodyFromPlayerState(this.playerState);
    const eyeY = body.position.y + PLAYER_STANDING_EYE_HEIGHT;
    const dx = body.position.x - x;
    const dy = eyeY - y;
    const dz = body.position.z - z;
    if ((dx * dx) + (dy * dy) + (dz * dz) > range * range) {
      return undefined;
    }

    return {
      getX: () => body.position.x,
      getY: () => body.position.y,
      getZ: () => body.position.z,
      getEyeY: () => eyeY,
      isAlive: () => true,
    };
  }

  private pushOverlappingLivingEntities(): void {
    const applications = new Map<string, (delta: Vec3) => void>();
    const participants: LivingEntityPushParticipant[] = [];

    for (const entity of this.entityRuntime.manager.getEntityGetter().getAll()) {
      if (entity.removalReason !== undefined) {
        continue;
      }

      const key = `entity:${entity.id}`;
      applications.set(key, (delta) => this.pushGeneratedEntity(entity, delta));
      participants.push({
        key,
        x: entity.position.x,
        z: entity.position.z,
        boundingBox: entity.getBoundingBox(),
        sourcePushes: this.entityRuntime.tickList.contains(entity),
        pushable: true,
        vehicle: entity.isVehicle(),
        noPhysics: false,
      });
    }

    if (this.playerState !== undefined) {
      const key = `player:${this.playerState.playerId}`;
      const body = movementBodyFromPlayerState(this.playerState);
      applications.set(key, (delta) => this.pushLocalPlayer(delta));
      participants.push({
        key,
        x: body.position.x,
        z: body.position.z,
        boundingBox: body.bounds,
        sourcePushes: true,
        pushable: true,
        vehicle: false,
        noPhysics: false,
      });
    }

    const deltas = collectLivingEntityPushDeltas(participants);
    for (const [key, delta] of deltas) {
      applications.get(key)?.(delta);
    }
  }

  private pushGeneratedEntity(entity: GeneratedMobEntity, delta: Vec3): void {
    if (delta.horizontalDistanceSqr() <= 1.0e-14) {
      return;
    }

    const nextX = entity.position.x + delta.x;
    const nextZ = entity.position.z + delta.z;
    const stableY = this.findStableMobStandingY(nextX, nextZ, entity.position.y);
    if (stableY === undefined) {
      return;
    }

    const nextBox = entity.getBoundingBox().move(delta.x, stableY - entity.position.y, delta.z);
    if (!this.noMobPathCollision(nextBox)) {
      return;
    }

    entity.setPosition(nextX, stableY, nextZ);
    entity.onGround = true;
  }

  private pushLocalPlayer(delta: Vec3): void {
    if (this.playerState === undefined || delta.horizontalDistanceSqr() <= 1.0e-14) {
      return;
    }

    const previous = this.playerState;
    const next = pushPlayerStateWithCollision(
      previous,
      new Vec3(delta.x, 0.0, delta.z),
      previous.tick + 1,
      (bounds) => this.noMobPathCollision(bounds),
    );
    if (next !== previous) {
      this.playerState = next;
      this.enqueuePlayerStateMessage();
    }
  }

  private shouldResetMobNoActionTime(entity: GeneratedMobEntity): boolean {
    const player = this.playerState?.position;
    if (player !== undefined) {
      const dx = player.x - entity.position.x;
      const dy = player.y - entity.position.y;
      const dz = player.z - entity.position.z;
      if ((dx * dx) + (dy * dy) + (dz * dz) < 32 * 32) {
        return true;
      }
    }

    const chunkX = SectionPos.blockToSectionCoord(entity.blockPosition().getX());
    const chunkZ = SectionPos.blockToSectionCoord(entity.blockPosition().getZ());
    return this.entityChunkStatuses.get(chunkKey(chunkX, chunkZ)) === FullChunkStatus.ENTITY_TICKING;
  }

  private isStableMobDestination(pos: BlockPos): boolean {
    const state = this.getLoadedBlockState(pos);
    const above = this.getLoadedBlockState(pos.above());
    const below = this.getLoadedBlockState(pos.below());
    return state !== undefined
      && above !== undefined
      && below !== undefined
      && !state.getMaterial().blocksMotion()
      && !above.getMaterial().blocksMotion()
      && below.getMaterial().isSolid();
  }

  private findStableMobStandingY(x: number, z: number, nearY: number): number | undefined {
    const blockX = floor(x);
    const blockZ = floor(z);
    const centerY = floor(nearY);
    const minY = this.level.getMinBuildHeight() + 1;
    const maxY = this.level.getMaxBuildHeight() - 2;
    const topY = Math.min(maxY, centerY + MOB_STABLE_STANDING_SCAN_UP);
    const bottomY = Math.max(minY, centerY - MOB_STABLE_STANDING_SCAN_DOWN);

    for (let y = topY; y >= bottomY; y--) {
      if (this.isStableMobDestination(new BlockPos(blockX, y, blockZ))) {
        return y;
      }
    }

    return undefined;
  }

  private isWaterMobPosition(pos: BlockPos): boolean {
    return this.getLoadedBlockState(pos)?.getFluidState().getType().isSame(Fluids.WATER) ?? false;
  }

  private isSolidMobPosition(pos: BlockPos): boolean {
    return this.getLoadedBlockState(pos)?.getMaterial().isSolid() ?? false;
  }

  private getMobPathfindingBlockState(pos: BlockPos): BlockState {
    return this.getLoadedBlockState(pos) ?? this.getMobMissingPathBlockState();
  }

  private getMobMissingPathBlockState(): BlockState {
    return this.options.blockStateById.find((state) => !state.isAir() && state.getMaterial().blocksMotion()) ?? this.options.airState;
  }

  private noMobPathCollision(bounds: AABB): boolean {
    const epsilon = AABB.epsilon();
    const minX = Math.floor(bounds.minX - epsilon);
    const minY = Math.floor(bounds.minY - epsilon);
    const minZ = Math.floor(bounds.minZ - epsilon);
    const maxX = Math.floor(bounds.maxX + epsilon);
    const maxY = Math.floor(bounds.maxY + epsilon);
    const maxZ = Math.floor(bounds.maxZ + epsilon);
    const pos = new BlockPos.MutableBlockPos();
    const blockGetter = this.createMobPathBlockGetter();

    for (let y = minY; y <= maxY; y++) {
      for (let z = minZ; z <= maxZ; z++) {
        for (let x = minX; x <= maxX; x++) {
          pos.set(x, y, z);
          const state = this.getLoadedBlockState(pos);
          if (state === undefined) {
            return false;
          }
          if (state.isAir()) {
            continue;
          }

          const shape = state.getCollisionShape(blockGetter, pos);
          if (shape.isEmpty()) {
            continue;
          }

          for (const box of shape.toAabbs()) {
            if (box.move(x, y, z).intersects(bounds)) {
              return false;
            }
          }
        }
      }
    }

    return true;
  }

  private createMobPathBlockGetter() {
    return {
      getBlockState: (pos: BlockPos) => this.getMobPathfindingBlockState(pos),
      getFluidState: (pos: BlockPos) => this.getMobPathfindingBlockState(pos).getFluidState(),
      getMaxLightLevel: () => 15,
    };
  }

  private getLoadedBlockState(pos: BlockPos): BlockState | undefined {
    const chunk = this.level.getAuthorityChunk(
      SectionPos.blockToSectionCoord(pos.getX()),
      SectionPos.blockToSectionCoord(pos.getZ()),
    );
    return chunk?.getBlockState(pos);
  }

  private getAcceptedCurrentChunkLight(chunkX: number, chunkZ: number): PackedChunkLight | undefined {
    const key = chunkKey(chunkX, chunkZ);
    const light = this.acceptedChunkLight.get(key);
    if (light === undefined || this.acceptedChunkLightRevisions.get(key) !== this.getChunkRevision(chunkX, chunkZ)) {
      return undefined;
    }

    return light;
  }

  private async ensureLightingForLoadedChunks(
    chunks: readonly GeneratedLevelChunk[],
    chunkViewRevision: number,
  ): Promise<void> {
    return this.ensureLightingForLoadedChunksCooperative(chunks, chunkViewRevision, yieldToEventLoop);
  }

  private async ensureLightingForLoadedChunksCooperative(
    chunks: readonly GeneratedLevelChunk[],
    chunkViewRevision: number,
    yieldStep: () => Promise<void>,
    options: EnsureLightingOptions = {},
  ): Promise<void> {
    const lightingService = this.lightingService;
    if (lightingService === undefined || chunks.length === 0) {
      return;
    }

    await this.flushPendingLightBlockChanges(yieldStep);
    if (options.isCancelled?.() === true) {
      return;
    }

    const lightEligibleChunks = chunks.filter((chunk) => this.level.isChunkFeaturesStable(chunk.chunkX, chunk.chunkZ));
    if (lightEligibleChunks.length === 0) {
      return;
    }

    const dependencyChunks = this.collectLightingDependencyChunks(lightEligibleChunks);
    for (const chunk of dependencyChunks) {
      await yieldStep();
      if (options.isCancelled?.() === true) {
        return;
      }
      await this.upsertLightInputChunk(chunk, chunkViewRevision);
    }

    const pending = new Map<string, number>();
    let acceptedBeforeRequests = false;
    for (const chunk of lightEligibleChunks) {
      if (options.isCancelled?.() === true) {
        return;
      }

      const key = chunkKey(chunk.chunkX, chunk.chunkZ);
      const chunkRevision = this.getChunkRevision(chunk.chunkX, chunk.chunkZ);
      const accepted = this.getAcceptedCurrentChunkLight(chunk.chunkX, chunk.chunkZ);
      if (accepted !== undefined && accepted.lightCorrect && this.sentLightInputRevisions.get(key) === chunkRevision) {
        this.markChunkFullAfterAcceptedLight(chunk.chunkX, chunk.chunkZ);
        await options.onInitialLightReady?.(chunk);
        acceptedBeforeRequests = true;
        continue;
      }

      const neighbors = this.collectInitialLightNeighborRevisions(chunk);
      await lightingService.requestInitialLight({
        type: "request_initial_light",
        chunkViewRevision,
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
        chunkRevision,
        neighbors,
      });
      pending.set(key, chunkRevision);
    }
    if (acceptedBeforeRequests) {
      await options.onInitialLightBatchReady?.();
    }

    while (pending.size > 0) {
      await yieldStep();
      if (options.isCancelled?.() === true) {
        return;
      }
      const batch = await lightingService.pollResults({
        type: "poll_light_results",
        maxResults: LIGHTING_RESULT_BATCH_SIZE,
      });
      if (batch.results.length === 0) {
        await yieldToEventLoop();
        continue;
      }

      let acceptedInBatch = false;
      for (const result of batch.results) {
        switch (result.type) {
          case "chunk_light_ready": {
            const accepted = this.acceptInitialLightResult(result, pending, chunkViewRevision);
            if (accepted) {
              const chunk = this.level.getChunk(result.chunkX, result.chunkZ, false);
              if (chunk !== null) {
                this.markChunkFullAfterAcceptedLight(chunk.chunkX, chunk.chunkZ);
                await options.onInitialLightReady?.(chunk);
                acceptedInBatch = true;
              }
            }
            break;
          }
          case "chunk_light_delta":
            this.acceptLightDeltaResult(result, chunkViewRevision);
            break;
          case "block_light_update_complete":
          case "light_performance":
            break;
          case "light_progress":
            options.onLightProgress?.(result);
            break;
          case "light_error":
            this.handleLightError(result, chunkViewRevision);
            break;
        }
      }
      if (acceptedInBatch) {
        await options.onInitialLightBatchReady?.();
      }
    }
  }

  private async setLightingView(request: SetChunkViewRequest, chunkViewRevision: number): Promise<void> {
    await this.lightingService?.setView({
      type: "set_light_view",
      chunkViewRevision,
      centerChunkX: request.centerChunkX,
      centerChunkZ: request.centerChunkZ,
      loadRadius: getGeneratedWorldAuthorityChunkRadius(request.radius),
      publishRadius: getGeneratedWorldViewChunkRadius(request.radius),
    });
  }

  private collectLightingDependencyChunks(chunks: readonly GeneratedLevelChunk[]): readonly GeneratedLevelChunk[] {
    const collected = new Map<string, GeneratedLevelChunk>();
    for (const chunk of chunks) {
      if (!this.level.isChunkFeaturesStable(chunk.chunkX, chunk.chunkZ)) {
        throw new Error(
          `Cannot request initial light for (${chunk.chunkX.toString()}, ${chunk.chunkZ.toString()}) before its 3x3 FEATURES neighborhood is complete`,
        );
      }

      for (let neighborChunkZ = chunk.chunkZ - 1; neighborChunkZ <= chunk.chunkZ + 1; neighborChunkZ++) {
        for (let neighborChunkX = chunk.chunkX - 1; neighborChunkX <= chunk.chunkX + 1; neighborChunkX++) {
          const neighbor = this.level.getChunk(neighborChunkX, neighborChunkZ, false);
          if (neighbor === null) {
            throw new Error(
              `Missing lighting neighbor chunk (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) for (${chunk.chunkX.toString()}, ${chunk.chunkZ.toString()})`,
            );
          }
          if (!this.level.hasChunkStatus(neighborChunkX, neighborChunkZ, GeneratedChunkStatus.FEATURES)) {
            throw new Error(
              `Lighting neighbor (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) for (${chunk.chunkX.toString()}, ${chunk.chunkZ.toString()}) is not at FEATURES`,
            );
          }

          collected.set(chunkKey(neighbor.chunkX, neighbor.chunkZ), neighbor);
        }
      }
    }

    return [...collected.values()];
  }

  private collectInitialLightNeighborRevisions(chunk: GeneratedLevelChunk): readonly LightingNeighborRevision[] {
    const neighbors: LightingNeighborRevision[] = [];
    for (let neighborChunkZ = chunk.chunkZ - 1; neighborChunkZ <= chunk.chunkZ + 1; neighborChunkZ++) {
      for (let neighborChunkX = chunk.chunkX - 1; neighborChunkX <= chunk.chunkX + 1; neighborChunkX++) {
        if (neighborChunkX === chunk.chunkX && neighborChunkZ === chunk.chunkZ) {
          continue;
        }

        const neighbor = this.level.getChunk(neighborChunkX, neighborChunkZ, false);
        if (neighbor === null) {
          throw new Error(
            `Missing lighting neighbor chunk (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) for (${chunk.chunkX.toString()}, ${chunk.chunkZ.toString()})`,
          );
        }
        if (!this.level.hasChunkStatus(neighborChunkX, neighborChunkZ, GeneratedChunkStatus.FEATURES)) {
          throw new Error(
            `Lighting neighbor (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) for (${chunk.chunkX.toString()}, ${chunk.chunkZ.toString()}) is not at FEATURES`,
          );
        }

        neighbors.push({
          chunkX: neighborChunkX,
          chunkZ: neighborChunkZ,
          chunkRevision: this.getChunkRevision(neighborChunkX, neighborChunkZ),
        });
      }
    }

    return neighbors;
  }

  private async upsertLightInputChunk(chunk: GeneratedLevelChunk, chunkViewRevision: number): Promise<void> {
    const key = chunkKey(chunk.chunkX, chunk.chunkZ);
    if (!this.level.hasChunkStatus(chunk.chunkX, chunk.chunkZ, GeneratedChunkStatus.FEATURES)) {
      throw new Error(
        `Cannot upsert light input for (${chunk.chunkX.toString()}, ${chunk.chunkZ.toString()}) before FEATURES is complete`,
      );
    }

    const chunkRevision = this.getChunkRevision(chunk.chunkX, chunk.chunkZ);
    if (this.sentLightInputRevisions.get(key) === chunkRevision) {
      return;
    }

    await this.lightingService!.upsertChunk({
      type: "upsert_light_chunk",
      chunkViewRevision,
      chunkX: chunk.chunkX,
      chunkZ: chunk.chunkZ,
      chunkRevision,
      decorated: true,
      sections: this.buildLightInputSections(chunk),
    });
    this.sentLightInputRevisions.set(key, chunkRevision);
    this.acceptedChunkLight.delete(key);
    this.acceptedChunkLightRevisions.delete(key);
    this.clearPendingLightBlockChangesForChunk(chunk.chunkX, chunk.chunkZ);
  }

  private buildLightInputSections(chunk: GeneratedLevelChunk): readonly { readonly y: number; readonly blockStateIds: Uint32Array }[] {
    const sections: Array<{ readonly y: number; readonly blockStateIds: Uint32Array }> = [];
    for (const section of chunk.getStoredSections()) {
      const blockStateIds = new Uint32Array(LEVEL_CHUNK_SECTION_SIZE);
      for (let index = 0; index < LEVEL_CHUNK_SECTION_SIZE; index++) {
        blockStateIds[index] = this.options.blockStateIds.idFor(section.getBlockStateByIndex(index));
      }

      sections.push({ y: section.sectionY, blockStateIds });
    }

    return sections;
  }

  private acceptInitialLightResult(
    result: ChunkLightReadyResult,
    pending: Map<string, number>,
    currentChunkViewRevision: number,
  ): boolean {
    const key = chunkKey(result.chunkX, result.chunkZ);
    if (
      result.chunkViewRevision !== currentChunkViewRevision
      || pending.get(key) !== result.chunkRevision
      || this.getChunkRevision(result.chunkX, result.chunkZ) !== result.chunkRevision
      || !this.level.isChunkFeaturesStable(result.chunkX, result.chunkZ)
    ) {
      return false;
    }

    this.acceptedChunkLight.set(key, result.light);
    this.acceptedChunkLightRevisions.set(key, result.chunkRevision);
    pending.delete(key);
    return true;
  }

  private acceptLightDeltaResult(
    result: ChunkLightDeltaResult,
    currentChunkViewRevision: number,
    options: { readonly enqueueMessage?: boolean } = {},
  ): boolean {
    if (
      result.chunkViewRevision !== currentChunkViewRevision
      || this.getChunkRevision(result.chunkX, result.chunkZ) !== result.chunkRevision
    ) {
      return false;
    }

    const key = chunkKey(result.chunkX, result.chunkZ);
    const currentLight = this.acceptedChunkLight.get(key);
    if (currentLight === undefined) {
      return false;
    }

    this.acceptedChunkLight.set(key, applyPackedChunkLightDelta(currentLight, result.light));
    this.acceptedChunkLightRevisions.set(key, result.chunkRevision);

    if (options.enqueueMessage ?? true) {
      this.pendingMessages.push({
        type: "chunk_light_delta",
        chunkX: result.chunkX,
        chunkZ: result.chunkZ,
        light: result.light,
      });
    }

    return true;
  }

  private handleLightError(result: LightErrorResult, currentChunkViewRevision: number): void {
    if (result.chunkViewRevision !== undefined && result.chunkViewRevision !== currentChunkViewRevision) {
      return;
    }

    throw new Error(result.message);
  }

  private enqueueDirtyLightDeltas(): void {
    if (this.pendingLightBlockChanges.size === 0) {
      return;
    }

    this.queueStorageSideEffect(() => this.flushPendingLightBlockChanges(yieldToEventLoop));
  }

  private async flushPendingLightBlockChanges(yieldStep: () => Promise<void>): Promise<void> {
    const lightingService = this.lightingService;
    if (lightingService === undefined || this.pendingLightBlockChanges.size === 0) {
      return;
    }

    const batchChanges: PendingLightBlockChange[] = [];
    const batchChangeKeys: bigint[] = [];
    const chunkRevisions = new Map<string, {
      readonly chunkViewRevision: number;
      readonly chunkX: number;
      readonly chunkZ: number;
      readonly chunkRevision: number;
    }>();
    const chunkViewRevision = this.chunkViewJobRevision;

    for (const [posKey, change] of this.pendingLightBlockChanges) {
      const changedChunkKey = chunkKey(change.chunkX, change.chunkZ);
      const sentRevision = this.sentLightInputRevisions.get(changedChunkKey);
      if (sentRevision === undefined || this.level.getChunk(change.chunkX, change.chunkZ, false) === null) {
        this.pendingLightBlockChanges.delete(posKey);
        continue;
      }

      const currentRevision = this.getChunkRevision(change.chunkX, change.chunkZ);
      if (sentRevision === currentRevision) {
        this.pendingLightBlockChanges.delete(posKey);
        continue;
      }

      batchChanges.push(change);
      batchChangeKeys.push(posKey);
      this.collectLightDeltaRevisionWindow(change.chunkX, change.chunkZ, chunkViewRevision, chunkRevisions);
    }

    if (batchChanges.length === 0 || chunkRevisions.size === 0) {
      return;
    }

    const batchId = this.nextLightBlockChangeBatchId++;
    const revisionList = [...chunkRevisions.values()];
    await lightingService.enqueueBlockChanges({
      type: "block_light_update_batch",
      batchId,
      chunkViewRevision,
      changes: batchChanges.map((change) => ({
        x: change.x,
        y: change.y,
        z: change.z,
        oldBlockStateId: change.oldBlockStateId,
        newBlockStateId: change.newBlockStateId,
      })),
      chunkRevisions: revisionList,
    });

    const acceptedDeltaChunks = new Set<string>();
    for (let attempt = 0; attempt < 10_000; attempt++) {
      await yieldStep();
      const batch = await lightingService.pollResults({
        type: "poll_light_results",
        maxResults: LIGHTING_RESULT_BATCH_SIZE,
      });
      if (batch.results.length === 0) {
        await yieldToEventLoop();
        continue;
      }

      for (const result of batch.results) {
        switch (result.type) {
          case "chunk_light_ready":
            break;
          case "chunk_light_delta": {
            const accepted = this.acceptLightDeltaResult(result, chunkViewRevision, {
              enqueueMessage: this.shouldPublishLightDeltaMessage(result.chunkX, result.chunkZ),
            });
            if (accepted) {
              acceptedDeltaChunks.add(chunkKey(result.chunkX, result.chunkZ));
            }
            break;
          }
          case "block_light_update_complete":
            if (result.batchId !== batchId || result.chunkViewRevision !== chunkViewRevision) {
              break;
            }

            for (const revision of result.chunkRevisions) {
              const key = chunkKey(revision.chunkX, revision.chunkZ);
              this.sentLightInputRevisions.set(key, revision.chunkRevision);
              if (!acceptedDeltaChunks.has(key) && this.acceptedChunkLight.has(key)) {
                this.acceptedChunkLightRevisions.set(key, revision.chunkRevision);
              }
            }

            for (let index = 0; index < batchChangeKeys.length; index++) {
              const key = batchChangeKeys[index]!;
              const sentChange = batchChanges[index]!;
              const current = this.pendingLightBlockChanges.get(key);
              if (
                current !== undefined
                && current.x === sentChange.x
                && current.y === sentChange.y
                && current.z === sentChange.z
                && current.oldBlockStateId === sentChange.oldBlockStateId
                && current.newBlockStateId === sentChange.newBlockStateId
              ) {
                this.pendingLightBlockChanges.delete(key);
              }
            }
            return;
          case "light_error":
            this.handleLightError(result, chunkViewRevision);
            break;
          case "light_progress":
          case "light_performance":
            break;
        }
      }
    }

    throw new Error("Lighting worker did not complete block light update batch");
  }

  private collectLightDeltaRevisionWindow(
    chunkX: number,
    chunkZ: number,
    chunkViewRevision: number,
    revisions: Map<string, {
      readonly chunkViewRevision: number;
      readonly chunkX: number;
      readonly chunkZ: number;
      readonly chunkRevision: number;
    }>,
  ): void {
    for (let neighborChunkZ = chunkZ - 1; neighborChunkZ <= chunkZ + 1; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - 1; neighborChunkX <= chunkX + 1; neighborChunkX++) {
        const key = chunkKey(neighborChunkX, neighborChunkZ);
        if (this.sentLightInputRevisions.get(key) === undefined || this.level.getChunk(neighborChunkX, neighborChunkZ, false) === null) {
          continue;
        }

        revisions.set(key, {
          chunkViewRevision,
          chunkX: neighborChunkX,
          chunkZ: neighborChunkZ,
          chunkRevision: this.getChunkRevision(neighborChunkX, neighborChunkZ),
        });
      }
    }
  }

  private shouldPublishLightDeltaMessage(chunkX: number, chunkZ: number): boolean {
    const key = chunkKey(chunkX, chunkZ);
    return this.publishedChunkSnapshots.has(key)
      && !this.dirtyChunksForPublication.has(key)
      && this.isChunkInCurrentPublishView(chunkX, chunkZ);
  }

  private clearPendingLightBlockChangesForChunk(chunkX: number, chunkZ: number): void {
    for (const [key, change] of this.pendingLightBlockChanges) {
      if (change.chunkX === chunkX && change.chunkZ === chunkZ) {
        this.pendingLightBlockChanges.delete(key);
      }
    }
  }

  private discardChunkLighting(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    const chunkRevision = this.getChunkRevision(chunkX, chunkZ);
    this.sentLightInputRevisions.delete(key);
    this.acceptedChunkLight.delete(key);
    this.acceptedChunkLightRevisions.delete(key);
    this.clearPendingLightBlockChangesForChunk(chunkX, chunkZ);
    this.chunkRevisions.delete(key);
    const lightingService = this.lightingService;
    if (lightingService !== undefined) {
      this.queueStorageSideEffect(() => lightingService.removeChunk({
        type: "remove_light_chunk",
        chunkViewRevision: this.chunkViewJobRevision,
        chunkX,
        chunkZ,
        chunkRevision,
      }), key);
    }
  }

  private discardUnloadedChunkState(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    this.publishedChunkSnapshots.delete(key);
    this.forgetPublishedEntitiesForChunk(chunkX, chunkZ);
    this.dirtyChunksForPublication.delete(key);
    const holder = this.chunkHolders.get(key);
    const changed = holder === undefined
      ? this.applyEntityChunkStatus(chunkX, chunkZ, FullChunkStatus.INACCESSIBLE)
      : this.updateChunkHolderFullStatus(holder, this.getCurrentChunkHolderFullStatus(chunkX, chunkZ));
    if (changed) {
      this.entityRuntime.processLifecycle();
      this.noteEntityChunkStatusCounts();
      this.noteChunkFullStatusCounts();
    }
    this.discardChunkLighting(chunkX, chunkZ);
  }

  private forgetPublishedEntitiesForChunk(chunkX: number, chunkZ: number): void {
    for (const [entityId, snapshot] of this.publishedEntitySnapshots) {
      if (snapshot.chunkX === chunkX && snapshot.chunkZ === chunkZ) {
        this.publishedEntitySnapshots.delete(entityId);
      }
    }
  }

  private ensureLocalSessionState(): void {
    if (this.worldOpened === undefined) {
      throw new Error("Local generated world session was accessed before openWorld()");
    }

    this.sessionState ??= {
      sessionId: LOCAL_WORLD_SESSION_ID,
      playerId: LOCAL_PLAYER_ID,
      playerProfile: DEFAULT_PLAYER_PROFILE,
      saveId: this.worldOpened.saveMetadata.saveId,
      resumed: false,
      revision: 0,
      chunkView: undefined,
    };
    this.playerState ??= createInitialPlayerState(LOCAL_PLAYER_ID);
  }

  private updateSessionState(): void {
    this.sessionState = {
      ...this.sessionState!,
      revision: this.sessionState!.revision + 1,
      chunkView: createSessionChunkViewState(this.currentChunkView),
    };
  }

  private createSessionStateMessage(): Extract<WorldHostMessage, { type: "session_state" }> {
    return {
      type: "session_state",
      state: this.sessionState!,
    };
  }

  private createPlayerStateMessage(): Extract<WorldHostMessage, { type: "player_state" }> {
    return {
      type: "player_state",
      state: this.playerState!,
    };
  }

  private tickPlayerLoop(nowMs = this.nowMs()): void {
    if (this.playerState === undefined) {
      return;
    }

    const dueTicks = Math.floor((nowMs - this.lastPlayerTickAtMs) / PLAYER_TICK_INTERVAL_MS);
    if (dueTicks <= 0) {
      return;
    }

    for (let tickIndex = 0; tickIndex < dueTicks; tickIndex++) {
      const nextPlayerState = tickPlayerStateWithCommandQueue(
        this.playerState,
        this.playerCommandQueue,
        this.playerState.tick + 1,
        createGeneratedLevelCollisionWorld(this.level),
      );
      if (nextPlayerState !== this.playerState) {
        this.playerState = nextPlayerState;
        this.enqueuePlayerStateMessage();
      }
    }

    this.lastPlayerTickAtMs += dueTicks * PLAYER_TICK_INTERVAL_MS;
  }

  private enqueuePlayerStateMessage(): void {
    this.pendingMessages = this.pendingMessages.filter((message) => message.type !== "player_state");
    this.pendingMessages.push(this.createPlayerStateMessage());
    this.notePendingMessageBacklog();
  }

  private enqueueWorldError(error: unknown): void {
    const message = error instanceof Error ? error.message : String(error);
    this.pendingMessages.push({ type: "world_error", message });
    this.notePendingMessageBacklog();
  }

  private drainPendingMessages(maxMessages: number | undefined): readonly WorldHostMessage[] {
    this.notePendingMessageBacklog();
    const drained = drainWorldHostMessages(this.pendingMessages, maxMessages);
    this.pendingMessages = drained.remaining;
    return [...drained.messages, this.createWorldPerformanceMessage()];
  }

  private createWorldPerformanceMessage(): WorldPerformanceMessage {
    return {
      type: "world_perf",
      performance: {
        lighting: this.lightingService?.getPerformanceCounters?.(),
        worldgen: this.createWorldgenPerformanceCounters(),
      },
    };
  }

  private incrementWorldgenCount(name: string, amount = 1): void {
    this.worldgenPerformance.counts.set(name, (this.worldgenPerformance.counts.get(name) ?? 0) + amount);
  }

  private setWorldgenCount(name: string, value: number): void {
    this.worldgenPerformance.counts.set(name, value);
  }

  private noteChunkHolderCount(): void {
    const count = this.chunkHolders.size;
    this.setWorldgenCount("chunk_holders_resident_current", count);
    this.setWorldgenCount(
      "chunk_holders_resident_max",
      Math.max(this.worldgenPerformance.counts.get("chunk_holders_resident_max") ?? 0, count),
    );
  }

  private notePendingUnloadChunkHolderCount(): void {
    const count = this.pendingUnloadChunkHolders.size;
    this.setWorldgenCount("chunk_holders_pending_unload_current", count);
    this.setWorldgenCount(
      "chunk_holders_pending_unload_max",
      Math.max(this.worldgenPerformance.counts.get("chunk_holders_pending_unload_max") ?? 0, count),
    );
  }

  private noteChunkHolderUnloadQueueCount(): void {
    const count = this.chunkHolderUnloadQueue.size;
    this.setWorldgenCount("chunk_holders_unload_queue_current", count);
    this.setWorldgenCount(
      "chunk_holders_unload_queue_max",
      Math.max(this.worldgenPerformance.counts.get("chunk_holders_unload_queue_max") ?? 0, count),
    );
  }

  private noteChunkResidencyTicketCount(): void {
    const tickets = this.chunkResidencyTickets.getDebugRecords();
    const ticketedChunks = this.chunkResidencyTickets.getCoveredChunkCount();
    this.setWorldgenCount("chunk_residency_tickets_current", tickets.length);
    this.setWorldgenCount("chunk_residency_ticketed_chunks_current", ticketedChunks);
    this.setWorldgenCount(
      "chunk_residency_ticketed_chunks_max",
      Math.max(this.worldgenPerformance.counts.get("chunk_residency_ticketed_chunks_max") ?? 0, ticketedChunks),
    );
  }

  private noteChunkFullStatusCounts(): void {
    let inaccessible = 0;
    let border = 0;
    let ticking = 0;
    let entityTicking = 0;
    for (const holder of this.chunkHolders.values()) {
      switch (holder.fullStatus) {
        case FullChunkStatus.INACCESSIBLE:
          inaccessible++;
          break;
        case FullChunkStatus.BORDER:
          border++;
          break;
        case FullChunkStatus.TICKING:
          ticking++;
          break;
        case FullChunkStatus.ENTITY_TICKING:
          entityTicking++;
          break;
      }
    }

    this.setWorldgenCount("chunk_full_status_holders_current", this.chunkHolders.size);
    this.setWorldgenCount("chunk_full_statuses_inaccessible_current", inaccessible);
    this.setWorldgenCount("chunk_full_statuses_border_current", border);
    this.setWorldgenCount("chunk_full_statuses_ticking_current", ticking);
    this.setWorldgenCount("chunk_full_statuses_entity_ticking_current", entityTicking);
  }

  private noteEntityChunkStatusCounts(): void {
    let ticking = 0;
    let tracked = 0;
    for (const status of this.entityChunkStatuses.values()) {
      if (status === FullChunkStatus.ENTITY_TICKING) {
        ticking++;
      } else {
        tracked++;
      }
    }

    this.setWorldgenCount("entity_chunk_statuses_current", this.entityChunkStatuses.size);
    this.setWorldgenCount("entity_chunk_statuses_ticking_current", ticking);
    this.setWorldgenCount("entity_chunk_statuses_tracked_current", tracked);
  }

  private notePendingStorageWriteCount(): void {
    const count = this.pendingStorageWrites.size;
    this.setWorldgenCount("storage_pending_writes_current", count);
    this.setWorldgenCount(
      "storage_pending_writes_max",
      Math.max(this.worldgenPerformance.counts.get("storage_pending_writes_max") ?? 0, count),
    );
  }

  private noteStorageSideEffectQueueDepth(): void {
    this.setWorldgenCount("storage_side_effect_queue_depth_current", this.pendingStorageSideEffects);
    this.setWorldgenCount(
      "storage_side_effect_queue_depth_max",
      Math.max(this.worldgenPerformance.counts.get("storage_side_effect_queue_depth_max") ?? 0, this.pendingStorageSideEffects),
    );
  }

  private noteActiveStorageSideEffectCount(): void {
    this.setWorldgenCount("storage_side_effects_active_current", this.activeStorageSideEffects);
    this.setWorldgenCount(
      "storage_side_effects_active_max",
      Math.max(this.worldgenPerformance.counts.get("storage_side_effects_active_max") ?? 0, this.activeStorageSideEffects),
    );
  }

  private noteStorageSideEffectKeyCount(): void {
    this.setWorldgenCount("storage_side_effect_keys_current", this.storageSideEffectKeyTails.size);
    this.setWorldgenCount(
      "storage_side_effect_keys_max",
      Math.max(this.worldgenPerformance.counts.get("storage_side_effect_keys_max") ?? 0, this.storageSideEffectKeyTails.size),
    );
  }

  private recordWorldgenDuration(phase: string, elapsedMs: number): void {
    const existing = this.worldgenPerformance.phases.get(phase);
    if (existing === undefined) {
      this.worldgenPerformance.phases.set(phase, {
        count: 1,
        totalMs: elapsedMs,
        maxMs: elapsedMs,
      });
      return;
    }

    existing.count++;
    existing.totalMs += elapsedMs;
    existing.maxMs = Math.max(existing.maxMs, elapsedMs);
  }

  private recordWorldgenPhase<T>(phase: string, run: () => T): T {
    const startedAtMs = monotonicNowMs();
    try {
      return run();
    } finally {
      this.recordWorldgenDuration(phase, monotonicNowMs() - startedAtMs);
    }
  }

  private async recordWorldgenPhaseAsync<T>(phase: string, run: () => Promise<T>): Promise<T> {
    const startedAtMs = monotonicNowMs();
    try {
      return await run();
    } finally {
      this.recordWorldgenDuration(phase, monotonicNowMs() - startedAtMs);
    }
  }

  private notePendingMessageBacklog(): void {
    this.worldgenPerformance.maxPendingMessages = Math.max(
      this.worldgenPerformance.maxPendingMessages,
      this.pendingMessages.length,
    );
  }

  private createWorldgenPerformanceCounters(): WorldgenPerformanceCounters {
    const phases: Record<string, WorldgenPhasePerformanceCounters> = {};
    for (const [name, counters] of this.worldgenPerformance.phases) {
      phases[name] = {
        count: counters.count,
        totalMs: counters.totalMs,
        maxMs: counters.maxMs,
      };
    }

    return {
      phases,
      counts: Object.fromEntries(this.worldgenPerformance.counts),
      maxPendingMessages: this.worldgenPerformance.maxPendingMessages,
      currentPendingMessages: this.pendingMessages.length,
      ...(this.activeChunkViewJobRevision === undefined ? {} : { activeChunkViewJobRevision: this.activeChunkViewJobRevision }),
    };
  }
}
