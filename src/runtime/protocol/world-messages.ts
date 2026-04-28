import type { WorldSaveMetadata } from "../storage/world-storage";
import type { LightingServicePerformanceCounters } from "../lighting/lighting-protocol";
import type { PackedChunkLightDelta, PackedChunkSnapshot } from "../../world/level/packed-chunk-snapshot";

export type OpenWorldPreset = "default" | "browser_smoke" | "flat_grass" | "small_island";
export type WorldEngineLightingMode = "vanilla17" | "none";
export type WorldEngineLiquidSimulationMode = "vanilla17" | "none";
export type WorldStorageMode = "default" | "none";

export interface PlayerProfile {
  readonly name: string;
  readonly profileId?: string;
}

export const DEFAULT_PLAYER_NAME = "Player";
export const DEFAULT_PLAYER_PROFILE: PlayerProfile = { name: DEFAULT_PLAYER_NAME };

export function normalizePlayerProfile(profile: PlayerProfile | undefined): PlayerProfile {
  const name = profile?.name.trim() || DEFAULT_PLAYER_NAME;
  const profileId = profile?.profileId?.trim();
  return {
    name,
    ...(profileId === undefined || profileId.length === 0 ? {} : { profileId }),
  };
}

export interface WorldEngineConfig {
  readonly lightingMode?: WorldEngineLightingMode;
  readonly liquidSimulationMode?: WorldEngineLiquidSimulationMode;
}

export interface NormalizedWorldEngineConfig {
  readonly lightingMode: WorldEngineLightingMode;
  readonly liquidSimulationMode: WorldEngineLiquidSimulationMode;
}

export function normalizeWorldEngineConfig(config: WorldEngineConfig | undefined): NormalizedWorldEngineConfig {
  return {
    lightingMode: config?.lightingMode === "vanilla17" ? "vanilla17" : "none",
    liquidSimulationMode: config?.liquidSimulationMode === "none" ? "none" : "vanilla17",
  };
}

export function isDefaultWorldEngineConfig(config: WorldEngineConfig | undefined): boolean {
  const normalized = normalizeWorldEngineConfig(config);
  return normalized.lightingMode === "none" && normalized.liquidSimulationMode === "vanilla17";
}

export interface OpenWorldRequest {
  readonly type: "open_world";
  readonly seed: bigint;
  readonly preset: OpenWorldPreset;
  readonly config?: WorldEngineConfig;
  readonly storageMode?: WorldStorageMode;
  readonly playerProfile?: PlayerProfile;
}

export interface SetChunkViewRequest {
  readonly type: "set_chunk_view";
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
}

export interface PlayerInputCommand {
  readonly sequence: number;
  readonly moveX: number;
  readonly moveY: number;
  readonly moveZ: number;
  readonly yaw: number;
  readonly pitch: number;
  readonly clientTimeUs?: number;
  readonly commandQuantumUs?: number;
  readonly stepCount?: number;
  readonly buttons?: number;
  readonly edgeButtons?: number;
  readonly physicsRevision?: number;
  readonly collisionRevision?: number;
}

export interface SetPlayerInputRequest {
  readonly type: "set_player_input";
  readonly input: PlayerInputCommand;
}

export interface PollWorldUpdatesRequest {
  readonly type: "poll_world_updates";
  readonly maxMessages?: number;
}

export interface SessionChunkViewState {
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
}

export interface ClientSessionState {
  readonly sessionId: string;
  readonly playerId: string;
  readonly playerProfile: PlayerProfile;
  readonly saveId: string;
  readonly resumed: boolean;
  readonly revision: number;
  readonly chunkView?: SessionChunkViewState;
}

export type ClientMovementBodyMode = "ground" | "air" | "water" | "flying";

export interface ClientMovementBodySnapshot {
  readonly position: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };
  readonly velocity: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };
  readonly bounds: {
    readonly minX: number;
    readonly minY: number;
    readonly minZ: number;
    readonly maxX: number;
    readonly maxY: number;
    readonly maxZ: number;
  };
  readonly onGround: boolean;
  readonly mode: ClientMovementBodyMode;
  readonly jumpHeld: boolean;
  readonly physicsRevision: number;
  readonly collisionRevision?: number;
  readonly commandQuantumUs: number;
  readonly lastProcessedCommandSequence: number;
}

export interface ClientPlayerState {
  readonly playerId: string;
  readonly position: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };
  readonly rotation: {
    readonly yaw: number;
    readonly pitch: number;
  };
  readonly acknowledgedInputSequence: number;
  readonly tick: number;
  readonly revision: number;
  readonly movementBody?: ClientMovementBodySnapshot;
}

export type EntitySnapshotCategory =
  | "monster"
  | "creature"
  | "ambient"
  | "underground_water_creature"
  | "water_creature"
  | "water_ambient"
  | "misc";

export const PLAYER_ENTITY_TYPE_ID = "minecraft:player";
export const PLAYER_ENTITY_DATA_KIND = "player";
export const COW_ENTITY_TYPE_ID = "minecraft:cow";

export interface EntitySnapshot {
  readonly id: number;
  readonly uuid: string;
  readonly typeId: string;
  readonly category: EntitySnapshotCategory;
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly position: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };
  readonly rotation: {
    readonly yaw: number;
    readonly pitch: number;
  };
  readonly width: number;
  readonly height: number;
  readonly onGround: boolean;
  readonly age?: number;
  readonly data?: Readonly<Record<string, number | boolean | string>>;
}

export interface EntityUpdate {
  readonly id: number;
  readonly chunkX?: number;
  readonly chunkZ?: number;
  readonly position?: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };
  readonly rotation?: {
    readonly yaw: number;
    readonly pitch: number;
  };
  readonly width?: number;
  readonly height?: number;
  readonly onGround?: boolean;
  readonly age?: number;
  readonly data?: Readonly<Record<string, number | boolean | string>>;
}

export interface WorldOpenedMessage {
  readonly type: "world_opened";
  readonly minBuildHeight: number;
  readonly height: number;
  readonly saveMetadata: WorldSaveMetadata;
}

export interface SessionStateMessage {
  readonly type: "session_state";
  readonly state: ClientSessionState;
}

export interface PlayerStateMessage {
  readonly type: "player_state";
  readonly state: ClientPlayerState;
}

export interface ChunkSnapshotMessage {
  readonly type: "chunk_snapshot";
  readonly snapshot: PackedChunkSnapshot;
}

export interface ChunkUnloadMessage {
  readonly type: "chunk_unload";
  readonly chunkX: number;
  readonly chunkZ: number;
}

export interface ChunkLightDeltaMessage {
  readonly type: "chunk_light_delta";
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly light: PackedChunkLightDelta;
}

export interface EntitySnapshotMessage {
  readonly type: "entity_snapshot";
  readonly entity: EntitySnapshot;
}

export interface EntityUpdateMessage {
  readonly type: "entity_update";
  readonly update: EntityUpdate;
}

export interface EntityRemoveMessage {
  readonly type: "entity_remove";
  readonly entityId: number;
  readonly uuid?: string;
  readonly reason?: string;
}

export interface WorldProgressMessage {
  readonly type: "world_progress";
  readonly stage: string;
  readonly detail?: string;
  readonly current: number;
  readonly total: number;
}

export interface WorldgenPhasePerformanceCounters {
  readonly count: number;
  readonly totalMs: number;
  readonly maxMs: number;
}

export interface WorldgenPerformanceCounters {
  readonly phases: Readonly<Record<string, WorldgenPhasePerformanceCounters>>;
  readonly counts: Readonly<Record<string, number>>;
  readonly maxPendingMessages: number;
  readonly currentPendingMessages: number;
  readonly activeChunkViewJobRevision?: number;
}

export interface WorldPerformanceSnapshot {
  readonly lighting?: LightingServicePerformanceCounters;
  readonly worldgen?: WorldgenPerformanceCounters;
}

export interface WorldPerformanceMessage {
  readonly type: "world_perf";
  readonly performance: WorldPerformanceSnapshot;
}

export interface WorldErrorMessage {
  readonly type: "world_error";
  readonly message: string;
}

export type WorldClientMessage = OpenWorldRequest | SetChunkViewRequest | SetPlayerInputRequest | PollWorldUpdatesRequest;
export type WorldHostMessage =
  | WorldOpenedMessage
  | SessionStateMessage
  | PlayerStateMessage
  | EntitySnapshotMessage
  | EntityUpdateMessage
  | EntityRemoveMessage
  | ChunkSnapshotMessage
  | ChunkLightDeltaMessage
  | ChunkUnloadMessage
  | WorldProgressMessage
  | WorldPerformanceMessage
  | WorldErrorMessage;
