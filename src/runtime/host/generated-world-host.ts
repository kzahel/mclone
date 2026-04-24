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
  readonly mutateWorld?: (level: WorldGenLevel) => void;
  readonly worldStorage?: WorldStorage;
}

const LOCAL_WORLD_SESSION_ID = "local";

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

        this.level.setChunk(hydrateChunkFromSnapshot(
          unpackChunkSnapshot(snapshot, this.options.blockStateIds),
          this.options.airState,
          this.resolveBlockState,
        ));
      }
    }
  }

  private async persistLoadedChunks(chunks: readonly ReturnType<GeneratedRenderLevel["getLoadedChunks"]>[number][]): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    for (const chunk of chunks) {
      await this.storageSession.chunks.saveChunk(this.buildPackedChunkSnapshot(chunk));
    }
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

  private drainPendingMessages(maxMessages: number | undefined): readonly WorldHostMessage[] {
    if (maxMessages === undefined || maxMessages >= this.pendingMessages.length) {
      const drained = this.pendingMessages;
      this.pendingMessages = [];
      return drained;
    }

    const drained = this.pendingMessages.slice(0, maxMessages);
    this.pendingMessages = this.pendingMessages.slice(maxMessages);
    return drained;
  }
}
