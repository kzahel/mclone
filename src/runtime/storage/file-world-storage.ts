import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import type { PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";
import {
  deserializePackedChunkSnapshot,
  serializePackedChunkSnapshot,
  type SerializedPackedChunkSnapshot,
} from "../protocol/packed-chunk-wire";
import {
  createWorldSaveMetadata,
  isWorldSaveMetadataCompatible,
  touchWorldSaveMetadata,
  type ChunkStorage,
  type OpenWorldStorageRequest,
  type WorldSaveMetadata,
  type WorldStorage,
  type WorldStorageSession,
} from "./world-storage";

const WORLD_METADATA_FILENAME = "world.json";
const CHUNKS_DIRECTORY_NAME = "chunks";
export const FILE_CHUNK_RECORD_SCHEMA_VERSION = 1;

interface FileChunkRecord {
  readonly schemaVersion: typeof FILE_CHUNK_RECORD_SCHEMA_VERSION;
  readonly saveId: string;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly snapshot: SerializedPackedChunkSnapshot;
  readonly savedAtMs: number;
  readonly lastLoadedAtMs?: number;
  readonly lastEvictedAtMs?: number;
}

function isEnoent(error: unknown): error is NodeJS.ErrnoException {
  return typeof error === "object" && error !== null && "code" in error && error.code === "ENOENT";
}

async function readJsonFile<T>(filePath: string): Promise<T | undefined> {
  try {
    return JSON.parse(await readFile(filePath, "utf8")) as T;
  } catch (error) {
    if (isEnoent(error)) {
      return undefined;
    }

    throw new Error(`Unable to read JSON file ${filePath}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

async function writeJsonFile(filePath: string, value: unknown): Promise<void> {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

async function resetSaveDirectory(worldDirectory: string): Promise<void> {
  await rm(worldDirectory, { recursive: true, force: true });
  await mkdir(path.resolve(worldDirectory, CHUNKS_DIRECTORY_NAME), { recursive: true });
}

function getSaveDirectoryName(saveId: string): string {
  return encodeURIComponent(saveId);
}

export function getFileWorldSaveDirectory(rootDirectory: string, saveId: string): string {
  return path.resolve(rootDirectory, getSaveDirectoryName(saveId));
}

export function getFileWorldMetadataPath(rootDirectory: string, saveId: string): string {
  return path.resolve(getFileWorldSaveDirectory(rootDirectory, saveId), WORLD_METADATA_FILENAME);
}

export function getFileChunkRecordPath(rootDirectory: string, saveId: string, chunkX: number, chunkZ: number): string {
  return path.resolve(getFileWorldSaveDirectory(rootDirectory, saveId), CHUNKS_DIRECTORY_NAME, `${chunkX.toString()},${chunkZ.toString()}.json`);
}

class FileChunkStorage implements ChunkStorage {
  public constructor(
    private readonly rootDirectory: string,
    private readonly saveId: string,
    private readonly now: () => number,
  ) {}

  public async loadChunk(chunkX: number, chunkZ: number): Promise<PackedChunkSnapshot | undefined> {
    const filePath = getFileChunkRecordPath(this.rootDirectory, this.saveId, chunkX, chunkZ);
    const record = await readJsonFile<FileChunkRecord>(filePath);
    if (record === undefined) {
      return undefined;
    }
    if (record.schemaVersion !== FILE_CHUNK_RECORD_SCHEMA_VERSION) {
      throw new Error(`Unsupported file chunk record schemaVersion ${String(record.schemaVersion)} in ${filePath}`);
    }

    await writeJsonFile(filePath, {
      ...record,
      lastLoadedAtMs: this.now(),
    } satisfies FileChunkRecord);
    return deserializePackedChunkSnapshot(record.snapshot);
  }

  public async saveChunk(snapshot: PackedChunkSnapshot): Promise<void> {
    await writeJsonFile(
      getFileChunkRecordPath(this.rootDirectory, this.saveId, snapshot.chunkX, snapshot.chunkZ),
      {
        schemaVersion: FILE_CHUNK_RECORD_SCHEMA_VERSION,
        saveId: this.saveId,
        chunkX: snapshot.chunkX,
        chunkZ: snapshot.chunkZ,
        snapshot: serializePackedChunkSnapshot(snapshot),
        savedAtMs: this.now(),
      } satisfies FileChunkRecord,
    );
  }

  public async evictChunk(chunkX: number, chunkZ: number): Promise<void> {
    const filePath = getFileChunkRecordPath(this.rootDirectory, this.saveId, chunkX, chunkZ);
    const record = await readJsonFile<FileChunkRecord>(filePath);
    if (record === undefined) {
      return;
    }
    if (record.schemaVersion !== FILE_CHUNK_RECORD_SCHEMA_VERSION) {
      throw new Error(`Unsupported file chunk record schemaVersion ${String(record.schemaVersion)} in ${filePath}`);
    }

    await writeJsonFile(filePath, {
      ...record,
      lastEvictedAtMs: this.now(),
    } satisfies FileChunkRecord);
  }
}

class FileWorldStorageSession implements WorldStorageSession {
  public readonly chunks: ChunkStorage;

  public constructor(
    public readonly metadata: WorldSaveMetadata,
    rootDirectory: string,
    now: () => number,
  ) {
    this.chunks = new FileChunkStorage(rootDirectory, metadata.saveId, now);
  }

  public async close(): Promise<void> {}
}

export class FileWorldStorage implements WorldStorage {
  private readonly rootDirectory: string;

  public constructor(
    rootDirectory: string,
    private readonly now: () => number = () => Date.now(),
  ) {
    this.rootDirectory = path.resolve(rootDirectory);
  }

  public async openWorld(request: OpenWorldStorageRequest): Promise<WorldStorageSession> {
    const worldDirectory = getFileWorldSaveDirectory(this.rootDirectory, request.saveId);
    const metadataPath = getFileWorldMetadataPath(this.rootDirectory, request.saveId);
    const existing = await readJsonFile<WorldSaveMetadata>(metadataPath);

    let metadata: WorldSaveMetadata;
    if (existing !== undefined && isWorldSaveMetadataCompatible(existing, request)) {
      await mkdir(path.resolve(worldDirectory, CHUNKS_DIRECTORY_NAME), { recursive: true });
      metadata = touchWorldSaveMetadata(existing, request.openedAtMs);
    } else {
      await resetSaveDirectory(worldDirectory);
      metadata = createWorldSaveMetadata(request);
    }

    await writeJsonFile(metadataPath, metadata);
    return new FileWorldStorageSession(metadata, this.rootDirectory, this.now);
  }

  public getRootDirectory(): string {
    return this.rootDirectory;
  }
}
