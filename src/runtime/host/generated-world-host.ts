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
  type ChunkLightSectionSnapshot,
  type ChunkLightSnapshot,
} from "../../world/level/chunk-snapshot";
import { packChunkSnapshot, unpackChunkSnapshot, type PackedChunkSnapshot, type PackedLightSectionUpdate } from "../../world/level/packed-chunk-snapshot";
import { GeneratedRenderLevel } from "../../world/level/generated-render-level";
import { DataLayer } from "../../world/level/chunk/data-layer";
import type { LevelChunk } from "../../world/level/chunk/level-chunk";
import type { LightChunkGetter } from "../../world/level/chunk/light-chunk-getter";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { BlockStateIdMap } from "../../world/level/block/state/block-state-id";
import type { BlockGetter } from "../../world/level/block-getter";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import { LightLayer } from "../../world/level/light-layer";
import { LevelLightEngine } from "../../world/level/lighting/level-light-engine";
import type { Fluid } from "../../world/level/material/fluid";
import { Fluids } from "../../world/level/material/fluids";
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
  type ChunkLightDeltaMessage,
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
}

const LOCAL_WORLD_SESSION_ID = "local";
const COOPERATIVE_CHUNK_PHASE_BUDGET_MS = 8;
const COOPERATIVE_LIGHT_UPDATE_BUDGET = 262_144;
const LIQUID_TICK_READ_RADIUS_BLOCKS = 4;

interface ChunkViewJobRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  loadedFromStorage: boolean;
}

type StoredChunkPreloadResult = "loaded" | "already_loaded" | "missing";

interface DirtyLightSectionRecord {
  readonly layer: LightLayer;
  readonly section: SectionPos;
}

type GeneratedLevelChunk = LevelChunk;

class GeneratedWorldLightChunkGetter implements LightChunkGetter {
  public constructor(
    private readonly level: GeneratedRenderLevel,
    private readonly onLightUpdateCallback: (layer: LightLayer, section: SectionPos) => void,
  ) {}

  public getChunkForLighting(chunkX: number, chunkZ: number): BlockGetter | null {
    return this.level.getChunk(chunkX, chunkZ, false);
  }

  public onLightUpdate(layer: LightLayer, section: SectionPos): void {
    this.onLightUpdateCallback(layer, section);
  }

  public getLevel(): GeneratedRenderLevel {
    return this.level;
  }
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
  private readonly lightEngine: LevelLightEngine | undefined;
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
  private readonly lightInitializedChunks = new Set<string>();
  private readonly lightCorrectChunks = new Set<string>();
  private readonly dirtyLightSections = new Map<string, DirtyLightSectionRecord>();
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
    this.lightEngine = this.engineConfig.lightingMode === "none"
      ? undefined
      : new LevelLightEngine(
        new GeneratedWorldLightChunkGetter(this.level, (layer, section) => this.markLightSectionDirty(layer, section)),
        true,
        true,
      );
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

    const loadedBefore = new Map(this.level.getLoadedChunks().map((chunk) => [chunkKey(chunk.chunkX, chunk.chunkZ), chunk] as const));
    await this.preloadStoredChunks(request.centerChunkX, request.centerChunkZ, request.radius);
    const changed = this.level.ensureChunksForCamera(
      SectionPos.sectionToBlockCoord(request.centerChunkX) + 8,
      SectionPos.sectionToBlockCoord(request.centerChunkZ) + 8,
      request.radius,
    );
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

    if (!changed) {
      return messages;
    }

    this.options.mutateWorld?.(this.liquidLevel);

    const loadedAfter = this.level.getLoadedChunks();
    const loadedAfterKeys = new Set(loadedAfter.map((chunk) => chunkKey(chunk.chunkX, chunk.chunkZ)));
    this.ensureLightingForLoadedChunks(loadedAfter);
    await this.writeLoadedChunkSnapshotsByPolicy(loadedAfter);

    for (const [key, chunk] of loadedBefore) {
      if (loadedAfterKeys.has(key)) {
        continue;
      }

      await this.saveDirtyChunkBeforeUnload(chunk);
      await this.storageSession?.chunks.evictChunk(chunk.chunkX, chunk.chunkZ);
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

  private setChunkViewCooperative(request: SetChunkViewRequest): readonly WorldHostMessage[] {
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

    for (const chunk of update.unloadedChunks) {
      this.queueStorageSideEffect(this.queueEvictAfterDirtySave(chunk));
      this.discardUnloadedChunkState(chunk.chunkX, chunk.chunkZ);
      messages.push({
        type: "chunk_unload",
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
      });
    }

    const chunkViewJobRevision = chunkViewChanged ? ++this.chunkViewJobRevision : this.chunkViewJobRevision;
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

    const viewRadius = getGeneratedWorldViewChunkRadius(radius);
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

      if (this.isChunkInCurrentView(job.chunkX, job.chunkZ)) {
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
      this.isChunkInCurrentView(job.chunkX, job.chunkZ)
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
        this.isChunkInCurrentView(job.chunkX, job.chunkZ)
        && this.level.getChunk(job.chunkX, job.chunkZ, false) === null
      ) {
        await this.level.generateChunkTerrainCooperative(job.chunkX, job.chunkZ, yieldStep);
      }

      terrainDone++;
      this.enqueueWorldProgress("Generating missing chunks", terrainDone, generationJobs.length, chunkViewJobRevision);
    }

    const decorationJobs = jobs.filter((job) =>
      !job.loadedFromStorage
      && this.isChunkInCurrentView(job.chunkX, job.chunkZ)
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
        this.isChunkInCurrentView(job.chunkX, job.chunkZ)
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
      return this.isChunkInCurrentView(job.chunkX, job.chunkZ)
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

      await this.ensureLightingForLoadedChunksCooperative(lightingChunks, yieldStep);
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
      if (!this.isChunkInCurrentView(job.chunkX, job.chunkZ) || this.publishedChunkSnapshots.has(key)) {
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
      if (!this.isChunkInCurrentView(job.chunkX, job.chunkZ) || this.publishedChunkSnapshots.has(key)) {
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

  private isChunkInCurrentView(chunkX: number, chunkZ: number): boolean {
    if (this.currentChunkView === undefined) {
      return false;
    }

    const viewRadius = getGeneratedWorldViewChunkRadius(this.currentChunkView.radius);
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
      && this.isChunkInCurrentView(chunkX, chunkZ)
      && this.level.getChunk(chunkX, chunkZ, false) !== null
      && this.level.isChunkDecorated(chunkX, chunkZ);
  }

  private markChunkSnapshotPublished(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    this.publishedChunkSnapshots.add(key);
    this.dirtyChunksForPublication.delete(key);
    this.clearDirtyLightSectionsForChunk(chunkX, chunkZ);
  }

  private markHostBlockChanged(pos: BlockPos, oldState: BlockState, newState: BlockState): void {
    const chunkX = SectionPos.blockToSectionCoord(pos.getX());
    const chunkZ = SectionPos.blockToSectionCoord(pos.getZ());
    const key = chunkKey(chunkX, chunkZ);
    this.dirtyDurableChunks.add(key);
    this.dirtyChunksForPublication.add(key);
    this.queueBlockLightUpdate(pos, oldState, newState);
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
    if (yieldStep === undefined) {
      this.runPendingLightingUntilIdle();
    } else {
      await this.runPendingLightingUntilIdleCooperative(yieldStep);
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
      this.clearDirtyLightSectionsForChunk(chunkX, chunkZ);
      if (!this.isChunkInCurrentView(chunkX, chunkZ) || !this.publishedChunkSnapshots.has(key)) {
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
    this.ensureLightingForLoadedChunks([chunk]);
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
    const light = this.buildChunkLightSnapshot(chunk);

    return packChunkSnapshot(
      light === undefined ? snapshot : { ...snapshot, light },
      this.options.blockStateIds,
      this.resolveBlockState,
    );
  }

  private ensureLightingForLoadedChunks(chunks: readonly GeneratedLevelChunk[]): void {
    const lightEngine = this.lightEngine;
    if (lightEngine === undefined) {
      return;
    }

    let queuedWork = false;
    for (const chunk of chunks) {
      const key = chunkKey(chunk.chunkX, chunk.chunkZ);
      if (this.lightInitializedChunks.has(key)) {
        continue;
      }

      this.initializeChunkLighting(lightEngine, chunk);
      this.lightInitializedChunks.add(key);
      queuedWork = true;
    }

    if (queuedWork || lightEngine.hasLightWork()) {
      this.runLightingUntilIdle(lightEngine);
      for (const chunk of chunks) {
        this.lightCorrectChunks.add(chunkKey(chunk.chunkX, chunk.chunkZ));
      }
    }
  }

  private async ensureLightingForLoadedChunksCooperative(
    chunks: readonly GeneratedLevelChunk[],
    yieldStep: () => Promise<void>,
  ): Promise<void> {
    const lightEngine = this.lightEngine;
    if (lightEngine === undefined) {
      return;
    }

    let queuedWork = false;
    for (const chunk of chunks) {
      await yieldStep();
      const key = chunkKey(chunk.chunkX, chunk.chunkZ);
      if (this.lightInitializedChunks.has(key)) {
        continue;
      }

      this.initializeChunkLighting(lightEngine, chunk);
      this.lightInitializedChunks.add(key);
      queuedWork = true;
    }

    if (queuedWork || lightEngine.hasLightWork()) {
      await this.runLightingUntilIdleCooperative(lightEngine, yieldStep);
      for (const chunk of chunks) {
        this.lightCorrectChunks.add(chunkKey(chunk.chunkX, chunk.chunkZ));
      }
    }
  }

  private queueBlockLightUpdate(pos: BlockPos, oldState: BlockState, newState: BlockState): void {
    const lightEngine = this.lightEngine;
    if (lightEngine === undefined) {
      return;
    }

    const section = SectionPos.fromBlockPos(pos);
    const chunk = this.level.getChunk(section.x(), section.z(), false);
    if (chunk !== null) {
      const sectionMinY = SectionPos.sectionToBlockCoord(section.y());
      lightEngine.updateSectionStatus(section, chunk.isYSpaceEmpty(sectionMinY, sectionMinY + 15));
    }

    lightEngine.checkBlock(pos);
    const newEmission = newState.getLightEmission();
    if (newEmission > oldState.getLightEmission()) {
      lightEngine.onBlockEmissionIncrease(pos, newEmission);
    }
    this.lightCorrectChunks.delete(chunkKey(section.x(), section.z()));
  }

  private markLightSectionDirty(layer: LightLayer, section: SectionPos): void {
    this.dirtyLightSections.set(
      `${layer},${section.x()},${section.y()},${section.z()}`,
      { layer, section },
    );
  }

  private clearDirtyLightSectionsForChunk(chunkX: number, chunkZ: number): void {
    for (const [key, record] of this.dirtyLightSections) {
      if (record.section.x() === chunkX && record.section.z() === chunkZ) {
        this.dirtyLightSections.delete(key);
      }
    }
  }

  private runPendingLightingUntilIdle(): void {
    const lightEngine = this.lightEngine;
    if (lightEngine !== undefined && lightEngine.hasLightWork()) {
      this.runLightingUntilIdle(lightEngine);
    }
  }

  private async runPendingLightingUntilIdleCooperative(yieldStep: () => Promise<void>): Promise<void> {
    const lightEngine = this.lightEngine;
    if (lightEngine !== undefined && lightEngine.hasLightWork()) {
      await this.runLightingUntilIdleCooperative(lightEngine, yieldStep);
    }
  }

  private enqueueDirtyLightDeltas(): void {
    this.pendingMessages.push(...this.drainDirtyLightDeltas());
  }

  private drainDirtyLightDeltas(): ChunkLightDeltaMessage[] {
    const lightEngine = this.lightEngine;
    if (lightEngine === undefined || this.dirtyLightSections.size === 0) {
      return [];
    }

    const grouped = new Map<string, {
      readonly chunkX: number;
      readonly chunkZ: number;
      readonly sky: PackedLightSectionUpdate[];
      readonly block: PackedLightSectionUpdate[];
    }>();
    const dirty = [...this.dirtyLightSections.values()];
    this.dirtyLightSections.clear();

    for (const record of dirty) {
      const chunkX = record.section.x();
      const chunkZ = record.section.z();
      const key = chunkKey(chunkX, chunkZ);
      if (!this.publishedChunkSnapshots.has(key) || !this.isChunkInCurrentView(chunkX, chunkZ)) {
        continue;
      }

      const dataLayer = lightEngine.getLayerListener(record.layer).getDataLayerData(record.section);
      if (dataLayer === undefined) {
        continue;
      }

      let group = grouped.get(key);
      if (group === undefined) {
        group = { chunkX, chunkZ, sky: [], block: [] };
        grouped.set(key, group);
      }

      const update: PackedLightSectionUpdate = dataLayer.isEmpty()
        ? { y: record.section.y() }
        : { y: record.section.y(), data: this.copyDataLayerBytes(dataLayer) };
      if (record.layer === LightLayer.SKY) {
        group.sky.push(update);
      } else {
        group.block.push(update);
      }
    }

    return [...grouped.values()]
      .sort((left, right) => left.chunkZ - right.chunkZ || left.chunkX - right.chunkX)
      .map((group) => ({
        type: "chunk_light_delta",
        chunkX: group.chunkX,
        chunkZ: group.chunkZ,
        light: {
          sky: group.sky.length === 0 ? undefined : group.sky.sort((left, right) => left.y - right.y),
          block: group.block.length === 0 ? undefined : group.block.sort((left, right) => left.y - right.y),
        },
      }));
  }

  private initializeChunkLighting(
    lightEngine: LevelLightEngine,
    chunk: GeneratedLevelChunk,
  ): void {
    const nonEmptySections = new Set<number>();
    const lightEmitters: Array<{ readonly pos: BlockPos; readonly lightEmission: number }> = [];
    for (const entry of chunk.getBlockEntries()) {
      nonEmptySections.add(SectionPos.blockToSectionCoord(entry.pos.getY()));
      const lightEmission = entry.state.getLightEmission();
      if (lightEmission > 0) {
        lightEmitters.push({ pos: entry.pos, lightEmission });
      }
    }

    const minSection = this.level.getMinSection();
    const sectionCount = this.level.getSectionsCount();
    for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
      const sectionY = minSection + sectionOffset;
      lightEngine.updateSectionStatus(
        SectionPos.of(chunk.chunkX, sectionY, chunk.chunkZ),
        !nonEmptySections.has(sectionY),
      );
    }

    lightEngine.enableLightSources({ x: chunk.chunkX, z: chunk.chunkZ }, true);
    for (const emitter of lightEmitters) {
      lightEngine.onBlockEmissionIncrease(emitter.pos, emitter.lightEmission);
    }
  }

  private discardChunkLighting(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    this.lightInitializedChunks.delete(key);
    this.lightCorrectChunks.delete(key);
    this.clearDirtyLightSectionsForChunk(chunkX, chunkZ);
    const lightEngine = this.lightEngine;
    if (lightEngine === undefined) {
      return;
    }

    lightEngine.retainData({ x: chunkX, z: chunkZ }, false);
    lightEngine.enableLightSources({ x: chunkX, z: chunkZ }, false);

    for (let sectionY = lightEngine.getMinLightSection(); sectionY < lightEngine.getMaxLightSection(); sectionY++) {
      const section = SectionPos.of(chunkX, sectionY, chunkZ);
      lightEngine.queueSectionData(LightLayer.BLOCK, section, undefined, true);
      lightEngine.queueSectionData(LightLayer.SKY, section, undefined, true);
    }

    const minSection = this.level.getMinSection();
    const sectionCount = this.level.getSectionsCount();
    for (let sectionOffset = 0; sectionOffset < sectionCount; sectionOffset++) {
      lightEngine.updateSectionStatus(SectionPos.of(chunkX, minSection + sectionOffset, chunkZ), true);
    }
  }

  private discardUnloadedChunkState(chunkX: number, chunkZ: number): void {
    const key = chunkKey(chunkX, chunkZ);
    this.publishedChunkSnapshots.delete(key);
    this.dirtyChunksForPublication.delete(key);
    this.discardChunkLighting(chunkX, chunkZ);
  }

  private runLightingUntilIdle(lightEngine: LevelLightEngine): void {
    for (let iteration = 0; iteration < 100; iteration++) {
      if (!lightEngine.hasLightWork()) {
        return;
      }

      lightEngine.runUpdates(1_000_000, true, true);
    }

    throw new Error("Generated world light engine did not become idle");
  }

  private async runLightingUntilIdleCooperative(
    lightEngine: LevelLightEngine,
    yieldStep: () => Promise<void>,
  ): Promise<void> {
    for (let iteration = 0; iteration < 10_000; iteration++) {
      if (!lightEngine.hasLightWork()) {
        return;
      }

      lightEngine.runUpdates(COOPERATIVE_LIGHT_UPDATE_BUDGET, true, true);
      await yieldStep();
    }

    throw new Error("Generated world light engine did not become idle");
  }

  private buildChunkLightSnapshot(chunk: GeneratedLevelChunk): ChunkLightSnapshot | undefined {
    const lightEngine = this.lightEngine;
    if (lightEngine === undefined) {
      return undefined;
    }

    return {
      sky: this.collectChunkLightSections(lightEngine, LightLayer.SKY, chunk),
      block: this.collectChunkLightSections(lightEngine, LightLayer.BLOCK, chunk),
      lightCorrect: this.lightCorrectChunks.has(chunkKey(chunk.chunkX, chunk.chunkZ)),
    };
  }

  private collectChunkLightSections(
    lightEngine: LevelLightEngine,
    layer: LightLayer,
    chunk: GeneratedLevelChunk,
  ): ChunkLightSectionSnapshot[] {
    const listener = lightEngine.getLayerListener(layer);
    const sections: ChunkLightSectionSnapshot[] = [];
    for (let sectionY = lightEngine.getMinLightSection(); sectionY < lightEngine.getMaxLightSection(); sectionY++) {
      const dataLayer = listener.getDataLayerData(SectionPos.of(chunk.chunkX, sectionY, chunk.chunkZ));
      if (dataLayer === undefined) {
        continue;
      }

      sections.push({
        y: sectionY,
        data: this.copyDataLayerBytes(dataLayer),
      });
    }

    return sections;
  }

  private copyDataLayerBytes(dataLayer: DataLayer): Uint8Array {
    return new Uint8Array(dataLayer.getData());
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
