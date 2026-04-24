import { SectionPos } from "../../core/section-pos";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import { ChunkBiomeContainer } from "../../worldgen/biome/chunk-biome-container";
import { NoiseBasedChunkGenerator } from "../../worldgen/levelgen/noise-based-chunk-generator";
import { buildChunkSnapshot, createBlockStateResolver, hydrateChunkFromSnapshot } from "../../world/level/chunk-snapshot";
import { packChunkSnapshot, unpackChunkSnapshot, type PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";
import { GeneratedRenderLevel } from "../../world/level/generated-render-level";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { BlockStateIdMap } from "../../world/level/block/state/block-state-id";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { WorldHost } from "../protocol/world-host";
import { drainWorldHostMessages } from "../protocol/world-message-queue";
import type {
  ClientPlayerState,
  ClientSessionState,
  OpenWorldPreset,
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  SessionChunkViewState,
  WorldHostMessage,
  WorldOpenedMessage,
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

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

// Bump when generated chunk content changes in a way that makes older cached snapshots misleading.
export const GENERATED_WORLD_STORAGE_VERSION = 3;

export function createGeneratedWorldSaveId(seed: bigint, preset: OpenWorldPreset): string {
  return `generated-world-v${GENERATED_WORLD_STORAGE_VERSION.toString()}-${preset}-${seed.toString()}`;
}

function createStorageOpenRequest(
  request: OpenWorldRequest,
  minBuildHeight: number,
  height: number,
  openedAtMs: number,
): OpenWorldStorageRequest {
  return {
    saveId: createGeneratedWorldSaveId(request.seed, request.preset),
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

export interface GeneratedWorldHostOptions {
  readonly seed: bigint;
  readonly airState: BlockState;
  readonly blockStateById: readonly BlockState[];
  readonly blockStateIds: BlockStateIdMap;
  readonly chunkViewScheduling?: "synchronous" | "cooperative";
  readonly mutateWorld?: (level: WorldGenLevel) => void;
  readonly worldStorage?: WorldStorage;
}

const LOCAL_WORLD_SESSION_ID = "local";
const COOPERATIVE_CHUNK_PHASE_BUDGET_MS = 8;

interface ChunkViewJobRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  loadedFromStorage: boolean;
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
  private readonly resolveBlockState;
  private storageSession: WorldStorageSession | undefined;
  private worldOpened: WorldOpenedMessage | undefined;
  private sessionState: ClientSessionState | undefined;
  private playerState: ClientPlayerState | undefined;
  private playerInput: SetPlayerInputRequest["input"] | undefined;
  private currentChunkView: SetChunkViewRequest | undefined;
  private pendingMessages: WorldHostMessage[] = [];
  private lastPlayerTickAtMs = Date.now();
  private playerAnchoredToChunkView = false;
  private readonly publishedChunkSnapshots = new Set<string>();
  private opened = false;

  public constructor(private readonly options: GeneratedWorldHostOptions) {
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

    this.options.mutateWorld?.(this.level);

    const loadedAfter = this.level.getLoadedChunks();
    const loadedAfterKeys = new Set(loadedAfter.map((chunk) => chunkKey(chunk.chunkX, chunk.chunkZ)));
    await this.persistLoadedChunks(loadedAfter);

    for (const [key, chunk] of loadedBefore) {
      if (loadedAfterKeys.has(key)) {
        continue;
      }

      await this.storageSession?.chunks.evictChunk(chunk.chunkX, chunk.chunkZ);
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
      this.queueStorageSideEffect(this.storageSession?.chunks.evictChunk(chunk.chunkX, chunk.chunkZ));
      this.publishedChunkSnapshots.delete(chunkKey(chunk.chunkX, chunk.chunkZ));
      messages.push({
        type: "chunk_unload",
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
      });
    }

    if (!update.changed) {
      return messages;
    }

    const chunkViewJob = this.runChunkViewJobs(this.collectChunkViewJobs(update.missingChunks));
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

  private async preloadStoredChunk(chunkX: number, chunkZ: number): Promise<boolean> {
    if (this.storageSession === undefined || this.level.getChunk(chunkX, chunkZ, false) !== null) {
      return false;
    }

    const snapshot = await this.storageSession.chunks.loadChunk(chunkX, chunkZ);
    if (snapshot === undefined) {
      return false;
    }

    this.level.setChunk(
      hydrateChunkFromSnapshot(
        unpackChunkSnapshot(snapshot, this.options.blockStateIds),
        this.options.airState,
        this.resolveBlockState,
      ),
      true,
    );
    return true;
  }

  private async persistLoadedChunks(chunks: readonly ReturnType<GeneratedRenderLevel["getLoadedChunks"]>[number][]): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    for (const chunk of chunks) {
      await this.storageSession.chunks.saveChunk(this.buildPackedChunkSnapshot(chunk));
    }
  }

  private async persistPackedChunkSnapshot(snapshot: PackedChunkSnapshot): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    await this.storageSession.chunks.saveChunk(snapshot);
  }

  private async runChunkViewJobs(
    missingChunks: readonly (readonly [number, number])[],
  ): Promise<void> {
    const yieldStep = createCooperativeYield();
    const jobs: ChunkViewJobRecord[] = missingChunks.map(([chunkX, chunkZ]) => ({
      chunkX,
      chunkZ,
      loadedFromStorage: false,
    }));

    for (const job of jobs) {
      await yieldStep();
      if (!this.isChunkInCurrentView(job.chunkX, job.chunkZ)) {
        continue;
      }

      job.loadedFromStorage = await this.preloadStoredChunk(job.chunkX, job.chunkZ);
      if (!this.isChunkInCurrentView(job.chunkX, job.chunkZ)) {
        continue;
      }
      if (job.loadedFromStorage || this.level.getChunk(job.chunkX, job.chunkZ, false) !== null) {
        continue;
      }

      await this.level.generateChunkTerrainCooperative(job.chunkX, job.chunkZ, yieldStep);
    }

    for (const job of jobs) {
      await yieldStep();
      if (job.loadedFromStorage || !this.isChunkInCurrentView(job.chunkX, job.chunkZ)) {
        continue;
      }

      await this.level.decorateChunkCooperative(job.chunkX, job.chunkZ, yieldStep);
    }

    for (const job of jobs) {
      await yieldStep();
      const key = chunkKey(job.chunkX, job.chunkZ);
      if (!this.isChunkInCurrentView(job.chunkX, job.chunkZ) || this.publishedChunkSnapshots.has(key)) {
        continue;
      }

      const chunk = this.level.getChunk(job.chunkX, job.chunkZ, false);
      if (chunk === null || !this.level.isChunkDecorated(job.chunkX, job.chunkZ)) {
        continue;
      }

      this.options.mutateWorld?.(this.level);
      await yieldStep();
      const snapshot = this.buildPackedChunkSnapshot(chunk);
      await this.persistPackedChunkSnapshot(snapshot);
      if (!this.isChunkInCurrentView(job.chunkX, job.chunkZ) || this.publishedChunkSnapshots.has(key)) {
        continue;
      }

      this.pendingMessages.push({
        type: "chunk_snapshot",
        snapshot,
      });
      this.publishedChunkSnapshots.add(key);
    }
  }

  private isChunkInCurrentView(chunkX: number, chunkZ: number): boolean {
    if (this.currentChunkView === undefined) {
      return false;
    }

    const viewRadius = getGeneratedWorldViewChunkRadius(this.currentChunkView.radius);
    return Math.abs(chunkX - this.currentChunkView.centerChunkX) <= viewRadius
      && Math.abs(chunkZ - this.currentChunkView.centerChunkZ) <= viewRadius;
  }

  private queueStorageSideEffect(task: Promise<void> | undefined): void {
    if (task === undefined) {
      return;
    }

    task.catch((error: unknown) => {
      this.enqueueWorldError(error);
    });
  }

  private buildPackedChunkSnapshot(chunk: ReturnType<GeneratedRenderLevel["getLoadedChunks"]>[number]): PackedChunkSnapshot {
    return packChunkSnapshot(
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
      this.options.blockStateIds,
      this.resolveBlockState,
    );
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
