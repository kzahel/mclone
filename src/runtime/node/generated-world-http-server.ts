import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import type { Duplex } from "node:stream";
import { pathToFileURL } from "node:url";
import { SectionPos } from "../../core/section-pos";
import type { AABB } from "../../world/phys/aabb";
import { applyPackedChunkLightDeltaToSnapshot, type PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";
import { ClientChunkCache } from "../../world/level/client-chunk-cache";
import { createBlockStateResolver } from "../../world/level/chunk-snapshot";
import { registerGeneratedRenderBlocks, type GeneratedRenderBlockPalette } from "../../world/level/generated-render-blocks";
import { OverworldBiomeSource } from "../../worldgen/biome/overworld-biome-source";
import { createGeneratedWorldHostForRequest } from "../host/generated-world-host-factory";
import { createNodeLightingService } from "../lighting/node-lighting-worker-client";
import { createGeneratedWorldSaveId, getGeneratedWorldViewChunkRadius } from "../host/generated-world-host";
import type { WorldHost } from "../protocol/world-host";
import {
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
import {
  deserializeWorldClientMessage,
  serializeWorldHostMessages,
  WORLD_REMOTE_PROTOCOL_VERSION,
  type WorldRemoteErrorCode,
  type WorldSocketClientFrame,
  type WorldSocketServerFrame,
} from "../protocol/world-wire-protocol";
import { drainWorldHostMessages } from "../protocol/world-message-queue";
import {
  isDefaultWorldEngineConfig,
  normalizePlayerProfile,
  normalizeWorldEngineConfig,
  PLAYER_ENTITY_DATA_KIND,
  PLAYER_ENTITY_TYPE_ID,
  type ClientSessionState,
  type EntitySnapshot,
  type EntityUpdate,
  type ClientPlayerState,
  type OpenWorldRequest,
  type PollWorldUpdatesRequest,
  type PlayerProfile,
  type SetPlayerInputRequest,
  type SessionChunkViewState,
  type SetChunkViewRequest,
  type WorldHostMessage,
  type WorldOpenedMessage,
  type WorldStorageMode,
} from "../protocol/world-messages";
import {
  anchorPlayerStateToChunkView,
  createPlayerCommandQueue,
  createInitialPlayerState,
  enqueuePlayerInputCommand,
  PLAYER_TICK_INTERVAL_MS,
  tickPlayerStateWithCommandQueue,
  type PlayerCommandQueue,
} from "../session/player-loop";
import { blockGetterCollisionWorld, type CollisionWorld } from "../movement/collision-world";
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
  pendingMessages: WorldHostMessage[];
};

type PlayerSlotRecord = {
  readonly id: string;
  readonly entityId: number;
  readonly profile: PlayerProfile;
  state: ClientPlayerState;
  commandQueue: PlayerCommandQueue;
  anchoredToChunkView: boolean;
};

type SharedWorldRecord = {
  readonly saveId: string;
  readonly host: WorldHost;
  readonly worldOpened: WorldOpenedMessage;
  readonly collisionLevel: ClientChunkCache;
  readonly sessionIds: Set<string>;
  readonly playerSlots: Map<string, PlayerSlotRecord>;
  readonly loadedSnapshots: Map<string, PackedChunkSnapshot>;
  readonly loadedEntitySnapshots: Map<number, EntitySnapshot>;
  nextPlayerEntityId: number;
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

function entitySnapshotChunkKey(entity: EntitySnapshot): string {
  return chunkKey(entity.chunkX, entity.chunkZ);
}

function applyEntityUpdateToSnapshot(entity: EntitySnapshot, update: EntityUpdate): EntitySnapshot {
  const position = update.position ?? entity.position;
  return {
    ...entity,
    chunkX: update.chunkX ?? (update.position === undefined ? entity.chunkX : SectionPos.blockToSectionCoord(position.x)),
    chunkZ: update.chunkZ ?? (update.position === undefined ? entity.chunkZ : SectionPos.blockToSectionCoord(position.z)),
    position,
    rotation: update.rotation ?? entity.rotation,
    width: update.width ?? entity.width,
    height: update.height ?? entity.height,
    onGround: update.onGround ?? entity.onGround,
    age: update.age ?? entity.age,
    data: update.data ?? entity.data,
  };
}

function createEntityUpdateMessage(
  entity: EntitySnapshot,
): Extract<WorldHostMessage, { type: "entity_update" }> {
  return {
    type: "entity_update",
    update: {
      id: entity.id,
      chunkX: entity.chunkX,
      chunkZ: entity.chunkZ,
      position: entity.position,
      rotation: entity.rotation,
      width: entity.width,
      height: entity.height,
      onGround: entity.onGround,
      age: entity.age,
      data: entity.data,
    },
  };
}

function createEntityRemoveMessage(
  entity: EntitySnapshot,
  reason: string,
): Extract<WorldHostMessage, { type: "entity_remove" }> {
  return {
    type: "entity_remove",
    entityId: entity.id,
    uuid: entity.uuid,
    reason,
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

function normalizeWorldStorageMode(storageMode: WorldStorageMode | undefined): WorldStorageMode {
  return storageMode === "none" ? "none" : "default";
}

function isSameOpenWorldRequest(left: OpenWorldRequest, right: OpenWorldRequest): boolean {
  if (left.seed !== right.seed || left.preset !== right.preset) {
    return false;
  }

  if (normalizeWorldStorageMode(left.storageMode) !== normalizeWorldStorageMode(right.storageMode)) {
    return false;
  }

  if (isDefaultWorldEngineConfig(left.config) && isDefaultWorldEngineConfig(right.config)) {
    return true;
  }

  const leftConfig = normalizeWorldEngineConfig(left.config);
  const rightConfig = normalizeWorldEngineConfig(right.config);
  return leftConfig.lightingMode === rightConfig.lightingMode
    && leftConfig.liquidSimulationMode === rightConfig.liquidSimulationMode;
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

function createRemoteCollisionLevel(
  request: OpenWorldRequest,
  worldOpened: WorldOpenedMessage,
  generatedBlocks: GeneratedRenderBlockPalette,
): ClientChunkCache {
  return new ClientChunkCache({
    airState: generatedBlocks.airState,
    minBuildHeight: worldOpened.minBuildHeight,
    height: worldOpened.height,
    biomeSource: new OverworldBiomeSource(request.seed),
    biomeZoomSeed: request.seed,
    blockStateResolver: createBlockStateResolver(generatedBlocks.airState),
    blockStateIds: generatedBlocks.blockStateIds,
  });
}

function createSharedWorldCollisionWorld(world: SharedWorldRecord): CollisionWorld {
  const loadedWorld = blockGetterCollisionWorld(world.collisionLevel);
  return {
    queryBlockCollisions(bounds: AABB) {
      const epsilon = 1.0e-7;
      const minChunkX = SectionPos.blockToSectionCoord(Math.floor(bounds.minX - epsilon));
      const maxChunkX = SectionPos.blockToSectionCoord(Math.floor(bounds.maxX + epsilon));
      const minChunkZ = SectionPos.blockToSectionCoord(Math.floor(bounds.minZ - epsilon));
      const maxChunkZ = SectionPos.blockToSectionCoord(Math.floor(bounds.maxZ + epsilon));
      for (let chunkZ = minChunkZ; chunkZ <= maxChunkZ; chunkZ++) {
        for (let chunkX = minChunkX; chunkX <= maxChunkX; chunkX++) {
          if (!world.loadedSnapshots.has(chunkKey(chunkX, chunkZ))) {
            return {
              type: "missing",
              reason: `missing_snapshot_chunk:${chunkX.toString()},${chunkZ.toString()}`,
            };
          }
        }
      }

      return loadedWorld.queryBlockCollisions(bounds);
    },
  };
}

function createPlayerSlot(profile: PlayerProfile, entityId: number): PlayerSlotRecord {
  const playerId = `player-${randomUUID()}`;
  return {
    id: playerId,
    entityId,
    profile,
    state: createInitialPlayerState(playerId),
    commandQueue: createPlayerCommandQueue(),
    anchoredToChunkView: false,
  };
}

function createRemotePlayerEntitySnapshot(player: PlayerSlotRecord): EntitySnapshot {
  const state = player.state;
  const position = state.movementBody?.position ?? state.position;
  return {
    id: player.entityId,
    uuid: `mclone:player/${player.id}`,
    typeId: PLAYER_ENTITY_TYPE_ID,
    category: "misc",
    chunkX: SectionPos.blockToSectionCoord(position.x),
    chunkZ: SectionPos.blockToSectionCoord(position.z),
    position: {
      x: position.x,
      y: position.y,
      z: position.z,
    },
    rotation: {
      yaw: state.rotation.yaw,
      pitch: state.rotation.pitch,
    },
    width: 0.6,
    height: 1.8,
    onGround: state.movementBody?.onGround ?? true,
    data: {
      kind: PLAYER_ENTITY_DATA_KIND,
      playerId: player.id,
      name: player.profile.name,
    },
  };
}

function createRemotePlayerEntitySnapshotMessage(player: PlayerSlotRecord): Extract<WorldHostMessage, { type: "entity_snapshot" }> {
  return {
    type: "entity_snapshot",
    entity: createRemotePlayerEntitySnapshot(player),
  };
}

function getSessionPlayer(world: SharedWorldRecord, session: SessionRecord): PlayerSlotRecord {
  const player = world.playerSlots.get(session.playerId);
  if (player === undefined) {
    throw new GeneratedWorldRemoteServiceError(`Unknown player slot ${session.playerId}`, "unknown_session", 404);
  }

  return player;
}

function createSessionStateMessage(
  session: SessionRecord,
  world: SharedWorldRecord,
  resumed: boolean,
): Extract<WorldHostMessage, { type: "session_state" }> {
  const player = getSessionPlayer(world, session);
  const state: ClientSessionState = {
    sessionId: session.id,
    playerId: player.id,
    playerProfile: player.profile,
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

function createPlayerStateMessage(player: PlayerSlotRecord): Extract<WorldHostMessage, { type: "player_state" }> {
  return {
    type: "player_state",
    state: player.state,
  };
}

function enqueuePlayerStateMessage(session: SessionRecord, player: PlayerSlotRecord): void {
  session.pendingMessages = session.pendingMessages.filter((message) => message.type !== "player_state");
  session.pendingMessages.push(createPlayerStateMessage(player));
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
      case "world_progress":
      case "world_perf":
        break;
      case "entity_snapshot":
        world.loadedEntitySnapshots.set(message.entity.id, message.entity);
        break;
      case "entity_update": {
        const existing = world.loadedEntitySnapshots.get(message.update.id);
        if (existing !== undefined) {
          world.loadedEntitySnapshots.set(message.update.id, applyEntityUpdateToSnapshot(existing, message.update));
        }
        break;
      }
      case "entity_remove":
        world.loadedEntitySnapshots.delete(message.entityId);
        break;
      case "chunk_snapshot":
        world.loadedSnapshots.set(chunkKey(message.snapshot.chunkX, message.snapshot.chunkZ), message.snapshot);
        world.collisionLevel.applyPackedChunkSnapshot(message.snapshot);
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
        {
          const key = chunkKey(message.chunkX, message.chunkZ);
          world.loadedSnapshots.delete(key);
          world.collisionLevel.applyChunkUnload(message.chunkX, message.chunkZ);
          for (const [entityId, entity] of world.loadedEntitySnapshots) {
            if (entitySnapshotChunkKey(entity) === key) {
              world.loadedEntitySnapshots.delete(entityId);
            }
          }
        }
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

function getVisibleEntitySnapshotsForChunk(
  world: SharedWorldRecord,
  key: string,
  excludePlayerId?: string,
): readonly EntitySnapshot[] {
  const snapshots = [
    ...world.loadedEntitySnapshots.values(),
    ...[...world.playerSlots.values()]
      .filter((player) => player.anchoredToChunkView && player.id !== excludePlayerId)
      .map((player) => createRemotePlayerEntitySnapshot(player)),
  ];

  return snapshots
    .filter((entity) => entitySnapshotChunkKey(entity) === key)
    .sort((left, right) => left.id - right.id);
}

function getEntitySnapshotMessagesForChunk(
  world: SharedWorldRecord,
  key: string,
  excludePlayerId?: string,
): Extract<WorldHostMessage, { type: "entity_snapshot" }>[] {
  return getVisibleEntitySnapshotsForChunk(world, key, excludePlayerId)
    .map((entity) => ({
      type: "entity_snapshot",
      entity,
    }));
}

function getEntityRemoveMessagesForChunk(
  world: SharedWorldRecord,
  key: string,
  excludePlayerId?: string,
): Extract<WorldHostMessage, { type: "entity_remove" }>[] {
  return getVisibleEntitySnapshotsForChunk(world, key, excludePlayerId)
    .map((entity) => createEntityRemoveMessage(entity, "unloaded_to_chunk"));
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
    return {
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages(await this.openWorldMessages(request, resumeSessionId)),
    };
  }

  public async openWorldMessages(request: OpenWorldRequest, resumeSessionId?: string): Promise<readonly WorldHostMessage[]> {
    if (resumeSessionId !== undefined) {
      return await this.resumeWorldMessages(request, resumeSessionId);
    }

    const world = await this.getOrCreateWorld(request);
    const sessionId = randomUUID();
    const player = createPlayerSlot(normalizePlayerProfile(request.playerProfile), world.nextPlayerEntityId--);
    world.playerSlots.set(player.id, player);
    const session: SessionRecord = {
      id: sessionId,
      playerId: player.id,
      worldSaveId: world.saveId,
      openWorldRequest: request,
      chunkView: undefined,
      visibleChunks: new Set<string>(),
      revision: 0,
      pendingMessages: [],
    };
    this.sessions.set(sessionId, session);
    world.sessionIds.add(sessionId);
    return [
      world.worldOpened,
      createSessionStateMessage(session, world, false),
      createPlayerStateMessage(player),
    ];
  }

  public async setChunkView(sessionId: string, request: SetChunkViewRequest): Promise<SessionChunkViewResponse> {
    return {
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages(await this.setChunkViewMessages(sessionId, request)),
    };
  }

  public async setChunkViewMessages(sessionId: string, request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
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
      const player = getSessionPlayer(queuedWorld, queuedSession);

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
      if (!player.anchoredToChunkView) {
        player.state = anchorPlayerStateToChunkView(
          player.state,
          createSessionChunkViewState(request)!,
          player.state.tick,
        );
        player.anchoredToChunkView = true;
        responseMessages.push(createPlayerStateMessage(player));
        this.enqueueRemotePlayerEntitySnapshot(queuedWorld, player, queuedSession.id);
      }

      for (const key of previousVisibleChunks) {
        if (!visibleChunks.has(key)) {
          responseMessages.push(...getEntityRemoveMessagesForChunk(queuedWorld, key, queuedSession.playerId));
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
          queuedSession.pendingMessages.push(...getEntitySnapshotMessagesForChunk(queuedWorld, key, queuedSession.playerId));
        }
      }

      queuedSession.visibleChunks = visibleChunks;
      return responseMessages;
    });
  }

  public async setPlayerInput(sessionId: string, request: SetPlayerInputRequest): Promise<SessionPlayerInputResponse> {
    return {
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages(await this.setPlayerInputMessages(sessionId, request)),
    };
  }

  public async setPlayerInputMessages(sessionId: string, request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world session ${sessionId}`, "unknown_session", 404);
    }

    const world = this.worlds.get(session.worldSaveId);
    if (world === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world ${session.worldSaveId}`, "unknown_session", 404);
    }
    const player = getSessionPlayer(world, session);

    if (enqueuePlayerInputCommand(player.commandQueue, player.state, request.input)) {
      session.revision++;
    }
    return [
      createSessionStateMessage(session, world, false),
    ];
  }

  public async pollUpdates(sessionId: string, request: PollWorldUpdatesRequest): Promise<SessionPollUpdatesResponse> {
    return {
      protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
      messages: serializeWorldHostMessages(await this.drainSessionUpdates(sessionId, request)),
    };
  }

  public async drainSessionUpdates(
    sessionId: string,
    request: PollWorldUpdatesRequest = { type: "poll_world_updates" },
  ): Promise<readonly WorldHostMessage[]> {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world session ${sessionId}`, "unknown_session", 404);
    }

    const world = this.worlds.get(session.worldSaveId);
    if (world === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world ${session.worldSaveId}`, "unknown_session", 404);
    }

    await this.drainAuthoritativeHostMessages(world);
    return drainPendingMessages(session, request.maxMessages);
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
        const world = this.worlds.get(session.worldSaveId);
        if (world === undefined) {
          continue;
        }
        const player = getSessionPlayer(world, session);
        const previousEntity = player.anchoredToChunkView ? createRemotePlayerEntitySnapshot(player) : undefined;
        const nextPlayerState = tickPlayerStateWithCommandQueue(
          player.state,
          player.commandQueue,
          this.currentTick,
          createSharedWorldCollisionWorld(world),
        );
        if (nextPlayerState !== player.state) {
          player.state = nextPlayerState;
          enqueuePlayerStateMessage(session, player);
          if (previousEntity !== undefined) {
            this.enqueueRemotePlayerEntityUpdate(world, previousEntity, player, session.id);
          }
        }
      }
    }
  }

  private isPendingEntityLifecycleMessageForId(message: WorldHostMessage, entityId: number): boolean {
    return (message.type === "entity_snapshot" && message.entity.id === entityId)
      || (message.type === "entity_update" && message.update.id === entityId)
      || (message.type === "entity_remove" && message.entityId === entityId);
  }

  private enqueueRemotePlayerEntitySnapshot(
    world: SharedWorldRecord,
    player: PlayerSlotRecord,
    excludeSessionId?: string,
  ): void {
    if (!player.anchoredToChunkView) {
      return;
    }

    const message = createRemotePlayerEntitySnapshotMessage(player);
    const key = entitySnapshotChunkKey(message.entity);
    for (const session of this.getWorldSessions(world)) {
      if (session.id === excludeSessionId) {
        continue;
      }

      if (session.visibleChunks.has(key)) {
        session.pendingMessages = session.pendingMessages.filter(
          (pending) => !this.isPendingEntityLifecycleMessageForId(pending, player.entityId),
        );
        session.pendingMessages.push(message);
      }
    }
  }

  private enqueueRemotePlayerEntityUpdate(
    world: SharedWorldRecord,
    previousEntity: EntitySnapshot,
    player: PlayerSlotRecord,
    excludeSessionId?: string,
  ): void {
    if (!player.anchoredToChunkView) {
      return;
    }

    const nextEntity = createRemotePlayerEntitySnapshot(player);
    const previousKey = entitySnapshotChunkKey(previousEntity);
    const nextKey = entitySnapshotChunkKey(nextEntity);
    for (const session of this.getWorldSessions(world)) {
      if (session.id === excludeSessionId) {
        continue;
      }

      const sawPrevious = session.visibleChunks.has(previousKey);
      const seesNext = session.visibleChunks.has(nextKey);
      if (sawPrevious && seesNext) {
        session.pendingMessages = session.pendingMessages.filter(
          (pending) => pending.type !== "entity_update" || pending.update.id !== player.entityId,
        );
        session.pendingMessages.push(createEntityUpdateMessage(nextEntity));
      } else if (sawPrevious) {
        session.pendingMessages = session.pendingMessages.filter(
          (pending) => !this.isPendingEntityLifecycleMessageForId(pending, player.entityId),
        );
        session.pendingMessages.push(createEntityRemoveMessage(previousEntity, "changed_chunk"));
      } else if (seesNext) {
        session.pendingMessages = session.pendingMessages.filter(
          (pending) => !this.isPendingEntityLifecycleMessageForId(pending, player.entityId),
        );
        session.pendingMessages.push({
          type: "entity_snapshot",
          entity: nextEntity,
        });
      }
    }
  }

  public dropSession(sessionId: string): void {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      return;
    }

    this.sessions.delete(sessionId);
    const world = this.worlds.get(session.worldSaveId);
    if (world === undefined) {
      return;
    }

    world.sessionIds.delete(sessionId);
    const player = world.playerSlots.get(session.playerId);
    if (player === undefined) {
      return;
    }

    world.playerSlots.delete(session.playerId);
    if (!player.anchoredToChunkView) {
      return;
    }

    const entity = createRemotePlayerEntitySnapshot(player);
    const key = entitySnapshotChunkKey(entity);
    const message = createEntityRemoveMessage(entity, "unloaded_with_player");
    for (const remainingSession of this.getWorldSessions(world)) {
      if (!remainingSession.visibleChunks.has(key)) {
        continue;
      }

      remainingSession.pendingMessages = remainingSession.pendingMessages.filter(
        (pending) => !this.isPendingEntityLifecycleMessageForId(pending, player.entityId),
      );
      remainingSession.pendingMessages.push(message);
    }
  }

  public clearSessions(): void {
    this.sessions.clear();
    for (const world of this.worlds.values()) {
      world.sessionIds.clear();
      world.playerSlots.clear();
      world.aggregateChunkView = undefined;
    }
  }

  public dispose(): void {
    if (this.tickTimer !== undefined) {
      clearInterval(this.tickTimer);
    }
    for (const world of this.worlds.values()) {
      world.host.close?.();
    }
  }

  private async resumeWorldMessages(request: OpenWorldRequest, sessionId: string): Promise<readonly WorldHostMessage[]> {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      throw new GeneratedWorldRemoteServiceError(`Unknown world session ${sessionId}`, "unknown_session", 404);
    }

    if (!isSameOpenWorldRequest(session.openWorldRequest, request)) {
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
    const player = getSessionPlayer(world, session);

    const messages: WorldHostMessage[] = [
      world.worldOpened,
      createSessionStateMessage(session, world, true),
      createPlayerStateMessage(player),
    ];
    for (const key of session.visibleChunks) {
      const snapshot = world.loadedSnapshots.get(key);
      if (snapshot !== undefined) {
        messages.push({
          type: "chunk_snapshot",
          snapshot,
        });
      }
      messages.push(...getEntitySnapshotMessagesForChunk(world, key, session.playerId));
    }

    return messages;
  }

  private async getOrCreateWorld(request: OpenWorldRequest): Promise<SharedWorldRecord> {
    const saveId = createGeneratedWorldSaveId(request.seed, request.preset, request.config);
    const existing = this.worlds.get(saveId);
    if (existing !== undefined) {
      return existing;
    }

    const pending = this.pendingWorlds.get(saveId);
    if (pending !== undefined) {
      return await pending;
    }

    const creation = (async () => {
      const collisionBlocks = registerGeneratedRenderBlocks();
      const host = this.createSessionHost(request);
      const messages = await host.openWorld(request);
      const worldOpened = extractWorldOpened(messages);
      const world: SharedWorldRecord = {
        saveId,
        host,
        worldOpened,
        collisionLevel: createRemoteCollisionLevel(request, worldOpened, collisionBlocks),
        sessionIds: new Set<string>(),
        playerSlots: new Map<string, PlayerSlotRecord>(),
        loadedSnapshots: new Map<string, PackedChunkSnapshot>(),
        loadedEntitySnapshots: new Map<number, EntitySnapshot>(),
        nextPlayerEntityId: -1,
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
        case "world_progress":
          for (const session of this.getWorldSessions(world)) {
            session.pendingMessages = session.pendingMessages.filter((pending) => pending.type !== "world_progress");
            session.pendingMessages.push(message);
          }
          break;
        case "world_perf":
          for (const session of this.getWorldSessions(world)) {
            session.pendingMessages = session.pendingMessages.filter((pending) => pending.type !== "world_perf");
            session.pendingMessages.push(message);
          }
          break;
        case "entity_snapshot": {
          const key = entitySnapshotChunkKey(message.entity);
          world.loadedEntitySnapshots.set(message.entity.id, message.entity);
          for (const session of this.getWorldSessions(world)) {
            if (session.visibleChunks.has(key)) {
              session.pendingMessages.push(message);
            }
          }
          break;
        }
        case "entity_update": {
          const previous = world.loadedEntitySnapshots.get(message.update.id);
          if (previous === undefined) {
            break;
          }

          const next = applyEntityUpdateToSnapshot(previous, message.update);
          world.loadedEntitySnapshots.set(next.id, next);
          const previousKey = entitySnapshotChunkKey(previous);
          const nextKey = entitySnapshotChunkKey(next);
          const updateMessage = createEntityUpdateMessage(next);
          const snapshotMessage = {
            type: "entity_snapshot",
            entity: next,
          } satisfies Extract<WorldHostMessage, { type: "entity_snapshot" }>;
          const removeMessage = createEntityRemoveMessage(previous, "changed_chunk");
          for (const session of this.getWorldSessions(world)) {
            const sawPrevious = session.visibleChunks.has(previousKey);
            const seesNext = session.visibleChunks.has(nextKey);
            if (sawPrevious && seesNext) {
              session.pendingMessages.push(updateMessage);
            } else if (sawPrevious) {
              session.pendingMessages.push(removeMessage);
            } else if (seesNext) {
              session.pendingMessages.push(snapshotMessage);
            }
          }
          break;
        }
        case "entity_remove": {
          const previous = world.loadedEntitySnapshots.get(message.entityId);
          world.loadedEntitySnapshots.delete(message.entityId);
          if (previous === undefined) {
            for (const session of this.getWorldSessions(world)) {
              session.pendingMessages.push(message);
            }
            break;
          }

          const key = entitySnapshotChunkKey(previous);
          for (const session of this.getWorldSessions(world)) {
            if (session.visibleChunks.has(key)) {
              session.pendingMessages.push(message);
            }
          }
          break;
        }
        case "chunk_snapshot": {
          const key = chunkKey(message.snapshot.chunkX, message.snapshot.chunkZ);
          world.loadedSnapshots.set(key, message.snapshot);
          world.collisionLevel.applyPackedChunkSnapshot(message.snapshot);
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
          world.collisionLevel.applyChunkUnload(message.chunkX, message.chunkZ);
          for (const [entityId, entity] of world.loadedEntitySnapshots) {
            if (entitySnapshotChunkKey(entity) === key) {
              world.loadedEntitySnapshots.delete(entityId);
            }
          }
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
    const engineConfig = normalizeWorldEngineConfig(request.config);
    return createGeneratedWorldHostForRequest(request, {
      chunkViewScheduling: "cooperative",
      worldStorage: request.storageMode === "none" ? undefined : this.worldStorage,
      lightingService: engineConfig.lightingMode === "none" ? undefined : createNodeLightingService(),
    });
  }
}

const WEB_SOCKET_ACCEPT_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const WEB_SOCKET_PUSH_MAX_MESSAGES = 16;

function findSessionId(messages: readonly WorldHostMessage[]): string | undefined {
  for (let index = messages.length - 1; index >= 0; index--) {
    const message = messages[index];
    if (message?.type === "session_state") {
      return message.state.sessionId;
    }
  }

  return undefined;
}

function isPerformanceOnlyPush(messages: readonly WorldHostMessage[]): boolean {
  return messages.length > 0 && messages.every((message) => message.type === "world_perf");
}

function createProtocolVersionMismatchError(protocolVersion: unknown): GeneratedWorldRemoteServiceError {
  return new GeneratedWorldRemoteServiceError(
    `protocolVersion ${String(protocolVersion)} did not match server protocol version ${WORLD_REMOTE_PROTOCOL_VERSION.toString()}`,
    "protocol_version_mismatch",
    409,
    WORLD_REMOTE_PROTOCOL_VERSION,
  );
}

function createMalformedSocketMessageError(message: string): GeneratedWorldRemoteServiceError {
  return new GeneratedWorldRemoteServiceError(message, "malformed_message", 400);
}

function createUnsupportedSocketMessageError(message: string): GeneratedWorldRemoteServiceError {
  return new GeneratedWorldRemoteServiceError(message, "unsupported_message", 400);
}

function worldSocketErrorCode(error: unknown): WorldRemoteErrorCode {
  if (error instanceof GeneratedWorldRemoteServiceError) {
    return error.code;
  }
  return "world_request_mismatch";
}

function worldSocketErrorExpectedProtocolVersion(error: unknown): number | undefined {
  return error instanceof GeneratedWorldRemoteServiceError ? error.expectedProtocolVersion : undefined;
}

function createWebSocketAccept(key: string): string {
  return createHash("sha1").update(`${key}${WEB_SOCKET_ACCEPT_GUID}`).digest("base64");
}

function sendUpgradeError(socket: Duplex, statusCode: number, statusText: string): void {
  socket.end(`HTTP/1.1 ${statusCode.toString()} ${statusText}\r\nConnection: close\r\n\r\n`);
}

class NodeTextWebSocket {
  private pending: Buffer<ArrayBufferLike> = Buffer.alloc(0);
  private closed = false;
  public onText: (text: string) => void = () => {};
  public onClose: () => void = () => {};

  public constructor(private readonly socket: Duplex) {
    socket.on("data", (chunk) => {
      this.readData(typeof chunk === "string" ? Buffer.from(chunk) : chunk);
    });
    socket.on("close", () => {
      this.closed = true;
      this.onClose();
    });
    socket.on("error", () => {
      this.close();
    });
  }

  public sendJson(frame: WorldSocketServerFrame): void {
    this.sendText(JSON.stringify(frame));
  }

  public close(): void {
    if (this.closed) {
      return;
    }

    this.closed = true;
    this.sendFrame(0x8, Buffer.alloc(0));
    this.socket.end();
  }

  public readBufferedData(chunk: Buffer<ArrayBufferLike>): void {
    this.readData(chunk);
  }

  private readData(chunk: Buffer<ArrayBufferLike>): void {
    if (this.closed) {
      return;
    }

    this.pending = this.pending.length === 0 ? chunk : Buffer.concat([this.pending, chunk]);
    while (this.tryReadFrame()) {}
  }

  private tryReadFrame(): boolean {
    if (this.pending.length < 2) {
      return false;
    }

    const first = this.pending[0]!;
    const second = this.pending[1]!;
    const opcode = first & 0x0f;
    const masked = (second & 0x80) !== 0;
    let offset = 2;
    let payloadLength = second & 0x7f;
    if (payloadLength === 126) {
      if (this.pending.length < offset + 2) {
        return false;
      }
      payloadLength = this.pending.readUInt16BE(offset);
      offset += 2;
    } else if (payloadLength === 127) {
      if (this.pending.length < offset + 8) {
        return false;
      }
      const largeLength = this.pending.readBigUInt64BE(offset);
      if (largeLength > BigInt(Number.MAX_SAFE_INTEGER)) {
        this.close();
        return false;
      }
      payloadLength = Number(largeLength);
      offset += 8;
    }

    if (!masked) {
      this.close();
      return false;
    }
    if (this.pending.length < offset + 4 + payloadLength) {
      return false;
    }

    const mask = this.pending.subarray(offset, offset + 4);
    offset += 4;
    const payload = Buffer.from(this.pending.subarray(offset, offset + payloadLength));
    this.pending = this.pending.subarray(offset + payloadLength);
    for (let index = 0; index < payload.length; index++) {
      payload[index] = payload[index]! ^ mask[index % 4]!;
    }

    switch (opcode) {
      case 0x1:
        this.onText(payload.toString("utf8"));
        break;
      case 0x8:
        this.close();
        break;
      case 0x9:
        this.sendFrame(0xA, payload);
        break;
      case 0xA:
        break;
      default:
        this.close();
        break;
    }

    return true;
  }

  private sendText(text: string): void {
    this.sendFrame(0x1, Buffer.from(text, "utf8"));
  }

  private sendFrame(opcode: number, payload: Buffer<ArrayBufferLike>): void {
    if (this.socket.destroyed) {
      return;
    }

    let header: Buffer;
    if (payload.length <= 125) {
      header = Buffer.from([0x80 | opcode, payload.length]);
    } else if (payload.length <= 65_535) {
      header = Buffer.alloc(4);
      header[0] = 0x80 | opcode;
      header[1] = 126;
      header.writeUInt16BE(payload.length, 2);
    } else {
      header = Buffer.alloc(10);
      header[0] = 0x80 | opcode;
      header[1] = 127;
      header.writeBigUInt64BE(BigInt(payload.length), 2);
    }

    this.socket.write(Buffer.concat([header, payload]));
  }
}

function acceptTextWebSocket(request: IncomingMessage, socket: Duplex): NodeTextWebSocket | undefined {
  if (request.headers.upgrade?.toLowerCase() !== "websocket") {
    sendUpgradeError(socket, 400, "Bad Request");
    return undefined;
  }

  const key = request.headers["sec-websocket-key"];
  if (typeof key !== "string" || key.length === 0) {
    sendUpgradeError(socket, 400, "Bad Request");
    return undefined;
  }

  socket.write([
    "HTTP/1.1 101 Switching Protocols",
    "Upgrade: websocket",
    "Connection: Upgrade",
    `Sec-WebSocket-Accept: ${createWebSocketAccept(key)}`,
    "",
    "",
  ].join("\r\n"));
  return new NodeTextWebSocket(socket);
}

class GeneratedWorldWebSocketConnection {
  private sessionId: string | undefined;
  private nextSequence = 1;
  private closed = false;
  private pushPendingPromise: Promise<void> | undefined;

  public constructor(
    private readonly service: GeneratedWorldRemoteService,
    private readonly socket: NodeTextWebSocket,
    onClose: () => void,
  ) {
    this.socket.onText = (text) => {
      void this.handleText(text);
    };
    this.socket.onClose = () => {
      this.closed = true;
      onClose();
    };
  }

  public close(): void {
    this.closed = true;
    this.socket.close();
  }

  public async pushPending(): Promise<void> {
    if (this.closed || this.sessionId === undefined) {
      return;
    }
    if (this.pushPendingPromise !== undefined) {
      await this.pushPendingPromise;
      return;
    }

    const push = (async () => {
      if (this.sessionId === undefined) {
        return;
      }

      try {
        const messages = await this.service.drainSessionUpdates(this.sessionId, {
          type: "poll_world_updates",
          maxMessages: WEB_SOCKET_PUSH_MAX_MESSAGES,
        });
        if (messages.length > 0 && !isPerformanceOnlyPush(messages)) {
          this.send({
            protocolVersion: WORLD_REMOTE_PROTOCOL_VERSION,
            kind: "push",
            sequence: this.nextSequence++,
            messages: serializeWorldHostMessages(messages),
          });
        }
      } catch (error) {
        if (error instanceof GeneratedWorldRemoteServiceError && error.code === "unknown_session") {
          this.sessionId = undefined;
        }
        this.sendError(undefined, error);
      }
    })();
    this.pushPendingPromise = push;
    try {
      await push;
    } finally {
      if (this.pushPendingPromise === push) {
        this.pushPendingPromise = undefined;
      }
    }
  }

  private async handleText(text: string): Promise<void> {
    let frame: Partial<WorldSocketClientFrame> & { readonly kind?: unknown; readonly requestId?: unknown };
    try {
      frame = JSON.parse(text) as Partial<WorldSocketClientFrame> & { readonly kind?: unknown; readonly requestId?: unknown };
    } catch {
      this.sendError(undefined, createMalformedSocketMessageError("Malformed WebSocket JSON frame"));
      return;
    }

    try {
      this.assertProtocolVersion(frame.protocolVersion);
      if (frame.kind === "ack") {
        return;
      }
      if (frame.kind !== "request") {
        throw createUnsupportedSocketMessageError(`Unsupported WebSocket frame kind ${String(frame.kind)}`);
      }
      if (typeof frame.requestId !== "number" || !Number.isSafeInteger(frame.requestId)) {
        throw createMalformedSocketMessageError("WebSocket request frame requires an integer requestId");
      }
      if (frame.message === undefined) {
        throw createMalformedSocketMessageError("WebSocket request frame requires a message");
      }

      await this.handleRequest(frame as Extract<WorldSocketClientFrame, { kind: "request" }>);
    } catch (error) {
      if (error instanceof GeneratedWorldRemoteServiceError && error.code === "unknown_session") {
        this.sessionId = undefined;
      }
      this.sendError(typeof frame.requestId === "number" ? frame.requestId : undefined, error);
    }
  }

  private async handleRequest(frame: Extract<WorldSocketClientFrame, { kind: "request" }>): Promise<void> {
    const message = deserializeWorldClientMessage(frame.message);
    let messages: readonly WorldHostMessage[];
    switch (message.type) {
      case "open_world":
        messages = await this.service.openWorldMessages(message, frame.resumeSessionId ?? this.sessionId);
        this.updateSessionId(messages);
        break;
      case "set_chunk_view":
        messages = await this.service.setChunkViewMessages(this.requireSessionId(), message);
        this.updateSessionId(messages);
        break;
      case "set_player_input":
        messages = await this.service.setPlayerInputMessages(this.requireSessionId(), message);
        this.updateSessionId(messages);
        break;
      case "poll_world_updates":
        messages = await this.service.drainSessionUpdates(this.requireSessionId(), message);
        break;
    }

    this.send({
      protocolVersion: WORLD_REMOTE_PROTOCOL_VERSION,
      kind: "response",
      requestId: frame.requestId,
      messages: serializeWorldHostMessages(messages),
    });
    await this.pushPending();
  }

  private assertProtocolVersion(protocolVersion: unknown): void {
    if (protocolVersion !== WORLD_REMOTE_PROTOCOL_VERSION) {
      throw createProtocolVersionMismatchError(protocolVersion);
    }
  }

  private requireSessionId(): string {
    if (this.sessionId === undefined) {
      throw new GeneratedWorldRemoteServiceError("WebSocket session has not opened a world", "unknown_session", 404);
    }

    return this.sessionId;
  }

  private updateSessionId(messages: readonly WorldHostMessage[]): void {
    const nextSessionId = findSessionId(messages);
    if (nextSessionId !== undefined) {
      this.sessionId = nextSessionId;
    }
  }

  private send(frame: WorldSocketServerFrame): void {
    if (!this.closed) {
      this.socket.sendJson(frame);
    }
  }

  private sendError(requestId: number | undefined, error: unknown): void {
    this.send({
      protocolVersion: WORLD_REMOTE_PROTOCOL_VERSION,
      kind: "error",
      ...(requestId === undefined ? {} : { requestId }),
      code: worldSocketErrorCode(error),
      message: formatUnknownError(error),
      ...(worldSocketErrorExpectedProtocolVersion(error) === undefined
        ? {}
        : { expectedProtocolVersion: worldSocketErrorExpectedProtocolVersion(error) }),
    });
  }
}

export class GeneratedWorldHttpServer {
  private readonly config: GeneratedWorldHttpServerConfig;
  private readonly service: GeneratedWorldRemoteService;
  private readonly server: Server;
  private readonly webSocketConnections = new Set<GeneratedWorldWebSocketConnection>();
  private readonly pushTimer: ReturnType<typeof setInterval>;

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
    this.server.on("upgrade", (request, socket, head) => {
      this.handleUpgrade(request, socket, head);
    });
    this.pushTimer = setInterval(() => {
      void this.pushWebSocketUpdates();
    }, 10);
    this.pushTimer.unref?.();
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
    clearInterval(this.pushTimer);
    for (const connection of this.webSocketConnections) {
      connection.close();
    }
    this.webSocketConnections.clear();
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

  public forceTick(tickCount = 1): void {
    this.service.forceTick(tickCount);
    void this.pushWebSocketUpdates();
  }

  public dropSession(sessionId: string): void {
    this.service.dropSession(sessionId);
    void this.pushWebSocketUpdates();
  }

  private handleUpgrade(request: IncomingMessage, socket: Duplex, head: Buffer<ArrayBufferLike>): void {
    const url = new URL(request.url ?? "/", "http://mclone.invalid");
    if (url.pathname !== "/api/world/socket") {
      sendUpgradeError(socket, 404, "Not Found");
      return;
    }

    const webSocket = acceptTextWebSocket(request, socket);
    if (webSocket === undefined) {
      return;
    }

    let connection: GeneratedWorldWebSocketConnection;
    connection = new GeneratedWorldWebSocketConnection(this.service, webSocket, () => {
      this.webSocketConnections.delete(connection);
    });
    this.webSocketConnections.add(connection);
    if (head.length > 0) {
      webSocket.readBufferedData(head);
    }
  }

  private async pushWebSocketUpdates(): Promise<void> {
    await Promise.all([...this.webSocketConnections].map((connection) => connection.pushPending()));
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
