import { SectionPos } from "../../core/section-pos";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import { ChunkBiomeContainer } from "../../worldgen/biome/chunk-biome-container";
import { NoiseBasedChunkGenerator } from "../../worldgen/levelgen/noise-based-chunk-generator";
import { buildChunkSnapshot, createBlockStateResolver, hydrateChunkFromSnapshot } from "../../world/level/chunk-snapshot";
import { GeneratedRenderLevel } from "../../world/level/generated-render-level";
import type { BlockState } from "../../world/level/block/state/block-state";
import type { WorldGenLevel } from "../../world/level/world-gen-level";
import type { WorldHost } from "../protocol/world-host";
import type { OpenWorldPreset, OpenWorldRequest, SetChunkViewRequest, WorldHostMessage, WorldOpenedMessage } from "../protocol/world-messages";
import {
  createWorldSaveMetadata,
  type OpenWorldStorageRequest,
  type WorldStorage,
  type WorldStorageSession,
} from "../storage/world-storage";

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

export const GENERATED_WORLD_STORAGE_VERSION = 1;

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
  readonly mutateWorld?: (level: WorldGenLevel) => void;
  readonly worldStorage?: WorldStorage;
}

export class GeneratedWorldHost implements WorldHost {
  private readonly biomeSource;
  private readonly generator;
  private readonly level;
  private readonly resolveBlockState;
  private storageSession: WorldStorageSession | undefined;
  private worldOpened: WorldOpenedMessage | undefined;
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
    return [this.worldOpened];
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    if (!this.opened) {
      return [{ type: "world_error", message: "set_chunk_view received before open_world" }];
    }

    const loadedBefore = new Map(this.level.getLoadedChunks().map((chunk) => [chunkKey(chunk.chunkX, chunk.chunkZ), chunk] as const));
    await this.preloadStoredChunks(request.centerChunkX, request.centerChunkZ, request.radius);
    const changed = this.level.ensureChunksForCamera(
      SectionPos.sectionToBlockCoord(request.centerChunkX) + 8,
      SectionPos.sectionToBlockCoord(request.centerChunkZ) + 8,
      request.radius,
    );
    if (!changed) {
      return [];
    }

    this.options.mutateWorld?.(this.level);

    const loadedAfter = this.level.getLoadedChunks();
    const loadedAfterKeys = new Set(loadedAfter.map((chunk) => chunkKey(chunk.chunkX, chunk.chunkZ)));
    await this.persistLoadedChunks(loadedAfter);
    const messages: WorldHostMessage[] = [];

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
        snapshot: buildChunkSnapshot(
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
      });
    }

    return messages;
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

        this.level.setChunk(hydrateChunkFromSnapshot(snapshot, this.options.airState, this.resolveBlockState));
      }
    }
  }

  private async persistLoadedChunks(chunks: readonly ReturnType<GeneratedRenderLevel["getLoadedChunks"]>[number][]): Promise<void> {
    if (this.storageSession === undefined) {
      return;
    }

    for (const chunk of chunks) {
      await this.storageSession.chunks.saveChunk(
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
      );
    }
  }
}
