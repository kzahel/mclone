import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldSaveId } from "../../src/runtime/host/generated-world-host";
import { GeneratedWorldRemoteService } from "../../src/runtime/node/generated-world-http-server";
import {
  deserializeWorldClientMessage,
  type OpenWorldSessionRequest,
  type SessionChunkViewRequest,
} from "../../src/runtime/protocol/world-http-protocol";
import { RemoteWorldClient, RemoteWorldTransport } from "../../src/runtime/transport/remote-world-transport";
import { createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";

const TEMP_DIRECTORIES: string[] = [];
const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "browser_smoke",
} as const;

async function createTempDirectory(): Promise<string> {
  const directory = await mkdtemp(path.join(tmpdir(), "mclone-remote-world-transport-"));
  TEMP_DIRECTORIES.push(directory);
  return directory;
}

function createServiceFetch(service: GeneratedWorldRemoteService): typeof fetch {
  return async (input, init) => {
    const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url);
    const method = init?.method ?? (typeof input === "object" && "method" in input ? input.method : "GET");
    const body = typeof init?.body === "string" ? init.body : "";

    if (method === "POST" && url.pathname === "/api/world/session") {
      const request = JSON.parse(body) as OpenWorldSessionRequest;
      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "open_world") {
        throw new Error(`Expected open_world message, got ${message.type}`);
      }

      return new Response(JSON.stringify(await service.openWorld(message)), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }

    const sessionChunkMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/chunk-view$/);
    if (method === "POST" && sessionChunkMatch !== null) {
      const request = JSON.parse(body) as SessionChunkViewRequest;
      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "set_chunk_view") {
        throw new Error(`Expected set_chunk_view message, got ${message.type}`);
      }

      return new Response(JSON.stringify(await service.setChunkView(decodeURIComponent(sessionChunkMatch[1]!), message)), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }

    return new Response(JSON.stringify({ error: `Unhandled route ${method} ${url.pathname}` }), {
      status: 404,
      headers: { "content-type": "application/json" },
    });
  };
}

function createRemoteWorldClient(baseUrl: string, fetchImpl: typeof fetch): RemoteWorldClient {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);
  return new RemoteWorldClient(
    new RemoteWorldTransport(baseUrl, fetchImpl),
    (worldOpened) => new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: worldOpened.minBuildHeight,
      height: worldOpened.height,
      biomeSource,
      biomeZoomSeed: 12345n,
      blockStateResolver: createBlockStateResolver(blocks.airState),
    }),
  );
}

describe("RemoteWorld transport", () => {
  const services: GeneratedWorldRemoteService[] = [];

  afterEach(async () => {
    Registry.BLOCK.clear();

    services.length = 0;

    while (TEMP_DIRECTORIES.length > 0) {
      await rm(TEMP_DIRECTORIES.pop()!, { recursive: true, force: true });
    }
  });

  test("streams generated chunks through the remote HTTP host boundary", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    services.push(service);

    const client = createRemoteWorldClient("http://127.0.0.1:4173", createServiceFetch(service));
    await expect(client.openWorld(OPEN_WORLD_REQUEST)).resolves.toEqual({
      type: "world_opened",
      minBuildHeight: 0,
      height: 256,
      saveMetadata: {
        saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
        storageVersion: 1,
        seed: "12345",
        preset: "browser_smoke",
        minBuildHeight: 0,
        height: 256,
        createdAtMs: expect.any(Number),
        lastOpenedAtMs: expect.any(Number),
      },
    });

    expect(await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).toBe(true);

    expect(client.getLevel().getLoadedChunkCount()).toBe(25);
    expect(service.getSessionCount()).toBe(1);
  });

  test("supports two concurrent remote client sessions against one Node host", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    services.push(service);
    const fetchImpl = createServiceFetch(service);

    const firstClient = createRemoteWorldClient("http://127.0.0.1:4173", fetchImpl);
    const secondClient = createRemoteWorldClient("http://127.0.0.1:4173", fetchImpl);
    const [firstOpened, secondOpened] = await Promise.all([
      firstClient.openWorld(OPEN_WORLD_REQUEST),
      secondClient.openWorld(OPEN_WORLD_REQUEST),
    ]);

    expect(firstOpened.saveMetadata.saveId).toBe(secondOpened.saveMetadata.saveId);
    await Promise.all([
      firstClient.setChunkView({
        type: "set_chunk_view",
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      }),
      secondClient.setChunkView({
        type: "set_chunk_view",
        centerChunkX: 2,
        centerChunkZ: 0,
        radius: 1,
      }),
    ]);

    expect(firstClient.getLevel().getLoadedChunkCount()).toBe(25);
    expect(secondClient.getLevel().getLoadedChunkCount()).toBe(25);
    expect(service.getSessionCount()).toBe(2);
  });
});
