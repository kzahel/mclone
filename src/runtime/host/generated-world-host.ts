import { BlockPos } from "../../core/block-pos";
import { SectionPos } from "../../core/section-pos";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import { ChunkBiomeContainer } from "../../worldgen/biome/chunk-biome-container";
import { NoiseBasedChunkGenerator } from "../../worldgen/levelgen/noise-based-chunk-generator";
import {
  buildChunkSnapshot,
  createBlockStateResolver,
  hydrateChunkFromSnapshot,
  type ChunkSnapshot,
} from "../../world/level/chunk-snapshot";
import {
  applyPackedChunkLightDelta,
  packChunkSnapshot,
  unpackChunkSnapshot,
  type PackedChunkLight,
  type PackedChunkSnapshot,
} from "../../world/level/packed-chunk-snapshot";
import {
  FEATURES_CHUNK_DEPENDENCY_RADIUS,
  FEATURES_WRITE_RADIUS_CUTOFF,
} from "../../world/level/generated-decoration-region";
import { GeneratedChunkStatus } from "../../world/level/generated-chunk-status";
import { GeneratedRenderLevel } from "../../world/level/generated-render-level";
import type { LevelChunk } from "../../world/level/chunk/level-chunk";
import { LEVEL_CHUNK_SECTION_SIZE } from "../../world/level/chunk/level-chunk-section";
import { FullChunkStatus } from "../../world/level/entity/full-chunk-status";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { BlockStateIdMap } from "../../world/level/block/state/block-state-id";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { Fluid } from "../../world/level/material/fluid";
import { Fluids } from "../../world/level/material/fluids";
import type { GeneratedMobEntity } from "../../world/entity/entity-type";
import type {
  ChunkLightDeltaResult,
  ChunkLightReadyResult,
  LightErrorResult,
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
import type { WorldHost } from "../protocol/world-host";
import { drainWorldHostMessages } from "../protocol/world-message-queue";
import {
  normalizeWorldEngineConfig,
  type ClientPlayerState,
  type ClientSessionState,
  type EntitySnapshot,
  type EntitySnapshotCategory,
  type EntitySnapshotMessage,
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
  type WorldProgressMessage,
} from "../protocol/world-messages";
import {
  anchorPlayerStateToChunkView,
  createPlayerCommandQueue,
  createInitialPlayerState,
  enqueuePlayerInputCommand,
  PLAYER_TICK_INTERVAL_MS,
  tickPlayerStateWithCommandQueue,
  type PlayerCommandQueue,
} from "../session/player-loop";
import {
  createWorldSaveMetadata,
  type OpenWorldStorageRequest,
  type WorldStorage,
  type WorldStorageSession,
} from "../storage/world-storage";
import { EntityRuntime } from "./entity-runtime";
import { LiquidSimulationLevel } from "./liquid-simulation-level";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function sortChunkCoordinates(
  chunks: Iterable<readonly [number, number]>,
): readonly (readonly [number, number])[] {
  return [...chunks].sort((a, b) => a[1] - b[1] || a[0] - b[0]);
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

const LOCAL_WORLD_SESSION_ID = "local";
const COOPERATIVE_CHUNK_PHASE_BUDGET_MS = 8;
const LIGHTING_RESULT_BATCH_SIZE = 64;
const LIQUID_TICK_READ_RADIUS_BLOCKS = 4;

interface ChunkViewJobRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  loadedFromStorage: boolean;
}

interface EnsureLightingOptions {
  readonly isCancelled?: () => boolean;
  readonly onInitialLightReady?: (chunk: GeneratedLevelChunk) => Promise<void>;
}

interface PublishReadyChunksOptions {
  readonly targetMessages?: WorldHostMessage[];
  readonly awaitStorage?: boolean;
  readonly onChunkPublished?: (publishedInPass: number) => void;
}

type StoredChunkPreloadResult = "loaded" | "already_loaded" | "missing";

type GeneratedLevelChunk = LevelChunk;

interface PendingLightBlockChange {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly oldBlockStateId: number;
  readonly newBlockStateId: number;
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

export class GeneratedWorldHost implements WorldHost {
  private readonly biomeSource;
  private readonly generator;
  private readonly level;
  private readonly lightingService: LightingService | undefined;
  private readonly liquidLevel: LiquidSimulationLevel;
  private readonly liquidTicks: ServerTickList<Fluid>;
  private readonly entityRuntime = new EntityRuntime<GeneratedMobEntity>();
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
  private readonly chunkRevisions = new Map<string, number>();
  private readonly sentLightInputRevisions = new Map<string, number>();
  private readonly acceptedChunkLight = new Map<string, PackedChunkLight>();
  private readonly acceptedChunkLightRevisions = new Map<string, number>();
  private readonly pendingLightBlockChanges = new Map<bigint, PendingLightBlockChange>();
  private readonly dirtyChunksForPublication = new Set<string>();
  private readonly dirtyDurableChunks = new Set<string>();
  private readonly spawnedOriginalMobChunks = new Set<string>();
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
    this.biomeSource = new OverworldBiomeSource(options.seed);
    this.generator = new NoiseBasedChunkGenerator(this.biomeSource, options.seed);
    this.resolveBlockState = createBlockStateResolver(options.airState);
    this.level = new GeneratedRenderLevel(
      options.airState,
      this.generator,
      this.biomeSource,
      options.seed,
      options.blockStateById,
    );
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
    await this.setLightingView(request, this.chunkViewJobRevision);

    const messages: WorldHostMessage[] = [this.createSessionStateMessage()];
    if (!this.playerAnchoredToChunkView) {
      this.playerState = anchorPlayerStateToChunkView(this.playerState!, createSessionChunkViewState(request)!, this.playerState!.tick);
      this.playerAnchoredToChunkView = true;
      messages.push(this.createPlayerStateMessage());
    }

    if (!update.changed) {
      return messages;
    }

    const jobs = this.collectChunkViewJobs(update.missingChunks);
    for (const [chunkX, chunkZ] of jobs) {
      if (this.isChunkInCurrentAuthorityView(chunkX, chunkZ)) {
        await this.preloadStoredChunk(chunkX, chunkZ);
      }
    }

    for (const [chunkX, chunkZ] of jobs) {
      if (
        this.isChunkInCurrentAuthorityView(chunkX, chunkZ)
        && !this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS)
      ) {
        this.level.generateChunkTerrain(chunkX, chunkZ);
      }
    }

    for (const [chunkX, chunkZ] of jobs) {
      if (
        this.isChunkInCurrentFeaturesView(chunkX, chunkZ)
        && this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS)
        && !this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)
      ) {
        this.level.decorateChunk(chunkX, chunkZ);
      }
    }

    this.options.mutateWorld?.(this.liquidLevel);
    await this.ensureFullStatusesForCurrentView(this.chunkViewJobRevision);

    for (const chunk of update.removedChunks) {
      await this.saveDirtyChunkBeforeUnload(chunk);
      await this.storageSession?.chunks.evictChunk(chunk.chunkX, chunk.chunkZ);
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
    }

    for (const chunk of update.unloadedChunks) {
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
      messages.push({
        type: "chunk_unload",
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
      });
    }

    await this.publishReadyChunksForCurrentView(this.chunkViewJobRevision, undefined, {
      targetMessages: messages,
      awaitStorage: true,
    });

    return messages;
  }

  private async setChunkViewCooperative(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
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

    for (const chunk of update.removedChunks) {
      this.queueStorageSideEffect(this.queueEvictAfterDirtySave(chunk));
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
    }

    for (const chunk of update.unloadedChunks) {
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
      messages.push({
        type: "chunk_unload",
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
      });
    }

    const chunkViewJobRevision = chunkViewChanged ? ++this.chunkViewJobRevision : this.chunkViewJobRevision;
    await this.setLightingView(request, chunkViewJobRevision);
    if (!update.changed) {
      return messages;
    }

    const chunkViewJob = this.runChunkViewJobs(this.collectChunkViewJobs(update.missingChunks), chunkViewJobRevision);
    chunkViewJob.catch((error: unknown) => {
      this.enqueueWorldError(error);
    });
    return messages;
  }

  private collectChunkViewJobs(missingChunks: readonly (readonly [number, number])[]): readonly (readonly [number, number])[] {
    const jobs = new Map<string, readonly [number, number]>();
    for (const chunk of missingChunks) {
      jobs.set(chunkKey(chunk[0], chunk[1]), chunk);
    }

    if (this.currentChunkView === undefined) {
      return sortChunkCoordinates(jobs.values());
    }

    const featuresRadius = getGeneratedWorldFeaturesChunkRadius(this.currentChunkView.radius);
    const featuresTerrainRadius = featuresRadius + FEATURES_WRITE_RADIUS_CUTOFF;
    for (let chunkZ = this.currentChunkView.centerChunkZ - featuresTerrainRadius; chunkZ <= this.currentChunkView.centerChunkZ + featuresTerrainRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - featuresTerrainRadius; chunkX <= this.currentChunkView.centerChunkX + featuresTerrainRadius; chunkX++) {
        if (this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.LIQUID_CARVERS)) {
          continue;
        }

        jobs.set(chunkKey(chunkX, chunkZ), [chunkX, chunkZ]);
      }
    }

    // Runtime: FEATURES initializes the rest of its vanilla dependency window as metadata-only.
    for (let chunkZ = this.currentChunkView.centerChunkZ - featuresRadius; chunkZ <= this.currentChunkView.centerChunkZ + featuresRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - featuresRadius; chunkX <= this.currentChunkView.centerChunkX + featuresRadius; chunkX++) {
        if (this.level.hasChunkStatus(chunkX, chunkZ, GeneratedChunkStatus.FEATURES)) {
          continue;
        }

        jobs.set(chunkKey(chunkX, chunkZ), [chunkX, chunkZ]);
      }
    }

    const fullRadius = getGeneratedWorldFullChunkRadius(this.currentChunkView.radius);
    for (let chunkZ = this.currentChunkView.centerChunkZ - fullRadius; chunkZ <= this.currentChunkView.centerChunkZ + fullRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - fullRadius; chunkX <= this.currentChunkView.centerChunkX + fullRadius; chunkX++) {
        if (this.level.isChunkFull(chunkX, chunkZ)) {
          continue;
        }

        jobs.set(chunkKey(chunkX, chunkZ), [chunkX, chunkZ]);
      }
    }

    const publishRadius = getGeneratedWorldViewChunkRadius(this.currentChunkView.radius);
    for (let chunkZ = this.currentChunkView.centerChunkZ - publishRadius; chunkZ <= this.currentChunkView.centerChunkZ + publishRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - publishRadius; chunkX <= this.currentChunkView.centerChunkX + publishRadius; chunkX++) {
        const key = chunkKey(chunkX, chunkZ);
        if (this.publishedChunkSnapshots.has(key)) {
          continue;
        }

        jobs.set(key, [chunkX, chunkZ]);
      }
    }

    return sortChunkCoordinates(jobs.values());
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
    this.lightingService?.close?.();
  }

  private async preloadStoredChunk(chunkX: number, chunkZ: number): Promise<StoredChunkPreloadResult> {
    if (this.level.getChunk(chunkX, chunkZ, false) !== null) {
      return "already_loaded";
    }

    if (this.storageSession === undefined) {
      return "missing";
    }

    const snapshot = await this.storageSession.chunks.loadChunk(chunkX, chunkZ);
    if (snapshot === undefined) {
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
    return "loaded";
  }

  private async writeChunkSnapshotByPolicy(snapshot: PackedChunkSnapshot): Promise<void> {
    const key = chunkKey(snapshot.chunkX, snapshot.chunkZ);
    if (this.dirtyDurableChunks.has(key)) {
      await this.saveDirtyChunkSnapshot(key, snapshot);
      return;
    }

    await this.cacheGeneratedChunkSnapshot(snapshot);
  }

  private async cacheGeneratedChunkSnapshot(snapshot: PackedChunkSnapshot): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    await this.storageSession.chunks.saveChunk(withoutPersistedLight(snapshot));
  }

  private async saveDirtyChunkSnapshot(key: string, snapshot: PackedChunkSnapshot): Promise<void> {
    if (this.storageSession === undefined) {
      this.dirtyDurableChunks.delete(key);
      return;
    }

    await this.storageSession.chunks.saveChunk(withoutPersistedLight(snapshot));
    this.dirtyDurableChunks.delete(key);
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

    await this.storageSession?.chunks.evictChunk(chunk.chunkX, chunk.chunkZ);
  }

  private async runChunkViewJobs(
    missingChunks: readonly (readonly [number, number])[],
    chunkViewJobRevision: number,
  ): Promise<void> {
    this.activeChunkViewJobRevision = chunkViewJobRevision;
    try {
      await this.runChunkViewJobsInternal(missingChunks, chunkViewJobRevision);
    } finally {
      if (this.activeChunkViewJobRevision === chunkViewJobRevision) {
        this.activeChunkViewJobRevision = undefined;
      }
    }
  }

  private async runChunkViewJobsInternal(
    missingChunks: readonly (readonly [number, number])[],
    chunkViewJobRevision: number,
  ): Promise<void> {
    const yieldStep = createCooperativeYield();
    const jobs: ChunkViewJobRecord[] = missingChunks.map(([chunkX, chunkZ]) => ({
      chunkX,
      chunkZ,
      loadedFromStorage: false,
    }));
    if (jobs.length === 0) {
      return;
    }

    let checkedChunks = 0;
    let storedChunks = 0;
    let existingChunks = 0;
    let missingStoredChunks = 0;
    const storageDetail = (): string =>
      `stored ${storedChunks.toString()}, existing ${existingChunks.toString()}, missing ${missingStoredChunks.toString()}`;
    this.enqueueWorldProgress("Checking saved chunks", checkedChunks, jobs.length, chunkViewJobRevision, storageDetail());
    for (const job of jobs) {
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      if (this.isChunkInCurrentAuthorityView(job.chunkX, job.chunkZ)) {
        const preloadResult = await this.preloadStoredChunk(job.chunkX, job.chunkZ);
        if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
          return;
        }

        switch (preloadResult) {
          case "loaded":
            job.loadedFromStorage = true;
            storedChunks++;
            break;
          case "already_loaded":
            existingChunks++;
            break;
          case "missing":
            missingStoredChunks++;
            break;
        }
      }

      checkedChunks++;
      this.enqueueWorldProgress("Checking saved chunks", checkedChunks, jobs.length, chunkViewJobRevision, storageDetail());
    }

    const generationJobs = jobs.filter((job) =>
      this.isChunkInCurrentAuthorityView(job.chunkX, job.chunkZ)
      && !job.loadedFromStorage
      && !this.level.hasChunkStatus(job.chunkX, job.chunkZ, GeneratedChunkStatus.LIQUID_CARVERS)
    );
    let terrainDone = 0;
    this.enqueueWorldProgress("Generating status chunks", terrainDone, generationJobs.length, chunkViewJobRevision);
    for (const job of generationJobs) {
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      if (
        this.isChunkInCurrentAuthorityView(job.chunkX, job.chunkZ)
        && !this.level.hasChunkStatus(job.chunkX, job.chunkZ, GeneratedChunkStatus.LIQUID_CARVERS)
      ) {
        await this.level.generateChunkTerrainCooperative(job.chunkX, job.chunkZ, yieldStep);
      }

      terrainDone++;
      this.enqueueWorldProgress("Generating status chunks", terrainDone, generationJobs.length, chunkViewJobRevision);
    }

    const decorationJobs = jobs.filter((job) =>
      !job.loadedFromStorage
      && this.isChunkInCurrentFeaturesView(job.chunkX, job.chunkZ)
      && this.level.hasChunkStatus(job.chunkX, job.chunkZ, GeneratedChunkStatus.LIQUID_CARVERS)
      && !this.level.hasChunkStatus(job.chunkX, job.chunkZ, GeneratedChunkStatus.FEATURES)
    );
    let decorationDone = 0;
    this.enqueueWorldProgress("Advancing FEATURES", decorationDone, decorationJobs.length, chunkViewJobRevision);
    for (const job of decorationJobs) {
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      if (
        this.isChunkInCurrentFeaturesView(job.chunkX, job.chunkZ)
        && this.level.hasChunkStatus(job.chunkX, job.chunkZ, GeneratedChunkStatus.LIQUID_CARVERS)
        && !this.level.hasChunkStatus(job.chunkX, job.chunkZ, GeneratedChunkStatus.FEATURES)
      ) {
        await this.level.decorateChunkCooperative(job.chunkX, job.chunkZ, yieldStep);
      }

      decorationDone++;
      this.enqueueWorldProgress("Advancing FEATURES", decorationDone, decorationJobs.length, chunkViewJobRevision);
    }

    const lightingChunks = this.collectFullStatusChunksForCurrentView();
    let lightingDone = 0;
    let publishDone = 0;
    this.enqueueWorldProgress("Computing light", lightingDone, lightingChunks.length, chunkViewJobRevision);
    const publishTotal = this.countUnpublishedChunksInCurrentPublishView();
    this.enqueueWorldProgress("Publishing chunks", publishDone, publishTotal, chunkViewJobRevision);
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
          this.enqueueWorldProgress("Computing light", lightingDone, lightingChunks.length, chunkViewJobRevision);
          publishDone += await this.publishReadyChunksForCurrentView(chunkViewJobRevision, yieldStep, {
            onChunkPublished: (publishedInPass) => {
              this.enqueueWorldProgress("Publishing chunks", publishDone + publishedInPass, publishTotal, chunkViewJobRevision);
            },
          });
          this.enqueueWorldProgress("Publishing chunks", publishDone, publishTotal, chunkViewJobRevision);
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

        this.markChunkFullWithoutLighting(chunk);
      }
    }
    publishDone += await this.publishReadyChunksForCurrentView(chunkViewJobRevision, yieldStep, {
      onChunkPublished: (publishedInPass) => {
        this.enqueueWorldProgress("Publishing chunks", publishDone + publishedInPass, publishTotal, chunkViewJobRevision);
      },
    });
    this.enqueueWorldProgress("Publishing chunks", publishDone, publishTotal, chunkViewJobRevision);
    this.enqueueWorldProgress("Computing light", lightingChunks.length, lightingChunks.length, chunkViewJobRevision);
    this.enqueueWorldProgress("Publishing chunks", publishTotal, publishTotal, chunkViewJobRevision);
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

  private isChunkInCurrentFeaturesView(chunkX: number, chunkZ: number): boolean {
    if (this.currentChunkView === undefined) {
      return false;
    }

    const viewRadius = getGeneratedWorldFeaturesChunkRadius(this.currentChunkView.radius);
    return Math.abs(chunkX - this.currentChunkView.centerChunkX) <= viewRadius
      && Math.abs(chunkZ - this.currentChunkView.centerChunkZ) <= viewRadius;
  }

  private isChunkInCurrentAuthorityView(chunkX: number, chunkZ: number): boolean {
    if (this.currentChunkView === undefined) {
      return false;
    }

    const viewRadius = getGeneratedWorldAuthorityChunkRadius(this.currentChunkView.radius);
    return Math.abs(chunkX - this.currentChunkView.centerChunkX) <= viewRadius
      && Math.abs(chunkZ - this.currentChunkView.centerChunkZ) <= viewRadius;
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
        this.markChunkFullWithoutLighting(chunk);
      }
      return;
    }

    await this.ensureLightingForLoadedChunks(chunks, chunkViewRevision);
  }

  private markChunkFullWithoutLighting(chunk: GeneratedLevelChunk): void {
    if (!this.level.isChunkFeaturesStable(chunk.chunkX, chunk.chunkZ)) {
      return;
    }

    this.level.markChunkLighted(chunk.chunkX, chunk.chunkZ);
    this.level.markChunkFull(chunk.chunkX, chunk.chunkZ);
  }

  private markChunkFullAfterAcceptedLight(chunkX: number, chunkZ: number): void {
    if (!this.level.isChunkFeaturesStable(chunkX, chunkZ)) {
      return;
    }

    this.level.markChunkLighted(chunkX, chunkZ);
    this.level.markChunkFull(chunkX, chunkZ);
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
    if (
      !this.liquidSimulationEnabled
      || this.publishedChunkSnapshots.size === 0
      || this.activeChunkViewJobRevision !== undefined
    ) {
      this.lastWorldTickAtMs += dueTicks * tickIntervalMs;
      return;
    }

    this.hydratePublishedLiquidTicks();
    for (let tickIndex = 0; tickIndex < dueTicks; tickIndex++) {
      this.gameTime++;
      this.liquidTicks.tick();
    }
    this.hydratePublishedLiquidTicks();
    this.lastWorldTickAtMs += dueTicks * tickIntervalMs;
    await this.flushDirtyPublishedChunks(
      this.options.chunkViewScheduling === "cooperative" ? createCooperativeYield() : undefined,
    );
    this.enqueueDirtyLightDeltas();
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

      const snapshot = this.buildPackedChunkSnapshot(chunk);
      await this.writeChunkSnapshotByPolicy(snapshot);
      if (!this.isChunkInCurrentPublishView(chunkX, chunkZ) || !this.publishedChunkSnapshots.has(key)) {
        this.dirtyChunksForPublication.delete(key);
        continue;
      }

      this.pendingMessages.push({
        type: "chunk_snapshot",
        snapshot,
      });
      this.dirtyChunksForPublication.delete(key);
    }
  }

  private queueStorageSideEffect(task: Promise<void> | undefined): void {
    if (task === undefined) {
      return;
    }

    task.catch((error: unknown) => {
      this.enqueueWorldError(error);
    });
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
    const snapshot = this.buildPackedChunkSnapshot(chunk);
    const messages = targetMessages ?? this.pendingMessages;
    messages.push({
      type: "chunk_snapshot",
      snapshot,
    });
    this.markChunkSnapshotPublished(chunk.chunkX, chunk.chunkZ);
    this.publishEntitySnapshotsForChunk(chunk.chunkX, chunk.chunkZ, messages);
    const writeTask = this.writeChunkSnapshotByPolicy(snapshot);
    if (awaitStorage) {
      await writeTask;
    } else {
      this.queueStorageSideEffect(writeTask);
    }
    return true;
  }

  private buildPackedChunkSnapshot(chunk: GeneratedLevelChunk): PackedChunkSnapshot {
    if (this.liquidSimulationEnabled) {
      this.hydrateLiquidTicksFromChunk(chunk);
    }
    const snapshot = this.withPendingLiquidTicks(
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
    );
    const light = this.getAcceptedCurrentChunkLight(chunk.chunkX, chunk.chunkZ);

    return packChunkSnapshot(
      light === undefined ? snapshot : { ...snapshot, light },
      this.options.blockStateIds,
      this.resolveBlockState,
    );
  }

  private ensureOriginalMobsForChunk(chunk: GeneratedLevelChunk): void {
    const key = chunkKey(chunk.chunkX, chunk.chunkZ);
    this.entityRuntime.updateChunkStatus(chunk.chunkX, chunk.chunkZ, FullChunkStatus.BORDER);
    this.entityRuntime.tick();
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
    this.entityRuntime.tick();
  }

  private publishEntitySnapshotsForChunk(chunkX: number, chunkZ: number, messages: WorldHostMessage[]): void {
    const snapshots = this.entityRuntime.manager.getEntityGetter().getAll()
      .filter((entity) => SectionPos.blockToSectionCoord(entity.blockPosition().getX()) === chunkX
        && SectionPos.blockToSectionCoord(entity.blockPosition().getZ()) === chunkZ)
      .sort((left, right) => left.id - right.id)
      .map((entity) => this.createEntitySnapshotMessage(entity));

    messages.push(...snapshots);
  }

  private createEntitySnapshotMessage(entity: GeneratedMobEntity): EntitySnapshotMessage {
    const data = Object.keys(entity.data).length === 0 ? undefined : entity.data;
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
      ...(data === undefined ? {} : { data }),
    };

    return {
      type: "entity_snapshot",
      entity: snapshot,
    };
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

      for (const result of batch.results) {
        switch (result.type) {
          case "chunk_light_ready": {
            const accepted = this.acceptInitialLightResult(result, pending, chunkViewRevision);
            if (accepted) {
              const chunk = this.level.getChunk(result.chunkX, result.chunkZ, false);
              if (chunk !== null) {
                this.markChunkFullAfterAcceptedLight(chunk.chunkX, chunk.chunkZ);
                await options.onInitialLightReady?.(chunk);
              }
            }
            break;
          }
          case "chunk_light_delta":
            this.acceptLightDeltaResult(result, chunkViewRevision);
            break;
          case "block_light_update_complete":
          case "light_progress":
          case "light_performance":
            break;
          case "light_error":
            this.handleLightError(result, chunkViewRevision);
            break;
        }
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

    this.queueStorageSideEffect(this.flushPendingLightBlockChanges(yieldToEventLoop));
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
    this.queueStorageSideEffect(this.lightingService?.removeChunk({
      type: "remove_light_chunk",
      chunkViewRevision: this.chunkViewJobRevision,
      chunkX,
      chunkZ,
      chunkRevision,
    }));
  }

  private discardUnloadedChunkState(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    this.publishedChunkSnapshots.delete(key);
    this.dirtyChunksForPublication.delete(key);
    this.entityRuntime.updateChunkStatus(chunkX, chunkZ, FullChunkStatus.INACCESSIBLE);
    this.entityRuntime.tick();
    this.discardChunkLighting(chunkX, chunkZ);
  }

  private ensureLocalSessionState(): void {
    if (this.worldOpened === undefined) {
      throw new Error("Local generated world session was accessed before openWorld()");
    }

    this.sessionState ??= {
      sessionId: LOCAL_WORLD_SESSION_ID,
      playerId: LOCAL_WORLD_SESSION_ID,
      saveId: this.worldOpened.saveMetadata.saveId,
      resumed: false,
      revision: 0,
      chunkView: undefined,
    };
    this.playerState ??= createInitialPlayerState(LOCAL_WORLD_SESSION_ID);
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
  }

  private enqueueWorldError(error: unknown): void {
    const message = error instanceof Error ? error.message : String(error);
    this.pendingMessages.push({ type: "world_error", message });
  }

  private drainPendingMessages(maxMessages: number | undefined): readonly WorldHostMessage[] {
    const drained = drainWorldHostMessages(this.pendingMessages, maxMessages);
    this.pendingMessages = drained.remaining;
    return [...drained.messages, this.createWorldPerformanceMessage()];
  }

  private createWorldPerformanceMessage(): WorldPerformanceMessage {
    return {
      type: "world_perf",
      performance: {
        lighting: this.lightingService?.getPerformanceCounters?.(),
      },
    };
  }
}
