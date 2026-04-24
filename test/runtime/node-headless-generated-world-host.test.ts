import { spawnSync } from "node:child_process";
import { mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldSaveId } from "../../src/runtime/host/generated-world-host";
import {
  runHeadlessGeneratedWorldHost,
  type HeadlessGeneratedWorldHostResult,
} from "../../src/runtime/node/headless-generated-world-host";
import { getFileWorldSaveDirectory } from "../../src/runtime/storage/file-world-storage";

const TEMP_DIRECTORIES: string[] = [];
const NODE_HEADLESS_HOST_TIMEOUT_MS = 30_000;

async function createTempDirectory(): Promise<string> {
  const directory = await mkdtemp(path.join(tmpdir(), "mclone-node-host-"));
  TEMP_DIRECTORIES.push(directory);
  return directory;
}

describe("Headless Node host", () => {
  afterEach(async () => {
    Registry.BLOCK.clear();

    while (TEMP_DIRECTORIES.length > 0) {
      await rm(TEMP_DIRECTORIES.pop()!, { recursive: true, force: true });
    }
  });

  test("runs the shared authoritative host core against file-backed storage", async () => {
    const saveRoot = await createTempDirectory();
    const result = await runHeadlessGeneratedWorldHost({
      seed: "12345",
      preset: "default",
      saveRoot,
      chunkViews: [{
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      }],
    });

    expect(result.saveId).toBe(createGeneratedWorldSaveId(12345n, "default"));
    expect(result.viewResults).toEqual([{
      request: {
        type: "set_chunk_view",
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      },
      chunkChanged: true,
      snapshotCount: 25,
      unloadCount: 0,
    }]);
    expect(await readdir(path.resolve(getFileWorldSaveDirectory(saveRoot, result.saveId), "chunks"))).toHaveLength(25);
  }, NODE_HEADLESS_HOST_TIMEOUT_MS);

  test("boots through the Node CLI config entry point", async () => {
    const tempDirectory = await createTempDirectory();
    const configPath = path.resolve(tempDirectory, "node-host.json");

    await writeFile(configPath, `${JSON.stringify({
      seed: "12345",
      preset: "default",
      saveRoot: "./saves",
      chunkViews: [
        { centerChunkX: 0, centerChunkZ: 0, radius: 1 },
        { centerChunkX: 2, centerChunkZ: 0, radius: 1 },
      ],
    }, null, 2)}\n`, "utf8");

    const spawned = spawnSync(
      process.execPath,
      [
        "--disable-warning=ExperimentalWarning",
        "--experimental-transform-types",
        "--experimental-loader",
        "./scripts/node-ts-loader.mjs",
        "./src/runtime/node/headless-generated-world-host.ts",
        "--config",
        configPath,
      ],
      {
        cwd: process.cwd(),
        encoding: "utf8",
      },
    );

    expect(spawned.status, spawned.stderr).toBe(0);

    const result = JSON.parse(spawned.stdout) as HeadlessGeneratedWorldHostResult;
    expect(result.saveId).toBe(createGeneratedWorldSaveId(12345n, "default"));
    expect(result.viewResults).toEqual([
      expect.objectContaining({
        chunkChanged: true,
        snapshotCount: 25,
        unloadCount: 0,
      }),
      expect.objectContaining({
        chunkChanged: true,
        snapshotCount: 25,
        unloadCount: 10,
      }),
    ]);
    expect(result.saveDirectory).toBe(path.resolve(tempDirectory, "saves", encodeURIComponent(result.saveId)));
  }, NODE_HEADLESS_HOST_TIMEOUT_MS);
});
