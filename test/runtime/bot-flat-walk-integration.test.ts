import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, test } from "vitest";
import { Registry } from "../../src/core/registry";
import {
  BotRuntime,
  WalkToPointBotController,
  createBotClientRuntime,
  type BotClientRuntimeOptions,
} from "../../src/runtime/bot";
import type { ClientRuntime } from "../../src/runtime/client/client-runtime";
import { GeneratedWorldHttpServer, GeneratedWorldRemoteService } from "../../src/runtime/node/generated-world-http-server";
import {
  WORLD_HTTP_PROTOCOL_VERSION,
  type OpenWorldSessionRequest,
  type SessionChunkViewRequest,
  type SessionPlayerInputRequest,
  type SessionPollUpdatesRequest,
} from "../../src/runtime/protocol/world-http-protocol";
import {
  deserializeWorldClientMessage,
  serializeWorldHostMessages,
} from "../../src/runtime/protocol/world-wire-protocol";
import type { ClientPlayerState } from "../../src/runtime/protocol/world-messages";
import { RemoteWorldTransport, RemoteWorldWebSocketTransport } from "../../src/runtime/transport/remote-world-transport";

const TEMP_DIRECTORIES: string[] = [];
const SERVICES: GeneratedWorldRemoteService[] = [];
const SERVERS: GeneratedWorldHttpServer[] = [];

async function createTempDirectory(): Promise<string> {
  const directory = await mkdtemp(path.join(tmpdir(), "mclone-bot-flat-walk-"));
  TEMP_DIRECTORIES.push(directory);
  return directory;
}

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
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
      return jsonResponse(await service.openWorld(message, request.resumeSessionId));
    }

    const sessionChunkMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/chunk-view$/);
    if (method === "POST" && sessionChunkMatch !== null) {
      const request = JSON.parse(body) as SessionChunkViewRequest;
      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "set_chunk_view") {
        throw new Error(`Expected set_chunk_view message, got ${message.type}`);
      }
      return jsonResponse(await service.setChunkView(decodeURIComponent(sessionChunkMatch[1]!), message));
    }

    const sessionInputMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/player-input$/);
    if (method === "POST" && sessionInputMatch !== null) {
      const request = JSON.parse(body) as SessionPlayerInputRequest;
      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "set_player_input") {
        throw new Error(`Expected set_player_input message, got ${message.type}`);
      }
      return jsonResponse(await service.setPlayerInput(decodeURIComponent(sessionInputMatch[1]!), message));
    }

    const sessionPollMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/updates\/poll$/);
    if (method === "POST" && sessionPollMatch !== null) {
      const request = JSON.parse(body) as SessionPollUpdatesRequest;
      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "poll_world_updates") {
        throw new Error(`Expected poll_world_updates message, got ${message.type}`);
      }
      return jsonResponse(await service.pollUpdates(decodeURIComponent(sessionPollMatch[1]!), message));
    }

    return new Response(JSON.stringify({
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages([{
        type: "world_error",
        message: `Unhandled route ${method} ${url.pathname}`,
      }]),
    }), {
      status: 404,
      headers: { "content-type": "application/json" },
    });
  };
}

function playerPosition(playerState: ClientPlayerState): { readonly x: number; readonly y: number; readonly z: number } {
  return playerState.movementBody?.position ?? playerState.position;
}

function sleep(ms = 0): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function drainCenterChunk(runtime: ClientRuntime): Promise<void> {
  for (let attempt = 0; attempt < 200; attempt++) {
    await runtime.drainTransportUpdates();
    if (runtime.getClientWorld().getChunkSnapshot(0, 0) !== undefined) {
      return;
    }
    await sleep(1);
  }

  throw new Error("expected flat grass center chunk to load");
}

function createRemoteBotClientRuntime(
  service: GeneratedWorldRemoteService,
  options: Omit<BotClientRuntimeOptions, "transport">,
): ClientRuntime {
  return createBotClientRuntime({
    ...options,
    transport: new RemoteWorldTransport("http://mclone.test", {
      fetchImpl: createServiceFetch(service),
    }),
  });
}

function createWebSocketBotClientRuntime(
  server: GeneratedWorldHttpServer,
  options: Omit<BotClientRuntimeOptions, "transport">,
): ClientRuntime {
  return createBotClientRuntime({
    ...options,
    transport: new RemoteWorldWebSocketTransport(server.getBaseUrl()),
  });
}

function createWalkBot(
  clientRuntime: ClientRuntime,
  controller: WalkToPointBotController,
): BotRuntime {
  return new BotRuntime({
    clientRuntime,
    openWorldRequest: {
      type: "open_world",
      seed: 12345n,
      preset: "flat_grass",
      storageMode: "none",
      config: {
        lightingMode: "none",
        liquidSimulationMode: "none",
      },
      playerProfile: {
        name: "FlatWalkBot",
      },
    },
    viewRadius: 0,
    controller,
  });
}

async function runWalkAcceptance(options: {
  readonly bot: BotRuntime;
  readonly clientRuntime: ClientRuntime;
  readonly controller: WalkToPointBotController;
  readonly advanceHost: () => Promise<void> | void;
}): Promise<void> {
  try {
    await options.bot.open();
    await drainCenterChunk(options.clientRuntime);
    const startState = options.clientRuntime.publishPresentationState().localPlayerState;
    expect(startState).toBeDefined();
    const start = playerPosition(startState!);

    for (let tick = 0; tick < 220 && !options.controller.getSnapshot().arrived; tick++) {
      await options.bot.tick(50);
      await options.advanceHost();
      await options.clientRuntime.drainTransportUpdates();
      await sleep(1);
    }

    const snapshot = options.controller.getSnapshot();
    const finalState = options.clientRuntime.publishPresentationState().localPlayerState;
    expect(snapshot.target).toBeDefined();
    expect(snapshot.arrived).toBe(true);
    expect(finalState).toBeDefined();

    const final = playerPosition(finalState!);
    expect(finalState!.acknowledgedInputSequence).toBeGreaterThan(10);
    expect(finalState!.revision).toBeGreaterThan(startState!.revision);
    expect(Math.hypot(final.x - start.x, final.z - start.z)).toBeGreaterThan(2.0);
    expect(Math.hypot(final.x - snapshot.target!.x, final.z - snapshot.target!.z)).toBeLessThanOrEqual(1.0);
    expect(final.y).toBeGreaterThanOrEqual(63.9);
    expect(final.y).toBeLessThan(65.0);
  } finally {
    await options.bot.close();
  }
}

describe("flat-grass bot walking acceptance", () => {
  afterEach(async () => {
    for (const server of SERVERS.splice(0)) {
      await server.stop();
    }
    for (const service of SERVICES.splice(0)) {
      service.dispose();
    }
    for (const directory of TEMP_DIRECTORIES.splice(0)) {
      await rm(directory, { recursive: true, force: true });
    }
    Registry.BLOCK.clear();
  });

  test("walk-to-point bot connects to a flat remote world and reaches a nearby target", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
      autoTick: false,
    });
    SERVICES.push(service);
    const clientRuntime = createRemoteBotClientRuntime(service, {
      seed: 12345n,
      pollUpdateMaxMessages: 64,
    });
    const controller = new WalkToPointBotController({
      randomSeed: 17,
      minDistance: 4.0,
      maxDistance: 4.0,
      arrivalRadius: 0.9,
    });

    await runWalkAcceptance({
      bot: createWalkBot(clientRuntime, controller),
      clientRuntime,
      controller,
      advanceHost: () => service.forceTick(1),
    });
  }, 20_000);

  test("walk-to-point bot reaches a nearby target through the real WebSocket server", async () => {
    const server = new GeneratedWorldHttpServer({
      host: "127.0.0.1",
      port: 0,
      saveRoot: await createTempDirectory(),
    });
    await server.start();
    SERVERS.push(server);
    const clientRuntime = createWebSocketBotClientRuntime(server, {
      seed: 12345n,
      pollUpdateMaxMessages: 64,
    });
    const controller = new WalkToPointBotController({
      randomSeed: 17,
      minDistance: 4.0,
      maxDistance: 4.0,
      arrivalRadius: 0.9,
    });

    await runWalkAcceptance({
      bot: createWalkBot(clientRuntime, controller),
      clientRuntime,
      controller,
      advanceHost: () => server.forceTick(1),
    });
  }, 20_000);
});
