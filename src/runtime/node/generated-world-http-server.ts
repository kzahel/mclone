import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { randomUUID } from "node:crypto";
import { mkdir, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";
import { applyPackedChunkLightDeltaToSnapshot, type PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";
import { createGeneratedWorldHostForRequest } from "../host/generated-world-host-factory";
import { createGeneratedWorldSaveId, getGeneratedWorldViewChunkRadius } from "../host/generated-world-host";
import type { WorldHost } from "../protocol/world-host";
import {
  deserializeWorldClientMessage,
  serializeWorldHostMessages,
  WORLD_HTTP_PROTOCOL_VERSION,
  type OpenWorldSessionRequest,
  type OpenWorldSessionResponse,
  type SessionChunkViewRequest,
  type SessionChunkViewResponse,
  type SessionPlayerInputRequest,
  type SessionPlayerInputResponse,
  type SessionPollUpdatesRequest,
  type SessionPollUpdatesResponse,
  type WorldHttpErrorCode,
  type WorldHttpErrorResponse,
} from "../protocol/world-http-protocol";
import { drainWorldHostMessages } from "../protocol/world-message-queue";
import type {
  ClientSessionState,
  ClientPlayerState,
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetPlayerInputRequest,
  SessionChunkViewState,
  SetChunkViewRequest,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../protocol/world-messages";
import {
  anchorPlayerStateToChunkView,
  createInitialPlayerState,
  PLAYER_TICK_INTERVAL_MS,
  tickPlayerState,
} from "../session/player-loop";
import { FileWorldStorage } from "../storage/file-world-storage";

const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 4173;
const DEFAULT_SAVE_ROOT = path.resolve(tmpdir(), "mclone-node-worlds");

export interface GeneratedWorldHttpServerOptions {
  readonly host?: string;
  readonly port?: number;
  readonly saveRoot?: string;
}

export interface GeneratedWorldRemoteServiceOptions {
  readonly saveRoot?: string;
  readonly autoTick?: boolean;
}

interface GeneratedWorldHttpServerConfig {
  host: string;
  port: number;
  saveRoot: string;
}

interface ParsedCliOptions {
  configPath?: string;
  host?: string;
  port?: number;
  saveRoot?: string;
}

interface MutableGeneratedWorldHttpServerConfig {
  host?: string;
  port?: number;
  saveRoot?: string;
}

type SessionRecord = {
  readonly id: string;
  readonly playerId: string;
  readonly worldSaveId: string;
  readonly openWorldRequest: OpenWorldRequest;
  chunkView: SetChunkViewRequest | undefined;
  visibleChunks: Set<string>;
  revision: number;
  playerState: ClientPlayerState;
  playerInput: SetPlayerInputRequest["input"] | undefined;
  pendingMessages: WorldHostMessage[];
  playerAnchoredToChunkView: boolean;
};

type SharedWorldRecord = {
  readonly saveId: string;
  readonly host: WorldHost;
  readonly worldOpened: WorldOpenedMessage;
  readonly sessionIds: Set<string>;
  readonly loadedSnapshots: Map<string, PackedChunkSnapshot>;
  aggregateChunkView: SetChunkViewRequest | undefined;
  pendingOperation: Promise<void>;
  pendingHostDrain: Promise<void> | undefined;
};

class GeneratedWorldRemoteServiceError extends Error {
  public constructor(
    message: string,
    public readonly code: WorldHttpErrorCode,
    public readonly statusCode: number,
    public readonly expectedProtocolVersion?: number,
  ) {
    super(message);
  }
}

function formatUnknownError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function parsePort(value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > 65_535) {
    throw new Error(`Expected port to be an integer between 0 and 65535, got ${value}`);
  }

  return parsed;
}

function chunkKey(chunkX: number, chunkZ: number): string {
  return `${chunkX},${chunkZ}`;
}

function parseChunkKey(key: string): { readonly chunkX: number; readonly chunkZ: number } {
  const [chunkX, chunkZ] = key.split(",", 2);
  if (chunkX === undefined || chunkZ === undefined) {
    throw new Error(`Invalid chunk key ${key}`);
  }

  return {
    chunkX: Number.parseInt(chunkX, 10),
    chunkZ: Number.parseInt(chunkZ, 10),
  };
}

function createChunkUnloadMessage(key: string): Extract<WorldHostMessage, { type: "chunk_unload" }> {
  const { chunkX, chunkZ } = parseChunkKey(key);
  return {
    type: "chunk_unload",
    chunkX,
    chunkZ,
  };
}

function isSameChunkView(left: SetChunkViewRequest | undefined, right: SetChunkViewRequest | undefined): boolean {
  if (left === undefined || right === undefined) {
    return left === right;
  }

  return left.centerChunkX === right.centerChunkX
    && left.centerChunkZ === right.centerChunkZ
    && left.radius === right.radius;
}

function createSessionChunkViewState(chunkView: SetChunkViewRequest | undefined): SessionChunkViewState | undefined {
  if (chunkView === undefined) {
    return undefined;
  }

  return {
    centerChunkX: chunkView.centerChunkX,
    centerChunkZ: chunkView.centerChunkZ,
    radius: chunkView.radius,
  };
}

function createSessionStateMessage(
  session: SessionRecord,
  world: SharedWorldRecord,
  resumed: boolean,
): Extract<WorldHostMessage, { type: "session_state" }> {
  const state: ClientSessionState = {
    sessionId: session.id,
    playerId: session.playerId,
    saveId: world.saveId,
    resumed,
    revision: session.revision,
    chunkView: createSessionChunkViewState(session.chunkView),
  };
  return {
    type: "session_state",
    state,
  };
}

function createPlayerStateMessage(session: SessionRecord): Extract<WorldHostMessage, { type: "player_state" }> {
  return {
    type: "player_state",
    state: session.playerState,
  };
}

function enqueuePlayerStateMessage(session: SessionRecord): void {
  session.pendingMessages = session.pendingMessages.filter((message) => message.type !== "player_state");
  session.pendingMessages.push(createPlayerStateMessage(session));
}

function drainPendingMessages(session: SessionRecord, maxMessages: number | undefined): readonly WorldHostMessage[] {
  const drained = drainWorldHostMessages(session.pendingMessages, maxMessages);
  session.pendingMessages = drained.remaining;
  return drained.messages;
}

function extractWorldOpened(messages: readonly WorldHostMessage[]): WorldOpenedMessage {
  const opened = messages.find((message) => message.type === "world_opened");
  if (opened === undefined) {
    throw new Error("GeneratedWorld host did not return world_opened");
  }

  return opened;
}

function applyAuthoritativeMessages(world: SharedWorldRecord, messages: readonly WorldHostMessage[]): void {
  for (const message of messages) {
    switch (message.type) {
      case "world_opened":
      case "session_state":
      case "player_state":
        break;
      case "chunk_snapshot":
        world.loadedSnapshots.set(chunkKey(message.snapshot.chunkX, message.snapshot.chunkZ), message.snapshot);
        break;
      case "chunk_light_delta": {
        const key = chunkKey(message.chunkX, message.chunkZ);
        const snapshot = world.loadedSnapshots.get(key);
        if (snapshot !== undefined) {
          world.loadedSnapshots.set(key, applyPackedChunkLightDeltaToSnapshot(snapshot, message.light));
        }
        break;
      }
      case "chunk_unload":
        world.loadedSnapshots.delete(chunkKey(message.chunkX, message.chunkZ));
        break;
      case "world_error":
        throw new Error(message.message);
    }
  }
}

function getSessionVisibleChunks(chunkView: SetChunkViewRequest | undefined): Set<string> {
  const keys = new Set<string>();
  if (chunkView === undefined) {
    return keys;
  }

  const viewRadius = getGeneratedWorldViewChunkRadius(chunkView.radius);
  for (let chunkZ = chunkView.centerChunkZ - viewRadius; chunkZ <= chunkView.centerChunkZ + viewRadius; chunkZ++) {
    for (let chunkX = chunkView.centerChunkX - viewRadius; chunkX <= chunkView.centerChunkX + viewRadius; chunkX++) {
      keys.add(chunkKey(chunkX, chunkZ));
    }
  }

  return keys;
}

function computeAggregateChunkView(sessions: Iterable<SessionRecord>): SetChunkViewRequest | undefined {
  let minChunkX = Number.POSITIVE_INFINITY;
  let maxChunkX = Number.NEGATIVE_INFINITY;
  let minChunkZ = Number.POSITIVE_INFINITY;
  let maxChunkZ = Number.NEGATIVE_INFINITY;
  let hasChunkView = false;

  for (const session of sessions) {
    if (session.chunkView === undefined) {
      continue;
    }

    hasChunkView = true;
    const viewRadius = getGeneratedWorldViewChunkRadius(session.chunkView.radius);
    minChunkX = Math.min(minChunkX, session.chunkView.centerChunkX - viewRadius);
    maxChunkX = Math.max(maxChunkX, session.chunkView.centerChunkX + viewRadius);
    minChunkZ = Math.min(minChunkZ, session.chunkView.centerChunkZ - viewRadius);
    maxChunkZ = Math.max(maxChunkZ, session.chunkView.centerChunkZ + viewRadius);
  }

  if (!hasChunkView) {
    return undefined;
  }

  const centerChunkX = Math.floor((minChunkX + maxChunkX) / 2);
  const centerChunkZ = Math.floor((minChunkZ + maxChunkZ) / 2);
  const requiredViewRadius = Math.max(
    centerChunkX - minChunkX,
    maxChunkX - centerChunkX,
    centerChunkZ - minChunkZ,
    maxChunkZ - centerChunkZ,
  );
  return {
    type: "set_chunk_view",
    centerChunkX,
    centerChunkZ,
    radius: Math.max(0, requiredViewRadius - 1),
  };
}

async function readRequestBody(request: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    chunks.push(typeof chunk === "string" ? Buffer.from(chunk) : chunk);
  }

  return Buffer.concat(chunks).toString("utf8");
}

async function readJsonBody<T>(request: IncomingMessage): Promise<T> {
  const body = await readRequestBody(request);
  if (body.length === 0) {
    throw new Error("Expected JSON request body");
  }

  return JSON.parse(body) as T;
}

function sendJson(response: ServerResponse, statusCode: number, body: unknown): void {
  response.statusCode = statusCode;
  response.setHeader("access-control-allow-origin", "*");
  response.setHeader("access-control-allow-methods", "GET,POST,OPTIONS");
  response.setHeader("access-control-allow-headers", "content-type");
  response.setHeader("content-type", "application/json");
  response.end(`${JSON.stringify(body)}\n`);
}

function setCorsHeaders(response: ServerResponse): void {
  response.setHeader("access-control-allow-origin", "*");
  response.setHeader("access-control-allow-methods", "GET,POST,OPTIONS");
  response.setHeader("access-control-allow-headers", "content-type");
}

async function readConfigFile(configPath: string): Promise<MutableGeneratedWorldHttpServerConfig> {
  const fileUrl = path.resolve(configPath);
  const value = JSON.parse(await readFile(fileUrl, "utf8")) as unknown;
  if (typeof value !== "object" || value === null) {
    throw new Error(`Generated world HTTP server config ${fileUrl} must contain an object`);
  }

  const record = value as Record<string, unknown>;
  const parsed: MutableGeneratedWorldHttpServerConfig = {};

  if (record.host !== undefined) {
    if (typeof record.host !== "string") {
      throw new Error(`Generated world HTTP server config ${fileUrl} field host must be a string`);
    }

    parsed.host = record.host;
  }

  if (record.port !== undefined) {
    if (typeof record.port !== "number" || !Number.isSafeInteger(record.port)) {
      throw new Error(`Generated world HTTP server config ${fileUrl} field port must be an integer`);
    }

    parsed.port = record.port;
  }

  if (record.saveRoot !== undefined) {
    if (typeof record.saveRoot !== "string") {
      throw new Error(`Generated world HTTP server config ${fileUrl} field saveRoot must be a string`);
    }

    parsed.saveRoot = path.isAbsolute(record.saveRoot)
      ? record.saveRoot
      : path.resolve(path.dirname(fileUrl), record.saveRoot);
  }

  return parsed;
}

function parseCliOptions(argv: readonly string[]): ParsedCliOptions {
  const parsed: ParsedCliOptions = {};

  for (let index = 0; index < argv.length; index++) {
    const option = argv[index];
    if (option === undefined || option === "--") {
      continue;
    }

    const value = argv[index + 1];
    if (!option.startsWith("--")) {
      throw new Error(`Unexpected argument ${option}`);
    }

    if (value === undefined) {
      throw new Error(`Missing value for ${option}`);
    }

    switch (option) {
      case "--config":
        parsed.configPath = value;
        break;
      case "--host":
        parsed.host = value;
        break;
      case "--port":
        parsed.port = parsePort(value);
        break;
      case "--save-root":
        parsed.saveRoot = path.resolve(value);
        break;
      default:
        throw new Error(`Unknown option ${option}`);
    }

    index++;
  }

  return parsed;
}

async function loadConfig(argv: readonly string[]): Promise<GeneratedWorldHttpServerConfig> {
  const cli = parseCliOptions(argv);
  const fileConfig = cli.configPath === undefined ? {} : await readConfigFile(cli.configPath);
  return {
    host: cli.host ?? fileConfig.host ?? DEFAULT_HOST,
    port: cli.port ?? fileConfig.port ?? DEFAULT_PORT,
    saveRoot: cli.saveRoot ?? fileConfig.saveRoot ?? DEFAULT_SAVE_ROOT,
  };
}

export class GeneratedWorldRemoteService {
  private readonly worldStorage: FileWorldStorage;
  private readonly worlds = new Map<string, SharedWorldRecord>();
  private readonly pendingWorlds = new Map<string, Promise<SharedWorldRecord>>();
  private readonly sessions = new Map<string, SessionRecord>();
  private readonly autoTick: boolean;
  private readonly tickTimer: ReturnType<typeof setInterval> | undefined;
  private currentTick = 0;

  public constructor(options: GeneratedWorldRemoteServiceOptions = {}) {
    this.worldStorage = new FileWorldStorage(path.resolve(options.saveRoot ?? DEFAULT_SAVE_ROOT));
    this.autoTick = options.autoTick ?? false;
    this.tickTimer = this.autoTick
      ? setInterval(() => {
        this.forceTick();
      }, PLAYER_TICK_INTERVAL_MS)
      : undefined;
    this.tickTimer?.unref?.();
  }

  public async openWorld(request: OpenWorldRequest, resumeSessionId?: string): Promise<OpenWorldSessionResponse> {
    if (resumeSessionId !== undefined) {
      return await this.resumeWorld(request, resumeSessionId);
    }

    const world = await this.getOrCreateWorld(request);
    const sessionId = randomUUID();
    const session: SessionRecord = {
      id: sessionId,
      playerId: sessionId,
      worldSaveId: world.saveId,
      openWorldRequest: request,
      chunkView: undefined,
      visibleChunks: new Set<string>(),
      revision: 0,
      playerState: createInitialPlayerState(sessionId),
      playerInput: undefined,
      pendingMessages: [],
      playerAnchoredToChunkView: false,
    };
    this.sessions.set(sessionId, session);
    world.sessionIds.add(sessionId);
    return {
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages([
        world.worldOpened,
        createSessionStateMessage(session, world, false),
        createPlayerStateMessage(session),
      ]),
    };
  }

  public async setChunkView(sessionId: string, request: SetChunkViewRequest): Promise<SessionChunkViewResponse> {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world session ${sessionId}`, "unknown_session", 404);
    }

    const world = this.worlds.get(session.worldSaveId);
    if (world === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world ${session.worldSaveId}`, "unknown_session", 404);
    }

    return await this.runWorldOperation(world, async () => {
      const queuedSession = this.sessions.get(sessionId);
      if (queuedSession === undefined) {
        throw new GeneratedWorldRemoteServiceError(`Unknown world session ${sessionId}`, "unknown_session", 404);
      }

      const queuedWorld = this.worlds.get(queuedSession.worldSaveId);
      if (queuedWorld === undefined) {
        throw new GeneratedWorldRemoteServiceError(`Unknown world ${queuedSession.worldSaveId}`, "unknown_session", 404);
      }

      const previousChunkView = queuedSession.chunkView;
      const previousVisibleChunks = new Set(queuedSession.visibleChunks);
      const chunkViewChanged = !isSameChunkView(previousChunkView, request);
      if (chunkViewChanged) {
        queuedSession.chunkView = request;
        queuedSession.revision++;
      }

      const aggregateChunkView = computeAggregateChunkView(this.getWorldSessions(queuedWorld));
      const aggregateChunkViewChanged = !isSameChunkView(queuedWorld.aggregateChunkView, aggregateChunkView);
      if (aggregateChunkViewChanged && aggregateChunkView !== undefined) {
        // Shared-host runtime: serialize stateful host mutations per world save.
        applyAuthoritativeMessages(queuedWorld, await queuedWorld.host.setChunkView(aggregateChunkView));
        queuedWorld.aggregateChunkView = aggregateChunkView;
      } else if (aggregateChunkViewChanged) {
        queuedWorld.aggregateChunkView = undefined;
      }

      const visibleChunks = getSessionVisibleChunks(queuedSession.chunkView);
      const responseMessages: WorldHostMessage[] = [createSessionStateMessage(queuedSession, queuedWorld, false)];
      if (!queuedSession.playerAnchoredToChunkView) {
        queuedSession.playerState = anchorPlayerStateToChunkView(
          queuedSession.playerState,
          createSessionChunkViewState(request)!,
          queuedSession.playerState.tick,
        );
        queuedSession.playerAnchoredToChunkView = true;
        responseMessages.push(createPlayerStateMessage(queuedSession));
      }

      for (const key of previousVisibleChunks) {
        if (!visibleChunks.has(key)) {
          responseMessages.push(createChunkUnloadMessage(key));
        }
      }

      if (chunkViewChanged || aggregateChunkViewChanged) {
        for (const key of visibleChunks) {
          if (previousVisibleChunks.has(key)) {
            continue;
          }

          const snapshot = queuedWorld.loadedSnapshots.get(key);
          if (snapshot !== undefined) {
            queuedSession.pendingMessages.push({
              type: "chunk_snapshot",
              snapshot,
            });
          }
        }
      }

      queuedSession.visibleChunks = visibleChunks;
      return {
        protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
        messages: serializeWorldHostMessages(responseMessages),
      };
    });
  }

  public async setPlayerInput(sessionId: string, request: SetPlayerInputRequest): Promise<SessionPlayerInputResponse> {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world session ${sessionId}`, "unknown_session", 404);
    }

    const world = this.worlds.get(session.worldSaveId);
    if (world === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world ${session.worldSaveId}`, "unknown_session", 404);
    }

    session.playerInput = request.input;
    session.revision++;
    return {
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages([
        createSessionStateMessage(session, world, false),
      ]),
    };
  }

  public async pollUpdates(sessionId: string, request: PollWorldUpdatesRequest): Promise<SessionPollUpdatesResponse> {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world session ${sessionId}`, "unknown_session", 404);
    }

    const world = this.worlds.get(session.worldSaveId);
    if (world === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world ${session.worldSaveId}`, "unknown_session", 404);
    }

    await this.drainAuthoritativeHostMessages(world);
    return {
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages(drainPendingMessages(session, request.maxMessages)),
    };
  }

  public getSessionCount(): number {
    return this.sessions.size;
  }

  public getWorldCount(): number {
    return this.worlds.size;
  }

  public forceTick(tickCount = 1): void {
    for (let tickIndex = 0; tickIndex < tickCount; tickIndex++) {
      this.currentTick++;
      for (const session of this.sessions.values()) {
        const nextPlayerState = tickPlayerState(session.playerState, session.playerInput, this.currentTick);
        if (nextPlayerState !== session.playerState) {
          session.playerState = nextPlayerState;
          enqueuePlayerStateMessage(session);
        }
      }
    }
  }

  public dropSession(sessionId: string): void {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      return;
    }

    this.sessions.delete(sessionId);
    this.worlds.get(session.worldSaveId)?.sessionIds.delete(sessionId);
  }

  public clearSessions(): void {
    this.sessions.clear();
    for (const world of this.worlds.values()) {
      world.sessionIds.clear();
      world.aggregateChunkView = undefined;
    }
  }

  public dispose(): void {
    if (this.tickTimer !== undefined) {
      clearInterval(this.tickTimer);
    }
  }

  private async resumeWorld(request: OpenWorldRequest, sessionId: string): Promise<OpenWorldSessionResponse> {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world session ${sessionId}`, "unknown_session", 404);
    }

    if (session.openWorldRequest.seed !== request.seed || session.openWorldRequest.preset !== request.preset) {
      throw new GeneratedWorldRemoteServiceError(
        `resume_session request did not match the original world request for session ${sessionId}`,
        "world_request_mismatch",
        409,
      );
    }

    const world = this.worlds.get(session.worldSaveId);
    if (world === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world ${session.worldSaveId}`, "unknown_session", 404);
    }

    const messages: WorldHostMessage[] = [
      world.worldOpened,
      createSessionStateMessage(session, world, true),
      createPlayerStateMessage(session),
    ];
    for (const key of session.visibleChunks) {
      const snapshot = world.loadedSnapshots.get(key);
      if (snapshot !== undefined) {
        messages.push({
          type: "chunk_snapshot",
          snapshot,
        });
      }
    }

    return {
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages(messages),
    };
  }

  private async getOrCreateWorld(request: OpenWorldRequest): Promise<SharedWorldRecord> {
    const saveId = createGeneratedWorldSaveId(request.seed, request.preset);
    const existing = this.worlds.get(saveId);
    if (existing !== undefined) {
      return existing;
    }

    const pending = this.pendingWorlds.get(saveId);
    if (pending !== undefined) {
      return await pending;
    }

    const creation = (async () => {
      const host = this.createSessionHost(request);
      const messages = await host.openWorld(request);
      const world: SharedWorldRecord = {
        saveId,
        host,
        worldOpened: extractWorldOpened(messages),
        sessionIds: new Set<string>(),
        loadedSnapshots: new Map<string, PackedChunkSnapshot>(),
        aggregateChunkView: undefined,
        pendingOperation: Promise.resolve(),
        pendingHostDrain: undefined,
      };
      applyAuthoritativeMessages(world, messages);
      this.worlds.set(saveId, world);
      return world;
    })();
    this.pendingWorlds.set(saveId, creation);

    try {
      return await creation;
    } finally {
      this.pendingWorlds.delete(saveId);
    }
  }

  private *getWorldSessions(world: SharedWorldRecord): Iterable<SessionRecord> {
    for (const sessionId of world.sessionIds) {
      const session = this.sessions.get(sessionId);
      if (session !== undefined) {
        yield session;
      }
    }
  }

  private async runWorldOperation<T>(world: SharedWorldRecord, operation: () => Promise<T>): Promise<T> {
    const previousOperation = world.pendingOperation;
    let resolveCurrentOperation!: () => void;
    world.pendingOperation = new Promise<void>((resolve) => {
      resolveCurrentOperation = resolve;
    });

    await previousOperation;
    try {
      return await operation();
    } finally {
      resolveCurrentOperation();
    }
  }

  private async drainAuthoritativeHostMessages(world: SharedWorldRecord): Promise<void> {
    if (world.pendingHostDrain !== undefined) {
      await world.pendingHostDrain;
      return;
    }

    const drain = (async () => {
      const messages = await world.host.pollUpdates({ type: "poll_world_updates" });
      this.publishAuthoritativeHostMessages(world, messages);
    })();
    world.pendingHostDrain = drain;
    try {
      await drain;
    } finally {
      if (world.pendingHostDrain === drain) {
        world.pendingHostDrain = undefined;
      }
    }
  }

  private publishAuthoritativeHostMessages(world: SharedWorldRecord, messages: readonly WorldHostMessage[]): void {
    for (const message of messages) {
      switch (message.type) {
        case "world_opened":
        case "session_state":
        case "player_state":
          break;
        case "chunk_snapshot": {
          const key = chunkKey(message.snapshot.chunkX, message.snapshot.chunkZ);
          world.loadedSnapshots.set(key, message.snapshot);
          for (const session of this.getWorldSessions(world)) {
            if (session.visibleChunks.has(key)) {
              session.pendingMessages.push(message);
            }
          }
          break;
        }
        case "chunk_light_delta": {
          const key = chunkKey(message.chunkX, message.chunkZ);
          const snapshot = world.loadedSnapshots.get(key);
          if (snapshot !== undefined) {
            world.loadedSnapshots.set(key, applyPackedChunkLightDeltaToSnapshot(snapshot, message.light));
          }
          for (const session of this.getWorldSessions(world)) {
            if (session.visibleChunks.has(key)) {
              session.pendingMessages.push(message);
            }
          }
          break;
        }
        case "chunk_unload": {
          const key = chunkKey(message.chunkX, message.chunkZ);
          world.loadedSnapshots.delete(key);
          for (const session of this.getWorldSessions(world)) {
            if (session.visibleChunks.has(key)) {
              session.pendingMessages.push(message);
            }
          }
          break;
        }
        case "world_error":
          for (const session of this.getWorldSessions(world)) {
            session.pendingMessages.push(message);
          }
          break;
      }
    }
  }

  private createSessionHost(request: OpenWorldRequest): WorldHost {
    return createGeneratedWorldHostForRequest(request, {
      chunkViewScheduling: "cooperative",
      worldStorage: this.worldStorage,
    });
  }
}

export class GeneratedWorldHttpServer {
  private readonly config: GeneratedWorldHttpServerConfig;
  private readonly service: GeneratedWorldRemoteService;
  private readonly server: Server;

  public constructor(options: GeneratedWorldHttpServerOptions = {}) {
    this.config = {
      host: options.host ?? DEFAULT_HOST,
      port: options.port ?? DEFAULT_PORT,
      saveRoot: path.resolve(options.saveRoot ?? DEFAULT_SAVE_ROOT),
    };
    this.service = new GeneratedWorldRemoteService({
      saveRoot: this.config.saveRoot,
      autoTick: true,
    });
    this.server = createServer((request, response) => {
      void this.handleRequest(request, response);
    });
  }

  public async start(): Promise<void> {
    await mkdir(this.config.saveRoot, { recursive: true });
    await new Promise<void>((resolve, reject) => {
      this.server.once("error", reject);
      this.server.listen(this.config.port, this.config.host, () => {
        this.server.off("error", reject);
        resolve();
      });
    });
  }

  public async stop(): Promise<void> {
    this.service.dispose();
    this.service.clearSessions();
    if (!this.server.listening) {
      return;
    }

    await new Promise<void>((resolve, reject) => {
      this.server.close((error) => {
        if (error) {
          reject(error);
          return;
        }

        resolve();
      });
      this.server.closeAllConnections();
    });
  }

  public getBaseUrl(): string {
    const address = this.server.address();
    if (address === null || typeof address === "string") {
      throw new Error("GeneratedWorldHttpServer has no bound TCP address");
    }

    return `http://${address.address}:${address.port.toString()}`;
  }

  public getSessionCount(): number {
    return this.service.getSessionCount();
  }

  private async handleRequest(request: IncomingMessage, response: ServerResponse): Promise<void> {
    try {
      setCorsHeaders(response);
      if (request.method === "OPTIONS") {
        response.statusCode = 204;
        response.end();
        return;
      }

      const url = new URL(request.url ?? "/", "http://mclone.invalid");
      if (request.method === "GET" && url.pathname === "/healthz") {
        sendJson(response, 200, {
          ok: true,
          protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          sessionCount: this.service.getSessionCount(),
          worldCount: this.service.getWorldCount(),
        });
        return;
      }

      if (request.method === "POST" && url.pathname === "/api/world/session") {
        const body = await readJsonBody<OpenWorldSessionRequest>(request);
        this.assertProtocolVersion(body.protocolVersion);
        const openWorldRequest = deserializeWorldClientMessage(body.message);
        if (openWorldRequest.type !== "open_world") {
          throw new Error(`Expected open_world message, got ${openWorldRequest.type}`);
        }

        sendJson(response, 200, await this.service.openWorld(openWorldRequest, body.resumeSessionId));
        return;
      }

      const sessionChunkMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/chunk-view$/);
      if (request.method === "POST" && sessionChunkMatch !== null) {
        const sessionId = decodeURIComponent(sessionChunkMatch[1]!);
        const body = await readJsonBody<SessionChunkViewRequest>(request);
        this.assertProtocolVersion(body.protocolVersion);
        const chunkViewRequest = deserializeWorldClientMessage(body.message);
        if (chunkViewRequest.type !== "set_chunk_view") {
          throw new Error(`Expected set_chunk_view message, got ${chunkViewRequest.type}`);
        }

        sendJson(response, 200, await this.service.setChunkView(sessionId, chunkViewRequest));
        return;
      }

      const sessionPlayerInputMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/player-input$/);
      if (request.method === "POST" && sessionPlayerInputMatch !== null) {
        const sessionId = decodeURIComponent(sessionPlayerInputMatch[1]!);
        const body = await readJsonBody<SessionPlayerInputRequest>(request);
        this.assertProtocolVersion(body.protocolVersion);
        const playerInputRequest = deserializeWorldClientMessage(body.message);
        if (playerInputRequest.type !== "set_player_input") {
          throw new Error(`Expected set_player_input message, got ${playerInputRequest.type}`);
        }

        sendJson(response, 200, await this.service.setPlayerInput(sessionId, playerInputRequest));
        return;
      }

      const sessionPollUpdatesMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/updates\/poll$/);
      if (request.method === "POST" && sessionPollUpdatesMatch !== null) {
        const sessionId = decodeURIComponent(sessionPollUpdatesMatch[1]!);
        const body = await readJsonBody<SessionPollUpdatesRequest>(request);
        this.assertProtocolVersion(body.protocolVersion);
        const pollUpdatesRequest = deserializeWorldClientMessage(body.message);
        if (pollUpdatesRequest.type !== "poll_world_updates") {
          throw new Error(`Expected poll_world_updates message, got ${pollUpdatesRequest.type}`);
        }

        sendJson(response, 200, await this.service.pollUpdates(sessionId, pollUpdatesRequest));
        return;
      }

      sendJson(response, 404, {
        protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
        error: {
          code: "world_request_mismatch",
          message: `Unhandled route ${request.method ?? "UNKNOWN"} ${url.pathname}`,
        },
      } satisfies WorldHttpErrorResponse);
    } catch (error) {
      if (error instanceof GeneratedWorldRemoteServiceError) {
        sendJson(response, error.statusCode, {
          protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          error: {
            code: error.code,
            message: error.message,
            expectedProtocolVersion: error.expectedProtocolVersion,
          },
        } satisfies WorldHttpErrorResponse);
        return;
      }

      if (error instanceof Error && error.message.startsWith("protocolVersion")) {
        sendJson(response, 409, {
          protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          error: {
            code: "protocol_version_mismatch",
            message: error.message,
            expectedProtocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
          },
        } satisfies WorldHttpErrorResponse);
        return;
      }

      sendJson(response, 500, {
        protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
        error: {
          code: "world_request_mismatch",
          message: formatUnknownError(error),
        },
      } satisfies WorldHttpErrorResponse);
    }
  }

  private assertProtocolVersion(protocolVersion: number): void {
    if (protocolVersion !== WORLD_HTTP_PROTOCOL_VERSION) {
      throw new Error(
        `protocolVersion ${protocolVersion.toString()} did not match server protocol version ${WORLD_HTTP_PROTOCOL_VERSION.toString()}`,
      );
    }
  }
}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<void> {
  const config = await loadConfig(argv);
  const server = new GeneratedWorldHttpServer(config);
  await server.start();
  process.stdout.write(`${JSON.stringify({
    type: "listening",
    url: server.getBaseUrl(),
    saveRoot: path.resolve(config.saveRoot),
    protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
  })}\n`);
}

function isDirectExecution(): boolean {
  return process.argv[1] !== undefined && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
}

if (isDirectExecution()) {
  void main().catch((error) => {
    process.stderr.write(`${formatUnknownError(error)}\n`);
    process.exitCode = 1;
  });
}
