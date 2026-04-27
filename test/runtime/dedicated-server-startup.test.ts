import { spawn, type ChildProcess } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldSaveId } from "../../src/runtime/host/generated-world-host";
import { WORLD_HTTP_PROTOCOL_VERSION } from "../../src/runtime/protocol/world-http-protocol";
import type { WorldHostMessage } from "../../src/runtime/protocol/world-messages";
import { RemoteWorldWebSocketTransport } from "../../src/runtime/transport/remote-world-transport";

const TEMP_DIRECTORIES: string[] = [];
const SERVER_PROCESSES: ChildProcess[] = [];
const TRANSPORTS: RemoteWorldWebSocketTransport[] = [];
const DEDICATED_SERVER_STARTUP_TIMEOUT_MS = 30_000;

interface DedicatedServerStartupMessage {
  readonly type: "listening";
  readonly url: string;
  readonly saveRoot: string;
  readonly protocolVersion: number;
}

async function createTempDirectory(): Promise<string> {
  const directory = await mkdtemp(path.join(tmpdir(), "mclone-dedicated-startup-"));
  TEMP_DIRECTORIES.push(directory);
  return directory;
}

function startDedicatedServer(configPath: string): Promise<DedicatedServerStartupMessage> {
  const child = spawn(
    process.execPath,
    [
      "--disable-warning=ExperimentalWarning",
      "--experimental-transform-types",
      "--experimental-loader",
      "./scripts/node-ts-loader.mjs",
      "./src/runtime/node/generated-world-http-server.ts",
      "--config",
      configPath,
    ],
    {
      cwd: process.cwd(),
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  SERVER_PROCESSES.push(child);
  if (child.stdout === null || child.stderr === null) {
    return Promise.reject(new Error("dedicated server child process did not expose stdout/stderr"));
  }

  return new Promise((resolve, reject) => {
    let stdout = "";
    let stderr = "";
    let settled = false;
    const timer = setTimeout(() => {
      finish(new Error(`dedicated server did not report readiness within ${DEDICATED_SERVER_STARTUP_TIMEOUT_MS.toString()}ms\n${stderr}`));
      child.kill("SIGTERM");
    }, DEDICATED_SERVER_STARTUP_TIMEOUT_MS);

    const finish = (error: Error | undefined, message?: DedicatedServerStartupMessage): void => {
      if (settled) {
        return;
      }

      settled = true;
      clearTimeout(timer);
      if (error !== undefined) {
        reject(error);
        return;
      }

      resolve(message!);
    };

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      stdout += chunk;
      const newlineIndex = stdout.indexOf("\n");
      if (newlineIndex < 0) {
        return;
      }

      const line = stdout.slice(0, newlineIndex);
      try {
        finish(undefined, JSON.parse(line) as DedicatedServerStartupMessage);
      } catch (error) {
        finish(error instanceof Error ? error : new Error(String(error)));
      }
    });
    child.stderr.on("data", (chunk: string) => {
      stderr += chunk;
    });
    child.on("error", (error) => {
      finish(error);
    });
    child.on("close", (code, signal) => {
      if (!settled) {
        finish(new Error(`dedicated server exited before readiness: code=${String(code)} signal=${String(signal)}\n${stderr}`));
      }
    });
  });
}

async function stopServerProcess(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }

  child.kill("SIGTERM");
  await new Promise<void>((resolve) => {
    const timer = setTimeout(() => {
      if (child.exitCode === null && child.signalCode === null) {
        child.kill("SIGKILL");
      }
      resolve();
    }, 2_000);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

async function readHealth(baseUrl: string): Promise<Record<string, unknown>> {
  const response = await fetch(`${baseUrl}/healthz`);
  expect(response.ok).toBe(true);
  return await response.json() as Record<string, unknown>;
}

function collectChunkSnapshots(messages: readonly WorldHostMessage[]): number {
  return messages.filter((message) => message.type === "chunk_snapshot").length;
}

async function waitForChunkSnapshots(
  transport: RemoteWorldWebSocketTransport,
  expectedCount: number,
): Promise<void> {
  let snapshotCount = 0;
  const deadline = Date.now() + DEDICATED_SERVER_STARTUP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 10));
    snapshotCount += collectChunkSnapshots(await transport.pollUpdates({
      type: "poll_world_updates",
      maxMessages: 128,
    }));
    if (snapshotCount >= expectedCount) {
      return;
    }
  }

  throw new Error(`expected ${expectedCount.toString()} headless client chunk snapshots, got ${snapshotCount.toString()}`);
}

describe("Dedicated server startup", () => {
  afterEach(async () => {
    Registry.BLOCK.clear();

    while (TRANSPORTS.length > 0) {
      TRANSPORTS.pop()!.close();
    }

    while (SERVER_PROCESSES.length > 0) {
      await stopServerProcess(SERVER_PROCESSES.pop()!);
    }

    while (TEMP_DIRECTORIES.length > 0) {
      await rm(TEMP_DIRECTORIES.pop()!, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    }
  }, DEDICATED_SERVER_STARTUP_TIMEOUT_MS);

  test("boots from a config file and accepts a no-GPU WebSocket client", async () => {
    const tempDirectory = await createTempDirectory();
    const saveRoot = path.resolve(tempDirectory, "saves");
    const configPath = path.resolve(tempDirectory, "dedicated-server.json");
    await writeFile(configPath, `${JSON.stringify({
      host: "127.0.0.1",
      port: 0,
      saveRoot,
    }, null, 2)}\n`, "utf8");

    const startup = await startDedicatedServer(configPath);
    expect(startup).toEqual({
      type: "listening",
      url: expect.stringMatching(/^http:\/\/127\.0\.0\.1:\d+$/),
      saveRoot,
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
    });
    expect(await readHealth(startup.url)).toEqual({
      ok: true,
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      sessionCount: 0,
      worldCount: 0,
    });

    const transport = new RemoteWorldWebSocketTransport(startup.url);
    TRANSPORTS.push(transport);
    const openMessages = await transport.openWorld({
      type: "open_world",
      seed: 12345n,
      preset: "browser_smoke",
      config: {
        lightingMode: "none",
      },
      playerProfile: {
        name: "Headless Client",
      },
    });

    expect(openMessages).toContainEqual(expect.objectContaining({
      type: "world_opened",
      saveMetadata: expect.objectContaining({
        saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
        seed: "12345",
        preset: "browser_smoke",
      }),
    }));
    expect(openMessages).toContainEqual(expect.objectContaining({
      type: "session_state",
      state: expect.objectContaining({
        playerProfile: {
          name: "Headless Client",
        },
        saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
      }),
    }));

    await transport.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await waitForChunkSnapshots(transport, 25);

    expect(await readHealth(startup.url)).toEqual({
      ok: true,
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      sessionCount: 1,
      worldCount: 1,
    });
  }, DEDICATED_SERVER_STARTUP_TIMEOUT_MS);
});
