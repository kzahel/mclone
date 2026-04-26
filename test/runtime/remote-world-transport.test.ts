import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, test } from "vitest";
import fixture from "../fixtures/creatures/overworld-seed-12345-chunk--7--15-entities.json";
import { Registry } from "../../src/core/registry";
import { createGeneratedWorldSaveId, GENERATED_WORLD_STORAGE_VERSION } from "../../src/runtime/host/generated-world-host";
import { GeneratedWorldHttpServer, GeneratedWorldRemoteService } from "../../src/runtime/node/generated-world-http-server";
import { type ClientRuntime, WorldClientRuntimeFacade } from "../../src/runtime/client/client-runtime";
import {
  deserializeWorldHostMessages,
  deserializeWorldClientMessage,
  WORLD_HTTP_PROTOCOL_VERSION,
  type OpenWorldSessionRequest,
  type SessionChunkViewRequest,
  type SessionPlayerInputRequest,
  type SessionPollUpdatesRequest,
  type WorldHttpErrorCode,
  type WorldHttpErrorResponse,
} from "../../src/runtime/protocol/world-http-protocol";
import type { ChunkSnapshotMessage, ClientSessionState, EntitySnapshot, WorldHostMessage } from "../../src/runtime/protocol/world-messages";
import { RemoteWorldClient, RemoteWorldTransport, RemoteWorldWebSocketTransport } from "../../src/runtime/transport/remote-world-transport";
import type { WorldTransport } from "../../src/runtime/transport/local-world-transport";
import { createBlockStateResolver } from "../../src/world/level/chunk-snapshot";
import { ClientChunkCache } from "../../src/world/level/client-chunk-cache";
import { registerGeneratedRenderBlocks } from "../../src/world/level/generated-render-blocks";
import { OverworldBiomeSource } from "../../src/worldgen/biome/overworld-biome-source";
import type { CreatureGenerationFixture } from "../../src/oracle/integration/creature-fixture";

const TEMP_DIRECTORIES: string[] = [];
const REMOTE_SERVERS: GeneratedWorldHttpServer[] = [];
const creatureFixture = fixture as unknown as CreatureGenerationFixture;
const REMOTE_PLAYER_PROFILE = {
  name: "Remote Player",
} as const;
const OPEN_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "browser_smoke",
  config: {
    lightingMode: "none",
  },
  playerProfile: REMOTE_PLAYER_PROFILE,
} as const;
const OPEN_CREATURE_WORLD_REQUEST = {
  type: "open_world",
  seed: 12345n,
  preset: "default",
  storageMode: "none",
  config: {
    lightingMode: "none",
  },
  playerProfile: REMOTE_PLAYER_PROFILE,
} as const;
const REMOTE_WORLD_TRANSPORT_TIMEOUT_MS = 30_000;

async function createTempDirectory(): Promise<string> {
  const directory = await mkdtemp(path.join(tmpdir(), "mclone-remote-world-transport-"));
  TEMP_DIRECTORIES.push(directory);
  return directory;
}

async function createRemoteServer(): Promise<GeneratedWorldHttpServer> {
  const server = new GeneratedWorldHttpServer({
    host: "127.0.0.1",
    port: 0,
    saveRoot: await createTempDirectory(),
  });
  await server.start();
  REMOTE_SERVERS.push(server);
  return server;
}

function createServiceFetch(service: GeneratedWorldRemoteService): typeof fetch {
  return async (input, init) => {
    const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url);
    const method = init?.method ?? (typeof input === "object" && "method" in input ? input.method : "GET");
    const body = typeof init?.body === "string" ? init.body : "";

    if (method === "POST" && url.pathname === "/api/world/session") {
      const request = JSON.parse(body) as OpenWorldSessionRequest;
      if (request.protocolVersion !== WORLD_HTTP_PROTOCOL_VERSION) {
        return new Response(JSON.stringify({
          protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          error: {
            code: "protocol_version_mismatch",
            message: "protocol mismatch",
            expectedProtocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          },
        } satisfies WorldHttpErrorResponse), {
          status: 409,
          headers: { "content-type": "application/json" },
        });
      }

      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "open_world") {
        throw new Error(`Expected open_world message, got ${message.type}`);
      }

      try {
        return new Response(JSON.stringify(await service.openWorld(message, request.resumeSessionId)), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      } catch (error) {
        if (error instanceof Error && "code" in error && "statusCode" in error) {
          const remoteError = error as { code: WorldHttpErrorCode; statusCode: number };
          return new Response(JSON.stringify({
            protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
            error: {
              code: remoteError.code,
              message: error.message,
            },
          } satisfies WorldHttpErrorResponse), {
            status: remoteError.statusCode,
            headers: { "content-type": "application/json" },
          });
        }

        throw error;
      }
    }

    const sessionChunkMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/chunk-view$/);
    if (method === "POST" && sessionChunkMatch !== null) {
      const request = JSON.parse(body) as SessionChunkViewRequest;
      if (request.protocolVersion !== WORLD_HTTP_PROTOCOL_VERSION) {
        return new Response(JSON.stringify({
          protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          error: {
            code: "protocol_version_mismatch",
            message: "protocol mismatch",
            expectedProtocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          },
        } satisfies WorldHttpErrorResponse), {
          status: 409,
          headers: { "content-type": "application/json" },
        });
      }

      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "set_chunk_view") {
        throw new Error(`Expected set_chunk_view message, got ${message.type}`);
      }

      try {
        return new Response(JSON.stringify(await service.setChunkView(decodeURIComponent(sessionChunkMatch[1]!), message)), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      } catch (error) {
        if (error instanceof Error && "code" in error && "statusCode" in error) {
          const remoteError = error as { code: WorldHttpErrorCode; statusCode: number };
          return new Response(JSON.stringify({
            protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
            error: {
              code: remoteError.code,
              message: error.message,
            },
          } satisfies WorldHttpErrorResponse), {
            status: remoteError.statusCode,
            headers: { "content-type": "application/json" },
          });
        }

        throw error;
      }
    }

    const sessionPlayerInputMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/player-input$/);
    if (method === "POST" && sessionPlayerInputMatch !== null) {
      const request = JSON.parse(body) as SessionPlayerInputRequest;
      if (request.protocolVersion !== WORLD_HTTP_PROTOCOL_VERSION) {
        return new Response(JSON.stringify({
          protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          error: {
            code: "protocol_version_mismatch",
            message: "protocol mismatch",
            expectedProtocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          },
        } satisfies WorldHttpErrorResponse), {
          status: 409,
          headers: { "content-type": "application/json" },
        });
      }

      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "set_player_input") {
        throw new Error(`Expected set_player_input message, got ${message.type}`);
      }

      try {
        return new Response(JSON.stringify(await service.setPlayerInput(decodeURIComponent(sessionPlayerInputMatch[1]!), message)), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      } catch (error) {
        if (error instanceof Error && "code" in error && "statusCode" in error) {
          const remoteError = error as { code: WorldHttpErrorCode; statusCode: number };
          return new Response(JSON.stringify({
            protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
            error: {
              code: remoteError.code,
              message: error.message,
            },
          } satisfies WorldHttpErrorResponse), {
            status: remoteError.statusCode,
            headers: { "content-type": "application/json" },
          });
        }

        throw error;
      }
    }

    const sessionPollUpdatesMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/updates\/poll$/);
    if (method === "POST" && sessionPollUpdatesMatch !== null) {
      const request = JSON.parse(body) as SessionPollUpdatesRequest;
      if (request.protocolVersion !== WORLD_HTTP_PROTOCOL_VERSION) {
        return new Response(JSON.stringify({
          protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          error: {
            code: "protocol_version_mismatch",
            message: "protocol mismatch",
            expectedProtocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          },
        } satisfies WorldHttpErrorResponse), {
          status: 409,
          headers: { "content-type": "application/json" },
        });
      }

      const message = deserializeWorldClientMessage(request.message);
      if (message.type !== "poll_world_updates") {
        throw new Error(`Expected poll_world_updates message, got ${message.type}`);
      }

      try {
        return new Response(JSON.stringify(await service.pollUpdates(decodeURIComponent(sessionPollUpdatesMatch[1]!), message)), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      } catch (error) {
        if (error instanceof Error && "code" in error && "statusCode" in error) {
          const remoteError = error as { code: WorldHttpErrorCode; statusCode: number };
          return new Response(JSON.stringify({
            protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
            error: {
              code: remoteError.code,
              message: error.message,
            },
          } satisfies WorldHttpErrorResponse), {
            status: remoteError.statusCode,
            headers: { "content-type": "application/json" },
          });
        }

        throw error;
      }
    }

    return new Response(JSON.stringify({
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      error: {
        code: "world_request_mismatch",
        message: `Unhandled route ${method} ${url.pathname}`,
      },
    } satisfies WorldHttpErrorResponse), {
      status: 404,
      headers: { "content-type": "application/json" },
    });
  };
}

function createRemoteWorldClientForTransport(transport: WorldTransport): RemoteWorldClient {
  const blocks = registerGeneratedRenderBlocks();
  const biomeSource = new OverworldBiomeSource(12345n);
  return new RemoteWorldClient(
    transport,
    (worldOpened) => new ClientChunkCache({
      airState: blocks.airState,
      minBuildHeight: worldOpened.minBuildHeight,
      height: worldOpened.height,
      biomeSource,
      biomeZoomSeed: 12345n,
      blockStateResolver: createBlockStateResolver(blocks.airState),
      blockStateIds: blocks.blockStateIds,
    }),
  );
}

function createRemoteWorldClient(baseUrl: string, fetchImpl: typeof fetch, sessionId?: string): RemoteWorldClient {
  return createRemoteWorldClientForTransport(new RemoteWorldTransport(baseUrl, { fetchImpl, sessionId }));
}

function createRemoteWorldWebSocketClient(baseUrl: string, sessionId?: string): RemoteWorldClient {
  return createRemoteWorldClientForTransport(new RemoteWorldWebSocketTransport(baseUrl, { sessionId }));
}

function findSessionState(messages: readonly WorldHostMessage[]): ClientSessionState {
  const sessionState = messages.find((message) => message.type === "session_state");
  if (sessionState?.type !== "session_state") {
    throw new Error("expected session_state message");
  }

  return sessionState.state;
}

function findSessionId(messages: readonly WorldHostMessage[]): string {
  return findSessionState(messages).sessionId;
}

function chunkSnapshots(messages: readonly WorldHostMessage[]): ChunkSnapshotMessage[] {
  return messages.filter((message): message is ChunkSnapshotMessage => message.type === "chunk_snapshot");
}

function sleep(ms = 0): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function drainRemoteClientChunks(client: RemoteWorldClient, expectedCount: number): Promise<void> {
  const deadline = Date.now() + REMOTE_WORLD_TRANSPORT_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (client.getLevel().getLoadedChunkCount() >= expectedCount) {
      return;
    }

    await sleep(10);
    await client.pollUpdates();
  }

  throw new Error(`expected ${expectedCount.toString()} loaded chunks, got ${client.getLevel().getLoadedChunkCount().toString()}`);
}

async function drainRemoteRuntimeChunks(runtime: ClientRuntime, expectedCount: number): Promise<void> {
  const level = runtime.getClientWorld().getRenderView().getRenderLevel();
  const deadline = Date.now() + REMOTE_WORLD_TRANSPORT_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (level.getLoadedChunkCount() >= expectedCount) {
      return;
    }

    await sleep(10);
    await runtime.drainTransportUpdates();
  }

  throw new Error(`expected ${expectedCount.toString()} loaded chunks, got ${level.getLoadedChunkCount().toString()}`);
}

async function waitForRuntimePlayerAck(runtime: ClientRuntime, sequence: number): Promise<void> {
  const deadline = Date.now() + REMOTE_WORLD_TRANSPORT_TIMEOUT_MS;
  while (Date.now() < deadline) {
    await sleep(10);
    await runtime.drainTransportUpdates();
    if ((runtime.publishPresentationState().localPlayerState?.acknowledgedInputSequence ?? 0) >= sequence) {
      return;
    }
  }

  throw new Error(`expected acknowledged player input sequence ${sequence.toString()}`);
}

function targetChunkEntitySnapshots(entities: readonly EntitySnapshot[]): readonly EntitySnapshot[] {
  const chunk = creatureFixture.chunks[0]!;
  return entities
    .filter((entity) => entity.chunkX === chunk.chunkX && entity.chunkZ === chunk.chunkZ)
    .sort((left, right) => left.id - right.id);
}

async function drainServiceSnapshots(
  service: GeneratedWorldRemoteService,
  sessionId: string,
  expectedCount: number,
): Promise<ChunkSnapshotMessage[]> {
  const snapshots: ChunkSnapshotMessage[] = [];
  const deadline = Date.now() + REMOTE_WORLD_TRANSPORT_TIMEOUT_MS;
  while (Date.now() < deadline) {
    await sleep(10);
    const messages = deserializeWorldHostMessages((await service.pollUpdates(sessionId, { type: "poll_world_updates" })).messages);
    snapshots.push(...chunkSnapshots(messages));
    if (snapshots.length >= expectedCount) {
      return snapshots.slice(0, expectedCount);
    }
  }

  throw new Error(`expected ${expectedCount.toString()} chunk snapshots, got ${snapshots.length.toString()}`);
}

describe("RemoteWorld transport", () => {
  afterEach(async () => {
    Registry.BLOCK.clear();

    while (REMOTE_SERVERS.length > 0) {
      await REMOTE_SERVERS.pop()!.stop();
    }

    while (TEMP_DIRECTORIES.length > 0) {
      await rm(TEMP_DIRECTORIES.pop()!, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    }
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("streams generated chunks through the remote HTTP host boundary", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });

    const client = createRemoteWorldClient("http://127.0.0.1:4173", createServiceFetch(service));
    await expect(client.openWorld(OPEN_WORLD_REQUEST)).resolves.toEqual({
      type: "world_opened",
      minBuildHeight: 0,
      height: 256,
      saveMetadata: {
        saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
        storageVersion: GENERATED_WORLD_STORAGE_VERSION,
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
    })).toBe(false);

    await drainRemoteClientChunks(client, 25);
    expect(client.getLevel().getLoadedChunkCount()).toBe(25);
    expect(client.getSessionState()).toEqual({
      sessionId: expect.any(String),
      playerId: expect.any(String),
      playerProfile: REMOTE_PLAYER_PROFILE,
      saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
      resumed: false,
      revision: 1,
      chunkView: {
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      },
    });
    expect(client.getSessionState()?.playerId).not.toBe(client.getSessionState()?.sessionId);
    expect(service.getSessionCount()).toBe(1);
    expect(service.getWorldCount()).toBe(1);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("publishes remote state through ClientRuntime and ClientWorld views", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    const runtime = new WorldClientRuntimeFacade(createRemoteWorldClient("http://127.0.0.1:4173", createServiceFetch(service)));

    await runtime.openWorld(OPEN_WORLD_REQUEST);
    expect(runtime.publishPresentationState()).toMatchObject({
      sessionState: {
        sessionId: expect.any(String),
        playerId: expect.any(String),
        playerProfile: REMOTE_PLAYER_PROFILE,
        saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
        resumed: false,
        revision: 0,
      },
      localPlayerState: {
        playerId: expect.any(String),
        revision: 0,
      },
      entities: [],
    });
    const openedPresentation = runtime.publishPresentationState();
    expect(openedPresentation.sessionState?.playerId).not.toBe(openedPresentation.sessionState?.sessionId);
    expect(openedPresentation.localPlayerState?.playerId).toBe(openedPresentation.sessionState?.playerId);

    expect(await runtime.setChunkInterest({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).toBe(false);
    await drainRemoteRuntimeChunks(runtime, 25);

    const clientWorld = runtime.getClientWorld();
    const presentation = runtime.publishPresentationState();
    expect(clientWorld.getSessionState()).toEqual(presentation.sessionState);
    expect(clientWorld.getLocalPlayerState()).toEqual(presentation.localPlayerState);
    expect(clientWorld.getEntitySnapshots()).toEqual(presentation.entities);
    expect(clientWorld.getRevisionFacts()).toMatchObject({
      sessionRevision: 1,
      localPlayerRevision: 0,
    });
    expect(clientWorld.getRenderView().getRenderLevel().getLoadedChunkCount()).toBe(25);
    expect(clientWorld.getChunkSnapshot(0, 0)).toBeDefined();

    const initialPlayerState = presentation.localPlayerState;
    expect(initialPlayerState).toBeDefined();
    expect(await runtime.sendPlayerCommand({
      type: "set_player_input",
      input: {
        sequence: 1,
        moveX: 1,
        moveY: 0,
        moveZ: 0,
        yaw: 90,
        pitch: 10,
      },
    })).toBe(true);
    service.forceTick();
    expect(await runtime.drainTransportUpdates()).toBe(true);

    const updatedPlayerState = runtime.publishPresentationState().localPlayerState;
    expect(updatedPlayerState).toBeDefined();
    expect(updatedPlayerState!.acknowledgedInputSequence).toBe(1);
    expect(updatedPlayerState!.revision).toBeGreaterThan(initialPlayerState!.revision);
    expect(clientWorld.getPredictionView()).toMatchObject({
      movementPhysicsRevision: updatedPlayerState!.movementBody?.physicsRevision,
      collisionRevision: updatedPlayerState!.movementBody?.collisionRevision,
    });
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("streams generated entity snapshots through the remote HTTP host boundary", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    const client = createRemoteWorldClient("http://127.0.0.1:4173", createServiceFetch(service));

    await client.openWorld(OPEN_CREATURE_WORLD_REQUEST);
    const chunk = creatureFixture.chunks[0]!;
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: chunk.chunkX,
      centerChunkZ: chunk.chunkZ,
      radius: 1,
    });

    const deadline = Date.now() + REMOTE_WORLD_TRANSPORT_TIMEOUT_MS;
    while (Date.now() < deadline && targetChunkEntitySnapshots(client.getEntitySnapshots()).length === 0) {
      await sleep(10);
      await client.pollUpdates();
    }

    const snapshots = targetChunkEntitySnapshots(client.getEntitySnapshots());
    expect(snapshots.length).toBeGreaterThan(0);
    expect(snapshots.every((entity) => entity.category === "creature")).toBe(true);
    expect(snapshots.every((entity) => entity.position.x >= chunk.chunkX * 16 && entity.position.x < (chunk.chunkX + 1) * 16)).toBe(true);
    expect(snapshots.every((entity) => entity.position.z >= chunk.chunkZ * 16 && entity.position.z < (chunk.chunkZ + 1) * 16)).toBe(true);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("acknowledges remote chunk interest before streaming snapshots through poll updates", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });

    const openMessages = deserializeWorldHostMessages((await service.openWorld(OPEN_WORLD_REQUEST)).messages);
    const sessionId = findSessionId(openMessages);
    const chunkViewMessages = deserializeWorldHostMessages((await service.setChunkView(sessionId, {
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).messages);
    expect(chunkViewMessages.map((message) => message.type)).toEqual(["session_state", "player_state"]);

    const inputMessages = deserializeWorldHostMessages((await service.setPlayerInput(sessionId, {
      type: "set_player_input",
      input: {
        sequence: 1,
        moveX: 1,
        moveY: 0,
        moveZ: 0,
        yaw: 90,
        pitch: 10,
      },
    })).messages);
    expect(inputMessages.map((message) => message.type)).toEqual(["session_state"]);

    const snapshots = await drainServiceSnapshots(service, sessionId, 25);
    expect(snapshots).toHaveLength(25);
    expect(snapshots.some((message) => message.snapshot.chunkX === 0 && message.snapshot.chunkZ === 0)).toBe(true);

    const nextChunkViewMessages = deserializeWorldHostMessages((await service.setChunkView(sessionId, {
      type: "set_chunk_view",
      centerChunkX: 1,
      centerChunkZ: 0,
      radius: 1,
    })).messages);
    expect(chunkSnapshots(nextChunkViewMessages)).toHaveLength(0);
    expect(nextChunkViewMessages.some((message) => message.type === "chunk_unload")).toBe(true);
    await expect(drainServiceSnapshots(service, sessionId, 5)).resolves.toHaveLength(5);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("delivers authoritative player-state updates through the remote poll path", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    const client = createRemoteWorldClient("http://127.0.0.1:4173", createServiceFetch(service));

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await drainRemoteClientChunks(client, 25);

    const initialPlayerState = client.getPlayerState();
    expect(initialPlayerState).toBeDefined();

    expect(await client.setPlayerInput({
      type: "set_player_input",
      input: {
        sequence: 1,
        moveX: 1,
        moveY: 0,
        moveZ: 0,
        yaw: 90,
        pitch: 10,
      },
    })).toBe(true);

    service.forceTick();
    expect(await client.pollUpdates()).toBe(true);

    expect(client.getPlayerState()).toMatchObject({
      playerId: expect.any(String),
      position: {
        x: expect.any(Number),
        y: expect.any(Number),
        z: expect.any(Number),
      },
      rotation: {
        yaw: 90,
        pitch: 10,
      },
      acknowledgedInputSequence: 1,
      tick: 1,
      revision: expect.any(Number),
      movementBody: {
        position: {
          x: expect.any(Number),
          y: expect.any(Number),
          z: expect.any(Number),
        },
        velocity: {
          x: expect.any(Number),
          y: expect.any(Number),
          z: expect.any(Number),
        },
        lastProcessedCommandSequence: 1,
      },
    });
    expect(client.getPlayerState()!.position).not.toEqual(initialPlayerState!.position);
    expect(client.getPlayerState()!.revision).toBeGreaterThan(initialPlayerState!.revision);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("supports two concurrent remote client sessions against one shared authoritative world", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    const fetchImpl = createServiceFetch(service);

    const firstClient = createRemoteWorldClient("http://127.0.0.1:4173", fetchImpl);
    const secondClient = createRemoteWorldClient("http://127.0.0.1:4173", fetchImpl);
    const firstRequest = {
      ...OPEN_WORLD_REQUEST,
      playerProfile: { name: "Remote Player One" },
    } as const;
    const secondRequest = {
      ...OPEN_WORLD_REQUEST,
      playerProfile: { name: "Remote Player Two" },
    } as const;
    const [firstOpened, secondOpened] = await Promise.all([
      firstClient.openWorld(firstRequest),
      secondClient.openWorld(secondRequest),
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

    await Promise.all([
      drainRemoteClientChunks(firstClient, 25),
      drainRemoteClientChunks(secondClient, 25),
    ]);
    expect(firstClient.getLevel().getLoadedChunkCount()).toBe(25);
    expect(secondClient.getLevel().getLoadedChunkCount()).toBe(25);
    expect(service.getSessionCount()).toBe(2);
    expect(service.getWorldCount()).toBe(1);
    expect(firstClient.getSessionState()?.playerId).not.toBe(firstClient.getSessionState()?.sessionId);
    expect(secondClient.getSessionState()?.playerId).not.toBe(secondClient.getSessionState()?.sessionId);
    expect(firstClient.getSessionState()?.playerId).not.toBe(secondClient.getSessionState()?.playerId);
    expect(firstClient.getSessionState()?.playerProfile).toEqual(firstRequest.playerProfile);
    expect(secondClient.getSessionState()?.playerProfile).toEqual(secondRequest.playerProfile);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("resumes an existing remote session and resyncs its visible chunks", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    const fetchImpl = createServiceFetch(service);

    const firstClient = createRemoteWorldClient("http://127.0.0.1:4173", fetchImpl);
    await firstClient.openWorld(OPEN_WORLD_REQUEST);
    await firstClient.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await drainRemoteClientChunks(firstClient, 25);

    const sessionId = firstClient.getSessionState()?.sessionId;
    const playerId = firstClient.getSessionState()?.playerId;
    expect(sessionId).toBeDefined();
    expect(playerId).toBeDefined();

    const resumedClient = createRemoteWorldClient("http://127.0.0.1:4173", fetchImpl, sessionId);
    await resumedClient.openWorld(OPEN_WORLD_REQUEST);

    expect(resumedClient.getLevel().getLoadedChunkCount()).toBe(25);
    expect(resumedClient.getSessionState()).toEqual({
      sessionId,
      playerId,
      playerProfile: REMOTE_PLAYER_PROFILE,
      saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
      resumed: true,
      revision: 1,
      chunkView: {
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      },
    });
    expect(resumedClient.getSessionState()?.playerId).not.toBe(resumedClient.getSessionState()?.sessionId);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("reopens and resyncs automatically when a remote session disappears", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    const fetchImpl = createServiceFetch(service);
    const client = createRemoteWorldClient("http://127.0.0.1:4173", fetchImpl);

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await drainRemoteClientChunks(client, 25);

    const previousSessionId = client.getSessionState()?.sessionId;
    expect(previousSessionId).toBeDefined();
    service.dropSession(previousSessionId!);

    expect(await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).toBe(true);

    await drainRemoteClientChunks(client, 25);
    expect(client.getLevel().getLoadedChunkCount()).toBe(25);
    expect(client.getSessionState()?.sessionId).not.toBe(previousSessionId);
    expect(client.getSessionState()?.playerId).not.toBe(client.getSessionState()?.sessionId);
    expect(client.getSessionState()?.resumed).toBe(false);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("restores player input and state when a polling session disappears", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    const fetchImpl = createServiceFetch(service);
    const client = createRemoteWorldClient("http://127.0.0.1:4173", fetchImpl);

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await drainRemoteClientChunks(client, 25);
    await client.setPlayerInput({
      type: "set_player_input",
      input: {
        sequence: 2,
        moveX: 0,
        moveY: 0,
        moveZ: 1,
        yaw: 180,
        pitch: 0,
      },
    });

    const previousSessionId = client.getSessionState()?.sessionId;
    expect(previousSessionId).toBeDefined();
    service.dropSession(previousSessionId!);

    expect(await client.pollUpdates()).toBe(true);
    service.forceTick();
    expect(await client.pollUpdates()).toBe(true);

    expect(client.getSessionState()?.sessionId).not.toBe(previousSessionId);
    expect(client.getSessionState()?.playerId).not.toBe(client.getSessionState()?.sessionId);
    expect(client.getPlayerState()?.acknowledgedInputSequence).toBe(2);
    expect(client.getPlayerState()?.tick).toBeGreaterThan(0);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("streams generated chunks through the remote WebSocket push channel", async () => {
    const server = await createRemoteServer();
    const client = createRemoteWorldWebSocketClient(server.getBaseUrl());

    await expect(client.openWorld(OPEN_WORLD_REQUEST)).resolves.toEqual({
      type: "world_opened",
      minBuildHeight: 0,
      height: 256,
      saveMetadata: {
        saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
        storageVersion: GENERATED_WORLD_STORAGE_VERSION,
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
    })).toBe(false);

    await drainRemoteClientChunks(client, 25);
    expect(client.getLevel().getLoadedChunkCount()).toBe(25);
    expect(client.getSessionState()).toEqual({
      sessionId: expect.any(String),
      playerId: expect.any(String),
      playerProfile: REMOTE_PLAYER_PROFILE,
      saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
      resumed: false,
      revision: 1,
      chunkView: {
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      },
    });
    expect(client.getSessionState()?.playerId).not.toBe(client.getSessionState()?.sessionId);
    expect(server.getSessionCount()).toBe(1);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("pushes WebSocket player-state updates into ClientRuntime drain queues", async () => {
    const server = await createRemoteServer();
    const runtime = new WorldClientRuntimeFacade(createRemoteWorldWebSocketClient(server.getBaseUrl()));

    await runtime.openWorld(OPEN_WORLD_REQUEST);
    expect(await runtime.setChunkInterest({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).toBe(false);
    await drainRemoteRuntimeChunks(runtime, 25);

    const initialPlayerState = runtime.publishPresentationState().localPlayerState;
    expect(initialPlayerState).toBeDefined();
    expect(await runtime.sendPlayerCommand({
      type: "set_player_input",
      input: {
        sequence: 1,
        moveX: 1,
        moveY: 0,
        moveZ: 0,
        yaw: 90,
        pitch: 10,
      },
    })).toBe(true);

    server.forceTick();
    await waitForRuntimePlayerAck(runtime, 1);

    const updatedPlayerState = runtime.publishPresentationState().localPlayerState;
    expect(updatedPlayerState).toBeDefined();
    expect(updatedPlayerState!.revision).toBeGreaterThan(initialPlayerState!.revision);
    expect(updatedPlayerState!.tick).toBeGreaterThan(initialPlayerState!.tick);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("supports two concurrent WebSocket client sessions against one shared world", async () => {
    const server = await createRemoteServer();
    const firstClient = createRemoteWorldWebSocketClient(server.getBaseUrl());
    const secondClient = createRemoteWorldWebSocketClient(server.getBaseUrl());
    const firstRequest = {
      ...OPEN_WORLD_REQUEST,
      playerProfile: { name: "Remote Player One" },
    } as const;
    const secondRequest = {
      ...OPEN_WORLD_REQUEST,
      playerProfile: { name: "Remote Player Two" },
    } as const;
    const [firstOpened, secondOpened] = await Promise.all([
      firstClient.openWorld(firstRequest),
      secondClient.openWorld(secondRequest),
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

    await Promise.all([
      drainRemoteClientChunks(firstClient, 25),
      drainRemoteClientChunks(secondClient, 25),
    ]);
    expect(firstClient.getLevel().getLoadedChunkCount()).toBe(25);
    expect(secondClient.getLevel().getLoadedChunkCount()).toBe(25);
    expect(server.getSessionCount()).toBe(2);
    expect(firstClient.getSessionState()?.playerId).not.toBe(firstClient.getSessionState()?.sessionId);
    expect(secondClient.getSessionState()?.playerId).not.toBe(secondClient.getSessionState()?.sessionId);
    expect(firstClient.getSessionState()?.playerId).not.toBe(secondClient.getSessionState()?.playerId);
    expect(firstClient.getSessionState()?.playerProfile).toEqual(firstRequest.playerProfile);
    expect(secondClient.getSessionState()?.playerProfile).toEqual(secondRequest.playerProfile);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("resumes an existing WebSocket session and resyncs visible chunks", async () => {
    const server = await createRemoteServer();
    const firstClient = createRemoteWorldWebSocketClient(server.getBaseUrl());
    await firstClient.openWorld(OPEN_WORLD_REQUEST);
    await firstClient.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await drainRemoteClientChunks(firstClient, 25);

    const sessionId = firstClient.getSessionState()?.sessionId;
    const playerId = firstClient.getSessionState()?.playerId;
    expect(sessionId).toBeDefined();
    expect(playerId).toBeDefined();
    firstClient.close();

    const resumedClient = createRemoteWorldWebSocketClient(server.getBaseUrl(), sessionId);
    await resumedClient.openWorld(OPEN_WORLD_REQUEST);

    expect(resumedClient.getLevel().getLoadedChunkCount()).toBe(25);
    expect(resumedClient.getSessionState()).toEqual({
      sessionId,
      playerId,
      playerProfile: REMOTE_PLAYER_PROFILE,
      saveId: createGeneratedWorldSaveId(12345n, "browser_smoke"),
      resumed: true,
      revision: 1,
      chunkView: {
        centerChunkX: 0,
        centerChunkZ: 0,
        radius: 1,
      },
    });
    expect(resumedClient.getSessionState()?.playerId).not.toBe(resumedClient.getSessionState()?.sessionId);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("reopens and resyncs automatically when a WebSocket session disappears", async () => {
    const server = await createRemoteServer();
    const client = createRemoteWorldWebSocketClient(server.getBaseUrl());

    await client.openWorld(OPEN_WORLD_REQUEST);
    await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    });
    await drainRemoteClientChunks(client, 25);

    const previousSessionId = client.getSessionState()?.sessionId;
    expect(previousSessionId).toBeDefined();
    server.dropSession(previousSessionId!);

    expect(await client.setChunkView({
      type: "set_chunk_view",
      centerChunkX: 0,
      centerChunkZ: 0,
      radius: 1,
    })).toBe(true);

    await drainRemoteClientChunks(client, 25);
    expect(client.getLevel().getLoadedChunkCount()).toBe(25);
    expect(client.getSessionState()?.sessionId).not.toBe(previousSessionId);
    expect(client.getSessionState()?.playerId).not.toBe(client.getSessionState()?.sessionId);
    expect(client.getSessionState()?.resumed).toBe(false);
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);

  test("close drops remote transport session handles and rejects later requests", async () => {
    const service = new GeneratedWorldRemoteService({
      saveRoot: await createTempDirectory(),
    });
    const transport = new RemoteWorldTransport("http://127.0.0.1:4173", {
      fetchImpl: createServiceFetch(service),
    });
    const runtime = new WorldClientRuntimeFacade(createRemoteWorldClientForTransport(transport));

    await runtime.openWorld(OPEN_WORLD_REQUEST);
    expect(transport.getSessionId()).toBeDefined();

    runtime.close();

    expect(transport.getSessionId()).toBeUndefined();
    await expect(runtime.drainTransportUpdates()).rejects.toThrow("RemoteWorldTransport.pollUpdates() called after close()");
  }, REMOTE_WORLD_TRANSPORT_TIMEOUT_MS);
});
