import type { PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";
import type { GeneratedChunkStorageRecord } from "../../world/level/generated-proto-chunk";
import type { OpenWorldPreset } from "../protocol/world-messages";

export interface WorldSaveMetadata {
  readonly saveId: string;
  readonly storageVersion: number;
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly minBuildHeight: number;
  readonly height: number;
  readonly createdAtMs: number;
  readonly lastOpenedAtMs: number;
}

export interface OpenWorldStorageRequest {
  readonly saveId: string;
  readonly storageVersion: number;
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly minBuildHeight: number;
  readonly height: number;
  readonly openedAtMs: number;
}

export interface SaveChunkOptions {
  readonly generatedRecordWriteVersion?: number;
}

export interface ChunkStorage {
  loadChunk(chunkX: number, chunkZ: number): Promise<PackedChunkSnapshot | undefined>;

  saveChunk(snapshot: PackedChunkSnapshot, options?: SaveChunkOptions): Promise<void>;

  loadGeneratedChunk(chunkX: number, chunkZ: number): Promise<GeneratedChunkStorageRecord | undefined>;

  saveGeneratedChunk(record: GeneratedChunkStorageRecord): Promise<void>;

  evictChunk(chunkX: number, chunkZ: number): Promise<void>;
}

export interface WorldStorageSession {
  readonly metadata: WorldSaveMetadata;
  readonly chunks: ChunkStorage;

  close(): Promise<void>;
}

export interface WorldStorage {
  openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession>;
}

export function createWorldSaveMetadata(request: OpenWorldStorageRequest): WorldSaveMetadata {
  return {
    saveId: request.saveId,
    storageVersion: request.storageVersion,
    seed: request.seed,
    preset: request.preset,
    minBuildHeight: request.minBuildHeight,
    height: request.height,
    createdAtMs: request.openedAtMs,
    lastOpenedAtMs: request.openedAtMs,
  };
}

export function isWorldSaveMetadataCompatible(metadata: WorldSaveMetadata, request: OpenWorldStorageRequest): boolean {
  return metadata.storageVersion === request.storageVersion
    && metadata.seed === request.seed
    && metadata.preset === request.preset
    && metadata.minBuildHeight === request.minBuildHeight
    && metadata.height === request.height;
}

export function touchWorldSaveMetadata(metadata: WorldSaveMetadata, openedAtMs: number): WorldSaveMetadata {
  return {
    ...metadata,
    lastOpenedAtMs: openedAtMs,
  };
}
