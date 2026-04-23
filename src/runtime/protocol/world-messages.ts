import type { WorldSaveMetadata } from "../storage/world-storage";
import type { ChunkSnapshot } from "../../world/level/chunk-snapshot";

export type OpenWorldPreset = "default" | "browser_smoke";

export interface OpenWorldRequest {
  readonly type: "open_world";
  readonly seed: bigint;
  readonly preset: OpenWorldPreset;
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
  readonly saveId: string;
  readonly resumed: boolean;
  readonly revision: number;
  readonly chunkView?: SessionChunkViewState;
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
  readonly snapshot: ChunkSnapshot;
}

export interface ChunkUnloadMessage {
  readonly type: "chunk_unload";
  readonly chunkX: number;
  readonly chunkZ: number;
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
  | ChunkSnapshotMessage
  | ChunkUnloadMessage
  | WorldErrorMessage;
