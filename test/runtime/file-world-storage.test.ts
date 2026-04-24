import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, test } from "vitest";
import { FILE_CHUNK_RECORD_SCHEMA_VERSION, FileWorldStorage, getFileChunkRecordPath, getFileWorldMetadataPath } from "../../src/runtime/storage/file-world-storage";
import { serializePackedChunkSnapshot } from "../../src/runtime/protocol/packed-chunk-wire";
import type { PackedChunkSnapshot } from "../../src/world/level/packed-chunk-snapshot";

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
const CHUNK_SNAPSHOT: PackedChunkSnapshot = {
  chunkX: 0,
  chunkZ: 0,
  biomes: [0],
  sections: [{
    y: 0,
    paletteStateIds: new Uint32Array([0, 1]),
    bitsPerBlock: 4,
    packedBlockIndices: new BigInt64Array([0x0123456789ABCDEFn, -1n]),
  }],
  blockTicks: [],
  liquidTicks: [],
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
      schemaVersion: FILE_CHUNK_RECORD_SCHEMA_VERSION,
      saveId: STORAGE_REQUEST.saveId,
      chunkX: 0,
      chunkZ: 0,
      snapshot: serializePackedChunkSnapshot(CHUNK_SNAPSHOT),
      savedAtMs: 1_000,
      lastLoadedAtMs: 2_000,
      lastEvictedAtMs: 2_000,
    });
    expect(JSON.stringify(await readJsonFile(getFileChunkRecordPath(saveRoot, STORAGE_REQUEST.saveId, 0, 0)))).not.toContain("properties");
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
