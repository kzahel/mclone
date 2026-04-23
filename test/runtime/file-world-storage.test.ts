import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, test } from "vitest";
import { FileWorldStorage, getFileChunkRecordPath, getFileWorldMetadataPath } from "../../src/runtime/storage/file-world-storage";
import type { ChunkSnapshot } from "../../src/world/level/chunk-snapshot";

const TEMP_DIRECTORIES: string[] = [];
const STORAGE_REQUEST = {
  saveId: "test-save",
  storageVersion: 1,
  seed: "12345",
  preset: "default",
  minBuildHeight: 0,
  height: 256,
  openedAtMs: 1_000,
} as const;
const CHUNK_SNAPSHOT: ChunkSnapshot = {
  chunkX: 0,
  chunkZ: 0,
  biomes: [0],
  sections: [],
};

async function createTempDirectory(): Promise<string> {
  const directory = await mkdtemp(path.join(tmpdir(), "mclone-file-world-storage-"));
  TEMP_DIRECTORIES.push(directory);
  return directory;
}

async function readJsonFile<T>(filePath: string): Promise<T> {
  return JSON.parse(await readFile(filePath, "utf8")) as T;
}

describe("FileWorldStorage", () => {
  afterEach(async () => {
    while (TEMP_DIRECTORIES.length > 0) {
      await rm(TEMP_DIRECTORIES.pop()!, { recursive: true, force: true });
    }
  });

  test("reopens compatible saves and keeps chunk records on disk", async () => {
    const saveRoot = await createTempDirectory();
    let now = 1_000;
    const storage = new FileWorldStorage(saveRoot, () => now);
    const firstSession = await storage.openWorld(STORAGE_REQUEST);

    await firstSession.chunks.saveChunk(CHUNK_SNAPSHOT);

    now = 2_000;
    const secondSession = await storage.openWorld({
      ...STORAGE_REQUEST,
      openedAtMs: now,
    });

    expect(secondSession.metadata).toEqual({
      saveId: STORAGE_REQUEST.saveId,
      storageVersion: STORAGE_REQUEST.storageVersion,
      seed: STORAGE_REQUEST.seed,
      preset: STORAGE_REQUEST.preset,
      minBuildHeight: STORAGE_REQUEST.minBuildHeight,
      height: STORAGE_REQUEST.height,
      createdAtMs: 1_000,
      lastOpenedAtMs: 2_000,
    });
    expect(await secondSession.chunks.loadChunk(0, 0)).toEqual(CHUNK_SNAPSHOT);
    await secondSession.chunks.evictChunk(0, 0);

    expect(await readJsonFile(getFileWorldMetadataPath(saveRoot, STORAGE_REQUEST.saveId))).toEqual({
      saveId: STORAGE_REQUEST.saveId,
      storageVersion: STORAGE_REQUEST.storageVersion,
      seed: STORAGE_REQUEST.seed,
      preset: STORAGE_REQUEST.preset,
      minBuildHeight: STORAGE_REQUEST.minBuildHeight,
      height: STORAGE_REQUEST.height,
      createdAtMs: 1_000,
      lastOpenedAtMs: 2_000,
    });
    expect(await readJsonFile(getFileChunkRecordPath(saveRoot, STORAGE_REQUEST.saveId, 0, 0))).toEqual({
      saveId: STORAGE_REQUEST.saveId,
      chunkX: 0,
      chunkZ: 0,
      snapshot: CHUNK_SNAPSHOT,
      savedAtMs: 1_000,
      lastLoadedAtMs: 2_000,
      lastEvictedAtMs: 2_000,
    });
  });

  test("resets incompatible saves and clears stale chunk records", async () => {
    const saveRoot = await createTempDirectory();
    let now = 1_000;
    const storage = new FileWorldStorage(saveRoot, () => now);
    const firstSession = await storage.openWorld(STORAGE_REQUEST);
    await firstSession.chunks.saveChunk(CHUNK_SNAPSHOT);

    now = 3_000;
    const secondSession = await storage.openWorld({
      ...STORAGE_REQUEST,
      height: 384,
      openedAtMs: now,
    });

    expect(secondSession.metadata).toEqual({
      saveId: STORAGE_REQUEST.saveId,
      storageVersion: STORAGE_REQUEST.storageVersion,
      seed: STORAGE_REQUEST.seed,
      preset: STORAGE_REQUEST.preset,
      minBuildHeight: STORAGE_REQUEST.minBuildHeight,
      height: 384,
      createdAtMs: 3_000,
      lastOpenedAtMs: 3_000,
    });
    expect(await secondSession.chunks.loadChunk(0, 0)).toBeUndefined();
  });
});
