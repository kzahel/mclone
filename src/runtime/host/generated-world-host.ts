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
import { packChunkSnapshot, unpackChunkSnapshot, type PackedChunkLight, type PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";
import { FEATURES_CHUNK_DEPENDENCY_RADIUS } from "../../world/level/generated-decoration-region";
import { GeneratedRenderLevel } from "../../world/level/generated-render-level";
import type { LevelChunk } from "../../world/level/chunk/level-chunk";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { BlockStateIdMap } from "../../world/level/block/state/block-state-id";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { Fluid } from "../../world/level/material/fluid";
import { Fluids } from "../../world/level/material/fluids";
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
  type WorldProgressMessage,
} from "../protocol/world-messages";
import {
  anchorPlayerStateToChunkView,
  createInitialPlayerState,
  PLAYER_TICK_INTERVAL_MS,
  tickPlayerState,
} from "../session/player-loop";
import {
  createWorldSaveMetadata,
  type OpenWorldStorageRequest,
  type WorldStorage,
  type WorldStorageSession,
} from "../storage/world-storage";
import { LiquidSimulationLevel } from "./liquid-simulation-level";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

// Bump when persisted generated-world meaning changes: terrain/decor algorithms,
// biome/block-state/tick encodings, metadata compatibility, or future trusted light.
export const GENERATED_WORLD_STORAGE_VERSION = 4;

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

function getGeneratedWorldAuthorityChunkRadius(radius: number): number {
  return getGeneratedWorldViewChunkRadius(radius) + FEATURES_CHUNK_DEPENDENCY_RADIUS;
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

type StoredChunkPreloadResult = "loaded" | "already_loaded" | "missing";

type GeneratedLevelChunk = LevelChunk;

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
  private readonly engineConfig: NormalizedWorldEngineConfig;
  private readonly liquidSimulationEnabled: boolean;
  private readonly nowMs: () => number;
  private readonly resolveBlockState;
  private storageSession: WorldStorageSession | undefined;
  private worldOpened: WorldOpenedMessage | undefined;
  private sessionState: ClientSessionState | undefined;
  private playerState: ClientPlayerState | undefined;
  private playerInput: SetPlayerInputRequest["input"] | undefined;
  private currentChunkView: SetChunkViewRequest | undefined;
  private pendingMessages: WorldHostMessage[] = [];
  private gameTime = 0;
  private lastWorldTickAtMs: number;
  private lastPlayerTickAtMs = Date.now();
  private playerAnchoredToChunkView = false;
  private readonly publishedChunkSnapshots = new Set<string>();
  private readonly chunkRevisions = new Map<string, number>();
  private readonly sentLightInputRevisions = new Map<string, number>();
  private readonly acceptedChunkLight = new Map<string, PackedChunkLight>();
  private readonly dirtyChunksForPublication = new Set<string>();
  private readonly dirtyDurableChunks = new Set<string>();
  private opened = false;
  private chunkViewJobRevision = 0;

  public constructor(private readonly options: GeneratedWorldHostOptions) {
    this.engineConfig = normalizeWorldEngineConfig({
      lightingMode: options.lightingMode,
      liquidSimulationMode: options.liquidSimulationMode,
    });
    this.nowMs = options.nowMs ?? (() => Date.now());
    this.lastWorldTickAtMs = this.nowMs();
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

    await this.preloadStoredChunks(request.centerChunkX, request.centerChunkZ, request.radius);
    const update = this.level.updateChunkView(request.centerChunkX, request.centerChunkZ, request.radius);
    for (const [chunkX, chunkZ] of update.missingChunks) {
      this.level.getChunk(chunkX, chunkZ, true);
    }
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

    this.options.mutateWorld?.(this.liquidLevel);

    const loadedAfter = this.level.getLoadedChunks();
    await this.ensureLightingForLoadedChunks(loadedAfter, this.chunkViewJobRevision);
    await this.writeLoadedChunkSnapshotsByPolicy(loadedAfter);

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

    for (const chunk of loadedAfter) {
      messages.push({
        type: "chunk_snapshot",
        snapshot: this.buildPackedChunkSnapshot(chunk),
      });
      this.markChunkSnapshotPublished(chunk.chunkX, chunk.chunkZ);
    }

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
      return [...jobs.values()];
    }

    const viewRadius = getGeneratedWorldViewChunkRadius(this.currentChunkView.radius);
    for (let chunkZ = this.currentChunkView.centerChunkZ - viewRadius; chunkZ <= this.currentChunkView.centerChunkZ + viewRadius; chunkZ++) {
      for (let chunkX = this.currentChunkView.centerChunkX - viewRadius; chunkX <= this.currentChunkView.centerChunkX + viewRadius; chunkX++) {
        const key = chunkKey(chunkX, chunkZ);
        if (this.publishedChunkSnapshots.has(key)) {
          continue;
        }
        if (this.level.getChunk(chunkX, chunkZ, false) === null) {
          continue;
        }

        jobs.set(key, [chunkX, chunkZ]);
      }
    }

    return [...jobs.values()];
  }

  public async setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    if (!this.opened) {
      return [{ type: "world_error", message: "set_player_input received before open_world" }];
    }

    this.ensureLocalSessionState();
    this.tickPlayerLoop();
    await this.tickWorldLoop();
    this.playerInput = request.input;
    this.updateSessionState();
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

  private async preloadStoredChunks(centerChunkX: number, centerChunkZ: number, radius: number): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    const viewRadius = getGeneratedWorldAuthorityChunkRadius(radius);
    for (let chunkZ = centerChunkZ - viewRadius; chunkZ <= centerChunkZ + viewRadius; chunkZ++) {
      for (let chunkX = centerChunkX - viewRadius; chunkX <= centerChunkX + viewRadius; chunkX++) {
        if (this.level.getChunk(chunkX, chunkZ, false) !== null) {
          continue;
        }

        const snapshot = await this.storageSession.chunks.loadChunk(chunkX, chunkZ);
        if (snapshot === undefined) {
          continue;
        }

        this.level.setChunk(
          hydrateChunkFromSnapshot(
            unpackChunkSnapshot(snapshot, this.options.blockStateIds),
            this.options.airState,
            this.resolveBlockState,
          ),
          true,
        );
      }
    }
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

  private async writeLoadedChunkSnapshotsByPolicy(chunks: readonly GeneratedLevelChunk[]): Promise<void> {
    await this.ensureLightingForLoadedChunks(chunks, this.chunkViewJobRevision);
    for (const chunk of chunks) {
      await this.writeChunkSnapshotByPolicy(this.buildPackedChunkSnapshot(chunk));
    }
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
      && this.level.getChunk(job.chunkX, job.chunkZ, false) === null
    );
    let terrainDone = 0;
    this.enqueueWorldProgress("Generating missing chunks", terrainDone, generationJobs.length, chunkViewJobRevision);
    for (const job of generationJobs) {
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      if (
        this.isChunkInCurrentAuthorityView(job.chunkX, job.chunkZ)
        && this.level.getChunk(job.chunkX, job.chunkZ, false) === null
      ) {
        await this.level.generateChunkTerrainCooperative(job.chunkX, job.chunkZ, yieldStep);
      }

      terrainDone++;
      this.enqueueWorldProgress("Generating missing chunks", terrainDone, generationJobs.length, chunkViewJobRevision);
    }

    const decorationJobs = jobs.filter((job) =>
      !job.loadedFromStorage
      && this.isChunkInCurrentPublishView(job.chunkX, job.chunkZ)
      && this.level.getChunk(job.chunkX, job.chunkZ, false) !== null
      && !this.level.isChunkDecorated(job.chunkX, job.chunkZ)
    );
    let decorationDone = 0;
    this.enqueueWorldProgress("Decorating new chunks", decorationDone, decorationJobs.length, chunkViewJobRevision);
    for (const job of decorationJobs) {
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      if (
        this.isChunkInCurrentPublishView(job.chunkX, job.chunkZ)
        && this.level.getChunk(job.chunkX, job.chunkZ, false) !== null
        && !this.level.isChunkDecorated(job.chunkX, job.chunkZ)
      ) {
        await this.level.decorateChunkCooperative(job.chunkX, job.chunkZ, yieldStep);
      }

      decorationDone++;
      this.enqueueWorldProgress("Decorating new chunks", decorationDone, decorationJobs.length, chunkViewJobRevision);
    }

    const publishJobs = jobs.filter((job) => {
      const key = chunkKey(job.chunkX, job.chunkZ);
      return this.isChunkInCurrentPublishView(job.chunkX, job.chunkZ)
        && !this.publishedChunkSnapshots.has(key)
        && this.level.getChunk(job.chunkX, job.chunkZ, false) !== null
        && this.level.isChunkDecorated(job.chunkX, job.chunkZ);
    });
    const lightingChunks = publishJobs
      .map((job) => this.level.getChunk(job.chunkX, job.chunkZ, false))
      .filter((chunk): chunk is GeneratedLevelChunk => chunk !== null);
    this.enqueueWorldProgress("Computing light", 0, lightingChunks.length, chunkViewJobRevision);
    if (lightingChunks.length > 0) {
      this.options.mutateWorld?.(this.liquidLevel);
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      await this.ensureLightingForLoadedChunksCooperative(lightingChunks, chunkViewJobRevision, yieldStep);
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }
    }
    this.enqueueWorldProgress("Computing light", lightingChunks.length, lightingChunks.length, chunkViewJobRevision);
    this.enqueueDirtyLightDeltas();

    let publishDone = 0;
    this.enqueueWorldProgress("Publishing chunks", publishDone, publishJobs.length, chunkViewJobRevision);
    for (const job of publishJobs) {
      await yieldStep();
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }

      const key = chunkKey(job.chunkX, job.chunkZ);
      if (!this.isChunkInCurrentPublishView(job.chunkX, job.chunkZ) || this.publishedChunkSnapshots.has(key)) {
        publishDone++;
        this.enqueueWorldProgress("Publishing chunks", publishDone, publishJobs.length, chunkViewJobRevision);
        continue;
      }

      const chunk = this.level.getChunk(job.chunkX, job.chunkZ, false);
      if (chunk === null || !this.level.isChunkDecorated(job.chunkX, job.chunkZ)) {
        publishDone++;
        this.enqueueWorldProgress("Publishing chunks", publishDone, publishJobs.length, chunkViewJobRevision);
        continue;
      }

      await yieldStep();
      const snapshot = this.buildPackedChunkSnapshot(chunk);
      if (!this.isCurrentChunkViewJob(chunkViewJobRevision)) {
        return;
      }
      if (!this.isChunkInCurrentPublishView(job.chunkX, job.chunkZ) || this.publishedChunkSnapshots.has(key)) {
        publishDone++;
        this.enqueueWorldProgress("Publishing chunks", publishDone, publishJobs.length, chunkViewJobRevision);
        continue;
      }

      this.pendingMessages.push({
        type: "chunk_snapshot",
        snapshot,
      });
      this.markChunkSnapshotPublished(job.chunkX, job.chunkZ);
      this.queueStorageSideEffect(this.writeChunkSnapshotByPolicy(snapshot));
      this.enqueueDirtyLightDeltas();
      publishDone++;
      this.enqueueWorldProgress("Publishing chunks", publishDone, publishJobs.length, chunkViewJobRevision);
    }
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

  private isChunkInCurrentAuthorityView(chunkX: number, chunkZ: number): boolean {
    if (this.currentChunkView === undefined) {
      return false;
    }

    const viewRadius = getGeneratedWorldAuthorityChunkRadius(this.currentChunkView.radius);
    return Math.abs(chunkX - this.currentChunkView.centerChunkX) <= viewRadius
      && Math.abs(chunkZ - this.currentChunkView.centerChunkZ) <= viewRadius;
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
      && this.level.isChunkDecorated(chunkX, chunkZ);
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
    this.sentLightInputRevisions.delete(key);
    for (let neighborChunkZ = chunkZ - 1; neighborChunkZ <= chunkZ + 1; neighborChunkZ++) {
      for (let neighborChunkX = chunkX - 1; neighborChunkX <= chunkX + 1; neighborChunkX++) {
        this.acceptedChunkLight.delete(chunkKey(neighborChunkX, neighborChunkZ));
      }
    }
    return revision;
  }

  private markHostBlockChanged(pos: BlockPos, _oldState: BlockState, _newState: BlockState): void {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    const key = chunkKey(chunkX, chunkZ);
    this.dirtyDurableChunks.add(key);
    this.dirtyChunksForPublication.add(key);
    this.incrementChunkRevision(chunkX, chunkZ);
    this.acceptedChunkLight.delete(key);
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
    if (!this.liquidSimulationEnabled || this.publishedChunkSnapshots.size === 0) {
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
    this.dirtyChunksForPublication.clear();
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
        continue;
      }

      const snapshot = this.buildPackedChunkSnapshot(chunk);
      await this.writeChunkSnapshotByPolicy(snapshot);
      if (!this.isChunkInCurrentPublishView(chunkX, chunkZ) || !this.publishedChunkSnapshots.has(key)) {
        continue;
      }

      this.pendingMessages.push({
        type: "chunk_snapshot",
        snapshot,
      });
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
    const light = this.acceptedChunkLight.get(chunkKey(chunk.chunkX, chunk.chunkZ));

    return packChunkSnapshot(
      light === undefined ? snapshot : { ...snapshot, light },
      this.options.blockStateIds,
      this.resolveBlockState,
    );
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
  ): Promise<void> {
    const lightingService = this.lightingService;
    if (lightingService === undefined || chunks.length === 0) {
      return;
    }

    const dependencyChunks = this.collectLightingDependencyChunks(chunks);
    for (const chunk of dependencyChunks) {
      await yieldStep();
      await this.upsertLightInputChunk(chunk, chunkViewRevision);
    }

    const pending = new Map<string, number>();
    for (const chunk of chunks) {
      const key = chunkKey(chunk.chunkX, chunk.chunkZ);
      const chunkRevision = this.getChunkRevision(chunk.chunkX, chunk.chunkZ);
      const accepted = this.acceptedChunkLight.get(key);
      if (accepted !== undefined && accepted.lightCorrect && this.sentLightInputRevisions.get(key) === chunkRevision) {
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
      const batch = await lightingService.pollResults({
        type: "poll_light_results",
        maxResults: LIGHTING_RESULT_BATCH_SIZE,
      });
      for (const result of batch.results) {
        switch (result.type) {
          case "chunk_light_ready":
            this.acceptInitialLightResult(result, pending, chunkViewRevision);
            break;
          case "chunk_light_delta":
            this.acceptLightDeltaResult(result, chunkViewRevision);
            break;
          case "light_error":
            this.handleLightError(result, chunkViewRevision);
            break;
          case "light_progress":
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
      for (let neighborChunkZ = chunk.chunkZ - 1; neighborChunkZ <= chunk.chunkZ + 1; neighborChunkZ++) {
        for (let neighborChunkX = chunk.chunkX - 1; neighborChunkX <= chunk.chunkX + 1; neighborChunkX++) {
          const neighbor = this.level.getChunk(neighborChunkX, neighborChunkZ, false);
          if (neighbor === null) {
            throw new Error(
              `Missing lighting neighbor chunk (${neighborChunkX.toString()}, ${neighborChunkZ.toString()}) for (${chunk.chunkX.toString()}, ${chunk.chunkZ.toString()})`,
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
      decorated: this.level.isChunkDecorated(chunk.chunkX, chunk.chunkZ),
      sections: this.buildLightInputSections(chunk),
    });
    this.sentLightInputRevisions.set(key, chunkRevision);
    this.acceptedChunkLight.delete(key);
  }

  private buildLightInputSections(chunk: GeneratedLevelChunk): readonly { readonly y: number; readonly blockStateIds: Uint32Array }[] {
    const sections: Array<{ readonly y: number; readonly blockStateIds: Uint32Array }> = [];
    const pos = new BlockPos.MutableBlockPos();
    const chunkBlockX = SectionPos.sectionToBlockCoord(chunk.chunkX);
    const chunkBlockZ = SectionPos.sectionToBlockCoord(chunk.chunkZ);
    const minSection = this.level.getMinSection();
    const sectionCount = this.level.getSectionsCount();
    for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
      const sectionY = minSection + sectionOffset;
      const sectionMinY = SectionPos.sectionToBlockCoord(sectionY);
      const blockStateIds = new Uint32Array(16 * 16 * 16);
      let index = 0;
      for (let localY = 0; localY < 16; localY++) {
        for (let localZ = 0; localZ < 16; localZ++) {
          for (let localX = 0; localX < 16; localX++) {
            pos.set(chunkBlockX + localX, sectionMinY + localY, chunkBlockZ + localZ);
            blockStateIds[index++] = this.options.blockStateIds.idFor(chunk.getBlockState(pos));
          }
        }
      }

      sections.push({ y: sectionY, blockStateIds });
    }

    return sections;
  }

  private acceptInitialLightResult(
    result: ChunkLightReadyResult,
    pending: Map<string, number>,
    currentChunkViewRevision: number,
  ): void {
    const key = chunkKey(result.chunkX, result.chunkZ);
    if (
      result.chunkViewRevision !== currentChunkViewRevision
      || pending.get(key) !== result.chunkRevision
      || this.getChunkRevision(result.chunkX, result.chunkZ) !== result.chunkRevision
    ) {
      return;
    }

    this.acceptedChunkLight.set(key, result.light);
    pending.delete(key);
  }

  private acceptLightDeltaResult(result: ChunkLightDeltaResult, currentChunkViewRevision: number): void {
    if (
      result.chunkViewRevision !== currentChunkViewRevision
      || this.getChunkRevision(result.chunkX, result.chunkZ) !== result.chunkRevision
    ) {
      return;
    }

    this.pendingMessages.push({
      type: "chunk_light_delta",
      chunkX: result.chunkX,
      chunkZ: result.chunkZ,
      light: result.light,
    });
  }

  private handleLightError(result: LightErrorResult, currentChunkViewRevision: number): void {
    if (result.chunkViewRevision !== undefined && result.chunkViewRevision !== currentChunkViewRevision) {
      return;
    }

    throw new Error(result.message);
  }

  private enqueueDirtyLightDeltas(): void {}

  private discardChunkLighting(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    const chunkRevision = this.getChunkRevision(chunkX, chunkZ);
    this.sentLightInputRevisions.delete(key);
    this.acceptedChunkLight.delete(key);
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

  private tickPlayerLoop(nowMs = Date.now()): void {
    if (this.playerState === undefined) {
      return;
    }

    const dueTicks = Math.floor((nowMs - this.lastPlayerTickAtMs) / PLAYER_TICK_INTERVAL_MS);
    if (dueTicks <= 0) {
      return;
    }

    for (let tickIndex = 0; tickIndex < dueTicks; tickIndex++) {
      const nextPlayerState = tickPlayerState(
        this.playerState,
        this.playerInput,
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
    return drained.messages;
  }
}
